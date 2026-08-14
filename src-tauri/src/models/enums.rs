//! Enums de domínio compartilhados pelos modelos.
//!
//! Este arquivo é apenas a FACHADA: as definições vivem em `enums/`, divididas
//! por área de domínio, e são re-exportadas aqui para que todos os caminhos
//! `crate::models::enums::X` continuem válidos.
//!
//! ⚠️ A ORDEM das variantes é parte do contrato de serialização — não reordene.
//!
//! O submódulo `mercado` saiu em 12/08/2026. Ele trazia `NewsType`, `ProposalStatus` e
//! `RefusalReason`, e nenhum dos três tinha um consumidor: os dois primeiros eram cópias
//! divergentes de [`crate::news::NewsType`] e [`crate::market::proposals::ProposalStatus`]
//! (que é quem o crate inteiro importa, e o único com `from_str_strict`), e o terceiro não
//! era citado em lugar nenhum. Duas definições do mesmo enum, uma delas com menos
//! variantes, é armadilha esperando o próximo `use` errado.

mod clima;
mod contrato;
mod corrida;
mod piloto;
mod temporada;

pub use clima::*;
pub use contrato::*;
pub use corrida::*;
pub use piloto::*;
pub use temporada::*;
