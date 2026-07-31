//! Testes de [`crate::commands::career`].
//!
//! Extraidos do bloco `#[cfg(test)]` que ficava no fim de `career.rs`.

use chrono::{Datelike, NaiveDate};
use std::fs;

use super::*;
use crate::commands::career_team_dossier::{
    get_team_history_dossier_in_base_dir, get_team_records_ranking_in_base_dir,
};
use crate::commands::career_types::TeamRecordsRow;

#[test]
fn test_validate_input_valid() {
    let input = CreateCareerInput {
        player_name: "Joao Silva".to_string(),
        player_nationality: "br".to_string(),
        player_age: Some(22),
        category: "mazda_rookie".to_string(),
        team_index: 2,
        difficulty: "medio".to_string(),
    };
    assert!(validate_create_career_input(&input).is_ok());
}

#[test]
fn test_validate_input_empty_name() {
    let input = CreateCareerInput {
        player_name: "   ".to_string(),
        player_nationality: "br".to_string(),
        player_age: Some(22),
        category: "mazda_rookie".to_string(),
        team_index: 2,
        difficulty: "medio".to_string(),
    };
    assert!(validate_create_career_input(&input).is_err());
}

#[test]
fn test_validate_input_invalid_category() {
    let input = CreateCareerInput {
        player_name: "Joao".to_string(),
        player_nationality: "br".to_string(),
        player_age: Some(22),
        category: "gt4".to_string(),
        team_index: 2,
        difficulty: "medio".to_string(),
    };
    assert!(validate_create_career_input(&input).is_err());
}

#[test]
fn offer_salary_uses_real_money_instead_of_legacy_budget() {
    let mut team = crate::models::team::placeholder_team_from_db(
        "TGT4".to_string(),
        "GT4 Rich".to_string(),
        "gt4".to_string(),
        "2026-01-01".to_string(),
    );
    team.cash_balance = 6_000_000.0;
    team.debt_balance = 0.0;
    team.financial_state = "healthy".to_string();
    team.budget = 1.0;

    let mut driver = Driver::new(
        "P001".to_string(),
        "Piloto Forte".to_string(),
        "br".to_string(),
        "M".to_string(),
        24,
        2026,
    );
    driver.atributos.skill = 80.0;

    let offer = calculate_offer_salary_for_team(&team, &driver);

    assert!(offer > 100_000.0);
}

#[test]
fn test_validate_input_invalid_team_index() {
    let input = CreateCareerInput {
        player_name: "Joao".to_string(),
        player_nationality: "br".to_string(),
        player_age: Some(22),
        category: "toyota_rookie".to_string(),
        team_index: 9,
        difficulty: "medio".to_string(),
    };
    assert!(validate_create_career_input(&input).is_err());
}

#[test]
fn test_validate_input_invalid_difficulty() {
    let input = CreateCareerInput {
        player_name: "Joao".to_string(),
        player_nationality: "br".to_string(),
        player_age: Some(22),
        category: "toyota_rookie".to_string(),
        team_index: 2,
        difficulty: "insano".to_string(),
    };
    assert!(validate_create_career_input(&input).is_err());
}

#[test]
fn test_next_career_id_empty_dir() {
    let base = unique_test_dir("empty");
    let saves_dir = base.join("saves");
    let next = next_career_id(&saves_dir);
    assert_eq!(next, "career_001");
    let _ = fs::remove_dir_all(base);
}

#[test]
fn test_next_career_id_with_existing() {
    let base = unique_test_dir("existing");
    let saves_dir = base.join("saves");
    fs::create_dir_all(saves_dir.join("career_001")).expect("career 001");
    fs::create_dir_all(saves_dir.join("career_003")).expect("career 003");
    let next = next_career_id(&saves_dir);
    assert_eq!(next, "career_004");
    let _ = fs::remove_dir_all(base);
}

#[test]
fn test_create_career_full_flow() {
    let base_dir = unique_test_dir("full_flow");
    fs::create_dir_all(&base_dir).expect("base dir");

    let input = CreateCareerInput {
        player_name: "Joao Silva".to_string(),
        player_nationality: "br".to_string(),
        player_age: Some(22),
        category: "mazda_rookie".to_string(),
        team_index: 2,
        difficulty: "medio".to_string(),
    };

    let result = create_career_in_base_dir(&base_dir, input).expect("career should be created");
    assert!(result.success);
    // Modelo fechado: apenas os 204 fundadores com contrato (grid). Sem pools.
    assert_eq!(result.total_drivers, 204);
    assert_eq!(result.total_teams, 102);
    // Modelo 9D: calendário unificado com todas as 9 divisões (74 corridas).
    assert_eq!(result.total_races, 74);

    let db_path = std::path::PathBuf::from(&result.save_path).join("career.db");
    assert!(db_path.exists());
    let meta_path = std::path::PathBuf::from(&result.save_path).join("meta.json");
    assert!(meta_path.exists());

    let db = Database::open_existing(&db_path).expect("db should open");
    let drivers_count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM drivers", [], |row| row.get(0))
        .expect("drivers count");
    let teams_count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM teams", [], |row| row.get(0))
        .expect("teams count");
    let contracts_count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM contracts", [], |row| row.get(0))
        .expect("contracts count");
    let seasons_count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM seasons", [], |row| row.get(0))
        .expect("seasons count");
    let calendar_count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM calendar", [], |row| row.get(0))
        .expect("calendar count");

    assert_eq!(drivers_count, 204);
    assert_eq!(teams_count, 102);
    assert_eq!(contracts_count, 204);
    assert_eq!(seasons_count, 1);
    // 74 corridas: todas as 9 divisões no modelo 9D.
    assert_eq!(calendar_count, 74);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_create_career_seeds_initial_licenses_for_active_grid() {
    let base_dir = unique_test_dir("seed_initial_licenses");
    fs::create_dir_all(&base_dir).expect("base dir");

    let input = CreateCareerInput {
        player_name: "Joao Silva".to_string(),
        player_nationality: "br".to_string(),
        player_age: Some(22),
        category: "mazda_rookie".to_string(),
        team_index: 2,
        difficulty: "medio".to_string(),
    };

    let result = create_career_in_base_dir(&base_dir, input).expect("career should be created");
    let db_path = std::path::PathBuf::from(&result.save_path).join("career.db");
    let db = Database::open_existing(&db_path).expect("db should open");

    let seeded_licenses: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM licenses", [], |row| row.get(0))
        .expect("licenses count");
    let gt3_without_license: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
             FROM contracts c
             JOIN teams t ON t.id = c.equipe_id
             LEFT JOIN licenses l
               ON l.piloto_id = c.piloto_id
              AND CAST(l.nivel AS INTEGER) >= 3
             WHERE c.status = 'Ativo'
               AND c.tipo = 'Regular'
               AND t.categoria = 'gt3'
               AND l.piloto_id IS NULL",
            [],
            |row| row.get(0),
        )
        .expect("gt3 license coverage");
    let gt4_without_license: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
             FROM contracts c
             JOIN teams t ON t.id = c.equipe_id
             LEFT JOIN licenses l
               ON l.piloto_id = c.piloto_id
              AND CAST(l.nivel AS INTEGER) >= 2
             WHERE c.status = 'Ativo'
               AND c.tipo = 'Regular'
               AND t.categoria = 'gt4'
               AND l.piloto_id IS NULL",
            [],
            |row| row.get(0),
        )
        .expect("gt4 license coverage");

    assert_eq!(seeded_licenses, 180);
    assert_eq!(gt3_without_license, 0);
    assert_eq!(gt4_without_license, 0);

    let _ = fs::remove_dir_all(base_dir);
}

// ── Etapa 5: Integração Modelo 9D ─────────────────────────────────────────

#[test]
fn nova_carreira_cria_temporada_com_fase_temporada() {
    let base_dir = unique_test_dir("e5_fase_temporada");
    fs::create_dir_all(&base_dir).expect("base dir");
    let input = CreateCareerInput {
        player_name: "Test9D".to_string(),
        player_nationality: "br".to_string(),
        player_age: Some(22),
        category: "mazda_rookie".to_string(),
        team_index: 0,
        difficulty: "medio".to_string(),
    };
    create_career_in_base_dir(&base_dir, input).expect("career");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    assert_eq!(season.fase.as_str(), "Temporada");
    assert_eq!(season.status.as_str(), "EmAndamento");
    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn nova_carreira_tem_74_entradas_todas_pendentes_com_season_week() {
    let base_dir = unique_test_dir("e5_74_entradas");
    fs::create_dir_all(&base_dir).expect("base dir");
    let input = CreateCareerInput {
        player_name: "Test9D".to_string(),
        player_nationality: "br".to_string(),
        player_age: Some(22),
        category: "mazda_rookie".to_string(),
        team_index: 0,
        difficulty: "medio".to_string(),
    };
    create_career_in_base_dir(&base_dir, input).expect("career");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let total: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM calendar", [], |r| r.get(0))
        .expect("count");
    let pendentes: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM calendar WHERE status = 'Pendente'",
            [],
            |r| r.get(0),
        )
        .expect("pendentes");
    let com_season_week: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM calendar WHERE season_week IS NOT NULL
             AND season_week >= 10 AND season_week <= 51",
            [],
            |r| r.get(0),
        )
        .expect("season_week range");
    let fase_temporada: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM calendar WHERE season_phase = 'Temporada'",
            [],
            |r| r.get(0),
        )
        .expect("season_phase");

    assert_eq!(total, 74, "devem existir 74 entradas no calendário 9D");
    assert_eq!(pendentes, 74, "todas as 74 entradas devem estar Pendente");
    assert_eq!(
        com_season_week, 74,
        "todas as entradas devem ter season_week 10-51"
    );
    assert_eq!(
        fase_temporada, 74,
        "todas as entradas devem ter season_phase=Temporada"
    );
    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn nova_carreira_nao_tem_special_team_entries_nem_contratos_especiais() {
    let base_dir = unique_test_dir("e5_no_special");
    fs::create_dir_all(&base_dir).expect("base dir");
    let input = CreateCareerInput {
        player_name: "Test9D".to_string(),
        player_nationality: "br".to_string(),
        player_age: Some(22),
        category: "mazda_rookie".to_string(),
        team_index: 0,
        difficulty: "medio".to_string(),
    };
    create_career_in_base_dir(&base_dir, input).expect("career");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let special_entries: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM special_team_entries", [], |r| {
            r.get(0)
        })
        .expect("special_team_entries");
    let especial_contracts: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM contracts WHERE tipo = 'Especial'",
            [],
            |r| r.get(0),
        )
        .expect("especial contracts");

    assert_eq!(
        special_entries, 0,
        "mundo novo não deve ter special_team_entries"
    );
    assert_eq!(
        especial_contracts, 0,
        "mundo novo não deve ter contratos Especial"
    );
    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn nova_carreira_simula_primeira_corrida_com_fase_temporada() {
    let base_dir = unique_test_dir("e5_simulate_first_race");
    fs::create_dir_all(&base_dir).expect("base dir");
    let input = CreateCareerInput {
        player_name: "Test9D".to_string(),
        player_nationality: "br".to_string(),
        player_age: Some(22),
        category: "mazda_rookie".to_string(),
        team_index: 0,
        difficulty: "medio".to_string(),
    };
    create_career_in_base_dir(&base_dir, input).expect("career");

    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    let next_race =
        crate::db::queries::calendar::get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race query")
            .expect("pending mazda_rookie race");
    drop(db);

    let result = crate::commands::race::simulate_race_weekend_in_base_dir(
        &base_dir,
        "career_001",
        &next_race.id,
    )
    .expect("simulate deve funcionar com fase Temporada");

    assert_eq!(result.player_race.race_results.len(), 12);
    let _ = fs::remove_dir_all(base_dir);
}

// ── Fim Etapa 5 ───────────────────────────────────────────────────────────

