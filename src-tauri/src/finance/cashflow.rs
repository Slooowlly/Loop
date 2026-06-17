use crate::finance::events::technical_breakthrough_chance;
use crate::finance::planning::category_finance_scale;
use crate::models::team::Team;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamRoundFinanceContext {
    pub sponsorship_income: f64,
    pub result_bonus: f64,
    pub partial_prize_income: f64,
    pub aid_income: f64,
    pub salary_expense: f64,
    pub event_operations_cost: f64,
    pub structural_maintenance_cost: f64,
    pub technical_investment_cost: f64,
    pub debt_service_cost: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OffseasonCompetitivenessImpact {
    pub reliability_delta: f64,
    pub car_performance_delta: f64,
    pub engineering_delta: f64,
    pub facilities_delta: f64,
}

pub fn calculate_round_income(
    sponsorship_income: f64,
    result_bonus: f64,
    partial_prize_income: f64,
    aid_income: f64,
) -> f64 {
    sponsorship_income.max(0.0)
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

/// Pilar B: controla o quão rápido os retornos do investimento no carro decaem.
/// Quanto maior o `car_performance` atual, menos cada unidade de "drive" rende
/// (subir o carro fica progressivamente mais caro) — sem teto, só mais lento.
const CAR_INVEST_DIMINISH_K: f64 = 14.0;

/// Fração do ganho de carro que as equipes FICTÍCIAS (sem marca de fábrica) obtêm
/// quando disputam contra fábricas reais (GT3 e GT3 do endurance). As reais ganham
/// 100%; as fictícias ganham menos, então temporada a temporada as fábricas reais se
/// distanciam — deixando as fictícias como escolhas de "azarão". Ajustável.
const FICTIONAL_CAR_DEV_FACTOR: f64 = 0.6;

/// Em quantos anos de CARREIRA a penalidade fictícia some por completo. A penalidade
/// é forte no começo (azarão recém-chegado) e desaparece linearmente, deixando o
/// jogador "underdog" capaz de competir de igual para igual ao final do período.
pub(crate) const PENALTY_FADE_YEARS: f64 = 5.0;

/// Quanto cada equipe multiplica seu GANHO de car_performance nesta offseason.
/// É 1.0 para fábricas reais e para qualquer categoria sem disputa real-vs-fictícia;
/// nas arenas GT3 (sprint) e GT3-endurance as fictícias rendem `FICTIONAL_CAR_DEV_FACTOR`
/// no ano 0 da carreira, subindo linearmente até 1.0 no ano `PENALTY_FADE_YEARS`.
/// `career_year` é o ano da carreira do jogador (0 = primeiro ano).
fn car_dev_gain_factor(team: &Team, career_year: i32) -> f64 {
    let competes_with_real_marques = team.categoria == "gt3"
        || (team.categoria == "endurance" && team.classe.as_deref() == Some("gt3"));
    if team.marca.is_none() && competes_with_real_marques {
        let fade = (career_year.max(0) as f64 / PENALTY_FADE_YEARS).min(1.0);
        FICTIONAL_CAR_DEV_FACTOR + (1.0 - FICTIONAL_CAR_DEV_FACTOR) * fade
    } else {
        1.0
    }
}

/// `career_year` é o ano da carreira do jogador (0 = primeiro ano). Modula a
/// penalidade fictícia GT3, que some por completo ao atingir `PENALTY_FADE_YEARS`.
pub fn calculate_offseason_competitiveness_impact(
    team: &Team,
    career_year: i32,
) -> OffseasonCompetitivenessImpact {
    let efficiency = management_efficiency_modifier(team);
    // Teto de caixa elevado (1.2 -> 2.5): equipes realmente ricas investem mais
    // no carro (Pilar B). Os retornos decrescentes abaixo é que limitam o ganho.
    let cash_strength = (team.cash_balance / 1_000_000.0).clamp(-0.5, 2.5);
    // Pilar B: o investimento no CARRO escala com o caixa numa faixa muito maior
    // que o cash_strength geral (teto 2.5). Assim o caixa gigante das categorias
    // de topo (endurance ~12-60M) vira carro de verdade — gating economico em vez
    // de teto rigido. Confiabilidade/estrutura seguem no cash_strength original.
    let car_cash_strength = (team.cash_balance / 1_000_000.0).clamp(-0.5, 12.0);
    let debt_pressure = (team.debt_balance / 900_000.0).clamp(0.0, 1.2);
    let state = financial_state_bias(&team.financial_state);
    let strategy = season_strategy_bias(&team.season_strategy);
    let breakthrough_expected_value = technical_breakthrough_chance(team) * 4.0;

    let reliability_delta =
        (cash_strength * 1.8 - debt_pressure * 3.2 + state.reliability + strategy.reliability)
            * efficiency;
    // "Drive" bruto de investimento no carro nesta offseason.
    let car_drive = (car_cash_strength * 0.55 - debt_pressure * 0.65
        + state.car_performance
        + strategy.car_performance
        + breakthrough_expected_value)
        * efficiency;
    // Retornos decrescentes (Pilar B): o mesmo drive rende menos quanto melhor o
    // carro já é. Quedas (drive < 0, carro defasado por falta de verba) aplicam
    // direto, para que elites defundadas ainda percam terreno. O fator real-vs-
    // fictícia só penaliza o GANHO (não as quedas), para as fábricas se distanciarem.
    let car_performance_delta = if car_drive > 0.0 {
        car_drive * car_dev_gain_factor(team, career_year)
            / (1.0 + team.car_performance.max(0.0) / CAR_INVEST_DIMINISH_K)
    } else {
        car_drive
    };
    let structure_delta =
        (cash_strength * 1.15 - debt_pressure * 2.25 + state.structure + strategy.structure)
            * efficiency;

    OffseasonCompetitivenessImpact {
        reliability_delta: reliability_delta.clamp(-6.0, 4.0),
        // Clamp largo (de ±1.4) só como rede de segurança contra saltos extremos;
        // os retornos decrescentes é que regulam o ganho temporada a temporada.
        car_performance_delta: car_performance_delta.clamp(-3.0, 3.0),
        engineering_delta: structure_delta.clamp(-3.5, 2.5),
        facilities_delta: (structure_delta * 0.75).clamp(-2.5, 1.8),
    }
}

/// `career_year` é o ano da carreira do jogador (0 = primeiro ano); é repassado a
/// `calculate_offseason_competitiveness_impact` para modular a penalidade fictícia GT3.
pub fn apply_offseason_competitiveness_impact(
    team: &mut Team,
    career_year: i32,
) -> OffseasonCompetitivenessImpact {
    let impact = calculate_offseason_competitiveness_impact(team, career_year);

    team.confiabilidade = (team.confiabilidade + impact.reliability_delta).clamp(0.0, 100.0);
    // Sem teto superior (Pilar B): só piso em −5; os retornos decrescentes regulam o topo.
    team.car_performance = (team.car_performance + impact.car_performance_delta).max(-5.0);
    team.engineering = (team.engineering + impact.engineering_delta).clamp(0.0, 100.0);
    team.facilities = (team.facilities + impact.facilities_delta).clamp(0.0, 100.0);

    impact
}

fn management_efficiency_modifier(team: &Team) -> f64 {
    let morale_score = ((team.morale - 0.5) * 100.0).clamp(0.0, 100.0);
    let raw_efficiency = team.engineering * 0.40
        + team.facilities * 0.25
        + morale_score * 0.20
        + team.reputacao * 0.15;

    0.75 + (raw_efficiency.clamp(0.0, 100.0) / 100.0) * 0.50
}

#[derive(Debug, Clone, Copy)]
struct FinanceBias {
    reliability: f64,
    car_performance: f64,
    structure: f64,
}

fn financial_state_bias(state: &str) -> FinanceBias {
    match state {
        "elite" => FinanceBias {
            reliability: 0.9,
            car_performance: 0.45,
            structure: 0.75,
        },
        "healthy" => FinanceBias {
            reliability: 0.55,
            car_performance: 0.25,
            structure: 0.45,
        },
        "pressured" => FinanceBias {
            reliability: -0.55,
            car_performance: 0.05,
            structure: -0.35,
        },
        "crisis" => FinanceBias {
            reliability: -1.25,
            car_performance: -0.25,
            structure: -0.95,
        },
        "collapse" => FinanceBias {
            reliability: -2.25,
            car_performance: -0.65,
            structure: -1.85,
        },
        _ => FinanceBias {
            reliability: 0.0,
            car_performance: 0.0,
            structure: 0.0,
        },
    }
}

fn season_strategy_bias(strategy: &str) -> FinanceBias {
    match strategy {
        "expansion" => FinanceBias {
            reliability: 0.15,
            car_performance: 0.55,
            structure: 0.55,
        },
        "austerity" => FinanceBias {
            reliability: 0.2,
            car_performance: -0.25,
            structure: -0.15,
        },
        "all_in" => FinanceBias {
            reliability: -0.8,
            car_performance: 0.95,
            structure: -0.45,
        },
        "survival" => FinanceBias {
            reliability: -0.45,
            car_performance: -0.55,
            structure: -0.85,
        },
        _ => FinanceBias {
            reliability: 0.15,
            car_performance: 0.15,
            structure: 0.05,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::team::placeholder_team_from_db;

    fn sample_team(id: &str, cash: f64, debt: f64, state: &str, strategy: &str) -> Team {
        let mut team = placeholder_team_from_db(
            id.to_string(),
            "Equipe Financeira".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = cash;
        team.debt_balance = debt;
        team.financial_state = state.to_string();
        team.season_strategy = strategy.to_string();
        team.budget = 55.0;
        team.engineering = 60.0;
        team.facilities = 58.0;
        team.reputacao = 52.0;
        team.morale = 1.0;
        team.confiabilidade = 70.0;
        team.car_performance = 8.0;
        team
    }

    #[test]
    fn real_gt3_marque_outdevelops_an_identical_fictional_team() {
        // Same well-funded GT3 team; only the brand differs. The real manufacturer
        // (marca Some) gains more car performance per offseason than the fictional
        // one (marca None), so factory teams pull away season over season while the
        // fictional teams stay "underdogs".
        let fictional = sample_team("FIC", 1_500_000.0, 0.0, "healthy", "competitive");
        let mut real = sample_team("REAL", 1_500_000.0, 0.0, "healthy", "competitive");
        real.marca = Some("Ferrari".to_string());

        let fictional_gain =
            calculate_offseason_competitiveness_impact(&fictional, 0).car_performance_delta;
        let real_gain = calculate_offseason_competitiveness_impact(&real, 0).car_performance_delta;

        assert!(fictional_gain > 0.0, "both teams are investing the car upward");
        assert!(
            real_gain > fictional_gain,
            "real marque ({real_gain}) must out-develop the fictional team ({fictional_gain})"
        );
        // The fictional gain is exactly the real gain scaled by the penalty factor.
        assert!((fictional_gain - real_gain * FICTIONAL_CAR_DEV_FACTOR).abs() < 1e-9);
    }

    #[test]
    fn the_gt3_penalty_does_not_touch_real_marques_or_other_categories() {
        // A fictional team OUTSIDE the real-vs-fictional GT3 arenas keeps full growth.
        let mut amador = sample_team("AMA", 1_500_000.0, 0.0, "healthy", "competitive");
        amador.categoria = "mazda_amador".to_string();
        assert!((car_dev_gain_factor(&amador, 0) - 1.0).abs() < 1e-9);

        // A real marque in GT3-endurance is never penalised.
        let mut real_endurance = sample_team("RE", 1_500_000.0, 0.0, "healthy", "competitive");
        real_endurance.categoria = "endurance".to_string();
        real_endurance.classe = Some("gt3".to_string());
        real_endurance.marca = Some("Audi".to_string());
        assert!((car_dev_gain_factor(&real_endurance, 0) - 1.0).abs() < 1e-9);

        // ...but a fictional GT3-endurance team is.
        let mut fic_endurance = sample_team("FE", 1_500_000.0, 0.0, "healthy", "competitive");
        fic_endurance.categoria = "endurance".to_string();
        fic_endurance.classe = Some("gt3".to_string());
        assert!((car_dev_gain_factor(&fic_endurance, 0) - FICTIONAL_CAR_DEV_FACTOR).abs() < 1e-9);
    }

    #[test]
    fn fictional_gt3_penalty_fades_out_after_five_career_years() {
        // Identical fictional vs real GT3 pair. Early in the career the fictional
        // team develops slower; by career year 5 (and beyond) the penalty is gone
        // and both develop the car at the same rate.
        let fictional = sample_team("FIC", 1_500_000.0, 0.0, "healthy", "competitive");
        let mut real = sample_team("REAL", 1_500_000.0, 0.0, "healthy", "competitive");
        real.marca = Some("Ferrari".to_string());

        // Year 0: full penalty — fictional develops less than the real marque.
        let fictional_y0 =
            calculate_offseason_competitiveness_impact(&fictional, 0).car_performance_delta;
        let real_y0 = calculate_offseason_competitiveness_impact(&real, 0).car_performance_delta;
        assert!(
            fictional_y0 < real_y0,
            "year 0: fictional ({fictional_y0}) must develop less than real ({real_y0})"
        );

        // Year 5 and 10: penalty fully faded — gains are identical.
        let fictional_y5 =
            calculate_offseason_competitiveness_impact(&fictional, 5).car_performance_delta;
        let real_y5 = calculate_offseason_competitiveness_impact(&real, 5).car_performance_delta;
        assert!(
            (fictional_y5 - real_y5).abs() < 1e-9,
            "year 5: penalty gone, fictional ({fictional_y5}) == real ({real_y5})"
        );

        let fictional_y10 =
            calculate_offseason_competitiveness_impact(&fictional, 10).car_performance_delta;
        let real_y10 = calculate_offseason_competitiveness_impact(&real, 10).car_performance_delta;
        assert!(
            (fictional_y10 - real_y10).abs() < 1e-9,
            "year 10: penalty gone, fictional ({fictional_y10}) == real ({real_y10})"
        );
    }

    #[test]
    fn round_income_stays_positive_for_basic_team_revenue() {
        let round_income = calculate_round_income(125_000.0, 25_000.0, 8_000.0, 0.0);
        assert!(round_income > 0.0);
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
                sponsorship_income: 120_000.0,
                result_bonus: 25_000.0,
                partial_prize_income: 10_000.0,
                aid_income: 0.0,
                salary_expense: 45_000.0,
                event_operations_cost: 20_000.0,
                structural_maintenance_cost: 15_000.0,
                technical_investment_cost: 18_000.0,
                debt_service_cost: 2_500.0,
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
                sponsorship_income: 100_000.0,
                result_bonus: 0.0,
                partial_prize_income: 0.0,
                aid_income: 0.0,
                salary_expense: 100_000.0,
                event_operations_cost: 0.0,
                structural_maintenance_cost: 0.0,
                technical_investment_cost: 0.0,
                debt_service_cost: 0.0,
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
                sponsorship_income: 50_000.0,
                result_bonus: 0.0,
                partial_prize_income: 0.0,
                aid_income: 0.0,
                salary_expense: 50_000.0,
                event_operations_cost: 0.0,
                structural_maintenance_cost: 0.0,
                technical_investment_cost: 0.0,
                debt_service_cost: 0.0,
            },
        );

        assert_eq!(
            team.debt_balance, 6_000_000.0,
            "sem caixa acima da reserva, nada de dívida é pago"
        );
    }

    #[test]
    fn finance_impact_rewards_healthy_cash_with_reliability_support() {
        let rich = sample_team("T001", 1_500_000.0, 0.0, "healthy", "balanced");
        let poor = sample_team("T002", -100_000.0, 650_000.0, "crisis", "survival");

        let rich_impact = calculate_offseason_competitiveness_impact(&rich, 10);
        let poor_impact = calculate_offseason_competitiveness_impact(&poor, 10);

        assert!(rich_impact.reliability_delta > poor_impact.reliability_delta);
        assert!(poor_impact.reliability_delta < 0.0);
    }

    #[test]
    fn finance_impact_gives_all_in_more_car_project_variance_than_balanced() {
        let balanced = sample_team("T001", 600_000.0, 0.0, "stable", "balanced");
        let all_in = sample_team("T002", 600_000.0, 0.0, "pressured", "all_in");

        let balanced_impact = calculate_offseason_competitiveness_impact(&balanced, 10);
        let all_in_impact = calculate_offseason_competitiveness_impact(&all_in, 10);

        assert!(all_in_impact.car_performance_delta > balanced_impact.car_performance_delta);
        assert!(all_in_impact.reliability_delta < balanced_impact.reliability_delta);
    }

    #[test]
    fn applying_finance_impact_changes_team_attributes_with_safe_clamps() {
        let mut team = sample_team("T001", -100_000.0, 900_000.0, "collapse", "survival");
        team.confiabilidade = 4.0;
        team.car_performance = -4.8;
        team.engineering = 2.0;
        team.facilities = 2.0;

        apply_offseason_competitiveness_impact(&mut team, 10);

        assert!((0.0..=100.0).contains(&team.confiabilidade));
        // Pilar B: piso em −5, sem teto superior.
        assert!(team.car_performance >= -5.0);
        assert!((0.0..=100.0).contains(&team.engineering));
        assert!((0.0..=100.0).contains(&team.facilities));
    }

    #[test]
    fn car_investment_has_diminishing_returns() {
        // Mesma força financeira; o carro já alto ganha menos que o carro baixo.
        let mut low = sample_team("T001", 5_000_000.0, 0.0, "elite", "balanced");
        low.car_performance = 2.0;
        let mut high = sample_team("T002", 5_000_000.0, 0.0, "elite", "balanced");
        high.car_performance = 24.0;

        let low_gain = calculate_offseason_competitiveness_impact(&low, 10).car_performance_delta;
        let high_gain = calculate_offseason_competitiveness_impact(&high, 10).car_performance_delta;

        assert!(low_gain > 0.0 && high_gain > 0.0);
        assert!(
            low_gain > high_gain,
            "retornos decrescentes: carro baixo ({low_gain:.3}) deveria ganhar mais que carro alto ({high_gain:.3})"
        );
    }

    #[test]
    fn rich_team_car_grows_past_old_ceiling_of_16() {
        // Equipe muito rica e bem gerida: ao longo de várias offseasons o carro
        // ultrapassa o antigo teto rígido de 16 (Pilar B: sem teto).
        let mut team = sample_team("T001", 30_000_000.0, 0.0, "elite", "expansion");
        team.car_performance = 15.0;
        for _ in 0..8 {
            apply_offseason_competitiveness_impact(&mut team, 10);
        }
        assert!(
            team.car_performance > 16.0,
            "carro deveria passar de 16 (ficou em {:.2})",
            team.car_performance
        );
    }
}
