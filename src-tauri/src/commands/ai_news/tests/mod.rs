    use super::*;
    use crate::race_eval::Assessment;
    use serde_json::json;
    use serial_test::serial;

    /// Os fatos saem no locale ativo; estes testes conferem a prosa PT, então fixam o
    /// idioma antes de rodar. `#[serial]` porque o locale é estado global do processo.
    fn pt() {
        rust_i18n::set_locale("pt-BR");
    }

    fn sig() -> PostRaceSignals {
        // Base neutra: terminou dentro do esperado, sem drama.
        PostRaceSignals {
            is_dnf: false,
            dnf_mechanical: false,
            grid: 6,
            finish: 6,
            positions_gained: 0,
            has_fastest_lap: false,
            assessment: Some(Assessment::Dentro),
            target_low: 5,
            target_high: 7,
            duel: None,
            track_name: "Interlagos".to_string(),
        }
    }

    fn thesis_of(s: &PostRaceSignals) -> String {
        select_post_race_thesis(s).0
    }

    #[test]
    #[serial]
    fn dnf_mecanico_vence_tudo_e_isenta_o_piloto() {
        pt();
        let mut s = sig();
        s.is_dnf = true;
        s.dnf_mechanical = true;
        s.assessment = Some(Assessment::MuitoAbaixo);
        let (stmt, support) = select_post_race_thesis(&s);
        assert!(stmt.contains("DRAMA MECÂNICO"));
        assert!(stmt.contains("não foi erro"));
        assert!(support.contains(&"breakdowns"));
    }

    #[test]
    #[serial]
    fn dnf_por_incidente_e_fim_precoce() {
        pt();
        let mut s = sig();
        s.is_dnf = true;
        s.dnf_mechanical = false;
        assert!(thesis_of(&s).contains("FIM PRECOCE"));
    }

    #[test]
    #[serial]
    fn vitoria_e_a_manchete() {
        pt();
        let mut s = sig();
        s.finish = 1;
        s.positions_gained = 5;
        s.has_fastest_lap = true;
        let stmt = thesis_of(&s);
        assert!(stmt.contains("VITÓRIA"));
        assert!(stmt.contains("volta mais rápida"));
    }

    #[test]
    #[serial]
    fn remontada_quando_ganha_muitas_posicoes() {
        pt();
        let mut s = sig();
        s.grid = 12;
        s.finish = 4;
        s.positions_gained = 8;
        s.assessment = Some(Assessment::Acima);
        assert!(thesis_of(&s).contains("RECUPERAÇÃO"));
    }

    #[test]
    #[serial]
    fn colapso_quando_perde_muitas_posicoes() {
        pt();
        let mut s = sig();
        s.grid = 3;
        s.finish = 11;
        s.positions_gained = -8;
        s.assessment = Some(Assessment::Abaixo);
        assert!(thesis_of(&s).contains("ESCAPOU"));
    }

    #[test]
    #[serial]
    fn acima_e_abaixo_do_esperado_sem_drama() {
        pt();
        let mut over = sig();
        over.finish = 3;
        over.assessment = Some(Assessment::Acima);
        assert!(thesis_of(&over).contains("ACIMA DO ESPERADO"));

        let mut under = sig();
        under.finish = 9;
        under.assessment = Some(Assessment::Abaixo);
        assert!(thesis_of(&under).contains("AQUÉM"));
    }

    #[test]
    #[serial]
    fn duelo_decide_um_dia_morno() {
        pt();
        let mut s = sig(); // assessment Dentro, nada extremo
        s.duel = Some(PostRaceDuel {
            name: "K. Novak".to_string(),
            player_won: true,
            is_nemesis: true,
            h2h: Some((3, 2)),
        });
        let stmt = thesis_of(&s);
        assert!(stmt.contains("O DUELO"));
        assert!(stmt.contains("K. Novak"));
        assert!(stmt.contains("nemesis"));
        assert!(stmt.contains("3-2"));
    }

    #[test]
    #[serial]
    fn dia_de_somar_quando_nada_se_destaca() {
        pt();
        assert!(thesis_of(&sig()).contains("DIA DE SOMAR"));
    }

    #[test]
    #[serial]
    fn telemetry_facts_resume_ritmo_ultrapassagens_e_erro() {
        pt();
        let tel = json!({
            "has_telemetry": true,
            "pace": { "vs_grid_ms": -506.0, "vs_grid_reliable": true, "good_laps": 8 },
            "position_flow": { "gained_on_track": 4, "lost_on_track": 1 },
            "best_moment": { "lap": 8, "positions_gained": 3 },
            "mistake": { "lap": 9, "positions_lost": 1, "time_lost_ms": 600.0 },
            "charts": {
                "rival_name": "Massimo Caruso",
                "rival_gap": [ { "lap": 13.0, "gap_s": 0.8 } ],
                "lap_times": [
                    { "lap": 6.0, "time_s": 71.0 },
                    { "lap": 7.0, "time_s": 71.3 },
                    { "lap": 8.0, "time_s": 71.6 },
                    { "lap": 9.0, "time_s": 71.9 }
                ],
                "cars": [
                    { "idx": 0, "is_player": true, "name": "Você", "points": [
                        { "lap": 6.0, "position": 7 },
                        { "lap": 7.4, "position": 4 },
                        { "lap": 9.0, "position": 6 }
                    ] },
                    { "idx": 1, "is_player": false, "name": "Bruno Perez", "points": [
                        { "lap": 6.0, "position": 4 },
                        { "lap": 7.4, "position": 7 },
                        { "lap": 9.0, "position": 4 }
                    ] }
                ]
            }
        });
        let out = telemetry_facts(Some(&tel), 8);
        assert!(out.contains("MAIS RÁPIDO"), "ritmo vs grid: {out}");
        assert!(out.contains("Degradação"), "degradação: {out}");
        assert!(
            out.contains("volta 7.4: passou Bruno Perez"),
            "feed de ultrapassagem: {out}"
        );
        assert!(out.contains("Largada: P8 → P7"), "largada: {out}");
        assert!(out.contains("Erro mais caro: volta 9"), "erro: {out}");
        assert!(
            out.contains("Massimo Caruso terminou à sua frente"),
            "duelo direto: {out}"
        );
    }

    #[test]
    fn telemetry_facts_vazio_sem_telemetria() {
        assert!(telemetry_facts(None, 5).is_empty());
        assert!(telemetry_facts(Some(&json!({ "has_telemetry": false })), 5).is_empty());
    }
