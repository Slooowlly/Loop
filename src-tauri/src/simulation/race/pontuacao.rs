//! Pontuação de segmento e degradação: os pesos por trecho da corrida, o quanto o carro pesa
//! por categoria e o desgaste de pneu e de físico cobrado ao fim de cada segmento.

use rand::Rng;

use crate::simulation::context::{SimDriver, SimulationContext};
use crate::simulation::math::{
    car_weight_scale, category_car_performance, normalize_car_performance, rain_intensity_for,
};
use crate::simulation::track_profile::TrackCharacter;

use super::tipos::{RaceSegment, RaceState};

#[derive(Debug, Clone, Copy)]
struct SegmentWeights {
    skill: f64,
    habilidade_largada: f64,
    racecraft: f64,
    car_performance: f64,
    gestao_pneus: f64,
    fitness: f64,
    mentalidade: f64,
    confianca: f64,
}

fn segment_weights(segment: RaceSegment) -> SegmentWeights {
    match segment {
        RaceSegment::Start => SegmentWeights {
            skill: 0.20,
            habilidade_largada: 0.35,
            racecraft: 0.25,
            car_performance: 0.20,
            gestao_pneus: 0.0,
            fitness: 0.0,
            mentalidade: 0.0,
            confianca: 0.0,
        },
        RaceSegment::Early => SegmentWeights {
            skill: 0.35,
            habilidade_largada: 0.0,
            racecraft: 0.20,
            car_performance: 0.30,
            gestao_pneus: 0.15,
            fitness: 0.0,
            mentalidade: 0.0,
            confianca: 0.0,
        },
        RaceSegment::Mid => SegmentWeights {
            skill: 0.35,
            habilidade_largada: 0.0,
            racecraft: 0.0,
            car_performance: 0.30,
            gestao_pneus: 0.20,
            fitness: 0.15,
            mentalidade: 0.0,
            confianca: 0.0,
        },
        RaceSegment::Late => SegmentWeights {
            skill: 0.25,
            habilidade_largada: 0.0,
            racecraft: 0.0,
            car_performance: 0.20,
            gestao_pneus: 0.25,
            fitness: 0.20,
            mentalidade: 0.10,
            confianca: 0.0,
        },
        RaceSegment::Finish => SegmentWeights {
            skill: 0.25,
            habilidade_largada: 0.0,
            racecraft: 0.25,
            car_performance: 0.20,
            gestao_pneus: 0.0,
            fitness: 0.0,
            mentalidade: 0.10,
            confianca: 0.20,
        },
    }
}

/// Escala o peso do `car_performance` por um fator e redistribui o delta
/// proporcionalmente aos demais pesos (de piloto), preservando a soma = 1.0.
/// fator < 1 desloca influência do carro para o piloto (rookie); fator > 1
/// aumenta a do carro (topo).
fn scale_segment_car_weight(mut w: SegmentWeights, scale: f64) -> SegmentWeights {
    let car = w.car_performance;
    let new_car = car * scale;
    let non_car_total = w.skill
        + w.habilidade_largada
        + w.racecraft
        + w.gestao_pneus
        + w.fitness
        + w.mentalidade
        + w.confianca;
    if non_car_total > 0.0 {
        // Tudo que sai (ou entra) no carro é redistribuído nos pesos de piloto.
        let factor = (non_car_total + (car - new_car)) / non_car_total;
        w.skill *= factor;
        w.habilidade_largada *= factor;
        w.racecraft *= factor;
        w.gestao_pneus *= factor;
        w.fitness *= factor;
        w.mentalidade *= factor;
        w.confianca *= factor;
    }
    w.car_performance = new_car;
    w
}

