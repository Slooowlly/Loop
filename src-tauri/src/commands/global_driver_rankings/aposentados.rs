//! Snapshots dos aposentados e normalização da temporada de aposentadoria.

use super::*;

pub(super) fn load_retired_snapshots(
    conn: &Connection,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
    team_lookup: &TeamLookup,
) -> Result<Vec<RetiredDriverSnapshot>, String> {
    if !table_exists(conn, "retired")? {
        return Ok(Vec::new());
    }

    let mut stmt = conn
        .prepare("SELECT piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas FROM retired")
        .map_err(|e| format!("Falha ao preparar aposentados globais: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar aposentados globais: {e}"))?;

    let mut retired = Vec::new();
    for row in rows {
        let (id, name, retirement_season, category, stats_json) =
            row.map_err(|e| format!("Falha ao ler aposentado global: {e}"))?;
        let snapshot: Value = serde_json::from_str(&stats_json).unwrap_or_default();
        let retirement_season = normalize_retirement_season(conn, &retirement_season)?;
        let archived_titles =
            valid_archived_title_count_for_pilot(conn, &id, team_title_stats_by_driver)?;
        let title_categories = valid_archived_title_categories_for_pilot(
            conn,
            &id,
            team_title_stats_by_driver,
            team_lookup,
        )?
        .unwrap_or_else(|| {
            title_categories(
                &[CategoryStats {
                    category: category.clone(),
                    class_name: None,
                    points: json_f64(&snapshot, "pontos")
                        .or_else(|| json_f64(&snapshot, "pontos_total"))
                        .or_else(|| json_f64(&snapshot, "carreira_pontos_total"))
                        .unwrap_or(0.0),
                    wins: json_i32(&snapshot, "vitorias"),
                    podiums: json_i32(&snapshot, "podios"),
                    poles: json_i32(&snapshot, "poles"),
                    races: json_i32(&snapshot, "corridas"),
                    titles: json_i32(&snapshot, "titulos"),
                    title_years: Vec::new(),
                    dnfs: json_i32(&snapshot, "dnfs"),
                }],
                team_lookup,
            )
        });
        let snapshot_titles = json_i32(&snapshot, "titulos");
        retired.push(RetiredDriverSnapshot {
            id,
            name,
            retirement_season,
            category: category.clone(),
            career_start_year: json_i32_option(&snapshot, "ano_inicio_carreira"),
            career_years: json_i32_option(&snapshot, "anos_carreira")
                .or_else(|| json_i32_option(&snapshot, "temporadas")),
            stats: CategoryStats {
                category,
                class_name: None,
                points: json_f64(&snapshot, "pontos")
                    .or_else(|| json_f64(&snapshot, "pontos_total"))
                    .or_else(|| json_f64(&snapshot, "carreira_pontos_total"))
                    .unwrap_or(0.0),
                wins: json_i32(&snapshot, "vitorias"),
                podiums: json_i32(&snapshot, "podios"),
                poles: json_i32(&snapshot, "poles"),
                races: json_i32(&snapshot, "corridas"),
                titles: archived_titles.unwrap_or(snapshot_titles),
                title_years: Vec::new(),
                dnfs: json_i32(&snapshot, "dnfs"),
            },
            title_categories,
        });
    }
    Ok(retired)
}

pub(super) fn normalize_retirement_season(
    conn: &Connection,
    value: &str,
) -> Result<String, String> {
    let Some(parsed) = parse_positive_i32(value) else {
        return Ok(value.to_string());
    };
    if parsed >= 1900 {
        return Ok(value.to_string());
    }

    conn.query_row(
        "SELECT ano FROM seasons WHERE numero = ?1 LIMIT 1",
        params![parsed],
        |row| row.get::<_, i32>(0),
    )
    .optional()
    .map(|year| {
        year.map(|value| value.to_string())
            .unwrap_or_else(|| value.to_string())
    })
    .map_err(|e| format!("Falha ao normalizar temporada de aposentadoria '{value}': {e}"))
}
