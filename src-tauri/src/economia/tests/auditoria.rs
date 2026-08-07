//! Medições da auditoria de alcance da âncora nova.
//!
//! Não são guards de comportamento do módulo `economia` — são réguas para dimensionar o
//! que quebra quando `finance::planning::category_finance_scale` for substituída. Rode com
//! `cargo test --lib economia::tests::auditoria -- --nocapture`.

use super::legado::{escala_de_peca_legada, operacional_legado, COEFICIENTE_DE_PECA};
use crate::economia::ancora::DIVISOES;
use crate::economia::temporada;
use crate::finance::planning::category_finance_scale;
use crate::finance::state::{derive_financial_state, financial_health_score};
use crate::models::team::placeholder_team_from_db;

fn nome(categoria: &str, classe: Option<&str>) -> String {
    match classe {
        Some(c) => format!("{categoria}:{c}"),
        None => categoria.to_string(),
    }
}

/// As DUAS âncoras antigas andam juntas: `expected_cash_midpoint` é ~2,05× o
/// `operating_cost_midpoint` em toda a escada. Ou seja, a tabela velha diz — sem nunca
/// dizer — que uma equipe saudável guarda ~24 MESES de operação em caixa.
///
/// Isso importa porque `financial_health_score` divide caixa pela âncora de ESTOQUE e
/// poder de gasto pela de FLUXO. Enquanto a razão entre as duas for constante, o score é
/// estável; no instante em que o caixa esperado virar consequência (seção 3.2), a razão
/// muda e TODO time se move de faixa de uma vez.
#[test]
fn relatorio_razao_entre_as_duas_ancoras_antigas() {
    println!("\n=== As duas âncoras antigas: caixa esperado ÷ custo operacional ===");
    println!(
        "{:<24} {:>14} {:>14} {:>8} {:>10}",
        "categoria", "caixa médio", "operacional", "razão", "meses"
    );
    for categoria in [
        "mazda_rookie",
        "mazda_amador",
        "bmw_m2",
        "gt4",
        "gt3",
        "lmp2",
        "endurance",
    ] {
        let s = category_finance_scale(categoria);
        let razao = s.expected_cash_midpoint() / operacional_legado(categoria);
        println!(
            "{:<24} {:>14.0} {:>14.0} {:>8.2} {:>10.1}",
            categoria,
            s.expected_cash_midpoint(),
            operacional_legado(categoria),
            razao,
            razao * 12.0
        );
    }
}

