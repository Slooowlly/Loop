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

pub use comandos::*;
pub(crate) use backup::*;
pub(crate) use comum::*;
pub(crate) use restore::*;

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
