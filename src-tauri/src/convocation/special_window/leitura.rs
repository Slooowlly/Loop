//! Leitura do estado da janela para montar o payload: seções visíveis por
//! equipe, candidatos elegíveis, ofertas do jogador e o log enriquecido.

use super::*;

pub(super) fn get_window_state(conn: &Connection, season_id: &str) -> Result<Option<WindowStateRow>, DbError> {
    conn.query_row(
        "SELECT current_day, total_days, status, active_offer_id, player_result
         FROM special_window_state
         WHERE season_id = ?1
         LIMIT 1",
        params![season_id],
        |row| {
            Ok(WindowStateRow {
                current_day: row.get(0)?,
                total_days: row.get(1)?,
                status: row.get(2)?,
                active_offer_id: row.get(3)?,
                player_result: row.get(4)?,
            })
        },
    )
    .optional()
    .map_err(DbError::from)
}

pub(super) fn load_visible_team_sections(
    conn: &Connection,
    season_id: &str,
    current_day: i32,
) -> Result<Vec<SpecialWindowCategorySection>, DbError> {
    if !has_legacy_window_classes() {
        return Ok(Vec::new());
    }

    let visible = load_visible_assignments(conn, season_id, current_day)?;
    let mut categories = legacy_window_classes()
        .map(|cfg| cfg.special_category)
        .collect::<Vec<_>>();
    categories.sort_unstable();
    categories.dedup();
    let mut sections = Vec::new();

    for category in categories {
        let Some(category_config) = get_category_config(category) else {
            continue;
        };
        let mut teams = Vec::new();

        for class_info in category_config.classes {
            let class_teams = special_entry_queries::get_entry_teams_for_class(
                conn,
                season_id,
                category,
                class_info.class_name,
            )?;
            for team in class_teams {
                let pilot_1 = visible
                    .iter()
                    .find(|assignment| {
                        assignment.team_id == team.id && assignment.papel == TeamRole::Numero1
                    })
                    .cloned();
                let pilot_2 = visible
                    .iter()
                    .find(|assignment| {
                        assignment.team_id == team.id && assignment.papel == TeamRole::Numero2
                    })
                    .cloned();

                let piloto_1_nome = pilot_1
                    .as_ref()
                    .map(|assignment| driver_queries::get_driver(conn, &assignment.driver_id))
                    .transpose()?
                    .map(|driver| driver.nome);
                let piloto_2_nome = pilot_2
                    .as_ref()
                    .map(|assignment| driver_queries::get_driver(conn, &assignment.driver_id))
                    .transpose()?
                    .map(|driver| driver.nome);

                teams.push(SpecialWindowTeamSummary {
                    id: team.id.clone(),
                    nome: team.nome.clone(),
                    nome_curto: team.nome_curto.clone(),
                    cor_primaria: team.cor_primaria.clone(),
                    cor_secundaria: team.cor_secundaria.clone(),
                    categoria: team.categoria.clone(),
                    classe: Some(class_info.class_name.to_string()),
                    piloto_1_id: pilot_1
                        .as_ref()
                        .map(|assignment| assignment.driver_id.clone()),
                    piloto_1_nome,
                    piloto_1_new_badge_day: pilot_1
                        .as_ref()
                        .and_then(|assignment| assignment.new_badge_day),
                    piloto_2_id: pilot_2
                        .as_ref()
                        .map(|assignment| assignment.driver_id.clone()),
                    piloto_2_nome,
                    piloto_2_new_badge_day: pilot_2
                        .as_ref()
                        .and_then(|assignment| assignment.new_badge_day),
                });
            }
        }

        sections.push(SpecialWindowCategorySection {
            category: category.to_string(),
            label: category_config.nome_curto.to_string(),
            teams,
        });
    }

    Ok(sections)
}

