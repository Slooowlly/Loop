#![allow(dead_code)]

use super::dados::TEAMS;
use super::tipos::TeamTemplate;

pub fn get_team_templates(category_id: &str) -> Vec<&'static TeamTemplate> {
    TEAMS
        .iter()
        .filter(|team| team.categoria == category_id)
        .collect()
}

pub fn get_teams_for_category(category_id: &str) -> Vec<&'static TeamTemplate> {
    get_team_templates(category_id)
}

pub fn get_reference_team_template(
    category_id: &str,
    class_name: Option<&str>,
) -> Option<&'static TeamTemplate> {
    if let Some(template) = get_team_templates(category_id)
        .into_iter()
        .find(|team| class_name.is_none() || team.classe == class_name)
    {
        return Some(template);
    }

    let reference_category = match (category_id, class_name) {
        ("production_challenger", Some("mazda")) => Some("mazda_amador"),
        ("production_challenger", Some("toyota")) => Some("toyota_amador"),
        ("production_challenger", Some("bmw")) => Some("bmw_m2"),
        ("endurance", Some("gt4")) => Some("gt4"),
        ("endurance", Some("gt3")) => Some("gt3"),
        ("endurance", Some("lmp2")) => Some("lmp2"),
        _ => None,
    }?;

    get_team_templates(reference_category).into_iter().next()
}

pub fn get_all_team_templates() -> &'static [TeamTemplate] {
    TEAMS
}

pub fn count_teams() -> usize {
    TEAMS.len()
}

pub fn get_teams_by_endurance_class(classe: &str) -> Vec<&'static TeamTemplate> {
    TEAMS
        .iter()
        .filter(|team| team.categoria == "endurance" && team.classe == Some(classe))
        .collect()
}

pub fn get_teams_by_brand(marca: &str) -> Vec<&'static TeamTemplate> {
    TEAMS
        .iter()
        .filter(|team| team.marca == Some(marca))
        .collect()
}
