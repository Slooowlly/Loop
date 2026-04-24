use chrono::Local;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

use crate::config::app_config::{AppConfig, SaveMeta};
use crate::db::connection::Database;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::seasons as season_queries;

const SNAPSHOT_SIDE_CAR_FILES: &[&str] = &[
    "meta.json",
    "race_results.json",
    "resume_context.json",
    "briefing_phrase_history.json",
    "preseason_plan.json",
];

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

pub(crate) fn backup_season_internal(
    db_path: &Path,
    career_dir: &Path,
    season_number: u32,
    meta_path: &Path,
) -> Result<BackupInfo, String> {
    let db = Database::open_existing(db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let backups_dir = career_dir.join("backups");
    std::fs::create_dir_all(&backups_dir)
        .map_err(|e| format!("Falha ao criar diretorio de backups: {e}"))?;

    let file_name = season_backup_filename(season_number);
    let final_db = backups_dir.join(&file_name);
    let staged_db = staged_backup_db_path(&final_db);
    let final_sidecars = backup_sidecars_dir(&backups_dir, season_number);
    let staged_sidecars = staged_backup_sidecars_dir(&backups_dir, season_number);

    cleanup_staged_backup_artifacts(&staged_db, &staged_sidecars)?;

    let result = (|| -> Result<BackupInfo, String> {
        db.backup(&staged_db)
            .map_err(|e| format!("Falha ao criar backup: {e}"))?;
        snapshot_sidecar_files(career_dir, &staged_sidecars)?;

        let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        update_meta_timestamps(meta_path, |meta| {
            meta.last_backup = Some(now.clone());
            meta.last_saved = Some(now.clone());
        })?;
        std::fs::copy(meta_path, staged_sidecars.join("meta.json"))
            .map_err(|e| format!("Falha ao atualizar meta.json no snapshot do backup: {e}"))?;

        replace_backup_file(&staged_db, &final_db)?;
        replace_backup_sidecars(&staged_sidecars, &final_sidecars)?;

        file_backup_info(&final_db, season_number, &file_name)
    })();

    if result.is_err() {
        let _ = cleanup_staged_backup_artifacts(&staged_db, &staged_sidecars);
    }

    result
}

pub(crate) fn list_backups_in_career_dir(career_dir: &Path) -> Result<Vec<BackupInfo>, String> {
    let backups_dir = career_dir.join("backups");
    if !backups_dir.exists() {
        return Ok(Vec::new());
    }

    let mut backups = scan_backups_dir(&backups_dir)?;
    backups.sort_by(|a, b| a.season_number.cmp(&b.season_number));
    Ok(backups)
}

pub(crate) fn restore_backup_internal(
    db_path: &Path,
    career_dir: &Path,
    season_number: u32,
) -> Result<(), String> {
    let backup_path = career_dir
        .join("backups")
        .join(season_backup_filename(season_number));

    if !backup_path.exists() {
        return Err(format!(
            "Backup da temporada {} nao encontrado.",
            season_number
        ));
    }

    if db_path.exists() {
        let db = Database::open_existing(db_path)
            .map_err(|e| format!("Falha ao abrir banco atual: {e}"))?;
        checkpoint_wal(&db)?;
        drop(db);

        let safety = career_dir.join("career.db.bak");
        std::fs::copy(db_path, &safety)
            .map_err(|e| format!("Falha ao criar copia de seguranca do banco atual: {e}"))?;

        let _ = std::fs::remove_file(career_dir.join("career.db-wal"));
        let _ = std::fs::remove_file(career_dir.join("career.db-shm"));
    }

    std::fs::copy(&backup_path, db_path).map_err(|e| format!("Falha ao restaurar backup: {e}"))?;
    restore_sidecar_snapshot(career_dir, season_number)
}

fn season_backup_filename(season_number: u32) -> String {
    format!("temporada_{season_number:03}.db")
}

fn checkpoint_wal(db: &Database) -> Result<(), String> {
    db.conn
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|e| format!("Falha no WAL checkpoint: {e}"))
}

fn update_meta_timestamps<F>(meta_path: &Path, mutate: F) -> Result<(), String>
where
    F: FnOnce(&mut SaveMeta),
{
    let content =
        std::fs::read_to_string(meta_path).map_err(|e| format!("Falha ao ler meta.json: {e}"))?;
    let mut meta: SaveMeta =
        serde_json::from_str(&content).map_err(|e| format!("Falha ao parsear meta.json: {e}"))?;
    mutate(&mut meta);
    let updated = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Falha ao serializar meta.json: {e}"))?;
    std::fs::write(meta_path, updated).map_err(|e| format!("Falha ao gravar meta.json: {e}"))
}

