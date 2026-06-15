#![allow(dead_code)]

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};

use crate::calendar::{
    calendar_week_for_round, display_date_for_category_round, display_date_for_season_week,
    CalendarEntry,
};
use crate::db::connection::DbError;
use crate::models::enums::{RaceStatus, SeasonPhase, ThematicSlot, WeatherCondition};
use crate::models::temporal::SeasonTemporalSummary;

pub fn insert_calendar_entry(conn: &Connection, entry: &CalendarEntry) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO calendar (
            id, temporada_id, season_id, categoria, rodada, nome,
            pista, track_id, track_name, track_config, clima, temperatura,
            voltas, duracao, duracao_corrida_min, duracao_classificacao_min,
            status, horario, data, week_of_year, season_phase, thematic_slot, season_week
        ) VALUES (
            :id, :temporada_id, :season_id, :categoria, :rodada, :nome,
            :pista, :track_id, :track_name, :track_config, :clima, :temperatura,
            :voltas, :duracao, :duracao_corrida_min, :duracao_classificacao_min,
            :status, :horario, :data, :week_of_year, :season_phase, :thematic_slot, :season_week
        )",
        rusqlite::named_params! {
            ":id": &entry.id,
            ":temporada_id": &entry.season_id,
            ":season_id": &entry.season_id,
            ":categoria": &entry.categoria,
            ":rodada": entry.rodada,
            ":nome": &entry.nome,
            ":pista": &entry.track_name,
            ":track_id": entry.track_id as i64,
            ":track_name": &entry.track_name,
            ":track_config": &entry.track_config,
            ":clima": entry.clima.as_str(),
            ":temperatura": entry.temperatura,
            ":voltas": entry.voltas,
            ":duracao": entry.duracao_corrida_min,
            ":duracao_corrida_min": entry.duracao_corrida_min,
            ":duracao_classificacao_min": entry.duracao_classificacao_min,
            ":status": entry.status.as_str(),
            ":horario": &entry.horario,
            ":data": &entry.display_date,
            ":week_of_year": entry.week_of_year,
            ":season_phase": entry.season_phase.as_str(),
            ":thematic_slot": entry.thematic_slot.as_str(),
            ":season_week": entry.season_week.map(|v| v as i64),
        },
    )?;
    Ok(())
}

pub fn insert_calendar_entries(
    conn: &Connection,
    entries: &[CalendarEntry],
) -> Result<(), DbError> {
    for entry in entries {
        insert_calendar_entry(conn, entry)?;
    }
    Ok(())
}

pub fn get_calendar(
    conn: &Connection,
    season_id: &str,
    categoria: &str,
) -> Result<Vec<CalendarEntry>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1 AND categoria = ?2
         ORDER BY rodada ASC",
    )?;
    let mapped = stmt.query_map(params![season_id, categoria], calendar_from_row)?;
    collect_entries(mapped)
}

pub(crate) fn calendar_entry_season_week(entry: &CalendarEntry) -> i32 {
    entry
        .season_week
        .map(|week| week as i32)
        .unwrap_or(entry.week_of_year + 4)
}

pub fn normalize_calendar_display_dates_for_weekday_policy(
    conn: &Connection,
    season_id: &str,
    season_year: i32,
) -> Result<usize, DbError> {
    let rows = {
        let mut stmt = conn.prepare(
            "SELECT id, categoria, rodada, week_of_year, season_phase, data
             FROM calendar
             WHERE COALESCE(season_id, temporada_id) = ?1
               AND week_of_year BETWEEN 1 AND 52",
        )?;

        let mapped = stmt.query_map(params![season_id], |row| {
            Ok((
                row.get::<_, String>("id")?,
                row.get::<_, String>("categoria")?,
                row.get::<_, i32>("rodada")?,
                row.get::<_, i32>("week_of_year")?,
                optional_string(row, "season_phase")?.unwrap_or_else(|| "BlocoRegular".to_string()),
                optional_string(row, "data")?,
            ))
        })?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };

    let special_totals = rows
        .iter()
        .fold(HashMap::<String, i32>::new(), |mut acc, row| {
            let (_, categoria, rodada, _, season_phase, _) = row;
            if season_phase == "BlocoEspecial" {
                acc.entry(categoria.clone())
                    .and_modify(|total| *total = (*total).max(*rodada))
                    .or_insert(*rodada);
            }
            acc
        });

    let mut updated = 0;
    for (id, categoria, rodada, week_of_year, season_phase, current_date) in rows {
        let season_phase = SeasonPhase::from_str_strict(&season_phase)
            .map_err(|err| DbError::InvalidData(err.to_string()))?;
        let expected_week = if season_phase == SeasonPhase::BlocoEspecial {
            calendar_week_for_round(
                season_year,
                season_phase,
                rodada,
                *special_totals.get(&categoria).unwrap_or(&rodada),
            )
        } else {
            week_of_year
        };
        let expected_date = display_date_for_category_round(
            season_year,
            expected_week,
            &categoria,
            season_phase,
            rodada,
        );

        if week_of_year == expected_week && current_date.as_deref() == Some(expected_date.as_str())
        {
            continue;
        }

        conn.execute(
            "UPDATE calendar SET week_of_year = ?1, data = ?2 WHERE id = ?3",
            params![expected_week, expected_date, id],
        )?;
        updated += 1;
    }

    Ok(updated)
}

