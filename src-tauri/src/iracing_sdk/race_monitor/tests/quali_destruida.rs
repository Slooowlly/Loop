//! Testes da regra do CARRO DESTRUÍDO NA CLASSIFICAÇÃO — o castigo que o Loop impõe por
//! comando de admin porque o iRacing devolve o carro inteiro para a corrida.
//!
//! Fica à parte de [`super::quali`] de propósito: aquele arquivo é sobre medir a
//! classificatória, este é sobre puni-la.

use super::super::*;
use super::comum::*;

// ── Carro destruído na CLASSIFICAÇÃO ─────────────────────────────────────────
/// Monitor com a regra armada e a quali (sessão 1) já vivida com o carro `num`: o jogador
/// bateu de verdade e o sim pediu `reparo_s` de conserto OBRIGATÓRIO.
fn monitor_apos_quali_com_numero(reparo_s: f64, armado: bool, num: i32) -> RaceMonitor {
    let mut m = RaceMonitor::new();
    m.quali_wreck_on = Some(armado);
    m.qualy_session_num = 1;
    m.race_session_num = 2;
    m.history.player_car_idx = 0;
    m.car_number[0] = num;

    m.observe(&frame_do_jogador(1, 0, SURFACE_ON_TRACK));
    let mut destruiu = frame_do_jogador(1, 4, SURFACE_ON_TRACK);
    destruiu.long_accel = -60.0; // pancada de verdade
    destruiu.pit_repair_needed = reparo_s;
    m.observe(&destruiu);
    m
}

fn monitor_apos_quali(reparo_s: f64, armado: bool) -> RaceMonitor {
    monitor_apos_quali_com_numero(reparo_s, armado, 64)
}

/// Primeiro tick da CORRIDA, ainda na formação (antes do verde).
fn frame_de_formacao() -> IracingTelemetry {
    let mut f = frame_do_jogador(2, 0, SURFACE_ON_TRACK);
    f.session_state = STATE_RACING - 1;
    f
}

/// Batida "grave" (aqui via piso do reparo): a classificação é dada por encerrada NO RÁDIO,
/// na hora, e a corrida sai do fundo pelo `!eol`. Nada de `!dq` aqui — ele é terminal no
/// iRacing e apagaria o `!eol`, que é justamente o castigo desta faixa.
#[test]
fn batida_grave_encerra_a_quali_no_radio_e_larga_do_fundo() {
    let mut m = monitor_apos_quali(40.0, true);
    // O veredito saiu DENTRO da quali, no instante da batida — mas só como fala.
    assert!(
        m.pending_breakdown_cmds.is_empty(),
        "um !dq aqui desqualificaria o fim de semana inteiro: {:?}",
        m.pending_breakdown_cmds
    );
    let aviso = m
        .player_warning_log
        .last()
        .expect("motivo no rádio, na hora");
    assert!(matches!(aviso.tipo, TipoAvisoProprio::QualiDestruida));
    assert_eq!(aviso.severidade, "quali_grave");

    // Na corrida, o castigo de verdade — e o rádio repete o motivo, porque é ali que o
    // jogador vê o bom tempo dele virar último lugar.
    m.observe(&frame_de_formacao());
    assert_eq!(m.pending_breakdown_cmds, vec!["!eol #64".to_string()]);
    assert_eq!(m.player_warning_log.last().unwrap().severidade, "eol");
    // E não repete no tick seguinte: o castigo é um só.
    m.observe(&frame_de_formacao());
    assert_eq!(m.pending_breakdown_cmds.len(), 1);
}

