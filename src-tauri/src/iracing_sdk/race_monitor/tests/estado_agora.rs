//! Testes de [`super::super::estado_agora`]: as contas que a fala do engenheiro consome —
//! gap, voltas restantes, bandeira, combustível e idade do pneu.

use super::super::*;
use super::comum::*;

// ── EstadoAgora: as contas que a fala consome ────────────────────────────

#[test]
fn estado_agora_do_lider_nao_inventa_carro_a_frente() {
    // Mesmo sentinela, os outros dois lugares que fazem a busca: a amostra de gap
    // (de onde sai a tendência) e a vizinhança que a fala consome.
    let mut m = RaceMonitor::new();
    let t = frame_de_lider_com_sentinela();

    m.guardar_estado_agora(&t);

    let a = m.gap_hist.last().expect("amostra de gap");
    assert_eq!(a.idx_frente, -1, "position 0 não vira vizinho da frente");
    assert!(a.gap_frente < 0.0, "gap fabricado, veio {}", a.gap_frente);
    assert_eq!(a.idx_atras, 2);

    let e = m.montar_estado_agora();
    assert!(
        e.frente.is_none(),
        "o líder não tem ninguém à frente para o engenheiro comentar"
    );
    assert!(e.atras.is_some(), "o segundo colocado tem de aparecer");
}

#[test]
fn gap_fecha_o_circulo_quando_o_da_frente_ja_cruzou_a_linha() {
    // Eu a 95% de uma volta de 92 s (est 87,4); ele a 2% (est 1,8) — ele acabou de cruzar.
    // A subtração crua daria -85,6; o gap real é ~6,4 s.
    let g = gap_circular(87.4, 1.8, 92.0);
    assert!((g - 6.4).abs() < 0.01, "esperava ~6,4 s, veio {g}");
}

#[test]
fn gap_sem_volta_de_referencia_admite_que_nao_sabe() {
    // Sem tempo de volta não há círculo a fechar, e um número aqui seria invenção.
    assert_eq!(gap_circular(10.0, 4.0, 0.0), -1.0);
    assert_eq!(gap_circular(f64::NAN, 4.0, 92.0), -1.0);
}

#[test]
fn voltas_restantes_por_voltas_vem_do_sim_e_por_tempo_e_estimativa() {
    // Prova por VOLTAS: o sim responde, e a resposta não é estimativa.
    assert_eq!(voltas_restantes(12, -1.0, 92.0), (12, false));
    // Prova por TEMPO: o sim manda o sentinela e a conta é nossa — 500 s a 92 s/volta.
    let (n, estimada) = voltas_restantes(32767, 500.0, 92.0);
    assert_eq!(
        (n, estimada),
        (6, true),
        "sentinela tem de cair na estimativa"
    );
}

#[test]
fn sentinela_de_voltas_nunca_vaza_como_numero() {
    // O defeito que esta constante existe para impedir: anunciar "faltam 32767 voltas".
    // Sem ritmo nem tempo não há como estimar, e o certo é dizer que não se sabe.
    assert_eq!(voltas_restantes(32767, 0.0, 0.0), (-1, false));
}

/// O retrato que segura o cabeçalho da torre em prova por TEMPO, onde não existe total
/// previsto para limitar a conta.
#[test]
fn a_bandeirada_congela_a_volta_do_lider() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;

    // Corrida rolando na última volta: o líder fechou 7.
    m.observe(&race_frame(2, 7));
    assert_eq!(
        m.volta_final_lider, 0,
        "corrida em andamento não congela nada"
    );

    // O líder cruza a linha final: 8 completas, e a bandeirada cai no mesmo frame.
    let mut bandeirada = race_frame(2, 8);
    bandeirada.session_state = STATE_CHECKERED;
    m.observe(&bandeirada);
    assert_eq!(m.volta_final_lider, 8);

    // Cool down com o pelotão ainda girando: `lap_completed` sobe e o retrato não.
    let mut cool_down = race_frame(2, 9);
    cool_down.session_state = STATE_CHECKERED + 1;
    m.observe(&cool_down);
    assert_eq!(
        m.volta_final_lider, 8,
        "o congelamento é do instante da bandeirada, não do último frame visto"
    );
}

