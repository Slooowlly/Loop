use tauri::{AppHandle, Manager};

use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::seasons as season_queries;
use crate::market::preseason::{load_preseason_plan, refresh_preseason_state_display_date};
use crate::models::enums::SeasonPhase;
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

    let temporal_category = if season.fase == SeasonPhase::Temporada {
        resolve_regular_contract_category(&db.conn)?.unwrap_or(player_category)
    } else {
        player_category
    };

    let mut summary = calendar_queries::get_season_temporal_summary(
        &db.conn,
        &season_id,
        &temporal_category,
        &season.fase,
    )
    .map_err(|e| e.to_string())?;

    if season.fase == SeasonPhase::PreTemporada {
        let career_dir = config.saves_dir().join(&career_id);
        if let Some(mut plan) = load_preseason_plan(&career_dir)? {
            refresh_preseason_state_display_date(&db.conn, &season.id, &mut plan.state)?;
            if let Some(display_date) = plan.state.current_display_date {
                summary.current_display_date = display_date;
                summary.days_until_next_event = summary
                    .next_event_display_date
                    .as_deref()
                    .and_then(|next_date| {
                        days_between_display_dates(&summary.current_display_date, next_date)
                    });
            }
        }
    }

    Ok(summary)
}

fn resolve_regular_contract_category(
    conn: &rusqlite::Connection,
) -> Result<Option<String>, String> {
    let player = driver_queries::get_player_driver(conn)
        .map_err(|e| format!("Falha ao carregar jogador para resumo temporal: {e}"))?;
    let contract = contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao carregar contrato regular do jogador: {e}"))?;
    Ok(contract.map(|contract| contract.categoria))
}

fn days_between_display_dates(from: &str, to: &str) -> Option<i32> {
    let from_date = chrono::NaiveDate::parse_from_str(from, "%Y-%m-%d").ok()?;
    let to_date = chrono::NaiveDate::parse_from_str(to, "%Y-%m-%d").ok()?;
    i32::try_from((to_date - from_date).num_days()).ok()
}
