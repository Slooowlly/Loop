use std::collections::HashMap;

use rand::Rng;
use rusqlite::Connection;

use crate::db::queries::meta as meta_queries;

use crate::calendar::generate_all_calendars_with_id_factory;
use crate::constants::categories::get_all_categories;
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::evolution::context::StandingEntry;
use crate::generators::ids::{next_id, next_ids, IdType};
use crate::models::season::Season;

pub(crate) fn create_and_persist_new_season(
    conn: &Connection,
    season: &Season,
) -> Result<Season, String> {
    let new_season_id = next_id(conn, IdType::Season)
        .map_err(|e| format!("Falha ao gerar ID da nova temporada: {e}"))?;
    let new_year = season.ano + 1;
    let new_season = Season::new(new_season_id, season.numero + 1, new_year);
    season_queries::insert_season(conn, &new_season)
        .map_err(|e| format!("Falha ao inserir nova temporada: {e}"))?;
    Ok(new_season)
}

/// Arquiva o snapshot completo de cada piloto ao fim da temporada.
/// Deve ser chamado DEPOIS do crescimento de atributos e ANTES da promoção/rebaixamento,
/// para capturar atributos finais e categoria original da temporada.
pub(crate) fn archive_driver_season(
    conn: &Connection,
    season: &Season,
    standings_by_driver: &HashMap<String, StandingEntry>,
) -> Result<(), String> {
    let drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos para arquivo historico: {e}"))?;

    for driver in &drivers {
        let standing = standings_by_driver.get(&driver.id);
        let categoria = standing.map(|s| s.category.as_str()).unwrap_or_default();
        let team_id = standing.and_then(|s| s.team_id.as_deref());
        let posicao_campeonato = standing.map(|s| s.position);
        let total_pilotos = standing.map(|s| s.total_drivers);
        let titulos = if posicao_campeonato == Some(1) { 1 } else { 0 };

        let snapshot = serde_json::json!({
            "piloto_id":            driver.id,
            "nome":                 driver.nome,
            "idade":                driver.idade,
            "nacionalidade":        driver.nacionalidade,
            "is_jogador":           driver.is_jogador,
            "season_number":        season.numero,
            "ano":                  season.ano,
            "categoria":            categoria,
            "team_id":              team_id,
            "posicao_campeonato":   posicao_campeonato,
            "total_pilotos":        total_pilotos,
            "pontos":               driver.stats_temporada.pontos,
            "vitorias":             driver.stats_temporada.vitorias,
            "podios":               driver.stats_temporada.podios,
            "poles":                driver.stats_temporada.poles,
            "titulos":              titulos,
            "corridas":             driver.stats_temporada.corridas,
            "dnfs":                 driver.stats_temporada.dnfs,
            "posicao_media":        driver.stats_temporada.posicao_media,
            "melhor_resultado":     driver.melhor_resultado_temp,
            "ultimos_resultados":   driver.ultimos_resultados,
            "atributos": {
                "skill":                driver.atributos.skill,
                "consistencia":         driver.atributos.consistencia,
                "racecraft":            driver.atributos.racecraft,
                "defesa":               driver.atributos.defesa,
                "ritmo_classificacao":  driver.atributos.ritmo_classificacao,
                "gestao_pneus":         driver.atributos.gestao_pneus,
                "habilidade_largada":   driver.atributos.habilidade_largada,
                "adaptabilidade":       driver.atributos.adaptabilidade,
                "fator_chuva":          driver.atributos.fator_chuva,
                "fitness":              driver.atributos.fitness,
                "experiencia":          driver.atributos.experiencia,
                "desenvolvimento":      driver.atributos.desenvolvimento,
                "aggression":           driver.atributos.aggression,
                "smoothness":           driver.atributos.smoothness,
                "midia":                driver.atributos.midia,
                "mentalidade":          driver.atributos.mentalidade,
                "confianca":            driver.atributos.confianca,
            },
            "motivacao":                driver.motivacao,
            "temporadas_na_categoria":  driver.temporadas_na_categoria,
        });

        conn.execute(
            "INSERT OR REPLACE INTO driver_season_archive
             (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                &driver.id,
                season.numero,
                season.ano,
                &driver.nome,
                categoria,
                posicao_campeonato,
                driver.stats_temporada.pontos,
                snapshot.to_string(),
            ],
        )
        .map_err(|e| {
            format!(
                "Falha ao arquivar temporada do piloto '{}': {e}",
                driver.id
            )
        })?;
    }
    Ok(())
}

pub(crate) fn reset_driver_season_stats(conn: &Connection) -> Result<(), String> {
    let mut drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao recarregar pilotos apos a nova temporada: {e}"))?;
    for driver in &mut drivers {
        driver.reset_season_stats();
        driver_queries::update_driver(conn, driver)
            .map_err(|e| format!("Falha ao resetar stats do piloto '{}': {e}", driver.nome))?;
    }
    Ok(())
}

pub(crate) fn reset_team_season_stats(
    conn: &Connection,
    new_season_numero: i32,
) -> Result<(), String> {
    let teams = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao recarregar equipes: {e}"))?;
    for team in &teams {
        team_queries::reset_team_season_stats(conn, &team.id)
            .map_err(|e| format!("Falha ao resetar stats da equipe '{}': {e}", team.id))?;
        conn.execute(
            "UPDATE teams SET temporada_atual = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            rusqlite::params![new_season_numero, &team.id],
        )
        .map_err(|e| format!("Falha ao atualizar temporada da equipe '{}': {e}", team.id))?;
    }
    Ok(())
}

pub(crate) fn seed_new_calendar(
    conn: &Connection,
    new_season_id: &str,
    new_year: i32,
    rng: &mut impl Rng,
) -> Result<(), String> {
    let total_new_races: u32 = get_all_categories()
        .iter()
        .map(|category| category.corridas_por_temporada as u32)
        .sum();
    let race_ids = next_ids(conn, IdType::Race, total_new_races)
        .map_err(|e| format!("Falha ao gerar IDs do calendario: {e}"))?;
    let mut race_ids_iter = race_ids.into_iter();
    let calendars = generate_all_calendars_with_id_factory(
        new_season_id,
        new_year,
        &mut || race_ids_iter.next().expect("calendar race id"),
        rng,
    )?;
    let all_entries: Vec<_> = calendars
        .values()
        .flat_map(|entries| entries.iter().cloned())
        .collect();
    calendar_queries::insert_calendar_entries(conn, &all_entries)
        .map_err(|e| format!("Falha ao inserir calendario da nova temporada: {e}"))?;
    Ok(())
}

pub(crate) fn update_meta_for_new_season(
    conn: &Connection,
    new_season_numero: i32,
    new_year: i32,
) -> Result<(), String> {
    meta_queries::set_current_season(conn, new_season_numero)
        .map_err(|e| format!("Falha ao atualizar meta current_season: {e}"))?;
    meta_queries::set_current_year(conn, new_year)
        .map_err(|e| format!("Falha ao atualizar meta current_year: {e}"))?;
    Ok(())
}
