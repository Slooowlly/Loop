//! Os casos vêm da captura real `race_1785016497.jsonl` — os números abaixo são os que o SDK
//! entregou de fato, não valores inventados para o teste passar.

use super::*;

/// Um tique de conveniência: os campos que o caso não usa ficam neutros.
fn tq(agora: f64, contagem: i32, sdk: f64) -> Tique {
    Tique {
        agora,
        contagem,
        ultimo_tempo_sdk: sdk,
        cronometro_do_sim: None,
        combustivel_l: -1.0,
        no_box: false,
    }
}

/// Roda uma sequência de tiques e devolve as voltas que fecharam.
fn rodar(c: &mut ColetorDeVoltas, tiques: &[Tique]) -> Vec<VoltaFechada> {
    tiques.iter().filter_map(|t| c.tique(*t)).collect()
}

/// Amostra a 58 Hz entre dois instantes, repetindo o valor do SDK.
fn andar(de: f64, ate: f64, contagem: i32, sdk: f64) -> Vec<Tique> {
    let mut out = Vec::new();
    let mut t = de;
    while t < ate {
        out.push(tq(t, contagem, sdk));
        t += 1.0 / 58.0;
    }
    out
}

#[test]
fn a_volta_nao_leva_o_tempo_da_anterior() {
    // A sequência exata da captura, com os atrasos medidos: em cada virada o campo do SDK
    // ainda exibe o tempo da volta anterior, e o desta volta só chega décimos depois.
    //   t=712,33 vira p/ 7, campo=97,581 → +0,417 s vem 96,531 (a volta 7)
    //   t=810,20 vira p/ 8, campo=96,531 → +0,150 s vem 97,877 (a volta 8)
    //   t=907,52 vira p/ 9, campo=97,877 → +0,433 s vem 97,313 (a volta 9)
    let mut c = ColetorDeVoltas::DEFAULT;
    let mut tiques = andar(700.0, 712.33, 6, 97.581);
    tiques.push(tq(712.33, 7, 97.581));
    tiques.extend(andar(712.4, 712.747, 7, 97.581));
    tiques.extend(andar(712.747, 810.2, 7, 96.531));
    tiques.push(tq(810.2, 8, 96.531));
    tiques.extend(andar(810.25, 810.35, 8, 96.531));
    tiques.extend(andar(810.35, 907.52, 8, 97.877));
    tiques.push(tq(907.52, 9, 97.877));
    tiques.extend(andar(907.6, 907.95, 9, 97.877));
    tiques.extend(andar(907.95, 1007.0, 9, 97.313));

    let voltas = rodar(&mut c, &tiques);
    let numeros: Vec<i32> = voltas.iter().map(|v| v.volta).collect();
    assert_eq!(numeros, vec![7, 8, 9]);
    let tempos: Vec<f64> = voltas.iter().map(|v| v.tempo_s).collect();
    for (t, esperado) in tempos.iter().zip([96.531, 97.877, 97.313]) {
        assert!(
            (t - esperado).abs() < 1e-9,
            "cada volta leva o tempo DELA: esperado {esperado}, veio {t}"
        );
    }
    assert!(voltas.iter().all(|v| v.oficial));
}

#[test]
fn o_salto_de_sessao_nao_vira_volta() {
    // Medido: na virada treino → corrida a contagem pulou 0 → 6 no meio da pista, com o
    // `LapLastLapTime` ainda marcando 97,581 s — uma volta do TREINO. Era a "volta 6" que
    // aparecia no painel da corrida.
    let mut c = ColetorDeVoltas::DEFAULT;
    let mut tiques = andar(690.0, 700.72, 0, 97.581);
    tiques.push(tq(700.72, 6, 97.581));
    tiques.extend(andar(700.8, 712.3, 6, 97.581));

    assert!(
        rodar(&mut c, &tiques).is_empty(),
        "um salto de contagem não é uma volta observada"
    );
}

#[test]
fn nao_nasce_volta_zero() {
    let mut c = ColetorDeVoltas::DEFAULT;
    let tiques = andar(0.0, 5.0, 0, 0.0);
    assert!(rodar(&mut c, &tiques).is_empty());
}

#[test]
fn a_primeira_volta_observada_fecha_com_o_tempo_certo() {
    // O monitor entra no meio da sessão: a primeira virada que ele vê é a da volta 8. Ela
    // fecha, porque o SDK publica o tempo dela depois da virada.
    let mut c = ColetorDeVoltas::DEFAULT;
    let mut tiques = andar(805.0, 810.2, 7, 96.531);
    tiques.push(tq(810.2, 8, 96.531));
    tiques.extend(andar(810.3, 810.35, 8, 96.531));
    tiques.extend(andar(810.35, 815.0, 8, 97.877));

    let voltas = rodar(&mut c, &tiques);
    assert_eq!(voltas.len(), 1);
    assert_eq!(voltas[0].volta, 8);
    assert!((voltas[0].tempo_s - 97.877).abs() < 1e-9);
}

#[test]
fn volta_anulada_pelo_sim_fecha_pelo_cronometro() {
    // Medido: `LapLastLapTime` volta -1 em volta anulada e em saída do box. O gate `> 0`
    // antigo descartava a volta inteira e abria buraco na lista.
    let mut c = ColetorDeVoltas::DEFAULT;
    let mut tiques = andar(0.0, 50.0, 2, 98.294);
    // Virada para a 3: é ela que planta a âncora do cronômetro.
    tiques.push(tq(50.0, 3, 98.294));
    tiques.extend(andar(50.1, 150.0, 3, 98.294));
    tiques.push(tq(150.0, 4, 98.294));
    // A 4 foi anulada: o campo cai para -1 e nunca publica tempo nenhum.
    tiques.extend(andar(150.1, 160.0, 4, -1.0));

    let voltas = rodar(&mut c, &tiques);
    assert_eq!(
        voltas.len(),
        1,
        "a volta existe mesmo sem o sim cronometrá-la"
    );
    assert_eq!(voltas[0].volta, 4);
    assert!(!voltas[0].oficial);
    assert!(
        (voltas[0].tempo_s - 100.0).abs() < 0.05,
        "cronometrada entre as duas viradas, e não vinda do campo obsoleto"
    );
}

