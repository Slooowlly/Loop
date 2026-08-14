use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::commands::career::{
    advance_market_week_in_base_dir, advance_season_in_base_dir, create_career_in_base_dir,
    delete_career_in_base_dir, finalize_preseason_in_base_dir,
    get_briefing_phrase_history_in_base_dir, get_calendar_for_category_in_base_dir,
    get_displaced_driver_context_in_base_dir, get_driver_detail_in_base_dir,
    get_driver_dossier_ranks_in_base_dir, get_drivers_by_category_in_base_dir,
    get_news_in_base_dir, get_player_dossier_in_base_dir, get_player_interests_in_base_dir,
    get_player_poach_offer_in_base_dir, get_player_proposals_in_base_dir,
    get_preseason_free_agents_in_base_dir, get_preseason_state_in_base_dir,
    get_previous_champions_in_base_dir, get_race_reading_in_base_dir,
    get_race_results_by_category_in_base_dir, get_season_champion_payload_in_base_dir,
    get_season_market_board_in_base_dir, get_teams_car_parts_in_base_dir,
    get_teams_standings_in_base_dir, list_saves_in_base_dir, load_career_in_base_dir,
    persist_resume_context_in_base_dir, resolve_player_poach_offer_in_base_dir,
    respond_to_proposal_in_base_dir, save_briefing_phrase_history_in_base_dir,
    skip_all_pending_races_in_base_dir, PlayerInterests, PlayerProposalView, ProposalResponse,
};
#[cfg(debug_assertions)]
use crate::commands::career::{
    debug_force_player_poach_offer_in_base_dir, debug_poaching_auctions_in_base_dir,
    debug_prepare_market_scenario_in_base_dir, debug_skip_to_season_finale_in_base_dir,
    debug_stamp_player_championship_in_base_dir,
};
use crate::commands::career_team_dossier::{
    get_team_finance_report_in_base_dir, get_team_history_dossier_in_base_dir,
    get_team_records_ranking_in_base_dir,
};
use crate::commands::career_types::{
    BandChampionsPayload, BriefingPhraseEntryInput, BriefingPhraseHistory, CareerData,
    CareerDraftState, CareerResumeView, CreateCareerInput, CreateCareerResult,
    CreateHistoricalDraftInput, DriverCareerRankEntry, DriverDetail, DriverSummary,
    DriverWorldRank, FinalizeHistoricalDraftInput, FreeAgentPreview, GlobalDriverRankingPayload,
    GlobalTeamHistoryPayload, RaceReading, RaceSummary, SaveInfo, SeasonChampionPayload,
    SeasonMarketBoard, TeamCarParts, TeamFinanceReport, TeamHistoryDossier, TeamRecordsRanking,
    TeamStanding, UpdateDraftIdentityInput,
};
use crate::commands::global_driver_rankings::{
    get_driver_world_rank_in_base_dir, get_global_driver_rankings_in_base_dir,
};
use crate::commands::global_team_history::{
    get_band_champions_in_base_dir, get_global_team_history_in_base_dir,
};
use crate::commands::historical_draft::{
    create_historical_career_draft_in_base_dir, discard_career_draft_in_base_dir,
    finalize_career_draft_in_base_dir, get_career_draft_in_base_dir,
    update_career_draft_identity_in_base_dir,
};
use crate::commands::race_history::{DriverRaceHistory, PreviousChampions};
use crate::evolution::pipeline::EndOfSeasonResult;
use crate::market::preseason::{PreSeasonState, WeekResult};
use crate::news::NewsItem;

