use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use crate::commands::career_types::{
    GlobalDriverRankingLeaders, GlobalDriverRankingPayload, GlobalDriverRankingRow,
    GlobalDriverTitleCategorySummary,
};
use crate::config::app_config::AppConfig;
use crate::constants::categories::{get_feeder_categories, is_especial};
use crate::db::connection::Database;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::injuries as injury_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::models::driver::Driver;
use crate::models::enums::DriverStatus;

#[derive(Debug, Clone, Default)]
struct CategoryStats {
    category: String,
    class_name: Option<String>,
    points: f64,
    wins: i32,
    podiums: i32,
    poles: i32,
    races: i32,
    titles: i32,
    title_years: Vec<i32>,
    dnfs: i32,
}

#[derive(Debug, Clone)]
struct RetiredDriverSnapshot {
    id: String,
    name: String,
    retirement_season: String,
    category: String,
    stats: CategoryStats,
    title_categories: Vec<GlobalDriverTitleCategorySummary>,
    career_start_year: Option<i32>,
    career_years: Option<i32>,
}

#[derive(Debug, Clone)]
struct RankingEntry {
    row: GlobalDriverRankingRow,
    stats_by_category: Vec<CategoryStats>,
}

#[derive(Debug, Clone, Default)]
struct RaceContribution {
    category: String,
    points: f64,
    wins: i32,
    podiums: i32,
    poles: i32,
    races: i32,
    dnfs: i32,
}

type TitleEventKey = (i32, String, Option<String>);
type TeamTitleStatsByDriver = HashMap<String, Vec<(TitleEventKey, CategoryStats)>>;

#[derive(Debug, Clone)]
struct TeamTitleDriverScore {
    driver_id: String,
    points: f64,
    wins: i32,
    podiums: i32,
    best_finish: i32,
    races: i32,
}

#[derive(Debug, Clone)]
struct SpecialTeamTitleCandidate {
    event_key: TitleEventKey,
    season_number: i32,
    year: i32,
    category: String,
    class_name: Option<String>,
    team_id: String,
    points: f64,
    wins: i32,
    podiums: i32,
    poles: i32,
    races: i32,
}

