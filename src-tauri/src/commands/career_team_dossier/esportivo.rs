//! Recorte esportivo do dossie: marcos, superlativos, resultados temporada a
//! temporada, linha do tempo e sequencias (streaks).

use super::*;

/// Marcos cronológicos (primeira vitória, primeiro pódio, primeiro título).
pub(super) fn build_team_milestones(
    facts: &[TeamRaceFact],
    titles: &[TeamTitleFact],
) -> Vec<TeamHistoryMilestone> {
    let mut milestones = Vec::new();
    if let Some(year) = facts
        .iter()
        .filter(|f| f.podium)
        .map(|f| f.season_year)
        .min()
    {
        milestones.push(TeamHistoryMilestone {
            label: rust_i18n::t!("team_dossier.first_milestone.podium").to_string(),
            year: year.to_string(),
            kind: "first_podium".to_string(),
        });
    }
    if let Some(year) = facts.iter().filter(|f| f.win).map(|f| f.season_year).min() {
        milestones.push(TeamHistoryMilestone {
            label: rust_i18n::t!("team_dossier.first_milestone.win").to_string(),
            year: year.to_string(),
            kind: "first_win".to_string(),
        });
    }
    if let Some(year) = titles.iter().map(|t| t.season_year).min() {
        milestones.push(TeamHistoryMilestone {
            label: rust_i18n::t!("team_dossier.first_milestone.title").to_string(),
            year: year.to_string(),
            kind: "first_title".to_string(),
        });
    }
    milestones
}

/// Quantas corridas a fita de forma recente carrega. Dez cabe numa linha sem
/// virar quadrado de 8px, e cobre pouco mais de uma temporada — o bastante para
/// uma troca de categoria recente aparecer inteira.
const FORM_RACES: usize = 10;

/// As últimas corridas, da mais antiga para a mais nova. `facts` já vem ordenado
/// por temporada e rodada.
pub(super) fn build_team_recent_form(facts: &[TeamRaceFact]) -> Vec<TeamHistoryFormRace> {
    facts
        .iter()
        .rev()
        .take(FORM_RACES)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|fact| TeamHistoryFormRace {
            year: fact.season_year.to_string(),
            round: fact.round,
            category: team_history_category_label(&fact.category),
            category_id: fact.category.clone(),
            position: fact.best_position,
        })
        .collect()
}

/// Distribuição das corridas por faixa de colocação. Corrida sem posição
/// registrada cai em `outside` junto com o resto de fora do top 10: para quem lê
/// a barra, "não pontuou" e "não terminou" são o mesmo fim de semana.
pub(super) fn build_team_result_spread(facts: &[TeamRaceFact]) -> TeamHistoryResultSpread {
    let mut spread = TeamHistoryResultSpread::default();
    for fact in facts {
        spread.races += 1;
        match fact.best_position {
            Some(1) => spread.first += 1,
            Some(2..=3) => spread.podium += 1,
            Some(4..=5) => spread.near_miss += 1,
            Some(6..=10) => spread.top_ten += 1,
            _ => spread.outside += 1,
        }
    }
    spread
}

/// Resultados temporada a temporada (ano, categoria dominante, vitórias, pódios,
/// pontos) — base da aba Esportivo, em ordem cronológica.
pub(super) fn build_team_season_results(
    facts: &[TeamRaceFact],
    positions: &HashMap<i32, i32>,
) -> Vec<TeamHistorySeasonResult> {
    use std::collections::BTreeMap;

    // Acumulador por temporada. Virou struct quando os degraus do pódio entraram:
    // uma tupla de oito campos posicionais é convite a erro na primeira mudança.
    #[derive(Default)]
    struct Acumulado {
        year: i32,
        wins: i32,
        seconds: i32,
        thirds: i32,
        fourths: i32,
        fifths: i32,
        podiums: i32,
        points: f64,
        races: i32,
        categories: HashMap<String, i32>,
    }

    let mut by_season: BTreeMap<i32, Acumulado> = BTreeMap::new();
    for fact in facts {
        let entry = by_season.entry(fact.season_number).or_default();
        entry.year = fact.season_year;
        if fact.podium {
            entry.podiums += 1;
        }
        // As colocações são EXCLUSIVAS entre si: cada corrida entra uma única
        // vez, pelo melhor carro da equipe. Sem isso, uma dobradinha 1º-3º
        // contaria duas colocações na mesma corrida. O contador de pódios acima
        // segue independente — ele é o total de top 3, usado em outras telas.
        match fact.best_position {
            Some(1) => entry.wins += 1,
            Some(2) => entry.seconds += 1,
            Some(3) => entry.thirds += 1,
            Some(4) => entry.fourths += 1,
            Some(5) => entry.fifths += 1,
            _ => {}
        }
        entry.points += fact.points;
        entry.races += 1;
        *entry.categories.entry(fact.category.clone()).or_insert(0) += 1;
    }

    by_season
        .into_iter()
        .map(|(season_number, season)| {
            let category_id = season
                .categories
                .iter()
                .max_by_key(|(_, races)| **races)
                .map(|(cat, _)| cat.clone())
                .unwrap_or_default();
            let category = if category_id.is_empty() {
                String::new()
            } else {
                team_history_category_label(&category_id)
            };
            let position = positions
                .get(&season_number)
                .map(|pos| format!("P{pos}"))
                .unwrap_or_else(|| "—".to_string());
            TeamHistorySeasonResult {
                year: season.year.to_string(),
                category,
                category_id,
                position,
                wins: season.wins,
                podiums: season.podiums,
                points: format!("{}", season.points.round() as i64),
                races: season.races,
                seconds: season.seconds,
                thirds: season.thirds,
                fourths: season.fourths,
                fifths: season.fifths,
            }
        })
        .collect()
}

