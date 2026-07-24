#![allow(dead_code)]

//! Rivalidades entre pilotos — índice do módulo.
//!
//! A lógica vive nos submódulos irmãos:
//! - `intensidade`: níveis semânticos e cruzamento de thresholds
//! - `eventos`: upsert com dois eixos e leitura por piloto
//! - `noticias`: notícia de escalada (montagem e persistência)
//! - `gatilhos`: hierarquia, campeonato e colisões
//! - `decaimento`: decaimento de fim de temporada
//! - `team`: o gêmeo de rivalidade entre equipes

pub mod team;

mod decaimento;
mod eventos;
mod gatilhos;
mod intensidade;
mod noticias;

pub use decaimento::*;
pub use eventos::*;
pub use gatilhos::*;
pub use intensidade::*;
pub use noticias::*;

// Reexportado só para a suíte `tests`, que enxerga este módulo via `use super::*`
// e depende desse nome desde quando a lógica morava aqui.
#[cfg(test)]
use crate::models::rivalry::RivalryType;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
