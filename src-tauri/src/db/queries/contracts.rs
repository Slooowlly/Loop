#![allow(dead_code)]

//! Fachada das queries de contratos.
//!
//! A implementação vive nos submódulos de `contracts/`; aqui só reexportamos
//! para que todos os caminhos públicos continuem idênticos.

mod agentes_livres;
mod escrita;
mod especial;
mod leitura;
mod mapeamento;

pub use agentes_livres::*;
pub use escrita::*;
pub use especial::*;
pub use leitura::*;

#[cfg(test)]
mod tests;
