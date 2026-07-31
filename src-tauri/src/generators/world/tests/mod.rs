use std::collections::{HashMap, HashSet};

use rand::{rngs::StdRng, SeedableRng};

use super::*;
use crate::constants::categories::{get_all_categories, runs_in_special_phase, uses_regular_teams};
use crate::constants::historical_timeline::team_start_year;
use crate::constants::teams::get_team_templates;
use crate::models::enums::ContractType;

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
    // 66 equipes regulares + 5 LMP2 sem feeder regular; Production/Endurance ainda sem templates.
    assert_eq!(world.teams.len(), 102);
    // Modelo fechado: apenas os 204 fundadores com contrato (grid). Sem pools.
    assert_eq!(world.drivers.len(), 204);
    // Apenas 132 contratos — categorias especiais não geram contratos
    assert_eq!(world.contracts.len(), 204);
}

#[test]
fn test_align_world_career_start_years_uses_game_seasons_not_age() {
    let mut world = sample_world();
    align_world_career_start_years(&mut world, 2024);

    let player = world
        .drivers
        .iter()
        .find(|driver| driver.is_jogador)
        .expect("player");
    assert_eq!(player.ano_inicio_carreira, 2024);

    for driver in &world.drivers {
        let expected = if driver.stats_carreira.temporadas == 0 {
            2024
        } else {
            2024_u32.saturating_sub(driver.stats_carreira.temporadas.saturating_sub(1))
        };
        assert_eq!(driver.ano_inicio_carreira, expected, "{}", driver.nome);
    }
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
        .filter(|team| uses_regular_teams(&team.categoria))
        .all(|team| team.piloto_1_id.is_some() && team.piloto_2_id.is_some()));
}

#[test]
fn test_real_special_teams_have_regular_lineups_without_special_activity() {
    let world = sample_world();

    let driver_map: HashMap<_, _> = world
        .drivers
        .iter()
        .map(|driver| (driver.id.clone(), driver))
        .collect();

    for team in world
        .teams
        .iter()
        .filter(|team| runs_in_special_phase(&team.categoria))
    {
        let piloto_1_id = team
            .piloto_1_id
            .as_ref()
            .expect("real special team should have piloto_1");
        let piloto_2_id = team
            .piloto_2_id
            .as_ref()
            .expect("real special team should have piloto_2");

        for pilot_id in [piloto_1_id, piloto_2_id] {
            let driver = driver_map
                .get(pilot_id)
                .expect("special lineup driver should exist");
            assert_eq!(
                driver.categoria_atual.as_deref(),
                Some(team.categoria.as_str())
            );
            assert!(
                driver.categoria_especial_ativa.is_none(),
                "{} should not start with categoria_especial_ativa",
                driver.nome
            );
        }
    }

    assert!(world
        .teams
        .iter()
        .filter(|team| runs_in_special_phase(&team.categoria))
        .all(|team| team.classe.is_some()));
}

#[test]
fn test_generate_world_initial_regular_contracts_for_real_special_teams() {
    let world = sample_world();

    assert_eq!(
        world
            .contracts
            .iter()
            .filter(|contract| contract.tipo == ContractType::Regular)
            .count(),
        204
    );
    assert_eq!(
        world
            .contracts
            .iter()
            .filter(|contract| contract.tipo == ContractType::Especial)
            .count(),
        0
    );

    assert_eq!(
        count_contracts_by_category(&world, "production_challenger"),
        36
    );
    assert_eq!(
        count_contracts_by_category_and_class(&world, "production_challenger", "mazda"),
        12
    );
    assert_eq!(
        count_contracts_by_category_and_class(&world, "production_challenger", "toyota"),
        12
    );
    assert_eq!(
        count_contracts_by_category_and_class(&world, "production_challenger", "bmw"),
        12
    );

    assert_eq!(count_contracts_by_category(&world, "endurance"), 36);
    assert_eq!(
        count_contracts_by_category_and_class(&world, "endurance", "gt4"),
        12
    );
    assert_eq!(
        count_contracts_by_category_and_class(&world, "endurance", "gt3"),
        12
    );
    assert_eq!(
        count_contracts_by_category_and_class(&world, "endurance", "lmp2"),
        12
    );
    assert_eq!(count_contracts_by_category(&world, "lmp2"), 0);

    let regular_pilot_ids: Vec<_> = world
        .contracts
        .iter()
        .filter(|contract| contract.tipo == ContractType::Regular)
        .map(|contract| contract.piloto_id.clone())
        .collect();
    let unique_regular_pilot_ids: HashSet<_> = regular_pilot_ids.iter().cloned().collect();
    assert_eq!(unique_regular_pilot_ids.len(), regular_pilot_ids.len());

    for team in world
        .teams
        .iter()
        .filter(|team| team.categoria == "production_challenger" || team.categoria == "endurance")
    {
        assert_eq!(
            world
                .contracts
                .iter()
                .filter(|contract| {
                    contract.tipo == ContractType::Regular
                        && contract.equipe_id == team.id
                        && contract.categoria == team.categoria
                        && contract.classe == team.classe
                })
                .count(),
            2,
            "{} should have exactly two matching regular contracts",
            team.nome
        );
    }
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

    // Equipes persistentes reais devem nascer com hierarquia inicial.
    for team in world
        .teams
        .iter()
        .filter(|t| uses_regular_teams(&t.categoria))
    {
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
        if category.id == "endurance" {
            assert_eq!(count, get_team_templates(category.id).len());
        } else if uses_regular_teams(category.id) {
            assert_eq!(count, category.num_equipes as usize);
        } else {
            assert_eq!(count, get_team_templates(category.id).len());
        }
    }
}

