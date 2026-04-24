use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::models::driver::Driver;
use crate::models::enums::DriverStatus;

/// As 4 fontes do grid especial.
#[derive(Debug, Clone, PartialEq)]
pub enum FonteConvocacao {
    /// A: pilotos do feeder com contrato regular ativo e bom desempenho.
    MeritoRegular,
    /// B: pilotos que já competiram nesta categoria especial+classe anteriormente.
    ContinuidadeHistorica,
    /// C: pilotos sem contrato regular nem especial ativo (pool global de livres).
    PoolGlobal,
    /// D: subconjunto excepcional reclassificado de A (wildcard narrativo).
    Wildcard,
}

#[derive(Debug, Clone)]
pub struct Candidato {
    pub driver_id: String,
    pub fonte: FonteConvocacao,
    pub driver: Driver,
}

/// Coleta todos os candidatos para uma classe específica de uma categoria especial.
/// Deduplicação: A > B > C. D é reclassificação interna de A (máx 1 por classe).
///
/// # Parâmetros
/// - `special_category`: "production_challenger" ou "endurance"
/// - `class_name`: ex "mazda", "gt3"
/// - `feeder_category`: categoria regular feeder da classe (ex "mazda_amador", "gt3")
pub fn coletar_candidatos(
    conn: &Connection,
    special_category: &str,
    class_name: &str,
    feeder_category: &str,
) -> Result<Vec<Candidato>, DbError> {
    let mut candidatos: Vec<Candidato> = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // ── Fonte A: MeritoRegular ────────────────────────────────────────────────
    let feeder_drivers = driver_queries::get_drivers_by_category(conn, feeder_category)?;
    let mut wildcard_candidates: Vec<Driver> = Vec::new();

    for driver in feeder_drivers {
        if driver.status != DriverStatus::Ativo {
            continue;
        }
        if !contract_queries::has_active_regular_contract(conn, &driver.id)? {
            continue;
        }
        if contract_queries::has_active_especial_contract(conn, &driver.id)? {
            continue;
        }
        let driver_id = driver.id.clone();

        // Verificar se é wildcard (reclassificação interna de A)
        if is_wildcard_candidate(&driver) {
            wildcard_candidates.push(driver);
        } else {
            candidatos.push(Candidato {
                driver_id: driver_id.clone(),
                fonte: FonteConvocacao::MeritoRegular,
                driver,
            });
        }
        seen_ids.insert(driver_id);
    }

    let selected_wildcard_id = wildcard_candidates
        .iter()
        .max_by(|a, b| {
            wildcard_sort_score(a)
                .total_cmp(&wildcard_sort_score(b))
                .then_with(|| b.atributos.skill.total_cmp(&a.atributos.skill))
                .then_with(|| b.stats_temporada.vitorias.cmp(&a.stats_temporada.vitorias))
                .then_with(|| a.nome.cmp(&b.nome))
        })
        .map(|driver| driver.id.clone());

    let mut wildcards: Vec<Candidato> = Vec::new();
    for driver in wildcard_candidates {
        if selected_wildcard_id.as_deref() == Some(driver.id.as_str()) {
            wildcards.push(Candidato {
                driver_id: driver.id.clone(),
                fonte: FonteConvocacao::Wildcard,
                driver,
            });
        } else {
            candidatos.push(Candidato {
                driver_id: driver.id.clone(),
                fonte: FonteConvocacao::MeritoRegular,
                driver,
            });
        }
    }

    // ── Fonte B: ContinuidadeHistorica ────────────────────────────────────────
    let historico_ids =
        contract_queries::get_pilots_with_especial_history(conn, special_category, class_name)?;

    for pid in historico_ids {
        if seen_ids.contains(&pid) {
            continue;
        }
        let driver = driver_queries::get_driver(conn, &pid)?;
        if driver.status != DriverStatus::Ativo {
            continue;
        }
        if contract_queries::has_active_especial_contract(conn, &pid)? {
            continue;
        }
        seen_ids.insert(pid.clone());
        candidatos.push(Candidato {
            driver_id: pid,
            fonte: FonteConvocacao::ContinuidadeHistorica,
            driver,
        });
    }

    // ── Fonte C: PoolGlobal ───────────────────────────────────────────────────
    let pool_drivers = driver_queries::get_drivers_without_active_contract(conn)?;

    for driver in pool_drivers {
        if seen_ids.contains(&driver.id) {
            continue;
        }
        let driver_id = driver.id.clone();
        seen_ids.insert(driver_id.clone());
        candidatos.push(Candidato {
            driver_id,
            fonte: FonteConvocacao::PoolGlobal,
            driver,
        });
    }

    // Adicionar wildcards ao final (serão distribuídos pela cota D no pipeline)
    candidatos.extend(wildcards);

    Ok(candidatos)
}

