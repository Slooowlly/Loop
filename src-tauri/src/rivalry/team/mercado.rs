//! Fonte 2: Roubo de talento (Mercado) — o Elo 2.
//!
//! Todo site do mercado onde o `equipe_id` de um piloto muda de B→A semeia rancor no par de
//! times. O rancor é proporcional ao que se perdeu (astro > titular > reserva) e ao
//! descaramento (assédio mid-contrato > troca livre). É isto que dá memória duradoura ao
//! "piloto largou e foi pro rival" — antes o destino do piloto não deixava marca no mundo.

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::db::queries::teams as team_queries;
use crate::models::driver::Driver;
use crate::models::team_rivalry::TeamRivalryType;

use super::motor::{apply_team_rivalry_event, TeamRivalryEvent};
use super::noticias::emit_team_rivalry_news;

/// Skill a partir da qual o piloto conta como "astro" para o rancor de mercado.
const STAR_SKILL: f64 = 80.0;
/// Mídia a partir da qual o piloto conta como "astro" (holofote), mesmo sem skill de elite.
const STAR_MIDIA: f64 = 70.0;
/// Skill mínimo para contar como "titular" (abaixo é peça menor/reserva).
const STARTER_SKILL: f64 = 50.0;

/// Semeia/reforça a rivalidade de MERCADO quando um piloto muda de `from_team` para
/// `to_team`. `is_poaching` = assédio mid-contrato (rancor máximo). Best-effort.
pub fn seed_team_rivalry_from_transfer(
    conn: &Connection,
    from_team_id: &str,
    to_team_id: &str,
    driver: &Driver,
    is_poaching: bool,
    temporada: i32,
) -> Result<(), DbError> {
    if from_team_id == to_team_id || from_team_id.is_empty() || to_team_id.is_empty() {
        return Ok(());
    }
    let Some(from_team) = team_queries::get_team_by_id(conn, from_team_id)? else {
        return Ok(());
    };
    let Some(to_team) = team_queries::get_team_by_id(conn, to_team_id)? else {
        return Ok(());
    };

    let was_n1 = from_team.hierarquia_n1_id.as_deref() == Some(driver.id.as_str());
    let is_star =
        driver.atributos.skill >= STAR_SKILL || driver.atributos.midia >= STAR_MIDIA || was_n1;

    let (mut h, mut r) = if is_star && is_poaching {
        (8.0, 16.0)
    } else if is_star {
        (6.0, 12.0)
    } else if driver.atributos.skill >= STARTER_SKILL {
        (3.0, 8.0)
    } else {
        (1.0, 4.0)
    };
    // Rivalidade entre divisões (categorias diferentes) pesa metade.
    if from_team.categoria != to_team.categoria {
        h *= 0.5;
        r *= 0.5;
    }

    let applied = apply_team_rivalry_event(
        conn,
        &TeamRivalryEvent {
            team_a: from_team_id.to_string(),
            team_b: to_team_id.to_string(),
            tipo: TeamRivalryType::Mercado,
            historical_delta: h,
            recent_delta: r,
            temporada,
        },
    )?;
    emit_team_rivalry_news(
        conn,
        &applied,
        TeamRivalryType::Mercado,
        from_team_id,
        to_team_id,
        Some(&to_team.categoria),
        None,
        temporada,
    )?;
    Ok(())
}
