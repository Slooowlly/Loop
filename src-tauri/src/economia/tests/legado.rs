//! A tabela financeira ANTIGA, congelada como dado histórico.
//!
//! `finance::planning::category_finance_scale` já não devolve estes números: o custo
//! operacional dela passou a vir de [`crate::economia::temporada`]. Sem uma cópia congelada,
//! todo teste que media "quanto o modelo novo diverge do antigo" viraria uma tautologia —
//! compararia a âncora nova com ela mesma e daria 1,00 em toda a escada, exatamente o
//! sintoma de calibração que o redesign proíbe.
//!
//! Então a régua vira dado. Estes são os literais que estavam em `category_finance_scale`
//! antes da troca, e eles não devem ser atualizados nunca: o valor deles é justamente serem
//! o retrato de onde a economia estava.

/// `(categoria, operating_cost_min, operating_cost_max)` da tabela escrita à mão.
const OPERACIONAL_LEGADO: [(&str, f64, f64); 7] = [
    ("mazda_rookie", 120_000.0, 250_000.0),
    ("mazda_amador", 250_000.0, 600_000.0),
    ("bmw_m2", 600_000.0, 1_600_000.0),
    ("gt4", 1_500_000.0, 4_000_000.0),
    ("gt3", 4_000_000.0, 12_000_000.0),
    ("lmp2", 7_000_000.0, 20_000_000.0),
    ("endurance", 8_000_000.0, 25_000_000.0),
];

/// Custo operacional médio que a tabela antiga declarava para a categoria.
///
/// Aceita as categorias que compartilhavam linha (toyota com mazda, production com bmw),
/// porque era assim que a tabela velha agrupava — e é esse agrupamento que o modelo novo
/// desfez ao passar a orçar por divisão.
pub fn operacional_legado(categoria: &str) -> f64 {
    OPERACIONAL_LEGADO
        .iter()
        .find(|(c, _, _)| *c == chave_legada(categoria))
        .map(|(_, min, max)| (min + max) / 2.0)
        .unwrap_or(1_100_000.0)
}

/// `(categoria, cash_min, cash_max)` da tabela escrita à mão — a âncora de ESTOQUE antes
/// de virar consequência.
///
/// Congelada pelo mesmo motivo que o operacional: `category_finance_scale` agora deriva o
/// caixa de meses de operação, então comparar com ela mesma não mediria nada. Estes números
/// são o retrato do que a economia declarava — e, medidos contra o operacional legado, eles
/// dizem que a equipe mediana devia guardar ~24 meses de operação em caixa.
const CAIXA_LEGADO: [(&str, f64, f64); 7] = [
    ("mazda_rookie", 100_000.0, 700_000.0),
    ("mazda_amador", 250_000.0, 1_500_000.0),
    ("bmw_m2", 750_000.0, 4_000_000.0),
    ("gt4", 2_000_000.0, 9_000_000.0),
    ("gt3", 6_000_000.0, 25_000_000.0),
    ("lmp2", 10_000_000.0, 45_000_000.0),
    ("endurance", 12_000_000.0, 60_000_000.0),
];

/// Caixa-médio que a tabela antiga declarava para a categoria.
pub fn caixa_legado(categoria: &str) -> f64 {
    CAIXA_LEGADO
        .iter()
        .find(|(c, _, _)| *c == chave_legada(categoria))
        .map(|(_, min, max)| (min + max) / 2.0)
        .unwrap_or(2_375_000.0)
}

/// O agrupamento da tabela velha: toyota dividia linha com mazda, production com bmw_m2. É
/// esse agrupamento que o modelo novo desfez ao passar a orçar por divisão.
fn chave_legada(categoria: &str) -> &str {
    match categoria {
        "toyota_rookie" => "mazda_rookie",
        "toyota_amador" => "mazda_amador",
        "production_challenger" => "bmw_m2",
        outro => outro,
    }
}

/// **A razão travada.** O caixa esperado da tabela antiga era ~2,05× o custo operacional em
/// TODA a escada — nunca declarado, sempre presente. Era ela que mantinha estável toda conta
/// do tipo estoque÷fluxo do jogo, e é a quebra dela que move `financial_health_score`,
/// `spending_power` e o teto salarial de uma vez só.
#[test]
fn a_tabela_antiga_mandava_guardar_dois_anos_de_operacao() {
    for (categoria, _, _) in CAIXA_LEGADO {
        let meses = caixa_legado(categoria) / (operacional_legado(categoria) / 12.0);
        assert!(
            (22.0..27.0).contains(&meses),
            "{categoria}: a tabela velha implicava {meses:.1} meses de caixa"
        );
    }
}

/// A escala de peça antiga (`car::cost::category_cost_scale` antes de ser descongelada).
/// Também congelada, e pelo mesmo motivo: a função de produção agora deriva da âncora, e
/// comparar com ela mesma não mediria nada.
const ESCALA_DE_PECA_LEGADA: [(&str, f64); 7] = [
    ("mazda_rookie", 120.0),
    ("mazda_amador", 280.0),
    ("bmw_m2", 715.0),
    ("gt4", 1_800.0),
    ("gt3", 5_200.0),
    ("lmp2", 8_800.0),
    ("endurance", 10_700.0),
];

pub fn escala_de_peca_legada(categoria: &str) -> f64 {
    ESCALA_DE_PECA_LEGADA
        .iter()
        .find(|(c, _)| *c == chave_legada(categoria))
        .map(|(_, v)| *v)
        .unwrap_or(715.0)
}

/// Amplitude da escada antiga: 16,5 milhões do Endurance sobre 185 mil do Rookie.
pub fn amplitude_legada() -> f64 {
    operacional_legado("endurance") / operacional_legado("mazda_rookie")
}

/// O coeficiente que a escala de peça sempre embutiu, agora explícito em
/// `car::cost::PART_SCALE_OF_OPERATING`.
pub const COEFICIENTE_DE_PECA: f64 = 0.00065;

/// A prova de que a escala de peça era uma CÓPIA da âncora, não uma segunda fonte: os sete
/// números escritos à mão são `operating_cost_midpoint × 0,00065` do legado, dentro de 4%.
///
/// Este teste não pode mais ser feito contra a função de produção (ela agora deriva, então
/// a igualdade seria trivial). Contra os dois congelados ele continua dizendo a mesma coisa,
/// que é o motivo pelo qual descongelar era obrigatório e não cosmético.
#[test]
fn a_escala_de_peca_antiga_era_copia_da_ancora_antiga() {
    for (categoria, _, _) in OPERACIONAL_LEGADO {
        let escrito = escala_de_peca_legada(categoria);
        let derivado = operacional_legado(categoria) * COEFICIENTE_DE_PECA;
        let desvio = (escrito / derivado - 1.0).abs();
        assert!(
            desvio < 0.04,
            "{categoria}: {escrito} não é {derivado:.0} — a premissa da troca caiu"
        );
    }
}

/// E o coeficiente congelado é o mesmo que a produção passou a usar ao vivo. Se alguém
/// mexer num sem mexer no outro, o preço de peça descola da âncora de novo — que é
/// exatamente o estado do qual esta rodada saiu.
#[test]
fn o_coeficiente_de_peca_da_producao_e_o_mesmo_do_legado() {
    assert!((crate::car::cost::PART_SCALE_OF_OPERATING - COEFICIENTE_DE_PECA).abs() < 1e-12);
}
