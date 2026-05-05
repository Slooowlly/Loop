use std::collections::HashMap;

use rusqlite::Connection;

use crate::db::queries::contracts as contract_queries;
use crate::db::queries::teams as team_queries;
use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::enums::{ContractStatus, DriverStatus};

pub(crate) fn sync_team_slots_from_active_regular_contracts(
    conn: &Connection,
    teams: &[crate::models::team::Team],
    drivers_by_id: &HashMap<String, Driver>,
) -> Result<(), String> {
    let active_contracts = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao carregar contratos ativos: {e}"))?;
    let mut valid_contracts: HashMap<String, Vec<Contract>> = HashMap::new();

    for contract in active_contracts {
        let driver = drivers_by_id.get(&contract.piloto_id).ok_or_else(|| {
            format!(
                "Contrato ativo '{}' referencia piloto inexistente '{}'",
                contract.id, contract.piloto_id
            )
        })?;
        if driver.status == DriverStatus::Aposentado {
            contract_queries::update_contract_status(
                conn,
                &contract.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato invalido '{}': {e}",
                    contract.id
                )
            })?;
            continue;
        }
        valid_contracts
            .entry(contract.equipe_id.clone())
            .or_default()
            .push(contract);
    }

    for team in teams {
        let mut contracts = valid_contracts.remove(&team.id).unwrap_or_default();
        contracts.sort_by(|a, b| {
            let skill_a = drivers_by_id
                .get(&a.piloto_id)
                .expect("piloto validado deve existir para ordenacao")
                .atributos
                .skill;
            let skill_b = drivers_by_id
                .get(&b.piloto_id)
                .expect("piloto validado deve existir para ordenacao")
                .atributos
                .skill;
            skill_b.total_cmp(&skill_a)
        });
        let piloto_1 = contracts
            .first()
            .map(|contract| contract.piloto_id.as_str());
        let piloto_2 = contracts.get(1).map(|contract| contract.piloto_id.as_str());

        for contract in contracts.iter().take(2) {
            conn.execute(
                "UPDATE drivers SET categoria_atual = ?1 WHERE id = ?2",
                rusqlite::params![&contract.categoria, &contract.piloto_id],
            )
            .map_err(|e| {
                format!(
                    "Falha ao sincronizar categoria atual do piloto '{}': {e}",
                    contract.piloto_id
                )
            })?;
        }

        for contract in contracts.iter().skip(2) {
            contract_queries::update_contract_status(
                conn,
                &contract.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato excedente '{}': {e}",
                    contract.id
                )
            })?;
        }

        team_queries::update_team_pilots(conn, &team.id, piloto_1, piloto_2).map_err(|e| {
            format!(
                "Falha ao sincronizar pilotos da equipe '{}': {e}",
                team.nome
            )
        })?;
    }

    for contracts in valid_contracts.values() {
        for contract in contracts {
            contract_queries::update_contract_status(
                conn,
                &contract.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato fora da grade sincronizada '{}': {e}",
                    contract.id
                )
            })?;
        }
    }

    conn.execute(
        "UPDATE drivers SET categoria_atual = NULL
         WHERE categoria_atual IS NOT NULL
         AND id NOT IN (
             SELECT piloto_id FROM contracts
             WHERE status = 'Ativo' AND tipo = 'Regular'
         )",
        [],
    )
    .map_err(|e| format!("Falha ao limpar categoria_atual de pilotos sem contrato: {e}"))?;

    Ok(())
}
