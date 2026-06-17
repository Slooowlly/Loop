//! Pilar C do redesign carro/dinastias: planos estratégicos de longo prazo (3
//! temporadas) por equipe. Em vez de a equipe reagir ano a ano (o que fazia a
//! estratégia oscilar e os títulos parecerem aleatórios), cada equipe se
//! compromete com um arco de investimento por várias temporadas — é isso que
//! permite build-ups sustentados e, com o Pilar B (sem teto), dinastias.
//!
//! O plano vive numa tabela lateral (`team_strategic_plan`) e apenas DERIVA o
//! campo `season_strategy` que o `cashflow::season_strategy_bias` já consome — o
//! resto do pipeline financeiro fica intacto.

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::db::queries::teams as team_queries;
use crate::models::team::Team;

/// Escolhe um plano de longo prazo conforme o estado financeiro e o carro atual.
/// (`elite_dominance` não é escolhido aqui — é atribuído às elites no Pilar D.)
pub fn choose_strategic_plan(team: &Team) -> &'static str {
    match team.financial_state.as_str() {
        "crisis" | "collapse" | "pressured" => "rebuild",
        "elite" | "healthy" => {
            // Bem-financiada: mira janela de título enquanto o carro tem o que
            // crescer; já no topo, apenas sustenta.
            if team.car_performance < 18.0 {
                "title_push"
            } else {
                "sustainable"
            }
        }
        _ => "sustainable", // stable / desconhecido
    }
}

/// Estratégia de temporada (`season_strategy`) derivada do plano e dos anos
/// restantes do arco. Mantém o `season_strategy_bias` do cashflow inalterado.
pub fn season_strategy_from_plan(plan_type: &str, remaining_years: i32) -> &'static str {
    match plan_type {
        "title_push" => "all_in",
        "elite_dominance" => "expansion",
        // Austeridade enquanto há arco pela frente; no último ano, empurra.
        "rebuild" => {
            if remaining_years > 1 {
                "austerity"
            } else {
                "expansion"
            }
        }
        _ => "balanced", // sustainable
    }
}

/// Duração do arco (2–3 temporadas) escalonada de forma determinística pelo id da
/// equipe, para os planos não expirarem todos na mesma temporada (mundo
/// desincronizado, sem precisar semear stagger em todo lugar).
fn plan_horizon_for(team_id: &str) -> i32 {
    let hash = team_id
        .bytes()
        .fold(0usize, |acc, b| acc.wrapping_mul(31).wrapping_add(b as usize));
    2 + (hash % 2) as i32
}

