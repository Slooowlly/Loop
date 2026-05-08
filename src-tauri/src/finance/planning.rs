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

pub fn category_finance_scale(category: &str) -> CategoryFinanceScale {
    match category {
        "mazda_rookie" | "toyota_rookie" => CategoryFinanceScale {
            cash_min: 100_000.0,
            cash_max: 700_000.0,
            operating_cost_min: 120_000.0,
            operating_cost_max: 250_000.0,
        },
        "mazda_amador" | "toyota_amador" => CategoryFinanceScale {
            cash_min: 250_000.0,
            cash_max: 1_500_000.0,
            operating_cost_min: 250_000.0,
            operating_cost_max: 600_000.0,
        },
        "bmw_m2" | "production_challenger" => CategoryFinanceScale {
            cash_min: 750_000.0,
            cash_max: 4_000_000.0,
            operating_cost_min: 600_000.0,
            operating_cost_max: 1_600_000.0,
        },
        "gt4" => CategoryFinanceScale {
            cash_min: 2_000_000.0,
            cash_max: 9_000_000.0,
            operating_cost_min: 1_500_000.0,
            operating_cost_max: 4_000_000.0,
        },
        "gt3" => CategoryFinanceScale {
            cash_min: 6_000_000.0,
            cash_max: 25_000_000.0,
            operating_cost_min: 4_000_000.0,
            operating_cost_max: 12_000_000.0,
        },
        "lmp2" => CategoryFinanceScale {
            cash_min: 10_000_000.0,
            cash_max: 45_000_000.0,
            operating_cost_min: 7_000_000.0,
            operating_cost_max: 20_000_000.0,
        },
        "endurance" => CategoryFinanceScale {
            cash_min: 12_000_000.0,
            cash_max: 60_000_000.0,
            operating_cost_min: 8_000_000.0,
            operating_cost_max: 25_000_000.0,
        },
        _ => CategoryFinanceScale {
            cash_min: 750_000.0,
            cash_max: 4_000_000.0,
            operating_cost_min: 600_000.0,
            operating_cost_max: 1_600_000.0,
        },
    }
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

pub fn calculate_projected_income(team: &Team) -> f64 {
    let scale = category_finance_scale(&team.categoria);
    let reputation_factor = 0.70 + team.reputacao.clamp(0.0, 100.0) / 250.0;
    let performance_factor = 0.85 + (team.car_performance + 5.0).clamp(0.0, 21.0) / 105.0;

    scale.expected_cash_midpoint() * 0.45 * reputation_factor * performance_factor
        + team.parachute_payment_remaining.max(0.0)
}

pub fn calculate_committed_costs(team: &Team) -> f64 {
    let scale = category_finance_scale(&team.categoria);
    let structure_factor = 0.70
        + team.facilities.clamp(0.0, 100.0) / 350.0
        + team.engineering.clamp(0.0, 100.0) / 450.0
        + team.pit_crew_quality.clamp(0.0, 100.0) / 550.0;

    scale.operating_cost_midpoint() * structure_factor
}

pub fn calculate_available_credit(team: &Team) -> f64 {
    let scale = category_finance_scale(&team.categoria);
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

pub fn calculate_safety_reserve(team: &Team) -> f64 {
    let scale = category_finance_scale(&team.categoria);
    scale.operating_cost_midpoint() * safety_reserve_multiplier_for_state(&team.financial_state)
}

pub fn calculate_spending_power(team: &Team) -> f64 {
    let projected_income = calculate_projected_income(team);
    let committed_costs = calculate_committed_costs(team);
    let available_credit = calculate_available_credit(team);
    let debt_pressure = calculate_debt_pressure(team);
    let safety_reserve = calculate_safety_reserve(team);

    team.cash_balance
        + projected_income * income_confidence_for_state(&team.financial_state)
        + team.parachute_payment_remaining.max(0.0)
        + available_credit * credit_aggressiveness_for_state(&team.financial_state)
        - committed_costs
        - debt_pressure
        - safety_reserve
}

pub fn derive_budget_index_from_money(team: &Team) -> f64 {
    let scale = category_finance_scale(&team.categoria);
    let projected_income = calculate_projected_income(team);
    let spending_power = calculate_spending_power(team);
    let debt_pressure = calculate_debt_pressure(team);
    let category_window = (scale.cash_max - scale.cash_min).max(1.0);
    let effective_money =
        team.cash_balance + spending_power * 0.45 + projected_income * 0.25 - debt_pressure * 0.35;

    ((effective_money - scale.cash_min) / category_window * 100.0).clamp(0.0, 100.0)
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

    #[test]
    fn same_cash_is_weaker_in_higher_category() {
        let rookie = sample_team("mazda_rookie", 700_000.0, 0.0, "healthy");
        let gt3 = sample_team("gt3", 700_000.0, 0.0, "healthy");

        let rookie_plan = calculate_financial_plan(&rookie);
        let gt3_plan = calculate_financial_plan(&gt3);

        assert!(rookie_plan.budget_index > gt3_plan.budget_index);
    }
}
