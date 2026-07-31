//! Suíte de testes do pipeline de mercado (extraída de `pipeline.rs`).
//!
//! Continua sendo o mesmo módulo `tests` de antes: `use super::*` enxerga o
//! módulo `pipeline` inteiro, incluindo os itens privados.

use rand::{rngs::StdRng, SeedableRng};
use rusqlite::Connection;

use super::*;

/// Guarda a i18n do mercado nos dois locales: rótulos de lance + interpolação das
/// notas/eventos (sem `%{...}` cru). `#[serial]` (troca o locale global).
#[test]
#[serial_test::serial]
fn i18n_do_mercado_resolve_nos_dois_locales() {
    rust_i18n::set_locale("pt-BR");
    assert_eq!(bid_label(0), "abertura");
    assert_eq!(bid_label(3), "lance 3");
    let stayed = rust_i18n::t!("market.poach_outcome.stayed", team = "Alfa").to_string();
    assert!(
        stayed.contains("Alfa") && !stayed.contains("%{"),
        "{stayed}"
    );
    let dep = rust_i18n::t!(
        "market.event.departure_headline",
        driver = "Ana",
        team = "Beta"
    )
    .to_string();
    assert!(
        dep.contains("Ana") && dep.contains("Beta") && !dep.contains("%{"),
        "{dep}"
    );

    rust_i18n::set_locale("en-US");
    assert_eq!(bid_label(0), "opening");
    assert_eq!(bid_label(3), "bid 3");
    let deal = rust_i18n::t!("market.event.deal", category = "GT3").to_string();
    assert!(deal.contains("GT3") && !deal.contains("%{"), "{deal}");
    rust_i18n::set_locale("pt-BR"); // restaura
}

#[test]
fn feeder_promotion_prefers_talent_over_mediocre_champion() {
    // O craque skill-80 em carro fraco (8º) DEVE ser promovido na frente do
    // campeão skill-60 (1º) — era o inverso quando a ordem era só posição.
    let craque = feeder_promotion_score(80.0, 8);
    let campeao_mediano = feeder_promotion_score(60.0, 1);
    assert!(
        craque > campeao_mediano,
        "talento deve superar campeão medíocre: craque={craque} campeão={campeao_mediano}"
    );
    // Mas o empurrão de campeonato AINDA vale entre skills próximas: o campeão
    // (skill 70) supera um talento marginalmente melhor (skill 74) mal colocado.
    assert!(feeder_promotion_score(70.0, 1) > feeder_promotion_score(74.0, 10));
    // Quem não correu (pos 99) não ganha bônus nenhum.
    assert_eq!(feeder_promotion_score(70.0, 99), 70.0);
    // O bônus do campeão é limitado (não vira o critério dominante): +7.2 no 1º.
    assert!((feeder_promotion_score(0.0, 1) - 7.2).abs() < 1e-9);
}
use crate::constants::teams::get_team_templates;
use crate::db::migrations;
use crate::db::queries::seasons as season_queries;
use crate::models::license::driver_has_required_license_for_category;
use crate::models::season::Season;

#[test]
fn test_market_fills_all_vacancies() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(300);

    let report = run_market(&conn, 2, &mut rng).expect("market should run");

    assert_eq!(report.unresolved_vacancies, 0);
    assert!(find_vacancies(&conn).expect("vacancies").is_empty());
}

#[test]
fn test_merit_relegation_swaps_weak_top_driver_with_feeder_champion() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");

    let mut team_rng = StdRng::seed_from_u64(800);
    let bmw_team = sample_team("bmw_m2", "TBMW", &mut team_rng);
    let amateur_team = sample_team("mazda_amador", "TAMA", &mut team_rng);
    team_queries::insert_team(&conn, &bmw_team).expect("bmw team");
    team_queries::insert_team(&conn, &amateur_team).expect("amateur team");

    // BMW: titular forte (P_BMW1) + fraco que terminou em último (P_WEAK).
    let bmw1 = sample_driver(
        "P_BMW1",
        "BMW Forte",
        Some("bmw_m2"),
        70.0,
        DriverStatus::Ativo,
    );
    let weak = sample_driver(
        "P_WEAK",
        "BMW Fraco",
        Some("bmw_m2"),
        52.0,
        DriverStatus::Ativo,
    );
    // Amador: campeão com licença 1 (exigida pela BMW) -> merece subir.
    let champ = sample_driver(
        "P_CHAMP",
        "Amador Campeao",
        Some("mazda_amador"),
        74.0,
        DriverStatus::Ativo,
    );
    for driver in [&bmw1, &weak, &champ] {
        driver_queries::insert_driver(&conn, driver).expect("driver");
    }
    let seed_contract = |id: &str,
                         driver: &Driver,
                         team: &crate::models::team::Team,
                         role: TeamRole,
                         category: &str| {
        let contract = Contract::new(
            id.to_string(),
            driver.id.clone(),
            driver.nome.clone(),
            team.id.clone(),
            team.nome.clone(),
            1,
            2,
            100_000.0,
            role,
            category.to_string(),
        );
        contract_queries::insert_contract(&conn, &contract).expect("contract");
    };
    seed_contract("CB1", &bmw1, &bmw_team, TeamRole::Numero1, "bmw_m2");
    seed_contract("CWK", &weak, &bmw_team, TeamRole::Numero2, "bmw_m2");
    seed_contract(
        "CCH",
        &champ,
        &amateur_team,
        TeamRole::Numero1,
        "mazda_amador",
    );
    team_queries::update_team_pilots(&conn, &bmw_team.id, Some("P_BMW1"), Some("P_WEAK"))
        .expect("bmw lineup");
    team_queries::update_team_pilots(&conn, &amateur_team.id, Some("P_CHAMP"), None)
        .expect("amateur lineup");
    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         VALUES ('P_CHAMP', '1', 'mazda_amador', '2024-12-31T00:00:00', 1)",
        [],
    )
    .expect("license champ");

    let ctx = |pos: i32, total: i32, categoria: &str, tier: u8| DriverMarketContext {
        posicao_campeonato: pos,
        total_pilotos: total,
        categoria: categoria.to_string(),
        category_tier: tier,
        vitorias: 0,
        poles: 0,
        titulos: 0,
        papel: TeamRole::Numero2,
    };
    let mut contexts = HashMap::new();
    contexts.insert("P_BMW1".to_string(), ctx(1, 2, "bmw_m2", 2));
    contexts.insert("P_WEAK".to_string(), ctx(2, 2, "bmw_m2", 2)); // último na BMW
    contexts.insert("P_CHAMP".to_string(), ctx(1, 10, "mazda_amador", 1)); // campeão do amador

    let teams = team_queries::get_all_teams(&conn).expect("teams");
    let mut report = MarketReport::default();
    apply_merit_relegations(&conn, &teams, 2, &contexts, &mut report)
        .expect("rebaixamento deve rodar");

    // O campeão do amador subiu para a vaga da BMW; o fraco da BMW desceu.
    let champ_contract = contract_queries::get_active_regular_contract_for_pilot(&conn, "P_CHAMP")
        .expect("champ contract query")
        .expect("champ has active contract");
    let weak_contract = contract_queries::get_active_regular_contract_for_pilot(&conn, "P_WEAK")
        .expect("weak contract query")
        .expect("weak has active contract");
    assert_eq!(champ_contract.categoria, "bmw_m2");
    assert_eq!(champ_contract.equipe_id, "TBMW");
    assert_eq!(weak_contract.categoria, "mazda_amador");
    assert_eq!(weak_contract.equipe_id, "TAMA");
    assert!(report
        .new_signings
        .iter()
        .any(|s| s.tipo == "promocao_merito"));
    assert!(report.new_signings.iter().any(|s| s.tipo == "rebaixamento"));
}

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

