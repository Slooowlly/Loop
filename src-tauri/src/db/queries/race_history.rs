#![allow(dead_code)]

//! Fachada do histórico de corridas: agrupa as queries por área (carreira,
//! classificação, recordes, pistas, rodadas, forma e dossiê do jogador) e
//! reexporta tudo, de modo que os caminhos públicos continuam idênticos.

#[path = "race_history/carreira.rs"]
mod carreira;
#[path = "race_history/classificacao.rs"]
mod classificacao;
#[path = "race_history/forma.rs"]
mod forma;
#[path = "race_history/jogador.rs"]
mod jogador;
#[path = "race_history/pistas.rs"]
mod pistas;
#[path = "race_history/recordes.rs"]
mod recordes;
#[path = "race_history/rodadas.rs"]
mod rodadas;

pub use carreira::*;
pub use classificacao::*;
pub use forma::*;
pub use jogador::*;
pub use pistas::*;
pub use recordes::*;
pub use rodadas::*;

#[cfg(test)]
#[path = "race_history/tests/mod.rs"]
mod tests;
