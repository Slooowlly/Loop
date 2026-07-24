//! Tipos da corrida simulada: o segmento, o estado vivo de cada carro e os DTOs de resultado
//! que atravessam a ponte para o React.

use serde::{Deserialize, Serialize};

use crate::simulation::incidents::{IncidentResult, PendingDamage};
use crate::simulation::qualifying::QualifyingResult;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RaceSegment {
    Start,
    Early,
    Mid,
    Late,
    Finish,
}

impl RaceSegment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "START",
            Self::Early => "EARLY",
            Self::Mid => "MID",
            Self::Late => "LATE",
            Self::Finish => "FINISH",
        }
    }

    /// Ordinal para comparação de segmento em DNF ordering (maior = mais tarde na corrida).
    pub(crate) fn ordinal(self) -> u8 {
        match self {
            Self::Start => 0,
            Self::Early => 1,
            Self::Mid => 2,
            Self::Late => 3,
            Self::Finish => 4,
        }
    }

    /// Em qual dos 5 segmentos uma volta cai. O cérebro da quebra raciocina POR VOLTA; a
    /// simulação raciocina por segmento — é aqui que os dois se encontram. Volta 1 é a
    /// largada; a última volta é o FINISH.
    pub(crate) fn from_lap(lap: u32, total_laps: i32) -> Self {
        let total = total_laps.max(1) as f64;
        let idx = (((lap.max(1) - 1) as f64 / total) * 5.0).floor() as usize;
        match idx.min(4) {
            0 => Self::Start,
            1 => Self::Early,
            2 => Self::Mid,
            3 => Self::Late,
            _ => Self::Finish,
        }
    }
}

/// Desfecho de QUEBRA DE PEÇA pré-rolado pelo cérebro (`car::breakdown`) e injetado na corrida
/// simulada — a Fase 7 do Sistema de Quebra. A simulação não sabe de peça, desgaste nem
/// economia: recebe só "este piloto ficou N segundos parado (ou abandonou) nesta volta" e
/// cobra o preço na moeda dela. Quem sabe de peça é quem rola, em `commands::race`.
#[derive(Debug, Clone)]
pub struct MechanicalOutcome {
    pub pilot_id: String,
    /// Volta em que a peça largou (1-based).
    pub lap: u32,
    /// A quebra encerrou a corrida do carro.
    pub is_dnf: bool,
    /// Segundos parados no box consertando. 0 quando `is_dnf`.
    pub penalty_secs: u32,
    /// Frase do problema ("motor fundiu por superaquecimento") — vira o `dnf_reason` no abandono.
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClassificationStatus {
    Finished,
    Dnf,
}

#[derive(Debug, Clone)]
pub struct RaceState {
    pub driver_id: String,
    pub tire_wear: f64,
    pub physical_condition: f64,
    pub cumulative_score: f64,
    pub is_dnf: bool,
    pub current_position: i32,
    pub incidents: Vec<IncidentResult>,
    pub dnf_reason: Option<String>,
    pub dnf_segment: Option<RaceSegment>,
    /// Danos latentes pós-colisão aguardando manifestação.
    pub pending_damage: Vec<PendingDamage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceDriverResult {
    pub pilot_id: String,
    pub pilot_name: String,
    pub team_id: String,
    pub team_name: String,
    pub grid_position: i32,
    pub finish_position: i32,
    pub positions_gained: i32,
    pub best_lap_time_ms: f64,
    pub total_race_time_ms: f64,
    pub gap_to_winner_ms: f64,
    pub is_dnf: bool,
    pub dnf_reason: Option<String>,
    pub dnf_segment: Option<String>,
    #[serde(default)]
    pub incidents_count: i32,
    #[serde(default)]
    pub incidents: Vec<IncidentResult>,
    pub has_fastest_lap: bool,
    pub points_earned: i32,
    pub is_jogador: bool,
    pub laps_completed: i32,
    pub final_tire_wear: f64,
    pub final_physical: f64,
    pub classification_status: ClassificationStatus,
    /// Descrição de conveniência do pior incidente (narrative_importance_hint >= 2).
    /// Campo derivado — não é fonte factual primária.
    #[serde(default)]
    pub notable_incident: Option<String>,
    /// ID da entry do catálogo do incidente que causou o DNF.
    #[serde(default)]
    pub dnf_catalog_id: Option<String>,
    /// Segmento de origem do dano (pode diferir do segmento do DNF para dano latente).
    #[serde(default)]
    pub damage_origin_segment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceResult {
    pub qualifying_results: Vec<QualifyingResult>,
    pub race_results: Vec<RaceDriverResult>,
    pub pole_sitter_id: String,
    pub winner_id: String,
    pub fastest_lap_id: String,
    pub total_laps: i32,
    pub weather: String,
    pub track_name: String,
    #[serde(default)]
    pub total_incidents: i32,
    #[serde(default)]
    pub total_dnfs: i32,
    /// Incidentes com narrative_importance_hint >= 1.
    #[serde(default)]
    pub main_incident_count: i32,
    /// Pilot IDs com incidente headline (hint >= 2).
    #[serde(default)]
    pub notable_incident_pilot_ids: Vec<String>,
    /// Piloto que mais ganhou posições.
    #[serde(default)]
    pub most_positions_gained_id: Option<String>,
    /// Segmentos da corrida NEUTRALIZADOS por bandeira amarela, derivados dos
    /// incidentes (ver `derive_caution_segments`). A amarela é REGISTRADA, não
    /// simulada: não agrupa o pelotão nem mexe em posição nenhuma.
    #[serde(default)]
    pub caution_segments: Vec<String>,
    /// Índices, no slice de [`MechanicalOutcome`] passado à simulação, das quebras que a corrida
    /// de fato COBROU. Um carro que já tinha abandonado por batida antes da volta da quebra não
    /// entra: a peça largaria num carro que não estava mais na pista. O caller persiste só isto
    /// em `race_breakdowns`, pra tela e notícia nunca falarem de quebra que não houve.
    ///
    /// TRANSITÓRIO: vale só no retorno da simulação. `#[serde(skip)]` de propósito — não é
    /// estado da corrida, não vai pro `race_screens.json` nem pro save.
    #[serde(skip)]
    pub applied_mechanicals: Vec<usize>,
}
