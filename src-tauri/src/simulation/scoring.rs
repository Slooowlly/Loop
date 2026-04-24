use crate::constants::scoring::{get_points_for_position, BONUS_FASTEST_LAP};

use super::race::RaceDriverResult;

pub fn determine_fastest_lap(results: &mut [RaceDriverResult]) -> Option<String> {
    let fastest_id = results
        .iter()
        .filter(|result| !result.is_dnf)
        .min_by(|left, right| {
            left.best_lap_time_ms
                .partial_cmp(&right.best_lap_time_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|result| result.pilot_id.clone());

    if let Some(ref fastest_id) = fastest_id {
        for result in results.iter_mut() {
            result.has_fastest_lap = result.pilot_id == *fastest_id;
        }
    }

    fastest_id
}

pub fn assign_points(results: &mut Vec<RaceDriverResult>, is_endurance: bool) {
    for result in results.iter_mut() {
        if result.is_dnf {
            result.points_earned = 0;
            continue;
        }

        result.points_earned =
            get_points_for_position(result.finish_position as u8, is_endurance) as i32;
    }

    if let Some(fastest_driver) = results
        .iter()
        .find(|result| result.has_fastest_lap && !result.is_dnf && result.finish_position <= 10)
        .map(|result| result.pilot_id.clone())
    {
        if let Some(result) = results
            .iter_mut()
            .find(|result| result.pilot_id == fastest_driver)
        {
            result.points_earned += BONUS_FASTEST_LAP as i32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_result(position: i32) -> RaceDriverResult {
        RaceDriverResult {
            pilot_id: format!("P{position:03}"),
            pilot_name: format!("Driver {position}"),
            team_id: "T001".to_string(),
            team_name: "Team".to_string(),
            grid_position: position,
            finish_position: position,
            positions_gained: 0,
            best_lap_time_ms: 90_000.0 + position as f64 * 100.0,
            total_race_time_ms: 900_000.0 + position as f64 * 500.0,
            gap_to_winner_ms: if position == 1 {
                0.0
            } else {
                position as f64 * 500.0
            },
            is_dnf: false,
            dnf_reason: None,
            dnf_segment: None,
            incidents_count: 0,
            incidents: Vec::new(),
            has_fastest_lap: false,
            points_earned: 0,
            is_jogador: false,
            laps_completed: 12,
            final_tire_wear: 0.8,
            final_physical: 0.9,
            classification_status: crate::simulation::race::ClassificationStatus::Finished,
            notable_incident: None,
            dnf_catalog_id: None,
            damage_origin_segment: None,
        }
    }

    #[test]
    fn test_points_assigned_correctly() {
        let mut results = vec![sample_result(1), sample_result(2), sample_result(3)];
        assign_points(&mut results, false);
        assert_eq!(results[0].points_earned, 25);
    }

    #[test]
    fn test_fastest_lap_bonus() {
        let mut results = vec![sample_result(1), sample_result(2), sample_result(11)];
        results[1].best_lap_time_ms = 88_000.0;
        let fastest_id = determine_fastest_lap(&mut results);
        assign_points(&mut results, false);

        assert_eq!(fastest_id.as_deref(), Some("P002"));
        assert_eq!(results[1].points_earned, 19);
    }

    #[test]
    fn test_dnf_gets_zero_points() {
        let mut results = vec![sample_result(1), sample_result(2)];
        results[1].is_dnf = true;
        assign_points(&mut results, false);
        assert_eq!(results[1].points_earned, 0);
    }
}
