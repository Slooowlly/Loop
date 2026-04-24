use serde::{Deserialize, Serialize};

use crate::evolution::growth::{GrowthReport, SeasonStats};
use crate::evolution::motivation::MotivationReport;
use crate::promotion::PromotionResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndOfSeasonResult {
    pub growth_reports: Vec<GrowthReport>,
    pub motivation_reports: Vec<MotivationReport>,
    pub retirements: Vec<RetirementInfo>,
    pub rookies_generated: Vec<RookieInfo>,
    pub new_season_id: String,
    pub new_year: i32,
    pub licenses_earned: Vec<LicenseEarned>,
    pub promotion_result: PromotionResult,
    pub preseason_initialized: bool,
    pub preseason_total_weeks: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetirementInfo {
    pub driver_id: String,
    pub driver_name: String,
    pub age: i32,
    pub reason: String,
    pub categoria: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RookieInfo {
    pub driver_id: String,
    pub driver_name: String,
    pub nationality: String,
    pub age: i32,
    pub skill: u8,
    pub tipo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseEarned {
    pub driver_id: String,
    pub driver_name: String,
    pub license_level: u8,
    pub category: String,
}

#[derive(Debug, Clone)]
pub(crate) struct StandingEntry {
    pub(crate) driver_id: String,
    pub(crate) driver_name: String,
    pub(crate) category: String,
    pub(crate) team_id: Option<String>,
    pub(crate) position: i32,
    pub(crate) total_drivers: i32,
    pub(crate) seasons_in_category: i32,
    pub(crate) stats: SeasonStats,
}
