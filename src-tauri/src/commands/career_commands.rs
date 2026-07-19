use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::commands::career::{
    advance_market_week_in_base_dir, advance_season_in_base_dir, create_career_in_base_dir,
    debug_poaching_auctions_in_base_dir, debug_prepare_market_scenario_in_base_dir,
    debug_stamp_player_championship_in_base_dir,
    delete_career_in_base_dir,
    finalize_preseason_in_base_dir,
    get_briefing_phrase_history_in_base_dir, get_calendar_for_category_in_base_dir,
    get_driver_detail_in_base_dir, get_driver_in_base_dir, get_drivers_by_category_in_base_dir,
    get_player_dossier_in_base_dir, get_player_interests_in_base_dir,
    debug_force_player_poach_offer_in_base_dir,
    get_news_in_base_dir, get_player_poach_offer_in_base_dir, get_player_proposals_in_base_dir,
    get_preseason_free_agents_in_base_dir,
    get_preseason_state_in_base_dir, get_previous_champions_in_base_dir,
    resolve_player_poach_offer_in_base_dir,
    get_race_results_by_category_in_base_dir, get_team_finance_report_in_base_dir,
    get_team_history_dossier_in_base_dir,
    get_teams_standings_in_base_dir, list_saves_in_base_dir, load_career_in_base_dir,
    persist_resume_context_in_base_dir, respond_to_proposal_in_base_dir,
    save_briefing_phrase_history_in_base_dir, skip_all_pending_races_in_base_dir,
    PlayerInterests, PlayerProposalView, ProposalResponse,
};
use crate::commands::career_types::{
    BriefingPhraseEntryInput, BriefingPhraseHistory, CareerData, CareerDraftState,
    CareerResumeView, CreateCareerInput, CreateCareerResult, CreateHistoricalDraftInput,
    DriverDetail, DriverSummary, FinalizeHistoricalDraftInput, FreeAgentPreview,
    GlobalDriverRankingPayload, GlobalTeamHistoryPayload, RaceSummary, SaveInfo,
    TeamFinanceReport, TeamHistoryDossier, TeamStanding,
};
use crate::commands::global_driver_rankings::get_global_driver_rankings_in_base_dir;
use crate::commands::global_team_history::get_global_team_history_in_base_dir;
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

/// DEBUG: prepara o mercado num cenário (agente livre + posição forçada) antes de o
/// chamador avançar a temporada. Cenários: "no_team", "first", "fifth".
#[tauri::command]
pub async fn debug_prepare_market_scenario(
    app: AppHandle,
    career_id: String,
    scenario: String,
) -> Result<(), String> {
    let base_dir = app_data_dir(&app)?;
    debug_prepare_market_scenario_in_base_dir(&base_dir, &career_id, &scenario)
}

/// Quebra de contrato do jogador (Fase 2b.3): a oferta ativa da janela, ou null.
#[tauri::command]
pub async fn get_player_poach_offer(
    app: AppHandle,
    career_id: String,
) -> Result<Option<crate::market::pipeline::PlayerPoachOffer>, String> {
    let base_dir = app_data_dir(&app)?;
    get_player_poach_offer_in_base_dir(&base_dir, &career_id)
}

/// Resolve a decisão do jogador na quebra de contrato (accept = sair; false = ficar).
#[tauri::command]
pub async fn resolve_player_poach_offer(
    app: AppHandle,
    career_id: String,
    offer: crate::market::pipeline::PlayerPoachOffer,
    accept: bool,
) -> Result<crate::market::pipeline::PlayerPoachOutcome, String> {
    let base_dir = app_data_dir(&app)?;
    resolve_player_poach_offer_in_base_dir(&base_dir, &career_id, &offer, accept)
}

/// DEBUG: força uma proposta de quebra de contrato pro jogador (Fase 2b.3).
#[tauri::command]
pub async fn debug_force_player_poach_offer(
    app: AppHandle,
    career_id: String,
) -> Result<Option<crate::market::pipeline::PlayerPoachOffer>, String> {
    let base_dir = app_data_dir(&app)?;
    debug_force_player_poach_offer_in_base_dir(&base_dir, &career_id)
}

/// DEBUG: simula o leilão de poaching (Fase 2b.2) e DESFAZ tudo, devolvendo o
/// raio-x de cada assédio. Não altera o save.
#[tauri::command]
pub async fn debug_poaching_auctions(
    app: AppHandle,
    career_id: String,
) -> Result<crate::commands::career::PoachDebugReport, String> {
    let base_dir = app_data_dir(&app)?;
    debug_poaching_auctions_in_base_dir(&base_dir, &career_id)
}