/// Treino e classificatória também chegam a `STATE_CHECKERED`, e nenhum dos dois tem
/// volta final de corrida para congelar.
#[test]
fn bandeirada_fora_da_corrida_nao_congela_volta() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;

    let mut quali = race_frame(1, 4);
    quali.session_state = STATE_CHECKERED;
    m.observe(&quali);

    assert_eq!(m.volta_final_lider, 0);
}

#[test]
fn total_da_prova_nao_entrega_o_sentinela_ao_engenheiro() {
    // Prova por TEMPO: `SessionLapsTotal` vem sentinelado e o contrato do campo é 0.
    // Sem isto o contexto do rádio dizia "Volta: 12 de 32767".
    assert_eq!(volta_e_total(12, 32767), (12, 0));
    // Prova por VOLTAS: o total é o número da prova mesmo.
    assert_eq!(volta_e_total(7, 8), (7, 8));
}

#[test]
fn volta_do_engenheiro_para_no_fim_da_prova() {
    // Última volta fechada: o jogador completou as 8 de uma prova de 8.
    assert_eq!(volta_e_total(8, 8), (8, 8));
    // Cool down: o jogador segue girando e o `lap_completed` sobe com a corrida acabada.
    // A prova não tem uma nona volta para o rádio anunciar.
    assert_eq!(volta_e_total(9, 8), (8, 8));
    // Sem total conhecido não há teto a aplicar, e o número segue como veio.
    assert_eq!(volta_e_total(9, 0), (9, 0));
    // Antes da primeira volta o SDK pode mandar -1; volta negativa não existe.
    assert_eq!(volta_e_total(-1, 8), (0, 8));
}

#[test]
fn alcance_so_existe_quando_a_distancia_encurta() {
    // 8 s de gap, meio segundo por volta mais rápido → 16 voltas.
    let v = voltas_para_alcancar(8.0, -0.5);
    assert!((v - 16.0).abs() < 0.01, "esperava 16 voltas, veio {v}");
    // Mais lento que ele: o alcance nunca acontece. -1, não um número gigante.
    assert_eq!(voltas_para_alcancar(8.0, 0.3), -1.0);
    // Ritmo idêntico: idem — a disputa está congelada.
    assert_eq!(voltas_para_alcancar(8.0, 0.0), -1.0);
}

#[test]
fn tendencia_exige_janela_de_observacao() {
    // Duas amostras a 3 s de distância: dentro do ruído de uma freada, não é tendência.
    let curta = [(100.0, 2.0), (103.0, 1.4)];
    assert_eq!(tendencia_por_volta(&curta, 92.0), 0.0);
    // 20 s de janela, gap caindo de 2,0 para 1,0 → -0,05 s/s × 92 s = -4,6 s por volta.
    let longa = [(100.0, 2.0), (120.0, 1.0)];
    let t = tendencia_por_volta(&longa, 92.0);
    assert!(
        t < 0.0,
        "gap encurtando tem de dar tendência negativa, veio {t}"
    );
    assert!((t + 4.6).abs() < 0.01, "esperava ~-4,6, veio {t}");
}

#[test]
fn bandeira_anuncia_a_mais_grave_e_nao_a_primeira() {
    // A PRETA É INVISÍVEL para este rótulo: no Loop ela é o mecanismo de quebra de peça,
    // não punição. E, por ser a segunda na ordem de gravidade, ela ESCONDIA a amarela de
    // verdade que estivesse ativa junto — que é o caso testado aqui.
    assert_eq!(
        rotulo_bandeira(FLAG_BLACK | FLAG_CAUTION),
        "Bandeira amarela"
    );
    assert_eq!(rotulo_bandeira(FLAG_BLACK), "");
    // A desclassificação continua acima de tudo: essa é real.
    assert_eq!(
        rotulo_bandeira(FLAG_DISQUALIFY | FLAG_BLACK),
        "Desclassificado"
    );
    assert_eq!(rotulo_bandeira(FLAG_YELLOW), "Bandeira amarela");
    assert_eq!(rotulo_bandeira(FLAG_CAUTION_WAVING), "Bandeira amarela");
    // Nada digno de nota devolve vazio — não "Verde", que viraria fala sem motivo.
    assert_eq!(rotulo_bandeira(0), "");
}

