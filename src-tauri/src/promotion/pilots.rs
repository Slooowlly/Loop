use rusqlite::Connection;

use crate::constants::categories::{get_category_config, is_especial};
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::teams as team_queries;
use crate::models::enums::ContractStatus;
use crate::models::license::driver_has_required_license_level;
use crate::promotion::{MovementType, PilotEffect, PilotEffectType, TeamMovement};

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

pub fn resolve_pilot_situations(
    conn: &Connection,
    movements: &[TeamMovement],
) -> Result<Vec<PilotEffect>, String> {
    let mut effects = Vec::new();
    for movement in movements {
        let team = team_queries::get_team_by_id(conn, &movement.team_id)
            .map_err(|e| format!("Falha ao buscar equipe '{}': {e}", movement.team_id))?
            .ok_or_else(|| format!("Equipe '{}' nao encontrada", movement.team_id))?;

        for pilot_id in [team.piloto_1_id.as_deref(), team.piloto_2_id.as_deref()]
            .into_iter()
            .flatten()
        {
            let driver = driver_queries::get_driver(conn, pilot_id)
                .map_err(|e| format!("Falha ao buscar piloto '{pilot_id}': {e}"))?;
            let effect = match movement.movement_type {
                MovementType::Promocao => {
                    let required_license = get_category_config(&movement.to_category)
                        .and_then(|config| config.licenca_necessaria);
                    let has_license = check_driver_has_license(conn, pilot_id, required_license)?;
                    if has_license {
                        PilotEffect {
                            driver_id: driver.id.clone(),
                            driver_name: driver.nome.clone(),
                            team_id: movement.team_id.clone(),
                            effect: PilotEffectType::MovesWithTeam,
                            reason: "Tem licenca, sobe com a equipe".to_string(),
                        }
                    } else if driver.is_jogador {
                        PilotEffect {
                            driver_id: driver.id.clone(),
                            driver_name: driver.nome.clone(),
                            team_id: movement.team_id.clone(),
                            effect: PilotEffectType::FreedPlayerStays,
                            reason: "Jogador sem licenca, fica livre na categoria atual"
                                .to_string(),
                        }
                    } else {
                        PilotEffect {
                            driver_id: driver.id.clone(),
                            driver_name: driver.nome.clone(),
                            team_id: movement.team_id.clone(),
                            effect: PilotEffectType::FreedNoLicense,
                            reason: "Sem licenca para a nova categoria".to_string(),
                        }
                    }
                }
                MovementType::Rebaixamento => PilotEffect {
                    driver_id: driver.id.clone(),
                    driver_name: driver.nome.clone(),
                    team_id: movement.team_id.clone(),
                    effect: PilotEffectType::MovesWithTeam,
                    reason: "Desce com a equipe".to_string(),
                },
            };
            effects.push(effect);
        }
    }

    Ok(effects)
}

