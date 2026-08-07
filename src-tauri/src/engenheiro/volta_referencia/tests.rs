use super::*;

/// Desenha uma volta inteira a um ritmo constante e a fecha.
///
/// Amostra DENSO, como a telemetria a 60 Hz: dez amostras por balde. Um por balde parecia
/// suficiente e não é — `3/200 * 200` dá 2,9999999999999996 em ponto flutuante, e o balde 3
/// ficava vazio. Foi assim que a fragilidade do piso de cobertura apareceu.
fn volta(v: &mut VoltaReferencia, tempo_s: f64) {
    let amostras = BALDES * 10;
    for i in 0..amostras {
        let pct = i as f64 / amostras as f64;
        v.amostrar(pct, pct * tempo_s);
    }
    v.fechar_volta(tempo_s);
}

#[test]
fn sem_referencia_nao_ha_delta() {
    // A primeira tentativa não tem contra o que comparar, e inventar um delta ali seria o pior
    // momento para errar: é a volta em que o piloto menos quer palpite.
    let v = VoltaReferencia::novo();
    assert_eq!(v.delta_s(0.5, 45.0), None);
    assert!(!v.tem_referencia());
    assert!(!v.volta_morta(0.5, 999.0));
}

#[test]
fn a_melhor_volta_vira_a_curva() {
    let mut v = VoltaReferencia::novo();
    volta(&mut v, 90.0);
    assert!(v.tem_referencia());
    assert!((v.melhor_s() - 90.0).abs() < 1e-9);
    // Meio da volta, no mesmo ritmo: delta zero.
    assert!(v.delta_s(0.5, 45.0).unwrap().abs() < 0.5);
    // Meio da volta, dois segundos atrás.
    assert!((v.delta_s(0.5, 47.0).unwrap() - 2.0).abs() < 0.5);
}

#[test]
fn so_a_melhor_substitui_a_referencia() {
    let mut v = VoltaReferencia::novo();
    volta(&mut v, 90.0);
    volta(&mut v, 95.0); // pior: não entra
    assert!((v.melhor_s() - 90.0).abs() < 1e-9);
    volta(&mut v, 88.0); // melhor: entra
    assert!((v.melhor_s() - 88.0).abs() < 1e-9);
}

#[test]
fn volta_com_buraco_nao_vira_referencia() {
    // O defeito que este guarda evita é silencioso e grande: uma volta em que o carro entrou no
    // box no meio deixa baldes zerados, e a curva passaria a dizer que aquele trecho leva zero
    // segundo. O delta viraria negativo enorme e o rádio anunciaria que estamos voando.
    let mut v = VoltaReferencia::novo();
    let amostras = BALDES * 10;
    for i in 0..(amostras / 2) {
        let pct = i as f64 / amostras as f64;
        v.amostrar(pct, pct * 90.0);
    }
    v.fechar_volta(90.0);
    assert!(!v.tem_referencia(), "meia volta virou referência");
}

#[test]
fn o_primeiro_valor_do_balde_e_que_vale() {
    // Passar duas vezes pelo mesmo balde (carro lento, ou trepidando na linha do balde) não pode
    // reescrever a curva com o instante da SAÍDA do trecho.
    let mut v = VoltaReferencia::novo();
    let amostras = BALDES * 10;
    for i in 0..amostras {
        let pct = i as f64 / amostras as f64;
        v.amostrar(pct, pct * 90.0);
        v.amostrar(pct, pct * 90.0 + 5.0); // segunda passagem, mais tarde
    }
    v.fechar_volta(90.0);
    assert!(v.delta_s(0.5, 45.0).unwrap().abs() < 0.5);
}

#[test]
fn a_largada_da_volta_nao_produz_delta() {
    // No balde zero toda volta vale ~0, e comparar ali diria "está no páreo" em qualquer
    // tentativa — inclusive na que vai morrer três curvas adiante.
    let mut v = VoltaReferencia::novo();
    volta(&mut v, 90.0);
    assert_eq!(v.delta_s(0.0, 0.0), None);
    assert_eq!(v.delta_s(0.001, 0.0), None);
    assert!(v.delta_s(0.02, 2.0).is_some());
}

#[test]
fn a_volta_so_morre_depois_do_limiar() {
    let mut v = VoltaReferencia::novo();
    volta(&mut v, 90.0);
    // Três décimos é ruído de pilotagem — dizer "perdeu essa" aqui custaria a volta ao piloto.
    assert!(!v.volta_morta(0.5, 45.3));
    assert!(!v.volta_morta(0.5, 45.0 + LIMIAR_VOLTA_MORTA_S - 0.1));
    assert!(v.volta_morta(0.5, 45.0 + LIMIAR_VOLTA_MORTA_S + 0.1));
}

#[test]
fn um_quadro_perdido_nao_derruba_a_volta() {
    // O piso de cobertura existe porque 100% é frágil: um quadro perdido na telemetria abre um
    // buraco, e sem folga a melhor volta da sessão seria descartada por causa dele.
    let mut v = VoltaReferencia::novo();
    let amostras = BALDES * 10;
    for i in 0..amostras {
        let pct = i as f64 / amostras as f64;
        // Some com um trecho de 1% da volta — dois baldes.
        if (0.40..0.42).contains(&pct) {
            continue;
        }
        v.amostrar(pct, pct * 90.0);
    }
    v.fechar_volta(90.0);
    assert!(v.tem_referencia(), "um buraco de 1% derrubou a volta");
    // E o buraco foi interpolado, não deixado em zero — que produziria delta absurdo ali.
    let d = v.delta_s(0.41, 0.41 * 90.0).expect("sem delta no trecho interpolado");
    assert!(d.abs() < 0.5, "o buraco não foi interpolado: delta {d}");
}

#[test]
fn descartar_nao_apaga_a_referencia() {
    // A volta de saída do box é descartada toda sessão; se ela levasse a referência junto, o
    // delta morreria na primeira ida ao box.
    let mut v = VoltaReferencia::novo();
    volta(&mut v, 90.0);
    v.amostrar(0.3, 40.0);
    v.descartar_volta();
    assert!(v.tem_referencia());
    assert!(v.delta_s(0.5, 45.0).is_some());
}

#[test]
fn reiniciar_esquece_a_sessao() {
    // A melhor do classificatório não vale para a corrida: outro tanque, outro pneu, outro
    // objetivo. Herdar a curva faria o engenheiro cobrar na corrida um ritmo de volta lançada.
    let mut v = VoltaReferencia::novo();
    volta(&mut v, 90.0);
    v.reiniciar();
    assert!(!v.tem_referencia());
    assert_eq!(v.melhor_s(), 0.0);
}
