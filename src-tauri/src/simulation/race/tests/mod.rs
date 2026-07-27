//! Testes da corrida simulada: grid, degradação, ordenação de abandono e a quebra de peça da Fase 7.

use rand::{rngs::StdRng, SeedableRng};

use crate::models::driver::Driver;
use crate::models::enums::WeatherCondition;
use crate::models::team::placeholder_team_from_db;
use crate::simulation::context::SimulationContext;
use crate::simulation::track_profile::TrackCharacter;

use super::*;
use crate::simulation::catalog::IncidentCatalog;
use crate::simulation::context::SimDriver;
use crate::simulation::incidents::IncidentType;
use crate::simulation::qualifying::simulate_qualifying;

fn sample_context(duration: i32, weather: WeatherCondition) -> SimulationContext {
    SimulationContext {
        weather,
        race_duration_minutes: duration,
        ..SimulationContext::test_default()
    }
}

fn sample_context_with_incidents(duration: i32, weather: WeatherCondition) -> SimulationContext {
    SimulationContext {
        incidents_enabled: true,
        weather,
        race_duration_minutes: duration,
        ..SimulationContext::test_default()
    }
}

fn build_driver(
    id: &str,
    skill: f64,
    racecraft: f64,
    pneus: f64,
    fitness: f64,
    car: f64,
) -> SimDriver {
    let mut driver = Driver::create_player(
        id.to_string(),
        format!("Driver {}", id),
        "🇧🇷 Brasileiro".to_string(),
        20,
    );
    driver.is_jogador = false;
    driver.atributos.skill = skill;
    driver.atributos.consistencia = 88.0;
    driver.atributos.racecraft = racecraft;
    driver.atributos.ritmo_classificacao = skill;
    driver.atributos.habilidade_largada = 70.0;
    driver.atributos.gestao_pneus = pneus;
    driver.atributos.fitness = fitness;
    driver.atributos.mentalidade = 72.0;
    driver.atributos.confianca = 70.0;
    driver.atributos.adaptabilidade = 68.0;
    driver.atributos.fator_chuva = 50.0;

    let mut team = placeholder_team_from_db(
        format!("T{}", id),
        format!("Team {}", id),
        "gt4".to_string(),
        "2026-01-01T00:00:00".to_string(),
    );
    team.car_performance = car;
    team.confiabilidade = 80.0;

    SimDriver::from_driver_and_team(&driver, &team)
}

fn build_grid() -> Vec<SimDriver> {
    (0..12)
        .map(|index| {
            build_driver(
                &format!("{:03}", index + 1),
                60.0 + index as f64,
                60.0,
                65.0,
                70.0,
                8.0,
            )
        })
        .collect()
}

#[test]
fn test_race_returns_all_drivers() {
    let grid = build_grid();
    let mut rng = StdRng::seed_from_u64(21);
    let ctx = sample_context(30, WeatherCondition::Dry);
    let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
    let result = simulate_race_with_breakdowns(
        &grid,
        &qualifying,
        &ctx,
        &IncidentCatalog::empty(),
        false,
        None,
        &mut rng,
    );

    assert_eq!(result.race_results.len(), 12);
}

// O casamento shape↔pista (carro de X anda melhor em pista de X) é testado
// deterministicamente em `car_build::track_delta_from_shape` e em
// `context::test_carro_de_potencia_rende_mais_na_pista_de_power`. O antigo teste de
// corrida completa por perfil discreto foi aposentado com o `CarBuildProfile`.

#[test]
fn test_race_positions_sequential() {
    let grid = build_grid();
    let mut rng = StdRng::seed_from_u64(22);
    let ctx = sample_context(30, WeatherCondition::Dry);
    let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
    let result = simulate_race_with_breakdowns(
        &grid,
        &qualifying,
        &ctx,
        &IncidentCatalog::empty(),
        false,
        None,
        &mut rng,
    );
    assert_eq!(
        result
            .race_results
            .iter()
            .map(|value| value.finish_position)
            .collect::<Vec<_>>(),
        (1..=12).collect::<Vec<_>>()
    );
}

