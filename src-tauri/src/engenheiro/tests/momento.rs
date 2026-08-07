//! Quando o rádio se cala.
//!
//! O erro barato aqui é calar demais: um engenheiro mudo a corrida inteira não parece
//! concentrado, parece quebrado. O erro caro é o oposto — falar no meio de uma disputa —,
//! e é por isso que as condições existem. Os casos abaixo cercam os dois lados.

use crate::engenheiro::momento::{quente, Motivo};
use crate::iracing_sdk::race_monitor::EstadoAgora;

use super::{estado_base, vizinho};

/// Uma corrida em curso, na volta 10, sem ninguém por perto.
fn correndo() -> EstadoAgora {
    let mut e = estado_base();
    e.em_corrida = true;
    e.em_formacao = false;
    e.volta = 10;
    e.voltas_restantes = 12;
    e.voltas_restantes_estimadas = false;
    e.bandeira = String::new();
    e.frente = None;
    e.atras = None;
    e
}

#[test]
fn corrida_tranquila_deixa_o_radio_falar() {
    // O caso mais importante da lista. Se este virar "quente", o engenheiro emudece a
    // corrida toda e ninguém liga a causa ao efeito.
    assert_eq!(quente(&correndo()), None);
}

#[test]
fn fora_de_corrida_nao_ha_o_que_suprimir() {
    // No treino e na classificação a fala não solicitada é o que dá vida ao box.
    let mut e = correndo();
    e.em_corrida = false;
    assert_eq!(quente(&e), None);

    let mut e = correndo();
    e.em_formacao = true;
    assert_eq!(quente(&e), None);
}

#[test]
fn a_largada_cala() {
    let mut e = correndo();
    e.volta = 1;
    assert_eq!(quente(&e), Some(Motivo::Largada));
    e.volta = 2;
    assert_eq!(quente(&e), None);
}

#[test]
fn a_ultima_volta_cala() {
    let mut e = correndo();
    e.bandeira = "Última volta".to_string();
    assert_eq!(quente(&e), Some(Motivo::UltimaVolta));

    // E pela contagem, quando não há bandeira branca.
    let mut e = correndo();
    e.voltas_restantes = 1;
    assert_eq!(quente(&e), Some(Motivo::UltimaVolta));
    e.voltas_restantes = 2;
    assert_eq!(quente(&e), None);
}

#[test]
fn estimativa_de_prova_por_TEMPO_nao_cala_o_radio() {
    // `voltas_restantes` é chute em prova por tempo — o `SessionLapsRemainEx` vem
    // sentinelado. Calar o rádio nas últimas voltas de uma estimativa faria o engenheiro
    // sumir a meia hora do fim, por causa de uma conta.
    let mut e = correndo();
    e.voltas_restantes = 1;
    e.voltas_restantes_estimadas = true;
    assert_eq!(quente(&e), None);
}

#[test]
fn duelo_dos_dois_lados_cala() {
    for frente in [true, false] {
        let mut e = correndo();
        let v = vizinho("James Cooper", 0.6);
        if frente {
            e.frente = Some(v);
        } else {
            e.atras = Some(v);
        }
        assert_eq!(quente(&e), Some(Motivo::Duelo), "frente={frente}");
    }
}

#[test]
fn um_segundo_e_meio_ja_nao_e_duelo() {
    let mut e = correndo();
    e.frente = Some(vizinho("James Cooper", 1.5));
    assert_eq!(quente(&e), None);
}

#[test]
fn trafego_NAO_e_duelo() {
    // Carro no box e carro uma volta à parte estão perto no relógio e não estão na sua
    // corrida. Calar o rádio por causa deles seria suprimir a notícia por causa de trânsito.
    for ajuste in 0..2 {
        let mut e = correndo();
        let mut v = vizinho("James Cooper", 0.4);
        if ajuste == 0 {
            v.no_box = true;
        } else {
            v.volta_a_parte = true;
        }
        e.frente = Some(v);
        assert_eq!(quente(&e), None, "ajuste {ajuste}");
    }
}

