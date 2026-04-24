use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::constants::scoring::QUALI_SCORE_TO_LAP_MS;

use super::context::{SimDriver, SimulationContext};
use super::math::{adjusted_weather_multiplier, normalize_car_performance};
use super::track_profile::TrackCharacter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualifyingResult {
    pub pilot_id: String,
    pub pilot_name: String,
    pub team_id: String,
    pub team_name: String,
    pub position: i32,
    pub quali_score: f64,
    pub best_lap_time_ms: f64,
    pub gap_to_pole_ms: f64,
    pub is_pole: bool,
    pub is_jogador: bool,
}

/// Retorna os pesos (skill, ritmo_classificacao, car_performance, adaptabilidade)
/// adaptados ao caráter da pista. Somam 1.0.
fn qual_weights(character: TrackCharacter) -> (f64, f64, f64, f64) {
    match character {
        TrackCharacter::Flowing => (0.43, 0.27, 0.27, 0.03), // velocidade/carro dominam
        TrackCharacter::Technical => (0.40, 0.25, 0.25, 0.10), // baseline equilibrado
        TrackCharacter::Tight => (0.35, 0.22, 0.18, 0.25),   // adaptabilidade dominante
        TrackCharacter::Roval => (0.45, 0.25, 0.25, 0.05),   // skill/velocidade dominam
    }
}

