//! Leitura crua do Atlas: limites de anos, injecao da temporada em andamento,
//! linhas do arquivo, titulos historicos e o dedupe por familia.

use super::*;

#[derive(Debug, Clone)]
pub(super) struct TeamArchiveRow {
    pub(super) team_id: String,
    pub(super) nome: String,
    pub(super) nome_curto: String,
    pub(super) cor_primaria: String,
    pub(super) cor_secundaria: String,
    pub(super) year: i32,
    pub(super) category: String,
    pub(super) class_name: Option<String>,
    pub(super) position: i32,
    pub(super) points: i32,
    pub(super) wins: i32,
    pub(super) titles: i32,
}

pub(super) struct HistoryYearBounds {
    pub(super) min_year: i32,
    pub(super) max_year: i32,
    pub(super) current_year: i32,
    /// Last season already ARCHIVED (finished). Used to decide who is the reigning
    /// champion — the in-progress season has no decided title, so the crown must
    /// stay on the champion of this year, not vanish while the season runs.
    pub(super) last_completed_year: i32,
    /// True when there is an active season whose year is beyond the archive — i.e.
    /// a season that has started but not finished, whose live standings should be
    /// injected as the timeline's current-year column.
    pub(super) in_progress: bool,
}

pub(super) fn history_year_bounds(conn: &Connection) -> Result<HistoryYearBounds, String> {
    let archive_max = conn
        .query_row("SELECT MAX(ano) FROM team_season_archive", [], |row| {
            row.get::<_, Option<i32>>(0)
        })
        .optional()
        .map_err(|e| format!("Falha ao consultar anos do historico de equipes: {e}"))?
        .flatten();
    let active_year = conn
        .query_row(
            "SELECT ano FROM seasons WHERE status = 'Ativa' ORDER BY numero DESC LIMIT 1",
            [],
            |row| row.get::<_, i32>(0),
        )
        .optional()
        .unwrap_or(None);
    let max_year = archive_max
        .into_iter()
        .chain(active_year)
        .max()
        .unwrap_or(DEFAULT_MAX_YEAR)
        .max(DEFAULT_MAX_YEAR);
    // current_year is the active season's year; falls back to max_year when no
    // active season exists (historical draft, pre-start, or career over).
    let current_year = active_year.unwrap_or(max_year);
    // In-progress = there is an active season that the archive does not yet cover
    // (its year is past the last archived one, or there is no archive at all).
    let in_progress = active_year.is_some_and(|ay| archive_max.map_or(true, |am| ay > am));
    // Reigning-champion anchor: the real last archived year (unfloored). When there
    // is no archive at all, use one year before the data start as a "nothing decided
    // yet" sentinel so no team is falsely crowned.
    let last_completed_year = archive_max.unwrap_or(DEFAULT_START_YEAR - 1);
    Ok(HistoryYearBounds {
        min_year: DEFAULT_START_YEAR,
        max_year,
        current_year,
        last_completed_year,
        in_progress,
    })
}

