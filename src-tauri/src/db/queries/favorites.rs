//! Pilotos favoritados pelo jogador (watchlist). Marca quem o jogador quer
//! acompanhar — alimenta a ênfase do feed do mercado e o filtro "Favoritos" na aba
//! de pilotos. Puramente cosmético/editorial: NÃO toca na lógica de simulação. O
//! favorito acompanha o piloto por equipe/categoria/aposentadoria (chave = piloto_id).

use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;

/// O piloto está favoritado?
pub fn is_favorite(conn: &Connection, piloto_id: &str) -> Result<bool, DbError> {
    let found: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM driver_favorites WHERE piloto_id = ?1",
            params![piloto_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

/// Define explicitamente o estado de favorito (idempotente).
pub fn set_favorite(conn: &Connection, piloto_id: &str, favorite: bool) -> Result<(), DbError> {
    if favorite {
        conn.execute(
            "INSERT OR IGNORE INTO driver_favorites (piloto_id) VALUES (?1)",
            params![piloto_id],
        )?;
    } else {
        conn.execute(
            "DELETE FROM driver_favorites WHERE piloto_id = ?1",
            params![piloto_id],
        )?;
    }
    Ok(())
}

/// Inverte o estado de favorito e devolve o NOVO estado (true = agora favoritado).
pub fn toggle_favorite(conn: &Connection, piloto_id: &str) -> Result<bool, DbError> {
    let now_favorite = !is_favorite(conn, piloto_id)?;
    set_favorite(conn, piloto_id, now_favorite)?;
    Ok(now_favorite)
}

/// Conjunto de todos os piloto_id favoritados (p/ marcar payloads em lote / feed).
pub fn get_favorite_ids(conn: &Connection) -> Result<HashSet<String>, DbError> {
    let mut stmt = conn.prepare("SELECT piloto_id FROM driver_favorites")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut set = HashSet::new();
    for id in rows {
        set.insert(id?);
    }
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE driver_favorites (
                piloto_id  TEXT PRIMARY KEY,
                criado_em  TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn toggle_flips_state_and_persists() {
        let conn = setup_db();
        assert!(!is_favorite(&conn, "P1").unwrap());

        assert!(toggle_favorite(&conn, "P1").unwrap()); // agora favorito
        assert!(is_favorite(&conn, "P1").unwrap());

        assert!(!toggle_favorite(&conn, "P1").unwrap()); // desfavorita
        assert!(!is_favorite(&conn, "P1").unwrap());
    }

    #[test]
    fn set_favorite_is_idempotent() {
        let conn = setup_db();
        set_favorite(&conn, "P1", true).unwrap();
        set_favorite(&conn, "P1", true).unwrap(); // sem erro de PK
        assert!(is_favorite(&conn, "P1").unwrap());
        set_favorite(&conn, "P1", false).unwrap();
        set_favorite(&conn, "P1", false).unwrap();
        assert!(!is_favorite(&conn, "P1").unwrap());
    }

    #[test]
    fn get_favorite_ids_returns_all() {
        let conn = setup_db();
        set_favorite(&conn, "P1", true).unwrap();
        set_favorite(&conn, "P2", true).unwrap();
        let ids = get_favorite_ids(&conn).unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains("P1") && ids.contains("P2"));
    }
}
