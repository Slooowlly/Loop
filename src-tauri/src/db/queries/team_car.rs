//! Persistência do estado do carro por time (Sistema de Nível do Carro).
//!
//! Uma linha por `(team_id, part_type)` com nível, desgaste e esgotamento. O carro é
//! carregado/salvo como unidade (as 11 peças). A tabela é criada pela migration v48;
//! `ensure_table` reaplica de forma idempotente para conexões de teste in-memory que não
//! rodam migrações. Ver design em
//! `docs/superpowers/specs/2026-07-17-car-level-system-design.md`.

use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::car::{Car, CarPart, PartType};
use crate::db::connection::DbError;

fn ensure_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS team_car (
            team_id    TEXT NOT NULL,
            part_type  TEXT NOT NULL,
            level      INTEGER NOT NULL DEFAULT 1,
            wear       REAL NOT NULL DEFAULT 0.0,
            spent      INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (team_id, part_type)
        );",
    )?;
    Ok(())
}

/// Carrega o carro de um time. `None` se o time ainda não tem carro persistido.
/// Peças ausentes no banco caem para um default seguro (nível 1, sem desgaste).
pub fn get_team_car(conn: &Connection, team_id: &str) -> Result<Option<Car>, DbError> {
    ensure_table(conn)?;
    let mut stmt =
        conn.prepare("SELECT part_type, level, wear, spent FROM team_car WHERE team_id = ?1")?;
    let rows = stmt
        .query_map(params![team_id], |r| {
            let part: String = r.get(0)?;
            let level: i64 = r.get(1)?;
            let wear: f64 = r.get(2)?;
            let spent: i64 = r.get(3)?;
            Ok((part, level, wear, spent))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if rows.is_empty() {
        return Ok(None);
    }

    let mut by_type: HashMap<PartType, CarPart> = HashMap::new();
    for (part, level, wear, spent) in rows {
        if let Some(part_type) = PartType::from_str(&part) {
            by_type.insert(
                part_type,
                CarPart {
                    part_type,
                    level: level.clamp(1, 10) as u8,
                    wear,
                    spent: spent != 0,
                },
            );
        }
    }

    let parts = PartType::ALL
        .iter()
        .map(|&part_type| {
            by_type.get(&part_type).copied().unwrap_or(CarPart {
                part_type,
                level: 1,
                wear: 0.0,
                spent: false,
            })
        })
        .collect();

    Ok(Some(Car { parts }))
}

/// Grava/atualiza as 11 peças do carro de um time (idempotente por `(team_id, part_type)`).
pub fn upsert_team_car(conn: &Connection, team_id: &str, car: &Car) -> Result<(), DbError> {
    ensure_table(conn)?;
    for part in &car.parts {
        conn.execute(
            "INSERT INTO team_car (team_id, part_type, level, wear, spent)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(team_id, part_type) DO UPDATE SET
                level = excluded.level,
                wear = excluded.wear,
                spent = excluded.spent",
            params![
                team_id,
                part.part_type.as_str(),
                part.level as i64,
                part.wear,
                part.spent as i64,
            ],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn get_em_time_sem_carro_e_none() {
        let c = conn();
        assert!(get_team_car(&c, "T1").unwrap().is_none());
    }

    #[test]
    fn round_trip_preserva_nivel_desgaste_e_spent() {
        let c = conn();
        let mut car = Car::uniform(5);
        car.set_level(PartType::Engine, 8);
        car.set_wear(PartType::Engine, 0.42);
        if let Some(p) = car.parts.iter_mut().find(|p| p.part_type == PartType::Brakes) {
            p.spent = true;
        }
        upsert_team_car(&c, "T1", &car).unwrap();

        let loaded = get_team_car(&c, "T1").unwrap().unwrap();
        assert_eq!(loaded.parts.len(), 11);
        let engine = loaded.part(PartType::Engine).unwrap();
        assert_eq!(engine.level, 8);
        assert!((engine.wear - 0.42).abs() < 1e-9);
        assert!(loaded.part(PartType::Brakes).unwrap().spent);
        assert_eq!(loaded, car);
    }

    #[test]
    fn upsert_e_idempotente_nao_duplica_linhas() {
        let c = conn();
        let car = Car::uniform(4);
        upsert_team_car(&c, "T1", &car).unwrap();
        upsert_team_car(&c, "T1", &car).unwrap();

        let count: i64 = c
            .query_row("SELECT COUNT(*) FROM team_car WHERE team_id = 'T1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 11, "deveria haver exatamente 11 peças, sem duplicar");
    }

    #[test]
    fn nao_vaza_entre_times() {
        let c = conn();
        upsert_team_car(&c, "A", &Car::uniform(7)).unwrap();
        upsert_team_car(&c, "B", &Car::uniform(2)).unwrap();
        assert_eq!(get_team_car(&c, "A").unwrap().unwrap().display_level(), 7);
        assert_eq!(get_team_car(&c, "B").unwrap().unwrap().display_level(), 2);
    }
}