// PORTÃO DOS COMANDOS DE DEPURAÇÃO
//
// São cinco, e quatro deles escrevem no save: `debug_skip_to_season_finale` (simula as
// etapas pendentes), `debug_prepare_market_scenario` (rescinde o contrato do jogador e
// força classificação e fama por SQL cru), `debug_force_player_poach_offer` (grava a
// oferta no plano de pré-temporada) e `debug_stamp_player_championship` (carimba a
// posição no arquivo da temporada). O quinto, `debug_poaching_auctions`, roda dentro de
// uma transação com rollback e não persiste nada.
//
// Nenhum deles pode ser invocável num build de release: qualquer devtools aberto
// corromperia uma carreira real. O portão anterior era um `if cfg!(debug_assertions)` no
// corpo, que ainda deixava o comando REGISTRADO no `invoke_handler` do release — e a
// justificativa escrita ali (a macro não aceitaria atributos condicionais) não vale:
// `tauri::generate_handler!` parseia atributos externos em cada entrada e os repassa para
// o braço do `match`. Então o gate hoje é `#[cfg(debug_assertions)]` em toda a cadeia:
// módulo (`commands/career.rs`), lógica (`commands/career/debug.rs`), casca (aqui) e
// registro (`lib.rs`). No release o nome simplesmente não existe na ponte.
//
// Guard: `scripts/tests/comandos-de-debug-fora-do-release.test.mjs`.

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
    // async + spawn_blocking: o draft simula 26 temporadas (trabalho de MINUTOS) e
    // rodava síncrono dentro do runtime async, segurando os outros comandos. O
    // progresso continua saindo por meta.json, lido pelo polling da tela.
    tauri::async_runtime::spawn_blocking(move || {
        create_historical_career_draft_in_base_dir(&base_dir, input)
    })
    .await
    .map_err(|e| format!("Falha ao executar o draft historico: {e}"))?
}

#[tauri::command]
pub fn get_career_draft(app: AppHandle) -> Result<CareerDraftState, String> {
    let base_dir = app_data_dir(&app)?;
    get_career_draft_in_base_dir(&base_dir)
}

#[tauri::command]
pub async fn update_career_draft_identity(
    app: AppHandle,
    input: UpdateDraftIdentityInput,
) -> Result<CareerDraftState, String> {
    let base_dir = app_data_dir(&app)?;
    update_career_draft_identity_in_base_dir(&base_dir, input)
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

/// Payload da tela "Campeão da Temporada". Sem `category` usa a do jogador; sem
/// `season_number` usa a temporada ativa (passar o número abre um ano já encerrado).
/// `None` quando não há etapa disputada — a UI então não abre o pop-up.
#[tauri::command]
pub async fn get_season_champion_payload(
    app: AppHandle,
    career_id: String,
    category: Option<String>,
    season_number: Option<i32>,
) -> Result<Option<SeasonChampionPayload>, String> {
    let base_dir = app_data_dir(&app)?;
    get_season_champion_payload_in_base_dir(
        &base_dir,
        &career_id,
        category.as_deref(),
        season_number,
    )
}

/// DEBUG: simula tudo menos a última corrida da categoria do jogador, deixando o save
/// a um "Avançar calendário" da final da temporada.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn debug_skip_to_season_finale(app: AppHandle, career_id: String) -> Result<(), String> {
    let base_dir = app_data_dir(&app)?;
    debug_skip_to_season_finale_in_base_dir(&base_dir, &career_id)
}

/// DEBUG: prepara o mercado num cenário (agente livre + posição forçada) antes de o
/// chamador avançar a temporada. Cenários: "no_team", "first", "fifth".
#[cfg(debug_assertions)]
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
#[cfg(debug_assertions)]
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
#[cfg(debug_assertions)]
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
#[cfg(debug_assertions)]
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

