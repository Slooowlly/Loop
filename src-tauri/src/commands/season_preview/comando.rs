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
    /// ok | cached | unavailable | backoff | rate_limited | error
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

/// Janela de silêncio depois de uma tentativa FALHA de gerar a matéria. O template não é
/// cacheado de propósito (uma próxima abertura ainda pode conseguir a IA), mas sem freio
/// isso vira uma requisição de rede — de até 45s — a cada vez que a revista abre, e num
/// save em cooldown ela falha sempre. Dentro da janela devolvemos o determinístico na hora.
pub(super) const RETRY_BACKOFF_SECS: i64 = 15 * 60;

/// Mapa nome da equipe → cor guardado junto da matéria. Sem ele (linha antiga ou mapa
/// vazio) o front simplesmente não colore os nomes citados.
pub(super) fn cached_teams(row: &ai_story::AiStoryRow) -> Option<serde_json::Value> {
    let raw = row.teams_json.as_deref()?;
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    match value.as_object() {
        Some(m) if !m.is_empty() => Some(value),
        _ => None,
    }
}

/// A linha foi gravada dentro da janela de backoff? `created_at` é o timestamp Unix em
/// texto; linha sem carimbo legível conta como antiga (tenta de novo).
pub(super) fn recent_attempt(row: &ai_story::AiStoryRow) -> bool {
    let Ok(at) = row.created_at.parse::<i64>() else {
        return false;
    };
    let agora = chrono::Local::now().timestamp();
    agora >= at && agora - at < RETRY_BACKOFF_SECS
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
        gerar_preview(&base_dir, &career_id)
    })
    .await
    .map_err(|e| format!("Falha ao executar matéria de pré-temporada: {e}"))?
}

/// Pré-gera a matéria em BACKGROUND ao abrir a carreira, para a revista já abrir com o
/// texto pronto — o mesmo que `spawn_prewarm_boletim` faz com o boletim da corrida.
///
/// Só vale na PRÉ-TEMPORADA: passada a 1ª etapa a revista troca a matéria de expectativas
/// pela edição da corrida, e gerar aqui queimaria cota num texto que ninguém veria. Como
/// passa pelo mesmo `gerar_preview`, o cache e o backoff de tentativa falha valem igual —
/// entrar e sair do save não regenera nem fica batendo no servidor.
pub(crate) fn spawn_prewarm_season_preview(base_dir: std::path::PathBuf, career_id: String) {
    std::thread::spawn(move || {
        let _ = gerar_preview(&base_dir, &career_id);
    });
}

/// O corpo do comando, síncrono. Vive separado porque o pré-aquecimento roda no mesmo
/// caminho, só que numa thread própria e descartando o resultado.
fn gerar_preview(
    base_dir: &std::path::Path,
    career_id: &str,
) -> Result<SeasonPreviewResult, String> {
    {
        let mut config = AppConfig::load_or_default(base_dir);
        let install_id = config.get_or_create_install_id();
        let lang = config.language.clone();

        let db_path = config.saves_dir().join(career_id).join("career.db");
        let db =
            Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

        // A chave do cache sai de duas leituras baratas — de propósito, ANTES de montar os
        // fatos: `build_preview_data` varre o grid inteiro e busca o contrato de cada piloto,
        // e num cache hit esse trabalho todo seria jogado fora.
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

        let cached_row = ai_story::get_story(&db.conn, &cache_key).ok().flatten();

        // Cache → reabrir a revista não regenera (sem custo, sem cooldown). As cores das
        // equipes vêm da própria linha, então nem os fatos precisam ser montados.
        if let Some(row) = &cached_row {
            if let Some(raw) = &row.story {
                if let Ok(c) = serde_json::from_str::<CachedPreview>(raw) {
                    return Ok(SeasonPreviewResult {
                        headline: Some(c.headline),
                        standfirst: Some(c.standfirst),
                        body: Some(c.body),
                        source: "ai".to_string(),
                        status: "cached".to_string(),
                        teams: cached_teams(row),
                    });
                }
            }
        }

        let Some(data) = build_preview_data(&db.conn, base_dir, career_id) else {
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

        // Tentativa falha recente (linha sem `story`): devolve o determinístico sem tocar
        // na rede. Passada a janela, a próxima abertura tenta a IA de novo.
        if let Some(row) = &cached_row {
            if row.story.is_none() && recent_attempt(row) {
                let fb = deterministic_article(&data);
                return Ok(SeasonPreviewResult {
                    headline: Some(fb.headline),
                    standfirst: Some(fb.standfirst),
                    body: Some(fb.body),
                    source: "template".to_string(),
                    status: "backoff".to_string(),
                    teams,
                });
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
                // Marca a tentativa (linha com `story` NULL) para o backoff acima ter uma
                // referência de tempo na próxima abertura.
                let teams_json = serde_json::to_string(&data.teams).unwrap_or_else(|_| "{}".into());
                if let Err(e) =
                    ai_story::store_race_facts(&db.conn, &cache_key, &data.facts, &teams_json)
                {
                    eprintln!("[season-preview] Falha ao marcar tentativa: {e:?}");
                }
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
    }
}
