//! Testes de `career::market_window`: pre-temporada, propostas e janela de mercado.
//!
//! Fatiado de `tests/mod.rs`, que juntava as dez areas num arquivo so. Os helpers
//! e os `use` continuam no `mod.rs` e chegam aqui pelo glob.

use super::*;

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

    // A mensagem passa pelo i18n (`career::errors`), então compara pela chave.
    assert_eq!(
        error,
        crate::commands::career::errors::preseason_not_finished()
    );

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

    assert_eq!(
        error,
        crate::commands::career::errors::proposal_not_pending()
    );

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

/// A janela tem que contratar em TODAS as categorias, todas as semanas.
///
/// A escada preenche de cima pra baixo (endurance, gt3, gt4, production, bmw, e por
/// último mazda/toyota). Com um teto semanal único para o grid inteiro, o topo gastava o
/// orçamento sozinho: as categorias de base não assinavam ninguém por seis semanas e
/// caíam TODAS no fechamento — que preenche o que sobrou sem teto nenhum. O jogador via
/// um mercado morto por semanas e depois meio grid mudando de uma vez.
///
/// O teste roda a janela inteira e cobra as duas coisas que aquele desenho quebrava:
/// nenhuma categoria pode estrear no fechamento, e o fechamento não pode ser o grosso do
/// mercado. Os limiares são frouxos de propósito — o mundo é sorteado a cada execução, e
/// o que está sendo travado é a FORMA da distribuição, não um número.
#[test]
fn test_janela_contrata_em_todas_as_categorias_antes_da_ultima_semana() {
    let base_dir = create_test_career_dir("distribuicao_mercado");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");

    let mut por_semana: Vec<(i32, Vec<String>)> = Vec::new();
    loop {
        let semana = advance_market_week_in_base_dir(&base_dir, "career_001", None)
            .expect("advance market week");
        let assinaturas: Vec<String> = semana
            .events
            .iter()
            .filter(|e| {
                matches!(
                    e.movement_kind.as_deref(),
                    Some("promotion")
                        | Some("relegation")
                        | Some("lateral")
                        | Some("rookie")
                        | Some("signing")
                )
            })
            .filter_map(|e| e.categoria.clone())
            .collect();
        let ultima = semana.is_last_week;
        por_semana.push((semana.week_number, assinaturas));
        if ultima {
            break;
        }
    }

    let total: usize = por_semana.iter().map(|(_, a)| a.len()).sum();
    assert!(total > 0, "a janela nao contratou ninguem");
    let (_, fechamento) = por_semana.last().expect("a janela tem semanas");
    let quota_do_fechamento = 100 * fechamento.len() / total;
    assert!(
        quota_do_fechamento <= 40,
        "o fechamento concentrou {quota_do_fechamento}% do mercado ({} de {total}): {por_semana:?}",
        fechamento.len()
    );

    // Nenhuma categoria pode ter esperado o fechamento pra existir.
    let antes: std::collections::HashSet<&str> = por_semana[..por_semana.len() - 1]
        .iter()
        .flat_map(|(_, a)| a.iter().map(|c| c.as_str()))
        .collect();
    let estreantes_no_fechamento: Vec<&str> = fechamento
        .iter()
        .map(|c| c.as_str())
        .filter(|c| !antes.contains(c))
        .collect();
    assert!(
        estreantes_no_fechamento.is_empty(),
        "categorias que so contrataram no fechamento: {estreantes_no_fechamento:?} — {por_semana:?}"
    );

    let _ = fs::remove_dir_all(base_dir);
}

