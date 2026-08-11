//! Suíte de testes da virada de temporada (extraída de `evolution/pipeline.rs`).
//!
//! `use super::*` enxerga o módulo `pipeline` inteiro, incluindo os itens privados.

/// Guard de ordem dos passos da virada (lê o fonte de `orquestracao.rs`).
mod ordem;

use rand::{rngs::StdRng, SeedableRng};
use rusqlite::Connection;

use super::*;
use crate::calendar::generate_calendar_for_category;
use crate::constants::teams::get_team_templates;
use crate::db::migrations;
use crate::db::queries::calendar as calendar_queries;
use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::enums::{ContractType, DriverStatus, TeamRole};
use crate::models::team::Team;

#[test]
fn test_end_of_season_increments_year() {
    let (mut conn, season) = setup_pipeline_fixture();
    let save_path = unique_test_dir("eos_year");

    let result = run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

    assert_eq!(result.new_year, season.ano + 1);
    assert!(
        result.promotion_result.errors.is_empty(),
        "promotion/relegation should keep invariants in fixture: {:?}",
        result.promotion_result.errors
    );
    assert!(result.preseason_initialized);
    assert!(result.preseason_total_weeks >= 3);
    let meta_year: String = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'current_year'",
            [],
            |row| row.get(0),
        )
        .expect("meta current year");
    assert_eq!(meta_year, (season.ano + 1).to_string());
    assert!(save_path.join("preseason_plan.json").exists());
    let _ = std::fs::remove_dir_all(save_path);
}

#[test]
fn end_of_season_does_not_double_count_the_season_in_career_stats() {
    let (mut conn, season) = setup_pipeline_fixture();
    let save_path = unique_test_dir("eos_no_double_count");

    // Estado de quem acabou de correr a temporada: `commands::race` já somou
    // cada corrida NA CARREIRA, corrida a corrida. A carreira aqui é o espelho
    // exato da temporada — é assim que o piloto chega na virada do ano.
    let mut driver = driver_queries::get_driver(&conn, "P001").expect("driver");
    driver.stats_carreira.pontos_total = driver.stats_temporada.pontos;
    driver.stats_carreira.vitorias = driver.stats_temporada.vitorias;
    driver.stats_carreira.podios = driver.stats_temporada.podios;
    driver.stats_carreira.poles = driver.stats_temporada.poles;
    driver.stats_carreira.corridas = driver.stats_temporada.corridas;
    driver.stats_carreira.dnfs = driver.stats_temporada.dnfs;
    driver.stats_carreira.temporadas = 0;
    driver_queries::update_driver(&conn, &driver).expect("update driver");
    let esperado = driver.stats_carreira.clone();

    run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

    let depois = driver_queries::get_driver(&conn, "P001").expect("driver");
    // A virada do ano fecha a temporada; não recontabiliza o que já foi contado.
    assert_eq!(depois.stats_carreira.pontos_total, esperado.pontos_total);
    assert_eq!(depois.stats_carreira.vitorias, esperado.vitorias);
    assert_eq!(depois.stats_carreira.podios, esperado.podios);
    assert_eq!(depois.stats_carreira.poles, esperado.poles);
    assert_eq!(depois.stats_carreira.corridas, esperado.corridas);
    assert_eq!(depois.stats_carreira.dnfs, esperado.dnfs);
    assert_eq!(depois.stats_carreira.temporadas, 1);
    let _ = std::fs::remove_dir_all(save_path);
}

#[test]
fn test_end_of_season_creates_new_season() {
    let (mut conn, season) = setup_pipeline_fixture();
    let save_path = unique_test_dir("eos_new_season");

    let result = run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

    let active = season_queries::get_active_season(&conn)
        .expect("active season query")
        .expect("new active season");
    assert_eq!(active.id, result.new_season_id);
    assert_eq!(active.numero, season.numero + 1);
    assert_eq!(active.ano, season.ano + 1);
    let _ = std::fs::remove_dir_all(save_path);
}

#[test]
fn test_end_of_season_resets_stats() {
    let (mut conn, season) = setup_pipeline_fixture();
    let save_path = unique_test_dir("eos_reset_stats");

    run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

    let drivers = driver_queries::get_drivers_by_category(&conn, "mazda_rookie")
        .expect("drivers should load");
    assert!(drivers
        .iter()
        .all(|driver| driver.stats_temporada.corridas == 0));
    assert!(drivers
        .iter()
        .all(|driver| driver.stats_temporada.pontos == 0.0));

    let teams =
        team_queries::get_teams_by_category(&conn, "mazda_rookie").expect("teams should load");
    assert!(teams.iter().all(|team| team.stats_pontos == 0));
    assert!(teams.iter().all(|team| team.stats_vitorias == 0));
    let _ = std::fs::remove_dir_all(save_path);
}