pub fn apply_pilot_effect(
    conn: &Connection,
    effect: &PilotEffect,
    movements: &[TeamMovement],
) -> Result<(), String> {
    with_savepoint(conn, "promotion_pilot_effect", || {
        let movement = movements
            .iter()
            .find(|movement| movement.team_id == effect.team_id)
            .ok_or_else(|| format!("Movimento nao encontrado para equipe '{}'", effect.team_id))?;
        let mut driver = driver_queries::get_driver(conn, &effect.driver_id)
            .map_err(|e| format!("Falha ao buscar piloto '{}': {e}", effect.driver_id))?;

        match effect.effect {
            PilotEffectType::MovesWithTeam => {
                driver.categoria_atual = Some(movement.to_category.clone());
                driver.categoria_especial_ativa = if is_especial(&movement.to_category) {
                    Some(movement.to_category.clone())
                } else {
                    None
                };
                driver_queries::update_driver(conn, &driver)
                    .map_err(|e| format!("Falha ao atualizar piloto '{}': {e}", driver.nome))?;
                if let Some(contract) =
                    contract_queries::get_active_regular_contract_for_pilot(conn, &driver.id)
                        .map_err(|e| {
                            format!("Falha ao buscar contrato regular de '{}': {e}", driver.nome)
                        })?
                {
                    conn.execute(
                        "UPDATE contracts SET categoria = ?1 WHERE id = ?2",
                        rusqlite::params![&movement.to_category, &contract.id],
                    )
                    .map_err(|e| {
                        format!(
                            "Falha ao atualizar categoria do contrato '{}': {e}",
                            contract.id
                        )
                    })?;
                }
            }
            PilotEffectType::FreedNoLicense => {
                driver.categoria_atual = Some(movement.from_category.clone());
                driver_queries::update_driver(conn, &driver).map_err(|e| {
                    format!("Falha ao atualizar piloto livre '{}': {e}", driver.nome)
                })?;
                remove_driver_from_team(conn, &effect.team_id, &effect.driver_id)?;
                if let Some(contract) =
                    contract_queries::get_active_regular_contract_for_pilot(conn, &driver.id)
                        .map_err(|e| {
                            format!("Falha ao buscar contrato regular de '{}': {e}", driver.nome)
                        })?
                {
                    contract_queries::update_contract_status(
                        conn,
                        &contract.id,
                        &ContractStatus::Rescindido,
                    )
                    .map_err(|e| format!("Falha ao rescindir contrato '{}': {e}", contract.id))?;
                }
            }
            PilotEffectType::FreedPlayerStays => {
                driver.categoria_atual = Some(movement.from_category.clone());
                driver_queries::update_driver(conn, &driver).map_err(|e| {
                    format!("Falha ao atualizar jogador livre '{}': {e}", driver.nome)
                })?;
                // Varredura defensiva: limpa qualquer referência de time que ainda aponte
                // para o jogador, garantindo que ele não fique em estado parcialmente vinculado.
                clear_all_team_references(conn, &effect.driver_id)?;
                if let Some(contract) =
                    contract_queries::get_active_regular_contract_for_pilot(conn, &driver.id)
                        .map_err(|e| {
                            format!("Falha ao buscar contrato regular de '{}': {e}", driver.nome)
                        })?
                {
                    contract_queries::update_contract_status(
                        conn,
                        &contract.id,
                        &ContractStatus::Rescindido,
                    )
                    .map_err(|e| format!("Falha ao rescindir contrato '{}': {e}", contract.id))?;
                }
            }
        }

        Ok(())
    })
}

fn remove_driver_from_team(
    conn: &Connection,
    team_id: &str,
    driver_id: &str,
) -> Result<(), String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao buscar equipe '{team_id}': {e}"))?
        .ok_or_else(|| format!("Equipe '{team_id}' nao encontrada"))?;
    let piloto_1 = team
        .piloto_1_id
        .as_deref()
        .filter(|current| *current != driver_id);
    let piloto_2 = team
        .piloto_2_id
        .as_deref()
        .filter(|current| *current != driver_id);
    team_queries::update_team_pilots(conn, team_id, piloto_1, piloto_2)
        .map_err(|e| format!("Falha ao atualizar pilotos da equipe '{team_id}': {e}"))?;
    Ok(())
}

fn clear_all_team_references(conn: &Connection, driver_id: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE teams SET piloto_1_id = NULL WHERE piloto_1_id = ?1",
        rusqlite::params![driver_id],
    )
    .map_err(|e| format!("Falha ao limpar slot 1 do jogador '{driver_id}': {e}"))?;
    conn.execute(
        "UPDATE teams SET piloto_2_id = NULL WHERE piloto_2_id = ?1",
        rusqlite::params![driver_id],
    )
    .map_err(|e| format!("Falha ao limpar slot 2 do jogador '{driver_id}': {e}"))?;
    Ok(())
}

