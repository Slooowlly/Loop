use crate::constants::categories::get_category_config;
use crate::finance::planning::{calculate_financial_plan, derive_budget_index_from_money};
use crate::models::team::Team;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviousTeamStanding {
    pub position: i32,
    pub total_teams: usize,
}

pub fn category_pit_crew_cap(category: &str) -> f64 {
    match category {
        "mazda_rookie" | "toyota_rookie" => 55.0,
        "mazda_amador" | "toyota_amador" => 64.0,
        "bmw_m2" => 72.0,
        "production_challenger" => 76.0,
        "gt4" => 84.0,
        "gt3" => 93.0,
        "endurance" => 97.0,
        _ => 80.0,
    }
}

pub fn category_risk_modifier(category: &str) -> f64 {
    match category {
        "mazda_rookie" | "toyota_rookie" => -6.0,
        "mazda_amador" | "toyota_amador" => -4.0,
        "bmw_m2" => -2.0,
        "production_challenger" => 0.0,
        "gt4" => 2.0,
        "gt3" => 3.0,
        "endurance" => -2.0,
        _ => 0.0,
    }
}

pub fn seed_pit_crew_quality(
    category: &str,
    budget: f64,
    engineering: f64,
    facilities: f64,
) -> f64 {
    pit_crew_target_quality(category, budget, engineering, facilities, 0.0)
}

pub fn seed_pit_strategy_risk(
    category: &str,
    budget: f64,
    car_performance: f64,
    team_id: &str,
) -> f64 {
    let budget_strength = (budget / 100.0).clamp(0.0, 1.0);
    let car_strength = ((car_performance + 5.0) / 21.0).clamp(0.0, 1.0);
    let identity = team_risk_identity(team_id);
    (45.0 - budget_strength * 10.0 - car_strength * 8.0
        + category_risk_modifier(category)
        + identity * 6.0)
        .clamp(0.0, 100.0)
}

pub fn seed_pit_crew_quality_from_team(team: &Team) -> f64 {
    pit_crew_target_quality(
        &team.categoria,
        derive_budget_index_from_money(team),
        team.engineering,
        team.facilities,
        0.0,
    )
}

pub fn seed_pit_strategy_risk_from_team(team: &Team) -> f64 {
    let money_strength = (derive_budget_index_from_money(team) / 100.0).clamp(0.0, 1.0);
    let car_strength = ((team.car_performance + 5.0) / 21.0).clamp(0.0, 1.0);
    let identity = team_risk_identity(&team.id);
    (45.0 - money_strength * 10.0 - car_strength * 8.0
        + category_risk_modifier(&team.categoria)
        + financial_risk_modifier(&team.financial_state)
        + identity * 6.0)
        .clamp(0.0, 100.0)
}

pub fn recalculate_pit_crew_quality(
    team: &Team,
    previous_standing: Option<PreviousTeamStanding>,
) -> f64 {
    let momentum = season_momentum(team, previous_standing);
    let plan = calculate_financial_plan(team);
    let target = pit_crew_target_quality(
        &team.categoria,
        plan.budget_index,
        team.engineering,
        team.facilities,
        momentum,
    );
    (team.pit_crew_quality * 0.65 + target * 0.35)
        .clamp(0.0, category_pit_crew_cap(&team.categoria))
}

