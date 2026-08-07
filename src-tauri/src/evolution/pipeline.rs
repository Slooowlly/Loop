use std::collections::{HashMap, HashSet};
use std::path::Path;

use rand::{rngs::StdRng, Rng, SeedableRng};
use rusqlite::Connection;

use crate::constants::categories::{
    get_category_config, get_feeder_categories, runs_in_special_phase,
};
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::evolution::context::StandingEntry;
use crate::evolution::decline::apply_age_decline;
use crate::evolution::growth::{calculate_growth, GrowthReport};
use crate::evolution::licenses::persist_licenses;
use crate::evolution::motivation::{
    adjust_end_of_season_motivation, adjust_offseason_motivation, MotivationContext,
    MotivationReport, OffseasonContext,
};
use crate::evolution::retirement::{
    check_retirement, idle_orphan_retirement_chance, process_retirement,
};
use crate::evolution::season_transition::{
    archive_driver_season, create_next_season_9d, reset_driver_season_stats,
    reset_team_season_stats, update_meta_for_new_season,
};
use crate::evolution::standings::build_and_persist_standings;
use crate::finance::prize::constructor_prize;
use crate::finance::rescue::apply_team_sale;
use crate::finance::state::refresh_team_financial_state;
use crate::generators::ids::{next_id, IdType};
use crate::market::preseason::{advance_week, initialize_preseason, save_preseason_plan};
use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::enums::{ContractStatus, DriverStatus, SeasonPhase};
use crate::models::license::required_license_for_division;
use crate::models::season::Season;
use crate::models::team::Team;
use crate::promotion::pipeline::run_promotion_relegation_for_year;
use crate::rivalry::apply_season_end_rivalry_decay;
use crate::world::team_archive::archive_team_season;

// Reexports para compatibilidade — callsites externos usam crate::evolution::pipeline::*
pub use crate::evolution::context::{EndOfSeasonResult, RetirementInfo, RookieInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndOfSeasonMode {
    Playable,
    HistoricalDraft,
}

// Etapas da virada de temporada. Este arquivo guarda só os imports compartilhados
// e o modo de execução; cada etapa mora no seu módulo e enxerga os imports acima
// via `use super::*`.
mod contexto;
mod financas;
mod orquestracao;
mod pilotos;
mod transicao;

// O glob é `pub` onde há caminho público a preservar: `evolution::pipeline::…`
// continua resolvendo igual.
use contexto::*;
use financas::*;
pub use orquestracao::*;
pub(crate) use pilotos::*;
use transicao::*;

#[cfg(test)]
#[path = "pipeline/tests/mod.rs"]
mod tests;