pub(super) fn load_eligible_candidates(
    conn: &Connection,
    season_id: &str,
) -> Result<Vec<SpecialWindowEligibleCandidate>, DbError> {
    if !has_legacy_window_classes() {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        "SELECT driver_id, driver_name, origin_category, license_level, desirability,
                production_eligible, endurance_eligible
         FROM special_window_candidate_pool
         WHERE season_id = ?1 AND status = 'Livre'
         ORDER BY desirability DESC, driver_name ASC",
    )?;
    let rows = stmt.query_map(params![season_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, i32>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
        ))
    })?;

    let rankings = build_visible_category_rankings(conn)?;
    let mut result = Vec::new();
    for row in rows {
        let (
            driver_id,
            driver_name,
            origin_category,
            license_level,
            desirability,
            _production,
            _endurance,
        ) = row?;
        let driver = driver_queries::get_driver(conn, &driver_id)?;
        let Some(regular_contract) =
            contract_queries::get_active_regular_contract_for_pilot(conn, &driver_id)?
        else {
            continue;
        };

        let current_category = driver
            .categoria_atual
            .clone()
            .filter(|category| !category.is_empty())
            .or_else(|| {
                if regular_contract.categoria.is_empty() {
                    None
                } else {
                    Some(regular_contract.categoria.clone())
                }
            })
            .unwrap_or(origin_category);
        if !is_visible_regular_origin(&current_category) {
            continue;
        }

        let production_eligible = is_visible_production_origin(&current_category);
        let endurance_eligible = is_visible_endurance_origin(&current_category);
        if !production_eligible && !endurance_eligible {
            continue;
        }

        let (license_nivel, license_sigla) = license_badge(license_level.map(|value| value as u8));
        let ranking = rankings
            .get(&(driver_id.clone(), current_category.clone()))
            .copied();
        result.push(RankedEligibleCandidate {
            candidate: SpecialWindowEligibleCandidate {
                driver_id,
                driver_name,
                origin_category: current_category,
                license_nivel: license_nivel.to_string(),
                license_sigla: license_sigla.to_string(),
                desirability,
                production_eligible,
                endurance_eligible,
                championship_position: ranking.map(|value| value.0),
                championship_total_drivers: ranking.map(|value| value.1),
            },
            championship_position: ranking.map(|value| value.0),
            championship_total: ranking.map(|value| value.1),
        });
    }

    result.sort_by(|left, right| {
        left.candidate
            .origin_category
            .cmp(&right.candidate.origin_category)
            .then_with(
                || match (left.championship_position, right.championship_position) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                },
            )
            .then_with(
                || match (left.championship_total, right.championship_total) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    _ => std::cmp::Ordering::Equal,
                },
            )
            .then_with(|| {
                right
                    .candidate
                    .desirability
                    .cmp(&left.candidate.desirability)
            })
            .then_with(|| left.candidate.driver_name.cmp(&right.candidate.driver_name))
    });

    let mut kept_per_origin: HashMap<String, usize> = HashMap::new();
    let mut shortlisted = Vec::new();

    for entry in result {
        let current_count = kept_per_origin
            .entry(entry.candidate.origin_category.clone())
            .or_insert(0);
        if *current_count >= VISIBLE_SHORTLIST_LIMIT_PER_ORIGIN {
            continue;
        }
        *current_count += 1;
        shortlisted.push(entry.candidate);
    }

    Ok(shortlisted)
}

