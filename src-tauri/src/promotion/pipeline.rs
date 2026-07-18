use rand::Rng;
use rusqlite::Connection;

use std::collections::HashSet;

use crate::db::queries::drivers as driver_queries;
use crate::db::queries::teams as team_queries;
use crate::promotion::block1::execute_block1_for_year;
use crate::promotion::block2::execute_block2_with_exclusions;
use crate::promotion::block3::execute_block3_for_year;
use crate::models::team::Team;
use crate::promotion::effects::{
    apply_attribute_deltas, calculate_promotion_effects, calculate_relegation_effects,
    promotion_car_landing_delta,
};
use crate::promotion::pilots::{apply_pilot_effect, resolve_pilot_situations};
use crate::promotion::{MovementType, PromotionResult, TeamMovement};

/// Soft-landing da promoção (Ideia 1), atrás de flag pra permitir A/B no Monte
/// Carlo sem tocar no jogo real. `IRACER_PROMO_SOFT_LANDING=1` liga; retorna a
/// margem (pontos de carro acima do pior incumbente) — `IRACER_PROMO_LANDING_MARGIN`
/// ajusta (default 1.0). `None` = comportamento default (mantém o carro atual).
fn promotion_soft_landing_margin() -> Option<f64> {
    let enabled = std::env::var("IRACER_PROMO_SOFT_LANDING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !enabled {
        return None;
    }
    let margin = std::env::var("IRACER_PROMO_LANDING_MARGIN")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(1.0);
    Some(margin)
}

/// car_performance dos incumbentes que PERMANECEM na categoria de destino do
/// promovido — mesma categoria (e classe, quando houver), excluindo o próprio
/// promovido. Nesta altura do pipeline a rebaixada já saiu na troca (as trocas de
/// categoria foram aplicadas antes do loop de deltas), então o campo é só quem fica.
fn promotion_field_cars(conn: &Connection, team: &Team) -> Result<Vec<f64>, String> {
    let teams = match team.classe.as_deref() {
        Some(class) => {
            team_queries::get_teams_by_category_and_class(conn, &team.categoria, class)
        }
        None => team_queries::get_teams_by_category(conn, &team.categoria),
    }
    .map_err(|e| format!("Falha ao buscar campo da categoria '{}': {e}", team.categoria))?;
    Ok(teams
        .into_iter()
        .filter(|t| t.id != team.id)
        .map(|t| t.car_performance)
        .collect())
}

fn with_savepoint<T, F>(conn: &Connection, name: &str, action: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    conn.execute_batch(&format!("SAVEPOINT {name}"))
        .map_err(|e| format!("Falha ao abrir savepoint '{name}': {e}"))?;

    match action() {
        Ok(value) => {
            conn.execute_batch(&format!("RELEASE SAVEPOINT {name}"))
                .map_err(|e| format!("Falha ao confirmar savepoint '{name}': {e}"))?;
            Ok(value)
        }
        Err(err) => {
            conn.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {name}; RELEASE SAVEPOINT {name};"
            ))
            .map_err(|rollback_err| {
                format!("{err}; alem disso falhou o rollback do savepoint '{name}': {rollback_err}")
            })?;
            Err(err)
        }
    }
}

#[cfg(test)]
pub fn run_promotion_relegation(
    conn: &Connection,
    season_number: i32,
    rng: &mut impl Rng,
) -> Result<PromotionResult, String> {
    run_promotion_relegation_for_year(conn, season_number, i32::MAX, rng)
}