#[test]
fn test_non_rookie_vacancy_is_filled_by_promoting_from_feeder_then_rookie_at_base() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");
    season_queries::insert_season(&conn, &Season::new("S002".to_string(), 2, 2025))
        .expect("season");

    let mut team_rng = StdRng::seed_from_u64(700);
    let rookie_team = sample_team("mazda_rookie", "TMR", &mut team_rng);
    let amateur_team = sample_team("mazda_amador", "TMA", &mut team_rng);
    team_queries::insert_team(&conn, &rookie_team).expect("rookie team");
    team_queries::insert_team(&conn, &amateur_team).expect("amateur team");

    // Grid de rookie cheio (fonte da promoção); amador com 1 titular -> N2 vago.
    let r1 = sample_driver(
        "PR1",
        "Rookie Um",
        Some("mazda_rookie"),
        60.0,
        DriverStatus::Ativo,
    );
    let r2 = sample_driver(
        "PR2",
        "Rookie Dois",
        Some("mazda_rookie"),
        50.0,
        DriverStatus::Ativo,
    );
    let a1 = sample_driver(
        "PA1",
        "Amador Um",
        Some("mazda_amador"),
        65.0,
        DriverStatus::Ativo,
    );
    for driver in [&r1, &r2, &a1] {
        driver_queries::insert_driver(&conn, driver).expect("driver");
    }
    let seed_contract = |id: &str,
                         driver: &Driver,
                         team: &crate::models::team::Team,
                         role: TeamRole,
                         category: &str| {
        let contract = Contract::new(
            id.to_string(),
            driver.id.clone(),
            driver.nome.clone(),
            team.id.clone(),
            team.nome.clone(),
            1,
            2,
            100_000.0,
            role,
            category.to_string(),
        );
        contract_queries::insert_contract(&conn, &contract).expect("contract");
    };
    seed_contract("CR1", &r1, &rookie_team, TeamRole::Numero1, "mazda_rookie");
    seed_contract("CR2", &r2, &rookie_team, TeamRole::Numero2, "mazda_rookie");
    seed_contract("CA1", &a1, &amateur_team, TeamRole::Numero1, "mazda_amador");
    team_queries::update_team_pilots(&conn, &rookie_team.id, Some("PR1"), Some("PR2"))
        .expect("rookie lineup");
    team_queries::update_team_pilots(&conn, &amateur_team.id, Some("PA1"), None)
        .expect("amateur lineup");
    // PR1 conquistou a licença 0 (top-metade do rookie) -> elegível por mérito a
    // subir para o amador (que exige licença 0). PR2 não tem -> não sobe.
    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         VALUES ('PR1', '0', 'mazda_rookie', '2024-12-31T00:00:00', 1)",
        [],
    )
    .expect("license PR1");
    conn.execute(
        "UPDATE meta SET value = '500' WHERE key = 'next_driver_id'",
        [],
    )
    .expect("driver counter");

    let teams = team_queries::get_all_teams(&conn).expect("teams");
    let mut report = MarketReport::default();
    let mut rng = StdRng::seed_from_u64(701);

    fill_remaining_vacancies_with_rookies(
        &conn,
        &teams,
        2,
        &mut report,
        &mut rng,
        None,
        &HashSet::new(),
    )
    .expect("cascata deve preencher a vaga sem erro");

    // A vaga não-estreia foi preenchida por PROMOÇÃO (não pelo pool, não por erro).
    assert!(
        report.new_signings.iter().any(|s| s.tipo == "promocao"),
        "deveria haver uma promoção da categoria de baixo"
    );
    // O melhor rookie (maior skill) foi o promovido.
    assert!(report
        .new_signings
        .iter()
        .any(|s| s.tipo == "promocao" && s.driver_id == "PR1"));
    // A base recebeu exatamente 1 rookie novo (o assento aberto na estreia).
    assert_eq!(
        driver_queries::count_drivers(&conn).expect("count"),
        4,
        "3 originais + 1 rookie gerado na base da cascata"
    );
    // Nenhuma vaga regular sobrou.
    assert_eq!(
        find_vacancies(&conn)
            .expect("vacancies")
            .into_iter()
            .filter(is_regular_vacancy)
            .count(),
        0
    );
}

#[test]
fn test_market_expired_contracts_processed() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(301);

    let report = run_market(&conn, 2, &mut rng).expect("market should run");

    assert!(report.contracts_expired >= 1);
    let status: String = conn
        .query_row(
            "SELECT status FROM contracts WHERE id = 'C002'",
            [],
            |row| row.get(0),
        )
        .expect("expired contract status");
    assert_eq!(status, "Expirado");
}

#[test]
fn test_market_all_teams_have_two_pilots() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(302);

    run_market(&conn, 2, &mut rng).expect("market should run");

    let teams = team_queries::get_all_teams(&conn).expect("teams");
    assert!(teams
        .iter()
        .all(|team| team.piloto_1_id.is_some() && team.piloto_2_id.is_some()));
}

#[test]
fn test_final_vacancy_fill_handles_production_as_regular_contract_category() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(312);
    let mut team_rng = StdRng::seed_from_u64(313);
    let production_team = sample_team("production_challenger", "T900", &mut team_rng);
    team_queries::insert_team(&conn, &production_team).expect("production team");
    for index in 0..4 {
        let driver_id = format!("P90{index}");
        let driver = sample_driver(
            &driver_id,
            &format!("Piloto Livre {index}"),
            None,
            65.0 + index as f64,
            DriverStatus::Ativo,
        );
        driver_queries::insert_driver(&conn, &driver).expect("free driver");
    }

    fill_all_remaining_vacancies(&conn, 2, &mut rng).expect("fill regular vacancies");

    let production_empty_slots: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM teams
             WHERE id = 'T900'
               AND (piloto_1_id IS NULL OR piloto_2_id IS NULL)",
            [],
            |row| row.get(0),
        )
        .expect("production empty slots");
    let production_contracts: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM contracts
             WHERE equipe_id = 'T900'
               AND status = 'Ativo'
               AND tipo = 'Regular'
               AND categoria = 'production_challenger'
               AND classe = 'mazda'",
            [],
            |row| row.get(0),
        )
        .expect("production regular contracts");

    assert_eq!(production_empty_slots, 0);
    assert_eq!(production_contracts, 2);
}

#[test]
fn test_regular_market_vacancy_discovery_includes_real_special_phase_categories() {
    let conn = setup_market_fixture();
    let mut team_rng = StdRng::seed_from_u64(316);
    let mut production_team = sample_team("production_challenger", "T902", &mut team_rng);
    production_team.piloto_1_id = None;
    production_team.piloto_2_id = None;
    team_queries::insert_team(&conn, &production_team).expect("production team");
    let mut endurance_team = sample_team("endurance", "T903", &mut team_rng);
    endurance_team.piloto_1_id = None;
    endurance_team.piloto_2_id = None;
    team_queries::insert_team(&conn, &endurance_team).expect("endurance team");
    let mut lmp2_team = sample_team("endurance", "T904", &mut team_rng);
    lmp2_team.classe = Some("lmp2".to_string());
    lmp2_team.piloto_1_id = None;
    lmp2_team.piloto_2_id = None;
    team_queries::insert_team(&conn, &lmp2_team).expect("endurance lmp2 team");

    let vacancies = find_vacancies(&conn).expect("vacancies");

    assert!(vacancies.iter().any(|vacancy| {
        vacancy.team_id == "T902" && vacancy.categoria == "production_challenger"
    }));
    assert!(vacancies
        .iter()
        .any(|vacancy| vacancy.team_id == "T903" && vacancy.categoria == "endurance"));
    assert!(vacancies
        .iter()
        .any(|vacancy| vacancy.team_id == "T904" && vacancy.categoria == "endurance"));
    assert!(!vacancies.iter().any(|vacancy| vacancy.categoria == "lmp2"));
}

#[test]
fn test_market_creates_regular_contracts_with_team_class_for_endurance_slots() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(314);
    let mut team_rng = StdRng::seed_from_u64(315);
    let mut endurance_team = sample_team("endurance", "T901", &mut team_rng);
    endurance_team.classe = Some("lmp2".to_string());
    endurance_team.piloto_1_id = None;
    endurance_team.piloto_2_id = None;
    team_queries::insert_team(&conn, &endurance_team).expect("endurance team");

    // Endurance agora recruta SÓ do gt3 (feeder [gt3], não mais [gt4, gt3]). A
    // fixture base traz feeder em gt4, então damos 2 pilotos de gt3 para as vagas
    // lmp2 do endurance — refletindo a escada nova (gt3 → endurance).
    for (id, nome, skill) in [
        ("P920", "GT3 Feeder Um", 80.0),
        ("P921", "GT3 Feeder Dois", 79.0),
    ] {
        let feeder = sample_driver(id, nome, Some("gt3"), skill, DriverStatus::Ativo);
        driver_queries::insert_driver(&conn, &feeder).expect("insert gt3 feeder");
    }

    run_market(&conn, 2, &mut rng).expect("market should run");

    let active_regular_endurance_contracts: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM contracts
             WHERE status = 'Ativo'
               AND tipo = 'Regular'
               AND categoria = 'endurance'
               AND classe = 'lmp2'
               AND equipe_id = 'T901'",
            [],
            |row| row.get(0),
        )
        .expect("regular endurance contracts");
    let special_contracts: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM contracts
             WHERE tipo = 'Especial'
               AND categoria IN ('production_challenger', 'endurance')",
            [],
            |row| row.get(0),
        )
        .expect("special contracts");
    let endurance_team_after = team_queries::get_team_by_id(&conn, "T901")
        .expect("team query")
        .expect("endurance team after market");

    assert_eq!(active_regular_endurance_contracts, 2);
    assert_eq!(special_contracts, 0);
    assert!(endurance_team_after.piloto_1_id.is_some());
    assert!(endurance_team_after.piloto_2_id.is_some());
}