pub(super) fn load_player_offers(
    conn: &Connection,
    season_id: &str,
    player_id: &str,
    current_day: i32,
) -> Result<Vec<SpecialWindowPlayerOffer>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, team_id, team_name, special_category, class_name, papel, status, available_from_day
         FROM player_special_offers
         WHERE season_id = ?1 AND player_driver_id = ?2
           AND special_category NOT IN ('production_challenger', 'endurance')
           AND (
                available_from_day <= ?3
                OR status IN ('AceitaAtiva', 'Selecionado', 'PerdidaNoFechamento')
           )
         ORDER BY available_from_day ASC, team_name ASC",
    )?;
    let rows = stmt.query_map(params![season_id, player_id, current_day], |row| {
        Ok(SpecialWindowPlayerOffer {
            id: row.get(0)?,
            team_id: row.get(1)?,
            team_name: row.get(2)?,
            special_category: row.get(3)?,
            class_name: row.get(4)?,
            papel: row.get::<_, String>(5)?,
            status: row.get(6)?,
            available_from_day: row.get(7)?,
            is_available_today: row.get::<_, i32>(7)? <= current_day,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub(super) fn load_last_day_log(
    conn: &Connection,
    season_id: &str,
    current_day: i32,
) -> Result<Vec<SpecialWindowLogEntry>, DbError> {
    let log_day = current_day.saturating_sub(1);
    if log_day < 1 {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        "SELECT day_number, event_type, message, special_category, class_name, team_id, driver_id
         FROM special_window_daily_log
         WHERE season_id = ?1 AND day_number = ?2
         ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(params![season_id, log_day], |row| {
        Ok(SpecialWindowLogEntry {
            day: row.get(0)?,
            event_type: row.get(1)?,
            message: row.get(2)?,
            special_category: row.get(3)?,
            class_name: row.get(4)?,
            team_id: row.get(5)?,
            driver_id: row.get(6)?,
            team_name: None,
            driver_name: None,
            driver_origin_category: None,
            driver_license_nivel: None,
            driver_license_sigla: None,
            championship_position: None,
            championship_total_drivers: None,
        })
    })?;

    let rankings = build_visible_category_rankings(conn)?;
    let license_levels = load_license_levels(conn)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(enrich_log_entry(
            conn,
            season_id,
            row?,
            &rankings,
            &license_levels,
        )?);
    }
    Ok(result)
}

pub(super) fn enrich_log_entry(
    conn: &Connection,
    season_id: &str,
    mut entry: SpecialWindowLogEntry,
    rankings: &HashMap<(String, String), (i32, i32)>,
    license_levels: &HashMap<String, u8>,
) -> Result<SpecialWindowLogEntry, DbError> {
    if let Some(team_id) = entry.team_id.as_deref() {
        entry.team_name = Some(
            team_queries::get_team_by_id(conn, team_id)?
                .map(|team| team.nome)
                .unwrap_or_else(|| "Equipe especial".to_string()),
        );
    }

    let Some(driver_id) = entry.driver_id.clone() else {
        return Ok(entry);
    };

    let pool_row = conn
        .query_row(
            "SELECT driver_name, origin_category, license_level
             FROM special_window_candidate_pool
             WHERE season_id = ?1 AND driver_id = ?2
             LIMIT 1",
            params![season_id, driver_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            },
        )
        .optional()?;

    let driver = driver_queries::get_driver(conn, &driver_id)?;
    entry.driver_name = pool_row
        .as_ref()
        .map(|row| row.0.clone())
        .or_else(|| Some(driver.nome.clone()));

    let regular_contract =
        contract_queries::get_active_regular_contract_for_pilot(conn, &driver_id)?;
    let class_name = entry.class_name.clone();
    let origin_category = driver
        .categoria_atual
        .clone()
        .filter(|category| !category.is_empty())
        .or_else(|| {
            regular_contract.as_ref().and_then(|contract| {
                (!contract.categoria.is_empty()).then(|| contract.categoria.clone())
            })
        })
        .or_else(|| pool_row.as_ref().map(|row| row.1.clone()))
        .or_else(|| feeder_category_for_class(class_name.as_deref()).map(str::to_string));

    if let Some(origin_category) = origin_category {
        let ranking = rankings
            .get(&(driver_id.clone(), origin_category.clone()))
            .copied();
        entry.driver_origin_category = Some(origin_category);
        entry.championship_position = ranking
            .map(|value| value.0)
            .or_else(|| driver.melhor_resultado_temp.map(|value| value as i32));
        entry.championship_total_drivers = ranking.map(|value| value.1);
    }

    let license_level = pool_row
        .as_ref()
        .and_then(|row| row.2.map(|value| value as u8))
        .or_else(|| license_levels.get(&driver_id).copied());
    let (license_nivel, license_sigla) = license_badge(license_level);
    entry.driver_license_nivel = Some(license_nivel.to_string());
    entry.driver_license_sigla = Some(license_sigla.to_string());

    Ok(entry)
}

pub(super) fn load_visible_assignments(
    conn: &Connection,
    season_id: &str,
    current_day: i32,
) -> Result<Vec<VisibleAssignment>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT team_id, driver_id, papel, new_badge_day
         FROM special_window_assignments
         WHERE season_id = ?1 AND revealed = 1",
    )?;
    let rows = stmt.query_map(params![season_id], |row| {
        let new_badge_day = row.get::<_, Option<i32>>(3)?;
        Ok(VisibleAssignment {
            team_id: row.get(0)?,
            driver_id: row.get(1)?,
            papel: TeamRole::from_str_strict(&row.get::<_, String>(2)?)
                .map_err(rusqlite::Error::InvalidParameterName)?,
            new_badge_day: if new_badge_day == Some(current_day) {
                new_badge_day
            } else {
                None
            },
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}