#[test]
fn test_load_career_returns_player() {
    let base_dir = create_test_career_dir("load_player");
    let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");

    assert!(career.player.is_jogador);
    assert_eq!(career.player.nome, "Joao Silva");

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_load_career_returns_team() {
    let base_dir = create_test_career_dir("load_team");
    let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
    let player_team = career.player_team.as_ref().expect("player team");

    assert!(!player_team.id.is_empty());
    assert!(player_team.piloto_1_id.is_some());
    assert!(player_team.piloto_2_id.is_some());
    assert!((0.0..=100.0).contains(&player_team.pit_strategy_risk));
    assert!((0.0..=100.0).contains(&player_team.pit_crew_quality));
    assert!(player_team.cash_balance >= 0.0);
    assert!(player_team.debt_balance >= 0.0);
    assert!(!player_team.financial_state.is_empty());
    assert!(!player_team.season_strategy.is_empty());
    assert!(player_team.spending_power.is_finite());
    assert!(player_team.salary_ceiling > 0.0);
    assert!((0.0..=100.0).contains(&player_team.budget_index));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_load_career_returns_season() {
    let base_dir = create_test_career_dir("load_season");
    let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");

    assert_eq!(career.season.numero, 1);
    assert_eq!(career.season.ano, 2024);
    assert!(career.season.total_rodadas > 0);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_load_career_includes_next_race_briefing() {
    let base_dir = create_test_career_dir("load_briefing_contract");
    let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
    let career_json = serde_json::to_value(&career).expect("career json");

    assert!(
        career_json.get("next_race_briefing").is_some(),
        "expected load_career payload to expose next_race_briefing",
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_load_career_restores_resume_context_snapshot() {
    let base_dir = create_test_career_dir("load_resume_context");
    mark_all_races_completed(&base_dir, "career_001");

    let result =
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season should work");
    let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
    let resume_context = career.resume_context.expect("resume context");

    assert_eq!(resume_context.active_view, CareerResumeView::EndOfSeason);
    assert_eq!(
        resume_context
            .end_of_season_result
            .as_ref()
            .map(|snapshot| snapshot.new_year),
        Some(result.new_year)
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_load_career_repairs_early_convocation_with_regular_races_pending() {
    let base_dir = create_test_career_dir("load_repair_early_convocation");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    // Forçar estado legado BlocoRegular antes de simular convocação antecipada.
    force_legacy_blocoregular_state(&db);
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    season_queries::update_season_fase(&db.conn, &season.id, &SeasonPhase::JanelaConvocacao)
        .expect("force early convocation");

    let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");

    assert_eq!(career.season.fase, "BlocoRegular");
    let refreshed_season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    assert_eq!(refreshed_season.fase, SeasonPhase::BlocoRegular);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_load_career_serializes_convocation_state_fields() {
    let base_dir = create_test_career_dir("load_convocation_contract_payload");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let mut player = driver_queries::get_player_driver(&db.conn).expect("player");
    player.categoria_atual = Some("gt4".to_string());
    player.atributos.skill = 98.0;
    driver_queries::update_driver(&db.conn, &player).expect("update player");

    force_legacy_blocoregular_state(&db);
    mark_regular_races_completed(&db);
    crate::convocation::advance_to_convocation_window(&db.conn).expect("advance convocation");
    crate::convocation::run_convocation_window(&db.conn).expect("run convocation");
    let offers = crate::commands::convocation::get_player_special_offers_in_base_dir(
        &base_dir,
        "career_001",
    )
    .expect("special offers");
    assert!(offers.is_empty());

    let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
    let payload = serde_json::to_value(&career).expect("serialize payload");

    assert_eq!(payload["season"]["fase"], "JanelaConvocacao");
    assert!(payload["player"]["categoria_especial_ativa"].is_null());
    assert!(
        payload["player_team"].get("classe").is_some(),
        "player_team.classe deveria ser serializado para a UI"
    );
    assert!(payload["accepted_special_offer"].is_null());

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_load_career_prefers_regular_team_outside_special_phase() {
    let base_dir = create_test_career_dir("load_regular_team_outside_special_phase");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let regular_contract =
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
            .expect("regular contract")
            .expect("player regular contract");
    let special_team = insert_test_endurance_team(&db.conn);
    let season = season_queries::get_active_season(&db.conn)
        .expect("season")
        .expect("active season");

    let special_contract = contract_queries::generate_especial_contract(
        next_id(&db.conn, IdType::Contract).expect("special contract id"),
        &player.id,
        &player.nome,
        &special_team.id,
        &special_team.nome,
        TeamRole::Numero1,
        "endurance",
        special_team.classe.as_deref().unwrap_or("gt4"),
        season.numero,
    );
    contract_queries::insert_contract(&db.conn, &special_contract).expect("insert special");
    driver_queries::update_driver_especial_category(&db.conn, &player.id, Some("endurance"))
        .expect("set special category");
    team_queries::update_team_pilots(&db.conn, &special_team.id, Some(&player.id), None)
        .expect("set special lineup");
    season_queries::update_season_fase(&db.conn, &season.id, &SeasonPhase::BlocoRegular)
        .expect("keep regular phase");

    let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
    let player_team = career.player_team.as_ref().expect("player team");

    assert_eq!(player_team.id, regular_contract.equipe_id);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_load_career_repairs_duplicate_regular_contract_state() {
    let base_dir = create_test_career_dir("repair_duplicate_regular_contract_state");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let mut player = driver_queries::get_player_driver(&db.conn).expect("player");
    player.atributos.skill = 99.0;
    player.categoria_atual = Some("gt4".to_string());
    driver_queries::update_driver(&db.conn, &player).expect("update player");

    let original_contract =
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
            .expect("original contract")
            .expect("player regular contract");
    let replacement_team = team_queries::get_teams_by_category(&db.conn, "mazda_rookie")
        .expect("rookie teams")
        .into_iter()
        .find(|team| team.id != original_contract.equipe_id)
        .expect("replacement team");
    let displaced_contract =
        contract_queries::get_active_contracts_for_team(&db.conn, &replacement_team.id)
            .expect("replacement contracts")
            .into_iter()
            .find(|contract| contract.tipo == crate::models::enums::ContractType::Regular)
            .expect("regular driver to displace");
    contract_queries::update_contract_status(
        &db.conn,
        &displaced_contract.id,
        &ContractStatus::Rescindido,
    )
    .expect("rescind replacement seat");
    db.conn
        .execute_batch("DROP INDEX IF EXISTS idx_contracts_active_pilot_tipo;")
        .expect("drop active-contract uniqueness guard for corruption scenario");

    let mut replacement_contract = crate::models::contract::Contract::new(
        next_id(&db.conn, IdType::Contract).expect("replacement contract id"),
        player.id.clone(),
        player.nome.clone(),
        replacement_team.id.clone(),
        replacement_team.nome.clone(),
        original_contract.temporada_inicio,
        2,
        250_000.0,
        TeamRole::Numero1,
        replacement_team.categoria.clone(),
    );
    replacement_contract.created_at = "9999-12-31T23:59:59".to_string();
    contract_queries::insert_contract(&db.conn, &replacement_contract)
        .expect("insert replacement contract");

    let gt4_team = team_queries::get_teams_by_category(&db.conn, "gt4")
        .expect("gt4 teams")
        .into_iter()
        .next()
        .expect("gt4 team");
    team_queries::update_team_pilots(
        &db.conn,
        &gt4_team.id,
        Some(&player.id),
        gt4_team.piloto_2_id.as_deref(),
    )
    .expect("corrupt gt4 lineup");

    let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
    let refreshed_db = Database::open_existing(&db_path).expect("db reopen");
    let refreshed_player = driver_queries::get_player_driver(&refreshed_db.conn).expect("player");
    let active_regular_contracts =
        contract_queries::get_contracts_for_pilot(&refreshed_db.conn, &player.id)
            .expect("player contracts")
            .into_iter()
            .filter(|contract| {
                contract.status == ContractStatus::Ativo
                    && contract.tipo == crate::models::enums::ContractType::Regular
            })
            .collect::<Vec<_>>();
    let original_contract_after =
        contract_queries::get_contract_by_id(&refreshed_db.conn, &original_contract.id)
            .expect("query original contract")
            .expect("original contract exists");
    let refreshed_replacement_team =
        team_queries::get_team_by_id(&refreshed_db.conn, &replacement_team.id)
            .expect("query replacement team")
            .expect("replacement team");
    let refreshed_gt4_team = team_queries::get_team_by_id(&refreshed_db.conn, &gt4_team.id)
        .expect("query gt4 team")
        .expect("gt4 team");
    let player_team = career.player_team.as_ref().expect("player team");

    assert_eq!(player_team.id, replacement_team.id);
    assert_eq!(active_regular_contracts.len(), 1);
    assert_eq!(active_regular_contracts[0].id, replacement_contract.id);
    assert_eq!(original_contract_after.status, ContractStatus::Rescindido);
    assert_eq!(
        refreshed_player.categoria_atual.as_deref(),
        Some(replacement_team.categoria.as_str())
    );
    assert!(
        refreshed_gt4_team.piloto_1_id.as_deref() != Some(player.id.as_str())
            && refreshed_gt4_team.piloto_2_id.as_deref() != Some(player.id.as_str())
    );
    assert!(
        refreshed_replacement_team.piloto_1_id.as_deref() == Some(player.id.as_str())
            || refreshed_replacement_team.piloto_2_id.as_deref() == Some(player.id.as_str())
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_next_race_briefing_summarizes_track_history() {
    let base_dir = create_test_career_dir("load_briefing_track_history");
    let career_id = "career_001";
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season")
        .expect("active season");
    let calendar =
        calendar_queries::get_calendar(&db.conn, &season.id, "mazda_rookie").expect("calendar");
    let race_one = calendar.first().expect("race one");
    let race_two = calendar.get(1).expect("race two");

    db.conn
        .execute(
            "UPDATE calendar SET track_name = ?1 WHERE id IN (?2, ?3)",
            rusqlite::params!["Pista Espelho", race_one.id, race_two.id],
        )
        .expect("update track names");

    let race_result = crate::commands::race::simulate_race_weekend_in_base_dir(
        &base_dir,
        career_id,
        &race_one.id,
    )
    .expect("simulate race");
    let player_finish = race_result
        .player_race
        .race_results
        .iter()
        .find(|entry| entry.is_jogador)
        .map(|entry| entry.finish_position)
        .expect("player finish");
    let player_dnf = race_result
        .player_race
        .race_results
        .iter()
        .find(|entry| entry.is_jogador)
        .map(|entry| entry.is_dnf)
        .expect("player dnf flag");

    let career = load_career_in_base_dir(&base_dir, career_id).expect("load career");
    let track_history = career
        .next_race_briefing
        .as_ref()
        .and_then(|briefing| briefing.track_history.as_ref())
        .expect("track history");

    assert!(track_history.has_data);
    assert_eq!(track_history.starts, 1);
    assert_eq!(
        track_history.best_finish,
        if player_dnf {
            None
        } else {
            Some(player_finish)
        }
    );
    assert_eq!(track_history.last_finish, Some(player_finish));
    assert_eq!(track_history.dnfs, if player_dnf { 1 } else { 0 });
    assert_eq!(track_history.last_visit_season, Some(1));
    assert_eq!(track_history.last_visit_round, Some(1));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_next_race_briefing_exposes_primary_rival() {
    let base_dir = create_test_career_dir("load_briefing_primary_rival");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let rival_driver = driver_queries::get_drivers_by_category(&db.conn, "mazda_rookie")
        .expect("category drivers")
        .into_iter()
        .find(|driver| !driver.is_jogador)
        .expect("ai rival");

    db.conn
        .execute(
            "UPDATE drivers SET temp_pontos = 90.0, temp_vitorias = 3, temp_podios = 4 WHERE id = ?1",
            rusqlite::params![player.id],
        )
        .expect("update player");
    db.conn
        .execute(
            "UPDATE drivers SET temp_pontos = 96.0, temp_vitorias = 4, temp_podios = 5 WHERE id = ?1",
            rusqlite::params![rival_driver.id],
        )
        .expect("update rival");

    let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
    let rival = career
        .next_race_briefing
        .as_ref()
        .and_then(|briefing| briefing.primary_rival.as_ref())
        .expect("primary rival");

    assert_eq!(rival.driver_id, rival_driver.id);
    assert_eq!(rival.driver_name, rival_driver.nome);
    assert_eq!(rival.championship_position, 1);
    assert_eq!(rival.gap_points, 6);
    assert!(rival.is_ahead);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_next_race_briefing_filters_weekend_stories() {
    let base_dir = create_test_career_dir("load_briefing_weekend_stories");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");

    news_queries::insert_news_batch(
        &db.conn,
        &vec![
            NewsItem {
                id: "BRF001".to_string(),
                tipo: NewsType::Rivalidade,
                icone: "R".to_string(),
                titulo: "Duelo esquenta a abertura".to_string(),
                texto: "A tensao entre os protagonistas cresce antes da etapa de abertura."
                    .to_string(),
                rodada: Some(1),
                semana_pretemporada: None,
                temporada: season.numero,
                categoria_id: Some("mazda_rookie".to_string()),
                categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                importancia: NewsImportance::Destaque,
                timestamp: 300,
                driver_id: Some("P001".to_string()),
                driver_id_secondary: Some("P002".to_string()),
                team_id: None,
            },
            NewsItem {
                id: "BRF002".to_string(),
                tipo: NewsType::Hierarquia,
                icone: "H".to_string(),
                titulo: "Equipe reavalia ordem interna".to_string(),
                texto: "O box chega atento ao equilibrio interno antes da largada.".to_string(),
                rodada: Some(1),
                semana_pretemporada: None,
                temporada: season.numero,
                categoria_id: Some("mazda_rookie".to_string()),
                categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                importancia: NewsImportance::Alta,
                timestamp: 250,
                driver_id: Some("P001".to_string()),
                driver_id_secondary: None,
                team_id: None,
            },
            NewsItem {
                id: "BRF003".to_string(),
                tipo: NewsType::Corrida,
                icone: "C".to_string(),
                titulo: "Abertura promete grid apertado".to_string(),
                texto: "A etapa de abertura deve embaralhar o pelotao logo nas primeiras voltas."
                    .to_string(),
                rodada: Some(1),
                semana_pretemporada: None,
                temporada: season.numero,
                categoria_id: Some("mazda_rookie".to_string()),
                categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                importancia: NewsImportance::Alta,
                timestamp: 200,
                driver_id: Some("P001".to_string()),
                driver_id_secondary: None,
                team_id: None,
            },
            NewsItem {
                id: "BRF004".to_string(),
                tipo: NewsType::Corrida,
                icone: "X".to_string(),
                titulo: "Outra categoria movimenta a semana".to_string(),
                texto: "Essa noticia nao deve entrar na previa da etapa do jogador.".to_string(),
                rodada: Some(1),
                semana_pretemporada: None,
                temporada: season.numero,
                categoria_id: Some("gt4".to_string()),
                categoria_nome: Some("GT4".to_string()),
                importancia: NewsImportance::Destaque,
                timestamp: 400,
                driver_id: None,
                driver_id_secondary: None,
                team_id: None,
            },
        ],
    )
    .expect("seed news");

    let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
    let stories = &career
        .next_race_briefing
        .as_ref()
        .expect("briefing")
        .weekend_stories;

    assert_eq!(stories.len(), 3);
    assert_eq!(stories[0].title, "Duelo esquenta a abertura");
    assert!(stories
        .iter()
        .all(|story| !story.title.contains("Outra categoria")));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_load_career_invalid_id() {
    let base_dir = unique_test_dir("load_invalid");
    fs::create_dir_all(&base_dir).expect("base dir");

    let error = load_career_in_base_dir(&base_dir, "career_999").expect_err("should fail");
    assert!(error.contains("Save nao encontrado"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_list_saves_format() {
    let base_dir = create_test_career_dir("list_saves");
    let saves = list_saves_in_base_dir(&base_dir).expect("list saves");

    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0].career_id, "career_001");
    assert_eq!(saves[0].player_name, "Joao Silva");
    assert_eq!(saves[0].category, "mazda_rookie");
    assert_eq!(saves[0].season, 1);
    assert!(saves[0].total_races > 0);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_drivers_by_category_returns_ordered_standings() {
    let base_dir = create_test_career_dir("drivers_by_category");
    let standings = get_drivers_by_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("driver standings");

    assert_eq!(standings.len(), 12);
    assert_eq!(standings[0].posicao_campeonato, 1);
    assert!(standings
        .windows(2)
        .all(|window| window[0].pontos >= window[1].pontos));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_drivers_by_category_marks_rookies() {
    let base_dir = create_test_career_dir("drivers_rookie_marker");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let mut rookie = driver_queries::get_player_driver(&db.conn).expect("player");
    rookie.stats_carreira.corridas = 0;
    rookie.stats_carreira.temporadas = 0;
    rookie.temporadas_na_categoria = 0;
    driver_queries::update_driver(&db.conn, &rookie).expect("update rookie");

    let mut veteran = driver_queries::get_drivers_by_category(&db.conn, "mazda_rookie")
        .expect("drivers")
        .into_iter()
        .find(|driver| !driver.is_jogador)
        .expect("non-player driver");
    veteran.stats_carreira.corridas = 12;
    veteran.stats_carreira.temporadas = 1;
    veteran.temporadas_na_categoria = 0;
    driver_queries::update_driver(&db.conn, &veteran).expect("update veteran");

    let tx = db.conn.unchecked_transaction().expect("injury tx");
    crate::db::queries::injuries::insert_injury(
        &tx,
        &crate::models::injury::Injury {
            id: "I_TEST_LIGHT".to_string(),
            pilot_id: rookie.id.clone(),
            injury_type: crate::models::enums::InjuryType::Moderada,
            injury_name: "Braço machucado".to_string(),
            modifier: 0.85,
            races_total: 2,
            races_remaining: 2,
            skill_penalty: 0.1,
            season: 1,
            race_occurred: "R001".to_string(),
            active: true,
        },
    )
    .expect("insert rookie injury");
    crate::db::queries::injuries::insert_injury(
        &tx,
        &crate::models::injury::Injury {
            id: "I_TEST_GRAVE".to_string(),
            pilot_id: veteran.id.clone(),
            injury_type: crate::models::enums::InjuryType::Grave,
            injury_name: "Braço fraturado".to_string(),
            modifier: 0.65,
            races_total: 4,
            races_remaining: 4,
            skill_penalty: 0.25,
            season: 1,
            race_occurred: "R002".to_string(),
            active: true,
        },
    )
    .expect("insert veteran injury");
    tx.commit().expect("commit injuries");
    drop(db);

    let standings = get_drivers_by_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("driver standings");
    let rookie_entry = standings
        .iter()
        .find(|entry| entry.id == rookie.id)
        .expect("rookie entry");
    let veteran_entry = standings
        .iter()
        .find(|entry| entry.id == veteran.id)
        .expect("veteran entry");

    assert!(rookie_entry.is_estreante);
    assert!(rookie_entry.is_estreante_da_vida);
    assert!(veteran_entry.is_estreante);
    assert!(!veteran_entry.is_estreante_da_vida);
    assert_eq!(rookie_entry.lesao_ativa_tipo.as_deref(), Some("Moderada"));
    assert_eq!(veteran_entry.lesao_ativa_tipo.as_deref(), Some("Grave"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_drivers_by_category_excludes_non_participants_once_category_has_results() {
    let base_dir = create_test_career_dir("drivers_exclude_non_participants");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let mut outsider = crate::models::driver::Driver::new(
        "P_OUTSIDER".to_string(),
        "Miguel Garcia".to_string(),
        "br".to_string(),
        "M".to_string(),
        19,
        2025,
    );
    outsider.categoria_atual = Some("mazda_rookie".to_string());
    driver_queries::insert_driver(&db.conn, &outsider).expect("insert outsider");

    let participant = driver_queries::get_player_driver(&db.conn).expect("player");
    let participant_team = find_player_team(
        &db.conn,
        &participant.id,
        crate::models::enums::SeasonPhase::BlocoRegular,
    )
    .expect("player team")
    .expect("active player team");
    let race_id: String = db
        .conn
        .query_row(
            "SELECT id FROM calendar WHERE categoria = 'mazda_rookie' ORDER BY rodada ASC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("mazda rookie calendar race");
    db.conn
        .execute(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos)
             VALUES (?1, ?2, ?3, 1, 25.0)",
            rusqlite::params![race_id, participant.id, participant_team.id],
        )
        .expect("seed participant race result");
    drop(db);

    let standings = get_drivers_by_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("driver standings");

    assert!(
        standings.iter().all(|entry| entry.id != "P_OUTSIDER"),
        "driver without season participation should not appear once the category already has race results"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_open_career_repairs_regular_category_vacancies() {
    let base_dir = create_test_career_dir("repair_regular_vacancies");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let team = team_queries::get_teams_by_category(&db.conn, "toyota_rookie")
        .expect("toyota teams")
        .into_iter()
        .next()
        .expect("toyota team");
    let removed_driver = team
        .piloto_2_id
        .clone()
        .expect("test team should have second driver");
    let removed_contract =
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &removed_driver)
            .expect("contract query")
            .expect("active contract");

    contract_queries::update_contract_status(
        &db.conn,
        &removed_contract.id,
        &ContractStatus::Rescindido,
    )
    .expect("rescind contract");
    team_queries::update_team_pilots(&db.conn, &team.id, team.piloto_1_id.as_deref(), None)
        .expect("clear team slot");
    drop(db);

    load_career_in_base_dir(&base_dir, "career_001").expect("load career should repair");
    let standings = get_drivers_by_category_in_base_dir(&base_dir, "career_001", "toyota_rookie")
        .expect("driver standings after repair");
    let db = Database::open_existing(&db_path).expect("db after repair");
    let empty_slots: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
             FROM teams
             WHERE categoria = 'toyota_rookie'
               AND ativa = 1
               AND (piloto_1_id IS NULL OR piloto_2_id IS NULL)",
            [],
            |row| row.get(0),
        )
        .expect("empty slots");
    let active_contracts: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(DISTINCT piloto_id)
             FROM contracts
             WHERE categoria = 'toyota_rookie'
               AND tipo = 'Regular'
               AND status = 'Ativo'",
            [],
            |row| row.get(0),
        )
        .expect("active contracts");

    assert_eq!(standings.len(), 12);
    assert_eq!(empty_slots, 0);
    assert_eq!(active_contracts, 12);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_lmp2_category_read_does_not_create_standalone_resources() {
    let base_dir = create_test_career_dir("no_standalone_lmp2_read");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");

    let drivers =
        get_drivers_by_category_in_base_dir(&base_dir, "career_001", "lmp2").expect("lmp2 drivers");
    let teams =
        get_teams_standings_in_base_dir(&base_dir, "career_001", "lmp2").expect("lmp2 teams");
    let calendar = get_calendar_for_category_in_base_dir(&base_dir, "career_001", "lmp2")
        .expect("lmp2 calendar");
    let db = Database::open_existing(&db_path).expect("db after lmp2 read");
    let standalone_lmp2_teams =
        team_queries::count_teams_by_category(&db.conn, "lmp2").expect("lmp2 teams");
    let standalone_lmp2_contracts: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM contracts WHERE categoria = 'lmp2'",
            [],
            |row| row.get(0),
        )
        .expect("lmp2 contracts");

    assert!(drivers.is_empty());
    assert!(teams.is_empty());
    assert!(calendar.is_empty());
    assert_eq!(standalone_lmp2_teams, 0);
    assert_eq!(standalone_lmp2_contracts, 0);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_lmp2_standings_read_does_not_run_global_regular_repair() {
    let base_dir = create_test_career_dir("lmp2_read_is_lightweight");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let team = team_queries::get_teams_by_category(&db.conn, "toyota_rookie")
        .expect("toyota teams")
        .into_iter()
        .find(|candidate| candidate.piloto_2_id.is_some())
        .expect("toyota team with second driver");
    let removed_driver = team
        .piloto_2_id
        .clone()
        .expect("test team should have second driver");
    let removed_contract =
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &removed_driver)
            .expect("contract query")
            .expect("active contract");

    contract_queries::update_contract_status(
        &db.conn,
        &removed_contract.id,
        &ContractStatus::Rescindido,
    )
    .expect("rescind contract");
    team_queries::update_team_pilots(&db.conn, &team.id, team.piloto_1_id.as_deref(), None)
        .expect("clear team slot");
    drop(db);

    let standings = get_drivers_by_category_in_base_dir(&base_dir, "career_001", "lmp2")
        .expect("lmp2 standings");
    let db = Database::open_existing(&db_path).expect("db after lmp2 read");
    let empty_toyota_slots: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
             FROM teams
             WHERE categoria = 'toyota_rookie'
               AND ativa = 1
               AND (piloto_1_id IS NULL OR piloto_2_id IS NULL)",
            [],
            |row| row.get(0),
        )
        .expect("empty toyota slots");

    assert!(standings.is_empty());
    assert_eq!(empty_toyota_slots, 1);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_open_career_repairs_regular_contracts_in_special_categories() {
    let base_dir = create_test_career_dir("repair_regular_special_contract");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let special_team = insert_test_endurance_team(&db.conn);
    let mut driver = crate::models::driver::Driver::new(
        "P_BAD_SPECIAL".to_string(),
        "Regular Especial".to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        24,
        2024,
    );
    driver.categoria_atual = Some("endurance".to_string());
    driver_queries::insert_driver(&db.conn, &driver).expect("insert driver");
    let bad_contract = crate::models::contract::Contract::new(
        "C_BAD_SPECIAL".to_string(),
        driver.id.clone(),
        driver.nome.clone(),
        special_team.id.clone(),
        special_team.nome.clone(),
        1,
        3,
        100_000.0,
        TeamRole::Numero1,
        "endurance".to_string(),
    );
    assert!(bad_contract.classe.is_none());
    contract_queries::insert_contract(&db.conn, &bad_contract).expect("insert bad contract");
    team_queries::update_team_pilots(&db.conn, &special_team.id, Some(&driver.id), None)
        .expect("assign special team");
    mark_regular_races_completed(&db);
    drop(db);

    load_career_in_base_dir(&base_dir, "career_001").expect("load career should repair");

    let repaired_db = Database::open_existing(&db_path).expect("db after repair");
    let repaired_contract =
        contract_queries::get_contract_by_id(&repaired_db.conn, "C_BAD_SPECIAL")
            .expect("contract query")
            .expect("contract");
    let repaired_driver =
        driver_queries::get_driver(&repaired_db.conn, "P_BAD_SPECIAL").expect("driver");

    assert_eq!(repaired_contract.status, ContractStatus::Rescindido);
    // No modelo fechado, o piloto liberado pode ser re-contratado para a mesma
    // vaga de endurance — mas agora com um contrato VÁLIDO (com classe). O reparo
    // cumpriu seu papel desde que não reste nenhum contrato regular de endurance
    // SEM classe (que é a inconsistência original).
    let _ = repaired_driver;
    let invalid_endurance_remaining =
        contract_queries::get_all_active_regular_contracts(&repaired_db.conn)
            .expect("active contracts")
            .into_iter()
            .any(|contract| {
                contract.piloto_id == "P_BAD_SPECIAL"
                    && contract.categoria == "endurance"
                    && contract.classe.is_none()
            });
    assert!(
        !invalid_endurance_remaining,
        "nao deve restar contrato regular de endurance sem classe apos o reparo"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_open_career_keeps_regular_endurance_contract_with_valid_class() {
    let base_dir = create_test_career_dir("repair_valid_endurance_regular_contract");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let mut endurance_team = insert_test_endurance_team(&db.conn);
    endurance_team.classe = Some("gt3".to_string());
    team_queries::update_team(&db.conn, &endurance_team).expect("update endurance class");
    let mut driver = crate::models::driver::Driver::new(
        "P_VALID_ENDURANCE".to_string(),
        "Regular Endurance GT3".to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        25,
        2024,
    );
    driver.categoria_atual = Some("endurance".to_string());
    driver_queries::insert_driver(&db.conn, &driver).expect("insert driver");
    let mut contract = crate::models::contract::Contract::new(
        "C_VALID_ENDURANCE".to_string(),
        driver.id.clone(),
        driver.nome.clone(),
        endurance_team.id.clone(),
        endurance_team.nome.clone(),
        1,
        3,
        100_000.0,
        TeamRole::Numero1,
        "endurance".to_string(),
    );
    contract.classe = Some("gt3".to_string());
    contract_queries::insert_contract(&db.conn, &contract).expect("insert valid contract");
    team_queries::update_team_pilots(&db.conn, &endurance_team.id, Some(&driver.id), None)
        .expect("assign team");
    mark_regular_races_completed(&db);
    drop(db);

    load_career_in_base_dir(&base_dir, "career_001").expect("load career should repair");

    let repaired_db = Database::open_existing(&db_path).expect("db after repair");
    let repaired_contract =
        contract_queries::get_contract_by_id(&repaired_db.conn, "C_VALID_ENDURANCE")
            .expect("contract query")
            .expect("contract");

    assert_eq!(repaired_contract.status, ContractStatus::Ativo);
    assert_eq!(repaired_contract.classe.as_deref(), Some("gt3"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_open_career_keeps_regular_production_contract_with_valid_class() {
    let base_dir = create_test_career_dir("repair_valid_production_regular_contract");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let production_team = insert_test_production_team(&db.conn, "mazda");
    let mut driver = crate::models::driver::Driver::new(
        "P_VALID_PRODUCTION".to_string(),
        "Regular Production Mazda".to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        25,
        2024,
    );
    driver.categoria_atual = Some("production_challenger".to_string());
    driver_queries::insert_driver(&db.conn, &driver).expect("insert driver");
    let mut contract = crate::models::contract::Contract::new(
        "C_VALID_PRODUCTION".to_string(),
        driver.id.clone(),
        driver.nome.clone(),
        production_team.id.clone(),
        production_team.nome.clone(),
        1,
        3,
        75_000.0,
        TeamRole::Numero1,
        "production_challenger".to_string(),
    );
    contract.classe = Some("mazda".to_string());
    contract_queries::insert_contract(&db.conn, &contract).expect("insert valid contract");
    team_queries::update_team_pilots(&db.conn, &production_team.id, Some(&driver.id), None)
        .expect("assign team");
    mark_regular_races_completed(&db);
    drop(db);

    load_career_in_base_dir(&base_dir, "career_001").expect("load career should repair");

    let repaired_db = Database::open_existing(&db_path).expect("db after repair");
    let repaired_contract =
        contract_queries::get_contract_by_id(&repaired_db.conn, "C_VALID_PRODUCTION")
            .expect("contract query")
            .expect("contract");

    assert_eq!(repaired_contract.status, ContractStatus::Ativo);
    assert_eq!(repaired_contract.classe.as_deref(), Some("mazda"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_open_career_does_not_fill_regular_vacancies_after_regular_block_ends() {
    let base_dir = create_test_career_dir("skip_regular_repair_after_block_end");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let team = team_queries::get_teams_by_category(&db.conn, "toyota_rookie")
        .expect("toyota teams")
        .into_iter()
        .find(|candidate| candidate.piloto_2_id.is_some())
        .expect("toyota team with second driver");
    let removed_driver = team
        .piloto_2_id
        .clone()
        .expect("test team should have second driver");
    let removed_contract =
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &removed_driver)
            .expect("contract query")
            .expect("active contract");

    contract_queries::update_contract_status(
        &db.conn,
        &removed_contract.id,
        &ContractStatus::Rescindido,
    )
    .expect("rescind contract");
    team_queries::update_team_pilots(&db.conn, &team.id, team.piloto_1_id.as_deref(), None)
        .expect("clear team slot");
    force_legacy_blocoregular_state(&db);
    mark_regular_races_completed(&db);
    drop(db);

    load_career_in_base_dir(&base_dir, "career_001")
        .expect("load career should repair state without hiring replacement");
    let db = Database::open_existing(&db_path).expect("db after load");
    let empty_slots: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
             FROM teams
             WHERE categoria = 'toyota_rookie'
               AND ativa = 1
               AND (piloto_1_id IS NULL OR piloto_2_id IS NULL)",
            [],
            |row| row.get(0),
        )
        .expect("empty slots");
    let active_contracts: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(DISTINCT piloto_id)
             FROM contracts
             WHERE categoria = 'toyota_rookie'
               AND tipo = 'Regular'
               AND status = 'Ativo'",
            [],
            |row| row.get(0),
        )
        .expect("active contracts");
    let removed_driver_after =
        driver_queries::get_driver(&db.conn, &removed_driver).expect("removed driver");

    assert_eq!(empty_slots, 1);
    assert_eq!(active_contracts, 11);
    assert_eq!(removed_driver_after.categoria_atual, None);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_open_career_does_not_fill_regular_vacancies_during_preseason() {
    let base_dir = create_test_career_dir("skip_regular_repair_during_preseason");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");

    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    // No modelo fechado, ao entrar na pré-temporada o grid de estreia pode ter
    // vagas (o mercado as preenche ao longo das semanas). Para o teste ser
    // determinístico, garantimos um time de toyota_rookie com os 2 assentos
    // ocupados antes de medir o baseline.
    {
        let target = team_queries::get_teams_by_category(&db.conn, "toyota_rookie")
            .expect("toyota teams")
            .into_iter()
            .next()
            .expect("a toyota_rookie team");
        for (idx, slot) in [&target.piloto_1_id, &target.piloto_2_id]
            .iter()
            .enumerate()
        {
            if slot.is_some() {
                continue;
            }
            let role = if idx == 0 {
                TeamRole::Numero1
            } else {
                TeamRole::Numero2
            };
            let driver_id = format!("P_TR_SEED_{idx}");
            let mut driver = crate::models::driver::Driver::new(
                driver_id.clone(),
                format!("Seed TR {idx}"),
                "Brasil".to_string(),
                "M".to_string(),
                19,
                2024,
            );
            driver.categoria_atual = Some("toyota_rookie".to_string());
            driver_queries::insert_driver(&db.conn, &driver).expect("seed tr driver");
            let contract = crate::models::contract::Contract::new(
                format!("C_TR_SEED_{idx}"),
                driver_id,
                driver.nome.clone(),
                target.id.clone(),
                target.nome.clone(),
                1,
                2,
                50_000.0,
                role,
                "toyota_rookie".to_string(),
            );
            contract_queries::insert_contract(&db.conn, &contract).expect("seed tr contract");
        }
        let refreshed = team_queries::get_team_by_id(&db.conn, &target.id)
            .expect("seeded team query")
            .expect("seeded team");
        team_queries::update_team_pilots(
            &db.conn,
            &target.id,
            refreshed
                .piloto_1_id
                .clone()
                .or(Some("P_TR_SEED_0".to_string()))
                .as_deref(),
            refreshed
                .piloto_2_id
                .clone()
                .or(Some("P_TR_SEED_1".to_string()))
                .as_deref(),
        )
        .expect("seed tr lineup");
    }

    let baseline_empty_slots: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
             FROM teams
             WHERE categoria = 'toyota_rookie'
               AND ativa = 1
               AND (piloto_1_id IS NULL OR piloto_2_id IS NULL)",
            [],
            |row| row.get(0),
        )
        .expect("baseline empty slots");
    let baseline_active_contracts: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(DISTINCT piloto_id)
             FROM contracts
             WHERE categoria = 'toyota_rookie'
               AND tipo = 'Regular'
               AND status = 'Ativo'",
            [],
            |row| row.get(0),
        )
        .expect("baseline active contracts");
    let team = team_queries::get_teams_by_category(&db.conn, "toyota_rookie")
        .expect("toyota teams")
        .into_iter()
        .find(|candidate| candidate.piloto_2_id.is_some())
        .expect("toyota team with second driver");
    let removed_driver = team
        .piloto_2_id
        .clone()
        .expect("test team should have second driver");
    let removed_contract =
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &removed_driver)
            .expect("contract query")
            .expect("active contract");

    contract_queries::update_contract_status(
        &db.conn,
        &removed_contract.id,
        &ContractStatus::Rescindido,
    )
    .expect("rescind contract");
    team_queries::update_team_pilots(&db.conn, &team.id, team.piloto_1_id.as_deref(), None)
        .expect("clear team slot");
    let expected_empty_slots = baseline_empty_slots + 1;
    let expected_active_contracts = baseline_active_contracts - 1;
    drop(db);

    get_preseason_state_in_base_dir(&base_dir, "career_001").expect("preseason state");

    let db = Database::open_existing(&db_path).expect("db after load");
    let empty_slots: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
             FROM teams
             WHERE categoria = 'toyota_rookie'
               AND ativa = 1
               AND (piloto_1_id IS NULL OR piloto_2_id IS NULL)",
            [],
            |row| row.get(0),
        )
        .expect("empty slots");
    let active_contracts: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(DISTINCT piloto_id)
             FROM contracts
             WHERE categoria = 'toyota_rookie'
               AND tipo = 'Regular'
               AND status = 'Ativo'",
            [],
            |row| row.get(0),
        )
        .expect("active contracts");

    assert_eq!(empty_slots, expected_empty_slots);
    assert_eq!(active_contracts, expected_active_contracts);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_open_career_preserves_player_team_when_regular_roles_are_duplicated() {
    let base_dir = create_test_career_dir("preserve_player_team_on_duplicate_roles");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    let player_team = find_player_team(&db.conn, &player.id, season.fase)
        .expect("player team lookup")
        .expect("player team");
    let teammate_id = player_team
        .piloto_1_id
        .clone()
        .filter(|id| id != &player.id)
        .or_else(|| {
            player_team
                .piloto_2_id
                .clone()
                .filter(|id| id != &player.id)
        })
        .expect("teammate id");
    let teammate_contract =
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &teammate_id)
            .expect("teammate contract query")
            .expect("teammate active contract");
    let player_contract =
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
            .expect("player contract query")
            .expect("player active contract");

    db.conn
        .execute(
            "UPDATE contracts SET papel = 'Numero2', created_at = '9999-12-31T23:59:59' WHERE id = ?1",
            rusqlite::params![&teammate_contract.id],
        )
        .expect("force duplicated role on teammate contract");
    db.conn
        .execute(
            "UPDATE contracts SET papel = 'Numero2', created_at = '2020-01-01T00:00:00' WHERE id = ?1",
            rusqlite::params![&player_contract.id],
        )
        .expect("force duplicated role on player contract");
    drop(db);

    let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
    let repaired_db = Database::open_existing(&db_path).expect("db after repair");
    let repaired_player_contract =
        contract_queries::get_active_regular_contract_for_pilot(&repaired_db.conn, &player.id)
            .expect("player contract query after repair")
            .expect("player contract should remain active");
    let repaired_team = team_queries::get_team_by_id(&repaired_db.conn, &player_team.id)
        .expect("team query after repair")
        .expect("player team after repair");
    let repaired_teammate_contract =
        contract_queries::get_active_regular_contract_for_pilot(&repaired_db.conn, &teammate_id)
            .expect("teammate contract query after repair")
            .expect("teammate contract should remain active");

    assert_eq!(career.player.id, player.id);
    assert_eq!(repaired_player_contract.equipe_id, player_team.id);
    assert_eq!(repaired_teammate_contract.equipe_id, player_team.id);
    assert_ne!(
        repaired_player_contract.papel,
        repaired_teammate_contract.papel
    );
    let repaired_slots = [
        repaired_team.piloto_1_id.as_deref(),
        repaired_team.piloto_2_id.as_deref(),
    ];
    assert!(repaired_slots.contains(&Some(player.id.as_str())));
    assert!(repaired_slots.contains(&Some(teammate_id.as_str())));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_concurrent_career_loads_serialize_regular_contract_repair() {
    let base_dir = create_test_career_dir("serialize_regular_repair");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let team = team_queries::get_teams_by_category(&db.conn, "toyota_rookie")
        .expect("toyota teams")
        .into_iter()
        .next()
        .expect("toyota team");
    // Modelo fechado: o genesis não cria agentes livres, então inserimos um
    // piloto livre explicitamente para anexar o contrato excedente do teste.
    let mut free_driver = crate::models::driver::Driver::new(
        "P_FREE_SURPLUS".to_string(),
        "Livre Excedente".to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        24,
        2024,
    );
    free_driver.categoria_atual = None;
    driver_queries::insert_driver(&db.conn, &free_driver).expect("insert free driver");
    let mut surplus_contract = crate::models::contract::Contract::new(
        next_id(&db.conn, IdType::Contract).expect("contract id"),
        free_driver.id.clone(),
        free_driver.nome.clone(),
        team.id.clone(),
        team.nome.clone(),
        1,
        1,
        50_000.0,
        TeamRole::Numero2,
        team.categoria.clone(),
    );
    surplus_contract.created_at = "0000-01-01T00:00:00".to_string();
    contract_queries::insert_contract(&db.conn, &surplus_contract)
        .expect("insert surplus contract");
    drop(db);

    let handles = (0..4)
        .map(|_| {
            let base_dir = base_dir.clone();
            std::thread::spawn(move || load_career_in_base_dir(&base_dir, "career_001"))
        })
        .collect::<Vec<_>>();

    for handle in handles {
        handle.join().expect("thread should finish").expect("load");
    }

    let db = Database::open_existing(&db_path).expect("db after repair");
    let repaired_contract = contract_queries::get_contract_by_id(&db.conn, &surplus_contract.id)
        .expect("surplus query")
        .expect("surplus contract");
    assert_eq!(repaired_contract.status, ContractStatus::Rescindido);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_drivers_by_category_uses_recent_results_fallback_from_driver_record() {
    let base_dir = create_test_career_dir("drivers_recent_fallback");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let mut driver = driver_queries::get_player_driver(&db.conn).expect("player");
    driver.stats_temporada.corridas = 3;
    driver.ultimos_resultados = serde_json::json!([
        { "position": 8, "is_dnf": false },
        { "position": 6, "is_dnf": false },
        { "position": 4, "is_dnf": false }
    ]);
    driver_queries::update_driver(&db.conn, &driver).expect("update driver");

    let results_path = config
        .saves_dir()
        .join("career_001")
        .join("race_results.json");
    if results_path.exists() {
        fs::remove_file(&results_path).expect("remove history file");
    }

    let standings = get_drivers_by_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("driver standings");
    let player = standings
        .into_iter()
        .find(|entry| entry.is_jogador)
        .expect("player standing");

    let fallback_tail: Vec<i32> = player
        .results
        .iter()
        .flatten()
        .map(|result| result.position)
        .collect();

    assert_eq!(fallback_tail, vec![8, 6, 4]);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_drivers_by_category_keeps_special_standings_after_skip_cleanup() {
    let base_dir = create_test_career_dir("special_standings_after_skip");

    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    force_legacy_blocoregular_state(&db);
    db.conn
        .execute(
            "UPDATE calendar SET status = 'Concluida' WHERE season_phase = 'BlocoRegular'",
            [],
        )
        .expect("complete regular block");
    crate::convocation::advance_to_convocation_window(&db.conn).expect("advance convocation");
    crate::convocation::run_convocation_window(&db.conn).expect("run convocation");
    crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");
    drop(db);

    crate::commands::race::simulate_special_block_in_base_dir(&base_dir, "career_001")
        .expect("simulate special block");
    let db = Database::open_existing(&db_path).expect("db after special sim");
    crate::convocation::encerrar_bloco_especial(&db.conn).expect("end special block");
    crate::convocation::run_pos_especial(&db.conn).expect("run pos especial");
    drop(db);

    let standings =
        get_drivers_by_category_in_base_dir(&base_dir, "career_001", "production_challenger")
            .expect("production special standings");

    assert!(
        !standings.is_empty(),
        "standings especiais devem continuar visiveis apos o cleanup"
    );
    assert!(
        standings.iter().any(|driver| driver.pontos > 0),
        "standings especiais devem refletir pontos simulados"
    );
    assert!(
        standings
            .iter()
            .any(|driver| driver.results.iter().any(Option::is_some)),
        "standings especiais devem manter resultados por rodada"
    );
    assert!(
        standings
            .iter()
            .any(|driver| driver.classe.as_deref() == Some("bmw")),
        "standings especiais devem carregar a classe/carro do piloto"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_teams_standings_keeps_special_lineup_after_skip_cleanup() {
    let base_dir = create_test_career_dir("special_team_standings_after_skip");

    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    force_legacy_blocoregular_state(&db);
    db.conn
        .execute(
            "UPDATE calendar SET status = 'Concluida' WHERE season_phase = 'BlocoRegular'",
            [],
        )
        .expect("complete regular block");
    crate::convocation::advance_to_convocation_window(&db.conn).expect("advance convocation");
    crate::convocation::run_convocation_window(&db.conn).expect("run convocation");
    crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");
    drop(db);

    crate::commands::race::simulate_special_block_in_base_dir(&base_dir, "career_001")
        .expect("simulate special block");
    let db = Database::open_existing(&db_path).expect("db after special sim");
    crate::convocation::encerrar_bloco_especial(&db.conn).expect("end special block");
    crate::convocation::run_pos_especial(&db.conn).expect("run pos especial");
    drop(db);

    let standings =
        get_teams_standings_in_base_dir(&base_dir, "career_001", "production_challenger")
            .expect("production team standings");

    assert!(
        !standings.is_empty(),
        "standings de equipes especiais devem continuar visiveis apos o cleanup"
    );
    assert!(
        standings.iter().any(|team| team.pontos > 0),
        "standings de equipes especiais devem refletir pontos simulados"
    );
    assert!(
        standings
            .iter()
            .any(|team| { team.piloto_1_nome.is_some() || team.piloto_2_nome.is_some() }),
        "standings de equipes especiais devem preservar os pilotos pelo historico de corrida"
    );
    assert!(
        standings
            .iter()
            .any(|team| team.classe.as_deref() == Some("bmw")),
        "standings de equipes especiais devem carregar a classe/carro da equipe"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_teams_standings_returns_category_grid() {
    let base_dir = create_test_career_dir("teams_standings");
    let standings = get_teams_standings_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("team standings");

    assert_eq!(standings.len(), 6);
    assert_eq!(standings[0].posicao, 1);
    assert!(standings[0].founded_year > 0);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_teams_standings_uses_previous_season_order_before_first_race() {
    let base_dir = create_test_career_dir("teams_standings_previous_order");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let teams = team_queries::get_teams_by_category(&db.conn, "mazda_rookie").expect("teams");
    let first_team = teams.first().expect("first team");
    let second_team = teams.get(1).expect("second team");

    db.conn
        .execute(
            "UPDATE seasons SET numero = 2, ano = 2026 WHERE status = 'EmAndamento'",
            [],
        )
        .expect("move active season");
    db.conn
        .execute(
            "INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
             VALUES ('S_PREV_TEAM_ORDER', 1, 2025, 'Finalizada', 8, 'PosEspecial', '', '')",
            [],
        )
        .expect("insert previous season");
    db.conn
        .execute(
            "INSERT INTO drivers (id, nome, idade, nacionalidade, genero)
             VALUES
                ('P_PREV_LOW', 'Piloto Anterior Baixo', 24, 'Brasil', 'M'),
                ('P_PREV_HIGH', 'Piloto Anterior Alto', 26, 'Brasil', 'M')",
            [],
        )
        .expect("insert previous drivers");
    db.conn
        .execute(
            "INSERT INTO standings (
                temporada_id, piloto_id, equipe_id, categoria, posicao, pontos, vitorias, podios, poles, corridas
             ) VALUES
                ('S_PREV_TEAM_ORDER', 'P_PREV_LOW', ?1, 'mazda_rookie', 2, 12, 0, 0, 0, 8),
                ('S_PREV_TEAM_ORDER', 'P_PREV_HIGH', ?2, 'mazda_rookie', 1, 88, 4, 6, 0, 8)",
            rusqlite::params![&first_team.id, &second_team.id],
        )
        .expect("insert previous standings");

    let standings = get_teams_standings_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("team standings");

    assert_eq!(standings[0].id, second_team.id);
    assert_eq!(standings[0].posicao, 1);
    assert_eq!(standings[1].id, first_team.id);
    assert_eq!(standings[1].posicao, 2);
    assert_eq!(
        standings[0].pontos, 0,
        "temporada atual ainda deve estar zerada"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
#[serial_test::serial]
fn test_get_team_history_dossier_uses_real_race_results_for_any_team() {
    rust_i18n::set_locale("pt-BR"); // dossiê assevera prosa PT (ver race_eval).
    let base_dir = create_test_career_dir("team_history_dossier_real_results");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let teams = get_teams_standings_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("team standings");
    let selected = teams.first().expect("selected team");
    let rival = teams.get(1).expect("rival team");
    let (selected_driver_1, selected_driver_2) =
        team_driver_ids(&db.conn, &selected.id).expect("selected drivers");
    let (rival_driver_1, _) = team_driver_ids(&db.conn, &rival.id).expect("rival drivers");
    let race_ids: Vec<String> = db
        .conn
        .prepare(
            "SELECT id FROM calendar
             WHERE categoria = 'mazda_rookie'
             ORDER BY rodada ASC
             LIMIT 4",
        )
        .expect("prepare races")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query races")
        .collect::<Result<Vec<_>, _>>()
        .expect("race ids");

    db.conn
        .execute("DELETE FROM race_results", [])
        .expect("clear race results");
    for (race_id, driver_id, team_id, finish, points) in [
        (&race_ids[0], &selected_driver_1, &selected.id, 1, 25.0),
        (&race_ids[0], &selected_driver_2, &selected.id, 4, 12.0),
        (&race_ids[0], &rival_driver_1, &rival.id, 2, 18.0),
        (&race_ids[1], &selected_driver_1, &selected.id, 2, 18.0),
        (&race_ids[1], &selected_driver_2, &selected.id, 5, 10.0),
        (&race_ids[1], &rival_driver_1, &rival.id, 1, 25.0),
        (&race_ids[2], &selected_driver_1, &selected.id, 8, 4.0),
        (&race_ids[2], &selected_driver_2, &selected.id, 9, 2.0),
        (&race_ids[2], &rival_driver_1, &rival.id, 1, 25.0),
        (&race_ids[3], &selected_driver_1, &selected.id, 3, 15.0),
        (&race_ids[3], &selected_driver_2, &selected.id, 6, 8.0),
        (&race_ids[3], &rival_driver_1, &rival.id, 1, 25.0),
    ] {
        db.conn
            .execute(
                "INSERT INTO race_results (
                    race_id, piloto_id, equipe_id, posicao_final, pontos
                ) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![race_id, driver_id, team_id, finish, points],
            )
            .expect("insert race result");
    }
    db.conn
        .execute(
            "UPDATE teams
             SET cash_balance = ?1,
                 debt_balance = ?2,
                 financial_state = ?3,
                 last_round_income = ?4,
                 last_round_expenses = ?5,
                 last_round_net = ?6,
                 car_performance = ?7,
                 engineering = ?8,
                 facilities = ?9
             WHERE id = ?10",
            rusqlite::params![
                4_200_000.0,
                1_250_000.0,
                "pressured",
                380_000.0,
                510_000.0,
                -130_000.0,
                7.4,
                63.0,
                58.0,
                &selected.id,
            ],
        )
        .expect("update real finance snapshot");
    // O "pacote técnico" do dossiê é o Nível do Carro (as 11 peças), NÃO a coluna legada
    // `car_performance` acima — que o sistema de peças nunca atualiza. Semeia o carro no
    // nível 7 pra o dossiê ter o que ler.
    crate::db::queries::team_car::upsert_team_car(
        &db.conn,
        &selected.id,
        &crate::car::Car::uniform(7),
    )
    .expect("seed team car");
    drop(db);

    let dossier =
        get_team_history_dossier_in_base_dir(&base_dir, "career_001", &selected.id, "mazda_rookie")
            .expect("team dossier");

    assert!(dossier.has_history);
    // Os cards de record comparam dentro da CATEGORIA, não do grupo: o card
    // responde "onde esta equipe está entre as que correm com ela", e quem corre
    // com ela é a categoria. É também o que faz o card e a tabela de recordes
    // (que abre em "só a categoria") mostrarem o mesmo número.
    assert_eq!(dossier.record_scope, "Mazda Rookie");
    assert_eq!(dossier.sport.races, 4);
    assert_eq!(dossier.sport.wins, 1);
    assert_eq!(dossier.sport.podiums, 3);
    assert_eq!(dossier.sport.win_rate, "25%");
    assert_eq!(dossier.sport.podium_rate, "75%");
    assert_eq!(dossier.sport.seasons, "1 Temporada");
    assert_eq!(dossier.sport.current_streak, "1 temporada no nível Rookie");
    assert_eq!(dossier.sport.best_streak, "2 Pódios consecutivos");
    assert!(dossier
        .timeline
        .iter()
        .any(|item| item.text.contains("vitória real")));
    assert_eq!(
        dossier
            .records
            .iter()
            .find(|record| record.label == "Vitórias")
            .map(|record| (record.rank.as_str(), record.value.as_str())),
        Some(("2º", "1"))
    );
    // Todos os records comparam contra o MESMO universo: as equipes que correram
    // no grupo. Títulos rankeava só contra as campeãs, e o dossiê mostrava
    // denominadores diferentes lado a lado ("10º de 10" junto de "14º de 19").
    let record_by_id = |id: &str| {
        dossier
            .records
            .iter()
            .find(|record| record.id == id)
            .unwrap_or_else(|| panic!("record {id}"))
            .clone()
    };
    assert_eq!(record_by_id("titles").rank_total, 2);
    assert_eq!(record_by_id("wins").rank_total, 2);
    assert_eq!(record_by_id("podiums").rank_total, 2);
    assert_eq!(record_by_id("titles").value, "0");
    // A média do grupo em títulos conta os zeros das não-campeãs.
    assert_eq!(record_by_id("titles").group_average, "0,0");
    // Colocações da temporada: 1º, 2º, 8º e 3º nas quatro corridas — o 8º não
    // entra em nenhum degrau, e os degraus não se sobrepõem.
    let season = dossier.season_results.first().expect("season result");
    assert_eq!(
        (
            season.races,
            season.wins,
            season.seconds,
            season.thirds,
            season.fourths,
            season.fifths,
            season.podiums
        ),
        (4, 1, 1, 1, 0, 0, 3)
    );
    // Fita de forma recente: uma entrada por corrida, da mais antiga para a mais
    // nova, com a colocação de cada uma.
    assert_eq!(
        dossier
            .recent_form
            .iter()
            .map(|race| race.position)
            .collect::<Vec<_>>(),
        vec![Some(1), Some(2), Some(8), Some(3)]
    );
    assert_eq!(dossier.recent_form[0].category_id, "mazda_rookie");
    // Assinatura: as faixas são exclusivas e somam as corridas. O 8º cai em
    // 6º-10º, e não some como caía da faixa de top 5.
    let spread = &dossier.result_spread;
    assert_eq!(
        (
            spread.races,
            spread.first,
            spread.podium,
            spread.near_miss,
            spread.top_ten,
            spread.outside
        ),
        (4, 1, 2, 0, 1, 0)
    );
    // Campanha do campeonato: a equipe do dossiê contra o campo, rodada a
    // rodada. Só as duas equipes com resultado viram linha, e o acumulado soma
    // os DOIS carros de cada uma — 25+12 na primeira, 18+10 na segunda...
    let run = dossier
        .championship_run
        .as_ref()
        .expect("campanha do campeonato");
    assert_eq!(run.rounds.len(), 4);
    assert_eq!(run.lines.len(), 2);
    let minha = run
        .lines
        .iter()
        .find(|line| line.selected)
        .expect("linha da equipe do dossiê");
    assert_eq!(minha.points, vec![37.0, 65.0, 71.0, 94.0]);
    assert_eq!(minha.total, "94");
    // 94 a 93: a ordenação é pela pontuação final, e é ela que dá a colocação.
    assert_eq!(minha.position, 1);
    let rival_line = run
        .lines
        .iter()
        .find(|line| !line.selected)
        .expect("linha do rival");
    assert_eq!(rival_line.points, vec![18.0, 43.0, 68.0, 93.0]);
    assert_eq!(rival_line.position, 2);
    // O nome vem da tabela de equipes: linha sem nome não dá nem para
    // identificar no tooltip do campo cinza.
    assert!(!rival_line.team.is_empty());

    // Tabela de recordes: o destino dos cards. O contrato é que ela e o card
    // saiam do MESMO agregado — o dossiê diz que a equipe é a "2ª" em vitórias
    // num recorte de 2, e a tabela ordenada por vitórias tem de pôr ela em
    // segundo. Duas contagens separadas divergiriam no primeiro empate.
    let ranking = get_team_records_ranking_in_base_dir(&base_dir, "career_001", "mazda_rookie", "group", None)
        .expect("ranking");
    assert_eq!(ranking.scope, "Grupo Mazda");
    assert_eq!(ranking.rows.len(), 2);
    let minha = ranking
        .rows
        .iter()
        .find(|row| row.team_id == selected.id)
        .expect("linha da equipe");
    assert_eq!(
        (
            minha.wins,
            minha.podiums,
            minha.races,
            minha.win_rate,
            minha.podium_rate
        ),
        (1, 3, 4, 25, 75)
    );
    assert!(!minha.team.is_empty());
    let mut por_vitorias: Vec<&TeamRecordsRow> = ranking.rows.iter().collect();
    por_vitorias.sort_by(|a, b| b.wins.cmp(&a.wins));
    assert_eq!(por_vitorias[1].team_id, selected.id);
    // As três amplitudes são três perguntas, e cada uma muda o recorte de fato —
    // não só o rótulo. Só a categoria: apenas a Mazda Rookie.
    assert_eq!(ranking.scope_kind, "group");
    let so_categoria =
        get_team_records_ranking_in_base_dir(&base_dir, "career_001", "mazda_rookie", "category", None)
            .expect("categoria");
    assert_eq!(so_categoria.scope, "Mazda Rookie");
    assert_eq!(so_categoria.scope_kind, "category");
    // A promessa que sustenta a tela: a contagem é do RECORTE, não da carreira.
    // Estas 4 corridas são todas de mazda_rookie, então categoria e grupo dão o
    // mesmo número aqui; o que o teste trava é que a conta sai dos fatos do
    // recorte, e não de um total guardado em outro lugar.
    let minha_categoria = so_categoria
        .rows
        .iter()
        .find(|row| row.team_id == selected.id)
        .expect("linha na categoria");
    assert_eq!(minha_categoria.races, 4);
    // O período vem dos mesmos fatos: os anos das corridas que foram contadas.
    assert!(!minha_categoria.first_year.is_empty());
    assert_eq!(minha_categoria.first_year, minha_categoria.last_year);
    // O par recorte/carreira: aqui as 4 corridas são as únicas do save, então os
    // dois números coincidem e a tela não desenha o segundo. O que o teste trava
    // é que o total existe e é uma conta À PARTE — sem ele, o recorte agiria em
    // silêncio e um "5" solto se pareceria com uma equipe que mal correu.
    assert_eq!(minha_categoria.total_races, minha_categoria.races);
    assert_eq!(minha_categoria.total_wins, minha_categoria.wins);
    // Pedir uma categoria em que a equipe nunca correu não devolve a carreira
    // dela em outro lugar — devolve nada. É o caso que mais importa: era ele que
    // fazia uma equipe da Production aparecer com as vitórias que fez na Mazda.
    let outra_escada =
        get_team_records_ranking_in_base_dir(&base_dir, "career_001", "gt3", "category", None).expect("gt3");
    assert!(outra_escada
        .rows
        .iter()
        .all(|row| row.team_id != selected.id));
    // O grupo junta Rookie e Championship, e é por isso que o rótulo tem de dizer
    // "Grupo": foi tratá-lo como categoria que fez os títulos da Championship
    // aparecerem debaixo de um filtro escrito "Mazda Rookie".
    assert_ne!(so_categoria.scope, ranking.scope);
    assert_eq!(so_categoria.scope_categories, vec!["Mazda Rookie".to_string()]);
    // O Grupo Mazda vai até a Production, que é onde a escada da marca termina —
    // a equipe sobe sem trocar de mundo, o carro continua sendo o mesmo.
    assert_eq!(
        ranking.scope_categories,
        vec![
            "Mazda Rookie".to_string(),
            "Mazda Championship".to_string(),
            "Production".to_string()
        ]
    );
    // Mas só a classe da marca: Toyota e BMW correm a MESMA categoria em
    // campeonatos separados, e nunca dividiram a pista com uma Mazda.
    assert_eq!(ranking.scope_family, "mazda");
    let grupo_toyota =
        get_team_records_ranking_in_base_dir(&base_dir, "career_001", "toyota_rookie", "group", None)
            .expect("grupo toyota");
    assert_eq!(grupo_toyota.scope_family, "toyota");
    // A Production não tem marca própria — é o ponto onde as três escadas
    // convergem —, então o grupo dela segue sendo a convergência inteira, sem
    // recorte de classe.
    let grupo_production =
        get_team_records_ranking_in_base_dir(&base_dir, "career_001", "production_challenger", "group", None)
            .expect("grupo production");
    assert_eq!(grupo_production.scope, "Grupo Production");
    assert_eq!(grupo_production.scope_categories.len(), 6);
    assert_eq!(grupo_production.scope_family, "");
    // O mundo ignora a categoria pedida: a mesma resposta venha de onde vier.
    let mundo = get_team_records_ranking_in_base_dir(&base_dir, "career_001", "mazda_rookie", "world", None)
        .expect("mundo");
    let mundo_pela_gt3 =
        get_team_records_ranking_in_base_dir(&base_dir, "career_001", "gt3", "world", None).expect("mundo gt3");
    assert_eq!(mundo.scope_kind, "world");
    assert_eq!(mundo.rows.len(), mundo_pela_gt3.rows.len());
    // Na amplitude mundial recorte e carreira são a mesma conta por definição, e
    // é por isso que a tela não desenha o segundo número lá.
    let minha_no_mundo = mundo
        .rows
        .iter()
        .find(|row| row.team_id == selected.id)
        .expect("linha no mundo");
    assert_eq!(minha_no_mundo.total_races, minha_no_mundo.races);
    // Amplitude desconhecida cai em grupo, que é a porta por onde a tela abre.
    let padrao = get_team_records_ranking_in_base_dir(&base_dir, "career_001", "mazda_rookie", "xpto", None)
        .expect("padrão");
    assert_eq!(padrao.scope_kind, "group");
    // A escada sai do backend porque é regra de domínio, e traz o grupo de cada
    // categoria junto — é o que deixa a tela dizer o que "grupo" significa AQUI
    // antes de o jogador escolher.
    // 14, e não 10: as duas multiclasse (Production e Endurance) abrem em três
    // entradas cada, uma por carro.
    assert_eq!(ranking.categories.len(), 14);
    let rookie = ranking
        .categories
        .iter()
        .find(|item| item.id == "mazda_rookie")
        .expect("mazda rookie na escada");
    assert_eq!((rookie.label.as_str(), rookie.group_label.as_str()), ("Mazda Rookie", "Grupo Mazda"));
    let championship = ranking
        .categories
        .iter()
        .find(|item| item.id == "mazda_amador")
        .expect("mazda championship na escada");
    assert_eq!(championship.label, "Mazda Championship");

    assert_eq!(dossier.identity.origin, "Mazda Rookie");
    assert_eq!(dossier.identity.current, "Mazda Rookie");
    assert_eq!(dossier.identity.profile, "Dominante");
    assert_eq!(dossier.identity.rival.name, rival.nome);
    assert_eq!(dossier.identity.rival.current_category, "Mazda Rookie");
    assert!(dossier
        .identity
        .rival
        .note
        .contains("4 disputas diretas reais"));
    assert_eq!(
        dossier.identity.symbol_driver,
        driver_name(&db_path, &selected_driver_1)
    );
    assert!(dossier
        .identity
        .symbol_driver_detail
        .contains("4 corridas, 1 vitória, 3 pódios"));
    assert_eq!(dossier.management.peak_cash, "$4,200,000");
    assert_eq!(dossier.management.worst_crisis, "$1,250,000 de dívida");
    assert_eq!(dossier.management.healthy_years, "0 Temporadas");
    assert_eq!(dossier.management.operation_health, "Pressionada");
    assert!(dossier.management.efficiency.contains("pts/temporada"));
    assert!(dossier
        .management
        .efficiency_detail
        .contains("média esportiva"));
    assert_eq!(
        dossier.management.biggest_investment,
        "Nível 7 - pacote técnico atual"
    );
    assert!(dossier.management.summary.contains("Pressionada"));

    // Galeria por vaga: os dois titulares da temporada em curso, um em cada
    // coluna, com os números da PASSAGEM e não da carreira. Ambos seguem na
    // equipe, então ambos são vigentes — o que não pode acontecer é a mesma vaga
    // ter duas passagens marcadas como atuais.
    let lineup = &dossier.lineup;
    assert_eq!(lineup.len(), 2);
    let vaga = |slot: i32| {
        lineup
            .iter()
            .find(|term| term.slot == slot)
            .unwrap_or_else(|| panic!("vaga {slot}"))
    };
    assert_eq!(vaga(1).driver_id, selected_driver_1);
    assert_eq!(vaga(2).driver_id, selected_driver_2);
    assert_eq!((vaga(1).races, vaga(1).wins, vaga(1).podiums), (4, 1, 3));
    // 4º, 5º, 9º e 6º: nenhum pódio, e o melhor resultado é o que sobra de
    // concreto para separar quem chegou perto de quem nunca ameaçou.
    assert_eq!((vaga(2).races, vaga(2).wins, vaga(2).podiums), (4, 0, 0));
    assert_eq!(vaga(2).best_position, 4);
    assert!(vaga(1).still_here && vaga(2).still_here);
    for slot in [1, 2] {
        assert_eq!(
            lineup
                .iter()
                .filter(|term| term.slot == slot && term.still_here)
                .count(),
            1,
            "vaga {slot} não pode ter dois titulares atuais"
        );
    }

    // Confiabilidade: sem `dnf` marcado, as quatro largadas de cada carro viraram
    // chegada — e a taxa do grupo sai da mesma conta para todo mundo.
    assert_eq!(
        (
            dossier.reliability.races,
            dossier.reliability.finished,
            dossier.reliability.finish_rate,
            dossier.reliability.mechanical,
            dossier.reliability.driver_error,
            dossier.reliability.other
        ),
        (8, 8, 100, 0, 0, 0)
    );
    assert_eq!(dossier.reliability.group_finish_rate, 100);

    let _ = fs::remove_dir_all(base_dir);
}

fn driver_name(db_path: &Path, driver_id: &str) -> String {
    let db = Database::open_existing(db_path).expect("db");
    db.conn
        .query_row(
            "SELECT nome FROM drivers WHERE id = ?1",
            rusqlite::params![driver_id],
            |row| row.get::<_, String>(0),
        )
        .expect("driver name")
}

fn team_driver_ids(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<(String, String), rusqlite::Error> {
    conn.query_row(
        "SELECT piloto_1_id, piloto_2_id FROM teams WHERE id = ?1",
        rusqlite::params![team_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )
}

#[test]
fn test_consecutive_team_seasons_up_to_counts_only_current_streak() {
    let mut season_one = crate::models::contract::Contract::new(
        "C001".to_string(),
        "P001".to_string(),
        "Piloto".to_string(),
        "T001".to_string(),
        "Equipe 1".to_string(),
        1,
        1,
        100_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );
    season_one.status = ContractStatus::Expirado;
    let mut season_two = crate::models::contract::Contract::new(
        "C002".to_string(),
        "P001".to_string(),
        "Piloto".to_string(),
        "T001".to_string(),
        "Equipe 1".to_string(),
        2,
        1,
        110_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );
    season_two.status = ContractStatus::Expirado;
    let season_three = crate::models::contract::Contract::new(
        "C003".to_string(),
        "P001".to_string(),
        "Piloto".to_string(),
        "T001".to_string(),
        "Equipe 1".to_string(),
        3,
        2,
        120_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );
    let mut different_team = crate::models::contract::Contract::new(
        "C004".to_string(),
        "P002".to_string(),
        "Piloto 2".to_string(),
        "T001".to_string(),
        "Equipe 1".to_string(),
        1,
        1,
        95_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );
    different_team.status = ContractStatus::Expirado;
    let current_other_team = crate::models::contract::Contract::new(
        "C005".to_string(),
        "P002".to_string(),
        "Piloto 2".to_string(),
        "T002".to_string(),
        "Equipe 2".to_string(),
        3,
        1,
        105_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );

    let veteran_streak =
        consecutive_team_seasons_up_to(&[season_one, season_two, season_three], "T001", 3);
    let newcomer_streak =
        consecutive_team_seasons_up_to(&[different_team, current_other_team], "T002", 3);

    assert_eq!(veteran_streak, Some(3));
    assert_eq!(newcomer_streak, Some(1));
}

#[test]
fn test_get_calendar_for_category_returns_races() {
    let base_dir = create_test_career_dir("calendar_category");
    let races = get_calendar_for_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("calendar");

    assert_eq!(races.len(), 5);
    assert_eq!(races[0].rodada, 1);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_race_results_by_category_returns_round_history_after_simulation() {
    let base_dir = create_test_career_dir("race_history");
    let career_id = "career_001";
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season")
        .expect("active season");
    let next_race = calendar_queries::get_next_race(&db.conn, &season.id, "mazda_rookie")
        .expect("next race")
        .expect("pending race");

    crate::commands::race::simulate_race_weekend_in_base_dir(&base_dir, career_id, &next_race.id)
        .expect("simulate race");

    let histories = get_race_results_by_category_in_base_dir(&base_dir, career_id, "mazda_rookie")
        .expect("race history");

    assert_eq!(histories.len(), 12);
    assert!(histories.iter().all(|history| history.results.len() == 5));
    assert!(histories.iter().any(|history| history.results[0].is_some()));
    assert!(
        histories.iter().any(|history| {
            history
                .results
                .iter()
                .flatten()
                .any(|result| result.has_fastest_lap)
        }),
        "expected persisted race history to retain the fastest-lap marker",
    );
    assert!(
        histories
            .iter()
            .flat_map(|history| history.results.iter().flatten())
            .all(|result| result.grid_position > 0),
        "expected persisted race history to retain grid positions",
    );
    assert!(
        histories
            .iter()
            .flat_map(|history| history.results.iter().flatten())
            .all(|result| result.positions_gained == result.grid_position - result.position),
        "expected persisted race history to retain positions gained",
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_driver_detail_counts_fastest_laps_from_persisted_history() {
    let base_dir = create_test_career_dir("driver_detail_fastest_lap");
    let career_id = "career_001";
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season")
        .expect("active season");
    let next_race = calendar_queries::get_next_race(&db.conn, &season.id, "mazda_rookie")
        .expect("next race")
        .expect("pending race");

    let race_result = crate::commands::race::simulate_race_weekend_in_base_dir(
        &base_dir,
        career_id,
        &next_race.id,
    )
    .expect("simulate race");
    let fastest_lap_driver_id = race_result
        .player_race
        .race_results
        .iter()
        .find(|entry| entry.has_fastest_lap)
        .map(|entry| entry.pilot_id.clone())
        .expect("fastest lap driver");

    let detail = get_driver_detail_in_base_dir(&base_dir, career_id, &fastest_lap_driver_id)
        .expect("driver detail");

    assert_eq!(detail.performance.temporada.voltas_rapidas, Some(1));
    assert_eq!(detail.performance.carreira.voltas_rapidas, Some(1));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_previous_champions_returns_empty_for_first_season() {
    let base_dir = create_test_career_dir("previous_champions");
    let champions = get_previous_champions_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("previous champions");

    assert!(champions.driver_champion_id.is_none());
    assert!(champions.constructor_champions.is_empty());

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_previous_champions_returns_last_season_category_champion() {
    let base_dir = create_test_career_dir("previous_champions_second_season");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let season = season_queries::get_active_season(&db.conn)
        .expect("temporada ativa")
        .expect("temporada ativa existente");

    // Duas corridas da categoria na temporada 1, com um piloto somando mais.
    let races: Vec<String> = db
        .conn
        .prepare(
            "SELECT id FROM calendar WHERE categoria = 'mazda_rookie' ORDER BY rodada ASC LIMIT 2",
        )
        .expect("prepare calendario")
        .query_map([], |row| row.get(0))
        .expect("query calendario")
        .map(|row| row.expect("linha do calendario"))
        .collect();
    assert_eq!(
        races.len(),
        2,
        "categoria deveria ter ao menos duas rodadas"
    );

    let drivers = get_drivers_by_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("grid da categoria");
    let champion_id = drivers[0].id.clone();
    let runner_up_id = drivers[1].id.clone();

    // race_results tem FK para teams, entao o resultado precisa de equipe real.
    // Qual delas nao importa: o campeao sai por soma de pontos por piloto.
    let team_id: String = db
        .conn
        .query_row(
            "SELECT id FROM teams WHERE categoria = 'mazda_rookie' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("equipe da categoria");

    for race_id in &races {
        db.conn
            .execute(
                "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos)
                 VALUES (?1, ?2, ?3, 1, 25.0)",
                rusqlite::params![race_id, champion_id, team_id],
            )
            .expect("resultado do campeao");
        db.conn
            .execute(
                "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos)
                 VALUES (?1, ?2, ?3, 2, 18.0)",
                rusqlite::params![race_id, runner_up_id, team_id],
            )
            .expect("resultado do vice");
    }

    // Encerra a temporada 1 e abre a 2 — o campeão passa a ser "reinante".
    // Nessa ordem: há índice único garantindo uma só temporada em andamento.
    db.conn
        .execute(
            "UPDATE seasons SET status = 'Finalizada' WHERE id = ?1",
            rusqlite::params![season.id],
        )
        .expect("encerra temporada anterior");
    db.conn
        .execute(
            "INSERT INTO seasons (id, numero, ano, status)
             SELECT 'S_TEST_NEXT', numero + 1, ano + 1, 'EmAndamento' FROM seasons WHERE id = ?1",
            rusqlite::params![season.id],
        )
        .expect("cria temporada seguinte");
    drop(db);

    let champions = get_previous_champions_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("previous champions");

    assert_eq!(
        champions.driver_champion_id.as_deref(),
        Some(champion_id.as_str()),
        "campeao reinante deveria ser quem somou mais pontos na temporada anterior"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_driver_detail_returns_contracted_ai_payload() {
    let base_dir = create_test_career_dir("driver_detail_contracted");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season")
        .expect("active season");
    let mut driver = driver_queries::get_drivers_by_category(&db.conn, "mazda_rookie")
        .expect("drivers")
        .into_iter()
        .find(|candidate| !candidate.is_jogador)
        .expect("ai driver");

    driver.atributos.skill = 97.0;
    driver.atributos.gestao_pneus = 20.0;
    driver.motivacao = 82.0;
    driver.melhor_resultado_temp = Some(2);
    driver.stats_temporada.corridas = 3;
    driver.stats_temporada.pontos = 28.0;
    driver.stats_temporada.vitorias = 1;
    driver.stats_temporada.podios = 2;
    driver.stats_temporada.poles = 0;
    driver.stats_temporada.dnfs = 0;
    driver.stats_carreira.corridas = 9;
    driver.stats_carreira.pontos_total = 84.0;
    driver.stats_carreira.vitorias = 2;
    driver.stats_carreira.podios = 4;
    driver.stats_carreira.poles = 1;
    driver.stats_carreira.dnfs = 1;
    driver.stats_carreira.titulos = 2;
    driver_queries::update_driver(&db.conn, &driver).expect("update driver");

    let contract = contract_queries::get_active_contract_for_pilot(&db.conn, &driver.id)
        .expect("active contract")
        .expect("contract");
    let team = team_queries::get_team_by_id(&db.conn, &contract.equipe_id)
        .expect("team query")
        .expect("team");

    let detail =
        get_driver_detail_in_base_dir(&base_dir, "career_001", &driver.id).expect("driver detail");
    let detail_json = serde_json::to_value(&detail).expect("serialize driver detail");

    assert_eq!(detail.id, driver.id);
    assert_eq!(detail.nome, driver.nome);
    assert_eq!(detail.status, "ativo");
    assert_eq!(
        detail.equipe_id.as_deref(),
        Some(contract.equipe_id.as_str())
    );
    assert_eq!(detail.equipe_nome.as_deref(), Some(team.nome.as_str()));
    assert_eq!(
        detail.equipe_cor_primaria.as_deref(),
        Some(team.cor_primaria.as_str())
    );
    assert_eq!(
        detail.equipe_cor_secundaria.as_deref(),
        Some(team.cor_secundaria.as_str())
    );
    assert_eq!(detail.papel.as_deref(), Some(contract.papel.as_str()));
    assert!(detail.personalidade_primaria.is_some());
    assert!(detail.personalidade_secundaria.is_some());
    assert_eq!(detail.motivacao, 82);
    assert_eq!(detail.stats_temporada.corridas, 3);
    assert_eq!(detail.stats_temporada.pontos, 28);
    assert_eq!(detail.stats_temporada.melhor_resultado, 2);
    assert_eq!(detail.stats_carreira.corridas, 9);
    assert_eq!(detail.stats_carreira.pontos, 84);
    assert_eq!(
        detail.contrato.as_ref().map(|value| value.anos_restantes),
        Some(contract.anos_restantes(season.numero))
    );
    assert!(detail.tags.iter().any(|tag| {
        tag.attribute_name == "skill"
            && tag.tag_text == "Alien"
            && tag.level == "elite"
            && tag.color == "#bc8cff"
    }));
    assert!(detail.tags.iter().any(|tag| {
        tag.attribute_name == "gestao_pneus" && tag.level == "defeito" && tag.color == "#db6d28"
    }));
    assert!(
        detail_json.get("perfil").is_some(),
        "expected modular profile block"
    );
    assert!(
        detail_json.get("competitivo").is_some(),
        "expected modular competitive block",
    );
    assert!(
        detail_json.get("performance").is_some(),
        "expected modular performance block",
    );
    assert!(
        detail_json.get("leitura_tecnica").is_some(),
        "expected backend technical-reading block",
    );
    assert_eq!(detail.leitura_tecnica.itens.len(), 4);
    assert!(detail
        .leitura_tecnica
        .itens
        .iter()
        .any(|item| item.chave == "velocidade" && item.nivel == "Elite"));
    assert!(
        detail_json.get("forma").is_some(),
        "expected current-form block"
    );
    assert!(
        detail_json.get("trajetoria").is_some(),
        "expected basic career-path block",
    );
    assert_eq!(detail.trajetoria.titulos, 2);
    assert!(detail.trajetoria.foi_campeao);
    assert!(
        detail_json.get("contrato_mercado").is_some(),
        "expected contract-and-market block",
    );
    assert!(
        detail.contrato_mercado.mercado.is_some(),
        "expected market block to be connected for active drivers",
    );
    assert_eq!(
        detail_json.pointer("/performance/temporada/pontos"),
        None,
        "expected points to stop being a primary dossier metric",
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_driver_detail_marks_active_driver_without_contract_as_livre() {
    let base_dir = create_test_career_dir("driver_detail_free");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let free_driver = Driver::new(
        "P-LIVRE-001".to_string(),
        "Piloto Livre".to_string(),
        "🇧🇷 Brasileiro".to_string(),
        "M".to_string(),
        27,
        2020,
    );
    driver_queries::insert_driver(&db.conn, &free_driver).expect("insert free driver");

    let detail = get_driver_detail_in_base_dir(&base_dir, "career_001", &free_driver.id)
        .expect("driver detail");
    let detail_json = serde_json::to_value(&detail).expect("serialize driver detail");

    assert_eq!(detail.id, free_driver.id);
    assert_eq!(detail.status, "livre");
    assert!(detail.equipe_id.is_none());
    assert!(detail.equipe_nome.is_none());
    assert!(detail.papel.is_none());
    assert!(detail.contrato.is_none());
    assert_eq!(detail.stats_temporada.melhor_resultado, 0);
    assert_eq!(detail.stats_carreira.melhor_resultado, 0);
    assert_eq!(detail.resumo_atual.veredito, "Estreante");
    assert_eq!(detail.resumo_atual.tom, "info");
    assert!(
        detail_json.get("contrato_mercado").is_some(),
        "expected contract/market block to exist structurally",
    );
    assert!(
        detail_json.pointer("/contrato_mercado/mercado").is_some(),
        "expected market data to be connected even for free active drivers",
    );
    assert!(
        detail_json.get("relacionamentos").is_none()
            || detail_json
                .get("relacionamentos")
                .is_some_and(|value| value.is_null()),
        "expected relationships block to stay empty when there is no real data",
    );
    assert!(
        detail_json.get("reputacao").is_none()
            || detail_json
                .get("reputacao")
                .is_some_and(|value| value.is_null()),
        "expected reputation block to stay empty when there is no real data",
    );
    assert!(
        detail_json.get("saude").is_none()
            || detail_json
                .get("saude")
                .is_some_and(|value| value.is_null()),
        "expected health block to stay empty when there is no real data",
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_driver_detail_includes_active_injury_context() {
    let base_dir = create_test_career_dir("driver_detail_active_injury");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season")
        .expect("active season");
    let mut driver = driver_queries::get_drivers_by_category(&db.conn, "mazda_rookie")
        .expect("drivers")
        .into_iter()
        .find(|candidate| !candidate.is_jogador)
        .expect("ai driver");
    driver.status = crate::models::enums::DriverStatus::Lesionado;
    driver_queries::update_driver(&db.conn, &driver).expect("update injured driver");
    let race = calendar_queries::get_calendar(&db.conn, &season.id, "mazda_rookie")
        .expect("calendar")
        .into_iter()
        .next()
        .expect("race");

    let tx = db.conn.unchecked_transaction().expect("injury tx");
    crate::db::queries::injuries::insert_injury(
        &tx,
        &crate::models::injury::Injury {
            id: "I-DETAIL-001".to_string(),
            pilot_id: driver.id.clone(),
            injury_type: crate::models::enums::InjuryType::Moderada,
            injury_name: "".to_string(),
            modifier: 0.88,
            races_total: 4,
            races_remaining: 3,
            skill_penalty: 0.10,
            season: season.numero,
            race_occurred: race.id.clone(),
            active: true,
        },
    )
    .expect("insert active injury");
    crate::db::queries::injuries::insert_injury(
        &tx,
        &crate::models::injury::Injury {
            id: "I-DETAIL-002".to_string(),
            pilot_id: driver.id.clone(),
            injury_type: crate::models::enums::InjuryType::Leve,
            injury_name: "Dor no braço".to_string(),
            modifier: 0.95,
            races_total: 2,
            races_remaining: 0,
            skill_penalty: 0.05,
            season: season.numero,
            race_occurred: race.id.clone(),
            active: false,
        },
    )
    .expect("insert light injury history");
    crate::db::queries::injuries::insert_injury(
        &tx,
        &crate::models::injury::Injury {
            id: "I-DETAIL-003".to_string(),
            pilot_id: driver.id.clone(),
            injury_type: crate::models::enums::InjuryType::Grave,
            injury_name: "Braço fraturado".to_string(),
            modifier: 0.75,
            races_total: 8,
            races_remaining: 0,
            skill_penalty: 0.15,
            season: season.numero,
            race_occurred: race.id.clone(),
            active: false,
        },
    )
    .expect("insert grave injury history");
    tx.commit().expect("commit injury");

    let detail =
        get_driver_detail_in_base_dir(&base_dir, "career_001", &driver.id).expect("driver detail");
    let active_injury = detail
        .saude
        .as_ref()
        .and_then(|health| health.lesao_ativa.as_ref())
        .expect("active injury context");

    assert_eq!(active_injury.tipo, "Moderada");
    assert_eq!(active_injury.nome.as_deref(), Some("Dor forte nas costas"));
    assert_eq!(active_injury.corridas_total, 4);
    assert_eq!(active_injury.corridas_restantes, 3);
    assert_eq!(active_injury.corrida_ocorrida_id, race.id);
    assert_eq!(active_injury.corrida_ocorrida_rodada, Some(race.rodada));
    assert_eq!(
        active_injury.corrida_ocorrida_pista.as_deref(),
        Some(race.track_name.as_str())
    );
    assert_eq!(detail.trajetoria.historico.lesoes.leves, 1);
    assert_eq!(detail.trajetoria.historico.lesoes.moderadas, 1);
    assert_eq!(detail.trajetoria.historico.lesoes.graves, 1);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_advance_season_rejects_pending_races() {
    let base_dir = create_test_career_dir("advance_pending");

    let error =
        advance_season_in_base_dir(&base_dir, "career_001").expect_err("should reject advance");

    assert!(error.contains("corridas pendentes"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_advance_season_rejects_completed_regular_before_special_flow() {
    let base_dir = create_test_career_dir("advance_regular_before_special");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    // Forçar estado legado para testar o fluxo de rejeição BlocoRegular→advance.
    force_legacy_blocoregular_state(&db);
    db.conn
        .execute("UPDATE calendar SET status = 'Concluida'", [])
        .expect("mark calendar completed");

    let error =
        advance_season_in_base_dir(&base_dir, "career_001").expect_err("should reject advance");

    assert!(error.contains("convocacao"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_advance_season_updates_meta_and_creates_next_season() {
    let base_dir = create_test_career_dir("advance_success");
    mark_all_races_completed(&base_dir, "career_001");

    let result =
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season should work");

    assert_eq!(result.new_year, 2025);
    assert!(result.preseason_initialized);
    assert_eq!(
        result.preseason_total_weeks,
        i32::from(crate::constants::timeline::MARKET_DURATION_WEEKS)
    );

    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let active_season = season_queries::get_active_season(&db.conn)
        .expect("active season query")
        .expect("active season");
    let meta = read_save_meta(&config.saves_dir().join("career_001").join("meta.json"))
        .expect("read meta");
    let total_races =
        count_season_calendar_entries(&db.conn, &active_season.id).expect("season race count");
    let distinct_race_ids: i32 = db
        .conn
        .query_row(
            "SELECT COUNT(DISTINCT id) FROM calendar
             WHERE COALESCE(season_id, temporada_id) = ?1",
            rusqlite::params![&active_season.id],
            |row| row.get(0),
        )
        .expect("distinct race ids");

    assert_eq!(active_season.id, result.new_season_id);
    assert_eq!(active_season.numero, 2);
    assert_eq!(active_season.ano, 2025);
    assert_eq!(meta.current_season, 2);
    assert_eq!(meta.current_year, 2025);
    assert_eq!(meta.total_races, total_races);
    assert!(total_races > 0);
    assert_eq!(distinct_race_ids, total_races);
    assert!(config
        .saves_dir()
        .join("career_001")
        .join("preseason_plan.json")
        .exists());
    let resume_context = read_resume_context(&config.saves_dir().join("career_001"))
        .expect("read resume context")
        .expect("resume context");
    assert_eq!(resume_context.active_view, CareerResumeView::EndOfSeason);
    assert_eq!(
        resume_context
            .end_of_season_result
            .as_ref()
            .map(|snapshot| snapshot.new_year),
        Some(result.new_year)
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_advance_season_succeeds_even_if_resume_context_write_fails() {
    let base_dir = create_test_career_dir("advance_resume_context_failure");
    mark_all_races_completed(&base_dir, "career_001");

    let config = AppConfig::load_or_default(&base_dir);
    let save_dir = config.saves_dir().join("career_001");
    fs::create_dir_all(save_dir.join("resume_context.json")).expect("block resume context path");

    let result = advance_season_in_base_dir(&base_dir, "career_001")
        .expect("advance season should still succeed");

    let db_path = save_dir.join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let active_season = season_queries::get_active_season(&db.conn)
        .expect("active season query")
        .expect("active season");

    assert_eq!(result.new_year, 2025);
    assert_eq!(active_season.numero, 2);
    assert_eq!(active_season.ano, 2025);
    assert!(save_dir.join("resume_context.json").is_dir());

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_preseason_state_returns_initialized_state() {
    let base_dir = create_test_career_dir("preseason_state");
    mark_all_races_completed(&base_dir, "career_001");

    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let state = get_preseason_state_in_base_dir(&base_dir, "career_001").expect("preseason state");

    assert_eq!(state.current_week, 1);
    assert!(!state.is_complete);
    assert_eq!(
        state.total_weeks,
        i32::from(crate::constants::timeline::MARKET_DURATION_WEEKS)
    );
    assert!(
        state.current_display_date.is_some(),
        "preseason state should expose a simulation date",
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn advance_season_9d_cycle_opens_preseason_with_full_calendar_then_racing_resumes() {
    let base_dir = create_test_career_dir("advance_9d_cycle");

    skip_all_pending_races_in_base_dir(&base_dir, "career_001").expect("skip season");
    let config = AppConfig::load_or_default(&base_dir);
    let save_dir = config.saves_dir().join("career_001");
    let db_path = save_dir.join("career.db");
    let db = Database::open_existing(&db_path).expect("db after skip");
    let closed_season = season_queries::get_active_season(&db.conn)
        .expect("closed season query")
        .expect("closed active season");
    assert_eq!(closed_season.fase, SeasonPhase::Encerramento);
    drop(db);

    let result = advance_season_in_base_dir(&base_dir, "career_001").expect("advance to season 2");

    let db = Database::open_existing(&db_path).expect("db after advance");
    let active_count: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM seasons WHERE status IN ('EmAndamento', 'Ativa')",
            [],
            |row| row.get(0),
        )
        .expect("active season count");
    let season = season_queries::get_active_season(&db.conn)
        .expect("active season query")
        .expect("active season");
    assert_eq!(active_count, 1);
    assert_eq!(result.new_season_id, season.id);
    assert_eq!(season.numero, 2);
    assert_eq!(season.fase, SeasonPhase::PreTemporada);

    let entries =
        calendar_queries::get_pending_races(&db.conn, &season.id).expect("pending calendar");
    assert_eq!(entries.len(), 74);
    assert!(entries.iter().all(|entry| {
        entry.status == crate::models::enums::RaceStatus::Pendente
            && matches!(entry.season_week, Some(10..=51))
    }));
    drop(db);

    let preseason =
        get_preseason_state_in_base_dir(&base_dir, "career_001").expect("preseason state");
    assert_eq!(preseason.current_week, 1);
    assert!(!preseason.is_complete);

    force_complete_preseason_plan(&save_dir);
    finalize_preseason_in_base_dir(&base_dir, "career_001").expect("finalize preseason");

    let db = Database::open_existing(&db_path).expect("db after finalize");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season after finalize")
        .expect("active season");
    assert_eq!(season.fase, SeasonPhase::Temporada);
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    if contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
        .expect("active player contract query")
        .is_none()
    {
        let team = team_queries::get_teams_by_category(&db.conn, "mazda_rookie")
            .expect("rookie teams")
            .into_iter()
            .next()
            .expect("rookie team");
        if let Some(displaced_driver_id) = team.piloto_2_id.as_deref() {
            if let Some(displaced_contract) =
                contract_queries::get_active_regular_contract_for_pilot(
                    &db.conn,
                    displaced_driver_id,
                )
                .expect("displaced contract query")
            {
                contract_queries::update_contract_status(
                    &db.conn,
                    &displaced_contract.id,
                    &ContractStatus::Rescindido,
                )
                .expect("rescind displaced driver contract");
            }
            db.conn
                .execute(
                    "UPDATE drivers SET categoria_atual = NULL WHERE id = ?1",
                    rusqlite::params![displaced_driver_id],
                )
                .expect("clear displaced driver category");
        }
        let mut contract = crate::models::contract::generate_initial_contract(
            next_id(&db.conn, IdType::Contract).expect("contract id"),
            &player.id,
            &player.nome,
            &team.id,
            &team.nome,
            TeamRole::Numero2,
            "mazda_rookie",
            season.numero,
        );
        contract.classe = team.classe.clone();
        contract_queries::insert_contract(&db.conn, &contract)
            .expect("insert player test contract");
        team_queries::update_team_pilots(
            &db.conn,
            &team.id,
            team.piloto_1_id.as_deref(),
            Some(&player.id),
        )
        .expect("place player in test team");
    }
    let contract = contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
        .expect("active player contract")
        .expect("player should have regular contract");
    let first_race = calendar_queries::get_next_race(&db.conn, &season.id, &contract.categoria)
        .expect("next race")
        .expect("first race");
    drop(db);

    crate::commands::race::simulate_race_weekend_in_base_dir(
        &base_dir,
        "career_001",
        &first_race.id,
    )
    .expect("first race of season 2 should simulate");

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn advance_season_9d_rejects_pending_races_without_panicking() {
    let base_dir = create_test_career_dir("advance_9d_pending_gate");

    let error = advance_season_in_base_dir(&base_dir, "career_001")
        .expect_err("advance should reject pending races");

    assert!(error.contains("corridas pendentes"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn advance_season_9d_rejects_preseason() {
    let base_dir = create_test_career_dir("advance_9d_preseason_gate");
    skip_all_pending_races_in_base_dir(&base_dir, "career_001").expect("skip season");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance to preseason");

    let error = advance_season_in_base_dir(&base_dir, "career_001")
        .expect_err("advance should reject preseason");

    assert!(error.contains("temporada ainda nao comecou"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn advance_season_legacy_pos_especial_crosses_to_9d_preseason_and_cleans_special_state() {
    let base_dir = create_test_career_dir("advance_legacy_to_9d");
    let config = AppConfig::load_or_default(&base_dir);
    let save_dir = config.saves_dir().join("career_001");
    let db_path = save_dir.join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let special_team = team_queries::get_teams_by_category(&db.conn, "production_challenger")
        .expect("production teams")
        .into_iter()
        .next()
        .expect("production team");
    let mut special_contract = crate::models::contract::Contract::new(
        "C_LEGACY_SPECIAL".to_string(),
        player.id.clone(),
        player.nome.clone(),
        special_team.id.clone(),
        special_team.nome.clone(),
        1,
        1,
        80_000.0,
        TeamRole::Numero1,
        "production_challenger".to_string(),
    );
    special_contract.tipo = crate::models::enums::ContractType::Especial;
    special_contract.classe = special_team.classe.clone();
    contract_queries::insert_contract(&db.conn, &special_contract)
        .expect("insert legacy special contract");
    db.conn
        .execute(
            "UPDATE drivers SET categoria_especial_ativa = 'production_challenger'
             WHERE id = ?1",
            rusqlite::params![&player.id],
        )
        .expect("set legacy special category");
    db.conn
        .execute("UPDATE calendar SET status = 'Concluida'", [])
        .expect("complete calendar");
    db.conn
        .execute(
            "UPDATE seasons SET fase = 'PosEspecial' WHERE status = 'EmAndamento'",
            [],
        )
        .expect("set legacy pos especial");
    drop(db);

    advance_season_in_base_dir(&base_dir, "career_001").expect("advance legacy season");

    let db = Database::open_existing(&db_path).expect("db after advance");
    let season = season_queries::get_active_season(&db.conn)
        .expect("active season query")
        .expect("active season");
    assert_eq!(season.numero, 2);
    assert_eq!(season.fase, SeasonPhase::PreTemporada);

    let active_special_contracts: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM contracts WHERE tipo = 'Especial' AND status = 'Ativo'",
            [],
            |row| row.get(0),
        )
        .expect("active special contracts");
    let special_marks: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM drivers WHERE categoria_especial_ativa IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .expect("special marks");
    assert_eq!(active_special_contracts, 0);
    assert_eq!(special_marks, 0);

    let entries = calendar_queries::get_pending_races(&db.conn, &season.id)
        .expect("new season pending calendar");
    assert_eq!(entries.len(), 74);
    assert!(entries
        .iter()
        .all(|entry| matches!(entry.season_week, Some(10..=51))));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_advance_market_week_updates_plan_state() {
    let base_dir = create_test_career_dir("advance_market_week");
    mark_all_races_completed(&base_dir, "career_001");

    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let initial_state =
        get_preseason_state_in_base_dir(&base_dir, "career_001").expect("preseason state");
    let initial_date = initial_state
        .current_display_date
        .as_deref()
        .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
        .expect("valid initial preseason date");

    let week = advance_market_week_in_base_dir(&base_dir, "career_001", None)
        .expect("advance market week");
    let state = get_preseason_state_in_base_dir(&base_dir, "career_001").expect("preseason state");
    let advanced_date = state
        .current_display_date
        .as_deref()
        .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
        .expect("valid advanced preseason date");

    assert_eq!(week.week_number, 1);
    // (enriquecer o feed com championship_position é polish de Fase 3.)
    assert!(state.current_week >= 2 || state.is_complete);
    assert_eq!(
        advanced_date.signed_duration_since(initial_date).num_days(),
        7,
        "advancing the preseason should move the simulated date by one week",
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_preseason_dates_stay_inside_december_to_february_window() {
    let base_dir = create_test_career_dir("preseason_market_window");
    mark_all_races_completed(&base_dir, "career_001");

    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("active season query")
        .expect("active season");
    let mut state =
        get_preseason_state_in_base_dir(&base_dir, "career_001").expect("preseason state");
    let mut dates = Vec::new();

    // Semanas VARIÁVEIS (janela define o fim): avança até fechar, exigindo que toda
    // data fique dentro da janela Dez(ano-1) → Jan/Fev(ano) e cresça a cada semana.
    let mut guard = 0;
    while !state.is_complete {
        let current_date = state
            .current_display_date
            .as_deref()
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
            .expect("valid preseason date");
        let in_december = current_date.year() == season.ano - 1 && current_date.month() == 12;
        let in_jan_feb = current_date.year() == season.ano && matches!(current_date.month(), 1 | 2);
        assert!(
            in_december || in_jan_feb,
            "data da pre-temporada fora da janela Dez-Fev: {current_date}"
        );
        dates.push(current_date);

        advance_market_week_in_base_dir(&base_dir, "career_001", None)
            .expect("advance market week");
        state = get_preseason_state_in_base_dir(&base_dir, "career_001").expect("preseason state");
        guard += 1;
        assert!(guard < 30, "a janela deve fechar em tempo razoavel");
    }
    assert!(state.is_complete);
    // Não-decrescente: a data avança a cada semana e, se a janela passar do teto
    // de exibição (9 sem.), faz platô na última data (artefato de display benigno).
    for pair in dates.windows(2) {
        assert!(
            pair[1] >= pair[0],
            "preseason dates should not go backwards: {:?}",
            pair
        );
    }
    // Houve progresso geral ao longo da janela.
    if dates.len() > 1 {
        assert!(
            dates.last().unwrap() > dates.first().unwrap(),
            "as datas da pre-temporada devem avancar ao longo da janela: {:?}",
            dates
        );
    }

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_preseason_free_agents_payload_keeps_regular_history_when_special_exists() {
    let base_dir = create_test_career_dir("preseason_free_agents_regular_history");
    mark_all_races_completed(&base_dir, "career_001");

    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");

    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let regular_team = team_queries::get_teams_by_category(&db.conn, "mazda_amador")
        .expect("regular teams")
        .into_iter()
        .next()
        .expect("regular team");
    let special_team = team_queries::get_teams_by_category(&db.conn, "mazda_amador")
        .expect("special entry teams")
        .into_iter()
        .next()
        .expect("special entry team");

    let mut driver = Driver::new(
        "P-PRESEASON-SPECIAL-001".to_string(),
        "Piloto Historico".to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        26,
        2021,
    );
    driver.status = DriverStatus::Ativo;
    driver.categoria_atual = Some("mazda_amador".to_string());
    driver_queries::insert_driver(&db.conn, &driver).expect("insert driver");

    let mut regular_contract = crate::models::contract::Contract::new(
        next_id(&db.conn, IdType::Contract).expect("regular contract id"),
        driver.id.clone(),
        driver.nome.clone(),
        regular_team.id.clone(),
        regular_team.nome.clone(),
        2,
        3,
        80_000.0,
        TeamRole::Numero1,
        "mazda_amador".to_string(),
    );
    regular_contract.status = ContractStatus::Expirado;
    regular_contract.created_at = "2026-01-01T08:00:00".to_string();
    contract_queries::insert_contract(&db.conn, &regular_contract).expect("insert regular");

    let mut special_contract = contract_queries::generate_especial_contract(
        next_id(&db.conn, IdType::Contract).expect("special contract id"),
        &driver.id,
        &driver.nome,
        &special_team.id,
        &special_team.nome,
        TeamRole::Numero2,
        "production_challenger",
        "mazda",
        4,
    );
    special_contract.status = ContractStatus::Expirado;
    special_contract.created_at = "2026-06-01T08:00:00".to_string();
    contract_queries::insert_contract(&db.conn, &special_contract).expect("insert special");
    db.conn
        .execute(
            "INSERT OR REPLACE INTO driver_season_archive
             (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                &driver.id,
                1,
                2025,
                &driver.nome,
                "mazda_amador",
                12,
                95.0,
                serde_json::json!({
                    "total_pilotos": 20
                })
                .to_string()
            ],
        )
        .expect("insert archive");

    let free_agents =
        get_preseason_free_agents_in_base_dir(&base_dir, "career_001").expect("free agents");
    let preview = free_agents
        .into_iter()
        .find(|item| item.driver_id == driver.id)
        .expect("driver preview");

    assert_eq!(preview.categoria, "mazda_amador");
    assert_eq!(
        preview.previous_team_name.as_deref(),
        Some(regular_team.nome.as_str())
    );
    assert_eq!(
        preview.previous_team_color.as_deref(),
        Some(regular_team.cor_primaria.as_str())
    );
    assert_eq!(preview.seasons_at_last_team, 3);
    assert_eq!(preview.total_career_seasons, 3);
    assert_eq!(preview.last_championship_position, Some(12));
    assert_eq!(preview.last_championship_total_drivers, Some(20));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_advance_season_clears_current_standings_results_and_archives_previous_season() {
    let base_dir = create_test_career_dir("advance_archives_recent_results");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let mut player = driver_queries::get_player_driver(&db.conn).expect("player");
    player.stats_temporada.corridas = 3;
    player.stats_temporada.pontos = 41.0;
    player.stats_temporada.vitorias = 1;
    player.stats_temporada.podios = 2;
    player.ultimos_resultados = serde_json::json!([
        { "position": 9, "is_dnf": false },
        { "position": 5, "is_dnf": false },
        { "position": 1, "is_dnf": false }
    ]);
    driver_queries::update_driver(&db.conn, &player).expect("update player");

    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");

    let refreshed_player_record = driver_queries::get_player_driver(&db.conn).expect("player");
    let detail_after_advance = get_driver_detail_in_base_dir(&base_dir, "career_001", &player.id)
        .expect("driver detail after advance");
    let snapshot_json: String = db
        .conn
        .query_row(
            "SELECT snapshot_json
             FROM driver_season_archive
             WHERE piloto_id = ?1 AND season_number = 1",
            rusqlite::params![&player.id],
            |row| row.get(0),
        )
        .expect("archived season snapshot");
    let snapshot: serde_json::Value =
        serde_json::from_str(&snapshot_json).expect("valid snapshot json");

    assert!(
        refreshed_player_record.ultimos_resultados == serde_json::json!([]),
        "new season player record should not keep previous season recent results"
    );
    assert_eq!(
        detail_after_advance.forma.ultimas_5.len(),
        3,
        "driver detail should keep reading recent form from the previous season archive"
    );
    assert_eq!(
        detail_after_advance.forma.ultimas_10.len(),
        3,
        "driver detail should expose archived recent form in the 10-race chart payload"
    );
    assert_eq!(detail_after_advance.forma.ultimas_5[0].chegada, Some(9));
    assert_eq!(detail_after_advance.forma.ultimas_5[1].chegada, Some(5));
    assert_eq!(detail_after_advance.forma.ultimas_5[2].chegada, Some(1));
    assert_eq!(detail_after_advance.forma.media_chegada, Some(5.0));
    assert_eq!(
        refreshed_player_record.stats_temporada.corridas, 0,
        "new season player record should reset season race count"
    );
    assert_eq!(
        snapshot["ultimos_resultados"],
        serde_json::json!([
            { "position": 9, "is_dnf": false },
            { "position": 5, "is_dnf": false },
            { "position": 1, "is_dnf": false }
        ]),
        "snapshot should preserve ultimos_resultados from the archived season"
    );
    assert_eq!(snapshot["corridas"], 3, "snapshot should preserve corridas");
    assert!(
        snapshot["atributos"]["skill"].is_number(),
        "snapshot should include atributos"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_news_filters_by_season_and_type() {
    let base_dir = create_test_career_dir("get_news_filters");
    mark_all_races_completed(&base_dir, "career_001");

    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    advance_market_week_in_base_dir(&base_dir, "career_001", None).expect("advance market week");

    // news generation is now stubbed; just check the query runs without error
    let _ = get_news_in_base_dir(&base_dir, "career_001", Some(1), None, Some(50)).expect("news");
    let _ = get_news_in_base_dir(&base_dir, "career_001", Some(2), Some("Mercado"), Some(50))
        .expect("market news");

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_news_rejects_invalid_type_filter() {
    let base_dir = create_test_career_dir("get_news_invalid_type");
    let error = get_news_in_base_dir(
        &base_dir,
        "career_001",
        Some(1),
        Some("TipoInvalido"),
        Some(50),
    )
    .expect_err("invalid news type should fail");

    assert!(error.contains("NewsType"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_is_team_role_vacant_rejects_invalid_role() {
    let base_dir = create_test_career_dir("invalid_team_role_vacancy");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let error = is_team_role_vacant(&db.conn, "T001", "PapelInvalido")
        .expect_err("invalid role should fail");

    assert!(error.contains("TeamRole"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_finalize_preseason_rejects_incomplete_plan() {
    let base_dir = create_test_career_dir("finalize_preseason_incomplete");
    mark_all_races_completed(&base_dir, "career_001");

    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let error = finalize_preseason_in_base_dir(&base_dir, "career_001")
        .expect_err("should reject incomplete preseason");

    assert!(error.contains("nao foi concluida"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_player_proposals_returns_pending_only() {
    let base_dir = create_test_career_dir("player_proposals_pending_only");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    seed_player_proposal(&db.conn, &season.id, &player.id, "T001", "Pendente");
    seed_player_proposal(&db.conn, &season.id, &player.id, "T002", "Recusada");

    let proposals =
        get_player_proposals_in_base_dir(&base_dir, "career_001").expect("player proposals");

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].status, "Pendente");

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_player_proposals_enriched_with_team_data() {
    let base_dir = create_test_career_dir("player_proposals_enriched");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    seed_player_proposal(&db.conn, &season.id, &player.id, "T001", "Pendente");

    let proposals =
        get_player_proposals_in_base_dir(&base_dir, "career_001").expect("player proposals");

    assert!(!proposals.is_empty());
    assert!(!proposals[0].equipe_nome.is_empty());
    assert!(!proposals[0].categoria_nome.is_empty());
    assert!(proposals[0].car_performance_rating <= 100);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_accept_proposal_creates_contract_and_expires_other_proposals() {
    let base_dir = create_test_career_dir("accept_proposal");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    seed_player_proposal(&db.conn, &season.id, &player.id, "T001", "Pendente");
    seed_player_proposal(&db.conn, &season.id, &player.id, "T002", "Pendente");

    let response = respond_to_proposal_in_base_dir(&base_dir, "career_001", "MP-T001-P001", true)
        .expect("accept proposal");

    assert!(response.success);
    assert_eq!(response.action, "accepted");

    let refreshed_db = Database::open_existing(&db_path).expect("db reopen");
    let active_contract =
        contract_queries::get_active_contract_for_pilot(&refreshed_db.conn, &player.id)
            .expect("active contract")
            .expect("contract");
    assert_eq!(active_contract.equipe_id, "T001");
    let expired = crate::db::queries::market_proposals::get_market_proposal_by_id(
        &refreshed_db.conn,
        &season.id,
        "MP-T002-P001",
    )
    .expect("proposal query")
    .expect("proposal");
    assert_eq!(expired.status.as_str(), "Expirada");

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_accept_proposal_replaces_only_regular_contract_when_special_exists() {
    let base_dir = create_test_career_dir("accept_proposal_with_special_residue");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    let special_team = insert_test_endurance_team(&db.conn);

    let special_contract = contract_queries::generate_especial_contract(
        next_id(&db.conn, IdType::Contract).expect("special contract id"),
        &player.id,
        &player.nome,
        &special_team.id,
        &special_team.nome,
        TeamRole::Numero1,
        "endurance",
        special_team.classe.as_deref().unwrap_or("gt4"),
        season.numero,
    );
    contract_queries::insert_contract(&db.conn, &special_contract).expect("insert special");
    driver_queries::update_driver_especial_category(&db.conn, &player.id, Some("endurance"))
        .expect("set special category");
    team_queries::update_team_pilots(&db.conn, &special_team.id, Some(&player.id), None)
        .expect("set special lineup");

    seed_player_proposal(&db.conn, &season.id, &player.id, "T001", "Pendente");

    let response = respond_to_proposal_in_base_dir(&base_dir, "career_001", "MP-T001-P001", true)
        .expect("accept proposal");

    assert!(response.success);

    let refreshed_db = Database::open_existing(&db_path).expect("db reopen");
    let active_regular =
        contract_queries::get_active_regular_contract_for_pilot(&refreshed_db.conn, &player.id)
            .expect("regular contract query")
            .expect("active regular contract");
    let active_regular_count: i64 = refreshed_db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM contracts
             WHERE piloto_id = ?1 AND status = 'Ativo' AND tipo = 'Regular'",
            rusqlite::params![&player.id],
            |row| row.get(0),
        )
        .expect("count active regular contracts");

    assert_eq!(active_regular.equipe_id, "T001");
    assert_eq!(active_regular_count, 1);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_accept_proposal_to_full_team_replaces_incumbent_instead_of_creating_third_driver() {
    let base_dir = create_test_career_dir("accept_proposal_replaces_full_team_incumbent");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let mut player = driver_queries::get_player_driver(&db.conn).expect("player");
    player.atributos.skill = 1.0;
    driver_queries::update_driver(&db.conn, &player).expect("downgrade player skill");

    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    let current_contract = latest_regular_contract_for_driver(&db.conn, &player.id);
    let target_team_seed =
        team_queries::get_teams_by_category(&db.conn, &current_contract.categoria)
            .expect("teams by category")
            .into_iter()
            .find(|team| team.id != current_contract.equipe_id)
            .expect("target team");
    backfill_team_vacancy(&db.conn, &target_team_seed.id, season.numero, season.ano)
        .expect("first target vacancy backfill");
    backfill_team_vacancy(&db.conn, &target_team_seed.id, season.numero, season.ano)
        .expect("second target vacancy backfill");
    let target_team = team_queries::get_team_by_id(&db.conn, &target_team_seed.id)
        .expect("target team query")
        .expect("target team");
    assert!(
        target_team.piloto_1_id.is_some() && target_team.piloto_2_id.is_some(),
        "target team should be explicitly filled before accepting the proposal"
    );
    // Qual dos dois titulares perde a vaga depende do papel gravado no contrato,
    // nao da hierarquia por skill do lineup — entao o teste checa que exatamente
    // um deles saiu, sem fixar qual.
    let incumbent_ids = [
        target_team
            .piloto_1_id
            .clone()
            .expect("full target team should have n1 incumbent"),
        target_team
            .piloto_2_id
            .clone()
            .expect("full target team should have n2 incumbent"),
    ];

    seed_player_proposal(
        &db.conn,
        &season.id,
        &player.id,
        &target_team.id,
        "Pendente",
    );

    respond_to_proposal_in_base_dir(
        &base_dir,
        "career_001",
        &format!("MP-{}-{}", target_team.id, player.id),
        true,
    )
    .expect("accept proposal");

    let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
    let refreshed_db = Database::open_existing(&db_path).expect("db reopen");
    let refreshed_target_team = team_queries::get_team_by_id(&refreshed_db.conn, &target_team.id)
        .expect("query target team")
        .expect("target team");
    let target_contracts =
        contract_queries::get_active_contracts_for_team(&refreshed_db.conn, &target_team.id)
            .expect("target team contracts")
            .into_iter()
            .filter(|contract| contract.tipo == crate::models::enums::ContractType::Regular)
            .collect::<Vec<_>>();
    let incumbents_still_at_team = incumbent_ids
        .iter()
        .filter(|driver_id| {
            contract_queries::get_active_regular_contract_for_pilot(&refreshed_db.conn, driver_id)
                .expect("incumbent contract query")
                .is_some_and(|contract| contract.equipe_id == target_team.id)
        })
        .count();
    let player_team = career.player_team.as_ref().expect("player team");

    assert_eq!(player_team.id, target_team.id);
    assert_eq!(target_contracts.len(), 2);
    assert!(
        refreshed_target_team.piloto_1_id.as_deref() == Some(player.id.as_str())
            || refreshed_target_team.piloto_2_id.as_deref() == Some(player.id.as_str()),
        "accepted player should remain in the target lineup after consistency repair"
    );
    assert_eq!(
        incumbents_still_at_team, 1,
        "exactly one incumbent should keep an active regular contract for the target team after the player takes a seat"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_accept_proposal_rejects_team_without_required_license() {
    let base_dir = create_test_career_dir("accept_proposal_without_required_license");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    let invalid_team = team_queries::get_teams_by_category(&db.conn, "gt4")
        .expect("gt4 teams")
        .into_iter()
        .next()
        .expect("gt4 team");

    seed_player_proposal(
        &db.conn,
        &season.id,
        &player.id,
        &invalid_team.id,
        "Pendente",
    );

    let error = respond_to_proposal_in_base_dir(
        &base_dir,
        "career_001",
        &format!("MP-{}-{}", invalid_team.id, player.id),
        true,
    )
    .expect_err("accept proposal should fail without required license");

    assert!(error.to_lowercase().contains("licenc"));

    let refreshed_db = Database::open_existing(&db_path).expect("db reopen");
    let active_regular =
        contract_queries::get_active_regular_contract_for_pilot(&refreshed_db.conn, &player.id);
    let active_regular = active_regular.expect("regular contract query");
    assert!(active_regular
        .as_ref()
        .is_none_or(|contract| contract.equipe_id != invalid_team.id));

    let invalid_team_contracts: i64 = refreshed_db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM contracts
             WHERE piloto_id = ?1 AND status = 'Ativo' AND tipo = 'Regular' AND equipe_id = ?2",
            rusqlite::params![&player.id, &invalid_team.id],
            |row| row.get(0),
        )
        .expect("count invalid team contracts");
    assert_eq!(invalid_team_contracts, 0);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_accept_proposal_removes_pending_player_events_from_preseason_plan() {
    let base_dir = create_test_career_dir("accept_proposal_clears_pending_player_events");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let config = AppConfig::load_or_default(&base_dir);
    let save_dir = config.saves_dir().join("career_001");
    let db_path = save_dir.join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    let current_contract = latest_regular_contract_for_driver(&db.conn, &player.id);
    let gt4_team = team_queries::get_teams_by_category(&db.conn, "gt4")
        .expect("gt4 teams")
        .into_iter()
        .find(|team| team.id != current_contract.equipe_id)
        .expect("gt4 team");

    seed_player_proposal(
        &db.conn,
        &season.id,
        &player.id,
        &current_contract.equipe_id,
        "Pendente",
    );

    let mut plan = crate::market::preseason::load_preseason_plan(&save_dir)
        .expect("load plan")
        .expect("preseason plan");
    plan.planned_events.push(PlannedEvent {
        week: 2,
        executed: false,
        event: PendingAction::ExpireContract {
            contract_id: current_contract.id.clone(),
            driver_id: player.id.clone(),
            driver_name: player.nome.clone(),
            team_id: current_contract.equipe_id.clone(),
            team_name: current_contract.equipe_nome.clone(),
        },
    });
    plan.planned_events.push(PlannedEvent {
        week: 3,
        executed: false,
        event: PendingAction::Transfer {
            driver_id: player.id.clone(),
            driver_name: player.nome.clone(),
            from_team_id: Some(current_contract.equipe_id.clone()),
            from_team_name: Some(current_contract.equipe_nome.clone()),
            from_categoria: Some(current_contract.categoria.clone()),
            to_team_id: gt4_team.id.clone(),
            to_team_name: gt4_team.nome.clone(),
            salary: 120_000.0,
            duration: 1,
            role: TeamRole::Numero2.as_str().to_string(),
        },
    });
    save_preseason_plan(&save_dir, &plan).expect("save mutated plan");

    let response = respond_to_proposal_in_base_dir(
        &base_dir,
        "career_001",
        &format!("MP-{}-{}", current_contract.equipe_id, player.id),
        true,
    )
    .expect("accept proposal");

    assert!(response.success);

    let plan = crate::market::preseason::load_preseason_plan(&save_dir)
        .expect("reload plan")
        .expect("preseason plan");
    assert!(
        !plan.planned_events.iter().any(|event| {
            !event.executed
                && matches!(
                    &event.event,
                    PendingAction::ExpireContract { driver_id, .. }
                        | PendingAction::RenewContract { driver_id, .. }
                        | PendingAction::Transfer { driver_id, .. }
                        if driver_id == &player.id
                )
        }),
        "nenhum evento pendente do jogador deve sobreviver apos aceitar proposta"
    );
    assert!(
        !plan.planned_events.iter().any(|event| {
            !event.executed
                && matches!(
                    &event.event,
                    PendingAction::PlayerProposal { proposal } if proposal.piloto_id == player.id
                )
        }),
        "nenhuma proposta futura do jogador deve continuar pendente no plano"
    );

    let refreshed_db = Database::open_existing(&db_path).expect("db reopen");
    let active_regular =
        contract_queries::get_active_regular_contract_for_pilot(&refreshed_db.conn, &player.id)
            .expect("regular contract query")
            .expect("active regular contract");

    assert_eq!(active_regular.equipe_id, current_contract.equipe_id);
    assert_eq!(active_regular.categoria, current_contract.categoria);

    let gt4_contracts: i64 = refreshed_db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM contracts
             WHERE piloto_id = ?1 AND status = 'Ativo' AND tipo = 'Regular' AND equipe_id = ?2",
            rusqlite::params![&player.id, &gt4_team.id],
            |row| row.get(0),
        )
        .expect("count gt4 contracts");
    assert_eq!(gt4_contracts, 0);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_accept_proposal_removes_stale_place_rookie_for_accepted_team_role() {
    let base_dir = create_test_career_dir("accept_proposal_clears_backfilled_rookie");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let config = AppConfig::load_or_default(&base_dir);
    let save_dir = config.saves_dir().join("career_001");
    let db_path = save_dir.join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");

    if contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
        .expect("active regular contract query")
        .is_none()
    {
        let mut news_items = Vec::new();
        force_place_player(&db.conn, &player, &season, &mut news_items)
            .expect("force place player");
    }

    let current_contract = latest_regular_contract_for_driver(&db.conn, &player.id);
    let target_team = team_queries::get_teams_by_category(&db.conn, &current_contract.categoria)
        .expect("teams by category")
        .into_iter()
        .find(|team| team.id != current_contract.equipe_id)
        .expect("target team");
    seed_player_proposal(
        &db.conn,
        &season.id,
        &player.id,
        &target_team.id,
        "Pendente",
    );

    let mut plan = crate::market::preseason::load_preseason_plan(&save_dir)
        .expect("load plan")
        .expect("preseason plan");
    plan.planned_events.push(PlannedEvent {
        week: 4,
        executed: false,
        event: PendingAction::PlaceRookie {
            driver: Driver::new(
                "P-PLAN-ROOKIE".to_string(),
                "Rookie de Plano".to_string(),
                "🇧🇷 Brasileiro".to_string(),
                "M".to_string(),
                18,
                2025,
            ),
            team_id: target_team.id.clone(),
            team_name: target_team.nome.clone(),
            salary: 22_000.0,
            duration: 1,
            role: TeamRole::Numero1.as_str().to_string(),
        },
    });
    save_preseason_plan(&save_dir, &plan).expect("save mutated plan");

    let response = respond_to_proposal_in_base_dir(
        &base_dir,
        "career_001",
        &format!("MP-{}-{}", target_team.id, player.id),
        true,
    )
    .expect("accept proposal");

    assert!(response.success);

    let plan = crate::market::preseason::load_preseason_plan(&save_dir)
        .expect("reload plan")
        .expect("preseason plan");
    assert!(
        !plan.planned_events.iter().any(|event| {
            !event.executed
                && matches!(
                    &event.event,
                    PendingAction::PlaceRookie { team_id, role, .. }
                        if team_id == &target_team.id
                            && role == TeamRole::Numero1.as_str()
                )
        }),
        "a vaga preenchida pelo aceite nao deve manter PlaceRookie pendente"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_reject_proposal_marks_recusada_and_generates_news() {
    let base_dir = create_test_career_dir("reject_proposal");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    seed_player_proposal(&db.conn, &season.id, &player.id, "T001", "Pendente");

    let response = respond_to_proposal_in_base_dir(&base_dir, "career_001", "MP-T001-P001", false)
        .expect("reject proposal");

    assert!(response.success);
    assert_eq!(response.action, "rejected");

    let refreshed_db = Database::open_existing(&db_path).expect("db reopen");
    let proposal = crate::db::queries::market_proposals::get_market_proposal_by_id(
        &refreshed_db.conn,
        &season.id,
        "MP-T001-P001",
    )
    .expect("proposal query")
    .expect("proposal");
    assert_eq!(proposal.status.as_str(), "Recusada");

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_finalize_allows_player_without_team_when_plan_is_resolved() {
    let base_dir = create_test_career_dir("finalize_without_team");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    if let Some(contract) = contract_queries::get_active_contract_for_pilot(&db.conn, &player.id)
        .expect("active contract")
    {
        contract_queries::update_contract_status(
            &db.conn,
            &contract.id,
            &crate::models::enums::ContractStatus::Rescindido,
        )
        .expect("rescind old contract");
        team_queries::remove_pilot_from_team(&db.conn, &player.id, &contract.equipe_id)
            .expect("remove from team");
    }
    force_complete_preseason_plan(&config.saves_dir().join("career_001"));

    finalize_preseason_in_base_dir(&base_dir, "career_001")
        .expect("should allow advancing even without an active player team");

    let save_dir = config.saves_dir().join("career_001");
    assert!(
        !save_dir.join("preseason_plan.json").exists(),
        "finalizacao deve limpar o plano da pre-temporada"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_preseason_feed_has_promotions_and_relegations() {
    // Regressão: o feed da pré-temporada deve mostrar promoções/rebaixamentos
    // (movimentos de tier), não só contratações laterais. Bug anterior: a categoria
    // de origem dos dispensados era limpa antes do snapshot → tudo virava "signing".
    let base_dir = create_test_career_dir("feed_tier_moves");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let mut promotions = 0;
    let mut relegations = 0;
    let mut guard = 0;
    loop {
        let week = advance_market_week_in_base_dir(&base_dir, "career_001", None).expect("advance");
        for e in &week.events {
            match e.movement_kind.as_deref() {
                Some("promotion") => promotions += 1,
                Some("relegation") => relegations += 1,
                _ => {}
            }
        }
        if week.is_last_week {
            break;
        }
        guard += 1;
        assert!(guard < 40);
    }
    assert!(
        promotions > 0,
        "o feed deve mostrar promoções (pilotos subindo de divisão)"
    );
    assert!(
        relegations > 0,
        "o feed deve mostrar rebaixamentos (pilotos descendo de divisão)"
    );
    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_teamless_player_is_placed_by_window_close() {
    // Garantia de porta: um jogador agente livre NUNCA termina a pré-temporada sem
    // equipe (num save NOVO/limpo). Isola "é bug do código" de "é o save antigo".
    let base_dir = create_test_career_dir("teamless_player_guarantee");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");

    // Rescinde o contrato do jogador → vira agente livre.
    {
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        if let Some(contract) =
            contract_queries::get_active_contract_for_pilot(&db.conn, &player.id)
                .expect("active contract")
        {
            contract_queries::update_contract_status(
                &db.conn,
                &contract.id,
                &crate::models::enums::ContractStatus::Rescindido,
            )
            .expect("rescind");
            team_queries::remove_pilot_from_team(&db.conn, &player.id, &contract.equipe_id)
                .expect("remove from team");
        }
    }

    // Avança o mercado até FECHAR (jogador sempre espera = None).
    let mut guard = 0;
    loop {
        let week = advance_market_week_in_base_dir(&base_dir, "career_001", None)
            .expect("advance market week");
        if week.is_last_week {
            break;
        }
        guard += 1;
        assert!(guard < 40, "a janela deve fechar");
    }

    // O jogador DEVE ter contrato ao fim — nunca em limbo.
    let db = Database::open_existing(&db_path).expect("db reopen");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    assert!(
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
            .expect("contract")
            .is_some(),
        "o jogador agente livre nunca deve terminar a pre-temporada sem equipe"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_finalize_succeeds_when_all_resolved() {
    let base_dir = create_test_career_dir("finalize_success");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let config = AppConfig::load_or_default(&base_dir);
    let save_dir = config.saves_dir().join("career_001");
    force_complete_preseason_plan(&save_dir);
    persist_resume_context_in_base_dir(&base_dir, "career_001", CareerResumeView::Preseason, None)
        .expect("persist preseason resume context");

    finalize_preseason_in_base_dir(&base_dir, "career_001").expect("finalize preseason");

    assert!(!save_dir.join("preseason_plan.json").exists());
    assert!(read_resume_context(&save_dir)
        .expect("read resume context")
        .is_none());

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_can_advance_from_second_season_after_finalizing_preseason() {
    let base_dir = create_test_career_dir("advance_second_season");
    mark_all_races_completed(&base_dir, "career_001");

    advance_season_in_base_dir(&base_dir, "career_001").expect("advance to season 2");
    let config = AppConfig::load_or_default(&base_dir);
    let save_dir = config.saves_dir().join("career_001");
    let db_path = save_dir.join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("active season query")
        .expect("active season");
    if contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
        .expect("active regular contract query")
        .is_none()
    {
        let mut news_items = Vec::new();
        force_place_player(&db.conn, &player, &season, &mut news_items)
            .expect("force place player for season 2");
    }

    force_complete_preseason_plan(&save_dir);
    finalize_preseason_in_base_dir(&base_dir, "career_001").expect("finalize preseason");

    mark_all_races_completed(&base_dir, "career_001");
    let result = advance_season_in_base_dir(&base_dir, "career_001")
        .expect("advance to season 3 should work");

    let refreshed_db = Database::open_existing(&db_path).expect("db");
    let active_season = season_queries::get_active_season(&refreshed_db.conn)
        .expect("active season query")
        .expect("active season");

    assert_eq!(result.new_year, 2026);
    assert_eq!(active_season.numero, 3);
    assert_eq!(active_season.ano, 2026);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_skip_all_pending_races_allows_teamless_player_to_reach_next_preseason() {
    let base_dir = create_test_career_dir("skip_teamless_second_season");
    mark_all_races_completed(&base_dir, "career_001");

    advance_season_in_base_dir(&base_dir, "career_001").expect("advance to season 2");
    let config = AppConfig::load_or_default(&base_dir);
    let save_dir = config.saves_dir().join("career_001");
    let db_path = save_dir.join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");

    if let Some(contract) =
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
            .expect("active regular contract")
    {
        contract_queries::update_contract_status(
            &db.conn,
            &contract.id,
            &crate::models::enums::ContractStatus::Rescindido,
        )
        .expect("rescind old contract");
        team_queries::remove_pilot_from_team(&db.conn, &player.id, &contract.equipe_id)
            .expect("remove from team");
    }

    force_complete_preseason_plan(&save_dir);
    finalize_preseason_in_base_dir(&base_dir, "career_001")
        .expect("finalize preseason without team");

    skip_all_pending_races_in_base_dir(&base_dir, "career_001")
        .expect("teamless player should be able to skip season");
    let result = advance_season_in_base_dir(&base_dir, "career_001")
        .expect("advance to season 3 should work after skipping teamless season");

    let refreshed_db = Database::open_existing(&db_path).expect("db");
    let active_season = season_queries::get_active_season(&refreshed_db.conn)
        .expect("active season query")
        .expect("active season");

    assert_eq!(result.new_year, 2026);
    assert_eq!(active_season.numero, 3);
    assert_eq!(active_season.ano, 2026);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_teamless_player_skip_path_keeps_special_grids_assignable() {
    let base_dir = create_test_career_dir("skip_teamless_special_grid");
    mark_all_races_completed(&base_dir, "career_001");

    advance_season_in_base_dir(&base_dir, "career_001").expect("advance to season 2");
    let config = AppConfig::load_or_default(&base_dir);
    let save_dir = config.saves_dir().join("career_001");
    let db_path = save_dir.join("career.db");
    let mut db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");

    if let Some(contract) =
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
            .expect("active regular contract")
    {
        contract_queries::update_contract_status(
            &db.conn,
            &contract.id,
            &crate::models::enums::ContractStatus::Rescindido,
        )
        .expect("rescind old contract");
        team_queries::remove_pilot_from_team(&db.conn, &player.id, &contract.equipe_id)
            .expect("remove from team");
    }

    force_complete_preseason_plan(&save_dir);
    finalize_preseason_in_base_dir(&base_dir, "career_001")
        .expect("finalize preseason without team");

    force_legacy_blocoregular_state(&db);
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    let pending_regular =
        calendar_queries::get_pending_races(&db.conn, &season.id).expect("pending races");
    for race in &pending_regular {
        crate::commands::race::simulate_category_race(&mut db, race, false)
            .expect("simulate regular race while skipping");
    }

    crate::convocation::advance_to_convocation_window(&db.conn).expect("advance to convocation");
    let convocation =
        crate::convocation::run_convocation_window(&db.conn).expect("run convocation");
    assert!(
        convocation.errors.is_empty(),
        "convocation should not report structural errors: {:?}",
        convocation.errors
    );
    crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");

    for category_id in ["production_challenger", "endurance"] {
        let active_drivers = driver_queries::get_drivers_by_active_category(&db.conn, category_id)
            .expect("active special drivers");
        let contracts =
            contract_queries::get_active_especial_contracts_by_category(&db.conn, category_id)
                .expect("active special contracts");
        let assigned_ids: std::collections::HashSet<String> = contracts
            .iter()
            .map(|contract| contract.piloto_id.clone())
            .collect();
        let orphaned: Vec<String> = active_drivers
            .iter()
            .filter(|driver| !assigned_ids.contains(&driver.id))
            .map(|driver| format!("{} ({})", driver.nome, driver.id))
            .collect();

        assert!(
            orphaned.is_empty(),
            "special category '{}' should not contain drivers without lineup: {}",
            category_id,
            orphaned.join(", ")
        );
    }

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_cannot_accept_already_resolved_proposal() {
    let base_dir = create_test_career_dir("accept_resolved_proposal");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    seed_player_proposal(&db.conn, &season.id, &player.id, "T001", "Recusada");

    let error = respond_to_proposal_in_base_dir(&base_dir, "career_001", "MP-T001-P001", true)
        .expect_err("should reject resolved proposal");

    assert!(error.contains("nao esta mais pendente"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_player_rejects_all_gets_emergency_proposals() {
    let base_dir = create_test_career_dir("reject_all_emergency");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    if let Some(contract) = contract_queries::get_active_contract_for_pilot(&db.conn, &player.id)
        .expect("active contract")
    {
        contract_queries::update_contract_status(
            &db.conn,
            &contract.id,
            &crate::models::enums::ContractStatus::Rescindido,
        )
        .expect("rescind old contract");
        team_queries::remove_pilot_from_team(&db.conn, &player.id, &contract.equipe_id)
            .expect("remove from team");
    }
    seed_player_proposal(&db.conn, &season.id, &player.id, "T001", "Pendente");

    let response = respond_to_proposal_in_base_dir(&base_dir, "career_001", "MP-T001-P001", false)
        .expect("reject proposal");

    assert_eq!(response.action, "rejected");
    assert!(response.remaining_proposals > 0);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_briefing_phrase_history_persists_and_keeps_only_last_five_rounds_per_driver_bucket() {
    let base_dir = create_test_career_dir("briefing_phrase_history");
    let career_id = "career_001";

    for round_number in 1..=7 {
        save_briefing_phrase_history_in_base_dir(
            &base_dir,
            career_id,
            1,
            vec![BriefingPhraseEntryInput {
                round_number,
                driver_id: "drv-player".to_string(),
                bucket_key: "p1".to_string(),
                phrase_id: format!("p1-baseline-{round_number}"),
            }],
        )
        .expect("save phrase history");
    }

    let history =
        get_briefing_phrase_history_in_base_dir(&base_dir, career_id).expect("phrase history");

    assert_eq!(history.season_number, 1);
    assert_eq!(history.entries.len(), 5);
    assert_eq!(
        history
            .entries
            .iter()
            .map(|entry| entry.round_number)
            .collect::<Vec<_>>(),
        vec![7, 6, 5, 4, 3]
    );
    assert!(history
        .entries
        .iter()
        .all(|entry| entry.driver_id == "drv-player" && entry.bucket_key == "p1"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_briefing_phrase_history_resets_when_season_changes() {
    let base_dir = create_test_career_dir("briefing_phrase_history_reset");
    let career_id = "career_001";

    save_briefing_phrase_history_in_base_dir(
        &base_dir,
        career_id,
        1,
        vec![BriefingPhraseEntryInput {
            round_number: 5,
            driver_id: "drv-player".to_string(),
            bucket_key: "p2".to_string(),
            phrase_id: "p2-stable-1".to_string(),
        }],
    )
    .expect("save season one");

    let history = save_briefing_phrase_history_in_base_dir(
        &base_dir,
        career_id,
        2,
        vec![BriefingPhraseEntryInput {
            round_number: 1,
            driver_id: "drv-player".to_string(),
            bucket_key: "p2".to_string(),
            phrase_id: "p2-stable-2".to_string(),
        }],
    )
    .expect("save season two");

    assert_eq!(history.season_number, 2);
    assert_eq!(history.entries.len(), 1);
    assert_eq!(history.entries[0].round_number, 1);
    assert_eq!(history.entries[0].phrase_id, "p2-stable-2");

    let _ = fs::remove_dir_all(base_dir);
}

fn create_test_career_dir(label: &str) -> std::path::PathBuf {
    let base_dir = unique_test_dir(label);
    fs::create_dir_all(&base_dir).expect("base dir");

    let input = CreateCareerInput {
        player_name: "Joao Silva".to_string(),
        player_nationality: "br".to_string(),
        player_age: Some(22),
        category: "mazda_rookie".to_string(),
        team_index: 2,
        difficulty: "medio".to_string(),
    };

    let _ = create_career_in_base_dir(&base_dir, input).expect("career should be created");
    base_dir
}

fn mark_all_races_completed(base_dir: &Path, career_id: &str) {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    db.conn
        .execute("UPDATE calendar SET status = 'Concluida'", [])
        .expect("mark all races completed");
    db.conn
        .execute(
            "UPDATE seasons SET fase = 'PosEspecial' WHERE status = 'EmAndamento'",
            [],
        )
        .expect("mark season as post-special");
}

fn mark_regular_races_completed(db: &Database) {
    db.conn
        .execute(
            "UPDATE calendar SET status = 'Concluida' WHERE season_phase = 'BlocoRegular'",
            [],
        )
        .expect("complete regular block");
}

/// Força a temporada ativa e o calendário para o estado legado BlocoRegular.
/// Necessário em testes que exercem o fluxo de convocação legado (BlocoRegular →
/// JanelaConvocacao) em saves criados pelo modelo 9D (fase Temporada).
/// Remove as entradas de production_challenger e endurance do calendário 9D para
/// que iniciar_bloco_especial possa gerá-las no estilo legado BlocoEspecial.
fn force_legacy_blocoregular_state(db: &Database) {
    db.conn
        .execute(
            "UPDATE seasons SET fase = 'BlocoRegular' WHERE status = 'EmAndamento'",
            [],
        )
        .expect("set season to BlocoRegular");
    db.conn
        .execute(
            "DELETE FROM calendar WHERE categoria IN ('production_challenger', 'endurance')",
            [],
        )
        .expect("remove 9D special category entries");
    db.conn
        .execute("UPDATE calendar SET season_phase = 'BlocoRegular'", [])
        .expect("set calendar to BlocoRegular phase");
}

fn insert_test_endurance_team(conn: &rusqlite::Connection) -> Team {
    let mut team = crate::models::team::placeholder_team_from_db(
        "T_TEST_ENDURANCE".to_string(),
        "Endurance Test Team".to_string(),
        "endurance".to_string(),
        crate::common::time::current_timestamp(),
    );
    team.classe = Some("gt4".to_string());
    team_queries::insert_team(conn, &team).expect("insert endurance test team");
    team
}

fn insert_test_production_team(conn: &rusqlite::Connection, class_name: &str) -> Team {
    let mut team = crate::models::team::placeholder_team_from_db(
        format!("T_TEST_PRODUCTION_{}", class_name.to_uppercase()),
        format!("Production {class_name} Test Team"),
        "production_challenger".to_string(),
        crate::common::time::current_timestamp(),
    );
    team.classe = Some(class_name.to_string());
    team_queries::insert_team(conn, &team).expect("insert production test team");
    team
}

fn unique_test_dir(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("iracerapp_{label}_{nanos}"))
}

fn seed_player_proposal(
    conn: &rusqlite::Connection,
    season_id: &str,
    player_id: &str,
    team_id: &str,
    status: &str,
) {
    let team = team_queries::get_team_by_id(conn, team_id)
        .expect("team query")
        .expect("team");
    let player = driver_queries::get_driver(conn, player_id).expect("player");
    crate::db::queries::market_proposals::insert_player_proposal(
        conn,
        season_id,
        &crate::market::proposals::MarketProposal {
            id: format!("MP-{team_id}-{player_id}"),
            equipe_id: team.id.clone(),
            equipe_nome: team.nome.clone(),
            piloto_id: player.id.clone(),
            piloto_nome: player.nome.clone(),
            categoria: team.categoria.clone(),
            papel: crate::models::enums::TeamRole::Numero1,
            salario_oferecido: 95_000.0,
            duracao_anos: 2,
            status: match status {
                "Aceita" => crate::market::proposals::ProposalStatus::Aceita,
                "Recusada" => crate::market::proposals::ProposalStatus::Recusada,
                "Expirada" => crate::market::proposals::ProposalStatus::Expirada,
                _ => crate::market::proposals::ProposalStatus::Pendente,
            },
            motivo_recusa: None,
        },
    )
    .expect("insert player proposal");
}

fn force_complete_preseason_plan(save_dir: &Path) {
    let mut plan = crate::market::preseason::load_preseason_plan(save_dir)
        .expect("load plan")
        .expect("plan");
    plan.state.is_complete = true;
    plan.state.current_week = plan.state.total_weeks + 1;
    plan.state.phase = crate::market::preseason::PreSeasonPhase::Complete;
    plan.state.player_has_pending_proposals = false;
    crate::market::preseason::save_preseason_plan(save_dir, &plan).expect("save plan");
}

fn latest_regular_contract_for_driver(
    conn: &rusqlite::Connection,
    driver_id: &str,
) -> crate::models::contract::Contract {
    contract_queries::get_contracts_for_pilot(conn, driver_id)
        .expect("driver contracts query")
        .into_iter()
        .filter(|contract| contract.tipo == crate::models::enums::ContractType::Regular)
        .max_by(|a, b| {
            a.temporada_inicio
                .cmp(&b.temporada_inicio)
                .then_with(|| a.created_at.cmp(&b.created_at))
        })
        .expect("latest regular contract")
}

/// A escada da marca vai até a Production, mas a Production é multiclasse: três
/// marcas disputam a MESMA categoria em campeonatos separados. O Grupo Mazda
/// conta a Production da classe Mazda e ignora a de Toyota — elas nunca
/// dividiram a pista.
#[test]
#[serial_test::serial]
fn test_team_records_group_counts_only_the_family_class_in_production() {
    rust_i18n::set_locale("pt-BR");
    let base_dir = create_test_career_dir("team_records_family_class");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let corrida_production: Option<String> = db
        .conn
        .query_row(
            "SELECT id FROM calendar WHERE categoria = 'production_challenger' ORDER BY rodada LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("consulta calendário");
    let Some(corrida_production) = corrida_production else {
        // Save de teste sem Production no calendário: não há o que separar.
        return;
    };

    let equipes: Vec<String> = db
        .conn
        .prepare("SELECT id FROM teams ORDER BY id LIMIT 2")
        .expect("prepare")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("equipes");
    let (mazda, toyota) = (&equipes[0], &equipes[1]);
    let (piloto_mazda, _) = team_driver_ids(&db.conn, mazda).expect("piloto mazda");
    let (piloto_toyota, _) = team_driver_ids(&db.conn, toyota).expect("piloto toyota");

    db.conn
        .execute("DELETE FROM race_results", [])
        .expect("limpa resultados");
    for (team_id, classe) in [(mazda, "mazda"), (toyota, "toyota")] {
        db.conn
            .execute(
                "UPDATE teams SET classe = ?1 WHERE id = ?2",
                rusqlite::params![classe, team_id],
            )
            .expect("classe da equipe");
    }
    // Uma vitória na Production para cada uma, na mesma corrida.
    for (piloto, equipe) in [(&piloto_mazda, mazda), (&piloto_toyota, toyota)] {
        db.conn
            .execute(
                "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos)
                 VALUES (?1, ?2, ?3, 1, 25.0)",
                rusqlite::params![&corrida_production, piloto, equipe],
            )
            .expect("resultado production");
    }
    drop(db);

    let grupo_mazda =
        get_team_records_ranking_in_base_dir(&base_dir, "career_001", "mazda_rookie", "group", None)
            .expect("grupo mazda");
    let linha = |ranking: &crate::commands::career_types::TeamRecordsRanking, id: &str| {
        ranking
            .rows
            .iter()
            .find(|row| row.team_id == id)
            .map(|row| row.races)
            .unwrap_or(0)
    };
    // A corrida da equipe Mazda entra; a da Toyota, não — mesma categoria,
    // campeonato outro.
    assert_eq!(linha(&grupo_mazda, mazda), 1);
    assert_eq!(linha(&grupo_mazda, toyota), 0);

    // Espelho: no Grupo Toyota a conta se inverte.
    let grupo_toyota =
        get_team_records_ranking_in_base_dir(&base_dir, "career_001", "toyota_rookie", "group", None)
            .expect("grupo toyota");
    assert_eq!(linha(&grupo_toyota, toyota), 1);
    assert_eq!(linha(&grupo_toyota, mazda), 0);

    // A escada abre as multiclasse por carro: a Production não é um campeonato,
    // são três correndo na mesma pista. Escolher "Production" inteira somaria
    // Mazda, Toyota e BMW num número que não existe em classificação nenhuma.
    let producao: Vec<&crate::commands::career_types::TeamRecordsCategory> = grupo_mazda
        .categories
        .iter()
        .filter(|item| item.id == "production_challenger")
        .collect();
    assert_eq!(producao.len(), 3);
    assert_eq!(producao[0].key, "production_challenger:mazda");
    assert_eq!(producao[0].label, "Production · Mazda");
    // Monomarca segue com uma entrada só, e a chave é o próprio id.
    let gt3 = grupo_mazda
        .categories
        .iter()
        .find(|item| item.id == "gt3")
        .expect("gt3 na escada");
    assert_eq!((gt3.key.as_str(), gt3.class.as_str()), ("gt3", ""));

    // E cada campeonato da Production conta só o seu: pedir a classe é pedir um
    // dos três, não a categoria inteira.
    let producao_mazda = get_team_records_ranking_in_base_dir(
        &base_dir,
        "career_001",
        "production_challenger",
        "category",
        Some("mazda"),
    )
    .expect("production mazda");
    assert_eq!(producao_mazda.scope, "Production · Mazda");
    assert_eq!(producao_mazda.scope_family, "mazda");
    assert_eq!(linha(&producao_mazda, mazda), 1);
    assert_eq!(linha(&producao_mazda, toyota), 0);
    let producao_toda =
        get_team_records_ranking_in_base_dir(&base_dir, "career_001", "production_challenger", "category", None)
            .expect("production toda");
    assert_eq!(linha(&producao_toda, mazda), 1);
    assert_eq!(linha(&producao_toda, toyota), 1);

    // E o mundo não recorta por marca: as duas corridas existem.
    let mundo = get_team_records_ranking_in_base_dir(&base_dir, "career_001", "mazda_rookie", "world", None)
        .expect("mundo");
    assert_eq!(linha(&mundo, mazda), 1);
    assert_eq!(linha(&mundo, toyota), 1);
}

/// Os cards de record são da CATEGORIA, não do grupo. Uma equipe com corridas na
/// Mazda Rookie e na Mazda Championship vê, na ficha aberta pela Rookie, só o
/// que fez na Rookie — a média e o "17º de 22" que vinham do grupo somavam um
/// campeonato que ela nem sempre disputou.
#[test]
#[serial_test::serial]
fn test_team_dossier_records_are_scoped_to_the_category_not_the_group() {
    rust_i18n::set_locale("pt-BR");
    let base_dir = create_test_career_dir("team_dossier_records_por_categoria");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let corrida = |categoria: &str| -> Option<String> {
        db.conn
            .query_row(
                "SELECT id FROM calendar WHERE categoria = ?1 ORDER BY rodada LIMIT 1",
                rusqlite::params![categoria],
                |row| row.get(0),
            )
            .optional()
            .expect("consulta calendário")
    };
    let (Some(na_rookie), Some(na_championship)) =
        (corrida("mazda_rookie"), corrida("mazda_amador"))
    else {
        // Sem as duas categorias no calendário não há grupo para separar.
        return;
    };

    let teams = get_teams_standings_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("standings");
    let equipe = teams.first().expect("equipe").id.clone();
    let (piloto, _) = team_driver_ids(&db.conn, &equipe).expect("piloto");

    db.conn
        .execute("DELETE FROM race_results", [])
        .expect("limpa resultados");
    // Uma vitória na Rookie e duas na Championship. No grupo seriam 3 vitórias em
    // 3 corridas; na categoria são 1 em 1.
    for (race_id, posicao) in [(&na_rookie, 1), (&na_championship, 1)] {
        db.conn
            .execute(
                "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos)
                 VALUES (?1, ?2, ?3, ?4, 25.0)",
                rusqlite::params![race_id, &piloto, &equipe, posicao],
            )
            .expect("resultado");
    }
    drop(db);

    let ficha_rookie =
        get_team_history_dossier_in_base_dir(&base_dir, "career_001", &equipe, "mazda_rookie")
            .expect("ficha rookie");
    assert_eq!(ficha_rookie.record_scope, "Mazda Rookie");
    assert_eq!(ficha_rookie.sport.races, 1);
    assert_eq!(ficha_rookie.sport.wins, 1);

    // A mesma equipe, aberta pela Championship: o card muda de recorte junto.
    let ficha_championship =
        get_team_history_dossier_in_base_dir(&base_dir, "career_001", &equipe, "mazda_amador")
            .expect("ficha championship");
    assert_eq!(ficha_championship.record_scope, "Mazda Championship");
    assert_eq!(ficha_championship.sport.races, 1);

    // Mas a HISTÓRIA continua sendo a do grupo: a fita de forma recente mostra as
    // duas corridas, porque ela conta a trajetória e não a comparação.
    assert_eq!(ficha_rookie.recent_form.len(), 2);
    // E "tem histórico" não depende do recorte dos cards: uma equipe que subiu de
    // tier não tem corrida na categoria de baixo, e o dossiê inteiro cairia em
    // "sem histórico" por causa de um filtro que só vale para os cards.
    assert!(ficha_rookie.has_history);
}
