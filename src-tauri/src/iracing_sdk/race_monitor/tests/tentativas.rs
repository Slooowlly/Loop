//! Testes de [`super::super::tentativas`]: o reinício da corrida, o que ele apaga e o que
//! a rebobinada do replay NÃO é.

use super::super::quebras::BREAKDOWN_GRACE_SECS;
use super::super::*;
use super::comum::*;

// ── Reinício da corrida: a tentativa abandonada não deixa rastro ─────────────

/// Frame de corrida verde com o jogador (idx 0) na pista e o líder (idx 1) à frente. O par
/// (`session_time`, `lap_completed`) é justamente o que o detector de reinício compara.
fn frame_de_corrida(session_time: f64, lap: i32) -> IracingTelemetry {
    IracingTelemetry {
        session_num: 2,
        session_state: STATE_RACING,
        session_time,
        player_car_idx: 0,
        lap_completed: lap,
        track_surface: SURFACE_ON_TRACK,
        // O jogador PILOTANDO: é o que separa o reinício da rebobinada do replay.
        on_track: true,
        speed_ms: 50.0,
        cars: vec![on_track(0, 2, lap), on_track(1, 1, lap)],
        ..Default::default()
    }
}

/// Um monitor já com meia corrida disputada na tentativa que o jogador vai jogar fora.
fn monitor_em_corrida_disputada() -> RaceMonitor {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;
    m.prev_session_num = 2;
    m.observe(&frame_de_corrida(100.0, 8));
    m.observe(&frame_de_corrida(720.0, 12));
    m
}

/// O reinício da corrida É um salto de relógio: o `SessionTime` volta de uma vez para perto
/// de zero. Enquanto o guarda de salto de replay zerava o `prev` nesse mesmo tick,
/// `restarted()` ficava sem o que comparar — nem pelo tempo, nem pela queda de
/// `lap_completed` — e o reinício NUNCA era detectado. A tentativa abandonada seguia aberta e
/// tudo que aconteceu nela continuava alimentando a corrida que valeu.
#[test]
fn reinicio_e_detectado_apesar_do_salto_de_relogio() {
    let mut m = monitor_em_corrida_disputada();
    let abandonada = m.current_attempt;
    assert!(
        m.attempts.last().unwrap().evidence.raced,
        "a tentativa tem de ter largado antes do reinício valer"
    );

    // RESTART: o relógio volta ao zero e as voltas somem.
    m.observe(&frame_de_corrida(0.4, 0));

    assert!(
        m.current_attempt > abandonada,
        "o reinício tem de abrir uma tentativa nova"
    );
    let morta = m
        .attempts
        .iter()
        .find(|a| a.number == abandonada)
        .expect("a tentativa abandonada");
    assert_eq!(morta.ended_by, Some(FimDaTentativa::Restart));
    assert_ne!(morta.status, StatusTentativa::Active);
    let viva = m.attempts.last().expect("tentativa nova");
    assert_eq!(viva.status, StatusTentativa::Active);
    assert_eq!(viva.laps_completed, 0);
    assert!(!viva.evidence.raced || viva.evidence.incident_points == 0);
}

/// Rebobinar o replay para a largada leva o `SessionTime` de volta ao zero exatamente como um
/// reinício — e com o replay PAUSADO o `is_replay_playing` cai a zero junto. O que separa os
/// dois é o jogador estar dentro do carro: assistindo, a corrida em andamento não pode ser
/// jogada fora, e o `prev` fica congelado até ele voltar a pilotar.
#[test]
fn rebobinar_o_replay_nao_e_reinicio() {
    let mut m = monitor_em_corrida_disputada();
    let disputada = m.current_attempt;

    // Replay pausado na largada: o relógio volta ao zero, o jogador está fora do carro.
    let mut assistindo = frame_de_corrida(0.4, 0);
    assistindo.on_track = false;
    m.observe(&assistindo);
    assert_eq!(
        m.current_attempt, disputada,
        "assistir ao replay não pode descartar a corrida"
    );
    assert_eq!(m.attempts.last().unwrap().status, StatusTentativa::Active);

    // De volta ao carro, ao vivo, com a corrida onde ela parou.
    m.observe(&frame_de_corrida(725.0, 12));
    assert_eq!(m.current_attempt, disputada);

    // E o reinício de verdade, depois disso, segue sendo pego.
    m.observe(&frame_de_corrida(0.4, 0));
    assert!(m.current_attempt > disputada);
}

