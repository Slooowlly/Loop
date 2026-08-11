//! Os sorteios em si: pane, erro de pilotagem e colisão, mais a leitura da vizinhança do
//! pelotão que decide com quem o carro se bate.

use rand::Rng;

use crate::models::enums::WeatherCondition;

use crate::simulation::context::SimDriver;
use crate::simulation::race::{RaceSegment, RaceState};

use super::risco::{
    collision_segment_mult, driver_error_segment_mult, mechanical_segment_mult, rain_base,
    rain_collision_mult, ABSORCAO_DE_CHUVA_PELO_PILOTO, AGRESSAO_NEUTRA, AGRESSAO_SOMA_A_COLISAO,
    COLISAO_ATE_AQUI_CUSTA_CARO, COLISAO_ATE_AQUI_E_GRAVE, COLISAO_ATE_AQUI_E_LEVE,
    COLISAO_ATE_AQUI_TIRA_O_CARRO, COLISAO_NA_PONTA, COLISAO_NO_FUNDO, COLISAO_NO_MEIO,
    COLLISION_BASE_CHANCE, CONFIABILIDADE_ESCALA, CONFIABILIDADE_PESO, CONFIABILIDADE_PIVO,
    DIVISOR_DA_AGRESSAO_NO_ERRO, DRIVER_ERROR_BASE_CHANCE, EXPERIENCIA_REDUZ_O_ERRO,
    FADIGA_SOMA_AO_ERRO, FRACAO_DE_ERRO_LEVE, FRACAO_DE_PANE_LEVE, FRONTEIRA_DA_PONTA,
    FRONTEIRA_DO_MEIO, MECHANICAL_BASE_CHANCE, PNEU_GASTO_SOMA_AO_ERRO,
    POSICOES_PERDIDAS_NA_COLISAO_BARATA, POSICOES_PERDIDAS_NA_COLISAO_CARA,
    POSICOES_PERDIDAS_NO_LEVE, RACECRAFT_TIRA_DA_COLISAO, RAIO_DA_VIZINHANCA, SEGMENTS,
    SUAVIDADE_REDUZ_O_ERRO, TETO_DE_RISCO_DE_COLISAO, TETO_DE_RISCO_DE_ERRO,
    VIZINHANCA_SOMA_A_COLISAO,
};
use super::tipos::IncidentSeverity;

pub(crate) fn roll_mechanical(
    car_reliability: f64,
    segment: RaceSegment,
    incident_rate_multiplier: f64,
    rng: &mut impl Rng,
) -> Option<(IncidentSeverity, bool, i32)> {
    let base = MECHANICAL_BASE_CHANCE / SEGMENTS;
    let reliability_mod = (1.0
        - ((car_reliability - CONFIABILIDADE_PIVO) / CONFIABILIDADE_ESCALA * CONFIABILIDADE_PESO))
        .clamp(0.1, 3.0);
    let chance =
        base * reliability_mod * mechanical_segment_mult(segment) * incident_rate_multiplier;

    if rng.gen::<f64>() >= chance {
        return None;
    }

    let (min, max) = POSICOES_PERDIDAS_NO_LEVE;
    if rng.gen::<f64>() < FRACAO_DE_PANE_LEVE {
        Some((IncidentSeverity::Minor, false, rng.gen_range(min..=max)))
    } else {
        Some((IncidentSeverity::Major, true, 0))
    }
}

pub(crate) fn roll_driver_error(
    driver: &SimDriver,
    state: &RaceState,
    segment: RaceSegment,
    weather: WeatherCondition,
    _is_championship_deciding: bool,
    incident_rate_multiplier: f64,
    start_chaos_multiplier: f64,
    rng: &mut impl Rng,
) -> Option<(IncidentSeverity, bool, i32)> {
    let base = DRIVER_ERROR_BASE_CHANCE / SEGMENTS;

    let consistency_core = (1.0 - driver.consistencia as f64 / 100.0).max(0.05);
    let aggression_core = 1.0 + driver.aggression as f64 / DIVISOR_DA_AGRESSAO_NO_ERRO;
    let experience_mod = 1.0 - driver.experiencia as f64 / 100.0 * EXPERIENCIA_REDUZ_O_ERRO;
    let smoothness_mod = 1.0 - driver.smoothness as f64 / 100.0 * SUAVIDADE_REDUZ_O_ERRO;

    let rb = rain_base(weather);
    let rain_absorption = driver.fator_chuva as f64 / 100.0 * ABSORCAO_DE_CHUVA_PELO_PILOTO;
    let rain_penalty = rb * (1.0 - rain_absorption);

    // Pressão de campeonato (clutch/choke) é per-piloto, calculada no setup da
    // corrida (ver simulation::pressure). <1 acalma (clutch), >1 atrapalha (choke).
    let pressure_mod = driver.pressure_error_mult;

    let tire_mod = 1.0 + (1.0 - state.tire_wear) * PNEU_GASTO_SOMA_AO_ERRO;
    let fatigue_mod = 1.0 + (1.0 - state.physical_condition) * FADIGA_SOMA_AO_ERRO;

    let chaos_mult = if segment == RaceSegment::Start {
        start_chaos_multiplier
    } else {
        1.0
    };

    let chance = (base
        * consistency_core
        * aggression_core
        * experience_mod
        * smoothness_mod
        * (1.0 + rain_penalty)
        * pressure_mod
        * tire_mod
        * fatigue_mod
        * driver_error_segment_mult(segment)
        * incident_rate_multiplier
        * chaos_mult)
        .min(TETO_DE_RISCO_DE_ERRO);

    if rng.gen::<f64>() >= chance {
        return None;
    }

    let (min, max) = POSICOES_PERDIDAS_NO_LEVE;
    if rng.gen::<f64>() < FRACAO_DE_ERRO_LEVE {
        Some((IncidentSeverity::Minor, false, rng.gen_range(min..=max)))
    } else {
        Some((IncidentSeverity::Major, true, 0))
    }
}

