    use std::fs;

    use super::*;
    use crate::commands::career::{create_career_in_base_dir, CreateCareerInput};
    use crate::db::queries::calendar::get_next_race;
    use crate::db::queries::news as news_queries;
    use crate::models::team::placeholder_team_from_db;
    use crate::simulation::race::{ClassificationStatus, RaceDriverResult};

    #[test]
    fn telemetry_facts_vazio_sem_telemetria_ou_sem_nome() {
        use crate::iracing_sdk::telemetry_analysis::TelemetryAnalysis;
        // Sem telemetria → nada.
        let tel = TelemetryAnalysis::default();
        assert!(telemetry_context_facts(&tel, "Ana").is_empty());
        // Com telemetria mas sem nome do jogador → nada (não dá pra citar).
        let tel = TelemetryAnalysis {
            has_telemetry: true,
            ..Default::default()
        };
        assert!(telemetry_context_facts(&tel, "  ").is_empty());
    }

    #[test]
    fn telemetry_facts_cobre_ritmo_duelo_e_melhor_momento() {
        use crate::iracing_sdk::telemetry_analysis::{
            BestMoment, PaceAnalysis, RivalCard, TelemetryAnalysis,
        };
        let tel = TelemetryAnalysis {
            has_telemetry: true,
            pace: Some(PaceAnalysis {
                vs_grid_ms: -500.0, // 0,5s/volta mais rápido que o grid
                vs_grid_reliable: true,
                consistency_reliable: true,
                good_laps: 9,
                total_laps: 10,
                ..Default::default()
            }),
            rival: Some(RivalCard {
                pilot_name: "Rafael Costa".to_string(),
                laps_battled: 7,
                avg_gap_s: 0.6,
            }),
            best_moment: Some(BestMoment {
                lap: 0,
                kind: "rival_beaten".to_string(),
                positions_gained: 0,
                time_gain_ms: 0.0,
                streak: 7,
                rival_name: "Rafael Costa".to_string(),
                confidence: "alta".to_string(),
            }),
            ..Default::default()
        };
        let facts = telemetry_context_facts(&tel, "Ana Souza");
        // Cita o jogador pelo nome e cobre os três sinais.
        assert!(facts.iter().all(|f| f.contains("Ana Souza")));
        assert!(facts.iter().any(|f| f.contains("mais rápido que o ritmo do grid")));
        assert!(facts.iter().any(|f| f.contains("consistente")));
        assert!(facts.iter().any(|f| f.contains("duelo de 7 voltas com Rafael Costa")));
        assert!(facts.iter().any(|f| f.contains("levou a melhor sobre Rafael Costa")));
    }

    /// Passou o rival no meio da corrida mas foi repassado antes da bandeirada: os
    /// fatos precisam dizer QUEM terminou na frente, senão a IA escreve que o
    /// jogador deixou o rival para trás.
    #[test]
    fn telemetry_facts_dizem_quem_terminou_na_frente_no_duelo() {
        use crate::iracing_sdk::telemetry_analysis::{
            BestMoment, ChartCar, ChartTracePoint, RaceCharts, RivalCard, TelemetryAnalysis,
        };
        let ponto = |lap: f64, position: i32| ChartTracePoint {
            lap,
            gap: 0.0,
            position,
        };
        let tel = TelemetryAnalysis {
            has_telemetry: true,
            rival: Some(RivalCard {
                pilot_name: "Benedikt Muller".to_string(),
                laps_battled: 9,
                avg_gap_s: 0.8,
            }),
            best_moment: Some(BestMoment {
                lap: 5,
                kind: "rival_beaten".to_string(),
                positions_gained: 0,
                time_gain_ms: 0.0,
                streak: 0,
                rival_name: "Benedikt Muller".to_string(),
                confidence: "alta".to_string(),
            }),
            charts: Some(RaceCharts {
                cars: vec![
                    ChartCar {
                        idx: 0,
                        name: "Rodrigo Carvalho".to_string(),
                        is_player: true,
                        // Passou na volta 5 (P9 → P8) e foi repassado na última (P9).
                        points: vec![ponto(1.0, 9), ponto(5.0, 8), ponto(7.0, 9)],
                    },
                    ChartCar {
                        idx: 1,
                        name: "Benedikt Muller".to_string(),
                        is_player: false,
                        points: vec![ponto(1.0, 8), ponto(5.0, 9), ponto(7.0, 8)],
                    },
                ],
                yellow_laps: vec![],
                lap_times: vec![],
                car_lap_times: vec![],
                rival_gap: vec![],
                rival_name: "Benedikt Muller".to_string(),
            }),
            ..Default::default()
        };
        let facts = telemetry_context_facts(&tel, "Rodrigo Carvalho");
        assert!(
            facts
                .iter()
                .any(|f| f.contains("quem terminou à frente foi Benedikt Muller")),
            "desfecho do duelo: {facts:?}"
        );
        assert!(
            facts.iter().any(|f| f.contains("não sustentou a posição até o fim")),
            "melhor momento não pode virar 'levou a melhor': {facts:?}"
        );
        assert!(
            !facts.iter().any(|f| f.contains("levou a melhor")),
            "não pode afirmar vitória no duelo: {facts:?}"
        );
    }

    #[test]
    fn round_finance_context_uses_real_money_instead_of_raw_budget() {
        let mut rich = placeholder_team_from_db(
            "TRICH".to_string(),
            "Rich Team".to_string(),
            "gt4".to_string(),
            "2026-01-01".to_string(),
        );
        rich.cash_balance = 7_000_000.0;
        rich.debt_balance = 0.0;
        rich.budget = 1.0;
        rich.reputacao = 75.0;
        rich.financial_state = "healthy".to_string();

        let mut poor = rich.clone();
        poor.id = "TPOOR".to_string();
        poor.cash_balance = 150_000.0;
        poor.debt_balance = 3_000_000.0;
        poor.budget = 99.0;
        poor.reputacao = 40.0;
        poor.financial_state = "crisis".to_string();

        let rich_context = calculate_team_round_finance_context(
            &rich,
            0.0,
            4,
            0,
            0,
            8,
            35_000.0,
            8.0,
            GlobalEconomicHealth::Neutral,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        let poor_context = calculate_team_round_finance_context(
            &poor,
            0.0,
            4,
            0,
            0,
            8,
            35_000.0,
            8.0,
            GlobalEconomicHealth::Neutral,
            0.0,
            0.0,
            0.0,
            1.0,
        );

        assert!(rich_context.sponsorship_income > poor_context.sponsorship_income);
        assert!(poor_context.debt_service_cost > rich_context.debt_service_cost);
    }

    #[test]
    fn fama_do_lineup_sobe_o_patrocinio() {
        // Mesmo time, mesma reputação: um lineup famoso (presença 85) capta MAIS
        // patrocínio que um sem fama (presença 0). É o motor da "2ª moeda".
        let mut team = placeholder_team_from_db(
            "TFAME".to_string(),
            "Fame Team".to_string(),
            "gt4".to_string(),
            "2026-01-01".to_string(),
        );
        team.reputacao = 50.0;
        let sem_fama = calculate_team_round_finance_context(
            &team, 0.0, 4, 0, 0, 8, 35_000.0, 8.0, GlobalEconomicHealth::Neutral, 0.0, 0.0, 0.0,
            1.0,
        );
        let com_estrela = calculate_team_round_finance_context(
            &team, 85.0, 4, 0, 0, 8, 35_000.0, 8.0, GlobalEconomicHealth::Neutral, 0.0, 0.0, 0.0,
            1.0,
        );
        assert!(
            com_estrela.sponsorship_income > sem_fama.sponsorship_income,
            "lineup famoso deve captar mais patrocínio: {} vs {}",
            com_estrela.sponsorship_income,
            sem_fama.sponsorship_income
        );
    }

    #[test]
    fn fama_do_lineup_sobe_a_bilheteria() {
        // Mesmo evento e mesmo grid: um lineup famoso leva uma fatia MAIOR da bilheteria
        // que um anônimo (cota competitiva por fama). É a 2ª receita de fama da Fase 3.
        let mut team = placeholder_team_from_db(
            "TGATE".to_string(),
            "Gate Team".to_string(),
            "gt4".to_string(),
            "2026-01-01".to_string(),
        );
        team.reputacao = 50.0;
        // Evento de prestígio 60, grid com presença total 300 e 8 times.
        let anonimo = calculate_team_round_finance_context(
            &team, 30.0, 0, 0, 0, 8, 35_000.0, 8.0, GlobalEconomicHealth::Neutral, 0.0, 60.0,
            300.0, 8.0,
        );
        let estrela = calculate_team_round_finance_context(
            &team, 150.0, 0, 0, 0, 8, 35_000.0, 8.0, GlobalEconomicHealth::Neutral, 0.0, 60.0,
            300.0, 8.0,
        );
        assert!(
            estrela.gate_income > anonimo.gate_income,
            "lineup famoso deve puxar mais bilheteria: {} vs {}",
            estrela.gate_income,
            anonimo.gate_income
        );
        assert!(anonimo.gate_income > 0.0, "piso de público garante bilheteria > 0");
    }

    fn sample_driver_result(
        pilot_id: &str,
        team_id: &str,
        finish_position: i32,
    ) -> RaceDriverResult {
        RaceDriverResult {
            pilot_id: pilot_id.to_string(),
            pilot_name: pilot_id.to_string(),
            team_id: team_id.to_string(),
            team_name: team_id.to_string(),
            grid_position: finish_position,
            finish_position,
            positions_gained: 0,
            best_lap_time_ms: 90_000.0 + finish_position as f64,
            total_race_time_ms: 900_000.0 + finish_position as f64,
            gap_to_winner_ms: if finish_position == 1 {
                0.0
            } else {
                finish_position as f64
            },
            is_dnf: false,
            dnf_reason: None,
            dnf_segment: None,
            incidents_count: 0,
            incidents: Vec::new(),
            has_fastest_lap: false,
            points_earned: 0,
            is_jogador: false,
            laps_completed: 20,
            final_tire_wear: 0.5,
            final_physical: 0.8,
            classification_status: ClassificationStatus::Finished,
            notable_incident: None,
            dnf_catalog_id: None,
            damage_origin_segment: None,
        }
    }

    fn sample_special_team(team_id: &str, class_name: &str) -> Team {
        let mut team = placeholder_team_from_db(
            team_id.to_string(),
            team_id.to_string(),
            "endurance".to_string(),
            "2026-01-01".to_string(),
        );
        team.classe = Some(class_name.to_string());
        team
    }

    #[test]
    fn special_results_are_scored_by_class_position() {
        let teams = vec![
            sample_special_team("LMP-A", "lmp2"),
            sample_special_team("LMP-B", "lmp2"),
            sample_special_team("GT3-A", "gt3"),
            sample_special_team("GT4-A", "gt4"),
        ];
        let mut result = RaceResult {
            qualifying_results: Vec::new(),
            race_results: vec![
                sample_driver_result("P-LMP-A", "LMP-A", 1),
                sample_driver_result("P-GT3-A", "GT3-A", 2),
                sample_driver_result("P-GT4-A", "GT4-A", 3),
                sample_driver_result("P-LMP-B", "LMP-B", 4),
            ],
            pole_sitter_id: "P-LMP-A".to_string(),
            winner_id: "P-LMP-A".to_string(),
            fastest_lap_id: String::new(),
            total_laps: 20,
            weather: "dry".to_string(),
            track_name: "Test".to_string(),
            total_incidents: 0,
            total_dnfs: 0,
            main_incident_count: 0,
            notable_incident_pilot_ids: Vec::new(),
            most_positions_gained_id: None,
            caution_segments: Vec::new(),
            applied_mechanicals: Vec::new(),
        };

        apply_special_class_scoring(&mut result, &teams, true);

        let by_pilot: std::collections::HashMap<_, _> = result
            .race_results
            .iter()
            .map(|entry| (entry.pilot_id.as_str(), entry))
            .collect();

        assert_eq!(by_pilot["P-LMP-A"].finish_position, 1);
        assert_eq!(by_pilot["P-LMP-A"].points_earned, 35);
        assert_eq!(by_pilot["P-LMP-B"].finish_position, 2);
        assert_eq!(by_pilot["P-LMP-B"].points_earned, 28);
        assert_eq!(by_pilot["P-GT3-A"].finish_position, 1);
        assert_eq!(by_pilot["P-GT3-A"].points_earned, 35);
        assert_eq!(by_pilot["P-GT4-A"].finish_position, 1);
        assert_eq!(by_pilot["P-GT4-A"].points_earned, 35);
    }

    /// Fase 7 ponta a ponta: peça no fim da vida entra na corrida SIMULADA → a corrida cobra
    /// o preço → o desfecho fica gravado com a peça culpada e o tempo perdido.
    ///
    /// É o buraco que a Fase 7 fechou: antes disto, corrida não-dirigida não quebrava peça
    /// nenhuma e a tela de debrief não tinha o que mostrar. O carro é forçado ALÉM da parede
    /// (`HARD_WALL` = 1.20) de propósito — ali a falha é certa, então o teste mede o WIRING
    /// (roll → simulação → `race_breakdowns`), não a sorte do modelo.
    #[test]
    fn corrida_simulada_grava_a_peca_que_quebrou_e_o_tempo_perdido() {
        use crate::db::queries::team_car as tcq;

        let base_dir = unique_test_dir("fase7_quebra_na_sim");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let mut db = Database::open_existing(&db_path).expect("db");

        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");

        // Detona TODOS os carros da categoria: cada peça entra além da parede. Assim o grid
        // inteiro tem falha forçada e o teste não depende de qual time o sorteio escolheria.
        let teams = team_queries::get_teams_by_category(&db.conn, "mazda_rookie").expect("teams");
        assert!(!teams.is_empty(), "categoria precisa ter equipes");
        for team in &teams {
            if let Ok(Some(mut car)) = tcq::get_team_car(&db.conn, &team.id) {
                for part in car.parts.iter_mut() {
                    part.wear = 1.30; // > HARD_WALL (1.20) → falha certa na 1ª volta
                }
                tcq::upsert_team_car(&db.conn, &team.id, &car).expect("upsert car gasto");
            }
        }

        let (result, _) = simulate_category_race(&mut db, &race, false).expect("simular corrida");

        let rows = crate::db::queries::race_breakdowns::get_breakdowns_for_race(&db.conn, &race.id)
            .expect("ler quebras");
        assert!(
            !rows.is_empty(),
            "carro todo além da parede tinha que gravar quebra; veio vazio"
        );

        // Cada linha carrega o culpado e a consequência — é isso que a tela lê.
        for row in &rows {
            assert!(!row.part.is_empty(), "quebra sem peça culpada");
            assert!(!row.label.is_empty(), "quebra sem descrição do problema");
            assert!(row.lap >= 1, "quebra sem volta");
            match row.severity.as_str() {
                "dnf" => assert!(row.penalty_secs.is_none(), "DNF não tem tempo de box"),
                "light" | "heavy" => assert!(
                    row.penalty_secs.unwrap_or(0) > 0,
                    "penalidade de box tem que custar tempo"
                ),
                other => panic!("severidade inesperada: {other}"),
            }
        }

        // Nada de quebra fantasma: só quem está no resultado pode ter linha gravada.
        for row in &rows {
            assert!(
                result
                    .race_results
                    .iter()
                    .any(|r| r.pilot_id == row.driver_id),
                "quebra gravada para piloto fora do resultado: {}",
                row.driver_id
            );
        }

        // Quem abandonou POR quebra tem que sair da corrida com a frase da peça como motivo.
        for row in rows.iter().filter(|r| r.severity == "dnf") {
            let entry = result
                .race_results
                .iter()
                .find(|r| r.pilot_id == row.driver_id)
                .expect("piloto no resultado");
            assert!(entry.is_dnf, "quebra fatal não tirou o carro da corrida");
            assert_eq!(entry.dnf_reason.as_deref(), Some(row.label.as_str()));
        }
    }

    #[test]
    fn production_special_race_uses_regular_contract_grid_without_special_entries() {
        let base_dir = unique_test_dir("production_regular_special_grid");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let mut db = Database::open_existing(&db_path).expect("db");
        force_legacy_blocoregular_state(&db);
        mark_regular_races_completed(&db);
        crate::convocation::advance_to_convocation_window(&db.conn).expect("advance convocation");
        crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");

        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let special_entries: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM special_team_entries
                 WHERE season_id = ?1 AND special_category = 'production_challenger'",
                rusqlite::params![season.id],
                |row| row.get(0),
            )
            .expect("count production special entries");
        assert_eq!(special_entries, 0);

        let next_race = get_next_race(&db.conn, &season.id, "production_challenger")
            .expect("next production race")
            .expect("pending production race");
        let (result, _) = simulate_category_race(&mut db, &next_race, false)
            .expect("simulate production special race");

        assert_eq!(result.race_results.len(), 36);
        assert_eq!(
            result
                .race_results
                .iter()
                .map(|entry| entry.team_id.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len(),
            18
        );
        for entry in &result.race_results {
            let contract =
                contract_queries::get_active_regular_contract_for_pilot(&db.conn, &entry.pilot_id)
                    .expect("regular contract")
                    .expect("driver should have regular contract");
            assert_eq!(contract.equipe_id, entry.team_id);
            assert_eq!(contract.categoria, "production_challenger");
            assert!(
                matches!(contract.classe.as_deref(), Some("mazda" | "toyota" | "bmw")),
                "contrato Production deve carregar classe real: {:?}",
                contract.classe
            );
        }

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn endurance_special_race_uses_regular_contract_grid_with_lmp2_class_teams() {
        let base_dir = unique_test_dir("endurance_regular_special_grid");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let mut db = Database::open_existing(&db_path).expect("db");
        force_legacy_blocoregular_state(&db);
        mark_regular_races_completed(&db);
        crate::convocation::advance_to_convocation_window(&db.conn).expect("advance convocation");
        crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");

        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let special_entries: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM special_team_entries
                 WHERE season_id = ?1 AND special_category = 'endurance'",
                rusqlite::params![season.id],
                |row| row.get(0),
            )
            .expect("count endurance special entries");
        assert_eq!(special_entries, 0);

        let next_race = get_next_race(&db.conn, &season.id, "endurance")
            .expect("next endurance race")
            .expect("pending endurance race");
        let (result, _) = simulate_category_race(&mut db, &next_race, false)
            .expect("simulate endurance special race");

        let unique_team_ids: std::collections::HashSet<_> = result
            .race_results
            .iter()
            .map(|entry| entry.team_id.as_str())
            .collect();
        let lmp2_team_count = unique_team_ids
            .iter()
            .filter(|team_id| {
                crate::db::queries::teams::get_team_by_id(&db.conn, team_id)
                    .expect("team lookup")
                    .is_some_and(|team| {
                        team.categoria == "endurance" && team.classe.as_deref() == Some("lmp2")
                    })
            })
            .count();

        assert_eq!(result.race_results.len(), 36);
        assert_eq!(unique_team_ids.len(), 18);
        assert_eq!(lmp2_team_count, 6);
        for entry in &result.race_results {
            let team = crate::db::queries::teams::get_team_by_id(&db.conn, &entry.team_id)
                .expect("team lookup")
                .expect("team");
            let contract =
                contract_queries::get_active_regular_contract_for_pilot(&db.conn, &entry.pilot_id)
                    .expect("regular contract")
                    .expect("driver should have regular contract");
            assert_eq!(contract.equipe_id, entry.team_id);

            assert_eq!(team.categoria, "endurance");
            assert_eq!(contract.categoria, "endurance");
            assert!(
                matches!(contract.classe.as_deref(), Some("gt4" | "gt3" | "lmp2")),
                "contrato Endurance deve carregar classe real: {:?}",
                contract.classe
            );
        }

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_updates_state() {
        let base_dir = unique_test_dir("simulate_weekend");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");

        let result = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        assert_eq!(result.player_race.race_results.len(), 12);
        assert!(
            result.other_categories.total_races_simulated >= 1,
            "same-week categories should be simulated with the player's race"
        );

        let updated_db = Database::open_existing(&db_path).expect("reopen db");
        let season_after = season_queries::get_active_season(&updated_db.conn)
            .expect("season after")
            .expect("active season after");
        assert_eq!(season_after.rodada_atual, 2);

        let completed = calendar_queries::get_calendar_entry_by_id(&updated_db.conn, &next_race.id)
            .expect("race by id")
            .expect("calendar entry");
        assert_eq!(completed.status.as_str(), "Concluida");

        let driver = driver_queries::get_player_driver(&updated_db.conn).expect("player driver");
        assert!(driver.stats_temporada.corridas >= 1);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn simulate_race_weekend_moves_9d_season_to_closing_after_last_race() {
        let base_dir = unique_test_dir("simulate_last_9d_race");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        db.conn
            .execute(
                "UPDATE calendar SET status = 'Concluida'
                 WHERE COALESCE(season_id, temporada_id) = ?1 AND id <> ?2",
                rusqlite::params![&season.id, &next_race.id],
            )
            .expect("complete all other races");
        drop(db);

        simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate last race");

        let updated_db = Database::open_existing(&db_path).expect("reopen db");
        let season_after = season_queries::get_active_season(&updated_db.conn)
            .expect("season after")
            .expect("active season after");
        assert_eq!(
            season_after.fase,
            crate::models::enums::SeasonPhase::Encerramento
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_updates_team_finance_snapshot() {
        let base_dir = unique_test_dir("simulate_team_finance");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let contract =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
                .expect("active contract")
                .expect("player contract");
        let team_before = team_queries::get_team_by_id(&db.conn, &contract.equipe_id)
            .expect("team before")
            .expect("existing team before");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        drop(db);

        simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let team_after = team_queries::get_team_by_id(&updated_db.conn, &contract.equipe_id)
            .expect("team after")
            .expect("existing team after");

        assert_ne!(team_after.cash_balance, team_before.cash_balance);
        assert!(
            team_after.last_round_income > 0.0,
            "team should record round income"
        );
        assert!(
            team_after.last_round_expenses > 0.0,
            "team should record round expenses"
        );
        assert_eq!(
            team_after.last_round_net,
            team_after.last_round_income - team_after.last_round_expenses
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_applies_crisis_finance_event() {
        let base_dir = unique_test_dir("simulate_crisis_finance");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let contract =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
                .expect("active contract")
                .expect("player contract");
        let mut team = team_queries::get_team_by_id(&db.conn, &contract.equipe_id)
            .expect("team before")
            .expect("existing team before");
        team.cash_balance = -100_000.0;
        team.debt_balance = 850_000.0;
        team.financial_state = "collapse".to_string();
        team_queries::update_team(&db.conn, &team).expect("update crisis team");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        drop(db);

        simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let team_after = team_queries::get_team_by_id(&updated_db.conn, &contract.equipe_id)
            .expect("team after")
            .expect("existing team after");

        assert!(team_after.cash_balance > -100_000.0);
        assert!(team_after.debt_balance > 850_000.0);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_rejects_completed_race() {
        let base_dir = unique_test_dir("simulate_completed");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");

        simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("first simulation");
        let error = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect_err("second simulation should fail");

        assert!(
            error.contains("ja foi concluida ou simulada"),
            "Erro inesperado: {}",
            error
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_rejects_out_of_order_race() {
        let base_dir = unique_test_dir("simulate_wrong_order");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let schedule =
            calendar_queries::get_calendar(&db.conn, &season.id, "mazda_rookie").expect("schedule");
        let later_race = schedule
            .into_iter()
            .find(|entry| entry.rodada == 2)
            .expect("round 2 race");

        let error = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &later_race.id)
            .expect_err("out of order race should fail");

        assert!(
            error.contains("proxima corrida valida"),
            "erro inesperado: {}",
            error
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_rejects_other_category_race() {
        let base_dir = unique_test_dir("simulate_wrong_category");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let other_category_race = get_next_race(&db.conn, &season.id, "gt3")
            .expect("next gt3 race")
            .expect("pending gt3 race");

        let error =
            simulate_race_weekend_in_base_dir(&base_dir, "career_001", &other_category_race.id)
                .expect_err("other category race should fail");

        assert!(
            error.contains("proxima corrida valida"),
            "erro inesperado: {}",
            error
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_rejects_active_driver_without_team() {
        let base_dir = unique_test_dir("simulate_orphan_driver");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let mut db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        let player = driver_queries::get_player_driver(&db.conn).expect("player driver");
        let player_team = team_queries::get_teams_by_category(&db.conn, "mazda_rookie")
            .expect("teams")
            .into_iter()
            .find(|team| {
                team.piloto_1_id.as_deref() == Some(player.id.as_str())
                    || team.piloto_2_id.as_deref() == Some(player.id.as_str())
            })
            .expect("player team");
        team_queries::remove_pilot_from_team(&db.conn, &player.id, &player_team.id)
            .expect("remove player from team");

        let error = simulate_category_race(&mut db, &next_race, true)
            .expect_err("active driver without team should fail");
        assert!(
            error.contains("Pilotos ativos sem equipe"),
            "erro inesperado: {}",
            error
        );
        assert!(
            error.contains(&player.id),
            "mensagem deveria apontar piloto orfao: {}",
            error
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_persists_news() {
        let base_dir = unique_test_dir("simulate_news");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        align_next_race_display_date(&db.conn, &season.id, "gt3", &next_race.display_date);
        news_queries::delete_all_news(&db.conn).expect("clear news");
        drop(db);

        simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let news = news_queries::get_recent_news(&updated_db.conn, 50).expect("recent news");
        assert!(
            news.iter()
                .any(|item| item.categoria_id.as_deref() == Some("mazda_rookie")),
            "deveria existir noticia da corrida do jogador"
        );
        assert!(
            news.iter()
                .any(|item| item.categoria_id.as_deref() == Some("gt3")),
            "deveria existir noticia de outra categoria simulada"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_ignores_invalid_meta_after_persisting_race() {
        let base_dir = unique_test_dir("simulate_invalid_meta");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let career_dir = config.saves_dir().join("career_001");
        let db_path = career_dir.join("career.db");
        let meta_path = career_dir.join("meta.json");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        drop(db);

        fs::write(&meta_path, "{meta invalida").expect("corrupt meta");

        let result = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id);
        assert!(
            result.is_ok(),
            "simulacao nao deveria falhar por meta invalida"
        );

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let completed = calendar_queries::get_calendar_entry_by_id(&updated_db.conn, &next_race.id)
            .expect("race by id")
            .expect("calendar entry");
        assert_eq!(completed.status.as_str(), "Concluida");

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_returns_other_results() {
        let base_dir = unique_test_dir("simulate_other_results");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        align_next_race_display_date(&db.conn, &season.id, "gt3", &next_race.display_date);

        let result = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        assert_eq!(result.player_race.track_name, next_race.track_name);
        assert!(
            result.other_categories.total_races_simulated >= 1,
            "at least one other category should simulate in the same week"
        );
        assert!(result
            .other_categories
            .categories_simulated
            .iter()
            .all(|category| category.category_id != "mazda_rookie"));
        assert!(
            result
                .other_categories
                .categories_simulated
                .iter()
                .any(|category| category.category_id == "gt3"),
            "gt3 should be included among the simulated same-week categories"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_processes_same_week_categories_even_on_different_days() {
        let base_dir = unique_test_dir("simulate_same_week_categories");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        // No modelo 9D, bmw_m2 e gt3 também iniciam na semana 6 (sexta e domingo)
        // enquanto toyota_rookie inicia na semana 7 (par de conflito deslocado).
        let bmw_race = get_next_race(&db.conn, &season.id, "bmw_m2")
            .expect("bmw race")
            .expect("pending bmw race");
        let gt3_race = get_next_race(&db.conn, &season.id, "gt3")
            .expect("gt3 race")
            .expect("pending gt3 race");
        assert_eq!(
            calendar_queries::calendar_entry_season_week(&bmw_race),
            calendar_queries::calendar_entry_season_week(&next_race),
            "bmw_m2 should share the same season_week as the player race"
        );
        assert_eq!(
            calendar_queries::calendar_entry_season_week(&gt3_race),
            calendar_queries::calendar_entry_season_week(&next_race),
            "gt3 should share the same season_week as the player race"
        );
        assert_ne!(
            bmw_race.display_date, next_race.display_date,
            "bmw_m2 should stay on a different day within the same week"
        );
        assert_ne!(
            gt3_race.display_date, next_race.display_date,
            "gt3 should stay on a different day within the same week"
        );
        drop(db);

        let result = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        let simulated_categories = result
            .other_categories
            .categories_simulated
            .iter()
            .map(|category| category.category_id.as_str())
            .collect::<Vec<_>>();

        assert!(
            simulated_categories.contains(&"bmw_m2"),
            "bmw_m2 should simulate in the same week even on a different day"
        );
        assert!(
            simulated_categories.contains(&"gt3"),
            "gt3 should simulate in the same week even on a different day"
        );

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let bmw_after = calendar_queries::get_calendar_entry_by_id(&updated_db.conn, &bmw_race.id)
            .expect("bmw by id")
            .expect("bmw entry");
        let gt3_after = calendar_queries::get_calendar_entry_by_id(&updated_db.conn, &gt3_race.id)
            .expect("gt3 by id")
            .expect("gt3 entry");

        assert_eq!(bmw_after.status.as_str(), "Concluida");
        assert_eq!(gt3_after.status.as_str(), "Concluida");

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_all_categories_complete_after_last_race() {
        let base_dir = unique_test_dir("simulate_all_categories");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");

        loop {
            let db = Database::open_existing(&db_path).expect("db");
            let season = season_queries::get_active_season(&db.conn)
                .expect("season")
                .expect("active season");
            let Some(next_race) =
                get_next_race(&db.conn, &season.id, "mazda_rookie").expect("next race")
            else {
                break;
            };
            drop(db);

            simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
                .expect("simulate round");
        }

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let season = season_queries::get_active_season(&updated_db.conn)
            .expect("season")
            .expect("active season");
        let pending =
            calendar_queries::get_pending_races(&updated_db.conn, &season.id).expect("pending");

        assert!(pending.is_empty(), "all categories should be complete");

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_last_player_race_drains_pending_races_without_showing_off_day_results() {
        let base_dir = unique_test_dir("simulate_last_race_hidden_drain");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let player_schedule = calendar_queries::get_calendar(&db.conn, &season.id, "mazda_rookie")
            .expect("player calendar");
        let last_player_race = player_schedule.last().expect("last player race").clone();

        for entry in player_schedule
            .iter()
            .filter(|entry| entry.id != last_player_race.id)
        {
            calendar_queries::mark_race_completed(&db.conn, &entry.id)
                .expect("complete previous player race");
        }

        db.conn
            .execute(
                "UPDATE calendar
                 SET data = '2024-02-01'
                 WHERE categoria != 'mazda_rookie'
                   AND status = 'Pendente'",
                [],
            )
            .expect("move other categories away from player race day");

        let pending_before = calendar_queries::get_pending_races(&db.conn, &season.id)
            .expect("pending before")
            .into_iter()
            .filter(|entry| entry.categoria != "mazda_rookie")
            .count();
        assert!(
            pending_before > 0,
            "test setup should leave other categories pending"
        );
        drop(db);

        let result =
            simulate_race_weekend_in_base_dir(&base_dir, "career_001", &last_player_race.id)
                .expect("simulate last player race");

        assert_eq!(result.other_categories.total_races_simulated, 0);
        assert!(result.other_categories.categories_simulated.is_empty());
        assert!(result.other_categories.highlights.is_empty());

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let pending_after =
            calendar_queries::get_pending_races(&updated_db.conn, &season.id).expect("pending");
        assert!(
            pending_after.is_empty(),
            "hidden drain should still complete the season calendar"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_stats_updated_for_other_categories() {
        let base_dir = unique_test_dir("simulate_other_stats");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        align_next_race_display_date(&db.conn, &season.id, "gt3", &next_race.display_date);
        drop(db);

        simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let gt3_driver = driver_queries::get_drivers_by_category(&updated_db.conn, "gt3")
            .expect("gt3 drivers")
            .into_iter()
            .next()
            .expect("at least one gt3 driver");

        assert!(
            gt3_driver.stats_temporada.corridas > 0,
            "other categories should update driver stats"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_race_history_saved_for_other_categories() {
        let base_dir = unique_test_dir("simulate_other_history");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let career_dir = config.saves_dir().join("career_001");
        let db_path = career_dir.join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        align_next_race_display_date(&db.conn, &season.id, "gt3", &next_race.display_date);
        drop(db);

        simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        let history_path = career_dir.join("race_results.json");
        let history =
            fs::read_to_string(history_path).expect("race history should be written to disk");

        assert!(history.contains("\"mazda_rookie\""));
        assert!(history.contains("\"gt3\""));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_special_convocation_does_not_place_player_in_production_endurance_grid() {
        let base_dir = unique_test_dir("simulate_special_player");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

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

        let special_contract =
            crate::db::queries::contracts::get_active_especial_contract_for_pilot(
                &db.conn, &player.id,
            )
            .expect("special contract query");
        let refreshed_player = driver_queries::get_player_driver(&db.conn).expect("player");

        assert!(special_contract.is_none());
        assert!(refreshed_player.categoria_especial_ativa.is_none());

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_special_block_fast_forwards_when_player_stays_out() {
        let base_dir = unique_test_dir("simulate_special_block_skip");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

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
        crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");
        drop(db);

        let result =
            simulate_special_block_in_base_dir(&base_dir, "career_001").expect("fast sim special");

        assert_eq!(result.total_races_simulated, 16);

        let refreshed_db = Database::open_existing(&db_path).expect("refreshed db");
        let season = season_queries::get_active_season(&refreshed_db.conn)
            .expect("season")
            .expect("active season");
        let pending_specials = calendar_queries::get_pending_races_for_category(
            &refreshed_db.conn,
            &season.id,
            "production_challenger",
        )
        .expect("production pending")
        .len()
            + calendar_queries::get_pending_races_for_category(
                &refreshed_db.conn,
                &season.id,
                "endurance",
            )
            .expect("endurance pending")
            .len();
        assert_eq!(
            pending_specials, 0,
            "nao deveria restar corrida especial pendente"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_special_block_rejects_player_inside_special_grid() {
        let base_dir = unique_test_dir("simulate_special_block_player_inside");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

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
        crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");
        drop(db);

        simulate_special_block_in_base_dir(&base_dir, "career_001")
            .expect("player outside real regular special grid can fast sim");

        let _ = fs::remove_dir_all(base_dir);
    }

    fn unique_test_dir(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("iracerapp_{label}_{nanos}"))
    }

    fn mark_regular_races_completed(db: &Database) {
        db.conn
            .execute(
                "UPDATE calendar SET status = 'Concluida' WHERE season_phase = 'BlocoRegular'",
                [],
            )
            .expect("complete regular block");
    }

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

    fn align_next_race_display_date(
        conn: &rusqlite::Connection,
        season_id: &str,
        category_id: &str,
        display_date: &str,
    ) {
        let race = get_next_race(conn, season_id, category_id)
            .expect("next race for category")
            .expect("pending race for category");
        conn.execute(
            "UPDATE calendar SET data = ?1 WHERE id = ?2",
            rusqlite::params![display_date, race.id],
        )
        .expect("align next race display date");
    }
