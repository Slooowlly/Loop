    use super::{
        backup_season_internal, list_backups_in_career_dir, parse_backup_filename,
        restore_backup_internal,
    };
    use crate::commands::career::{
        advance_market_week_in_base_dir, advance_season_in_base_dir, create_career_in_base_dir,
        finalize_preseason_in_base_dir, get_player_proposals_in_base_dir,
        respond_to_proposal_in_base_dir, CreateCareerInput,
    };
    use crate::commands::race::simulate_race_weekend_in_base_dir;
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::calendar as calendar_queries;
    use crate::db::queries::contracts as contract_queries;
    use crate::db::queries::drivers as driver_queries;
    use crate::db::queries::market_proposals as market_proposal_queries;
    use crate::db::queries::seasons as season_queries;
    use crate::db::queries::teams as team_queries;
    use crate::market::proposals::{MarketProposal, ProposalStatus};
    use crate::models::enums::TeamRole;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("iracer_save_cmd_{label}_{nanos}"))
    }

    fn create_test_career_dir(label: &str) -> PathBuf {
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

        create_career_in_base_dir(&base_dir, input).expect("career should be created");
        base_dir
    }

    fn career_paths(base_dir: &Path) -> (AppConfig, PathBuf, PathBuf, PathBuf) {
        let config = AppConfig::load_or_default(base_dir);
        let career_dir = config.saves_dir().join("career_001");
        let db_path = career_dir.join("career.db");
        let meta_path = career_dir.join("meta.json");
        (config, career_dir, db_path, meta_path)
    }

    fn mark_all_races_completed(db_path: &Path) {
        let db = Database::open_existing(db_path).expect("db");
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

    fn force_complete_preseason_plan(save_dir: &Path) {
        let mut plan = crate::market::preseason::load_preseason_plan(save_dir)
            .expect("load preseason plan")
            .expect("preseason plan");
        plan.state.is_complete = true;
        plan.state.current_week = plan.state.total_weeks + 1;
        plan.state.phase = crate::market::preseason::PreSeasonPhase::Complete;
        plan.state.player_has_pending_proposals = false;
        crate::market::preseason::save_preseason_plan(save_dir, &plan)
            .expect("save completed preseason plan");
    }

    fn seed_player_regular_proposal(
        conn: &rusqlite::Connection,
        season_id: &str,
        proposal: &MarketProposal,
    ) {
        market_proposal_queries::insert_player_proposal(conn, season_id, proposal)
            .expect("insert player proposal");
    }

    #[test]
    fn backup_restore_round_trip_restores_sidecar_snapshot() {
        let base_dir = create_test_career_dir("restore_sidecars");
        let (_config, career_dir, db_path, meta_path) = career_paths(&base_dir);
        let race_results_path = career_dir.join("race_results.json");
        let resume_context_path = career_dir.join("resume_context.json");
        let briefing_path = career_dir.join("briefing_phrase_history.json");
        let preseason_path = career_dir.join("preseason_plan.json");

        fs::write(&race_results_path, "{\"version\":1}").expect("seed race results");
        fs::write(&resume_context_path, "{\"active_view\":\"preseason\"}")
            .expect("seed resume context");
        fs::write(&briefing_path, "{\"season_number\":1,\"entries\":[]}").expect("seed briefing");
        fs::write(&preseason_path, "{\"state\":{\"current_week\":1}}").expect("seed preseason");

        let original_meta = fs::read_to_string(&meta_path).expect("read original meta");
        backup_season_internal(&db_path, &career_dir, 1, &meta_path).expect("backup should work");

        fs::write(&race_results_path, "{\"version\":2}").expect("mutate race results");
        fs::remove_file(&resume_context_path).expect("remove resume context");
        fs::write(
            &briefing_path,
            "{\"season_number\":99,\"entries\":[{\"id\":\"changed\"}]}",
        )
        .expect("mutate briefing");
        fs::remove_file(&preseason_path).expect("remove preseason");
        fs::write(
            &meta_path,
            original_meta.replace("\"current_season\": 1", "\"current_season\": 99"),
        )
        .expect("mutate meta");

        restore_backup_internal(&db_path, &career_dir, 1).expect("restore should work");

        assert_eq!(
            fs::read_to_string(&race_results_path).expect("restored race results"),
            "{\"version\":1}"
        );
        assert_eq!(
            fs::read_to_string(&resume_context_path).expect("restored resume context"),
            "{\"active_view\":\"preseason\"}"
        );
        assert_eq!(
            fs::read_to_string(&briefing_path).expect("restored briefing"),
            "{\"season_number\":1,\"entries\":[]}"
        );
        assert_eq!(
            fs::read_to_string(&preseason_path).expect("restored preseason"),
            "{\"state\":{\"current_week\":1}}"
        );

        let restored_meta = fs::read_to_string(&meta_path).expect("restored meta");
        assert!(restored_meta.contains("\"current_season\": 1"));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn full_career_flow_backup_restore_round_trip() {
        let base_dir = create_test_career_dir("full_flow_backup_restore");
        let (_config, career_dir, db_path, meta_path) = career_paths(&base_dir);

        let db = Database::open_existing(&db_path).expect("db");
        let active_season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        let next_race =
            calendar_queries::get_next_race(&db.conn, &active_season.id, "mazda_rookie")
                .expect("next race query")
                .expect("pending race");

        let race_result = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate opening race");
        assert!(
            !race_result.player_race.race_results.is_empty(),
            "player race should persist race results",
        );

        let race_results_path = career_dir.join("race_results.json");
        assert!(
            race_results_path.exists(),
            "simulating a real race should create race_results.json",
        );

        mark_all_races_completed(&db_path);

        let season_result =
            advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        assert_eq!(season_result.new_year, 2025);
        assert!(season_result.preseason_initialized);
        assert!(
            season_result.promotion_result.errors.is_empty(),
            "promotion/relegation should finish without errors: {:?}",
            season_result.promotion_result.errors
        );

        let advanced_week = advance_market_week_in_base_dir(&base_dir, "career_001", None)
            .expect("advance market week");
        assert_eq!(advanced_week.week_number, 1);

        let preseason_path = career_dir.join("preseason_plan.json");
        assert!(
            preseason_path.exists(),
            "preseason plan should exist after season advance"
        );

        backup_season_internal(&db_path, &career_dir, 2, &meta_path)
            .expect("season 2 backup should work");

        let expected_meta = fs::read_to_string(&meta_path).expect("read backed-up meta");
        let expected_race_results =
            fs::read_to_string(&race_results_path).expect("read backed-up race results");
        let expected_preseason =
            fs::read_to_string(&preseason_path).expect("read backed-up preseason");

        let mutated_meta = expected_meta
            .replace("\"current_season\": 2", "\"current_season\": 99")
            .replace("\"current_year\": 2025", "\"current_year\": 2099");
        fs::write(&meta_path, mutated_meta).expect("mutate meta");
        fs::write(&race_results_path, "{\"version\":999}").expect("mutate race results");
        fs::write(&preseason_path, "{\"state\":{\"current_week\":99}}").expect("mutate preseason");

        let db = Database::open_existing(&db_path).expect("db before restore");
        db.conn
            .execute(
                "UPDATE seasons SET numero = 99, ano = 2099 WHERE status = 'EmAndamento'",
                [],
            )
            .expect("mutate active season");

        restore_backup_internal(&db_path, &career_dir, 2).expect("restore season 2 backup");

        let restored_db = Database::open_existing(&db_path).expect("restored db");
        let restored_active_season = season_queries::get_active_season(&restored_db.conn)
            .expect("restored season query")
            .expect("restored active season");
        assert_eq!(restored_active_season.numero, 2);
        assert_eq!(restored_active_season.ano, 2025);

        assert_eq!(
            fs::read_to_string(&meta_path).expect("restored meta"),
            expected_meta
        );
        assert_eq!(
            fs::read_to_string(&race_results_path).expect("restored race results"),
            expected_race_results
        );
        assert_eq!(
            fs::read_to_string(&preseason_path).expect("restored preseason"),
            expected_preseason
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn full_preseason_player_proposal_flow_reaches_season_start() {
        let base_dir = create_test_career_dir("full_preseason_player_proposal_flow");
        let (config, career_dir, db_path, _meta_path) = career_paths(&base_dir);

        mark_all_races_completed(&db_path);
        let season_result =
            advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        assert!(season_result.preseason_initialized);

        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        let active_regular =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
                .expect("active regular contract query");
        let current_regular_team_id = active_regular
            .as_ref()
            .map(|contract| contract.equipe_id.clone());
        let player_regular_category = active_regular
            .map(|contract| contract.categoria)
            .or_else(|| player.categoria_atual.clone())
            .unwrap_or_else(|| "mazda_rookie".to_string());
        let target_team = team_queries::get_teams_by_category(&db.conn, &player_regular_category)
            .expect("teams by category")
            .into_iter()
            .find(|team| current_regular_team_id.as_ref() != Some(&team.id))
            .unwrap_or_else(|| {
                team_queries::get_teams_by_category(&db.conn, &player_regular_category)
                    .expect("fallback teams by category")
                    .into_iter()
                    .next()
                    .expect("at least one team in player regular category")
            });
        let proposal = MarketProposal {
            id: format!("MP-{}-{}", target_team.id, player.id),
            equipe_id: target_team.id.clone(),
            equipe_nome: target_team.nome.clone(),
            piloto_id: player.id.clone(),
            piloto_nome: player.nome.clone(),
            categoria: target_team.categoria.clone(),
            papel: TeamRole::Numero1,
            salario_oferecido: 125_000.0,
            duracao_anos: 2,
            status: ProposalStatus::Pendente,
            motivo_recusa: None,
        };
        seed_player_regular_proposal(&db.conn, &season.id, &proposal);
        drop(db);

        let proposals =
            get_player_proposals_in_base_dir(&base_dir, "career_001").expect("player proposals");
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].proposal_id, proposal.id);

        let response = respond_to_proposal_in_base_dir(&base_dir, "career_001", &proposal.id, true)
            .expect("accept proposal");
        assert!(response.success);
        assert_eq!(response.action, "accepted");
        assert_eq!(response.remaining_proposals, 0);
        assert_eq!(
            response.new_team_name.as_deref(),
            Some(target_team.nome.as_str())
        );

        force_complete_preseason_plan(&career_dir);
        finalize_preseason_in_base_dir(&base_dir, "career_001").expect("finalize preseason");

        let finalized_db = Database::open_existing(&db_path).expect("db after finalize");
        let finalized_contract =
            contract_queries::get_active_regular_contract_for_pilot(&finalized_db.conn, &player.id)
                .expect("active contract after finalize")
                .expect("player should keep active regular contract");
        let finalized_season = season_queries::get_active_season(&finalized_db.conn)
            .expect("active season after finalize")
            .expect("active season");

        assert_eq!(finalized_contract.equipe_id, target_team.id);
        assert_eq!(finalized_season.numero, 2);
        assert_eq!(finalized_season.ano, 2025);
        assert!(
            !config
                .saves_dir()
                .join("career_001")
                .join("preseason_plan.json")
                .exists(),
            "finalizar a pre-temporada deve remover o plano salvo"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn backup_season_internal_does_not_leave_backup_when_meta_update_fails() {
        let base_dir = create_test_career_dir("meta_fail");
        let (_config, career_dir, db_path, _meta_path) = career_paths(&base_dir);
        let invalid_meta_path = career_dir.join("missing").join("meta.json");
        let backups_dir = career_dir.join("backups");
        let backup_file = backups_dir.join("temporada_001.db");
        let sidecars_dir = backups_dir.join("temporada_001.files");

        let err = backup_season_internal(&db_path, &career_dir, 1, &invalid_meta_path)
            .expect_err("invalid meta path should fail");

        assert!(err.contains("meta.json"));
        assert!(
            !backup_file.exists(),
            "failed backup should not leave final db"
        );
        assert!(
            !sidecars_dir.exists(),
            "failed backup should not leave final sidecar snapshot"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn list_backups_in_career_dir_propagates_filesystem_errors() {
        let base_dir = unique_test_dir("list_backups_error");
        fs::create_dir_all(&base_dir).expect("base dir");
        let fake_career_dir = base_dir.join("career_001");
        let backups_file = fake_career_dir.join("backups");
        fs::create_dir_all(&fake_career_dir).expect("career dir");
        fs::write(&backups_file, "not a directory").expect("seed backups file");

        let err = list_backups_in_career_dir(&fake_career_dir)
            .expect_err("read_dir failure should propagate");

        assert!(err.contains("backups"));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn restore_legacy_backup_rebuilds_meta_and_clears_stale_sidecars() {
        let base_dir = create_test_career_dir("legacy_restore");
        let (_config, career_dir, db_path, meta_path) = career_paths(&base_dir);
        let backups_dir = career_dir.join("backups");
        fs::create_dir_all(&backups_dir).expect("backups dir");

        let legacy_backup = backups_dir.join("temporada_001.db");
        let db = Database::open_existing(&db_path).expect("db");
        db.backup(&legacy_backup).expect("legacy db-only backup");

        let race_results_path = career_dir.join("race_results.json");
        let resume_context_path = career_dir.join("resume_context.json");
        let briefing_path = career_dir.join("briefing_phrase_history.json");
        let preseason_path = career_dir.join("preseason_plan.json");
        fs::write(&race_results_path, "{\"version\":2}").expect("seed race results");
        fs::write(&resume_context_path, "{\"active_view\":\"market\"}").expect("seed resume");
        fs::write(&briefing_path, "{\"season_number\":99,\"entries\":[]}").expect("seed briefing");
        fs::write(&preseason_path, "{\"state\":{\"current_week\":7}}").expect("seed preseason");

        let db = Database::open_existing(&db_path).expect("db");
        db.conn
            .execute(
                "UPDATE seasons SET numero = 99, ano = 2099 WHERE status = 'EmAndamento'",
                [],
            )
            .expect("mutate season");
        fs::write(
            &meta_path,
            fs::read_to_string(&meta_path)
                .expect("read meta")
                .replace("\"current_season\": 1", "\"current_season\": 99")
                .replace("\"current_year\": 2024", "\"current_year\": 2099"),
        )
        .expect("mutate meta");

        restore_backup_internal(&db_path, &career_dir, 1).expect("legacy restore should work");

        let restored_db = Database::open_existing(&db_path).expect("restored db");
        let active_season = season_queries::get_active_season(&restored_db.conn)
            .expect("season query")
            .expect("active season");
        assert_eq!(active_season.numero, 1);
        assert_eq!(active_season.ano, 2024);

        let restored_meta = fs::read_to_string(&meta_path).expect("restored meta");
        assert!(restored_meta.contains("\"current_season\": 1"));
        assert!(restored_meta.contains("\"current_year\": 2024"));
        assert!(!race_results_path.exists());
        assert!(!resume_context_path.exists());
        assert!(!briefing_path.exists());
        assert!(!preseason_path.exists());

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn parse_backup_filename_accepts_current_and_legacy_names() {
        assert_eq!(parse_backup_filename("temporada_007.db"), Some(7));
        assert_eq!(parse_backup_filename("season_042.db"), Some(42));
    }
