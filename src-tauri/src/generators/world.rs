#![allow(dead_code)]

use std::cmp::Ordering;
use std::collections::HashSet;

use rand::Rng;

use crate::constants::categories::{get_all_categories, get_category_config, is_especial};
use crate::models::contract::{generate_initial_contract, Contract};
use crate::models::driver::Driver;
use crate::models::enums::TeamRole;
use crate::models::team::{generate_teams_for_category, Team};

#[derive(Debug, Clone)]
pub struct WorldData {
    pub drivers: Vec<Driver>,
    pub teams: Vec<Team>,
    pub contracts: Vec<Contract>,
    pub player: Driver,
    pub player_team_id: String,
    pub player_contract: Contract,
}

#[derive(Debug, Clone)]
pub struct HistoricalWorldData {
    pub drivers: Vec<Driver>,
    pub teams: Vec<Team>,
    pub contracts: Vec<Contract>,
}

#[derive(Debug, Default)]
struct LocalIdAllocator {
    next_driver: u32,
    next_team: u32,
    next_contract: u32,
}

impl LocalIdAllocator {
    fn new() -> Self {
        Self {
            next_driver: 1,
            next_team: 1,
            next_contract: 1,
        }
    }

    fn next_driver_id(&mut self) -> String {
        let id = format!("P{:03}", self.next_driver);
        self.next_driver += 1;
        id
    }

    fn next_team_id(&mut self) -> String {
        let id = format!("T{:03}", self.next_team);
        self.next_team += 1;
        id
    }

    fn next_contract_id(&mut self) -> String {
        let id = format!("C{:03}", self.next_contract);
        self.next_contract += 1;
        id
    }
}

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

        if is_especial(category.id) {
            category_teams.retain(|team| {
                team.classe
                    .as_deref()
                    .and_then(|class_name| {
                        category
                            .classes
                            .iter()
                            .find(|class| class.class_name == class_name)
                    })
                    .is_some_and(|class| get_category_config(class.car_categoria).is_none())
            });
            teams.extend(category_teams);
            continue;
        }

        let total_slots = category_teams.len() * category.pilotos_por_equipe as usize;
        let mut driver_id_generator = || ids.next_driver_id();
        let mut ai_drivers = Driver::generate_for_category_with_id_factory(
            category.id,
            category.tier,
            difficulty,
            total_slots,
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

        for (rank, team_index) in team_order.into_iter().enumerate() {
            let team = &mut category_teams[team_index];
            let n1_driver = n1_drivers
                .get(rank)
                .cloned()
                .ok_or_else(|| format!("Missing N1 driver for team {}", team.id))?;
            let n2_driver = n2_drivers
                .next()
                .ok_or_else(|| format!("Missing N2 driver for team {}", team.id))?;

            team.piloto_1_id = Some(n1_driver.id.clone());
            team.piloto_2_id = Some(n2_driver.id.clone());
            team.hierarquia_n1_id = Some(n1_driver.id.clone());
            team.hierarquia_n2_id = Some(n2_driver.id.clone());
            team.hierarquia_status = "estavel".to_string();
            team.hierarquia_tensao = 0.0;
            team.is_player_team = false;

            drivers.push(n1_driver.clone());
            contracts.push(generate_initial_contract(
                ids.next_contract_id(),
                &n1_driver.id,
                &n1_driver.nome,
                &team.id,
                &team.nome,
                TeamRole::Numero1,
                category.id,
                1,
            ));

            drivers.push(n2_driver.clone());
            contracts.push(generate_initial_contract(
                ids.next_contract_id(),
                &n2_driver.id,
                &n2_driver.nome,
                &team.id,
                &team.nome,
                TeamRole::Numero2,
                category.id,
                1,
            ));
        }

        teams.extend(category_teams);
    }

    let pool_drivers = generate_specialist_pool(&mut ids, &mut existing_names, difficulty, rng);
    drivers.extend(pool_drivers);

    Ok(HistoricalWorldData {
        drivers,
        teams,
        contracts,
    })
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

        // Categorias especiais existem desde o início do ano, mas montam lineup
        // apenas na janela de convocação (Passos 6+). Geram equipes sem pilotos.
        if is_especial(category.id) {
            category_teams.retain(|team| {
                team.classe
                    .as_deref()
                    .and_then(|class_name| {
                        category
                            .classes
                            .iter()
                            .find(|class| class.class_name == class_name)
                    })
                    .is_some_and(|class| get_category_config(class.car_categoria).is_none())
            });
            teams.extend(category_teams);
            continue;
        }

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
            contracts.push(generate_initial_contract(
                ids.next_contract_id(),
                &n1_driver.id,
                &n1_driver.nome,
                &team.id,
                &team.nome,
                TeamRole::Numero1,
                category.id,
                1,
            ));

            if is_player_team {
                team.piloto_2_id = Some(player_id.clone());
                team.hierarquia_n2_id = Some(player_id.clone());
                player_team_id = Some(team.id.clone());

                let contract = generate_initial_contract(
                    ids.next_contract_id(),
                    &player_id,
                    &player_name_owned,
                    &team.id,
                    &team.nome,
                    TeamRole::Numero2,
                    category.id,
                    1,
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
                contracts.push(generate_initial_contract(
                    ids.next_contract_id(),
                    &n2_driver.id,
                    &n2_driver.nome,
                    &team.id,
                    &team.nome,
                    TeamRole::Numero2,
                    category.id,
                    1,
                ));
            }
        }

        teams.extend(category_teams);
    }

    // Gerar pool de especialistas livres para convocação especial de meio de ano.
    // Para cada categoria especial, gerar drivers por classe com categoria_atual
    // apontando para a categoria regular de referência da classe (sem equipe nem contrato).
    let pool_drivers = generate_specialist_pool(&mut ids, &mut existing_names, difficulty, rng);
    drivers.extend(pool_drivers);

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