#[test]
fn test_sync_reopens_slots_when_active_contract_category_or_class_differs_from_team() {
    let conn = setup_market_fixture();
    let mut team_rng = StdRng::seed_from_u64(317);
    let mut production_team = sample_team("production_challenger", "T904", &mut team_rng);

    let driver_a = sample_driver(
        "P904",
        "Piloto Categoria Errada",
        Some("gt4"),
        70.0,
        DriverStatus::Ativo,
    );
    let driver_b = sample_driver(
        "P905",
        "Piloto Classe Errada",
        Some("production_challenger"),
        69.0,
        DriverStatus::Ativo,
    );
    driver_queries::insert_driver(&conn, &driver_a).expect("driver a");
    driver_queries::insert_driver(&conn, &driver_b).expect("driver b");

    production_team.piloto_1_id = Some(driver_a.id.clone());
    production_team.piloto_2_id = Some(driver_b.id.clone());
    team_queries::insert_team(&conn, &production_team).expect("production team");

    let wrong_category = Contract::new(
        "C904".to_string(),
        driver_a.id.clone(),
        driver_a.nome.clone(),
        production_team.id.clone(),
        production_team.nome.clone(),
        1,
        2,
        70_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );
    let mut wrong_class = Contract::new(
        "C905".to_string(),
        driver_b.id.clone(),
        driver_b.nome.clone(),
        production_team.id.clone(),
        production_team.nome.clone(),
        1,
        2,
        70_000.0,
        TeamRole::Numero2,
        "production_challenger".to_string(),
    );
    wrong_class.classe = Some("toyota".to_string());
    contract_queries::insert_contract(&conn, &wrong_category).expect("wrong category");
    contract_queries::insert_contract(&conn, &wrong_class).expect("wrong class");

    let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(&conn)
        .expect("drivers")
        .into_iter()
        .map(|driver| (driver.id.clone(), driver))
        .collect();
    sync_team_slots(&conn, &[production_team.clone()], &drivers_by_id).expect("sync team slots");

    let vacancies = find_vacancies(&conn).expect("vacancies");
    let reopened = vacancies
        .iter()
        .filter(|vacancy| vacancy.team_id == production_team.id)
        .count();

    assert_eq!(reopened, 2);
}

#[test]
fn test_market_hierarchy_updated() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(303);

    run_market(&conn, 2, &mut rng).expect("market should run");

    let teams = team_queries::get_all_teams(&conn).expect("teams");
    assert!(teams.iter().all(|team| team.hierarquia_n1_id.is_some()));
    assert!(teams.iter().all(|team| team.hierarquia_n2_id.is_some()));
    assert!(teams
        .iter()
        .all(|team| team.hierarquia_status == TeamHierarchyClimate::Estavel.as_str()));
}

#[test]
fn test_run_market_classifies_existing_free_agent_as_transfer() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(300);

    let report = run_market(&conn, 2, &mut rng).expect("market should run");
    let signing = report
        .new_signings
        .iter()
        .find(|signing| signing.driver_id == "P004")
        .expect("experienced free agent should be signed");

    assert_eq!(
        signing.tipo, "transferencia",
        "piloto veterano ja existente no save nao deve ser classificado como rookie"
    );
}

#[test]
fn test_rookie_signing_candidate_only_counts_real_rookie_categories() {
    let driver = sample_driver("P999", "Piloto Novo", None, 60.0, DriverStatus::Ativo);
    let candidate = AvailableDriver {
        driver,
        visibility: 6.0,
        posicao_campeonato: 99,
        categoria_atual: String::new(),
        category_tier: 0,
        max_license_level: Some(4),
    };
    let expiring = HashMap::new();

    assert!(is_rookie_signing_candidate(
        &candidate,
        &expiring,
        "mazda_rookie"
    ));
    assert!(is_rookie_signing_candidate(
        &candidate,
        &expiring,
        "toyota_rookie"
    ));
    assert!(!is_rookie_signing_candidate(&candidate, &expiring, "gt3"));
}

#[test]
fn test_pool_fallback_skill_floor_blocks_weak_orphan_but_allows_capable_one() {
    // Item B (unitário): o pool de resgate exige skill compatível com o nível da
    // categoria (piso = média do tier − margem). Um lanterna (skill ~28) não é
    // mais içado para GT3/Endurance; um skill compatível ainda passa.
    let mut rng = StdRng::seed_from_u64(77);
    let gt3_team = sample_team("gt3", "TGT3B", &mut rng);
    let gt3_vac = fallback_vacancy_from_team(&gt3_team);
    let floor = pool_fallback_skill_floor(gt3_vac.category_tier);
    assert!(floor > 40.0, "o piso do GT3 deve ser alto o suficiente");

    let mk = |id: &str, cat: Option<&str>, skill: f64| AvailableDriver {
        driver: sample_driver(id, id, cat, skill, DriverStatus::Ativo),
        visibility: 1.0,
        posicao_campeonato: 99,
        categoria_atual: cat.map(str::to_string).unwrap_or_default(),
        category_tier: 0,
        max_license_level: Some(0),
    };

    // Lanterna órfão (o caso "Oliver", skill 28) → BLOQUEADO no GT3.
    let weak = mk("O_WEAK", None, 28.0);
    assert!(
        !is_pool_fallback_candidate(&weak, &gt3_vac),
        "órfão fraco não pode ser resgatado direto para GT3"
    );

    // Órfão com skill no nível da categoria → PERMITIDO.
    let capable = mk("O_CAP", None, floor + 5.0);
    assert!(
        is_pool_fallback_candidate(&capable, &gt3_vac),
        "órfão com skill compatível ainda pode preencher GT3"
    );

    // Piloto COM categoria atual (não é órfão) → nunca é candidato de pool.
    let contracted = mk("O_CONTRACTED", Some("gt3"), 90.0);
    assert!(!is_pool_fallback_candidate(&contracted, &gt3_vac));
}

#[test]
fn affordability_penalty_is_zero_within_budget_and_saturates_over() {
    // Dentro do teto → sem penalidade.
    assert_eq!(affordability_penalty(90_000.0, 100_000.0), 0.0);
    assert_eq!(affordability_penalty(100_000.0, 100_000.0), 0.0);
    // Acima do teto → cresce com o excesso.
    let mild = affordability_penalty(120_000.0, 100_000.0); // 20% acima
    let steep = affordability_penalty(200_000.0, 100_000.0); // 100% acima
    assert!(mild > 0.0 && steep > mild);
    // Satura no teto duro (2× acima já excede o CAP).
    assert_eq!(
        affordability_penalty(400_000.0, 100_000.0),
        AFFORDABILITY_PENALTY_CAP
    );
    // Teto inválido/zero → sem penalidade (robustez).
    assert_eq!(affordability_penalty(100_000.0, 0.0), 0.0);
}

#[test]
fn candidate_market_price_grows_with_skill_and_role() {
    let mut rng = StdRng::seed_from_u64(3);
    let tier = fallback_vacancy_from_team(&sample_team("gt3", "TP", &mut rng)).category_tier;
    // Monotônico na skill.
    assert!(candidate_market_price(90.0, tier, true) > candidate_market_price(60.0, tier, true));
    // N1 (titular) custa mais que N2 na mesma skill.
    assert!(candidate_market_price(75.0, tier, true) > candidate_market_price(75.0, tier, false));
}

#[test]
fn seat_desirability_rewards_better_car_and_more_prestige() {
    let mut rng = StdRng::seed_from_u64(4);
    let base = fallback_vacancy_from_team(&sample_team("gt3", "TSD", &mut rng));
    let mut better_car = base.clone();
    better_car.car_strength = base.car_strength + 4.0;
    let mut more_prestige = base.clone();
    more_prestige.reputacao = (base.reputacao + 30.0).min(100.0);
    assert!(seat_desirability(&better_car) > seat_desirability(&base));
    assert!(seat_desirability(&more_prestige) > seat_desirability(&base));
}

#[test]
fn affordability_makes_broke_seat_descend_to_cheaper_candidate() {
    // Item 1 (seleção): um assento que NÃO comporta o craque prefere um candidato mais
    // barato que CABE; um assento rico assina o craque; sem a flag (None), sempre o
    // craque (comportamento antigo). Gates duros iguais → o VALOR desempata.
    let mut rng = StdRng::seed_from_u64(5);
    let gt3 = sample_team("gt3", "TAFF", &mut rng);
    let vac = fallback_vacancy_from_team(&gt3);
    let tier = vac.category_tier;
    let is_n1 = matches!(vac.papel_necessario, TeamRole::Numero1);

    let mk = |id: &str, skill: f64| {
        let mut d = sample_driver(id, id, None, skill, DriverStatus::Ativo);
        d.atributos.midia = 0.0; // isola o efeito: sem fama
        AvailableDriver {
            driver: d,
            visibility: 1.0,
            posicao_campeonato: 5,
            categoria_atual: String::new(),
            category_tier: 0,
            max_license_level: Some(20), // gate de licença idêntico entre os dois
        }
    };
    let star = mk("STAR", 90.0);
    let cheap = mk("CHEAP", 64.0);

    let need = crate::fame::TEAM_NEED_MIN;
    let star_price = candidate_market_price(90.0, tier, is_n1);
    let cheap_price = candidate_market_price(64.0, tier, is_n1);
    assert!(star_price > cheap_price);

    // Teto que comporta o barato mas NÃO o craque → o assento DESCE para o barato.
    assert_eq!(
        compare_pool_fallback_candidates(&star, &cheap, &vac, need, Some(cheap_price)),
        std::cmp::Ordering::Less,
        "assento sem caixa deve preferir o candidato mais barato que cabe no teto"
    );
    // Teto folgado (assina o craque).
    assert_eq!(
        compare_pool_fallback_candidates(&star, &cheap, &vac, need, Some(star_price * 2.0)),
        std::cmp::Ordering::Greater,
        "assento rico deve assinar o craque"
    );
    // Flag off (None) → comportamento antigo: sempre o craque.
    assert_eq!(
        compare_pool_fallback_candidates(&star, &cheap, &vac, need, None),
        std::cmp::Ordering::Greater,
        "sem affordability o melhor skill sempre vence"
    );
}

