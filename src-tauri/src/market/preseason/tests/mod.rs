//! Suíte de testes da pré-temporada (extraída de `preseason.rs`).
//!
//! Continua sendo o mesmo módulo `tests` de antes: `use super::*` enxerga o
//! módulo `preseason` inteiro, incluindo os itens privados.

use std::fs;
use std::path::PathBuf;

use rand::{rngs::StdRng, SeedableRng};
use rusqlite::{params, Connection};

use super::*;
use crate::calendar::CalendarEntry;
use crate::constants::teams::get_team_templates;
use crate::db::migrations;
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::finance::planning::derive_budget_index_from_money;
use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::enums::{
    DriverStatus, RaceStatus, SeasonPhase, TeamRole, ThematicSlot, WeatherCondition,
};
use crate::models::season::Season;

fn sample_calendar_entry(
    id: &str,
    season_id: &str,
    category: &str,
    rodada: i32,
    track_id: u32,
) -> CalendarEntry {
    CalendarEntry {
        id: id.to_string(),
        season_id: season_id.to_string(),
        categoria: category.to_string(),
        rodada,
        nome: format!("Round {rodada}"),
        track_id,
        track_name: format!("Track {track_id}"),
        track_config: "Full".to_string(),
        clima: WeatherCondition::Dry,
        temperatura: 22.0,
        voltas: 20,
        duracao_corrida_min: 30,
        duracao_classificacao_min: 15,
        status: RaceStatus::Pendente,
        horario: "14:00".to_string(),
        week_of_year: rodada,
        season_phase: SeasonPhase::BlocoRegular,
        display_date: "2025-02-01".to_string(),
        thematic_slot: ThematicSlot::NaoClassificado,
        season_week: None,
    }
}

#[test]
fn test_initialize_preseason_creates_plan() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(500);

    let plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

    assert_eq!(plan.state.season_number, 2);
    assert_eq!(plan.state.current_week, 1);
    assert_eq!(plan.state.phase, PreSeasonPhase::Transfers);
    assert!(!plan.state.is_complete);
    // A Janela de Transferências É o mercado — sem timeline de replay agendada.
    assert!(plan.planned_events.is_empty());
}

#[test]
fn test_initialize_preseason_reaches_complete_via_window() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(501);

    let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");
    assert_eq!(plan.state.phase, PreSeasonPhase::Transfers);
    let mut guard = 0;
    while !plan.state.is_complete {
        advance_week(&conn, &mut plan, None).expect("week should advance");
        guard += 1;
        assert!(guard < 30, "a janela deve fechar em tempo razoavel");
    }
    assert_eq!(plan.state.phase, PreSeasonPhase::Complete);
}

#[test]
fn test_plan_total_weeks_reasonable() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(502);

    let plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

    assert_eq!(
        plan.state.total_weeks,
        i32::from(crate::constants::timeline::MARKET_DURATION_WEEKS)
    );
}