#[test]
fn test_generate_world_persists_real_special_rosters_by_class() {
    let world = sample_world();

    assert_eq!(count_teams_by_category(&world, "production_challenger"), 18);
    assert_eq!(
        count_teams_by_category_and_class(&world, "production_challenger", "mazda"),
        6
    );
    assert_eq!(
        count_teams_by_category_and_class(&world, "production_challenger", "toyota"),
        6
    );
    assert_eq!(
        count_teams_by_category_and_class(&world, "production_challenger", "bmw"),
        6
    );

    assert_eq!(count_teams_by_category(&world, "endurance"), 18);
    assert_eq!(
        count_teams_by_category_and_class(&world, "endurance", "gt4"),
        6
    );
    assert_eq!(
        count_teams_by_category_and_class(&world, "endurance", "gt3"),
        6
    );
    assert_eq!(
        count_teams_by_category_and_class(&world, "endurance", "lmp2"),
        6
    );

    assert_eq!(count_teams_by_category(&world, "lmp2"), 0);
    assert!(world.teams.iter().any(|team| team.categoria == "endurance"
        && team.classe.as_deref() == Some("lmp2")
        && team.nome == "Meridian"));
}

#[test]
fn test_generate_historical_world_assigns_timeline_foundation_years() {
    let mut rng = StdRng::seed_from_u64(20260426);
    let world = generate_historical_world_with_rng("medio", 2000, &mut rng)
        .expect("historical world should generate");

    let mazda_rookie_teams: Vec<_> = world
        .teams
        .iter()
        .filter(|team| team.categoria == "mazda_rookie")
        .collect();
    assert_eq!(mazda_rookie_teams.len(), 10);
    assert!(mazda_rookie_teams
        .iter()
        .all(|team| (2020..=2024).contains(&team.ano_fundacao)));
    assert!(mazda_rookie_teams
        .iter()
        .any(|team| team.nome == "Amateur Hour Racing"));

    let mazda_cup_teams: Vec<_> = world
        .teams
        .iter()
        .filter(|team| team.categoria == "mazda_amador")
        .collect();
    assert_eq!(mazda_cup_teams.len(), 6);
    assert!(mazda_cup_teams.iter().all(|team| team.ano_fundacao == 2016));
    assert!(!mazda_cup_teams
        .iter()
        .any(|team| team.nome == "Amateur Hour Racing"));

    let toyota_rookie_teams: Vec<_> = world
        .teams
        .iter()
        .filter(|team| team.categoria == "toyota_rookie")
        .collect();
    assert_eq!(toyota_rookie_teams.len(), 10);
    assert!(toyota_rookie_teams
        .iter()
        .all(|team| (2020..=2024).contains(&team.ano_fundacao)));

    let toyota_cup_teams: Vec<_> = world
        .teams
        .iter()
        .filter(|team| team.categoria == "toyota_amador")
        .collect();
    assert_eq!(toyota_cup_teams.len(), 6);
    assert!(toyota_cup_teams
        .iter()
        .all(|team| team.ano_fundacao == 2012));

    let ferrari = world
        .teams
        .iter()
        .find(|team| team.nome == "Ferrari")
        .expect("Ferrari should exist");
    let obsidian = world
        .teams
        .iter()
        .find(|team| team.nome == "Obsidian")
        .expect("fictional GT3 team should exist");
    assert_eq!(ferrari.ano_fundacao, 1929);
    assert!(obsidian.ano_fundacao > 1999);
}

