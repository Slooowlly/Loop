use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::db::connection::DbError;
use crate::models::enums::InjuryType;
use crate::models::injury::Injury;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InjurySeverityCounts {
    pub leves: i32,
    pub moderadas: i32,
    pub graves: i32,
}

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
            id, pilot_id, type, injury_name, modifier, races_total, races_remaining, skill_penalty, season, race_occurred, active
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            injury.id,
            injury.pilot_id,
            injury.injury_type.as_str(),
            injury.injury_name,
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
        "SELECT i.id, i.pilot_id, i.type, COALESCE(i.injury_name, ''), i.modifier, i.races_total, i.races_remaining, i.skill_penalty, i.season, i.race_occurred, i.active
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
            injury_name: row.get(3)?,
            modifier: row.get(4)?,
            races_total: row.get(5)?,
            races_remaining: row.get(6)?,
            skill_penalty: row.get(7)?,
            season: row.get(8)?,
            race_occurred: row.get(9)?,
            active: row.get::<_, i32>(10)? == 1,
        })
    })?;

    let mut injuries = Vec::new();
    for i in iter {
        injuries.push(i?);
    }
    Ok(injuries)
}

pub fn get_active_injury_types_by_pilot(
    conn: &Connection,
    pilot_ids: &[String],
) -> Result<HashMap<String, String>, DbError> {
    if pilot_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders = std::iter::repeat_n("?", pilot_ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT pilot_id, type
         FROM injuries
         WHERE active = 1 AND pilot_id IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(pilot_ids.iter()), |row| {
        let injury_type = row.get::<_, String>(1)?;
        InjuryType::from_str_strict(&injury_type).map_err(rusqlite::Error::InvalidParameterName)?;
        Ok((row.get::<_, String>(0)?, injury_type))
    })?;

    let mut injuries_by_pilot = HashMap::new();
    for row in rows {
        let (pilot_id, injury_type) = row?;
        injuries_by_pilot.insert(pilot_id, injury_type);
    }
    Ok(injuries_by_pilot)
}

pub fn get_active_injury_for_pilot(
    conn: &Connection,
    pilot_id: &str,
) -> Result<Option<Injury>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, pilot_id, type, COALESCE(injury_name, ''), modifier, races_total, races_remaining, skill_penalty, season, race_occurred, active
         FROM injuries
         WHERE active = 1 AND pilot_id = ?1
         LIMIT 1",
    )?;
    let injury = stmt
        .query_row(params![pilot_id], |row| {
            Ok(Injury {
                id: row.get(0)?,
                pilot_id: row.get(1)?,
                injury_type: InjuryType::from_str_strict(&row.get::<_, String>(2)?)
                    .map_err(rusqlite::Error::InvalidParameterName)?,
                injury_name: row.get(3)?,
                modifier: row.get(4)?,
                races_total: row.get(5)?,
                races_remaining: row.get(6)?,
                skill_penalty: row.get(7)?,
                season: row.get(8)?,
                race_occurred: row.get(9)?,
                active: row.get::<_, i32>(10)? == 1,
            })
        })
        .optional()?;
    Ok(injury)
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

fn table_exists(conn: &Connection, table_name: &str) -> Result<bool, DbError> {
    let exists = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
        params![table_name],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(exists > 0)
}

fn table_has_column(
    conn: &Connection,
    table_name: &str,
    column_name: &str,
) -> Result<bool, DbError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;

    for column in columns {
        if column? == column_name {
            return Ok(true);
        }
    }

    Ok(false)
}

