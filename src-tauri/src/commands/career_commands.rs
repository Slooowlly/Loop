use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::commands::career::{
    advance_market_week_in_base_dir, advance_season_in_base_dir, create_career_in_base_dir,
    delete_career_in_base_dir, finalize_preseason_in_base_dir,
    get_briefing_phrase_history_in_base_dir, get_calendar_for_category_in_base_dir,
    get_driver_detail_in_base_dir, get_driver_in_base_dir, get_drivers_by_category_in_base_dir,
    get_news_in_base_dir, get_player_proposals_in_base_dir, get_preseason_free_agents_in_base_dir,
    get_preseason_state_in_base_dir, get_previous_champions_in_base_dir,
    get_race_results_by_category_in_base_dir, get_team_history_dossier_in_base_dir,
    get_teams_standings_in_base_dir, list_saves_in_base_dir, load_career_in_base_dir,
    persist_resume_context_in_base_dir, respond_to_proposal_in_base_dir,
    save_briefing_phrase_history_in_base_dir, skip_all_pending_races_in_base_dir,
    PlayerProposalView, ProposalResponse,
};
use crate::commands::career_types::{
    BriefingPhraseEntryInput, BriefingPhraseHistory, CareerData, CareerDraftState,
    CareerResumeView, CreateCareerInput, CreateCareerResult, CreateHistoricalDraftInput,
    DriverDetail, DriverSummary, FinalizeHistoricalDraftInput, FreeAgentPreview, RaceSummary,
    SaveInfo, TeamHistoryDossier, TeamStanding,
};
use crate::commands::historical_draft::{
    create_historical_career_draft_in_base_dir, discard_career_draft_in_base_dir,
    finalize_career_draft_in_base_dir, get_career_draft_in_base_dir,
};
use crate::commands::race_history::{DriverRaceHistory, PreviousChampions};
use crate::evolution::pipeline::EndOfSeasonResult;
use crate::market::preseason::{PreSeasonState, WeekResult};
use crate::models::driver::Driver;
use crate::news::NewsItem;

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))
}

#[tauri::command]
pub async fn create_career(
    app: AppHandle,
    input: CreateCareerInput,
) -> Result<CreateCareerResult, String> {
    let base_dir = app_data_dir(&app)?;
    create_career_in_base_dir(&base_dir, input)
}

#[tauri::command]
pub async fn create_historical_career_draft(
    app: AppHandle,
    input: CreateHistoricalDraftInput,
) -> Result<CareerDraftState, String> {
    let base_dir = app_data_dir(&app)?;
    create_historical_career_draft_in_base_dir(&base_dir, input)
}

#[tauri::command]
pub fn get_career_draft(app: AppHandle) -> Result<CareerDraftState, String> {
    let base_dir = app_data_dir(&app)?;
    get_career_draft_in_base_dir(&base_dir)
}

#[tauri::command]
pub async fn discard_career_draft(app: AppHandle) -> Result<(), String> {
    let base_dir = app_data_dir(&app)?;
    discard_career_draft_in_base_dir(&base_dir)
}

#[tauri::command]
pub async fn finalize_career_draft(
    app: AppHandle,
    input: FinalizeHistoricalDraftInput,
) -> Result<CreateCareerResult, String> {
    let base_dir = app_data_dir(&app)?;
    finalize_career_draft_in_base_dir(&base_dir, input)
}

#[tauri::command]
pub async fn load_career(app: AppHandle, career_id: String) -> Result<CareerData, String> {
    let base_dir = app_data_dir(&app)?;
    load_career_in_base_dir(&base_dir, &career_id)
}

#[tauri::command]
pub async fn advance_season(
    app: AppHandle,
    career_id: String,
) -> Result<EndOfSeasonResult, String> {
    let base_dir = app_data_dir(&app)?;
    advance_season_in_base_dir(&base_dir, &career_id)
}

#[tauri::command]
pub async fn skip_all_pending_races(app: AppHandle, career_id: String) -> Result<(), String> {
    let base_dir = app_data_dir(&app)?;
    skip_all_pending_races_in_base_dir(&base_dir, &career_id)
}

#[tauri::command]
pub async fn advance_market_week(app: AppHandle, career_id: String) -> Result<WeekResult, String> {
    let base_dir = app_data_dir(&app)?;
    advance_market_week_in_base_dir(&base_dir, &career_id)
}

#[tauri::command]
pub async fn get_preseason_state(
    app: AppHandle,
    career_id: String,
) -> Result<PreSeasonState, String> {
    let base_dir = app_data_dir(&app)?;
    get_preseason_state_in_base_dir(&base_dir, &career_id)
}

#[tauri::command]
pub async fn finalize_preseason(app: AppHandle, career_id: String) -> Result<(), String> {
    let base_dir = app_data_dir(&app)?;
    finalize_preseason_in_base_dir(&base_dir, &career_id)
}

#[tauri::command]
pub async fn set_career_resume_context(
    app: AppHandle,
    career_id: String,
    active_view: CareerResumeView,
    end_of_season_result: Option<EndOfSeasonResult>,
) -> Result<(), String> {
    let base_dir = app_data_dir(&app)?;
    persist_resume_context_in_base_dir(&base_dir, &career_id, active_view, end_of_season_result)
}