#[test]
fn test_end_of_season_retirement_report_keeps_final_category() {
    let (mut conn, season) = setup_pipeline_fixture();
    let save_path = unique_test_dir("eos_retirement_category");

    let mut driver = driver_queries::get_driver(&conn, "P001").expect("retiring driver");
    driver.idade = 47;
    driver_queries::update_driver(&conn, &driver).expect("update retiring driver");

    let result = run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

    let retirement = result
        .retirements
        .iter()
        .find(|entry| entry.driver_id == "P001")
        .expect("driver should retire");
    assert_eq!(retirement.categoria.as_deref(), Some("mazda_rookie"));

    let _ = std::fs::remove_dir_all(save_path);
}

#[test]
fn test_end_of_season_archive_excludes_newly_generated_rookies() {
    let (mut conn, season) = setup_pipeline_fixture();
    let save_path = unique_test_dir("eos_archive_excludes_rookies");

    let result = run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

    let archived_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM driver_season_archive WHERE season_number = ?1",
            rusqlite::params![season.numero],
            |row| row.get(0),
        )
        .expect("archive count");
    assert_eq!(
        archived_count, 2,
        "only season participants should be archived"
    );

    for rookie in &result.rookies_generated {
        let rookie_archived: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM driver_season_archive WHERE piloto_id = ?1 AND season_number = ?2",
                rusqlite::params![&rookie.driver_id, season.numero],
                |row| row.get(0),
            )
            .expect("rookie archive count");
        assert_eq!(
            rookie_archived, 0,
            "rookie '{}' should not be archived for the previous season",
            rookie.driver_id
        );
    }

    let _ = std::fs::remove_dir_all(save_path);
}

#[test]
fn test_end_of_season_standings_keep_regular_team_when_special_contract_is_active() {
    let (mut conn, season) = setup_pipeline_fixture();
    let save_path = unique_test_dir("eos_regular_contract_priority");

    let special_team = sample_named_team(
        "production_challenger",
        "SP001",
        "Special Team",
        Some("mazda"),
        1234,
    );
    team_queries::insert_team(&conn, &special_team).expect("insert special team");

    let mut special_contract = Contract::new(
        "C900".to_string(),
        "P001".to_string(),
        "Piloto A".to_string(),
        special_team.id.clone(),
        special_team.nome.clone(),
        1,
        1,
        50_000.0,
        TeamRole::Numero1,
        "production_challenger".to_string(),
    );
    special_contract.tipo = ContractType::Especial;
    special_contract.classe = Some("mazda".to_string());
    contract_queries::insert_contract(&conn, &special_contract).expect("insert special contract");

    run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

    let standings_team_id: String = conn
        .query_row(
            "SELECT equipe_id FROM standings
             WHERE temporada_id = ?1 AND piloto_id = ?2",
            rusqlite::params![&season.id, "P001"],
            |row| row.get(0),
        )
        .expect("standing for driver");
    assert_eq!(standings_team_id, "T001");

    let _ = std::fs::remove_dir_all(save_path);
}

#[test]
fn test_promotion_initializes_preseason_after_movements() {
    let (mut conn, season, promoted_team_id, _second_driver_id) = setup_promotion_order_fixture();
    let save_path = unique_test_dir("eos_preseason_order");

    let result = run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

    // Fase 4B: a campea do gt4 sobe para a Endurance classe gt4 levando os
    // pilotos; nenhum movimento toca LMP2.
    assert!(result
        .promotion_result
        .movements
        .iter()
        .all(|movement| movement.from_category != "lmp2" && movement.to_category != "lmp2"));
    assert!(result.preseason_initialized);
    assert!(result.preseason_total_weeks >= 3);

    let promoted_team = team_queries::get_team_by_id(&conn, &promoted_team_id)
        .expect("team query")
        .expect("promoted team");
    assert_eq!(promoted_team.categoria, "endurance");
    assert_eq!(promoted_team.classe.as_deref(), Some("gt4"));
    assert!(promoted_team.piloto_1_id.is_some() || promoted_team.piloto_2_id.is_some());

    assert!(save_path.join("preseason_plan.json").exists());
    let _ = std::fs::remove_dir_all(save_path);
}

