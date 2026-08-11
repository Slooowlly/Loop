//! Testes da CLASSIFICATÓRIA (ver [`super::super::quali`]): o ciclo de volta, a melhor
//! volta válida e o que uma quali nova zera.

use super::super::*;
use super::comum::*;

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
