use std::path::{Path, PathBuf};

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::commands::career_types::SpecialWindowPayload;
use crate::config::app_config::AppConfig;
use crate::convocation::player_offers::{
    expire_remaining_player_special_offers_for_season,
    get_pending_player_special_offers_for_season, get_player_special_offer_by_id_for_season,
    update_player_special_offer_status_for_season,
};
use crate::convocation::special_window;
use crate::convocation::{
    advance_to_convocation_window as adv_fn, encerrar_bloco_especial as encerrar_fn,
    iniciar_bloco_especial as iniciar_fn, run_convocation_window as run_fn,
    run_pos_especial as pos_fn, ConvocationResult, PlayerSpecialOffer, PosEspecialResult,
};
use crate::db::connection::Database;
use crate::db::queries::{
    contracts as contract_queries, drivers as driver_queries, seasons as season_queries,
    teams as team_queries,
};
use crate::generators::ids::{next_id, IdType};
use crate::models::driver::Driver;
use crate::models::enums::{ContractStatus, SeasonPhase, TeamRole};
use crate::models::season::Season;

#[path = "convocation/comandos.rs"]
mod comandos;
#[path = "convocation/comum.rs"]
mod comum;
#[path = "convocation/janela.rs"]
mod janela;
#[path = "convocation/ofertas.rs"]
mod ofertas;

pub use comandos::*;
// Só o próprio módulo (e os irmãos, via `use super::*`) consomem o helper de caminho.
use comum::*;
pub(crate) use janela::*;
pub(crate) use ofertas::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSpecialOfferResponse {
    pub success: bool,
    pub action: String,
    pub message: String,
    pub special_category: Option<String>,
    pub remaining_offers: i32,
}

#[cfg(test)]
#[path = "convocation/tests/mod.rs"]
mod tests;