/// O RITMO DE VOLTA deste piloto neste trecho, em pontos (maior = mais rápido).
///
/// Continua sendo o cérebro da simulação — a combinação de piloto, carro, pneu, físico,
/// clima e caráter de pista. O que mudou é o que ele produz: era uma parcela de um saldo de
/// pontos, virou um ritmo que o motor converte em tempo. **Determinístico**: o ruído saiu
/// daqui (ver [`amplitude_de_ritmo`]).
pub(crate) fn calculate_segment_score(
    driver: &SimDriver,
    state: &RaceState,
    segment: RaceSegment,
    ctx: &SimulationContext,
) -> f64 {
    // Peso do carro escalado por categoria (rookie baixo, topo alto); carro spec
    // no rookie (todos idênticos). Pilar A do redesign carro/dinastias.
    let car_scale = car_weight_scale(&ctx.category_id);
    let weights = scale_segment_car_weight(segment_weights(segment), car_scale);
    let car_norm = normalize_car_performance(category_car_performance(
        &ctx.category_id,
        driver.car_performance,
    ));
    let mut score = driver.skill as f64 * weights.skill
        + driver.habilidade_largada as f64 * weights.habilidade_largada
        + driver.racecraft as f64 * weights.racecraft
        + car_norm * weights.car_performance
        + driver.gestao_pneus as f64 * weights.gestao_pneus
        + driver.fitness as f64 * weights.fitness
        + driver.mentalidade as f64 * weights.mentalidade
        + driver.confianca as f64 * weights.confianca;

    // Penalidade de pneu
    let tire_penalty = (1.0 - state.tire_wear) * 0.15;
    score *= 1.0 - tire_penalty;

    // Penalidade de fadiga (apenas Late e Finish)
    if matches!(segment, RaceSegment::Late | RaceSegment::Finish) {
        let fatigue_penalty = (1.0 - state.physical_condition) * 0.10;
        score *= 1.0 - fatigue_penalty;
    }

    // Chuva: MESMA penalidade de skill por piloto do export iRacing (curva validada
    // `rain_skill_penalty`). Seco = 0. Rain-good (fator alto) perde MENOS pontos →
    // re-rank consistente: o pelotão todo cai e os bons-de-chuva sobem relativos.
    // (O score está na escala ~0–100 do skill, então subtrair os pontos casa com o export.)
    // A `rain_sensitivity` da pista/categoria ESCALA a curva validada (pacote G). Antes ela era
    // calculada no perfil, guardada no contexto e nunca lida — chuva rendia igual em Spa e em
    // Tsukuba. Sensibilidade 1,0 devolve exatamente a penalidade de antes.
    score -= crate::simulation::math::rain_penalty_escalada(
        ctx.weather,
        driver.fator_chuva as f64,
        ctx.rain_sensitivity,
    );

    // Bônus contextual em pista difícil: adaptabilidade vale mais
    if ctx.track_difficulty_multiplier > 1.0 {
        let difficulty_bonus =
            (driver.adaptabilidade as f64 / 100.0) * (ctx.track_difficulty_multiplier - 1.0) * 0.05;
        let consistency_bonus =
            (driver.consistencia as f64 / 100.0) * (ctx.track_difficulty_multiplier - 1.0) * 0.03;
        score += difficulty_bonus + consistency_bonus;
    }

    // Bias de caráter de pista: pequenos ajustes relativos de atributos (skill, car, adaptabilidade)
    let (char_skill_bias, char_car_bias, char_adapt_bias) = match ctx.track_character {
        TrackCharacter::Flowing => (0.02_f64, 0.02, -0.03),
        TrackCharacter::Technical => (0.00, 0.00, 0.00),
        TrackCharacter::Tight => (-0.03, -0.04, 0.05),
        TrackCharacter::Roval => (0.04, 0.03, -0.05),
    };
    score += driver.skill as f64 * char_skill_bias
        + car_norm * char_car_bias * car_scale
        + driver.adaptabilidade as f64 * char_adapt_bias;

    // Comprime ou expande spread de habilidade (endurance = campo mais fechado, rookie = mais aberto)
    let midpoint = 60.0_f64;
    score = midpoint + (score - midpoint) * ctx.race_pace_spread_multiplier;

    if driver.corridas_na_categoria < 10 {
        let inexperience_factor = (10 - driver.corridas_na_categoria).max(0) as f64 * 0.003;
        score *= 1.0 - inexperience_factor;
    }

    score.max(5.0)
}

/// Amplitude do ruído de ritmo deste piloto neste trecho, em pontos de ritmo.
///
/// Era a faixa do sorteio que vivia dentro de [`calculate_segment_score`]. Saiu de lá porque
/// o ruído deixou de ser "um número por segmento": agora ele tem ESCALA (por volta, não por
/// segmento) e MEMÓRIA (correlacionado entre trechos), e as duas coisas são do laço da
/// corrida, não do cálculo de ritmo. A fórmula em si não mudou.
pub(crate) fn amplitude_de_ritmo(
    driver: &SimDriver,
    ctx: &SimulationContext,
    segment: RaceSegment,
) -> f64 {
    amplitude_de_ritmo_com_relancamento(driver, ctx, segment, false)
}

/// Idem, com a opção de tratar o trecho como RELANÇAMENTO de safety car.
///
/// Um relançamento é uma segunda largada — pneu frio, pelotão colado, acordeão — então ele
/// reusa exatamente a amplificação de caos que o segmento de largada já tinha, em vez de um
/// mecanismo novo. É o que impede o safety car de ser só uma animação que comprime gaps sem
/// mudar nada (ver `estrategia::CAOS_DO_RELANCAMENTO`).
pub(crate) fn amplitude_de_ritmo_com_relancamento(
    driver: &SimDriver,
    ctx: &SimulationContext,
    segment: RaceSegment,
    relancamento: bool,
) -> f64 {
    let base = (100.0 - driver.consistencia as f64) / 100.0 * 5.0;
    let escalada = base * ctx.race_variance_multiplier;

    // Caos extra na largada — e no relançamento — amplificado por densidade do pelotão.
    if segment == RaceSegment::Start {
        escalada * ctx.start_chaos_multiplier * ctx.pack_density_factor
    } else if relancamento {
        escalada
            * ctx.start_chaos_multiplier
            * ctx.pack_density_factor
            * super::estrategia::CAOS_DO_RELANCAMENTO
    } else {
        escalada
    }
}

pub(crate) fn apply_tire_degradation(
    state: &mut RaceState,
    driver: &SimDriver,
    ctx: &SimulationContext,
) {
    let mgmt_factor = 1.0 - (driver.gestao_pneus as f64 / 100.0 * 0.50);
    let smoothness_factor = 1.0 - (driver.smoothness as f64 / 100.0 * 0.20);
    let duration_factor = (ctx.race_duration_minutes as f64 / 30.0).max(0.25);
    let actual_degradation =
        ctx.tire_degradation_rate * mgmt_factor * smoothness_factor * duration_factor;
    state.tire_wear = (state.tire_wear - actual_degradation).max(0.1);
}

pub(crate) fn apply_physical_degradation(
    state: &mut RaceState,
    driver: &SimDriver,
    ctx: &SimulationContext,
) {
    let fit_factor = 1.0 - (driver.fitness as f64 / 100.0 * 0.60);
    let duration_factor = (ctx.race_duration_minutes as f64 / 30.0).max(0.25);
    let actual_degradation = ctx.physical_degradation_rate * fit_factor * duration_factor;
    state.physical_condition = (state.physical_condition - actual_degradation).max(0.2);
}
