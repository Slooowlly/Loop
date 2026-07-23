use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::constants::skill_ranges;
use crate::models::driver::{derive_potential_ceiling, Driver, POTENTIAL_HARD_MAX};
use crate::models::driver_attributes::DriverAttributeKey;

// ── Pesos de resultado: pódio rende muito mais que só terminar a corrida ──────
const PODIUM_GROWTH_WEIGHT: f64 = 0.9;
const WIN_GROWTH_BONUS: f64 = 0.5;
const FINISH_GROWTH_WEIGHT: f64 = 0.25;
const DNF_GROWTH_PENALTY: f64 = 0.3;

/// Janela de aproximação ao teto pessoal: quanto menor, mais cedo a evolução
/// desacelera perto do potencial. O crescimento é cheio enquanto a folga (teto −
/// atual) for ≥ este valor.
const POTENTIAL_APPROACH_WINDOW: f64 = 22.0;

/// Skill é o ÚNICO atributo que exporta direto pra força da IA no iRacing
/// (skill→driverSkill). Por isso ele cresce como uma CARREIRA, não num pulo: um
/// prodígio ainda chega ao teto, mas ao longo de várias temporadas. Duas travas
/// combinadas, aplicadas só ao skill e só ao ganho positivo:
///  1. teto rígido absoluto por temporada;
///  2. consumo máximo de uma fração da folga (teto − skill) por temporada — faz a
///     subida virar rampa que desacelera perto do teto, sem depender só do (1).
/// As demais competências (consistência, racecraft, etc.) seguem o loop normal;
/// os atributos de ESTILO (agressividade/suavidade/confiança) saem do crescimento
/// monotônico no Trilho B (perfis de piloto) — ver [[project_driver_growth_redesign]].
const MAX_SKILL_GAIN_PER_SEASON: i8 = 3;
const SKILL_HEADROOM_FRACTION_PER_SEASON: f64 = 0.25;

/// Aplica as travas de skill sobre um delta já calculado. Só morde o ganho positivo
/// do skill; devolve o delta intacto pra qualquer outro atributo ou queda.
fn cap_skill_growth(attribute: DriverAttributeKey, delta: i8, current_skill: f64, potencial: f64) -> i8 {
    if attribute != DriverAttributeKey::Skill || delta <= 0 {
        return delta;
    }
    let headroom = (potencial - current_skill).max(0.0);
    // ceil pra que, enquanto houver qualquer folga, a rampa ainda ande +1 no fim.
    let fraction_cap = (headroom * SKILL_HEADROOM_FRACTION_PER_SEASON).ceil() as i8;
    delta.min(MAX_SKILL_GAIN_PER_SEASON).min(fraction_cap.max(0))
}

/// Quanto cada pódio "de azarão" (piloto abaixo da média da categoria) empurra o
/// teto pessoal para cima. O efeito se autolimita: ao crescer e deixar de ser
/// azarão, o piloto para de receber o boost.
const POTENTIAL_BOOST_PER_SURPRISE: f64 = 1.3;

/// Peso do termo de "superação de expectativa": o quanto render ACIMA do que o
/// carro entregaria acelera o crescimento. É o que converte o talento reprimido
/// (piloto com teto de craque preso num carro fraco) em skill de verdade, sem
/// depender de pódio absoluto. Calibrável.
const OVERPERF_WEIGHT: f64 = 8.0;

/// Teto do termo de superação: mesmo uma temporada milagrosa (pior carro vira
/// campeão) não injeta crescimento sem limite. Calibrável.
const OVERPERF_CAP: f64 = 4.0;

/// Maturação da juventude: até esta idade, o jovem talentoso cresce só por TEMPO DE
/// PISTA (correr uma temporada), independente de pódio/superação — é o prodígio que
/// floresce mesmo num carro adequado terminando onde o carro manda. Acima disto o
/// crescimento volta a depender só de resultado.
const YOUTH_MATURATION_AGE_CAP: u32 = 24;

