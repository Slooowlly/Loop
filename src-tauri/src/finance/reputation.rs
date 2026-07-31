//! Reputação viva (Sistema de equipes — ideia 1).
//!
//! Até aqui `reputacao` (0–100) era semeada na criação da equipe e só mudava em
//! degraus discretos: promoção (+), rebaixamento (−) e venda. Nunca por
//! resultado — apesar de já alimentar patrocínio ([`commands::race`]), teto
//! salarial ([`finance::salary`]), saúde financeira/estratégia
//! ([`finance::state`]) e o `elite_score` das dinastias ([`finance::strategy`]).
//!
//! Este módulo faz a reputação **se mover ao fim de cada temporada**, de forma
//! determinística (sem RNG — reproduzível no Monte Carlo), com duas forças:
//!
//! 1. **Alvo por desempenho.** A partir da posição no campeonato e do tamanho do
//!    grid (do `team_season_archive`), calcula-se uma "força na temporada" em
//!    `[-1, +1]` (1º = +1, último = −1, meio = 0). O alvo é `BASE + força·AMPLITUDE`.
//! 2. **Inércia + reversão à média.** A reputação CAMINHA até o alvo a
//!    `ADJUST_RATE` por ano — não pula. Quem para de ganhar decai suavemente para
//!    o meio (dinastia parece conquistada, não dada); um novato dominante leva
//!    várias temporadas vencendo para virar potência.
//!
//! Sensação travada com o usuário: inércia **Média** (`ADJUST_RATE = 0.20`) e
//! piso **Protegido** (`FLOOR = 25`) — a lanterna não afunda o bastante para
//! entrar na espiral receita→queda→venda, mantendo o grid estável.

use rusqlite::Connection;

use crate::db::queries::teams as team_queries;
use crate::models::season::Season;

/// Reputação de uma equipe mediana (meio do grid) em regime — âncora da reversão.
const BASE: f64 = 48.0;
/// Meia-amplitude do alvo por desempenho: campeão de grid cheio mira `BASE + AMPLITUDE`,
/// lanterna mira `BASE − AMPLITUDE` (antes do piso do alvo).
const AMPLITUDE: f64 = 45.0;
/// Fração da distância até o alvo percorrida por temporada (inércia "Média").
const ADJUST_RATE: f64 = 0.20;
/// Empurrão de legado do título: distingue "campeão" de "1º de um grid minúsculo".
const TITLE_KICKER: f64 = 2.0;
/// Piso do ALVO (não da reputação): mantém a suavização convergindo perto do piso
/// da banda em vez de mirar um valor muito abaixo dele.
const TARGET_FLOOR: f64 = 22.0;
/// Banda de operação da reputação. O piso protegido evita a espiral de morte do
/// fundo do grid; o teto deixa uma folga para o campeão perpétuo não "grudar".
const FLOOR: f64 = 25.0;
const CEIL: f64 = 98.0;

/// Alvo de reputação para uma temporada, dado o resultado no campeonato.
///
/// `position` é 1-indexado; `grid_size` é o número de equipes no grupo de
/// campeonato (categoria + classe). Grids degenerados (`<= 1`) não têm ordenação
/// significativa → força neutra (só a reversão à média age).
pub fn season_reputation_target(position: i32, grid_size: i32) -> f64 {
    let strength = if grid_size <= 1 {
        0.0
    } else {
        let clamped = position.clamp(1, grid_size);
        1.0 - 2.0 * ((clamped - 1) as f64) / ((grid_size - 1) as f64)
    };
    (BASE + strength * AMPLITUDE).max(TARGET_FLOOR)
}

/// Avança a reputação de uma equipe uma temporada em direção ao alvo de desempenho.
///
/// Puro e determinístico: mesmos argumentos → mesmo resultado. O campeão (1º)
/// ganha um pequeno bônus de legado; o valor final é preso à banda `[FLOOR, CEIL]`.
pub fn advance_team_reputation(current: f64, position: i32, grid_size: i32) -> f64 {
    let target = season_reputation_target(position, grid_size);
    let mut next = current + (target - current) * ADJUST_RATE;
    if position == 1 {
        next += TITLE_KICKER;
    }
    next.clamp(FLOOR, CEIL)
}

