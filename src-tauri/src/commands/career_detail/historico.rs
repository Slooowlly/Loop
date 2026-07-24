//! Historico agregado da carreira: presenca, primeiros marcos, auge, mobilidade e lesoes, lidos do arquivo de temporadas e do corrida-a-corrida.

use super::*;

pub(super) fn build_career_history_block(
    conn: &Connection,
    driver_id: &str,
) -> Result<DriverCareerHistoryBlock, String> {
    let seasons = load_career_season_archive_rows(conn, driver_id)?;
    let races = load_career_race_history_rows(conn, driver_id)?;

    let active_seasons: Vec<&CareerSeasonArchiveRow> = seasons
        .iter()
        .filter(|season| season.corridas > 0)
        .collect();
    let mut categories = HashSet::new();
    for season in &active_seasons {
        if !season.categoria.trim().is_empty() {
            categories.insert(season.categoria.clone());
        }
    }

    let presenca = DriverCareerPresenceBlock {
        tempo_carreira: career_duration_from_archive(&seasons),
        temporadas_disputadas: active_seasons.len() as i32,
        anos_desempregado: seasons
            .iter()
            .filter(|season| season.corridas == 0 && season.categoria.trim().is_empty())
            .count() as i32,
        periodos_desempregado: unemployment_periods(&seasons),
        corridas: active_seasons
            .iter()
            .map(|season| season.corridas)
            .sum::<i32>(),
        categorias_disputadas: categories.len() as i32,
    };

    let primeiros_marcos = DriverCareerFirstMarksBlock {
        primeiro_podio_corrida: races
            .iter()
            .find(|race| !race.is_dnf && race.position <= 3)
            .map(|race| race.race_index),
        primeira_vitoria_corrida: races
            .iter()
            .find(|race| !race.is_dnf && race.position == 1)
            .map(|race| race.race_index),
        primeiro_dnf_corrida: races
            .iter()
            .find(|race| race.is_dnf)
            .map(|race| race.race_index),
    };

    let auge = DriverCareerPeakBlock {
        melhor_temporada: best_career_season(&active_seasons),
        maior_sequencia_vitorias: longest_win_streak(&races),
    };

    let mobility_counts = count_category_mobility(&active_seasons);
    let team_summary = summarize_team_mobility(&races);
    let mobilidade = DriverCareerMobilityBlock {
        promocoes: mobility_counts.0,
        rebaixamentos: mobility_counts.1,
        equipes_defendidas: team_summary.0,
        tempo_medio_por_equipe: team_summary.1,
    };
    let injury_counts = injury_queries::count_injuries_by_severity_for_pilot(conn, driver_id)
        .map_err(|e| format!("Falha ao contar lesoes historicas do piloto: {e}"))?;
    let lesoes = DriverCareerInjuryBlock {
        leves: injury_counts.leves,
        moderadas: injury_counts.moderadas,
        graves: injury_counts.graves,
    };
    let eventos_especiais = build_special_events_block(conn, driver_id)?;

    Ok(DriverCareerHistoryBlock {
        presenca,
        primeiros_marcos,
        auge,
        mobilidade,
        lesoes,
        eventos_especiais,
    })
}

pub(super) fn unemployment_periods(seasons: &[CareerSeasonArchiveRow]) -> Vec<String> {
    let mut periods = Vec::new();
    let mut current_start: Option<i32> = None;
    let mut current_end: Option<i32> = None;

    for season in seasons {
        let unemployed = season.corridas == 0 && season.categoria.trim().is_empty();
        if unemployed {
            match current_end {
                Some(end) if season.ano == end + 1 => current_end = Some(season.ano),
                Some(end) => {
                    periods.push(format_year_period(current_start.unwrap_or(end), end));
                    current_start = Some(season.ano);
                    current_end = Some(season.ano);
                }
                None => {
                    current_start = Some(season.ano);
                    current_end = Some(season.ano);
                }
            }
        } else if let Some(end) = current_end {
            periods.push(format_year_period(current_start.unwrap_or(end), end));
            current_start = None;
            current_end = None;
        }
    }

    if let Some(end) = current_end {
        periods.push(format_year_period(current_start.unwrap_or(end), end));
    }

    periods
}

pub(super) fn career_duration_from_archive(seasons: &[CareerSeasonArchiveRow]) -> i32 {
    let Some(first_year) = seasons.iter().map(|season| season.ano).min() else {
        return 0;
    };
    let Some(last_year) = seasons.iter().map(|season| season.ano).max() else {
        return 0;
    };

    (last_year - first_year + 1).max(0)
}

pub(super) fn format_year_period(start: i32, end: i32) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}->{end}")
    }
}

