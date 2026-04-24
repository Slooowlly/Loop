use std::collections::HashSet;

use rand::Rng;

use crate::generators::driver_helpers::{
    career_start_year_from_age, random_primary_personality, random_secondary_personality,
};
use crate::generators::names::generate_pilot_identity;
use crate::models::driver::{Driver, DriverAttributes};
use crate::models::enums::DriverStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RookieType {
    Comum,
    Talento,
    Genio,
}

pub fn generate_rookies(
    count: usize,
    existing_names: &mut HashSet<String>,
    rng: &mut impl Rng,
) -> Vec<Driver> {
    (0..count)
        .map(|index| generate_single_rookie(index, existing_names, rng))
        .collect()
}

pub fn classify_rookie(skill: u8) -> &'static str {
    if skill >= 56 {
        "Genio"
    } else if skill >= 40 {
        "Talento"
    } else {
        "Comum"
    }
}

fn generate_single_rookie(
    index: usize,
    existing_names: &mut HashSet<String>,
    rng: &mut impl Rng,
) -> Driver {
    let rookie_type = roll_rookie_type(rng);
    let identity = generate_pilot_identity(existing_names, rng);
    existing_names.insert(identity.nome_completo.clone());

    let age = rng.gen_range(16..=20);
    let skill = match rookie_type {
        RookieType::Comum => rng.gen_range(25..=45),
        RookieType::Talento => rng.gen_range(40..=55),
        RookieType::Genio => rng.gen_range(55..=70),
    } as f64;

    let development = match rookie_type {
        RookieType::Comum => rng.gen_range(40..=60),
        RookieType::Talento | RookieType::Genio => rng.gen_range(60..=90),
    } as f64;

    let aggression = rng.gen_range(30..=70) as f64;
    let smoothness = (100.0 - aggression + rng.gen_range(-10.0..=10.0)).clamp(0.0, 100.0);
    let fitness = match age {
        16..=17 => rng.gen_range(75..=90),
        18..=19 => rng.gen_range(70..=85),
        _ => rng.gen_range(65..=82),
    } as f64;
    let ano_inicio = career_start_year_from_age(age as u32);

    let mut driver = Driver::new(
        format!("ROOKIE-TMP-{:03}", index + 1),
        identity.nome_completo,
        identity.nacionalidade_label,
        identity.genero,
        age as u32,
        ano_inicio,
    );
    driver.categoria_atual = None;
    driver.status = DriverStatus::Ativo;
    driver.personalidade_primaria = Some(random_primary_personality(rng));
    driver.personalidade_secundaria = Some(random_secondary_personality(rng));
    driver.motivacao = rng.gen_range(70..=90) as f64;
    driver.atributos = DriverAttributes {
        skill,
        consistencia: correlated_stat(skill, 12.0, rng),
        racecraft: correlated_stat(skill, 10.0, rng),
        defesa: correlated_stat(skill, 10.0, rng),
        ritmo_classificacao: correlated_stat(skill, 12.0, rng),
        gestao_pneus: rng.gen_range(35..=68) as f64,
        habilidade_largada: rng.gen_range(35..=72) as f64,
        adaptabilidade: rng.gen_range(40..=75) as f64,
        fator_chuva: rng.gen_range(30..=70) as f64,
        fitness,
        experiencia: rng.gen_range(5..=25) as f64,
        desenvolvimento: development,
        aggression,
        smoothness,
        midia: rng.gen_range(20..=55) as f64,
        mentalidade: rng.gen_range(40..=75) as f64,
        confianca: rng.gen_range(55..=80) as f64,
    };
    driver
}

fn roll_rookie_type(rng: &mut impl Rng) -> RookieType {
    let roll = rng.gen::<f64>();
    if roll < 0.05 {
        RookieType::Genio
    } else if roll < 0.30 {
        RookieType::Talento
    } else {
        RookieType::Comum
    }
}

fn correlated_stat(base: f64, variance: f64, rng: &mut impl Rng) -> f64 {
    (base + rng.gen_range(-variance..=variance)).clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn test_generate_rookies_count() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut existing_names = HashSet::new();

        let rookies = generate_rookies(4, &mut existing_names, &mut rng);

        assert_eq!(rookies.len(), 4);
    }

    #[test]
    fn test_rookie_age_range() {
        let mut rng = StdRng::seed_from_u64(2);
        let mut existing_names = HashSet::new();

        let rookies = generate_rookies(20, &mut existing_names, &mut rng);

        assert!(rookies
            .iter()
            .all(|rookie| (16..=20).contains(&(rookie.idade as i32))));
        assert!(rookies
            .iter()
            .all(|rookie| rookie.categoria_atual.is_none()));
    }

    #[test]
    fn test_rookie_types_distribution() {
        let mut common = 0;
        let mut talent = 0;
        let mut genius = 0;

        for seed in 0..400 {
            let mut rng = StdRng::seed_from_u64(seed);
            let mut existing_names = HashSet::new();
            let rookie = generate_rookies(1, &mut existing_names, &mut rng)
                .into_iter()
                .next()
                .expect("rookie");

            match classify_rookie(rookie.atributos.skill.round() as u8) {
                "Comum" => common += 1,
                "Talento" => talent += 1,
                "Genio" => genius += 1,
                _ => unreachable!(),
            }
        }

        assert!(common > talent);
        assert!(talent > genius);
        assert!(genius > 0);
    }
}
