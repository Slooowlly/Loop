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
        driver
    }
}
