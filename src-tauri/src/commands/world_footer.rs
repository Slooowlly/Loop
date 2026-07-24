//! Rodapé "Do mundo do Grid" do boletim (aba Notícias / revista).
//!
//! Notinhas curtas com VOZ DE REVISTA (3ª pessoa, jornalística — nunca se dirige ao
//! jogador). O laço com o jogador é só o CRITÉRIO DE SELEÇÃO, não aparece no texto.
//!
//! Cascata de assuntos, sempre da categoria ATUAL do jogador:
//!   1. Ex-equipes e ex-companheiros DO JOGADOR (com estado digno de nota).
//!   2. Se faltar, o mesmo para o 1º e o 2º do campeonato (ex-time/ex-parceiro deles).
//!   3. Se ainda faltar, RECORDES da categoria — com ênfase nos que estão a caminho
//!      ("Fulano está a N vitórias de igualar o recorde histórico").
//!
//! Estado lido de campos REAIS de `teams`. Fonte determinística (fallback); os mesmos
//! fatos podem virar IA via `/world-notes` (contrato em `docs/world-notes-endpoint.md`).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::Manager;

use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::models::enums::{ContractStatus, DriverStatus};

#[path = "world_footer/astro.rs"]
mod astro;
#[path = "world_footer/coleta.rs"]
mod coleta;
#[path = "world_footer/comandos.rs"]
mod comandos;
#[path = "world_footer/equipes.rs"]
mod equipes;
#[path = "world_footer/recordes.rs"]
mod recordes;
#[path = "world_footer/rotulos.rs"]
mod rotulos;
#[path = "world_footer/tipos.rs"]
mod tipos;

// Comandos e DTOs seguem públicos nos MESMOS caminhos de antes; o resto é interno.
pub use comandos::*;
pub use tipos::*;
use astro::*;
use coleta::*;
use equipes::*;
use recordes::*;
use rotulos::*;

/// Quantas notas tentar reunir antes de recorrer aos recordes, e o teto duro.
const TARGET_NOTES: usize = 4;
const MAX_NOTES: usize = 5;
/// Distância máxima (em vitórias/pódios/largadas) para um recorde contar como "a caminho".
const RECORD_GAP_MAX: i32 = 3;

#[cfg(test)]
#[path = "world_footer/tests/mod.rs"]
mod ai_tests;
