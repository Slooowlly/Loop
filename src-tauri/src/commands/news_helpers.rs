use chrono::{Datelike, Duration, NaiveDate};

use crate::commands::news_tab::NewsTabContext;
use crate::news::{NewsImportance, NewsItem, NewsType};
use crate::public_presence::team::TeamPublicPresenceTier;

pub(crate) fn scope_class_label(class_name: &str) -> &'static str {
    match class_name {
        "mazda" => "Mazda",
        "toyota" => "Toyota",
        "bmw" => "BMW",
        "gt4" => "GT4",
        "gt3" => "GT3",
        "lmp2" => "LMP2",
        _ => "Classe",
    }
}

pub(crate) fn importance_rank(value: &NewsImportance) -> i32 {
    match value {
        NewsImportance::Destaque => 4,
        NewsImportance::Alta => 3,
        NewsImportance::Media => 2,
        NewsImportance::Baixa => 1,
    }
}

pub(crate) fn importance_label(value: &NewsImportance) -> &'static str {
    match value {
        NewsImportance::Destaque => "Destaque",
        NewsImportance::Alta => "Alta",
        NewsImportance::Media => "Media",
        NewsImportance::Baixa => "Baixa",
    }
}

pub(crate) fn story_accent(importance: &NewsImportance, news_type: &NewsType) -> &'static str {
    match importance {
        NewsImportance::Destaque => "gold",
        NewsImportance::Alta => "blue",
        _ if matches!(news_type, NewsType::Mercado) => "warm",
        _ => "steel",
    }
}

pub(crate) fn story_sort_key(item: &NewsItem) -> (i32, i64) {
    (importance_rank(&item.importancia), item.timestamp)
}

pub(crate) fn freshness_bonus(newest_timestamp: i64, timestamp: i64) -> i32 {
    let delta = newest_timestamp.saturating_sub(timestamp);
    let bucket = (delta / 10).clamp(0, 24) as i32;
    24 - bucket
}

pub(crate) fn team_presence_label(value: &TeamPublicPresenceTier) -> &'static str {
    match value {
        TeamPublicPresenceTier::Elite => "elite",
        TeamPublicPresenceTier::Alta => "alta",
        TeamPublicPresenceTier::Relevante => "relevante",
        TeamPublicPresenceTier::Baixa => "baixa",
    }
}

pub(crate) fn team_color_pair(
    context: &NewsTabContext,
    team_id: &str,
) -> (Option<String>, Option<String>) {
    context
        .team_colors
        .get(team_id)
        .map(|(primary, secondary)| (Some(primary.clone()), Some(secondary.clone())))
        .unwrap_or((None, None))
}

pub(crate) fn build_meta_label(item: &NewsItem) -> String {
    let mut parts = vec![item.tipo.as_str().to_string()];
    if let Some(round) = item.rodada.filter(|value| *value > 0) {
        parts.push(format!("R{round}"));
    }
    parts.push(format!("T{}", item.temporada));
    parts.join(" · ")
}

pub(crate) fn build_story_time_label(context: &NewsTabContext, item: &NewsItem) -> String {
    if let Some(label) = build_round_time_label(context, item) {
        return label;
    }
    if let Some(label) = build_preseason_time_label(context, item) {
        return label;
    }
    format!(
        "Temporada {} · {}",
        item.temporada,
        infer_story_season_year(
            context.career.season.numero,
            context.career.season.ano,
            item.temporada,
        )
    )
}

pub(crate) fn infer_story_season_year(
    current_season_number: i32,
    current_year: i32,
    item_season_number: i32,
) -> i32 {
    if current_season_number <= 0 || item_season_number <= 0 {
        return current_year;
    }

    current_year + (item_season_number - current_season_number)
}

pub(crate) fn build_round_time_label(context: &NewsTabContext, item: &NewsItem) -> Option<String> {
    let category_id = item.categoria_id.as_deref()?;
    let round = item.rodada.filter(|value| *value > 0)?;
    let display_date = context.race_dates.get(&format!("{category_id}:{round}"))?;
    Some(format!(
        "Rodada {} · {}",
        round,
        format_display_date_label(display_date)?
    ))
}

pub(crate) fn build_preseason_time_label(
    context: &NewsTabContext,
    item: &NewsItem,
) -> Option<String> {
    let category_id = item.categoria_id.as_deref()?;
    let preseason_week = item.semana_pretemporada.filter(|value| *value > 0)?;
    let max_week = context
        .max_preseason_week_by_season
        .get(&item.temporada)
        .copied()
        .filter(|value| *value >= preseason_week)?;
    let first_round_date = context
        .race_dates
        .get(&format!("{category_id}:1"))
        .and_then(|value| parse_iso_date(value))?;
    let weeks_before_start = i64::from(max_week - preseason_week + 1);
    let editorial_date = first_round_date - Duration::weeks(weeks_before_start);
    Some(format!(
        "Pre-temporada Semana {} · {}",
        preseason_week,
        format_naive_date_label(editorial_date)
    ))
}

pub(crate) fn format_display_date_label(display_date: &str) -> Option<String> {
    Some(format_naive_date_label(parse_iso_date(display_date)?))
}

pub(crate) fn parse_iso_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

pub(crate) fn format_naive_date_label(date: NaiveDate) -> String {
    let month = match date.month() {
        1 => "jan",
        2 => "fev",
        3 => "mar",
        4 => "abr",
        5 => "mai",
        6 => "jun",
        7 => "jul",
        8 => "ago",
        9 => "set",
        10 => "out",
        11 => "nov",
        12 => "dez",
        _ => "jan",
    };
    format!("{:02} {} {}", date.day(), month, date.year())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_story_season_year_keeps_current_year_for_current_season() {
        assert_eq!(infer_story_season_year(3, 2026, 3), 2026);
    }

    #[test]
    fn test_infer_story_season_year_recovers_previous_season_year() {
        assert_eq!(infer_story_season_year(3, 2026, 1), 2024);
    }

    #[test]
    fn test_infer_story_season_year_falls_back_for_invalid_numbers() {
        assert_eq!(infer_story_season_year(0, 2026, 1), 2026);
        assert_eq!(infer_story_season_year(3, 2026, 0), 2026);
    }
}
