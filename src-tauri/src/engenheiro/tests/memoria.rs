//! A memória da conversa.
//!
//! O modo de falha aqui é uma conta certa sobre a coisa errada. "São três segundos a menos
//! que da última vez" é aritmética impecável — e vira mentira se, entre as duas perguntas,
//! o carro da frente deixou de ser o mesmo. O piloto ouve um número correto sobre uma
//! disputa que não existe, e ataca.
//!
//! Daí a maioria dos casos abaixo ser sobre a memória se RECUSAR a comparar.

use crate::engenheiro::memoria::{familia_memoria, linha, peca, Memoria};
use crate::engenheiro::responder::{self, Extras};
use crate::engenheiro::Intencao;
use crate::iracing_sdk::race_monitor::EstadoAgora;

use super::{estado_base, vizinho};

const SESSAO: i64 = 4242;

/// Um estado com um vizinho identificado à frente.
fn com_frente(idx: i32, gap: f64, volta: i32) -> EstadoAgora {
    let mut e = estado_base();
    let mut v = vizinho("James Cooper", gap);
    v.idx = idx;
    e.frente = Some(v);
    e.volta = volta;
    e
}

#[test]
fn a_primeira_pergunta_nao_tem_com_o_que_comparar() {
    let mut m = Memoria::default();
    let e = com_frente(7, 1.2, 3);
    assert_eq!(m.consultar(&e, SESSAO, Intencao::Frente), None);
}

#[test]
fn a_segunda_pergunta_diz_a_variacao() {
    let mut m = Memoria::default();
    let antes = com_frente(7, 1.2, 3);
    m.consultar(&antes, SESSAO, Intencao::Frente);
    m.registrar(&antes, SESSAO, Intencao::Frente);

    // Ele veio quatro décimos.
    let agora = com_frente(7, 0.8, 5);
    let d = m.consultar(&agora, SESSAO, Intencao::Frente).unwrap();
    assert!((d - 0.4).abs() < 1e-9, "delta {d}");
    assert_eq!(peca(d), Some("mem_menos_gap_0_4".to_string()));
}

#[test]
fn gap_que_ABRE_tem_a_outra_fala() {
    let mut m = Memoria::default();
    let antes = com_frente(7, 0.8, 3);
    m.registrar(&antes, SESSAO, Intencao::Frente);
    let agora = com_frente(7, 1.2, 4);
    let d = m.consultar(&agora, SESSAO, Intencao::Frente).unwrap();
    assert_eq!(peca(d), Some("mem_mais_gap_0_4".to_string()));
}

#[test]
fn OUTRO_carro_derruba_a_comparacao() {
    // O caso que justifica guardar o `idx`. Entre as duas perguntas houve ultrapassagem, e
    // o carro da frente agora é outro: comparar os dois gaps é comparar duas corridas.
    let mut m = Memoria::default();
    m.registrar(&com_frente(7, 3.0, 3), SESSAO, Intencao::Frente);
    let outro = com_frente(9, 0.5, 4);
    assert_eq!(m.consultar(&outro, SESSAO, Intencao::Frente), None);
}

#[test]
fn memoria_velha_nao_e_a_ULTIMA_vez() {
    // Cinco voltas depois a comparação continua verdadeira e deixou de ser útil: houve
    // parada, tráfego, talvez amarela. "Desde a última vez" implica recente.
    let mut m = Memoria::default();
    m.registrar(&com_frente(7, 3.0, 1), SESSAO, Intencao::Frente);
    assert!(m.consultar(&com_frente(7, 1.0, 6), SESSAO, Intencao::Frente).is_some());
    let mut m = Memoria::default();
    m.registrar(&com_frente(7, 3.0, 1), SESSAO, Intencao::Frente);
    assert_eq!(m.consultar(&com_frente(7, 1.0, 7), SESSAO, Intencao::Frente), None);
}

#[test]
fn variacao_dentro_do_RUIDO_nao_vira_fala() {
    // O gap oscila sozinho entre uma curva e outra. Comentar um décimo faria o engenheiro
    // narrar o próprio erro de leitura.
    let mut m = Memoria::default();
    m.registrar(&com_frente(7, 1.2, 3), SESSAO, Intencao::Frente);
    assert_eq!(m.consultar(&com_frente(7, 1.1, 4), SESSAO, Intencao::Frente), None);
    let mut m = Memoria::default();
    m.registrar(&com_frente(7, 1.2, 3), SESSAO, Intencao::Frente);
    assert!(m.consultar(&com_frente(7, 1.0, 4), SESSAO, Intencao::Frente).is_some());
}

#[test]
fn corrida_NOVA_apaga_a_conversa() {
    // Cada corrida é uma conversa. Carregar o gap da anterior compararia dois eventos.
    let mut m = Memoria::default();
    m.registrar(&com_frente(7, 3.0, 3), SESSAO, Intencao::Frente);
    assert_eq!(m.consultar(&com_frente(7, 1.0, 4), SESSAO + 1, Intencao::Frente), None);
}