fn check_driver_has_license(
    conn: &Connection,
    driver_id: &str,
    required_license: Option<u8>,
) -> Result<bool, String> {
    let Some(level) = required_license else {
        return Ok(true);
    };
    driver_has_required_license_level(conn, driver_id, level)
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};
    use rusqlite::Connection;

    use super::*;
    use crate::constants::teams::get_team_templates;
    use crate::db::migrations;
    use crate::db::queries::contracts as contract_queries;
    use crate::db::queries::drivers as driver_queries;
    use crate::db::queries::teams as team_queries;
    use crate::models::contract::Contract;
    use crate::models::driver::Driver;
    use crate::models::enums::TeamRole;
    use crate::models::team::Team;
    use crate::promotion::{MovementType, PilotEffectType, TeamMovement};

    #[test]
    fn test_pilot_with_license_moves() {
        let conn = setup_pilots_db();
        conn.execute(
            "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
             VALUES ('P001', '3', 'gt4', '2026-01-01T00:00:00', 1)",
            [],
        )
        .expect("insert license");

        let effects = resolve_pilot_situations(&conn, &[promotion_to_gt3("T001")])
            .expect("resolve pilot situations");

        assert!(effects.iter().any(|effect| {
            effect.driver_id == "P001" && effect.effect == PilotEffectType::MovesWithTeam
        }));
    }

    #[test]
    fn test_pilot_without_license_freed() {
        let conn = setup_pilots_db();

        let effects = resolve_pilot_situations(&conn, &[promotion_to_gt3("T001")])
            .expect("resolve pilot situations");

        assert!(effects.iter().any(|effect| {
            effect.driver_id == "P001" && effect.effect == PilotEffectType::FreedNoLicense
        }));
    }

    #[test]
    fn test_relegated_pilots_always_move() {
        let conn = setup_pilots_db();

        let effects = resolve_pilot_situations(
            &conn,
            &[TeamMovement {
                team_id: "T001".to_string(),
                team_name: "Equipe 1".to_string(),
                from_category: "gt3".to_string(),
                to_category: "gt4".to_string(),
                movement_type: MovementType::Rebaixamento,
                reason: "Teste".to_string(),
            }],
        )
        .expect("resolve pilot situations");

        assert!(effects
            .iter()
            .all(|effect| effect.effect == PilotEffectType::MovesWithTeam));
    }

    #[test]
    fn test_apply_pilot_effect_frees_driver_and_rescinds_contract() {
        let conn = setup_pilots_db();
        let movement = promotion_to_gt3("T001");
        let effect = PilotEffect {
            driver_id: "P001".to_string(),
            driver_name: "Piloto 1".to_string(),
            team_id: "T001".to_string(),
            effect: PilotEffectType::FreedNoLicense,
            reason: "Sem licenca".to_string(),
        };

        apply_pilot_effect(&conn, &effect, &[movement]).expect("apply effect");

        let driver = driver_queries::get_driver(&conn, "P001").expect("driver query");
        let team = team_queries::get_team_by_id(&conn, "T001")
            .expect("team query")
            .expect("team exists");
        let contract = contract_queries::get_active_regular_contract_for_pilot(&conn, "P001")
            .expect("contract query");

        assert_eq!(driver.categoria_atual.as_deref(), Some("gt4"));
        assert_ne!(team.piloto_1_id.as_deref(), Some("P001"));
        assert!(contract.is_none());
    }

    #[test]
    fn test_apply_pilot_effect_rolls_back_driver_update_if_contract_step_fails() {
        let conn = setup_pilots_db();
        conn.execute("DROP TABLE contracts", [])
            .expect("drop contracts table");

        let effect = PilotEffect {
            driver_id: "P001".to_string(),
            driver_name: "Piloto 1".to_string(),
            team_id: "T001".to_string(),
            effect: PilotEffectType::MovesWithTeam,
            reason: "Tem licenca".to_string(),
        };

        let err = apply_pilot_effect(&conn, &effect, &[promotion_to_gt3("T001")])
            .expect_err("effect should fail without contracts table");

        assert!(err.contains("contrato regular"), "unexpected error: {err}");

        let driver = driver_queries::get_driver(&conn, "P001").expect("driver query");
        assert_eq!(driver.categoria_atual.as_deref(), Some("gt4"));
    }

    #[test]
    fn test_apply_pilot_effect_sets_categoria_especial_ativa_for_special_category() {
        let conn = setup_pilots_db();
        let effect = PilotEffect {
            driver_id: "P001".to_string(),
            driver_name: "Piloto 1".to_string(),
            team_id: "T001".to_string(),
            effect: PilotEffectType::MovesWithTeam,
            reason: "Tem licenca".to_string(),
        };

        apply_pilot_effect(&conn, &effect, &[promotion_to_endurance("T001")])
            .expect("apply effect");

        let driver = driver_queries::get_driver(&conn, "P001").expect("driver query");
        assert_eq!(driver.categoria_atual.as_deref(), Some("endurance"));
        assert_eq!(
            driver.categoria_especial_ativa.as_deref(),
            Some("endurance")
        );
    }

    fn setup_pilots_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        let team = sample_team("gt4", "T001");
        team_queries::insert_team(&conn, &team).expect("insert team");

        let mut driver_1 = sample_driver("P001", "Piloto 1", true);
        driver_1.is_jogador = false;
        driver_1.categoria_atual = Some("gt4".to_string());
        driver_queries::insert_driver(&conn, &driver_1).expect("driver 1");

        let mut driver_2 = sample_driver("P002", "Piloto 2", false);
        driver_2.categoria_atual = Some("gt4".to_string());
        driver_queries::insert_driver(&conn, &driver_2).expect("driver 2");

        team_queries::update_team_pilots(&conn, "T001", Some("P001"), Some("P002"))
            .expect("update pilots");

        let contract_1 = Contract::new(
            "C001".to_string(),
            "P001".to_string(),
            "Piloto 1".to_string(),
            "T001".to_string(),
            "Equipe 1".to_string(),
            1,
            2,
            100_000.0,
            TeamRole::Numero1,
            "gt4".to_string(),
        );
        let contract_2 = Contract::new(
            "C002".to_string(),
            "P002".to_string(),
            "Piloto 2".to_string(),
            "T001".to_string(),
            "Equipe 1".to_string(),
            1,
            2,
            90_000.0,
            TeamRole::Numero2,
            "gt4".to_string(),
        );
        contract_queries::insert_contract(&conn, &contract_1).expect("contract 1");
        contract_queries::insert_contract(&conn, &contract_2).expect("contract 2");

        conn
    }

    fn sample_team(category: &str, id: &str) -> Team {
        let template = get_team_templates(category)[0];
        let mut rng = StdRng::seed_from_u64(505);
        let mut team =
            Team::from_template_with_rng(template, category, id.to_string(), 2025, &mut rng);
        team.nome = "Equipe 1".to_string();
        team.nome_curto = "Equipe 1".to_string();
        team
    }

    fn sample_driver(id: &str, name: &str, has_license: bool) -> Driver {
        let mut driver = Driver::new(
            id.to_string(),
            name.to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2020,
        );
        if has_license {
            driver.temporadas_na_categoria = 1;
        }
        driver
    }

    fn promotion_to_gt3(team_id: &str) -> TeamMovement {
        TeamMovement {
            team_id: team_id.to_string(),
            team_name: "Equipe 1".to_string(),
            from_category: "gt4".to_string(),
            to_category: "gt3".to_string(),
            movement_type: MovementType::Promocao,
            reason: "Teste".to_string(),
        }
    }

    fn promotion_to_endurance(team_id: &str) -> TeamMovement {
        TeamMovement {
            team_id: team_id.to_string(),
            team_name: "Equipe 1".to_string(),
            from_category: "gt4".to_string(),
            to_category: "endurance".to_string(),
            movement_type: MovementType::Promocao,
            reason: "Teste".to_string(),
        }
    }
}
