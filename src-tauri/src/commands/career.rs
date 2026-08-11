use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use chrono::Local;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rusqlite::{OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::calendar::{full_season::generate_full_season_calendar, CalendarEntry};
use crate::commands::career_detail::build_driver_detail_payload;
use crate::commands::career_types::{
    AcceptedSpecialOfferSummary, BriefingPhraseEntry, BriefingPhraseEntryInput,
    BriefingPhraseHistory, BriefingStorySummary, CareerData, CareerResumeContext, CareerResumeView,
    ContractWarningInfo, CreateCareerResult, DriverCareerRankEntry, DriverDetail, DriverSummary,
    NextRaceBriefingSummary, OpenSeat, PrimaryRivalSummary, RaceReading, RaceReadingCar,
    RaceReadingSafetyCar, RaceSummary, SaveInfo, SeasonMarketBoard, SeasonSummary,
    TeamCarPartLevel, TeamCarParts, TeamStanding, TeamSummary, TrackHistorySummary,
    VerifyDatabaseResponse, WeekendReading,
};
use crate::commands::race_history::{
    build_driver_histories, empty_previous_champions, ConstructorChampion, DriverRaceHistory,
    PreviousChampions, RoundResult, TrophyInfo,
};
use crate::config::app_config::{AppConfig, SaveMeta};
use crate::constants::historical_timeline::historical_team_foundation_year;
use crate::constants::{categories, scoring};
use crate::db::connection::Database;
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::injuries as injury_queries;
use crate::db::queries::market_proposals as market_proposal_queries;
use crate::db::queries::meta as meta_queries;
use crate::db::queries::news as news_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::special_team_entries as special_entry_queries;
use crate::db::queries::standings as standings_queries;
use crate::db::queries::standings::ChampionshipContext;
use crate::db::queries::team_car as team_car_queries;
use crate::db::queries::teams as team_queries;
use crate::event_interest::{
    calculate_expected_event_interest, to_summary, EventInterestContext, EventInterestSummary,
};
use crate::evolution::pipeline::{run_end_of_season, EndOfSeasonResult};
use crate::finance::planning::calculate_financial_plan;
use crate::finance::salary::{calculate_offer_salary_from_money, calculate_salary_ceiling};
use crate::generators::ids::{next_id, IdType};
use crate::generators::nationality::{format_nationality, get_nationality};
use crate::generators::world::{align_world_career_start_years, generate_world};
use crate::market::pipeline::fill_all_remaining_vacancies;
use crate::market::preseason::{
    advance_week, delete_preseason_plan, load_preseason_plan, save_preseason_plan, PendingAction,
    PlannedEvent, PreSeasonPlan, PreSeasonState, WeekResult,
};
use crate::market::proposals::{MarketProposal, ProposalStatus};
use crate::models::driver::Driver;
use crate::models::enums::{ContractStatus, DriverStatus, SeasonPhase, TeamRole};
use crate::models::license::{
    driver_has_required_license_for_division, ensure_driver_can_join_division,
    grant_driver_license_for_division_if_needed,
};
use crate::models::season::Season;
use crate::models::team::{Team, TeamHierarchyClimate};
use crate::news::{NewsImportance, NewsItem, NewsType};

pub use crate::commands::career_types::CreateCareerInput;

#[path = "career/briefing.rs"]
mod briefing;
#[path = "career/champion.rs"]
mod champion;
#[path = "career/debug.rs"]
mod debug;
// `pub(crate)` para os irmãos alcançarem `errors::*` pelo `use super::*;`.
#[path = "career/errors.rs"]
pub(crate) mod errors;
#[path = "career/interests.rs"]
mod interests;
#[path = "career/lifecycle.rs"]
mod lifecycle;
#[path = "career/market_window.rs"]
mod market_window;
#[path = "career/queries.rs"]
mod queries;
#[path = "career/save_state.rs"]
mod save_state;
#[path = "career/season_flow.rs"]
mod season_flow;
#[path = "career/standings.rs"]
mod standings;
#[path = "career/vacancies.rs"]
mod vacancies;

pub(crate) use briefing::{
    build_next_race_briefing_summary, build_primary_rival_summary, empty_next_race_briefing_summary,
};
pub(crate) use champion::get_season_champion_payload_in_base_dir;
pub(crate) use debug::{
    debug_force_player_poach_offer_in_base_dir, debug_poaching_auctions_in_base_dir,
    debug_prepare_market_scenario_in_base_dir, debug_skip_to_season_finale_in_base_dir,
    debug_stamp_player_championship_in_base_dir,
};
pub(crate) use interests::{get_player_interests_in_base_dir, select_player_interests};
pub use interests::{PlayerInterests, RivalInterest};
pub(crate) use lifecycle::{career_number_from_id, count_rows, open_career_resources};
pub(crate) use lifecycle::{
    create_career_in_base_dir, delete_career_in_base_dir, list_saves_in_base_dir,
    load_career_in_base_dir,
};
#[cfg(test)]
pub(crate) use lifecycle::{
    needs_regular_contract_repair, next_career_id, validate_create_career_input,
};
pub(super) use lifecycle::open_career_resources_read_only;
#[allow(unused_imports)]
pub(crate) use lifecycle::{test_create_driver, test_list_drivers, verify_database};
#[cfg(test)]
pub(crate) use market_window::is_team_role_vacant;
pub(crate) use market_window::{
    advance_market_week_in_base_dir, finalize_preseason_in_base_dir,
    get_player_poach_offer_in_base_dir, get_player_proposals_in_base_dir,
    get_preseason_free_agents_in_base_dir, get_preseason_state_in_base_dir,
    resolve_player_poach_offer_in_base_dir, respond_to_proposal_in_base_dir, PoachDebugReport,
};
pub use market_window::{PlayerProposalView, ProposalResponse};
#[cfg(test)]
pub(crate) use queries::consecutive_team_seasons_up_to;
pub(crate) use queries::{
    build_accepted_special_offer_summary, build_team_summary, find_player_team,
    get_driver_slot_info, team_founded_year_for_payload,
};
pub(crate) use queries::{
    calculate_consecutive_team_tenure, count_calendar_entries,
    get_calendar_for_category_in_base_dir, get_displaced_driver_context_in_base_dir,
    get_driver_detail_in_base_dir,
    get_driver_dossier_ranks_in_base_dir,
    get_drivers_by_category_in_base_dir, get_news_in_base_dir, get_player_dossier_in_base_dir,
    get_previous_champions_in_base_dir, get_race_reading_in_base_dir,
    get_race_results_by_category_in_base_dir, toggle_driver_favorite_in_base_dir,
};
pub(crate) use save_state::{
    delete_resume_context, get_briefing_phrase_history_in_base_dir,
    persist_resume_context_in_base_dir, read_resume_context, read_save_meta,
    save_briefing_phrase_history_in_base_dir, save_meta_to_info, write_resume_context,
    write_save_meta,
};
#[cfg(test)]
pub(crate) use season_flow::count_season_calendar_entries;
pub(crate) use season_flow::{advance_season_in_base_dir, skip_all_pending_races_in_base_dir};
pub(crate) use standings::{
    get_regular_standings_participant_ids, get_special_driver_standings_from_results,
    get_teams_car_parts_in_base_dir, get_teams_standings_in_base_dir,
    merge_recent_results_fallback,
};
pub(crate) use vacancies::backfill_team_vacancy;
#[cfg(test)]
pub(crate) use vacancies::calculate_offer_salary_for_team;
pub(crate) use vacancies::{
    force_place_player, generate_emergency_player_proposals, get_season_market_board_in_base_dir,
    normalize_car_performance, normalize_regular_contracts_for_team, refresh_team_hierarchy_now,
};

pub(crate) struct HistoricalSpecialStanding {
    driver_id: String,
    points: f64,
    wins: i32,
    podiums: i32,
    latest_team_id: Option<String>,
    latest_class_name: Option<String>,
}

pub(crate) struct HistoricalSpecialTeamStanding {
    team_id: String,
    points: f64,
    wins: i32,
    class_name: Option<String>,
}

fn warn_if_noncritical<T>(result: Result<T, String>, context: &str) {
    if let Err(error) = result {
        eprintln!("Aviso: {context}: {error}");
    }
}

#[cfg(test)]
#[path = "career/tests/mod.rs"]
mod tests;