/// A batida PIORA enquanto o veredito já saiu: o `!dq` tem de chegar ainda na classificação.
///
/// O pior desfecho possível, medido em 2026-08-11: o primeiro instante em que a pontuação
/// cruzou "grave" congelava a decisão em `eol`, a mesma batida terminava em "destruído", e o
/// `!dq` só saía na virada de sessão — com o jogador já no grid, dirigindo um carro que
/// funciona, obrigado a abandonar uma corrida começada. Quem é desqualificado tem de ser
/// impedido de ENTRAR no grid, e é isto que o teste trava.
#[test]
fn a_batida_que_piora_desqualifica_ainda_na_quali() {
    let mut m = monitor_apos_quali(40.0, true); // primeiro veredito: "grave" → eol, sem comando
    assert!(m.pending_breakdown_cmds.is_empty());
    assert_eq!(m.quali_wreck_pending, Some(CastigoDaQuali::Eol));

    // O carro piora ANTES de a sessão virar — segue na quali, mesmo número de sessão.
    m.attempts.last_mut().unwrap().peak_crash_score = 180.0; // "destruído"
    m.observe(&frame_do_jogador(1, 4, SURFACE_ON_TRACK));

    assert_eq!(
        m.pending_breakdown_cmds,
        vec!["!dq #64".to_string()],
        "o !dq tem de sair DENTRO da classificação, não na largada"
    );
    assert_eq!(m.quali_wreck_pending, Some(CastigoDaQuali::Dq));
    assert_eq!(
        m.player_warning_log.last().unwrap().severidade,
        "quali_destruido",
        "o rádio tem de dizer que o fim de semana acabou, não que larga do fundo"
    );
}

/// A promoção fala UMA vez por faixa: ticks seguintes com o mesmo posto não repetem nada.
#[test]
fn o_veredito_ao_vivo_nao_se_repete_na_mesma_faixa() {
    let mut m = monitor_apos_quali(40.0, true);
    let falas = m.player_warning_log.len();
    for _ in 0..5 {
        m.observe(&frame_do_jogador(1, 4, SURFACE_ON_TRACK));
    }
    assert_eq!(m.player_warning_log.len(), falas, "uma fala por faixa");
    assert!(m.pending_breakdown_cmds.is_empty());
}

/// Batida "destruído": DQ na quali E na corrida (reafirmado). Aqui o `!dq` ser terminal é o
/// que se QUER — o carro não corre mesmo, e não há o que limpar depois.
#[test]
fn carro_irrecuperavel_nao_corre_o_fim_de_semana() {
    let mut m = monitor_apos_quali(80.0, true);
    assert_eq!(m.pending_breakdown_cmds, vec!["!dq #64".to_string()]);
    assert_eq!(
        m.player_warning_log.last().unwrap().severidade,
        "quali_destruido"
    );

    // A fila da corrida é nova (o `!dq` da quali já foi drenado ao vivo); o DQ é reafirmado.
    m.observe(&frame_de_formacao());
    assert_eq!(m.pending_breakdown_cmds, vec!["!dq #64".to_string()]);
    assert_eq!(m.player_warning_log.last().unwrap().severidade, "dq");
}

/// A severidade decide sozinha quando os outros canais ficam mudos — e "catastrófico" só
/// muda a fala do rádio, não a consequência (DQ nos dois casos).
#[test]
fn severidade_castiga_mesmo_com_o_canal_de_reparo_mudo() {
    // Pico mutado DEPOIS da quali → o lockout ao vivo não viu; a fronteira pega.
    let mut m = monitor_apos_quali(0.0, true);
    m.attempts.last_mut().unwrap().peak_crash_score = 180.0; // "destruído"
    m.observe(&frame_de_formacao());
    assert_eq!(m.pending_breakdown_cmds, vec!["!dq #64".to_string()]);

    let mut m = monitor_apos_quali(0.0, true);
    m.attempts.last_mut().unwrap().peak_crash_score = 240.0; // "catastrófico"
    m.observe(&frame_de_formacao());
    assert_eq!(m.pending_breakdown_cmds, vec!["!dq #64".to_string()]);
}

/// O rádio do lockout gradua pela severidade: "catastrófico" pergunta se o piloto está
/// inteiro em vez de falar de conserto.
#[test]
fn catastrofico_muda_a_fala_do_lockout() {
    let mut m = monitor_apos_quali_com_numero(0.0, true, 64);
    m.attempts.last_mut().unwrap().peak_crash_score = 240.0;
    // Mais um tick de quali para o lockout ao vivo avaliar o pico já alto.
    m.observe(&frame_do_jogador(1, 4, SURFACE_ON_TRACK));
    assert_eq!(
        m.player_warning_log.last().unwrap().severidade,
        "quali_catastrofico"
    );
}

