//! Escrita do calendario: insercao de etapas, marcacao de corrida concluida,
//! remocao por temporada e normalizacao das datas de exibicao pela politica de dia da semana.

use super::*;

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

pub fn mark_race_completed(conn: &Connection, id: &str) -> Result<(), DbError> {
    conn.execute(
        "UPDATE calendar SET status = 'Concluida' WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn delete_calendar_for_season(conn: &Connection, season_id: &str) -> Result<(), DbError> {
    conn.execute(
        "DELETE FROM calendar WHERE COALESCE(season_id, temporada_id) = ?1",
        params![season_id],
    )?;
    Ok(())
}
