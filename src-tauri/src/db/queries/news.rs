#![allow(dead_code)]

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{
        get_latest_news_timestamp, get_news_by_driver, get_news_by_preseason_week,
        get_news_by_season, get_news_by_team, get_news_by_type, get_recent_news, insert_news,
        insert_news_batch, trim_news,
    };
    use crate::db::migrations;
    use crate::db::queries::seasons::insert_season;
    use crate::models::season::Season;
    use crate::news::{NewsImportance, NewsItem, NewsType};

    #[test]
    fn test_insert_and_get_news() {
        let conn = setup_news_db();
        let item = sample_news("N001", 1, Some(3), None, NewsType::Corrida, 10);

        insert_news(&conn, &item).expect("insert");

        let items = get_recent_news(&conn, 10).expect("recent news");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].titulo, item.titulo);
    }

    #[test]
    fn test_get_news_by_season() {
        let conn = setup_news_db();
        insert_news(
            &conn,
            &sample_news("N001", 1, Some(1), None, NewsType::Corrida, 10),
        )
        .expect("insert 1");
        insert_news(
            &conn,
            &sample_news("N002", 2, Some(1), None, NewsType::Mercado, 20),
        )
        .expect("insert 2");

        let season_news = get_news_by_season(&conn, 2, 10).expect("season news");

        assert_eq!(season_news.len(), 1);
        assert_eq!(season_news[0].temporada, 2);
    }

    #[test]
    fn test_get_news_by_type() {
        let conn = setup_news_db();
        insert_news(
            &conn,
            &sample_news("N001", 1, Some(1), None, NewsType::Corrida, 10),
        )
        .expect("insert corrida");
        insert_news(
            &conn,
            &sample_news("N002", 1, Some(-2), Some(2), NewsType::Mercado, 20),
        )
        .expect("insert mercado");

        let market = get_news_by_type(&conn, &NewsType::Mercado, 10).expect("market news");

        assert_eq!(market.len(), 1);
        assert_eq!(market[0].tipo, NewsType::Mercado);
    }

    #[test]
    fn test_get_news_by_preseason_week() {
        let conn = setup_news_db();
        insert_news(
            &conn,
            &sample_news("N001", 2, Some(-3), Some(3), NewsType::Mercado, 15),
        )
        .expect("insert week 3");
        insert_news(
            &conn,
            &sample_news("N002", 2, Some(-4), Some(4), NewsType::Mercado, 20),
        )
        .expect("insert week 4");

        let week_news = get_news_by_preseason_week(&conn, 2, 3).expect("week news");

        assert_eq!(week_news.len(), 1);
        assert_eq!(week_news[0].semana_pretemporada, Some(3));
    }

    #[test]
    fn test_trim_news_removes_oldest() {
        let conn = setup_news_db();
        let batch = vec![
            sample_news("N001", 1, Some(1), None, NewsType::Corrida, 10),
            sample_news("N002", 1, Some(2), None, NewsType::Corrida, 20),
            sample_news("N003", 1, Some(3), None, NewsType::Corrida, 30),
        ];
        insert_news_batch(&conn, &batch).expect("batch");

        let removed = trim_news(&conn, 2).expect("trim");

        assert_eq!(removed, 1);
        let items = get_recent_news(&conn, 10).expect("recent");
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|item| item.id != "N001"));
    }

    #[test]
    fn test_insert_news_batch_rolls_back_if_any_item_fails() {
        let conn = setup_news_db();
        let batch = vec![
            sample_news("N001", 1, Some(1), None, NewsType::Corrida, 10),
            sample_news("N_INVALID", 999, Some(2), None, NewsType::Mercado, 20),
        ];

        let err = insert_news_batch(&conn, &batch).expect_err("batch should fail");
        assert!(err.to_string().contains("Query returned no rows"));

        let items = get_recent_news(&conn, 10).expect("recent");
        assert!(items.is_empty(), "batch should rollback previous inserts");
    }

    #[test]
    fn test_news_ordered_by_timestamp() {
        let conn = setup_news_db();
        insert_news(
            &conn,
            &sample_news("N001", 1, Some(1), None, NewsType::Corrida, 10),
        )
        .expect("insert first");
        insert_news(
            &conn,
            &sample_news("N002", 1, Some(2), None, NewsType::Mercado, 20),
        )
        .expect("insert second");

        let items = get_recent_news(&conn, 10).expect("recent");

        assert_eq!(items[0].id, "N002");
        assert_eq!(items[1].id, "N001");
    }

    #[test]
    fn test_get_news_by_driver_matches_secondary_driver() {
        let conn = setup_news_db();
        let mut item = sample_news("N003", 1, Some(1), None, NewsType::Rivalidade, 30);
        item.driver_id = Some("P001".to_string());
        item.driver_id_secondary = Some("P002".to_string());
        insert_news(&conn, &item).expect("insert rivalry");

        let items = get_news_by_driver(&conn, "P002", 10).expect("driver news");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "N003");
    }

    #[test]
    fn test_invalid_news_type_from_db_returns_error() {
        let conn = setup_news_db();
        insert_news(
            &conn,
            &sample_news("N900", 1, Some(1), None, NewsType::Corrida, 10),
        )
        .expect("insert");
        conn.execute(
            "UPDATE news SET tipo = 'tipo_quebrado' WHERE id = 'N900'",
            [],
        )
        .expect("corrupt type");

        let err = get_recent_news(&conn, 10).expect_err("invalid type should fail");
        assert!(err.to_string().contains("NewsType inválido"));
    }

    #[test]
    fn test_invalid_news_importance_from_payload_returns_error() {
        let conn = setup_news_db();
        insert_news(
            &conn,
            &sample_news("N901", 1, Some(1), None, NewsType::Corrida, 10),
        )
        .expect("insert");
        conn.execute(
            "UPDATE news
             SET texto = '{\"texto\":\"Texto N901\",\"icone\":\"i\",\"semana_pretemporada\":null,\"categoria_id\":\"gt4\",\"categoria_nome\":\"GT4\",\"importancia\":\"importancia_quebrada\",\"timestamp\":10,\"driver_id\":\"P001\",\"driver_id_secondary\":null,\"team_id\":\"T001\"}'
             WHERE id = 'N901'",
            [],
        )
        .expect("corrupt payload");

        let err = get_recent_news(&conn, 10).expect_err("invalid importance should fail");
        assert!(err.to_string().contains("NewsImportance inválida"));
    }

    #[test]
    fn test_invalid_news_payload_from_db_returns_error() {
        let conn = setup_news_db();
        insert_news(
            &conn,
            &sample_news("N902", 1, Some(1), None, NewsType::Corrida, 10),
        )
        .expect("insert");
        conn.execute("UPDATE news SET texto = '{' WHERE id = 'N902'", [])
            .expect("corrupt payload");

        let err = get_recent_news(&conn, 10).expect_err("invalid payload should fail");
        assert!(err
            .to_string()
            .contains("Falha ao interpretar payload da noticia"));
    }

    #[test]
    fn test_invalid_latest_news_timestamp_from_db_returns_error() {
        let conn = setup_news_db();
        insert_news(
            &conn,
            &sample_news("N903", 1, Some(1), None, NewsType::Corrida, 10),
        )
        .expect("insert");
        conn.execute(
            "UPDATE news SET criado_em = 'quebrado' WHERE id = 'N903'",
            [],
        )
        .expect("corrupt timestamp");

        let err = get_latest_news_timestamp(&conn).expect_err("invalid timestamp should fail");
        assert!(err.to_string().contains("Timestamp de noticia invalido"));
    }

    #[test]
    fn test_invalid_created_at_from_legacy_news_row_returns_error() {
        let conn = setup_news_db();
        let season_id: String = conn
            .query_row("SELECT id FROM seasons WHERE numero = 1", [], |row| {
                row.get(0)
            })
            .expect("season id");

        conn.execute(
            "INSERT INTO news (
                id, tipo, titulo, texto, chave_dedup, temporada_id, rodada, criado_em, lida
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
            (
                "LEGACY_BAD_TS",
                "Corrida",
                "Titulo legado",
                "Texto legado",
                "legacy_bad_ts",
                season_id,
                1,
                "quebrado",
            ),
        )
        .expect("insert legacy row");

        let err = get_recent_news(&conn, 10).expect_err("invalid created_at should fail");
        assert!(err.to_string().contains("Timestamp de noticia invalido"));
    }

    #[test]
    fn test_get_news_by_driver_reaches_items_beyond_recent_window() {
        let conn = setup_news_db();

        let mut target = sample_news("TARGET", 1, Some(1), None, NewsType::Corrida, 1);
        target.driver_id = Some("P999".to_string());
        target.driver_id_secondary = None;
        insert_news(&conn, &target).expect("insert target");

        for idx in 0..450 {
            let mut item = sample_news(
                &format!("N{:03}", idx),
                1,
                Some(1),
                None,
                NewsType::Mercado,
                100 + idx as i64,
            );
            item.driver_id = Some("P001".to_string());
            insert_news(&conn, &item).expect("insert filler");
        }

        let items = get_news_by_driver(&conn, "P999", 10).expect("driver news");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "TARGET");
    }

    #[test]
    fn test_get_news_by_team_reaches_items_beyond_recent_window() {
        let conn = setup_news_db();

        let mut target = sample_news("TEAM_TARGET", 1, Some(1), None, NewsType::Corrida, 1);
        target.team_id = Some("TEAM_SPECIAL".to_string());
        insert_news(&conn, &target).expect("insert target");

        for idx in 0..450 {
            let mut item = sample_news(
                &format!("T{:03}", idx),
                1,
                Some(1),
                None,
                NewsType::Mercado,
                100 + idx as i64,
            );
            item.team_id = Some("T001".to_string());
            insert_news(&conn, &item).expect("insert filler");
        }

        let items = get_news_by_team(&conn, "TEAM_SPECIAL", 10).expect("team news");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "TEAM_TARGET");
    }

    fn setup_news_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        let mut season1 = Season::new("S001".to_string(), 1, 2024);
        season1.finalizar();
        insert_season(&conn, &season1).expect("season 1");
        insert_season(&conn, &Season::new("S002".to_string(), 2, 2025)).expect("season 2");
        conn
    }

    fn sample_news(
        id: &str,
        temporada: i32,
        rodada: Option<i32>,
        semana_pretemporada: Option<i32>,
        tipo: NewsType,
        timestamp: i64,
    ) -> NewsItem {
        NewsItem {
            id: id.to_string(),
            tipo: tipo.clone(),
            icone: tipo.icone().to_string(),
            titulo: format!("Noticia {}", id),
            texto: format!("Texto {}", id),
            rodada,
            semana_pretemporada,
            temporada,
            categoria_id: Some("gt4".to_string()),
            categoria_nome: Some("GT4".to_string()),
            importancia: NewsImportance::Media,
            timestamp,
            driver_id: Some("P001".to_string()),
            driver_id_secondary: None,
            team_id: Some("T001".to_string()),
        }
    }
}
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db::connection::DbError;
use crate::news::{NewsImportance, NewsItem, NewsType};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredNewsPayload {
    texto: String,
    icone: String,
    semana_pretemporada: Option<i32>,
    categoria_id: Option<String>,
    categoria_nome: Option<String>,
    importancia: String,
    timestamp: i64,
    driver_id: Option<String>,
    driver_id_secondary: Option<String>,
    team_id: Option<String>,
}

