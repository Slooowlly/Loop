//! Dossie historico e relatorio financeiro de uma equipe.
//!
//! Extraido de `career.rs`: agregacoes de corrida, marcos, timeline, rivais,
//! streaks e os rotulos/formatadores usados so por esse subsistema.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use rusqlite::OptionalExtension;

use crate::commands::career::{
    get_teams_standings_in_base_dir, open_career_resources_for_category_read,
    open_career_resources_read_only,
};
use crate::commands::career_types::{
    TeamFinanceCashPoint, TeamFinanceReport, TeamFinanceRound, TeamHistoryCategoryStep,
    TeamHistoryDossier, TeamHistoryHighlight, TeamHistoryIdentity, TeamHistoryManagement,
    TeamHistoryMilestone, TeamHistoryMovement, TeamHistoryOwnershipEvent, TeamHistoryRecord,
    TeamHistoryRival, TeamHistorySeasonResult, TeamHistorySport, TeamHistoryTimelineItem,
    TeamHistoryTitleCategory,
};
use crate::constants::categories;
use crate::db::queries::teams as team_queries;

#[derive(Debug, Clone)]
struct TeamRaceFact {
    team_id: String,
    season_number: i32,
    season_year: i32,
    category: String,
    round: i32,
    points: f64,
    win: bool,
    podium: bool,
}

#[derive(Debug, Clone, Default)]
struct TeamHistoryAggregate {
    races: i32,
    wins: i32,
    podiums: i32,
    points: f64,
}

#[derive(Debug, Clone, Default)]
struct DriverSymbolAggregate {
    name: String,
    races: i32,
    wins: i32,
    podiums: i32,
}

/// Dossiê financeiro REAL de uma equipe, lido da tabela `team_finance_history`. Fonte
/// única da aba My Team: ledgers da última rodada, rosca de custos acumulados da
/// temporada e gráfico de caixa — substituindo os números fabricados no front. Save
/// sem histórico (antigo / sem corridas ainda) devolve um relatório vazio (o front
/// mostra estado honesto).
pub(crate) fn get_team_finance_report_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    category: &str,
    team_id: &str,
) -> Result<TeamFinanceReport, String> {
    /// Máximo de rodadas exibidas no gráfico de caixa.
    const TIMELINE_MAX: usize = 12;
    /// Janela lida do histórico: cobre a temporada corrente inteira + a cauda do gráfico.
    const HISTORY_WINDOW: i64 = 200;

    let category = category.trim().to_lowercase();
    let (db, _, _) = open_career_resources_for_category_read(base_dir, career_id, &category)?;

    let entries = team_queries::get_team_finance_history_recent(&db.conn, team_id, HISTORY_WINDOW)
        .map_err(|e| format!("Falha ao carregar historico financeiro da equipe: {e}"))?;

    let Some(latest_entry) = entries.last().cloned() else {
        return Ok(TeamFinanceReport::default());
    };

    let current_season = latest_entry.season_number;

    // Acumulado da temporada corrente (rosca de custos + leitura de receita).
    let mut season = TeamFinanceRound {
        season_number: current_season,
        ..Default::default()
    };
    let mut season_rounds = 0;
    for entry in entries.iter().filter(|e| e.season_number == current_season) {
        season.sponsorship_income += entry.sponsorship_income;
        season.gate_income += entry.gate_income;
        season.result_bonus += entry.result_bonus;
        season.partial_prize_income += entry.partial_prize_income;
        season.aid_income += entry.aid_income;
        season.salary_expense += entry.salary_expense;
        season.event_operations_cost += entry.event_operations_cost;
        season.structural_maintenance_cost += entry.structural_maintenance_cost;
        season.technical_investment_cost += entry.technical_investment_cost;
        season.debt_service_cost += entry.debt_service_cost;
        season.constructor_prize_income += entry.constructor_prize_income;
        season.income_total += entry.income_total;
        season.expenses_total += entry.expenses_total;
        season.net += entry.net;
        season_rounds += 1;
    }
    season.round = season_rounds; // aqui `round` carrega a CONTAGEM de rodadas somadas.

    let latest = TeamFinanceRound {
        season_number: latest_entry.season_number,
        round: latest_entry.round,
        sponsorship_income: latest_entry.sponsorship_income,
        gate_income: latest_entry.gate_income,
        result_bonus: latest_entry.result_bonus,
        partial_prize_income: latest_entry.partial_prize_income,
        aid_income: latest_entry.aid_income,
        salary_expense: latest_entry.salary_expense,
        event_operations_cost: latest_entry.event_operations_cost,
        structural_maintenance_cost: latest_entry.structural_maintenance_cost,
        technical_investment_cost: latest_entry.technical_investment_cost,
        debt_service_cost: latest_entry.debt_service_cost,
        constructor_prize_income: latest_entry.constructor_prize_income,
        income_total: latest_entry.income_total,
        expenses_total: latest_entry.expenses_total,
        net: latest_entry.net,
    };

    let start = entries.len().saturating_sub(TIMELINE_MAX);
    let cash_timeline = entries[start..]
        .iter()
        .map(|entry| TeamFinanceCashPoint {
            season_number: entry.season_number,
            round: entry.round,
            cash_balance: entry.cash_balance,
            net: entry.net,
            is_season_close: entry.constructor_prize_income > 0.0,
        })
        .collect();

    // Projeção: prêmio de construtores ESTIMADO se a temporada terminasse agora, pela
    // posição atual no campeonato. Reusa as classificações (fórmula única em `prize`); é
    // só exibição — não toca caixa nem IA. Falha nas classificações → projeção neutra (0).
    let (expected_constructor_prize, current_position, grid_size) =
        match get_teams_standings_in_base_dir(base_dir, career_id, &category) {
            Ok(standings) => {
                let grid_size = standings.len() as i32;
                match standings.iter().find(|s| s.id == team_id) {
                    Some(standing) => (
                        crate::finance::prize::constructor_prize(
                            &category,
                            standing.posicao,
                            grid_size,
                        ),
                        standing.posicao,
                        grid_size,
                    ),
                    None => (0.0, 0, grid_size),
                }
            }
            Err(_) => (0.0, 0, 0),
        };

    Ok(TeamFinanceReport {
        rounds_recorded: entries.len() as i32,
        latest: Some(latest),
        season: Some(season),
        cash_timeline,
        expected_constructor_prize,
        current_position,
        grid_size,
    })
}

