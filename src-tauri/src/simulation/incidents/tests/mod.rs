//! Testes dos incidentes: frequência por perfil de piloto, peso do segmento e da chuva, e a bilateralidade da colisão.

use std::collections::HashSet;

use rand::{rngs::StdRng, SeedableRng};

use super::*;
use crate::models::enums::WeatherCondition;
use crate::simulation::catalog::{IncidentCatalog, VehicleClass};
use crate::simulation::context::SimDriver;
use crate::simulation::race::{RaceSegment, RaceState};

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
        motivacao: 70.0,
        car_performance: 8.0,
        car_reliability: reliability,
        team_id: format!("T{id}"),
        team_name: format!("Team {id}"),
        corridas_na_categoria: 10,
        pressure_error_mult: 1.0,
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
