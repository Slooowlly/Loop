//! Leitura do archive de temporadas: histórico por categoria, fama arquivada e ano de estreia.

use super::*;

/// Lê o histórico por categoria do archive (uma `CategoryStats` por temporada-
/// categoria) e os eventos de título já contados. Núcleo compartilhado entre o
/// caminho de ativo (`load_driver_category_stats`) e o de aposentado por id
/// (`load_archive_category_stats`).
pub(super) fn read_archive_category_stats(
    conn: &Connection,
    driver_id: &str,
) -> Result<(Vec<CategoryStats>, HashSet<TitleEventKey>), String> {
    let mut stmt = conn
        .prepare(
            "SELECT categoria, pontos, snapshot_json, posicao_campeonato, season_number, ano
             FROM driver_season_archive
             WHERE piloto_id = ?1",
        )
        .map_err(|e| format!("Falha ao preparar historico global do piloto: {e}"))?;
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
        .map_err(|e| format!("Falha ao consultar historico global do piloto: {e}"))?;

    let mut stats = Vec::new();
    let mut counted_title_events = HashSet::<TitleEventKey>::new();
    for row in rows {
        let (category, points, snapshot_json, championship_position, season_number, year) =
            row.map_err(|e| format!("Falha ao ler historico global do piloto: {e}"))?;
        let snapshot: Value = serde_json::from_str(&snapshot_json).unwrap_or_default();
        let category = normalized_archive_category(&snapshot, category);
        let class_name =
            archived_title_class(conn, driver_id, &category, season_number, &snapshot)?;
        let points = json_f64(&snapshot, "pontos").unwrap_or(points);
        let wins = json_i32(&snapshot, "vitorias");
        let podiums = json_i32(&snapshot, "podios");
        let poles = json_i32(&snapshot, "poles");
        let races = json_i32(&snapshot, "corridas");
        let titles = valid_archived_title_count(
            json_i32_option(&snapshot, "titulos"),
            championship_position,
            points,
            wins,
            podiums,
            poles,
            races,
        );
        if titles > 0 {
            counted_title_events.insert(title_event_key(
                season_number,
                &category,
                class_name.as_deref(),
            ));
        }
        let title_team_id =
            json_string(&snapshot, "team_id").filter(|value| !value.trim().is_empty());
        stats.push(CategoryStats {
            category,
            class_name,
            points,
            wins,
            podiums,
            poles,
            races,
            titles,
            title_years: title_years_for_event(titles, year, title_team_id),
            dnfs: json_i32(&snapshot, "dnfs"),
        });
    }

    Ok((stats, counted_title_events))
}

