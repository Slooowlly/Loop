#![allow(dead_code)]

//! Catálogo estático de equipes.
//!
//! Fachada do módulo: os caminhos públicos continuam sendo
//! `constants::teams::TeamTemplate` e `constants::teams::get_*`.
//! O conteúdo mora nos submódulos:
//! - [`tipos`] — a struct [`TeamTemplate`]
//! - [`dados`] — o catálogo estático `TEAMS`
//! - [`consultas`] — as funções de busca e filtro

mod consultas;
mod dados;
mod tipos;

pub use consultas::*;
pub use tipos::*;

#[cfg(test)]
mod tests;
