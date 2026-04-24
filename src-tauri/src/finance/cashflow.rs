use crate::finance::events::technical_breakthrough_chance;
use crate::models::team::Team;

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

    if team.parachute_payment_remaining > 0.0 {
        team.parachute_payment_remaining =
            (team.parachute_payment_remaining - context.aid_income.max(0.0)).max(0.0);
    }

    summary
}

pub fn calculate_offseason_competitiveness_impact(team: &Team) -> OffseasonCompetitivenessImpact {
    let efficiency = management_efficiency_modifier(team);
    let cash_strength = (team.cash_balance / 1_000_000.0).clamp(-0.5, 1.2);
    let debt_pressure = (team.debt_balance / 900_000.0).clamp(0.0, 1.2);
    let state = financial_state_bias(&team.financial_state);
    let strategy = season_strategy_bias(&team.season_strategy);
    let breakthrough_expected_value = technical_breakthrough_chance(team) * 4.0;

    let reliability_delta =
        (cash_strength * 1.8 - debt_pressure * 3.2 + state.reliability + strategy.reliability)
            * efficiency;
    let car_performance_delta = (cash_strength * 0.55 - debt_pressure * 0.65
        + state.car_performance
        + strategy.car_performance
        + breakthrough_expected_value)
        * efficiency;
    let structure_delta =
        (cash_strength * 1.15 - debt_pressure * 2.25 + state.structure + strategy.structure)
            * efficiency;

    OffseasonCompetitivenessImpact {
        reliability_delta: reliability_delta.clamp(-6.0, 4.0),
        car_performance_delta: car_performance_delta.clamp(-1.4, 1.4),
        engineering_delta: structure_delta.clamp(-3.5, 2.5),
        facilities_delta: (structure_delta * 0.75).clamp(-2.5, 1.8),
    }
}

pub fn apply_offseason_competitiveness_impact(team: &mut Team) -> OffseasonCompetitivenessImpact {
    let impact = calculate_offseason_competitiveness_impact(team);

    team.confiabilidade = (team.confiabilidade + impact.reliability_delta).clamp(0.0, 100.0);
    team.car_performance = (team.car_performance + impact.car_performance_delta).clamp(-5.0, 16.0);
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
    fn finance_impact_rewards_healthy_cash_with_reliability_support() {
        let rich = sample_team("T001", 1_500_000.0, 0.0, "healthy", "balanced");
        let poor = sample_team("T002", -100_000.0, 650_000.0, "crisis", "survival");

        let rich_impact = calculate_offseason_competitiveness_impact(&rich);
        let poor_impact = calculate_offseason_competitiveness_impact(&poor);

        assert!(rich_impact.reliability_delta > poor_impact.reliability_delta);
        assert!(poor_impact.reliability_delta < 0.0);
    }

    #[test]
    fn finance_impact_gives_all_in_more_car_project_variance_than_balanced() {
        let balanced = sample_team("T001", 600_000.0, 0.0, "stable", "balanced");
        let all_in = sample_team("T002", 600_000.0, 0.0, "pressured", "all_in");

        let balanced_impact = calculate_offseason_competitiveness_impact(&balanced);
        let all_in_impact = calculate_offseason_competitiveness_impact(&all_in);

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

        apply_offseason_competitiveness_impact(&mut team);

        assert!((0.0..=100.0).contains(&team.confiabilidade));
        assert!((-5.0..=16.0).contains(&team.car_performance));
        assert!((0.0..=100.0).contains(&team.engineering));
        assert!((0.0..=100.0).contains(&team.facilities));
    }
}
