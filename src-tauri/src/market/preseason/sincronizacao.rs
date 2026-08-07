//! Sincronização dos assentos das equipes com os contratos ativos e a contagem
//! de vagas restantes.

use super::*;

pub(super) fn sync_team_slots_from_active_contracts(conn: &Connection) -> Result<(), String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao carregar equipes: {e}"))?;
    let drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos: {e}"))?;
    let drivers_by_id = drivers
        .into_iter()
        .map(|driver| (driver.id.clone(), driver))
        .collect::<std::collections::HashMap<_, _>>();
    sync_team_slots_from_active_regular_contracts(conn, &teams, &drivers_by_id)
}

/// Assentos abertos AGRUPADOS por categoria. É a base do ritmo da escada: sem a
/// separação por categoria o teto semanal é global, as vagas de topo (que vêm primeiro na
/// ordem da escada) comem o orçamento inteiro, e as categorias de baixo só entram na
/// última semana — quando o fechamento preenche tudo de uma vez.
///
/// Conta só onde a escada pode assinar (categorias de contrato regular).
pub(super) fn vagas_por_categoria(
    conn: &Connection,
) -> Result<std::collections::HashMap<String, i32>, String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao contar vagas: {e}"))?;
    let mut por_categoria: std::collections::HashMap<String, i32> =
        std::collections::HashMap::new();
    for team in teams {
        if !crate::constants::categories::uses_regular_contracts(&team.categoria) {
            continue;
        }
        let abertas = i32::from(team.piloto_1_id.is_none()) + i32::from(team.piloto_2_id.is_none());
        if abertas > 0 {
            *por_categoria.entry(team.categoria.clone()).or_default() += abertas;
        }
    }
    Ok(por_categoria)
}

pub(super) fn count_remaining_vacancies(conn: &Connection) -> Result<i32, String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao contar vagas: {e}"))?;
    Ok(teams
        .iter()
        .map(|team| {
            let mut open = 0;
            if team.piloto_1_id.is_none() {
                open += 1;
            }
            if team.piloto_2_id.is_none() {
                open += 1;
            }
            open
        })
        .sum())
}