pub(crate) fn get_team_history_dossier_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    team_id: &str,
    category: &str,
) -> Result<TeamHistoryDossier, String> {
    let category = category.trim().to_lowercase();
    let team_id = team_id.trim();
    let (db, _, _) = open_career_resources_read_only(base_dir, career_id)?;
    let category_ids = team_history_group_categories(&category);
    let record_scope = team_history_group_label(&category);
    let all_facts = load_team_race_facts(&db.conn, &category_ids)?;
    let selected_facts: Vec<TeamRaceFact> = all_facts
        .iter()
        .filter(|fact| fact.team_id == team_id)
        .cloned()
        .collect();
    let aggregates = aggregate_team_history(&all_facts);
    let selected = aggregates.get(team_id).cloned().unwrap_or_default();
    let titles_by_team = load_constructor_titles_by_team(&db.conn, &category_ids)?;
    let selected_titles = titles_by_team.get(team_id).cloned().unwrap_or_default();
    let title_count = selected_titles.len() as i32;

    let races = selected.races.max(0);
    let wins = selected.wins.max(0);
    let podiums = selected.podiums.max(0);
    let win_rate = percentage(wins, races);
    let podium_rate = percentage(podiums, races);
    let seasons = distinct_seasons(&selected_facts);
    let has_history = races > 0;
    let sport = TeamHistorySport {
        seasons: season_count_label(seasons.len() as i32),
        current_streak: current_level_streak_label(&selected_facts),
        best_streak: best_real_streak_label(&selected_facts),
        podium_rate: format!("{podium_rate}%"),
        win_rate: format!("{win_rate}%"),
        races,
        wins,
        podiums,
    };

    let season_positions = load_team_season_positions(&db.conn, team_id);

    Ok(TeamHistoryDossier {
        team_id: team_id.to_string(),
        category: category.clone(),
        record_scope: record_scope.clone(),
        has_history,
        records: vec![
            TeamHistoryRecord {
                label: rust_i18n::t!("team_dossier.records.titles").to_string(),
                rank: rank_for_i32(&titles_by_team, team_id),
                value: title_count.to_string(),
            },
            TeamHistoryRecord {
                label: rust_i18n::t!("team_dossier.records.wins").to_string(),
                rank: rank_for_aggregate(&aggregates, team_id, |entry| entry.wins as f64),
                value: wins.to_string(),
            },
            TeamHistoryRecord {
                label: rust_i18n::t!("team_dossier.records.podiums").to_string(),
                rank: rank_for_aggregate(&aggregates, team_id, |entry| entry.podiums as f64),
                value: podiums.to_string(),
            },
            TeamHistoryRecord {
                label: rust_i18n::t!("team_dossier.records.podium_rate").to_string(),
                rank: rank_for_aggregate(&aggregates, team_id, |entry| {
                    if entry.races > 0 {
                        entry.podiums as f64 / entry.races as f64
                    } else {
                        0.0
                    }
                }),
                value: format!("{podium_rate}%"),
            },
            TeamHistoryRecord {
                label: rust_i18n::t!("team_dossier.records.win_rate").to_string(),
                rank: rank_for_aggregate(&aggregates, team_id, |entry| {
                    if entry.races > 0 {
                        entry.wins as f64 / entry.races as f64
                    } else {
                        0.0
                    }
                }),
                value: format!("{win_rate}%"),
            },
        ],
        sport,
        identity: build_real_team_identity(
            &db.conn,
            team_id,
            &category,
            &record_scope,
            &selected_facts,
            &aggregates,
            title_count,
        )?,
        management: build_real_team_management(&db.conn, team_id, &selected_facts)?,
        timeline: build_real_team_timeline(&selected_facts),
        title_categories: selected_titles
            .iter()
            .enumerate()
            .map(|(index, title)| TeamHistoryTitleCategory {
                category: team_history_category_label(&title.category),
                year: title.season_year.to_string(),
                color: history_palette(index),
            })
            .collect(),
        category_path: build_real_category_path(&selected_facts),
        ownership_events: load_team_ownership_events(&db.conn, team_id)?,
        highlights: build_team_highlights(&selected_facts, &selected_titles, &season_positions),
        milestones: build_team_milestones(&selected_facts, &selected_titles),
        season_results: build_team_season_results(&selected_facts, &season_positions),
        movement: build_team_movement(&selected_facts),
    })
}

/// Marcos cronológicos (primeira vitória, primeiro pódio, primeiro título).
fn build_team_milestones(
    facts: &[TeamRaceFact],
    titles: &[TeamTitleFact],
) -> Vec<TeamHistoryMilestone> {
    let mut milestones = Vec::new();
    if let Some(year) = facts
        .iter()
        .filter(|f| f.podium)
        .map(|f| f.season_year)
        .min()
    {
        milestones.push(TeamHistoryMilestone {
            label: rust_i18n::t!("team_dossier.first_milestone.podium").to_string(),
            year: year.to_string(),
        });
    }
    if let Some(year) = facts.iter().filter(|f| f.win).map(|f| f.season_year).min() {
        milestones.push(TeamHistoryMilestone {
            label: rust_i18n::t!("team_dossier.first_milestone.win").to_string(),
            year: year.to_string(),
        });
    }
    if let Some(year) = titles.iter().map(|t| t.season_year).min() {
        milestones.push(TeamHistoryMilestone {
            label: rust_i18n::t!("team_dossier.first_milestone.title").to_string(),
            year: year.to_string(),
        });
    }
    milestones
}

/// Posição final no campeonato por temporada (melhor posição se multiclasse).
/// Degrada para vazio se a tabela de arquivo ainda não existe.
fn load_team_season_positions(conn: &rusqlite::Connection, team_id: &str) -> HashMap<i32, i32> {
    let mut positions = HashMap::new();
    let mut stmt = match conn.prepare(
        "SELECT season_number, MIN(posicao_campeonato)
         FROM team_season_archive
         WHERE team_id = ?1 AND posicao_campeonato IS NOT NULL
         GROUP BY season_number",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return positions,
    };
    if let Ok(rows) = stmt.query_map(rusqlite::params![team_id], |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?))
    }) {
        for row in rows.flatten() {
            positions.insert(row.0, row.1);
        }
    }
    positions
}

