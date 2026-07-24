#![allow(dead_code)]

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};

use crate::calendar::{
    calendar_week_for_round, display_date_for_category_round, display_date_for_season_week,
    CalendarEntry,
};
use crate::db::connection::DbError;
use crate::models::enums::{RaceStatus, SeasonPhase, ThematicSlot, WeatherCondition};
use crate::models::temporal::SeasonTemporalSummary;

#[path = "calendar/consultas.rs"]
mod consultas;
#[path = "calendar/escrita.rs"]
mod escrita;
#[path = "calendar/mapeamento.rs"]
mod mapeamento;
#[path = "calendar/temporal.rs"]
mod temporal;

pub use consultas::*;
pub use escrita::*;
// Helpers de linha/semana: internos ao crate, nunca fizeram parte da API publica.
pub(crate) use mapeamento::*;
pub use temporal::*;

#[cfg(test)]
#[path = "calendar/tests/mod.rs"]
mod tests;
