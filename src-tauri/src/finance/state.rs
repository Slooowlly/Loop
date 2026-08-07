//! Saúde financeira de uma equipe.
//!
//! # A reancoragem em MESES DE OPERAÇÃO
//!
//! O estado do mundo (`financial_state`) era um score 0–100 composto de seis termos, binado
//! em 70/55/40/25/12. O termo dominante era `caixa ÷ expected_cash_midpoint`, e esse divisor
//! é a âncora de ESTOQUE da seção 2.1 do redesign — um número escrito à mão que a auditoria
//! da seção 3.3.3 mediu contra a conta bottom-up e achou **torto, não só errado**:
//!
//! | divisão | custo real/ano | midpoint antigo | quão folgado o divisor deixa a equipe |
//! |---|---:|---:|---:|
//! | rookie | ~210 k | 185 k | 0,88× (APERTA) |
//! | amador | ~394 k | 425 k | 1,08× |
//! | gt4 | 1,46 M | 2,75 M | 1,9× |
//! | gt3 | 3,37 M | 8,0 M | 2,4× |
//! | endurance (gt4) | 1,57 M | 16,5 M | **10,5×** |
//!
//! A mesma equipe, com o mesmo caixa em meses de operação, era lida como "elite" no Endurance
//! e como "pressionada" na Rookie. **Toda taxa de crise medida contra esse instrumento é
//! leitura de termômetro descalibrado** — inclusive os 8–15% do critério 7 da Parte 4.
//!
//! A unidade nova é a que a seção 3.2 já pedia: **quantos meses de operação a equipe consegue
//! bancar**. Ela é comparável entre categorias por construção, é o que uma equipe de corrida
//! de verdade acompanha, e é a mesma unidade em que `economia::desenvolvimento` decide
//! investir (reserva de 12 meses).

use crate::finance::planning::{calculate_financial_plan, category_finance_scale};
use crate::models::team::Team;

/// Custo de operar UM ANO nesta divisão.
///
/// Delega para `economia::temporada`, que é a conta bottom-up de verdade (folha técnica,
/// sede, frota, seguros, mais os eventos da temporada). Não há tabela aqui de propósito: uma
/// segunda cópia da âncora é exatamente o defeito que `car::cost::category_cost_scale`
/// cometeu — "uma cópia velha que não sabe que é cópia", nas palavras da seção 3.3.4.
///
/// O Endurance resolve **por classe** porque é onde o erro da âncora velha era maior: um
/// midpoint único de 16,5 M orçava uma equipe de GT4 como se fosse LMP2, 10,5× o custo real
/// dela.
pub fn custo_operacional_anual(categoria: &str, classe: Option<&str>) -> f64 {
    crate::economia::temporada::custo_operacional_anual_de_referencia(categoria, classe)
}

/// Custo de operar um MÊS. É o divisor da unidade nova.
pub fn custo_operacional_mensal(categoria: &str, classe: Option<&str>) -> f64 {
    custo_operacional_anual(categoria, classe) / 12.0
}

/// Quantos meses de operação a equipe consegue bancar com o que tem hoje.
///
/// Dívida entra abatendo: quem deve mais do que tem não tem fôlego, tem passivo — e é assim
/// que a T101 do save real (24,6 M de dívida) aparece como colapso em vez de aparecer como
/// uma equipe de caixa razoável.
pub fn meses_de_operacao(team: &Team) -> f64 {
    let mensal = custo_operacional_mensal(&team.categoria, team.classe.as_deref());
    if mensal <= 0.0 {
        return 0.0;
    }
    (team.cash_balance - team.debt_balance) / mensal
}

/// As fronteiras entre os seis estados, em MESES de operação. Cada campo é o piso do estado
/// que o nomeia; abaixo de `crise` é colapso.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaixasDeMeses {
    pub elite: f64,
    pub saudavel: f64,
    pub estavel: f64,
    pub pressionada: f64,
    pub crise: f64,
}

impl Default for FaixasDeMeses {
    /// Calibrado no harness de economia (`varrer_estado_financeiro`) contra a distribuição
    /// de população que a seção 2.9 diz que o mundo deveria ter: elite rara, crise visível.
    ///
    /// O piso de `saudavel` em 12 meses não é arbitrário — é a mesma reserva acima da qual
    /// `economia::desenvolvimento` deixa a equipe investir o excedente. Os dois conceitos
    /// precisam concordar, senão uma equipe "saudável" seria uma que não pode investir.
    fn default() -> Self {
        Self {
            elite: 24.0,
            saudavel: 12.0,
            estavel: 6.0,
            pressionada: 3.0,
            crise: 0.0,
        }
    }
}

