//! Resumo temporal da temporada: semana efetiva, data corrente exibida,
//! proximo evento do jogador e a contagem de pendencias da fase.

use super::*;

/// Monta SeasonTemporalSummary combinando as funções existentes.
/// current_phase é passado pelo chamador (já carregou a Season).
pub fn get_season_temporal_summary(
    conn: &Connection,
    season_id: &str,
    player_category: &str,
    current_phase: &SeasonPhase,
) -> Result<SeasonTemporalSummary, DbError> {
    let effective_week = if current_phase.is_legacy() {
        get_current_effective_week_legacy(conn, season_id)?
    } else {
        get_current_effective_week(conn, season_id)?
    };
    let next_player_event = if *current_phase == SeasonPhase::Encerramento {
        None
    } else {
        get_next_race(conn, season_id, player_category)?
    };
    let pending_in_phase = match current_phase {
        SeasonPhase::Temporada => count_pending_races_for_season(conn, season_id)?,
        SeasonPhase::Encerramento => 0,
        _ => count_pending_races_in_phase(conn, season_id, current_phase)?,
    };

    // Se não há mais eventos do jogador mas há corridas pendentes na fase,
    // buscamos o próximo evento genérico para que o calendário não trave.
    let next_any_event = if *current_phase != SeasonPhase::Encerramento
        && next_player_event.is_none()
        && pending_in_phase > 0
    {
        get_next_any_race_in_phase(conn, season_id, current_phase)?
    } else {
        None
    };

    let current_display_date = resolve_current_display_date(
        conn,
        season_id,
        effective_week,
        next_player_event.as_ref().or(next_any_event.as_ref()),
        current_phase,
    )?;

    let next_event_display_date = next_player_event
        .as_ref()
        .or(next_any_event.as_ref())
        .map(|entry| entry.display_date.clone())
        .filter(|value| !value.is_empty());

    let days_until_next_event = next_event_display_date
        .as_deref()
        .and_then(|next_date| days_between_display_dates(&current_display_date, next_date));
    Ok(SeasonTemporalSummary {
        fase: *current_phase, // SeasonPhase é Copy
        effective_week,
        current_display_date,
        next_player_event,
        next_event_display_date,
        days_until_next_event,
        pending_in_phase,
    })
}

pub(crate) fn count_pending_races_for_season(
    conn: &Connection,
    season_id: &str,
) -> Result<i32, DbError> {
    let count = conn.query_row(
        "SELECT COUNT(*) FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND status = 'Pendente'",
        params![season_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub(crate) fn resolve_current_display_date(
    conn: &Connection,
    season_id: &str,
    effective_season_week: Option<i32>,
    next_player_event: Option<&CalendarEntry>,
    current_phase: &SeasonPhase,
) -> Result<String, DbError> {
    if let Some(week) = effective_season_week {
        let date = if current_phase.is_legacy() {
            latest_completed_display_date_for_legacy_week(conn, season_id, week)?
        } else {
            latest_completed_display_date_for_season_week(conn, season_id, week)?
        };
        if let Some(date) = date {
            return Ok(date);
        }
    }

    let inferred_date = next_player_event.and_then(|entry| {
        if current_phase.is_legacy() {
            infer_pre_event_display_date_legacy(&entry.display_date)
        } else {
            infer_pre_event_display_date(entry)
        }
    });
    if let Some(date) = inferred_date {
        return Ok(date);
    }

    Ok(next_player_event
        .map(|entry| entry.display_date.clone())
        .unwrap_or_default())
}

pub(crate) fn get_current_effective_week_legacy(
    conn: &Connection,
    season_id: &str,
) -> Result<Option<i32>, DbError> {
    let result = conn.query_row(
        "SELECT MAX(week_of_year) FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND status = 'Concluida'
           AND week_of_year > 0",
        params![season_id],
        |row| row.get::<_, Option<i32>>(0),
    )?;
    Ok(result)
}

pub(crate) fn latest_completed_display_date_for_season_week(
    conn: &Connection,
    season_id: &str,
    season_week: i32,
) -> Result<Option<String>, DbError> {
    // A régua season_week ↔ week_of_year mora em `models::temporal` e vem daqui em SQL, para
    // não existir uma segunda aritmética escrita à mão dentro da query.
    let sql = format!(
        "SELECT data
         FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND status = 'Concluida'
           AND {} = ?2
         ORDER BY data DESC
         LIMIT 1",
        crate::models::temporal::SQL_SEASON_WEEK_DERIVADA
    );
    conn.query_row(&sql, params![season_id, season_week], |row| row.get(0))
        .optional()
        .map_err(Into::into)
}

pub(crate) fn latest_completed_display_date_for_legacy_week(
    conn: &Connection,
    season_id: &str,
    week_of_year: i32,
) -> Result<Option<String>, DbError> {
    conn.query_row(
        "SELECT data
         FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND status = 'Concluida'
           AND week_of_year = ?2
         ORDER BY data DESC
         LIMIT 1",
        params![season_id, week_of_year],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub(crate) fn infer_pre_event_display_date(entry: &CalendarEntry) -> Option<String> {
    let season_week = calendar_entry_season_week(entry);
    if !(2..=51).contains(&season_week) {
        return infer_pre_event_display_date_legacy(&entry.display_date);
    }
    let date = parse_display_date(&entry.display_date)?;
    let season_year = if season_week <= 4 {
        date.year() + 1
    } else {
        date.year()
    };
    display_date_for_season_week((season_week - 1) as u8, season_year, date.weekday()).ok()
}

pub(crate) fn infer_pre_event_display_date_legacy(display_date: &str) -> Option<String> {
    let date = parse_display_date(display_date)?;
    Some(
        date.checked_sub_signed(chrono::Duration::days(7))?
            .format("%Y-%m-%d")
            .to_string(),
    )
}

pub(crate) fn days_between_display_dates(from: &str, to: &str) -> Option<i32> {
    let from_date = parse_display_date(from)?;
    let to_date = parse_display_date(to)?;
    let days = (to_date - from_date).num_days();
    i32::try_from(days).ok()
}

pub(crate) fn parse_display_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}
