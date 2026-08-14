//! Assédio (quebra de contrato): o leilão entre IAs e o leilão que o JOGADOR decide.

use super::super::*;
use super::*;
#[test]
fn poaching_pass_ia_arranca_astro_contratado_pagando_multa() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");

    let mut team_rng = StdRng::seed_from_u64(4242);
    let mut poacher = sample_team("gt3", "TPOA", &mut team_rng);
    poacher.cash_balance = 2_000_000.0;
    let mut seller = sample_team("gt3", "TSEL", &mut team_rng);
    // Caixa do vendedor dá pra pagar o salário do astro, mas não pra cobrir o
    // assediante no leilão de retenção (Fase 2b.2).
    seller.cash_balance = 900_000.0;
    team_queries::insert_team(&conn, &poacher).expect("poacher team");
    team_queries::insert_team(&conn, &seller).expect("seller team");

    // Poacher: um razoável (mantém) + um FRACO (será dispensado no poaching).
    let poa_keep = sample_driver(
        "P_KEEP",
        "Poacher N1",
        Some("gt3"),
        75.0,
        DriverStatus::Ativo,
    );
    let mut poa_weak = sample_driver(
        "P_WEAK",
        "Poacher N2 Fraco",
        Some("gt3"),
        55.0,
        DriverStatus::Ativo,
    );
    poa_weak.atributos.midia = 20.0;
    // Seller: o ASTRO (skill+fama alto) + um coadjuvante forte (que NÃO é upgrade
    // sobre o elenco do poacher → o seller nunca vira poacher).
    let mut astro = sample_driver("P_ASTRO", "O Astro", Some("gt3"), 92.0, DriverStatus::Ativo);
    astro.atributos.midia = 90.0;
    let sel_other = sample_driver(
        "P_SEL2",
        "Seller N2",
        Some("gt3"),
        80.0,
        DriverStatus::Ativo,
    );
    for d in [&poa_keep, &poa_weak, &astro, &sel_other] {
        driver_queries::insert_driver(&conn, d).expect("driver");
    }

    let seed =
        |id: &str, d: &Driver, t: &crate::models::team::Team, role: TeamRole, salary: f64| {
            let c = Contract::new(
                id.to_string(),
                d.id.clone(),
                d.nome.clone(),
                t.id.clone(),
                t.nome.clone(),
                1,
                2,
                salary,
                role,
                "gt3".to_string(),
            );
            contract_queries::insert_contract(&conn, &c).expect("contract");
        };
    seed("C_KEEP", &poa_keep, &poacher, TeamRole::Numero1, 150_000.0);
    seed("C_WEAK", &poa_weak, &poacher, TeamRole::Numero2, 100_000.0);
    seed("C_ASTRO", &astro, &seller, TeamRole::Numero1, 300_000.0);
    seed("C_SEL2", &sel_other, &seller, TeamRole::Numero2, 120_000.0);
    team_queries::update_team_pilots(&conn, &poacher.id, Some("P_KEEP"), Some("P_WEAK"))
        .expect("poacher lineup");
    team_queries::update_team_pilots(&conn, &seller.id, Some("P_ASTRO"), Some("P_SEL2"))
        .expect("seller lineup");

    let expected_buyout = crate::market::poaching::buyout_fee(300_000.0, 1, 92.0, 90.0);

    let teams = team_queries::get_all_teams(&conn).expect("teams");
    let mut rng = StdRng::seed_from_u64(7);
    let mut report = MarketReport::default();
    run_poaching_pass(&conn, &teams, 2, &mut rng, &mut report, &mut Vec::new())
        .expect("poaching pass");

    // O astro foi arrancado pra TPOA.
    let astro_contract = contract_queries::get_active_regular_contract_for_pilot(&conn, "P_ASTRO")
        .expect("query")
        .expect("astro tem contrato ativo");
    assert_eq!(astro_contract.equipe_id, "TPOA");
    // O dispensado ficou sem contrato e virou agente livre LIMPO (categoria None).
    assert!(
        contract_queries::get_active_regular_contract_for_pilot(&conn, "P_WEAK")
            .expect("query")
            .is_none()
    );
    let weak = driver_queries::get_driver(&conn, "P_WEAK").expect("weak driver");
    assert!(
        weak.categoria_atual.is_none(),
        "dispensado deve ser agente livre limpo"
    );
    // A multa andou de TPOA → TSEL.
    let poa = team_queries::get_team_by_id(&conn, "TPOA")
        .expect("q")
        .expect("poa");
    let sel = team_queries::get_team_by_id(&conn, "TSEL")
        .expect("q")
        .expect("sel");
    assert!(
        (poa.cash_balance - (2_000_000.0 - expected_buyout)).abs() < 1.0,
        "poacher pagou a multa: {}",
        poa.cash_balance
    );
    assert!(
        sel.cash_balance > seller.cash_balance,
        "seller recebeu a multa"
    );
    // Feed marcou como poaching.
    assert!(report
        .new_signings
        .iter()
        .any(|s| s.tipo == "poaching" && s.driver_id == "P_ASTRO"));
    // O leilão custou dinheiro: ele não vai pelo salário antigo.
    assert!(
        astro_contract.salario_anual > 300_000.0,
        "salario do arrancado: {}",
        astro_contract.salario_anual
    );
}

