#![allow(dead_code)]

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

/// Retorna os resultados de todos os pilotos numa rodada específica de uma categoria.
/// Retorna Vec<(driver_id, posicao_largada, posicao_final, is_dnf)>.
pub fn get_results_for_round(
    conn: &Connection,
    temporada_id: &str,
    categoria: &str,
    round: i32,
) -> Result<Vec<(String, i32, i32, bool)>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT r.piloto_id, r.posicao_largada, r.posicao_final, r.dnf
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE c.temporada_id = ?1 AND c.categoria = ?2 AND c.rodada = ?3
         ORDER BY r.posicao_final ASC",
    )?;
    let mut results = Vec::new();
    let mut rows = stmt.query(rusqlite::params![temporada_id, categoria, round])?;
    while let Some(row) = rows.next()? {
        results.push((
            row.get::<_, String>(0)?,
            row.get::<_, i32>(1)?,
            row.get::<_, i32>(2)?,
            row.get::<_, i32>(3)? != 0,
        ));
    }
    Ok(results)
}

/// Retorna fatos de DNF catalogado por piloto numa rodada de uma categoria.
/// Vec<(driver_id, incident_source, is_dnf, dnf_segment)>
/// incident_source vem de incident_catalog.incident_source (Mechanical/DriverError/PostCollision/Operational)
/// ou None se o piloto não tiver dnf_catalog_id.
pub fn get_dnf_incident_facts_for_round(
    conn: &Connection,
    temporada_id: &str,
    categoria: &str,
    round: i32,
) -> Result<Vec<(String, Option<String>, bool, Option<String>)>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT r.piloto_id, ic.incident_source, r.dnf, r.dnf_segment
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         LEFT JOIN incident_catalog ic ON r.dnf_catalog_id = ic.id
         WHERE c.temporada_id = ?1 AND c.categoria = ?2 AND c.rodada = ?3",
    )?;
    let mut results = Vec::new();
    let mut rows = stmt.query(rusqlite::params![temporada_id, categoria, round])?;
    while let Some(row) = rows.next()? {
        results.push((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, i32>(2)? != 0,
            row.get::<_, Option<String>>(3)?,
        ));
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE seasons (id TEXT PRIMARY KEY, numero INTEGER NOT NULL);
            CREATE TABLE calendar (
                id TEXT PRIMARY KEY,
                temporada_id TEXT NOT NULL,
                rodada INTEGER NOT NULL,
                categoria TEXT NOT NULL
            );
            CREATE TABLE drivers (id TEXT PRIMARY KEY, nome TEXT NOT NULL);
            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                posicao_largada INTEGER NOT NULL DEFAULT 0,
                posicao_final INTEGER NOT NULL,
                dnf INTEGER NOT NULL DEFAULT 0,
                pontos REAL NOT NULL
            );
            INSERT INTO seasons (id, numero) VALUES ('S1', 1), ('S2', 2);
            INSERT INTO drivers (id, nome) VALUES ('P001', 'Piloto Um'), ('P002', 'Piloto Dois');
            INSERT INTO calendar (id, temporada_id, rodada, categoria) VALUES
                ('R1', 'S1', 1, 'gt4'),
                ('R2', 'S1', 2, 'gt4'),
                ('R3', 'S2', 1, 'gt4');
            ",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_get_last_career_win() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos)
             VALUES ('R2', 'P001', 'T001', 1, 25.0)",
            [],
        )
        .unwrap();

        let result = get_last_career_win(&conn, "P001").unwrap();
        assert_eq!(result, Some((1, 2)));
    }

    #[test]
    fn test_get_last_career_win_none() {
        let conn = setup_db();
        let result = get_last_career_win(&conn, "P001").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_wins_with_team() {
        let conn = setup_db();
        conn.execute_batch(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
             ('R1', 'P001', 'T001', 1, 25.0),
             ('R2', 'P001', 'T001', 1, 25.0),
             ('R3', 'P001', 'T002', 1, 25.0);",
        )
        .unwrap();

        assert_eq!(get_wins_with_team(&conn, "P001", "T001").unwrap(), 2);
        assert_eq!(get_wins_with_team(&conn, "P001", "T002").unwrap(), 1);
        assert_eq!(get_wins_with_team(&conn, "P001", "T003").unwrap(), 0);
    }

    #[test]
    fn test_get_category_standings() {
        let conn = setup_db();
        conn.execute_batch(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
             ('R1', 'P001', 'T001', 1, 25.0),
             ('R1', 'P002', 'T002', 2, 18.0),
             ('R2', 'P001', 'T001', 2, 18.0),
             ('R2', 'P002', 'T002', 1, 25.0);",
        )
        .unwrap();

        let standings = get_category_standings(&conn, "S1", "gt4").unwrap();
        assert_eq!(standings.len(), 2);
        assert_eq!(standings[0].points, 43.0);
        assert_eq!(standings[0].position, 1);
        assert_eq!(standings[1].position, 2);
    }

    #[test]
    fn test_get_category_wins_this_season() {
        let conn = setup_db();
        conn.execute_batch(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
             ('R1', 'P001', 'T001', 1, 25.0),
             ('R2', 'P001', 'T001', 2, 18.0);",
        )
        .unwrap();

        assert_eq!(
            get_category_wins_this_season(&conn, "P001", "S1", "gt4").unwrap(),
            1
        );
        assert_eq!(
            get_category_wins_this_season(&conn, "P001", "S2", "gt4").unwrap(),
            0
        );
    }

    #[test]
    fn test_get_category_standings_breaks_ties_by_name_when_points_and_wins_tie() {
        let conn = setup_db();
        conn.execute_batch(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
             ('R3', 'P001', 'T001', 2, 18.0),
             ('R3', 'P002', 'T002', 2, 18.0);",
        )
        .unwrap();

        let standings = get_category_standings(&conn, "S2", "gt4").unwrap();
        assert_eq!(standings.len(), 2);
        assert_eq!(standings[0].pilot_id, "P002");
        assert_eq!(standings[1].pilot_id, "P001");
    }

    #[test]
    fn test_get_category_leader_before_round_uses_deterministic_tiebreak() {
        let conn = setup_db();
        conn.execute_batch(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
             ('R1', 'P001', 'T001', 1, 25.0),
             ('R1', 'P002', 'T002', 2, 18.0),
             ('R2', 'P001', 'T001', 2, 18.0),
             ('R2', 'P002', 'T002', 1, 25.0);",
        )
        .unwrap();

        let leader = get_category_leader_before_round(&conn, "S1", "gt4", 3).unwrap();
        assert_eq!(leader.as_deref(), Some("P001"));
    }

    #[test]
    fn test_get_results_for_round_propagates_invalid_row_error() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO race_results
                (race_id, piloto_id, equipe_id, posicao_largada, posicao_final, dnf, pontos)
             VALUES ('R1', 'P001', 'T001', 1, 'quebrado', 0, 25.0)",
            [],
        )
        .unwrap();

        let err = get_results_for_round(&conn, "S1", "gt4", 1)
            .expect_err("invalid row should fail instead of being ignored");
        assert!(err.to_string().contains("SQLite error"));
    }
}
