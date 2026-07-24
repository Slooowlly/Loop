//! Montagem da grade de largada: times, contratos e duplas de cada categoria, incluindo o caso especial dos eventos com classes.

use super::*;

pub(super) fn build_team_lookup(
    teams: &[crate::models::team::Team],
) -> HashMap<String, &crate::models::team::Team> {
    let mut lookup = HashMap::new();
    for team in teams {
        if let Some(driver_id) = &team.piloto_1_id {
            lookup.insert(driver_id.clone(), team);
        }
        if let Some(driver_id) = &team.piloto_2_id {
            lookup.insert(driver_id.clone(), team);
        }
    }
    lookup
}

pub(super) fn uses_regular_special_event_grid(category: &str) -> bool {
    matches!(category, "production_challenger" | "endurance")
}

pub(super) fn get_regular_special_event_teams(
    conn: &rusqlite::Connection,
    category: &str,
) -> Result<Vec<crate::models::team::Team>, DbError> {
    team_queries::get_teams_by_category(conn, category)
}

pub(super) fn get_regular_special_event_contracts(
    conn: &rusqlite::Connection,
    category: &str,
    grid_teams: &[crate::models::team::Team],
) -> Result<Vec<crate::models::contract::Contract>, String> {
    let active_contracts = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao buscar contratos regulares ativos: {e}"))?;
    let active_contracts = filter_regular_special_event_contracts(active_contracts, category);
    if !active_contracts.is_empty() {
        return Ok(active_contracts);
    }

    // Safety fallback for old saves/history imports that predate active regular
    // contracts in these real special-phase divisions. Normal new saves should
    // return through the active-contract path above.
    let mut fallback_contracts = Vec::new();
    fallback_contracts.extend(
        contract_queries::get_contracts_by_category(conn, category)
            .map_err(|e| format!("Falha ao buscar historico regular de contratos: {e}"))?,
    );

    // O histórico inclui contratos rescindidos; após promoção/rebaixamento parte
    // dessas equipes já saiu da categoria e não pertence mais ao grid.
    let grid_team_ids: std::collections::HashSet<&str> =
        grid_teams.iter().map(|team| team.id.as_str()).collect();
    fallback_contracts.retain(|contract| grid_team_ids.contains(contract.equipe_id.as_str()));

    Ok(filter_regular_special_event_contracts(
        fallback_contracts,
        category,
    ))
}

pub(super) fn filter_regular_special_event_contracts(
    contracts: Vec<crate::models::contract::Contract>,
    category: &str,
) -> Vec<crate::models::contract::Contract> {
    contracts
        .into_iter()
        .filter(|contract| match category {
            "production_challenger" => contract.categoria == "production_challenger",
            "endurance" => contract.categoria == "endurance",
            _ => contract.categoria == category,
        })
        .filter(|contract| contract.tipo == crate::models::enums::ContractType::Regular)
        .collect()
}

pub(super) fn get_drivers_for_contracts(
    conn: &rusqlite::Connection,
    contracts: &[crate::models::contract::Contract],
) -> Result<Vec<Driver>, String> {
    let mut drivers = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for contract in contracts {
        if !seen.insert(contract.piloto_id.clone()) {
            continue;
        }
        let driver = driver_queries::get_driver(conn, &contract.piloto_id).map_err(|e| {
            format!(
                "Falha ao buscar piloto contratado '{}': {e}",
                contract.piloto_id
            )
        })?;
        drivers.push(driver);
    }

    Ok(drivers)
}

pub(super) fn get_drivers_for_team_lineups(
    conn: &rusqlite::Connection,
    teams: &[crate::models::team::Team],
) -> Result<Vec<Driver>, String> {
    let mut drivers = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for team in teams {
        for pilot_id in [team.piloto_1_id.as_ref(), team.piloto_2_id.as_ref()]
            .into_iter()
            .flatten()
        {
            if !seen.insert(pilot_id.clone()) {
                continue;
            }
            let driver = driver_queries::get_driver(conn, pilot_id)
                .map_err(|e| format!("Falha ao buscar piloto do lineup '{}': {e}", pilot_id))?;
            drivers.push(driver);
        }
    }

    Ok(drivers)
}

