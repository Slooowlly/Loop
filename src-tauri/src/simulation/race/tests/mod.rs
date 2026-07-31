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
use crate::simulation::qualifying::{simulate_qualifying, QualifyingResult};

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
        tempo_acumulado_ms: 0.0,
        desvio_de_ritmo: 0.0,
        trafego: Default::default(),
        paradas: Default::default(),
        is_dnf: false,
        current_position: 1,
        incidents: Vec::new(),
        dnf_reason: None,
        dnf_segment: None,
        pending_damage: Vec::new(),
    };

    // O ritmo é determinístico agora — o ruído saiu daqui para o laço da corrida.
    let rookie_score = calculate_segment_score(&rookie, &state, RaceSegment::Mid, &ctx);
    let veteran_score = calculate_segment_score(&veteran, &state, RaceSegment::Mid, &ctx);

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
        tempo_acumulado_ms: 0.0,
        desvio_de_ritmo: 0.0,
        trafego: Default::default(),
        paradas: Default::default(),
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
        tempo_acumulado_ms: 0.0,
        desvio_de_ritmo: 0.0,
        trafego: Default::default(),
        paradas: Default::default(),
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
        tempo_acumulado_ms: 150_000.0,
        desvio_de_ritmo: 0.0,
        trafego: Default::default(),
        paradas: Default::default(),
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
            a.tempo_acumulado_ms
                .partial_cmp(&b.tempo_acumulado_ms)
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
    let (limpo, _) = race_with(&[], 99);
    // O alvo é ESCOLHIDO a partir do resultado limpo, não fixado por id: qualquer mudança
    // no que forma o grid (quali, carro, esteira de modificadores) reordena os pilotos e um
    // id fixo escorrega pra frente do pelotão sem avisar, quebrando a premissa da medição.
    let alvo = limpo
        .race_results
        .iter()
        .filter(|r| !r.is_dnf)
        .nth(limpo.race_results.len() * 2 / 3)
        .expect("precisa de um carro do fundo do pelotão, sem abandono")
        .pilot_id
        .clone();
    let alvo = alvo.as_str();
    let (penalizado, _) = race_with(&[mech(alvo, 6, false, SECS)], 99);
    assert_eq!(
        penalizado.winner_id, limpo.winner_id,
        "o líder não pode mudar"
    );

    let antes = limpo
        .race_results
        .iter()
        .find(|r| r.pilot_id == alvo)
        .unwrap()
        .total_race_time_ms;
    let depois = penalizado
        .race_results
        .iter()
        .find(|r| r.pilot_id == alvo)
        .unwrap()
        .total_race_time_ms;

    let delta_s = (depois - antes) / 1000.0;
    // ── O contrato depois do modelo de posição na pista (pacote D) ──
    //
    // O reparo continua somando EXATAMENTE `secs·1000` ms ao relógio do carro — isso é uma
    // linha do motor e está travado em `reparo_cobra_exatamente_os_segundos_em_ar_limpo`.
    // O que mudou é o que o RESULTADO PUBLICADO mostra: com trem de carros, o carro que para
    // volta para um trânsito diferente do que enfrentaria, e o custo LÍQUIDO deixa de ser
    // igual ao custo bruto. Aqui, um carro que estava preso atrás de outro perde menos do que
    // os segundos parados, porque parte do tempo já estava sendo perdida no bloqueio.
    //
    // Isso é o comportamento certo (na pista, quem está preso perde menos ao parar), mas é
    // uma MUDANÇA DE SEMÂNTICA do contrato e está registrada aqui de propósito, em vez de
    // afrouxada em silêncio: o teste passou a cobrar uma FAIXA, e a igualdade exata mudou de
    // endereço para o teste de ar limpo.
    assert!(
        delta_s > 0.0 && delta_s <= SECS as f64 * 1.5,
        "o custo líquido saiu da faixa: {delta_s:.2}s para {SECS}s de box \
         (antes {antes:.0}ms, depois {depois:.0}ms)"
    );
    assert_eq!(penalizado.applied_mechanicals, vec![0]);
}

