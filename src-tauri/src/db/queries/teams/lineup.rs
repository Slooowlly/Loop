//! Vínculos de pilotos com a equipe: lineup, hierarquia interna e contadores de duelo.

use rusqlite::{params, Connection};

use crate::db::connection::DbError;
use crate::models::team::{Team, TeamHierarchyClimate};

use super::leitura::get_team_by_id;
use super::mapeamento::ensure_team_rows_affected;

/// Limpa `piloto_1_id` e `piloto_2_id` de todas as equipes especiais.
/// Afeta production_challenger (mazda/toyota/bmw) e endurance (gt4/gt3/lmp2).
pub fn clear_special_team_lineups(conn: &Connection) -> Result<usize, DbError> {
    let n = conn.execute(
        "UPDATE teams SET piloto_1_id = NULL, piloto_2_id = NULL
         WHERE categoria IN ('production_challenger', 'endurance')",
        [],
    )?;
    Ok(n)
}

/// Reseta todos os campos de hierarquia das equipes especiais.
pub fn reset_special_team_hierarchies(conn: &Connection) -> Result<(), DbError> {
    conn.execute(
        "UPDATE teams SET
            hierarquia_n1_id = NULL, hierarquia_n2_id = NULL,
            hierarquia_status = 'estavel', hierarquia_tensao = 0.0,
            hierarquia_duelos_total = 0, hierarquia_duelos_n2_vencidos = 0,
            hierarquia_sequencia_n2 = 0, hierarquia_sequencia_n1 = 0,
            hierarquia_inversoes_temporada = 0
         WHERE categoria IN ('production_challenger', 'endurance')",
        [],
    )?;
    Ok(())
}

pub fn update_team_pilots(
    conn: &Connection,
    team_id: &str,
    piloto_1_id: Option<&str>,
    piloto_2_id: Option<&str>,
) -> Result<(), DbError> {
    let affected = conn.execute(
        "UPDATE teams SET piloto_1_id = ?1, piloto_2_id = ?2 WHERE id = ?3",
        params![piloto_1_id, piloto_2_id, team_id],
    )?;
    ensure_team_rows_affected(affected, team_id, "atualizar pilotos da equipe")?;
    Ok(())
}

pub fn update_team_hierarchy(
    conn: &Connection,
    team_id: &str,
    n1_id: Option<&str>,
    n2_id: Option<&str>,
    status: &str,
    tensao: f64,
) -> Result<(), DbError> {
    let normalized = TeamHierarchyClimate::from_str_strict(status)
        .map_err(DbError::InvalidData)?
        .as_str()
        .to_string();
    let affected = conn.execute(
        "UPDATE teams
         SET hierarquia_n1_id = ?1,
             hierarquia_n2_id = ?2,
             hierarquia_status = ?3,
             hierarquia_tensao = ?4
         WHERE id = ?5",
        params![n1_id, n2_id, normalized, tensao, team_id],
    )?;
    ensure_team_rows_affected(affected, team_id, "atualizar hierarquia da equipe")?;
    Ok(())
}

/// Persiste todos os 9 campos da hierarquia interna de uma equipe de uma vez.
/// Use este após processar o sistema de hierarquia pós-corrida.
pub fn update_team_hierarchy_full(conn: &Connection, team: &Team) -> Result<(), DbError> {
    TeamHierarchyClimate::from_str_strict(&team.hierarquia_status).map_err(DbError::InvalidData)?;
    let affected = conn.execute(
        "UPDATE teams
         SET hierarquia_n1_id = ?1,
             hierarquia_n2_id = ?2,
             hierarquia_status = ?3,
             hierarquia_tensao = ?4,
             hierarquia_duelos_total = ?5,
             hierarquia_duelos_n2_vencidos = ?6,
             hierarquia_sequencia_n2 = ?7,
             hierarquia_sequencia_n1 = ?8,
             hierarquia_inversoes_temporada = ?9
         WHERE id = ?10",
        rusqlite::params![
            &team.hierarquia_n1_id,
            &team.hierarquia_n2_id,
            &team.hierarquia_status,
            team.hierarquia_tensao,
            team.hierarquia_duelos_total,
            team.hierarquia_duelos_n2_vencidos,
            team.hierarquia_sequencia_n2,
            team.hierarquia_sequencia_n1,
            team.hierarquia_inversoes_temporada,
            &team.id,
        ],
    )?;
    ensure_team_rows_affected(
        affected,
        &team.id,
        "atualizar hierarquia completa da equipe",
    )?;
    Ok(())
}

pub fn update_team_duel_counters(
    conn: &Connection,
    team_id: &str,
    duelos_total: i32,
    duelos_n2_vencidos: i32,
    sequencia_n2: i32,
    sequencia_n1: i32,
    inversoes_temporada: i32,
) -> Result<(), DbError> {
    let affected = conn.execute(
        "UPDATE teams
         SET hierarquia_duelos_total = ?1,
             hierarquia_duelos_n2_vencidos = ?2,
             hierarquia_sequencia_n2 = ?3,
             hierarquia_sequencia_n1 = ?4,
             hierarquia_inversoes_temporada = ?5
         WHERE id = ?6",
        params![
            duelos_total,
            duelos_n2_vencidos,
            sequencia_n2,
            sequencia_n1,
            inversoes_temporada,
            team_id
        ],
    )?;
    ensure_team_rows_affected(affected, team_id, "atualizar contadores de duelo da equipe")?;
    Ok(())
}

pub fn remove_pilot_from_team(
    conn: &Connection,
    driver_id: &str,
    team_id: &str,
) -> Result<(), DbError> {
    let team = get_team_by_id(conn, team_id)?
        .ok_or_else(|| DbError::NotFound(format!("Equipe '{team_id}' nao encontrada")))?;
    let piloto_1 = if team.piloto_1_id.as_deref() == Some(driver_id) {
        None
    } else {
        team.piloto_1_id.as_deref()
    };
    let piloto_2 = if team.piloto_2_id.as_deref() == Some(driver_id) {
        None
    } else {
        team.piloto_2_id.as_deref()
    };
    let removed_from_hierarchy = team.hierarquia_n1_id.as_deref() == Some(driver_id)
        || team.hierarquia_n2_id.as_deref() == Some(driver_id);
    update_team_pilots(conn, team_id, piloto_1, piloto_2)?;
    if removed_from_hierarchy {
        update_team_hierarchy(
            conn,
            team_id,
            None,
            None,
            TeamHierarchyClimate::Estavel.as_str(),
            0.0,
        )?;
        update_team_duel_counters(conn, team_id, 0, 0, 0, 0, 0)?;
    }
    Ok(())
}
