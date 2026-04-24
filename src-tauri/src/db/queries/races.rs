use crate::db::connection::DbError;
use crate::simulation::race::RaceDriverResult;
use rusqlite::Transaction;
use std::collections::HashSet;

pub fn insert_race_results_batch(
    tx: &Transaction<'_>,
    race_id: &str,
    results: &[RaceDriverResult],
) -> Result<(), DbError> {
    if results.is_empty() {
        return Err(DbError::InvalidData(format!(
            "batch de race_results vazio para corrida '{race_id}'"
        )));
    }

    let mut seen_pilots = HashSet::new();
    for result in results {
        if !seen_pilots.insert(result.pilot_id.clone()) {
            return Err(DbError::InvalidData(format!(
                "piloto duplicado no batch de race_results para corrida '{race_id}': {}",
                result.pilot_id
            )));
        }
    }

    let mut stmt = tx.prepare(
        "
        INSERT INTO race_results (
            race_id,
            piloto_id,
            equipe_id,
            posicao_largada,
            posicao_final,
            voltas_completadas,
            dnf,
            pontos,
            tempo_total,
            fastest_lap,
            dnf_reason,
            dnf_segment,
            incidents_count,
            gap_to_winner_ms,
            final_tire_wear,
            dnf_catalog_id,
            damage_origin_segment
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17
        )
        ",
    )?;

    for result in results {
        let dnf_int = if result.is_dnf { 1 } else { 0 };
        let fastest_lap_int = if result.has_fastest_lap { 1 } else { 0 };

        stmt.execute(rusqlite::params![
            race_id,
            result.pilot_id,
            result.team_id,
            result.grid_position,
            result.finish_position,
            result.laps_completed,
            dnf_int,
            result.points_earned as f64,
            result.total_race_time_ms,
            fastest_lap_int,
            result.dnf_reason,
            result.dnf_segment,
            result.incidents_count,
            result.gap_to_winner_ms,
            result.final_tire_wear,
            result.dnf_catalog_id,
            result.damage_origin_segment
        ])?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::teams::get_team_templates;
    use crate::db::migrations::run_all;
    use crate::db::queries::{
        drivers as driver_queries, seasons as season_queries, teams as team_queries,
    };
    use crate::models::driver::Driver;
    use crate::models::season::Season;
    use crate::models::team::Team;
    use crate::simulation::race::{ClassificationStatus, RaceDriverResult};
    use rand::{rngs::StdRng, SeedableRng};
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        run_all(&conn).unwrap();

        let season = Season::new("S001".to_string(), 1, 2026);
        season_queries::insert_season(&conn, &season).unwrap();
        conn.execute(
            "INSERT INTO calendar (id, temporada_id, rodada, pista, categoria, clima, duracao, data)
             VALUES ('C001', 'S001', 1, 'Interlagos', 'gt3', 'Seco', 60, '2026-01-01')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO races (id, temporada_id, calendar_id, rodada, pista, data, clima, status)
             VALUES ('R001', 'S001', 'C001', 1, 'Interlagos', '2026-01-01', 'Seco', 'Pendente')",
            [],
        )
        .unwrap();

        let mut driver = Driver::new(
            "P001".to_string(),
            "Driver 1".to_string(),
            "BR".to_string(),
            "M".to_string(),
            25,
            2020,
        );
        driver.categoria_atual = Some("gt3".to_string());
        driver_queries::insert_driver(&conn, &driver).unwrap();

        let template = get_team_templates("gt3")[0];
        let mut rng = StdRng::seed_from_u64(42);
        let team =
            Team::from_template_with_rng(template, "gt3", "T001".to_string(), 2026, &mut rng);
        team_queries::insert_team(&conn, &team).unwrap();

        conn
    }

    fn sample_result() -> RaceDriverResult {
        RaceDriverResult {
            pilot_id: "P001".to_string(),
            pilot_name: "Driver 1".to_string(),
            team_id: "T001".to_string(),
            team_name: "Team 1".to_string(),
            grid_position: 1,
            finish_position: 1,
            positions_gained: 0,
            best_lap_time_ms: 90000.0,
            total_race_time_ms: 3_600_000.0,
            gap_to_winner_ms: 0.0,
            is_dnf: false,
            dnf_reason: None,
            dnf_segment: None,
            incidents_count: 0,
            incidents: Vec::new(),
            has_fastest_lap: true,
            points_earned: 25,
            is_jogador: false,
            laps_completed: 20,
            final_tire_wear: 0.4,
            final_physical: 0.8,
            classification_status: ClassificationStatus::Finished,
            notable_incident: None,
            dnf_catalog_id: None,
            damage_origin_segment: None,
        }
    }

    #[test]
    fn test_insert_race_results_batch_rejects_empty_batch() {
        let mut conn = setup_test_db();
        let tx = conn.transaction().unwrap();

        let err = insert_race_results_batch(&tx, "R001", &[]).expect_err("empty batch should fail");
        assert!(err.to_string().contains("batch de race_results vazio"));
    }

    #[test]
    fn test_insert_race_results_batch_rejects_duplicate_pilot_in_same_race() {
        let mut conn = setup_test_db();
        let tx = conn.transaction().unwrap();

        let result = sample_result();
        let err = insert_race_results_batch(&tx, "R001", &[result.clone(), result])
            .expect_err("duplicate pilot should fail");
        assert!(err.to_string().contains("piloto duplicado"));
    }
}