/// Fama (`atributos.midia`) registrada no snapshot MAIS RECENTE do archive de
/// temporadas — a base pra medir "quanto a fama subiu" nesta temporada. `None`
/// quando não há archive/tabela/snapshot com o campo (ex.: 1ª temporada).
pub(super) fn latest_archived_media(
    conn: &Connection,
    driver_id: &str,
) -> Result<Option<f64>, String> {
    if !table_exists(conn, "driver_season_archive")? {
        return Ok(None);
    }
    let snapshot_json: Option<String> = conn
        .query_row(
            "SELECT snapshot_json FROM driver_season_archive
             WHERE piloto_id = ?1
             ORDER BY season_number DESC, ano DESC
             LIMIT 1",
            params![driver_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| format!("Falha ao carregar fama arquivada do piloto: {e}"))?;
    let Some(snapshot_json) = snapshot_json else {
        return Ok(None);
    };
    let snapshot: Value = serde_json::from_str(&snapshot_json).unwrap_or_default();
    Ok(snapshot
        .get("atributos")
        .and_then(|atributos| atributos.get("midia"))
        .and_then(Value::as_f64))
}

/// Histórico por categoria de um piloto (por id), incluindo títulos como campeão
/// de equipe. Vazio se não houver archive — o chamador decide o fallback.
pub(super) fn load_archive_category_stats(
    conn: &Connection,
    driver_id: &str,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
) -> Result<Vec<CategoryStats>, String> {
    if !table_exists(conn, "driver_season_archive")? {
        return Ok(Vec::new());
    }
    let (mut stats, counted_title_events) = read_archive_category_stats(conn, driver_id)?;
    let team_title_stats = team_champion_title_stats_for_driver(
        driver_id,
        &counted_title_events,
        team_title_stats_by_driver,
    );
    stats.extend(team_title_stats);
    Ok(stats)
}

pub(super) fn load_driver_category_stats(
    conn: &Connection,
    driver: &Driver,
    fallback_category: Option<&str>,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
    real_career: &RealCareerIndex,
) -> Result<Vec<CategoryStats>, String> {
    let fallback =
        || real_career.history_for(&driver.id, stats_from_driver(driver, fallback_category));
    if !table_exists(conn, "driver_season_archive")? {
        return Ok(vec![fallback()]);
    }

    let (mut stats, counted_title_events) = read_archive_category_stats(conn, &driver.id)?;
    let team_title_stats = team_champion_title_stats_for_driver(
        &driver.id,
        &counted_title_events,
        team_title_stats_by_driver,
    );
    if stats.is_empty() {
        stats.push(fallback());
    }
    stats.extend(team_title_stats);

    Ok(stats)
}

pub(super) fn active_driver_debut_year(
    conn: &Connection,
    driver: &Driver,
    current_year: i32,
) -> Result<i32, String> {
    let fallback_year = driver.ano_inicio_carreira as i32;
    if !table_exists(conn, "driver_season_archive")? {
        return Ok(inferred_active_driver_debut_year(
            driver,
            current_year,
            fallback_year,
        ));
    }

    let mut stmt = conn
        .prepare(
            "SELECT ano, categoria, snapshot_json
             FROM driver_season_archive
             WHERE piloto_id = ?1",
        )
        .map_err(|e| format!("Falha ao preparar estreia historica do piloto: {e}"))?;
    let rows = stmt
        .query_map(params![driver.id], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar estreia historica do piloto: {e}"))?;

    let mut archive_year: Option<i32> = None;
    for row in rows {
        let (year, category, snapshot_json) =
            row.map_err(|e| format!("Falha ao ler estreia historica do piloto: {e}"))?;
        let snapshot: Value = serde_json::from_str(&snapshot_json).unwrap_or_default();
        let category = normalized_archive_category(&snapshot, category);
        if category == "unknown"
            || is_especial(&category)
            || !has_competitive_archive_participation(&snapshot)
        {
            continue;
        }
        archive_year = Some(archive_year.map_or(year, |current| current.min(year)));
    }

    Ok(match archive_year {
        Some(year) => year,
        None => inferred_active_driver_debut_year(driver, current_year, fallback_year),
    })
}

pub(super) fn inferred_active_driver_debut_year(
    driver: &Driver,
    current_year: i32,
    fallback_year: i32,
) -> i32 {
    if current_year > 0 {
        let career_seasons = driver.stats_carreira.temporadas as i32;
        if career_seasons > 0 {
            return (current_year - career_seasons + 1).max(1);
        }
        // Sem temporada fechada, toda largada que ele tem é da temporada em
        // curso: a estreia é este ano. `ano_inicio_carreira` NÃO serve aqui —
        // nasce como pano de fundo (o ano em que o piloto pegou num kart, aos
        // 16), não como estreia na carreira, e fazia o piloto do jogador saltar
        // de 0 pra 5 anos assim que largava pela primeira vez.
        return current_year;
    }

    fallback_year.max(0)
}

pub(super) fn has_competitive_archive_participation(snapshot: &Value) -> bool {
    json_i32(snapshot, "corridas") > 0
        || json_f64(snapshot, "pontos").unwrap_or(0.0) > 0.0
        || json_i32(snapshot, "vitorias") > 0
        || json_i32(snapshot, "podios") > 0
        || json_i32(snapshot, "poles") > 0
        || json_i32(snapshot, "titulos") > 0
}