#[test]
fn test_race_tire_degradation() {
    let grid = build_grid();
    let mut rng = StdRng::seed_from_u64(23);
    let ctx = sample_context(45, WeatherCondition::Dry);
    let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
    let result = simulate_race_with_breakdowns(
        &grid,
        &qualifying,
        &ctx,
        &IncidentCatalog::empty(),
        false,
        None,
        &mut rng,
    );

    assert!(result
        .race_results
        .iter()
        .all(|driver| driver.final_tire_wear < 1.0));
}

#[test]
fn test_race_physical_degradation() {
    let grid = build_grid();
    let mut rng = StdRng::seed_from_u64(24);
    let ctx = sample_context(45, WeatherCondition::Dry);
    let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
    let result = simulate_race_with_breakdowns(
        &grid,
        &qualifying,
        &ctx,
        &IncidentCatalog::empty(),
        false,
        None,
        &mut rng,
    );

    assert!(result
        .race_results
        .iter()
        .all(|driver| driver.final_physical < 1.0));
}

#[test]
fn test_race_positions_gained_calculated() {
    let grid = build_grid();
    let mut rng = StdRng::seed_from_u64(25);
    let ctx = sample_context(30, WeatherCondition::Dry);
    let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
    let result = simulate_race_with_breakdowns(
        &grid,
        &qualifying,
        &ctx,
        &IncidentCatalog::empty(),
        false,
        None,
        &mut rng,
    );

    assert!(result.race_results.iter().all(|driver| {
        driver.positions_gained == driver.grid_position - driver.finish_position
    }));
}

#[test]
fn test_race_good_driver_tends_to_win() {
    let ace = build_driver("ACE", 95.0, 90.0, 85.0, 88.0, 15.0);
    let grid: Vec<SimDriver> = std::iter::once(ace.clone())
        .chain((0..11).map(|index| build_driver(&format!("R{index}"), 60.0, 60.0, 60.0, 60.0, 6.0)))
        .collect();

    let mut wins = 0;
    for seed in 0..50 {
        let mut rng = StdRng::seed_from_u64(seed);
        let ctx = sample_context(35, WeatherCondition::Dry);
        let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
        let result = simulate_race_with_breakdowns(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            None,
            &mut rng,
        );
        if result.winner_id == ace.id {
            wins += 1;
        }
    }

    assert!(wins >= 35, "ace driver only won {} times", wins);
}

#[test]
fn test_pilar_a_car_decides_at_top_not_rookie() {
    // Bom piloto / carro ruim  vs  piloto ruim / carro excelente.
    let good_driver_bad_car = build_driver("SKILL", 85.0, 80.0, 75.0, 78.0, 0.0);
    let bad_driver_top_car = build_driver("CAR", 65.0, 65.0, 65.0, 65.0, 16.0);
    let grid = vec![good_driver_bad_car.clone(), bad_driver_top_car.clone()];

    fn car_wins(grid: &[SimDriver], category: &str, car_id: &str) -> i32 {
        let mut wins = 0;
        for seed in 0..50 {
            let mut rng = StdRng::seed_from_u64(seed);
            let ctx = SimulationContext {
                category_id: category.to_string(),
                ..sample_context(35, WeatherCondition::Dry)
            };
            let q = simulate_qualifying(grid, &ctx, &mut rng);
            let r = simulate_race_with_breakdowns(
                grid,
                &q,
                &ctx,
                &IncidentCatalog::empty(),
                false,
                None,
                &mut rng,
            );
            if r.winner_id == car_id {
                wins += 1;
            }
        }
        wins
    }

    let rookie_car_wins = car_wins(&grid, "mazda_rookie", &bad_driver_top_car.id);
    let endurance_car_wins = car_wins(&grid, "endurance", &bad_driver_top_car.id);

    // Rookie: carro spec (idêntico) + peso baixo -> o piloto ruim quase nunca
    // vence só pelo carro; o talento decide.
    assert!(
        rookie_car_wins <= 12,
        "rookie: carro nao deveria decidir, mas venceu {rookie_car_wins}/50"
    );
    // Endurance: peso do carro alto -> o carro topo transforma o piloto ruim em
    // vencedor frequente.
    assert!(
        endurance_car_wins >= 35,
        "endurance: carro deveria dominar, mas venceu so {endurance_car_wins}/50"
    );
    // E o impacto do carro deve ser muito maior no topo do que no rookie.
    assert!(
        endurance_car_wins > rookie_car_wins + 20,
        "impacto do carro deveria ser muito maior no topo (rookie={rookie_car_wins}, endurance={endurance_car_wins})"
    );
}

