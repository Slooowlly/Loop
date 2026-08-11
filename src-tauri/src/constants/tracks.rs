#![allow(dead_code)]

//! Catálogo estático de pistas.
//!
//! Fachada do módulo: os caminhos públicos continuam sendo
//! `constants::tracks::TrackInfo` e `constants::tracks::get_*`.
//! O conteúdo mora nos submódulos:
//! - [`tipos`] — as structs [`TrackInfo`] / [`TrackDefinition`]
//! - [`dados`] — o catálogo estático `TRACKS`
//! - [`consultas`] — as funções de busca, filtro e duração de quali

mod consultas;
mod dados;
mod tipos;

pub use consultas::*;
pub use dados::LIT_TRACK_ID;
pub use tipos::*;

#[cfg(test)]
mod tests;
