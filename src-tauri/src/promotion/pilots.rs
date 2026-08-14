use rusqlite::Connection;

use crate::constants::categories::runs_in_special_phase;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::teams as team_queries;
use crate::licensing::{
    driver_has_required_license_level, grant_driver_license_for_division_if_needed,
};
use crate::models::enums::ContractStatus;
use crate::models::license::required_license_for_division;
use crate::promotion::{MovementType, PilotEffect, PilotEffectType, TeamMovement};

use crate::db::savepoint::with_savepoint;

pub fn resolve_pilot_situations(
    conn: &Connection,
    movements: &[TeamMovement],
) -> Result<Vec<PilotEffect>, String> {
    let mut effects = Vec::new();
    for movement in movements {
        let team = team_queries::get_team_by_id(conn, &movement.team_id)
            .map_err(|e| format!("Falha ao buscar equipe '{}': {e}", movement.team_id))?
            .ok_or_else(|| format!("Equipe '{}' nao encontrada", movement.team_id))?;

        // O elenco da equipe é o lineup mais quem tem contrato regular ativo:
        // em Production/Endurance o grid é montado pelos contratos e o lineup
        // (piloto_1/piloto_2) pode estar vazio.
        let mut pilot_ids: Vec<String> = [team.piloto_1_id.clone(), team.piloto_2_id.clone()]
            .into_iter()
            .flatten()
            .collect();
        let team_contracts =
            contract_queries::get_active_regular_contracts_by_team(conn, &movement.team_id)
                .map_err(|e| {
                    format!(
                        "Falha ao buscar contratos da equipe '{}': {e}",
                        movement.team_id
                    )
                })?;
        for contract in &team_contracts {
            if !pilot_ids.contains(&contract.piloto_id) {
                pilot_ids.push(contract.piloto_id.clone());
            }
        }

        for pilot_id in &pilot_ids {
            let driver = driver_queries::get_driver(conn, pilot_id)
                .map_err(|e| format!("Falha ao buscar piloto '{pilot_id}': {e}"))?;
            let effect = match movement.movement_type {
                // Subida estrutural para Production/Endurance: a equipe leva os
                // pilotos e a licença da categoria é concedida junto (ficar livre
                // é exceção).
                MovementType::Promocao if runs_in_special_phase(&movement.to_category) => {
                    PilotEffect {
                        driver_id: driver.id.clone(),
                        driver_name: driver.nome.clone(),
                        team_id: movement.team_id.clone(),
                        effect: PilotEffectType::MovesWithTeam,
                        reason: "Sobe com a equipe; licenca da categoria garantida".to_string(),
                    }
                }
                MovementType::Promocao => {
                    // Promoção não-especial: alvos são categorias regulares (sem
                    // classe), então a divisão é (categoria, None).
                    let required_license =
                        required_license_for_division(&movement.to_category, None);
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
                // Movimento estrutural: Production/Endurance usam categoria_atual como
                // qualquer categoria; categoria_especial_ativa é legado da convocação.
                driver.mover_para_categoria(Some(movement.to_category.clone()));
                driver.categoria_especial_ativa = None;
                driver_queries::update_driver(conn, &driver)
                    .map_err(|e| format!("Falha ao atualizar piloto '{}': {e}", driver.nome))?;
                if movement.movement_type == MovementType::Promocao {
                    grant_driver_license_for_division_if_needed(
                        conn,
                        &driver.id,
                        &movement.to_category,
                        target_class_for_movement(movement).as_deref(),
                    )?;
                }
                if let Some(contract) =
                    contract_queries::get_active_regular_contract_for_pilot(conn, &driver.id)
                        .map_err(|e| {
                            format!("Falha ao buscar contrato regular de '{}': {e}", driver.nome)
                        })?
                {
                    let target_class = target_class_for_movement(movement);
                    conn.execute(
                        "UPDATE contracts SET categoria = ?1, classe = ?2 WHERE id = ?3",
                        rusqlite::params![&movement.to_category, &target_class, &contract.id],
                    )
                    .map_err(|e| {
                        format!(
                            "Falha ao atualizar categoria/classe do contrato '{}': {e}",
                            contract.id
                        )
                    })?;
                }
            }
            PilotEffectType::FreedNoLicense => {
                driver.categoria_atual = Some(movement.from_category.clone());
                // O outro instante em que a vaga se perde de verdade: a equipe subiu, o
                // piloto não tinha a licença e fica para trás com o contrato rescindido
                // logo abaixo. Este braço é IA por construção — o jogador sem licença
                // sai por `FreedPlayerStays`, que não aplica nada. A gravação vai junto
                // do `update_driver` abaixo, no mesmo write do evento.
                crate::evolution::motivation::adjust_lost_seat_motivation(&mut driver);
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

fn target_class_for_movement(movement: &TeamMovement) -> Option<String> {
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
    fn test_apply_pilot_effect_does_not_use_categoria_especial_ativa() {
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
        assert!(
            driver.categoria_especial_ativa.is_none(),
            "movimento estrutural nao deve usar categoria_especial_ativa"
        );
    }

    #[test]
    fn test_promotion_to_production_moves_pilots_and_grants_license() {
        let conn = setup_pilots_db();
        let movement = TeamMovement {
            team_id: "T001".to_string(),
            team_name: "Equipe 1".to_string(),
            from_category: "mazda_amador".to_string(),
            to_category: "production_challenger".to_string(),
            movement_type: MovementType::Promocao,
            reason: "Campea do amador".to_string(),
        };

        let effects =
            resolve_pilot_situations(&conn, &[movement.clone()]).expect("resolve situations");
        assert!(
            effects
                .iter()
                .all(|effect| effect.effect == PilotEffectType::MovesWithTeam),
            "pilotos da equipe promovida para Production devem acompanhar a equipe"
        );

        for effect in &effects {
            apply_pilot_effect(&conn, effect, &[movement.clone()]).expect("apply effect");
        }

        for driver_id in ["P001", "P002"] {
            let driver = driver_queries::get_driver(&conn, driver_id).expect("driver query");
            assert_eq!(
                driver.categoria_atual.as_deref(),
                Some("production_challenger")
            );
            assert!(driver.categoria_especial_ativa.is_none());
            let has_license = crate::licensing::driver_has_required_license_for_category(
                &conn,
                driver_id,
                "production_challenger",
            )
            .expect("license query");
            assert!(has_license, "piloto promovido deve receber a licenca");

            let contract =
                contract_queries::get_active_regular_contract_for_pilot(&conn, driver_id)
                    .expect("contract query")
                    .expect("contract should stay active");
            assert_eq!(contract.categoria, "production_challenger");
        }
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
