//! Testes de FORMA: o que a fatura tem que fazer independentemente da calibração.
//!
//! Estes sobrevivem a qualquer mexida nos números da âncora. Os que fixam magnitude
//! moram em [`super::faixas`].

use crate::economia::ancora::{self, DIVISOES};
use crate::economia::evento::{
    fatura_da_etapa, km_faturados_de_frete, preco_da_passagem, COMBUSTIVEL, DIARIAS, ESTADIA,
    FRETE, INSCRICAO, PNEUS, REVISAO, VIAGEM,
};
use crate::economia::tipos::{
    Bloco, EntradaDaEtapa, DISTANCIA_CASA_KM, DISTANCIA_CONTINENTAL_KM,
    DISTANCIA_INTERCONTINENTAL_KM,
};

/// O contrato central do módulo: `total` é sempre `quantidade × preço_unitário`. É isso
/// que impede a linha de voltar a ser um peso de orçamento vestindo o nome de um objeto.
#[test]
fn toda_linha_e_quantidade_vezes_preco() {
    for (categoria, classe) in DIVISOES {
        let fatura = fatura_da_etapa(&EntradaDaEtapa::tipica(categoria, classe));
        for l in &fatura.linhas {
            let esperado = l.quantidade * l.preco_unitario;
            assert!(
                (l.total() - esperado).abs() < 1e-9,
                "{categoria:?}/{classe:?} linha '{}' não fecha",
                l.chave
            );
            assert!(
                l.quantidade > 0.0 && l.preco_unitario > 0.0,
                "{categoria:?}/{classe:?} linha '{}' com quantidade ou preço nulo",
                l.chave
            );
        }
        assert!(
            (fatura.total() - fatura.linhas.iter().map(|l| l.total()).sum::<f64>()).abs() < 1e-9
        );
    }
}

/// Toda divisão do jogo tem que estar na tabela. Se uma categoria nova entrar sem linha
/// própria ela cai no fallback de `bmw_m2` silenciosamente — este teste é o alarme.
#[test]
fn a_tabela_cobre_todas_as_divisoes_e_nenhuma_cai_no_fallback() {
    let bmw = ancora::parametros("bmw_m2", None);
    for (categoria, classe) in DIVISOES {
        if categoria == "bmw_m2" {
            continue;
        }
        let p = ancora::parametros(categoria, classe);
        assert_ne!(
            p, bmw,
            "divisão {categoria}:{classe:?} está caindo no fallback de bmw_m2"
        );
    }
    // E o fallback continua existindo para dado sujo.
    assert_eq!(ancora::parametros("inexistente", None), bmw);
    assert_eq!(
        ancora::parametros("gt3", Some("classe_que_nao_existe")),
        bmw
    );
}

/// As oito linhas do desenho, nos três blocos de despesa da seção 3.7.
#[test]
fn a_fatura_tem_as_oito_linhas_nos_tres_blocos() {
    let fatura = fatura_da_etapa(&EntradaDaEtapa::tipica("gt3", None));
    for chave in [
        COMBUSTIVEL,
        PNEUS,
        REVISAO,
        FRETE,
        VIAGEM,
        ESTADIA,
        INSCRICAO,
        DIARIAS,
    ] {
        assert!(fatura.linha(chave).is_some(), "faltou a linha '{chave}'");
    }
    assert_eq!(fatura.linhas.len(), 8);
    for bloco in [Bloco::Corrida, Bloco::Logistica, Bloco::Equipe] {
        assert!(fatura.total_do_bloco(bloco) > 0.0, "bloco {bloco:?} vazio");
    }
}

/// A escada tem que subir monotonicamente no sprint: cada degrau custa mais que o
/// anterior. Isto não é calibração contra a tabela velha — é a ordem que o próprio
/// modelo físico produz (mais potência, mais pneu, mais gente).
#[test]
fn o_custo_da_etapa_sobe_a_escada() {
    let escada = [
        "mazda_rookie",
        "mazda_amador",
        "bmw_m2",
        "gt4",
        "gt3",
        "lmp2",
    ];
    let mut anterior = 0.0;
    for categoria in escada {
        let total = fatura_da_etapa(&EntradaDaEtapa::tipica(categoria, None)).total();
        assert!(
            total > anterior,
            "{categoria} custa {total:.0}, não mais que o degrau anterior ({anterior:.0})"
        );
        anterior = total;
    }
}