/// Critério de wildcard: talento excepcional do feeder regular.
/// Máximo 1 por classe; o pipeline aplica a cota.
fn is_wildcard_candidate(driver: &Driver) -> bool {
    let muito_jovem_e_talentoso = driver.idade < 21 && driver.atributos.skill > 75.0;
    let campiao_recente =
        driver.melhor_resultado_temp == Some(1) && driver.stats_temporada.vitorias >= 3;
    muito_jovem_e_talentoso || campiao_recente
}

fn wildcard_sort_score(driver: &Driver) -> f64 {
    let age_score = if driver.idade < 21 {
        (21_u32.saturating_sub(driver.idade)) as f64 * 100.0
    } else {
        0.0
    };
    let champion_score = if driver.melhor_resultado_temp == Some(1) {
        10_000.0 + driver.stats_temporada.vitorias as f64 * 100.0
    } else {
        0.0
    };
    champion_score + age_score + driver.atributos.skill
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rand::{rngs::StdRng, SeedableRng};
    use rusqlite::Connection;

    use super::*;
    use crate::db::migrations;
    use crate::generators::world::generate_world_with_rng;

    fn setup_world_db() -> (Connection, String) {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("migrations");

        let mut rng = StdRng::seed_from_u64(42);
        let world = generate_world_with_rng(
            "Test Player",
            "🇧🇷 Brasileiro",
            20,
            "mazda_rookie",
            0,
            "medio",
            &mut rng,
        )
        .expect("world generation");

        let season_id = "S001".to_string();
        let season = crate::models::season::Season::new(season_id.clone(), 1, 2024);
        crate::db::queries::seasons::insert_season(&conn, &season).expect("insert season");
        for driver in &world.drivers {
            driver_queries::insert_driver(&conn, driver).expect("insert driver");
        }
        crate::db::queries::teams::insert_teams(&conn, &world.teams).expect("insert teams");
        crate::db::queries::contracts::insert_contracts(&conn, &world.contracts)
            .expect("insert contracts");

        (conn, season_id)
    }

    #[test]
    fn test_coletar_candidatos_source_a_excludes_already_contracted() {
        let (conn, _) = setup_world_db();

        let candidatos =
            coletar_candidatos(&conn, "production_challenger", "mazda", "mazda_amador")
                .expect("coletar candidatos");

        // Nenhum candidato deve ter contrato especial ativo
        for c in &candidatos {
            assert!(
                !contract_queries::has_active_especial_contract(&conn, &c.driver_id)
                    .expect("check especial"),
                "piloto {} tem contrato especial mas apareceu como candidato",
                c.driver.nome
            );
        }
    }

    #[test]
    fn test_coletar_candidatos_dedup_across_sources() {
        let (conn, _) = setup_world_db();

        let candidatos =
            coletar_candidatos(&conn, "endurance", "gt3", "gt3").expect("coletar candidatos");

        // Nenhum driver_id duplicado
        let ids: HashSet<_> = candidatos.iter().map(|c| &c.driver_id).collect();
        assert_eq!(
            ids.len(),
            candidatos.len(),
            "driver_ids duplicados detectados"
        );
    }

    #[test]
    fn test_coletar_candidatos_source_a_only_active_drivers() {
        let (conn, _) = setup_world_db();

        let candidatos = coletar_candidatos(&conn, "production_challenger", "bmw", "bmw_m2")
            .expect("coletar candidatos");

        for c in &candidatos {
            if c.fonte == FonteConvocacao::MeritoRegular || c.fonte == FonteConvocacao::Wildcard {
                assert_eq!(
                    c.driver.status,
                    DriverStatus::Ativo,
                    "piloto de fonte A/D não está ativo"
                );
            }
        }
    }

    #[test]
    fn test_wildcard_max_one_per_class() {
        let (conn, _) = setup_world_db();

        let candidatos =
            coletar_candidatos(&conn, "endurance", "gt4", "gt4").expect("coletar candidatos");

        let wildcard_count = candidatos
            .iter()
            .filter(|c| c.fonte == FonteConvocacao::Wildcard)
            .count();

        assert!(
            wildcard_count <= 1,
            "mais de 1 wildcard na mesma classe: {}",
            wildcard_count
        );
    }

    #[test]
    fn test_pool_global_uses_contract_absence_not_categoria_null() {
        let (conn, _) = setup_world_db();

        // Todos os pilotos do pool não devem ter contrato ativo
        let candidatos =
            coletar_candidatos(&conn, "production_challenger", "toyota", "toyota_amador")
                .expect("coletar candidatos");

        for c in candidatos
            .iter()
            .filter(|c| c.fonte == FonteConvocacao::PoolGlobal)
        {
            let has_regular =
                contract_queries::has_active_regular_contract(&conn, &c.driver_id).expect("check");
            let has_especial =
                contract_queries::has_active_especial_contract(&conn, &c.driver_id).expect("check");
            assert!(
                !has_regular && !has_especial,
                "piloto do pool {} tem contrato ativo",
                c.driver.nome
            );
        }
    }

    #[test]
    fn test_wildcard_selects_best_candidate_not_first_name() {
        let (conn, _) = setup_world_db();

        let mut gt4_drivers =
            driver_queries::get_drivers_by_category(&conn, "gt4").expect("gt4 drivers should load");
        gt4_drivers.sort_by(|a, b| a.nome.cmp(&b.nome));
        let first_id = gt4_drivers[0].id.clone();
        let second_id = gt4_drivers[1].id.clone();

        let mut alpha = driver_queries::get_driver(&conn, &first_id).expect("alpha");
        alpha.nome = "Alpha Driver".to_string();
        alpha.idade = 20;
        alpha.atributos.skill = 76.0;
        alpha.stats_temporada.vitorias = 0;
        alpha.melhor_resultado_temp = None;
        driver_queries::update_driver(&conn, &alpha).expect("update alpha");

        let mut zulu = driver_queries::get_driver(&conn, &second_id).expect("zulu");
        zulu.nome = "Zulu Driver".to_string();
        zulu.idade = 18;
        zulu.atributos.skill = 92.0;
        zulu.stats_temporada.vitorias = 4;
        zulu.melhor_resultado_temp = Some(1);
        driver_queries::update_driver(&conn, &zulu).expect("update zulu");

        let candidatos =
            coletar_candidatos(&conn, "endurance", "gt4", "gt4").expect("coletar candidatos");
        let wildcard = candidatos
            .iter()
            .find(|c| c.fonte == FonteConvocacao::Wildcard)
            .expect("wildcard");

        assert_eq!(wildcard.driver_id, second_id);
    }

    #[test]
    fn test_historical_candidate_load_error_is_propagated() {
        let (conn, _) = setup_world_db();

        let driver = driver_queries::get_drivers_by_category(&conn, "gt3")
            .expect("gt3 drivers")
            .into_iter()
            .next()
            .expect("at least one gt3 driver");

        conn.execute(
            "UPDATE contracts
             SET tipo = 'Especial', categoria = 'endurance', classe = 'gt3', status = 'Expirado'
             WHERE piloto_id = ?1",
            rusqlite::params![&driver.id],
        )
        .expect("mutate history contract");
        conn.execute(
            "UPDATE drivers SET status = 'Quebrado' WHERE id = ?1",
            rusqlite::params![&driver.id],
        )
        .expect("corrupt driver status");

        let err = coletar_candidatos(&conn, "endurance", "gt3", "gt3").expect_err("should fail");
        assert!(
            err.to_string().contains("DriverStatus inválido")
                || err.to_string().contains("Quebrado"),
            "erro deveria propagar falha real de leitura: {err}"
        );
    }
}
