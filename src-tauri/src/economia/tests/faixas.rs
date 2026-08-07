//! Faixas DECLARADAS: cada linha e cada total dentro de uma banda escrita à mão.
//!
//! A regra destes testes é que a banda é uma afirmação sobre o MUNDO, não sobre o código:
//! "um fim de semana de MX-5 queima entre 15 e 40 litros por carro" é verdade ou mentira
//! independentemente do que a tabela de âncoras diz. Se um número da âncora mudar e a
//! banda quebrar, o teste está fazendo o trabalho dele.
//!
//! As bandas são largas de propósito (~±30%): elas existem para pegar erro de unidade,
//! fator de dois e regressão de fórmula — não para congelar a calibração.

use crate::economia::ancora::{self, DIVISOES};
use crate::economia::evento::{
    custo_de_eventos_da_temporada, fatura_da_etapa, COMBUSTIVEL, PNEUS, VIAGEM,
};
use crate::economia::tipos::EntradaDaEtapa;
use crate::finance::planning::category_finance_scale;

/// Uma linha da tabela de bandas. Tudo por ETAPA TÍPICA: equipe mediana, dois carros,
/// pista média, distância continental, prova completa.
struct Banda {
    categoria: &'static str,
    classe: Option<&'static str>,
    /// Litros por CARRO no fim de semana inteiro (treino + quali + corrida).
    litros_por_carro: (f64, f64),
    /// Jogos de pneu por CARRO no fim de semana.
    jogos_por_carro: (f64, f64),
    /// Pessoas na comitiva.
    comitiva: (f64, f64),
    /// Total da fatura da etapa, em reais.
    total_da_etapa: (f64, f64),
    /// Custo de eventos de uma temporada inteira, em reais.
    total_da_temporada: (f64, f64),
}

