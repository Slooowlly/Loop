//! DTOs serde que cruzam a fronteira Rust ↔ React.
//!
//! Este arquivo é apenas a FACHADA: as definições vivem em `career_types/`,
//! divididas por área de domínio, e são re-exportadas aqui para que todos os
//! caminhos `crate::commands::career_types::X` continuem válidos.

mod carreira;
mod corrida;
mod equipe;
mod especial;
mod mercado;
mod piloto;
mod ranking;

pub use carreira::*;
pub use corrida::*;
pub use equipe::*;
pub use especial::*;
pub use mercado::*;
pub use piloto::*;
pub use ranking::*;
