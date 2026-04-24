#![allow(dead_code)]

use rusqlite::Connection;

use crate::common::time::current_timestamp;
use crate::db::connection::DbError;
use crate::simulation::incidents::IncidentType;
use crate::simulation::race::RaceDriverResult;

/// Registro de DNF em uma pista específica.
/// Usado para narrativas de "redenção" quando o piloto vence onde antes sofreu.
#[derive(Debug, Clone)]
pub struct TrackDnfRecord {
    pub id: String,
    pub piloto_id: String,
    pub track_name: String,
    pub season_num: i32,
    pub round: i32,
    pub dnf_reason: String,
    pub collision_with: Option<String>,
    pub created_at: String,
}

/// Insere um registro de DNF no histórico de pistas.
pub fn insert_track_dnf(conn: &Connection, record: &TrackDnfRecord) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO track_dnf_history
         (id, piloto_id, track_name, season_num, round, dnf_reason, collision_with, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            record.id,
            record.piloto_id,
            record.track_name,
            record.season_num,
            record.round,
            record.dnf_reason,
            record.collision_with,
            record.created_at,
        ],
    )?;
    Ok(())
}

/// Busca o DNF mais recente de um piloto em uma pista específica.
/// Retorna None se o piloto nunca abandonou nessa pista.
pub fn get_pilot_dnf_at_track(
    conn: &Connection,
    piloto_id: &str,
    track_name: &str,
) -> Result<Option<TrackDnfRecord>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, piloto_id, track_name, season_num, round, dnf_reason, collision_with, created_at
         FROM track_dnf_history
         WHERE piloto_id = ?1 AND track_name = ?2
         ORDER BY season_num DESC, round DESC
         LIMIT 1",
    )?;

    let result = stmt.query_row(rusqlite::params![piloto_id, track_name], |row| {
        Ok(TrackDnfRecord {
            id: row.get(0)?,
            piloto_id: row.get(1)?,
            track_name: row.get(2)?,
            season_num: row.get(3)?,
            round: row.get(4)?,
            dnf_reason: row.get(5)?,
            collision_with: row.get(6)?,
            created_at: row.get(7)?,
        })
    });

    match result {
        Ok(record) => Ok(Some(record)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DbError::Sqlite(e)),
    }
}

/// Registra todos os DNFs de uma corrida no histórico de pistas.
/// Deve ser chamado após persistir os resultados da corrida.
/// Erros individuais são logados mas não propagados — camada narrativa, não factual.
pub fn record_race_dnfs(
    conn: &Connection,
    race_results: &[RaceDriverResult],
    track_name: &str,
    season_num: i32,
    round: i32,
) -> Result<(), DbError> {
    for result in race_results {
        if !result.is_dnf {
            continue;
        }

        // Encontrar colisão que causou DNF, se houver
        let collision_with = result
            .incidents
            .iter()
            .find(|inc| inc.incident_type == IncidentType::Collision && inc.is_dnf)
            .and_then(|inc| inc.linked_pilot_id.clone());

        let record = TrackDnfRecord {
            id: build_track_dnf_id(season_num, round, &result.pilot_id, track_name),
            piloto_id: result.pilot_id.clone(),
            track_name: track_name.to_string(),
            season_num,
            round,
            dnf_reason: result
                .dnf_reason
                .clone()
                .unwrap_or_else(|| "Unknown".to_string()),
            collision_with,
            created_at: current_timestamp(),
        };

        insert_track_dnf(conn, &record)?;
    }

    Ok(())
}