pub(crate) fn run_promotion_relegation_for_year(
    conn: &Connection,
    season_number: i32,
    season_year: i32,
    rng: &mut impl Rng,
) -> Result<PromotionResult, String> {
    if season_number < 1 {
        return Ok(PromotionResult::empty());
    }

    with_savepoint(conn, "promotion_run", || {
        let mut all_movements = Vec::new();
        let block1_movements = execute_block1_for_year(conn, season_year, rng)?;
        let excluded_from_block2: HashSet<String> = block1_movements
            .iter()
            .filter(|movement| {
                movement.movement_type == MovementType::Rebaixamento
                    && (movement.from_category == "mazda_amador"
                        || movement.from_category == "toyota_amador")
            })
            .map(|movement| movement.team_id.clone())
            .collect();
        let block2_movements = execute_block2_with_exclusions(
            conn,
            season_number,
            season_year,
            &excluded_from_block2,
            rng,
        )?;
        let block3_movements = execute_block3_for_year(conn, season_number, season_year, rng)?;

        all_movements.extend(block1_movements);
        all_movements.extend(block2_movements);
        all_movements.extend(block3_movements);

        // Zera a categoria de origem de todas as equipes antes de aplicar os
        // movimentos desta temporada, para o badge de movimento da pré-temporada
        // (e os ajustes de car build / pit) refletirem apenas quem se moveu agora,
        // sem acumular rebaixamentos/promoções de temporadas anteriores.
        team_queries::clear_all_categoria_anterior(conn)
            .map_err(|e| format!("Falha ao limpar categoria anterior das equipes: {e}"))?;
        for movement in &all_movements {
            apply_team_category_change(conn, movement)?;
        }

        let pilot_effects = resolve_pilot_situations(conn, &all_movements)?;
        for effect in &pilot_effects {
            apply_pilot_effect(conn, effect, &all_movements)?;
        }

        let soft_landing_margin = promotion_soft_landing_margin();
        let mut attribute_deltas = Vec::new();
        for movement in &all_movements {
            let team = team_queries::get_team_by_id(conn, &movement.team_id)
                .map_err(|e| format!("Falha ao buscar equipe '{}': {e}", movement.team_id))?
                .ok_or_else(|| format!("Equipe '{}' nao encontrada", movement.team_id))?;
            let delta = match movement.movement_type {
                MovementType::Promocao => {
                    let mut delta = calculate_promotion_effects(&team, rng);
                    // Ideia 1 (soft landing): substitui o car_performance_delta default
                    // (0.0, mantém carro atual) por um alvo relativo ao campo de destino.
                    if let Some(margin) = soft_landing_margin {
                        let field = promotion_field_cars(conn, &team)?;
                        if let Some(car_delta) =
                            promotion_car_landing_delta(team.car_performance, &field, margin)
                        {
                            delta.car_performance_delta = car_delta;
                        }
                    }
                    delta
                }
                MovementType::Rebaixamento => calculate_relegation_effects(&team, rng),
            };
            apply_attribute_deltas(conn, &movement.team_id, &delta)?;
            attribute_deltas.push(delta);
        }

        let mut errors = verify_team_driver_consistency(conn, &all_movements);
        errors.extend(verify_category_sizes(conn)?);
        Ok(PromotionResult {
            movements: all_movements,
            pilot_effects,
            attribute_deltas,
            errors,
        })
    })
}

fn apply_team_category_change(conn: &Connection, movement: &TeamMovement) -> Result<(), String> {
    let mut team = team_queries::get_team_by_id(conn, &movement.team_id)
        .map_err(|e| format!("Falha ao buscar equipe '{}': {e}", movement.team_id))?
        .ok_or_else(|| format!("Equipe '{}' nao encontrada", movement.team_id))?;
    // Persiste a categoria de origem para exibição na pré-temporada
    team.categoria_anterior = Some(team.categoria.clone());
    team.categoria = movement.to_category.clone();
    team.classe = infer_team_class(movement);
    team_queries::update_team(conn, &team)
        .map_err(|e| format!("Falha ao atualizar equipe '{}': {e}", team.nome))?;
    // Sistema de Nível do Carro: re-ancora o carro ao teto da nova categoria no ATO da
    // mudança. Um time rebaixado não pode carregar um carro acima do teto de onde chegou
    // (regride ao teto, valendo já na 1ª corrida); um promovido mantém o carro — abaixo do
    // novo teto — e evolui (soft landing).
    if let Some(car) = team.car.as_mut() {
        let ceiling = crate::car::cost::category_ceiling(&team.categoria);
        for part in car.parts.iter_mut() {
            if part.level > ceiling {
                part.level = ceiling;
            }
        }
        crate::db::queries::team_car::upsert_team_car(conn, &team.id, car)
            .map_err(|e| format!("Falha ao re-ancorar carro de '{}': {e}", team.nome))?;
    }
    // Pilar C: a mudança de categoria zera o plano estratégico — a próxima
    // pré-temporada re-avalia o arco para a nova realidade da equipe.
    team_queries::reset_strategic_plan(conn, &team.id)
        .map_err(|e| format!("Falha ao resetar plano estratégico de '{}': {e}", team.nome))?;
    Ok(())
}