/// Resultados temporada a temporada (ano, categoria dominante, vitórias, pódios,
/// pontos) — base da aba Esportivo, em ordem cronológica.
fn build_team_season_results(
    facts: &[TeamRaceFact],
    positions: &HashMap<i32, i32>,
) -> Vec<TeamHistorySeasonResult> {
    use std::collections::BTreeMap;

    // season_number → (ano, vitórias, pódios, pontos, categoria→corridas).
    let mut by_season: BTreeMap<i32, (i32, i32, i32, f64, HashMap<String, i32>)> = BTreeMap::new();
    for fact in facts {
        let entry = by_season
            .entry(fact.season_number)
            .or_insert_with(|| (fact.season_year, 0, 0, 0.0, HashMap::new()));
        entry.0 = fact.season_year;
        if fact.win {
            entry.1 += 1;
        }
        if fact.podium {
            entry.2 += 1;
        }
        entry.3 += fact.points;
        *entry.4.entry(fact.category.clone()).or_insert(0) += 1;
    }

    by_season
        .into_iter()
        .map(|(season_number, (year, wins, podiums, points, cats))| {
            let category = cats
                .iter()
                .max_by_key(|(_, races)| **races)
                .map(|(cat, _)| team_history_category_label(cat))
                .unwrap_or_default();
            let position = positions
                .get(&season_number)
                .map(|pos| format!("P{pos}"))
                .unwrap_or_else(|| "—".to_string());
            TeamHistorySeasonResult {
                year: year.to_string(),
                category,
                position,
                wins,
                podiums,
                points: format!("{}", points.round() as i64),
            }
        })
        .collect()
}

/// Superlativos da equipe a partir do histórico real: melhor temporada (vitórias),
/// pico de pódios numa temporada e maior sequência de títulos consecutivos.
fn build_team_highlights(
    facts: &[TeamRaceFact],
    titles: &[TeamTitleFact],
    positions: &HashMap<i32, i32>,
) -> Vec<TeamHistoryHighlight> {
    use std::collections::BTreeMap;

    // Agrega por temporada: (ano, vitórias, pódios, categoria→corridas).
    let mut by_season: BTreeMap<i32, (i32, i32, i32, HashMap<String, i32>)> = BTreeMap::new();
    for fact in facts {
        let entry = by_season
            .entry(fact.season_number)
            .or_insert_with(|| (fact.season_year, 0, 0, HashMap::new()));
        entry.0 = fact.season_year;
        if fact.win {
            entry.1 += 1;
        }
        if fact.podium {
            entry.2 += 1;
        }
        *entry.3.entry(fact.category.clone()).or_insert(0) += 1;
    }

    let dominant_category = |cats: &HashMap<String, i32>| -> String {
        cats.iter()
            .max_by_key(|(_, races)| **races)
            .map(|(cat, _)| team_history_category_label(cat))
            .unwrap_or_default()
    };

    let mut highlights = Vec::new();

    // Melhor temporada por vitórias.
    if let Some((_, (year, wins, _, cats))) = by_season.iter().max_by_key(|(_, v)| v.1) {
        if *wins > 0 {
            highlights.push(TeamHistoryHighlight {
                label: rust_i18n::t!("team_dossier.highlight.best_season").to_string(),
                value: rust_i18n::t!("team_dossier.highlight.best_season_value", count = wins)
                    .to_string(),
                detail: rust_i18n::t!(
                    "team_dossier.highlight.detail_year_category",
                    year = year,
                    category = dominant_category(cats)
                )
                .to_string(),
            });
        }
    }

    // Pico de pódios numa temporada.
    if let Some((_, (year, _, podiums, cats))) = by_season.iter().max_by_key(|(_, v)| v.2) {
        if *podiums > 0 {
            highlights.push(TeamHistoryHighlight {
                label: rust_i18n::t!("team_dossier.highlight.most_podiums").to_string(),
                value: rust_i18n::t!("team_dossier.highlight.most_podiums_value", count = podiums)
                    .to_string(),
                detail: rust_i18n::t!(
                    "team_dossier.highlight.detail_year_category",
                    year = year,
                    category = dominant_category(cats)
                )
                .to_string(),
            });
        }
    }

    // Maior sequência de títulos consecutivos.
    let mut years: Vec<i32> = titles.iter().map(|title| title.season_year).collect();
    years.sort_unstable();
    years.dedup();
    let mut best_run = 0;
    let mut best_run_end = 0;
    let mut run = 0;
    let mut prev: Option<i32> = None;
    for year in &years {
        run = if prev == Some(year - 1) { run + 1 } else { 1 };
        if run > best_run {
            best_run = run;
            best_run_end = *year;
        }
        prev = Some(*year);
    }
    if best_run >= 2 {
        highlights.push(TeamHistoryHighlight {
            label: rust_i18n::t!("team_dossier.highlight.biggest_dynasty").to_string(),
            value: rust_i18n::t!("team_dossier.highlight.biggest_dynasty_value", count = best_run)
                .to_string(),
            detail: rust_i18n::t!("team_dossier.highlight.detail_until", year = best_run_end)
                .to_string(),
        });
    }

    // Melhor campanha (menor posição final no campeonato).
    if let Some((season, position)) = positions.iter().min_by_key(|(_, pos)| **pos) {
        let year = by_season.get(season).map(|entry| entry.0).unwrap_or(0);
        let value = if *position == 1 {
            rust_i18n::t!("team_dossier.highlight.champion_value").to_string()
        } else {
            format!("P{position}")
        };
        highlights.push(TeamHistoryHighlight {
            label: rust_i18n::t!("team_dossier.highlight.best_campaign").to_string(),
            value,
            detail: rust_i18n::t!("team_dossier.highlight.detail_year", year = year).to_string(),
        });
    }

    highlights
}