/// Ao fim da temporada, atualiza a reputação de toda equipe que competiu, lendo as
/// posições recém-arquivadas em `team_season_archive`.
///
/// Deve rodar DEPOIS de `archive_team_season` (posições conhecidas) e ANTES de
/// `run_promotion_relegation` (que soma o próprio degrau de reputação por cima) —
/// ver [`crate::evolution::pipeline`].
pub(crate) fn update_team_reputations_from_season(
    conn: &Connection,
    season: &Season,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT team_id, posicao_campeonato,
                    COUNT(*) OVER (PARTITION BY categoria, COALESCE(classe, '')) AS grid_size
             FROM team_season_archive
             WHERE season_number = ?1 AND posicao_campeonato IS NOT NULL",
        )
        .map_err(|e| format!("Falha ao preparar consulta de reputação: {e}"))?;
    let rows = stmt
        .query_map([season.numero], |row| {
            let team_id: String = row.get(0)?;
            let position: i32 = row.get(1)?;
            let grid_size: i32 = row.get(2)?;
            Ok((team_id, position, grid_size))
        })
        .map_err(|e| format!("Falha ao consultar reputação da temporada: {e}"))?;

    let mut updates: Vec<(String, i32, i32)> = Vec::new();
    for row in rows {
        updates.push(row.map_err(|e| format!("Falha ao mapear reputação: {e}"))?);
    }
    drop(stmt);

    for (team_id, position, grid_size) in updates {
        let mut team = match team_queries::get_team_by_id(conn, &team_id) {
            Ok(Some(team)) => team,
            Ok(None) => continue,
            Err(e) => return Err(format!("Falha ao carregar equipe {team_id}: {e}")),
        };
        team.reputacao = advance_team_reputation(team.reputacao, position, grid_size);
        team_queries::update_team(conn, &team)
            .map_err(|e| format!("Falha ao atualizar reputação da equipe {team_id}: {e}"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_GRID: i32 = 6;

    #[test]
    fn champion_target_is_high_and_last_target_is_low() {
        let champ = season_reputation_target(1, FULL_GRID);
        let last = season_reputation_target(FULL_GRID, FULL_GRID);
        assert!(champ > 85.0, "campeão deve mirar alto, foi {champ}");
        assert!(
            last <= TARGET_FLOOR + 0.01,
            "lanterna deve mirar o piso do alvo, foi {last}"
        );
        assert!(champ > last);
    }

    #[test]
    fn mid_grid_target_sits_near_base() {
        // Grid de 5: posição 3 é exatamente o meio → alvo == BASE.
        let mid = season_reputation_target(3, 5);
        assert!(
            (mid - BASE).abs() < 0.01,
            "meio de tabela deve mirar a base, foi {mid}"
        );
    }

    #[test]
    fn winning_lifts_a_mid_team_but_gradually() {
        let after = advance_team_reputation(BASE, 1, FULL_GRID);
        // Sobe, mas longe do alvo (~93): inércia média não pula.
        assert!(
            after > BASE + 6.0,
            "deveria subir visivelmente, foi {after}"
        );
        assert!(
            after < 65.0,
            "não deveria pular pro alvo em uma temporada, foi {after}"
        );
    }

    #[test]
    fn losing_pulls_a_strong_team_down_toward_the_mean() {
        // Potência (90) que termina em último decai — mas não despenca.
        let after = advance_team_reputation(90.0, FULL_GRID, FULL_GRID);
        assert!(after < 90.0, "deveria cair, foi {after}");
        assert!(
            after > 70.0,
            "queda deveria ser gradual (inércia), foi {after}"
        );
    }

    #[test]
    fn mean_reversion_lifts_an_underrated_team_finishing_mid() {
        // Time com reputação baixa (25) que termina no meio é puxado PRA CIMA.
        let after = advance_team_reputation(25.0, 3, 5);
        assert!(
            after > 25.0,
            "meio de tabela deveria recuperar reputação, foi {after}"
        );
    }

    #[test]
    fn perpetual_champion_converges_high_without_pegging_immediately() {
        let mut rep = BASE;
        for _ in 0..12 {
            rep = advance_team_reputation(rep, 1, FULL_GRID);
        }
        assert!(
            rep > 90.0,
            "campeão perpétuo deveria virar potência, ficou {rep}"
        );
        assert!(rep <= CEIL);
    }

    #[test]
    fn perpetual_backmarker_settles_at_the_protected_floor() {
        let mut rep = BASE;
        for _ in 0..15 {
            rep = advance_team_reputation(rep, FULL_GRID, FULL_GRID);
        }
        assert!(
            (rep - FLOOR).abs() < 1.0,
            "lanterna crônica deveria assentar no piso, ficou {rep}"
        );
    }

    #[test]
    fn result_is_always_clamped_to_band() {
        assert!(advance_team_reputation(120.0, 1, FULL_GRID) <= CEIL);
        assert!(advance_team_reputation(-50.0, FULL_GRID, FULL_GRID) >= FLOOR);
    }

    #[test]
    fn degenerate_grid_only_mean_reverts() {
        // Grid de 1: sem ordenação; alvo == BASE, então uma reputação alta cai um
        // pouco em direção à base (só reversão, mais o kicker de "campeão").
        let target = season_reputation_target(1, 1);
        assert!((target - BASE).abs() < 0.01);
    }

    #[test]
    fn is_deterministic() {
        let a = advance_team_reputation(60.0, 2, FULL_GRID);
        let b = advance_team_reputation(60.0, 2, FULL_GRID);
        assert_eq!(a, b);
    }

    mod orchestrator {
        use super::super::*;
        use crate::db::migrations;
        use crate::db::queries::teams as team_queries;
        use crate::models::season::Season;
        use crate::models::team::placeholder_team_from_db;
        use rusqlite::{params, Connection};

        fn seed_team(conn: &Connection, id: &str, category: &str, reputacao: f64) {
            let mut team = placeholder_team_from_db(
                id.to_string(),
                format!("Equipe {id}"),
                category.to_string(),
                "2026-01-01".to_string(),
            );
            team.reputacao = reputacao;
            team_queries::insert_team(conn, &team).expect("insert team");
        }

        fn archive_row(conn: &Connection, team_id: &str, category: &str, position: i32) {
            conn.execute(
                "INSERT INTO team_season_archive (
                    team_id, season_number, ano, categoria, classe, posicao_campeonato,
                    pontos, vitorias, podios, poles, corridas, titulos_construtores,
                    piloto_1_id, piloto_2_id, snapshot_json
                 ) VALUES (?1, 1, 2026, ?2, NULL, ?3, 0, 0, 0, 0, 12, 0, NULL, NULL, '')",
                params![team_id, category, position],
            )
            .expect("insert archive row");
        }

        #[test]
        fn season_update_lifts_champion_and_lowers_backmarker() {
            let conn = Connection::open_in_memory().expect("db");
            migrations::run_all(&conn).expect("schema");

            // Grid de 4 em gt3: campeão (rep 48) e lanterna (rep 48) partem do meio.
            seed_team(&conn, "T1", "gt3", 48.0);
            seed_team(&conn, "T2", "gt3", 48.0);
            seed_team(&conn, "T3", "gt3", 48.0);
            seed_team(&conn, "T4", "gt3", 48.0);
            archive_row(&conn, "T1", "gt3", 1);
            archive_row(&conn, "T2", "gt3", 2);
            archive_row(&conn, "T3", "gt3", 3);
            archive_row(&conn, "T4", "gt3", 4);

            let season = Season::new("S1".to_string(), 1, 2026);
            update_team_reputations_from_season(&conn, &season).expect("update");

            let champ = team_queries::get_team_by_id(&conn, "T1").unwrap().unwrap();
            let last = team_queries::get_team_by_id(&conn, "T4").unwrap().unwrap();
            assert!(champ.reputacao > 48.0, "campeão subiu: {}", champ.reputacao);
            assert!(last.reputacao < 48.0, "lanterna caiu: {}", last.reputacao);
            assert!(champ.reputacao <= CEIL && last.reputacao >= FLOOR);
        }
    }
}