/// O MEATBALL é piso de "grave": o sim declarou reparo obrigatório, e isso encerra a quali
/// mesmo quando o score fica curto (pista molhada e G subamostrado marcaram "grave" num
/// carro sem roda — caso medido em 2026-08-10).
#[test]
fn meatball_na_quali_encerra_mesmo_com_score_curto() {
    let mut m = monitor_apos_quali(0.0, true);
    let mut meatball = frame_do_jogador(1, 4, SURFACE_ON_TRACK);
    meatball.session_flags = 0x0010_0000; // FLAG_REPAIR
    m.observe(&meatball);
    // Veredito ao vivo, na quali mesmo — no rádio, sem comando ao sim.
    assert!(m.pending_breakdown_cmds.is_empty());
    assert_eq!(
        m.player_warning_log.last().unwrap().severidade,
        "quali_grave"
    );

    m.observe(&frame_de_formacao());
    assert_eq!(m.pending_breakdown_cmds, vec!["!eol #64".to_string()]);
}

/// Se o carro PIOROU depois do veredito (o pico subiu a "destruído" depois de a quali já ter
/// sido dada por encerrada), a fronteira PROMOVE a pendência: eol vira dq, nunca o contrário.
#[test]
fn piorar_o_carro_depois_do_veredito_promove_o_castigo() {
    let mut m = monitor_apos_quali(0.0, true);
    let mut meatball = frame_do_jogador(1, 4, SURFACE_ON_TRACK);
    meatball.session_flags = 0x0010_0000;
    m.observe(&meatball); // veredito "grave" → pendência eol
    m.attempts.last_mut().unwrap().peak_crash_score = 180.0; // piorou: "destruído"

    m.observe(&frame_de_formacao());
    assert_eq!(
        m.pending_breakdown_cmds,
        vec!["!dq #64".to_string()],
        "a corrida tem de sair como DQ"
    );
}

/// O veredito tem de sair NA HORA, com a batida ainda aberta. A velocidade perdida é o
/// componente que separa o encostão da destruição, e só o FECHAMENTO da batida a gravava —
/// esperar dez segundos de silêncio numa quali destruída é esperar por algo que pode nunca
/// vir. Medido em 2026-08-10: a fronteira dizia "grave" e o rádio nunca saía.
#[test]
fn o_veredito_nao_espera_a_batida_fechar() {
    let mut m = RaceMonitor::new();
    m.quali_wreck_on = Some(true);
    m.qualy_session_num = 1;
    m.race_session_num = 2;
    m.history.player_car_idx = 0;
    m.car_number[0] = 64;

    // Rodando a 60 m/s; a batida ainda não existe.
    let mut rodando = frame_do_jogador(1, 0, SURFACE_ON_TRACK);
    rodando.speed_ms = 60.0;
    m.observe(&rodando);
    assert!(m.player_warning_log.is_empty());

    // Muro: contato do sim + G, e o carro para. A batida NÃO fechou (nada de esperar a
    // janela de fusão), mas os 60 m/s perdidos já valem sozinhos mais de "grave".
    let mut muro = frame_do_jogador(1, 4, SURFACE_ON_TRACK);
    muro.long_accel = -60.0;
    muro.speed_ms = 0.0;
    m.observe(&muro);

    assert!(m.in_crash, "a batida segue ABERTA — é esse o cenário");
    let aviso = m
        .player_warning_log
        .last()
        .expect("o veredito tem de sair com a batida ainda aberta");
    assert!(matches!(aviso.tipo, TipoAvisoProprio::QualiDestruida));
    assert!(m.quali_wreck_pending.is_some(), "a corrida tem de cobrar");
}

/// O rádio emudecia PARA SEMPRE depois de um reinício: o `id` da fala é a posição no log, os
/// logs são esvaziados a cada tentativa, e o overlay só mostra id INÉDITO — então tudo que
/// vinha depois do primeiro reinício era descartado como "já vi essa". Sem erro em lugar
/// nenhum. Medido em 2026-08-10, com o jogador reiniciando a quali várias vezes.
#[test]
fn os_ids_do_radio_nao_voltam_atras_depois_de_um_reinicio() {
    let mut m = RaceMonitor::new();
    m.player_warning_log.push(PlayerWarning {
        tipo: TipoAvisoProprio::Poupar,
        part: "",
        wear_pct: 0,
        severidade: "",
    });
    m.ritmo_log.push(FalaDeRitmo::Tomamos("x".to_string()));
    let ultimo_id_antes = m.radio_epoch + m.player_warning_log.len() - 1;

    m.start_attempt(0.0); // reinício: os logs vão embora

    assert!(
        m.radio_epoch > ultimo_id_antes,
        "o id da PRÓXIMA fala ({}) tem de superar o da última já vista ({ultimo_id_antes})",
        m.radio_epoch
    );
}