#[test]
fn test_initialize_preseason_recalculates_pit_and_strategy() {
    let conn = setup_market_fixture();
    for entry in [
        sample_calendar_entry("R101", "S002", "gt4", 1, 93),
        sample_calendar_entry("R102", "S002", "gt4", 2, 287),
        sample_calendar_entry("R103", "S002", "gt4", 3, 188),
        sample_calendar_entry("R104", "S002", "gt4", 4, 397),
    ] {
        calendar_queries::insert_calendar_entry(&conn, &entry).expect("insert calendar entry");
    }

    let mut team_a = team_queries::get_team_by_id(&conn, "T001")
        .expect("load team a")
        .expect("team a exists");
    team_a.car_performance = 12.0;
    team_a.budget = 85.0;
    team_a.engineering = 82.0;
    team_a.facilities = 80.0;
    team_queries::update_team(&conn, &team_a).expect("update team a");

    let mut team_b = team_queries::get_team_by_id(&conn, "T002")
        .expect("load team b")
        .expect("team b exists");
    team_b.car_performance = 4.0;
    team_b.budget = 18.0;
    team_b.cash_balance = 2_500_000.0;
    team_b.engineering = 35.0;
    team_b.facilities = 30.0;
    team_queries::update_team(&conn, &team_b).expect("update team b");

    let mut rng = StdRng::seed_from_u64(502);
    let _plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

    let updated_team_b = team_queries::get_team_by_id(&conn, "T002")
        .expect("reload team b")
        .expect("team b exists after preseason");
    let updated_team_a = team_queries::get_team_by_id(&conn, "T001")
        .expect("reload team a")
        .expect("team a exists after preseason");
    let expected_budget = derive_budget_index_from_money(&updated_team_b);
    assert!(
        (updated_team_b.budget - expected_budget).abs() < 0.0001,
        "expected derived budget {expected_budget}, got {}",
        updated_team_b.budget
    );
    assert!(
        updated_team_b.pit_strategy_risk > updated_team_a.pit_strategy_risk,
        "backmarker should carry more pit risk: weak={} strong={}",
        updated_team_b.pit_strategy_risk,
        updated_team_a.pit_strategy_risk
    );
    assert!(
        updated_team_a.pit_crew_quality > updated_team_b.pit_crew_quality,
        "richer team should keep stronger pit crew: strong={} weak={}",
        updated_team_a.pit_crew_quality,
        updated_team_b.pit_crew_quality
    );
    // Pilar C: a season_strategy agora vem do plano de 3 temporadas (a
    // seleção é testada em finance::strategy). Aqui garantimos que o plano
    // rodou e produziu uma estratégia válida, e que o time forte/rico não
    // cai em modo de contenção (austeridade/sobrevivência).
    let valid = ["balanced", "all_in", "expansion", "austerity", "survival"];
    assert!(valid.contains(&updated_team_a.season_strategy.as_str()));
    assert!(valid.contains(&updated_team_b.season_strategy.as_str()));
    assert!(
        !matches!(
            updated_team_a.season_strategy.as_str(),
            "survival" | "austerity"
        ),
        "time forte/rico nao deveria entrar em contencao, veio {}",
        updated_team_a.season_strategy
    );
}

#[test]
fn test_initialize_preseason_applies_financial_crisis_drag_to_team_quality() {
    let conn = setup_market_fixture();
    for entry in [
        sample_calendar_entry("R201", "S002", "gt4", 1, 93),
        sample_calendar_entry("R202", "S002", "gt4", 2, 397),
    ] {
        calendar_queries::insert_calendar_entry(&conn, &entry).expect("insert calendar entry");
    }

    let mut team = team_queries::get_team_by_id(&conn, "T002")
        .expect("load team")
        .expect("team exists");
    team.confiabilidade = 70.0;
    team.engineering = 45.0;
    team.facilities = 45.0;
    team.cash_balance = -100_000.0;
    team.debt_balance = 900_000.0;
    team.financial_state = "collapse".to_string();
    team.season_strategy = "survival".to_string();
    team_queries::update_team(&conn, &team).expect("update crisis team");

    let mut rng = StdRng::seed_from_u64(506);
    let _plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

    let updated_team = team_queries::get_team_by_id(&conn, "T002")
        .expect("reload team")
        .expect("team exists after preseason");

    assert!(updated_team.confiabilidade < 70.0);
    assert!(updated_team.engineering < 45.0);
    assert!(updated_team.facilities < 45.0);
}

#[test]
fn test_advance_week_executes_events() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(503);
    let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

    let result = advance_week(&conn, &mut plan, None).expect("week should advance");

    assert_eq!(result.week_number, 1);
    // A semana avançou pela janela (seguiu p/ a próxima ou fechou).
    assert!(plan.state.current_week >= 2 || plan.state.is_complete);
}

#[test]
fn test_advance_week_increments_week() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(504);
    let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

    advance_week(&conn, &mut plan, None).expect("week should advance");

    assert_eq!(plan.state.current_week, 2);
}

#[test]
fn test_advance_week_phase_transitions() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(505);
    let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

    while !plan.state.is_complete {
        let result = advance_week(&conn, &mut plan, None).expect("week should advance");
        // Durante a janela a fase é sempre Transfers até fechar.
        assert_eq!(result.phase, PreSeasonPhase::Transfers);
    }

    assert_eq!(plan.state.phase, PreSeasonPhase::Complete);
}

