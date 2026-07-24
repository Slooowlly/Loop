#![allow(dead_code)]

//! Fachada das queries de equipe. A implementação vive nos submódulos de
//! `teams/`, fatiados por operação; todos os caminhos públicos continuam sendo
//! `crate::db::queries::teams::*`.

mod escrita;
mod financas;
mod gestao;
mod leitura;
mod lineup;
mod mapeamento;
mod recordes;

pub use escrita::*;
pub use financas::*;
pub use gestao::*;
pub use leitura::*;
pub use lineup::*;
pub use recordes::*;

#[cfg(test)]
mod tests;