#[test]
fn saldo_de_combustivel_se_recusa_a_responder_pela_metade() {
    // 14 voltas de autonomia, 12 restantes → sobram 2.
    assert!((saldo_combustivel(14.0, 12) - 2.0).abs() < 0.001);
    // Falta combustível: negativo, e é a informação que faz parar no box.
    assert!(saldo_combustivel(9.0, 12) < 0.0);
    // Sem voltas restantes conhecidas, um saldo seria número com cara de resposta.
    assert!(saldo_combustivel(14.0, -1).is_nan());
    assert!(saldo_combustivel(-1.0, 12).is_nan());
}

#[test]
fn saldo_desconhecido_nao_se_confunde_com_faltar_uma_volta() {
    // O motivo de este campo usar NaN e não -1 como os vizinhos: `-1` é uma RESPOSTA aqui
    // ("falta combustível para uma volta"), e é a resposta que manda o piloto ao box.
    // Com sentinela numérico, ela seria indistinguível de "não sei" — e o dossiê calaria
    // exatamente o fato mais urgente da corrida.
    let falta_uma = saldo_combustivel(11.0, 12);
    assert!(
        (falta_uma + 1.0).abs() < 0.001,
        "esperava -1,0, veio {falta_uma}"
    );
    assert!(
        falta_uma.is_finite(),
        "faltar uma volta é resposta, não ausência"
    );
    assert!(saldo_combustivel(-1.0, 12).is_nan(), "ausência é NaN");
}

#[test]
fn idade_do_pneu_conta_da_ultima_parada_com_troca() {
    use crate::iracing_sdk::tire_strategy::PitStop;
    let parada = |car_idx, lap, secs| PitStop {
        car_idx,
        lap,
        stationary_secs: secs,
        track_wet_at_stop: false,
    };
    // Duas paradas: a da volta 6 só abasteceu (18 s, abaixo do bloco de pneu), a da volta
    // 12 trocou (23 s). Na volta 20, o pneu tem 8 voltas — não 14.
    let paradas = [parada(7, 6, 18.0), parada(7, 12, 23.0)];
    assert_eq!(idade_do_pneu(&paradas, 7, 20), 8);

    // Parada de OUTRO carro não conta. Sem troca própria, o pneu é o da largada e a idade
    // é a corrida inteira — que é a resposta certa, não uma ausência.
    let paradas = [parada(3, 12, 23.0)];
    assert_eq!(idade_do_pneu(&paradas, 7, 20), 20);
    assert_eq!(idade_do_pneu(&[], 7, 20), 20);
}

#[test]
fn abastecimento_longo_nao_e_confundido_com_troca_de_pneu() {
    use crate::iracing_sdk::tire_strategy::PitStop;
    // O caso que a constante de 20 s existe para separar: o maior abastecimento sozinho
    // medido foi ~19 s, e o serviço mínimo de pneu ~21 s. Uma parada de 19,5 s NÃO trocou
    // pneu, e tratá-la como troca zeraria a idade — o engenheiro anunciaria pneu novo num
    // carro que está com o mesmo jogo desde a largada.
    let quase = [PitStop {
        car_idx: 7,
        lap: 12,
        stationary_secs: 19.5,
        track_wet_at_stop: false,
    }];
    assert_eq!(
        idade_do_pneu(&quase, 7, 20),
        20,
        "abastecimento virou troca"
    );
}
