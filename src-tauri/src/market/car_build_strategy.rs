use crate::calendar::CalendarEntry;
use crate::constants::categories::get_category_config;
use crate::finance::planning::{calculate_financial_plan, category_finance_scale};
use crate::models::team::Team;
use crate::simulation::car_build::{profile_money_cost, track_advantage, CarBuildProfile};
use crate::simulation::track_profile::get_track_simulation_data;

const ALL_PROFILES: [CarBuildProfile; 7] = [
    CarBuildProfile::Balanced,
    CarBuildProfile::AccelerationIntermediate,
    CarBuildProfile::PowerIntermediate,
    CarBuildProfile::HandlingIntermediate,
    CarBuildProfile::AccelerationExtreme,
    CarBuildProfile::PowerExtreme,
    CarBuildProfile::HandlingExtreme,
];

const CALENDAR_WEIGHT: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct CompetitiveContext {
    title_pressure: f64,
    promotion_pressure: f64,
    stability_pressure: f64,
    relegation_pressure: f64,
}

pub fn choose_car_build_profile(
    team: &Team,
    category_peers: &[Team],
    calendar: &[CalendarEntry],
) -> CarBuildProfile {
    let context = build_competitive_context(team, category_peers);
    let mut best_profile = CarBuildProfile::Balanced;
    let mut best_score = f64::NEG_INFINITY;

    for profile in ALL_PROFILES {
        let score = score_profile(team, category_peers, calendar, context, profile);
        if score > best_score {
            best_score = score;
            best_profile = profile;
        }
    }

    best_profile
}

fn score_profile(
    team: &Team,
    category_peers: &[Team],
    calendar: &[CalendarEntry],
    context: CompetitiveContext,
    profile: CarBuildProfile,
) -> f64 {
    calendar_fit_score(calendar, profile)
        + strategy_bias(context, profile)
        + affordability_bias(team, profile)
        + car_strength_bias(team, category_peers, profile)
        + movement_bias(team, profile)
        + team_identity_bias(team, profile)
}

fn calendar_fit_score(calendar: &[CalendarEntry], profile: CarBuildProfile) -> f64 {
    if calendar.is_empty() {
        return 0.0;
    }

    let total_advantage: f64 = calendar
        .iter()
        .map(|entry| {
            let data = get_track_simulation_data(entry.track_id);
            track_advantage(
                profile,
                (
                    data.acceleration_weight,
                    data.power_weight,
                    data.handling_weight,
                ),
            )
        })
        .sum();

    (total_advantage / calendar.len() as f64) * CALENDAR_WEIGHT
}

fn strategy_bias(context: CompetitiveContext, profile: CarBuildProfile) -> f64 {
    match profile {
        CarBuildProfile::Balanced => {
            context.title_pressure * 8.0 + context.stability_pressure * 4.5
                - context.promotion_pressure * 1.5
                - context.relegation_pressure * 5.5
        }
        CarBuildProfile::AccelerationIntermediate
        | CarBuildProfile::PowerIntermediate
        | CarBuildProfile::HandlingIntermediate => {
            context.title_pressure * 1.5
                + context.promotion_pressure * 4.0
                + context.stability_pressure * 2.0
                + context.relegation_pressure * 1.5
        }
        CarBuildProfile::AccelerationExtreme
        | CarBuildProfile::PowerExtreme
        | CarBuildProfile::HandlingExtreme => {
            -context.title_pressure * 6.0 + context.promotion_pressure * 1.5
                - context.stability_pressure * 2.5
                + context.relegation_pressure * 7.0
        }
    }
}

fn affordability_bias(team: &Team, profile: CarBuildProfile) -> f64 {
    let plan = calculate_financial_plan(team);
    let scale = category_finance_scale(&team.categoria);
    let car_unit = scale.operating_cost_midpoint() * 0.30;
    let cost = profile_money_cost(profile, car_unit);
    let remaining = plan.spending_power - cost;

    if remaining < -car_unit * 0.75 {
        return -40.0;
    }

    let mut score = (remaining / car_unit * 6.0).clamp(-18.0, 18.0);

    if plan.spending_power < car_unit {
        score += match profile {
            CarBuildProfile::Balanced => -12.0,
            CarBuildProfile::AccelerationIntermediate
            | CarBuildProfile::PowerIntermediate
            | CarBuildProfile::HandlingIntermediate => -4.0,
            _ => 4.0,
        };
    } else if plan.spending_power > car_unit * 3.0 {
        score += match profile {
            CarBuildProfile::Balanced => 4.0,
            CarBuildProfile::AccelerationIntermediate
            | CarBuildProfile::PowerIntermediate
            | CarBuildProfile::HandlingIntermediate => 1.0,
            _ => -2.0,
        };
    }

    score
}

