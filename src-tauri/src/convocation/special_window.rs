use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};

use crate::commands::career_types::{
    SpecialWindowCategorySection, SpecialWindowEligibleCandidate, SpecialWindowLogEntry,
    SpecialWindowPayload, SpecialWindowPlayerOffer, SpecialWindowTeamSummary,
};
use crate::common::time::current_timestamp;
use crate::constants::categories::get_category_config;
use crate::convocation::eligibility::coletar_candidatos;
use crate::convocation::pipeline::GridClasse;
use crate::convocation::scoring::calcular_score;
use crate::db::connection::DbError;
use crate::db::queries::{
    contracts as contract_queries, drivers as driver_queries,
    special_team_entries as special_entry_queries, teams as team_queries,
};
use crate::models::driver::Driver;
use crate::models::enums::TeamRole;

pub const TOTAL_SPECIAL_WINDOW_DAYS: i32 = 7;

struct ClassConfig {
    special_category: &'static str,
    class_name: &'static str,
    feeder_category: &'static str,
}

const CLASSES_CONVOCADAS: &[ClassConfig] = &[
    ClassConfig {
        special_category: "production_challenger",
        class_name: "mazda",
        feeder_category: "mazda_amador",
    },
    ClassConfig {
        special_category: "production_challenger",
        class_name: "toyota",
        feeder_category: "toyota_amador",
    },
    ClassConfig {
        special_category: "production_challenger",
        class_name: "bmw",
        feeder_category: "bmw_m2",
    },
    ClassConfig {
        special_category: "endurance",
        class_name: "gt4",
        feeder_category: "gt4",
    },
    ClassConfig {
        special_category: "endurance",
        class_name: "gt3",
        feeder_category: "gt3",
    },
    ClassConfig {
        special_category: "endurance",
        class_name: "lmp2",
        feeder_category: "endurance",
    },
];

fn uses_regular_special_event_grid(category: &str) -> bool {
    matches!(category, "production_challenger" | "endurance")
}

fn legacy_window_classes() -> impl Iterator<Item = &'static ClassConfig> {
    CLASSES_CONVOCADAS
        .iter()
        .filter(|cfg| !uses_regular_special_event_grid(cfg.special_category))
}

fn has_legacy_window_classes() -> bool {
    legacy_window_classes().next().is_some()
}

#[derive(Debug, Clone)]
struct WindowStateRow {
    current_day: i32,
    total_days: i32,
    status: String,
    active_offer_id: Option<String>,
    player_result: Option<String>,
}

#[derive(Debug, Clone)]
struct CandidateAccumulator {
    driver_name: String,
    origin_category: String,
    license_level: Option<u8>,
    desirability: i32,
    production_eligible: bool,
    endurance_eligible: bool,
}

#[derive(Debug, Clone)]
struct VisibleAssignment {
    team_id: String,
    driver_id: String,
    papel: TeamRole,
    new_badge_day: Option<i32>,
}

#[derive(Debug, Clone)]
struct RankedEligibleCandidate {
    candidate: SpecialWindowEligibleCandidate,
    championship_position: Option<i32>,
    championship_total: Option<i32>,
}

const VISIBLE_PRODUCTION_ORIGINS: &[&str] = &["mazda_amador", "toyota_amador", "bmw_m2"];
const VISIBLE_ENDURANCE_ORIGINS: &[&str] = &["gt4", "gt3", "endurance"];
const VISIBLE_SHORTLIST_LIMIT_PER_ORIGIN: usize = 12;

// Etapas da janela especial. Este arquivo guarda os imports, tipos e constantes
// compartilhados; cada etapa mora no seu módulo e enxerga tudo acima via
// `use super::*`.
mod comum;
mod janela;
mod leitura;
mod revelacao;
mod semeadura;

// `janela` expõe a API pública (`convocation::special_window::…`); os demais só
// re-entram no namespace da fachada para que os módulos-irmãos se enxerguem.
use comum::*;
pub use janela::*;
use leitura::*;
use revelacao::*;
use semeadura::*;
