//! O comando Tauri da matéria: cache por temporada+categoria, IA e queda para o template.

use super::*;

// ── Comando ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SeasonPreviewResult {
    pub headline: Option<String>,
    pub standfirst: Option<String>,
    pub body: Option<String>,
    /// "ai" | "template" — de onde veio o texto exibido.
    pub source: String,
    /// ok | cached | unavailable | rate_limited | error
    pub status: String,
    /// Mapa nome da equipe → cor (para o front colorir os nomes citados).
    pub teams: Option<serde_json::Value>,
}

/// Serializa/desserializa o trio no campo `story` do cache (evita nova tabela).
#[derive(serde::Serialize, serde::Deserialize)]
struct CachedPreview {
    headline: String,
    standfirst: String,
    body: String,
}

/// Comando: matéria "O Que Esperar" da temporada. O front chama ao abrir a revista
/// enquanto não há edição de corrida. Cacheia por temporada+categoria. Se a IA falhar,
/// devolve a versão determinística (`source: "template"`) — a aba nunca fica vazia.
#[tauri::command]
pub async fn enrich_season_preview_ai(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<SeasonPreviewResult, String> {
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

        let Some(data) = build_preview_data(&db.conn, &base_dir, &career_id) else {
            return Ok(SeasonPreviewResult {
                headline: None,
                standfirst: None,
                body: None,
                source: "template".to_string(),
                status: "unavailable".to_string(),
                teams: None,
            });
        };

        let teams = if data.teams.as_object().map_or(true, |m| m.is_empty()) {
            None
        } else {
            Some(data.teams.clone())
        };

        let season_id = crate::db::queries::seasons::get_active_season(&db.conn)
            .ok()
            .flatten()
            .map(|s| s.id)
            .unwrap_or_default();
        let category = crate::db::queries::drivers::get_player_driver(&db.conn)
            .ok()
            .map(|p| player_category(&db.conn, &p))
            .unwrap_or_default();
        let cache_key = format!("season-preview:{season_id}:{category}");

        // Cache → reabrir a revista não regenera (sem custo, sem cooldown).
        if let Ok(Some(row)) = ai_story::get_story(&db.conn, &cache_key) {
            if let Some(raw) = row.story {
                if let Ok(c) = serde_json::from_str::<CachedPreview>(&raw) {
                    return Ok(SeasonPreviewResult {
                        headline: Some(c.headline),
                        standfirst: Some(c.standfirst),
                        body: Some(c.body),
                        source: "ai".to_string(),
                        status: "cached".to_string(),
                        teams,
                    });
                }
            }
        }

        // 1ª vez: gera no servidor. Em QUALQUER falha cai no determinístico (que NÃO é
        // cacheado — assim uma próxima abertura ainda pode conseguir a versão de IA).
        match client::fetch_season_preview(&data.facts, &lang, &install_id) {
            Ok(p) => {
                let cached = CachedPreview {
                    headline: p.headline.clone(),
                    standfirst: p.standfirst.clone(),
                    body: p.body.clone(),
                };
                if let Ok(json) = serde_json::to_string(&cached) {
                    let teams_json =
                        serde_json::to_string(&data.teams).unwrap_or_else(|_| "{}".into());
                    if let Err(e) =
                        ai_story::store_race_facts(&db.conn, &cache_key, &data.facts, &teams_json)
                    {
                        eprintln!("[season-preview] Falha ao guardar fatos: {e:?}");
                    }
                    if let Err(e) = ai_story::set_story(&db.conn, &cache_key, &json) {
                        eprintln!("[season-preview] Falha ao cachear matéria: {e:?}");
                    }
                }
                Ok(SeasonPreviewResult {
                    headline: Some(p.headline),
                    standfirst: Some(p.standfirst),
                    body: Some(p.body),
                    source: "ai".to_string(),
                    status: "ok".to_string(),
                    teams,
                })
            }
            Err(err) => {
                let fb = deterministic_article(&data);
                Ok(SeasonPreviewResult {
                    headline: Some(fb.headline),
                    standfirst: Some(fb.standfirst),
                    body: Some(fb.body),
                    source: "template".to_string(),
                    status: match err {
                        StoryError::RateLimited => "rate_limited".to_string(),
                        _ => "error".to_string(),
                    },
                    teams,
                })
            }
        }
    })
    .await
    .map_err(|e| format!("Falha ao executar matéria de pré-temporada: {e}"))?
}
