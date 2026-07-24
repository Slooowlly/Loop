//! Títulos de equipe (e especiais por classe) atribuídos ao piloto de referência.

use super::*;

pub(super) fn team_champion_title_stats_for_driver(
    driver_id: &str,
    counted_title_events: &HashSet<TitleEventKey>,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
) -> Vec<CategoryStats> {
    let mut seen_team_events = HashSet::<TitleEventKey>::new();
    let mut stats = Vec::new();

    if let Some(driver_stats) = team_title_stats_by_driver.get(driver_id) {
        for (event_key, title_stats) in driver_stats {
            if counted_title_events.contains(event_key)
                || !seen_team_events.insert(event_key.clone())
            {
                continue;
            }
            stats.push(title_stats.clone());
        }
    }

    stats
}

pub(super) fn push_team_title_stat(
    stats_by_driver: &mut TeamTitleStatsByDriver,
    driver_id: String,
    event_key: TitleEventKey,
    title_stats: CategoryStats,
) {
    stats_by_driver
        .entry(driver_id)
        .or_default()
        .push((event_key, title_stats));
}

pub(super) fn load_all_team_champion_title_stats(conn: &Connection) -> Result<TeamTitleStatsByDriver, String> {
    let mut stats_by_driver = load_all_special_class_champion_title_stats(conn)?;

    if !table_exists(conn, "team_season_archive")? {
        return Ok(stats_by_driver);
    }

    let mut stmt = conn
        .prepare(
            "SELECT season_number, ano, categoria, classe, pontos, vitorias, podios, poles,
                    corridas, team_id, piloto_1_id, piloto_2_id
             FROM team_season_archive
             WHERE posicao_campeonato = 1",
        )
        .map_err(|e| format!("Falha ao preparar titulos de equipe do piloto: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
                row.get::<_, Option<i32>>(5)?.unwrap_or(0),
                row.get::<_, Option<i32>>(6)?.unwrap_or(0),
                row.get::<_, Option<i32>>(7)?.unwrap_or(0),
                row.get::<_, Option<i32>>(8)?.unwrap_or(0),
                row.get::<_, String>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar titulos de equipe do piloto: {e}"))?;

    for row in rows {
        let (
            season_number,
            year,
            category,
            class_name,
            points,
            wins,
            podiums,
            poles,
            races,
            team_id,
            driver_one_id,
            driver_two_id,
        ) = row.map_err(|e| format!("Falha ao ler titulo de equipe do piloto: {e}"))?;
        let category = if category.trim().is_empty() {
            "unknown".to_string()
        } else {
            category
        };
        if !uses_team_archive_title_fallback(&category) {
            continue;
        }
        let class_name = if let Some(class_name) = clean_optional_string(class_name) {
            Some(class_name)
        } else if let Some(class_name) =
            archived_special_entry_class(conn, &team_id, &category, season_number)?
        {
            Some(class_name)
        } else {
            let class_from_first = match driver_one_id.as_deref() {
                Some(driver_id) => {
                    archived_contract_class(conn, driver_id, &category, season_number)?
                }
                None => None,
            };
            if class_from_first.is_some() {
                class_from_first
            } else {
                match driver_two_id.as_deref() {
                    Some(driver_id) => {
                        archived_contract_class(conn, driver_id, &category, season_number)?
                    }
                    None => None,
                }
            }
        };
        let event_key = title_event_key(season_number, &category, class_name.as_deref());
        if !has_title_worthy_participation(points, wins, podiums, poles, races) {
            continue;
        }
        let Some(title_driver_id) = best_team_title_driver_id(
            conn,
            season_number,
            &category,
            Some(&team_id),
            driver_one_id,
            driver_two_id,
        )?
        else {
            continue;
        };
        push_team_title_stat(
            &mut stats_by_driver,
            title_driver_id,
            event_key,
            CategoryStats {
                category,
                class_name,
                points: 0.0,
                wins: 0,
                podiums: 0,
                poles: 0,
                races: 0,
                titles: 1,
                title_years: vec![TitleYear {
                    year,
                    team_id: Some(team_id.clone()),
                }],
                dnfs: 0,
            },
        );
    }

    Ok(stats_by_driver)
}

pub(super) fn load_all_special_class_champion_title_stats(
    conn: &Connection,
) -> Result<TeamTitleStatsByDriver, String> {
    if !table_exists(conn, "special_team_entries")?
        || !table_exists(conn, "race_results")?
        || !table_exists(conn, "calendar")?
        || !table_exists(conn, "seasons")?
    {
        return Ok(HashMap::new());
    }

    let mut stmt = conn
        .prepare(
            "SELECT
                s.numero,
                s.ano,
                e.special_category,
                e.class_name,
                e.team_id,
                COALESCE(SUM(rr.pontos), 0.0) AS pontos,
                COALESCE(SUM(CASE WHEN rr.posicao_final = 1 THEN 1 ELSE 0 END), 0) AS vitorias,
                COALESCE(SUM(CASE WHEN rr.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END), 0) AS podios,
                COALESCE(SUM(CASE WHEN rr.posicao_largada = 1 THEN 1 ELSE 0 END), 0) AS poles,
                COUNT(DISTINCT rr.race_id) AS corridas
             FROM special_team_entries e
             INNER JOIN seasons s ON s.id = e.season_id
             INNER JOIN calendar c
                ON COALESCE(c.season_id, c.temporada_id) = e.season_id
               AND c.categoria = e.special_category
             INNER JOIN race_results rr
                ON rr.race_id = c.id
               AND rr.equipe_id = e.team_id
             WHERE e.special_category IN ('production_challenger', 'endurance')
             GROUP BY s.numero, s.ano, e.special_category, e.class_name, e.team_id
             HAVING COUNT(DISTINCT rr.race_id) > 0",
        )
        .map_err(|e| format!("Falha ao preparar campeoes especiais por classe: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            let season_number = row.get::<_, i32>(0)?;
            let year = row.get::<_, i32>(1)?;
            let category = row.get::<_, String>(2)?;
            let class_name = row.get::<_, String>(3)?;
            let team_id = row.get::<_, String>(4)?;
            let points = row.get::<_, f64>(5)?;
            let wins = row.get::<_, i32>(6)?;
            let podiums = row.get::<_, i32>(7)?;
            let poles = row.get::<_, i32>(8)?;
            let races = row.get::<_, i32>(9)?;
            let class_name = clean_optional_string(Some(class_name));
            let event_key = title_event_key(season_number, &category, class_name.as_deref());
            Ok(SpecialTeamTitleCandidate {
                event_key,
                season_number,
                year,
                category,
                class_name,
                team_id,
                points,
                wins,
                podiums,
                poles,
                races,
            })
        })
        .map_err(|e| format!("Falha ao consultar campeoes especiais por classe: {e}"))?;

    let mut candidates_by_event = HashMap::<TitleEventKey, Vec<SpecialTeamTitleCandidate>>::new();
    for row in rows {
        let candidate =
            row.map_err(|e| format!("Falha ao ler campeao especial por classe: {e}"))?;
        candidates_by_event
            .entry(candidate.event_key.clone())
            .or_default()
            .push(candidate);
    }

    let mut stats_by_driver = HashMap::new();
    for (event_key, mut candidates) in candidates_by_event {
        candidates.sort_by(compare_special_team_title_candidates);
        let Some(champion) = candidates.into_iter().next() else {
            continue;
        };
        let Some(title_driver_id) = best_team_title_driver_id(
            conn,
            champion.season_number,
            &champion.category,
            Some(&champion.team_id),
            None,
            None,
        )?
        else {
            continue;
        };
        push_team_title_stat(
            &mut stats_by_driver,
            title_driver_id,
            event_key,
            CategoryStats {
                category: champion.category,
                class_name: champion.class_name,
                points: 0.0,
                wins: 0,
                podiums: 0,
                poles: 0,
                races: 0,
                titles: 1,
                title_years: vec![TitleYear {
                    year: champion.year,
                    team_id: Some(champion.team_id),
                }],
                dnfs: 0,
            },
        );
    }

    Ok(stats_by_driver)
}

pub(super) fn compare_special_team_title_candidates(
    left: &SpecialTeamTitleCandidate,
    right: &SpecialTeamTitleCandidate,
) -> std::cmp::Ordering {
    right
        .points
        .total_cmp(&left.points)
        .then_with(|| right.wins.cmp(&left.wins))
        .then_with(|| right.podiums.cmp(&left.podiums))
        .then_with(|| right.poles.cmp(&left.poles))
        .then_with(|| right.races.cmp(&left.races))
        .then_with(|| left.team_id.cmp(&right.team_id))
}

pub(super) fn title_event_key(season_number: i32, category: &str, class_name: Option<&str>) -> TitleEventKey {
    (
        season_number,
        category.trim().to_string(),
        class_name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    )
}

pub(super) fn uses_team_archive_title_fallback(category: &str) -> bool {
    matches!(category, "production_challenger" | "endurance")
}

pub(super) fn best_team_title_driver_id(
    conn: &Connection,
    season_number: i32,
    category: &str,
    team_id: Option<&str>,
    driver_one_id: Option<String>,
    driver_two_id: Option<String>,
) -> Result<Option<String>, String> {
    let candidates = team_title_driver_candidates(
        conn,
        season_number,
        category,
        team_id,
        driver_one_id,
        driver_two_id,
    )?;
    if candidates.len() == 1 {
        return Ok(candidates.into_iter().next());
    }

    let mut scores = Vec::new();
    for driver_id in candidates {
        if let Some(score) =
            team_title_driver_score(conn, &driver_id, season_number, category, team_id)?
        {
            scores.push(score);
        }
    }
    scores.sort_by(compare_team_title_driver_scores);
    Ok(scores.into_iter().next().map(|score| score.driver_id))
}

pub(super) fn team_title_driver_candidates(
    conn: &Connection,
    season_number: i32,
    category: &str,
    team_id: Option<&str>,
    driver_one_id: Option<String>,
    driver_two_id: Option<String>,
) -> Result<Vec<String>, String> {
    let mut candidates = [driver_one_id, driver_two_id]
        .into_iter()
        .flatten()
        .map(|driver_id| driver_id.trim().to_string())
        .filter(|driver_id| !driver_id.is_empty())
        .collect::<Vec<_>>();

    if let Some(team_id) = team_id.filter(|value| !value.trim().is_empty()) {
        if table_exists(conn, "race_results")?
            && table_exists(conn, "calendar")?
            && table_exists(conn, "seasons")?
        {
            let mut stmt = conn
                .prepare(
                    "SELECT DISTINCT rr.piloto_id
                     FROM race_results rr
                     INNER JOIN calendar c ON c.id = rr.race_id
                     INNER JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
                     WHERE rr.equipe_id = ?1
                       AND s.numero = ?2
                       AND c.categoria = ?3
                       AND rr.piloto_id IS NOT NULL
                       AND TRIM(rr.piloto_id) <> ''",
                )
                .map_err(|e| format!("Falha ao preparar pilotos da equipe campea: {e}"))?;
            let rows = stmt
                .query_map(params![team_id, season_number, category], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(|e| format!("Falha ao consultar pilotos da equipe campea: {e}"))?;
            for row in rows {
                let driver_id =
                    row.map_err(|e| format!("Falha ao ler piloto da equipe campea: {e}"))?;
                let driver_id = driver_id.trim().to_string();
                if !driver_id.is_empty() {
                    candidates.push(driver_id);
                }
            }
        }
    }

    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}

pub(super) fn team_title_driver_score(
    conn: &Connection,
    driver_id: &str,
    season_number: i32,
    category: &str,
    team_id: Option<&str>,
) -> Result<Option<TeamTitleDriverScore>, String> {
    if !table_exists(conn, "race_results")?
        || !table_exists(conn, "calendar")?
        || !table_exists(conn, "seasons")?
    {
        return Ok(None);
    }

    let (points, wins, podiums, best_finish, races) = conn
        .query_row(
            "SELECT
                COALESCE(SUM(rr.pontos), 0.0),
                COALESCE(SUM(CASE WHEN rr.posicao_final = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN rr.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END), 0),
                COALESCE(MIN(NULLIF(rr.posicao_final, 0)), 9999),
                COUNT(*)
             FROM race_results rr
             INNER JOIN calendar c ON c.id = rr.race_id
             INNER JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
             WHERE rr.piloto_id = ?1
               AND s.numero = ?2
               AND c.categoria = ?3
               AND (?4 IS NULL OR rr.equipe_id = ?4)",
            params![driver_id, season_number, category, team_id],
            |row| {
                Ok((
                    row.get::<_, f64>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, i32>(2)?,
                    row.get::<_, i32>(3)?,
                    row.get::<_, i32>(4)?,
                ))
            },
        )
        .map_err(|e| format!("Falha ao pontuar piloto em titulo de equipe: {e}"))?;

    if races <= 0 {
        return Ok(None);
    }

    Ok(Some(TeamTitleDriverScore {
        driver_id: driver_id.to_string(),
        points,
        wins,
        podiums,
        best_finish,
        races,
    }))
}

pub(super) fn compare_team_title_driver_scores(
    left: &TeamTitleDriverScore,
    right: &TeamTitleDriverScore,
) -> std::cmp::Ordering {
    right
        .points
        .total_cmp(&left.points)
        .then_with(|| right.wins.cmp(&left.wins))
        .then_with(|| right.podiums.cmp(&left.podiums))
        .then_with(|| left.best_finish.cmp(&right.best_finish))
        .then_with(|| right.races.cmp(&left.races))
        .then_with(|| left.driver_id.cmp(&right.driver_id))
}