#[test]
fn test_race_bad_tires_hurt_late_segments() {
    let tire_saver = build_driver("SAVE", 78.0, 72.0, 92.0, 75.0, 10.0);
    let tire_abuser = build_driver("ABUSE", 78.0, 72.0, 25.0, 75.0, 10.0);
    let grid = vec![tire_saver.clone(), tire_abuser.clone()];

    let mut saver_better = 0;
    for seed in 0..30 {
        let mut rng = StdRng::seed_from_u64(seed);
        let ctx = sample_context(60, WeatherCondition::Dry);
        let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
        let result = simulate_race_with_breakdowns(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            None,
            &mut rng,
        );
        if result.winner_id == tire_saver.id {
            saver_better += 1;
        }
    }

    assert!(saver_better >= 20);
}

#[test]
fn test_rookie_category_experience_penalizes_race_score() {
    let mut rookie = build_driver("ROOKIE", 80.0, 78.0, 75.0, 76.0, 10.0);
    rookie.corridas_na_categoria = 2;

    let mut veteran = rookie.clone();
    veteran.id = "VETERAN".to_string();
    veteran.nome = "Driver VETERAN".to_string();
    veteran.corridas_na_categoria = 18;

    let ctx = SimulationContext {
        race_variance_multiplier: 0.0,
        ..sample_context(30, WeatherCondition::Dry)
    };
    let state = RaceState {
        driver_id: rookie.id.clone(),
        tire_wear: 1.0,
        physical_condition: 1.0,
        cumulative_score: 0.0,
        is_dnf: false,
        current_position: 1,
        incidents: Vec::new(),
        dnf_reason: None,
        dnf_segment: None,
        pending_damage: Vec::new(),
    };

    let mut rookie_rng = StdRng::seed_from_u64(101);
    let rookie_score =
        calculate_segment_score(&rookie, &state, RaceSegment::Mid, &ctx, &mut rookie_rng);

    let mut veteran_rng = StdRng::seed_from_u64(101);
    let veteran_score =
        calculate_segment_score(&veteran, &state, RaceSegment::Mid, &ctx, &mut veteran_rng);

    assert!(
        rookie_score < veteran_score,
        "rookie_score={rookie_score} should be lower than veteran_score={veteran_score}"
    );
}

#[test]
fn test_smoothness_reduces_tire_degradation() {
    let ctx = sample_context(45, WeatherCondition::Dry);
    let mut smooth = build_driver("SMOOTH", 75.0, 72.0, 70.0, 74.0, 10.0);
    smooth.smoothness = 92;

    let mut rough = smooth.clone();
    rough.id = "ROUGH".to_string();
    rough.nome = "Driver ROUGH".to_string();
    rough.smoothness = 18;

    let mut smooth_state = RaceState {
        driver_id: smooth.id.clone(),
        tire_wear: 1.0,
        physical_condition: 1.0,
        cumulative_score: 0.0,
        is_dnf: false,
        current_position: 1,
        incidents: Vec::new(),
        dnf_reason: None,
        dnf_segment: None,
        pending_damage: Vec::new(),
    };
    let mut rough_state = smooth_state.clone();
    rough_state.driver_id = rough.id.clone();

    apply_tire_degradation(&mut smooth_state, &smooth, &ctx);
    apply_tire_degradation(&mut rough_state, &rough, &ctx);

    assert!(
        smooth_state.tire_wear > rough_state.tire_wear,
        "smooth tire_wear={} should be greater than rough tire_wear={}",
        smooth_state.tire_wear,
        rough_state.tire_wear
    );
}