/// O estado, a partir dos meses de operação.
pub fn estado_por_meses(meses: f64, faixas: FaixasDeMeses) -> &'static str {
    if meses >= faixas.elite {
        "elite"
    } else if meses >= faixas.saudavel {
        "healthy"
    } else if meses >= faixas.estavel {
        "stable"
    } else if meses >= faixas.pressionada {
        "pressured"
    } else if meses >= faixas.crise {
        "crisis"
    } else {
        "collapse"
    }
}

/// Quantos estados a escada tem. Cada um ocupa uma FATIA IGUAL da escala 0–100 em
/// [`posicao_na_escada`] — é a única constante do mapeamento, e ela não é calibrada: é a
/// contagem de `estado_por_meses`.
pub const ESTADOS_DA_ESCADA: f64 = 6.0;

/// Os pisos das cinco bandas nomeadas, em ordem crescente. `colapso` é a banda aberta
/// abaixo de `crise`, e `elite` é a banda aberta acima do último piso.
fn pisos(faixas: FaixasDeMeses) -> [f64; 5] {
    [
        faixas.crise,
        faixas.pressionada,
        faixas.estavel,
        faixas.saudavel,
        faixas.elite,
    ]
}

/// **Meses de operação, na escala 0–100 da escada de estados.**
///
/// A régua contínua que faltava. `estado_por_meses` responde *em que banda a equipe está*;
/// esta responde *onde dentro da escada inteira*, sem perder a posição dentro da banda.
///
/// A regra é uma só, e não tem constante calibrada: **cada um dos seis estados ocupa uma
/// fatia igual da escala** (100 ÷ 6 ≈ 16,7 pontos), e dentro da fatia a posição é linear
/// entre o piso e o teto da banda. Isso dá ao índice um significado que se lê sem tabela —
/// 50,0 é o piso de `estavel`, 66,7 o de `saudavel`, 83,3 o de `elite` — e faz o índice
/// herdar a calibração das faixas em vez de fundar uma segunda em paralelo. Quem
/// recalibrar `FaixasDeMeses` move os dois juntos.
///
/// # As duas bandas abertas
///
/// `colapso` não tem piso e `elite` não tem teto, e é exatamente aí que uma janela linear
/// empilharia o grid nas pontas: o mundo em regime roda a 10,9–22,1 meses de mediana, com
/// campeão medido em 78,4 — quatro vezes o piso de `elite`. As duas bandas abertas usam
/// portanto um mapa que **satura sem nunca chegar ao extremo**: `x ÷ (x + largura)`, com
/// `largura` sendo a da banda fechada vizinha. A afirmação é que a banda aberta avança na
/// mesma taxa que a fechada ao lado dela e depois desacelera — 0 e 100 são limites, não
/// valores. Um índice que ninguém atinge é um índice que ninguém empilha.
pub fn posicao_na_escada(meses: f64, faixas: FaixasDeMeses) -> f64 {
    let pisos = pisos(faixas);
    let fatia = 100.0 / ESTADOS_DA_ESCADA;

    // Colapso: banda aberta para baixo. Escala de saturação = largura da banda de crise.
    if meses < pisos[0] {
        let profundidade = pisos[0] - meses;
        let largura = (pisos[1] - pisos[0]).max(f64::EPSILON);
        return fatia * (1.0 - profundidade / (profundidade + largura));
    }

    for i in 0..4 {
        if meses < pisos[i + 1] {
            let largura = pisos[i + 1] - pisos[i];
            let dentro = if largura > 0.0 {
                (meses - pisos[i]) / largura
            } else {
                0.0
            };
            return fatia * (1.0 + i as f64 + dentro);
        }
    }

    // Elite: banda aberta para cima. Escala de saturação = largura da banda de saudável.
    let acima = meses - pisos[4];
    let largura = (pisos[4] - pisos[3]).max(f64::EPSILON);
    fatia * (5.0 + acima / (acima + largura))
}

