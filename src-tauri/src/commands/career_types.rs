use serde::{Deserialize, Serialize};

use crate::commands::race_history::{RoundResult, TrophyInfo};
use crate::event_interest::EventInterestSummary;
use crate::evolution::pipeline::EndOfSeasonResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SaveLifecycleStatus {
    Draft,
    Failed,
    #[default]
    Active,
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

/// Resumo do mundo simulado (histórico), mostrado na confirmação da carreira.
/// Todas as contagens vêm dos arquivos de temporada persistidos no banco do draft.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSummary {
    pub temporadas: i64,
    pub pilotos: i64,
    pub corridas: i64,
    /// Pilotos DISTINTOS que foram campeões ao menos uma vez (cada um conta 1).
    pub campeoes: i64,
    /// Pilotos que chegaram a 3+ títulos (tricampeões ou mais).
    pub tricampeoes: i64,
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
    pub world_summary: Option<WorldSummary>,
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
    #[serde(default)]
    pub is_estreante: bool,
    #[serde(default)]
    pub is_estreante_da_vida: bool,
    #[serde(default)]
    pub lesao_ativa_tipo: Option<String>,
    /// Piloto que encerrou a carreira (aposentado). Fica congelado na classificação da
    /// temporada com os pontos que somou, mas não volta à pista. Ganha selo na UI.
    #[serde(default)]
    pub is_aposentado: bool,
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
    /// Nível do Carro (1–10) do Sistema de Nível do Carro — a ÚNICA leitura de carro que
    /// o jogador vê. Derivado das 11 peças (média dos níveis).
    #[serde(default)]
    pub car_level: u8,
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
    /// Papel narrativo da corrida (ex.: "FinalDaTemporada"/"FinalEspecial" marcam
    /// o final de campeonato). Usado pela UI para decidir a aba pós-corrida.
    pub thematic_slot: String,
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
    /// Piloto favoritado pelo jogador (watchlist). Comanda a estrela no dossiê.
    #[serde(default)]
    pub is_favorito: bool,
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
    #[serde(default)]
    pub leitura_tecnica: DriverTechnicalReadBlock,
    #[serde(default)]
    pub estrelato: DriverStardomBlock,
    pub performance: DriverPerformanceBlock,
    pub forma: DriverFormBlock,
    #[serde(default)]
    pub resumo_atual: DriverCurrentSummaryBlock,
    #[serde(default)]
    pub leitura_desempenho: DriverPerformanceReadBlock,
    pub trajetoria: DriverCareerPathBlock,
    #[serde(default)]
    pub rankings_carreira: DriverCareerRankBlock,
    #[serde(default)]
    pub rivais: DriverRivalsBlock,
    pub contrato_mercado: DriverContractMarketBlock,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relacionamentos: Option<DriverRelationshipsBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reputacao: Option<DriverReputationBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saude: Option<DriverHealthBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalDriverRankingPayload {
    pub selected_driver_id: Option<String>,
    pub player_driver: Option<GlobalDriverRankingRow>,
    pub rows: Vec<GlobalDriverRankingRow>,
    pub leaders: GlobalDriverRankingLeaders,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalDriverRankingLeaders {
    pub historical_index_driver_id: Option<String>,
    pub wins_driver_id: Option<String>,
    pub titles_driver_id: Option<String>,
    pub injuries_driver_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalDriverRankingRow {
    pub id: String,
    pub nome: String,
    pub nacionalidade: String,
    pub idade: i32,
    pub status: String,
    pub status_tone: String,
    pub is_jogador: bool,
    /// Piloto favoritado pelo jogador (watchlist). Comanda a estrela inline + filtro.
    #[serde(default)]
    pub is_favorito: bool,
    pub is_lesionado: bool,
    pub lesao_ativa_tipo: Option<String>,
    pub equipe_nome: Option<String>,
    pub equipe_cor_primaria: Option<String>,
    pub categoria_atual: Option<String>,
    pub categorias_historicas: Vec<String>,
    pub salario_anual: Option<f64>,
    pub ano_inicio_carreira: Option<i32>,
    pub anos_carreira: Option<i32>,
    pub temporada_aposentadoria: Option<String>,
    pub anos_aposentado: Option<i32>,
    pub historical_index: f64,
    pub historical_rank: i32,
    pub historical_rank_delta: Option<i32>,
    /// Estrelato: fama (`midia`) e carisma atuais, 0–100. `fama_delta` = quanto a
    /// fama subiu/caiu desde o fim da temporada passada (snapshot arquivado); `None`
    /// quando não há histórico para comparar (ex.: 1ª temporada). Alimenta a seta ▲/▼.
    #[serde(default)]
    pub fama: i32,
    #[serde(default)]
    pub carisma: i32,
    #[serde(default)]
    pub fama_delta: Option<i32>,
    pub wins_rank: i32,
    pub titles_rank: i32,
    pub podiums_rank: i32,
    pub injuries_rank: i32,
    pub corridas: i32,
    pub pontos: i32,
    pub vitorias: i32,
    pub podios: i32,
    /// Pódios que NÃO foram vitória, quebrados por posição. Vêm dos resultados reais
    /// (`race_results`), então cobrem a carreira inteira jogada; pilotos históricos
    /// pré-gerados (sem `race_results`) ficam em 0 — o front trata como "sem detalhe".
    /// `segundos + terceiros + vitorias` pode ser < `podios` nesse caso.
    #[serde(default)]
    pub segundos: i32,
    #[serde(default)]
    pub terceiros: i32,
    pub poles: i32,
    pub titulos: i32,
    #[serde(default)]
    pub titulos_por_categoria: Vec<GlobalDriverTitleCategorySummary>,
    pub dnfs: i32,
    pub lesoes: i32,
    pub lesoes_leves: i32,
    pub lesoes_moderadas: i32,
    pub lesoes_graves: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalDriverTitleCategorySummary {
    pub categoria: String,
    #[serde(default)]
    pub classe: Option<String>,
    pub titulos: i32,
    #[serde(default)]
    pub anos: Vec<i32>,
    /// Equipe com a qual o piloto conquistou cada título, na mesma ordem de `anos`.
    /// O nome é resolvido pela identidade atual do `team_id` (o nome histórico não é
    /// arquivado), e `equipe`/`equipe_cor` ficam nulos quando o time não é resolvível.
    #[serde(default)]
    pub anos_equipes: Vec<GlobalDriverTitleYearTeam>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalDriverTitleYearTeam {
    pub ano: i32,
    #[serde(default)]
    pub equipe: Option<String>,
    #[serde(default)]
    pub equipe_cor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTeamHistoryPayload {
    pub selected_family: String,
    pub min_year: i32,
    pub max_year: i32,
    pub window_start: i32,
    pub window_end: i32,
    pub window_size: i32,
    /// Year of the currently-active season in the career.  Used by the frontend
    /// to set the axis end (current_year + 5) so the hatch zone extends a few
    /// years into the future.  Falls back to `max_year` when no active season
    /// exists (e.g. pre-game or historical draft).
    pub current_year: i32,
    pub families: Vec<GlobalTeamHistoryFamily>,
    pub bands: Vec<GlobalTeamHistoryBand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTeamHistoryFamily {
    pub id: String,
    pub label: String,
    pub bands: Vec<GlobalTeamHistoryFamilyBand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTeamHistoryFamilyBand {
    pub key: String,
    pub label: String,
    pub category: String,
    #[serde(default)]
    pub class_name: Option<String>,
    pub starts_year: i32,
    pub is_special: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTeamHistoryBand {
    pub key: String,
    pub label: String,
    pub category: String,
    #[serde(default)]
    pub class_name: Option<String>,
    pub starts_year: i32,
    pub is_special: bool,
    pub rows: Vec<GlobalTeamHistoryTeamRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamTitleCount {
    /// Matches a key in `TeamHistoryBandDef` (e.g. "mazda_rookie", "production_mazda").
    /// Stable identifier — the frontend maps this to a band-specific trophy image.
    pub band_key: String,
    pub band_label: String,
    pub count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTeamHistoryTeamRow {
    pub team_id: String,
    pub nome: String,
    pub nome_curto: String,
    pub cor_primaria: String,
    pub cor_secundaria: String,
    pub base_position: i32,
    /// All-time constructor titles for this team within the displayed family, grouped by
    /// band (level). Ordered from lowest band index to highest (e.g. Rookie → Cup →
    /// Production). Empty when the team has never won a championship in this family.
    pub titles: Vec<TeamTitleCount>,
    /// True when this team holds the championship of this band in the last year of the
    /// current window (`window_end`).
    pub is_reigning_champion: bool,
    pub points: Vec<GlobalTeamHistoryPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTeamHistoryPoint {
    pub year: i32,
    pub slot: String,
    pub position: i32,
    pub points: i32,
    pub wins: i32,
    pub titles: i32,
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
    pub ano_inicio: i32,
    pub ano_fim: i32,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverTechnicalReadBlock {
    pub itens: Vec<DriverTechnicalReadItem>,
}

/// Bloco de ESTRELATO — a segunda moeda ("Nasce um astro") na ficha do piloto.
/// `fama` é o estoque público (atributo `midia`, 0–100), `carisma` o traço estável
/// (0–100) que modula quão rápido a fama sobe/desce. Os níveis são rótulos legíveis
/// e `tom` mapeia para as cores de tom já usadas no dossiê (neutral/info/success/elite…).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverStardomBlock {
    pub fama: u8,
    pub carisma: u8,
    pub nivel_fama: String,
    pub tom_fama: String,
    pub nivel_carisma: String,
    pub tom_carisma: String,
    /// Leitura de uma linha combinando fama × carisma (a dinâmica, não os números).
    pub resumo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverTechnicalReadItem {
    pub chave: String,
    pub label: String,
    pub nivel: String,
    pub tom: String,
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
    pub ultimas_10: Vec<FormResultEntry>,
    pub ultimas_5: Vec<FormResultEntry>,
    pub media_chegada: Option<f64>,
    pub tendencia: String,
    pub momento: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contexto: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCurrentSummaryBlock {
    pub veredito: String,
    pub tom: String,
    pub posicao_campeonato: Option<i32>,
    pub pontos: i32,
    pub vitorias: i32,
    pub podios: i32,
    pub top_10: Option<i32>,
    pub media_recente: Option<f64>,
    pub tendencia: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverPerformanceReadBlock {
    pub esperado_posicao: Option<i32>,
    pub entregue_posicao: Option<i32>,
    pub delta_posicao: Option<i32>,
    pub car_performance: Option<f64>,
    pub companheiro_nome: Option<String>,
    pub companheiro_pontos: Option<i32>,
    pub piloto_pontos: i32,
    pub leitura: String,
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
    #[serde(default)]
    pub categorias_timeline: Vec<DriverCareerCategoryStint>,
    pub temporadas_na_categoria: i32,
    pub corridas_na_categoria: i32,
    pub titulos: i32,
    pub foi_campeao: bool,
    #[serde(default)]
    pub historico: DriverCareerHistoryBlock,
    pub marcos: Vec<CareerMilestone>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerCategoryStint {
    pub categoria: String,
    pub ano_inicio: i32,
    pub ano_fim: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerHistoryBlock {
    pub presenca: DriverCareerPresenceBlock,
    pub primeiros_marcos: DriverCareerFirstMarksBlock,
    pub auge: DriverCareerPeakBlock,
    pub mobilidade: DriverCareerMobilityBlock,
    pub lesoes: DriverCareerInjuryBlock,
    pub eventos_especiais: DriverCareerSpecialEventsBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerPresenceBlock {
    pub tempo_carreira: i32,
    pub temporadas_disputadas: i32,
    pub anos_desempregado: i32,
    pub periodos_desempregado: Vec<String>,
    pub corridas: i32,
    pub categorias_disputadas: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerFirstMarksBlock {
    pub primeiro_podio_corrida: Option<i32>,
    pub primeira_vitoria_corrida: Option<i32>,
    pub primeiro_dnf_corrida: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerPeakBlock {
    pub melhor_temporada: Option<DriverBestSeasonBlock>,
    pub maior_sequencia_vitorias: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverBestSeasonBlock {
    pub ano: i32,
    pub categoria: String,
    pub posicao_campeonato: Option<i32>,
    pub pontos: i32,
    pub vitorias: i32,
    pub podios: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerMobilityBlock {
    pub promocoes: i32,
    pub rebaixamentos: i32,
    pub equipes_defendidas: i32,
    pub tempo_medio_por_equipe: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerInjuryBlock {
    pub leves: i32,
    pub moderadas: i32,
    pub graves: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerSpecialEventsBlock {
    pub participacoes: i32,
    pub convocacoes: i32,
    pub vitorias: i32,
    pub podios: i32,
    #[serde(default)]
    pub rankings: DriverSpecialEventRankBlock,
    pub melhor_campanha: Option<DriverSpecialCampaignBlock>,
    pub ultimo_evento: Option<DriverSpecialEventEntry>,
    pub timeline: Vec<DriverSpecialEventEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverSpecialEventRankBlock {
    pub participacoes: Option<i32>,
    pub convocacoes: Option<i32>,
    pub vitorias: Option<i32>,
    pub podios: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverSpecialCampaignBlock {
    pub ano: i32,
    pub categoria: String,
    pub classe: Option<String>,
    pub equipe: Option<String>,
    pub pontos: i32,
    pub vitorias: i32,
    pub podios: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverSpecialEventEntry {
    pub ano: i32,
    pub categoria: String,
    pub classe: Option<String>,
    pub equipe: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerRankBlock {
    pub corridas: Option<i32>,
    pub vitorias: Option<i32>,
    pub podios: Option<i32>,
    pub titulos: Option<i32>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverRivalsBlock {
    pub itens: Vec<DriverRivalInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverRivalInfo {
    pub driver_id: String,
    pub nome: String,
    pub tipo: String,
    pub intensidade: u8,
    pub intensidade_historica: u8,
    pub atividade_recente: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverReputationBlock {
    pub popularidade: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverHealthBlock {
    pub saude_geral: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lesao_ativa: Option<DriverActiveInjuryBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverActiveInjuryBlock {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nome: Option<String>,
    pub tipo: String,
    pub corrida_ocorrida_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrida_ocorrida_rotulo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrida_ocorrida_rodada: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrida_ocorrida_pista: Option<String>,
    pub corridas_total: i32,
    pub corridas_restantes: i32,
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
    /// Nível do Carro (1–10) — Sistema de Nível do Carro (a leitura de carro do jogador).
    #[serde(default)]
    pub car_level: u8,
    #[serde(default)]
    pub founded_year: i32,
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
    /// Históricos acumulados de carreira da equipe (reais, do modelo Team). Alimentam o
    /// ranking e o fallback do drawer sem estimativas fabricadas no front.
    #[serde(default)]
    pub historico_vitorias: i32,
    #[serde(default)]
    pub historico_podios: i32,
    #[serde(default)]
    pub historico_titulos_construtores: i32,
}

/// Resposta do comando `get_team_finance_report`: a divisão financeira REAL da equipe,
/// lida da tabela `team_finance_history`. Números crus (o front cuida de rótulos, cores
/// e formatação). Substitui os valores fabricados na aba My Team.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamFinanceReport {
    /// Total de rodadas com histórico registrado (0 = save antigo / sem corridas ainda).
    pub rounds_recorded: i32,
    /// Última rodada com histórico (para os ledgers de entradas/saídas).
    pub latest: Option<TeamFinanceRound>,
    /// Acumulado da temporada corrente (para a rosca de custos e a leitura de receita).
    /// Aqui `round` carrega a CONTAGEM de rodadas somadas na temporada.
    pub season: Option<TeamFinanceRound>,
    /// Caixa ao fim de cada rodada recente, em ordem cronológica (para o gráfico de caixa).
    pub cash_timeline: Vec<TeamFinanceCashPoint>,
    /// Prêmio de construtores ESTIMADO se a temporada terminasse agora (pela posição atual
    /// no campeonato). Só projeção de exibição — NÃO entra no caixa nem nas decisões da IA.
    pub expected_constructor_prize: f64,
    /// Posição atual da equipe no campeonato de construtores (0 = indisponível).
    pub current_position: i32,
    /// Nº de equipes no grupo do campeonato (para contextualizar a posição/projeção).
    pub grid_size: i32,
}

/// As 9 linhas reais de receita/despesa + totais. Serve tanto para a última rodada
/// quanto para o acumulado da temporada.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamFinanceRound {
    pub season_number: i32,
    pub round: i32,
    pub sponsorship_income: f64,
    pub result_bonus: f64,
    pub partial_prize_income: f64,
    pub aid_income: f64,
    pub salary_expense: f64,
    pub event_operations_cost: f64,
    pub structural_maintenance_cost: f64,
    pub technical_investment_cost: f64,
    pub debt_service_cost: f64,
    /// Prêmio de construtores creditado (só > 0 na linha de encerramento da temporada).
    pub constructor_prize_income: f64,
    pub income_total: f64,
    pub expenses_total: f64,
    pub net: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamFinanceCashPoint {
    pub season_number: i32,
    pub round: i32,
    pub cash_balance: f64,
    pub net: f64,
    /// `true` na linha de ENCERRAMENTO (prêmio de construtores) — o front rotula/colore
    /// esse ponto de forma distinta no gráfico de caixa.
    pub is_season_close: bool,
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
    /// Eventos de propriedade/diretoria (ex.: venda por colapso). Consumido pelas
    /// abas Identidade (como "eras" do time) e Gestão (como evento financeiro).
    pub ownership_events: Vec<TeamHistoryOwnershipEvent>,
    /// Superlativos da equipe (melhor temporada, maior sequência de títulos...).
    /// Enriquecem a aba Record além dos rankings comparativos.
    pub highlights: Vec<TeamHistoryHighlight>,
    /// Marcos da história (primeira vitória, primeiro título...) com o ano.
    pub milestones: Vec<TeamHistoryMilestone>,
    /// Resultados temporada a temporada — alimenta a aba Esportivo (resultados
    /// ao longo do tempo), distinguindo-a de Categorias (movimento entre tiers).
    pub season_results: Vec<TeamHistorySeasonResult>,
    /// Resumo de movimento entre categorias (real) — alimenta a aba Categorias.
    pub movement: TeamHistoryMovement,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamHistoryMovement {
    pub promotions: i32,
    pub relegations: i32,
    pub time_by_category: String,
    pub best_category: String,
    pub hardest_category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryHighlight {
    pub label: String,
    pub value: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryMilestone {
    pub label: String,
    pub year: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistorySeasonResult {
    pub year: String,
    pub category: String,
    /// Posição final no campeonato naquela temporada ("P3", "—" se desconhecida).
    pub position: String,
    pub wins: i32,
    pub podiums: i32,
    pub points: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryOwnershipEvent {
    pub year: String,
    pub event_type: String,
    /// Título curto para a aba Identidade (ex.: "Nova diretoria").
    pub title: String,
    /// Descrição contextual.
    pub detail: String,
    /// Resumo financeiro para a aba Gestão (ex.: "Dívida de $38M zerada · aporte de $7M").
    pub financial_note: String,
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
    /// Herança por experiência real (temporadas): Novata → Tradicional.
    pub heritage: String,
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
    pub operation_health: String,
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
    /// "start" | "promotion" | "relegation" | "same" — marcador da escada.
    pub movement: String,
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
    /// Tier de prestígio (0=Rookie … 6=Endurance) da categoria onde ele corre hoje.
    /// É a chave de agrupamento da coluna (faixa de nível). `None` = rookie/sem categoria.
    pub market_tier: Option<u8>,
    /// Temporadas parado (ver `FreeAgentRaw::seasons_idle`). Usado pelo marcador "parado".
    pub seasons_idle: Option<i32>,
    /// IDs das categorias onde ele pode realmente pegar vaga (mesma regra do leilão:
    /// tier ±1 + licença exigida, com +1 de promoção liberado). Usado pelo filtro do topo.
    pub eligible_categories: Vec<String>,
}
