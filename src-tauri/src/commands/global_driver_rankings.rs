//! Ranking global de pilotos: fachada do módulo. Os `use` daqui são o vocabulário
//! compartilhado — cada submódulo puxa tudo com `use super::*;`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use crate::commands::career_types::{
    DriverWorldRank, GlobalDriverRankingLeaders, GlobalDriverRankingPayload,
    GlobalDriverRankingRow, GlobalDriverTitleCategorySummary, GlobalDriverTitleYearTeam,
};
use crate::config::app_config::AppConfig;
use crate::constants::categories::{
    competitive_division_key, get_feeder_categories, is_especial, is_valid_competitive_division,
};
use crate::db::connection::Database;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::injuries as injury_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::models::driver::Driver;
use crate::models::enums::DriverStatus;

#[path = "global_driver_rankings/aposentados.rs"]
mod aposentados;
#[path = "global_driver_rankings/arquivo.rs"]
mod arquivo;
#[path = "global_driver_rankings/carreira_real.rs"]
mod carreira_real;
#[path = "global_driver_rankings/categorias.rs"]
mod categorias;
#[path = "global_driver_rankings/classes.rs"]
mod classes;
#[path = "global_driver_rankings/classificacao.rs"]
mod classificacao;
#[path = "global_driver_rankings/entradas.rs"]
mod entradas;
#[path = "global_driver_rankings/payload.rs"]
mod payload;
#[path = "global_driver_rankings/pontuacao.rs"]
mod pontuacao;
#[path = "global_driver_rankings/tipos.rs"]
mod tipos;
#[path = "global_driver_rankings/titulos.rs"]
mod titulos;
#[path = "global_driver_rankings/titulos_equipe.rs"]
mod titulos_equipe;
#[path = "global_driver_rankings/utilitarios.rs"]
mod utilitarios;

// `payload` e `pontuacao` carregam os dois pontos de entrada do módulo
// (`get_global_driver_rankings_in_base_dir` e `historical_index_for_driver`).
pub(crate) use payload::*;
pub(crate) use pontuacao::*;

// A regra do que CONTA como título é usada também pela ficha do piloto, que
// lista os campeonatos por ano: o número na ficha e o número na tabela que
// ordena o mundo têm que sair da mesma régua.
pub(crate) use titulos::valid_archived_title_count;

// O resto é vocabulário interno: os irmãos enxergam via `use super::*`.
use aposentados::*;
use arquivo::*;
use carreira_real::*;
use categorias::*;
use classes::*;
use classificacao::*;
use entradas::*;
use tipos::*;
use titulos::*;
use titulos_equipe::*;
use utilitarios::*;

#[cfg(test)]
#[path = "global_driver_rankings/tests/mod.rs"]
mod tests;