/// Níveis das 11 peças do carro de cada equipe da categoria — o detalhe que o
/// `car_level` de `TeamStanding` resume numa média só.
#[tauri::command]
pub async fn get_teams_car_parts(
    app: AppHandle,
    career_id: String,
    category: String,
) -> Result<Vec<TeamCarParts>, String> {
    let base_dir = app_data_dir(&app)?;
    get_teams_car_parts_in_base_dir(&base_dir, &career_id, &category)
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

/// Os assentos vazios do mundo, com o veredito de elegibilidade do jogador em cada
/// um. É o painel de mercado do MEIO da temporada: fora da janela de pré-temporada
/// o jogador não tinha onde perguntar que cadeira abriu. Read-only.
#[tauri::command]
pub async fn get_season_market_board(
    app: AppHandle,
    career_id: String,
) -> Result<SeasonMarketBoard, String> {
    let base_dir = app_data_dir(&app)?;
    get_season_market_board_in_base_dir(&base_dir, &career_id)
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
pub async fn get_team_records_ranking(
    app: AppHandle,
    career_id: String,
    category: String,
    scope: Option<String>,
    class: Option<String>,
) -> Result<TeamRecordsRanking, String> {
    let base_dir = app_data_dir(&app)?;
    get_team_records_ranking_in_base_dir(
        &base_dir,
        &career_id,
        &category,
        scope.as_deref().unwrap_or("group"),
        class.as_deref(),
    )
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

// Não há mais um comando `get_driver` cru. Ele nasceu com a assinatura antiga
// (`career_number: u32`, de antes de todo mundo passar a receber `career_id`) e a tela
// nunca chegou a chamá-lo: quem lê um piloto é `get_driver_detail`, que devolve a ficha
// montada. Saiu em 11/08/2026, junto com `get_driver_in_base_dir`.

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

/// O que o jogador já viveu com cada piloto de uma lista: confronto direto,
/// rivalidade e nêmesis. Ver `DisplacedDriverContext`.
#[tauri::command]
pub fn get_displaced_driver_context(
    app: AppHandle,
    career_id: String,
    driver_ids: Vec<String>,
) -> Result<Vec<crate::commands::career_types::DisplacedDriverContext>, String> {
    let base_dir = app_data_dir(&app)?;
    get_displaced_driver_context_in_base_dir(&base_dir, &career_id, &driver_ids)
}

/// A leitura de uma corrida: traçado de posição por trecho, custo do box, trânsito e
/// safety cars. Ver `RaceReading` e a migração v55.
#[tauri::command]
pub fn get_race_reading(
    app: AppHandle,
    career_id: String,
    race_id: String,
) -> Result<RaceReading, String> {
    let base_dir = app_data_dir(&app)?;
    get_race_reading_in_base_dir(&base_dir, &career_id, &race_id)
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

/// Os recordes do dossie de carreira, so quando o jogador liga o toggle.
///
/// Mesmo desenho de `get_driver_world_rank`, e pelo mesmo motivo: montar isto
/// varre `race_results` e o arquivo de temporadas do mundo inteiro. Dentro do
/// payload da ficha eram 503ms de espera em toda abertura e em toda troca de
/// piloto — 98% do custo do bloco de historico — para alimentar um toggle que
/// nasce desligado.
#[tauri::command]
pub async fn get_driver_dossier_ranks(
    app: AppHandle,
    career_id: String,
    driver_id: String,
) -> Result<std::collections::HashMap<String, DriverCareerRankEntry>, String> {
    let base_dir = app_data_dir(&app)?;
    get_driver_dossier_ranks_in_base_dir(&base_dir, &career_id, &driver_id)
}

/// Posicao do piloto no ranking mundial, para a marca no topo da ficha.
///
/// Comando separado de proposito: `get_driver_detail` responde na hora e a ficha
/// abre; a posicao no mundo exige rodar o ranking inteiro e chega depois, sem
/// segurar a tela. Erro aqui nao e erro da ficha — o front so nao desenha a marca.
#[tauri::command]
pub async fn get_driver_world_rank(
    app: AppHandle,
    career_id: String,
    driver_id: String,
) -> Result<Option<DriverWorldRank>, String> {
    let base_dir = app_data_dir(&app)?;
    get_driver_world_rank_in_base_dir(&base_dir, &career_id, &driver_id)
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
pub async fn get_band_champions(
    app: AppHandle,
    career_id: String,
    band_key: String,
) -> Result<BandChampionsPayload, String> {
    let base_dir = app_data_dir(&app)?;
    get_band_champions_in_base_dir(&base_dir, &career_id, &band_key)
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
