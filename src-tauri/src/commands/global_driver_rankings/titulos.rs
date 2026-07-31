//! Títulos do piloto: o que conta como título arquivado e o resumo por categoria/ano.

use super::*;

pub(super) fn title_categories(
    stats: &[CategoryStats],
    team_lookup: &TeamLookup,
) -> Vec<GlobalDriverTitleCategorySummary> {
    let mut totals = HashMap::<(String, Option<String>), (i32, Vec<TitleYear>)>::new();
    for entry in stats {
        if entry.titles <= 0 {
            continue;
        }
        let total = totals
            .entry((entry.category.clone(), entry.class_name.clone()))
            .or_default();
        total.0 += entry.titles;
        total.1.extend(entry.title_years.iter().cloned());
    }
    let mut summaries = totals
        .into_iter()
        .map(|((categoria, classe), (titulos, years))| {
            let (anos, anos_equipes) = build_title_year_teams(&years, team_lookup);
            GlobalDriverTitleCategorySummary {
                categoria,
                classe,
                titulos,
                anos,
                anos_equipes,
            }
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        right
            .titulos
            .cmp(&left.titulos)
            .then_with(|| left.categoria.cmp(&right.categoria))
            .then_with(|| left.classe.cmp(&right.classe))
    });
    summaries
}

pub(super) fn load_team_lookup(conn: &Connection) -> Result<TeamLookup, String> {
    let teams = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao carregar equipes para titulos: {e}"))?;
    Ok(teams
        .into_iter()
        .map(|team| (team.id, (team.nome, team.cor_primaria)))
        .collect())
}

/// Agrega os anos de título (com a equipe de cada ano) em (`anos` ordenados desc,
/// `anos_equipes` na mesma ordem, com o nome/cor da equipe resolvidos pelo `team_id`).
pub(super) fn build_title_year_teams(
    years: &[TitleYear],
    team_lookup: &TeamLookup,
) -> (Vec<i32>, Vec<GlobalDriverTitleYearTeam>) {
    let mut ordered: Vec<i32> = Vec::new();
    let mut team_by_year: HashMap<i32, Option<String>> = HashMap::new();
    for entry in years {
        if entry.year <= 0 {
            continue;
        }
        match team_by_year.get_mut(&entry.year) {
            Some(existing) => {
                if existing.is_none() && entry.team_id.is_some() {
                    *existing = entry.team_id.clone();
                }
            }
            None => {
                team_by_year.insert(entry.year, entry.team_id.clone());
                ordered.push(entry.year);
            }
        }
    }
    ordered.sort_unstable_by(|left, right| right.cmp(left));
    let anos_equipes = ordered
        .iter()
        .map(|&ano| {
            let (equipe, equipe_cor) = team_by_year
                .get(&ano)
                .and_then(|team_id| team_id.as_deref())
                .and_then(|team_id| team_lookup.get(team_id))
                .map(|(nome, cor)| (Some(nome.clone()), Some(cor.clone())))
                .unwrap_or((None, None));
            GlobalDriverTitleYearTeam {
                ano,
                equipe,
                equipe_cor,
            }
        })
        .collect();
    (ordered, anos_equipes)
}

pub(super) fn valid_archived_title_count(
    snapshot_titles: Option<i32>,
    championship_position: Option<i32>,
    points: f64,
    wins: i32,
    podiums: i32,
    poles: i32,
    races: i32,
) -> i32 {
    let title_count = snapshot_titles.unwrap_or_else(|| {
        if championship_position == Some(1) {
            1
        } else {
            0
        }
    });
    if title_count <= 0 || !has_title_worthy_participation(points, wins, podiums, poles, races) {
        0
    } else {
        title_count
    }
}

pub(super) fn has_title_worthy_participation(
    points: f64,
    wins: i32,
    podiums: i32,
    poles: i32,
    races: i32,
) -> bool {
    races > 0 && (points > 0.0 || wins > 0 || podiums > 0 || poles > 0)
}

