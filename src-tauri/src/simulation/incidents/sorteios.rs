//! Os sorteios em si: pane, erro de pilotagem e colisão, mais a leitura da vizinhança do
//! pelotão que decide com quem o carro se bate.

use rand::Rng;

use crate::models::enums::WeatherCondition;

use crate::simulation::context::SimDriver;
use crate::simulation::race::{RaceSegment, RaceState};

use super::risco::{
    collision_segment_mult, driver_error_segment_mult, mechanical_segment_mult, rain_base,
    rain_collision_mult, COLLISION_BASE_CHANCE, DRIVER_ERROR_BASE_CHANCE, MECHANICAL_BASE_CHANCE,
    SEGMENTS,
};
use super::tipos::IncidentSeverity;

pub(crate) fn roll_mechanical(
    car_reliability: f64,
    segment: RaceSegment,
    incident_rate_multiplier: f64,
    rng: &mut impl Rng,
) -> Option<(IncidentSeverity, bool, i32)> {
    let base = MECHANICAL_BASE_CHANCE / SEGMENTS;
    let reliability_mod = (1.0 - ((car_reliability - 70.0) / 25.0 * 0.70)).clamp(0.1, 3.0);
    let chance =
        base * reliability_mod * mechanical_segment_mult(segment) * incident_rate_multiplier;

    if rng.gen::<f64>() >= chance {
        return None;
    }

    if rng.gen::<f64>() < 0.15 {
        Some((IncidentSeverity::Minor, false, rng.gen_range(1..=4)))
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
    let aggression_core = 1.0 + driver.aggression as f64 / 200.0;
    let experience_mod = 1.0 - driver.experiencia as f64 / 100.0 * 0.30;
    let smoothness_mod = 1.0 - driver.smoothness as f64 / 100.0 * 0.25;

    let rb = rain_base(weather);
    let rain_absorption = driver.fator_chuva as f64 / 100.0 * 0.80;
    let rain_penalty = rb * (1.0 - rain_absorption);

    // Pressão de campeonato (clutch/choke) é per-piloto, calculada no setup da
    // corrida (ver simulation::pressure). <1 acalma (clutch), >1 atrapalha (choke).
    let pressure_mod = driver.pressure_error_mult;

    let tire_mod = 1.0 + (1.0 - state.tire_wear) * 0.5;
    let fatigue_mod = 1.0 + (1.0 - state.physical_condition) * 0.4;

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
        .min(0.25);

    if rng.gen::<f64>() >= chance {
        return None;
    }

    if rng.gen::<f64>() < 0.70 {
        Some((IncidentSeverity::Minor, false, rng.gen_range(1..=4)))
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

    let aggression_mod = 1.0 + driver.aggression as f64 / 100.0 * 0.60;
    let racecraft_mod = 1.0 - driver.racecraft as f64 / 100.0 * 0.50;
    let nearby_mod = 1.0 + avg_neighbor_aggression / 100.0 * 0.30;

    let pct = position as f64 / total_drivers.max(1) as f64;
    let pack_mod = if pct <= 0.25 {
        0.7
    } else if pct <= 0.75 {
        1.2
    } else {
        0.9
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
        .min(0.20);

    if rng.gen::<f64>() >= chance {
        return None;
    }

    let roll = rng.gen::<f64>();
    if roll < 0.55 {
        Some(IncidentSeverity::Minor)
    } else if roll < 0.95 {
        Some(IncidentSeverity::Major)
    } else {
        Some(IncidentSeverity::Critical)
    }
}

pub(crate) fn resolve_collision_consequence(rng: &mut impl Rng) -> (bool, i32) {
    let roll = rng.gen::<f64>();
    if roll < 0.40 {
        (true, 0)
    } else if roll < 0.70 {
        (false, rng.gen_range(3..=5))
    } else {
        (false, rng.gen_range(1..=2))
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
        if (state.current_position - position).abs() <= 2 {
            if let Some(neighbor) = drivers.iter().find(|d| d.id == state.driver_id) {
                total += neighbor.aggression as f64;
                count += 1;
            }
        }
    }

    if count > 0 {
        total / count as f64
    } else {
        50.0
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
