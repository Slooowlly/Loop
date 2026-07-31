//! Resultados de UMA rodada/etapa: grid, chegadas, abandonos e o bloco do aiseason.

use rusqlite::Connection;

use crate::db::connection::DbError;

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

/// Contagem de DNFs MECÂNICOS (fonte Mechanical/Operational) por EQUIPE numa janela de rodadas
/// da temporada/categoria. Base da "desconfiança mecânica" que faz o time POUPAR as peças (o
/// loop emergente da quebra). Só conta corridas já persistidas (janela = rodadas ANTERIORES à
/// atual). Vazio fora da janela.
pub fn mechanical_dnf_counts_by_team(
    conn: &Connection,
    temporada_id: &str,
    categoria: &str,
    min_round: i32,
    max_round: i32,
) -> Result<std::collections::HashMap<String, u32>, DbError> {
    let mut out = std::collections::HashMap::new();
    if min_round > max_round {
        return Ok(out);
    }
    let mut stmt = conn.prepare(
        "SELECT r.equipe_id, COUNT(*)
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN incident_catalog ic ON r.dnf_catalog_id = ic.id
         WHERE c.temporada_id = ?1 AND c.categoria = ?2
           AND c.rodada BETWEEN ?3 AND ?4
           AND r.dnf = 1
           AND ic.incident_source IN ('Mechanical', 'Operational')
         GROUP BY r.equipe_id",
    )?;
    let mut rows = stmt.query(rusqlite::params![
        temporada_id,
        categoria,
        min_round,
        max_round
    ])?;
    while let Some(row) = rows.next()? {
        let team: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        if !team.is_empty() && count > 0 {
            out.insert(team, count as u32);
        }
    }
    Ok(out)
}

/// Uma linha de resultado de etapa (com dados do piloto) — p/ exportar ao iRacing.
pub struct EventResultRow {
    pub piloto_id: String,
    pub nome: String,
    pub is_jogador: bool,
    pub finish: i64,
    pub start: i64,
    pub laps: i64,
    pub total_ms: f64,
    pub gap_ms: f64,
    pub incidents: i64,
    pub dnf: bool,
    pub has_fastest: bool,
    pub dnf_reason: Option<String>,
}

/// Resultados de UMA corrida (todos os pilotos + dados pro bloco do aiseason).
pub fn get_event_results(conn: &Connection, race_id: &str) -> Result<Vec<EventResultRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT rr.piloto_id, d.nome, d.is_jogador, rr.posicao_final, rr.posicao_largada,
                rr.voltas_completadas, rr.tempo_total, rr.gap_to_winner_ms, rr.incidents_count,
                rr.dnf, rr.fastest_lap, rr.dnf_reason
         FROM race_results rr
         JOIN drivers d ON rr.piloto_id = d.id
         WHERE rr.race_id = ?1
         ORDER BY rr.posicao_final",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![race_id], |row| {
            Ok(EventResultRow {
                piloto_id: row.get(0)?,
                nome: row.get(1)?,
                is_jogador: row.get::<_, i32>(2)? != 0,
                finish: row.get(3)?,
                start: row.get(4)?,
                laps: row.get(5)?,
                total_ms: row.get::<_, f64>(6)?,
                gap_ms: row.get::<_, f64>(7)?,
                incidents: row.get(8)?,
                dnf: row.get::<_, i32>(9)? != 0,
                has_fastest: row.get::<_, f64>(10)? != 0.0,
                dnf_reason: row.get(11)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
