use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::models::driver::Driver;
use crate::models::enums::DriverStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetirementResult {
    pub should_retire: bool,
    pub reason: Option<String>,
}

pub fn check_retirement(
    driver: &Driver,
    consecutive_low_motivation_seasons: i32,
    has_severe_injury: bool,
    rng: &mut impl Rng,
) -> RetirementResult {
    if has_severe_injury && rng.gen::<f64>() < 0.40 {
        return RetirementResult {
            should_retire: true,
            reason: Some("Aposentou-se devido a lesao grave".to_string()),
        };
    }

    if driver.motivacao < 20.0 && consecutive_low_motivation_seasons >= 2 {
        return RetirementResult {
            should_retire: true,
            reason: Some("Aposentou-se por falta de motivacao".to_string()),
        };
    }

    let age = driver.idade;
    let skill = driver.atributos.skill;

    // Pilotos da IA que nunca competiram e já passaram da idade de estreia
    // dificilmente entrarão num grid — aposentam cedo para não se acumularem
    // como agentes livres eternos (órfãos que inflavam o mundo sem nunca correr).
    if !driver.is_jogador && driver.stats_carreira.corridas == 0 {
        let never_raced_chance = match age {
            27..=29 => 0.40,
            30..=32 => 0.70,
            33.. => 0.95,
            _ => 0.0,
        };
        if never_raced_chance > 0.0 && rng.gen::<f64>() < never_raced_chance {
            return RetirementResult {
                should_retire: true,
                reason: Some(format!("Aposentou-se aos {age} anos sem nunca competir")),
            };
        }
    }

    let chance = match age {
        36..=37 => {
            if skill < 35.0 {
                0.30
            } else {
                0.05
            }
        }
        38 => {
            if skill < 40.0 {
                0.35
            } else {
                0.15
            }
        }
        39 => 0.20,
        40 => 0.30,
        41 => 0.40,
        42 => 0.50,
        43 => 0.60,
        44 => 0.70,
        45 => 0.85,
        46 => 0.95,
        47.. => 1.00,
        _ => 0.0,
    };

    if chance > 0.0 && rng.gen::<f64>() < chance {
        return RetirementResult {
            should_retire: true,
            reason: Some(format!("Aposentou-se aos {} anos", age)),
        };
    }

    RetirementResult {
        should_retire: false,
        reason: None,
    }
}

pub fn process_retirement(driver: &mut Driver) {
    driver.status = DriverStatus::Aposentado;
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn test_no_retirement_young() {
        let driver = sample_driver(24, 60.0, 80.0);
        let mut rng = StdRng::seed_from_u64(1);

        let result = check_retirement(&driver, 0, false, &mut rng);

        assert!(!result.should_retire);
        assert!(result.reason.is_none());
    }

    #[test]
    fn test_guaranteed_retirement_47_plus() {
        let driver = sample_driver(47, 60.0, 60.0);
        let mut rng = StdRng::seed_from_u64(2);

        let result = check_retirement(&driver, 0, false, &mut rng);

        assert!(result.should_retire);
        assert_eq!(result.reason.as_deref(), Some("Aposentou-se aos 47 anos"));
    }

    #[test]
    fn test_low_motivation_retirement() {
        let driver = sample_driver(31, 60.0, 10.0);
        let mut rng = StdRng::seed_from_u64(3);

        let result = check_retirement(&driver, 2, false, &mut rng);

        assert!(result.should_retire);
        assert_eq!(
            result.reason.as_deref(),
            Some("Aposentou-se por falta de motivacao")
        );
    }

    fn sample_driver(age: u32, skill: f64, motivation: f64) -> Driver {
        let mut driver = Driver::new(
            "P004".to_string(),
            "Piloto Veteranissimo".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            age,
            2020,
        );
        driver.atributos.skill = skill;
        driver.motivacao = motivation;
        // Por padrão o piloto de teste já competiu na carreira (caso comum).
        driver.stats_carreira.corridas = 100;
        driver
    }

    #[test]
    fn test_never_raced_ai_retires_far_more_than_a_raced_peer() {
        let mut never_raced_retired = 0;
        let mut raced_retired = 0;

        for seed in 0..200 {
            let raced = sample_driver(31, 60.0, 80.0);
            let mut rng = StdRng::seed_from_u64(seed);
            if check_retirement(&raced, 0, false, &mut rng).should_retire {
                raced_retired += 1;
            }

            let mut never_raced = sample_driver(31, 60.0, 80.0);
            never_raced.stats_carreira.corridas = 0;
            let mut rng = StdRng::seed_from_u64(seed);
            if check_retirement(&never_raced, 0, false, &mut rng).should_retire {
                never_raced_retired += 1;
            }
        }

        // Aos 31 o veterano que correu nao tem chance por idade (0%), enquanto o
        // que nunca correu deve aposentar com folga (~70%).
        assert_eq!(raced_retired, 0);
        assert!(never_raced_retired > 100);
    }

    #[test]
    fn test_never_raced_player_is_never_force_retired() {
        for seed in 0..100 {
            let mut player = sample_driver(33, 60.0, 80.0);
            player.is_jogador = true;
            player.stats_carreira.corridas = 0;
            let mut rng = StdRng::seed_from_u64(seed);

            assert!(
                !check_retirement(&player, 0, false, &mut rng).should_retire,
                "jogador nunca deve ser aposentado pela regra de nunca-correu (seed {seed})"
            );
        }
    }

    #[test]
    fn test_never_raced_young_ai_is_not_retired() {
        for seed in 0..100 {
            let mut rookie = sample_driver(24, 60.0, 80.0);
            rookie.stats_carreira.corridas = 0;
            let mut rng = StdRng::seed_from_u64(seed);

            assert!(!check_retirement(&rookie, 0, false, &mut rng).should_retire);
        }
    }
}
