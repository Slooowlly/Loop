//! Régua da FATURA VISÍVEL: a fatura de uma etapa lida em voz alta, degrau a degrau.
//!
//! ```text
//! cargo test --lib relatorio_da_fatura_visivel -- --ignored --nocapture
//! ```
//!
//! **A pergunta que ele existe para responder:** cada linha, com o rótulo que o jogador
//! lê, tem ordem de grandeza defensável? Combustível de uma etapa de BMW M2 é ~$589 —
//! se sair $9.400, o rótulo e o número estão contando histórias diferentes, que é
//! exatamente o defeito que originou o redesign.
//!
//! Não asserta calibração: imprime. O que ele ASSERTA é a forma (4 blocos, no máximo 8
//! linhas de despesa) e a invariante de que nada visível vale zero — isso está nos
//! testes de `economia::fatura`.

use std::collections::HashMap;

use crate::constants::categories::get_category_config;
use crate::economia::evento::fatura_da_etapa;
use crate::economia::fatura::{
    fatura_visivel, temporada_tipica, BlocoDaFatura, EntradaDaFatura, FaturaVisivel,
};
use crate::economia::receita::ReceitaDaEtapa;
use crate::economia::tipos::EntradaDaEtapa;

/// Um degrau da escada, com a classe quando a categoria é multi-classe.
const DEGRAUS: [(&str, Option<&str>); 9] = [
    ("mazda_rookie", None),
    ("mazda_amador", None),
    ("bmw_m2", None),
    ("production_challenger", Some("bmw")),
    ("gt4", None),
    ("gt3", None),
    ("lmp2", None),
    ("endurance", Some("gt3")),
    ("endurance", Some("lmp2")),
];

fn etapas_de(categoria: &str) -> f64 {
    get_category_config(categoria)
        .map(|c| f64::from(c.corridas_por_temporada.max(1)))
        .unwrap_or(10.0)
}

fn monta(categoria: &str, classe: Option<&str>) -> FaturaVisivel {
    let etapa = fatura_da_etapa(&EntradaDaEtapa::tipica(categoria, classe));
    let temporada = temporada_tipica(categoria, classe);
    fatura_visivel(&EntradaDaFatura {
        etapa: &etapa,
        temporada: &temporada,
        etapas_na_temporada: etapas_de(categoria),
        // Receita zerada: esta régua é da DESPESA, que é onde mora a queixa de
        // ordem de grandeza. A receita tem harness próprio.
        receita: ReceitaDaEtapa::default(),
        peca_comprada: 0.0,
        folha_de_pilotos_anual: None,
    })
}

fn rotulo(categoria: &str, classe: Option<&str>) -> String {
    match classe {
        Some(c) => format!("{categoria}:{c}"),
        None => categoria.to_string(),
    }
}

