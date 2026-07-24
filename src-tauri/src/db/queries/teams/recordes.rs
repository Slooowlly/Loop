//! Recordes agregados de equipe por categoria (títulos, vitórias, dobradinhas).

use rusqlite::{params, Connection};

use crate::db::connection::DbError;

/// Total de títulos de CONSTRUTORES de uma equipe na categoria (soma do arquivo).
pub fn get_team_category_constructor_titles(
    conn: &Connection,
    team_id: &str,
    categoria: &str,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COALESCE(SUM(titulos_construtores), 0) FROM team_season_archive
         WHERE team_id = ?1 AND categoria = ?2",
        params![team_id, categoria],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Maior número de títulos de construtores na categoria entre TODAS as equipes EXCETO
/// uma (para saber se a campeã da temporada virou dona isolada do recorde). 0 se nenhuma.
pub fn get_category_constructor_titles_leader_excluding(
    conn: &Connection,
    categoria: &str,
    exclude_team: &str,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COALESCE(MAX(cnt), 0) FROM (
            SELECT team_id, SUM(titulos_construtores) AS cnt FROM team_season_archive
            WHERE categoria = ?1 AND team_id <> ?2
            GROUP BY team_id
         )",
        params![categoria, exclude_team],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Vitórias de uma equipe na categoria (todas as temporadas), contadas do resultado.
pub fn get_team_category_wins(
    conn: &Connection,
    team_id: &str,
    categoria: &str,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COUNT(*) FROM race_results r JOIN calendar c ON r.race_id = c.id
         WHERE r.equipe_id = ?1 AND c.categoria = ?2 AND r.posicao_final = 1",
        params![team_id, categoria],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Maior número de vitórias na categoria entre TODAS as equipes EXCETO uma. 0 se nenhuma.
pub fn get_category_team_win_leader_excluding(
    conn: &Connection,
    categoria: &str,
    exclude_team: &str,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COALESCE(MAX(cnt), 0) FROM (
            SELECT r.equipe_id, COUNT(*) AS cnt FROM race_results r
            JOIN calendar c ON r.race_id = c.id
            WHERE c.categoria = ?1 AND r.posicao_final = 1 AND r.equipe_id <> ?2
            GROUP BY r.equipe_id
         )",
        params![categoria, exclude_team],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Dobradinhas (1-2 na mesma corrida) de uma equipe na categoria: corridas em que a
/// equipe terminou com um carro em 1º E outro em 2º.
pub fn get_team_category_one_two(
    conn: &Connection,
    team_id: &str,
    categoria: &str,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COUNT(*) FROM (
            SELECT r.race_id FROM race_results r JOIN calendar c ON r.race_id = c.id
            WHERE r.equipe_id = ?1 AND c.categoria = ?2 AND r.posicao_final IN (1, 2)
            GROUP BY r.race_id HAVING COUNT(*) = 2
         )",
        params![team_id, categoria],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Maior número de dobradinhas na categoria entre TODAS as equipes EXCETO uma. 0 se nenhuma.
pub fn get_category_one_two_leader_excluding(
    conn: &Connection,
    categoria: &str,
    exclude_team: &str,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COALESCE(MAX(cnt), 0) FROM (
            SELECT equipe_id, COUNT(*) AS cnt FROM (
                SELECT r.equipe_id, r.race_id FROM race_results r
                JOIN calendar c ON r.race_id = c.id
                WHERE c.categoria = ?1 AND r.posicao_final IN (1, 2) AND r.equipe_id <> ?2
                GROUP BY r.equipe_id, r.race_id HAVING COUNT(*) = 2
            )
            GROUP BY equipe_id
         )",
        params![categoria, exclude_team],
        |r| r.get(0),
    )?;
    Ok(n)
}