/// Magnitude por temporada cheia do termo de maturação (escalado pela folga até o
/// teto e pelo tempo de pista). Calibrável.
const YOUTH_MATURATION: f64 = 4.0;

/// Denominador da folga (teto − skill) na maturação: quanto maior a folga, mais
/// rápido o jovem sobe — autolimitante, pois some ao chegar perto do teto.
const YOUTH_MATURATION_FOLGA_SCALE: f64 = 30.0;

/// Folga mínima até o teto para a maturação valer (evita ruído em quem já chegou).
const YOUTH_MATURATION_MIN_FOLGA: f64 = 3.0;

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
    car_field_percentile: f64,
    rng: &mut impl Rng,
) -> GrowthReport {
    let mut changes = Vec::new();

    // Garante que o piloto tem um teto pessoal definido (jogador / saves antigos
    // derivam sob demanda e persistem a partir daqui).
    if driver.atributos.potencial <= 0.0 {
        driver.atributos.potencial = derive_potential_ceiling(
            driver.atributos.skill,
            driver.atributos.desenvolvimento,
            driver.idade,
        );
    }
    let potencial = driver.atributos.potencial.min(POTENTIAL_HARD_MAX);

    let base_growth = (result_base_growth(season_stats)
        + car_bonus(team_car_performance)
        + overperformance_growth(season_stats, car_field_percentile)
        + youth_maturation_growth(
            driver.idade,
            driver.atributos.skill,
            potencial,
            season_stats.corridas,
        ))
    .max(0.0);

    for (attribute, weight) in GROWABLE_ATTRIBUTES {
        let delta = growth_for_attribute(
            get_attribute(driver, attribute).round().clamp(0.0, 100.0) as u8,
            base_growth * weight,
            driver.idade as i32,
            category_tier,
            potencial,
            rng,
        );
        // Skill exporta pro iRacing → cresce em rampa de carreira, nunca num pulo.
        let delta = cap_skill_growth(attribute, delta, driver.atributos.skill, potencial);
        if let Some(change) = apply_growth(driver, attribute, delta, "Evolucao por resultados") {
            changes.push(change);
        }
    }

    apply_potential_boost(driver, season_stats, category_tier);

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

    // Resultado por corrida: um pódio vale muito mais que só terminar. Vitórias
    // ganham um extra sobre o pódio. Terminar fora do pódio ainda evolui pouco.
    let podios = stats.podios.max(0) as f64;
    let vitorias = stats.vitorias.max(0) as f64;
    let dnfs = stats.dnfs.max(0) as f64;
    let plain_finishes = (stats.corridas as f64 - podios - dnfs).max(0.0);

    let base = podios * PODIUM_GROWTH_WEIGHT
        + vitorias * WIN_GROWTH_BONUS
        + plain_finishes * FINISH_GROWTH_WEIGHT
        - dnfs * DNF_GROWTH_PENALTY;
    base.max(0.0)
}

/// Superação de expectativa: o piloto rendeu ACIMA do que o carro dele entregaria?
/// `car_field_percentile` = posição do carro na categoria (0 = pior carro do grid,
/// 1 = melhor). Isola talento de máquina: terminar 8º num carro que vale 12º cresce;
/// vencer no melhor carro (o esperado) não dá bônus. Só superação POSITIVA conta —
/// render abaixo do carro já é penalizado pelo caminho de pódio/DNF. É a trava
/// anti-inflação: a skill injetada é limitada pela "surpresa" real da temporada.
fn overperformance_growth(stats: &SeasonStats, car_field_percentile: f64) -> f64 {
    if stats.corridas <= 0 || stats.total_pilotos <= 1 {
        return 0.0;
    }
    let car_pct = car_field_percentile.clamp(0.0, 1.0);
    // Pior carro (pct 0) → esperado no fundo (frac 1); melhor carro (pct 1) → frac 0.
    let esperado_frac = 1.0 - car_pct;
    // Posição real normalizada: 0 = campeão, 1 = último.
    let real_frac =
        ((stats.posicao_campeonato - 1).max(0) as f64) / ((stats.total_pilotos - 1) as f64);
    let superacao = (esperado_frac - real_frac).max(0.0);
    (OVERPERF_WEIGHT * superacao).min(OVERPERF_CAP)
}

