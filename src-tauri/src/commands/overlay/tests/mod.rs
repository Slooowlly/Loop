    use super::formato::{
        best_positive_lap, history_matches_subsession, name_key, roster_with_telemetry,
    };
    use super::ordem::{
        modo_da_sessao, ordem_pre_sessao, ordenar, OrderInput, OrderMode, PreSinal,
    };
    use crate::iracing_sdk::{race_monitor::YamlCarMeta, CarSnapshot};

    /// Carro de teste: sem posição oficial, sem volta, sem grid — cada caso liga só
    /// o campo que está exercitando.
    fn carro(car_number: i64) -> OrderInput {
        OrderInput {
            class_position: 0,
            lap_completed: -1,
            lap_dist_pct: -1.0,
            best_lap_ms: i64::MAX,
            qualy_best_ms: i64::MAX,
            pre_ordem: i64::MAX,
            car_number,
        }
    }

    /// Piloto da carreira já com temporada em andamento.
    fn no_campeonato(pontos: i32, vitorias: i32) -> PreSinal {
        PreSinal {
            corridas_temporada: 3,
            pontos,
            vitorias,
            podios: vitorias,
            expectativa: 0.0,
            conhecido: true,
        }
    }

    /// Piloto antes da 1ª corrida: só a expectativa de pré-temporada existe.
    fn na_pre_temporada(expectativa: f64) -> PreSinal {
        PreSinal {
            corridas_temporada: 0,
            pontos: 0,
            vitorias: 0,
            podios: 0,
            expectativa,
            conhecido: true,
        }
    }

    #[test]
    fn best_positive_lap_uses_recorded_when_live_is_absent_or_zero() {
        assert_eq!(best_positive_lap(None, Some(82.4)), 82.4);
        assert_eq!(best_positive_lap(Some(0.0), Some(82.4)), 82.4);
    }

    #[test]
    fn best_positive_lap_uses_live_when_recorded_is_absent_or_zero() {
        assert_eq!(best_positive_lap(Some(81.7), None), 81.7);
        assert_eq!(best_positive_lap(Some(81.7), Some(0.0)), 81.7);
    }

    #[test]
    fn best_positive_lap_chooses_the_lower_positive_time() {
        assert_eq!(best_positive_lap(Some(81.7), Some(82.4)), 81.7);
        assert_eq!(best_positive_lap(Some(82.4), Some(81.7)), 81.7);
    }

    #[test]
    fn best_positive_lap_returns_zero_without_a_positive_time() {
        assert_eq!(best_positive_lap(None, None), 0.0);
        assert_eq!(best_positive_lap(Some(0.0), Some(0.0)), 0.0);
    }

    #[test]
    fn history_match_exige_ids_iguais_online_e_offline() {
        // Online: ids iguais e positivos casam; diferentes não.
        assert!(history_matches_subsession(4242, 4242));
        assert!(!history_matches_subsession(4242, 4243));
        // Transição de evento online (histórico ainda não resetado) NÃO casa.
        assert!(!history_matches_subsession(0, 4242));
        assert!(!history_matches_subsession(4242, 0));
        // OFFLINE (aiseason de IA): SubSessionID = 0 nos dois lados → CASA. Sem isto o
        // overlay descartava grid/delta e os ícones de pneu na corrida de IA.
        assert!(history_matches_subsession(0, 0));
    }

    #[test]
    fn roster_with_telemetry_keeps_all_non_pace_cars_and_joins_available_snapshot() {
        let roster = vec![
            YamlCarMeta {
                idx: 1,
                is_ai: true,
                is_pace: false,
                class_id: 10,
                car_number: 11,
            },
            YamlCarMeta {
                idx: 2,
                is_ai: true,
                is_pace: false,
                class_id: 10,
                car_number: 22,
            },
            YamlCarMeta {
                idx: 3,
                is_ai: false,
                is_pace: true,
                class_id: 10,
                car_number: 0,
            },
        ];
        let telemetry = vec![CarSnapshot {
            idx: 2,
            class_position: 1,
            ..CarSnapshot::default()
        }];

        let joined = roster_with_telemetry(&roster, &telemetry);
        let joined_indices: Vec<(i32, Option<i32>)> = joined
            .into_iter()
            .map(|(meta, car)| (meta.idx, car.map(|snapshot| snapshot.idx)))
            .collect();

        assert_eq!(joined_indices, vec![(1, None), (2, Some(2))]);
    }

    #[test]
    fn name_key_matches_sdk_username_against_roster_name() {
        // O `UserName` do SDK volta com caixa/espaços próprios; o join por nome não
        // pode depender disso.
        assert_eq!(name_key("  Ana Ribeiro "), name_key("ana ribeiro"));
        assert_ne!(name_key("Ana Ribeiro"), name_key("Ana Ribeira"));
    }

    #[test]
    fn modo_segue_o_tipo_e_o_estado_da_sessao() {
        // Treino/quali sempre por tempo, em qualquer estado.
        assert_eq!(modo_da_sessao("Q", 4), OrderMode::Tempo);
        assert_eq!(modo_da_sessao("P", 1), OrderMode::Tempo);
        // Corrida: antes do verde é grid; rolando é progresso; na bandeirada é oficial.
        assert_eq!(modo_da_sessao("R", 1), OrderMode::Grid); // GetInCar
        assert_eq!(modo_da_sessao("R", 3), OrderMode::Grid); // ParadeLaps
        assert_eq!(modo_da_sessao("R", 4), OrderMode::Progresso);
        assert_eq!(modo_da_sessao("R", 5), OrderMode::Oficial); // Checkered
        assert_eq!(modo_da_sessao("R", 6), OrderMode::Oficial); // CoolDown
    }

    #[test]
    fn quali_ordena_pelo_melhor_tempo_e_quem_nao_marcou_fica_no_fim() {
        let cars = vec![
            OrderInput { best_lap_ms: 80_500, ..carro(7) },
            OrderInput { best_lap_ms: i64::MAX, ..carro(3) },
            OrderInput { best_lap_ms: 79_900, ..carro(12) },
        ];

        assert_eq!(ordenar(OrderMode::Tempo, &cars), vec![2, 0, 1]);
    }

    #[test]
    fn corrida_rolando_ordena_por_progresso_real_e_nao_pela_posicao_oficial() {
        // O #7 já ultrapassou o #12 no meio da volta, mas o SDK ainda diz P1 pro #12
        // (a posição oficial só vira na linha). A torre tem de mostrar a ultrapassagem.
        let cars = vec![
            OrderInput { class_position: 2, lap_completed: 3, lap_dist_pct: 0.62, ..carro(7) },
            OrderInput { class_position: 1, lap_completed: 3, lap_dist_pct: 0.58, ..carro(12) },
        ];

        assert_eq!(ordenar(OrderMode::Progresso, &cars), vec![0, 1]);
    }

    #[test]
    fn progresso_conta_a_volta_inteira_e_manda_lapeado_e_ausente_pro_fim() {
        let cars = vec![
            // Lapeado: uma volta atrás, mesmo estando adiante na fração da volta.
            OrderInput { lap_completed: 2, lap_dist_pct: 0.95, ..carro(5) },
            // Líder: acabou de cruzar a linha.
            OrderInput { lap_completed: 3, lap_dist_pct: 0.02, ..carro(1) },
            // Fora do mundo (abandonou / nunca apareceu na telemetria).
            carro(9),
        ];

        assert_eq!(ordenar(OrderMode::Progresso, &cars), vec![1, 0, 2]);
    }

    #[test]
    fn grid_ordena_pela_melhor_volta_da_quali() {
        let cars = vec![
            OrderInput { qualy_best_ms: 80_400, ..carro(7) },
            OrderInput { qualy_best_ms: 79_800, ..carro(12) },
            OrderInput { qualy_best_ms: i64::MAX, ..carro(3) },
        ];

        assert_eq!(ordenar(OrderMode::Grid, &cars), vec![1, 0, 2]);
    }

    #[test]
    fn grid_sem_nenhum_tempo_de_quali_cai_na_posicao_oficial() {
        // Corrida sem quali (grade fixa): sem tempo pra ordenar, a posição do SDK é o
        // que existe de melhor — cair no número do carro embaralharia a grade.
        let cars = vec![
            OrderInput { class_position: 3, ..carro(7) },
            OrderInput { class_position: 1, ..carro(12) },
            OrderInput { class_position: 2, ..carro(3) },
        ];

        assert_eq!(ordenar(OrderMode::Grid, &cars), vec![1, 2, 0]);
    }

    #[test]
    fn antes_da_primeira_corrida_a_previa_e_a_expectativa_de_pre_temporada() {
        // Ninguém correu ainda: o campeonato está zerado pra todo mundo e a única
        // hierarquia é a percepção pública da matéria de pré-temporada.
        let sinais = vec![
            na_pre_temporada(120.0),
            na_pre_temporada(480.0),
            na_pre_temporada(300.0),
        ];

        assert_eq!(ordem_pre_sessao(&sinais), vec![3, 1, 2]);
    }

    #[test]
    fn depois_da_primeira_corrida_a_previa_e_a_posicao_no_campeonato() {
        // Com temporada rolando a tabela manda — mesmo que a expectativa dissesse
        // outra coisa (o #0 era o mais cotado e está sem ponto).
        let sinais = vec![
            PreSinal { expectativa: 999.0, ..no_campeonato(0, 0) },
            no_campeonato(50, 2),
            no_campeonato(50, 1),
        ];

        assert_eq!(ordem_pre_sessao(&sinais), vec![3, 1, 2]);
    }

    #[test]
    fn piloto_sem_dono_na_carreira_nao_entra_na_previa() {
        let sinais = vec![PreSinal::default(), na_pre_temporada(10.0)];

        assert_eq!(ordem_pre_sessao(&sinais), vec![i64::MAX, 1]);
    }

    #[test]
    fn quali_sem_nenhum_tempo_usa_a_previa_no_lugar_do_numero_do_carro() {
        // O #3 tem o número mais baixo, mas é o último da prévia: sem isto a torre
        // abria a classificatória numa fila por número de carro.
        let cars = vec![
            OrderInput { pre_ordem: 2, ..carro(7) },
            OrderInput { pre_ordem: 3, ..carro(3) },
            OrderInput { pre_ordem: 1, ..carro(12) },
        ];

        assert_eq!(ordenar(OrderMode::Tempo, &cars), vec![2, 0, 1]);
    }

    #[test]
    fn a_previa_nao_atropela_quem_ja_marcou_tempo() {
        // Marcou volta? O tempo decide. A prévia só desempata quem ainda não marcou —
        // e quem marcou fica sempre à frente de quem não marcou.
        let cars = vec![
            OrderInput { best_lap_ms: 80_500, pre_ordem: 9, ..carro(7) },
            OrderInput { pre_ordem: 1, ..carro(1) },
            OrderInput { best_lap_ms: 79_900, pre_ordem: 10, ..carro(12) },
        ];

        assert_eq!(ordenar(OrderMode::Tempo, &cars), vec![2, 0, 1]);
    }

    #[test]
    fn oficial_classifica_primeiro_e_desempata_por_numero() {
        let cars = vec![
            carro(12),
            OrderInput { class_position: 2, ..carro(9) },
            OrderInput { class_position: 1, ..carro(4) },
            carro(3),
        ];

        assert_eq!(ordenar(OrderMode::Oficial, &cars), vec![2, 1, 3, 0]);
    }

    // ── Medição do custo de `get_overlay_data` (ignorada por padrão) ─────────
    // Roda contra um save REAL. Não é asserção: imprime os tempos para decidir
    // onde otimizar o caminho de 500 ms do overlay.
    //   LOOP_BENCH_DB=<career.db> LOOP_BENCH_YAML=<session.yaml> \
    //     cargo test --manifest-path src-tauri/Cargo.toml bench_overlay -- --ignored --nocapture
    #[test]
    #[ignore]
    fn bench_overlay_custo() {
        use std::time::Instant;

        let db_path = std::env::var("LOOP_BENCH_DB").expect("LOOP_BENCH_DB");
        let yaml_path = std::env::var("LOOP_BENCH_YAML").expect("LOOP_BENCH_YAML");
        let yaml = std::fs::read_to_string(&yaml_path).expect("yaml");
        let db_path = std::path::PathBuf::from(db_path);

        let media = |rot: u32, t: std::time::Duration| t.as_secs_f64() * 1000.0 / rot as f64;

        // 1) Abrir o banco (inclui `migrations::run_pending`).
        let rot = 20;
        let t0 = Instant::now();
        for _ in 0..rot {
            let _db = crate::db::connection::Database::open_existing(&db_path).expect("abrir");
        }
        println!("abrir_banco: {:.2} ms/chamada", media(rot, t0.elapsed()));

        let db = crate::db::connection::Database::open_existing(&db_path).expect("abrir");

        // 2) Piloto do jogador + categoria (contrato ativo → equipe).
        let t0 = Instant::now();
        for _ in 0..rot {
            let p = crate::db::queries::drivers::get_player_driver(&db.conn).ok();
            let _cat = p
                .as_ref()
                .and_then(|p| {
                    crate::db::queries::contracts::get_active_contract_for_pilot(&db.conn, &p.id)
                        .ok()
                        .flatten()
                })
                .and_then(|c| {
                    crate::db::queries::teams::get_team_by_id(&db.conn, &c.equipe_id)
                        .ok()
                        .flatten()
                });
        }
        println!("jogador+categoria: {:.2} ms/chamada", media(rot, t0.elapsed()));

        // 3) Papéis de rivalidade.
        let t0 = Instant::now();
        for _ in 0..rot {
            let current =
                crate::db::queries::player_nemesis::get_current_nemesis(&db.conn).unwrap_or(None);
            let _ = crate::commands::career::select_player_interests(&db.conn, current.as_deref());
        }
        println!("rivalidade: {:.2} ms/chamada", media(rot, t0.elapsed()));

        // 4) `resolve` por carro do grid: piloto + contrato + equipe + percepção.
        let ids: Vec<String> = crate::db::queries::drivers::get_all_drivers(&db.conn)
            .expect("elenco")
            .into_iter()
            .take(30)
            .map(|d| d.id)
            .collect();
        println!("(grid simulado: {} carros)", ids.len());
        let t0 = Instant::now();
        for _ in 0..rot {
            for id in &ids {
                let d = crate::db::queries::drivers::get_driver(&db.conn, id).expect("piloto");
                let _team =
                    crate::db::queries::contracts::get_active_contract_for_pilot(&db.conn, id)
                        .ok()
                        .flatten()
                        .and_then(|c| {
                            crate::db::queries::teams::get_team_by_id(&db.conn, &c.equipe_id)
                                .ok()
                                .flatten()
                        });
                let _ = crate::commands::season_preview::perception_score(&d);
            }
        }
        println!("resolve(grid inteiro): {:.2} ms/chamada", media(rot, t0.elapsed()));

        // 5) Parse do YAML de sessão (28 KB reais).
        let rot_yaml = 100;
        let t0 = Instant::now();
        for _ in 0..rot_yaml {
            let _ = super::formato::parse_session_types(&yaml);
        }
        println!("parse_session_types: {:.2} ms/chamada", media(rot_yaml, t0.elapsed()));
    }