#[test]
fn test_initialize_preseason_expires_ending_contracts_immediately() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(5061);

    let active_before = contract_queries::get_active_regular_contract_for_pilot(&conn, "P007")
        .expect("active contract query before preseason")
        .expect("player should start with active contract");
    assert_eq!(active_before.id, "C004");

    let plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

    let active_after = contract_queries::get_active_regular_contract_for_pilot(&conn, "P007")
        .expect("active contract query after preseason");
    assert!(
        active_after.is_none(),
        "piloto com contrato encerrado na temporada anterior deve entrar na pre-temporada sem contrato ativo"
    );

    let player_contract_status: String = conn
        .query_row(
            "SELECT status FROM contracts WHERE id = 'C004'",
            [],
            |row| row.get(0),
        )
        .expect("player contract status");
    assert_ne!(player_contract_status, "Ativo");

    assert!(
        !plan.planned_events.iter().any(|event| {
            event.week > 1
                && matches!(event.event, PendingAction::ExpireContract { .. })
        }),
        "expiracoes de contrato nao devem ficar adiadas para semanas posteriores ao inicio da janela"
    );
}

#[test]
fn test_renewal_week() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(507);
    let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

    while !plan.state.is_complete {
        let result = advance_week(&conn, &mut plan, None).expect("week should advance");
        if result
            .events
            .iter()
            .any(|event| event.event_type == MarketEventType::ContractRenewed)
        {
            break;
        }
    }

    let renewed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM contracts WHERE piloto_id = 'P001' AND temporada_inicio = 2 AND status = 'Ativo'",
            [],
            |row| row.get(0),
        )
        .expect("renewed count");
    assert_eq!(renewed, 1);
}

#[test]
fn test_initialize_preseason_does_not_schedule_automatic_move_for_player() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(507);
    let plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");
    let _player = driver_queries::get_player_driver(&conn).expect("player");

    // Sem timeline de replay: o jogador nunca é movido por evento automático
    // agendado — ele decide pela Janela de Transferências (player_choice).
    assert!(
        plan.planned_events.is_empty(),
        "a pre-temporada não deve agendar nenhum movimento automático; a janela é o mercado"
    );
}

#[test]
fn test_transfer_week() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(508);
    let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

    while !plan.state.is_complete {
        let result = advance_week(&conn, &mut plan, None).expect("week should advance");
        if result
            .events
            .iter()
            .any(|event| event.event_type == MarketEventType::TransferCompleted)
        {
            break;
        }
    }

    let team = team_queries::get_team_by_id(&conn, "T002")
        .expect("team query")
        .expect("team");
    assert!(team.piloto_1_id.is_some() || team.piloto_2_id.is_some());
}

#[test]
fn test_rookie_placement_week() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(509);
    let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

    while !plan.state.is_complete {
        let result = advance_week(&conn, &mut plan, None).expect("week should advance");
        if result
            .events
            .iter()
            .any(|event| event.event_type == MarketEventType::RookieSigned)
        {
            break;
        }
    }

    let rookie_contracts: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM contracts WHERE temporada_inicio = 2 AND status = 'Ativo'",
            [],
            |row| row.get(0),
        )
        .expect("rookie contracts");
    assert!(rookie_contracts >= 2);
}

#[test]
fn test_all_teams_filled_after_complete() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(510);
    let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

    while !plan.state.is_complete {
        advance_week(&conn, &mut plan, None).expect("week should advance");
    }
    // As vagas que sobraram após a janela são preenchidas no finalize (rookies).
    crate::market::pipeline::fill_all_remaining_vacancies(&conn, 2, &mut rng)
        .expect("fill remaining at finalize");

    let teams = team_queries::get_all_teams(&conn).expect("teams");
    assert!(teams
        .iter()
        .all(|team| team.piloto_1_id.is_some() && team.piloto_2_id.is_some()));
}