pub fn get_next_race(
    conn: &Connection,
    season_id: &str,
    categoria: &str,
) -> Result<Option<CalendarEntry>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND categoria = ?2
           AND status = 'Pendente'
         ORDER BY COALESCE(season_week, week_of_year + 4) ASC, data ASC, rodada ASC
         LIMIT 1",
    )?;
    let entry = stmt
        .query_row(params![season_id, categoria], calendar_from_row)
        .optional()?;
    Ok(entry)
}

pub fn get_calendar_entry_by_id(
    conn: &Connection,
    id: &str,
) -> Result<Option<CalendarEntry>, DbError> {
    let mut stmt = conn.prepare("SELECT * FROM calendar WHERE id = ?1")?;
    let entry = stmt.query_row(params![id], calendar_from_row).optional()?;
    Ok(entry)
}

pub fn mark_race_completed(conn: &Connection, id: &str) -> Result<(), DbError> {
    conn.execute(
        "UPDATE calendar SET status = 'Concluida' WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn get_pending_races(
    conn: &Connection,
    season_id: &str,
) -> Result<Vec<CalendarEntry>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND status = 'Pendente'
         ORDER BY categoria ASC, rodada ASC",
    )?;
    let mapped = stmt.query_map(params![season_id], calendar_from_row)?;
    collect_entries(mapped)
}

pub fn get_pending_races_for_category(
    conn: &Connection,
    season_id: &str,
    category_id: &str,
) -> Result<Vec<CalendarEntry>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND categoria = ?2
           AND status = 'Pendente'
         ORDER BY rodada ASC",
    )?;
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
    let mut stmt = conn.prepare(
        "SELECT * FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND categoria = ?2
           AND status = 'Pendente'
           AND week_of_year > 0
           AND COALESCE(season_week, week_of_year + 4) <= ?3
         ORDER BY COALESCE(season_week, week_of_year + 4) ASC, rodada ASC",
    )?;
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
    let mut stmt = conn.prepare(
        "SELECT * FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND season_phase = ?2
           AND status = 'Pendente'
         ORDER BY COALESCE(season_week, week_of_year + 4) ASC, data ASC
         LIMIT 1",
    )?;
    let entry = stmt
        .query_row(params![season_id, phase.as_str()], calendar_from_row)
        .optional()?;
    Ok(entry)
}

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

