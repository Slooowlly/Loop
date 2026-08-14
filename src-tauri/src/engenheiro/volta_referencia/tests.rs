use super::*;

/// Desenha uma volta inteira a um ritmo constante e a fecha nos dois tempos reais: a curva sai
/// de cena na virada, o tempo chega depois.
///
/// Amostra DENSO, como a telemetria a 60 Hz: dez amostras por balde. Um por balde parecia
/// suficiente e não é — `3/200 * 200` dá 2,9999999999999996 em ponto flutuante, e o balde 3
/// ficava vazio. Foi assim que a fragilidade do piso de cobertura apareceu.
fn volta(v: &mut VoltaReferencia, tempo_s: f64) {
    desenhar(v, tempo_s);
    v.suspender_volta();
    v.confirmar_suspensa(tempo_s);
}

/// Só o desenho, sem fechar. Serve aos casos que precisam mexer na curva antes da virada.
fn desenhar(v: &mut VoltaReferencia, tempo_s: f64) {
    let amostras = BALDES * 10;
    for i in 0..amostras {
        let pct = i as f64 / amostras as f64;
        v.amostrar(pct, pct * tempo_s);
    }
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
    v.suspender_volta();
    v.confirmar_suspensa(90.0);
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
    v.suspender_volta();
    v.confirmar_suspensa(90.0);
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
    v.suspender_volta();
    v.confirmar_suspensa(90.0);
    assert!(v.tem_referencia(), "um buraco de 1% derrubou a volta");
    // E o buraco foi interpolado, não deixado em zero — que produziria delta absurdo ali.
    let d = v
        .delta_s(0.41, 0.41 * 90.0)
        .expect("sem delta no trecho interpolado");
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
fn confirmar_a_suspensa_nao_apaga_a_volta_em_curso() {
    // O defeito que a suspensão evita. O tempo oficial chega alguns décimos DEPOIS da virada, e
    // nesse intervalo a volta seguinte já está sendo desenhada. O fecho num passo só terminava
    // descartando o desenho: a volta nova perdia o começo e era reprovada no piso de cobertura,
    // então a melhor volta da sessão simplesmente não entrava.
    let mut v = VoltaReferencia::novo();
    let amostras = BALDES * 10;

    desenhar(&mut v, 95.0);
    v.suspender_volta();

    // Os primeiros 10% da volta seguinte, desenhados enquanto o tempo da anterior não chega.
    for i in 0..(amostras / 10) {
        let pct = i as f64 / amostras as f64;
        v.amostrar(pct, pct * 90.0);
    }
    v.confirmar_suspensa(95.0);
    // O resto dela, já com a anterior confirmada.
    for i in (amostras / 10)..amostras {
        let pct = i as f64 / amostras as f64;
        v.amostrar(pct, pct * 90.0);
    }
    v.suspender_volta();
    v.confirmar_suspensa(90.0);

    assert!(
        (v.melhor_s() - 90.0).abs() < 1e-9,
        "a volta de 90 s perdeu o começo e foi reprovada na cobertura: melhor {}",
        v.melhor_s()
    );
}

#[test]
fn suspensa_sem_tempo_nao_atrasa_a_referencia_em_uma_volta() {
    // O tempo pode nunca chegar (volta anulada pelo sim sem cronômetro nosso). A suspensa velha
    // não pode sobreviver até a próxima confirmação: ela casaria a curva de uma volta com o
    // relógio da seguinte, que é o mesmo par trocado que o ciclo de volta existe para matar.
    let mut v = VoltaReferencia::novo();
    desenhar(&mut v, 95.0);
    v.suspender_volta(); // volta lenta, tempo nunca confirmado
    desenhar(&mut v, 90.0);
    v.suspender_volta(); // volta rápida, esta é a que vai receber o tempo
    v.confirmar_suspensa(90.0);

    assert!((v.melhor_s() - 90.0).abs() < 1e-9);
    // A curva guardada é a de 90 s: no meio da volta ela marca ~45 s, não ~47,5 s.
    let d = v.delta_s(0.5, 45.0).expect("há referência");
    assert!(d.abs() < 0.5, "a curva ficou a da volta anterior: {d}");
}

#[test]
fn confirmar_sem_suspensa_nao_faz_nada() {
    // A volta de preparação e a volta suja nem chegam a ser suspensas, e o ciclo de volta do
    // monitor entrega o tempo delas do mesmo jeito. Aceitar aqui faria a curva da tentativa
    // ANTERIOR ser regravada com o tempo do passeio de saída do box.
    let mut v = VoltaReferencia::novo();
    volta(&mut v, 90.0);
    v.confirmar_suspensa(70.0);
    assert!((v.melhor_s() - 90.0).abs() < 1e-9);
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