fn infer_team_class(movement: &TeamMovement) -> Option<String> {
    match movement.to_category.as_str() {
        "production_challenger" => match movement.from_category.as_str() {
            "mazda_amador" => Some("mazda".to_string()),
            "toyota_amador" => Some("toyota".to_string()),
            "bmw_m2" => Some("bmw".to_string()),
            _ => None,
        },
        "endurance" => match movement.from_category.as_str() {
            "gt4" => Some("gt4".to_string()),
            "gt3" => Some("gt3".to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn verify_team_driver_consistency(conn: &Connection, movements: &[TeamMovement]) -> Vec<String> {
    let mut errors = Vec::new();
    for movement in movements {
        let Ok(Some(team)) = team_queries::get_team_by_id(conn, &movement.team_id) else {
            continue;
        };
        // Production/Endurance têm equipes e contratos reais desde a Fase 3A:
        // pilotos usam categoria_atual como em qualquer categoria.
        for pilot_id in [team.piloto_1_id.as_deref(), team.piloto_2_id.as_deref()]
            .into_iter()
            .flatten()
        {
            match driver_queries::get_driver(conn, pilot_id) {
                Err(_) => errors.push(format!(
                    "CONSISTENCIA: equipe '{}' referencia piloto '{pilot_id}' inexistente",
                    team.nome
                )),
                Ok(driver) => {
                    if driver.categoria_atual.as_deref() != Some(team.categoria.as_str()) {
                        errors.push(format!(
                            "CONSISTENCIA: piloto '{}' tem categoria '{:?}' mas equipe '{}' esta em '{}'",
                            driver.nome, driver.categoria_atual, team.nome, team.categoria
                        ));
                    }
                }
            }
        }
    }
    errors
}

fn verify_category_sizes(conn: &Connection) -> Result<Vec<String>, String> {
    let expected_sizes = [("bmw_m2", 10), ("gt4", 10), ("gt3", 14)];
    let mut errors = Vec::new();
    errors.extend(verify_ladder_pool_size(
        conn,
        "mazda_rookie",
        "mazda_amador",
        16,
    )?);
    errors.extend(verify_ladder_pool_size(
        conn,
        "toyota_rookie",
        "toyota_amador",
        16,
    )?);
    for (category, expected) in expected_sizes {
        let actual = team_queries::count_teams_by_category(conn, category)
            .map_err(|e| format!("Falha ao contar equipes em '{category}': {e}"))?;
        if actual != expected {
            errors.push(format!(
                "INVARIANTE VIOLADO: {category} tem {actual} equipes (esperado {expected})"
            ));
        }
    }
    for class_name in ["mazda", "toyota", "bmw"] {
        let actual = team_queries::count_teams_by_category_and_class(
            conn,
            "production_challenger",
            class_name,
        )
        .map_err(|e| {
            format!("Falha ao contar equipes da classe '{class_name}' na Production: {e}")
        })?;
        if actual != 6 {
            errors.push(format!(
                "INVARIANTE VIOLADO: production_challenger classe {class_name} tem {actual} equipes (esperado 6)"
            ));
        }
    }
    // A classe lmp2 da Endurance é representada pelas equipes da categoria
    // lmp2 (cobertas pelo expected_sizes acima); só gt4/gt3 têm equipes
    // próprias de Endurance.
    for class_name in ["gt4", "gt3", "lmp2"] {
        let actual = team_queries::count_teams_by_category_and_class(conn, "endurance", class_name)
            .map_err(|e| {
                format!("Falha ao contar equipes da classe '{class_name}' na Endurance: {e}")
            })?;
        if actual != 6 {
            errors.push(format!(
                "INVARIANTE VIOLADO: endurance classe {class_name} tem {actual} equipes (esperado 6)"
            ));
        }
    }
    Ok(errors)
}

fn verify_ladder_pool_size(
    conn: &Connection,
    rookie_category: &str,
    amateur_category: &str,
    expected_total: i32,
) -> Result<Vec<String>, String> {
    let rookie_count = team_queries::count_teams_by_category(conn, rookie_category)
        .map_err(|e| format!("Falha ao contar equipes em '{rookie_category}': {e}"))?;
    let amateur_count = team_queries::count_teams_by_category(conn, amateur_category)
        .map_err(|e| format!("Falha ao contar equipes em '{amateur_category}': {e}"))?;
    let actual_total = rookie_count + amateur_count;
    if actual_total == expected_total {
        return Ok(Vec::new());
    }

    Ok(vec![format!(
        "INVARIANTE VIOLADO: {rookie_category}/{amateur_category} tem {actual_total} equipes (esperado {expected_total})"
    )])
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};
    use rusqlite::Connection;

    use super::*;
    use crate::db::migrations;
    use crate::db::queries::contracts as contract_queries;
    use crate::db::queries::drivers as driver_queries;
    use crate::db::queries::teams as team_queries;
    use crate::models::contract::Contract;
    use crate::models::driver::Driver;
    use crate::models::enums::TeamRole;
    use crate::models::team::Team;

    #[test]
    fn test_no_promotion_invalid_season() {
        let conn = setup_promotion_db();
        let mut rng = StdRng::seed_from_u64(50);

        let result = run_promotion_relegation(&conn, 0, &mut rng).expect("promotion should run");

        assert!(result.movements.is_empty());
        assert!(result.pilot_effects.is_empty());
        assert!(result.attribute_deltas.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_full_promotion_season_1() {
        let conn = setup_promotion_db();
        let mut rng = StdRng::seed_from_u64(50);

        let result = run_promotion_relegation(&conn, 1, &mut rng).expect("promotion should run");

        assert_eq!(result.movements.len(), 14);
        assert!(!result.attribute_deltas.is_empty());
    }

    #[test]
    fn test_full_promotion_all_blocks() {
        let conn = setup_promotion_db();
        let mut rng = StdRng::seed_from_u64(51);

        let result = run_promotion_relegation(&conn, 2, &mut rng).expect("promotion should run");

        assert_eq!(result.movements.len(), 14);
        assert!(!result.attribute_deltas.is_empty());
    }

    #[test]
    fn test_production_swap_moves_team_pilots_and_contracts_per_class() {
        let conn = setup_promotion_db();
        let mut rng = StdRng::seed_from_u64(60);
        for (amateur_category, champion_id, relegated_id, class_name) in [
            ("mazda_amador", "MA1", "PM6", "mazda"),
            ("toyota_amador", "TA1", "PT6", "toyota"),
            ("bmw_m2", "BM1", "PB6", "bmw"),
        ] {
            attach_pilot_with_contract(
                &conn,
                champion_id,
                &format!("UP_{class_name}"),
                amateur_category,
            );
            attach_pilot_with_contract(
                &conn,
                relegated_id,
                &format!("DOWN_{class_name}"),
                "production_challenger",
            );
        }

        let result = run_promotion_relegation(&conn, 1, &mut rng).expect("promotion should run");
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

        for (amateur_category, champion_id, relegated_id, class_name) in [
            ("mazda_amador", "MA1", "PM6", "mazda"),
            ("toyota_amador", "TA1", "PT6", "toyota"),
            ("bmw_m2", "BM1", "PB6", "bmw"),
        ] {
            let promoted = team_queries::get_team_by_id(&conn, champion_id)
                .expect("team query")
                .expect("promoted team");
            assert_eq!(promoted.categoria, "production_challenger");
            assert_eq!(promoted.classe.as_deref(), Some(class_name));

            let relegated = team_queries::get_team_by_id(&conn, relegated_id)
                .expect("team query")
                .expect("relegated team");
            assert_eq!(relegated.categoria, amateur_category);
            assert_eq!(relegated.classe, None);

            // Pilotos acompanham a equipe nas duas direções, com contrato regular
            // unico apontando para a nova categoria e licenca compativel.
            let up_driver = driver_queries::get_driver(&conn, &format!("UP_{class_name}"))
                .expect("promoted driver");
            assert_eq!(
                up_driver.categoria_atual.as_deref(),
                Some("production_challenger")
            );
            assert!(up_driver.categoria_especial_ativa.is_none());
            assert!(
                crate::models::license::driver_has_required_license_for_division(
                    &conn,
                    &up_driver.id,
                    "production_challenger",
                    Some(class_name),
                )
                .expect("license query")
            );

            let down_driver = driver_queries::get_driver(&conn, &format!("DOWN_{class_name}"))
                .expect("relegated driver");
            assert_eq!(
                down_driver.categoria_atual.as_deref(),
                Some(amateur_category)
            );
            assert!(down_driver.categoria_especial_ativa.is_none());

            for driver_id in [&up_driver.id, &down_driver.id] {
                let active_regular_count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM contracts
                         WHERE piloto_id = ?1 AND tipo = 'Regular' AND status = 'Ativo'",
                        rusqlite::params![driver_id],
                        |row| row.get(0),
                    )
                    .expect("contract count");
                assert_eq!(active_regular_count, 1, "piloto '{driver_id}'");
                let especial_count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM contracts
                         WHERE piloto_id = ?1 AND tipo = 'Especial'",
                        rusqlite::params![driver_id],
                        |row| row.get(0),
                    )
                    .expect("especial count");
                assert_eq!(
                    especial_count, 0,
                    "nenhum contrato Especial deve ser criado"
                );
            }

            let up_contract = contract_queries::get_active_regular_contract_for_pilot(
                &conn,
                &format!("UP_{class_name}"),
            )
            .expect("contract query")
            .expect("promoted contract");
            assert_eq!(up_contract.categoria, "production_challenger");

            let down_contract = contract_queries::get_active_regular_contract_for_pilot(
                &conn,
                &format!("DOWN_{class_name}"),
            )
            .expect("contract query")
            .expect("relegated contract");
            assert_eq!(down_contract.categoria, amateur_category);
        }

        // Invariantes de tamanho: Production segue 6/6/6 e nenhuma equipe
        // aparece em duas categorias (categoria é coluna única por equipe).
        for class_name in ["mazda", "toyota", "bmw"] {
            let count = team_queries::count_teams_by_category_and_class(
                &conn,
                "production_challenger",
                class_name,
            )
            .expect("count by class");
            assert_eq!(count, 6);
        }
        for (category, expected) in [("mazda_amador", 10), ("toyota_amador", 10), ("bmw_m2", 10)] {
            let count =
                team_queries::count_teams_by_category(&conn, category).expect("count by category");
            assert_eq!(count, expected, "{category}");
        }
    }

    #[test]
    fn test_endurance_swap_moves_team_pilots_and_contracts_per_class() {
        let conn = setup_promotion_db();
        let mut rng = StdRng::seed_from_u64(61);
        for (feeder_category, champion_id, relegated_id, class_name) in [
            ("gt4", "GT41", "EG46", "gt4"),
            ("gt3", "GT31", "EG36", "gt3"),
        ] {
            attach_pilot_with_contract(
                &conn,
                champion_id,
                &format!("EUP_{class_name}"),
                feeder_category,
            );
            attach_pilot_with_contract(
                &conn,
                relegated_id,
                &format!("EDOWN_{class_name}"),
                "endurance",
            );
        }

        let result = run_promotion_relegation(&conn, 1, &mut rng).expect("promotion should run");
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

        for (feeder_category, champion_id, relegated_id, class_name) in [
            ("gt4", "GT41", "EG46", "gt4"),
            ("gt3", "GT31", "EG36", "gt3"),
        ] {
            let promoted = team_queries::get_team_by_id(&conn, champion_id)
                .expect("team query")
                .expect("promoted team");
            assert_eq!(promoted.categoria, "endurance");
            assert_eq!(promoted.classe.as_deref(), Some(class_name));

            let relegated = team_queries::get_team_by_id(&conn, relegated_id)
                .expect("team query")
                .expect("relegated team");
            assert_eq!(relegated.categoria, feeder_category);
            assert_eq!(relegated.classe, None);

            let up_driver = driver_queries::get_driver(&conn, &format!("EUP_{class_name}"))
                .expect("promoted driver");
            assert_eq!(up_driver.categoria_atual.as_deref(), Some("endurance"));
            assert!(up_driver.categoria_especial_ativa.is_none());
            assert!(
                crate::models::license::driver_has_required_license_for_division(
                    &conn,
                    &up_driver.id,
                    "endurance",
                    Some(class_name),
                )
                .expect("license query"),
                "piloto promovido deve receber a licenca da Endurance"
            );

            let down_driver = driver_queries::get_driver(&conn, &format!("EDOWN_{class_name}"))
                .expect("relegated driver");
            assert_eq!(
                down_driver.categoria_atual.as_deref(),
                Some(feeder_category)
            );
            assert!(down_driver.categoria_especial_ativa.is_none());

            for driver_id in [&up_driver.id, &down_driver.id] {
                let active_regular_count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM contracts
                         WHERE piloto_id = ?1 AND tipo = 'Regular' AND status = 'Ativo'",
                        rusqlite::params![driver_id],
                        |row| row.get(0),
                    )
                    .expect("contract count");
                assert_eq!(active_regular_count, 1, "piloto '{driver_id}'");
                let especial_count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM contracts
                         WHERE piloto_id = ?1 AND tipo = 'Especial'",
                        rusqlite::params![driver_id],
                        |row| row.get(0),
                    )
                    .expect("especial count");
                assert_eq!(
                    especial_count, 0,
                    "nenhum contrato Especial deve ser criado"
                );
            }

            let up_contract = contract_queries::get_active_regular_contract_for_pilot(
                &conn,
                &format!("EUP_{class_name}"),
            )
            .expect("contract query")
            .expect("promoted contract");
            assert_eq!(up_contract.categoria, "endurance");

            let down_contract = contract_queries::get_active_regular_contract_for_pilot(
                &conn,
                &format!("EDOWN_{class_name}"),
            )
            .expect("contract query")
            .expect("relegated contract");
            assert_eq!(down_contract.categoria, feeder_category);
        }

        // Invariantes: Endurance segue 6/6/6, feeders mantêm tamanho e LMP2
        // fica fixa nos dois lados.
        for class_name in ["gt4", "gt3", "lmp2"] {
            let count =
                team_queries::count_teams_by_category_and_class(&conn, "endurance", class_name)
                    .expect("endurance count");
            assert_eq!(count, 6, "endurance classe {class_name}");
        }
        for (category, expected) in [("gt4", 10), ("gt3", 14)] {
            let count =
                team_queries::count_teams_by_category(&conn, category).expect("count by category");
            assert_eq!(count, expected, "{category}");
        }
        let lmp2_count = team_queries::count_teams_by_category(&conn, "lmp2").expect("lmp2 count");
        assert_eq!(lmp2_count, 0);
    }

    #[test]
    fn test_promotion_never_moves_lmp2_teams() {
        let conn = setup_promotion_db();
        let mut rng = StdRng::seed_from_u64(62);

        let result = run_promotion_relegation(&conn, 1, &mut rng).expect("promotion should run");

        assert!(result
            .movements
            .iter()
            .all(|movement| movement.from_category != "lmp2"
                && movement.to_category != "lmp2"
                && !movement.team_id.starts_with("LMP")));
        let lmp2_count = team_queries::count_teams_by_category(&conn, "lmp2").expect("lmp2 count");
        let endurance_lmp2 =
            team_queries::count_teams_by_category_and_class(&conn, "endurance", "lmp2")
                .expect("endurance lmp2 count");
        assert_eq!(lmp2_count, 0);
        assert_eq!(endurance_lmp2, 6);
    }

    fn attach_pilot_with_contract(
        conn: &Connection,
        team_id: &str,
        driver_id: &str,
        category: &str,
    ) {
        let mut driver = Driver::new(
            driver_id.to_string(),
            driver_id.to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2020,
        );
        driver.categoria_atual = Some(category.to_string());
        driver_queries::insert_driver(conn, &driver).expect("insert driver");
        team_queries::update_team_pilots(conn, team_id, Some(driver_id), None)
            .expect("attach pilot");
        let contract = Contract::new(
            format!("C_{driver_id}"),
            driver_id.to_string(),
            driver_id.to_string(),
            team_id.to_string(),
            team_id.to_string(),
            1,
            2,
            80_000.0,
            TeamRole::Numero1,
            category.to_string(),
        );
        contract_queries::insert_contract(conn, &contract).expect("insert contract");
    }

    #[test]
    fn test_invariant_maintained() {
        let conn = setup_promotion_db();
        let mut rng = StdRng::seed_from_u64(52);

        let result = run_promotion_relegation(&conn, 2, &mut rng).expect("promotion should run");

        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn test_run_promotion_relegation_rolls_back_if_pilot_effect_fails_midway() {
        let conn = setup_promotion_db();
        let mut rng = StdRng::seed_from_u64(50);

        let mut driver = Driver::new(
            "P901".to_string(),
            "Piloto Teste".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2020,
        );
        driver.categoria_atual = Some("mazda_rookie".to_string());
        driver_queries::insert_driver(&conn, &driver).expect("insert driver");
        team_queries::update_team_pilots(&conn, "MR1", Some("P901"), None)
            .expect("attach driver to champion team");

        let champion_team = team_queries::get_team_by_id(&conn, "MR1")
            .expect("team query")
            .expect("team exists");
        let contract = Contract::new(
            "C901".to_string(),
            "P901".to_string(),
            "Piloto Teste".to_string(),
            champion_team.id.clone(),
            champion_team.nome.clone(),
            1,
            2,
            50_000.0,
            TeamRole::Numero1,
            "mazda_rookie".to_string(),
        );
        contract_queries::insert_contract(&conn, &contract).expect("insert contract");

        conn.execute("DROP TABLE contracts", [])
            .expect("drop contracts table");

        let err = run_promotion_relegation(&conn, 1, &mut rng)
            .expect_err("promotion should fail when pilot effect cannot update contract");

        assert!(
            err.contains("contrato regular") || err.contains("contratos da equipe"),
            "unexpected error: {err}"
        );

        let team = team_queries::get_team_by_id(&conn, "MR1")
            .expect("team query after failure")
            .expect("team still exists");
        assert_eq!(team.categoria, "mazda_rookie");
        assert_eq!(team.categoria_anterior, None);

        let driver = driver_queries::get_driver(&conn, "P901").expect("driver query after failure");
        assert_eq!(driver.categoria_atual.as_deref(), Some("mazda_rookie"));
    }

    #[test]
    fn test_promotion_clears_stale_previous_category_for_teams_that_stay() {
        let conn = setup_promotion_db();
        let mut rng = StdRng::seed_from_u64(7);

        // Simula um rebaixamento de temporada passada que ficou registrado no time.
        let mut stable = team_queries::get_team_by_id(&conn, "MA5")
            .expect("team query")
            .expect("team exists");
        stable.categoria_anterior = Some("bmw_m2".to_string());
        team_queries::update_team(&conn, &stable).expect("seed stale previous category");

        let result = run_promotion_relegation(&conn, 1, &mut rng).expect("promotion should run");

        // MA5 (meio de tabela) não se move nesta temporada: o campo precisa estar
        // limpo para não exibir badge fantasma de movimento na pré-temporada.
        let stable = team_queries::get_team_by_id(&conn, "MA5")
            .expect("team query after promotion")
            .expect("team exists");
        assert_eq!(stable.categoria, "mazda_amador");
        assert_eq!(stable.categoria_anterior, None);

        // Quem realmente se moveu nesta temporada continua com a categoria de origem.
        let moved = result.movements.first().expect("at least one movement");
        let moved_team = team_queries::get_team_by_id(&conn, &moved.team_id)
            .expect("moved team query")
            .expect("moved team exists");
        assert_eq!(
            moved_team.categoria_anterior.as_deref(),
            Some(moved.from_category.as_str())
        );
    }

    #[test]
    fn test_promotion_field_cars_excludes_self_and_scopes_to_category() {
        let conn = setup_promotion_db();
        // gt3 tem 14 equipes; o campo visto por uma delas deve ter 13 (exclui a própria).
        let gt3_team = team_queries::get_team_by_id(&conn, "GT31")
            .expect("team query")
            .expect("gt3 team");
        let field = promotion_field_cars(&conn, &gt3_team).expect("field cars");
        assert_eq!(field.len(), 13, "13 incumbentes (14 - a própria)");

        // Para uma categoria com classe (endurance/gt3), o campo é escopado à classe.
        let end_gt3 = team_queries::get_team_by_id(&conn, "EG31")
            .expect("team query")
            .expect("endurance gt3 team");
        let end_field = promotion_field_cars(&conn, &end_gt3).expect("field cars");
        assert_eq!(end_field.len(), 5, "6 da classe gt3 - a própria");
    }

    #[test]
    fn test_verify_category_sizes_propagates_database_errors() {
        let conn = setup_promotion_db();
        conn.execute("DROP TABLE teams", [])
            .expect("drop teams table");

        let err = verify_category_sizes(&conn).expect_err("count failure should propagate");

        assert!(
            err.contains("Falha ao contar equipes"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_verify_category_sizes_allows_historical_ladder_redistribution() {
        let conn = setup_promotion_db();
        conn.execute(
            "UPDATE teams SET categoria = 'mazda_rookie' WHERE id IN ('MA7', 'MA8', 'MA9', 'MA10')",
            [],
        )
        .expect("move mazda teams into historical feeder pool");
        conn.execute(
            "UPDATE teams SET categoria = 'toyota_rookie' WHERE id IN ('TA7', 'TA8', 'TA9', 'TA10')",
            [],
        )
        .expect("move toyota teams into historical feeder pool");

        let errors = verify_category_sizes(&conn).expect("size verification should run");

        assert!(errors.is_empty(), "unexpected size errors: {errors:?}");
    }

    fn setup_promotion_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        insert_ranked_teams(&conn, "mazda_rookie", "MR", 6, None);
        insert_ranked_teams(&conn, "toyota_rookie", "TR", 6, None);
        insert_ranked_teams(&conn, "mazda_amador", "MA", 10, None);
        insert_ranked_teams(&conn, "toyota_amador", "TA", 10, None);
        insert_ranked_teams(&conn, "bmw_m2", "BM", 10, None);
        insert_ranked_teams(&conn, "production_challenger", "PM", 6, Some("mazda"));
        insert_ranked_teams(&conn, "production_challenger", "PT", 6, Some("toyota"));
        insert_ranked_teams(&conn, "production_challenger", "PB", 6, Some("bmw"));
        insert_ranked_teams(&conn, "gt4", "GT4", 10, None);
        insert_ranked_teams(&conn, "gt3", "GT3", 14, None);
        insert_ranked_teams(&conn, "endurance", "EG4", 6, Some("gt4"));
        insert_ranked_teams(&conn, "endurance", "EG3", 6, Some("gt3"));
        insert_ranked_teams(&conn, "endurance", "LMP", 6, Some("lmp2"));

        conn
    }

    fn insert_ranked_teams(
        conn: &Connection,
        category: &str,
        prefix: &str,
        count: usize,
        class: Option<&str>,
    ) {
        for index in 0..count {
            let rank = index + 1;
            let mut team = sample_team(
                category,
                &format!("{prefix}{rank}"),
                &format!("{prefix} Team {rank}"),
                class,
            );
            team.stats_pontos = ((count - index) * 10) as i32;
            team.stats_vitorias = (count - index) as i32;
            team.stats_melhor_resultado = rank as i32;
            team_queries::insert_team(conn, &team).expect("insert ranked team");
        }
    }

    fn sample_team(category: &str, id: &str, name: &str, class: Option<&str>) -> Team {
        let template = crate::constants::teams::get_reference_team_template(category, class)
            .expect("team template");
        let mut rng = StdRng::seed_from_u64(id.bytes().map(u64::from).sum());
        let mut team =
            Team::from_template_with_rng(template, category, id.to_string(), 2025, &mut rng);
        team.nome = name.to_string();
        team.nome_curto = name.to_string();
        team.classe = class.map(str::to_string);
        team
    }
}
