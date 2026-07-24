//! Classificação (standings) de pilotos e equipes por categoria/temporada.

use rusqlite::Connection;

use crate::db::connection::DbError;

/// Entrada de standings para uma categoria/temporada.
#[derive(Debug, Clone)]
pub struct StandingEntry {
    pub pilot_id: String,
    pub pilot_name: String,
    pub points: f64,
    pub position: i32,
}

/// Retorna os standings completos de uma categoria em uma temporada.
/// Ordenado por pontos (desc), posição calculada sequencialmente.
pub fn get_category_standings(
    conn: &Connection,
    temporada_id: &str,
    categoria: &str,
) -> Result<Vec<StandingEntry>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT
            r.piloto_id,
            d.nome,
            SUM(r.pontos) as total_points,
            SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) as total_wins
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN drivers d ON r.piloto_id = d.id
         WHERE c.temporada_id = ?1 AND c.categoria = ?2
         GROUP BY r.piloto_id
         ORDER BY total_points DESC, total_wins DESC, d.nome ASC",
    )?;

    let mut standings = Vec::new();
    let mut rows = stmt.query(rusqlite::params![temporada_id, categoria])?;
    let mut position = 1;

    while let Some(row) = rows.next()? {
        standings.push(StandingEntry {
            pilot_id: row.get(0)?,
            pilot_name: row.get(1)?,
            points: row.get(2)?,
            position,
        });
        position += 1;
    }

    Ok(standings)
}

/// Entrada de standings de EQUIPES para uma categoria/temporada.
#[derive(Debug, Clone)]
pub struct TeamStandingEntry {
    pub team_id: String,
    pub team_name: String,
    pub points: f64,
    pub position: i32,
}

/// Retorna os standings de equipes (soma dos pontos de todos os pilotos da equipe)
/// de uma categoria na temporada. Ordenado por pontos (desc); posição sequencial.
/// Linhas sem `equipe_id` correspondente em `teams` são ignoradas pelo JOIN.
pub fn get_team_standings(
    conn: &Connection,
    temporada_id: &str,
    categoria: &str,
) -> Result<Vec<TeamStandingEntry>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT
            r.equipe_id,
            t.nome,
            SUM(r.pontos) as total_points
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN teams t ON r.equipe_id = t.id
         WHERE c.temporada_id = ?1 AND c.categoria = ?2
         GROUP BY r.equipe_id
         ORDER BY total_points DESC, t.nome ASC",
    )?;

    let mut standings = Vec::new();
    let mut rows = stmt.query(rusqlite::params![temporada_id, categoria])?;
    let mut position = 1;

    while let Some(row) = rows.next()? {
        standings.push(TeamStandingEntry {
            team_id: row.get(0)?,
            team_name: row.get(1)?,
            points: row.get(2)?,
            position,
        });
        position += 1;
    }

    Ok(standings)
}

/// Retorna o ID do piloto que liderava a categoria na temporada com base nos resultados
/// anteriores à rodada indicada (exclusive). Retorna None se não há rodadas anteriores.
/// Usado para detectar mudança de liderança: se o líder atual for diferente deste valor,
/// houve troca de liderança na rodada mais recente.
pub fn get_category_leader_before_round(
    conn: &Connection,
    temporada_id: &str,
    categoria: &str,
    before_round: i32,
) -> Result<Option<String>, DbError> {
    let result = conn.query_row(
        "SELECT r.piloto_id
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE c.temporada_id = ?1 AND c.categoria = ?2 AND c.rodada < ?3
         GROUP BY r.piloto_id
         ORDER BY
            SUM(r.pontos) DESC,
            SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) DESC,
            r.piloto_id ASC
         LIMIT 1",
        rusqlite::params![temporada_id, categoria, before_round],
        |row| row.get::<_, String>(0),
    );
    match result {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DbError::Sqlite(e)),
    }
}

/// Campeão da categoria numa temporada: o piloto que somou mais pontos. `None` se a
/// temporada não tem resultados na categoria. Base do sinal "campeão reinante" (a IA
/// que defende o título da temporada passada). Empate → menor `piloto_id` (determinístico).
pub fn get_category_champion_for_season(
    conn: &Connection,
    temporada_id: &str,
    categoria: &str,
) -> Result<Option<String>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT r.piloto_id, COALESCE(SUM(r.pontos), 0.0) AS pts
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE c.temporada_id = ?1 AND c.categoria = ?2
         GROUP BY r.piloto_id
         ORDER BY pts DESC, r.piloto_id ASC
         LIMIT 1",
    )?;
    let mut rows = stmt.query(rusqlite::params![temporada_id, categoria])?;
    match rows.next()? {
        Some(row) => Ok(Some(row.get::<_, String>(0)?)),
        None => Ok(None),
    }
}
