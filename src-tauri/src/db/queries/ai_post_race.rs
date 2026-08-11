//! Cache do DEBRIEF pós-corrida gerado por IA (voz do engenheiro → piloto), mostrado
//! na tela de classificação final. Chave = `race_id` (um debrief por etapa). Assim
//! reentrar/reabrir a tela não regenera (sem custo e sem esbarrar no cooldown). A tabela
//! nasceu fora das migrações e entrou nelas na v62; o `ensure_table` reaplica o MESMO DDL,
//! de forma idempotente, para conexões de teste in-memory que não migram.

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;

/// DDL da tabela, num lugar só — a migração v62 executa esta MESMA constante.
pub(crate) const DDL_AI_POST_RACE_DEBRIEF: &str = "
    CREATE TABLE IF NOT EXISTS ai_post_race_debrief (
        race_id    TEXT PRIMARY KEY,
        headline   TEXT NOT NULL DEFAULT '',
        body       TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT ''
    );
";

fn ensure_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(DDL_AI_POST_RACE_DEBRIEF)?;
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