pub fn insert_news(conn: &Connection, news: &NewsItem) -> Result<(), DbError> {
    let season_id = season_id_from_number(conn, news.temporada)?;
    let payload = StoredNewsPayload {
        texto: news.texto.clone(),
        icone: news.icone.clone(),
        semana_pretemporada: news.semana_pretemporada,
        categoria_id: news.categoria_id.clone(),
        categoria_nome: news.categoria_nome.clone(),
        importancia: news.importancia.as_str().to_string(),
        timestamp: news.timestamp,
        driver_id: news.driver_id.clone(),
        driver_id_secondary: news.driver_id_secondary.clone(),
        team_id: news.team_id.clone(),
    };
    let serialized = serde_json::to_string(&payload)
        .map_err(|e| DbError::Migration(format!("Falha ao serializar noticia: {e}")))?;
    conn.execute(
        "INSERT OR IGNORE INTO news (
            id, tipo, titulo, texto, chave_dedup, temporada_id, rodada, criado_em, lida
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
        params![
            news.id,
            news.tipo.as_str(),
            news.titulo,
            serialized,
            dedup_key(news),
            season_id,
            news.rodada.unwrap_or(0),
            news.timestamp.to_string(),
        ],
    )?;
    Ok(())
}

pub fn insert_news_batch(conn: &Connection, items: &[NewsItem]) -> Result<(), DbError> {
    let tx = conn.unchecked_transaction()?;
    for item in items {
        insert_news(&tx, item)?;
    }
    tx.commit()?;
    Ok(())
}

