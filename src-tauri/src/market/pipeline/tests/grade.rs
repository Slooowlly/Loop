//! A passada completa do mercado sobre a grade: nenhuma vaga sobra, contrato vencido
//! expira, toda equipe fecha com dois pilotos e as categorias de classe (endurance/
//! production) recebem contrato regular com a classe certa.

use super::super::*;
use super::*;
#[test]
fn test_market_fills_all_vacancies() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(300);

    let report = run_market(&conn, 2, &mut rng).expect("market should run");

    assert_eq!(report.unresolved_vacancies, 0);
    assert!(find_vacancies(&conn).expect("vacancies").is_empty());
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

/// O `run_market` acima resolve o mercado num passo só, e NENHUM caminho de produção faz
/// isso: a pré-temporada parcela o mesmo trabalho em semanas (`run_market_prepasses` →
/// `run_market_movements` → o preenchimento do fechamento). Sem este teste, a invariante
/// mais cara do modelo fechado — nenhum assento vazio quando a temporada começa — só
/// estaria coberta pelo caminho que o jogo não percorre.
#[test]
fn sequencia_de_producao_tambem_fecha_a_grade() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(4141);
    let mut report = MarketReport::default();

    // Semana 1: só o futuro de quem já tem assento (expira, rescinde, renova).
    let contratos = run_market_prepasses(&conn, 2, &mut rng).expect("pré-passes de contratos");
    report.new_signings.extend(contratos.new_signings);

    // Semana da abertura: os movimentos entre equipes que a IA resolve.
    let movimentos = run_market_movements(&conn, 2, &mut rng).expect("passada de movimentos");
    report.new_signings.extend(movimentos.new_signings);

    // Fechamento: a escada preenche o que sobrou.
    fill_all_remaining_vacancies_reported(&conn, 2, &mut rng, &mut report)
        .expect("preenchimento final");

    let sobraram = find_vacancies(&conn).expect("vagas");
    let regulares: Vec<_> = sobraram.into_iter().filter(is_regular_vacancy).collect();
    assert!(
        regulares.is_empty(),
        "a sequência de produção deixou assento vazio: {:?}",
        regulares
            .iter()
            .map(|v| (v.team_name.as_str(), v.categoria.as_str()))
            .collect::<Vec<_>>()
    );
}
