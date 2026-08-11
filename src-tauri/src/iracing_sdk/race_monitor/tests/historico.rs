//! Testes de [`super::super::historico`]: o race trace volta a volta, os snapshots por
//! evento, as paradas de box e o gap dos vizinhos que alimenta o pós-corrida.

use super::super::*;
use super::comum::*;

#[test]
fn historico_da_tentativa_recebe_subsession_atual() {
    let mut m = RaceMonitor::new();
    m.set_session_subsession_id(4242);
    m.attempts.push(active_attempt());

    m.record_history(&race_frame(2, 1));

    assert_eq!(m.history.subsession_id, 4242);
}

#[test]
fn quali_nao_entra_no_historico_da_corrida() {
    let mut m = RaceMonitor::new();
    m.qualy_session_num = 1;
    m.attempts.push(active_attempt());

    // Tick da CLASSIFICATÓRIA (session_num == qualy) não grava nada.
    m.record_history(&race_frame(1, 3));
    assert!(
        m.history.laps.is_empty(),
        "a quali não pode entrar no histórico da corrida"
    );

    // Tick da CORRIDA (outro session_num) grava normalmente.
    m.record_history(&race_frame(2, 3));
    assert!(
        !m.history.laps.is_empty(),
        "a corrida deve gravar snapshots"
    );
    assert_eq!(m.hist_session_num, 2);
}

#[test]
fn troca_de_sessao_zera_o_historico() {
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());

    // Sessão anterior (ex.: treino, session 0) grava alguns snapshots.
    m.record_history(&race_frame(0, 5));
    assert!(!m.history.laps.is_empty());

    // Corrida (session 2) → histórico limpo, sem herdar a sessão anterior.
    m.record_history(&race_frame(2, 1));
    assert_eq!(m.hist_session_num, 2);
    assert!(
        m.history.laps.iter().all(|s| s.lap <= 1),
        "a corrida não pode herdar as voltas da sessão anterior"
    );
}

#[test]
fn troca_de_posicao_gera_snapshot_na_hora() {
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());

    // Volta 3 do líder; jogador (idx 0) em P3.
    let mut f = race_frame(2, 3);
    f.session_time = 10.0;
    f.cars = vec![on_track(0, 3, 2), on_track(1, 1, 3), on_track(2, 2, 3)];
    m.record_history(&f);
    let n0 = m.history.laps.len();
    assert!(n0 >= 1);

    // MESMA volta do líder, mas o jogador passa o idx 2 (P3 → P2) no meio da
    // volta. Líder a 40% da volta → o ponto sai em 3.4, não na virada.
    let mut f2 = race_frame(2, 3);
    f2.session_time = 12.0;
    let mut leader = on_track(1, 1, 3);
    leader.lap_dist_pct = 0.4;
    f2.cars = vec![on_track(0, 2, 2), leader, on_track(2, 3, 2)];
    m.record_history(&f2);

    assert_eq!(
        m.history.laps.len(),
        n0 + 1,
        "troca de posição deve gerar um snapshot na hora (sem virar a volta)"
    );
    let last = m.history.laps.last().unwrap();
    assert_eq!(last.lap, 3);
    assert!(
        (last.progress - 0.4).abs() < 1e-5,
        "progresso do líder na volta"
    );
    assert_eq!(
        last.cars.iter().find(|c| c.idx == 0).unwrap().position,
        2,
        "jogador subiu para P2 no ponto registrado"
    );
}

#[test]
fn setores_do_jogador_sao_cronometrados() {
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());
    // Jogador (idx 0) avança pela pista; capture_player_sectors cronometra ao
    // fechar cada setor entrado LIMPO (do começo dele).
    fn push(m: &mut RaceMonitor, pct: f64, time: f64, lap: i32) {
        let mut f = race_frame(2, 3);
        f.session_time = time;
        f.lap = lap;
        f.lap_dist_pct = pct;
        f.track_surface = SURFACE_ON_TRACK;
        f.cars = vec![on_track(0, 2, lap - 1), on_track(1, 1, lap)];
        m.record_history(&f);
    }
    push(&mut m, 0.10, 100.0, 2); // base no S1 (parcial → não grava)
    push(&mut m, 0.40, 110.0, 2); // entra S2 LIMPO
    push(&mut m, 0.70, 118.0, 2); // entra S3 → fecha S2 (8s)
    push(&mut m, 0.05, 126.0, 3); // cruza a linha → fecha S3 (8s)

    let secs: Vec<(i32, f64)> = m
        .history
        .player_sectors
        .iter()
        .map(|s| (s.sector, (s.time * 10.0).round() / 10.0))
        .collect();
    assert!(secs.contains(&(2, 8.0)), "S2 cronometrado: {secs:?}");
    assert!(secs.contains(&(3, 8.0)), "S3 cronometrado: {secs:?}");
    // O S1 inicial era parcial (entramos no meio) → não pode ter sido gravado.
    assert!(
        !secs.iter().any(|(s, _)| *s == 1),
        "S1 parcial não grava: {secs:?}"
    );
}

