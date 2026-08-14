//! Cache da PRÉVIA pré-corrida gerada por IA (narrativa + voz da equipe, curtas),
//! mostrada na Sala de Estratégia. Chave = `race_id` (uma prévia por etapa). Assim
//! reentrar na tela não regenera (sem custo e sem esbarrar no cooldown). A tabela nasceu
//! fora das migrações e entrou nelas na v62; o `ensure_table` reaplica o MESMO DDL, de
//! forma idempotente, para conexões de teste in-memory que não migram.

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;

/// DDL da tabela, num lugar só — a migração v62 executa esta MESMA constante.
pub(crate) const DDL_AI_PRE_RACE_BRIEFING: &str = "
    CREATE TABLE IF NOT EXISTS ai_pre_race_briefing (
        race_id    TEXT PRIMARY KEY,
        headline   TEXT NOT NULL DEFAULT '',
        narrative  TEXT NOT NULL,
        team_voice TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT ''
    );
";

/// Reaplica o DDL para conexões de teste in-memory que não migram. Só isso: schema
/// permanente é assunto das migrações, e o `ALTER` de `headline` (tabela criada antes da
/// manchete cinematográfica) mora na v62, que é o lado que existe para consertar save em
/// campo. Aqui ele era no-op: a constante acima já declara a coluna.
fn ensure_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(DDL_AI_PRE_RACE_BRIEFING)?;
    Ok(())
}

/// Prévia em cache (manchete + corpo/narrativa + voz da equipe).
pub struct PreRaceRow {
    pub headline: String,
    pub narrative: String,
    pub team_voice: String,
}

/// Lê a prévia em cache de uma etapa, se já gerada.
pub fn get_pre_race(conn: &Connection, race_id: &str) -> Result<Option<PreRaceRow>, DbError> {
    ensure_table(conn)?;
    let row = conn
        .query_row(
            "SELECT headline, narrative, team_voice FROM ai_pre_race_briefing WHERE race_id = ?1",
            params![race_id],
            |r| {
                Ok(PreRaceRow {
                    headline: r.get(0)?,
                    narrative: r.get(1)?,
                    team_voice: r.get(2)?,
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
    headline: &str,
    narrative: &str,
    team_voice: &str,
) -> Result<(), DbError> {
    ensure_table(conn)?;
    let now = chrono::Local::now().timestamp().to_string();
    conn.execute(
        "INSERT OR REPLACE INTO ai_pre_race_briefing (race_id, headline, narrative, team_voice, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![race_id, headline, narrative, team_voice, now],
    )?;
    Ok(())
}
