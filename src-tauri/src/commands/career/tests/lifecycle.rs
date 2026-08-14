//! Testes de `career::lifecycle`: criacao, carga e reparo de abertura do save.
//!
//! Fatiado de `tests/mod.rs`, que juntava as dez areas num arquivo so. Os helpers
//! e os `use` continuam no `mod.rs` e chegam aqui pelo glob.

use super::*;

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
    // A carreira regular nasce no MESMO ano jogável do draft histórico — fonte única.
    assert_eq!(
        career.season.ano,
        crate::constants::historical_timeline::PLAYABLE_START_YEAR
    );
    assert!(career.season.total_rodadas > 0);

    let _ = fs::remove_dir_all(base_dir);
}

/// TEMPORADA 1, ANTES DA PRIMEIRA VIRADA: a `meta` do banco tem de contar o mesmo ano
/// da temporada ativa. O seed das migrações é de 2024 e só a virada de temporada tocava
/// em `current_year`, então a carreira inteira até a virada era jogada com o ano errado
/// no banco, e `career_start_year` nunca saía do seed.
#[test]
fn temporada_1_ja_nasce_com_o_ano_do_mundo_na_meta() {
    let base_dir = create_test_career_dir("meta_ano_do_mundo");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let esperado = crate::constants::historical_timeline::PLAYABLE_START_YEAR.to_string();
    for chave in ["current_year", "career_start_year"] {
        assert_eq!(
            meta_queries::get_meta_value(&db.conn, chave).expect("meta"),
            Some(esperado.clone()),
            "meta '{chave}' devia nascer no ano jogável"
        );
    }

    drop(db);
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
fn test_load_career_invalid_id() {
    let base_dir = unique_test_dir("load_invalid");
    fs::create_dir_all(&base_dir).expect("base dir");

    let error = load_career_in_base_dir(&base_dir, "career_999").expect_err("should fail");
    // A mensagem passa pelo i18n (`career::errors`), então compara pela chave e não
    // pela prosa — ver o teste de locales em `career/errors.rs`.
    assert_eq!(error, crate::commands::career::errors::save_not_found());

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

// ── Triagem do reparo de contratos regulares ──────────────────────────────
//
// O reparo abre transação IMMEDIATE, varre time por time e recalcula hierarquia. A
// triagem responde "há algo a reparar?" antes disso, e por isso ela precisa ser
// CONSERVADORA: cada caso abaixo é um estado que o reparo mudaria, e todos têm de
// devolver `true`. Só o save recém-criado devolve `false`.

/// Helper: o `career.db` do save de teste, já aberto.
fn abrir_db_do_save(base_dir: &Path) -> Database {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    Database::open_existing(&db_path).expect("db")
}

#[test]
fn triagem_do_reparo_nao_dispara_em_save_recem_criado() {
    let base_dir = create_test_career_dir("triagem_save_limpo");
    let db = abrir_db_do_save(&base_dir);

    assert!(
        !needs_regular_contract_repair(&db.conn, false).expect("triagem"),
        "save recem-criado nao deveria pedir reparo de contratos"
    );

    drop(db);
    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn triagem_do_reparo_dispara_com_contrato_regular_duplicado() {
    let base_dir = create_test_career_dir("triagem_duplicado");
    let db = abrir_db_do_save(&base_dir);
    let contrato = contract_queries::get_all_active_regular_contracts(&db.conn)
        .expect("contratos")
        .into_iter()
        .next()
        .expect("pelo menos um contrato regular ativo");
    // O índice único `(piloto_id, tipo)` para status Ativo impede o estado duplicado nos
    // saves de hoje; ele existe justamente por causa do defeito que o reparo cobre. Cair
    // o índice reproduz o save legado — mesma manobra de
    // `test_load_career_repairs_duplicate_regular_contract_state`.
    db.conn
        .execute_batch("DROP INDEX IF EXISTS idx_contracts_active_pilot_tipo;")
        .expect("derruba indice unico");
    let mut duplicado = contrato.clone();
    duplicado.id = format!("{}-DUP", contrato.id);
    contract_queries::insert_contract(&db.conn, &duplicado).expect("insere duplicado");

    assert!(
        needs_regular_contract_repair(&db.conn, false).expect("triagem"),
        "dois contratos regulares ativos para o mesmo piloto tem de pedir reparo"
    );

    drop(db);
    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn triagem_do_reparo_dispara_com_lineup_fora_dos_contratos() {
    let base_dir = create_test_career_dir("triagem_lineup");
    let db = abrir_db_do_save(&base_dir);
    let contrato = contract_queries::get_all_active_regular_contracts(&db.conn)
        .expect("contratos")
        .into_iter()
        .next()
        .expect("pelo menos um contrato regular ativo");
    // A coluna da equipe deixa de apontar para quem tem contrato.
    db.conn
        .execute(
            "UPDATE teams SET piloto_1_id = NULL, piloto_2_id = NULL WHERE id = ?1",
            rusqlite::params![&contrato.equipe_id],
        )
        .expect("zera lineup");

    assert!(
        needs_regular_contract_repair(&db.conn, false).expect("triagem"),
        "lineup da equipe divergente dos contratos tem de pedir reparo"
    );

    drop(db);
    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn triagem_do_reparo_dispara_com_piloto_fora_da_categoria_da_equipe() {
    let base_dir = create_test_career_dir("triagem_categoria");
    let db = abrir_db_do_save(&base_dir);
    let contrato = contract_queries::get_all_active_regular_contracts(&db.conn)
        .expect("contratos")
        .into_iter()
        .next()
        .expect("pelo menos um contrato regular ativo");
    db.conn
        .execute(
            "UPDATE drivers SET categoria_atual = 'gt3' WHERE id = ?1",
            rusqlite::params![&contrato.piloto_id],
        )
        .expect("desloca categoria");

    assert!(
        needs_regular_contract_repair(&db.conn, false).expect("triagem"),
        "piloto com categoria_atual fora da equipe do contrato tem de pedir reparo"
    );

    drop(db);
    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn triagem_do_reparo_dispara_com_categoria_orfa_sem_contrato() {
    let base_dir = create_test_career_dir("triagem_orfa");
    let db = abrir_db_do_save(&base_dir);
    let mut sem_contrato = crate::models::driver::Driver::new(
        "P_SEM_CONTRATO".to_string(),
        "Piloto Sem Contrato".to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        24,
        2026,
    );
    sem_contrato.categoria_atual = Some("gt3".to_string());
    driver_queries::insert_driver(&db.conn, &sem_contrato).expect("insere piloto");

    assert!(
        needs_regular_contract_repair(&db.conn, false).expect("triagem"),
        "categoria_atual em piloto sem contrato ativo tem de pedir reparo"
    );

    drop(db);
    let _ = fs::remove_dir_all(base_dir);
}

/// O `keep_retired_seated` das semanas de abertura da janela é a única regra que a
/// triagem desliga: com ele ligado, aposentado no assento é estado LEGÍTIMO e o reparo
/// não deve ser chamado só por isso.
#[test]
fn triagem_do_reparo_respeita_o_aposentado_sentado_da_janela() {
    let base_dir = create_test_career_dir("triagem_aposentado");
    let db = abrir_db_do_save(&base_dir);
    let contrato = contract_queries::get_all_active_regular_contracts(&db.conn)
        .expect("contratos")
        .into_iter()
        .next()
        .expect("pelo menos um contrato regular ativo");
    db.conn
        .execute(
            "UPDATE drivers SET status = 'Aposentado' WHERE id = ?1",
            rusqlite::params![&contrato.piloto_id],
        )
        .expect("aposenta piloto");

    assert!(
        needs_regular_contract_repair(&db.conn, false).expect("triagem"),
        "aposentado com contrato ativo pede reparo no fluxo normal"
    );
    assert!(
        !needs_regular_contract_repair(&db.conn, true).expect("triagem"),
        "nas semanas de abertura da janela o aposentado sentado nao e motivo de reparo"
    );

    drop(db);
    let _ = fs::remove_dir_all(base_dir);
}

// ─── Contexto de interesse da próxima etapa ──────────────────────────────────
//
// `next_race_interest_context` saiu de dentro de `build_next_race_interest_summary`, que
// misturava a leitura da classificação com as duas regras abaixo. Separada a consulta, as
// regras passaram a ser conferíveis sem banco e sem carreira aberta.

fn etapa_de_teste(rodada: i32) -> crate::calendar::CalendarEntry {
    crate::calendar::CalendarEntry {
        id: format!("R-{rodada}"),
        season_id: "S-1".to_string(),
        categoria: "gt3".to_string(),
        rodada,
        nome: "Etapa de teste".to_string(),
        track_id: 1,
        track_name: "Interlagos".to_string(),
        track_config: "GP".to_string(),
        clima: crate::models::enums::WeatherCondition::Dry,
        temperatura: 24.0,
        voltas: 30,
        duracao_corrida_min: 45,
        duracao_classificacao_min: 15,
        status: crate::models::enums::RaceStatus::Pendente,
        horario: "14:00".to_string(),
        week_of_year: 20,
        season_phase: SeasonPhase::BlocoRegular,
        display_date: "18/05".to_string(),
        thematic_slot: crate::models::enums::ThematicSlot::NaoClassificado,
        season_week: Some(20),
    }
}

fn piloto_de_teste() -> Driver {
    Driver::new(
        "DRV-1".to_string(),
        "Piloto Teste".to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        24,
        2026,
    )
}

#[test]
fn a_penultima_etapa_com_o_titulo_perto_e_decisiva() {
    // Restam 2 rodadas (limite), 30 pontos de distância e o jogador está classificado.
    let ctx = next_race_interest_context(
        &etapa_de_teste(12),
        &piloto_de_teste(),
        14,
        &ChampionshipContext {
            player_position: 2,
            gap_to_leader: 30,
        },
    );
    assert!(ctx.is_title_decider_candidate);
    assert_eq!(ctx.player_championship_position, Some(2));
    assert_eq!(ctx.championship_gap_to_leader, Some(30));
}

#[test]
fn meio_de_temporada_nao_e_decisiva_por_mais_perto_que_esteja_o_lider() {
    let ctx = next_race_interest_context(
        &etapa_de_teste(4),
        &piloto_de_teste(),
        14,
        &ChampionshipContext {
            player_position: 1,
            gap_to_leader: 0,
        },
    );
    assert!(
        !ctx.is_title_decider_candidate,
        "faltando 10 rodadas nenhuma etapa decide o título"
    );
}

#[test]
fn jogador_fora_da_classificacao_nao_manda_posicao_nem_vira_decisiva() {
    // `player_position == 0` é o retorno de quem ainda não pontuou na categoria — e é
    // também o fallback quando a consulta falha. Nos dois casos a etapa não é decisiva.
    let ctx = next_race_interest_context(
        &etapa_de_teste(14),
        &piloto_de_teste(),
        14,
        &ChampionshipContext {
            player_position: 0,
            gap_to_leader: 0,
        },
    );
    assert!(!ctx.is_title_decider_candidate);
    assert_eq!(ctx.player_championship_position, None);
    assert_eq!(ctx.championship_gap_to_leader, None);
}

#[test]
fn o_lider_manda_a_distancia_zero_em_vez_de_omiti_la() {
    // Distância 0 com posição 1 é informação (o jogador lidera), e não ausência de dado.
    let ctx = next_race_interest_context(
        &etapa_de_teste(13),
        &piloto_de_teste(),
        14,
        &ChampionshipContext {
            player_position: 1,
            gap_to_leader: 0,
        },
    );
    assert_eq!(ctx.championship_gap_to_leader, Some(0));
    assert!(ctx.is_title_decider_candidate);
}