/// Fase 2b.2: o mesmo assédio, mas o time atual é rico e o astro é "Casa"
/// (vínculo 95) — o vínculo + caixa seguram o piloto, e a retenção sai cara.
#[test]
fn poaching_retencao_time_atual_segura_o_astro_com_vinculo_e_caixa() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");

    let mut team_rng = StdRng::seed_from_u64(4242);
    let mut poacher = sample_team("gt3", "TPOA", &mut team_rng);
    poacher.cash_balance = 2_000_000.0;
    let mut seller = sample_team("gt3", "TSEL", &mut team_rng);
    seller.cash_balance = 20_000_000.0; // cofre pra brigar
    team_queries::insert_team(&conn, &poacher).expect("poacher team");
    team_queries::insert_team(&conn, &seller).expect("seller team");

    let poa_keep = sample_driver(
        "P_KEEP",
        "Poacher N1",
        Some("gt3"),
        75.0,
        DriverStatus::Ativo,
    );
    let mut poa_weak = sample_driver(
        "P_WEAK",
        "Poacher N2 Fraco",
        Some("gt3"),
        55.0,
        DriverStatus::Ativo,
    );
    poa_weak.atributos.midia = 20.0;
    let mut astro = sample_driver("P_ASTRO", "O Astro", Some("gt3"), 92.0, DriverStatus::Ativo);
    astro.atributos.midia = 90.0;
    let sel_other = sample_driver(
        "P_SEL2",
        "Seller N2",
        Some("gt3"),
        80.0,
        DriverStatus::Ativo,
    );
    for d in [&poa_keep, &poa_weak, &astro, &sel_other] {
        driver_queries::insert_driver(&conn, d).expect("driver");
    }

    let seed =
        |id: &str, d: &Driver, t: &crate::models::team::Team, role: TeamRole, salary: f64| {
            let c = Contract::new(
                id.to_string(),
                d.id.clone(),
                d.nome.clone(),
                t.id.clone(),
                t.nome.clone(),
                1,
                2,
                salary,
                role,
                "gt3".to_string(),
            );
            contract_queries::insert_contract(&conn, &c).expect("contract");
        };
    seed("C_KEEP", &poa_keep, &poacher, TeamRole::Numero1, 150_000.0);
    seed("C_WEAK", &poa_weak, &poacher, TeamRole::Numero2, 100_000.0);
    seed("C_ASTRO", &astro, &seller, TeamRole::Numero1, 300_000.0);
    seed("C_SEL2", &sel_other, &seller, TeamRole::Numero2, 120_000.0);
    team_queries::update_team_pilots(&conn, &poacher.id, Some("P_KEEP"), Some("P_WEAK"))
        .expect("poacher lineup");
    team_queries::update_team_pilots(&conn, &seller.id, Some("P_ASTRO"), Some("P_SEL2"))
        .expect("seller lineup");
    // O astro é "Casa" no time atual.
    conn.execute(
        "INSERT INTO driver_team_bond (piloto_id, equipe_id, vinculo, temporadas)
         VALUES ('P_ASTRO', 'TSEL', 95.0, 6)",
        [],
    )
    .expect("seed bond");

    let teams = team_queries::get_all_teams(&conn).expect("teams");
    let mut rng = StdRng::seed_from_u64(7);
    let mut report = MarketReport::default();
    let mut audit = Vec::new();
    run_poaching_pass(&conn, &teams, 2, &mut rng, &mut report, &mut audit).expect("poaching pass");

    // O raio-x do debug enxerga a briga (é o que a tela de debug mostra).
    let a = audit.first().expect("houve um assedio");
    assert_eq!(a.target_name, "O Astro");
    assert_eq!(a.bond_holder, 95.0);
    assert!(!a.poacher_wins);
    assert!(a.bids.len() >= 2, "houve lance do assediante: {:?}", a.bids);

    // Ficou onde estava, e o dispensado NÃO foi dispensado.
    let astro_contract = contract_queries::get_active_regular_contract_for_pilot(&conn, "P_ASTRO")
        .expect("query")
        .expect("astro tem contrato ativo");
    assert_eq!(astro_contract.equipe_id, "TSEL");
    assert!(
        contract_queries::get_active_regular_contract_for_pilot(&conn, "P_WEAK")
            .expect("query")
            .is_some()
    );
    // Nenhuma multa andou.
    let poa = team_queries::get_team_by_id(&conn, "TPOA")
        .expect("q")
        .expect("poa");
    assert_eq!(poa.cash_balance, 2_000_000.0);
    // Segurar custou aumento, e o aumento ficou no contrato.
    assert!(
        astro_contract.salario_anual > 300_000.0,
        "retencao deve custar aumento: {}",
        astro_contract.salario_anual
    );
    assert!(report
        .new_signings
        .iter()
        .any(|s| s.tipo == "retencao" && s.driver_id == "P_ASTRO" && s.team_id == "TSEL"));
}