/// Gera pilotos livres para o pool de convocação especial.
/// Para cada categoria especial, itera sobre suas classes e gera drivers
/// usando a categoria regular de referência da classe como `categoria_atual`.
/// Os drivers não têm equipe nem contrato — ficam disponíveis para a janela
/// de convocação de meio de ano (Passos 6+).
fn generate_specialist_pool<R: Rng>(
    ids: &mut LocalIdAllocator,
    existing_names: &mut HashSet<String>,
    difficulty: &str,
    rng: &mut R,
) -> Vec<Driver> {
    let mut pool = Vec::new();

    for category in get_all_categories().iter().filter(|c| is_especial(c.id)) {
        for class in category.classes {
            let count = (class.num_equipes * category.pilotos_por_equipe) as usize;
            let ref_tier = get_category_config(class.car_categoria)
                .map(|c| c.tier)
                .unwrap_or(2);

            let mut driver_id_gen = || ids.next_driver_id();
            // Os drivers são gerados com categoria_atual = Some(class.car_categoria)
            // definida dentro de generate_for_category_with_id_factory.
            let mut class_drivers = Driver::generate_for_category_with_id_factory(
                class.car_categoria,
                ref_tier,
                difficulty,
                count,
                existing_names,
                &mut driver_id_gen,
                rng,
            );
            // Pilotos do pool são livres — sem contrato e sem categoria ativa.
            // categoria_atual é limpa para evitar que apareçam nas simulações regulares.
            // Seu nível de habilidade reflete o tier de referência da classe.
            for driver in &mut class_drivers {
                driver.categoria_atual = None;
            }
            pool.extend(class_drivers);
        }
    }

    pool
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use rand::{rngs::StdRng, SeedableRng};

    use super::*;
    use crate::constants::categories::{get_all_categories, is_especial};

    fn sample_world() -> WorldData {
        let mut rng = StdRng::seed_from_u64(20260318);
        generate_world_with_rng(
            "Lucas Teste",
            "🇧🇷 Brasileiro",
            20,
            "mazda_rookie",
            2,
            "medio",
            &mut rng,
        )
        .expect("world generation should succeed")
    }

    #[test]
    fn test_generate_world_total_counts() {
        let world = sample_world();
        // 66 equipes regulares + 5 LMP2 sem feeder regular.
        assert_eq!(world.teams.len(), 71);
        // 196 pilotos: 132 com contrato (regular) + 64 no pool (livres)
        // production_challenger: 15 equipes × 2 = 30; endurance: 17 equipes × 2 = 34 → 64 pool
        assert_eq!(world.drivers.len(), 196);
        // Apenas 132 contratos — categorias especiais não geram contratos
        assert_eq!(world.contracts.len(), 132);
    }

    #[test]
    fn test_generate_world_player_in_correct_team() {
        let world = sample_world();
        let team = world
            .teams
            .iter()
            .find(|team| team.id == world.player_team_id)
            .expect("player team must exist");

        assert!(
            team.piloto_1_id.as_deref() == Some(world.player.id.as_str())
                || team.piloto_2_id.as_deref() == Some(world.player.id.as_str())
        );
        assert_eq!(team.piloto_2_id.as_deref(), Some(world.player.id.as_str()));
    }

    #[test]
    fn test_regular_teams_have_two_pilots() {
        let world = sample_world();
        assert!(world
            .teams
            .iter()
            .filter(|team| !is_especial(&team.categoria))
            .all(|team| team.piloto_1_id.is_some() && team.piloto_2_id.is_some()));
    }

    #[test]
    fn test_special_teams_have_no_pilots() {
        let world = sample_world();
        assert!(world
            .teams
            .iter()
            .filter(|team| is_especial(&team.categoria))
            .all(|team| team.piloto_1_id.is_none() && team.piloto_2_id.is_none()));
        assert!(world
            .teams
            .iter()
            .filter(|team| is_especial(&team.categoria))
            .all(|team| team.classe.as_deref() == Some("lmp2")));
    }

    #[test]
    fn test_generate_world_no_duplicate_names() {
        let world = sample_world();
        let unique_names: HashSet<_> = world
            .drivers
            .iter()
            .map(|driver| driver.nome.clone())
            .collect();
        assert_eq!(unique_names.len(), world.drivers.len());
    }

    #[test]
    fn test_generate_world_no_pilot_in_two_teams() {
        let world = sample_world();
        let mut seen = HashSet::new();

        for team in &world.teams {
            for pilot_id in [team.piloto_1_id.as_ref(), team.piloto_2_id.as_ref()]
                .into_iter()
                .flatten()
            {
                assert!(seen.insert(pilot_id.clone()));
            }
        }
    }

    #[test]
    fn test_generate_world_contracts_match_teams() {
        let world = sample_world();
        let team_map: HashMap<_, _> = world
            .teams
            .iter()
            .map(|team| (team.id.clone(), team))
            .collect();

        for contract in world
            .contracts
            .iter()
            .filter(|contract| contract.is_ativo())
        {
            let team = team_map
                .get(&contract.equipe_id)
                .expect("contract team should exist");
            assert!(
                team.piloto_1_id.as_deref() == Some(contract.piloto_id.as_str())
                    || team.piloto_2_id.as_deref() == Some(contract.piloto_id.as_str())
            );
        }
    }

    #[test]
    fn test_generate_world_hierarchy_set() {
        let world = sample_world();
        let driver_map: HashMap<_, _> = world
            .drivers
            .iter()
            .map(|driver| (driver.id.clone(), driver))
            .collect();

        // Equipes especiais não têm lineup — ignorar hierarquia delas neste teste.
        for team in world.teams.iter().filter(|t| !is_especial(&t.categoria)) {
            let n1_id = team.hierarquia_n1_id.as_ref().expect("n1 id should be set");
            let n2_id = team.hierarquia_n2_id.as_ref().expect("n2 id should be set");
            let n1 = driver_map.get(n1_id).expect("n1 driver should exist");
            let n2 = driver_map.get(n2_id).expect("n2 driver should exist");

            assert!(n1.atributos.skill >= n2.atributos.skill);
        }
    }

    #[test]
    fn test_generate_world_all_categories_populated() {
        let world = sample_world();

        for category in get_all_categories() {
            let count = world
                .teams
                .iter()
                .filter(|team| team.categoria == category.id)
                .count();
            if is_especial(category.id) {
                let persistent_special_teams = category
                    .classes
                    .iter()
                    .filter(|class| get_category_config(class.car_categoria).is_none())
                    .map(|class| class.num_equipes as usize)
                    .sum::<usize>();
                assert_eq!(count, persistent_special_teams);
            } else {
                assert_eq!(count, category.num_equipes as usize);
            }
        }
    }
}