fn count_pending_races_for_season(conn: &Connection, season_id: &str) -> Result<i32, DbError> {
    let count = conn.query_row(
        "SELECT COUNT(*) FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND status = 'Pendente'",
        params![season_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn delete_calendar_for_season(conn: &Connection, season_id: &str) -> Result<(), DbError> {
    conn.execute(
        "DELETE FROM calendar WHERE COALESCE(season_id, temporada_id) = ?1",
        params![season_id],
    )?;
    Ok(())
}

fn collect_entries<T>(mapped: T) -> Result<Vec<CalendarEntry>, DbError>
where
    T: IntoIterator<Item = rusqlite::Result<CalendarEntry>>,
{
    let mut entries = Vec::new();
    for row in mapped {
        entries.push(row?);
    }
    Ok(entries)
}

fn calendar_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CalendarEntry> {
    Ok(CalendarEntry {
        id: row.get("id")?,
        season_id: optional_string(row, "season_id")?
            .or_else(|| optional_string(row, "temporada_id").ok().flatten())
            .unwrap_or_default(),
        categoria: row.get("categoria")?,
        rodada: row.get("rodada")?,
        nome: optional_string(row, "nome")?.unwrap_or_else(|| {
            let pista = optional_string(row, "track_name")
                .ok()
                .flatten()
                .or_else(|| optional_string(row, "pista").ok().flatten())
                .unwrap_or_default();
            format!(
                "Rodada {} - {}",
                row.get::<_, i32>("rodada").unwrap_or(0),
                pista
            )
        }),
        track_id: parse_non_negative_u32(row, "track_id", 0)?,
        track_name: optional_string(row, "track_name")?
            .or_else(|| optional_string(row, "pista").ok().flatten())
            .unwrap_or_default(),
        track_config: optional_string(row, "track_config")?.unwrap_or_default(),
        clima: WeatherCondition::from_str_strict(&row.get::<_, String>("clima")?)
            .map_err(rusqlite::Error::InvalidParameterName)?,
        temperatura: optional_f64(row, "temperatura")?.unwrap_or(25.0),
        voltas: parse_non_negative_i32(row, "voltas", 10)?,
        duracao_corrida_min: optional_i64(row, "duracao_corrida_min")?
            .or_else(|| optional_i64(row, "duracao").ok().flatten())
            .map(|value| parse_non_negative_i32_value("duracao_corrida_min", value))
            .transpose()?
            .unwrap_or(60),
        duracao_classificacao_min: parse_non_negative_i32(row, "duracao_classificacao_min", 15)?,
        status: RaceStatus::from_str_strict(
            &optional_string(row, "status")?.unwrap_or_else(|| "Pendente".to_string()),
        )
        .map_err(rusqlite::Error::InvalidParameterName)?,
        horario: optional_string(row, "horario")?.unwrap_or_else(|| "14:00".to_string()),
        week_of_year: parse_non_negative_i32(row, "week_of_year", 0)?,
        season_phase: match optional_string(row, "season_phase")? {
            None => SeasonPhase::BlocoRegular,
            Some(s) => {
                SeasonPhase::from_str_strict(&s).map_err(rusqlite::Error::InvalidParameterName)?
            }
        },
        display_date: optional_string(row, "data")?.unwrap_or_default(),
        thematic_slot: match optional_string(row, "thematic_slot")? {
            // NULL no banco (saves pré-v12) → NaoClassificado
            None => ThematicSlot::NaoClassificado,
            // string presente: parse estrito — string inválida é erro, não fallback silencioso
            Some(s) => {
                ThematicSlot::from_str_strict(&s).map_err(rusqlite::Error::InvalidParameterName)?
            }
        },
        // Coluna adicionada em v33; None para saves anteriores ou entradas pré-backfill.
        // unwrap_or(None) absorve InvalidColumnName quando a coluna ainda não existe
        // (ex.: testes que criam schema mínimo sem rodar run_all).
        season_week: row
            .get::<_, Option<i64>>("season_week")
            .unwrap_or(None)
            .and_then(|v| if v > 0 { Some(v as u32) } else { None }),
    })
}

fn invalid_calendar_data_error(message: impl Into<String>) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(message.into())
}

fn parse_non_negative_u32(
    row: &rusqlite::Row<'_>,
    column_name: &str,
    default: u32,
) -> rusqlite::Result<u32> {
    optional_i64(row, column_name)?
        .map(|value| {
            u32::try_from(value).map_err(|_| {
                invalid_calendar_data_error(format!(
                    "Campo '{column_name}' invalido: esperado inteiro nao negativo, recebido {value}"
                ))
            })
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn parse_non_negative_i32(
    row: &rusqlite::Row<'_>,
    column_name: &str,
    default: i32,
) -> rusqlite::Result<i32> {
    optional_i64(row, column_name)?
        .map(|value| parse_non_negative_i32_value(column_name, value))
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn parse_non_negative_i32_value(column_name: &str, value: i64) -> rusqlite::Result<i32> {
    let parsed = i32::try_from(value).map_err(|_| {
        invalid_calendar_data_error(format!(
            "Campo '{column_name}' invalido: esperado inteiro nao negativo, recebido {value}"
        ))
    })?;
    if parsed < 0 {
        return Err(invalid_calendar_data_error(format!(
            "Campo '{column_name}' invalido: esperado inteiro nao negativo, recebido {value}"
        )));
    }
    Ok(parsed)
}

fn optional_string(row: &rusqlite::Row<'_>, column_name: &str) -> rusqlite::Result<Option<String>> {
    match row.get_ref(column_name)? {
        rusqlite::types::ValueRef::Null => Ok(None),
        _ => row.get(column_name).map(Some),
    }
}

fn optional_i64(row: &rusqlite::Row<'_>, column_name: &str) -> rusqlite::Result<Option<i64>> {
    match row.get_ref(column_name)? {
        rusqlite::types::ValueRef::Null => Ok(None),
        _ => row.get(column_name).map(Some),
    }
}

fn optional_f64(row: &rusqlite::Row<'_>, column_name: &str) -> rusqlite::Result<Option<f64>> {
    match row.get_ref(column_name)? {
        rusqlite::types::ValueRef::Null => Ok(None),
        _ => row.get(column_name).map(Some),
    }
}

fn resolve_current_display_date(
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

fn get_current_effective_week_legacy(
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

fn latest_completed_display_date_for_season_week(
    conn: &Connection,
    season_id: &str,
    season_week: i32,
) -> Result<Option<String>, DbError> {
    conn.query_row(
        "SELECT data
         FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND status = 'Concluida'
           AND COALESCE(season_week, week_of_year + 4) = ?2
         ORDER BY data DESC
         LIMIT 1",
        params![season_id, season_week],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

fn latest_completed_display_date_for_legacy_week(
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

fn infer_pre_event_display_date(entry: &CalendarEntry) -> Option<String> {
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

fn infer_pre_event_display_date_legacy(display_date: &str) -> Option<String> {
    let date = parse_display_date(display_date)?;
    Some(
        date.checked_sub_signed(chrono::Duration::days(7))?
            .format("%Y-%m-%d")
            .to_string(),
    )
}

fn days_between_display_dates(from: &str, to: &str) -> Option<i32> {
    let from_date = parse_display_date(from)?;
    let to_date = parse_display_date(to)?;
    let days = (to_date - from_date).num_days();
    i32::try_from(days).ok()
}

fn parse_display_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use chrono::Datelike;
    use rand::{rngs::StdRng, SeedableRng};
    use rusqlite::Connection;

    use super::*;
    use crate::calendar::{generate_and_insert_special_calendars, generate_calendar_for_category};
    use crate::db::migrations;
    use crate::db::queries::seasons::insert_season;
    use crate::models::season::Season;

    fn test_entry(
        id: &str,
        season_id: &str,
        categoria: &str,
        rodada: i32,
        week_of_year: i32,
        season_week: Option<u32>,
    ) -> CalendarEntry {
        CalendarEntry {
            id: id.to_string(),
            season_id: season_id.to_string(),
            categoria: categoria.to_string(),
            rodada,
            nome: format!("Round {rodada}"),
            track_id: rodada as u32,
            track_name: format!("Track {rodada}"),
            track_config: "Full".to_string(),
            clima: WeatherCondition::Dry,
            temperatura: 22.0,
            voltas: 20,
            duracao_corrida_min: 30,
            duracao_classificacao_min: 15,
            status: RaceStatus::Pendente,
            horario: "14:00".to_string(),
            week_of_year,
            season_phase: SeasonPhase::Temporada,
            display_date: format!("2024-{:02}-01", rodada.clamp(1, 12)),
            thematic_slot: ThematicSlot::NaoClassificado,
            season_week,
        }
    }

    #[test]
    fn test_insert_and_get_calendar() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(10);
        let entries = generate_calendar_for_category("S001", "gt4", &mut rng).expect("calendar");

        insert_calendar_entries(&conn, &entries).expect("insert");
        let loaded = get_calendar(&conn, "S001", "gt4").expect("select");
        assert_eq!(loaded.len(), entries.len());
    }

    #[test]
    fn test_normalize_calendar_display_dates_repairs_legacy_saturday_dates() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(12);
        let entries = generate_calendar_for_category("S001", "gt3", &mut rng).expect("calendar");
        let first_id = entries[0].id.clone();
        let expected_date = entries[0].display_date.clone();

        insert_calendar_entries(&conn, &entries).expect("insert");
        conn.execute(
            "UPDATE calendar SET data = '2024-03-02' WHERE id = ?1",
            params![first_id],
        )
        .expect("legacy saturday date");

        let updated = normalize_calendar_display_dates_for_weekday_policy(&conn, "S001", 2024)
            .expect("normalize dates");
        let repaired = get_calendar(&conn, "S001", "gt3").expect("calendar");

        assert!(updated >= 1);
        assert_eq!(repaired[0].display_date, expected_date);
    }

    #[test]
    fn test_normalize_calendar_display_dates_preserves_endurance_special_policy() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(188);
        let entries =
            generate_calendar_for_category("S001", "endurance", &mut rng).expect("calendar");

        insert_calendar_entries(&conn, &entries).expect("insert");
        conn.execute("UPDATE calendar SET data = '2024-03-02'", [])
            .expect("legacy saturday dates");

        let updated = normalize_calendar_display_dates_for_weekday_policy(&conn, "S001", 2024)
            .expect("normalize dates");
        let repaired = get_calendar(&conn, "S001", "endurance").expect("calendar");

        assert!(updated >= 1);
        assert!(repaired
            .iter()
            .all(|entry| parse_display_date(&entry.display_date)
                .expect("endurance date")
                .weekday()
                == chrono::Weekday::Sun));
    }

    #[test]
    fn test_normalize_calendar_display_dates_redistributes_legacy_special_calendar_to_late_december(
    ) {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(13);
        let entries = generate_calendar_for_category("S001", "production_challenger", &mut rng)
            .expect("special calendar");
        let last_id = entries.last().expect("last special").id.clone();

        insert_calendar_entries(&conn, &entries).expect("insert");
        conn.execute(
            "UPDATE calendar
             SET week_of_year = 49, data = '2024-12-08'
             WHERE id = ?1",
            params![last_id],
        )
        .expect("legacy compressed special date");

        let updated = normalize_calendar_display_dates_for_weekday_policy(&conn, "S001", 2024)
            .expect("normalize dates");
        let repaired = get_calendar(&conn, "S001", "production_challenger").expect("calendar");
        let last = repaired.last().expect("last repaired special");

        assert!(updated >= 1);
        assert_eq!(last.week_of_year, 52);
        assert_eq!(last.display_date, "2024-12-29");
    }

    #[test]
    fn test_get_next_race() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(11);
        let entries =
            generate_calendar_for_category("S001", "mazda_rookie", &mut rng).expect("calendar");

        insert_calendar_entries(&conn, &entries).expect("insert");
        let next = get_next_race(&conn, "S001", "mazda_rookie")
            .expect("next race")
            .expect("entry");
        assert_eq!(next.rodada, 1);
    }

    #[test]
    fn test_get_next_race_orders_by_season_week_not_round_or_week_of_year() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");

        let entries = [
            test_entry("R_LATE_ROUND", "S001", "gt3", 1, 6, Some(10)),
            test_entry("R_EARLY_SEASON_WEEK", "S001", "gt3", 2, 49, Some(1)),
        ];
        insert_calendar_entries(&conn, &entries).expect("insert");

        let next = get_next_race(&conn, "S001", "gt3")
            .expect("next race")
            .expect("entry");

        assert_eq!(next.id, "R_EARLY_SEASON_WEEK");
    }

    #[test]
    fn test_mark_race_completed() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(12);
        let entry = generate_calendar_for_category("S001", "gt3", &mut rng)
            .expect("calendar")
            .into_iter()
            .next()
            .expect("entry");

        insert_calendar_entry(&conn, &entry).expect("insert");
        mark_race_completed(&conn, &entry.id).expect("update");

        let loaded = get_calendar_entry_by_id(&conn, &entry.id)
            .expect("select")
            .expect("entry");
        assert_eq!(loaded.status, RaceStatus::Concluida);
    }

    #[test]
    fn test_get_pending_races_up_to_week_basic() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(20);
        let entries =
            generate_calendar_for_category("S001", "gt3", &mut rng).expect("gt3 calendar");
        insert_calendar_entries(&conn, &entries).expect("insert");

        // A primeira corrida regular cai apenas após a abertura da janela de fevereiro.
        // Com target=1 não deve retornar nada; com target=52 deve retornar todas.
        let none = get_pending_races_up_to_week(&conn, "S001", "gt3", 1).expect("query");
        assert!(
            none.is_empty(),
            "target_week=1 should return nothing for gt3"
        );

        let all = get_pending_races_up_to_week(&conn, "S001", "gt3", 52).expect("query");
        assert_eq!(all.len(), entries.len());
    }

    #[test]
    fn test_get_pending_races_up_to_week_ordering() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(21);
        let entries =
            generate_calendar_for_category("S001", "gt3", &mut rng).expect("gt3 calendar");
        insert_calendar_entries(&conn, &entries).expect("insert");

        let all = get_pending_races_up_to_week(&conn, "S001", "gt3", 52).expect("query");
        for window in all.windows(2) {
            assert!(
                calendar_entry_season_week(&window[0]) <= calendar_entry_season_week(&window[1]),
                "results must be ordered by season_week ASC"
            );
        }
    }

    #[test]
    fn test_pending_races_up_to_week_uses_season_week_window_and_order() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");

        let entries = [
            test_entry("R_SW10", "S001", "gt3", 1, 6, Some(10)),
            test_entry("R_SW01", "S001", "gt3", 2, 49, Some(1)),
            test_entry("R_SW05", "S001", "gt3", 3, 1, Some(5)),
        ];
        insert_calendar_entries(&conn, &entries).expect("insert");

        let up_to_five = get_pending_races_up_to_week(&conn, "S001", "gt3", 5).expect("query");
        let ids: Vec<_> = up_to_five.iter().map(|entry| entry.id.as_str()).collect();
        assert_eq!(ids, vec!["R_SW01", "R_SW05"]);

        let all = get_pending_races_up_to_week(&conn, "S001", "gt3", 10).expect("query");
        let ids: Vec<_> = all.iter().map(|entry| entry.id.as_str()).collect();
        assert_eq!(ids, vec!["R_SW01", "R_SW05", "R_SW10"]);
    }

    #[test]
    fn test_legacy_backfill_fallback_preserves_week_of_year_order() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");

        let entries = [
            test_entry("R_WOY49", "S001", "production_challenger", 1, 49, None),
            test_entry("R_WOY48", "S001", "production_challenger", 2, 48, None),
            test_entry("R_WOY52", "S001", "production_challenger", 3, 52, None),
        ];
        insert_calendar_entries(&conn, &entries).expect("insert");

        let all = get_pending_races_up_to_week(&conn, "S001", "production_challenger", 56)
            .expect("query");
        let ids: Vec<_> = all.iter().map(|entry| entry.id.as_str()).collect();

        assert_eq!(ids, vec!["R_WOY48", "R_WOY49", "R_WOY52"]);
    }

    #[test]
    fn test_get_pending_races_up_to_week_skips_zero() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(22);
        let mut entries =
            generate_calendar_for_category("S001", "gt3", &mut rng).expect("gt3 calendar");
        // Forçar week_of_year=0 para simular dado legado
        for e in &mut entries {
            e.week_of_year = 0;
        }
        insert_calendar_entries(&conn, &entries).expect("insert");

        let result = get_pending_races_up_to_week(&conn, "S001", "gt3", 52).expect("query");
        assert!(
            result.is_empty(),
            "entries with week_of_year=0 must be ignored"
        );
    }

    #[test]
    fn test_invalid_weather_condition_from_db_returns_error() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(23);
        let entry = generate_calendar_for_category("S001", "gt4", &mut rng)
            .expect("calendar")
            .into_iter()
            .next()
            .expect("entry");

        insert_calendar_entry(&conn, &entry).expect("insert");
        conn.execute(
            "UPDATE calendar SET clima = 'tempo_quebrado' WHERE id = ?1",
            [entry.id.as_str()],
        )
        .expect("corrupt weather");

        let err =
            get_calendar_entry_by_id(&conn, &entry.id).expect_err("invalid weather should fail");
        assert!(err.to_string().contains("WeatherCondition inv"));
    }

    #[test]
    fn test_negative_track_id_from_db_returns_error() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(24);
        let entry = generate_calendar_for_category("S001", "gt4", &mut rng)
            .expect("calendar")
            .into_iter()
            .next()
            .expect("entry");

        insert_calendar_entry(&conn, &entry).expect("insert");
        conn.execute(
            "UPDATE calendar SET track_id = -1 WHERE id = ?1",
            params![&entry.id],
        )
        .expect("corrupt track id");

        let err = get_calendar_entry_by_id(&conn, &entry.id).expect_err("invalid track id");
        assert!(err.to_string().contains("track_id"));
    }

    #[test]
    fn test_negative_week_of_year_from_db_returns_error() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(25);
        let entry = generate_calendar_for_category("S001", "gt4", &mut rng)
            .expect("calendar")
            .into_iter()
            .next()
            .expect("entry");

        insert_calendar_entry(&conn, &entry).expect("insert");
        conn.execute(
            "UPDATE calendar SET week_of_year = -3 WHERE id = ?1",
            params![&entry.id],
        )
        .expect("corrupt week");

        let err = get_calendar_entry_by_id(&conn, &entry.id).expect_err("invalid week");
        assert!(err.to_string().contains("week_of_year"));
    }

    #[test]
    fn test_specials_excluded_before_special_window() {
        use crate::calendar::generate_and_insert_special_calendars;
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        // Precisa inserir pilotos/equipes mínimos para o guard não falhar?
        // Não — generate_and_insert_special_calendars só precisa do conn + season_id.
        let mut rng = StdRng::seed_from_u64(23);
        generate_and_insert_special_calendars(&conn, "S001", 2024, &mut rng)
            .expect("special calendars");

        let first_special_week: i32 = conn
            .query_row(
                "SELECT MIN(COALESCE(season_week, week_of_year + 4))
                 FROM calendar WHERE categoria = 'production_challenger'",
                [],
                |row| row.get(0),
            )
            .expect("first special week");
        let last_special_week: i32 = conn
            .query_row(
                "SELECT MAX(COALESCE(season_week, week_of_year + 4))
                 FROM calendar WHERE categoria = 'production_challenger'",
                [],
                |row| row.get(0),
            )
            .expect("last special week");

        // Antes da primeira semana especial não deve retornar nenhuma corrida especial.
        let pc = get_pending_races_up_to_week(
            &conn,
            "S001",
            "production_challenger",
            first_special_week - 1,
        )
        .expect("query pc");
        let end = get_pending_races_up_to_week(&conn, "S001", "endurance", first_special_week - 1)
            .expect("query end");
        assert!(
            pc.is_empty(),
            "production_challenger should not appear before the first scheduled special week"
        );
        assert!(
            end.is_empty(),
            "endurance should not appear before the first scheduled special week"
        );

        // Na última semana especial, todas já devem estar visíveis.
        let pc_all =
            get_pending_races_up_to_week(&conn, "S001", "production_challenger", last_special_week)
                .expect("query pc all");
        assert_eq!(
            pc_all.len(),
            10,
            "production_challenger should have 10 races"
        );
    }

    #[test]
    fn test_get_pending_races_for_category_filters_and_orders() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(13);

        let mazda_entries =
            generate_calendar_for_category("S001", "mazda_rookie", &mut rng).expect("mazda");
        let mut gt3_entries = generate_calendar_for_category("S001", "gt3", &mut rng).expect("gt3");
        for entry in &mut gt3_entries {
            entry.id = format!("gt3_{}", entry.id);
        }

        insert_calendar_entries(&conn, &mazda_entries).expect("insert mazda");
        insert_calendar_entries(&conn, &gt3_entries).expect("insert gt3");
        mark_race_completed(&conn, &gt3_entries[0].id).expect("complete gt3 round 1");

        let pending =
            get_pending_races_for_category(&conn, "S001", "gt3").expect("pending gt3 races");

        assert_eq!(pending.len(), gt3_entries.len() - 1);
        assert!(pending.iter().all(|entry| entry.categoria == "gt3"));
        assert_eq!(pending[0].rodada, 2);
    }

    #[test]
    fn test_get_current_effective_week_empty() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let result = get_current_effective_week(&conn, "S001").expect("query");
        assert_eq!(result, None, "no completed races should return None");
    }

    #[test]
    fn test_get_current_effective_week_with_completions() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(40);
        let entries = generate_calendar_for_category("S001", "gt3", &mut rng).expect("calendar");
        insert_calendar_entries(&conn, &entries).expect("insert");

        // Mark first and last as completed
        mark_race_completed(&conn, &entries[0].id).expect("complete first");
        mark_race_completed(&conn, &entries[entries.len() - 1].id).expect("complete last");

        let result = get_current_effective_week(&conn, "S001")
            .expect("query")
            .expect("should have a value");

        let expected_max = calendar_entry_season_week(&entries[0])
            .max(calendar_entry_season_week(&entries[entries.len() - 1]));
        assert_eq!(result, expected_max);
    }

    #[test]
    fn test_count_pending_in_phase_regular() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(41);
        let entries = generate_calendar_for_category("S001", "gt3", &mut rng).expect("calendar");
        insert_calendar_entries(&conn, &entries).expect("insert");

        let count = count_pending_races_in_phase(
            &conn,
            "S001",
            &crate::models::enums::SeasonPhase::BlocoRegular,
        )
        .expect("count");

        assert_eq!(count, entries.len() as i32);
    }

    #[test]
    fn test_count_pending_in_phase_zero() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(42);
        let entries = generate_calendar_for_category("S001", "gt3", &mut rng).expect("calendar");
        insert_calendar_entries(&conn, &entries).expect("insert");

        let count = count_pending_races_in_phase(
            &conn,
            "S001",
            &crate::models::enums::SeasonPhase::BlocoEspecial,
        )
        .expect("count");

        assert_eq!(count, 0, "BlocoEspecial has no entries");
    }

    #[test]
    fn test_get_temporal_summary_basic() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(43);
        let entries = generate_calendar_for_category("S001", "gt3", &mut rng).expect("calendar");
        insert_calendar_entries(&conn, &entries).expect("insert");

        mark_race_completed(&conn, &entries[0].id).expect("complete 0");
        mark_race_completed(&conn, &entries[1].id).expect("complete 1");

        let phase = crate::models::enums::SeasonPhase::BlocoRegular;
        let summary = get_season_temporal_summary(&conn, "S001", "gt3", &phase).expect("summary");

        assert_eq!(
            summary.fase,
            crate::models::enums::SeasonPhase::BlocoRegular
        );

        let expected_week = entries[0].week_of_year.max(entries[1].week_of_year);
        assert_eq!(summary.effective_week, Some(expected_week));

        assert!(summary.next_player_event.is_some());
        assert_eq!(summary.next_player_event.unwrap().id, entries[2].id);

        assert_eq!(summary.pending_in_phase, (entries.len() - 2) as i32);
        assert_eq!(summary.current_display_date, entries[1].display_date);
        assert_eq!(
            summary.next_event_display_date.as_deref(),
            Some(entries[2].display_date.as_str())
        );
        let expected_days_until = (parse_display_date(&entries[2].display_date).expect("next date")
            - parse_display_date(&entries[1].display_date).expect("current date"))
        .num_days() as i32;
        assert_eq!(summary.days_until_next_event, Some(expected_days_until));
    }

    #[test]
    fn test_temporal_summary_temporada_uses_season_week_and_full_pending_count() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");

        let mut completed = test_entry("GT3_DONE", "S001", "gt3", 1, 6, Some(10));
        completed.status = RaceStatus::Concluida;
        completed.display_date = "2024-02-10".to_string();
        let mut player_next = test_entry("GT3_NEXT", "S001", "gt3", 2, 49, Some(11));
        player_next.display_date = "2024-02-17".to_string();
        let other_next = test_entry("END_NEXT", "S001", "endurance", 1, 1, Some(12));
        insert_calendar_entries(&conn, &[completed.clone(), player_next.clone(), other_next])
            .expect("insert");

        let summary = get_season_temporal_summary(&conn, "S001", "gt3", &SeasonPhase::Temporada)
            .expect("summary");

        assert_eq!(summary.effective_week, Some(10));
        assert_eq!(
            summary.next_player_event.as_ref().map(|e| e.id.as_str()),
            Some("GT3_NEXT")
        );
        assert_eq!(summary.pending_in_phase, 2);
        assert_eq!(summary.current_display_date, completed.display_date);
        assert_eq!(
            summary.next_event_display_date.as_deref(),
            Some(player_next.display_date.as_str())
        );
        assert_eq!(summary.days_until_next_event, Some(7));
    }

    #[test]
    fn test_temporal_summary_temporada_resolves_real_9d_divisions() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");

        let gt3 = test_entry("GT3_NEXT", "S001", "gt3", 1, 6, Some(10));
        let production = test_entry(
            "PRODUCTION_TOYOTA_NEXT",
            "S001",
            "production_challenger",
            1,
            7,
            Some(11),
        );
        let endurance = test_entry("ENDURANCE_LMP2_NEXT", "S001", "endurance", 1, 8, Some(12));
        insert_calendar_entries(&conn, &[gt3.clone(), production.clone(), endurance.clone()])
            .expect("insert");

        for (category, expected_id) in [
            ("gt3", gt3.id.as_str()),
            ("production_challenger", production.id.as_str()),
            ("endurance", endurance.id.as_str()),
        ] {
            let summary =
                get_season_temporal_summary(&conn, "S001", category, &SeasonPhase::Temporada)
                    .expect("summary");
            assert_eq!(
                summary
                    .next_player_event
                    .as_ref()
                    .map(|entry| entry.id.as_str()),
                Some(expected_id),
                "summary should resolve next event for {category}"
            );
            assert_eq!(summary.pending_in_phase, 3);
        }
    }

    #[test]
    fn test_temporal_summary_encerramento_exposes_year_end_state() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");

        let mut completed = test_entry("GT3_DONE", "S001", "gt3", 1, 47, Some(51));
        completed.status = RaceStatus::Concluida;
        completed.display_date = "2024-11-23".to_string();
        insert_calendar_entry(&conn, &completed).expect("insert");

        let summary = get_season_temporal_summary(&conn, "S001", "gt3", &SeasonPhase::Encerramento)
            .expect("summary");

        assert_eq!(summary.fase, SeasonPhase::Encerramento);
        assert_eq!(summary.effective_week, Some(51));
        assert!(summary.next_player_event.is_none());
        assert_eq!(summary.next_event_display_date, None);
        assert_eq!(summary.days_until_next_event, None);
        assert_eq!(summary.pending_in_phase, 0);
        assert_eq!(summary.current_display_date, completed.display_date);
    }

    #[test]
    fn test_get_temporal_summary_uses_next_any_event_when_player_has_no_pending_race() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(45);
        generate_and_insert_special_calendars(&conn, "S001", 2024, &mut rng)
            .expect("special calendars");

        let next_any = get_next_any_race_in_phase(&conn, "S001", &SeasonPhase::BlocoEspecial)
            .expect("next any")
            .expect("special event");
        let expected_current = infer_pre_event_display_date_legacy(&next_any.display_date)
            .expect("pre event display date");

        let summary =
            get_season_temporal_summary(&conn, "S001", "gt3", &SeasonPhase::BlocoEspecial)
                .expect("summary");

        assert!(summary.next_player_event.is_none());
        assert_eq!(
            summary.next_event_display_date.as_deref(),
            Some(next_any.display_date.as_str())
        );
        assert_eq!(summary.current_display_date, expected_current);
        assert_eq!(summary.days_until_next_event, Some(7));
    }

    #[test]
    fn test_invalid_season_phase_in_calendar_row_returns_error() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).expect("season");
        let mut rng = StdRng::seed_from_u64(44);
        let entry = generate_calendar_for_category("S001", "gt3", &mut rng)
            .expect("calendar")
            .into_iter()
            .next()
            .expect("entry");

        insert_calendar_entry(&conn, &entry).expect("insert");
        conn.execute(
            "UPDATE calendar SET season_phase = 'fase_quebrada' WHERE id = ?1",
            params![&entry.id],
        )
        .expect("corrupt season phase");

        let err = get_calendar_entry_by_id(&conn, &entry.id).expect_err("invalid season phase");
        assert!(err.to_string().contains("SeasonPhase inválido"));
    }
}
