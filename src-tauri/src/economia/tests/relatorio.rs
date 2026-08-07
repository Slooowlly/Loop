//! Relatório legível da fatura e da temporada. Não é guard — é a lente.
//!
//! Rode com `cargo test --lib economia::tests::relatorio -- --nocapture` para ver a
//! tabela de parâmetros, a fatura de uma etapa típica de cada divisão e o custo de
//! temporada que sai disso ao lado da âncora antiga.

use super::legado::{amplitude_legada, caixa_legado, operacional_legado};
use crate::economia::ancora::{self, DIVISOES};
use crate::economia::evento::fatura_da_etapa;
use crate::economia::temporada;
use crate::economia::tipos::EntradaDaEtapa;

fn nome(categoria: &str, classe: Option<&str>) -> String {
    match classe {
        Some(c) => format!("{categoria}:{c}"),
        None => categoria.to_string(),
    }
}

#[test]
fn relatorio_da_tabela_de_parametros() {
    println!("\n=== PARÂMETROS FÍSICOS POR DIVISÃO ===");
    println!(
        "{:<28} {:>6} {:>6} {:>8} {:>8} {:>7} {:>7} {:>8} {:>9} {:>6} {:>6} {:>10}",
        "divisão",
        "km/h",
        "min",
        "km prova",
        "km T+Q",
        "l/km",
        "jog.fix",
        "km/jogo",
        "US$/jogo",
        "comit",
        "fixa",
        "inscrição"
    );
    for (categoria, classe) in DIVISOES {
        let p = ancora::parametros(categoria, classe);
        println!(
            "{:<28} {:>6.0} {:>6.0} {:>8.0} {:>8.0} {:>7.2} {:>7.1} {:>8.0} {:>9.0} {:>6.0} {:>6.0} {:>10.0}",
            nome(categoria, classe),
            p.velocidade_media_kmh,
            p.duracao_corrida_min,
            p.km_de_corrida(),
            p.km_treino_quali,
            p.consumo_l_por_km,
            p.jogos_de_pneu_fixos,
            p.km_por_jogo_de_corrida,
            p.preco_do_jogo_de_pneu,
            p.comitiva,
            p.equipe_fixa,
            p.taxa_de_inscricao,
        );
    }
}

#[test]
fn relatorio_da_fatura_de_uma_etapa() {
    println!(
        "\n=== FATURA DE UMA ETAPA TÍPICA (equipe mediana, 2 carros, pista média, continental) ==="
    );
    for (categoria, classe) in DIVISOES {
        let entrada = EntradaDaEtapa::tipica(categoria, classe);
        let fatura = fatura_da_etapa(&entrada);
        println!("\n-- {} --", nome(categoria, classe));
        for l in &fatura.linhas {
            println!(
                "  {:<12} {:<10} {:>10.1} {:<12} × {:>10.2} = {:>12.0}",
                l.chave,
                l.bloco.chave(),
                l.quantidade,
                l.unidade.chave(),
                l.preco_unitario,
                l.total(),
            );
        }
        println!("  {:<12} {:>62.0}", "TOTAL", fatura.total());
    }
}

#[test]
fn relatorio_dos_recorrentes_de_temporada() {
    println!("\n=== RECORRENTES ANUAIS (equipe mediana, instalações 50) ===");
    println!(
        "{:<28} {:>6} {:>12} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "divisão", "fixa", "folha", "sede", "frota", "seguro", "fábrica", "simul.", "dados"
    );
    for (categoria, classe) in DIVISOES {
        let p = temporada::parametros_de_temporada(categoria, classe);
        let fisico = ancora::parametros(categoria, classe);
        let zero = |v: f64| {
            if v > 0.0 {
                format!("{v:.0}")
            } else {
                "—".to_string()
            }
        };
        println!(
            "{:<28} {:>6.0} {:>12.0} {:>10.0} {:>10.0} {:>10.0} {:>10} {:>10} {:>10}",
            nome(categoria, classe),
            fisico.equipe_fixa,
            fisico.equipe_fixa * p.salario_medio_anual,
            p.sede_anual,
            p.frota_anual,
            p.seguro_e_licenca_anual,
            zero(p.suporte_de_fabrica_anual),
            zero(p.simulador_anual),
            zero(p.aquisicao_de_dados_anual),
        );
    }
}

