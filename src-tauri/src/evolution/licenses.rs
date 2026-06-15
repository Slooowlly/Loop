use std::collections::HashMap;

use rusqlite::Connection;

use crate::common::time::current_timestamp;
use crate::constants::categories::get_category_config;
use crate::evolution::context::{LicenseEarned, StandingEntry};
use crate::models::license::granted_license_for_division;

pub(crate) fn persist_licenses(
    conn: &Connection,
    standings: &[StandingEntry],
    standings_by_driver: &HashMap<String, StandingEntry>,
) -> Result<Vec<LicenseEarned>, rusqlite::Error> {
    // Production/Endurance têm classificação por classe; a licença vale para a
    // metade superior de cada classe, não da categoria inteira.
    let mut grouped: HashMap<(&str, Option<&str>), Vec<&StandingEntry>> = HashMap::new();
    for standing in standings {
        grouped
            .entry((&standing.category, standing.classe.as_deref()))
            .or_default()
            .push(standing);
    }

    let timestamp = current_timestamp();
    let mut licenses_earned = Vec::new();
    for ((category, classe), mut entries) in grouped {
        entries.sort_by_key(|entry| entry.position);
        // A licença concedida é por divisão composta (categoria + classe): a
        // Endurance concede níveis distintos por classe (gt4=4, gt3=5, lmp2=6).
        let license_level = granted_license_for_division(category, classe)
            .or_else(|| get_category_config(category).map(|config| config.tier))
            .unwrap_or(0);
        let cutoff = entries.len().div_ceil(2);
        for standing in entries.into_iter().take(cutoff) {
            let seasons_in_category = standings_by_driver
                .get(&standing.driver_id)
                .map(|entry| entry.seasons_in_category)
                .unwrap_or(0);
            let inserted = conn.execute(
                "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
                 SELECT ?1, ?2, ?3, ?4, ?5
                 WHERE NOT EXISTS (
                     SELECT 1 FROM licenses WHERE piloto_id = ?1 AND nivel = ?2 AND categoria_origem = ?3
                 )",
                rusqlite::params![
                    &standing.driver_id,
                    license_level.to_string(),
                    category,
                    &timestamp,
                    seasons_in_category,
                ],
            )?;
            if inserted == 0 {
                continue;
            }

            licenses_earned.push(LicenseEarned {
                driver_id: standing.driver_id.clone(),
                driver_name: standing.driver_name.clone(),
                license_level,
                category: category.to_string(),
            });
        }
    }

    Ok(licenses_earned)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rusqlite::Connection;

    use super::*;
    use crate::db::migrations;
    use crate::db::queries::drivers as driver_queries;
    use crate::evolution::growth::SeasonStats;
    use crate::models::driver::Driver;

    #[test]
    fn test_persist_licenses_reports_only_newly_inserted_rows() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        let standing = sample_standing("P001", "Piloto A", "mazda_rookie", 2);
        insert_driver_fixture(&conn, &standing);
        let standings = vec![standing.clone()];
        let standings_by_driver = HashMap::from([(standing.driver_id.clone(), standing.clone())]);

        let first = persist_licenses(&conn, &standings, &standings_by_driver)
            .expect("first persist should succeed");
        let second = persist_licenses(&conn, &standings, &standings_by_driver)
            .expect("second persist should succeed");

        assert_eq!(first.len(), 1);
        assert!(
            second.is_empty(),
            "existing license should not be reported again"
        );
    }

    #[test]
    fn test_persist_licenses_stores_seasons_in_category() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        let standing = sample_standing("P001", "Piloto A", "mazda_rookie", 3);
        insert_driver_fixture(&conn, &standing);
        let standings = vec![standing.clone()];
        let standings_by_driver = HashMap::from([(standing.driver_id.clone(), standing.clone())]);

        persist_licenses(&conn, &standings, &standings_by_driver).expect("persist should succeed");

        let seasons_in_category: i32 = conn
            .query_row(
                "SELECT temporadas_na_categoria FROM licenses WHERE piloto_id = ?1",
                rusqlite::params![&standing.driver_id],
                |row| row.get(0),
            )
            .expect("stored license");
        assert_eq!(seasons_in_category, 3);
    }

    #[test]
    fn test_persist_licenses_grants_division_level_for_endurance_lmp2() {
        assert_granted_division_level("endurance", Some("lmp2"), 6);
    }

    #[test]
    fn test_persist_licenses_grants_division_level_for_endurance_gt3() {
        assert_granted_division_level("endurance", Some("gt3"), 5);
    }

    #[test]
    fn test_persist_licenses_grants_division_level_for_production_mazda() {
        assert_granted_division_level("production_challenger", Some("mazda"), 2);
    }

    fn assert_granted_division_level(category: &str, classe: Option<&str>, expected_level: i64) {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        let mut standing = sample_standing("P001", "Top Half", category, 2);
        standing.classe = classe.map(str::to_string);
        insert_driver_fixture(&conn, &standing);
        let standings = vec![standing.clone()];
        let standings_by_driver = HashMap::from([(standing.driver_id.clone(), standing.clone())]);

        persist_licenses(&conn, &standings, &standings_by_driver).expect("persist should succeed");

        let granted_level: i64 = conn
            .query_row(
                "SELECT MAX(CAST(nivel AS INTEGER)) FROM licenses WHERE piloto_id = ?1",
                rusqlite::params![&standing.driver_id],
                |row| row.get(0),
            )
            .expect("granted license");
        assert_eq!(granted_level, expected_level);
    }

    fn sample_standing(
        driver_id: &str,
        driver_name: &str,
        category: &str,
        seasons_in_category: i32,
    ) -> StandingEntry {
        StandingEntry {
            driver_id: driver_id.to_string(),
            driver_name: driver_name.to_string(),
            category: category.to_string(),
            classe: None,
            team_id: Some("T001".to_string()),
            position: 1,
            total_drivers: 1,
            seasons_in_category,
            stats: SeasonStats {
                posicao_campeonato: 1,
                total_pilotos: 1,
                pontos: 100,
                vitorias: 4,
                podios: 6,
                corridas: 10,
                dnfs: 0,
            },
        }
    }

    fn insert_driver_fixture(conn: &Connection, standing: &StandingEntry) {
        let mut driver = Driver::new(
            standing.driver_id.clone(),
            standing.driver_name.clone(),
            "Brasil".to_string(),
            "M".to_string(),
            21,
            2019,
        );
        driver.categoria_atual = Some(standing.category.clone());
        driver.temporadas_na_categoria = standing.seasons_in_category.saturating_sub(1) as u32;
        driver_queries::insert_driver(conn, &driver).expect("driver fixture");
    }
}