/// A inversa de [`posicao_na_escada`]: quantos meses de operação valem este ponto da
/// escala 0–100.
///
/// Existe porque há consumidor que fala em PONTOS de índice e precisa pagar em dinheiro —
/// `promotion::effects` move a equipe N pontos ao promovê-la. Com a escala nova um ponto
/// não vale um pedaço fixo do caixa da categoria, então a conversão só fecha passando pela
/// mesma escada, nos dois sentidos.
///
/// Os extremos são assíntotas: 0 e 100 não têm pré-imagem finita, e a função os grampeia na
/// borda da última banda representável em vez de devolver infinito.
pub fn meses_na_posicao(indice: f64, faixas: FaixasDeMeses) -> f64 {
    let pisos = pisos(faixas);
    let fatia = 100.0 / ESTADOS_DA_ESCADA;
    // 1e-6 de fatia é a borda representável: mais perto do extremo e a divisão explode.
    let indice = indice.clamp(fatia * 1e-6, 100.0 - fatia * 1e-6);

    if indice < fatia {
        let largura = (pisos[1] - pisos[0]).max(f64::EPSILON);
        let t = indice / fatia; // t = largura/(profundidade+largura)
        return pisos[0] - largura * (1.0 - t) / t;
    }

    let banda = ((indice / fatia).floor() as usize).min(5);
    if banda >= 5 {
        let largura = (pisos[4] - pisos[3]).max(f64::EPSILON);
        let t = indice / fatia - 5.0; // t = acima/(acima+largura)
        return pisos[4] + largura * t / (1.0 - t);
    }

    let i = banda - 1;
    let dentro = indice / fatia - 1.0 - i as f64;
    pisos[i] + (pisos[i + 1] - pisos[i]) * dentro
}

/// **Instrumento LEGADO.** Bina o score composto 0–100 de [`financial_health_score`], cujo
/// termo dominante divide o caixa pela âncora de estoque torta descrita no topo do módulo.
///
/// Não é mais o que define `Team::financial_state` — [`refresh_team_financial_state`] passou
/// a usar [`estado_por_meses`]. Continua exportado porque a auditoria de âncoras em
/// `economia::tests` mede justamente a distorção dele, e apagá-lo apagaria a evidência.
pub fn derive_financial_state(score: f64) -> &'static str {
    match score {
        value if value >= 70.0 => "elite",
        value if value >= 55.0 => "healthy",
        value if value >= 40.0 => "stable",
        value if value >= 25.0 => "pressured",
        value if value >= 12.0 => "crisis",
        _ => "collapse",
    }
}

pub fn financial_health_score(team: &Team) -> f64 {
    let plan = calculate_financial_plan(team);
    let scale = category_finance_scale(&team.categoria);
    let cash_score =
        (team.cash_balance / scale.expected_cash_midpoint() * 65.0).clamp(-20.0, 100.0);
    let spending_score =
        (plan.spending_power / scale.operating_cost_midpoint() * 55.0).clamp(-25.0, 100.0);
    let debt_penalty =
        (plan.debt_pressure / scale.expected_cash_midpoint() * 80.0).clamp(0.0, 70.0);
    let structure_score = ((team.engineering + team.facilities) / 2.0).clamp(0.0, 100.0);
    let support_score = ((plan.budget_index + team.reputacao) / 2.0).clamp(0.0, 100.0);
    let momentum_score =
        (team.last_round_net / 50_000.0).clamp(-15.0, 15.0) + team.stats_pontos as f64 * 0.05;

    (cash_score * 0.32
        + spending_score * 0.28
        + structure_score * 0.20
        + support_score * 0.15
        + momentum_score * 0.05
        - debt_penalty)
        .clamp(0.0, 100.0)
}

pub fn choose_season_strategy(team: &Team) -> &'static str {
    let plan = calculate_financial_plan(team);
    let scale = category_finance_scale(&team.categoria);

    if plan.debt_pressure >= scale.expected_cash_midpoint() * 0.75 {
        return "survival";
    }

    // Sem caixa E com carro fraco → aposta tudo. Limiares convertidos do domínio 0–16 para
    // a escala única 0–100 de `car_strength` (6/16 ≈ 38; 8/16 = 50).
    if plan.spending_power < scale.operating_cost_midpoint() * 0.20 && team.car_strength() < 38.0 {
        return "all_in";
    }

    // O estado vem da unidade nova: estratégia e `financial_state` precisam concordar, senão
    // a equipe se comporta segundo um termômetro e é exibida segundo outro.
    //
    // Os DOIS guardas acima continuam ancorados nos divisores velhos (`expected_cash_midpoint`
    // e `operating_cost_midpoint`, via `calculate_financial_plan`). Não foram reancorados aqui
    // de propósito: `finance/planning.rs` está sendo trocado por outra sessão, e mexer nos
    // dois lados ao mesmo tempo tornaria a medição ilegível.
    match estado_por_meses(meses_de_operacao(team), FaixasDeMeses::default()) {
        "elite" => "balanced",
        "healthy" => {
            if team.car_strength() < 50.0 {
                "expansion"
            } else {
                "balanced"
            }
        }
        "stable" => {
            if plan.spending_power < scale.operating_cost_midpoint() * 0.50 {
                "austerity"
            } else {
                "balanced"
            }
        }
        "pressured" => "all_in",
        "crisis" | "collapse" => "survival",
        _ => "balanced",
    }
}

