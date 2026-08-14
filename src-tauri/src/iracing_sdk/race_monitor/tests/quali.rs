//! Testes da CLASSIFICATÓRIA (ver [`super::super::quali`]): o ciclo de volta, a melhor
//! volta válida e o que uma quali nova zera.

use super::super::*;
use super::comum::*;

/// Um tique do JOGADOR na classificatória, com os canais que o rádio do engenheiro lê.
fn tique(
    agora: f64,
    volta: i32,
    pct: f64,
    na_volta_s: f64,
    sdk: f64,
    no_box: bool,
) -> IracingTelemetry {
    IracingTelemetry {
        session_num: 1,
        session_time: agora,
        session_time_remain: 600.0,
        on_track: true,
        player_on_pit_road: no_box,
        track_surface: SURFACE_ON_TRACK,
        lap_completed: volta,
        lap_dist_pct: pct,
        lap_current_time: na_volta_s,
        last_lap_time: sdk,
        speed_ms: 50.0,
        ..Default::default()
    }
}

/// Desenha uma volta inteira do jogador e a fecha, do jeito que o SDK entrega de verdade.
///
/// Duas mil amostras: a curva de referência tem 200 baldes e exige 95% deles visitados, então
/// uma amostra por balde seria justamente o caso frágil que o piso existe para tolerar.
///
/// No tique da VIRADA o `LapLastLapTime` ainda exibe `sdk_antes` — o valor desta volta só
/// aparece 0,2 s depois. Devolve o `session_time` do último tique.
fn desenhar_volta(
    m: &mut RaceMonitor,
    inicio_s: f64,
    volta_que_fecha: i32,
    tempo: f64,
    sdk_antes: f64,
) -> f64 {
    const PASSOS: usize = 2000;
    for k in 0..PASSOS {
        let pct = k as f64 / PASSOS as f64;
        m.tick_classificacao(&tique(
            inicio_s + pct * tempo,
            volta_que_fecha - 1,
            pct,
            pct * tempo,
            sdk_antes,
            false,
        ));
    }
    // A virada. O `LapCurrentTime` também ainda marca a volta que fechou.
    let virada = inicio_s + tempo;
    m.tick_classificacao(&tique(
        virada,
        volta_que_fecha,
        0.0,
        tempo,
        sdk_antes,
        false,
    ));
    // O tempo oficial chega depois — 0,2 s, dentro da faixa de 0,067 s a 0,433 s medida.
    m.tick_classificacao(&tique(
        virada + 0.2,
        volta_que_fecha,
        0.002,
        0.2,
        sdk_antes,
        false,
    ));
    virada + 0.2
}

/// A saída do box, que marca a volta em curso como a de PREPARAÇÃO.
fn sair_do_box(m: &mut RaceMonitor) {
    m.tick_classificacao(&tique(0.0, 0, 0.0, 0.0, 0.0, true));
}

/// O bug de origem, no rádio da classificatória: o `LapLastLapTime` lido no tique da virada
/// ainda é o tempo da volta ANTERIOR. Aqui ele doía em dobro — a curva desenhada era de uma
/// volta e o relógio era de outra, então a melhor volta da sessão nascia com o par trocado e o
/// delta passava a medir contra uma volta que não existiu.
#[test]
fn a_referencia_da_quali_nao_leva_o_tempo_da_volta_anterior() {
    let mut m = RaceMonitor::new();
    m.qualy_session_num = 1;

    sair_do_box(&mut m);
    // Out lap de 120 s: não concorre a referência.
    let t1 = desenhar_volta_com_oficial(&mut m, 1.0, 1, 120.0, 0.0);
    // Volta lançada de 90 s. Na virada o campo ainda exibe os 120 s da out lap.
    desenhar_volta_com_oficial(&mut m, t1, 2, 90.0, 120.0);

    let melhor = m.volta_ref.melhor_s();
    assert!(
        (melhor - 90.0).abs() < 1e-6,
        "a referência fechou com o tempo de outra volta: {melhor}"
    );
}

/// A mesma volta, com o tempo oficial chegando atrasado — que é o caso normal, não a exceção.
fn desenhar_volta_com_oficial(
    m: &mut RaceMonitor,
    inicio_s: f64,
    volta_que_fecha: i32,
    tempo: f64,
    sdk_antes: f64,
) -> f64 {
    let fim = desenhar_volta(m, inicio_s, volta_que_fecha, tempo, sdk_antes);
    m.tick_classificacao(&tique(fim + 0.1, volta_que_fecha, 0.004, 0.3, tempo, false));
    fim + 0.1
}