/// **Armadilha 2, medida.** Onde caem as fronteiras de `financial_state` hoje, expressas em
/// MESES DE OPERAÇÃO — primeiro pela âncora velha, depois pela nova.
///
/// A pergunta era se o estado satura MAIS com a âncora honesta. A medição diz que sim, e
/// diz por quanto: a mesma fronteira que hoje exige dezenas de meses de caixa passa a
/// exigir poucos, porque o divisor encolheu sem que os pesos do score mudassem.
#[test]
fn relatorio_saturacao_do_financial_state() {
    println!("\n=== Fronteiras de financial_state em MESES DE OPERAÇÃO (gt3) ===");

    let anual_novo = temporada::custo_operacional_anual_de_referencia("gt3", None);
    let escala = category_finance_scale("gt3");
    let mes_velho = operacional_legado("gt3") / 12.0;
    let mes_novo = anual_novo / 12.0;

    println!(
        "  um mês de operação: velho {:.0}  novo {:.0}  (divisor encolheu {:.1}×)",
        mes_velho,
        mes_novo,
        mes_velho / mes_novo
    );

    let mut anterior = "";
    let mut estados = 0;
    println!("\n  caixa            meses(velho) meses(novo)  score  estado");
    for passo in 0..=80 {
        let caixa = passo as f64 * 500_000.0;
        let mut team = placeholder_team_from_db(
            "TAUD".to_string(),
            "Auditoria".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = caixa;
        team.reputacao = 55.0;
        team.engineering = 55.0;
        team.facilities = 55.0;

        let score = financial_health_score(&team);
        let estado = derive_financial_state(score);
        if estado != anterior {
            println!(
                "  {:>14.0} {:>12.1} {:>11.1} {:>6.1}  {estado}",
                caixa,
                caixa / mes_velho,
                caixa / mes_novo,
                score
            );
            anterior = estado;
            estados += 1;
        }
    }
    assert!(estados >= 3, "a varredura precisa atravessar faixas");

    // O ponto onde o termo de caixa satura: cash_score = caixa/caixa_médio × 65, teto 100.
    let satura_velho = escala.expected_cash_midpoint() * 100.0 / 65.0;
    println!(
        "\n  cash_score satura em {:.0} de caixa = {:.1} meses de operação (âncora velha)",
        satura_velho,
        satura_velho / mes_velho
    );
    println!(
        "  os mesmos {:.0} valem {:.1} meses pela âncora nova — a MESMA equipe fica {:.1}× \
         mais folgada sem ter ganhado um centavo",
        satura_velho,
        satura_velho / mes_novo,
        mes_velho / mes_novo
    );
}

/// **A contaminação de fluxo dentro do índice de orçamento.**
///
/// `derive_budget_index_from_money` normaliza um `effective_money` pela janela
/// `cash_max − cash_min`. O numerador soma caixa (estoque) com `spending_power` e
/// `projected_income` (fluxos anuais); o denominador é só caixa. Enquanto o caixa esperado
/// valia ~24 meses de operação, a janela era larga o bastante para absorver os fluxos e o
/// índice espalhava. Com o caixa virando 1–11 meses, os termos de fluxo passam a dominar um
/// denominador dez vezes menor.
///
/// Este relatório mostra onde o índice cai numa varredura de caixa: se ele grudar em 100 na
/// maior parte da faixa, o índice deixou de discriminar e quem lê `budget` (fama, geração de
/// equipe, preseason) perde a capacidade de separar rico de pobre.
///
/// **Roda as duas fórmulas no mesmo binário**, sobre as mesmas equipes — a velha congelada em
/// [`budget_index_legado`], a nova em produção. Comparação entre execuções não serviria: a
/// âncora se move debaixo das duas.
#[test]
fn relatorio_contaminacao_de_fluxo_no_budget_index() {
    use crate::finance::planning::derive_budget_index_from_money;

    println!("\n=== budget_index numa varredura de caixa (gt3) ===");
    println!(
        "  {:>10} {:>12} {:>10} {:>10}",
        "meses", "caixa", "VELHO", "NOVO"
    );
    let mut saturados = 0;
    let mut saturados_novo = 0;
    let mut amostras = 0;
    for meses in [0.0, 0.5, 1.0, 2.0, 4.0, 6.0, 8.0, 11.0, 16.0, 24.0, 48.0] {
        let caixa = temporada::caixa_para_meses("gt3", None, meses);
        let mut team = placeholder_team_from_db(
            "TIDX".to_string(),
            "Indice".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = caixa;
        team.reputacao = 55.0;
        team.engineering = 55.0;
        team.facilities = 55.0;

        let velho = budget_index_legado(&team);
        let novo = derive_budget_index_from_money(&team);
        println!("  {meses:>10.1} {caixa:>12.0} {velho:>10.1} {novo:>10.1}");
        amostras += 1;
        if velho >= 99.5 {
            saturados += 1;
        }
        if novo >= 99.5 {
            saturados_novo += 1;
        }
    }
    println!("\n  saturados em 100 — VELHO: {saturados}/{amostras}   NOVO: {saturados_novo}/{amostras}");
}

/// **A fórmula VELHA de `budget_index`, congelada.**
///
/// `caixa + spending_power×0,45 + receita×0,25 − dívida×0,35`, normalizada pela janela de
/// caixa da categoria. Depois da re-derivação de `calculate_spending_power` os três termos
/// extras já estão dentro dele — é o triplo-contado da seção 4.6. Fica aqui porque apagá-la
/// apagaria a única forma de medir o deslocamento no mesmo binário, que é o padrão que a
/// auditoria inteira segue (`super::legado`, `spending_power_legado`).
fn budget_index_legado(team: &crate::models::team::Team) -> f64 {
    use crate::finance::planning::{
        calculate_debt_pressure, calculate_projected_income, calculate_spending_power,
        category_finance_scale_for,
    };

    let scale = category_finance_scale_for(&team.categoria, team.classe.as_deref());
    let category_window = (scale.cash_max - scale.cash_min).max(1.0);
    let effective_money = team.cash_balance
        + calculate_spending_power(team) * 0.45
        + calculate_projected_income(team) * 0.25
        - calculate_debt_pressure(team) * 0.35;

    ((effective_money - scale.cash_min) / category_window * 100.0).clamp(0.0, 100.0)
}

fn quartis(mut valores: Vec<f64>) -> (f64, f64, f64, f64, f64) {
    valores.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let em = |q: f64| -> f64 {
        let pos = q * (valores.len() - 1) as f64;
        let baixo = pos.floor() as usize;
        let alto = pos.ceil() as usize;
        valores[baixo] + (valores[alto] - valores[baixo]) * (pos - baixo as f64)
    };
    (em(0.0), em(0.25), em(0.5), em(0.75), em(1.0))
}

/// **A distribuição de `budget_index` sobre o grid inteiro, velho contra novo.**
///
/// Um índice de 0 a 100 existe para separar. O teste dele não é se a fórmula "faz sentido" —
/// é se a distribuição sobre a população real usa a escala ou empilha nas pontas. Duas
/// populações, porque elas são diferentes e as duas importam:
///
/// - **nascimento**: a faixa de caixa declarada de cada divisão, 1–11 meses, varrida pelo
///   mesmo `budget_seed` que `Team::from_template` usa para semear o caixa;
/// - **regime**: os meses de operação medidos na seção 4.7 do redesign, 20 temporadas de
///   Monte Carlo — campeão, meio e lanterna de nove categorias. É o mundo em que o índice de
///   fato é lido, e ele roda MUITO acima da faixa de nascimento (10,9–22,1 de mediana).
///
/// **Só seis categorias têm equipe de nascimento** (as duas rookie, as duas amador,
/// `production_challenger` e `endurance`); as outras oito divisões recebem equipe por
/// promoção. Por isso a varredura por divisão usa a faixa declarada, que existe para as
/// catorze, e o grid gerado de verdade entra como conferência à parte.
///
/// Rode com `cargo test --lib relatorio_distribuicao_do_budget_index -- --nocapture`.
#[test]
fn relatorio_distribuicao_do_budget_index() {
    use crate::finance::planning::{derive_budget_index_from_money, meses_projetados};

    let equipe_em = |categoria: &str, classe: Option<&str>, meses: f64| {
        let mut team = placeholder_team_from_db(
            "TIDX".to_string(),
            "Indice".to_string(),
            categoria.to_string(),
            "2026-01-01".to_string(),
        );
        team.classe = classe.map(str::to_string);
        team.reputacao = 55.0;
        team.engineering = 55.0;
        team.facilities = 55.0;
        team.cash_balance = temporada::caixa_para_meses(categoria, classe, meses);
        team
    };

    let resumo = |rotulo: &str, v: &[f64]| {
        let (min, q1, mediana, q3, max) = quartis(v.to_vec());
        let em_zero = v.iter().filter(|x| **x <= 0.5).count();
        let em_cem = v.iter().filter(|x| **x >= 99.5).count();
        println!(
            "  {rotulo:<8} mín {min:>5.1}  Q1 {q1:>5.1}  med {mediana:>5.1}  Q3 {q3:>5.1}  \
             máx {max:>5.1}   |   em 0: {em_zero:>3}/{n}  em 100: {em_cem:>3}/{n}",
            n = v.len()
        );
    };

    println!("\n=== budget_index na faixa de NASCIMENTO (14 divisões, caixa de 1 a 11 meses) ===");
    println!(
        "{:<24} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}",
        "divisão", "V.mín", "V.med", "V.máx", "N.mín", "N.med", "N.máx"
    );

    let mut todos_velho: Vec<f64> = Vec::new();
    let mut todos_novo: Vec<f64> = Vec::new();
    for (categoria, classe) in DIVISOES {
        let mut velhos = Vec::new();
        let mut novos = Vec::new();
        for passo in 0..=10 {
            let seed = passo as f64 / 10.0;
            let meses = temporada::CAIXA_MESES_MIN
                + (temporada::CAIXA_MESES_MAX - temporada::CAIXA_MESES_MIN) * seed;
            let team = equipe_em(categoria, classe, meses);
            velhos.push(budget_index_legado(&team));
            novos.push(derive_budget_index_from_money(&team));
        }
        let (vmin, _, vmed, _, vmax) = quartis(velhos.clone());
        let (nmin, _, nmed, _, nmax) = quartis(novos.clone());
        println!(
            "{:<24} {:>7.1} {:>7.1} {:>7.1} {:>7.1} {:>7.1} {:>7.1}",
            nome(categoria, classe),
            vmin,
            vmed,
            vmax,
            nmin,
            nmed,
            nmax
        );
        todos_velho.extend(velhos);
        todos_novo.extend(novos);
    }
    println!(
        "\n  Faixa de nascimento inteira ({} pontos):",
        todos_novo.len()
    );
    resumo("VELHO", &todos_velho);
    resumo("NOVO", &todos_novo);

    // O grid gerado de verdade: as seis categorias que têm template. Aqui reputação,
    // instalações e engenharia variam de equipe para equipe, então mede também a
    // dispersão que o termo de FLUXO acrescenta e que a varredura acima não vê.
    println!("\n=== o grid GERADO (só as categorias com equipe de nascimento) ===");
    let mut gerado_velho = Vec::new();
    let mut gerado_novo = Vec::new();
    for categoria in [
        "mazda_rookie",
        "toyota_rookie",
        "mazda_amador",
        "toyota_amador",
        "production_challenger",
        "endurance",
    ] {
        let mut n = 0usize;
        let mut gerador = || {
            n += 1;
            format!("{categoria}-{n}")
        };
        let equipes =
            crate::models::team::generate_teams_for_category(categoria, 2026, &mut gerador);
        let velhos: Vec<f64> = equipes.iter().map(budget_index_legado).collect();
        let novos: Vec<f64> = equipes
            .iter()
            .map(derive_budget_index_from_money)
            .collect();
        let (vmin, _, _, _, vmax) = quartis(velhos.clone());
        let (nmin, _, _, _, nmax) = quartis(novos.clone());
        println!(
            "{:<24} n={:<4} VELHO {:>5.1}–{:<5.1}   NOVO {:>5.1}–{:<5.1}",
            categoria,
            equipes.len(),
            vmin,
            vmax,
            nmin,
            nmax
        );
        gerado_velho.extend(velhos);
        gerado_novo.extend(novos);
    }
    println!("\n  Grid gerado inteiro ({} equipes):", gerado_novo.len());
    resumo("VELHO", &gerado_velho);
    resumo("NOVO", &gerado_novo);

    // ── A população que o índice de fato encontra ────────────────────────────────────
    //
    // Meses de operação medidos na 4.7 (20 temporadas, campeão / meio / lanterna). O
    // caixa é reconstruído a partir dos meses, o resto da equipe fica no ponto neutro:
    // isola a variável que a 4.7 mediu.
    println!("\n=== budget_index sobre o mundo EM REGIME (medições da 4.7) ===");
    println!(
        "{:<18} {:>9} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "categoria", "posição", "meses", "proj.", "VELHO", "NOVO", "VELHO", "NOVO"
    );
    const REGIME: [(&str, [f64; 3]); 9] = [
        ("mazda_rookie", [19.8, 22.1, 20.2]),
        ("toyota_rookie", [18.0, 20.5, 18.5]),
        ("mazda_amador", [76.7, 3.3, 10.5]),
        ("toyota_amador", [62.8, 11.4, 15.3]),
        ("bmw_m2", [61.2, 16.6, 14.3]),
        ("production_challenger", [66.1, 14.3, 6.7]),
        ("gt4", [78.4, 12.6, 6.3]),
        ("gt3", [46.4, 16.9, 9.6]),
        ("endurance", [43.2, 14.8, 9.2]),
    ];
    let mut regime_velho = Vec::new();
    let mut regime_novo = Vec::new();
    for (categoria, triplo) in REGIME {
        for (rotulo, meses) in ["campeão", "meio", "lanterna"].iter().zip(triplo) {
            let team = equipe_em(categoria, None, meses);

            let velho = budget_index_legado(&team);
            let novo = derive_budget_index_from_money(&team);
            println!(
                "{:<18} {:>9} {:>8.1} {:>8.1} {:>8.1} {:>8.1}",
                categoria,
                rotulo,
                meses,
                meses_projetados(&team),
                velho,
                novo
            );
            regime_velho.push(velho);
            regime_novo.push(novo);
        }
    }
    println!("\n  Mundo em regime ({} pontos):", regime_novo.len());
    resumo("VELHO", &regime_velho);
    resumo("NOVO", &regime_novo);
}

/// **Quem lê `budget_index`, e o que a re-derivação fez com eles.**
///
/// A metade do trabalho que não é a fórmula. O índice saturado entregava ~100 para quase
/// todo mundo, e cada consumidor tratava esse 100 como se fosse informação. Este relatório
/// mede os quatro no mesmo binário, sobre o mundo em regime da 4.7:
///
/// - `fame::team_need_factor` — 0,6 de peso no índice, decide o quanto a fama do piloto vale
///   na conta da equipe. Com todo mundo em 100 o fator ficava colado no piso;
/// - o patrocínio de `commands::race::financas` — `budget_index × base × 0,002`, um termo de
///   RECEITA. Aqui a queda do índice é queda de dinheiro de verdade, e é o efeito colateral
///   caro desta mudança;
/// - `market::pit_strategy` — `budget_index/100` como força financeira, mais um bônus de
///   risco abaixo de 30 pontos que nunca disparava;
/// - `finance::state::financial_health_score` — o `support_score`, instrumento legado.
///
/// Rode com `cargo test --lib relatorio_consumidores_do_budget_index -- --nocapture`.
#[test]
fn relatorio_consumidores_do_budget_index() {
    use crate::fame::team_need_factor;
    use crate::finance::planning::derive_budget_index_from_money;

    const REGIME: [(&str, [f64; 3]); 5] = [
        ("mazda_rookie", [19.8, 22.1, 20.2]),
        ("mazda_amador", [76.7, 3.3, 10.5]),
        ("bmw_m2", [61.2, 16.6, 14.3]),
        ("gt4", [78.4, 12.6, 6.3]),
        ("gt3", [46.4, 16.9, 9.6]),
    ];

    println!("\n=== Consumidores de budget_index, no mundo em regime da 4.7 ===");
    println!(
        "{:<16} {:>8} {:>7} {:>7} {:>8} {:>8} {:>9} {:>9}",
        "divisão", "meses", "V.idx", "N.idx", "V.need", "N.need", "V.patroc", "N.patroc"
    );

    let (mut need_v, mut need_n) = (Vec::new(), Vec::new());
    let (mut patroc_v, mut patroc_n) = (Vec::new(), Vec::new());
    let mut fracos_v = 0;
    let mut fracos_n = 0;
    let mut pontos = 0;

    for (categoria, triplo) in REGIME {
        for meses in triplo {
            let mut team = placeholder_team_from_db(
                "TCON".to_string(),
                "Consumidor".to_string(),
                categoria.to_string(),
                "2026-01-01".to_string(),
            );
            team.reputacao = 55.0;
            team.engineering = 55.0;
            team.facilities = 55.0;
            team.cash_balance = temporada::caixa_para_meses(categoria, None, meses);

            let velho = budget_index_legado(&team);
            let novo = derive_budget_index_from_money(&team);

            // `commands::race::financas`: o termo entra como fração da base operacional da
            // rodada. Em pontos dessa base, para ficar comparável entre divisões.
            let patrocinio = |idx: f64| idx * 0.002;

            println!(
                "{:<16} {:>8.1} {:>7.1} {:>7.1} {:>8.3} {:>8.3} {:>9.4} {:>9.4}",
                categoria,
                meses,
                velho,
                novo,
                team_need_factor(velho, team.reputacao),
                team_need_factor(novo, team.reputacao),
                patrocinio(velho),
                patrocinio(novo)
            );

            need_v.push(team_need_factor(velho, team.reputacao));
            need_n.push(team_need_factor(novo, team.reputacao));
            patroc_v.push(patrocinio(velho));
            patroc_n.push(patrocinio(novo));
            // `market::pit_strategy::recalculate_pit_strategy_risk`: bônus de risco
            // para quem tem menos de 30 pontos de orçamento.
            if velho < 30.0 {
                fracos_v += 1;
            }
            if novo < 30.0 {
                fracos_n += 1;
            }
            pontos += 1;
        }
    }

    let amplitude = |v: &[f64]| {
        let (min, _, _, _, max) = quartis(v.to_vec());
        (min, max, max - min)
    };
    let (nv_min, nv_max, nv_amp) = amplitude(&need_v);
    let (nn_min, nn_max, nn_amp) = amplitude(&need_n);
    println!(
        "\n  fame::team_need_factor   VELHO {nv_min:.3}–{nv_max:.3} (amplitude {nv_amp:.3})   \
         NOVO {nn_min:.3}–{nn_max:.3} (amplitude {nn_amp:.3})"
    );
    println!(
        "    (o fator vai de {:.2} a {:.2} por construção; a amplitude usada é o quanto o \
         mecanismo de fato existe)",
        crate::fame::TEAM_NEED_MIN,
        crate::fame::TEAM_NEED_MAX
    );

    let media = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    println!(
        "\n  patrocínio (termo do índice, em frações da base da rodada)   VELHO {:.4}   \
         NOVO {:.4}   →  {:+.1}%",
        media(&patroc_v),
        media(&patroc_n),
        (media(&patroc_n) / media(&patroc_v) - 1.0) * 100.0
    );
    println!(
        "    o termo inteiro do patrocínio é `0,27 + reputação×0,004 + índice×0,002 + fama`;\n    \
         com reputação 55 isso dá {:.4} de base — a queda do índice tira {:.1}% do canal.",
        0.27 + 55.0 * 0.004,
        (media(&patroc_v) - media(&patroc_n)) / (0.27 + 55.0 * 0.004 + media(&patroc_v)) * 100.0
    );

    println!(
        "\n  pit_strategy: equipes abaixo de 30 pontos (bônus de risco)   VELHO {fracos_v}/{pontos}   \
         NOVO {fracos_n}/{pontos}"
    );
}

/// **Armadilha 1, resolvida.** Quanto a escala de peça ANDOU ao ser descongelada.
///
/// A prova de que ela era uma cópia da âncora velha mora agora em
/// [`super::legado`] — aqui já não daria para medir, porque `car::cost::category_cost_scale`
/// passou a derivar da âncora ao vivo e a comparação seria trivialmente 1,00.
///
/// O que este relatório mostra é o TAMANHO do desastre que não aconteceu: se a escala
/// tivesse ficado congelada, a peça de reposição — o único débito que escala com riqueza —
/// teria ficado relativamente 2,4× mais cara na GT3 e 10,5× no GT4 do Endurance.
#[test]
fn relatorio_deriva_da_escala_de_peca() {
    println!("\n=== Quanto a escala de peça andou ao ser descongelada ===");
    println!(
        "{:<28} {:>12} {:>12} {:>9} {:>26}",
        "divisão", "congelada", "derivada", "fator", "se tivesse ficado"
    );
    for (categoria, classe) in DIVISOES {
        let escrito = escala_de_peca_legada(categoria);
        let anual_novo = temporada::custo_operacional_anual_de_referencia(categoria, classe);
        let deveria = anual_novo * COEFICIENTE_DE_PECA;
        let fator = deveria / escrito;
        println!(
            "{:<28} {:>12.0} {:>12.0} {:>9.2} {:>25.1}×",
            nome(categoria, classe),
            escrito,
            deveria,
            fator,
            1.0 / fator
        );
    }
}
