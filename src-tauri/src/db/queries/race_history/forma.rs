//! Forma recente e confronto direto: últimas chegadas, corridas e rivalidades.

use rusqlite::{Connection, OptionalExtension};

use crate::db::connection::DbError;

/// Uma chegada recente do piloto (mais recente primeiro) — base para leitura de
/// forma e para a média que alimenta o mérito da posição esperada.
#[derive(Debug, Clone)]
pub struct RecentFinish {
    pub season_num: i32,
    pub round: i32,
    pub finish: i32,
    pub is_dnf: bool,
}

/// Últimas `limit` chegadas do piloto na categoria ANTERIORES à (temporada, rodada)
/// dada — EXCLUI a corrida atual (que já está persistida). Mais recente primeiro.
pub fn get_recent_finishes_before(
    conn: &Connection,
    pilot_id: &str,
    categoria: &str,
    before_season_num: i32,
    before_round: i32,
    limit: i32,
) -> Result<Vec<RecentFinish>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT s.numero, c.rodada, r.posicao_final, r.dnf
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN seasons s ON c.temporada_id = s.id
         WHERE r.piloto_id = ?1 AND c.categoria = ?2
           AND (s.numero < ?3 OR (s.numero = ?3 AND c.rodada < ?4))
         ORDER BY s.numero DESC, c.rodada DESC
         LIMIT ?5",
    )?;
    let rows = stmt
        .query_map(
            rusqlite::params![pilot_id, categoria, before_season_num, before_round, limit],
            |row| {
                Ok(RecentFinish {
                    season_num: row.get(0)?,
                    round: row.get(1)?,
                    finish: row.get(2)?,
                    is_dnf: row.get::<_, i32>(3)? != 0,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Uma corrida recente do piloto, com a CHAVE da etapa (`race_id` = `calendar.id`)
/// e o nome da pista — para reencontrar os artefatos de IA (debrief) daquela etapa.
/// Diferente de `RecentFinish`, que só carrega forma (temporada/rodada/chegada).
#[derive(Debug, Clone)]
pub struct RecentRace {
    pub season_num: i32,
    pub round: i32,
    pub race_id: String,
    pub finish: i32,
    pub is_dnf: bool,
    pub track_name: String,
}

/// Últimas `limit` corridas do piloto na categoria ANTERIORES à (temporada, rodada)
/// dada — EXCLUI a corrida atual. Mais recente primeiro. Igual a
/// `get_recent_finishes_before`, mas devolve também `race_id` e pista, para o arco
/// narrativo pré-corrida reler o debrief de cada etapa.
pub fn get_recent_races_before(
    conn: &Connection,
    pilot_id: &str,
    categoria: &str,
    before_season_num: i32,
    before_round: i32,
    limit: i32,
) -> Result<Vec<RecentRace>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT s.numero, c.rodada, c.id, r.posicao_final, r.dnf, c.track_name
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN seasons s ON c.temporada_id = s.id
         WHERE r.piloto_id = ?1 AND c.categoria = ?2
           AND (s.numero < ?3 OR (s.numero = ?3 AND c.rodada < ?4))
         ORDER BY s.numero DESC, c.rodada DESC
         LIMIT ?5",
    )?;
    let rows = stmt
        .query_map(
            rusqlite::params![pilot_id, categoria, before_season_num, before_round, limit],
            |row| {
                Ok(RecentRace {
                    season_num: row.get(0)?,
                    round: row.get(1)?,
                    race_id: row.get(2)?,
                    finish: row.get(3)?,
                    is_dnf: row.get::<_, i32>(4)? != 0,
                    track_name: row.get(5)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Resultado do piloto por rodada na temporada/categoria — (rodada, chegada, dnf).
/// Usado para o confronto interno entre companheiros de equipe ao longo do ano.
pub fn get_pilot_season_results(
    conn: &Connection,
    pilot_id: &str,
    temporada_id: &str,
    categoria: &str,
) -> Result<Vec<(i32, i32, bool)>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT c.rodada, r.posicao_final, r.dnf
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE r.piloto_id = ?1 AND c.temporada_id = ?2 AND c.categoria = ?3
         ORDER BY c.rodada ASC",
    )?;
    let rows = stmt
        .query_map(
            rusqlite::params![pilot_id, temporada_id, categoria],
            |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i32>(2)? != 0)),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Confronto direto entre o jogador e um rival na categoria (todas as temporadas
/// persistidas). `races_together` = corridas em que ambos aparecem no resultado;
/// `player_ahead` = quantas o jogador terminou à frente. `best_*` = a corrida em
/// que o jogador teve sua MELHOR posição entre as que bateu o rival (para citar).
#[derive(Debug, Clone, Default)]
pub struct HeadToHead {
    pub races_together: i32,
    pub player_ahead: i32,
    pub best_finish: Option<i32>,
    pub best_track: Option<String>,
}

pub fn get_head_to_head(
    conn: &Connection,
    player_id: &str,
    rival_id: &str,
    categoria: &str,
) -> Result<HeadToHead, DbError> {
    let mut stmt = conn.prepare(
        "SELECT pr.posicao_final, rr.posicao_final,
                COALESCE(NULLIF(c.track_name, ''), c.pista)
         FROM race_results pr
         JOIN race_results rr ON rr.race_id = pr.race_id AND rr.piloto_id = ?2
         JOIN calendar c ON c.id = pr.race_id
         WHERE pr.piloto_id = ?1 AND c.categoria = ?3",
    )?;
    let mut rows = stmt.query(rusqlite::params![player_id, rival_id, categoria])?;
    let mut h = HeadToHead::default();
    while let Some(row) = rows.next()? {
        let player_pos: i32 = row.get(0)?;
        let rival_pos: i32 = row.get(1)?;
        let track: String = row.get(2)?;
        h.races_together += 1;
        if player_pos < rival_pos {
            h.player_ahead += 1;
            if h.best_finish.map_or(true, |b| player_pos < b) {
                h.best_finish = Some(player_pos);
                h.best_track = Some(track);
            }
        }
    }
    Ok(h)
}

/// Piloto ATIVO na categoria (não-jogador) com quem o jogador mais correu — o rival
/// de história mais longa no grid atual. `None` se o jogador nunca correu ali.
pub fn most_faced_rival(
    conn: &Connection,
    player_id: &str,
    categoria: &str,
) -> Result<Option<String>, DbError> {
    let id = conn
        .query_row(
            "SELECT rr.piloto_id
             FROM race_results pr
             JOIN race_results rr ON rr.race_id = pr.race_id AND rr.piloto_id != pr.piloto_id
             JOIN calendar c ON c.id = pr.race_id
             JOIN drivers d ON d.id = rr.piloto_id
             WHERE pr.piloto_id = ?1 AND c.categoria = ?2
               AND d.categoria_atual = ?2 AND d.is_jogador = 0 AND d.status != 'Aposentado'
             GROUP BY rr.piloto_id
             ORDER BY COUNT(*) DESC, rr.piloto_id ASC
             LIMIT 1",
            rusqlite::params![player_id, categoria],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    Ok(id)
}
