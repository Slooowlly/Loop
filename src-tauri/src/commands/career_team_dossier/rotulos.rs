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

/// Posição da equipe numa métrica, com o tamanho do grupo e a média dele.
///
/// O ordenamento já tinha os três — o que faltava era devolvê-los. A UI mostra
/// "10º de 24" e compara com a média porque um rank solto não diz se a equipe
/// está perto ou longe do pelotão.
pub(super) struct RecordRanking {
    pub ordinal: String,
    pub position: i32,
    pub total: i32,
    pub average: f64,
}

fn rank_from_values(values: Vec<(String, f64)>, selected_team_id: &str) -> RecordRanking {
    let mut ordered = values;
    ordered.sort_by(|(left_id, left), (right_id, right)| {
        right.total_cmp(left).then_with(|| left_id.cmp(right_id))
    });
    let total = ordered.len() as i32;
    let position = ordered
        .iter()
        .position(|(team_id, _)| team_id.as_str() == selected_team_id)
        .map(|index| index + 1)
        .unwrap_or(1) as i32;
    let average = if ordered.is_empty() {
        0.0
    } else {
        ordered.iter().map(|(_, value)| value).sum::<f64>() / ordered.len() as f64
    };
    RecordRanking {
        ordinal: format_ordinal_i32(position),
        position,
        total,
        average,
    }
}

/// Record de contagem (títulos, vitórias, pódios): a média do grupo sai com uma
/// casa decimal porque "2,1 vitórias" é a leitura honesta de um valor médio.
pub(super) fn count_record(
    id: &str,
    label: String,
    value: i32,
    ranking: RecordRanking,
) -> TeamHistoryRecord {
    TeamHistoryRecord {
        id: id.to_string(),
        label,
        rank: ranking.ordinal,
        value: value.to_string(),
        rank_position: ranking.position,
        rank_total: ranking.total,
        group_average: format_decimal_pt(ranking.average, 1),
    }
}

/// Record de taxa: a métrica ordenada é fração (0..1) e o valor exibido é
/// percentual inteiro — a média acompanha o formato do valor, senão a
/// comparação lado a lado fica ilegível.
pub(super) fn rate_record(
    id: &str,
    label: String,
    percent: i32,
    ranking: RecordRanking,
) -> TeamHistoryRecord {
    TeamHistoryRecord {
        id: id.to_string(),
        label,
        rank: ranking.ordinal,
        value: format!("{percent}%"),
        rank_position: ranking.position,
        rank_total: ranking.total,
        group_average: format!("{}%", (ranking.average * 100.0).round() as i32),
    }
}

pub(super) fn rank_for_aggregate<F>(
    aggregates: &HashMap<String, TeamHistoryAggregate>,
    selected_team_id: &str,
    metric: F,
) -> RecordRanking
where
    F: Fn(&TeamHistoryAggregate) -> f64,
{
    rank_from_values(
        aggregates
            .iter()
            .map(|(team_id, aggregate)| (team_id.clone(), metric(aggregate)))
            .collect(),
        selected_team_id,
    )
}

/// Ranking de títulos.
///
/// O universo é `aggregates` — TODA equipe que correu no grupo —, não o mapa de
/// títulos. `titles_by_team` só tem quem já ganhou algo; rankear por ele dizia
/// "10º de 10" (última entre as campeãs) enquanto vitórias e pódios diziam "de
/// 19", e inflava a média do grupo por não contar nenhum zero.
pub(super) fn rank_for_titles(
    titles_by_team: &HashMap<String, Vec<TeamTitleFact>>,
    aggregates: &HashMap<String, TeamHistoryAggregate>,
    selected_team_id: &str,
) -> RecordRanking {
    let mut ordered: Vec<(String, f64)> = aggregates
        .keys()
        .map(|team_id| {
            let titles = titles_by_team.get(team_id).map_or(0, |list| list.len());
            (team_id.clone(), titles as f64)
        })
        .collect();
    // Campeã que não aparece em `aggregates` (título arquivado sem corrida no
    // recorte) e a própria equipe consultada entram mesmo assim: ninguém pode
    // sumir do ranking em que está sendo comparado.
    for team_id in titles_by_team.keys() {
        if !aggregates.contains_key(team_id) {
            ordered.push((team_id.clone(), titles_by_team[team_id].len() as f64));
        }
    }
    if !ordered
        .iter()
        .any(|(team_id, _)| team_id == selected_team_id)
    {
        ordered.push((selected_team_id.to_string(), 0.0));
    }
    rank_from_values(ordered, selected_team_id)
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
