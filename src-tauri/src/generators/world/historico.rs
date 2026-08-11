//! Mundo historico: grid inicial de uma linha do tempo que comeca no passado,
//! com anos de fundacao, faixas de performance e as equipes feeder das rookies.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use rand::Rng;

use super::pareamento::fill_category_grid;
use super::tipos::{HistoricalWorldData, LocalIdAllocator};
use crate::constants::categories::get_all_categories;
use crate::constants::historical_timeline::{
    apply_historical_performance_band, category_start_year, historical_team_foundation_year,
};
use crate::constants::teams::{get_team_templates, TeamTemplate};
use crate::models::driver::Driver;
use crate::models::team::{generate_teams_for_category, Team};

const HISTORICAL_AMATEUR_STARTING_TEAMS: usize = 6;

pub fn generate_historical_world(
    difficulty: &str,
    start_year: i32,
) -> Result<HistoricalWorldData, String> {
    let mut rng = rand::thread_rng();
    generate_historical_world_with_rng(difficulty, start_year, &mut rng)
}

pub(crate) fn generate_historical_world_with_rng<R: Rng>(
    difficulty: &str,
    start_year: i32,
    rng: &mut R,
) -> Result<HistoricalWorldData, String> {
    let mut ids = LocalIdAllocator::new();
    let mut existing_names = HashSet::new();
    let mut drivers = Vec::new();
    let mut teams = Vec::new();
    let mut contracts = Vec::new();

    for category in get_all_categories() {
        let mut team_id_generator = || ids.next_team_id();
        let mut category_teams =
            generate_teams_for_category(category.id, start_year, &mut team_id_generator);
        apply_historical_foundation_years(&mut category_teams, category.id);
        prepare_historical_development_grid(
            &mut category_teams,
            category.id,
            start_year,
            &mut ids,
            rng,
        );

        let total_slots = category_teams.len() * category.pilotos_por_equipe as usize;
        let mut driver_id_generator = || ids.next_driver_id();
        let ai_drivers = Driver::generate_for_category_with_id_factory(
            category.id,
            category.tier,
            difficulty,
            total_slots,
            // O ano do MUNDO, não o do relógio: num mundo que começa em 2000, o
            // `Local::now()` dava a todo piloto uma carreira iniciada no futuro dele.
            start_year.max(0) as u32,
            &mut existing_names,
            &mut driver_id_generator,
            rng,
        );

        // O mesmo laço do genesis, sem assento de jogador — ver `world/pareamento.rs`.
        let fill = fill_category_grid(
            category.id,
            category.tier,
            &mut category_teams,
            ai_drivers,
            None,
            &mut ids,
            rng,
        )?;
        drivers.extend(fill.drivers);
        contracts.extend(fill.contracts);

        teams.extend(category_teams);
    }

    // Modelo fechado: sem pool de agentes livres no genesis (ver generate_world_with_rng).

    Ok(HistoricalWorldData {
        drivers,
        teams,
        contracts,
    })
}

fn apply_historical_foundation_years(teams: &mut [Team], category_id: &str) {
    if teams.iter().any(|team| team.classe.is_some()) {
        apply_historical_multiclass_foundation_years(teams, category_id);
        return;
    }

    apply_historical_foundation_years_for_group(teams, category_id, None);
}

fn apply_historical_multiclass_foundation_years(teams: &mut [Team], category_id: &str) {
    let mut class_groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, team) in teams.iter().enumerate() {
        if let Some(class_name) = team.classe.as_deref() {
            class_groups
                .entry(class_name.to_string())
                .or_default()
                .push(index);
        }
    }

    for (class_name, indexes) in class_groups {
        let mut group: Vec<Team> = indexes.iter().map(|index| teams[*index].clone()).collect();
        apply_historical_foundation_years_for_group(&mut group, category_id, Some(&class_name));

        for (source, target_index) in group.into_iter().zip(indexes) {
            teams[target_index].ano_fundacao = source.ano_fundacao;
            teams[target_index].car_performance = source.car_performance;
        }
    }
}

fn apply_historical_foundation_years_for_group(
    teams: &mut [Team],
    category_id: &str,
    _class_name: Option<&str>,
) {
    let mut order: Vec<usize> = (0..teams.len()).collect();
    order.sort_by(|left, right| {
        teams[*right]
            .car_performance
            .total_cmp(&teams[*left].car_performance)
            .then_with(|| teams[*left].nome.cmp(&teams[*right].nome))
    });
    let total = order.len();

    for (rank_index, team_index) in order.into_iter().enumerate() {
        let team = &mut teams[team_index];
        team.ano_fundacao =
            historical_team_foundation_year(&team.nome, category_id, rank_index, total);
        apply_historical_performance_band(team);
    }
}

fn prepare_historical_development_grid<R: Rng>(
    teams: &mut Vec<Team>,
    category_id: &str,
    start_year: i32,
    ids: &mut LocalIdAllocator,
    rng: &mut R,
) {
    match category_id {
        "mazda_amador" | "toyota_amador" => {
            retain_historical_starting_amateur_teams(teams, category_id)
        }
        "mazda_rookie" => {
            add_historical_feeder_teams(teams, "mazda_amador", "mazda_rookie", start_year, ids, rng)
        }
        "toyota_rookie" => add_historical_feeder_teams(
            teams,
            "toyota_amador",
            "toyota_rookie",
            start_year,
            ids,
            rng,
        ),
        _ => {}
    }
}

fn retain_historical_starting_amateur_teams(teams: &mut Vec<Team>, category_id: &str) {
    let initial_names = initial_historical_amateur_team_names(category_id);
    let start_year = category_start_year(category_id);

    teams.retain(|team| initial_names.contains(team.nome.as_str()));
    for team in teams {
        team.ano_fundacao = start_year;
    }
}

fn add_historical_feeder_teams<R: Rng>(
    teams: &mut Vec<Team>,
    amateur_category: &str,
    rookie_category: &str,
    start_year: i32,
    ids: &mut LocalIdAllocator,
    rng: &mut R,
) {
    let initial_names = initial_historical_amateur_team_names(amateur_category);
    let mut feeder_templates: Vec<_> = ranked_team_templates(amateur_category)
        .into_iter()
        .filter(|template| !initial_names.contains(template.nome))
        .collect();
    feeder_templates.sort_by(compare_team_templates);

    let native_count = teams.len();
    let total = native_count + feeder_templates.len();
    for (offset, template) in feeder_templates.into_iter().enumerate() {
        let mut team = Team::from_template_with_rng(
            template,
            rookie_category,
            ids.next_team_id(),
            start_year,
            rng,
        );
        team.ano_fundacao = historical_team_foundation_year(
            &team.nome,
            rookie_category,
            native_count + offset,
            total,
        );
        apply_historical_performance_band(&mut team);
        teams.push(team);
    }
}

fn initial_historical_amateur_team_names(category_id: &str) -> HashSet<&'static str> {
    ranked_team_templates(category_id)
        .into_iter()
        .take(HISTORICAL_AMATEUR_STARTING_TEAMS)
        .map(|template| template.nome)
        .collect()
}

fn ranked_team_templates(category_id: &str) -> Vec<&'static TeamTemplate> {
    let mut templates = get_team_templates(category_id);
    templates.sort_by(compare_team_templates);
    templates
}

fn compare_team_templates(left: &&TeamTemplate, right: &&TeamTemplate) -> Ordering {
    right
        .car_performance_base
        .total_cmp(&left.car_performance_base)
        .then_with(|| left.nome.cmp(right.nome))
}
