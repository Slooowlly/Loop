//! Comandos da Janela de Transferências interativa (Fase 2).
//!
//! O jogador avança a janela semana a semana: vê suas ofertas, aceita uma ou
//! espera, e o estado (serializado) evolui e é persistido entre os comandos. Ao
//! fechar, todas as assinaturas da janela são aplicadas no banco.

use std::path::Path;

use crate::config::app_config::AppConfig;
use crate::constants::categories;
use crate::db::connection::Database;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::market::pipeline;
use crate::market::transfer_window::{PlayerOffer, Signing};
use crate::simulation::math::normalize_car_performance;

/// Oferta da janela ENRIQUECIDA para a UI (nome/cor do time, categoria, carro,
/// companheiro de equipe) — montada a partir do `PlayerOffer` puro do motor.
#[derive(serde::Serialize)]
pub struct PlayerOfferView {
    pub seat_id: String,
    pub team_id: String,
    pub team_name: String,
    pub team_color: String,
    pub team_color_secondary: String,
    pub category: String,
    pub category_label: String,
    pub category_tier: u8,
    pub class: Option<String>,
    pub role: String,
    pub salary: f64,
    pub car_performance_rating: u8,
    pub teammate_name: Option<String>,
    pub teammate_skill: Option<u8>,
    // ── Estatísticas do companheiro (tooltip do card) ──
    /// Temporadas consecutivas do companheiro na equipe (None se desconhecido).
    pub teammate_tenure: Option<i32>,
    pub teammate_age: Option<u32>,
    pub teammate_races: Option<u32>,
    pub teammate_wins: Option<u32>,
    pub teammate_podiums: Option<u32>,
    pub teammate_poles: Option<u32>,
    pub teammate_titles: Option<u32>,
    pub teammate_career_points: Option<f64>,
    pub teammate_salary: Option<f64>,
    // ── Ficha da equipe (dados ricos para o modal robusto de ofertas) ──
    pub team_reputation: u8,
    pub team_reliability: u8,
    /// Caixa real da equipe (dinheiro em conta), não um índice.
    pub team_cash: f64,
    pub team_founded_year: i32,
    pub team_country: String,
    pub team_titles_drivers: i32,
    pub team_titles_constructors: i32,
    pub team_historic_wins: i32,
    pub team_historic_podiums: i32,
    /// Posição final da equipe na última temporada arquivada (None = estreante/sem histórico).
    pub team_last_position: Option<i32>,
}

/// Estado da janela apresentado à UI.
#[derive(serde::Serialize)]
pub struct TransferWindowPayload {
    pub week: u32,
    pub closed: bool,
    /// Ofertas que o jogador recebeu nesta semana (pra decidir).
    pub player_offers: Vec<PlayerOfferView>,
    /// Assinaturas da janela até agora (feed do mercado).
    pub signings: Vec<Signing>,
}

/// Posição final da equipe na temporada arquivada mais recente (None se estreante
/// ou se a tabela de histórico ainda não existir no save).
fn last_championship_position(conn: &rusqlite::Connection, team_id: &str) -> Option<i32> {
    conn.query_row(
        "SELECT posicao_campeonato FROM team_season_archive
         WHERE team_id = ?1 AND posicao_campeonato IS NOT NULL
         ORDER BY season_number DESC
         LIMIT 1",
        [team_id],
        |row| row.get::<_, i32>(0),
    )
    .ok()
}