/// DEBUG: carimba a posição do jogador no arquivo APÓS o avanço da temporada (o avanço
/// recalcula standings só de quem correu e exclui o agente livre). Cenários: "first"/"fifth".
#[tauri::command]
pub async fn debug_stamp_player_championship(
    app: AppHandle,
    career_id: String,
    scenario: String,
) -> Result<(), String> {
    let base_dir = app_data_dir(&app)?;
    debug_stamp_player_championship_in_base_dir(&base_dir, &career_id, &scenario)
}

#[tauri::command]
pub async fn advance_market_week(
    app: AppHandle,
    career_id: String,
    accepted_seat_id: Option<String>,
) -> Result<WeekResult, String> {
    let base_dir = app_data_dir(&app)?;
    advance_market_week_in_base_dir(&base_dir, &career_id, accepted_seat_id.as_deref())
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

/// Os pilotos de interesse do jogador (1 Nemesis + até 2 Rivais) para decorar os
/// nomes nas telas com o marcador de rivalidade. Vem do estado acumulado do motor.
#[tauri::command]
pub async fn get_player_interests(
    app: AppHandle,
    career_id: String,
) -> Result<PlayerInterests, String> {
    let base_dir = app_data_dir(&app)?;
    get_player_interests_in_base_dir(&base_dir, &career_id)
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
pub async fn get_team_finance_report(
    app: AppHandle,
    career_id: String,
    category: String,
    team_id: String,
) -> Result<TeamFinanceReport, String> {
    let base_dir = app_data_dir(&app)?;
    get_team_finance_report_in_base_dir(&base_dir, &career_id, &category, &team_id)
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

/// Dossiê de habilidade do jogador (atributos inferidos do desempenho real; só
/// visual). Ver `crate::player_skill`.
#[tauri::command]
pub fn get_player_dossier(
    app: AppHandle,
    career_id: String,
) -> Result<crate::player_skill::PlayerDossier, String> {
    let base_dir = app_data_dir(&app)?;
    get_player_dossier_in_base_dir(&base_dir, &career_id)
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
pub async fn get_global_driver_rankings(
    app: AppHandle,
    career_id: String,
    selected_driver_id: Option<String>,
) -> Result<GlobalDriverRankingPayload, String> {
    let base_dir = app_data_dir(&app)?;
    get_global_driver_rankings_in_base_dir(&base_dir, &career_id, selected_driver_id.as_deref())
}

/// Favorita/desfavorita um piloto (watchlist). Devolve o NOVO estado (true = agora
/// favoritado). Alimenta a ênfase do feed do mercado + o filtro "Favoritos".
#[tauri::command]
pub async fn toggle_driver_favorite(
    app: AppHandle,
    career_id: String,
    driver_id: String,
) -> Result<bool, String> {
    let base_dir = app_data_dir(&app)?;
    crate::commands::career::toggle_driver_favorite_in_base_dir(&base_dir, &career_id, &driver_id)
}

/// Estado da Janela de Transferências (Fase 2): ofertas do jogador + feed.
#[tauri::command]
pub async fn get_transfer_window_state(
    app: AppHandle,
    career_id: String,
) -> Result<crate::commands::transfer_market::TransferWindowPayload, String> {
    let base_dir = app_data_dir(&app)?;
    crate::commands::transfer_market::get_transfer_window_state_in_base_dir(&base_dir, &career_id)
}

/// Avança uma semana da Janela de Transferências com a escolha do jogador.
#[tauri::command]
pub async fn advance_transfer_window(
    app: AppHandle,
    career_id: String,
    accepted_seat_id: Option<String>,
) -> Result<crate::commands::transfer_market::TransferWindowPayload, String> {
    let base_dir = app_data_dir(&app)?;
    crate::commands::transfer_market::advance_transfer_window_in_base_dir(
        &base_dir,
        &career_id,
        accepted_seat_id.as_deref(),
    )
}

#[tauri::command]
pub async fn get_global_team_history(
    app: AppHandle,
    career_id: String,
    family: Option<String>,
    start_year: Option<i32>,
    window_size: Option<i32>,
) -> Result<GlobalTeamHistoryPayload, String> {
    let base_dir = app_data_dir(&app)?;
    get_global_team_history_in_base_dir(
        &base_dir,
        &career_id,
        family.as_deref(),
        start_year,
        window_size,
    )
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