fn build_track_dnf_id(season_num: i32, round: i32, pilot_id: &str, track_name: &str) -> String {
    let normalized_track = track_name
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() {
                char.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("DNF-{season_num}-{round}-{pilot_id}-{normalized_track}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    use crate::simulation::incidents::{IncidentResult, IncidentSeverity};

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE track_dnf_history (
                id TEXT PRIMARY KEY,
                piloto_id TEXT NOT NULL,
                track_name TEXT NOT NULL,
                season_num INTEGER NOT NULL,
                round INTEGER NOT NULL,
                dnf_reason TEXT NOT NULL,
                collision_with TEXT,
                created_at TEXT NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_insert_and_get_dnf() {
        let conn = setup_db();
        let record = TrackDnfRecord {
            id: "DNF-1-3-P001".to_string(),
            piloto_id: "P001".to_string(),
            track_name: "Interlagos".to_string(),
            season_num: 1,
            round: 3,
            dnf_reason: "Collision".to_string(),
            collision_with: Some("P002".to_string()),
            created_at: "2024-01-01T00:00:00".to_string(),
        };

        insert_track_dnf(&conn, &record).unwrap();

        let found = get_pilot_dnf_at_track(&conn, "P001", "Interlagos").unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.dnf_reason, "Collision");
        assert_eq!(found.collision_with, Some("P002".to_string()));
    }

    #[test]
    fn test_no_dnf_returns_none() {
        let conn = setup_db();
        let found = get_pilot_dnf_at_track(&conn, "P001", "Spa").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_returns_most_recent_dnf() {
        let conn = setup_db();

        insert_track_dnf(
            &conn,
            &TrackDnfRecord {
                id: "DNF-1-2-P001".to_string(),
                piloto_id: "P001".to_string(),
                track_name: "Monza".to_string(),
                season_num: 1,
                round: 2,
                dnf_reason: "Mechanical".to_string(),
                collision_with: None,
                created_at: "2024-01-01T00:00:00".to_string(),
            },
        )
        .unwrap();

        insert_track_dnf(
            &conn,
            &TrackDnfRecord {
                id: "DNF-2-5-P001".to_string(),
                piloto_id: "P001".to_string(),
                track_name: "Monza".to_string(),
                season_num: 2,
                round: 5,
                dnf_reason: "Collision".to_string(),
                collision_with: Some("P003".to_string()),
                created_at: "2025-01-01T00:00:00".to_string(),
            },
        )
        .unwrap();

        let found = get_pilot_dnf_at_track(&conn, "P001", "Monza")
            .unwrap()
            .unwrap();
        assert_eq!(found.season_num, 2);
        assert_eq!(found.round, 5);
        assert_eq!(found.dnf_reason, "Collision");
    }

    #[test]
    fn test_record_race_dnfs_uses_track_in_identifier() {
        let conn = setup_db();
        let race_results = vec![sample_dnf_result("P001", "Collision", Some("P002"))];

        record_race_dnfs(&conn, &race_results, "Interlagos", 1, 3).unwrap();
        record_race_dnfs(&conn, &race_results, "Spa-Francorchamps", 1, 3).unwrap();

        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM track_dnf_history", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_record_race_dnfs_propagates_insert_errors() {
        let conn = setup_db();
        let race_results = vec![sample_dnf_result("P001", "Collision", Some("P002"))];

        record_race_dnfs(&conn, &race_results, "Interlagos", 1, 3).unwrap();
        let err = record_race_dnfs(&conn, &race_results, "Interlagos", 1, 3)
            .expect_err("duplicate insert should fail");

        assert!(err.to_string().contains("UNIQUE"));
    }

    fn sample_dnf_result(
        pilot_id: &str,
        dnf_reason: &str,
        collision_with: Option<&str>,
    ) -> RaceDriverResult {
        RaceDriverResult {
            pilot_id: pilot_id.to_string(),
            pilot_name: format!("Pilot {}", pilot_id),
            team_id: "T001".to_string(),
            team_name: "Equipe".to_string(),
            grid_position: 1,
            finish_position: 20,
            positions_gained: -19,
            best_lap_time_ms: 0.0,
            total_race_time_ms: 0.0,
            gap_to_winner_ms: 0.0,
            is_dnf: true,
            dnf_reason: Some(dnf_reason.to_string()),
            dnf_segment: Some("EARLY".to_string()),
            incidents_count: 1,
            incidents: vec![IncidentResult {
                pilot_id: pilot_id.to_string(),
                incident_type: IncidentType::Collision,
                severity: IncidentSeverity::Major,
                segment: "EARLY".to_string(),
                positions_lost: 0,
                is_dnf: true,
                description: dnf_reason.to_string(),
                linked_pilot_id: collision_with.map(|id| id.to_string()),
                is_two_car_incident: collision_with.is_some(),
                injury_risk_multiplier: 1.0,
                narrative_importance_hint: 2,
                catalog_id: None,
                damage_origin_segment: None,
            }],
            has_fastest_lap: false,
            points_earned: 0,
            is_jogador: false,
            laps_completed: 0,
            final_tire_wear: 0.0,
            final_physical: 0.0,
            classification_status: crate::simulation::race::ClassificationStatus::Dnf,
            notable_incident: None,
            dnf_catalog_id: None,
            damage_origin_segment: None,
        }
    }
}
