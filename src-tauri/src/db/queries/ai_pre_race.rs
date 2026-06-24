//! Cache da PRÉVIA pré-corrida gerada por IA (narrativa + voz da equipe, curtas),
//! mostrada na Sala de Estratégia. Chave = `race_id` (uma prévia por etapa). Assim
//! reentrar na tela não regenera (sem custo e sem esbarrar no cooldown). A tabela
//! é criada de forma idempotente — não depende do sistema de migrações.

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;

fn ensure_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ai_pre_race_briefing (
            race_id    TEXT PRIMARY KEY,
            narrative  TEXT NOT NULL,
            team_voice TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT ''
        );",
    )?;
    Ok(())
}

/// Prévia em cache (narrativa + voz da equipe).
pub struct PreRaceRow {
    pub narrative: String,
    pub team_voice: String,
}

/// Lê a prévia em cache de uma etapa, se já gerada.
pub fn get_pre_race(conn: &Connection, race_id: &str) -> Result<Option<PreRaceRow>, DbError> {
    ensure_table(conn)?;
    let row = conn
        .query_row(
            "SELECT narrative, team_voice FROM ai_pre_race_briefing WHERE race_id = ?1",
            params![race_id],
            |r| {
                Ok(PreRaceRow {
                    narrative: r.get(0)?,
                    team_voice: r.get(1)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

/// Guarda (cacheia) a prévia gerada pelo servidor para uma etapa. Sobrescreve.
pub fn set_pre_race(
    conn: &Connection,
    race_id: &str,
    narrative: &str,
    team_voice: &str,
) -> Result<(), DbError> {
    ensure_table(conn)?;
    let now = chrono::Local::now().timestamp().to_string();
    conn.execute(
        "INSERT OR REPLACE INTO ai_pre_race_briefing (race_id, narrative, team_voice, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![race_id, narrative, team_voice, now],
    )?;
    Ok(())
}
