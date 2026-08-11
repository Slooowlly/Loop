//! Leitura do calendario: etapas por categoria, proxima corrida, pendencias por
//! janela de semana e contagens por status ou por fase da temporada.

use super::*;

pub fn get_calendar(
    conn: &Connection,
    season_id: &str,
    categoria: &str,
) -> Result<Vec<CalendarEntry>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1 AND categoria = ?2
         ORDER BY rodada ASC",
        colunas_select_calendar()
    ))?;
    let mapped = stmt.query_map(params![season_id, categoria], calendar_from_row)?;
    collect_entries(mapped)
}

pub fn get_next_race(
    conn: &Connection,
    season_id: &str,
    categoria: &str,
) -> Result<Option<CalendarEntry>, DbError> {
    // A ordenação é pela régua da temporada, que mora em `models::temporal`. A aritmética
    // solta que vivia aqui jogava as semanas de dezembro (woy 49–52) para 53–56, ou seja,
    // para o FIM da lista, quando elas são o começo da temporada.
    let sql = format!(
        "SELECT {colunas} FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND categoria = ?2
           AND status = 'Pendente'
         ORDER BY {regua} ASC, data ASC, rodada ASC
         LIMIT 1",
        colunas = colunas_select_calendar(),
        regua = crate::models::temporal::SQL_SEASON_WEEK_DERIVADA
    );
    let mut stmt = conn.prepare(&sql)?;
    let entry = stmt
        .query_row(params![season_id, categoria], calendar_from_row)
        .optional()?;
    Ok(entry)
}

pub fn get_calendar_entry_by_id(
    conn: &Connection,
    id: &str,
) -> Result<Option<CalendarEntry>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM calendar WHERE id = ?1",
        colunas_select_calendar()
    ))?;
    let entry = stmt.query_row(params![id], calendar_from_row).optional()?;
    Ok(entry)
}

pub fn get_pending_races(
    conn: &Connection,
    season_id: &str,
) -> Result<Vec<CalendarEntry>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND status = 'Pendente'
         ORDER BY categoria ASC, rodada ASC",
        colunas_select_calendar()
    ))?;
    let mapped = stmt.query_map(params![season_id], calendar_from_row)?;
    collect_entries(mapped)
}

pub fn get_pending_races_for_category(
    conn: &Connection,
    season_id: &str,
    category_id: &str,
) -> Result<Vec<CalendarEntry>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND categoria = ?2
           AND status = 'Pendente'
         ORDER BY rodada ASC",
        colunas_select_calendar()
    ))?;
    let mapped = stmt.query_map(params![season_id, category_id], calendar_from_row)?;
    collect_entries(mapped)
}

/// Retorna corridas pendentes de uma categoria com season_week até target_week,
/// ordenadas cronologicamente. Entradas com week_of_year = 0 (saves legados) são ignoradas.
pub fn get_pending_races_up_to_week(
    conn: &Connection,
    season_id: &str,
    category_id: &str,
    target_week: i32,
) -> Result<Vec<CalendarEntry>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND categoria = ?2
           AND status = 'Pendente'
           AND week_of_year > 0
           AND COALESCE(season_week, week_of_year + 4) <= ?3
         ORDER BY COALESCE(season_week, week_of_year + 4) ASC, rodada ASC",
        colunas_select_calendar()
    ))?;
    let mapped = stmt.query_map(
        params![season_id, category_id, target_week],
        calendar_from_row,
    )?;
    collect_entries(mapped)
}

pub fn count_races_by_status(
    conn: &Connection,
    season_id: &str,
    categoria: &str,
    status: &RaceStatus,
) -> Result<i32, DbError> {
    let count = conn.query_row(
        "SELECT COUNT(*) FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND categoria = ?2
           AND status = ?3",
        params![season_id, categoria, status.as_str()],
        |row| row.get(0),
    )?;
    Ok(count)
}

/// MAX(season_week) das corridas concluídas na temporada (todas as categorias).
/// None se nenhuma corrida foi concluída ainda.
pub fn get_current_effective_week(
    conn: &Connection,
    season_id: &str,
) -> Result<Option<i32>, DbError> {
    let result = conn.query_row(
        "SELECT MAX(COALESCE(season_week, week_of_year + 4)) FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND status = 'Concluida'
           AND week_of_year > 0",
        params![season_id],
        |row| row.get::<_, Option<i32>>(0),
    )?;
    Ok(result)
}

/// COUNT de corridas Pendente para a fase informada.
pub fn count_pending_races_in_phase(
    conn: &Connection,
    season_id: &str,
    phase: &SeasonPhase,
) -> Result<i32, DbError> {
    let count = conn.query_row(
        "SELECT COUNT(*) FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND status = 'Pendente'
           AND season_phase = ?2",
        params![season_id, phase.as_str()],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn get_next_any_race_in_phase(
    conn: &Connection,
    season_id: &str,
    phase: &SeasonPhase,
) -> Result<Option<CalendarEntry>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND season_phase = ?2
           AND status = 'Pendente'
         ORDER BY COALESCE(season_week, week_of_year + 4) ASC, data ASC
         LIMIT 1",
        colunas_select_calendar()
    ))?;
    let entry = stmt
        .query_row(params![season_id, phase.as_str()], calendar_from_row)
        .optional()?;
    Ok(entry)
}