/// Harness de medição: o que a janela faz com a POPULAÇÃO do mundo.
///
/// O grid tem assentos fixos, então todo estreante gerado empurra alguém para fora dele.
/// Mexer no ritmo da escada mexe nisso de lado — rode aqui antes e depois pra ver se o
/// mundo engordou. Não assevera nada: é medição, e o mundo é sorteado a cada execução.
#[test]
#[ignore = "harness de medição da população pós-janela (lento); rodar sob demanda"]
fn medir_populacao_apos_a_janela() {
    let base_dir = create_test_career_dir("populacao_janela");
    mark_all_races_completed(&base_dir, "career_001");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let conta = |sql: &str| -> i64 {
        let db = Database::open_existing(&db_path).expect("db");
        db.conn.query_row(sql, [], |r| r.get(0)).unwrap_or(-1)
    };
    let ativos = "SELECT COUNT(*) FROM drivers WHERE status = 'Ativo'";
    let livres = "SELECT COUNT(*) FROM drivers d WHERE d.status = 'Ativo' AND NOT EXISTS \
        (SELECT 1 FROM contracts c WHERE c.piloto_id = d.id AND c.status = 'Ativo' AND c.tipo = 'Regular')";

    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    let ativos_antes = conta(ativos);

    let mut rookies = 0usize;
    let mut assinaturas = 0usize;
    loop {
        let semana = advance_market_week_in_base_dir(&base_dir, "career_001", None)
            .expect("advance market week");
        for e in &semana.events {
            match e.movement_kind.as_deref() {
                Some("rookie") => {
                    rookies += 1;
                    assinaturas += 1;
                }
                Some("promotion") | Some("relegation") | Some("lateral") | Some("signing") => {
                    assinaturas += 1;
                }
                _ => {}
            }
        }
        if semana.is_last_week {
            break;
        }
    }
    println!(
        "ativos antes {ativos_antes} | ativos depois {} | livres depois {} | assinaturas {assinaturas} | rookies gerados {rookies}",
        conta(ativos),
        conta(livres)
    );
    let _ = fs::remove_dir_all(base_dir);
}

/// B85 — A SEMANA DE MERCADO CRUZA ARQUIVO E BANCO. O plano da janela é um arquivo, e
/// ele era gravado ANTES do commit: uma queda entre as duas escritas deixava o plano
/// descrevendo uma semana que o banco nunca comitou, e a janela seguia de um estado que
/// nunca existiu. Com o staging, a falha antes do commit devolve os dois ao mesmo lado.
#[test]
fn semana_de_mercado_nao_publica_o_plano_quando_a_operacao_morre_antes_do_commit() {
    let base_dir = create_test_career_dir("semana_antes_do_commit");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");

    let config = AppConfig::load_or_default(&base_dir);
    let career_dir = config.saves_dir().join("career_001");
    let semana_antes = semana_do_plano(&career_dir);

    sabotar_commit_da_janela(true);
    let falha = advance_market_week_in_base_dir(&base_dir, "career_001", None);
    sabotar_commit_da_janela(false);

    assert!(falha.is_err(), "a falha injetada devia derrubar a semana");
    assert_eq!(
        semana_do_plano(&career_dir),
        semana_antes,
        "o plano nao pode andar sozinho com o banco em rollback"
    );
    assert!(
        !career_dir.join("preseason_plan.json.novo").exists(),
        "o staging do plano nao pode sobreviver a falha"
    );

    // E o caminho feliz continua movendo os dois juntos.
    advance_market_week_in_base_dir(&base_dir, "career_001", None).expect("advance market week");
    assert!(semana_do_plano(&career_dir) > semana_antes);

    let _ = fs::remove_dir_all(base_dir);
}

