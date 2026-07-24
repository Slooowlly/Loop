//! Montagem das faixas do payload: agrupa as linhas por equipe, casa faixa x linha
//! e deriva a serie temporal e a coroa de campeao reinante de cada equipe.

use super::*;

pub(super) fn build_band_payload(
    band: &TeamHistoryBandDef,
    archive_rows: &[TeamArchiveRow],
    all_time_titles: &HashMap<String, Vec<TeamTitleCount>>,
    window_start: i32,
    window_end: i32,
    last_completed_year: i32,
) -> GlobalTeamHistoryBand {
    let mut by_team: HashMap<String, Vec<&TeamArchiveRow>> = HashMap::new();
    for row in archive_rows.iter().filter(|row| band_matches(band, row)) {
        by_team.entry(row.team_id.clone()).or_default().push(row);
    }

    let mut rows = by_team
        .into_values()
        .filter_map(|rows| {
            build_team_row(
                band,
                rows,
                all_time_titles,
                window_start,
                window_end,
                last_completed_year,
            )
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.base_position
            .cmp(&right.base_position)
            .then_with(|| left.nome.cmp(&right.nome))
    });

    GlobalTeamHistoryBand {
        key: band.key.to_string(),
        label: band.label.to_string(),
        category: band.category.to_string(),
        class_name: band.class_name.map(str::to_string),
        starts_year: band_start_year(band),
        is_special: band.is_special,
        rows,
    }
}

pub(super) fn band_matches(band: &TeamHistoryBandDef, row: &TeamArchiveRow) -> bool {
    if row.category != band.category {
        return false;
    }
    match band.class_name {
        Some(class_name) => normalize_opt(row.class_name.as_deref()) == Some(class_name),
        None => true,
    }
}

fn normalize_opt(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn build_team_row(
    band: &TeamHistoryBandDef,
    mut rows: Vec<&TeamArchiveRow>,
    all_time_titles: &HashMap<String, Vec<TeamTitleCount>>,
    _window_start: i32,
    window_end: i32,
    last_completed_year: i32,
) -> Option<GlobalTeamHistoryTeamRow> {
    rows.sort_by(|left, right| {
        left.year
            .cmp(&right.year)
            .then_with(|| left.position.cmp(&right.position))
    });
    rows.dedup_by(|left, right| left.year == right.year);
    let first = rows.first()?;
    let slot = if band.is_special {
        "special"
    } else {
        "regular"
    };
    let points = rows
        .iter()
        .map(|row| GlobalTeamHistoryPoint {
            year: row.year,
            slot: slot.to_string(),
            position: row.position,
            points: row.points,
            wins: row.wins,
            titles: row.titles,
        })
        .collect();

    let titles = all_time_titles
        .get(&first.team_id)
        .cloned()
        .unwrap_or_default();
    // Reigning champion = won the latest COMPLETED season shown in the window. Clamp
    // to `last_completed_year` so the in-progress season's provisional row (title not
    // decided) never strips the crown from last season's champion, and so a scrolled
    // view still crowns the champion of that view's last finished year.
    let completed_window_end = window_end.min(last_completed_year);
    let is_reigning_champion = rows
        .iter()
        .filter(|row| row.year <= completed_window_end)
        .next_back()
        .is_some_and(|row| row.titles == 1 && row.year == completed_window_end);

    Some(GlobalTeamHistoryTeamRow {
        team_id: first.team_id.clone(),
        nome: first.nome.clone(),
        nome_curto: first.nome_curto.clone(),
        cor_primaria: first.cor_primaria.clone(),
        cor_secundaria: first.cor_secundaria.clone(),
        base_position: first.position,
        titles,
        is_reigning_champion,
        points,
    })
}
