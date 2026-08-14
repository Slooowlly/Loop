//! Integridade: o que o mercado faz quando o dado está corrompido, o passo falha no
//! meio ou o sincronismo de assento encontra contrato inconsistente. Mais a i18n.

use super::super::*;
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
