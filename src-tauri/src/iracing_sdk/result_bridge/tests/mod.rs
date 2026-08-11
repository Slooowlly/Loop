use std::collections::HashMap;

use rusqlite::Connection;

use crate::iracing_sdk::race_monitor::{
    CarGapPoint, CarLap, CarMeta, LapSnapshot, RaceEvent, RaceHistory, RaceStatus,
};
use crate::simulation::race::RaceResult;

use super::{build_race_result_from_aiseason, build_race_result_from_session, player_worst_severity};

fn meta(idx: i32, car_number: i32, class_position: i32, class_id: i64) -> CarMeta {
    CarMeta {
        idx,
        is_ai: idx != 0,
        is_pace: false,
        class_id,
        class_position,
        car_number,
        grid_class_position: 0,
    }
}

fn history() -> RaceHistory {
    let mut h = RaceHistory {
        laps: vec![LapSnapshot {
            lap: 10,
            progress: 0.0,
            cars: vec![
                CarGapPoint {
                    idx: 0,
                    position: 2,
                    gap: 1.5,
                    ..Default::default()
                },
                CarGapPoint {
                    idx: 1,
                    position: 1,
                    gap: 0.0,
                    ..Default::default()
                },
                CarGapPoint {
                    idx: 2,
                    position: 3,
                    gap: 4.0,
                    ..Default::default()
                },
            ],
        }],
        player_laps: vec![],
        player_track: vec![],
        yellow_laps: vec![],
        player_car_idx: 0,
        attempt_number: 1,
        finished: true,
        outcome: "Finalizada".into(),
        car_laps: vec![],
        cars_meta: vec![
            meta(0, 10, 2, 100), // jogador, P2
            meta(1, 21, 1, 100), // IA vencedora
            meta(2, 33, 3, 100), // IA P3
        ],
        track_id: 1,
        subsession_id: 0,
        qualy_laps: vec![],
        qualy_best_valid: vec![],
        pit_stops: vec![],
        weather: Default::default(),
        player_sectors: vec![],
    };
    // Tempos de volta: idx1 mais rápido (90s), jogador 91s, idx2 92s.
    for (idx, t) in [(0, 91.0), (1, 90.0), (2, 92.0)] {
        for lap in 1..=10 {
            h.car_laps.push(CarLap {
                car_idx: idx,
                lap,
                time: t,
            });
        }
    }
    // Quali: jogador foi pole (89s), idx1 90s, idx2 91s.
    for (idx, t) in [(0, 89.0), (1, 90.0), (2, 91.0)] {
        h.qualy_laps.push(CarLap {
            car_idx: idx,
            lap: 1,
            time: t,
        });
    }
    h
}

/// O grid gravado na carreira sai da classificatória, e a classificatória só conhece
/// volta que valeu. Aqui o idx2 marcou 85 s numa volta anulada por limite de pista: ela
/// está nas voltas cruas, mas o melhor VÁLIDO dele continua sendo 91 s. Sem a regra, ele
/// largaria em primeiro no registro da corrida.
#[test]
fn grid_ignora_a_volta_anulada_da_quali() {
    let mut h = history();
    h.qualy_laps.push(CarLap {
        car_idx: 2,
        lap: 2,
        time: 85.0,
    });
    h.qualy_best_valid = vec![(0, 89.0), (1, 90.0), (2, 91.0)];

    let grid = super::agregacao::grid_by_car(&h);

    assert_eq!(grid.get(&0), Some(&1), "o jogador fez a pole com 89 s");
    assert_eq!(grid.get(&1), Some(&2));
    assert_eq!(grid.get(&2), Some(&3), "a volta anulada não adianta ninguém");
}

/// Save gravado antes de o melhor válido existir: sem ele, o grid continua saindo das
/// voltas cruas em vez de sumir.
#[test]
fn grid_de_save_antigo_cai_nas_voltas_cruas() {
    let mut h = history();
    h.qualy_best_valid = vec![];

    let grid = super::agregacao::grid_by_car(&h);

    assert_eq!(grid.get(&0), Some(&1));
    assert_eq!(grid.get(&1), Some(&2));
    assert_eq!(grid.get(&2), Some(&3));
}

