//! Harness de estatísticas de simulação (Monte Carlo).
//!
//! Roda N carreiras independentes, cada uma avançando M temporadas completas,
//! e agrega métricas populacionais sobre os pilotos da IA:
//!   • Lesões: % de pilotos que se machucam por temporada, por gravidade.
//!   • Evolução: % que SOBE / DESCE / ESTAGNA (delta de atributos por idade).
//!   • Aposentadorias: % por temporada, idade média, causas.
//!   • Promoções/Rebaixamentos: pilotos que sobem/descem de categoria.
//!
//! Não é um teste de invariante — é um coletor. Sempre "passa"; o valor está
//! no relatório impresso. Rode com:
//!   cargo test --release sim_stats::experimento::monte_carlo -- --nocapture --ignored
//!
//! Escala configurável por env:
//!   IRACER_MC_RUNS=10  IRACER_MC_SEASONS=10  cargo test ...

#![cfg(test)]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::commands::career::{
    advance_market_week_in_base_dir, advance_season_in_base_dir, create_career_in_base_dir,
    finalize_preseason_in_base_dir, skip_all_pending_races_in_base_dir, CreateCareerInput,
};
use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::market::preseason::PreSeasonPhase;
use crate::promotion::{MovementType, PilotEffectType};
// Cada área do harness mora no seu módulo e enxerga os imports acima via
// `use super::*`.
mod acompanhamento;
mod ciclo;
mod classificacao;
mod experimento;
mod metricas;
mod snapshots;
mod totais;

use acompanhamento::*;
use ciclo::*;
use classificacao::*;
use metricas::*;
use snapshots::*;
use totais::*;
