//! Restauracao de um backup de temporada: inspecao do backup, troca do banco atual
//! por staging, recuperacao dos arquivos auxiliares e reconstrucao do meta.json
//! quando o snapshot nao existe.

use super::*;

/// Interruptor SO DE TESTE: apaga o arquivo em staging logo antes da troca, para
/// exercitar o rollback da substituicao. Some inteiro do binario de release.
#[cfg(test)]
thread_local! {
    pub(crate) static SABOTAR_TROCA_DO_RESTORE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
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

    // A inspecao vem ANTES de qualquer escrita: um backup de schema incompativel tem
    // que ser recusado com o banco vivo ainda intocado. Restaurar primeiro e descobrir
    // depois deixaria o jogador com a carreira substituida por um arquivo que o jogo
    // nao sabe abrir.
    inspecionar_backup(&backup_path, season_number)?;

    if db_path.exists() {
        match Database::open_existing(db_path) {
            Ok(db) => {
                checkpoint_wal(&db)?;
                drop(db);
                copiar_seguranca(db_path, career_dir, false)?;
            }
            // Banco vivo que ESTA versao nao sabe abrir (schema mais novo que o binario,
            // arquivo corrompido) e exatamente o caso em que restaurar um backup e a
            // saida. Sem checkpoint possivel o WAL vai inteiro para a copia de seguranca,
            // em vez de ser descartado junto com o que ele ainda carrega.
            Err(_) => copiar_seguranca(db_path, career_dir, true)?,
        }

        // Com o checkpoint TRUNCATE feito (ou com o WAL ja guardado na copia de
        // seguranca), sair com os dois auxiliares nao perde commit nenhum.
        let _ = std::fs::remove_file(career_dir.join("career.db-wal"));
        let _ = std::fs::remove_file(career_dir.join("career.db-shm"));
    }

    trocar_banco_por_staging(&backup_path, db_path)?;
    restore_sidecar_snapshot(career_dir, season_number)
}

/// Abre o backup so para leitura e cobra a compatibilidade de schema.
///
/// A regua e a mesma de `run_pending` ([`verificar_compatibilidade_do_schema`]): backup
/// anterior a baseline nao tem caminho de atualizacao, e backup mais novo que este
/// binario nao tem como ser rebaixado. Aqui o `0` tambem e recusado, e ele significa
/// coisa diferente: no save novo significa "banco vazio, deixe a baseline nascer", num
/// backup significa "este arquivo nao e um save carimbado".
///
/// A conexao e READ_ONLY de proposito: `Database::open_existing` rodaria `run_pending`
/// e MIGRARIA o arquivo de backup so por ter sido inspecionado.
fn inspecionar_backup(backup_path: &Path, season_number: u32) -> Result<(), String> {
    let conn = rusqlite::Connection::open_with_flags(
        backup_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| {
        format!(
            "Falha ao abrir o backup da temporada {season_number} para inspecao ('{}'): {e}",
            backup_path.display()
        )
    })?;

    let versao = crate::db::migrations::get_schema_version(&conn).map_err(|e| {
        format!("Falha ao ler a versao de schema do backup da temporada {season_number}: {e}")
    })?;

    if versao == 0 {
        return Err(format!(
            "O backup da temporada {season_number} nao tem carimbo de schema: o arquivo nao e \
             um save do Loop ou esta corrompido. O banco atual nao foi alterado."
        ));
    }

    crate::db::migrations::verificar_compatibilidade_do_schema(versao).map_err(|e| {
        format!(
            "Backup da temporada {season_number} incompativel com esta versao do Loop: {e} \
             O banco atual nao foi alterado."
        )
    })
}

/// Copia o backup para um arquivo de staging ao lado do banco vivo e so entao troca os
/// dois, pelo mesmo mecanismo com rollback que o backup usa.
///
/// A copia direta por cima do `career.db` escrevia no arquivo bom durante a operacao:
/// falha no meio (disco cheio, arquivo travado) deixava a carreira com um banco pela
/// metade, sem original e sem backup aplicado.
fn trocar_banco_por_staging(backup_path: &Path, db_path: &Path) -> Result<(), String> {
    let staged = caminho_de_staging(db_path);

    if staged.exists() {
        std::fs::remove_file(&staged).map_err(|e| {
            format!(
                "Falha ao limpar banco em staging '{}': {e}",
                staged.display()
            )
        })?;
    }

    std::fs::copy(backup_path, &staged).map_err(|e| {
        let _ = std::fs::remove_file(&staged);
        format!("Falha ao restaurar backup: {e}")
    })?;

    #[cfg(test)]
    if SABOTAR_TROCA_DO_RESTORE.with(|interruptor| interruptor.get()) {
        let _ = std::fs::remove_file(&staged);
    }

    substituir_preservando_anterior(&staged, db_path, "banco da carreira").map_err(|e| {
        let _ = std::fs::remove_file(&staged);
        e
    })
}