fn empty_status() -> RaceStatus {
    RaceStatus {
        connected: true,
        attempt_number: 1,
        event: None,
        session_state_label: String::new(),
        track_surface_label: String::new(),
        lap_completed: 10,
        incident_count: 0,
        crash_score: 0.0,
        crash_severity_now: String::new(),
        g_force: 0.0,
        speed_kmh: 0.0,
        tow_time: 0.0,
        cars_count: 3,
        crash_in_progress: false,
        crash_progress_score: 0.0,
        crash_progress_severity: String::new(),
        is_green: true,
        cars_debug: vec![],
        attempts: vec![],
        events: vec![],
    }
}

#[test]
fn reconstructs_positions_grid_and_fastest() {
    let h = history();
    let s = empty_status();
    let by_number: HashMap<i64, String> =
        [(21, "ai-winner".to_string()), (33, "ai-third".to_string())]
            .into_iter()
            .collect();
    let conn = Connection::open_in_memory().unwrap();

    let r = build_race_result_from_session(&h, &s, &conn, &by_number, None, "Seco", "Test");

    // 3 carros classificados.
    assert_eq!(r.race_results.len(), 3);
    // Volta rápida é do idx1 (90s) → "ai-winner".
    assert_eq!(r.fastest_lap_id, "ai-winner");
    assert_eq!(r.winner_id, "ai-winner");
    // Grid pole foi do jogador (quali 89s), mas sem player_driver o id é placeholder.
    let player = r.race_results.iter().find(|x| x.is_jogador).unwrap();
    assert_eq!(player.grid_position, 1);
    assert_eq!(player.finish_position, 2);
    assert_eq!(player.positions_gained, -1);
    assert_eq!(r.total_laps, 10);
}

#[test]
fn ai_dnf_marks_driver_out() {
    let h = history();
    let mut s = empty_status();
    s.events.push(RaceEvent {
        session_time: 100.0,
        lap: 5,
        kind: "dnf_confirmed".into(),
        car_idx: Some(2),
        detail: "Acidente na curva 3".into(),
        severity: Some("grave".into()),
    });
    let by_number: HashMap<i64, String> =
        [(21, "ai-winner".to_string()), (33, "ai-third".to_string())]
            .into_iter()
            .collect();
    let conn = Connection::open_in_memory().unwrap();

    let r = build_race_result_from_session(&h, &s, &conn, &by_number, None, "Seco", "Test");
    let third = r
        .race_results
        .iter()
        .find(|x| x.pilot_id == "ai-third")
        .unwrap();
    assert!(third.is_dnf);
    assert_eq!(third.dnf_reason.as_deref(), Some("Acidente na curva 3"));
    assert_eq!(
        third.notable_incident.as_deref(),
        Some("Acidente na curva 3")
    );
    assert_eq!(r.total_dnfs, 1);
}

