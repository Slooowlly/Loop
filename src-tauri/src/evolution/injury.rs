use rand::Rng;
use rusqlite::Transaction;
use std::collections::HashMap;

use crate::db::connection::DbError;
use crate::db::queries::drivers::update_driver_status;
use crate::db::queries::injuries::{
    get_active_injuries_for_category, has_active_injury_for_pilot, insert_injury,
    update_injury_status,
};
use crate::models::enums::DriverStatus;
use crate::models::injury::Injury;
use crate::simulation::incidents::IncidentResult;
use crate::simulation::injuries::generate_injury_from_incident;

pub fn process_injury_recovery(tx: &Transaction, category_id: &str) -> Result<(), DbError> {
    let active_injuries = get_active_injuries_for_category(tx, category_id)?;

    for mut injury in active_injuries {
        injury.races_remaining -= 1;

        if injury.races_remaining <= 0 {
            injury.races_remaining = 0;
            injury.active = false;
        }

        update_injury_status(tx, &injury.id, injury.races_remaining, injury.active)?;

        if !injury.active && !has_active_injury_for_pilot(tx, &injury.pilot_id)? {
            // Driver recovered
            update_driver_status(tx, &injury.pilot_id, &DriverStatus::Ativo)?;
        }
    }

    Ok(())
}

