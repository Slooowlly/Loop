//! Os cenários que mais de um arquivo desta suíte monta.
//!
//! Só entra aqui o que TEM consumidor em dois arquivos diferentes — um ajudante usado por
//! um só arquivo mora nele, perto do teste que o explica. A regra evita o destino comum de
//! módulos assim: virar o depósito onde todo ajudante acaba, longe do caso que o motiva.

use super::super::*;

pub(super) fn active_attempt() -> Attempt {
    Attempt {
        number: 1,
        status: StatusTentativa::Active,
        started_at_session_time: 0.0,
        laps_completed: 0,
        ended_by: None,
        reason: None,
        worst_crash: None,
        evidence: AttemptEvidence::default(),
        crashes: Vec::new(),
        peak_crash_score: 0.0,
        collided_with_car_number: None,
        peak_impact_dir: None,
        sim_repair_needed_s: 0.0,
        sim_repair_required_s: 0.0,
        style: crate::car::driving_style::StyleAccumulator::new(),
    }
}

pub(super) fn on_track(idx: i32, position: i32, lap_completed: i32) -> CarSnapshot {
    CarSnapshot {
        idx,
        position,
        lap_completed,
        track_surface: SURFACE_ON_TRACK,
        ..Default::default()
    }
}

/// Uma volta de classificatória entregue do jeito que o SDK entrega de verdade.
///
/// O `CarIdxLastLapTime` NÃO acompanha a virada da contagem: no instante em que o carro cruza
/// a linha ele ainda exibe o tempo da volta ANTERIOR (`sdk_antes`), e o valor desta volta só
/// aparece alguns décimos depois. É por isso que um tique isolado não fecha volta nenhuma —
/// ver `race_monitor/voltas.rs`, e a medição que originou o módulo.
pub(super) fn volta_de_quali(
    m: &mut RaceMonitor,
    session_num: i32,
    idx: i32,
    volta: i32,
    tempo: f64,
    sdk_antes: f64,
    best: f64,
) {
    let inicio = volta as f64 * 200.0;
    for (agora, contagem, sdk) in [
        (inicio, volta - 1, sdk_antes),
        (inicio + tempo, volta, sdk_antes),
        (inicio + tempo + 0.2, volta, tempo),
    ] {
        let mut car = on_track(idx, 0, contagem);
        car.last_lap_time = sdk;
        car.best_lap_time = best;
        m.capture_qualy(&IracingTelemetry {
            session_num,
            session_time: agora,
            cars: vec![car],
            ..Default::default()
        });
    }
}

/// Frame de corrida verde: líder (idx 1) em P1 na volta `leader_lap`, jogador
/// (idx 0) em P2 logo atrás.
pub(super) fn race_frame(session_num: i32, leader_lap: i32) -> IracingTelemetry {
    IracingTelemetry {
        session_num,
        session_state: STATE_RACING,
        session_time: 100.0,
        player_car_idx: 0,
        cars: vec![on_track(0, 2, leader_lap - 1), on_track(1, 1, leader_lap)],
        ..Default::default()
    }
}

/// Frame com o jogador na PONTA (idx 0, P1), um carro SENTINELADO (idx 7,
/// `position` 0 — o "sem posição atribuída" do iRacing: garagem, fora do mundo,
/// sessão de quali) e o segundo colocado real (idx 2). O sentinelado vem antes
/// no vetor de propósito: é ele que um `find` desguardado acha primeiro.
pub(super) fn frame_de_lider_com_sentinela() -> IracingTelemetry {
    let mut eu = on_track(0, 1, 3);
    eu.est_time = 10.0;
    eu.best_lap_time = 95.0;
    let mut fantasma = on_track(7, 0, 0);
    fantasma.est_time = 70.0;
    let mut segundo = on_track(2, 2, 3);
    segundo.est_time = 8.0;
    IracingTelemetry {
        session_num: 2,
        session_state: STATE_RACING,
        session_time: 100.0,
        player_car_idx: 0,
        cars: vec![fantasma, eu, segundo],
        ..Default::default()
    }
}

/// Frame do jogador (idx 0) correndo, com os canais que o scorer de batida lê.
pub(super) fn frame_do_jogador(
    session_num: i32,
    incidentes: i32,
    surface: i32,
) -> IracingTelemetry {
    IracingTelemetry {
        session_num,
        session_state: STATE_RACING,
        session_time: 100.0,
        player_car_idx: 0,
        lap_completed: 3,
        incident_count: incidentes,
        track_surface: surface,
        speed_ms: 50.0,
        cars: vec![on_track(0, 2, 3), on_track(1, 1, 3)],
        ..Default::default()
    }
}