#[test]
fn a_AMARELA_desarma_tudo() {
    // Sob amarela os carros ficam colados e ninguém está disputando nada — é a janela
    // clássica de conversa de rádio. Sem esta regra, o teste de duelo calaria o engenheiro
    // exatamente no melhor momento para ele falar.
    let mut e = correndo();
    e.frente = Some(vizinho("James Cooper", 0.3));
    e.volta = 1;
    assert_eq!(quente(&e), Some(Motivo::Largada));
    e.bandeira = "Bandeira amarela".to_string();
    assert_eq!(quente(&e), None);
}

// ─── As ocasiões ─────────────────────────────────────────────────────────────

#[test]
fn a_volta_de_formacao_e_a_bandeirada_sao_as_ocasioes() {
    use crate::engenheiro::momento::{ocasiao, Ocasiao};

    let mut e = correndo();
    e.em_formacao = true;
    assert_eq!(ocasiao(&e), Some(Ocasiao::AntesDaLargada));

    let mut e = correndo();
    e.bandeira = "Bandeirada".to_string();
    assert_eq!(ocasiao(&e), Some(Ocasiao::DepoisDaBandeirada));

    // No meio da corrida não há ocasião nenhuma — e é o caso mais importante, porque um
    // falso positivo aqui vira uma fala longa do modelo por cima de uma disputa.
    assert_eq!(ocasiao(&correndo()), None);
}

#[test]
fn a_bandeirada_GANHA_da_formacao() {
    // Os dois sinais podem coexistir num instante de troca de sessão. A bandeirada é a que
    // tem resultado para comentar; a formação já passou.
    use crate::engenheiro::momento::{ocasiao, Ocasiao};
    let mut e = correndo();
    e.em_formacao = true;
    e.bandeira = "Bandeirada".to_string();
    assert_eq!(ocasiao(&e), Some(Ocasiao::DepoisDaBandeirada));
}

#[test]
fn a_BANDEIRADA_desarma_o_duelo() {
    // O par do teste da amarela, e o caso que quase custou a fala mais cara do rádio. Depois
    // da bandeirada os carros cruzam a linha colados e desaceleram juntos — retrato exato de
    // um duelo para quem só olha o gap. Sem esta regra, atravessar a linha quatro décimos
    // atrás de alguém seguraria a fala do título na fila de anúncios até ela ser descartada
    // por validade, e o piloto ouviria silêncio no fim da temporada.
    use crate::engenheiro::momento::{ocasiao, Ocasiao};

    let mut e = correndo();
    e.frente = Some(vizinho("James Cooper", 0.4));
    assert_eq!(quente(&e), Some(Motivo::Duelo));
    e.bandeira = "Bandeirada".to_string();
    assert_eq!(quente(&e), None);
    // E ela continua sendo a ocasião: as duas coisas têm de valer ao mesmo tempo, senão a
    // fala existe e a fila não a solta.
    assert_eq!(ocasiao(&e), Some(Ocasiao::DepoisDaBandeirada));
}

#[test]
fn desconectado_nao_tem_ocasiao() {
    // Os campos de um estado vazio são todos falsos, mas um save recém-aberto sem iRacing
    // não é uma volta de formação — e a fala custaria uma ida ao modelo.
    use crate::engenheiro::momento::ocasiao;
    let mut e = correndo();
    e.conectado = false;
    e.em_formacao = true;
    assert_eq!(ocasiao(&e), None);
}

#[test]
fn a_ocasiao_nao_e_momento_quente() {
    // As duas coisas convivem e não se confundem: a formação é ocasião E é calma, então a
    // fala longa sai e a fila de anúncios não é segurada por ela.
    use crate::engenheiro::momento::ocasiao;
    let mut e = correndo();
    e.em_formacao = true;
    assert!(ocasiao(&e).is_some());
    assert_eq!(quente(&e), None);
}

#[test]
fn gap_invalido_nao_inventa_duelo() {
    // `-1` é "não sei", e `NaN` é o mesmo. Nenhum dos dois é proximidade — tratá-los como
    // zero calaria o rádio sempre que o gap ficasse desconhecido por um instante.
    for gap in [-1.0, f64::NAN] {
        let mut e = correndo();
        e.frente = Some(vizinho("James Cooper", gap));
        assert_eq!(quente(&e), None, "gap {gap}");
    }
}