pub fn process_new_injuries(
    tx: &Transaction,
    season: i32,
    race_id: &str,
    incidents: &[IncidentResult],
    rng: &mut impl Rng,
) -> Result<Vec<Injury>, DbError> {
    let mut new_injuries = Vec::new();

    // Only max 1 injury per pilot per race. Let's group by pilot.
    let mut pilot_incidents: HashMap<String, Vec<&IncidentResult>> = HashMap::new();
    for inc in incidents {
        pilot_incidents
            .entry(inc.pilot_id.clone())
            .or_default()
            .push(inc);
    }

    for (pilot_id, pil_incidents) in pilot_incidents {
        if has_active_injury_for_pilot(tx, &pilot_id)? {
            continue;
        }

        // We just run generation on the first critical one we find (since it's max 1 per pilot anyway).
        for inc in pil_incidents {
            if let Some(injury) = generate_injury_from_incident(inc, season, race_id, rng) {
                // To avoid immediate decay, we will just insert it as active.
                insert_injury(tx, &injury)?;
                update_driver_status(tx, &pilot_id, &DriverStatus::Lesionado)?;
                new_injuries.push(injury);
                break; // Only 1 injury per pilot per race
            }
        }
    }

    Ok(new_injuries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::run_all;
    use crate::models::driver::Driver;
    use crate::models::enums::{DriverStatus, InjuryType};
    use crate::simulation::incidents::{IncidentSeverity, IncidentType};
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        run_all(&conn).unwrap();

        // Insert driver
        let driver = Driver::create_player(
            "P001".to_string(),
            "Senna".to_string(),
            "BR".to_string(),
            30,
        );
        crate::db::queries::drivers::insert_driver(&conn, &driver).unwrap();

        conn
    }

    #[test]
    fn test_process_injury_recovery_ticks_down_and_recovers() {
        let mut conn = setup_test_db();

        let tx = conn.transaction().unwrap();

        // Start by making the driver injured
        update_driver_status(&tx, "P001", &DriverStatus::Lesionado).unwrap();

        // Manually insert an injury with 2 races remaining
        let injury = crate::models::injury::Injury {
            id: "INJ-1".to_string(),
            pilot_id: "P001".to_string(),
            injury_type: InjuryType::Leve,
            injury_name: "Dor no braço".to_string(),
            modifier: 0.95,
            races_total: 2,
            races_remaining: 2,
            skill_penalty: 0.05,
            season: 1,
            race_occurred: "R001".to_string(),
            active: true,
        };
        insert_injury(&tx, &injury).unwrap();

        // Update driver's category just in case so the fetch queries it correctly
        tx.execute(
            "UPDATE drivers SET categoria_atual = 'F1' WHERE id = 'P001'",
            [],
        )
        .unwrap();

        // Tick 1
        process_injury_recovery(&tx, "F1").unwrap();

        let mut stmt = tx
            .prepare("SELECT races_remaining, active FROM injuries")
            .unwrap();
        let (rem, act): (i32, bool) = stmt
            .query_row([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap();
        assert_eq!(rem, 1);
        assert!(act);

        let status: String = tx
            .query_row("SELECT status FROM drivers WHERE id = 'P001'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "Lesionado");

        // Tick 2 (Recovers!)
        process_injury_recovery(&tx, "F1").unwrap();

        let (rem2, act2): (i32, bool) = tx
            .query_row("SELECT races_remaining, active FROM injuries", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(rem2, 0);
        assert!(!act2);

        let status2: String = tx
            .query_row("SELECT status FROM drivers WHERE id = 'P001'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status2, "Ativo");
    }

    #[test]
    fn test_process_new_injuries_generation() {
        let mut conn = setup_test_db();
        let tx = conn.transaction().unwrap();
        tx.execute(
            "UPDATE drivers SET categoria_atual = 'F1' WHERE id = 'P001'",
            [],
        )
        .unwrap();

        // Mock a 100% chance RNG for testing
        struct ForceInjuryRng;
        impl rand::RngCore for ForceInjuryRng {
            fn next_u32(&mut self) -> u32 {
                1
            } // Will roll 1 on gen_range -> Leve
            fn next_u64(&mut self) -> u64 {
                1
            }
            fn fill_bytes(&mut self, _dest: &mut [u8]) {}
            fn try_fill_bytes(&mut self, _dest: &mut [u8]) -> Result<(), rand::Error> {
                Ok(())
            }
        }

        let incident = IncidentResult {
            pilot_id: "P001".to_string(),
            incident_type: IncidentType::Collision,
            severity: IncidentSeverity::Critical, // Crucial
            segment: "Lap 1".to_string(),
            positions_lost: 20,
            is_dnf: true,
            description: "Huge crash".to_string(),
            linked_pilot_id: None,
            is_two_car_incident: false,
            injury_risk_multiplier: 1.5,
            narrative_importance_hint: 2,
            catalog_id: None,
            damage_origin_segment: None,
        };

        let mut rng = ForceInjuryRng;

        let new_injuries = process_new_injuries(&tx, 1, "R001", &[incident], &mut rng).unwrap();
        assert_eq!(new_injuries.len(), 1);
        assert_eq!(new_injuries[0].pilot_id, "P001");

        let count: i32 = tx
            .query_row("SELECT COUNT(*) FROM injuries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let status: String = tx
            .query_row("SELECT status FROM drivers WHERE id = 'P001'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "Lesionado");
    }

    #[test]
    fn test_process_injury_recovery_keeps_driver_injured_if_other_active_injury_remains() {
        let mut conn = setup_test_db();
        let tx = conn.transaction().unwrap();

        update_driver_status(&tx, "P001", &DriverStatus::Lesionado).unwrap();
        tx.execute(
            "UPDATE drivers SET categoria_atual = 'F1' WHERE id = 'P001'",
            [],
        )
        .unwrap();

        let first = crate::models::injury::Injury {
            id: "INJ-1".to_string(),
            pilot_id: "P001".to_string(),
            injury_type: InjuryType::Leve,
            injury_name: "Dor no braço".to_string(),
            modifier: 0.95,
            races_total: 1,
            races_remaining: 1,
            skill_penalty: 0.05,
            season: 1,
            race_occurred: "R001".to_string(),
            active: true,
        };
        let second = crate::models::injury::Injury {
            id: "INJ-2".to_string(),
            pilot_id: "P001".to_string(),
            injury_type: InjuryType::Moderada,
            injury_name: "Braço machucado".to_string(),
            modifier: 0.88,
            races_total: 3,
            races_remaining: 3,
            skill_penalty: 0.10,
            season: 1,
            race_occurred: "R002".to_string(),
            active: true,
        };
        insert_injury(&tx, &first).unwrap();
        tx.execute_batch("DROP INDEX IF EXISTS idx_injuries_single_active_per_pilot;")
            .unwrap();
        tx.execute(
            "INSERT INTO injuries (
                id, pilot_id, type, injury_name, modifier, races_total, races_remaining, skill_penalty, season, race_occurred, active
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                second.id,
                second.pilot_id,
                second.injury_type.as_str(),
                second.injury_name,
                second.modifier,
                second.races_total,
                second.races_remaining,
                second.skill_penalty,
                second.season,
                second.race_occurred,
                1,
            ],
        )
        .unwrap();

        process_injury_recovery(&tx, "F1").unwrap();

        let status: String = tx
            .query_row("SELECT status FROM drivers WHERE id = 'P001'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "Lesionado");

        let active_count: i32 = tx
            .query_row(
                "SELECT COUNT(*) FROM injuries WHERE pilot_id = 'P001' AND active = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(active_count, 1);
    }

    #[test]
    fn test_process_new_injuries_skips_driver_with_active_injury() {
        let mut conn = setup_test_db();
        let tx = conn.transaction().unwrap();
        tx.execute(
            "UPDATE drivers SET categoria_atual = 'F1' WHERE id = 'P001'",
            [],
        )
        .unwrap();
        update_driver_status(&tx, "P001", &DriverStatus::Lesionado).unwrap();

        let existing = crate::models::injury::Injury {
            id: "INJ-EXISTING".to_string(),
            pilot_id: "P001".to_string(),
            injury_type: InjuryType::Grave,
            injury_name: "Braço fraturado".to_string(),
            modifier: 0.75,
            races_total: 8,
            races_remaining: 5,
            skill_penalty: 0.15,
            season: 1,
            race_occurred: "R000".to_string(),
            active: true,
        };
        insert_injury(&tx, &existing).unwrap();

        struct ForceInjuryRng;
        impl rand::RngCore for ForceInjuryRng {
            fn next_u32(&mut self) -> u32 {
                1
            }
            fn next_u64(&mut self) -> u64 {
                1
            }
            fn fill_bytes(&mut self, _dest: &mut [u8]) {}
            fn try_fill_bytes(&mut self, _dest: &mut [u8]) -> Result<(), rand::Error> {
                Ok(())
            }
        }

        let incident = IncidentResult {
            pilot_id: "P001".to_string(),
            incident_type: IncidentType::Collision,
            severity: IncidentSeverity::Critical,
            segment: "Lap 1".to_string(),
            positions_lost: 20,
            is_dnf: true,
            description: "Huge crash".to_string(),
            linked_pilot_id: None,
            is_two_car_incident: false,
            injury_risk_multiplier: 1.5,
            narrative_importance_hint: 2,
            catalog_id: None,
            damage_origin_segment: None,
        };

        let mut rng = ForceInjuryRng;
        let new_injuries = process_new_injuries(&tx, 1, "R001", &[incident], &mut rng).unwrap();
        assert!(new_injuries.is_empty());

        let count: i32 = tx
            .query_row(
                "SELECT COUNT(*) FROM injuries WHERE pilot_id = 'P001'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