#[test]
fn test_pool_fallback_for_non_rookie_vacancy_uses_experienced_lower_license_before_debutant() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");

    let mut team_rng = StdRng::seed_from_u64(611);
    let team = sample_team("gt3", "T900", &mut team_rng);
    team_queries::insert_team(&conn, &team).expect("insert gt3 team");

    let mut existing = sample_driver(
        "P900",
        "Piloto Titular",
        Some("gt3"),
        72.0,
        DriverStatus::Ativo,
    );
    existing.stats_carreira.corridas = 80;
    let mut experienced_lower_license =
        sample_driver("P901", "Piloto Experiente", None, 70.0, DriverStatus::Ativo);
    experienced_lower_license.stats_carreira.corridas = 24;
    let mut debutant_with_license =
        sample_driver("P902", "Piloto Estreante", None, 95.0, DriverStatus::Ativo);
    debutant_with_license.stats_carreira.corridas = 0;

    for driver in [
        &existing,
        &experienced_lower_license,
        &debutant_with_license,
    ] {
        driver_queries::insert_driver(&conn, driver).expect("insert driver");
    }

    let contract = Contract::new(
        "C900".to_string(),
        existing.id.clone(),
        existing.nome.clone(),
        team.id.clone(),
        team.nome.clone(),
        1,
        2,
        120_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &contract).expect("insert contract");
    team_queries::update_team_pilots(&conn, &team.id, Some(&existing.id), None)
        .expect("seed lineup");

    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         VALUES
         ('P900', '3', 'gt3', '2024-12-31T00:00:00', 2),
         ('P901', '1', 'mazda_amador', '2024-12-31T00:00:00', 2),
         ('P902', '3', 'gt3', '2024-12-31T00:00:00', 0)",
        [],
    )
    .expect("insert licenses");
    conn.execute(
        "UPDATE meta SET value = '901' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");
    conn.execute(
        "UPDATE meta SET value = '903' WHERE key = 'next_driver_id'",
        [],
    )
    .expect("driver counter");

    let teams = team_queries::get_all_teams(&conn).expect("teams");
    let mut report = MarketReport::default();
    let mut rng = StdRng::seed_from_u64(612);

    fill_remaining_vacancies_with_rookies(
        &conn,
        &teams,
        2,
        &mut report,
        &mut rng,
        None,
        &HashSet::new(),
    )
    .expect("fill vacancy");

    let refreshed = team_queries::get_team_by_id(&conn, &team.id)
        .expect("team query")
        .expect("team");
    assert_eq!(refreshed.piloto_2_id.as_deref(), Some("P901"));
    assert_ne!(refreshed.piloto_2_id.as_deref(), Some("P902"));
    assert!(
        driver_has_required_license_for_category(&conn, "P901", "gt3")
            .expect("fallback license should be granted"),
        "piloto experiente de carteira inferior deve ser regularizado ao ser usado como fallback"
    );
}

#[test]
fn test_pool_fallback_for_rookie_vacancy_keeps_retrying_rookie_before_veteran() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");
    let previous = Season::new("S001".to_string(), 1, 2024);
    let next = Season::new("S002".to_string(), 2, 2025);
    season_queries::insert_season(&conn, &previous).expect("previous season");
    season_queries::finalize_season(&conn, &previous.id).expect("finalize previous");
    season_queries::insert_season(&conn, &next).expect("next season");

    let mut team_rng = StdRng::seed_from_u64(613);
    let team = sample_team("mazda_rookie", "T920", &mut team_rng);
    team_queries::insert_team(&conn, &team).expect("insert rookie team");

    let mut existing = sample_driver(
        "P920",
        "Piloto Titular",
        Some("mazda_rookie"),
        62.0,
        DriverStatus::Ativo,
    );
    existing.stats_carreira.corridas = 0;
    existing.stats_carreira.temporadas = 0;
    let mut retrying_rookie = sample_driver(
        "P921",
        "Rookie Tentando",
        Some("mazda_rookie"),
        50.0,
        DriverStatus::Ativo,
    );
    retrying_rookie.stats_carreira.corridas = 8;
    retrying_rookie.stats_carreira.temporadas = 1;
    let mut amateur_veteran =
        sample_driver("P922", "Veterano Amador", None, 95.0, DriverStatus::Ativo);
    amateur_veteran.stats_carreira.corridas = 40;
    amateur_veteran.stats_carreira.temporadas = 4;

    for driver in [&existing, &retrying_rookie, &amateur_veteran] {
        driver_queries::insert_driver(&conn, driver).expect("insert driver");
    }

    let contract = Contract::new(
        "C920".to_string(),
        existing.id.clone(),
        existing.nome.clone(),
        team.id.clone(),
        team.nome.clone(),
        1,
        2,
        15_000.0,
        TeamRole::Numero1,
        "mazda_rookie".to_string(),
    );
    contract_queries::insert_contract(&conn, &contract).expect("insert contract");
    team_queries::update_team_pilots(&conn, &team.id, Some(&existing.id), None)
        .expect("seed lineup");
    insert_standing(
        &conn,
        &previous.id,
        &retrying_rookie.id,
        &team.id,
        "mazda_rookie",
        12,
        8.0,
        0,
        0,
    );
    insert_standing(
        &conn,
        &previous.id,
        &amateur_veteran.id,
        &team.id,
        "mazda_amador",
        8,
        30.0,
        0,
        0,
    );
    conn.execute(
        "UPDATE meta SET value = '921' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");

    let teams = team_queries::get_all_teams(&conn).expect("teams");
    let mut report = MarketReport::default();
    let mut rng = StdRng::seed_from_u64(614);

    fill_remaining_vacancies_with_rookies(
        &conn,
        &teams,
        2,
        &mut report,
        &mut rng,
        None,
        &HashSet::new(),
    )
    .expect("fill vacancy");

    let refreshed = team_queries::get_team_by_id(&conn, &team.id)
        .expect("team query")
        .expect("team");
    let lineup = [
        refreshed.piloto_1_id.as_deref(),
        refreshed.piloto_2_id.as_deref(),
    ];
    assert!(lineup.contains(&Some("P921")), "lineup: {lineup:?}");
    assert!(!lineup.contains(&Some("P922")), "lineup: {lineup:?}");
}

#[test]
fn test_pool_fallback_for_rookie_vacancy_generates_new_rookie_before_veteran() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");
    let previous = Season::new("S001".to_string(), 1, 2024);
    let next = Season::new("S002".to_string(), 2, 2025);
    season_queries::insert_season(&conn, &previous).expect("previous season");
    season_queries::finalize_season(&conn, &previous.id).expect("finalize previous");
    season_queries::insert_season(&conn, &next).expect("next season");

    let mut team_rng = StdRng::seed_from_u64(615);
    let team = sample_team("mazda_rookie", "T930", &mut team_rng);
    team_queries::insert_team(&conn, &team).expect("insert rookie team");

    let mut existing = sample_driver(
        "P930",
        "Piloto Titular",
        Some("mazda_rookie"),
        62.0,
        DriverStatus::Ativo,
    );
    existing.stats_carreira.corridas = 0;
    existing.stats_carreira.temporadas = 0;
    let mut amateur_veteran =
        sample_driver("P931", "Veterano Amador", None, 95.0, DriverStatus::Ativo);
    amateur_veteran.stats_carreira.corridas = 40;
    amateur_veteran.stats_carreira.temporadas = 4;

    for driver in [&existing, &amateur_veteran] {
        driver_queries::insert_driver(&conn, driver).expect("insert driver");
    }

    let contract = Contract::new(
        "C930".to_string(),
        existing.id.clone(),
        existing.nome.clone(),
        team.id.clone(),
        team.nome.clone(),
        1,
        2,
        15_000.0,
        TeamRole::Numero1,
        "mazda_rookie".to_string(),
    );
    contract_queries::insert_contract(&conn, &contract).expect("insert contract");
    team_queries::update_team_pilots(&conn, &team.id, Some(&existing.id), None)
        .expect("seed lineup");
    insert_standing(
        &conn,
        &previous.id,
        &amateur_veteran.id,
        &team.id,
        "mazda_amador",
        8,
        30.0,
        0,
        0,
    );
    conn.execute(
        "UPDATE meta SET value = '931' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");
    conn.execute(
        "UPDATE meta SET value = '932' WHERE key = 'next_driver_id'",
        [],
    )
    .expect("driver counter");

    let teams = team_queries::get_all_teams(&conn).expect("teams");
    let mut report = MarketReport::default();
    let mut rng = StdRng::seed_from_u64(616);

    fill_remaining_vacancies_with_rookies(
        &conn,
        &teams,
        2,
        &mut report,
        &mut rng,
        None,
        &HashSet::new(),
    )
    .expect("fill vacancy");

    let refreshed = team_queries::get_team_by_id(&conn, &team.id)
        .expect("team query")
        .expect("team");
    let lineup = [
        refreshed.piloto_1_id.as_deref(),
        refreshed.piloto_2_id.as_deref(),
    ];
    assert!(!lineup.contains(&Some("P931")), "lineup: {lineup:?}");
    assert_eq!(report.rookies_placed, 1);

    let generated_id = lineup
        .iter()
        .flatten()
        .find(|driver_id| **driver_id != "P930")
        .expect("generated rookie in lineup");
    let generated = driver_queries::get_driver(&conn, generated_id).expect("generated driver");
    assert_eq!(generated.stats_carreira.corridas, 0);
    assert_eq!(generated.stats_carreira.temporadas, 0);
    assert_eq!(generated.ano_inicio_carreira, 2025);
}

