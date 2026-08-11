use std::collections::HashSet;

use rand::Rng;

use crate::constants::{
    categories::{get_category_config, CategoryConfig},
    scoring, skill_ranges,
};
use crate::generators::driver_helpers::{
    career_start_year_from_age, random_primary_personality, random_secondary_personality,
    roll_carisma,
};
use crate::generators::names::generate_pilot_identity;
use crate::models::driver::{potential_headroom, Driver, DriverAttributes, POTENTIAL_HARD_MAX};

const ROOKIE_PRODIGY_CHANCE_PERCENT: u8 = 5;
const ROOKIE_COMMON_FLAW_MAX: u8 = 32;
const ROOKIE_HEAVY_FLAW_MAX: u8 = 24;

/// Geração avulsa, com o ano do mundo tirado do RELÓGIO DA MÁQUINA.
///
/// Só serve a quem gera um piloto solto sem ter a temporada em mãos (o comando de
/// debug e os testes deste crate). Todo gerador de MUNDO passa o ano explicitamente
/// por [`generate_for_category_in_year`] — ver a nota em
/// [`crate::generators::driver_helpers::career_start_year_from_age`].
pub fn generate_for_category(
    category_id: &str,
    category_tier: u8,
    difficulty: &str,
    count: usize,
    existing_names: &mut HashSet<String>,
    rng: &mut impl Rng,
) -> Vec<Driver> {
    generate_for_category_in_year(
        category_id,
        category_tier,
        difficulty,
        count,
        crate::common::time::current_year(),
        existing_names,
        rng,
    )
}