/// Carrega os eventos de propriedade/diretoria da equipe (ex.: venda por colapso).
/// Degrada graciosamente para vazio se a tabela ainda não existe (saves antigos
/// abertos somente-leitura antes da migração v36).
fn load_team_ownership_events(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<Vec<TeamHistoryOwnershipEvent>, String> {
    let mut stmt = match conn.prepare(
        "SELECT ano, event_type, debt_cleared, cash_injected, detail
         FROM team_ownership_events
         WHERE team_id = ?1
         ORDER BY ano ASC, id ASC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Ok(Vec::new()),
    };
    let rows = stmt
        .query_map(rusqlite::params![team_id], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar eventos de propriedade: {e}"))?;

    let mut events = Vec::new();
    for row in rows {
        let (ano, event_type, debt_cleared, cash_injected, detail) =
            row.map_err(|e| format!("Falha ao mapear evento de propriedade: {e}"))?;
        let title = match event_type.as_str() {
            "sale" => rust_i18n::t!("team_dossier.ownership.sale_title").to_string(),
            _ => rust_i18n::t!("team_dossier.ownership.change_title").to_string(),
        };
        let financial_note = rust_i18n::t!(
            "team_dossier.ownership.financial_note",
            cleared = format_brl(debt_cleared),
            injected = format_brl(cash_injected)
        )
        .to_string();
        events.push(TeamHistoryOwnershipEvent {
            year: ano.to_string(),
            event_type,
            title,
            detail,
            financial_note,
        });
    }
    Ok(events)
}

fn build_real_team_management(
    conn: &rusqlite::Connection,
    team_id: &str,
    facts: &[TeamRaceFact],
) -> Result<TeamHistoryManagement, String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar gestão histórica da equipe: {e}"))?
        .ok_or_else(|| format!("Equipe '{team_id}' não encontrada para gestão histórica"))?;
    let cash = team.cash_balance.max(0.0);
    let debt = team.debt_balance.max(0.0);
    let points: f64 = facts.iter().map(|fact| fact.points).sum();
    let seasons = distinct_seasons(facts);
    let seasons_count = seasons.len().max(1) as f64;
    let points_per_season = points / seasons_count;
    let healthy_years = if debt <= 0.0 && team.financial_state == "healthy" {
        seasons.len() as i32
    } else {
        0
    };
    let state_label = financial_state_label_for_dossier(&team.financial_state);
    // "Nível do pacote" aqui é o Nível do Carro (1–10) — a MESMA leitura de carro que o
    // jogador vê no ranking e na aba da equipe. Antes era o escalar legado arredondado numa
    // faixa 0–16: outro número, outra escala, e ainda por cima cego ao sistema de peças.
    let technical_level = team
        .car
        .as_ref()
        .map(|car| car.display_level())
        .unwrap_or(1) as i32;

    let efficiency_value = format_decimal_pt(points_per_season, 1);
    let points_int = points.round() as i32;
    Ok(TeamHistoryManagement {
        operation_health: state_label.clone(),
        peak_cash: format_brl(cash),
        worst_crisis: if debt > 0.0 {
            rust_i18n::t!("team_dossier.management.worst_crisis_debt", debt = format_brl(debt))
                .to_string()
        } else {
            rust_i18n::t!("team_dossier.management.worst_crisis_none").to_string()
        },
        healthy_years: rust_i18n::t!("team_dossier.management.healthy_years", count = healthy_years)
            .to_string(),
        efficiency: rust_i18n::t!(
            "team_dossier.management.efficiency",
            value = efficiency_value.as_str()
        )
        .to_string(),
        biggest_investment: rust_i18n::t!(
            "team_dossier.management.biggest_investment",
            level = technical_level
        )
        .to_string(),
        summary: rust_i18n::t!(
            "team_dossier.management.summary",
            state = state_label.as_str(),
            cash = format_brl(cash),
            debt = format_brl(debt),
            points = points_int
        )
        .to_string(),
        peak_cash_detail: rust_i18n::t!("team_dossier.management.peak_cash_detail").to_string(),
        worst_crisis_detail: if debt > 0.0 {
            rust_i18n::t!("team_dossier.management.worst_crisis_detail_debt").to_string()
        } else {
            rust_i18n::t!("team_dossier.management.worst_crisis_detail_none").to_string()
        },
        healthy_years_detail: rust_i18n::t!("team_dossier.management.healthy_years_detail")
            .to_string(),
        efficiency_detail: rust_i18n::t!(
            "team_dossier.management.efficiency_detail",
            points = points_int,
            avg = efficiency_value.as_str()
        )
        .to_string(),
        investment_detail: rust_i18n::t!("team_dossier.management.investment_detail").to_string(),
    })
}

fn financial_state_label_for_dossier(state: &str) -> String {
    let key = match state {
        "dominant" | "healthy" => "healthy",
        "stable" => "stable",
        "pressured" => "pressured",
        "critical" => "critical",
        _ => "monitored",
    };
    let full = format!("team_dossier.state.{key}");
    rust_i18n::t!(&full).to_string()
}

fn format_brl(value: f64) -> String {
    let rounded = value.round().max(0.0) as i64;
    let raw = rounded.to_string();
    let mut formatted = String::new();
    for (index, ch) in raw.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(ch);
    }
    let grouped: String = formatted.chars().rev().collect();
    format!("${grouped}")
}

fn format_decimal_pt(value: f64, decimals: usize) -> String {
    format!("{value:.decimals$}").replace('.', ",")
}

#[derive(Debug, Clone)]
struct TeamTitleFact {
    season_year: i32,
    category: String,
}

fn load_team_race_facts(
    conn: &rusqlite::Connection,
    category_ids: &[String],
) -> Result<Vec<TeamRaceFact>, String> {
    if category_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    let sql = format!(
        "SELECT
            r.equipe_id,
            s.numero,
            s.ano,
            c.categoria,
            c.rodada,
            r.race_id,
            SUM(r.pontos) AS team_points,
            MAX(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS has_win,
            MAX(CASE WHEN r.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END) AS has_podium
         FROM race_results r
         JOIN calendar c ON c.id = r.race_id
         JOIN seasons s ON s.id = c.temporada_id
         WHERE c.categoria IN ({placeholders})
         GROUP BY r.equipe_id, s.numero, c.categoria, c.rodada, r.race_id
         ORDER BY s.numero ASC, c.rodada ASC, r.race_id ASC, r.equipe_id ASC"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Falha ao preparar histórico real da equipe: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(category_ids.iter()), |row| {
            Ok(TeamRaceFact {
                team_id: row.get(0)?,
                season_number: row.get(1)?,
                season_year: row.get(2)?,
                category: row.get(3)?,
                round: row.get(4)?,
                points: row.get(6)?,
                win: row.get::<_, i32>(7)? > 0,
                podium: row.get::<_, i32>(8)? > 0,
            })
        })
        .map_err(|e| format!("Falha ao consultar histórico real da equipe: {e}"))?;

    let mut facts = Vec::new();
    for row in rows {
        facts.push(row.map_err(|e| format!("Falha ao ler histórico real da equipe: {e}"))?);
    }
    Ok(facts)
}

fn aggregate_team_history(facts: &[TeamRaceFact]) -> HashMap<String, TeamHistoryAggregate> {
    let mut aggregates: HashMap<String, TeamHistoryAggregate> = HashMap::new();
    for fact in facts {
        let entry = aggregates.entry(fact.team_id.clone()).or_default();
        entry.races += 1;
        entry.points += fact.points;
        if fact.win {
            entry.wins += 1;
        }
        if fact.podium {
            entry.podiums += 1;
        }
    }
    aggregates
}

