#![allow(dead_code)]

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;
use crate::models::rivalry::{perceived_intensity, Rivalry, RivalryType};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn row_to_rivalry(row: &rusqlite::Row) -> rusqlite::Result<Rivalry> {
    let tipo_raw: String = row.get(5)?;
    let tipo = match tipo_raw.trim() {
        "Colisao" => RivalryType::Colisao,
        "Companheiros" => RivalryType::Companheiros,
        "Campeonato" => RivalryType::Campeonato,
        "Pista" => RivalryType::Pista,
        other => {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "RivalryType inválido: '{other}'"
            )))
        }
    };
    Ok(Rivalry {
        id: row.get(0)?,
        piloto1_id: row.get(1)?,
        piloto2_id: row.get(2)?,
        historical_intensity: row.get(3)?,
        recent_activity: row.get(4)?,
        tipo,
        criado_em: row.get(6)?,
        ultima_atualizacao: row.get(7)?,
        temporada_update: row.get(8)?,
    })
}

const SELECT_COLS: &str = "id, piloto1_id, piloto2_id, historical_intensity, recent_activity, \
     tipo, criado_em, ultima_atualizacao, temporada_update";

// ── Leitura ───────────────────────────────────────────────────────────────────

/// Busca rivalidade pelo par normalizado (piloto1_id < piloto2_id).
pub fn get_rivalry_by_pair(
    conn: &Connection,
    piloto1_id: &str,
    piloto2_id: &str,
) -> Result<Option<Rivalry>, DbError> {
    let sql =
        format!("SELECT {SELECT_COLS} FROM rivalries WHERE piloto1_id = ?1 AND piloto2_id = ?2");
    conn.query_row(&sql, params![piloto1_id, piloto2_id], row_to_rivalry)
        .optional()
        .map_err(DbError::from)
}

/// Retorna todas as rivalidades de um piloto, ordenadas por intensidade percebida decrescente.
pub fn get_rivalries_for_pilot(conn: &Connection, pilot_id: &str) -> Result<Vec<Rivalry>, DbError> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM rivalries
         WHERE piloto1_id = ?1 OR piloto2_id = ?1
         ORDER BY (historical_intensity * 0.4 + recent_activity * 0.6) DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let iter = stmt.query_map(params![pilot_id], row_to_rivalry)?;
    let mut out = Vec::new();
    for r in iter {
        out.push(r?);
    }
    Ok(out)
}

/// Retorna todas as rivalidades existentes (usado no decaimento de fim de temporada).
pub fn get_all_rivalries(conn: &Connection) -> Result<Vec<Rivalry>, DbError> {
    let sql = format!("SELECT {SELECT_COLS} FROM rivalries");
    let mut stmt = conn.prepare(&sql)?;
    let iter = stmt.query_map([], row_to_rivalry)?;
    let mut out = Vec::new();
    for r in iter {
        out.push(r?);
    }
    Ok(out)
}

// ── Escrita ───────────────────────────────────────────────────────────────────

/// Insere uma rivalidade nova.
/// A coluna `intensidade` (legado) é mantida em sincronia com a percebida.
pub fn insert_rivalry(conn: &Connection, rivalry: &Rivalry) -> Result<(), DbError> {
    if rivalry.piloto1_id == rivalry.piloto2_id {
        return Err(DbError::InvalidData(
            "rivalidade invalida: piloto1_id e piloto2_id nao podem ser iguais".to_string(),
        ));
    }
    if rivalry.piloto1_id > rivalry.piloto2_id {
        return Err(DbError::InvalidData(
            "rivalidade invalida: par deve estar normalizado (piloto1_id < piloto2_id)".to_string(),
        ));
    }
    let perceived = rivalry.perceived_intensity();
    conn.execute(
        "INSERT INTO rivalries
             (id, piloto1_id, piloto2_id, intensidade, historical_intensity,
              recent_activity, tipo, criado_em, ultima_atualizacao, temporada_update)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            rivalry.id,
            rivalry.piloto1_id,
            rivalry.piloto2_id,
            perceived,
            rivalry.historical_intensity,
            rivalry.recent_activity,
            rivalry.tipo.as_str(),
            rivalry.criado_em,
            rivalry.ultima_atualizacao,
            rivalry.temporada_update,
        ],
    )?;
    Ok(())
}

