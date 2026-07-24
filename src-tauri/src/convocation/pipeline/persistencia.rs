//! Persistência transacional dos grids do bloco especial: prepara as operações
//! em memória, aplica tudo numa transação só e grava as ofertas do jogador.

use super::*;

struct PendingOp {
    contract: crate::models::contract::Contract,
    driver_id: String,
    special_category: String,
}

#[cfg(test)]
pub(super) fn persistir_grids(
    conn: &Connection,
    grids: &[GridClasse],
    season_number: i32,
) -> Result<(), DbError> {
    // Coletar todos os dados necessários antes da transação (next_id precisa de conn)
    // Gerar IDs de contrato antecipadamente
    let total = grids.iter().map(|g| g.assignments.len()).sum::<usize>();
    let contract_ids = crate::generators::ids::next_ids(conn, IdType::Contract, total as u32)?;

    let mut contract_idx = 0;

    // Agrupar assignments por team para update_team_pilots
    // Estrutura: team_id → (n1_id, n2_id)
    let mut team_lineup: std::collections::HashMap<String, (Option<String>, Option<String>)> =
        std::collections::HashMap::new();

    // Coletar dados de teams para obter nome
    let mut team_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    // Coletar dados de drivers para obter nome
    let mut driver_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    // Pre-carregar teams e drivers necessários
    for grid in grids {
        for a in &grid.assignments {
            if !team_map.contains_key(&a.team_id) {
                if let Ok(Some(team)) = team_queries::get_team_by_id(conn, &a.team_id) {
                    team_map.insert(team.id.clone(), team.nome.clone());
                }
            }
            if !driver_map.contains_key(&a.driver_id) {
                if let Ok(driver) = driver_queries::get_driver(conn, &a.driver_id) {
                    driver_map.insert(driver.id.clone(), driver.nome.clone());
                }
            }
        }
    }

    // Construir contratos e lineup updates em memória
    struct PendingOp {
        contract: crate::models::contract::Contract,
        driver_id: String,
        special_category: String,
    }

    let mut ops: Vec<PendingOp> = Vec::new();

    // Mapa de class_name → special_category
    let class_to_cat: std::collections::HashMap<&str, &str> = CLASSES_CONVOCADAS
        .iter()
        .map(|c| (c.class_name, c.special_category))
        .collect();

    for grid in grids {
        let special_cat = class_to_cat
            .get(grid.class_name.as_str())
            .copied()
            .unwrap_or("unknown");

        for a in &grid.assignments {
            let contract_id = contract_ids[contract_idx].clone();
            contract_idx += 1;

            let team_nome = team_map
                .get(&a.team_id)
                .cloned()
                .unwrap_or_else(|| a.team_id.clone());
            let driver_nome = driver_map
                .get(&a.driver_id)
                .cloned()
                .unwrap_or_else(|| a.driver_id.clone());

            let papel = if a.papel == TeamRole::Numero1 {
                TeamRole::Numero1
            } else {
                TeamRole::Numero2
            };

            let contract = contract_queries::generate_especial_contract(
                contract_id,
                &a.driver_id,
                &driver_nome,
                &a.team_id,
                &team_nome,
                papel.clone(),
                special_cat,
                &grid.class_name,
                season_number,
            );

            // Atualizar lineup
            let entry = team_lineup.entry(a.team_id.clone()).or_insert((None, None));
            match papel {
                TeamRole::Numero1 => entry.0 = Some(a.driver_id.clone()),
                TeamRole::Numero2 => entry.1 = Some(a.driver_id.clone()),
            }

            ops.push(PendingOp {
                contract,
                driver_id: a.driver_id.clone(),
                special_category: special_cat.to_string(),
            });
        }
    }

    let tx = conn.unchecked_transaction()?;

    driver_queries::clear_all_categoria_especial_ativa(&tx)?;
    team_queries::clear_special_team_lineups(&tx)?;
    team_queries::reset_special_team_hierarchies(&tx)?;

    // Persistir tudo
    for op in &ops {
        contract_queries::insert_contract(&tx, &op.contract)?;
        driver_queries::update_driver_especial_category(
            &tx,
            &op.driver_id,
            Some(&op.special_category),
        )?;
    }

    for (team_id, (n1, n2)) in &team_lineup {
        team_queries::update_team_pilots(&tx, team_id, n1.as_deref(), n2.as_deref())?;

        // Hierarquia: N1 = hierarquia_n1_id, N2 = hierarquia_n2_id
        if let (Some(n1_id), Some(n2_id)) = (n1, n2) {
            team_queries::update_team_hierarchy(
                &tx,
                team_id,
                Some(n1_id.as_str()),
                Some(n2_id.as_str()),
                "Claro",
                0.0,
            )?;
        }
    }

    tx.commit()?;

    Ok(())
}

