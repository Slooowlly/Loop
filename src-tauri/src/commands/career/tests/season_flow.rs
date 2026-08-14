//! Testes de `career::season_flow`: avanco de temporada e pulo de etapas.
//!
//! Fatiado de `tests/mod.rs`, que juntava as dez areas num arquivo so. Os helpers
//! e os `use` continuam no `mod.rs` e chegam aqui pelo glob.

use super::*;

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

    // Temporada 2 = o ano jogável + 1; o ano de partida é fonte única.
    assert_eq!(
        result.new_year,
        crate::constants::historical_timeline::PLAYABLE_START_YEAR + 1
    );
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
    assert_eq!(
        active_season.ano,
        crate::constants::historical_timeline::PLAYABLE_START_YEAR + 1
    );
    assert_eq!(meta.current_season, 2);
    assert_eq!(
        meta.current_year,
        crate::constants::historical_timeline::PLAYABLE_START_YEAR as u32 + 1
    );
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

    assert_eq!(
        result.new_year,
        crate::constants::historical_timeline::PLAYABLE_START_YEAR + 1
    );
    assert_eq!(active_season.numero, 2);
    assert_eq!(
        active_season.ano,
        crate::constants::historical_timeline::PLAYABLE_START_YEAR + 1
    );
    assert!(save_dir.join("resume_context.json").is_dir());

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

    assert_eq!(error, crate::commands::career::errors::season_not_started());

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

    assert_eq!(
        result.new_year,
        crate::constants::historical_timeline::PLAYABLE_START_YEAR + 2
    );
    assert_eq!(active_season.numero, 3);
    assert_eq!(
        active_season.ano,
        crate::constants::historical_timeline::PLAYABLE_START_YEAR + 2
    );

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

// --------------------------------------------------------------------------
// Temporadas de dois dígitos: `temporada_inicio` é coluna TEXT, então a
// igualdade contra um parâmetro inteiro só acerta enquanto os dois lados
// escreverem o número igual. `CAST(... AS INTEGER)` fecha isso, e é o que os
// casos abaixo cobram com 9, 10, 12 e 26.
// --------------------------------------------------------------------------

/// Banco só com o schema real: o que interessa aqui é o tipo TEXT das colunas
/// de vigência, e ele vem da migração.
fn conn_com_schema_para_limpeza_9d() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    crate::db::migrations::run_all(&conn).expect("schema");
    conn
}

/// Contrato Especial ativo com a temporada gravada **como texto cru**, para o
/// teste poder escrever `'09'` e `'026'` do jeito que o banco aceita.
fn especial_ativo_em(conn: &rusqlite::Connection, id: &str, piloto_id: &str, inicio_texto: &str) {
    // As chaves estrangeiras de `contracts` valem nesta conexão: piloto e equipe primeiro.
    conn.execute(
        "INSERT OR IGNORE INTO drivers (id, nome, idade, nacionalidade)
         VALUES (?1, 'Piloto', 28, 'BR')",
        rusqlite::params![piloto_id],
    )
    .expect("insert piloto");
    conn.execute(
        "INSERT OR IGNORE INTO teams (id, nome, categoria)
         VALUES ('T001', 'Equipe', 'production_challenger')",
        [],
    )
    .expect("insert equipe");
    conn.execute(
        "INSERT INTO contracts (
            id, piloto_id, piloto_nome, equipe_id, equipe_nome, categoria,
            tipo, status, papel, salario, salario_anual, duracao_anos,
            temporada_inicio, temporada_fim, created_at
        ) VALUES (
            ?1, ?2, 'Piloto', 'T001', 'Equipe', 'production_challenger',
            'Especial', 'Ativo', 'Numero1', 0.0, 0.0, 1,
            ?3, ?3, '2026-01-01T00:00:00Z'
        )",
        rusqlite::params![id, piloto_id, inicio_texto],
    )
    .expect("insert contrato especial");
}

fn status_do_contrato(conn: &rusqlite::Connection, id: &str) -> String {
    conn.query_row(
        "SELECT status FROM contracts WHERE id = ?1",
        rusqlite::params![id],
        |row| row.get(0),
    )
    .expect("status do contrato")
}

/// A limpeza legada 9D atinge a temporada pedida e só ela. Com comparação em
/// texto, a temporada 9 gravada como `'09'` escapava e as demais ficavam.
#[test]
fn limpeza_legada_9d_compara_a_temporada_como_numero() {
    let conn = conn_com_schema_para_limpeza_9d();
    especial_ativo_em(&conn, "E09", "P09", "09");
    especial_ativo_em(&conn, "E10", "P10", "10");
    especial_ativo_em(&conn, "E12", "P12", "12");
    especial_ativo_em(&conn, "E26", "P26", "26");

    crate::commands::career::season_flow::cleanup_legacy_special_state_for_9d_transition(&conn, 9)
        .expect("limpeza da temporada 9");

    assert_eq!(
        status_do_contrato(&conn, "E09"),
        "Expirado",
        "a temporada 9 gravada como '09' é a mesma temporada 9",
    );
    for id in ["E10", "E12", "E26"] {
        assert_eq!(
            status_do_contrato(&conn, id),
            "Ativo",
            "{id} não é da temporada 9 e não pode ser expirado junto",
        );
    }
}

/// O outro lado da mesma moeda: pedir a temporada 26 não pode arrastar a 9, que
/// é a maior das quatro em ordem lexicográfica.
#[test]
fn limpeza_legada_9d_na_temporada_26_nao_arrasta_a_temporada_9() {
    let conn = conn_com_schema_para_limpeza_9d();
    especial_ativo_em(&conn, "E09", "P09", "9");
    especial_ativo_em(&conn, "E10", "P10", "10");
    especial_ativo_em(&conn, "E12", "P12", "12");
    especial_ativo_em(&conn, "E26", "P26", "026");

    crate::commands::career::season_flow::cleanup_legacy_special_state_for_9d_transition(&conn, 26)
        .expect("limpeza da temporada 26");

    assert_eq!(
        status_do_contrato(&conn, "E26"),
        "Expirado",
        "a temporada 26 gravada como '026' é a mesma temporada 26",
    );
    for id in ["E09", "E10", "E12"] {
        assert_eq!(
            status_do_contrato(&conn, id),
            "Ativo",
            "{id} não é da temporada 26 e não pode ser expirado junto",
        );
    }
}