fn count_legacy_inferred_injuries_by_severity_for_pilot(
    conn: &Connection,
    pilot_id: &str,
    injuries_table_exists: bool,
) -> Result<InjurySeverityCounts, DbError> {
    if !table_exists(conn, "race_results")?
        || !table_has_column(conn, "race_results", "piloto_id")?
        || !table_has_column(conn, "race_results", "race_id")?
        || !table_has_column(conn, "race_results", "dnf")?
        || !table_has_column(conn, "race_results", "dnf_reason")?
    {
        return Ok(InjurySeverityCounts::default());
    }

    let explicit_injury_exclusion = if injuries_table_exists
        && table_has_column(conn, "injuries", "pilot_id")?
        && table_has_column(conn, "injuries", "race_occurred")?
    {
        "AND NOT EXISTS (
            SELECT 1
            FROM injuries i
            WHERE i.pilot_id = r.piloto_id
              AND i.race_occurred = r.race_id
        )"
    } else {
        ""
    };

    let sql = format!(
        "WITH legacy_candidates AS (
            SELECT LOWER(COALESCE(r.dnf_reason, '')) AS reason
            FROM race_results r
            WHERE r.piloto_id = ?1
              AND r.dnf = 1
              AND (
                LOWER(COALESCE(r.dnf_reason, '')) LIKE '%colis%'
                OR LOWER(COALESCE(r.dnf_reason, '')) LIKE '%contato%'
                OR LOWER(COALESCE(r.dnf_reason, '')) LIKE '%batid%'
                OR LOWER(COALESCE(r.dnf_reason, '')) LIKE '%rodou%'
                OR LOWER(COALESCE(r.dnf_reason, '')) LIKE '%barreira%'
                OR LOWER(COALESCE(r.dnf_reason, '')) LIKE '%impact%'
                OR LOWER(COALESCE(r.dnf_reason, '')) LIKE '%capot%'
                OR LOWER(COALESCE(r.dnf_reason, '')) LIKE '%suspens%'
                OR LOWER(COALESCE(r.dnf_reason, '')) LIKE '%acident%'
                OR LOWER(COALESCE(r.dnf_reason, '')) LIKE '%escap%'
              )
              {explicit_injury_exclusion}
         )
         SELECT
            COALESCE(SUM(CASE
                WHEN reason NOT LIKE '%barreira%'
                 AND reason NOT LIKE '%impact%'
                 AND reason NOT LIKE '%capot%'
                 AND reason NOT LIKE '%forte%'
                 AND reason NOT LIKE '%colis%'
                 AND reason NOT LIKE '%contato%'
                 AND reason NOT LIKE '%batid%'
                 AND reason NOT LIKE '%rodou%'
                 AND reason NOT LIKE '%suspens%'
                 AND reason NOT LIKE '%acident%'
                THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE
                WHEN reason NOT LIKE '%barreira%'
                 AND reason NOT LIKE '%impact%'
                 AND reason NOT LIKE '%capot%'
                 AND reason NOT LIKE '%forte%'
                 AND (
                    reason LIKE '%colis%'
                    OR reason LIKE '%contato%'
                    OR reason LIKE '%batid%'
                    OR reason LIKE '%rodou%'
                    OR reason LIKE '%suspens%'
                    OR reason LIKE '%acident%'
                 )
                THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE
                WHEN reason LIKE '%barreira%'
                  OR reason LIKE '%impact%'
                  OR reason LIKE '%capot%'
                  OR reason LIKE '%forte%'
                THEN 1 ELSE 0 END), 0)
         FROM legacy_candidates"
    );

    let counts = conn.query_row(&sql, params![pilot_id], |row| {
        Ok(InjurySeverityCounts {
            leves: row.get(0)?,
            moderadas: row.get(1)?,
            graves: row.get(2)?,
        })
    })?;

    Ok(counts)
}

fn stable_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn injury_count_total(counts: InjurySeverityCounts) -> i32 {
    counts.leves + counts.moderadas + counts.graves
}

fn add_missing_exposure_counts(
    observed: InjurySeverityCounts,
    exposure: InjurySeverityCounts,
) -> InjurySeverityCounts {
    let observed_total = injury_count_total(observed);
    let exposure_total = injury_count_total(exposure);
    if observed_total >= exposure_total {
        return observed;
    }

    let mut result = observed;
    let mut remaining = exposure_total - observed_total;

    let light_missing = (exposure.leves - result.leves).max(0).min(remaining);
    result.leves += light_missing;
    remaining -= light_missing;

    let moderate_missing = (exposure.moderadas - result.moderadas)
        .max(0)
        .min(remaining);
    result.moderadas += moderate_missing;
    remaining -= moderate_missing;

    let grave_missing = (exposure.graves - result.graves).max(0).min(remaining);
    result.graves += grave_missing;
    remaining -= grave_missing;

    result.leves += remaining;
    result
}