/// B85 — A QUEBRA DE CONTRATO CRUZA NA ORDEM INVERSA: o banco comitava primeiro e a
/// oferta só era consumida do plano depois. Morrer entre as duas deixava o contrato já
/// mexido e a oferta ainda viva, e o jogador decidia duas vezes sobre a mesma quebra.
#[test]
fn quebra_de_contrato_nao_mexe_no_banco_sem_consumir_a_oferta_do_plano() {
    let base_dir = create_test_career_dir("poach_antes_do_commit");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");

    let config = AppConfig::load_or_default(&base_dir);
    let career_dir = config.saves_dir().join("career_001");
    let db_path = career_dir.join("career.db");

    let oferta = semear_oferta_de_quebra(&career_dir, &db_path);
    let salario_antes = salario_do_contrato(&db_path, &oferta.current_contract_id);

    sabotar_commit_da_janela(true);
    let falha = resolve_player_poach_offer_in_base_dir(&base_dir, "career_001", &oferta, false);
    sabotar_commit_da_janela(false);

    assert!(
        falha.is_err(),
        "a falha injetada devia derrubar a resolucao"
    );
    assert_eq!(
        salario_do_contrato(&db_path, &oferta.current_contract_id),
        salario_antes,
        "o aumento de retencao nao podia ter sido comitado"
    );
    let plano = crate::market::preseason::load_preseason_plan(&career_dir)
        .expect("plano")
        .expect("plano da janela");
    assert!(
        plano.player_poach_offer.is_some(),
        "a oferta tem de continuar viva enquanto o banco nao mudou"
    );
    assert!(
        !career_dir.join("preseason_plan.json.novo").exists(),
        "o staging do plano nao pode sobreviver a falha"
    );

    // Caminho feliz: o aumento entra no banco E a oferta sai do plano, juntos.
    let outcome = resolve_player_poach_offer_in_base_dir(&base_dir, "career_001", &oferta, false)
        .expect("resolve poach");
    assert!(outcome.applied, "a oferta viva devia ser aplicada");
    assert!(salario_do_contrato(&db_path, &oferta.current_contract_id) > salario_antes);
    let plano = crate::market::preseason::load_preseason_plan(&career_dir)
        .expect("plano")
        .expect("plano da janela");
    assert!(
        plano.player_poach_offer.is_none(),
        "a oferta devia ter sido consumida junto do commit"
    );

    let _ = fs::remove_dir_all(base_dir);
}

/// B85, TERCEIRO CAMINHO: o aceite de proposta comitava o banco primeiro e reconciliava o
/// plano depois, em best-effort. Morrer entre as duas escritas deixava o contrato já
/// trocado e o plano ainda carregando os eventos do jogador — que a semana seguinte
/// executaria por cima do assento recém-assinado. Com o staging, a falha antes do commit
/// devolve os dois ao mesmo lado.
#[test]
fn aceite_de_proposta_nao_publica_o_plano_quando_a_operacao_morre_antes_do_commit() {
    let base_dir = create_test_career_dir("aceite_antes_do_commit");
    mark_all_races_completed(&base_dir, "career_001");
    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");

    let config = AppConfig::load_or_default(&base_dir);
    let career_dir = config.saves_dir().join("career_001");
    let db_path = career_dir.join("career.db");

    let (player_id, proposal_id, contrato_antes) = {
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("jogador");
        let season = season_queries::get_active_season(&db.conn)
            .expect("temporada")
            .expect("temporada ativa");
        let contrato = latest_regular_contract_for_driver(&db.conn, &player.id);
        seed_player_proposal(
            &db.conn,
            &season.id,
            &player.id,
            &contrato.equipe_id,
            "Pendente",
        );
        (
            player.id.clone(),
            format!("MP-{}-{}", contrato.equipe_id, player.id),
            contrato.id.clone(),
        )
    };

    // Um evento pendente do jogador no plano: é ele que a reconciliação tem de remover, e
    // é por ele que se enxerga o plano andando sozinho.
    let mut plano = crate::market::preseason::load_preseason_plan(&career_dir)
        .expect("plano")
        .expect("plano da janela");
    plano.planned_events.push(PlannedEvent {
        week: 2,
        executed: false,
        event: PendingAction::ExpireContract {
            contract_id: contrato_antes.clone(),
            driver_id: player_id.clone(),
            driver_name: "jogador".to_string(),
            team_id: "T001".to_string(),
            team_name: "equipe".to_string(),
        },
    });
    plano.state.player_has_pending_proposals = true;
    crate::market::preseason::save_preseason_plan(&career_dir, &plano).expect("plano semeado");

    sabotar_commit_da_janela(true);
    let falha = respond_to_proposal_in_base_dir(&base_dir, "career_001", &proposal_id, true);
    sabotar_commit_da_janela(false);

    assert!(falha.is_err(), "a falha injetada devia derrubar o aceite");
    assert_eq!(
        status_da_proposta(&db_path, &proposal_id),
        "Pendente",
        "a proposta nao podia ter sido aceita com o banco em rollback"
    );
    assert!(
        eventos_pendentes_do_jogador(&career_dir, &player_id) > 0,
        "o plano nao pode avancar sozinho com o banco em rollback"
    );
    assert!(
        !career_dir.join("preseason_plan.json.novo").exists(),
        "o staging do plano nao pode sobreviver a falha"
    );

    // E o caminho feliz continua movendo os dois juntos.
    let resposta = respond_to_proposal_in_base_dir(&base_dir, "career_001", &proposal_id, true)
        .expect("aceite");
    assert!(resposta.success);
    assert_eq!(status_da_proposta(&db_path, &proposal_id), "Aceita");
    assert_eq!(
        eventos_pendentes_do_jogador(&career_dir, &player_id),
        0,
        "o aceite comitado tem de levar a reconciliacao do plano junto"
    );
    assert!(!career_dir.join("preseason_plan.json.novo").exists());

    let _ = fs::remove_dir_all(base_dir);
}