#[test]
fn test_final_vacancy_fill_leaves_non_debut_vacancy_open_when_no_candidate() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");
    // Ano jogável: gt3 não é categoria de entrada (seu feeder gt4 já existe),
    // então a vaga sem candidato fica aberta em vez de gerar um rookie.
    season_queries::insert_season(&conn, &Season::new("S002".to_string(), 2, 2025))
        .expect("season");

    let mut team_rng = StdRng::seed_from_u64(621);
    let team = sample_team("gt3", "T910", &mut team_rng);
    team_queries::insert_team(&conn, &team).expect("insert gt3 team");

    let mut existing = sample_driver(
        "P910",
        "Piloto Titular",
        Some("gt3"),
        72.0,
        DriverStatus::Ativo,
    );
    existing.stats_carreira.corridas = 80;
    driver_queries::insert_driver(&conn, &existing).expect("insert driver");

    let contract = Contract::new(
        "C910".to_string(),
        existing.id.clone(),
        existing.nome.clone(),
        team.id.clone(),
        team.nome.clone(),
        1,
        2,
        120_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &contract).expect("insert contract");
    team_queries::update_team_pilots(&conn, &team.id, Some(&existing.id), None)
        .expect("seed lineup");
    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         VALUES ('P910', '3', 'gt3', '2024-12-31T00:00:00', 2)",
        [],
    )
    .expect("insert license");
    conn.execute(
        "UPDATE meta SET value = '911' WHERE key = 'next_driver_id'",
        [],
    )
    .expect("driver counter");

    let teams = team_queries::get_all_teams(&conn).expect("teams");
    let mut report = MarketReport::default();
    let mut rng = StdRng::seed_from_u64(622);

    // Vaga não-estreia sem candidato (sem pool e sem feeder elegível): o fill
    // não aborta a temporada e completa o grid pelo fallback de emergência. O
    // essencial é não travar nem deixar o grid quebrado.
    fill_remaining_vacancies_with_rookies(
        &conn,
        &teams,
        2,
        &mut report,
        &mut rng,
        None,
        &HashSet::new(),
    )
    .expect("fill deve concluir sem abortar mesmo sem candidato");

    assert!(
        !find_vacancies(&conn)
            .expect("vacancies")
            .into_iter()
            .any(|vacancy| is_regular_vacancy(&vacancy)),
        "o grid deve terminar completo (sem vaga regular aberta)"
    );
}

#[test]
fn test_run_market_does_not_auto_sign_player() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");

    let previous = Season::new("S001".to_string(), 1, 2024);
    let next = Season::new("S002".to_string(), 2, 2025);
    season_queries::insert_season(&conn, &previous).expect("previous season");
    season_queries::finalize_season(&conn, &previous.id).expect("finalize previous");
    season_queries::insert_season(&conn, &next).expect("next season");

    let mut team_rng = StdRng::seed_from_u64(404);
    let current_team = sample_team("mazda_rookie", "T001", &mut team_rng);
    let vacancy_team = sample_team("mazda_rookie", "T002", &mut team_rng);
    team_queries::insert_team(&conn, &current_team).expect("current team");
    team_queries::insert_team(&conn, &vacancy_team).expect("vacancy team");

    let mut player = sample_driver(
        "P001",
        "Jogador",
        Some("mazda_rookie"),
        80.0,
        DriverStatus::Ativo,
    );
    player.is_jogador = true;
    let retired = sample_driver(
        "P002",
        "Veterano",
        Some("mazda_rookie"),
        55.0,
        DriverStatus::Aposentado,
    );
    driver_queries::insert_driver(&conn, &player).expect("insert player");
    driver_queries::insert_driver(&conn, &retired).expect("insert retired");

    let player_contract = Contract::new(
        "C001".to_string(),
        player.id.clone(),
        player.nome.clone(),
        current_team.id.clone(),
        current_team.nome.clone(),
        1,
        1,
        45_000.0,
        TeamRole::Numero1,
        "mazda_rookie".to_string(),
    );
    let retired_contract = Contract::new(
        "C002".to_string(),
        retired.id.clone(),
        retired.nome.clone(),
        vacancy_team.id.clone(),
        vacancy_team.nome.clone(),
        1,
        1,
        20_000.0,
        TeamRole::Numero1,
        "mazda_rookie".to_string(),
    );
    contract_queries::insert_contract(&conn, &player_contract).expect("insert player contract");
    contract_queries::insert_contract(&conn, &retired_contract).expect("insert retired contract");

    team_queries::update_team_pilots(&conn, &current_team.id, Some(&player.id), None)
        .expect("current team lineup");
    team_queries::update_team_pilots(&conn, &vacancy_team.id, Some(&retired.id), None)
        .expect("vacancy team lineup");

    insert_standing(
        &conn,
        &previous.id,
        &player.id,
        &current_team.id,
        "mazda_rookie",
        2,
        90.0,
        1,
        1,
    );

    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         VALUES ('P001', '1', 'mazda_rookie', '2024-12-31T00:00:00', 1)",
        [],
    )
    .expect("insert player license");
    conn.execute(
        "UPDATE meta SET value = '3' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");
    conn.execute(
        "UPDATE meta SET value = '3' WHERE key = 'next_driver_id'",
        [],
    )
    .expect("driver counter");

    let mut rng = StdRng::seed_from_u64(405);
    let report = run_market(&conn, 2, &mut rng).expect("market should run");
    let active_contracts = contract_queries::get_contracts_for_pilot(&conn, &player.id)
        .expect("player contracts")
        .into_iter()
        .filter(|contract| contract.status == ContractStatus::Ativo)
        .collect::<Vec<_>>();

    assert!(
        report
            .new_signings
            .iter()
            .all(|signing| signing.driver_id != player.id),
        "o mercado não deve auto-assinar o jogador"
    );
    assert!(
        active_contracts.is_empty(),
        "o jogador não deveria ganhar contrato automático; contratos ativos: {:?}",
        active_contracts
            .iter()
            .map(|contract| (&contract.id, &contract.equipe_id, &contract.categoria))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_player_reserved_seats_holds_multiple_for_free_agent() {
    // Design do usuário: quando o jogador está sem vaga, o mercado SEGURA alguns
    // assentos (2-3) que a carteira dele alcança — só vagas vazias, nunca dispensando
    // ninguém. Com contrato ativo, não segura nada.
    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("schema");
    conn.execute(
        "UPDATE meta SET value = '10' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");

    // Dois times gt3 vazios → 4 vagas regulares abertas.
    let mut rng = StdRng::seed_from_u64(880);
    let t1 = sample_team("gt3", "T001", &mut rng);
    let t2 = sample_team("gt3", "T002", &mut rng);
    team_queries::insert_team(&conn, &t1).expect("t1");
    team_queries::insert_team(&conn, &t2).expect("t2");

    // Jogador livre, licenciado pra gt3, com último contrato em gt3 (define o tier).
    let mut player = sample_driver("P001", "Jogador", None, 80.0, DriverStatus::Ativo);
    player.is_jogador = true;
    driver_queries::insert_driver(&conn, &player).expect("player");
    let old = Contract::new(
        "C001".to_string(),
        player.id.clone(),
        player.nome.clone(),
        t1.id.clone(),
        t1.nome.clone(),
        1,
        1,
        90_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &old).expect("old contract");
    contract_queries::update_contract_status(&conn, &old.id, &ContractStatus::Rescindido)
        .expect("rescind");
    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         VALUES ('P001', '3', 'gt3', '2024-12-31T00:00:00', 2)",
        [],
    )
    .expect("license");

    let seats = player_reserved_seats(&conn, 2).expect("seats");
    assert_eq!(
        seats.len(),
        MAX_PLAYER_RESERVED_SEATS,
        "deve segurar {MAX_PLAYER_RESERVED_SEATS} assentos pro agente livre licenciado, teve {}: {seats:?}",
        seats.len()
    );
    let uniq: std::collections::HashSet<_> = seats.iter().collect();
    assert_eq!(
        uniq.len(),
        seats.len(),
        "assentos reservados nao devem repetir"
    );

    // Com contrato ATIVO → não reserva nada (o mercado roda normal).
    let active = Contract::new(
        "C002".to_string(),
        player.id.clone(),
        player.nome.clone(),
        t1.id.clone(),
        t1.nome.clone(),
        2,
        3,
        90_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &active).expect("active contract");
    team_queries::update_team_pilots(&conn, &t1.id, Some(&player.id), None).expect("seat");
    assert!(
        player_reserved_seats(&conn, 2).expect("seats2").is_empty(),
        "jogador com contrato ativo nao deve ter assentos reservados"
    );
}

