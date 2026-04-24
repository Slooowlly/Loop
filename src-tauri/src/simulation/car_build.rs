use serde::{Deserialize, Serialize};

use crate::simulation::track_profile::BALANCED_CAR_WEIGHTS;

pub type CarAttributeWeights = (f64, f64, f64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CarBuildProfile {
    Balanced,
    AccelerationIntermediate,
    PowerIntermediate,
    HandlingIntermediate,
    AccelerationExtreme,
    PowerExtreme,
    HandlingExtreme,
}

impl CarBuildProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            CarBuildProfile::Balanced => "balanced",
            CarBuildProfile::AccelerationIntermediate => "acceleration_intermediate",
            CarBuildProfile::PowerIntermediate => "power_intermediate",
            CarBuildProfile::HandlingIntermediate => "handling_intermediate",
            CarBuildProfile::AccelerationExtreme => "acceleration_extreme",
            CarBuildProfile::PowerExtreme => "power_extreme",
            CarBuildProfile::HandlingExtreme => "handling_extreme",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "acceleration_intermediate" => Self::AccelerationIntermediate,
            "power_intermediate" => Self::PowerIntermediate,
            "handling_intermediate" => Self::HandlingIntermediate,
            "acceleration_extreme" => Self::AccelerationExtreme,
            "power_extreme" => Self::PowerExtreme,
            "handling_extreme" => Self::HandlingExtreme,
            _ => Self::Balanced,
        }
    }

    pub fn from_str_strict(value: &str) -> Result<Self, String> {
        match value.trim().to_lowercase().as_str() {
            "balanced" => Ok(Self::Balanced),
            "acceleration_intermediate" => Ok(Self::AccelerationIntermediate),
            "power_intermediate" => Ok(Self::PowerIntermediate),
            "handling_intermediate" => Ok(Self::HandlingIntermediate),
            "acceleration_extreme" => Ok(Self::AccelerationExtreme),
            "power_extreme" => Ok(Self::PowerExtreme),
            "handling_extreme" => Ok(Self::HandlingExtreme),
            other => Err(format!("CarBuildProfile invalido: '{other}'")),
        }
    }
}

pub fn weights_for_profile(profile: CarBuildProfile) -> CarAttributeWeights {
    match profile {
        CarBuildProfile::Balanced => BALANCED_CAR_WEIGHTS,
        CarBuildProfile::AccelerationIntermediate => (47.0, 26.5, 26.5),
        CarBuildProfile::PowerIntermediate => (26.5, 47.0, 26.5),
        CarBuildProfile::HandlingIntermediate => (26.5, 26.5, 47.0),
        CarBuildProfile::AccelerationExtreme => (60.0, 20.0, 20.0),
        CarBuildProfile::PowerExtreme => (20.0, 60.0, 20.0),
        CarBuildProfile::HandlingExtreme => (20.0, 20.0, 60.0),
    }
}

pub fn profile_cost_multiplier(profile: CarBuildProfile) -> f64 {
    match profile {
        CarBuildProfile::Balanced => 1.20,
        CarBuildProfile::AccelerationIntermediate
        | CarBuildProfile::PowerIntermediate
        | CarBuildProfile::HandlingIntermediate => 1.0,
        CarBuildProfile::AccelerationExtreme
        | CarBuildProfile::PowerExtreme
        | CarBuildProfile::HandlingExtreme => 0.85,
    }
}

pub fn profile_budget_cost(profile: CarBuildProfile) -> f64 {
    match profile {
        CarBuildProfile::Balanced => 12.0,
        CarBuildProfile::AccelerationIntermediate
        | CarBuildProfile::PowerIntermediate
        | CarBuildProfile::HandlingIntermediate => 7.0,
        CarBuildProfile::AccelerationExtreme
        | CarBuildProfile::PowerExtreme
        | CarBuildProfile::HandlingExtreme => 3.0,
    }
}

pub fn profile_money_cost_multiplier(profile: CarBuildProfile) -> f64 {
    match profile {
        CarBuildProfile::Balanced => 1.25,
        CarBuildProfile::AccelerationIntermediate
        | CarBuildProfile::PowerIntermediate
        | CarBuildProfile::HandlingIntermediate => 1.05,
        CarBuildProfile::AccelerationExtreme
        | CarBuildProfile::PowerExtreme
        | CarBuildProfile::HandlingExtreme => 0.85,
    }
}

pub fn profile_money_cost(profile: CarBuildProfile, category_car_cost_unit: f64) -> f64 {
    category_car_cost_unit.max(0.0) * profile_money_cost_multiplier(profile)
}

pub fn dot_match_score(weights: CarAttributeWeights, track_weights: CarAttributeWeights) -> f64 {
    let (team_acc, team_power, team_handling) = weights;
    let (track_acc, track_power, track_handling) = track_weights;
    (team_acc * track_acc + team_power * track_power + team_handling * track_handling) / 100.0
}

pub fn balanced_match_score(track_weights: CarAttributeWeights) -> f64 {
    dot_match_score(BALANCED_CAR_WEIGHTS, track_weights)
}

pub fn track_advantage(profile: CarBuildProfile, track_weights: CarAttributeWeights) -> f64 {
    let team_match = dot_match_score(weights_for_profile(profile), track_weights);
    team_match - balanced_match_score(track_weights)
}

pub fn track_delta(profile: CarBuildProfile, track_weights: CarAttributeWeights) -> f64 {
    (track_advantage(profile, track_weights) / 2.5).clamp(-6.0, 6.0)
}

pub fn effective_car_performance(
    base_car_performance: f64,
    profile: CarBuildProfile,
    track_weights: CarAttributeWeights,
) -> f64 {
    base_car_performance + track_delta(profile, track_weights)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_profile_is_neutral_on_any_track() {
        let delta = track_delta(CarBuildProfile::Balanced, (50.0, 10.0, 40.0));
        assert!(
            delta.abs() < 0.0001,
            "balanced delta should be neutral, got {delta}"
        );
    }

    #[test]
    fn matching_acceleration_profile_gets_positive_delta() {
        let delta = track_delta(CarBuildProfile::AccelerationExtreme, (50.0, 10.0, 40.0));
        assert!(delta > 0.0, "expected positive delta, got {delta}");
    }

    #[test]
    fn wrong_power_profile_gets_negative_delta_on_tsukuba() {
        let delta = track_delta(CarBuildProfile::PowerExtreme, (50.0, 10.0, 40.0));
        assert!(delta < 0.0, "expected negative delta, got {delta}");
    }

    #[test]
    fn delta_is_clamped_to_positive_six() {
        let delta = track_delta(CarBuildProfile::PowerExtreme, (0.0, 1000.0, 0.0));
        assert_eq!(delta, 6.0);
    }

    #[test]
    fn delta_is_clamped_to_negative_six() {
        let delta = track_delta(CarBuildProfile::PowerExtreme, (1000.0, 0.0, 0.0));
        assert_eq!(delta, -6.0);
    }

    #[test]
    fn acceleration_extreme_beats_power_extreme_on_accel_track() {
        let accel = track_delta(CarBuildProfile::AccelerationExtreme, (50.0, 10.0, 40.0));
        let power = track_delta(CarBuildProfile::PowerExtreme, (50.0, 10.0, 40.0));
        assert!(accel > power, "expected accel {accel} > power {power}");
    }

    #[test]
    fn balanced_profile_costs_more_money_than_extreme_profile() {
        let category_unit = 1_000_000.0;

        assert!(
            profile_money_cost(CarBuildProfile::Balanced, category_unit)
                > profile_money_cost(CarBuildProfile::AccelerationExtreme, category_unit)
        );
    }
}
