//! Leitura e listagem de equipes.

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;
use crate::models::team::Team;

use super::mapeamento::{attach_cars, collect_teams, colunas_select_team, team_from_row};

/// Fama (midia) dos pilotos do lineup REGULAR ativo de uma equipe. Base da presença
/// pública do time → patrocínio ([[fama → dinheiro]]). Vazio se o time não tem lineup.
pub fn get_team_lineup_medias(conn: &Connection, team_id: &str) -> Result<Vec<f64>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT d.midia FROM drivers d
         JOIN contracts c ON c.piloto_id = d.id
         WHERE c.equipe_id = ?1 AND c.status = 'Ativo' AND c.tipo = 'Regular'",
    )?;
    let medias = stmt
        .query_map(params![team_id], |row| row.get::<_, f64>(0))?
        .collect::<Result<Vec<f64>, _>>()?;
    Ok(medias)
}

pub fn get_team_by_id(conn: &Connection, id: &str) -> Result<Option<Team>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM teams WHERE id = ?1",
        colunas_select_team()
    ))?;
    let mut team = stmt.query_row(params![id], team_from_row).optional()?;
    if let Some(team) = team.as_mut() {
        team.car = crate::db::queries::team_car::get_team_car(conn, &team.id)?;
    }
    Ok(team)
}

pub fn get_all_teams(conn: &Connection) -> Result<Vec<Team>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM teams ORDER BY nome",
        colunas_select_team()
    ))?;
    let mapped = stmt.query_map([], team_from_row)?;
    let mut teams = collect_teams(mapped)?;
    attach_cars(conn, &mut teams)?;
    Ok(teams)
}

pub fn get_teams_by_category(conn: &Connection, category_id: &str) -> Result<Vec<Team>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM teams WHERE categoria = ?1 ORDER BY nome",
        colunas_select_team()
    ))?;
    let mapped = stmt.query_map(params![category_id], team_from_row)?;
    let mut teams = collect_teams(mapped)?;
    attach_cars(conn, &mut teams)?;
    Ok(teams)
}

/// Equipes de uma categoria filtradas por classe, ordenadas por desempenho desc.
/// Usado na convocação especial para montar o grid classe a classe.
pub fn get_teams_by_category_and_class(
    conn: &Connection,
    categoria: &str,
    classe: &str,
) -> Result<Vec<crate::models::team::Team>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM teams WHERE categoria = ?1 AND classe = ?2 ORDER BY car_performance DESC",
        colunas_select_team()
    ))?;
    let mapped = stmt.query_map(params![categoria, classe], team_from_row)?;
    let mut teams = collect_teams(mapped)?;
    attach_cars(conn, &mut teams)?;
    Ok(teams)
}

pub fn count_teams_by_category(conn: &Connection, category_id: &str) -> Result<i32, DbError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM teams WHERE categoria = ?1",
        params![category_id],
        |row| row.get(0),
    )?;
    Ok(count as i32)
}

pub fn count_teams_by_category_and_class(
    conn: &Connection,
    category_id: &str,
    class_name: &str,
) -> Result<i32, DbError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM teams WHERE categoria = ?1 AND classe = ?2",
        params![category_id, class_name],
        |row| row.get(0),
    )?;
    Ok(count as i32)
}
