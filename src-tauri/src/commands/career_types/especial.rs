//! DTOs da janela do bloco especial (convocações fora do calendário regular).

use serde::{Deserialize, Serialize};

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