#[test]
fn aiseason_marks_laps_down_and_gap_and_pole() {
    use crate::iracing_sdk::aiseason_results::{AiEventResult, AiResultRow};
    let row = |laps: i32, num: i32, interval: f64| AiResultRow {
        laps_complete: laps,
        car_number: num,
        reason_out: "Running".into(),
        best_lap_time_ms: 80000.0,
        interval_ms: interval,
        grid_position: num, // só pra ter pole no #1
        ..Default::default()
    };
    let event = AiEventResult {
        track_id: 1,
        laps_complete: 11,
        // P1 líder; DOIS carros parados juntos lá atrás (3/11 — colisão); um lapeado.
        rows: vec![
            row(11, 1, 0.0),
            row(3, 6, 0.0),
            row(3, 7, 0.0),
            row(10, 2, 25000.0),
        ],
        qualify: vec![AiResultRow {
            car_number: 1,
            cust_id: 0,
            best_lap_time_ms: 84000.0,
            grid_position: 1,
            ..Default::default()
        }],
    };
    let conn = Connection::open_in_memory().unwrap();
    let empty = std::collections::HashSet::new();
    let r = build_race_result_from_aiseason(
        &event,
        &conn,
        &HashMap::new(),
        0,
        None,
        false,
        None,
        &empty,
        "Seco",
        "T",
        &[],
        "nenhum",
        "",
    );
    let leader = r
        .race_results
        .iter()
        .find(|x| x.laps_completed == 11)
        .unwrap();
    let lapped = r
        .race_results
        .iter()
        .find(|x| x.laps_completed == 10)
        .unwrap();
    let back: Vec<_> = r
        .race_results
        .iter()
        .filter(|x| x.laps_completed == 3)
        .collect();
    assert!(!leader.is_dnf, "líder não é DNF");
    assert!(!lapped.is_dnf, "lapeado (10/11) terminou");
    assert_eq!(back.len(), 2);
    assert!(back.iter().all(|x| x.is_dnf), "3/11 voltas = DNF");
    // Gap do carro lapeado vem do interval.
    assert!((lapped.gap_to_winner_ms - 25000.0).abs() < 1.0);
    // Tempo da pole (#1) veio da quali.
    let pole = r.qualifying_results.iter().find(|q| q.is_pole).unwrap();
    assert!((pole.best_lap_time_ms - 84000.0).abs() < 1.0);
    // Os dois parados juntos lá atrás → inferido como COLISÃO entre si.
    for car in &back {
        let inc = car.incidents.first().expect("incidente inferido");
        assert!(inc.is_two_car_incident, "colisão entre os dois");
        assert!(inc.linked_pilot_id.is_some());
    }
}

/// O caso que faltava: o jogador BATEU mas TERMINOU a corrida. Toda a inferência
/// de incidente é gated por `is_dnf`, então sem os marcadores do monitor ele ficava
/// sem incidente nenhum — e a revista não tinha o que citar.
#[test]
#[serial_test::serial] // afere prosa PT → fixa o locale, que é global do processo
fn aiseason_registra_batida_do_jogador_que_terminou() {
    use crate::iracing_sdk::aiseason_results::{AiEventResult, AiResultRow};
    use crate::iracing_sdk::race_monitor::PlayerIncidentMark;
    use crate::simulation::incidents::{IncidentSeverity, IncidentType};

    let row = |laps: i32, num: i32, cust: i64| AiResultRow {
        laps_complete: laps,
        car_number: num,
        cust_id: cust,
        reason_out: "Running".into(),
        best_lap_time_ms: 80000.0,
        ..Default::default()
    };
    let event = AiEventResult {
        track_id: 1,
        laps_complete: 11,
        rows: vec![row(11, 1, 100), row(11, 64, 99)],
        qualify: vec![],
    };
    let empty = std::collections::HashSet::new();
    let mark = |points: i32, lap_f: f64| PlayerIncidentMark {
        lap_f,
        points,
        off_track: false,
    };
    let build_dir = |marks: &[PlayerIncidentMark], crash: &str, dir: &str| {
        let conn = Connection::open_in_memory().unwrap();
        build_race_result_from_aiseason(
            &event,
            &conn,
            &HashMap::new(),
            99,
            None,
            false,
            None,
            &empty,
            "Seco",
            "T",
            marks,
            crash,
            dir,
        )
    };
    let build = |marks: &[PlayerIncidentMark], crash: &str| build_dir(marks, crash, "");
    let player_inc = |r: &RaceResult| {
        r.race_results
            .iter()
            .find(|x| x.is_jogador)
            .unwrap()
            .incidents
            .first()
            .cloned()
    };

    // Contato (4 pts) sem impacto medido → batida de verdade, com o outro carro.
    let r = build(&[mark(1, 2.0), mark(4, 7.5)], "nenhum");
    assert!(!r.race_results.iter().find(|x| x.is_jogador).unwrap().is_dnf);
    let inc = player_inc(&r).expect("batida registrada");
    assert_eq!(inc.incident_type, IncidentType::Collision);
    assert_eq!(inc.severity, IncidentSeverity::Major);
    assert!(
        inc.description.contains('7'),
        "cita a volta: {}",
        inc.description
    );

    // Rodada (2 pts) → o "algo pequeno", citado como pequeno.
    let r = build(&[mark(2, 4.2)], "nenhum");
    assert_eq!(player_inc(&r).unwrap().severity, IncidentSeverity::Minor);

    // Só saída de pista (<= 1 pt) e nenhum impacto → ruído, não vira nota.
    let r = build(&[mark(1, 3.0)], "nenhum");
    assert!(player_inc(&r).is_none());

    // O PONTO-CHAVE: para o iRacing "contato" é 4 pts tanto pro encostão quanto pra
    // pancada que destrói o carro. O score de impacto do monitor (o mesmo que cobra o
    // conserto) desempata — batida grave NÃO pode ser narrada como toque leve.
    let r = build(&[mark(4, 5.0)], "grave");
    assert_eq!(player_inc(&r).unwrap().severity, IncidentSeverity::Critical);
    let r = build(&[mark(4, 5.0)], "leve");
    assert_eq!(player_inc(&r).unwrap().severity, IncidentSeverity::Minor);

    // Impacto medido SEM marcador de incidente (bateu na parede sozinho): ainda
    // vira fato, só que sem volta.
    let r = build(&[], "moderado");
    let inc = player_inc(&r).expect("impacto sem marcador");
    assert_eq!(inc.severity, IncidentSeverity::Major);
    assert_eq!(inc.incident_type, IncidentType::DriverError);

    // Direção do impacto entra no fato: levar pancada na TRASEIRA é outra história
    // (foi atingido) do que bater de FRENTE (bateu em alguém).
    rust_i18n::set_locale("pt-BR");
    let r = build_dir(&[mark(4, 6.0)], "moderado", "rear");
    let desc = player_inc(&r).unwrap().description;
    assert!(desc.contains("traseira"), "direção no fato: {desc}");
    let r = build_dir(&[mark(4, 6.0)], "moderado", "front");
    assert!(player_inc(&r).unwrap().description.contains("dianteira"));
    // Sem direção medida não inventa lado nenhum.
    let r = build_dir(&[mark(4, 6.0)], "moderado", "");
    let desc = player_inc(&r).unwrap().description;
    assert!(
        !desc.contains("traseira") && !desc.contains("dianteira"),
        "{desc}"
    );
}