pub fn simulate_qualifying(
    drivers: &[SimDriver],
    ctx: &SimulationContext,
    rng: &mut impl Rng,
) -> Vec<QualifyingResult> {
    let (w_skill, w_ritmo, w_car, w_adapt) = qual_weights(ctx.track_character);

    let mut results: Vec<QualifyingResult> = drivers
        .iter()
        .map(|driver| {
            let mut score = driver.skill as f64 * w_skill
                + driver.ritmo_classificacao as f64 * w_ritmo
                + normalize_car_performance(driver.car_performance) * w_car
                + driver.adaptabilidade as f64 * w_adapt;

            // Chuva com sensibilidade do contexto (fórmula canônica)
            score *=
                adjusted_weather_multiplier(ctx.weather, driver.fator_chuva, ctx.rain_sensitivity);

            if driver.corridas_na_categoria < 10 {
                let experience_penalty = (10 - driver.corridas_na_categoria) as f64 * 0.005;
                score *= 1.0 - experience_penalty;
            }

            // Variância escalada pelo perfil da categoria
            let variance_range = (100.0 - driver.consistencia as f64) / 100.0
                * 8.0
                * ctx.qualifying_variance_multiplier;
            score += rng.gen_range(-variance_range..=variance_range);
            score = score.max(10.0);

            QualifyingResult {
                pilot_id: driver.id.clone(),
                pilot_name: driver.nome.clone(),
                team_id: driver.team_id.clone(),
                team_name: driver.team_name.clone(),
                position: 0,
                quali_score: score,
                best_lap_time_ms: 0.0,
                gap_to_pole_ms: 0.0,
                is_pole: false,
                is_jogador: driver.is_jogador,
            }
        })
        .collect();

    results.sort_by(|a, b| {
        b.quali_score
            .partial_cmp(&a.quali_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let pole_score = results
        .first()
        .map(|result| result.quali_score)
        .unwrap_or(0.0);
    let pole_time = ctx.base_lap_time_ms;

    for (index, result) in results.iter_mut().enumerate() {
        result.position = index as i32 + 1;
        result.is_pole = index == 0;
        result.best_lap_time_ms = ctx.base_lap_time_ms
            + (pole_score - result.quali_score).max(0.0) * QUALI_SCORE_TO_LAP_MS;
        result.gap_to_pole_ms = (result.best_lap_time_ms - pole_time).max(0.0);
    }

    results
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use crate::models::driver::Driver;
    use crate::models::enums::WeatherCondition;
    use crate::models::team::placeholder_team_from_db;
    use crate::simulation::car_build::CarBuildProfile;
    use crate::simulation::context::SimulationContext;
    use crate::simulation::track_profile::TrackCharacter;

    use super::*;

    fn sample_context(weather: WeatherCondition) -> SimulationContext {
        SimulationContext {
            weather,
            ..SimulationContext::test_default()
        }
    }

    fn build_driver(id: &str, skill: f64, quali: f64, rain: f64, consistency: f64) -> SimDriver {
        let mut driver = Driver::create_player(
            id.to_string(),
            format!("Driver {}", id),
            "🇧🇷 Brasileiro".to_string(),
            20,
        );
        driver.is_jogador = false;
        driver.atributos.skill = skill;
        driver.atributos.ritmo_classificacao = quali;
        driver.atributos.fator_chuva = rain;
        driver.atributos.consistencia = consistency;
        driver.atributos.adaptabilidade = 60.0;

        let mut team = placeholder_team_from_db(
            format!("T{}", id),
            format!("Team {}", id),
            "mazda_rookie".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.car_performance = 8.0;

        SimDriver::from_driver_and_team(&driver, &team)
    }

    fn sample_grid() -> Vec<SimDriver> {
        (0..12)
            .map(|index| {
                build_driver(
                    &format!("{:03}", index + 1),
                    50.0 + index as f64,
                    48.0 + index as f64,
                    50.0,
                    85.0,
                )
            })
            .collect()
    }

    fn build_driver_for_track(
        id: &str,
        profile: CarBuildProfile,
        track_id: u32,
        track_character: TrackCharacter,
    ) -> (SimDriver, SimulationContext) {
        let mut driver = Driver::create_player(
            id.to_string(),
            format!("Driver {}", id),
            "Brasileiro".to_string(),
            20,
        );
        driver.is_jogador = false;
        driver.atributos.skill = 75.0;
        driver.atributos.ritmo_classificacao = 75.0;
        driver.atributos.fator_chuva = 50.0;
        driver.atributos.consistencia = 80.0;
        driver.atributos.adaptabilidade = 65.0;

        let mut team = placeholder_team_from_db(
            format!("T{}", id),
            format!("Team {}", id),
            "gt4".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.car_performance = 8.0;
        team.car_build_profile = profile;

        let ctx = SimulationContext {
            track_id,
            track_character,
            ..SimulationContext::test_default()
        };

        (
            SimDriver::from_driver_team_and_track(&driver, &team, track_id),
            ctx,
        )
    }

    #[test]
    fn test_qualifying_returns_all_drivers() {
        let mut rng = StdRng::seed_from_u64(11);
        let results = simulate_qualifying(
            &sample_grid(),
            &sample_context(WeatherCondition::Dry),
            &mut rng,
        );
        assert_eq!(results.len(), 12);
    }

    #[test]
    fn test_qualifying_positions_sequential() {
        let mut rng = StdRng::seed_from_u64(12);
        let results = simulate_qualifying(
            &sample_grid(),
            &sample_context(WeatherCondition::Dry),
            &mut rng,
        );
        let positions: Vec<i32> = results.iter().map(|result| result.position).collect();
        assert_eq!(positions, (1..=12).collect::<Vec<_>>());
    }

    #[test]
    fn test_qualifying_higher_skill_tends_to_pole() {
        let elite = build_driver("A", 95.0, 95.0, 50.0, 90.0);
        let grid: Vec<SimDriver> = std::iter::once(elite.clone())
            .chain((0..11).map(|index| build_driver(&format!("B{index}"), 55.0, 55.0, 50.0, 75.0)))
            .collect();

        let mut top3_finishes = 0;
        for seed in 0..100 {
            let mut rng = StdRng::seed_from_u64(seed);
            let results =
                simulate_qualifying(&grid, &sample_context(WeatherCondition::Dry), &mut rng);
            let pos = results
                .iter()
                .find(|result| result.pilot_id == elite.id)
                .expect("elite result")
                .position;
            if pos <= 3 {
                top3_finishes += 1;
            }
        }

        assert!(
            top3_finishes >= 80,
            "elite driver only reached top3 {} times",
            top3_finishes
        );
    }

    #[test]
    fn test_qualifying_rain_penalty_applied() {
        let wet_specialist = build_driver("RAIN", 75.0, 75.0, 95.0, 90.0);
        let wet_weak = build_driver("DRY", 75.0, 75.0, 20.0, 90.0);
        let grid = vec![wet_specialist.clone(), wet_weak.clone()];

        let mut better_in_rain = 0;
        for seed in 0..30 {
            let mut rng = StdRng::seed_from_u64(seed);
            let results = simulate_qualifying(
                &grid,
                &sample_context(WeatherCondition::HeavyRain),
                &mut rng,
            );
            if results[0].pilot_id == wet_specialist.id {
                better_in_rain += 1;
            }
        }

        assert!(better_in_rain >= 20);
    }

    #[test]
    fn test_qualifying_gap_to_pole() {
        let mut rng = StdRng::seed_from_u64(14);
        let results = simulate_qualifying(
            &sample_grid(),
            &sample_context(WeatherCondition::Dry),
            &mut rng,
        );
        assert_eq!(results[0].gap_to_pole_ms, 0.0);
        assert!(results
            .iter()
            .skip(1)
            .all(|result| result.gap_to_pole_ms > 0.0));
    }

    #[test]
    fn test_qualifying_lap_times_ordered() {
        let mut rng = StdRng::seed_from_u64(15);
        let results = simulate_qualifying(
            &sample_grid(),
            &sample_context(WeatherCondition::Dry),
            &mut rng,
        );
        assert!(results
            .windows(2)
            .all(|window| window[0].best_lap_time_ms <= window[1].best_lap_time_ms));
    }

    #[test]
    fn test_tight_track_favors_adaptability_over_flowing() {
        use super::qual_weights;
        let (_, _, _, tight_adapt) = qual_weights(TrackCharacter::Tight);
        let (_, _, _, flowing_adapt) = qual_weights(TrackCharacter::Flowing);
        assert!(
            tight_adapt > flowing_adapt,
            "Tight adapt weight={} should > Flowing={}",
            tight_adapt,
            flowing_adapt
        );
    }

    #[test]
    fn test_roval_favors_skill_over_tight() {
        use super::qual_weights;
        let (roval_skill, _, _, _) = qual_weights(TrackCharacter::Roval);
        let (tight_skill, _, _, _) = qual_weights(TrackCharacter::Tight);
        assert!(
            roval_skill > tight_skill,
            "Roval skill weight={} should > Tight={}",
            roval_skill,
            tight_skill
        );
    }

    #[test]
    fn test_gt3_has_less_variance_than_rookie() {
        use crate::simulation::profile::resolve_simulation_profile;

        let rookie_profile =
            resolve_simulation_profile("mazda_rookie", 47, 22.0, WeatherCondition::Dry, 15, 12);
        let gt3_profile =
            resolve_simulation_profile("gt3", 47, 22.0, WeatherCondition::Dry, 50, 20);

        let rookie_ctx = SimulationContext {
            category_id: "mazda_rookie".to_string(),
            category_tier: 0,
            qualifying_variance_multiplier: rookie_profile.qualifying_variance_multiplier,
            base_lap_time_ms: rookie_profile.base_lap_time_ms,
            ..SimulationContext::test_default()
        };
        let gt3_ctx = SimulationContext {
            category_id: "gt3".to_string(),
            category_tier: 4,
            qualifying_variance_multiplier: gt3_profile.qualifying_variance_multiplier,
            base_lap_time_ms: gt3_profile.base_lap_time_ms,
            ..SimulationContext::test_default()
        };

        // Grid uniforme para isolar o efeito da variância
        let grid: Vec<SimDriver> = (0..10)
            .map(|i| build_driver(&format!("D{i}"), 70.0, 70.0, 50.0, 80.0))
            .collect();

        let mut rookie_spread_sum = 0.0_f64;
        let mut gt3_spread_sum = 0.0_f64;
        let runs = 50;

        for seed in 0..runs {
            let mut rng = StdRng::seed_from_u64(seed);
            let rookie_results = simulate_qualifying(&grid, &rookie_ctx, &mut rng);
            let rookie_scores: Vec<f64> = rookie_results.iter().map(|r| r.quali_score).collect();
            let rookie_max = rookie_scores
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max);
            let rookie_min = rookie_scores.iter().cloned().fold(f64::INFINITY, f64::min);
            rookie_spread_sum += rookie_max - rookie_min;

            let mut rng2 = StdRng::seed_from_u64(seed + 1000);
            let gt3_results = simulate_qualifying(&grid, &gt3_ctx, &mut rng2);
            let gt3_scores: Vec<f64> = gt3_results.iter().map(|r| r.quali_score).collect();
            let gt3_max = gt3_scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let gt3_min = gt3_scores.iter().cloned().fold(f64::INFINITY, f64::min);
            gt3_spread_sum += gt3_max - gt3_min;
        }

        let rookie_avg_spread = rookie_spread_sum / runs as f64;
        let gt3_avg_spread = gt3_spread_sum / runs as f64;

        assert!(
            rookie_avg_spread > gt3_avg_spread,
            "rookie avg spread={:.2} should > gt3 avg spread={:.2}",
            rookie_avg_spread,
            gt3_avg_spread
        );
    }

    #[test]
    fn test_qualifying_power_profile_beats_wrong_profile_at_monza() {
        let (power_driver, ctx) = build_driver_for_track(
            "PWR",
            CarBuildProfile::PowerExtreme,
            93,
            TrackCharacter::Flowing,
        );
        let (accel_driver, _) = build_driver_for_track(
            "ACC",
            CarBuildProfile::AccelerationExtreme,
            93,
            TrackCharacter::Flowing,
        );

        let mut rng = StdRng::seed_from_u64(123);
        let results = simulate_qualifying(
            &[power_driver.clone(), accel_driver.clone()],
            &ctx,
            &mut rng,
        );

        let power_pos = results
            .iter()
            .find(|result| result.pilot_id == power_driver.id)
            .expect("power result")
            .position;
        let accel_pos = results
            .iter()
            .find(|result| result.pilot_id == accel_driver.id)
            .expect("accel result")
            .position;

        assert!(
            power_pos < accel_pos,
            "expected power profile to qualify ahead at Monza"
        );
    }
}