/// As bandas, com a justificativa física de cada faixa de consumo.
const BANDAS: &[Banda] = &[
    // MX-5 Cup, 2.0 aspirado: uma corrida de 15 min queima ~8 litros; com treino e quali
    // o fim de semana fica na casa de duas dezenas.
    Banda {
        categoria: "mazda_rookie",
        classe: None,
        litros_por_carro: (15.0, 40.0),
        jogos_por_carro: (0.7, 1.5),
        comitiva: (4.0, 7.0),
        total_da_etapa: (6_000.0, 11_000.0),
        total_da_temporada: (28_000.0, 52_000.0),
    },
    Banda {
        categoria: "toyota_rookie",
        classe: None,
        litros_por_carro: (18.0, 45.0),
        jogos_por_carro: (0.7, 1.5),
        comitiva: (4.0, 7.0),
        total_da_etapa: (6_000.0, 11_500.0),
        total_da_temporada: (28_000.0, 54_000.0),
    },
    // Mesmo carro, prova de 25 min e fim de semana mais longo.
    Banda {
        categoria: "mazda_amador",
        classe: None,
        litros_por_carro: (25.0, 55.0),
        jogos_por_carro: (1.5, 2.6),
        comitiva: (6.0, 9.0),
        total_da_etapa: (10_500.0, 19_000.0),
        total_da_temporada: (95_000.0, 170_000.0),
    },
    Banda {
        categoria: "toyota_amador",
        classe: None,
        litros_por_carro: (28.0, 60.0),
        jogos_por_carro: (1.5, 2.6),
        comitiva: (6.0, 9.0),
        total_da_etapa: (11_000.0, 19_500.0),
        total_da_temporada: (98_000.0, 175_000.0),
    },
    // O M2 é a âncora medida do documento: ~40 litros numa corrida de 30 min. A nossa
    // prova tem 25 min, então a corrida sozinha fica em ~33 l e o fim de semana em ~87.
    Banda {
        categoria: "bmw_m2",
        classe: None,
        litros_por_carro: (60.0, 120.0),
        jogos_por_carro: (1.5, 2.6),
        comitiva: (7.0, 10.0),
        total_da_etapa: (15_000.0, 26_000.0),
        total_da_temporada: (130_000.0, 232_000.0),
    },
    Banda {
        categoria: "production_challenger",
        classe: Some("mazda"),
        litros_por_carro: (30.0, 60.0),
        jogos_por_carro: (1.5, 2.6),
        comitiva: (7.0, 10.0),
        total_da_etapa: (12_000.0, 21_500.0),
        total_da_temporada: (132_000.0, 235_000.0),
    },
    Banda {
        categoria: "production_challenger",
        classe: Some("toyota"),
        litros_por_carro: (33.0, 66.0),
        jogos_por_carro: (1.5, 2.6),
        comitiva: (7.0, 10.0),
        total_da_etapa: (12_500.0, 22_000.0),
        total_da_temporada: (135_000.0, 242_000.0),
    },
    Banda {
        categoria: "production_challenger",
        classe: Some("bmw"),
        litros_por_carro: (70.0, 135.0),
        jogos_por_carro: (1.5, 2.6),
        comitiva: (7.0, 10.0),
        total_da_etapa: (15_000.0, 26_500.0),
        total_da_temporada: (162_000.0, 289_000.0),
    },
    // GT4: ~430 cv com aerodinâmica. Um stint de GT4 gasta ~50 l em 80 km.
    Banda {
        categoria: "gt4",
        classe: None,
        litros_por_carro: (90.0, 175.0),
        jogos_por_carro: (1.5, 2.6),
        comitiva: (10.0, 14.0),
        total_da_etapa: (22_000.0, 39_000.0),
        total_da_temporada: (240_000.0, 425_000.0),
    },
    // GT3: um stint real são ~120 litros para ~170 km. Prova de 50 min mais treino e
    // quali colocam o fim de semana em torno de 230 l por carro.
    Banda {
        categoria: "gt3",
        classe: None,
        litros_por_carro: (180.0, 300.0),
        jogos_por_carro: (2.3, 3.8),
        comitiva: (15.0, 21.0),
        total_da_etapa: (41_000.0, 73_000.0),
        total_da_temporada: (610_000.0, 1_080_000.0),
    },
    // LMP2: mais potente que o GT3 e mesmo assim mais econômico por quilômetro — é o que
    // a aerodinâmica de protótipo faz. O fim de semana é maior porque a prova é de 60 min.
    Banda {
        categoria: "lmp2",
        classe: None,
        litros_por_carro: (220.0, 350.0),
        jogos_por_carro: (2.3, 3.8),
        comitiva: (19.0, 26.0),
        total_da_etapa: (51_000.0, 90_000.0),
        total_da_temporada: (546_000.0, 975_000.0),
    },
    // Endurance: prova média de 225 min. É aqui que a economia velha errava mais feio —
    // ela cobrava ~188.000 de "gasolina" numa etapa que consome ~1.300 litros.
    Banda {
        categoria: "endurance",
        classe: Some("gt4"),
        litros_por_carro: (380.0, 650.0),
        jogos_por_carro: (4.5, 7.5),
        comitiva: (13.0, 19.0),
        total_da_etapa: (51_000.0, 91_000.0),
        total_da_temporada: (300_000.0, 535_000.0),
    },
    Banda {
        categoria: "endurance",
        classe: Some("gt3"),
        litros_por_carro: (500.0, 850.0),
        jogos_por_carro: (6.0, 10.0),
        comitiva: (19.0, 26.0),
        total_da_etapa: (76_000.0, 135_000.0),
        total_da_temporada: (445_000.0, 795_000.0),
    },
    Banda {
        categoria: "endurance",
        classe: Some("lmp2"),
        litros_por_carro: (530.0, 900.0),
        jogos_por_carro: (7.0, 11.0),
        comitiva: (22.0, 31.0),
        total_da_etapa: (95_000.0, 169_000.0),
        total_da_temporada: (557_000.0, 995_000.0),
    },
];

fn rotulo(b: &Banda) -> String {
    match b.classe {
        Some(c) => format!("{}:{c}", b.categoria),
        None => b.categoria.to_string(),
    }
}

fn dentro(valor: f64, faixa: (f64, f64), rotulo: &str, grandeza: &str) {
    assert!(
        valor >= faixa.0 && valor <= faixa.1,
        "{rotulo}: {grandeza} deu {valor:.1}, fora da faixa declarada [{:.1}, {:.1}]",
        faixa.0,
        faixa.1
    );
}

/// A banda tem que cobrir toda divisão da tabela — senão uma divisão nova entra sem
/// faixa nenhuma e passa despercebida.
#[test]
fn toda_divisao_tem_banda_declarada() {
    assert_eq!(BANDAS.len(), DIVISOES.len());
    for (categoria, classe) in DIVISOES {
        assert!(
            BANDAS
                .iter()
                .any(|b| b.categoria == categoria && b.classe == classe),
            "divisão {categoria}:{classe:?} sem banda declarada"
        );
    }
}

/// O consumo é a linha que o documento aponta como a mais errada da economia velha
/// (~50× acima do real). Esta é a asserção física direta: quantos litros o fim de semana
/// cabe no tanque.
#[test]
fn combustivel_em_litros_por_carro() {
    for b in BANDAS {
        let entrada = EntradaDaEtapa::tipica(b.categoria, b.classe);
        let carros = entrada.equipe.carros_inscritos as f64;
        let fatura = fatura_da_etapa(&entrada);
        let litros = fatura.linha(COMBUSTIVEL).unwrap().quantidade / carros;
        dentro(litros, b.litros_por_carro, &rotulo(b), "litros por carro");
    }
}