pub(super) fn load_career_season_archive_rows(
    conn: &Connection,
    driver_id: &str,
) -> Result<Vec<CareerSeasonArchiveRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT season_number, ano, categoria, posicao_campeonato, pontos, snapshot_json
             FROM driver_season_archive
             WHERE piloto_id = ?1
             ORDER BY season_number ASC",
        )
        .map_err(|e| format!("Falha ao preparar historico de temporadas do piloto: {e}"))?;
    let mapped = stmt
        .query_map(rusqlite::params![driver_id], |row| {
            let snapshot_json: String = row.get(5)?;
            let snapshot: serde_json::Value =
                serde_json::from_str(&snapshot_json).unwrap_or_default();
            let categoria: String = row.get(2)?;
            Ok(CareerSeasonArchiveRow {
                ano: row.get(1)?,
                categoria: snapshot
                    .get("categoria")
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(categoria.as_str())
                    .to_string(),
                posicao_campeonato: row.get(3)?,
                pontos: snapshot
                    .get("pontos")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(row.get(4)?),
                corridas: snapshot
                    .get("corridas")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0) as i32,
                vitorias: snapshot
                    .get("vitorias")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0) as i32,
                podios: snapshot
                    .get("podios")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0) as i32,
            })
        })
        .map_err(|e| format!("Falha ao consultar historico de temporadas do piloto: {e}"))?;

    let mut rows = Vec::new();
    for row in mapped {
        rows.push(row.map_err(|e| format!("Falha ao ler historico de temporada: {e}"))?);
    }
    Ok(rows)
}

pub(super) fn load_career_race_history_rows(
    conn: &Connection,
    driver_id: &str,
) -> Result<Vec<CareerRaceHistoryRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                COALESCE(s.numero, 0) AS season_number,
                COALESCE(NULLIF(r.equipe_id, ''), '-') AS equipe_id,
                r.posicao_final,
                r.dnf
             FROM race_results r
             INNER JOIN calendar c ON c.id = r.race_id
             LEFT JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
             WHERE r.piloto_id = ?1
             ORDER BY COALESCE(s.numero, 0) ASC, c.rodada ASC, r.id ASC",
        )
        .map_err(|e| format!("Falha ao preparar historico corrida-a-corrida: {e}"))?;
    let mapped = stmt
        .query_map(rusqlite::params![driver_id], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)? != 0,
            ))
        })
        .map_err(|e| format!("Falha ao consultar historico corrida-a-corrida: {e}"))?;

    let mut rows = Vec::new();
    for (index, row) in mapped.enumerate() {
        let (season_number, team_id, position, is_dnf) =
            row.map_err(|e| format!("Falha ao ler historico corrida-a-corrida: {e}"))?;
        rows.push(CareerRaceHistoryRow {
            race_index: index as i32 + 1,
            season_number,
            team_id,
            position,
            is_dnf,
        });
    }
    Ok(rows)
}

pub(super) fn best_career_season(seasons: &[&CareerSeasonArchiveRow]) -> Option<DriverBestSeasonBlock> {
    seasons
        .iter()
        .copied()
        .max_by(|a, b| {
            best_season_score(a)
                .cmp(&best_season_score(b))
                .then_with(|| a.pontos.total_cmp(&b.pontos))
                .then_with(|| a.vitorias.cmp(&b.vitorias))
                .then_with(|| a.podios.cmp(&b.podios))
        })
        .map(|season| DriverBestSeasonBlock {
            ano: season.ano,
            categoria: season.categoria.clone(),
            posicao_campeonato: season.posicao_campeonato,
            pontos: season.pontos.round() as i32,
            vitorias: season.vitorias,
            podios: season.podios,
        })
}

pub(super) fn best_season_score(season: &CareerSeasonArchiveRow) -> i32 {
    let position_score = season
        .posicao_campeonato
        .map(|position| (50 - position).max(0) * 100)
        .unwrap_or(0);
    position_score + season.vitorias * 15 + season.podios * 5 + season.pontos.round() as i32
}

pub(super) fn longest_win_streak(races: &[CareerRaceHistoryRow]) -> i32 {
    let mut current = 0;
    let mut best = 0;
    for race in races {
        if !race.is_dnf && race.position == 1 {
            current += 1;
            best = best.max(current);
        } else {
            current = 0;
        }
    }
    best
}

pub(super) fn count_category_mobility(seasons: &[&CareerSeasonArchiveRow]) -> (i32, i32) {
    let mut promocoes = 0;
    let mut rebaixamentos = 0;
    let mut previous_tier = None;
    for season in seasons {
        let Some(tier) =
            categories::get_category_config(&season.categoria).map(|config| config.tier)
        else {
            continue;
        };
        if let Some(previous) = previous_tier {
            if tier > previous {
                promocoes += 1;
            } else if tier < previous {
                rebaixamentos += 1;
            }
        }
        previous_tier = Some(tier);
    }
    (promocoes, rebaixamentos)
}

pub(super) fn summarize_team_mobility(races: &[CareerRaceHistoryRow]) -> (i32, Option<f64>) {
    let mut teams = HashSet::new();
    let mut team_seasons = HashSet::new();
    for race in races {
        if race.team_id == "-" {
            continue;
        }
        teams.insert(race.team_id.clone());
        team_seasons.insert((race.season_number, race.team_id.clone()));
    }
    let team_count = teams.len() as i32;
    let average = if team_count > 0 {
        let raw = team_seasons.len() as f64 / team_count as f64;
        Some((raw * 10.0).round() / 10.0)
    } else {
        None
    };
    (team_count, average)
}