pub(crate) fn roll_collision(
    driver: &SimDriver,
    position: i32,
    total_drivers: i32,
    avg_neighbor_aggression: f64,
    segment: RaceSegment,
    weather: WeatherCondition,
    incident_rate_multiplier: f64,
    start_chaos_multiplier: f64,
    pack_density_factor: f64,
    rng: &mut impl Rng,
) -> Option<IncidentSeverity> {
    let base = COLLISION_BASE_CHANCE / SEGMENTS;

    let aggression_mod = 1.0 + driver.aggression as f64 / 100.0 * AGRESSAO_SOMA_A_COLISAO;
    let racecraft_mod = 1.0 - driver.racecraft as f64 / 100.0 * RACECRAFT_TIRA_DA_COLISAO;
    let nearby_mod = 1.0 + avg_neighbor_aggression / 100.0 * VIZINHANCA_SOMA_A_COLISAO;

    let pct = position as f64 / total_drivers.max(1) as f64;
    let pack_mod = if pct <= FRONTEIRA_DA_PONTA {
        COLISAO_NA_PONTA
    } else if pct <= FRONTEIRA_DO_MEIO {
        COLISAO_NO_MEIO
    } else {
        COLISAO_NO_FUNDO
    };

    let chaos_mult = if segment == RaceSegment::Start {
        start_chaos_multiplier
    } else {
        1.0
    };

    let chance = (base
        * aggression_mod
        * racecraft_mod
        * nearby_mod
        * pack_mod
        * rain_collision_mult(weather)
        * collision_segment_mult(segment)
        * incident_rate_multiplier
        * chaos_mult
        * pack_density_factor)
        .min(TETO_DE_RISCO_DE_COLISAO);

    if rng.gen::<f64>() >= chance {
        return None;
    }

    let roll = rng.gen::<f64>();
    if roll < COLISAO_ATE_AQUI_E_LEVE {
        Some(IncidentSeverity::Minor)
    } else if roll < COLISAO_ATE_AQUI_E_GRAVE {
        Some(IncidentSeverity::Major)
    } else {
        Some(IncidentSeverity::Critical)
    }
}

pub(crate) fn resolve_collision_consequence(rng: &mut impl Rng) -> (bool, i32) {
    let roll = rng.gen::<f64>();
    if roll < COLISAO_ATE_AQUI_TIRA_O_CARRO {
        (true, 0)
    } else if roll < COLISAO_ATE_AQUI_CUSTA_CARO {
        let (min, max) = POSICOES_PERDIDAS_NA_COLISAO_CARA;
        (false, rng.gen_range(min..=max))
    } else {
        let (min, max) = POSICOES_PERDIDAS_NA_COLISAO_BARATA;
        (false, rng.gen_range(min..=max))
    }
}

pub(crate) fn avg_neighbor_aggression(
    driver_id: &str,
    position: i32,
    drivers: &[SimDriver],
    states: &[RaceState],
) -> f64 {
    let mut total = 0.0;
    let mut count = 0;

    for state in states {
        if state.is_dnf || state.driver_id == driver_id {
            continue;
        }
        if (state.current_position - position).abs() <= RAIO_DA_VIZINHANCA {
            if let Some(neighbor) = drivers.iter().find(|d| d.id == state.driver_id) {
                total += neighbor.aggression as f64;
                count += 1;
            }
        }
    }

    if count > 0 {
        total / count as f64
    } else {
        AGRESSAO_NEUTRA
    }
}

pub(crate) fn find_neighbor(
    driver_id: &str,
    position: i32,
    states: &[RaceState],
    excluded: &[String],
) -> Option<String> {
    for target_pos in [position + 1, position - 1] {
        if let Some(state) = states.iter().find(|s| {
            s.current_position == target_pos
                && !s.is_dnf
                && s.driver_id != driver_id
                && !excluded.contains(&s.driver_id)
        }) {
            return Some(state.driver_id.clone());
        }
    }
    None
}
