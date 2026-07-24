use std::collections::HashMap;

use super::combustivel::analyze_fuel;
use super::setores::analyze_sectors;
use super::*;
use crate::iracing_sdk::race_monitor::{
    CarLap, LapSnapshot, PlayerLap, PlayerTrackPoint, RaceHistory,
};

fn base_history() -> RaceHistory {
    let mut h = RaceHistory::clone(&empty());
    h.player_car_idx = 0;
    h
}
// RaceHistory não expõe Default; montamos pelo serde de um JSON mínimo.
fn empty() -> RaceHistory {
    serde_json::from_value(serde_json::json!({
        "laps": [], "player_laps": [], "player_track": [], "yellow_laps": [],
        "player_car_idx": 0, "attempt_number": 1, "finished": true, "outcome": "Finalizada",
        "car_laps": [], "cars_meta": [], "track_id": 1, "qualy_laps": []
    }))
    .unwrap()
}

#[test]
fn pace_e_consistencia() {
    let mut h = base_history();
    // Voltas do jogador: 90, 90.5, 91, e uma ruim 96 (erro).
    h.player_laps = vec![
        PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 2, time: 90.5, fuel_remaining: -1.0 },
        PlayerLap { lap: 3, time: 91.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 4, time: 96.0, fuel_remaining: -1.0 },
    ];
    // Campo um pouco mais lento.
    for lap in 1..=4 {
        h.car_laps.push(CarLap { car_idx: 1, lap, time: 92.0 });
    }
    let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
    let p = a.pace.expect("tem ritmo");
    assert!((p.best_lap_ms - 90_000.0).abs() < 1.0);
    // Volta limpa exclui a de 96 (96000 > 90000*1.04=93600).
    assert_eq!(p.good_laps, 3);
    assert_eq!(p.total_laps, 4);
    // Perdeu tempo por causa da volta ruim.
    assert!(p.lost_per_lap_ms > 0.0);
    // Mais rápido que o campo (limpo ~90.5s vs campo 92s).
    assert!(p.vs_grid_ms < 0.0);
    // 4 voltas → consistência confiável; 4 voltas do campo → vs_grid confiável.
    assert!(p.consistency_reliable);
    assert!(p.vs_grid_reliable);
    assert_eq!(p.grid_sample, 4);
}

#[test]
fn ritmo_some_mas_consistencia_nao_confiavel_com_2_voltas() {
    let mut h = base_history();
    h.player_laps = vec![
        PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 2, time: 90.5, fuel_remaining: -1.0 },
    ];
    let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
    let p = a.pace.expect("2 voltas já dá ritmo");
    assert_eq!(p.total_laps, 2);
    // < 3 voltas → consistência não confiável (a tela esconde o card).
    assert!(!p.consistency_reliable);
    // sem voltas do campo → vs_grid não confiável.
    assert!(!p.vs_grid_reliable);
}

#[test]
fn uma_volta_so_nao_gera_ritmo() {
    let mut h = base_history();
    h.player_laps = vec![PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 }];
    let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
    assert!(a.pace.is_none());
}

#[test]
fn rival_e_quem_mais_brigou() {
    let mut h = base_history();
    // O carro idx 5 fica à frente/atrás por 3 voltas; o 9 só 1 volta.
    h.player_track = vec![
        PlayerTrackPoint { session_time: 1.0, lap: 1, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 0.5, behind_idx: 9, gap_behind: 0.8 },
        PlayerTrackPoint { session_time: 2.0, lap: 2, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 0.7, behind_idx: -1, gap_behind: 0.0 },
        PlayerTrackPoint { session_time: 3.0, lap: 3, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 0.6, behind_idx: -1, gap_behind: 0.0 },
    ];
    let mut names = HashMap::new();
    names.insert(5, "Lucas Silva".to_string());
    names.insert(9, "Rafael Costa".to_string());
    let a = analyze(&h, &names, &HashMap::new(), &PlayerIncidents::default());
    let r = a.rival.expect("tem rival");
    assert_eq!(r.pilot_name, "Lucas Silva");
    assert_eq!(r.laps_battled, 3);
    assert!((r.avg_gap_s - 0.6).abs() < 0.05);
}

