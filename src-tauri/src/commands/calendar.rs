use tauri::{AppHandle, Manager};

use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::seasons as season_queries;
use crate::models::temporal::SeasonTemporalSummary;

#[tauri::command]
pub fn get_temporal_summary(
    career_id: String,
    season_id: String,
    player_category: String,
    app: AppHandle,
) -> Result<SeasonTemporalSummary, String> {
    let base_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| e.to_string())?;

    let season = season_queries::get_season_by_id(&db.conn, &season_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Season not found: {season_id}"))?;
    calendar_queries::normalize_calendar_display_dates_for_weekday_policy(
        &db.conn, &season.id, season.ano,
    )
    .map_err(|e| e.to_string())?;

    calendar_queries::get_season_temporal_summary(
        &db.conn,
        &season_id,
        &player_category,
        &season.fase,
    )
    .map_err(|e| e.to_string())
}
