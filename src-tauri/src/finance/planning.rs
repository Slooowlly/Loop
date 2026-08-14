use crate::models::team::Team;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CategoryFinanceScale {
    pub cash_min: f64,
    pub cash_max: f64,
    pub operating_cost_min: f64,
    pub operating_cost_max: f64,
}

impl CategoryFinanceScale {
    pub fn expected_cash_midpoint(self) -> f64 {
        (self.cash_min + self.cash_max) / 2.0
    }

    pub fn operating_cost_midpoint(self) -> f64 {
        (self.operating_cost_min + self.operating_cost_max) / 2.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamFinancialPlan {
    pub projected_income: f64,
    pub committed_costs: f64,
    pub safety_reserve: f64,
    pub available_credit: f64,
    pub debt_pressure: f64,
    pub spending_power: f64,
    pub budget_index: f64,
}

/// Caixa esperado por divisão — a âncora de ESTOQUE, agora também derivada.
///
/// **Deixou de ser tabela.** A seção 3.2 do redesign diz que o caixa esperado é uma
/// consequência: quantos meses de operação a equipe consegue bancar. É exatamente isso que
/// [`crate::economia::temporada::faixa_de_caixa`] devolve — a faixa vai de 1 mês (equipe que
/// vive de etapa em etapa) a 11 (quase uma temporada guardada), sobre o custo operacional
/// que a conta física produz.
///
/// A tabela que saiu daqui declarava 6 a 25 milhões na GT3 sem dizer em que unidade isso
/// estava. Medido, estava em meses — `expected_cash_midpoint` era ~2,05× o
/// `operating_cost_midpoint` em TODA a escada, ou seja, ela mandava a equipe mediana guardar
/// ~24 meses de operação. Nenhuma equipe de corrida de cliente opera assim, e essa razão
/// travada em 2,05 era o que mantinha estável toda conta do tipo estoque÷fluxo do jogo.
fn category_cash_scale(category: &str, classe: Option<&str>) -> (f64, f64) {
    crate::economia::temporada::faixa_de_caixa(category, classe)
}

/// Escala financeira de uma DIVISÃO competitiva.
///
/// O custo operacional **deixou de ser tabela**: vem de
/// [`crate::economia::temporada::faixa_de_custo_operacional_anual`], que o constrói de baixo
/// para cima — quilômetros, litros, jogos de pneu, gente, sede, frota, seguro. A tabela
/// antiga ia de 185 mil (rookie) a 16,5 milhões (endurance), 89× de escada; a conta física
/// dá ~20×, e principalmente ela é TORTA em relação à antiga: subfinanciava a Rookie em 14%
/// e superfinanciava o GT4 do Endurance em 11×.
///
/// A `classe` importa porque `production_challenger` e `endurance` são campeonatos
/// multi-classe e um único orçamento para os três não existe: no Endurance, um GT4 e um
/// LMP2 dividiam um midpoint de 16,5 milhões que não descreve nenhum dos dois. Mesmo padrão
/// de [`crate::car::cost::category_ceiling_for`], que já resolvia teto de peça por classe
/// pelo mesmo motivo.
///
/// O caixa (`cash_min`/`cash_max`) continua vindo da tabela — ver [`category_cash_scale`].
///
/// Aceita a chave com classe embutida (`"endurance:gt3"`); sem classe, cai na divisão de
/// referência do campeonato. Par exato de [`crate::car::cost::category_ceiling`], que
/// resolve o teto de peça pela mesma convenção e pelo mesmo motivo.
pub fn category_finance_scale(category: &str) -> CategoryFinanceScale {
    let base = category.split(':').next().unwrap_or(category);
    let classe = category
        .split_once(':')
        .map(|(_, classe)| classe)
        .filter(|classe| !classe.is_empty());
    category_finance_scale_for(base, classe)
}

/// Escala financeira com a classe EXPLÍCITA — a forma que quem tem a equipe em mãos deve
/// usar, porque a classe mora em `Team::classe` e não na string da categoria.
///
/// Sem classe, um campeonato multi-classe resolve para a divisão de referência
/// ([`representative_division_for_tier`]): é uma aproximação com perda, e é por isso que a
/// forma explícita existe.
pub fn category_finance_scale_for(category: &str, classe: Option<&str>) -> CategoryFinanceScale {
    let classe = classe.filter(|c| !c.is_empty()).or_else(|| {
        // Campeonato multi-classe consultado sem classe: usa a de referência em vez de
        // cair no fallback genérico da âncora, que devolveria um bmw_m2. A pergunta aqui é
        // "quanto custa OPERAR", então a resposta é a divisão TÍPICA — ver
        // `economia::divisao`, onde as duas respostas possíveis moram lado a lado.
        crate::economia::divisao::classe_de_referencia(
            category,
            crate::economia::divisao::ClasseDeReferencia::Tipica,
        )
    });
    let (cash_min, cash_max) = category_cash_scale(category, classe);
    let (operating_cost_min, operating_cost_max) =
        crate::economia::temporada::faixa_de_custo_operacional_anual(category, classe);

    CategoryFinanceScale {
        cash_min,
        cash_max,
        operating_cost_min,
        operating_cost_max,
    }
}

/// Divisão representativa de um tier: `(categoria, classe)`.
///
/// Existe porque cinco consumidores da base salarial só têm o TIER em mãos
/// (`models::contract::salary_range_for_tier` e a cadeia do mercado), e não a divisão.
///
/// Para os campeonatos multi-classe é uma aproximação com perda declarada, e a escolha da
/// classe representativa não é neutra: o tier existe aqui para posicionar SALÁRIO, e o
/// mercado de um tier é o do trabalho mais caro que ele oferece. Por isso o tier 6 resolve
/// para `endurance:lmp2`, a classe de ápice — não para a classe do meio.
///
/// Escolher `endurance:gt3` (a classe que o funil de promoção alimenta) parecia natural e
/// quebrava a escada: o GT3 do Endurance custa MENOS que uma operação de LMP2 do tier 5, e
/// o tier 6 passava a pagar menos que o 5. A monotonicidade da escada salarial não é uma
/// coincidência que dá para perder — `contract::salary_range_for_tier` a assevera.
///
/// Mesmo com o ápice, a distância entre os tiers 5 e 6 fica em ~1%: são o mesmo carro e o
/// mesmo tamanho de equipe. Isso é o modelo físico dizendo que LMP2 de sprint e LMP2 de
/// endurance são a mesma operação, e não um erro de calibração.
///
/// Quem TEM a divisão em mãos deve usar [`category_finance_scale_for`] direto — é o caso de
/// `finance::salary::calculate_salary_ceiling`, que lê a classe da própria equipe.
pub fn representative_division_for_tier(tier: u8) -> (&'static str, Option<&'static str>) {
    match tier {
        0 => ("mazda_rookie", None),
        1 => ("mazda_amador", None),
        2 => ("bmw_m2", None),
        3 => ("gt4", None),
        4 => ("gt3", None),
        5 => ("lmp2", None),
        // Tier 6: a pergunta é "quanto se PAGA", então a resposta é o ÁPICE — ver
        // `economia::divisao`, que guarda esta escolha ao lado da outra (a TÍPICA, que
        // `category_finance_scale_for` usa) em vez de deixá-las espalhadas.
        _ => (
            "endurance",
            crate::economia::divisao::classe_de_referencia(
                "endurance",
                crate::economia::divisao::ClasseDeReferencia::Apice,
            ),
        ),
    }
}

/// Custo operacional médio de cada tier — a MESMA escala de `category_finance_scale`,
/// resolvida por tier em vez de por divisão. É a âncora da base salarial
/// (`finance::salary::category_salary_base_for_tier`): a escada de salário é DERIVADA
/// desta, não escrita à mão em paralelo.
///
/// Com a âncora bottom-up a escada salarial comprime junto com ela: a base por piloto no
/// topo cai de ~1,05 milhão para ~215 mil, porque os 15% incidem sobre um operacional que
/// deixou de ser inflado. A base do rookie sobe de ~11,8 mil para ~13,3 mil pelo mesmo
/// motivo, ao contrário.
pub fn operating_cost_midpoint_for_tier(tier: u8) -> f64 {
    let (category, classe) = representative_division_for_tier(tier);
    category_finance_scale_for(category, classe).operating_cost_midpoint()
}

pub fn income_confidence_for_state(state: &str) -> f64 {
    match state {
        "elite" => 0.90,
        "healthy" => 0.80,
        "stable" => 0.60,
        "pressured" => 0.45,
        "crisis" => 0.35,
        "collapse" => 0.25,
        _ => 0.60,
    }
}

pub fn credit_aggressiveness_for_state(state: &str) -> f64 {
    match state {
        "elite" => 0.10,
        "healthy" => 0.20,
        "stable" => 0.30,
        "pressured" => 0.55,
        "crisis" => 0.75,
        "collapse" => 0.40,
        _ => 0.30,
    }
}

/// **Instrumento LEGADO.** A reserva de segurança como múltiplo do custo operacional ANUAL.
///
/// Os valores dizem o que a tabela velha achava razoável guardar: 1,50 do anual para uma
/// equipe elite são **dezoito meses** de operação parados no banco. Isso fazia sentido
/// enquanto o caixa esperado da categoria valia ~24 meses; com o caixa virando 1–11 meses,
/// uma reserva de 18 excede tudo que a equipe pode ter, e o poder de gasto vira negativo
/// por construção.
///
/// Continua exportado porque é a régua histórica de
/// `commands::race::despesa::relatorio_poder_de_gasto_contra_a_ancora_velha`, que mede
/// exatamente esse deslocamento. **Não é mais usado pelo planejamento** — ver
/// [`RESERVA_DE_SOBREVIVENCIA_EM_MESES`].
pub fn safety_reserve_multiplier_for_state(state: &str) -> f64 {
    match state {
        "elite" => 1.50,
        "healthy" => 1.20,
        "stable" => 0.90,
        "pressured" => 0.45,
        "crisis" => 0.10,
        "collapse" => 0.00,
        _ => 0.90,
    }
}

/// O caixa que a equipe não gasta, em MESES de operação.
///
/// Não é um número novo: é o piso de `pressionada` em
/// [`crate::finance::state::FaixasDeMeses`] — a linha abaixo da qual o mundo já declara que
/// a equipe está em crise. A afirmação é simples e verificável: **uma equipe não planeja
/// gastar o que a levaria para dentro da crise**.
///
/// Ler da mesma fonte que define os estados é deliberado. Uma segunda constante aqui seria
/// uma cópia que envelhece: quem recalibrar as faixas mexeria no significado de "crise" sem
/// mexer no que o planejamento protege, e os dois passariam a discordar em silêncio.
pub fn reserva_de_sobrevivencia_em_meses() -> f64 {
    crate::finance::state::FaixasDeMeses::default().pressionada
}

/// Receita ANUAL projetada da equipe.
///
/// **Este era o erro da seção 2.1 em pessoa.** A fórmula antiga era
/// `expected_cash_midpoint × 0,45 × …`: um ESTOQUE (quanto a equipe deveria ter no banco)
/// usado para projetar um FLUXO (quanto ela vai faturar no ano). São grandezas de dimensões
/// diferentes, e confundi-las é o que fazia a receita de uma categoria escalar com a riqueza
/// esperada dela em vez de escalar com o custo de operar nela.
///
/// A âncora certa de uma receita anual é o CUSTO OPERACIONAL anual: uma equipe mediana
/// projeta cobrir os próprios custos, e é dos fatores de reputação e de carro que sai quem
/// projeta acima e quem projeta abaixo. Daí o coeficiente 1,0 — não é um número calibrado,
/// é a afirmação de que o ponto neutro da projeção é o empate.
///
/// O NÍVEL da receita de verdade não mora aqui (isto é planejamento, não dinheiro): mora nos
/// canais de `commands::race::financas`, e é da sessão de receita.
///
/// # O paraquedas entra AQUI, e só aqui
///
/// `parachute_payment_remaining` é o saldo que a equipe rebaixada ainda tem a receber
/// (`promotion::effects`), e ele vira dinheiro de verdade em parcelas por rodada — o
/// `aid_income` de `commands::race::financas`, que `finance::cashflow` abate do saldo. Somá-lo
/// à receita projetada do ano é a leitura certa da grandeza: um FLUXO que a equipe espera
/// receber ao longo da temporada, na mesma unidade dos outros termos daqui.
///
/// É por isso que ele **não** aparece de novo em [`calculate_spending_power`]: o poder de
/// gasto já lê esta função inteira dentro do resultado projetado. Ele aparecia nos dois, e o
/// paraquedas de uma equipe rebaixada entrava 1,x vez no que ela achava que podia gastar.
pub fn calculate_projected_income(team: &Team) -> f64 {
    let scale = category_finance_scale_for(&team.categoria, team.classe.as_deref());
    let reputation_factor = 0.70 + team.reputacao.clamp(0.0, 100.0) / 250.0;
    // Mesma amplitude de antes (0,85–1,05), agora sobre a escala única 0–100: o divisor 500
    // reproduz exatamente o `/105` do domínio −5..16 que estava aqui.
    let performance_factor = 0.85 + team.car_strength() / 500.0;

    scale.operating_cost_midpoint() * reputation_factor * performance_factor
        + team.parachute_payment_remaining.max(0.0)
}

pub fn calculate_committed_costs(team: &Team) -> f64 {
    let scale = category_finance_scale_for(&team.categoria, team.classe.as_deref());
    let structure_factor = 0.70
        + team.facilities.clamp(0.0, 100.0) / 350.0
        + team.engineering.clamp(0.0, 100.0) / 450.0
        + team.pit_crew_quality.clamp(0.0, 100.0) / 550.0;

    scale.operating_cost_midpoint() * structure_factor
}

pub fn calculate_available_credit(team: &Team) -> f64 {
    let scale = category_finance_scale_for(&team.categoria, team.classe.as_deref());
    let reputation_credit = 0.45 + team.reputacao.clamp(0.0, 100.0) / 160.0;
    let gross_credit = scale.expected_cash_midpoint() * reputation_credit;

    (gross_credit - team.debt_balance.max(0.0)).max(0.0)
}

pub fn calculate_debt_pressure(team: &Team) -> f64 {
    let state_multiplier = match team.financial_state.as_str() {
        "elite" | "healthy" => 0.85,
        "stable" => 1.0,
        "pressured" => 1.2,
        "crisis" => 1.45,
        "collapse" => 1.75,
        _ => 1.0,
    };

    team.debt_balance.max(0.0) * state_multiplier
}

/// A reserva de sobrevivência em dinheiro: [`reserva_de_sobrevivencia_em_meses`] vezes o
/// custo de operar um mês nesta divisão.
///
/// Deixou de depender do estado. O estado continua modulando o planejamento — mas onde ele
/// de fato significa alguma coisa, que é a **confiança** na receita projetada e a
/// **agressividade** no crédito. Quanto custa sobreviver não muda porque a equipe está
/// otimista.
pub fn calculate_safety_reserve(team: &Team) -> f64 {
    crate::finance::state::custo_operacional_mensal(&team.categoria, team.classe.as_deref())
        * reserva_de_sobrevivencia_em_meses()
}

/// **Quanto esta equipe pode gastar nesta temporada sem se matar.**
///
/// # A re-derivação na unidade nova
///
/// A fórmula anterior somava um ESTOQUE (caixa) com FLUXOS anuais e subtraía compromissos,
/// dívida e uma reserva que valia até 1,5× o custo operacional **anual**. Enquanto o caixa
/// esperado da categoria valia ~24 meses de operação, o termo de estoque pagava a reserva
/// sozinho e sobrava: a equipe mediana tinha `spending_power` de +0,79 anuais. Quando o
/// caixa virou consequência do modelo físico — 1 a 11 meses —, o estoque encolheu ~4× e
/// nenhum coeficiente de fluxo mudou junto. Medido: **−0,76 anuais para a equipe mediana, e
/// 15,2 meses de caixa necessários só para o resultado virar positivo**, contra uma faixa
/// declarada de 1 a 11. Nenhuma equipe dentro da própria faixa do modelo podia gastar nada,
/// e o cérebro de manutenção parou de comprar peça.
///
/// A conta agora tem três parcelas, e cada uma responde a uma pergunta diferente:
///
/// | parcela | pergunta |
/// |---|---|
/// | folga de caixa | quanto já tenho acima do piso de sobrevivência? |
/// | resultado projetado | quanto a temporada deve me deixar (ou tirar)? |
/// | crédito usável | quanto eu tomaria emprestado neste estado? |
///
/// Três mudanças estruturais, não de constante:
///
/// - **A folga sai de [`crate::finance::state::meses_de_operacao`]**, a mesma medida que
///   define o estado do time. Ela já abate a dívida do caixa, então `debt_pressure` deixou
///   de entrar como termo separado: subtrair os dois era cobrar a dívida duas vezes.
/// - **A reserva virou meses**, e é o piso de `pressionada`. Deixou de ser um múltiplo do
///   custo ANUAL, que é a dimensão errada para um caixa medido em meses.
/// - **O custo comprometido do ano só aparece dentro do resultado projetado.** A folga já é
///   medida em meses DESSE custo; subtraí-lo outra vez, cheio, era pedir à equipe que
///   pré-pagasse a temporada duas vezes.
///
/// # A confiança é assimétrica, e isso é a decisão
///
/// O estado desconta o que a equipe ESPERA receber, nunca o que ela espera perder. Um
/// resultado projetado positivo entra pela confiança do estado; um negativo entra inteiro.
/// Antes o desconto caía sobre a receita bruta enquanto o custo comprometido era cobrado a
/// 100% — o que fazia a equipe mediana planejar um buraco anual de meio custo operacional
/// todo ano, para sempre. A regra nova diz a coisa prudente sem dizer a coisa falsa: *não
/// conto com o que talvez não venha, e conto inteiro com o que talvez me falte.*
///
/// # O paraquedas não é uma quarta parcela
///
/// A tabela acima tem três linhas, e a soma tem que ter três termos. `parachute_payment_remaining`
/// era somado aqui **por fora**, depois de já ter entrado inteiro em
/// [`calculate_projected_income`] e portanto no resultado projetado: cada real do paraquedas
/// contava uma vez pela confiança do estado e outra vez cheio. Uma equipe rebaixada em estado
/// `stable` lia 1,6 paraquedas de poder de gasto; em `elite`, 1,9. O termo saiu — o paraquedas
/// continua inteiro no planejamento, pela porta da receita, que é onde a grandeza dele fecha.
pub fn calculate_spending_power(team: &Team) -> f64 {
    let mensal =
        crate::finance::state::custo_operacional_mensal(&team.categoria, team.classe.as_deref());
    let meses = crate::finance::state::meses_de_operacao(team);

    let folga_de_caixa = (meses - reserva_de_sobrevivencia_em_meses()) * mensal;

    let resultado_projetado = calculate_projected_income(team) - calculate_committed_costs(team);
    let resultado_ponderado = if resultado_projetado > 0.0 {
        resultado_projetado * income_confidence_for_state(&team.financial_state)
    } else {
        resultado_projetado
    };

    let credito_usavel =
        calculate_available_credit(team) * credit_aggressiveness_for_state(&team.financial_state);

    folga_de_caixa + resultado_ponderado + credito_usavel
}

/// **Quantos meses de operação a equipe terá quando a temporada acabar.**
///
/// O estoque de hoje ([`crate::finance::state::meses_de_operacao`], que já abate a dívida)
/// mais o resultado do ano inteiro convertido para a mesma unidade. Os dois lados em meses,
/// que é a condição que a seção 3.3.4 do redesign estabelece para uma razão significar
/// alguma coisa.
///
/// **O resultado entra CRU, sem a confiança do estado.** A ponderação assimétrica de
/// [`calculate_spending_power`] é uma postura — quanto a equipe se permite contar com o que
/// espera receber — e postura é o que ela decide, não o que ela é. Deixá-la fora é o que
/// mantém esta medida independente de `financial_state`, e desfaz metade da circularidade
/// da seção 2.4: hoje o estado sai dos meses, o índice sairia do estado e a receita sairia
/// do índice.
pub fn meses_projetados(team: &Team) -> f64 {
    let mensal =
        crate::finance::state::custo_operacional_mensal(&team.categoria, team.classe.as_deref());
    if mensal <= 0.0 {
        return 0.0;
    }

    let resultado = calculate_projected_income(team) - calculate_committed_costs(team);
    crate::finance::state::meses_de_operacao(team) + resultado / mensal
}

/// **Quão bem financiada esta equipe está, de 0 a 100.**
///
/// # O triplo-contado que saiu daqui
///
/// A fórmula era `caixa + spending_power×0,45 + receita×0,25 − dívida×0,35`, normalizada
/// pela janela `cash_max − cash_min`. Depois que [`calculate_spending_power`] foi
/// re-derivado, os três termos extras passaram a estar **dentro** dele: a folga de caixa é
/// o caixa, o resultado projetado é a receita, e a folga já abate a dívida. Somá-los por
/// fora contava cada um duas ou três vezes.
///
/// E a janela era da armadilha da 3.3.4 pela sexta vez: um `effective_money` que mistura
/// estoque com fluxo, dividido por uma janela de **caixa puro**. Medido, o índice saturava
/// em 100 a partir de ~9 meses de fôlego enquanto o mundo em regime rodava a 10,9–22,1 —
/// boa parte do grid lia 100, e um índice de 0 a 100 que não distingue ninguém é um campo
/// morto que três sistemas ainda leem.
///
/// # O que ele responde, e por que não é nenhum dos vizinhos
///
/// | função | pergunta | unidade |
/// |---|---|---|
/// | `financial_state` | está doente? | banda discreta sobre o ESTOQUE |
/// | `spending_power` | quanto pode gastar? | dinheiro, absoluto |
/// | `budget_index` | quão bem financiada, contra as outras da categoria? | 0–100, adimensional |
///
/// A conta é [`meses_projetados`] lido pela escada de estados
/// ([`crate::finance::state::posicao_na_escada`]). Duas escolhas fazem dele uma medida
/// própria e não um verniz sobre `spending_power`:
///
/// - **Não tem crédito.** Poder tomar emprestado é poder gastar, não é ser bem financiada;
///   uma equipe em crise toma crédito com MAIS agressividade (0,75 contra 0,10 da elite), e
///   um índice que somasse isso premiaria a doença.
/// - **Não tem a ponderação do estado**, pelo motivo da doc de [`meses_projetados`].
///
/// O que resta é comparável entre categorias por construção: os meses são medidos no custo
/// operacional da divisão da própria equipe, e a escada é a mesma para todas. É isso que
/// torna a pergunta "contra as outras da categoria" respondível sem ter as outras em mãos.
pub fn derive_budget_index_from_money(team: &Team) -> f64 {
    crate::finance::state::posicao_na_escada(
        meses_projetados(team),
        crate::finance::state::FaixasDeMeses::default(),
    )
}

pub fn calculate_financial_plan(team: &Team) -> TeamFinancialPlan {
    TeamFinancialPlan {
        projected_income: calculate_projected_income(team),
        committed_costs: calculate_committed_costs(team),
        safety_reserve: calculate_safety_reserve(team),
        available_credit: calculate_available_credit(team),
        debt_pressure: calculate_debt_pressure(team),
        spending_power: calculate_spending_power(team),
        budget_index: derive_budget_index_from_money(team),
    }
}

pub fn sync_legacy_budget_index(team: &mut Team) {
    team.budget = derive_budget_index_from_money(team);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::team::{placeholder_team_from_db, Team};

    fn sample_team(category: &str, cash: f64, debt: f64, state: &str) -> Team {
        let mut team = placeholder_team_from_db(
            "T001".to_string(),
            "Equipe Planejada".to_string(),
            category.to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = cash;
        team.debt_balance = debt;
        team.financial_state = state.to_string();
        team.reputacao = 55.0;
        team.engineering = 55.0;
        team.facilities = 55.0;
        team
    }

    #[test]
    fn category_scale_makes_gt3_more_expensive_than_rookie() {
        let rookie = category_finance_scale("mazda_rookie");
        let gt3 = category_finance_scale("gt3");

        assert!(gt3.expected_cash_midpoint() > rookie.expected_cash_midpoint());
        assert!(gt3.operating_cost_midpoint() > rookie.operating_cost_midpoint());
    }

    #[test]
    fn unknown_category_gets_safe_mid_tier_scale() {
        let scale = category_finance_scale("unknown");

        assert!(scale.cash_min > 0.0);
        assert!(scale.operating_cost_min > 0.0);
    }

    #[test]
    fn spending_power_penalizes_debt_and_committed_costs() {
        let rich = sample_team("gt3", 12_000_000.0, 0.0, "healthy");
        let indebted = sample_team("gt3", 12_000_000.0, 8_000_000.0, "crisis");

        let rich_plan = calculate_financial_plan(&rich);
        let indebted_plan = calculate_financial_plan(&indebted);

        assert!(rich_plan.spending_power > indebted_plan.spending_power);
        assert!(indebted_plan.debt_pressure > rich_plan.debt_pressure);
    }

    #[test]
    fn budget_index_is_derived_from_money_not_raw_budget_field() {
        let mut team = sample_team("gt4", 6_000_000.0, 0.0, "healthy");
        team.budget = 1.0;

        let plan = calculate_financial_plan(&team);

        assert!(plan.budget_index > 50.0);
    }

    #[test]
    fn sync_legacy_budget_index_overwrites_raw_budget_from_money() {
        let mut team = sample_team("gt4", 6_000_000.0, 0.0, "healthy");
        team.budget = 1.0;

        sync_legacy_budget_index(&mut team);

        assert!((team.budget - derive_budget_index_from_money(&team)).abs() < 0.0001);
        assert!(team.budget > 1.0);
    }

    #[test]
    fn spending_power_can_be_negative_for_collapsed_team() {
        let team = sample_team("gt4", -100_000.0, 7_000_000.0, "collapse");

        let plan = calculate_financial_plan(&team);

        assert!(plan.spending_power < 0.0);
    }

    /// **O guarda que faltava.** Toda asserção sobre `spending_power` era direcional — rico
    /// maior que endividado, colapso abaixo de zero — e todas continuaram passando enquanto a
    /// equipe MEDIANA da escada inteira tinha poder de gasto de −0,76 custos operacionais.
    /// Comparações sobrevivem a um nível errado; foi por isso que o defeito atravessou duas
    /// rodadas sem um teste vermelho.
    ///
    /// A afirmação aqui é de NÍVEL, e é a mínima que o modelo precisa sustentar: uma equipe
    /// no caixa de referência da própria divisão — o ponto em que a economia foi desenhada —
    /// tem que poder gastar alguma coisa. Se este teste ficar vermelho, o cérebro de
    /// manutenção parou de comprar peça e o mercado parou de separar rico de pobre.
    #[test]
    fn a_equipe_no_caixa_de_referencia_pode_gastar() {
        for (categoria, classe) in crate::economia::ancora::DIVISOES {
            let mut team = sample_team(categoria, 0.0, 0.0, "stable");
            team.classe = classe.map(str::to_string);
            team.cash_balance = crate::economia::temporada::caixa_de_referencia(categoria, classe);

            let poder = calculate_spending_power(&team);
            assert!(
                poder > 0.0,
                "{categoria}{}: a equipe mediana precisa poder gastar, e o poder deu {poder:.0}",
                classe.map(|c| format!(":{c}")).unwrap_or_default()
            );
        }
    }

    /// O piso de sobrevivência é o que separa quem pode gastar de quem não pode, e ele está
    /// declarado em MESES — a mesma unidade em que `finance::state` define os estados. Abaixo
    /// dele o poder de gasto é negativo; acima, positivo.
    #[test]
    fn o_piso_de_sobrevivencia_e_a_fronteira_do_poder_de_gasto() {
        let piso = reserva_de_sobrevivencia_em_meses();
        let mut team = sample_team("gt3", 0.0, 0.0, "stable");

        team.cash_balance = crate::economia::temporada::caixa_para_meses("gt3", None, piso - 2.0);
        assert!(calculate_spending_power(&team) < 0.0, "abaixo do piso");

        team.cash_balance = crate::economia::temporada::caixa_para_meses("gt3", None, piso + 3.0);
        assert!(calculate_spending_power(&team) > 0.0, "acima do piso");
    }

    /// **A prova de que o paraquedas entra uma vez.**
    ///
    /// Ele era somado em [`calculate_projected_income`] e OUTRA vez, cheio, no fim de
    /// [`calculate_spending_power`]. Como o poder de gasto lê a receita projetada inteira
    /// dentro do resultado, cada real do paraquedas contava `1 + confiança_do_estado` vezes —
    /// 1,6 em `stable`, 1,9 em `elite`. Nenhuma asserção direcional via isso: mais paraquedas
    /// dava mais poder de gasto tanto na conta certa quanto na errada.
    ///
    /// As duas afirmações aqui prendem o defeito pelos dois lados. A primeira é a
    /// decomposição documentada — três parcelas, três termos, nada por fora. A segunda é o
    /// TETO do efeito: o paraquedas não pode acrescentar ao poder de gasto mais do que ele
    /// mesmo vale.
    #[test]
    fn o_paraquedas_entra_uma_vez_no_poder_de_gasto() {
        const PARAQUEDAS: f64 = 900_000.0;

        for estado in [
            "elite",
            "healthy",
            "stable",
            "pressured",
            "crisis",
            "collapse",
        ] {
            let mut sem = sample_team("gt3", 0.0, 0.0, estado);
            sem.cash_balance = crate::economia::temporada::caixa_de_referencia("gt3", None);
            let mut com = sem.clone();
            com.parachute_payment_remaining = PARAQUEDAS;

            // A decomposição da doc, recomputada aqui: folga + resultado ponderado + crédito.
            let mensal = crate::finance::state::custo_operacional_mensal("gt3", None);
            let folga = (crate::finance::state::meses_de_operacao(&com)
                - reserva_de_sobrevivencia_em_meses())
                * mensal;
            let resultado = calculate_projected_income(&com) - calculate_committed_costs(&com);
            let ponderado = if resultado > 0.0 {
                resultado * income_confidence_for_state(estado)
            } else {
                resultado
            };
            let credito =
                calculate_available_credit(&com) * credit_aggressiveness_for_state(estado);

            let poder = calculate_spending_power(&com);
            assert!(
                (poder - (folga + ponderado + credito)).abs() < 1.0,
                "{estado}: o poder de gasto tem um termo fora das três parcelas \
                 documentadas — leu {poder:.0} contra {:.0}",
                folga + ponderado + credito
            );

            // O paraquedas entra INTEIRO na receita projetada, e é essa a única porta.
            let delta_receita = calculate_projected_income(&com) - calculate_projected_income(&sem);
            assert!(
                (delta_receita - PARAQUEDAS).abs() < 1.0,
                "{estado}: o paraquedas deveria entrar inteiro na receita projetada e \
                 entrou {delta_receita:.0}"
            );

            let delta_poder = poder - calculate_spending_power(&sem);
            assert!(
                delta_poder <= PARAQUEDAS + 1.0,
                "{estado}: {PARAQUEDAS:.0} de paraquedas acrescentaram {delta_poder:.0} ao \
                 poder de gasto — mais do que o paraquedas vale, ou seja, ele foi contado \
                 mais de uma vez"
            );
            assert!(
                delta_poder > 0.0,
                "{estado}: o paraquedas precisa continuar valendo alguma coisa"
            );
        }
    }

    #[test]
    fn same_cash_is_weaker_in_higher_category() {
        let rookie = sample_team("mazda_rookie", 700_000.0, 0.0, "healthy");
        let gt3 = sample_team("gt3", 700_000.0, 0.0, "healthy");

        let rookie_plan = calculate_financial_plan(&rookie);
        let gt3_plan = calculate_financial_plan(&gt3);

        assert!(rookie_plan.budget_index > gt3_plan.budget_index);
    }

    /// **O guarda de NÍVEL do índice, e o motivo dele.**
    ///
    /// Um índice de 0 a 100 existe para SEPARAR. A fórmula anterior era direcionalmente
    /// impecável — mais caixa dava mais índice, categoria mais cara dava menos — e mesmo
    /// assim saturava em 100 a partir de ~9 meses de fôlego, num mundo que roda a 10,9–22,1.
    /// Vinte e três dos vinte e sete pontos do regime medido liam exatamente 100, e nenhuma
    /// asserção de ordem podia ver isso, porque `100 > 100` nunca foi perguntado.
    ///
    /// Este teste varre a faixa em que o mundo de fato vive e exige DISPERSÃO: nada colado
    /// nas pontas, e o alcance ocupando pelo menos metade da escala. É a asserção que teria
    /// ficado vermelha na rodada em que a âncora de caixa encolheu 4×.
    #[test]
    fn o_indice_espalha_o_grid_em_regime_em_vez_de_empilhar_nas_pontas() {
        // Meses de operação medidos na seção 4.7 do redesign: campeão, meio e lanterna de
        // nove categorias, 20 temporadas. É a população que o índice encontra na prática, e
        // ela vive MUITO acima da faixa de nascimento declarada (1 a 11 meses).
        const REGIME: [(&str, [f64; 3]); 4] = [
            ("mazda_rookie", [19.8, 22.1, 20.2]),
            ("bmw_m2", [61.2, 16.6, 14.3]),
            ("gt4", [78.4, 12.6, 6.3]),
            ("gt3", [46.4, 16.9, 9.6]),
        ];

        let mut indices = Vec::new();
        for (categoria, triplo) in REGIME {
            for meses in triplo {
                let mut team = sample_team(categoria, 0.0, 0.0, "stable");
                team.cash_balance =
                    crate::economia::temporada::caixa_para_meses(categoria, None, meses);
                indices.push((categoria, meses, derive_budget_index_from_money(&team)));
            }
        }

        for (categoria, meses, indice) in &indices {
            assert!(
                *indice > 1.0 && *indice < 99.0,
                "{categoria} com {meses:.1} meses leu {indice:.1} — o índice encostou na \
                 ponta da escala, que é onde ele para de distinguir equipe de equipe"
            );
        }

        let minimo = indices.iter().map(|(_, _, i)| *i).fold(f64::MAX, f64::min);
        let maximo = indices.iter().map(|(_, _, i)| *i).fold(f64::MIN, f64::max);
        assert!(
            maximo - minimo > 50.0,
            "o grid em regime ocupou só {:.1} pontos da escala ({minimo:.1} a {maximo:.1}); \
             um índice que comprime o mundo num punhado de pontos não separa ninguém",
            maximo - minimo
        );
    }

    /// O índice tem que ler a escada onde ela está declarada, não onde deu certo.
    ///
    /// As três fronteiras nomeadas de [`crate::finance::state::FaixasDeMeses`] caem em
    /// pontos exatos e legíveis da escala — 50,0 é o piso de `estavel`, 66,7 o de
    /// `saudavel`, 83,3 o de `elite`. A equipe testada aqui está no ponto neutro de fluxo,
    /// então `meses_projetados` é o caixa dela; a asserção prende o MAPA, não a equipe.
    #[test]
    fn as_fronteiras_da_escada_caem_em_pontos_declarados_da_escala() {
        use crate::finance::state::{posicao_na_escada, FaixasDeMeses};

        let faixas = FaixasDeMeses::default();
        for (meses, esperado) in [
            (faixas.crise, 100.0 / 6.0),
            (faixas.pressionada, 200.0 / 6.0),
            (faixas.estavel, 300.0 / 6.0),
            (faixas.saudavel, 400.0 / 6.0),
            (faixas.elite, 500.0 / 6.0),
        ] {
            let lido = posicao_na_escada(meses, faixas);
            assert!(
                (lido - esperado).abs() < 0.001,
                "{meses:.0} meses deveria ler {esperado:.1} e leu {lido:.1}"
            );
        }
    }

    /// **A prova de que o índice não é `spending_power` maquiado.**
    ///
    /// Se ele fosse uma transformação monotônica do poder de gasto, ele não teria por que
    /// existir — bastaria derivá-lo explicitamente e apagar a função. A separação é real e
    /// tem um caso que a demonstra: `spending_power` inclui o CRÉDITO, e a agressividade de
    /// crédito é maior no estado pior (0,75 em crise contra 0,10 na elite), porque quem está
    /// afogado toma emprestado. Uma equipe em crise pode portanto ter MAIS poder de gasto que
    /// uma estável com o mesmo caixa — e é exatamente por isso que ela não pode ter mais
    /// índice: gastar dinheiro emprestado não é estar bem financiada.
    #[test]
    fn o_indice_nao_e_uma_transformacao_monotona_do_poder_de_gasto() {
        let mut em_crise = sample_team("gt3", 0.0, 0.0, "crisis");
        em_crise.cash_balance = crate::economia::temporada::caixa_para_meses("gt3", None, 5.0);
        let mut estavel = sample_team("gt3", 0.0, 0.0, "stable");
        estavel.cash_balance = crate::economia::temporada::caixa_para_meses("gt3", None, 5.0);

        assert!(
            calculate_spending_power(&em_crise) > calculate_spending_power(&estavel),
            "o caso depende de a crise tomar mais crédito; se isso mudou, o teste precisa \
             de outro caso, não de ser apagado"
        );
        assert!(
            (derive_budget_index_from_money(&em_crise) - derive_budget_index_from_money(&estavel))
                .abs()
                < 0.001,
            "o índice seguiu o poder de gasto: as duas equipes têm o mesmo dinheiro e o \
             mesmo custo, e só diferem na postura de crédito"
        );
    }
}
