use super::pontuacao::passes_dignity;
use super::*;
use rand::{rngs::StdRng, SeedableRng};

fn seat(id: &str, tier: u8, car: f64, prestige: f64, ceiling: f64) -> Seat {
    Seat {
        id: id.to_string(),
        team_id: format!("T-{id}"),
        category: match tier {
            0 => "mazda_rookie",
            1 => "mazda_amador",
            2 => "bmw_m2",
            3 => "gt4",
            4 => "gt3",
            _ => "endurance",
        }
        .to_string(),
        class: None,
        tier,
        is_n1: true,
        car_norm: car,
        prestige,
        required_license: 0,
        salary_floor: 10_000.0,
        salary_ceiling: ceiling,
    }
}
fn cand(id: &str, skill: f64, tier: u8) -> Candidate {
    Candidate {
        id: id.to_string(),
        skill,
        tier,
        brand: None,
        slam_target: None,
        max_license: 10,
        market_value: 50_000.0,
        ai_respects_brand: true,
        category: String::new(),
    }
}
fn rng() -> StdRng {
    StdRng::seed_from_u64(1)
}

#[test]
fn best_driver_takes_most_prestigious_seat() {
    // 2 vagas tier 4: uma com prestígio/carro alto, outra baixa. 1 craque + 1 mediano.
    let seats = vec![
        seat("A", 4, 90.0, 90.0, 200_000.0), // top
        seat("B", 4, 50.0, 20.0, 120_000.0), // fraca
    ];
    let cands = vec![cand("star", 88.0, 4), cand("mid", 70.0, 4)];
    let res = run_window(seats, cands, &WindowConfig::default(), &mut rng());
    let star = res.signings.iter().find(|s| s.driver_id == "star").unwrap();
    assert_eq!(
        star.seat_id, "A",
        "craque deve pegar a vaga mais prestigiada"
    );
    assert_eq!(res.signings.len(), 2);
}

#[test]
fn bid_escalates_until_accept() {
    // Vaga forte mas com piso baixo; o piloto só aceita quando o lance sobe.
    // market_value alto força a 1ª oferta a já ser decente; checamos que fecha.
    let seats = vec![seat("A", 3, 80.0, 70.0, 300_000.0)];
    let mut c = cand("p", 80.0, 3);
    c.market_value = 20_000.0; // começa baixo → escala
    let res = run_window(seats, vec![c], &WindowConfig::default(), &mut rng());
    assert_eq!(res.signings.len(), 1);
    assert!(res.signings[0].salary >= 20_000.0);
}

#[test]
fn dignity_floor_blocks_deep_drop() {
    let cfg = WindowConfig::default();
    let star = cand("star", 90.0, 4); // nível tier 4
                                      // Recusa cair 2 tiers (pra tier 2 ou abaixo); aceita lateral/1-tier-abaixo (tier 3).
    assert!(!passes_dignity(
        &cfg,
        &seat("t2", 2, 60.0, 60.0, 1.0),
        &star
    ));
    assert!(!passes_dignity(
        &cfg,
        &seat("t1", 1, 60.0, 60.0, 1.0),
        &star
    ));
    assert!(passes_dignity(&cfg, &seat("t3", 3, 60.0, 60.0, 1.0), &star));
    assert!(passes_dignity(&cfg, &seat("t4", 4, 60.0, 60.0, 1.0), &star));
}

#[test]
fn brand_ladder_prefers_same_brand() {
    // Piloto Mazda recebe oferta Mazda Cup e Toyota Cup (mesmo tier/prestígio).
    let mut mazda = seat("M", 1, 70.0, 70.0, 100_000.0);
    mazda.category = "mazda_amador".to_string();
    let mut toyota = seat("T", 1, 70.0, 70.0, 100_000.0);
    toyota.category = "toyota_amador".to_string();
    let mut c = cand("driver", 75.0, 1);
    c.brand = Some("mazda".to_string());
    let res = run_window(
        vec![mazda, toyota],
        vec![c],
        &WindowConfig::default(),
        &mut rng(),
    );
    let sign = &res.signings[0];
    assert_eq!(sign.category, "mazda_amador", "IA respeita a marca");
}

#[test]
fn cross_brand_only_when_no_same_brand() {
    // Só há vaga Toyota; o piloto Mazda aceita (fallback cross-brand).
    let mut toyota = seat("T", 1, 70.0, 70.0, 100_000.0);
    toyota.category = "toyota_amador".to_string();
    let mut c = cand("driver", 75.0, 1);
    c.brand = Some("mazda".to_string());
    let res = run_window(vec![toyota], vec![c], &WindowConfig::default(), &mut rng());
    assert_eq!(res.signings.len(), 1);
    assert_eq!(res.signings[0].category, "toyota_amador");
}

#[test]
fn craque_safety_net_always_signs() {
    // Vaga indigna (tier 0) e um craque tier 4: aceitação normal recusa, mas a
    // rede de segurança garante que o craque assina.
    let seats = vec![seat("rookie", 0, 50.0, 40.0, 80_000.0)];
    let res = run_window(
        seats,
        vec![cand("star", 90.0, 4)],
        &WindowConfig::default(),
        &mut rng(),
    );
    assert_eq!(res.signings.len(), 1, "craque nunca fica desempregado");
    assert!(res.unsigned.is_empty());
}

#[test]
fn weak_driver_may_go_unsigned() {
    // Sem vaga e piloto fraco → fica sem contrato (não há rede pra ele).
    let res = run_window(
        vec![],
        vec![cand("weak", 50.0, 1)],
        &WindowConfig::default(),
        &mut rng(),
    );
    assert!(res.signings.is_empty());
    assert_eq!(res.unsigned.len(), 1);
}