pub(super) fn persistir_grids_e_ofertas(
    conn: &Connection,
    season_id: &str,
    grids: &[GridClasse],
    season_number: i32,
    player_offers_payload: Option<&(String, Vec<PlayerSpecialOffer>)>,
) -> Result<(), DbError> {
    if grids.is_empty() {
        if let Some((player_id, offers)) = player_offers_payload {
            player_offers::replace_player_special_offers(conn, season_id, player_id, offers)?;
        }
        return Ok(());
    }

    let ops = preparar_persistencia_grids(conn, grids, season_number)?;
    let tx = conn.unchecked_transaction()?;
    aplicar_persistencia_grids(&tx, &ops)?;

    if let Some((player_id, offers)) = player_offers_payload {
        player_offers::replace_player_special_offers(&tx, season_id, player_id, offers)?;
    }

    tx.commit()?;
    Ok(())
}

fn preparar_persistencia_grids(
    conn: &Connection,
    grids: &[GridClasse],
    season_number: i32,
) -> Result<Vec<PendingOp>, DbError> {
    let total = grids.iter().map(|g| g.assignments.len()).sum::<usize>();
    let contract_ids = crate::generators::ids::next_ids(conn, IdType::Contract, total as u32)?;
    let mut contract_idx = 0;
    let mut team_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut driver_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for grid in grids {
        for assignment in &grid.assignments {
            if !team_map.contains_key(&assignment.team_id) {
                if let Ok(Some(team)) = team_queries::get_team_by_id(conn, &assignment.team_id) {
                    team_map.insert(team.id.clone(), team.nome.clone());
                }
            }
            if !driver_map.contains_key(&assignment.driver_id) {
                if let Ok(driver) = driver_queries::get_driver(conn, &assignment.driver_id) {
                    driver_map.insert(driver.id.clone(), driver.nome.clone());
                }
            }
        }
    }

    let class_to_cat: std::collections::HashMap<&str, &str> = CLASSES_CONVOCADAS
        .iter()
        .map(|cfg| (cfg.class_name, cfg.special_category))
        .collect();
    let mut ops = Vec::new();

    for grid in grids {
        let special_cat = class_to_cat
            .get(grid.class_name.as_str())
            .copied()
            .unwrap_or("unknown");

        for assignment in &grid.assignments {
            let contract_id = contract_ids[contract_idx].clone();
            contract_idx += 1;

            let team_nome = team_map
                .get(&assignment.team_id)
                .cloned()
                .unwrap_or_else(|| assignment.team_id.clone());
            let driver_nome = driver_map
                .get(&assignment.driver_id)
                .cloned()
                .unwrap_or_else(|| assignment.driver_id.clone());
            let papel = if assignment.papel == TeamRole::Numero1 {
                TeamRole::Numero1
            } else {
                TeamRole::Numero2
            };

            let contract = contract_queries::generate_especial_contract(
                contract_id,
                &assignment.driver_id,
                &driver_nome,
                &assignment.team_id,
                &team_nome,
                papel.clone(),
                special_cat,
                &grid.class_name,
                season_number,
            );

            ops.push(PendingOp {
                contract,
                driver_id: assignment.driver_id.clone(),
                special_category: special_cat.to_string(),
            });
        }
    }

    Ok(ops)
}

fn aplicar_persistencia_grids(conn: &Connection, ops: &[PendingOp]) -> Result<(), DbError> {
    driver_queries::clear_all_categoria_especial_ativa(conn)?;
    team_queries::clear_special_team_lineups(conn)?;
    team_queries::reset_special_team_hierarchies(conn)?;

    let mut team_lineups: std::collections::HashMap<String, (Option<String>, Option<String>)> =
        std::collections::HashMap::new();

    for op in ops {
        contract_queries::insert_contract(conn, &op.contract)?;
        driver_queries::update_driver_especial_category(
            conn,
            &op.driver_id,
            Some(&op.special_category),
        )?;

        let lineup = team_lineups
            .entry(op.contract.equipe_id.clone())
            .or_insert((None, None));
        match op.contract.papel {
            TeamRole::Numero1 => lineup.0 = Some(op.driver_id.clone()),
            TeamRole::Numero2 => lineup.1 = Some(op.driver_id.clone()),
        }
    }

    for (team_id, (n1, n2)) in &team_lineups {
        team_queries::update_team_pilots(conn, team_id, n1.as_deref(), n2.as_deref())?;
        if let (Some(n1_id), Some(n2_id)) = (n1, n2) {
            team_queries::update_team_hierarchy(
                conn,
                team_id,
                Some(n1_id.as_str()),
                Some(n2_id.as_str()),
                "Claro",
                0.0,
            )?;
        }
    }

    Ok(())
}
