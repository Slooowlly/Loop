//! Estatísticas de CARREIRA do piloto: vitórias, títulos, jejum e sequências.

use rusqlite::{Connection, OptionalExtension};

use crate::db::connection::DbError;

/// Retorna a última vitória na carreira do piloto (qualquer categoria/temporada).
/// Retorna (season_num, round) ou None se nunca venceu.
pub fn get_last_career_win(
    conn: &Connection,
    pilot_id: &str,
) -> Result<Option<(i32, i32)>, DbError> {
    let result: Result<(i32, i32), _> = conn.query_row(
        "SELECT s.numero, c.rodada
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN seasons s ON c.temporada_id = s.id
         WHERE r.piloto_id = ?1 AND r.posicao_final = 1
         ORDER BY s.numero DESC, c.rodada DESC
         LIMIT 1",
        rusqlite::params![pilot_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );

    match result {
        Ok(pair) => Ok(Some(pair)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DbError::Sqlite(e)),
    }
}

/// Retorna o número de vitórias do piloto com uma equipe específica (histórico completo).
pub fn get_wins_with_team(
    conn: &Connection,
    pilot_id: &str,
    team_id: &str,
) -> Result<i32, DbError> {
    let count: i32 = conn.query_row(
        "SELECT COUNT(*)
         FROM race_results
         WHERE piloto_id = ?1 AND equipe_id = ?2 AND posicao_final = 1",
        rusqlite::params![pilot_id, team_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

/// Estatística de carreira de um piloto NA CATEGORIA (todas as temporadas no DB).
#[derive(Debug, Clone)]
pub struct DriverCategoryCareer {
    pub wins: i32,
    pub podiums: i32,
    pub starts: i32,
}

/// Vitórias, pódios e largadas de um piloto na categoria, somando todas as temporadas.
/// Inclui a corrida atual se ela já estiver persistida em `race_results`.
pub fn get_driver_category_career(
    conn: &Connection,
    pilot_id: &str,
    categoria: &str,
) -> Result<DriverCategoryCareer, DbError> {
    let row = conn.query_row(
        "SELECT
            COALESCE(SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN r.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END), 0),
            COUNT(*)
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE r.piloto_id = ?1 AND c.categoria = ?2",
        rusqlite::params![pilot_id, categoria],
        |r| {
            Ok(DriverCategoryCareer {
                wins: r.get(0)?,
                podiums: r.get(1)?,
                starts: r.get(2)?,
            })
        },
    )?;
    Ok(row)
}

/// Total de TÍTULOS de um piloto na categoria (temporadas encerradas como 1º), lido do
/// arquivo histórico `driver_season_archive`.
pub fn get_pilot_category_titles(
    conn: &Connection,
    pilot_id: &str,
    categoria: &str,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COUNT(*) FROM driver_season_archive
         WHERE piloto_id = ?1 AND categoria = ?2 AND posicao_campeonato = 1",
        rusqlite::params![pilot_id, categoria],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Temporada do prêmio da vitória mais recente do piloto na categoria ANTES de (temporada,
/// rodada) atual — para medir jejum. `None` se nunca venceu antes.
pub fn get_pilot_previous_win_season(
    conn: &Connection,
    pilot_id: &str,
    categoria: &str,
    before_season_num: i32,
    before_round: i32,
) -> Result<Option<i32>, DbError> {
    let row = conn
        .query_row(
            "SELECT s.numero
             FROM race_results r
             JOIN calendar c ON r.race_id = c.id
             JOIN seasons s ON c.temporada_id = s.id
             WHERE r.piloto_id = ?1 AND c.categoria = ?2 AND r.posicao_final = 1
               AND (s.numero < ?3 OR (s.numero = ?3 AND c.rodada < ?4))
             ORDER BY s.numero DESC, c.rodada DESC LIMIT 1",
            rusqlite::params![pilot_id, categoria, before_season_num, before_round],
            |r| r.get(0),
        )
        .optional()?;
    Ok(row)
}

/// Retorna o número de vitórias do piloto na categoria esta temporada.
pub fn get_category_wins_this_season(
    conn: &Connection,
    pilot_id: &str,
    temporada_id: &str,
    categoria: &str,
) -> Result<i32, DbError> {
    let count: i32 = conn.query_row(
        "SELECT COUNT(*)
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE r.piloto_id = ?1
           AND c.temporada_id = ?2
           AND c.categoria = ?3
           AND r.posicao_final = 1",
        rusqlite::params![pilot_id, temporada_id, categoria],
        |row| row.get(0),
    )?;
    Ok(count)
}

/// Retorna a sequência atual de vitórias do piloto na categoria/temporada indicadas.
/// Conta rodadas consecutivas com posicao_final = 1 a partir da mais recente para trás.
/// Retorna 0 se o piloto nunca venceu ou não disputou corridas nessa categoria/temporada.
pub fn get_win_streak(
    conn: &Connection,
    pilot_id: &str,
    temporada_id: &str,
    categoria: &str,
) -> Result<u32, DbError> {
    let mut stmt = conn.prepare(
        "SELECT r.posicao_final
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE r.piloto_id = ?1
           AND c.temporada_id = ?2
           AND c.categoria = ?3
         ORDER BY c.rodada DESC",
    )?;
    let mut positions: Vec<i32> = Vec::new();
    let mut rows = stmt.query(rusqlite::params![pilot_id, temporada_id, categoria])?;
    while let Some(row) = rows.next()? {
        positions.push(row.get::<_, i32>(0)?);
    }
    let streak = positions.iter().take_while(|&&pos| pos == 1).count() as u32;
    Ok(streak)
}
