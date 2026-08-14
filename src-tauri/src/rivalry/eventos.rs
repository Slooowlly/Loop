//! Upsert de rivalidade com dois eixos e leitura por piloto
//! (extraído de `rivalry/mod.rs`).

use rusqlite::Connection;

use crate::common::time::current_timestamp;

use crate::db::connection::DbError;
use crate::db::queries::rivalries::{
    get_rivalries_for_pilot, get_rivalry_by_pair, insert_rivalry, update_rivalry_axes,
};
use crate::generators::ids::{next_id, IdType};
use crate::models::rivalry::{normalize_pair, perceived_intensity, Rivalry, RivalryType};

use super::intensidade::clamp;

// ── Passo 13: Evento de rivalidade com dois eixos ─────────────────────────────

pub struct RivalryEvent {
    pub piloto_a: String,
    pub piloto_b: String,
    /// Origem do evento — define o tipo da rivalidade se ela for nova.
    pub tipo: RivalryType,
    /// Quanto adicionar ao eixo histórico (memória duradoura).
    pub historical_delta: f64,
    /// Quanto adicionar ao eixo recente (calor atual).
    pub recent_delta: f64,
    /// Temporada corrente — atualiza `temporada_update`.
    pub temporada: i32,
}

// ── Resultado de apply_rivalry_event ─────────────────────────────────────────

pub struct RivalryApplied {
    pub rivalry_id: String,
    /// Intensidade percebida antes do evento.
    pub old_perceived: f64,
    /// Intensidade percebida depois do evento.
    pub new_perceived: f64,
}

// ── Passo 13: Upsert com dois eixos ──────────────────────────────────────────

pub fn apply_rivalry_event(
    conn: &Connection,
    event: &RivalryEvent,
) -> Result<RivalryApplied, DbError> {
    let pair = match normalize_pair(&event.piloto_a, &event.piloto_b) {
        Some(p) => p,
        None => {
            return Ok(RivalryApplied {
                rivalry_id: String::new(),
                old_perceived: 0.0,
                new_perceived: 0.0,
            });
        }
    };

    let now = current_timestamp();

    match get_rivalry_by_pair(conn, &pair.piloto1_id, &pair.piloto2_id)? {
        Some(existing) => {
            let old_perceived = existing.perceived_intensity();
            let new_historical = clamp(existing.historical_intensity + event.historical_delta);
            let new_recent = clamp(existing.recent_activity + event.recent_delta);
            let new_perceived = perceived_intensity(new_historical, new_recent);
            update_rivalry_axes(
                conn,
                &existing.id,
                new_historical,
                new_recent,
                &now,
                event.temporada,
            )?;
            Ok(RivalryApplied {
                rivalry_id: existing.id,
                old_perceived,
                new_perceived,
            })
        }
        None => {
            let id = next_id(conn, IdType::Rivalry)?;
            let new_historical = clamp(event.historical_delta);
            let new_recent = clamp(event.recent_delta);
            let new_perceived = perceived_intensity(new_historical, new_recent);
            let piloto1_id = pair.piloto1_id.clone();
            let piloto2_id = pair.piloto2_id.clone();
            let rivalry = Rivalry {
                id: id.clone(),
                piloto1_id: piloto1_id.clone(),
                piloto2_id: piloto2_id.clone(),
                historical_intensity: new_historical,
                recent_activity: new_recent,
                tipo: event.tipo.clone(),
                criado_em: now.clone(),
                ultima_atualizacao: now,
                temporada_update: event.temporada,
            };
            match insert_rivalry(conn, &rivalry) {
                Ok(()) => Ok(RivalryApplied {
                    rivalry_id: id,
                    old_perceived: 0.0,
                    new_perceived,
                }),
                Err(err) if is_rivalry_pair_constraint(&err) => {
                    let existing = get_rivalry_by_pair(conn, &piloto1_id, &piloto2_id)?
                        .ok_or_else(|| {
                            DbError::InvalidData(format!(
                                "Par de rivalidade '{}' x '{}' conflitou no insert, mas nao foi encontrado no reload",
                                piloto1_id, piloto2_id
                            ))
                        })?;
                    let old_perceived = existing.perceived_intensity();
                    let new_historical =
                        clamp(existing.historical_intensity + event.historical_delta);
                    let new_recent = clamp(existing.recent_activity + event.recent_delta);
                    let new_perceived = perceived_intensity(new_historical, new_recent);
                    update_rivalry_axes(
                        conn,
                        &existing.id,
                        new_historical,
                        new_recent,
                        &current_timestamp(),
                        event.temporada,
                    )?;
                    Ok(RivalryApplied {
                        rivalry_id: existing.id,
                        old_perceived,
                        new_perceived,
                    })
                }
                Err(err) => Err(err),
            }
        }
    }
}

// ── Passo 12: Leitura por piloto ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PilotRivalrySummary {
    pub rivalry_id: String,
    pub rival_id: String,
    pub historical_intensity: f64,
    pub recent_activity: f64,
    /// Calculado em tempo de leitura (0.4*hist + 0.6*rec).
    pub perceived_intensity: f64,
    pub tipo: RivalryType,
    pub ultima_atualizacao: String,
}

pub fn get_pilot_rivalries(
    conn: &Connection,
    pilot_id: &str,
) -> Result<Vec<PilotRivalrySummary>, DbError> {
    let rivalries = get_rivalries_for_pilot(conn, pilot_id)?;
    let summaries = rivalries
        .into_iter()
        .map(|r| {
            let rival_id = if r.piloto1_id == pilot_id {
                r.piloto2_id.clone()
            } else {
                r.piloto1_id.clone()
            };
            let perceived = r.perceived_intensity();
            PilotRivalrySummary {
                rivalry_id: r.id,
                rival_id,
                historical_intensity: r.historical_intensity,
                recent_activity: r.recent_activity,
                perceived_intensity: perceived,
                tipo: r.tipo,
                ultima_atualizacao: r.ultima_atualizacao,
            }
        })
        .collect();
    Ok(summaries)
}

pub(crate) fn is_rivalry_pair_constraint(err: &DbError) -> bool {
    matches!(
        err,
        DbError::Sqlite(rusqlite::Error::SqliteFailure(inner, _))
            if inner.code == rusqlite::ErrorCode::ConstraintViolation
    )
}
