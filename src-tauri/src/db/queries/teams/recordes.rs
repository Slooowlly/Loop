//! Recordes agregados de equipe por categoria (títulos, vitórias, dobradinhas).

use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::db::connection::DbError;

/// Títulos de construtores de TODAS as equipes de uma categoria, de uma consulta só.
///
/// A versão por equipe (`get_team_category_constructor_titles`) serve ao dossiê, que
/// olha uma equipe de cada vez. O grid da pré-temporada olha o grid inteiro — chamar
/// aquela em laço custaria uma consulta por equipe por categoria.
pub fn get_category_constructor_titles_by_team(
    conn: &Connection,
    categoria: &str,
) -> Result<HashMap<String, i32>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT team_id, COALESCE(SUM(titulos_construtores), 0) FROM team_season_archive
         WHERE categoria = ?1 GROUP BY team_id",
    )?;
    let rows = stmt.query_map(params![categoria], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i32>(1)?))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (team_id, total) = row?;
        map.insert(team_id, total);
    }
    Ok(map)
}

/// Vitórias de TODAS as equipes de uma categoria, de uma consulta só.
pub fn get_category_wins_by_team(
    conn: &Connection,
    categoria: &str,
) -> Result<HashMap<String, i32>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT r.equipe_id, COUNT(*) FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE c.categoria = ?1 AND r.posicao_final = 1 AND r.equipe_id IS NOT NULL
         GROUP BY r.equipe_id",
    )?;
    let rows = stmt.query_map(params![categoria], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i32>(1)?))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (team_id, total) = row?;
        map.insert(team_id, total);
    }
    Ok(map)
}

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