/// Uma etapa de Endurance custa mais que um sprint da MESMA classe de carro — mas MUITO
/// menos que a proporção da distância, e essa desproporção é um resultado do modelo, não
/// um erro dele.
///
/// A prova de endurance é 4,5× mais longa que a de GT3 (225 min contra 50), e mesmo assim
/// a fatura sobe só ~1,9×. O motivo é que boa parte da conta é o custo de APARECER na
/// etapa — frete, inscrição, comitiva, hotel — e isso não sabe quantas horas a corrida
/// dura. É o mesmo motivo pelo qual uma temporada de 6 etapas de endurance sai mais barata
/// que uma de 14 etapas de GT3, apesar de cada etapa ser maior.
#[test]
fn endurance_custa_mais_que_o_sprint_mas_menos_que_a_distancia() {
    let sprint = fatura_da_etapa(&EntradaDaEtapa::tipica("gt3", None));
    let enduro = fatura_da_etapa(&EntradaDaEtapa::tipica("endurance", Some("gt3")));

    let razao_de_distancia = ancora::parametros("endurance", Some("gt3")).km_de_corrida()
        / ancora::parametros("gt3", None).km_de_corrida();
    let razao = enduro.total() / sprint.total();

    assert!(
        (1.6..3.0).contains(&razao),
        "endurance/gt3 deu {razao:.2}× o sprint de gt3"
    );
    assert!(
        razao < razao_de_distancia,
        "a fatura subiu {razao:.2}× para uma prova {razao_de_distancia:.2}× mais longa — \
         o custo de aparecer na etapa deveria amortecer isso"
    );

    // E o combustível — que é puro quilômetro — tem que subir bem mais que o total,
    // porque a logística e a comitiva não escalam com a duração da prova.
    let razao_combustivel = enduro.valor(COMBUSTIVEL) / sprint.valor(COMBUSTIVEL);
    assert!(
        razao_combustivel > razao,
        "combustível subiu {razao_combustivel:.2}× e o total {razao:.2}× — a distância não está mandando"
    );
}

/// Abandonar cedo economiza combustível e pneu de corrida, mas não devolve o que treino e
/// classificação já queimaram, nem a inscrição, nem o hotel.
#[test]
fn abandono_encurta_a_corrida_e_nao_a_logistica() {
    let mut abandono = EntradaDaEtapa::tipica("gt3", None);
    abandono.corrida.voltas_completadas = abandono.corrida.voltas_da_prova * 0.1;
    abandono.corrida.desgaste_final_pneus = 0.1;

    let inteira = fatura_da_etapa(&EntradaDaEtapa::tipica("gt3", None));
    let curta = fatura_da_etapa(&abandono);

    assert!(curta.valor(COMBUSTIVEL) < inteira.valor(COMBUSTIVEL));
    assert!(curta.valor(PNEUS) < inteira.valor(PNEUS));
    assert!(curta.valor(REVISAO) < inteira.valor(REVISAO));
    assert_eq!(curta.valor(INSCRICAO), inteira.valor(INSCRICAO));
    assert_eq!(curta.valor(ESTADIA), inteira.valor(ESTADIA));
    assert_eq!(curta.valor(FRETE), inteira.valor(FRETE));

    // Treino e quali seguram um piso: mesmo abandonando na volta 1 sobra combustível pago.
    let p = ancora::parametros("gt3", None);
    let piso = p.km_treino_quali * 2.0 * p.consumo_l_por_km;
    assert!(
        curta.linha(COMBUSTIVEL).unwrap().quantidade > piso,
        "abandono não pode zerar o combustível de treino e classificação"
    );
}

/// Distância da etapa move logística e nada mais. É o que separa "onde a corrida foi" de
/// "o que o carro fez".
#[test]
fn distancia_move_so_a_logistica() {
    let com = |d: f64| {
        let mut e = EntradaDaEtapa::tipica("gt4", None);
        e.pista.distancia_da_sede_km = d;
        fatura_da_etapa(&e)
    };
    let casa = com(DISTANCIA_CASA_KM);
    let cont = com(DISTANCIA_CONTINENTAL_KM);
    let longe = com(DISTANCIA_INTERCONTINENTAL_KM);

    assert!(casa.total() < cont.total() && cont.total() < longe.total());
    assert_eq!(casa.valor(COMBUSTIVEL), longe.valor(COMBUSTIVEL));
    assert_eq!(casa.valor(PNEUS), longe.valor(PNEUS));
    assert_eq!(
        casa.total_do_bloco(Bloco::Equipe),
        longe.total_do_bloco(Bloco::Equipe)
    );
    assert!(casa.total_do_bloco(Bloco::Logistica) < longe.total_do_bloco(Bloco::Logistica));
}

/// O frete de uma etapa intercontinental não pode ser frete rodoviário vezes 8.500 km —
/// acima do limite o modal muda e o preço por quilômetro cai.
#[test]
fn frete_longo_e_degressivo() {
    let curto = km_faturados_de_frete(DISTANCIA_CASA_KM);
    let longo = km_faturados_de_frete(DISTANCIA_INTERCONTINENTAL_KM);

    assert_eq!(
        curto,
        DISTANCIA_CASA_KM * 2.0,
        "abaixo do limite é ida e volta cheia"
    );
    assert!(longo < DISTANCIA_INTERCONTINENTAL_KM * 2.0);
    // 1.500 cheios + 45% dos 7.000 restantes, ida e volta.
    assert!((longo - (1_500.0 + 7_000.0 * 0.45) * 2.0).abs() < 1e-6);
}

