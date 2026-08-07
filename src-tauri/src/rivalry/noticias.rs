//! Notícia de escalada de rivalidade — montagem e persistência
//! (extraído de `rivalry/mod.rs`).

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::db::queries::drivers::get_driver;
use crate::db::queries::news::insert_news;
use crate::generators::ids::{next_id, IdType};
use crate::models::rivalry::RivalryType;
use crate::news::{NewsImportance, NewsItem, NewsType};

use super::eventos::RivalryApplied;
use super::intensidade::{crossed_threshold, RivalryIntensityLevel};

// ── Passo 10: Geração de notícia (atualizado para percebida) ──────────────────

pub(crate) fn build_rivalry_news_item(
    id: String,
    applied: &RivalryApplied,
    tipo: &RivalryType,
    nome_a: &str,
    nome_b: &str,
    categoria_id: &str,
    temporada: i32,
    rodada: i32,
    piloto_a_id: &str,
    piloto_b_id: &str,
    team_id: Option<&str>,
    _driver_midia: &std::collections::HashMap<String, f64>,
) -> Option<NewsItem> {
    let crossed = crossed_threshold(applied.old_perceived, applied.new_perceived)?;
    let importance = match crossed {
        RivalryIntensityLevel::Inicial => NewsImportance::Media,
        RivalryIntensityLevel::Clara => NewsImportance::Alta,
        RivalryIntensityLevel::Forte | RivalryIntensityLevel::Intensa => NewsImportance::Destaque,
        RivalryIntensityLevel::AtritoLeve => NewsImportance::Media,
    };
    let origem = match tipo {
        RivalryType::Companheiros => rust_i18n::t!("rivalry.news.escalation.origin_teammates"),
        RivalryType::Campeonato => rust_i18n::t!("rivalry.news.escalation.origin_championship"),
        RivalryType::Colisao => rust_i18n::t!("rivalry.news.escalation.origin_collision"),
        RivalryType::Pista => rust_i18n::t!("rivalry.news.escalation.origin_track"),
    };
    let nivel_label = crossed.label();
    let titulo = rust_i18n::t!(
        "rivalry.news.escalation.title",
        a = nome_a,
        b = nome_b,
        level = nivel_label
    )
    .to_string();
    // Gatilhos de fim de temporada (`process_teammate_season_rivalry`) não têm rodada:
    // o fato deles é o placar do ano inteiro. Passam `rodada = 0` e ganham uma frase
    // com escopo de temporada — citar "na rodada 0" seria mentira de calendário.
    let escopo_temporada = rodada <= 0;
    let texto = if escopo_temporada {
        rust_i18n::t!(
            "rivalry.news.escalation.text_season",
            a = nome_a,
            b = nome_b,
            origin = origem
        )
        .to_string()
    } else {
        rust_i18n::t!(
            "rivalry.news.escalation.text",
            a = nome_a,
            b = nome_b,
            origin = origem,
            round = rodada
        )
        .to_string()
    };

    Some(NewsItem {
        id,
        tipo: NewsType::Rivalidade,
        icone: NewsType::Rivalidade.icone().to_string(),
        titulo,
        texto,
        rodada: if escopo_temporada { None } else { Some(rodada) },
        semana_pretemporada: None,
        temporada,
        categoria_id: Some(categoria_id.to_string()),
        categoria_nome: None,
        importancia: importance,
        timestamp: chrono::Local::now().timestamp(),
        driver_id: Some(piloto_a_id.to_string()),
        driver_id_secondary: Some(piloto_b_id.to_string()),
        team_id: team_id.map(str::to_string),
    })
}

pub(crate) fn load_rivalry_driver_midia(
    conn: &Connection,
    piloto_a_id: &str,
    piloto_b_id: &str,
) -> std::collections::HashMap<String, f64> {
    let mut driver_midia = std::collections::HashMap::new();

    for driver_id in [piloto_a_id, piloto_b_id] {
        if let Ok(driver) = get_driver(conn, driver_id) {
            driver_midia.insert(driver_id.to_string(), driver.atributos.midia);
        }
    }

    driver_midia
}

pub(crate) fn persist_rivalry_news(
    conn: &Connection,
    applied: &RivalryApplied,
    tipo: &RivalryType,
    nome_a: &str,
    nome_b: &str,
    categoria_id: &str,
    temporada: i32,
    rodada: i32,
    piloto_a_id: &str,
    piloto_b_id: &str,
    team_id: Option<&str>,
    driver_midia: &std::collections::HashMap<String, f64>,
) -> Result<(), DbError> {
    let Some(item) = build_rivalry_news_item(
        next_id(conn, IdType::News)?,
        applied,
        tipo,
        nome_a,
        nome_b,
        categoria_id,
        temporada,
        rodada,
        piloto_a_id,
        piloto_b_id,
        team_id,
        driver_midia,
    ) else {
        return Ok(());
    };

    insert_news(conn, &item)?;
    Ok(())
}
