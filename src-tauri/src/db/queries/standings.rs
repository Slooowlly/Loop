use rusqlite::{params, Connection, Result as DbResult};

#[derive(Debug)]
pub struct ChampionshipContext {
    pub player_position: i32,
    pub gap_to_leader: i32,
}

/// Retorna posição e gap do jogador na categoria do evento usando a temporada ativa
/// e os resultados oficiais já persistidos.
pub fn get_championship_context(
    conn: &Connection,
    race_categoria: &str,
) -> DbResult<ChampionshipContext> {
    let mut stmt = conn.prepare(
        "SELECT
            d.id,
            COALESCE(SUM(r.pontos), 0.0) AS total_points,
            SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS total_wins,
            SUM(CASE WHEN r.posicao_final <= 3 THEN 1 ELSE 0 END) AS total_podiums,
            SUM(CASE
                    WHEN r.id IS NOT NULL
                     AND typeof(r.pontos) NOT IN ('real', 'integer')
                    THEN 1
                    ELSE 0
                END) AS invalid_points_rows,
            d.is_jogador
         FROM drivers d
         JOIN seasons s
           ON s.status = 'EmAndamento'
         LEFT JOIN calendar c
           ON c.temporada_id = s.id
          AND c.categoria = ?1
         LEFT JOIN race_results r
           ON r.race_id = c.id
          AND r.piloto_id = d.id
         WHERE d.categoria_atual = ?1
           AND d.status != 'Aposentado'
         GROUP BY d.id, d.is_jogador
         ORDER BY total_points DESC, total_wins DESC, total_podiums DESC, d.id ASC",
    )?;

    struct Row {
        pontos: f64,
        is_jogador: bool,
        invalid_points_rows: i32,
    }

    let mapped = stmt.query_map(params![race_categoria], |row| {
        Ok(Row {
            pontos: row.get(1)?,
            invalid_points_rows: row.get(4)?,
            is_jogador: row.get::<_, i32>(5)? != 0,
        })
    })?;

    let mut rows = Vec::new();
    for row in mapped {
        let row = row?;
        if row.invalid_points_rows > 0 {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "race_results.pontos invalido no standing",
                )),
            ));
        }
        rows.push(row);
    }

    let leader_points = rows.first().map(|row| row.pontos).unwrap_or(0.0);
    let player_idx = rows.iter().position(|row| row.is_jogador);
    let player_position = player_idx.map(|idx| idx as i32 + 1).unwrap_or(0);
    let player_points = player_idx
        .and_then(|idx| rows.get(idx))
        .map(|row| row.pontos)
        .unwrap_or(0.0);
    let gap = if leader_points > player_points {
        (leader_points - player_points).round() as i32
    } else {
        0
    };

    Ok(ChampionshipContext {
        player_position,
        gap_to_leader: gap,
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::get_championship_context;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE seasons (
                id TEXT PRIMARY KEY,
                numero INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                status TEXT NOT NULL
            );
            CREATE TABLE drivers (
                id TEXT PRIMARY KEY,
                categoria_atual TEXT,
                status TEXT NOT NULL,
                is_jogador INTEGER NOT NULL
            );
            CREATE TABLE calendar (
                id TEXT PRIMARY KEY,
                temporada_id TEXT NOT NULL,
                categoria TEXT NOT NULL
            );
            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                posicao_final INTEGER NOT NULL,
                pontos REAL NOT NULL
            );",
        )
        .expect("schema");
        conn
    }

    #[test]
    fn test_get_championship_context_uses_active_season_results() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO seasons (id, numero, ano, status) VALUES ('S1', 1, 2025, 'EmAndamento')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO drivers (id, categoria_atual, status, is_jogador) VALUES
             ('P1', 'gt4', 'Ativo', 1),
             ('P2', 'gt4', 'Ativo', 0),
             ('P3', 'gt4', 'Ativo', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO calendar (id, temporada_id, categoria) VALUES
             ('R1', 'S1', 'gt4'),
             ('R2', 'S1', 'gt4')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO race_results (race_id, piloto_id, posicao_final, pontos) VALUES
             ('R1', 'P2', 1, 40.0),
             ('R1', 'P1', 2, 35.0),
             ('R2', 'P2', 2, 40.0),
             ('R2', 'P1', 1, 40.0),
             ('R2', 'P3', 3, 30.0)",
            [],
        )
        .unwrap();

        let champ = get_championship_context(&conn, "gt4").unwrap();

        assert_eq!(champ.player_position, 2);
        assert_eq!(champ.gap_to_leader, 5);
    }

    #[test]
    fn test_get_championship_context_returns_zeroes_without_active_season() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO drivers (id, categoria_atual, status, is_jogador) VALUES
             ('P1', 'gt4', 'Ativo', 1)",
            [],
        )
        .unwrap();

        let champ = get_championship_context(&conn, "gt4").unwrap();

        assert_eq!(champ.player_position, 0);
        assert_eq!(champ.gap_to_leader, 0);
    }

    #[test]
    fn test_get_championship_context_propagates_invalid_row_error() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO seasons (id, numero, ano, status) VALUES ('S1', 1, 2025, 'EmAndamento')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO drivers (id, categoria_atual, status, is_jogador) VALUES
             ('P1', 'gt4', 'Ativo', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO calendar (id, temporada_id, categoria) VALUES ('R1', 'S1', 'gt4')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO race_results (race_id, piloto_id, posicao_final, pontos)
             VALUES ('R1', 'P1', 1, 'quebrado')",
            [],
        )
        .unwrap();

        let err = get_championship_context(&conn, "gt4").expect_err("invalid row should fail");
        assert!(err.to_string().contains("race_results.pontos invalido"));
    }
}
