//! DTOs do ciclo de vida da carreira: criação, draft histórico, save e retomada.

use serde::{Deserialize, Serialize};

use crate::evolution::pipeline::EndOfSeasonResult;

use super::corrida::{NextRaceBriefingSummary, RaceSummary};
use super::equipe::TeamSummary;
use super::especial::AcceptedSpecialOfferSummary;
use super::piloto::DriverSummary;

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

/// Troca a identidade pendente de um draft já simulado. Nome, nacionalidade e
/// idade do jogador não entram na geração do mundo histórico (só a dificuldade
/// entra, moldando os atributos da IA), então mudá-los não exige regerar nada:
/// basta reescrever o meta.json que a finalização vai ler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDraftIdentityInput {
    pub career_id: String,
    pub player_name: String,
    pub player_nationality: String,
    pub player_age: Option<i32>,
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
    /// Identidade pendente do piloto, gravada no meta.json na criação do draft.
    /// Volta para a tela de nova carreira reidratar o formulário ao retomar um
    /// draft: sem isto o wizard reabria com o nome em branco, o jogador digitava
    /// o nome de novo e a mudança de identidade descartava o mundo já simulado.
    #[serde(default)]
    pub player_name: Option<String>,
    #[serde(default)]
    pub player_nationality: Option<String>,
    #[serde(default)]
    pub player_age: Option<i32>,
    #[serde(default)]
    pub difficulty: Option<String>,
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
pub struct SeasonSummary {
    pub id: String,
    pub numero: i32,
    pub ano: i32,
    pub rodada_atual: i32,
    pub total_rodadas: i32,
    pub status: String,
    pub fase: String,
}
