//! DTOs da próxima corrida e do briefing de fim de semana.

use serde::{Deserialize, Serialize};

use crate::event_interest::EventInterestSummary;

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
    /// Cota do público/bilheteria que a equipe do JOGADOR captura neste evento (Fase 3
    /// do Estrelato): piso + prêmio de fama do lineup, ∈ [0,1]. `None` quando o jogador
    /// não tem equipe. Alimenta a linha "sua estrela puxa ~Y% do público" na Sala de
    /// Estratégia.
    pub public_fame_share: Option<f64>,
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
