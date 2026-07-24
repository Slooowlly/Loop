//! Persistencia dos arquivos auxiliares do save: meta, contexto de retomada e historico de frases do briefing.

use super::*;

pub(crate) fn read_save_meta(path: &Path) -> Result<SaveMeta, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Falha ao ler meta.json: {e}"))?;
    serde_json::from_str::<SaveMeta>(&content)
        .map_err(|e| format!("Falha ao parsear meta.json: {e}"))
}

pub(crate) fn resume_context_path(career_dir: &Path) -> PathBuf {
    career_dir.join("resume_context.json")
}

pub(crate) fn read_resume_context(
    career_dir: &Path,
) -> Result<Option<CareerResumeContext>, String> {
    let path = resume_context_path(career_dir);
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Falha ao ler resume_context.json: {e}"))?;
    let context = serde_json::from_str::<CareerResumeContext>(&content)
        .map_err(|e| format!("Falha ao parsear resume_context.json: {e}"))?;
    normalize_resume_context(career_dir, context)
}

pub(crate) fn normalize_resume_context(
    career_dir: &Path,
    context: CareerResumeContext,
) -> Result<Option<CareerResumeContext>, String> {
    match context.active_view {
        CareerResumeView::Dashboard => Ok(None),
        CareerResumeView::EndOfSeason => {
            if context.end_of_season_result.is_some() {
                Ok(Some(context))
            } else if load_preseason_plan(career_dir)?.is_some() {
                Ok(Some(CareerResumeContext {
                    active_view: CareerResumeView::Preseason,
                    end_of_season_result: None,
                }))
            } else {
                Ok(None)
            }
        }
        CareerResumeView::Preseason => {
            if load_preseason_plan(career_dir)?.is_some() {
                Ok(Some(CareerResumeContext {
                    active_view: CareerResumeView::Preseason,
                    end_of_season_result: None,
                }))
            } else {
                Ok(None)
            }
        }
    }
}

pub(crate) fn write_resume_context(
    career_dir: &Path,
    context: &CareerResumeContext,
) -> Result<(), String> {
    let path = resume_context_path(career_dir);
    let payload = serde_json::to_string_pretty(context)
        .map_err(|e| format!("Falha ao serializar resume_context.json: {e}"))?;
    std::fs::write(&path, payload).map_err(|e| format!("Falha ao gravar resume_context.json: {e}"))
}

pub(crate) fn delete_resume_context(career_dir: &Path) -> Result<(), String> {
    let path = resume_context_path(career_dir);
    if !path.exists() {
        return Ok(());
    }

    std::fs::remove_file(&path).map_err(|e| format!("Falha ao remover resume_context.json: {e}"))
}

pub(crate) fn persist_resume_context_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    active_view: CareerResumeView,
    end_of_season_result: Option<EndOfSeasonResult>,
) -> Result<(), String> {
    // Persiste só estado de UI em JSON (nada de banco). Antes abria com reparo, o que
    // rodava a transação de escrita de contratos a cada troca de tela. Read-only.
    let (_db, career_dir, _) = open_career_resources_read_only(base_dir, career_id)?;

    match active_view {
        CareerResumeView::Dashboard => delete_resume_context(&career_dir),
        CareerResumeView::EndOfSeason => {
            let Some(result) = end_of_season_result else {
                return Err(
                    "Estado de fim de temporada requer payload para restauracao.".to_string(),
                );
            };

            write_resume_context(
                &career_dir,
                &CareerResumeContext {
                    active_view,
                    end_of_season_result: Some(result),
                },
            )
        }
        CareerResumeView::Preseason => write_resume_context(
            &career_dir,
            &CareerResumeContext {
                active_view,
                end_of_season_result: None,
            },
        ),
    }
}

pub(crate) fn get_briefing_phrase_history_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<BriefingPhraseHistory, String> {
    // Só precisamos do career_dir para ler o JSON de frases — nada de banco. Abrir
    // com reparo aqui rodava uma transação de ESCRITA (BEGIN IMMEDIATE + varredura de
    // contratos/equipes) e pegava o lock global a cada carregamento do briefing,
    // brigando com os SELECTs paralelos e travando a Sala de Estratégia. Read-only.
    let (_db, career_dir, _meta) = open_career_resources_read_only(base_dir, career_id)?;
    read_briefing_phrase_history(&briefing_phrase_history_path(&career_dir))
}

