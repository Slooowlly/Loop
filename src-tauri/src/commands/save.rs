use chrono::Local;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

use crate::commands::career_types::SaveLifecycleStatus;
use crate::config::app_config::{AppConfig, SaveMeta};
use crate::db::connection::Database;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::seasons as season_queries;

#[path = "save/backup.rs"]
mod backup;
#[path = "save/comandos.rs"]
mod comandos;
#[path = "save/comum.rs"]
mod comum;
#[path = "save/restore.rs"]
mod restore;

pub(crate) use backup::*;
pub use comandos::*;
pub(crate) use comum::*;
pub(crate) use restore::*;

/// Os arquivos auxiliares que viajam junto com o banco no snapshot da temporada.
/// FONTE ÚNICA do backup e do restore: as duas listas viviam separadas, e um arquivo
/// novo entrava no snapshot sem entrar na restauração (ou o contrário) sem nada acusar.
pub(crate) const SIDECAR_FILES: &[&str] = &[
    "race_results.json",
    "resume_context.json",
    "briefing_phrase_history.json",
    "preseason_plan.json",
];

/// As telas pós-corrida (`race_screens/<race_id>.json`) são estado da carreira e entram
/// no snapshot como DIRETÓRIO INTEIRO, nunca arquivo a arquivo: os IDs de corrida são
/// reaproveitados pela linha temporal que nasce do restore, então uma tela sobrevivente
/// do futuro abandonado voltaria a ser aberta como se fosse da corrida recém-disputada.
pub(crate) const RACE_SCREENS_DIR: &str = "race_screens";

#[derive(Debug, serde::Serialize)]
pub struct FlushResult {
    pub last_saved: String,
}

#[derive(Debug, serde::Serialize)]
pub struct BackupInfo {
    pub season_number: u32,
    pub file_name: String,
    pub file_path: String,
    pub size_kb: u64,
    pub modified_at: String,
}

#[cfg(test)]
#[path = "save/tests/mod.rs"]
mod tests;