#[test]
fn test_end_of_season_rolls_back_when_preseason_plan_save_fails() {
    let (mut conn, season) = setup_pipeline_fixture();
    let blocked_path = unique_test_dir("eos_save_failure").join("blocked_path");
    std::fs::write(&blocked_path, "not a directory").expect("blocker file");
    let mut retiring_driver = driver_queries::get_driver(&conn, "P001").expect("retiring driver");
    retiring_driver.idade = 47;
    driver_queries::update_driver(&conn, &retiring_driver).expect("update retiring driver");

    let result = run_end_of_season(&mut conn, &season, &blocked_path);

    assert!(
        result.is_err(),
        "pipeline should fail when save path is invalid"
    );
    let active = season_queries::get_active_season(&conn)
        .expect("active season query")
        .expect("original season should remain active");
    assert_eq!(active.id, season.id);
    let all_seasons = season_queries::get_all_seasons(&conn).expect("all seasons");
    assert_eq!(all_seasons.len(), 1, "new season should not be persisted");

    let retired_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM retired", [], |row| row.get(0))
        .expect("retired count");
    assert_eq!(retired_count, 0, "retirement snapshot should rollback");
    let driver = driver_queries::get_driver(&conn, "P001").expect("driver after rollback");
    assert_eq!(driver.status, DriverStatus::Ativo);
    assert_eq!(driver.categoria_atual.as_deref(), Some("mazda_rookie"));
    assert_eq!(driver.idade, 47);

    let _ = std::fs::remove_dir_all(blocked_path.parent().expect("parent"));
}

#[test]
fn champion_who_ends_the_year_injured_still_gets_the_title_credited() {
    let (mut conn, season) = setup_pipeline_fixture();
    let save_path = unique_test_dir("eos_titulo_lesionado");

    // P001 lidera a pontuação da fixture (120 x 90) e vai fechar campeão — mas
    // chega na virada lesionado, que é o caso que o crédito de título perdia:
    // o arquivo grava a linha de campeão de todo mundo, e o contador de carreira
    // parava no filtro `status != Ativo`.
    conn.execute(
        "UPDATE drivers SET status = 'Lesionado' WHERE id = 'P001'",
        [],
    )
    .expect("lesiona o campeao");

    run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

    let titulos: i64 = conn
        .query_row(
            "SELECT carreira_titulos FROM drivers WHERE id = 'P001'",
            [],
            |row| row.get(0),
        )
        .expect("contador de titulos");
    let arquivo_campeao: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM driver_season_archive
              WHERE piloto_id = 'P001' AND posicao_campeonato = 1",
            [],
            |row| row.get(0),
        )
        .expect("linhas campeas no arquivo");

    assert_eq!(arquivo_campeao, 1, "o arquivo registra o campeao");
    assert_eq!(
        titulos, arquivo_campeao,
        "contador de carreira e arquivo tem que contar o mesmo titulo"
    );
    let _ = std::fs::remove_dir_all(save_path);
}