/// Copia de seguranca do banco vivo antes da troca. Com `levar_wal`, o `-wal` vai junto
/// como `career.db.bak-wal`: e o unico jeito de nao descartar commits de um banco que
/// nao pode ser drenado porque nem abre.
fn copiar_seguranca(db_path: &Path, career_dir: &Path, levar_wal: bool) -> Result<(), String> {
    std::fs::copy(db_path, career_dir.join("career.db.bak"))
        .map_err(|e| format!("Falha ao criar copia de seguranca do banco atual: {e}"))?;

    if levar_wal {
        let wal = career_dir.join("career.db-wal");
        if wal.exists() {
            std::fs::copy(&wal, career_dir.join("career.db.bak-wal")).map_err(|e| {
                format!("Falha ao guardar o WAL do banco atual na copia de seguranca: {e}")
            })?;
        }
    }

    Ok(())
}

fn caminho_de_staging(db_path: &Path) -> PathBuf {
    let mut nome = db_path
        .file_name()
        .map(|nome| nome.to_os_string())
        .unwrap_or_default();
    nome.push(".novo");
    db_path.with_file_name(nome)
}

/// Devolve os arquivos auxiliares ao estado do snapshot, TODOS OU NENHUM.
///
/// Cada arquivo era copiado por cima do vivo, um de cada vez: a falha do meio (disco
/// cheio, arquivo travado) deixava o save com metade da linha temporal antiga e metade da
/// nova, e nada no jogo acusava. Aqui a restauracao inteira e preparada em staging e
/// publicada de uma vez, com rollback — ver [`TrocaEmLote`].
///
/// Sem snapshot no backup (backup gravado antes de o snapshot existir), os auxiliares
/// vivos sao REMOVIDOS: eles descrevem a temporada que esta sendo abandonada, e mante-los
/// ao lado de um banco de outro momento e justamente o estado misto que isto evita.
fn restore_sidecar_snapshot(career_dir: &Path, season_number: u32) -> Result<(), String> {
    let backups_dir = career_dir.join("backups");
    let sidecars_dir = backup_sidecars_dir(&backups_dir, season_number);
    let tem_snapshot = sidecars_dir.exists();

    let mut troca = TrocaEmLote::nova();

    for file_name in SIDECAR_FILES {
        let do_snapshot = sidecars_dir.join(file_name);
        let vivo = career_dir.join(file_name);

        if tem_snapshot && do_snapshot.exists() {
            troca.preparar_copia(&do_snapshot, &vivo, file_name)?;
        } else if vivo.exists() {
            troca.preparar_remocao(&vivo, file_name);
        }
    }

    // Telas pos-corrida: o diretorio inteiro, nunca arquivo a arquivo — ver
    // `RACE_SCREENS_DIR`. Restaurar por arquivo deixaria de pe a tela de um ID que a
    // linha temporal nova vai reaproveitar, e o jogador reabriria o pos-corrida de uma
    // corrida que nunca aconteceu.
    let telas_do_snapshot = sidecars_dir.join(RACE_SCREENS_DIR);
    let telas_vivas = career_dir.join(RACE_SCREENS_DIR);
    if tem_snapshot && telas_do_snapshot.exists() {
        troca.preparar_copia(&telas_do_snapshot, &telas_vivas, RACE_SCREENS_DIR)?;
    } else if telas_vivas.exists() {
        troca.preparar_remocao(&telas_vivas, RACE_SCREENS_DIR);
    }

    let meta_do_snapshot = sidecars_dir.join("meta.json");
    let meta_vem_do_snapshot = tem_snapshot && meta_do_snapshot.exists();
    if meta_vem_do_snapshot {
        troca.preparar_copia(
            &meta_do_snapshot,
            &career_dir.join("meta.json"),
            "meta.json",
        )?;
    }

    troca.confirmar()?;

    if !meta_vem_do_snapshot {
        rebuild_meta_from_restored_db(career_dir)?;
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