pub(crate) fn parse_career_number(career_id: &str) -> Result<u32, String> {
    let s = career_id.trim_start_matches("career_");
    s.parse::<u32>()
        .map_err(|_| format!("career_id invalido: '{career_id}'"))
}

fn scan_backups_dir(backups_dir: &Path) -> Result<Vec<BackupInfo>, String> {
    let entries = std::fs::read_dir(backups_dir).map_err(|e| {
        format!(
            "Falha ao ler diretorio de backups '{}': {e}",
            backups_dir.display()
        )
    })?;

    let mut backups = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| {
            format!(
                "Falha ao listar arquivos de backup em '{}': {e}",
                backups_dir.display()
            )
        })?;
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(season_number) = parse_backup_filename(&name) else {
            continue;
        };
        let path = entry.path();
        backups.push(file_backup_info(&path, season_number, &name)?);
    }

    Ok(backups)
}

fn parse_backup_filename(name: &str) -> Option<u32> {
    let stem = name.strip_suffix(".db")?;
    let digits = stem
        .strip_prefix("temporada_")
        .or_else(|| stem.strip_prefix("season_"))?;
    digits.parse::<u32>().ok()
}

fn file_backup_info(
    path: &Path,
    season_number: u32,
    file_name: &str,
) -> Result<BackupInfo, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| format!("Falha ao ler metadata de '{}': {e}", path.display()))?;
    let modified = metadata.modified().map_err(|e| {
        format!(
            "Falha ao ler data de modificacao de '{}': {e}",
            path.display()
        )
    })?;
    let secs = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("Falha ao converter timestamp de '{}': {e}", path.display()))?
        .as_secs();
    let modified_at = chrono::DateTime::from_timestamp(secs as i64, 0)
        .unwrap_or_default()
        .with_timezone(&Local)
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();

    Ok(BackupInfo {
        season_number,
        file_name: file_name.to_string(),
        file_path: path.to_string_lossy().to_string(),
        size_kb: metadata.len() / 1024,
        modified_at,
    })
}

fn staged_backup_db_path(final_db: &Path) -> PathBuf {
    final_db.with_extension("db.tmp")
}

fn backup_sidecars_dir(backups_dir: &Path, season_number: u32) -> PathBuf {
    backups_dir.join(format!("temporada_{season_number:03}.files"))
}

fn staged_backup_sidecars_dir(backups_dir: &Path, season_number: u32) -> PathBuf {
    backups_dir.join(format!("temporada_{season_number:03}.files.tmp"))
}

fn cleanup_staged_backup_artifacts(staged_db: &Path, staged_sidecars: &Path) -> Result<(), String> {
    if staged_db.exists() {
        std::fs::remove_file(staged_db).map_err(|e| {
            format!(
                "Falha ao limpar arquivo temporario de backup '{}': {e}",
                staged_db.display()
            )
        })?;
    }

    if staged_sidecars.exists() {
        std::fs::remove_dir_all(staged_sidecars).map_err(|e| {
            format!(
                "Falha ao limpar diretorio temporario de backup '{}': {e}",
                staged_sidecars.display()
            )
        })?;
    }

    Ok(())
}

fn snapshot_sidecar_files(career_dir: &Path, snapshot_dir: &Path) -> Result<(), String> {
    if snapshot_dir.exists() {
        std::fs::remove_dir_all(snapshot_dir).map_err(|e| {
            format!(
                "Falha ao limpar snapshot temporario '{}': {e}",
                snapshot_dir.display()
            )
        })?;
    }

    std::fs::create_dir_all(snapshot_dir).map_err(|e| {
        format!(
            "Falha ao criar snapshot temporario '{}': {e}",
            snapshot_dir.display()
        )
    })?;

    for file_name in SNAPSHOT_SIDE_CAR_FILES {
        let source = career_dir.join(file_name);
        if !source.exists() {
            continue;
        }

        if !source.is_file() {
            continue;
        }

        std::fs::copy(&source, snapshot_dir.join(file_name)).map_err(|e| {
            format!(
                "Falha ao copiar '{}' para o snapshot do backup: {e}",
                source.display()
            )
        })?;
    }

    Ok(())
}

fn replace_backup_file(staged_db: &Path, final_db: &Path) -> Result<(), String> {
    if final_db.exists() {
        std::fs::remove_file(final_db).map_err(|e| {
            format!(
                "Falha ao sobrescrever backup anterior '{}': {e}",
                final_db.display()
            )
        })?;
    }

    std::fs::rename(staged_db, final_db).map_err(|e| {
        format!(
            "Falha ao finalizar backup '{}' a partir de '{}': {e}",
            final_db.display(),
            staged_db.display()
        )
    })
}

