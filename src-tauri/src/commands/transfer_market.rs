//! Comandos da Janela de Transferências interativa (Fase 2).
//!
//! O jogador avança a janela semana a semana: vê suas ofertas, aceita uma ou
//! espera, e o estado (serializado) evolui e é persistido entre os comandos. Ao
//! fechar, todas as assinaturas da janela são aplicadas no banco.

use std::path::Path;

use rand::{rngs::StdRng, SeedableRng};

use crate::config::app_config::AppConfig;
use crate::constants::categories;
use crate::db::connection::Database;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::market::pipeline;
use crate::market::transfer_window::{PlayerOffer, Signing, WindowState};
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

/// Enriquece uma oferta do motor com os dados de exibição (time, categoria, carro).
fn build_offer_view(
    conn: &rusqlite::Connection,
    offer: &PlayerOffer,
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
        teammate_name: companion.as_ref().map(|d| d.nome.clone()),
        teammate_skill: companion
            .as_ref()
            .map(|d| d.atributos.skill.round().clamp(0.0, 100.0) as u8),
    })
}

fn build_payload(
    conn: &rusqlite::Connection,
    state: &WindowState,
) -> Result<TransferWindowPayload, String> {
    let player_offers = state
        .player_offers()
        .iter()
        .map(|offer| build_offer_view(conn, offer))
        .collect::<Result<Vec<_>, String>>()?;
    Ok(TransferWindowPayload {
        week: state.week(),
        closed: state.is_closed(),
        player_offers,
        signings: state.signings().to_vec(),
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

/// Estado atual da janela (inicia uma nova se ainda não existe).
pub(crate) fn get_transfer_window_state_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<TransferWindowPayload, String> {
    let (db, season) = open_career(base_dir, career_id)?;
    let mut rng = StdRng::seed_from_u64(season as u64);
    let state = pipeline::window_get_or_init(&db.conn, season, &mut rng)?;
    build_payload(&db.conn, &state)
}

/// Avança uma semana com a escolha do jogador (`accepted_seat_id` = aceita aquela
/// vaga; `None` = espera). Ao fechar, aplica as assinaturas no banco.
pub(crate) fn advance_transfer_window_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    accepted_seat_id: Option<&str>,
) -> Result<TransferWindowPayload, String> {
    let (db, season) = open_career(base_dir, career_id)?;
    let mut rng = StdRng::seed_from_u64(season as u64);
    let state = pipeline::window_advance(&db.conn, season, accepted_seat_id, &mut rng)?;
    build_payload(&db.conn, &state)
}