pub(super) fn valid_archived_title_count_for_pilot(
    conn: &Connection,
    driver_id: &str,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
) -> Result<Option<i32>, String> {
    let mut total = 0;
    let mut saw_archive = false;
    let mut counted_title_events = HashSet::<TitleEventKey>::new();

    if table_exists(conn, "driver_season_archive")? {
        let mut stmt = conn
            .prepare(
                "SELECT categoria, pontos, snapshot_json, posicao_campeonato, season_number
                 FROM driver_season_archive
                 WHERE piloto_id = ?1",
            )
            .map_err(|e| format!("Falha ao preparar titulos historicos do piloto: {e}"))?;
        let rows = stmt
            .query_map(params![driver_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i32>>(3)?,
                    row.get::<_, i32>(4)?,
                ))
            })
            .map_err(|e| format!("Falha ao consultar titulos historicos do piloto: {e}"))?;

        for row in rows {
            let (category, points, snapshot_json, championship_position, season_number) =
                row.map_err(|e| format!("Falha ao ler titulo historico do piloto: {e}"))?;
            saw_archive = true;
            let snapshot: Value = serde_json::from_str(&snapshot_json).unwrap_or_default();
            let category = normalized_archive_category(&snapshot, category);
            let class_name =
                archived_title_class(conn, driver_id, &category, season_number, &snapshot)?;
            let points = json_f64(&snapshot, "pontos").unwrap_or(points);
            let titles = valid_archived_title_count(
                json_i32_option(&snapshot, "titulos"),
                championship_position,
                points,
                json_i32(&snapshot, "vitorias"),
                json_i32(&snapshot, "podios"),
                json_i32(&snapshot, "poles"),
                json_i32(&snapshot, "corridas"),
            );
            if titles > 0 {
                counted_title_events.insert(title_event_key(
                    season_number,
                    &category,
                    class_name.as_deref(),
                ));
                total += titles;
            }
        }
    }

    let team_title_stats = team_champion_title_stats_for_driver(
        driver_id,
        &counted_title_events,
        team_title_stats_by_driver,
    );
    saw_archive = saw_archive || !team_title_stats.is_empty();
    total += team_title_stats
        .iter()
        .map(|stats| stats.titles)
        .sum::<i32>();

    Ok(saw_archive.then_some(total))
}

pub(super) fn valid_archived_title_categories_for_pilot(
    conn: &Connection,
    driver_id: &str,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
    team_lookup: &TeamLookup,
) -> Result<Option<Vec<GlobalDriverTitleCategorySummary>>, String> {
    let mut saw_archive = false;
    let mut totals = HashMap::<(String, Option<String>), (i32, Vec<TitleYear>)>::new();
    let mut counted_title_events = HashSet::<TitleEventKey>::new();

    if table_exists(conn, "driver_season_archive")? {
        let mut stmt = conn
            .prepare(
                "SELECT categoria, pontos, snapshot_json, posicao_campeonato, season_number, ano
                 FROM driver_season_archive
                 WHERE piloto_id = ?1",
            )
            .map_err(|e| format!("Falha ao preparar categorias campeas do piloto: {e}"))?;
        let rows = stmt
            .query_map(params![driver_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i32>>(3)?,
                    row.get::<_, i32>(4)?,
                    row.get::<_, i32>(5)?,
                ))
            })
            .map_err(|e| format!("Falha ao consultar categorias campeas do piloto: {e}"))?;

        for row in rows {
            let (category, points, snapshot_json, championship_position, season_number, year) =
                row.map_err(|e| format!("Falha ao ler categoria campea do piloto: {e}"))?;
            saw_archive = true;
            let snapshot: Value = serde_json::from_str(&snapshot_json).unwrap_or_default();
            let category = normalized_archive_category(&snapshot, category);
            let class_name =
                archived_title_class(conn, driver_id, &category, season_number, &snapshot)?;
            let points = json_f64(&snapshot, "pontos").unwrap_or(points);
            let titles = valid_archived_title_count(
                json_i32_option(&snapshot, "titulos"),
                championship_position,
                points,
                json_i32(&snapshot, "vitorias"),
                json_i32(&snapshot, "podios"),
                json_i32(&snapshot, "poles"),
                json_i32(&snapshot, "corridas"),
            );
            if titles > 0 {
                counted_title_events.insert(title_event_key(
                    season_number,
                    &category,
                    class_name.as_deref(),
                ));
                let title_team_id =
                    json_string(&snapshot, "team_id").filter(|value| !value.trim().is_empty());
                let total = totals.entry((category, class_name)).or_default();
                total.0 += titles;
                total
                    .1
                    .extend(title_years_for_event(titles, year, title_team_id));
            }
        }
    }

    let team_title_stats = team_champion_title_stats_for_driver(
        driver_id,
        &counted_title_events,
        team_title_stats_by_driver,
    );
    saw_archive = saw_archive || !team_title_stats.is_empty();
    for stats in team_title_stats {
        if stats.titles > 0 {
            let total = totals
                .entry((stats.category, stats.class_name))
                .or_default();
            total.0 += stats.titles;
            total.1.extend(stats.title_years);
        }
    }

    if !saw_archive {
        return Ok(None);
    }

    let mut summaries = totals
        .into_iter()
        .map(|((categoria, classe), (titulos, years))| {
            let (anos, anos_equipes) = build_title_year_teams(&years, team_lookup);
            GlobalDriverTitleCategorySummary {
                categoria,
                classe,
                titulos,
                anos,
                anos_equipes,
            }
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        right
            .titulos
            .cmp(&left.titulos)
            .then_with(|| left.categoria.cmp(&right.categoria))
            .then_with(|| left.classe.cmp(&right.classe))
    });
    Ok(Some(summaries))
}

pub(super) fn title_years_for_event(
    titles: i32,
    year: i32,
    team_id: Option<String>,
) -> Vec<TitleYear> {
    if titles > 0 && year > 0 {
        vec![TitleYear { year, team_id }]
    } else {
        Vec::new()
    }
}