/// Maturação da juventude: crescimento por TEMPO DE PISTA para o jovem (≤ idade-teto)
/// com folga até o teto pessoal, escalado pela folga e pelas corridas disputadas.
/// É o que converte o talento reprimido que não pódia nem supera o carro — o prodígio
/// num carro adequado que hoje estagna. Anti-inflação: gate de idade (veterano não
/// pega), escala pela folga (quem está perto do teto quase não muda) e o cap por
/// headroom em `growth_for_attribute` (nunca passa do teto pessoal). Não muda o
/// equilíbrio (limitado pelos tetos): só faz o capaz CHEGAR ao teto antes de
/// envelhecer, em vez de virar "preso velho" e se aposentar sem converter.
fn youth_maturation_growth(idade: u32, skill: f64, potencial: f64, corridas: i32) -> f64 {
    if idade > YOUTH_MATURATION_AGE_CAP || corridas <= 0 {
        return 0.0;
    }
    let folga = (potencial - skill).max(0.0);
    if folga <= YOUTH_MATURATION_MIN_FOLGA {
        return 0.0;
    }
    let folga_frac = (folga / YOUTH_MATURATION_FOLGA_SCALE).min(1.0);
    let corridas_frac = (corridas as f64 / 8.0).clamp(0.0, 1.0);
    YOUTH_MATURATION * folga_frac * corridas_frac
}

