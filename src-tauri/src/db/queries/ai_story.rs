//! Cache do boletim de IA por notícia de corrida.
//!
//! Guarda os FATOS curados (gerados no fim da corrida, quando o `RaceResult`
//! está em mãos) e o BOLETIM já redigido pelo servidor (preenchido sob demanda,
//! no 1º abrir da notícia). A tabela é criada de forma idempotente — não depende
//! do sistema de migrações versionadas.

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;

fn ensure_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ai_race_story (
            news_id    TEXT PRIMARY KEY,
            facts      TEXT NOT NULL,
            story      TEXT,
            created_at TEXT NOT NULL DEFAULT ''
        );",
    )?;
    // `teams_json` (mapa nome da equipe → cor primária, p/ o front colorir os nomes
    // no boletim) foi adicionada depois do schema inicial. ALTER idempotente: em DBs
    // que já têm a coluna o erro "duplicate column" é ignorado de propósito.
    let _ = conn.execute("ALTER TABLE ai_race_story ADD COLUMN teams_json TEXT", []);
    Ok(())
}

/// Fatos curados + (talvez) o boletim já gerado + cores das equipes da corrida.
pub struct AiStoryRow {
    pub facts: String,
    pub story: Option<String>,
    pub teams_json: Option<String>,
}

/// Guarda os fatos curados de uma corrida, atrelados à notícia de Corrida.
/// Chamado no fim da corrida do jogador. Sobrescreve se já existir.
/// `teams_json` é o mapa JSON nome→cor das equipes participantes (pode ser "{}").
pub fn store_race_facts(
    conn: &Connection,
    news_id: &str,
    facts: &str,
    teams_json: &str,
) -> Result<(), DbError> {
    ensure_table(conn)?;
    let now = chrono::Local::now().timestamp().to_string();
    conn.execute(
        "INSERT OR REPLACE INTO ai_race_story (news_id, facts, story, created_at, teams_json)
         VALUES (?1, ?2, NULL, ?3, ?4)",
        params![news_id, facts, now, teams_json],
    )?;
    Ok(())
}

/// Lê os fatos (e o boletim em cache, se houver) de uma notícia.
pub fn get_story(conn: &Connection, news_id: &str) -> Result<Option<AiStoryRow>, DbError> {
    ensure_table(conn)?;
    let row = conn
        .query_row(
            "SELECT facts, story, teams_json FROM ai_race_story WHERE news_id = ?1",
            params![news_id],
            |r| {
                Ok(AiStoryRow {
                    facts: r.get(0)?,
                    story: r.get(1)?,
                    teams_json: r.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

/// Salva o boletim gerado pelo servidor (cache), para não regenerar e não
/// esbarrar no cooldown ao reabrir a mesma notícia.
pub fn set_story(conn: &Connection, news_id: &str, story: &str) -> Result<(), DbError> {
    ensure_table(conn)?;
    conn.execute(
        "UPDATE ai_race_story SET story = ?1 WHERE news_id = ?2",
        params![story, news_id],
    )?;
    Ok(())
}
