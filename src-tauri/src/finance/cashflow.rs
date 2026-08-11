//! O fluxo de caixa de uma rodada: receita, despesa, saldo e amortização de dívida.
//!
//! O que **não** está mais aqui: o offseason de competitividade, que antes ocupava um terço
//! do arquivo. Ele foi substituído em produção por `economia::desenvolvimento` e mora agora
//! no submódulo `offseason`, que só existe sob `cfg(test)` — ver o cabeçalho de lá.

use crate::finance::planning::category_finance_scale;
use crate::models::team::Team;

/// O offseason aposentado. **Só existe em teste**, de propósito: é o braço de controle do
/// harness A/B de economia, e um caller de produção novo passa a não compilar em vez de
/// ressuscitar em silêncio a estrutura de graça.
#[cfg(test)]
pub mod offseason;

// A fachada pelo caminho que o harness A/B e os testes de `finance::strategy` já usavam.
// `OffseasonCompetitivenessImpact` e a forma `calculate_*` entram junto porque são o tipo de
// retorno e a variante pura do mesmo mecanismo: quem for reabrir o braço de controle precisa
// dos três pelo mesmo caminho.
#[cfg(test)]
#[allow(unused_imports)]
pub use offseason::{
    apply_offseason_competitiveness_impact, calculate_offseason_competitiveness_impact,
    OffseasonCompetitivenessImpact,
};

/// Fração do caixa excedente (acima da reserva de segurança) destinada a abater
/// o principal da dívida a cada corrida.
const DEBT_AMORTIZATION_RATE: f64 = 0.25;
/// Reserva de segurança mantida antes de amortizar, como fração do custo
/// operacional médio da categoria (equipes maiores guardam mais caixa).
const DEBT_AMORTIZATION_RESERVE_FACTOR: f64 = 0.05;

/// Caixa mínimo que a equipe mantém antes de usar a folga para pagar dívida.
fn debt_amortization_reserve(category: &str) -> f64 {
    category_finance_scale(category).operating_cost_midpoint() * DEBT_AMORTIZATION_RESERVE_FACTOR
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundCashflowSummary {
    pub income: f64,
    pub expenses: f64,
    pub net: f64,
}

/// As OITO linhas de despesa que ficam gravadas no ledger.
///
/// É a mesma forma que a fatura visível (`economia::fatura`) trava por decisão de design:
/// sete linhas físicas da etapa mais o rateio da estrutura do ano. Elas não são um resumo
/// do que saiu do caixa — elas **são** o que saiu do caixa, e é por isso que moram dentro
/// de [`TeamRoundFinanceContext`] em vez de serem recalculadas na hora de gravar. Um
/// segundo cálculo é um segundo lugar onde a conta pode divergir, e o defeito que este
/// redesign existe para corrigir era exatamente esse.
///
/// `viagem` e `estadia` do modelo interno entram somadas porque são a mesma decisão —
/// mandar N pessoas para longe por M noites — e a tela as mostra como uma linha só.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LinhasDaDespesa {
    pub combustivel: f64,
    pub pneus: f64,
    /// Revisão mecânica amortizada por quilômetro. **Não** é a compra de peça de
    /// reposição, que é decisão do cérebro de manutenção e vive em
    /// `technical_investment_cost`.
    pub desgaste_de_peca: f64,
    pub frete: f64,
    pub viagem_e_estadia: f64,
    pub inscricao: f64,
    pub diarias: f64,
    /// A fatia desta rodada nos recorrentes do ano — folha técnica, sede, frota, seguro e
    /// os contratos categóricos. **Sem** a folha de pilotos: ela sai do caixa como
    /// `salary_expense`, e somá-la aqui pagaria piloto duas vezes.
    pub estrutura: f64,
}

impl LinhasDaDespesa {
    /// As sete linhas da ETAPA. É o `event_operations_cost`.
    pub fn total_da_etapa(&self) -> f64 {
        self.combustivel
            + self.pneus
            + self.desgaste_de_peca
            + self.frete
            + self.viagem_e_estadia
            + self.inscricao
            + self.diarias
    }