pub fn refresh_team_financial_state(team: &mut Team) {
    refresh_team_financial_state_com(team, FaixasDeMeses::default());
}

/// Variante com fronteiras explícitas. Existe para o harness de economia poder varrer as
/// faixas sem recompilar — a distribuição de população que elas produzem é uma decisão de
/// design, e decisão de design se mede antes de escrever.
pub fn refresh_team_financial_state_com(team: &mut Team, faixas: FaixasDeMeses) {
    team.financial_state = estado_por_meses(meses_de_operacao(team), faixas).to_string();
}

#[cfg(test)]
mod tests_escada {
    use super::*;

    /// A escada é monotônica e mora inteira dentro de 0–100, incluindo as duas bandas
    /// abertas — em meses absurdos dos dois lados.
    #[test]
    fn a_escada_e_monotonica_e_nunca_sai_de_zero_a_cem() {
        let faixas = FaixasDeMeses::default();
        let mut anterior = f64::MIN;
        for passo in -200..=2000 {
            let meses = passo as f64 / 4.0;
            let indice = posicao_na_escada(meses, faixas);
            assert!(
                (0.0..=100.0).contains(&indice),
                "{meses:.1} meses saiu da escala: {indice}"
            );
            assert!(
                indice >= anterior,
                "a escada desceu em {meses:.1} meses: {anterior:.4} → {indice:.4}"
            );
            anterior = indice;
        }
    }

    /// **As pontas são assíntotas, não valores.** É a propriedade que impede o empilhamento
    /// em 0 e 100 que derrubou a fórmula anterior do índice de orçamento: por mais fôlego
    /// que a equipe tenha, sempre sobra escala acima dela.
    #[test]
    fn as_bandas_abertas_saturam_sem_chegar_no_extremo() {
        let faixas = FaixasDeMeses::default();

        assert!(posicao_na_escada(1_000.0, faixas) < 100.0);
        assert!(posicao_na_escada(10_000.0, faixas) > posicao_na_escada(1_000.0, faixas));
        assert!(posicao_na_escada(-1_000.0, faixas) > 0.0);
        assert!(posicao_na_escada(-10_000.0, faixas) < posicao_na_escada(-1_000.0, faixas));
    }

    /// A inversa fecha nos dois sentidos — inclusive dentro das bandas abertas, que é onde
    /// `promotion::effects` precisa dela e onde a fórmula não é linear.
    #[test]
    fn a_inversa_da_escada_fecha_o_ciclo() {
        let faixas = FaixasDeMeses::default();
        for passo in -40..=800 {
            let meses = passo as f64 / 4.0;
            let volta = meses_na_posicao(posicao_na_escada(meses, faixas), faixas);
            assert!(
                (volta - meses).abs() < 0.01,
                "{meses:.2} meses voltaram como {volta:.2}"
            );
        }
    }

