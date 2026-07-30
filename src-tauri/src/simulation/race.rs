//! Corrida simulada — fachada do módulo.
//!
//! O corpo vive em `race/`: os tipos que atravessam a ponte, a pontuação por segmento, o dano
//! latente pós-colisão, o fechamento do resultado e o laço que amarra tudo. Aqui só ficam as
//! declarações e os re-exports, para que todos os caminhos públicos continuem sendo
//! `crate::simulation::race::*`.

#[path = "race/danos.rs"]
mod danos;
#[path = "race/estrategia.rs"]
pub mod estrategia;
#[path = "race/motor.rs"]
mod motor;
#[path = "race/pontuacao.rs"]
mod pontuacao;
#[path = "race/resultados.rs"]
mod resultados;
#[path = "race/tipos.rs"]
mod tipos;
#[path = "race/trafego.rs"]
pub mod trafego;

pub(crate) use danos::*;
pub use estrategia::*;
pub use motor::*;
pub(crate) use pontuacao::*;
pub use resultados::*;
pub use tipos::*;
pub use trafego::*;

#[cfg(test)]
#[path = "race/tests/mod.rs"]
mod tests;