#[tauri::command]
pub async fn get_player_proposals(
    app: AppHandle,
    career_id: String,
) -> Result<Vec<PlayerProposalView>, String> {
    let base_dir = app_data_dir(&app)?;
    get_player_proposals_in_base_dir(&base_dir, &career_id)
}

#[tauri::command]
pub async fn respond_to_proposal(
    app: AppHandle,
    career_id: String,
    proposal_id: String,
    accept: bool,
) -> Result<ProposalResponse, String> {
    let base_dir = app_data_dir(&app)?;
    respond_to_proposal_in_base_dir(&base_dir, &career_id, &proposal_id, accept)
}

#[tauri::command]
pub async fn get_news(
    app: AppHandle,
    career_id: String,
    season: Option<i32>,
    tipo: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<NewsItem>, String> {
    let base_dir = app_data_dir(&app)?;
    get_news_in_base_dir(&base_dir, &career_id, season, tipo.as_deref(), limit)
}

#[tauri::command]
pub async fn delete_career(app: AppHandle, career_id: String) -> Result<String, String> {
    let base_dir = app_data_dir(&app)?;
    delete_career_in_base_dir(&base_dir, &career_id)
}

#[tauri::command]
pub fn list_saves(app: AppHandle) -> Result<Vec<SaveInfo>, String> {
    let base_dir = app_data_dir(&app)?;
    list_saves_in_base_dir(&base_dir)
}

#[tauri::command]
pub async fn get_drivers_by_category(
    app: AppHandle,
    career_id: String,
    category: String,
) -> Result<Vec<DriverSummary>, String> {
    let base_dir = app_data_dir(&app)?;
    get_drivers_by_category_in_base_dir(&base_dir, &career_id, &category)
}

#[tauri::command]
pub async fn get_teams_standings(
    app: AppHandle,
    career_id: String,
    category: String,
) -> Result<Vec<TeamStanding>, String> {
    let base_dir = app_data_dir(&app)?;
    get_teams_standings_in_base_dir(&base_dir, &career_id, &category)
}

#[tauri::command]
pub async fn get_team_history_dossier(
    app: AppHandle,
    career_id: String,
    team_id: String,
    category: String,
) -> Result<TeamHistoryDossier, String> {
    let base_dir = app_data_dir(&app)?;
    get_team_history_dossier_in_base_dir(&base_dir, &career_id, &team_id, &category)
}

#[tauri::command]
pub async fn get_race_results_by_category(
    app: AppHandle,
    career_id: String,
    category: String,
) -> Result<Vec<DriverRaceHistory>, String> {
    let base_dir = app_data_dir(&app)?;
    get_race_results_by_category_in_base_dir(&base_dir, &career_id, &category)
}

#[tauri::command]
pub async fn get_previous_champions(
    app: AppHandle,
    career_id: String,
    category: String,
) -> Result<PreviousChampions, String> {
    let base_dir = app_data_dir(&app)?;
    get_previous_champions_in_base_dir(&base_dir, &career_id, &category)
}

#[tauri::command]
pub async fn get_calendar_for_category(
    app: AppHandle,
    career_id: String,
    category: String,
) -> Result<Vec<RaceSummary>, String> {
    let base_dir = app_data_dir(&app)?;
    get_calendar_for_category_in_base_dir(&base_dir, &career_id, &category)
}

#[tauri::command]
pub fn get_driver(app: AppHandle, career_number: u32, driver_id: String) -> Result<Driver, String> {
    let base_dir = app_data_dir(&app)?;
    get_driver_in_base_dir(&base_dir, career_number, &driver_id)
}

#[tauri::command]
pub async fn get_driver_detail(
    app: AppHandle,
    career_id: String,
    driver_id: String,
) -> Result<DriverDetail, String> {
    let base_dir = app_data_dir(&app)?;
    get_driver_detail_in_base_dir(&base_dir, &career_id, &driver_id)
}

#[tauri::command]
pub async fn get_briefing_phrase_history(
    app: AppHandle,
    career_id: String,
) -> Result<BriefingPhraseHistory, String> {
    let base_dir = app_data_dir(&app)?;
    get_briefing_phrase_history_in_base_dir(&base_dir, &career_id)
}

#[tauri::command]
pub async fn save_briefing_phrase_history(
    app: AppHandle,
    career_id: String,
    season_number: i32,
    entries: Vec<BriefingPhraseEntryInput>,
) -> Result<BriefingPhraseHistory, String> {
    let base_dir = app_data_dir(&app)?;
    save_briefing_phrase_history_in_base_dir(&base_dir, &career_id, season_number, entries)
}

#[tauri::command]
pub async fn get_preseason_free_agents(
    app: AppHandle,
    career_id: String,
) -> Result<Vec<FreeAgentPreview>, String> {
    let base_dir = app_data_dir(&app)?;
    get_preseason_free_agents_in_base_dir(&base_dir, &career_id)
}
