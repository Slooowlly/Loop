use super::*;
use std::collections::HashSet;

fn preparacao(ate_a_linha_s: f64) -> Momento {
    Momento {
        ate_a_linha_s,
        restante_s: 600.0,
        volta_referencia_s: 90.0,
        volta_morta: false,
        em_preparacao: true,
        voando: false,
    }
}

fn voando(ate_a_linha_s: f64, morta: bool) -> Momento {
    Momento {
        ate_a_linha_s,
        restante_s: 600.0,
        volta_referencia_s: 90.0,
        volta_morta: morta,
        em_preparacao: false,
        voando: true,
    }
}

#[test]
fn a_despedida_sai_uma_vez_e_perto_da_linha() {
    let mut o = Observador::novo();
    // Meio da volta de preparação: cedo demais. Falar aqui deixaria um silêncio esquisito entre
    // a fala e a volta lançada, que é o oposto do efeito.
    assert_eq!(o.observar(preparacao(40.0)), None);
    assert_eq!(o.observar(preparacao(10.0)), None);
    // Dentro da janela.
    let f = o.observar(preparacao(4.0)).expect("não despediu");
    assert_eq!(f.pecas.len(), 1);
    assert!(f.pecas[0].starts_with("cl_despedida_"));
    // E não repete na mesma volta.
    assert_eq!(o.observar(preparacao(3.0)), None);
    assert_eq!(o.observar(preparacao(1.0)), None);
}

#[test]
fn a_despedida_nao_atravessa_a_linha() {
    // O prazo é duro: uma fala que começa faltando meio segundo termina dentro da volta lançada,
    // com o piloto entrando na primeira curva ouvindo o engenheiro. É o pior defeito possível
    // desta família, e por isso a janela tem piso e não só teto.
    let mut o = Observador::novo();
    for restante in [0.5, 0.2, 0.0, -1.0] {
        assert_eq!(
            o.observar(preparacao(restante)),
            None,
            "despediu a {restante}s da linha"
        );
    }
}

#[test]
fn na_volta_lancada_ele_cala() {
    let mut o = Observador::novo();
    o.observar(preparacao(4.0));
    for pct in [0.9, 0.5, 0.1] {
        assert_eq!(
            o.observar(voando(90.0 * pct, false)),
            None,
            "falou na volta boa"
        );
    }
}

#[test]
fn a_volta_morta_reconhece_e_conta_as_tentativas() {
    let mut o = Observador::novo();
    o.observar(preparacao(4.0));
    let f = o
        .observar(voando(45.0, true))
        .expect("calou na volta morta");
    assert_eq!(f.pecas.len(), 2);
    assert!(f.pecas[0].starts_with("cl_perdeu_"));
    // 600 s de sessão, menos os 45 até a linha, dividido por duas voltas de 90 = 3 tentativas.
    assert_eq!(f.pecas[1], "cl_restam_3");
    // E só comenta uma vez por volta.
    assert_eq!(o.observar(voando(20.0, true)), None);
}

#[test]
fn sem_tempo_para_outra_ele_nao_consola() {
    // Dizer "ainda dá tempo" quando não dá é a única coisa que faria o piloto desconfiar de tudo
    // o mais que este rádio diz.
    let mut o = Observador::novo();
    o.observar(preparacao(4.0));
    let mut m = voando(30.0, true);
    m.restante_s = 100.0; // sobra 70 s, e uma tentativa custa 180
    let f = o.observar(m).expect("calou");
    assert!(
        f.pecas[1].starts_with("cl_acabou_"),
        "prometeu tentativa que não cabe: {:?}",
        f.pecas
    );
}

#[test]
fn sem_referencia_de_volta_ele_reconhece_e_para_por_ai() {
    // Sem saber quanto custa uma tentativa não há como dizer quantas cabem. Meia fala honesta é
    // melhor que um número inventado — e é o caso da PRIMEIRA tentativa, que é justamente
    // quando o erro dói mais.
    let mut o = Observador::novo();
    o.observar(preparacao(4.0));
    let mut m = voando(30.0, true);
    m.volta_referencia_s = 0.0;
    let f = o.observar(m).expect("calou");
    assert_eq!(f.pecas.len(), 1);
    assert!(f.pecas[0].starts_with("cl_perdeu_"));
}

#[test]
fn cada_tentativa_ganha_uma_despedida_nova() {
    // A mesma frase duas vezes na mesma sessão soa mecânica justo na família que existe para
    // soar humana. O rodízio é por SESSÃO, não por volta.
    let mut o = Observador::novo();
    let mut vistas = HashSet::new();
    for _ in 0..4 {
        let f = o.observar(preparacao(4.0)).expect("não despediu");
        vistas.insert(f.pecas[0].clone());
        // Volta lançada e de volta à preparação: é o ciclo de uma tentativa.
        o.observar(voando(45.0, false));
        o.observar(preparacao(60.0));
    }
    assert_eq!(
        vistas.len(),
        4,
        "repetiu despedida na mesma sessão: {vistas:?}"
    );
}

#[test]
fn a_conta_de_tentativas_desconta_a_volta_de_preparacao() {
    // Cada tentativa custa DUAS voltas. Contar só a lançada prometeria uma tentativa que não
    // existe, e a fala perderia a única coisa que a torna útil.
    assert_eq!(tentativas_que_cabem(180.0, 90.0), Some(1));
    assert_eq!(tentativas_que_cabem(179.0, 90.0), Some(0));
    assert_eq!(tentativas_que_cabem(540.0, 90.0), Some(3));
    assert_eq!(tentativas_que_cabem(100.0, 0.0), None);
}

#[test]
fn o_catalogo_nao_repete_chave_e_toda_fala_esta_nele() {
    let v = familia_classificacao();
    let chaves: HashSet<&String> = v.iter().map(|(k, _)| k).collect();
    assert_eq!(chaves.len(), v.len(), "chave repetida no catálogo");
    assert_eq!(v.len(), 6 + 3 + 5 + 3);
    for (k, _) in &v {
        assert!(k.starts_with(PREFIXO), "{k} fora do prefixo da família");
    }
    // O teto do `restam` cai na peça genérica em vez de pedir um arquivo que ninguém gravou.
    assert_eq!(restam(50).0, "cl_restam_muitas");
}

#[test]
fn a_despedida_nao_carrega_dado_nenhum() {
    // É a única fala do acervo que não informa nada, e é disso que ela vive. Um número aqui a
    // transformaria em boletim — que é exatamente o que o resto do rádio já faz.
    for i in 0..6 {
        let (chave, texto) = despedida(i);
        assert!(
            !texto.chars().any(|c| c.is_ascii_digit()),
            "{chave} virou boletim: {texto:?}",
        );
    }
}