/// `LapLastLapTime` volta -1 com frequência (volta anulada pelo sim, out lap, carro fora do
/// mundo). O tempo não vem, e a volta não pode ser jogada fora por isso: ela fecha com a
/// duração que NÓS cronometramos entre as duas viradas, que erra 0,010 s contra o oficial.
#[test]
fn a_volta_de_quali_sem_tempo_oficial_fecha_pelo_cronometro() {
    let mut m = RaceMonitor::new();
    m.qualy_session_num = 1;

    sair_do_box(&mut m);
    // Out lap e volta lançada, com o canal do tempo sentinelado o tempo inteiro.
    let t1 = desenhar_volta(&mut m, 1.0, 1, 120.0, -1.0);
    let t2 = desenhar_volta(&mut m, t1, 2, 90.0, -1.0);
    // A janela de espera do tempo oficial é de 1,5 s, e ela só fecha rodando: a terceira volta
    // é o que faz o relógio da sessão andar até lá.
    desenhar_volta(&mut m, t2, 3, 95.0, -1.0);

    let melhor = m.volta_ref.melhor_s();
    assert!(
        (melhor - 90.0).abs() < 1.0,
        "a volta sem tempo oficial sumiu ou entrou errada: {melhor}"
    );
}

/// Carro fora do mundo (garagem, guincho): o SDK devolve -1 na contagem de voltas. Não é volta
/// nem reinício — é ausência de dado, e tratá-lo como virada tiraria de cena a curva de uma
/// volta que não fechou e desmontaria o bookkeeping de preparação.
#[test]
fn contagem_negativa_na_quali_nao_e_uma_virada() {
    let mut m = RaceMonitor::new();
    m.qualy_session_num = 1;

    sair_do_box(&mut m);
    let t1 = desenhar_volta_com_oficial(&mut m, 1.0, 1, 120.0, 0.0);
    let t2 = desenhar_volta_com_oficial(&mut m, t1, 2, 90.0, 120.0);
    assert_eq!(m.quali_volta, 2);

    m.tick_classificacao(&tique(t2 + 1.0, -1, 0.0, 0.0, -1.0, false));

    assert_eq!(m.quali_volta, 2, "a sentinela -1 virou uma volta");
    let melhor = m.volta_ref.melhor_s();
    assert!(
        (melhor - 90.0).abs() < 1e-6,
        "o carro sair do mundo derrubou a referência: {melhor}"
    );
}

#[test]
fn snapshot_de_quali_fica_disponivel_apos_captura() {
    let mut m = RaceMonitor::new();
    m.qualy_session_num = 1;
    m.car_is_pace[7] = false;

    volta_de_quali(&mut m, 1, 7, 1, 82.345, 0.0, 82.345);

    let snapshot = m.qualy_laps_snapshot();
    assert_eq!(snapshot.len(), 1);
    let lap = &snapshot[0];
    assert_eq!(lap.car_idx, 7);
    assert_eq!(lap.lap, 1);
    assert!((lap.time - 82.345).abs() < f64::EPSILON);
}

/// O bug de origem, no lado da quali: o tempo lido no tique da virada é o da volta anterior.
/// Sem o ciclo de volta, a volta 2 era gravada com o tempo da volta 1.
#[test]
fn a_volta_de_quali_nao_leva_o_tempo_da_anterior() {
    let mut m = RaceMonitor::new();
    m.qualy_session_num = 1;

    volta_de_quali(&mut m, 1, 7, 1, 82.345, 0.0, 82.345);
    volta_de_quali(&mut m, 1, 7, 2, 81.100, 82.345, 81.100);

    let snapshot = m.qualy_laps_snapshot();
    assert_eq!(snapshot.len(), 2);
    assert_eq!(snapshot[0].lap, 1);
    assert!((snapshot[0].time - 82.345).abs() < f64::EPSILON);
    assert_eq!(snapshot[1].lap, 2);
    assert!(
        (snapshot[1].time - 81.100).abs() < f64::EPSILON,
        "a volta 2 tem de levar o tempo DA 2, não os 82,345 que o campo ainda exibia"
    );
}

/// A regra da classificatória: o tempo que vale é o VÁLIDO. O carro corta a pista e
/// marca 80,1 s; o iRacing anula a volta, então o `CarIdxBestLapTime` continua nos
/// 82,3 s da volta limpa, e é esse que a torre tem de usar. As voltas cruas guardam a
/// cortada com o tempo que ela marcou, e é justamente por isso que elas não servem
/// para ordenar a quali.
#[test]
fn melhor_volta_valida_da_quali_ignora_a_volta_anulada() {
    let mut m = RaceMonitor::new();
    m.set_qualy_session_num(1);

    volta_de_quali(&mut m, 1, 7, 1, 82.345, 0.0, 82.345);
    // Volta seguinte, mais rápida no relógio e anulada: o melhor VÁLIDO não se move.
    volta_de_quali(&mut m, 1, 7, 2, 80.100, 82.345, 82.345);

    assert_eq!(
        m.qualy_best_valid_snapshot(),
        vec![(7, 82.345)],
        "a volta anulada não pode virar o melhor tempo da classificatória"
    );
    // A volta crua continua registrada — ela alimenta outros consumidores.
    assert_eq!(m.qualy_laps_snapshot().len(), 2);
}

