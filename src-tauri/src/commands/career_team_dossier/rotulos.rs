//! Rotulos, formatadores e rankings usados so pelo dossie: agrupamento de categorias,
//! percentuais, ordinais e a paleta da linha do tempo.

use super::*;

pub(super) fn format_brl(value: f64) -> String {
    let rounded = value.round().max(0.0) as i64;
    let raw = rounded.to_string();
    let mut formatted = String::new();
    for (index, ch) in raw.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(ch);
    }
    let grouped: String = formatted.chars().rev().collect();
    format!("${grouped}")
}

pub(super) fn format_decimal_pt(value: f64, decimals: usize) -> String {
    format!("{value:.decimals$}").replace('.', ",")
}

pub(super) fn percentage(numerator: i32, denominator: i32) -> i32 {
    if denominator <= 0 {
        0
    } else {
        ((numerator as f64 / denominator as f64) * 100.0).round() as i32
    }
}

pub(super) fn rank_for_aggregate<F>(
    aggregates: &HashMap<String, TeamHistoryAggregate>,
    selected_team_id: &str,
    metric: F,
) -> String
where
    F: Fn(&TeamHistoryAggregate) -> f64,
{
    let mut ordered: Vec<(&String, f64)> = aggregates
        .iter()
        .map(|(team_id, aggregate)| (team_id, metric(aggregate)))
        .collect();
    ordered.sort_by(|(left_id, left), (right_id, right)| {
        right.total_cmp(left).then_with(|| left_id.cmp(right_id))
    });
    let rank = ordered
        .iter()
        .position(|(team_id, _)| team_id.as_str() == selected_team_id)
        .map(|index| index + 1)
        .unwrap_or(1);
    format_ordinal_i32(rank as i32)
}

pub(super) fn rank_for_i32(
    values: &HashMap<String, Vec<TeamTitleFact>>,
    selected_team_id: &str,
) -> String {
    let mut ordered: Vec<(String, i32)> = values
        .iter()
        .map(|(team_id, titles)| (team_id.clone(), titles.len() as i32))
        .collect();
    if !values.contains_key(selected_team_id) {
        ordered.push((selected_team_id.to_string(), 0));
    }
    ordered.sort_by(|(left_id, left), (right_id, right)| {
        right.cmp(left).then_with(|| left_id.cmp(right_id))
    });
    let rank = ordered
        .iter()
        .position(|(team_id, _)| team_id.as_str() == selected_team_id)
        .map(|index| index + 1)
        .unwrap_or(1);
    format_ordinal_i32(rank as i32)
}

fn format_ordinal_i32(value: i32) -> String {
    format!("{value}º")
}

pub(super) fn team_history_group_categories(category: &str) -> Vec<String> {
    match category {
        "mazda_rookie" | "mazda_amador" => {
            vec!["mazda_rookie".to_string(), "mazda_amador".to_string()]
        }
        "toyota_rookie" | "toyota_amador" => {
            vec!["toyota_rookie".to_string(), "toyota_amador".to_string()]
        }
        "bmw_m2" => vec!["bmw_m2".to_string()],
        "production_challenger" => vec![
            "mazda_rookie".to_string(),
            "mazda_amador".to_string(),
            "toyota_rookie".to_string(),
            "toyota_amador".to_string(),
            "bmw_m2".to_string(),
            "production_challenger".to_string(),
        ],
        "gt4" => vec!["gt4".to_string()],
        "gt3" => vec!["gt3".to_string()],
        "lmp2" => vec!["lmp2".to_string()],
        "endurance" => vec!["endurance".to_string()],
        other => vec![other.to_string()],
    }
}

pub(super) fn team_history_group_label(category: &str) -> String {
    let key = match category {
        "mazda_rookie" | "mazda_amador" => "mazda",
        "toyota_rookie" | "toyota_amador" => "toyota",
        "bmw_m2" => "bmw",
        "production_challenger" => "production",
        "gt4" => "gt4",
        "gt3" => "gt3",
        "lmp2" => "lmp2",
        "endurance" => "endurance",
        _ => "generic",
    };
    let full = format!("career.group.{key}");
    rust_i18n::t!(&full).to_string()
}

pub(super) fn team_history_category_label(category: &str) -> String {
    categories::get_category_config(category)
        .map(|config| config.nome_curto.to_string())
        .unwrap_or_else(|| category.to_string())
}

pub(super) fn history_palette(index: usize) -> String {
    ["#58a6ff", "#f2c46d", "#5ee7a8", "#ff6b6b"][index % 4].to_string()
}
