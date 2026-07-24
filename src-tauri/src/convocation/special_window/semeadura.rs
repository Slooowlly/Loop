//! Semeadura inicial da janela: pool de candidatos, agenda de revelação dos
//! grids e o escalonamento das ofertas do jogador por dia.

use super::*;

pub(super) fn seed_candidate_pool(conn: &Connection, season_id: &str) -> Result<(), DbError> {
    let license_levels = load_license_levels(conn)?;
    let mut drivers: HashMap<String, CandidateAccumulator> = HashMap::new();

    for cfg in legacy_window_classes() {
        let candidatos = coletar_candidatos(
            conn,
            cfg.special_category,
            cfg.class_name,
            cfg.feeder_category,
        )?;

        for candidato in candidatos {
            let historico = contract_queries::get_especial_contract_count(
                conn,
                &candidato.driver_id,
                cfg.special_category,
                cfg.class_name,
            )
            .unwrap_or(0);
            let score =
                calcular_score(&candidato.driver, &candidato.fonte, historico).round() as i32;
            let license_level = license_levels.get(&candidato.driver_id).copied();

            let entry = drivers
                .entry(candidato.driver_id.clone())
                .or_insert_with(|| CandidateAccumulator {
                    driver_name: candidato.driver.nome.clone(),
                    origin_category: candidato
                        .driver
                        .categoria_atual
                        .clone()
                        .unwrap_or_else(|| cfg.feeder_category.to_string()),
                    license_level,
                    desirability: score,
                    production_eligible: false,
                    endurance_eligible: false,
                });

            entry.desirability = entry.desirability.max(score);
            if entry.origin_category.is_empty() {
                entry.origin_category = candidato
                    .driver
                    .categoria_atual
                    .clone()
                    .unwrap_or_else(|| cfg.feeder_category.to_string());
            }
            if entry.license_level.is_none() {
                entry.license_level = license_level;
            }

            match cfg.special_category {
                "production_challenger" => {
                    if license_level.unwrap_or(0) >= 1 {
                        entry.production_eligible = true;
                    }
                }
                "endurance" => {
                    if license_level.unwrap_or(0) >= 3 && score >= 80 {
                        entry.endurance_eligible = true;
                    }
                }
                _ => {}
            }
        }
    }

    for (driver_id, entry) in drivers {
        conn.execute(
            "INSERT INTO special_window_candidate_pool (
                season_id, driver_id, driver_name, origin_category, license_level,
                desirability, production_eligible, endurance_eligible, status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'Livre')",
            params![
                season_id,
                driver_id,
                entry.driver_name,
                entry.origin_category,
                entry.license_level.map(|value| value as i64),
                entry.desirability,
                entry.production_eligible as i64,
                entry.endurance_eligible as i64,
            ],
        )?;
    }

    Ok(())
}

pub(super) fn seed_assignment_schedule(
    conn: &Connection,
    season_id: &str,
    grids: &[GridClasse],
) -> Result<(), DbError> {
    for grid in grids {
        let mut ranked = grid.assignments.clone();
        ranked.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let total = ranked.len().max(1);

        for (index, assignment) in ranked.iter().enumerate() {
            let Some(team) = team_queries::get_team_by_id(conn, &assignment.team_id)? else {
                continue;
            };
            let special_category = legacy_window_classes()
                .find(|cfg| cfg.class_name == grid.class_name)
                .map(|cfg| cfg.special_category)
                .unwrap_or(team.categoria.as_str());
            let reveal_day = schedule_reveal_day(index, total, team.car_strength(), &team.id);
            conn.execute(
                "INSERT INTO special_window_assignments (
                    id, season_id, special_category, class_name, team_id, driver_id, papel,
                    reveal_day, revealed, is_player
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, 0)",
                params![
                    format!(
                        "SWA-{season_id}-{}-{}-{}",
                        assignment.team_id,
                        assignment.driver_id,
                        assignment.papel.as_str()
                    ),
                    season_id,
                    special_category,
                    grid.class_name,
                    assignment.team_id,
                    assignment.driver_id,
                    assignment.papel.as_str(),
                    reveal_day,
                ],
            )?;
        }
    }

    Ok(())
}

pub(super) fn schedule_player_offer_days(
    conn: &Connection,
    season_id: &str,
    player: &Driver,
) -> Result<(), DbError> {
    let desirability = derive_player_desirability(player);
    let base_day = if desirability >= 92 {
        1
    } else if desirability >= 84 {
        2
    } else if desirability >= 76 {
        3
    } else if desirability >= 68 {
        4
    } else {
        5
    };

    let mut stmt = conn.prepare(
        "SELECT pso.id, COALESCE(t.car_performance, 50.0) AS perf
         FROM player_special_offers pso
         LEFT JOIN teams t ON t.id = pso.team_id
         WHERE pso.season_id = ?1 AND pso.player_driver_id = ?2
         ORDER BY perf DESC, pso.team_name ASC",
    )?;
    let rows = stmt.query_map(params![season_id, player.id.clone()], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;

    let mut ordered = Vec::new();
    for row in rows {
        ordered.push(row?);
    }

    for (index, (offer_id, _)) in ordered.iter().enumerate() {
        let available_from_day = (base_day + index as i32).clamp(1, TOTAL_SPECIAL_WINDOW_DAYS);
        conn.execute(
            "UPDATE player_special_offers
             SET available_from_day = ?1, selected_for_day = 0
             WHERE season_id = ?2 AND id = ?3",
            params![available_from_day, season_id, offer_id],
        )?;
    }

    Ok(())
}
