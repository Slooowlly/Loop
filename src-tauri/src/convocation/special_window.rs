use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};

use crate::commands::career_types::{
    SpecialWindowCategorySection, SpecialWindowEligibleCandidate, SpecialWindowLogEntry,
    SpecialWindowPayload, SpecialWindowPlayerOffer, SpecialWindowTeamSummary,
};
use crate::common::time::current_timestamp;
use crate::constants::categories::get_category_config;
use crate::convocation::eligibility::coletar_candidatos;
use crate::convocation::pipeline::GridClasse;
use crate::convocation::scoring::calcular_score;
use crate::db::connection::DbError;
use crate::db::queries::{
    contracts as contract_queries, drivers as driver_queries,
    special_team_entries as special_entry_queries, teams as team_queries,
};
use crate::models::driver::Driver;
use crate::models::enums::TeamRole;

pub const TOTAL_SPECIAL_WINDOW_DAYS: i32 = 7;

struct ClassConfig {
    special_category: &'static str,
    class_name: &'static str,
    feeder_category: &'static str,
}

const CLASSES_CONVOCADAS: &[ClassConfig] = &[
    ClassConfig {
        special_category: "production_challenger",
        class_name: "mazda",
        feeder_category: "mazda_amador",
    },
    ClassConfig {
        special_category: "production_challenger",
        class_name: "toyota",
        feeder_category: "toyota_amador",
    },
    ClassConfig {
        special_category: "production_challenger",
        class_name: "bmw",
        feeder_category: "bmw_m2",
    },
    ClassConfig {
        special_category: "endurance",
        class_name: "gt4",
        feeder_category: "gt4",
    },
    ClassConfig {
        special_category: "endurance",
        class_name: "gt3",
        feeder_category: "gt3",
    },
    ClassConfig {
        special_category: "endurance",
        class_name: "lmp2",
        feeder_category: "lmp2",
    },
];

#[derive(Debug, Clone)]
struct WindowStateRow {
    current_day: i32,
    total_days: i32,
    status: String,
    active_offer_id: Option<String>,
    player_result: Option<String>,
}

#[derive(Debug, Clone)]
struct CandidateAccumulator {
    driver_name: String,
    origin_category: String,
    license_level: Option<u8>,
    desirability: i32,
    production_eligible: bool,
    endurance_eligible: bool,
}

#[derive(Debug, Clone)]
struct VisibleAssignment {
    team_id: String,
    driver_id: String,
    papel: TeamRole,
    new_badge_day: Option<i32>,
}

#[derive(Debug, Clone)]
struct RankedEligibleCandidate {
    candidate: SpecialWindowEligibleCandidate,
    championship_position: Option<i32>,
    championship_total: Option<i32>,
}

const VISIBLE_PRODUCTION_ORIGINS: &[&str] = &["mazda_amador", "toyota_amador", "bmw_m2"];
const VISIBLE_ENDURANCE_ORIGINS: &[&str] = &["gt4", "gt3", "lmp2"];
const VISIBLE_SHORTLIST_LIMIT_PER_ORIGIN: usize = 12;

pub fn initialize_special_window(
    conn: &Connection,
    season_id: &str,
    player: Option<&Driver>,
    grids: &[GridClasse],
) -> Result<(), DbError> {
    conn.execute(
        "DELETE FROM special_window_state WHERE season_id = ?1",
        params![season_id],
    )?;
    conn.execute(
        "DELETE FROM special_window_assignments WHERE season_id = ?1",
        params![season_id],
    )?;
    conn.execute(
        "DELETE FROM special_window_candidate_pool WHERE season_id = ?1",
        params![season_id],
    )?;
    conn.execute(
        "DELETE FROM special_window_daily_log WHERE season_id = ?1",
        params![season_id],
    )?;

    conn.execute(
        "INSERT INTO special_window_state (
            season_id, current_day, total_days, status, active_offer_id, player_result, created_at, updated_at
        ) VALUES (?1, 1, ?2, 'Aberta', NULL, NULL, ?3, ?3)",
        params![season_id, TOTAL_SPECIAL_WINDOW_DAYS, current_timestamp()],
    )?;

    seed_candidate_pool(conn, season_id)?;
    seed_assignment_schedule(conn, season_id, grids)?;

    if let Some(player) = player {
        schedule_player_offer_days(conn, season_id, player)?;
    }

    // O primeiro dia da janela ja precisa nascer com parte do grid visivel.
    reveal_market_assignments(conn, season_id, 1, false, TOTAL_SPECIAL_WINDOW_DAYS)?;

    Ok(())
}

