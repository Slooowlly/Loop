use rand::Rng;
use rusqlite::Connection;

use std::collections::HashSet;

use crate::constants::categories::is_especial;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::teams as team_queries;
use crate::promotion::block1::execute_block1;
use crate::promotion::block2::execute_block2_with_exclusions;
use crate::promotion::block3::execute_block3;
use crate::promotion::effects::{
    apply_attribute_deltas, calculate_promotion_effects, calculate_relegation_effects,
};
use crate::promotion::pilots::{apply_pilot_effect, resolve_pilot_situations};
use crate::promotion::{MovementType, PromotionResult, TeamMovement};

fn with_savepoint<T, F>(conn: &Connection, name: &str, action: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    conn.execute_batch(&format!("SAVEPOINT {name}"))
        .map_err(|e| format!("Falha ao abrir savepoint '{name}': {e}"))?;

    match action() {
        Ok(value) => {
            conn.execute_batch(&format!("RELEASE SAVEPOINT {name}"))
                .map_err(|e| format!("Falha ao confirmar savepoint '{name}': {e}"))?;
            Ok(value)
        }
        Err(err) => {
            conn.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {name}; RELEASE SAVEPOINT {name};"
            ))
            .map_err(|rollback_err| {
                format!("{err}; alem disso falhou o rollback do savepoint '{name}': {rollback_err}")
            })?;
            Err(err)
        }
    }
}

pub fn run_promotion_relegation(
    conn: &Connection,
    season_number: i32,
    rng: &mut impl Rng,
) -> Result<PromotionResult, String> {
    if season_number < 1 {
        return Ok(PromotionResult::empty());
    }

    with_savepoint(conn, "promotion_run", || {
        let mut all_movements = Vec::new();
        let block1_movements = execute_block1(conn, rng)?;
        let excluded_from_block2: HashSet<String> = block1_movements
            .iter()
            .filter(|movement| {
                movement.movement_type == MovementType::Rebaixamento
                    && (movement.from_category == "mazda_amador"
                        || movement.from_category == "toyota_amador")
            })
            .map(|movement| movement.team_id.clone())
            .collect();
        let block2_movements = execute_block2_with_exclusions(conn, &excluded_from_block2, rng)?;
        let block3_movements = execute_block3(conn, rng)?;

        all_movements.extend(block1_movements);
        all_movements.extend(block2_movements);
        all_movements.extend(block3_movements);

        for movement in &all_movements {
            apply_team_category_change(conn, movement)?;
        }

        let pilot_effects = resolve_pilot_situations(conn, &all_movements)?;
        for effect in &pilot_effects {
            apply_pilot_effect(conn, effect, &all_movements)?;
        }

        let mut attribute_deltas = Vec::new();
        for movement in &all_movements {
            let team = team_queries::get_team_by_id(conn, &movement.team_id)
                .map_err(|e| format!("Falha ao buscar equipe '{}': {e}", movement.team_id))?
                .ok_or_else(|| format!("Equipe '{}' nao encontrada", movement.team_id))?;
            let delta = match movement.movement_type {
                MovementType::Promocao => calculate_promotion_effects(&team, rng),
                MovementType::Rebaixamento => calculate_relegation_effects(&team, rng),
            };
            apply_attribute_deltas(conn, &movement.team_id, &delta)?;
            attribute_deltas.push(delta);
        }

        let mut errors = verify_team_driver_consistency(conn, &all_movements);
        errors.extend(verify_category_sizes(conn)?);
        Ok(PromotionResult {
            movements: all_movements,
            pilot_effects,
            attribute_deltas,
            errors,
        })
    })
}

fn apply_team_category_change(conn: &Connection, movement: &TeamMovement) -> Result<(), String> {
    let mut team = team_queries::get_team_by_id(conn, &movement.team_id)
        .map_err(|e| format!("Falha ao buscar equipe '{}': {e}", movement.team_id))?
        .ok_or_else(|| format!("Equipe '{}' nao encontrada", movement.team_id))?;
    // Persiste a categoria de origem para exibição na pré-temporada
    team.categoria_anterior = Some(team.categoria.clone());
    team.categoria = movement.to_category.clone();
    team.classe = infer_team_class(movement);
    team_queries::update_team(conn, &team)
        .map_err(|e| format!("Falha ao atualizar equipe '{}': {e}", team.nome))?;
    Ok(())
}