/// Synthesizes one archive-shaped row per ACTIVE team for the in-progress season,
/// so the Atlas can plot where each team sits RIGHT NOW (its live division), not
/// only where it finished last completed season. Positions are provisional: teams
/// are ranked within their division by the same order as the live standings and the
/// relegation logic (points, wins, best result, name). Constructor titles are 0 —
/// the season is not decided. Rows for every family are returned; the per-family
/// dedupe/band-match downstream keeps only the ones that belong to the shown family.
pub(super) fn load_current_season_rows(
    conn: &Connection,
    current_year: i32,
) -> Result<Vec<TeamArchiveRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                id,
                COALESCE(nome, id) AS nome,
                COALESCE(NULLIF(TRIM(nome_curto), ''), COALESCE(nome, id)) AS nome_curto,
                COALESCE(NULLIF(TRIM(cor_primaria), ''), '#58a6ff') AS cor_primaria,
                COALESCE(NULLIF(TRIM(cor_secundaria), ''), '#0d1727') AS cor_secundaria,
                categoria,
                NULLIF(TRIM(classe), '') AS classe,
                COALESCE(stats_pontos, 0) AS pontos,
                COALESCE(stats_vitorias, 0) AS vitorias,
                COALESCE(stats_melhor_resultado, 99) AS melhor_resultado
             FROM teams
             WHERE ativa = 1",
        )
        .map_err(|e| format!("Falha ao preparar equipes ativas do historico: {e}"))?;
    // (group key, best_result) kept alongside each row so we can rank within division.
    let mut entries: Vec<(String, i32, TeamArchiveRow)> = stmt
        .query_map([], |row| {
            let category: String = row.get(5)?;
            let class_name: Option<String> = row.get(6)?;
            let best_result: i32 = row.get(9)?;
            let group = current_ranking_group_key(&category, class_name.as_deref());
            Ok((
                group,
                best_result,
                TeamArchiveRow {
                    team_id: row.get(0)?,
                    nome: row.get(1)?,
                    nome_curto: row.get(2)?,
                    cor_primaria: row.get(3)?,
                    cor_secundaria: row.get(4)?,
                    year: current_year,
                    category,
                    class_name,
                    position: 0,
                    points: row.get(7)?,
                    wins: row.get(8)?,
                    titles: 0,
                },
            ))
        })
        .map_err(|e| format!("Falha ao consultar equipes ativas do historico: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Falha ao ler equipes ativas do historico: {e}"))?;

    // Rank within each division and stamp the provisional championship position.
    let mut by_group: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, (group, _, _)) in entries.iter().enumerate() {
        by_group.entry(group.clone()).or_default().push(index);
    }
    for indices in by_group.values_mut() {
        indices.sort_by(|&left, &right| {
            let (_, left_best, left_row) = &entries[left];
            let (_, right_best, right_row) = &entries[right];
            right_row
                .points
                .cmp(&left_row.points)
                .then_with(|| right_row.wins.cmp(&left_row.wins))
                .then_with(|| left_best.cmp(right_best))
                .then_with(|| left_row.nome.cmp(&right_row.nome))
        });
        for (position, &index) in indices.iter().enumerate() {
            entries[index].2.position = position as i32 + 1;
        }
    }

    Ok(entries.into_iter().map(|(_, _, row)| row).collect())
}

/// Division grouping for provisional standings — mirrors the archive's
/// `constructor_ranking_group_key`: multiclass categories (production/endurance) are
/// ranked per class, everyone else by category alone.
fn current_ranking_group_key(category: &str, class_name: Option<&str>) -> String {
    if matches!(category, "production_challenger" | "endurance") {
        format!("{category}::{}", class_name.unwrap_or(""))
    } else {
        category.to_string()
    }
}

pub(super) fn load_archive_rows(
    conn: &Connection,
    window_start: i32,
    window_end: i32,
) -> Result<Vec<TeamArchiveRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                a.team_id,
                COALESCE(t.nome, a.team_id) AS nome,
                COALESCE(NULLIF(TRIM(t.nome_curto), ''), COALESCE(t.nome, a.team_id)) AS nome_curto,
                COALESCE(NULLIF(TRIM(t.cor_primaria), ''), '#58a6ff') AS cor_primaria,
                COALESCE(NULLIF(TRIM(t.cor_secundaria), ''), '#0d1727') AS cor_secundaria,
                a.ano,
                a.categoria,
                a.classe,
                COALESCE(a.posicao_campeonato, 999) AS posicao_campeonato,
                COALESCE(a.pontos, 0.0) AS pontos,
                COALESCE(a.vitorias, 0) AS vitorias,
                COALESCE(a.titulos_construtores, 0) AS titulos_construtores
             FROM team_season_archive a
             LEFT JOIN teams t ON t.id = a.team_id
             WHERE a.ano BETWEEN ?1 AND ?2
             ORDER BY a.ano ASC, a.categoria ASC, posicao_campeonato ASC, a.team_id ASC",
        )
        .map_err(|e| format!("Falha ao preparar historico mundial de equipes: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![window_start, window_end], |row| {
            Ok(TeamArchiveRow {
                team_id: row.get(0)?,
                nome: row.get(1)?,
                nome_curto: row.get(2)?,
                cor_primaria: row.get(3)?,
                cor_secundaria: row.get(4)?,
                year: row.get(5)?,
                category: row.get(6)?,
                class_name: row.get(7)?,
                position: row.get(8)?,
                points: row.get::<_, f64>(9)?.round() as i32,
                wins: row.get(10)?,
                titles: row.get(11)?,
            })
        })
        .map_err(|e| format!("Falha ao consultar historico mundial de equipes: {e}"))?;

    let mut collected = Vec::new();
    for row in rows {
        collected.push(row.map_err(|e| format!("Falha ao ler historico mundial de equipes: {e}"))?);
    }
    Ok(collected)
}

