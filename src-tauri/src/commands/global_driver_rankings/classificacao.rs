//! Ordenação do ranking, deltas de posição e líderes por métrica.

use super::*;

pub(super) fn assign_ranks(rows: &mut [GlobalDriverRankingRow]) {
    rows.sort_by(compare_historical_rows);
    for (index, row) in rows.iter_mut().enumerate() {
        row.historical_rank = index as i32 + 1;
    }
    assign_metric_rank(rows, |row| row.vitorias, |row, rank| row.wins_rank = rank);
    assign_metric_rank(rows, |row| row.titulos, |row, rank| row.titles_rank = rank);
    assign_metric_rank(rows, |row| row.podios, |row, rank| row.podiums_rank = rank);
    assign_metric_rank(rows, |row| row.lesoes, |row, rank| row.injuries_rank = rank);
}

pub(super) fn assign_rank_deltas(
    conn: &Connection,
    rows: &mut [GlobalDriverRankingRow],
    stats_by_driver: &HashMap<String, Vec<CategoryStats>>,
) -> Result<(), String> {
    let contributions = load_latest_race_contributions(conn)?;
    if contributions.is_empty() {
        return Ok(());
    }

    let previous_ranks = previous_historical_ranks(rows, stats_by_driver, &contributions);
    for row in rows {
        if let Some(previous_rank) = previous_ranks.get(&row.id) {
            let delta = previous_rank - row.historical_rank;
            if delta != 0 {
                row.historical_rank_delta = Some(delta);
            }
        }
    }

    Ok(())
}

pub(super) fn previous_historical_ranks(
    rows: &[GlobalDriverRankingRow],
    stats_by_driver: &HashMap<String, Vec<CategoryStats>>,
    contributions: &HashMap<String, Vec<RaceContribution>>,
) -> HashMap<String, i32> {
    let mut previous_rows = rows
        .iter()
        .map(|row| {
            let previous_index = stats_by_driver
                .get(&row.id)
                .map(|stats| previous_historical_index(stats, contributions.get(&row.id)))
                .unwrap_or(row.historical_index);
            let driver_contributions = contributions.get(&row.id).cloned().unwrap_or_default();
            let wins_delta = driver_contributions
                .iter()
                .map(|entry| entry.wins)
                .sum::<i32>();
            let podiums_delta = driver_contributions
                .iter()
                .map(|entry| entry.podiums)
                .sum::<i32>();

            (
                row.id.clone(),
                row.nome.clone(),
                previous_index,
                row.titulos,
                (row.vitorias - wins_delta).max(0),
                (row.podios - podiums_delta).max(0),
            )
        })
        .collect::<Vec<_>>();

    previous_rows.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.3.cmp(&a.3))
            .then_with(|| b.4.cmp(&a.4))
            .then_with(|| b.5.cmp(&a.5))
            .then_with(|| a.1.cmp(&b.1))
    });

    previous_rows
        .into_iter()
        .enumerate()
        .map(|(index, (id, _, _, _, _, _))| (id, index as i32 + 1))
        .collect()
}

pub(super) fn previous_historical_index(
    stats: &[CategoryStats],
    contributions: Option<&Vec<RaceContribution>>,
) -> f64 {
    let Some(contributions) = contributions else {
        return compute_historical_index(stats);
    };
    let mut previous_stats = stats.to_vec();

    for contribution in contributions {
        if let Some(entry) = previous_stats
            .iter_mut()
            .find(|entry| entry.category == contribution.category)
        {
            entry.points = (entry.points - contribution.points).max(0.0);
            entry.wins = (entry.wins - contribution.wins).max(0);
            entry.podiums = (entry.podiums - contribution.podiums).max(0);
            entry.poles = (entry.poles - contribution.poles).max(0);
            entry.races = (entry.races - contribution.races).max(0);
            entry.dnfs = (entry.dnfs - contribution.dnfs).max(0);
        }
    }

    compute_historical_index(&previous_stats)
}