pub(crate) fn get_global_driver_rankings_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    selected_driver_id: Option<&str>,
) -> Result<GlobalDriverRankingPayload, String> {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    if !db_path.exists() {
        return Err("Banco da carreira nao encontrado.".to_string());
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    build_global_driver_rankings(&db.conn, selected_driver_id)
}

fn build_global_driver_rankings(
    conn: &Connection,
    selected_driver_id: Option<&str>,
) -> Result<GlobalDriverRankingPayload, String> {
    let current_year = season_queries::get_active_season(conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa do ranking global: {e}"))?
        .map(|season| season.ano)
        .unwrap_or(2024);
    let drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos globais: {e}"))?;
    let team_title_stats_by_driver = load_all_team_champion_title_stats(conn)?;
    let mut entries = Vec::new();
    let mut seen_driver_ids = HashSet::new();
    let mut retired_by_id: HashMap<String, RetiredDriverSnapshot> =
        load_retired_snapshots(conn, &team_title_stats_by_driver)?
            .into_iter()
            .map(|retired| (retired.id.clone(), retired))
            .collect();

    for driver in drivers {
        seen_driver_ids.insert(driver.id.clone());
        if driver.status == DriverStatus::Aposentado {
            if let Some(retired) = retired_by_id.remove(&driver.id) {
                entries.push(build_retired_driver_entry_from_driver(
                    retired,
                    &driver,
                    current_year,
                ));
                continue;
            }
        }
        entries.push(build_current_driver_entry(
            conn,
            &driver,
            current_year,
            &team_title_stats_by_driver,
        )?);
    }

    for retired in retired_by_id.into_values() {
        if seen_driver_ids.contains(&retired.id) {
            continue;
        }
        entries.push(build_retired_driver_entry(retired, current_year));
    }

    let unranked_player_driver = entries
        .iter()
        .find(|entry| entry.row.is_jogador)
        .map(|entry| entry.row.clone());
    entries.retain(|entry| has_ranking_visibility(&entry.row));
    let stats_by_driver = entries
        .iter()
        .map(|entry| (entry.row.id.clone(), entry.stats_by_category.clone()))
        .collect::<HashMap<_, _>>();
    let mut rows = entries
        .into_iter()
        .map(|entry| entry.row)
        .collect::<Vec<_>>();
    rows.retain(has_ranking_visibility);
    assign_ranks(&mut rows);
    assign_rank_deltas(conn, &mut rows, &stats_by_driver)?;
    let leaders = build_leaders(&rows);
    let player_driver = rows
        .iter()
        .find(|row| row.is_jogador)
        .cloned()
        .or(unranked_player_driver);

    Ok(GlobalDriverRankingPayload {
        selected_driver_id: selected_driver_id.map(str::to_string),
        player_driver,
        rows,
        leaders,
    })
}

fn build_current_driver_entry(
    conn: &Connection,
    driver: &Driver,
    current_year: i32,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
) -> Result<RankingEntry, String> {
    let contract = contract_queries::get_active_regular_contract_for_pilot(conn, &driver.id)
        .map_err(|e| format!("Falha ao buscar contrato regular ativo do piloto: {e}"))?;
    let team = contract.as_ref().and_then(|value| {
        team_queries::get_team_by_id(conn, &value.equipe_id)
            .ok()
            .flatten()
    });
    let category = contract
        .as_ref()
        .and_then(|value| regular_category(Some(&value.categoria)))
        .or_else(|| regular_category(driver.categoria_atual.as_deref()));
    let stats_by_category = load_driver_category_stats(
        conn,
        driver,
        category.as_deref(),
        team_title_stats_by_driver,
    )?;
    let historical_index = stats_by_category
        .iter()
        .map(score_category_stats)
        .sum::<f64>();
    let injuries = injury_queries::count_injuries_by_severity_for_pilot(conn, &driver.id)
        .map_err(|e| format!("Falha ao contar lesoes do piloto: {e}"))?;
    let active_injury_type = injury_queries::get_active_injury_for_pilot(conn, &driver.id)
        .map_err(|e| format!("Falha ao buscar lesao ativa do piloto: {e}"))?
        .map(|injury| injury.injury_type.as_str().to_string());
    let total = total_stats(&stats_by_category);
    let (status, status_tone) = driver_status_label(driver, contract.is_some());
    let mut extra_historical_categories = load_contract_categories(conn, &driver.id)?;
    extra_historical_categories.extend(inferred_foundation_categories(
        driver,
        category.as_deref(),
        &stats_by_category,
    ));
    let historical_categories = historical_categories(
        &stats_by_category,
        category.as_deref(),
        &extra_historical_categories,
    );
    let debut_year = active_driver_debut_year(conn, driver)?;

    let row = GlobalDriverRankingRow {
        id: driver.id.clone(),
        nome: driver.nome.clone(),
        nacionalidade: driver.nacionalidade.clone(),
        idade: driver.idade as i32,
        status,
        status_tone,
        is_jogador: driver.is_jogador,
        is_lesionado: active_injury_type.is_some(),
        lesao_ativa_tipo: active_injury_type,
        equipe_nome: contract.as_ref().map(|value| value.equipe_nome.clone()),
        equipe_cor_primaria: team.map(|value| value.cor_primaria),
        categoria_atual: category,
        categorias_historicas: historical_categories,
        salario_anual: contract.as_ref().map(|value| value.salario_anual),
        ano_inicio_carreira: Some(debut_year),
        anos_carreira: years_since(debut_year, current_year),
        temporada_aposentadoria: None,
        anos_aposentado: None,
        historical_index: round_one(historical_index),
        historical_rank: 0,
        historical_rank_delta: None,
        wins_rank: 0,
        titles_rank: 0,
        podiums_rank: 0,
        injuries_rank: 0,
        corridas: total.races,
        pontos: total.points.round() as i32,
        vitorias: total.wins,
        podios: total.podiums,
        poles: total.poles,
        titulos: total.titles,
        titulos_por_categoria: title_categories(&stats_by_category),
        dnfs: total.dnfs,
        lesoes: injuries.leves + injuries.moderadas + injuries.graves,
        lesoes_leves: injuries.leves,
        lesoes_moderadas: injuries.moderadas,
        lesoes_graves: injuries.graves,
    };

    Ok(RankingEntry {
        row,
        stats_by_category,
    })
}

fn build_retired_driver_entry(retired: RetiredDriverSnapshot, current_year: i32) -> RankingEntry {
    let score = score_category_stats(&retired.stats);
    let retirement_year = parse_year(&retired.retirement_season);
    let career_years = retired.career_years.or_else(|| {
        retired
            .career_start_year
            .and_then(|start| retirement_year.and_then(|end| years_since(start, end)))
    });
    let years_retired = retirement_year.map(|year| (current_year - year).max(0));
    let stats_by_category = vec![retired.stats.clone()];
    let title_categories = if retired.title_categories.is_empty() {
        title_categories(&stats_by_category)
    } else {
        retired.title_categories.clone()
    };
    let row = GlobalDriverRankingRow {
        id: retired.id,
        nome: retired.name,
        nacionalidade: "".to_string(),
        idade: 0,
        status: "Aposentado".to_string(),
        status_tone: "retired".to_string(),
        is_jogador: false,
        is_lesionado: false,
        lesao_ativa_tipo: None,
        equipe_nome: None,
        equipe_cor_primaria: None,
        categoria_atual: Some(retired.category.clone()),
        categorias_historicas: historical_categories(
            &[retired.stats.clone()],
            Some(&retired.category),
            &[],
        ),
        salario_anual: None,
        ano_inicio_carreira: retired.career_start_year,
        anos_carreira: career_years,
        temporada_aposentadoria: Some(retired.retirement_season),
        anos_aposentado: years_retired,
        historical_index: score,
        historical_rank: 0,
        historical_rank_delta: None,
        wins_rank: 0,
        titles_rank: 0,
        podiums_rank: 0,
        injuries_rank: 0,
        corridas: retired.stats.races,
        pontos: retired.stats.points.round() as i32,
        vitorias: retired.stats.wins,
        podios: retired.stats.podiums,
        poles: retired.stats.poles,
        titulos: retired.stats.titles,
        titulos_por_categoria: title_categories,
        dnfs: retired.stats.dnfs,
        lesoes: 0,
        lesoes_leves: 0,
        lesoes_moderadas: 0,
        lesoes_graves: 0,
    };

    RankingEntry {
        row,
        stats_by_category,
    }
}

fn build_retired_driver_entry_from_driver(
    retired: RetiredDriverSnapshot,
    driver: &Driver,
    current_year: i32,
) -> RankingEntry {
    let mut entry = build_retired_driver_entry(retired, current_year);
    entry.row.nacionalidade = driver.nacionalidade.clone();
    entry.row.idade = driver.idade as i32;
    entry.row.is_jogador = driver.is_jogador;
    entry.row.ano_inicio_carreira = entry
        .row
        .ano_inicio_carreira
        .or(Some(driver.ano_inicio_carreira as i32));
    entry
}

fn driver_status_label(driver: &Driver, has_active_contract: bool) -> (String, String) {
    if driver.status == DriverStatus::Aposentado {
        return ("Aposentado".to_string(), "retired".to_string());
    }
    if has_active_contract || driver.categoria_especial_ativa.is_some() {
        return ("Ativo".to_string(), "active".to_string());
    }
    ("Livre".to_string(), "dimmed".to_string())
}

fn load_driver_category_stats(
    conn: &Connection,
    driver: &Driver,
    fallback_category: Option<&str>,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
) -> Result<Vec<CategoryStats>, String> {
    if !table_exists(conn, "driver_season_archive")? {
        return Ok(vec![stats_from_driver(driver, fallback_category)]);
    }

    let mut stmt = conn
        .prepare(
            "SELECT categoria, pontos, snapshot_json, posicao_campeonato, season_number, ano
             FROM driver_season_archive
             WHERE piloto_id = ?1",
        )
        .map_err(|e| format!("Falha ao preparar historico global do piloto: {e}"))?;
    let rows = stmt
        .query_map(params![driver.id], |row| {
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
            archived_title_class(conn, &driver.id, &category, season_number, &snapshot)?;
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
        stats.push(CategoryStats {
            category,
            class_name,
            points,
            wins,
            podiums,
            poles,
            races,
            titles,
            title_years: title_years_for_event(titles, year),
            dnfs: json_i32(&snapshot, "dnfs"),
        });
    }

    let team_title_stats = team_champion_title_stats_for_driver(
        &driver.id,
        &counted_title_events,
        team_title_stats_by_driver,
    );
    if stats.is_empty() {
        stats.push(stats_from_driver(driver, fallback_category));
    }
    stats.extend(team_title_stats);

    Ok(stats)
}

fn stats_from_driver(driver: &Driver, category: Option<&str>) -> CategoryStats {
    CategoryStats {
        category: category.unwrap_or("unknown").to_string(),
        class_name: None,
        points: driver.stats_carreira.pontos_total,
        wins: driver.stats_carreira.vitorias as i32,
        podiums: driver.stats_carreira.podios as i32,
        poles: driver.stats_carreira.poles as i32,
        races: driver.stats_carreira.corridas as i32,
        titles: driver.stats_carreira.titulos as i32,
        title_years: Vec::new(),
        dnfs: driver.stats_carreira.dnfs as i32,
    }
}

fn regular_category(category: Option<&str>) -> Option<String> {
    let category = category?.trim();
    if category.is_empty() || is_especial(category) {
        None
    } else {
        Some(category.to_string())
    }
}

fn active_driver_debut_year(conn: &Connection, driver: &Driver) -> Result<i32, String> {
    let fallback_year = driver.ano_inicio_carreira as i32;
    if !table_exists(conn, "driver_season_archive")? {
        return Ok(fallback_year);
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

    Ok(match (archive_year, fallback_year > 0) {
        (Some(year), true) => fallback_year.min(year),
        (Some(year), false) => year,
        (None, true) => fallback_year,
        (None, false) => 0,
    })
}

fn has_competitive_archive_participation(snapshot: &Value) -> bool {
    json_i32(snapshot, "corridas") > 0
        || json_f64(snapshot, "pontos").unwrap_or(0.0) > 0.0
        || json_i32(snapshot, "vitorias") > 0
        || json_i32(snapshot, "podios") > 0
        || json_i32(snapshot, "poles") > 0
        || json_i32(snapshot, "titulos") > 0
}

fn load_contract_categories(conn: &Connection, driver_id: &str) -> Result<Vec<String>, String> {
    let contracts = contract_queries::get_contracts_for_pilot(conn, driver_id)
        .map_err(|e| format!("Falha ao carregar historico de contratos do piloto: {e}"))?;
    let mut categories = Vec::new();
    for contract in contracts {
        push_category(&mut categories, &contract.categoria);
    }
    Ok(categories)
}

fn load_retired_snapshots(
    conn: &Connection,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
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
        let title_categories =
            valid_archived_title_categories_for_pilot(conn, &id, team_title_stats_by_driver)?
                .unwrap_or_else(|| {
                    title_categories(&[CategoryStats {
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
                    }])
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

fn normalize_retirement_season(conn: &Connection, value: &str) -> Result<String, String> {
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

fn total_stats(stats: &[CategoryStats]) -> CategoryStats {
    stats
        .iter()
        .fold(CategoryStats::default(), |mut total, entry| {
            total.points += entry.points;
            total.wins += entry.wins;
            total.podiums += entry.podiums;
            total.poles += entry.poles;
            total.races += entry.races;
            total.titles += entry.titles;
            total.dnfs += entry.dnfs;
            total
        })
}

fn title_categories(stats: &[CategoryStats]) -> Vec<GlobalDriverTitleCategorySummary> {
    let mut totals = HashMap::<(String, Option<String>), (i32, Vec<i32>)>::new();
    for entry in stats {
        if entry.titles <= 0 {
            continue;
        }
        let total = totals
            .entry((entry.category.clone(), entry.class_name.clone()))
            .or_default();
        total.0 += entry.titles;
        total.1.extend(entry.title_years.iter().copied());
    }
    let mut summaries = totals
        .into_iter()
        .map(|((categoria, classe), (titulos, mut anos))| {
            sort_title_years(&mut anos);
            GlobalDriverTitleCategorySummary {
                categoria,
                classe,
                titulos,
                anos,
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

fn historical_categories(
    stats: &[CategoryStats],
    fallback_category: Option<&str>,
    extra_categories: &[String],
) -> Vec<String> {
    let mut categories = Vec::new();
    for category in stats
        .iter()
        .map(|entry| entry.category.as_str())
        .chain(fallback_category.into_iter())
        .chain(extra_categories.iter().map(String::as_str))
    {
        push_category(&mut categories, category);
    }
    categories
}

fn push_category(categories: &mut Vec<String>, category: &str) {
    let category = category.trim();
    if category.is_empty() || category == "unknown" {
        return;
    }
    if !categories.iter().any(|value| value == category) {
        categories.push(category.to_string());
    }
}

fn inferred_foundation_categories(
    driver: &Driver,
    current_category: Option<&str>,
    stats: &[CategoryStats],
) -> Vec<String> {
    let Some(current_category) = current_category else {
        return Vec::new();
    };
    if current_category == "mazda_rookie" || current_category == "toyota_rookie" {
        return Vec::new();
    }
    if stats.iter().any(|entry| {
        let category = entry.category.as_str();
        category != current_category && category != "unknown"
    }) {
        return Vec::new();
    }
    if driver.stats_carreira.corridas == 0 && driver.stats_carreira.temporadas == 0 {
        return Vec::new();
    }

    inferred_ladder_for_category(&driver.id, current_category)
}

fn inferred_ladder_for_category(driver_id: &str, category: &str) -> Vec<String> {
    match category {
        "mazda_amador" => vec!["mazda_rookie".to_string()],
        "toyota_amador" => vec!["toyota_rookie".to_string()],
        "bmw_m2" => branded_foundation(driver_id),
        "production_challenger" => match stable_bucket(driver_id, 3) {
            0 => vec!["mazda_rookie".to_string(), "mazda_amador".to_string()],
            1 => vec!["toyota_rookie".to_string(), "toyota_amador".to_string()],
            _ => {
                let mut values = branded_foundation(driver_id);
                values.push("bmw_m2".to_string());
                values
            }
        },
        "gt4" => inferred_gt4_foundation(driver_id),
        "gt3" => {
            let mut ladder = inferred_gt4_foundation(driver_id);
            ladder.push("gt4".to_string());
            ladder
        }
        "endurance" => {
            let mut ladder = inferred_gt4_foundation(driver_id);
            ladder.push("gt4".to_string());
            ladder.push("gt3".to_string());
            ladder
        }
        other => get_feeder_categories(other)
            .into_iter()
            .map(str::to_string)
            .collect(),
    }
}

fn inferred_gt4_foundation(driver_id: &str) -> Vec<String> {
    match stable_bucket(driver_id, 4) {
        0 => vec!["mazda_rookie".to_string(), "mazda_amador".to_string()],
        1 => vec!["toyota_rookie".to_string(), "toyota_amador".to_string()],
        2 => {
            let mut values = branded_foundation(driver_id);
            values.push("bmw_m2".to_string());
            values
        }
        _ => {
            let mut values = inferred_ladder_for_category(driver_id, "production_challenger");
            values.push("production_challenger".to_string());
            values
        }
    }
}

fn branded_foundation(driver_id: &str) -> Vec<String> {
    if stable_bucket(driver_id, 2) == 0 {
        vec!["mazda_rookie".to_string(), "mazda_amador".to_string()]
    } else {
        vec!["toyota_rookie".to_string(), "toyota_amador".to_string()]
    }
}

fn stable_bucket(value: &str, buckets: usize) -> usize {
    if buckets == 0 {
        return 0;
    }
    value.bytes().fold(0_usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    }) % buckets
}

fn score_category_stats(stats: &CategoryStats) -> f64 {
    balanced_score(
        &stats.category,
        stats.titles,
        stats.wins,
        stats.podiums,
        stats.poles,
        stats.points,
        stats.races,
        stats.dnfs,
    )
}

fn category_multiplier(category: &str) -> f64 {
    match category {
        "mazda_rookie" | "toyota_rookie" => 0.75,
        "mazda_amador" | "toyota_amador" => 0.85,
        "bmw_m2" => 0.95,
        "gt4" => 1.08,
        "production_challenger" => 1.12,
        "gt3" => 1.22,
        "endurance" => 1.25,
        _ => 1.0,
    }
}

fn balanced_score(
    category: &str,
    titles: i32,
    wins: i32,
    podiums: i32,
    poles: i32,
    points: f64,
    races: i32,
    dnfs: i32,
) -> f64 {
    let normalized_points = points.max(0.0).sqrt() * 0.4;
    let race_bonus = (races.max(0) as f64).sqrt() * 0.5;
    let base = titles as f64 * 520.0
        + wins as f64 * 70.0
        + podiums as f64 * 4.0
        + poles as f64 * 8.0
        + normalized_points
        + race_bonus
        - dnfs.max(0) as f64 * 1.5;
    round_one(base.max(0.0) * category_multiplier(category))
}

fn assign_ranks(rows: &mut Vec<GlobalDriverRankingRow>) {
    rows.sort_by(compare_historical_rows);
    for (index, row) in rows.iter_mut().enumerate() {
        row.historical_rank = index as i32 + 1;
    }
    assign_metric_rank(rows, |row| row.vitorias, |row, rank| row.wins_rank = rank);
    assign_metric_rank(rows, |row| row.titulos, |row, rank| row.titles_rank = rank);
    assign_metric_rank(rows, |row| row.podios, |row, rank| row.podiums_rank = rank);
    assign_metric_rank(rows, |row| row.lesoes, |row, rank| row.injuries_rank = rank);
}

fn assign_rank_deltas(
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

fn previous_historical_ranks(
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

fn previous_historical_index(
    stats: &[CategoryStats],
    contributions: Option<&Vec<RaceContribution>>,
) -> f64 {
    let Some(contributions) = contributions else {
        return stats.iter().map(score_category_stats).sum();
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

    previous_stats.iter().map(score_category_stats).sum()
}

fn load_latest_race_contributions(
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

fn compare_historical_rows(
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

fn assign_metric_rank(
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

fn build_leaders(rows: &[GlobalDriverRankingRow]) -> GlobalDriverRankingLeaders {
    GlobalDriverRankingLeaders {
        historical_index_driver_id: rows.first().map(|row| row.id.clone()),
        wins_driver_id: leader_by(rows, |row| row.vitorias),
        titles_driver_id: leader_by(rows, |row| row.titulos),
        injuries_driver_id: leader_by(rows, |row| row.lesoes),
    }
}

fn has_competitive_history(row: &GlobalDriverRankingRow) -> bool {
    row.historical_index > 0.0
        || row.corridas > 0
        || row.pontos > 0
        || row.titulos > 0
        || row.vitorias > 0
        || row.podios > 0
        || row.poles > 0
        || row.dnfs > 0
}

fn has_ranking_visibility(row: &GlobalDriverRankingRow) -> bool {
    has_competitive_history(row) || is_current_regular_grid_driver(row)
}

fn is_current_regular_grid_driver(row: &GlobalDriverRankingRow) -> bool {
    row.status == "Ativo" && row.categoria_atual.is_some() && row.equipe_nome.is_some()
}

fn leader_by(
    rows: &[GlobalDriverRankingRow],
    metric: fn(&GlobalDriverRankingRow) -> i32,
) -> Option<String> {
    rows.iter()
        .max_by(|a, b| metric(a).cmp(&metric(b)).then_with(|| b.nome.cmp(&a.nome)))
        .map(|row| row.id.clone())
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
        params![table],
        |_| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(|e| format!("Falha ao verificar tabela {table}: {e}"))
}

fn json_i32(value: &Value, key: &str) -> i32 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0) as i32
}

fn json_i32_option(value: &Value, key: &str) -> Option<i32> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .map(|value| value as i32)
}

fn valid_archived_title_count(
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

fn has_title_worthy_participation(
    points: f64,
    wins: i32,
    podiums: i32,
    poles: i32,
    races: i32,
) -> bool {
    races > 0 && (points > 0.0 || wins > 0 || podiums > 0 || poles > 0)
}

fn valid_archived_title_count_for_pilot(
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

fn valid_archived_title_categories_for_pilot(
    conn: &Connection,
    driver_id: &str,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
) -> Result<Option<Vec<GlobalDriverTitleCategorySummary>>, String> {
    let mut saw_archive = false;
    let mut totals = HashMap::<(String, Option<String>), (i32, Vec<i32>)>::new();
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
                let total = totals.entry((category, class_name)).or_default();
                total.0 += titles;
                total.1.extend(title_years_for_event(titles, year));
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
        .map(|((categoria, classe), (titulos, mut anos))| {
            sort_title_years(&mut anos);
            GlobalDriverTitleCategorySummary {
                categoria,
                classe,
                titulos,
                anos,
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

fn team_champion_title_stats_for_driver(
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

fn push_team_title_stat(
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

fn load_all_team_champion_title_stats(conn: &Connection) -> Result<TeamTitleStatsByDriver, String> {
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
                title_years: vec![year],
                dnfs: 0,
            },
        );
    }

    Ok(stats_by_driver)
}

fn load_all_special_class_champion_title_stats(
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
                title_years: vec![champion.year],
                dnfs: 0,
            },
        );
    }

    Ok(stats_by_driver)
}

fn compare_special_team_title_candidates(
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

fn title_event_key(season_number: i32, category: &str, class_name: Option<&str>) -> TitleEventKey {
    (
        season_number,
        category.trim().to_string(),
        class_name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    )
}

fn uses_team_archive_title_fallback(category: &str) -> bool {
    matches!(category, "production_challenger" | "endurance")
}

fn best_team_title_driver_id(
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

fn team_title_driver_candidates(
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

fn team_title_driver_score(
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

fn compare_team_title_driver_scores(
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

fn title_years_for_event(titles: i32, year: i32) -> Vec<i32> {
    if titles > 0 && year > 0 {
        vec![year]
    } else {
        Vec::new()
    }
}

fn sort_title_years(years: &mut Vec<i32>) {
    years.sort_unstable_by(|left, right| right.cmp(left));
    years.dedup();
}

fn normalized_archive_category(snapshot: &Value, fallback: String) -> String {
    let category = json_string(snapshot, "categoria")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback);
    if category.trim().is_empty() {
        "unknown".to_string()
    } else {
        category
    }
}

fn clean_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn archived_title_class(
    conn: &Connection,
    driver_id: &str,
    category: &str,
    season_number: i32,
    snapshot: &Value,
) -> Result<Option<String>, String> {
    if let Some(class_name) = snapshot_class(snapshot) {
        return Ok(Some(class_name));
    }

    if let Some(team_id) = json_string(snapshot, "team_id").filter(|value| !value.trim().is_empty())
    {
        if let Some(class_name) = archived_team_class(conn, &team_id, category, season_number)? {
            return Ok(Some(class_name));
        }
        if let Some(class_name) =
            archived_special_entry_class(conn, &team_id, category, season_number)?
        {
            return Ok(Some(class_name));
        }
    }

    archived_contract_class(conn, driver_id, category, season_number)
}

fn snapshot_class(snapshot: &Value) -> Option<String> {
    ["classe", "class_name", "special_class"]
        .iter()
        .find_map(|key| json_string(snapshot, key))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn archived_team_class(
    conn: &Connection,
    team_id: &str,
    category: &str,
    season_number: i32,
) -> Result<Option<String>, String> {
    if !table_exists(conn, "team_season_archive")? {
        return Ok(None);
    }

    conn.query_row(
        "SELECT classe
         FROM team_season_archive
         WHERE team_id = ?1
           AND season_number = ?2
           AND categoria = ?3
           AND classe IS NOT NULL
           AND TRIM(classe) <> ''
         LIMIT 1",
        params![team_id, season_number, category],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| format!("Falha ao buscar classe historica da equipe: {e}"))
}

fn archived_special_entry_class(
    conn: &Connection,
    team_id: &str,
    category: &str,
    season_number: i32,
) -> Result<Option<String>, String> {
    if !table_exists(conn, "special_team_entries")? || !table_exists(conn, "seasons")? {
        return Ok(None);
    }

    conn.query_row(
        "SELECT e.class_name
         FROM special_team_entries e
         INNER JOIN seasons s ON s.id = e.season_id
         WHERE e.team_id = ?1
           AND e.special_category = ?2
           AND s.numero = ?3
           AND e.class_name IS NOT NULL
           AND TRIM(e.class_name) <> ''
         ORDER BY e.class_name
         LIMIT 1",
        params![team_id, category, season_number],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| format!("Falha ao buscar classe historica da inscricao especial: {e}"))
}

fn archived_contract_class(
    conn: &Connection,
    driver_id: &str,
    category: &str,
    season_number: i32,
) -> Result<Option<String>, String> {
    if !table_exists(conn, "contracts")? {
        return Ok(None);
    }

    conn.query_row(
        "SELECT classe
         FROM contracts
         WHERE piloto_id = ?1
           AND categoria = ?2
           AND classe IS NOT NULL
           AND TRIM(classe) <> ''
           AND CAST(temporada_inicio AS INTEGER) <= ?3
           AND CAST(temporada_fim AS INTEGER) >= ?3
         ORDER BY tipo DESC, created_at DESC
         LIMIT 1",
        params![driver_id, category, season_number],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| format!("Falha ao buscar classe historica do contrato: {e}"))
}

fn json_f64(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn json_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn round_one(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn years_since(start_year: i32, current_year: i32) -> Option<i32> {
    if start_year <= 0 || current_year <= 0 || current_year < start_year {
        return None;
    }
    Some(current_year - start_year + 1)
}

fn parse_year(value: &str) -> Option<i32> {
    parse_positive_i32(value).filter(|year| *year >= 1900)
}

fn parse_positive_i32(value: &str) -> Option<i32> {
    value.trim().parse::<i32>().ok().filter(|year| *year > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::run_all;
    use crate::db::queries::drivers::insert_driver;
    use crate::db::queries::seasons::insert_season;
    use crate::db::queries::teams::insert_team;
    use crate::models::driver::Driver;
    use crate::models::enums::{DriverStatus, InjuryType};
    use crate::models::injury::Injury;
    use crate::models::season::Season;
    use crate::models::team::placeholder_team_from_db;
    use rusqlite::Connection;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        run_all(&conn).expect("migrations");
        conn
    }

    fn driver_with_stats(
        id: &str,
        name: &str,
        category: Option<&str>,
        wins: u32,
        podiums: u32,
        titles: u32,
    ) -> Driver {
        let mut driver = Driver::new(
            id.to_string(),
            name.to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            28,
            2020,
        );
        driver.categoria_atual = category.map(str::to_string);
        driver.stats_carreira.vitorias = wins;
        driver.stats_carreira.podios = podiums;
        driver.stats_carreira.titulos = titles;
        driver.stats_carreira.poles = wins / 2;
        driver.stats_carreira.corridas = wins.max(1) * 4;
        driver.stats_carreira.pontos_total = f64::from(wins * 25 + podiums * 12);
        driver
    }

    fn insert_active_regular_contract(
        conn: &Connection,
        contract_id: &str,
        driver_id: &str,
        driver_name: &str,
        category: &str,
    ) {
        let team_id = format!("T_{contract_id}");
        insert_team(
            conn,
            &placeholder_team_from_db(
                team_id.clone(),
                format!("Equipe {contract_id}"),
                category.to_string(),
                "2026-01-01".to_string(),
            ),
        )
        .expect("insert active contract team");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome, temporada_inicio,
                duracao_anos, temporada_fim, salario, salario_anual, papel, status, tipo, categoria, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 1, 1, 10000, 10000, 'Numero1', 'Ativo', 'Regular', ?6, '2026-01-01')",
            rusqlite::params![
                contract_id,
                driver_id,
                driver_name,
                team_id,
                format!("Equipe {contract_id}"),
                category,
            ],
        )
        .expect("insert active regular contract");
    }

    #[test]
    fn balanced_index_weights_higher_categories_without_erasing_lower_category_dominance() {
        let conn = setup_conn();
        insert_driver(
            &conn,
            &driver_with_stats("D_GT3", "GT3 Forte", Some("gt3"), 2, 3, 0),
        )
        .expect("insert gt3");
        insert_driver(
            &conn,
            &driver_with_stats("D_ROOKIE", "Rookie Forte", Some("mazda_rookie"), 2, 3, 0),
        )
        .expect("insert rookie");
        insert_driver(
            &conn,
            &driver_with_stats("D_DOM", "Rookie Dominante", Some("mazda_rookie"), 12, 16, 1),
        )
        .expect("insert dominant");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let gt3 = payload.rows.iter().find(|row| row.id == "D_GT3").unwrap();
        let rookie = payload
            .rows
            .iter()
            .find(|row| row.id == "D_ROOKIE")
            .unwrap();
        let dominant = payload.rows.iter().find(|row| row.id == "D_DOM").unwrap();

        assert!(gt3.historical_index > rookie.historical_index);
        assert!(dominant.historical_index > gt3.historical_index);
    }

    #[test]
    fn balanced_index_treats_podium_volume_without_wins_as_consistency_not_greatness() {
        let podium_collector = balanced_score("gt3", 0, 0, 178, 0, 3000.0, 391, 8);
        let proven_winner = balanced_score("gt3", 0, 20, 35, 10, 1200.0, 90, 4);

        assert!(
            proven_winner > podium_collector,
            "Declan Gauthier-like career should not outrank a frequent winner: winner={proven_winner}, podium_collector={podium_collector}"
        );
    }

    #[test]
    fn balanced_index_keeps_titles_above_large_win_totals() {
        let champion = balanced_score("gt3", 1, 6, 12, 3, 900.0, 40, 1);
        let non_champion_winner = balanced_score("gt3", 0, 12, 25, 6, 1500.0, 60, 2);

        assert!(
            champion > non_champion_winner,
            "historical index should treat titles as the top achievement: champion={champion}, non_champion={non_champion_winner}"
        );
    }

    #[test]
    fn payload_includes_active_free_and_retired_drivers_with_dimmed_statuses() {
        let conn = setup_conn();
        let active = driver_with_stats("D_ACTIVE", "Piloto Ativo", Some("gt4"), 3, 5, 0);
        let free = driver_with_stats("D_FREE", "Piloto Livre", None, 1, 2, 0);
        insert_driver(&conn, &active).expect("insert active");
        insert_driver(&conn, &free).expect("insert free");
        insert_active_regular_contract(&conn, "C_ACTIVE", "D_ACTIVE", "Piloto Ativo", "gt4");
        conn.execute(
            "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "D_RET",
                "Lenda Aposentada",
                "2025",
                "gt3",
                r#"{"vitorias": 7, "podios": 12, "titulos": 1, "corridas": 30, "pontos": 220}"#,
                "Aposentadoria"
            ],
        )
        .expect("insert retired");

        let payload = build_global_driver_rankings(&conn, Some("D_FREE")).expect("payload");
        let active = payload
            .rows
            .iter()
            .find(|row| row.id == "D_ACTIVE")
            .unwrap();
        let free = payload.rows.iter().find(|row| row.id == "D_FREE").unwrap();
        let retired = payload.rows.iter().find(|row| row.id == "D_RET").unwrap();

        assert_eq!(payload.selected_driver_id.as_deref(), Some("D_FREE"));
        assert_eq!(active.status, "Ativo");
        assert_eq!(free.status, "Livre");
        assert_eq!(retired.status, "Aposentado");
        assert_eq!(free.status_tone, "dimmed");
        assert_eq!(retired.status_tone, "retired");
    }

    #[test]
    fn payload_marks_driver_without_active_regular_contract_as_free_even_with_last_category() {
        let conn = setup_conn();
        let free = driver_with_stats("D_FREE_STALE", "Livre Com Categoria", Some("bmw_m2"), 1, 2, 0);
        insert_driver(&conn, &free).expect("insert free");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_FREE_STALE")
            .expect("free driver should remain ranked by history");

        assert_eq!(row.status, "Livre");
        assert_eq!(row.status_tone, "dimmed");
        assert_eq!(row.categoria_atual.as_deref(), Some("bmw_m2"));
    }

    #[test]
    fn payload_keeps_current_contracted_driver_without_competitive_history() {
        let conn = setup_conn();
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_ROOKIE".to_string(),
                "Equipe Rookie".to_string(),
                "mazda_rookie".to_string(),
                "2026-01-01".to_string(),
            ),
        )
        .expect("insert team");
        let mut rookie = driver_with_stats("D_ROOKIE_ZERO", "Rookie Sem Historico", Some("mazda_rookie"), 0, 0, 0);
        rookie.stats_carreira.corridas = 0;
        rookie.stats_carreira.pontos_total = 0.0;
        insert_driver(&conn, &rookie).expect("insert rookie");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome, temporada_inicio,
                duracao_anos, temporada_fim, salario, salario_anual, papel, status, tipo, categoria, created_at
            ) VALUES (
                'C_ROOKIE_ZERO', 'D_ROOKIE_ZERO', 'Rookie Sem Historico', 'T_ROOKIE', 'Equipe Rookie', 1,
                1, 1, 10000, 10000, 'Numero1', 'Ativo', 'Regular', 'mazda_rookie', '2026-01-01'
            )",
            [],
        )
        .expect("insert active contract");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_ROOKIE_ZERO")
            .expect("current contracted rookie should be visible");

        assert_eq!(row.status, "Ativo");
        assert_eq!(row.corridas, 0);
        assert_eq!(row.historical_index, 0.0);
    }

    #[test]
    fn payload_keeps_player_driver_available_when_not_ranked() {
        let conn = setup_conn();
        insert_driver(
            &conn,
            &driver_with_stats("D_RANKED", "Piloto Ranqueado", Some("gt4"), 2, 3, 0),
        )
        .expect("insert ranked");
        let mut player = Driver::new(
            "D_PLAYER".to_string(),
            "Piloto Usuario".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            21,
            2026,
        );
        player.is_jogador = true;
        player.categoria_atual = Some("mazda_rookie".to_string());
        insert_driver(&conn, &player).expect("insert player");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");

        assert!(payload.rows.iter().all(|row| row.id != "D_PLAYER"));
        let player_row = payload.player_driver.as_ref().expect("player driver");
        assert_eq!(player_row.id, "D_PLAYER");
        assert_eq!(player_row.nome, "Piloto Usuario");
        assert!(player_row.is_jogador);
    }

    #[test]
    fn injuries_are_reported_but_do_not_reduce_historical_index() {
        let mut conn = setup_conn();
        let no_injury = driver_with_stats("D_SAFE", "Seguro", Some("gt4"), 4, 8, 0);
        let injured = driver_with_stats("D_INJ", "Lesionado", Some("gt4"), 4, 8, 0);
        insert_driver(&conn, &no_injury).expect("insert safe");
        insert_driver(&conn, &injured).expect("insert injured");
        let tx = conn.transaction().expect("tx");
        crate::db::queries::injuries::insert_injury(
            &tx,
            &Injury {
                id: "I_INJ".to_string(),
                pilot_id: "D_INJ".to_string(),
                injury_type: InjuryType::Moderada,
                injury_name: "Ombro".to_string(),
                modifier: 0.9,
                races_total: 2,
                races_remaining: 1,
                skill_penalty: 0.1,
                season: 1,
                race_occurred: "R001".to_string(),
                active: true,
            },
        )
        .expect("insert injury");
        tx.commit().expect("commit");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let safe = payload.rows.iter().find(|row| row.id == "D_SAFE").unwrap();
        let injured = payload.rows.iter().find(|row| row.id == "D_INJ").unwrap();

        assert_eq!(safe.historical_index, injured.historical_index);
        assert_eq!(safe.lesoes, 0);
        assert_eq!(injured.lesoes, 1);
    }

    #[test]
    fn injured_active_driver_keeps_active_status_label() {
        let mut conn = setup_conn();
        let mut injured =
            driver_with_stats("D_INJ_STATUS", "Piloto Lesionado", Some("gt4"), 4, 8, 0);
        injured.status = DriverStatus::Lesionado;
        insert_driver(&conn, &injured).expect("insert injured");
        insert_active_regular_contract(&conn, "C_INJ_STATUS", "D_INJ_STATUS", "Piloto Lesionado", "gt4");
        let tx = conn.transaction().expect("tx");
        crate::db::queries::injuries::insert_injury(
            &tx,
            &Injury {
                id: "I_INJ_STATUS".to_string(),
                pilot_id: "D_INJ_STATUS".to_string(),
                injury_type: InjuryType::Moderada,
                injury_name: "Ombro".to_string(),
                modifier: 0.9,
                races_total: 2,
                races_remaining: 1,
                skill_penalty: 0.1,
                season: 1,
                race_occurred: "R001".to_string(),
                active: true,
            },
        )
        .expect("insert injury");
        tx.commit().expect("commit");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_INJ_STATUS")
            .unwrap();

        assert_eq!(row.status, "Ativo");
        assert_eq!(row.status_tone, "active");
        assert!(row.is_lesionado);
        assert_eq!(row.lesao_ativa_tipo.as_deref(), Some("Moderada"));
    }

    #[test]
    fn payload_includes_salary_career_and_retirement_context() {
        let conn = setup_conn();
        conn.execute("DELETE FROM seasons", [])
            .expect("clear seeded seasons");
        insert_season(&conn, &Season::new("S_OLD".to_string(), 1, 2024))
            .expect("insert previous season");
        season_queries::finalize_season(&conn, "S_OLD").expect("finalize previous season");
        insert_season(&conn, &Season::new("S_TEST".to_string(), 2, 2026))
            .expect("insert active season");

        let mut active = driver_with_stats("D_ACTIVE", "Piloto Ativo", Some("gt4"), 3, 5, 0);
        active.ano_inicio_carreira = 2020;
        insert_driver(&conn, &active).expect("insert active");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_GT4".to_string(),
                "Equipe Azul".to_string(),
                "gt4".to_string(),
                "2026-01-01".to_string(),
            ),
        )
        .expect("insert team");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome, temporada_inicio,
                duracao_anos, temporada_fim, salario, salario_anual, papel, status, tipo, categoria, created_at
            ) VALUES (
                'C_ACTIVE', 'D_ACTIVE', 'Piloto Ativo', 'T_GT4', 'Equipe Azul', 2,
                1, 2, 250000, 250000, 'Numero1', 'Ativo', 'Regular', 'gt4', CURRENT_TIMESTAMP
            )",
            [],
        )
        .expect("insert contract");

        conn.execute(
            "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "D_RET",
                "Lenda Aposentada",
                "2024",
                "gt3",
                r#"{"vitorias": 7, "podios": 12, "titulos": 1, "corridas": 30, "pontos": 220, "ano_inicio_carreira": 2018}"#,
                "Aposentadoria"
            ],
        )
        .expect("insert retired");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let active = payload
            .rows
            .iter()
            .find(|row| row.id == "D_ACTIVE")
            .unwrap();
        let retired = payload.rows.iter().find(|row| row.id == "D_RET").unwrap();

        assert_eq!(active.salario_anual, Some(250000.0));
        assert_eq!(active.ano_inicio_carreira, Some(2020));
        assert_eq!(active.anos_carreira, Some(7));
        assert_eq!(retired.temporada_aposentadoria.as_deref(), Some("2024"));
        assert_eq!(retired.anos_aposentado, Some(2));
        assert_eq!(retired.anos_carreira, Some(7));
    }

    #[test]
    fn active_driver_debut_year_uses_earliest_competitive_archive_entry() {
        let conn = setup_conn();
        conn.execute("DELETE FROM seasons", [])
            .expect("clear seeded seasons");
        insert_season(&conn, &Season::new("S_TEST".to_string(), 1, 2025))
            .expect("insert active season");

        let mut driver =
            driver_with_stats("D_ARCHIVE_START", "Arquivo Antigo", Some("gt3"), 1, 2, 0);
        driver.ano_inicio_carreira = 2024;
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES
                ('D_ARCHIVE_START', 23, 2022, 'Arquivo Antigo', 'mazda_rookie', 4, 67.0,
                 '{\"categoria\":\"mazda_rookie\",\"corridas\":5,\"pontos\":67,\"vitorias\":0,\"podios\":2}'),
                ('D_ARCHIVE_START', 24, 2023, 'Arquivo Antigo', '', NULL, 0.0,
                 '{\"categoria\":\"\",\"corridas\":0,\"pontos\":0,\"vitorias\":0,\"podios\":0}'),
                ('D_ARCHIVE_START', 25, 2024, 'Arquivo Antigo', 'gt3', 25, 0.0,
                 '{\"categoria\":\"gt3\",\"corridas\":14,\"pontos\":0,\"vitorias\":0,\"podios\":0}')",
            [],
        )
        .expect("insert archive");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_ARCHIVE_START")
            .unwrap();

        assert_eq!(row.ano_inicio_carreira, Some(2022));
        assert_eq!(row.anos_carreira, Some(4));
    }

    #[test]
    fn active_driver_current_category_uses_regular_career_over_special_contract() {
        let conn = setup_conn();
        let mut driver =
            driver_with_stats("D_SPECIAL_ACTIVE", "Especial Ativo", Some("gt3"), 2, 3, 0);
        driver.ano_inicio_carreira = 2024;
        insert_driver(&conn, &driver).expect("insert driver");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_GT3".to_string(),
                "Equipe GT3".to_string(),
                "gt3".to_string(),
                "2026-01-01".to_string(),
            ),
        )
        .expect("insert regular team");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_END".to_string(),
                "Equipe Endurance".to_string(),
                "endurance".to_string(),
                "2026-01-01".to_string(),
            ),
        )
        .expect("insert special team");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome, temporada_inicio,
                duracao_anos, temporada_fim, salario, salario_anual, papel, status, tipo, categoria, created_at
            ) VALUES
                ('C_REG', 'D_SPECIAL_ACTIVE', 'Especial Ativo', 'T_GT3', 'Equipe GT3', 1,
                 2, 2, 150000, 150000, 'Numero1', 'Ativo', 'Regular', 'gt3', '2026-01-01'),
                ('C_SPEC', 'D_SPECIAL_ACTIVE', 'Especial Ativo', 'T_END', 'Equipe Endurance', 2,
                 1, 2, 50000, 50000, 'Numero1', 'Ativo', 'Especial', 'endurance', '2026-02-01')",
            [],
        )
        .expect("insert contracts");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_SPECIAL_ACTIVE")
            .unwrap();

        assert_eq!(row.categoria_atual.as_deref(), Some("gt3"));
        assert_eq!(row.equipe_nome.as_deref(), Some("Equipe GT3"));
        assert_eq!(row.salario_anual, Some(150000.0));
    }

    #[test]
    fn active_driver_current_category_ignores_contaminated_special_category_field() {
        let conn = setup_conn();
        let mut driver = driver_with_stats(
            "D_BAD_CURRENT",
            "Categoria Contaminada",
            Some("endurance"),
            1,
            2,
            0,
        );
        driver.ano_inicio_carreira = 2024;
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (
                'D_BAD_CURRENT', 25, 2024, 'Categoria Contaminada', 'gt3', 8, 80.0,
                '{\"categoria\":\"gt3\",\"corridas\":10,\"pontos\":80,\"vitorias\":1,\"podios\":2}'
            )",
            [],
        )
        .expect("insert archive");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_BAD_CURRENT")
            .unwrap();

        assert_ne!(row.categoria_atual.as_deref(), Some("endurance"));
    }

    #[test]
    fn retired_driver_points_fall_back_to_career_points_total_snapshot_field() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "D_RET_POINTS",
                "Aposentado Com Pontos",
                "2025",
                "gt3",
                r#"{"vitorias": 3, "podios": 10, "titulos": 0, "corridas": 40, "pontos_total": 612.5}"#,
                "Aposentadoria"
            ],
        )
        .expect("insert retired");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let retired = payload
            .rows
            .iter()
            .find(|row| row.id == "D_RET_POINTS")
            .unwrap();

        assert_eq!(retired.pontos, 613);
    }

    #[test]
    fn retired_driver_career_years_fall_back_to_career_seasons_snapshot_field() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "D_RET_YEARS",
                "Aposentado Com Duracao",
                "2025",
                "gt3",
                r#"{"vitorias": 3, "podios": 10, "titulos": 0, "corridas": 40, "temporadas": 18}"#,
                "Aposentadoria"
            ],
        )
        .expect("insert retired");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let retired = payload
            .rows
            .iter()
            .find(|row| row.id == "D_RET_YEARS")
            .unwrap();

        assert_eq!(retired.anos_carreira, Some(18));
    }

    #[test]
    fn retired_driver_titles_ignore_archived_zero_race_championships() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "D_RET_ZERO_TITLE",
                "Aposentado Sem Corrida Campeao",
                "2025",
                "endurance",
                r#"{"vitorias": 0, "podios": 0, "titulos": 5, "corridas": 135, "pontos_total": 2.0}"#,
                "Aposentadoria"
            ],
        )
        .expect("insert retired");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "D_RET_ZERO_TITLE",
                1,
                2024,
                "Aposentado Sem Corrida Campeao",
                "endurance",
                1,
                0.0,
                r#"{"categoria":"endurance","posicao_campeonato":1,"titulos":1,"corridas":0,"pontos":0,"vitorias":0,"podios":0}"#
            ],
        )
        .expect("insert invalid archive title");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let retired = payload
            .rows
            .iter()
            .find(|row| row.id == "D_RET_ZERO_TITLE")
            .unwrap();

        assert_eq!(retired.titulos, 0);
    }

    #[test]
    fn retired_driver_title_breakdown_uses_archived_winning_categories() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "D_RET_BREAKDOWN",
                "Aposentado Multiclasse",
                "2025",
                "gt3",
                r#"{"vitorias": 10, "podios": 20, "titulos": 3, "corridas": 80, "pontos_total": 900}"#,
                "Aposentadoria"
            ],
        )
        .expect("insert retired");
        for (season_number, category, points, snapshot_json) in [
            (
                1,
                "gt4",
                120.0,
                r#"{"categoria":"gt4","posicao_campeonato":1,"titulos":1,"corridas":10,"pontos":120,"vitorias":2,"podios":5}"#,
            ),
            (
                2,
                "gt3",
                180.0,
                r#"{"categoria":"gt3","posicao_campeonato":1,"titulos":1,"corridas":12,"pontos":180,"vitorias":4,"podios":7}"#,
            ),
            (
                3,
                "gt3",
                190.0,
                r#"{"categoria":"gt3","posicao_campeonato":1,"titulos":1,"corridas":12,"pontos":190,"vitorias":4,"podios":8}"#,
            ),
        ] {
            conn.execute(
                "INSERT INTO driver_season_archive (
                    piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    "D_RET_BREAKDOWN",
                    season_number,
                    2020 + season_number,
                    "Aposentado Multiclasse",
                    category,
                    1,
                    points,
                    snapshot_json
                ],
            )
            .expect("insert archive");
        }

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let retired = payload
            .rows
            .iter()
            .find(|row| row.id == "D_RET_BREAKDOWN")
            .unwrap();

        assert_eq!(retired.titulos_por_categoria.len(), 2);
        assert_eq!(retired.titulos_por_categoria[0].categoria, "gt3");
        assert_eq!(retired.titulos_por_categoria[0].titulos, 2);
        assert_eq!(retired.titulos_por_categoria[1].categoria, "gt4");
        assert_eq!(retired.titulos_por_categoria[1].titulos, 1);
    }

    #[test]
    fn archived_zero_race_championship_position_does_not_count_as_title() {
        let conn = setup_conn();
        let driver = driver_with_stats(
            "D_ZERO_TITLE",
            "Campeao Sem Corrida",
            Some("endurance"),
            0,
            0,
            0,
        );
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "D_ZERO_TITLE",
                1,
                2024,
                "Campeao Sem Corrida",
                "endurance",
                1,
                0.0,
                r#"{"categoria":"endurance","posicao_campeonato":1,"titulos":1,"corridas":0,"pontos":0,"vitorias":0,"podios":0}"#
            ],
        )
        .expect("insert invalid archive title");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "D_ZERO_TITLE",
                2,
                2025,
                "Campeao Sem Corrida",
                "gt3",
                4,
                20.0,
                r#"{"categoria":"gt3","posicao_campeonato":4,"titulos":0,"corridas":10,"pontos":20,"vitorias":0,"podios":0}"#
            ],
        )
        .expect("insert valid archive history");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_ZERO_TITLE")
            .unwrap();

        assert_eq!(row.titulos, 0);
    }

    #[test]
    fn payload_groups_titles_by_won_category() {
        let conn = setup_conn();
        let driver = driver_with_stats(
            "D_TITLE_BREAKDOWN",
            "Campeao Multiclasse",
            Some("gt3"),
            3,
            5,
            0,
        );
        insert_driver(&conn, &driver).expect("insert driver");
        for (season_number, category, position, points, snapshot_json) in [
            (
                1,
                "gt4",
                Some(1),
                160.0,
                r#"{"categoria":"gt4","posicao_campeonato":1,"titulos":1,"corridas":10,"pontos":160,"vitorias":3,"podios":6}"#,
            ),
            (
                2,
                "gt3",
                Some(1),
                190.0,
                r#"{"categoria":"gt3","posicao_campeonato":1,"titulos":1,"corridas":12,"pontos":190,"vitorias":4,"podios":7}"#,
            ),
            (
                3,
                "gt3",
                Some(1),
                210.0,
                r#"{"categoria":"gt3","posicao_campeonato":1,"titulos":1,"corridas":12,"pontos":210,"vitorias":5,"podios":8}"#,
            ),
            (
                4,
                "endurance",
                Some(1),
                0.0,
                r#"{"categoria":"endurance","posicao_campeonato":1,"titulos":1,"corridas":0,"pontos":0,"vitorias":0,"podios":0}"#,
            ),
        ] {
            conn.execute(
                "INSERT INTO driver_season_archive (
                    piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    "D_TITLE_BREAKDOWN",
                    season_number,
                    2020 + season_number,
                    "Campeao Multiclasse",
                    category,
                    position,
                    points,
                    snapshot_json
                ],
            )
            .expect("insert archive");
        }

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_TITLE_BREAKDOWN")
            .unwrap();

        assert_eq!(row.titulos, 3);
        assert_eq!(row.titulos_por_categoria.len(), 2);
        assert_eq!(row.titulos_por_categoria[0].categoria, "gt3");
        assert_eq!(row.titulos_por_categoria[0].titulos, 2);
        assert_eq!(row.titulos_por_categoria[0].anos, vec![2023, 2022]);
        assert_eq!(row.titulos_por_categoria[1].categoria, "gt4");
        assert_eq!(row.titulos_por_categoria[1].titulos, 1);
        assert_eq!(row.titulos_por_categoria[1].anos, vec![2021]);
    }

    #[test]
    fn payload_groups_special_titles_by_category_and_class() {
        let conn = setup_conn();
        let driver = driver_with_stats(
            "D_SPECIAL_TITLE_BREAKDOWN",
            "Campeao Production",
            Some("production_challenger"),
            4,
            8,
            0,
        );
        insert_driver(&conn, &driver).expect("insert driver");
        for (season_number, class_name, points, snapshot_json) in [
            (
                1,
                "mazda",
                160.0,
                r#"{"categoria":"production_challenger","classe":"mazda","posicao_campeonato":1,"titulos":1,"corridas":10,"pontos":160,"vitorias":3,"podios":6}"#,
            ),
            (
                2,
                "toyota",
                180.0,
                r#"{"categoria":"production_challenger","classe":"toyota","posicao_campeonato":1,"titulos":1,"corridas":12,"pontos":180,"vitorias":4,"podios":7}"#,
            ),
        ] {
            conn.execute(
                "INSERT INTO driver_season_archive (
                    piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    "D_SPECIAL_TITLE_BREAKDOWN",
                    season_number,
                    2020 + season_number,
                    "Campeao Production",
                    "production_challenger",
                    1,
                    points,
                    snapshot_json
                ],
            )
            .expect("insert archive");
            assert_eq!(
                class_name,
                json_string(&serde_json::from_str(snapshot_json).unwrap(), "classe").unwrap()
            );
        }

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_SPECIAL_TITLE_BREAKDOWN")
            .unwrap();

        assert_eq!(row.titulos, 2);
        assert_eq!(row.titulos_por_categoria.len(), 2);
        assert_eq!(
            row.titulos_por_categoria[0].categoria,
            "production_challenger"
        );
        assert_eq!(
            row.titulos_por_categoria[0].classe.as_deref(),
            Some("mazda")
        );
        assert_eq!(row.titulos_por_categoria[0].titulos, 1);
        assert_eq!(
            row.titulos_por_categoria[1].categoria,
            "production_challenger"
        );
        assert_eq!(
            row.titulos_por_categoria[1].classe.as_deref(),
            Some("toyota")
        );
        assert_eq!(row.titulos_por_categoria[1].titulos, 1);
    }

    #[test]
    fn archived_special_title_class_falls_back_to_team_archive() {
        let conn = setup_conn();
        let driver = driver_with_stats(
            "D_TEAM_CLASS_TITLE",
            "Campeao Endurance",
            Some("endurance"),
            5,
            9,
            0,
        );
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "INSERT INTO team_season_archive (
                team_id, season_number, ano, categoria, classe, posicao_campeonato,
                pontos, vitorias, podios, poles, corridas, titulos_construtores, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                "T_LMP2",
                3,
                2023,
                "endurance",
                "lmp2",
                1,
                220.0,
                5,
                8,
                2,
                12,
                1,
                r#"{"classe":"lmp2"}"#
            ],
        )
        .expect("insert team archive");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "D_TEAM_CLASS_TITLE",
                3,
                2023,
                "Campeao Endurance",
                "endurance",
                1,
                200.0,
                r#"{"categoria":"endurance","team_id":"T_LMP2","posicao_campeonato":1,"titulos":1,"corridas":12,"pontos":200,"vitorias":5,"podios":8}"#
            ],
        )
        .expect("insert driver archive");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_TEAM_CLASS_TITLE")
            .unwrap();

        assert_eq!(row.titulos, 1);
        assert_eq!(row.titulos_por_categoria.len(), 1);
        assert_eq!(row.titulos_por_categoria[0].categoria, "endurance");
        assert_eq!(row.titulos_por_categoria[0].classe.as_deref(), Some("lmp2"));
    }

    #[test]
    fn payload_counts_special_team_champion_title_for_driver() {
        let conn = setup_conn();
        insert_season(
            &conn,
            &Season::new("S_PRODUCTION_TITLE".to_string(), 4, 2024),
        )
        .expect("insert season");
        let driver = driver_with_stats(
            "D_TEAM_PRODUCTION_TITLE",
            "Campeao Production Equipe",
            Some("production_challenger"),
            0,
            0,
            0,
        );
        insert_driver(&conn, &driver).expect("insert driver");
        insert_driver(
            &conn,
            &driver_with_stats(
                "D_TEAMMATE",
                "Colega Production",
                Some("production_challenger"),
                0,
                0,
                0,
            ),
        )
        .expect("insert teammate");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_PRODUCTION".to_string(),
                "Equipe Production".to_string(),
                "production_challenger".to_string(),
                "2024-01-01".to_string(),
            ),
        )
        .expect("insert team");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome, temporada_inicio,
                duracao_anos, temporada_fim, salario, salario_anual, papel, status, tipo,
                categoria, classe, created_at
            ) VALUES (
                'C_TEAM_PRODUCTION_TITLE', 'D_TEAM_PRODUCTION_TITLE', 'Campeao Production Equipe',
                'T_PRODUCTION', 'Equipe Production', 4, 1, 4, 120000, 120000, 'Numero1',
                'Expirado', 'Especial', 'production_challenger', 'mazda', CURRENT_TIMESTAMP
            )",
            [],
        )
        .expect("insert special contract");
        conn.execute(
            "INSERT INTO team_season_archive (
                team_id, season_number, ano, categoria, classe, posicao_campeonato,
                pontos, vitorias, podios, poles, corridas, titulos_construtores,
                piloto_1_id, piloto_2_id, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            rusqlite::params![
                "T_PRODUCTION",
                4,
                2024,
                "production_challenger",
                Option::<String>::None,
                1,
                341.0,
                6,
                10,
                3,
                12,
                1,
                "D_TEAM_PRODUCTION_TITLE",
                "D_TEAMMATE",
                r#"{"categoria":"production_challenger","posicao_campeonato":1}"#
            ],
        )
        .expect("insert team archive");
        conn.execute(
            "INSERT INTO calendar (id, temporada_id, rodada, pista, categoria, clima, duracao, data)
             VALUES ('R_PRODUCTION_1', 'S_PRODUCTION_TITLE', 1, 'Interlagos', 'production_challenger', 'Seco', 60, '2024-05-01')",
            [],
        )
        .expect("insert calendar");
        conn.execute(
            "INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final,
                voltas_completadas, dnf, pontos
             ) VALUES
                ('R_PRODUCTION_1', 'D_TEAM_PRODUCTION_TITLE', 'T_PRODUCTION', 1, 1, 20, 0, 25.0),
                ('R_PRODUCTION_1', 'D_TEAMMATE', 'T_PRODUCTION', 2, 2, 20, 0, 18.0)",
            [],
        )
        .expect("insert race results");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_TEAM_PRODUCTION_TITLE")
            .unwrap();

        assert_eq!(row.titulos, 1);
        assert_eq!(row.titulos_por_categoria.len(), 1);
        assert_eq!(
            row.titulos_por_categoria[0].categoria,
            "production_challenger"
        );
        assert_eq!(
            row.titulos_por_categoria[0].classe.as_deref(),
            Some("mazda")
        );
    }

    #[test]
    fn team_archive_title_counts_only_the_best_scoring_driver() {
        let conn = setup_conn();
        insert_season(&conn, &Season::new("S_SPECIAL_TITLE".to_string(), 4, 2024))
            .expect("insert season");
        let first_driver = driver_with_stats(
            "D_TEAM_TITLE_P1",
            "Colega Campeao",
            Some("endurance"),
            1,
            2,
            0,
        );
        let second_driver = driver_with_stats(
            "D_TEAM_TITLE_P2",
            "Campeao Individual",
            Some("endurance"),
            2,
            3,
            0,
        );
        insert_driver(&conn, &first_driver).expect("insert first driver");
        insert_driver(&conn, &second_driver).expect("insert second driver");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_ENDURANCE_GT3".to_string(),
                "Equipe Endurance".to_string(),
                "endurance".to_string(),
                "2024-01-01".to_string(),
            ),
        )
        .expect("insert team");
        conn.execute(
            "INSERT INTO team_season_archive (
                team_id, season_number, ano, categoria, classe, posicao_campeonato,
                pontos, vitorias, podios, poles, corridas, titulos_construtores,
                piloto_1_id, piloto_2_id, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            rusqlite::params![
                "T_ENDURANCE_GT3",
                4,
                2024,
                "endurance",
                "gt3",
                1,
                330.0,
                3,
                6,
                1,
                2,
                1,
                "D_TEAM_TITLE_P1",
                "D_TEAM_TITLE_P2",
                r#"{"categoria":"endurance","classe":"gt3","posicao_campeonato":1}"#
            ],
        )
        .expect("insert team archive");
        conn.execute(
            "INSERT INTO calendar (id, temporada_id, rodada, pista, categoria, clima, duracao, data)
             VALUES
                ('R_END_1', 'S_SPECIAL_TITLE', 1, 'Spa', 'endurance', 'Seco', 120, '2024-05-01'),
                ('R_END_2', 'S_SPECIAL_TITLE', 2, 'Le Mans', 'endurance', 'Seco', 120, '2024-06-01')",
            [],
        )
        .expect("insert calendar");
        conn.execute(
            "INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final,
                voltas_completadas, dnf, pontos
             ) VALUES
                ('R_END_1', 'D_TEAM_TITLE_P1', 'T_ENDURANCE_GT3', 2, 2, 20, 0, 18.0),
                ('R_END_2', 'D_TEAM_TITLE_P1', 'T_ENDURANCE_GT3', 2, 2, 20, 0, 18.0),
                ('R_END_1', 'D_TEAM_TITLE_P2', 'T_ENDURANCE_GT3', 1, 1, 20, 0, 25.0),
                ('R_END_2', 'D_TEAM_TITLE_P2', 'T_ENDURANCE_GT3', 1, 1, 20, 0, 25.0)",
            [],
        )
        .expect("insert race results");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let first = payload
            .rows
            .iter()
            .find(|row| row.id == "D_TEAM_TITLE_P1")
            .unwrap();
        let second = payload
            .rows
            .iter()
            .find(|row| row.id == "D_TEAM_TITLE_P2")
            .unwrap();

        assert_eq!(first.titulos, 0);
        assert_eq!(second.titulos, 1);
        assert_eq!(
            second.titulos_por_categoria[0].classe.as_deref(),
            Some("gt3")
        );
        assert_eq!(second.titulos_por_categoria[0].anos, vec![2024]);
    }

    #[test]
    fn special_class_entries_create_individual_champions_per_class() {
        let conn = setup_conn();
        insert_season(
            &conn,
            &Season::new("S_SPECIAL_CLASSES".to_string(), 5, 2025),
        )
        .expect("insert season");
        let bmw_driver = driver_with_stats(
            "D_PROD_BMW_CHAMP",
            "Campeao BMW",
            Some("production_challenger"),
            2,
            3,
            0,
        );
        let mazda_driver = driver_with_stats(
            "D_PROD_MAZDA_CHAMP",
            "Campeao Mazda",
            Some("production_challenger"),
            1,
            2,
            0,
        );
        insert_driver(&conn, &bmw_driver).expect("insert bmw driver");
        insert_driver(&conn, &mazda_driver).expect("insert mazda driver");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_PROD_BMW".to_string(),
                "Equipe BMW".to_string(),
                "production_challenger".to_string(),
                "2025-01-01".to_string(),
            ),
        )
        .expect("insert bmw team");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_PROD_MAZDA".to_string(),
                "Equipe Mazda".to_string(),
                "production_challenger".to_string(),
                "2025-01-01".to_string(),
            ),
        )
        .expect("insert mazda team");
        conn.execute(
            "INSERT INTO special_team_entries (
                season_id, special_category, class_name, team_id, source_category, qualified_via
             ) VALUES
                ('S_SPECIAL_CLASSES', 'production_challenger', 'bmw', 'T_PROD_BMW', 'bmw_m2', 'champion'),
                ('S_SPECIAL_CLASSES', 'production_challenger', 'mazda', 'T_PROD_MAZDA', 'mazda_amador', 'champion')",
            [],
        )
        .expect("insert special entries");
        conn.execute(
            "INSERT INTO calendar (id, temporada_id, season_id, rodada, pista, categoria, clima, duracao, data)
             VALUES
                ('R_PROD_BMW', 'S_SPECIAL_CLASSES', 'S_SPECIAL_CLASSES', 1, 'Interlagos', 'production_challenger', 'Seco', 60, '2025-05-01'),
                ('R_PROD_MAZDA', 'S_SPECIAL_CLASSES', 'S_SPECIAL_CLASSES', 1, 'Interlagos', 'production_challenger', 'Seco', 60, '2025-05-01')",
            [],
        )
        .expect("insert calendar");
        conn.execute(
            "INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final,
                voltas_completadas, dnf, pontos
             ) VALUES
                ('R_PROD_BMW', 'D_PROD_BMW_CHAMP', 'T_PROD_BMW', 1, 1, 20, 0, 25.0),
                ('R_PROD_MAZDA', 'D_PROD_MAZDA_CHAMP', 'T_PROD_MAZDA', 1, 1, 20, 0, 25.0)",
            [],
        )
        .expect("insert race results");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let bmw = payload
            .rows
            .iter()
            .find(|row| row.id == "D_PROD_BMW_CHAMP")
            .unwrap();
        let mazda = payload
            .rows
            .iter()
            .find(|row| row.id == "D_PROD_MAZDA_CHAMP")
            .unwrap();

        assert_eq!(bmw.titulos, 1);
        assert_eq!(bmw.titulos_por_categoria[0].classe.as_deref(), Some("bmw"));
        assert_eq!(mazda.titulos, 1);
        assert_eq!(
            mazda.titulos_por_categoria[0].classe.as_deref(),
            Some("mazda")
        );
    }

    #[test]
    fn regular_team_archive_does_not_create_driver_title() {
        let conn = setup_conn();
        insert_season(&conn, &Season::new("S_GT3_TEAM_TITLE".to_string(), 6, 2005))
            .expect("insert season");
        let individual_champion = driver_with_stats(
            "D_GT3_DRIVER_CHAMP",
            "Campeao Individual",
            Some("gt3"),
            2,
            4,
            0,
        );
        let team_champion_driver = driver_with_stats(
            "D_GT3_TEAM_DRIVER",
            "Piloto Equipe Campea",
            Some("gt3"),
            1,
            3,
            0,
        );
        let other_team_driver = driver_with_stats(
            "D_GT3_OTHER_DRIVER",
            "Colega Equipe Campea",
            Some("gt3"),
            1,
            2,
            0,
        );
        insert_driver(&conn, &individual_champion).expect("insert individual champion");
        insert_driver(&conn, &team_champion_driver).expect("insert team driver");
        insert_driver(&conn, &other_team_driver).expect("insert other team driver");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_GT3_TEAM_CHAMP".to_string(),
                "Equipe GT3 Campea".to_string(),
                "gt3".to_string(),
                "2005-01-01".to_string(),
            ),
        )
        .expect("insert team");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "D_GT3_DRIVER_CHAMP",
                6,
                2005,
                "Campeao Individual",
                "gt3",
                1,
                550.0,
                r#"{"categoria":"gt3","posicao_campeonato":1,"titulos":1,"corridas":20,"pontos":550,"vitorias":10,"podios":14}"#
            ],
        )
        .expect("insert driver archive");
        conn.execute(
            "INSERT INTO team_season_archive (
                team_id, season_number, ano, categoria, classe, posicao_campeonato,
                pontos, vitorias, podios, poles, corridas, titulos_construtores,
                piloto_1_id, piloto_2_id, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            rusqlite::params![
                "T_GT3_TEAM_CHAMP",
                6,
                2005,
                "gt3",
                Option::<String>::None,
                1,
                353.0,
                12,
                14,
                4,
                14,
                1,
                "D_GT3_TEAM_DRIVER",
                "D_GT3_OTHER_DRIVER",
                r#"{"categoria":"gt3","posicao_campeonato":1}"#
            ],
        )
        .expect("insert team archive");
        conn.execute(
            "INSERT INTO calendar (id, temporada_id, rodada, pista, categoria, clima, duracao, data)
             VALUES ('R_GT3_TEAM_1', 'S_GT3_TEAM_TITLE', 1, 'Spa', 'gt3', 'Seco', 60, '2005-05-01')",
            [],
        )
        .expect("insert calendar");
        conn.execute(
            "INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final,
                voltas_completadas, dnf, pontos
             ) VALUES
                ('R_GT3_TEAM_1', 'D_GT3_TEAM_DRIVER', 'T_GT3_TEAM_CHAMP', 1, 1, 20, 0, 25.0),
                ('R_GT3_TEAM_1', 'D_GT3_OTHER_DRIVER', 'T_GT3_TEAM_CHAMP', 2, 2, 20, 0, 18.0)",
            [],
        )
        .expect("insert race results");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let individual = payload
            .rows
            .iter()
            .find(|row| row.id == "D_GT3_DRIVER_CHAMP")
            .unwrap();
        let team_driver = payload
            .rows
            .iter()
            .find(|row| row.id == "D_GT3_TEAM_DRIVER")
            .unwrap();

        assert_eq!(individual.titulos, 1);
        assert_eq!(individual.titulos_por_categoria[0].anos, vec![2005]);
        assert_eq!(team_driver.titulos, 0);
    }

    #[test]
    fn retired_driver_keeps_retirement_context_when_still_present_in_drivers_table() {
        let conn = setup_conn();
        conn.execute("DELETE FROM seasons", [])
            .expect("clear seeded seasons");
        insert_season(&conn, &Season::new("S_OLD".to_string(), 1, 2024))
            .expect("insert previous season");
        season_queries::finalize_season(&conn, "S_OLD").expect("finalize previous season");
        insert_season(&conn, &Season::new("S_TEST".to_string(), 2, 2026))
            .expect("insert active season");

        let mut driver = driver_with_stats("D_RET_ACTIVE", "Aposentado Persistido", None, 0, 0, 0);
        driver.status = DriverStatus::Aposentado;
        driver.stats_carreira.corridas = 40;
        insert_driver(&conn, &driver).expect("insert retired driver");
        conn.execute(
            "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "D_RET_ACTIVE",
                "Aposentado Persistido",
                "1",
                "gt3",
                r#"{"vitorias": 8, "podios": 15, "titulos": 1, "corridas": 40, "pontos": 360, "ano_inicio_carreira": 2019}"#,
                "Aposentadoria"
            ],
        )
        .expect("insert retired snapshot");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_RET_ACTIVE")
            .unwrap();

        assert_eq!(row.status, "Aposentado");
        assert_eq!(row.categoria_atual.as_deref(), Some("gt3"));
        assert_eq!(row.temporada_aposentadoria.as_deref(), Some("2024"));
        assert_eq!(row.anos_aposentado, Some(2));
        assert_eq!(row.titulos, 1);
    }

    #[test]
    fn payload_excludes_drivers_without_competitive_history() {
        let conn = setup_conn();
        let mut empty =
            driver_with_stats("D_EMPTY", "Sem Historico", Some("mazda_rookie"), 0, 0, 0);
        empty.stats_carreira.corridas = 0;
        let mut scorer = driver_with_stats("D_SCORE", "Com Pontos", Some("mazda_rookie"), 0, 0, 0);
        scorer.stats_carreira.pontos_total = 12.0;
        scorer.stats_carreira.corridas = 2;
        insert_driver(&conn, &empty).expect("insert empty");
        insert_driver(&conn, &scorer).expect("insert scorer");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");

        assert!(payload.rows.iter().all(|row| row.id != "D_EMPTY"));
        assert!(payload.rows.iter().any(|row| row.id == "D_SCORE"));
    }

    #[test]
    fn payload_includes_historical_categories_and_active_injury_tag() {
        let mut conn = setup_conn();
        let driver = driver_with_stats("D_HIST", "Piloto Historico", Some("gt4"), 3, 5, 0);
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "D_HIST",
                1,
                2024,
                "Piloto Historico",
                "mazda_rookie",
                3,
                180.0,
                r#"{"vitorias": 2, "podios": 5, "corridas": 12, "pontos": 180}"#
            ],
        )
        .expect("insert archive");

        let tx = conn.transaction().expect("tx");
        crate::db::queries::injuries::insert_injury(
            &tx,
            &Injury {
                id: "I_HIST".to_string(),
                pilot_id: "D_HIST".to_string(),
                injury_type: InjuryType::Grave,
                injury_name: "Joelho lesionado".to_string(),
                modifier: 0.75,
                races_total: 8,
                races_remaining: 3,
                skill_penalty: 0.15,
                season: 2,
                race_occurred: "R002".to_string(),
                active: true,
            },
        )
        .expect("insert injury");
        tx.commit().expect("commit");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload.rows.iter().find(|row| row.id == "D_HIST").unwrap();

        assert_eq!(row.lesao_ativa_tipo.as_deref(), Some("Grave"));
        assert!(row.is_lesionado);
        assert!(row.categorias_historicas.contains(&"gt4".to_string()));
        assert!(row
            .categorias_historicas
            .contains(&"mazda_rookie".to_string()));
    }

    #[test]
    fn payload_infers_rookie_foundation_for_seeded_veteran_careers() {
        let conn = setup_conn();
        let mut driver = driver_with_stats("D_GT4_SEEDED", "Veterano GT4", Some("gt4"), 3, 5, 0);
        driver.stats_carreira.temporadas = 4;
        driver.stats_carreira.corridas = 38;
        driver.temporadas_na_categoria = 2;
        driver.corridas_na_categoria = 18;
        insert_driver(&conn, &driver).expect("insert seeded veteran");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload
            .rows
            .iter()
            .find(|row| row.id == "D_GT4_SEEDED")
            .unwrap();

        assert!(row.categorias_historicas.contains(&"gt4".to_string()));
        assert!(
            row.categorias_historicas
                .iter()
                .any(|category| matches!(category.as_str(), "mazda_rookie" | "toyota_rookie")),
            "seeded veteran should expose a rookie foundation: {:?}",
            row.categorias_historicas
        );
    }

    #[test]
    fn payload_counts_archived_championship_positions_as_titles() {
        let conn = setup_conn();
        let mut driver = driver_with_stats("D_CHAMP", "Campeao Arquivado", Some("gt4"), 3, 5, 0);
        driver.stats_carreira.titulos = 1;
        insert_driver(&conn, &driver).expect("insert champion");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "D_CHAMP",
                1,
                2025,
                "Campeao Arquivado",
                "gt4",
                1,
                220.0,
                r#"{"vitorias": 5, "podios": 8, "corridas": 10, "pontos": 220}"#
            ],
        )
        .expect("insert archive without titles field");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let row = payload.rows.iter().find(|row| row.id == "D_CHAMP").unwrap();

        assert_eq!(row.titulos, 1);
        assert_eq!(row.titles_rank, 1);
    }

    #[test]
    fn payload_reports_rank_delta_since_latest_race() {
        let conn = setup_conn();
        insert_season(&conn, &Season::new("S_TEST".to_string(), 1, 2026))
            .expect("insert active season");
        insert_team(
            &conn,
            &placeholder_team_from_db(
                "T_GT4".to_string(),
                "Equipe Azul".to_string(),
                "gt4".to_string(),
                "2026-01-01".to_string(),
            ),
        )
        .expect("insert team");

        let climber = driver_with_stats("D_CLIMB", "Piloto Subindo", Some("gt4"), 1, 1, 0);
        let mut falling = driver_with_stats("D_FALL", "Piloto Caindo", Some("gt4"), 0, 0, 0);
        falling.stats_carreira.pontos_total = 90.0;
        falling.stats_carreira.corridas = 3;

        insert_driver(&conn, &climber).expect("insert climber");
        insert_driver(&conn, &falling).expect("insert falling");

        conn.execute(
            "INSERT INTO calendar (id, temporada_id, rodada, pista, categoria, clima, duracao, data)
             VALUES ('R_GT4_1', 'S_TEST', 1, 'Interlagos', 'gt4', 'Seco', 60, '2026-05-03')",
            [],
        )
        .expect("insert calendar");
        conn.execute(
            "INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final,
                voltas_completadas, dnf, pontos
             ) VALUES
                ('R_GT4_1', 'D_CLIMB', 'T_GT4', 1, 1, 20, 0, 100.0),
                ('R_GT4_1', 'D_FALL', 'T_GT4', 2, 2, 20, 0, 0.0)",
            [],
        )
        .expect("insert race results");

        let payload = build_global_driver_rankings(&conn, None).expect("payload");
        let climber = payload.rows.iter().find(|row| row.id == "D_CLIMB").unwrap();
        let falling = payload.rows.iter().find(|row| row.id == "D_FALL").unwrap();

        assert_eq!(climber.historical_rank, 1);
        assert_eq!(climber.historical_rank_delta, Some(1));
        assert_eq!(falling.historical_rank, 2);
        assert_eq!(falling.historical_rank_delta, Some(-1));
    }
}