#[test]
fn aiseason_links_player_to_who_hit_them() {
    use crate::iracing_sdk::aiseason_results::{AiEventResult, AiResultRow};
    let row = |laps: i32, num: i32, cust: i64| AiResultRow {
        laps_complete: laps,
        car_number: num,
        cust_id: cust,
        reason_out: "Running".into(),
        best_lap_time_ms: 80000.0,
        ..Default::default()
    };
    let event = AiEventResult {
        track_id: 1,
        laps_complete: 11,
        rows: vec![
            row(11, 1, 100), // líder
            row(0, 64, 99),  // jogador parado (0 voltas) — DNF, cust 99
            row(11, 7, 700), // culpado, que TERMINOU (11 voltas)
        ],
        qualify: vec![],
    };
    let conn = Connection::open_in_memory().unwrap();
    let by_number: HashMap<i64, String> = [(7i64, "ai-7".to_string())].into_iter().collect();
    let empty = std::collections::HashSet::new();
    let r = build_race_result_from_aiseason(
        &event,
        &conn,
        &by_number,
        99,
        None,
        true,
        Some("ai-7"),
        &empty,
        "Seco",
        "T",
        &[],
        "nenhum",
        "",
    );
    let player = r.race_results.iter().find(|x| x.is_jogador).unwrap();
    assert!(player.is_dnf);
    let pi = player.incidents.first().expect("incidente do jogador");
    assert!(pi.is_two_car_incident);
    assert_eq!(pi.linked_pilot_id.as_deref(), Some("ai-7"));
    // O culpado ganha o incidente recíproco, MESMO tendo terminado a corrida.
    let culprit = r
        .race_results
        .iter()
        .find(|x| x.pilot_id == "ai-7")
        .unwrap();
    assert_eq!(
        culprit
            .incidents
            .first()
            .and_then(|i| i.linked_pilot_id.clone()),
        Some(player.pilot_id.clone())
    );
}

// ── Conserto do carro: o que conta como batida ───────────────────────────────
use crate::iracing_sdk::race_monitor::{Attempt, AttemptEvidence, CrashEvent};

