use serde::{Deserialize, Serialize};

use crate::commands::race_history::{RoundResult, TrophyInfo};
use crate::event_interest::EventInterestSummary;
use crate::evolution::pipeline::EndOfSeasonResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveLifecycleStatus {
    Draft,
    Failed,
    Active,
}

impl Default for SaveLifecycleStatus {
    fn default() -> Self {
        Self::Active
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCareerInput {
    pub player_name: String,
    pub player_nationality: String,
    pub player_age: Option<i32>,
    pub category: String,
    pub team_index: usize,
    pub difficulty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCareerResult {
    pub success: bool,
    pub career_id: String,
    pub save_path: String,
    pub player_id: String,
    pub player_team_id: String,
    pub player_team_name: String,
    pub season_id: String,
    pub total_drivers: usize,
    pub total_teams: usize,
    pub total_races: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateHistoricalDraftInput {
    pub player_name: String,
    pub player_nationality: String,
    pub player_age: Option<i32>,
    pub difficulty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizeHistoricalDraftInput {
    pub career_id: String,
    pub category: String,
    pub team_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftTeamOption {
    pub id: String,
    pub nome: String,
    pub nome_curto: String,
    pub categoria: String,
    pub cor_primaria: String,
    pub cor_secundaria: String,
    pub car_performance: f64,
    pub reputacao: f64,
    pub n1_nome: Option<String>,
    pub n2_nome: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CareerDraftState {
    pub exists: bool,
    pub career_id: Option<String>,
    pub lifecycle_status: SaveLifecycleStatus,
    pub progress_year: Option<u32>,
    pub error: Option<String>,
    pub categories: Vec<String>,
    pub teams: Vec<DraftTeamOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveInfo {
    pub career_id: String,
    pub player_name: String,
    pub category: String,
    pub category_name: String,
    pub season: i32,
    pub year: i32,
    pub difficulty: String,
    pub created: String,
    pub last_played: String,
    pub total_races: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CareerData {
    pub career_id: String,
    pub save_path: String,
    pub difficulty: String,
    pub player: DriverSummary,
    pub player_team: Option<TeamSummary>,
    pub season: SeasonSummary,
    #[serde(default)]
    pub accepted_special_offer: Option<AcceptedSpecialOfferSummary>,
    pub next_race: Option<RaceSummary>,
    pub next_race_briefing: Option<NextRaceBriefingSummary>,
    pub total_drivers: usize,
    pub total_teams: usize,
    #[serde(default)]
    pub resume_context: Option<CareerResumeContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CareerResumeView {
    Dashboard,
    EndOfSeason,
    Preseason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CareerResumeContext {
    pub active_view: CareerResumeView,
    #[serde(default)]
    pub end_of_season_result: Option<EndOfSeasonResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverSummary {
    pub id: String,
    pub nome: String,
    pub nacionalidade: String,
    pub idade: i32,
    pub skill: u8,
    #[serde(default)]
    pub categoria_especial_ativa: Option<String>,
    pub equipe_id: Option<String>,
    pub equipe_nome: Option<String>,
    pub equipe_nome_curto: Option<String>,
    pub equipe_cor: String,
    #[serde(default)]
    pub classe: Option<String>,
    pub is_jogador: bool,
    pub pontos: i32,
    pub vitorias: i32,
    pub podios: i32,
    pub posicao_campeonato: i32,
    pub results: Vec<Option<RoundResult>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamSummary {
    pub id: String,
    pub nome: String,
    pub nome_curto: String,
    pub cor_primaria: String,
    pub cor_secundaria: String,
    pub categoria: String,
    #[serde(default)]
    pub classe: Option<String>,
    pub car_performance: f64,
    #[serde(default)]
    pub car_build_profile: String,
    pub confiabilidade: f64,
    #[serde(default)]
    pub pit_strategy_risk: f64,
    #[serde(default)]
    pub pit_crew_quality: f64,
    pub budget: f64,
    #[serde(default)]
    pub spending_power: f64,
    #[serde(default)]
    pub salary_ceiling: f64,
    #[serde(default)]
    pub budget_index: f64,
    #[serde(default)]
    pub cash_balance: f64,
    #[serde(default)]
    pub debt_balance: f64,
    #[serde(default)]
    pub financial_state: String,
    #[serde(default)]
    pub season_strategy: String,
    #[serde(default)]
    pub last_round_income: f64,
    #[serde(default)]
    pub last_round_expenses: f64,
    #[serde(default)]
    pub last_round_net: f64,
    #[serde(default)]
    pub parachute_payment_remaining: f64,
    pub piloto_1_id: Option<String>,
    pub piloto_1_nome: Option<String>,
    #[serde(default)]
    pub piloto_1_salario_anual: Option<f64>,
    pub piloto_2_id: Option<String>,
    pub piloto_2_nome: Option<String>,
    #[serde(default)]
    pub piloto_2_salario_anual: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonSummary {
    pub id: String,
    pub numero: i32,
    pub ano: i32,
    pub rodada_atual: i32,
    pub total_rodadas: i32,
    pub status: String,
    pub fase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptedSpecialOfferSummary {
    pub id: String,
    pub team_id: String,
    pub team_name: String,
    pub special_category: String,
    pub class_name: String,
    pub papel: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialWindowPayload {
    pub current_day: i32,
    pub total_days: i32,
    pub status: String,
    #[serde(default)]
    pub active_offer_id: Option<String>,
    #[serde(default)]
    pub player_result: Option<String>,
    #[serde(default)]
    pub team_sections: Vec<SpecialWindowCategorySection>,
    #[serde(default)]
    pub eligible_candidates: Vec<SpecialWindowEligibleCandidate>,
    #[serde(default)]
    pub player_offers: Vec<SpecialWindowPlayerOffer>,
    #[serde(default)]
    pub last_day_log: Vec<SpecialWindowLogEntry>,
    pub can_advance_day: bool,
    pub can_confirm_special_block: bool,
    pub is_finished: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialWindowCategorySection {
    pub category: String,
    pub label: String,
    pub teams: Vec<SpecialWindowTeamSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialWindowTeamSummary {
    pub id: String,
    pub nome: String,
    pub nome_curto: String,
    pub cor_primaria: String,
    pub cor_secundaria: String,
    pub categoria: String,
    #[serde(default)]
    pub classe: Option<String>,
    #[serde(default)]
    pub piloto_1_id: Option<String>,
    #[serde(default)]
    pub piloto_1_nome: Option<String>,
    #[serde(default)]
    pub piloto_1_new_badge_day: Option<i32>,
    #[serde(default)]
    pub piloto_2_id: Option<String>,
    #[serde(default)]
    pub piloto_2_nome: Option<String>,
    #[serde(default)]
    pub piloto_2_new_badge_day: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialWindowEligibleCandidate {
    pub driver_id: String,
    pub driver_name: String,
    pub origin_category: String,
    pub license_nivel: String,
    pub license_sigla: String,
    pub desirability: i32,
    pub production_eligible: bool,
    pub endurance_eligible: bool,
    #[serde(default)]
    pub championship_position: Option<i32>,
    #[serde(default)]
    pub championship_total_drivers: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialWindowPlayerOffer {
    pub id: String,
    pub team_id: String,
    pub team_name: String,
    pub special_category: String,
    pub class_name: String,
    pub papel: String,
    pub status: String,
    pub available_from_day: i32,
    pub is_available_today: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialWindowLogEntry {
    pub day: i32,
    pub event_type: String,
    pub message: String,
    #[serde(default)]
    pub special_category: Option<String>,
    #[serde(default)]
    pub class_name: Option<String>,
    #[serde(default)]
    pub team_id: Option<String>,
    #[serde(default)]
    pub driver_id: Option<String>,
    #[serde(default)]
    pub team_name: Option<String>,
    #[serde(default)]
    pub driver_name: Option<String>,
    #[serde(default)]
    pub driver_origin_category: Option<String>,
    #[serde(default)]
    pub driver_license_nivel: Option<String>,
    #[serde(default)]
    pub driver_license_sigla: Option<String>,
    #[serde(default)]
    pub championship_position: Option<i32>,
    #[serde(default)]
    pub championship_total_drivers: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceSummary {
    pub id: String,
    pub rodada: i32,
    pub track_name: String,
    pub clima: String,
    pub duracao_corrida_min: i32,
    pub status: String,
    pub temperatura: f64,
    pub horario: String,
    pub week_of_year: i32,
    pub season_phase: String,
    pub display_date: String,
    pub event_interest: Option<EventInterestSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractWarningInfo {
    pub temporada_fim: i32,
    pub equipe_nome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextRaceBriefingSummary {
    pub track_history: Option<TrackHistorySummary>,
    pub primary_rival: Option<PrimaryRivalSummary>,
    #[serde(default)]
    pub weekend_stories: Vec<BriefingStorySummary>,
    pub contract_warning: Option<ContractWarningInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackHistorySummary {
    pub has_data: bool,
    pub starts: i32,
    pub best_finish: Option<i32>,
    pub last_finish: Option<i32>,
    pub dnfs: i32,
    pub last_visit_season: Option<i32>,
    pub last_visit_round: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimaryRivalSummary {
    pub driver_id: String,
    pub driver_name: String,
    pub championship_position: i32,
    pub gap_points: i32,
    pub is_ahead: bool,
    pub rivalry_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingStorySummary {
    pub id: String,
    pub icon: String,
    pub title: String,
    pub summary: String,
    pub importance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BriefingPhraseHistory {
    pub season_number: i32,
    #[serde(default)]
    pub entries: Vec<BriefingPhraseEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingPhraseEntry {
    #[serde(default)]
    pub season_number: i32,
    pub round_number: i32,
    pub driver_id: String,
    pub bucket_key: String,
    pub phrase_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingPhraseEntryInput {
    pub round_number: i32,
    pub driver_id: String,
    pub bucket_key: String,
    pub phrase_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsTabBootstrap {
    pub default_scope_type: String,
    pub default_scope_id: String,
    pub default_primary_filter: Option<String>,
    pub default_context_type: Option<String>,
    pub default_context_id: Option<String>,
    pub scopes: Vec<NewsTabScopeTab>,
    pub season_number: i32,
    pub season_year: i32,
    pub current_round: i32,
    pub total_rounds: i32,
    pub season_completed: bool,
    pub pub_date_label: String,
    pub last_race_name: Option<String>,
    pub next_race_date_label: Option<String>,
    pub next_race_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsTabScopeTab {
    pub id: String,
    pub label: String,
    pub short_label: String,
    pub scope_type: String,
    pub special: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsTabSnapshotRequest {
    pub scope_type: String,
    pub scope_id: String,
    pub scope_class: Option<String>,
    pub primary_filter: Option<String>,
    pub context_type: Option<String>,
    pub context_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsTabSnapshot {
    pub hero: NewsTabHero,
    pub primary_filters: Vec<NewsTabFilterOption>,
    pub contextual_filters: Vec<NewsTabFilterOption>,
    pub stories: Vec<NewsTabStory>,
    pub scope_meta: NewsTabScopeMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsTabHero {
    pub section_label: String,
    pub title: String,
    pub subtitle: String,
    pub badge: String,
    pub badge_tone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsTabFilterOption {
    pub id: String,
    pub label: String,
    pub meta: Option<String>,
    pub tone: Option<String>,
    pub kind: Option<String>,
    pub color_primary: Option<String>,
    pub color_secondary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsTabScopeMeta {
    pub scope_type: String,
    pub scope_id: String,
    pub scope_label: String,
    pub scope_class: Option<String>,
    pub primary_filter: Option<String>,
    pub context_type: Option<String>,
    pub context_id: Option<String>,
    pub context_label: Option<String>,
    pub is_special: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsTabStoryBlock {
    pub label: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsTabStory {
    pub id: String,
    pub icon: String,
    pub title: String,
    pub headline: String,
    pub summary: String,
    pub deck: String,
    pub body_text: String,
    pub blocks: Vec<NewsTabStoryBlock>,
    pub news_type: String,
    pub importance: String,
    pub importance_label: String,
    pub category_label: Option<String>,
    pub meta_label: String,
    pub time_label: String,
    pub entity_label: Option<String>,
    pub driver_label: Option<String>,
    pub team_label: Option<String>,
    pub race_label: Option<String>,
    pub accent_tone: String,
    pub driver_id: Option<String>,
    pub team_id: Option<String>,
    pub round: Option<i32>,

    // 1.1 — campos brutos do NewsItem
    pub original_text: Option<String>,
    pub preseason_week: Option<i32>,
    pub season_number: i32,
    pub driver_id_secondary: Option<String>,
    pub driver_secondary_label: Option<String>,

    // 1.2 — contexto competitivo
    pub driver_position: Option<i32>,
    pub driver_points: Option<i32>,
    pub team_position: Option<i32>,
    pub team_points: Option<i32>,

    // 1.3 — contexto visual de equipe
    pub team_color_primary: Option<String>,
    pub team_color_secondary: Option<String>,

    // 1.4 — próxima etapa
    pub next_race_label: Option<String>,
    pub next_race_date_label: Option<String>,

    // 1.5 — presença pública da equipe
    pub team_public_presence_tier: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverDetail {
    pub id: String,
    pub nome: String,
    pub nacionalidade: String,
    pub idade: i32,
    pub genero: String,
    pub is_jogador: bool,
    pub status: String,
    pub equipe_id: Option<String>,
    pub equipe_nome: Option<String>,
    pub equipe_cor_primaria: Option<String>,
    pub equipe_cor_secundaria: Option<String>,
    pub papel: Option<String>,
    pub personalidade_primaria: Option<PersonalityInfo>,
    pub personalidade_secundaria: Option<PersonalityInfo>,
    pub motivacao: u8,
    pub tags: Vec<TagInfo>,
    pub stats_temporada: StatsBlock,
    pub stats_carreira: StatsBlock,
    pub contrato: Option<ContractDetail>,
    pub perfil: DriverProfileBlock,
    pub competitivo: DriverCompetitiveBlock,
    pub performance: DriverPerformanceBlock,
    pub forma: DriverFormBlock,
    pub trajetoria: DriverCareerPathBlock,
    pub contrato_mercado: DriverContractMarketBlock,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relacionamentos: Option<DriverRelationshipsBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reputacao: Option<DriverReputationBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saude: Option<DriverHealthBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityInfo {
    pub tipo: String,
    pub emoji: String,
    pub descricao: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagInfo {
    pub attribute_name: String,
    pub tag_text: String,
    pub level: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsBlock {
    pub corridas: i32,
    pub pontos: i32,
    pub vitorias: i32,
    pub podios: i32,
    pub poles: i32,
    pub melhor_resultado: i32,
    pub dnfs: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractDetail {
    pub equipe_nome: String,
    pub papel: String,
    pub salario_anual: f64,
    pub temporada_inicio: i32,
    pub temporada_fim: i32,
    pub anos_restantes: i32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverProfileBlock {
    pub nome: String,
    pub bandeira: String,
    pub nacionalidade: String,
    pub idade: i32,
    pub genero: String,
    pub status: String,
    pub is_jogador: bool,
    pub equipe_nome: Option<String>,
    pub papel: Option<String>,
    pub licenca: DriverLicenseInfo,
    pub badges: Vec<DriverBadge>,
    pub equipe_cor_primaria: Option<String>,
    pub equipe_cor_secundaria: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverLicenseInfo {
    pub nivel: String,
    pub sigla: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverBadge {
    pub label: String,
    pub variant: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverCompetitiveBlock {
    pub personalidade_primaria: Option<PersonalityInfo>,
    pub personalidade_secundaria: Option<PersonalityInfo>,
    pub motivacao: u8,
    pub qualidades: Vec<TagInfo>,
    pub defeitos: Vec<TagInfo>,
    pub neutro: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverPerformanceBlock {
    pub temporada: PerformanceStatsBlock,
    pub carreira: PerformanceStatsBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStatsBlock {
    pub vitorias: i32,
    pub podios: i32,
    pub top_10: Option<i32>,
    pub fora_top_10: Option<i32>,
    pub poles: i32,
    pub voltas_rapidas: Option<i32>,
    pub hat_tricks: Option<i32>,
    pub corridas: i32,
    pub dnfs: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverFormBlock {
    pub ultimas_5: Vec<FormResultEntry>,
    pub media_chegada: Option<f64>,
    pub tendencia: String,
    pub momento: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormResultEntry {
    pub rodada: i32,
    pub chegada: Option<i32>,
    pub dnf: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverCareerPathBlock {
    pub ano_estreia: i32,
    pub equipe_estreia: Option<String>,
    pub categoria_atual: Option<String>,
    pub temporadas_na_categoria: i32,
    pub corridas_na_categoria: i32,
    pub titulos: i32,
    pub foi_campeao: bool,
    pub marcos: Vec<CareerMilestone>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CareerMilestone {
    pub tipo: String,
    pub titulo: String,
    pub descricao: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverContractMarketBlock {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contrato: Option<ContractDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mercado: Option<DriverMarketBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverMarketBlock {
    pub valor_mercado: Option<f64>,
    pub salario_estimado: Option<f64>,
    pub chance_transferencia: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverRelationshipsBlock {
    pub rival_principal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverReputationBlock {
    pub popularidade: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverHealthBlock {
    pub saude_geral: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamStanding {
    pub posicao: i32,
    pub id: String,
    pub nome: String,
    pub nome_curto: String,
    pub cor_primaria: String,
    #[serde(default)]
    pub cash_balance: f64,
    #[serde(default)]
    pub car_performance: f64,
    #[serde(default)]
    pub car_build_profile: String,
    pub pontos: i32,
    pub vitorias: i32,
    pub piloto_1_nome: Option<String>,
    pub piloto_1_tenure_seasons: Option<i32>,
    pub piloto_2_nome: Option<String>,
    pub piloto_2_tenure_seasons: Option<i32>,
    pub trofeus: Vec<TrophyInfo>,
    pub classe: Option<String>,
    pub temp_posicao: i32,
    pub categoria_anterior: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryDossier {
    pub team_id: String,
    pub category: String,
    pub record_scope: String,
    pub has_history: bool,
    pub records: Vec<TeamHistoryRecord>,
    pub sport: TeamHistorySport,
    pub identity: TeamHistoryIdentity,
    pub management: TeamHistoryManagement,
    pub timeline: Vec<TeamHistoryTimelineItem>,
    pub title_categories: Vec<TeamHistoryTitleCategory>,
    pub category_path: Vec<TeamHistoryCategoryStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryRecord {
    pub label: String,
    pub rank: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistorySport {
    pub seasons: String,
    pub current_streak: String,
    pub best_streak: String,
    pub podium_rate: String,
    pub win_rate: String,
    pub races: i32,
    pub wins: i32,
    pub podiums: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryIdentity {
    pub origin: String,
    pub current: String,
    pub profile: String,
    pub summary: String,
    pub rival: TeamHistoryRival,
    pub symbol_driver: String,
    pub symbol_driver_detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryRival {
    pub name: String,
    pub current_category: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryManagement {
    pub peak_cash: String,
    pub worst_crisis: String,
    pub healthy_years: String,
    pub efficiency: String,
    pub biggest_investment: String,
    pub summary: String,
    pub peak_cash_detail: String,
    pub worst_crisis_detail: String,
    pub healthy_years_detail: String,
    pub efficiency_detail: String,
    pub investment_detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryTimelineItem {
    pub year: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryTitleCategory {
    pub category: String,
    pub year: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryCategoryStep {
    pub category: String,
    pub years: String,
    pub detail: String,
    pub color: String,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct VerifyDatabaseResponse {
    pub career_number: u32,
    pub db_path: String,
    pub table_count: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeAgentPreview {
    pub driver_id: String,
    pub driver_name: String,
    pub categoria: String,
    pub is_rookie: bool,
    pub previous_team_name: Option<String>,
    pub previous_team_color: Option<String>,
    pub previous_team_abbr: Option<String>,
    pub seasons_at_last_team: i32,
    pub total_career_seasons: i32,
    pub license_nivel: String,
    pub license_sigla: String,
    pub last_championship_position: Option<i32>,
    pub last_championship_total_drivers: Option<i32>,
}
