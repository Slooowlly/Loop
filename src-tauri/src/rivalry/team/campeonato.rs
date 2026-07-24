//! Fonte 1: Briga de construtores (Campeonato).
//!
//! Fim de temporada, lendo `team_season_archive` (mesma fonte que reputação/moral). Dentro
//! de cada categoria/classe pega os top-4; um par vira rivalidade se os dois estão no top-3
//! OU o gap de pontos é apertado (≤ 15% dos pontos do líder). É a espinha dorsal: reforça a
//! cada temporada que a briga se repete, fazendo um clássico crescer no eixo histórico.

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::models::team_rivalry::TeamRivalryType;

use super::motor::{apply_team_rivalry_event, TeamRivalryEvent};
use super::noticias::emit_team_rivalry_news;

/// Fração dos pontos do líder dentro da qual o gap de pontos conta como "briga apertada".
const CONSTRUCTOR_CLOSE_FRAC: f64 = 0.15;

/// Reforça rivalidades entre construtores que brigaram na temporada que fecha. Roda no
/// pipeline de fim de temporada, depois do arquivamento.
pub fn process_constructor_battle_rivalry(
    conn: &Connection,
    temporada: i32,
) -> Result<(), DbError> {
    let mut stmt = conn.prepare(
        "SELECT team_id, categoria, COALESCE(classe, ''), posicao_campeonato, pontos
         FROM team_season_archive
         WHERE season_number = ?1 AND posicao_campeonato IS NOT NULL
         ORDER BY categoria, COALESCE(classe, ''), posicao_campeonato",
    )?;
    let rows = stmt.query_map([temporada], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i32>(3)?,
            row.get::<_, f64>(4)?,
        ))
    })?;

    // Agrupa por (categoria, classe): (team_id, categoria, posição, pontos).
    let mut groups: std::collections::BTreeMap<(String, String), Vec<(String, String, i32, f64)>> =
        std::collections::BTreeMap::new();
    for row in rows {
        let (team_id, categoria, classe, pos, pontos) = row?;
        groups
            .entry((categoria.clone(), classe))
            .or_default()
            .push((team_id, categoria, pos, pontos));
    }
    drop(stmt);

    for (_key, mut teams) in groups {
        teams.sort_by_key(|t| t.2); // por posição ascendente
        teams.truncate(4); // só os top-4 brigam por "clássico"
        if teams.len() < 2 {
            continue;
        }
        let leader_points = teams.iter().map(|t| t.3).fold(f64::MIN, f64::max).max(1.0);
        for i in 0..teams.len() {
            for j in (i + 1)..teams.len() {
                let (a_id, categoria, a_pos, a_pts) = &teams[i];
                let (b_id, _, b_pos, b_pts) = &teams[j];
                let both_top3 = *a_pos <= 3 && *b_pos <= 3;
                let gap = (a_pts - b_pts).abs();
                let close = gap <= CONSTRUCTOR_CLOSE_FRAC * leader_points;
                if !(both_top3 || close) {
                    continue;
                }
                // +50% se o par decidiu o título (1º vs 2º).
                let title_decider =
                    (*a_pos == 1 && *b_pos == 2) || (*a_pos == 2 && *b_pos == 1);
                let (h, r) = if title_decider { (6.0, 15.0) } else { (4.0, 10.0) };
                let applied = apply_team_rivalry_event(
                    conn,
                    &TeamRivalryEvent {
                        team_a: a_id.clone(),
                        team_b: b_id.clone(),
                        tipo: TeamRivalryType::Campeonato,
                        historical_delta: h,
                        recent_delta: r,
                        temporada,
                    },
                )?;
                emit_team_rivalry_news(
                    conn,
                    &applied,
                    TeamRivalryType::Campeonato,
                    a_id,
                    b_id,
                    Some(categoria),
                    None,
                    temporada,
                )?;
            }
        }
    }
    Ok(())
}
