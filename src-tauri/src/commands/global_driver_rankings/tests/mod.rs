    use super::*;
    use crate::db::migrations::run_all;
    use crate::db::queries::drivers::insert_driver;
    use crate::db::queries::seasons::insert_season;
    use crate::db::queries::teams::insert_team;
    use crate::models::driver::Driver;
    use crate::models::enums::{DriverStatus, InjuryType};
    use crate::models::injury::Injury;
    use crate::models::season::Season;
    use crate::models::team::placeholder_team_from_db;
    use rusqlite::Connection;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        run_all(&conn).expect("migrations");
        conn
    }

    fn driver_with_stats(
        id: &str,
        name: &str,
        category: Option<&str>,
        wins: u32,
        podiums: u32,
        titles: u32,
    ) -> Driver {
        let mut driver = Driver::new(
            id.to_string(),
            name.to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            28,
            2020,
        );
        driver.categoria_atual = category.map(str::to_string);
        driver.stats_carreira.vitorias = wins;
        driver.stats_carreira.podios = podiums;
        driver.stats_carreira.titulos = titles;
        driver.stats_carreira.poles = wins / 2;
        driver.stats_carreira.corridas = wins.max(1) * 4;
        driver.stats_carreira.pontos_total = f64::from(wins * 25 + podiums * 12);
        driver
    }

    fn insert_active_regular_contract(
        conn: &Connection,
        contract_id: &str,
        driver_id: &str,
        driver_name: &str,
        category: &str,
    ) {
        let team_id = format!("T_{contract_id}");
        insert_team(
            conn,
            &placeholder_team_from_db(
                team_id.clone(),
                format!("Equipe {contract_id}"),
                category.to_string(),
                "2026-01-01".to_string(),
            ),
        )
        .expect("insert active contract team");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome, temporada_inicio,
                duracao_anos, temporada_fim, salario, salario_anual, papel, status, tipo, categoria, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 1, 1, 10000, 10000, 'Numero1', 'Ativo', 'Regular', ?6, '2026-01-01')",
            rusqlite::params![
                contract_id,
                driver_id,
                driver_name,
                team_id,
                format!("Equipe {contract_id}"),
                category,
            ],
        )
        .expect("insert active regular contract");
    }

    fn insert_active_regular_contract_with_class(
        conn: &Connection,
        contract_id: &str,
        driver_id: &str,
        driver_name: &str,
        category: &str,
        class_name: Option<&str>,
    ) {
        let team_id = format!("T_{contract_id}");
        let mut team = placeholder_team_from_db(
            team_id.clone(),
            format!("Equipe {contract_id}"),
            category.to_string(),
            "2026-01-01".to_string(),
        );
        team.classe = class_name.map(str::to_string);
        insert_team(conn, &team).expect("insert active contract team");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome, temporada_inicio,
                duracao_anos, temporada_fim, salario, salario_anual, papel, status, tipo, categoria, classe, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 1, 1, 10000, 10000, 'Numero1', 'Ativo', 'Regular', ?6, ?7, '2026-01-01')",
            rusqlite::params![
                contract_id,
                driver_id,
                driver_name,
                team_id,
                format!("Equipe {contract_id}"),
                category,
                class_name,
            ],
        )
        .expect("insert active regular contract");
    }

    #[test]
    fn balanced_index_weights_higher_categories_without_erasing_lower_category_dominance() {
        let conn = setup_conn();
        insert_driver(
            &conn,
            &driver_with_stats("D_GT3", "GT3 Forte", Some("gt3"), 2, 3, 0),
        )
        .expect("insert gt3");
        insert_driver(
            &conn,
            &driver_with_stats("D_ROOKIE", "Rookie Forte", Some("mazda_rookie"), 2, 3, 0),
        )
        .expect("insert rookie");
        insert_driver(
            &conn,
            &driver_with_stats("D_DOM", "Rookie Dominante", Some("mazda_rookie"), 12, 16, 1),
        )
        .expect("insert dominant");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let gt3 = payload.rows.iter().find(|row| row.id == "D_GT3").unwrap();
        let rookie = payload
            .rows
            .iter()
            .find(|row| row.id == "D_ROOKIE")
            .unwrap();
        let dominant = payload.rows.iter().find(|row| row.id == "D_DOM").unwrap();

        assert!(gt3.historical_index > rookie.historical_index);
        assert!(dominant.historical_index > gt3.historical_index);
    }

    #[test]
    fn balanced_index_treats_podium_volume_without_wins_as_consistency_not_greatness() {
        let podium_collector = balanced_score("gt3", 0, 0, 178, 0, 3000.0, 391, 8);
        let proven_winner = balanced_score("gt3", 0, 20, 35, 10, 1200.0, 90, 4);

        assert!(
            proven_winner > podium_collector,
            "Declan Gauthier-like career should not outrank a frequent winner: winner={proven_winner}, podium_collector={podium_collector}"
        );
    }

    #[test]
    fn balanced_index_keeps_titles_above_large_win_totals() {
        let champion = balanced_score("gt3", 1, 6, 12, 3, 900.0, 40, 1);
        let non_champion_winner = balanced_score("gt3", 0, 12, 25, 6, 1500.0, 60, 2);

        assert!(
            champion > non_champion_winner,
            "historical index should treat titles as the top achievement: champion={champion}, non_champion={non_champion_winner}"
        );
    }

    #[test]
    fn crown_bonus_follows_user_prestige_hierarchy() {
        let title = |cat: &str, class: Option<&str>| CategoryStats {
            category: cat.to_string(),
            class_name: class.map(str::to_string),
            titles: 1,
            races: 10,
            ..Default::default()
        };
        let production1 = vec![title("production_challenger", Some("mazda"))];
        let cup_slam = vec![
            title("mazda_amador", None),
            title("toyota_amador", None),
            title("bmw_m2", None),
        ];
        let gt_slam = vec![title("gt4", None), title("gt3", None)];
        let production_slam = vec![
            title("production_challenger", Some("mazda")),
            title("production_challenger", Some("toyota")),
            title("production_challenger", Some("bmw")),
        ];
        // GT Super = GT Slam (gt4+gt3, base) + vencer a classe LMP2 (que só existe
        // dentro da Endurance) → vale 5000 + 2000 (Endurance 1 classe), pois o título
        // de LMP2 É um título de classe da Endurance.
        let gt_super_slam = vec![
            title("gt4", None),
            title("gt3", None),
            title("endurance", Some("lmp2")),
        ];
        let endurance_slam = vec![
            title("endurance", Some("gt4")),
            title("endurance", Some("gt3")),
            title("endurance", Some("lmp2")),
        ];

        assert_eq!(crown_bonus(&production1), 800.0);
        assert_eq!(crown_bonus(&cup_slam), 1500.0);
        assert_eq!(crown_bonus(&gt_slam), 2500.0);
        assert_eq!(crown_bonus(&production_slam), 3500.0);
        assert_eq!(crown_bonus(&gt_super_slam), 7000.0);
        assert_eq!(crown_bonus(&endurance_slam), 8000.0);

        // Ordem de prestígio exigida pelo user.
        let chain = [
            crown_bonus(&production1),
            crown_bonus(&cup_slam),
            crown_bonus(&gt_slam),
            crown_bonus(&production_slam),
            crown_bonus(&gt_super_slam),
            crown_bonus(&endurance_slam),
        ];
        assert!(
            chain.windows(2).all(|w| w[0] < w[1]),
            "hierarquia de coroas fora de ordem: {chain:?}"
        );
    }

    #[test]
    fn payload_includes_active_free_and_retired_drivers_with_dimmed_statuses() {
        let conn = setup_conn();
        let active = driver_with_stats("D_ACTIVE", "Piloto Ativo", Some("gt4"), 3, 5, 0);
        let free = driver_with_stats("D_FREE", "Piloto Livre", None, 1, 2, 0);
        insert_driver(&conn, &active).expect("insert active");
        insert_driver(&conn, &free).expect("insert free");
        insert_active_regular_contract(&conn, "C_ACTIVE", "D_ACTIVE", "Piloto Ativo", "gt4");
        conn.execute(
            "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "D_RET",
                "Lenda Aposentada",
                "2025",
                "gt3",
                r#"{"vitorias": 7, "podios": 12, "titulos": 1, "corridas": 30, "pontos": 220}"#,
                "Aposentadoria"
            ],
        )
        .expect("insert retired");

        let payload = build_global_driver_rankings(&conn, Some("D_FREE")).expect("payload");
        let active = payload
            .rows
            .iter()
            .find(|row| row.id == "D_ACTIVE")
            .unwrap();
        let free = payload.rows.iter().find(|row| row.id == "D_FREE").unwrap();
        let retired = payload.rows.iter().find(|row| row.id == "D_RET").unwrap();

        assert_eq!(payload.selected_driver_id.as_deref(), Some("D_FREE"));
        assert_eq!(active.status, "Ativo");
        assert_eq!(free.status, "Livre");
        assert_eq!(retired.status, "Aposentado");
        assert_eq!(free.status_tone, "dimmed");
        assert_eq!(retired.status_tone, "retired");
    }

    #[test]
    fn payload_marks_driver_without_active_regular_contract_as_free_even_with_last_category() {
        let conn = setup_conn();
        let free = driver_with_stats(
            "D_FREE_STALE",
            "Livre Com Categoria",
            Some("bmw_m2"),
            1,
            2,
            0,
        );
        insert_driver(&conn, &free).expect("insert free");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_FREE_STALE")
            .expect("free driver should remain ranked by history");

        assert_eq!(row.status, "Livre");
        assert_eq!(row.status_tone, "dimmed");
        assert_eq!(row.categoria_atual.as_deref(), Some("bmw_m2"));
    }

    #[test]
    fn payload_keeps_current_contracted_driver_without_competitive_history() {
        let conn = setup_conn();
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_ROOKIE".to_string(),
                "Equipe Rookie".to_string(),
                "mazda_rookie".to_string(),
                "2026-01-01".to_string(),
            ),
        )
        .expect("insert team");
        let mut rookie = driver_with_stats(
            "D_ROOKIE_ZERO",
            "Rookie Sem Historico",
            Some("mazda_rookie"),
            0,
            0,
            0,
        );
        rookie.stats_carreira.corridas = 0;
        rookie.stats_carreira.pontos_total = 0.0;
        insert_driver(&conn, &rookie).expect("insert rookie");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome, temporada_inicio,
                duracao_anos, temporada_fim, salario, salario_anual, papel, status, tipo, categoria, created_at
            ) VALUES (
                'C_ROOKIE_ZERO', 'D_ROOKIE_ZERO', 'Rookie Sem Historico', 'T_ROOKIE', 'Equipe Rookie', 1,
                1, 1, 10000, 10000, 'Numero1', 'Ativo', 'Regular', 'mazda_rookie', '2026-01-01'
            )",
            [],
        )
        .expect("insert active contract");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_ROOKIE_ZERO")
            .expect("current contracted rookie should be visible");

        assert_eq!(row.status, "Ativo");
        assert_eq!(row.corridas, 0);
        assert_eq!(row.historical_index, 0.0);
    }

    #[test]
    fn payload_keeps_current_endurance_driver_and_separates_gt3_divisions() {
        let conn = setup_conn();
        let mut regular_gt3 =
            driver_with_stats("D_GT3_ACTIVE", "GT3 Regular", Some("gt3"), 0, 0, 0);
        regular_gt3.stats_carreira.corridas = 0;
        regular_gt3.stats_carreira.pontos_total = 0.0;
        let mut endurance_gt3 = driver_with_stats(
            "D_END_GT3_ACTIVE",
            "GT3 Endurance",
            Some("endurance"),
            0,
            0,
            0,
        );
        endurance_gt3.stats_carreira.corridas = 0;
        endurance_gt3.stats_carreira.pontos_total = 0.0;
        insert_driver(&conn, &regular_gt3).expect("insert regular gt3");
        insert_driver(&conn, &endurance_gt3).expect("insert endurance gt3");
        insert_active_regular_contract_with_class(
            &conn,
            "C_GT3_ACTIVE",
            "D_GT3_ACTIVE",
            "GT3 Regular",
            "gt3",
            None,
        );
        insert_active_regular_contract_with_class(
            &conn,
            "C_END_GT3_ACTIVE",
            "D_END_GT3_ACTIVE",
            "GT3 Endurance",
            "endurance",
            Some("gt3"),
        );

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let regular = payload
            .rows
            .iter()
            .find(|row| row.id == "D_GT3_ACTIVE")
            .expect("regular gt3 should be visible");
        let endurance = payload
            .rows
            .iter()
            .find(|row| row.id == "D_END_GT3_ACTIVE")
            .expect("endurance gt3 should be visible");

        assert_eq!(regular.categoria_atual.as_deref(), Some("gt3"));
        assert_eq!(endurance.categoria_atual.as_deref(), Some("endurance:gt3"));
        assert_ne!(regular.categoria_atual, endurance.categoria_atual);
    }

    #[test]
    fn payload_keeps_current_production_driver_and_separates_mazda_divisions() {
        let conn = setup_conn();
        let mut mazda_amador = driver_with_stats(
            "D_MAZDA_ACTIVE",
            "Mazda Amador",
            Some("mazda_amador"),
            0,
            0,
            0,
        );
        mazda_amador.stats_carreira.corridas = 0;
        mazda_amador.stats_carreira.pontos_total = 0.0;
        let mut production_mazda = driver_with_stats(
            "D_PROD_MAZDA_ACTIVE",
            "Mazda Production",
            Some("production_challenger"),
            0,
            0,
            0,
        );
        production_mazda.stats_carreira.corridas = 0;
        production_mazda.stats_carreira.pontos_total = 0.0;
        insert_driver(&conn, &mazda_amador).expect("insert mazda amador");
        insert_driver(&conn, &production_mazda).expect("insert production mazda");
        insert_active_regular_contract_with_class(
            &conn,
            "C_MAZDA_ACTIVE",
            "D_MAZDA_ACTIVE",
            "Mazda Amador",
            "mazda_amador",
            None,
        );
        insert_active_regular_contract_with_class(
            &conn,
            "C_PROD_MAZDA_ACTIVE",
            "D_PROD_MAZDA_ACTIVE",
            "Mazda Production",
            "production_challenger",
            Some("mazda"),
        );

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let amador = payload
            .rows
            .iter()
            .find(|row| row.id == "D_MAZDA_ACTIVE")
            .expect("mazda amador should be visible");
        let production = payload
            .rows
            .iter()
            .find(|row| row.id == "D_PROD_MAZDA_ACTIVE")
            .expect("mazda production should be visible");

        assert_eq!(amador.categoria_atual.as_deref(), Some("mazda_amador"));
        assert_eq!(
            production.categoria_atual.as_deref(),
            Some("production_challenger:mazda")
        );
        assert_ne!(amador.categoria_atual, production.categoria_atual);
    }

    #[test]
    fn payload_keeps_player_driver_available_when_not_ranked() {
        let conn = setup_conn();
        insert_driver(
            &conn,
            &driver_with_stats("D_RANKED", "Piloto Ranqueado", Some("gt4"), 2, 3, 0),
        )
        .expect("insert ranked");
        let mut player = Driver::new(
            "D_PLAYER".to_string(),
            "Piloto Usuario".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            21,
            2026,
        );
        player.is_jogador = true;
        player.categoria_atual = Some("mazda_rookie".to_string());
        insert_driver(&conn, &player).expect("insert player");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");

        assert!(payload.rows.iter().all(|row| row.id != "D_PLAYER"));
        let player_row = payload.player_driver.as_ref().expect("player driver");
        assert_eq!(player_row.id, "D_PLAYER");
        assert_eq!(player_row.nome, "Piloto Usuario");
        assert!(player_row.is_jogador);
    }

    #[test]
    fn injuries_are_reported_but_do_not_reduce_historical_index() {
        let mut conn = setup_conn();
        let no_injury = driver_with_stats("D_SAFE", "Seguro", Some("gt4"), 4, 8, 0);
        let injured = driver_with_stats("D_INJ", "Lesionado", Some("gt4"), 4, 8, 0);
        insert_driver(&conn, &no_injury).expect("insert safe");
        insert_driver(&conn, &injured).expect("insert injured");
        let tx = conn.transaction().expect("tx");
        crate::db::queries::injuries::insert_injury(
            &tx,
            &Injury {
                id: "I_INJ".to_string(),
                pilot_id: "D_INJ".to_string(),
                injury_type: InjuryType::Moderada,
                injury_name: "Ombro".to_string(),
                modifier: 0.9,
                races_total: 2,
                races_remaining: 1,
                skill_penalty: 0.1,
                season: 1,
                race_occurred: "R001".to_string(),
                active: true,
            },
        )
        .expect("insert injury");
        tx.commit().expect("commit");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let safe = payload.rows.iter().find(|row| row.id == "D_SAFE").unwrap();
        let injured = payload.rows.iter().find(|row| row.id == "D_INJ").unwrap();

        assert_eq!(safe.historical_index, injured.historical_index);
        assert_eq!(safe.lesoes, 0);
        assert_eq!(injured.lesoes, 1);
    }

    #[test]
    fn injured_active_driver_keeps_active_status_label() {
        let mut conn = setup_conn();
        let mut injured =
            driver_with_stats("D_INJ_STATUS", "Piloto Lesionado", Some("gt4"), 4, 8, 0);
        injured.status = DriverStatus::Lesionado;
        insert_driver(&conn, &injured).expect("insert injured");
        insert_active_regular_contract(
            &conn,
            "C_INJ_STATUS",
            "D_INJ_STATUS",
            "Piloto Lesionado",
            "gt4",
        );
        let tx = conn.transaction().expect("tx");
        crate::db::queries::injuries::insert_injury(
            &tx,
            &Injury {
                id: "I_INJ_STATUS".to_string(),
                pilot_id: "D_INJ_STATUS".to_string(),
                injury_type: InjuryType::Moderada,
                injury_name: "Ombro".to_string(),
                modifier: 0.9,
                races_total: 2,
                races_remaining: 1,
                skill_penalty: 0.1,
                season: 1,
                race_occurred: "R001".to_string(),
                active: true,
            },
        )
        .expect("insert injury");
        tx.commit().expect("commit");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_INJ_STATUS")
            .unwrap();

        assert_eq!(row.status, "Ativo");
        assert_eq!(row.status_tone, "active");
        assert!(row.is_lesionado);
        assert_eq!(row.lesao_ativa_tipo.as_deref(), Some("Moderada"));
    }

    #[test]
    fn payload_includes_salary_career_and_retirement_context() {
        let conn = setup_conn();
        conn.execute("DELETE FROM seasons", [])
            .expect("clear seeded seasons");
        insert_season(&conn, &Season::new("S_OLD".to_string(), 1, 2024))
            .expect("insert previous season");
        season_queries::finalize_season(&conn, "S_OLD").expect("finalize previous season");
        insert_season(&conn, &Season::new("S_TEST".to_string(), 2, 2026))
            .expect("insert active season");

        let mut active = driver_with_stats("D_ACTIVE", "Piloto Ativo", Some("gt4"), 3, 5, 0);
        active.ano_inicio_carreira = 2020;
        active.stats_carreira.temporadas = 7;
        insert_driver(&conn, &active).expect("insert active");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_GT4".to_string(),
                "Equipe Azul".to_string(),
                "gt4".to_string(),
                "2026-01-01".to_string(),
            ),
        )
        .expect("insert team");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome, temporada_inicio,
                duracao_anos, temporada_fim, salario, salario_anual, papel, status, tipo, categoria, created_at
            ) VALUES (
                'C_ACTIVE', 'D_ACTIVE', 'Piloto Ativo', 'T_GT4', 'Equipe Azul', 2,
                1, 2, 250000, 250000, 'Numero1', 'Ativo', 'Regular', 'gt4', CURRENT_TIMESTAMP
            )",
            [],
        )
        .expect("insert contract");

        conn.execute(
            "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "D_RET",
                "Lenda Aposentada",
                "2024",
                "gt3",
                r#"{"vitorias": 7, "podios": 12, "titulos": 1, "corridas": 30, "pontos": 220, "ano_inicio_carreira": 2018}"#,
                "Aposentadoria"
            ],
        )
        .expect("insert retired");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let active = payload
            .rows
            .iter()
            .find(|row| row.id == "D_ACTIVE")
            .unwrap();
        let retired = payload.rows.iter().find(|row| row.id == "D_RET").unwrap();

        assert_eq!(active.salario_anual, Some(250000.0));
        assert_eq!(active.ano_inicio_carreira, Some(2020));
        assert_eq!(active.anos_carreira, Some(7));
        assert_eq!(retired.temporada_aposentadoria.as_deref(), Some("2024"));
        assert_eq!(retired.anos_aposentado, Some(2));
        assert_eq!(retired.anos_carreira, Some(7));
    }

    #[test]
    fn active_driver_debut_year_uses_earliest_competitive_archive_entry() {
        let conn = setup_conn();
        conn.execute("DELETE FROM seasons", [])
            .expect("clear seeded seasons");
        insert_season(&conn, &Season::new("S_TEST".to_string(), 1, 2025))
            .expect("insert active season");

        let mut driver =
            driver_with_stats("D_ARCHIVE_START", "Arquivo Antigo", Some("gt3"), 1, 2, 0);
        driver.ano_inicio_carreira = 2024;
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES
                ('D_ARCHIVE_START', 23, 2022, 'Arquivo Antigo', 'mazda_rookie', 4, 67.0,
                 '{\"categoria\":\"mazda_rookie\",\"corridas\":5,\"pontos\":67,\"vitorias\":0,\"podios\":2}'),
                ('D_ARCHIVE_START', 24, 2023, 'Arquivo Antigo', '', NULL, 0.0,
                 '{\"categoria\":\"\",\"corridas\":0,\"pontos\":0,\"vitorias\":0,\"podios\":0}'),
                ('D_ARCHIVE_START', 25, 2024, 'Arquivo Antigo', 'gt3', 25, 0.0,
                 '{\"categoria\":\"gt3\",\"corridas\":14,\"pontos\":0,\"vitorias\":0,\"podios\":0}')",
            [],
        )
        .expect("insert archive");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_ARCHIVE_START")
            .unwrap();

        assert_eq!(row.ano_inicio_carreira, Some(2022));
        assert_eq!(row.anos_carreira, Some(4));
    }

    #[test]
    fn active_driver_debut_year_uses_current_year_for_new_debutants() {
        let conn = setup_conn();
        conn.execute("DELETE FROM seasons", [])
            .expect("clear seeded seasons");
        insert_season(&conn, &Season::new("S_TEST".to_string(), 1, 2024))
            .expect("insert active season");

        let mut driver =
            driver_with_stats("D_NEW_ROOKIE", "Novo Rookie", Some("mazda_rookie"), 0, 0, 0);
        driver.ano_inicio_carreira = 2020;
        driver.stats_carreira.corridas = 0;
        driver.stats_carreira.temporadas = 0;
        insert_driver(&conn, &driver).expect("insert driver");
        insert_active_regular_contract(
            &conn,
            "C_NEW_ROOKIE",
            "D_NEW_ROOKIE",
            "Novo Rookie",
            "mazda_rookie",
        );

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_NEW_ROOKIE")
            .unwrap();

        assert_eq!(row.ano_inicio_carreira, Some(2024));
        assert_eq!(row.anos_carreira, Some(0));
    }

    #[test]
    fn active_driver_current_category_uses_regular_career_over_special_contract() {
        let conn = setup_conn();
        let mut driver =
            driver_with_stats("D_SPECIAL_ACTIVE", "Especial Ativo", Some("gt3"), 2, 3, 0);
        driver.ano_inicio_carreira = 2024;
        insert_driver(&conn, &driver).expect("insert driver");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_GT3".to_string(),
                "Equipe GT3".to_string(),
                "gt3".to_string(),
                "2026-01-01".to_string(),
            ),
        )
        .expect("insert regular team");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_END".to_string(),
                "Equipe Endurance".to_string(),
                "endurance".to_string(),
                "2026-01-01".to_string(),
            ),
        )
        .expect("insert special team");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome, temporada_inicio,
                duracao_anos, temporada_fim, salario, salario_anual, papel, status, tipo, categoria, created_at
            ) VALUES
                ('C_REG', 'D_SPECIAL_ACTIVE', 'Especial Ativo', 'T_GT3', 'Equipe GT3', 1,
                 2, 2, 150000, 150000, 'Numero1', 'Ativo', 'Regular', 'gt3', '2026-01-01'),
                ('C_SPEC', 'D_SPECIAL_ACTIVE', 'Especial Ativo', 'T_END', 'Equipe Endurance', 2,
                 1, 2, 50000, 50000, 'Numero1', 'Ativo', 'Especial', 'endurance', '2026-02-01')",
            [],
        )
        .expect("insert contracts");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_SPECIAL_ACTIVE")
            .unwrap();

        assert_eq!(row.categoria_atual.as_deref(), Some("gt3"));
        assert_eq!(row.equipe_nome.as_deref(), Some("Equipe GT3"));
        assert_eq!(row.salario_anual, Some(150000.0));
    }

    #[test]
    fn active_driver_current_category_ignores_contaminated_special_category_field() {
        let conn = setup_conn();
        let mut driver = driver_with_stats(
            "D_BAD_CURRENT",
            "Categoria Contaminada",
            Some("endurance"),
            1,
            2,
            0,
        );
        driver.ano_inicio_carreira = 2024;
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (
                'D_BAD_CURRENT', 25, 2024, 'Categoria Contaminada', 'gt3', 8, 80.0,
                '{\"categoria\":\"gt3\",\"corridas\":10,\"pontos\":80,\"vitorias\":1,\"podios\":2}'
            )",
            [],
        )
        .expect("insert archive");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_BAD_CURRENT")
            .unwrap();

        assert_ne!(row.categoria_atual.as_deref(), Some("endurance"));
    }

    #[test]
    fn retired_driver_points_fall_back_to_career_points_total_snapshot_field() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "D_RET_POINTS",
                "Aposentado Com Pontos",
                "2025",
                "gt3",
                r#"{"vitorias": 3, "podios": 10, "titulos": 0, "corridas": 40, "pontos_total": 612.5}"#,
                "Aposentadoria"
            ],
        )
        .expect("insert retired");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let retired = payload
            .rows
            .iter()
            .find(|row| row.id == "D_RET_POINTS")
            .unwrap();

        assert_eq!(retired.pontos, 613);
    }

    #[test]
    fn retired_driver_career_years_fall_back_to_career_seasons_snapshot_field() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "D_RET_YEARS",
                "Aposentado Com Duracao",
                "2025",
                "gt3",
                r#"{"vitorias": 3, "podios": 10, "titulos": 0, "corridas": 40, "temporadas": 18}"#,
                "Aposentadoria"
            ],
        )
        .expect("insert retired");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let retired = payload
            .rows
            .iter()
            .find(|row| row.id == "D_RET_YEARS")
            .unwrap();

        assert_eq!(retired.anos_carreira, Some(18));
    }

    #[test]
    fn retired_driver_without_any_start_has_no_career_years() {
        // Órfão que atravessou 7 temporadas sem assento e aposentou sem nunca
        // largar: o `temporadas` do snapshot não pode virar "7 anos de carreira".
        let snapshot = RetiredDriverSnapshot {
            id: "D_RET_ORPHAN".to_string(),
            name: "Aposentado Sem Largada".to_string(),
            retirement_season: "2025".to_string(),
            category: "SemCategoria".to_string(),
            stats: CategoryStats::default(),
            title_categories: Vec::new(),
            career_start_year: Some(2018),
            career_years: Some(7),
        };

        // Save COM resultado gravado (o piloto simplesmente não tem nenhum):
        // é assim que o bloco carimbado deixa de valer como carreira.
        let real_career = RealCareerIndex {
            by_driver: HashMap::new(),
            has_results: true,
        };
        let entry = build_retired_driver_entry(
            snapshot,
            2026,
            &TeamLookup::new(),
            Vec::new(),
            &real_career,
        );

        assert_eq!(entry.row.corridas, 0);
        assert_eq!(entry.row.anos_carreira, Some(0));
    }

    #[test]
    fn active_driver_without_any_start_has_no_career_years() {
        let mut orphan = driver_with_stats("D_ORPHAN", "Orfao Sem Assento", None, 0, 0, 0);
        orphan.ano_inicio_carreira = 2020;
        orphan.stats_carreira.corridas = 0;
        orphan.stats_carreira.dnfs = 0;
        orphan.stats_carreira.pontos_total = 0.0;
        // 7 fins de temporada sem assento: o acumulador soma `temporadas` mesmo
        // sem o piloto largar uma única vez.
        orphan.stats_carreira.temporadas = 7;

        assert_eq!(
            active_driver_career_years(&orphan, &CategoryStats::default(), 2020, 2026),
            Some(0)
        );

        // Uma largada basta pra existir carreira — inclusive uma que terminou em DNF.
        let started = CategoryStats {
            dnfs: 1,
            ..CategoryStats::default()
        };
        assert_eq!(
            active_driver_career_years(&orphan, &started, 2020, 2026),
            Some(7)
        );
    }

    #[test]
    fn debut_year_ignores_kart_backstory_year_after_first_start() {
        let conn = setup_conn();
        conn.execute("DELETE FROM seasons", [])
            .expect("clear seeded seasons");
        insert_season(&conn, &Season::new("S_TEST".to_string(), 27, 2026))
            .expect("insert active season");

        // Perfil do piloto do jogador: `ano_inicio_carreira` nasce como o ano do
        // kart (2026 - (idade - 16)), sem nenhuma temporada fechada. Depois da
        // primeira largada a estreia tem de ser o ano corrente, não 2022.
        let mut driver = driver_with_stats("D_DEBUT", "Estreante", Some("mazda_rookie"), 0, 0, 0);
        driver.ano_inicio_carreira = 2022;
        driver.stats_carreira.corridas = 1;
        driver.stats_carreira.temporadas = 0;
        insert_driver(&conn, &driver).expect("insert driver");
        insert_active_regular_contract(&conn, "C_DEBUT", "D_DEBUT", "Estreante", "mazda_rookie");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_DEBUT")
            .unwrap();

        assert_eq!(row.ano_inicio_carreira, Some(2026));
        assert_eq!(row.anos_carreira, Some(1));
    }

    #[test]
    fn retired_row_shows_what_he_raced_not_the_seeded_career_block() {
        let conn = setup_conn();
        // Perfil do save real: a carreira acumulada diz 120 corridas / 688 pontos,
        // mas na pista foram 40 largadas / 344 pontos. A diferença é o bloco
        // carimbado no nascimento mais a contagem dobrada de saves antigos.
        conn.execute(
            "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "D_RET_SEEDED",
                "Veterano Carimbado",
                "2025",
                "gt3",
                r#"{"vitorias": 8, "podios": 22, "titulos": 0, "corridas": 120, "pontos": 688, "temporadas": 13}"#,
                "Aposentadoria"
            ],
        )
        .expect("insert retired");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (
                'D_RET_SEEDED', 25, 2024, 'Veterano Carimbado', 'gt3', 5, 344.0,
                '{\"categoria\":\"gt3\",\"corridas\":40,\"pontos\":344,\"vitorias\":4,\"podios\":11}'
            )",
            [],
        )
        .expect("insert archive");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_RET_SEEDED")
            .unwrap();

        assert_eq!(row.corridas, 40);
        assert_eq!(row.pontos, 344);
        assert_eq!(row.vitorias, 4);
        assert_eq!(row.podios, 11);
    }

    #[test]
    fn seeded_career_block_without_any_race_result_is_not_history() {
        let conn = setup_conn();
        conn.execute("DELETE FROM seasons", [])
            .expect("clear seeded seasons");
        insert_season(&conn, &Season::new("S_TEST".to_string(), 27, 2026))
            .expect("insert active season");

        // Piloto gerado direto numa categoria não-rookie: nasce com 60 corridas
        // carimbadas por `seed_initial_career_history` e nunca largou. O save TEM
        // resultado gravado (de outro piloto), então há verdade de campo a consultar.
        let mut carimbado = driver_with_stats("D_SEED", "Carimbado", Some("gt4"), 0, 0, 0);
        carimbado.stats_carreira.corridas = 60;
        carimbado.stats_carreira.temporadas = 5;
        insert_driver(&conn, &carimbado).expect("insert driver");
        insert_active_regular_contract(&conn, "C_SEED", "D_SEED", "Carimbado", "gt4");

        let correu = driver_with_stats("D_RACED", "Correu", Some("gt4"), 1, 2, 0);
        insert_driver(&conn, &correu).expect("insert raced driver");
        insert_active_regular_contract(&conn, "C_RACED", "D_RACED", "Correu", "gt4");
        conn.execute(
            "INSERT INTO calendar (id, temporada_id, rodada, pista, categoria, clima, duracao, data)
             VALUES ('R_SEED_1', 'S_TEST', 1, 'Interlagos', 'gt4', 'Seco', 60, '2026-05-01')",
            [],
        )
        .expect("insert calendar");
        conn.execute(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_largada, posicao_final, dnf, pontos)
             VALUES ('R_SEED_1', 'D_RACED', 'T_C_RACED', 1, 1, 0, 25.0)",
            [],
        )
        .expect("insert race result");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_SEED")
            .unwrap();

        // Visível porque está na grade, mas sem uma linha de história.
        assert_eq!(row.corridas, 0);
        assert_eq!(row.anos_carreira, Some(0));
        assert_eq!(row.historical_index, 0.0);
    }

    #[test]
    fn retired_driver_titles_ignore_archived_zero_race_championships() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "D_RET_ZERO_TITLE",
                "Aposentado Sem Corrida Campeao",
                "2025",
                "endurance",
                r#"{"vitorias": 0, "podios": 0, "titulos": 5, "corridas": 135, "pontos_total": 2.0}"#,
                "Aposentadoria"
            ],
        )
        .expect("insert retired");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "D_RET_ZERO_TITLE",
                1,
                2024,
                "Aposentado Sem Corrida Campeao",
                "endurance",
                1,
                0.0,
                r#"{"categoria":"endurance","posicao_campeonato":1,"titulos":1,"corridas":0,"pontos":0,"vitorias":0,"podios":0}"#
            ],
        )
        .expect("insert invalid archive title");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let retired = payload
            .rows
            .iter()
            .find(|row| row.id == "D_RET_ZERO_TITLE")
            .unwrap();

        assert_eq!(retired.titulos, 0);
    }

    #[test]
    fn retired_driver_title_breakdown_uses_archived_winning_categories() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "D_RET_BREAKDOWN",
                "Aposentado Multiclasse",
                "2025",
                "gt3",
                r#"{"vitorias": 10, "podios": 20, "titulos": 3, "corridas": 80, "pontos_total": 900}"#,
                "Aposentadoria"
            ],
        )
        .expect("insert retired");
        for (season_number, category, points, snapshot_json) in [
            (
                1,
                "gt4",
                120.0,
                r#"{"categoria":"gt4","posicao_campeonato":1,"titulos":1,"corridas":10,"pontos":120,"vitorias":2,"podios":5}"#,
            ),
            (
                2,
                "gt3",
                180.0,
                r#"{"categoria":"gt3","posicao_campeonato":1,"titulos":1,"corridas":12,"pontos":180,"vitorias":4,"podios":7}"#,
            ),
            (
                3,
                "gt3",
                190.0,
                r#"{"categoria":"gt3","posicao_campeonato":1,"titulos":1,"corridas":12,"pontos":190,"vitorias":4,"podios":8}"#,
            ),
        ] {
            conn.execute(
                "INSERT INTO driver_season_archive (
                    piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    "D_RET_BREAKDOWN",
                    season_number,
                    2020 + season_number,
                    "Aposentado Multiclasse",
                    category,
                    1,
                    points,
                    snapshot_json
                ],
            )
            .expect("insert archive");
        }

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let retired = payload
            .rows
            .iter()
            .find(|row| row.id == "D_RET_BREAKDOWN")
            .unwrap();

        assert_eq!(retired.titulos_por_categoria.len(), 2);
        assert_eq!(retired.titulos_por_categoria[0].categoria, "gt3");
        assert_eq!(retired.titulos_por_categoria[0].titulos, 2);
        assert_eq!(retired.titulos_por_categoria[1].categoria, "gt4");
        assert_eq!(retired.titulos_por_categoria[1].titulos, 1);
    }

    #[test]
    fn archived_zero_race_championship_position_does_not_count_as_title() {
        let conn = setup_conn();
        let driver = driver_with_stats(
            "D_ZERO_TITLE",
            "Campeao Sem Corrida",
            Some("endurance"),
            0,
            0,
            0,
        );
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "D_ZERO_TITLE",
                1,
                2024,
                "Campeao Sem Corrida",
                "endurance",
                1,
                0.0,
                r#"{"categoria":"endurance","posicao_campeonato":1,"titulos":1,"corridas":0,"pontos":0,"vitorias":0,"podios":0}"#
            ],
        )
        .expect("insert invalid archive title");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "D_ZERO_TITLE",
                2,
                2025,
                "Campeao Sem Corrida",
                "gt3",
                4,
                20.0,
                r#"{"categoria":"gt3","posicao_campeonato":4,"titulos":0,"corridas":10,"pontos":20,"vitorias":0,"podios":0}"#
            ],
        )
        .expect("insert valid archive history");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_ZERO_TITLE")
            .unwrap();

        assert_eq!(row.titulos, 0);
    }

    #[test]
    fn payload_groups_titles_by_won_category() {
        let conn = setup_conn();
        let driver = driver_with_stats(
            "D_TITLE_BREAKDOWN",
            "Campeao Multiclasse",
            Some("gt3"),
            3,
            5,
            0,
        );
        insert_driver(&conn, &driver).expect("insert driver");
        for (season_number, category, position, points, snapshot_json) in [
            (
                1,
                "gt4",
                Some(1),
                160.0,
                r#"{"categoria":"gt4","posicao_campeonato":1,"titulos":1,"corridas":10,"pontos":160,"vitorias":3,"podios":6}"#,
            ),
            (
                2,
                "gt3",
                Some(1),
                190.0,
                r#"{"categoria":"gt3","posicao_campeonato":1,"titulos":1,"corridas":12,"pontos":190,"vitorias":4,"podios":7}"#,
            ),
            (
                3,
                "gt3",
                Some(1),
                210.0,
                r#"{"categoria":"gt3","posicao_campeonato":1,"titulos":1,"corridas":12,"pontos":210,"vitorias":5,"podios":8}"#,
            ),
            (
                4,
                "endurance",
                Some(1),
                0.0,
                r#"{"categoria":"endurance","posicao_campeonato":1,"titulos":1,"corridas":0,"pontos":0,"vitorias":0,"podios":0}"#,
            ),
        ] {
            conn.execute(
                "INSERT INTO driver_season_archive (
                    piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    "D_TITLE_BREAKDOWN",
                    season_number,
                    2020 + season_number,
                    "Campeao Multiclasse",
                    category,
                    position,
                    points,
                    snapshot_json
                ],
            )
            .expect("insert archive");
        }

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_TITLE_BREAKDOWN")
            .unwrap();

        assert_eq!(row.titulos, 3);
        assert_eq!(row.titulos_por_categoria.len(), 2);
        assert_eq!(row.titulos_por_categoria[0].categoria, "gt3");
        assert_eq!(row.titulos_por_categoria[0].titulos, 2);
        assert_eq!(row.titulos_por_categoria[0].anos, vec![2023, 2022]);
        assert_eq!(row.titulos_por_categoria[1].categoria, "gt4");
        assert_eq!(row.titulos_por_categoria[1].titulos, 1);
        assert_eq!(row.titulos_por_categoria[1].anos, vec![2021]);
    }

    #[test]
    fn individual_title_year_carries_champion_team_logo() {
        let conn = setup_conn();
        let driver = driver_with_stats("D_GT3_LOGO", "Campeao GT3", Some("gt3"), 3, 5, 0);
        insert_driver(&conn, &driver).expect("insert driver");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_GT3_LOGO".to_string(),
                "Equipe GT3 Logo".to_string(),
                "gt3".to_string(),
                "2024-01-01".to_string(),
            ),
        )
        .expect("insert team");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "D_GT3_LOGO",
                2,
                2024,
                "Campeao GT3",
                "gt3",
                1,
                190.0,
                r#"{"categoria":"gt3","posicao_campeonato":1,"titulos":1,"corridas":12,"pontos":190,"vitorias":4,"podios":7,"team_id":"T_GT3_LOGO"}"#
            ],
        )
        .expect("insert archive");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_GT3_LOGO")
            .unwrap();

        assert_eq!(row.titulos_por_categoria.len(), 1);
        let summary = &row.titulos_por_categoria[0];
        assert_eq!(summary.categoria, "gt3");
        assert_eq!(summary.anos, vec![2024]);
        // O ano de título individual resolve a equipe a partir do team_id do snapshot.
        assert_eq!(summary.anos_equipes.len(), 1);
        assert_eq!(summary.anos_equipes[0].ano, 2024);
        assert_eq!(
            summary.anos_equipes[0].equipe.as_deref(),
            Some("Equipe GT3 Logo")
        );
        assert!(summary.anos_equipes[0].equipe_cor.is_some());
    }

    #[test]
    fn payload_groups_special_titles_by_category_and_class() {
        let conn = setup_conn();
        let driver = driver_with_stats(
            "D_SPECIAL_TITLE_BREAKDOWN",
            "Campeao Production",
            Some("production_challenger"),
            4,
            8,
            0,
        );
        insert_driver(&conn, &driver).expect("insert driver");
        for (season_number, class_name, points, snapshot_json) in [
            (
                1,
                "mazda",
                160.0,
                r#"{"categoria":"production_challenger","classe":"mazda","posicao_campeonato":1,"titulos":1,"corridas":10,"pontos":160,"vitorias":3,"podios":6}"#,
            ),
            (
                2,
                "toyota",
                180.0,
                r#"{"categoria":"production_challenger","classe":"toyota","posicao_campeonato":1,"titulos":1,"corridas":12,"pontos":180,"vitorias":4,"podios":7}"#,
            ),
        ] {
            conn.execute(
                "INSERT INTO driver_season_archive (
                    piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    "D_SPECIAL_TITLE_BREAKDOWN",
                    season_number,
                    2020 + season_number,
                    "Campeao Production",
                    "production_challenger",
                    1,
                    points,
                    snapshot_json
                ],
            )
            .expect("insert archive");
            assert_eq!(
                class_name,
                json_string(&serde_json::from_str(snapshot_json).unwrap(), "classe").unwrap()
            );
        }

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_SPECIAL_TITLE_BREAKDOWN")
            .unwrap();

        assert_eq!(row.titulos, 2);
        assert_eq!(row.titulos_por_categoria.len(), 2);
        assert_eq!(
            row.titulos_por_categoria[0].categoria,
            "production_challenger"
        );
        assert_eq!(
            row.titulos_por_categoria[0].classe.as_deref(),
            Some("mazda")
        );
        assert_eq!(row.titulos_por_categoria[0].titulos, 1);
        assert_eq!(
            row.titulos_por_categoria[1].categoria,
            "production_challenger"
        );
        assert_eq!(
            row.titulos_por_categoria[1].classe.as_deref(),
            Some("toyota")
        );
        assert_eq!(row.titulos_por_categoria[1].titulos, 1);
    }

    #[test]
    fn archived_special_title_class_falls_back_to_team_archive() {
        let conn = setup_conn();
        let driver = driver_with_stats(
            "D_TEAM_CLASS_TITLE",
            "Campeao Endurance",
            Some("endurance"),
            5,
            9,
            0,
        );
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "INSERT INTO team_season_archive (
                team_id, season_number, ano, categoria, classe, posicao_campeonato,
                pontos, vitorias, podios, poles, corridas, titulos_construtores, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                "T_LMP2",
                3,
                2023,
                "endurance",
                "lmp2",
                1,
                220.0,
                5,
                8,
                2,
                12,
                1,
                r#"{"classe":"lmp2"}"#
            ],
        )
        .expect("insert team archive");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "D_TEAM_CLASS_TITLE",
                3,
                2023,
                "Campeao Endurance",
                "endurance",
                1,
                200.0,
                r#"{"categoria":"endurance","team_id":"T_LMP2","posicao_campeonato":1,"titulos":1,"corridas":12,"pontos":200,"vitorias":5,"podios":8}"#
            ],
        )
        .expect("insert driver archive");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_TEAM_CLASS_TITLE")
            .unwrap();

        assert_eq!(row.titulos, 1);
        assert_eq!(row.titulos_por_categoria.len(), 1);
        assert_eq!(row.titulos_por_categoria[0].categoria, "endurance");
        assert_eq!(row.titulos_por_categoria[0].classe.as_deref(), Some("lmp2"));
    }

    #[test]
    fn payload_counts_special_team_champion_title_for_driver() {
        let conn = setup_conn();
        insert_season(
            &conn,
            &Season::new("S_PRODUCTION_TITLE".to_string(), 4, 2024),
        )
        .expect("insert season");
        let driver = driver_with_stats(
            "D_TEAM_PRODUCTION_TITLE",
            "Campeao Production Equipe",
            Some("production_challenger"),
            0,
            0,
            0,
        );
        insert_driver(&conn, &driver).expect("insert driver");
        insert_driver(
            &conn,
            &driver_with_stats(
                "D_TEAMMATE",
                "Colega Production",
                Some("production_challenger"),
                0,
                0,
                0,
            ),
        )
        .expect("insert teammate");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_PRODUCTION".to_string(),
                "Equipe Production".to_string(),
                "production_challenger".to_string(),
                "2024-01-01".to_string(),
            ),
        )
        .expect("insert team");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome, temporada_inicio,
                duracao_anos, temporada_fim, salario, salario_anual, papel, status, tipo,
                categoria, classe, created_at
            ) VALUES (
                'C_TEAM_PRODUCTION_TITLE', 'D_TEAM_PRODUCTION_TITLE', 'Campeao Production Equipe',
                'T_PRODUCTION', 'Equipe Production', 4, 1, 4, 120000, 120000, 'Numero1',
                'Expirado', 'Especial', 'production_challenger', 'mazda', CURRENT_TIMESTAMP
            )",
            [],
        )
        .expect("insert special contract");
        conn.execute(
            "INSERT INTO team_season_archive (
                team_id, season_number, ano, categoria, classe, posicao_campeonato,
                pontos, vitorias, podios, poles, corridas, titulos_construtores,
                piloto_1_id, piloto_2_id, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            rusqlite::params![
                "T_PRODUCTION",
                4,
                2024,
                "production_challenger",
                Option::<String>::None,
                1,
                341.0,
                6,
                10,
                3,
                12,
                1,
                "D_TEAM_PRODUCTION_TITLE",
                "D_TEAMMATE",
                r#"{"categoria":"production_challenger","posicao_campeonato":1}"#
            ],
        )
        .expect("insert team archive");
        conn.execute(
            "INSERT INTO calendar (id, temporada_id, rodada, pista, categoria, clima, duracao, data)
             VALUES ('R_PRODUCTION_1', 'S_PRODUCTION_TITLE', 1, 'Interlagos', 'production_challenger', 'Seco', 60, '2024-05-01')",
            [],
        )
        .expect("insert calendar");
        conn.execute(
            "INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final,
                voltas_completadas, dnf, pontos
             ) VALUES
                ('R_PRODUCTION_1', 'D_TEAM_PRODUCTION_TITLE', 'T_PRODUCTION', 1, 1, 20, 0, 25.0),
                ('R_PRODUCTION_1', 'D_TEAMMATE', 'T_PRODUCTION', 2, 2, 20, 0, 18.0)",
            [],
        )
        .expect("insert race results");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_TEAM_PRODUCTION_TITLE")
            .unwrap();

        assert_eq!(row.titulos, 1);
        assert_eq!(row.titulos_por_categoria.len(), 1);
        assert_eq!(
            row.titulos_por_categoria[0].categoria,
            "production_challenger"
        );
        assert_eq!(
            row.titulos_por_categoria[0].classe.as_deref(),
            Some("mazda")
        );
    }

    #[test]
    fn team_archive_title_counts_only_the_best_scoring_driver() {
        let conn = setup_conn();
        insert_season(&conn, &Season::new("S_SPECIAL_TITLE".to_string(), 4, 2024))
            .expect("insert season");
        let first_driver = driver_with_stats(
            "D_TEAM_TITLE_P1",
            "Colega Campeao",
            Some("endurance"),
            1,
            2,
            0,
        );
        let second_driver = driver_with_stats(
            "D_TEAM_TITLE_P2",
            "Campeao Individual",
            Some("endurance"),
            2,
            3,
            0,
        );
        insert_driver(&conn, &first_driver).expect("insert first driver");
        insert_driver(&conn, &second_driver).expect("insert second driver");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_ENDURANCE_GT3".to_string(),
                "Equipe Endurance".to_string(),
                "endurance".to_string(),
                "2024-01-01".to_string(),
            ),
        )
        .expect("insert team");
        conn.execute(
            "INSERT INTO team_season_archive (
                team_id, season_number, ano, categoria, classe, posicao_campeonato,
                pontos, vitorias, podios, poles, corridas, titulos_construtores,
                piloto_1_id, piloto_2_id, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            rusqlite::params![
                "T_ENDURANCE_GT3",
                4,
                2024,
                "endurance",
                "gt3",
                1,
                330.0,
                3,
                6,
                1,
                2,
                1,
                "D_TEAM_TITLE_P1",
                "D_TEAM_TITLE_P2",
                r#"{"categoria":"endurance","classe":"gt3","posicao_campeonato":1}"#
            ],
        )
        .expect("insert team archive");
        conn.execute(
            "INSERT INTO calendar (id, temporada_id, rodada, pista, categoria, clima, duracao, data)
             VALUES
                ('R_END_1', 'S_SPECIAL_TITLE', 1, 'Spa', 'endurance', 'Seco', 120, '2024-05-01'),
                ('R_END_2', 'S_SPECIAL_TITLE', 2, 'Le Mans', 'endurance', 'Seco', 120, '2024-06-01')",
            [],
        )
        .expect("insert calendar");
        conn.execute(
            "INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final,
                voltas_completadas, dnf, pontos
             ) VALUES
                ('R_END_1', 'D_TEAM_TITLE_P1', 'T_ENDURANCE_GT3', 2, 2, 20, 0, 18.0),
                ('R_END_2', 'D_TEAM_TITLE_P1', 'T_ENDURANCE_GT3', 2, 2, 20, 0, 18.0),
                ('R_END_1', 'D_TEAM_TITLE_P2', 'T_ENDURANCE_GT3', 1, 1, 20, 0, 25.0),
                ('R_END_2', 'D_TEAM_TITLE_P2', 'T_ENDURANCE_GT3', 1, 1, 20, 0, 25.0)",
            [],
        )
        .expect("insert race results");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let first = payload
            .rows
            .iter()
            .find(|row| row.id == "D_TEAM_TITLE_P1")
            .unwrap();
        let second = payload
            .rows
            .iter()
            .find(|row| row.id == "D_TEAM_TITLE_P2")
            .unwrap();

        assert_eq!(first.titulos, 0);
        assert_eq!(second.titulos, 1);
        assert_eq!(
            second.titulos_por_categoria[0].classe.as_deref(),
            Some("gt3")
        );
        assert_eq!(second.titulos_por_categoria[0].anos, vec![2024]);
        // O título de equipe carrega a equipe campeã (resolvida pelo team_id) por ano.
        let year_teams = &second.titulos_por_categoria[0].anos_equipes;
        assert_eq!(year_teams.len(), 1);
        assert_eq!(year_teams[0].ano, 2024);
        assert_eq!(year_teams[0].equipe.as_deref(), Some("Equipe Endurance"));
    }

    #[test]
    fn special_class_entries_create_individual_champions_per_class() {
        let conn = setup_conn();
        insert_season(
            &conn,
            &Season::new("S_SPECIAL_CLASSES".to_string(), 5, 2025),
        )
        .expect("insert season");
        let bmw_driver = driver_with_stats(
            "D_PROD_BMW_CHAMP",
            "Campeao BMW",
            Some("production_challenger"),
            2,
            3,
            0,
        );
        let mazda_driver = driver_with_stats(
            "D_PROD_MAZDA_CHAMP",
            "Campeao Mazda",
            Some("production_challenger"),
            1,
            2,
            0,
        );
        insert_driver(&conn, &bmw_driver).expect("insert bmw driver");
        insert_driver(&conn, &mazda_driver).expect("insert mazda driver");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_PROD_BMW".to_string(),
                "Equipe BMW".to_string(),
                "production_challenger".to_string(),
                "2025-01-01".to_string(),
            ),
        )
        .expect("insert bmw team");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_PROD_MAZDA".to_string(),
                "Equipe Mazda".to_string(),
                "production_challenger".to_string(),
                "2025-01-01".to_string(),
            ),
        )
        .expect("insert mazda team");
        conn.execute(
            "INSERT INTO special_team_entries (
                season_id, special_category, class_name, team_id, source_category, qualified_via
             ) VALUES
                ('S_SPECIAL_CLASSES', 'production_challenger', 'bmw', 'T_PROD_BMW', 'bmw_m2', 'champion'),
                ('S_SPECIAL_CLASSES', 'production_challenger', 'mazda', 'T_PROD_MAZDA', 'mazda_amador', 'champion')",
            [],
        )
        .expect("insert special entries");
        conn.execute(
            "INSERT INTO calendar (id, temporada_id, season_id, rodada, pista, categoria, clima, duracao, data)
             VALUES
                ('R_PROD_BMW', 'S_SPECIAL_CLASSES', 'S_SPECIAL_CLASSES', 1, 'Interlagos', 'production_challenger', 'Seco', 60, '2025-05-01'),
                ('R_PROD_MAZDA', 'S_SPECIAL_CLASSES', 'S_SPECIAL_CLASSES', 1, 'Interlagos', 'production_challenger', 'Seco', 60, '2025-05-01')",
            [],
        )
        .expect("insert calendar");
        conn.execute(
            "INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final,
                voltas_completadas, dnf, pontos
             ) VALUES
                ('R_PROD_BMW', 'D_PROD_BMW_CHAMP', 'T_PROD_BMW', 1, 1, 20, 0, 25.0),
                ('R_PROD_MAZDA', 'D_PROD_MAZDA_CHAMP', 'T_PROD_MAZDA', 1, 1, 20, 0, 25.0)",
            [],
        )
        .expect("insert race results");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let bmw = payload
            .rows
            .iter()
            .find(|row| row.id == "D_PROD_BMW_CHAMP")
            .unwrap();
        let mazda = payload
            .rows
            .iter()
            .find(|row| row.id == "D_PROD_MAZDA_CHAMP")
            .unwrap();

        assert_eq!(bmw.titulos, 1);
        assert_eq!(bmw.titulos_por_categoria[0].classe.as_deref(), Some("bmw"));
        assert_eq!(mazda.titulos, 1);
        assert_eq!(
            mazda.titulos_por_categoria[0].classe.as_deref(),
            Some("mazda")
        );
    }

    #[test]
    fn regular_team_archive_does_not_create_driver_title() {
        let conn = setup_conn();
        insert_season(&conn, &Season::new("S_GT3_TEAM_TITLE".to_string(), 6, 2005))
            .expect("insert season");
        let individual_champion = driver_with_stats(
            "D_GT3_DRIVER_CHAMP",
            "Campeao Individual",
            Some("gt3"),
            2,
            4,
            0,
        );
        let team_champion_driver = driver_with_stats(
            "D_GT3_TEAM_DRIVER",
            "Piloto Equipe Campea",
            Some("gt3"),
            1,
            3,
            0,
        );
        let other_team_driver = driver_with_stats(
            "D_GT3_OTHER_DRIVER",
            "Colega Equipe Campea",
            Some("gt3"),
            1,
            2,
            0,
        );
        insert_driver(&conn, &individual_champion).expect("insert individual champion");
        insert_driver(&conn, &team_champion_driver).expect("insert team driver");
        insert_driver(&conn, &other_team_driver).expect("insert other team driver");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_GT3_TEAM_CHAMP".to_string(),
                "Equipe GT3 Campea".to_string(),
                "gt3".to_string(),
                "2005-01-01".to_string(),
            ),
        )
        .expect("insert team");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "D_GT3_DRIVER_CHAMP",
                6,
                2005,
                "Campeao Individual",
                "gt3",
                1,
                550.0,
                r#"{"categoria":"gt3","posicao_campeonato":1,"titulos":1,"corridas":20,"pontos":550,"vitorias":10,"podios":14}"#
            ],
        )
        .expect("insert driver archive");
        conn.execute(
            "INSERT INTO team_season_archive (
                team_id, season_number, ano, categoria, classe, posicao_campeonato,
                pontos, vitorias, podios, poles, corridas, titulos_construtores,
                piloto_1_id, piloto_2_id, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            rusqlite::params![
                "T_GT3_TEAM_CHAMP",
                6,
                2005,
                "gt3",
                Option::<String>::None,
                1,
                353.0,
                12,
                14,
                4,
                14,
                1,
                "D_GT3_TEAM_DRIVER",
                "D_GT3_OTHER_DRIVER",
                r#"{"categoria":"gt3","posicao_campeonato":1}"#
            ],
        )
        .expect("insert team archive");
        conn.execute(
            "INSERT INTO calendar (id, temporada_id, rodada, pista, categoria, clima, duracao, data)
             VALUES ('R_GT3_TEAM_1', 'S_GT3_TEAM_TITLE', 1, 'Spa', 'gt3', 'Seco', 60, '2005-05-01')",
            [],
        )
        .expect("insert calendar");
        conn.execute(
            "INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final,
                voltas_completadas, dnf, pontos
             ) VALUES
                ('R_GT3_TEAM_1', 'D_GT3_TEAM_DRIVER', 'T_GT3_TEAM_CHAMP', 1, 1, 20, 0, 25.0),
                ('R_GT3_TEAM_1', 'D_GT3_OTHER_DRIVER', 'T_GT3_TEAM_CHAMP', 2, 2, 20, 0, 18.0)",
            [],
        )
        .expect("insert race results");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let individual = payload
            .rows
            .iter()
            .find(|row| row.id == "D_GT3_DRIVER_CHAMP")
            .unwrap();
        let team_driver = payload
            .rows
            .iter()
            .find(|row| row.id == "D_GT3_TEAM_DRIVER")
            .unwrap();

        assert_eq!(individual.titulos, 1);
        assert_eq!(individual.titulos_por_categoria[0].anos, vec![2005]);
        assert_eq!(team_driver.titulos, 0);
    }

    #[test]
    fn retired_driver_keeps_retirement_context_when_still_present_in_drivers_table() {
        let conn = setup_conn();
        conn.execute("DELETE FROM seasons", [])
            .expect("clear seeded seasons");
        insert_season(&conn, &Season::new("S_OLD".to_string(), 1, 2024))
            .expect("insert previous season");
        season_queries::finalize_season(&conn, "S_OLD").expect("finalize previous season");
        insert_season(&conn, &Season::new("S_TEST".to_string(), 2, 2026))
            .expect("insert active season");

        let mut driver = driver_with_stats("D_RET_ACTIVE", "Aposentado Persistido", None, 0, 0, 0);
        driver.status = DriverStatus::Aposentado;
        driver.stats_carreira.corridas = 40;
        insert_driver(&conn, &driver).expect("insert retired driver");
        conn.execute(
            "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "D_RET_ACTIVE",
                "Aposentado Persistido",
                "1",
                "gt3",
                r#"{"vitorias": 8, "podios": 15, "titulos": 1, "corridas": 40, "pontos": 360, "ano_inicio_carreira": 2019}"#,
                "Aposentadoria"
            ],
        )
        .expect("insert retired snapshot");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_RET_ACTIVE")
            .unwrap();

        assert_eq!(row.status, "Aposentado");
        assert_eq!(row.categoria_atual.as_deref(), Some("gt3"));
        assert_eq!(row.temporada_aposentadoria.as_deref(), Some("2024"));
        assert_eq!(row.anos_aposentado, Some(2));
        assert_eq!(row.titulos, 1);
    }

    #[test]
    fn payload_excludes_drivers_without_competitive_history() {
        let conn = setup_conn();
        let mut empty =
            driver_with_stats("D_EMPTY", "Sem Historico", Some("mazda_rookie"), 0, 0, 0);
        empty.stats_carreira.corridas = 0;
        let mut scorer = driver_with_stats("D_SCORE", "Com Pontos", Some("mazda_rookie"), 0, 0, 0);
        scorer.stats_carreira.pontos_total = 12.0;
        scorer.stats_carreira.corridas = 2;
        insert_driver(&conn, &empty).expect("insert empty");
        insert_driver(&conn, &scorer).expect("insert scorer");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");

        assert!(payload.rows.iter().all(|row| row.id != "D_EMPTY"));
        assert!(payload.rows.iter().any(|row| row.id == "D_SCORE"));
    }

    #[test]
    fn payload_includes_historical_categories_and_active_injury_tag() {
        let mut conn = setup_conn();
        let driver = driver_with_stats("D_HIST", "Piloto Historico", Some("gt4"), 3, 5, 0);
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "D_HIST",
                1,
                2024,
                "Piloto Historico",
                "mazda_rookie",
                3,
                180.0,
                r#"{"vitorias": 2, "podios": 5, "corridas": 12, "pontos": 180}"#
            ],
        )
        .expect("insert archive");

        let tx = conn.transaction().expect("tx");
        crate::db::queries::injuries::insert_injury(
            &tx,
            &Injury {
                id: "I_HIST".to_string(),
                pilot_id: "D_HIST".to_string(),
                injury_type: InjuryType::Grave,
                injury_name: "Joelho lesionado".to_string(),
                modifier: 0.75,
                races_total: 8,
                races_remaining: 3,
                skill_penalty: 0.15,
                season: 2,
                race_occurred: "R002".to_string(),
                active: true,
            },
        )
        .expect("insert injury");
        tx.commit().expect("commit");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload.rows.iter().find(|row| row.id == "D_HIST").unwrap();

        assert_eq!(row.lesao_ativa_tipo.as_deref(), Some("Grave"));
        assert!(row.is_lesionado);
        assert!(row.categorias_historicas.contains(&"gt4".to_string()));
        assert!(row
            .categorias_historicas
            .contains(&"mazda_rookie".to_string()));
    }

    #[test]
    fn payload_infers_rookie_foundation_for_seeded_veteran_careers() {
        let conn = setup_conn();
        let mut driver = driver_with_stats("D_GT4_SEEDED", "Veterano GT4", Some("gt4"), 3, 5, 0);
        driver.stats_carreira.temporadas = 4;
        driver.stats_carreira.corridas = 38;
        driver.temporadas_na_categoria = 2;
        driver.corridas_na_categoria = 18;
        insert_driver(&conn, &driver).expect("insert seeded veteran");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_GT4_SEEDED")
            .unwrap();

        assert!(row.categorias_historicas.contains(&"gt4".to_string()));
        assert!(
            row.categorias_historicas
                .iter()
                .any(|category| matches!(category.as_str(), "mazda_rookie" | "toyota_rookie")),
            "seeded veteran should expose a rookie foundation: {:?}",
            row.categorias_historicas
        );
    }

    #[test]
    fn payload_counts_archived_championship_positions_as_titles() {
        let conn = setup_conn();
        let mut driver = driver_with_stats("D_CHAMP", "Campeao Arquivado", Some("gt4"), 3, 5, 0);
        driver.stats_carreira.titulos = 1;
        insert_driver(&conn, &driver).expect("insert champion");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "D_CHAMP",
                1,
                2025,
                "Campeao Arquivado",
                "gt4",
                1,
                220.0,
                r#"{"vitorias": 5, "podios": 8, "corridas": 10, "pontos": 220}"#
            ],
        )
        .expect("insert archive without titles field");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload.rows.iter().find(|row| row.id == "D_CHAMP").unwrap();

        assert_eq!(row.titulos, 1);
        assert_eq!(row.titles_rank, 1);
    }

    #[test]
    fn payload_reports_rank_delta_since_latest_race() {
        let conn = setup_conn();
        insert_season(&conn, &Season::new("S_TEST".to_string(), 1, 2026))
            .expect("insert active season");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_GT4".to_string(),
                "Equipe Azul".to_string(),
                "gt4".to_string(),
                "2026-01-01".to_string(),
            ),
        )
        .expect("insert team");

        let climber = driver_with_stats("D_CLIMB", "Piloto Subindo", Some("gt4"), 1, 1, 0);
        let mut falling = driver_with_stats("D_FALL", "Piloto Caindo", Some("gt4"), 0, 0, 0);
        falling.stats_carreira.pontos_total = 90.0;
        falling.stats_carreira.corridas = 3;

        insert_driver(&conn, &climber).expect("insert climber");
        insert_driver(&conn, &falling).expect("insert falling");

        conn.execute(
            "INSERT INTO calendar (id, temporada_id, rodada, pista, categoria, clima, duracao, data)
             VALUES ('R_GT4_1', 'S_TEST', 1, 'Interlagos', 'gt4', 'Seco', 60, '2026-05-03')",
            [],
        )
        .expect("insert calendar");
        conn.execute(
            "INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final,
                voltas_completadas, dnf, pontos
             ) VALUES
                ('R_GT4_1', 'D_CLIMB', 'T_GT4', 1, 1, 20, 0, 100.0),
                ('R_GT4_1', 'D_FALL', 'T_GT4', 2, 2, 20, 0, 0.0)",
            [],
        )
        .expect("insert race results");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let climber = payload.rows.iter().find(|row| row.id == "D_CLIMB").unwrap();
        let falling = payload.rows.iter().find(|row| row.id == "D_FALL").unwrap();

        assert_eq!(climber.historical_rank, 1);
        assert_eq!(climber.historical_rank_delta, Some(1));
        assert_eq!(falling.historical_rank, 2);
        assert_eq!(falling.historical_rank_delta, Some(-1));
    }
