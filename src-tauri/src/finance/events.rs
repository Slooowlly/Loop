use crate::models::team::Team;

#[derive(Debug, Clone, PartialEq)]
pub struct FinanceEventOutcome {
    pub kind: String,
    pub cash_delta: f64,
    pub debt_delta: f64,
}

pub fn debt_service(debt_balance: f64, round_interest_rate: f64) -> f64 {
    if debt_balance <= 0.0 {
        return 0.0;
    }

    debt_balance * round_interest_rate.max(0.0)
}

pub fn debt_interest_rate_for_state(state: &str) -> f64 {
    match state {
        "elite" | "healthy" => 0.0075,
        "stable" => 0.0125,
        "pressured" => 0.020,
        "crisis" => 0.0325,
        "collapse" => 0.050,
        _ => 0.015,
    }
}

pub fn debt_service_for_state(debt_balance: f64, state: &str) -> f64 {
    debt_service(debt_balance, debt_interest_rate_for_state(state))
}

pub fn emergency_loan_amount(team: &Team) -> Option<f64> {
    if team.financial_state != "collapse" {
        return None;
    }

    if team.cash_balance > -75_000.0 && team.debt_balance < 750_000.0 {
        return None;
    }

    Some(
        category_emergency_loan_base(&team.categoria)
            * (0.85 + team.reputacao.clamp(0.0, 100.0) / 500.0),
    )
}

pub fn technical_breakthrough_chance(team: &Team) -> f64 {
    if team.engineering < 70.0 {
        return 0.0;
    }

    let pressure_bonus = match team.financial_state.as_str() {
        "pressured" => 0.015,
        "crisis" => 0.025,
        "collapse" => 0.01,
        _ => 0.005,
    };
    let engineering_bonus = ((team.engineering - 70.0) / 30.0).clamp(0.0, 1.0) * 0.05;

    (pressure_bonus + engineering_bonus).clamp(0.0, 0.10)
}

pub fn parachute_payment_for_relegation(team: &Team) -> f64 {
    category_parachute_payment_base(&team.categoria)
        * (0.85 + team.reputacao.clamp(0.0, 100.0) / 400.0)
}

pub fn apply_crisis_event_if_needed(team: &mut Team) -> Option<FinanceEventOutcome> {
    let loan_amount = emergency_loan_amount(team)?;
    let debt_fee_multiplier = 1.18;
    let debt_delta = loan_amount * debt_fee_multiplier;

    team.cash_balance += loan_amount;
    team.debt_balance += debt_delta;

    Some(FinanceEventOutcome {
        kind: "emergency_loan".to_string(),
        cash_delta: loan_amount,
        debt_delta,
    })
}

fn category_emergency_loan_base(category: &str) -> f64 {
    match category {
        "mazda_rookie" | "toyota_rookie" => 150_000.0,
        "mazda_amador" | "toyota_amador" => 225_000.0,
        "bmw_m2" => 300_000.0,
        "production_challenger" => 375_000.0,
        "gt4" => 475_000.0,
        "gt3" => 650_000.0,
        "endurance" => 800_000.0,
        _ => 250_000.0,
    }
}

fn category_parachute_payment_base(category: &str) -> f64 {
    match category {
        "mazda_rookie" | "toyota_rookie" => 120_000.0,
        "mazda_amador" | "toyota_amador" => 180_000.0,
        "bmw_m2" => 250_000.0,
        "production_challenger" => 325_000.0,
        "gt4" => 425_000.0,
        "gt3" => 575_000.0,
        "endurance" => 700_000.0,
        _ => 200_000.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::team::placeholder_team_from_db;

    fn sample_team(
        state: &str,
        cash: f64,
        debt: f64,
        engineering: f64,
    ) -> crate::models::team::Team {
        let mut team = placeholder_team_from_db(
            "T001".to_string(),
            "Equipe em Crise".to_string(),
            "gt4".to_string(),
            "2026-01-01".to_string(),
        );
        team.financial_state = state.to_string();
        team.cash_balance = cash;
        team.debt_balance = debt;
        team.engineering = engineering;
        team.reputacao = 55.0;
        team
    }

    #[test]
    fn debt_service_is_positive_when_team_owes_money() {
        assert!(debt_service(100_000.0, 0.015) > 0.0);
    }

    #[test]
    fn debt_service_is_harsher_for_collapse_than_healthy_state() {
        let debt = 1_000_000.0;

        let healthy = debt_service_for_state(debt, "healthy");
        let collapse = debt_service_for_state(debt, "collapse");

        assert!(collapse > healthy);
    }

    #[test]
    fn unknown_debt_service_state_uses_safe_default() {
        assert_eq!(
            debt_service_for_state(1_000_000.0, "mystery"),
            debt_service(1_000_000.0, 0.015)
        );
    }

    #[test]
    fn state_based_debt_service_stays_zero_without_debt() {
        assert_eq!(debt_service_for_state(0.0, "collapse"), 0.0);
    }

    #[test]
    fn collapse_team_can_trigger_emergency_loan() {
        let team = sample_team("collapse", -100_000.0, 850_000.0, 45.0);

        let loan = emergency_loan_amount(&team).expect("collapse team should receive loan option");

        assert!(loan > 0.0);
    }

    #[test]
    fn technical_breakthrough_requires_good_engineering() {
        let weak = sample_team("pressured", 50_000.0, 0.0, 35.0);
        let clever = sample_team("pressured", 50_000.0, 0.0, 82.0);

        assert_eq!(technical_breakthrough_chance(&weak), 0.0);
        assert!(technical_breakthrough_chance(&clever) > 0.0);
    }

    #[test]
    fn relegated_team_gets_parachute_payment() {
        let team = sample_team("stable", 200_000.0, 0.0, 55.0);

        let payment = parachute_payment_for_relegation(&team);

        assert!(payment > 0.0);
    }

    #[test]
    fn applying_crisis_event_improves_cash_but_increases_debt() {
        let mut team = sample_team("collapse", -100_000.0, 850_000.0, 45.0);
        let before_cash = team.cash_balance;
        let before_debt = team.debt_balance;

        let event = apply_crisis_event_if_needed(&mut team).expect("event should be applied");

        assert_eq!(event.kind, "emergency_loan");
        assert!(team.cash_balance > before_cash);
        assert!(team.debt_balance > before_debt);
    }
}
