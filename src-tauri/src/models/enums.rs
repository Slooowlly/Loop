//! Enums de domínio compartilhados pelos modelos.
//!
//! Este arquivo é apenas a FACHADA: as definições vivem em `enums/`, divididas
//! por área de domínio, e são re-exportadas aqui para que todos os caminhos
//! `crate::models::enums::X` continuem válidos.
//!
//! ⚠️ A ORDEM das variantes é parte do contrato de serialização — não reordene.

#![allow(dead_code)]

mod clima;
mod contrato;
mod corrida;
mod mercado;
mod piloto;
mod temporada;

pub use clima::*;
pub use contrato::*;
pub use corrida::*;
// Enums de mercado/imprensa ainda não consumidos pelo crate (mesmo motivo do
// `allow(dead_code)` acima) — o re-export existe para manter o caminho público.
#[allow(unused_imports)]
pub use mercado::*;
pub use piloto::*;
pub use temporada::*;
