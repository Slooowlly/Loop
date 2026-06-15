use std::collections::HashMap;

use rusqlite::Connection;

use crate::constants::categories::{get_all_categories, is_multiclass_category};
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
        if is_multiclass_category(category.id) {
            let entries = build_special_category_standings(conn, season, category.id)?;
            for standing in &entries {
                persist_standing_row(conn, season, standing)?;
            }
            all_standings.extend(entries);
            continue;
        }
        let mut drivers = driver_queries::get_drivers_by_active_category(conn, category.id)
            .map_err(|e| format!("Falha ao buscar pilotos de '{}': {e}", category.id))?;
        drivers.retain(|driver| {
            contracts_by_driver
                .get(&driver.id)
                .and_then(|contract| teams_by_id.get(&contract.equipe_id))
                .is_none_or(|team| is_team_active_in_year(team, season.ano))
        });
        drivers.retain(has_season_participation);
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
                classe: None,
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

            persist_standing_row(conn, season, &standing)?;

            all_standings.push(standing);
        }
    }

    Ok(all_standings)
}

fn persist_standing_row(
    conn: &Connection,
    season: &Season,
    standing: &StandingEntry,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO standings (
            temporada_id, piloto_id, equipe_id, categoria, classe, posicao, pontos, vitorias, podios, poles, corridas
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            &season.id,
            &standing.driver_id,
            standing.team_id.as_deref(),
            &standing.category,
            standing.classe.as_deref(),
            standing.position,
            standing.stats.pontos as f64,
            standing.stats.vitorias,
            standing.stats.podios,
            0,
            standing.stats.corridas,
        ],
    )
    .map_err(|e| format!("Falha ao persistir standings: {e}"))?;
    Ok(())
}

struct SpecialClassRow {
    driver_id: String,
    driver_name: String,
    seasons_in_category: i32,
    team_id: String,
    classe: String,
    pontos: f64,
    vitorias: i32,
    podios: i32,
    corridas: i32,
    dnfs: i32,
}

/// Production/Endurance: o campeonato é decidido por classe (mazda/toyota/bmw e
/// gt4/gt3/lmp2). Os pontos vêm dos resultados oficiais das corridas da categoria
/// (já calculados por classe na simulação), e não de stats_temporada — pilotos de
/// lmp2 acumulam ali também os pontos do campeonato regular de LMP2.
fn build_special_category_standings(
    conn: &Connection,
    season: &Season,
    category_id: &str,
) -> Result<Vec<StandingEntry>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                rr.piloto_id,
                d.nome,
                d.temporadas_na_categoria,
                MAX(rr.equipe_id) AS equipe_id,
                NULLIF(TRIM(t.classe), '') AS classe,
                COALESCE(SUM(rr.pontos), 0.0) AS pontos,
                SUM(CASE WHEN rr.dnf = 0 AND rr.posicao_final = 1 THEN 1 ELSE 0 END) AS vitorias,
                SUM(CASE WHEN rr.dnf = 0 AND rr.posicao_final <= 3 THEN 1 ELSE 0 END) AS podios,
                COUNT(*) AS corridas,
                SUM(CASE WHEN rr.dnf = 1 THEN 1 ELSE 0 END) AS dnfs
             FROM race_results rr
             JOIN calendar c ON c.id = rr.race_id
             JOIN drivers d ON d.id = rr.piloto_id
             JOIN teams t ON t.id = rr.equipe_id
             WHERE COALESCE(c.season_id, c.temporada_id) = ?1
               AND c.categoria = ?2
               AND NULLIF(TRIM(t.classe), '') IS NOT NULL
             GROUP BY rr.piloto_id, NULLIF(TRIM(t.classe), '')",
        )
        .map_err(|e| format!("Falha ao preparar standings especiais: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![&season.id, category_id], |row| {
            Ok(SpecialClassRow {
                driver_id: row.get(0)?,
                driver_name: row.get(1)?,
                seasons_in_category: row.get::<_, i64>(2)? as i32,
                team_id: row.get(3)?,
                classe: row.get(4)?,
                pontos: row.get(5)?,
                vitorias: row.get(6)?,
                podios: row.get(7)?,
                corridas: row.get(8)?,
                dnfs: row.get(9)?,
            })
        })
        .map_err(|e| format!("Falha ao consultar standings especiais: {e}"))?;

    let mut rows_by_class: HashMap<String, Vec<SpecialClassRow>> = HashMap::new();
    for row in rows {
        let row = row.map_err(|e| format!("Falha ao mapear standings especiais: {e}"))?;
        rows_by_class
            .entry(row.classe.clone())
            .or_default()
            .push(row);
    }

    let mut entries = Vec::new();
    for class_rows in rows_by_class.values_mut() {
        class_rows.sort_by(|a, b| {
            b.pontos
                .total_cmp(&a.pontos)
                .then_with(|| b.vitorias.cmp(&a.vitorias))
                .then_with(|| b.podios.cmp(&a.podios))
                .then_with(|| a.driver_name.cmp(&b.driver_name))
        });

        let total_drivers = class_rows.len() as i32;
        for (index, row) in class_rows.iter().enumerate() {
            let position = index as i32 + 1;
            entries.push(StandingEntry {
                driver_id: row.driver_id.clone(),
                driver_name: row.driver_name.clone(),
                category: category_id.to_string(),
                classe: Some(row.classe.clone()),
                team_id: Some(row.team_id.clone()),
                position,
                total_drivers,
                seasons_in_category: row.seasons_in_category + 1,
                stats: SeasonStats {
                    posicao_campeonato: position,
                    total_pilotos: total_drivers,
                    pontos: row.pontos.round() as i32,
                    vitorias: row.vitorias,
                    podios: row.podios,
                    corridas: row.corridas,
                    dnfs: row.dnfs,
                },
            });
        }
    }

    Ok(entries)
}

