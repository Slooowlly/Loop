use rusqlite::{params, Transaction};

use crate::db::connection::DbError;
use crate::models::enums::InjuryType;
use crate::models::injury::Injury;

fn validate_injury(injury: &Injury) -> Result<(), DbError> {
    if injury.season <= 0 {
        return Err(DbError::InvalidData(format!(
            "temporada invalida para lesao '{}': {}",
            injury.id, injury.season
        )));
    }

    if injury.races_total <= 0 {
        return Err(DbError::InvalidData(format!(
            "races_total invalido para lesao '{}': {}",
            injury.id, injury.races_total
        )));
    }

    if injury.races_remaining < 0 || injury.races_remaining > injury.races_total {
        return Err(DbError::InvalidData(format!(
            "races_remaining invalido para lesao '{}': {} de {}",
            injury.id, injury.races_remaining, injury.races_total
        )));
    }

    Ok(())
}

pub fn insert_injury(tx: &Transaction, injury: &Injury) -> Result<(), DbError> {
    validate_injury(injury)?;

    if injury.active {
        let existing_active: i32 = tx.query_row(
            "SELECT COUNT(*) FROM injuries WHERE pilot_id = ?1 AND active = 1",
            params![injury.pilot_id],
            |row| row.get(0),
        )?;
        if existing_active > 0 {
            return Err(DbError::InvalidData(format!(
                "piloto '{}' ja possui lesao ativa",
                injury.pilot_id
            )));
        }
    }

    tx.execute(
        "INSERT INTO injuries (
            id, pilot_id, type, modifier, races_total, races_remaining, skill_penalty, season, race_occurred, active
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            injury.id,
            injury.pilot_id,
            injury.injury_type.as_str(),
            injury.modifier,
            injury.races_total,
            injury.races_remaining,
            injury.skill_penalty,
            injury.season,
            injury.race_occurred,
            if injury.active { 1 } else { 0 },
        ],
    )?;
    Ok(())
}

pub fn get_active_injuries_for_category(
    tx: &Transaction,
    category_id: &str,
) -> Result<Vec<Injury>, DbError> {
    let mut stmt = tx.prepare(
        "SELECT i.id, i.pilot_id, i.type, i.modifier, i.races_total, i.races_remaining, i.skill_penalty, i.season, i.race_occurred, i.active
         FROM injuries i
         JOIN drivers d ON i.pilot_id = d.id
         WHERE i.active = 1 AND d.categoria_atual = ?1",
    )?;

    let iter = stmt.query_map(params![category_id], |row| {
        Ok(Injury {
            id: row.get(0)?,
            pilot_id: row.get(1)?,
            injury_type: InjuryType::from_str_strict(&row.get::<_, String>(2)?)
                .map_err(rusqlite::Error::InvalidParameterName)?,
            modifier: row.get(3)?,
            races_total: row.get(4)?,
            races_remaining: row.get(5)?,
            skill_penalty: row.get(6)?,
            season: row.get(7)?,
            race_occurred: row.get(8)?,
            active: row.get::<_, i32>(9)? == 1,
        })
    })?;

    let mut injuries = Vec::new();
    for i in iter {
        injuries.push(i?);
    }
    Ok(injuries)
}

pub fn update_injury_status(
    tx: &Transaction,
    injury_id: &str,
    races_remaining: i32,
    active: bool,
) -> Result<(), DbError> {
    if races_remaining < 0 {
        return Err(DbError::InvalidData(format!(
            "races_remaining invalido para lesao '{injury_id}': {races_remaining}"
        )));
    }

    let pilot_id: String = tx
        .query_row(
            "SELECT pilot_id FROM injuries WHERE id = ?1",
            params![injury_id],
            |row| row.get(0),
        )
        .map_err(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("Lesao '{injury_id}' nao encontrada"))
            }
            other => DbError::Sqlite(other),
        })?;

    if active {
        let other_active_count: i32 = tx.query_row(
            "SELECT COUNT(*) FROM injuries WHERE pilot_id = ?1 AND active = 1 AND id <> ?2",
            params![pilot_id, injury_id],
            |row| row.get(0),
        )?;
        if other_active_count > 0 {
            return Err(DbError::InvalidData(format!(
                "piloto '{}' ja possui outra lesao ativa",
                pilot_id
            )));
        }
    }

    let rows = tx.execute(
        "UPDATE injuries SET races_remaining = ?1, active = ?2 WHERE id = ?3",
        params![races_remaining, if active { 1 } else { 0 }, injury_id],
    )?;

    if rows == 0 {
        return Err(DbError::NotFound(format!(
            "Lesao '{injury_id}' nao encontrada"
        )));
    }

    Ok(())
}

