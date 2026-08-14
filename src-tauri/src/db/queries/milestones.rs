//! Marcos históricos da categoria com MEMÓRIA TEMPORAL — "o recorde de vitórias caiu
//! na temporada X". Os recordes em si são derivados de `race_results` a cada consulta
//! (`race_history::get_category_records`), mas isso não guarda QUANDO um recorde foi
//! batido. Esta tabela registra esse instante, para notícias de "recorde quebrado" com
//! data e para o rodapé do mundo. As três tabelas nasceram fora das migrações e entraram
//! nelas na v62; o `ensure_table` reaplica o MESMO DDL, de forma idempotente, para
//! conexões de teste in-memory que não migram.

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;

/// DDL dos marcos, num lugar só — a migração v62 executa esta MESMA constante.
pub(crate) const DDL_RECORD_MILESTONES: &str = "
    CREATE TABLE IF NOT EXISTS record_milestones (
        id              TEXT PRIMARY KEY,
        categoria       TEXT NOT NULL,
        metric          TEXT NOT NULL,
        pilot_id        TEXT NOT NULL,
        pilot_name      TEXT NOT NULL,
        value           INTEGER NOT NULL,
        previous_value  INTEGER,
        context         TEXT NOT NULL DEFAULT '',
        season_number   INTEGER NOT NULL,
        ano             INTEGER NOT NULL,
        round           INTEGER NOT NULL,
        created_at      TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX IF NOT EXISTS idx_record_milestones_cat
        ON record_milestones(categoria, season_number DESC);
";

/// Fonte da verdade do RECORDE DE VOLTA por (categoria, pista): o tempo de volta NÃO é
/// persistido no histórico (`race_results.fastest_lap` é só um booleano), então guardamos o
/// recorde aqui, atualizado a cada corrida. O marco (em `record_milestones`) só é emitido
/// quando um recorde EXISTENTE é superado — o inaugural fica guardado aqui em silêncio.
pub(crate) const DDL_TRACK_LAP_RECORDS: &str = "
    CREATE TABLE IF NOT EXISTS track_lap_records (
        categoria     TEXT NOT NULL,
        track_name    TEXT NOT NULL,
        pilot_id      TEXT NOT NULL,
        pilot_name    TEXT NOT NULL,
        lap_ms        INTEGER NOT NULL,
        season_number INTEGER NOT NULL,
        round         INTEGER NOT NULL,
        PRIMARY KEY (categoria, track_name)
    );
";

/// Reaplica o DDL para conexões de teste in-memory que não migram. Só isso: schema
/// permanente é assunto das migrações, e o `ALTER` de `context` (tabela criada antes da
/// coluna) mora na v62, que é o lado que existe para consertar save em campo. Aqui ele era
/// no-op: a constante acima já declara a coluna.
fn ensure_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(DDL_RECORD_MILESTONES)?;
    conn.execute_batch(DDL_TRACK_LAP_RECORDS)?;
    Ok(())
}

/// Armazém genérico de RECORDES ESCALARES por (categoria, tipo): um único valor
/// "campeão" que vive aqui e é atualizado incrementalmente. Serve para recordes que não
/// são reconstruíveis de `race_results` de forma barata (idade no evento, gap do
/// campeonato, dupla mais longeva, etc.) ou cujo "atual" precisa ser lembrado.
pub(crate) const DDL_CATEGORY_SCALAR_RECORDS: &str = "
    CREATE TABLE IF NOT EXISTS category_scalar_records (
        categoria     TEXT NOT NULL,
        kind          TEXT NOT NULL,
        subject_id    TEXT NOT NULL,
        subject_name  TEXT NOT NULL,
        value         INTEGER NOT NULL,
        context       TEXT NOT NULL DEFAULT '',
        season_number INTEGER NOT NULL,
        round         INTEGER NOT NULL,
        PRIMARY KEY (categoria, kind)
    );
";