pub fn get_news_by_season(
    conn: &Connection,
    temporada: i32,
    limit: i32,
) -> Result<Vec<NewsItem>, DbError> {
    load_news(
        conn,
        "SELECT n.*, s.numero AS temporada_numero
         FROM news n
         JOIN seasons s ON s.id = n.temporada_id
         WHERE s.numero = ?1
         ORDER BY CAST(n.criado_em AS INTEGER) DESC, n.id DESC
         LIMIT ?2",
        params![temporada, limit.max(1)],
    )
}

pub fn get_news_by_type(
    conn: &Connection,
    tipo: &NewsType,
    limit: i32,
) -> Result<Vec<NewsItem>, DbError> {
    load_news(
        conn,
        "SELECT n.*, s.numero AS temporada_numero
         FROM news n
         JOIN seasons s ON s.id = n.temporada_id
         WHERE n.tipo = ?1
         ORDER BY CAST(n.criado_em AS INTEGER) DESC, n.id DESC
         LIMIT ?2",
        params![tipo.as_str(), limit.max(1)],
    )
}

pub fn get_news_by_preseason_week(
    conn: &Connection,
    temporada: i32,
    semana: i32,
) -> Result<Vec<NewsItem>, DbError> {
    load_news(
        conn,
        "SELECT n.*, s.numero AS temporada_numero
         FROM news n
         JOIN seasons s ON s.id = n.temporada_id
         WHERE s.numero = ?1 AND n.rodada = ?2
         ORDER BY CAST(n.criado_em AS INTEGER) DESC, n.id DESC",
        params![temporada, -semana.abs()],
    )
}

