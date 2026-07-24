//! Tipos compartilhados do ranking global de pilotos.

use super::*;

#[derive(Debug, Clone, Default)]
pub(super) struct CategoryStats {
    pub(super) category: String,
    pub(super) class_name: Option<String>,
    pub(super) points: f64,
    pub(super) wins: i32,
    pub(super) podiums: i32,
    pub(super) poles: i32,
    pub(super) races: i32,
    pub(super) titles: i32,
    pub(super) title_years: Vec<TitleYear>,
    pub(super) dnfs: i32,
}

/// Um ano de título e a equipe (por `team_id`) com a qual foi conquistado.
#[derive(Debug, Clone)]
pub(super) struct TitleYear {
    pub(super) year: i32,
    pub(super) team_id: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct RetiredDriverSnapshot {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) retirement_season: String,
    pub(super) category: String,
    pub(super) stats: CategoryStats,
    pub(super) title_categories: Vec<GlobalDriverTitleCategorySummary>,
    pub(super) career_start_year: Option<i32>,
    pub(super) career_years: Option<i32>,
}

#[derive(Debug, Clone)]
pub(super) struct RankingEntry {
    pub(super) row: GlobalDriverRankingRow,
    pub(super) stats_by_category: Vec<CategoryStats>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct RaceContribution {
    pub(super) category: String,
    pub(super) points: f64,
    pub(super) wins: i32,
    pub(super) podiums: i32,
    pub(super) poles: i32,
    pub(super) races: i32,
    pub(super) dnfs: i32,
}

pub(super) type TitleEventKey = (i32, String, Option<String>);
pub(super) type TeamTitleStatsByDriver = HashMap<String, Vec<(TitleEventKey, CategoryStats)>>;
/// `team_id` -> (nome atual da equipe, cor primária), usado para resolver a logo
/// da equipe campeã por ano de título.
pub(super) type TeamLookup = HashMap<String, (String, String)>;

#[derive(Debug, Clone)]
pub(super) struct TeamTitleDriverScore {
    pub(super) driver_id: String,
    pub(super) points: f64,
    pub(super) wins: i32,
    pub(super) podiums: i32,
    pub(super) best_finish: i32,
    pub(super) races: i32,
}

#[derive(Debug, Clone)]
pub(super) struct SpecialTeamTitleCandidate {
    pub(super) event_key: TitleEventKey,
    pub(super) season_number: i32,
    pub(super) year: i32,
    pub(super) category: String,
    pub(super) class_name: Option<String>,
    pub(super) team_id: String,
    pub(super) points: f64,
    pub(super) wins: i32,
    pub(super) podiums: i32,
    pub(super) poles: i32,
    pub(super) races: i32,
}
