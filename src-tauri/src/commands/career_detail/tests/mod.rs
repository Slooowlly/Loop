    use super::{
        build_archived_recent_results_for_driver, build_career_history_block,
        build_category_timeline, build_current_summary_block, build_driver_career_path_block,
        build_driver_form_block, career_debut_year_from_archive, expected_position_from_grid,
        fallback_injury_display_name, resolve_driver_category, CareerSeasonArchiveRow,
        HistoricalRaceResult,
    };
    use crate::constants::categories::competitive_division_label;
    use crate::models::contract::Contract;
    use crate::models::driver::Driver;
    use crate::models::enums::{InjuryType, TeamRole};

    fn sample_driver() -> Driver {
        let mut driver = Driver::new(
            "P001".to_string(),
            "Piloto Teste".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            22,
            2024,
        );
        driver.stats_carreira.corridas = 5;
        driver.stats_temporada.corridas = 5;
        driver
    }

    /// Grid SPEC (rookie): 6 equipes, 12 assentos, TODAS com o mesmo carro. Ninguém pode
    /// "esperar" posição de fundo por causa do pacote — o pacote não separa ninguém, então
    /// a expectativa honesta é o meio do grid, igual pra todo mundo.
    #[test]
    fn grid_spec_espera_o_meio_do_grid_pra_todo_mundo() {
        let grid: Vec<(f64, i32)> = vec![(0.0, 2); 6];

        assert_eq!(expected_position_from_grid(0.0, &grid), Some(6));
    }

    /// Carro claramente melhor → topo; claramente pior → fundo. O rank é por assentos, não
    /// por uma tabela de limiares absolutos.
    #[test]
    fn rank_segue_os_assentos_a_frente() {
        // 3 equipes de 2 assentos: carros 10, 5 e 1.
        let grid = [(10.0, 2), (5.0, 2), (1.0, 2)];

        assert_eq!(expected_position_from_grid(10.0, &grid), Some(1));
        assert_eq!(expected_position_from_grid(5.0, &grid), Some(3));
        assert_eq!(expected_position_from_grid(1.0, &grid), Some(5));
    }

    /// Assento VAZIO não conta: o grid é o que está na pista, não a capacidade nominal.
    #[test]
    fn assento_vazio_nao_empurra_a_expectativa() {
        // A líder só tem 1 piloto inscrito → quem vem atrás espera P2, não P3.
        let grid = [(10.0, 1), (5.0, 2)];

        assert_eq!(expected_position_from_grid(5.0, &grid), Some(2));
    }

    /// Equipe sem nenhum assento ocupado não tem expectativa a dar.
    #[test]
    fn sem_assento_ocupado_nao_ha_expectativa() {
        let grid = [(10.0, 2), (5.0, 0)];

        assert_eq!(expected_position_from_grid(5.0, &grid), None);
    }

    fn finish(rodada: i32, position: i32) -> HistoricalRaceResult {
        HistoricalRaceResult {
            rodada,
            position,
            is_dnf: false,
            has_fastest_lap: false,
        }
    }

    #[test]
    #[serial_test::serial]
    fn fallback_injury_display_name_uses_the_severity_pool() {
        rust_i18n::set_locale("pt-BR"); // nome de lesão resolve no locale ativo.
        assert_eq!(
            fallback_injury_display_name(&InjuryType::Moderada, "A"),
            "Ombro machucado"
        );
        assert_eq!(
            fallback_injury_display_name(&InjuryType::Moderada, "B"),
            "Pescoço travado"
        );
    }

    #[test]
    #[serial_test::serial]
    fn current_summary_uses_avaliacao_instead_of_em_avaliacao() {
        rust_i18n::set_locale("pt-BR"); // veredito assevera prosa PT (ver race_eval).
        let driver = sample_driver();
        let results = vec![finish(1, 12), finish(2, 13)];

        let summary = build_current_summary_block(&driver, &results, None);

        assert_eq!(summary.veredito, "Avaliação");
        assert_eq!(summary.tom, "info");
    }

    #[test]
    #[serial_test::serial]
    fn current_summary_names_bad_and_critical_seasons() {
        rust_i18n::set_locale("pt-BR");
        let driver = sample_driver();
        let bad_results = vec![finish(1, 11), finish(2, 12), finish(3, 13)];
        let critical_results = vec![finish(1, 18), finish(2, 19), finish(3, 20)];

        let bad = build_current_summary_block(&driver, &bad_results, Some(16));
        let critical = build_current_summary_block(&driver, &critical_results, Some(22));

        assert_eq!(bad.veredito, "Ruim");
        assert_eq!(bad.tom, "danger");
        assert_eq!(critical.veredito, "Crítico");
        assert_eq!(critical.tom, "danger");
    }

    #[test]
    fn archived_recent_results_marks_previous_season_without_team() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE driver_season_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL DEFAULT '',
                posicao_campeonato INTEGER,
                pontos REAL,
                snapshot_json TEXT NOT NULL
            );
            INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (
                'P001', 25, 2024, 'Piloto Teste', '', NULL, 0.0,
                '{\"corridas\":0,\"categoria\":\"\",\"ultimos_resultados\":[]}'
            );
            ",
        )
        .expect("archive setup");

        let archived =
            build_archived_recent_results_for_driver(&conn, 26, "P001").expect("archive results");

        assert!(archived.results.is_empty());
        assert_eq!(
            archived.form_context.as_deref(),
            Some("sem_time_temporada_passada")
        );
    }

    #[test]
    fn driver_form_block_exposes_previous_season_without_team_context() {
        let form = build_driver_form_block(&[], Some("sem_time_temporada_passada"));

        assert_eq!(form.momento, "sem_dados");
        assert_eq!(form.contexto.as_deref(), Some("sem_time_temporada_passada"));
    }

    #[test]
    fn career_history_block_derives_presence_marks_peak_and_mobility() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE driver_season_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL DEFAULT '',
                posicao_campeonato INTEGER,
                pontos REAL,
                snapshot_json TEXT NOT NULL
            );
            CREATE TABLE seasons (
                id TEXT PRIMARY KEY,
                numero INTEGER NOT NULL,
                ano INTEGER NOT NULL
            );
            CREATE TABLE calendar (
                id TEXT PRIMARY KEY,
                temporada_id TEXT NOT NULL,
                season_id TEXT,
                rodada INTEGER NOT NULL,
                categoria TEXT NOT NULL
            );
            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                posicao_final INTEGER NOT NULL,
                dnf INTEGER NOT NULL DEFAULT 0,
                pontos REAL NOT NULL DEFAULT 0.0
            );

            INSERT INTO seasons (id, numero, ano) VALUES
                ('S001', 1, 2020),
                ('S002', 2, 2021),
                ('S003', 3, 2022),
                ('S004', 4, 2023),
                ('S005', 5, 2024);

            INSERT INTO driver_season_archive
                (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
            VALUES
                ('P001', 1, 2020, 'Piloto Teste', 'mazda_rookie', 4, 50.0,
                 '{\"corridas\":5,\"vitorias\":0,\"podios\":1,\"pontos\":50,\"categoria\":\"mazda_rookie\"}'),
                ('P001', 2, 2021, 'Piloto Teste', 'mazda_amador', 2, 180.0,
                 '{\"corridas\":8,\"vitorias\":3,\"podios\":5,\"pontos\":180,\"categoria\":\"mazda_amador\"}'),
                ('P001', 3, 2022, 'Piloto Teste', '', NULL, 0.0,
                 '{\"corridas\":0,\"vitorias\":0,\"podios\":0,\"pontos\":0,\"categoria\":\"\"}'),
                ('P001', 4, 2023, 'Piloto Teste', '', NULL, 0.0,
                 '{\"corridas\":0,\"vitorias\":0,\"podios\":0,\"pontos\":0,\"categoria\":\"\"}'),
                ('P001', 5, 2024, 'Piloto Teste', 'gt4', 5, 90.0,
                 '{\"corridas\":10,\"vitorias\":1,\"podios\":2,\"pontos\":90,\"categoria\":\"gt4\"}'),
                ('P001', 6, 2025, 'Piloto Teste', '', NULL, 0.0,
                 '{\"corridas\":0,\"vitorias\":0,\"podios\":0,\"pontos\":0,\"categoria\":\"\"}'),
                ('P001', 7, 2026, 'Piloto Teste', 'bmw_m2', 1, 220.0,
                 '{\"corridas\":8,\"vitorias\":4,\"podios\":6,\"pontos\":220,\"categoria\":\"bmw_m2\"}');
            ",
        )
        .expect("history schema");

        for (season, races) in [("S001", 5), ("S002", 8), ("S004", 10), ("S005", 8)] {
            for rodada in 1..=races {
                conn.execute(
                    "INSERT INTO calendar (id, temporada_id, season_id, rodada, categoria)
                     VALUES (?1, ?2, ?2, ?3, 'mazda_rookie')",
                    rusqlite::params![format!("{season}_R{rodada:02}"), season, rodada],
                )
                .expect("calendar");
            }
        }

        for (race_id, team_id, position, dnf) in [
            ("S001_R01", "T1", 5, 0),
            ("S001_R02", "T1", 4, 0),
            ("S001_R03", "T1", 3, 0),
            ("S001_R04", "T1", 12, 1),
            ("S001_R05", "T1", 4, 0),
            ("S002_R01", "T2", 2, 0),
            ("S002_R02", "T2", 1, 0),
            ("S002_R03", "T2", 1, 0),
            ("S002_R04", "T2", 1, 0),
            ("S002_R05", "T2", 4, 0),
            ("S004_R01", "T3", 9, 0),
            ("S005_R01", "T3", 1, 0),
        ] {
            conn.execute(
                "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, dnf, pontos)
                 VALUES (?1, 'P001', ?2, ?3, ?4, 0.0)",
                rusqlite::params![race_id, team_id, position, dnf],
            )
            .expect("race result");
        }

        let history = build_career_history_block(&conn, "P001").expect("history block");

        assert_eq!(history.presenca.temporadas_disputadas, 4);
        assert_eq!(history.presenca.tempo_carreira, 7);
        assert_eq!(history.presenca.anos_desempregado, 3);
        assert_eq!(
            history.presenca.periodos_desempregado,
            vec!["2022->2023".to_string(), "2025".to_string()]
        );
        assert_eq!(history.presenca.categorias_disputadas, 4);
        assert_eq!(history.primeiros_marcos.primeiro_podio_corrida, Some(3));
        assert_eq!(history.primeiros_marcos.primeira_vitoria_corrida, Some(7));
        assert_eq!(history.primeiros_marcos.primeiro_dnf_corrida, Some(4));
        assert_eq!(history.auge.maior_sequencia_vitorias, 3);
        assert_eq!(
            history.auge.melhor_temporada.as_ref().map(|item| item.ano),
            Some(2026)
        );
        assert_eq!(
            history
                .auge
                .melhor_temporada
                .as_ref()
                .map(|item| item.categoria.as_str()),
            Some("bmw_m2")
        );
        assert_eq!(history.mobilidade.promocoes, 2);
        assert_eq!(history.mobilidade.rebaixamentos, 1);
        assert_eq!(history.mobilidade.equipes_defendidas, 3);
        assert!((history.mobilidade.tempo_medio_por_equipe.unwrap() - 1.3).abs() < 0.05);
    }

    #[test]
    fn category_timeline_compresses_category_stints_and_returns() {
        let seasons = vec![
            season_archive_row(2017, "mazda_rookie", 5),
            season_archive_row(2018, "mazda_rookie", 5),
            season_archive_row(2022, "mazda_amador", 8),
            season_archive_row(2023, "mazda_amador", 8),
            season_archive_row(2024, "", 0),
            season_archive_row(2025, "mazda_rookie", 5),
        ];

        let timeline = build_category_timeline(&seasons, Some("mazda_rookie"), 2025);

        assert_eq!(timeline.len(), 3);
        assert_eq!(timeline[0].categoria, "mazda_rookie");
        assert_eq!(timeline[0].ano_inicio, 2017);
        assert_eq!(timeline[0].ano_fim, 2018);
        assert_eq!(timeline[1].categoria, "mazda_amador");
        assert_eq!(timeline[1].ano_inicio, 2022);
        assert_eq!(timeline[2].categoria, "mazda_rookie");
        assert_eq!(timeline[2].ano_inicio, 2025);
    }

    #[test]
    fn category_timeline_ignores_current_special_category() {
        let seasons = vec![
            season_archive_row(2022, "mazda_rookie", 5),
            season_archive_row(2023, "", 0),
            season_archive_row(2024, "gt3", 14),
        ];

        let timeline = build_category_timeline(&seasons, Some("endurance"), 2025);

        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].categoria, "mazda_rookie");
        assert_eq!(timeline[1].categoria, "gt3");
        assert!(timeline.iter().all(|item| item.categoria != "endurance"));
    }

    #[test]
    fn career_detail_resolves_endurance_contract_as_gt3_endurance() {
        let mut driver = sample_driver();
        driver.categoria_atual = Some("endurance".to_string());
        let mut contract = Contract::new(
            "C_END_GT3".to_string(),
            driver.id.clone(),
            driver.nome.clone(),
            "T_END_GT3".to_string(),
            "GT3 Endurance Team".to_string(),
            1,
            2,
            100_000.0,
            TeamRole::Numero1,
            "endurance".to_string(),
        );
        contract.classe = Some("gt3".to_string());

        let category = resolve_driver_category(&driver, Some(&contract), None);

        assert_eq!(category.as_deref(), Some("endurance:gt3"));
        assert_eq!(
            competitive_division_label(&contract.categoria, contract.classe.as_deref()),
            "GT3 Endurance"
        );
    }

    #[test]
    fn career_detail_resolves_production_contract_as_mazda_production() {
        let mut driver = sample_driver();
        driver.categoria_atual = Some("production_challenger".to_string());
        let mut contract = Contract::new(
            "C_PROD_MAZDA".to_string(),
            driver.id.clone(),
            driver.nome.clone(),
            "T_PROD_MAZDA".to_string(),
            "Mazda Production Team".to_string(),
            1,
            2,
            70_000.0,
            TeamRole::Numero1,
            "production_challenger".to_string(),
        );
        contract.classe = Some("mazda".to_string());

        let category = resolve_driver_category(&driver, Some(&contract), None);

        assert_eq!(category.as_deref(), Some("production_challenger:mazda"));
        assert_eq!(
            competitive_division_label(&contract.categoria, contract.classe.as_deref()),
            "Mazda Production"
        );
    }

    #[test]
    fn career_debut_year_uses_earliest_competitive_archive_entry() {
        let seasons = vec![
            season_archive_row(2022, "mazda_rookie", 5),
            season_archive_row(2023, "", 0),
            season_archive_row(2024, "gt3", 14),
        ];

        assert_eq!(career_debut_year_from_archive(&seasons, 2024), 2022);
    }

    #[test]
    fn career_path_without_archive_uses_current_season_for_debutants() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE driver_season_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL DEFAULT '',
                posicao_campeonato INTEGER,
                pontos REAL,
                snapshot_json TEXT NOT NULL
            );
            CREATE TABLE seasons (
                id TEXT PRIMARY KEY,
                numero INTEGER NOT NULL,
                ano INTEGER NOT NULL
            );
            CREATE TABLE calendar (
                id TEXT PRIMARY KEY,
                temporada_id TEXT NOT NULL,
                season_id TEXT,
                rodada INTEGER NOT NULL,
                categoria TEXT NOT NULL
            );
            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                posicao_final INTEGER NOT NULL,
                dnf INTEGER NOT NULL DEFAULT 0,
                pontos REAL NOT NULL DEFAULT 0.0
            );
            ",
        )
        .expect("history schema");

        // `ano_inicio_carreira` é o ano do kart (aos 16), não a estreia: sem
        // temporada fechada a estreia é o ano corrente, nunca 2020.
        let mut rookie = sample_driver();
        rookie.ano_inicio_carreira = 2020;
        rookie.stats_carreira.corridas = 0;
        rookie.stats_carreira.temporadas = 0;

        let path =
            build_driver_career_path_block(&conn, &rookie, None, None, Some("mazda_rookie"), 2024)
                .expect("career path");

        // Ele já largou 5 vezes na temporada em curso (ainda não arquivada):
        // está no primeiro ano de carreira.
        assert_eq!(path.ano_estreia, 2024);
        assert_eq!(path.historico.presenca.tempo_carreira, 1);

        // Antes da primeira largada não há carreira nenhuma: ele ainda é um novato.
        let mut sem_largada = rookie.clone();
        sem_largada.stats_temporada.corridas = 0;
        let path = build_driver_career_path_block(
            &conn,
            &sem_largada,
            None,
            None,
            Some("mazda_rookie"),
            2024,
        )
        .expect("career path");

        assert_eq!(path.ano_estreia, 2024);
        assert_eq!(path.historico.presenca.tempo_carreira, 0);
    }

    #[test]
    fn career_path_without_archive_uses_seeded_seasons_for_veterans() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE driver_season_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL DEFAULT '',
                posicao_campeonato INTEGER,
                pontos REAL,
                snapshot_json TEXT NOT NULL
            );
            CREATE TABLE seasons (
                id TEXT PRIMARY KEY,
                numero INTEGER NOT NULL,
                ano INTEGER NOT NULL
            );
            CREATE TABLE calendar (
                id TEXT PRIMARY KEY,
                temporada_id TEXT NOT NULL,
                season_id TEXT,
                rodada INTEGER NOT NULL,
                categoria TEXT NOT NULL
            );
            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                posicao_final INTEGER NOT NULL,
                dnf INTEGER NOT NULL DEFAULT 0,
                pontos REAL NOT NULL DEFAULT 0.0
            );
            ",
        )
        .expect("history schema");

        let mut veteran = sample_driver();
        veteran.stats_carreira.temporadas = 3;
        veteran.stats_carreira.corridas = 24;

        let path = build_driver_career_path_block(&conn, &veteran, None, None, Some("gt4"), 2024)
            .expect("career path");

        assert_eq!(path.ano_estreia, 2022);
        assert_eq!(path.historico.presenca.tempo_carreira, 3);
    }

    #[test]
    fn career_history_block_derives_special_event_summary() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE driver_season_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL DEFAULT '',
                posicao_campeonato INTEGER,
                pontos REAL,
                snapshot_json TEXT NOT NULL
            );
            CREATE TABLE seasons (
                id TEXT PRIMARY KEY,
                numero INTEGER NOT NULL,
                ano INTEGER NOT NULL
            );
            CREATE TABLE calendar (
                id TEXT PRIMARY KEY,
                temporada_id TEXT NOT NULL,
                season_id TEXT,
                rodada INTEGER NOT NULL,
                categoria TEXT NOT NULL
            );
            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                posicao_final INTEGER NOT NULL,
                dnf INTEGER NOT NULL DEFAULT 0,
                pontos REAL NOT NULL DEFAULT 0.0
            );
            CREATE TABLE contracts (
                id TEXT PRIMARY KEY,
                piloto_id TEXT NOT NULL,
                piloto_nome TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                equipe_nome TEXT NOT NULL,
                temporada_inicio INTEGER NOT NULL,
                temporada_fim INTEGER NOT NULL,
                duracao_anos INTEGER NOT NULL,
                salario_anual REAL NOT NULL DEFAULT 0.0,
                papel TEXT NOT NULL DEFAULT 'Numero1',
                status TEXT NOT NULL DEFAULT 'Expirado',
                tipo TEXT NOT NULL DEFAULT 'Especial',
                categoria TEXT NOT NULL,
                classe TEXT,
                created_at TEXT NOT NULL DEFAULT ''
            );

            INSERT INTO seasons (id, numero, ano) VALUES
                ('S006', 6, 2026),
                ('S008', 8, 2028);

            INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome, temporada_inicio,
                temporada_fim, duracao_anos, tipo, categoria, classe, status
            ) VALUES
                ('CSP1', 'P001', 'Piloto Teste', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP2', 'P001', 'Piloto Teste', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP3', 'P002', 'Piloto Ranking 2', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP4', 'P002', 'Piloto Ranking 2', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP5', 'P002', 'Piloto Ranking 2', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP6', 'P002', 'Piloto Ranking 2', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP7', 'P002', 'Piloto Ranking 2', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP8', 'P002', 'Piloto Ranking 2', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP9', 'P003', 'Piloto Ranking 3', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP10', 'P003', 'Piloto Ranking 3', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP11', 'P003', 'Piloto Ranking 3', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP12', 'P003', 'Piloto Ranking 3', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP13', 'P003', 'Piloto Ranking 3', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP14', 'P004', 'Piloto Ranking 4', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP15', 'P004', 'Piloto Ranking 4', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP16', 'P004', 'Piloto Ranking 4', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP17', 'P004', 'Piloto Ranking 4', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP18', 'P005', 'Piloto Ranking 5', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP19', 'P005', 'Piloto Ranking 5', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP20', 'P005', 'Piloto Ranking 5', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado');

            INSERT INTO calendar (id, temporada_id, season_id, rodada, categoria) VALUES
                ('SP6_R01', 'S006', 'S006', 1, 'production_challenger'),
                ('SP6_R02', 'S006', 'S006', 2, 'production_challenger'),
                ('SP8_R01', 'S008', 'S008', 1, 'endurance'),
                ('SP8_R02', 'S008', 'S008', 2, 'endurance');

            INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, dnf, pontos) VALUES
                ('SP6_R01', 'P001', 'SP1', 2, 0, 18.0),
                ('SP6_R02', 'P001', 'SP1', 6, 0, 8.0),
                ('SP8_R01', 'P001', 'SP2', 1, 0, 25.0),
                ('SP8_R02', 'P001', 'SP2', 3, 0, 17.0),
                ('SP6_R01', 'P002', 'SP1', 1, 0, 25.0),
                ('SP6_R02', 'P002', 'SP1', 1, 0, 25.0),
                ('SP8_R01', 'P002', 'SP2', 2, 0, 18.0),
                ('SP8_R02', 'P002', 'SP2', 2, 0, 18.0),
                ('SP6_R01', 'P003', 'SP1', 2, 0, 18.0),
                ('SP6_R02', 'P003', 'SP1', 2, 0, 18.0),
                ('SP8_R01', 'P003', 'SP2', 2, 0, 18.0),
                ('SP8_R02', 'P003', 'SP2', 2, 0, 18.0),
                ('SP6_R01', 'P004', 'SP1', 3, 0, 15.0),
                ('SP6_R02', 'P004', 'SP1', 3, 0, 15.0),
                ('SP8_R01', 'P004', 'SP2', 3, 0, 15.0),
                ('SP8_R02', 'P004', 'SP2', 3, 0, 15.0);
            ",
        )
        .expect("special event schema");

        let history = build_career_history_block(&conn, "P001").expect("history block");
        let special = history.eventos_especiais;

        assert_eq!(special.participacoes, 2);
        assert_eq!(special.convocacoes, 2);
        assert_eq!(special.vitorias, 1);
        assert_eq!(special.podios, 3);
        assert_eq!(special.rankings.participacoes, Some(5));
        assert_eq!(special.rankings.convocacoes, Some(5));
        assert_eq!(special.rankings.vitorias, Some(2));
        assert_eq!(special.rankings.podios, Some(4));
        assert_eq!(special.timeline.len(), 2);
        assert_eq!(special.timeline[0].ano, 2026);
        assert_eq!(special.timeline[0].categoria, "production_challenger");
        assert_eq!(special.timeline[0].classe.as_deref(), Some("bmw"));
        assert_eq!(special.timeline[1].ano, 2028);
        assert_eq!(
            special.ultimo_evento.as_ref().map(|item| item.ano),
            Some(2028)
        );
        assert_eq!(
            special
                .melhor_campanha
                .as_ref()
                .map(|campaign| (campaign.ano, campaign.pontos)),
            Some((2028, 42))
        );
    }

    fn season_archive_row(ano: i32, categoria: &str, corridas: i32) -> CareerSeasonArchiveRow {
        CareerSeasonArchiveRow {
            ano,
            categoria: categoria.to_string(),
            posicao_campeonato: None,
            pontos: 0.0,
            corridas,
            vitorias: 0,
            podios: 0,
        }
    }
