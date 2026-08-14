//! Horizonte de planejamento e DNA da equipe: o que cada time PERSEGUE no carro,
//! antes de qualquer dinheiro entrar na conta.

use super::super::*;
use super::*;
// -------- Horizonte de planejamento --------

#[test]
fn horizonte_distribuicao_bate_20_30_30_20() {
    let n = 3000;
    let (mut single, mut three, mut five, mut season) = (0, 0, 0, 0);
    for i in 0..n {
        match planning_horizon(&format!("T{i}"), 1) {
            PlanningHorizon::SingleTrack => single += 1,
            PlanningHorizon::ThreeRaces => three += 1,
            PlanningHorizon::FiveRaces => five += 1,
            PlanningHorizon::FullSeason => season += 1,
        }
    }
    let frac = |x: i32| x as f64 / n as f64;
    assert!(
        (frac(single) - 0.20).abs() < 0.04,
        "single={}",
        frac(single)
    );
    assert!((frac(three) - 0.30).abs() < 0.04, "three={}", frac(three));
    assert!((frac(five) - 0.30).abs() < 0.04, "five={}", frac(five));
    assert!(
        (frac(season) - 0.20).abs() < 0.04,
        "season={}",
        frac(season)
    );
}

#[test]
fn horizonte_e_deterministico() {
    assert_eq!(
        planning_horizon("Team-42", 7),
        planning_horizon("Team-42", 7)
    );
}

#[test]
fn horizonte_re_rola_por_temporada() {
    let n = 500;
    let changed = (0..n)
        .filter(|i| planning_horizon(&format!("T{i}"), 1) != planning_horizon(&format!("T{i}"), 2))
        .count();
    assert!(
        changed > 200,
        "esperado muitos times re-rolando; mudaram {changed}"
    );
}

// -------- Demanda de manutenção --------

#[test]
fn demanda_normaliza_e_le_as_pistas() {
    // Vazio → balanceado.
    let (p, h, a) = maintenance_demand(&[]);
    assert!((p + h + a - 1.0).abs() < 1e-9);
    assert!((p - 1.0 / 3.0).abs() < 1e-9);
    // Monza (239) é power-heavy → P domina.
    let (p2, h2, a2) = maintenance_demand(&[239]);
    assert!((p2 + h2 + a2 - 1.0).abs() < 1e-6);
    assert!(
        p2 > h2 && p2 > a2,
        "Monza deveria exigir Power: P={p2} H={h2} A={a2}"
    );
}

// -------- Decisão de manutenção --------

#[test]
fn dna_e_estavel_e_nao_depende_de_temporada() {
    // Determinístico e permanente (a assinatura nem recebe temporada).
    assert_eq!(team_car_focus("Team-42"), team_car_focus("Team-42"));
}

#[test]
fn dna_distribuicao_40_20_20_20() {
    let n = 3000;
    let (mut b, mut p, mut h, mut a) = (0, 0, 0, 0);
    for i in 0..n {
        match team_car_focus(&format!("T{i}")) {
            CarFocus::Balanced => b += 1,
            CarFocus::Power => p += 1,
            CarFocus::Handling => h += 1,
            CarFocus::Acceleration => a += 1,
        }
    }
    let frac = |x: i32| x as f64 / n as f64;
    assert!((frac(b) - 0.40).abs() < 0.05, "balanced={}", frac(b));
    assert!((frac(p) - 0.20).abs() < 0.05, "power={}", frac(p));
    assert!((frac(h) - 0.20).abs() < 0.05, "handling={}", frac(h));
    assert!((frac(a) - 0.20).abs() < 0.05, "accel={}", frac(a));
}

#[test]
fn dna_pica_a_demanda_que_o_calendario_diverso_lavaria() {
    // Calendário diverso (2 P + 1 H + 1 A) → média ~balanceada, spread abaixo do gatilho.
    let diverse = maintenance_demand(&[239, 523, 489, 179]); // Monza+Spa(P) Ledenon(H) LongBeach(A)
    assert!(
        demand_spread(diverse) < DEMAND_PEAK_THRESHOLD,
        "calendário diverso deveria lavar pra balanceado: spread={}",
        demand_spread(diverse)
    );
    // DNA de potência empurra a demanda efetiva acima do gatilho.
    let blended = blend_with_focus(diverse, CarFocus::Power);
    let (p, h, a) = blended;
    assert!(
        p > h && p > a,
        "DNA deveria puxar power: P={p:.2} H={h:.2} A={a:.2}"
    );
    assert!(
        demand_spread(blended) >= DEMAND_PEAK_THRESHOLD,
        "DNA deveria peakar a demanda: spread={}",
        demand_spread(blended)
    );
    // DNA balanceado NÃO peaka (generalista continua generalista).
    assert!(demand_spread(blend_with_focus(diverse, CarFocus::Balanced)) < DEMAND_PEAK_THRESHOLD);
}

#[test]
fn time_com_dna_de_potencia_foca_mesmo_em_calendario_diverso() {
    use crate::models::team::placeholder_team_from_db;
    // Acha um id cujo DNA é potência.
    let team_id = (0..10_000)
        .map(|i| format!("P{i}"))
        .find(|id| team_car_focus(id) == CarFocus::Power)
        .expect("deveria existir id com DNA de potência");

    let conn = Connection::open_in_memory().unwrap();
    let mut team = placeholder_team_from_db(
        team_id.clone(),
        "Power DNA".to_string(),
        "gt3".to_string(),
        "2026-01-01T00:00:00".to_string(),
    );
    team.cash_balance = 1e12;
    team.debt_balance = 0.0;
    team.financial_state = "healthy".to_string();
    let car = seed_car("gt3", 0.5);
    team_car::upsert_team_car(&conn, &team_id, &car).unwrap();
    team.car = Some(car);

    // Calendário DIVERSO cuja média até pende de leve pra HANDLING (contra o DNA):
    // se mesmo assim o carro pesa power, é o DNA sustentando o foco, não o calendário.
    let diverse = [489, 325, 180, 318, 93, 188];
    for season in 1..=4 {
        for _ in 0..15 {
            maintain_team_car(
                &conn,
                &team,
                "gt3",
                season,
                &diverse,
                WearConditions::neutral(),
                None,
            )
            .unwrap();
            team.car = team_car::get_team_car(&conn, &team_id).unwrap();
        }
    }

    let car = team.car.as_ref().unwrap();
    let (p, h, a) = car.pha();
    assert!(
        p > h && p > a,
        "carro de DNA-power deveria pesar power num calendário diverso: P={p:.1} H={h:.1} A={a:.1}"
    );
    let engine = car.part(PartType::Engine).unwrap().level;
    let brakes = car.part(PartType::Brakes).unwrap().level;
    assert!(
        engine > brakes,
        "peça de power (motor {engine}) deveria superar a de handling (freios {brakes})"
    );
}

// -------- Feedback físico da quebra (§4.6) --------