/// Atualiza os dois eixos de intensidade e o timestamp de uma rivalidade existente.
/// A coluna `intensidade` (legado) é mantida em sincronia (= percebida).
/// O tipo original é preservado (identidade narrativa).
pub fn update_rivalry_axes(
    conn: &Connection,
    id: &str,
    historical_intensity: f64,
    recent_activity: f64,
    ultima_atualizacao: &str,
    temporada_update: i32,
) -> Result<(), DbError> {
    let perceived = perceived_intensity(historical_intensity, recent_activity);
    let affected = conn.execute(
        "UPDATE rivalries
         SET intensidade = ?1, historical_intensity = ?2, recent_activity = ?3,
             ultima_atualizacao = ?4, temporada_update = ?5
         WHERE id = ?6",
        params![
            perceived,
            historical_intensity,
            recent_activity,
            ultima_atualizacao,
            temporada_update,
            id,
        ],
    )?;
    if affected == 0 {
        return Err(DbError::NotFound(format!(
            "rivalidade nao encontrada: {id}"
        )));
    }
    Ok(())
}

/// Remove uma rivalidade pelo id.
pub fn delete_rivalry(conn: &Connection, id: &str) -> Result<(), DbError> {
    let affected = conn.execute("DELETE FROM rivalries WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(DbError::NotFound(format!(
            "rivalidade nao encontrada: {id}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE rivalries (
                id TEXT PRIMARY KEY,
                piloto1_id TEXT NOT NULL,
                piloto2_id TEXT NOT NULL,
                intensidade REAL NOT NULL DEFAULT 0.0,
                historical_intensity REAL NOT NULL DEFAULT 0.0,
                recent_activity REAL NOT NULL DEFAULT 0.0,
                tipo TEXT NOT NULL DEFAULT 'Pista',
                criado_em TEXT NOT NULL DEFAULT '',
                ultima_atualizacao TEXT NOT NULL DEFAULT '',
                temporada_update INTEGER NOT NULL DEFAULT 0
            );",
        )
        .unwrap();
        conn
    }

    fn sample_rivalry() -> Rivalry {
        Rivalry {
            id: "RIV-001".to_string(),
            piloto1_id: "P001".to_string(),
            piloto2_id: "P002".to_string(),
            historical_intensity: 20.0,
            recent_activity: 30.0,
            tipo: RivalryType::Pista,
            criado_em: "2026-01-01T00:00:00".to_string(),
            ultima_atualizacao: "2026-01-01T00:00:00".to_string(),
            temporada_update: 1,
        }
    }

    #[test]
    fn test_insert_rivalry_rejects_non_normalized_pair() {
        let conn = setup_db();
        let mut rivalry = sample_rivalry();
        rivalry.piloto1_id = "P010".to_string();
        rivalry.piloto2_id = "P002".to_string();

        let err = insert_rivalry(&conn, &rivalry).expect_err("non-normalized pair should fail");

        assert!(matches!(err, DbError::InvalidData(_)));
    }

    #[test]
    fn test_update_rivalry_axes_returns_not_found_for_missing_id() {
        let conn = setup_db();

        let err = update_rivalry_axes(&conn, "RIV-404", 10.0, 10.0, "2026-01-01T00:00:00", 1)
            .expect_err("missing rivalry should fail");

        assert!(matches!(err, DbError::NotFound(_)));
    }

    #[test]
    fn test_delete_rivalry_returns_not_found_for_missing_id() {
        let conn = setup_db();

        let err = delete_rivalry(&conn, "RIV-404").expect_err("missing rivalry should fail");

        assert!(matches!(err, DbError::NotFound(_)));
    }

    #[test]
    fn test_get_rivalry_by_pair_returns_error_for_invalid_type() {
        let conn = setup_db();
        let rivalry = sample_rivalry();
        insert_rivalry(&conn, &rivalry).expect("insert rivalry");
        conn.execute(
            "UPDATE rivalries SET tipo = 'quebrado' WHERE id = 'RIV-001'",
            [],
        )
        .unwrap();

        let err = get_rivalry_by_pair(&conn, "P001", "P002").expect_err("invalid type should fail");

        assert!(err.to_string().contains("RivalryType"));
    }
}