    /// A escada e a binagem contam a mesma história: o índice de uma equipe cai na fatia do
    /// estado que `estado_por_meses` declara para ela. Se as duas discordarem, a equipe é
    /// exibida segundo um instrumento e lida segundo outro.
    #[test]
    fn a_fatia_da_escada_concorda_com_o_estado_binado() {
        let faixas = FaixasDeMeses::default();
        let fatia = 100.0 / ESTADOS_DA_ESCADA;
        for passo in -40..=400 {
            let meses = passo as f64 / 4.0;
            let esperado = match estado_por_meses(meses, faixas) {
                "collapse" => 0,
                "crisis" => 1,
                "pressured" => 2,
                "stable" => 3,
                "healthy" => 4,
                _ => 5,
            };
            let lida = ((posicao_na_escada(meses, faixas) / fatia).floor() as usize).min(5);
            assert_eq!(
                lida, esperado,
                "{meses:.2} meses: estado diz fatia {esperado}, escada diz {lida}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::team::{placeholder_team_from_db, Team};

    fn sample_team(category: &str, cash: f64, debt: f64, state: &str) -> Team {
        let mut team = placeholder_team_from_db(
            "T999".to_string(),
            "Equipe Estado".to_string(),
            category.to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = cash;
        team.debt_balance = debt;
        team.financial_state = state.to_string();
        team.reputacao = 45.0;
        team.engineering = 45.0;
        team.facilities = 45.0;
        team.car_performance = 8.0;
        team
    }

    #[test]
    fn high_financial_health_maps_to_elite() {
        assert_eq!(derive_financial_state(90.0), "elite");
    }

    #[test]
    fn low_financial_health_maps_to_collapse() {
        assert_eq!(derive_financial_state(10.0), "collapse");
    }

    #[test]
    fn rich_structured_team_scores_as_elite() {
        let mut team = placeholder_team_from_db(
            "T001".to_string(),
            "Equipe Rica".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = 24_000_000.0;
        team.debt_balance = 0.0;
        team.budget = 82.0;
        team.reputacao = 85.0;
        team.engineering = 88.0;
        team.facilities = 84.0;

        assert_eq!(
            derive_financial_state(financial_health_score(&team)),
            "elite"
        );
        assert_eq!(choose_season_strategy(&team), "balanced");
    }

    #[test]
    fn indebted_team_falls_into_survival_mode() {
        let mut team = placeholder_team_from_db(
            "T002".to_string(),
            "Equipe Quebrada".to_string(),
            "gt4".to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = -250_000.0;
        team.debt_balance = 1_200_000.0;
        team.budget = 18.0;
        team.engineering = 28.0;
        team.facilities = 24.0;
        team.last_round_net = -90_000.0;

        assert_eq!(
            derive_financial_state(financial_health_score(&team)),
            "collapse"
        );
        assert_eq!(choose_season_strategy(&team), "survival");
    }

    #[test]
    fn low_budget_field_does_not_hide_healthy_cash_position() {
        let mut team = sample_team("gt4", 6_500_000.0, 0.0, "stable");
        team.budget = 1.0;
        team.reputacao = 20.0;
        team.engineering = 40.0;
        team.facilities = 40.0;

        assert!(
            financial_health_score(&team) > 55.0,
            "healthy cash should matter more than legacy budget field"
        );
    }

    /// Austeridade agora é decidida em MESES de operação, não contra a âncora de estoque.
    ///
    /// O caso antigo deste teste (uma GT4 com 3,5 M em caixa) mudou de veredito com a
    /// reancoragem, e a mudança é o conserto, não uma regressão: 3,5 M dividido pelo custo
    /// real de operar uma GT4 (1,46 M/ano) são **28,7 meses de fôlego**. Chamar isso de
    /// austeridade só fazia sentido contra um divisor 1,9× inflado. Quem entra em austeridade
    /// é quem está entre 6 e 12 meses e sem poder de gasto.
    /// A estratégia passa a ler o estado em MESES de operação.
    ///
    /// O caso antigo deste teste (uma GT4 com 3,5 M em caixa lida como "austeridade") mudou
    /// de veredito, e a mudança é o conserto: 3,5 M dividido pelo custo real de operar uma
    /// GT4 (1,46 M/ano) são **28,7 meses de fôlego**. Chamar isso de austeridade só fazia
    /// sentido contra um divisor 1,9× inflado.
    ///
    /// O que este teste NÃO afirma: o ramo `austerity`. Ele depende do guarda
    /// `spending_power < operating_cost_midpoint × 0,50`, que continua ancorado no divisor
    /// velho de propósito (ver o comentário em `choose_season_strategy`) — afirmá-lo aqui
    /// seria travar um comportamento que ainda vai mudar quando `planning.rs` for trocado.
    #[test]
    fn estrategia_le_meses_de_operacao() {
        let mut team = sample_team("gt4", 3_500_000.0, 0.0, "stable");
        team.budget = 99.0;
        team.reputacao = 75.0;
        team.engineering = 70.0;
        team.facilities = 70.0;
        team.car_performance = 9.0;
        assert!(meses_de_operacao(&team) > 24.0);
        assert_eq!(choose_season_strategy(&team), "balanced");

        // Fôlego de um mês: crise, e o plano vira sobrevivência.
        team.cash_balance = custo_operacional_mensal("gt4", None);
        assert_eq!(choose_season_strategy(&team), "survival");
    }

    /// A unidade nova é comparável entre categorias — que é a coisa toda. A mesma equipe, com
    /// o mesmo fôlego real, tem que ler o mesmo estado em qualquer degrau da escada.
    #[test]
    fn o_mesmo_folego_le_o_mesmo_estado_em_toda_a_escada() {
        for (categoria, classe) in [
            ("mazda_rookie", None),
            ("bmw_m2", None),
            ("gt4", None),
            ("gt3", None),
            ("endurance", Some("gt4")),
            ("endurance", Some("lmp2")),
        ] {
            let mensal = custo_operacional_mensal(categoria, classe);
            let mut team = sample_team(categoria, mensal * 9.0, 0.0, "stable");
            team.classe = classe.map(str::to_string);
            assert_eq!(
                estado_por_meses(meses_de_operacao(&team), FaixasDeMeses::default()),
                "stable",
                "{categoria}/{classe:?} leu diferente com o mesmo fôlego"
            );
        }
    }

    /// A dívida abate o fôlego. É o caso T101 do save real: 24,6 M de dívida numa GT3 não é
    /// uma equipe de caixa razoável, é colapso.
    #[test]
    fn divida_derruba_o_folego() {
        // 6 M numa GT3 (3,37 M/ano) são 21,3 meses — saudável, não elite. A âncora velha
        // dizia 6/15,5 = 0,39 do "caixa-médio" e a lia como apertada.
        let mut team = sample_team("gt3", 6_000_000.0, 0.0, "stable");
        assert_eq!(
            estado_por_meses(meses_de_operacao(&team), FaixasDeMeses::default()),
            "healthy"
        );
        team.debt_balance = 24_600_000.0;
        assert!(meses_de_operacao(&team) < 0.0);
        assert_eq!(
            estado_por_meses(meses_de_operacao(&team), FaixasDeMeses::default()),
            "collapse"
        );
    }

    /// GUARD: a âncora viva não pode se afastar da tabela publicada na **seção 3.3.3** do
    /// `docs/economia-redesign.md` sem que alguém saiba.
    ///
    /// A tabela do doc é o número que sustentou a decisão de reancorar; a âncora viva é
    /// `economia::temporada`, que continua sendo refinada. Enquanto as duas concordarem, o
    /// documento e o código contam a mesma história. Quando divergirem, este teste falha e a
    /// pergunta certa é qual dos dois está desatualizado — não é para "consertar" mexendo na
    /// tolerância.
    #[test]
    fn a_ancora_viva_ainda_bate_com_a_tabela_do_doc() {
        // (categoria, classe, total/ano publicado em 3.3.3)
        const PUBLICADO: &[(&str, Option<&str>, f64)] = &[
            ("mazda_rookie", None, 210_000.0),
            ("toyota_rookie", None, 210_000.0),
            ("mazda_amador", None, 394_000.0),
            ("toyota_amador", None, 394_000.0),
            ("gt4", None, 1_461_012.0),
            ("gt3", None, 3_372_419.0),
            ("lmp2", None, 4_218_470.0),
            ("endurance", Some("gt4"), 1_570_000.0),
            ("endurance", Some("gt3"), 3_190_000.0),
            ("endurance", Some("lmp2"), 4_280_000.0),
        ];
        for (categoria, classe, publicado) in PUBLICADO {
            let vivo = custo_operacional_anual(categoria, *classe);
            let desvio = (vivo - publicado).abs() / publicado;
            assert!(
                desvio < 0.25,
                "{categoria}/{classe:?}: doc diz {publicado:.0}, código diz {vivo:.0} ({:.0}% de desvio)",
                desvio * 100.0
            );
        }
    }

    /// O Endurance por CLASSE é onde a âncora velha errava mais (10,5× na GT4). Uma equipe de
    /// GT4 no Endurance não pode ser lida como uma de LMP2.
    #[test]
    fn endurance_le_por_classe() {
        let gt4 = custo_operacional_mensal("endurance", Some("gt4"));
        let lmp2 = custo_operacional_mensal("endurance", Some("lmp2"));
        assert!(lmp2 > gt4 * 2.5, "gt4 {gt4:.0} · lmp2 {lmp2:.0}");

        // Mesmo caixa, classes diferentes → fôlegos diferentes, como tem que ser.
        let mut equipe_gt4 = sample_team("endurance", 3_000_000.0, 0.0, "stable");
        equipe_gt4.classe = Some("gt4".to_string());
        let mut equipe_lmp2 = sample_team("endurance", 3_000_000.0, 0.0, "stable");
        equipe_lmp2.classe = Some("lmp2".to_string());
        assert!(meses_de_operacao(&equipe_gt4) > meses_de_operacao(&equipe_lmp2) * 2.5);
    }
}