/// A comitiva é gente: número inteiro, nunca fracionário, nunca menos que dois.
#[test]
fn a_comitiva_e_de_pessoas_inteiras() {
    for pit_crew in [0.0, 25.0, 50.0, 75.0, 100.0] {
        for (categoria, classe) in DIVISOES {
            let mut e = EntradaDaEtapa::tipica(categoria, classe);
            e.equipe.qualidade_pit_crew = pit_crew;
            let fatura = fatura_da_etapa(&e);
            let pessoas = fatura.linha(VIAGEM).unwrap().quantidade;
            assert_eq!(
                pessoas,
                pessoas.round(),
                "comitiva fracionária em {categoria}"
            );
            assert!(pessoas >= 2.0);
        }
    }
}

/// Equipe sem carro na pista não paga fatura de etapa. Caso de borda de equipe dissolvida
/// no meio da temporada.
#[test]
fn equipe_sem_carro_nao_tem_fatura() {
    let mut e = EntradaDaEtapa::tipica("gt3", None);
    e.equipe.carros_inscritos = 0;
    let fatura = fatura_da_etapa(&e);
    assert!(fatura.linhas.is_empty());
    assert_eq!(fatura.total(), 0.0);
}

/// Dobrar os carros dobra o que é por carro e não mexe no que é por operação.
#[test]
fn carro_extra_dobra_o_consumivel_e_nao_a_comitiva() {
    let um = {
        let mut e = EntradaDaEtapa::tipica("gt4", None);
        e.equipe.carros_inscritos = 1;
        fatura_da_etapa(&e)
    };
    let dois = fatura_da_etapa(&EntradaDaEtapa::tipica("gt4", None));

    for chave in [COMBUSTIVEL, PNEUS, REVISAO, INSCRICAO] {
        assert!(
            (dois.valor(chave) - um.valor(chave) * 2.0).abs() < 1e-6,
            "linha '{chave}' não dobrou com o segundo carro"
        );
    }
    assert_eq!(dois.valor(FRETE), um.valor(FRETE));
    assert_eq!(dois.valor(VIAGEM), um.valor(VIAGEM));
}

/// A passagem tem parte fixa e parte por quilômetro — viajar mais longe custa mais, e
/// ninguém viaja de graça.
#[test]
fn passagem_cresce_com_a_distancia() {
    assert!(preco_da_passagem(0.0) > 0.0);
    assert!(preco_da_passagem(DISTANCIA_CASA_KM) < preco_da_passagem(DISTANCIA_CONTINENTAL_KM));
    assert!(
        preco_da_passagem(DISTANCIA_CONTINENTAL_KM)
            < preco_da_passagem(DISTANCIA_INTERCONTINENTAL_KM)
    );
}

/// Numa etapa multi-classe o CARRO manda no consumo e no pneu, o CAMPEONATO manda na
/// prova e na inscrição. Um MX-5 na Production bebe como MX-5.
#[test]
fn multiclasse_separa_o_carro_do_campeonato() {
    let mazda_prod = ancora::parametros("production_challenger", Some("mazda"));
    let bmw_prod = ancora::parametros("production_challenger", Some("bmw"));
    let mazda_solo = ancora::parametros("mazda_amador", None);
    let bmw_solo = ancora::parametros("bmw_m2", None);

    // O carro é o mesmo: consumo e pneu vêm da classe, não do campeonato.
    assert_eq!(mazda_prod.consumo_l_por_km, mazda_solo.consumo_l_por_km);
    assert_eq!(bmw_prod.consumo_l_por_km, bmw_solo.consumo_l_por_km);
    assert_eq!(
        mazda_prod.preco_do_jogo_de_pneu,
        mazda_solo.preco_do_jogo_de_pneu
    );
    // O campeonato é o mesmo: prova e inscrição são iguais entre as classes.
    assert_eq!(mazda_prod.duracao_corrida_min, bmw_prod.duracao_corrida_min);
    assert_eq!(mazda_prod.taxa_de_inscricao, bmw_prod.taxa_de_inscricao);
    // E a prova da Production é mais longa que a do BMW solo, então a etapa custa mais.
    assert!(bmw_prod.duracao_corrida_min > bmw_solo.duracao_corrida_min);
}

/// As voltas de referência não podem herdar o grampo de 50 de
/// `calendar::montagem::estimate_laps` — numa prova de 3h45 isso seria uma mentira física.
#[test]
fn voltas_de_endurance_passam_do_grampo_do_calendario() {
    let voltas = ancora::voltas_de_referencia("endurance", Some("gt3"));
    assert!(
        voltas > 100.0,
        "prova de 225 min a 175 km/h deu {voltas:.0} voltas"
    );
}