/// Cenário do jogador (Fase 2b.3): jogador famoso num time pequeno é cobiçado por
/// um time claramente melhor e rico. Monta a semente comum e devolve a conexão.
fn seed_player_poach_scenario() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");

    let mut team_rng = StdRng::seed_from_u64(99);
    // Time atual do jogador: fraco e pobre.
    let mut small = sample_team("gt3", "TSMALL", &mut team_rng);
    small.reputacao = 30.0;
    small.car_performance = 40.0;
    small.cash_balance = 400_000.0;
    small.historico_titulos_pilotos = 0;
    small.historico_titulos_construtores = 0;
    // Pretendente: gigante rico.
    let mut giant = sample_team("gt3", "TGIANT", &mut team_rng);
    giant.reputacao = 90.0;
    giant.car_performance = 92.0;
    giant.cash_balance = 15_000_000.0;
    giant.historico_titulos_pilotos = 5;
    giant.historico_titulos_construtores = 4;
    team_queries::insert_team(&conn, &small).expect("small team");
    team_queries::insert_team(&conn, &giant).expect("giant team");

    // Jogador: skill alto + FAMA de estrela.
    let mut player = sample_driver("PLR", "O Jogador", Some("gt3"), 88.0, DriverStatus::Ativo);
    player.is_jogador = true;
    player.atributos.midia = 90.0;
    // Coadjuvante do jogador no time pequeno.
    let small_n2 = sample_driver("SN2", "Coadjuvante", Some("gt3"), 60.0, DriverStatus::Ativo);
    // Dupla do gigante: um forte (N1) e um mais fraco (N2, será deslocado).
    let g1 = sample_driver("G1", "Gigante N1", Some("gt3"), 85.0, DriverStatus::Ativo);
    let g2 = sample_driver("G2", "Gigante N2", Some("gt3"), 72.0, DriverStatus::Ativo);
    for d in [&player, &small_n2, &g1, &g2] {
        driver_queries::insert_driver(&conn, d).expect("driver");
    }

    let seed =
        |id: &str, d: &Driver, t: &crate::models::team::Team, role: TeamRole, salary: f64| {
            let c = Contract::new(
                id.to_string(),
                d.id.clone(),
                d.nome.clone(),
                t.id.clone(),
                t.nome.clone(),
                1,
                2,
                salary,
                role,
                "gt3".to_string(),
            );
            contract_queries::insert_contract(&conn, &c).expect("contract");
        };
    seed("CPLR", &player, &small, TeamRole::Numero1, 200_000.0);
    seed("CSN2", &small_n2, &small, TeamRole::Numero2, 90_000.0);
    seed("CG1", &g1, &giant, TeamRole::Numero1, 400_000.0);
    seed("CG2", &g2, &giant, TeamRole::Numero2, 250_000.0);
    team_queries::update_team_pilots(&conn, "TSMALL", Some("PLR"), Some("SN2"))
        .expect("small lineup");
    team_queries::update_team_pilots(&conn, "TGIANT", Some("G1"), Some("G2"))
        .expect("giant lineup");
    conn
}

#[test]
fn player_poach_offer_surfaces_when_a_bigger_team_wants_the_star() {
    let conn = seed_player_poach_scenario();
    let offer = compute_player_poach_offer(&conn, 2)
        .expect("compute")
        .expect("o gigante deve cobiçar o jogador famoso");
    assert_eq!(offer.suitor_team_id, "TGIANT");
    assert_eq!(offer.current_team_id, "TSMALL");
    assert!(offer.buyout > 0.0);
    // O leilão tem lance do assediante (senão nem apareceria).
    assert!(offer.bids.iter().any(|b| b.is_poacher));
    // O gigante rico oferece bem acima do salário atual.
    assert!(
        offer.poacher_best > offer.current_salary,
        "poacher_best={}",
        offer.poacher_best
    );
    // Quem sairia da vaga do gigante é o mais fraco (G2).
    assert_eq!(offer.incumbent_name.as_deref(), Some("Gigante N2"));
}

