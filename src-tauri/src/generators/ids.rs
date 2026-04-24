#![allow(dead_code)]

use rusqlite::Connection;

use crate::db::connection::DbError;

// ── Tipos de entidade com ID sequencial ───────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum IdType {
    Driver,
    Team,
    Season,
    Race,
    Contract,
    News,
    Rivalry,
}

impl IdType {
    /// Chave na tabela meta que guarda o próximo contador.
    fn counter_key(self) -> &'static str {
        match self {
            IdType::Driver => "next_driver_id",
            IdType::Team => "next_team_id",
            IdType::Season => "next_season_id",
            IdType::Race => "next_race_id",
            IdType::Contract => "next_contract_id",
            IdType::News => "next_news_id",
            IdType::Rivalry => "next_rivalry_id",
        }
    }

    /// Prefixo legível do ID (ex: "P", "T", "RV").
    fn prefix(self) -> &'static str {
        match self {
            IdType::Driver => "P",
            IdType::Team => "T",
            IdType::Season => "S",
            IdType::Race => "R",
            IdType::Contract => "C",
            IdType::News => "N",
            IdType::Rivalry => "RV",
        }
    }

    fn tables(self) -> &'static [&'static str] {
        match self {
            IdType::Driver => &["drivers"],
            IdType::Team => &["teams"],
            IdType::Season => &["seasons"],
            IdType::Race => &["calendar", "races"],
            IdType::Contract => &["contracts"],
            IdType::News => &["news"],
            IdType::Rivalry => &["rivalries"],
        }
    }
}

// ── Geração de IDs ────────────────────────────────────────────────────────────

/// Retorna o próximo ID sequencial para o tipo dado e incrementa o contador.
///
/// Exemplo: `next_id(conn, IdType::Driver)` → `"P001"`, depois `"P002"`, etc.
pub fn next_id(conn: &Connection, id_type: IdType) -> Result<String, DbError> {
    let key = id_type.counter_key();
    let prefix = id_type.prefix();
    let current = current_counter(conn, id_type)?;

    conn.execute(
        "UPDATE meta SET value = ?1 WHERE key = ?2",
        rusqlite::params![(current + 1).to_string(), key],
    )?;

    Ok(format!("{}{:03}", prefix, current))
}

/// Gera `count` IDs sequenciais de uma vez (operação atômica).
///
/// Exemplo: `next_ids(conn, IdType::Driver, 3)` → `["P001", "P002", "P003"]`
pub fn next_ids(conn: &Connection, id_type: IdType, count: u32) -> Result<Vec<String>, DbError> {
    if count == 0 {
        return Ok(Vec::new());
    }

    let key = id_type.counter_key();
    let prefix = id_type.prefix();
    let current = current_counter(conn, id_type)?;

    let next = current + count as i64;
    conn.execute(
        "UPDATE meta SET value = ?1 WHERE key = ?2",
        rusqlite::params![next.to_string(), key],
    )?;

    let ids = (current..next)
        .map(|n| format!("{}{:03}", prefix, n))
        .collect();

    Ok(ids)
}

fn current_counter(conn: &Connection, id_type: IdType) -> Result<i64, DbError> {
    let key = id_type.counter_key();
    let stored: i64 = conn
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM meta WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        )
        .map_err(|_| DbError::NotFound(format!("Contador '{}' não encontrado em meta", key)))?;

    Ok(stored.max(observed_next_counter(conn, id_type)?))
}

fn observed_next_counter(conn: &Connection, id_type: IdType) -> Result<i64, DbError> {
    let prefix = id_type.prefix();
    let mut next_counter = 1_i64;

    for table in id_type.tables() {
        let sql = format!("SELECT id FROM {table}");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

        for row in rows {
            if let Some(numeric_suffix) = parse_canonical_numeric_id(&row?, prefix) {
                next_counter = next_counter.max(numeric_suffix + 1);
            }
        }
    }

    Ok(next_counter)
}

fn parse_canonical_numeric_id(id: &str, prefix: &str) -> Option<i64> {
    let suffix = id.strip_prefix(prefix)?;
    if suffix.is_empty() || !suffix.chars().all(|char| char.is_ascii_digit()) {
        return None;
    }

    suffix.parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::db::migrations;
    use crate::db::queries::drivers as driver_queries;
    use crate::db::queries::meta as meta_queries;
    use crate::models::driver::Driver;

    #[test]
    fn next_ids_resyncs_stale_driver_counter_with_existing_rows() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");

        let mut driver_a = Driver::new(
            "P001".to_string(),
            "Driver A".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            22,
            2018,
        );
        driver_a.categoria_atual = Some("mazda_rookie".to_string());
        let mut driver_b = Driver::new(
            "P265".to_string(),
            "Driver B".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            23,
            2017,
        );
        driver_b.categoria_atual = Some("mazda_rookie".to_string());
        driver_queries::insert_driver(&conn, &driver_a).expect("insert driver a");
        driver_queries::insert_driver(&conn, &driver_b).expect("insert driver b");
        meta_queries::set_meta_value(&conn, "next_driver_id", "201").expect("stale driver counter");

        let generated = next_ids(&conn, IdType::Driver, 3).expect("generate ids");
        let stored_counter = meta_queries::get_meta_value(&conn, "next_driver_id")
            .expect("counter query")
            .expect("counter value");

        assert_eq!(generated, vec!["P266", "P267", "P268"]);
        assert_eq!(stored_counter, "269");
    }
}
