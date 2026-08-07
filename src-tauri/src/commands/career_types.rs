//! DTOs serde que cruzam a fronteira Rust ↔ React.
//!
//! Este arquivo é apenas a FACHADA: as definições vivem em `career_types/`,
//! divididas por área de domínio, e são re-exportadas aqui para que todos os
//! caminhos `crate::commands::career_types::X` continuem válidos.

mod campeao;
mod carreira;
mod corrida;
mod equipe;
mod especial;
mod fatura;
mod mercado;
mod piloto;
mod ranking;

pub use campeao::*;
pub use carreira::*;
pub use corrida::*;
pub use equipe::*;
pub use especial::*;
pub use fatura::*;
pub use mercado::*;
pub use piloto::*;
pub use ranking::*;
