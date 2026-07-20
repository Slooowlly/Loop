//! Cache das notinhas do rodapé "Do mundo do Grid" reescritas por IA. Chave =
//! `temporada:rodada` (o estado do mundo muda a cada rodada, então a nota também).
//! Reabrir a revista não regenera (sem custo, sem esbarrar no cooldown). A tabela é
//! criada de forma idempotente — não depende do sistema de migrações.
//!
//! Guarda um blob JSON opaco (as notas já reescritas, serializadas pelo comando) —
//! a camada de query não conhece o tipo `WorldNote`, só o texto.

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;

fn ensure_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ai_world_notes (
            cache_key  TEXT PRIMARY KEY,
            notes_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT ''
        );",
    )?;
    Ok(())
}

/// Lê o JSON em cache das notas de IA para uma chave, se já gerado.
pub fn get_cached(conn: &Connection, cache_key: &str) -> Result<Option<String>, DbError> {
    ensure_table(conn)?;
    let row = conn
        .query_row(
            "SELECT notes_json FROM ai_world_notes WHERE cache_key = ?1",
            params![cache_key],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    Ok(row)
}

/// Guarda (cacheia) o JSON das notas reescritas para uma chave. Sobrescreve.
pub fn set_cached(conn: &Connection, cache_key: &str, notes_json: &str) -> Result<(), DbError> {
    ensure_table(conn)?;
    let now = chrono::Local::now().timestamp().to_string();
    conn.execute(
        "INSERT OR REPLACE INTO ai_world_notes (cache_key, notes_json, created_at)
         VALUES (?1, ?2, ?3)",
        params![cache_key, notes_json, now],
    )?;
    Ok(())
}