fn load_constructor_titles_by_team(
    conn: &rusqlite::Connection,
    category_ids: &[String],
) -> Result<HashMap<String, Vec<TeamTitleFact>>, String> {
    if category_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    let sql = format!(
        "SELECT
            st.temporada_id,
            s.numero,
            s.ano,
            st.equipe_id,
            st.categoria,
            SUM(st.pontos) AS team_points,
            SUM(st.vitorias) AS team_wins
         FROM standings st
         JOIN seasons s ON s.id = st.temporada_id
         WHERE st.equipe_id IS NOT NULL
           AND st.categoria IN ({placeholders})
         GROUP BY st.temporada_id, s.numero, s.ano, st.equipe_id, st.categoria
         ORDER BY s.numero ASC, team_points DESC, team_wins DESC, st.equipe_id ASC"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Falha ao preparar títulos reais de equipes: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(category_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, i32>(6)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar títulos reais de equipes: {e}"))?;

    let mut best_by_season_category: BTreeMap<String, (i32, i32, String, String, f64, i32)> =
        BTreeMap::new();
    for row in rows {
        let (season_id, season_number, season_year, team_id, category, points, wins) =
            row.map_err(|e| format!("Falha ao ler títulos reais de equipes: {e}"))?;
        let key = format!("{season_id}:{category}");
        let replace = best_by_season_category
            .get(&key)
            .map(|(_, _, current_team, _, current_points, current_wins)| {
                points > *current_points
                    || ((points - *current_points).abs() < f64::EPSILON
                        && (wins > *current_wins
                            || (wins == *current_wins && team_id < *current_team)))
            })
            .unwrap_or(true);
        if replace {
            best_by_season_category.insert(
                key,
                (season_number, season_year, team_id, category, points, wins),
            );
        }
    }

    let mut titles: HashMap<String, Vec<TeamTitleFact>> = HashMap::new();
    for (_, (_season_number, season_year, team_id, category, _, _)) in best_by_season_category {
        titles.entry(team_id).or_default().push(TeamTitleFact {
            season_year,
            category,
        });
    }
    Ok(titles)
}

