//! Gestão de longo prazo da equipe: plano estratégico, colapso, promoções,
//! contadores de resgate e eventos de propriedade/diretoria.

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;

/// Plano estratégico de longo prazo (3 temporadas) da equipe (Pilar C).
/// Retorna (tipo, anos_restantes); default ("sustainable", 0) se não houver
/// registro — o que faz a próxima pré-temporada escolher um plano novo.
pub fn get_strategic_plan(conn: &Connection, team_id: &str) -> Result<(String, i32), DbError> {
    let row = conn
        .query_row(
            "SELECT plan_type, remaining_years FROM team_strategic_plan WHERE team_id = ?1",
            params![team_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
        )
        .optional()?;
    Ok(row.unwrap_or_else(|| ("sustainable".to_string(), 0)))
}

/// Grava o plano estratégico da equipe (upsert).
pub fn set_strategic_plan(
    conn: &Connection,
    team_id: &str,
    plan_type: &str,
    remaining_years: i32,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO team_strategic_plan (team_id, plan_type, remaining_years)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(team_id) DO UPDATE SET
            plan_type = excluded.plan_type,
            remaining_years = excluded.remaining_years",
        params![team_id, plan_type, remaining_years],
    )?;
    Ok(())
}

/// Zera o plano da equipe (remove o registro). Usado quando a equipe muda de
/// categoria (promoção/rebaixamento): a próxima pré-temporada re-avalia o plano
/// para a nova realidade. (Pilar C)
pub fn reset_strategic_plan(conn: &Connection, team_id: &str) -> Result<(), DbError> {
    conn.execute(
        "DELETE FROM team_strategic_plan WHERE team_id = ?1",
        params![team_id],
    )?;
    Ok(())
}

/// Temporadas consecutivas em colapso financeiro de uma equipe (0 se não houver
/// registro). Suporta o evento de venda/nova diretoria.
pub fn get_collapse_streak(conn: &Connection, team_id: &str) -> Result<i32, DbError> {
    let streak = conn
        .query_row(
            "SELECT streak FROM team_collapse_streak WHERE team_id = ?1",
            params![team_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(streak.unwrap_or(0))
}

/// Grava o contador de temporadas consecutivas em colapso de uma equipe.
pub fn set_collapse_streak(conn: &Connection, team_id: &str, streak: i32) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO team_collapse_streak (team_id, streak) VALUES (?1, ?2)
         ON CONFLICT(team_id) DO UPDATE SET streak = excluded.streak",
        params![team_id, streak],
    )?;
    Ok(())
}

/// Histórico de promoção de uma equipe: `(última temporada em que subiu, nº de promoções
/// na janela móvel)`. `(0, 0)` se não houver registro — nunca subiu (ou save antigo). Base
/// do retorno decrescente anti-snowball do chain-promotion (ver `promotion::effects`).
pub fn get_promotion_history(conn: &Connection, team_id: &str) -> Result<(i32, i32), DbError> {
    let row = conn
        .query_row(
            "SELECT last_promotion_season, recent_promotions
             FROM team_promotion_history WHERE team_id = ?1",
            params![team_id],
            |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?)),
        )
        .optional()?;
    Ok(row.unwrap_or((0, 0)))
}

/// Grava o histórico de promoção de uma equipe (upsert). `last_promotion_season = 0` zera
/// a contagem (ex.: rebaixamento quebra a cadeia de promoções).
pub fn set_promotion_history(
    conn: &Connection,
    team_id: &str,
    last_promotion_season: i32,
    recent_promotions: i32,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO team_promotion_history (team_id, last_promotion_season, recent_promotions)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(team_id) DO UPDATE SET
            last_promotion_season = excluded.last_promotion_season,
            recent_promotions = excluded.recent_promotions",
        params![team_id, last_promotion_season, recent_promotions],
    )?;
    Ok(())
}

/// Incrementa um contador agregado de eventos de resgate (ex.: "sold",
/// "self_rescued"). Usado para estatística e telemetria.
pub fn incr_rescue_counter(conn: &Connection, key: &str) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO team_rescue_counters (key, value) VALUES (?1, 1)
         ON CONFLICT(key) DO UPDATE SET value = value + 1",
        params![key],
    )?;
    Ok(())
}

/// Lê um contador agregado de eventos de resgate (0 se ausente).
pub fn get_rescue_counter(conn: &Connection, key: &str) -> Result<i64, DbError> {
    let value = conn
        .query_row(
            "SELECT value FROM team_rescue_counters WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value.unwrap_or(0))
}

/// Registra um evento de propriedade/diretoria da equipe (ex.: venda por colapso
/// crônico). Exibido na ficha da equipe.
#[allow(clippy::too_many_arguments)]
pub fn insert_team_ownership_event(
    conn: &Connection,
    team_id: &str,
    season_number: i32,
    ano: i32,
    event_type: &str,
    debt_cleared: f64,
    cash_injected: f64,
    detail: &str,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO team_ownership_events
            (team_id, season_number, ano, event_type, debt_cleared, cash_injected, detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            team_id,
            season_number,
            ano,
            event_type,
            debt_cleared,
            cash_injected,
            detail
        ],
    )?;
    Ok(())
}

/// Evento de mudança de dono/diretoria mais recente de uma equipe (ex.: venda após
/// colapso). Devolve `(event_type, season_number)` do último evento, se houver. Usado
/// no rodapé de notícias para a notinha "nova diretoria assumiu a equipe X".
pub fn get_latest_ownership_event(
    conn: &Connection,
    team_id: &str,
) -> Result<Option<(String, i32)>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT event_type, season_number
         FROM team_ownership_events
         WHERE team_id = ?1
         ORDER BY season_number DESC, id DESC
         LIMIT 1",
    )?;
    let row = stmt
        .query_row(params![team_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    Ok(row)
}
