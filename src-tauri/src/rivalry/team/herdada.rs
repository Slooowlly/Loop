//! Fonte 4: Transbordamento de piloto (Herdada).
//!
//! O "Verstappen×Hamilton → RBR×Merc": rivalidades de PILOTO vivas e intensas (percebida ≥
//! 60) cujos dois pilotos estão em times diferentes pingam um trickle na rivalidade dos
//! times. Trickle deliberadamente pequeno — é eco, não origem.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::models::rivalry::normalize_team_pair;
use crate::models::team_rivalry::TeamRivalryType;

use super::motor::{apply_team_rivalry_event, TeamRivalryEvent};
use super::noticias::emit_team_rivalry_news;

/// Percebida mínima da rivalidade de PILOTO para transbordar aos times (faixa Forte).
const BLEED_MIN_PERCEIVED: f64 = 60.0;

/// Varre as rivalidades de piloto e transborda as intensas cross-time para os times.
/// `team_by_driver` mapeia driver_id → team_id (dos participantes da corrida).
pub fn process_driver_rivalry_bleed(
    conn: &Connection,
    team_by_driver: &HashMap<String, String>,
    categoria_id: &str,
    rodada: i32,
    temporada: i32,
) -> Result<(), DbError> {
    use crate::db::queries::rivalries::get_all_rivalries;

    let rivalries = get_all_rivalries(conn)?;
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for rv in rivalries {
        if rv.perceived_intensity() < BLEED_MIN_PERCEIVED {
            continue;
        }
        let (Some(ta), Some(tb)) = (
            team_by_driver.get(&rv.piloto1_id),
            team_by_driver.get(&rv.piloto2_id),
        ) else {
            continue;
        };
        if ta == tb {
            continue;
        }
        let Some(pair) = normalize_team_pair(ta, tb) else {
            continue;
        };
        // Dedupe: dois pares de pilotos podem mapear pro mesmo par de times.
        if !seen.insert((pair.team1_id.clone(), pair.team2_id.clone())) {
            continue;
        }
        let applied = apply_team_rivalry_event(
            conn,
            &TeamRivalryEvent {
                team_a: pair.team1_id.clone(),
                team_b: pair.team2_id.clone(),
                tipo: TeamRivalryType::Herdada,
                historical_delta: 1.0,
                recent_delta: 3.0,
                temporada,
            },
        )?;
        emit_team_rivalry_news(
            conn,
            &applied,
            TeamRivalryType::Herdada,
            &pair.team1_id,
            &pair.team2_id,
            Some(categoria_id),
            Some(rodada),
            temporada,
        )?;
    }
    Ok(())
}
