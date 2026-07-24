//! A casca `#[tauri::command]`: resolve base_dir/config, cacheia e chama o servidor.

use super::*;

#[tauri::command]
pub async fn enrich_race_news_ai(
    app: tauri::AppHandle,
    career_id: String,
    news_id: String,
    reading_seconds: Option<f64>,
) -> Result<AiNewsResult, String> {
    // async + spawn_blocking: o fetch de IA é bloqueante (até 45s) e travaria a THREAD
    // PRINCIPAL do Tauri se rodasse síncrono — a janela inteira congela ("não está
    // respondendo") enquanto o servidor gera.
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

        // Idioma + install_id vêm do config do app (get_or_create persiste o id).
        let mut config = AppConfig::load_or_default(&base_dir);
        let install_id = config.get_or_create_install_id();
        let lang = config.language.clone();

        let db_path = config.saves_dir().join(&career_id).join("career.db");
        let db =
            Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

        let row = ai_story::get_story(&db.conn, &news_id)
            .map_err(|e| format!("Falha ao ler cache do boletim: {e:?}"))?;

        // Sem fatos guardados → não há boletim de IA para esta notícia (ex.: corrida
        // antiga, ou notícia que não é a do jogador). Front usa o template.
        let Some(row) = row else {
            return Ok(AiNewsResult {
                story: None,
                status: "unavailable".to_string(),
                teams: None,
            });
        };

        // Cores das equipes da corrida (mapa nome→cor). Acompanha story em todo retorno.
        let teams = row
            .teams_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());

        // Já gerado antes → devolve do cache (instantâneo, sem tocar no servidor).
        if let Some(story) = row.story {
            return Ok(AiNewsResult {
                story: Some(story),
                status: "cached".to_string(),
                teams,
            });
        }

        // 1ª vez: gera no servidor e cacheia.
        match client::fetch_story(&row.facts, &lang, &install_id, reading_seconds) {
            Ok(story) => {
                if let Err(e) = ai_story::set_story(&db.conn, &news_id, &story) {
                    eprintln!("[narrative] Falha ao cachear boletim: {e:?}");
                }
                Ok(AiNewsResult {
                    story: Some(story),
                    status: "ok".to_string(),
                    teams,
                })
            }
            Err(StoryError::RateLimited) => Ok(AiNewsResult {
                story: None,
                status: "rate_limited".to_string(),
                teams,
            }),
            Err(_) => Ok(AiNewsResult {
                story: None,
                status: "error".to_string(),
                teams,
            }),
        }
    })
    .await
    .map_err(|e| format!("Falha ao executar boletim de IA: {e}"))?
}

/// O front monta os `facts` do briefing e chama isto ao abrir a tela. Cacheia por
/// `race_id` (reentrar não regenera). Em cooldown/rede/cota → textos `None` e o front
/// usa o template. O cooldown de 10 min entre prévias é imposto pelo servidor
/// (escopo "pre-race", separado do boletim pós-corrida).
#[tauri::command]
pub async fn pre_race_briefing_ai(
    app: tauri::AppHandle,
    career_id: String,
    race_id: String,
    facts: String,
    force: Option<bool>,
) -> Result<PreRaceAiResult, String> {
    // async + spawn_blocking: ver enrich_race_news_ai — fetch bloqueante fora da main.
    tauri::async_runtime::spawn_blocking(move || {
        let force = force.unwrap_or(false);
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

        // Cache por etapa → reentrar na tela não regenera (sem custo, sem cooldown).
        // No reroll de debug (force) ignoramos o cache e regeramos pelo servidor.
        if !force {
            if let Ok(Some(row)) = ai_pre_race::get_pre_race(&db.conn, &race_id) {
                return Ok(PreRaceAiResult {
                    headline: Some(row.headline),
                    narrative: Some(row.narrative),
                    team_voice: Some(row.team_voice),
                    status: "cached".to_string(),
                });
            }

            // Gate de engajamento: se o jogador não vem lendo a prévia, segura/alterna no
            // template para não gastar IA (sem tocar no servidor). O reroll (force) pula.
            let streak = meta::get_meta_value(&db.conn, PRE_RACE_STREAK_KEY)
                .ok()
                .flatten()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            if !pre_race_use_ai(streak) {
                return Ok(PreRaceAiResult {
                    headline: None,
                    narrative: None,
                    team_voice: None,
                    status: "engagement_template".to_string(),
                });
            }
        }

        if facts.trim().is_empty() {
            return Ok(PreRaceAiResult {
                headline: None,
                narrative: None,
                team_voice: None,
                status: "unavailable".to_string(),
            });
        }

        // Arco narrativo entre etapas: anexa a memória das últimas corridas (chegada +
        // manchete do debrief, e o corpo do debrief anterior) para o engenheiro retomar de
        // onde parou. Vazio na 1ª corrida da carreira/categoria → briefing inalterado.
        let facts = {
            let arc = build_recent_arc_facts(&db.conn, &race_id);
            if arc.trim().is_empty() {
                facts
            } else {
                format!("{facts}\n{arc}")
            }
        };

        match client::fetch_pre_race_briefing(&facts, &lang, &install_id, force) {
            Ok(b) => {
                // O corpo cinematográfico é guardado na coluna `narrative` do cache.
                if let Err(e) = ai_pre_race::set_pre_race(
                    &db.conn,
                    &race_id,
                    &b.headline,
                    &b.body,
                    &b.team_voice,
                ) {
                    eprintln!("[narrative] Falha ao cachear prévia pré-corrida: {e:?}");
                }
                Ok(PreRaceAiResult {
                    headline: Some(b.headline),
                    narrative: Some(b.body),
                    team_voice: Some(b.team_voice),
                    status: "ok".to_string(),
                })
            }
            Err(StoryError::RateLimited) => Ok(PreRaceAiResult {
                headline: None,
                narrative: None,
                team_voice: None,
                status: "rate_limited".to_string(),
            }),
            Err(_) => Ok(PreRaceAiResult {
                headline: None,
                narrative: None,
                team_voice: None,
                status: "error".to_string(),
            }),
        }
    })
    .await
    .map_err(|e| format!("Falha ao executar prévia pré-corrida: {e}"))?
}

