//! Resolução da identidade de carreira de um carro da sessão (piloto + equipe).

use std::collections::HashMap;

use rusqlite::Connection;

use crate::db::queries::{contracts, teams};
use crate::models::driver::Driver;

/// Identidade resolvida de um carro da sessão (piloto + equipe da carreira).
pub(super) struct CarIdentity {
    pub(super) driver_id: String,
    pub(super) driver_name: String,
    pub(super) team_id: String,
    pub(super) team_name: String,
}

/// Resolve `driver_id`/nome/equipe de um carro. O jogador vem do
/// `player_driver`; a IA vem do mapa número→`driver_id` salvo na geração do
/// roster. Sem casamento, devolve um placeholder estável (não quebra o pipeline).
pub(super) fn resolve_identity(
    conn: &Connection,
    car_number: i32,
    is_player: bool,
    player_driver: Option<&Driver>,
    by_number: &HashMap<i64, String>,
) -> CarIdentity {
    let driver_id = if is_player {
        player_driver.map(|d| d.id.clone())
    } else {
        by_number.get(&(car_number as i64)).cloned()
    };

    let driver_id = match driver_id {
        Some(id) => id,
        None => {
            return CarIdentity {
                driver_id: format!("car-{}", car_number),
                driver_name: format!("Carro #{}", car_number),
                team_id: String::new(),
                team_name: "—".to_string(),
            };
        }
    };

    // Nome: jogador já em mãos; IA busca no banco.
    let driver_name = if is_player {
        player_driver.map(|d| d.nome.clone())
    } else {
        crate::db::queries::drivers::get_driver(conn, &driver_id)
            .ok()
            .map(|d| d.nome)
    }
    .unwrap_or_else(|| format!("Carro #{}", car_number));

    // Equipe: contrato regular ativo → equipe.
    let (team_id, team_name) = contracts::get_active_regular_contract_for_pilot(conn, &driver_id)
        .ok()
        .flatten()
        .map(|c| {
            let name = teams::get_team_by_id(conn, &c.equipe_id)
                .ok()
                .flatten()
                .map(|t| t.nome)
                .unwrap_or_else(|| c.equipe_id.clone());
            (c.equipe_id, name)
        })
        .unwrap_or_else(|| (String::new(), "—".to_string()));

    CarIdentity {
        driver_id,
        driver_name,
        team_id,
        team_name,
    }
}