/// Carro na garagem SAI de `cars`, e com ele sairia o `CarIdxBestLapTime`. Sem a trava,
/// o piloto que marcou o melhor tempo e voltou pro box desaparecia da ordenação.
#[test]
fn melhor_volta_valida_sobrevive_ao_carro_sair_do_mundo() {
    let mut m = RaceMonitor::new();
    m.set_qualy_session_num(1);

    let mut car = on_track(7, 0, 1);
    car.last_lap_time = 82.345;
    car.best_lap_time = 82.345;
    m.capture_qualy(&IracingTelemetry {
        session_num: 1,
        cars: vec![car],
        ..Default::default()
    });

    // Tique seguinte já sem o carro no mundo (foi pra garagem).
    m.capture_qualy(&IracingTelemetry {
        session_num: 1,
        cars: vec![],
        ..Default::default()
    });

    assert_eq!(m.qualy_best_valid_snapshot(), vec![(7, 82.345)]);
}

/// Quali nova zera o melhor válido junto com o resto. Sem isso o tempo da pista
/// anterior seguiria ordenando a torre.
#[test]
fn quali_nova_zera_o_melhor_valido() {
    let mut m = RaceMonitor::new();
    m.set_qualy_session_num(1);

    let mut car = on_track(7, 0, 1);
    car.last_lap_time = 82.345;
    car.best_lap_time = 82.345;
    m.capture_qualy(&IracingTelemetry {
        session_num: 1,
        cars: vec![car],
        ..Default::default()
    });
    assert_eq!(m.qualy_best_valid_snapshot().len(), 1);

    m.set_qualy_session_num(2);

    assert!(m.qualy_best_valid_snapshot().is_empty());
}

#[test]
fn troca_do_numero_da_quali_reseta_estado_e_captura_a_sessao_nova() {
    let mut m = RaceMonitor::new();
    m.set_qualy_session_num(1);

    volta_de_quali(&mut m, 1, 7, 1, 82.345, 0.0, 82.345);
    assert_eq!(m.qualy_laps_snapshot().len(), 1);

    m.set_qualy_session_num(2);
    assert!(
        !m.prev_in_qualy,
        "a nova sessão precisa rearmar a borda de entrada"
    );
    assert!(
        m.qualy_laps_snapshot().is_empty(),
        "as voltas antigas devem sumir imediatamente"
    );
    assert!(
        m.voltas_quali.iter().all(|c| c.ocioso()),
        "uma volta aberta na quali anterior fecharia dentro da nova"
    );

    volta_de_quali(&mut m, 2, 7, 1, 81.234, 0.0, 81.234);
    assert!(m.prev_in_qualy);
    let snapshot = m.qualy_laps_snapshot();
    assert_eq!(snapshot.len(), 1);
    assert!((snapshot[0].time - 81.234).abs() < f64::EPSILON);
    assert_eq!(snapshot[0].lap, 1);
}

#[test]
fn troca_de_subsession_reseta_quali_com_session_num_igual() {
    let mut m = RaceMonitor::new();
    m.set_qualy_session_num(1);
    m.set_session_subsession_id(1001);

    volta_de_quali(&mut m, 1, 7, 1, 82.345, 0.0, 82.345);
    assert_eq!(m.qualy_laps_snapshot().len(), 1);

    m.set_session_subsession_id(1002);

    assert_eq!(m.session_subsession_id, 1002);
    assert_eq!(m.qualy_session_num, 1);
    assert!(!m.prev_in_qualy);
    assert!(m.qualy_laps_snapshot().is_empty());
    assert!(m.voltas_quali.iter().all(|c| c.ocioso()));
}

#[test]
fn subsession_zero_nao_descarta_estado_valido() {
    let mut m = RaceMonitor::new();
    m.set_qualy_session_num(1);
    m.set_session_subsession_id(1001);

    volta_de_quali(&mut m, 1, 7, 1, 82.345, 0.0, 82.345);

    m.set_session_subsession_id(0);

    assert_eq!(m.session_subsession_id, 1001);
    assert!(m.prev_in_qualy);
    assert_eq!(m.qualy_laps_snapshot().len(), 1);
    assert!(
        !m.voltas_quali[7].ocioso(),
        "subsession 0 é sentinela e não pode zerar o ciclo de volta em andamento"
    );
}