#[test]
fn test_pedigree_boost_is_bounded_and_monotonic() {
    // Rookie (índice 0) não ganha nada; boost cresce com o índice e satura no teto.
    assert_eq!(pedigree_boost_from_index(0.0), 0.0);
    assert_eq!(
        pedigree_boost_from_index(-50.0),
        0.0,
        "índice negativo tratado como 0"
    );
    let mid = pedigree_boost_from_index(PEDIGREE_BOOST_SCALE); // índice = escala → metade do teto
    assert!((mid - PEDIGREE_BOOST_MAX / 2.0).abs() < 1e-9);
    let strong = pedigree_boost_from_index(4.0 * PEDIGREE_BOOST_SCALE);
    assert!(strong > mid, "mais pedigree → mais boost");
    assert!(
        pedigree_boost_from_index(1_000_000.0) < PEDIGREE_BOOST_MAX,
        "nunca ultrapassa o teto"
    );
}

#[test]
fn test_generate_player_window_proposals_courts_free_agent_by_merit() {
    // Fase A: agente livre FORTE (bate o pool) recebe proposta formal ("Proposta
    // recebida"), o assento é segurado, é idempotente (não duplica), e jogador COM
    // contrato ativo não recebe nada.
    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("schema");
    conn.execute(
        "UPDATE meta SET value = '20' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");

    let s1 = Season::new("S001".to_string(), 1, 2024);
    let s2 = Season::new("S002".to_string(), 2, 2025);
    season_queries::insert_season(&conn, &s1).expect("s1");
    season_queries::finalize_season(&conn, &s1.id).expect("finalize s1");
    season_queries::insert_season(&conn, &s2).expect("s2");

    let mut rng = StdRng::seed_from_u64(915);
    let team = sample_team("gt3", "T001", &mut rng); // vazio → vagas gt3 abertas
    team_queries::insert_team(&conn, &team).expect("team");

    // Jogador livre, forte, licenciado gt3, com standing dominante na s1 (visibilidade alta).
    let mut player = sample_driver("P001", "Jogador", Some("gt3"), 95.0, DriverStatus::Ativo);
    player.is_jogador = true;
    driver_queries::insert_driver(&conn, &player).expect("player");
    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         VALUES ('P001', '3', 'gt3', '2024-12-31T00:00:00', 3)",
        [],
    )
    .expect("license");
    insert_standing(&conn, &s1.id, &player.id, &team.id, "gt3", 1, 400.0, 10, 8);

    let held = generate_player_window_proposals(&conn, 2, 1, &mut rng).expect("gen");
    assert!(
        !held.is_empty(),
        "jogador forte livre deve receber ao menos uma proposta formal: held={held:?}"
    );
    let pending = crate::db::queries::market_proposals::get_pending_player_proposals(
        &conn, &s2.id, &player.id,
    )
    .expect("pending");
    assert!(
        !pending.is_empty(),
        "deve persistir proposta pendente pro jogador"
    );

    // Idempotência: rodar de novo não duplica (mesmo ID) e segue segurando o assento.
    let held2 = generate_player_window_proposals(&conn, 2, 1, &mut rng).expect("gen2");
    let pending2 = crate::db::queries::market_proposals::get_pending_player_proposals(
        &conn, &s2.id, &player.id,
    )
    .expect("pending2");
    assert_eq!(
        pending.len(),
        pending2.len(),
        "nao deve duplicar propostas ao rodar de novo"
    );
    assert!(
        !held2.is_empty(),
        "deve continuar segurando o assento da proposta pendente"
    );

    // Jogador COM contrato ativo → nenhuma proposta formal na Fase A.
    let contract = Contract::new(
        "C100".to_string(),
        player.id.clone(),
        player.nome.clone(),
        team.id.clone(),
        team.nome.clone(),
        2,
        3,
        100_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &contract).expect("contract");
    team_queries::update_team_pilots(&conn, &team.id, Some(&player.id), None).expect("seat");
    let held3 = generate_player_window_proposals(&conn, 2, 1, &mut rng).expect("gen3");
    assert!(
        held3.is_empty(),
        "jogador com contrato ativo nao recebe proposta formal na Fase A"
    );
}

#[test]
fn test_player_window_proposals_expire_after_ttl() {
    // Fase B: proposta criada na semana 1 (prazo = 1 + PROPOSAL_TTL_WEEKS) expira na
    // semana do prazo e não é reoferecida (o assento deixa de ser segurado).
    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("schema");
    conn.execute(
        "UPDATE meta SET value = '30' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");

    let s1 = Season::new("S001".to_string(), 1, 2024);
    let s2 = Season::new("S002".to_string(), 2, 2025);
    season_queries::insert_season(&conn, &s1).expect("s1");
    season_queries::finalize_season(&conn, &s1.id).expect("finalize s1");
    season_queries::insert_season(&conn, &s2).expect("s2");

    let mut rng = StdRng::seed_from_u64(732);
    let team = sample_team("gt3", "T001", &mut rng);
    team_queries::insert_team(&conn, &team).expect("team");

    // Titular ocupa a N1 → sobra só UMA vaga (N2), pra o teste ser determinístico.
    let teammate = sample_driver("P002", "Titular", Some("gt3"), 70.0, DriverStatus::Ativo);
    driver_queries::insert_driver(&conn, &teammate).expect("teammate");
    team_queries::update_team_pilots(&conn, &team.id, Some(&teammate.id), None).expect("lineup");
    let tc = Contract::new(
        "C001".to_string(),
        teammate.id.clone(),
        teammate.nome.clone(),
        team.id.clone(),
        team.nome.clone(),
        1,
        3,
        90_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &tc).expect("teammate contract");

    let mut player = sample_driver("P001", "Jogador", Some("gt3"), 95.0, DriverStatus::Ativo);
    player.is_jogador = true;
    driver_queries::insert_driver(&conn, &player).expect("player");
    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         VALUES ('P001', '3', 'gt3', '2024-12-31T00:00:00', 3)",
        [],
    )
    .expect("license");
    insert_standing(&conn, &s1.id, &player.id, &team.id, "gt3", 1, 400.0, 10, 8);

    let held_w1 = generate_player_window_proposals(&conn, 2, 1, &mut rng).expect("w1");
    let pending_w1 = crate::db::queries::market_proposals::get_pending_player_proposals(
        &conn, &s2.id, &player.id,
    )
    .expect("p1");
    assert_eq!(pending_w1.len(), 1, "semana 1 cria a proposta");
    assert!(!held_w1.is_empty(), "semana 1 segura o assento da proposta");

    // Semana 4 = criada(1) + TTL(3): expira e não reoferece.
    let held_w4 = generate_player_window_proposals(&conn, 2, 4, &mut rng).expect("w4");
    let pending_w4 = crate::db::queries::market_proposals::get_pending_player_proposals(
        &conn, &s2.id, &player.id,
    )
    .expect("p4");
    assert!(
        pending_w4.is_empty(),
        "proposta expira na semana do prazo e não é reoferecida"
    );
    assert!(held_w4.is_empty(), "assento expirado não é mais segurado");
}

