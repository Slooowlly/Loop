//! Estado diario da janela especial: leitura do payload, escolha da oferta do dia e
//! avanco de um dia da janela.

use super::*;

pub(crate) fn get_special_window_state_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<SpecialWindowPayload, String> {
    let db_path = career_db_path(base_dir, career_id);
    let db = Database::open_existing(&db_path).map_err(|e| e.to_string())?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Nenhuma temporada ativa.".to_string())?;

    special_window::load_special_window_payload(&db.conn, &season.id, &player.id)
        .map_err(|e| format!("Falha ao carregar janela especial: {e}"))
}

pub(crate) fn accept_special_offer_for_day_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    offer_id: &str,
) -> Result<SpecialWindowPayload, String> {
    let db_path = career_db_path(base_dir, career_id);
    let db = Database::open_existing(&db_path).map_err(|e| e.to_string())?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Nenhuma temporada ativa.".to_string())?;

    special_window::select_player_offer_for_day(&db.conn, &season.id, &player.id, offer_id)
        .map_err(|e| format!("Falha ao definir escolha diaria da convocacao: {e}"))
}

pub(crate) fn advance_special_window_day_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<SpecialWindowPayload, String> {
    let db_path = career_db_path(base_dir, career_id);
    let db = Database::open_existing(&db_path).map_err(|e| e.to_string())?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Nenhuma temporada ativa.".to_string())?;

    special_window::advance_special_window_day(&db.conn, &season.id, &player.id)
        .map_err(|e| format!("Falha ao avancar dia da janela especial: {e}"))
}
