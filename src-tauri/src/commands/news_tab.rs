use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate};
use tauri::{AppHandle, Manager};

use crate::commands::career::{
    get_calendar_for_category_in_base_dir, get_drivers_by_category_in_base_dir,
    get_news_in_base_dir, get_teams_standings_in_base_dir, load_career_in_base_dir,
};
use crate::commands::career_types::{
    NewsTabBootstrap, NewsTabFilterOption, NewsTabHero, NewsTabScopeMeta, NewsTabScopeTab,
    NewsTabSnapshot, NewsTabSnapshotRequest, NewsTabStory, NewsTabStoryBlock,
};
use crate::commands::news_helpers::{
    build_meta_label, build_story_time_label, format_display_date_label, freshness_bonus,
    importance_label, importance_rank, scope_class_label, story_accent, story_sort_key,
    team_color_pair, team_presence_label,
};
use crate::config::app_config::AppConfig;
use crate::constants::categories;
use crate::db::connection::Database;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::race_history as race_history_queries;
use crate::db::queries::rivalries as rivalry_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::models::rivalry::Rivalry;
use crate::news::{NewsImportance, NewsItem, NewsType};
use crate::public_presence::team::derive_team_public_presence;

const PRIMARY_FILTER_IDS: [&str; 4] = ["Corridas", "Pilotos", "Equipes", "Mercado"];
const FAMOUS_FILTER_IDS: [&str; 3] = ["Pilotos", "Equipes", "Mercado"];

#[tauri::command]
pub async fn get_news_tab_bootstrap(
    app: AppHandle,
    career_id: String,
) -> Result<NewsTabBootstrap, String> {
    let base_dir = app_data_dir(&app)?;
    get_news_tab_bootstrap_in_base_dir(&base_dir, &career_id)
}

#[tauri::command]
pub async fn get_news_tab_snapshot(
    app: AppHandle,
    career_id: String,
    request: NewsTabSnapshotRequest,
) -> Result<NewsTabSnapshot, String> {
    let base_dir = app_data_dir(&app)?;
    get_news_tab_snapshot_in_base_dir(&base_dir, &career_id, request)
}

pub(crate) fn get_news_tab_bootstrap_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<NewsTabBootstrap, String> {
    let career = load_career_in_base_dir(base_dir, career_id)?;
    let player_categoria = career
        .player_team
        .as_ref()
        .map(|t| t.categoria.clone())
        .unwrap_or_default();
    let current_round = career.season.rodada_atual;
    let calendar = if player_categoria.is_empty() {
        Vec::new()
    } else {
        get_calendar_for_category_in_base_dir(base_dir, career_id, &player_categoria)?
    };
    let last_race = calendar
        .iter()
        .filter(|r| r.rodada < current_round)
        .max_by_key(|r| r.rodada);
    let next_race = calendar.iter().find(|r| r.rodada >= current_round);
    let season_completed = calendar
        .iter()
        .max_by_key(|r| r.rodada)
        .map(|r| r.status == "Concluida")
        .unwrap_or(false);
    let pub_date_label = last_race
        .map(|r| format_display_date(&r.display_date))
        .unwrap_or_else(|| career.season.ano.to_string());
    let last_race_name = last_race.map(|r| r.track_name.clone());
    let next_race_date_label = next_race.map(|r| format_display_date(&r.display_date));
    let next_race_name = next_race.map(|r| r.track_name.clone());
    Ok(NewsTabBootstrap {
        default_scope_type: "category".to_string(),
        default_scope_id: player_categoria.clone(),
        default_primary_filter: Some("Corridas".to_string()),
        default_context_type: last_race.map(|_| "race".to_string()),
        default_context_id: last_race.map(|r| r.id.clone()),
        scopes: build_scope_tabs(),
        season_number: career.season.numero,
        season_year: career.season.ano,
        current_round,
        total_rounds: career.season.total_rodadas,
        season_completed,
        pub_date_label,
        last_race_name,
        next_race_date_label,
        next_race_name,
    })
}

fn format_display_date(date_str: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Fev", "Mar", "Abr", "Mai", "Jun", "Jul", "Ago", "Set", "Out", "Nov", "Dez",
    ];
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map(|d| {
            format!(
                "{} {} de {}",
                d.day(),
                MONTHS[d.month0() as usize],
                d.year()
            )
        })
        .unwrap_or_else(|_| date_str.to_string())
}

pub(crate) fn get_news_tab_snapshot_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    request: NewsTabSnapshotRequest,
) -> Result<NewsTabSnapshot, String> {
    let context = load_context(base_dir, career_id)?;
    let scope_type = normalize_scope_type(&request.scope_type);
    let scope_id = normalize_scope_id(
        scope_type,
        &request.scope_id,
        context
            .career
            .player_team
            .as_ref()
            .map(|t| t.categoria.as_str())
            .unwrap_or(""),
    );
    let scope_class = normalize_scope_class(&scope_id, request.scope_class.as_deref());
    let primary_filter = normalize_primary_filter(scope_type, request.primary_filter.as_deref());
    let primary_filters = build_primary_filters(scope_type);
    let scope_label = if scope_type == "famous" {
        "Mais famosos".to_string()
    } else {
        build_scope_label(&context, &scope_id, scope_class.as_deref())
    };
    let scoped_items = select_scope_items(&context, scope_type, &scope_id, scope_class.as_deref())?;
    let primary_items = select_primary_items(
        &context,
        scope_type,
        &scope_id,
        scope_class.as_deref(),
        primary_filter.as_deref(),
        scoped_items,
    );
    let contextual_filters = if scope_type == "famous" {
        build_famous_context_filters(
            &context,
            primary_filter.as_deref(),
            &primary_items,
            request.context_type.as_deref(),
            request.context_id.as_deref(),
        )?
    } else {
        build_category_context_filters(
            &context,
            &scope_id,
            scope_class.as_deref(),
            primary_filter.as_deref(),
            &primary_items,
            request.context_type.as_deref(),
            request.context_id.as_deref(),
        )?
    };
    let selection = resolve_context_selection(
        &contextual_filters,
        request.context_type.as_deref(),
        request.context_id.as_deref(),
    );
    let mut selected_items = primary_items;
    if let Some(selection) = selection.as_ref() {
        selected_items.retain(|item| story_matches_context(&context, item, selection));
    }
    let stories = build_stories(&context, selected_items)?;

    Ok(NewsTabSnapshot {
        hero: build_hero(
            &context,
            scope_type,
            &scope_id,
            &scope_label,
            scope_class.as_deref(),
            primary_filter.as_deref(),
        ),
        primary_filters,
        contextual_filters,
        stories,
        scope_meta: NewsTabScopeMeta {
            scope_type: scope_type.to_string(),
            scope_id,
            scope_label,
            scope_class,
            primary_filter,
            context_type: selection.as_ref().map(|value| value.kind.clone()),
            context_id: selection.as_ref().map(|value| value.id.clone()),
            context_label: selection.as_ref().map(|value| value.label.clone()),
            is_special: scope_type == "famous",
        },
    })
}

pub(crate) struct NewsTabContext {
    pub(crate) base_dir: PathBuf,
    pub(crate) career_id: String,
    pub(crate) db: Database,
    pub(crate) active_season_id: String,
    pub(crate) career: crate::commands::career_types::CareerData,
    pub(crate) all_news: Vec<NewsItem>,
    pub(crate) newest_timestamp: i64,
    pub(crate) driver_names: HashMap<String, String>,
    pub(crate) driver_media: HashMap<String, f64>,
    pub(crate) team_names: HashMap<String, String>,
    pub(crate) team_colors: HashMap<String, (String, String)>,
    pub(crate) team_driver_ids: HashMap<String, Vec<String>>,
    pub(crate) category_names: HashMap<String, String>,
    pub(crate) race_rounds: HashMap<String, i32>,
    pub(crate) race_labels: HashMap<String, String>,
    pub(crate) race_dates: HashMap<String, String>,
    pub(crate) max_preseason_week_by_season: HashMap<i32, i32>,
    /// "category_id:driver_id" → posição no campeonato
    pub(crate) driver_positions: HashMap<String, i32>,
    /// "category_id:driver_id" → pontos arredondados
    pub(crate) driver_points: HashMap<String, i32>,
    /// team_id → posição no campeonato da categoria da equipe
    pub(crate) team_positions: HashMap<String, i32>,
    /// team_id → pontos da equipe
    pub(crate) team_points: HashMap<String, i32>,
    /// category_id → próxima corrida da categoria
    pub(crate) next_race_by_category: HashMap<String, NextRaceInfo>,
    /// team_id → tier de presença pública ("elite", "alta", "relevante", "baixa")
    pub(crate) team_public_presence: HashMap<String, String>,
    /// "category_id:driver_id" → sequência atual de vitórias consecutivas nesta temporada
    pub(crate) driver_win_streaks: HashMap<String, u32>,
    /// category_id → driver_id do líder do campeonato antes da rodada mais recente.
    /// Ausente se não há rodadas anteriores (primeira corrida da temporada).
    pub(crate) category_prev_leaders: HashMap<String, String>,
    /// "category_id:driver_id" → (posicao_largada, posicao_final, is_dnf) na última rodada disputada.
    pub(crate) latest_race_results: HashMap<String, (i32, i32, bool)>,
    /// "category_id:driver_id" → fatos de DNF catalogado na última rodada (apenas pilotos com dnf_catalog_id).
    pub(crate) latest_incident_facts: HashMap<String, IncidentEditorialFacts>,
    /// category_id → lista de driver_ids ordenados por pontos nesta temporada (índice 0 = líder).
    pub(crate) category_standings_top: HashMap<String, Vec<String>>,
    /// driver_id → vitórias na temporada atual (de stats_temporada.vitorias).
    pub(crate) driver_season_wins: HashMap<String, u32>,
    /// driver_id → vitórias de carreira (de stats_carreira.vitorias).
    pub(crate) driver_career_wins: HashMap<String, u32>,
}

pub(crate) struct NextRaceInfo {
    pub(crate) label: String,
    pub(crate) date_label: String,
    #[allow(dead_code)]
    pub(crate) round: i32,
}

#[derive(Clone)]
struct ContextSelection {
    kind: String,
    id: String,
    label: String,
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))
}

fn load_context(base_dir: &Path, career_id: &str) -> Result<NewsTabContext, String> {
    let career = load_career_in_base_dir(base_dir, career_id)?;
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let all_news = get_news_in_base_dir(
        base_dir,
        career_id,
        Some(active_season.numero),
        None,
        Some(400),
    )?;
    let newest_timestamp = all_news.first().map(|item| item.timestamp).unwrap_or(0);

    let all_drivers = driver_queries::get_all_drivers(&db.conn)
        .map_err(|e| format!("Falha ao buscar pilotos do mundo: {e}"))?;
    let all_teams = team_queries::get_all_teams(&db.conn)
        .map_err(|e| format!("Falha ao buscar equipes do mundo: {e}"))?;

    let current_round = career.season.rodada_atual;
    let mut race_rounds = HashMap::new();
    let mut race_labels = HashMap::new();
    let mut race_dates = HashMap::new();
    let mut next_race_by_category: HashMap<String, NextRaceInfo> = HashMap::new();
    for category in categories::get_all_categories() {
        for race in get_calendar_for_category_in_base_dir(base_dir, career_id, category.id)? {
            race_rounds.insert(race.id.clone(), race.rodada);
            race_labels.insert(
                format!("{}:{}", category.id, race.rodada),
                race.track_name.clone(),
            );
            race_dates.insert(
                format!("{}:{}", category.id, race.rodada),
                race.display_date.clone(),
            );
            if race.rodada >= current_round && !next_race_by_category.contains_key(category.id) {
                let date_label = format_display_date_label(&race.display_date).unwrap_or_default();
                next_race_by_category.insert(
                    category.id.to_string(),
                    NextRaceInfo {
                        label: race.track_name,
                        date_label,
                        round: race.rodada,
                    },
                );
            }
        }
    }
    // driver standings: chave composta "category_id:driver_id"
    let mut driver_groups: HashMap<String, Vec<&crate::models::driver::Driver>> = HashMap::new();
    for driver in &all_drivers {
        if let Some(cat) = driver.categoria_atual.as_deref() {
            driver_groups
                .entry(cat.to_string())
                .or_default()
                .push(driver);
        }
    }
    let mut driver_positions: HashMap<String, i32> = HashMap::new();
    let mut driver_points: HashMap<String, i32> = HashMap::new();
    for (cat_id, group) in &mut driver_groups {
        group.sort_by(|a, b| {
            b.stats_temporada
                .pontos
                .partial_cmp(&a.stats_temporada.pontos)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.stats_temporada.vitorias.cmp(&a.stats_temporada.vitorias))
                .then(b.stats_temporada.podios.cmp(&a.stats_temporada.podios))
        });
        for (pos, driver) in group.iter().enumerate() {
            let key = format!("{cat_id}:{}", driver.id);
            driver_positions.insert(key.clone(), pos as i32 + 1);
            driver_points.insert(key, driver.stats_temporada.pontos.round() as i32);
        }
    }

    // team standings: chave team_id (uma categoria por equipe)
    let mut team_groups: HashMap<String, Vec<&crate::models::team::Team>> = HashMap::new();
    for team in &all_teams {
        team_groups
            .entry(team.categoria.clone())
            .or_default()
            .push(team);
    }
    let mut team_positions: HashMap<String, i32> = HashMap::new();
    let mut team_points: HashMap<String, i32> = HashMap::new();
    for group in team_groups.values_mut() {
        group.sort_by(|a, b| {
            b.stats_pontos
                .cmp(&a.stats_pontos)
                .then(b.stats_vitorias.cmp(&a.stats_vitorias))
                .then(b.stats_podios.cmp(&a.stats_podios))
        });
        for (pos, team) in group.iter().enumerate() {
            team_positions.insert(team.id.clone(), pos as i32 + 1);
            team_points.insert(team.id.clone(), team.stats_pontos);
        }
    }

    // presença pública por equipe
    let team_public_presence: HashMap<String, String> = all_teams
        .iter()
        .map(|team| {
            let driver_media_values: Vec<f64> = [&team.piloto_1_id, &team.piloto_2_id]
                .iter()
                .filter_map(|opt| opt.as_ref())
                .filter_map(|id| {
                    all_drivers
                        .iter()
                        .find(|d| &d.id == id)
                        .map(|d| d.atributos.midia)
                })
                .collect();
            let tier = team_presence_label(&derive_team_public_presence(&driver_media_values).tier)
                .to_string();
            (team.id.clone(), tier)
        })
        .collect();

    let mut max_preseason_week_by_season = HashMap::new();
    for item in &all_news {
        if let Some(week) = item.semana_pretemporada.filter(|week| *week > 0) {
            max_preseason_week_by_season
                .entry(item.temporada)
                .and_modify(|current: &mut i32| *current = (*current).max(week))
                .or_insert(week);
        }
    }

    let mut driver_win_streaks: HashMap<String, u32> = HashMap::new();
    for driver in &all_drivers {
        if let Some(cat) = driver.categoria_atual.as_deref() {
            let streak =
                race_history_queries::get_win_streak(&db.conn, &driver.id, &active_season.id, cat)
                    .unwrap_or(0);
            if streak > 0 {
                driver_win_streaks.insert(format!("{cat}:{}", driver.id), streak);
            }
        }
    }

    // Para cada categoria, quem liderava antes da rodada mais recente.
    // current_round é a próxima corrida a disputar; current_round - 1 é a última completada.
    let mut category_prev_leaders: HashMap<String, String> = HashMap::new();
    for category in categories::get_all_categories() {
        if let Ok(Some(prev_leader_id)) = race_history_queries::get_category_leader_before_round(
            &db.conn,
            &active_season.id,
            category.id,
            current_round - 1,
        ) {
            category_prev_leaders.insert(category.id.to_string(), prev_leader_id);
        }
    }

    // Resultados da última rodada disputada e standings por categoria.
    let completed_round = current_round - 1;
    let mut latest_race_results: HashMap<String, (i32, i32, bool)> = HashMap::new();
    let mut latest_incident_facts: HashMap<String, IncidentEditorialFacts> = HashMap::new();
    let mut category_standings_top: HashMap<String, Vec<String>> = HashMap::new();
    if completed_round >= 1 {
        for category in categories::get_all_categories() {
            if let Ok(results) = race_history_queries::get_results_for_round(
                &db.conn,
                &active_season.id,
                category.id,
                completed_round,
            ) {
                for (driver_id, grid_pos, finish_pos, is_dnf) in results {
                    latest_race_results.insert(
                        format!("{}:{}", category.id, driver_id),
                        (grid_pos, finish_pos, is_dnf),
                    );
                }
            }
            if let Ok(facts) = race_history_queries::get_dnf_incident_facts_for_round(
                &db.conn,
                &active_season.id,
                category.id,
                completed_round,
            ) {
                for (driver_id, incident_source, is_dnf, segment) in facts {
                    let incident_type = incident_fact_type_from_source(incident_source.as_deref());
                    latest_incident_facts.insert(
                        format!("{}:{}", category.id, driver_id),
                        IncidentEditorialFacts {
                            incident_type,
                            is_dnf,
                            segment,
                        },
                    );
                }
            }
            if let Ok(standings) = race_history_queries::get_category_standings(
                &db.conn,
                &active_season.id,
                category.id,
            ) {
                category_standings_top.insert(
                    category.id.to_string(),
                    standings.into_iter().map(|s| s.pilot_id).collect(),
                );
            }
        }
    }

    // Vitórias por piloto nesta temporada e na carreira (de stats já persistidos no drivers).
    let driver_season_wins: HashMap<String, u32> = all_drivers
        .iter()
        .filter(|d| d.stats_temporada.vitorias > 0)
        .map(|d| (d.id.clone(), d.stats_temporada.vitorias))
        .collect();
    let driver_career_wins: HashMap<String, u32> = all_drivers
        .iter()
        .filter(|d| d.stats_carreira.vitorias > 0)
        .map(|d| (d.id.clone(), d.stats_carreira.vitorias))
        .collect();

    Ok(NewsTabContext {
        base_dir: base_dir.to_path_buf(),
        career_id: career_id.to_string(),
        db,
        active_season_id: active_season.id.clone(),
        career,
        all_news,
        newest_timestamp,
        driver_names: all_drivers
            .iter()
            .map(|d| (d.id.clone(), d.nome.clone()))
            .collect(),
        driver_media: all_drivers
            .iter()
            .map(|d| (d.id.clone(), d.atributos.midia))
            .collect(),
        team_names: all_teams
            .iter()
            .map(|t| (t.id.clone(), t.nome.clone()))
            .collect(),
        team_colors: all_teams
            .iter()
            .map(|team| {
                (
                    team.id.clone(),
                    (team.cor_primaria.clone(), team.cor_secundaria.clone()),
                )
            })
            .collect(),
        team_driver_ids: all_teams
            .iter()
            .map(|team| {
                let mut ids = Vec::new();
                if let Some(id) = team.piloto_1_id.clone() {
                    ids.push(id);
                }
                if let Some(id) = team.piloto_2_id.clone() {
                    ids.push(id);
                }
                (team.id.clone(), ids)
            })
            .collect(),
        category_names: categories::get_all_categories()
            .iter()
            .map(|category| (category.id.to_string(), category.nome.to_string()))
            .collect(),
        race_rounds,
        race_labels,
        race_dates,
        max_preseason_week_by_season,
        driver_positions,
        driver_points,
        team_positions,
        team_points,
        next_race_by_category,
        team_public_presence,
        driver_win_streaks,
        category_prev_leaders,
        latest_race_results,
        latest_incident_facts,
        category_standings_top,
        driver_season_wins,
        driver_career_wins,
    })
}

fn build_scope_tabs() -> Vec<NewsTabScopeTab> {
    let mut tabs: Vec<NewsTabScopeTab> = categories::get_all_categories()
        .iter()
        .map(|category| NewsTabScopeTab {
            id: category.id.to_string(),
            label: category.nome.to_string(),
            short_label: category.nome_curto.to_string(),
            scope_type: "category".to_string(),
            special: false,
        })
        .collect();
    tabs.push(NewsTabScopeTab {
        id: "mais_famosos".to_string(),
        label: "Mais famosos".to_string(),
        short_label: "Mais famosos".to_string(),
        scope_type: "famous".to_string(),
        special: true,
    });
    tabs
}

fn build_primary_filters(scope_type: &str) -> Vec<NewsTabFilterOption> {
    let ids = if scope_type == "famous" {
        &FAMOUS_FILTER_IDS[..]
    } else {
        &PRIMARY_FILTER_IDS[..]
    };
    ids.iter()
        .map(|id| NewsTabFilterOption {
            id: (*id).to_string(),
            label: (*id).to_string(),
            meta: None,
            tone: Some(if *id == "Mercado" {
                "warm".to_string()
            } else {
                "cool".to_string()
            }),
            kind: Some("tag".to_string()),
            color_primary: None,
            color_secondary: None,
        })
        .collect()
}

fn build_scope_label(
    context: &NewsTabContext,
    scope_id: &str,
    scope_class: Option<&str>,
) -> String {
    let base_label = context
        .category_names
        .get(scope_id)
        .cloned()
        .unwrap_or_else(|| scope_id.to_string());

    match scope_class {
        Some(class_name) => format!("{base_label} · {}", scope_class_label(class_name)),
        None => base_label,
    }
}

fn scope_team_ids(
    context: &NewsTabContext,
    category_id: &str,
    scope_class: Option<&str>,
) -> Result<HashSet<String>, String> {
    if let Some(scope_class) = scope_class {
        if let Some(feeder_category) = feeder_category_for_scope_class(category_id, scope_class) {
            return Ok(
                team_queries::get_teams_by_category(&context.db.conn, feeder_category)
                    .map_err(|e| {
                        format!("Falha ao buscar equipes da categoria {feeder_category}: {e}")
                    })?
                    .into_iter()
                    .map(|team| team.id)
                    .collect(),
            );
        }

        return Ok(team_queries::get_teams_by_category_and_class(
            &context.db.conn,
            category_id,
            scope_class,
        )
        .map_err(|e| format!("Falha ao buscar equipes da classe {scope_class}: {e}"))?
        .into_iter()
        .map(|team| team.id)
        .collect());
    }

    Ok(
        get_teams_standings_in_base_dir(&context.base_dir, &context.career_id, category_id)?
            .into_iter()
            .map(|team| team.id)
            .collect(),
    )
}

fn scope_driver_ids(
    context: &NewsTabContext,
    category_id: &str,
    scope_class: Option<&str>,
    team_ids: &HashSet<String>,
) -> Result<HashSet<String>, String> {
    if scope_class.is_none() {
        return Ok(get_drivers_by_category_in_base_dir(
            &context.base_dir,
            &context.career_id,
            category_id,
        )?
        .into_iter()
        .map(|driver| driver.id)
        .collect());
    }

    let mut driver_ids = HashSet::new();
    for team_id in team_ids {
        if let Some(ids) = context.team_driver_ids.get(team_id) {
            driver_ids.extend(ids.iter().cloned());
        }

        for contract in
            contract_queries::get_active_contracts_for_team(&context.db.conn, team_id)
                .map_err(|e| format!("Falha ao buscar lineup ativo da equipe {team_id}: {e}"))?
        {
            driver_ids.insert(contract.piloto_id);
        }
    }

    Ok(driver_ids)
}

fn scope_drivers(
    context: &NewsTabContext,
    category_id: &str,
    scope_class: Option<&str>,
) -> Result<Vec<crate::commands::career_types::DriverSummary>, String> {
    let mut drivers =
        get_drivers_by_category_in_base_dir(&context.base_dir, &context.career_id, category_id)?;
    if let Some(scope_class) = scope_class {
        let team_ids = scope_team_ids(context, category_id, Some(scope_class))?;
        let driver_ids = scope_driver_ids(context, category_id, Some(scope_class), &team_ids)?;
        drivers.retain(|driver| driver_ids.contains(&driver.id));
    }
    Ok(drivers)
}

fn scope_team_standings(
    context: &NewsTabContext,
    category_id: &str,
    scope_class: Option<&str>,
) -> Result<Vec<crate::commands::career_types::TeamStanding>, String> {
    let mut teams =
        get_teams_standings_in_base_dir(&context.base_dir, &context.career_id, category_id)?;
    if let Some(scope_class) = scope_class {
        if let Some(feeder_category) = feeder_category_for_scope_class(category_id, scope_class) {
            return get_teams_standings_in_base_dir(
                &context.base_dir,
                &context.career_id,
                feeder_category,
            );
        }

        let team_ids = scope_team_ids(context, category_id, Some(scope_class))?;
        teams.retain(|team| team_ids.contains(&team.id));
    }
    Ok(teams)
}

fn feeder_category_for_scope_class(category_id: &str, scope_class: &str) -> Option<&'static str> {
    let category = categories::get_category_config(category_id)?;
    if !categories::is_especial(category_id) {
        return None;
    }

    let class = category
        .classes
        .iter()
        .find(|class_info| class_info.class_name == scope_class)?;
    categories::get_category_config(class.car_categoria).map(|_| class.car_categoria)
}

fn build_category_context_filters(
    context: &NewsTabContext,
    category_id: &str,
    scope_class: Option<&str>,
    primary_filter: Option<&str>,
    scoped_items: &[NewsItem],
    requested_context_type: Option<&str>,
    requested_context_id: Option<&str>,
) -> Result<Vec<NewsTabFilterOption>, String> {
    match primary_filter {
        Some("Corridas") => Ok(get_calendar_for_category_in_base_dir(
            &context.base_dir,
            &context.career_id,
            category_id,
        )?
        .into_iter()
        .map(|race| NewsTabFilterOption {
            id: race.id,
            label: race.track_name,
            meta: Some(format!("R{}", race.rodada)),
            tone: Some("cool".to_string()),
            kind: Some("race".to_string()),
            color_primary: None,
            color_secondary: None,
        })
        .collect()),
        Some("Pilotos") => build_pilot_context_filters(
            context,
            category_id,
            scope_class,
            requested_context_type,
            requested_context_id,
        ),
        Some("Equipes") => Ok(scope_team_standings(context, category_id, scope_class)?
            .into_iter()
            .map(|team| {
                let (color_primary, color_secondary) = team_color_pair(context, &team.id);
                NewsTabFilterOption {
                    id: team.id,
                    label: team.nome,
                    meta: Some(format!("{} pts", team.pontos)),
                    tone: Some("cool".to_string()),
                    kind: Some("team".to_string()),
                    color_primary,
                    color_secondary,
                }
            })
            .collect()),
        Some("Mercado") => Ok(build_entity_context_filters(context, scoped_items, 6)),
        _ => Ok(Vec::new()),
    }
}

fn build_famous_context_filters(
    context: &NewsTabContext,
    primary_filter: Option<&str>,
    scoped_items: &[NewsItem],
    _requested_context_type: Option<&str>,
    _requested_context_id: Option<&str>,
) -> Result<Vec<NewsTabFilterOption>, String> {
    match primary_filter {
        Some("Pilotos") => Ok(build_famous_driver_filters(context, 8)),
        Some("Equipes") => Ok(build_famous_team_filters(context, 8)),
        Some("Mercado") => Ok(build_entity_context_filters(context, scoped_items, 6)),
        _ => Ok(Vec::new()),
    }
}

fn build_pilot_context_filters(
    context: &NewsTabContext,
    category_id: &str,
    scope_class: Option<&str>,
    requested_context_type: Option<&str>,
    requested_context_id: Option<&str>,
) -> Result<Vec<NewsTabFilterOption>, String> {
    let drivers = scope_drivers(context, category_id, scope_class)?;
    let driver_map: HashMap<String, String> = drivers
        .iter()
        .map(|driver| (driver.id.clone(), driver.nome.clone()))
        .collect();
    let active_driver_id = match (requested_context_type, requested_context_id) {
        (Some("driver"), Some(driver_id)) if driver_map.contains_key(driver_id) => {
            Some(driver_id.to_string())
        }
        (Some("rivalry"), Some(rivalry_id)) => rivalry_id
            .split('|')
            .next()
            .filter(|driver_id| driver_map.contains_key(*driver_id))
            .map(|value| value.to_string()),
        _ => None,
    };

    if let Some(active_driver_id) = active_driver_id {
        let mut filters = vec![NewsTabFilterOption {
            id: active_driver_id.clone(),
            label: driver_map
                .get(&active_driver_id)
                .cloned()
                .unwrap_or_else(|| active_driver_id.clone()),
            meta: Some("Piloto em foco".to_string()),
            tone: Some("accent".to_string()),
            kind: Some("driver".to_string()),
            color_primary: None,
            color_secondary: None,
        }];

        for rivalry in build_rivalries_for_category(context, category_id, scope_class)? {
            let other_driver_id = if rivalry.piloto1_id == active_driver_id {
                Some(rivalry.piloto2_id.clone())
            } else if rivalry.piloto2_id == active_driver_id {
                Some(rivalry.piloto1_id.clone())
            } else {
                None
            };
            let Some(other_driver_id) = other_driver_id else {
                continue;
            };
            let active_driver_label = driver_map
                .get(&active_driver_id)
                .cloned()
                .unwrap_or_else(|| active_driver_id.clone());
            let other_driver_label = driver_map
                .get(&other_driver_id)
                .cloned()
                .unwrap_or_else(|| other_driver_id.clone());
            filters.push(NewsTabFilterOption {
                id: format!("{}|{}", active_driver_id, other_driver_id),
                label: format!("{active_driver_label} vs {other_driver_label}"),
                meta: Some(format!("{:.0}% intensidade", rivalry.perceived_intensity())),
                tone: Some("warm".to_string()),
                kind: Some("rivalry".to_string()),
                color_primary: None,
                color_secondary: None,
            });
        }

        return Ok(filters);
    }

    Ok(drivers
        .into_iter()
        .map(|driver| NewsTabFilterOption {
            id: driver.id,
            label: driver.nome,
            meta: Some(format!("{} pts", driver.pontos)),
            tone: Some(if driver.is_jogador {
                "accent".to_string()
            } else {
                "cool".to_string()
            }),
            kind: Some("driver".to_string()),
            color_primary: None,
            color_secondary: None,
        })
        .collect())
}

fn build_rivalries_for_category(
    context: &NewsTabContext,
    category_id: &str,
    scope_class: Option<&str>,
) -> Result<Vec<Rivalry>, String> {
    let team_ids = scope_team_ids(context, category_id, scope_class)?;
    let driver_ids = scope_driver_ids(context, category_id, scope_class, &team_ids)?;
    let mut rivalries: Vec<Rivalry> = rivalry_queries::get_all_rivalries(&context.db.conn)
        .map_err(|e| format!("Falha ao buscar rivalidades: {e}"))?
        .into_iter()
        .filter(|r| driver_ids.contains(&r.piloto1_id) && driver_ids.contains(&r.piloto2_id))
        .collect();
    rivalries.sort_by(|left, right| {
        right
            .perceived_intensity()
            .partial_cmp(&left.perceived_intensity())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(rivalries)
}

fn build_famous_driver_filters(context: &NewsTabContext, limit: usize) -> Vec<NewsTabFilterOption> {
    let mut items: Vec<(String, String, f64)> = context
        .driver_media
        .iter()
        .filter_map(|(id, media)| {
            context
                .driver_names
                .get(id)
                .map(|name| (id.clone(), name.clone(), *media))
        })
        .collect();
    items.sort_by(|left, right| {
        right
            .2
            .partial_cmp(&left.2)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    items
        .into_iter()
        .take(limit)
        .map(|(id, label, media)| NewsTabFilterOption {
            id,
            label,
            meta: Some(format!("midia {:.0}", media)),
            tone: Some("accent".to_string()),
            kind: Some("driver".to_string()),
            color_primary: None,
            color_secondary: None,
        })
        .collect()
}

fn build_famous_team_filters(context: &NewsTabContext, limit: usize) -> Vec<NewsTabFilterOption> {
    let mut scored: Vec<(String, String, f64, String)> = context
        .team_driver_ids
        .iter()
        .filter_map(|(team_id, driver_ids)| {
            let label = context.team_names.get(team_id)?.clone();
            let medias: Vec<f64> = driver_ids
                .iter()
                .filter_map(|id| context.driver_media.get(id).copied())
                .collect();
            let presence = derive_team_public_presence(&medias);
            Some((
                team_id.clone(),
                label,
                presence.raw_score,
                team_presence_label(&presence.tier).to_string(),
            ))
        })
        .collect();
    scored.sort_by(|left, right| {
        right
            .2
            .partial_cmp(&left.2)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored
        .into_iter()
        .take(limit)
        .map(|(id, label, raw, tier)| {
            let (color_primary, color_secondary) = team_color_pair(context, &id);
            NewsTabFilterOption {
                id,
                label,
                meta: Some(format!("{tier} {:.0}", raw)),
                tone: Some("accent".to_string()),
                kind: Some("team".to_string()),
                color_primary,
                color_secondary,
            }
        })
        .collect()
}

fn build_entity_context_filters(
    context: &NewsTabContext,
    items: &[NewsItem],
    limit: usize,
) -> Vec<NewsTabFilterOption> {
    let mut scored: HashMap<(String, String), f64> = HashMap::new();
    for item in items {
        let weight = importance_rank(&item.importancia) as f64 + 1.0;
        if let Some(team_id) = item.team_id.as_ref() {
            *scored
                .entry(("team".to_string(), team_id.clone()))
                .or_insert(0.0) += weight;
        } else if let Some(driver_id) = item.driver_id.as_ref() {
            *scored
                .entry(("driver".to_string(), driver_id.clone()))
                .or_insert(0.0) += weight;
        }
    }

    let mut ranked = scored.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked
        .into_iter()
        .take(limit)
        .filter_map(|((kind, id), score)| {
            if kind == "team" {
                context.team_names.get(&id).map(|label| {
                    let (color_primary, color_secondary) = team_color_pair(context, &id);
                    NewsTabFilterOption {
                        id,
                        label: label.clone(),
                        meta: Some(format!("Equipe em pauta {:.0}", score)),
                        tone: Some("cool".to_string()),
                        kind: Some("team".to_string()),
                        color_primary,
                        color_secondary,
                    }
                })
            } else {
                context
                    .driver_names
                    .get(&id)
                    .map(|label| NewsTabFilterOption {
                        id,
                        label: label.clone(),
                        meta: Some(format!("Piloto em pauta {:.0}", score)),
                        tone: Some("accent".to_string()),
                        kind: Some("driver".to_string()),
                        color_primary: None,
                        color_secondary: None,
                    })
            }
        })
        .collect()
}

fn resolve_context_selection(
    contextual_filters: &[NewsTabFilterOption],
    context_type: Option<&str>,
    context_id: Option<&str>,
) -> Option<ContextSelection> {
    let kind = context_type?;
    let id = context_id?;
    let matched = contextual_filters
        .iter()
        .find(|filter| filter.id == id && filter.kind.as_deref() == Some(kind))?;
    Some(ContextSelection {
        kind: matched.kind.clone().unwrap_or_default(),
        id: matched.id.clone(),
        label: matched.label.clone(),
    })
}

fn select_scope_items(
    context: &NewsTabContext,
    scope_type: &str,
    scope_id: &str,
    scope_class: Option<&str>,
) -> Result<Vec<NewsItem>, String> {
    let driver_ids_in_scope: HashSet<String> = if scope_type == "category" {
        let team_ids = scope_team_ids(context, scope_id, scope_class)?;
        scope_driver_ids(context, scope_id, scope_class, &team_ids)?
    } else {
        HashSet::new()
    };
    let team_ids_in_scope: HashSet<String> = if scope_type == "category" {
        scope_team_ids(context, scope_id, scope_class)?
    } else {
        HashSet::new()
    };
    let famous_driver_ids: HashSet<String> = build_famous_driver_filters(context, 8)
        .into_iter()
        .map(|filter| filter.id)
        .collect();
    let famous_team_ids: HashSet<String> = build_famous_team_filters(context, 8)
        .into_iter()
        .map(|filter| filter.id)
        .collect();

    Ok(context
        .all_news
        .iter()
        .filter(|item| {
            if scope_type == "famous" {
                story_is_famous(item, &famous_driver_ids, &famous_team_ids)
            } else {
                story_belongs_to_category(
                    item,
                    scope_id,
                    scope_class,
                    &driver_ids_in_scope,
                    &team_ids_in_scope,
                )
            }
        })
        .cloned()
        .collect())
}

fn select_primary_items(
    context: &NewsTabContext,
    scope_type: &str,
    scope_id: &str,
    scope_class: Option<&str>,
    primary_filter: Option<&str>,
    items: Vec<NewsItem>,
) -> Vec<NewsItem> {
    if primary_filter.is_none() {
        if scope_type == "famous" {
            return select_public_briefing_items(context, items);
        }
        return select_category_briefing_items(context, scope_id, scope_class, items);
    }

    filter_items_for_primary(context, scope_id, primary_filter, items)
}

fn filter_items_for_primary(
    context: &NewsTabContext,
    scope_id: &str,
    primary_filter: Option<&str>,
    items: Vec<NewsItem>,
) -> Vec<NewsItem> {
    let mut filtered = items
        .into_iter()
        .filter(|item| story_matches_primary_filter(context, scope_id, item, primary_filter))
        .collect::<Vec<_>>();
    filtered.sort_by(|left, right| story_sort_key(right).cmp(&story_sort_key(left)));
    filtered
}

fn select_category_briefing_items(
    context: &NewsTabContext,
    category_id: &str,
    scope_class: Option<&str>,
    items: Vec<NewsItem>,
) -> Vec<NewsItem> {
    let class_ids: Option<(HashSet<String>, HashSet<String>)> = if scope_class.is_some() {
        scope_team_ids(context, category_id, scope_class)
            .ok()
            .and_then(|team_ids| {
                scope_driver_ids(context, category_id, scope_class, &team_ids)
                    .ok()
                    .map(|driver_ids| (team_ids, driver_ids))
            })
    } else {
        None
    };
    let mut scored = items
        .into_iter()
        .filter_map(|item| {
            category_briefing_score(context, category_id, class_ids.as_ref(), &item)
                .map(|score| (item, score))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.0.timestamp.cmp(&left.0.timestamp))
    });

    let mut selected = scored
        .iter()
        .filter(|(_, score)| *score >= 130)
        .map(|(item, _)| item.clone())
        .collect::<Vec<_>>();

    if selected.len() < 3 {
        selected = scored.into_iter().take(3).map(|(item, _)| item).collect();
    }

    selected
}

fn select_public_briefing_items(context: &NewsTabContext, items: Vec<NewsItem>) -> Vec<NewsItem> {
    let mut scored = items
        .into_iter()
        .filter_map(|item| public_briefing_score(context, &item).map(|score| (item, score)))
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.0.timestamp.cmp(&left.0.timestamp))
    });

    let mut selected = scored
        .iter()
        .filter(|(_, score)| *score >= 110)
        .map(|(item, _)| item.clone())
        .collect::<Vec<_>>();

    if selected.len() < 4 {
        selected = scored.into_iter().take(4).map(|(item, _)| item).collect();
    }

    selected
}
fn build_stories(
    context: &NewsTabContext,
    items: Vec<NewsItem>,
) -> Result<Vec<NewsTabStory>, String> {
    let mut result = Vec::with_capacity(items.len());

    for item in items {
        let meta_label = build_meta_label(&item);
        let time_label = build_story_time_label(context, &item);
        let news_type = item.tipo.as_str().to_string();
        let importance = item.importancia.as_str().to_string();
        let importance_label = importance_label(&item.importancia).to_string();
        let accent_tone = story_accent(&item.importancia, &item.tipo).to_string();

        let category_label = item.categoria_nome.clone().or_else(|| {
            item.categoria_id
                .as_ref()
                .and_then(|id| context.category_names.get(id).cloned())
        });
        let team_label = item
            .team_id
            .as_ref()
            .and_then(|id| context.team_names.get(id).cloned());
        let driver_label = item
            .driver_id
            .as_ref()
            .and_then(|id| context.driver_names.get(id).cloned());
        let entity_label = team_label.clone().or_else(|| driver_label.clone());
        let race_label = item
            .categoria_id
            .as_ref()
            .and_then(|cat| item.rodada.map(|r| format!("{cat}:{r}")))
            .and_then(|key| context.race_labels.get(&key).cloned());
        let next_race_info = item
            .categoria_id
            .as_ref()
            .and_then(|cat| context.next_race_by_category.get(cat));
        let next_race_label = next_race_info.map(|r| r.label.clone());
        let next_race_date_label = next_race_info.map(|r| r.date_label.clone());
        let driver_secondary_label = item
            .driver_id_secondary
            .as_ref()
            .and_then(|id| context.driver_names.get(id).cloned());
        let driver_key = item
            .categoria_id
            .as_ref()
            .zip(item.driver_id.as_ref())
            .map(|(cat, drv)| format!("{cat}:{drv}"));
        let driver_position = driver_key
            .as_deref()
            .and_then(|k| context.driver_positions.get(k).copied());
        let driver_points = driver_key
            .as_deref()
            .and_then(|k| context.driver_points.get(k).copied());
        let team_position = item
            .team_id
            .as_ref()
            .and_then(|id| context.team_positions.get(id).copied());
        let team_points = item
            .team_id
            .as_ref()
            .and_then(|id| context.team_points.get(id).copied());
        let (team_color_primary, team_color_secondary) = item
            .team_id
            .as_ref()
            .and_then(|id| context.team_colors.get(id))
            .map(|(p, s)| (Some(p.clone()), Some(s.clone())))
            .unwrap_or((None, None));
        let team_public_presence_tier = item
            .team_id
            .as_ref()
            .and_then(|id| context.team_public_presence.get(id).cloned());

        let titulo = item.titulo.clone();
        let texto = item.texto.clone();
        let bundle = if item.tipo == NewsType::Corrida {
            compose_race_story_bundle(&item, context)?
        } else {
            Vec::new()
        };
        let pilot_composed = if item.tipo == NewsType::Hierarquia && item.driver_id.is_some() {
            compose_pilot_story(&item, context)?
        } else {
            None
        };
        let team_composed = if item.team_id.is_some()
            && item.driver_id.is_none()
            && item.tipo != NewsType::Mercado
            && item.tipo != NewsType::Incidente
            && item.tipo != NewsType::FramingSazonal
        {
            compose_team_story(&item, context)
        } else {
            None
        };
        let market_composed = if item.tipo == NewsType::Mercado {
            compose_market_story(&item, context)
        } else {
            None
        };
        let injury_composed = if item.tipo == NewsType::Lesao {
            compose_injury_story(&item, context)?
        } else {
            None
        };
        let incident_composed = if item.tipo == NewsType::Incidente {
            compose_incident_story(&item, context)?
        } else {
            None
        };
        let (headline, body_text) = if let Some(c) = bundle.first() {
            (c.headline.clone(), c.body.clone())
        } else if let Some(ref c) = pilot_composed {
            (c.headline.clone(), c.body.clone())
        } else if let Some(ref c) = team_composed {
            (c.headline.clone(), c.body.clone())
        } else if let Some(ref c) = market_composed {
            (c.headline.clone(), c.body.clone())
        } else if let Some(ref c) = injury_composed {
            (c.headline.clone(), c.body.clone())
        } else if let Some(ref c) = incident_composed {
            (c.headline.clone(), c.body.clone())
        } else {
            (titulo, texto)
        };
        let blocks: Vec<NewsTabStoryBlock> = Vec::new();

        let primary_story = NewsTabStory {
            id: item.id,
            icon: item.icone,
            title: headline.clone(),
            headline: headline.clone(),
            summary: String::new(),
            deck: String::new(),
            body_text,
            blocks,
            news_type,
            importance,
            importance_label,
            category_label,
            meta_label,
            time_label,
            entity_label,
            driver_label,
            team_label,
            race_label,
            accent_tone,
            driver_id: item.driver_id,
            team_id: item.team_id,
            round: item.rodada,
            original_text: Some(item.texto),
            preseason_week: item.semana_pretemporada,
            season_number: item.temporada,
            driver_id_secondary: item.driver_id_secondary,
            driver_secondary_label,
            driver_position,
            driver_points,
            team_position,
            team_points,
            team_color_primary,
            team_color_secondary,
            next_race_label,
            next_race_date_label,
            team_public_presence_tier,
        };

        // Segunda story: leader's bad result (Parte 5).
        // Reutiliza todos os campos da primary exceto id, headline e body.
        if let Some(second) = bundle.get(1) {
            let second_story = NewsTabStory {
                id: format!("{}_2", primary_story.id),
                title: second.headline.clone(),
                headline: second.headline.clone(),
                summary: String::new(),
                body_text: second.body.clone(),
                original_text: None,
                driver_id: None,
                driver_label: None,
                entity_label: None,
                ..primary_story.clone()
            };
            result.push(primary_story);
            result.push(second_story);
        } else {
            result.push(primary_story);
        }
    }

    Ok(result)
}

// ── Sistema editorial de Corrida — Parte 1 ───────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum RaceTrigger {
    LeadChanged,
    LeaderWon,
    ViceWon,
    /// Primeiro triunfo do piloto em toda a carreira.
    FirstWinOfCareer,
    /// Primeira vitória do piloto nesta temporada (mas não da carreira).
    FirstWinOfSeason,
    /// Vitória de piloto de posição muito baixa no campeonato (pos ≥ SHOCK_WIN_THRESHOLD).
    ShockWin,
    MidfieldDriverWon,
    LeaderHadBadResult,
    FallbackRaceResult,
}

/// Posição mínima no campeonato para considerar uma vitória como ShockWin.
const SHOCK_WIN_THRESHOLD: i32 = 8;

/// Posição de largada mínima para considerar uma vitória como "recuperação de posições".
const RECOVERY_WIN_MIN_GRID: i32 = 5;

struct RaceStoryContext {
    driver_name: String,
    category_name: String,
    driver_position: Option<i32>,
    #[allow(dead_code)]
    driver_points: Option<i32>,
    /// Sequência atual de vitórias consecutivas (0 se desconhecida).
    win_streak: u32,
    /// Semente para seleção de variante de texto (derivada de item.timestamp).
    item_seed: u64,
    /// True quando o piloto assumiu a liderança do campeonato nesta corrida.
    is_lead_change: bool,
    /// True se a notícia tem importância Destaque (vitória dominante/controlada).
    is_dominant_win: bool,
    /// Posição de chegada do rival principal na última corrida. None se desconhecida.
    rival_finish_position: Option<i32>,
    /// True se o rival principal abandonou (DNF) na última corrida.
    rival_dnf: bool,
    /// True se o piloto venceu largando da pole position (posicao_largada == 1).
    pole_plus_win: bool,
    /// True se o piloto venceu partindo da posicao_largada >= RECOVERY_WIN_MIN_GRID.
    recovery_win: bool,
    /// True se esta é a primeira vitória do piloto nesta temporada.
    first_win_of_season: bool,
    /// True se esta é a primeira vitória do piloto em toda a carreira.
    first_win_of_career: bool,
}

struct ComposedRaceStory {
    headline: String,
    body: String,
}

fn historical_standings_after_round(
    context: &NewsTabContext,
    season_number: i32,
    category_id: &str,
    round: i32,
) -> Result<Option<Vec<(String, i32, i32)>>, String> {
    let mut stmt = context
        .db
        .conn
        .prepare(
            "SELECT r.piloto_id,
                    CAST(ROUND(SUM(r.pontos)) AS INTEGER) AS total_points,
                    SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS wins,
                    SUM(CASE WHEN r.posicao_final <= 3 THEN 1 ELSE 0 END) AS podiums
             FROM race_results r
             JOIN calendar c ON r.race_id = c.id
             JOIN seasons s ON c.temporada_id = s.id
             WHERE s.numero = ?1 AND c.categoria = ?2 AND c.rodada <= ?3
             GROUP BY r.piloto_id
             ORDER BY total_points DESC, wins DESC, podiums DESC, r.piloto_id ASC",
        )
        .map_err(|e| format!("Falha ao preparar standings historicos: {e}"))?;

    let rows = stmt
        .query_map(
            rusqlite::params![season_number, category_id, round],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
        )
        .map_err(|e| format!("Falha ao consultar standings historicos: {e}"))?;

    let mut standings = Vec::new();
    for row in rows {
        let (driver_id, points) =
            row.map_err(|e| format!("Falha ao ler standings historicos: {e}"))?;
        standings.push((driver_id, standings.len() as i32 + 1, points));
    }

    if standings.is_empty() {
        Ok(None)
    } else {
        Ok(Some(standings))
    }
}

fn historical_round_results(
    context: &NewsTabContext,
    category_id: &str,
    round: i32,
) -> Result<Option<Vec<(String, i32, i32, bool)>>, String> {
    race_history_queries::get_results_for_round(
        &context.db.conn,
        &context.active_season_id,
        category_id,
        round,
    )
    .map(Some)
    .map_err(|e| format!("Falha ao consultar resultados historicos: {e}"))
}

fn incident_fact_type_from_source(source: Option<&str>) -> Option<IncidentFactType> {
    match source {
        Some("Mechanical") => Some(IncidentFactType::Mechanical),
        Some("DriverError") => Some(IncidentFactType::DriverError),
        Some("PostCollision") => Some(IncidentFactType::Collision),
        _ => None,
    }
}

fn historical_incident_facts_for_round(
    context: &NewsTabContext,
    category_id: &str,
    driver_id: &str,
    round: i32,
) -> Result<Option<IncidentEditorialFacts>, String> {
    let facts = race_history_queries::get_dnf_incident_facts_for_round(
        &context.db.conn,
        &context.active_season_id,
        category_id,
        round,
    )
    .map_err(|e| format!("Falha ao consultar incidentes historicos: {e}"))?;
    Ok(facts
        .into_iter()
        .find(|(candidate_driver_id, _, _, _)| candidate_driver_id == driver_id)
        .map(
            |(_, incident_source, is_dnf, segment)| IncidentEditorialFacts {
                incident_type: incident_fact_type_from_source(incident_source.as_deref()),
                is_dnf,
                segment,
            },
        ))
}

fn historical_driver_position_after_round(
    context: &NewsTabContext,
    season_number: i32,
    category_id: &str,
    driver_id: &str,
    round: i32,
) -> Result<Option<i32>, String> {
    Ok(
        historical_standings_after_round(context, season_number, category_id, round)?.and_then(
            |standings| {
                standings
                    .into_iter()
                    .find(|(candidate_driver_id, _, _)| candidate_driver_id == driver_id)
                    .map(|(_, position, _)| position)
            },
        ),
    )
}

fn historical_win_streak_through_round(
    context: &NewsTabContext,
    pilot_id: &str,
    season_number: i32,
    category_id: &str,
    round: i32,
) -> Result<Option<u32>, String> {
    let mut stmt = context
        .db
        .conn
        .prepare(
            "SELECT r.posicao_final
             FROM race_results r
             JOIN calendar c ON r.race_id = c.id
             JOIN seasons s ON c.temporada_id = s.id
             WHERE r.piloto_id = ?1
               AND s.numero = ?2
               AND c.categoria = ?3
               AND c.rodada <= ?4
             ORDER BY c.rodada DESC",
        )
        .map_err(|e| format!("Falha ao preparar win streak historico: {e}"))?;

    let positions = stmt
        .query_map(
            rusqlite::params![pilot_id, season_number, category_id, round],
            |row| row.get::<_, i32>(0),
        )
        .map_err(|e| format!("Falha ao consultar win streak historico: {e}"))?;
    let mut collected = Vec::new();
    for row in positions {
        collected.push(row.map_err(|e| format!("Falha ao ler win streak historico: {e}"))?);
    }

    Ok(Some(
        collected
            .iter()
            .take_while(|&&position| position == 1)
            .count() as u32,
    ))
}

fn historical_season_wins_through_round(
    context: &NewsTabContext,
    pilot_id: &str,
    season_number: i32,
    category_id: &str,
    round: i32,
) -> Result<Option<u32>, String> {
    let count = context
        .db
        .conn
        .query_row(
            "SELECT COUNT(*)
             FROM race_results r
             JOIN calendar c ON r.race_id = c.id
             JOIN seasons s ON c.temporada_id = s.id
             WHERE r.piloto_id = ?1
               AND s.numero = ?2
               AND c.categoria = ?3
               AND c.rodada <= ?4
               AND r.posicao_final = 1",
            rusqlite::params![pilot_id, season_number, category_id, round],
            |row| row.get::<_, i32>(0),
        )
        .map_err(|e| format!("Falha ao consultar vitorias historicas da temporada: {e}"))?;
    Ok(u32::try_from(count).ok())
}

fn historical_career_wins_through_round(
    context: &NewsTabContext,
    pilot_id: &str,
    season_number: i32,
    round: i32,
) -> Result<Option<u32>, String> {
    let count = context
        .db
        .conn
        .query_row(
            "SELECT COUNT(*)
             FROM race_results r
             JOIN calendar c ON r.race_id = c.id
             JOIN seasons s ON c.temporada_id = s.id
             WHERE r.piloto_id = ?1
               AND r.posicao_final = 1
               AND (
                 s.numero < ?2
                 OR (s.numero = ?2 AND c.rodada <= ?3)
               )",
            rusqlite::params![pilot_id, season_number, round],
            |row| row.get::<_, i32>(0),
        )
        .map_err(|e| format!("Falha ao consultar vitorias historicas da carreira: {e}"))?;
    Ok(u32::try_from(count).ok())
}

/// Compõe uma ou duas stories editoriais para um NewsItem de Corrida.
/// Retorna Vec vazio se não há composição possível (itens sem driver/categoria).
fn compose_race_story_bundle(
    item: &NewsItem,
    context: &NewsTabContext,
) -> Result<Vec<ComposedRaceStory>, String> {
    let driver_name = match item
        .driver_id
        .as_ref()
        .and_then(|id| context.driver_names.get(id).cloned())
    {
        Some(n) => n,
        None => return Ok(Vec::new()),
    };

    let category_name = match item.categoria_nome.clone().or_else(|| {
        item.categoria_id
            .as_ref()
            .and_then(|id| context.category_names.get(id).cloned())
    }) {
        Some(n) => n,
        None => return Ok(Vec::new()),
    };

    let driver_key = item
        .categoria_id
        .as_ref()
        .zip(item.driver_id.as_ref())
        .map(|(cat, drv)| format!("{cat}:{drv}"));
    let category_id = match item.categoria_id.as_deref() {
        Some(category_id) => category_id,
        None => return Ok(Vec::new()),
    };
    let driver_id_str = item.driver_id.as_deref().unwrap_or("");
    let editorial_round = item.rodada.filter(|round| *round > 0);
    let historical_standings = match editorial_round {
        Some(round) => {
            historical_standings_after_round(context, item.temporada, category_id, round)?
        }
        None => None,
    };
    let historical_results = match editorial_round {
        Some(round) => historical_round_results(context, category_id, round)?,
        None => None,
    };
    let historical_prev_leader = match editorial_round {
        Some(round) => race_history_queries::get_category_leader_before_round(
            &context.db.conn,
            &context.active_season_id,
            category_id,
            round,
        )
        .map_err(|e| format!("Falha ao consultar lider historico da categoria: {e}"))?,
        None => None,
    };

    let win_streak = editorial_round
        .map(|round| {
            historical_win_streak_through_round(
                context,
                driver_id_str,
                item.temporada,
                category_id,
                round,
            )
        })
        .transpose()?
        .flatten()
        .or_else(|| {
            driver_key
                .as_deref()
                .and_then(|k| context.driver_win_streaks.get(k).copied())
        })
        .unwrap_or(0);

    let driver_position = historical_standings
        .as_ref()
        .and_then(|standings| {
            standings
                .iter()
                .find(|(driver_id, _, _)| driver_id == driver_id_str)
                .map(|(_, position, _)| *position)
        })
        .or_else(|| {
            driver_key
                .as_deref()
                .and_then(|k| context.driver_positions.get(k).copied())
        });
    let driver_points = historical_standings
        .as_ref()
        .and_then(|standings| {
            standings
                .iter()
                .find(|(driver_id, _, _)| driver_id == driver_id_str)
                .map(|(_, _, points)| *points)
        })
        .or_else(|| {
            driver_key
                .as_deref()
                .and_then(|k| context.driver_points.get(k).copied())
        });

    // Houve mudança de liderança se: o piloto é agora o líder (pos 1) E era diferente do
    // líder anterior. Sem líder anterior (primeira corrida) → não é mudança.
    let is_lead_change = driver_position == Some(1)
        && historical_prev_leader.as_deref() != item.driver_id.as_deref()
        && historical_prev_leader.is_some();

    // ── Dados do próprio piloto (disponíveis antes da detecção do trigger) ─────
    let driver_race_result: Option<(i32, i32, bool)> = historical_results
        .as_ref()
        .and_then(|results| {
            results
                .iter()
                .find(|(driver_id, _, _, _)| driver_id == driver_id_str)
                .map(|(_, grid, finish, dnf)| (*grid, *finish, *dnf))
        })
        .or_else(|| {
            driver_key
                .as_deref()
                .and_then(|k| context.latest_race_results.get(k).copied())
        });
    let driver_grid_pos = driver_race_result.map(|(g, _, _)| g).unwrap_or(0);
    let season_wins = editorial_round
        .map(|round| {
            historical_season_wins_through_round(
                context,
                driver_id_str,
                item.temporada,
                category_id,
                round,
            )
        })
        .transpose()?
        .flatten()
        .or_else(|| context.driver_season_wins.get(driver_id_str).copied())
        .unwrap_or(0);
    let career_wins = editorial_round
        .map(|round| {
            historical_career_wins_through_round(context, driver_id_str, item.temporada, round)
        })
        .transpose()?
        .flatten()
        .or_else(|| context.driver_career_wins.get(driver_id_str).copied())
        .unwrap_or(0);

    // Contexto parcial — inclui win flags para que detect_race_trigger possa promovê-los.
    // Apenas os campos de rival ficam de fora (dependem do trigger).
    let partial_ctx = RaceStoryContext {
        driver_name: driver_name.clone(),
        category_name: category_name.clone(),
        driver_position,
        driver_points,
        win_streak,
        item_seed: item.timestamp as u64,
        is_lead_change,
        is_dominant_win: matches!(item.importancia, NewsImportance::Destaque),
        rival_finish_position: None,
        rival_dnf: false,
        pole_plus_win: driver_grid_pos == 1,
        recovery_win: driver_grid_pos >= RECOVERY_WIN_MIN_GRID,
        first_win_of_season: season_wins == 1,
        first_win_of_career: career_wins == 1,
    };

    let trigger = detect_race_trigger(&partial_ctx, &item.importancia);

    // Rival principal depende do trigger — capturado como String para reutilização.
    //   LeaderWon / LeadChanged / ViceWon têm rival definido; outros não.
    let rival_id_opt: Option<String> = match trigger {
        RaceTrigger::LeaderWon => historical_standings
            .as_ref()
            .and_then(|standings| standings.get(1))
            .map(|(driver_id, _, _)| driver_id.clone())
            .or_else(|| {
                context
                    .category_standings_top
                    .get(category_id)
                    .and_then(|standings| standings.get(1))
                    .cloned()
            }),
        RaceTrigger::LeadChanged => historical_prev_leader
            .clone()
            .or_else(|| context.category_prev_leaders.get(category_id).cloned()),
        RaceTrigger::ViceWon => historical_standings
            .as_ref()
            .and_then(|standings| standings.get(0))
            .map(|(driver_id, _, _)| driver_id.clone())
            .or_else(|| {
                context
                    .category_standings_top
                    .get(category_id)
                    .and_then(|standings| standings.get(0))
                    .cloned()
            }),
        _ => None,
    };

    let rival_result: Option<(i32, i32, bool)> = rival_id_opt.as_deref().and_then(|rival_id| {
        historical_results
            .as_ref()
            .and_then(|results| {
                results
                    .iter()
                    .find(|(driver_id, _, _, _)| driver_id == rival_id)
                    .map(|(_, grid, finish, dnf)| (*grid, *finish, *dnf))
            })
            .or_else(|| {
                context
                    .latest_race_results
                    .get(&format!("{category_id}:{rival_id}"))
                    .copied()
            })
    });

    let race_ctx = RaceStoryContext {
        rival_finish_position: rival_result.map(|(_, pos, _)| pos),
        rival_dnf: rival_result.map(|(_, _, dnf)| dnf).unwrap_or(false),
        ..partial_ctx
    };

    let main_body = match compose_race_body(trigger, &race_ctx) {
        Some(b) => b,
        None => return Ok(Vec::new()),
    };

    let mut bundle = vec![ComposedRaceStory {
        headline: item.titulo.clone(),
        body: main_body,
    }];

    // Parte 5: segunda story — LeaderHadBadResult — quando o trigger principal
    // é ViceWon ou LeadChanged e o rival (líder) teve um resultado muito ruim.
    if should_generate_second_story(&race_ctx, trigger) {
        if let Some(rival_id) = &rival_id_opt {
            if let Some(leader_name) = context.driver_names.get(rival_id).cloned() {
                let leader_ctx = LeaderBadResultContext {
                    leader_name,
                    category_name: race_ctx.category_name.clone(),
                    finish_position: if race_ctx.rival_dnf {
                        None
                    } else {
                        race_ctx.rival_finish_position
                    },
                    is_dnf: race_ctx.rival_dnf,
                    seed: race_ctx.item_seed,
                };
                bundle.push(compose_leader_bad_result_story(&leader_ctx));
            }
        }
    }

    Ok(bundle)
}

// ── Sistema editorial de Corrida — Parte 5: story dupla ─────────────────────

/// Threshold de posição para considerar que o líder teve um resultado ruim o suficiente
/// para gerar uma segunda story independente.
const LEADER_BAD_RESULT_THRESHOLD: i32 = 8;

struct LeaderBadResultContext {
    leader_name: String,
    category_name: String,
    /// Posição final do líder. None se DNF.
    finish_position: Option<i32>,
    is_dnf: bool,
    seed: u64,
}

/// Retorna true se o contexto da corrida justifica uma segunda story sobre o mau resultado
/// do líder. Só se aplica quando o trigger principal é ViceWon ou LeadChanged.
fn should_generate_second_story(ctx: &RaceStoryContext, trigger: RaceTrigger) -> bool {
    matches!(trigger, RaceTrigger::ViceWon | RaceTrigger::LeadChanged)
        && (ctx.rival_dnf
            || ctx
                .rival_finish_position
                .map_or(false, |p| p >= LEADER_BAD_RESULT_THRESHOLD))
}

fn compose_leader_bad_result_story(ctx: &LeaderBadResultContext) -> ComposedRaceStory {
    let l = &ctx.leader_name;
    let c = &ctx.category_name;
    let v = ctx.seed as usize;

    let headline = match v % 2 {
        0 => format!("{l} atravessa rodada difícil e vê rivais encostarem em {c}"),
        _ => format!("Líder perde terreno: {l} sai da etapa sob pressão crescente"),
    };

    let body = if ctx.is_dnf {
        match v % 2 {
            0 => format!(
                "O abandono de {l} nesta etapa é exatamente o tipo de resultado que os \
                 rivais aguardavam. Com zero pontos na rodada, a vantagem que o líder \
                 havia construído em {c} ficou mais frágil do que estava há uma semana."
            ),
            _ => format!(
                "{l} abandonou antes de ver a bandeira quadriculada — e as consequências \
                 no campeonato de {c} serão sentidas nas próximas semanas. Rodadas assim \
                 não se apagam da tabela."
            ),
        }
    } else {
        let pos = ctx.finish_position.unwrap_or(0);
        let ordinal = format!("{pos}º");
        match v % 2 {
            0 => format!(
                "A {ordinal} posição de {l} é exatamente o tipo de resultado que os \
                 rivais do campeonato de {c} aguardavam. A vantagem que o líder havia \
                 construído perdeu parte do seu peso neste fim de semana."
            ),
            _ => format!(
                "Terminar em {ordinal} não estava no script de {l}. A etapa custou \
                 pontos que fazem falta, e o campeonato de {c} chega à próxima rodada \
                 com a margem no topo sensivelmente menor."
            ),
        }
    };

    ComposedRaceStory { headline, body }
}

fn detect_race_trigger(ctx: &RaceStoryContext, importancia: &NewsImportance) -> RaceTrigger {
    let is_high = matches!(importancia, NewsImportance::Alta | NewsImportance::Destaque);
    match ctx.driver_position {
        // P1 no campeonato — triggers mais fortes possíveis
        Some(1) if is_high && ctx.is_lead_change => RaceTrigger::LeadChanged,
        Some(1) if is_high => RaceTrigger::LeaderWon,
        Some(1) => RaceTrigger::LeaderHadBadResult,
        // P2 no campeonato
        Some(2) if is_high => RaceTrigger::ViceWon,
        // P3+ no campeonato — FirstWin é o eixo mais forte disponível
        Some(_) if is_high && ctx.first_win_of_career => RaceTrigger::FirstWinOfCareer,
        Some(_) if is_high && ctx.first_win_of_season => RaceTrigger::FirstWinOfSeason,
        // P8+ = ShockWin; P5-7 = MidfieldDriverWon
        Some(pos) if pos >= SHOCK_WIN_THRESHOLD && is_high => RaceTrigger::ShockWin,
        Some(pos) if pos >= 5 && is_high => RaceTrigger::MidfieldDriverWon,
        _ => RaceTrigger::FallbackRaceResult,
    }
}

// ── Sistema editorial de Corrida — Parte 2: progressão por sequência ─────────

/// Converte uma sequência bruta de vitórias num bucket de 1 a 11.
/// 0 ou 1 → bucket 1; 2-10 → bucket igual; 11+ → bucket 11.
fn streak_bucket(streak: u32) -> u32 {
    streak.max(1).min(11)
}

fn compose_leader_won_body(d: &str, c: &str, bucket: u32, v: usize) -> String {
    match (bucket, v % 2) {
        (1, 0) => format!(
            "{d} confirma o que o campeonato já vinha sugerindo: há um patamar acima \
             do restante do grid em {c} nesta temporada. Esse resultado praticamente \
             fecha o debate sobre o favoritismo — pelo menos por mais algumas rodadas."
        ),
        (1, _) => format!(
            "Corrida controlada. {d} foi buscar a vitória onde ela estava — na frente, \
             com margem, sem drama. Em {c}, começa a se desenhar uma hierarquia que o \
             restante do grid ainda vai precisar encontrar resposta."
        ),
        (2, 0) => format!(
            "Duas vitórias seguidas. {d} começa a construir uma cadência em {c} que os \
             rivais ainda não encontraram como quebrar. A regularidade, quando se instala \
             assim, costuma ser difícil de remover."
        ),
        (2, _) => format!(
            "O segundo triunfo consecutivo de {d} em {c} não é coincidência — é padrão. \
             E padrões assim tendem a se tornar temporadas."
        ),
        (3, 0) => format!(
            "Três rodadas seguidas no alto do pódio. {d} transformou {c} em território \
             próprio e os rivais, por ora, assistem de longe. A sequência já passou do \
             ponto em que alguém pode chamar de sorte."
        ),
        (3, _) => format!(
            "Há três rodadas que {d} não deixa espaço em {c}. O campeonato ainda está \
             aberto no papel — mas o que acontece na pista conta outra história."
        ),
        (4, 0) => format!(
            "Quatro vitórias seguidas. {d} não está apenas vencendo em {c} — está \
             estabelecendo um ritmo que o grid inteiro tenta acompanhar e ainda não \
             consegue. A temporada começa a ganhar um protagonista."
        ),
        (4, _) => format!(
            "Quando a sequência chega a quatro, ela começa a ter peso próprio. {d} em \
             {c} está num ciclo onde confiança, acerto e resultado se alimentam \
             mutuamente — e isso é difícil de interromper de fora."
        ),
        (5, 0) => format!(
            "Cinco rodadas, cinco vitórias. {d} encontrou em {c} aquele equilíbrio raro \
             entre piloto e máquina que a maioria das temporadas não chega a ver. A \
             pergunta já não é se vai ganhar — é até quando."
        ),
        (5, _) => format!(
            "A quinta vitória seguida de {d} em {c} começa a ter a textura de algo que \
             não se explica só com técnica ou estratégia. Tem um componente psicológico \
             agora — e ele pesa dos dois lados."
        ),
        (6, 0) => format!(
            "Seis vitórias consecutivas. {d} está escrevendo uma versão de {c} em que \
             os outros aparecem para disputar o segundo lugar. Isso é duro de aceitar \
             para os rivais — mas é o que os números dizem."
        ),
        (6, _) => format!(
            "{d} chegou ao sexto triunfo seguido em {c} de uma forma que já nem precisa \
             de adjetivos. A sequência fala por si — e o que ela diz é que o campeonato \
             tem um dono por enquanto."
        ),
        (7, 0) => format!(
            "Sete. {d} transformou {c} em algo que os adversários só conseguem observar \
             de longe. Não há mais fórmula para quebrar esse ritmo de fora — precisaria \
             de um erro interno, e esse erro não vem chegando."
        ),
        (7, _) => format!(
            "A sétima vitória seguida numa categoria que, há algumas rodadas, ainda \
             parecia ter equilíbrio. {d} não deu tempo para a disputa se organizar. \
             Agora o campeonato é uma pergunta de uma única resposta."
        ),
        (8, 0) => format!(
            "Oito vitórias consecutivas. {d} não está mais numa fase — está numa era. \
             O que acontece dentro desse cockpit em {c} rodada após rodada seria difícil \
             de replicar mesmo que os rivais soubessem exatamente o que é."
        ),
        (8, _) => format!(
            "Quando a sequência passa de oito, a narrativa muda. {d} em {c} não é mais \
             só dominante — é referência. O que os outros estão tentando construir, ele \
             já tem faz rodadas."
        ),
        (9, 0) => format!(
            "A nona vitória seguida de {d} em {c} já tem um estatuto que vai além desta \
             temporada. É o tipo de sequência que entra nos registros — e que muda a \
             forma como todos os outros pilotos encaram a corrida antes mesmo do começo."
        ),
        (9, _) => format!(
            "Nove de seguida. {d} está fazendo em {c} algo que os manuais de campeonato \
             raramente registram: uma consistência tão completa que parece inevitável. A \
             diferença entre sorte e domínio ficou para trás há muito tempo."
        ),
        (10, 0) => format!(
            "Dez vitórias consecutivas. {d} fez algo em {c} que vai ser lembrado muito \
             depois de esta temporada terminar. Não se trata mais de campeonato — isso \
             já entrou na história da categoria."
        ),
        (10, _) => format!(
            "A décima vitória seguida de {d} em {c} marca um limiar que poucos pilotos \
             chegam perto. O que começou como uma boa fase tornou-se um capítulo \
             separado — e não há sinal de que está prestes a fechar."
        ),
        (_, 0) => format!(
            "Além do décimo triunfo consecutivo, {d} está num território onde as \
             comparações começam a escassear em {c}. As sequências assim não se analisam \
             enquanto acontecem — se contemplam. E ainda estão acontecendo."
        ),
        _ => format!(
            "A sequência de {d} em {c} já ultrapassou qualquer referência razoável para \
             uma temporada. O que está acontecendo vai precisar de perspectiva para ser \
             compreendido. Por enquanto, só resta acompanhar."
        ),
    }
}

fn compose_vice_won_body(d: &str, c: &str, bucket: u32, v: usize) -> String {
    match (bucket, v % 2) {
        (1, 0) => format!(
            "{d} respondeu na pista à única linguagem que importa em {c}: pontos. Com \
             esse resultado, a perseguição pelo título ganha outra cara — e o pelotão \
             da frente vai precisar recalcular a rota."
        ),
        (1, _) => format!(
            "Quando {d} vence em {c}, a tabela se mexe de um jeito que incomoda quem \
             está na frente. Não é só o resultado de hoje — é o sinal de que a pressão \
             não vai diminuir."
        ),
        (2, 0) => format!(
            "Segunda vitória seguida de {d} em {c}. A perseguição ao título ganhou \
             fôlego — e isso não é pouco num campeonato onde o ritmo do líder ainda \
             era a principal certeza da grade."
        ),
        (2, _) => format!(
            "Duas de seguida. {d} está transformando a pressão de quem persegue em \
             pontos concretos. Em {c}, o campeonato parou de ser uma pergunta retórica."
        ),
        (3, 0) => format!(
            "Três vitórias seguidas de {d} em {c}. O que era perseguição começa a \
             ganhar contornos de briga de fato. O líder ainda está à frente — mas o \
             chão debaixo da vantagem está encolhendo a cada rodada."
        ),
        (3, _) => format!(
            "Três de seguida. {d} está encontrando em {c} uma cadência que o líder do \
             campeonato não esperava enfrentar. A tabela ainda pende para um lado — o \
             momentum pende para o outro."
        ),
        (4, 0) => format!(
            "Quatro vitórias seguidas e {d} reescreveu o roteiro do campeonato de {c}. \
             O que era uma disputa com favorito claro passou a ter dois protagonistas — \
             e o segundo veio de baixo."
        ),
        (4, _) => format!(
            "{d} chegou à quarta vitória seguida em {c} e colocou a temporada num ponto \
             de inflexão. Quando a sequência de quem persegue se equipara à do líder, o \
             campeonato muda de natureza."
        ),
        (5, 0) => format!(
            "Cinco vitórias seguidas de {d} em {c} transformaram o que era desvantagem \
             em vantagem psicológica. O líder ainda tem pontos. {d} tem o momentum — e \
             nos últimos cinco fins de semana, o momentum valeu mais."
        ),
        (5, _) => format!(
            "A quinta de seguida. {d} está num ciclo em {c} que o líder do campeonato \
             não conseguiu interromper em cinco tentativas. A pergunta muda: não é mais \
             'vai chegar?', mas 'quando vai passar?'."
        ),
        (6, 0) => format!(
            "Seis vitórias seguidas em {c}. {d} não está mais perseguindo o campeonato \
             — está moldando ele. A tabela talvez ainda não reflita isso. O que acontece \
             na pista, reflete."
        ),
        (6, _) => format!(
            "A sexta de seguida de {d} em {c} é mais do que uma sequência individual \
             — é a história de um campeonato que virou. Quem assistiu ao começo da \
             temporada não reconheceria essa tabela."
        ),
        (7, 0) => format!(
            "Sete vitórias consecutivas. {d} transformou a perseguição em algo que os \
             rivais em {c} passaram a temer. A vantagem do líder, se ainda existe, está \
             sendo consumida num ritmo que já não parece sustentável do outro lado."
        ),
        (7, _) => format!(
            "A sétima seguida de {d} em {c} tem uma qualidade diferente: não parece \
             mais reversível. O que começou como uma recuperação tornou-se uma narrativa \
             própria — e essa narrativa aponta para cima."
        ),
        (8, 0) => format!(
            "Oito vitórias seguidas. {d} construiu em {c} uma sequência que faria \
             qualquer campeonato dobrar. Se há um líder formal ainda à frente, é porque \
             os números do passado carregam peso. O presente já decidiu o seu lado."
        ),
        (8, _) => format!(
            "Oito de seguida. {d} está fazendo em {c} o que raramente se vê de quem \
             veio atrás: uma sequência tão consistente que apagou a diferença inicial \
             e criou uma nova. E esta nova diferença favorece quem chegou depois."
        ),
        (9, 0) => format!(
            "Nove vitórias consecutivas de {d} em {c}. Houve um ponto nessa temporada \
             em que essa sequência parecia impossível. Agora ela está aqui — e o \
             campeonato, que tinha dono claro, entrou num território que ninguém planejou."
        ),
        (9, _) => format!(
            "A nona de seguida. O que {d} está fazendo em {c} começa a criar uma \
             perspectiva que vai durar além desta temporada: a de que, quando tudo se \
             alinha, a diferença na largada pode ser desmontada round a round."
        ),
        (10, 0) => format!(
            "Dez vitórias seguidas e {d} fez de {c} um palco para uma das recuperações \
             mais completas que uma temporada pode registrar. Não importa o que acontecer \
             a partir de agora — esta sequência já tem endereço próprio na história da \
             categoria."
        ),
        (10, _) => format!(
            "A décima de seguida de {d} em {c} não é mais perseguição — é protagonismo. \
             Quem vem com dez vitórias seguidas não está atrás de ninguém. Quem tem isso \
             na manga está à frente, seja lá o que diga a tabela."
        ),
        (_, 0) => format!(
            "Além da décima vitória consecutiva, {d} está fazendo em {c} algo que não \
             se encaixa mais nas categorias habituais de análise. Uma sequência deste \
             comprimento, partindo da posição que partiu, é para ser registrada — não \
             explicada."
        ),
        _ => format!(
            "A sequência de vitórias de {d} em {c} já ultrapassou qualquer parâmetro \
             razoável para uma temporada de perseguição. O que começou como uma \
             recuperação tornou-se algo que esta categoria não via há muito tempo — \
             se é que viu."
        ),
    }
}

// ── Sistema editorial de Corrida — Parte 3: mudança de liderança ─────────────

/// Bucket simplificado para LeadChanged: 1 / 2-3 / 4+.
fn lead_change_bucket(streak: u32) -> u32 {
    match streak {
        0 | 1 => 1,
        2 | 3 => 2,
        _ => 3,
    }
}

fn compose_lead_changed_body(d: &str, c: &str, bucket: u32, v: usize) -> String {
    match (bucket, v % 2) {
        (1, 0) => format!(
            "{d} venceu e tomou a liderança do campeonato no momento em que a disputa \
             da ponta mais precisava de uma ruptura. A troca muda o eixo da temporada \
             e devolve pressão imediata a quem antes controlava a frente."
        ),
        (1, _) => format!(
            "A vitória de {d} em {c} tinha esse subtexto que só aparece no placar: \
             mudança de liderança. O resultado da corrida foi importante — mas o que \
             ele fez com a tabela foi o que vai definir a narrativa das próximas rodadas."
        ),
        (2, 0) => format!(
            "{d} transformou a sequência recente em mudança concreta de topo e agora \
             aparece como novo líder do campeonato. A troca de posição na tabela dá \
             outro peso à fase que ele já vinha construindo nas últimas corridas."
        ),
        (2, _) => format!(
            "O que vinha se construindo nas últimas rodadas ganhou um endereço \
             definitivo: {d} é o novo líder do campeonato de {c}. A sequência de \
             bons resultados não se limitou a encolher a desvantagem — ela a apagou."
        ),
        (_, 0) => format!(
            "{d} empurrou a própria fase para outro patamar ao transformar a sequência \
             de vitórias em liderança do campeonato. O topo muda de mãos num momento \
             em que o restante do grid já começa a correr em reação ao seu ritmo."
        ),
        _ => format!(
            "Tinha ficado claro nos últimos fins de semana que algo ia se quebrar na \
             tabela de {c}. {d} só precisou de mais uma rodada para completar o \
             processo: saiu de dentro de uma fase dominante e chegou ao topo do \
             campeonato."
        ),
    }
}

// ── Sistema editorial de Corrida — Parte 4: modificadores ────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum RaceModifier {
    /// Rival principal abandonou (DNF). Prioridade 1.
    MainRivalDnf,
    /// Rival principal terminou longe (posição 6+). Prioridade 2.
    MainRivalFinishedFar { position: i32 },
    /// Rival principal terminou perto (posição 2–5). Prioridade 3.
    MainRivalFinishedClose,
    /// Corrida vencida de forma dominante (importância Destaque). Prioridade 4.
    DominantWin,
    /// Vitória partindo de posição baixa no grid (recuperação). Prioridade 5.
    RecoveryWin,
    /// Venceu largando da pole position. Prioridade 6.
    PolePlusWin,
    /// Primeira vitória de toda a carreira do piloto. Prioridade 7.
    FirstWinOfCareer,
    /// Primeira vitória desta temporada (mas não da carreira). Prioridade 8.
    FirstWinOfSeason,
}

/// Detecta modificadores aplicáveis ao contexto da corrida.
/// Prioridade: RivalDnf > RivalFar > RivalClose > Dominant > Recovery > Pole > FirstCareer > FirstSeason.
/// Máximo de 2 modificadores. Aplicável a qualquer trigger de vitória.
/// Modificadores que já são o trigger principal são automaticamente ignorados.
fn detect_race_modifiers(ctx: &RaceStoryContext, trigger: RaceTrigger) -> Vec<RaceModifier> {
    let is_win_trigger = matches!(
        trigger,
        RaceTrigger::LeaderWon
            | RaceTrigger::LeadChanged
            | RaceTrigger::ViceWon
            | RaceTrigger::FirstWinOfCareer
            | RaceTrigger::FirstWinOfSeason
            | RaceTrigger::ShockWin
            | RaceTrigger::MidfieldDriverWon
    );
    if !is_win_trigger {
        return Vec::new();
    }

    let has_rival = matches!(
        trigger,
        RaceTrigger::LeaderWon | RaceTrigger::LeadChanged | RaceTrigger::ViceWon
    );

    let mut modifiers: Vec<RaceModifier> = Vec::new();

    // P1–P3: rival-based (só triggers com rival conhecido)
    if has_rival {
        if ctx.rival_dnf {
            modifiers.push(RaceModifier::MainRivalDnf);
        } else if let Some(pos) = ctx.rival_finish_position {
            if pos >= 6 {
                modifiers.push(RaceModifier::MainRivalFinishedFar { position: pos });
            } else if pos >= 2 {
                modifiers.push(RaceModifier::MainRivalFinishedClose);
            }
        }
    }

    // P4: DominantWin
    if ctx.is_dominant_win && modifiers.len() < 2 {
        modifiers.push(RaceModifier::DominantWin);
    }

    // P5: RecoveryWin
    if ctx.recovery_win && modifiers.len() < 2 {
        modifiers.push(RaceModifier::RecoveryWin);
    }

    // P6: PolePlusWin
    if ctx.pole_plus_win && modifiers.len() < 2 {
        modifiers.push(RaceModifier::PolePlusWin);
    }

    // P7: FirstWinOfCareer — ignorado se já é o trigger principal
    if ctx.first_win_of_career && trigger != RaceTrigger::FirstWinOfCareer && modifiers.len() < 2 {
        modifiers.push(RaceModifier::FirstWinOfCareer);
    } else if ctx.first_win_of_season
        && !matches!(
            trigger,
            RaceTrigger::FirstWinOfCareer | RaceTrigger::FirstWinOfSeason
        )
        && modifiers.len() < 2
    {
        // P8: FirstWinOfSeason — ignorado se já é o trigger ou se FirstWinOfCareer é o trigger
        modifiers.push(RaceModifier::FirstWinOfSeason);
    }

    modifiers
}

fn compose_modifier_phrase(modifier: RaceModifier, ctx: &RaceStoryContext, v: usize) -> String {
    let d = &ctx.driver_name;
    match modifier {
        RaceModifier::DominantWin => match v % 2 {
            0 => "A margem no final não deixou dúvidas sobre quem controlou a corrida \
                  de ponta a ponta."
                .to_string(),
            _ => format!(
                "{d} não apenas venceu — dominou. Ritmo, liderança e gestão foram \
                 de uma categoria acima neste fim de semana."
            ),
        },
        RaceModifier::MainRivalFinishedClose => match v % 2 {
            0 => "O rival direto terminou nas proximidades, mas não o suficiente \
                  para ameaçar o que foi construído na frente."
                .to_string(),
            _ => format!(
                "A ameaça existia — terminou perto — mas {d} soube administrar \
                 a diferença quando importava."
            ),
        },
        RaceModifier::MainRivalFinishedFar { position } => {
            let ordinal = format!("{position}º");
            match v % 2 {
                0 => format!(
                    "O principal rival do campeonato atravessou um fim de semana \
                     difícil e terminou em {ordinal} — distante da briga pelo título."
                ),
                _ => format!(
                    "Enquanto {d} somava pontos, o adversário de campeonato \
                     naufragava até o {ordinal} lugar. A lacuna na tabela vai crescer."
                ),
            }
        }
        RaceModifier::MainRivalDnf => match v % 2 {
            0 => "O rival mais próximo na tabela abandonou antes do fim — o que \
                  torna esse resultado ainda mais valioso no cômputo geral."
                .to_string(),
            _ => "Abandono do principal adversário na mesma corrida transforma uma \
                  vitória já importante em algo que pode definir o campeonato."
                .to_string(),
        },
        RaceModifier::RecoveryWin => match v % 2 {
            0 => format!(
                "{d} não chegou à frente pelo caminho mais curto — chegou pelo mais \
                 difícil. Partir de longe e cruzar a linha na primeira posição é o tipo \
                 de resultado que muda a narrativa de um fim de semana."
            ),
            _ => format!(
                "A vitória de {d} veio de trás. Cada posição conquistada ao longo da \
                 corrida foi parte do placar — e o placar final foi máximo."
            ),
        },
        RaceModifier::PolePlusWin => match v % 2 {
            0 => format!(
                "{d} controlou o fim de semana do início ao fim: pole, ritmo de corrida \
                 e bandeira quadriculada. Não há muito a questionar nesse pacote."
            ),
            _ => format!(
                "Da pole à vitória, {d} não deu brechas. Quando o fim de semana começa \
                 assim, raramente termina de outro jeito."
            ),
        },
        RaceModifier::FirstWinOfCareer => match v % 2 {
            0 => format!(
                "É a primeira vitória na carreira de {d} — e chegou em grande estilo. \
                 Esse resultado vai figurar nas primeiras linhas do currículo por muito \
                 tempo."
            ),
            _ => format!(
                "{d} abre a conta de vitórias na carreira. O primeiro triunfo é sempre \
                 o mais especial — e este foi conquistado da forma certa."
            ),
        },
        RaceModifier::FirstWinOfSeason => match v % 2 {
            0 => format!(
                "É a primeira vitória de {d} nesta temporada — e o momento não poderia \
                 ser melhor para desbloquear o triunfo e entrar de vez na conversa pelo \
                 campeonato."
            ),
            _ => format!(
                "{d} vence pela primeira vez nesta temporada. O bloqueio está superado \
                 e a sequência pode começar agora."
            ),
        },
    }
}

// ── Sistema editorial de Corrida — Parte 7: triggers principais raros ────────

fn compose_first_win_of_career_body(d: &str, c: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "A espera acabou. {d} cruzou a linha de chegada em primeiro lugar em {c} \
             pela primeira vez na carreira — e esse resultado vai permanecer nos \
             registros por muito tempo. Vitórias de estreia não se repetem."
        ),
        1 => format!(
            "Não é fácil vencer pela primeira vez. {d} sabe disso melhor do que \
             ninguém agora. A vitória em {c} encerra um ciclo e abre outro — o de um \
             piloto que provou que é capaz de chegar lá quando importa."
        ),
        _ => format!(
            "Primeira vitória na carreira de {d}. Em {c}, num fim de semana que \
             poucos vão esquecer. O caminho percorrido até aqui está todo ali, na \
             linha de chegada."
        ),
    }
}

fn compose_first_win_of_season_body(d: &str, c: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "Era só uma questão de tempo. {d} converteu ritmo em resultado e \
             conquistou a primeira vitória da temporada em {c}. O bloqueio está \
             superado — e o campeonato pode se redesenhar a partir daqui."
        ),
        1 => format!(
            "A primeira vitória de {d} nesta temporada em {c} chega num momento \
             oportuno. Com o triunfo desbloqueado, a conversa sobre o campeonato \
             inclui um nome a mais na lista de candidatos."
        ),
        _ => format!(
            "{d} estreia na coluna de vitórias desta temporada em {c}. Às vezes é \
             isso que faltava — um resultado que muda o ritmo e a leitura do \
             campeonato que está por vir."
        ),
    }
}

fn compose_shock_win_body(d: &str, c: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "Ninguém estava esperando por isso. {d} chegou de longe na tabela do \
             campeonato de {c}, venceu a corrida e virou o fim de semana de cabeça \
             para baixo. Resultados assim não cabem em nenhum modelo."
        ),
        1 => format!(
            "A vitória de {d} em {c} vai exigir algumas releituras. Não era o nome \
             favorito, não era o mais badalado — mas foi o mais rápido quando \
             importava. O campeonato tem mais um fator a equacionar."
        ),
        _ => format!(
            "Surpresa total em {c}. {d} aproveitou cada brecha que a corrida ofereceu \
             e entregou um resultado que ninguém tinha no roteiro desta etapa. Em \
             corridas assim, o script fica de lado."
        ),
    }
}

fn compose_race_body(trigger: RaceTrigger, ctx: &RaceStoryContext) -> Option<String> {
    let d = &ctx.driver_name;
    let c = &ctx.category_name;
    let v = ctx.item_seed as usize;
    let main_body = match trigger {
        RaceTrigger::LeadChanged => {
            let bucket = lead_change_bucket(ctx.win_streak);
            compose_lead_changed_body(d, c, bucket, v)
        }
        RaceTrigger::LeaderWon => {
            let bucket = streak_bucket(ctx.win_streak);
            compose_leader_won_body(d, c, bucket, v)
        }
        RaceTrigger::ViceWon => {
            let bucket = streak_bucket(ctx.win_streak);
            compose_vice_won_body(d, c, bucket, v)
        }
        RaceTrigger::FirstWinOfCareer => compose_first_win_of_career_body(d, c, v),
        RaceTrigger::FirstWinOfSeason => compose_first_win_of_season_body(d, c, v),
        RaceTrigger::ShockWin => compose_shock_win_body(d, c, v),
        RaceTrigger::MidfieldDriverWon => format!(
            "Isso não estava no roteiro. {d} saiu do meio do pelotão, aproveitou cada \
             centímetro da janela que se abriu em {c} e entregou uma corrida calculada \
             do começo ao fim. Resultados assim não se explicam — se presenciam."
        ),
        RaceTrigger::LeaderHadBadResult => format!(
            "O líder do campeonato em {c} deixou pontos preciosos na pista num fim \
             de semana para esquecer. A janela para os perseguidores se abre — e os \
             rivais diretos não vão demorar para passar por ela."
        ),
        RaceTrigger::FallbackRaceResult => return None,
    };

    let modifiers = detect_race_modifiers(ctx, trigger);
    if modifiers.is_empty() {
        return Some(main_body);
    }

    let mut parts = vec![main_body];
    for modifier in &modifiers {
        parts.push(compose_modifier_phrase(*modifier, ctx, v));
    }
    Some(parts.join(" "))
}

// ── Sistema editorial de Piloto — Parte 8 ────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum PilotTrigger {
    /// Piloto em fase forte — streak recente, posição de topo, importância alta.
    PilotInStrongForm,
    /// Piloto chega pressionado — resultados ruins, importância baixa/média.
    PilotUnderPressure,
    /// Piloto saiu da periferia e virou pauta — mid-range com tração.
    PilotBecameRelevant,
    /// Mudança clara de momento — importância Destaque, fase virou.
    PilotMomentumShift,
    /// Fallback: contexto insuficiente para trigger específico.
    FallbackPilotStory,
}

struct PilotStoryContext {
    driver_name: String,
    category_name: String,
    /// Posição no campeonato. None se desconhecida.
    driver_position: Option<i32>,
    /// Sequência atual de vitórias (0 se nenhuma / desconhecida).
    win_streak: u32,
    /// Semente para variante de texto.
    item_seed: u64,
    // ── Parte 9: campos para modificadores ───────────────────────────────────
    /// Posição de chegada na última rodada disputada. None se não disputou.
    last_race_finish: Option<i32>,
    /// Se abandonou (DNF) na última rodada disputada.
    last_race_dnf: bool,
    /// Diferença de pontos para o líder do campeonato. None se desconhecida.
    points_gap_to_leader: Option<i32>,
}

fn detect_pilot_trigger(ctx: &PilotStoryContext, importancia: &NewsImportance) -> PilotTrigger {
    let is_high = matches!(importancia, NewsImportance::Alta | NewsImportance::Destaque);
    let is_top = matches!(importancia, NewsImportance::Destaque);

    // Importância baixa/média → piloto sob pressão
    if !is_high {
        return PilotTrigger::PilotUnderPressure;
    }

    // Destaque + sequência clara → forma forte indiscutível
    if is_top && ctx.win_streak >= 2 {
        return PilotTrigger::PilotInStrongForm;
    }

    // Destaque sem sequência → virada de fase (momentum shift)
    if is_top {
        return PilotTrigger::PilotMomentumShift;
    }

    // Alta + posição de topo (P1-P3) → forma forte
    if matches!(ctx.driver_position, Some(p) if p <= 3) {
        return PilotTrigger::PilotInStrongForm;
    }

    // Alta + mid-range (P4-P8) → virou pauta
    if matches!(ctx.driver_position, Some(p) if p <= 8) {
        return PilotTrigger::PilotBecameRelevant;
    }

    PilotTrigger::FallbackPilotStory
}

fn compose_pilot_strong_form_body(d: &str, c: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "{d} entra numa fase cada vez mais sólida em {c} e começa a deixar de \
             ser coadjuvante na disputa principal. A sequência recente mudou o ritmo \
             de como ele aparece na tabela — e a trajetória agora aponta para cima."
        ),
        1 => format!(
            "O trabalho de {d} em {c} virou referência neste trecho da temporada. \
             Os resultados são consistentes, o grid já não o trata como surpresa — \
             e o campeonato passou a contar com esse nome na conversa pela frente."
        ),
        _ => format!(
            "{d} está subindo de patamar em {c}. A acumulação de bons resultados \
             recentes deixou de ser coincidência e passou a ser argumento real na \
             disputa pelo topo da tabela."
        ),
    }
}

fn compose_pilot_under_pressure_body(d: &str, c: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "{d} chega à próxima etapa em {c} com menos margem do que nas rodadas \
             anteriores e já corre sob cobrança mais visível por resposta. O momento \
             recente deixou de ser oscilação isolada e passou a pesar de verdade \
             sobre sua temporada."
        ),
        1 => format!(
            "A pressão sobre {d} aumentou visivelmente nas últimas semanas em {c}. \
             O que antes parecia um ciclo ruim começa a ganhar contornos de problema \
             real — e as próximas etapas vão dizer se é passageiro ou estrutural."
        ),
        _ => format!(
            "{d} perdeu margem e ganhou cobrança. O trecho atual em {c} não está \
             combinando com o ritmo esperado para o momento da temporada, e o \
             campeonato não espera."
        ),
    }
}

fn compose_pilot_became_relevant_body(d: &str, c: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "{d} saiu do bloco lateral da temporada e agora aparece como nome real \
             da conversa sobre a frente do campeonato em {c}. O crescimento recente \
             fez o paddock e a tabela olharem para ele com outra régua."
        ),
        1 => format!(
            "O trabalho de {d} em {c} começou a ganhar visibilidade além do bloco \
             de costume. A proximidade com a disputa principal deixou de ser eventual \
             e passou a ser parte da narrativa desta temporada."
        ),
        _ => format!(
            "Em {c}, {d} foi ganhando tração ao longo das rodadas e agora figura \
             como variável real na equação do campeonato. Nomes assim são os que \
             mais complicam os favoritos quando a atenção está em outro lugar."
        ),
    }
}

fn compose_pilot_momentum_shift_body(d: &str, c: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "O momento de {d} em {c} virou. O que estava pesado ficou para trás — \
             e o que vem pela frente tem outra cara. Temporadas mudam de eixo em \
             momentos assim, e o campeonato ainda tem rodadas de sobra para essa \
             virada produzir resultados concretos."
        ),
        1 => format!(
            "{d} fechou um ciclo difícil e abriu outro em {c}. A mudança de fase \
             não veio sem custo, mas chegou — e a tabela vai refletir isso nas \
             próximas etapas se o ritmo recente se confirmar."
        ),
        _ => format!(
            "Algo mudou na trajetória de {d} nesta temporada em {c}. O ciclo ruim \
             ficou para trás, e o piloto que aparece agora na pista é diferente do \
             que estava nas últimas rodadas. Transições assim costumam ser decisivas."
        ),
    }
}

// ── Sistema editorial de Piloto — Parte 9: modificadores ─────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum PilotModifier {
    /// Menciona a posição atual no campeonato (P1-P8).
    DriverPositionContext,
    /// Piloto vem de sequência recente de bons resultados.
    RecentGoodRun,
    /// Piloto vem de sequência recente de maus resultados.
    RecentBadRun,
    /// Piloto já está dentro do alcance do topo da tabela.
    TopTableProximity,
    /// Próxima etapa funciona como teste direto — resposta urgente.
    NeedImmediateResponse,
}

/// Limite de pontos de diferença para considerar o piloto "perto do topo".
const TOP_TABLE_GAP_THRESHOLD: i32 = 30;

fn detect_pilot_modifiers(ctx: &PilotStoryContext, trigger: PilotTrigger) -> Vec<PilotModifier> {
    let mut modifiers: Vec<PilotModifier> = Vec::new();

    let has_good_run = ctx.win_streak > 0 || matches!(ctx.last_race_finish, Some(p) if p <= 5);
    let has_bad_run = ctx.last_race_dnf || matches!(ctx.last_race_finish, Some(p) if p > 10);

    // Trigger negativo: foco em resultados ruins e urgência
    if matches!(trigger, PilotTrigger::PilotUnderPressure) {
        if has_bad_run {
            modifiers.push(PilotModifier::RecentBadRun);
        }
        if modifiers.len() < 2 {
            modifiers.push(PilotModifier::NeedImmediateResponse);
        }
        return modifiers;
    }

    // Triggers positivos/neutros — prioridade: posição → proximidade → forma recente

    // P1: DriverPositionContext — posição top-8 conhecida
    if matches!(ctx.driver_position, Some(p) if p <= 8) {
        modifiers.push(PilotModifier::DriverPositionContext);
    }
    if modifiers.len() >= 2 {
        return modifiers;
    }

    // P2: TopTableProximity — dentro de alcance do topo
    let is_near_top = matches!(ctx.driver_position, Some(p) if p <= 5)
        && ctx
            .points_gap_to_leader
            .map_or(false, |gap| gap <= TOP_TABLE_GAP_THRESHOLD);
    if is_near_top {
        modifiers.push(PilotModifier::TopTableProximity);
    }
    if modifiers.len() >= 2 {
        return modifiers;
    }

    // P3: RecentGoodRun
    if has_good_run {
        modifiers.push(PilotModifier::RecentGoodRun);
    }

    modifiers
}

fn compose_pilot_modifier_driver_position(pos: i32, v: usize) -> String {
    let ordinal = format!("{pos}º");
    match v % 3 {
        0 => format!("Hoje aparece em {ordinal} no campeonato."),
        1 => format!("O momento pesa ainda mais porque já corre como {ordinal} colocado."),
        _ => format!("A cobrança cresce porque hoje ele está em {ordinal} na tabela."),
    }
}

fn compose_pilot_modifier_recent_good_run(v: usize) -> String {
    match v % 3 {
        0 => "Ele já vem de uma sequência curta de resultados fortes.".to_string(),
        1 => "As últimas corridas já tinham colocado esse nome em trajetória de alta.".to_string(),
        _ => "O crescimento recente deixou de ser episódio isolado.".to_string(),
    }
}

fn compose_pilot_modifier_recent_bad_run(v: usize) -> String {
    match v % 3 {
        0 => "O momento recente já vinha tirando margem para erro.".to_string(),
        1 => "As últimas rodadas deixaram a cobrança mais visível.".to_string(),
        _ => "A sequência ruim faz a próxima resposta pesar ainda mais.".to_string(),
    }
}

fn compose_pilot_modifier_top_table_proximity(v: usize) -> String {
    match v % 3 {
        0 => "Ele já encostou no bloco principal da tabela.".to_string(),
        1 => "A aproximação ao topo mudou o tamanho da cobrança.".to_string(),
        _ => "O caso ficou maior porque a diferença para os primeiros encolheu.".to_string(),
    }
}

fn compose_pilot_modifier_need_immediate_response(v: usize) -> String {
    match v % 3 {
        0 => "A próxima etapa já funciona como teste direto para esse momento.".to_string(),
        1 => "O que acontece na sequência vai dizer se a fase se sustenta.".to_string(),
        _ => "A resposta agora precisa vir rápido para não perder tração.".to_string(),
    }
}

fn compose_pilot_modifier_phrase(
    modifier: PilotModifier,
    ctx: &PilotStoryContext,
    v: usize,
) -> String {
    match modifier {
        PilotModifier::DriverPositionContext => {
            let pos = ctx.driver_position.unwrap_or(0);
            compose_pilot_modifier_driver_position(pos, v)
        }
        PilotModifier::RecentGoodRun => compose_pilot_modifier_recent_good_run(v),
        PilotModifier::RecentBadRun => compose_pilot_modifier_recent_bad_run(v),
        PilotModifier::TopTableProximity => compose_pilot_modifier_top_table_proximity(v),
        PilotModifier::NeedImmediateResponse => compose_pilot_modifier_need_immediate_response(v),
    }
}

/// Compõe uma story editorial para um NewsItem de Piloto (Hierarquia com driver_id).
/// Retorna None se não houver driver/categoria identificáveis.
fn compose_pilot_headline(trigger: PilotTrigger, ctx: &PilotStoryContext) -> String {
    let d = &ctx.driver_name;
    let v = ctx.item_seed as usize;
    match trigger {
        PilotTrigger::PilotInStrongForm => match v % 2 {
            0 => format!("{d} entra em fase forte e se aproxima da disputa principal"),
            _ => format!("{d} ganha tracao pessoal e sobe de vez na conversa do campeonato"),
        },
        PilotTrigger::PilotUnderPressure => match v % 2 {
            0 => format!("{d} chega pressionado para a proxima etapa"),
            _ => format!("Pressao cresce sobre {d} na sequencia do campeonato"),
        },
        PilotTrigger::PilotBecameRelevant => match v % 2 {
            0 => format!("{d} sai da periferia e entra no radar principal"),
            _ => format!("{d} ganha tracao e muda de patamar na temporada"),
        },
        PilotTrigger::PilotMomentumShift => match v % 2 {
            0 => format!("Virada de fase recoloca {d} no centro da disputa"),
            _ => format!("{d} vira a pagina e abre outra janela no campeonato"),
        },
        PilotTrigger::FallbackPilotStory => String::new(),
    }
}

fn compose_pilot_story(
    item: &NewsItem,
    context: &NewsTabContext,
) -> Result<Option<ComposedRaceStory>, String> {
    let driver_id = match item.driver_id.as_deref() {
        Some(driver_id) => driver_id,
        None => return Ok(None),
    };
    let driver_name = item
        .driver_id
        .as_ref()
        .and_then(|id| context.driver_names.get(id).cloned());
    let Some(driver_name) = driver_name else {
        return Ok(None);
    };

    let category_name = item.categoria_nome.clone().or_else(|| {
        item.categoria_id
            .as_ref()
            .and_then(|id| context.category_names.get(id).cloned())
    });
    let Some(category_name) = category_name else {
        return Ok(None);
    };

    let driver_key = item
        .categoria_id
        .as_ref()
        .zip(item.driver_id.as_ref())
        .map(|(cat, drv)| format!("{cat}:{drv}"));

    let category_id = item.categoria_id.as_deref();
    let editorial_round = item.rodada.filter(|round| *round > 0);
    let historical_standings = match (category_id, editorial_round) {
        (Some(category_id), Some(round)) => {
            historical_standings_after_round(context, item.temporada, category_id, round)?
        }
        _ => None,
    };
    let historical_results = match (category_id, editorial_round) {
        (Some(category_id), Some(round)) => historical_round_results(context, category_id, round)?,
        _ => None,
    };

    let driver_position = historical_standings
        .as_ref()
        .and_then(|standings| {
            standings
                .iter()
                .find(|(candidate_driver_id, _, _)| candidate_driver_id == driver_id)
                .map(|(_, position, _)| *position)
        })
        .or_else(|| {
            driver_key
                .as_deref()
                .and_then(|k| context.driver_positions.get(k).copied())
        });

    let win_streak = match (category_id, editorial_round) {
        (Some(category_id), Some(round)) => historical_win_streak_through_round(
            context,
            driver_id,
            item.temporada,
            category_id,
            round,
        )?,
        _ => None,
    }
    .or_else(|| {
        driver_key
            .as_deref()
            .and_then(|k| context.driver_win_streaks.get(k).copied())
    })
    .unwrap_or(0);

    let last_race_result = historical_results
        .as_ref()
        .and_then(|results| {
            results
                .iter()
                .find(|(candidate_driver_id, _, _, _)| candidate_driver_id == driver_id)
                .map(|(_, grid, finish, dnf)| (*grid, *finish, *dnf))
        })
        .or_else(|| {
            driver_key
                .as_deref()
                .and_then(|k| context.latest_race_results.get(k).copied())
        });
    let last_race_finish = last_race_result.map(|(_, finish, _)| finish);
    let last_race_dnf = last_race_result.map(|(_, _, dnf)| dnf).unwrap_or(false);

    let points_gap_to_leader = historical_standings
        .as_ref()
        .and_then(|standings| {
            let (_, _, leader_points) = standings.first()?;
            let (_, _, driver_points) = standings
                .iter()
                .find(|(candidate_driver_id, _, _)| candidate_driver_id == driver_id)?;
            Some(*leader_points - *driver_points)
        })
        .or_else(|| {
            item.categoria_id.as_ref().and_then(|cat| {
                let standings = context.category_standings_top.get(cat)?;
                let leader_id = standings.first()?;
                if item.driver_id.as_deref() == Some(leader_id.as_str()) {
                    return Some(0);
                }
                let leader_key = format!("{cat}:{leader_id}");
                let leader_pts = context.driver_points.get(&leader_key).copied()?;
                let driver_pts = driver_key
                    .as_deref()
                    .and_then(|k| context.driver_points.get(k).copied())?;
                Some(leader_pts - driver_pts)
            })
        });

    let ctx = PilotStoryContext {
        driver_name,
        category_name,
        driver_position,
        win_streak,
        item_seed: item.timestamp as u64,
        last_race_finish,
        last_race_dnf,
        points_gap_to_leader,
    };

    let trigger = detect_pilot_trigger(&ctx, &item.importancia);
    if trigger == PilotTrigger::FallbackPilotStory {
        return Ok(None);
    }
    let headline = compose_pilot_headline(trigger, &ctx);
    let body = compose_pilot_body(trigger, &ctx);

    Ok(Some(ComposedRaceStory { headline, body }))
}

fn compose_pilot_body(trigger: PilotTrigger, ctx: &PilotStoryContext) -> String {
    let d = &ctx.driver_name;
    let c = &ctx.category_name;
    let v = ctx.item_seed as usize;
    let main_body = match trigger {
        PilotTrigger::PilotInStrongForm => compose_pilot_strong_form_body(d, c, v),
        PilotTrigger::PilotUnderPressure => compose_pilot_under_pressure_body(d, c, v),
        PilotTrigger::PilotBecameRelevant => compose_pilot_became_relevant_body(d, c, v),
        PilotTrigger::PilotMomentumShift => compose_pilot_momentum_shift_body(d, c, v),
        PilotTrigger::FallbackPilotStory => return ctx.driver_name.clone(), // fallback é item.texto
    };
    let modifiers = detect_pilot_modifiers(ctx, trigger);
    if modifiers.is_empty() {
        return main_body;
    }
    let mut parts = vec![main_body];
    for modifier in &modifiers {
        parts.push(compose_pilot_modifier_phrase(*modifier, ctx, v));
    }
    parts.join(" ")
}

// ── Sistema editorial de Mercado — Parte 10 ──────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum MarketTrigger {
    /// Piloto virou pauta central de mercado — interesse crescente, sem confirmação.
    MarketHeatedAroundDriver,
    /// Equipe abriu frente concreta de mercado — movimentação com efeito no paddock.
    MarketHeatedAroundTeam,
    /// Movimento ativo e concreto — saiu da especulação para a fase de definição.
    ConcreteMoveUnderway,
    /// Pré-temporada encurta o espaço entre rumor e decisão.
    PreseasonMarketPressure,
    /// Fallback: contexto insuficiente para trigger específico.
    FallbackMarketStory,
}

struct MarketStoryContext {
    driver_name: Option<String>,
    team_name: Option<String>,
    /// None se a notícia não tem categoria identificável.
    #[allow(dead_code)]
    category_name: Option<String>,
    /// True se o item pertence à pré-temporada.
    is_preseason: bool,
    item_seed: u64,
    // ── Parte 11: campos para modificadores ──────────────────────────────────
    /// Semana da pré-temporada (1, 2, 3…). None fora da pré-temporada.
    preseason_week: Option<i32>,
    /// Tier de presença pública do sujeito principal ("elite", "alta", "relevante", "baixa").
    presence_tier: Option<String>,
    /// True quando o sujeito principal é um piloto (driver_name.is_some()).
    subject_is_driver: bool,
    /// True quando o sujeito principal é uma equipe sem piloto associado.
    subject_is_team: bool,
}

fn detect_market_trigger(ctx: &MarketStoryContext, importancia: &NewsImportance) -> MarketTrigger {
    let is_high = matches!(importancia, NewsImportance::Alta | NewsImportance::Destaque);
    let is_top = matches!(importancia, NewsImportance::Destaque);

    // Pré-temporada com importância relevante → urgência de calendário
    if ctx.is_preseason && is_high {
        return MarketTrigger::PreseasonMarketPressure;
    }

    // Destaque → movimento concreto em andamento
    if is_top {
        return MarketTrigger::ConcreteMoveUnderway;
    }

    // Alta com piloto identificado → mercado aquecido em torno do piloto
    if is_high && ctx.driver_name.is_some() {
        return MarketTrigger::MarketHeatedAroundDriver;
    }

    // Alta com equipe identificada → mercado aquecido em torno da equipe
    if is_high && ctx.team_name.is_some() {
        return MarketTrigger::MarketHeatedAroundTeam;
    }

    MarketTrigger::FallbackMarketStory
}

fn compose_market_driver_heated_body(d: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "{d} entrou de vez no centro da conversa de mercado e já não é tratado \
             como nome lateral para a próxima janela. O interesse ganhou escala porque \
             a combinação entre fase esportiva e momento contratual passou a coincidir."
        ),
        1 => format!(
            "O mercado em torno de {d} ganhou escala e o nome já circula com peso \
             real nas conversas do paddock. O que antes era especulação começou a \
             acumular substância suficiente para mudar o quadro da próxima janela."
        ),
        _ => format!(
            "{d} virou pauta concreta de mercado nesta fase. O interesse cresce porque \
             o momento esportivo e o calendário contratual passaram a coincidir de \
             forma que o paddock não consegue mais ignorar."
        ),
    }
}

fn compose_market_team_heated_body(t: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "{t} abriu uma frente concreta de mercado e passou a puxar parte \
             importante da conversa do paddock nesta fase. O caso ganha força porque \
             a movimentação já começa a produzir expectativa real no restante do grid."
        ),
        1 => format!(
            "O mercado em torno de {t} saiu do rumor e entrou na fase de decisão. \
             O paddock já monitora os movimentos — e o efeito começa a reconfigurar \
             o que o restante do grid estava planejando para a próxima janela."
        ),
        _ => format!(
            "{t} se tornou o centro da movimentação desta janela de mercado. A força \
             do caso cresce porque cada sinal vindo da equipe agora é lido como dado \
             concreto, não como boato."
        ),
    }
}

fn compose_market_concrete_move_body(driver: Option<&str>, team: Option<&str>, v: usize) -> String {
    match driver.or(team) {
        Some(s) => match v % 3 {
            0 => format!(
                "{s} passou da especulação para a fase concreta do processo de \
                 mercado. A indefinição recuou — o que antes circulava como rumor \
                 agora tem substância suficiente para mexer no quadro do grid."
            ),
            1 => format!(
                "O caso de {s} saiu do campo da expectativa e entrou na fase ativa \
                 de negociação. O paddock que antes especulava agora aguarda a \
                 formalização."
            ),
            _ => format!(
                "{s} é mercado real nesta janela. O que circulava como rumor ganhou \
                 substância suficiente para mudar o quadro do grid na próxima temporada."
            ),
        },
        None => match v % 3 {
            0 => "O mercado entrou na fase de definição. O que era expectativa saiu \
                  do campo abstrato e ganhou contornos concretos."
                .to_string(),
            1 => "Um movimento real de mercado está em curso. O espaço para indefinição \
                  diminuiu — e o paddock aguarda o próximo passo."
                .to_string(),
            _ => "O caso saiu da especulação. O que circulava como boato tem agora \
                  substância suficiente para mudar o quadro desta janela."
                .to_string(),
        },
    }
}

fn compose_market_preseason_pressure_body(
    driver: Option<&str>,
    team: Option<&str>,
    v: usize,
) -> String {
    match driver.or(team) {
        Some(s) => match v % 3 {
            0 => format!(
                "O mercado em torno de {s} ganhou urgência na pré-temporada e já \
                 começa a encurtar a distância entre especulação e decisão. A fase do \
                 calendário pesa porque o espaço para definição fica menor a cada semana."
            ),
            1 => format!(
                "{s} já é pauta de mercado com pressão de calendário. A pré-temporada \
                 transforma o que era rumor em prazo — e o paddock começa a operar \
                 como se a definição estivesse próxima."
            ),
            _ => format!(
                "Pré-temporada acende o mercado em torno de {s}. O caso que antes \
                 tinha tempo para se resolver passou a operar sob outra régua — o \
                 espaço para indefinição encolheu."
            ),
        },
        None => match v % 3 {
            0 => "O mercado ganhou urgência na pré-temporada. O que era especulação \
                  passou a operar com pressão de calendário — e o espaço para \
                  indefinição ficou menor a cada semana."
                .to_string(),
            1 => "A pré-temporada transformou o rumor em prazo. O paddock que esperava \
                  mais tempo para definição se viu diante de um processo que acelerou \
                  mais rápido que o esperado."
                .to_string(),
            _ => "O calendário da pré-temporada encurtou o espaço de manobra. O que \
                  podia esperar passou a ter data — e o mercado respondeu com urgência real."
                .to_string(),
        },
    }
}

// ── Sistema editorial de Mercado — Parte 11: modificadores ───────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum MarketModifier {
    /// Fase da pré-temporada (início / meio / fim) amplifica a urgência.
    PreseasonPhaseContext,
    /// Presença pública alta do sujeito amplia o alcance do caso.
    PublicPresenceContext,
    /// Movimento iniciado por uma equipe com capacidade de reconfigurar o grid.
    TeamCenteredMove,
    /// Piloto que ganhou tração de mercado a partir do momento em pista.
    DriverCenteredMove,
    /// Fecho: o que falta para sair do rumor e virar fato.
    NeedsConcreteFollowUp,
}

fn detect_market_modifiers(
    ctx: &MarketStoryContext,
    trigger: MarketTrigger,
) -> Vec<MarketModifier> {
    let mut modifiers: Vec<MarketModifier> = Vec::new();

    // P1: PreseasonPhaseContext — timing pesa, mas não quando o trigger já é de pré-temporada
    if ctx.is_preseason && trigger != MarketTrigger::PreseasonMarketPressure {
        modifiers.push(MarketModifier::PreseasonPhaseContext);
    }
    if modifiers.len() >= 2 {
        return modifiers;
    }

    // P2: Sujeito principal — piloto ou equipe (mutuamente exclusivos; sem redundância com trigger)
    if ctx.subject_is_driver && trigger != MarketTrigger::MarketHeatedAroundDriver {
        modifiers.push(MarketModifier::DriverCenteredMove);
    } else if ctx.subject_is_team && trigger != MarketTrigger::MarketHeatedAroundTeam {
        modifiers.push(MarketModifier::TeamCenteredMove);
    }
    if modifiers.len() >= 2 {
        return modifiers;
    }

    // P3: PublicPresenceContext — visibilidade alta amplifica o caso
    if matches!(ctx.presence_tier.as_deref(), Some("elite") | Some("alta")) {
        modifiers.push(MarketModifier::PublicPresenceContext);
    }
    if modifiers.len() >= 2 {
        return modifiers;
    }

    // P4: NeedsConcreteFollowUp — fecho útil que quase sempre cabe
    modifiers.push(MarketModifier::NeedsConcreteFollowUp);
    modifiers
}

fn compose_market_modifier_preseason_phase(week: Option<i32>, v: usize) -> String {
    let is_late = week.map_or(false, |w| w >= 5);
    let is_mid = week.map_or(false, |w| w >= 3 && w < 5);
    if is_late {
        match v % 3 {
            0 => "A fase da pré-temporada reduz a margem para esse movimento seguir só no rumor.",
            1 => "O espaço para indefinição encolheu — a pré-temporada entra na fase de encerramento.",
            _ => "O timing pesa porque o calendário já deixa pouco espaço para o caso seguir sem definição.",
        }
    } else if is_mid {
        match v % 3 {
            0 => "O timing pesa porque a pré-temporada já entrou em fase de definição.",
            1 => "O caso ganha urgência porque a janela de decisão começa a encurtar.",
            _ => "A fase da pré-temporada muda o peso do rumor — o espaço para manobra já diminuiu.",
        }
    } else {
        match v % 3 {
            0 => "O caso ganha urgência por acontecer quando a janela de decisão começa a se abrir.",
            1 => "O início da pré-temporada coloca o mercado num ritmo que tende a acelerar nas semanas seguintes.",
            _ => "O calendário entra num trecho em que o rumor começa a se transformar em processo.",
        }
    }
    .to_string()
}

fn compose_market_modifier_driver_centered(v: usize) -> String {
    match v % 3 {
        0 => "A fase recente ajuda a explicar por que esse nome ganhou tração fora da pista.",
        1 => "O interesse cresce porque o piloto já deixou de ser aposta lateral nesta temporada.",
        _ => "O caso fica mais forte porque desempenho e mercado passaram a correr juntos.",
    }
    .to_string()
}

fn compose_market_modifier_team_centered(v: usize) -> String {
    match v % 3 {
        0 => "O movimento ficou maior porque parte de uma equipe que pode alterar mais de uma frente do grid.",
        1 => "A equipe entra no centro da pauta num momento em que o resto do mercado já começa a reagir.",
        _ => "O caso pesa mais porque a iniciativa parte de uma estrutura com capacidade real de mexer no desenho do grid.",
    }
    .to_string()
}

fn compose_market_modifier_public_presence(v: usize) -> String {
    match v % 3 {
        0 => "O peso do caso cresce porque envolve um nome de forte circulação no paddock.",
        1 => "A visibilidade do nome amplia o tamanho da conversa no grid.",
        _ => "O assunto passa a valer mais porque já circula com força além do resultado em pista.",
    }
    .to_string()
}

fn compose_market_modifier_needs_followup(v: usize) -> String {
    match v % 3 {
        0 => "A pauta só avança de verdade quando algum dos envolvidos parar de testar terreno e começar a se mover.",
        1 => "Sem gesto concreto de um dos lados, a conversa continua grande demais para virar definição.",
        _ => "A pauta só muda de patamar quando aparecer resposta observável dos envolvidos.",
    }
    .to_string()
}

fn compose_market_modifier_phrase(
    modifier: MarketModifier,
    ctx: &MarketStoryContext,
    v: usize,
) -> String {
    match modifier {
        MarketModifier::PreseasonPhaseContext => {
            compose_market_modifier_preseason_phase(ctx.preseason_week, v)
        }
        MarketModifier::DriverCenteredMove => compose_market_modifier_driver_centered(v),
        MarketModifier::TeamCenteredMove => compose_market_modifier_team_centered(v),
        MarketModifier::PublicPresenceContext => compose_market_modifier_public_presence(v),
        MarketModifier::NeedsConcreteFollowUp => compose_market_modifier_needs_followup(v),
    }
}

fn compose_market_body(trigger: MarketTrigger, ctx: &MarketStoryContext) -> String {
    let v = ctx.item_seed as usize;
    let d = ctx.driver_name.as_deref();
    let t = ctx.team_name.as_deref();
    let main_body = match trigger {
        MarketTrigger::MarketHeatedAroundDriver => {
            compose_market_driver_heated_body(d.unwrap_or(""), v)
        }
        MarketTrigger::MarketHeatedAroundTeam => {
            compose_market_team_heated_body(t.unwrap_or(""), v)
        }
        MarketTrigger::ConcreteMoveUnderway => compose_market_concrete_move_body(d, t, v),
        MarketTrigger::PreseasonMarketPressure => compose_market_preseason_pressure_body(d, t, v),
        MarketTrigger::FallbackMarketStory => return String::new(), // nunca chamado
    };
    let modifiers = detect_market_modifiers(ctx, trigger);
    if modifiers.is_empty() {
        return main_body;
    }
    let mut parts = vec![main_body];
    for modifier in &modifiers {
        parts.push(compose_market_modifier_phrase(*modifier, ctx, v));
    }
    parts.join(" ")
}

/// Compõe uma story editorial para um NewsItem de Mercado.
/// Retorna None se o trigger for Fallback ou o contexto for insuficiente.
fn compose_market_headline(trigger: MarketTrigger, ctx: &MarketStoryContext) -> String {
    let v = ctx.item_seed as usize;
    let d = ctx.driver_name.as_deref();
    let t = ctx.team_name.as_deref();
    match trigger {
        MarketTrigger::MarketHeatedAroundDriver => match v % 2 {
            0 => format!(
                "{} esquenta o mercado da proxima janela",
                d.unwrap_or("Nome forte")
            ),
            _ => format!(
                "Nome de {} ganha forca real no mercado",
                d.unwrap_or("piloto")
            ),
        },
        MarketTrigger::MarketHeatedAroundTeam => match v % 2 {
            0 => format!(
                "{} puxa a pauta de mercado do paddock",
                t.unwrap_or("Equipe")
            ),
            _ => format!(
                "{} entra no centro da proxima janela",
                t.unwrap_or("Equipe")
            ),
        },
        MarketTrigger::ConcreteMoveUnderway => match d.or(t) {
            Some(subject) => match v % 2 {
                0 => format!("Movimento por {subject} entra na fase concreta"),
                _ => format!("Caso de {subject} deixa o rumor e vira processo real"),
            },
            None => match v % 2 {
                0 => "Mercado entra na fase concreta da janela".to_string(),
                _ => "Rumor perde espaco e da lugar a processo real".to_string(),
            },
        },
        MarketTrigger::PreseasonMarketPressure => match d.or(t) {
            Some(subject) => match v % 2 {
                0 => format!("Pre-temporada acelera definicao em torno de {subject}"),
                _ => format!("{subject} entra na janela sob pressao de calendario"),
            },
            None => match v % 2 {
                0 => "Pre-temporada acelera o ritmo do mercado".to_string(),
                _ => "Calendario encurta o espaco para definicoes do grid".to_string(),
            },
        },
        MarketTrigger::FallbackMarketStory => String::new(),
    }
}

fn compose_market_story(item: &NewsItem, context: &NewsTabContext) -> Option<ComposedRaceStory> {
    let driver_name = item
        .driver_id
        .as_ref()
        .and_then(|id| context.driver_names.get(id).cloned());
    let team_name = item
        .team_id
        .as_ref()
        .and_then(|id| context.team_names.get(id).cloned());
    let category_name = item.categoria_nome.clone().or_else(|| {
        item.categoria_id
            .as_ref()
            .and_then(|id| context.category_names.get(id).cloned())
    });
    let is_preseason = item.semana_pretemporada.is_some();
    let subject_is_driver = driver_name.is_some();
    let subject_is_team = team_name.is_some() && driver_name.is_none();

    // Presence tier: piloto vem de driver_media, equipe de team_public_presence
    let presence_tier = item
        .driver_id
        .as_ref()
        .and_then(|id| context.driver_media.get(id).copied())
        .map(|score| {
            if score >= 0.8 {
                "elite"
            } else if score >= 0.6 {
                "alta"
            } else {
                "baixa"
            }
            .to_string()
        })
        .or_else(|| {
            item.team_id
                .as_ref()
                .and_then(|id| context.team_public_presence.get(id).cloned())
        });

    let ctx = MarketStoryContext {
        driver_name,
        team_name,
        category_name,
        is_preseason,
        item_seed: item.timestamp as u64,
        preseason_week: item.semana_pretemporada,
        presence_tier,
        subject_is_driver,
        subject_is_team,
    };

    let trigger = detect_market_trigger(&ctx, &item.importancia);
    if trigger == MarketTrigger::FallbackMarketStory {
        return None;
    }

    let headline = compose_market_headline(trigger, &ctx);
    let body = compose_market_body(trigger, &ctx);
    Some(ComposedRaceStory { headline, body })
}

// ── Sistema editorial de Incidente — Parte 12 ────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum IncidentTrigger {
    /// Dois pilotos identificados envolvidos — o caso nasce pesado por natureza.
    TwoDriverIncident,
    /// Dano direto num piloto — toque, batida, prejuízo claro de pontos.
    DriverIncidentDamage,
    /// Problema mecânico com impacto forte — sem rival, dano isolado.
    MechanicalFailureHitStrongly,
    /// Caso ainda em aberto depois da prova — algo segue pendente.
    IncidentStillOpen,
    /// Fallback: contexto insuficiente para trigger específico.
    FallbackIncidentStory,
}

/// Tipo de incidente na camada editorial, desacoplado do motor de simulacao.
#[derive(Debug, Clone, Copy, PartialEq)]
enum IncidentFactType {
    Mechanical,
    DriverError,
    Collision,
}

#[derive(Clone)]
/// Fatos catalogados do ultimo resultado, usados para enriquecer a leitura editorial.
pub(crate) struct IncidentEditorialFacts {
    incident_type: Option<IncidentFactType>,
    is_dnf: bool,
    segment: Option<String>,
}

struct IncidentStoryContext {
    driver_name: Option<String>,
    secondary_driver_name: Option<String>,
    #[allow(dead_code)]
    category_name: Option<String>,
    /// True quando não há outro piloto envolvido (falha mecânica, acidente isolado).
    is_mechanical: bool,
    /// True quando o caso segue pendente depois da prova.
    is_still_open: bool,
    is_dnf: bool,
    segment: Option<String>,
    item_seed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IncidentModifier {
    IncidentCausedDnf,
    LateRaceHit,
    MidRaceHit,
}

fn detect_incident_trigger(
    ctx: &IncidentStoryContext,
    importancia: &NewsImportance,
) -> IncidentTrigger {
    let is_high = matches!(importancia, NewsImportance::Alta | NewsImportance::Destaque);

    // Dois pilotos identificados → caso com dois nomes
    if ctx.secondary_driver_name.is_some() && is_high {
        return IncidentTrigger::TwoDriverIncident;
    }

    // Mecânico/isolado → sem rival
    if ctx.is_mechanical && is_high {
        return IncidentTrigger::MechanicalFailureHitStrongly;
    }

    // Caso ainda aberto → peso que não fechou com a etapa
    if ctx.is_still_open && is_high {
        return IncidentTrigger::IncidentStillOpen;
    }

    // Piloto com dano direto
    if ctx.driver_name.is_some() && is_high {
        return IncidentTrigger::DriverIncidentDamage;
    }

    IncidentTrigger::FallbackIncidentStory
}

#[allow(dead_code)]
fn compose_incident_driver_damage_body(d: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "{d} saiu da etapa com o prejuízo mais pesado da rodada depois de um \
             incidente que tirou pontos e margem esportiva num momento sensível do \
             campeonato."
        ),
        1 => format!(
            "O incidente deixou {d} com saldo negativo claro: pontos perdidos, ritmo \
             interrompido e pouca margem para absorver o resultado sem que o campeonato sinta."
        ),
        _ => format!(
            "A etapa saiu do controle para {d} depois de um incidente que transformou \
             uma chance real em dano aberto para a sequência."
        ),
    }
}

#[allow(dead_code)]
fn compose_incident_two_driver_body(d: &str, s: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "O toque entre {d} e {s} empurrou os dois para o centro da pauta da \
             semana e deixou a próxima etapa sob pressão mais direta do que o normal."
        ),
        1 => format!(
            "{d} e {s} saíram do incidente com relações esportivas mais tensas do \
             que entraram. O caso pesa porque ambos têm motivos concretos para querer \
             resolver o placar nas próximas rodadas."
        ),
        _ => format!(
            "O que aconteceu entre {d} e {s} na pista vai além do resultado imediato. \
             O campeonato absorveu mais uma camada de tensão entre dois nomes que já \
             dividiam espaço apertado na tabela."
        ),
    }
}

#[allow(dead_code)]
fn compose_incident_mechanical_body(d: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "A quebra tirou de {d} uma chance real de sustentar o momento e \
             transformou a rodada em dano aberto para a sequência."
        ),
        1 => format!(
            "O problema chegou no momento errado para {d} e converteu o que podia \
             ser etapa positiva em furo no campeonato."
        ),
        _ => format!(
            "{d} saiu sem pontos depois de uma falha que não dependeu de disputa — \
             o que torna o prejuízo mais difícil de processar para a sequência da temporada."
        ),
    }
}

fn compose_incident_still_open_body(d: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "O caso que envolve {d} não se fechou com o fim da etapa. O que ficou \
             pendente vai pesar na leitura da próxima rodada — e o paddock espera \
             uma resposta observável antes de seguir."
        ),
        1 => format!(
            "O incidente que marcou a etapa de {d} ainda não tem encerramento claro. \
             A abertura do caso muda a leitura do que está em jogo nas rodadas seguintes."
        ),
        _ => format!(
            "{d} terminou a semana com um incidente ainda não resolvido. O peso do \
             que está aberto pode se tornar fator real dependendo de como o caso se \
             desenvolver nas próximas etapas."
        ),
    }
}

fn compose_incident_driver_damage_body_polished(d: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "{d} saiu da etapa com pontos perdidos e a corrida quebrada por um \
             incidente que tirou margem de reacao num momento sensivel do campeonato."
        ),
        1 => format!(
            "O incidente empurrou {d} para uma rodada de dano concreto: perdeu pontos, \
             perdeu ritmo e saiu sem uma volta final capaz de reorganizar a prova."
        ),
        _ => format!(
            "Depois do incidente, {d} passou a correr apenas para limitar perdas e saiu \
             da etapa com uma resposta obrigatoria ja na proxima rodada."
        ),
    }
}

fn compose_incident_two_driver_body_polished(d: &str, s: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "O toque entre {d} e {s} colocou os dois no foco da semana e deixou a \
             proxima etapa carregada de tensao competitiva."
        ),
        1 => format!(
            "{d} e {s} sairam do incidente com a disputa aberta de vez. O campeonato \
             agora leva para as proximas rodadas um confronto que ganhou peso esportivo claro."
        ),
        _ => format!(
            "O contato entre {d} e {s} nao parou na bandeira quadriculada. A tabela e a \
             proxima etapa ficaram mais tensas porque os dois agora carregam um caso direto entre si."
        ),
    }
}

fn compose_incident_mechanical_body_polished(d: &str, v: usize) -> String {
    match v % 3 {
        0 => format!(
            "A quebra tirou de {d} uma corrida que ainda estava viva e \
             transformou a rodada em dano imediato para a sequencia."
        ),
        1 => format!(
            "O problema chegou no momento errado para {d} e converteu uma etapa que \
             prometia pontos em rombo direto no campeonato."
        ),
        _ => format!(
            "{d} saiu sem pontos depois de uma falha que nao dependeu de disputa, o tipo \
             de golpe que desmonta a rodada e cobra recuperacao ja na sequencia da temporada."
        ),
    }
}

fn detect_incident_modifiers(
    ctx: &IncidentStoryContext,
    trigger: IncidentTrigger,
) -> Vec<IncidentModifier> {
    if matches!(
        trigger,
        IncidentTrigger::FallbackIncidentStory | IncidentTrigger::IncidentStillOpen
    ) {
        return Vec::new();
    }

    let mut modifiers = Vec::new();

    if ctx.is_dnf {
        modifiers.push(IncidentModifier::IncidentCausedDnf);
    }

    let segment = ctx.segment.as_deref().map(|s| s.to_ascii_lowercase());
    match segment.as_deref() {
        Some("late") | Some("finish") | Some("final") => {
            modifiers.push(IncidentModifier::LateRaceHit);
        }
        Some("mid") | Some("middle") => {
            modifiers.push(IncidentModifier::MidRaceHit);
        }
        _ => {}
    }

    modifiers.truncate(2);
    modifiers
}

fn compose_incident_modifier_phrase_polished(
    modifier: IncidentModifier,
    trigger: IncidentTrigger,
    _ctx: &IncidentStoryContext,
    v: usize,
) -> String {
    match modifier {
        IncidentModifier::IncidentCausedDnf => {
            if trigger == IncidentTrigger::TwoDriverIncident {
                match v % 3 {
                    0 => "Quando o toque termina em abandono, o caso deixa de ser so tensao entre dois nomes e passa a pesar diretamente na tabela.".to_string(),
                    1 => "O abandono de um dos lados tirou o episodio do campo do contato isolado e levou a disputa para dano esportivo real.".to_string(),
                    _ => "O desfecho com abandono ampliou o tamanho do caso porque transformou o contato em prejuizo concreto, nao so em atrito de pista.".to_string(),
                }
            } else {
                match v % 3 {
                    0 => "Sair sem ver a bandeira quadriculada transforma o caso em dano direto de campeonato.".to_string(),
                    1 => "Com abandono, o prejuizo deixa de ser so posicional e vira perda total de pontos na rodada.".to_string(),
                    _ => "O abandono fecha a etapa sem espaco para resposta e transforma o incidente em prejuizo esportivo completo.".to_string(),
                }
            }
        }
        IncidentModifier::LateRaceHit => match v % 3 {
            0 => "O problema apareceu tarde demais para qualquer resposta esportiva dentro da propria corrida.".to_string(),
            1 => "O dano veio no trecho final, quando ja nao havia espaco real para salvar a etapa.".to_string(),
            _ => "Sofrer o problema no fim da prova fechou qualquer rota real de recuperacao dentro da etapa.".to_string(),
        },
        IncidentModifier::MidRaceHit => match v % 3 {
            0 => "O episodio no meio da prova desmontou a corrida antes que ela entrasse na fase decisiva.".to_string(),
            1 => "Ainda havia corrida pela frente, mas o incidente empurrou a etapa para modo de sobrevivencia bem antes do fim.".to_string(),
            _ => "O impacto no trecho intermediario quebrou o rumo da prova e obrigou a rodada a ser administrada no prejuizo.".to_string(),
        },
    }
}

#[allow(dead_code)]
fn compose_incident_modifier_phrase(
    modifier: IncidentModifier,
    trigger: IncidentTrigger,
    _ctx: &IncidentStoryContext,
    v: usize,
) -> String {
    match modifier {
        IncidentModifier::IncidentCausedDnf => {
            if trigger == IncidentTrigger::TwoDriverIncident {
                match v % 3 {
                    0 => "Quando o toque termina em abandono, o caso deixa de ser so tensao entre dois nomes e passa a pesar diretamente na tabela.".to_string(),
                    1 => "O abandono de um dos lados tirou o caso do campo do contato isolado e empurrou o episodio para dano esportivo real.".to_string(),
                    _ => "O desfecho com abandono aumenta o tamanho do caso porque transforma o contato em prejuizo concreto, nao so em atrito de pista.".to_string(),
                }
            } else {
                match v % 3 {
                    0 => "Sair sem ver a bandeira quadriculada transforma o caso em dano direto de campeonato.".to_string(),
                    1 => "Com abandono, o prejuizo deixa de ser so posicional e vira perda total de margem na rodada.".to_string(),
                    _ => "O abandono fecha a conta da etapa da forma mais pesada possivel, sem espaco para limitar o dano no fim.".to_string(),
                }
            }
        }
        IncidentModifier::LateRaceHit => match v % 3 {
            0 => "O problema apareceu tarde demais para qualquer resposta esportiva dentro da propria corrida.".to_string(),
            1 => "O dano veio no trecho final, quando ja nao havia espaco real para salvar a etapa.".to_string(),
            _ => "Sofrer o golpe no fim da prova torna a rodada ainda mais dificil de absorver.".to_string(),
        },
        IncidentModifier::MidRaceHit => match v % 3 {
            0 => "O episodio no meio da prova desmontou a corrida antes que ela chegasse ao momento decisivo.".to_string(),
            1 => "O dano ainda deixou alguma corrida pela frente, mas ja com a etapa claramente comprometida.".to_string(),
            _ => "O impacto veio no meio da prova e quebrou o ritmo antes de qualquer tentativa de recuperacao.".to_string(),
        },
    }
}

fn compose_incident_body(trigger: IncidentTrigger, ctx: &IncidentStoryContext) -> String {
    let v = ctx.item_seed as usize;
    let d = ctx.driver_name.as_deref().unwrap_or("o piloto");
    let s = ctx.secondary_driver_name.as_deref().unwrap_or("o rival");
    let main_body = match trigger {
        IncidentTrigger::DriverIncidentDamage => compose_incident_driver_damage_body_polished(d, v),
        IncidentTrigger::TwoDriverIncident => compose_incident_two_driver_body_polished(d, s, v),
        IncidentTrigger::MechanicalFailureHitStrongly => {
            compose_incident_mechanical_body_polished(d, v)
        }
        IncidentTrigger::IncidentStillOpen => compose_incident_still_open_body(d, v),
        IncidentTrigger::FallbackIncidentStory => String::new(), // nunca chamado
    };
    let modifiers = detect_incident_modifiers(ctx, trigger);
    if modifiers.is_empty() {
        return main_body;
    }

    let mut parts = vec![main_body];
    for modifier in modifiers {
        parts.push(compose_incident_modifier_phrase_polished(
            modifier, trigger, ctx, v,
        ));
    }
    parts.join(" ")
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TeamTrigger {
    TeamInStrongMoment,
    TeamUnderPressure,
    TeamBecameRelevant,
    TeamLostGround,
    FallbackTeamStory,
}

struct TeamStoryContext {
    team_name: String,
    category_name: Option<String>,
    team_position: Option<i32>,
    team_points: Option<i32>,
    presence_tier: Option<String>,
    next_race_label: Option<String>,
    item_seed: u64,
}

fn detect_team_trigger(ctx: &TeamStoryContext, importancia: &NewsImportance) -> TeamTrigger {
    let is_high = matches!(importancia, NewsImportance::Alta | NewsImportance::Destaque);
    let is_top = matches!(ctx.team_position, Some(p) if p <= 3);
    let is_mid = matches!(ctx.team_position, Some(p) if (4..=8).contains(&p));
    let has_high_presence = matches!(ctx.presence_tier.as_deref(), Some("elite") | Some("alta"));

    if is_high && is_top {
        return TeamTrigger::TeamInStrongMoment;
    }
    if is_high && is_mid {
        return TeamTrigger::TeamBecameRelevant;
    }
    if !is_high && is_top {
        return TeamTrigger::TeamUnderPressure;
    }
    if !is_high && is_mid && has_high_presence {
        return TeamTrigger::TeamLostGround;
    }

    TeamTrigger::FallbackTeamStory
}

fn team_scope_suffix(ctx: &TeamStoryContext) -> String {
    ctx.category_name
        .as_deref()
        .map(|c| format!(" em {c}"))
        .unwrap_or_default()
}

fn compose_team_strong_moment_body(ctx: &TeamStoryContext, v: usize) -> String {
    let t = &ctx.team_name;
    let scope = team_scope_suffix(ctx);
    let _ = ctx.team_points;
    match v % 3 {
        0 => format!(
            "{t} chega a este trecho{scope} no melhor momento coletivo da temporada. \
             O conjunto entregou consistencia suficiente para a equipe deixar de aparecer \
             em flashes e virar bloco com presenca real na frente."
        ),
        1 => format!(
            "Ha mais tracao coletiva em {t}{scope}. A equipe deixou de aparecer so em \
             flashes e passou a sustentar influencia real na parte alta da tabela."
        ),
        _ => format!(
            "{t} ganhou corpo como estrutura competitiva{scope}. O que era presenca \
             eventual virou forca mais constante na frente, e isso mudou o tamanho da \
             equipe no campeonato."
        ),
    }
}

fn compose_team_under_pressure_body(ctx: &TeamStoryContext, v: usize) -> String {
    let t = &ctx.team_name;
    let scope = team_scope_suffix(ctx);
    match v % 3 {
        0 => format!(
            "{t} chega a este trecho{scope} sem conseguir transformar estrutura em \
             resultado consistente. O que funcionava na teoria passou a cobrar fatura \
             na pratica — e o campeonato nao tem paciencia para ciclos longos de ajuste."
        ),
        1 => format!(
            "{t} perdeu a folga que tinha no inicio deste trecho{scope} e agora corre \
             pressionada a provar que a estrutura ainda consegue responder no nivel esperado."
        ),
        _ => format!(
            "A fase recente colocou {t} sob um tipo diferente de cobranca{scope}. A \
             equipe ainda tem base para reagir, mas a margem encolheu e a resposta ja \
             ficou urgente."
        ),
    }
}

fn compose_team_became_relevant_body(ctx: &TeamStoryContext, v: usize) -> String {
    let t = &ctx.team_name;
    let scope = team_scope_suffix(ctx);
    match v % 3 {
        0 => format!(
            "O crescimento coletivo de {t}{scope} saiu do campo da pontualidade e \
             virou presenca mais constante na frente. A equipe passou a aparecer \
             onde importa — e o paddock comecou a notar."
        ),
        1 => format!(
            "{t}{scope} nao chegou ao bloco da frente por acidente. A estrutura \
             encontrou cadencia, a pontuacao ficou consistente — e o paddock comecou \
             a apontar a equipe como nome a levar a serio."
        ),
        _ => format!(
            "Ha algo diferente no funcionamento de {t}{scope} neste trecho. O conjunto \
             passou a entregar mais do que prometia — e isso comecou a aparecer \
             na tabela de forma que nao da mais para ignorar."
        ),
    }
}

fn compose_team_lost_ground_body(ctx: &TeamStoryContext, v: usize) -> String {
    let t = &ctx.team_name;
    let scope = team_scope_suffix(ctx);
    match v % 3 {
        0 => format!(
            "{t} cedeu espaco{scope} justamente quando parecia ganhar tracao. A rodada \
             interrompeu a sequencia que a equipe vinha construindo e reabriu a discussao \
             sobre onde ela realmente se encaixa no grid."
        ),
        1 => format!(
            "{t} perdeu terreno{scope} num momento em que parecia pronta para se firmar \
             mais acima. O recuo nao e definitivo, mas deixa a equipe em posicao mais \
             fragil do que estava ha poucas rodadas."
        ),
        _ => format!(
            "{t} deixou a rodada com menos forca esportiva do que vinha sugerindo{scope} \
             e agora aparece mais distante do bloco que realmente dita o ritmo da categoria."
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TeamModifier {
    TopTableContext,
    NeedImmediateResponse,
    GrowingPublicWeight,
}

fn detect_team_modifiers(ctx: &TeamStoryContext, trigger: TeamTrigger) -> Vec<TeamModifier> {
    let mut modifiers: Vec<TeamModifier> = Vec::new();

    if matches!(trigger, TeamTrigger::TeamInStrongMoment)
        && matches!(ctx.team_position, Some(p) if p <= 3)
    {
        modifiers.push(TeamModifier::TopTableContext);
    }
    if modifiers.len() >= 2 {
        return modifiers;
    }

    if matches!(trigger, TeamTrigger::TeamUnderPressure) {
        modifiers.push(TeamModifier::NeedImmediateResponse);
    }
    if modifiers.len() >= 2 {
        return modifiers;
    }

    if matches!(
        trigger,
        TeamTrigger::TeamInStrongMoment | TeamTrigger::TeamBecameRelevant
    ) && matches!(ctx.presence_tier.as_deref(), Some("elite") | Some("alta"))
    {
        modifiers.push(TeamModifier::GrowingPublicWeight);
    }

    modifiers.truncate(2);
    modifiers
}

fn compose_team_modifier_top_table(v: usize) -> String {
    match v % 3 {
        0 => "Hoje a estrutura ja aparece entre as equipes que realmente moldam a parte alta da tabela.".to_string(),
        1 => "A equipe ja entrou no grupo que dita boa parte do ritmo na zona mais alta do campeonato.".to_string(),
        _ => "Nao e mais uma estrutura de passagem na frente: o time ja pesa de verdade na parte alta da tabela.".to_string(),
    }
}

fn compose_team_modifier_need_immediate_response(
    next_race_label: Option<&str>,
    v: usize,
) -> String {
    match next_race_label {
        Some(race) => match v % 3 {
            0 => format!(
                "A ida para {race} agora funciona como teste direto para saber se a equipe ainda sustenta o lugar que tentava construir."
            ),
            1 => format!(
                "{race} virou a referencia imediata para medir se a estrutura ainda consegue recolocar a fase nos trilhos."
            ),
            _ => format!(
                "O que vier em {race} ja vai dizer se a equipe ainda segura o tamanho da ambicao que vinha desenhando."
            ),
        },
        None => match v % 3 {
            0 => "A sequencia agora funciona como teste direto para saber se a equipe ainda sustenta o lugar que tentava construir.".to_string(),
            1 => "A proxima etapa virou a medida mais clara para saber se a estrutura ainda consegue recolocar a fase nos trilhos.".to_string(),
            _ => "O que vier na sequencia ja vai dizer se a equipe ainda segura o tamanho da ambicao que vinha desenhando.".to_string(),
        },
    }
}

fn compose_team_modifier_public_weight(v: usize) -> String {
    match v % 3 {
        0 => "A fase esportiva ficou mais visivel porque a estrutura passou a ocupar outro espaco no campeonato.".to_string(),
        1 => "A fase ganha escala porque o nome da equipe ja corre com mais forca fora da pista.".to_string(),
        _ => "O crescimento esportivo ficou mais visivel porque a estrutura tambem ganhou outro tamanho na conversa do paddock.".to_string(),
    }
}

fn compose_team_modifier_phrase(
    modifier: TeamModifier,
    ctx: &TeamStoryContext,
    v: usize,
) -> String {
    match modifier {
        TeamModifier::TopTableContext => compose_team_modifier_top_table(v),
        TeamModifier::NeedImmediateResponse => {
            compose_team_modifier_need_immediate_response(ctx.next_race_label.as_deref(), v)
        }
        TeamModifier::GrowingPublicWeight => compose_team_modifier_public_weight(v),
    }
}

fn compose_team_body(trigger: TeamTrigger, ctx: &TeamStoryContext) -> String {
    let v = ctx.item_seed as usize;
    let main_body = match trigger {
        TeamTrigger::TeamInStrongMoment => compose_team_strong_moment_body(ctx, v),
        TeamTrigger::TeamUnderPressure => compose_team_under_pressure_body(ctx, v),
        TeamTrigger::TeamBecameRelevant => compose_team_became_relevant_body(ctx, v),
        TeamTrigger::TeamLostGround => compose_team_lost_ground_body(ctx, v),
        TeamTrigger::FallbackTeamStory => return String::new(),
    };

    let modifiers = detect_team_modifiers(ctx, trigger);
    if modifiers.is_empty() {
        return main_body;
    }

    let mut parts = vec![main_body];
    for modifier in &modifiers {
        parts.push(compose_team_modifier_phrase(*modifier, ctx, v));
    }
    parts.join(" ")
}

fn compose_team_headline(trigger: TeamTrigger, ctx: &TeamStoryContext) -> String {
    let t = &ctx.team_name;
    let v = ctx.item_seed as usize;
    match trigger {
        TeamTrigger::TeamInStrongMoment => match v % 2 {
            0 => format!("{t} consolida forca coletiva e entra no bloco da frente"),
            _ => format!("{t} ganha corpo como estrutura e sobe de patamar na categoria"),
        },
        TeamTrigger::TeamUnderPressure => match v % 2 {
            0 => format!("{t} entra pressionada na proxima etapa"),
            _ => format!("Cobranca sobe sobre {t} neste trecho do campeonato"),
        },
        TeamTrigger::TeamBecameRelevant => match v % 2 {
            0 => format!("{t} começa a converter pontos com regularidade e sobe no grid"),
            _ => format!("{t} encontra cadencia coletiva e entra no bloco de frente"),
        },
        TeamTrigger::TeamLostGround => match v % 2 {
            0 => format!("{t} perde terreno e fecha um ciclo ruim no campeonato"),
            _ => format!("{t} deixa a rodada com menos forca no campeonato"),
        },
        TeamTrigger::FallbackTeamStory => String::new(),
    }
}

fn compose_team_story(item: &NewsItem, context: &NewsTabContext) -> Option<ComposedRaceStory> {
    let team_name = item
        .team_id
        .as_ref()
        .and_then(|id| context.team_names.get(id).cloned())?;

    let category_name = item.categoria_nome.clone().or_else(|| {
        item.categoria_id
            .as_ref()
            .and_then(|id| context.category_names.get(id).cloned())
    });

    let ctx = TeamStoryContext {
        team_name,
        category_name,
        team_position: item
            .team_id
            .as_ref()
            .and_then(|id| context.team_positions.get(id).copied()),
        team_points: item
            .team_id
            .as_ref()
            .and_then(|id| context.team_points.get(id).copied()),
        presence_tier: item
            .team_id
            .as_ref()
            .and_then(|id| context.team_public_presence.get(id).cloned()),
        next_race_label: item
            .categoria_id
            .as_ref()
            .and_then(|cat| context.next_race_by_category.get(cat))
            .map(|r| r.label.clone()),
        item_seed: item.timestamp as u64,
    };

    let trigger = detect_team_trigger(&ctx, &item.importancia);
    if trigger == TeamTrigger::FallbackTeamStory {
        return None;
    }

    Some(ComposedRaceStory {
        headline: compose_team_headline(trigger, &ctx),
        body: compose_team_body(trigger, &ctx),
    })
}

/// Heurística textual: detecta incidente mecânico/isolado a partir do título e corpo da notícia.
/// Só considera mecânico quando não há piloto secundário envolvido.
#[derive(Debug, Clone, Copy, PartialEq)]
enum InjuryTrigger {
    DriverRuledOutByInjury,
    InjuryStatusStillUncertain,
    DriverReturnsFromInjury,
    FallbackInjuryStory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InjuryModifier {
    TopDriverContext,
    NextRacePressure,
    ReturnChangesGridReading,
}

struct InjuryStoryContext {
    driver_name: String,
    category_name: Option<String>,
    driver_position: Option<i32>,
    next_race_label: Option<String>,
    item_seed: u64,
    is_ruled_out: bool,
    is_returning: bool,
    is_uncertain: bool,
}

fn detect_injury_flags(titulo: &str, texto: &str) -> (bool, bool, bool) {
    let combined = format!("{} {}", titulo.to_lowercase(), texto.to_lowercase());
    let is_ruled_out = [
        "desfalque",
        "vetado",
        "fora da proxima etapa",
        "fora da próxima etapa",
        "nao corre",
        "não corre",
        "nao participa",
        "não participa",
        "esta fora",
        "está fora",
    ]
    .iter()
    .any(|kw| combined.contains(kw));
    let is_returning = [
        "retorna",
        "retorno",
        "volta ao grid",
        "de volta",
        "retorna ao grid",
        "volta a correr",
    ]
    .iter()
    .any(|kw| combined.contains(kw));
    let is_uncertain = [
        "duvida",
        "dúvida",
        "segue em aberto",
        "situacao em aberto",
        "situação em aberto",
        "sem definicao",
        "sem definição",
        "incerto",
        "incerta",
        "ainda nao ha definicao",
        "ainda não há definição",
    ]
    .iter()
    .any(|kw| combined.contains(kw));
    (is_ruled_out, is_returning, is_uncertain)
}

fn detect_injury_trigger(ctx: &InjuryStoryContext) -> InjuryTrigger {
    if ctx.is_ruled_out {
        return InjuryTrigger::DriverRuledOutByInjury;
    }
    if ctx.is_returning {
        return InjuryTrigger::DriverReturnsFromInjury;
    }
    if ctx.is_uncertain {
        return InjuryTrigger::InjuryStatusStillUncertain;
    }
    InjuryTrigger::FallbackInjuryStory
}

fn detect_injury_modifiers(
    ctx: &InjuryStoryContext,
    trigger: InjuryTrigger,
) -> Vec<InjuryModifier> {
    if trigger == InjuryTrigger::FallbackInjuryStory {
        return Vec::new();
    }

    let mut modifiers = Vec::new();

    if matches!(ctx.driver_position, Some(p) if p <= 3) {
        modifiers.push(InjuryModifier::TopDriverContext);
    }
    if modifiers.len() >= 2 {
        return modifiers;
    }

    if trigger == InjuryTrigger::DriverRuledOutByInjury && ctx.next_race_label.is_some() {
        modifiers.push(InjuryModifier::NextRacePressure);
    }
    if modifiers.len() >= 2 {
        return modifiers;
    }

    if trigger == InjuryTrigger::DriverReturnsFromInjury {
        modifiers.push(InjuryModifier::ReturnChangesGridReading);
    }

    modifiers.truncate(2);
    modifiers
}

fn injury_scope_suffix(ctx: &InjuryStoryContext) -> String {
    ctx.category_name
        .as_deref()
        .map(|c| format!(" em {c}"))
        .unwrap_or_default()
}

fn injury_ruled_out_target(ctx: &InjuryStoryContext) -> String {
    ctx.next_race_label
        .as_deref()
        .map(|r| format!("da etapa de {r}"))
        .unwrap_or_else(|| "da proxima etapa".to_string())
}

fn injury_return_arrival_target(ctx: &InjuryStoryContext) -> String {
    ctx.next_race_label
        .as_deref()
        .map(|r| format!("para a etapa de {r}"))
        .unwrap_or_else(|| "para a proxima etapa".to_string())
}

fn injury_return_event_target(ctx: &InjuryStoryContext) -> String {
    ctx.next_race_label
        .as_deref()
        .map(|r| format!("da etapa de {r}"))
        .unwrap_or_else(|| "da proxima etapa".to_string())
}

fn injury_uncertain_target(ctx: &InjuryStoryContext) -> String {
    ctx.next_race_label
        .as_deref()
        .map(|r| format!("para a etapa de {r}"))
        .unwrap_or_else(|| "para a proxima etapa".to_string())
}

fn injury_absence_weight_clause(ctx: &InjuryStoryContext) -> String {
    match ctx.driver_position {
        Some(p) if p <= 3 => {
            " A ausencia pesa ainda mais porque tira da disputa um nome ainda instalado na parte mais alta da tabela.".to_string()
        }
        Some(p) if p <= 8 => {
            " A ausencia pesa porque interrompe a sequencia de um piloto ainda perto do bloco principal da categoria.".to_string()
        }
        _ => {
            " A ausencia pesa porque chega num ponto em que o campeonato ja nao oferecia muita margem para interrupcoes.".to_string()
        }
    }
}

fn injury_return_weight_clause(ctx: &InjuryStoryContext) -> String {
    match ctx.driver_position {
        Some(p) if p <= 3 => {
            " A volta recoloca no grid um nome que ainda tem peso direto na parte mais alta da tabela.".to_string()
        }
        Some(p) if p <= 8 => {
            " O retorno devolve a categoria um nome ainda presente na conversa principal da tabela.".to_string()
        }
        _ => {
            " O retorno muda a leitura da etapa porque recoloca esse nome no fluxo competitivo do fim de semana.".to_string()
        }
    }
}

fn compose_injury_headline(trigger: InjuryTrigger, ctx: &InjuryStoryContext) -> String {
    let d = &ctx.driver_name;
    let v = ctx.item_seed as usize;
    match trigger {
        InjuryTrigger::DriverRuledOutByInjury => match v % 3 {
            0 => format!("{d} vira desfalque e aumenta a pressao sobre a proxima etapa"),
            1 => format!("Lesao tira {d} da sequencia imediata do campeonato"),
            _ => format!("{d} fica fora e muda a conta esportiva da proxima rodada"),
        },
        InjuryTrigger::InjuryStatusStillUncertain => match v % 3 {
            0 => format!("Situacao fisica de {d} mantem proxima etapa em aberto"),
            1 => format!("{d} chega sob duvida para a sequencia do campeonato"),
            _ => format!("Disponibilidade de {d} segue sem definicao para a rodada"),
        },
        InjuryTrigger::DriverReturnsFromInjury => match v % 3 {
            0 => format!("{d} retorna ao grid e recoloca um nome forte na proxima etapa"),
            1 => format!("Volta de {d} muda a leitura da rodada"),
            _ => format!("{d} volta a ficar disponivel e reabre o quadro esportivo da etapa"),
        },
        InjuryTrigger::FallbackInjuryStory => String::new(),
    }
}

fn compose_injury_modifier_phrase(
    modifier: InjuryModifier,
    trigger: InjuryTrigger,
    ctx: &InjuryStoryContext,
    v: usize,
) -> String {
    match modifier {
        InjuryModifier::TopDriverContext => match trigger {
            InjuryTrigger::DriverRuledOutByInjury => match v % 3 {
                0 => "O impacto cresce porque se trata de um nome que ja corria no bloco principal da categoria.".to_string(),
                1 => "O peso da ausencia aumenta porque a lesao tira da disputa um dos nomes mais fortes da tabela neste momento.".to_string(),
                _ => "Fica maior porque a baixa atinge um piloto ainda instalado entre os nomes que puxavam a frente do campeonato.".to_string(),
            },
            InjuryTrigger::InjuryStatusStillUncertain => match v % 3 {
                0 => "O caso pesa ainda mais porque a duvida envolve um dos nomes mais fortes da tabela neste momento.".to_string(),
                1 => "A incerteza ganha outro tamanho porque paira sobre um piloto que ainda ocupava o bloco principal da categoria.".to_string(),
                _ => "Nao e uma duvida lateral do grid: a espera envolve um nome que ainda carregava peso real na parte alta da tabela.".to_string(),
            },
            InjuryTrigger::DriverReturnsFromInjury => match v % 3 {
                0 => "A volta recoloca em circulacao um nome que ainda carregava peso direto na frente da categoria.".to_string(),
                1 => "O retorno ganha tamanho extra porque devolve ao grid um dos pilotos mais fortes da tabela neste momento.".to_string(),
                _ => "Nao volta apenas um nome conhecido: retorna um piloto que ainda tinha peso claro no bloco principal do campeonato.".to_string(),
            },
            InjuryTrigger::FallbackInjuryStory => String::new(),
        },
        InjuryModifier::NextRacePressure => match ctx.next_race_label.as_deref() {
            Some(race) => match v % 3 {
                0 => "A proxima etapa agora recebe menos um candidato e mais um vazio relevante no grid.".to_string(),
                1 => format!("A ausencia muda a leitura imediata de {race}, que perde um nome com peso real antes mesmo de a rodada comecar."),
                _ => format!("Para {race}, o efeito mais direto e um grid mais curto justamente onde a categoria esperava um nome de frente."),
            },
            None => match v % 3 {
                0 => "A proxima etapa agora passa a receber menos um candidato e mais um vazio relevante no grid.".to_string(),
                1 => "A ausencia muda a leitura imediata da rodada seguinte, que perde um nome com peso real antes mesmo de comecar.".to_string(),
                _ => "O efeito mais direto ja cai sobre a etapa seguinte, que fica sem um nome esperado no bloco principal do grid.".to_string(),
            },
        },
        InjuryModifier::ReturnChangesGridReading => match v % 3 {
            0 => "A presenca por si so ja recompoe o grid antes de qualquer confirmacao sobre o ritmo que ele pode sustentar de imediato.".to_string(),
            1 => "So por estar de volta, o grid ja ganha outra configuracao esportiva antes mesmo de se saber quanto ritmo competitivo ele traz de imediato.".to_string(),
            _ => "A volta ja altera a conta da rodada no papel, porque recoloca no grid um nome que muda a distribuicao de forcas antes mesmo do primeiro stint.".to_string(),
        },
    }
}

fn compose_injury_body(trigger: InjuryTrigger, ctx: &InjuryStoryContext) -> String {
    let d = &ctx.driver_name;
    let scope = injury_scope_suffix(ctx);
    let v = ctx.item_seed as usize;
    let main_body = match trigger {
        InjuryTrigger::DriverRuledOutByInjury => {
            let target = injury_ruled_out_target(ctx);
            match v % 3 {
                0 => format!(
                    "{d} esta fora {target} e deixa um vazio esportivo que vai alem da troca de nome no grid.{}",
                    injury_absence_weight_clause(ctx)
                ),
                1 => format!(
                    "A lesao tira {d} {target} e muda a leitura da rodada antes mesmo de a disputa de ritmo comecar.{}",
                    injury_absence_weight_clause(ctx)
                ),
                _ => format!(
                    "{d} vira desfalque {target}{scope}, e a pauta deixa de ser desempenho para virar disponibilidade competitiva.{}",
                    injury_absence_weight_clause(ctx)
                ),
            }
        }
        InjuryTrigger::InjuryStatusStillUncertain => {
            let target = injury_uncertain_target(ctx);
            match v % 3 {
                0 => format!(
                    "A situacao fisica de {d} segue em aberto{scope} e transforma a espera {target} num problema de disponibilidade antes mesmo de virar disputa de ritmo."
                ),
                1 => format!(
                    "{d} chega sob duvida {target}, e isso empurra a categoria para uma espera pouco esportiva: primeiro pela confirmacao, depois pelo que ela vai significar na pista."
                ),
                _ => format!(
                    "Ainda nao ha clareza sobre a presenca de {d}{scope} {target}. Antes de pensar em ritmo, a categoria precisa saber se esse nome estara disponivel."
                ),
            }
        }
        InjuryTrigger::DriverReturnsFromInjury => {
            let arrival_target = injury_return_arrival_target(ctx);
            let event_target = injury_return_event_target(ctx);
            match v % 3 {
                0 => format!(
                    "{d} retorna ao grid depois do periodo fora {arrival_target} e recoloca um nome relevante na composicao esportiva da rodada.{}",
                    injury_return_weight_clause(ctx)
                ),
                1 => format!(
                    "A volta de {d}{scope} muda a leitura competitiva {event_target}. O foco agora deixa de ser apenas a recuperacao e passa a ser o quanto desse nome reaparece na pista."
                ),
                _ => format!(
                    "{d} volta a ficar disponivel {arrival_target} e devolve a categoria um nome que faz diferenca na composicao do grid.{}",
                    injury_return_weight_clause(ctx)
                ),
            }
        }
        InjuryTrigger::FallbackInjuryStory => String::new(),
    };

    let modifiers = detect_injury_modifiers(ctx, trigger);
    if modifiers.is_empty() {
        return main_body;
    }

    let mut parts = vec![main_body];
    for modifier in modifiers {
        parts.push(compose_injury_modifier_phrase(modifier, trigger, ctx, v));
    }
    parts.join(" ")
}

fn compose_injury_story(
    item: &NewsItem,
    context: &NewsTabContext,
) -> Result<Option<ComposedRaceStory>, String> {
    let driver_name = item
        .driver_id
        .as_ref()
        .and_then(|id| context.driver_names.get(id).cloned());
    let Some(driver_name) = driver_name else {
        return Ok(None);
    };
    let category_name = item.categoria_nome.clone().or_else(|| {
        item.categoria_id
            .as_ref()
            .and_then(|id| context.category_names.get(id).cloned())
    });
    let driver_position = match (
        item.categoria_id.as_deref(),
        item.driver_id.as_deref(),
        item.rodada,
    ) {
        (Some(category_id), Some(driver_id), Some(round)) if round >= 1 => {
            historical_driver_position_after_round(
                context,
                item.temporada,
                category_id,
                driver_id,
                round,
            )?
            .or_else(|| {
                context
                    .driver_positions
                    .get(&format!("{category_id}:{driver_id}"))
                    .copied()
            })
        }
        (Some(category_id), Some(driver_id), _) => context
            .driver_positions
            .get(&format!("{category_id}:{driver_id}"))
            .copied(),
        _ => None,
    };
    let next_race_label = match (item.categoria_id.as_deref(), item.rodada) {
        (Some(category_id), Some(round)) if round >= 1 => context
            .race_labels
            .get(&format!("{category_id}:{}", round + 1))
            .cloned()
            .or_else(|| {
                context
                    .next_race_by_category
                    .get(category_id)
                    .map(|r| r.label.clone())
            }),
        (Some(category_id), _) => context
            .next_race_by_category
            .get(category_id)
            .map(|r| r.label.clone()),
        _ => None,
    };
    let (is_ruled_out, is_returning, is_uncertain) = detect_injury_flags(&item.titulo, &item.texto);

    let ctx = InjuryStoryContext {
        driver_name,
        category_name,
        driver_position,
        next_race_label,
        item_seed: item.timestamp as u64,
        is_ruled_out,
        is_returning,
        is_uncertain,
    };

    let trigger = detect_injury_trigger(&ctx);
    if trigger == InjuryTrigger::FallbackInjuryStory {
        return Ok(None);
    }

    Ok(Some(ComposedRaceStory {
        headline: compose_injury_headline(trigger, &ctx),
        body: compose_injury_body(trigger, &ctx),
    }))
}

fn is_mechanical_incident(titulo: &str, texto: &str, has_secondary: bool) -> bool {
    if has_secondary {
        return false;
    }
    let combined = format!("{} {}", titulo.to_lowercase(), texto.to_lowercase());
    [
        "quebra",
        "quebrou",
        "quebrado",
        "falha mec",
        "problema mec",
        "pane",
        "motor",
    ]
    .iter()
    .any(|kw| combined.contains(kw))
}

fn is_dnf_incident_text(titulo: &str, texto: &str) -> bool {
    let combined = format!("{} {}", titulo.to_lowercase(), texto.to_lowercase());
    [
        "abandona",
        "abandono",
        "sem ver a bandeira quadriculada",
        "fora da corrida",
        "encerrou a prova",
    ]
    .iter()
    .any(|kw| combined.contains(kw))
}

fn compose_incident_headline(trigger: IncidentTrigger, ctx: &IncidentStoryContext) -> String {
    let d = ctx.driver_name.as_deref().unwrap_or("Piloto");
    let s = ctx.secondary_driver_name.as_deref().unwrap_or("rival");
    let v = ctx.item_seed as usize;
    match trigger {
        IncidentTrigger::DriverIncidentDamage => match v % 3 {
            0 => format!("{d} sai com prejuízo pesado depois de incidente na etapa"),
            1 => format!("Incidente custa pontos e margem a {d} num momento sensível"),
            _ => format!("{d} absorve dano direto e perde terreno na temporada"),
        },
        IncidentTrigger::TwoDriverIncident => match v % 3 {
            0 => format!("Toque entre {d} e {s} vira pauta da semana no campeonato"),
            1 => format!("{d} e {s} saem com placar aberto para as próximas etapas"),
            _ => format!("Incidente entre {d} e {s} deixa tensão na sequência do campeonato"),
        },
        IncidentTrigger::MechanicalFailureHitStrongly => match v % 3 {
            0 => format!("Quebra interrompe rodada de {d} e abre dano no campeonato"),
            1 => format!("{d} perde etapa para falha mecânica no pior momento"),
            _ => format!("Problema mecânico transforma etapa de {d} em prejuízo puro"),
        },
        IncidentTrigger::IncidentStillOpen => match v % 3 {
            0 => format!("Caso de {d} segue em aberto depois do fim da etapa"),
            1 => format!("Incidente de {d} não se fecha com o resultado — paddock aguarda"),
            _ => format!("{d} termina semana com caso ainda sem desfecho claro"),
        },
        IncidentTrigger::FallbackIncidentStory => String::new(),
    }
}

/// Compõe uma story editorial para um NewsItem de Incidente.
/// Retorna None se o trigger for Fallback ou o contexto for insuficiente.
fn compose_incident_story(
    item: &NewsItem,
    context: &NewsTabContext,
) -> Result<Option<ComposedRaceStory>, String> {
    let driver_name = item
        .driver_id
        .as_ref()
        .and_then(|id| context.driver_names.get(id).cloned());
    let secondary_driver_name = item
        .driver_id_secondary
        .as_ref()
        .and_then(|id| context.driver_names.get(id).cloned());
    let category_name = item.categoria_nome.clone().or_else(|| {
        item.categoria_id
            .as_ref()
            .and_then(|id| context.category_names.get(id).cloned())
    });
    let completed_round = context.career.season.rodada_atual.saturating_sub(1);
    let facts = match (
        item.categoria_id.as_deref(),
        item.driver_id.as_deref(),
        item.rodada,
    ) {
        (Some(category_id), Some(driver_id), Some(round))
            if round >= 1 && round != completed_round =>
        {
            historical_incident_facts_for_round(context, category_id, driver_id, round)?
        }
        (Some(category_id), Some(driver_id), _) => context
            .latest_incident_facts
            .get(&format!("{category_id}:{driver_id}"))
            .cloned(),
        _ => None,
    };
    let is_mechanical = match facts.as_ref().and_then(|f| f.incident_type) {
        Some(IncidentFactType::Mechanical) => true,
        Some(_) => false,
        None => is_mechanical_incident(
            &item.titulo,
            &item.texto,
            item.driver_id_secondary.is_some(),
        ),
    };

    let ctx = IncidentStoryContext {
        driver_name,
        secondary_driver_name,
        category_name,
        is_mechanical,
        is_still_open: false,
        is_dnf: facts
            .as_ref()
            .map(|f| f.is_dnf)
            .unwrap_or_else(|| is_dnf_incident_text(&item.titulo, &item.texto)),
        segment: facts.as_ref().and_then(|f| f.segment.clone()),
        item_seed: item.timestamp as u64,
    };

    let trigger = detect_incident_trigger(&ctx, &item.importancia);
    if trigger == IncidentTrigger::FallbackIncidentStory {
        return Ok(None);
    }

    let headline = compose_incident_headline(trigger, &ctx);
    let body = compose_incident_body(trigger, &ctx);
    Ok(Some(ComposedRaceStory { headline, body }))
}

fn build_hero(
    context: &NewsTabContext,
    scope_type: &str,
    scope_id: &str,
    scope_label: &str,
    _scope_class: Option<&str>,
    primary_filter: Option<&str>,
) -> NewsTabHero {
    let next_race = if scope_type == "category" {
        next_race_for_category(context, scope_id)
    } else {
        None
    };
    let badge = if primary_filter.is_none() && scope_type == "famous" {
        "Briefing Publico".to_string()
    } else if primary_filter.is_none() {
        next_race
            .as_ref()
            .map(|(track_name, round)| format!("Briefing R{} | {}", round, track_name))
            .unwrap_or_else(|| "Briefing da Rodada".to_string())
    } else if primary_filter == Some("Mercado") {
        "Mercado Aquecido".to_string()
    } else if scope_type == "famous" {
        "Em Alta".to_string()
    } else if context.career.season.rodada_atual <= 1 {
        "Inicio de Temporada".to_string()
    } else if context.career.season.rodada_atual >= context.career.season.total_rodadas {
        "Reta Final".to_string()
    } else {
        format!(
            "Rodada {}/{}",
            context.career.season.rodada_atual, context.career.season.total_rodadas
        )
    };

    let subtitle = match (scope_type, primary_filter) {
        ("famous", Some("Pilotos")) => {
            "Os nomes mais quentes do paddock neste momento.".to_string()
        }
        ("famous", Some("Equipes")) => {
            "As estruturas com maior presenca publica do campeonato.".to_string()
        }
        ("famous", Some("Mercado")) => {
            "Quem domina a conversa publica nas movimentacoes do grid.".to_string()
        }
        ("famous", _) => {
            "Quem domina a conversa agora e leva atencao para o proximo momento do campeonato."
                .to_string()
        }
        (_, Some("Corridas")) => {
            format!("Etapas e acontecimentos que estao moldando {scope_label}.")
        }
        (_, Some("Mercado")) => format!("Rumores e movimentos que agitam {scope_label}."),
        (_, Some("Equipes")) => {
            format!("O paddock institucional e esportivo que sustenta {scope_label}.")
        }
        (_, Some("Pilotos")) => format!("Os protagonistas e as tensoes centrais de {scope_label}."),
        _ => {
            if let Some((track_name, _)) = next_race {
                format!(
                    "O que segue vivo no paddock antes de {} e o que pesa para a proxima largada em {scope_label}.",
                    track_name
                )
            } else {
                format!("O que segue no ar antes da proxima rodada em {scope_label}.")
            }
        }
    };

    NewsTabHero {
        section_label: "Central de Notícias".to_string(),
        title: "Panorama do Campeonato".to_string(),
        subtitle,
        badge,
        badge_tone: if scope_type == "famous" {
            "gold".to_string()
        } else {
            "blue".to_string()
        },
    }
}

fn normalize_scope_type(value: &str) -> &str {
    if value.eq_ignore_ascii_case("famous") {
        "famous"
    } else {
        "category"
    }
}

fn normalize_scope_id(scope_type: &str, requested: &str, fallback_category: &str) -> String {
    if scope_type == "famous" {
        "mais_famosos".to_string()
    } else if categories::get_category_config(requested).is_some() {
        requested.to_string()
    } else {
        fallback_category.to_string()
    }
}

fn normalize_scope_class(scope_id: &str, requested: Option<&str>) -> Option<String> {
    let requested = requested.map(str::trim).filter(|value| !value.is_empty())?;
    let category = categories::get_category_config(scope_id)?;
    category
        .classes
        .iter()
        .find(|class_info| class_info.class_name.eq_ignore_ascii_case(requested))
        .map(|class_info| class_info.class_name.to_string())
}

fn normalize_primary_filter(scope_type: &str, requested: Option<&str>) -> Option<String> {
    let requested = requested.map(str::trim).filter(|value| !value.is_empty())?;
    let allowed = if scope_type == "famous" {
        &FAMOUS_FILTER_IDS[..]
    } else {
        &PRIMARY_FILTER_IDS[..]
    };
    allowed
        .iter()
        .find(|value| value.eq_ignore_ascii_case(requested))
        .map(|value| (*value).to_string())
}

fn story_belongs_to_category(
    item: &NewsItem,
    category_id: &str,
    scope_class: Option<&str>,
    driver_ids: &HashSet<String>,
    team_ids: &HashSet<String>,
) -> bool {
    let matches_scope_members = item_mentions_any_driver(item, driver_ids)
        || item
            .team_id
            .as_ref()
            .map(|value| team_ids.contains(value))
            .unwrap_or(false);

    if scope_class.is_some() {
        matches_scope_members
    } else {
        item.categoria_id.as_deref() == Some(category_id) || matches_scope_members
    }
}

fn story_is_famous(
    item: &NewsItem,
    famous_driver_ids: &HashSet<String>,
    famous_team_ids: &HashSet<String>,
) -> bool {
    item_mentions_any_driver(item, famous_driver_ids)
        || item
            .team_id
            .as_ref()
            .map(|value| famous_team_ids.contains(value))
            .unwrap_or(false)
        || matches!(
            item.importancia,
            NewsImportance::Alta | NewsImportance::Destaque
        )
}

fn story_matches_primary_filter(
    _context: &NewsTabContext,
    _scope_id: &str,
    item: &NewsItem,
    primary_filter: Option<&str>,
) -> bool {
    match primary_filter {
        Some("Mercado") => matches!(item.tipo, NewsType::Mercado | NewsType::PreTemporada),
        Some("Corridas") => item.rodada.unwrap_or(0) > 0,
        Some("Equipes") => {
            item.team_id.is_some()
                || item.driver_id.is_some()
                || item.driver_id_secondary.is_some()
                || matches!(item.tipo, NewsType::Hierarquia)
        }
        Some("Pilotos") => {
            item.driver_id.is_some()
                || item.driver_id_secondary.is_some()
                || matches!(item.tipo, NewsType::Rivalidade | NewsType::Incidente)
        }
        _ => true,
    }
}

fn item_mentions_driver(item: &NewsItem, driver_id: &str) -> bool {
    item.driver_id.as_deref() == Some(driver_id)
        || item.driver_id_secondary.as_deref() == Some(driver_id)
}

fn item_mentions_any_driver(item: &NewsItem, driver_ids: &HashSet<String>) -> bool {
    item.driver_id
        .as_ref()
        .map(|value| driver_ids.contains(value))
        .unwrap_or(false)
        || item
            .driver_id_secondary
            .as_ref()
            .map(|value| driver_ids.contains(value))
            .unwrap_or(false)
}

fn category_briefing_score(
    context: &NewsTabContext,
    scope_id: &str,
    class_ids: Option<&(HashSet<String>, HashSet<String>)>,
    item: &NewsItem,
) -> Option<i32> {
    let mut score = match item.tipo {
        NewsType::FramingSazonal => 220,
        NewsType::PreTemporada => 190,
        NewsType::Rivalidade => 170,
        NewsType::Incidente => 160,
        NewsType::Corrida => 135,
        NewsType::Hierarquia => 115,
        NewsType::Mercado => 70,
        _ if matches!(
            item.importancia,
            NewsImportance::Alta | NewsImportance::Destaque
        ) =>
        {
            85
        }
        _ => 0,
    };

    if score == 0 {
        return None;
    }

    if item.categoria_id.as_deref() == Some(scope_id) {
        score += 18;
    }

    if let Some((team_ids, driver_ids)) = class_ids {
        if item_mentions_any_driver(item, driver_ids)
            || item
                .team_id
                .as_ref()
                .map(|team_id| team_ids.contains(team_id))
                .unwrap_or(false)
        {
            score += 16;
        }
    }

    let current_round = context.career.season.rodada_atual.max(1);
    let previous_round = current_round.saturating_sub(1);
    if item.rodada == Some(current_round) {
        score += 28;
    } else if previous_round > 0 && item.rodada == Some(previous_round) {
        score += 22;
    } else if item.tipo == NewsType::Corrida {
        score -= 26;
    }

    if matches!(item.tipo, NewsType::Mercado) && item.rodada.unwrap_or(0) <= 0 {
        score -= 24;
    }

    score += importance_rank(&item.importancia) * 9;
    score += freshness_bonus(context.newest_timestamp, item.timestamp);
    Some(score)
}

fn public_briefing_score(context: &NewsTabContext, item: &NewsItem) -> Option<i32> {
    let mut score = match item.tipo {
        NewsType::FramingSazonal => 170,
        NewsType::Rivalidade => 155,
        NewsType::Mercado => 145,
        NewsType::Corrida => 125,
        _ if matches!(
            item.importancia,
            NewsImportance::Alta | NewsImportance::Destaque
        ) =>
        {
            110
        }
        _ => 0,
    };

    if score == 0 {
        return None;
    }

    score += importance_rank(&item.importancia) * 8;
    score += freshness_bonus(context.newest_timestamp, item.timestamp);
    Some(score)
}

fn next_race_for_category(context: &NewsTabContext, category_id: &str) -> Option<(String, i32)> {
    get_calendar_for_category_in_base_dir(&context.base_dir, &context.career_id, category_id)
        .ok()?
        .into_iter()
        .find(|race| race.rodada >= context.career.season.rodada_atual)
        .map(|race| (race.track_name, race.rodada))
}

fn story_matches_context(
    context: &NewsTabContext,
    item: &NewsItem,
    selection: &ContextSelection,
) -> bool {
    match selection.kind.as_str() {
        "race" => context
            .race_rounds
            .get(&selection.id)
            .map(|round| item.rodada == Some(*round))
            .unwrap_or(false),
        "team" => {
            item.team_id.as_deref() == Some(selection.id.as_str())
                || context
                    .team_driver_ids
                    .get(&selection.id)
                    .map(|ids| {
                        ids.iter()
                            .any(|driver_id| item_mentions_driver(item, driver_id))
                    })
                    .unwrap_or(false)
        }
        "driver" => item_mentions_driver(item, selection.id.as_str()),
        "rivalry" => {
            let ids: Vec<&str> = selection.id.split('|').collect();
            ids.iter()
                .any(|driver_id| item_mentions_driver(item, driver_id))
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::Duration;

    use super::{get_news_tab_bootstrap_in_base_dir, get_news_tab_snapshot_in_base_dir};
    use crate::commands::career::{
        create_career_in_base_dir, get_calendar_for_category_in_base_dir,
    };
    use crate::commands::career_types::{CreateCareerInput, NewsTabSnapshotRequest};
    use crate::commands::news_helpers::{
        format_display_date_label, format_naive_date_label, parse_iso_date,
    };
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::news as news_queries;
    use crate::db::queries::seasons as season_queries;
    use crate::db::queries::teams as team_queries;
    use crate::news::{NewsImportance, NewsItem, NewsType};
    use serde_json::Value;

    #[test]
    fn test_news_tab_bootstrap_defaults_to_player_category_and_includes_famous_scope() {
        let base_dir = create_test_career_dir("news_bootstrap");
        let bootstrap =
            get_news_tab_bootstrap_in_base_dir(&base_dir, "career_001").expect("bootstrap");

        assert_eq!(bootstrap.default_scope_type, "category");
        assert_eq!(bootstrap.default_scope_id, "mazda_rookie");
        assert_eq!(
            bootstrap.default_primary_filter.as_deref(),
            Some("Corridas")
        );
        assert!(bootstrap
            .scopes
            .iter()
            .any(|scope| scope.id == "mazda_rookie"));
        assert!(bootstrap
            .scopes
            .iter()
            .any(|scope| scope.id == "mais_famosos" && scope.special));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_snapshot_for_race_focus_returns_round_chips_and_filtered_stories() {
        let base_dir = create_test_career_dir("news_snapshot_race_focus");
        seed_news_items(&base_dir, "career_001");

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "mazda_rookie".to_string(),
                scope_class: None,
                primary_filter: Some("Corridas".to_string()),
                context_type: Some("race".to_string()),
                context_id: Some("R001".to_string()),
            },
        )
        .expect("snapshot");
        let snapshot_json = serde_json::to_value(&snapshot).expect("snapshot json");

        assert!(snapshot
            .contextual_filters
            .iter()
            .any(|chip| chip.id == "R001"));
        let stories = snapshot_json
            .get("stories")
            .and_then(|value| value.as_array())
            .expect("stories array");
        assert!(stories
            .iter()
            .all(|story| story.get("round") == Some(&Value::from(1))));
        assert_eq!(
            snapshot.scope_meta.primary_filter.as_deref(),
            Some("Corridas")
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_snapshot_market_builds_entity_context_filters_from_market_stories() {
        let base_dir = create_test_career_dir("news_snapshot_market_context");
        let team_id = current_team_id(&base_dir, "career_001");
        seed_news_items(&base_dir, "career_001");

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "mazda_rookie".to_string(),
                scope_class: None,
                primary_filter: Some("Mercado".to_string()),
                context_type: None,
                context_id: None,
            },
        )
        .expect("snapshot");
        let snapshot_json = serde_json::to_value(&snapshot).expect("snapshot json");

        assert_eq!(snapshot.scope_meta.context_type, None);
        assert_eq!(snapshot.scope_meta.context_id, None);
        assert!(snapshot
            .contextual_filters
            .iter()
            .any(|chip| chip.id == team_id));
        let stories = snapshot_json
            .get("stories")
            .and_then(|value| value.as_array())
            .expect("stories array");
        assert!(stories.len() >= 2);
        assert!(stories.iter().all(|story| {
            matches!(
                story.get("news_type").and_then(|value| value.as_str()),
                Some("Mercado" | "PreTemporada")
            )
        }));
        let featured_story = stories
            .iter()
            .find(|story| {
                story.get("original_text")
                    == Some(&Value::from(
                        "O paddock comenta uma movimentacao de mercado ao redor da equipe.",
                    ))
            })
            .expect("featured market story");
        assert_ne!(
            featured_story.get("title"),
            Some(&Value::from("A equipe do jogador observa reforcos")),
            "story de mercado deve expor headline editorial, nao herdar item.titulo"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_snapshot_market_requires_matching_context_type_and_id() {
        let base_dir = create_test_career_dir("news_snapshot_market_type_guard");
        let team_id = current_team_id(&base_dir, "career_001");
        seed_news_items(&base_dir, "career_001");

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "mazda_rookie".to_string(),
                scope_class: None,
                primary_filter: Some("Mercado".to_string()),
                context_type: Some("driver".to_string()),
                context_id: Some(team_id),
            },
        )
        .expect("snapshot");

        assert_eq!(snapshot.scope_meta.context_type, None);
        assert_eq!(snapshot.scope_meta.context_id, None);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_snapshot_driver_context_matches_secondary_driver_in_rivalry_story() {
        let base_dir = create_test_career_dir("news_snapshot_driver_secondary");
        seed_news_items(&base_dir, "career_001");

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "mazda_rookie".to_string(),
                scope_class: None,
                primary_filter: Some("Pilotos".to_string()),
                context_type: Some("driver".to_string()),
                context_id: Some("P002".to_string()),
            },
        )
        .expect("snapshot");

        assert!(snapshot.stories.iter().any(|story| {
            story
                .title
                .contains("Thomas Baker e Kenji Sato entram em rota de colisao")
        }));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_snapshot_rivalry_context_requires_both_drivers_in_story() {
        let base_dir = create_test_career_dir("news_snapshot_rivalry_strict");
        seed_news_items(&base_dir, "career_001");

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "mazda_rookie".to_string(),
                scope_class: None,
                primary_filter: Some("Pilotos".to_string()),
                context_type: Some("rivalry".to_string()),
                context_id: Some("P001|P002".to_string()),
            },
        )
        .expect("snapshot");

        assert!(snapshot.stories.iter().any(|story| {
            story
                .title
                .contains("Thomas Baker e Kenji Sato entram em rota de colisao")
        }));
        assert!(!snapshot.stories.iter().any(|story| {
            story
                .title
                .contains("Thomas Baker abandona a corrida apos quebra")
        }));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_bootstrap_returns_error_when_player_calendar_is_invalid() {
        let base_dir = create_test_career_dir("news_bootstrap_invalid_calendar");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        db.conn
            .execute(
                "UPDATE calendar SET status = 'status_quebrado' WHERE categoria = 'mazda_rookie'",
                [],
            )
            .expect("corrupt calendar status");

        let err = get_news_tab_bootstrap_in_base_dir(&base_dir, "career_001")
            .expect_err("bootstrap should fail");
        assert!(
            err.contains("RaceStatus inválido"),
            "erro deve denunciar calendario invalido: {err}"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_snapshot_returns_error_when_historical_race_queries_fail() {
        let base_dir = create_test_career_dir("news_snapshot_history_broken");
        seed_news_items(&base_dir, "career_001");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        db.conn
            .execute_batch("DROP TABLE race_results;")
            .expect("drop race_results");

        let err = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "mazda_rookie".to_string(),
                scope_class: None,
                primary_filter: Some("Corridas".to_string()),
                context_type: Some("race".to_string()),
                context_id: Some("R001".to_string()),
            },
        )
        .expect_err("snapshot should fail");
        assert!(
            err.contains("race_results") || err.contains("historic"),
            "erro deve denunciar historico indisponivel: {err}"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_snapshot_for_famous_scope_uses_reduced_filters() {
        let base_dir = create_test_career_dir("news_snapshot_famous");
        seed_news_items(&base_dir, "career_001");

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "famous".to_string(),
                scope_id: "mais_famosos".to_string(),
                scope_class: None,
                primary_filter: Some("Pilotos".to_string()),
                context_type: None,
                context_id: None,
            },
        )
        .expect("snapshot");

        let filter_ids: Vec<&str> = snapshot
            .primary_filters
            .iter()
            .map(|filter| filter.id.as_str())
            .collect();
        assert_eq!(filter_ids, vec!["Pilotos", "Equipes", "Mercado"]);
        assert!(snapshot.scope_meta.is_special);
        assert!(!snapshot.contextual_filters.is_empty());

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_snapshot_without_filter_exposes_reader_story_list() {
        let base_dir = create_test_career_dir("news_snapshot_reader_list");
        seed_news_items(&base_dir, "career_001");

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "mazda_rookie".to_string(),
                scope_class: None,
                primary_filter: None,
                context_type: None,
                context_id: None,
            },
        )
        .expect("snapshot");
        let snapshot_json = serde_json::to_value(&snapshot).expect("snapshot json");

        assert_eq!(
            snapshot_json
                .get("scope_meta")
                .and_then(|value| value.get("primary_filter")),
            Some(&Value::Null)
        );
        assert!(snapshot_json.get("cover_lead").is_none());
        assert!(snapshot_json.get("feed").is_none());
        assert!(snapshot_json
            .get("stories")
            .and_then(|value| value.as_array())
            .map(|stories| !stories.is_empty())
            .unwrap_or(false));
        let filter_ids: Vec<&str> = snapshot
            .primary_filters
            .iter()
            .map(|filter| filter.id.as_str())
            .collect();
        assert_eq!(
            filter_ids,
            vec!["Corridas", "Pilotos", "Equipes", "Mercado"]
        );
        assert!(snapshot.hero.subtitle.contains("proxima"));
        assert_eq!(snapshot.contextual_filters.len(), 0);

        let _ = fs::remove_dir_all(base_dir);
    }

    mod race_editorial_tests {
        use super::super::{
            compose_race_body, detect_race_modifiers, detect_race_trigger, lead_change_bucket,
            streak_bucket, RaceModifier, RaceStoryContext, RaceTrigger,
        };
        use crate::news::NewsImportance;

        fn ctx(position: Option<i32>) -> RaceStoryContext {
            RaceStoryContext {
                driver_name: "Carlos Mendes".to_string(),
                category_name: "Mazda MX-5 Rookie Cup".to_string(),
                driver_position: position,
                driver_points: Some(120),
                win_streak: 1,
                item_seed: 0,
                is_lead_change: false,
                is_dominant_win: false,
                rival_finish_position: None,
                rival_dnf: false,
                pole_plus_win: false,
                recovery_win: false,
                first_win_of_season: false,
                first_win_of_career: false,
            }
        }

        fn ctx_streak(position: Option<i32>, streak: u32) -> RaceStoryContext {
            RaceStoryContext {
                win_streak: streak,
                ..ctx(position)
            }
        }

        fn ctx_seed(position: Option<i32>, streak: u32, seed: u64) -> RaceStoryContext {
            RaceStoryContext {
                win_streak: streak,
                item_seed: seed,
                ..ctx(position)
            }
        }

        fn ctx_lead_change(streak: u32) -> RaceStoryContext {
            RaceStoryContext {
                driver_position: Some(1),
                win_streak: streak,
                is_lead_change: true,
                ..ctx(Some(1))
            }
        }

        // ── Parte 1: detecção de trigger ──────────────────────────────────────

        #[test]
        fn test_detect_leader_won() {
            assert_eq!(
                detect_race_trigger(&ctx(Some(1)), &NewsImportance::Alta),
                RaceTrigger::LeaderWon
            );
            assert_eq!(
                detect_race_trigger(&ctx(Some(1)), &NewsImportance::Destaque),
                RaceTrigger::LeaderWon
            );
        }

        #[test]
        fn test_detect_leader_had_bad_result() {
            assert_eq!(
                detect_race_trigger(&ctx(Some(1)), &NewsImportance::Baixa),
                RaceTrigger::LeaderHadBadResult
            );
            assert_eq!(
                detect_race_trigger(&ctx(Some(1)), &NewsImportance::Media),
                RaceTrigger::LeaderHadBadResult
            );
        }

        #[test]
        fn test_detect_vice_won() {
            assert_eq!(
                detect_race_trigger(&ctx(Some(2)), &NewsImportance::Alta),
                RaceTrigger::ViceWon
            );
        }

        #[test]
        fn test_detect_midfield_won() {
            assert_eq!(
                detect_race_trigger(&ctx(Some(7)), &NewsImportance::Destaque),
                RaceTrigger::MidfieldDriverWon
            );
            assert_eq!(
                detect_race_trigger(&ctx(Some(5)), &NewsImportance::Alta),
                RaceTrigger::MidfieldDriverWon
            );
        }

        #[test]
        fn test_detect_fallback_when_no_position() {
            assert_eq!(
                detect_race_trigger(&ctx(None), &NewsImportance::Alta),
                RaceTrigger::FallbackRaceResult
            );
        }

        #[test]
        fn test_detect_fallback_for_position_3_or_4() {
            assert_eq!(
                detect_race_trigger(&ctx(Some(3)), &NewsImportance::Alta),
                RaceTrigger::FallbackRaceResult
            );
            assert_eq!(
                detect_race_trigger(&ctx(Some(4)), &NewsImportance::Destaque),
                RaceTrigger::FallbackRaceResult
            );
        }

        #[test]
        fn test_fallback_trigger_body_is_none() {
            let result = compose_race_body(RaceTrigger::FallbackRaceResult, &ctx(Some(3)));
            assert!(
                result.is_none(),
                "FallbackRaceResult deve retornar None para body"
            );
        }

        #[test]
        fn test_body_mentions_driver_name() {
            // LeaderHadBadResult é intencionalmente impessoal ("O líder do campeonato...")
            for trigger in [
                RaceTrigger::LeaderWon,
                RaceTrigger::ViceWon,
                RaceTrigger::MidfieldDriverWon,
            ] {
                let body = compose_race_body(trigger, &ctx(Some(1))).expect("body should exist");
                assert!(
                    body.contains("Carlos Mendes"),
                    "{trigger:?}: body deve mencionar o piloto: {body}",
                );
            }
        }

        #[test]
        fn test_body_does_not_equal_headline() {
            let headline = "Abertura em Okayama esquenta o grid";
            for trigger in [
                RaceTrigger::LeaderWon,
                RaceTrigger::ViceWon,
                RaceTrigger::MidfieldDriverWon,
                RaceTrigger::LeaderHadBadResult,
            ] {
                let body = compose_race_body(trigger, &ctx(Some(1))).expect("body should exist");
                assert_ne!(
                    body, headline,
                    "{trigger:?}: body não deve ser idêntico à headline"
                );
            }
        }

        // ── Parte 2: bucketização e progressão de sequência ───────────────────

        #[test]
        fn test_streak_bucket_mapping() {
            assert_eq!(streak_bucket(0), 1, "0 deve cair no bucket 1");
            assert_eq!(streak_bucket(1), 1);
            assert_eq!(streak_bucket(5), 5);
            assert_eq!(streak_bucket(9), 9);
            assert_eq!(streak_bucket(10), 10);
            assert_eq!(streak_bucket(11), 11, "11 deve cair no bucket especial");
            assert_eq!(streak_bucket(20), 11, "20+ deve cair no bucket especial");
            assert_eq!(streak_bucket(99), 11);
        }

        #[test]
        fn test_leader_won_text_changes_with_streak() {
            let body1 = compose_race_body(RaceTrigger::LeaderWon, &ctx_streak(Some(1), 1)).unwrap();
            let body3 = compose_race_body(RaceTrigger::LeaderWon, &ctx_streak(Some(1), 3)).unwrap();
            let body10 =
                compose_race_body(RaceTrigger::LeaderWon, &ctx_streak(Some(1), 10)).unwrap();
            let body11 =
                compose_race_body(RaceTrigger::LeaderWon, &ctx_streak(Some(1), 11)).unwrap();

            assert_ne!(body1, body3, "streak 1 e 3 devem gerar textos diferentes");
            assert_ne!(body3, body10, "streak 3 e 10 devem gerar textos diferentes");
            assert_ne!(
                body10, body11,
                "streak 10 e 11+ devem gerar textos diferentes"
            );
            assert_ne!(
                body1, body11,
                "streak 1 e 11+ devem gerar textos diferentes"
            );
        }

        #[test]
        fn test_vice_won_text_changes_with_streak() {
            let body1 = compose_race_body(RaceTrigger::ViceWon, &ctx_streak(Some(2), 1)).unwrap();
            let body3 = compose_race_body(RaceTrigger::ViceWon, &ctx_streak(Some(2), 3)).unwrap();
            let body10 = compose_race_body(RaceTrigger::ViceWon, &ctx_streak(Some(2), 10)).unwrap();
            let body11 = compose_race_body(RaceTrigger::ViceWon, &ctx_streak(Some(2), 11)).unwrap();

            assert_ne!(
                body1, body3,
                "vice streak 1 e 3 devem gerar textos diferentes"
            );
            assert_ne!(
                body3, body10,
                "vice streak 3 e 10 devem gerar textos diferentes"
            );
            assert_ne!(
                body10, body11,
                "vice streak 10 e 11+ devem gerar textos diferentes"
            );
            assert_ne!(
                body1, body11,
                "vice streak 1 e 11+ devem gerar textos diferentes"
            );
        }

        #[test]
        fn test_bucket_11plus_uses_special_tone() {
            // O bucket 11+ deve usar linguagem histórica/contemplativa distinta dos outros
            let body_leader =
                compose_race_body(RaceTrigger::LeaderWon, &ctx_streak(Some(1), 15)).unwrap();
            let body_vice =
                compose_race_body(RaceTrigger::ViceWon, &ctx_streak(Some(2), 15)).unwrap();

            // Deve ser diferente de bucket 1 e bucket 10
            let leader1 =
                compose_race_body(RaceTrigger::LeaderWon, &ctx_streak(Some(1), 1)).unwrap();
            let leader10 =
                compose_race_body(RaceTrigger::LeaderWon, &ctx_streak(Some(1), 10)).unwrap();
            assert_ne!(body_leader, leader1);
            assert_ne!(body_leader, leader10);

            let vice1 = compose_race_body(RaceTrigger::ViceWon, &ctx_streak(Some(2), 1)).unwrap();
            let vice10 = compose_race_body(RaceTrigger::ViceWon, &ctx_streak(Some(2), 10)).unwrap();
            assert_ne!(body_vice, vice1);
            assert_ne!(body_vice, vice10);
        }

        #[test]
        fn test_variant_seed_produces_different_texts_for_same_streak() {
            // seed par e ímpar devem produzir textos diferentes (2 variantes por bucket)
            let v0 = compose_race_body(RaceTrigger::LeaderWon, &ctx_seed(Some(1), 5, 0)).unwrap();
            let v1 = compose_race_body(RaceTrigger::LeaderWon, &ctx_seed(Some(1), 5, 1)).unwrap();
            assert_ne!(
                v0, v1,
                "variante 0 e variante 1 do bucket 5 devem ser diferentes"
            );

            let v0v = compose_race_body(RaceTrigger::ViceWon, &ctx_seed(Some(2), 5, 0)).unwrap();
            let v1v = compose_race_body(RaceTrigger::ViceWon, &ctx_seed(Some(2), 5, 1)).unwrap();
            assert_ne!(
                v0v, v1v,
                "vice variante 0 e variante 1 do bucket 5 devem ser diferentes"
            );
        }

        // ── Parte 3: mudança de liderança ────────────────────────────────────

        #[test]
        fn test_lead_change_bucket_mapping() {
            assert_eq!(lead_change_bucket(0), 1);
            assert_eq!(lead_change_bucket(1), 1);
            assert_eq!(lead_change_bucket(2), 2);
            assert_eq!(lead_change_bucket(3), 2);
            assert_eq!(lead_change_bucket(4), 3);
            assert_eq!(lead_change_bucket(10), 3);
        }

        #[test]
        fn test_vice_wins_no_lead_change_stays_vice_won() {
            // Piloto em 2° no campeonato, vence a corrida mas não assume a liderança
            let ctx = RaceStoryContext {
                driver_position: Some(2),
                is_lead_change: false,
                ..ctx(Some(2))
            };
            assert_eq!(
                detect_race_trigger(&ctx, &NewsImportance::Alta),
                RaceTrigger::ViceWon
            );
        }

        #[test]
        fn test_vice_wins_and_takes_lead_fires_lead_changed() {
            // Piloto assume o topo do campeonato → LeadChanged tem prioridade
            let ctx = ctx_lead_change(1);
            assert_eq!(
                detect_race_trigger(&ctx, &NewsImportance::Alta),
                RaceTrigger::LeadChanged
            );
            assert_eq!(
                detect_race_trigger(&ctx, &NewsImportance::Destaque),
                RaceTrigger::LeadChanged
            );
        }

        #[test]
        fn test_lead_changed_body_differs_from_vice_won() {
            let lead_body =
                compose_race_body(RaceTrigger::LeadChanged, &ctx_lead_change(1)).unwrap();
            let vice_body =
                compose_race_body(RaceTrigger::ViceWon, &ctx_streak(Some(2), 1)).unwrap();
            assert_ne!(
                lead_body, vice_body,
                "LeadChanged e ViceWon devem ter textos distintos"
            );
        }

        #[test]
        fn test_lead_changed_body_changes_with_streak() {
            let body1 = compose_race_body(RaceTrigger::LeadChanged, &ctx_lead_change(1)).unwrap();
            let body3 = compose_race_body(RaceTrigger::LeadChanged, &ctx_lead_change(3)).unwrap();
            let body5 = compose_race_body(RaceTrigger::LeadChanged, &ctx_lead_change(5)).unwrap();
            assert_ne!(body1, body3, "streak 1 e 3 devem gerar textos diferentes");
            assert_ne!(body3, body5, "streak 3 e 5 devem gerar textos diferentes");
        }

        #[test]
        fn test_lead_changed_body_mentions_driver() {
            for streak in [1u32, 3, 5] {
                let body =
                    compose_race_body(RaceTrigger::LeadChanged, &ctx_lead_change(streak)).unwrap();
                assert!(
                    body.contains("Carlos Mendes"),
                    "streak {streak}: body de LeadChanged deve mencionar o piloto: {body}",
                );
            }
        }

        /// Dump de revisão — não é assertion, só imprime para leitura humana.
        /// Execute com: cargo test dump_lead_change -- --nocapture
        #[test]
        fn dump_lead_change_progression_for_review() {
            let checkpoints = [1u32, 2, 3, 5];

            println!("\n=== LEAD CHANGE PROGRESSION ===");
            for &streak in &checkpoints {
                let ctx = RaceStoryContext {
                    item_seed: 0,
                    ..ctx_lead_change(streak)
                };
                let body = compose_race_body(RaceTrigger::LeadChanged, &ctx).unwrap_or_default();
                println!("streak={streak}: {body}");
            }
        }

        /// Dump de revisão — não é assertion, só imprime para leitura humana.
        /// Execute com: cargo test dump_race_win_streak -- --nocapture
        #[test]
        fn dump_race_win_streak_progression_for_review() {
            let checkpoints = [1u32, 2, 3, 5, 7, 10, 11];

            println!("\n=== LEADER WIN STREAK PROGRESSION ===");
            for &streak in &checkpoints {
                let ctx = ctx_seed(Some(1), streak, 0);
                let label = if streak >= 11 {
                    "11+".to_string()
                } else {
                    streak.to_string()
                };
                let body = compose_race_body(RaceTrigger::LeaderWon, &ctx).unwrap_or_default();
                println!("streak={label}: {body}");
            }

            println!("\n=== VICE WIN STREAK PROGRESSION ===");
            for &streak in &checkpoints {
                let ctx = ctx_seed(Some(2), streak, 0);
                let label = if streak >= 11 {
                    "11+".to_string()
                } else {
                    streak.to_string()
                };
                let body = compose_race_body(RaceTrigger::ViceWon, &ctx).unwrap_or_default();
                println!("streak={label}: {body}");
            }
        }

        // ── Parte 4: modificadores ────────────────────────────────────────────

        fn base_ctx() -> RaceStoryContext {
            RaceStoryContext {
                driver_name: "Ana Silva".to_string(),
                category_name: "GT4".to_string(),
                driver_position: Some(1),
                driver_points: Some(200),
                win_streak: 1,
                item_seed: 0,
                is_lead_change: false,
                is_dominant_win: false,
                rival_finish_position: None,
                rival_dnf: false,
                pole_plus_win: false,
                recovery_win: false,
                first_win_of_season: false,
                first_win_of_career: false,
            }
        }

        #[test]
        fn test_no_modifiers_by_default() {
            let mods = detect_race_modifiers(&base_ctx(), RaceTrigger::LeaderWon);
            assert!(mods.is_empty());
        }

        #[test]
        fn test_dominant_win_modifier() {
            let ctx = RaceStoryContext {
                is_dominant_win: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(mods, vec![RaceModifier::DominantWin]);
        }

        #[test]
        fn test_close_rival_modifier() {
            let ctx = RaceStoryContext {
                rival_finish_position: Some(2),
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(mods, vec![RaceModifier::MainRivalFinishedClose]);
        }

        #[test]
        fn test_close_rival_position_5_is_close() {
            let ctx = RaceStoryContext {
                rival_finish_position: Some(5),
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(mods, vec![RaceModifier::MainRivalFinishedClose]);
        }

        #[test]
        fn test_far_rival_position_6_is_far() {
            let ctx = RaceStoryContext {
                rival_finish_position: Some(6),
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(
                mods,
                vec![RaceModifier::MainRivalFinishedFar { position: 6 }]
            );
        }

        #[test]
        fn test_far_rival_modifier() {
            let ctx = RaceStoryContext {
                rival_finish_position: Some(8),
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(
                mods,
                vec![RaceModifier::MainRivalFinishedFar { position: 8 }]
            );
        }

        #[test]
        fn test_dnf_rival_modifier() {
            let ctx = RaceStoryContext {
                rival_dnf: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(mods, vec![RaceModifier::MainRivalDnf]);
        }

        #[test]
        fn test_dnf_takes_priority_over_far_position() {
            let ctx = RaceStoryContext {
                rival_finish_position: Some(9),
                rival_dnf: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(mods, vec![RaceModifier::MainRivalDnf]);
        }

        #[test]
        fn test_dnf_plus_dominant_fills_two_slots() {
            let ctx = RaceStoryContext {
                rival_dnf: true,
                is_dominant_win: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(mods.len(), 2);
            assert!(mods.contains(&RaceModifier::MainRivalDnf));
            assert!(mods.contains(&RaceModifier::DominantWin));
        }

        #[test]
        fn test_far_plus_dominant_fills_two_slots() {
            let ctx = RaceStoryContext {
                rival_finish_position: Some(8),
                is_dominant_win: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(mods.len(), 2);
            assert!(mods.contains(&RaceModifier::MainRivalFinishedFar { position: 8 }));
            assert!(mods.contains(&RaceModifier::DominantWin));
        }

        #[test]
        fn test_max_two_modifiers() {
            let ctx = RaceStoryContext {
                rival_dnf: true,
                rival_finish_position: Some(9),
                is_dominant_win: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert!(mods.len() <= 2);
        }

        #[test]
        fn test_no_modifiers_for_fallback_trigger() {
            let ctx = RaceStoryContext {
                is_dominant_win: true,
                ..base_ctx()
            };
            assert!(detect_race_modifiers(&ctx, RaceTrigger::FallbackRaceResult).is_empty());
        }

        #[test]
        fn test_no_modifiers_for_midfield_without_flags() {
            // MidfieldDriverWon sem nenhum flag de modificador → vazio
            assert!(detect_race_modifiers(&base_ctx(), RaceTrigger::MidfieldDriverWon).is_empty());
        }

        #[test]
        fn test_dominant_win_fires_for_midfield_trigger() {
            // Parte 6: MidfieldDriverWon agora é elegível para DominantWin
            let ctx = RaceStoryContext {
                is_dominant_win: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::MidfieldDriverWon);
            assert_eq!(mods, vec![RaceModifier::DominantWin]);
        }

        #[test]
        fn test_no_modifiers_for_bad_result_trigger() {
            let ctx = RaceStoryContext {
                is_dominant_win: true,
                ..base_ctx()
            };
            assert!(detect_race_modifiers(&ctx, RaceTrigger::LeaderHadBadResult).is_empty());
        }

        #[test]
        fn test_lead_changed_trigger_supports_dnf_modifier() {
            let ctx = RaceStoryContext {
                is_lead_change: true,
                rival_dnf: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeadChanged);
            assert_eq!(mods, vec![RaceModifier::MainRivalDnf]);
        }

        #[test]
        fn test_vice_won_trigger_supports_far_modifier() {
            let ctx = RaceStoryContext {
                driver_position: Some(2),
                rival_finish_position: Some(7),
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::ViceWon);
            assert_eq!(
                mods,
                vec![RaceModifier::MainRivalFinishedFar { position: 7 }]
            );
        }

        #[test]
        fn test_body_with_modifier_is_longer_than_without() {
            let plain = compose_race_body(RaceTrigger::LeaderWon, &base_ctx()).unwrap();
            let ctx = RaceStoryContext {
                rival_dnf: true,
                ..base_ctx()
            };
            let with_mod = compose_race_body(RaceTrigger::LeaderWon, &ctx).unwrap();
            assert!(
                with_mod.len() > plain.len(),
                "body com modificador deve ser mais longo"
            );
        }

        #[test]
        fn test_body_with_two_modifiers_is_longer_than_one() {
            let ctx1 = RaceStoryContext {
                rival_dnf: true,
                ..base_ctx()
            };
            let ctx2 = RaceStoryContext {
                rival_dnf: true,
                is_dominant_win: true,
                ..base_ctx()
            };
            let body1 = compose_race_body(RaceTrigger::LeaderWon, &ctx1).unwrap();
            let body2 = compose_race_body(RaceTrigger::LeaderWon, &ctx2).unwrap();
            assert!(
                body2.len() > body1.len(),
                "body com 2 modificadores deve ser mais longo que 1"
            );
        }

        /// Dump de revisão — não é assertion, só imprime para leitura humana.
        /// Execute com: cargo test dump_race_modifiers -- --nocapture
        #[test]
        fn dump_race_modifiers_for_review() {
            let triggers = [
                (RaceTrigger::LeaderWon, "LeaderWon"),
                (RaceTrigger::LeadChanged, "LeadChanged"),
                (RaceTrigger::ViceWon, "ViceWon"),
            ];
            let scenarios: &[(&str, RaceStoryContext)] = &[
                (
                    "DominantWin",
                    RaceStoryContext {
                        is_dominant_win: true,
                        ..base_ctx()
                    },
                ),
                (
                    "RivalClose(3)",
                    RaceStoryContext {
                        rival_finish_position: Some(3),
                        ..base_ctx()
                    },
                ),
                (
                    "RivalFar(8)",
                    RaceStoryContext {
                        rival_finish_position: Some(8),
                        ..base_ctx()
                    },
                ),
                (
                    "RivalDnf",
                    RaceStoryContext {
                        rival_dnf: true,
                        ..base_ctx()
                    },
                ),
                (
                    "DnfPlusDominant",
                    RaceStoryContext {
                        rival_dnf: true,
                        is_dominant_win: true,
                        ..base_ctx()
                    },
                ),
                (
                    "FarPlusDominant",
                    RaceStoryContext {
                        rival_finish_position: Some(9),
                        is_dominant_win: true,
                        ..base_ctx()
                    },
                ),
            ];
            println!("\n=== RACE MODIFIERS DUMP ===");
            for (trigger, trigger_name) in &triggers {
                println!("\n--- {trigger_name} ---");
                for (label, ctx) in scenarios {
                    let body =
                        compose_race_body(*trigger, ctx).unwrap_or_else(|| "None".to_string());
                    println!("[{label}]\n  {body}\n");
                }
            }
        }

        // ── Parte 6: modificadores estendidos ────────────────────────────────

        use super::super::{compose_modifier_phrase, RECOVERY_WIN_MIN_GRID};

        #[test]
        fn test_pole_plus_win_modifier() {
            let ctx = RaceStoryContext {
                pole_plus_win: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(mods, vec![RaceModifier::PolePlusWin]);
        }

        #[test]
        fn test_recovery_win_modifier() {
            let ctx = RaceStoryContext {
                recovery_win: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(mods, vec![RaceModifier::RecoveryWin]);
        }

        #[test]
        fn test_first_win_of_season_modifier() {
            let ctx = RaceStoryContext {
                first_win_of_season: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(mods, vec![RaceModifier::FirstWinOfSeason]);
        }

        #[test]
        fn test_first_win_of_career_modifier() {
            let ctx = RaceStoryContext {
                first_win_of_career: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(mods, vec![RaceModifier::FirstWinOfCareer]);
        }

        #[test]
        fn test_first_win_of_career_beats_first_win_of_season() {
            // Primeira da carreira implica primeira da temporada — só FirstWinOfCareer deve aparecer
            let ctx = RaceStoryContext {
                first_win_of_career: true,
                first_win_of_season: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert!(mods.contains(&RaceModifier::FirstWinOfCareer));
            assert!(!mods.iter().any(|m| *m == RaceModifier::FirstWinOfSeason));
        }

        #[test]
        fn test_recovery_win_modifier_for_midfield_trigger() {
            // MidfieldDriverWon agora é elegível para modificadores
            let ctx = RaceStoryContext {
                recovery_win: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::MidfieldDriverWon);
            assert_eq!(mods, vec![RaceModifier::RecoveryWin]);
        }

        #[test]
        fn test_first_win_of_career_for_midfield_trigger() {
            let ctx = RaceStoryContext {
                first_win_of_career: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::MidfieldDriverWon);
            assert_eq!(mods, vec![RaceModifier::FirstWinOfCareer]);
        }

        #[test]
        fn test_rival_dnf_plus_first_career_fills_two_slots() {
            // DNF (P1) + FirstWinOfCareer (P7) sem nada entre eles
            let ctx = RaceStoryContext {
                rival_dnf: true,
                first_win_of_career: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(mods.len(), 2);
            assert!(mods.contains(&RaceModifier::MainRivalDnf));
            assert!(mods.contains(&RaceModifier::FirstWinOfCareer));
        }

        #[test]
        fn test_pole_takes_slot_2_before_first_win() {
            // DNF (P1) + PolePlusWin (P6) → PolePlusWin ganha slot 2; FirstWinOfCareer (P7) fica de fora
            let ctx = RaceStoryContext {
                rival_dnf: true,
                pole_plus_win: true,
                first_win_of_career: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(mods.len(), 2);
            assert!(mods.contains(&RaceModifier::MainRivalDnf));
            assert!(mods.contains(&RaceModifier::PolePlusWin));
            assert!(!mods.iter().any(|m| *m == RaceModifier::FirstWinOfCareer));
        }

        #[test]
        fn test_recovery_beats_pole_plus_win_both_false_by_default() {
            // Recovery e Pole são mutuamente exclusivos no contexto real (grid 1 ≠ grid ≥5),
            // mas se ambos fossem true, Recovery (P5) tem prioridade sobre Pole (P6)
            let ctx = RaceStoryContext {
                recovery_win: true,
                pole_plus_win: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert_eq!(mods.len(), 2);
            assert!(mods.contains(&RaceModifier::RecoveryWin));
            assert!(mods.contains(&RaceModifier::PolePlusWin));
        }

        #[test]
        fn test_recovery_win_min_grid_constant() {
            // Grid exatamente no threshold dispara; abaixo não dispara
            // (este teste valida a constante sem depender do DB)
            assert!(
                RECOVERY_WIN_MIN_GRID >= 3,
                "threshold deve ser pelo menos 3"
            );
            assert!(
                RECOVERY_WIN_MIN_GRID <= 8,
                "threshold não deve ser excessivamente alto"
            );
        }

        #[test]
        fn test_extended_modifier_bodies_mention_driver_name() {
            let new_modifiers = [
                RaceModifier::RecoveryWin,
                RaceModifier::PolePlusWin,
                RaceModifier::FirstWinOfCareer,
                RaceModifier::FirstWinOfSeason,
            ];
            for modifier in &new_modifiers {
                for seed in 0usize..2 {
                    let phrase = compose_modifier_phrase(*modifier, &base_ctx(), seed);
                    assert!(
                        phrase.contains("Ana Silva"),
                        "{modifier:?} seed={seed}: frase deve mencionar o piloto: {phrase}"
                    );
                }
            }
        }

        #[test]
        fn test_extended_modifier_bodies_have_two_variants() {
            let new_modifiers = [
                RaceModifier::RecoveryWin,
                RaceModifier::PolePlusWin,
                RaceModifier::FirstWinOfCareer,
                RaceModifier::FirstWinOfSeason,
            ];
            for modifier in &new_modifiers {
                let v0 = compose_modifier_phrase(*modifier, &base_ctx(), 0);
                let v1 = compose_modifier_phrase(*modifier, &base_ctx(), 1);
                assert_ne!(
                    v0, v1,
                    "{modifier:?}: variante 0 e variante 1 devem diferir"
                );
            }
        }

        #[test]
        fn test_max_two_modifiers_with_extended_set() {
            let ctx = RaceStoryContext {
                rival_dnf: true,
                is_dominant_win: true,
                recovery_win: true,
                pole_plus_win: true,
                first_win_of_career: true,
                first_win_of_season: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeaderWon);
            assert!(
                mods.len() <= 2,
                "nunca deve exceder 2 modificadores: {:?}",
                mods
            );
        }

        /// Dump de revisão — não é assertion, só imprime para leitura humana.
        /// Execute com: cargo test dump_race_extended_modifiers -- --nocapture
        #[test]
        fn dump_race_extended_modifiers_for_review() {
            let scenarios: &[(&str, RaceStoryContext, RaceTrigger)] = &[
                (
                    "leader_streak3 + dominant + pole_plus_win",
                    RaceStoryContext {
                        win_streak: 3,
                        is_dominant_win: true,
                        pole_plus_win: true,
                        ..base_ctx()
                    },
                    RaceTrigger::LeaderWon,
                ),
                (
                    "vice_won + rival_9th + first_win_of_season",
                    RaceStoryContext {
                        driver_position: Some(2),
                        rival_finish_position: Some(9),
                        first_win_of_season: true,
                        ..base_ctx()
                    },
                    RaceTrigger::ViceWon,
                ),
                (
                    "midfield_win + recovery_win + first_win_of_career",
                    RaceStoryContext {
                        driver_position: Some(7),
                        recovery_win: true,
                        first_win_of_career: true,
                        ..base_ctx()
                    },
                    RaceTrigger::MidfieldDriverWon,
                ),
                (
                    "lead_changed_streak2 + rival_dnf + first_win_of_career",
                    RaceStoryContext {
                        driver_position: Some(1),
                        win_streak: 2,
                        is_lead_change: true,
                        rival_dnf: true,
                        first_win_of_career: true,
                        ..base_ctx()
                    },
                    RaceTrigger::LeadChanged,
                ),
                (
                    "leader_pole_plus_win solo",
                    RaceStoryContext {
                        pole_plus_win: true,
                        ..base_ctx()
                    },
                    RaceTrigger::LeaderWon,
                ),
            ];
            println!("\n=== EXTENDED MODIFIER CASES ===");
            for (label, ctx, trigger) in scenarios {
                let body = compose_race_body(*trigger, ctx).unwrap_or_else(|| "None".to_string());
                println!("\ncase={label}");
                println!("body: {body}");
            }
        }

        // ── Parte 7: triggers principais raros ───────────────────────────────

        use super::super::{
            compose_first_win_of_career_body, compose_first_win_of_season_body,
            compose_shock_win_body, SHOCK_WIN_THRESHOLD,
        };

        fn ctx_first_win_of_career(position: i32) -> RaceStoryContext {
            RaceStoryContext {
                driver_position: Some(position),
                first_win_of_career: true,
                ..base_ctx()
            }
        }

        fn ctx_first_win_of_season(position: i32) -> RaceStoryContext {
            RaceStoryContext {
                driver_position: Some(position),
                first_win_of_season: true,
                ..base_ctx()
            }
        }

        #[test]
        fn test_first_win_of_career_becomes_primary_trigger_when_no_stronger() {
            // P3 no campeonato, primeira vitória da carreira → deve disparar FirstWinOfCareer
            let ctx = ctx_first_win_of_career(3);
            assert_eq!(
                detect_race_trigger(&ctx, &NewsImportance::Alta),
                RaceTrigger::FirstWinOfCareer
            );
        }

        #[test]
        fn test_first_win_of_career_at_p5_is_first_win_trigger_not_midfield() {
            // P5 com first_win_of_career → FirstWinOfCareer tem prioridade sobre MidfieldDriverWon
            let ctx = ctx_first_win_of_career(5);
            assert_eq!(
                detect_race_trigger(&ctx, &NewsImportance::Alta),
                RaceTrigger::FirstWinOfCareer
            );
        }

        #[test]
        fn test_first_win_of_career_at_p10_is_first_win_trigger_not_shock() {
            // P10 com first_win_of_career → FirstWinOfCareer tem prioridade sobre ShockWin
            let ctx = ctx_first_win_of_career(10);
            assert_eq!(
                detect_race_trigger(&ctx, &NewsImportance::Alta),
                RaceTrigger::FirstWinOfCareer
            );
        }

        #[test]
        fn test_lead_changed_beats_first_win_of_career() {
            // LeadChanged tem prioridade sobre FirstWinOfCareer
            let ctx = RaceStoryContext {
                driver_position: Some(1),
                is_lead_change: true,
                first_win_of_career: true,
                ..base_ctx()
            };
            assert_eq!(
                detect_race_trigger(&ctx, &NewsImportance::Alta),
                RaceTrigger::LeadChanged
            );
        }

        #[test]
        fn test_leader_won_beats_first_win_of_career() {
            // LeaderWon (P1 sem lead change) tem prioridade sobre FirstWinOfCareer
            let ctx = RaceStoryContext {
                driver_position: Some(1),
                is_lead_change: false,
                first_win_of_career: true,
                ..base_ctx()
            };
            assert_eq!(
                detect_race_trigger(&ctx, &NewsImportance::Alta),
                RaceTrigger::LeaderWon
            );
        }

        #[test]
        fn test_vice_won_beats_first_win_of_career() {
            // ViceWon (P2) tem prioridade sobre FirstWinOfCareer
            let ctx = RaceStoryContext {
                driver_position: Some(2),
                first_win_of_career: true,
                ..base_ctx()
            };
            assert_eq!(
                detect_race_trigger(&ctx, &NewsImportance::Alta),
                RaceTrigger::ViceWon
            );
        }

        #[test]
        fn test_first_win_of_season_becomes_primary_trigger_when_no_stronger() {
            let ctx = ctx_first_win_of_season(4);
            assert_eq!(
                detect_race_trigger(&ctx, &NewsImportance::Alta),
                RaceTrigger::FirstWinOfSeason
            );
        }

        #[test]
        fn test_first_win_of_career_beats_first_win_of_season_as_trigger() {
            // Se ambos true, FirstWinOfCareer dispara (mais específico)
            let ctx = RaceStoryContext {
                driver_position: Some(5),
                first_win_of_career: true,
                first_win_of_season: true,
                ..base_ctx()
            };
            assert_eq!(
                detect_race_trigger(&ctx, &NewsImportance::Alta),
                RaceTrigger::FirstWinOfCareer
            );
        }

        #[test]
        fn test_shock_win_fires_at_threshold() {
            let ctx = RaceStoryContext {
                driver_position: Some(SHOCK_WIN_THRESHOLD),
                ..base_ctx()
            };
            assert_eq!(
                detect_race_trigger(&ctx, &NewsImportance::Alta),
                RaceTrigger::ShockWin
            );
        }

        #[test]
        fn test_midfield_win_below_shock_threshold() {
            let ctx = RaceStoryContext {
                driver_position: Some(SHOCK_WIN_THRESHOLD - 1),
                ..base_ctx()
            };
            assert_eq!(
                detect_race_trigger(&ctx, &NewsImportance::Alta),
                RaceTrigger::MidfieldDriverWon
            );
        }

        #[test]
        fn test_shock_win_body_differs_from_midfield_body() {
            let midfield_ctx = RaceStoryContext {
                driver_position: Some(6),
                ..base_ctx()
            };
            let shock_ctx = RaceStoryContext {
                driver_position: Some(10),
                ..base_ctx()
            };
            let midfield_body =
                compose_race_body(RaceTrigger::MidfieldDriverWon, &midfield_ctx).unwrap();
            let shock_body = compose_race_body(RaceTrigger::ShockWin, &shock_ctx).unwrap();
            assert_ne!(
                midfield_body, shock_body,
                "ShockWin e MidfieldDriverWon devem ter textos distintos"
            );
        }

        #[test]
        fn test_first_win_of_career_body_is_not_first_win_of_season_body() {
            let career_body = compose_first_win_of_career_body("Piloto", "Cat", 0);
            let season_body = compose_first_win_of_season_body("Piloto", "Cat", 0);
            assert_ne!(career_body, season_body);
        }

        #[test]
        fn test_first_win_of_career_body_mentions_driver() {
            for v in 0..3 {
                let body = compose_first_win_of_career_body("Ana Silva", "GT4", v);
                assert!(body.contains("Ana Silva"), "v={v}: {body}");
            }
        }

        #[test]
        fn test_first_win_of_season_body_mentions_driver() {
            for v in 0..3 {
                let body = compose_first_win_of_season_body("Ana Silva", "GT4", v);
                assert!(body.contains("Ana Silva"), "v={v}: {body}");
            }
        }

        #[test]
        fn test_shock_win_body_mentions_driver() {
            for v in 0..3 {
                let body = compose_shock_win_body("Ana Silva", "GT4", v);
                assert!(body.contains("Ana Silva"), "v={v}: {body}");
            }
        }

        #[test]
        fn test_first_win_of_career_not_added_as_modifier_when_it_is_trigger() {
            // Quando FirstWinOfCareer é o trigger, não deve aparecer também como modifier
            let ctx = RaceStoryContext {
                driver_position: Some(5),
                first_win_of_career: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::FirstWinOfCareer);
            assert!(
                !mods.iter().any(|m| *m == RaceModifier::FirstWinOfCareer),
                "FirstWinOfCareer não deve aparecer como modifier quando já é o trigger"
            );
        }

        #[test]
        fn test_first_win_of_season_not_added_as_modifier_when_it_is_trigger() {
            let ctx = RaceStoryContext {
                driver_position: Some(4),
                first_win_of_season: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::FirstWinOfSeason);
            assert!(
                !mods.iter().any(|m| *m == RaceModifier::FirstWinOfSeason),
                "FirstWinOfSeason não deve aparecer como modifier quando já é o trigger"
            );
        }

        #[test]
        fn test_first_win_of_career_can_still_be_modifier_for_lead_changed() {
            // LeadChanged como trigger + first_win_of_career → career win vira modifier
            let ctx = RaceStoryContext {
                driver_position: Some(1),
                is_lead_change: true,
                first_win_of_career: true,
                ..base_ctx()
            };
            let mods = detect_race_modifiers(&ctx, RaceTrigger::LeadChanged);
            assert!(
                mods.contains(&RaceModifier::FirstWinOfCareer),
                "FirstWinOfCareer deve aparecer como modifier quando trigger é LeadChanged: {:?}",
                mods
            );
        }

        /// Dump de revisão — não é assertion, só imprime para leitura humana.
        /// Execute com: cargo test dump_race_rare_primary_triggers -- --nocapture
        #[test]
        fn dump_race_rare_primary_triggers_for_review() {
            let cases: &[(&str, RaceTrigger, RaceStoryContext)] = &[
                (
                    "first_win_of_career (p5)",
                    RaceTrigger::FirstWinOfCareer,
                    RaceStoryContext {
                        driver_position: Some(5),
                        first_win_of_career: true,
                        ..base_ctx()
                    },
                ),
                (
                    "first_win_of_career + recovery",
                    RaceTrigger::FirstWinOfCareer,
                    RaceStoryContext {
                        driver_position: Some(7),
                        first_win_of_career: true,
                        recovery_win: true,
                        ..base_ctx()
                    },
                ),
                (
                    "first_win_of_season (p4)",
                    RaceTrigger::FirstWinOfSeason,
                    RaceStoryContext {
                        driver_position: Some(4),
                        first_win_of_season: true,
                        ..base_ctx()
                    },
                ),
                (
                    "first_win_of_season + dominant",
                    RaceTrigger::FirstWinOfSeason,
                    RaceStoryContext {
                        driver_position: Some(3),
                        first_win_of_season: true,
                        is_dominant_win: true,
                        ..base_ctx()
                    },
                ),
                (
                    "shock_win (p10)",
                    RaceTrigger::ShockWin,
                    RaceStoryContext {
                        driver_position: Some(10),
                        ..base_ctx()
                    },
                ),
                (
                    "shock_win + first_win_of_season",
                    RaceTrigger::ShockWin,
                    RaceStoryContext {
                        driver_position: Some(9),
                        first_win_of_season: true,
                        ..base_ctx()
                    },
                ),
                (
                    "lead_changed + first_win_of_career (career as modifier)",
                    RaceTrigger::LeadChanged,
                    RaceStoryContext {
                        driver_position: Some(1),
                        is_lead_change: true,
                        first_win_of_career: true,
                        ..base_ctx()
                    },
                ),
            ];

            println!("\n=== RARE PRIMARY TRIGGERS ===");
            for (label, trigger, ctx) in cases {
                let body = compose_race_body(*trigger, ctx).unwrap_or_else(|| "None".to_string());
                println!("\ncase={label}");
                println!("headline: (do item.titulo)");
                println!("body: {body}");
            }
        }

        // ── Parte 5: story dupla ──────────────────────────────────────────────

        use super::super::{
            compose_leader_bad_result_story, should_generate_second_story, LeaderBadResultContext,
            LEADER_BAD_RESULT_THRESHOLD,
        };

        fn leader_bad_ctx(
            seed: u64,
            position: Option<i32>,
            is_dnf: bool,
        ) -> LeaderBadResultContext {
            LeaderBadResultContext {
                leader_name: "Felipe Correa".to_string(),
                category_name: "GT4".to_string(),
                finish_position: position,
                is_dnf,
                seed,
            }
        }

        #[test]
        fn test_no_second_story_for_leader_won() {
            // LeaderWon não dispara segunda story, independente do rival
            let ctx = RaceStoryContext {
                rival_finish_position: Some(LEADER_BAD_RESULT_THRESHOLD),
                ..base_ctx()
            };
            assert!(!should_generate_second_story(&ctx, RaceTrigger::LeaderWon));
        }

        #[test]
        fn test_no_second_story_for_midfield_or_fallback() {
            let ctx = RaceStoryContext {
                rival_finish_position: Some(LEADER_BAD_RESULT_THRESHOLD),
                ..base_ctx()
            };
            assert!(!should_generate_second_story(
                &ctx,
                RaceTrigger::MidfieldDriverWon
            ));
            assert!(!should_generate_second_story(
                &ctx,
                RaceTrigger::FallbackRaceResult
            ));
            assert!(!should_generate_second_story(
                &ctx,
                RaceTrigger::LeaderHadBadResult
            ));
        }

        #[test]
        fn test_no_second_story_when_rival_finished_close() {
            // Rival em posição abaixo do threshold (ex: 5) → não é ruim o suficiente
            let ctx = RaceStoryContext {
                rival_finish_position: Some(5),
                ..base_ctx()
            };
            assert!(!should_generate_second_story(&ctx, RaceTrigger::ViceWon));
            assert!(!should_generate_second_story(
                &ctx,
                RaceTrigger::LeadChanged
            ));
        }

        #[test]
        fn test_second_story_fires_for_vice_won_with_bad_result() {
            let ctx = RaceStoryContext {
                rival_finish_position: Some(LEADER_BAD_RESULT_THRESHOLD),
                ..base_ctx()
            };
            assert!(should_generate_second_story(&ctx, RaceTrigger::ViceWon));
        }

        #[test]
        fn test_second_story_fires_for_lead_changed_with_bad_result() {
            let ctx = RaceStoryContext {
                rival_finish_position: Some(11),
                ..base_ctx()
            };
            assert!(should_generate_second_story(&ctx, RaceTrigger::LeadChanged));
        }

        #[test]
        fn test_second_story_fires_when_rival_dnf_vice_won() {
            let ctx = RaceStoryContext {
                rival_dnf: true,
                ..base_ctx()
            };
            assert!(should_generate_second_story(&ctx, RaceTrigger::ViceWon));
        }

        #[test]
        fn test_threshold_boundary_exclusive() {
            // Posição exatamente no threshold dispara; abaixo não dispara
            let ctx_at = RaceStoryContext {
                rival_finish_position: Some(LEADER_BAD_RESULT_THRESHOLD),
                ..base_ctx()
            };
            let ctx_below = RaceStoryContext {
                rival_finish_position: Some(LEADER_BAD_RESULT_THRESHOLD - 1),
                ..base_ctx()
            };
            assert!(should_generate_second_story(&ctx_at, RaceTrigger::ViceWon));
            assert!(!should_generate_second_story(
                &ctx_below,
                RaceTrigger::ViceWon
            ));
        }

        #[test]
        fn test_second_story_headline_differs_from_first() {
            let race_ctx = RaceStoryContext {
                driver_position: Some(2),
                rival_finish_position: Some(10),
                ..base_ctx()
            };
            let first_body = compose_race_body(RaceTrigger::ViceWon, &race_ctx).unwrap();
            let first_headline = "Vice vence pela primeira vez".to_string();

            let leader_ctx = leader_bad_ctx(0, Some(10), false);
            let second = compose_leader_bad_result_story(&leader_ctx);

            assert_ne!(first_headline, second.headline);
            assert_ne!(first_body, second.body);
        }

        #[test]
        fn test_second_story_body_mentions_leader_name() {
            let ctx = leader_bad_ctx(0, Some(11), false);
            let story = compose_leader_bad_result_story(&ctx);
            assert!(
                story.body.contains("Felipe Correa"),
                "body da segunda story deve mencionar o líder: {}",
                story.body
            );
        }

        #[test]
        fn test_second_story_body_mentions_ordinal_position() {
            let ctx = leader_bad_ctx(1, Some(11), false);
            let story = compose_leader_bad_result_story(&ctx);
            assert!(
                story.body.contains("11º"),
                "body deve mencionar a posição ordinal: {}",
                story.body
            );
        }

        #[test]
        fn test_second_story_dnf_body_does_not_use_ordinal() {
            let ctx = leader_bad_ctx(0, None, true);
            let story = compose_leader_bad_result_story(&ctx);
            // DNF body não deve mencionar ordinal genérico com "º"
            assert!(
                !story.body.contains("0º"),
                "body de DNF não deve mencionar 0º: {}",
                story.body
            );
        }

        #[test]
        fn test_second_story_variants_differ_between_seeds() {
            let ctx0 = leader_bad_ctx(0, Some(9), false);
            let ctx1 = leader_bad_ctx(1, Some(9), false);
            let s0 = compose_leader_bad_result_story(&ctx0);
            let s1 = compose_leader_bad_result_story(&ctx1);
            assert_ne!(
                s0.body, s1.body,
                "variante 0 e variante 1 do body devem diferir"
            );
            assert_ne!(
                s0.headline, s1.headline,
                "variante 0 e variante 1 da headline devem diferir"
            );
        }

        /// Dump de revisão — não é assertion, só imprime para leitura humana.
        /// Execute com: cargo test dump_race_double_story -- --nocapture
        #[test]
        fn dump_race_double_story_cases_for_review() {
            let cases: &[(&str, RaceStoryContext, Option<LeaderBadResultContext>)] = &[
                (
                    "vice_won_streak3 + leader_11th",
                    RaceStoryContext {
                        driver_position: Some(2),
                        win_streak: 3,
                        rival_finish_position: Some(11),
                        ..base_ctx()
                    },
                    Some(leader_bad_ctx(0, Some(11), false)),
                ),
                (
                    "lead_changed_streak1 + leader_dnf",
                    RaceStoryContext {
                        driver_position: Some(1),
                        win_streak: 1,
                        is_lead_change: true,
                        rival_dnf: true,
                        ..base_ctx()
                    },
                    Some(leader_bad_ctx(0, None, true)),
                ),
                (
                    "vice_won_streak1 + leader_8th",
                    RaceStoryContext {
                        driver_position: Some(2),
                        win_streak: 1,
                        rival_finish_position: Some(8),
                        ..base_ctx()
                    },
                    Some(leader_bad_ctx(0, Some(8), false)),
                ),
                (
                    "vice_won_no_double_story (leader 5th)",
                    RaceStoryContext {
                        driver_position: Some(2),
                        win_streak: 1,
                        rival_finish_position: Some(5),
                        ..base_ctx()
                    },
                    None,
                ),
            ];

            println!("\n=== DOUBLE STORY CASES ===");
            for (label, race_ctx, leader_ctx_opt) in cases {
                println!("\ncase={label}");
                let trigger = if race_ctx.is_lead_change {
                    RaceTrigger::LeadChanged
                } else {
                    RaceTrigger::ViceWon
                };
                let story1_body =
                    compose_race_body(trigger, race_ctx).unwrap_or_else(|| "None".to_string());
                println!("  story_1_headline: (do item.titulo)");
                println!("  story_1_body: {story1_body}");
                if let Some(lctx) = leader_ctx_opt {
                    let second = compose_leader_bad_result_story(lctx);
                    println!("  story_2_headline: {}", second.headline);
                    println!("  story_2_body: {}", second.body);
                } else {
                    println!("  (sem segunda story)");
                }
            }
        }
    }

    #[test]
    fn test_news_tab_snapshot_without_filter_prioritizes_briefing_story_over_generic_market_dump() {
        let base_dir = create_test_career_dir("news_snapshot_briefing_priority");
        seed_news_items(&base_dir, "career_001");

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "mazda_rookie".to_string(),
                scope_class: None,
                primary_filter: None,
                context_type: None,
                context_id: None,
            },
        )
        .expect("snapshot");

        assert_eq!(
            snapshot.stories.first().map(|story| story.title.as_str()),
            Some("Pressao sobe para a proxima etapa em Okayama")
        );
        assert!(
            snapshot
                .stories
                .iter()
                .all(|story| story.title != "Arquivo do mercado ainda repercute no grid"),
            "briefing edition should not devolve into a generic all-news dump",
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_time_label_uses_round_and_career_date_for_race_news() {
        let base_dir = create_test_career_dir("news_time_label_round");
        seed_news_items(&base_dir, "career_001");
        let expected_round_label =
            get_calendar_for_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
                .expect("calendar")
                .into_iter()
                .find(|entry| entry.rodada == 1)
                .and_then(|entry| format_display_date_label(&entry.display_date))
                .expect("round 1 date");

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "mazda_rookie".to_string(),
                scope_class: None,
                primary_filter: Some("Corridas".to_string()),
                context_type: None,
                context_id: None,
            },
        )
        .expect("snapshot");

        let story = snapshot
            .stories
            .iter()
            .find(|story| story.title == "Abertura em Okayama esquenta o grid")
            .expect("race story");

        assert_eq!(
            story.time_label,
            format!("Rodada 1 · {expected_round_label}")
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_snapshot_keeps_editorial_race_body_after_a_new_round() {
        let base_dir = create_test_career_dir("news_story_persists_after_next_round");
        seed_news_items(&base_dir, "career_001");
        seed_round_results_for_news_history(&base_dir, "career_001");
        advance_active_season_round(&base_dir, "career_001", 3);

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "mazda_rookie".to_string(),
                scope_class: None,
                primary_filter: Some("Corridas".to_string()),
                context_type: Some("race".to_string()),
                context_id: Some("R001".to_string()),
            },
        )
        .expect("snapshot");

        let story = snapshot
            .stories
            .iter()
            .find(|story| story.id == "NT001")
            .expect("round one race story");

        assert_ne!(
            story.body_text,
            "Uma largada intensa abriu a temporada com disputa em toda a reta."
        );
        assert!(
            story.body_text.contains("campeonato")
                || story.body_text.contains("rodada")
                || story.body_text.contains("temporada"),
            "older race story should keep editorial body instead of raw summary"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_snapshot_keeps_editorial_incident_body_after_a_new_round() {
        let base_dir = create_test_career_dir("news_incident_story_persists_after_next_round");
        seed_news_items(&base_dir, "career_001");
        seed_incident_results_for_news_history(&base_dir, "career_001");
        advance_active_season_round(&base_dir, "career_001", 3);

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "mazda_rookie".to_string(),
                scope_class: None,
                primary_filter: Some("Pilotos".to_string()),
                context_type: Some("driver".to_string()),
                context_id: Some("P001".to_string()),
            },
        )
        .expect("snapshot");

        let story = snapshot
            .stories
            .iter()
            .find(|story| story.id == "NT004")
            .expect("round one incident story");

        assert_ne!(
            story.body_text,
            "Uma quebra encerrou a prova de Thomas Baker ainda antes da metade da corrida."
        );
        assert!(
            story.body_text.contains("abandono")
                || story.body_text.contains("bandeira quadriculada"),
            "older incident story should keep abandonment framing instead of raw summary"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_snapshot_keeps_editorial_injury_body_after_a_new_round() {
        let base_dir = create_test_career_dir("news_injury_story_persists_after_next_round");
        seed_news_items(&base_dir, "career_001");
        seed_injury_news_item(&base_dir, "career_001");
        advance_active_season_round(&base_dir, "career_001", 3);
        let expected_next_race =
            get_calendar_for_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
                .expect("calendar")
                .into_iter()
                .find(|entry| entry.rodada == 2)
                .map(|entry| entry.track_name)
                .expect("round 2 race");

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "mazda_rookie".to_string(),
                scope_class: None,
                primary_filter: Some("Pilotos".to_string()),
                context_type: Some("driver".to_string()),
                context_id: Some("P001".to_string()),
            },
        )
        .expect("snapshot");

        let story = snapshot
            .stories
            .iter()
            .find(|story| story.id == "NT005")
            .expect("round one injury story");

        assert_ne!(
            story.body_text,
            "Thomas Baker esta fora da proxima etapa apos lesao confirmada. Situacao sera reavaliada nos proximos dias."
        );
        assert!(
            story.body_text.contains(&expected_next_race),
            "older injury story should keep the original next-race target instead of drifting to the current one: {}",
            story.body_text
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_snapshot_keeps_editorial_pilot_body_after_a_new_round() {
        let base_dir = create_test_career_dir("news_pilot_story_persists_after_next_round");
        seed_news_items(&base_dir, "career_001");
        seed_round_results_for_news_history(&base_dir, "career_001");
        seed_pilot_news_item(&base_dir, "career_001");
        advance_active_season_round(&base_dir, "career_001", 3);

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "mazda_rookie".to_string(),
                scope_class: None,
                primary_filter: Some("Pilotos".to_string()),
                context_type: Some("driver".to_string()),
                context_id: Some("P001".to_string()),
            },
        )
        .expect("snapshot");

        let story = snapshot
            .stories
            .iter()
            .find(|story| story.id == "NT006")
            .expect("round one pilot story");

        assert_ne!(
            story.body_text,
            "Thomas Baker ganhou moral no paddock depois de um fim de semana muito forte."
        );
        assert!(
            story.body_text.contains("campeonato")
                || story.body_text.contains("temporada")
                || story.body_text.contains("tabela"),
            "older pilot story should keep editorial body instead of raw summary"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_time_label_uses_preseason_week_and_derived_date() {
        let base_dir = create_test_career_dir("news_time_label_preseason");
        seed_news_items(&base_dir, "career_001");
        seed_preseason_news_items(&base_dir, "career_001");
        let first_round_date =
            get_calendar_for_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
                .expect("calendar")
                .into_iter()
                .find(|entry| entry.rodada == 1)
                .and_then(|entry| parse_iso_date(&entry.display_date))
                .expect("round 1 date");
        let expected_preseason_date = first_round_date - Duration::weeks(1);

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "mazda_rookie".to_string(),
                scope_class: None,
                primary_filter: Some("Mercado".to_string()),
                context_type: None,
                context_id: None,
            },
        )
        .expect("snapshot");

        let story = snapshot
            .stories
            .iter()
            .find(|story| story.title == "Semana 2 agita o mercado da Mazda")
            .expect("preseason story");

        assert_eq!(
            story.time_label,
            format!(
                "Pre-temporada Semana 2 · {}",
                format_naive_date_label(expected_preseason_date)
            )
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_snapshot_shared_scope_class_filters_everything_to_the_selected_production_class(
    ) {
        let base_dir = create_test_career_dir("news_snapshot_production_scope_class");
        seed_shared_scope_news_items(&base_dir, "career_001");

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "production_challenger".to_string(),
                scope_class: Some("mazda".to_string()),
                primary_filter: None,
                context_type: None,
                context_id: None,
            },
        )
        .expect("snapshot");

        let story_titles: Vec<&str> = snapshot
            .stories
            .iter()
            .map(|story| story.title.as_str())
            .collect();

        assert!(
            story_titles
                .iter()
                .any(|title| title.contains("Mazda lidera o ritmo da Production")),
            "o briefing precisa manter a historia da classe Mazda",
        );
        assert!(
            story_titles.iter().all(|title| !title.contains("BMW")),
            "nenhuma historia da classe BMW deve sobreviver no recorte Mazda",
        );
        assert_eq!(snapshot.scope_meta.scope_id, "production_challenger");
        assert_eq!(snapshot.scope_meta.scope_class.as_deref(), Some("mazda"));
        assert!(
            snapshot.scope_meta.scope_label.contains("Mazda"),
            "o label ativo precisa refletir a classe compartilhada",
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_news_tab_snapshot_shared_scope_class_limits_context_filters_to_the_selected_class() {
        let base_dir = create_test_career_dir("news_snapshot_production_class_filters");
        seed_shared_scope_news_items(&base_dir, "career_001");

        let snapshot = get_news_tab_snapshot_in_base_dir(
            &base_dir,
            "career_001",
            NewsTabSnapshotRequest {
                scope_type: "category".to_string(),
                scope_id: "production_challenger".to_string(),
                scope_class: Some("mazda".to_string()),
                primary_filter: Some("Equipes".to_string()),
                context_type: None,
                context_id: None,
            },
        )
        .expect("snapshot");

        let filter_labels: Vec<&str> = snapshot
            .contextual_filters
            .iter()
            .map(|filter| filter.label.as_str())
            .collect();

        assert!(
            !filter_labels.is_empty(),
            "o filtro contextual de equipes precisa continuar populado",
        );
        assert!(
            filter_labels
                .iter()
                .all(|label| !label.to_ascii_lowercase().contains("bmw")),
            "equipes BMW nao devem aparecer no recorte Mazda",
        );
        assert!(
            snapshot
                .stories
                .iter()
                .all(|story| !story.title.contains("BMW")),
            "historias da lista tambem devem respeitar o mesmo recorte",
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    fn current_team_id(base_dir: &std::path::Path, career_id: &str) -> String {
        super::load_career_in_base_dir(base_dir, career_id)
            .expect("career")
            .player_team
            .expect("player team")
            .id
    }

    fn current_team_by_class(
        base_dir: &std::path::Path,
        career_id: &str,
        category_id: &str,
        class_name: &str,
    ) -> String {
        let config = AppConfig::load_or_default(base_dir);
        let db_path = config.saves_dir().join(career_id).join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let category_for_lookup = match (category_id, class_name) {
            ("production_challenger", "mazda") => "mazda_amador",
            ("production_challenger", "toyota") => "toyota_amador",
            ("production_challenger", "bmw") => "bmw_m2",
            ("endurance", "gt4") => "gt4",
            ("endurance", "gt3") => "gt3",
            _ => category_id,
        };

        if category_for_lookup == category_id {
            team_queries::get_teams_by_category_and_class(&db.conn, category_id, class_name)
                .expect("teams by class")
                .into_iter()
                .next()
                .expect("team in class")
                .id
        } else {
            team_queries::get_teams_by_category(&db.conn, category_for_lookup)
                .expect("teams by feeder category")
                .into_iter()
                .next()
                .expect("team in feeder category")
                .id
        }
    }

    fn advance_active_season_round(base_dir: &std::path::Path, career_id: &str, round: i32) {
        let config = AppConfig::load_or_default(base_dir);
        let db_path = config.saves_dir().join(career_id).join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        season_queries::update_season_rodada(&db.conn, &season.id, round)
            .expect("update season round");
    }

    fn seed_round_results_for_news_history(base_dir: &std::path::Path, career_id: &str) {
        let config = AppConfig::load_or_default(base_dir);
        let db_path = config.saves_dir().join(career_id).join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player_team_id = current_team_id(base_dir, career_id);
        let rival_team_id = team_queries::get_all_teams(&db.conn)
            .expect("all teams")
            .into_iter()
            .find(|team| team.categoria == "mazda_rookie" && team.id != player_team_id)
            .expect("rival team")
            .id;
        let calendar = get_calendar_for_category_in_base_dir(base_dir, career_id, "mazda_rookie")
            .expect("calendar");
        let round_1_id = calendar
            .iter()
            .find(|entry| entry.rodada == 1)
            .map(|entry| entry.id.clone())
            .expect("round 1 race");
        let round_2_id = calendar
            .iter()
            .find(|entry| entry.rodada == 2)
            .map(|entry| entry.id.clone())
            .expect("round 2 race");

        for (race_id, driver_id, team_id, grid_position, finish_position, points) in [
            (&round_1_id, "P001", player_team_id.as_str(), 2, 1, 25.0_f64),
            (&round_1_id, "P002", rival_team_id.as_str(), 1, 2, 18.0_f64),
            (&round_2_id, "P001", player_team_id.as_str(), 6, 4, 12.0_f64),
            (&round_2_id, "P002", rival_team_id.as_str(), 3, 1, 25.0_f64),
        ] {
            db.conn
                .execute(
                    "INSERT INTO race_results (
                        race_id,
                        piloto_id,
                        equipe_id,
                        posicao_largada,
                        posicao_final,
                        voltas_completadas,
                        dnf,
                        pontos,
                        tempo_total,
                        fastest_lap,
                        dnf_reason,
                        dnf_segment,
                        incidents_count,
                        gap_to_winner_ms,
                        final_tire_wear,
                        dnf_catalog_id,
                        damage_origin_segment
                    ) VALUES (?1, ?2, ?3, ?4, ?5, 20, 0, ?6, 0, 0, NULL, NULL, 0, 0, 0.0, NULL, NULL)",
                    rusqlite::params![
                        race_id,
                        driver_id,
                        team_id,
                        grid_position,
                        finish_position,
                        points,
                    ],
                )
                .expect("insert race result");
        }
    }

    fn seed_incident_results_for_news_history(base_dir: &std::path::Path, career_id: &str) {
        let config = AppConfig::load_or_default(base_dir);
        let db_path = config.saves_dir().join(career_id).join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player_team_id = current_team_id(base_dir, career_id);
        let rival_team_id = team_queries::get_all_teams(&db.conn)
            .expect("all teams")
            .into_iter()
            .find(|team| team.categoria == "mazda_rookie" && team.id != player_team_id)
            .expect("rival team")
            .id;
        let calendar = get_calendar_for_category_in_base_dir(base_dir, career_id, "mazda_rookie")
            .expect("calendar");
        let round_1_id = calendar
            .iter()
            .find(|entry| entry.rodada == 1)
            .map(|entry| entry.id.clone())
            .expect("round 1 race");
        let round_2_id = calendar
            .iter()
            .find(|entry| entry.rodada == 2)
            .map(|entry| entry.id.clone())
            .expect("round 2 race");

        for (
            race_id,
            driver_id,
            team_id,
            grid_position,
            finish_position,
            is_dnf,
            points,
            dnf_segment,
            dnf_catalog_id,
        ) in [
            (
                &round_1_id,
                "P001",
                player_team_id.as_str(),
                4,
                18,
                true,
                0.0_f64,
                Some("Mid"),
                Some("SB_S_MEC_01"),
            ),
            (
                &round_1_id,
                "P002",
                rival_team_id.as_str(),
                1,
                1,
                false,
                25.0_f64,
                None,
                None,
            ),
            (
                &round_2_id,
                "P001",
                player_team_id.as_str(),
                3,
                2,
                false,
                18.0_f64,
                None,
                None,
            ),
            (
                &round_2_id,
                "P002",
                rival_team_id.as_str(),
                2,
                1,
                false,
                25.0_f64,
                None,
                None,
            ),
        ] {
            db.conn
                .execute(
                    "INSERT INTO race_results (
                        race_id,
                        piloto_id,
                        equipe_id,
                        posicao_largada,
                        posicao_final,
                        voltas_completadas,
                        dnf,
                        pontos,
                        tempo_total,
                        fastest_lap,
                        dnf_reason,
                        dnf_segment,
                        incidents_count,
                        gap_to_winner_ms,
                        final_tire_wear,
                        dnf_catalog_id,
                        damage_origin_segment
                    ) VALUES (?1, ?2, ?3, ?4, ?5, 12, ?6, ?7, 0, 0, NULL, ?8, 1, 0, 0.0, ?9, ?8)",
                    rusqlite::params![
                        race_id,
                        driver_id,
                        team_id,
                        grid_position,
                        finish_position,
                        if is_dnf { 1 } else { 0 },
                        points,
                        dnf_segment,
                        dnf_catalog_id,
                    ],
                )
                .expect("insert incident race result");
        }
    }

    fn seed_injury_news_item(base_dir: &std::path::Path, career_id: &str) {
        let config = AppConfig::load_or_default(base_dir);
        let db_path = config.saves_dir().join(career_id).join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");

        news_queries::insert_news_batch(
            &db.conn,
            &vec![NewsItem {
                id: "NT005".to_string(),
                tipo: NewsType::Lesao,
                icone: "L".to_string(),
                titulo: "desfalque confirmado".to_string(),
                texto: "Thomas Baker esta fora da proxima etapa apos lesao confirmada. Situacao sera reavaliada nos proximos dias.".to_string(),
                rodada: Some(1),
                semana_pretemporada: None,
                temporada: season.numero,
                categoria_id: Some("mazda_rookie".to_string()),
                categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                importancia: NewsImportance::Alta,
                timestamp: 160,
                driver_id: Some("P001".to_string()),
                driver_id_secondary: None,
                team_id: None,
            }],
        )
        .expect("insert injury news");
    }

    fn seed_pilot_news_item(base_dir: &std::path::Path, career_id: &str) {
        let config = AppConfig::load_or_default(base_dir);
        let db_path = config.saves_dir().join(career_id).join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");

        news_queries::insert_news_batch(
            &db.conn,
            &vec![NewsItem {
                id: "NT006".to_string(),
                tipo: NewsType::Hierarquia,
                icone: "P".to_string(),
                titulo: "Thomas Baker cresce internamente depois da abertura".to_string(),
                texto:
                    "Thomas Baker ganhou moral no paddock depois de um fim de semana muito forte."
                        .to_string(),
                rodada: Some(1),
                semana_pretemporada: None,
                temporada: season.numero,
                categoria_id: Some("mazda_rookie".to_string()),
                categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                importancia: NewsImportance::Alta,
                timestamp: 170,
                driver_id: Some("P001".to_string()),
                driver_id_secondary: None,
                team_id: None,
            }],
        )
        .expect("insert pilot news");
    }

    fn seed_news_items(base_dir: &std::path::Path, career_id: &str) {
        let config = AppConfig::load_or_default(base_dir);
        let db_path = config.saves_dir().join(career_id).join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        let team_id = current_team_id(base_dir, career_id);

        news_queries::insert_news_batch(
            &db.conn,
            &vec![
                NewsItem {
                    id: "NT000".to_string(),
                    tipo: NewsType::FramingSazonal,
                    icone: "F".to_string(),
                    titulo: "Pressao sobe para a proxima etapa em Okayama".to_string(),
                    texto: "A disputa chega aquecida para a proxima rodada, com tensao crescente entre os nomes do topo.".to_string(),
                    rodada: Some(1),
                    semana_pretemporada: None,
                    temporada: season.numero,
                    categoria_id: Some("mazda_rookie".to_string()),
                    categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                    importancia: NewsImportance::Alta,
                    timestamp: 275,
                    driver_id: Some("P001".to_string()),
                    driver_id_secondary: None,
                    team_id: None,
                },
                NewsItem {
                    id: "NT001".to_string(),
                    tipo: NewsType::Corrida,
                    icone: "R".to_string(),
                    titulo: "Abertura em Okayama esquenta o grid".to_string(),
                    texto: "Uma largada intensa abriu a temporada com disputa em toda a reta.".to_string(),
                    rodada: Some(1),
                    semana_pretemporada: None,
                    temporada: season.numero,
                    categoria_id: Some("mazda_rookie".to_string()),
                    categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                    importancia: NewsImportance::Alta,
                    timestamp: 100,
                    driver_id: Some("P001".to_string()),
                    driver_id_secondary: None,
                    team_id: Some(team_id.clone()),
                },
                NewsItem {
                    id: "NT002".to_string(),
                    tipo: NewsType::Mercado,
                    icone: "M".to_string(),
                    titulo: "A equipe do jogador observa reforcos".to_string(),
                    texto: "O paddock comenta uma movimentacao de mercado ao redor da equipe.".to_string(),
                    rodada: Some(2),
                    semana_pretemporada: None,
                    temporada: season.numero,
                    categoria_id: Some("mazda_rookie".to_string()),
                    categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                    importancia: NewsImportance::Destaque,
                    timestamp: 200,
                    driver_id: Some("P002".to_string()),
                    driver_id_secondary: None,
                    team_id: Some(team_id.clone()),
                },
                NewsItem {
                    id: "NT002B".to_string(),
                    tipo: NewsType::Mercado,
                    icone: "M".to_string(),
                    titulo: "Arquivo do mercado ainda repercute no grid".to_string(),
                    texto: "Um rumor antigo segue circulando, mas ja sem impacto claro sobre a proxima corrida.".to_string(),
                    rodada: Some(0),
                    semana_pretemporada: None,
                    temporada: season.numero,
                    categoria_id: Some("mazda_rookie".to_string()),
                    categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                    importancia: NewsImportance::Destaque,
                    timestamp: 260,
                    driver_id: Some("P002".to_string()),
                    driver_id_secondary: None,
                    team_id: None,
                },
                NewsItem {
                    id: "NT003".to_string(),
                    tipo: NewsType::Rivalidade,
                    icone: "V".to_string(),
                    titulo: "Thomas Baker e Kenji Sato entram em rota de colisao".to_string(),
                    texto: "O duelo direto em pista comeca a definir o tom do campeonato.".to_string(),
                    rodada: Some(1),
                    semana_pretemporada: None,
                    temporada: season.numero,
                    categoria_id: Some("mazda_rookie".to_string()),
                    categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                    importancia: NewsImportance::Alta,
                    timestamp: 300,
                    driver_id: Some("P001".to_string()),
                    driver_id_secondary: Some("P002".to_string()),
                    team_id: None,
                },
                NewsItem {
                    id: "NT004".to_string(),
                    tipo: NewsType::Incidente,
                    icone: "I".to_string(),
                    titulo: "Thomas Baker abandona a corrida apos quebra".to_string(),
                    texto: "Uma quebra encerrou a prova de Thomas Baker ainda antes da metade da corrida.".to_string(),
                    rodada: Some(1),
                    semana_pretemporada: None,
                    temporada: season.numero,
                    categoria_id: Some("mazda_rookie".to_string()),
                    categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                    importancia: NewsImportance::Alta,
                    timestamp: 150,
                    driver_id: Some("P001".to_string()),
                    driver_id_secondary: None,
                    team_id: Some(team_id.clone()),
                },
            ],
        )
        .expect("insert news");
    }

    fn seed_shared_scope_news_items(base_dir: &std::path::Path, career_id: &str) {
        let config = AppConfig::load_or_default(base_dir);
        let db_path = config.saves_dir().join(career_id).join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        news_queries::delete_all_news(&db.conn).expect("clear baseline news");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        let mazda_team_id =
            current_team_by_class(base_dir, career_id, "production_challenger", "mazda");
        let bmw_team_id =
            current_team_by_class(base_dir, career_id, "production_challenger", "bmw");

        news_queries::insert_news_batch(
            &db.conn,
            &vec![
                NewsItem {
                    id: "SP001".to_string(),
                    tipo: NewsType::FramingSazonal,
                    icone: "P".to_string(),
                    titulo: "Mazda lidera o ritmo da Production".to_string(),
                    texto: "A ala Mazda chega forte ao fim de semana e puxa a conversa do paddock."
                        .to_string(),
                    rodada: Some(2),
                    semana_pretemporada: None,
                    temporada: season.numero,
                    categoria_id: Some("production_challenger".to_string()),
                    categoria_nome: Some("Production Car Challenger".to_string()),
                    importancia: NewsImportance::Destaque,
                    timestamp: 320,
                    driver_id: None,
                    driver_id_secondary: None,
                    team_id: Some(mazda_team_id.clone()),
                },
                NewsItem {
                    id: "SP002".to_string(),
                    tipo: NewsType::Corrida,
                    icone: "B".to_string(),
                    titulo: "BMW domina o treino da Production".to_string(),
                    texto: "A frente BMW abriu vantagem no treino compartilhado.".to_string(),
                    rodada: Some(2),
                    semana_pretemporada: None,
                    temporada: season.numero,
                    categoria_id: Some("production_challenger".to_string()),
                    categoria_nome: Some("Production Car Challenger".to_string()),
                    importancia: NewsImportance::Alta,
                    timestamp: 310,
                    driver_id: None,
                    driver_id_secondary: None,
                    team_id: Some(bmw_team_id.clone()),
                },
                NewsItem {
                    id: "SP003".to_string(),
                    tipo: NewsType::Mercado,
                    icone: "M".to_string(),
                    titulo: "Equipe Mazda acelera conversa de mercado na Production".to_string(),
                    texto: "A classe Mazda discute reforcos antes da proxima etapa multiclasses."
                        .to_string(),
                    rodada: Some(2),
                    semana_pretemporada: None,
                    temporada: season.numero,
                    categoria_id: Some("production_challenger".to_string()),
                    categoria_nome: Some("Production Car Challenger".to_string()),
                    importancia: NewsImportance::Alta,
                    timestamp: 300,
                    driver_id: None,
                    driver_id_secondary: None,
                    team_id: Some(mazda_team_id),
                },
            ],
        )
        .expect("insert shared scope news");
    }

    fn seed_preseason_news_items(base_dir: &std::path::Path, career_id: &str) {
        let config = AppConfig::load_or_default(base_dir);
        let db_path = config.saves_dir().join(career_id).join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");

        news_queries::insert_news_batch(
            &db.conn,
            &vec![NewsItem {
                id: "NTP001".to_string(),
                tipo: NewsType::PreTemporada,
                icone: "P".to_string(),
                titulo: "Semana 2 agita o mercado da Mazda".to_string(),
                texto: "A segunda semana da pre-temporada acelera rumores e definicoes no paddock."
                    .to_string(),
                rodada: None,
                semana_pretemporada: Some(2),
                temporada: season.numero,
                categoria_id: Some("mazda_rookie".to_string()),
                categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                importancia: NewsImportance::Alta,
                timestamp: 450,
                driver_id: None,
                driver_id_secondary: None,
                team_id: None,
            }],
        )
        .expect("insert preseason news");
    }

    fn create_test_career_dir(label: &str) -> std::path::PathBuf {
        let base_dir = unique_test_dir(label);
        fs::create_dir_all(&base_dir).expect("base dir");

        let input = CreateCareerInput {
            player_name: "Joao Silva".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(22),
            category: "mazda_rookie".to_string(),
            team_index: 2,
            difficulty: "medio".to_string(),
        };

        let _ = create_career_in_base_dir(&base_dir, input).expect("career should be created");
        base_dir
    }

    fn unique_test_dir(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("iracerapp_news_tab_{label}_{nanos}"))
    }

    // ── Parte 12: sistema editorial de Incidente ──────────────────────────────

    mod incident_editorial_tests {
        use super::super::{
            compose_incident_body, compose_incident_headline, detect_incident_trigger,
            is_mechanical_incident, IncidentStoryContext, IncidentTrigger,
        };
        use crate::news::NewsImportance;

        fn ctx(
            driver: Option<&str>,
            secondary: Option<&str>,
            is_mechanical: bool,
            is_still_open: bool,
        ) -> IncidentStoryContext {
            IncidentStoryContext {
                driver_name: driver.map(|s| s.to_string()),
                secondary_driver_name: secondary.map(|s| s.to_string()),
                category_name: Some("GT4 Challenge".to_string()),
                is_mechanical,
                is_still_open,
                is_dnf: false,
                segment: None,
                item_seed: 0,
            }
        }

        // ── Trigger detection ─────────────────────────────────────────────────

        #[test]
        fn test_two_driver_fires_when_secondary_present_alta() {
            assert_eq!(
                detect_incident_trigger(
                    &ctx(Some("Rodrigo"), Some("Marcelo"), false, false),
                    &NewsImportance::Alta
                ),
                IncidentTrigger::TwoDriverIncident
            );
        }

        #[test]
        fn test_two_driver_fires_for_destaque() {
            assert_eq!(
                detect_incident_trigger(
                    &ctx(Some("Rodrigo"), Some("Marcelo"), false, false),
                    &NewsImportance::Destaque
                ),
                IncidentTrigger::TwoDriverIncident
            );
        }

        #[test]
        fn test_driver_incident_damage_fires_for_alta_with_driver_only() {
            assert_eq!(
                detect_incident_trigger(
                    &ctx(Some("Rodrigo"), None, false, false),
                    &NewsImportance::Alta
                ),
                IncidentTrigger::DriverIncidentDamage
            );
        }

        #[test]
        fn test_mechanical_fires_when_flag_set() {
            assert_eq!(
                detect_incident_trigger(
                    &ctx(Some("Rodrigo"), None, true, false),
                    &NewsImportance::Alta
                ),
                IncidentTrigger::MechanicalFailureHitStrongly
            );
        }

        #[test]
        fn test_two_driver_beats_mechanical_flag() {
            // Secondary presente → TwoDriverIncident tem prioridade
            assert_eq!(
                detect_incident_trigger(
                    &ctx(Some("Rodrigo"), Some("Marcelo"), true, false),
                    &NewsImportance::Alta
                ),
                IncidentTrigger::TwoDriverIncident
            );
        }

        #[test]
        fn test_still_open_fires_when_flag_set_and_no_secondary() {
            assert_eq!(
                detect_incident_trigger(
                    &ctx(Some("Rodrigo"), None, false, true),
                    &NewsImportance::Alta
                ),
                IncidentTrigger::IncidentStillOpen
            );
        }

        #[test]
        fn test_mechanical_beats_still_open() {
            // Mecânico tem prioridade sobre still_open
            assert_eq!(
                detect_incident_trigger(
                    &ctx(Some("Rodrigo"), None, true, true),
                    &NewsImportance::Alta
                ),
                IncidentTrigger::MechanicalFailureHitStrongly
            );
        }

        #[test]
        fn test_fallback_for_baixa_importance() {
            assert_eq!(
                detect_incident_trigger(
                    &ctx(Some("Rodrigo"), Some("Marcelo"), false, false),
                    &NewsImportance::Baixa
                ),
                IncidentTrigger::FallbackIncidentStory
            );
        }

        #[test]
        fn test_fallback_for_media_importance() {
            assert_eq!(
                detect_incident_trigger(
                    &ctx(Some("Rodrigo"), None, false, false),
                    &NewsImportance::Media
                ),
                IncidentTrigger::FallbackIncidentStory
            );
        }

        // ── Body composition ──────────────────────────────────────────────────

        #[test]
        fn test_driver_damage_body_mentions_driver() {
            let c = ctx(Some("Rodrigo"), None, false, false);
            let body = compose_incident_body(IncidentTrigger::DriverIncidentDamage, &c);
            assert!(
                body.contains("Rodrigo"),
                "body deve mencionar o piloto: {body}"
            );
        }

        #[test]
        fn test_two_driver_body_mentions_both_drivers() {
            let c = ctx(Some("Rodrigo"), Some("Marcelo"), false, false);
            let body = compose_incident_body(IncidentTrigger::TwoDriverIncident, &c);
            assert!(
                body.contains("Rodrigo"),
                "body deve mencionar o piloto principal: {body}"
            );
            assert!(
                body.contains("Marcelo"),
                "body deve mencionar o piloto secundário: {body}"
            );
        }

        #[test]
        fn test_mechanical_body_mentions_driver() {
            let c = ctx(Some("Rodrigo"), None, true, false);
            let body = compose_incident_body(IncidentTrigger::MechanicalFailureHitStrongly, &c);
            assert!(
                body.contains("Rodrigo"),
                "body de mecânico deve mencionar o piloto: {body}"
            );
        }

        #[test]
        fn test_still_open_body_mentions_driver() {
            let c = ctx(Some("Rodrigo"), None, false, true);
            let body = compose_incident_body(IncidentTrigger::IncidentStillOpen, &c);
            assert!(
                body.contains("Rodrigo"),
                "body de caso aberto deve mencionar o piloto: {body}"
            );
        }

        #[test]
        fn test_all_triggers_have_two_body_variants() {
            let triggers = [
                IncidentTrigger::DriverIncidentDamage,
                IncidentTrigger::TwoDriverIncident,
                IncidentTrigger::MechanicalFailureHitStrongly,
                IncidentTrigger::IncidentStillOpen,
            ];
            for trigger in &triggers {
                let c0 = IncidentStoryContext {
                    item_seed: 0,
                    ..ctx(Some("Rodrigo"), Some("Marcelo"), true, true)
                };
                let c1 = IncidentStoryContext {
                    item_seed: 1,
                    ..ctx(Some("Rodrigo"), Some("Marcelo"), true, true)
                };
                let b0 = compose_incident_body(*trigger, &c0);
                let b1 = compose_incident_body(*trigger, &c1);
                assert_ne!(
                    b0, b1,
                    "{trigger:?}: variante seed=0 e seed=1 devem diferir"
                );
            }
        }

        #[test]
        fn test_body_uses_generic_name_when_no_driver() {
            let c = ctx(None, None, false, false);
            // Com Baixa, fallback — mas body direto não tem guardas de trigger
            let body = compose_incident_body(IncidentTrigger::DriverIncidentDamage, &c);
            assert!(!body.is_empty(), "body genérico não deve estar vazio");
        }

        // ── Headline composition ──────────────────────────────────────────────

        #[test]
        fn test_headline_mentions_driver_for_driver_damage() {
            let c = ctx(Some("Rodrigo"), None, false, false);
            let h = compose_incident_headline(IncidentTrigger::DriverIncidentDamage, &c);
            assert!(
                h.contains("Rodrigo"),
                "headline deve mencionar o piloto: {h}"
            );
        }

        #[test]
        fn test_headline_mentions_both_drivers_for_two_driver() {
            let c = ctx(Some("Rodrigo"), Some("Marcelo"), false, false);
            let h = compose_incident_headline(IncidentTrigger::TwoDriverIncident, &c);
            assert!(
                h.contains("Rodrigo"),
                "headline deve mencionar piloto principal: {h}"
            );
            assert!(
                h.contains("Marcelo"),
                "headline deve mencionar piloto secundário: {h}"
            );
        }

        #[test]
        fn test_headline_differs_from_plain_driver_name() {
            // A headline editorial é concreta, não é só o nome
            let c = ctx(Some("Rodrigo"), None, false, false);
            let h = compose_incident_headline(IncidentTrigger::DriverIncidentDamage, &c);
            assert_ne!(h, "Rodrigo");
            assert!(!h.is_empty());
        }

        #[test]
        fn test_headline_has_two_variants() {
            let triggers = [
                IncidentTrigger::DriverIncidentDamage,
                IncidentTrigger::TwoDriverIncident,
                IncidentTrigger::MechanicalFailureHitStrongly,
                IncidentTrigger::IncidentStillOpen,
            ];
            for trigger in &triggers {
                let c0 = IncidentStoryContext {
                    item_seed: 0,
                    ..ctx(Some("Rodrigo"), Some("Marcelo"), true, true)
                };
                let c1 = IncidentStoryContext {
                    item_seed: 1,
                    ..ctx(Some("Rodrigo"), Some("Marcelo"), true, true)
                };
                let h0 = compose_incident_headline(*trigger, &c0);
                let h1 = compose_incident_headline(*trigger, &c1);
                assert_ne!(
                    h0, h1,
                    "{trigger:?}: headline seed=0 e seed=1 devem diferir"
                );
            }
        }

        // ── Heurística mecânica ───────────────────────────────────────────────

        #[test]
        fn test_is_mechanical_fires_for_quebra_in_titulo() {
            assert!(is_mechanical_incident(
                "Motor quebrou na reta final",
                "",
                false
            ));
        }

        #[test]
        fn test_is_mechanical_fires_for_motor_in_texto() {
            assert!(is_mechanical_incident(
                "",
                "Rodrigo abandona com problema no motor",
                false
            ));
        }

        #[test]
        fn test_is_mechanical_fires_for_pane() {
            assert!(is_mechanical_incident(
                "Pane elétrica elimina piloto",
                "",
                false
            ));
        }

        #[test]
        fn test_is_mechanical_fires_for_falha_mec_prefix() {
            assert!(is_mechanical_incident(
                "",
                "falha mecânica destrói chance de pódio",
                false
            ));
        }

        #[test]
        fn test_is_mechanical_does_not_fire_with_secondary_driver() {
            // Se há piloto secundário, é colisão, não mecânico
            assert!(!is_mechanical_incident(
                "Motor quebrou na batida",
                "quebra após toque",
                true
            ));
        }

        #[test]
        fn test_is_mechanical_does_not_fire_for_generic_text() {
            assert!(!is_mechanical_incident(
                "Incidente na curva 3",
                "piloto sai de pista",
                false
            ));
        }

        #[test]
        fn test_mechanical_trigger_fires_via_heuristic_in_detection() {
            // Contexto com is_mechanical=true → trigger deve ser MechanicalFailureHitStrongly
            let c = ctx(Some("Rodrigo"), None, true, false);
            assert_eq!(
                detect_incident_trigger(&c, &NewsImportance::Alta),
                IncidentTrigger::MechanicalFailureHitStrongly
            );
        }

        /// Dump de revisão — não é assertion, só imprime para leitura humana.
        /// Execute com: cargo test dump_incident_primary_triggers -- --nocapture
        #[test]
        fn dump_incident_primary_triggers_for_review() {
            let cases: &[(&str, IncidentTrigger, IncidentStoryContext)] = &[
                (
                    "driver_damage_alta",
                    IncidentTrigger::DriverIncidentDamage,
                    ctx(Some("Rodrigo"), None, false, false),
                ),
                (
                    "two_driver_alta",
                    IncidentTrigger::TwoDriverIncident,
                    ctx(Some("Rodrigo"), Some("Marcelo"), false, false),
                ),
                (
                    "mechanical_via_heuristic",
                    IncidentTrigger::MechanicalFailureHitStrongly,
                    ctx(Some("Rodrigo"), None, true, false),
                ),
                (
                    "still_open_alta",
                    IncidentTrigger::IncidentStillOpen,
                    ctx(Some("Rodrigo"), None, false, true),
                ),
            ];

            println!("\n=== INCIDENT PRIMARY TRIGGERS ===");
            for (label, trigger, c) in cases {
                let headline = compose_incident_headline(*trigger, c);
                let body = compose_incident_body(*trigger, c);
                println!("case={label}");
                println!("headline: {headline}");
                println!("body: {body}");
                println!();
            }
        }
    }

    // ── Parte 10: sistema editorial de Mercado ────────────────────────────────

    mod incident_facts_tests {
        use super::super::{is_mechanical_incident, IncidentFactType};

        #[test]
        fn test_mechanical_heuristic_matches_motor() {
            assert!(is_mechanical_incident(
                "Problema no motor afasta piloto",
                "",
                false
            ));
        }

        #[test]
        fn test_mechanical_heuristic_negative() {
            assert!(!is_mechanical_incident(
                "Toque entre pilotos",
                "sairam de pista",
                false
            ));
        }

        #[test]
        fn test_two_car_overrides_mechanical_heuristic() {
            assert!(!is_mechanical_incident(
                "quebra no cambio",
                "outro piloto envolvido",
                true
            ));
        }

        #[test]
        fn test_factual_mechanical_beats_no_keyword() {
            let fact_type = Some(IncidentFactType::Mechanical);
            let heuristic_would_say = false;
            let result = match fact_type {
                Some(IncidentFactType::Mechanical) => true,
                Some(_) => false,
                None => heuristic_would_say,
            };
            assert!(result);
        }

        #[test]
        fn test_factual_driver_error_overrides_mechanical_keyword() {
            let fact_type = Some(IncidentFactType::DriverError);
            let heuristic_would_say = true;
            let result = match fact_type {
                Some(IncidentFactType::Mechanical) => true,
                Some(_) => false,
                None => heuristic_would_say,
            };
            assert!(
                !result,
                "DriverError factual deve anular heuristica mecanica"
            );
        }

        #[test]
        fn test_no_factual_falls_back_to_heuristic() {
            let fact_type: Option<IncidentFactType> = None;
            let heuristic_would_say = true;
            let result = match fact_type {
                Some(IncidentFactType::Mechanical) => true,
                Some(_) => false,
                None => heuristic_would_say,
            };
            assert!(result, "None factual deve usar heuristica");
        }
    }

    mod incident_modifier_tests {
        use super::super::{
            compose_incident_body, compose_incident_headline,
            compose_incident_modifier_phrase_polished, detect_incident_modifiers, IncidentModifier,
            IncidentStoryContext, IncidentTrigger,
        };

        fn ctx(
            is_dnf: bool,
            segment: Option<&str>,
            is_mechanical: bool,
            has_secondary: bool,
        ) -> IncidentStoryContext {
            IncidentStoryContext {
                driver_name: Some("Rodrigo".to_string()),
                secondary_driver_name: has_secondary.then(|| "Marcelo".to_string()),
                category_name: Some("GT4 Challenge".to_string()),
                is_mechanical,
                is_still_open: false,
                is_dnf,
                segment: segment.map(|s| s.to_string()),
                item_seed: 0,
            }
        }

        #[test]
        fn test_detect_incident_modifiers_prioritizes_dnf_then_late() {
            let mods = detect_incident_modifiers(
                &ctx(true, Some("Late"), true, false),
                IncidentTrigger::MechanicalFailureHitStrongly,
            );
            assert_eq!(
                mods,
                vec![
                    IncidentModifier::IncidentCausedDnf,
                    IncidentModifier::LateRaceHit
                ]
            );
        }

        #[test]
        fn test_detect_incident_modifiers_uses_mid_when_present() {
            let mods = detect_incident_modifiers(
                &ctx(false, Some("Mid"), false, false),
                IncidentTrigger::DriverIncidentDamage,
            );
            assert_eq!(mods, vec![IncidentModifier::MidRaceHit]);
        }

        #[test]
        fn test_detect_incident_modifiers_respects_two_modifier_cap() {
            let mods = detect_incident_modifiers(
                &ctx(true, Some("Late"), false, true),
                IncidentTrigger::TwoDriverIncident,
            );
            assert!(
                mods.len() <= 2,
                "incidente nunca deve exceder dois modificadores: {mods:?}"
            );
        }

        #[test]
        fn test_detect_incident_modifiers_empty_when_no_factual_context() {
            let mods = detect_incident_modifiers(
                &ctx(false, None, false, false),
                IncidentTrigger::DriverIncidentDamage,
            );
            assert!(mods.is_empty());
        }

        #[test]
        fn test_incident_modifier_phrases_have_two_variants() {
            let c = ctx(true, Some("Late"), true, false);
            for modifier in [
                IncidentModifier::IncidentCausedDnf,
                IncidentModifier::LateRaceHit,
                IncidentModifier::MidRaceHit,
            ] {
                let v0 = compose_incident_modifier_phrase_polished(
                    modifier,
                    IncidentTrigger::DriverIncidentDamage,
                    &c,
                    0,
                );
                let v1 = compose_incident_modifier_phrase_polished(
                    modifier,
                    IncidentTrigger::DriverIncidentDamage,
                    &c,
                    1,
                );
                assert_ne!(v0, v1, "{modifier:?}: variante 0 e 1 devem diferir");
            }
        }

        #[test]
        fn test_two_driver_dnf_uses_contextual_modifier_phrase() {
            let phrase = compose_incident_modifier_phrase_polished(
                IncidentModifier::IncidentCausedDnf,
                IncidentTrigger::TwoDriverIncident,
                &ctx(true, None, false, true),
                0,
            );
            assert!(
                phrase.contains("toque") && phrase.contains("dois nomes"),
                "two-driver + dnf deve usar frase contextual ao caso entre dois nomes: {phrase}"
            );
        }

        #[test]
        fn test_driver_damage_variant_can_frame_next_round_response() {
            let body = compose_incident_body(
                IncidentTrigger::DriverIncidentDamage,
                &IncidentStoryContext {
                    item_seed: 2,
                    ..ctx(false, None, false, false)
                },
            );
            assert!(
                body.contains("proxima rodada") || body.contains("resposta"),
                "driver damage deve poder apontar resposta futura concreta: {body}"
            );
        }

        #[test]
        fn test_midrace_modifier_can_turn_timing_into_survival_context() {
            let phrase = compose_incident_modifier_phrase_polished(
                IncidentModifier::MidRaceHit,
                IncidentTrigger::DriverIncidentDamage,
                &ctx(false, Some("Mid"), false, false),
                1,
            );
            assert!(
                phrase.contains("sobrevivencia") || phrase.contains("prejuizo"),
                "mid-race deve fazer mais do que informar timing: {phrase}"
            );
        }

        #[test]
        fn test_compose_incident_body_appends_dnf_and_late_modifiers() {
            let body = compose_incident_body(
                IncidentTrigger::MechanicalFailureHitStrongly,
                &ctx(true, Some("Late"), true, false),
            );
            assert!(
                body.contains("A quebra tirou de Rodrigo uma corrida que ainda estava viva"),
                "body deve preservar o principal: {body}"
            );
            assert!(
                body.contains("bandeira quadriculada"),
                "body deve incluir o modificador de DNF: {body}"
            );
            assert!(
                body.contains("tarde demais"),
                "body deve incluir o modificador de timing tardio: {body}"
            );
        }

        #[test]
        fn test_compose_incident_body_appends_midrace_modifier() {
            let body = compose_incident_body(
                IncidentTrigger::DriverIncidentDamage,
                &ctx(false, Some("Mid"), false, false),
            );
            assert!(
                body.contains("meio da prova") || body.contains("alguma corrida pela frente"),
                "body deve incluir nuance de incidente no meio da prova: {body}"
            );
        }

        /// Dump de revisao - nao e assertion, so imprime para leitura humana.
        /// Execute com: cargo test dump_incident_modifier_cases_for_review -- --nocapture
        #[test]
        fn dump_incident_modifier_cases_for_review() {
            let cases: &[(&str, IncidentTrigger, IncidentStoryContext)] = &[
                (
                    "driver_damage_dnf",
                    IncidentTrigger::DriverIncidentDamage,
                    ctx(true, None, false, false),
                ),
                (
                    "mechanical_late",
                    IncidentTrigger::MechanicalFailureHitStrongly,
                    ctx(false, Some("Late"), true, false),
                ),
                (
                    "driver_damage_mid",
                    IncidentTrigger::DriverIncidentDamage,
                    ctx(false, Some("Mid"), false, false),
                ),
                (
                    "two_driver_plain",
                    IncidentTrigger::TwoDriverIncident,
                    ctx(false, None, false, true),
                ),
                (
                    "two_driver_dnf",
                    IncidentTrigger::TwoDriverIncident,
                    ctx(true, None, false, true),
                ),
            ];

            println!("\n=== INCIDENT MODIFIER CASES ===");
            for (label, trigger, c) in cases {
                let headline = compose_incident_headline(*trigger, c);
                let body = compose_incident_body(*trigger, c);
                println!();
                println!("case={label}");
                println!("headline: {headline}");
                println!("body: {body}");
            }
        }
    }

    mod market_editorial_tests {
        use super::super::{
            compose_market_body, compose_market_headline, compose_market_modifier_phrase,
            detect_market_modifiers, detect_market_trigger, MarketModifier, MarketStoryContext,
            MarketTrigger,
        };
        use crate::news::NewsImportance;

        fn ctx(driver: Option<&str>, team: Option<&str>, is_preseason: bool) -> MarketStoryContext {
            MarketStoryContext {
                driver_name: driver.map(|s| s.to_string()),
                team_name: team.map(|s| s.to_string()),
                category_name: Some("GT4 Challenge".to_string()),
                is_preseason,
                item_seed: 0,
                preseason_week: if is_preseason { Some(3) } else { None },
                presence_tier: None,
                subject_is_driver: driver.is_some(),
                subject_is_team: team.is_some() && driver.is_none(),
            }
        }

        fn ctx_mod(
            driver: Option<&str>,
            team: Option<&str>,
            is_preseason: bool,
            week: Option<i32>,
            tier: Option<&str>,
        ) -> MarketStoryContext {
            MarketStoryContext {
                preseason_week: week,
                presence_tier: tier.map(|s| s.to_string()),
                ..ctx(driver, team, is_preseason)
            }
        }

        // ── Trigger detection ─────────────────────────────────────────────────

        #[test]
        fn test_market_driver_heated_fires_for_alta_with_driver() {
            assert_eq!(
                detect_market_trigger(&ctx(Some("Rodrigo"), None, false), &NewsImportance::Alta),
                MarketTrigger::MarketHeatedAroundDriver
            );
        }

        #[test]
        fn test_market_team_heated_fires_for_alta_with_team_only() {
            assert_eq!(
                detect_market_trigger(
                    &ctx(None, Some("Equipe Solaris"), false),
                    &NewsImportance::Alta
                ),
                MarketTrigger::MarketHeatedAroundTeam
            );
        }

        #[test]
        fn test_driver_trigger_beats_team_when_both_present() {
            // driver_name presente → MarketHeatedAroundDriver tem prioridade sobre Team
            assert_eq!(
                detect_market_trigger(
                    &ctx(Some("Rodrigo"), Some("Equipe Solaris"), false),
                    &NewsImportance::Alta
                ),
                MarketTrigger::MarketHeatedAroundDriver
            );
        }

        #[test]
        fn test_concrete_move_fires_for_destaque_non_preseason() {
            assert_eq!(
                detect_market_trigger(
                    &ctx(Some("Rodrigo"), None, false),
                    &NewsImportance::Destaque
                ),
                MarketTrigger::ConcreteMoveUnderway
            );
        }

        #[test]
        fn test_concrete_move_fires_for_destaque_with_team() {
            assert_eq!(
                detect_market_trigger(
                    &ctx(None, Some("Equipe Solaris"), false),
                    &NewsImportance::Destaque
                ),
                MarketTrigger::ConcreteMoveUnderway
            );
        }

        #[test]
        fn test_preseason_pressure_fires_for_alta_in_preseason() {
            assert_eq!(
                detect_market_trigger(&ctx(Some("Rodrigo"), None, true), &NewsImportance::Alta),
                MarketTrigger::PreseasonMarketPressure
            );
        }

        #[test]
        fn test_preseason_takes_precedence_over_concrete_move() {
            // Destaque + pré-temporada → PreseasonMarketPressure ganha
            assert_eq!(
                detect_market_trigger(&ctx(Some("Rodrigo"), None, true), &NewsImportance::Destaque),
                MarketTrigger::PreseasonMarketPressure
            );
        }

        #[test]
        fn test_fallback_for_low_importance() {
            assert_eq!(
                detect_market_trigger(&ctx(Some("Rodrigo"), None, false), &NewsImportance::Baixa),
                MarketTrigger::FallbackMarketStory
            );
        }

        #[test]
        fn test_fallback_for_alta_without_driver_or_team() {
            assert_eq!(
                detect_market_trigger(&ctx(None, None, false), &NewsImportance::Alta),
                MarketTrigger::FallbackMarketStory
            );
        }

        // ── Body composition ──────────────────────────────────────────────────

        #[test]
        fn test_driver_heated_body_mentions_driver_name() {
            let c = ctx(Some("Rodrigo"), None, false);
            let body = compose_market_body(MarketTrigger::MarketHeatedAroundDriver, &c);
            assert!(
                body.contains("Rodrigo"),
                "body deve mencionar o piloto: {body}"
            );
        }

        #[test]
        fn test_team_heated_body_mentions_team_name() {
            let c = ctx(None, Some("Equipe Solaris"), false);
            let body = compose_market_body(MarketTrigger::MarketHeatedAroundTeam, &c);
            assert!(
                body.contains("Equipe Solaris"),
                "body deve mencionar a equipe: {body}"
            );
        }

        #[test]
        fn test_concrete_move_body_mentions_subject_when_driver_present() {
            let c = ctx(Some("Rodrigo"), None, false);
            let body = compose_market_body(MarketTrigger::ConcreteMoveUnderway, &c);
            assert!(
                body.contains("Rodrigo"),
                "body de ConcreteMoveUnderway deve mencionar o piloto: {body}"
            );
        }

        #[test]
        fn test_preseason_body_mentions_subject_when_driver_present() {
            let c = ctx(Some("Rodrigo"), None, true);
            let body = compose_market_body(MarketTrigger::PreseasonMarketPressure, &c);
            assert!(
                body.contains("Rodrigo"),
                "body de PreseasonMarketPressure deve mencionar o piloto: {body}"
            );
        }

        #[test]
        fn test_all_triggers_have_two_body_variants() {
            let triggers = [
                MarketTrigger::MarketHeatedAroundDriver,
                MarketTrigger::MarketHeatedAroundTeam,
                MarketTrigger::ConcreteMoveUnderway,
                MarketTrigger::PreseasonMarketPressure,
            ];
            for trigger in &triggers {
                let c0 = MarketStoryContext {
                    item_seed: 0,
                    ..ctx(Some("Rodrigo"), Some("Equipe Solaris"), false)
                };
                let c1 = MarketStoryContext {
                    item_seed: 1,
                    ..ctx(Some("Rodrigo"), Some("Equipe Solaris"), false)
                };
                let b0 = compose_market_body(*trigger, &c0);
                let b1 = compose_market_body(*trigger, &c1);
                assert_ne!(
                    b0, b1,
                    "{trigger:?}: variante seed=0 e seed=1 devem diferir"
                );
            }
        }

        #[test]
        fn test_concrete_move_body_is_not_empty_without_subject() {
            let c = ctx(None, None, false);
            let body = compose_market_body(MarketTrigger::ConcreteMoveUnderway, &c);
            assert!(
                !body.is_empty(),
                "ConcreteMoveUnderway deve gerar texto mesmo sem driver/team"
            );
        }

        #[test]
        fn test_preseason_body_is_not_empty_without_subject() {
            let c = ctx(None, None, true);
            let body = compose_market_body(MarketTrigger::PreseasonMarketPressure, &c);
            assert!(
                !body.is_empty(),
                "PreseasonMarketPressure deve gerar texto mesmo sem driver/team"
            );
        }

        // ── Parte 11: modificadores de Mercado ───────────────────────────────

        #[test]
        fn test_driver_headline_mentions_driver_name() {
            let c = ctx(Some("Rodrigo"), None, false);
            let headline = compose_market_headline(MarketTrigger::MarketHeatedAroundDriver, &c);
            assert!(
                headline.contains("Rodrigo"),
                "headline deve mencionar o piloto: {headline}"
            );
        }

        #[test]
        fn test_team_headline_mentions_team_name() {
            let c = ctx(None, Some("Equipe Solaris"), false);
            let headline = compose_market_headline(MarketTrigger::MarketHeatedAroundTeam, &c);
            assert!(
                headline.contains("Equipe Solaris"),
                "headline deve mencionar a equipe: {headline}"
            );
        }

        #[test]
        fn test_headline_has_two_variants_for_each_market_trigger() {
            let triggers = [
                MarketTrigger::MarketHeatedAroundDriver,
                MarketTrigger::MarketHeatedAroundTeam,
                MarketTrigger::ConcreteMoveUnderway,
                MarketTrigger::PreseasonMarketPressure,
            ];
            for trigger in &triggers {
                let c0 = MarketStoryContext {
                    item_seed: 0,
                    ..ctx(Some("Rodrigo"), Some("Equipe Solaris"), false)
                };
                let c1 = MarketStoryContext {
                    item_seed: 1,
                    ..ctx(Some("Rodrigo"), Some("Equipe Solaris"), false)
                };
                let h0 = compose_market_headline(*trigger, &c0);
                let h1 = compose_market_headline(*trigger, &c1);
                assert_ne!(h0, h1, "{trigger:?}: variante 0 e 1 devem diferir");
            }
        }

        #[test]
        fn test_preseason_phase_fires_for_non_preseason_trigger() {
            // ConcreteMoveUnderway em pré-temporada → PreseasonPhaseContext dispara
            let c = ctx_mod(Some("Rodrigo"), None, true, Some(3), None);
            let mods = detect_market_modifiers(&c, MarketTrigger::ConcreteMoveUnderway);
            assert!(mods.contains(&MarketModifier::PreseasonPhaseContext));
        }

        #[test]
        fn test_preseason_phase_excluded_when_trigger_is_preseason() {
            // PreseasonMarketPressure já fala de pré-temporada → modificador não deve duplicar
            let c = ctx_mod(Some("Rodrigo"), None, true, Some(3), None);
            let mods = detect_market_modifiers(&c, MarketTrigger::PreseasonMarketPressure);
            assert!(!mods.contains(&MarketModifier::PreseasonPhaseContext));
        }

        #[test]
        fn test_driver_centered_fires_for_concrete_move_with_driver() {
            let c = ctx_mod(Some("Rodrigo"), None, false, None, None);
            let mods = detect_market_modifiers(&c, MarketTrigger::ConcreteMoveUnderway);
            assert!(mods.contains(&MarketModifier::DriverCenteredMove));
        }

        #[test]
        fn test_driver_centered_excluded_when_trigger_is_driver_heated() {
            // MarketHeatedAroundDriver já centra no piloto → DriverCenteredMove seria redundante
            let c = ctx_mod(Some("Rodrigo"), None, false, None, None);
            let mods = detect_market_modifiers(&c, MarketTrigger::MarketHeatedAroundDriver);
            assert!(!mods.contains(&MarketModifier::DriverCenteredMove));
        }

        #[test]
        fn test_team_centered_fires_for_concrete_move_with_team() {
            let c = ctx_mod(None, Some("Equipe Solaris"), false, None, None);
            let mods = detect_market_modifiers(&c, MarketTrigger::ConcreteMoveUnderway);
            assert!(mods.contains(&MarketModifier::TeamCenteredMove));
        }

        #[test]
        fn test_team_centered_excluded_when_trigger_is_team_heated() {
            let c = ctx_mod(None, Some("Equipe Solaris"), false, None, None);
            let mods = detect_market_modifiers(&c, MarketTrigger::MarketHeatedAroundTeam);
            assert!(!mods.contains(&MarketModifier::TeamCenteredMove));
        }

        #[test]
        fn test_public_presence_fires_for_alta_tier() {
            // DriverHeated (sujeito excluído) + alta presença → PublicPresenceContext dispara
            let c = ctx_mod(Some("Rodrigo"), None, false, None, Some("alta"));
            let mods = detect_market_modifiers(&c, MarketTrigger::MarketHeatedAroundDriver);
            assert!(mods.contains(&MarketModifier::PublicPresenceContext));
        }

        #[test]
        fn test_public_presence_fires_for_elite_tier() {
            let c = ctx_mod(Some("Rodrigo"), None, false, None, Some("elite"));
            let mods = detect_market_modifiers(&c, MarketTrigger::MarketHeatedAroundDriver);
            assert!(mods.contains(&MarketModifier::PublicPresenceContext));
        }

        #[test]
        fn test_public_presence_does_not_fire_for_baixa_tier() {
            let c = ctx_mod(Some("Rodrigo"), None, false, None, Some("baixa"));
            let mods = detect_market_modifiers(&c, MarketTrigger::MarketHeatedAroundDriver);
            assert!(!mods.contains(&MarketModifier::PublicPresenceContext));
        }

        #[test]
        fn test_needs_followup_fires_as_fecho_when_no_higher_priority() {
            // DriverHeated sem presença → DriverCenteredMove excluído, sem presença → NeedsConcreteFollowUp
            let c = ctx_mod(Some("Rodrigo"), None, false, None, None);
            let mods = detect_market_modifiers(&c, MarketTrigger::MarketHeatedAroundDriver);
            assert!(mods.contains(&MarketModifier::NeedsConcreteFollowUp));
        }

        #[test]
        fn test_max_two_market_modifiers() {
            // Tudo ativo: pré-temporada, driver, alta presença → ≤ 2
            let c = ctx_mod(Some("Rodrigo"), None, true, Some(4), Some("elite"));
            let mods = detect_market_modifiers(&c, MarketTrigger::ConcreteMoveUnderway);
            assert!(
                mods.len() <= 2,
                "nunca deve exceder 2 modificadores: {mods:?}"
            );
        }

        #[test]
        fn test_market_modifier_phrases_have_two_variants() {
            let c = ctx_mod(
                Some("Rodrigo"),
                Some("Equipe Solaris"),
                true,
                Some(4),
                Some("alta"),
            );
            let modifiers = [
                MarketModifier::PreseasonPhaseContext,
                MarketModifier::DriverCenteredMove,
                MarketModifier::TeamCenteredMove,
                MarketModifier::PublicPresenceContext,
                MarketModifier::NeedsConcreteFollowUp,
            ];
            for modifier in &modifiers {
                let v0 = compose_market_modifier_phrase(*modifier, &c, 0);
                let v1 = compose_market_modifier_phrase(*modifier, &c, 1);
                assert_ne!(v0, v1, "{modifier:?}: variante 0 e 1 devem diferir");
            }
        }

        #[test]
        fn test_preseason_phase_phrase_changes_with_week() {
            let early = ctx_mod(None, None, true, Some(1), None);
            let mid = ctx_mod(None, None, true, Some(3), None);
            let late = ctx_mod(None, None, true, Some(6), None);
            let phrases: Vec<String> = [&early, &mid, &late]
                .iter()
                .map(|c| {
                    compose_market_modifier_phrase(MarketModifier::PreseasonPhaseContext, c, 0)
                })
                .collect();
            assert_ne!(
                phrases[0], phrases[1],
                "semana 1 e 3 devem gerar frases diferentes"
            );
            assert_ne!(
                phrases[1], phrases[2],
                "semana 3 e 6 devem gerar frases diferentes"
            );
        }

        #[test]
        fn test_composed_body_includes_modifier_for_concrete_move_with_driver() {
            // ConcreteMoveUnderway + driver → DriverCenteredMove aparece no body final
            let c = MarketStoryContext {
                item_seed: 0,
                ..ctx_mod(Some("Rodrigo"), None, false, None, None)
            };
            let body = compose_market_body(MarketTrigger::ConcreteMoveUnderway, &c);
            // DriverCenteredMove v0: "A fase recente ajuda..."
            assert!(
                body.contains("fase recente")
                    || body.contains("aposta lateral")
                    || body.contains("desempenho e mercado"),
                "body deve conter frase de DriverCenteredMove: {body}"
            );
        }

        /// Dump de revisão — não é assertion, só imprime para leitura humana.
        /// Execute com: cargo test dump_market_modifiers -- --nocapture
        #[test]
        fn dump_market_modifiers_for_review() {
            let cases: &[(&str, MarketTrigger, MarketStoryContext)] = &[
                (
                    "driver_heated + alta_presence",
                    MarketTrigger::MarketHeatedAroundDriver,
                    ctx_mod(Some("Rodrigo"), None, false, None, Some("alta")),
                ),
                (
                    "team_heated + no_presence",
                    MarketTrigger::MarketHeatedAroundTeam,
                    ctx_mod(None, Some("Equipe Solaris"), false, None, None),
                ),
                (
                    "concrete_move + driver + preseason_mid",
                    MarketTrigger::ConcreteMoveUnderway,
                    ctx_mod(Some("Rodrigo"), None, true, Some(3), None),
                ),
                (
                    "concrete_move + team + elite_presence",
                    MarketTrigger::ConcreteMoveUnderway,
                    ctx_mod(None, Some("Equipe Solaris"), false, None, Some("elite")),
                ),
                (
                    "preseason_pressure + driver + late_week",
                    MarketTrigger::PreseasonMarketPressure,
                    ctx_mod(Some("Rodrigo"), None, true, Some(6), None),
                ),
            ];

            println!("\n=== MARKET MODIFIER REVIEW ===");
            for (label, trigger, c) in cases {
                let body = compose_market_body(*trigger, c);
                println!("case={label}");
                println!("body: {body}");
                println!();
            }
        }

        /// Dump de revisão — não é assertion, só imprime para leitura humana.
        /// Execute com: cargo test dump_market_primary_triggers -- --nocapture
        #[test]
        fn dump_market_primary_triggers_for_review() {
            let cases: &[(&str, MarketTrigger, MarketStoryContext)] = &[
                (
                    "driver_heated",
                    MarketTrigger::MarketHeatedAroundDriver,
                    ctx(Some("Rodrigo"), None, false),
                ),
                (
                    "team_heated",
                    MarketTrigger::MarketHeatedAroundTeam,
                    ctx(None, Some("Equipe Solaris"), false),
                ),
                (
                    "concrete_move_driver",
                    MarketTrigger::ConcreteMoveUnderway,
                    ctx(Some("Rodrigo"), None, false),
                ),
                (
                    "concrete_move_team",
                    MarketTrigger::ConcreteMoveUnderway,
                    ctx(None, Some("Equipe Solaris"), false),
                ),
                (
                    "preseason_pressure_driver",
                    MarketTrigger::PreseasonMarketPressure,
                    ctx(Some("Rodrigo"), None, true),
                ),
                (
                    "preseason_pressure_no_subject",
                    MarketTrigger::PreseasonMarketPressure,
                    ctx(None, None, true),
                ),
            ];

            println!("\n=== MARKET PRIMARY TRIGGERS ===");
            for (label, trigger, c) in cases {
                let body = compose_market_body(*trigger, c);
                let headline = compose_market_headline(*trigger, c);
                println!("case={label}");
                println!("headline: {headline}");
                println!("body: {body}");
                println!();
            }
        }
    }

    // ── Parte 8: sistema editorial de Piloto ─────────────────────────────────

    mod pilot_editorial_tests {
        use super::super::{
            compose_pilot_body, compose_pilot_headline, compose_pilot_modifier_phrase,
            detect_pilot_modifiers, detect_pilot_trigger, PilotModifier, PilotStoryContext,
            PilotTrigger, TOP_TABLE_GAP_THRESHOLD,
        };
        use crate::news::NewsImportance;

        fn ctx(position: Option<i32>, win_streak: u32) -> PilotStoryContext {
            PilotStoryContext {
                driver_name: "Rafael Medina".to_string(),
                category_name: "GT4 Challenge".to_string(),
                driver_position: position,
                win_streak,
                item_seed: 0,
                last_race_finish: None,
                last_race_dnf: false,
                points_gap_to_leader: None,
            }
        }

        #[test]
        fn test_pilot_in_strong_form_destaque_streak2() {
            assert_eq!(
                detect_pilot_trigger(&ctx(Some(1), 2), &NewsImportance::Destaque),
                PilotTrigger::PilotInStrongForm
            );
        }

        #[test]
        fn test_pilot_in_strong_form_alta_top3() {
            assert_eq!(
                detect_pilot_trigger(&ctx(Some(3), 0), &NewsImportance::Alta),
                PilotTrigger::PilotInStrongForm
            );
        }

        #[test]
        fn test_pilot_under_pressure_for_baixa() {
            assert_eq!(
                detect_pilot_trigger(&ctx(Some(1), 5), &NewsImportance::Baixa),
                PilotTrigger::PilotUnderPressure
            );
        }

        #[test]
        fn test_pilot_under_pressure_for_media() {
            assert_eq!(
                detect_pilot_trigger(&ctx(Some(2), 3), &NewsImportance::Media),
                PilotTrigger::PilotUnderPressure
            );
        }

        #[test]
        fn test_pilot_became_relevant_alta_midfield() {
            assert_eq!(
                detect_pilot_trigger(&ctx(Some(6), 0), &NewsImportance::Alta),
                PilotTrigger::PilotBecameRelevant
            );
        }

        #[test]
        fn test_pilot_momentum_shift_destaque_no_streak() {
            assert_eq!(
                detect_pilot_trigger(&ctx(Some(5), 0), &NewsImportance::Destaque),
                PilotTrigger::PilotMomentumShift
            );
        }

        #[test]
        fn test_pilot_fallback_alta_no_position() {
            assert_eq!(
                detect_pilot_trigger(&ctx(None, 0), &NewsImportance::Alta),
                PilotTrigger::FallbackPilotStory
            );
        }

        #[test]
        fn test_pilot_fallback_alta_position_beyond_8() {
            assert_eq!(
                detect_pilot_trigger(&ctx(Some(9), 0), &NewsImportance::Alta),
                PilotTrigger::FallbackPilotStory
            );
        }

        #[test]
        fn test_pilot_body_mentions_driver_for_all_non_fallback_triggers() {
            let base = ctx(Some(1), 2);
            for trigger in [
                PilotTrigger::PilotInStrongForm,
                PilotTrigger::PilotUnderPressure,
                PilotTrigger::PilotBecameRelevant,
                PilotTrigger::PilotMomentumShift,
            ] {
                let body = compose_pilot_body(trigger, &base);
                assert!(
                    body.contains("Rafael Medina"),
                    "{trigger:?}: body deve mencionar o piloto: {body}"
                );
            }
        }

        #[test]
        fn test_pilot_body_has_at_least_two_variants() {
            let ctx0 = PilotStoryContext {
                item_seed: 0,
                ..ctx(Some(1), 2)
            };
            let ctx1 = PilotStoryContext {
                item_seed: 1,
                ..ctx(Some(1), 2)
            };
            for trigger in [
                PilotTrigger::PilotInStrongForm,
                PilotTrigger::PilotUnderPressure,
                PilotTrigger::PilotBecameRelevant,
                PilotTrigger::PilotMomentumShift,
            ] {
                let b0 = compose_pilot_body(trigger, &ctx0);
                let b1 = compose_pilot_body(trigger, &ctx1);
                assert_ne!(
                    b0, b1,
                    "{trigger:?}: variante seed=0 e seed=1 devem diferir"
                );
            }
        }

        /// Dump de revisão — não é assertion, só imprime para leitura humana.
        #[test]
        fn test_pilot_headline_mentions_driver_for_all_non_fallback_triggers() {
            let base = ctx(Some(1), 2);
            for trigger in [
                PilotTrigger::PilotInStrongForm,
                PilotTrigger::PilotUnderPressure,
                PilotTrigger::PilotBecameRelevant,
                PilotTrigger::PilotMomentumShift,
            ] {
                let headline = compose_pilot_headline(trigger, &base);
                assert!(
                    headline.contains("Rafael Medina"),
                    "{trigger:?}: headline deve mencionar o piloto: {headline}"
                );
            }
        }

        #[test]
        fn test_pilot_headline_has_at_least_two_variants() {
            let ctx0 = PilotStoryContext {
                item_seed: 0,
                ..ctx(Some(1), 2)
            };
            let ctx1 = PilotStoryContext {
                item_seed: 1,
                ..ctx(Some(1), 2)
            };
            for trigger in [
                PilotTrigger::PilotInStrongForm,
                PilotTrigger::PilotUnderPressure,
                PilotTrigger::PilotBecameRelevant,
                PilotTrigger::PilotMomentumShift,
            ] {
                let h0 = compose_pilot_headline(trigger, &ctx0);
                let h1 = compose_pilot_headline(trigger, &ctx1);
                assert_ne!(
                    h0, h1,
                    "{trigger:?}: variante seed=0 e seed=1 devem diferir"
                );
            }
        }

        #[test]
        fn test_pilot_strong_form_headline_uses_individual_editorial_voice() {
            let ctx0 = PilotStoryContext {
                item_seed: 0,
                ..ctx(Some(1), 2)
            };
            let ctx1 = PilotStoryContext {
                item_seed: 1,
                ..ctx(Some(1), 2)
            };

            assert_eq!(
                compose_pilot_headline(PilotTrigger::PilotInStrongForm, &ctx0),
                "Rafael Medina entra em fase forte e se aproxima da disputa principal"
            );
            assert_eq!(
                compose_pilot_headline(PilotTrigger::PilotInStrongForm, &ctx1),
                "Rafael Medina ganha tracao pessoal e sobe de vez na conversa do campeonato"
            );
        }

        #[test]
        fn test_pilot_strong_form_body_moves_away_from_weight_language() {
            let body = compose_pilot_body(
                PilotTrigger::PilotInStrongForm,
                &PilotStoryContext {
                    item_seed: 0,
                    ..ctx_mod(Some(1), 3, Some(2), false, Some(0))
                },
            );

            assert!(
                !body.contains("peso na temporada"),
                "PilotInStrongForm nao deve depender de linguagem de peso abstrato: {body}"
            );
            assert!(
                body.contains("ritmo") || body.contains("trajetoria") || body.contains("patamar"),
                "PilotInStrongForm deve soar mais pessoal e menos institucional: {body}"
            );
        }

        #[test]
        fn test_pilot_strong_form_body_v1_stays_out_of_momentumshift_zone() {
            let body = compose_pilot_body(
                PilotTrigger::PilotInStrongForm,
                &PilotStoryContext {
                    item_seed: 1,
                    ..ctx(Some(3), 0)
                },
            );
            assert!(
                !body.contains("ritmo diferente"),
                "PilotInStrongForm v1 nao deve soar como virada — zona semantica do MomentumShift: {body}"
            );
        }

        #[test]
        fn test_pilot_momentum_shift_headline_avoids_leitura() {
            let headline = compose_pilot_headline(
                PilotTrigger::PilotMomentumShift,
                &PilotStoryContext {
                    item_seed: 1,
                    ..ctx(Some(4), 0)
                },
            );
            assert!(
                !headline.contains("leitura"),
                "PilotMomentumShift headline nao deve usar campo de 'leitura' (saturado em Lesao e Team): {headline}"
            );
        }

        /// Execute com: cargo test dump_pilot_primary_triggers -- --nocapture
        #[test]
        fn dump_pilot_primary_triggers_for_review() {
            let cases: &[(&str, PilotTrigger, PilotStoryContext)] = &[
                (
                    "strong_form destaque streak3",
                    PilotTrigger::PilotInStrongForm,
                    PilotStoryContext {
                        item_seed: 0,
                        ..ctx(Some(1), 3)
                    },
                ),
                (
                    "under_pressure media p2",
                    PilotTrigger::PilotUnderPressure,
                    PilotStoryContext {
                        item_seed: 0,
                        ..ctx(Some(2), 0)
                    },
                ),
                (
                    "became_relevant alta p7",
                    PilotTrigger::PilotBecameRelevant,
                    PilotStoryContext {
                        item_seed: 0,
                        ..ctx(Some(7), 0)
                    },
                ),
                (
                    "momentum_shift destaque p4",
                    PilotTrigger::PilotMomentumShift,
                    PilotStoryContext {
                        item_seed: 0,
                        ..ctx(Some(4), 0)
                    },
                ),
            ];
            println!("\n=== PILOT PRIMARY TRIGGERS ===");
            for (label, trigger, c) in cases {
                let body = compose_pilot_body(*trigger, c);
                let headline = compose_pilot_headline(*trigger, c);
                println!("\ncase={label}");
                println!("headline: {headline}");
                println!("body: {body}");
            }
        }

        // ── Parte 9: modificadores de Piloto ─────────────────────────────────

        fn ctx_mod(
            position: Option<i32>,
            win_streak: u32,
            last_finish: Option<i32>,
            dnf: bool,
            gap: Option<i32>,
        ) -> PilotStoryContext {
            PilotStoryContext {
                last_race_finish: last_finish,
                last_race_dnf: dnf,
                points_gap_to_leader: gap,
                ..ctx(position, win_streak)
            }
        }

        #[test]
        fn test_driver_position_context_fires_for_top8() {
            let c = ctx_mod(Some(3), 0, None, false, None);
            let mods = detect_pilot_modifiers(&c, PilotTrigger::PilotInStrongForm);
            assert!(mods.contains(&PilotModifier::DriverPositionContext));
        }

        #[test]
        fn test_driver_position_context_does_not_fire_beyond_8() {
            let c = ctx_mod(Some(9), 0, None, false, None);
            let mods = detect_pilot_modifiers(&c, PilotTrigger::PilotInStrongForm);
            assert!(!mods.contains(&PilotModifier::DriverPositionContext));
        }

        #[test]
        fn test_recent_good_run_fires_on_win_streak() {
            // position > 8 → DriverPositionContext não dispara; RecentGoodRun deve aparecer
            let c = ctx_mod(Some(9), 2, None, false, None);
            let mods = detect_pilot_modifiers(&c, PilotTrigger::PilotInStrongForm);
            assert!(mods.contains(&PilotModifier::RecentGoodRun));
        }

        #[test]
        fn test_recent_good_run_fires_on_top5_last_race() {
            let c = ctx_mod(Some(9), 0, Some(4), false, None);
            let mods = detect_pilot_modifiers(&c, PilotTrigger::PilotInStrongForm);
            assert!(mods.contains(&PilotModifier::RecentGoodRun));
        }

        #[test]
        fn test_recent_bad_run_fires_for_under_pressure_with_bad_last_race() {
            let c = ctx_mod(Some(7), 0, Some(12), false, None);
            let mods = detect_pilot_modifiers(&c, PilotTrigger::PilotUnderPressure);
            assert!(mods.contains(&PilotModifier::RecentBadRun));
        }

        #[test]
        fn test_recent_bad_run_fires_for_under_pressure_with_dnf() {
            let c = ctx_mod(Some(5), 0, None, true, None);
            let mods = detect_pilot_modifiers(&c, PilotTrigger::PilotUnderPressure);
            assert!(mods.contains(&PilotModifier::RecentBadRun));
        }

        #[test]
        fn test_need_immediate_response_fires_alone_when_no_bad_run() {
            // Sem bad run → RecentBadRun pula, NeedImmediateResponse ocupa o slot
            let c = ctx_mod(Some(3), 0, None, false, None);
            let mods = detect_pilot_modifiers(&c, PilotTrigger::PilotUnderPressure);
            assert!(mods.contains(&PilotModifier::NeedImmediateResponse));
            assert!(!mods.contains(&PilotModifier::RecentBadRun));
        }

        #[test]
        fn test_top_table_proximity_fires_when_close_to_leader() {
            // position ≤ 5 e gap ≤ threshold → TopTableProximity no segundo slot
            let c = ctx_mod(Some(4), 0, None, false, Some(TOP_TABLE_GAP_THRESHOLD));
            let mods = detect_pilot_modifiers(&c, PilotTrigger::PilotMomentumShift);
            assert!(mods.contains(&PilotModifier::TopTableProximity));
        }

        #[test]
        fn test_top_table_proximity_does_not_fire_when_gap_too_large() {
            let c = ctx_mod(Some(3), 0, None, false, Some(TOP_TABLE_GAP_THRESHOLD + 1));
            let mods = detect_pilot_modifiers(&c, PilotTrigger::PilotInStrongForm);
            assert!(!mods.contains(&PilotModifier::TopTableProximity));
        }

        #[test]
        fn test_max_two_pilot_modifiers() {
            // Tudo ativo: position P3, gap pequeno, win_streak, good last race → ≤ 2
            let c = ctx_mod(Some(3), 3, Some(2), false, Some(10));
            let mods = detect_pilot_modifiers(&c, PilotTrigger::PilotInStrongForm);
            assert!(
                mods.len() <= 2,
                "nunca deve exceder 2 modificadores: {mods:?}"
            );
        }

        #[test]
        fn test_under_pressure_max_two_modifiers() {
            // bad run + NeedImmediateResponse ≤ 2
            let c = ctx_mod(Some(6), 0, Some(15), true, None);
            let mods = detect_pilot_modifiers(&c, PilotTrigger::PilotUnderPressure);
            assert!(
                mods.len() <= 2,
                "pressão: nunca deve exceder 2 modificadores: {mods:?}"
            );
        }

        #[test]
        fn test_positive_trigger_does_not_get_bad_run_modifier() {
            // PilotInStrongForm não deve receber RecentBadRun nem NeedImmediateResponse
            let c = ctx_mod(Some(9), 0, Some(13), true, None);
            let mods = detect_pilot_modifiers(&c, PilotTrigger::PilotInStrongForm);
            assert!(!mods.contains(&PilotModifier::RecentBadRun));
            assert!(!mods.contains(&PilotModifier::NeedImmediateResponse));
        }

        #[test]
        fn test_modifier_phrases_have_two_variants() {
            let c = ctx_mod(Some(3), 1, Some(3), false, Some(15));
            let modifiers = [
                PilotModifier::DriverPositionContext,
                PilotModifier::RecentGoodRun,
                PilotModifier::RecentBadRun,
                PilotModifier::TopTableProximity,
                PilotModifier::NeedImmediateResponse,
            ];
            for modifier in &modifiers {
                let v0 = compose_pilot_modifier_phrase(*modifier, &c, 0);
                let v1 = compose_pilot_modifier_phrase(*modifier, &c, 1);
                assert_ne!(v0, v1, "{modifier:?}: variante 0 e 1 devem diferir");
            }
        }

        #[test]
        fn test_composed_body_includes_modifier_text() {
            // PilotBecameRelevant com position=7 → DriverPositionContext deve aparecer no body
            let c = PilotStoryContext {
                item_seed: 0,
                ..ctx_mod(Some(7), 0, None, false, None)
            };
            let body = compose_pilot_body(PilotTrigger::PilotBecameRelevant, &c);
            assert!(
                body.contains("7º"),
                "body deve mencionar a posição do piloto via modificador: {body}"
            );
        }

        /// Dump de revisão — não é assertion, só imprime para leitura humana.
        /// Execute com: cargo test dump_pilot_modifiers -- --nocapture
        #[test]
        fn dump_pilot_modifiers_for_review() {
            let cases: &[(&str, PilotTrigger, PilotStoryContext)] = &[
                (
                    "strong_form + 3rd_place + good_run",
                    PilotTrigger::PilotInStrongForm,
                    ctx_mod(Some(3), 2, Some(2), false, Some(15)),
                ),
                (
                    "strong_form + 1st_place + near_top",
                    PilotTrigger::PilotInStrongForm,
                    ctx_mod(Some(1), 3, Some(1), false, Some(0)),
                ),
                (
                    "became_relevant + 6th_place + good_last_race",
                    PilotTrigger::PilotBecameRelevant,
                    ctx_mod(Some(6), 0, Some(4), false, None),
                ),
                (
                    "momentum_shift + 4th_place + proximity",
                    PilotTrigger::PilotMomentumShift,
                    ctx_mod(Some(4), 0, Some(3), false, Some(20)),
                ),
                (
                    "under_pressure + 8th_place + bad_last_race",
                    PilotTrigger::PilotUnderPressure,
                    ctx_mod(Some(8), 0, Some(13), false, None),
                ),
                (
                    "under_pressure + dnf + need_response",
                    PilotTrigger::PilotUnderPressure,
                    ctx_mod(Some(5), 0, None, true, None),
                ),
                (
                    "under_pressure + no_bad_run + need_response_only",
                    PilotTrigger::PilotUnderPressure,
                    ctx_mod(Some(3), 0, None, false, None),
                ),
            ];

            println!("\n=== PILOT MODIFIER REVIEW ===");
            for (label, trigger, c) in cases {
                let body = compose_pilot_body(*trigger, c);
                println!("case={label}");
                println!("body: {body}");
                println!();
            }
        }
    }

    mod team_editorial_tests {
        use super::super::{
            compose_team_body, compose_team_headline, detect_team_modifiers, detect_team_trigger,
            TeamModifier, TeamStoryContext, TeamTrigger,
        };
        use crate::news::NewsImportance;

        fn ctx(
            position: Option<i32>,
            points: Option<i32>,
            presence_tier: Option<&str>,
            next_race_label: Option<&str>,
        ) -> TeamStoryContext {
            TeamStoryContext {
                team_name: "Equipe Solaris".to_string(),
                category_name: Some("GT4 Challenge".to_string()),
                team_position: position,
                team_points: points,
                presence_tier: presence_tier.map(|s| s.to_string()),
                next_race_label: next_race_label.map(|s| s.to_string()),
                item_seed: 0,
            }
        }

        #[test]
        fn test_team_in_strong_moment_for_top_team_with_high_importance() {
            assert_eq!(
                detect_team_trigger(
                    &ctx(Some(2), Some(88), Some("alta"), Some("Laguna Seca")),
                    &NewsImportance::Destaque
                ),
                TeamTrigger::TeamInStrongMoment
            );
        }

        #[test]
        fn test_team_became_relevant_for_midfield_team_with_high_importance() {
            assert_eq!(
                detect_team_trigger(
                    &ctx(Some(6), Some(41), Some("relevante"), Some("Laguna Seca")),
                    &NewsImportance::Alta
                ),
                TeamTrigger::TeamBecameRelevant
            );
        }

        #[test]
        fn test_team_under_pressure_for_top_team_without_high_importance() {
            assert_eq!(
                detect_team_trigger(
                    &ctx(Some(3), Some(72), Some("alta"), Some("Laguna Seca")),
                    &NewsImportance::Media
                ),
                TeamTrigger::TeamUnderPressure
            );
        }

        #[test]
        fn test_team_lost_ground_for_high_presence_team_outside_front_group() {
            assert_eq!(
                detect_team_trigger(
                    &ctx(Some(7), Some(35), Some("elite"), Some("Laguna Seca")),
                    &NewsImportance::Media
                ),
                TeamTrigger::TeamLostGround
            );
        }

        #[test]
        fn test_team_fallback_when_context_is_too_thin() {
            assert_eq!(
                detect_team_trigger(&ctx(None, None, None, None), &NewsImportance::Baixa),
                TeamTrigger::FallbackTeamStory
            );
        }

        #[test]
        fn test_team_body_mentions_team_in_all_non_fallback_triggers() {
            let c = ctx(Some(2), Some(88), Some("alta"), Some("Laguna Seca"));
            for trigger in [
                TeamTrigger::TeamInStrongMoment,
                TeamTrigger::TeamUnderPressure,
                TeamTrigger::TeamBecameRelevant,
                TeamTrigger::TeamLostGround,
            ] {
                let body = compose_team_body(trigger, &c);
                assert!(
                    body.contains("Equipe Solaris"),
                    "{trigger:?}: body deve mencionar a equipe: {body}"
                );
            }
        }

        #[test]
        fn test_team_lost_ground_body_avoids_pressure_language() {
            let body = compose_team_body(
                TeamTrigger::TeamLostGround,
                &TeamStoryContext {
                    item_seed: 0,
                    ..ctx(Some(7), Some(35), Some("elite"), Some("Laguna Seca"))
                },
            );
            assert!(
                !body.contains("responder"),
                "TeamLostGround nao deve soar como UnderPressure: {body}"
            );
            assert!(
                body.contains("cedeu espaco")
                    || body.contains("perdeu terreno")
                    || body.contains("menos peso")
                    || body.contains("menos forca esportiva"),
                "TeamLostGround deve comunicar recuo perceptivel: {body}"
            );
        }

        #[test]
        fn test_top_table_context_fires_for_strong_moment_top_team() {
            let mods = detect_team_modifiers(
                &ctx(Some(2), Some(88), Some("relevante"), Some("Laguna Seca")),
                TeamTrigger::TeamInStrongMoment,
            );
            assert!(mods.contains(&TeamModifier::TopTableContext));
        }

        #[test]
        fn test_need_immediate_response_fires_for_under_pressure() {
            let mods = detect_team_modifiers(
                &ctx(Some(3), Some(72), Some("alta"), Some("Laguna Seca")),
                TeamTrigger::TeamUnderPressure,
            );
            assert!(mods.contains(&TeamModifier::NeedImmediateResponse));
        }

        #[test]
        fn test_growing_public_weight_fires_for_relevant_team_with_high_presence() {
            let mods = detect_team_modifiers(
                &ctx(Some(6), Some(41), Some("alta"), Some("Laguna Seca")),
                TeamTrigger::TeamBecameRelevant,
            );
            assert!(mods.contains(&TeamModifier::GrowingPublicWeight));
        }

        #[test]
        fn test_team_modifiers_never_exceed_two() {
            let mods = detect_team_modifiers(
                &ctx(Some(2), Some(88), Some("elite"), Some("Laguna Seca")),
                TeamTrigger::TeamInStrongMoment,
            );
            assert!(
                mods.len() <= 2,
                "Equipe nunca deve exceder 2 modificadores: {mods:?}"
            );
        }

        #[test]
        fn test_team_body_with_modifiers_is_longer_than_without_modifiers() {
            let with_modifiers = compose_team_body(
                TeamTrigger::TeamBecameRelevant,
                &ctx(Some(6), Some(41), Some("alta"), Some("Laguna Seca")),
            );
            let without_modifiers = compose_team_body(
                TeamTrigger::TeamBecameRelevant,
                &ctx(Some(6), Some(41), Some("relevante"), None),
            );
            assert!(
                with_modifiers.len() > without_modifiers.len(),
                "body com modificadores deve ser maior que body sem modificadores"
            );
        }

        /// Dump de revisão editorial de Equipe.
        #[test]
        fn test_team_headline_mentions_team_for_all_non_fallback_triggers() {
            let c = ctx(Some(2), Some(88), Some("alta"), Some("Laguna Seca"));
            for trigger in [
                TeamTrigger::TeamInStrongMoment,
                TeamTrigger::TeamUnderPressure,
                TeamTrigger::TeamBecameRelevant,
                TeamTrigger::TeamLostGround,
            ] {
                let headline = compose_team_headline(trigger, &c);
                assert!(
                    headline.contains("Equipe Solaris"),
                    "{trigger:?}: headline deve mencionar a equipe: {headline}"
                );
            }
        }

        #[test]
        fn test_team_headline_has_two_variants() {
            let c0 = TeamStoryContext {
                item_seed: 0,
                ..ctx(Some(2), Some(88), Some("alta"), Some("Laguna Seca"))
            };
            let c1 = TeamStoryContext {
                item_seed: 1,
                ..ctx(Some(2), Some(88), Some("alta"), Some("Laguna Seca"))
            };
            for trigger in [
                TeamTrigger::TeamInStrongMoment,
                TeamTrigger::TeamUnderPressure,
                TeamTrigger::TeamBecameRelevant,
                TeamTrigger::TeamLostGround,
            ] {
                let h0 = compose_team_headline(trigger, &c0);
                let h1 = compose_team_headline(trigger, &c1);
                assert_ne!(h0, h1, "{trigger:?}: variante 0 e 1 devem diferir");
            }
        }

        #[test]
        fn test_team_strong_moment_headline_uses_collective_editorial_voice() {
            let c0 = TeamStoryContext {
                item_seed: 0,
                ..ctx(Some(2), Some(88), Some("alta"), Some("Laguna Seca"))
            };
            let c1 = TeamStoryContext {
                item_seed: 1,
                ..ctx(Some(2), Some(88), Some("alta"), Some("Laguna Seca"))
            };

            assert_eq!(
                compose_team_headline(TeamTrigger::TeamInStrongMoment, &c0),
                "Equipe Solaris consolida forca coletiva e entra no bloco da frente"
            );
            assert_eq!(
                compose_team_headline(TeamTrigger::TeamInStrongMoment, &c1),
                "Equipe Solaris ganha corpo como estrutura e sobe de patamar na categoria"
            );
        }

        #[test]
        fn test_team_strong_moment_body_avoids_weight_language() {
            let body = compose_team_body(
                TeamTrigger::TeamInStrongMoment,
                &TeamStoryContext {
                    item_seed: 0,
                    ..ctx(Some(2), Some(88), Some("alta"), Some("Laguna Seca"))
                },
            );

            assert!(
                !body.contains("peso real na disputa"),
                "TeamInStrongMoment nao deve ecoar linguagem abstrata de peso: {body}"
            );
            assert!(
                body.contains("conjunto") || body.contains("operacao") || body.contains("bloco"),
                "TeamInStrongMoment deve soar mais coletivo e estrutural: {body}"
            );
        }

        #[test]
        fn test_team_lost_ground_body_avoids_reducing_weight_formula() {
            let body = compose_team_body(
                TeamTrigger::TeamLostGround,
                &TeamStoryContext {
                    item_seed: 0,
                    ..ctx(Some(7), Some(35), Some("elite"), Some("Laguna Seca"))
                },
            );

            assert!(
                !body.contains("reduzir o peso"),
                "TeamLostGround nao deve reciclar a formula de perda de peso editorial: {body}"
            );
        }

        #[test]
        fn test_team_became_relevant_headline_avoids_periferia() {
            let headline = compose_team_headline(
                TeamTrigger::TeamBecameRelevant,
                &ctx(Some(6), Some(41), Some("relevante"), Some("Laguna Seca")),
            );
            assert!(
                !headline.contains("periferia"),
                "TeamBecameRelevant headline nao deve espelhar Pilot v0 ('sai da periferia'): {headline}"
            );
        }

        #[test]
        fn test_team_became_relevant_body_avoids_bloco_lateral() {
            let body = compose_team_body(
                TeamTrigger::TeamBecameRelevant,
                &TeamStoryContext {
                    item_seed: 0,
                    ..ctx(Some(6), Some(41), Some("relevante"), Some("Laguna Seca"))
                },
            );
            assert!(
                !body.contains("bloco lateral"),
                "TeamBecameRelevant body v0 nao deve espelhar Pilot v0 ('saiu do bloco lateral'): {body}"
            );
        }

        #[test]
        fn test_team_under_pressure_body_avoids_pilot_mirror() {
            let body = compose_team_body(
                TeamTrigger::TeamUnderPressure,
                &TeamStoryContext {
                    item_seed: 0,
                    ..ctx(Some(3), Some(72), Some("alta"), Some("Laguna Seca"))
                },
            );
            assert!(
                !body.contains("cobranca mais visivel"),
                "TeamUnderPressure body v0 nao deve espelhar Pilot v0 ('cobranca mais visivel'): {body}"
            );
        }

        #[test]
        fn test_team_lost_ground_headline_avoids_leitura() {
            let headline = compose_team_headline(
                TeamTrigger::TeamLostGround,
                &ctx(Some(7), Some(35), Some("elite"), None),
            );
            assert!(
                !headline.contains("leitura"),
                "TeamLostGround headline nao deve usar campo de 'leitura' (saturado no sistema): {headline}"
            );
        }

        #[test]
        fn test_team_lost_ground_body_v1_avoids_leitura() {
            let body = compose_team_body(
                TeamTrigger::TeamLostGround,
                &TeamStoryContext {
                    item_seed: 1,
                    ..ctx(Some(7), Some(35), Some("elite"), None)
                },
            );
            assert!(
                !body.contains("leitura"),
                "TeamLostGround body v1 nao deve usar campo de 'leitura': {body}"
            );
        }

        /// Execute com: cargo test dump_team_primary_triggers_for_review -- --nocapture
        #[test]
        fn dump_team_primary_triggers_for_review() {
            let cases: &[(&str, TeamTrigger, TeamStoryContext)] = &[
                (
                    "team_in_strong_moment",
                    TeamTrigger::TeamInStrongMoment,
                    TeamStoryContext {
                        item_seed: 0,
                        ..ctx(Some(2), Some(88), Some("alta"), Some("Laguna Seca"))
                    },
                ),
                (
                    "team_under_pressure",
                    TeamTrigger::TeamUnderPressure,
                    TeamStoryContext {
                        item_seed: 0,
                        ..ctx(Some(3), Some(72), Some("alta"), Some("Laguna Seca"))
                    },
                ),
                (
                    "team_became_relevant",
                    TeamTrigger::TeamBecameRelevant,
                    TeamStoryContext {
                        item_seed: 0,
                        ..ctx(Some(6), Some(41), Some("relevante"), Some("Laguna Seca"))
                    },
                ),
                (
                    "team_lost_ground",
                    TeamTrigger::TeamLostGround,
                    TeamStoryContext {
                        item_seed: 0,
                        ..ctx(Some(7), Some(35), Some("elite"), Some("Laguna Seca"))
                    },
                ),
                (
                    "team_fallback",
                    TeamTrigger::FallbackTeamStory,
                    TeamStoryContext {
                        team_name: "Equipe Solaris".to_string(),
                        category_name: Some("GT4 Challenge".to_string()),
                        team_position: None,
                        team_points: None,
                        presence_tier: None,
                        next_race_label: None,
                        item_seed: 0,
                    },
                ),
            ];

            println!("\n=== TEAM PRIMARY TRIGGERS ===");
            for (label, trigger, c) in cases {
                let body = compose_team_body(*trigger, c);
                let headline = compose_team_headline(*trigger, c);
                println!("case={label}");
                println!("headline: {headline}");
                println!("body: {body}");
                println!();
            }
        }

        /// Dump de revisão editorial dos modificadores de Equipe.
        /// Execute com: cargo test dump_team_modifiers_for_review -- --nocapture
        #[test]
        fn dump_team_modifiers_for_review() {
            let cases: &[(&str, TeamTrigger, TeamStoryContext)] = &[
                (
                    "strong_moment_top_table",
                    TeamTrigger::TeamInStrongMoment,
                    TeamStoryContext {
                        item_seed: 0,
                        ..ctx(Some(2), Some(88), Some("relevante"), Some("Laguna Seca"))
                    },
                ),
                (
                    "under_pressure_need_response",
                    TeamTrigger::TeamUnderPressure,
                    TeamStoryContext {
                        item_seed: 0,
                        ..ctx(Some(3), Some(72), Some("alta"), Some("Laguna Seca"))
                    },
                ),
                (
                    "became_relevant_public_weight",
                    TeamTrigger::TeamBecameRelevant,
                    TeamStoryContext {
                        item_seed: 0,
                        ..ctx(Some(6), Some(41), Some("alta"), Some("Laguna Seca"))
                    },
                ),
                (
                    "strong_moment_top_table_plus_public_weight",
                    TeamTrigger::TeamInStrongMoment,
                    TeamStoryContext {
                        item_seed: 0,
                        ..ctx(Some(2), Some(88), Some("elite"), Some("Laguna Seca"))
                    },
                ),
            ];

            println!("\n=== TEAM MODIFIER REVIEW ===");
            for (label, trigger, c) in cases {
                let body = compose_team_body(*trigger, c);
                println!("case={label}");
                println!("body: {body}");
                println!();
            }
        }

        #[test]
        fn test_team_strong_moment_modifier_public_weight_avoids_caso_e_paddock() {
            // seed=0, position=2, presence_tier="alta" → TopTableContext + GrowingPublicWeight
            let body = compose_team_body(
                TeamTrigger::TeamInStrongMoment,
                &TeamStoryContext {
                    item_seed: 0,
                    ..ctx(Some(2), Some(88), Some("alta"), Some("Laguna Seca"))
                },
            );
            assert!(
                !body.contains("caso ficou maior"),
                "GrowingPublicWeight modifier v0 nao deve usar linguagem de Mercado ('caso ficou maior'): {body}"
            );
        }
    }

    mod injury_editorial_tests {
        use super::super::{
            compose_injury_body, compose_injury_headline, detect_injury_modifiers,
            detect_injury_trigger, InjuryModifier, InjuryStoryContext, InjuryTrigger,
        };

        fn ctx(
            ruled_out: bool,
            returning: bool,
            uncertain: bool,
            position: Option<i32>,
            next_race_label: Option<&str>,
        ) -> InjuryStoryContext {
            InjuryStoryContext {
                driver_name: "Rodrigo".to_string(),
                category_name: Some("GT4 Challenge".to_string()),
                driver_position: position,
                next_race_label: next_race_label.map(|s| s.to_string()),
                item_seed: 0,
                is_ruled_out: ruled_out,
                is_returning: returning,
                is_uncertain: uncertain,
            }
        }

        #[test]
        fn test_driver_ruled_out_by_injury_trigger() {
            assert_eq!(
                detect_injury_trigger(&ctx(true, false, false, Some(4), Some("Laguna Seca"))),
                InjuryTrigger::DriverRuledOutByInjury
            );
        }

        #[test]
        fn test_driver_returns_from_injury_trigger() {
            assert_eq!(
                detect_injury_trigger(&ctx(false, true, false, Some(4), Some("Laguna Seca"))),
                InjuryTrigger::DriverReturnsFromInjury
            );
        }

        #[test]
        fn test_injury_status_still_uncertain_trigger() {
            assert_eq!(
                detect_injury_trigger(&ctx(false, false, true, Some(4), Some("Laguna Seca"))),
                InjuryTrigger::InjuryStatusStillUncertain
            );
        }

        #[test]
        fn test_injury_fallback_when_flags_absent() {
            assert_eq!(
                detect_injury_trigger(&ctx(false, false, false, Some(4), Some("Laguna Seca"))),
                InjuryTrigger::FallbackInjuryStory
            );
        }

        #[test]
        fn test_injury_body_mentions_driver_in_all_non_fallback_triggers() {
            let c = ctx(true, false, false, Some(4), Some("Laguna Seca"));
            for trigger in [
                InjuryTrigger::DriverRuledOutByInjury,
                InjuryTrigger::DriverReturnsFromInjury,
                InjuryTrigger::InjuryStatusStillUncertain,
            ] {
                let body = compose_injury_body(trigger, &c);
                assert!(
                    body.contains("Rodrigo"),
                    "{trigger:?}: body deve mencionar o piloto: {body}"
                );
            }
        }

        #[test]
        fn test_top_driver_context_fires_for_top_driver() {
            let mods = detect_injury_modifiers(
                &ctx(true, false, false, Some(2), Some("Laguna Seca")),
                InjuryTrigger::DriverRuledOutByInjury,
            );
            assert!(mods.contains(&InjuryModifier::TopDriverContext));
        }

        #[test]
        fn test_next_race_pressure_fires_when_next_race_exists() {
            let mods = detect_injury_modifiers(
                &ctx(true, false, false, Some(5), Some("Laguna Seca")),
                InjuryTrigger::DriverRuledOutByInjury,
            );
            assert!(mods.contains(&InjuryModifier::NextRacePressure));
        }

        #[test]
        fn test_return_changes_grid_reading_fires_for_return_trigger() {
            let mods = detect_injury_modifiers(
                &ctx(false, true, false, Some(5), Some("Laguna Seca")),
                InjuryTrigger::DriverReturnsFromInjury,
            );
            assert!(mods.contains(&InjuryModifier::ReturnChangesGridReading));
        }

        #[test]
        fn test_injury_modifiers_never_exceed_two() {
            let mods = detect_injury_modifiers(
                &ctx(false, true, false, Some(2), Some("Laguna Seca")),
                InjuryTrigger::DriverReturnsFromInjury,
            );
            assert!(
                mods.len() <= 2,
                "Lesao nunca deve exceder 2 modificadores: {mods:?}"
            );
        }

        #[test]
        fn test_injury_body_with_modifiers_is_longer_than_without_modifiers() {
            let with_modifiers = compose_injury_body(
                InjuryTrigger::DriverRuledOutByInjury,
                &ctx(true, false, false, Some(2), Some("Laguna Seca")),
            );
            let without_modifiers = compose_injury_body(
                InjuryTrigger::DriverRuledOutByInjury,
                &ctx(true, false, false, Some(8), None),
            );
            assert!(
                with_modifiers.len() > without_modifiers.len(),
                "body com modificadores deve ser maior que body sem modificadores"
            );
        }

        /// Dump de revisao editorial dos triggers principais de Lesao.
        /// Execute com: cargo test dump_injury_primary_triggers_for_review -- --nocapture
        #[test]
        fn dump_injury_primary_triggers_for_review() {
            let cases: &[(&str, InjuryTrigger, InjuryStoryContext)] = &[
                (
                    "ruled_out",
                    InjuryTrigger::DriverRuledOutByInjury,
                    ctx(true, false, false, Some(4), Some("Laguna Seca")),
                ),
                (
                    "returning",
                    InjuryTrigger::DriverReturnsFromInjury,
                    ctx(false, true, false, Some(4), Some("Laguna Seca")),
                ),
                (
                    "uncertain",
                    InjuryTrigger::InjuryStatusStillUncertain,
                    ctx(false, false, true, Some(4), Some("Laguna Seca")),
                ),
                (
                    "fallback",
                    InjuryTrigger::FallbackInjuryStory,
                    ctx(false, false, false, Some(4), Some("Laguna Seca")),
                ),
            ];

            println!("\n=== INJURY PRIMARY TRIGGERS ===");
            for (label, trigger, c) in cases {
                let headline = compose_injury_headline(*trigger, c);
                let body = compose_injury_body(*trigger, c);
                println!("case={label}");
                println!("headline: {headline}");
                println!("body: {body}");
                println!();
            }
        }

        /// Dump de revisao editorial dos modificadores de Lesao.
        /// Execute com: cargo test dump_injury_modifiers_for_review -- --nocapture
        #[test]
        fn dump_injury_modifiers_for_review() {
            let cases: &[(&str, InjuryTrigger, InjuryStoryContext)] = &[
                (
                    "ruled_out_top_driver",
                    InjuryTrigger::DriverRuledOutByInjury,
                    ctx(true, false, false, Some(2), Some("Laguna Seca")),
                ),
                (
                    "uncertain_top_driver",
                    InjuryTrigger::InjuryStatusStillUncertain,
                    ctx(false, false, true, Some(2), Some("Laguna Seca")),
                ),
                (
                    "returning_grid_reading",
                    InjuryTrigger::DriverReturnsFromInjury,
                    ctx(false, true, false, Some(5), Some("Laguna Seca")),
                ),
            ];

            println!("\n=== INJURY MODIFIER REVIEW ===");
            for (label, trigger, c) in cases {
                let headline = compose_injury_headline(*trigger, c);
                let body = compose_injury_body(*trigger, c);
                println!("case={label}");
                println!("headline: {headline}");
                println!("body: {body}");
                println!();
            }
        }

        #[test]
        fn test_injury_ruled_out_body_mentions_next_race_only_once() {
            let body = compose_injury_body(
                InjuryTrigger::DriverRuledOutByInjury,
                &ctx(true, false, false, Some(2), Some("Laguna Seca")),
            );

            assert_eq!(
                body.matches("Laguna Seca").count(),
                1,
                "DriverRuledOutByInjury nao deve repetir a proxima etapa em excesso: {body}"
            );
        }

        #[test]
        fn test_injury_returning_body_does_not_repeat_reading_language() {
            let body = compose_injury_body(
                InjuryTrigger::DriverReturnsFromInjury,
                &ctx(false, true, false, Some(5), Some("Laguna Seca")),
            );

            assert!(
                body.matches("leitura").count() <= 1,
                "DriverReturnsFromInjury nao deve saturar o eixo de leitura da rodada: {body}"
            );
        }

        #[test]
        fn test_injury_returning_modifier_avoids_leitura_do_grid() {
            // seed=0, position=9 (>8): sem TopDriverContext, sem NextRacePressure —
            // só ReturnChangesGridReading ativa, variante v%3=0
            let body = compose_injury_body(
                InjuryTrigger::DriverReturnsFromInjury,
                &ctx(false, true, false, Some(9), None),
            );
            assert!(
                !body.contains("muda a leitura do grid"),
                "ReturnChangesGridReading v0 nao deve usar formula 'muda a leitura do grid': {body}"
            );
        }
    }

    mod editorial_audit_tests {
        use super::super::{
            compose_incident_body, compose_incident_headline, compose_injury_body,
            compose_injury_headline, compose_market_body, compose_market_headline,
            compose_pilot_body, compose_pilot_headline, compose_race_body, compose_team_body,
            compose_team_headline, IncidentStoryContext, IncidentTrigger, InjuryStoryContext,
            InjuryTrigger, MarketStoryContext, MarketTrigger, PilotStoryContext, PilotTrigger,
            RaceStoryContext, RaceTrigger, TeamStoryContext, TeamTrigger,
        };

        fn race_ctx(position: Option<i32>, streak: u32) -> RaceStoryContext {
            RaceStoryContext {
                driver_name: "Carlos Mendes".to_string(),
                category_name: "Mazda MX-5 Rookie Cup".to_string(),
                driver_position: position,
                driver_points: Some(120),
                win_streak: streak,
                item_seed: 0,
                is_lead_change: false,
                is_dominant_win: false,
                rival_finish_position: None,
                rival_dnf: false,
                pole_plus_win: false,
                recovery_win: false,
                first_win_of_season: false,
                first_win_of_career: false,
            }
        }

        fn pilot_ctx(position: Option<i32>, streak: u32) -> PilotStoryContext {
            PilotStoryContext {
                driver_name: "Rafael Medina".to_string(),
                category_name: "GT4 Challenge".to_string(),
                driver_position: position,
                win_streak: streak,
                item_seed: 0,
                last_race_finish: None,
                last_race_dnf: false,
                points_gap_to_leader: None,
            }
        }

        fn market_ctx(
            driver: Option<&str>,
            team: Option<&str>,
            is_preseason: bool,
        ) -> MarketStoryContext {
            MarketStoryContext {
                driver_name: driver.map(|s| s.to_string()),
                team_name: team.map(|s| s.to_string()),
                category_name: Some("GT4 Challenge".to_string()),
                is_preseason,
                item_seed: 0,
                preseason_week: if is_preseason { Some(3) } else { None },
                presence_tier: None,
                subject_is_driver: driver.is_some(),
                subject_is_team: team.is_some() && driver.is_none(),
            }
        }

        fn incident_ctx(
            driver: Option<&str>,
            secondary: Option<&str>,
            is_mechanical: bool,
            is_still_open: bool,
            is_dnf: bool,
            segment: Option<&str>,
        ) -> IncidentStoryContext {
            IncidentStoryContext {
                driver_name: driver.map(|s| s.to_string()),
                secondary_driver_name: secondary.map(|s| s.to_string()),
                category_name: Some("GT4 Challenge".to_string()),
                is_mechanical,
                is_still_open,
                is_dnf,
                segment: segment.map(|s| s.to_string()),
                item_seed: 0,
            }
        }

        fn team_ctx(
            position: Option<i32>,
            points: Option<i32>,
            presence_tier: Option<&str>,
            next_race_label: Option<&str>,
        ) -> TeamStoryContext {
            TeamStoryContext {
                team_name: "Equipe Solaris".to_string(),
                category_name: Some("GT4 Challenge".to_string()),
                team_position: position,
                team_points: points,
                presence_tier: presence_tier.map(|s| s.to_string()),
                next_race_label: next_race_label.map(|s| s.to_string()),
                item_seed: 0,
            }
        }

        fn injury_ctx(
            ruled_out: bool,
            returning: bool,
            uncertain: bool,
            position: Option<i32>,
            next_race_label: Option<&str>,
        ) -> InjuryStoryContext {
            InjuryStoryContext {
                driver_name: "Rodrigo".to_string(),
                category_name: Some("GT4 Challenge".to_string()),
                driver_position: position,
                next_race_label: next_race_label.map(|s| s.to_string()),
                item_seed: 0,
                is_ruled_out: ruled_out,
                is_returning: returning,
                is_uncertain: uncertain,
            }
        }

        /// Dump transversal curto para auditar headlines do sistema editorial.
        /// Execute com: cargo test dump_editorial_headlines_review -- --nocapture
        #[test]
        fn dump_editorial_headlines_review() {
            println!("\n=== EDITORIAL HEADLINES REVIEW ===");

            println!("type=race");
            println!("case=leader_won");
            println!("headline: (herda item.titulo / bundle de Corrida no fluxo atual)");
            println!();

            println!("type=pilot");
            println!("case=strong_form");
            println!(
                "headline: {}",
                compose_pilot_headline(PilotTrigger::PilotInStrongForm, &pilot_ctx(Some(1), 3))
            );
            println!();

            println!("type=market");
            println!("case=heated_driver");
            println!(
                "headline: {}",
                compose_market_headline(
                    MarketTrigger::MarketHeatedAroundDriver,
                    &market_ctx(Some("Rodrigo"), None, false),
                )
            );
            println!();

            let incident_damage =
                incident_ctx(Some("Rodrigo"), None, false, false, true, Some("Late"));
            let incident_two_driver =
                incident_ctx(Some("Rodrigo"), Some("Marcelo"), false, false, false, None);
            println!("type=incident");
            println!("case=driver_damage");
            println!(
                "headline: {}",
                compose_incident_headline(IncidentTrigger::DriverIncidentDamage, &incident_damage)
            );
            println!("case=two_driver");
            println!(
                "headline: {}",
                compose_incident_headline(IncidentTrigger::TwoDriverIncident, &incident_two_driver)
            );
            println!();

            println!("type=team");
            println!("case=strong_moment");
            println!(
                "headline: {}",
                compose_team_headline(
                    TeamTrigger::TeamInStrongMoment,
                    &team_ctx(Some(2), Some(88), Some("alta"), Some("Laguna Seca")),
                )
            );
            println!();

            let injury_ruled_out = injury_ctx(true, false, false, Some(2), Some("Laguna Seca"));
            let injury_returning = injury_ctx(false, true, false, Some(5), Some("Laguna Seca"));
            println!("type=injury");
            println!("case=ruled_out");
            println!(
                "headline: {}",
                compose_injury_headline(InjuryTrigger::DriverRuledOutByInjury, &injury_ruled_out)
            );
            println!("case=returning");
            println!(
                "headline: {}",
                compose_injury_headline(InjuryTrigger::DriverReturnsFromInjury, &injury_returning)
            );
            println!();
        }

        /// Dump transversal curto para auditar bodies lado a lado.
        /// Execute com: cargo test dump_editorial_bodies_review -- --nocapture
        #[test]
        fn dump_editorial_bodies_review() {
            let race_leader = compose_race_body(RaceTrigger::LeaderWon, &race_ctx(Some(1), 3))
                .unwrap_or_default();
            let race_lead_change = compose_race_body(
                RaceTrigger::LeadChanged,
                &RaceStoryContext {
                    driver_position: Some(1),
                    is_lead_change: true,
                    ..race_ctx(Some(1), 2)
                },
            )
            .unwrap_or_default();

            let pilot_strong =
                compose_pilot_body(PilotTrigger::PilotInStrongForm, &pilot_ctx(Some(1), 3));
            let pilot_pressure =
                compose_pilot_body(PilotTrigger::PilotUnderPressure, &pilot_ctx(Some(2), 0));

            let market_heated = compose_market_body(
                MarketTrigger::MarketHeatedAroundDriver,
                &market_ctx(Some("Rodrigo"), None, false),
            );
            let market_concrete = compose_market_body(
                MarketTrigger::ConcreteMoveUnderway,
                &market_ctx(Some("Rodrigo"), None, false),
            );

            let incident_damage =
                incident_ctx(Some("Rodrigo"), None, false, false, true, Some("Late"));
            let incident_two_driver =
                incident_ctx(Some("Rodrigo"), Some("Marcelo"), false, false, false, None);
            let incident_damage_body =
                compose_incident_body(IncidentTrigger::DriverIncidentDamage, &incident_damage);
            let incident_two_driver_body =
                compose_incident_body(IncidentTrigger::TwoDriverIncident, &incident_two_driver);

            let team_strong = compose_team_body(
                TeamTrigger::TeamInStrongMoment,
                &team_ctx(Some(2), Some(88), Some("alta"), Some("Laguna Seca")),
            );
            let team_lost = compose_team_body(
                TeamTrigger::TeamLostGround,
                &team_ctx(Some(7), Some(35), Some("elite"), Some("Laguna Seca")),
            );

            let injury_ruled_out = compose_injury_body(
                InjuryTrigger::DriverRuledOutByInjury,
                &injury_ctx(true, false, false, Some(2), Some("Laguna Seca")),
            );
            let injury_returning = compose_injury_body(
                InjuryTrigger::DriverReturnsFromInjury,
                &injury_ctx(false, true, false, Some(5), Some("Laguna Seca")),
            );

            println!("\n=== EDITORIAL BODIES REVIEW ===");
            println!("type=race");
            println!("case=leader_won");
            println!("body: {race_leader}");
            println!("case=lead_changed");
            println!("body: {race_lead_change}");
            println!();

            println!("type=pilot");
            println!("case=strong_form");
            println!("body: {pilot_strong}");
            println!("case=under_pressure");
            println!("body: {pilot_pressure}");
            println!();

            println!("type=market");
            println!("case=heated_driver");
            println!("body: {market_heated}");
            println!("case=concrete_move");
            println!("body: {market_concrete}");
            println!();

            println!("type=incident");
            println!("case=driver_damage_dnf_late");
            println!("body: {incident_damage_body}");
            println!("case=two_driver_plain");
            println!("body: {incident_two_driver_body}");
            println!();

            println!("type=team");
            println!("case=strong_moment_top_table");
            println!("body: {team_strong}");
            println!("case=lost_ground");
            println!("body: {team_lost}");
            println!();

            println!("type=injury");
            println!("case=ruled_out_top_driver");
            println!("body: {injury_ruled_out}");
            println!("case=returning_grid_reading");
            println!("body: {injury_returning}");
            println!();
        }

        #[test]
        fn test_market_heated_driver_body_avoids_case_and_next_step_bureaucracy() {
            let body = compose_market_body(
                MarketTrigger::MarketHeatedAroundDriver,
                &market_ctx(Some("Rodrigo"), None, false),
            );

            assert!(
                !body.contains("peso do caso"),
                "MarketHeatedAroundDriver nao deve usar burocracia de 'peso do caso': {body}"
            );
            assert!(
                !body.contains("proximo passo"),
                "MarketHeatedAroundDriver nao deve fechar com formula burocratica de proximo passo: {body}"
            );
        }

        #[test]
        fn test_market_needs_followup_modifier_avoids_proximo_passo_accented() {
            // presence_tier=None → só NeedsConcreteFollowUp ativa (v%3=0)
            let body = compose_market_body(
                MarketTrigger::MarketHeatedAroundDriver,
                &market_ctx(Some("Rodrigo"), None, false),
            );
            assert!(
                !body.contains("próximo passo"),
                "NeedsConcreteFollowUp v0 nao deve usar formula burocratica 'próximo passo': {body}"
            );
        }

        #[test]
        fn test_market_concrete_move_body_avoids_contours_and_next_step_bureaucracy() {
            let body = compose_market_body(
                MarketTrigger::ConcreteMoveUnderway,
                &market_ctx(Some("Rodrigo"), None, false),
            );

            assert!(
                !body.contains("contornos definidos"),
                "ConcreteMoveUnderway nao deve soar como despacho burocratico: {body}"
            );
            assert!(
                !body.contains("proximo passo"),
                "ConcreteMoveUnderway nao deve depender da formula de proximo passo: {body}"
            );
        }
    }
}
