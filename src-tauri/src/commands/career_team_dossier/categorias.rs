//! Escada de categorias da equipe: janelas por categoria, o caminho percorrido
//! (promocao/rebaixamento) e o resumo de movimento.

use super::*;

/// Etapa interna por categoria: temporada e ano de início/fim.
pub(super) struct CategorySpan {
    pub(super) category: String,
    pub(super) start_season: i32,
    pub(super) start_year: i32,
    pub(super) end_year: i32,
}

/// Agrupa os fatos por categoria e ordena cronologicamente (por temporada de
/// estreia). Base compartilhada da escada (category_path) e do movimento.
pub(super) fn category_spans(facts: &[TeamRaceFact]) -> Vec<CategorySpan> {
    let mut by_category: BTreeMap<String, (i32, i32, i32, i32)> = BTreeMap::new();
    for fact in facts {
        by_category
            .entry(fact.category.clone())
            .and_modify(|(start, end, start_year, end_year)| {
                if fact.season_number < *start {
                    *start = fact.season_number;
                    *start_year = fact.season_year;
                }
                if fact.season_number > *end {
                    *end = fact.season_number;
                    *end_year = fact.season_year;
                }
            })
            .or_insert((
                fact.season_number,
                fact.season_number,
                fact.season_year,
                fact.season_year,
            ));
    }
    let mut spans: Vec<CategorySpan> = by_category
        .into_iter()
        .map(
            |(category, (start, _end, start_year, end_year))| CategorySpan {
                category,
                start_season: start,
                start_year,
                end_year,
            },
        )
        .collect();
    spans.sort_by_key(|span| span.start_season);
    spans
}

pub(super) fn build_real_category_path(facts: &[TeamRaceFact]) -> Vec<TeamHistoryCategoryStep> {
    let spans = category_spans(facts);
    let mut steps = Vec::new();
    let mut prev_tier: Option<u8> = None;
    for (index, span) in spans.iter().enumerate() {
        let tier = categories::get_category(&span.category).map(|config| config.tier);
        let movement = match (prev_tier, tier) {
            (None, _) => "start",
            (Some(prev), Some(current)) if current > prev => "promotion",
            (Some(prev), Some(current)) if current < prev => "relegation",
            _ => "same",
        };
        if tier.is_some() {
            prev_tier = tier;
        }
        let detail = match movement {
            "promotion" => "Promoção: subiu de categoria.".to_string(),
            "relegation" => "Rebaixamento: caiu de categoria.".to_string(),
            "start" => "Categoria de estreia da equipe.".to_string(),
            _ => "Permaneceu no mesmo nível.".to_string(),
        };
        let years = if span.start_year == span.end_year {
            span.start_year.to_string()
        } else {
            format!("{}-{}", span.start_year, span.end_year)
        };
        steps.push(TeamHistoryCategoryStep {
            category: team_history_category_label(&span.category),
            years,
            detail,
            color: history_palette(index),
            movement: movement.to_string(),
        });
    }
    steps
}

/// Resumo real de movimento entre categorias para a aba Categorias.
pub(super) fn build_team_movement(facts: &[TeamRaceFact]) -> TeamHistoryMovement {
    let spans = category_spans(facts);

    // Promoções / rebaixamentos a partir das transições de tier.
    let mut promotions = 0;
    let mut relegations = 0;
    let mut prev_tier: Option<u8> = None;
    for span in &spans {
        if let Some(tier) = categories::get_category(&span.category).map(|config| config.tier) {
            if let Some(prev) = prev_tier {
                if tier > prev {
                    promotions += 1;
                } else if tier < prev {
                    relegations += 1;
                }
            }
            prev_tier = Some(tier);
        }
    }

    // Tempo por categoria (em temporadas), pela contagem de temporadas distintas.
    let mut seasons_by_category: BTreeMap<String, std::collections::HashSet<i32>> = BTreeMap::new();
    let mut wins_by_category: BTreeMap<String, (i32, i32)> = BTreeMap::new(); // (vitórias, corridas)
    for fact in facts {
        seasons_by_category
            .entry(fact.category.clone())
            .or_default()
            .insert(fact.season_number);
        let entry = wins_by_category
            .entry(fact.category.clone())
            .or_insert((0, 0));
        if fact.win {
            entry.0 += 1;
        }
        entry.1 += 1;
    }

    let time_by_category = spans
        .iter()
        .map(|span| {
            let years = seasons_by_category
                .get(&span.category)
                .map(|set| set.len())
                .unwrap_or(0);
            format!(
                "{}: {} {}",
                team_history_category_label(&span.category),
                years,
                if years == 1 { "ano" } else { "anos" }
            )
        })
        .collect::<Vec<_>>()
        .join(" · ");

    // Melhor / mais difícil categoria por taxa de vitória (mín. de corridas).
    let mut best: Option<(String, f64)> = None;
    let mut hardest: Option<(String, f64)> = None;
    for (category, (wins, races)) in &wins_by_category {
        if *races < 3 {
            continue;
        }
        let rate = *wins as f64 / *races as f64;
        if best.as_ref().map(|(_, r)| rate > *r).unwrap_or(true) {
            best = Some((team_history_category_label(category), rate));
        }
        if hardest.as_ref().map(|(_, r)| rate < *r).unwrap_or(true) {
            hardest = Some((team_history_category_label(category), rate));
        }
    }

    TeamHistoryMovement {
        promotions,
        relegations,
        time_by_category,
        best_category: best.map(|(c, _)| c).unwrap_or_else(|| "—".to_string()),
        hardest_category: hardest.map(|(c, _)| c).unwrap_or_else(|| "—".to_string()),
    }
}
