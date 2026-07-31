use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension};

use crate::commands::career_types::{
    GlobalTeamHistoryBand, GlobalTeamHistoryFamily, GlobalTeamHistoryFamilyBand,
    GlobalTeamHistoryPayload, GlobalTeamHistoryPoint, GlobalTeamHistoryTeamRow, TeamTitleCount,
};
use crate::config::app_config::AppConfig;
use crate::constants::historical_timeline::{
    category_start_year, class_start_year as timeline_class_start_year,
};
use crate::db::connection::Database;

#[path = "global_team_history/bandas.rs"]
mod bandas;
#[path = "global_team_history/campeoes.rs"]
mod campeoes;
#[path = "global_team_history/dados.rs"]
mod dados;
#[path = "global_team_history/familias.rs"]
mod familias;

// Consumidos pela propria fachada e pelos irmaos, via `use super::*`.
use bandas::*;
pub(crate) use campeoes::get_band_champions_in_base_dir;
use dados::*;
use familias::*;

const DEFAULT_FAMILY: &str = "mazda";
const DEFAULT_START_YEAR: i32 = 2000;
const DEFAULT_MAX_YEAR: i32 = 2025;
const DEFAULT_WINDOW_SIZE: i32 = 8;
const MIN_WINDOW_SIZE: i32 = 4;
const MAX_WINDOW_SIZE: i32 = 32;

pub(crate) fn get_global_team_history_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    family: Option<&str>,
    start_year: Option<i32>,
    window_size: Option<i32>,
) -> Result<GlobalTeamHistoryPayload, String> {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    if !db_path.exists() {
        return Err("Banco da carreira nao encontrado.".to_string());
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    build_global_team_history(
        &db.conn,
        family.unwrap_or(DEFAULT_FAMILY),
        start_year.unwrap_or(DEFAULT_START_YEAR),
        window_size.unwrap_or(DEFAULT_WINDOW_SIZE),
    )
}

pub(crate) fn build_global_team_history(
    conn: &Connection,
    family: &str,
    start_year: i32,
    window_size: i32,
) -> Result<GlobalTeamHistoryPayload, String> {
    let family_def = resolve_family(family);
    let window_size = window_size.clamp(MIN_WINDOW_SIZE, MAX_WINDOW_SIZE);
    let bounds = history_year_bounds(conn)?;
    let min_year = bounds.min_year;
    let max_year = bounds.max_year;
    let current_year = bounds.current_year;
    let latest_start = (max_year - window_size + 1).max(min_year);
    let window_start = start_year.clamp(min_year, latest_start);
    let window_end = (window_start + window_size - 1).min(max_year);
    let mut archive_rows = load_archive_rows(conn, window_start, window_end)?;
    // Inject the in-progress season so the Atlas reflects each team's CURRENT
    // division — not just the last COMPLETED season. Without this, a team relegated
    // for the ongoing season still shows in its old division (the archive only holds
    // finished seasons). Only when the active season sits inside the visible window.
    if bounds.in_progress && current_year >= window_start && current_year <= window_end {
        archive_rows.extend(load_current_season_rows(
            conn,
            current_year,
            bounds.last_completed_year,
        )?);
    }
    let archive_rows = dedupe_archive_rows_for_family(family_def, archive_rows);
    // Titles are loaded once across ALL years (no window filter) so the count is
    // stable and does not shift when the user scrolls the timeline.
    let all_time_titles = load_all_time_titles(conn, family_def)?;
    let bands = family_def
        .bands
        .iter()
        .map(|band| {
            build_band_payload(
                band,
                &archive_rows,
                &all_time_titles,
                window_start,
                window_end,
                bounds.last_completed_year,
            )
        })
        .collect::<Vec<_>>();

    Ok(GlobalTeamHistoryPayload {
        selected_family: family_def.id.to_string(),
        min_year,
        max_year,
        window_start,
        window_end,
        window_size,
        current_year,
        in_progress: bounds.in_progress,
        last_completed_year: bounds.last_completed_year,
        families: FAMILY_DEFS.iter().map(family_payload).collect(),
        bands,
    })
}

#[cfg(test)]
mod tests;
