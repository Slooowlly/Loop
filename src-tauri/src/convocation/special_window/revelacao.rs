//! Revelação diária do mercado: expõe os assentos agendados, registra o log do
//! dia e resolve a seleção do jogador contra o titular.

use super::*;

pub(super) fn reveal_market_assignments(
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

pub(super) fn log_market_assignments_for_day(
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

pub(super) fn resolve_player_selection(
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