fn setup_pipeline_fixture() -> (Connection, Season) {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");

    let season = Season::new("S001".to_string(), 1, 2024);
    season_queries::insert_season(&conn, &season).expect("season insert");
    seed_pipeline_supporting_teams(&conn);

    let mut rng = StdRng::seed_from_u64(10);
    let team_a = sample_team("mazda_rookie", "T001", &mut rng);
    let team_b = sample_team("mazda_rookie", "T002", &mut rng);
    team_queries::insert_team(&conn, &team_a).expect("team a");
    team_queries::insert_team(&conn, &team_b).expect("team b");

    let driver_a = sample_driver("P001", "Piloto A", "mazda_rookie", 120.0, 3, 5, 0);
    let driver_b = sample_driver("P002", "Piloto B", "mazda_rookie", 90.0, 1, 4, 1);
    driver_queries::insert_driver(&conn, &driver_a).expect("driver a");
    driver_queries::insert_driver(&conn, &driver_b).expect("driver b");

    let contract_a = Contract::new(
        "C001".to_string(),
        driver_a.id.clone(),
        driver_a.nome.clone(),
        team_a.id.clone(),
        team_a.nome.clone(),
        1,
        2,
        100_000.0,
        TeamRole::Numero1,
        "mazda_rookie".to_string(),
    );
    let contract_b = Contract::new(
        "C002".to_string(),
        driver_b.id.clone(),
        driver_b.nome.clone(),
        team_b.id.clone(),
        team_b.nome.clone(),
        1,
        2,
        90_000.0,
        TeamRole::Numero1,
        "mazda_rookie".to_string(),
    );
    contract_queries::insert_contract(&conn, &contract_a).expect("contract a");
    contract_queries::insert_contract(&conn, &contract_b).expect("contract b");

    let mut calendar_rng = StdRng::seed_from_u64(20);
    let entry = generate_calendar_for_category(&season.id, "mazda_rookie", &mut calendar_rng)
        .expect("calendar")
        .into_iter()
        .next()
        .expect("calendar entry");
    calendar_queries::insert_calendar_entry(&conn, &entry).expect("calendar insert");
    calendar_queries::mark_race_completed(&conn, &entry.id).expect("mark complete");
    conn.execute(
        "UPDATE meta SET value = '3' WHERE key = 'next_driver_id'",
        [],
    )
    .expect("meta driver counter");
    conn.execute(
        "UPDATE meta SET value = '3' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("meta contract counter");
    conn.execute(
        "UPDATE meta SET value = '2' WHERE key = 'next_season_id'",
        [],
    )
    .expect("meta season counter");
    conn.execute("UPDATE meta SET value = '2' WHERE key = 'next_race_id'", [])
        .expect("meta race counter");

    (conn, season)
}

fn setup_promotion_order_fixture() -> (Connection, Season, String, String) {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");

    let previous = Season::new("OLD1".to_string(), 1, 2024);
    season_queries::insert_season(&conn, &previous).expect("previous season");
    season_queries::finalize_season(&conn, &previous.id).expect("finalize previous season");

    let season = Season::new("CUR2".to_string(), 2, 2025);
    season_queries::insert_season(&conn, &season).expect("current season");

    seed_promotion_teams(&conn);
    seed_gt4_promotion_drivers(&conn);

    conn.execute(
        "UPDATE meta SET value = '2' WHERE key = 'current_season'",
        [],
    )
    .expect("meta current season");
    conn.execute(
        "UPDATE meta SET value = '2025' WHERE key = 'current_year'",
        [],
    )
    .expect("meta current year");

    (conn, season, "GT4PROMO".to_string(), "GT4LOW".to_string())
}

fn seed_promotion_teams(conn: &Connection) {
    insert_ranked_teams(conn, "mazda_rookie", "MR", 6, None);
    insert_ranked_teams(conn, "toyota_rookie", "TR", 6, None);
    insert_ranked_teams(conn, "mazda_amador", "MA", 10, None);
    insert_ranked_teams(conn, "toyota_amador", "TA", 10, None);
    insert_ranked_teams(conn, "bmw_m2", "BM", 10, None);
    insert_ranked_teams(conn, "production_challenger", "PM", 6, Some("mazda"));
    insert_ranked_teams(conn, "production_challenger", "PT", 6, Some("toyota"));
    insert_ranked_teams(conn, "production_challenger", "PB", 6, Some("bmw"));
    insert_ranked_teams(conn, "gt4", "GT4", 9, None);
    insert_ranked_teams(conn, "gt3", "GT3", 14, None);
    insert_ranked_teams(conn, "endurance", "EG4", 6, Some("gt4"));
    insert_ranked_teams(conn, "endurance", "EG3", 6, Some("gt3"));
    insert_ranked_teams(conn, "endurance", "LMP", 6, Some("lmp2"));

    let mut promoted_team = sample_named_team("gt4", "GT4PROMO", "GT4 Promo Team", None, 9001);
    promoted_team.stats_pontos = 999;
    promoted_team.stats_vitorias = 8;
    promoted_team.stats_melhor_resultado = 1;
    team_queries::insert_team(conn, &promoted_team).expect("insert promoted gt4 team");
}

fn seed_pipeline_supporting_teams(conn: &Connection) {
    insert_ranked_teams(conn, "mazda_rookie", "MR", 4, None);
    insert_ranked_teams(conn, "toyota_rookie", "TR", 6, None);
    insert_ranked_teams(conn, "mazda_amador", "MA", 10, None);
    insert_ranked_teams(conn, "toyota_amador", "TA", 10, None);
    insert_ranked_teams(conn, "bmw_m2", "BM", 10, None);
    insert_ranked_teams(conn, "production_challenger", "PM", 6, Some("mazda"));
    insert_ranked_teams(conn, "production_challenger", "PT", 6, Some("toyota"));
    insert_ranked_teams(conn, "production_challenger", "PB", 6, Some("bmw"));
    insert_ranked_teams(conn, "gt4", "GT4", 10, None);
    insert_ranked_teams(conn, "gt3", "GT3", 14, None);
    insert_ranked_teams(conn, "endurance", "EG4", 6, Some("gt4"));
    insert_ranked_teams(conn, "endurance", "EG3", 6, Some("gt3"));
    insert_ranked_teams(conn, "endurance", "LMP", 6, Some("lmp2"));
}

fn seed_gt4_promotion_drivers(conn: &Connection) {
    let licensed_driver = sample_driver("GT4TOP", "Piloto Licenciado", "gt4", 200.0, 4, 10, 0);
    let unlicensed_driver = sample_driver("GT4LOW", "Piloto Sem Licenca", "gt4", 5.0, 0, 10, 2);
    let support_drivers = [
        sample_driver("GT4D1", "GT4 Driver 1", "gt4", 150.0, 3, 10, 0),
        sample_driver("GT4D2", "GT4 Driver 2", "gt4", 130.0, 2, 10, 0),
        sample_driver("GT4D3", "GT4 Driver 3", "gt4", 110.0, 2, 10, 0),
        sample_driver("GT4D4", "GT4 Driver 4", "gt4", 90.0, 1, 10, 1),
        sample_driver("GT4D5", "GT4 Driver 5", "gt4", 70.0, 1, 10, 1),
        sample_driver("GT4D6", "GT4 Driver 6", "gt4", 50.0, 0, 10, 1),
    ];

    for driver in [&licensed_driver, &unlicensed_driver] {
        driver_queries::insert_driver(conn, driver).expect("insert promoted team driver");
    }
    for driver in &support_drivers {
        driver_queries::insert_driver(conn, driver).expect("insert support driver");
    }

    let contract_1 = Contract::new(
        "KGT401".to_string(),
        licensed_driver.id.clone(),
        licensed_driver.nome.clone(),
        "GT4PROMO".to_string(),
        "GT4 Promo Team".to_string(),
        2,
        2,
        150_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );
    let contract_2 = Contract::new(
        "KGT402".to_string(),
        unlicensed_driver.id.clone(),
        unlicensed_driver.nome.clone(),
        "GT4PROMO".to_string(),
        "GT4 Promo Team".to_string(),
        2,
        2,
        120_000.0,
        TeamRole::Numero2,
        "gt4".to_string(),
    );
    contract_queries::insert_contract(conn, &contract_1).expect("insert contract 1");
    contract_queries::insert_contract(conn, &contract_2).expect("insert contract 2");
    team_queries::update_team_pilots(
        conn,
        "GT4PROMO",
        Some(&licensed_driver.id),
        Some(&unlicensed_driver.id),
    )
    .expect("assign promoted team pilots");
}

fn insert_ranked_teams(
    conn: &Connection,
    category: &str,
    prefix: &str,
    count: usize,
    class: Option<&str>,
) {
    for index in 0..count {
        let rank = index + 1;
        let mut team = sample_named_team(
            category,
            &format!("{prefix}{rank}"),
            &format!("{prefix} Team {rank}"),
            class,
            rank as u64 + prefix.bytes().map(u64::from).sum::<u64>(),
        );
        team.stats_pontos = ((count - index) * 10) as i32;
        team.stats_vitorias = (count - index) as i32;
        team.stats_melhor_resultado = rank as i32;
        team_queries::insert_team(conn, &team).expect("insert ranked team");
    }
}

fn sample_driver(
    id: &str,
    name: &str,
    category: &str,
    points: f64,
    wins: u32,
    races: u32,
    dnfs: u32,
) -> Driver {
    let mut driver = Driver::new(
        id.to_string(),
        name.to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        24,
        2020,
    );
    driver.categoria_atual = Some(category.to_string());
    driver.stats_temporada.pontos = points;
    driver.stats_temporada.vitorias = wins;
    driver.stats_temporada.podios = wins + 1;
    driver.stats_temporada.corridas = races;
    driver.stats_temporada.dnfs = dnfs;
    driver.stats_temporada.poles = wins;
    driver.stats_temporada.posicao_media = 4.0;
    driver
}

fn sample_team(category: &str, id: &str, rng: &mut StdRng) -> Team {
    let template = get_team_templates(category)[0];
    Team::from_template_with_rng(template, category, id.to_string(), 2024, rng)
}

fn sample_named_team(category: &str, id: &str, name: &str, class: Option<&str>, seed: u64) -> Team {
    let template = crate::constants::teams::get_reference_team_template(category, class)
        .expect("team template");
    let mut rng = StdRng::seed_from_u64(seed);
    let mut team = Team::from_template_with_rng(template, category, id.to_string(), 2025, &mut rng);
    team.nome = name.to_string();
    team.nome_curto = name.to_string();
    team.classe = class.map(str::to_string);
    team
}

// ── Falência: o ciclo colapso → alerta → venda ────────────────────────────────

/// Equipe GT3 insolvente com dupla completa, para medir o que a venda tira.
/// `salario_n1` > `salario_n2` para o corte de folha ter um alvo determinado.
fn falencia_fixture() -> (Connection, Season, Team) {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");

    let season = Season::new("S001".to_string(), 4, 2027);
    season_queries::insert_season(&conn, &season).expect("season insert");

    let mut team = sample_named_team("gt3", "GT3F", "Escuderia Falida", None, 909);
    team.cash_balance = -250_000.0;
    team.debt_balance = 24_600_000.0; // o passivo medido no save real (T101)
    team.financial_state = "collapse".to_string();
    team.engineering = 62.0;
    team.facilities = 58.0;
    team.pit_crew_quality = 55.0;
    team.reputacao = 50.0;
    team.car_performance = 6.0;
    team_queries::insert_team(&conn, &team).expect("insert team");

    for (id, nome, salario, papel) in [
        ("PN1", "Piloto Caro", 900_000.0, TeamRole::Numero1),
        ("PN2", "Piloto Barato", 300_000.0, TeamRole::Numero2),
    ] {
        let driver = sample_driver(id, nome, "gt3", 50.0, 0, 10, 0);
        driver_queries::insert_driver(&conn, &driver).expect("insert driver");
        let contract = Contract::new(
            format!("C{id}"),
            driver.id.clone(),
            driver.nome.clone(),
            team.id.clone(),
            team.nome.clone(),
            season.numero,
            2,
            salario,
            papel,
            "gt3".to_string(),
        );
        contract_queries::insert_contract(&conn, &contract).expect("insert contract");
    }

    (conn, season, team)
}

fn eventos_de_propriedade(conn: &Connection, team_id: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT event_type FROM team_ownership_events WHERE team_id = ?1 ORDER BY id")
        .expect("prepare");
    let rows = stmt
        .query_map([team_id], |r| r.get::<_, String>(0))
        .expect("query");
    rows.map(|r| r.expect("row")).collect()
}

fn contar_noticias(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM news", [], |r| r.get(0))
        .expect("count news")
}

#[test]
fn primeiro_ano_insolvente_publica_alerta_e_registra_na_ficha() {
    // A quebra tem dois tempos. O primeiro é o alerta: sem ele a venda do ano
    // seguinte aparece do nada, e hoje ela não aparecia em lugar nenhum.
    let (conn, season, team) = falencia_fixture();
    let mut rng = StdRng::seed_from_u64(1);

    process_collapse_lifecycle(&conn, &season, &mut rng).expect("ciclo de colapso");

    assert_eq!(
        eventos_de_propriedade(&conn, &team.id),
        vec!["collapse_warning".to_string()],
        "o alerta tem que ficar na ficha da equipe"
    );
    assert_eq!(contar_noticias(&conn), 1, "o alerta tem que virar notícia");
    // Ninguém foi vendido ainda: a equipe segue com o passivo e com a dupla.
    let atual = team_queries::get_team_by_id(&conn, &team.id)
        .expect("team")
        .expect("existe");
    assert!(atual.debt_balance > 0.0, "no alerta a dívida continua");
    assert_eq!(
        contract_queries::get_active_regular_contracts_by_team(&conn, &team.id)
            .expect("contratos")
            .len(),
        2
    );
}

#[test]
fn venda_corta_a_folha_e_publica_a_manchete() {
    // Segundo ano insolvente: venda. O piloto mais caro sai (a nova diretoria não
    // paga aquela folha) e o mundo fica sabendo.
    let (conn, season, team) = falencia_fixture();
    team_queries::set_collapse_streak(&conn, &team.id, 1).expect("streak");
    let mut rng = StdRng::seed_from_u64(2);

    process_collapse_lifecycle(&conn, &season, &mut rng).expect("ciclo de colapso");

    assert_eq!(
        eventos_de_propriedade(&conn, &team.id),
        vec!["sale".to_string()],
        "a venda tem que ficar na ficha"
    );
    assert_eq!(contar_noticias(&conn), 1, "a venda tem que virar notícia");

    // O corte é NÃO RENOVAR: o contrato caro passa a terminar nesta temporada e
    // expira pela via normal do mercado. O assento não fica vazio no meio do
    // caminho — quem repõe é o leilão da pré-temporada.
    let contratos =
        contract_queries::get_active_regular_contracts_by_team(&conn, &team.id).expect("contratos");
    assert_eq!(contratos.len(), 2, "ninguém é rescindido no ato");
    let caro = contratos
        .iter()
        .find(|c| c.piloto_id == "PN1")
        .expect("contrato do caro");
    let barato = contratos
        .iter()
        .find(|c| c.piloto_id == "PN2")
        .expect("contrato do barato");
    assert_eq!(
        caro.temporada_fim, season.numero,
        "o mais caro não é renovado"
    );
    assert!(
        barato.temporada_fim > season.numero,
        "o mais barato segue sob contrato"
    );

    // E a equipe REGRIDE: carro e estrutura piores do que antes da quebra.
    let atual = team_queries::get_team_by_id(&conn, &team.id)
        .expect("team")
        .expect("existe");
    assert!(
        atual.car_performance < team.car_performance,
        "carro tinha que regredir: {} → {}",
        team.car_performance,
        atual.car_performance
    );
    assert!(atual.engineering < team.engineering);
    assert!(atual.reputacao < team.reputacao);
    assert_eq!(atual.debt_balance, 0.0, "o passivo é assumido na venda");
}

#[test]
fn venda_nunca_dispensa_o_jogador() {
    // Tirar o assento do jogador por evento do mundo seria decidir a carreira por
    // ele. Mesmo sendo o mais caro do elenco, quem sai é o companheiro.
    let (conn, season, team) = falencia_fixture();
    conn.execute("UPDATE drivers SET is_jogador = 1 WHERE id = 'PN1'", [])
        .expect("marca jogador");
    team_queries::set_collapse_streak(&conn, &team.id, 1).expect("streak");
    let mut rng = StdRng::seed_from_u64(3);

    process_collapse_lifecycle(&conn, &season, &mut rng).expect("ciclo de colapso");

    let contratos =
        contract_queries::get_active_regular_contracts_by_team(&conn, &team.id).expect("contratos");
    let jogador = contratos
        .iter()
        .find(|c| c.piloto_id == "PN1")
        .expect("contrato do jogador");
    let companheiro = contratos
        .iter()
        .find(|c| c.piloto_id == "PN2")
        .expect("contrato do companheiro");
    assert!(
        jogador.temporada_fim > season.numero,
        "o contrato do jogador não é tocado"
    );
    assert_eq!(
        companheiro.temporada_fim, season.numero,
        "quem perde o assento é o companheiro"
    );
    // E a manchete dele existe além da notícia geral da venda.
    assert_eq!(
        contar_noticias(&conn),
        2,
        "a equipe do jogador rende manchete própria"
    );
}

#[test]
fn equipe_com_um_piloto_so_nao_perde_o_ultimo() {
    // Corte de folha não pode esvaziar o grid: com um contrato só, não há folha a
    // cortar e a vigência dele fica intacta.
    let (conn, season, team) = falencia_fixture();
    conn.execute("DELETE FROM contracts WHERE piloto_id = 'PN2'", [])
        .expect("remove segundo contrato");
    team_queries::set_collapse_streak(&conn, &team.id, 1).expect("streak");
    let mut rng = StdRng::seed_from_u64(4);

    process_collapse_lifecycle(&conn, &season, &mut rng).expect("ciclo de colapso");

    let contratos =
        contract_queries::get_active_regular_contracts_by_team(&conn, &team.id).expect("contratos");
    assert_eq!(contratos.len(), 1);
    assert!(
        contratos[0].temporada_fim > season.numero,
        "o único piloto não é cortado"
    );
}

#[test]
fn contrato_que_ja_terminava_nao_conta_como_corte() {
    // Se o caro já estava no último ano, não há consequência nova a anunciar — o
    // mercado ia decidir sobre ele de qualquer jeito. O corte cai no outro, ou em
    // ninguém.
    let (conn, season, team) = falencia_fixture();
    conn.execute(
        "UPDATE contracts SET temporada_fim = ?1",
        [&season.numero.to_string()],
    )
    .expect("todos terminando");
    team_queries::set_collapse_streak(&conn, &team.id, 1).expect("streak");
    let mut rng = StdRng::seed_from_u64(6);

    process_collapse_lifecycle(&conn, &season, &mut rng).expect("ciclo de colapso");

    // A venda acontece; o que não acontece é um corte de folha inventado.
    assert_eq!(eventos_de_propriedade(&conn, &team.id), vec!["sale"]);
    let contratos =
        contract_queries::get_active_regular_contracts_by_team(&conn, &team.id).expect("contratos");
    assert!(contratos.iter().all(|c| c.temporada_fim == season.numero));
}

#[test]
fn equipe_recuperada_nao_gera_nem_alerta_nem_venda() {
    // O caminho feliz continua mudo: quem se salva sozinha no ano de all-in não
    // vira notícia de falência nem evento na ficha.
    let (conn, season, team) = falencia_fixture();
    conn.execute(
        "UPDATE teams SET financial_state = 'stable' WHERE id = ?1",
        [&team.id],
    )
    .expect("recupera");
    team_queries::set_collapse_streak(&conn, &team.id, 1).expect("streak");
    let mut rng = StdRng::seed_from_u64(5);

    process_collapse_lifecycle(&conn, &season, &mut rng).expect("ciclo de colapso");

    assert!(eventos_de_propriedade(&conn, &team.id).is_empty());
    assert_eq!(contar_noticias(&conn), 0);
    assert_eq!(
        team_queries::get_collapse_streak(&conn, &team.id).expect("streak"),
        0
    );
}

fn retention_fixture() -> (Connection, Season, Team, Driver, Contract) {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");

    let season = Season::new("S001".to_string(), 3, 2026);

    let team = sample_named_team("gt3", "GT3X", "GT3 Team X", None, 4242);
    assert!(!team.is_player_team);
    team_queries::insert_team(&conn, &team).expect("insert team");

    let mut veteran = sample_driver("VET", "Veterano", "gt3", 100.0, 5, 10, 0);
    veteran.atributos.skill = 85.0;
    veteran.stats_carreira.corridas = 250;
    driver_queries::insert_driver(&conn, &veteran).expect("insert veteran");

    let contract = Contract::new(
        "CVET".to_string(),
        veteran.id.clone(),
        veteran.nome.clone(),
        team.id.clone(),
        team.nome.clone(),
        season.numero,
        1,
        300_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &contract).expect("insert contract");

    (conn, season, team, veteran, contract)
}

#[test]
fn test_retains_irreplaceable_veteran_with_raise() {
    let (conn, season, _team, veteran, contract) = retention_fixture();
    let contracts_by_driver: HashMap<String, Contract> = [(veteran.id.clone(), contract.clone())]
        .into_iter()
        .collect();

    // Sem candidato licenciado na gt4 → deve reter.
    let retained = try_retain_irreplaceable_veteran(&conn, &veteran, &contracts_by_driver, &season)
        .expect("retention check");
    assert!(retained, "veterano sem substituto deve ser retido");

    // Contrato antigo não está mais ativo.
    let old_active: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM contracts WHERE id = 'CVET' AND status = 'Ativo'",
            [],
            |row| row.get(0),
        )
        .expect("old contract status");
    assert_eq!(old_active, 0, "contrato antigo deve ser rescindido");

    // Novo contrato: 1 ativo, +40% salário, começa na próxima temporada.
    let (count, new_salary, new_start): (i64, f64, String) = conn
        .query_row(
            "SELECT COUNT(*), MAX(salario_anual), MAX(temporada_inicio)
             FROM contracts WHERE piloto_id = 'VET' AND status = 'Ativo'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("new contract");
    assert_eq!(count, 1);
    assert!((new_salary - 300_000.0 * 1.40).abs() < 1.0);
    assert_eq!(new_start, (season.numero + 1).to_string());
}

#[test]
fn test_does_not_retain_veteran_when_licensed_substitute_exists() {
    let (conn, season, _team, veteran, contract) = retention_fixture();

    // Substituto na gt4 (feeder) com skill >= veterano e licença nível 3.
    let mut sub = sample_driver("SUB", "Substituto", "gt4", 80.0, 3, 10, 0);
    sub.atributos.skill = 90.0;
    sub.stats_carreira.corridas = 60;
    driver_queries::insert_driver(&conn, &sub).expect("insert sub");
    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao)
         VALUES ('SUB', '3', 'gt4', '')",
        [],
    )
    .expect("insert license");

    let contracts_by_driver: HashMap<String, Contract> = [(veteran.id.clone(), contract.clone())]
        .into_iter()
        .collect();

    let retained = try_retain_irreplaceable_veteran(&conn, &veteran, &contracts_by_driver, &season)
        .expect("retention check");
    assert!(
        !retained,
        "havendo substituto licenciado à altura, não deve reter"
    );
}

#[test]
fn test_retains_irreplaceable_veteran_even_on_player_team() {
    // É decisão do time: o jogador não controla retenção. Estar no time do jogador
    // NÃO isenta a retenção do companheiro insubstituível.
    let (conn, season, mut team, veteran, contract) = retention_fixture();
    team.is_player_team = true;
    let contracts_by_driver: HashMap<String, Contract> = [(veteran.id.clone(), contract.clone())]
        .into_iter()
        .collect();

    let retained = try_retain_irreplaceable_veteran(&conn, &veteran, &contracts_by_driver, &season)
        .expect("retention check");
    assert!(retained, "retenção vale inclusive para o time do jogador");
}

fn unique_test_dir(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("iracerapp_eos_{label}_{nanos}"));
    std::fs::create_dir_all(&path).expect("temp dir");
    path
}