#[test]
fn test_plan_persistence() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(511);
    let plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");
    let temp_dir = unique_test_dir("preseason_persistence");
    fs::create_dir_all(&temp_dir).expect("temp dir");

    save_preseason_plan(&temp_dir, &plan).expect("plan should save");
    let loaded = load_preseason_plan(&temp_dir)
        .expect("plan should load")
        .expect("plan should exist");

    assert_eq!(loaded.state.season_number, plan.state.season_number);
    assert_eq!(loaded.state.total_weeks, plan.state.total_weeks);
    assert_eq!(loaded.planned_events.len(), plan.planned_events.len());

    delete_preseason_plan(&temp_dir).expect("delete plan");
    assert!(load_preseason_plan(&temp_dir)
        .expect("load after delete")
        .is_none());

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_temp_preseason_clone_cleans_up_file_on_drop() {
    let conn = setup_market_fixture();
    let temp_path;
    let wal_path;
    let shm_path;

    {
        let clone = TempPreseasonClone::new(&conn).expect("temp clone");
        temp_path = clone.path().to_path_buf();
        wal_path = PathBuf::from(format!("{}-wal", temp_path.to_string_lossy()));
        shm_path = PathBuf::from(format!("{}-shm", temp_path.to_string_lossy()));

        assert!(
            temp_path.exists(),
            "temp clone should exist while guard is alive"
        );

        let contract_count: i64 = clone
            .connection()
            .query_row("SELECT COUNT(*) FROM contracts", [], |row| row.get(0))
            .expect("count contracts from temp clone");
        assert!(contract_count > 0, "temp clone should be readable");
    }

    assert!(
        !temp_path.exists(),
        "temp clone file should be removed after guard drop: {}",
        temp_path.display()
    );
    assert!(
        !wal_path.exists(),
        "temp clone wal file should be removed after guard drop: {}",
        wal_path.display()
    );
    assert!(
        !shm_path.exists(),
        "temp clone shm file should be removed after guard drop: {}",
        shm_path.display()
    );
}

#[test]
fn test_next_preseason_clone_path_is_unique_on_rapid_calls() {
    let mut seen = std::collections::HashSet::new();

    for _ in 0..128 {
        let path = next_preseason_clone_path().expect("unique clone path");
        assert!(
            seen.insert(path.clone()),
            "clone path duplicado gerado em chamadas rapidas: {}",
            path.display()
        );
    }
}

#[test]
fn test_cannot_advance_after_complete() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(512);
    let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

    while !plan.state.is_complete {
        advance_week(&conn, &mut plan, None).expect("week should advance");
    }

    let error = advance_week(&conn, &mut plan, None).expect_err("should reject after complete");
    assert!(error.contains("completa"));
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
    let mut player = sample_driver("P007", "Jogador", Some("gt4"), 72.0, DriverStatus::Ativo);
    player.is_jogador = true;
    for driver in [
        &driver_a, &driver_b, &driver_c, &driver_d, &driver_e, &driver_f, &player,
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
        1,
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
    let contract_d = Contract::new(
        "C004".to_string(),
        player.id.clone(),
        player.nome.clone(),
        team_b.id.clone(),
        team_b.nome.clone(),
        1,
        1,
        90_000.0,
        TeamRole::Numero2,
        "gt4".to_string(),
    );
    contract_queries::insert_contract(&conn, &contract_a).expect("contract a");
    contract_queries::insert_contract(&conn, &contract_b).expect("contract b");
    contract_queries::insert_contract(&conn, &contract_c).expect("contract c");
    contract_queries::insert_contract(&conn, &contract_d).expect("contract d");

    team_queries::update_team_pilots(&conn, &team_a.id, Some(&driver_a.id), Some(&driver_b.id))
        .expect("team a pilots");
    team_queries::update_team_pilots(&conn, &team_b.id, Some(&driver_c.id), Some(&player.id))
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
    insert_standing(
        &conn,
        &previous.id,
        &player.id,
        &team_b.id,
        "gt4",
        5,
        60.0,
        0,
        0,
    );

    // Licenças — necessárias para que o filtro de mercado não bloqueie os pilotos.
    // gt4 exige nível 2, gt3 exige nível 3.
    for (piloto_id, nivel) in [
        ("P001", 2),
        ("P002", 2),
        ("P003", 2),
        ("P004", 2),
        ("P005", 0),
        ("P006", 3),
        ("P007", 2),
    ] {
        conn.execute(
            "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
             VALUES (?1, ?2, 'gt4', '2024', 3)",
            params![piloto_id, nivel.to_string()],
        )
        .expect("insert license");
    }

    conn.execute(
        "UPDATE meta SET value = '5' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");
    conn.execute(
        "UPDATE meta SET value = '8' WHERE key = 'next_driver_id'",
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
    driver.stats_carreira.titulos = 1;
    driver
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

fn unique_test_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("iracerapp_{label}_{nanos}"))
}
