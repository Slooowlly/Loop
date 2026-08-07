//! A hipótese dos custos categóricos, medida.
//!
//! **Hipótese**: a íngreme da pirâmide do Loop vem de custos que simplesmente não existem
//! abaixo de certo degrau — simulador, contrato de fábrica, aquisição de dados — e não de
//! "mais gente do mesmo tipo".
//!
//! **Resultado: não se sustenta.** O mecanismo existe e está modelado — as três linhas
//! categóricas realmente nascem do zero em degraus concretos, e uma equipe de Rookie de
//! fato não tem uma versão barata de simulador. Mas elas nunca passam de ~10% do ano de
//! ninguém, e a escada com e sem elas é praticamente a mesma: 21× contra 19×.
//!
//! O que faz a pirâmide é a FOLHA: 4 pessoas a 28 mil na base contra 30 pessoas a 78 mil no
//! topo. Sete vezes e meia mais gente, cada uma custando quase três vezes mais — 21× de
//! escada saem daí, e é exatamente "mais gente do mesmo tipo, paga melhor".
//!
//! Os testes abaixo travam essa conclusão para que ela não seja desfeita por acidente, e o
//! de sensibilidade mostra que ela não depende da calibração que eu escolhi.

use super::legado::amplitude_legada;
use crate::economia::ancora::{self, DIVISOES};
use crate::economia::temporada::{self, DecomposicaoAnual};

fn decomposicoes() -> Vec<DecomposicaoAnual> {
    DIVISOES
        .iter()
        .map(|(c, k)| temporada::decomposicao_anual(c, *k))
        .collect()
}

fn amplitude(valores: impl Iterator<Item = f64>) -> f64 {
    let v: Vec<f64> = valores.collect();
    v.iter().cloned().fold(0.0, f64::max) / v.iter().cloned().fold(f64::MAX, f64::min)
}

/// O mecanismo EXISTE: as linhas categóricas são de fato cliff, não rampa. Elas valem
/// exatamente zero na base e a fatura nem as mostra.
#[test]
fn as_linhas_categoricas_sao_degrau_e_nao_rampa() {
    let equipe = temporada::EquipeNaTemporada::default();

    // Base da escada: nenhuma das três existe.
    for (categoria, classe) in [("mazda_rookie", None), ("mazda_amador", None)] {
        let fatura = temporada::fatura_de_temporada(categoria, classe, &equipe);
        for chave in temporada::LINHAS_CATEGORICAS {
            assert!(
                fatura.linha(chave).is_none(),
                "{categoria} não deveria ter linha '{chave}' nenhuma — nem barata"
            );
        }
    }

    // Simulador nasce no GT4 e não antes: no tier 2 ainda não existe.
    let p2 = temporada::parametros_de_temporada("bmw_m2", None);
    let p3 = temporada::parametros_de_temporada("gt4", None);
    assert_eq!(p2.simulador_anual, 0.0);
    assert!(p3.simulador_anual > 0.0);

    // Suporte de fábrica e dados nascem no tier 2, com o primeiro carro de corrida de
    // cliente do jogo.
    let p1 = temporada::parametros_de_temporada("mazda_amador", None);
    assert_eq!(p1.suporte_de_fabrica_anual, 0.0);
    assert_eq!(p1.aquisicao_de_dados_anual, 0.0);
    assert!(p2.suporte_de_fabrica_anual > 0.0);
    assert!(p2.aquisicao_de_dados_anual > 0.0);
}

/// Mas o mecanismo é PEQUENO. Em divisão nenhuma o categórico passa de um décimo do ano.
#[test]
fn o_categorico_nunca_domina_o_ano() {
    for (categoria, classe) in DIVISOES {
        let d = temporada::decomposicao_anual(categoria, classe);
        assert!(
            d.fracao_categorica() < 0.15,
            "{categoria}:{classe:?} tem {:.1}% de custo categórico",
            d.fracao_categorica() * 100.0
        );
    }
}

/// **A hipótese cai aqui.** Tirar TODO o custo categórico do modelo quase não mexe na
/// íngreme da escada — some 8% de inclinação, não a inclinação.
#[test]
fn tirar_os_categoricos_quase_nao_achata_a_escada() {
    let dec = decomposicoes();
    let com = amplitude(dec.iter().map(|d| d.total()));
    let sem = amplitude(dec.iter().map(|d| d.total() - d.recorrentes_categoricos));

    let contribuicao = com / sem - 1.0;
    assert!(
        contribuicao < 0.25,
        "os categóricos respondem por {:.0}% da íngreme — a hipótese se sustentaria",
        contribuicao * 100.0
    );
}

/// E o que a pirâmide É: a folha técnica. A amplitude da escada inteira é praticamente a
/// amplitude do custo de pessoal, que é headcount × salário.
#[test]
fn a_piramide_e_a_folha_tecnica() {
    let folha = |categoria: &str, classe: Option<&str>| {
        ancora::parametros(categoria, classe).equipe_fixa
            * temporada::parametros_de_temporada(categoria, classe).salario_medio_anual
    };

    let amplitude_da_folha = amplitude(DIVISOES.iter().map(|(c, k)| folha(c, *k)));
    let amplitude_do_ano = amplitude(decomposicoes().iter().map(|d| d.total()));

    // A folha explica a escada quase inteira: as duas amplitudes ficam a menos de 25% uma
    // da outra.
    let desvio = (amplitude_da_folha / amplitude_do_ano - 1.0).abs();
    assert!(
        desvio < 0.25,
        "folha {amplitude_da_folha:.1}× vs. ano {amplitude_do_ano:.1}× — a pirâmide deixou \
         de ser explicada por pessoal, vale reabrir a hipótese"
    );

    // E ela é o produto de duas coisas do mesmo tipo, não de uma coisa que só existe em cima.
    let pessoas = amplitude(
        DIVISOES
            .iter()
            .map(|(c, k)| ancora::parametros(c, *k).equipe_fixa),
    );
    let salario = amplitude(
        DIVISOES
            .iter()
            .map(|(c, k)| temporada::parametros_de_temporada(c, *k).salario_medio_anual),
    );
    assert!(pessoas > 5.0 && salario > 2.0);
}

/// A conclusão não depende da minha calibração. Mesmo TRIPLICANDO todo custo categórico a
/// escada não chega perto dos 89× da tabela velha — para chegar lá o topo precisaria de
/// dezenas de milhões em contratos que a corrida de cliente não tem.
#[test]
fn nem_triplicar_os_categoricos_reconstroi_a_escada_antiga() {
    let dec = decomposicoes();
    let triplicado = amplitude(
        dec.iter()
            .map(|d| d.total() + 2.0 * d.recorrentes_categoricos),
    );
    let antiga = amplitude_legada();

    assert!(
        triplicado < antiga / 2.5,
        "com categóricos 3× a escada deu {triplicado:.1}×, perto demais dos {antiga:.1}× antigos"
    );
}
