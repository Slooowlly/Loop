//! Restauracao de um backup de temporada: troca do banco atual, recuperacao dos
//! arquivos auxiliares e reconstrucao do meta.json quando o snapshot nao existe.

use super::*;

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
        lifecycle_status: SaveLifecycleStatus::Active,
        history_start_year: None,
        history_end_year: None,
        playable_start_year: None,
        draft_progress_year: None,
        draft_error: None,
        pending_player_nationality: None,
        pending_player_age: None,
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
        .or(player.categoria_atual)
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
