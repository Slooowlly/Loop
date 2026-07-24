//! Histórico por CIRCUITO: vitórias na pista, donos do traçado e trauma de batida.

use rusqlite::{Connection, OptionalExtension};

use crate::db::connection::DbError;

/// Vitórias de um piloto numa pista específica da categoria (todas as temporadas).
pub fn get_pilot_track_wins(
    conn: &Connection,
    pilot_id: &str,
    categoria: &str,
    track_name: &str,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COUNT(*) FROM race_results r JOIN calendar c ON r.race_id = c.id
         WHERE r.piloto_id = ?1 AND c.categoria = ?2 AND c.track_name = ?3
           AND r.posicao_final = 1",
        rusqlite::params![pilot_id, categoria, track_name],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Maior número de vitórias numa pista entre TODOS os pilotos EXCETO um (para saber se
/// o vencedor de hoje virou o "dono" isolado do circuito). 0 se ninguém mais venceu lá.
pub fn get_track_win_leader_excluding(
    conn: &Connection,
    categoria: &str,
    track_name: &str,
    exclude_pilot: &str,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COALESCE(MAX(cnt), 0) FROM (
            SELECT r.piloto_id, COUNT(*) AS cnt FROM race_results r
            JOIN calendar c ON r.race_id = c.id
            WHERE c.categoria = ?1 AND c.track_name = ?2 AND r.posicao_final = 1
              AND r.piloto_id <> ?3
            GROUP BY r.piloto_id
         )",
        rusqlite::params![categoria, track_name, exclude_pilot],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Pilotos que já BATERAM (DNF por DriverError/PostCollision) NESTA pista — trauma.
pub fn get_track_crash_pilots(
    conn: &Connection,
    track_id: u32,
) -> Result<std::collections::HashSet<String>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT r.piloto_id
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN incident_catalog ic ON r.dnf_catalog_id = ic.id
         WHERE c.track_id = ?1 AND r.dnf = 1
           AND ic.incident_source IN ('DriverError', 'PostCollision')",
    )?;
    let mut set = std::collections::HashSet::new();
    let mut rows = stmt.query(rusqlite::params![track_id])?;
    while let Some(row) = rows.next()? {
        set.insert(row.get::<_, String>(0)?);
    }
    Ok(set)
}

/// `track_id` da corrida (temporada/categoria/rodada). `None` se não encontrar.
pub fn get_round_track_id(
    conn: &Connection,
    temporada_id: &str,
    categoria: &str,
    round: i32,
) -> Result<Option<i64>, DbError> {
    let res: Option<Option<i64>> = conn
        .query_row(
            "SELECT track_id FROM calendar
             WHERE temporada_id = ?1 AND categoria = ?2 AND rodada = ?3",
            rusqlite::params![temporada_id, categoria, round],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?;
    Ok(res.flatten())
}

/// Histórico do piloto NESTE circuito (`track_id`), em todas as temporadas, EXCLUINDO
/// a rodada atual. Serve para a narrativa de "tem boa história nesta pista".
#[derive(Debug, Clone, Default)]
pub struct PilotTrackHistory {
    pub starts: i32,
    pub wins: i32,
    pub podiums: i32,
    /// Melhor chegada (sem contar abandonos). `None` se nunca completou aqui.
    pub best_finish: Option<i32>,
}

pub fn get_pilot_track_history(
    conn: &Connection,
    pilot_id: &str,
    track_id: i64,
    exclude_temporada_id: &str,
    exclude_round: i32,
) -> Result<PilotTrackHistory, DbError> {
    let row = conn.query_row(
        "SELECT
            COUNT(*),
            COALESCE(SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN r.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END), 0),
            MIN(CASE WHEN r.dnf = 0 THEN r.posicao_final END)
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE r.piloto_id = ?1 AND c.track_id = ?2
           AND NOT (c.temporada_id = ?3 AND c.rodada = ?4)",
        rusqlite::params![pilot_id, track_id, exclude_temporada_id, exclude_round],
        |r| {
            Ok(PilotTrackHistory {
                starts: r.get(0)?,
                wins: r.get(1)?,
                podiums: r.get(2)?,
                best_finish: r.get::<_, Option<i32>>(3)?,
            })
        },
    )?;
    Ok(row)
}