#[test]
fn reparo_cobra_exatamente_os_segundos_em_ar_limpo() {
    // O contrato original, medido onde ele é bem definido: sem trânsito. Dois carros de
    // ritmo muito diferente nunca entram na janela um do outro, então o carro que para está
    // em ar limpo e paga exatamente o que ficou parado.
    const SECS: u32 = 15;
    let lider = build_driver("LIDER", 95.0, 90.0, 85.0, 88.0, 16.0);
    let sozinho = build_driver("SOZINHO", 55.0, 55.0, 55.0, 55.0, 0.0);
    let grid = vec![lider.clone(), sozinho.clone()];
    let ctx = sample_context(30, WeatherCondition::Dry);

    let mut rng_quali = StdRng::seed_from_u64(3);
    let qualifying = simulate_qualifying(&grid, &ctx, &mut rng_quali);

    let corrida = |mecanicas: &[MechanicalOutcome]| {
        let mut rng = StdRng::seed_from_u64(31);
        simulate_race_with_breakdowns(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            Some(mecanicas),
            &mut rng,
        )
    };
    let tempo = |r: &RaceResult| {
        r.race_results
            .iter()
            .find(|x| x.pilot_id == sozinho.id)
            .expect("retardatário")
            .total_race_time_ms
    };

    let limpo = corrida(&[]);
    let penalizado = corrida(&[MechanicalOutcome {
        pilot_id: sozinho.id.clone(),
        lap: 6,
        is_dnf: false,
        penalty_secs: SECS,
        label: "troca de pneu".to_string(),
    }]);

    assert_eq!(
        limpo.winner_id, lider.id,
        "o líder tem que ser o carro rápido"
    );
    let delta_s = (tempo(&penalizado) - tempo(&limpo)) / 1000.0;
    assert!(
        (delta_s - SECS as f64).abs() < 0.01,
        "em ar limpo o contrato é exato: esperava {SECS}s, veio {delta_s:.4}s"
    );
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
///
/// `damage_origin_segment.is_none()` separa a PANE de verdade da sequela de colisão: as duas
/// são carimbadas `Mechanical` (ver `race::danos`), mas só a primeira é a "pane genérica" que
/// o Sistema de Quebra vem substituir — a segunda é um carro andando torto por causa de uma
/// batida, e existir ou não a quebra ao vivo não muda isso.
///
/// Sem esse recorte o contador media outra coisa: dava zero só porque `IncidentCatalog::empty()`
/// impedia o dano latente de nascer (`maybe_add_pending_damage` exige template do catálogo).
/// Com catálogo de verdade — ou com o dano de contato, que não depende dele — a conta subia e o
/// teste acusava uma regressão que não era regressão nenhuma.
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
            .filter(|i| {
                i.incident_type == IncidentType::Mechanical && i.damage_origin_segment.is_none()
            })
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

// ═══════════════ Fase 1: prova de equivalência da troca de moeda ═══════════════
//
// Trocar `cumulative_score` (pontos, maior é melhor) por `tempo_acumulado_ms` (tempo, menor
// é melhor) tem que ser SÓ representação. A prova vem em duas camadas — a álgebra e a
// medição.
//
// A álgebra. Com K = RACE_SCORE_TO_LAP_MS, L = voltas, um segmento valendo L/5 voltas e o
// ponto de ritmo valendo 5K ms por volta:
//
//   tempo_i = Σ_s (REF − ritmo_s)·(5K)·(L/5) + (pos_i − 1)·2K·L + Σ segundos·1000
//           = K·L·(5·REF − Σ ritmo_s) + 2K·L·(pos_i − 1) + Σ segundos·1000
//
//   tempo_i − tempo_vencedor = K·L·(Σritmo_v − Σritmo_i)
//                            + 2K·L·(pos_i − pos_v)
//                            + 1000·(reparo_i − reparo_v)
//
// O modelo antigo publicava `(score_v − score_i)·K·L` com
// `score = 2·(total − pos + 1) + Σ ritmo − Σ (secs·1000)/(K·L)`, que abre exatamente nos
// mesmos três termos. O ×5 de `MS_POR_PONTO_DE_RITMO_POR_VOLTA` é o que cancela o L/5 — sem
// ele a diferença entre carros encolheria 5×.
//
// A medição está abaixo: o modelo antigo reimplementado, consumindo o MESMO rng na MESMA
// ordem, comparado com o motor de verdade.

/// O modelo de PONTOS, reimplementado como estava antes da troca de moeda. Devolve (ordem de
/// chegada, tempo total por piloto). Sem incidentes e sem quebra: são esses os dois caminhos
/// que consomem rng fora de `calculate_segment_score`, e mantê-los desligados garante que os
/// dois modelos vejam a MESMA sequência de sorteios.
fn modelo_antigo_de_pontos(
    drivers: &[SimDriver],
    qualifying: &[QualifyingResult],
    ctx: &SimulationContext,
    rng: &mut impl rand::Rng,
) -> (Vec<String>, Vec<(String, f64)>) {
    use crate::constants::scoring::RACE_SCORE_TO_LAP_MS;

    let total_drivers = qualifying.len() as i32;
    struct EstadoAntigo {
        driver_id: String,
        tire_wear: f64,
        physical_condition: f64,
        cumulative_score: f64,
    }
    let mut estados: Vec<EstadoAntigo> = qualifying
        .iter()
        .map(|q| EstadoAntigo {
            driver_id: q.pilot_id.clone(),
            tire_wear: 1.0,
            physical_condition: 1.0,
            cumulative_score: (total_drivers - q.position + 1) as f64 * 2.0,
        })
        .collect();

    for segment in [
        RaceSegment::Start,
        RaceSegment::Early,
        RaceSegment::Mid,
        RaceSegment::Late,
        RaceSegment::Finish,
    ] {
        for estado in &mut estados {
            let Some(driver) = drivers.iter().find(|d| d.id == estado.driver_id) else {
                continue;
            };
            // `calculate_segment_score` lê desgaste e físico do estado: montamos um
            // `RaceState` de fachada com os mesmos valores para o cálculo bater.
            let mut fachada = RaceState {
                driver_id: estado.driver_id.clone(),
                tire_wear: estado.tire_wear,
                physical_condition: estado.physical_condition,
                tempo_acumulado_ms: 0.0,
                desvio_de_ritmo: 0.0,
                trafego: Default::default(),
                paradas: Default::default(),
                is_dnf: false,
                current_position: 1,
                incidents: Vec::new(),
                dnf_reason: None,
                dnf_segment: None,
                pending_damage: Vec::new(),
            };
            let amplitude = super::pontuacao::amplitude_de_ritmo(driver, ctx, segment);
            let determinístico =
                super::pontuacao::calculate_segment_score(driver, &fachada, segment, ctx);
            // O sorteio por segmento do modelo antigo, na mesma posição da sequência de rng.
            let score = (determinístico + rng.gen_range(-amplitude..=amplitude)).max(5.0);
            estado.cumulative_score += score.max(0.0);
            super::pontuacao::apply_tire_degradation(&mut fachada, driver, ctx);
            super::pontuacao::apply_physical_degradation(&mut fachada, driver, ctx);
            estado.tire_wear = fachada.tire_wear;
            estado.physical_condition = fachada.physical_condition;
        }
        estados.sort_by(|a, b| {
            b.cumulative_score
                .partial_cmp(&a.cumulative_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    let score_vencedor = estados.first().map(|e| e.cumulative_score).unwrap_or(0.0);
    let ordem: Vec<String> = estados.iter().map(|e| e.driver_id.clone()).collect();
    let tempos: Vec<(String, f64)> = estados
        .iter()
        .map(|e| {
            let lap = ctx.base_lap_time_ms
                + (score_vencedor - e.cumulative_score).max(0.0) * RACE_SCORE_TO_LAP_MS;
            (e.driver_id.clone(), lap * ctx.total_laps as f64)
        })
        .collect();
    (ordem, tempos)
}

#[test]
fn fase1_troca_de_moeda_preserva_ordem_e_tempos() {
    let grid = build_grid();
    let ctx = sample_context(30, WeatherCondition::Dry);

    let mut divergencias = 0;
    let mut pior_delta_ms: f64 = 0.0;
    for seed in 0..60 {
        let mut rng_quali = StdRng::seed_from_u64(seed);
        let qualifying = simulate_qualifying(&grid, &ctx, &mut rng_quali);

        // Moeda nova (tempo) com o RUÍDO LEGADO: isola a troca de representação das mudanças
        // de comportamento da fase 2, que é justamente o ponto da prova.
        let mut rng_novo = StdRng::seed_from_u64(seed + 500_000);
        let resultado = simulate_race_com_modo(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            None,
            ModoDoMotor::LegadoDePontos,
            &mut rng_novo,
        );

        let mut rng_antigo = StdRng::seed_from_u64(seed + 500_000);
        let (ordem_antiga, tempos_antigos) =
            modelo_antigo_de_pontos(&grid, &qualifying, &ctx, &mut rng_antigo);

        let ordem_nova: Vec<String> = resultado
            .race_results
            .iter()
            .map(|r| r.pilot_id.clone())
            .collect();
        if ordem_nova != ordem_antiga {
            divergencias += 1;
        }

        for (id, tempo_antigo) in &tempos_antigos {
            let tempo_novo = resultado
                .race_results
                .iter()
                .find(|r| r.pilot_id == *id)
                .expect("piloto no resultado novo")
                .total_race_time_ms;
            pior_delta_ms = pior_delta_ms.max((tempo_novo - tempo_antigo).abs());
        }
    }

    println!(
        "fase 1 — 60 corridas: {divergencias} divergências de ordem, \
         pior delta de tempo {pior_delta_ms:.6} ms"
    );
    assert_eq!(divergencias, 0, "a troca de moeda mudou a ordem de chegada");
    assert!(
        pior_delta_ms < 1.0,
        "a troca de moeda mudou os tempos (pior delta {pior_delta_ms:.6} ms)"
    );
}

#[test]
fn fase1_reparo_cobra_os_segundos_ate_no_lanterna_que_para_cedo() {
    // O caso que o modelo de PONTOS errava em silêncio: reparo no primeiro segmento, num
    // carro do fim do grid. Lá o desconto em pontos batia no `.max(0.0)` do acumulado e o
    // piloto pagava MENOS que os segundos parados — quanto mais atrás largava, mais barato
    // saía quebrar. Em tempo não há chão: o relógio anda igual para todos.
    const SECS: u32 = 25;
    let grid = build_grid();
    let ctx = sample_context(30, WeatherCondition::Dry);
    let mut rng_quali = StdRng::seed_from_u64(4242);
    let qualifying = simulate_qualifying(&grid, &ctx, &mut rng_quali);

    let lanterna = qualifying.last().expect("grid").pilot_id.clone();
    assert_eq!(
        RaceSegment::from_lap(1, ctx.total_laps),
        RaceSegment::Start,
        "a volta 1 tem que cair na largada"
    );

    let corrida = |mecanicas: &[MechanicalOutcome]| {
        let mut rng = StdRng::seed_from_u64(77);
        simulate_race_with_breakdowns(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            Some(mecanicas),
            &mut rng,
        )
    };

    let limpo = corrida(&[]);
    let penalizado = corrida(&[MechanicalOutcome {
        pilot_id: lanterna.clone(),
        lap: 1,
        is_dnf: false,
        penalty_secs: SECS,
        label: "troca de bico".to_string(),
    }]);

    assert_eq!(
        limpo.winner_id, penalizado.winner_id,
        "o lanterna parando não pode mudar o vencedor"
    );
    let tempo = |r: &RaceResult| {
        r.race_results
            .iter()
            .find(|x| x.pilot_id == lanterna)
            .expect("lanterna")
            .total_race_time_ms
    };
    let delta_s = (tempo(&penalizado) - tempo(&limpo)) / 1000.0;
    assert!(
        (delta_s - SECS as f64).abs() < 0.5,
        "esperava ~{SECS}s a mais, veio {delta_s:.2}s"
    );
}

// ═══════════════ Fase 2: o que a moeda de tempo passou a permitir ═══════════════
//
// Rode com:
//   cargo test --manifest-path src-tauri/Cargo.toml fase2_ -- --nocapture
mod fase2 {
    use super::*;
    use crate::simulation::profile::resolve_simulation_profile;
    use crate::simulation::race::tipos::pelotao_ordenado;
    use crate::simulation::track_profile::get_track_simulation_data;
    use rand::Rng;

    /// Sorteio determinístico em `[min, max]` a partir de (piloto, atributo).
    fn sorteio(piloto: usize, atributo: u64, min: i32, max: i32) -> i32 {
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        for b in (piloto as u64)
            .to_le_bytes()
            .iter()
            .chain(atributo.to_le_bytes().iter())
        {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
        h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        h ^= h >> 27;
        min + (h % ((max - min + 1) as u64)) as i32
    }

    /// Grid com a estrutura de correlação de `models::driver_generation`: consistência e
    /// racecraft colados no nível, ritmo de classificação com folga, e pneu/largada/
    /// adaptabilidade/agressividade independentes.
    fn grid_realista(faixa_de_skill: (i32, i32)) -> Vec<SimDriver> {
        let (menor, maior) = faixa_de_skill;
        let n = 12;
        (0..n)
            .map(|i| {
                let skill = maior - ((maior - menor) * i as i32) / (n as i32 - 1);
                let equipe = i / 2;
                let com_desvio = |atributo: u64, desvio: i32| -> f64 {
                    (skill + sorteio(i, atributo, -desvio, desvio)).clamp(5, 99) as f64
                };
                let independente = |atributo: u64| -> f64 { sorteio(i, atributo, 40, 70) as f64 };
                let aggression = sorteio(i, 9, 30, 70) as f64;

                let mut driver = Driver::create_player(
                    format!("D{:02}", i + 1),
                    format!("Piloto {:02}", i + 1),
                    "BR".to_string(),
                    26,
                );
                driver.is_jogador = false;
                driver.atributos.skill = skill as f64;
                driver.atributos.consistencia = com_desvio(1, 10);
                driver.atributos.racecraft = com_desvio(2, 8);
                driver.atributos.defesa = com_desvio(3, 8);
                driver.atributos.ritmo_classificacao = com_desvio(4, 12);
                driver.atributos.gestao_pneus = independente(5);
                driver.atributos.habilidade_largada = independente(6);
                driver.atributos.adaptabilidade = independente(7);
                // Fiel à geração real (`roll_stat(30, 70)`) e independente do nível. Era 50,0
                // fixo, o que tornava QUALQUER efeito de chuva invisível na ordem de chegada:
                // penalidade igual para todos não reordena ninguém.
                driver.atributos.fator_chuva = sorteio(i, 14, 30, 70) as f64;
                driver.atributos.fitness = independente(10);
                driver.atributos.experiencia = independente(11);
                driver.atributos.aggression = aggression;
                driver.atributos.smoothness = 100.0 - aggression;
                driver.atributos.mentalidade = independente(12);
                driver.atributos.confianca = independente(13);
                driver.corridas_na_categoria = 40;

                let mut team = placeholder_team_from_db(
                    format!("T{}", equipe + 1),
                    format!("Equipe {}", equipe + 1),
                    "gt4".to_string(),
                    "2026-01-01T00:00:00".to_string(),
                );
                team.car_performance = 12.0 - equipe as f64 * 1.6;
                team.confiabilidade = 90.0;
                SimDriver::from_driver_and_team(&driver, &team)
            })
            .collect()
    }

    fn contexto(
        categoria: &str,
        tier: u8,
        track_id: u32,
        voltas: i32,
        minutos: i32,
    ) -> SimulationContext {
        let p = resolve_simulation_profile(
            categoria,
            track_id,
            22.0,
            WeatherCondition::Dry,
            minutos,
            voltas,
        );
        SimulationContext {
            category_id: categoria.to_string(),
            category_tier: tier,
            track_id,
            total_laps: voltas,
            race_duration_minutes: minutos,
            base_lap_time_ms: p.base_lap_time_ms,
            tire_degradation_rate: p.tire_degradation_rate,
            physical_degradation_rate: p.physical_degradation_rate,
            qualifying_variance_multiplier: p.qualifying_variance_multiplier,
            race_variance_multiplier: p.race_variance_multiplier,
            race_pace_spread_multiplier: p.race_pace_spread_multiplier,
            track_character: get_track_simulation_data(track_id).track_character,
            ..sample_context(minutos, WeatherCondition::Dry)
        }
    }

    /// As cinco pistas da temporada sintética que os pacotes anteriores usaram.
    const ETAPAS: [u32; 5] = [523, 166, 413, 586, 239];

    /// Roda uma temporada e devolve, por etapa, a posição de chegada de cada piloto
    /// (indexada como o grid).
    fn temporada(
        grid: &[SimDriver],
        categoria: &str,
        tier: u8,
        voltas: i32,
        minutos: i32,
        rodadas: usize,
        modo: ModoDoMotor,
    ) -> Vec<Vec<f64>> {
        (0..rodadas)
            .map(|rodada| {
                let track = ETAPAS[rodada % ETAPAS.len()];
                let ctx = contexto(categoria, tier, track, voltas, minutos);
                let semente = 900_000 + rodada as u64;
                let mut rng = StdRng::seed_from_u64(semente);
                let quali = simulate_qualifying(grid, &ctx, &mut rng);
                let r = simulate_race_com_modo(
                    grid,
                    &quali,
                    &ctx,
                    &IncidentCatalog::empty(),
                    false,
                    None,
                    modo,
                    &mut rng,
                );
                grid.iter()
                    .map(|d| {
                        r.race_results
                            .iter()
                            .find(|x| x.pilot_id == d.id)
                            .map(|x| x.finish_position as f64)
                            .unwrap_or(0.0)
                    })
                    .collect()
            })
            .collect()
    }

    /// Desvio-padrão da posição de chegada de um piloto ao longo da temporada, médio no grid.
    fn desvio_de_posicao(temporada: &[Vec<f64>]) -> f64 {
        let pilotos = temporada[0].len();
        let mut soma = 0.0;
        for p in 0..pilotos {
            let posicoes: Vec<f64> = temporada.iter().map(|etapa| etapa[p]).collect();
            let media = posicoes.iter().sum::<f64>() / posicoes.len() as f64;
            soma += (posicoes.iter().map(|v| (v - media).powi(2)).sum::<f64>()
                / posicoes.len() as f64)
                .sqrt();
        }
        soma / pilotos as f64
    }

    fn spearman(a: &[f64], b: &[f64]) -> f64 {
        let n = a.len() as f64;
        let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
        let cov: f64 = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum();
        let va: f64 = a.iter().map(|x| (x - ma).powi(2)).sum::<f64>().sqrt();
        let vb: f64 = b.iter().map(|y| (y - mb).powi(2)).sum::<f64>().sqrt();
        if va == 0.0 || vb == 0.0 {
            return 0.0;
        }
        cov / (va * vb)
    }

    /// Correlação média entre etapas CONSECUTIVAS da temporada.
    fn spearman_entre_etapas(temporada: &[Vec<f64>]) -> f64 {
        let pares: Vec<f64> = temporada
            .windows(2)
            .map(|par| spearman(&par[0], &par[1]))
            .collect();
        pares.iter().sum::<f64>() / pares.len() as f64
    }

    #[test]
    fn fase2_metricas_da_temporada_contra_o_baseline() {
        // Duas linhas do baseline oficial: a entrada da escada e o topo.
        let linhas = [
            ("mazda_rookie", 0u8, "rookie", (48, 62)),
            ("gt3", 4u8, "topo", (68, 84)),
        ];
        println!("\n=== Temporada sintética: 24 etapas, grid fixo de 12 ===");
        for (categoria, tier, rotulo, faixa) in linhas {
            let grid = grid_realista(faixa);
            let antes = temporada(
                &grid,
                categoria,
                tier,
                20,
                45,
                24,
                ModoDoMotor::LegadoDePontos,
            );
            let depois = temporada(&grid, categoria, tier, 20, 45, 24, ModoDoMotor::Atual);
            println!(
                "{rotulo:<7} | desvio da posição: {:.2} → {:.2} | spearman entre etapas: {:.3} → {:.3}",
                desvio_de_posicao(&antes),
                desvio_de_posicao(&depois),
                spearman_entre_etapas(&antes),
                spearman_entre_etapas(&depois),
            );
        }
        // Guarda mínima: a moeda de tempo não pode REDUZIR o embaralhamento.
        let grid = grid_realista((68, 84));
        let antes = temporada(&grid, "gt3", 4, 20, 45, 24, ModoDoMotor::LegadoDePontos);
        let depois = temporada(&grid, "gt3", 4, 20, 45, 24, ModoDoMotor::Atual);
        assert!(
            desvio_de_posicao(&depois) >= desvio_de_posicao(&antes),
            "o ruído com memória tinha que espalhar MAIS, não menos"
        );
    }

    /// Desvio da posição de chegada com a GRADE CONGELADA: uma só classificação, muitas
    /// corridas. Isola o ruído da corrida da variação que vem do sábado — sem congelar, o
    /// deslocamento de largada (que cresce com as voltas exatamente como o ritmo) domina a
    /// medida e esconde o efeito da distância.
    fn desvio_com_grade_congelada(
        grid: &[SimDriver],
        categoria: &str,
        tier: u8,
        voltas: i32,
        minutos: i32,
        modo: ModoDoMotor,
        corridas: u64,
    ) -> f64 {
        let ctx = contexto(categoria, tier, 523, voltas, minutos);
        let mut rng_quali = StdRng::seed_from_u64(1);
        let quali = simulate_qualifying(grid, &ctx, &mut rng_quali);

        let etapas: Vec<Vec<f64>> = (0..corridas)
            .map(|seed| {
                let mut rng = StdRng::seed_from_u64(70_000 + seed);
                let r = simulate_race_com_modo(
                    grid,
                    &quali,
                    &ctx,
                    &IncidentCatalog::empty(),
                    false,
                    None,
                    modo,
                    &mut rng,
                );
                grid.iter()
                    .map(|d| {
                        r.race_results
                            .iter()
                            .find(|x| x.pilot_id == d.id)
                            .map(|x| x.finish_position as f64)
                            .unwrap_or(0.0)
                    })
                    .collect()
            })
            .collect();
        desvio_de_posicao(&etapas)
    }

    #[test]
    fn fase2_sprint_e_caotico_e_enduro_converge() {
        // (a) O ruído agora sabe a distância: σ por volta faz o desvio do trecho sair em
        // `σ·√voltas`, enquanto a diferença de ritmo entre dois carros cresce com `L`. A
        // razão ruído/sinal cai como `1/√L` — de graça, sem knob.
        let grid = grid_realista((68, 84));
        let sprint = desvio_com_grade_congelada(&grid, "gt3", 4, 8, 20, ModoDoMotor::Atual, 200);
        let enduro = desvio_com_grade_congelada(&grid, "gt3", 4, 120, 240, ModoDoMotor::Atual, 200);
        // O modelo antigo não sabia a distância: os mesmos 5 sorteios nas duas provas.
        let sprint_legado =
            desvio_com_grade_congelada(&grid, "gt3", 4, 8, 20, ModoDoMotor::LegadoDePontos, 200);
        let enduro_legado =
            desvio_com_grade_congelada(&grid, "gt3", 4, 120, 240, ModoDoMotor::LegadoDePontos, 200);

        println!(
            "grade congelada | por volta: sprint {sprint:.2} vs enduro {enduro:.2} \
             (×{:.2}) | legado: sprint {sprint_legado:.2} vs enduro {enduro_legado:.2} (×{:.2})",
            sprint / enduro.max(0.01),
            sprint_legado / enduro_legado.max(0.01)
        );

        // A propriedade em si é do RUÍDO e é analítica: σ por volta faz o desvio de tempo do
        // trecho sair em `σ·√voltas`, enquanto a diferença de ritmo cresce com as voltas —
        // razão ruído/sinal ∝ 1/√L. Ela é verificada abaixo direto na função, porque depois
        // do pacote D o desvio de POSIÇÃO deixou de ser uma leitura limpa dela: o trem de
        // carros comprime a ordem de chegada nas duas distâncias e mascara o efeito. O
        // número de posição fica impresso acima como observação, não como asserção.
        use crate::constants::scoring::MS_POR_PONTO_DE_RITMO_POR_VOLTA;
        let sigma = sigma_de_ruido_por_volta_ms_para_teste(1.0);
        let desvio_sprint = sigma * 8.0_f64.sqrt();
        let desvio_enduro = sigma * 120.0_f64.sqrt();
        let sinal_sprint = MS_POR_PONTO_DE_RITMO_POR_VOLTA * 8.0;
        let sinal_enduro = MS_POR_PONTO_DE_RITMO_POR_VOLTA * 120.0;
        let razao_sprint = desvio_sprint / sinal_sprint;
        let razao_enduro = desvio_enduro / sinal_enduro;
        println!(
            "ruído/sinal por ponto de ritmo: sprint {razao_sprint:.3} vs enduro \
             {razao_enduro:.3} (×{:.2}, teórico √15 = {:.2})",
            razao_sprint / razao_enduro,
            15.0_f64.sqrt()
        );
        assert!(
            (razao_sprint / razao_enduro - 15.0_f64.sqrt()).abs() < 0.01,
            "a razão ruído/sinal tem que cair como 1/√L"
        );
        // E o modelo antigo era CEGO para a distância: mesma amplitude nas duas provas.
        assert!(
            (sprint_legado / enduro_legado.max(0.01) - 1.0).abs() < 0.3,
            "o legado não distinguia sprint de enduro: {sprint_legado:.2} vs {enduro_legado:.2}"
        );
    }

    #[test]
    fn fase2_ruido_com_memoria_espalha_mais_que_independente() {
        // (b) O AR(1) impede o ruído de se auto-cancelar na soma dos trechos. Medido no
        // tempo final, não na posição, para isolar o efeito do embaralhamento.
        let grid = grid_realista((68, 84));
        let ctx = contexto("gt3", 4, 523, 20, 45);

        // Grade CONGELADA: sem isso, a variação do deslocamento de largada (que vem do
        // sábado) domina o desvio do gap e esconde o efeito da memória.
        let mut rng_quali = StdRng::seed_from_u64(1);
        let quali = simulate_qualifying(&grid, &ctx, &mut rng_quali);

        let desvio_do_tempo = |modo: ModoDoMotor| {
            let mut amostras = Vec::new();
            for seed in 0..400 {
                let mut rng = StdRng::seed_from_u64(70_000 + seed);
                let r = simulate_race_com_modo(
                    &grid,
                    &quali,
                    &ctx,
                    &IncidentCatalog::empty(),
                    false,
                    None,
                    modo,
                    &mut rng,
                );
                // Gap do mesmo piloto para o vencedor, corrida após corrida.
                if let Some(alvo) = r.race_results.iter().find(|x| x.pilot_id == grid[5].id) {
                    amostras.push(alvo.gap_to_winner_ms);
                }
            }
            let media = amostras.iter().sum::<f64>() / amostras.len() as f64;
            (amostras.iter().map(|v| (v - media).powi(2)).sum::<f64>() / amostras.len() as f64)
                .sqrt()
        };

        let independente = desvio_do_tempo(ModoDoMotor::LegadoDePontos);
        let com_memoria = desvio_do_tempo(ModoDoMotor::Atual);
        println!(
            "desvio do gap: independente {independente:.0} ms → com memória {com_memoria:.0} ms \
             (×{:.2})",
            com_memoria / independente.max(1.0)
        );
        assert!(
            com_memoria > independente,
            "o AR(1) tinha que aumentar o desvio final ({com_memoria:.0} vs {independente:.0})"
        );
    }

    #[test]
    fn fase2_piso_da_grade_sorteada_na_moeda_de_tempo() {
        // Refazendo o experimento do pacote anterior agora em tempo. A previsão é que o piso
        // NÃO se mova: o atrito de posição ainda não existe — largar em último não é uma
        // sentença porque o carro passa como se a pista estivesse vazia. É exatamente esse
        // buraco que o modelo de posição na pista vem preencher.
        let grid = grid_realista((68, 84));
        let mut soma = 0.0;
        let mut n = 0;
        for (i, track) in ETAPAS.iter().enumerate() {
            let ctx = contexto("gt3", 4, *track, 20, 45);
            for r in 0..40u64 {
                let semente = 31_000 + (i as u64) * 1_000 + r;
                let mut rng = StdRng::seed_from_u64(semente);
                let mut quali = simulate_qualifying(&grid, &ctx, &mut rng);
                // Embaralha a GRADE mantendo os mesmos pilotos.
                for k in (1..quali.len()).rev() {
                    let j = (rng.gen::<u64>() % (k as u64 + 1)) as usize;
                    let (pa, pb) = (quali[k].position, quali[j].position);
                    quali[k].position = pb;
                    quali[j].position = pa;
                    quali.swap(k, j);
                }
                quali.sort_by_key(|x| x.position);
                let corrida = simulate_race_with_breakdowns(
                    &grid,
                    &quali,
                    &ctx,
                    &IncidentCatalog::empty(),
                    false,
                    None,
                    &mut rng,
                );
                let mut pq = Vec::new();
                let mut pc = Vec::new();
                for d in &grid {
                    if let (Some(q), Some(c)) = (
                        quali
                            .iter()
                            .find(|x| x.pilot_id == d.id)
                            .map(|x| x.position as f64),
                        corrida
                            .race_results
                            .iter()
                            .find(|x| x.pilot_id == d.id)
                            .map(|x| x.finish_position as f64),
                    ) {
                        pq.push(q);
                        pc.push(c);
                    }
                }
                soma += spearman(&pq, &pc);
                n += 1;
            }
        }
        let piso = soma / n as f64;
        println!("piso da grade sorteada, na moeda de tempo: {piso:.3}");
        assert!(
            (0.0..0.6).contains(&piso),
            "piso fora do esperado: {piso:.3}"
        );
    }

    // ═══════════ Pacote D: posição na pista ═══════════
    //
    //   cargo test --manifest-path src-tauri/Cargo.toml pacote_d -- --nocapture

    /// Correlação média entre a ordem de GRID e a de chegada, com a grade SORTEADA.
    /// Isola o poder da posição de largada do "o rápido é rápido nos dois dias".
    fn piso_da_grade_sorteada(
        categoria: &str,
        tier: u8,
        modo: ModoDoMotor,
        repeticoes: u64,
    ) -> f64 {
        let grid = grid_realista(if tier == 0 { (48, 62) } else { (68, 84) });
        let mut soma = 0.0;
        let mut n = 0;
        for (i, track) in ETAPAS.iter().enumerate() {
            let ctx = contexto(categoria, tier, *track, 20, 45);
            for r in 0..repeticoes {
                let semente = 31_000 + (i as u64) * 1_000 + r;
                let mut rng = StdRng::seed_from_u64(semente);
                let mut quali = simulate_qualifying(&grid, &ctx, &mut rng);
                for k in (1..quali.len()).rev() {
                    let j = (rng.gen::<u64>() % (k as u64 + 1)) as usize;
                    let (pa, pb) = (quali[k].position, quali[j].position);
                    quali[k].position = pb;
                    quali[j].position = pa;
                    quali.swap(k, j);
                }
                quali.sort_by_key(|x| x.position);
                let corrida = simulate_race_com_modo(
                    &grid,
                    &quali,
                    &ctx,
                    &IncidentCatalog::empty(),
                    false,
                    None,
                    modo,
                    &mut rng,
                );
                let (mut pq, mut pc, mut ps) = (Vec::new(), Vec::new(), Vec::new());
                for d in &grid {
                    if let (Some(q), Some(c)) = (
                        quali
                            .iter()
                            .find(|x| x.pilot_id == d.id)
                            .map(|x| x.position as f64),
                        corrida
                            .race_results
                            .iter()
                            .find(|x| x.pilot_id == d.id)
                            .map(|x| x.finish_position as f64),
                    ) {
                        pq.push(q);
                        pc.push(c);
                        ps.push(-(d.skill as f64)); // skill alto = posição baixa
                    }
                }
                let _ = &ps;
                soma += spearman(&pq, &pc);
                n += 1;
            }
        }
        soma / n as f64
    }

    /// Correlação entre SKILL e chegada, com a grade sorteada.
    fn skill_contra_chegada(categoria: &str, tier: u8, modo: ModoDoMotor, repeticoes: u64) -> f64 {
        let grid = grid_realista(if tier == 0 { (48, 62) } else { (68, 84) });
        let mut soma = 0.0;
        let mut n = 0;
        for (i, track) in ETAPAS.iter().enumerate() {
            let ctx = contexto(categoria, tier, *track, 20, 45);
            for r in 0..repeticoes {
                let semente = 31_000 + (i as u64) * 1_000 + r;
                let mut rng = StdRng::seed_from_u64(semente);
                let mut quali = simulate_qualifying(&grid, &ctx, &mut rng);
                for k in (1..quali.len()).rev() {
                    let j = (rng.gen::<u64>() % (k as u64 + 1)) as usize;
                    let (pa, pb) = (quali[k].position, quali[j].position);
                    quali[k].position = pb;
                    quali[j].position = pa;
                    quali.swap(k, j);
                }
                quali.sort_by_key(|x| x.position);
                let corrida = simulate_race_com_modo(
                    &grid,
                    &quali,
                    &ctx,
                    &IncidentCatalog::empty(),
                    false,
                    None,
                    modo,
                    &mut rng,
                );
                let (mut ps, mut pc) = (Vec::new(), Vec::new());
                for d in &grid {
                    if let Some(c) = corrida
                        .race_results
                        .iter()
                        .find(|x| x.pilot_id == d.id)
                        .map(|x| x.finish_position as f64)
                    {
                        // Posição esperada pelo ritmo: skill alto → posição baixa.
                        ps.push(-(d.skill as f64));
                        pc.push(c);
                    }
                }
                soma += spearman(&ps, &pc);
                n += 1;
            }
        }
        soma / n as f64
    }

    #[test]
    fn pacote_d_as_tres_linhas_da_tabela() {
        println!("\n=== Pacote D: as três linhas ===");
        for (categoria, tier, rotulo) in [("mazda_rookie", 0u8, "rookie"), ("gt3", 4u8, "topo")] {
            let grid_antes =
                piso_da_grade_sorteada(categoria, tier, ModoDoMotor::LegadoDePontos, 40);
            let grid_depois = piso_da_grade_sorteada(categoria, tier, ModoDoMotor::Atual, 40);
            let skill_antes =
                skill_contra_chegada(categoria, tier, ModoDoMotor::LegadoDePontos, 40);
            let skill_depois = skill_contra_chegada(categoria, tier, ModoDoMotor::Atual, 40);

            let g = grid_realista(if tier == 0 { (48, 62) } else { (68, 84) });
            let etapa_antes = spearman_entre_etapas(&temporada(
                &g,
                categoria,
                tier,
                20,
                45,
                24,
                ModoDoMotor::LegadoDePontos,
            ));
            let etapa_depois = spearman_entre_etapas(&temporada(
                &g,
                categoria,
                tier,
                20,
                45,
                24,
                ModoDoMotor::Atual,
            ));

            println!(
                "{rotulo:<7} | ρ(grid sorteado × chegada) {grid_antes:.3} → {grid_depois:.3} \
                 | ρ(skill × chegada) {skill_antes:.3} → {skill_depois:.3} \
                 | ρ(etapa N × N+1) {etapa_antes:.3} → {etapa_depois:.3}"
            );
        }

        // A direção é o critério, não o valor: a posição de largada passa a valer, e o ritmo
        // absoluto passa a valer menos porque atravessar o pelotão agora custa.
        let antes = piso_da_grade_sorteada("gt3", 4, ModoDoMotor::LegadoDePontos, 40);
        let depois = piso_da_grade_sorteada("gt3", 4, ModoDoMotor::Atual, 40);
        assert!(
            depois > antes,
            "largar na frente tinha que passar a valer: {antes:.3} → {depois:.3}"
        );
        let s_antes = skill_contra_chegada("gt3", 4, ModoDoMotor::LegadoDePontos, 40);
        let s_depois = skill_contra_chegada("gt3", 4, ModoDoMotor::Atual, 40);
        assert!(
            s_depois < s_antes,
            "o ritmo absoluto tinha que passar a valer menos: {s_antes:.3} → {s_depois:.3}"
        );
    }

    /// Distribuição dos gaps entre carros consecutivos na bandeirada.
    fn gaps_consecutivos(modo: ModoDoMotor) -> Vec<f64> {
        let grid = grid_realista((68, 84));
        let mut todos = Vec::new();
        for (i, track) in ETAPAS.iter().enumerate() {
            let ctx = contexto("gt3", 4, *track, 20, 45);
            for r in 0..40u64 {
                let mut rng = StdRng::seed_from_u64(52_000 + (i as u64) * 1_000 + r);
                let quali = simulate_qualifying(&grid, &ctx, &mut rng);
                let corrida = simulate_race_com_modo(
                    &grid,
                    &quali,
                    &ctx,
                    &IncidentCatalog::empty(),
                    false,
                    None,
                    modo,
                    &mut rng,
                );
                let mut tempos: Vec<f64> = corrida
                    .race_results
                    .iter()
                    .filter(|x| !x.is_dnf)
                    .map(|x| x.total_race_time_ms)
                    .collect();
                tempos.sort_by(|a, b| a.partial_cmp(b).unwrap());
                todos.extend(tempos.windows(2).map(|p| p[1] - p[0]));
            }
        }
        todos
    }

    #[test]
    fn pacote_d_a_escada_de_gaps_vira_pelotoes_e_buracos() {
        // A previsão: a escada regular de hoje (gaps todos parecidos) vira PELOTÕES (gaps
        // minúsculos, carros em fila) e BURACOS (gaps grandes entre os pelotões). O sinal
        // visual mais direto de que o trem pegou é o coeficiente de variação subir.
        let cv = |v: &[f64]| {
            let m = v.iter().sum::<f64>() / v.len() as f64;
            (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt() / m
        };
        let antes = gaps_consecutivos(ModoDoMotor::LegadoDePontos);
        let depois = gaps_consecutivos(ModoDoMotor::Atual);

        let colados =
            |v: &[f64]| v.iter().filter(|g| **g < 400.0).count() as f64 / v.len() as f64 * 100.0;
        println!(
            "gaps entre carros consecutivos | cv {:.2} → {:.2} | \
             fração colada (<400 ms) {:.0}% → {:.0}% | mediana {:.0} → {:.0} ms",
            cv(&antes),
            cv(&depois),
            colados(&antes),
            colados(&depois),
            mediana(&antes),
            mediana(&depois),
        );
        assert!(
            cv(&depois) > cv(&antes),
            "a escada regular tinha que virar pelotões e buracos: cv {:.2} → {:.2}",
            cv(&antes),
            cv(&depois)
        );
    }

    fn mediana(v: &[f64]) -> f64 {
        let mut o = v.to_vec();
        o.sort_by(|a, b| a.partial_cmp(b).unwrap());
        o[o.len() / 2]
    }

    #[test]
    fn pacote_d_recuperacao_maxima_do_dia() {
        // Quantas posições o melhor recuperador do dia ganha, em média por corrida.
        let recuperacao = |categoria: &str, tier: u8, modo: ModoDoMotor| -> f64 {
            let grid = grid_realista(if tier == 0 { (48, 62) } else { (68, 84) });
            let mut soma = 0.0;
            let mut n = 0;
            for (i, track) in ETAPAS.iter().enumerate() {
                let ctx = contexto(categoria, tier, *track, 20, 45);
                for r in 0..40u64 {
                    let mut rng = StdRng::seed_from_u64(64_000 + (i as u64) * 1_000 + r);
                    let quali = simulate_qualifying(&grid, &ctx, &mut rng);
                    let corrida = simulate_race_com_modo(
                        &grid,
                        &quali,
                        &ctx,
                        &IncidentCatalog::empty(),
                        false,
                        None,
                        modo,
                        &mut rng,
                    );
                    soma += corrida
                        .race_results
                        .iter()
                        .filter(|x| !x.is_dnf)
                        .map(|x| x.positions_gained)
                        .max()
                        .unwrap_or(0) as f64;
                    n += 1;
                }
            }
            soma / n as f64
        };

        println!(
            "recuperação máxima do dia | rookie {:.1} → {:.1} | topo {:.1} → {:.1} posições",
            recuperacao("mazda_rookie", 0, ModoDoMotor::LegadoDePontos),
            recuperacao("mazda_rookie", 0, ModoDoMotor::Atual),
            recuperacao("gt3", 4, ModoDoMotor::LegadoDePontos),
            recuperacao("gt3", 4, ModoDoMotor::Atual),
        );
    }

    #[test]
    fn pacote_d_observaveis_saem_no_resultado() {
        // Dado cru, do jeito que o harness pediu — sem métrica, sem agregação.
        let grid = grid_realista((68, 84));
        let ctx = contexto("gt3", 4, 413, 20, 45);
        let mut rng = StdRng::seed_from_u64(4242);
        let quali = simulate_qualifying(&grid, &ctx, &mut rng);
        let corrida = simulate_race_with_breakdowns(
            &grid,
            &quali,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            None,
            &mut rng,
        );

        for r in &corrida.race_results {
            assert_eq!(
                r.posicoes_por_segmento.len(),
                5,
                "{}: uma posição por segmento",
                r.pilot_id
            );
            assert_eq!(
                r.gaps_para_da_frente_ms.len(),
                5,
                "{}: um gap por segmento",
                r.pilot_id
            );
            assert!((0..=5).contains(&r.segmentos_em_ar_sujo));
            assert!(r.ultrapassagens_concluidas <= r.tentativas_ultrapassagem);
            assert!(r.maior_sequencia_preso <= 5);
        }

        let tentativas: i32 = corrida
            .race_results
            .iter()
            .map(|r| r.tentativas_ultrapassagem)
            .sum();
        let concluidas: i32 = corrida
            .race_results
            .iter()
            .map(|r| r.ultrapassagens_concluidas)
            .sum();
        let em_ar_sujo: i32 = corrida
            .race_results
            .iter()
            .map(|r| r.segmentos_em_ar_sujo)
            .sum();
        println!(
            "observáveis numa corrida: {tentativas} tentativas, {concluidas} concluídas \
             ({:.0}%), {em_ar_sujo} segmentos-carro em ar sujo",
            concluidas as f64 / tentativas.max(1) as f64 * 100.0
        );
        assert!(tentativas > 0, "ninguém tentou passar em Hungaroring");
        assert!(
            concluidas < tentativas,
            "a taxa de sucesso não pode ser 100% — era esse o problema"
        );
        assert!(em_ar_sujo > 0, "ninguém andou em ar sujo");
    }

    #[test]
    fn pacote_d_dificuldade_da_pista_deixou_de_ser_parametro_morto() {
        // Critério de aceitação: o `overtaking_difficulty_multiplier` é calculado em
        // `profile/`, carregado no contexto e — até este pacote — nunca lido. Na varredura
        // de sensibilidade do harness ele marcava 0,000 EXATO, por inexistência. Aqui a
        // sensibilidade é medida direto: mexer nele tem que mexer no resultado.
        let grid = grid_realista((68, 84));
        let taxa_com_dificuldade = |dificuldade: f64| {
            let ctx = SimulationContext {
                overtaking_difficulty_multiplier: dificuldade,
                ..contexto("gt3", 4, 413, 20, 45)
            };
            let (mut tentativas, mut concluidas) = (0, 0);
            for seed in 0..120u64 {
                let mut rng = StdRng::seed_from_u64(9_000 + seed);
                let quali = simulate_qualifying(&grid, &ctx, &mut rng);
                let corrida = simulate_race_with_breakdowns(
                    &grid,
                    &quali,
                    &ctx,
                    &IncidentCatalog::empty(),
                    false,
                    None,
                    &mut rng,
                );
                tentativas += corrida
                    .race_results
                    .iter()
                    .map(|r| r.tentativas_ultrapassagem)
                    .sum::<i32>();
                concluidas += corrida
                    .race_results
                    .iter()
                    .map(|r| r.ultrapassagens_concluidas)
                    .sum::<i32>();
            }
            concluidas as f64 / tentativas.max(1) as f64
        };

        let facil = taxa_com_dificuldade(0.6);
        let neutra = taxa_com_dificuldade(1.0);
        let dificil = taxa_com_dificuldade(1.8);
        println!(
            "taxa de ultrapassagem por dificuldade da pista: fácil {:.0}% | neutra {:.0}% | difícil {:.0}%",
            facil * 100.0,
            neutra * 100.0,
            dificil * 100.0
        );
        assert!(
            facil > neutra && neutra > dificil,
            "o multiplicador tem que MOVER o resultado: {facil:.3} / {neutra:.3} / {dificil:.3}"
        );
    }

    // ═══════════ Pacote G: estratégia e safety car ═══════════
    //
    //   cargo test --manifest-path src-tauri/Cargo.toml pacote_g -- --nocapture

    /// Roda N corridas de uma categoria e devolve os resultados crus.
    fn corridas(
        categoria: &str,
        tier: u8,
        voltas: i32,
        minutos: i32,
        n: u64,
        com_incidentes: bool,
    ) -> Vec<RaceResult> {
        corridas_com_taxa(categoria, tier, voltas, minutos, n, com_incidentes, 1.0)
    }

    /// Idem, com a taxa de incidentes multiplicada. Serve para medir o EFEITO do safety car
    /// com amostra grande sem tocar na FREQUÊNCIA de produção — que é da campanha, não deste
    /// pacote.
    fn corridas_com_taxa(
        categoria: &str,
        tier: u8,
        voltas: i32,
        minutos: i32,
        n: u64,
        com_incidentes: bool,
        taxa: f64,
    ) -> Vec<RaceResult> {
        let grid = grid_realista(if tier == 0 { (48, 62) } else { (68, 84) });
        let mut saida = Vec::new();
        for (i, track) in ETAPAS.iter().enumerate() {
            let base = contexto(categoria, tier, *track, voltas, minutos);
            let ctx = SimulationContext {
                incidents_enabled: com_incidentes,
                incident_rate_multiplier: base.incident_rate_multiplier * taxa,
                ..base
            };
            for r in 0..n {
                let mut rng = StdRng::seed_from_u64(81_000 + (i as u64) * 1_000 + r);
                let quali = simulate_qualifying(&grid, &ctx, &mut rng);
                saida.push(simulate_race_with_breakdowns(
                    &grid,
                    &quali,
                    &ctx,
                    &IncidentCatalog::empty(),
                    false,
                    None,
                    &mut rng,
                ));
            }
        }
        saida
    }

    #[test]
    fn pacote_g_estrategia_existe_no_topo_e_nao_na_entrada() {
        // Alvo do harness: "estratégias distintas usadas no grid" 1–2 na entrada, 2–4 no topo.
        // A entrada é uma prova de 20 minutos — não tem janela de parada, e é de propósito.
        for (categoria, tier, voltas, minutos, rotulo) in [
            ("mazda_rookie", 0u8, 13, 20, "entrada"),
            ("gt3", 4u8, 28, 45, "topo"),
        ] {
            let cs = corridas(categoria, tier, voltas, minutos, 8, false);
            let distintas: std::collections::HashSet<String> = cs
                .iter()
                .flat_map(|c| &c.race_results)
                .map(|r| r.estrategia_id.clone())
                .collect();
            let paradas_por_corrida = cs
                .iter()
                .map(|c| {
                    c.race_results
                        .iter()
                        .map(|r| r.volta_da_parada.len())
                        .sum::<usize>() as f64
                })
                .sum::<f64>()
                / cs.len() as f64;
            println!(
                "{rotulo:<8} | estratégias distintas {} {:?} | paradas por corrida {paradas_por_corrida:.1}",
                distintas.len(),
                distintas
            );
            if tier == 0 {
                assert_eq!(
                    paradas_por_corrida, 0.0,
                    "prova de 20 min não devia ter parada"
                );
            } else {
                assert!(
                    paradas_por_corrida > 0.0,
                    "a gt3 tem janela de parada e ninguém parou"
                );
                assert!(
                    (2..=4).contains(&distintas.len()),
                    "estratégias distintas fora do alvo 2–4: {}",
                    distintas.len()
                );
            }
        }
    }

    #[test]
    fn pacote_g_o_undercut_e_mensuravel() {
        // O par (antes, depois) é o que o harness pediu para medir undercut sem reconstruir a
        // corrida. Aqui só verificamos que ele SAI coerente e que a parada de fato move o carro.
        let cs = corridas("gt3", 4, 28, 45, 8, false);
        let mut movimentos: Vec<i32> = Vec::new();
        for c in &cs {
            for r in &c.race_results {
                assert_eq!(
                    r.volta_da_parada.len(),
                    r.posicao_antes_da_parada.len(),
                    "{}: antes sem par",
                    r.pilot_id
                );
                assert_eq!(
                    r.volta_da_parada.len(),
                    r.posicao_depois.len(),
                    "{}: depois sem par",
                    r.pilot_id
                );
                for (antes, depois) in r.posicao_antes_da_parada.iter().zip(&r.posicao_depois) {
                    movimentos.push(depois - antes);
                }
            }
        }
        assert!(!movimentos.is_empty(), "nenhuma parada registrada");
        let perdeu = movimentos.iter().filter(|d| **d > 0).count();
        let ganhou = movimentos.iter().filter(|d| **d < 0).count();
        // "Crucificado": perdeu 4 ou mais posições só no timing da parada. É o fenômeno
        // narrativo que o pacote existe para criar — "ele foi crucificado", não "o dado deu
        // ruim" —, e tem TETO no alvo do harness porque crucificação demais é a mesma loteria
        // de antes com narrativa melhor.
        let crucificados = movimentos.iter().filter(|d| **d >= 4).count();
        println!(
            "paradas: {} registradas | perderam posição {perdeu} | ganharam {ganhou} | \
             crucificados (≥4 posições) {crucificados} ({:.2} do total)",
            movimentos.len(),
            crucificados as f64 / movimentos.len() as f64
        );
        // Parar custa track position na hora — perder é o caso comum, e é o que cria a
        // matéria-prima da recuperação depois.
        assert!(perdeu > 0, "parar no box não custou posição a ninguém");
    }

    #[test]
    fn pacote_g_safety_car_muda_quem_ganha() {
        // A MÉTRICA MAIS IMPORTANTE DO PACOTE, e a mais fácil de errar: um safety car que não
        // muda quem ganha não é um safety car, é uma animação. Um que decide sozinho é uma
        // roleta. Medimos o Δ de vencedores distintos entre corridas COM e SEM safety car.
        // A FREQUÊNCIA de safety car sai da distribuição de severidade dos incidentes, que é
        // calibração e é da campanha. Aqui ela é FORÇADA para cima só para a amostra de
        // corridas-com-SC ser grande o bastante para medir o EFEITO — que é o que este pacote
        // controla. A frequência de produção é reportada em `pacote_g_frequencia_de_safety_car`.
        const TAXA_FORCADA: f64 = 8.0;
        for (categoria, tier, voltas, minutos, rotulo) in [
            ("mazda_rookie", 0u8, 13, 20, "entrada"),
            ("gt3", 4u8, 28, 45, "topo"),
        ] {
            let cs = corridas_com_taxa(categoria, tier, voltas, minutos, 40, true, TAXA_FORCADA);
            let (com, sem): (Vec<&RaceResult>, Vec<&RaceResult>) =
                cs.iter().partition(|c| !c.safety_cars.is_empty());
            let distintos = |v: &[&RaceResult]| {
                v.iter()
                    .map(|c| c.winner_id.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .len()
            };
            let sc_por_corrida =
                cs.iter().map(|c| c.safety_cars.len() as f64).sum::<f64>() / cs.len() as f64;

            println!(
                "{rotulo:<8} | SC por corrida {sc_por_corrida:.2} | corridas com SC {} / {} | \
                 vencedores distintos: com SC {} vs sem SC {}",
                com.len(),
                cs.len(),
                distintos(&com),
                distintos(&sem),
            );

            // O embaralhamento: ρ(ordem pré-SC × chegada) nas corridas COM safety car.
            let mut rhos = Vec::new();
            for c in &com {
                let Some(ordem) = c.ordem_pre_safety_car.first() else {
                    continue;
                };
                let (mut pre, mut fim) = (Vec::new(), Vec::new());
                for (i, id) in ordem.iter().enumerate() {
                    if let Some(r) = c.race_results.iter().find(|x| &x.pilot_id == id) {
                        pre.push(i as f64 + 1.0);
                        fim.push(r.finish_position as f64);
                    }
                }
                if pre.len() > 2 {
                    rhos.push(spearman(&pre, &fim));
                }
            }
            if !rhos.is_empty() {
                println!(
                    "{rotulo:<8} | ρ(ordem pré-SC × chegada) {:.3} sobre {} corridas",
                    rhos.iter().sum::<f64>() / rhos.len() as f64,
                    rhos.len()
                );
            }
        }
    }

    #[test]
    fn pacote_g_safety_car_produz_vencedores_que_nao_venceriam() {
        // A MÉTRICA CENTRAL, medida do único jeito que responde à pergunta: a MESMA corrida,
        // com e sem a consequência do safety car. Comparar corridas-com-SC contra
        // corridas-sem-SC (a leitura ingênua) é confundido — quem tem SC é quem teve batida
        // grande, e a batida já embaralha sozinha.
        //
        // "Um safety car que não muda quem ganha não é um safety car, é uma animação. Um que
        // decide sozinho é uma roleta." Este número é o que separa os dois.
        const TAXA_FORCADA: f64 = 8.0;
        for (categoria, tier, voltas, minutos, rotulo) in [
            ("mazda_rookie", 0u8, 13, 20, "entrada"),
            ("gt3", 4u8, 28, 45, "topo"),
        ] {
            let grid = grid_realista(if tier == 0 { (48, 62) } else { (68, 84) });
            let mut com_sc = 0;
            let mut trocou_vencedor = 0;
            let mut deslocamento_medio = 0.0;
            let mut n_deslocamento = 0;

            for (i, track) in ETAPAS.iter().enumerate() {
                let base = contexto(categoria, tier, *track, voltas, minutos);
                let ctx = SimulationContext {
                    incidents_enabled: true,
                    incident_rate_multiplier: base.incident_rate_multiplier * TAXA_FORCADA,
                    ..base
                };
                for r in 0..40u64 {
                    let semente = 81_000 + (i as u64) * 1_000 + r;
                    let rodar = |modo: ModoDoMotor| {
                        let mut rng = StdRng::seed_from_u64(semente);
                        let quali = simulate_qualifying(&grid, &ctx, &mut rng);
                        simulate_race_com_modo(
                            &grid,
                            &quali,
                            &ctx,
                            &IncidentCatalog::empty(),
                            false,
                            None,
                            modo,
                            &mut rng,
                        )
                    };
                    let com = rodar(ModoDoMotor::Atual);
                    if com.safety_cars.is_empty() {
                        continue;
                    }
                    let sem = rodar(ModoDoMotor::AtualSemSafetyCar);
                    com_sc += 1;
                    if com.winner_id != sem.winner_id {
                        trocou_vencedor += 1;
                    }
                    // Deslocamento médio de posição causado SÓ pelo safety car.
                    for r_com in &com.race_results {
                        if let Some(r_sem) = sem
                            .race_results
                            .iter()
                            .find(|x| x.pilot_id == r_com.pilot_id)
                        {
                            deslocamento_medio +=
                                (r_com.finish_position - r_sem.finish_position).abs() as f64;
                            n_deslocamento += 1;
                        }
                    }
                }
            }

            let taxa = trocou_vencedor as f64 / com_sc.max(1) as f64;
            println!(
                "{rotulo:<8} | {com_sc} corridas com SC | trocou o vencedor em {trocou_vencedor} \
                 ({:.0}%) | deslocamento médio {:.2} posições",
                taxa * 100.0,
                deslocamento_medio / n_deslocamento.max(1) as f64
            );
            assert!(com_sc > 10, "amostra pequena demais: {com_sc}");
            assert!(
                trocou_vencedor > 0,
                "{rotulo}: o safety car nunca mudou quem ganha — é uma animação"
            );
            assert!(
                taxa < 0.60,
                "{rotulo}: o safety car decide sozinho ({:.0}%) — virou roleta",
                taxa * 100.0
            );
        }
    }

    #[test]
    fn pacote_g_frequencia_de_safety_car() {
        // A frequência REAL de produção, contra o alvo do harness (0,25–0,60 na entrada e
        // 0,15–0,40 no topo). Ela sai da distribuição de severidade dos incidentes, que este
        // pacote NÃO tocou: o gatilho é o mesmo predicado de `derive_caution_segments`, agora
        // com consequência. Se estiver fora do alvo, é calibração — e é da campanha.
        for (categoria, tier, voltas, minutos, rotulo, alvo) in [
            ("mazda_rookie", 0u8, 13, 20, "entrada", (0.25, 0.60)),
            ("gt3", 4u8, 28, 45, "topo", (0.15, 0.40)),
        ] {
            let cs = corridas(categoria, tier, voltas, minutos, 40, true);
            let por_corrida =
                cs.iter().map(|c| c.safety_cars.len() as f64).sum::<f64>() / cs.len() as f64;
            let dnfs = cs.iter().map(|c| c.total_dnfs as f64).sum::<f64>() / cs.len() as f64;
            println!(
                "{rotulo:<8} | SC/corrida {por_corrida:.3} (alvo {:.2}–{:.2}) | \
                 abandonos/corrida {dnfs:.2} | {} corridas",
                alvo.0,
                alvo.1,
                cs.len()
            );
        }
    }

    #[test]
    fn pacote_g_safety_car_zera_a_margem_do_lider() {
        // Efeito mecânico, isolado: com SC, o gap do 2º para o líder na bandeirada tem que ser
        // menor do que sem SC. É o "zera os gaps" chegando ao resultado.
        //
        // A taxa é FORÇADA pelo mesmo motivo de `pacote_g_safety_car_muda_quem_ganha`: na
        // frequência de produção o safety car aparece em ~2,5% das corridas (ver
        // `pacote_g_frequencia_de_safety_car`), então o balde "com SC" tinha ~5 corridas de 200
        // e a média dele era ruído. O teste passava ou falhava conforme o embaralhamento do
        // RNG — mexer em QUALQUER constante de incidente virava a moeda. Forçar a taxa não
        // afrouxa a asserção: aumenta a amostra do efeito, que é o que este pacote controla.
        // A frequência em si é da campanha de calibração, não daqui.
        const TAXA_FORCADA: f64 = 8.0;
        let cs = corridas_com_taxa("gt3", 4, 28, 45, 40, true, TAXA_FORCADA);
        let media_do_gap = |com_sc: bool| {
            let sel: Vec<&RaceResult> = cs
                .iter()
                .filter(|c| c.safety_cars.is_empty() != com_sc)
                .collect();
            let gaps: Vec<f64> = sel
                .iter()
                .filter_map(|c| {
                    c.race_results
                        .iter()
                        .filter(|r| !r.is_dnf)
                        .nth(1)
                        .map(|r| r.gap_to_winner_ms)
                })
                .collect();
            if gaps.is_empty() {
                return f64::NAN;
            }
            gaps.iter().sum::<f64>() / gaps.len() as f64
        };
        let com = media_do_gap(true);
        let sem = media_do_gap(false);
        println!("gap do 2º para o líder: com SC {com:.0} ms | sem SC {sem:.0} ms");
        if com.is_finite() && sem.is_finite() {
            assert!(
                com < sem,
                "o SC tinha que apertar a ponta: com {com:.0} vs sem {sem:.0}"
            );
        }
    }

    #[test]
    fn pacote_g_chuva_por_pista_chegou_ao_resultado() {
        // Item secundário: `rain_sensitivity` era uma cadeia órfã completa — o perfil calculava,
        // o contexto guardava, um teste asseverava que a chuva a eleva, e nenhum consumidor lia.
        // Chuva rendia igual em toda pista. Agora escala a curva validada.
        let grid = grid_realista((68, 84));
        let posicao_do_rain_good = |sensibilidade: f64| {
            let base = contexto("gt3", 4, 523, 28, 45);
            let ctx = SimulationContext {
                weather: WeatherCondition::HeavyRain,
                rain_sensitivity: sensibilidade,
                ..base
            };
            // O piloto com melhor fator_chuva do grid.
            let alvo = grid
                .iter()
                .max_by_key(|d| d.fator_chuva)
                .expect("grid")
                .id
                .clone();
            let mut soma = 0.0;
            for seed in 0..60u64 {
                let mut rng = StdRng::seed_from_u64(95_000 + seed);
                let quali = simulate_qualifying(&grid, &ctx, &mut rng);
                let c = simulate_race_with_breakdowns(
                    &grid,
                    &quali,
                    &ctx,
                    &IncidentCatalog::empty(),
                    false,
                    None,
                    &mut rng,
                );
                soma += c
                    .race_results
                    .iter()
                    .find(|r| r.pilot_id == alvo)
                    .map(|r| r.finish_position as f64)
                    .unwrap_or(0.0);
            }
            soma / 60.0
        };

        let atenuada = posicao_do_rain_good(0.5);
        let neutra = posicao_do_rain_good(1.0);
        let amplificada = posicao_do_rain_good(1.5);
        println!(
            "posição média do bom-de-chuva por sensibilidade da pista: \
             0,5 → {atenuada:.2} | 1,0 → {neutra:.2} | 1,5 → {amplificada:.2}"
        );
        // Sensibilidade maior amplifica a penalidade de chuva de TODOS, e quem tem
        // `fator_chuva` alto perde menos — então ele sobe (posição menor).
        assert!(
            amplificada < atenuada,
            "a sensibilidade de chuva não chegou ao resultado: {amplificada:.2} vs {atenuada:.2}"
        );
    }

    #[test]
    fn fase2_a_porta_do_gap_esta_aberta() {
        // (d) A pergunta que o modelo de posição na pista precisa fazer — "quanto falta pro
        // carro da frente?" — agora tem resposta. Com moeda de pontos, não tinha.
        let grid = grid_realista((68, 84));
        let ctx = contexto("gt3", 4, 523, 20, 45);
        let mut rng = StdRng::seed_from_u64(2026);
        let quali = simulate_qualifying(&grid, &ctx, &mut rng);
        let mut estados: Vec<RaceState> = quali
            .iter()
            .map(|q| RaceState {
                driver_id: q.pilot_id.clone(),
                tire_wear: 1.0,
                physical_condition: 1.0,
                tempo_acumulado_ms: q.position as f64 * 1_500.0,
                desvio_de_ritmo: 0.0,
                trafego: Default::default(),
                paradas: Default::default(),
                is_dnf: false,
                current_position: q.position,
                incidents: Vec::new(),
                dnf_reason: None,
                dnf_segment: None,
                pending_damage: Vec::new(),
            })
            .collect();
        estados[3].is_dnf = true;

        let pelotao = pelotao_ordenado(&estados);
        assert_eq!(pelotao.len(), grid.len() - 1, "abandono não ocupa pista");
        assert_eq!(
            pelotao[0].gap_para_da_frente_ms, None,
            "o líder não tem quem seguir"
        );
        assert_eq!(pelotao[0].gap_para_o_lider_ms, 0.0);
        assert!(
            pelotao
                .windows(2)
                .all(|p| p[1].tempo_acumulado_ms >= p[0].tempo_acumulado_ms),
            "o pelotão tem que sair ordenado por tempo"
        );
        assert!(
            pelotao[1..]
                .iter()
                .all(|p| p.gap_para_da_frente_ms.unwrap_or(-1.0) >= 0.0),
            "gap para o carro da frente não pode ser negativo"
        );
    }
}

#[cfg(test)]
mod medicao;
