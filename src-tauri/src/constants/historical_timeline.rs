#![allow(dead_code)]

use crate::models::team::Team;

pub fn category_start_year(category_id: &str) -> i32 {
    match category_id {
        "gt3" => 1999,
        "endurance" => 2000,
        "gt4" => 2002,
        "toyota_amador" => 2012,
        "bmw_m2" => 2015,
        "mazda_amador" => 2016,
        "production_challenger" => 2018,
        "mazda_rookie" => 2020,
        "toyota_rookie" => 2021,
        _ => 2000,
    }
}

pub fn is_category_active_in_year(category_id: &str, year: i32) -> bool {
    year >= category_start_year(category_id)
}

pub fn is_team_active_in_year(team: &Team, year: i32) -> bool {
    team.ativa && year >= team.ano_fundacao && is_category_active_in_year(&team.categoria, year)
}

pub fn historical_team_foundation_year(
    team_name: &str,
    category_id: &str,
    rank_index: usize,
    total_teams: usize,
) -> i32 {
    if let Some(year) = known_team_foundation_year(team_name) {
        return year;
    }

    let base_year = category_start_year(category_id);
    let max_offset = match category_id {
        "mazda_rookie" | "toyota_rookie" | "mazda_amador" | "toyota_amador" => 4,
        _ => 6,
    };
    let denominator = total_teams.saturating_sub(1).max(1) as f64;
    let rank_ratio = rank_index as f64 / denominator;
    base_year + (rank_ratio * f64::from(max_offset)).round() as i32
}

pub fn historical_team_performance_band(team_name: &str, category_id: &str) -> Option<(f64, f64)> {
    if category_id != "gt3" {
        return None;
    }

    let normalized = normalize_team_name(team_name);
    if normalized.contains("mercedes-amg") {
        Some((14.8, 16.0))
    } else if normalized.contains("porsche") {
        Some((14.4, 16.0))
    } else if normalized.contains("ferrari") {
        Some((14.3, 16.0))
    } else if normalized.contains("lamborghini") {
        Some((13.9, 15.8))
    } else if normalized.contains("mclaren") {
        Some((13.8, 15.7))
    } else if normalized.contains("bmw") {
        Some((10.5, 13.2))
    } else if normalized.contains("audi") {
        Some((9.0, 12.3))
    } else if normalized.contains("aston martin") {
        Some((8.8, 12.0))
    } else if normalized.contains("chevrolet") {
        Some((7.0, 10.5))
    } else if normalized.contains("ford mustang") {
        Some((6.5, 10.0))
    } else if normalized.contains("acura") {
        Some((4.5, 8.5))
    } else {
        None
    }
}

pub fn apply_historical_performance_band(team: &mut Team) {
    if let Some((min, max)) = historical_team_performance_band(&team.nome, &team.categoria) {
        team.car_performance = team.car_performance.clamp(min, max);
    }
}

fn known_team_foundation_year(team_name: &str) -> Option<i32> {
    let normalized = normalize_team_name(team_name);
    let known = [
        ("ferrari", 1929),
        ("porsche", 1931),
        ("ford mustang", 1903),
        ("chevrolet", 1911),
        ("bmw", 1916),
        ("mercedes-amg", 1967),
        ("lamborghini", 1963),
        ("mclaren", 1963),
        ("acura", 1986),
        ("aston martin", 1913),
        ("audi", 1909),
    ];

    known
        .iter()
        .find(|(name, _)| normalized.contains(*name))
        .map(|(_, year)| *year)
}

fn normalize_team_name(team_name: &str) -> String {
    team_name.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::teams::get_team_templates;
    use crate::models::team::Team;
    use rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn gt3_heritage_teams_have_protected_historical_performance_bands() {
        assert_eq!(
            historical_team_performance_band("Ferrari", "gt3"),
            Some((14.3, 16.0))
        );
        assert_eq!(
            historical_team_performance_band("Mercedes-AMG", "gt3"),
            Some((14.8, 16.0))
        );
        assert_eq!(
            historical_team_performance_band("Lamborghini", "gt3"),
            Some((13.9, 15.8))
        );
        assert_eq!(
            historical_team_performance_band("McLaren", "gt3"),
            Some((13.8, 15.7))
        );
    }

    #[test]
    fn gt3_acura_is_capped_below_heritage_manufacturers() {
        let acura = historical_team_performance_band("Acura", "gt3").expect("Acura band");
        let mclaren = historical_team_performance_band("McLaren", "gt3").expect("McLaren band");

        assert!(acura.1 < mclaren.0);
    }

    #[test]
    fn historical_performance_band_clamps_generated_gt3_team() {
        let template = get_team_templates("gt3")
            .into_iter()
            .find(|team| team.nome == "Acura")
            .expect("Acura template");
        let mut rng = StdRng::seed_from_u64(29);
        let mut team =
            Team::from_template_with_rng(template, "gt3", "T001".to_string(), 2000, &mut rng);
        team.car_performance = 14.0;

        apply_historical_performance_band(&mut team);

        assert_eq!(team.car_performance, 8.5);
    }
}