pub fn load_special_window_payload(
    conn: &Connection,
    season_id: &str,
    player_id: &str,
) -> Result<SpecialWindowPayload, DbError> {
    let state = get_window_state(conn, season_id)?.ok_or_else(|| {
        DbError::NotFound(format!(
            "Janela especial nao inicializada para temporada '{season_id}'"
        ))
    })?;

    let team_sections = load_visible_team_sections(conn, season_id, state.current_day)?;
    let eligible_candidates = load_eligible_candidates(conn, season_id)?;
    let player_offers = load_player_offers(conn, season_id, player_id, state.current_day)?;
    let last_day_log = load_last_day_log(conn, season_id, state.current_day)?;

    Ok(SpecialWindowPayload {
        current_day: state.current_day,
        total_days: state.total_days,
        status: state.status.clone(),
        active_offer_id: state.active_offer_id.clone(),
        player_result: state.player_result.clone(),
        team_sections,
        eligible_candidates,
        player_offers,
        last_day_log,
        can_advance_day: state.status != "Resolvida",
        can_confirm_special_block: state.status == "Resolvida",
        is_finished: state.status == "Resolvida",
    })
}

pub fn select_player_offer_for_day(
    conn: &Connection,
    season_id: &str,
    player_id: &str,
    offer_id: &str,
) -> Result<SpecialWindowPayload, DbError> {
    let state = get_window_state(conn, season_id)?.ok_or_else(|| {
        DbError::NotFound(format!(
            "Janela especial nao inicializada para temporada '{season_id}'"
        ))
    })?;
    if state.status == "Resolvida" {
        return Err(DbError::InvalidData(
            "A janela especial ja foi resolvida.".to_string(),
        ));
    }

    let offer = conn
        .query_row(
            "SELECT id, status, available_from_day
             FROM player_special_offers
             WHERE season_id = ?1 AND player_driver_id = ?2 AND id = ?3
             LIMIT 1",
            params![season_id, player_id, offer_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i32>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| DbError::NotFound("Oferta especial nao encontrada.".to_string()))?;

    if offer.1 != "Pendente" && offer.1 != "AceitaAtiva" {
        return Err(DbError::InvalidData(
            "A oferta especial nao esta disponivel para escolha diaria.".to_string(),
        ));
    }
    if offer.2 > state.current_day {
        return Err(DbError::InvalidData(
            "A oferta especial ainda nao ficou disponivel neste dia.".to_string(),
        ));
    }

    conn.execute(
        "UPDATE player_special_offers
         SET selected_for_day = 0,
             status = CASE
                 WHEN status = 'AceitaAtiva' THEN 'Pendente'
                 ELSE status
             END
         WHERE season_id = ?1 AND player_driver_id = ?2
           AND status IN ('Pendente', 'AceitaAtiva')",
        params![season_id, player_id],
    )?;
    conn.execute(
        "UPDATE player_special_offers
         SET selected_for_day = 1, status = 'AceitaAtiva'
         WHERE season_id = ?1 AND player_driver_id = ?2 AND id = ?3",
        params![season_id, player_id, offer_id],
    )?;
    conn.execute(
        "UPDATE special_window_state
         SET active_offer_id = ?2, updated_at = ?3
         WHERE season_id = ?1",
        params![season_id, offer_id, current_timestamp()],
    )?;

    load_special_window_payload(conn, season_id, player_id)
}

pub fn advance_special_window_day(
    conn: &Connection,
    season_id: &str,
    player_id: &str,
) -> Result<SpecialWindowPayload, DbError> {
    let state = get_window_state(conn, season_id)?.ok_or_else(|| {
        DbError::NotFound(format!(
            "Janela especial nao inicializada para temporada '{season_id}'"
        ))
    })?;
    if state.status == "Resolvida" {
        return load_special_window_payload(conn, season_id, player_id);
    }

    conn.execute(
        "DELETE FROM special_window_daily_log WHERE season_id = ?1 AND day_number = ?2",
        params![season_id, state.current_day],
    )?;

    resolve_player_selection(conn, season_id, player_id, state.current_day)?;
    reveal_market_assignments(conn, season_id, state.current_day, true, state.total_days)?;
    log_market_assignments_for_day(conn, season_id, state.current_day)?;

    let (next_day, next_status) = if state.current_day >= state.total_days {
        (state.total_days, "Resolvida".to_string())
    } else {
        (state.current_day + 1, "Aberta".to_string())
    };

    conn.execute(
        "UPDATE special_window_state
         SET current_day = ?2, status = ?3, updated_at = ?4
         WHERE season_id = ?1",
        params![season_id, next_day, next_status, current_timestamp()],
    )?;

    load_special_window_payload(conn, season_id, player_id)
}

fn seed_candidate_pool(conn: &Connection, season_id: &str) -> Result<(), DbError> {
    let license_levels = load_license_levels(conn)?;
    let mut drivers: HashMap<String, CandidateAccumulator> = HashMap::new();

    for cfg in CLASSES_CONVOCADAS {
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

fn seed_assignment_schedule(
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
            let special_category = CLASSES_CONVOCADAS
                .iter()
                .find(|cfg| cfg.class_name == grid.class_name)
                .map(|cfg| cfg.special_category)
                .unwrap_or(team.categoria.as_str());
            let reveal_day = schedule_reveal_day(index, total, team.car_performance, &team.id);
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

fn schedule_player_offer_days(
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

fn reveal_market_assignments(
    conn: &Connection,
    season_id: &str,
    day: i32,
    mark_as_new: bool,
    total_days: i32,
) -> Result<(), DbError> {
    let mut stmt = conn.prepare(
        "SELECT swa.id, swa.special_category, swa.class_name, swa.team_id, swa.driver_id
         FROM special_window_assignments swa
         WHERE swa.season_id = ?1 AND swa.revealed = 0 AND swa.reveal_day = ?2
         ORDER BY swa.special_category, swa.class_name, swa.team_id",
    )?;
    let rows = stmt.query_map(params![season_id, day], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    let mut revealed = Vec::new();
    for row in rows {
        revealed.push(row?);
    }

    for (assignment_id, _special_category, _class_name, _team_id, driver_id) in revealed {
        let new_badge_day = if mark_as_new {
            Some(display_day_for_reveal(day, total_days))
        } else {
            None
        };
        conn.execute(
            "UPDATE special_window_assignments
             SET revealed = 1, new_badge_day = ?2
             WHERE id = ?1",
            params![assignment_id, new_badge_day],
        )?;
        conn.execute(
            "UPDATE special_window_candidate_pool
             SET status = 'Convocado'
             WHERE season_id = ?1 AND driver_id = ?2",
            params![season_id, driver_id],
        )?;
    }

    Ok(())
}

fn log_market_assignments_for_day(
    conn: &Connection,
    season_id: &str,
    day: i32,
) -> Result<(), DbError> {
    let mut stmt = conn.prepare(
        "SELECT swa.special_category, swa.class_name, swa.team_id, swa.driver_id
         FROM special_window_assignments swa
         WHERE swa.season_id = ?1
           AND swa.reveal_day = ?2
           AND swa.revealed = 1
         ORDER BY swa.special_category, swa.class_name, swa.team_id, swa.papel",
    )?;
    let rows = stmt.query_map(params![season_id, day], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;

    let mut logged = Vec::new();
    for row in rows {
        logged.push(row?);
    }

    for (special_category, class_name, team_id, driver_id) in logged {
        let driver_name = driver_queries::get_driver(conn, &driver_id)?.nome;
        let team_name = team_queries::get_team_by_id(conn, &team_id)?
            .map(|team| team.nome)
            .unwrap_or_else(|| "Equipe especial".to_string());

        insert_log(
            conn,
            season_id,
            day,
            "convocado",
            &format!("{driver_name} foi convocado para {team_name}."),
            Some(&special_category),
            Some(&class_name),
            Some(&team_id),
            Some(&driver_id),
        )?;
    }

    Ok(())
}

fn resolve_player_selection(
    conn: &Connection,
    season_id: &str,
    player_id: &str,
    day: i32,
) -> Result<(), DbError> {
    let state = get_window_state(conn, season_id)?.ok_or_else(|| {
        DbError::NotFound(format!(
            "Janela especial nao inicializada para temporada '{season_id}'"
        ))
    })?;
    if matches!(state.player_result.as_deref(), Some("selected")) {
        return Ok(());
    }

    let active_offer = conn
        .query_row(
            "SELECT id, team_id, class_name, special_category, papel
             FROM player_special_offers
             WHERE season_id = ?1 AND player_driver_id = ?2
               AND selected_for_day = 1
               AND status = 'AceitaAtiva'
               AND available_from_day <= ?3
             LIMIT 1",
            params![season_id, player_id, day],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?;

    let Some((offer_id, team_id, class_name, special_category, papel)) = active_offer else {
        return Ok(());
    };
    let selected_badge_day = display_day_for_reveal(day, state.total_days);

    let incumbent = conn
        .query_row(
            "SELECT driver_id
             FROM special_window_assignments
             WHERE season_id = ?1 AND team_id = ?2 AND papel = ?3
             LIMIT 1",
            params![season_id, team_id, papel],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    let player_desirability = conn
        .query_row(
            "SELECT desirability FROM special_window_candidate_pool
             WHERE season_id = ?1 AND driver_id = ?2
             LIMIT 1",
            params![season_id, player_id],
            |row| row.get::<_, i32>(0),
        )
        .optional()?
        .unwrap_or(70);
    let incumbent_desirability = incumbent
        .as_deref()
        .and_then(|driver_id| {
            conn.query_row(
                "SELECT desirability FROM special_window_candidate_pool
                 WHERE season_id = ?1 AND driver_id = ?2
                 LIMIT 1",
                params![season_id, driver_id],
                |row| row.get::<_, i32>(0),
            )
            .optional()
            .ok()
            .flatten()
        })
        .unwrap_or(0);

    let Some(team) = team_queries::get_team_by_id(conn, &team_id)? else {
        return Ok(());
    };
    let profile_bonus = market_profile_modifier(&team.id);
    let player_wins = player_desirability + profile_bonus >= incumbent_desirability - 6;

    if player_wins {
        conn.execute(
            "UPDATE player_special_offers
             SET status = 'Selecionado', selected_for_day = 0, resolved_day = ?3
             WHERE season_id = ?1 AND player_driver_id = ?2 AND id = ?4",
            params![season_id, player_id, day, offer_id],
        )?;
        conn.execute(
            "UPDATE special_window_state
             SET player_result = 'selected', active_offer_id = ?2, updated_at = ?3
             WHERE season_id = ?1",
            params![season_id, offer_id, current_timestamp()],
        )?;
        conn.execute(
            "UPDATE special_window_assignments
             SET driver_id = ?4, is_player = 1, revealed = 1, new_badge_day = ?5
             WHERE season_id = ?1 AND team_id = ?2 AND papel = ?3",
            params![season_id, team_id, papel, player_id, selected_badge_day],
        )?;
        conn.execute(
            "UPDATE special_window_candidate_pool
             SET status = 'Convocado'
             WHERE season_id = ?1 AND driver_id = ?2",
            params![season_id, player_id],
        )?;
        if let Some(incumbent_id) = incumbent {
            conn.execute(
                "UPDATE special_window_candidate_pool
                 SET status = 'Livre'
                 WHERE season_id = ?1 AND driver_id = ?2",
                params![season_id, incumbent_id],
            )?;
        }
        let player_name = driver_queries::get_driver(conn, player_id)?.nome;
        insert_log(
            conn,
            season_id,
            day,
            "player_selected",
            &format!(
                "{player_name} convenceu {team_name} e garantiu a convocacao especial.",
                team_name = team.nome
            ),
            Some(&special_category),
            Some(&class_name),
            Some(&team_id),
            Some(player_id),
        )?;
    } else {
        conn.execute(
            "UPDATE player_special_offers
             SET status = 'PerdidaNoFechamento', selected_for_day = 0, resolved_day = ?3
             WHERE season_id = ?1 AND player_driver_id = ?2 AND id = ?4",
            params![season_id, player_id, day, offer_id],
        )?;
        conn.execute(
            "UPDATE special_window_state
             SET active_offer_id = NULL, updated_at = ?2
             WHERE season_id = ?1",
            params![season_id, current_timestamp()],
        )?;
        let player_name = driver_queries::get_driver(conn, player_id)?.nome;
        insert_log(
            conn,
            season_id,
            day,
            "player_missed",
            &format!(
                "{player_name} nao foi o escolhido final de {team_name}.",
                team_name = team.nome
            ),
            Some(&special_category),
            Some(&class_name),
            Some(&team_id),
            Some(player_id),
        )?;
    }

    Ok(())
}

fn get_window_state(conn: &Connection, season_id: &str) -> Result<Option<WindowStateRow>, DbError> {
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

fn load_visible_team_sections(
    conn: &Connection,
    season_id: &str,
    current_day: i32,
) -> Result<Vec<SpecialWindowCategorySection>, DbError> {
    let visible = load_visible_assignments(conn, season_id, current_day)?;
    let categories = ["production_challenger", "endurance"];
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

fn load_eligible_candidates(
    conn: &Connection,
    season_id: &str,
) -> Result<Vec<SpecialWindowEligibleCandidate>, DbError> {
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

fn load_player_offers(
    conn: &Connection,
    season_id: &str,
    player_id: &str,
    current_day: i32,
) -> Result<Vec<SpecialWindowPlayerOffer>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, team_id, team_name, special_category, class_name, papel, status, available_from_day
         FROM player_special_offers
         WHERE season_id = ?1 AND player_driver_id = ?2
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

fn load_last_day_log(
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

fn enrich_log_entry(
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

fn feeder_category_for_class(class_name: Option<&str>) -> Option<&'static str> {
    let class_name = class_name?;
    CLASSES_CONVOCADAS
        .iter()
        .find(|cfg| cfg.class_name == class_name)
        .map(|cfg| cfg.feeder_category)
}

fn load_visible_assignments(
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

fn display_day_for_reveal(day: i32, total_days: i32) -> i32 {
    if day >= total_days {
        total_days
    } else {
        day + 1
    }
}

fn is_visible_regular_origin(category: &str) -> bool {
    is_visible_production_origin(category) || is_visible_endurance_origin(category)
}

fn is_visible_production_origin(category: &str) -> bool {
    VISIBLE_PRODUCTION_ORIGINS.contains(&category)
}

fn is_visible_endurance_origin(category: &str) -> bool {
    VISIBLE_ENDURANCE_ORIGINS.contains(&category)
}

fn build_visible_category_rankings(
    conn: &Connection,
) -> Result<HashMap<(String, String), (i32, i32)>, DbError> {
    let mut rankings = HashMap::new();
    let visible_categories = VISIBLE_PRODUCTION_ORIGINS
        .iter()
        .chain(VISIBLE_ENDURANCE_ORIGINS.iter());

    for category in visible_categories {
        let mut drivers = driver_queries::get_drivers_by_category(conn, category)?
            .into_iter()
            .filter(|driver| driver.status == crate::models::enums::DriverStatus::Ativo)
            .collect::<Vec<_>>();
        drivers.sort_by(|left, right| {
            right
                .stats_temporada
                .pontos
                .total_cmp(&left.stats_temporada.pontos)
                .then_with(|| {
                    right
                        .stats_temporada
                        .vitorias
                        .cmp(&left.stats_temporada.vitorias)
                })
                .then_with(|| {
                    right
                        .stats_temporada
                        .podios
                        .cmp(&left.stats_temporada.podios)
                })
                .then_with(|| {
                    left.stats_temporada
                        .posicao_media
                        .total_cmp(&right.stats_temporada.posicao_media)
                })
                .then_with(|| left.nome.cmp(&right.nome))
        });

        let total = drivers.len() as i32;
        for (index, driver) in drivers.iter().enumerate() {
            rankings.insert(
                (driver.id.clone(), (*category).to_string()),
                (index as i32 + 1, total),
            );
        }
    }

    Ok(rankings)
}

fn insert_log(
    conn: &Connection,
    season_id: &str,
    day: i32,
    event_type: &str,
    message: &str,
    special_category: Option<&str>,
    class_name: Option<&str>,
    team_id: Option<&str>,
    driver_id: Option<&str>,
) -> Result<(), DbError> {
    let team_part = team_id.unwrap_or("sem-equipe");
    let driver_part = driver_id.unwrap_or("sem-piloto");
    conn.execute(
        "INSERT INTO special_window_daily_log (
            id, season_id, day_number, event_type, message, special_category,
            class_name, team_id, driver_id, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            format!("SWL-{season_id}-{day}-{event_type}-{team_part}-{driver_part}"),
            season_id,
            day,
            event_type,
            message,
            special_category,
            class_name,
            team_id,
            driver_id,
            current_timestamp(),
        ],
    )?;
    Ok(())
}

fn schedule_reveal_day(rank_index: usize, total: usize, team_strength: f64, team_id: &str) -> i32 {
    let base_day = if total <= 1 {
        1
    } else {
        1 + ((rank_index * (TOTAL_SPECIAL_WINDOW_DAYS as usize - 1)) / (total - 1)) as i32
    };
    let strength_modifier = if team_strength >= 12.0 {
        -1
    } else if team_strength <= 4.0 {
        1
    } else {
        0
    };
    let profile_modifier = market_profile_modifier(team_id);
    (base_day + strength_modifier + profile_modifier).clamp(1, TOTAL_SPECIAL_WINDOW_DAYS)
}

fn market_profile_modifier(team_id: &str) -> i32 {
    match team_id.bytes().fold(0_u32, |acc, value| acc + value as u32) % 4 {
        0 => -1,
        1 => 1,
        2 => 1,
        _ => 0,
    }
}

fn derive_player_desirability(player: &Driver) -> i32 {
    let champion_bonus = if player.melhor_resultado_temp == Some(1) {
        8
    } else {
        0
    };
    let wins_bonus = (player.stats_temporada.vitorias as i32).min(5) * 2;
    (player.atributos.skill.round() as i32 + champion_bonus + wins_bonus).clamp(50, 99)
}

fn license_badge(level: Option<u8>) -> (&'static str, &'static str) {
    match level {
        Some(0) => ("Rookie", "R"),
        Some(1) => ("Amador", "A"),
        Some(2) => ("Pro", "P"),
        Some(3) => ("Super Pro", "SP"),
        Some(4) => ("Elite", "E"),
        Some(_) => ("Super Elite", "SE"),
        None => ("Rookie", "R"),
    }
}

fn load_license_levels(conn: &Connection) -> Result<HashMap<String, u8>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT piloto_id, MAX(CAST(nivel AS INTEGER)) AS max_nivel
         FROM licenses
         GROUP BY piloto_id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u8))
    })?;

    let mut map = HashMap::new();
    for row in rows {
        let (piloto_id, nivel) = row?;
        map.insert(piloto_id, nivel);
    }
    Ok(map)
}