#[test]
fn rival_rejeitado_com_poucas_voltas() {
    let mut h = base_history();
    // Só 2 voltas ao lado do idx 5 — abaixo do mínimo de rival.
    h.player_track = vec![
        PlayerTrackPoint { session_time: 1.0, lap: 1, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 0.5, behind_idx: -1, gap_behind: 0.0 },
        PlayerTrackPoint { session_time: 2.0, lap: 2, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 0.6, behind_idx: -1, gap_behind: 0.0 },
    ];
    let mut names = HashMap::new();
    names.insert(5, "Lucas Silva".to_string());
    let a = analyze(&h, &names, &HashMap::new(), &PlayerIncidents::default());
    assert!(a.rival.is_none(), "2 voltas não é rival");
}

#[test]
fn rival_rejeitado_com_gap_grande() {
    let mut h = base_history();
    // 3 voltas, mas sempre muito longe (gap > 3s) — não é disputa.
    h.player_track = vec![
        PlayerTrackPoint { session_time: 1.0, lap: 1, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 8.0, behind_idx: -1, gap_behind: 0.0 },
        PlayerTrackPoint { session_time: 2.0, lap: 2, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 9.0, behind_idx: -1, gap_behind: 0.0 },
        PlayerTrackPoint { session_time: 3.0, lap: 3, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 7.5, behind_idx: -1, gap_behind: 0.0 },
    ];
    let mut names = HashMap::new();
    names.insert(5, "Lucas Silva".to_string());
    let a = analyze(&h, &names, &HashMap::new(), &PlayerIncidents::default());
    assert!(a.rival.is_none(), "gap grande não é rival");
}

#[test]
fn confianca_alta_quando_cobre_a_corrida_toda() {
    let mut h = base_history();
    // Corrida de 10 voltas (líder), jogador fez 10.
    for lap in 1..=10 {
        h.laps.push(LapSnapshot { lap, progress: 0.0, cars: vec![] });
        h.player_laps.push(PlayerLap { lap, time: 90.0, fuel_remaining: -1.0 });
    }
    let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
    assert_eq!(a.race_laps, 10);
    assert_eq!(a.laps_seen, 10);
    assert_eq!(a.confidence, "alta");
    assert!(!a.is_partial);
}

#[test]
fn confianca_baixa_e_parcial_quando_saiu_cedo() {
    let mut h = base_history();
    // Corrida de 12 voltas, jogador fez só 3 → saiu cedo.
    for lap in 1..=12 {
        h.laps.push(LapSnapshot { lap, progress: 0.0, cars: vec![] });
    }
    for lap in 1..=3 {
        h.player_laps.push(PlayerLap { lap, time: 90.0, fuel_remaining: -1.0 });
    }
    let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
    assert_eq!(a.race_laps, 12);
    assert_eq!(a.last_lap_seen, 3);
    assert_eq!(a.confidence, "baixa");
    assert!(a.is_partial);
}

#[test]
fn position_flow_conta_subidas_e_quedas() {
    let mut h = base_history();
    // Trajetória de posição: P14 → P12 (subiu 2) → P13 (caiu 1) → P8 (subiu 5).
    let pos_seq = [14, 14, 12, 12, 13, 8, 8];
    h.player_track = pos_seq
        .iter()
        .enumerate()
        .map(|(i, &pos)| PlayerTrackPoint {
            session_time: i as f64,
            lap: i as i32,
            position: pos,
            speed_kmh: 200.0,
            ahead_idx: -1,
            gap_ahead: 0.0,
            behind_idx: -1,
            gap_behind: 0.0,
        })
        .collect();
    let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
    let f = a.position_flow.expect("tem fluxo de posição");
    assert_eq!(f.gained_on_track, 7); // 2 + 5
    assert_eq!(f.lost_on_track, 1);
    assert_eq!(f.samples, 7);
}

