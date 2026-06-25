//! Cache do DEBRIEF pós-corrida gerado por IA (voz do engenheiro → piloto), mostrado
//! na tela de classificação final. Chave = `race_id` (um debrief por etapa). Assim
//! reentrar/reabrir a tela não regenera (sem custo e sem esbarrar no cooldown). A
//! tabela é criada de forma idempotente — não depende do sistema de migrações.

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;

fn ensure_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ai_post_race_debrief (
            race_id    TEXT PRIMARY KEY,
            headline   TEXT NOT NULL DEFAULT '',
            body       TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT ''
        );",
    )?;
    Ok(())
}

/// Debrief em cache (manchete + parágrafo do engenheiro).
pub struct PostRaceRow {
    pub headline: String,
    pub body: String,
}

/// Lê o debrief em cache de uma etapa, se já gerado.
pub fn get_post_race(conn: &Connection, race_id: &str) -> Result<Option<PostRaceRow>, DbError> {
    ensure_table(conn)?;
    let row = conn
        .query_row(
            "SELECT headline, body FROM ai_post_race_debrief WHERE race_id = ?1",
            params![race_id],
            |r| {
                Ok(PostRaceRow {
                    headline: r.get(0)?,
                    body: r.get(1)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

/// Guarda (cacheia) o debrief gerado pelo servidor para uma etapa. Sobrescreve.
pub fn set_post_race(
    conn: &Connection,
    race_id: &str,
    headline: &str,
    body: &str,
) -> Result<(), DbError> {
    ensure_table(conn)?;
    let now = chrono::Local::now().timestamp().to_string();
    conn.execute(
        "INSERT OR REPLACE INTO ai_post_race_debrief (race_id, headline, body, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![race_id, headline, body, now],
    )?;
    Ok(())
}
