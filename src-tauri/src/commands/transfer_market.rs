//! Comandos da Janela de Transferências interativa (Fase 2).
//!
//! O jogador avança a janela semana a semana: vê suas ofertas, aceita uma ou
//! espera, e o estado (serializado) evolui e é persistido entre os comandos. Ao
//! fechar, todas as assinaturas da janela são aplicadas no banco.

use std::path::Path;

use rand::{rngs::StdRng, SeedableRng};

use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::db::queries::seasons as season_queries;
use crate::market::pipeline;
use crate::market::transfer_window::{PlayerOffer, Signing, WindowState};

/// Estado da janela apresentado à UI.
#[derive(serde::Serialize)]
pub struct TransferWindowPayload {
    pub week: u32,
    pub closed: bool,
    /// Ofertas que o jogador recebeu nesta semana (pra decidir).
    pub player_offers: Vec<PlayerOffer>,
    /// Assinaturas da janela até agora (feed do mercado).
    pub signings: Vec<Signing>,
}

impl TransferWindowPayload {
    fn from_state(state: &WindowState) -> Self {
        Self {
            week: state.week(),
            closed: state.is_closed(),
            player_offers: state.player_offers(),
            signings: state.signings().to_vec(),
        }
    }
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
    Ok(TransferWindowPayload::from_state(&state))
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
    Ok(TransferWindowPayload::from_state(&state))
}
