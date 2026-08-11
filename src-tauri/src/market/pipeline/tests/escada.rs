//! Escada fechada: promoção do feeder, recrutamento profundo, pool de resgate e a
//! seleção de assento por prestígio/affordability.
//!
//! É o coração da garantia de grid cheio — `consolidacao.rs`.

use super::super::*;
use super::*;

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

