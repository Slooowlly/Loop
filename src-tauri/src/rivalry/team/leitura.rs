//! Leitura das rivalidades de um time (o "outro lado" do par já resolvido).

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::db::queries::team_rivalries::get_team_rivalries_for_team;
use crate::models::team_rivalry::TeamRivalryType;

#[derive(Debug, Clone)]
pub struct TeamRivalrySummary {
    pub rivalry_id: String,
    /// O "outro lado" do par, do ponto de vista do time consultado.
    pub rival_id: String,
    pub historical_intensity: f64,
    pub recent_activity: f64,
    pub perceived_intensity: f64,
    pub tipo: TeamRivalryType,
    pub ultima_atualizacao: String,
}

pub fn get_team_rivalries(
    conn: &Connection,
    team_id: &str,
) -> Result<Vec<TeamRivalrySummary>, DbError> {
    let rivalries = get_team_rivalries_for_team(conn, team_id)?;
    let summaries = rivalries
        .into_iter()
        .map(|r| {
            let rival_id = if r.team1_id == team_id {
                r.team2_id.clone()
            } else {
                r.team1_id.clone()
            };
            let perceived = r.perceived_intensity();
            TeamRivalrySummary {
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
