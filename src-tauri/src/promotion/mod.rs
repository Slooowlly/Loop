use serde::{Deserialize, Serialize};

pub mod block1;
pub mod block2;
pub mod block3;
pub mod effects;
pub mod pilots;
pub mod pipeline;
pub mod standings;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionResult {
    pub movements: Vec<TeamMovement>,
    pub pilot_effects: Vec<PilotEffect>,
    pub attribute_deltas: Vec<TeamAttributeDelta>,
    pub errors: Vec<String>,
}

impl PromotionResult {
    pub fn empty() -> Self {
        Self {
            movements: Vec::new(),
            pilot_effects: Vec::new(),
            attribute_deltas: Vec::new(),
            errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMovement {
    pub team_id: String,
    pub team_name: String,
    pub from_category: String,
    pub to_category: String,
    pub movement_type: MovementType,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MovementType {
    Promocao,
    Rebaixamento,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PilotEffect {
    pub driver_id: String,
    pub driver_name: String,
    pub team_id: String,
    pub effect: PilotEffectType,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PilotEffectType {
    MovesWithTeam,
    FreedNoLicense,
    FreedPlayerStays,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamAttributeDelta {
    pub team_id: String,
    pub team_name: String,
    pub movement_type: MovementType,
    pub car_performance_delta: f64,
    pub budget_delta: f64,
    pub facilities_delta: f64,
    pub engineering_delta: f64,
    pub morale_multiplier: f64,
    pub reputacao_delta: f64,
}