#[test]
fn erro_mais_caro_incidente_com_perda() {
    let mut h = base_history();
    // Ritmo limpo ~90s; volta 7 explode para 95s (perdeu ~5s).
    h.player_laps = vec![
        PlayerLap { lap: 5, time: 90.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 6, time: 90.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 7, time: 95.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 8, time: 90.0, fuel_remaining: -1.0 },
    ];
    // Caiu de P7 (fim da volta 6) para P9 (fim da volta 7).
    h.player_track = vec![
        PlayerTrackPoint { session_time: 6.0, lap: 6, position: 7, speed_kmh: 200.0, ahead_idx: -1, gap_ahead: 0.0, behind_idx: -1, gap_behind: 0.0 },
        PlayerTrackPoint { session_time: 7.0, lap: 7, position: 9, speed_kmh: 150.0, ahead_idx: -1, gap_ahead: 0.0, behind_idx: -1, gap_behind: 0.0 },
    ];
    let inc = PlayerIncidents { crash_laps: vec![7], is_dnf: false, dnf_lap: None };
    let a = analyze(&h, &HashMap::new(), &HashMap::new(), &inc);
    let m = a.mistake.expect("tem erro mais caro");
    assert_eq!(m.lap, 7);
    assert_eq!(m.kind, "incident");
    assert_eq!(m.positions_lost, 2);
    assert!(m.time_lost_ms > 3000.0);
    assert_eq!(m.confidence, "alta"); // lenta + perda + incidente
}

#[test]
fn erro_mais_caro_dnf_domina() {
    let mut h = base_history();
    h.player_laps = vec![PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 }, PlayerLap { lap: 2, time: 90.0, fuel_remaining: -1.0 }];
    let inc = PlayerIncidents { crash_laps: vec![], is_dnf: true, dnf_lap: Some(9) };
    let a = analyze(&h, &HashMap::new(), &HashMap::new(), &inc);
    let m = a.mistake.expect("DNF é o erro mais caro");
    assert_eq!(m.kind, "dnf");
    assert_eq!(m.lap, 9);
}

#[test]
fn corrida_limpa_nao_mostra_erro() {
    let mut h = base_history();
    // Voltas consistentes, sem incidente nem perda de posição.
    h.player_laps = (1..=8)
        .map(|lap| PlayerLap { lap, time: 90.0 + (lap as f64 % 2.0) * 0.2, fuel_remaining: -1.0 })
        .collect();
    let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
    assert!(a.mistake.is_none(), "corrida limpa não inventa erro");
}

#[test]
fn largada_lenta_nao_vira_erro_mais_caro() {
    let mut h = base_history();
    // Volta de largada (2) larga do grid: +10s vs ritmo. O erro REAL é na 13
    // (+4s). Sem a proteção, a largada (+10s) domina; com ela, o erro certo aparece.
    let mut laps = vec![PlayerLap { lap: 2, time: 100.0, fuel_remaining: -1.0 }];
    for lap in 3..=12 {
        laps.push(PlayerLap { lap, time: 90.0, fuel_remaining: -1.0 });
    }
    laps.push(PlayerLap { lap: 13, time: 94.0, fuel_remaining: -1.0 });
    h.player_laps = laps;
    let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
    let m = a.mistake.expect("o erro da volta 13 ainda é um erro");
    assert_eq!(m.lap, 13, "a largada não pode roubar o erro mais caro");
    assert_eq!(m.kind, "pace_drop");
}

#[test]
fn batida_na_largada_ainda_conta() {
    let mut h = base_history();
    // Largada lenta E com batida: o incidente na largada continua flagrado
    // (só a lentidão sistêmica é neutralizada, não o contato real).
    let mut laps = vec![PlayerLap { lap: 2, time: 100.0, fuel_remaining: -1.0 }];
    for lap in 3..=8 {
        laps.push(PlayerLap { lap, time: 90.0, fuel_remaining: -1.0 });
    }
    h.player_laps = laps;
    let inc = PlayerIncidents { crash_laps: vec![2], is_dnf: false, dnf_lap: None };
    let a = analyze(&h, &HashMap::new(), &HashMap::new(), &inc);
    let m = a.mistake.expect("batida na largada é um incidente");
    assert_eq!(m.lap, 2);
    assert_eq!(m.kind, "incident");
}

