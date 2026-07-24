//! Contexto lido uma vez no início da virada: equipes e contratos regulares ativos.

use super::*;

pub(super) fn build_context(
    conn: &Connection,
) -> Result<(HashMap<String, Team>, HashMap<String, Contract>), String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao buscar equipes: {e}"))?;
    let teams_by_id: HashMap<String, Team> = teams
        .into_iter()
        .map(|team| (team.id.clone(), team))
        .collect();
    let active_contracts = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao buscar contratos regulares ativos: {e}"))?;
    let contracts_by_driver: HashMap<String, Contract> = active_contracts
        .into_iter()
        .map(|contract| (contract.piloto_id.clone(), contract))
        .collect();
    Ok((teams_by_id, contracts_by_driver))
}