/// Nada do que a tentativa abandonada acumulou pode sobreviver ao reinício: é isso que ia
/// parar na notícia, no resumo, no histórico e na carreira.
#[test]
fn tentativa_abandonada_nao_deixa_rastro_no_reinicio() {
    let mut m = monitor_em_corrida_disputada();

    // O que aconteceu na tentativa que o jogador jogou fora.
    m.events.push(RaceEvent {
        session_time: 300.0,
        lap: 5,
        kind: "dnf_confirmed".to_string(),
        car_idx: Some(1),
        detail: "Carro 1 abandonou".to_string(),
        severity: Some("grave".to_string()),
    });
    m.breakdown_log.push(BreakdownOutcome {
        car_number: 12,
        part: "engine".to_string(),
        problem: 0,
        lap: 6,
        severity: crate::car::breakdown::Severity::Dnf,
        penalty_secs: None,
        forced: true,
        label: "Motor".to_string(),
    });
    m.player_pit_laps.push(4);
    m.pending_breakdown_cmds.push("!black #12 15".to_string());
    m.breakdown_repair_laps.push((3, 5));
    m.breakdown_flash_at[3] = 690.0;
    m.breakdown_alert[3] = Some(BreakdownAlert {
        severity: crate::car::breakdown::Severity::Heavy,
        entered_pit_since: false,
    });
    m.player_incidents.push(PlayerIncidentMark {
        lap_f: 5.3,
        points: 4,
        off_track: true,
    });

    m.observe(&frame_de_corrida(0.4, 0));

    // O log de eventos nasce com o marcador do reinício e mais nada — em especial, sem o
    // abandono da IA, que o import lê para marcar DNF e incidente notável.
    assert_eq!(
        m.events.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
        vec!["race_restarted"],
        "o log de eventos é da tentativa"
    );
    assert!(m.breakdown_log.is_empty(), "quebra da tentativa morta");
    assert!(m.player_pit_laps.is_empty(), "pit da tentativa morta");
    assert!(
        m.pending_breakdown_cmds.is_empty(),
        "um !black enfileirado lá puniria um carro nesta corrida"
    );
    assert!(m.breakdown_repair_laps.is_empty());
    assert_eq!(m.breakdown_flash_at[3], 0.0);
    assert!(m.breakdown_alert[3].is_none());
    assert!(m.player_incidents.is_empty());
    // E o histórico volta a volta cobre a tentativa nova, não a que morreu.
    assert_eq!(m.history.attempt_number, m.current_attempt);
    assert!(m.history.player_laps.is_empty());
}

/// O desgaste que as voltas da tentativa abandonada consumiram e as peças que largaram nela
/// são de uma corrida que não aconteceu — e viram consequência de carreira. O diretor volta
/// ao estado em que foi instalado.
#[test]
fn reinicio_devolve_o_diretor_de_quebra_ao_estado_de_instalacao() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;
    m.prev_session_num = 2;
    m.history.player_car_idx = 0;
    m.car_number[0] = 7;

    let mut car = crate::car::Car::uniform(3);
    car.set_wear(crate::car::PartType::Engine, 1.25); // além da PAREDE → falha forçada
    let live = crate::car::breakdown::LiveBreakdown::new(&car, 42, 50.0, (1.0, 1.0, 1.0));
    let mut dir = crate::car::breakdown::BreakdownDirector::new();
    dir.add_car(7, live, Vec::new());
    m.install_breakdown_director(dir, None, crate::car::breakdown::Weather::NEUTRAL, false);
    m.breakdown_needs_prime = false;

    // Tentativa 1: a carência passa e o motor larga.
    m.observe(&frame_de_corrida(100.0, 8));
    m.observe(&frame_de_corrida(100.0 + BREAKDOWN_GRACE_SECS + 60.0, 9));
    assert!(
        !m.breakdown_log.is_empty(),
        "a peça tinha de largar na tentativa abandonada"
    );

    // RESTART.
    m.observe(&frame_de_corrida(0.4, 0));
    assert!(m.breakdown_log.is_empty());

    // Tentativa 2: o motor volta a poder largar — o diretor não guardou a quebra da
    // tentativa morta nem o desgaste das voltas dela.
    m.observe(&frame_de_corrida(BREAKDOWN_GRACE_SECS + 60.0, 3));
    assert!(
        !m.breakdown_log.is_empty(),
        "o diretor tem de voltar ao estado de instalação"
    );
}
