#![allow(dead_code)]

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;
use crate::models::enums::{SeasonPhase, SeasonStatus};
use crate::models::season::Season;

pub fn insert_season(conn: &Connection, season: &Season) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO seasons (
            id, numero, ano, status, rodada_atual, fase, created_at, updated_at
        ) VALUES (
            :id, :numero, :ano, :status, :rodada_atual, :fase, :created_at, :updated_at
        )",
        rusqlite::named_params! {
            ":id": &season.id,
            ":numero": season.numero,
            ":ano": season.ano,
            ":status": season.status.as_str(),
            ":rodada_atual": season.rodada_atual,
            ":fase": season.fase.as_str(),
            ":created_at": &season.created_at,
            ":updated_at": &season.updated_at,
        },
    )?;
    Ok(())
}

pub fn get_season_by_id(conn: &Connection, id: &str) -> Result<Option<Season>, DbError> {
    let mut stmt = conn.prepare("SELECT * FROM seasons WHERE id = ?1")?;
    let season = stmt.query_row(params![id], season_from_row).optional()?;
    Ok(season)
}

pub fn get_active_season(conn: &Connection) -> Result<Option<Season>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM seasons
         WHERE status IN ('EmAndamento', 'Ativa')
         ORDER BY numero DESC",
    )?;
    let mapped = stmt.query_map([], season_from_row)?;
    let mut seasons = Vec::new();
    for row in mapped {
        seasons.push(row?);
    }

    if seasons.len() > 1 {
        let ids = seasons
            .iter()
            .map(|season| season.id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(DbError::InvalidData(format!(
            "multiplas temporadas ativas encontradas: {ids}"
        )));
    }

    Ok(seasons.into_iter().next())
}

pub fn update_season_rodada(conn: &Connection, id: &str, rodada: i32) -> Result<(), DbError> {
    let updated = conn.execute(
        "UPDATE seasons
         SET rodada_atual = ?2, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?1",
        params![id, rodada],
    )?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("season not found: {id}")));
    }
    Ok(())
}

pub fn finalize_season(conn: &Connection, id: &str) -> Result<(), DbError> {
    let updated = conn.execute(
        "UPDATE seasons
         SET status = 'Finalizada', updated_at = CURRENT_TIMESTAMP
         WHERE id = ?1",
        params![id],
    )?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("season not found: {id}")));
    }
    Ok(())
}

pub fn get_all_seasons(conn: &Connection) -> Result<Vec<Season>, DbError> {
    let mut stmt = conn.prepare("SELECT * FROM seasons ORDER BY numero ASC")?;
    let mapped = stmt.query_map([], season_from_row)?;
    let mut seasons = Vec::new();
    for row in mapped {
        seasons.push(row?);
    }
    Ok(seasons)
}

fn season_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Season> {
    let fase_str: String = row.get("fase")?;
    let fase =
        SeasonPhase::from_str_strict(&fase_str).map_err(rusqlite::Error::InvalidParameterName)?;
    let rodada_atual: i32 = row.get("rodada_atual")?;
    if rodada_atual <= 0 {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "Season.rodada_atual invalida: '{rodada_atual}'"
        )));
    }

    Ok(Season {
        id: row.get("id")?,
        numero: row.get("numero")?,
        ano: row.get("ano")?,
        status: SeasonStatus::from_str_strict(&row.get::<_, String>("status")?)
            .map_err(rusqlite::Error::InvalidParameterName)?,
        rodada_atual,
        fase,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn update_season_fase(conn: &Connection, id: &str, fase: &SeasonPhase) -> Result<(), DbError> {
    let updated = conn.execute(
        "UPDATE seasons
         SET fase = ?1, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?2",
        params![fase.as_str(), id],
    )?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("season not found: {id}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::db::migrations;

    #[test]
    fn test_insert_and_get_season() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");

        let season = Season::new("S001".to_string(), 1, 2024);
        insert_season(&conn, &season).expect("insert");

        let loaded = get_season_by_id(&conn, "S001")
            .expect("select")
            .expect("season");
        assert_eq!(loaded.numero, 1);
        assert_eq!(loaded.status, SeasonStatus::EmAndamento);
        assert_eq!(loaded.fase, SeasonPhase::BlocoRegular);
    }

    #[test]
    fn test_get_active_and_finalize_season() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");

        let season = Season::new("S001".to_string(), 1, 2024);
        insert_season(&conn, &season).expect("insert");
        assert!(get_active_season(&conn).expect("active").is_some());

        finalize_season(&conn, "S001").expect("finalize");
        assert!(get_active_season(&conn).expect("active after").is_none());
    }

    #[test]
    fn test_get_active_season_accepts_legacy_ativa_status() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");

        conn.execute(
            "INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
             VALUES ('S001', 1, 2024, 'Ativa', 1, 'BlocoRegular', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [],
        )
        .expect("insert legacy active season");

        let active = get_active_season(&conn)
            .expect("active season lookup")
            .expect("active season");
        assert_eq!(active.id, "S001");
        assert_eq!(active.status, SeasonStatus::EmAndamento);
    }

    #[test]
    fn test_get_active_season_rejects_multiple_active_seasons() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");

        conn.execute(
            "INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
             VALUES ('S001', 1, 2024, 'EmAndamento', 1, 'BlocoRegular', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
             VALUES ('S002', 2, 2025, 'Ativa', 1, 'BlocoRegular', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [],
        )
        .unwrap();

        let err = get_active_season(&conn).expect_err("duplicate active seasons should fail");
        assert!(err.to_string().contains("multiplas temporadas ativas"));
    }

    #[test]
    fn test_update_helpers_fail_when_season_is_missing() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");

        let err = update_season_rodada(&conn, "MISSING", 3).expect_err("missing season");
        assert!(err.to_string().contains("season not found"));

        let err = finalize_season(&conn, "MISSING").expect_err("missing season");
        assert!(err.to_string().contains("season not found"));

        let err = update_season_fase(&conn, "MISSING", &SeasonPhase::BlocoEspecial)
            .expect_err("missing season");
        assert!(err.to_string().contains("season not found"));
    }
}