fn ensure_scalar_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(DDL_CATEGORY_SCALAR_RECORDS)?;
    Ok(())
}

/// Recorde escalar atual: `(subject_id, subject_name, value, context)`.
pub fn get_scalar_record(
    conn: &Connection,
    categoria: &str,
    kind: &str,
) -> Result<Option<(String, String, i32, String)>, DbError> {
    ensure_scalar_table(conn)?;
    let row = conn
        .query_row(
            "SELECT subject_id, subject_name, value, context FROM category_scalar_records
             WHERE categoria = ?1 AND kind = ?2",
            params![categoria, kind],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    Ok(row)
}

#[allow(clippy::too_many_arguments)]
fn upsert_scalar_record(
    conn: &Connection,
    categoria: &str,
    kind: &str,
    subject_id: &str,
    subject_name: &str,
    value: i32,
    context: &str,
    season_number: i32,
    round: i32,
) -> Result<(), DbError> {
    ensure_scalar_table(conn)?;
    conn.execute(
        "INSERT INTO category_scalar_records
            (categoria, kind, subject_id, subject_name, value, context, season_number, round)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(categoria, kind) DO UPDATE SET
            subject_id = excluded.subject_id,
            subject_name = excluded.subject_name,
            value = excluded.value,
            context = excluded.context,
            season_number = excluded.season_number,
            round = excluded.round",
        params![
            categoria,
            kind,
            subject_id,
            subject_name,
            value,
            context,
            season_number,
            round
        ],
    )?;
    Ok(())
}

/// Atualiza um recorde escalar com um candidato e devolve `Some(previous_value)` SE ele
/// SUPEROU um recorde existente (→ o chamador emite o marco). `higher_is_better = true`
/// para máximos (mais velho, mais caótico); `false` para mínimos (mais jovem, menor gap).
/// Inaugural (sem recorde antes) grava em silêncio e devolve `None` — não é "quebra".
#[allow(clippy::too_many_arguments)]
pub fn update_scalar_and_check(
    conn: &Connection,
    categoria: &str,
    kind: &str,
    subject_id: &str,
    subject_name: &str,
    value: i32,
    context: &str,
    season_number: i32,
    round: i32,
    higher_is_better: bool,
) -> Result<Option<i32>, DbError> {
    let prev = get_scalar_record(conn, categoria, kind)?;
    let beats = match &prev {
        Some((_, _, pv, _)) => {
            if higher_is_better {
                value > *pv
            } else {
                value < *pv
            }
        }
        None => true,
    };
    if beats {
        upsert_scalar_record(
            conn,
            categoria,
            kind,
            subject_id,
            subject_name,
            value,
            context,
            season_number,
            round,
        )?;
        if let Some((_, _, pv, _)) = prev {
            return Ok(Some(pv));
        }
    }
    Ok(None)
}

/// Recordes CUMULATIVOS cujo dono muda com o tempo (ex.: "mais largadas sem vencer").
/// Só anuncia quando a COROA TROCA DE DONO (evita spam a cada corrida do mesmo líder) e
/// o valor atinge o piso. Devolve `Some((nome_do_dono_anterior, valor_anterior))` quando
/// há troca a anunciar. Inaugural grava baseline em silêncio (nada retroativo).
#[allow(clippy::too_many_arguments)]
pub fn update_leader_and_check_crown(
    conn: &Connection,
    categoria: &str,
    kind: &str,
    leader_id: &str,
    leader_name: &str,
    value: i32,
    floor: i32,
    season_number: i32,
    round: i32,
) -> Result<Option<(String, i32)>, DbError> {
    let prev = get_scalar_record(conn, categoria, kind)?;
    match prev {
        None => {
            if value >= floor {
                upsert_scalar_record(
                    conn,
                    categoria,
                    kind,
                    leader_id,
                    leader_name,
                    value,
                    "",
                    season_number,
                    round,
                )?;
            }
            Ok(None)
        }
        Some((pid, pname, pv, _)) => {
            if leader_id == pid {
                if value != pv {
                    upsert_scalar_record(
                        conn,
                        categoria,
                        kind,
                        leader_id,
                        leader_name,
                        value,
                        "",
                        season_number,
                        round,
                    )?;
                }
                Ok(None)
            } else if value >= floor {
                upsert_scalar_record(
                    conn,
                    categoria,
                    kind,
                    leader_id,
                    leader_name,
                    value,
                    "",
                    season_number,
                    round,
                )?;
                Ok(Some((pname, pv)))
            } else {
                Ok(None)
            }
        }
    }
}

/// Recorde de volta atual de uma pista na categoria: `(pilot_id, pilot_name, lap_ms)`.
pub fn get_track_lap_record(
    conn: &Connection,
    categoria: &str,
    track_name: &str,
) -> Result<Option<(String, String, i32)>, DbError> {
    ensure_table(conn)?;
    let row = conn
        .query_row(
            "SELECT pilot_id, pilot_name, lap_ms FROM track_lap_records
             WHERE categoria = ?1 AND track_name = ?2",
            params![categoria, track_name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    Ok(row)
}

/// Grava/atualiza o recorde de volta de uma pista (o chamador só chama quando é mais
/// rápido que o atual, ou quando não há recorde).
#[allow(clippy::too_many_arguments)]
pub fn upsert_track_lap_record(
    conn: &Connection,
    categoria: &str,
    track_name: &str,
    pilot_id: &str,
    pilot_name: &str,
    lap_ms: i32,
    season_number: i32,
    round: i32,
) -> Result<(), DbError> {
    ensure_table(conn)?;
    conn.execute(
        "INSERT INTO track_lap_records
            (categoria, track_name, pilot_id, pilot_name, lap_ms, season_number, round)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(categoria, track_name) DO UPDATE SET
            pilot_id = excluded.pilot_id,
            pilot_name = excluded.pilot_name,
            lap_ms = excluded.lap_ms,
            season_number = excluded.season_number,
            round = excluded.round",
        params![
            categoria,
            track_name,
            pilot_id,
            pilot_name,
            lap_ms,
            season_number,
            round
        ],
    )?;
    Ok(())
}

/// Um marco: um recorde histórico da categoria batido numa etapa específica.
#[derive(Debug, Clone)]
pub struct RecordMilestone {
    pub metric: String,
    pub pilot_id: String,
    pub pilot_name: String,
    pub value: i32,
    pub previous_value: Option<i32>,
    /// Dimensão extra que qualifica o recorde e o torna único (ex.: nome da pista no
    /// recorde de volta mais rápida). Vazio para recordes globais da categoria.
    pub context: String,
    pub season_number: i32,
    pub ano: i32,
    pub round: i32,
}

/// Registra um marco. Id determinístico (`categoria+metric+value`) → reprocessar a
/// mesma etapa não duplica (INSERT OR IGNORE). Camada narrativa: nunca deve quebrar o
/// fluxo de corrida — o chamador ignora o erro.
pub fn insert_milestone(
    conn: &Connection,
    categoria: &str,
    m: &RecordMilestone,
) -> Result<(), DbError> {
    ensure_table(conn)?;
    // Id determinístico. Inclui o `context` (quando houver) para que recordes de
    // dimensões diferentes (ex.: pistas diferentes) não colidam no mesmo valor.
    let id = if m.context.is_empty() {
        format!("REC-{categoria}-{}-{}", m.metric, m.value)
    } else {
        let ctx = m
            .context
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect::<String>();
        format!("REC-{categoria}-{}-{ctx}-{}", m.metric, m.value)
    };
    let now = chrono::Local::now().timestamp().to_string();
    conn.execute(
        "INSERT OR IGNORE INTO record_milestones
            (id, categoria, metric, pilot_id, pilot_name, value, previous_value,
             context, season_number, ano, round, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            id,
            categoria,
            m.metric,
            m.pilot_id,
            m.pilot_name,
            m.value,
            m.previous_value,
            m.context,
            m.season_number,
            m.ano,
            m.round,
            now,
        ],
    )?;
    Ok(())
}

/// Maior valor já registrado de uma métrica na categoria (ou `None` se não há marco).
/// Usado por recordes cujo "atual" vive nos próprios marcos (ex.: sequência de vitórias),
/// para só anunciar quando a marca é superada.
pub fn get_max_milestone_value(
    conn: &Connection,
    categoria: &str,
    metric: &str,
) -> Result<Option<i32>, DbError> {
    ensure_table(conn)?;
    let v: Option<i32> = conn.query_row(
        "SELECT MAX(value) FROM record_milestones WHERE categoria = ?1 AND metric = ?2",
        params![categoria, metric],
        |r| r.get(0),
    )?;
    Ok(v)
}

/// Marcos mais recentes de uma categoria (mais novo primeiro). Alimenta notícias de
/// "recorde quebrado" com data e o rodapé do mundo.
pub fn get_recent_milestones(
    conn: &Connection,
    categoria: &str,
    limit: i32,
) -> Result<Vec<RecordMilestone>, DbError> {
    ensure_table(conn)?;
    let mut stmt = conn.prepare(
        "SELECT metric, pilot_id, pilot_name, value, previous_value, context,
                season_number, ano, round
         FROM record_milestones
         WHERE categoria = ?1
         ORDER BY season_number DESC, round DESC, id DESC
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![categoria, limit], |r| {
            Ok(RecordMilestone {
                metric: r.get(0)?,
                pilot_id: r.get(1)?,
                pilot_name: r.get(2)?,
                value: r.get(3)?,
                previous_value: r.get(4)?,
                context: r.get(5)?,
                season_number: r.get(6)?,
                ano: r.get(7)?,
                round: r.get(8)?,
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

    fn sample(metric: &str, value: i32, season: i32, round: i32) -> RecordMilestone {
        RecordMilestone {
            metric: metric.to_string(),
            pilot_id: "P1".to_string(),
            pilot_name: "Fulano".to_string(),
            value,
            previous_value: Some(value - 1),
            context: String::new(),
            season_number: season,
            ano: 2040 + season,
            round,
        }
    }

    #[test]
    fn test_insert_is_idempotent_by_category_metric_value() {
        let c = conn();
        insert_milestone(&c, "gt3", &sample("wins", 14, 12, 3)).unwrap();
        // Reprocessar a mesma etapa (mesmo recorde) não duplica.
        insert_milestone(&c, "gt3", &sample("wins", 14, 12, 3)).unwrap();
        let got = get_recent_milestones(&c, "gt3", 10).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].value, 14);
        assert_eq!(got[0].previous_value, Some(13));
    }

    #[test]
    fn test_context_disambiguates_same_value() {
        let c = conn();
        let mk = |track: &str| RecordMilestone {
            metric: "lap_record".to_string(),
            pilot_id: "P1".to_string(),
            pilot_name: "Fulano".to_string(),
            value: 90_000,
            previous_value: Some(91_000),
            context: track.to_string(),
            season_number: 5,
            ano: 2045,
            round: 2,
        };
        insert_milestone(&c, "gt3", &mk("Monza")).unwrap();
        insert_milestone(&c, "gt3", &mk("Spa")).unwrap();
        // Mesmo valor (ms), pistas diferentes → contexto no id evita colisão.
        assert_eq!(get_recent_milestones(&c, "gt3", 10).unwrap().len(), 2);
    }

    #[test]
    fn test_update_scalar_min_and_max() {
        let c = conn();
        // Inaugural grava em silêncio (None) — nada retroativo.
        assert_eq!(
            update_scalar_and_check(&c, "gt3", "youngest_winner", "P1", "A", 24, "", 5, 2, false)
                .unwrap(),
            None
        );
        // 22 < 24 → supera (mínimo) e devolve o valor anterior.
        assert_eq!(
            update_scalar_and_check(&c, "gt3", "youngest_winner", "P2", "B", 22, "", 6, 1, false)
                .unwrap(),
            Some(24)
        );
        // 25 não é mais jovem que 22 → não supera.
        assert_eq!(
            update_scalar_and_check(&c, "gt3", "youngest_winner", "P3", "C", 25, "", 7, 1, false)
                .unwrap(),
            None
        );
        // O recorde guardado agora é o de P2 (22).
        assert_eq!(
            get_scalar_record(&c, "gt3", "youngest_winner")
                .unwrap()
                .unwrap()
                .2,
            22
        );
    }

    #[test]
    fn test_update_leader_crown_only_on_change() {
        let c = conn();
        // Inaugural: baseline silencioso.
        assert_eq!(
            update_leader_and_check_crown(&c, "gt3", "most_starts_no_win", "P1", "A", 40, 30, 5, 1)
                .unwrap(),
            None
        );
        // Mesmo dono, valor sobe → não anuncia.
        assert_eq!(
            update_leader_and_check_crown(&c, "gt3", "most_starts_no_win", "P1", "A", 41, 30, 6, 1)
                .unwrap(),
            None
        );
        // Troca de dono acima do piso → anuncia com o dono anterior.
        assert_eq!(
            update_leader_and_check_crown(&c, "gt3", "most_starts_no_win", "P2", "B", 42, 30, 7, 1)
                .unwrap(),
            Some(("A".to_string(), 41))
        );
    }

    #[test]
    fn test_get_max_milestone_value() {
        let c = conn();
        assert_eq!(
            get_max_milestone_value(&c, "gt3", "win_streak").unwrap(),
            None
        );
        insert_milestone(&c, "gt3", &sample("win_streak", 4, 5, 2)).unwrap();
        insert_milestone(&c, "gt3", &sample("win_streak", 6, 6, 1)).unwrap();
        insert_milestone(&c, "gt3", &sample("wins", 12, 6, 1)).unwrap();
        // Maior valor de win_streak = 6; métrica diferente não interfere.
        assert_eq!(
            get_max_milestone_value(&c, "gt3", "win_streak").unwrap(),
            Some(6)
        );
    }

    #[test]
    fn test_track_lap_record_upsert_and_get() {
        let c = conn();
        assert!(get_track_lap_record(&c, "gt3", "Monza").unwrap().is_none());
        upsert_track_lap_record(&c, "gt3", "Monza", "P1", "Fulano", 90_000, 5, 2).unwrap();
        let r = get_track_lap_record(&c, "gt3", "Monza").unwrap().unwrap();
        assert_eq!(r.0, "P1");
        assert_eq!(r.2, 90_000);
        // Nova volta mais rápida (mesma pista) sobrescreve o recorde.
        upsert_track_lap_record(&c, "gt3", "Monza", "P2", "Beltrano", 89_500, 6, 1).unwrap();
        let r2 = get_track_lap_record(&c, "gt3", "Monza").unwrap().unwrap();
        assert_eq!(r2.0, "P2");
        assert_eq!(r2.2, 89_500);
    }

    #[test]
    fn test_recent_orders_by_season_then_round_and_scopes_category() {
        let c = conn();
        insert_milestone(&c, "gt3", &sample("wins", 10, 8, 2)).unwrap();
        insert_milestone(&c, "gt3", &sample("wins", 14, 12, 5)).unwrap();
        insert_milestone(&c, "mazda", &sample("wins", 9, 20, 1)).unwrap();
        let gt3 = get_recent_milestones(&c, "gt3", 10).unwrap();
        assert_eq!(gt3.len(), 2, "não vaza a categoria mazda");
        assert_eq!(gt3[0].value, 14, "temporada 12 vem antes da 8");
        assert_eq!(gt3[1].value, 10);
    }
}