/// Superlativos da equipe a partir do histórico real: melhor temporada (vitórias),
/// pico de pódios numa temporada e maior sequência de títulos consecutivos.
pub(super) fn build_team_highlights(
    facts: &[TeamRaceFact],
    titles: &[TeamTitleFact],
    positions: &HashMap<i32, i32>,
) -> Vec<TeamHistoryHighlight> {
    use std::collections::BTreeMap;

    // Agrega por temporada: (ano, vitórias, pódios, categoria→corridas).
    let mut by_season: BTreeMap<i32, (i32, i32, i32, HashMap<String, i32>)> = BTreeMap::new();
    for fact in facts {
        let entry = by_season
            .entry(fact.season_number)
            .or_insert_with(|| (fact.season_year, 0, 0, HashMap::new()));
        entry.0 = fact.season_year;
        if fact.win {
            entry.1 += 1;
        }
        if fact.podium {
            entry.2 += 1;
        }
        *entry.3.entry(fact.category.clone()).or_insert(0) += 1;
    }

    let dominant_category = |cats: &HashMap<String, i32>| -> String {
        cats.iter()
            .max_by_key(|(_, races)| **races)
            .map(|(cat, _)| team_history_category_label(cat))
            .unwrap_or_default()
    };

    let mut highlights = Vec::new();

    // Melhor temporada por vitórias.
    if let Some((_, (year, wins, _, cats))) = by_season.iter().max_by_key(|(_, v)| v.1) {
        if *wins > 0 {
            highlights.push(TeamHistoryHighlight {
                label: rust_i18n::t!("team_dossier.highlight.best_season").to_string(),
                value: rust_i18n::t!("team_dossier.highlight.best_season_value", count = wins)
                    .to_string(),
                detail: rust_i18n::t!(
                    "team_dossier.highlight.detail_year_category",
                    year = year,
                    category = dominant_category(cats)
                )
                .to_string(),
            });
        }
    }

    // Pico de pódios numa temporada.
    if let Some((_, (year, _, podiums, cats))) = by_season.iter().max_by_key(|(_, v)| v.2) {
        if *podiums > 0 {
            highlights.push(TeamHistoryHighlight {
                label: rust_i18n::t!("team_dossier.highlight.most_podiums").to_string(),
                value: rust_i18n::t!("team_dossier.highlight.most_podiums_value", count = podiums)
                    .to_string(),
                detail: rust_i18n::t!(
                    "team_dossier.highlight.detail_year_category",
                    year = year,
                    category = dominant_category(cats)
                )
                .to_string(),
            });
        }
    }

    // Maior sequência de títulos consecutivos.
    let mut years: Vec<i32> = titles.iter().map(|title| title.season_year).collect();
    years.sort_unstable();
    years.dedup();
    let mut best_run = 0;
    let mut best_run_end = 0;
    let mut run = 0;
    let mut prev: Option<i32> = None;
    for year in &years {
        run = if prev == Some(year - 1) { run + 1 } else { 1 };
        if run > best_run {
            best_run = run;
            best_run_end = *year;
        }
        prev = Some(*year);
    }
    if best_run >= 2 {
        highlights.push(TeamHistoryHighlight {
            label: rust_i18n::t!("team_dossier.highlight.biggest_dynasty").to_string(),
            value: rust_i18n::t!(
                "team_dossier.highlight.biggest_dynasty_value",
                count = best_run
            )
            .to_string(),
            detail: rust_i18n::t!("team_dossier.highlight.detail_until", year = best_run_end)
                .to_string(),
        });
    }

    // Melhor campanha (menor posição final no campeonato).
    if let Some((season, position)) = positions.iter().min_by_key(|(_, pos)| **pos) {
        let year = by_season.get(season).map(|entry| entry.0).unwrap_or(0);
        let value = if *position == 1 {
            rust_i18n::t!("team_dossier.highlight.champion_value").to_string()
        } else {
            format!("P{position}")
        };
        highlights.push(TeamHistoryHighlight {
            label: rust_i18n::t!("team_dossier.highlight.best_campaign").to_string(),
            value,
            detail: rust_i18n::t!("team_dossier.highlight.detail_year", year = year).to_string(),
        });
    }

    highlights
}

