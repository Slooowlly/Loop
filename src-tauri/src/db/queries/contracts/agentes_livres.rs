//! Agentes livres da pré-temporada: pilotos sem contrato Regular ativo.

use rusqlite::{params, Connection};

use crate::db::connection::DbError;

pub struct FreeAgentRaw {
    pub driver_id: String,
    pub driver_name: String,
    pub categoria: String,
    pub is_rookie: bool,
    pub previous_team_name: Option<String>,
    pub previous_team_color: Option<String>,
    pub seasons_at_last_team: i32,
    pub total_career_seasons: i32,
    pub max_license_level: Option<u8>,
    pub last_championship_position: Option<i32>,
    pub last_championship_total_drivers: Option<i32>,
    /// Temporadas sem correr = (última temporada arquivada no mundo) − (última em que
    /// o piloto COMPETIU de fato).
    /// `None` = nunca correu (rookie). `0` = correu na última temporada (agente fresco).
    ///
    /// A linha do arquivo não serve de prova de que o piloto correu: `archive_driver_season`
    /// grava uma por piloto por temporada, inclusive para quem ficou sem vaga — nesse caso
    /// com `categoria` vazia e `posicao_campeonato` nula. Sem filtrar por isso a conta dava
    /// zero para todo mundo e o marcador "parado" nunca aparecia.
    pub seasons_idle: Option<i32>,
}

/// Retorna pilotos ativos sem contrato Regular ativo, com dados do último time e contagem de temporadas.
/// `is_rookie = true` para pilotos que nunca tiveram contrato algum.
/// A categoria exibida vem do campo `categoria` do último contrato expirado/rescindido,
/// pois `drivers.categoria_atual` costuma estar NULL para pilotos IA.
pub fn get_free_agents_for_preseason(conn: &Connection) -> Result<Vec<FreeAgentRaw>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT
             d.id   AS driver_id,
             d.nome AS driver_name,
             COALESCE(
                 (SELECT c.categoria
                  FROM contracts c
                  WHERE c.piloto_id = d.id
                    AND c.tipo = 'Regular'
                    AND c.status IN ('Expirado', 'Rescindido')
                  ORDER BY CAST(c.temporada_fim AS INTEGER) DESC, c.created_at DESC
                  LIMIT 1),
                 NULLIF(d.categoria_atual, '')
             ) AS categoria,
             CASE
                 WHEN EXISTS (
                     SELECT 1 FROM contracts
                     WHERE piloto_id = d.id AND tipo = 'Regular'
                 ) THEN 0
                 ELSE 1
             END
                 AS is_rookie,
             (SELECT c.equipe_nome
              FROM contracts c
              WHERE c.piloto_id = d.id
                AND c.tipo = 'Regular'
                AND c.status IN ('Expirado', 'Rescindido')
              ORDER BY CAST(c.temporada_fim AS INTEGER) DESC, c.created_at DESC
              LIMIT 1) AS prev_team_name,
             (SELECT e.cor_primaria
              FROM contracts c
              JOIN teams e ON e.id = c.equipe_id
              WHERE c.piloto_id = d.id
                AND c.tipo = 'Regular'
                AND c.status IN ('Expirado', 'Rescindido')
              ORDER BY CAST(c.temporada_fim AS INTEGER) DESC, c.created_at DESC
              LIMIT 1) AS prev_team_color,
             (SELECT COALESCE(SUM(c2.duracao_anos), 0)
              FROM contracts c2
              WHERE c2.piloto_id = d.id
                AND c2.tipo = 'Regular'
                AND c2.status IN ('Expirado', 'Rescindido')
                AND c2.equipe_id = (
                    SELECT c.equipe_id
                    FROM contracts c
                    WHERE c.piloto_id = d.id
                      AND c.tipo = 'Regular'
                      AND c.status IN ('Expirado', 'Rescindido')
                    ORDER BY CAST(c.temporada_fim AS INTEGER) DESC, c.created_at DESC
                    LIMIT 1
                )
             ) AS seasons_at_team,
             (SELECT COALESCE(SUM(duracao_anos), 0)
              FROM contracts
              WHERE piloto_id = d.id
                AND tipo = 'Regular') AS career_seasons,
             (SELECT MAX(CAST(nivel AS INTEGER))
              FROM licenses
              WHERE piloto_id = d.id) AS max_license,
             (
                 (SELECT MAX(CAST(season_number AS INTEGER)) FROM driver_season_archive)
                 - (SELECT MAX(CAST(season_number AS INTEGER))
                    FROM driver_season_archive
                    WHERE piloto_id = d.id
                      AND COALESCE(categoria, '') <> '')
             ) AS seasons_idle
         FROM drivers d
         WHERE NOT EXISTS (
             SELECT 1 FROM contracts c
             WHERE c.piloto_id = d.id
               AND c.status = 'Ativo'
               AND c.tipo = 'Regular'
         )
           AND d.status = 'Ativo'
           AND d.is_jogador = 0
         ORDER BY categoria, is_rookie ASC, d.nome",
    )?;

    let mut result = Vec::new();
    let mapped = stmt.query_map([], |row| {
        let is_rookie_int: i32 = row.get("is_rookie")?;
        let max_license_raw: Option<i64> = row.get("max_license")?;
        Ok(FreeAgentRaw {
            driver_id: row.get("driver_id")?,
            driver_name: row.get("driver_name")?,
            categoria: row
                .get::<_, Option<String>>("categoria")?
                .unwrap_or_default(),
            is_rookie: is_rookie_int != 0,
            previous_team_name: row.get("prev_team_name")?,
            previous_team_color: row.get("prev_team_color")?,
            seasons_at_last_team: row.get("seasons_at_team")?,
            total_career_seasons: row.get("career_seasons")?,
            max_license_level: max_license_raw.map(|v| v as u8),
            last_championship_position: None,
            last_championship_total_drivers: None,
            seasons_idle: row.get::<_, Option<i32>>("seasons_idle")?,
        })
    })?;
    for row in mapped {
        result.push(row?);
    }
    for agent in &mut result {
        if agent.categoria.is_empty() {
            continue;
        }
        if let Some((position, total_drivers)) =
            get_latest_driver_archive_summary(conn, &agent.driver_id, &agent.categoria)?
        {
            agent.last_championship_position = Some(position);
            agent.last_championship_total_drivers = Some(total_drivers);
        }
    }
    Ok(result)
}

fn get_latest_driver_archive_summary(
    conn: &Connection,
    pilot_id: &str,
    categoria: &str,
) -> Result<Option<(i32, i32)>, DbError> {
    let result: Result<(Option<i32>, String), _> = conn.query_row(
        "SELECT posicao_campeonato, snapshot_json
         FROM driver_season_archive
         WHERE piloto_id = ?1
           AND categoria = ?2
         ORDER BY season_number DESC
         LIMIT 1",
        params![pilot_id, categoria],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );

    match result {
        Ok((Some(position), snapshot_json)) => {
            let total_drivers = serde_json::from_str::<serde_json::Value>(&snapshot_json)
                .ok()
                .and_then(|json| json.get("total_pilotos").and_then(|value| value.as_i64()))
                .map(|value| value as i32);
            Ok(total_drivers.map(|total| (position, total)))
        }
        Ok((None, _)) => Ok(None),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DbError::Sqlite(e)),
    }
}
