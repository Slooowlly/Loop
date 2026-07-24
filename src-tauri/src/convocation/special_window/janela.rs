//! Entradas públicas da janela especial: inicialização, carga do payload e as
//! ações diárias do jogador (escolher oferta, avançar o dia).

use super::*;

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
               AND special_category NOT IN ('production_challenger', 'endurance')
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