#[test]
#[ignore] // validação em escala: cargo test --lib zzz_scale_validation -- --ignored --nocapture
fn zzz_scale_validation() {
    use rand::Rng;
    let mut r = StdRng::seed_from_u64(42);
    let cfg = WindowConfig::default();
    let cats = [
        "mazda_rookie",
        "mazda_amador",
        "bmw_m2",
        "gt4",
        "gt3",
        "endurance",
    ];
    let mut seats = Vec::new();
    let mut cands = Vec::new();
    for tier in 0u8..6 {
        for i in 0..6 {
            seats.push(Seat {
                id: format!("S{tier}-{i}"),
                team_id: format!("T{tier}-{i}"),
                category: cats[tier as usize].to_string(),
                class: None,
                tier,
                is_n1: i % 2 == 0,
                car_norm: r.gen_range(40.0..95.0),
                prestige: r.gen_range(10.0..95.0),
                required_license: tier,
                salary_floor: 8_000.0,
                salary_ceiling: 30_000.0 + tier as f64 * 40_000.0 + r.gen_range(0.0..60_000.0),
            });
        }
        for i in 0..7 {
            let skill = (40.0 + tier as f64 * 9.0 + r.gen_range(-8.0..12.0)).clamp(25.0, 98.0);
            cands.push(Candidate {
                id: format!("D{tier}-{i}"),
                skill,
                tier,
                brand: None,
                slam_target: None,
                max_license: 10,
                market_value: 20_000.0 + skill * 1_000.0,
                ai_respects_brand: true,
                category: String::new(),
            });
        }
    }
    let (n_seats, n_cands) = (seats.len(), cands.len());
    let craques = cands.iter().filter(|c| c.skill >= cfg.craque_skill).count();
    let res = run_window(seats, cands, &cfg, &mut r);
    let craque_unsigned = res
        .unsigned
        .iter()
        .filter(|c| c.skill >= cfg.craque_skill)
        .count();
    let sals: Vec<f64> = res.signings.iter().map(|s| s.salary).collect();
    let mn = sals.iter().cloned().fold(f64::INFINITY, f64::min);
    let mx = sals.iter().cloned().fold(0.0_f64, f64::max);
    println!("\n=== VALIDAÇÃO EM ESCALA ===");
    println!("vagas {n_seats} | pilotos {n_cands} | craques (skill≥80) {craques}");
    println!(
        "assinaturas {} | sem vaga {} | semanas {}",
        res.signings.len(),
        res.unsigned.len(),
        res.weeks
    );
    println!("craques sem vaga: {craque_unsigned} (deve ser 0)");
    println!("salário assinado: min {mn:.0} | max {mx:.0}");
    let fill_rate = res.signings.len() as f64 / n_seats as f64;
    println!(
        "taxa de preenchimento: {:.0}% (resto vai pra rookies/promoção)",
        fill_rate * 100.0
    );
    assert_eq!(craque_unsigned, 0, "craque nunca fica sem vaga");
    assert!(
        fill_rate >= 0.80,
        "motor deve preencher a maioria das vagas (resto = rookies)"
    );
}

#[test]
fn player_signs_the_seat_he_accepts() {
    // 2 vagas tier 2; o jogador escolhe a vaga B (mesmo sendo a pior).
    let seats = vec![
        seat("A", 2, 80.0, 70.0, 100_000.0),
        seat("B", 2, 60.0, 40.0, 100_000.0),
    ];
    let mut player = cand("PLAYER", 75.0, 2);
    player.ai_respects_brand = false;
    let mut state = WindowState::start(
        seats,
        vec![player],
        WindowConfig::default(),
        Some("PLAYER".to_string()),
    );
    assert!(
        !state.player_offers().is_empty(),
        "jogador recebe ofertas na semana 1"
    );
    state.advance(Some("B")); // aceita a vaga B
    while !state.is_closed() {
        state.advance(None);
    }
    let res = state.into_result();
    let player_sign = res
        .signings
        .iter()
        .find(|s| s.driver_id == "PLAYER")
        .unwrap();
    assert_eq!(
        player_sign.seat_id, "B",
        "jogador assina a vaga que escolheu"
    );
}

#[test]
fn serialized_state_round_trips() {
    // O estado serializa/deserializa (persistência entre comandos da Fase 2).
    let seats = vec![seat("A", 2, 70.0, 60.0, 100_000.0)];
    let mut player = cand("PLAYER", 75.0, 2);
    player.ai_respects_brand = false;
    let state = WindowState::start(
        seats,
        vec![player],
        WindowConfig::default(),
        Some("PLAYER".to_string()),
    );
    let json = serde_json::to_string(&state).expect("serializa");
    let mut restored: WindowState = serde_json::from_str(&json).expect("deserializa");
    assert_eq!(restored.week(), 1);
    assert!(!restored.player_offers().is_empty());
    restored.advance(Some("A"));
    while !restored.is_closed() {
        restored.advance(None);
    }
    assert!(restored
        .into_result()
        .signings
        .iter()
        .any(|s| s.driver_id == "PLAYER"));
}

#[test]
fn slam_target_pulls_driver_to_category() {
    // Duas vagas equivalentes; o slam-chaser prefere a da categoria-alvo.
    let mut a = seat("A", 2, 70.0, 60.0, 100_000.0);
    a.category = "bmw_m2".to_string();
    let mut b = seat("B", 2, 70.0, 60.0, 100_000.0);
    b.category = "production_challenger".to_string();
    let mut c = cand("chaser", 75.0, 2);
    c.slam_target = Some("production_challenger".to_string());
    let res = run_window(vec![a, b], vec![c], &WindowConfig::default(), &mut rng());
    assert_eq!(res.signings[0].category, "production_challenger");
}
