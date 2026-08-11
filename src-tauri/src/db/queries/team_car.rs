//! Persistência do estado do carro por time (Sistema de Nível do Carro).
//!
//! Uma linha por `(team_id, part_type)` com nível, desgaste e esgotamento. O carro é
//! carregado/salvo como unidade (as 11 peças). A tabela é criada pelas migrações (baseline
//! v53 + coluna `unit_seed` na v62); `ensure_table` reaplica o MESMO DDL de forma idempotente
//! para conexões de teste in-memory que não rodam migrações. Ver design em
//! `docs/superpowers/specs/2026-07-17-car-level-system-design.md`.

use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::car::{Car, CarPart, PartType};
use crate::db::connection::DbError;

/// DDL da tabela, num lugar só.
///
/// A migração v62 executa esta MESMA constante: enquanto o DDL vivia em duas cópias
/// textuais (aqui e na baseline), a coluna `unit_seed` existia de um lado só. Ver
/// `db::migrations::migrate_v62_tabelas_de_query_sob_as_migracoes`.
pub(crate) const DDL_TEAM_CAR: &str = "
    CREATE TABLE IF NOT EXISTS team_car (
        team_id    TEXT NOT NULL,
        part_type  TEXT NOT NULL,
        level      INTEGER NOT NULL DEFAULT 1,
        wear       REAL NOT NULL DEFAULT 0.0,
        spent      INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (team_id, part_type)
    );
";

fn ensure_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(DDL_TEAM_CAR)?;
    // Identidade da unidade (redesign 2026-07-22 §4.1). `0` = não semeado → o carregador
    // deriva o fallback determinístico por tipo. A v62 aplica o mesmo ALTER guardado nos
    // saves em campo; isto aqui segura as conexões de teste in-memory que não migram.
    crate::db::migrations::add_column_if_missing(
        conn,
        "team_car",
        "unit_seed",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    Ok(())
}

/// Carrega o carro de um time. `None` se o time ainda não tem carro persistido.
/// Peças ausentes no banco caem para um default seguro (nível 1, sem desgaste).
pub fn get_team_car(conn: &Connection, team_id: &str) -> Result<Option<Car>, DbError> {
    ensure_table(conn)?;
    let mut stmt = conn.prepare(
        "SELECT part_type, level, wear, spent, unit_seed FROM team_car WHERE team_id = ?1",
    )?;
    let rows = stmt
        .query_map(params![team_id], |r| {
            let part: String = r.get(0)?;
            let level: i64 = r.get(1)?;
            let wear: f64 = r.get(2)?;
            let spent: i64 = r.get(3)?;
            let unit_seed: i64 = r.get(4)?;
            Ok((part, level, wear, spent, unit_seed))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if rows.is_empty() {
        return Ok(None);
    }

    let mut by_type: HashMap<PartType, CarPart> = HashMap::new();
    for (part, level, wear, spent, unit_seed) in rows {
        if let Some(part_type) = PartType::from_str(&part) {
            by_type.insert(
                part_type,
                CarPart {
                    part_type,
                    level: level.clamp(1, 10) as u8,
                    wear,
                    spent: spent != 0,
                    // `0` = save anterior à identidade da unidade → sentinela resolvida por
                    // `wear::part_life_scale` -> `initial_unit_seed` (fallback por tipo).
                    unit_seed: unit_seed as u32,
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
                unit_seed: 0,
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
            "INSERT INTO team_car (team_id, part_type, level, wear, spent, unit_seed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(team_id, part_type) DO UPDATE SET
                level = excluded.level,
                wear = excluded.wear,
                spent = excluded.spent,
                unit_seed = excluded.unit_seed",
            params![
                team_id,
                part.part_type.as_str(),
                part.level as i64,
                part.wear,
                part.spent as i64,
                part.unit_seed as i64,
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
        if let Some(p) = car
            .parts
            .iter_mut()
            .find(|p| p.part_type == PartType::Brakes)
        {
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
            .query_row(
                "SELECT COUNT(*) FROM team_car WHERE team_id = 'T1'",
                [],
                |r| r.get(0),
            )
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