#[test]
fn blip_de_pit_stall_de_dwell_zero_nao_vira_parada() {
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());

    // Entra na corrida e assenta o histórico.
    m.record_history(&race_frame(2, 2));

    // Jogador pisca `InPitStall` por um instante (mesmo session_time → dwell 0).
    let mut blip = race_frame(2, 3);
    blip.cars[0].track_surface = SURFACE_IN_PIT_STALL;
    m.record_history(&blip);
    let mut leave = race_frame(2, 3);
    leave.cars[0].track_surface = SURFACE_ON_TRACK; // saiu no mesmo instante
    m.record_history(&leave);

    assert!(
        m.history.pit_stops.is_empty(),
        "blip transiente de pit stall (dwell ~0) não pode virar parada"
    );
}

// ── Histórico: o gap dos vizinhos que alimenta o pós-corrida ─────────────

/// Frame com o jogador (idx 0, P2) entre um carro à frente e um atrás, com
/// `f2_time` e `est_time` controlados. Os `f2_time` são os medidos numa captura
/// real de corrida de IA, onde os dois estavam a ~98 s do líder.
fn frame_de_briga(meu_est: f64, est_frente: f64, est_atras: f64) -> IracingTelemetry {
    let mut eu = on_track(0, 2, 3);
    eu.est_time = meu_est;
    eu.f2_time = 98.294_502_258_300_78;
    eu.best_lap_time = 95.0;
    let mut frente = on_track(1, 1, 3);
    frente.est_time = est_frente;
    frente.f2_time = 98.129_798_889_160_16;
    let mut atras = on_track(2, 3, 3);
    atras.est_time = est_atras;
    atras.f2_time = 98.294_502_258_300_78;
    IracingTelemetry {
        session_num: 2,
        session_state: STATE_RACING,
        session_time: 100.0,
        player_car_idx: 0,
        cars: vec![eu, frente, atras],
        ..Default::default()
    }
}

#[test]
fn gap_dos_vizinhos_sai_do_est_time_e_nao_do_f2_time() {
    // O caso que quebrava: `f2_time` a 0,165 s de distância (os dois a ~98 s do
    // líder) com o carro da frente a 40 s de pista. Pelo `f2_time` isso virava
    // card de rival e uma série de gráfico inventadas.
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());

    m.record_history(&frame_de_briga(10.0, 50.0, 8.0));

    let p = m.history.player_track.last().expect("amostra de vizinho");
    assert_eq!((p.ahead_idx, p.behind_idx), (1, 2));
    assert!(
        (p.gap_ahead - 40.0).abs() < 0.01,
        "gap à frente devia ser 40 s de pista, veio {}",
        p.gap_ahead
    );
    assert!(
        (p.gap_behind - 2.0).abs() < 0.01,
        "gap atrás devia ser 2 s de pista, veio {}",
        p.gap_behind
    );
}

#[test]
fn gap_dos_vizinhos_fecha_o_circulo_da_volta() {
    // Eu quase na linha (est 93 de uma volta de 95); ele a 1,5 s dela — já cruzou.
    // A subtração crua daria -91,5, e o carro a um segundo e meio viraria a corrida
    // inteira de distância.
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());

    m.record_history(&frame_de_briga(93.0, 1.5, 90.0));

    let p = m.history.player_track.last().expect("amostra de vizinho");
    assert!(
        (p.gap_ahead - 3.5).abs() < 0.01,
        "esperava ~3,5 s fechando o círculo, veio {}",
        p.gap_ahead
    );
}

#[test]
fn sem_vizinho_o_gap_e_desconhecido_e_nao_zero() {
    // O líder não tem ninguém à frente. Um 0 aqui seria lido como "colado nele" —
    // é assim que o card de rival nasce de uma corrida solitária.
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());
    let mut eu = on_track(0, 1, 3);
    eu.est_time = 10.0;
    eu.best_lap_time = 95.0;

    m.record_history(&IracingTelemetry {
        session_num: 2,
        session_state: STATE_RACING,
        session_time: 100.0,
        player_car_idx: 0,
        cars: vec![eu],
        ..Default::default()
    });

    let p = m.history.player_track.last().expect("amostra de vizinho");
    assert_eq!(p.ahead_idx, -1);
    assert!(p.gap_ahead < 0.0, "sem vizinho o gap não pode ser 0");
    assert_eq!(p.behind_idx, -1);
    assert!(p.gap_behind < 0.0, "sem vizinho o gap não pode ser 0");
}

#[test]
fn lider_nao_ganha_carro_a_frente_fabricado_do_sentinela() {
    // Com o jogador em P1, `me.position - 1` vale 0 — e 0 não é uma posição, é o
    // sentinela de "sem posição atribuída". Sem guarda, o líder recebia como
    // vizinho da frente um carro na garagem, com gap tirado do `est_time` dele.
    // Em captura real (race_1785889561.jsonl.gz) isso era 29% das amostras.
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());

    m.record_history(&frame_de_lider_com_sentinela());

    let p = m.history.player_track.last().expect("amostra de vizinho");
    assert_eq!(
        p.ahead_idx, -1,
        "carro de position 0 não é o carro à frente do líder"
    );
    assert!(
        p.gap_ahead < 0.0,
        "sem vizinho à frente o gap é desconhecido, veio {}",
        p.gap_ahead
    );
    assert_eq!(p.behind_idx, 2, "o carro de trás continua sendo achado");
}