/// **O efeito da geografia real no frete, antes e depois.**
///
/// ```text
/// cargo test --lib relatorio_do_frete_por_destino -- --ignored --nocapture
/// ```
///
/// O defeito que originou a troca apareceu na tela, não no modelo: uma etapa em Snetterton
/// (Reino Unido) e outra em Rudskogen (Noruega) cobraram os MESMOS 9.679,4 km de frete,
/// porque as duas caíam na faixa "outro continente" de uma equipe brasileira. Duas corridas
/// em países diferentes com a conta idêntica ensinam o jogador que o número é decorativo.
///
/// Este relatório mede o que cada destino passa a custar, e quanto o empate valia.
#[test]
#[ignore = "régua de medição — roda com --ignored --nocapture"]
fn relatorio_do_frete_por_destino() {
    use crate::constants::geografia;
    use crate::economia::evento::km_faturados_de_frete;
    use crate::economia::tipos::{
        DISTANCIA_CASA_KM, DISTANCIA_CONTINENTAL_KM, DISTANCIA_INTERCONTINENTAL_KM,
    };

    // A faixa antiga: casa / mesmo continente / outro continente.
    fn faixa_antiga(sede: &str, destino: &str) -> f64 {
        if geografia::mesmo_pais(sede, destino) {
            return DISTANCIA_CASA_KM;
        }
        let europa = |p: &str| {
            [
                "Reino Unido",
                "Alemanha",
                "França",
                "Itália",
                "Espanha",
                "Holanda",
                "Bélgica",
                "Portugal",
                "Áustria",
                "Noruega",
                "Hungria",
            ]
            .iter()
            .any(|e| geografia::mesmo_pais(p, e))
        };
        let mesmo_continente = (europa(sede) && europa(destino))
            || (geografia::mesmo_pais(sede, "EUA") && geografia::mesmo_pais(destino, "Canadá"));
        if mesmo_continente {
            DISTANCIA_CONTINENTAL_KM
        } else {
            DISTANCIA_INTERCONTINENTAL_KM
        }
    }

    fn empates_por_sede<'a>(
        mapa: &'a mut HashMap<String, HashMap<u64, u32>>,
        sede: &str,
    ) -> &'a mut HashMap<u64, u32> {
        mapa.entry(sede.to_string()).or_default()
    }

    println!("\n=== FRETE POR DESTINO: faixa × distância real ===\n");
    // Preço por km de frete da GT3, para traduzir km em dinheiro.
    let tarifa = crate::economia::ancora::parametros("gt3", None).tarifa_frete_por_km;

    for sede in ["🇧🇷 Brasil", "🇬🇧 Reino Unido", "🇺🇸 EUA"] {
        println!("── Sede: {sede} ──────────────────────────────────────────────");
        println!(
            "{:<20} {:>12} {:>12} {:>12} {:>14}",
            "destino", "faixa (km)", "real (km)", "faturados", "frete GT3"
        );
        println!("{}", "─".repeat(74));
        for destino in [
            "🇬🇧 Reino Unido",
            "🇳🇴 Noruega",
            "🇮🇹 Itália",
            "🇺🇸 EUA",
            "🇯🇵 Japão",
            "🇦🇺 Austrália",
            "🇧🇷 Brasil",
        ] {
            let antiga = faixa_antiga(sede, destino);
            let real = if geografia::mesmo_pais(sede, destino) {
                DISTANCIA_CASA_KM
            } else {
                geografia::distancia_entre_paises_km(sede, destino)
                    .map(|km| km.max(DISTANCIA_CASA_KM))
                    .unwrap_or(antiga)
            };
            let faturados = km_faturados_de_frete(real);
            println!(
                "{:<20} {:>12.0} {:>12.0} {:>12.0} {:>14}",
                destino,
                antiga,
                real,
                faturados,
                format!("${:.0}", faturados * tarifa)
            );
        }
        println!();
    }

    println!(
        "O empate que sumiu: com sede no Brasil, Reino Unido e Noruega davam os MESMOS\n\
         {DISTANCIA_INTERCONTINENTAL_KM:.0} km de faixa. Agora dão distâncias diferentes, e a\n\
         diferença aparece na linha de frete da fatura.\n"
    );

    // ── O efeito AGREGADO, sobre os pares que existem de verdade ────────────────────
    // Trocar faixa por distância não é neutro no nível: a faixa intercontinental era um
    // número só para tudo que cruzava oceano. Este bloco diz se o mundo ficou mais caro
    // ou mais barato, e quantos pares saíam empatados.
    let mut pares = 0u32;
    let mut soma_antiga = 0.0;
    let mut soma_nova = 0.0;
    let mut empates_desfeitos: HashMap<String, HashMap<u64, u32>> = HashMap::new();
    for equipe in crate::constants::teams::get_all_team_templates() {
        for pista in crate::constants::tracks::get_all_tracks() {
            let antiga = faixa_antiga(equipe.pais_sede, pista.pais);
            let nova = if geografia::mesmo_pais(equipe.pais_sede, pista.pais) {
                DISTANCIA_CASA_KM
            } else {
                geografia::distancia_entre_paises_km(equipe.pais_sede, pista.pais)
                    .map(|km| km.max(DISTANCIA_CASA_KM))
                    .unwrap_or(antiga)
            };
            pares += 1;
            soma_antiga += antiga;
            soma_nova += nova;
            *empates_por_sede(&mut empates_desfeitos, equipe.pais_sede)
                .entry(nova.round() as u64)
                .or_insert(0) += 1;
        }
    }
    let n = pares.max(1) as f64;
    // Quantos destinos DISTINTOS cada sede enxergava antes (no máximo 3: casa,
    // continente, outro continente) e quantos enxerga agora.
    let distintos_agora: usize = empates_desfeitos.values().map(HashMap::len).sum();
    println!(
        "Agregado sobre {pares} pares (equipe × pista) do catálogo:\n  \
         distância média {:.0} km → {:.0} km ({:+.1}%)\n  \
         destinos distintos por sede: no máximo 3 antes (as três faixas), {:.1} em média agora",
        soma_antiga / n,
        soma_nova / n,
        (soma_nova / soma_antiga - 1.0) * 100.0,
        distintos_agora as f64 / empates_desfeitos.len().max(1) as f64,
    );
    println!();
}