pub(super) fn load_latest_race_contributions(
    conn: &Connection,
) -> Result<HashMap<String, Vec<RaceContribution>>, String> {
    if !table_exists(conn, "calendar")? || !table_exists(conn, "race_results")? {
        return Ok(HashMap::new());
    }

    let latest_race = conn
        .query_row(
            "SELECT c.id, c.categoria
             FROM calendar c
             JOIN seasons s ON c.temporada_id = s.id
             WHERE EXISTS (
                SELECT 1 FROM race_results r WHERE r.race_id = c.id
             )
             ORDER BY s.numero DESC, c.rodada DESC, c.id DESC
             LIMIT 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|e| format!("Falha ao carregar ultima corrida do ranking global: {e}"))?;

    let Some((race_id, category)) = latest_race else {
        return Ok(HashMap::new());
    };

    let mut stmt = conn
        .prepare(
            "SELECT piloto_id, pontos, posicao_largada, posicao_final, dnf
             FROM race_results
             WHERE race_id = ?1",
        )
        .map_err(|e| format!("Falha ao preparar resultados da ultima corrida global: {e}"))?;
    let rows = stmt
        .query_map(params![race_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
                row.get::<_, i32>(4)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar resultados da ultima corrida global: {e}"))?;

    let mut contributions: HashMap<String, Vec<RaceContribution>> = HashMap::new();
    for row in rows {
        let (driver_id, points, grid_position, finish_position, dnf) =
            row.map_err(|e| format!("Falha ao ler resultado da ultima corrida global: {e}"))?;
        contributions
            .entry(driver_id)
            .or_default()
            .push(RaceContribution {
                category: category.clone(),
                points,
                wins: if finish_position == 1 { 1 } else { 0 },
                podiums: if (1..=3).contains(&finish_position) {
                    1
                } else {
                    0
                },
                poles: if grid_position == 1 { 1 } else { 0 },
                races: 1,
                dnfs: if dnf != 0 { 1 } else { 0 },
            });
    }

    Ok(contributions)
}

pub(super) fn compare_historical_rows(
    a: &GlobalDriverRankingRow,
    b: &GlobalDriverRankingRow,
) -> std::cmp::Ordering {
    b.historical_index
        .partial_cmp(&a.historical_index)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| b.titulos.cmp(&a.titulos))
        .then_with(|| b.vitorias.cmp(&a.vitorias))
        .then_with(|| b.podios.cmp(&a.podios))
        .then_with(|| a.nome.cmp(&b.nome))
}

pub(super) fn assign_metric_rank(
    rows: &mut [GlobalDriverRankingRow],
    metric: fn(&GlobalDriverRankingRow) -> i32,
    assign: fn(&mut GlobalDriverRankingRow, i32),
) {
    let mut ordered = rows
        .iter()
        .enumerate()
        .map(|(index, row)| (index, metric(row), row.nome.clone()))
        .collect::<Vec<_>>();
    ordered.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.2.cmp(&b.2)));
    for (rank_index, (row_index, _, _)) in ordered.into_iter().enumerate() {
        assign(&mut rows[row_index], rank_index as i32 + 1);
    }
}

pub(super) fn build_leaders(rows: &[GlobalDriverRankingRow]) -> GlobalDriverRankingLeaders {
    GlobalDriverRankingLeaders {
        historical_index_driver_id: rows.first().map(|row| row.id.clone()),
        wins_driver_id: leader_by(rows, |row| row.vitorias),
        titles_driver_id: leader_by(rows, |row| row.titulos),
        injuries_driver_id: leader_by(rows, |row| row.lesoes),
    }
}

pub(super) fn has_competitive_history(row: &GlobalDriverRankingRow) -> bool {
    row.historical_index > 0.0
        || row.corridas > 0
        || row.pontos > 0
        || row.titulos > 0
        || row.vitorias > 0
        || row.podios > 0
        || row.poles > 0
        || row.dnfs > 0
}

pub(super) fn has_ranking_visibility(row: &GlobalDriverRankingRow) -> bool {
    has_competitive_history(row) || is_current_regular_grid_driver(row)
}

pub(super) fn is_current_regular_grid_driver(row: &GlobalDriverRankingRow) -> bool {
    row.status == "Ativo" && row.categoria_atual.is_some() && row.equipe_nome.is_some()
}

pub(super) fn leader_by(
    rows: &[GlobalDriverRankingRow],
    metric: fn(&GlobalDriverRankingRow) -> i32,
) -> Option<String> {
    rows.iter()
        .max_by(|a, b| metric(a).cmp(&metric(b)).then_with(|| b.nome.cmp(&a.nome)))
        .map(|row| row.id.clone())
}