#[test]
fn melhor_momento_ataque_decisivo() {
    let mut h = base_history();
    // Ritmo ~90s; volta 2 é a melhor (89s) e ganhou 2 posições.
    h.player_laps = vec![
        PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 2, time: 89.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 3, time: 90.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 4, time: 90.0, fuel_remaining: -1.0 },
    ];
    // P10 no fim da volta 1 → P8 no fim da volta 2 (ganho de 2).
    h.player_track = vec![
        PlayerTrackPoint { session_time: 1.0, lap: 1, position: 10, speed_kmh: 200.0, ahead_idx: -1, gap_ahead: 0.0, behind_idx: -1, gap_behind: 0.0 },
        PlayerTrackPoint { session_time: 2.0, lap: 2, position: 8, speed_kmh: 205.0, ahead_idx: -1, gap_ahead: 0.0, behind_idx: -1, gap_behind: 0.0 },
    ];
    let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
    let b = a.best_moment.expect("tem melhor momento");
    assert_eq!(b.kind, "position_gain");
    assert_eq!(b.lap, 2);
    assert_eq!(b.positions_gained, 2);
    assert_eq!(b.confidence, "alta"); // melhor volta + ganho
}

#[test]
fn melhor_momento_rival_superado() {
    let mut h = base_history();
    // Sem voltas registradas (foco no rival); idx 7 fica ATRÁS por 6 voltas.
    h.player_track = (1..=6)
        .map(|lap| PlayerTrackPoint {
            session_time: lap as f64,
            lap,
            position: 5,
            speed_kmh: 200.0,
            ahead_idx: -1,
            gap_ahead: 0.0,
            behind_idx: 7,
            gap_behind: 0.5,
        })
        .collect();
    let mut names = HashMap::new();
    names.insert(7, "Carlos Mendes".to_string());
    let a = analyze(&h, &names, &HashMap::new(), &PlayerIncidents::default());
    let b = a.best_moment.expect("tem melhor momento");
    assert_eq!(b.kind, "rival_beaten");
    assert_eq!(b.rival_name, "Carlos Mendes");
    assert_eq!(b.streak, 6);
    assert_eq!(b.confidence, "alta");
}

#[test]
fn charts_monta_trace_e_tempos() {
    use crate::iracing_sdk::race_monitor::CarGapPoint;
    let mut h = base_history();
    // 2 voltas de trace com 2 carros (jogador idx 0 + idx 1).
    for lap in 1..=2 {
        h.laps.push(LapSnapshot {
            lap,
            progress: 0.0,
            cars: vec![
                CarGapPoint { idx: 0, position: 3, gap: 1.2, ..Default::default() },
                CarGapPoint { idx: 1, position: 1, gap: 0.0, ..Default::default() },
            ],
        });
        h.player_laps.push(PlayerLap { lap, time: 90.0, fuel_remaining: -1.0 });
    }
    let mut names = HashMap::new();
    names.insert(1, "Lider Silva".to_string());
    let a = analyze(&h, &names, &HashMap::new(), &PlayerIncidents::default());
    let c = a.charts.expect("tem gráficos");
    assert_eq!(c.cars.len(), 2);
    assert!(c.cars.iter().any(|car| car.is_player && car.points.len() == 2));
    assert_eq!(c.lap_times.len(), 2);
}

#[test]
fn sem_telemetria_nao_quebra() {
    let h = base_history();
    let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
    assert!(!a.has_telemetry);
    assert!(a.pace.is_none());
    assert!(a.rival.is_none());
}

#[test]
fn combustivel_calcula_consumo_e_autonomia() {
    let mut h = base_history();
    h.player_laps = vec![
        PlayerLap { lap: 1, time: 90.0, fuel_remaining: 40.0 },
        PlayerLap { lap: 2, time: 90.0, fuel_remaining: 38.0 },
        PlayerLap { lap: 3, time: 90.0, fuel_remaining: 36.0 },
    ];
    let fuel = analyze_fuel(&h).expect("tem combustível");
    assert!((fuel.used_per_lap_l - 2.0).abs() < 1e-6, "2 L/volta");
    assert!((fuel.remaining_l - 36.0).abs() < 1e-6);
    assert!((fuel.laps_left - 18.0).abs() < 1e-6, "36 / 2 = 18 voltas");
}

#[test]
fn combustivel_none_sem_dado() {
    let mut h = base_history();
    h.player_laps = vec![
        PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 2, time: 90.0, fuel_remaining: -1.0 },
    ];
    assert!(analyze_fuel(&h).is_none());
}

