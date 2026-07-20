//! Persistência dos DESFECHOS DE QUEBRA por corrida (Peça 3 do Sistema de Quebra).
//!
//! Uma linha por peça que largou numa corrida DIRIGIDA no iRacing (o disparo ao vivo
//! `!black`/`!dq`), por piloto. Só enche na corrida ao vivo — a simulação offline não dispara
//! quebra. Alimenta o debrief (jogador), a "Telemetria" (grade toda) e a notícia. A tabela é
//! criada pela migration v52; `ensure_table` reaplica de forma idempotente para conexões de
//! teste in-memory que não rodam migrações.

use rusqlite::{params, Connection};

use crate::db::connection::DbError;

/// Uma quebra registrada de um piloto numa corrida (já resolvido para `driver_id`).
#[derive(Clone, Debug, PartialEq)]
pub struct RaceBreakdownRow {
    pub driver_id: String,
    /// Chave da peça (`PartType::as_str`).
    pub part: String,
    /// Modo de falha concreto (0..3) — o "qual problema".
    pub problem: u8,
    pub lap: u32,
    /// "light" | "heavy" | "dnf".
    pub severity: String,
    /// Segundos de penalidade (`!black`); `None` = DNF (`!dq`).
    pub penalty_secs: Option<u32>,
    /// `true` se a peça bateu na parede (105%).
    pub forced: bool,
    /// Frase pronta do problema (peça + modo + severidade) pra narrativa.
    pub label: String,
}

fn ensure_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS race_breakdowns (
            race_id      TEXT NOT NULL,
            driver_id    TEXT NOT NULL,
            part         TEXT NOT NULL,
            problem      INTEGER NOT NULL DEFAULT 0,
            lap          INTEGER NOT NULL,
            severity     TEXT NOT NULL,
            penalty_secs INTEGER,
            forced       INTEGER NOT NULL DEFAULT 0,
            label        TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (race_id, driver_id, part, lap)
        );
        CREATE INDEX IF NOT EXISTS idx_race_breakdowns_race ON race_breakdowns(race_id);",
    )?;
    Ok(())
}

/// Grava os desfechos de quebra de uma corrida (idempotente por `(race_id, driver_id, part,
/// lap)` via `INSERT OR REPLACE`). No-op se `rows` vazio.
pub fn insert_breakdowns_batch(
    conn: &Connection,
    race_id: &str,
    rows: &[RaceBreakdownRow],
) -> Result<(), DbError> {
    ensure_table(conn)?;
    if rows.is_empty() {
        return Ok(());
    }
    let mut stmt = conn.prepare(
        "INSERT OR REPLACE INTO race_breakdowns
            (race_id, driver_id, part, problem, lap, severity, penalty_secs, forced, label)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    for r in rows {
        stmt.execute(params![
            race_id,
            r.driver_id,
            r.part,
            r.problem as i64,
            r.lap as i64,
            r.severity,
            r.penalty_secs.map(|s| s as i64),
            r.forced as i64,
            r.label,
        ])?;
    }
    Ok(())
}

/// Lê todos os desfechos de quebra de uma corrida, em ordem de volta.
pub fn get_breakdowns_for_race(
    conn: &Connection,
    race_id: &str,
) -> Result<Vec<RaceBreakdownRow>, DbError> {
    ensure_table(conn)?;
    let mut stmt = conn.prepare(
        "SELECT driver_id, part, problem, lap, severity, penalty_secs, forced, label
           FROM race_breakdowns WHERE race_id = ?1 ORDER BY lap ASC, driver_id ASC",
    )?;
    let rows = stmt
        .query_map(params![race_id], |r| {
            let problem: i64 = r.get(2)?;
            let lap: i64 = r.get(3)?;
            let penalty: Option<i64> = r.get(5)?;
            let forced: i64 = r.get(6)?;
            Ok(RaceBreakdownRow {
                driver_id: r.get(0)?,
                part: r.get(1)?,
                problem: problem.clamp(0, 255) as u8,
                lap: lap.max(0) as u32,
                severity: r.get(4)?,
                penalty_secs: penalty.map(|s| s.max(0) as u32),
                forced: forced != 0,
                label: r.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    fn row(driver: &str, part: &str, lap: u32, sev: &str, pen: Option<u32>) -> RaceBreakdownRow {
        RaceBreakdownRow {
            driver_id: driver.to_string(),
            part: part.to_string(),
            problem: 0,
            lap,
            severity: sev.to_string(),
            penalty_secs: pen,
            forced: false,
            label: format!("{part} deu problema"),
        }
    }

    #[test]
    fn grava_e_le_os_desfechos_por_corrida() {
        let db = conn();
        let rows = vec![
            row("d1", "engine", 14, "dnf", None),
            row("d2", "brakes", 8, "heavy", Some(9)),
        ];
        insert_breakdowns_batch(&db, "race-1", &rows).unwrap();

        let got = get_breakdowns_for_race(&db, "race-1").unwrap();
        assert_eq!(got.len(), 2);
        // Ordenado por volta: freios (8) antes do motor (14).
        assert_eq!(got[0].part, "brakes");
        assert_eq!(got[0].penalty_secs, Some(9));
        assert_eq!(got[1].part, "engine");
        assert_eq!(got[1].penalty_secs, None);
    }

    #[test]
    fn insert_vazio_e_noop_e_corrida_sem_quebra_le_vazio() {
        let db = conn();
        insert_breakdowns_batch(&db, "race-x", &[]).unwrap();
        assert!(get_breakdowns_for_race(&db, "race-x").unwrap().is_empty());
    }

    #[test]
    fn reimportar_a_mesma_corrida_substitui_sem_duplicar() {
        let db = conn();
        let rows = vec![row("d1", "engine", 14, "dnf", None)];
        insert_breakdowns_batch(&db, "race-1", &rows).unwrap();
        insert_breakdowns_batch(&db, "race-1", &rows).unwrap();
        assert_eq!(get_breakdowns_for_race(&db, "race-1").unwrap().len(), 1);
    }
}
