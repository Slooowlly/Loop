//! Log de EPISÓDIOS de rivalidade — a "memória da novela".
//!
//! O motor de rivalidade (`rivalry/mod.rs`) guarda só o ESTADO agregado (intensidade
//! por eixo). Aqui guardamos um registro por corrida em que dois rivais interagiram
//! (capítulo): rodada, pista, tipo de interação, quem levou a melhor, uma frase e a
//! intensidade no momento. Isso permite ao boletim recapitular o arco (origem, número
//! de capítulos, retrospecto direto, revanche, fase). Tabela criada de forma
//! idempotente — não depende do sistema de migrações versionadas.

use rusqlite::{params, Connection};

use crate::db::connection::DbError;

fn ensure_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS rivalry_episodes (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            piloto1_id  TEXT NOT NULL,
            piloto2_id  TEXT NOT NULL,
            temporada   INTEGER NOT NULL,
            rodada      INTEGER NOT NULL,
            ano         INTEGER NOT NULL,
            categoria   TEXT NOT NULL DEFAULT '',
            track_name  TEXT NOT NULL DEFAULT '',
            interaction TEXT NOT NULL DEFAULT '',
            winner_id   TEXT,
            summary     TEXT NOT NULL DEFAULT '',
            perceived   REAL NOT NULL DEFAULT 0.0,
            created_at  TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_rivalry_episodes_pair
            ON rivalry_episodes(piloto1_id, piloto2_id);",
    )?;
    Ok(())
}

/// Um capítulo da rivalidade. `piloto1_id < piloto2_id` (par normalizado).
#[derive(Debug, Clone)]
pub struct RivalryEpisode {
    pub piloto1_id: String,
    pub piloto2_id: String,
    pub temporada: i32,
    pub rodada: i32,
    pub ano: i32,
    pub categoria: String,
    pub track_name: String,
    /// "colisao" | "duelo" | "campeonato"
    pub interaction: String,
    /// Quem levou a melhor (terminou à frente / único a completar). `None` = empate/ambos fora.
    pub winner_id: Option<String>,
    pub summary: String,
    pub perceived: f64,
}

/// Registra um capítulo. O par é normalizado (menor id primeiro) antes de gravar.
pub fn insert_episode(conn: &Connection, ep: &RivalryEpisode) -> Result<(), DbError> {
    ensure_table(conn)?;
    let now = chrono::Local::now().timestamp().to_string();
    let (p1, p2) = if ep.piloto1_id <= ep.piloto2_id {
        (&ep.piloto1_id, &ep.piloto2_id)
    } else {
        (&ep.piloto2_id, &ep.piloto1_id)
    };
    conn.execute(
        "INSERT INTO rivalry_episodes
            (piloto1_id, piloto2_id, temporada, rodada, ano, categoria, track_name,
             interaction, winner_id, summary, perceived, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            p1,
            p2,
            ep.temporada,
            ep.rodada,
            ep.ano,
            ep.categoria,
            ep.track_name,
            ep.interaction,
            ep.winner_id,
            ep.summary,
            ep.perceived,
            now,
        ],
    )?;
    Ok(())
}

/// Todos os capítulos de um par, em ordem cronológica (temporada, rodada).
pub fn get_episodes_for_pair(
    conn: &Connection,
    pilot_a: &str,
    pilot_b: &str,
) -> Result<Vec<RivalryEpisode>, DbError> {
    ensure_table(conn)?;
    let (p1, p2) = if pilot_a <= pilot_b {
        (pilot_a, pilot_b)
    } else {
        (pilot_b, pilot_a)
    };
    let mut stmt = conn.prepare(
        "SELECT piloto1_id, piloto2_id, temporada, rodada, ano, categoria, track_name,
                interaction, winner_id, summary, perceived
         FROM rivalry_episodes
         WHERE piloto1_id = ?1 AND piloto2_id = ?2
         ORDER BY temporada ASC, rodada ASC, id ASC",
    )?;
    let rows = stmt.query_map(params![p1, p2], |r| {
        Ok(RivalryEpisode {
            piloto1_id: r.get(0)?,
            piloto2_id: r.get(1)?,
            temporada: r.get(2)?,
            rodada: r.get(3)?,
            ano: r.get(4)?,
            categoria: r.get(5)?,
            track_name: r.get(6)?,
            interaction: r.get(7)?,
            winner_id: r.get(8)?,
            summary: r.get(9)?,
            perceived: r.get(10)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}
