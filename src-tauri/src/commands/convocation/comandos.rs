//! Casca `#[tauri::command]` da convocacao: transicoes de fase do bloco especial e
//! delegacao das operacoes de oferta/janela para as funcoes internas.

use super::*;

/// BlocoRegular → JanelaConvocacao.
#[tauri::command]
pub fn advance_to_convocation_window(career_id: String, app: AppHandle) -> Result<(), String> {
    let base_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = career_db_path(&base_dir, &career_id);
    let db = Database::open_existing(&db_path).map_err(|e| e.to_string())?;
    adv_fn(&db.conn).map_err(|e| e.to_string())
}

/// Monta os grids das categorias especiais (permanece em JanelaConvocacao).
#[tauri::command]
pub fn run_convocation_window(
    career_id: String,
    app: AppHandle,
) -> Result<ConvocationResult, String> {
    let base_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = career_db_path(&base_dir, &career_id);
    let db = Database::open_existing(&db_path).map_err(|e| e.to_string())?;
    run_fn(&db.conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_player_special_offers(
    career_id: String,
    app: AppHandle,
) -> Result<Vec<PlayerSpecialOffer>, String> {
    let base_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    get_player_special_offers_in_base_dir(&base_dir, &career_id)
}

#[tauri::command]
pub fn get_special_window_state(
    career_id: String,
    app: AppHandle,
) -> Result<SpecialWindowPayload, String> {
    let base_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    get_special_window_state_in_base_dir(&base_dir, &career_id)
}

#[tauri::command]
pub fn accept_special_offer_for_day(
    career_id: String,
    offer_id: String,
    app: AppHandle,
) -> Result<SpecialWindowPayload, String> {
    let base_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    accept_special_offer_for_day_in_base_dir(&base_dir, &career_id, &offer_id)
}

#[tauri::command]
pub fn advance_special_window_day(
    career_id: String,
    app: AppHandle,
) -> Result<SpecialWindowPayload, String> {
    let base_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    advance_special_window_day_in_base_dir(&base_dir, &career_id)
}

#[tauri::command]
pub fn respond_player_special_offer(
    career_id: String,
    offer_id: String,
    accept: bool,
    app: AppHandle,
) -> Result<PlayerSpecialOfferResponse, String> {
    let base_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    respond_player_special_offer_in_base_dir(&base_dir, &career_id, &offer_id, accept)
}

/// JanelaConvocacao → BlocoEspecial.
#[tauri::command]
pub fn iniciar_bloco_especial(career_id: String, app: AppHandle) -> Result<(), String> {
    let base_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = career_db_path(&base_dir, &career_id);
    let mut db = Database::open_existing(&db_path).map_err(|e| e.to_string())?;

    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Nenhuma temporada ativa.".to_string())?;

    let selected_offer_id: Option<String> = db
        .conn
        .query_row(
            "SELECT active_offer_id
             FROM special_window_state
             WHERE season_id = ?1 AND player_result = 'selected'
             LIMIT 1",
            rusqlite::params![season.id.clone()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Falha ao carregar resultado da janela especial: {e}"))?
        .flatten();

    if let Some(offer_id) = selected_offer_id {
        let already_has_special =
            contract_queries::has_active_especial_contract(&db.conn, &player.id)
                .map_err(|e| format!("Falha ao verificar contrato especial do jogador: {e}"))?;
        if !already_has_special {
            let offer = get_player_special_offer_by_id_for_season(&db.conn, &season.id, &offer_id)
                .map_err(|e| format!("Falha ao carregar oferta especial selecionada: {e}"))?
                .ok_or_else(|| "Oferta especial selecionada nao encontrada.".to_string())?;

            // BEGIN IMMEDIATE pelo mesmo motivo de `respond_player_special_offer_in_base_dir`:
            // a consolidação lê antes de escrever, e em DEFERRED isso morre com
            // SQLITE_BUSY_SNAPSHOT se outra conexão comitar no meio.
            let tx = db
                .conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(|e| e.to_string())?;
            accept_player_special_offer_tx(&tx, &player, &season, &offer)?;
            tx.commit()
                .map_err(|e| format!("Falha ao consolidar convocacao especial do jogador: {e}"))?;
        }
    }

    iniciar_fn(&db.conn).map_err(|e| e.to_string())
}

/// BlocoEspecial → PosEspecial (fim esportivo das corridas especiais).
#[tauri::command]
pub fn encerrar_bloco_especial(career_id: String, app: AppHandle) -> Result<(), String> {
    let base_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = career_db_path(&base_dir, &career_id);
    let db = Database::open_existing(&db_path).map_err(|e| e.to_string())?;
    encerrar_fn(&db.conn).map_err(|e| e.to_string())
}

/// Desmontagem do bloco especial: expira contratos, limpa lineups, gera notícias.
/// Permanece em PosEspecial após execução.
#[tauri::command]
pub fn run_pos_especial(career_id: String, app: AppHandle) -> Result<PosEspecialResult, String> {
    let base_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = career_db_path(&base_dir, &career_id);
    let db = Database::open_existing(&db_path).map_err(|e| e.to_string())?;
    pos_fn(&db.conn).map_err(|e| e.to_string())
}
