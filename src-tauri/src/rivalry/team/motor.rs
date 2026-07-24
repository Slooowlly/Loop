//! Núcleo do motor: evento, upsert nos dois eixos e o clamp da escala.

use rusqlite::Connection;

use crate::common::time::current_timestamp;
use crate::db::connection::DbError;
use crate::db::queries::team_rivalries::{
    get_team_rivalry_by_pair, insert_team_rivalry, update_team_rivalry_axes,
};
use crate::generators::ids::{next_id, IdType};
use crate::models::rivalry::{normalize_pair, perceived_intensity};
use crate::models::team_rivalry::{TeamRivalry, TeamRivalryType};

// ── Constantes de domínio ─────────────────────────────────────────────────────

const AXIS_MAX: f64 = 100.0;
const AXIS_MIN: f64 = 0.0;

fn clamp(v: f64) -> f64 {
    v.clamp(AXIS_MIN, AXIS_MAX)
}

// ── Evento ────────────────────────────────────────────────────────────────────

/// Um reforço de rivalidade entre dois times. Os deltas seguem a mesma escala do sistema
/// de piloto (recente aquece rápido, histórico é memória).
pub struct TeamRivalryEvent {
    pub team_a: String,
    pub team_b: String,
    /// Origem — define o tipo se a rivalidade for nova (preservado nos reforços).
    pub tipo: TeamRivalryType,
    pub historical_delta: f64,
    pub recent_delta: f64,
    pub temporada: i32,
}

/// Resultado de [`apply_team_rivalry_event`] — a percebida antes/depois, para as fases
/// seguintes decidirem manchete por cruzamento de threshold.
pub struct TeamRivalryApplied {
    pub rivalry_id: String,
    pub old_perceived: f64,
    pub new_perceived: f64,
}

// ── Upsert com dois eixos ─────────────────────────────────────────────────────

/// Aplica um evento de rivalidade entre times: cria a rivalidade ou reforça a existente
/// (par normalizado). Idêntico em espírito ao `apply_rivalry_event` de piloto, incluindo
/// o tratamento da corrida de constraint no par único.
pub fn apply_team_rivalry_event(
    conn: &Connection,
    event: &TeamRivalryEvent,
) -> Result<TeamRivalryApplied, DbError> {
    let pair = match normalize_pair(&event.team_a, &event.team_b) {
        Some(p) => p,
        None => {
            return Ok(TeamRivalryApplied {
                rivalry_id: String::new(),
                old_perceived: 0.0,
                new_perceived: 0.0,
            });
        }
    };
    // `normalize_pair` devolve o par ordenado nos campos `piloto1_id/piloto2_id` — aqui
    // eles carregam os ids de TIME (a função é puramente ordenação de strings).
    let team1_id = pair.piloto1_id;
    let team2_id = pair.piloto2_id;
    let now = current_timestamp();

    match get_team_rivalry_by_pair(conn, &team1_id, &team2_id)? {
        Some(existing) => {
            let old_perceived = existing.perceived_intensity();
            let new_historical = clamp(existing.historical_intensity + event.historical_delta);
            let new_recent = clamp(existing.recent_activity + event.recent_delta);
            let new_perceived = perceived_intensity(new_historical, new_recent);
            update_team_rivalry_axes(
                conn,
                &existing.id,
                new_historical,
                new_recent,
                &now,
                event.temporada,
            )?;
            Ok(TeamRivalryApplied {
                rivalry_id: existing.id,
                old_perceived,
                new_perceived,
            })
        }
        None => {
            let id = next_id(conn, IdType::TeamRivalry)?;
            let new_historical = clamp(event.historical_delta);
            let new_recent = clamp(event.recent_delta);
            let new_perceived = perceived_intensity(new_historical, new_recent);
            let rivalry = TeamRivalry {
                id: id.clone(),
                team1_id: team1_id.clone(),
                team2_id: team2_id.clone(),
                historical_intensity: new_historical,
                recent_activity: new_recent,
                tipo: event.tipo.clone(),
                criado_em: now.clone(),
                ultima_atualizacao: now,
                temporada_update: event.temporada,
            };
            match insert_team_rivalry(conn, &rivalry) {
                Ok(()) => Ok(TeamRivalryApplied {
                    rivalry_id: id,
                    old_perceived: 0.0,
                    new_perceived,
                }),
                // Corrida: outro caminho criou o par entre o get e o insert → recarrega e reforça.
                Err(err) if is_pair_constraint(&err) => {
                    let existing = get_team_rivalry_by_pair(conn, &team1_id, &team2_id)?
                        .ok_or_else(|| {
                            DbError::InvalidData(format!(
                                "Par de rivalidade de equipe '{team1_id}' x '{team2_id}' conflitou no insert, mas nao foi encontrado no reload"
                            ))
                        })?;
                    let old_perceived = existing.perceived_intensity();
                    let new_historical =
                        clamp(existing.historical_intensity + event.historical_delta);
                    let new_recent = clamp(existing.recent_activity + event.recent_delta);
                    let new_perceived = perceived_intensity(new_historical, new_recent);
                    update_team_rivalry_axes(
                        conn,
                        &existing.id,
                        new_historical,
                        new_recent,
                        &current_timestamp(),
                        event.temporada,
                    )?;
                    Ok(TeamRivalryApplied {
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

// ── Helpers ───────────────────────────────────────────────────────────────────

fn is_pair_constraint(err: &DbError) -> bool {
    matches!(
        err,
        DbError::Sqlite(rusqlite::Error::SqliteFailure(inner, _))
            if inner.code == rusqlite::ErrorCode::ConstraintViolation
    )
}