fn archived_exposure_target_counts(
    pilot_id: &str,
    active_seasons: i32,
    total_races: i32,
) -> InjurySeverityCounts {
    if active_seasons < 4 && total_races < 35 {
        return InjurySeverityCounts::default();
    }

    let injury_rate_percent = 1 + (stable_hash(pilot_id) % 5) as i32;
    let target_total = ((total_races * injury_rate_percent) + 99) / 100;
    let target_total = target_total.max(1);

    let hash = stable_hash(&format!("{pilot_id}:historical-injuries"));
    let graves = if target_total >= 12 {
        target_total / 12
    } else if target_total >= 5 && hash % 7 == 0 {
        1
    } else {
        0
    };
    let remaining_after_graves = target_total - graves;
    let mut moderadas = remaining_after_graves / 4;
    if remaining_after_graves >= 4 && hash % 3 == 0 {
        moderadas += 1;
    }
    moderadas = moderadas.min(remaining_after_graves);
    let leves = target_total - moderadas - graves;

    InjurySeverityCounts {
        leves,
        moderadas,
        graves,
    }
}

fn count_archived_exposure_injuries_by_severity_for_pilot(
    conn: &Connection,
    pilot_id: &str,
) -> Result<InjurySeverityCounts, DbError> {
    if !table_exists(conn, "driver_season_archive")?
        || !table_has_column(conn, "driver_season_archive", "piloto_id")?
        || !table_has_column(conn, "driver_season_archive", "snapshot_json")?
    {
        return Ok(InjurySeverityCounts::default());
    }

    let mut stmt = conn.prepare(
        "SELECT snapshot_json
         FROM driver_season_archive
         WHERE piloto_id = ?1",
    )?;
    let rows = stmt.query_map(params![pilot_id], |row| row.get::<_, String>(0))?;

    let mut active_seasons = 0;
    let mut total_races = 0;
    for row in rows {
        let snapshot_json = row?;
        let snapshot: serde_json::Value = serde_json::from_str(&snapshot_json).unwrap_or_default();
        let races = snapshot
            .get("corridas")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0)
            .max(0) as i32;
        if races > 0 {
            active_seasons += 1;
            total_races += races;
        }
    }

    Ok(archived_exposure_target_counts(
        pilot_id,
        active_seasons,
        total_races,
    ))
}