fn car_strength_bias(team: &Team, category_peers: &[Team], profile: CarBuildProfile) -> f64 {
    let percentile = performance_percentile(team, category_peers);
    let front_bias = ((0.35 - percentile) / 0.35).clamp(0.0, 1.0);
    let back_bias = ((percentile - 0.65) / 0.35).clamp(0.0, 1.0);

    match profile {
        CarBuildProfile::Balanced => front_bias * 4.0 - back_bias * 4.0,
        CarBuildProfile::AccelerationIntermediate
        | CarBuildProfile::PowerIntermediate
        | CarBuildProfile::HandlingIntermediate => front_bias * 1.5 + back_bias * 1.5,
        CarBuildProfile::AccelerationExtreme
        | CarBuildProfile::PowerExtreme
        | CarBuildProfile::HandlingExtreme => -front_bias * 4.5 + back_bias * 4.5,
    }
}

fn movement_bias(team: &Team, profile: CarBuildProfile) -> f64 {
    let Some(previous_category) = team.categoria_anterior.as_deref() else {
        return 0.0;
    };
    let Some(previous) = get_category_config(previous_category) else {
        return 0.0;
    };
    let Some(current) = get_category_config(&team.categoria) else {
        return 0.0;
    };

    if current.tier > previous.tier {
        match profile {
            CarBuildProfile::Balanced => 5.0,
            CarBuildProfile::AccelerationIntermediate
            | CarBuildProfile::PowerIntermediate
            | CarBuildProfile::HandlingIntermediate => 2.0,
            _ => -4.0,
        }
    } else if current.tier < previous.tier {
        match profile {
            CarBuildProfile::Balanced => 2.0,
            CarBuildProfile::AccelerationIntermediate
            | CarBuildProfile::PowerIntermediate
            | CarBuildProfile::HandlingIntermediate => 3.0,
            _ => 1.0,
        }
    } else {
        0.0
    }
}

fn build_competitive_context(team: &Team, category_peers: &[Team]) -> CompetitiveContext {
    let percentile = performance_percentile(team, category_peers);
    let tier = get_category_config(&team.categoria)
        .map(|config| config.tier)
        .unwrap_or(0);
    let title_pressure = ((0.35 - percentile) / 0.35).clamp(0.0, 1.0);
    let relegation_pressure = ((percentile - 0.55) / 0.30).clamp(0.0, 1.0);
    let promotion_pressure = if tier < 4 {
        let promotion_window = (1.0 - ((percentile - 0.38).abs() / 0.22)).clamp(0.0, 1.0);
        promotion_window * (1.0 - title_pressure * 0.7)
    } else {
        0.0
    };
    let stability_pressure = (1.0
        - title_pressure
            .max(relegation_pressure)
            .max(promotion_pressure))
    .clamp(0.0, 1.0);

    CompetitiveContext {
        title_pressure,
        promotion_pressure,
        stability_pressure,
        relegation_pressure,
    }
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

fn team_identity_bias(team: &Team, profile: CarBuildProfile) -> f64 {
    let tilt = team_risk_tilt(team);
    let preference_for_stability = 1.0 - tilt.abs();

    match profile {
        CarBuildProfile::Balanced => preference_for_stability * 1.5,
        CarBuildProfile::AccelerationIntermediate
        | CarBuildProfile::PowerIntermediate
        | CarBuildProfile::HandlingIntermediate => 0.75 - tilt.abs() * 0.25,
        CarBuildProfile::AccelerationExtreme
        | CarBuildProfile::PowerExtreme
        | CarBuildProfile::HandlingExtreme => tilt.abs() * 1.5,
    }
}

fn team_risk_tilt(team: &Team) -> f64 {
    let seed = team.id.bytes().fold(0_u32, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(byte as u32)
    });
    ((seed % 200) as f64 / 100.0) - 1.0
}

#[cfg(test)]
mod tests {
    use crate::calendar::CalendarEntry;
    use crate::models::enums::{RaceStatus, SeasonPhase, ThematicSlot, WeatherCondition};
    use crate::models::team::placeholder_team_from_db;