/// Enriquece uma oferta do motor com os dados de exibição (time, categoria, carro).
fn build_offer_view(
    conn: &rusqlite::Connection,
    offer: &PlayerOffer,
    season: i32,
) -> Result<PlayerOfferView, String> {
    let team = team_queries::get_team_by_id(conn, &offer.team_id)
        .map_err(|e| format!("Falha ao carregar equipe da oferta: {e}"))?
        .ok_or_else(|| "Equipe da oferta nao encontrada.".to_string())?;
    let category = categories::get_category_config(&offer.category);
    // Companheiro = piloto da OUTRA vaga do time (se já preenchida).
    let companion_id = if offer.is_n1 {
        team.piloto_2_id.clone()
    } else {
        team.piloto_1_id.clone()
    };
    let companion = companion_id
        .as_deref()
        .map(|id| driver_queries::get_driver(conn, id))
        .transpose()
        .map_err(|e| format!("Falha ao carregar companheiro de equipe: {e}"))?;
    let companion_ref = companion.as_ref();
    let teammate_tenure = companion_ref.and_then(|d| {
        crate::commands::career::calculate_consecutive_team_tenure(conn, &d.id, &team.id, season)
    });
    let teammate_salary = companion_ref.and_then(|d| {
        contract_queries::get_active_contract_for_pilot(conn, &d.id)
            .ok()
            .flatten()
            .map(|c| c.salario_anual)
    });
    Ok(PlayerOfferView {
        seat_id: offer.seat_id.clone(),
        team_id: team.id.clone(),
        team_name: team.nome.clone(),
        team_color: team.cor_primaria.clone(),
        team_color_secondary: team.cor_secundaria.clone(),
        category: offer.category.clone(),
        category_label: category
            .map(|c| c.nome_curto.to_string())
            .unwrap_or_else(|| offer.category.clone()),
        category_tier: category.map(|c| c.tier).unwrap_or(0),
        class: offer.class.clone(),
        role: if offer.is_n1 { "N1" } else { "N2" }.to_string(),
        salary: offer.salary,
        car_performance_rating: normalize_car_performance(team.car_performance)
            .round()
            .clamp(0.0, 100.0) as u8,
        teammate_name: companion_ref.map(|d| d.nome.clone()),
        teammate_skill: companion_ref.map(|d| d.atributos.skill.round().clamp(0.0, 100.0) as u8),
        teammate_tenure,
        teammate_age: companion_ref.map(|d| d.idade),
        teammate_races: companion_ref.map(|d| d.stats_carreira.corridas),
        teammate_wins: companion_ref.map(|d| d.stats_carreira.vitorias),
        teammate_podiums: companion_ref.map(|d| d.stats_carreira.podios),
        teammate_poles: companion_ref.map(|d| d.stats_carreira.poles),
        teammate_titles: companion_ref.map(|d| d.stats_carreira.titulos),
        teammate_career_points: companion_ref.map(|d| d.stats_carreira.pontos_total),
        teammate_salary,
        team_reputation: team.reputacao.round().clamp(0.0, 100.0) as u8,
        team_reliability: team.confiabilidade.round().clamp(0.0, 100.0) as u8,
        team_cash: team.cash_balance,
        team_founded_year: team.ano_fundacao,
        team_country: team.pais_sede.clone(),
        team_titles_drivers: team.historico_titulos_pilotos,
        team_titles_constructors: team.historico_titulos_construtores,
        team_historic_wins: team.historico_vitorias,
        team_historic_podiums: team.historico_podios,
        team_last_position: last_championship_position(conn, &team.id),
    })
}

/// Monta o payload do mercado a partir da escada ao vivo: as ofertas do jogador são
/// derivadas das vagas regulares elegíveis (`pipeline::player_market_offers`). O feed
/// real do mercado vai pelas notícias da semana (não por este payload); `signings`
/// fica vazio. O fecho da janela é dirigido por `preseasonState.is_complete` na UI.
fn build_payload(conn: &rusqlite::Connection, season: i32) -> Result<TransferWindowPayload, String> {
    let offers = pipeline::player_market_offers(conn, season)?;
    let player_offers = offers
        .iter()
        .map(|offer| build_offer_view(conn, offer, season))
        .collect::<Result<Vec<_>, String>>()?;
    Ok(TransferWindowPayload {
        week: 0,
        closed: player_offers.is_empty(),
        player_offers,
        signings: Vec::new(),
    })
}

fn open_career(base_dir: &Path, career_id: &str) -> Result<(Database, i32), String> {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    if !db_path.exists() {
        return Err("Banco da carreira nao encontrado.".to_string());
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Nenhuma temporada ativa.".to_string())?;
    Ok((db, season.numero))
}

/// Ofertas atuais do jogador no mercado (vagas regulares elegíveis na sua faixa).
pub(crate) fn get_transfer_window_state_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<TransferWindowPayload, String> {
    let (db, season) = open_career(base_dir, career_id)?;
    build_payload(&db.conn, season)
}

/// O avanço real do mercado é feito por `advance_market_week` (que chama
/// `preseason::advance_week`). Este comando ficou legado: apenas devolve o estado
/// atual das ofertas, sem avançar nada — `accepted_seat_id` é ignorado.
pub(crate) fn advance_transfer_window_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    _accepted_seat_id: Option<&str>,
) -> Result<TransferWindowPayload, String> {
    let (db, season) = open_career(base_dir, career_id)?;
    build_payload(&db.conn, season)
}
