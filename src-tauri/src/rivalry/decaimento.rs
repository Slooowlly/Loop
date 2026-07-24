//! Decaimento anual das rivalidades (extraído de `rivalry/mod.rs`).

use rusqlite::Connection;

use crate::common::time::current_timestamp;
use crate::db::connection::DbError;
use crate::db::queries::rivalries::{delete_rivalry, get_all_rivalries, update_rivalry_axes};
use crate::models::rivalry::{rivalry_lifecycle, RivalryLifecycle};

// ── Passo 14: Decaimento de fim de temporada ──────────────────────────────────

/// Aplica decaimento anual a todas as rivalidades.
///
/// Regras:
/// - Rivalidade ativa na temporada atual (temporada_update == temporada_atual):
///   recent_activity *= 0.5  |  historical_intensity inalterado
/// - Rivalidade inativa (temporada_update < temporada_atual):
///   recent_activity *= 0.2  |  historical_intensity *= 0.85
/// - Se o ciclo de vida resultante for Extinta → remove do banco.
///
/// Deve ser chamada uma vez no pipeline de fim de temporada.
pub fn apply_season_end_rivalry_decay(
    conn: &Connection,
    temporada_atual: i32,
) -> Result<(), DbError> {
    let all = get_all_rivalries(conn)?;
    let now = current_timestamp();

    for r in all {
        let (new_historical, new_recent) = if r.temporada_update == temporada_atual {
            // Ativa nesta temporada: esfria o calor recente pela metade
            (r.historical_intensity, r.recent_activity * 0.5)
        } else {
            // Inativa: recente esfria bastante, histórico decai levemente
            (r.historical_intensity * 0.85, r.recent_activity * 0.2)
        };

        if matches!(
            rivalry_lifecycle(new_historical, new_recent),
            RivalryLifecycle::Extinta
        ) {
            delete_rivalry(conn, &r.id)?;
        } else {
            update_rivalry_axes(
                conn,
                &r.id,
                new_historical,
                new_recent,
                &now,
                r.temporada_update, // temporada_update não muda no decaimento
            )?;
        }
    }

    Ok(())
}