#[test]
fn os_dois_lados_tem_memorias_SEPARADAS() {
    // Um gap por lado. Uma memória só faria a pergunta sobre quem vem atrás ser respondida
    // com a variação de quem está na frente.
    let mut m = Memoria::default();
    let mut e = com_frente(7, 1.2, 3);
    let mut atras = vizinho("Marco Bianchi", 2.0);
    atras.idx = 9;
    e.atras = Some(atras);
    m.registrar(&e, SESSAO, Intencao::Frente);
    m.registrar(&e, SESSAO, Intencao::Atras);

    let mut depois = com_frente(7, 1.2, 4);
    let mut atras = vizinho("Marco Bianchi", 1.2);
    atras.idx = 9;
    depois.atras = Some(atras);
    // A frente não mudou; a de trás veio oito décimos.
    assert_eq!(m.consultar(&depois, SESSAO, Intencao::Frente), None);
    assert_eq!(
        peca(m.consultar(&depois, SESSAO, Intencao::Atras).unwrap()),
        Some("mem_menos_gap_0_8".to_string())
    );
}

#[test]
fn so_registra_o_que_foi_DITO() {
    // Um vizinho no box ou uma volta à parte teve OUTRA fala — nenhum gap foi anunciado.
    // Guardar o número aqui faria a próxima comparação partir de um instante sobre o qual o
    // engenheiro nunca abriu a boca.
    for ajuste in 0..2 {
        let mut m = Memoria::default();
        let mut e = com_frente(7, 3.0, 3);
        if let Some(v) = e.frente.as_mut() {
            if ajuste == 0 {
                v.no_box = true;
            } else {
                v.volta_a_parte = true;
            }
        }
        m.registrar(&e, SESSAO, Intencao::Frente);
        assert_eq!(
            m.consultar(&com_frente(7, 1.0, 4), SESSAO, Intencao::Frente),
            None,
            "ajuste {ajuste}"
        );
    }
}

#[test]
fn intencao_que_nao_diz_gap_nao_mexe_na_memoria() {
    let mut m = Memoria::default();
    m.registrar(&com_frente(7, 3.0, 3), SESSAO, Intencao::Pneu);
    assert_eq!(m.consultar(&com_frente(7, 1.0, 4), SESSAO, Intencao::Frente), None);
    assert_eq!(m.consultar(&com_frente(7, 1.0, 4), SESSAO, Intencao::Pneu), None);
}

#[test]
fn a_memoria_entra_no_FIM_da_resposta() {
    // A informação que muda o que o piloto faz agora vem primeiro; a variação é o porquê.
    let e = com_frente(7, 1.2, 4);
    let extras = Extras {
        memoria: Some(0.4),
        ..Extras::default()
    };
    let r = responder::renderizar_com(&e, &extras, Intencao::Frente).unwrap();
    assert_eq!(r.last().unwrap(), "mem_menos_gap_0_4");
    assert!(r.len() > 1, "a memória saiu sozinha: {r:?}");
}

#[test]
fn sem_resposta_a_memoria_nao_fala_sozinha() {
    // "São quatro décimos a menos que da última vez" não é resposta a pergunta nenhuma: ela
    // qualifica um número que precisa ter saído antes.
    let mut e = estado_base();
    let mut v = vizinho("James Cooper", 1.2);
    v.volta_a_parte = true; // derruba as duas falas de gap
    e.frente = Some(v);
    let extras = Extras {
        memoria: Some(0.4),
        ..Extras::default()
    };
    assert_eq!(responder::renderizar_com(&e, &extras, Intencao::Frente), None);
}

#[test]
fn variacao_fora_da_grade_gravada_some_em_vez_de_pedir_arquivo_inexistente() {
    assert_eq!(peca(250.0), None);
    assert_eq!(peca(-250.0), None);
}

#[test]
fn o_dossie_leva_a_conta_FECHADA() {
    // Mandar os dois gaps para o modelo subtrair é como se ganha uma conta errada em prosa
    // perfeita.
    let e = com_frente(7, 1.2, 4);
    let extras = Extras {
        memoria: Some(-0.4),
        ..Extras::default()
    };
    let d = responder::dossie_com(&e, &extras, Intencao::Frente);
    assert!(d.iter().any(|l| l.contains("AUMENTOU")), "{d:?}");
    assert!(linha(0.4).contains("DIMINUIU"));
}

#[test]
fn o_catalogo_cobre_toda_variacao_que_a_peca_pede() {
    let chaves: std::collections::HashSet<String> =
        familia_memoria().into_iter().map(|(c, _)| c).collect();
    for decimos in -610..=610 {
        if let Some(p) = peca(f64::from(decimos) / 10.0) {
            assert!(chaves.contains(&p), "peça '{p}' fora do catálogo");
        }
    }
}