fn distinct_seasons(facts: &[TeamRaceFact]) -> Vec<i32> {
    facts
        .iter()
        .map(|fact| fact.season_number)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn season_count_label(total: i32) -> String {
    match total {
        0 => rust_i18n::t!("team_dossier.season_count.none").to_string(),
        1 => rust_i18n::t!("team_dossier.season_count.one").to_string(),
        value => rust_i18n::t!("team_dossier.season_count.other", count = value).to_string(),
    }
}

/// Sequência atual por NÍVEL (rookie, amador, pro, ...) — quantas temporadas
/// consecutivas a equipe está no nível atual. Diferente do "grupo" (que a equipe
/// nunca troca), o nível muda com promoções/rebaixamentos, então o streak importa.
fn current_level_streak_label(facts: &[TeamRaceFact]) -> String {
    if facts.is_empty() {
        return rust_i18n::t!("team_dossier.streak.none").to_string();
    }

    // season → categoria dominante → nível.
    let mut by_season: BTreeMap<i32, HashMap<String, i32>> = BTreeMap::new();
    for fact in facts {
        *by_season
            .entry(fact.season_number)
            .or_default()
            .entry(fact.category.clone())
            .or_insert(0) += 1;
    }
    let mut season_levels: Vec<(i32, String)> = by_season
        .into_iter()
        .map(|(season, cats)| {
            let category = cats
                .iter()
                .max_by_key(|(_, races)| **races)
                .map(|(cat, _)| cat.clone())
                .unwrap_or_default();
            let level = categories::get_category(&category)
                .map(|config| crate::constants::category_tier_label(config.nivel))
                .unwrap_or_else(|| "—".to_string());
            (season, level)
        })
        .collect();
    season_levels.sort_by_key(|(season, _)| *season);

    let current_level = match season_levels.last() {
        Some((_, level)) => level.clone(),
        None => return rust_i18n::t!("team_dossier.streak.none").to_string(),
    };

    // Conta temporadas consecutivas (e contíguas) no nível atual, do fim para trás.
    let mut streak = 0;
    let mut prev_season: Option<i32> = None;
    for (season, level) in season_levels.iter().rev() {
        if *level != current_level {
            break;
        }
        if let Some(prev) = prev_season {
            if prev - season != 1 {
                break;
            }
        }
        streak += 1;
        prev_season = Some(*season);
    }

    if streak <= 1 {
        rust_i18n::t!("team_dossier.streak.level_one", level = current_level.as_str()).to_string()
    } else {
        rust_i18n::t!(
            "team_dossier.streak.level_other",
            count = streak,
            level = current_level.as_str()
        )
        .to_string()
    }
}

fn best_real_streak_label(facts: &[TeamRaceFact]) -> String {
    if facts.is_empty() {
        return rust_i18n::t!("team_dossier.streak.none").to_string();
    }
    let mut best_podium = 0;
    let mut current_podium = 0;
    let mut best_points = 0;
    let mut current_points = 0;
    for fact in facts {
        if fact.podium {
            current_podium += 1;
            best_podium = best_podium.max(current_podium);
        } else {
            current_podium = 0;
        }
        if fact.points > 0.0 {
            current_points += 1;
            best_points = best_points.max(current_points);
        } else {
            current_points = 0;
        }
    }
    if best_podium > 0 {
        if best_podium == 1 {
            rust_i18n::t!("team_dossier.streak.podium_one").to_string()
        } else {
            rust_i18n::t!("team_dossier.streak.podium_other", count = best_podium).to_string()
        }
    } else if best_points > 0 {
        if best_points == 1 {
            rust_i18n::t!("team_dossier.streak.points_one").to_string()
        } else {
            rust_i18n::t!("team_dossier.streak.points_other", count = best_points).to_string()
        }
    } else {
        rust_i18n::t!("team_dossier.streak.none").to_string()
    }
}

fn build_real_team_timeline(facts: &[TeamRaceFact]) -> Vec<TeamHistoryTimelineItem> {
    let Some(first) = facts.first() else {
        return vec![TeamHistoryTimelineItem {
            year: "-".to_string(),
            text: "Sem corridas registradas neste recorte.".to_string(),
        }];
    };
    let mut items = vec![TeamHistoryTimelineItem {
        year: first.season_year.to_string(),
        text: format!(
            "Primeira corrida registrada em {}, rodada {}.",
            team_history_category_label(&first.category),
            first.round
        ),
    }];

    if let Some(first_win) = facts.iter().find(|fact| fact.win) {
        items.push(TeamHistoryTimelineItem {
            year: first_win.season_year.to_string(),
            text: format!(
                "Primeira vitória real em {}, rodada {}.",
                team_history_category_label(&first_win.category),
                first_win.round
            ),
        });
    }

    if let Some((season, points)) = best_real_season_points(facts) {
        items.push(TeamHistoryTimelineItem {
            year: season.to_string(),
            text: format!(
                "Melhor temporada registrada: {} pts.",
                points.round() as i32
            ),
        });
    }

    if let Some(latest) = facts.last() {
        items.push(TeamHistoryTimelineItem {
            year: latest.season_year.to_string(),
            text: format!(
                "Último registro em {}, rodada {}.",
                team_history_category_label(&latest.category),
                latest.round
            ),
        });
    }

    items
}

fn build_real_team_identity(
    conn: &rusqlite::Connection,
    team_id: &str,
    category: &str,
    record_scope: &str,
    selected_facts: &[TeamRaceFact],
    aggregates: &HashMap<String, TeamHistoryAggregate>,
    titles: i32,
) -> Result<TeamHistoryIdentity, String> {
    let origin_category = selected_facts
        .first()
        .map(|fact| fact.category.as_str())
        .unwrap_or(category);
    let current = current_team_category_label(conn, team_id)
        .unwrap_or_else(|| team_history_category_label(category));
    let profile = real_team_profile(
        selected_facts.len() as i32,
        selected_facts.iter().filter(|fact| fact.win).count() as i32,
        selected_facts.iter().filter(|fact| fact.podium).count() as i32,
        titles,
    );
    let team_name = conn
        .query_row(
            "SELECT nome FROM teams WHERE id = ?1",
            rusqlite::params![team_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| format!("Falha ao buscar equipe para identidade real: {e}"))?
        .unwrap_or_else(|| rust_i18n::t!("team_dossier.team_fallback").to_string());
    let rival = real_team_rival(conn, team_id, selected_facts, aggregates, record_scope)?;
    let (symbol_driver, symbol_driver_detail) = real_symbol_driver(conn, team_id, selected_facts)?;

    let heritage = team_heritage_label(distinct_seasons(selected_facts).len() as i32);

    Ok(TeamHistoryIdentity {
        origin: team_history_category_label(origin_category),
        current,
        heritage,
        profile: team_profile_label(profile),
        summary: real_identity_summary(&team_name, profile, selected_facts.len() as i32, titles),
        rival,
        symbol_driver,
        symbol_driver_detail,
    })
}

fn current_team_category_label(conn: &rusqlite::Connection, team_id: &str) -> Option<String> {
    let category = conn
        .query_row(
            "SELECT categoria FROM teams WHERE id = ?1",
            rusqlite::params![team_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()?;
    Some(team_history_category_label(&category))
}

/// Herança da equipe por EXPERIÊNCIA real (temporadas competidas), substituindo
/// o antigo corte de ano de fundação (1970) que rotulava quase todo time igual.
fn team_heritage_label(seasons: i32) -> String {
    let key = match seasons {
        0 => "debutant",
        1..=2 => "new",
        3..=6 => "rising",
        7..=14 => "established",
        _ => "traditional",
    };
    let full = format!("team_dossier.heritage.{key}");
    rust_i18n::t!(&full).to_string()
}

/// Perfil de DESEMPENHO por histórico real, numa escada coerente do topo ao
/// fundo do grid. Fonte única de verdade (o fallback do frontend só vale durante
/// o carregamento). Taxas calculadas no nível de corrida.
/// Perfil de DESEMPENHO por histórico real. Retorna uma CHAVE estável (não o texto)
/// — o display resolve por `team_profile_label` e a lógica de resumo casa a chave,
/// evitando o antipattern de comparar prosa traduzível.
fn real_team_profile(races: i32, wins: i32, podiums: i32, titles: i32) -> &'static str {
    if races < 4 {
        return "forming";
    }
    let win_rate = wins as f64 / races as f64;
    let podium_rate = podiums as f64 / races as f64;

    if titles > 0 || win_rate >= 0.30 || podium_rate >= 0.60 {
        "dominant"
    } else if win_rate >= 0.10 {
        "winning"
    } else if podium_rate >= 0.30 {
        "competitive"
    } else if podium_rate >= 0.10 {
        "midfield"
    } else {
        "support"
    }
}

/// Rótulo de display do perfil (i18n) a partir da chave estável.
fn team_profile_label(key: &str) -> String {
    let full = format!("team_dossier.profile.{key}");
    rust_i18n::t!(&full).to_string()
}

fn real_identity_summary(team_name: &str, profile_key: &str, races: i32, _titles: i32) -> String {
    match profile_key {
        "dominant" => {
            rust_i18n::t!("team_dossier.identity_summary.dominant", team = team_name).to_string()
        }
        "winning" => {
            rust_i18n::t!("team_dossier.identity_summary.winning", team = team_name).to_string()
        }
        "competitive" => {
            rust_i18n::t!("team_dossier.identity_summary.competitive", team = team_name).to_string()
        }
        "midfield" => {
            rust_i18n::t!("team_dossier.identity_summary.midfield", team = team_name).to_string()
        }
        "support" => {
            rust_i18n::t!("team_dossier.identity_summary.support", team = team_name).to_string()
        }
        _ => rust_i18n::t!(
            "team_dossier.identity_summary.forming",
            team = team_name,
            races = races
        )
        .to_string(),
    }
}

fn real_team_rival(
    conn: &rusqlite::Connection,
    team_id: &str,
    selected_facts: &[TeamRaceFact],
    aggregates: &HashMap<String, TeamHistoryAggregate>,
    record_scope: &str,
) -> Result<TeamHistoryRival, String> {
    let selected_races: HashSet<(i32, i32, String)> = selected_facts
        .iter()
        .map(|fact| (fact.season_number, fact.round, fact.category.clone()))
        .collect();
    let mut shared_races: HashMap<String, i32> = HashMap::new();
    for fact in load_team_race_facts(
        conn,
        &selected_facts
            .iter()
            .map(|fact| fact.category.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
    )? {
        if fact.team_id == team_id {
            continue;
        }
        if selected_races.contains(&(fact.season_number, fact.round, fact.category.clone())) {
            *shared_races.entry(fact.team_id).or_default() += 1;
        }
    }

    let selected_points = aggregates
        .get(team_id)
        .map(|entry| entry.points)
        .unwrap_or(0.0);
    let rival_id = shared_races
        .iter()
        .max_by(|(left_id, left_shared), (right_id, right_shared)| {
            left_shared.cmp(right_shared).then_with(|| {
                let left_gap = aggregates
                    .get(left_id.as_str())
                    .map(|entry| (entry.points - selected_points).abs())
                    .unwrap_or(f64::MAX);
                let right_gap = aggregates
                    .get(right_id.as_str())
                    .map(|entry| (entry.points - selected_points).abs())
                    .unwrap_or(f64::MAX);
                right_gap.total_cmp(&left_gap)
            })
        })
        .map(|(id, _)| id.clone());

    let Some(rival_id) = rival_id else {
        return Ok(TeamHistoryRival {
            name: rust_i18n::t!("team_dossier.rival_none").to_string(),
            current_category: record_scope.to_string(),
            note: "Histórico real ainda sem confronto repetido o bastante para formar rivalidade."
                .to_string(),
        });
    };

    let (name, category): (String, String) = conn
        .query_row(
            "SELECT nome, categoria FROM teams WHERE id = ?1",
            rusqlite::params![&rival_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Falha ao buscar rival histórico real: {e}"))?;
    let shared = shared_races.get(&rival_id).copied().unwrap_or(0);
    Ok(TeamHistoryRival {
        name,
        current_category: team_history_category_label(&category),
        note: format!("{shared} disputas diretas reais no {record_scope}."),
    })
}

fn real_symbol_driver(
    conn: &rusqlite::Connection,
    team_id: &str,
    selected_facts: &[TeamRaceFact],
) -> Result<(String, String), String> {
    if selected_facts.is_empty() {
        return Ok((
            rust_i18n::t!("team_dossier.symbol_none").to_string(),
            "A equipe ainda não tem corridas registradas suficientes nesse recorte.".to_string(),
        ));
    }
    let category_ids = selected_facts
        .iter()
        .map(|fact| fact.category.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    let sql = format!(
        "SELECT
            r.piloto_id,
            d.nome,
            COUNT(DISTINCT r.race_id) AS races,
            SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS wins,
            SUM(CASE WHEN r.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END) AS podiums
         FROM race_results r
         JOIN calendar c ON c.id = r.race_id
         JOIN drivers d ON d.id = r.piloto_id
         WHERE r.equipe_id = ?1
           AND c.categoria IN ({placeholders})
         GROUP BY r.piloto_id, d.nome
         ORDER BY wins DESC, podiums DESC, races DESC, d.nome ASC
         LIMIT 1"
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&team_id];
    for category in &category_ids {
        params.push(category);
    }
    let symbol = conn
        .query_row(&sql, params.as_slice(), |row| {
            Ok(DriverSymbolAggregate {
                name: row.get(1)?,
                races: row.get(2)?,
                wins: row.get(3)?,
                podiums: row.get(4)?,
            })
        })
        .optional()
        .map_err(|e| format!("Falha ao buscar piloto símbolo real: {e}"))?;

    let Some(symbol) = symbol else {
        return Ok((
            rust_i18n::t!("team_dossier.symbol_none").to_string(),
            "A equipe ainda não tem piloto com resultados registrados nesse recorte.".to_string(),
        ));
    };

    Ok((
        symbol.name,
        format!(
            "{}, {}, {} pela equipe.",
            count_label(symbol.races, "corrida", "corridas"),
            count_label(symbol.wins, "vitória", "vitórias"),
            count_label(symbol.podiums, "pódio", "pódios")
        ),
    ))
}

fn count_label(count: i32, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {plural}")
    }
}

fn best_real_season_points(facts: &[TeamRaceFact]) -> Option<(i32, f64)> {
    let mut points_by_season: BTreeMap<i32, f64> = BTreeMap::new();
    for fact in facts {
        *points_by_season.entry(fact.season_year).or_default() += fact.points;
    }
    points_by_season
        .into_iter()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
}

/// Etapa interna por categoria: temporada e ano de início/fim.
struct CategorySpan {
    category: String,
    start_season: i32,
    start_year: i32,
    end_year: i32,
}

/// Agrupa os fatos por categoria e ordena cronologicamente (por temporada de
/// estreia). Base compartilhada da escada (category_path) e do movimento.
fn category_spans(facts: &[TeamRaceFact]) -> Vec<CategorySpan> {
    let mut by_category: BTreeMap<String, (i32, i32, i32, i32)> = BTreeMap::new();
    for fact in facts {
        by_category
            .entry(fact.category.clone())
            .and_modify(|(start, end, start_year, end_year)| {
                if fact.season_number < *start {
                    *start = fact.season_number;
                    *start_year = fact.season_year;
                }
                if fact.season_number > *end {
                    *end = fact.season_number;
                    *end_year = fact.season_year;
                }
            })
            .or_insert((
                fact.season_number,
                fact.season_number,
                fact.season_year,
                fact.season_year,
            ));
    }
    let mut spans: Vec<CategorySpan> = by_category
        .into_iter()
        .map(
            |(category, (start, _end, start_year, end_year))| CategorySpan {
                category,
                start_season: start,
                start_year,
                end_year,
            },
        )
        .collect();
    spans.sort_by_key(|span| span.start_season);
    spans
}

fn build_real_category_path(facts: &[TeamRaceFact]) -> Vec<TeamHistoryCategoryStep> {
    let spans = category_spans(facts);
    let mut steps = Vec::new();
    let mut prev_tier: Option<u8> = None;
    for (index, span) in spans.iter().enumerate() {
        let tier = categories::get_category(&span.category).map(|config| config.tier);
        let movement = match (prev_tier, tier) {
            (None, _) => "start",
            (Some(prev), Some(current)) if current > prev => "promotion",
            (Some(prev), Some(current)) if current < prev => "relegation",
            _ => "same",
        };
        if tier.is_some() {
            prev_tier = tier;
        }
        let detail = match movement {
            "promotion" => "Promoção: subiu de categoria.".to_string(),
            "relegation" => "Rebaixamento: caiu de categoria.".to_string(),
            "start" => "Categoria de estreia da equipe.".to_string(),
            _ => "Permaneceu no mesmo nível.".to_string(),
        };
        let years = if span.start_year == span.end_year {
            span.start_year.to_string()
        } else {
            format!("{}-{}", span.start_year, span.end_year)
        };
        steps.push(TeamHistoryCategoryStep {
            category: team_history_category_label(&span.category),
            years,
            detail,
            color: history_palette(index),
            movement: movement.to_string(),
        });
    }
    steps
}

/// Resumo real de movimento entre categorias para a aba Categorias.
fn build_team_movement(facts: &[TeamRaceFact]) -> TeamHistoryMovement {
    let spans = category_spans(facts);

    // Promoções / rebaixamentos a partir das transições de tier.
    let mut promotions = 0;
    let mut relegations = 0;
    let mut prev_tier: Option<u8> = None;
    for span in &spans {
        if let Some(tier) = categories::get_category(&span.category).map(|config| config.tier) {
            if let Some(prev) = prev_tier {
                if tier > prev {
                    promotions += 1;
                } else if tier < prev {
                    relegations += 1;
                }
            }
            prev_tier = Some(tier);
        }
    }

    // Tempo por categoria (em temporadas), pela contagem de temporadas distintas.
    let mut seasons_by_category: BTreeMap<String, std::collections::HashSet<i32>> = BTreeMap::new();
    let mut wins_by_category: BTreeMap<String, (i32, i32)> = BTreeMap::new(); // (vitórias, corridas)
    for fact in facts {
        seasons_by_category
            .entry(fact.category.clone())
            .or_default()
            .insert(fact.season_number);
        let entry = wins_by_category
            .entry(fact.category.clone())
            .or_insert((0, 0));
        if fact.win {
            entry.0 += 1;
        }
        entry.1 += 1;
    }

    let time_by_category = spans
        .iter()
        .map(|span| {
            let years = seasons_by_category
                .get(&span.category)
                .map(|set| set.len())
                .unwrap_or(0);
            format!(
                "{}: {} {}",
                team_history_category_label(&span.category),
                years,
                if years == 1 { "ano" } else { "anos" }
            )
        })
        .collect::<Vec<_>>()
        .join(" · ");

    // Melhor / mais difícil categoria por taxa de vitória (mín. de corridas).
    let mut best: Option<(String, f64)> = None;
    let mut hardest: Option<(String, f64)> = None;
    for (category, (wins, races)) in &wins_by_category {
        if *races < 3 {
            continue;
        }
        let rate = *wins as f64 / *races as f64;
        if best.as_ref().map(|(_, r)| rate > *r).unwrap_or(true) {
            best = Some((team_history_category_label(category), rate));
        }
        if hardest.as_ref().map(|(_, r)| rate < *r).unwrap_or(true) {
            hardest = Some((team_history_category_label(category), rate));
        }
    }

    TeamHistoryMovement {
        promotions,
        relegations,
        time_by_category,
        best_category: best.map(|(c, _)| c).unwrap_or_else(|| "—".to_string()),
        hardest_category: hardest.map(|(c, _)| c).unwrap_or_else(|| "—".to_string()),
    }
}

fn percentage(numerator: i32, denominator: i32) -> i32 {
    if denominator <= 0 {
        0
    } else {
        ((numerator as f64 / denominator as f64) * 100.0).round() as i32
    }
}

fn rank_for_aggregate<F>(
    aggregates: &HashMap<String, TeamHistoryAggregate>,
    selected_team_id: &str,
    metric: F,
) -> String
where
    F: Fn(&TeamHistoryAggregate) -> f64,
{
    let mut ordered: Vec<(&String, f64)> = aggregates
        .iter()
        .map(|(team_id, aggregate)| (team_id, metric(aggregate)))
        .collect();
    ordered.sort_by(|(left_id, left), (right_id, right)| {
        right.total_cmp(left).then_with(|| left_id.cmp(right_id))
    });
    let rank = ordered
        .iter()
        .position(|(team_id, _)| team_id.as_str() == selected_team_id)
        .map(|index| index + 1)
        .unwrap_or(1);
    format_ordinal_i32(rank as i32)
}

fn rank_for_i32(values: &HashMap<String, Vec<TeamTitleFact>>, selected_team_id: &str) -> String {
    let mut ordered: Vec<(String, i32)> = values
        .iter()
        .map(|(team_id, titles)| (team_id.clone(), titles.len() as i32))
        .collect();
    if !values.contains_key(selected_team_id) {
        ordered.push((selected_team_id.to_string(), 0));
    }
    ordered.sort_by(|(left_id, left), (right_id, right)| {
        right.cmp(left).then_with(|| left_id.cmp(right_id))
    });
    let rank = ordered
        .iter()
        .position(|(team_id, _)| team_id.as_str() == selected_team_id)
        .map(|index| index + 1)
        .unwrap_or(1);
    format_ordinal_i32(rank as i32)
}

fn format_ordinal_i32(value: i32) -> String {
    format!("{value}º")
}

fn team_history_group_categories(category: &str) -> Vec<String> {
    match category {
        "mazda_rookie" | "mazda_amador" => {
            vec!["mazda_rookie".to_string(), "mazda_amador".to_string()]
        }
        "toyota_rookie" | "toyota_amador" => {
            vec!["toyota_rookie".to_string(), "toyota_amador".to_string()]
        }
        "bmw_m2" => vec!["bmw_m2".to_string()],
        "production_challenger" => vec![
            "mazda_rookie".to_string(),
            "mazda_amador".to_string(),
            "toyota_rookie".to_string(),
            "toyota_amador".to_string(),
            "bmw_m2".to_string(),
            "production_challenger".to_string(),
        ],
        "gt4" => vec!["gt4".to_string()],
        "gt3" => vec!["gt3".to_string()],
        "lmp2" => vec!["lmp2".to_string()],
        "endurance" => vec!["endurance".to_string()],
        other => vec![other.to_string()],
    }
}

fn team_history_group_label(category: &str) -> String {
    let key = match category {
        "mazda_rookie" | "mazda_amador" => "mazda",
        "toyota_rookie" | "toyota_amador" => "toyota",
        "bmw_m2" => "bmw",
        "production_challenger" => "production",
        "gt4" => "gt4",
        "gt3" => "gt3",
        "lmp2" => "lmp2",
        "endurance" => "endurance",
        _ => "generic",
    };
    let full = format!("career.group.{key}");
    rust_i18n::t!(&full).to_string()
}

fn team_history_category_label(category: &str) -> String {
    categories::get_category_config(category)
        .map(|config| config.nome_curto.to_string())
        .unwrap_or_else(|| category.to_string())
}

fn history_palette(index: usize) -> String {
    ["#58a6ff", "#f2c46d", "#5ee7a8", "#ff6b6b"][index % 4].to_string()
}
