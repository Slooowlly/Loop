use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SimultaneousResults {
    pub categories_simulated: Vec<CategorySimResult>,
    pub total_races_simulated: i32,
    pub highlights: Vec<SimHighlight>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CategorySimResult {
    pub category_id: String,
    pub category_name: String,
    pub races_simulated: i32,
    pub results: Vec<BriefRaceResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BriefRaceResult {
    pub race_id: String,
    pub track_name: String,
    pub winner_name: String,
    pub winner_team: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimHighlight {
    pub headline: String,
    pub category: String,
}
