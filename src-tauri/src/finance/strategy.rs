//! Pilar C do redesign carro/dinastias: planos estratégicos de longo prazo (3
//! temporadas) por equipe. Em vez de a equipe reagir ano a ano (o que fazia a
//! estratégia oscilar e os títulos parecerem aleatórios), cada equipe se
//! compromete com um arco de investimento por várias temporadas — é isso que
//! permite build-ups sustentados e, com o Pilar B (sem teto), dinastias.
//!
//! O plano vive numa tabela lateral (`team_strategic_plan`) e apenas DERIVA o
//! campo `season_strategy` que o `cashflow::season_strategy_bias` já consome — o
//! resto do pipeline financeiro fica intacto.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::db::queries::teams as team_queries;
use crate::finance::planning::category_finance_scale;
use crate::models::team::Team;

/// Pilar D: categorias premium que sustentam dinastias (3 equipes elite por classe).
/// GT3/GT4/LMP2 standalone entram; rookie/amador/bmw não (lá o mérito ainda decide).
const PREMIUM_ELITE_CATEGORIES: &[&str] =
    &["production_challenger", "gt3", "gt4", "lmp2", "endurance"];

/// "Elite score" para o ranking de dinastia dentro da classe. Marcas reais (Ferrari,
/// Porsche, Mercedes…) recebem um bônus alto → ficam FIXAS como elite onde existem
/// (GT3/Endurance real, atendendo ao pedido de "manter forte equipes reais"); onde
/// não há marca real (Production e grids fictícios) o critério é reputação + títulos
/// → a elite emerge por mérito (auto).
fn elite_score(team: &Team) -> f64 {
    let real_bonus = if team.marca.is_some() { 1000.0 } else { 0.0 };
    team.reputacao + (team.historico_titulos_construtores as f64) * 2.0 + real_bonus
}

