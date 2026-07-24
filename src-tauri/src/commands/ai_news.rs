//! Comando lazy do boletim de IA.
//!
//! Quando o jogador ABRE uma notícia de corrida, o front chama este comando. Ele
//! lê os fatos curados guardados no fim da corrida, manda ao servidor (que chama
//! o Gemini) e devolve o boletim — cacheando o resultado. Em qualquer falha,
//! devolve `story: None` e o front cai no texto-template padrão (nunca quebra).
//!
//! Este arquivo é só a fachada: os submódulos vivem em `ai_news/` e todos os
//! caminhos públicos continuam saindo daqui.

use rusqlite::OptionalExtension;
use serde::Serialize;
use tauri::Manager;

use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::db::queries::{ai_post_race, ai_pre_race, ai_story, meta};
use crate::narrative::client::{self, StoryError};

#[path = "ai_news/arco.rs"]
mod arco;
#[path = "ai_news/comandos.rs"]
mod comandos;
#[path = "ai_news/engajamento.rs"]
mod engajamento;
#[path = "ai_news/fatos.rs"]
mod fatos;
#[path = "ai_news/telemetria.rs"]
mod telemetria;
#[path = "ai_news/tese.rs"]
mod tese;
#[path = "ai_news/tipos.rs"]
mod tipos;

pub use comandos::*;
pub use tipos::*;

pub(crate) use arco::*;
pub(crate) use engajamento::*;
pub(crate) use fatos::*;
pub(crate) use telemetria::*;
pub(crate) use tese::*;

#[cfg(test)]
#[path = "ai_news/tests/mod.rs"]
mod tests;