pub fn get_recent_news(conn: &Connection, limit: i32) -> Result<Vec<NewsItem>, DbError> {
    load_news(
        conn,
        "SELECT n.*, s.numero AS temporada_numero
         FROM news n
         JOIN seasons s ON s.id = n.temporada_id
         ORDER BY CAST(n.criado_em AS INTEGER) DESC, n.id DESC
         LIMIT ?1",
        params![limit.max(1)],
    )
}

pub fn get_news_by_driver(
    conn: &Connection,
    driver_id: &str,
    limit: i32,
) -> Result<Vec<NewsItem>, DbError> {
    load_news(
        conn,
        "SELECT n.*, s.numero AS temporada_numero
         FROM news n
         JOIN seasons s ON s.id = n.temporada_id
         WHERE json_extract(n.texto, '$.driver_id') = ?1
            OR json_extract(n.texto, '$.driver_id_secondary') = ?1
         ORDER BY CAST(n.criado_em AS INTEGER) DESC, n.id DESC
         LIMIT ?2",
        params![driver_id, limit.max(1)],
    )
}

pub fn get_news_by_team(
    conn: &Connection,
    team_id: &str,
    limit: i32,
) -> Result<Vec<NewsItem>, DbError> {
    load_news(
        conn,
        "SELECT n.*, s.numero AS temporada_numero
         FROM news n
         JOIN seasons s ON s.id = n.temporada_id
         WHERE json_extract(n.texto, '$.team_id') = ?1
         ORDER BY CAST(n.criado_em AS INTEGER) DESC, n.id DESC
         LIMIT ?2",
        params![team_id, limit.max(1)],
    )
}

pub fn count_news(conn: &Connection) -> Result<i32, DbError> {
    conn.query_row("SELECT COUNT(*) FROM news", [], |row| row.get(0))
        .map_err(DbError::from)
}

pub fn trim_news(conn: &Connection, max_items: i32) -> Result<i32, DbError> {
    let total = count_news(conn)?;
    let overflow = total - max_items.max(0);
    if overflow <= 0 {
        return Ok(0);
    }

    conn.execute(
        "DELETE FROM news
         WHERE id IN (
            SELECT id FROM news
            ORDER BY CAST(criado_em AS INTEGER) ASC, id ASC
            LIMIT ?1
         )",
        params![overflow],
    )?;
    Ok(overflow)
}

pub fn delete_all_news(conn: &Connection) -> Result<(), DbError> {
    conn.execute("DELETE FROM news", [])?;
    Ok(())
}