#[test]
fn sem_ancora_e_sem_tempo_do_sim_a_volta_nao_entra() {
    // Primeira virada observada e o SDK nunca publica nada novo: não temos como saber quanto
    // ela durou, e inventar seria exatamente o bug que este módulo existe para matar.
    let mut c = ColetorDeVoltas::DEFAULT;
    let mut tiques = andar(0.0, 2.0, 3, 98.294);
    tiques.push(tq(2.0, 4, 98.294));
    tiques.extend(andar(2.1, 12.0, 4, 98.294));

    assert!(rodar(&mut c, &tiques).is_empty());
}

#[test]
fn o_cronometro_do_sim_cobre_a_primeira_virada() {
    // Sem âncora, mas com `LapCurrentTime` — que no tique da virada ainda marca a volta que
    // fechou (medido: 97,895 contra 97,877 oficiais).
    let mut c = ColetorDeVoltas::DEFAULT;
    let mut tiques = andar(805.0, 810.2, 7, 96.531);
    tiques.push(Tique {
        cronometro_do_sim: Some(97.895),
        ..tq(810.2, 8, 96.531)
    });
    tiques.extend(andar(810.3, 815.0, 8, 96.531));

    let voltas = rodar(&mut c, &tiques);
    assert_eq!(voltas.len(), 1);
    assert!((voltas[0].tempo_s - 97.895).abs() < 1e-9);
    assert!(!voltas[0].oficial);
}

#[test]
fn fora_do_mundo_nao_reposiciona_nem_grava() {
    // `LapCompleted` cai para -1 quando o carro sai do mundo. Voltar de lá não é uma volta.
    let mut c = ColetorDeVoltas::DEFAULT;
    let mut tiques = andar(0.0, 100.0, 3, 98.294);
    tiques.extend(andar(100.0, 102.0, -1, -1.0));
    tiques.push(tq(102.0, 4, 98.294));
    tiques.extend(andar(102.1, 105.0, 4, 97.761));

    let voltas = rodar(&mut c, &tiques);
    assert_eq!(
        voltas.len(),
        1,
        "a volta 4 fecha normalmente depois do buraco"
    );
    assert_eq!(voltas[0].volta, 4);
    assert!((voltas[0].tempo_s - 97.761).abs() < 1e-9);
}

#[test]
fn o_box_de_uma_volta_nao_suja_a_seguinte() {
    let mut c = ColetorDeVoltas::DEFAULT;
    let mut tiques = andar(0.0, 50.0, 3, 98.294);
    // Passa pelo pit road no meio da volta 4.
    tiques.extend(
        andar(50.0, 70.0, 3, 98.294)
            .into_iter()
            .map(|t| Tique { no_box: true, ..t }),
    );
    tiques.extend(andar(70.0, 100.0, 3, 98.294));
    tiques.push(tq(100.0, 4, 98.294));
    tiques.extend(andar(100.1, 105.0, 4, 97.761));
    // Volta 5 inteira longe do box.
    tiques.extend(andar(105.0, 200.0, 4, 97.761));
    tiques.push(tq(200.0, 5, 97.761));
    tiques.extend(andar(200.1, 205.0, 5, 97.800));

    let voltas = rodar(&mut c, &tiques);
    assert_eq!(voltas.len(), 2);
    assert!(voltas[0].passou_no_box, "a volta 4 passou pelo box");
    assert!(!voltas[1].passou_no_box, "a 5 não pode herdar o box da 4");
}

#[test]
fn a_ordem_gravada_e_a_ordem_da_corrida() {
    let mut c = ColetorDeVoltas::DEFAULT;
    let mut tiques = Vec::new();
    let mut agora = 0.0;
    let tempos = [97.5, 98.1, 96.9, 99.4, 97.0];
    let mut sdk = 0.0;
    for (i, dur) in tempos.iter().enumerate() {
        tiques.extend(andar(agora, agora + dur, i as i32, sdk));
        agora += dur;
        tiques.push(tq(agora, i as i32 + 1, sdk));
        sdk = *dur;
        tiques.extend(andar(agora + 0.05, agora + 0.4, i as i32 + 1, sdk));
        agora += 0.4;
    }

    let voltas = rodar(&mut c, &tiques);
    let numeros: Vec<i32> = voltas.iter().map(|v| v.volta).collect();
    assert_eq!(
        numeros,
        vec![1, 2, 3, 4, 5],
        "sem buraco e em ordem crescente"
    );
    for (v, esperado) in voltas.iter().zip(tempos.iter()) {
        assert!(
            (v.tempo_s - esperado).abs() < 1e-9,
            "volta {} devia ter {esperado}, veio {}",
            v.volta,
            v.tempo_s
        );
    }
}

#[test]
fn reiniciar_nao_deixa_pendencia_atravessar() {
    let mut c = ColetorDeVoltas::DEFAULT;
    let mut tiques = andar(0.0, 100.0, 3, 98.294);
    tiques.push(tq(100.0, 4, 98.294));
    rodar(&mut c, &tiques);
    c.reiniciar();
    // Tempo novo chegando depois do reinício: pertence à corrida que morreu.
    assert!(rodar(&mut c, &andar(100.1, 105.0, 4, 97.761)).is_empty());
}