/// O limiar é alto de propósito: batida que o box conserta não trava a quali nem custa a
/// etapa — o jogador pode resetar e tentar de novo.
#[test]
fn batida_pequena_na_quali_nao_castiga() {
    let mut m = monitor_apos_quali(8.0, true);
    m.observe(&frame_de_formacao());

    assert!(m.pending_breakdown_cmds.is_empty());
    assert!(m.player_warning_log.is_empty());
}

/// A regra inteira está atrás de flag até os comandos serem confirmados na pista.
#[test]
fn regra_desarmada_nao_castiga_ninguem() {
    let mut m = monitor_apos_quali(80.0, false);
    m.observe(&frame_de_formacao());

    assert!(m.pending_breakdown_cmds.is_empty());
    assert!(m.player_warning_log.is_empty());
    // Mas a tentativa da quali segue identificada, porque o conserto dela é cobrado
    // no import independentemente do castigo esportivo.
    assert!(m.quali_attempt_number > 0);
}

/// Sem o número do carro o lockout adia (sem perder a pendência: a fronteira reavalia), e
/// se a largada vier antes de o YAML entregar o número, o castigo cai na bandeira preta.
#[test]
fn castigo_perdido_na_formacao_vira_bandeira_preta() {
    let mut m = monitor_apos_quali_com_numero(40.0, true, 0); // número desconhecido
    assert!(
        m.pending_breakdown_cmds.is_empty(),
        "sem número não há lockout ao vivo"
    );
    m.observe(&frame_de_formacao());
    assert!(m.pending_breakdown_cmds.is_empty(), "sem número, não manda");

    m.car_number[0] = 64;
    m.observe(&frame_do_jogador(2, 0, SURFACE_ON_TRACK)); // já em Racing
    assert_eq!(m.pending_breakdown_cmds, vec!["!black #64 15".to_string()]);
}

// ── O REGISTRO da calibração (A13.2 da vistoria) ─────────────────────────────
//
// Os cortes que decidem "grave", "destruído" e DQ são decisão de PRODUTO: é onde se
// escolheu que a punição passa a doer. Não há medição por trás deles e não havia nada
// escrito que os fixasse — mexer em `SEV_SEVERE` mudava, em silêncio, quem larga do fundo.
//
// Estes testes não CALIBRAM nada; eles REGISTRAM. Quem quiser mudar a régua muda, e o
// diff mostra que a régua mudou em vez de o comportamento mudar sozinho.

/// A escada de severidade, ponto a ponto, incluindo os dois lados de cada corte.
#[test]
fn a_regua_de_severidade_esta_registrada_ponto_a_ponto() {
    let casos = [
        (0.0, Severidade::Nenhum),
        (SEV_MINOR - 0.1, Severidade::Nenhum),
        (SEV_MINOR, Severidade::Leve),
        (SEV_MODERATE - 0.1, Severidade::Leve),
        (SEV_MODERATE, Severidade::Moderado),
        (SEV_SEVERE - 0.1, Severidade::Moderado),
        (SEV_SEVERE, Severidade::Grave),
        (SEV_TOTALED - 0.1, Severidade::Grave),
        (SEV_TOTALED, Severidade::Destruido),
        (SEV_CATASTROPHIC - 0.1, Severidade::Destruido),
        (SEV_CATASTROPHIC, Severidade::Catastrofico),
    ];
    for (score, esperada) in casos {
        assert_eq!(
            severity_label(score),
            esperada,
            "a régua mudou no ponto {score}"
        );
    }
}