pub fn get_latest_news_timestamp(conn: &Connection) -> Result<i64, DbError> {
    let value: Option<String> = conn
        .query_row(
            "SELECT criado_em FROM news ORDER BY CAST(criado_em AS INTEGER) DESC, id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    match value {
        Some(raw) => parse_news_timestamp(&raw),
        None => Ok(0),
    }
}

fn load_news<P: rusqlite::Params>(
    conn: &Connection,
    sql: &str,
    params: P,
) -> Result<Vec<NewsItem>, DbError> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params, news_from_row)?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

fn news_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<NewsItem> {
    let raw_text: String = row.get("texto")?;
    let payload = parse_news_payload(&raw_text)?;
    let rodada: i32 = row.get("rodada")?;
    let timestamp = payload
        .as_ref()
        .map(|payload| payload.timestamp)
        .map(Ok)
        .unwrap_or_else(|| parse_news_timestamp_from_row(&row.get::<_, String>("criado_em")?))?;

    Ok(NewsItem {
        id: row.get("id")?,
        tipo: NewsType::from_str_strict(&row.get::<_, String>("tipo")?)
            .map_err(rusqlite::Error::InvalidParameterName)?,
        icone: payload
            .as_ref()
            .map(|payload| payload.icone.clone())
            .unwrap_or_else(|| "📰".to_string()),
        titulo: row.get("titulo")?,
        texto: payload
            .as_ref()
            .map(|payload| payload.texto.clone())
            .unwrap_or(raw_text),
        rodada: Some(rodada),
        semana_pretemporada: payload
            .as_ref()
            .and_then(|payload| payload.semana_pretemporada)
            .or_else(|| if rodada < 0 { Some(-rodada) } else { None }),
        temporada: row.get("temporada_numero")?,
        categoria_id: payload
            .as_ref()
            .and_then(|payload| payload.categoria_id.clone()),
        categoria_nome: payload
            .as_ref()
            .and_then(|payload| payload.categoria_nome.clone()),
        importancia: match payload.as_ref() {
            Some(payload) => NewsImportance::from_str_strict(&payload.importancia)
                .map_err(rusqlite::Error::InvalidParameterName)?,
            None => NewsImportance::Media,
        },
        timestamp,
        driver_id: payload
            .as_ref()
            .and_then(|payload| payload.driver_id.clone()),
        driver_id_secondary: payload
            .as_ref()
            .and_then(|payload| payload.driver_id_secondary.clone()),
        team_id: payload.as_ref().and_then(|payload| payload.team_id.clone()),
    })
}

fn dedup_key(news: &NewsItem) -> String {
    let title = news
        .titulo
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() {
                char.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!(
        "{}_s{}_r{}_w{}_d{}_ds{}_t{}_{}",
        news.tipo.as_str().to_ascii_lowercase(),
        news.temporada,
        news.rodada.unwrap_or(0),
        news.semana_pretemporada.unwrap_or(0),
        news.driver_id.as_deref().unwrap_or("none"),
        news.driver_id_secondary.as_deref().unwrap_or("none"),
        news.team_id.as_deref().unwrap_or("none"),
        title
    )
}

fn season_id_from_number(conn: &Connection, season_number: i32) -> Result<String, DbError> {
    conn.query_row(
        "SELECT id FROM seasons WHERE numero = ?1 LIMIT 1",
        params![season_number],
        |row| row.get(0),
    )
    .map_err(DbError::from)
}

fn parse_news_timestamp(raw: &str) -> Result<i64, DbError> {
    raw.parse::<i64>()
        .map_err(|_| DbError::InvalidData(format!("Timestamp de noticia invalido: '{raw}'")))
}

fn parse_news_timestamp_from_row(raw: &str) -> rusqlite::Result<i64> {
    raw.parse::<i64>().map_err(|_| {
        rusqlite::Error::InvalidParameterName(format!("Timestamp de noticia invalido: '{raw}'"))
    })
}

fn parse_news_payload(raw_text: &str) -> rusqlite::Result<Option<StoredNewsPayload>> {
    if !raw_text.trim_start().starts_with('{') {
        return Ok(None);
    }

    serde_json::from_str::<StoredNewsPayload>(raw_text)
        .map(Some)
        .map_err(|error| {
            rusqlite::Error::InvalidParameterName(format!(
                "Falha ao interpretar payload da noticia: {error}"
            ))
        })
}