fn infer_team_class(movement: &TeamMovement) -> Option<String> {
    match movement.to_category.as_str() {
        "production_challenger" => match movement.from_category.as_str() {
            "mazda_amador" => Some("mazda".to_string()),
            "toyota_amador" => Some("toyota".to_string()),
            "bmw_m2" => Some("bmw".to_string()),
            _ => None,
        },
        "endurance" => match movement.from_category.as_str() {
            "gt4" => Some("gt4".to_string()),
            "gt3" => Some("gt3".to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn verify_team_driver_consistency(conn: &Connection, movements: &[TeamMovement]) -> Vec<String> {
    let mut errors = Vec::new();
    for movement in movements {
        let Ok(Some(team)) = team_queries::get_team_by_id(conn, &movement.team_id) else {
            continue;
        };
        // Equipes especiais usam categoria_especial_ativa em vez de categoria_atual
        if is_especial(&team.categoria) {
            for pilot_id in [team.piloto_1_id.as_deref(), team.piloto_2_id.as_deref()]
                .into_iter()
                .flatten()
            {
                match driver_queries::get_driver(conn, pilot_id) {
                    Err(_) => errors.push(format!(
                        "CONSISTENCIA: equipe especial '{}' referencia piloto '{pilot_id}' inexistente",
                        team.nome
                    )),
                    Ok(driver) => {
                        if driver.categoria_especial_ativa.as_deref() != Some(team.categoria.as_str()) {
                            errors.push(format!(
                                "CONSISTENCIA: piloto '{}' em especial sem categoria_especial_ativa correta (tem '{:?}', equipe '{}')",
                                driver.nome, driver.categoria_especial_ativa, team.categoria
                            ));
                        }
                    }
                }
            }
            continue;
        }

        for pilot_id in [team.piloto_1_id.as_deref(), team.piloto_2_id.as_deref()]
            .into_iter()
            .flatten()
        {
            match driver_queries::get_driver(conn, pilot_id) {
                Err(_) => errors.push(format!(
                    "CONSISTENCIA: equipe '{}' referencia piloto '{pilot_id}' inexistente",
                    team.nome
                )),
                Ok(driver) => {
                    if driver.categoria_atual.as_deref() != Some(team.categoria.as_str()) {
                        errors.push(format!(
                            "CONSISTENCIA: piloto '{}' tem categoria '{:?}' mas equipe '{}' esta em '{}'",
                            driver.nome, driver.categoria_atual, team.nome, team.categoria
                        ));
                    }
                }
            }
        }
    }
    errors
}

fn verify_category_sizes(conn: &Connection) -> Result<Vec<String>, String> {
    let expected_sizes = [
        ("mazda_rookie", 6),
        ("toyota_rookie", 6),
        ("mazda_amador", 10),
        ("toyota_amador", 10),
        ("bmw_m2", 10),
        ("gt4", 10),
        ("gt3", 14),
    ];
    let mut errors = Vec::new();
    for (category, expected) in expected_sizes {
        let actual = team_queries::count_teams_by_category(conn, category)
            .map_err(|e| format!("Falha ao contar equipes em '{category}': {e}"))?;
        if actual != expected {
            errors.push(format!(
                "INVARIANTE VIOLADO: {category} tem {actual} equipes (esperado {expected})"
            ));
        }
    }
    Ok(errors)
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};
    use rusqlite::Connection;

    use super::*;
    use crate::db::migrations;
    use crate::db::queries::contracts as contract_queries;
    use crate::db::queries::drivers as driver_queries;
    use crate::db::queries::teams as team_queries;
    use crate::models::contract::Contract;
    use crate::models::driver::Driver;
    use crate::models::enums::TeamRole;
    use crate::models::team::Team;

    #[test]
    fn test_no_promotion_invalid_season() {
        let conn = setup_promotion_db();
        let mut rng = StdRng::seed_from_u64(50);

        let result = run_promotion_relegation(&conn, 0, &mut rng).expect("promotion should run");

        assert!(result.movements.is_empty());
        assert!(result.pilot_effects.is_empty());
        assert!(result.attribute_deltas.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_full_promotion_season_1() {
        let conn = setup_promotion_db();
        let mut rng = StdRng::seed_from_u64(50);

        let result = run_promotion_relegation(&conn, 1, &mut rng).expect("promotion should run");

        assert_eq!(result.movements.len(), 4);
        assert!(!result.attribute_deltas.is_empty());
    }

    #[test]
    fn test_full_promotion_all_blocks() {
        let conn = setup_promotion_db();
        let mut rng = StdRng::seed_from_u64(51);

        let result = run_promotion_relegation(&conn, 2, &mut rng).expect("promotion should run");

        assert_eq!(result.movements.len(), 4);
        assert!(!result.attribute_deltas.is_empty());
    }

    #[test]
    fn test_invariant_maintained() {
        let conn = setup_promotion_db();
        let mut rng = StdRng::seed_from_u64(52);

        let result = run_promotion_relegation(&conn, 2, &mut rng).expect("promotion should run");

        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn test_run_promotion_relegation_rolls_back_if_pilot_effect_fails_midway() {
        let conn = setup_promotion_db();
        let mut rng = StdRng::seed_from_u64(50);

        let mut driver = Driver::new(
            "P901".to_string(),
            "Piloto Teste".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2020,
        );
        driver.categoria_atual = Some("mazda_rookie".to_string());
        driver_queries::insert_driver(&conn, &driver).expect("insert driver");
        team_queries::update_team_pilots(&conn, "MR1", Some("P901"), None)
            .expect("attach driver to champion team");

        let champion_team = team_queries::get_team_by_id(&conn, "MR1")
            .expect("team query")
            .expect("team exists");
        let contract = Contract::new(
            "C901".to_string(),
            "P901".to_string(),
            "Piloto Teste".to_string(),
            champion_team.id.clone(),
            champion_team.nome.clone(),
            1,
            2,
            50_000.0,
            TeamRole::Numero1,
            "mazda_rookie".to_string(),
        );
        contract_queries::insert_contract(&conn, &contract).expect("insert contract");

        conn.execute("DROP TABLE contracts", [])
            .expect("drop contracts table");

        let err = run_promotion_relegation(&conn, 1, &mut rng)
            .expect_err("promotion should fail when pilot effect cannot update contract");

        assert!(err.contains("contrato regular"), "unexpected error: {err}");

        let team = team_queries::get_team_by_id(&conn, "MR1")
            .expect("team query after failure")
            .expect("team still exists");
        assert_eq!(team.categoria, "mazda_rookie");
        assert_eq!(team.categoria_anterior, None);

        let driver = driver_queries::get_driver(&conn, "P901").expect("driver query after failure");
        assert_eq!(driver.categoria_atual.as_deref(), Some("mazda_rookie"));
    }

    #[test]
    fn test_verify_category_sizes_propagates_database_errors() {
        let conn = setup_promotion_db();
        conn.execute("DROP TABLE teams", [])
            .expect("drop teams table");

        let err = verify_category_sizes(&conn).expect_err("count failure should propagate");

        assert!(
            err.contains("Falha ao contar equipes"),
            "unexpected error: {err}"
        );
    }

    fn setup_promotion_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        insert_ranked_teams(&conn, "mazda_rookie", "MR", 6, None);
        insert_ranked_teams(&conn, "toyota_rookie", "TR", 6, None);
        insert_ranked_teams(&conn, "mazda_amador", "MA", 10, None);
        insert_ranked_teams(&conn, "toyota_amador", "TA", 10, None);
        insert_ranked_teams(&conn, "bmw_m2", "BM", 10, None);
        insert_ranked_teams(&conn, "production_challenger", "PM", 5, Some("mazda"));
        insert_ranked_teams(&conn, "production_challenger", "PT", 5, Some("toyota"));
        insert_ranked_teams(&conn, "production_challenger", "PB", 5, Some("bmw"));
        insert_ranked_teams(&conn, "gt4", "GT4", 10, None);
        insert_ranked_teams(&conn, "gt3", "GT3", 14, None);
        insert_ranked_teams(&conn, "endurance", "EG4", 6, Some("gt4"));
        insert_ranked_teams(&conn, "endurance", "EG3", 6, Some("gt3"));
        insert_ranked_teams(&conn, "endurance", "LMP", 5, Some("lmp2"));

        conn
    }

    fn insert_ranked_teams(
        conn: &Connection,
        category: &str,
        prefix: &str,
        count: usize,
        class: Option<&str>,
    ) {
        for index in 0..count {
            let rank = index + 1;
            let mut team = sample_team(
                category,
                &format!("{prefix}{rank}"),
                &format!("{prefix} Team {rank}"),
                class,
            );
            team.stats_pontos = ((count - index) * 10) as i32;
            team.stats_vitorias = (count - index) as i32;
            team.stats_melhor_resultado = rank as i32;
            team_queries::insert_team(conn, &team).expect("insert ranked team");
        }
    }

    fn sample_team(category: &str, id: &str, name: &str, class: Option<&str>) -> Team {
        let template = crate::constants::teams::get_reference_team_template(category, class)
            .expect("team template");
        let mut rng = StdRng::seed_from_u64(id.bytes().map(u64::from).sum());
        let mut team =
            Team::from_template_with_rng(template, category, id.to_string(), 2025, &mut rng);
        team.nome = name.to_string();
        team.nome_curto = name.to_string();
        team.classe = class.map(str::to_string);
        team
    }
}
