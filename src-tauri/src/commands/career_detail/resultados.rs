//! Resultados recentes do piloto (temporada em curso ou arquivo da anterior) e as agregacoes de performance derivadas deles.

use super::*;

pub(super) fn build_recent_results_for_driver(
    conn: &Connection,
    career_dir: &Path,
    season_id: &str,
    category: &str,
    driver_id: &str,
) -> Result<Vec<HistoricalRaceResult>, String> {
    let total_rounds = count_calendar_entries(conn, season_id, category)
        .map_err(|e| format!("Falha ao contar corridas da categoria: {e}"))?
        as usize;

    if total_rounds == 0 {
        return Ok(Vec::new());
    }

    let histories =
        build_driver_histories(career_dir, category, total_rounds, &[driver_id.to_string()])?;

    Ok(histories
        .into_iter()
        .next()
        .map(|history| {
            history
                .results
                .into_iter()
                .enumerate()
                .filter_map(|(index, result)| {
                    result.map(|value| HistoricalRaceResult {
                        rodada: index as i32 + 1,
                        position: value.position,
                        is_dnf: value.is_dnf,
                        has_fastest_lap: value.has_fastest_lap,
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

pub(super) fn build_archived_recent_results_for_driver(
    conn: &Connection,
    current_season_number: i32,
    driver_id: &str,
) -> Result<ArchivedRecentResults, String> {
    let archive_row: Option<(String, String)> = conn
        .query_row(
            "SELECT categoria, snapshot_json
             FROM driver_season_archive
             WHERE piloto_id = ?1 AND season_number < ?2
             ORDER BY season_number DESC
             LIMIT 1",
            rusqlite::params![driver_id, current_season_number],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map(Some)
        .or_else(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })
        .map_err(|e| format!("Falha ao buscar forma historica do piloto '{driver_id}': {e}"))?;

    let Some((archive_category, snapshot_json)) = archive_row else {
        return Ok(ArchivedRecentResults {
            results: Vec::new(),
            form_context: None,
        });
    };
    let snapshot: serde_json::Value = serde_json::from_str(&snapshot_json).map_err(|e| {
        format!("Falha ao interpretar forma historica do piloto '{driver_id}': {e}")
    })?;
    let result_values = snapshot
        .get("ultimos_resultados")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let form_context =
        archived_form_context_for_empty_results(&archive_category, &snapshot, &result_values);

    let results = result_values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            let position = value
                .get("position")
                .or_else(|| value.get("chegada"))
                .and_then(serde_json::Value::as_i64)? as i32;
            let is_dnf = value
                .get("is_dnf")
                .or_else(|| value.get("dnf"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            Some(HistoricalRaceResult {
                rodada: index as i32 + 1,
                position,
                is_dnf,
                has_fastest_lap: false,
            })
        })
        .collect();

    Ok(ArchivedRecentResults {
        results,
        form_context,
    })
}

pub(super) fn archived_form_context_for_empty_results(
    archive_category: &str,
    snapshot: &serde_json::Value,
    result_values: &[serde_json::Value],
) -> Option<String> {
    if !result_values.is_empty() {
        return None;
    }

    let races = snapshot
        .get("corridas")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    if races > 0 {
        return None;
    }

    let snapshot_category = snapshot
        .get("categoria")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(archive_category);

    if archive_category.trim().is_empty() && snapshot_category.trim().is_empty() {
        Some("sem_time_temporada_passada".to_string())
    } else {
        Some("sem_corridas_temporada_passada".to_string())
    }
}

pub(super) fn build_driver_performance_block(
    driver: &Driver,
    results: &[HistoricalRaceResult],
) -> DriverPerformanceBlock {
    let top_10 = results
        .iter()
        .filter(|result| !result.is_dnf && result.position <= 10)
        .count() as i32;
    let fastest_laps = results
        .iter()
        .filter(|result| result.has_fastest_lap)
        .count() as i32;
    let fora_top_10 = results
        .iter()
        .filter(|result| !result.is_dnf && result.position > 10)
        .count() as i32;
    let can_reuse_season_derivations = driver.stats_carreira.temporadas <= 1
        || driver.stats_carreira.corridas == driver.stats_temporada.corridas;

    DriverPerformanceBlock {
        temporada: PerformanceStatsBlock {
            vitorias: driver.stats_temporada.vitorias as i32,
            podios: driver.stats_temporada.podios as i32,
            top_10: Some(top_10),
            fora_top_10: Some(fora_top_10),
            poles: driver.stats_temporada.poles as i32,
            voltas_rapidas: Some(fastest_laps),
            hat_tricks: None,
            corridas: driver.stats_temporada.corridas as i32,
            dnfs: driver.stats_temporada.dnfs as i32,
        },
        carreira: PerformanceStatsBlock {
            vitorias: driver.stats_carreira.vitorias as i32,
            podios: driver.stats_carreira.podios as i32,
            top_10: can_reuse_season_derivations.then_some(top_10),
            fora_top_10: can_reuse_season_derivations.then_some(fora_top_10),
            poles: driver.stats_carreira.poles as i32,
            voltas_rapidas: can_reuse_season_derivations.then_some(fastest_laps),
            hat_tricks: None,
            corridas: driver.stats_carreira.corridas as i32,
            dnfs: driver.stats_carreira.dnfs as i32,
        },
    }
}

pub(super) fn average_finish(results: &[HistoricalRaceResult]) -> Option<f64> {
    let finishes: Vec<i32> = results
        .iter()
        .filter(|result| !result.is_dnf)
        .map(|result| result.position)
        .collect();

    if finishes.is_empty() {
        return None;
    }

    let total: i32 = finishes.iter().sum();
    Some(total as f64 / finishes.len() as f64)
}

pub(super) fn calculate_form_trend(results: &[HistoricalRaceResult]) -> String {
    if results.len() < 3 {
        return "\u{2192}".to_string();
    }

    let split_index = results.len() / 2;
    let previous = average_finish(&results[..split_index]);
    let recent = average_finish(&results[split_index..]);

    match (previous, recent) {
        (Some(previous), Some(recent)) if recent + 0.25 < previous => "\u{2197}".to_string(),
        (Some(previous), Some(recent)) if recent > previous + 0.25 => "\u{2198}".to_string(),
        _ => "\u{2192}".to_string(),
    }
}