fn has_season_participation(driver: &crate::models::driver::Driver) -> bool {
    driver.stats_temporada.corridas > 0
        || driver.stats_temporada.pontos > 0.0
        || driver.stats_temporada.vitorias > 0
        || driver.stats_temporada.podios > 0
        || driver.stats_temporada.poles > 0
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

    #[test]
    fn test_build_and_persist_standings_ignores_drivers_without_season_participation() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        let season = Season::new("S001".to_string(), 1, 2026);
        season_queries::insert_season(&conn, &season).expect("season insert");

        let mut participant = Driver::new(
            "P001".to_string(),
            "Participante".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2024,
        );
        participant.categoria_atual = Some("gt3".to_string());
        participant.stats_temporada.corridas = 3;
        participant.stats_temporada.pontos = 42.0;
        driver_queries::insert_driver(&conn, &participant).expect("participant insert");

        let mut inactive = Driver::new(
            "P002".to_string(),
            "Sem Corrida".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2024,
        );
        inactive.categoria_atual = Some("gt3".to_string());
        driver_queries::insert_driver(&conn, &inactive).expect("inactive insert");

        let standings = build_and_persist_standings(&conn, &season, &HashMap::new())
            .expect("standings should build");

        assert!(standings.iter().any(|entry| entry.driver_id == "P001"));
        assert!(!standings.iter().any(|entry| entry.driver_id == "P002"));
    }

    #[test]
    fn test_special_standings_are_ranked_per_class_from_race_results() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        let season = Season::new("S001".to_string(), 1, 2025);
        season_queries::insert_season(&conn, &season).expect("season insert");
        seed_special_race(
            &conn,
            "production_challenger",
            &[
                (
                    "P_MZ1",
                    "T_MZ1",
                    Some("mazda"),
                    "production_challenger",
                    1,
                    35.0,
                ),
                (
                    "P_MZ2",
                    "T_MZ2",
                    Some("mazda"),
                    "production_challenger",
                    2,
                    28.0,
                ),
                (
                    "P_TY1",
                    "T_TY1",
                    Some("toyota"),
                    "production_challenger",
                    1,
                    35.0,
                ),
                (
                    "P_BW1",
                    "T_BW1",
                    Some("bmw"),
                    "production_challenger",
                    1,
                    35.0,
                ),
            ],
        );

        let standings = build_and_persist_standings(&conn, &season, &HashMap::new())
            .expect("standings should build");

        let production: Vec<_> = standings
            .iter()
            .filter(|entry| entry.category == "production_challenger")
            .collect();
        assert_eq!(production.len(), 4);

        // Cada classe tem o proprio campeao; nao existe campeao geral obrigatorio.
        let champions: Vec<_> = production
            .iter()
            .filter(|entry| entry.position == 1)
            .collect();
        assert_eq!(champions.len(), 3);

        let mz2 = production
            .iter()
            .find(|entry| entry.driver_id == "P_MZ2")
            .expect("P_MZ2 standing");
        assert_eq!(mz2.position, 2);
        assert_eq!(mz2.total_drivers, 2);
        assert_eq!(mz2.classe.as_deref(), Some("mazda"));

        let persisted_classe: String = conn
            .query_row(
                "SELECT classe FROM standings
                 WHERE temporada_id = 'S001' AND piloto_id = 'P_MZ2'",
                [],
                |row| row.get(0),
            )
            .expect("classe persisted");
        assert_eq!(persisted_classe, "mazda");
    }

    #[test]
    fn test_special_standings_endurance_uses_lmp2_class() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        let season = Season::new("S001".to_string(), 1, 2025);
        season_queries::insert_season(&conn, &season).expect("season insert");
        seed_special_race(
            &conn,
            "endurance",
            &[
                ("P_GT4", "T_GT4", Some("gt4"), "endurance", 1, 35.0),
                ("P_GT3", "T_GT3", Some("gt3"), "endurance", 1, 35.0),
                ("P_LMP", "T_LMP", Some("lmp2"), "endurance", 1, 35.0),
            ],
        );

        let standings = build_and_persist_standings(&conn, &season, &HashMap::new())
            .expect("standings should build");

        let lmp = standings
            .iter()
            .find(|entry| entry.driver_id == "P_LMP" && entry.category == "endurance")
            .expect("lmp2 driver in endurance standings");
        assert_eq!(lmp.classe.as_deref(), Some("lmp2"));
        assert_eq!(lmp.position, 1);
    }

    fn seed_special_race(
        conn: &Connection,
        categoria: &str,
        results: &[(&str, &str, Option<&str>, &str, i32, f64)],
    ) {
        conn.execute(
            "INSERT INTO calendar (
                id, temporada_id, season_id, rodada, pista, categoria, status, nome,
                track_name, track_config
             ) VALUES ('R001', 'S001', 'S001', 1, 'Interlagos', ?1, 'Concluida', 'R001',
                'Interlagos', 'default')",
            rusqlite::params![categoria],
        )
        .expect("seed calendar");

        for (driver_id, team_id, classe, team_categoria, posicao, pontos) in results {
            conn.execute(
                "INSERT INTO drivers (
                    id, nome, idade, nacionalidade, genero, categoria_atual, status,
                    ano_inicio_carreira
                 ) VALUES (?1, ?1, 25, 'Brasil', 'M', ?2, 'Ativo', 2020)",
                rusqlite::params![driver_id, team_categoria],
            )
            .expect("seed driver");
            conn.execute(
                "INSERT INTO teams (
                    id, nome, nome_curto, categoria, classe, ativa, created_at, updated_at
                 ) VALUES (?1, ?1, ?1, ?2, ?3, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
                rusqlite::params![team_id, team_categoria, classe],
            )
            .expect("seed team");
            conn.execute(
                "INSERT INTO race_results (
                    race_id, piloto_id, equipe_id, posicao_largada, posicao_final,
                    voltas_completadas, pontos
                 ) VALUES ('R001', ?1, ?2, ?3, ?3, 10, ?4)",
                rusqlite::params![driver_id, team_id, posicao, pontos],
            )
            .expect("seed race result");
        }
    }
}
