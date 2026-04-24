use crate::finance::planning::{calculate_financial_plan, category_finance_scale};
use crate::models::team::Team;

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

    if plan.spending_power < scale.operating_cost_midpoint() * 0.20 && team.car_performance < 6.0 {
        return "all_in";
    }

    match derive_financial_state(financial_health_score(team)) {
        "elite" => "balanced",
        "healthy" => {
            if team.car_performance < 8.0 {
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
    team.financial_state = derive_financial_state(financial_health_score(team)).to_string();
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

    #[test]
    fn weak_real_money_position_can_trigger_austerity_despite_high_legacy_budget() {
        let mut team = sample_team("gt4", 3_500_000.0, 0.0, "stable");
        team.budget = 99.0;
        team.reputacao = 75.0;
        team.engineering = 70.0;
        team.facilities = 70.0;
        team.car_performance = 9.0;

        assert_eq!(choose_season_strategy(&team), "austerity");
    }
}