    use super::*;

    fn sample_team(id: &str, category: &str, car: f64, budget: f64) -> Team {
        let mut team = placeholder_team_from_db(
            id.to_string(),
            format!("Team {id}"),
            category.to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.car_performance = car;
        team.budget = budget;
        team
    }

    fn sample_calendar_entry(
        id: &str,
        season_id: &str,
        category: &str,
        rodada: i32,
        track_id: u32,
    ) -> CalendarEntry {
        CalendarEntry {
            id: id.to_string(),
            season_id: season_id.to_string(),
            categoria: category.to_string(),
            rodada,
            nome: format!("Round {rodada}"),
            track_id,
            track_name: format!("Track {track_id}"),
            track_config: "Full".to_string(),
            clima: WeatherCondition::Dry,
            temperatura: 22.0,
            voltas: 20,
            duracao_corrida_min: 30,
            duracao_classificacao_min: 15,
            status: RaceStatus::Pendente,
            horario: "14:00".to_string(),
            week_of_year: rodada,
            season_phase: SeasonPhase::BlocoRegular,
            display_date: "2026-01-01".to_string(),
            thematic_slot: ThematicSlot::NaoClassificado,
        }
    }

    #[test]
    fn strong_rich_team_prefers_balanced_on_mixed_calendar() {
        let target = sample_team("T001", "gt4", 12.0, 85.0);
        let peers = vec![
            target.clone(),
            sample_team("T002", "gt4", 10.0, 65.0),
            sample_team("T003", "gt4", 8.0, 55.0),
            sample_team("T004", "gt4", 6.0, 40.0),
        ];
        let calendar = vec![
            sample_calendar_entry("R1", "S002", "gt4", 1, 238),
            sample_calendar_entry("R2", "S002", "gt4", 2, 212),
            sample_calendar_entry("R3", "S002", "gt4", 3, 455),
            sample_calendar_entry("R4", "S002", "gt4", 4, 164),
        ];

        let profile = choose_car_build_profile(&target, &peers, &calendar);
        assert_eq!(profile, CarBuildProfile::Balanced);
    }

    #[test]
    fn rich_team_prefers_balanced_even_when_legacy_budget_is_low() {
        let mut target = sample_team("T001", "gt4", 12.0, 1.0);
        target.cash_balance = 8_000_000.0;
        target.debt_balance = 0.0;
        target.financial_state = "healthy".to_string();
        target.reputacao = 80.0;
        let peers = vec![
            target.clone(),
            sample_team("T002", "gt4", 10.0, 65.0),
            sample_team("T003", "gt4", 8.0, 55.0),
            sample_team("T004", "gt4", 6.0, 40.0),
        ];
        let calendar = vec![
            sample_calendar_entry("R1", "S002", "gt4", 1, 238),
            sample_calendar_entry("R2", "S002", "gt4", 2, 212),
            sample_calendar_entry("R3", "S002", "gt4", 3, 455),
            sample_calendar_entry("R4", "S002", "gt4", 4, 164),
        ];

        let profile = choose_car_build_profile(&target, &peers, &calendar);

        assert_eq!(profile, CarBuildProfile::Balanced);
    }

    #[test]
    fn affordability_bias_uses_real_money_not_legacy_budget() {
        let mut rich = sample_team("TRICH", "gt4", 10.0, 1.0);
        rich.cash_balance = 8_000_000.0;
        rich.debt_balance = 0.0;
        rich.financial_state = "healthy".to_string();

        let mut poor = sample_team("TPOOR", "gt4", 10.0, 99.0);
        poor.cash_balance = 100_000.0;
        poor.debt_balance = 2_500_000.0;
        poor.financial_state = "crisis".to_string();

        assert!(
            affordability_bias(&rich, CarBuildProfile::Balanced)
                > affordability_bias(&poor, CarBuildProfile::Balanced)
        );
    }

    #[test]
    fn poor_bottom_team_prefers_power_specialization_on_power_calendar() {
        let target = sample_team("T004", "gt4", 4.0, 18.0);
        let peers = vec![
            sample_team("T001", "gt4", 12.0, 85.0),
            sample_team("T002", "gt4", 10.0, 65.0),
            sample_team("T003", "gt4", 8.0, 55.0),
            target.clone(),
        ];
        let calendar = vec![
            sample_calendar_entry("R1", "S002", "gt4", 1, 93),
            sample_calendar_entry("R2", "S002", "gt4", 2, 287),
            sample_calendar_entry("R3", "S002", "gt4", 3, 188),
            sample_calendar_entry("R4", "S002", "gt4", 4, 397),
        ];

        let profile = choose_car_build_profile(&target, &peers, &calendar);
        assert!(matches!(
            profile,
            CarBuildProfile::PowerIntermediate | CarBuildProfile::PowerExtreme
        ));
    }

    #[test]
    fn poor_team_avoids_expensive_balanced_profile_even_with_high_legacy_budget() {
        let mut target = sample_team("T004", "gt4", 4.0, 99.0);
        target.cash_balance = 100_000.0;
        target.debt_balance = 2_500_000.0;
        target.financial_state = "crisis".to_string();
        let peers = vec![
            sample_team("T001", "gt4", 12.0, 85.0),
            sample_team("T002", "gt4", 10.0, 65.0),
            sample_team("T003", "gt4", 8.0, 55.0),
            target.clone(),
        ];
        let calendar = vec![
            sample_calendar_entry("R1", "S002", "gt4", 1, 93),
            sample_calendar_entry("R2", "S002", "gt4", 2, 287),
            sample_calendar_entry("R3", "S002", "gt4", 3, 188),
            sample_calendar_entry("R4", "S002", "gt4", 4, 397),
        ];

        let profile = choose_car_build_profile(&target, &peers, &calendar);

        assert_ne!(profile, CarBuildProfile::Balanced);
    }

    #[test]
    fn calendar_fit_score_is_normalized_by_calendar_length() {
        let short_calendar = vec![
            sample_calendar_entry("R1", "S002", "gt4", 1, 93),
            sample_calendar_entry("R2", "S002", "gt4", 2, 287),
            sample_calendar_entry("R3", "S002", "gt4", 3, 188),
            sample_calendar_entry("R4", "S002", "gt4", 4, 397),
        ];
        let mut long_calendar = short_calendar.clone();
        long_calendar.extend(short_calendar.iter().cloned().enumerate().map(
            |(index, mut entry)| {
                entry.id = format!("R{}", index + 10);
                entry.rodada += 4;
                entry
            },
        ));

        let short_score = calendar_fit_score(&short_calendar, CarBuildProfile::PowerExtreme);
        let long_score = calendar_fit_score(&long_calendar, CarBuildProfile::PowerExtreme);

        assert!(
            (short_score - long_score).abs() < 0.0001,
            "calendar normalization should keep scores stable: short={short_score}, long={long_score}"
        );
    }

    #[test]
    fn competitive_context_changes_smoothly_for_close_front_teams() {
        let leader = sample_team("T001", "gt4", 12.0, 80.0);
        let contender = sample_team("T002", "gt4", 11.6, 78.0);
        let peers = vec![
            leader.clone(),
            contender.clone(),
            sample_team("T003", "gt4", 8.0, 55.0),
            sample_team("T004", "gt4", 5.5, 30.0),
        ];

        let leader_context = build_competitive_context(&leader, &peers);
        let contender_context = build_competitive_context(&contender, &peers);

        assert!(leader_context.title_pressure > contender_context.title_pressure);
        assert!(contender_context.title_pressure > 0.0);
        assert!(
            (leader_context.title_pressure - contender_context.title_pressure) < 0.35,
            "close front-runners should not have a huge strategic gap: leader={:?}, contender={:?}",
            leader_context,
            contender_context
        );
    }

    #[test]
    fn performance_percentile_uses_strength_gap_not_grid_position_only() {
        let leader = sample_team("T001", "gt4", 12.0, 80.0);
        let contender = sample_team("T002", "gt4", 11.9, 78.0);
        let midfield = sample_team("T003", "gt4", 7.0, 55.0);
        let backmarker = sample_team("T004", "gt4", 4.0, 30.0);
        let peers = vec![leader.clone(), contender.clone(), midfield, backmarker];

        let leader_percentile = performance_percentile(&leader, &peers);
        let contender_percentile = performance_percentile(&contender, &peers);

        assert!(leader_percentile < 0.05);
        assert!(contender_percentile < 0.1);
        assert!(
            (contender_percentile - leader_percentile) < 0.05,
            "very similar cars should stay close in percentile: leader={leader_percentile}, contender={contender_percentile}"
        );
    }
}