pub fn count_injuries_by_severity_for_pilot(
    conn: &Connection,
    pilot_id: &str,
) -> Result<InjurySeverityCounts, DbError> {
    let injuries_table_exists = table_exists(conn, "injuries")?;

    let explicit_counts = if injuries_table_exists {
        conn.query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN type = 'Leve' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN type = 'Moderada' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN type IN ('Grave', 'Critica') THEN 1 ELSE 0 END), 0)
             FROM injuries
             WHERE pilot_id = ?1",
            params![pilot_id],
            |row| {
                Ok(InjurySeverityCounts {
                    leves: row.get(0)?,
                    moderadas: row.get(1)?,
                    graves: row.get(2)?,
                })
            },
        )?
    } else {
        InjurySeverityCounts::default()
    };

    let inferred_counts = count_legacy_inferred_injuries_by_severity_for_pilot(
        conn,
        pilot_id,
        injuries_table_exists,
    )?;
    let exposure_counts = count_archived_exposure_injuries_by_severity_for_pilot(conn, pilot_id)?;

    let observed_counts = InjurySeverityCounts {
        leves: explicit_counts.leves + inferred_counts.leves,
        moderadas: explicit_counts.moderadas + inferred_counts.moderadas,
        graves: explicit_counts.graves + inferred_counts.graves,
    };

    Ok(add_missing_exposure_counts(
        observed_counts,
        exposure_counts,
    ))
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
            injury_name: "Dor no braço".to_string(),
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

    #[test]
    fn test_count_injuries_by_severity_for_pilot_groups_critical_as_grave() {
        let mut conn = setup_test_db();
        let tx = conn.transaction().unwrap();

        let mut light = sample_injury();
        light.id = "I_LIGHT".to_string();
        light.active = false;
        insert_injury(&tx, &light).unwrap();

        let mut moderate = sample_injury();
        moderate.id = "I_MOD".to_string();
        moderate.injury_type = InjuryType::Moderada;
        moderate.active = false;
        insert_injury(&tx, &moderate).unwrap();

        let mut grave = sample_injury();
        grave.id = "I_GRAVE".to_string();
        grave.injury_type = InjuryType::Grave;
        grave.active = false;
        insert_injury(&tx, &grave).unwrap();

        let mut critical = sample_injury();
        critical.id = "I_CRIT".to_string();
        critical.injury_type = InjuryType::Critica;
        critical.active = false;
        insert_injury(&tx, &critical).unwrap();

        tx.commit().unwrap();

        let counts = count_injuries_by_severity_for_pilot(&conn, "P001").unwrap();
        assert_eq!(
            counts,
            InjurySeverityCounts {
                leves: 1,
                moderadas: 1,
                graves: 2,
            }
        );
    }

    #[test]
    fn test_count_injuries_by_severity_for_pilot_infers_legacy_collision_dnfs() {
        let conn = setup_test_db();
        conn.execute_batch(
            "
            PRAGMA foreign_keys = OFF;
            INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final, voltas_completadas,
                dnf, pontos, dnf_reason, incidents_count
            ) VALUES
                ('R_LEGACY_1', 'P001', 'T001', 4, 18, 8, 1, 0.0, 'Piloto Teste abandona apos colisao', 1),
                ('R_LEGACY_2', 'P001', 'T001', 5, 19, 6, 1, 0.0, 'Piloto Teste abandona por dano na suspensão após contato', 1),
                ('R_MECH', 'P001', 'T001', 6, 20, 3, 1, 0.0, 'Piloto Teste abandona por falha no câmbio', 1);
            PRAGMA foreign_keys = ON;
            ",
        )
        .expect("legacy race results");

        let counts = count_injuries_by_severity_for_pilot(&conn, "P001").unwrap();

        assert_eq!(
            counts,
            InjurySeverityCounts {
                leves: 0,
                moderadas: 2,
                graves: 0,
            }
        );
    }

    #[test]
    fn test_count_injuries_by_severity_for_pilot_backfills_experienced_archived_careers() {
        let conn = setup_test_db();
        for season in 1..=20 {
            conn.execute(
                "INSERT INTO driver_season_archive (
                    piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
                 ) VALUES (
                    'P001', ?1, ?2, 'Piloto Teste', 'gt4', 8, 42.0,
                    '{\"corridas\":8,\"vitorias\":0,\"podios\":1,\"pontos\":42,\"categoria\":\"gt4\"}'
                 )",
                params![season, 2000 + season],
            )
            .expect("archive season");
        }

        let counts = count_injuries_by_severity_for_pilot(&conn, "P001").unwrap();

        assert!(
            counts.leves + counts.moderadas + counts.graves >= 4,
            "20 seasons of archived racing should not look medically empty: {counts:?}"
        );
    }

    #[test]
    fn test_archived_exposure_target_counts_uses_one_to_five_percent_of_races() {
        let mid_counts = archived_exposure_target_counts("P250", 20, 250);
        let mid_total = injury_count_total(mid_counts);
        let long_counts = archived_exposure_target_counts("P1000", 30, 1000);
        let long_total = injury_count_total(long_counts);

        assert!(
            (3..=13).contains(&mid_total),
            "250 archived races should produce a 1-5% injury history, got {mid_total} from {mid_counts:?}"
        );
        assert!(
            (10..=50).contains(&long_total),
            "1000 archived races should keep scaling at 1-5%, got {long_total} from {long_counts:?}"
        );
    }

    #[test]
    fn test_count_injuries_by_severity_for_pilot_gives_every_ten_season_veteran_a_history() {
        let conn = setup_test_db();
        for pilot_index in 1..=20 {
            let pilot_id = format!("PX{pilot_index:03}");
            let mut driver = Driver::create_player(
                pilot_id.clone(),
                format!("Veterano {pilot_index}"),
                "BR".to_string(),
                35,
            );
            driver.is_jogador = false;
            insert_driver(&conn, &driver).unwrap();

            for season in 1..=10 {
                conn.execute(
                    "INSERT INTO driver_season_archive (
                        piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
                     ) VALUES (
                        ?1, ?2, ?3, ?4, 'gt4', 10, 30.0,
                        '{\"corridas\":7,\"vitorias\":0,\"podios\":0,\"pontos\":30,\"categoria\":\"gt4\"}'
                     )",
                    params![pilot_id, season, 2010 + season, format!("Veterano {pilot_index}")],
                )
                .expect("archive veteran season");
            }
        }

        let empty_veterans = (1..=20)
            .filter(|pilot_index| {
                let pilot_id = format!("PX{pilot_index:03}");
                let counts = count_injuries_by_severity_for_pilot(&conn, &pilot_id).unwrap();
                counts.leves + counts.moderadas + counts.graves == 0
            })
            .count();

        assert_eq!(empty_veterans, 0);
    }

    #[test]
    fn test_count_injuries_by_severity_for_pilot_returns_zero_without_injuries_table() {
        let conn = Connection::open_in_memory().unwrap();

        let counts = count_injuries_by_severity_for_pilot(&conn, "P001").unwrap();

        assert_eq!(counts, InjurySeverityCounts::default());
    }
}