pub fn generate_for_category_in_year(
    category_id: &str,
    category_tier: u8,
    difficulty: &str,
    count: usize,
    world_year: u32,
    existing_names: &mut HashSet<String>,
    rng: &mut impl Rng,
) -> Vec<Driver> {
    let mut generated = 1_usize;
    generate_for_category_with_id_factory(
        category_id,
        category_tier,
        difficulty,
        count,
        world_year,
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
    world_year: u32,
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

    for index in 0..count {
        let rookie_profile = if normalized_tier == 0 {
            Some(rookie_profile_for_slot(index, count))
        } else {
            None
        };
        let rookie_prodigy =
            rookie_profile.is_some() && rng.gen_range(0_u8..100_u8) < ROOKIE_PRODIGY_CHANCE_PERCENT;
        let identity = generate_pilot_identity(existing_names, rng);
        existing_names.insert(identity.nome_completo.clone());

        let idade = roll_age_for_profile(normalized_tier, rookie_prodigy, rng);

        let (skill_min, skill_max) = effective_skill_bounds(
            skill_range,
            difficulty_config,
            rookie_profile.is_some(),
            rookie_prodigy,
        );
        let skill = if rookie_prodigy {
            roll_stat(rng, 63, 74)
        } else {
            roll_stat(rng, skill_min, skill_max)
        };

        let consistencia = correlated_stat(rng, skill, 10);
        let racecraft = correlated_stat(rng, skill, 8);
        let defesa = correlated_stat(rng, skill, 8);
        let ritmo_classificacao = correlated_stat(rng, skill, 12);
        // Os EIXOS LIVRES nascem largos pela mesma razao que os de estilo, e por
        // uma a mais. Eles sao os unicos que NAO acompanham a escada — a
        // calibracao exige |rho| < 0,15 contra o talento —, entao numa categoria
        // de base eles sao a unica fonte possivel de diferenca entre um piloto e
        // o vizinho: os eixos correlacionados a skill vivem todos amontoados na
        // faixa do tier. Em 40–70 a mediana do grid ficava a menos de dez pontos
        // de todo mundo, e um grid inteiro de rookies saia sem um traco sequer.
        let gestao_pneus = roll_stat(rng, 34, 78);
        let habilidade_largada = roll_stat(rng, 34, 78);
        let adaptabilidade = roll_stat(rng, 34, 78);
        let fator_chuva = roll_stat(rng, 26, 78);
        let fitness = fitness_for_age(rng, idade);
        let experiencia = experience_for_profile(rng, idade, normalized_tier, rookie_prodigy);
        let desenvolvimento = development_for_profile(rng, idade, skill, rookie_prodigy);
        // Os EIXOS DE ESTILO nascem largos de proposito. Eles nao tem lado bom —
        // agressivo ganha na largada e paga em pneu e em incidente —, entao a
        // unica coisa que os torna interessantes e o piloto do lado ocupar outro
        // ponto da regua. Em 30–70 todo mundo saia calculista-morno e a ficha
        // desenhava trinta marcadores no mesmo lugar; `confianca` em 50–70 era
        // pior ainda: nenhum piloto gerado conseguia ser cauteloso, e metade do
        // eixo nunca existia. `smoothness` acompanha por ser o inverso.
        let aggression = roll_stat(rng, 20, 85);
        let smoothness = inverse_correlated_stat(rng, aggression);
        // `midia` para em 74 de proposito: 85 e o piso do tier `Elite` de
        // visibilidade de mercado ([`crate::market::visibility`]), e alargar ate
        // la mexeria em salario e patrocinio sem ninguem ter pedido.
        let midia = roll_stat(rng, 26, 74);
        let mentalidade = roll_stat(rng, 34, 78);
        let confianca = roll_stat(rng, 30, 80);
        let potencial = (skill as f64
            + potential_headroom(desenvolvimento as f64, idade) * rng.gen_range(0.85..=1.15))
        .min(POTENTIAL_HARD_MAX);

        let ano_inicio = career_start_year_from_age(world_year, idade);
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
        let carisma = roll_carisma(
            driver.personalidade_primaria.as_ref(),
            driver.personalidade_secundaria.as_ref(),
            rng,
        );
        let mut atributos = DriverAttributes {
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
            carisma,
            mentalidade: mentalidade as f64,
            confianca: confianca as f64,
            potencial,
        };
        if let Some(profile) = rookie_profile {
            apply_rookie_profile(&mut atributos, profile, rookie_prodigy, rng);
        }
        driver.atributos = atributos;
        seed_initial_career_history(&mut driver, category_id, normalized_tier, rng);
        drivers.push(driver);
    }

    drivers
}

fn seed_initial_career_history(
    driver: &mut Driver,
    category_id: &str,
    tier: u8,
    rng: &mut impl Rng,
) {
    if is_career_debut_category(category_id) {
        return;
    }

    let category = get_category_config(category_id);
    let races_per_season = category
        .map(|config| config.corridas_por_temporada.max(1) as u32)
        .unwrap_or(8);
    let seasons = initial_seasons_for_profile(driver.idade, tier, category, rng);
    let missed_races = rng.gen_range(0..=races_per_season.min(3));
    let career_races = (seasons * races_per_season)
        .saturating_sub(missed_races)
        .max(1);
    let category_seasons = seasons.clamp(1, 3);
    let category_races = (category_seasons * races_per_season)
        .saturating_sub(missed_races.min(races_per_season / 2))
        .max(1);

    driver.stats_carreira.temporadas = seasons;
    driver.stats_carreira.corridas = career_races;
    driver.temporadas_na_categoria = category_seasons;
    driver.corridas_na_categoria = category_races;
}

fn is_career_debut_category(category_id: &str) -> bool {
    matches!(category_id, "mazda_rookie" | "toyota_rookie")
}

fn initial_seasons_for_profile(
    age: u32,
    tier: u8,
    category: Option<&CategoryConfig>,
    rng: &mut impl Rng,
) -> u32 {
    let minimum = tier.max(1) as u32;
    let age_room = age.saturating_sub(18).max(1);
    let foundation_bias = category
        .map(|config| {
            if config.licenca_necessaria.is_some() {
                1
            } else {
                0
            }
        })
        .unwrap_or(0);
    let maximum = (minimum + age_room / 2 + foundation_bias).clamp(minimum, 8);
    rng.gen_range(minimum..=maximum)
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
    _difficulty: &scoring::DifficultyConfig,
    rookie: bool,
    rookie_prodigy: bool,
) -> (u8, u8) {
    if rookie_prodigy {
        return (63, 74);
    }

    if rookie {
        return (25, 62);
    }

    // Sem cap de IA por dificuldade: a faixa vem só do talento/tier da categoria.
    // O sistema de dificuldade será redesenhado à parte e não deve mais comprimir
    // a habilidade gerada.
    (range.skill_min, range.skill_max)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RookieProfile {
    Normal,
    Flawed,
    HeavyFlawed,
}

fn rookie_profile_for_slot(index: usize, count: usize) -> RookieProfile {
    let normal_slots = rookie_normal_slots(count);
    let heavy_slots = rookie_heavy_slots(count);

    if index < normal_slots {
        RookieProfile::Normal
    } else if index < normal_slots + heavy_slots {
        RookieProfile::HeavyFlawed
    } else {
        RookieProfile::Flawed
    }
}

fn rookie_normal_slots(count: usize) -> usize {
    if count == 0 {
        0
    } else if count >= 10 {
        2
    } else if count >= 5 {
        1
    } else {
        0
    }
}

fn rookie_heavy_slots(count: usize) -> usize {
    if count >= 10 {
        2
    } else if count >= 5 {
        1
    } else {
        0
    }
}

fn apply_rookie_profile(
    atributos: &mut DriverAttributes,
    profile: RookieProfile,
    rookie_prodigy: bool,
    rng: &mut impl Rng,
) {
    normalize_rookie_baseline(atributos);

    match profile {
        RookieProfile::Normal => {}
        RookieProfile::Flawed | RookieProfile::HeavyFlawed if rookie_prodigy => {
            apply_rookie_archetype(atributos, rng.gen_range(0_usize..12_usize), false, rng);
        }
        RookieProfile::Flawed => {
            apply_rookie_archetype(atributos, rng.gen_range(0_usize..12_usize), false, rng);
        }
        RookieProfile::HeavyFlawed => {
            let first = rng.gen_range(0_usize..12_usize);
            let mut second = rng.gen_range(0_usize..12_usize);
            if second == first {
                second = (second + 1) % 12;
            }
            apply_rookie_archetype(atributos, first, true, rng);
            apply_rookie_archetype(atributos, second, true, rng);
        }
    }
}

/// O piso existe para o defeito do rookie ser o ARQUETIPO e nao ruido: sem ele,
/// um sorteio azarado num eixo qualquer competiria com a fraqueza que o perfil
/// escolheu de proposito. Ele roda ANTES do arquetipo, entao nao levanta o
/// defeito depois de aplicado.
///
/// `experiencia` esta FORA dele, e essa e a excecao com nome. O valor bruto de um
/// novato e 8..30 — ele e inexperiente, e isso e o que o define —, e levantar
/// para 36 apagava a unica tag que todo rookie deveria ter. Pior: o OUTRO gerador
/// de rookie, o de vaga em temporada ([`crate::evolution::rookies`]), nao tem
/// piso e sorteia `U{5..=25}`. Dois novatos no mesmo grid, feitos por codigo
/// diferente, e so um se chamava "Calouro".
fn normalize_rookie_baseline(atributos: &mut DriverAttributes) {
    atributos.consistencia = atributos.consistencia.max(36.0);
    atributos.racecraft = atributos.racecraft.max(36.0);
    atributos.defesa = atributos.defesa.max(36.0);
    atributos.ritmo_classificacao = atributos.ritmo_classificacao.max(36.0);
    atributos.gestao_pneus = atributos.gestao_pneus.max(36.0);
    atributos.habilidade_largada = atributos.habilidade_largada.max(36.0);
    atributos.adaptabilidade = atributos.adaptabilidade.max(36.0);
    atributos.fator_chuva = atributos.fator_chuva.max(36.0);
    atributos.fitness = atributos.fitness.max(36.0);
    atributos.desenvolvimento = atributos.desenvolvimento.max(36.0);
    atributos.aggression = atributos.aggression.max(36.0);
    atributos.smoothness = atributos.smoothness.max(36.0);
    atributos.midia = atributos.midia.max(36.0);
    atributos.mentalidade = atributos.mentalidade.max(36.0);
    atributos.confianca = atributos.confianca.max(36.0);
}

fn apply_rookie_archetype(
    atributos: &mut DriverAttributes,
    archetype: usize,
    severe: bool,
    rng: &mut impl Rng,
) {
    let max = if severe {
        ROOKIE_HEAVY_FLAW_MAX
    } else {
        ROOKIE_COMMON_FLAW_MAX
    };

    match archetype % 12 {
        0 => {
            atributos.gestao_pneus = low_rookie_stat(max, rng);
            atributos.smoothness = low_rookie_stat(max, rng);
        }
        1 => {
            atributos.fator_chuva = low_rookie_stat(max, rng);
            atributos.adaptabilidade = low_rookie_stat(max, rng);
        }
        2 => {
            atributos.habilidade_largada = low_rookie_stat(max, rng);
            atributos.confianca = low_rookie_stat(max, rng);
        }
        3 => {
            atributos.mentalidade = low_rookie_stat(max, rng);
            atributos.confianca = low_rookie_stat(max, rng);
        }
        4 => {
            atributos.racecraft = low_rookie_stat(max, rng);
            atributos.defesa = low_rookie_stat(max, rng);
        }
        5 => {
            atributos.consistencia = low_rookie_stat(max, rng);
            atributos.mentalidade = low_rookie_stat(max, rng);
        }
        6 => {
            atributos.fitness = low_rookie_stat(max, rng);
            atributos.consistencia = low_rookie_stat(max, rng);
        }
        7 => {
            atributos.smoothness = low_rookie_stat(max, rng);
            atributos.defesa = low_rookie_stat(max, rng);
            atributos.aggression = roll_stat(rng, 78, 95) as f64;
        }
        8 => {
            atributos.midia = low_rookie_stat(max, rng);
            atributos.mentalidade = low_rookie_stat(max, rng);
        }
        9 => {
            atributos.racecraft = low_rookie_stat(max, rng);
            atributos.gestao_pneus = low_rookie_stat(max, rng);
            atributos.ritmo_classificacao = atributos
                .ritmo_classificacao
                .max(roll_stat(rng, 55, 70) as f64);
        }
        10 => {
            atributos.defesa = low_rookie_stat(max, rng);
            atributos.habilidade_largada = low_rookie_stat(max, rng);
            atributos.aggression = roll_stat(rng, 5, max.min(28)) as f64;
        }
        _ => {
            atributos.experiencia = low_rookie_stat(max, rng);
            atributos.adaptabilidade = low_rookie_stat(max, rng);
            atributos.confianca = low_rookie_stat(max, rng);
        }
    }
}

fn low_rookie_stat(max: u8, rng: &mut impl Rng) -> f64 {
    let min = if max <= ROOKIE_HEAVY_FLAW_MAX { 5 } else { 25 };
    roll_stat(rng, min, max) as f64
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
        0 => (16, 22),
        1 => (20, 28),
        2 => (22, 31),
        3 => (24, 35),
        _ => (26, 40),
    }
}

fn roll_age_for_profile(tier: u8, rookie_prodigy: bool, rng: &mut impl Rng) -> u32 {
    if tier == 0 {
        if rng.gen_range(0_u8..100_u8) < 3 {
            return 15;
        }
        return if rookie_prodigy {
            rng.gen_range(16..=19)
        } else {
            let (min_age, max_age) = tier_age_range(tier);
            rng.gen_range(min_age..=max_age)
        };
    }

    let (min_age, max_age) = tier_age_range(tier);
    rng.gen_range(min_age..=max_age)
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

#[cfg(test)]
mod tests {
    use super::generate_for_category_with_id_factory;
    use rand::{rngs::StdRng, SeedableRng};
    use std::collections::HashSet;

    #[test]
    fn rookie_category_can_generate_very_young_drivers() {
        let mut saw_age_15 = false;
        let mut saw_age_16 = false;

        for seed in 0..300 {
            let mut rng = StdRng::seed_from_u64(seed);
            let mut existing_names = HashSet::new();
            let mut next_id = 1_u32;
            let drivers = generate_for_category_with_id_factory(
                "mazda_rookie",
                0,
                "medio",
                12,
                2024,
                &mut existing_names,
                &mut || {
                    let id = format!("P{next_id:03}");
                    next_id += 1;
                    id
                },
                &mut rng,
            );

            for driver in drivers {
                assert!(
                    (15..=22).contains(&driver.idade),
                    "idade rookie fora da faixa esperada: {}",
                    driver.idade
                );
                saw_age_15 |= driver.idade == 15;
                saw_age_16 |= driver.idade == 16;
            }
        }

        assert!(saw_age_16, "geracao rookie deveria permitir 16 anos");
        assert!(
            saw_age_15,
            "geracao rookie deveria permitir raros pilotos de 15 anos"
        );
    }
}
