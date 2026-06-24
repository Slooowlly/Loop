//! Comando lazy do boletim de IA.
//!
//! Quando o jogador ABRE uma notícia de corrida, o front chama este comando. Ele
//! lê os fatos curados guardados no fim da corrida, manda ao servidor (que chama
//! o Gemini) e devolve o boletim — cacheando o resultado. Em qualquer falha,
//! devolve `story: None` e o front cai no texto-template padrão (nunca quebra).

use rusqlite::OptionalExtension;
use serde::Serialize;
use tauri::Manager;

use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::db::queries::{ai_pre_race, ai_story};
use crate::narrative::client::{self, StoryError};

#[derive(Serialize)]
pub struct AiNewsResult {
    /// O boletim redigido, se disponível. `None` → front usa o texto padrão.
    pub story: Option<String>,
    /// ok | cached | unavailable | rate_limited | error
    pub status: String,
    /// Mapa nome da equipe → cor primária das equipes da corrida (p/ colorir os
    /// nomes no boletim). `None` se a notícia não tem fatos de IA.
    pub teams: Option<serde_json::Value>,
}

#[tauri::command]
pub fn enrich_race_news_ai(
    app: tauri::AppHandle,
    career_id: String,
    news_id: String,
    reading_seconds: Option<f64>,
) -> Result<AiNewsResult, String> {
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
}

/// Resultado da prévia pré-corrida por IA (Sala de Estratégia). `None` nos textos →
/// o front cai no template atual (narrativa + voz da equipe geradas localmente).
#[derive(Serialize)]
pub struct PreRaceAiResult {
    pub narrative: Option<String>,
    pub team_voice: Option<String>,
    /// ok | cached | rate_limited | unavailable | error
    pub status: String,
}

/// Prévia pré-corrida (narrativa + voz da equipe, CURTAS) para a Sala de Estratégia.
/// O front monta os `facts` do briefing e chama isto ao abrir a tela. Cacheia por
/// `race_id` (reentrar não regenera). Em cooldown/rede/cota → textos `None` e o front
/// usa o template. O cooldown de 10 min entre prévias é imposto pelo servidor
/// (escopo "pre-race", separado do boletim pós-corrida).
#[tauri::command]
pub fn pre_race_briefing_ai(
    app: tauri::AppHandle,
    career_id: String,
    race_id: String,
    facts: String,
) -> Result<PreRaceAiResult, String> {
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
    if let Ok(Some(row)) = ai_pre_race::get_pre_race(&db.conn, &race_id) {
        return Ok(PreRaceAiResult {
            narrative: Some(row.narrative),
            team_voice: Some(row.team_voice),
            status: "cached".to_string(),
        });
    }

    if facts.trim().is_empty() {
        return Ok(PreRaceAiResult {
            narrative: None,
            team_voice: None,
            status: "unavailable".to_string(),
        });
    }

    match client::fetch_pre_race_briefing(&facts, &lang, &install_id) {
        Ok(b) => {
            if let Err(e) =
                ai_pre_race::set_pre_race(&db.conn, &race_id, &b.narrative, &b.team_voice)
            {
                eprintln!("[narrative] Falha ao cachear prévia pré-corrida: {e:?}");
            }
            Ok(PreRaceAiResult {
                narrative: Some(b.narrative),
                team_voice: Some(b.team_voice),
                status: "ok".to_string(),
            })
        }
        Err(StoryError::RateLimited) => Ok(PreRaceAiResult {
            narrative: None,
            team_voice: None,
            status: "rate_limited".to_string(),
        }),
        Err(_) => Ok(PreRaceAiResult {
            narrative: None,
            team_voice: None,
            status: "error".to_string(),
        }),
    }
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
