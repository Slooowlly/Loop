//! Dossie historico e relatorio financeiro de uma equipe.
//!
//! Extraido de `career.rs`: agregacoes de corrida, marcos, timeline, rivais,
//! streaks e os rotulos/formatadores usados so por esse subsistema.
//!
//! Fachada: a montagem do dossie vive aqui; as pecas moram nos submodulos de
//! `career_team_dossier/`.

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

#[path = "career_team_dossier/categorias.rs"]
mod categorias;
#[path = "career_team_dossier/esportivo.rs"]
mod esportivo;
#[path = "career_team_dossier/fatos.rs"]
mod fatos;
#[path = "career_team_dossier/financas.rs"]
mod financas;
#[path = "career_team_dossier/gestao.rs"]
mod gestao;
#[path = "career_team_dossier/identidade.rs"]
mod identidade;
#[path = "career_team_dossier/rotulos.rs"]
mod rotulos;

pub(crate) use financas::*;
// Consumidos pela propria fachada e pelos irmaos, via `use super::*`.
use categorias::*;
use esportivo::*;
use fatos::*;
use gestao::*;
use identidade::*;
use rotulos::*;

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