#[test]
fn test_historical_world_respects_special_class_start_years() {
    let mut rng = StdRng::seed_from_u64(20260610);
    let world = generate_historical_world_with_rng("medio", 2000, &mut rng)
        .expect("historical world should generate");

    assert_eq!(
        min_team_start_year(&world, "endurance", Some("gt3")),
        Some(2005)
    );
    assert_eq!(
        min_team_start_year(&world, "endurance", Some("gt4")),
        Some(2007)
    );
    assert_eq!(
        min_team_start_year(&world, "endurance", Some("lmp2")),
        Some(2004)
    );
    assert_eq!(min_team_start_year(&world, "lmp2", None), None);
    assert_eq!(
        min_team_start_year(&world, "production_challenger", Some("mazda")),
        Some(2018)
    );
    assert_eq!(
        min_team_start_year(&world, "production_challenger", Some("toyota")),
        Some(2018)
    );
    assert_eq!(
        min_team_start_year(&world, "production_challenger", Some("bmw")),
        Some(2018)
    );
}

#[test]
fn test_historical_world_categories_start_with_at_least_five_teams() {
    let mut rng = StdRng::seed_from_u64(20260508);
    let world = generate_historical_world_with_rng("medio", 2000, &mut rng)
        .expect("historical world should generate");

    for category in get_all_categories() {
        let category_teams: Vec<_> = world
            .teams
            .iter()
            .filter(|team| team.categoria == category.id)
            .collect();
        if category_teams.is_empty() {
            continue;
        }

        let start_year = crate::constants::historical_timeline::category_start_year(category.id);
        let active_at_start = category_teams
            .iter()
            .filter(|team| team.ano_fundacao <= start_year)
            .count();
        assert!(
            active_at_start >= 5,
            "{} nasceu em {} com apenas {} equipe(s)",
            category.id,
            start_year,
            active_at_start
        );
    }
}

#[test]
fn test_historical_world_non_rookie_categories_start_with_experienced_drivers() {
    let mut rng = StdRng::seed_from_u64(20260429);
    let world = generate_historical_world_with_rng("medio", 2000, &mut rng)
        .expect("historical world should generate");

    assert!(world.drivers.iter().any(|driver| {
        matches!(
            driver.categoria_atual.as_deref(),
            Some("mazda_rookie" | "toyota_rookie")
        ) && driver.stats_carreira.corridas == 0
    }));

    for driver in world.drivers.iter().filter(|driver| {
        !matches!(
            driver.categoria_atual.as_deref(),
            None | Some("mazda_rookie" | "toyota_rookie")
        )
    }) {
        assert!(
            driver.stats_carreira.corridas > 0,
            "{} nasceu em {:?} sem corridas de carreira",
            driver.nome,
            driver.categoria_atual
        );
        assert!(
            driver.corridas_na_categoria > 0,
            "{} nasceu em {:?} sem corridas na categoria",
            driver.nome,
            driver.categoria_atual
        );
    }
}

fn count_teams_by_category(world: &WorldData, category: &str) -> usize {
    world
        .teams
        .iter()
        .filter(|team| team.categoria == category)
        .count()
}

fn count_teams_by_category_and_class(world: &WorldData, category: &str, class_name: &str) -> usize {
    world
        .teams
        .iter()
        .filter(|team| team.categoria == category && team.classe.as_deref() == Some(class_name))
        .count()
}

fn count_contracts_by_category(world: &WorldData, category: &str) -> usize {
    world
        .contracts
        .iter()
        .filter(|contract| contract.categoria == category)
        .count()
}

fn count_contracts_by_category_and_class(
    world: &WorldData,
    category: &str,
    class_name: &str,
) -> usize {
    world
        .contracts
        .iter()
        .filter(|contract| {
            contract.categoria == category && contract.classe.as_deref() == Some(class_name)
        })
        .count()
}

fn min_team_start_year(
    world: &HistoricalWorldData,
    category: &str,
    class_name: Option<&str>,
) -> Option<i32> {
    world
        .teams
        .iter()
        .filter(|team| {
            team.categoria == category
                && (class_name.is_none() || team.classe.as_deref() == class_name)
        })
        .map(team_start_year)
        .min()
}