/// Onde cada castigo COMEÇA, em pontos de batida.
///
/// É a tradução da régua acima para o que o jogador sente: abaixo de [`SEV_SEVERE`] a
/// classificação vale; a partir dali ele larga do fundo; a partir de [`SEV_TOTALED`] ele
/// não larga. Os números são de produto e ficam aqui por escrito exatamente por isso.
#[test]
fn os_cortes_do_castigo_estao_registrados_em_pontos() {
    assert_eq!(
        severity_label(SEV_SEVERE),
        QUALI_WRECK_PENALTY_SEV,
        "o `!eol` começa em SEV_SEVERE"
    );
    assert_eq!(
        severity_label(SEV_TOTALED),
        QUALI_WRECK_DQ_SEV,
        "o `!dq` começa em SEV_TOTALED"
    );
    assert!(
        QUALI_WRECK_PENALTY_SEV < QUALI_WRECK_DQ_SEV,
        "o castigo terminal tem de ficar ACIMA do que só manda para o fundo"
    );
    assert!(
        severity_label(SEV_SEVERE - 0.1) < QUALI_WRECK_PENALTY_SEV,
        "um ponto abaixo do corte não pode castigar"
    );
}

/// O mesmo para os SEGUNDOS DE REPARO, que são o piso alternativo do castigo.
///
/// Estes dois números são os menos apoiados do conjunto: não há medição ligando 25 s de
/// conserto a "grave" nem 60 s a "destruído", e na prática eles quase nunca são o gatilho
/// — os canais `PitRepairLeft` são mudos fora do box (medido: carro destruído, meatball na
/// tela, 0.0 em todos os frames). O que este teste trava é a ORDEM e o efeito, para que uma
/// troca de valores não inverta as faixas sem ninguém perceber.
#[test]
fn os_segundos_de_reparo_que_castigam_estao_registrados() {
    assert!(
        QUALI_WRECK_PENALTY_S < QUALI_WRECK_DQ_S,
        "o piso do DQ tem de exigir MAIS conserto que o piso do fundo do grid"
    );

    // Logo abaixo do piso de "grave": a quali vale.
    let m = monitor_apos_quali(QUALI_WRECK_PENALTY_S - 1.0, true);
    assert!(m.quali_wreck_pending.is_none(), "castigou abaixo do piso");

    // No piso de "grave": larga do fundo.
    let mut m = monitor_apos_quali(QUALI_WRECK_PENALTY_S, true);
    m.observe(&frame_de_formacao());
    assert_eq!(m.pending_breakdown_cmds, vec!["!eol #64".to_string()]);

    // No piso de "destruído": não corre.
    let mut m = monitor_apos_quali(QUALI_WRECK_DQ_S, true);
    m.observe(&frame_de_formacao());
    assert_eq!(m.pending_breakdown_cmds, vec!["!dq #64".to_string()]);
}

/// A regra tem caminho por CONFIGURAÇÃO, e a variável de ambiente é só um atalho.
///
/// O item A13.2 ficou como parcial porque a env var continuava no caminho. Ela continua —
/// é o que o `testar-quali-destruida.cmd` usa para exercitar a regra na pista — mas o que
/// este teste fixa é que ela não é NECESSÁRIA: a preferência gravada arma e desarma
/// sozinha. Decidir se o atalho sai antes do release segue sendo decisão de produto.
#[test]
fn a_preferencia_arma_a_regra_sem_depender_do_ambiente() {
    // `quali_wreck_on` é a sobreposição local que os testes usam no lugar do estático
    // global; o caminho de produção equivalente é `set_quali_wreck_penalty`.
    let mut armado = monitor_apos_quali(QUALI_WRECK_DQ_S, true);
    armado.observe(&frame_de_formacao());
    assert_eq!(armado.pending_breakdown_cmds, vec!["!dq #64".to_string()]);

    let mut desarmado = monitor_apos_quali(QUALI_WRECK_DQ_S, false);
    desarmado.observe(&frame_de_formacao());
    assert!(
        desarmado.pending_breakdown_cmds.is_empty(),
        "com a regra desarmada nada pode ser enviado ao sim"
    );
}

// ── A prova de que o comando CHEGOU (A3.3 da vistoria) ───────────────────────
//
// Do lado do envio o furo é indetectável: com o sim em fullscreen exclusivo, o
// `SetForegroundWindow` e o `SendInput` devolvem sucesso e o comando não chega. O que estes
// testes travam é a conferência pelo outro lado — a bandeira que o comando deveria acender.

