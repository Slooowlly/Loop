//! A duração real da prova como entrada.
//!
//! Na primeira versão deste módulo o fim de semana de endurance ficava congelado nos 225
//! minutos da média, então uma prova de 6 horas e uma de 2 cobravam a mesma borracha e o
//! mesmo programa de treinos. As duas etapas mais diferentes do calendário custavam igual.

use crate::economia::ancora;
use crate::economia::evento::{duracoes_possiveis, fatura_da_etapa, COMBUSTIVEL, PNEUS, REVISAO};
use crate::economia::tipos::EntradaDaEtapa;

/// As durações que o modelo considera são as que o calendário sorteia de verdade
/// (`calendar::montagem::resolve_race_duration`): uma só nas categorias de sprint, quatro
/// no endurance.
#[test]
fn as_duracoes_espelham_o_que_o_calendario_sorteia() {
    assert_eq!(duracoes_possiveis("gt3", None), vec![50.0]);
    assert_eq!(duracoes_possiveis("mazda_rookie", None), vec![15.0]);
    assert_eq!(
        duracoes_possiveis("endurance", Some("gt3")),
        vec![120.0, 180.0, 240.0, 360.0]
    );
}

/// Uma prova de 6 horas custa mais que uma de 2 na MESMA divisão. É o teste que a versão
/// congelada em 225 min não passava — lá as duas custavam exatamente igual.
///
/// A razão do TOTAL é ~1,44×, não 3×, e isso está certo: os ~55 mil de aparecer na etapa
/// (frete, inscrição, comitiva, hotel, diárias) não sabem quantas horas a corrida dura. O
/// que triplica é o bloco de corrida, e é lá que a asserção morde.
///
/// Limitação conhecida: a taxa de inscrição fica congelada na duração de referência. Uma
/// inscrição de 6 horas é mais cara que uma de 2 no mundo real; aqui não é. Fica na conta
/// da parte que não escala, o que subestima um pouco a diferença entre as duas provas.
#[test]
fn prova_longa_custa_mais_que_prova_curta() {
    let curta = fatura_da_etapa(&EntradaDaEtapa::tipica_com_duracao(
        "endurance",
        Some("gt3"),
        120.0,
    ));
    let longa = fatura_da_etapa(&EntradaDaEtapa::tipica_com_duracao(
        "endurance",
        Some("gt3"),
        360.0,
    ));

    let razao = longa.total() / curta.total();
    assert!(
        (1.35..1.75).contains(&razao),
        "6 horas custou {razao:.2}× as 2 horas"
    );

    // Combustível e revisão são quilometragem pura: escalam com a prova, 3×.
    for chave in [COMBUSTIVEL, REVISAO] {
        let r = longa.valor(chave) / curta.valor(chave);
        assert!(
            (2.2..3.2).contains(&r),
            "linha '{chave}' escalou {r:.2}× para uma prova 3× mais longa"
        );
    }

    // Pneu escala quase tanto: a parcela de corrida triplica, a de treino e quali não.
    let pneu = longa.valor(PNEUS) / curta.valor(PNEUS);
    assert!(
        (2.0..3.0).contains(&pneu),
        "pneu escalou {pneu:.2}× para uma prova 3× mais longa"
    );
}

/// A inscrição de prova longa é mais cara que a de prova curta, e também de forma
/// amortecida: parte da taxa é o direito de estar no grid e não sabe quantas horas a
/// corrida dura.
#[test]
fn a_inscricao_cresce_amortecida_com_a_duracao() {
    let p = ancora::parametros("endurance", Some("gt3"));

    assert!(
        (p.taxa_de_inscricao_em(225.0) - p.taxa_de_inscricao).abs() < 1e-9,
        "na duração de referência a inscrição é a da tabela"
    );
    assert!(p.taxa_de_inscricao_em(120.0) < p.taxa_de_inscricao);
    assert!(p.taxa_de_inscricao_em(360.0) > p.taxa_de_inscricao);

    // Prova 3× maior cobra entre 1,5× e 2,3× a inscrição — nunca 3×.
    let razao = p.taxa_de_inscricao_em(360.0) / p.taxa_de_inscricao_em(120.0);
    assert!(
        (1.5..2.3).contains(&razao),
        "inscrição escalou {razao:.2}× para uma prova 3× mais longa"
    );

    // No sprint a duração é única, então a inscrição nunca sai do valor de tabela.
    for categoria in ["mazda_rookie", "bmw_m2", "gt3"] {
        let sp = ancora::parametros(categoria, None);
        assert!(
            (sp.taxa_de_inscricao_em(sp.duracao_corrida_min) - sp.taxa_de_inscricao).abs() < 1e-9
        );
    }
}

/// O programa de treinos cresce com a prova, mas MUITO menos que proporcionalmente: uma
/// prova três vezes maior não tem três vezes mais treino livre.
#[test]
fn o_treino_cresce_amortecido_com_a_duracao() {
    let p = ancora::parametros("endurance", Some("gt3"));
    let curto = p.km_treino_quali_em(120.0);
    let referencia = p.km_treino_quali_em(225.0);
    let longo = p.km_treino_quali_em(360.0);

    assert!(curto < referencia && referencia < longo);
    assert!(
        (referencia - p.km_treino_quali).abs() < 1e-9,
        "na duração de referência o treino tem que ser exatamente o da tabela"
    );

    // Prova 3× maior traz menos de 60% a mais de treino.
    let crescimento = longo / curto;
    assert!(
        crescimento < 1.6,
        "treino cresceu {crescimento:.2}× para uma prova 3× maior — amortecimento sumiu"
    );
}

/// Nas categorias de sprint a duração é única, então a etapa típica tem que continuar
/// idêntica à da tabela — a mudança não pode ter deslocado a escada de sprint sem querer.
#[test]
fn sprint_nao_se_mexeu_com_a_duracao_virando_entrada() {
    for categoria in ["mazda_rookie", "bmw_m2", "gt4", "gt3"] {
        let p = ancora::parametros(categoria, None);
        let tipica = EntradaDaEtapa::tipica(categoria, None);
        assert_eq!(tipica.duracao_corrida_min, p.duracao_corrida_min);

        let fatura = fatura_da_etapa(&tipica);
        // Jogos de pneu na etapa de referência = fixos + km_corrida/km_por_jogo, por carro.
        let esperado = (p.jogos_de_pneu_fixos
            + p.km_de_corrida() / p.km_por_jogo_de_corrida * (0.90 + 0.15 * 0.65))
            * tipica.equipe.carros_inscritos as f64;
        let jogos = fatura.linha(PNEUS).unwrap().quantidade;
        assert!(
            (jogos - esperado).abs() < 0.05,
            "{categoria}: {jogos:.2} jogos, esperado {esperado:.2}"
        );
    }
}

/// A temporada de endurance é orçada sobre as quatro durações possíveis, não sobre uma
/// prova média que nunca acontece. O custo tem que cair entre o da prova mais curta e o da
/// mais longa, vezes o número de etapas.
#[test]
fn a_temporada_de_endurance_usa_a_mistura_de_duracoes() {
    let etapas = ancora::etapas_por_temporada("endurance");
    let temporada =
        crate::economia::evento::custo_de_eventos_da_temporada("endurance", Some("gt3"));

    let etapa_com = |min: f64| {
        fatura_da_etapa(&EntradaDaEtapa::tipica_com_duracao(
            "endurance",
            Some("gt3"),
            min,
        ))
        .total()
    };

    assert!(temporada > etapa_com(120.0) * etapas);
    assert!(temporada < etapa_com(360.0) * etapas);
}
