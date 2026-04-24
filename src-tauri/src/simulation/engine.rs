use rand::Rng;

use super::catalog::IncidentCatalog;
use super::context::{SimDriver, SimulationContext};
use super::qualifying::simulate_qualifying;
use super::race::{simulate_race, RaceResult};
use super::scoring::{assign_points, determine_fastest_lap};

pub fn run_full_race(
    drivers: &[SimDriver],
    ctx: &SimulationContext,
    is_endurance: bool,
    catalog: &IncidentCatalog,
    rng: &mut impl Rng,
) -> RaceResult {
    let qualifying = simulate_qualifying(drivers, ctx, rng);
    let mut race_result = simulate_race(drivers, &qualifying, ctx, catalog, is_endurance, rng);

    let fastest_lap_id = determine_fastest_lap(&mut race_result.race_results).unwrap_or_default();
    assign_points(&mut race_result.race_results, is_endurance);
    race_result.fastest_lap_id = fastest_lap_id;

    race_result
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use crate::models::driver::Driver;
    use crate::models::enums::WeatherCondition;
    use crate::models::team::placeholder_team_from_db;

    use super::*;

    fn build_driver(index: usize) -> SimDriver {
        let mut driver = Driver::create_player(
            format!("P{:03}", index + 1),
            format!("Driver {}", index + 1),
            "🇧🇷 Brasileiro".to_string(),
            20,
        );
        driver.is_jogador = false;
        driver.atributos.skill = 60.0 + index as f64;
        driver.atributos.consistencia = 85.0;
        driver.atributos.racecraft = 68.0 + index as f64 * 0.3;
        driver.atributos.ritmo_classificacao = 62.0 + index as f64;
        driver.atributos.gestao_pneus = 65.0;
        driver.atributos.habilidade_largada = 70.0;
        driver.atributos.adaptabilidade = 68.0;
        driver.atributos.fator_chuva = 55.0;
        driver.atributos.fitness = 72.0;
        driver.atributos.mentalidade = 70.0;
        driver.atributos.confianca = 69.0;

        let mut team = placeholder_team_from_db(
            format!("T{:03}", index + 1),
            format!("Team {}", index + 1),
            "mazda_rookie".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.car_performance = 7.0 + index as f64 * 0.2;

        SimDriver::from_driver_and_team(&driver, &team)
    }

    fn sample_context() -> SimulationContext {
        SimulationContext {
            category_id: "mazda_rookie".to_string(),
            category_tier: 0,
            track_id: 1,
            track_name: "Laguna Seca".to_string(),
            weather: WeatherCondition::Dry,
            temperature: 25.0,
            total_laps: 12,
            race_duration_minutes: 20,
            is_championship_deciding: false,
            base_lap_time_ms: 90_000.0,
            tire_degradation_rate: 0.02,
            physical_degradation_rate: 0.01,
            incidents_enabled: false,
            ..SimulationContext::test_default()
        }
    }

    #[test]
    fn test_full_race_integration() {
        let drivers: Vec<SimDriver> = (0..12).map(build_driver).collect();
        let mut rng = StdRng::seed_from_u64(41);
        let result = run_full_race(
            &drivers,
            &sample_context(),
            false,
            &IncidentCatalog::empty(),
            &mut rng,
        );

        assert_eq!(result.qualifying_results.len(), 12);
        assert_eq!(result.race_results.len(), 12);
        assert!(!result.winner_id.is_empty());
        assert!(!result.fastest_lap_id.is_empty());
    }

    #[test]
    fn test_full_race_positions_consistent() {
        let drivers: Vec<SimDriver> = (0..12).map(build_driver).collect();
        let mut rng = StdRng::seed_from_u64(42);
        let result = run_full_race(
            &drivers,
            &sample_context(),
            false,
            &IncidentCatalog::empty(),
            &mut rng,
        );

        assert!(result.race_results[0].points_earned > result.race_results[1].points_earned);
    }
}
