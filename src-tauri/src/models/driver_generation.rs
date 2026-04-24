use std::collections::HashSet;

use rand::Rng;

use crate::constants::{scoring, skill_ranges};
use crate::generators::driver_helpers::{
    career_start_year_from_age, random_primary_personality, random_secondary_personality,
};
use crate::generators::names::generate_pilot_identity;
use crate::models::driver::{Driver, DriverAttributes};

pub fn generate_for_category(
    category_id: &str,
    category_tier: u8,
    difficulty: &str,
    count: usize,
    existing_names: &mut HashSet<String>,
    rng: &mut impl Rng,
) -> Vec<Driver> {
    let mut generated = 1_usize;
    generate_for_category_with_id_factory(
        category_id,
        category_tier,
        difficulty,
        count,
        existing_names,
        &mut || {
            let id = format!("PGEN-{}-{:03}", category_id, generated);
            generated += 1;
            id
        },
        rng,
    )
}

pub(crate) fn generate_for_category_with_id_factory<F, R>(
    category_id: &str,
    category_tier: u8,
    difficulty: &str,
    count: usize,
    existing_names: &mut HashSet<String>,
    id_factory: &mut F,
    rng: &mut R,
) -> Vec<Driver>
where
    F: FnMut() -> String,
    R: Rng,
{
    let normalized_tier = category_tier.min(4);
    let skill_range = skill_ranges::get_skill_range_by_tier(normalized_tier)
        .unwrap_or_else(|| skill_ranges::get_skill_range_by_tier(4).expect("skill range tier 4"));

    let difficulty_id = normalize_difficulty_id(difficulty);
    let difficulty_config = scoring::get_difficulty_config(difficulty_id)
        .or_else(|| scoring::get_difficulty_config("medio"))
        .expect("difficulty config should exist");

    let mut drivers = Vec::with_capacity(count);
    let guaranteed_prodigies = if normalized_tier == 0 {
        count.min(2)
    } else {
        0
    };

    for index in 0..count {
        let rookie_prodigy = index < guaranteed_prodigies;
        let identity = generate_pilot_identity(existing_names, rng);
        existing_names.insert(identity.nome_completo.clone());

        let idade = if rookie_prodigy {
            rng.gen_range(17..=19)
        } else {
            let (min_age, max_age) = tier_age_range(normalized_tier);
            rng.gen_range(min_age..=max_age)
        };

        let (skill_min, skill_max) =
            effective_skill_bounds(skill_range, difficulty_config, rookie_prodigy);
        let skill = if rookie_prodigy {
            roll_stat(rng, 60, 70)
        } else {
            roll_stat(rng, skill_min, skill_max)
        };

        let consistencia = correlated_stat(rng, skill, 10);
        let racecraft = correlated_stat(rng, skill, 8);
        let defesa = correlated_stat(rng, skill, 8);
        let ritmo_classificacao = correlated_stat(rng, skill, 12);
        let gestao_pneus = roll_stat(rng, 40, 70);
        let habilidade_largada = roll_stat(rng, 40, 70);
        let adaptabilidade = roll_stat(rng, 40, 70);
        let fator_chuva = roll_stat(rng, 30, 70);
        let fitness = fitness_for_age(rng, idade);
        let experiencia = experience_for_profile(rng, idade, normalized_tier, rookie_prodigy);
        let desenvolvimento = development_for_profile(rng, idade, skill, rookie_prodigy);
        let aggression = roll_stat(rng, 30, 70);
        let smoothness = inverse_correlated_stat(rng, aggression);
        let midia = roll_stat(rng, 30, 70);
        let mentalidade = roll_stat(rng, 40, 70);
        let confianca = roll_stat(rng, 50, 70);

        let ano_inicio = career_start_year_from_age(idade);
        let mut driver = Driver::new(
            id_factory(),
            identity.nome_completo,
            identity.nacionalidade_label,
            identity.genero,
            idade,
            ano_inicio,
        );
        driver.categoria_atual = Some(category_id.to_string());
        driver.personalidade_primaria = Some(random_primary_personality(rng));
        driver.personalidade_secundaria = Some(random_secondary_personality(rng));
        driver.motivacao = roll_stat(rng, 50, 80) as f64;
        driver.atributos = DriverAttributes {
            skill: skill as f64,
            consistencia: consistencia as f64,
            racecraft: racecraft as f64,
            defesa: defesa as f64,
            ritmo_classificacao: ritmo_classificacao as f64,
            gestao_pneus: gestao_pneus as f64,
            habilidade_largada: habilidade_largada as f64,
            adaptabilidade: adaptabilidade as f64,
            fator_chuva: fator_chuva as f64,
            fitness: fitness as f64,
            experiencia: experiencia as f64,
            desenvolvimento: desenvolvimento as f64,
            aggression: aggression as f64,
            smoothness: smoothness as f64,
            midia: midia as f64,
            mentalidade: mentalidade as f64,
            confianca: confianca as f64,
        };
        drivers.push(driver);
    }

    drivers
}