/// Reporta se o jogador LEU a prévia pré-corrida (ficou tempo suficiente na Sala de
/// Estratégia) e atualiza a sequência de "não-leu" no `meta`. Leu → zera; não leu →
/// +1 (limitado). O front chama isto ao simular/sair da tela. Devolve a sequência
/// nova (útil só para debug). Falha de IO vira erro string, mas o front ignora.
#[tauri::command]
pub fn report_pre_race_engagement(
    app: tauri::AppHandle,
    career_id: String,
    read: bool,
) -> Result<i64, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db =
        Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let streak = meta::get_meta_value(&db.conn, PRE_RACE_STREAK_KEY)
        .ok()
        .flatten()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let next = if read { 0 } else { (streak + 1).min(10) };
    meta::put_meta_value(&db.conn, PRE_RACE_STREAK_KEY, &next.to_string())
        .map_err(|e| format!("Falha ao gravar engajamento: {e:?}"))?;
    Ok(next)
}

/// Comando lazy do debrief pós-corrida: o front chama quando o jogador abre a aba
/// Debrief. Monta os fatos no Rust, manda ao servidor (voz única do engenheiro) e
/// cacheia por `race_id` — reabrir não regenera. Sem gate de engajamento (o jogador
/// sempre olha o resultado). Em qualquer falha devolve `None` e o front usa o
/// texto determinístico do cérebro (nunca quebra).
#[tauri::command]
pub async fn post_race_debrief_ai(
    app: tauri::AppHandle,
    career_id: String,
    race_id: String,
    force: Option<bool>,
) -> Result<PostRaceAiResult, String> {
    // async + spawn_blocking: ver enrich_race_news_ai. Este é o pior caso — a tela de
    // resultado chama assim que abre, e com o engenheiro "no rádio" o jogador ficava
    // olhando a janela congelada até o servidor responder.
    tauri::async_runtime::spawn_blocking(move || {
        let force = force.unwrap_or(false);
        let base_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

        let mut config = AppConfig::load_or_default(&base_dir);
        let install_id = config.get_or_create_install_id();
        let lang = config.language.clone();

        let career_dir = config.saves_dir().join(&career_id);
        let db_path = career_dir.join("career.db");
        let db =
            Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

        if !force {
            if let Ok(Some(row)) = ai_post_race::get_post_race(&db.conn, &race_id) {
                return Ok(PostRaceAiResult {
                    headline: Some(row.headline),
                    body: Some(row.body),
                    status: "cached".to_string(),
                });
            }
        }

        let facts = build_post_race_facts(&db.conn, &career_dir, &race_id);
        if facts.trim().is_empty() {
            return Ok(PostRaceAiResult {
                headline: None,
                body: None,
                status: "unavailable".to_string(),
            });
        }

        match client::fetch_post_race_debrief(&facts, &lang, &install_id, force) {
            Ok(d) => {
                if let Err(e) =
                    ai_post_race::set_post_race(&db.conn, &race_id, &d.headline, &d.body)
                {
                    eprintln!("[narrative] Falha ao cachear debrief pós-corrida: {e:?}");
                }
                Ok(PostRaceAiResult {
                    headline: Some(d.headline),
                    body: Some(d.body),
                    status: "ok".to_string(),
                })
            }
            Err(StoryError::RateLimited) => Ok(PostRaceAiResult {
                headline: None,
                body: None,
                status: "rate_limited".to_string(),
            }),
            Err(_) => Ok(PostRaceAiResult {
                headline: None,
                body: None,
                status: "error".to_string(),
            }),
        }
    })
    .await
    .map_err(|e| format!("Falha ao executar debrief pós-corrida: {e}"))?
}

/// Resolve o `news_id` da notícia de Corrida do JOGADOR para uma (temporada, rodada).
///
/// A revista (NewsMagazineTab) monta as edições a partir do calendário; para puxar o
/// boletim de IA de cada etapa ela precisa do `news_id` correspondente. Como os fatos
/// de IA só são guardados para a corrida do jogador (uma por rodada), basta cruzar
/// `ai_race_story` com `news` pela rodada da temporada. Devolve `None` quando não há
/// boletim para a etapa (ex.: corrida simulada antes do recurso existir) → o front
/// usa o texto-placeholder.
#[tauri::command]
pub fn player_race_news_id(
    app: tauri::AppHandle,
    career_id: String,
    season_id: String,
    rodada: i32,
) -> Result<Option<String>, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db =
        Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    // Se a tabela ai_race_story ainda não existe (save sem nenhum boletim), a query
    // falha — tratamos como "sem boletim" em vez de erro.
    let id = db
        .conn
        .query_row(
            "SELECT n.id
               FROM news n
               JOIN ai_race_story a ON a.news_id = n.id
              WHERE n.temporada_id = ?1 AND n.rodada = ?2
              LIMIT 1",
            rusqlite::params![season_id, rodada],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .unwrap_or(None);

    Ok(id)
}
