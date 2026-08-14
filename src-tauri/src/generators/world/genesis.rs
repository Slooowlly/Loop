//! Genesis da carreira do jogador: monta o mundo do ano 1 com o jogador
//! encaixado na equipe escolhida e alinha os anos de inicio de carreira.

use std::collections::HashSet;

use rand::Rng;

use super::pareamento::{fill_category_grid, PlayerSlot};
use super::tipos::{LocalIdAllocator, WorldData};
use crate::constants::categories::{get_all_categories, get_category_config};
use crate::generators::teams::generate_teams_for_category;
use crate::models::driver::Driver;

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
        let ai_drivers = Driver::generate_for_category_with_id_factory(
            category.id,
            category.tier,
            difficulty,
            ai_needed,
            // O genesis é a carreira que começa AGORA: o ano do mundo é o ano civil, o
            // mesmo que `Driver::create_player` usa para o jogador. Em seguida
            // `align_world_career_start_years` reescreve tudo pelas temporadas jogadas —
            // isto é só o ponto de partida coerente. O mundo HISTÓRICO, que é onde os
            // dois anos divergem, passa o ano dele (ver `historico.rs`).
            crate::common::time::current_year(),
            &mut existing_names,
            &mut driver_id_generator,
            rng,
        );

        // O laço de pareamento é o MESMO do mundo histórico (`world/pareamento.rs`); o
        // que só existe aqui é o assento do jogador.
        let fill = fill_category_grid(
            category.id,
            category.tier,
            &mut category_teams,
            ai_drivers,
            selected_player_team_id
                .as_deref()
                .map(|team_id| PlayerSlot {
                    team_id,
                    driver_id: &player_id,
                    driver_name: &player_name_owned,
                    skill: player.atributos.skill,
                }),
            &mut ids,
            rng,
        )?;
        drivers.extend(fill.drivers);
        contracts.extend(fill.contracts);
        if let Some(id) = fill.player_team_id {
            player_team_id = Some(id);
        }
        if let Some(contract) = fill.player_contract {
            player_contract = Some(contract);
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
