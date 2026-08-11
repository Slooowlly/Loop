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
//! ## Antes de tirar conclusão de calibração daqui, leia isto
//!
//! **O agregado deste harness não é uma régua confiável sozinho.** Duas execuções idênticas
//! variam entre si em torno de 8–10%: comparar a média de "antes" com a de "depois" de uma
//! mudança de constante não separa o efeito da mudança do sorteio da rodada. Já produziu
//! conclusão errada com cara de medida.
//!
//! O que serve de régua é a seção **TENDÊNCIA POR TEMPORADA**
//! (`experimento::tendencia`), que quebra as mesmas taxas pelo índice da temporada dentro da
//! run e imprime, em cada ponto, a média entre runs COM o desvio entre runs ao lado. Duas
//! coisas saem dali que o agregado não dá:
//!
//! - **A inclinação.** Um mundo que lesiona 1,2% na 1ª temporada e 2,8% na 8ª tem a mesma média
//!   de um que lesiona 2,0% sempre, e os dois são mundos opostos. Só a série mostra.
//! - **O chão de ruído, medido no próprio ponto.** A tabela marca como tendência apenas o que
//!   anda mais que o dobro do desvio entre runs; o resto sai rotulado "dentro do ruído", que é
//!   a resposta honesta e a que impede o achado inventado.
//!
//! Regra prática: **conclusão de calibração sai da tabela de tendência, não da média.**
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