#[test]
fn test_incidents_can_generate_dnfs_when_enabled() {
    let mut risky = build_driver("RISK", 65.0, 30.0, 50.0, 55.0, 5.0);
    risky.consistencia = 20;
    risky.aggression = 95;
    risky.experiencia = 10;
    risky.car_reliability = 20.0;

    let grid = vec![risky.clone()];
    let mut found_dnf = false;

    for seed in 0..300 {
        let mut rng = StdRng::seed_from_u64(seed);
        let ctx = sample_context_with_incidents(50, WeatherCondition::HeavyRain);
        let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
        let result = simulate_race_with_breakdowns(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            None,
            &mut rng,
        );

        if result.race_results.iter().any(|driver| driver.is_dnf) {
            found_dnf = true;
            break;
        }
    }

    assert!(
        found_dnf,
        "expected at least one DNF with incidents enabled"
    );
}

#[test]
fn test_race_result_tracks_total_incidents() {
    let mut risky = build_driver("RISK", 65.0, 30.0, 50.0, 55.0, 5.0);
    risky.consistencia = 25;
    risky.aggression = 90;
    risky.experiencia = 15;
    risky.car_reliability = 25.0;

    let grid = vec![risky];
    let mut rng = StdRng::seed_from_u64(999);
    let ctx = sample_context_with_incidents(45, WeatherCondition::Wet);
    let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
    let result = simulate_race_with_breakdowns(
        &grid,
        &qualifying,
        &ctx,
        &IncidentCatalog::empty(),
        false,
        None,
        &mut rng,
    );

    let sum: i32 = result.race_results.iter().map(|d| d.incidents_count).sum();
    assert_eq!(result.total_incidents, sum);
}