/// O número da seção 3.2, comparado com a tabela velha **total contra total**. O fator de
/// ponte de 0,62 morreu aqui: agora os dois lados cobrem o mesmo escopo — eventos mais
/// estrutura de um ano.
#[test]
fn relatorio_do_operacional_anual_contra_a_ancora_antiga() {
    println!("\n=== CUSTO OPERACIONAL ANUAL DE REFERÊNCIA vs. operating_cost_midpoint ===");
    println!(
        "{:<28} {:>6} {:>10} {:>11} {:>9} {:>9} {:>11} {:>12} {:>6}",
        "divisão",
        "etapas",
        "eventos",
        "escalares",
        "categór.",
        "pilotos",
        "TOTAL/ano",
        "midpoint",
        "razão"
    );
    for (categoria, classe) in DIVISOES {
        let d = temporada::decomposicao_anual(categoria, classe);
        let midpoint = operacional_legado(categoria);
        println!(
            "{:<28} {:>6.0} {:>10.0} {:>11.0} {:>9.0} {:>9.0} {:>11.0} {:>12.0} {:>6.2}",
            nome(categoria, classe),
            ancora::etapas_por_temporada(categoria),
            d.eventos,
            d.recorrentes_escalares,
            d.recorrentes_categoricos,
            d.folha_de_pilotos,
            d.total(),
            midpoint,
            d.total() / midpoint,
        );
    }

    // A folha de piloto que sai daqui, contra os 15% que `finance::salary` projeta. Não é
    // calibração — é conferência de escopo: se as duas ficassem longe, a comparação de
    // cima estaria comparando coisas diferentes de novo.
    println!("\n  folha de pilotos como % do ano:");
    for (categoria, classe) in DIVISOES {
        let d = temporada::decomposicao_anual(categoria, classe);
        println!(
            "    {:<26} {:>5.1}%",
            nome(categoria, classe),
            d.folha_de_pilotos / d.total() * 100.0
        );
    }
}

/// **Para a sessão que reancora `finance::state` em meses.** O fator pelo qual o divisor
/// se moveu debaixo dela, divisão por divisão.
///
/// `financial_health_score` divide `spending_power` por `operating_cost_midpoint`. Esse
/// denominador acabou de trocar de tabela para conta física, e não trocou por um fator
/// único: vai de 0,88 (a Rookie ficou MAIS cara) a 10,5 (o GT4 do Endurance encolheu dez
/// vezes). Uma constante global não reancora isso.
///
/// Ler assim: "divisor ÷" é quantas vezes o denominador encolheu. Um time que não ganhou um
/// centavo tem o `spending_score` multiplicado por esse número.
#[test]
fn relatorio_fator_do_divisor_para_o_financial_state() {
    println!("\n=== FATOR DO DIVISOR (para a reancoragem de financial_state) ===");
    println!(
        "{:<28} {:>14} {:>14} {:>10} {:>26}",
        "divisão", "midpoint velho", "midpoint novo", "divisor ÷", "spending_score ×"
    );
    for (categoria, classe) in DIVISOES {
        let velho = operacional_legado(categoria);
        let novo = temporada::custo_operacional_anual_de_referencia(categoria, classe);
        println!(
            "{:<28} {:>14.0} {:>14.0} {:>10.2} {:>25.2}×",
            nome(categoria, classe),
            velho,
            novo,
            velho / novo,
            velho / novo,
        );
    }
    println!(
        "\n  ATENÇÃO: o caixa esperado NÃO se moveu nesta rodada (âncora de estoque ficou\n  \
         para depois). Então `cash_score` está intacto e só `spending_score` andou — é por\n  \
         isso que a razão estoque÷fluxo, que era ~2,05 travada, agora varia por divisão."
    );
}