pub fn has_active_injury_for_pilot(tx: &Transaction, pilot_id: &str) -> Result<bool, DbError> {
    let count: i32 = tx.query_row(
        "SELECT COUNT(*) FROM injuries WHERE pilot_id = ?1 AND active = 1",
        params![pilot_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::run_all;
    use crate::db::queries::drivers::insert_driver;
    use crate::models::driver::Driver;
    use crate::models::enums::InjuryType;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        run_all(&conn).unwrap();

        let driver = Driver::create_player(
            "P001".to_string(),
            "Piloto Teste".to_string(),
            "BR".to_string(),
            28,
        );
        insert_driver(&conn, &driver).unwrap();
        conn
    }

    fn sample_injury() -> Injury {
        Injury {
            id: "I001".to_string(),
            pilot_id: "P001".to_string(),
            injury_type: InjuryType::Leve,
            modifier: 0.95,
            races_total: 3,
            races_remaining: 3,
            skill_penalty: 0.05,
            season: 1,
            race_occurred: "R001".to_string(),
            active: true,
        }
    }

    #[test]
    fn test_insert_injury_rejects_invalid_races_remaining() {
        let mut conn = setup_test_db();
        let tx = conn.transaction().unwrap();

        let mut injury = sample_injury();
        injury.races_remaining = 4;

        let err = insert_injury(&tx, &injury).expect_err("invalid injury should fail");
        assert!(matches!(err, DbError::InvalidData(_)));
        assert!(err.to_string().contains("races_remaining invalido"));
    }

    #[test]
    fn test_insert_injury_rejects_second_active_injury_for_same_pilot() {
        let mut conn = setup_test_db();
        let tx = conn.transaction().unwrap();

        let first = sample_injury();
        insert_injury(&tx, &first).unwrap();

        let mut second = sample_injury();
        second.id = "I002".to_string();
        second.race_occurred = "R002".to_string();

        let err = insert_injury(&tx, &second)
            .expect_err("second active injury for same pilot should fail");
        assert!(matches!(err, DbError::InvalidData(_)));
        assert!(err.to_string().contains("ja possui lesao ativa"));

        let count: i32 = tx
            .query_row(
                "SELECT COUNT(*) FROM injuries WHERE pilot_id = 'P001'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_update_injury_status_returns_not_found_for_missing_injury() {
        let mut conn = setup_test_db();
        let tx = conn.transaction().unwrap();

        let err = update_injury_status(&tx, "I404", 0, false)
            .expect_err("missing injury should return not found");
        assert!(matches!(err, DbError::NotFound(_)));
    }

    #[test]
    fn test_update_injury_status_rejects_activating_when_other_active_injury_exists() {
        let mut conn = setup_test_db();
        let tx = conn.transaction().unwrap();

        let first = sample_injury();
        insert_injury(&tx, &first).unwrap();

        let mut second = sample_injury();
        second.id = "I002".to_string();
        second.active = false;
        second.race_occurred = "R002".to_string();
        insert_injury(&tx, &second).unwrap();

        let err = update_injury_status(&tx, "I002", 1, true)
            .expect_err("activating second injury should fail");
        assert!(matches!(err, DbError::InvalidData(_)));
        assert!(err.to_string().contains("outra lesao ativa"));
    }
}
