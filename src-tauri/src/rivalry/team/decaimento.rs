//! Decaimento anual das rivalidades de equipe (fim de temporada).

use rusqlite::Connection;

use crate::common::time::current_timestamp;
use crate::db::connection::DbError;
use crate::db::queries::team_rivalries::{
    delete_team_rivalry, get_all_team_rivalries, update_team_rivalry_axes,
};
use crate::models::rivalry::{rivalry_lifecycle, RivalryLifecycle};

/// Aplica o decaimento anual a todas as rivalidades de equipe (mesma regra do piloto):
/// - Ativa nesta temporada (`temporada_update == atual`): `recent *= 0.5`, histórico intacto.
/// - Inativa: `recent *= 0.2`, `historical *= 0.85`.
/// - Ciclo de vida `Extinta` → removida do banco.
///
/// Deve ser chamada uma vez no pipeline de fim de temporada.
pub fn apply_season_end_team_rivalry_decay(
    conn: &Connection,
    temporada_atual: i32,
) -> Result<(), DbError> {
    let all = get_all_team_rivalries(conn)?;
    let now = current_timestamp();

    for r in all {
        let (new_historical, new_recent) = if r.temporada_update == temporada_atual {
            (r.historical_intensity, r.recent_activity * 0.5)
        } else {
            (r.historical_intensity * 0.85, r.recent_activity * 0.2)
        };

        if matches!(
            rivalry_lifecycle(new_historical, new_recent),
            RivalryLifecycle::Extinta
        ) {
            delete_team_rivalry(conn, &r.id)?;
        } else {
            update_team_rivalry_axes(
                conn,
                &r.id,
                new_historical,
                new_recent,
                &now,
                r.temporada_update,
            )?;
        }
    }

    Ok(())
}
