//! Genesis da carreira do jogador: monta o mundo do ano 1 com o jogador
//! encaixado na equipe escolhida e alinha os anos de inicio de carreira.

use std::cmp::Ordering;
use std::collections::HashSet;

use rand::seq::SliceRandom;
use rand::Rng;

use super::tipos::{contract_with_team_class, LocalIdAllocator, WorldData};
use crate::constants::categories::{get_all_categories, get_category_config};
use crate::models::contract::generate_initial_contract;
use crate::models::driver::Driver;
use crate::models::enums::TeamRole;
use crate::models::team::generate_teams_for_category;

pub fn generate_world(
    player_name: &str,
    player_nationality: &str,
    player_age: i32,
    player_category: &str,
    player_team_index: usize,
    difficulty: &str,
) -> Result<WorldData, String> {
    let mut rng = rand::thread_rng();
    generate_world_with_rng(
        player_name,
        player_nationality,
        player_age,
        player_category,
        player_team_index,
        difficulty,
        &mut rng,
    )
}

pub(crate) fn generate_world_with_rng<R: Rng>(
    player_name: &str,
    player_nationality: &str,
    player_age: i32,
    player_category: &str,
    player_team_index: usize,
    difficulty: &str,
    rng: &mut R,
) -> Result<WorldData, String> {
    if !matches!(player_category, "mazda_rookie" | "toyota_rookie") {
        return Err("player_category must be mazda_rookie or toyota_rookie".to_string());
    }

    if get_category_config(player_category).is_none() {
        return Err(format!("Unknown player category: {player_category}"));
    }

    let mut ids = LocalIdAllocator::new();
    let mut existing_names = HashSet::new();
    existing_names.insert(player_name.to_string());

    let mut player = Driver::create_player(
        ids.next_driver_id(),
        player_name.to_string(),
        player_nationality.to_string(),
        player_age,
    );
    player.categoria_atual = Some(player_category.to_string());

    let player_id = player.id.clone();
    let player_name_owned = player.nome.clone();

    let mut drivers = vec![player.clone()];
    let mut teams = Vec::new();
    let mut contracts = Vec::new();
    let mut player_team_id = None;
    let mut player_contract = None;

    for category in get_all_categories() {
        let mut team_id_generator = || ids.next_team_id();
        let mut category_teams =
            generate_teams_for_category(category.id, 1, &mut team_id_generator);

        // Production/Endurance ainda rodam pelo fluxo especial legado, mas suas
        // equipes reais ja nascem com lineup e contratos regulares iniciais.
        let selected_player_team_id = if category.id == player_category {
            if player_team_index >= category_teams.len() {
                return Err(format!(
                    "player_team_index {} is invalid for category {}",
                    player_team_index, player_category
                ));
            }
            Some(category_teams[player_team_index].id.clone())
        } else {
            None
        };

        let total_slots = category_teams.len() * 2;
        let ai_needed = if selected_player_team_id.is_some() {
            total_slots.saturating_sub(1)
        } else {
            total_slots
        };

        let mut driver_id_generator = || ids.next_driver_id();
        let mut ai_drivers = Driver::generate_for_category_with_id_factory(
            category.id,
            category.tier,
            difficulty,
            ai_needed,
            &mut existing_names,
            &mut driver_id_generator,
            rng,
        );

        ai_drivers.sort_by(|left, right| {
            right
                .atributos
                .skill
                .total_cmp(&left.atributos.skill)
                .then_with(|| left.nome.cmp(&right.nome))
        });

        let team_count = category_teams.len();
        let mut n1_pool = ai_drivers.into_iter();
        let n1_drivers: Vec<Driver> = n1_pool.by_ref().take(team_count).collect();
        let mut n2_drivers = n1_pool;

        let mut team_order: Vec<usize> = (0..category_teams.len()).collect();
        team_order.sort_by(|left, right| {
            category_teams[*right]
                .car_performance
                .total_cmp(&category_teams[*left].car_performance)
                .then(Ordering::Equal)
        });

        // Anti-"sempre a mesma equipe": nas categorias SPEC (rookie), o carro não
        // afeta o resultado (todos idênticos na sim), então casar o melhor piloto com
        // o time de maior `car_performance` só serve pra entregar o ás pro mesmo time
        // (o de maior template) em TODO save novo — que então vence a 1ª rookie e sobe
        // a escada, roteirizando o começo de cada carreira. Embaralhando a ordem, o
        // melhor talento rookie vai pra um time aleatório e qual equipe desponta varia
        // por save. Fora da rookie o pareamento melhor-piloto↔melhor-carro é mantido.
        if category.tier == 0 {
            team_order.shuffle(rng);
        }

        for (rank, team_index) in team_order.into_iter().enumerate() {
            let team = &mut category_teams[team_index];
            let n1_driver = n1_drivers
                .get(rank)
                .cloned()
                .ok_or_else(|| format!("Missing N1 driver for team {}", team.id))?;

            let is_player_team = selected_player_team_id
                .as_ref()
                .map(|selected| selected == &team.id)
                .unwrap_or(false);

            team.piloto_1_id = Some(n1_driver.id.clone());
            team.hierarquia_n1_id = Some(n1_driver.id.clone());
            team.hierarquia_status = "estavel".to_string();
            team.hierarquia_tensao = 0.0;
            team.is_player_team = is_player_team;

            drivers.push(n1_driver.clone());
            contracts.push(contract_with_team_class(
                generate_initial_contract(
                    ids.next_contract_id(),
                    &n1_driver.id,
                    &n1_driver.nome,
                    &team.id,
                    &team.nome,
                    TeamRole::Numero1,
                    category.id,
                    1,
                ),
                team,
            ));

            if is_player_team {
                team.piloto_2_id = Some(player_id.clone());
                if player.atributos.skill > n1_driver.atributos.skill {
                    team.hierarquia_n1_id = Some(player_id.clone());
                    team.hierarquia_n2_id = Some(n1_driver.id.clone());
                } else {
                    team.hierarquia_n2_id = Some(player_id.clone());
                }
                player_team_id = Some(team.id.clone());

                let contract = contract_with_team_class(
                    generate_initial_contract(
                        ids.next_contract_id(),
                        &player_id,
                        &player_name_owned,
                        &team.id,
                        &team.nome,
                        TeamRole::Numero2,
                        category.id,
                        1,
                    ),
                    team,
                );
                player_contract = Some(contract.clone());
                contracts.push(contract);
            } else {
                let n2_driver = n2_drivers
                    .next()
                    .ok_or_else(|| format!("Missing N2 driver for team {}", team.id))?;

                team.piloto_2_id = Some(n2_driver.id.clone());
                team.hierarquia_n2_id = Some(n2_driver.id.clone());

                drivers.push(n2_driver.clone());
                contracts.push(contract_with_team_class(
                    generate_initial_contract(
                        ids.next_contract_id(),
                        &n2_driver.id,
                        &n2_driver.nome,
                        &team.id,
                        &team.nome,
                        TeamRole::Numero2,
                        category.id,
                        1,
                    ),
                    team,
                ));
            }
        }

        teams.extend(category_teams);
    }

    // Modelo fechado: nada de pool de agentes livres no genesis. Os grids nascem
    // preenchidos pelos fundadores; daí em diante o mercado preenche vagas pela
    // escada (promoção da categoria de baixo) e só gera rookies na base.

    let player_team_id =
        player_team_id.ok_or_else(|| "Player team was not assigned".to_string())?;
    let player_contract =
        player_contract.ok_or_else(|| "Player contract was not generated".to_string())?;

    Ok(WorldData {
        drivers,
        teams,
        contracts,
        player,
        player_team_id,
        player_contract,
    })
}

pub(crate) fn align_world_career_start_years(world: &mut WorldData, current_year: u32) {
    for driver in &mut world.drivers {
        driver.ano_inicio_carreira = inferred_career_start_year(driver, current_year);
    }

    if let Some(player) = world
        .drivers
        .iter()
        .find(|driver| driver.id == world.player.id)
        .cloned()
    {
        world.player = player;
    }
}

fn inferred_career_start_year(driver: &Driver, current_year: u32) -> u32 {
    let career_seasons = driver.stats_carreira.temporadas;
    if career_seasons == 0 {
        current_year
    } else {
        current_year.saturating_sub(career_seasons.saturating_sub(1))
    }
}