/// Frame de corrida com um conjunto de bandeiras do JOGADOR na tela.
fn frame_com_bandeiras(session_time: f64, bandeiras: u32) -> IracingTelemetry {
    let mut f = frame_do_jogador(2, 0, SURFACE_ON_TRACK);
    f.session_time = session_time;
    f.session_flags = bandeiras as i32;
    f
}

/// O `!dq` acende [`FLAG_DISQUALIFY`], e a conferência fecha satisfeita.
#[test]
fn a_bandeira_de_dq_confirma_que_o_comando_chegou() {
    let mut m = monitor_apos_quali(80.0, true); // "destruído" → !dq
    m.observe(&frame_de_formacao());
    assert_eq!(m.pending_breakdown_cmds, vec!["!dq #64".to_string()]);
    let prova = m
        .castigo_a_confirmar
        .clone()
        .expect("a conferência tem de ficar armada");
    assert_eq!(prova.prova, FLAG_DISQUALIFY);

    m.observe(&frame_com_bandeiras(
        prova.enviado_em_s + 3.0,
        FLAG_DISQUALIFY,
    ));
    assert!(
        m.castigo_a_confirmar.is_none(),
        "com a bandeira acesa não há mais o que conferir"
    );
}

/// Passada a janela sem bandeira nenhuma, a conferência desiste — e é justamente essa
/// desistência que vira a linha de diagnóstico que o A3.3 pedia. Sem ela, um comando que
/// evapora no `SendInput` não deixa rastro nenhum.
#[test]
fn a_janela_sem_bandeira_fecha_a_conferencia_em_vez_de_ficar_pendurada() {
    let mut m = monitor_apos_quali(80.0, true);
    m.observe(&frame_de_formacao());
    let enviado = m.castigo_a_confirmar.as_ref().unwrap().enviado_em_s;

    // Ainda dentro da janela: continua esperando, porque a fila drena um comando a cada 1,5 s.
    m.observe(&frame_com_bandeiras(enviado + 5.0, 0));
    assert!(m.castigo_a_confirmar.is_some(), "desistiu cedo demais");

    m.observe(&frame_com_bandeiras(
        enviado + JANELA_PROVA_DO_CASTIGO_S + 1.0,
        0,
    ));
    assert!(m.castigo_a_confirmar.is_none());
}

/// O `!eol` não acende bandeira: ele reordena o grid. Marcar isso como "não confirmado"
/// seria inventar uma falha — a conferência registra que não HÁ prova possível por este
/// canal, e a medição de verdade fica devendo uma captura da largada.
#[test]
fn o_eol_e_registrado_como_sem_prova_e_nao_como_falha() {
    let mut m = monitor_apos_quali(40.0, true); // "grave" → !eol
    m.observe(&frame_de_formacao());
    assert_eq!(m.pending_breakdown_cmds, vec!["!eol #64".to_string()]);
    let prova = m
        .castigo_a_confirmar
        .clone()
        .expect("armada mesmo sem prova possível");
    assert_eq!(prova.prova, 0, "o !eol não tem bandeira que o denuncie");

    m.observe(&frame_com_bandeiras(
        prova.enviado_em_s + JANELA_PROVA_DO_CASTIGO_S + 1.0,
        0,
    ));
    assert!(m.castigo_a_confirmar.is_none());
}

/// Reinício leva o relógio para trás. Sem a guarda, a janela nunca fecharia (a conta fica
/// negativa) e a conferência ficaria pendurada até o fim da sessão, calada.
#[test]
fn o_reinicio_nao_deixa_a_conferencia_pendurada() {
    let mut m = monitor_apos_quali(80.0, true);
    m.observe(&frame_de_formacao());
    let enviado = m.castigo_a_confirmar.as_ref().unwrap().enviado_em_s;

    m.observe(&frame_com_bandeiras(enviado - 10.0, 0));
    assert!(
        m.castigo_a_confirmar.is_none(),
        "castigo de antes do reinício não descreve mais nada"
    );
}