#[test]
fn player_poach_accept_moves_the_player_and_pays_the_buyout() {
    let conn = seed_player_poach_scenario();
    let offer = compute_player_poach_offer(&conn, 2)
        .expect("compute")
        .expect("oferta");
    let small_cash_before = team_queries::get_team_by_id(&conn, "TSMALL")
        .unwrap()
        .unwrap()
        .cash_balance;

    let outcome = resolve_player_poach(&conn, &offer, true, 2).expect("resolve");
    assert!(outcome.applied && outcome.left);

    // O jogador agora está no gigante, pelo salário do melhor lance.
    let c = contract_queries::get_active_regular_contract_for_pilot(&conn, "PLR")
        .expect("q")
        .expect("contrato novo");
    assert_eq!(c.equipe_id, "TGIANT");
    assert_eq!(c.salario_anual, offer.poacher_best);
    // O N2 do gigante foi dispensado LIMPO (agente livre, categoria None).
    assert!(
        contract_queries::get_active_regular_contract_for_pilot(&conn, "G2")
            .expect("q")
            .is_none()
    );
    assert!(driver_queries::get_driver(&conn, "G2")
        .unwrap()
        .categoria_atual
        .is_none());
    // A multa entrou no caixa do time pequeno.
    let small_cash_after = team_queries::get_team_by_id(&conn, "TSMALL")
        .unwrap()
        .unwrap()
        .cash_balance;
    assert!((small_cash_after - (small_cash_before + offer.buyout)).abs() < 1.0);
}

#[test]
fn player_poach_decline_keeps_the_player_and_may_raise_salary() {
    let conn = seed_player_poach_scenario();
    let offer = compute_player_poach_offer(&conn, 2)
        .expect("compute")
        .expect("oferta");

    let outcome = resolve_player_poach(&conn, &offer, false, 2).expect("resolve");
    assert!(outcome.applied && !outcome.left);

    // Continua no time atual.
    let c = contract_queries::get_active_regular_contract_for_pilot(&conn, "PLR")
        .expect("q")
        .expect("contrato");
    assert_eq!(c.equipe_id, "TSMALL");
    // Se o time cobriu, o salário reflete a melhor cobertura (nunca abaixo do atual).
    assert_eq!(c.salario_anual, offer.holder_best.max(200_000.0));
    assert!(c.salario_anual >= 200_000.0);
}

#[test]
fn player_poach_none_for_an_unknown_free_agent() {
    // Jogador sem contrato / sem fama não é alvo de quebra de contrato.
    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("schema");
    let mut player = sample_driver("PLR", "Anônimo", Some("gt3"), 70.0, DriverStatus::Ativo);
    player.is_jogador = true;
    player.atributos.midia = 40.0; // longe de Estrela
    driver_queries::insert_driver(&conn, &player).expect("player");
    assert!(compute_player_poach_offer(&conn, 2)
        .expect("compute")
        .is_none());
}

#[test]
fn player_display_bids_dramatize_a_real_fight_to_min_turns() {
    // Leilão real curto (2 lances), mas os DOIS lados subiram → dramatiza p/ ≥5.
    let real = vec![
        PoachBid {
            team_name: "Y".into(),
            is_poacher: false,
            salary: 200_000.0,
            label: "abertura".into(),
        },
        PoachBid {
            team_name: "X".into(),
            is_poacher: true,
            salary: 400_000.0,
            label: "lance 1".into(),
        },
    ];
    let bids = build_player_display_bids(&real, "X", "Y", 200_000.0, 400_000.0, 340_000.0);
    assert!(bids.len() >= PLAYER_MIN_DISPLAY_BIDS, "len={}", bids.len());
    // Sobe monotônico.
    for w in bids.windows(2) {
        assert!(w[1].salary >= w[0].salary, "não-monotônico: {:?}", bids);
    }
    // Termina no vencedor (o maior lance = assediante 400k).
    let last = bids.last().unwrap();
    assert!(last.is_poacher && (last.salary - 400_000.0).abs() < 1.0);
    // Os dois lados aparecem na disputa.
    assert!(bids.iter().any(|b| b.is_poacher) && bids.iter().any(|b| !b.is_poacher));
}

#[test]
fn player_display_bids_dont_fake_a_fight_when_holder_stays_put() {
    // Time atual NÃO cobriu (holder_best == salário atual) → não inventa disputa.
    let real = vec![
        PoachBid {
            team_name: "Y".into(),
            is_poacher: false,
            salary: 200_000.0,
            label: "abertura".into(),
        },
        PoachBid {
            team_name: "X".into(),
            is_poacher: true,
            salary: 400_000.0,
            label: "lance 1".into(),
        },
    ];
    let bids = build_player_display_bids(&real, "X", "Y", 200_000.0, 400_000.0, 200_000.0);
    assert_eq!(bids.len(), real.len());
}
