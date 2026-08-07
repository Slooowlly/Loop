use std::path::Path;
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{Local, Weekday};
use rand::Rng;
use rand::{rngs::StdRng, SeedableRng};
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::calendar::display_date_for_season_week;
use crate::constants::timeline::{MARKET_DURATION_WEEKS, MARKET_SIGNINGS_START_WEEK};
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::meta as meta_queries;
use crate::db::queries::rivalries as rivalry_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::finance::cashflow::{apply_offseason_competitiveness_impact, PENALTY_FADE_YEARS};
use crate::finance::planning::{
    category_finance_scale, category_finance_scale_for, derive_budget_index_from_money,
};
use crate::finance::state::refresh_team_financial_state;
use crate::finance::strategy::{
    advance_strategic_plan, apply_elite_resource_floor, designate_elite_teams,
};
use crate::market::pit_strategy::{
    recalculate_pit_crew_quality, recalculate_pit_strategy_risk, PreviousTeamStanding,
};
use crate::market::proposals::MarketProposal;
use crate::market::sync::sync_team_slots_from_active_regular_contracts;
use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::license::repair_missing_licenses_for_current_categories;
// Cada etapa da pré-temporada mora no seu módulo e enxerga os imports acima via
// `use super::*`.
#[cfg(test)]
mod clone_temporario;
mod estado;
mod eventos;
mod expectativa;
mod inicializacao;
mod plano;
mod semana;
mod sincronizacao;
mod tipos;

// Os tipos e as funções de entrada continuam saindo por `market::preseason::*`.
pub use inicializacao::*;
pub use plano::*;
pub use semana::*;
pub use tipos::*;

#[cfg(test)]
use clone_temporario::*;
use estado::*;
use eventos::*;
use expectativa::*;
use sincronizacao::*;

#[cfg(test)]
mod tests;
