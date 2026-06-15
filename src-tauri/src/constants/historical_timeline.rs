#![allow(dead_code)]

use crate::models::team::Team;

pub fn category_start_year(category_id: &str) -> i32 {
    match category_id {
        "gt3" => 1999,
        "endurance" => 2004,
        "lmp2" => 2004,
        "gt4" => 2002,
        "toyota_amador" => 2012,
        "bmw_m2" => 2015,
        "mazda_amador" => 2016,
        "production_challenger" => 2018,
        "mazda_rookie" => 2020,
        "toyota_rookie" => 2020,
        _ => 2000,
    }
}

pub fn is_category_active_in_year(category_id: &str, year: i32) -> bool {
    year >= category_start_year(category_id)
}

pub fn class_start_year(category_id: &str, class_id: Option<&str>) -> i32 {
    match (category_id, class_id) {
        ("endurance", Some("gt3")) => 2005,
        ("endurance", Some("gt4")) => 2007,
        ("endurance", Some("lmp2")) => 2004,
        ("production_challenger", Some("mazda" | "toyota" | "bmw")) => 2018,
        _ => category_start_year(category_id),
    }
}

pub fn is_team_active_in_year(team: &Team, year: i32) -> bool {
    team.ativa && year >= team_start_year(team)
}

pub fn team_start_year(team: &Team) -> i32 {
    team.ano_fundacao
        .max(category_start_year(&team.categoria))
        .max(class_start_year(&team.categoria, team.classe.as_deref()))
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
    if rank_index < 5 {
        return base_year;
    }

    let max_offset = match category_id {
        "mazda_rookie" | "toyota_rookie" | "mazda_amador" | "toyota_amador" => 4,
        _ => 6,
    };
    let remaining_rank = rank_index.saturating_sub(5);
    let remaining_total = total_teams.saturating_sub(5).max(1);
    let denominator = remaining_total.saturating_sub(1).max(1) as f64;
    let rank_ratio = remaining_rank as f64 / denominator;
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
    fn class_start_year_separates_category_and_multiclass_entry_dates() {
        assert_eq!(class_start_year("endurance", Some("gt3")), 2005);
        assert_eq!(class_start_year("endurance", Some("gt4")), 2007);
        assert_eq!(class_start_year("endurance", Some("lmp2")), 2004);
        assert_eq!(
            class_start_year("production_challenger", Some("mazda")),
            2018
        );
        assert_eq!(
            class_start_year("production_challenger", Some("toyota")),
            2018
        );
        assert_eq!(class_start_year("production_challenger", Some("bmw")), 2018);
        assert_eq!(class_start_year("gt3", None), 1999);
        assert_eq!(class_start_year("gt4", None), 2002);
        assert_eq!(class_start_year("lmp2", None), 2004);
        assert_eq!(category_start_year("endurance"), 2004);
    }

    #[test]
    fn team_activity_uses_foundation_category_and_class_start_years() {
        let gt3 = test_team("gt3", None, 1990);
        assert!(!is_team_active_in_year(&gt3, 1998));
        assert!(is_team_active_in_year(&gt3, 1999));

        let endurance_gt3 = test_team("endurance", Some("gt3"), 1990);
        assert!(!is_team_active_in_year(&endurance_gt3, 2004));
        assert!(is_team_active_in_year(&endurance_gt3, 2005));

        let gt4 = test_team("gt4", None, 1990);
        assert!(!is_team_active_in_year(&gt4, 2001));
        assert!(is_team_active_in_year(&gt4, 2002));

        let endurance_gt4 = test_team("endurance", Some("gt4"), 1990);
        assert!(!is_team_active_in_year(&endurance_gt4, 2006));
        assert!(is_team_active_in_year(&endurance_gt4, 2007));

        let lmp2 = test_team("lmp2", None, 1990);
        assert!(!is_team_active_in_year(&lmp2, 2003));
        assert!(is_team_active_in_year(&lmp2, 2004));

        let endurance_lmp2 = test_team("endurance", Some("lmp2"), 1990);
        assert!(!is_team_active_in_year(&endurance_lmp2, 2003));
        assert!(is_team_active_in_year(&endurance_lmp2, 2004));

        let production_mazda = test_team("production_challenger", Some("mazda"), 1990);
        assert!(!is_team_active_in_year(&production_mazda, 2017));
        assert!(is_team_active_in_year(&production_mazda, 2018));
    }

    #[test]
    fn team_foundation_can_delay_activity_after_category_and_class_exist() {
        let late_gt3 = test_team("endurance", Some("gt3"), 2008);
        assert!(!is_team_active_in_year(&late_gt3, 2007));
        assert!(is_team_active_in_year(&late_gt3, 2008));
    }

    fn test_team(category: &str, class_name: Option<&str>, foundation_year: i32) -> Team {
        let mut team = crate::models::team::placeholder_team_from_db(
            "T_TEST".to_string(),
            "Timeline Test".to_string(),
            category.to_string(),
            "now".to_string(),
        );
        team.ano_fundacao = foundation_year;
        team.classe = class_name.map(str::to_string);
        team
    }

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