fn tentativa_com_reparo(
    peak: f64,
    checkered: bool,
    crashes: Vec<CrashEvent>,
    reparo_s: f64,
) -> Attempt {
    Attempt {
        sim_repair_needed_s: reparo_s,
        ..tentativa(peak, checkered, crashes)
    }
}

fn tentativa(peak: f64, checkered: bool, crashes: Vec<CrashEvent>) -> Attempt {
    Attempt {
        number: 1,
        status: "active".to_string(),
        started_at_session_time: 0.0,
        laps_completed: 10,
        ended_by: None,
        reason: None,
        worst_crash: None,
        evidence: AttemptEvidence {
            raced: true,
            reached_checkered: checkered,
            ..Default::default()
        },
        crashes,
        peak_crash_score: peak,
        collided_with_car_number: None,
        peak_impact_dir: None,
        sim_repair_needed_s: 0.0,
        sim_repair_required_s: 0.0,
        style: crate::car::driving_style::StyleAccumulator::new(),
    }
}

fn batida(score: f64, severity: &str, had_impact: bool) -> CrashEvent {
    CrashEvent {
        session_time: 100.0,
        lap: 4,
        score,
        severity: severity.to_string(),
        impact_severity: severity.to_string(),
        had_impact,
        factors: vec![],
    }
}

fn severidade(attempt: Attempt) -> String {
    let mut status = empty_status();
    status.attempts = vec![attempt];
    player_worst_severity(&status, 1)
}

/// Uma RODADA limpa pontua alto no scorer (incidente + guinada + rotação + fora da
/// pista) sem o carro ter tocado em nada. Sem o gate de impacto, essa corrida chegava
/// ao import como "moderado" e cobrava conserto de um fim de semana sem batida.
#[test]
fn perda_de_controle_sem_impacto_nao_e_batida() {
    let a = tentativa(0.0, true, vec![batida(61.0, "moderado", false)]);
    assert_eq!(severidade(a), "nenhum");
}

/// A regra do desfecho ("cruzou a bandeirada ⇒ não foi perda total") valia só para as
/// batidas fechadas; o pico bruto passava por cima dela no `max` e o custo de conserto
/// — que assume a severidade já rebaixada — cobrava um nível acima do devido.
#[test]
fn batida_de_quem_terminou_a_corrida_chega_rebaixada() {
    // Pico "grave" (≥110) numa corrida terminada → chega como "moderado".
    assert_eq!(severidade(tentativa(120.0, true, vec![])), "moderado");
    // O mesmo pico sem cruzar a bandeirada (abandonou) NÃO é rebaixado.
    assert_eq!(severidade(tentativa(120.0, false, vec![])), "grave");
    // Toque leve de quem terminou continua "leve" — que não cobra nada.
    assert_eq!(severidade(tentativa(20.0, true, vec![])), "leve");
}

/// A batida fechada COM impacto continua contando (o gate é sobre a evidência, não
/// sobre o caminho): quem bateu de verdade segue pagando.
#[test]
fn batida_com_impacto_continua_contando() {
    let a = tentativa(0.0, false, vec![batida(120.0, "grave", true)]);
    assert_eq!(severidade(a), "grave");
}

/// Dupla confirmação: o rebaixamento é uma presunção ("terminou, logo não foi terminal"),
/// e o sim tem a última palavra sobre ela. Pediu reparo ⇒ o carro quebrou de verdade e a
/// severidade fica de pé. O silêncio dos canais não conclui nada e não interfere.
#[test]
fn reparo_pedido_pelo_sim_sustenta_a_severidade() {
    // Mesmo pico "grave" e mesma bandeirada; só muda o que o sim reportou.
    let confirmado = tentativa_com_reparo(120.0, true, vec![], 42.0);
    assert_eq!(severidade(confirmado), "grave");
    let sem_sinal = tentativa_com_reparo(120.0, true, vec![], 0.0);
    assert_eq!(severidade(sem_sinal), "moderado");

    // E não cria dano onde não houve batida: sem impacto, o reparo sozinho não cobra.
    let so_reparo = tentativa_com_reparo(0.0, true, vec![batida(61.0, "moderado", false)], 42.0);
    assert_eq!(severidade(so_reparo), "nenhum");
}
