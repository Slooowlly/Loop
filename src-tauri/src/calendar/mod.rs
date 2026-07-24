//! Calendário da temporada — índice do módulo.
//!
//! A lógica vive nos submódulos irmãos:
//! - `entry`: a entidade `CalendarEntry`
//! - `janela`: janelas de temporada, semanas e datas visuais
//! - `selecao`: escolha e ordenação das pistas (pool temático, fixas, conflitos)
//! - `montagem`: montagem de cada etapa (horário, clima, duração, voltas)
//! - `geracao`: geração dos calendários por categoria e por fase

pub(crate) mod full_season;
mod generator;

mod entry;
mod geracao;
mod janela;
mod montagem;
mod selecao;

// Reexportados só para a suíte `tests`, que enxerga este módulo via `use super::*`
// e depende desses nomes desde quando a lógica morava aqui.
#[cfg(test)]
use chrono::{NaiveDate, Weekday};
#[cfg(test)]
use std::collections::HashSet;

#[cfg(test)]
use crate::models::enums::{SeasonPhase, WeatherCondition};

pub use entry::*;
pub use geracao::*;
pub use janela::*;
pub use montagem::*;
pub use selecao::*;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