#[test]
fn test_dnf_ordering_later_segment_ahead_of_earlier() {
    let driver_a = build_driver("A", 70.0, 70.0, 70.0, 70.0, 8.0);
    let driver_b = build_driver("B", 70.0, 70.0, 70.0, 70.0, 8.0);

    // Simular manualmente: A abandona no Late, B abandona no Early
    let state_a = RaceState {
        driver_id: "A".to_string(),
        tire_wear: 0.6,
        physical_condition: 0.8,
        cumulative_score: 200.0,
        is_dnf: true,
        current_position: 1,
        incidents: Vec::new(),
        dnf_reason: Some("Engine".to_string()),
        dnf_segment: Some(RaceSegment::Late),
        pending_damage: Vec::new(),
    };
    let state_b = RaceState {
        driver_id: "B".to_string(),
        tire_wear: 0.9,
        physical_condition: 0.95,
        cumulative_score: 50.0,
        is_dnf: true,
        current_position: 2,
        incidents: Vec::new(),
        dnf_reason: Some("Crash".to_string()),
        dnf_segment: Some(RaceSegment::Early),
        pending_damage: Vec::new(),
    };

    // A (Late DNF) deve ter laps_completed > B (Early DNF)
    let laps_a = estimate_laps_at_dnf(state_a.dnf_segment, 20);
    let laps_b = estimate_laps_at_dnf(state_b.dnf_segment, 20);
    assert!(
        laps_a > laps_b,
        "Late DNF laps={laps_a} should > Early DNF laps={laps_b}"
    );

    // Na ordenação de DNFs, A (Late) deve vir antes de B (Early)
    let mut dnfs = vec![&state_a, &state_b];
    dnfs.sort_by(|a, b| {
        let seg_ord_b = b.dnf_segment.map(|s| s.ordinal()).unwrap_or(0);
        let seg_ord_a = a.dnf_segment.map(|s| s.ordinal()).unwrap_or(0);
        seg_ord_b.cmp(&seg_ord_a).then_with(|| {
            b.cumulative_score
                .partial_cmp(&a.cumulative_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    assert_eq!(
        dnfs[0].driver_id, "A",
        "Late DNF driver should rank ahead of Early DNF"
    );

    let _ = (driver_a, driver_b); // suppress unused warnings
}

#[test]
fn test_dnf_gap_never_negative() {
    let mut risky = build_driver("RISK", 50.0, 30.0, 50.0, 50.0, 5.0);
    risky.consistencia = 15;
    risky.aggression = 95;
    risky.car_reliability = 15.0;

    let grid: Vec<SimDriver> = std::iter::once(risky)
        .chain((0..11).map(|i| build_driver(&format!("R{i}"), 70.0, 70.0, 70.0, 70.0, 8.0)))
        .collect();

    for seed in 0..50 {
        let mut rng = StdRng::seed_from_u64(seed);
        let ctx = sample_context_with_incidents(40, WeatherCondition::Wet);
        let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
        let result = simulate_race_with_breakdowns(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            None,
            &mut rng,
        );

        for r in &result.race_results {
            assert!(
                r.gap_to_winner_ms >= 0.0,
                "gap_to_winner_ms={} must be >= 0 for driver {}",
                r.gap_to_winner_ms,
                r.pilot_id
            );
        }
    }
}

#[test]
fn test_dnf_laps_completed_coherence() {
    let laps_start = estimate_laps_at_dnf(Some(RaceSegment::Start), 30);
    let laps_early = estimate_laps_at_dnf(Some(RaceSegment::Early), 30);
    let laps_mid = estimate_laps_at_dnf(Some(RaceSegment::Mid), 30);
    let laps_late = estimate_laps_at_dnf(Some(RaceSegment::Late), 30);
    let laps_finish = estimate_laps_at_dnf(Some(RaceSegment::Finish), 30);

    assert!(laps_start < laps_early);
    assert!(laps_early < laps_mid);
    assert!(laps_mid < laps_late);
    assert!(laps_late < laps_finish);
    assert!(laps_finish < 30);
}

#[test]
fn test_endurance_more_tire_degradation_than_sprint() {
    use crate::simulation::profile::resolve_simulation_profile;

    let endurance_profile =
        resolve_simulation_profile("endurance", 288, 25.0, WeatherCondition::Dry, 0, 10);
    let gt4_profile = resolve_simulation_profile("gt4", 47, 25.0, WeatherCondition::Dry, 30, 12);

    assert!(
        endurance_profile.tire_degradation_rate > gt4_profile.tire_degradation_rate,
        "endurance tire_degr={} should > gt4={}",
        endurance_profile.tire_degradation_rate,
        gt4_profile.tire_degradation_rate
    );
}

// ───────────────── Quebra de peça na corrida simulada (Fase 7) ─────────────────

fn mech(pilot: &str, lap: u32, is_dnf: bool, secs: u32) -> MechanicalOutcome {
    MechanicalOutcome {
        pilot_id: pilot.to_string(),
        lap,
        is_dnf,
        penalty_secs: secs,
        label: "câmbio travou na 3ª".to_string(),
    }
}

fn race_with(mechanicals: &[MechanicalOutcome], seed: u64) -> (RaceResult, Vec<SimDriver>) {
    let grid = build_grid();
    let mut rng = StdRng::seed_from_u64(seed);
    let ctx = sample_context(30, WeatherCondition::Dry);
    let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
    let result = simulate_race_with_breakdowns(
        &grid,
        &qualifying,
        &ctx,
        &IncidentCatalog::empty(),
        false,
        Some(mechanicals),
        &mut rng,
    );
    (result, grid)
}

#[test]
fn volta_cai_no_segmento_certo() {
    // 20 voltas → 4 por segmento. Volta 1 é largada; a última é FINISH.
    assert_eq!(RaceSegment::from_lap(1, 20), RaceSegment::Start);
    assert_eq!(RaceSegment::from_lap(4, 20), RaceSegment::Start);
    assert_eq!(RaceSegment::from_lap(5, 20), RaceSegment::Early);
    assert_eq!(RaceSegment::from_lap(12, 20), RaceSegment::Mid);
    assert_eq!(RaceSegment::from_lap(17, 20), RaceSegment::Finish);
    assert_eq!(RaceSegment::from_lap(20, 20), RaceSegment::Finish);
    // Volta além do fim (corrida encurtada por bandeira) não estoura o índice.
    assert_eq!(RaceSegment::from_lap(99, 20), RaceSegment::Finish);
    assert_eq!(RaceSegment::from_lap(1, 0), RaceSegment::Start);
}

#[test]
fn com_a_quebra_desligada_nenhuma_pane_e_cobrada() {
    // `None` = a Fase 7 não roda nesta corrida (rascunho histórico, grid sintético). Incidentes
    // LIGADOS de propósito: a pane do catálogo continua sendo a fonte de falha mecânica, mas
    // nada entra em `applied_mechanicals`.
    let grid = build_grid();
    let ctx = sample_context_with_incidents(30, WeatherCondition::Dry);

    let mut rng = StdRng::seed_from_u64(77);
    let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
    let result = simulate_race_with_breakdowns(
        &grid,
        &qualifying,
        &ctx,
        &IncidentCatalog::empty(),
        false,
        None,
        &mut rng,
    );

    assert!(result.applied_mechanicals.is_empty());
    assert_eq!(result.race_results.len(), grid.len());
}

#[test]
fn quebra_com_dnf_tira_o_carro_e_registra_a_peca_como_motivo() {
    let (result, _) = race_with(&[mech("003", 6, true, 0)], 31);
    let entry = result
        .race_results
        .iter()
        .find(|r| r.pilot_id == "003")
        .expect("piloto no resultado");
    assert!(
        entry.is_dnf,
        "a quebra deveria ter encerrado a corrida dele"
    );
    assert_eq!(entry.dnf_reason.as_deref(), Some("câmbio travou na 3ª"));
    assert_eq!(result.applied_mechanicals, vec![0]);
}

#[test]
fn reparo_custa_exatamente_os_segundos_perdidos() {
    // O contrato do `repair_secs_to_score`: 15s no box = 15s a mais no tempo de corrida.
    // Medido num carro do MEIO do grid — no líder a âncora do tempo se move junto e o
    // custo aparente encolhe pela margem que ele tinha (ver o doc da função).
    const SECS: u32 = 15;
    const ALVO: &str = "006";
    let (limpo, _) = race_with(&[], 99);
    assert_ne!(
        limpo.winner_id, ALVO,
        "o alvo do teste não pode ser o líder"
    );
    let (penalizado, _) = race_with(&[mech(ALVO, 6, false, SECS)], 99);
    assert_eq!(
        penalizado.winner_id, limpo.winner_id,
        "o líder não pode mudar"
    );

    let antes = limpo
        .race_results
        .iter()
        .find(|r| r.pilot_id == ALVO)
        .unwrap()
        .total_race_time_ms;
    let depois = penalizado
        .race_results
        .iter()
        .find(|r| r.pilot_id == ALVO)
        .unwrap()
        .total_race_time_ms;

    let delta_s = (depois - antes) / 1000.0;
    assert!(
        (delta_s - SECS as f64).abs() < 0.5,
        "esperava ~{SECS}s a mais, veio {delta_s:.2}s (antes {antes:.0}ms, depois {depois:.0}ms)"
    );
    assert_eq!(penalizado.applied_mechanicals, vec![0]);
}

#[test]
fn reparo_pesado_custa_posicao() {
    // 25s num sprint de 12 voltas tem que doer na classificação — se não doesse, a Fase 7
    // seria decorativa.
    let (limpo, _) = race_with(&[], 5);
    let (penalizado, _) = race_with(&[mech("012", 4, false, 25)], 5);
    let pos = |r: &RaceResult| {
        r.race_results
            .iter()
            .find(|e| e.pilot_id == "012")
            .unwrap()
            .finish_position
    };
    assert!(
        pos(&penalizado) > pos(&limpo),
        "P{} → P{} — o reparo não custou posição nenhuma",
        pos(&limpo),
        pos(&penalizado)
    );
}

#[test]
fn carro_ja_fora_por_batida_nao_registra_a_quebra() {
    // Duas quebras no MESMO piloto: a primeira (volta 3, DNF) o tira; a segunda (volta 10)
    // não pode ser cobrada nem registrada — a peça largaria num carro que não está mais lá.
    let (result, _) = race_with(&[mech("007", 3, true, 0), mech("007", 10, false, 12)], 13);
    assert_eq!(
        result.applied_mechanicals,
        vec![0],
        "só a quebra que aconteceu de verdade pode ser registrada"
    );
}

/// Roda MUITAS corridas com incidentes ligados e conta os abandonos por pane mecânica do
/// catálogo, com a quebra ligada (`Some`) e desligada (`None`).
fn panes_do_catalogo(mechanicals: Option<&[MechanicalOutcome]>, corridas: u64) -> usize {
    let grid = build_grid();
    let ctx = sample_context_with_incidents(30, WeatherCondition::Dry);
    let mut total = 0;
    for seed in 0..corridas {
        let mut rng = StdRng::seed_from_u64(seed);
        let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
        let result = simulate_race_with_breakdowns(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            mechanicals,
            &mut rng,
        );
        total += result
            .race_results
            .iter()
            .flat_map(|r| &r.incidents)
            .filter(|i| i.incident_type == IncidentType::Mechanical)
            .count();
    }
    total
}

#[test]
fn quebra_ligada_desliga_a_pane_generica_do_catalogo() {
    // FONTE ÚNICA: a pane do catálogo sorteia sobre a `confiabilidade` abstrata da equipe e
    // não nomeia peça nem danifica nada. Onde o Sistema de Quebra roda, ela some — senão a
    // taxa de abandono mecânico dobraria e carro de peça nova fundiria motor sem aviso.
    const CORRIDAS: u64 = 400;
    let com_quebra = panes_do_catalogo(Some(&[]), CORRIDAS);
    let sem_quebra = panes_do_catalogo(None, CORRIDAS);

    assert_eq!(
        com_quebra, 0,
        "com a quebra no comando o catálogo não pode gerar pane nenhuma"
    );
    assert!(
        sem_quebra > 0,
        "sem a quebra o catálogo TEM que continuar sendo a fonte de pane (veio {sem_quebra} \
         em {CORRIDAS} corridas) — se zerou, o teste não está medindo nada"
    );
}

#[test]
fn quebra_vale_mesmo_com_incidentes_desligados() {
    // `incidents_enabled: false` é o default do contexto de teste. A quebra não é incidente
    // de pilotagem: é o desgaste que o time trouxe, e tem que valer do mesmo jeito.
    let ctx = sample_context(30, WeatherCondition::Dry);
    assert!(!ctx.incidents_enabled);
    let (result, _) = race_with(&[mech("005", 8, true, 0)], 44);
    assert!(
        result
            .race_results
            .iter()
            .find(|r| r.pilot_id == "005")
            .unwrap()
            .is_dnf
    );
}
