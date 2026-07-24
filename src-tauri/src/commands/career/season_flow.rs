//! Avanco de temporada: fechamento anual, simulacao das corridas pendentes sem o
//! jogador e a limpeza do estado especial legado da transicao 9D.

use super::*;

pub(crate) fn advance_season_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<EndOfSeasonResult, String> {
    let career_number =
        career_number_from_id(career_id).ok_or_else(|| "ID de carreira invalido.".to_string())?;
    let mut config = AppConfig::load_or_default(base_dir);
    let (mut db, career_dir, mut meta) = open_career_resources(base_dir, career_id)?;
    let meta_path = career_dir.join("meta.json");
    let mut season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

    let pending_races = calendar_queries::get_pending_races(&db.conn, &season.id)
        .map_err(|e| format!("Falha ao verificar corridas pendentes: {e}"))?;
    let pending_error = || {
        let mut pending_categories: Vec<String> = pending_races
            .iter()
            .map(|race| race.categoria.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        pending_categories.sort();
        format!(
            "Ainda existem {} corridas pendentes na temporada {} ({})",
            pending_races.len(),
            season.numero,
            pending_categories.join(", ")
        )
    };

    // O fechamento anual so acontece depois das corridas especiais e do PosEspecial.
    // Assim o mercado normal nunca atropela a convocacao nem o bloco especial.
    match season.fase {
        SeasonPhase::PreTemporada => {
            return Err("A temporada ainda nao comecou.".to_string());
        }
        SeasonPhase::Temporada => {
            if !pending_races.is_empty() {
                return Err(pending_error());
            }
            season_queries::update_season_fase(&db.conn, &season.id, &SeasonPhase::Encerramento)
                .map_err(|e| format!("Falha ao encerrar temporada concluida: {e}"))?;
            season.fase = SeasonPhase::Encerramento;
        }
        SeasonPhase::Encerramento => {}
        SeasonPhase::PosEspecial => {
            if !pending_races.is_empty() {
                return Err(pending_error());
            }
            cleanup_legacy_special_state_for_9d_transition(&db.conn, season.numero)?;
        }
        SeasonPhase::BlocoRegular => {
            if !pending_races.is_empty() {
                return Err(pending_error());
            }
            return Err(
                "A temporada regular terminou, mas a janela de convocacao especial ainda precisa ser aberta."
                    .to_string(),
            );
        }
        SeasonPhase::JanelaConvocacao | SeasonPhase::BlocoEspecial => {
            if !pending_races.is_empty() {
                return Err(pending_error());
            }
            return Err(format!(
                "Nao e possivel avancar a temporada na fase '{}'. Encerre o bloco especial primeiro.",
                season.fase
            ));
        } // LEGADO 9D: fases do modelo novo nunca chegam aqui em saves pré-v33
    }

    // Backup canônico de fim de temporada — antes de qualquer mutação da próxima.
    // Falha aqui bloqueia o pipeline: melhor abortar do que avançar sem rede de segurança.
    let db_path = career_dir.join("career.db");
    crate::commands::save::backup_season_internal(
        &db_path,
        &career_dir,
        season.numero as u32,
        &meta_path,
    )
    .map_err(|e| format!("Falha ao criar backup de fim de temporada: {e}"))?;

    let result = run_end_of_season(&mut db.conn, &season, &career_dir)?;
    warn_if_noncritical(
        persist_end_of_season_news(&db.conn, &result, season.numero),
        "Falha ao persistir noticias de fim de temporada",
    );
    let total_races = count_season_calendar_entries(&db.conn, &result.new_season_id)
        .map_err(|e| format!("Falha ao contar corridas da nova temporada: {e}"))?;
    let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    meta.current_season = (season.numero + 1).max(1) as u32;
    meta.current_year = result.new_year.max(0) as u32;
    meta.last_played = now;
    meta.total_races = total_races;
    warn_if_noncritical(
        write_save_meta(&meta_path, &meta),
        "Falha ao atualizar meta.json apos avancar temporada",
    );

    config.last_career = Some(career_number);
    warn_if_noncritical(
        config
            .save()
            .map_err(|e| format!("Falha ao atualizar config do app: {e}")),
        "Falha ao atualizar config do app apos avancar temporada",
    );

    warn_if_noncritical(
        write_resume_context(
            &career_dir,
            &CareerResumeContext {
                active_view: CareerResumeView::EndOfSeason,
                end_of_season_result: Some(result.clone()),
            },
        ),
        "Falha ao persistir resume_context apos avancar temporada",
    );

    Ok(result)
}

/// Simula todas as corridas pendentes da temporada sem participação do jogador,
/// conduzindo a temporada por todas as fases: BlocoRegular → JanelaConvocacao →
/// BlocoEspecial → PosEspecial. Após esta função, advance_season pode ser chamado.
/// Usado quando o jogador está sem equipe e quer pular para a próxima pré-temporada.
pub(crate) fn skip_all_pending_races_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<(), String> {
    let config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(career_id);
    let db_path = career_dir.join("career.db");
    let mut db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;

    {
        let season = season_queries::get_active_season(&db.conn)
            .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
            .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

        if season.fase == SeasonPhase::Temporada {
            let pending = calendar_queries::get_pending_races(&db.conn, &season.id)
                .map_err(|e| format!("Falha ao buscar corridas pendentes: {e}"))?;
            for race in &pending {
                crate::commands::race::simulate_category_race(&mut db, race, false)?;
            }
            season_queries::move_to_encerramento_if_completed(&db.conn, &season)
                .map_err(|e| format!("Falha ao encerrar temporada 9D: {e}"))?;
            return Ok(());
        }
    }

    // ── Fase 1: BlocoRegular ─────────────────────────────────────────────────
    {
        let season = season_queries::get_active_season(&db.conn)
            .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
            .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

        if season.fase == SeasonPhase::BlocoRegular {
            let pending = calendar_queries::get_pending_races(&db.conn, &season.id)
                .map_err(|e| format!("Falha ao buscar corridas pendentes: {e}"))?;
            for race in &pending {
                crate::commands::race::simulate_category_race(&mut db, race, false)?;
            }
            crate::convocation::advance_to_convocation_window(&db.conn)
                .map_err(|e| format!("Falha ao avancar para janela de convocacao: {e}"))?;
        }
    }

    // ── Fase 2: JanelaConvocacao ─────────────────────────────────────────────
    {
        let season = season_queries::get_active_season(&db.conn)
            .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
            .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

        if season.fase == SeasonPhase::JanelaConvocacao {
            crate::convocation::run_convocation_window(&db.conn)
                .map_err(|e| format!("Falha ao executar janela de convocacao: {e}"))?;
            crate::convocation::iniciar_bloco_especial(&db.conn)
                .map_err(|e| format!("Falha ao iniciar bloco especial: {e}"))?;
        }
    }

    // ── Fase 3: BlocoEspecial ────────────────────────────────────────────────
    {
        let season = season_queries::get_active_season(&db.conn)
            .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
            .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

        if season.fase == SeasonPhase::BlocoEspecial {
            let player = driver_queries::get_player_driver(&db.conn)
                .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
            if player.categoria_especial_ativa.is_some() {
                return Err(
                    "O jogador participa do bloco especial ativo e deve correr essa fase normalmente."
                        .to_string(),
                );
            }

            for category_id in ["production_challenger", "endurance"] {
                let pending = calendar_queries::get_pending_races_for_category(
                    &db.conn,
                    &season.id,
                    category_id,
                )
                .map_err(|e| {
                    format!("Falha ao buscar corridas pendentes de {}: {e}", category_id)
                })?;
                for race in &pending {
                    crate::commands::race::simulate_category_race(&mut db, race, false)?;
                }
            }

            crate::convocation::encerrar_bloco_especial(&db.conn)
                .map_err(|e| format!("Falha ao encerrar bloco especial: {e}"))?;
            crate::convocation::run_pos_especial(&db.conn)
                .map_err(|e| format!("Falha ao executar pos-especial: {e}"))?;
        }
    }

    Ok(())
}

pub(crate) fn persist_end_of_season_news(
    _conn: &rusqlite::Connection,
    _result: &EndOfSeasonResult,
    _season_number: i32,
) -> Result<(), String> {
    Ok(())
}

pub(crate) fn count_season_calendar_entries(
    conn: &rusqlite::Connection,
    season_id: &str,
) -> Result<i32, rusqlite::Error> {
    conn.query_row(
        "SELECT COUNT(*) FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1",
        rusqlite::params![season_id],
        |row| row.get(0),
    )
}

pub(crate) fn cleanup_legacy_special_state_for_9d_transition(
    conn: &rusqlite::Connection,
    season_number: i32,
) -> Result<(), String> {
    conn.execute(
        "UPDATE contracts
         SET status = 'Expirado'
         WHERE tipo = 'Especial'
           AND status = 'Ativo'
           AND temporada_inicio = ?1",
        rusqlite::params![season_number],
    )
    .map_err(|e| format!("Falha ao expirar contratos especiais legados: {e}"))?;

    conn.execute(
        "UPDATE drivers
         SET categoria_especial_ativa = NULL
         WHERE categoria_especial_ativa IS NOT NULL",
        [],
    )
    .map_err(|e| format!("Falha ao limpar categoria especial ativa legada: {e}"))?;

    Ok(())
}
