//! **Para quem for consertar `finance::state`.** Os dois limiares de DÍVIDA na unidade nova.
//!
//! ```text
//! cargo test --lib relatorio_dos_limiares_de_divida -- --nocapture
//! ```
//!
//! `financial_health_score` e `choose_season_strategy` dividem a dívida pelo caixa esperado
//! da divisão. Esse divisor era a tabela escrita à mão (~24 meses de operação para toda
//! equipe) e virou [`crate::economia::temporada::caixa_de_referencia`] — 6 meses. Ele
//! encolheu de 3,8× (Rookie) a 46× (Endurance GT4). **A dívida não encolheu junto.**
//!
//! Os dois consumidores:
//!
//! - `debt_penalty = pressão_de_dívida ÷ caixa_esperado × 80`, com teto de 70. Ele desconta
//!   até 70 dos 100 pontos do score composto.
//! - `choose_season_strategy` declara `survival` quando `pressão_de_dívida ≥ 0,75 ×
//!   caixa_esperado`.
//!
//! A `pressão_de_dívida` é a dívida bruta vezes um multiplicador de estado (0,85 em
//! `healthy`, 1,75 em `collapse`), então uma equipe já quebrada atinge os dois limiares com
//! menos dívida ainda.
//!
//! O que este relatório imprime é a resposta na única unidade comparável entre categorias:
//! **quantos meses de operação de dívida** bastam para saturar o teto e para virar a chave
//! de sobrevivência, antes e depois da troca de âncora. Nada aqui asserta calibração — a
//! decisão de onde os limiares devem ficar não é deste módulo.

use crate::economia::temporada;
use crate::finance::planning::category_finance_scale_for;

/// As divisões da escada, na ordem em que o jogador as sobe.
const DIVISOES: [(&str, Option<&str>); 7] = [
    ("mazda_rookie", None),
    ("mazda_amador", None),
    ("bmw_m2", None),
    ("gt4", None),
    ("gt3", None),
    ("lmp2", None),
    ("endurance", Some("gt4")),
];

fn nome(categoria: &str, classe: Option<&str>) -> String {
    match classe {
        Some(c) => format!("{categoria}:{c}"),
        None => categoria.to_string(),
    }
}

/// Multiplicador de estado de `finance::planning::calculate_debt_pressure`, copiado aqui
/// porque a função recebe um `Team` inteiro e o que se mede é o limiar, não a equipe.
const PRESSAO_SAUDAVEL: f64 = 0.85;
const PRESSAO_COLAPSO: f64 = 1.75;

/// Meses de operação de dívida que saturam o teto de 70 do `debt_penalty`.
fn meses_para_saturar(caixa_esperado: f64, mensal: f64, pressao: f64) -> f64 {
    // debt_penalty = divida × pressao ÷ caixa × 80 ≥ 70
    caixa_esperado * (70.0 / 80.0) / pressao / mensal
}

/// Meses de operação de dívida que disparam o `survival`.
fn meses_para_survival(caixa_esperado: f64, mensal: f64, pressao: f64) -> f64 {
    caixa_esperado * 0.75 / pressao / mensal
}

#[test]
fn relatorio_dos_limiares_de_divida() {
    println!("\n=== LIMIARES DE DÍVIDA NA UNIDADE NOVA (meses de operação) ===\n");
    println!(
        "Quanta dívida basta para saturar o teto de 70 do `debt_penalty` e para a equipe\n\
         entrar em `survival`. Em MESES DE OPERAÇÃO, que é a única unidade comparável\n\
         entre categorias.\n"
    );
    println!(
        "{:<18} {:>10} {:>12} {:>12} {:>12} {:>12}",
        "divisão", "caixa (m)", "teto saud.", "teto colap.", "surv. saud.", "surv. colap."
    );
    println!("{}", "─".repeat(80));

    for (categoria, classe) in DIVISOES {
        let mensal = temporada::custo_operacional_anual_de_referencia(categoria, classe) / 12.0;
        let caixa = category_finance_scale_for(categoria, classe).expected_cash_midpoint();
        println!(
            "{:<18} {:>10.1} {:>12.1} {:>12.1} {:>12.1} {:>12.1}",
            nome(categoria, classe),
            caixa / mensal,
            meses_para_saturar(caixa, mensal, PRESSAO_SAUDAVEL),
            meses_para_saturar(caixa, mensal, PRESSAO_COLAPSO),
            meses_para_survival(caixa, mensal, PRESSAO_SAUDAVEL),
            meses_para_survival(caixa, mensal, PRESSAO_COLAPSO),
        );
    }

    // A mesma conta com a âncora ANTIGA. Ela era ~24 meses em toda a escada (a razão
    // travada em 2,05 entre as duas âncoras), então um único par de números descreve a
    // escada inteira — que era exatamente o problema dela.
    const MESES_ANTIGOS: f64 = 24.0;
    println!(
        "\n  Com a âncora ANTIGA (~{MESES_ANTIGOS:.0} meses em toda a escada):\n  \
         teto do debt_penalty saturava com {:.1} meses de dívida (saudável) / {:.1} (colapso)\n  \
         survival disparava com {:.1} meses (saudável) / {:.1} (colapso)",
        MESES_ANTIGOS * (70.0 / 80.0) / PRESSAO_SAUDAVEL,
        MESES_ANTIGOS * (70.0 / 80.0) / PRESSAO_COLAPSO,
        MESES_ANTIGOS * 0.75 / PRESSAO_SAUDAVEL,
        MESES_ANTIGOS * 0.75 / PRESSAO_COLAPSO,
    );

    println!(
        "\n  Referência de leitura: `FaixasDeMeses::default()` chama de CRISE quem tem menos\n  \
         de 3 meses de fôlego e de COLAPSO quem tem fôlego negativo. Os limiares acima estão\n  \
         na mesma ordem de grandeza que essas fronteiras — ou seja, o desconto de 70 pontos\n  \
         e a chave de sobrevivência passaram a disparar com uma dívida do tamanho de um\n  \
         trimestre de operação, onde antes exigiam quase dois anos."
    );
    println!();
}

/// GUARD. A relação entre os dois limiares e a âncora tem que continuar sendo o que este
/// relatório descreve: se alguém reancorar `debt_penalty` sem reancorar `choose_season_strategy`
/// (ou vice-versa), os dois deixam de contar a mesma história e o relatório mente.
#[test]
fn os_dois_limiares_saem_da_mesma_ancora() {
    let escala = category_finance_scale_for("gt3", None);
    let caixa = escala.expected_cash_midpoint();
    let referencia = temporada::caixa_de_referencia("gt3", None);
    assert!(
        (caixa - referencia).abs() < 1.0,
        "o caixa esperado da GT3 ({caixa:.0}) deixou de ser o caixa de referência ({referencia:.0})"
    );
    // E o caixa de referência é, por construção, meses de operação.
    let mensal = temporada::custo_operacional_anual_de_referencia("gt3", None) / 12.0;
    assert!(
        ((caixa / mensal) - temporada::caixa_meses_de_referencia()).abs() < 0.01,
        "o caixa esperado deixou de estar em meses de operação"
    );
}