fn replace_backup_sidecars(staged_dir: &Path, final_dir: &Path) -> Result<(), String> {
    if final_dir.exists() {
        std::fs::remove_dir_all(final_dir).map_err(|e| {
            format!(
                "Falha ao sobrescrever snapshot auxiliar '{}': {e}",
                final_dir.display()
            )
        })?;
    }

    std::fs::rename(staged_dir, final_dir).map_err(|e| {
        format!(
            "Falha ao finalizar snapshot auxiliar '{}' a partir de '{}': {e}",
            final_dir.display(),
            staged_dir.display()
        )
    })
}

fn restore_sidecar_snapshot(career_dir: &Path, season_number: u32) -> Result<(), String> {
    let backups_dir = career_dir.join("backups");
    let sidecars_dir = backup_sidecars_dir(&backups_dir, season_number);

    if !sidecars_dir.exists() {
        clear_runtime_sidecars(career_dir)?;
        rebuild_meta_from_restored_db(career_dir)?;
        return Ok(());
    }

    for file_name in [
        "race_results.json",
        "resume_context.json",
        "briefing_phrase_history.json",
        "preseason_plan.json",
    ] {
        let snapshot_file = sidecars_dir.join(file_name);
        let live_file = career_dir.join(file_name);

        if snapshot_file.exists() {
            std::fs::copy(&snapshot_file, &live_file).map_err(|e| {
                format!(
                    "Falha ao restaurar arquivo auxiliar '{}' do backup: {e}",
                    live_file.display()
                )
            })?;
        } else if live_file.exists() {
            std::fs::remove_file(&live_file).map_err(|e| {
                format!(
                    "Falha ao remover arquivo auxiliar obsoleto '{}' apos restore: {e}",
                    live_file.display()
                )
            })?;
        }
    }

    let snapshot_meta = sidecars_dir.join("meta.json");
    if snapshot_meta.exists() {
        std::fs::copy(&snapshot_meta, career_dir.join("meta.json"))
            .map_err(|e| format!("Falha ao restaurar meta.json do backup: {e}"))?;
    } else {
        rebuild_meta_from_restored_db(career_dir)?;
    }

    Ok(())
}