/// Pódios "de azarão" (piloto abaixo da média de habilidade da categoria) sobem o
/// teto pessoal — oportunidade (carro/sorte) destrava potencial. Mecânica
/// autolimitada: ao crescer acima da média, o piloto deixa de ser azarão e o
/// boost cessa. O fator `(HARD_MAX − teto)/HARD_MAX` desacelera perto de 98.
fn apply_potential_boost(driver: &mut Driver, stats: &SeasonStats, category_tier: u8) {
    if stats.podios <= 0 {
        return;
    }
    let median = skill_ranges::get_skill_range_by_tier(category_tier.min(4))
        .map(|range| range.skill_media as f64)
        .unwrap_or(60.0);
    if driver.atributos.skill >= median {
        return;
    }

    let current = driver.atributos.potencial.min(POTENTIAL_HARD_MAX);
    if current >= POTENTIAL_HARD_MAX {
        return;
    }
    let headroom_factor = ((POTENTIAL_HARD_MAX - current) / POTENTIAL_HARD_MAX).max(0.25);
    let gain = stats.podios as f64 * POTENTIAL_BOOST_PER_SURPRISE * headroom_factor;
    driver.atributos.potencial = (current + gain).min(POTENTIAL_HARD_MAX);
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
    potencial: f64,
    rng: &mut impl Rng,
) -> i8 {
    // Aproximação ao teto PESSOAL (não a um teto global): cheio enquanto a folga
    // for grande, desacelera ao chegar perto do potencial do piloto.
    let headroom = (potencial - current_value as f64).max(0.0);
    let approach = (headroom / POTENTIAL_APPROACH_WINDOW).clamp(0.0, 1.0);
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
    // tier_factor suavizado: o topo (elite) volta a evoluir de verdade.
    let tier_factor = match category_tier {
        0 => 1.4,
        1 => 1.25,
        2 => 1.1,
        3 => 0.95,
        _ => 0.8,
    };

    let raw_delta = base_growth * approach * age_factor * tier_factor;
    let variance = rng.gen_range(-0.5..=0.5);
    let delta = (raw_delta + variance).round();
    // Nunca ultrapassa o teto pessoal (mas permite a pequena variância negativa).
    delta.min(headroom) as i8
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
        DriverAttributeKey::Carisma => driver.atributos.carisma,
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
        DriverAttributeKey::Carisma => driver.atributos.carisma = value,
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

        let report = calculate_growth(&mut driver, &stats, 8.0, 0, 0.5, &mut rng);

        assert!(report.overall_delta > 0.0);
        assert!(driver.atributos.skill > 45.0);
        assert!(!report.changes.is_empty());
    }

    #[test]
    fn skill_never_jumps_more_than_cap_in_one_season() {
        // O caso Leroy: rookie campeão jovem, folga enorme até o teto. Antes pulava
        // ~+18 de skill numa temporada; agora o ganho é travado no teto por temporada.
        let mut leroy = sample_driver(20, 72.0);
        leroy.atributos.potencial = 92.0;
        let stats = SeasonStats {
            posicao_campeonato: 1,
            total_pilotos: 12,
            pontos: 195,
            vitorias: 3,
            podios: 5,
            corridas: 5,
            dnfs: 0,
        };
        let mut rng = StdRng::seed_from_u64(7);
        let before = leroy.atributos.skill;
        calculate_growth(&mut leroy, &stats, 2.0, 0, 0.6, &mut rng);
        let gain = leroy.atributos.skill - before;
        assert!(gain > 0.0, "prodígio campeão tem que crescer: {gain}");
        assert!(
            gain <= MAX_SKILL_GAIN_PER_SEASON as f64,
            "skill não pode pular mais que o teto por temporada: {gain}"
        );
    }

    #[test]
    fn cap_skill_growth_only_touches_positive_skill() {
        // Só o ganho positivo de skill é travado; queda e outros atributos passam.
        assert_eq!(cap_skill_growth(DriverAttributeKey::Skill, 18, 72.0, 92.0), 3);
        assert_eq!(cap_skill_growth(DriverAttributeKey::Skill, -2, 72.0, 92.0), -2);
        assert_eq!(cap_skill_growth(DriverAttributeKey::Racecraft, 18, 72.0, 92.0), 18);
        // Fração da folga morde antes do teto quando já está perto do potencial.
        assert_eq!(cap_skill_growth(DriverAttributeKey::Skill, 3, 90.0, 92.0), 1); // ceil(0.5)=1
        assert_eq!(cap_skill_growth(DriverAttributeKey::Skill, 3, 84.0, 92.0), 2); // ceil(2.0)=2
    }

    #[test]
    fn skill_reaches_ceiling_over_a_career_not_at_once() {
        // Mesmo dominando todo ano, o skill leva várias temporadas pra fechar a folga.
        let mut prodigy = sample_driver(19, 72.0);
        prodigy.atributos.potencial = 92.0;
        let stats = champion_stats();
        let mut seasons = 0;
        while prodigy.atributos.skill < 92.0 && seasons < 30 {
            let mut rng = StdRng::seed_from_u64(seasons as u64 + 1);
            let before = prodigy.atributos.skill;
            calculate_growth(&mut prodigy, &stats, 4.0, 0, 0.5, &mut rng);
            prodigy.idade = 19 + seasons; // envelhece junto
            assert!(
                prodigy.atributos.skill - before <= MAX_SKILL_GAIN_PER_SEASON as f64,
                "nenhuma temporada isolada pode estourar o teto"
            );
            seasons += 1;
        }
        assert!(seasons >= 5, "72→92 tem que ser uma carreira (>=5 temporadas), levou {seasons}");
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
        let report_top = calculate_growth(&mut top_driver, &stats_top, 4.0, 1, 0.5, &mut rng_top);

        let mut rng_last = StdRng::seed_from_u64(12);
        let report_last =
            calculate_growth(&mut last_driver, &stats_last, 4.0, 1, 0.5, &mut rng_last);

        assert!(report_top.overall_delta > report_last.overall_delta);
    }

    #[test]
    fn test_growth_young_driver_grows_faster() {
        let stats = champion_stats();
        let mut young = sample_driver(18, 50.0);
        let mut veteran = sample_driver(31, 50.0);

        let mut rng_young = StdRng::seed_from_u64(24);
        let report_young = calculate_growth(&mut young, &stats, 2.0, 1, 0.5, &mut rng_young);

        let mut rng_veteran = StdRng::seed_from_u64(24);
        let report_veteran = calculate_growth(&mut veteran, &stats, 2.0, 1, 0.5, &mut rng_veteran);

        assert!(report_young.overall_delta > report_veteran.overall_delta);
    }

    #[test]
    fn test_growth_high_skill_diminishing_returns() {
        let stats = champion_stats();
        let mut low_skill = sample_driver(21, 40.0);
        let mut high_skill = sample_driver(21, 88.0);

        let mut rng_low = StdRng::seed_from_u64(35);
        let report_low = calculate_growth(&mut low_skill, &stats, 5.0, 2, 0.5, &mut rng_low);

        let mut rng_high = StdRng::seed_from_u64(35);
        let report_high = calculate_growth(&mut high_skill, &stats, 5.0, 2, 0.5, &mut rng_high);

        assert!(report_low.overall_delta > report_high.overall_delta);
    }

    #[test]
    fn test_growth_low_tier_grows_more() {
        let stats = champion_stats();
        let mut rookie_driver = sample_driver(20, 50.0);
        let mut top_driver = sample_driver(20, 50.0);

        let mut rng_rookie = StdRng::seed_from_u64(46);
        let rookie_report =
            calculate_growth(&mut rookie_driver, &stats, 1.0, 0, 0.5, &mut rng_rookie);

        let mut rng_top = StdRng::seed_from_u64(46);
        let top_report = calculate_growth(&mut top_driver, &stats, 1.0, 4, 0.5, &mut rng_top);

        assert!(rookie_report.overall_delta > top_report.overall_delta);
    }

    // ── Superação de expectativa ──────────────────────────────────────────────
    fn stats_pos(posicao: i32, total: i32, corridas: i32) -> SeasonStats {
        SeasonStats {
            posicao_campeonato: posicao,
            total_pilotos: total,
            pontos: 0,
            vitorias: 0,
            podios: 0,
            corridas,
            dnfs: 0,
        }
    }

    #[test]
    fn overperf_positive_when_beating_the_car() {
        // Carro fraco (percentil 0.2) terminando 4º de 20 = muito acima do carro.
        let term = overperformance_growth(&stats_pos(4, 20, 12), 0.2);
        assert!(term > 0.0, "superar o carro tem que crescer, got {term}");
    }

    #[test]
    fn overperf_zero_when_only_meeting_car_expectation() {
        // Melhor carro do grid (percentil 1.0) vencendo = exatamente o esperado, sem
        // bônus. (Um carro quase-topo que vence ainda rende um resíduo pequeno, o que
        // é correto: venceu de uma posição de largada esperada logo atrás da frente.)
        let term = overperformance_growth(&stats_pos(1, 20, 12), 1.0);
        assert_eq!(term, 0.0, "vencer no melhor carro não é superação, got {term}");
    }

    #[test]
    fn overperf_zero_when_underperforming_the_car() {
        // Carro bom (0.9) terminando em último = rendeu ABAIXO do carro → sem bônus.
        let term = overperformance_growth(&stats_pos(20, 20, 12), 0.9);
        assert_eq!(term, 0.0);
    }

    #[test]
    fn overperf_is_capped() {
        // Pior carro (0.0) virando campeão = superação máxima, mas travada no CAP.
        let term = overperformance_growth(&stats_pos(1, 20, 12), 0.0);
        assert!((term - OVERPERF_CAP).abs() < 1e-9, "deveria bater o teto, got {term}");
    }

    #[test]
    fn overperf_guards_degenerate_seasons() {
        assert_eq!(overperformance_growth(&stats_pos(1, 20, 0), 0.1), 0.0); // sem corridas
        assert_eq!(overperformance_growth(&stats_pos(1, 1, 12), 0.1), 0.0); // grid de 1
    }

    #[test]
    fn stuck_talent_in_weak_car_grows_more_than_when_expected() {
        // Mesmo piloto capaz (teto de craque, skill baixo) e mesmo resultado (8º/20),
        // mas num carro FRACO (superou) vs num carro à altura (só cumpriu). O carro
        // fraco tem que render MAIS crescimento — é a conversão do "preso".
        let stats = stats_pos(8, 20, 12);
        let mut surprised = sample_driver(20, 55.0);
        let mut expected = sample_driver(20, 55.0);

        let mut rng_a = StdRng::seed_from_u64(99);
        let report_weak_car = calculate_growth(&mut surprised, &stats, 0.0, 3, 0.2, &mut rng_a);

        let mut rng_b = StdRng::seed_from_u64(99);
        let report_fair_car = calculate_growth(&mut expected, &stats, 0.0, 3, 0.6, &mut rng_b);

        assert!(
            report_weak_car.overall_delta > report_fair_car.overall_delta,
            "carro fraco={} deveria crescer mais que carro à altura={}",
            report_weak_car.overall_delta,
            report_fair_car.overall_delta
        );
        assert!(surprised.atributos.skill > 55.0);
    }

    // ── Maturação da juventude ────────────────────────────────────────────────
    #[test]
    fn maturation_only_for_the_young() {
        // Jovem capaz com folga cresce por tempo de pista; veterano igual não.
        let jovem = youth_maturation_growth(20, 58.0, 85.0, 12);
        let velho = youth_maturation_growth(30, 58.0, 85.0, 12);
        assert!(jovem > 0.0, "jovem capaz deveria maturar, got {jovem}");
        assert_eq!(velho, 0.0, "veterano não matura");
    }

    #[test]
    fn maturation_scales_with_folga_and_vanishes_at_ceiling() {
        // Muita folga cresce mais que pouca; quase-no-teto não pega nada.
        let muita = youth_maturation_growth(20, 55.0, 90.0, 12); // folga 35
        let pouca = youth_maturation_growth(20, 70.0, 76.0, 12); // folga 6
        let no_teto = youth_maturation_growth(20, 74.0, 76.0, 12); // folga 2 < min
        assert!(muita > pouca, "mais folga cresce mais: {muita} vs {pouca}");
        assert_eq!(no_teto, 0.0, "perto do teto não matura");
    }

    #[test]
    fn maturation_requires_seat_time() {
        assert_eq!(youth_maturation_growth(20, 55.0, 90.0, 0), 0.0);
    }

    #[test]
    fn young_prodigy_in_midfield_car_still_grows() {
        // Jovem capaz num carro adequado terminando ONDE O CARRO MANDA (percentil do
        // carro = posição real): não pódia, não supera — antes estagnava. Com a
        // maturação, ainda cresce só por correr a temporada.
        let stats = stats_pos(10, 20, 12); // meio de grid, sem pódio
        // carro percentil ~0.53 pra bater a posição real (10/20) → superação ≈ 0
        let mut prodigy = sample_driver(20, 56.0);
        let mut rng = StdRng::seed_from_u64(77);
        let report = calculate_growth(&mut prodigy, &stats, 0.0, 3, 0.53, &mut rng);
        assert!(
            prodigy.atributos.skill > 56.0,
            "prodígio jovem tem que crescer só por maturar, skill={}",
            prodigy.atributos.skill
        );
        assert!(report.overall_delta > 0.0);
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