pub(super) fn build_regular_contract_team_lookup<'a>(
    contracts: &[crate::models::contract::Contract],
    teams: &'a [crate::models::team::Team],
) -> HashMap<String, &'a crate::models::team::Team> {
    let teams_by_id: HashMap<&str, &crate::models::team::Team> =
        teams.iter().map(|team| (team.id.as_str(), team)).collect();
    let mut lookup = HashMap::new();

    for contract in contracts {
        if let Some(team) = teams_by_id.get(contract.equipe_id.as_str()) {
            lookup.insert(contract.piloto_id.clone(), *team);
        }
    }

    lookup
}

pub(super) fn build_special_team_lookup<'a>(
    conn: &rusqlite::Connection,
    teams: &'a [crate::models::team::Team],
    category: &str,
) -> Result<HashMap<String, &'a crate::models::team::Team>, String> {
    let teams_by_id: HashMap<&str, &crate::models::team::Team> =
        teams.iter().map(|team| (team.id.as_str(), team)).collect();
    let contracts = contract_queries::get_active_especial_contracts_by_category(conn, category)
        .map_err(|e| format!("Falha ao buscar contratos especiais ativos: {e}"))?;
    let mut lookup = HashMap::new();

    for contract in contracts {
        if let Some(team) = teams_by_id.get(contract.equipe_id.as_str()) {
            lookup.insert(contract.piloto_id, *team);
        }
    }

    Ok(lookup)
}

pub(super) fn apply_special_class_scoring(
    result: &mut RaceResult,
    teams: &[crate::models::team::Team],
    is_endurance: bool,
) {
    let class_by_team: HashMap<&str, &str> = teams
        .iter()
        .map(|team| {
            (
                team.id.as_str(),
                team.classe.as_deref().unwrap_or(team.categoria.as_str()),
            )
        })
        .collect();
    let mut result_indexes_by_class: HashMap<String, Vec<usize>> = HashMap::new();

    for (index, entry) in result.race_results.iter().enumerate() {
        let class_name = class_by_team
            .get(entry.team_id.as_str())
            .copied()
            .unwrap_or("geral");
        result_indexes_by_class
            .entry(class_name.to_string())
            .or_default()
            .push(index);
    }

    let fastest_lap_id = result.fastest_lap_id.clone();
    for indexes in result_indexes_by_class.values_mut() {
        indexes.sort_by(|left, right| {
            let left_result = &result.race_results[*left];
            let right_result = &result.race_results[*right];
            left_result
                .is_dnf
                .cmp(&right_result.is_dnf)
                .then_with(|| {
                    left_result
                        .finish_position
                        .cmp(&right_result.finish_position)
                })
                .then_with(|| left_result.pilot_name.cmp(&right_result.pilot_name))
        });

        for (class_index, result_index) in indexes.iter().enumerate() {
            let class_position = class_index as i32 + 1;
            let entry = &mut result.race_results[*result_index];
            entry.finish_position = class_position;
            entry.positions_gained = entry.grid_position - class_position;
            entry.points_earned = if entry.is_dnf {
                0
            } else {
                get_points_for_position(class_position as u8, is_endurance) as i32
            };
            if !entry.is_dnf && entry.pilot_id == fastest_lap_id && class_position <= 10 {
                entry.points_earned += BONUS_FASTEST_LAP as i32;
            }
        }
    }
}

pub(super) fn group_results_by_team(
    result: &RaceResult,
) -> HashMap<String, Vec<&crate::simulation::race::RaceDriverResult>> {
    let mut grouped: HashMap<String, Vec<&crate::simulation::race::RaceDriverResult>> =
        HashMap::new();
    for driver_result in &result.race_results {
        grouped
            .entry(driver_result.team_id.clone())
            .or_default()
            .push(driver_result);
    }
    grouped
}