pub fn recalculate_pit_strategy_risk(team: &Team, category_peers: &[Team]) -> f64 {
    let percentile = performance_percentile(team, category_peers);
    let tier = get_category_config(&team.categoria)
        .map(|config| config.tier)
        .unwrap_or(0);
    let title_pressure = ((0.30 - percentile) / 0.30).clamp(0.0, 1.0);
    let relegation_pressure = ((percentile - 0.60) / 0.40).clamp(0.0, 1.0);
    let promotion_pressure = if tier < 4 {
        (1.0 - ((percentile - 0.40).abs() / 0.25)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let backmarker_pressure = ((percentile - 0.70) / 0.30).clamp(0.0, 1.0);
    let plan = calculate_financial_plan(team);
    let budget_strength = (plan.budget_index / 100.0).clamp(0.0, 1.0);
    let weak_budget_bonus = if plan.budget_index < 30.0 { 6.0 } else { 0.0 };
    let front_runner_penalty = if percentile < 0.33 { 5.0 } else { 0.0 };
    let backmarker_bonus = if percentile > 0.66 { 5.0 } else { 0.0 };
    let identity = team_risk_identity(&team.id);

    let target = (45.0
        + relegation_pressure * 28.0
        + promotion_pressure * 18.0
        + backmarker_pressure * 14.0
        - title_pressure * 26.0
        - budget_strength * 8.0
        + weak_budget_bonus
        + backmarker_bonus
        - front_runner_penalty
        + category_risk_modifier(&team.categoria)
        + financial_risk_modifier(&team.financial_state)
        + identity * 6.0)
        .clamp(0.0, 100.0);

    (team.pit_strategy_risk * 0.40 + target * 0.60).clamp(0.0, 100.0)
}

fn pit_crew_target_quality(
    category: &str,
    budget: f64,
    engineering: f64,
    facilities: f64,
    momentum: f64,
) -> f64 {
    let cap = category_pit_crew_cap(category);
    let base = budget * 0.45 + engineering * 0.35 + facilities * 0.20;
    (base + momentum).clamp(0.0, cap)
}

fn financial_risk_modifier(state: &str) -> f64 {
    match state {
        "elite" => -8.0,
        "healthy" => -4.0,
        "stable" => 0.0,
        "pressured" => 6.0,
        "crisis" => 10.0,
        "collapse" => 14.0,
        _ => 0.0,
    }
}

fn season_momentum(team: &Team, previous_standing: Option<PreviousTeamStanding>) -> f64 {
    let mut momentum: f64 = 0.0;

    if let Some(previous_standing) = previous_standing {
        if previous_standing.position == 1 {
            momentum += 4.0;
        } else if previous_standing.position <= 3 {
            momentum += 2.0;
        } else if previous_standing.position as f64
            >= previous_standing.total_teams as f64 * (2.0 / 3.0)
        {
            momentum -= 2.0;
        }

        if previous_standing.position <= team.meta_posicao {
            momentum += 2.0;
        } else if previous_standing.position >= team.meta_posicao + 3 {
            momentum -= 2.0;
        }
    }

    if let Some(previous_category) = team.categoria_anterior.as_deref() {
        if let (Some(previous), Some(current)) = (
            get_category_config(previous_category),
            get_category_config(&team.categoria),
        ) {
            if current.tier > previous.tier {
                momentum += 2.0;
            } else if current.tier < previous.tier {
                momentum -= 3.0;
            }
        }
    }

    momentum.clamp(-6.0, 6.0)
}

fn performance_percentile(team: &Team, category_peers: &[Team]) -> f64 {
    if category_peers.len() <= 1 {
        return 0.5;
    }

    let strongest = category_peers
        .iter()
        .map(|candidate| candidate.car_performance)
        .fold(f64::NEG_INFINITY, f64::max);
    let weakest = category_peers
        .iter()
        .map(|candidate| candidate.car_performance)
        .fold(f64::INFINITY, f64::min);
    let spread = strongest - weakest;

    if spread.abs() < f64::EPSILON {
        return 0.5;
    }

    ((strongest - team.car_performance) / spread).clamp(0.0, 1.0)
}

fn team_risk_identity(team_id: &str) -> f64 {
    let seed = team_id.bytes().fold(0_u32, |acc, byte| {
        acc.wrapping_mul(33).wrapping_add(byte as u32)
    });
    ((seed % 200) as f64 / 100.0) - 1.0
}

#[cfg(test)]
mod tests {
    use crate::models::team::placeholder_team_from_db;

    use super::*;

    fn sample_team(
        id: &str,
        category: &str,
        car: f64,
        budget: f64,
        engineering: f64,
        facilities: f64,
    ) -> Team {
        let mut team = placeholder_team_from_db(
            id.to_string(),
            format!("Team {id}"),
            category.to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.car_performance = car;
        team.budget = budget;
        team.engineering = engineering;
        team.facilities = facilities;
        team.pit_strategy_risk = 50.0;
        team.pit_crew_quality = 50.0;
        team
    }

    #[test]
    fn rookie_categories_have_operational_cap() {
        let quality = seed_pit_crew_quality("mazda_rookie", 100.0, 100.0, 100.0);
        assert_eq!(quality, 55.0);
    }

    #[test]
    fn rich_team_gets_better_pit_crew_quality() {
        let rich = seed_pit_crew_quality("gt3", 85.0, 80.0, 78.0);
        let poor = seed_pit_crew_quality("gt3", 25.0, 40.0, 35.0);
        assert!(rich > poor, "expected rich {rich} > poor {poor}");
    }

    #[test]
    fn rich_team_gets_better_pit_crew_even_with_low_legacy_budget() {
        let mut rich = sample_team("TRICH", "gt3", 10.0, 1.0, 80.0, 78.0);
        rich.cash_balance = 18_000_000.0;
        rich.debt_balance = 0.0;
        rich.financial_state = "healthy".to_string();

        let mut poor = sample_team("TPOOR", "gt3", 10.0, 99.0, 40.0, 35.0);
        poor.cash_balance = 250_000.0;
        poor.debt_balance = 8_000_000.0;
        poor.financial_state = "crisis".to_string();

        assert!(
            recalculate_pit_crew_quality(&rich, None) > recalculate_pit_crew_quality(&poor, None)
        );
    }

    #[test]
    fn pressured_backmarker_gets_higher_risk() {
        let favorite = sample_team("T001", "gt4", 12.0, 85.0, 80.0, 78.0);
        let backmarker = sample_team("T004", "gt4", 4.0, 18.0, 35.0, 30.0);
        let peers = vec![
            favorite.clone(),
            sample_team("T002", "gt4", 10.0, 70.0, 65.0, 60.0),
            sample_team("T003", "gt4", 8.0, 55.0, 55.0, 55.0),
            backmarker.clone(),
        ];

        let favorite_risk = recalculate_pit_strategy_risk(&favorite, &peers);
        let backmarker_risk = recalculate_pit_strategy_risk(&backmarker, &peers);

        assert!(backmarker_risk > favorite_risk);
    }

    #[test]
    fn poor_pressured_team_gets_more_strategy_risk_than_rich_peer() {
        let mut rich = sample_team("TRICH", "gt4", 8.0, 1.0, 80.0, 78.0);
        rich.cash_balance = 8_000_000.0;
        rich.debt_balance = 0.0;
        rich.financial_state = "healthy".to_string();

        let mut poor = sample_team("TPOOR", "gt4", 8.0, 99.0, 40.0, 35.0);
        poor.cash_balance = 100_000.0;
        poor.debt_balance = 2_500_000.0;
        poor.financial_state = "crisis".to_string();

        let peers = vec![rich.clone(), poor.clone()];

        assert!(
            recalculate_pit_strategy_risk(&poor, &peers)
                > recalculate_pit_strategy_risk(&rich, &peers)
        );
    }

    #[test]
    fn pit_crew_quality_smoothing_preserves_continuity() {
        let mut team = sample_team("T001", "gt3", 10.0, 80.0, 82.0, 78.0);
        team.cash_balance = 18_000_000.0;
        team.debt_balance = 0.0;
        team.financial_state = "healthy".to_string();
        team.pit_crew_quality = 70.0;
        let updated = recalculate_pit_crew_quality(
            &team,
            Some(PreviousTeamStanding {
                position: 1,
                total_teams: 14,
            }),
        );
        assert!(updated > 70.0);
        assert!(updated < category_pit_crew_cap("gt3"));
    }
}