/// Avança o plano da equipe uma pré-temporada e retorna a `season_strategy`
/// daquela temporada. Adaptativo com guarda: planos agressivos
/// (title_push / elite_dominance) abortam em crise/colapso e re-escolhem.
/// Persiste o plano atualizado na tabela lateral. (Pilar C)
pub fn advance_strategic_plan(conn: &Connection, team: &Team) -> Result<&'static str, DbError> {
    let (mut plan_type, mut remaining) = team_queries::get_strategic_plan(conn, &team.id)?;

    let aggressive = plan_type == "title_push" || plan_type == "elite_dominance";
    let must_abort = aggressive && matches!(team.financial_state.as_str(), "crisis" | "collapse");

    if remaining <= 0 || must_abort {
        plan_type = choose_strategic_plan(team).to_string();
        remaining = plan_horizon_for(&team.id);
    }

    let season_strategy = season_strategy_from_plan(&plan_type, remaining);
    team_queries::set_strategic_plan(conn, &team.id, &plan_type, remaining - 1)?;
    Ok(season_strategy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finance::cashflow::apply_offseason_competitiveness_impact;
    use crate::models::team::placeholder_team_from_db;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE team_strategic_plan (
                team_id         TEXT PRIMARY KEY,
                plan_type       TEXT NOT NULL DEFAULT 'sustainable',
                remaining_years INTEGER NOT NULL DEFAULT 0
            );",
        )
        .expect("create plan table");
        conn
    }

    fn team(id: &str, cash: f64, state: &str, car: f64) -> Team {
        let mut t = placeholder_team_from_db(
            id.to_string(),
            "Equipe Plano".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        t.cash_balance = cash;
        t.financial_state = state.to_string();
        t.car_performance = car;
        t.engineering = 60.0;
        t.facilities = 58.0;
        t.reputacao = 52.0;
        t.morale = 1.0;
        t
    }

    #[test]
    fn rich_team_with_room_to_grow_picks_title_push() {
        assert_eq!(choose_strategic_plan(&team("T1", 20_000_000.0, "healthy", 10.0)), "title_push");
        assert_eq!(choose_strategic_plan(&team("T2", 20_000_000.0, "elite", 22.0)), "sustainable");
        assert_eq!(choose_strategic_plan(&team("T3", -50_000.0, "crisis", 8.0)), "rebuild");
    }

    #[test]
    fn season_strategy_derives_from_plan() {
        assert_eq!(season_strategy_from_plan("title_push", 3), "all_in");
        assert_eq!(season_strategy_from_plan("sustainable", 3), "balanced");
        assert_eq!(season_strategy_from_plan("rebuild", 3), "austerity");
        assert_eq!(season_strategy_from_plan("rebuild", 1), "expansion"); // último ano empurra
    }

    #[test]
    fn advance_decrements_and_rechooses_when_expired() {
        let conn = setup_conn();
        let t = team("T1", 20_000_000.0, "healthy", 10.0);
        // Sem registro: escolhe um plano novo e consome um ano.
        let s1 = advance_strategic_plan(&conn, &t).expect("advance");
        assert_eq!(s1, "all_in"); // title_push
        let (plan, remaining) = team_queries::get_strategic_plan(&conn, &t.id).unwrap();
        assert_eq!(plan, "title_push");
        assert!(remaining >= 1, "arco continua ativo");
    }

    #[test]
    fn aggressive_plan_aborts_to_rebuild_on_collapse() {
        let conn = setup_conn();
        let mut t = team("T1", 20_000_000.0, "healthy", 10.0);
        // Trava um title_push ativo.
        team_queries::set_strategic_plan(&conn, &t.id, "title_push", 2).unwrap();
        // Time despenca para colapso: o plano agressivo deve abortar.
        t.financial_state = "collapse".to_string();
        let s = advance_strategic_plan(&conn, &t).expect("advance");
        let (plan, _) = team_queries::get_strategic_plan(&conn, &t.id).unwrap();
        assert_eq!(plan, "rebuild", "plano agressivo deve abortar p/ rebuild em colapso");
        assert_eq!(s, "austerity");
    }

    #[test]
    fn committed_title_push_builds_more_car_than_reactive_sustainable() {
        let conn = setup_conn();
        // Mesma força financeira (moderada, para o clamp de ±3/temporada não
        // saturar e mascarar a diferença); um comprometido com title_push por 3
        // temporadas, outro com sustainable. O comprometido termina com carro
        // mais alto — é o efeito do arco sustentado (Pilar C).
        let mut pusher = team("PUSH", 1_500_000.0, "stable", 5.0);
        let mut steady = team("STDY", 1_500_000.0, "stable", 5.0);
        team_queries::set_strategic_plan(&conn, &pusher.id, "title_push", 3).unwrap();
        team_queries::set_strategic_plan(&conn, &steady.id, "sustainable", 3).unwrap();

        for _ in 0..3 {
            pusher.season_strategy = advance_strategic_plan(&conn, &pusher).unwrap().to_string();
            apply_offseason_competitiveness_impact(&mut pusher, 0);
            steady.season_strategy = advance_strategic_plan(&conn, &steady).unwrap().to_string();
            apply_offseason_competitiveness_impact(&mut steady, 0);
        }

        assert!(
            pusher.car_performance > steady.car_performance,
            "title_push ({:.2}) deveria render mais carro que sustainable ({:.2})",
            pusher.car_performance,
            steady.car_performance
        );
    }
}