pub(super) fn build_real_team_timeline(facts: &[TeamRaceFact]) -> Vec<TeamHistoryTimelineItem> {
    let Some(first) = facts.first() else {
        return vec![TeamHistoryTimelineItem {
            year: "-".to_string(),
            text: "Sem corridas registradas neste recorte.".to_string(),
            kind: "empty".to_string(),
        }];
    };
    let mut items = vec![TeamHistoryTimelineItem {
        year: first.season_year.to_string(),
        text: format!(
            "Primeira corrida registrada em {}, rodada {}.",
            team_history_category_label(&first.category),
            first.round
        ),
        kind: "first_race".to_string(),
    }];

    if let Some(first_win) = facts.iter().find(|fact| fact.win) {
        items.push(TeamHistoryTimelineItem {
            year: first_win.season_year.to_string(),
            text: format!(
                "Primeira vitória real em {}, rodada {}.",
                team_history_category_label(&first_win.category),
                first_win.round
            ),
            kind: "first_win".to_string(),
        });
    }

    // A "melhor temporada registrada: N pts" saiu daqui. O card de destaque em
    // Records já se chama "Melhor temporada" e mede outra coisa (vitórias), então
    // os dois apareciam com o mesmo nome, anos possivelmente diferentes e números
    // que não se conversam — leitura de contradição, não de informação a mais.

    if let Some(latest) = facts.last() {
        items.push(TeamHistoryTimelineItem {
            year: latest.season_year.to_string(),
            text: format!(
                "Último registro em {}, rodada {}.",
                team_history_category_label(&latest.category),
                latest.round
            ),
            kind: "last_record".to_string(),
        });
    }

    items
}

pub(super) fn season_count_label(total: i32) -> String {
    match total {
        0 => rust_i18n::t!("team_dossier.season_count.none").to_string(),
        1 => rust_i18n::t!("team_dossier.season_count.one").to_string(),
        value => rust_i18n::t!("team_dossier.season_count.other", count = value).to_string(),
    }
}

/// Sequência atual por NÍVEL (rookie, amador, pro, ...) — quantas temporadas
/// consecutivas a equipe está no nível atual. Diferente do "grupo" (que a equipe
/// nunca troca), o nível muda com promoções/rebaixamentos, então o streak importa.
pub(super) fn current_level_streak_label(facts: &[TeamRaceFact]) -> String {
    if facts.is_empty() {
        return rust_i18n::t!("team_dossier.streak.none").to_string();
    }

    // season → categoria dominante → nível.
    let mut by_season: BTreeMap<i32, HashMap<String, i32>> = BTreeMap::new();
    for fact in facts {
        *by_season
            .entry(fact.season_number)
            .or_default()
            .entry(fact.category.clone())
            .or_insert(0) += 1;
    }
    let mut season_levels: Vec<(i32, String)> = by_season
        .into_iter()
        .map(|(season, cats)| {
            let category = cats
                .iter()
                .max_by_key(|(_, races)| **races)
                .map(|(cat, _)| cat.clone())
                .unwrap_or_default();
            let level = categories::get_category(&category)
                .map(|config| crate::constants::category_tier_label(config.nivel))
                .unwrap_or_else(|| "—".to_string());
            (season, level)
        })
        .collect();
    season_levels.sort_by_key(|(season, _)| *season);

    let current_level = match season_levels.last() {
        Some((_, level)) => level.clone(),
        None => return rust_i18n::t!("team_dossier.streak.none").to_string(),
    };

    // Conta temporadas consecutivas (e contíguas) no nível atual, do fim para trás.
    let mut streak = 0;
    let mut prev_season: Option<i32> = None;
    for (season, level) in season_levels.iter().rev() {
        if *level != current_level {
            break;
        }
        if let Some(prev) = prev_season {
            if prev - season != 1 {
                break;
            }
        }
        streak += 1;
        prev_season = Some(*season);
    }

    if streak <= 1 {
        rust_i18n::t!(
            "team_dossier.streak.level_one",
            level = current_level.as_str()
        )
        .to_string()
    } else {
        rust_i18n::t!(
            "team_dossier.streak.level_other",
            count = streak,
            level = current_level.as_str()
        )
        .to_string()
    }
}

pub(super) fn best_real_streak_label(facts: &[TeamRaceFact]) -> String {
    if facts.is_empty() {
        return rust_i18n::t!("team_dossier.streak.none").to_string();
    }
    let mut best_podium = 0;
    let mut current_podium = 0;
    let mut best_points = 0;
    let mut current_points = 0;
    for fact in facts {
        if fact.podium {
            current_podium += 1;
            best_podium = best_podium.max(current_podium);
        } else {
            current_podium = 0;
        }
        if fact.points > 0.0 {
            current_points += 1;
            best_points = best_points.max(current_points);
        } else {
            current_points = 0;
        }
    }
    if best_podium > 0 {
        if best_podium == 1 {
            rust_i18n::t!("team_dossier.streak.podium_one").to_string()
        } else {
            rust_i18n::t!("team_dossier.streak.podium_other", count = best_podium).to_string()
        }
    } else if best_points > 0 {
        if best_points == 1 {
            rust_i18n::t!("team_dossier.streak.points_one").to_string()
        } else {
            rust_i18n::t!("team_dossier.streak.points_other", count = best_points).to_string()
        }
    } else {
        rust_i18n::t!("team_dossier.streak.none").to_string()
    }
}