#[test]
fn test_free_player_without_categoria_atual_still_gets_offers_at_last_level() {
    // Regressão do bug relatado: jogador que fica um tempo sem correr tem o
    // `categoria_atual` limpo pelo sync do mercado (sync.rs zera quem não tem
    // contrato regular ativo). Antes, o tier dele caía a 0 (rookie) e o feed só
    // mostrava vagas de estreia — ocupadas por rookies reais → ZERO propostas para
    // sempre, mesmo com a escada segurando um assento invisível. Agora o tier vem
    // do ÚLTIMO contrato (gt3) e ele volta a receber oferta no nível dele.
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");
    conn.execute(
        "UPDATE meta SET value = '10' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");

    // Time gt3 (tier 4) com UMA vaga aberta (só o titular ocupa um assento).
    // Nenhum time rookie no mundo: sem a correção, o jogador não veria vaga alguma.
    let mut team_rng = StdRng::seed_from_u64(770);
    let gt3_team = sample_team("gt3", "T001", &mut team_rng);
    team_queries::insert_team(&conn, &gt3_team).expect("insert gt3 team");

    // Jogador livre e SEM categoria_atual (limpo pelo sync após ficar sem correr).
    let mut player = sample_driver("P001", "Jogador", None, 80.0, DriverStatus::Ativo);
    player.is_jogador = true;
    let teammate = sample_driver(
        "P002",
        "Titular GT3",
        Some("gt3"),
        78.0,
        DriverStatus::Ativo,
    );
    driver_queries::insert_driver(&conn, &player).expect("insert player");
    driver_queries::insert_driver(&conn, &teammate).expect("insert teammate");
    team_queries::update_team_pilots(&conn, &gt3_team.id, Some(&teammate.id), None)
        .expect("gt3 lineup deixa a vaga do jogador aberta");

    // Contrato ATIVO do titular (ancora o time) + contrato PASSADO (rescindido) do
    // jogador em gt3 — a única pista da categoria dele, já que categoria_atual é NULL.
    let teammate_contract = Contract::new(
        "C001".to_string(),
        teammate.id.clone(),
        teammate.nome.clone(),
        gt3_team.id.clone(),
        gt3_team.nome.clone(),
        1,
        3,
        120_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &teammate_contract).expect("teammate contract");
    let player_old_contract = Contract::new(
        "C002".to_string(),
        player.id.clone(),
        player.nome.clone(),
        gt3_team.id.clone(),
        gt3_team.nome.clone(),
        1,
        1,
        90_000.0,
        TeamRole::Numero2,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &player_old_contract).expect("player old contract");
    contract_queries::update_contract_status(
        &conn,
        &player_old_contract.id,
        &ContractStatus::Rescindido,
    )
    .expect("rescind player contract");

    // Licença do jogador cobre gt3 (nível 3).
    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         VALUES ('P001', '3', 'gt3', '2024-12-31T00:00:00', 2)",
        [],
    )
    .expect("insert player license");

    // Sanidade: jogador está mesmo livre.
    assert!(
        contract_queries::get_active_regular_contract_for_pilot(&conn, &player.id)
            .expect("check contract")
            .is_none(),
        "jogador deve estar sem contrato ativo"
    );

    let offers = player_market_offers(&conn, 2).expect("offers");
    assert!(
        offers.iter().any(|o| o.category == "gt3"),
        "jogador livre sem categoria_atual deve receber oferta de gt3 (pelo último \
         contrato), não zero propostas: {offers:?}"
    );

    // E deve conseguir ASSINAR a vaga ofertada (sign consistente com a oferta).
    let seat = offers[0].seat_id.clone();
    sign_player_to_vacancy(&conn, 2, &seat)
        .expect("jogador deve conseguir assinar a vaga que lhe foi ofertada");
}

#[test]
fn test_load_market_contexts_fails_on_corrupted_standings_row() {
    let conn = setup_market_fixture();
    conn.execute(
        "UPDATE standings
         SET categoria = CAST(X'00' AS BLOB)
         WHERE temporada_id = 'S001' AND piloto_id = 'P001'",
        [],
    )
    .expect("corrupt standings row");

    let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(&conn)
        .expect("drivers")
        .into_iter()
        .map(|driver| (driver.id.clone(), driver))
        .collect();
    let expiring_by_driver: HashMap<String, Contract> = HashMap::new();

    let result = load_market_contexts(&conn, Some("S001"), &drivers_by_id, &expiring_by_driver);

    let err = result.expect_err("corrupted standings should fail");
    assert!(err.contains("Falha ao ler categoria do standings"));
    assert!(err.contains("P001"));
}

#[test]
fn test_invalid_season_status_from_db_returns_error() {
    let conn = setup_market_fixture();
    conn.execute(
        "UPDATE seasons SET status = 'status_quebrado' WHERE numero = 2",
        [],
    )
    .expect("corrupt season status");

    let err = get_season_by_number(&conn, 2).expect_err("invalid season status should fail");
    assert!(err.contains("SeasonStatus inv"));
}

#[test]
fn test_sync_team_slots_fails_when_active_contract_points_to_missing_driver() {
    let conn = setup_market_fixture();
    conn.execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("disable foreign keys for corruption setup");
    conn.execute(
        "UPDATE contracts SET piloto_id = 'P999' WHERE id = 'C001'",
        [],
    )
    .expect("corrupt contract driver reference");
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .expect("re-enable foreign keys after corruption setup");

    let teams = team_queries::get_all_teams(&conn).expect("teams");
    let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(&conn)
        .expect("drivers")
        .into_iter()
        .map(|driver| (driver.id.clone(), driver))
        .collect();

    let err = sync_team_slots(&conn, &teams, &drivers_by_id)
        .expect_err("sync should fail for orphan active contract");

    assert!(err.contains("C001"));
    assert!(err.contains("P999"));
}

#[test]
fn test_run_market_repairs_legacy_missing_licenses_before_matching() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(406);

    let report = run_market(&conn, 2, &mut rng).expect("market should run");

    assert!(
        driver_has_required_license_for_category(&conn, "P002", "gt4")
            .expect("gt4 license for expiring veteran"),
        "veteranos de gt4 sem licenca coerente devem ser corrigidos antes do mercado"
    );
    assert!(
        driver_has_required_license_for_category(&conn, "P004", "gt4")
            .expect("gt4 license for free veteran"),
        "pilotos livres da categoria atual devem receber a licenca minima"
    );
    assert!(
        driver_has_required_license_for_category(&conn, "P006", "gt3")
            .expect("gt3 license for free veteran"),
        "pilotos ativos de categorias superiores tambem precisam ser reparados"
    );
    assert!(
        report.proposals_made > 0,
        "com as licencas legadas reparadas o mercado precisa voltar a gerar propostas reais"
    );
}

#[test]
fn test_sign_driver_to_team_rolls_back_contract_when_driver_update_fails() {
    let conn = setup_market_fixture();
    let vacancy = find_vacancies(&conn)
        .expect("vacancies")
        .into_iter()
        .find(|vacancy| vacancy.team_id == "T002" && vacancy.papel_necessario == TeamRole::Numero2)
        .expect("target vacancy");
    let driver = driver_queries::get_all_drivers(&conn)
        .expect("drivers query")
        .into_iter()
        .find(|driver| driver.id == "P004")
        .expect("existing driver");

    conn.execute(
        "CREATE TRIGGER fail_driver_update
         BEFORE UPDATE ON drivers
         WHEN NEW.id = 'P004'
         BEGIN
             SELECT RAISE(ABORT, 'driver update blocked');
         END;",
        [],
    )
    .expect("create trigger");

    let err = sign_driver_to_team(
        &conn,
        &driver,
        &vacancy,
        2,
        calculate_offer_salary(&vacancy, &driver, &mut StdRng::seed_from_u64(7)),
        1,
        TeamRole::Numero2,
    )
    .expect_err("signing should fail");

    assert!(
        !err.is_empty(),
        "a falha precisa ser propagada quando o update do piloto nao puder ser aplicado"
    );
    let active_contracts = contract_queries::get_contracts_for_pilot(&conn, "P004")
        .expect("contracts for pilot")
        .into_iter()
        .filter(|contract| {
            contract.status == ContractStatus::Ativo && contract.temporada_inicio == 2
        })
        .collect::<Vec<_>>();
    assert!(
        active_contracts.is_empty(),
        "a assinatura deve ser atomica e nao deixar contrato ativo apos falha no update do piloto"
    );
}

#[test]
fn ordinary_transfer_seeds_team_rivalry_only_on_fresh_departure() {
    // Fonte 2 (Elo 2) na transferência NORMAL: um piloto de calibre que correu por T002 e
    // terminou na temporada 1, ao assinar com T001 na temporada 2 (saída FRESCA), semeia
    // rivalidade de mercado entre os dois times. Uma saída ANTIGA (não-fresca) não semeia.
    let build = || {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        let mut rng = StdRng::seed_from_u64(9);
        let team_a = sample_team("gt4", "T001", &mut rng);
        let team_b = sample_team("gt4", "T002", &mut rng);
        team_queries::insert_team(&conn, &team_a).expect("t001");
        team_queries::insert_team(&conn, &team_b).expect("t002");
        let driver = sample_driver("P001", "Astro", Some("gt4"), 88.0, DriverStatus::Ativo);
        driver_queries::insert_driver(&conn, &driver).expect("driver");
        // Contrato antigo em T002 começando na temporada 1, duração 1 → termina na temporada 1.
        let old = Contract::new(
            "C001".to_string(),
            driver.id.clone(),
            driver.nome.clone(),
            team_b.id.clone(),
            team_b.nome.clone(),
            1,
            1,
            120_000.0,
            TeamRole::Numero1,
            "gt4".to_string(),
        );
        contract_queries::insert_contract(&conn, &old).expect("old contract");
        (conn, driver)
    };

    // FRESCA: assina T001 na temporada 2, saída de T002 terminou na temporada 1 (== 2-1).
    let (conn, driver) = build();
    assert!(
        crate::rivalry::team::get_team_rivalries(&conn, "T001")
            .expect("riv")
            .is_empty(),
        "sem rivalidade antes da transferência"
    );
    seed_ordinary_transfer_rivalry(&conn, &driver, "T001", 2);
    assert!(
        !crate::rivalry::team::get_team_rivalries(&conn, "T001")
            .expect("riv")
            .is_empty(),
        "transferência fresca T002→T001 deve semear rivalidade de mercado"
    );

    // ANTIGA: mesmo contrato (termina na 1), mas assinando na temporada 5 (2-1 ≠ 1) → nada.
    let (conn2, driver2) = build();
    seed_ordinary_transfer_rivalry(&conn2, &driver2, "T001", 5);
    assert!(
        crate::rivalry::team::get_team_rivalries(&conn2, "T001")
            .expect("riv")
            .is_empty(),
        "saída não-fresca (de temporadas atrás) não deve semear rivalidade"
    );
}

