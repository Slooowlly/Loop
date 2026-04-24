#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalEconomicHealth {
    Boom,
    Neutral,
    Recession,
}

pub fn economy_income_modifier(health: GlobalEconomicHealth) -> f64 {
    match health {
        GlobalEconomicHealth::Boom => 1.12,
        GlobalEconomicHealth::Neutral => 1.0,
        GlobalEconomicHealth::Recession => 0.9,
    }
}

pub fn economy_cost_modifier(health: GlobalEconomicHealth) -> f64 {
    match health {
        GlobalEconomicHealth::Boom => 1.08,
        GlobalEconomicHealth::Neutral => 1.0,
        GlobalEconomicHealth::Recession => 0.92,
    }
}

pub fn global_economic_health_for_season(season_number: i32) -> GlobalEconomicHealth {
    match season_number.rem_euclid(6) {
        0 => GlobalEconomicHealth::Recession,
        3 => GlobalEconomicHealth::Boom,
        _ => GlobalEconomicHealth::Neutral,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_economy_modifier_defaults_to_one() {
        assert_eq!(economy_income_modifier(GlobalEconomicHealth::Neutral), 1.0);
    }

    #[test]
    fn recession_reduces_costs_as_well() {
        assert!(economy_cost_modifier(GlobalEconomicHealth::Recession) < 1.0);
    }

    #[test]
    fn boom_and_recession_move_income_in_opposite_directions() {
        assert!(
            economy_income_modifier(GlobalEconomicHealth::Boom)
                > economy_income_modifier(GlobalEconomicHealth::Neutral)
        );
        assert!(
            economy_income_modifier(GlobalEconomicHealth::Recession)
                < economy_income_modifier(GlobalEconomicHealth::Neutral)
        );
    }

    #[test]
    fn season_cycle_produces_booms_and_recessions() {
        assert_eq!(
            global_economic_health_for_season(3),
            GlobalEconomicHealth::Boom
        );
        assert_eq!(
            global_economic_health_for_season(6),
            GlobalEconomicHealth::Recession
        );
        assert_eq!(
            global_economic_health_for_season(7),
            GlobalEconomicHealth::Neutral
        );
    }
}