fn normalize_difficulty_id(input: &str) -> &'static str {
    match input.trim() {
        "facil" | "Facil" | "Fácil" => "facil",
        "medio" | "médio" | "Medio" | "Médio" | "Normal" | "normal" => "medio",
        "dificil" | "Difícil" | "Dificil" => "dificil",
        "lendario" | "lendário" | "Lendario" | "Lendário" | "Elite" | "elite" => "lendario",
        _ => "medio",
    }
}

fn effective_skill_bounds(
    range: &skill_ranges::SkillRangeConfig,
    difficulty: &scoring::DifficultyConfig,
    rookie_prodigy: bool,
) -> (u8, u8) {
    if rookie_prodigy {
        return (60, 70);
    }

    let min = range.skill_min.max(difficulty.skill_min_ia);
    let max = range.skill_max.min(difficulty.skill_max_ia);
    if min <= max {
        (min, max)
    } else {
        (range.skill_min, range.skill_max)
    }
}

fn roll_stat(rng: &mut impl Rng, min: u8, max: u8) -> u8 {
    rng.gen_range(min..=max)
}

fn correlated_stat(rng: &mut impl Rng, base: u8, variance: i16) -> u8 {
    let offset = rng.gen_range(-variance..=variance);
    clamp_stat(base as i16 + offset)
}

fn inverse_correlated_stat(rng: &mut impl Rng, aggression: u8) -> u8 {
    let offset = rng.gen_range(-10_i16..=10_i16);
    clamp_stat(100 - aggression as i16 + offset)
}

fn clamp_stat(value: i16) -> u8 {
    value.clamp(0, 100) as u8
}

fn tier_age_range(tier: u8) -> (u32, u32) {
    match tier {
        0 => (18, 24),
        1 => (20, 28),
        2 => (22, 31),
        3 => (24, 35),
        _ => (26, 40),
    }
}

fn fitness_for_age(rng: &mut impl Rng, age: u32) -> u8 {
    match age {
        0..=22 => roll_stat(rng, 70, 85),
        23..=32 => roll_stat(rng, 60, 75),
        33..=37 => roll_stat(rng, 50, 68),
        _ => roll_stat(rng, 40, 60),
    }
}

fn experience_for_profile(rng: &mut impl Rng, age: u32, tier: u8, rookie_prodigy: bool) -> u8 {
    let age_bonus = ((age.saturating_sub(17)) * 2) as i16;
    let tier_bonus = (tier as i16) * 10;
    let random_bonus = rng.gen_range(0_i16..=12_i16);
    let prodigy_bonus = if rookie_prodigy { 10 } else { 0 };
    clamp_stat(8 + age_bonus + tier_bonus + random_bonus + prodigy_bonus)
}

fn development_for_profile(rng: &mut impl Rng, age: u32, skill: u8, rookie_prodigy: bool) -> u8 {
    if rookie_prodigy || (age <= 21 && skill >= 60) {
        roll_stat(rng, 70, 90)
    } else if age >= 33 {
        roll_stat(rng, 20, 50)
    } else {
        roll_stat(rng, 40, 60)
    }
}
