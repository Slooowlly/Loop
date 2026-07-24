#![allow(dead_code)]
//! Motor de rivalidade entre EQUIPES (Fase 1: fundação).
//!
//! Gêmeo enxuto de [`crate::rivalry`] (piloto↔piloto) para o par de TIMES. Reusa o núcleo
//! puro de `models::rivalry` (`perceived_intensity`, `rivalry_lifecycle`, `normalize_pair`)
//! — que é agnóstico de piloto — e a camada de persistência `db::queries::team_rivalries`.
//!
//! Esta fase entrega SÓ o mecanismo: aplicar um evento (upsert nos dois eixos), ler por
//! time e decair no fim da temporada. As FONTES que geram os eventos (briga de
//! construtores, roubo de talento, guerra na pista, transbordamento de piloto) e as
//! CONSEQUÊNCIAS (manchete, moral de derby) entram nas fases seguintes.
//!
//! Ver `docs/superpowers/specs/2026-07-19-team-rivalry-design.md`.
//!
//! Fachada: o código vive nos submódulos de `team/`.
//! - [`motor`] — evento, upsert nos dois eixos e clamp da escala
//! - [`leitura`] — resumo das rivalidades de um time
//! - [`decaimento`] — decaimento anual de fim de temporada
//! - [`campeonato`] / [`mercado`] / [`pista`] / [`herdada`] — as quatro fontes
//! - [`derby`] — pulso de moral de derby (per-race)
//! - [`noticias`] — manchete ao cruzar threshold de percebida

mod campeonato;
mod decaimento;
mod derby;
mod herdada;
mod leitura;
mod mercado;
mod motor;
mod noticias;
mod pista;

pub use campeonato::*;
pub use decaimento::*;
pub use derby::*;
pub use herdada::*;
pub use leitura::*;
pub use mercado::*;
pub use motor::*;
pub use pista::*;

#[cfg(test)]
mod tests;
