//! Testes de [`super::super::pontuacao`]: o que o monitor aceita como batida do jogador, e
//! o que ele se recusa a contar.

use super::super::*;
use super::comum::*;

// ── Dano do jogador: o que o monitor aceita como batida ──────────────────────

/// Uma RODADA sem tocar em nada: pontos de incidente, guinada, rotação e fora da
/// pista somam bem acima de "moderado" no scorer — mas o carro não bateu. O pico, que
/// é a base do conserto, não pode registrar nada.
#[test]
fn rodada_limpa_nao_alimenta_o_pico_de_batida() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;

    // Tick de referência (estabelece o `prev_incident`).
    m.observe(&frame_do_jogador(2, 0, SURFACE_ON_TRACK));

    // Roda: +2 de incidente (perda de controle), guinada e rotação violentas, fora da
    // pista. Nenhuma aceleração de impacto — não houve contato.
    let mut rodada = frame_do_jogador(2, 2, SURFACE_OFF_TRACK);
    rodada.yaw_rate = 5.0;
    rodada.roll_rate = 5.0;
    rodada.speed_ms = 10.0;
    m.observe(&rodada);

    assert!(m.in_crash, "o scorer ainda vê o evento (bandeira, DNF)");
    let a = m.attempts.last().expect("tentativa ativa");
    assert_eq!(
        a.peak_crash_score, 0.0,
        "sem impacto não há dano: o pico é a base do conserto"
    );
    assert!(a.peak_impact_dir.is_none());

    // Fecha a batida (silêncio além da janela de fusão) e confere a marca do evento.
    let mut depois = frame_do_jogador(2, 2, SURFACE_ON_TRACK);
    depois.session_time = 100.0 + MERGE_WINDOW_SECS + 1.0;
    m.observe(&depois);
    let crash = m.attempts.last().unwrap().crashes.first().expect("evento");
    assert!(crash.score >= SEV_MODERATE, "o evento pontuou alto");
    assert!(!crash.had_impact, "mas sem impacto — não vira conserto");
}

/// DUPLA CONFIRMAÇÃO: o sim passando a pedir reparo, com uma batida em curso, confirma o
/// impacto que o G não pegou (toque de baixa energia que quebra a asa).
#[test]
fn reparo_pedido_pelo_sim_confirma_o_impacto() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;
    m.observe(&frame_do_jogador(2, 0, SURFACE_ON_TRACK));

    // Abre a janela de batida sem nenhum G: incidente, guinada, rotação, fora da pista.
    let mut rodada = frame_do_jogador(2, 2, SURFACE_OFF_TRACK);
    rodada.yaw_rate = 5.0;
    rodada.roll_rate = 5.0;
    m.observe(&rodada);
    assert_eq!(m.attempts.last().unwrap().peak_crash_score, 0.0);

    // O sim passa a pedir reparo: o carro quebrou de verdade.
    let mut quebrou = frame_do_jogador(2, 2, SURFACE_OFF_TRACK);
    quebrou.pit_opt_repair_needed = 12.0;
    m.observe(&quebrou);

    let a = m.attempts.last().expect("tentativa ativa");
    assert!(a.peak_crash_score >= SEV_MODERATE, "o dano passa a contar");
    assert_eq!(a.sim_repair_needed_s, 12.0);
    assert!(
        a.peak_impact_dir.is_none(),
        "sem G não há eixo dominante: a direção não pode ser inventada"
    );
}

/// A ausência dos canais de reparo não conclui nada: o carro que bateu com G continua
/// contando mesmo com o sim calado.
#[test]
fn silencio_do_reparo_nao_cancela_a_batida() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;
    m.observe(&frame_do_jogador(2, 0, SURFACE_ON_TRACK));

    let mut pancada = frame_do_jogador(2, 4, SURFACE_ON_TRACK);
    pancada.long_accel = -60.0; // e nenhum reparo reportado
    m.observe(&pancada);

    let a = m.attempts.last().expect("tentativa ativa");
    assert!(a.peak_crash_score > 0.0);
    assert_eq!(a.sim_repair_needed_s, 0.0);
}

/// O PICO é sempre um piso: a velocidade PERDIDA na pancada, que é o componente que separa
/// o encostão da sucata (vale até 160 pontos), só é calculada quando a batida FECHA. Ler só
/// o pico chamava de "leve" um carro que morreu no muro.
#[test]
fn pior_batida_bruta_nao_se_contenta_com_o_pico() {
    let mut a = active_attempt();
    a.peak_crash_score = 46.0; // "leve" — G + contato, sem a velocidade perdida
    assert_eq!(worst_raw_severity(&a), Severidade::Leve);

    a.crashes.push(CrashEvent {
        session_time: 10.0,
        lap: 1,
        score: 200.0,
        severity: Severidade::Destruido,
        impact_severity: Severidade::Destruido,
        had_impact: true,
        factors: vec![],
    });
    assert_eq!(
        worst_raw_severity(&a),
        Severidade::Destruido,
        "a batida fechada tem a conta inteira e tem de vencer o pico"
    );
}
