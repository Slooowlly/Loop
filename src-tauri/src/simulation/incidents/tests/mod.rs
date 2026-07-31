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
        car_performance_quali: 8.0,
        vies_de_pico: 0.0,
        qualidade_de_estrategia: 50.0,
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
        tempo_acumulado_ms: position as f64 * 5_000.0,
        desvio_de_ritmo: 0.0,
        trafego: Default::default(),
        paradas: Default::default(),
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
        let inc = process_segment_incidents_cfg(
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
            true,
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
        let inc = process_segment_incidents_cfg(
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
            true,
            &mut rng,
        );
        let inc = inc.incidents;
        good_mech += inc
            .iter()
            .filter(|i| i.incident_type == IncidentType::Mechanical)
            .count();

        let inc = process_segment_incidents_cfg(
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
            true,
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
        let inc = process_segment_incidents_cfg(
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
            true,
            &mut rng,
        );
        let inc = inc.incidents;
        dry_err += inc
            .iter()
            .filter(|i| i.incident_type == IncidentType::DriverError)
            .count();

        let inc = process_segment_incidents_cfg(
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
            true,
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
        let inc = process_segment_incidents_cfg(
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
            true,
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
    let inc = process_segment_incidents_cfg(
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
        true,
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
        let inc = process_segment_incidents_cfg(
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
            true,
            &mut rng,
        );
        let inc = inc.incidents;
        start_c += inc
            .iter()
            .filter(|i| i.incident_type == IncidentType::Collision)
            .count();

        let inc = process_segment_incidents_cfg(
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
            true,
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
        let inc = process_segment_incidents_cfg(
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
            true,
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
        let inc = process_segment_incidents_cfg(
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
            true,
            &mut rng_normal,
        );
        let inc = inc.incidents;
        normal_c += inc
            .iter()
            .filter(|i| i.incident_type == IncidentType::Collision)
            .count();

        let inc = process_segment_incidents_cfg(
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
            true,
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
        let inc = process_segment_incidents_cfg(
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
            true,
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
        let inc = process_segment_incidents_cfg(
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
            true,
            &mut rng1,
        );
        let inc = inc.incidents;
        dense_c += inc
            .iter()
            .filter(|i| i.incident_type == IncidentType::Collision)
            .count();

        let inc = process_segment_incidents_cfg(
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
            true,
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

/// Invariante que a narrativa depende sem dizer: **erro de pilotagem que abandona
/// nunca é `Minor`**.
///
/// `race_signals::dnf_kind` classifica o DNF só pelo `IncidentType` — um
/// `DriverError` que tira o carro da prova é sempre "batida", sem olhar a
/// severidade. Isso só é honesto porque o motor acopla as duas coisas no erro de
/// pilotagem: o sorteio devolve `Minor ⇒ segue na prova` / `Major ⇒ abandona`, e
/// a escalada por stall sobe a severidade JUNTO com o abandono. Se alguém
/// desacoplar, o boletim passa a chamar de batida um pião leve — e o teste morre
/// aqui, no motor, que é onde a regra mora.
///
/// A colisão fica de fora de propósito: lá severidade e consequência SÃO
/// sorteadas em separado (`roll_collision` × `resolve_collision_consequence`), e
/// um contato `Minor` que quebra a suspensão é caso legítimo. Para a narrativa
/// não muda nada — colisão é batida em qualquer severidade.
#[test]
fn erro_de_pilotagem_com_abandono_nunca_sai_minor() {
    let drivers: Vec<_> = (1..=8)
        .map(|i| make_driver(&format!("P{i}"), 40, 90, 30, 40.0))
        .collect();
    let states: Vec<_> = (1..=8).map(|i| make_state(&format!("P{i}"), i)).collect();
    let mut rng = StdRng::seed_from_u64(2024);

    let mut dnfs = 0;
    for rodada in 0..1500 {
        let segment = match rodada % 5 {
            0 => RaceSegment::Start,
            1 => RaceSegment::Early,
            2 => RaceSegment::Mid,
            3 => RaceSegment::Late,
            _ => RaceSegment::Finish,
        };
        let weather = if rodada % 2 == 0 {
            WeatherCondition::Dry
        } else {
            WeatherCondition::HeavyRain
        };

        let inc = process_segment_incidents_cfg(
            &drivers,
            &states,
            segment,
            weather,
            false,
            8.0, // taxa alta: queremos VOLUME de incidente, não realismo
            2.0,
            1.4,
            &IncidentCatalog::empty(),
            VehicleClass::StreetBased,
            false,
            true,
            &mut rng,
        );

        for i in inc
            .incidents
            .iter()
            .filter(|i| i.is_dnf && i.incident_type == IncidentType::DriverError)
        {
            dnfs += 1;
            assert_ne!(
                i.severity,
                IncidentSeverity::Minor,
                "erro de pilotagem com DNF saiu Minor: quebra a classificação de dnf_kind"
            );
        }
    }

    // Sem isto o teste passaria de graça caso o motor parasse de gerar abandonos.
    assert!(
        dnfs > 50,
        "amostra fraca: só {dnfs} abandonos por erro em 1500 segmentos"
    );
}

/// **A escada de risco de lesão, do mais grave ao mais banal.** O que este teste guarda não é
/// nenhum número em particular — é a ORDEM entre eles, que já se perdeu uma vez.
///
/// Três call sites montam a `IncidentResult` na mão em vez de passar por `compute_irm`
/// (`race::motor::empurrar_contato` e os dois desfechos de `race::danos`). Eles cravavam
/// multiplicadores altos e soltos: um encostão de disputa valia 50% de lesão, mais que uma
/// batida crítica; andar avariado perdendo posições valia 25%, mais que uma pane crítica. Numa
/// carreira de 27 temporadas isso deu 11,7% das largadas terminando com piloto machucado.
///
/// A regra é a intuição da corrida: bater machuca mais que quebrar, e quebrar machuca mais que
/// seguir andando torto.
#[test]
fn a_ordem_do_risco_de_lesao_e_a_da_corrida() {
    let chance = |tipo: IncidentType, irm: f64| injury_base_chance(tipo) * irm;

    let batida_critica = chance(
        IncidentType::Collision,
        compute_irm(IncidentType::Collision, IncidentSeverity::Critical),
    )
    .min(0.70); // o teto de `generate_injury_from_incident`
    let erro_critico = chance(
        IncidentType::DriverError,
        compute_irm(IncidentType::DriverError, IncidentSeverity::Critical),
    );
    let contato = chance(IncidentType::Collision, IRM_CONTATO_DE_DISPUTA);
    let pane_critica = chance(
        IncidentType::Mechanical,
        compute_irm(IncidentType::Mechanical, IncidentSeverity::Critical),
    );
    let avariado_abandona = chance(IncidentType::Mechanical, IRM_DANO_LATENTE_COM_ABANDONO);
    let avariado_segue = chance(IncidentType::Mechanical, IRM_DANO_LATENTE_SEM_ABANDONO);

    assert!(
        batida_critica > erro_critico,
        "batida crítica ({batida_critica}) tem de machucar mais que erro crítico ({erro_critico})"
    );
    assert!(
        erro_critico > contato,
        "erro crítico ({erro_critico}) tem de machucar mais que um encostão ({contato})"
    );
    assert!(
        contato > pane_critica,
        "bater ({contato}) tem de machucar mais que quebrar ({pane_critica})"
    );
    // Recolher um carro avariado não é impacto novo: vale o mesmo que a pane crítica.
    assert!(
        (avariado_abandona - pane_critica).abs() < 1e-9,
        "abandonar avariado ({avariado_abandona}) devia valer a pane crítica ({pane_critica})"
    );
    assert!(
        avariado_abandona > avariado_segue,
        "abandonar ({avariado_abandona}) tem de machucar mais que seguir torto ({avariado_segue})"
    );
    assert!(
        avariado_segue > 0.0,
        "seguir avariado ainda pode machucar — zerar isto DESLIGA o risco, não o calibra"
    );
}

/// **Carro avariado normalmente segue andando mal; abandonar é a exceção.** Era o contrário:
/// `0.70` fazia do abandono o desfecho de 7 em cada 10 manifestações, e o jogador via o grid
/// esvaziar por dano latente com mais frequência do que via alguém perder posições por ele.
#[test]
fn dano_latente_custa_posicoes_mais_do_que_tira_o_carro() {
    assert!(
        CHANCE_DE_ABANDONO_NA_MANIFESTACAO < 0.5,
        "perder posições ({}) tem de ser mais comum que abandonar ({CHANCE_DE_ABANDONO_NA_MANIFESTACAO})",
        1.0 - CHANCE_DE_ABANDONO_NA_MANIFESTACAO
    );
}
