use std::collections::HashMap;

use rusqlite::Connection;

use crate::constants::categories::get_all_categories;
use crate::constants::historical_timeline::{is_category_active_in_year, is_team_active_in_year};
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::teams as team_queries;
use crate::evolution::context::StandingEntry;
use crate::evolution::growth::SeasonStats;
use crate::models::contract::Contract;
use crate::models::season::Season;

pub(crate) fn build_and_persist_standings(
    conn: &Connection,
    season: &Season,
    contracts_by_driver: &HashMap<String, Contract>,
) -> Result<Vec<StandingEntry>, String> {
    conn.execute(
        "DELETE FROM standings WHERE temporada_id = ?1",
        rusqlite::params![&season.id],
    )
    .map_err(|e| format!("Falha ao limpar standings existentes: {e}"))?;

    let mut all_standings = Vec::new();
    let teams_by_id: HashMap<String, crate::models::team::Team> = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao buscar equipes para standings historicos: {e}"))?
        .into_iter()
        .map(|team| (team.id.clone(), team))
        .collect();

    for category in get_all_categories() {
        if !is_category_active_in_year(category.id, season.ano) {
            continue;
        }
        let mut drivers = driver_queries::get_drivers_by_category(conn, category.id)
            .map_err(|e| format!("Falha ao buscar pilotos de '{}': {e}", category.id))?;
        drivers.retain(|driver| {
            contracts_by_driver
                .get(&driver.id)
                .and_then(|contract| teams_by_id.get(&contract.equipe_id))
                .is_none_or(|team| is_team_active_in_year(team, season.ano))
        });
        if drivers.is_empty() {
            continue;
        }

        drivers.sort_by(|a, b| {
            b.stats_temporada
                .pontos
                .total_cmp(&a.stats_temporada.pontos)
                .then_with(|| b.stats_temporada.vitorias.cmp(&a.stats_temporada.vitorias))
                .then_with(|| b.stats_temporada.podios.cmp(&a.stats_temporada.podios))
                .then_with(|| a.nome.cmp(&b.nome))
        });

        let total_drivers = drivers.len() as i32;
        for (index, driver) in drivers.into_iter().enumerate() {
            let team_id = contracts_by_driver
                .get(&driver.id)
                .map(|contract| contract.equipe_id.clone());
            let standing = StandingEntry {
                driver_id: driver.id.clone(),
                driver_name: driver.nome.clone(),
                category: category.id.to_string(),
                team_id: team_id.clone(),
                position: index as i32 + 1,
                total_drivers,
                seasons_in_category: driver.temporadas_na_categoria as i32 + 1,
                stats: SeasonStats {
                    posicao_campeonato: index as i32 + 1,
                    total_pilotos: total_drivers,
                    pontos: driver.stats_temporada.pontos.round() as i32,
                    vitorias: driver.stats_temporada.vitorias as i32,
                    podios: driver.stats_temporada.podios as i32,
                    corridas: driver.stats_temporada.corridas as i32,
                    dnfs: driver.stats_temporada.dnfs as i32,
                },
            };

            conn.execute(
                "INSERT INTO standings (
                    temporada_id, piloto_id, equipe_id, categoria, posicao, pontos, vitorias, podios, poles, corridas
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    &season.id,
                    &standing.driver_id,
                    team_id.as_deref(),
                    &standing.category,
                    standing.position,
                    standing.stats.pontos as f64,
                    standing.stats.vitorias,
                    standing.stats.podios,
                    0,
                    standing.stats.corridas,
                ],
            )
            .map_err(|e| format!("Falha ao persistir standings: {e}"))?;

            all_standings.push(standing);
        }
    }

    Ok(all_standings)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rusqlite::Connection;

    use super::*;
    use crate::db::migrations;
    use crate::db::queries::drivers as driver_queries;
    use crate::db::queries::seasons as season_queries;
    use crate::models::driver::Driver;

    #[test]
    fn test_build_and_persist_standings_keeps_driver_without_regular_contract() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        let season = Season::new("S001".to_string(), 1, 2024);
        season_queries::insert_season(&conn, &season).expect("season insert");

        let mut driver = Driver::new(
            "P001".to_string(),
            "Piloto Livre".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2020,
        );
        driver.categoria_atual = Some("mazda_rookie".to_string());
        driver.stats_temporada.pontos = 80.0;
        driver.stats_temporada.vitorias = 2;
        driver.stats_temporada.podios = 4;
        driver.stats_temporada.corridas = 8;
        driver_queries::insert_driver(&conn, &driver).expect("driver insert");

        let standings = build_and_persist_standings(&conn, &season, &HashMap::new())
            .expect("standings should build");

        assert_eq!(standings.len(), 1);
        let persisted_team_id: Option<String> = conn
            .query_row(
                "SELECT equipe_id FROM standings WHERE temporada_id = ?1 AND piloto_id = ?2",
                rusqlite::params![&season.id, &driver.id],
                |row| row.get(0),
            )
            .expect("persisted standings row");
        assert!(
            persisted_team_id.is_none(),
            "driver without regular contract should still be persisted"
        );
    }
}