#[test]
fn jogos_de_pneu_por_carro() {
    for b in BANDAS {
        let entrada = EntradaDaEtapa::tipica(b.categoria, b.classe);
        let carros = entrada.equipe.carros_inscritos as f64;
        let fatura = fatura_da_etapa(&entrada);
        let jogos = fatura.linha(PNEUS).unwrap().quantidade / carros;
        dentro(jogos, b.jogos_por_carro, &rotulo(b), "jogos por carro");
    }
}

#[test]
fn tamanho_da_comitiva() {
    for b in BANDAS {
        let fatura = fatura_da_etapa(&EntradaDaEtapa::tipica(b.categoria, b.classe));
        let pessoas = fatura.linha(VIAGEM).unwrap().quantidade;
        dentro(pessoas, b.comitiva, &rotulo(b), "pessoas na comitiva");
    }
}

#[test]
fn total_da_etapa_tipica() {
    for b in BANDAS {
        let total = fatura_da_etapa(&EntradaDaEtapa::tipica(b.categoria, b.classe)).total();
        dentro(total, b.total_da_etapa, &rotulo(b), "total da etapa");
    }
}

/// A temporada inteira de cada divisão: a soma das etapas do campeonato sob uma mistura
/// de viagem realista. É o número que a seção 3.2 do redesign chama de custo operacional
/// de referência — só que agora ele é CONSEQUÊNCIA do modelo, não entrada dele.
#[test]
fn custo_de_eventos_da_temporada_inteira() {
    for b in BANDAS {
        let total = custo_de_eventos_da_temporada(b.categoria, b.classe);
        dentro(total, b.total_da_temporada, &rotulo(b), "temporada");
    }
}

/// Nenhuma linha pode carregar a fatura sozinha. O limite é 50% porque em prova longa o
/// pneu DEVE dominar — uma etapa de 4 horas gasta jogo por stint, e esconder isso seria
/// voltar a espalhar peso por rótulo.
#[test]
fn nenhuma_linha_domina_a_fatura() {
    for (categoria, classe) in DIVISOES {
        let fatura = fatura_da_etapa(&EntradaDaEtapa::tipica(categoria, classe));
        let total = fatura.total();
        for l in &fatura.linhas {
            let fatia = l.total() / total;
            assert!(
                fatia < 0.50,
                "{categoria}:{classe:?} linha '{}' ficou com {:.0}% da fatura",
                l.chave,
                fatia * 100.0
            );
        }
    }
}

// ── Onde foi parar a comparação com a âncora antiga ──────────────────────────────────
//
// Ela subiu para `super::temporada`. Enquanto este módulo só tinha eventos, comparar com
// `operating_cost_midpoint` exigia uma ponte (multiplicar o midpoint por 0,62, a fatia que
// a fatura velha cobrava do orçamento da rodada) — e essa ponte vinha do próprio modelo sob
// suspeita. Com `economia::temporada` os dois lados cobrem o ano inteiro e a comparação é
// direta, total contra total. A ponte morreu junto com a necessidade dela.

/// A tabela de âncoras não pode ter buraco: todo parâmetro físico é estritamente positivo,
/// em toda divisão. Um zero aqui produz uma etapa de graça sem quebrar nada.
#[test]
fn nenhum_parametro_fisico_e_nulo() {
    for (categoria, classe) in DIVISOES {
        let p = ancora::parametros(categoria, classe);
        for (nome, valor) in [
            ("velocidade", p.velocidade_media_kmh),
            ("duracao", p.duracao_corrida_min),
            ("km_treino_quali", p.km_treino_quali),
            ("consumo", p.consumo_l_por_km),
            ("jogos_de_pneu_fixos", p.jogos_de_pneu_fixos),
            ("km_por_jogo_de_corrida", p.km_por_jogo_de_corrida),
            ("preco_do_jogo", p.preco_do_jogo_de_pneu),
            ("revisao_por_km", p.revisao_por_km),
            ("comitiva", p.comitiva),
            ("equipe_fixa", p.equipe_fixa),
            ("tarifa_frete", p.tarifa_frete_por_km),
            ("inscricao", p.taxa_de_inscricao),
            ("noites", p.noites_de_hotel),
        ] {
            assert!(valor > 0.0, "{categoria}:{classe:?} tem {nome} = {valor}");
        }
    }
}