fn status_da_proposta(db_path: &Path, proposal_id: &str) -> String {
    let db = Database::open_existing(db_path).expect("db");
    db.conn
        .query_row(
            "SELECT status FROM market_proposals WHERE id = ?1",
            rusqlite::params![proposal_id],
            |row| row.get(0),
        )
        .expect("status da proposta")
}

fn eventos_pendentes_do_jogador(career_dir: &Path, player_id: &str) -> usize {
    crate::market::preseason::load_preseason_plan(career_dir)
        .expect("plano")
        .expect("plano da janela")
        .planned_events
        .iter()
        .filter(|event| !event.executed)
        .filter(|event| {
            crate::commands::career::market_window::pending_player_event_team_ids(
                &event.event,
                player_id,
            )
            .is_some()
        })
        .count()
}

/// Liga e desliga a falha injetada entre o staging do arquivo e o commit do banco.
fn sabotar_commit_da_janela(ligado: bool) {
    crate::commands::career::market_window::SABOTAR_COMMIT_DA_JANELA_DE_MERCADO
        .with(|interruptor| interruptor.set(ligado));
}

fn semana_do_plano(career_dir: &Path) -> i32 {
    crate::market::preseason::load_preseason_plan(career_dir)
        .expect("plano")
        .expect("plano da janela")
        .state
        .current_week
}

fn salario_do_contrato(db_path: &Path, contract_id: &str) -> f64 {
    let db = Database::open_existing(db_path).expect("db");
    db.conn
        .query_row(
            "SELECT salario_anual FROM contracts WHERE id = ?1",
            rusqlite::params![contract_id],
            |row| row.get(0),
        )
        .expect("salario do contrato")
}

/// Planta no plano da janela uma oferta de quebra de contrato do jogador, montada a
/// partir do contrato ativo dele. O braço escolhido é o de FICAR com aumento: ele mexe
/// no banco sem depender do grid do pretendente, então o que o teste mede é so a
/// coerencia entre o arquivo e o banco.
fn semear_oferta_de_quebra(
    career_dir: &Path,
    db_path: &Path,
) -> crate::market::pipeline::PlayerPoachOffer {
    let db = Database::open_existing(db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("jogador");
    let contrato = contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
        .expect("contrato do jogador")
        .expect("jogador com contrato ativo");
    let pretendente = team_queries::get_all_teams(&db.conn)
        .expect("equipes")
        .into_iter()
        .find(|team| team.id != contrato.equipe_id)
        .expect("outra equipe no mundo");
    drop(db);

    let oferta = crate::market::pipeline::PlayerPoachOffer {
        current_contract_id: contrato.id.clone(),
        current_team_id: contrato.equipe_id.clone(),
        suitor_team_id: pretendente.id.clone(),
        buyout: 0.0,
        current_salary: contrato.salario_anual,
        poacher_best: contrato.salario_anual * 2.0,
        holder_best: contrato.salario_anual + 50_000.0,
        suitor_name: pretendente.nome.clone(),
        suitor_color: pretendente.cor_primaria.clone(),
        suitor_car_rating: 50,
        current_team_name: contrato.equipe_nome.clone(),
        current_team_color: "#ffffff".to_string(),
        category_label: contrato.categoria.clone(),
        incumbent_name: None,
        player_fama: 50,
        bids: Vec::new(),
        poacher_wins: false,
    };

    let mut plano = crate::market::preseason::load_preseason_plan(career_dir)
        .expect("plano")
        .expect("plano da janela");
    plano.player_poach_offer = Some(oferta.clone());
    crate::market::preseason::save_preseason_plan(career_dir, &plano).expect("plano semeado");
    oferta
}
