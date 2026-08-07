//! Os comandos Tauri do rodapé: o template determinístico e a reescrita por IA.

use super::*;

/// Comando: notinhas do rodapé de notícias do mundo (determinístico). O front chama ao
/// abrir a revista e renderiza entre o boletim e o rodapé GRID·MAGAZINE. Nunca quebra:
/// em qualquer falha devolve lista vazia (o rodapé simplesmente não aparece).
#[tauri::command]
pub fn get_world_footer(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<WorldFooterResult, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let notes = collect_world_notes(&db.conn);
    let facts = notes
        .iter()
        .map(|n| format!("[{}] {} — {}", n.kind, n.subject, n.text))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(WorldFooterResult {
        notes,
        source: "template".to_string(),
        facts,
    })
}

/// Aplica as reescritas da IA sobre as notas determinísticas, casando 1-para-1 por
/// índice (o servidor devolve uma string por fato, na mesma ordem). Só substitui se a
/// contagem bate EXATAMENTE e nenhuma vem vazia — senão o alinhamento estaria quebrado
/// e é mais seguro manter o template. Pura e testável.
pub(super) fn apply_ai_texts(mut notes: Vec<WorldNote>, ai: &[String]) -> Option<Vec<WorldNote>> {
    if notes.is_empty() || ai.len() != notes.len() {
        return None;
    }
    for (n, text) in notes.iter_mut().zip(ai.iter()) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        n.text = trimmed.to_string();
    }
    Some(notes)
}

/// Comando: reescrita por IA do rodapé "Do mundo do Grid". O front chama DEPOIS de já
/// ter mostrado o template (`get_world_footer`) e troca as notas quando a IA chega —
/// sem bloquear a abertura da revista. Cacheado por `temporada:rodada`. Em QUALQUER
/// falha (inclusive o endpoint `/world-notes` ainda não existir no servidor) devolve
/// `notes: None` e o front simplesmente mantém o texto determinístico.
#[tauri::command]
pub async fn enrich_world_footer_ai(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<WorldFooterAiResult, String> {
    // async + spawn_blocking: o fetch de IA é bloqueante (até 45s) e travaria a THREAD
    // PRINCIPAL do Tauri se rodasse síncrono. Ver enrich_race_news_ai.
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
        let mut config = AppConfig::load_or_default(&base_dir);
        let install_id = config.get_or_create_install_id();
        let lang = config.language.clone();
        let db_path = config.saves_dir().join(&career_id).join("career.db");
        let db =
            Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

        // Chave de cache: temporada:rodada (as notas mudam de estado a cada rodada).
        let (season_num, rodada) = crate::db::queries::seasons::get_active_season(&db.conn)
            .ok()
            .flatten()
            .map(|s| (s.numero, s.rodada_atual))
            .unwrap_or((0, 0));
        let cache_key = format!("{season_num}:{rodada}");

        // Cache → reabrir a revista não regenera.
        if let Ok(Some(json)) = crate::db::queries::ai_world_notes::get_cached(&db.conn, &cache_key)
        {
            if let Ok(notes) = serde_json::from_str::<Vec<WorldNote>>(&json) {
                return Ok(WorldFooterAiResult {
                    notes: Some(notes),
                    source: "ai".to_string(),
                    status: "cached".to_string(),
                });
            }
        }

        // Antes de reconstruir as notas (varredura do grid): espera uma geração desta
        // rodada que já esteja em voo e relê — ver `narrative::em_voo`. A revista dispara
        // isto a cada abertura, então duas aberturas seguidas colidiam.
        let _passe = crate::narrative::em_voo::aguardar_vez(
            crate::narrative::em_voo::chave_rodape(&career_id, &cache_key),
        );
        if let Ok(Some(json)) = crate::db::queries::ai_world_notes::get_cached(&db.conn, &cache_key)
        {
            if let Ok(notes) = serde_json::from_str::<Vec<WorldNote>>(&json) {
                return Ok(WorldFooterAiResult {
                    notes: Some(notes),
                    source: "ai".to_string(),
                    status: "cached".to_string(),
                });
            }
        }

        // Reconstrói as notas determinísticas + os fatos (MESMA ordem do get_world_footer).
        let notes = collect_world_notes(&db.conn);
        if notes.is_empty() {
            return Ok(WorldFooterAiResult {
                notes: None,
                source: "template".to_string(),
                status: "unavailable".to_string(),
            });
        }
        let facts = notes
            .iter()
            .map(|n| format!("[{}] {} — {}", n.kind, n.subject, n.text))
            .collect::<Vec<_>>()
            .join("\n");

        match crate::narrative::client::fetch_world_notes(&facts, &lang, &install_id) {
            Ok(ai) => match apply_ai_texts(notes, &ai) {
                Some(enriched) => {
                    if let Ok(json) = serde_json::to_string(&enriched) {
                        let _ = crate::db::queries::ai_world_notes::set_cached(
                            &db.conn, &cache_key, &json,
                        );
                    }
                    Ok(WorldFooterAiResult {
                        notes: Some(enriched),
                        source: "ai".to_string(),
                        status: "ok".to_string(),
                    })
                }
                None => Ok(WorldFooterAiResult {
                    notes: None,
                    source: "template".to_string(),
                    status: "mismatch".to_string(),
                }),
            },
            Err(crate::narrative::client::StoryError::RateLimited) => Ok(WorldFooterAiResult {
                notes: None,
                source: "template".to_string(),
                status: "rate_limited".to_string(),
            }),
            Err(_) => Ok(WorldFooterAiResult {
                notes: None,
                source: "template".to_string(),
                status: "error".to_string(),
            }),
        }
    })
    .await
    .map_err(|e| format!("Falha ao executar rodapé do mundo: {e}"))?
}
