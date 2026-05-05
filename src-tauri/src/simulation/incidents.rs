#![allow(dead_code)]

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::models::enums::WeatherCondition;

use super::catalog::{IncidentCatalog, IncidentSource, TriggerType, VehicleClass};
use super::context::SimDriver;
use super::race::{RaceSegment, RaceState};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum IncidentType {
    Mechanical,
    DriverError,
    Collision,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum IncidentSeverity {
    Minor,
    Major,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentResult {
    pub pilot_id: String,
    pub incident_type: IncidentType,
    pub severity: IncidentSeverity,
    pub segment: String,
    pub positions_lost: i32,
    pub is_dnf: bool,
    pub description: String,
    #[serde(default)]
    pub linked_pilot_id: Option<String>,
    pub is_two_car_incident: bool,
    pub injury_risk_multiplier: f64,
    pub narrative_importance_hint: u8,
    /// ID da entry do catálogo de incidentes. None para incidentes sem catálogo (catálogo vazio
    /// ou versões anteriores do motor).
    #[serde(default)]
    pub catalog_id: Option<String>,
    /// Segmento onde o dano se originou (para dano pós-colisão latente).
    /// Difere de `segment` quando o dano foi causado por colisão anterior.
    #[serde(default)]
    pub damage_origin_segment: Option<String>,
}

/// Dano pós-colisão com possibilidade de manifestação latente em segmentos futuros.
#[derive(Debug, Clone)]
pub struct PendingDamage {
    /// ID da entry do catálogo (PostCollision).
    pub catalog_id: String,
    /// Segmento onde a colisão originou o dano.
    pub origin_segment: String,
    /// Chance de manifestação neste segmento; aumenta +0.15 por segmento sem manifestação.
    pub manifest_chance: f64,
    /// true se a colisão original era Major (dano pode causar DNF).
    pub is_dnf_capable: bool,
}

/// Retorno de `process_segment_incidents`, carregando incidentes do segmento e novos danos latentes.
pub struct SegmentIncidentResult {
    pub incidents: Vec<IncidentResult>,
    /// Pares (driver_id, PendingDamage) a serem adicionados aos estados correspondentes.
    pub new_pending_damage: Vec<(String, PendingDamage)>,
}

const MECHANICAL_BASE_CHANCE: f64 = 0.015;
const DRIVER_ERROR_BASE_CHANCE: f64 = 0.017;
const COLLISION_BASE_CHANCE: f64 = 0.006;
const SEGMENTS: f64 = 5.0;

fn mechanical_segment_mult(segment: RaceSegment) -> f64 {
    match segment {
        RaceSegment::Start => 0.5,
        RaceSegment::Early => 0.8,
        RaceSegment::Mid => 1.0,
        RaceSegment::Late => 1.2,
        RaceSegment::Finish => 1.5,
    }
}

fn driver_error_segment_mult(segment: RaceSegment) -> f64 {
    match segment {
        RaceSegment::Start => 1.5,
        RaceSegment::Early => 1.0,
        RaceSegment::Mid => 1.0,
        RaceSegment::Late => 1.2,
        RaceSegment::Finish => 1.5,
    }
}

fn collision_segment_mult(segment: RaceSegment) -> f64 {
    match segment {
        RaceSegment::Start => 2.5,
        RaceSegment::Early => 1.0,
        RaceSegment::Mid => 0.8,
        RaceSegment::Late => 0.8,
        RaceSegment::Finish => 1.2,
    }
}

fn rain_base(weather: WeatherCondition) -> f64 {
    match weather {
        WeatherCondition::Dry => 0.0,
        WeatherCondition::Damp => 0.30,
        WeatherCondition::Wet => 0.60,
        WeatherCondition::HeavyRain => 1.00,
    }
}

fn rain_collision_mult(weather: WeatherCondition) -> f64 {
    match weather {
        WeatherCondition::Dry => 1.0,
        WeatherCondition::Damp => 1.2,
        WeatherCondition::Wet => 1.4,
        WeatherCondition::HeavyRain => 1.6,
    }
}

fn compute_irm(incident_type: IncidentType, severity: IncidentSeverity) -> f64 {
    match (incident_type, severity) {
        (IncidentType::Collision, IncidentSeverity::Critical) => 1.5,
        (IncidentType::Collision, IncidentSeverity::Major) => 0.45,
        (IncidentType::DriverError, IncidentSeverity::Critical) => 1.0,
        (IncidentType::Mechanical, IncidentSeverity::Critical) => 0.6,
        _ => 0.0,
    }
}

pub(crate) fn injury_base_chance(incident_type: IncidentType) -> f64 {
    match incident_type {
        IncidentType::Collision => 0.50,
        IncidentType::DriverError => 0.40,
        IncidentType::Mechanical => 0.25,
    }
}

fn compute_narrative_hint(severity: IncidentSeverity, incident_type: IncidentType) -> u8 {
    match (severity, incident_type) {
        (IncidentSeverity::Critical, _) => 2,
        (IncidentSeverity::Major, IncidentType::Collision) => 1,
        _ => 0,
    }
}

fn roll_mechanical(
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

fn roll_driver_error(
    driver: &SimDriver,
    state: &RaceState,
    segment: RaceSegment,
    weather: WeatherCondition,
    is_championship_deciding: bool,
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

    let pressure_mod = if is_championship_deciding {
        1.3 - driver.mentalidade as f64 / 100.0 * 0.25
    } else {
        1.0
    };

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

fn roll_collision(
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

fn resolve_collision_consequence(rng: &mut impl Rng) -> (bool, i32) {
    let roll = rng.gen::<f64>();
    if roll < 0.40 {
        (true, 0)
    } else if roll < 0.70 {
        (false, rng.gen_range(3..=5))
    } else {
        (false, rng.gen_range(1..=2))
    }
}

fn avg_neighbor_aggression(
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

fn find_neighbor(
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

fn make_incident(
    pilot_id: String,
    incident_type: IncidentType,
    severity: IncidentSeverity,
    segment: &str,
    positions_lost: i32,
    is_dnf: bool,
    description: String,
    linked_pilot_id: Option<String>,
    is_two_car_incident: bool,
    catalog_id: Option<String>,
    damage_origin_segment: Option<String>,
) -> IncidentResult {
    IncidentResult {
        injury_risk_multiplier: compute_irm(incident_type, severity),
        narrative_importance_hint: compute_narrative_hint(severity, incident_type),
        pilot_id,
        incident_type,
        severity,
        segment: segment.to_string(),
        positions_lost,
        is_dnf,
        description,
        linked_pilot_id,
        is_two_car_incident,
        catalog_id,
        damage_origin_segment,
    }
}

pub fn process_segment_incidents(
    drivers: &[SimDriver],
    states: &[RaceState],
    segment: RaceSegment,
    weather: WeatherCondition,
    is_championship_deciding: bool,
    incident_rate_multiplier: f64,
    start_chaos_multiplier: f64,
    pack_density_factor: f64,
    catalog: &IncidentCatalog,
    vehicle_class: VehicleClass,
    is_endurance: bool,
    rng: &mut impl Rng,
) -> SegmentIncidentResult {
    let mut incidents = Vec::new();
    let mut new_pending_damage: Vec<(String, PendingDamage)> = Vec::new();
    let total_drivers = states.len() as i32;
    let seg = segment.as_str();
    let mut affected: Vec<String> = Vec::new();

    for state in states {
        if state.is_dnf || affected.contains(&state.driver_id) {
            continue;
        }

        let Some(driver) = drivers.iter().find(|d| d.id == state.driver_id) else {
            continue;
        };

        if let Some((severity, is_dnf, pos_lost)) = roll_mechanical(
            driver.car_reliability,
            segment,
            incident_rate_multiplier,
            rng,
        ) {
            let generic_desc = if is_dnf {
                format!("{} abandona com problema mecanico", driver.nome)
            } else {
                format!(
                    "{} perde {} posicoes por problema mecanico",
                    driver.nome, pos_lost
                )
            };
            let (catalog_id, desc) = match catalog.select_and_render(
                vehicle_class,
                is_endurance,
                IncidentSource::Mechanical,
                TriggerType::Spontaneous,
                is_dnf,
                &driver.nome,
                rng,
            ) {
                Some(sel) => (Some(sel.catalog_id), sel.rendered_text),
                None => (None, generic_desc),
            };
            incidents.push(make_incident(
                driver.id.clone(),
                IncidentType::Mechanical,
                severity,
                seg,
                pos_lost,
                is_dnf,
                desc,
                None,
                false,
                catalog_id,
                None,
            ));
            affected.push(driver.id.clone());
            continue;
        }

        if let Some((severity, is_dnf, pos_lost)) = roll_driver_error(
            driver,
            state,
            segment,
            weather,
            is_championship_deciding,
            incident_rate_multiplier,
            start_chaos_multiplier,
            rng,
        ) {
            // Stall check: DriverError Minor não-DNF pode escalar para DNF por stall
            let (final_severity, final_is_dnf, final_pos_lost, stall_catalog_id) =
                if severity == IncidentSeverity::Minor && !is_dnf {
                    let stall_base = 0.08;
                    let exp_mod = 1.0 - (driver.experiencia as f64 / 100.0 * 0.50);
                    let stall_chance = (stall_base * exp_mod).clamp(0.01, 0.08);

                    if rng.gen::<f64>() < stall_chance {
                        let stall_id = catalog
                            .select_and_render(
                                vehicle_class,
                                is_endurance,
                                IncidentSource::Operational,
                                TriggerType::PostSpinStall,
                                true,
                                &driver.nome,
                                rng,
                            )
                            .map(|sel| (sel.catalog_id, sel.rendered_text));
                        (IncidentSeverity::Major, true, 0, stall_id)
                    } else {
                        (severity, is_dnf, pos_lost, None)
                    }
                } else {
                    (severity, is_dnf, pos_lost, None)
                };

            let (catalog_id, desc) = if let Some((sid, stall_text)) = stall_catalog_id {
                (Some(sid), stall_text)
            } else {
                let generic_desc = if final_is_dnf {
                    format!("{} abandona apos erro de pilotagem", driver.nome)
                } else {
                    format!(
                        "{} comete erro e perde {} posicoes",
                        driver.nome, final_pos_lost
                    )
                };
                match catalog.select_and_render(
                    vehicle_class,
                    is_endurance,
                    IncidentSource::DriverError,
                    TriggerType::Spontaneous,
                    final_is_dnf,
                    &driver.nome,
                    rng,
                ) {
                    Some(sel) => (Some(sel.catalog_id), sel.rendered_text),
                    None => (None, generic_desc),
                }
            };

            incidents.push(make_incident(
                driver.id.clone(),
                IncidentType::DriverError,
                final_severity,
                seg,
                final_pos_lost,
                final_is_dnf,
                desc,
                None,
                false,
                catalog_id,
                None,
            ));
            affected.push(driver.id.clone());
            continue;
        }

        let neighbor_agg =
            avg_neighbor_aggression(&driver.id, state.current_position, drivers, states);

        if let Some(severity) = roll_collision(
            driver,
            state.current_position,
            total_drivers,
            neighbor_agg,
            segment,
            weather,
            incident_rate_multiplier,
            start_chaos_multiplier,
            pack_density_factor,
            rng,
        ) {
            let neighbor_id = find_neighbor(&driver.id, state.current_position, states, &affected);
            let has_neighbor = neighbor_id.is_some();
            let (trig_dnf, trig_lost) = resolve_collision_consequence(rng);
            // Resolução 4: colisões diretas não consultam o catálogo — texto genérico preservado.
            // O catálogo PostCollision é usado apenas para dano latente (Fase 2).
            let trig_desc = if trig_dnf {
                format!("{} abandona apos colisao", driver.nome)
            } else {
                format!("{} perde {} posicoes em colisao", driver.nome, trig_lost)
            };
            incidents.push(make_incident(
                driver.id.clone(),
                IncidentType::Collision,
                severity,
                seg,
                trig_lost,
                trig_dnf,
                trig_desc,
                neighbor_id.clone(),
                has_neighbor,
                None,
                None,
            ));
            affected.push(driver.id.clone());

            // Roll pós-colisão para o trigger (somente se não-DNF)
            if !trig_dnf {
                maybe_add_pending_damage(
                    &driver.id,
                    severity,
                    seg,
                    catalog,
                    vehicle_class,
                    is_endurance,
                    driver.car_reliability,
                    &mut new_pending_damage,
                    rng,
                );
            }

            if let Some(ref neighbor_id) = neighbor_id {
                let neighbor = drivers.iter().find(|d| d.id == *neighbor_id);
                let neighbor_name = neighbor.map(|d| d.nome.as_str()).unwrap_or("Piloto");
                let (nb_dnf, nb_lost) = resolve_collision_consequence(rng);
                let nb_desc = if nb_dnf {
                    format!(
                        "{} abandona apos colisao com {}",
                        neighbor_name, driver.nome
                    )
                } else {
                    format!(
                        "{} perde {} posicoes em colisao com {}",
                        neighbor_name, nb_lost, driver.nome
                    )
                };
                incidents.push(make_incident(
                    neighbor_id.clone(),
                    IncidentType::Collision,
                    severity,
                    seg,
                    nb_lost,
                    nb_dnf,
                    nb_desc,
                    Some(driver.id.clone()),
                    true,
                    None,
                    None,
                ));
                affected.push(neighbor_id.clone());

                // Roll pós-colisão para o neighbor (somente se não-DNF)
                if !nb_dnf {
                    if let Some(nb_driver) = neighbor {
                        maybe_add_pending_damage(
                            neighbor_id,
                            severity,
                            seg,
                            catalog,
                            vehicle_class,
                            is_endurance,
                            nb_driver.car_reliability,
                            &mut new_pending_damage,
                            rng,
                        );
                    }
                }
            }
        }
    }

    SegmentIncidentResult {
        incidents,
        new_pending_damage,
    }
}

/// Roll de dano pós-colisão: se sucesso, cria um PendingDamage e o adiciona à lista.
fn maybe_add_pending_damage(
    driver_id: &str,
    severity: IncidentSeverity,
    origin_segment: &str,
    catalog: &IncidentCatalog,
    vehicle_class: VehicleClass,
    is_endurance: bool,
    car_reliability: f64,
    pending: &mut Vec<(String, PendingDamage)>,
    rng: &mut impl Rng,
) {
    let base_chance = match severity {
        IncidentSeverity::Major | IncidentSeverity::Critical => 0.45,
        IncidentSeverity::Minor => 0.25,
    };
    let reliability_mod = 1.0 - (car_reliability / 100.0 * 0.40);
    let chance = (base_chance * reliability_mod).clamp(0.05, 0.80);

    if rng.gen::<f64>() < chance {
        if let Some(sel) = catalog.select_and_render(
            vehicle_class,
            is_endurance,
            IncidentSource::PostCollision,
            TriggerType::PostCollision,
            false, // is_dnf: seleciona template inicial (manifestação decide DNF)
            "?",   // placeholder — será substituído quando o dano manifestar
            rng,
        ) {
            pending.push((
                driver_id.to_string(),
                PendingDamage {
                    catalog_id: sel.catalog_id,
                    origin_segment: origin_segment.to_string(),
                    manifest_chance: 0.20,
                    is_dnf_capable: matches!(
                        severity,
                        IncidentSeverity::Major | IncidentSeverity::Critical
                    ),
                },
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    fn make_driver(
        id: &str,
        consistency: u8,
        aggression: u8,
        racecraft: u8,
        reliability: f64,
    ) -> SimDriver {
        SimDriver {
            id: id.to_string(),
            nome: format!("Driver {id}"),
            is_jogador: false,
            skill: 70,
            consistencia: consistency,
            racecraft,
            defesa: 50,
            ritmo_classificacao: 70,
            gestao_pneus: 60,
            habilidade_largada: 60,
            adaptabilidade: 50,
            fator_chuva: 50,
            fitness: 70,
            experiencia: 50,
            aggression,
            smoothness: 50,
            mentalidade: 60,
            confianca: 60,
            car_performance: 8.0,
            car_reliability: reliability,
            team_id: format!("T{id}"),
            team_name: format!("Team {id}"),
            corridas_na_categoria: 10,
        }
    }

    fn make_state(id: &str, position: i32) -> RaceState {
        RaceState {
            driver_id: id.to_string(),
            tire_wear: 1.0,
            physical_condition: 1.0,
            cumulative_score: 100.0 - position as f64 * 5.0,
            is_dnf: false,
            current_position: position,
            incidents: Vec::new(),
            dnf_reason: None,
            dnf_segment: None,
            pending_damage: Vec::new(),
        }
    }

    #[test]
    fn test_safe_driver_rarely_has_incidents() {
        let drivers = vec![make_driver("P1", 95, 30, 85, 95.0)];
        let states = vec![make_state("P1", 1)];
        let mut rng = StdRng::seed_from_u64(42);

        let mut total = 0;
        for _ in 0..200 {
            let inc = process_segment_incidents(
                &drivers,
                &states,
                RaceSegment::Mid,
                WeatherCondition::Dry,
                false,
                1.0,
                1.0,
                1.0,
                &IncidentCatalog::empty(),
                VehicleClass::StreetBased,
                false,
                &mut rng,
            );
            let inc = inc.incidents;
            total += inc.len();
        }

        assert!(
            total < 20,
            "safe driver had {total} incidents in 200 segments"
        );
    }

    #[test]
    fn test_unreliable_car_has_more_mechanicals() {
        let good = make_driver("G", 70, 50, 70, 95.0);
        let bad = make_driver("B", 70, 50, 70, 30.0);
        let mut rng = StdRng::seed_from_u64(123);

        let (mut good_mech, mut bad_mech) = (0, 0);
        for _ in 0..1000 {
            let inc = process_segment_incidents(
                &[good.clone()],
                &[make_state("G", 1)],
                RaceSegment::Mid,
                WeatherCondition::Dry,
                false,
                1.0,
                1.0,
                1.0,
                &IncidentCatalog::empty(),
                VehicleClass::StreetBased,
                false,
                &mut rng,
            );
            let inc = inc.incidents;
            good_mech += inc
                .iter()
                .filter(|i| i.incident_type == IncidentType::Mechanical)
                .count();

            let inc = process_segment_incidents(
                &[bad.clone()],
                &[make_state("B", 1)],
                RaceSegment::Mid,
                WeatherCondition::Dry,
                false,
                1.0,
                1.0,
                1.0,
                &IncidentCatalog::empty(),
                VehicleClass::StreetBased,
                false,
                &mut rng,
            );
            let inc = inc.incidents;
            bad_mech += inc
                .iter()
                .filter(|i| i.incident_type == IncidentType::Mechanical)
                .count();
        }

        assert!(
            bad_mech > good_mech,
            "bad={bad_mech} should > good={good_mech}"
        );
    }

    #[test]
    fn test_rain_increases_driver_errors() {
        let driver = make_driver("P1", 60, 50, 70, 80.0);
        let mut rng = StdRng::seed_from_u64(456);

        let (mut dry_err, mut wet_err) = (0, 0);
        for _ in 0..1000 {
            let state = make_state("P1", 5);
            let inc = process_segment_incidents(
                &[driver.clone()],
                &[state.clone()],
                RaceSegment::Mid,
                WeatherCondition::Dry,
                false,
                1.0,
                1.0,
                1.0,
                &IncidentCatalog::empty(),
                VehicleClass::StreetBased,
                false,
                &mut rng,
            );
            let inc = inc.incidents;
            dry_err += inc
                .iter()
                .filter(|i| i.incident_type == IncidentType::DriverError)
                .count();

            let inc = process_segment_incidents(
                &[driver.clone()],
                &[state],
                RaceSegment::Mid,
                WeatherCondition::HeavyRain,
                false,
                1.0,
                1.0,
                1.0,
                &IncidentCatalog::empty(),
                VehicleClass::StreetBased,
                false,
                &mut rng,
            );
            let inc = inc.incidents;
            wet_err += inc
                .iter()
                .filter(|i| i.incident_type == IncidentType::DriverError)
                .count();
        }

        assert!(wet_err > dry_err, "wet={wet_err} should > dry={dry_err}");
    }

    #[test]
    fn test_collision_can_involve_neighbor() {
        let drivers: Vec<_> = (1..=6)
            .map(|i| make_driver(&format!("P{i}"), 50, 90, 30, 80.0))
            .collect();
        let states: Vec<_> = (1..=6).map(|i| make_state(&format!("P{i}"), i)).collect();
        let mut rng = StdRng::seed_from_u64(789);

        let mut pairs = 0;
        for _ in 0..500 {
            let inc = process_segment_incidents(
                &drivers,
                &states,
                RaceSegment::Start,
                WeatherCondition::Dry,
                false,
                1.0,
                1.0,
                1.0,
                &IncidentCatalog::empty(),
                VehicleClass::StreetBased,
                false,
                &mut rng,
            );
            let inc = inc.incidents;
            let collisions = inc
                .iter()
                .filter(|i| i.incident_type == IncidentType::Collision)
                .count();
            if collisions >= 2 {
                pairs += 1;
            }
        }

        assert!(pairs > 0, "should produce collision pairs");
    }

    #[test]
    fn test_dnf_driver_not_processed() {
        let drivers = vec![make_driver("P1", 30, 90, 30, 20.0)];
        let mut state = make_state("P1", 1);
        state.is_dnf = true;

        let mut rng = StdRng::seed_from_u64(111);
        let inc = process_segment_incidents(
            &drivers,
            &[state],
            RaceSegment::Start,
            WeatherCondition::HeavyRain,
            true,
            1.0,
            1.0,
            1.0,
            &IncidentCatalog::empty(),
            VehicleClass::StreetBased,
            false,
            &mut rng,
        );
        assert!(inc.incidents.is_empty());
    }

    #[test]
    fn test_start_segment_more_collisions_than_mid() {
        let drivers: Vec<_> = (1..=12)
            .map(|i| make_driver(&format!("P{i}"), 60, 65, 55, 80.0))
            .collect();
        let states: Vec<_> = (1..=12).map(|i| make_state(&format!("P{i}"), i)).collect();
        let mut rng = StdRng::seed_from_u64(333);

        let (mut start_c, mut mid_c) = (0, 0);
        for _ in 0..500 {
            let inc = process_segment_incidents(
                &drivers,
                &states,
                RaceSegment::Start,
                WeatherCondition::Dry,
                false,
                1.0,
                1.0,
                1.0,
                &IncidentCatalog::empty(),
                VehicleClass::StreetBased,
                false,
                &mut rng,
            );
            let inc = inc.incidents;
            start_c += inc
                .iter()
                .filter(|i| i.incident_type == IncidentType::Collision)
                .count();

            let inc = process_segment_incidents(
                &drivers,
                &states,
                RaceSegment::Mid,
                WeatherCondition::Dry,
                false,
                1.0,
                1.0,
                1.0,
                &IncidentCatalog::empty(),
                VehicleClass::StreetBased,
                false,
                &mut rng,
            );
            let inc = inc.incidents;
            mid_c += inc
                .iter()
                .filter(|i| i.incident_type == IncidentType::Collision)
                .count();
        }

        assert!(start_c > mid_c, "start={start_c} should > mid={mid_c}");
    }

    #[test]
    fn test_one_incident_per_driver_per_segment() {
        let drivers: Vec<_> = (1..=8)
            .map(|i| make_driver(&format!("P{i}"), 40, 80, 30, 40.0))
            .collect();
        let states: Vec<_> = (1..=8).map(|i| make_state(&format!("P{i}"), i)).collect();
        let mut rng = StdRng::seed_from_u64(555);

        for _ in 0..200 {
            let inc = process_segment_incidents(
                &drivers,
                &states,
                RaceSegment::Start,
                WeatherCondition::Wet,
                true,
                1.0,
                1.0,
                1.0,
                &IncidentCatalog::empty(),
                VehicleClass::StreetBased,
                false,
                &mut rng,
            );
            let inc = inc.incidents;
            let mut seen = HashSet::new();
            for incident in &inc {
                assert!(
                    seen.insert(&incident.pilot_id),
                    "driver {} had duplicate incident",
                    incident.pilot_id
                );
            }
        }
    }

    #[test]
    fn test_start_chaos_multiplier_increases_start_collisions() {
        let drivers: Vec<_> = (1..=12)
            .map(|i| make_driver(&format!("P{i}"), 60, 65, 55, 80.0))
            .collect();
        let states: Vec<_> = (1..=12).map(|i| make_state(&format!("P{i}"), i)).collect();
        let mut rng_normal = StdRng::seed_from_u64(9001);
        let mut rng_chaos = StdRng::seed_from_u64(9001);

        let (mut normal_c, mut chaos_c) = (0, 0);
        for _ in 0..500 {
            let inc = process_segment_incidents(
                &drivers,
                &states,
                RaceSegment::Start,
                WeatherCondition::Dry,
                false,
                1.0,
                1.0,
                1.0,
                &IncidentCatalog::empty(),
                VehicleClass::StreetBased,
                false,
                &mut rng_normal,
            );
            let inc = inc.incidents;
            normal_c += inc
                .iter()
                .filter(|i| i.incident_type == IncidentType::Collision)
                .count();

            let inc = process_segment_incidents(
                &drivers,
                &states,
                RaceSegment::Start,
                WeatherCondition::Dry,
                false,
                1.0,
                2.0,
                1.0,
                &IncidentCatalog::empty(),
                VehicleClass::StreetBased,
                false,
                &mut rng_chaos,
            );
            let inc = inc.incidents;
            chaos_c += inc
                .iter()
                .filter(|i| i.incident_type == IncidentType::Collision)
                .count();
        }

        assert!(
            chaos_c > normal_c,
            "chaos={chaos_c} should > normal={normal_c}"
        );
    }

    #[test]
    fn test_injury_risk_multiplier_collision_gt_mechanical() {
        let collision_irm = compute_irm(IncidentType::Collision, IncidentSeverity::Critical);
        let mechanical_irm = compute_irm(IncidentType::Mechanical, IncidentSeverity::Critical);
        assert!(
            collision_irm > mechanical_irm,
            "collision IRM={collision_irm} should > mechanical IRM={mechanical_irm}"
        );
    }

    #[test]
    fn test_smoothness_reduces_driver_error_frequency() {
        let mut smooth = make_driver("SMOOTH", 55, 70, 40, 85.0);
        smooth.smoothness = 95;

        let mut rough = smooth.clone();
        rough.id = "ROUGH".to_string();
        rough.nome = "ROUGH".to_string();
        rough.smoothness = 10;

        let state = make_state("SMOOTH", 1);
        let runs = 5_000;
        let mut smooth_rng = StdRng::seed_from_u64(2026);
        let mut rough_rng = StdRng::seed_from_u64(2026);
        let mut smooth_errors = 0;
        let mut rough_errors = 0;

        for _ in 0..runs {
            if roll_driver_error(
                &smooth,
                &state,
                RaceSegment::Mid,
                WeatherCondition::Wet,
                false,
                1.0,
                1.0,
                &mut smooth_rng,
            )
            .is_some()
            {
                smooth_errors += 1;
            }

            if roll_driver_error(
                &rough,
                &state,
                RaceSegment::Mid,
                WeatherCondition::Wet,
                false,
                1.0,
                1.0,
                &mut rough_rng,
            )
            .is_some()
            {
                rough_errors += 1;
            }
        }

        assert!(
            smooth_errors < rough_errors,
            "smooth_errors={smooth_errors} should be lower than rough_errors={rough_errors}"
        );
    }

    #[test]
    fn test_is_two_car_incident_bilateral() {
        let drivers: Vec<_> = (1..=6)
            .map(|i| make_driver(&format!("P{i}"), 50, 90, 20, 80.0))
            .collect();
        let states: Vec<_> = (1..=6).map(|i| make_state(&format!("P{i}"), i)).collect();
        let mut rng = StdRng::seed_from_u64(7777);

        let mut found_bilateral = false;
        'outer: for _ in 0..500 {
            let inc = process_segment_incidents(
                &drivers,
                &states,
                RaceSegment::Start,
                WeatherCondition::Dry,
                false,
                1.0,
                1.0,
                1.0,
                &IncidentCatalog::empty(),
                VehicleClass::StreetBased,
                false,
                &mut rng,
            );
            let inc = inc.incidents;
            let collisions: Vec<_> = inc
                .iter()
                .filter(|i| i.incident_type == IncidentType::Collision)
                .collect();
            // Look for a pair where pilot A's linked_pilot_id == pilot B's id and vice versa
            for a in &collisions {
                if let Some(linked) = &a.linked_pilot_id {
                    if let Some(b) = collisions.iter().find(|b| &b.pilot_id == linked) {
                        if a.is_two_car_incident && b.is_two_car_incident {
                            found_bilateral = true;
                            break 'outer;
                        }
                    }
                }
            }
        }

        assert!(
            found_bilateral,
            "should produce bilateral collision with is_two_car_incident=true on both sides"
        );
    }

    #[test]
    fn test_irm_keeps_collision_major_eligible_but_other_non_critical_zero() {
        assert_eq!(
            compute_irm(IncidentType::Collision, IncidentSeverity::Minor),
            0.0
        );
        assert!(compute_irm(IncidentType::Collision, IncidentSeverity::Major) > 0.0);
        assert_eq!(
            compute_irm(IncidentType::DriverError, IncidentSeverity::Minor),
            0.0
        );
        assert_eq!(
            compute_irm(IncidentType::DriverError, IncidentSeverity::Major),
            0.0
        );
        assert_eq!(
            compute_irm(IncidentType::Mechanical, IncidentSeverity::Major),
            0.0
        );
    }

    #[test]
    fn test_narrative_hint_critical_is_2() {
        assert_eq!(
            compute_narrative_hint(IncidentSeverity::Critical, IncidentType::Mechanical),
            2
        );
        assert_eq!(
            compute_narrative_hint(IncidentSeverity::Critical, IncidentType::Collision),
            2
        );
    }

    #[test]
    fn test_narrative_hint_major_collision_is_1() {
        assert_eq!(
            compute_narrative_hint(IncidentSeverity::Major, IncidentType::Collision),
            1
        );
        assert_eq!(
            compute_narrative_hint(IncidentSeverity::Major, IncidentType::Mechanical),
            0
        );
    }

    #[test]
    fn test_high_pack_density_increases_collision_rate() {
        // Pista curta (pack_density=1.4) deve gerar mais colisões que pista longa (pack_density=0.75)
        let drivers: Vec<_> = (1..=12)
            .map(|i| make_driver(&format!("P{i}"), 50, 50, 50, 85.0))
            .collect();
        let states: Vec<_> = (1..=12).map(|i| make_state(&format!("P{i}"), i)).collect();

        let runs = 1000;
        let (mut dense_c, mut sparse_c) = (0, 0);

        let mut rng1 = StdRng::seed_from_u64(42424242);
        let mut rng2 = StdRng::seed_from_u64(42424242);

        for _ in 0..runs {
            let inc = process_segment_incidents(
                &drivers,
                &states,
                RaceSegment::Mid,
                WeatherCondition::Dry,
                false,
                1.0,
                1.0,
                1.40,
                &IncidentCatalog::empty(),
                VehicleClass::StreetBased,
                false,
                &mut rng1,
            );
            let inc = inc.incidents;
            dense_c += inc
                .iter()
                .filter(|i| i.incident_type == IncidentType::Collision)
                .count();

            let inc = process_segment_incidents(
                &drivers,
                &states,
                RaceSegment::Mid,
                WeatherCondition::Dry,
                false,
                1.0,
                1.0,
                0.75,
                &IncidentCatalog::empty(),
                VehicleClass::StreetBased,
                false,
                &mut rng2,
            );
            let inc = inc.incidents;
            sparse_c += inc
                .iter()
                .filter(|i| i.incident_type == IncidentType::Collision)
                .count();
        }

        assert!(
            dense_c > sparse_c,
            "Dense pack (1.4) collisions={} should > sparse (0.75)={}",
            dense_c,
            sparse_c
        );
    }
}