#[test]
fn test_run_market_rolls_back_when_market_persist_fails() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(407);

    conn.execute(
        "CREATE TRIGGER fail_market_insert
         BEFORE INSERT ON market
         BEGIN
             SELECT RAISE(ABORT, 'market persist blocked');
         END;",
        [],
    )
    .expect("create trigger");

    let err = run_market(&conn, 2, &mut rng).expect_err("market should fail late");
    assert!(err.contains("market persist blocked"));

    let status_c002: String = conn
        .query_row(
            "SELECT status FROM contracts WHERE id = 'C002'",
            [],
            |row| row.get(0),
        )
        .expect("contract status");
    assert_eq!(
        status_c002, "Ativo",
        "a expiracao de contratos deve ser revertida quando a persistencia final falhar"
    );

    let season_market_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM market WHERE temporada_id = 'S002'",
            [],
            |row| row.get(0),
        )
        .expect("market rows");
    assert_eq!(season_market_rows, 0);
}

fn setup_market_fixture() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");

    let previous = Season::new("S001".to_string(), 1, 2024);
    let next = Season::new("S002".to_string(), 2, 2025);
    season_queries::insert_season(&conn, &previous).expect("previous season");
    season_queries::finalize_season(&conn, &previous.id).expect("finalize previous");
    season_queries::insert_season(&conn, &next).expect("next season");

    let mut team_rng = StdRng::seed_from_u64(200);
    let team_a = sample_team("gt4", "T001", &mut team_rng);
    let team_b = sample_team("gt4", "T002", &mut team_rng);
    team_queries::insert_team(&conn, &team_a).expect("team a");
    team_queries::insert_team(&conn, &team_b).expect("team b");

    let driver_a = sample_driver("P001", "Piloto A", Some("gt4"), 78.0, DriverStatus::Ativo);
    let driver_b = sample_driver("P002", "Piloto B", Some("gt4"), 66.0, DriverStatus::Ativo);
    let driver_c = sample_driver(
        "P003",
        "Piloto C",
        Some("gt4"),
        62.0,
        DriverStatus::Aposentado,
    );
    let driver_d = sample_driver("P004", "Piloto D", Some("gt4"), 74.0, DriverStatus::Ativo);
    let driver_e = sample_driver("P005", "Piloto E", None, 59.0, DriverStatus::Ativo);
    let driver_f = sample_driver("P006", "Piloto F", Some("gt3"), 76.0, DriverStatus::Ativo);
    for driver in [
        &driver_a, &driver_b, &driver_c, &driver_d, &driver_e, &driver_f,
    ] {
        driver_queries::insert_driver(&conn, driver).expect("insert driver");
    }

    let contract_a = Contract::new(
        "C001".to_string(),
        driver_a.id.clone(),
        driver_a.nome.clone(),
        team_a.id.clone(),
        team_a.nome.clone(),
        1,
        2,
        140_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );
    let contract_b = Contract::new(
        "C002".to_string(),
        driver_b.id.clone(),
        driver_b.nome.clone(),
        team_a.id.clone(),
        team_a.nome.clone(),
        1,
        1,
        95_000.0,
        TeamRole::Numero2,
        "gt4".to_string(),
    );
    let contract_c = Contract::new(
        "C003".to_string(),
        driver_c.id.clone(),
        driver_c.nome.clone(),
        team_b.id.clone(),
        team_b.nome.clone(),
        1,
        2,
        85_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );
    contract_queries::insert_contract(&conn, &contract_a).expect("contract a");
    contract_queries::insert_contract(&conn, &contract_b).expect("contract b");
    contract_queries::insert_contract(&conn, &contract_c).expect("contract c");

    team_queries::update_team_pilots(&conn, &team_a.id, Some(&driver_a.id), Some(&driver_b.id))
        .expect("team a pilots");
    team_queries::update_team_pilots(&conn, &team_b.id, Some(&driver_c.id), None)
        .expect("team b pilots");

    insert_standing(
        &conn,
        &previous.id,
        &driver_a.id,
        &team_a.id,
        "gt4",
        1,
        120.0,
        3,
        2,
    );
    insert_standing(
        &conn,
        &previous.id,
        &driver_b.id,
        &team_a.id,
        "gt4",
        4,
        72.0,
        1,
        1,
    );
    insert_standing(
        &conn,
        &previous.id,
        &driver_c.id,
        &team_b.id,
        "gt4",
        6,
        40.0,
        0,
        0,
    );
    insert_standing(
        &conn,
        &previous.id,
        &driver_d.id,
        &team_b.id,
        "gt4",
        2,
        96.0,
        2,
        1,
    );
    insert_standing(
        &conn,
        &previous.id,
        &driver_f.id,
        &team_a.id,
        "gt3",
        3,
        88.0,
        1,
        2,
    );

    conn.execute(
        "UPDATE meta SET value = '4' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");
    conn.execute(
        "UPDATE meta SET value = '7' WHERE key = 'next_driver_id'",
        [],
    )
    .expect("driver counter");

    conn
}

fn sample_team(category: &str, id: &str, rng: &mut StdRng) -> crate::models::team::Team {
    let template = get_team_templates(category)[0];
    crate::models::team::Team::from_template_with_rng(template, category, id.to_string(), 2025, rng)
}

fn sample_driver(
    id: &str,
    name: &str,
    category: Option<&str>,
    skill: f64,
    status: DriverStatus,
) -> Driver {
    let mut driver = Driver::new(
        id.to_string(),
        name.to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        24,
        2020,
    );
    driver.categoria_atual = category.map(str::to_string);
    driver.status = status;
    driver.atributos.skill = skill;
    driver.atributos.consistencia = 68.0;
    driver.stats_temporada.vitorias = 1;
    driver.stats_temporada.poles = 1;
    driver.stats_carreira.corridas = 40;
    driver.stats_carreira.temporadas = 5;
    driver.stats_carreira.titulos = 1;
    driver
}

#[test]
fn slam_history_feeds_brain_to_chase_next_base() {
    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("schema");
    let mut driver = sample_driver(
        "P100",
        "Chaser",
        Some("mazda_amador"),
        67.0,
        DriverStatus::Ativo,
    );
    driver.personalidade_primaria = Some(PrimaryPersonality::Ambicioso);
    driver_queries::insert_driver(&conn, &driver).expect("driver");

    let insert = |season: i32, categoria: &str, pos: i32, snap: &str| {
        conn.execute(
            "INSERT INTO driver_season_archive
             (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params!["P100", season, 2020 + season, "Chaser", categoria, pos, 120.0, snap],
        )
        .expect("archive row");
    };
    insert(
        1,
        "mazda_amador",
        1,
        r#"{"categoria":"mazda_amador","posicao_campeonato":1,"titulos":1,"corridas":8,"vitorias":4}"#,
    );
    insert(
        2,
        "toyota_amador",
        1,
        r#"{"categoria":"toyota_amador","posicao_campeonato":1,"titulos":1,"corridas":8,"vitorias":5}"#,
    );

    let (history, current_results) = read_slam_history(&conn, &driver).expect("history");
    // 2 títulos no histórico; current_results só conta a categoria atual (mazda_amador).
    assert_eq!(history.len(), 2);
    assert_eq!(current_results, vec![true]);

    // Cup precisa de mazda + toyota + bmw → o cérebro manda caçar bmw_m2.
    let decision = slam_ambition::decide(
        &history,
        "mazda_amador",
        driver.atributos.skill,
        true,
        &current_results,
    );
    match decision {
        Some(SlamDecision::Chase { category, .. }) => assert_eq!(category, "bmw_m2"),
        other => panic!("esperava Chase bmw_m2, veio {other:?}"),
    }
}

#[test]
fn interactive_window_persists_and_closes() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(2);
    let season = 2;
    // Inicia + persiste a janela.
    let state = window_get_or_init(&conn, season, &mut rng).expect("init");
    let _ = state.week();
    assert!(
        load_window(&conn, season).expect("load").is_some(),
        "janela deve estar persistida após init"
    );
    // Avança até fechar (jogador sempre espera).
    let mut guard = 0;
    loop {
        let s = window_advance(&conn, season, None, &mut rng).expect("advance");
        if s.is_closed() {
            break;
        }
        guard += 1;
        assert!(guard < 30, "janela não fechou em tempo razoável");
    }
    // Persistida como fechada.
    let loaded = load_window(&conn, season)
        .expect("load")
        .expect("janela existe");
    assert!(loaded.is_closed(), "janela deve estar fechada após o ciclo");
}

fn insert_standing(
    conn: &Connection,
    season_id: &str,
    driver_id: &str,
    team_id: &str,
    category: &str,
    position: i32,
    points: f64,
    wins: i32,
    poles: i32,
) {
    conn.execute(
        "INSERT INTO standings (
            temporada_id, piloto_id, equipe_id, categoria, posicao, pontos, vitorias, podios, poles, corridas
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![season_id, driver_id, team_id, category, position, points, wins, wins + 1, poles, 8],
    )
    .expect("insert standing");
}
