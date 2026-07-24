//! Criacao e listagem de backups de temporada: staging do .db, snapshot dos arquivos
//! auxiliares e leitura dos metadados de cada backup no diretorio da carreira.

use super::*;

const SNAPSHOT_SIDE_CAR_FILES: &[&str] = &[
    "meta.json",
    "race_results.json",
    "resume_context.json",
    "briefing_phrase_history.json",
    "preseason_plan.json",
];

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

pub(super) fn season_backup_filename(season_number: u32) -> String {
    format!("temporada_{season_number:03}.db")
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

pub(super) fn parse_backup_filename(name: &str) -> Option<u32> {
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

pub(super) fn backup_sidecars_dir(backups_dir: &Path, season_number: u32) -> PathBuf {
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
