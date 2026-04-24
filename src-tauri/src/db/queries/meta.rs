#![allow(dead_code)]

use rusqlite::{Connection, OptionalExtension};

use crate::db::connection::DbError;

/// Le um valor da tabela meta. Retorna None se a chave nao existir.
pub fn get_meta_value(conn: &Connection, key: &str) -> Result<Option<String>, DbError> {
    conn.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        rusqlite::params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(DbError::from)
}

/// Atualiza um valor da tabela meta.
///
/// Presume que a chave ja existe (inserida pelas migrations).
/// Nao faz INSERT, nao faz UPSERT, nao faz INSERT OR REPLACE.
/// Se a chave nao existir, retorna erro explicito.
pub fn set_meta_value(conn: &Connection, key: &str, value: &str) -> Result<(), DbError> {
    let updated = conn.execute(
        "UPDATE meta SET value = ?1 WHERE key = ?2",
        rusqlite::params![value, key],
    )?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("meta key not found: {key}")));
    }
    Ok(())
}

/// Atalho semantico: atualiza current_season.
pub fn set_current_season(conn: &Connection, numero: i32) -> Result<(), DbError> {
    if numero <= 0 {
        return Err(DbError::InvalidData(format!(
            "current_season invalida: '{numero}'"
        )));
    }
    set_meta_value(conn, "current_season", &numero.to_string())
}

/// Atalho semantico: atualiza current_year.
pub fn set_current_year(conn: &Connection, year: i32) -> Result<(), DbError> {
    if year <= 0 {
        return Err(DbError::InvalidData(format!(
            "current_year invalido: '{year}'"
        )));
    }
    set_meta_value(conn, "current_year", &year.to_string())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::db::migrations;

    #[test]
    fn test_set_meta_value_fails_when_key_is_missing() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");

        let err = set_meta_value(&conn, "missing_key", "123").expect_err("missing key");
        assert!(err.to_string().contains("meta key not found"));
    }

    #[test]
    fn test_set_current_season_rejects_non_positive_values() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");

        let err = set_current_season(&conn, 0).expect_err("invalid season");
        assert!(err.to_string().contains("current_season invalida"));
    }

    #[test]
    fn test_set_current_year_rejects_non_positive_values() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");

        let err = set_current_year(&conn, -2024).expect_err("invalid year");
        assert!(err.to_string().contains("current_year invalido"));
    }
}