/// A bandeira preta do FALLBACK também é conferível — é o desfecho que o A3.3 mandou medir,
/// e o que dá para travar em teste é que ele nasce com prova associada em vez de mudo.
#[test]
fn a_bandeira_preta_do_fallback_nasce_conferivel() {
    let mut m = monitor_apos_quali_com_numero(40.0, true, 0);
    m.observe(&frame_de_formacao());
    m.car_number[0] = 64;
    m.observe(&frame_do_jogador(2, 0, SURFACE_ON_TRACK));
    assert_eq!(m.pending_breakdown_cmds, vec!["!black #64 15".to_string()]);

    let prova = m
        .castigo_a_confirmar
        .clone()
        .expect("o fallback também se confere");
    assert_eq!(prova.prova, FLAG_BLACK);
    m.observe(&frame_com_bandeiras(prova.enviado_em_s + 2.0, FLAG_BLACK));
    assert!(m.castigo_a_confirmar.is_none());
}

/// Contato de verdade (o 4x do próprio iRacing) segue virando dano.
#[test]
fn contato_de_verdade_alimenta_o_pico() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;
    m.observe(&frame_do_jogador(2, 0, SURFACE_ON_TRACK));

    let mut pancada = frame_do_jogador(2, 4, SURFACE_ON_TRACK);
    pancada.long_accel = -60.0; // freada violenta contra o muro
    m.observe(&pancada);

    let a = m.attempts.last().expect("tentativa ativa");
    assert!(a.peak_crash_score > 0.0, "contato tem de contar");
    assert_eq!(a.peak_impact_dir.as_deref(), Some("front"));
}

/// O iRacing troca de sessão na MESMA conexão. A tentativa é o container do dano do
/// jogador: sem cortar na fronteira, a batida do treino continuava viva na corrida e o
/// import cobrava conserto de uma corrida limpa.
#[test]
fn batida_do_treino_nao_atravessa_para_a_corrida() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;

    // TREINO (sessão 0): o jogador bate.
    m.observe(&frame_do_jogador(0, 0, SURFACE_ON_TRACK));
    let mut pancada = frame_do_jogador(0, 4, SURFACE_ON_TRACK);
    pancada.long_accel = -60.0;
    m.observe(&pancada);
    let treino = m.current_attempt;
    assert!(m.attempts.last().unwrap().peak_crash_score > 0.0);

    // CORRIDA (sessão 2): tentativa nova, sem herdar nada.
    m.observe(&frame_do_jogador(2, 0, SURFACE_ON_TRACK));
    assert!(
        m.current_attempt > treino,
        "a corrida tem de abrir uma tentativa própria"
    );
    let a = m.attempts.last().expect("tentativa da corrida");
    assert_eq!(a.status, StatusTentativa::Active);
    assert_eq!(
        a.peak_crash_score, 0.0,
        "a batida do treino não é dano da corrida"
    );
    assert!(a.crashes.is_empty());
    assert!(a.collided_with_car_number.is_none());
    // A tentativa do treino foi fechada, e por troca de sessão — não por abandono.
    let treino_fechado = m.attempts.iter().find(|x| x.number == treino).unwrap();
    assert_ne!(treino_fechado.status, StatusTentativa::Active);
    assert_eq!(treino_fechado.ended_by, Some(FimDaTentativa::SessionChange));
    assert!(
        !m.events.iter().any(|e| e.kind == "dnf_confirmed"),
        "trocar de sessão não é abandono"
    );
}

#[test]
fn composto_so_e_nomeado_dentro_do_dominio_de_dois() {
    use crate::iracing_sdk::tire_strategy::Compound;
    // O iRacing tem dois compostos e a tradução do índice é exata para eles.
    assert_eq!(Compound::from_indice(0), Compound::Dry);
    assert_eq!(Compound::from_indice(1), Compound::Wet);
    // Fora disso ninguém chuta: -1 é o "não informado" do carro mono-composto, e um 2
    // significaria que a premissa dos dois compostos caiu — em nenhum dos casos vale
    // arredondar para o vizinho mais próximo e sair falando "chuva".
    assert_eq!(Compound::from_indice(-1), Compound::Unknown);
    assert_eq!(Compound::from_indice(2), Compound::Unknown);
}