/// Designa as 3 equipes elite por classe nas categorias premium (Pilar D). As elites
/// recebem o plano `elite_dominance` + piso de recursos → sustentam carro no topo,
/// gerando dinastias com sustos (o título roda entre as 3 por piloto + sorte).
pub fn designate_elite_teams(teams: &[Team]) -> HashSet<String> {
    let mut by_class: HashMap<(&str, Option<&str>), Vec<&Team>> = HashMap::new();
    for team in teams {
        if !team.ativa || !PREMIUM_ELITE_CATEGORIES.contains(&team.categoria.as_str()) {
            continue;
        }
        by_class
            .entry((team.categoria.as_str(), team.classe.as_deref()))
            .or_default()
            .push(team);
    }
    let mut elites = HashSet::new();
    for (_, mut group) in by_class {
        // Só há "top-3" numa classe minimamente cheia. Classes reais premium têm 6
        // equipes; abaixo de 4 (grid encolhido/edge) ninguém é designado, evitando
        // "todo mundo é elite" em classes pequenas.
        if group.len() < 4 {
            continue;
        }
        group.sort_by(|a, b| {
            elite_score(b)
                .partial_cmp(&elite_score(a))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for team in group.into_iter().take(3) {
            elites.insert(team.id.clone());
        }
    }
    elites
}

/// Piso de recursos das elites (Pilar D): garante um caixa mínimo (patrocínio de
/// dinastia) por categoria para sustentar investimento máximo no carro temporada
/// após temporada — é o motor que separa a elite do meio do grid.
pub fn apply_elite_resource_floor(team: &mut Team) {
    let floor = category_finance_scale(&team.categoria).expected_cash_midpoint();
    if team.cash_balance < floor {
        team.cash_balance = floor;
    }
}

/// Força de carro (0–100) a partir da qual a equipe se considera NO TOPO e passa a
/// sustentar em vez de empurrar título. Equivale a ~nível 8,5 de 10.
const TOP_CAR_STRENGTH: f64 = 75.0;

/// Escolhe um plano de longo prazo conforme o estado financeiro e o carro atual.
/// (`elite_dominance` não é escolhido aqui — é atribuído às elites no Pilar D.)
pub fn choose_strategic_plan(team: &Team) -> &'static str {
    match team.financial_state.as_str() {
        "crisis" | "collapse" | "pressured" => "rebuild",
        "elite" | "healthy" => {
            // Bem-financiada: mira janela de título enquanto o carro tem o que
            // crescer; já no topo, apenas sustenta.
            // O limiar era `car_performance < 18.0` sobre um domínio cujo MÁXIMO é 16 — sempre
            // verdadeiro, então `sustainable` era inalcançável e toda equipe saudável empurrava
            // título pra sempre. Em 0–100, "no topo" é de fato o topo.
            if team.car_strength() < TOP_CAR_STRENGTH {
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
        // Dinastia: agressividade de carro própria (a maior de todas), sustentada
        // pelo piso de recursos → as 3 elites separam do meio do grid (Pilar D).
        "elite_dominance" => "elite_dominance",
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
    let hash = team_id.bytes().fold(0usize, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(b as usize)
    });
    2 + (hash % 2) as i32
}

/// Avança o plano da equipe uma pré-temporada e retorna a `season_strategy`
/// daquela temporada. Adaptativo com guarda: planos agressivos
/// (title_push / elite_dominance) abortam em crise/colapso e re-escolhem.
/// Persiste o plano atualizado na tabela lateral. (Pilar C)
pub fn advance_strategic_plan(
    conn: &Connection,
    team: &Team,
    is_elite: bool,
) -> Result<&'static str, DbError> {
    let (mut plan_type, mut remaining) = team_queries::get_strategic_plan(conn, &team.id)?;

    if is_elite {
        // Elites de dinastia (Pilar D) rodam Elite Dominance de forma permanente: o
        // piso de recursos as protege de crise, então o arco só se renova (nunca
        // rebaixa de plano). É isso que sustenta a dinastia por várias temporadas.
        if plan_type != "elite_dominance" || remaining <= 0 {
            plan_type = "elite_dominance".to_string();
            remaining = plan_horizon_for(&team.id);
        }
    } else {
        // Não-elite: um plano agressivo herdado (ex.: caiu do tier elite) aborta e
        // se re-escolhe por mérito financeiro; o resto expira normalmente.
        let aggressive = plan_type == "title_push" || plan_type == "elite_dominance";
        let must_abort =
            aggressive && matches!(team.financial_state.as_str(), "crisis" | "collapse");
        if remaining <= 0 || must_abort {
            plan_type = choose_strategic_plan(team).to_string();
            remaining = plan_horizon_for(&team.id);
        }
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
        assert_eq!(
            choose_strategic_plan(&team("T1", 20_000_000.0, "healthy", 10.0)),
            "title_push"
        );
        assert_eq!(
            choose_strategic_plan(&team("T2", 20_000_000.0, "elite", 22.0)),
            "sustainable"
        );
        assert_eq!(
            choose_strategic_plan(&team("T3", -50_000.0, "crisis", 8.0)),
            "rebuild"
        );
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
        let s1 = advance_strategic_plan(&conn, &t, false).expect("advance");
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
        let s = advance_strategic_plan(&conn, &t, false).expect("advance");
        let (plan, _) = team_queries::get_strategic_plan(&conn, &t.id).unwrap();
        assert_eq!(
            plan, "rebuild",
            "plano agressivo deve abortar p/ rebuild em colapso"
        );
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
            pusher.season_strategy = advance_strategic_plan(&conn, &pusher, false).unwrap().to_string();
            apply_offseason_competitiveness_impact(&mut pusher, 0, crate::finance::focus::TeamFocus::MeioDeGrid);
            steady.season_strategy = advance_strategic_plan(&conn, &steady, false).unwrap().to_string();
            apply_offseason_competitiveness_impact(&mut steady, 0, crate::finance::focus::TeamFocus::MeioDeGrid);
        }

        assert!(
            pusher.car_performance > steady.car_performance,
            "title_push ({:.2}) deveria render mais carro que sustainable ({:.2})",
            pusher.car_performance,
            steady.car_performance
        );
    }

    fn premium_team(id: &str, categoria: &str, classe: Option<&str>, marca: Option<&str>, rep: f64) -> Team {
        let mut t = placeholder_team_from_db(
            id.to_string(),
            format!("Equipe {id}"),
            categoria.to_string(),
            "2026-01-01".to_string(),
        );
        t.classe = classe.map(str::to_string);
        t.marca = marca.map(str::to_string);
        t.reputacao = rep;
        t
    }

    #[test]
    fn elite_designation_pins_real_brands_and_top3_by_class() {
        // GT3: 4 marcas reais + 2 fictícias. As 3 elites devem ser marcas reais (bônus),
        // mesmo que uma fictícia tenha reputação alta.
        let teams = vec![
            premium_team("real_low", "gt3", None, Some("Ferrari"), 40.0),
            premium_team("real_a", "gt3", None, Some("Porsche"), 60.0),
            premium_team("real_b", "gt3", None, Some("Mercedes"), 55.0),
            premium_team("fake_high", "gt3", None, None, 99.0),
            premium_team("fake_mid", "gt3", None, None, 50.0),
            // Categoria não-premium: nunca vira elite.
            premium_team("rookie", "mazda_rookie", None, None, 90.0),
        ];
        let elites = designate_elite_teams(&teams);
        assert_eq!(elites.len(), 3, "3 elites por classe premium");
        assert!(elites.contains("real_low"), "marca real fixa mesmo com reputação baixa");
        assert!(elites.contains("real_a"));
        assert!(elites.contains("real_b"));
        assert!(!elites.contains("fake_high"), "fictícia não passa da marca real");
        assert!(!elites.contains("rookie"), "categoria não-premium não tem elite");
    }

    #[test]
    fn elite_designation_is_auto_by_reputation_without_real_brands() {
        // Production (sem marcas reais): as 3 de maior reputação viram elite.
        let teams = vec![
            premium_team("p1", "production_challenger", Some("mazda"), None, 70.0),
            premium_team("p2", "production_challenger", Some("mazda"), None, 65.0),
            premium_team("p3", "production_challenger", Some("mazda"), None, 60.0),
            premium_team("p4", "production_challenger", Some("mazda"), None, 55.0),
        ];
        let elites = designate_elite_teams(&teams);
        assert_eq!(elites.len(), 3);
        assert!(elites.contains("p1") && elites.contains("p2") && elites.contains("p3"));
        assert!(!elites.contains("p4"));
    }

    #[test]
    fn elite_team_keeps_elite_dominance_across_seasons() {
        let conn = setup_conn();
        let t = team("ELITE", 5_000_000.0, "healthy", 20.0);
        // Roda 5 offseasons como elite: o plano deve permanecer elite_dominance,
        // nunca "expirar" para outro (a dinastia é estável).
        for _ in 0..5 {
            let s = advance_strategic_plan(&conn, &t, true).expect("advance elite");
            assert_eq!(s, "elite_dominance", "elite_dominance tem season_strategy própria");
            let (plan, _) = team_queries::get_strategic_plan(&conn, &t.id).unwrap();
            assert_eq!(plan, "elite_dominance", "elite mantém o plano de dinastia");
        }
    }

    #[test]
    fn resource_floor_lifts_a_broke_elite_to_the_category_midpoint() {
        let mut broke = team("BROKE", -200_000.0, "collapse", 15.0);
        broke.categoria = "gt3".to_string();
        apply_elite_resource_floor(&mut broke);
        let midpoint = category_finance_scale("gt3").expected_cash_midpoint();
        assert!(
            (broke.cash_balance - midpoint).abs() < 1.0,
            "piso deve elevar o caixa ao midpoint da categoria ({midpoint:.0}), veio {:.0}",
            broke.cash_balance
        );
    }
}
