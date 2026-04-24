use rand::Rng;

use crate::evolution::growth::{get_attribute, set_attribute, AttributeChange};
use crate::models::driver::Driver;
use crate::models::driver_attributes::DriverAttributeKey;

const DECLINE_RATES: [(DriverAttributeKey, f64); 12] = [
    (DriverAttributeKey::Fitness, 1.5),
    (DriverAttributeKey::RitmoClassificacao, 1.2),
    (DriverAttributeKey::Skill, 1.0),
    (DriverAttributeKey::HabilidadeLargada, 0.8),
    (DriverAttributeKey::Consistencia, 0.3),
    (DriverAttributeKey::Confianca, 0.3),
    (DriverAttributeKey::FatorChuva, 0.2),
    (DriverAttributeKey::Mentalidade, 0.2),
    (DriverAttributeKey::Smoothness, 0.1),
    (DriverAttributeKey::Racecraft, 0.1),
    (DriverAttributeKey::Defesa, 0.1),
    (DriverAttributeKey::GestaoPneus, 0.1),
];

pub fn apply_age_decline(driver: &mut Driver, rng: &mut impl Rng) -> Vec<AttributeChange> {
    if driver.idade <= 32 {
        return Vec::new();
    }

    let mut changes = Vec::new();
    for (attribute, rate) in DECLINE_RATES {
        let chance = (((driver.idade as i32 - 32) as f64) * 0.12).min(0.8);
        if rng.gen::<f64>() >= chance {
            continue;
        }

        let decline = rng.gen_range(0.3..=1.5) * rate;
        let current = get_attribute(driver, attribute);
        let new_value = (current - decline).max(0.0);
        if let Some(change) = build_change(driver, attribute, current, new_value) {
            changes.push(change);
        }
    }

    let exp_gain = rng.gen_range(1..=3) as f64;
    let current_exp = driver.atributos.experiencia;
    let new_exp = (current_exp + exp_gain).min(100.0);
    if let Some(change) = build_change(
        driver,
        DriverAttributeKey::Experiencia,
        current_exp,
        new_exp,
    ) {
        changes.push(change);
    }

    changes
}

fn build_change(
    driver: &mut Driver,
    key: DriverAttributeKey,
    old_value: f64,
    new_value: f64,
) -> Option<AttributeChange> {
    if (old_value - new_value).abs() < f64::EPSILON {
        return None;
    }

    set_attribute(driver, key, new_value);

    let old_rounded = old_value.round().clamp(0.0, 100.0) as u8;
    let new_rounded = new_value.round().clamp(0.0, 100.0) as u8;
    if old_rounded == new_rounded {
        return None;
    }

    let reason = if key == DriverAttributeKey::Experiencia {
        format!("Experiencia acumulada aos {}", driver.idade)
    } else {
        format!("Declinio por idade ({})", driver.idade)
    };

    Some(AttributeChange {
        attribute: key.as_str().to_string(),
        old_value: old_rounded,
        new_value: new_rounded,
        delta: new_rounded as i8 - old_rounded as i8,
        reason,
    })
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn test_no_decline_under_33() {
        let mut driver = sample_driver(32);
        let before = driver.atributos.clone();
        let mut rng = StdRng::seed_from_u64(1);

        let changes = apply_age_decline(&mut driver, &mut rng);

        assert!(changes.is_empty());
        assert_eq!(driver.atributos.skill, before.skill);
        assert_eq!(driver.atributos.experiencia, before.experiencia);
    }

    #[test]
    fn test_decline_increases_with_age() {
        let younger_total = total_decline_for_age(34);
        let older_total = total_decline_for_age(41);

        assert!(older_total > younger_total);
    }

    #[test]
    fn test_experience_never_declines() {
        for seed in 0..100 {
            let mut driver = sample_driver(39);
            let old_exp = driver.atributos.experiencia;
            let mut rng = StdRng::seed_from_u64(seed);

            apply_age_decline(&mut driver, &mut rng);

            assert!(driver.atributos.experiencia >= old_exp);
        }
    }

    #[test]
    fn test_fitness_declines_fastest() {
        let mut fitness_drop = 0.0;
        let mut consistency_drop = 0.0;

        for seed in 0..200 {
            let mut driver = sample_driver(42);
            let old_fitness = driver.atributos.fitness;
            let old_consistency = driver.atributos.consistencia;
            let mut rng = StdRng::seed_from_u64(seed);

            apply_age_decline(&mut driver, &mut rng);

            fitness_drop += old_fitness - driver.atributos.fitness;
            consistency_drop += old_consistency - driver.atributos.consistencia;
        }

        assert!(fitness_drop > consistency_drop);
    }

    fn total_decline_for_age(age: u32) -> f64 {
        let mut total = 0.0;
        for seed in 0..200 {
            let mut driver = sample_driver(age);
            let before = driver.atributos.clone();
            let mut rng = StdRng::seed_from_u64(seed);
            apply_age_decline(&mut driver, &mut rng);

            total += (before.skill - driver.atributos.skill).max(0.0);
            total += (before.ritmo_classificacao - driver.atributos.ritmo_classificacao).max(0.0);
            total += (before.fitness - driver.atributos.fitness).max(0.0);
        }
        total
    }

    fn sample_driver(age: u32) -> Driver {
        let mut driver = Driver::new(
            "P002".to_string(),
            "Veterano".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            age,
            2024_u32.saturating_sub(age.saturating_sub(16)),
        );
        driver.atributos.skill = 70.0;
        driver.atributos.consistencia = 70.0;
        driver.atributos.racecraft = 70.0;
        driver.atributos.defesa = 70.0;
        driver.atributos.ritmo_classificacao = 70.0;
        driver.atributos.gestao_pneus = 70.0;
        driver.atributos.habilidade_largada = 70.0;
        driver.atributos.adaptabilidade = 70.0;
        driver.atributos.fator_chuva = 70.0;
        driver.atributos.fitness = 70.0;
        driver.atributos.experiencia = 70.0;
        driver.atributos.mentalidade = 70.0;
        driver.atributos.confianca = 70.0;
        driver.atributos.smoothness = 70.0;
        driver
    }
}