#[test]
#[ignore = "régua de leitura em voz alta — roda com --ignored --nocapture"]
fn relatorio_da_fatura_visivel() {
    println!("\n=== A FATURA QUE O JOGADOR LÊ — uma etapa típica por degrau ===");
    println!("(receita zerada de propósito: a régua aqui é da despesa)\n");

    for (categoria, classe) in DEGRAUS {
        let f = monta(categoria, classe);
        println!(
            "── {} · {} etapas ──────────────────────────────────",
            rotulo(categoria, classe),
            etapas_de(categoria)
        );
        let mut bloco_atual: Option<BlocoDaFatura> = None;
        for linha in &f.linhas {
            if bloco_atual != Some(linha.bloco) {
                println!("  {}", linha.bloco.chave().to_uppercase());
                bloco_atual = Some(linha.bloco);
            }
            println!(
                "    {:<22} {:>12}",
                linha.chave,
                format!("${:.0}", linha.total())
            );
            if linha.tem_detalhe() {
                if linha.e_rateio() {
                    println!(
                        "        (1/{:.0} do ano — o ano inteiro soma ${:.0})",
                        linha.divisor,
                        linha.total_do_detalhe()
                    );
                }
                for d in &linha.detalhe {
                    println!(
                        "        {:<20} {:>10.1} {:<14} × ${:.2}",
                        d.chave,
                        d.quantidade,
                        d.unidade.chave(),
                        d.preco_unitario
                    );
                }
            } else if let Some(d) = linha.detalhe.first() {
                println!(
                    "        ({:.1} {} × ${:.2})",
                    d.quantidade,
                    d.unidade.chave(),
                    d.preco_unitario
                );
            }
        }
        println!(
            "    {:<22} {:>12}   ({} linhas)\n",
            "TOTAL DA ETAPA",
            format!("${:.0}", f.total_de_despesa()),
            f.linhas_de_despesa()
        );
    }

    // ── A tabela de leitura em voz alta: só os números que o rótulo promete ──
    println!("=== LEITURA EM VOZ ALTA (a linha diz o que o número é?) ===\n");
    println!(
        "{:<24} {:>10} {:>10} {:>10} {:>10} {:>12}",
        "Degrau", "combust.", "pneus", "peça", "frete", "total etapa"
    );
    println!("{}", "─".repeat(80));
    for (categoria, classe) in DEGRAUS {
        let f = monta(categoria, classe);
        println!(
            "{:<24} {:>10} {:>10} {:>10} {:>10} {:>12}",
            rotulo(categoria, classe),
            format!("${:.0}", f.valor(super::super::fatura::V_COMBUSTIVEL)),
            format!("${:.0}", f.valor(super::super::fatura::V_PNEUS)),
            format!("${:.0}", f.valor(super::super::fatura::V_REVISAO_MECANICA)),
            format!("${:.0}", f.valor(super::super::fatura::V_FRETE)),
            format!("${:.0}", f.total_de_despesa())
        );
    }
    println!(
        "\nReferência da §3.7: combustível de uma etapa de BMW M2 é ~$589 (o modelo\n\
         antigo cobrava $9.400 na mesma linha)."
    );
    println!(
        "\nA PEÇA COMPRADA não está nesta tabela: ela não é conta física de etapa, vem do\n\
         `technical_investment_cost` do ledger. O peso dela contra estes totais está medido\n\
         em `commands::race::despesa` — na GT3, 151% da coluna `total etapa` na média."
    );

    // ── Quanto da fatura é rateio? É a linha que domina a tela, e isso é um fato de
    // desenho que o jogador vai sentir: a etapa cobra o ano inteiro dividido.
    println!("\n=== PESO DO RATEIO NA FATURA DA ETAPA ===\n");
    println!(
        "{:<24} {:>8} {:>14} {:>12} {:>10}",
        "Degrau", "etapas", "rateio/etapa", "total etapa", "% da tela"
    );
    println!("{}", "─".repeat(72));
    for (categoria, classe) in DEGRAUS {
        let f = monta(categoria, classe);
        let rateio = f.valor(super::super::fatura::V_CUSTO_FIXO_DO_ANO);
        let total = f.total_de_despesa().max(1.0);
        println!(
            "{:<24} {:>8.0} {:>14} {:>12} {:>9.0}%",
            rotulo(categoria, classe),
            etapas_de(categoria),
            format!("${:.0}", rateio),
            format!("${:.0}", total),
            100.0 * rateio / total
        );
    }

    // ── Litros no tanque: a asserção que a fatura antiga não permitia fazer ──
    println!("\n=== O QUE CABE NO TANQUE (quantidade física por etapa) ===\n");
    println!(
        "{:<24} {:>10} {:>12} {:>12}",
        "Degrau", "litros", "jogos pneu", "km da equipe"
    );
    println!("{}", "─".repeat(62));
    for (categoria, classe) in DEGRAUS {
        let etapa = fatura_da_etapa(&EntradaDaEtapa::tipica(categoria, classe));
        let q = |chave: &str| etapa.linha(chave).map(|l| l.quantidade).unwrap_or(0.0);
        println!(
            "{:<24} {:>10.0} {:>12.1} {:>12.0}",
            rotulo(categoria, classe),
            q(crate::economia::evento::COMBUSTIVEL),
            q(crate::economia::evento::PNEUS),
            q(crate::economia::evento::REVISAO)
        );
    }
    println!();
}