pub(crate) fn save_briefing_phrase_history_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    season_number: i32,
    entries: Vec<BriefingPhraseEntryInput>,
) -> Result<BriefingPhraseHistory, String> {
    // Grava só o JSON de frases — não usa o banco. Read-only evita a transação de
    // reparo de contratos (e o lock global) a cada persistência de briefing.
    let (_db, career_dir, _meta) = open_career_resources_read_only(base_dir, career_id)?;
    let history_path = briefing_phrase_history_path(&career_dir);
    let current = read_briefing_phrase_history(&history_path)?;
    let updated = merge_briefing_phrase_history(current, season_number, entries);
    write_briefing_phrase_history(&history_path, &updated)?;
    Ok(updated)
}

pub(crate) fn briefing_phrase_history_path(career_dir: &Path) -> PathBuf {
    career_dir.join("briefing_phrase_history.json")
}

pub(crate) fn read_briefing_phrase_history(path: &Path) -> Result<BriefingPhraseHistory, String> {
    if !path.exists() {
        return Ok(BriefingPhraseHistory::default());
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Falha ao ler briefing_phrase_history.json: {e}"))?;
    serde_json::from_str::<BriefingPhraseHistory>(&content)
        .map_err(|e| format!("Falha ao parsear briefing_phrase_history.json: {e}"))
}

pub(crate) fn write_briefing_phrase_history(
    path: &Path,
    history: &BriefingPhraseHistory,
) -> Result<(), String> {
    let payload = serde_json::to_string_pretty(history)
        .map_err(|e| format!("Falha ao serializar briefing_phrase_history.json: {e}"))?;
    std::fs::write(path, payload)
        .map_err(|e| format!("Falha ao gravar briefing_phrase_history.json: {e}"))
}

pub(crate) fn merge_briefing_phrase_history(
    current: BriefingPhraseHistory,
    season_number: i32,
    entries: Vec<BriefingPhraseEntryInput>,
) -> BriefingPhraseHistory {
    let mut merged_entries = if current.season_number == season_number {
        current.entries
    } else {
        Vec::new()
    };

    for entry in entries {
        merged_entries.retain(|existing| {
            !(existing.round_number == entry.round_number
                && existing.driver_id == entry.driver_id
                && existing.bucket_key == entry.bucket_key)
        });

        merged_entries.push(BriefingPhraseEntry {
            season_number,
            round_number: entry.round_number,
            driver_id: entry.driver_id,
            bucket_key: entry.bucket_key,
            phrase_id: entry.phrase_id,
        });
    }

    merged_entries.sort_by(|left, right| {
        right
            .round_number
            .cmp(&left.round_number)
            .then_with(|| left.driver_id.cmp(&right.driver_id))
            .then_with(|| left.bucket_key.cmp(&right.bucket_key))
    });

    let mut per_bucket_counts: HashMap<(String, String), usize> = HashMap::new();
    merged_entries.retain(|entry| {
        let key = (entry.driver_id.clone(), entry.bucket_key.clone());
        let count = per_bucket_counts.entry(key).or_insert(0);
        if *count >= 5 {
            return false;
        }
        *count += 1;
        true
    });

    BriefingPhraseHistory {
        season_number,
        entries: merged_entries,
    }
}

pub(crate) fn write_save_meta(path: &Path, meta: &SaveMeta) -> Result<(), String> {
    let json = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("Falha ao serializar meta.json: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("Falha ao gravar meta.json: {e}"))
}

pub(crate) fn save_meta_to_info(meta: SaveMeta) -> SaveInfo {
    SaveInfo {
        career_id: format!("career_{:03}", meta.career_number),
        player_name: meta.player_name,
        category_name: categories::get_category_config(&meta.category)
            .map(|category| category.nome.to_string())
            .unwrap_or_else(|| meta.category.clone()),
        category: meta.category,
        season: meta.current_season as i32,
        year: meta.current_year as i32,
        difficulty: meta.difficulty,
        created: meta.created_at,
        last_played: meta.last_played,
        total_races: meta.total_races,
    }
}