/// Loads all-time constructor title counts for every team that ever won a band in
/// `family_def`, WITHOUT a year filter. The result is a map from `team_id` to a
/// `Vec<TeamTitleCount>` ordered by band index (lowest level first).
///
/// Querying all years (not the display window) keeps title counts stable as the user
/// scrolls the timeline.  Titles for bands whose `classe` was NULL in the archive
/// (pre-reform data) are silently excluded because they match no band definition —
/// this is an accepted limitation documented in the Atlas-1 task.
pub(super) fn load_all_time_titles(
    conn: &Connection,
    family_def: &TeamHistoryFamilyDef,
) -> Result<HashMap<String, Vec<TeamTitleCount>>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                team_id,
                categoria,
                COALESCE(NULLIF(TRIM(classe), ''), '') AS classe_norm,
                CAST(SUM(titulos_construtores) AS INTEGER) AS total
             FROM team_season_archive
             WHERE titulos_construtores = 1
             GROUP BY team_id, categoria, COALESCE(NULLIF(TRIM(classe), ''), '')",
        )
        .map_err(|e| format!("Falha ao preparar títulos históricos de equipes: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i32>(3)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar títulos históricos de equipes: {e}"))?;

    let mut by_team: HashMap<String, Vec<(usize, TeamTitleCount)>> = HashMap::new();

    for row in rows {
        let (team_id, categoria, classe_norm, count) =
            row.map_err(|e| format!("Falha ao ler títulos históricos de equipes: {e}"))?;

        let Some((band_idx, band)) = family_def.bands.iter().enumerate().find(|(_, band)| {
            if band.category != categoria {
                return false;
            }
            match band.class_name {
                Some(cn) => classe_norm == cn,
                None => classe_norm.is_empty(),
            }
        }) else {
            continue;
        };

        by_team.entry(team_id).or_default().push((
            band_idx,
            TeamTitleCount {
                band_key: band.key.to_string(),
                band_label: band.label.to_string(),
                count,
            },
        ));
    }

    Ok(by_team
        .into_iter()
        .map(|(team_id, mut entries)| {
            entries.sort_by_key(|(idx, _)| *idx);
            (team_id, entries.into_iter().map(|(_, tc)| tc).collect())
        })
        .collect())
}

pub(super) fn dedupe_archive_rows_for_family(
    family: &TeamHistoryFamilyDef,
    archive_rows: Vec<TeamArchiveRow>,
) -> Vec<TeamArchiveRow> {
    // Visual guard rail: if historical imports ever provide two snapshots for
    // the same team/year in a family, Atlas chooses the highest band instead
    // of showing the team twice. The model should still avoid duplicates.
    let mut selected_by_team_year: HashMap<(String, i32), (usize, usize)> = HashMap::new();

    for (row_index, row) in archive_rows.iter().enumerate() {
        let Some(band_index) = family.bands.iter().position(|band| band_matches(band, row)) else {
            continue;
        };
        let key = (row.team_id.clone(), row.year);
        match selected_by_team_year.get_mut(&key) {
            Some((current_band_index, current_row_index)) => {
                if band_index < *current_band_index {
                    *current_band_index = band_index;
                    *current_row_index = row_index;
                }
            }
            None => {
                selected_by_team_year.insert(key, (band_index, row_index));
            }
        }
    }

    let selected_indices = selected_by_team_year
        .into_values()
        .map(|(_, row_index)| row_index)
        .collect::<std::collections::HashSet<_>>();

    archive_rows
        .into_iter()
        .enumerate()
        .filter_map(|(index, row)| selected_indices.contains(&index).then_some(row))
        .collect()
}
