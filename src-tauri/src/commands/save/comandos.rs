//! Casca `#[tauri::command]` do save: resolve o diretorio da carreira a partir do
//! AppHandle e delega para as funcoes internas de flush, backup e restore.

use super::*;

#[tauri::command]
pub async fn flush_save(app: AppHandle, career_id: String) -> Result<FlushResult, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    let config = AppConfig::load_or_default(&base_dir);
    let career_number = parse_career_number(&career_id)?;

    let career_dir = config.career_dir(career_number);
    if !career_dir.exists() {
        return Err(format!("Save nao encontrado: {career_id}"));
    }

    let db_path = config.career_db_path(career_number);
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;
    checkpoint_wal(&db)?;

    let meta_path = config.career_meta_path(career_number);
    let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    update_meta_timestamps(&meta_path, |meta| {
        meta.last_saved = Some(now.clone());
    })?;

    Ok(FlushResult { last_saved: now })
}

#[tauri::command]
pub async fn create_season_backup(
    app: AppHandle,
    career_id: String,
    season_number: u32,
) -> Result<BackupInfo, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    let config = AppConfig::load_or_default(&base_dir);
    let career_number = parse_career_number(&career_id)?;
    let career_dir = config.career_dir(career_number);

    if !career_dir.exists() {
        return Err(format!("Save nao encontrado: {career_id}"));
    }

    let db_path = config.career_db_path(career_number);
    let meta_path = config.career_meta_path(career_number);

    backup_season_internal(&db_path, &career_dir, season_number, &meta_path)
}

#[tauri::command]
pub async fn list_backups(app: AppHandle, career_id: String) -> Result<Vec<BackupInfo>, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    let config = AppConfig::load_or_default(&base_dir);
    let career_number = parse_career_number(&career_id)?;
    let career_dir = config.career_dir(career_number);

    if !career_dir.exists() {
        return Err(format!("Save nao encontrado: {career_id}"));
    }

    list_backups_in_career_dir(&career_dir)
}

#[tauri::command]
pub async fn restore_backup(
    app: AppHandle,
    career_id: String,
    season_number: u32,
) -> Result<(), String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    let config = AppConfig::load_or_default(&base_dir);
    let career_number = parse_career_number(&career_id)?;
    let career_dir = config.career_dir(career_number);

    if !career_dir.exists() {
        return Err(format!("Save nao encontrado: {career_id}"));
    }

    let db_path = config.career_db_path(career_number);
    restore_backup_internal(&db_path, &career_dir, season_number)
}
