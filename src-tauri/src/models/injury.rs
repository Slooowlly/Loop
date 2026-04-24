use crate::models::enums::InjuryType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Injury {
    pub id: String,
    pub pilot_id: String,
    pub injury_type: InjuryType,
    pub modifier: f64,
    pub races_total: i32,
    pub races_remaining: i32,
    pub skill_penalty: f64,
    pub season: i32,
    pub race_occurred: String,
    pub active: bool,
}
