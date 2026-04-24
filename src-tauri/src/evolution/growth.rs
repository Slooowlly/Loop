use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::models::driver::Driver;
use crate::models::driver_attributes::DriverAttributeKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonStats {
    pub posicao_campeonato: i32,
    pub total_pilotos: i32,
    pub pontos: i32,
    pub vitorias: i32,
    pub podios: i32,
    pub corridas: i32,
    pub dnfs: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrowthReport {
    pub driver_id: String,
    pub driver_name: String,
    pub changes: Vec<AttributeChange>,
    pub overall_delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeChange {
    pub attribute: String,
    pub old_value: u8,
    pub new_value: u8,
    pub delta: i8,
    pub reason: String,
}

const GROWABLE_ATTRIBUTES: [(DriverAttributeKey, f64); 10] = [
    (DriverAttributeKey::Skill, 0.8),
    (DriverAttributeKey::Consistencia, 0.6),
    (DriverAttributeKey::Racecraft, 0.5),
    (DriverAttributeKey::Defesa, 0.4),
    (DriverAttributeKey::RitmoClassificacao, 0.7),
    (DriverAttributeKey::GestaoPneus, 0.5),
    (DriverAttributeKey::Adaptabilidade, 0.3),
    (DriverAttributeKey::Mentalidade, 0.4),
    (DriverAttributeKey::Confianca, 0.5),
    (DriverAttributeKey::Smoothness, 0.3),
];

pub fn calculate_growth(
    driver: &mut Driver,
    season_stats: &SeasonStats,
    team_car_performance: f64,
    category_tier: u8,
    rng: &mut impl Rng,
) -> GrowthReport {
    let mut changes = Vec::new();
    let base_growth = (result_base_growth(season_stats) + car_bonus(team_car_performance)).max(0.0);

    for (attribute, weight) in GROWABLE_ATTRIBUTES {
        let delta = growth_for_attribute(
            get_attribute(driver, attribute).round().clamp(0.0, 100.0) as u8,
            base_growth * weight,
            driver.idade as i32,
            category_tier,
            rng,
        );
        if let Some(change) = apply_growth(driver, attribute, delta, "Evolucao por resultados") {
            changes.push(change);
        }
    }

    let exp_gain = rng.gen_range(2..=5) as i8;
    if let Some(change) = apply_growth(
        driver,
        DriverAttributeKey::Experiencia,
        exp_gain,
        "Experiencia acumulada",
    ) {
        changes.push(change);
    }

    let media_delta = if changes.is_empty() {
        0.0
    } else {
        changes
            .iter()
            .map(|change| change.delta as f64)
            .sum::<f64>()
            / changes.len() as f64
    };

    let media_delta_int = media_delta.round() as i8;
    let development_delta = (media_delta_int + rng.gen_range(0..=2)).clamp(-2, 6);
    if let Some(change) = apply_growth(
        driver,
        DriverAttributeKey::Desenvolvimento,
        development_delta,
        "Desenvolvimento ajustado pela taxa de evolucao",
    ) {
        changes.push(change);
    }

    let media_boost = if season_stats.vitorias >= 3 {
        rng.gen_range(2..=3)
    } else if season_stats.podios >= 4 || season_stats.vitorias >= 1 {
        rng.gen_range(1..=2)
    } else {
        0
    } as i8;
    if let Some(change) = apply_growth(
        driver,
        DriverAttributeKey::Midia,
        media_boost,
        "Exposicao por resultados",
    ) {
        changes.push(change);
    }

    let overall_delta = changes.iter().map(|change| change.delta as f64).sum();

    GrowthReport {
        driver_id: driver.id.clone(),
        driver_name: driver.nome.clone(),
        changes,
        overall_delta,
    }
}

fn result_base_growth(stats: &SeasonStats) -> f64 {
    if stats.corridas <= 0 || stats.total_pilotos <= 0 {
        return 0.0;
    }

    let total = stats.total_pilotos.max(1) as f64;
    let position_ratio = if stats.total_pilotos <= 1 {
        1.0
    } else {
        1.0 - ((stats.posicao_campeonato.max(1) - 1) as f64 / (total - 1.0))
    };
    let base = position_ratio * 3.0;
    let win_bonus = (stats.vitorias.max(0) as f64 * 0.3).min(1.5);
    let dnf_penalty = stats.dnfs.max(0) as f64 * 0.2;
    (base + win_bonus - dnf_penalty).max(0.0)
}

fn car_bonus(team_car_performance: f64) -> f64 {
    if team_car_performance > 10.0 {
        0.5
    } else if team_car_performance > 5.0 {
        0.2
    } else if team_car_performance < 0.0 {
        -0.3
    } else {
        0.0
    }
}

fn growth_for_attribute(
    current_value: u8,
    base_growth: f64,
    age: i32,
    category_tier: u8,
    rng: &mut impl Rng,
) -> i8 {
    let diminishing = 1.0 - (current_value as f64 / 120.0);
    let age_factor = if age <= 20 {
        1.5
    } else if age <= 24 {
        1.2
    } else if age <= 28 {
        1.0
    } else if age <= 32 {
        0.7
    } else {
        0.3
    };
    let tier_factor = match category_tier {
        0 => 1.4,
        1 => 1.2,
        2 => 1.0,
        3 => 0.8,
        4 => 0.6,
        _ => 0.5,
    };

    let raw_delta = base_growth * diminishing * age_factor * tier_factor;
    let variance = rng.gen_range(-0.5..=0.5);
    (raw_delta + variance).round() as i8
}

fn apply_growth(
    driver: &mut Driver,
    key: DriverAttributeKey,
    delta: i8,
    reason: &str,
) -> Option<AttributeChange> {
    if delta == 0 {
        return None;
    }

    let current = get_attribute(driver, key);
    let new_value = (current + delta as f64).clamp(0.0, 100.0);
    if (new_value - current).abs() < f64::EPSILON {
        return None;
    }

    set_attribute(driver, key, new_value);

    let old_rounded = current.round().clamp(0.0, 100.0) as u8;
    let new_rounded = new_value.round().clamp(0.0, 100.0) as u8;
    if old_rounded == new_rounded {
        return None;
    }

    Some(AttributeChange {
        attribute: key.as_str().to_string(),
        old_value: old_rounded,
        new_value: new_rounded,
        delta: new_rounded as i8 - old_rounded as i8,
        reason: reason.to_string(),
    })
}

pub(crate) fn get_attribute(driver: &Driver, key: DriverAttributeKey) -> f64 {
    match key {
        DriverAttributeKey::Skill => driver.atributos.skill,
        DriverAttributeKey::Consistencia => driver.atributos.consistencia,
        DriverAttributeKey::Racecraft => driver.atributos.racecraft,
        DriverAttributeKey::Defesa => driver.atributos.defesa,
        DriverAttributeKey::RitmoClassificacao => driver.atributos.ritmo_classificacao,
        DriverAttributeKey::GestaoPneus => driver.atributos.gestao_pneus,
        DriverAttributeKey::HabilidadeLargada => driver.atributos.habilidade_largada,
        DriverAttributeKey::Adaptabilidade => driver.atributos.adaptabilidade,
        DriverAttributeKey::FatorChuva => driver.atributos.fator_chuva,
        DriverAttributeKey::Fitness => driver.atributos.fitness,
        DriverAttributeKey::Experiencia => driver.atributos.experiencia,
        DriverAttributeKey::Desenvolvimento => driver.atributos.desenvolvimento,
        DriverAttributeKey::Aggression => driver.atributos.aggression,
        DriverAttributeKey::Smoothness => driver.atributos.smoothness,
        DriverAttributeKey::Midia => driver.atributos.midia,
        DriverAttributeKey::Mentalidade => driver.atributos.mentalidade,
        DriverAttributeKey::Confianca => driver.atributos.confianca,
    }
}

pub(crate) fn set_attribute(driver: &mut Driver, key: DriverAttributeKey, value: f64) {
    match key {
        DriverAttributeKey::Skill => driver.atributos.skill = value,
        DriverAttributeKey::Consistencia => driver.atributos.consistencia = value,
        DriverAttributeKey::Racecraft => driver.atributos.racecraft = value,
        DriverAttributeKey::Defesa => driver.atributos.defesa = value,
        DriverAttributeKey::RitmoClassificacao => driver.atributos.ritmo_classificacao = value,
        DriverAttributeKey::GestaoPneus => driver.atributos.gestao_pneus = value,
        DriverAttributeKey::HabilidadeLargada => driver.atributos.habilidade_largada = value,
        DriverAttributeKey::Adaptabilidade => driver.atributos.adaptabilidade = value,
        DriverAttributeKey::FatorChuva => driver.atributos.fator_chuva = value,
        DriverAttributeKey::Fitness => driver.atributos.fitness = value,
        DriverAttributeKey::Experiencia => driver.atributos.experiencia = value,
        DriverAttributeKey::Desenvolvimento => driver.atributos.desenvolvimento = value,
        DriverAttributeKey::Aggression => driver.atributos.aggression = value,
        DriverAttributeKey::Smoothness => driver.atributos.smoothness = value,
        DriverAttributeKey::Midia => driver.atributos.midia = value,
        DriverAttributeKey::Mentalidade => driver.atributos.mentalidade = value,
        DriverAttributeKey::Confianca => driver.atributos.confianca = value,
    }
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn test_growth_champion_gets_positive_growth() {
        let mut driver = sample_driver(19, 45.0);
        let stats = champion_stats();
        let mut rng = StdRng::seed_from_u64(7);

        let report = calculate_growth(&mut driver, &stats, 8.0, 0, &mut rng);

        assert!(report.overall_delta > 0.0);
        assert!(driver.atributos.skill > 45.0);
        assert!(!report.changes.is_empty());
    }

    #[test]
    fn test_growth_last_place_gets_less() {
        let stats_top = champion_stats();
        let stats_last = SeasonStats {
            posicao_campeonato: 20,
            total_pilotos: 20,
            pontos: 5,
            vitorias: 0,
            podios: 0,
            corridas: 10,
            dnfs: 3,
        };
        let mut top_driver = sample_driver(22, 45.0);
        let mut last_driver = sample_driver(22, 45.0);

        let mut rng_top = StdRng::seed_from_u64(12);
        let report_top = calculate_growth(&mut top_driver, &stats_top, 4.0, 1, &mut rng_top);

        let mut rng_last = StdRng::seed_from_u64(12);
        let report_last = calculate_growth(&mut last_driver, &stats_last, 4.0, 1, &mut rng_last);

        assert!(report_top.overall_delta > report_last.overall_delta);
    }

    #[test]
    fn test_growth_young_driver_grows_faster() {
        let stats = champion_stats();
        let mut young = sample_driver(18, 50.0);
        let mut veteran = sample_driver(31, 50.0);

        let mut rng_young = StdRng::seed_from_u64(24);
        let report_young = calculate_growth(&mut young, &stats, 2.0, 1, &mut rng_young);

        let mut rng_veteran = StdRng::seed_from_u64(24);
        let report_veteran = calculate_growth(&mut veteran, &stats, 2.0, 1, &mut rng_veteran);

        assert!(report_young.overall_delta > report_veteran.overall_delta);
    }

    #[test]
    fn test_growth_high_skill_diminishing_returns() {
        let stats = champion_stats();
        let mut low_skill = sample_driver(21, 40.0);
        let mut high_skill = sample_driver(21, 88.0);

        let mut rng_low = StdRng::seed_from_u64(35);
        let report_low = calculate_growth(&mut low_skill, &stats, 5.0, 2, &mut rng_low);

        let mut rng_high = StdRng::seed_from_u64(35);
        let report_high = calculate_growth(&mut high_skill, &stats, 5.0, 2, &mut rng_high);

        assert!(report_low.overall_delta > report_high.overall_delta);
    }

    #[test]
    fn test_growth_low_tier_grows_more() {
        let stats = champion_stats();
        let mut rookie_driver = sample_driver(20, 50.0);
        let mut top_driver = sample_driver(20, 50.0);

        let mut rng_rookie = StdRng::seed_from_u64(46);
        let rookie_report = calculate_growth(&mut rookie_driver, &stats, 1.0, 0, &mut rng_rookie);

        let mut rng_top = StdRng::seed_from_u64(46);
        let top_report = calculate_growth(&mut top_driver, &stats, 1.0, 4, &mut rng_top);

        assert!(rookie_report.overall_delta > top_report.overall_delta);
    }

    fn sample_driver(age: u32, skill: f64) -> Driver {
        let mut driver = Driver::new(
            "P001".to_string(),
            "Piloto Teste".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            age,
            2024_u32.saturating_sub(age.saturating_sub(16)),
        );
        driver.atributos.skill = skill;
        driver.atributos.consistencia = skill;
        driver.atributos.racecraft = skill;
        driver.atributos.defesa = skill;
        driver.atributos.ritmo_classificacao = skill;
        driver.atributos.gestao_pneus = skill;
        driver.atributos.adaptabilidade = skill;
        driver.atributos.mentalidade = skill;
        driver.atributos.confianca = skill;
        driver.atributos.smoothness = skill;
        driver.atributos.desenvolvimento = 50.0;
        driver.atributos.experiencia = 30.0;
        driver.atributos.midia = 30.0;
        driver
    }

    fn champion_stats() -> SeasonStats {
        SeasonStats {
            posicao_campeonato: 1,
            total_pilotos: 20,
            pontos: 180,
            vitorias: 4,
            podios: 7,
            corridas: 10,
            dnfs: 0,
        }
    }
}