fn clear_runtime_sidecars(career_dir: &Path) -> Result<(), String> {
    for file_name in [
        "race_results.json",
        "resume_context.json",
        "briefing_phrase_history.json",
        "preseason_plan.json",
    ] {
        let path = career_dir.join(file_name);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| {
                format!(
                    "Falha ao limpar arquivo legado '{}' apos restore: {e}",
                    path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn rebuild_meta_from_restored_db(career_dir: &Path) -> Result<(), String> {
    let meta_path = career_dir.join("meta.json");
    let existing_meta = read_save_meta_if_present(&meta_path);
    let db_path = career_dir.join("career.db");
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco restaurado: {e}"))?;

    let active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa apos restore: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada apos restore.".to_string())?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao buscar piloto do jogador apos restore: {e}"))?;
    let active_contract = contract_queries::get_active_contract_for_pilot(&db.conn, &player.id)
        .map_err(|e| format!("Falha ao buscar contrato do jogador apos restore: {e}"))?;
    let total_races: i32 = db
        .conn
        .query_row("SELECT COUNT(*) FROM calendar", [], |row| row.get(0))
        .map_err(|e| format!("Falha ao contar corridas apos restore: {e}"))?;

    let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let mut meta = existing_meta.unwrap_or(SaveMeta {
        career_number: career_number_from_dir(career_dir).unwrap_or(1),
        player_name: player.nome.clone(),
        current_season: active_season.numero.max(1) as u32,
        current_year: active_season.ano.max(0) as u32,
        created_at: now.clone(),
        last_played: now.clone(),
        last_saved: None,
        last_backup: None,
        team_name: None,
        category: active_contract
            .as_ref()
            .map(|contract| contract.categoria.clone())
            .or_else(|| player.categoria_atual.clone())
            .unwrap_or_default(),
        difficulty: "medio".to_string(),
        total_races,
    });

    meta.player_name = player.nome;
    meta.current_season = active_season.numero.max(1) as u32;
    meta.current_year = active_season.ano.max(0) as u32;
    meta.last_played = now;
    meta.last_saved = None;
    meta.team_name = active_contract
        .as_ref()
        .map(|contract| contract.equipe_nome.clone());
    meta.category = active_contract
        .as_ref()
        .map(|contract| contract.categoria.clone())
        .or_else(|| player.categoria_atual)
        .unwrap_or(meta.category);
    meta.total_races = total_races;

    let payload = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Falha ao serializar meta restaurado: {e}"))?;
    std::fs::write(&meta_path, payload).map_err(|e| format!("Falha ao gravar meta restaurado: {e}"))
}

fn read_save_meta_if_present(path: &Path) -> Option<SaveMeta> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<SaveMeta>(&content).ok()
}

fn career_number_from_dir(career_dir: &Path) -> Option<u32> {
    let name = career_dir.file_name()?.to_string_lossy();
    let digits = name.strip_prefix("career_")?;
    digits.parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::{
        backup_season_internal, list_backups_in_career_dir, parse_backup_filename,
        restore_backup_internal,
    };
    use crate::commands::career::{
        advance_market_week_in_base_dir, advance_season_in_base_dir, create_career_in_base_dir,
        finalize_preseason_in_base_dir, get_player_proposals_in_base_dir,
        respond_to_proposal_in_base_dir, CreateCareerInput,
    };
    use crate::commands::convocation::{
        get_player_special_offers_in_base_dir, respond_player_special_offer_in_base_dir,
    };
    use crate::commands::race::simulate_race_weekend_in_base_dir;
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::calendar as calendar_queries;
    use crate::db::queries::contracts as contract_queries;
    use crate::db::queries::drivers as driver_queries;
    use crate::db::queries::market_proposals as market_proposal_queries;
    use crate::db::queries::seasons as season_queries;
    use crate::db::queries::teams as team_queries;
    use crate::market::proposals::{MarketProposal, ProposalStatus};
    use crate::models::enums::TeamRole;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("iracer_save_cmd_{label}_{nanos}"))
    }

    fn create_test_career_dir(label: &str) -> PathBuf {
        let base_dir = unique_test_dir(label);
        fs::create_dir_all(&base_dir).expect("base dir");

        let input = CreateCareerInput {
            player_name: "Joao Silva".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(22),
            category: "mazda_rookie".to_string(),
            team_index: 2,
            difficulty: "medio".to_string(),
        };

        create_career_in_base_dir(&base_dir, input).expect("career should be created");
        base_dir
    }

    fn career_paths(base_dir: &Path) -> (AppConfig, PathBuf, PathBuf, PathBuf) {
        let config = AppConfig::load_or_default(base_dir);
        let career_dir = config.saves_dir().join("career_001");
        let db_path = career_dir.join("career.db");
        let meta_path = career_dir.join("meta.json");
        (config, career_dir, db_path, meta_path)
    }

    fn mark_all_races_completed(db_path: &Path) {
        let db = Database::open_existing(db_path).expect("db");
        db.conn
            .execute("UPDATE calendar SET status = 'Concluida'", [])
            .expect("mark all races completed");
        db.conn
            .execute(
                "UPDATE seasons SET fase = 'PosEspecial' WHERE status = 'EmAndamento'",
                [],
            )
            .expect("mark season as post-special");
    }

    fn mark_regular_races_completed(db: &Database) {
        db.conn
            .execute(
                "UPDATE calendar SET status = 'Concluida' WHERE season_phase = 'BlocoRegular'",
                [],
            )
            .expect("complete regular block");
    }

    fn mark_remaining_special_races_completed(db_path: &Path, season_id: &str) {
        let db = Database::open_existing(db_path).expect("db");
        db.conn
            .execute(
                "UPDATE calendar
                 SET status = 'Concluida'
                 WHERE temporada_id = ?1
                   AND categoria IN ('production_challenger', 'endurance')",
                rusqlite::params![season_id],
            )
            .expect("mark remaining special races completed");
    }

    fn force_complete_preseason_plan(save_dir: &Path) {
        let mut plan = crate::market::preseason::load_preseason_plan(save_dir)
            .expect("load preseason plan")
            .expect("preseason plan");
        plan.state.is_complete = true;
        plan.state.current_week = plan.state.total_weeks + 1;
        plan.state.phase = crate::market::preseason::PreSeasonPhase::Complete;
        plan.state.player_has_pending_proposals = false;
        crate::market::preseason::save_preseason_plan(save_dir, &plan)
            .expect("save completed preseason plan");
    }

    fn seed_player_regular_proposal(
        conn: &rusqlite::Connection,
        season_id: &str,
        proposal: &MarketProposal,
    ) {
        market_proposal_queries::insert_player_proposal(conn, season_id, proposal)
            .expect("insert player proposal");
    }

    #[test]
    fn backup_restore_round_trip_restores_sidecar_snapshot() {
        let base_dir = create_test_career_dir("restore_sidecars");
        let (_config, career_dir, db_path, meta_path) = career_paths(&base_dir);
        let race_results_path = career_dir.join("race_results.json");
        let resume_context_path = career_dir.join("resume_context.json");
        let briefing_path = career_dir.join("briefing_phrase_history.json");
        let preseason_path = career_dir.join("preseason_plan.json");

        fs::write(&race_results_path, "{\"version\":1}").expect("seed race results");
        fs::write(&resume_context_path, "{\"active_view\":\"preseason\"}")
            .expect("seed resume context");
        fs::write(&briefing_path, "{\"season_number\":1,\"entries\":[]}").expect("seed briefing");
        fs::write(&preseason_path, "{\"state\":{\"current_week\":1}}").expect("seed preseason");

        let original_meta = fs::read_to_string(&meta_path).expect("read original meta");
        backup_season_internal(&db_path, &career_dir, 1, &meta_path).expect("backup should work");

        fs::write(&race_results_path, "{\"version\":2}").expect("mutate race results");
        fs::remove_file(&resume_context_path).expect("remove resume context");
        fs::write(
            &briefing_path,
            "{\"season_number\":99,\"entries\":[{\"id\":\"changed\"}]}",
        )
        .expect("mutate briefing");
        fs::remove_file(&preseason_path).expect("remove preseason");
        fs::write(
            &meta_path,
            original_meta.replace("\"current_season\": 1", "\"current_season\": 99"),
        )
        .expect("mutate meta");

        restore_backup_internal(&db_path, &career_dir, 1).expect("restore should work");

        assert_eq!(
            fs::read_to_string(&race_results_path).expect("restored race results"),
            "{\"version\":1}"
        );
        assert_eq!(
            fs::read_to_string(&resume_context_path).expect("restored resume context"),
            "{\"active_view\":\"preseason\"}"
        );
        assert_eq!(
            fs::read_to_string(&briefing_path).expect("restored briefing"),
            "{\"season_number\":1,\"entries\":[]}"
        );
        assert_eq!(
            fs::read_to_string(&preseason_path).expect("restored preseason"),
            "{\"state\":{\"current_week\":1}}"
        );

        let restored_meta = fs::read_to_string(&meta_path).expect("restored meta");
        assert!(restored_meta.contains("\"current_season\": 1"));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn full_career_flow_backup_restore_round_trip() {
        let base_dir = create_test_career_dir("full_flow_backup_restore");
        let (_config, career_dir, db_path, meta_path) = career_paths(&base_dir);

        let db = Database::open_existing(&db_path).expect("db");
        let active_season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        let next_race =
            calendar_queries::get_next_race(&db.conn, &active_season.id, "mazda_rookie")
                .expect("next race query")
                .expect("pending race");

        let race_result = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate opening race");
        assert!(
            !race_result.player_race.race_results.is_empty(),
            "player race should persist race results",
        );

        let race_results_path = career_dir.join("race_results.json");
        assert!(
            race_results_path.exists(),
            "simulating a real race should create race_results.json",
        );

        mark_all_races_completed(&db_path);

        let season_result =
            advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        assert_eq!(season_result.new_year, 2025);
        assert!(season_result.preseason_initialized);
        assert!(
            season_result.promotion_result.errors.is_empty(),
            "promotion/relegation should finish without errors: {:?}",
            season_result.promotion_result.errors
        );

        let advanced_week =
            advance_market_week_in_base_dir(&base_dir, "career_001").expect("advance market week");
        assert_eq!(advanced_week.week_number, 1);

        let preseason_path = career_dir.join("preseason_plan.json");
        assert!(
            preseason_path.exists(),
            "preseason plan should exist after season advance"
        );

        backup_season_internal(&db_path, &career_dir, 2, &meta_path)
            .expect("season 2 backup should work");

        let expected_meta = fs::read_to_string(&meta_path).expect("read backed-up meta");
        let expected_race_results =
            fs::read_to_string(&race_results_path).expect("read backed-up race results");
        let expected_preseason =
            fs::read_to_string(&preseason_path).expect("read backed-up preseason");

        let mutated_meta = expected_meta
            .replace("\"current_season\": 2", "\"current_season\": 99")
            .replace("\"current_year\": 2025", "\"current_year\": 2099");
        fs::write(&meta_path, mutated_meta).expect("mutate meta");
        fs::write(&race_results_path, "{\"version\":999}").expect("mutate race results");
        fs::write(&preseason_path, "{\"state\":{\"current_week\":99}}").expect("mutate preseason");

        let db = Database::open_existing(&db_path).expect("db before restore");
        db.conn
            .execute(
                "UPDATE seasons SET numero = 99, ano = 2099 WHERE status = 'EmAndamento'",
                [],
            )
            .expect("mutate active season");

        restore_backup_internal(&db_path, &career_dir, 2).expect("restore season 2 backup");

        let restored_db = Database::open_existing(&db_path).expect("restored db");
        let restored_active_season = season_queries::get_active_season(&restored_db.conn)
            .expect("restored season query")
            .expect("restored active season");
        assert_eq!(restored_active_season.numero, 2);
        assert_eq!(restored_active_season.ano, 2025);

        assert_eq!(
            fs::read_to_string(&meta_path).expect("restored meta"),
            expected_meta
        );
        assert_eq!(
            fs::read_to_string(&race_results_path).expect("restored race results"),
            expected_race_results
        );
        assert_eq!(
            fs::read_to_string(&preseason_path).expect("restored preseason"),
            expected_preseason
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn full_preseason_player_proposal_flow_reaches_season_start() {
        let base_dir = create_test_career_dir("full_preseason_player_proposal_flow");
        let (config, career_dir, db_path, _meta_path) = career_paths(&base_dir);

        mark_all_races_completed(&db_path);
        let season_result =
            advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        assert!(season_result.preseason_initialized);

        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        let active_regular =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
                .expect("active regular contract query");
        let current_regular_team_id = active_regular
            .as_ref()
            .map(|contract| contract.equipe_id.clone());
        let player_regular_category = active_regular
            .map(|contract| contract.categoria)
            .or_else(|| player.categoria_atual.clone())
            .unwrap_or_else(|| "mazda_rookie".to_string());
        let target_team = team_queries::get_teams_by_category(&db.conn, &player_regular_category)
            .expect("teams by category")
            .into_iter()
            .find(|team| current_regular_team_id.as_ref() != Some(&team.id))
            .unwrap_or_else(|| {
                team_queries::get_teams_by_category(&db.conn, &player_regular_category)
                    .expect("fallback teams by category")
                    .into_iter()
                    .next()
                    .expect("at least one team in player regular category")
            });
        let proposal = MarketProposal {
            id: format!("MP-{}-{}", target_team.id, player.id),
            equipe_id: target_team.id.clone(),
            equipe_nome: target_team.nome.clone(),
            piloto_id: player.id.clone(),
            piloto_nome: player.nome.clone(),
            categoria: target_team.categoria.clone(),
            papel: TeamRole::Numero1,
            salario_oferecido: 125_000.0,
            duracao_anos: 2,
            status: ProposalStatus::Pendente,
            motivo_recusa: None,
        };
        seed_player_regular_proposal(&db.conn, &season.id, &proposal);
        drop(db);

        let proposals =
            get_player_proposals_in_base_dir(&base_dir, "career_001").expect("player proposals");
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].proposal_id, proposal.id);

        let response = respond_to_proposal_in_base_dir(&base_dir, "career_001", &proposal.id, true)
            .expect("accept proposal");
        assert!(response.success);
        assert_eq!(response.action, "accepted");
        assert_eq!(response.remaining_proposals, 0);
        assert_eq!(
            response.new_team_name.as_deref(),
            Some(target_team.nome.as_str())
        );

        force_complete_preseason_plan(&career_dir);
        finalize_preseason_in_base_dir(&base_dir, "career_001").expect("finalize preseason");

        let finalized_db = Database::open_existing(&db_path).expect("db after finalize");
        let finalized_contract =
            contract_queries::get_active_regular_contract_for_pilot(&finalized_db.conn, &player.id)
                .expect("active contract after finalize")
                .expect("player should keep active regular contract");
        let finalized_season = season_queries::get_active_season(&finalized_db.conn)
            .expect("active season after finalize")
            .expect("active season");

        assert_eq!(finalized_contract.equipe_id, target_team.id);
        assert_eq!(finalized_season.numero, 2);
        assert_eq!(finalized_season.ano, 2025);
        assert!(
            !config
                .saves_dir()
                .join("career_001")
                .join("preseason_plan.json")
                .exists(),
            "finalizar a pre-temporada deve remover o plano salvo"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn full_special_block_player_offer_flow_cleans_up_after_pos_especial() {
        let base_dir = create_test_career_dir("full_special_block_player_offer_flow");
        let (_config, _career_dir, db_path, _meta_path) = career_paths(&base_dir);

        let db = Database::open_existing(&db_path).expect("db");
        let mut player = driver_queries::get_player_driver(&db.conn).expect("player");
        player.categoria_atual = Some("gt4".to_string());
        player.atributos.skill = 98.0;
        driver_queries::update_driver(&db.conn, &player).expect("update player");

        mark_regular_races_completed(&db);
        crate::convocation::advance_to_convocation_window(&db.conn).expect("advance convocation");
        crate::convocation::run_convocation_window(&db.conn).expect("run convocation");
        drop(db);

        let offers =
            get_player_special_offers_in_base_dir(&base_dir, "career_001").expect("special offers");
        assert!(
            !offers.is_empty(),
            "convocation should generate at least one special offer for the player"
        );

        let accepted =
            respond_player_special_offer_in_base_dir(&base_dir, "career_001", &offers[0].id, true)
                .expect("accept special offer");
        assert_eq!(accepted.action, "accepted");
        assert_eq!(accepted.special_category.as_deref(), Some("endurance"));

        let db = Database::open_existing(&db_path).expect("db after accept");
        crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");
        let season = season_queries::get_active_season(&db.conn)
            .expect("active season")
            .expect("season");
        let next_special_race = calendar_queries::get_next_race(&db.conn, &season.id, "endurance")
            .expect("next endurance race query")
            .expect("pending endurance race");
        drop(db);

        let race_result =
            simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_special_race.id)
                .expect("simulate accepted special race");
        assert!(
            race_result
                .player_race
                .race_results
                .iter()
                .any(|entry| entry.is_jogador),
            "special race grid should include the player after accepting the special offer"
        );

        mark_remaining_special_races_completed(&db_path, &season.id);
        let db = Database::open_existing(&db_path).expect("db before pos especial");
        crate::convocation::encerrar_bloco_especial(&db.conn).expect("end special block");
        let pos_result = crate::convocation::run_pos_especial(&db.conn).expect("run pos especial");
        assert!(pos_result.errors.is_empty());

        let refreshed_player =
            driver_queries::get_player_driver(&db.conn).expect("player refreshed");
        let active_special =
            contract_queries::get_active_especial_contract_for_pilot(&db.conn, &player.id)
                .expect("active special contract query");
        let refreshed_season = season_queries::get_active_season(&db.conn)
            .expect("season query after pos especial")
            .expect("active season after pos especial");

        assert_eq!(refreshed_season.fase.as_str(), "PosEspecial");
        assert!(
            refreshed_player.categoria_especial_ativa.is_none(),
            "player should leave the special category after pos especial"
        );
        assert!(
            active_special.is_none(),
            "special contracts should be cleared after pos especial"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn full_preseason_rejection_flow_generates_emergency_proposals() {
        let base_dir = create_test_career_dir("full_preseason_rejection_flow");
        let (_config, career_dir, db_path, _meta_path) = career_paths(&base_dir);

        mark_all_races_completed(&db_path);
        let season_result =
            advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        assert!(season_result.preseason_initialized);

        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");

        if let Some(contract) =
            contract_queries::get_active_contract_for_pilot(&db.conn, &player.id)
                .expect("active contract query")
        {
            contract_queries::update_contract_status(
                &db.conn,
                &contract.id,
                &crate::models::enums::ContractStatus::Rescindido,
            )
            .expect("rescind player contract");
            team_queries::remove_pilot_from_team(&db.conn, &player.id, &contract.equipe_id)
                .expect("remove player from team");
        }

        let team = team_queries::get_teams_by_category(
            &db.conn,
            player.categoria_atual.as_deref().unwrap_or("mazda_rookie"),
        )
        .expect("teams by player category")
        .into_iter()
        .next()
        .expect("at least one regular team");
        let proposal = MarketProposal {
            id: format!("MP-{}-{}", team.id, player.id),
            equipe_id: team.id.clone(),
            equipe_nome: team.nome.clone(),
            piloto_id: player.id.clone(),
            piloto_nome: player.nome.clone(),
            categoria: team.categoria.clone(),
            papel: TeamRole::Numero1,
            salario_oferecido: 80_000.0,
            duracao_anos: 1,
            status: ProposalStatus::Pendente,
            motivo_recusa: None,
        };
        seed_player_regular_proposal(&db.conn, &season.id, &proposal);
        drop(db);

        let response =
            respond_to_proposal_in_base_dir(&base_dir, "career_001", &proposal.id, false)
                .expect("reject last player proposal");
        assert!(response.success);
        assert_eq!(response.action, "rejected");
        assert!(response.remaining_proposals > 0);
        assert!(response.new_team_name.is_none());
        assert!(
            response
                .message
                .contains("Novas opcoes emergenciais foram geradas"),
            "rejecting the final proposal without a team should generate emergency proposals"
        );

        let emergency_proposals =
            get_player_proposals_in_base_dir(&base_dir, "career_001").expect("emergency proposals");
        assert!(
            !emergency_proposals.is_empty(),
            "player should receive emergency proposals after rejecting the last offer without a team"
        );

        let reopened_db = Database::open_existing(&db_path).expect("db reopen");
        assert!(
            contract_queries::get_active_regular_contract_for_pilot(&reopened_db.conn, &player.id)
                .expect("active regular contract after rejection")
                .is_none(),
            "player should remain without a regular team until one emergency proposal is resolved"
        );

        force_complete_preseason_plan(&career_dir);
        let finalize_error = finalize_preseason_in_base_dir(&base_dir, "career_001")
            .expect_err("finalize should block while emergency proposals remain pending");
        assert!(finalize_error.contains("pendente"));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn backup_season_internal_does_not_leave_backup_when_meta_update_fails() {
        let base_dir = create_test_career_dir("meta_fail");
        let (_config, career_dir, db_path, _meta_path) = career_paths(&base_dir);
        let invalid_meta_path = career_dir.join("missing").join("meta.json");
        let backups_dir = career_dir.join("backups");
        let backup_file = backups_dir.join("temporada_001.db");
        let sidecars_dir = backups_dir.join("temporada_001.files");

        let err = backup_season_internal(&db_path, &career_dir, 1, &invalid_meta_path)
            .expect_err("invalid meta path should fail");

        assert!(err.contains("meta.json"));
        assert!(
            !backup_file.exists(),
            "failed backup should not leave final db"
        );
        assert!(
            !sidecars_dir.exists(),
            "failed backup should not leave final sidecar snapshot"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn list_backups_in_career_dir_propagates_filesystem_errors() {
        let base_dir = unique_test_dir("list_backups_error");
        fs::create_dir_all(&base_dir).expect("base dir");
        let fake_career_dir = base_dir.join("career_001");
        let backups_file = fake_career_dir.join("backups");
        fs::create_dir_all(&fake_career_dir).expect("career dir");
        fs::write(&backups_file, "not a directory").expect("seed backups file");

        let err = list_backups_in_career_dir(&fake_career_dir)
            .expect_err("read_dir failure should propagate");

        assert!(err.contains("backups"));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn restore_legacy_backup_rebuilds_meta_and_clears_stale_sidecars() {
        let base_dir = create_test_career_dir("legacy_restore");
        let (_config, career_dir, db_path, meta_path) = career_paths(&base_dir);
        let backups_dir = career_dir.join("backups");
        fs::create_dir_all(&backups_dir).expect("backups dir");

        let legacy_backup = backups_dir.join("temporada_001.db");
        let db = Database::open_existing(&db_path).expect("db");
        db.backup(&legacy_backup).expect("legacy db-only backup");

        let race_results_path = career_dir.join("race_results.json");
        let resume_context_path = career_dir.join("resume_context.json");
        let briefing_path = career_dir.join("briefing_phrase_history.json");
        let preseason_path = career_dir.join("preseason_plan.json");
        fs::write(&race_results_path, "{\"version\":2}").expect("seed race results");
        fs::write(&resume_context_path, "{\"active_view\":\"market\"}").expect("seed resume");
        fs::write(&briefing_path, "{\"season_number\":99,\"entries\":[]}").expect("seed briefing");
        fs::write(&preseason_path, "{\"state\":{\"current_week\":7}}").expect("seed preseason");

        let db = Database::open_existing(&db_path).expect("db");
        db.conn
            .execute(
                "UPDATE seasons SET numero = 99, ano = 2099 WHERE status = 'EmAndamento'",
                [],
            )
            .expect("mutate season");
        fs::write(
            &meta_path,
            fs::read_to_string(&meta_path)
                .expect("read meta")
                .replace("\"current_season\": 1", "\"current_season\": 99")
                .replace("\"current_year\": 2024", "\"current_year\": 2099"),
        )
        .expect("mutate meta");

        restore_backup_internal(&db_path, &career_dir, 1).expect("legacy restore should work");

        let restored_db = Database::open_existing(&db_path).expect("restored db");
        let active_season = season_queries::get_active_season(&restored_db.conn)
            .expect("season query")
            .expect("active season");
        assert_eq!(active_season.numero, 1);
        assert_eq!(active_season.ano, 2024);

        let restored_meta = fs::read_to_string(&meta_path).expect("restored meta");
        assert!(restored_meta.contains("\"current_season\": 1"));
        assert!(restored_meta.contains("\"current_year\": 2024"));
        assert!(!race_results_path.exists());
        assert!(!resume_context_path.exists());
        assert!(!briefing_path.exists());
        assert!(!preseason_path.exists());

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn parse_backup_filename_accepts_current_and_legacy_names() {
        assert_eq!(parse_backup_filename("temporada_007.db"), Some(7));
        assert_eq!(parse_backup_filename("season_042.db"), Some(42));
    }
}