    /// As oito linhas.
    pub fn total(&self) -> f64 {
        self.total_da_etapa() + self.estrutura
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TeamRoundFinanceContext {
    pub sponsorship_income: f64,
    /// Bilheteria/portão da rodada (Fase 3 do Estrelato): público que a fama do
    /// lineup atrai, escalado pelo prestígio do evento. Canal distinto do patrocínio
    /// (portão volátil e por-evento vs. patrocínio suave e de-temporada).
    pub gate_income: f64,
    pub result_bonus: f64,
    pub partial_prize_income: f64,
    pub aid_income: f64,
    pub salary_expense: f64,
    pub event_operations_cost: f64,
    pub structural_maintenance_cost: f64,
    pub technical_investment_cost: f64,
    pub debt_service_cost: f64,
    /// A decomposição de `event_operations_cost` + `structural_maintenance_cost` em linhas
    /// nomeadas. Os dois agregados acima continuam existindo porque o resto do jogo os lê,
    /// mas são a SOMA destas linhas, escrita de uma origem só — ver
    /// `commands::race::despesa`.
    pub linhas: LinhasDaDespesa,
}

/// Fração do custo operacional médio da rodada que vira o BOLO de bilheteria de um
/// evento de prestígio médio. Botão único da magnitude da 2ª receita de fama —
/// calibrado por Monte Carlo. Alvo: bilheteria média por rodada MENOR que o
/// patrocínio (fama já ganha por 2 canais; não pode dominar a economia).
/// Sobrescrevível por `IRACER_GATE_SHARE` (lido uma vez) para varreduras de MC.
const DEFAULT_GATE_POT_COEFF: f64 = 0.12;

/// Peso do PISO de público (a parte da multidão que vem pela corrida, dividida em
/// partes iguais entre os times) vs. o PRÊMIO de estrela (dividido por cota de fama).
/// 0.5 = metade piso, metade estrela. Garante que um time sem astro não zere a
/// bilheteria, e que o estrelato seja um prêmio SOBRE um piso, não tudo-ou-nada.
pub(crate) const GATE_FLOOR_WEIGHT: f64 = 0.5;

pub(crate) fn gate_pot_coeff() -> f64 {
    static COEFF: std::sync::LazyLock<f64> = std::sync::LazyLock::new(|| {
        std::env::var("IRACER_GATE_SHARE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| *v > 0.0 && *v < 1.0)
            .unwrap_or(DEFAULT_GATE_POT_COEFF)
    });
    *COEFF
}

/// Bilheteria da rodada para UM time (Fase 3 do Estrelato). Motor puro e testável.
///
/// - `event_prestige_score`: score do `event_interest` do evento (pré-vendido / esperado).
///   Um evento maior (endurance, final, decisão de título) lota mais → bolo maior.
/// - `round_operating_base`: custo operacional médio da rodada (escala da categoria).
/// - `team_presence`: presença pública do lineup DESTE time (fama dos pilotos).
/// - `grid_total_presence`: soma da presença de TODOS os lineups do grid da categoria.
/// - `n_teams`: nº de times no grid (para o piso dividido em partes iguais).
/// - `income_modifier`: modulador macroeconômico (mesmo das outras receitas).
///
/// `fame_share` = cota competitiva do time no público (presença dele / grid). Se o grid
/// inteiro é anônimo (`grid_total_presence == 0`), cai para cota igual (1/N) — o piso
/// sozinho. Um astro num time pobre puxa portão mesmo sem vencer; o ídolo vale mais se
/// os rivais forem anônimos.
/// Cota de um time no PÚBLICO/bilheteria do evento: piso (dividido igual entre os N
/// times, a multidão que vem pela corrida) + prêmio de estrela (por fama do lineup).
/// Fração ∈ [0,1] do bolo do portão que o time captura. Reusada pela bilheteria e pela
/// prévia da Sala de Estratégia ("sua estrela puxa Y% do público"). Se o grid é anônimo
/// (`grid_total_presence == 0`), cai para cota igual (só o piso).
pub fn team_gate_share(team_presence: f64, grid_total_presence: f64, n_teams: f64) -> f64 {
    team_gate_share_with(
        team_presence,
        grid_total_presence,
        n_teams,
        GATE_FLOOR_WEIGHT,
    )
}

/// Igual a [`team_gate_share`], com o peso do piso explícito. Existe para o harness de
/// calibração varrer o quanto do portão é piso igualitário e o quanto é prêmio de estrela.
pub fn team_gate_share_with(
    team_presence: f64,
    grid_total_presence: f64,
    n_teams: f64,
    floor_weight: f64,
) -> f64 {
    let n = n_teams.max(1.0);
    let piso = floor_weight.clamp(0.0, 1.0);
    let fame_share = if grid_total_presence > 0.0 {
        (team_presence / grid_total_presence).clamp(0.0, 1.0)
    } else {
        1.0 / n
    };
    (piso / n + (1.0 - piso) * fame_share).clamp(0.0, 1.0)
}

pub fn calculate_gate_income(
    event_prestige_score: f64,
    round_operating_base: f64,
    team_presence: f64,
    grid_total_presence: f64,
    n_teams: f64,
    income_modifier: f64,
) -> f64 {
    calculate_gate_income_with(
        event_prestige_score,
        round_operating_base,
        team_presence,
        grid_total_presence,
        n_teams,
        income_modifier,
        gate_pot_coeff(),
        GATE_FLOOR_WEIGHT,
    )
}

/// Igual a [`calculate_gate_income`], com o coeficiente do bolo e o peso do piso explícitos.
///
/// Existe porque `gate_pot_coeff()` é um `LazyLock` — lê a env UMA vez por processo, o que
/// serve para uma varredura de fora mas não para o harness comparar dez valores no mesmo
/// processo. Produção segue na constante.
#[allow(clippy::too_many_arguments)]
pub fn calculate_gate_income_with(
    event_prestige_score: f64,
    round_operating_base: f64,
    team_presence: f64,
    grid_total_presence: f64,
    n_teams: f64,
    income_modifier: f64,
    pot_coeff: f64,
    floor_weight: f64,
) -> f64 {
    // 60 ≈ evento GT3 médio → fator 1.0; clamp evita bolo negativo/absurdo.
    let prestige_factor = (event_prestige_score / 60.0).clamp(0.3, 2.2);
    let gate_pot = round_operating_base * pot_coeff * prestige_factor * income_modifier;

    (gate_pot * team_gate_share_with(team_presence, grid_total_presence, n_teams, floor_weight))
        .max(0.0)
}

pub fn calculate_round_income(
    sponsorship_income: f64,
    gate_income: f64,
    result_bonus: f64,
    partial_prize_income: f64,
    aid_income: f64,
) -> f64 {
    sponsorship_income.max(0.0)
        + gate_income.max(0.0)
        + result_bonus.max(0.0)
        + partial_prize_income.max(0.0)
        + aid_income.max(0.0)
}

pub fn calculate_round_expenses(
    salary_expense: f64,
    event_operations_cost: f64,
    structural_maintenance_cost: f64,
    technical_investment_cost: f64,
    debt_service_cost: f64,
) -> f64 {
    salary_expense.max(0.0)
        + event_operations_cost.max(0.0)
        + structural_maintenance_cost.max(0.0)
        + technical_investment_cost.max(0.0)
        + debt_service_cost.max(0.0)
}

pub fn summarize_round_cashflow(income: f64, expenses: f64) -> RoundCashflowSummary {
    RoundCashflowSummary {
        income,
        expenses,
        net: income - expenses,
    }
}

pub fn apply_round_cashflow(
    team: &mut Team,
    context: TeamRoundFinanceContext,
) -> RoundCashflowSummary {
    let income = calculate_round_income(
        context.sponsorship_income,
        context.gate_income,
        context.result_bonus,
        context.partial_prize_income,
        context.aid_income,
    );
    let expenses = calculate_round_expenses(
        context.salary_expense,
        context.event_operations_cost,
        context.structural_maintenance_cost,
        context.technical_investment_cost,
        context.debt_service_cost,
    );
    let summary = summarize_round_cashflow(income, expenses);

    team.last_round_income = summary.income;
    team.last_round_expenses = summary.expenses;
    team.last_round_net = summary.net;
    team.cash_balance += summary.net;

    if team.cash_balance < -100_000.0 {
        let financed_amount = -100_000.0 - team.cash_balance;
        team.debt_balance += financed_amount;
        team.cash_balance = -100_000.0;
    }

    // Amortização: com dívida pendente e caixa acima da reserva de segurança,
    // parte do excedente abate o principal. É o espelho do financiamento acima
    // e o único caminho de recuperação para equipes endividadas — sem isso, a
    // dívida só cresce e "collapse" vira estado absorvente.
    if team.debt_balance > 0.0 {
        let reserve = debt_amortization_reserve(&team.categoria);
        let surplus = team.cash_balance - reserve;
        if surplus > 0.0 {
            let payment = (surplus * DEBT_AMORTIZATION_RATE).min(team.debt_balance);
            team.cash_balance -= payment;
            team.debt_balance -= payment;
        }
    }

    if team.parachute_payment_remaining > 0.0 {
        team.parachute_payment_remaining =
            (team.parachute_payment_remaining - context.aid_income.max(0.0)).max(0.0);
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::team::placeholder_team_from_db;

    #[test]
    fn round_income_stays_positive_for_basic_team_revenue() {
        let round_income = calculate_round_income(125_000.0, 12_000.0, 25_000.0, 8_000.0, 0.0);
        assert!(round_income > 0.0);
    }

    #[test]
    fn gate_income_sobe_com_a_fama_do_lineup() {
        // Mesmo evento, mesmo grid: o time com lineup mais famoso leva a maior fatia.
        let famoso = calculate_gate_income(60.0, 500_000.0, 160.0, 300.0, 8.0, 1.0);
        let anonimo = calculate_gate_income(60.0, 500_000.0, 40.0, 300.0, 8.0, 1.0);
        assert!(
            famoso > anonimo,
            "lineup famoso ({famoso}) deveria puxar mais bilheteria que o anônimo ({anonimo})"
        );
    }

    #[test]
    fn gate_income_sobe_com_o_prestigio_do_evento() {
        // Mesmo time/grid: evento de prestígio (endurance/final) lota mais que rodada fraca.
        let grande = calculate_gate_income(95.0, 500_000.0, 100.0, 300.0, 8.0, 1.0);
        let pequeno = calculate_gate_income(25.0, 500_000.0, 100.0, 300.0, 8.0, 1.0);
        assert!(
            grande > pequeno,
            "evento grande ({grande}) > pequeno ({pequeno})"
        );
    }

    #[test]
    fn team_gate_share_soma_piso_mais_premio_de_estrela() {
        // Estrela (presença 200/400) leva mais que anônimo (50/400); ambos ∈ (0,1).
        let estrela = team_gate_share(200.0, 400.0, 8.0);
        let anonimo = team_gate_share(50.0, 400.0, 8.0);
        assert!(estrela > anonimo);
        assert!(estrela < 1.0 && anonimo > 0.0);
        // Grid anônimo → só o piso (cota igual 1/N).
        let piso = team_gate_share(0.0, 0.0, 8.0);
        assert!((piso - 1.0 / 8.0).abs() < 1e-9);
    }

    #[test]
    fn gate_income_tem_piso_mesmo_sem_estrela() {
        // Grid inteiro anônimo (presença total 0): ainda há bilheteria (piso, cota igual).
        let sem_fama = calculate_gate_income(60.0, 500_000.0, 0.0, 0.0, 8.0, 1.0);
        assert!(
            sem_fama > 0.0,
            "piso de público deveria garantir bilheteria > 0"
        );
    }

    #[test]
    fn round_expenses_stay_positive_for_basic_team_costs() {
        let round_expenses =
            calculate_round_expenses(60_000.0, 22_000.0, 15_000.0, 9_500.0, 3_000.0);
        assert!(round_expenses > 0.0);
    }

    #[test]
    fn round_cashflow_summary_tracks_net_value() {
        let summary = summarize_round_cashflow(158_000.0, 121_500.0);

        assert_eq!(summary.net, 36_500.0);
    }

    #[test]
    fn apply_round_cashflow_updates_team_snapshot() {
        let mut team = placeholder_team_from_db(
            "T001".to_string(),
            "Equipe Financeira".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = 500_000.0;

        let summary = apply_round_cashflow(
            &mut team,
            TeamRoundFinanceContext {
                linhas: Default::default(),
                sponsorship_income: 120_000.0,
                gate_income: 0.0,
                result_bonus: 25_000.0,
                partial_prize_income: 10_000.0,
                aid_income: 0.0,
                salary_expense: 45_000.0,
                event_operations_cost: 20_000.0,
                structural_maintenance_cost: 15_000.0,
                technical_investment_cost: 18_000.0,
                debt_service_cost: 2_500.0,
                ..Default::default()
            },
        );

        assert_eq!(team.last_round_income, summary.income);
        assert_eq!(team.last_round_expenses, summary.expenses);
        assert_eq!(team.last_round_net, summary.net);
        assert_eq!(team.cash_balance, 554_500.0);
    }

    #[test]
    fn amortization_pays_down_debt_from_cash_surplus() {
        let mut team = placeholder_team_from_db(
            "T001".to_string(),
            "Equipe Endividada".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = 4_000_000.0;
        team.debt_balance = 5_000_000.0;

        // Rodada equilibrada (net = 0): a amortização vem só do caixa acumulado.
        apply_round_cashflow(
            &mut team,
            TeamRoundFinanceContext {
                linhas: Default::default(),
                sponsorship_income: 100_000.0,
                gate_income: 0.0,
                result_bonus: 0.0,
                partial_prize_income: 0.0,
                aid_income: 0.0,
                salary_expense: 100_000.0,
                event_operations_cost: 0.0,
                structural_maintenance_cost: 0.0,
                technical_investment_cost: 0.0,
                debt_service_cost: 0.0,
                ..Default::default()
            },
        );

        assert!(
            team.debt_balance < 5_000_000.0,
            "dívida deve cair (era 5M, ficou {})",
            team.debt_balance
        );
        // Caixa + dívida quitada deve ser conservado: o pagamento sai do caixa.
        assert!(team.cash_balance < 4_000_000.0);
        let paid = 5_000_000.0 - team.debt_balance;
        assert!((team.cash_balance - (4_000_000.0 - paid)).abs() < 1.0);
    }

    #[test]
    fn amortization_skipped_when_cash_below_reserve() {
        let mut team = placeholder_team_from_db(
            "T002".to_string(),
            "Equipe Quebrada".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = -50_000.0;
        team.debt_balance = 6_000_000.0;

        apply_round_cashflow(
            &mut team,
            TeamRoundFinanceContext {
                linhas: Default::default(),
                sponsorship_income: 50_000.0,
                gate_income: 0.0,
                result_bonus: 0.0,
                partial_prize_income: 0.0,
                aid_income: 0.0,
                salary_expense: 50_000.0,
                event_operations_cost: 0.0,
                structural_maintenance_cost: 0.0,
                technical_investment_cost: 0.0,
                debt_service_cost: 0.0,
                ..Default::default()
            },
        );

        assert_eq!(
            team.debt_balance, 6_000_000.0,
            "sem caixa acima da reserva, nada de dívida é pago"
        );
    }
}