#[test]
fn setores_apontam_o_ponto_fraco() {
    use crate::iracing_sdk::race_monitor::SectorSplit;
    let mut h = base_history();
    // S2 é o fraco: 30.0 e 31.0 → melhor 30.0, média 30.5, perda 0.5s.
    h.player_sectors = vec![
        SectorSplit { lap: 1, sector: 1, time: 20.0 },
        SectorSplit { lap: 2, sector: 1, time: 20.1 },
        SectorSplit { lap: 1, sector: 2, time: 30.0 },
        SectorSplit { lap: 2, sector: 2, time: 31.0 },
        SectorSplit { lap: 1, sector: 3, time: 25.0 },
        SectorSplit { lap: 2, sector: 3, time: 25.05 },
    ];
    let s = analyze_sectors(&h).expect("tem setores");
    assert_eq!(s.weakest_sector, 2);
    assert!((s.weakest_loss_ms - 500.0).abs() < 1.0, "perda ~0.5s no S2");
    assert!((s.best_ms[1] - 30000.0).abs() < 1.0, "melhor S2 = 30.0s");
}

fn track_point(session_time: f64, lap: i32, position: i32, ahead: i32, ga: f64, behind: i32, gb: f64) -> PlayerTrackPoint {
    PlayerTrackPoint {
        session_time,
        lap,
        position,
        speed_kmh: 200.0,
        ahead_idx: ahead,
        gap_ahead: ga,
        behind_idx: behind,
        gap_behind: gb,
    }
}

#[test]
fn dossie_consistencia_maior_para_voltas_parelhas() {
    let mut tight = base_history();
    tight.player_laps = vec![
        PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 2, time: 90.1, fuel_remaining: -1.0 },
        PlayerLap { lap: 3, time: 90.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 4, time: 90.2, fuel_remaining: -1.0 },
    ];
    let mut messy = base_history();
    messy.player_laps = vec![
        PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 2, time: 90.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 3, time: 90.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 4, time: 97.0, fuel_remaining: -1.0 },
    ];
    let empty_tel = TelemetryAnalysis::default();
    let c_tight = extract_player_race_telemetry(&tight, &empty_tel).unwrap().consistency;
    let c_messy = extract_player_race_telemetry(&messy, &empty_tel).unwrap().consistency;
    assert!(c_tight > c_messy, "parelho={c_tight} vs bagunçado={c_messy}");
    assert!(c_tight > 90.0);
}

#[test]
fn dossie_consistencia_indisponivel_com_poucas_voltas() {
    let mut h = base_history();
    h.player_laps = vec![
        PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 },
        PlayerLap { lap: 2, time: 90.0, fuel_remaining: -1.0 },
    ];
    let row = extract_player_race_telemetry(&h, &TelemetryAnalysis::default()).unwrap();
    assert_eq!(row.consistency, -1.0, "< 3 voltas → não computável");
    assert_eq!(row.laps_seen, 2);
}

#[test]
fn dossie_fracao_de_briga() {
    let mut h = base_history();
    // 12 amostras: 9 com vizinho a < 1s (briga), 3 sozinho longe.
    let mut pts = Vec::new();
    for i in 0..9 {
        pts.push(track_point(i as f64, 1, 5, 3, 0.4, -1, 0.0));
    }
    for i in 9..12 {
        pts.push(track_point(i as f64, 1, 5, 3, 5.0, -1, 0.0));
    }
    h.player_track = pts;
    let row = extract_player_race_telemetry(&h, &TelemetryAnalysis::default()).unwrap();
    assert!((row.battle_fraction - 9.0 / 12.0).abs() < 1e-6, "got {}", row.battle_fraction);
}

#[test]
fn dossie_start_delta_conta_ganho_na_largada() {
    let mut h = base_history();
    // Largou P10 (t=0), em +20s já é P6 → ganhou 4 na largada.
    h.player_track = vec![
        track_point(0.0, 1, 10, -1, 0.0, -1, 0.0),
        track_point(10.0, 1, 8, -1, 0.0, -1, 0.0),
        track_point(21.0, 1, 6, -1, 0.0, -1, 0.0),
        track_point(40.0, 2, 6, -1, 0.0, -1, 0.0),
    ];
    let row = extract_player_race_telemetry(&h, &TelemetryAnalysis::default()).unwrap();
    assert!(row.start_valid);
    assert_eq!(row.start_delta, 4);
}

#[test]
fn dossie_none_sem_nada_utilizavel() {
    let h = base_history();
    assert!(extract_player_race_telemetry(&h, &TelemetryAnalysis::default()).is_none());
}
