    use super::formato::{
        best_positive_lap, history_matches_subsession, name_key, roster_with_telemetry,
        tower_order_key,
    };
    use crate::iracing_sdk::{race_monitor::YamlCarMeta, CarSnapshot};

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
    fn tower_order_key_puts_classified_cars_first() {
        let classified = tower_order_key(12, &(i64::MAX, 99));
        let unclassified = tower_order_key(0, &(80_000, 1));

        assert!(classified < unclassified);
    }

    #[test]
    fn tower_order_key_sorts_classified_cars_by_official_position() {
        let first = tower_order_key(1, &(i64::MAX, 99));
        let second = tower_order_key(2, &(70_000, 1));

        assert!(first < second);
    }

    #[test]
    fn tower_order_key_sorts_unclassified_cars_by_best_qualifying_lap() {
        let faster = tower_order_key(0, &(79_999, 99));
        let slower = tower_order_key(0, &(80_000, 1));

        assert!(faster < slower);
    }

    #[test]
    fn tower_order_key_uses_car_number_as_tiebreaker_and_fallback() {
        let lower_number_with_same_lap = tower_order_key(0, &(80_000, 7));
        let higher_number_with_same_lap = tower_order_key(0, &(80_000, 12));
        let lower_number_without_lap = tower_order_key(0, &(i64::MAX, 7));
        let higher_number_without_lap = tower_order_key(0, &(i64::MAX, 12));

        assert!(lower_number_with_same_lap < higher_number_with_same_lap);
        assert!(lower_number_without_lap < higher_number_without_lap);
    }