/// **Para a sessão do `finance::state`, segunda entrega.** O fator da âncora de ESTOQUE.
///
/// `financial_health_score` tem TRÊS termos ancorados, e nesta rodada os três se moveram:
///
/// - `cash_score = caixa ÷ caixa_esperado × 65` — o divisor encolheu, então satura MAIS;
/// - `spending_score = spending_power ÷ operacional × 55` — o NUMERADOR encolheu agora
///   (`projected_income` e `available_credit` nasciam do caixa-médio), então este anda no
///   sentido OPOSTO ao da rodada passada;
/// - `debt_penalty = pressão_de_dívida ÷ caixa_esperado × 80`, teto 70 — é o mais perigoso:
///   o divisor encolheu igual ao do `cash_score`, e como a dívida de um save existente NÃO
///   encolheu junto, qualquer passivo herdado satura o teto de 70 pontos de cara.
///
/// O mesmo vale para `choose_season_strategy`, que declara `survival` quando a dívida passa
/// de 0,75 do caixa esperado.
#[test]
fn relatorio_fator_da_ancora_de_estoque() {
    println!("\n=== FATOR DA ÂNCORA DE ESTOQUE (caixa esperado) ===");
    println!(
        "{:<28} {:>14} {:>14} {:>10} {:>12}",
        "divisão", "caixa velho", "caixa novo", "encolheu ÷", "meses velhos"
    );
    for (categoria, classe) in DIVISOES {
        // O caixa-médio da tabela antiga, congelado em `legado`.
        let velho = caixa_legado(categoria);
        let novo = temporada::caixa_de_referencia(categoria, classe);
        let meses_velhos = velho / (operacional_legado(categoria) / 12.0);
        println!(
            "{:<28} {:>14.0} {:>14.0} {:>10.2} {:>12.1}",
            nome(categoria, classe),
            velho,
            novo,
            velho / novo,
            meses_velhos,
        );
    }
    println!(
        "\n  O caixa esperado agora é {:.0} meses de operação (era ~24 na tabela velha).",
        temporada::caixa_meses_de_referencia()
    );
    println!(
        "  ATENÇÃO ao `debt_penalty`: o divisor dele encolheu por estes mesmos fatores, mas\n  \
         a dívida de um save herdado não encolheu junto. O teto de 70 pontos satura."
    );
}

/// A medição da hipótese: quanto da íngreme da pirâmide vem de custo categórico.
#[test]
fn relatorio_da_hipotese_dos_custos_categoricos() {
    let dec: Vec<_> = DIVISOES
        .iter()
        .map(|(c, k)| (nome(c, *k), temporada::decomposicao_anual(c, *k)))
        .collect();

    println!("\n=== HIPÓTESE: a pirâmide vem de custos CATEGÓRICOS? ===");
    println!(
        "{:<28} {:>12} {:>14} {:>10}",
        "divisão", "TOTAL/ano", "sem categóricos", "% categór."
    );
    for (rotulo, d) in &dec {
        println!(
            "{:<28} {:>12.0} {:>14.0} {:>9.1}%",
            rotulo,
            d.total(),
            d.total() - d.recorrentes_categoricos,
            d.fracao_categorica() * 100.0
        );
    }

    let amplitude = |f: &dyn Fn(&temporada::DecomposicaoAnual) -> f64| {
        let vs: Vec<f64> = dec.iter().map(|(_, d)| f(d)).collect();
        vs.iter().cloned().fold(0.0, f64::max) / vs.iter().cloned().fold(f64::MAX, f64::min)
    };
    let com = amplitude(&|d| d.total());
    let sem = amplitude(&|d| d.total() - d.recorrentes_categoricos);
    let so_folha = amplitude(&|d| d.recorrentes_escalares);
    let antiga = amplitude_legada();

    println!("\n  amplitude da escada COM categóricos    {com:.1}×");
    println!("  amplitude da escada SEM categóricos    {sem:.1}×");
    println!("  amplitude só dos recorrentes escalares {so_folha:.1}×");
    println!("  amplitude da tabela ANTIGA             {antiga:.1}×");
    println!(
        "\n  contribuição dos categóricos para a íngreme: {:.0}%",
        (com / sem - 1.0) * 100.0
    );
}
