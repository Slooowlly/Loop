//! Tier 1: manchete de derby ao cruzar um threshold de percebida.

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::db::queries::news::insert_news;
use crate::db::queries::teams as team_queries;
use crate::generators::ids::{next_id, IdType};
use crate::models::team_rivalry::TeamRivalryType;
use crate::news::{NewsImportance, NewsItem, NewsType};
use crate::rivalry::{crossed_threshold, RivalryIntensityLevel};

use super::motor::TeamRivalryApplied;

fn team_name(conn: &Connection, team_id: &str) -> String {
    team_queries::get_team_by_id(conn, team_id)
        .ok()
        .flatten()
        .map(|t| t.nome)
        .unwrap_or_else(|| team_id.to_string())
}

/// Gera uma manchete de rivalidade de equipe ao CRUZAR um threshold de percebida (mesma
/// lógica `crossed_threshold` do piloto). Voz jornalística em 3ª pessoa (revista).
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_team_rivalry_news(
    conn: &Connection,
    applied: &TeamRivalryApplied,
    tipo: TeamRivalryType,
    team_a_id: &str,
    team_b_id: &str,
    categoria_id: Option<&str>,
    rodada: Option<i32>,
    temporada: i32,
) -> Result<(), DbError> {
    let Some(crossed) = crossed_threshold(applied.old_perceived, applied.new_perceived) else {
        return Ok(());
    };
    let importance = match crossed {
        RivalryIntensityLevel::Inicial => NewsImportance::Media,
        RivalryIntensityLevel::Clara => NewsImportance::Alta,
        RivalryIntensityLevel::Forte | RivalryIntensityLevel::Intensa => NewsImportance::Destaque,
        RivalryIntensityLevel::AtritoLeve => NewsImportance::Media,
    };
    let origem = match tipo {
        TeamRivalryType::Campeonato => rust_i18n::t!("team_rivalry.news.origin_championship"),
        TeamRivalryType::Mercado => rust_i18n::t!("team_rivalry.news.origin_market"),
        TeamRivalryType::Pista => rust_i18n::t!("team_rivalry.news.origin_track"),
        TeamRivalryType::Herdada => rust_i18n::t!("team_rivalry.news.origin_inherited"),
    };
    let nome_a = team_name(conn, team_a_id);
    let nome_b = team_name(conn, team_b_id);
    let titulo = rust_i18n::t!(
        "team_rivalry.news.title",
        a = nome_a,
        b = nome_b,
        level = crossed.label()
    )
    .to_string();
    let texto = rust_i18n::t!(
        "team_rivalry.news.text",
        a = nome_a,
        b = nome_b,
        origin = origem
    )
    .to_string();

    let item = NewsItem {
        id: next_id(conn, IdType::News)?,
        tipo: NewsType::Rivalidade,
        icone: NewsType::Rivalidade.icone().to_string(),
        titulo,
        texto,
        rodada,
        semana_pretemporada: None,
        temporada,
        categoria_id: categoria_id.map(str::to_string),
        categoria_nome: None,
        importancia: importance,
        timestamp: chrono::Local::now().timestamp(),
        driver_id: None,
        driver_id_secondary: None,
        team_id: Some(team_a_id.to_string()),
    };
    insert_news(conn, &item)?;
    Ok(())
}
