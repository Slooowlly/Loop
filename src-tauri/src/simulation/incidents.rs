#![allow(dead_code)]

//! Incidentes de corrida — fachada do módulo.
//!
//! O corpo vive em `incidents/`: os tipos do incidente, as tabelas de risco, os sorteios e a
//! passagem do segmento. Aqui só ficam as declarações e os re-exports, para que todos os
//! caminhos públicos continuem sendo `crate::simulation::incidents::*`.

#[path = "incidents/risco.rs"]
mod risco;
#[path = "incidents/segmento.rs"]
mod segmento;
#[path = "incidents/sorteios.rs"]
mod sorteios;
#[path = "incidents/tipos.rs"]
mod tipos;

pub(crate) use risco::*;
pub use segmento::*;
pub(crate) use sorteios::*;
pub use tipos::*;

#[cfg(test)]
#[path = "incidents/tests/mod.rs"]
mod tests;
