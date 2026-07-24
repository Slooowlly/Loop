//! Fonte 3: Guerra na pista (Pista).
//!
//! Piggyback no mesmo `flat_incidents` da rivalidade de piloto: resolve o time de cada
//! piloto em colisão e agrega POR PAR DE TIMES (só times diferentes — bater no companheiro
//! não é rivalidade de time), pegando a severidade máxima do par no evento.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::models::rivalry::normalize_pair;
use crate::models::team_rivalry::TeamRivalryType;

use super::motor::{apply_team_rivalry_event, TeamRivalryEvent};
use super::noticias::emit_team_rivalry_news;

/// Reforça rivalidades de PISTA a partir das colisões de uma corrida. `team_by_driver`
/// mapeia driver_id → team_id dos participantes.
pub fn process_team_collisions_rivalry(
    conn: &Connection,
    incidents: &[crate::simulation::incidents::IncidentResult],
    team_by_driver: &HashMap<String, String>,
    categoria_id: &str,
    rodada: i32,
    temporada: i32,
) -> Result<(), DbError> {
    use crate::simulation::incidents::{IncidentSeverity, IncidentType};

    let mut pairs: HashMap<(String, String), (f64, f64)> = HashMap::new();
    for inc in incidents {
        if inc.incident_type != IncidentType::Collision {
            continue;
        }
        let Some(linked) = &inc.linked_pilot_id else {
            continue;
        };
        let (Some(ta), Some(tb)) =
            (team_by_driver.get(&inc.pilot_id), team_by_driver.get(linked))
        else {
            continue;
        };
        if ta == tb {
            continue; // bater no próprio companheiro não é rivalidade de time
        }
        let Some(pair) = normalize_pair(ta, tb) else {
            continue;
        };
        // Severidade máxima do par → delta (base capado por corrida).
        let (h, r) = if inc.severity == IncidentSeverity::Critical || inc.is_dnf {
            (3.0, 8.0)
        } else {
            (2.0, 6.0)
        };
        let e = pairs
            .entry((pair.piloto1_id, pair.piloto2_id))
            .or_insert((0.0, 0.0));
        if h > e.0 {
            *e = (h, r);
        }
    }

    for ((t1, t2), (h, r)) in pairs {
        let applied = apply_team_rivalry_event(
            conn,
            &TeamRivalryEvent {
                team_a: t1.clone(),
                team_b: t2.clone(),
                tipo: TeamRivalryType::Pista,
                historical_delta: h,
                recent_delta: r,
                temporada,
            },
        )?;
        emit_team_rivalry_news(
            conn,
            &applied,
            TeamRivalryType::Pista,
            &t1,
            &t2,
            Some(categoria_id),
            Some(rodada),
            temporada,
        )?;
    }
    Ok(())
}
