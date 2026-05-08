use std::path::{Path, PathBuf};

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::commands::career_types::SpecialWindowPayload;
use crate::config::app_config::AppConfig;
use crate::convocation::player_offers::{
    expire_remaining_player_special_offers_for_season,
    get_pending_player_special_offers_for_season, get_player_special_offer_by_id_for_season,
    update_player_special_offer_status_for_season,
};
use crate::convocation::special_window;
use crate::convocation::{
    advance_to_convocation_window as adv_fn, encerrar_bloco_especial as encerrar_fn,
    iniciar_bloco_especial as iniciar_fn, run_convocation_window as run_fn,
    run_pos_especial as pos_fn, ConvocationResult, PlayerSpecialOffer, PosEspecialResult,
};
use crate::db::connection::Database;
use crate::db::queries::{
    contracts as contract_queries, drivers as driver_queries, seasons as season_queries,
    teams as team_queries,
};
use crate::generators::ids::{next_id, IdType};
use crate::models::driver::Driver;
use crate::models::enums::{ContractStatus, SeasonPhase, TeamRole};
use crate::models::season::Season;

fn career_db_path(base_dir: &Path, career_id: &str) -> PathBuf {
    let config = AppConfig::load_or_default(base_dir);
    config.saves_dir().join(career_id).join("career.db")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSpecialOfferResponse {
    pub success: bool,
    pub action: String,
    pub message: String,
    pub special_category: Option<String>,
    pub remaining_offers: i32,
}

pub(crate) fn get_player_special_offers_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<Vec<PlayerSpecialOffer>, String> {
    let db_path = career_db_path(base_dir, career_id);
    let db = Database::open_existing(&db_path).map_err(|e| e.to_string())?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Nenhuma temporada ativa.".to_string())?;
    get_pending_player_special_offers_for_season(&db.conn, &season.id, &player.id)
        .map_err(|e| format!("Falha ao carregar ofertas especiais: {e}"))
}

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

pub(crate) fn respond_player_special_offer_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    offer_id: &str,
    accept: bool,
) -> Result<PlayerSpecialOfferResponse, String> {
    let db_path = career_db_path(base_dir, career_id);
    let mut db = Database::open_existing(&db_path).map_err(|e| e.to_string())?;

    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Nenhuma temporada ativa.".to_string())?;

    if season.fase != SeasonPhase::JanelaConvocacao {
        return Err(
            "A resposta da convocacao especial so pode ocorrer na JanelaConvocacao.".to_string(),
        );
    }

    let pending_before =
        get_pending_player_special_offers_for_season(&db.conn, &season.id, &player.id)
            .map_err(|e| format!("Falha ao carregar ofertas especiais pendentes: {e}"))?;

    let offer = get_player_special_offer_by_id_for_season(&db.conn, &season.id, offer_id)
        .map_err(|e| format!("Falha ao carregar oferta especial: {e}"))?
        .ok_or_else(|| "Oferta especial nao encontrada.".to_string())?;
    if offer.player_driver_id != player.id {
        return Err("A oferta especial nao pertence ao jogador.".to_string());
    }
    if offer.status != "Pendente" {
        return Err("A oferta especial nao esta mais pendente.".to_string());
    }

    let response = if accept {
        let team_name = offer.team_name.clone();

        let tx = db.conn.transaction().map_err(|e| e.to_string())?;
        accept_player_special_offer_tx(&tx, &player, &season, &offer)?;
        tx.commit()
            .map_err(|e| format!("Falha ao confirmar aceite da oferta especial: {e}"))?;

        PlayerSpecialOfferResponse {
            success: true,
            action: "accepted".to_string(),
            message: format!("Voce aceitou a convocacao de {}.", team_name),
            special_category: Some(offer.special_category.clone()),
            remaining_offers: 0,
        }
    } else {
        update_player_special_offer_status_for_season(&db.conn, &season.id, offer_id, "Recusada")
            .map_err(|e| format!("Falha ao recusar oferta especial: {e}"))?;
        PlayerSpecialOfferResponse {
            success: true,
            action: "rejected".to_string(),
            message: format!("Voce recusou a convocacao de {}.", offer.team_name),
            special_category: None,
            remaining_offers: pending_before.len().saturating_sub(1) as i32,
        }
    };

    Ok(response)
}

fn accept_player_special_offer_tx(
    tx: &rusqlite::Transaction<'_>,
    player: &Driver,
    season: &Season,
    offer: &PlayerSpecialOffer,
) -> Result<(), String> {
    if contract_queries::has_active_especial_contract(tx, &player.id)
        .map_err(|e| format!("Falha ao verificar contrato especial do jogador: {e}"))?
    {
        return Err("O jogador ja possui contrato especial ativo.".to_string());
    }

    let team = team_queries::get_team_by_id(tx, &offer.team_id)
        .map_err(|e| format!("Falha ao carregar equipe da oferta especial: {e}"))?
        .ok_or_else(|| "Equipe da oferta especial nao encontrada.".to_string())?;

    let displaced_driver_id = match offer.papel {
        TeamRole::Numero1 => team.piloto_1_id.clone(),
        TeamRole::Numero2 => team.piloto_2_id.clone(),
    }
    .filter(|driver_id| driver_id != &player.id);

    if let Some(displaced_driver_id) = &displaced_driver_id {
        if let Some(contract) =
            contract_queries::get_active_especial_contract_for_pilot(tx, displaced_driver_id)
                .map_err(|e| format!("Falha ao localizar contrato especial substituido: {e}"))?
        {
            contract_queries::update_contract_status(tx, &contract.id, &ContractStatus::Rescindido)
                .map_err(|e| format!("Falha ao rescindir contrato especial substituido: {e}"))?;
        }
        driver_queries::update_driver_especial_category(tx, displaced_driver_id, None)
            .map_err(|e| format!("Falha ao liberar piloto substituido do especial: {e}"))?;
    }

    let contract = contract_queries::generate_especial_contract(
        next_id(tx, IdType::Contract).map_err(|e| format!("Falha ao gerar ID de contrato: {e}"))?,
        &player.id,
        &player.nome,
        &team.id,
        &team.nome,
        offer.papel.clone(),
        &offer.special_category,
        &offer.class_name,
        season.numero,
    );
    contract_queries::insert_contract(tx, &contract)
        .map_err(|e| format!("Falha ao criar contrato especial do jogador: {e}"))?;
    driver_queries::update_driver_especial_category(tx, &player.id, Some(&offer.special_category))
        .map_err(|e| format!("Falha ao ativar categoria especial do jogador: {e}"))?;

    let (piloto_1, piloto_2) = place_driver_in_special_team(&team, &player.id, offer.papel.clone());
    team_queries::update_team_pilots(tx, &team.id, piloto_1.as_deref(), piloto_2.as_deref())
        .map_err(|e| format!("Falha ao atualizar lineup da equipe especial: {e}"))?;
    team_queries::update_team_hierarchy(
        tx,
        &team.id,
        piloto_1.as_deref(),
        piloto_2.as_deref(),
        "estavel",
        0.0,
    )
    .map_err(|e| format!("Falha ao atualizar hierarquia da equipe especial: {e}"))?;

    update_player_special_offer_status_for_season(tx, &season.id, &offer.id, "Aceita")
        .map_err(|e| format!("Falha ao marcar oferta especial como aceita: {e}"))?;
    expire_remaining_player_special_offers_for_season(tx, &season.id, &player.id, &offer.id)
        .map_err(|e| format!("Falha ao expirar demais ofertas especiais: {e}"))?;

    Ok(())
}

fn place_driver_in_special_team(
    team: &crate::models::team::Team,
    player_id: &str,
    role: TeamRole,
) -> (Option<String>, Option<String>) {
    let current_n1 = team.piloto_1_id.clone().filter(|id| id != player_id);
    let current_n2 = team.piloto_2_id.clone().filter(|id| id != player_id);

    match role {
        TeamRole::Numero1 => (Some(player_id.to_string()), current_n2),
        TeamRole::Numero2 => (current_n1, Some(player_id.to_string())),
    }
}

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

            let tx = db.conn.transaction().map_err(|e| e.to_string())?;
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

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use rand::{rngs::StdRng, SeedableRng};

    use super::*;
    use crate::convocation::player_offers::get_player_special_offer_by_id;
    use crate::convocation::{advance_to_convocation_window, run_convocation_window};
    use crate::generators::world::generate_world_with_rng;

    fn create_test_base_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("iracerapp_convocation_{label}_{nanos}"))
    }

    fn seed_special_offer_career(base_dir: &Path) {
        let db_path = career_db_path(base_dir, "career_001");
        let db = Database::create_new(&db_path).expect("create db");

        let mut rng = StdRng::seed_from_u64(77);
        let world = generate_world_with_rng(
            "Test Player",
            "🇧🇷 Brasileiro",
            20,
            "mazda_rookie",
            0,
            "medio",
            &mut rng,
        )
        .expect("world generation");

        let season = crate::models::season::Season::new("S001".to_string(), 1, 2024);
        season_queries::insert_season(&db.conn, &season).expect("insert season");
        for driver in &world.drivers {
            driver_queries::insert_driver(&db.conn, driver).expect("insert driver");
        }
        team_queries::insert_teams(&db.conn, &world.teams).expect("insert teams");
        contract_queries::insert_contracts(&db.conn, &world.contracts).expect("insert contracts");
        db.conn
            .execute(
                "UPDATE meta SET value = ?1 WHERE key = 'next_contract_id'",
                rusqlite::params![(world.contracts.len() + 1).to_string()],
            )
            .expect("sync contract ids");

        let mut player = driver_queries::get_player_driver(&db.conn).expect("player");
        player.categoria_atual = Some("gt4".to_string());
        player.atributos.skill = 98.0;
        driver_queries::update_driver(&db.conn, &player).expect("update player");

        advance_to_convocation_window(&db.conn).expect("advance convocation");
        run_convocation_window(&db.conn).expect("run convocation");
    }

    fn assigned_driver_ids(
        conn: &rusqlite::Connection,
        season_id: &str,
    ) -> std::collections::HashSet<String> {
        let mut stmt = conn
            .prepare(
                "SELECT driver_id
                 FROM special_window_assignments
                 WHERE season_id = ?1",
            )
            .expect("prepare assigned ids");
        let rows = stmt
            .query_map(rusqlite::params![season_id], |row| row.get::<_, String>(0))
            .expect("query assigned ids");

        let mut result = std::collections::HashSet::new();
        for row in rows {
            result.insert(row.expect("assigned driver id"));
        }
        result
    }

    fn first_unassigned_driver_in_category(
        conn: &rusqlite::Connection,
        season_id: &str,
        category: &str,
    ) -> crate::models::driver::Driver {
        let assigned = assigned_driver_ids(conn, season_id);
        driver_queries::get_drivers_by_category(conn, category)
            .expect("drivers by category")
            .into_iter()
            .find(|driver| !driver.is_jogador && !assigned.contains(&driver.id))
            .expect("unassigned driver in category")
    }

    #[test]
    fn test_get_player_special_offers_returns_pending_only() {
        let base_dir = create_test_base_dir("list_pending");
        seed_special_offer_career(&base_dir);

        let offers =
            get_player_special_offers_in_base_dir(&base_dir, "career_001").expect("list offers");

        assert!(!offers.is_empty());
        assert!(offers.iter().all(|offer| offer.status == "Pendente"));
    }

    #[test]
    fn test_accept_player_special_offer_activates_contract_and_expires_others() {
        let base_dir = create_test_base_dir("accept_offer");
        seed_special_offer_career(&base_dir);

        let offers =
            get_player_special_offers_in_base_dir(&base_dir, "career_001").expect("list offers");
        assert!(
            offers.len() >= 2,
            "cenário de teste precisa de múltiplas ofertas"
        );

        let response =
            respond_player_special_offer_in_base_dir(&base_dir, "career_001", &offers[0].id, true)
                .expect("accept offer");

        assert_eq!(response.action, "accepted");
        assert_eq!(response.special_category.as_deref(), Some("endurance"));
        assert_eq!(response.remaining_offers, 0);

        let db_path = career_db_path(&base_dir, "career_001");
        let db = Database::open_existing(&db_path).expect("open db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let contract =
            contract_queries::get_active_especial_contract_for_pilot(&db.conn, &player.id)
                .expect("special contract lookup")
                .expect("active special contract");

        assert_eq!(
            player.categoria_especial_ativa.as_deref(),
            Some("endurance")
        );
        assert_eq!(contract.equipe_id, offers[0].team_id);

        let chosen = get_player_special_offer_by_id(&db.conn, &offers[0].id)
            .expect("chosen offer query")
            .expect("chosen offer");
        let other = get_player_special_offer_by_id(&db.conn, &offers[1].id)
            .expect("other offer query")
            .expect("other offer");
        assert_eq!(chosen.status, "Aceita");
        assert_eq!(other.status, "Expirada");
    }

    #[test]
    fn test_reject_player_special_offer_marks_recusada() {
        let base_dir = create_test_base_dir("reject_offer");
        seed_special_offer_career(&base_dir);

        let offers =
            get_player_special_offers_in_base_dir(&base_dir, "career_001").expect("list offers");
        let response =
            respond_player_special_offer_in_base_dir(&base_dir, "career_001", &offers[0].id, false)
                .expect("reject offer");

        assert_eq!(response.action, "rejected");
        assert!(response.remaining_offers >= 0);

        let db_path = career_db_path(&base_dir, "career_001");
        let db = Database::open_existing(&db_path).expect("open db");
        let rejected = get_player_special_offer_by_id(&db.conn, &offers[0].id)
            .expect("offer query")
            .expect("offer");
        assert_eq!(rejected.status, "Recusada");
    }

    #[test]
    fn test_get_player_special_offers_ignores_other_season_offers() {
        let base_dir = create_test_base_dir("ignore_other_season");
        seed_special_offer_career(&base_dir);

        let current_offers =
            get_player_special_offers_in_base_dir(&base_dir, "career_001").expect("list offers");
        let db_path = career_db_path(&base_dir, "career_001");
        let db = Database::open_existing(&db_path).expect("open db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let mut old_season = crate::models::season::Season::new("S999".to_string(), 999, 3024);
        old_season.status = crate::models::enums::SeasonStatus::Finalizada;
        season_queries::insert_season(&db.conn, &old_season).expect("insert old season");

        db.conn
            .execute(
                "INSERT INTO player_special_offers (
                    id, season_id, player_driver_id, team_id, team_name,
                    special_category, class_name, papel, status, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    "PSO-OLD-SEASON",
                    "S999",
                    &player.id,
                    &current_offers[0].team_id,
                    "Equipe Antiga",
                    "endurance",
                    "gt4",
                    TeamRole::Numero1.as_str(),
                    "Pendente",
                    crate::common::time::current_timestamp(),
                ],
            )
            .expect("insert old-season offer");

        let offers =
            get_player_special_offers_in_base_dir(&base_dir, "career_001").expect("list offers");

        assert!(
            offers.iter().all(|offer| offer.id != "PSO-OLD-SEASON"),
            "ofertas de temporada antiga nao deveriam aparecer na listagem atual"
        );
    }

    #[test]
    fn test_cannot_accept_already_resolved_special_offer() {
        let base_dir = create_test_base_dir("accept_resolved");
        seed_special_offer_career(&base_dir);

        let offers =
            get_player_special_offers_in_base_dir(&base_dir, "career_001").expect("list offers");
        respond_player_special_offer_in_base_dir(&base_dir, "career_001", &offers[0].id, true)
            .expect("first accept");

        let error =
            respond_player_special_offer_in_base_dir(&base_dir, "career_001", &offers[1].id, true)
                .expect_err("resolved offer should not be accepted");
        assert!(error.contains("nao esta mais pendente"));
    }

    #[test]
    fn test_cannot_accept_offer_from_other_season() {
        let base_dir = create_test_base_dir("accept_other_season");
        seed_special_offer_career(&base_dir);

        let current_offers =
            get_player_special_offers_in_base_dir(&base_dir, "career_001").expect("list offers");
        let db_path = career_db_path(&base_dir, "career_001");
        let db = Database::open_existing(&db_path).expect("open db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let mut old_season = crate::models::season::Season::new("S999".to_string(), 999, 3024);
        old_season.status = crate::models::enums::SeasonStatus::Finalizada;
        season_queries::insert_season(&db.conn, &old_season).expect("insert old season");

        db.conn
            .execute(
                "INSERT INTO player_special_offers (
                    id, season_id, player_driver_id, team_id, team_name,
                    special_category, class_name, papel, status, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    "PSO-FOREIGN-SEASON",
                    "S999",
                    &player.id,
                    &current_offers[0].team_id,
                    "Equipe Antiga",
                    "endurance",
                    "gt4",
                    TeamRole::Numero1.as_str(),
                    "Pendente",
                    crate::common::time::current_timestamp(),
                ],
            )
            .expect("insert old-season offer");

        let error = respond_player_special_offer_in_base_dir(
            &base_dir,
            "career_001",
            "PSO-FOREIGN-SEASON",
            true,
        )
        .expect_err("foreign season offer should not be accepted");

        assert!(error.contains("nao encontrada"));
    }

    #[test]
    fn test_special_window_state_starts_on_day_one_with_daily_payload() {
        let base_dir = create_test_base_dir("window_state_day_one");
        seed_special_offer_career(&base_dir);

        let state = get_special_window_state_in_base_dir(&base_dir, "career_001")
            .expect("load special window state");

        assert_eq!(state.current_day, 1);
        assert_eq!(state.total_days, 7);
        assert!(!state.team_sections.is_empty());
        assert!(state.eligible_candidates.iter().all(|candidate| {
            matches!(
                candidate.origin_category.as_str(),
                "mazda_amador" | "toyota_amador" | "bmw_m2" | "gt4" | "gt3" | "lmp2"
            )
        }));
        assert!(
            state.last_day_log.is_empty(),
            "o dia 1 nao deve mostrar fechamento antes do primeiro avancar"
        );
        assert!(state
            .team_sections
            .iter()
            .flat_map(|section| section.teams.iter())
            .all(|team| team.piloto_1_new_badge_day.is_none()
                && team.piloto_2_new_badge_day.is_none()));
        let visible_pilots = state
            .team_sections
            .iter()
            .flat_map(|section| section.teams.iter())
            .filter(|team| team.piloto_1_nome.is_some() || team.piloto_2_nome.is_some())
            .count();
        assert!(
            visible_pilots > 0,
            "o grid especial precisa nascer com alguns pilotos ja revelados no dia 1"
        );
    }

    #[test]
    fn test_accept_special_offer_for_day_keeps_single_active_choice() {
        let base_dir = create_test_base_dir("single_daily_choice");
        seed_special_offer_career(&base_dir);

        advance_special_window_day_in_base_dir(&base_dir, "career_001").expect("advance to day 2");
        advance_special_window_day_in_base_dir(&base_dir, "career_001").expect("advance to day 3");

        let state = get_special_window_state_in_base_dir(&base_dir, "career_001")
            .expect("load special window state");
        assert!(
            state.player_offers.len() >= 2,
            "cenario precisa de pelo menos duas ofertas para testar a troca diaria"
        );

        accept_special_offer_for_day_in_base_dir(
            &base_dir,
            "career_001",
            &state.player_offers[0].id,
        )
        .expect("accept first offer");

        let updated = accept_special_offer_for_day_in_base_dir(
            &base_dir,
            "career_001",
            &state.player_offers[1].id,
        )
        .expect("switch active offer");

        let active_count = updated
            .player_offers
            .iter()
            .filter(|offer| offer.status == "AceitaAtiva")
            .count();
        assert_eq!(active_count, 1);
        assert_eq!(
            updated.active_offer_id.as_deref(),
            Some(state.player_offers[1].id.as_str())
        );
    }

    #[test]
    fn test_advance_special_window_day_reveals_market_movements() {
        let base_dir = create_test_base_dir("advance_special_window_day");
        seed_special_offer_career(&base_dir);

        let before = get_special_window_state_in_base_dir(&base_dir, "career_001")
            .expect("load window before advance");
        let advanced = advance_special_window_day_in_base_dir(&base_dir, "career_001")
            .expect("advance special window day");

        assert_eq!(advanced.current_day, before.current_day + 1);
        assert!(
            !advanced.last_day_log.is_empty(),
            "avancar o dia deve produzir eventos de mercado"
        );
        assert!(
            advanced.last_day_log.iter().all(|entry| {
                entry.driver_name.is_some()
                    && entry.team_name.is_some()
                    && entry.driver_origin_category.is_some()
                    && entry.driver_license_sigla.is_some()
            }),
            "eventos de convocacao precisam carregar dados estruturados para o painel visual"
        );
    }

    #[test]
    fn test_special_window_eligible_candidates_show_only_current_main_names() {
        let base_dir = create_test_base_dir("eligible_shortlist");
        seed_special_offer_career(&base_dir);

        let db_path = career_db_path(&base_dir, "career_001");
        let db = Database::open_existing(&db_path).expect("open db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("active season")
            .expect("season");

        let mut top_amador =
            first_unassigned_driver_in_category(&db.conn, &season.id, "mazda_amador");
        let mut second_amador =
            first_unassigned_driver_in_category(&db.conn, &season.id, "toyota_amador");
        second_amador.categoria_atual = Some("mazda_amador".to_string());
        let mut rookie = first_unassigned_driver_in_category(&db.conn, &season.id, "mazda_rookie");
        let unemployed = first_unassigned_driver_in_category(&db.conn, &season.id, "gt4");

        top_amador.stats_temporada.pontos = 250.0;
        top_amador.stats_temporada.vitorias = 4;
        top_amador.stats_temporada.podios = 7;
        driver_queries::update_driver(&db.conn, &top_amador).expect("update top amador");

        second_amador.stats_temporada.pontos = 120.0;
        second_amador.stats_temporada.vitorias = 1;
        second_amador.stats_temporada.podios = 3;
        driver_queries::update_driver(&db.conn, &second_amador).expect("update second amador");

        rookie.stats_temporada.pontos = 999.0;
        rookie.stats_temporada.vitorias = 8;
        rookie.stats_temporada.podios = 8;
        driver_queries::update_driver(&db.conn, &rookie).expect("update rookie");

        let unemployed_contract =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &unemployed.id)
                .expect("active regular contract")
                .expect("unemployed regular contract");
        contract_queries::update_contract_status(
            &db.conn,
            &unemployed_contract.id,
            &ContractStatus::Rescindido,
        )
        .expect("expire unemployed regular contract");

        db.conn
            .execute(
                "INSERT OR REPLACE INTO special_window_candidate_pool (
                    season_id, driver_id, driver_name, origin_category, license_level,
                    desirability, production_eligible, endurance_eligible, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'Livre')",
                rusqlite::params![
                    &season.id,
                    &top_amador.id,
                    &top_amador.nome,
                    "mazda_amador",
                    2_i64,
                    84_i32,
                    0_i64,
                    0_i64,
                ],
            )
            .expect("upsert top amador");
        db.conn
            .execute(
                "INSERT OR REPLACE INTO special_window_candidate_pool (
                    season_id, driver_id, driver_name, origin_category, license_level,
                    desirability, production_eligible, endurance_eligible, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'Livre')",
                rusqlite::params![
                    &season.id,
                    &second_amador.id,
                    &second_amador.nome,
                    "mazda_amador",
                    2_i64,
                    99_i32,
                    0_i64,
                    0_i64,
                ],
            )
            .expect("upsert second amador");
        db.conn
            .execute(
                "INSERT OR REPLACE INTO special_window_candidate_pool (
                    season_id, driver_id, driver_name, origin_category, license_level,
                    desirability, production_eligible, endurance_eligible, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'Livre')",
                rusqlite::params![
                    &season.id,
                    &rookie.id,
                    &rookie.nome,
                    "mazda_rookie",
                    1_i64,
                    110_i32,
                    0_i64,
                    0_i64,
                ],
            )
            .expect("upsert rookie");
        db.conn
            .execute(
                "INSERT OR REPLACE INTO special_window_candidate_pool (
                    season_id, driver_id, driver_name, origin_category, license_level,
                    desirability, production_eligible, endurance_eligible, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'Livre')",
                rusqlite::params![
                    &season.id,
                    &unemployed.id,
                    &unemployed.nome,
                    "gt4",
                    4_i64,
                    101_i32,
                    0_i64,
                    0_i64,
                ],
            )
            .expect("upsert unemployed");

        let state = get_special_window_state_in_base_dir(&base_dir, "career_001")
            .expect("load special window state");

        let visible_names: Vec<_> = state
            .eligible_candidates
            .iter()
            .map(|candidate| candidate.driver_name.as_str())
            .collect();
        assert!(
            !visible_names.contains(&rookie.nome.as_str()),
            "rookies nao devem inflar a shortlist visivel"
        );
        assert!(
            !visible_names.contains(&unemployed.nome.as_str()),
            "pilotos sem contrato regular ativo nao devem aparecer como nomes principais"
        );

        let mazda_amador_names: Vec<_> = state
            .eligible_candidates
            .iter()
            .filter(|candidate| candidate.origin_category == "mazda_amador")
            .map(|candidate| candidate.driver_name.as_str())
            .collect();
        assert!(
            mazda_amador_names.len() >= 2,
            "cenario precisa manter pelo menos dois nomes principais do mesmo grid"
        );
        assert_eq!(
            mazda_amador_names[0],
            top_amador.nome.as_str(),
            "a shortlist visivel deve priorizar quem terminou melhor o campeonato"
        );
        let second_index = mazda_amador_names
            .iter()
            .position(|name| *name == second_amador.nome.as_str())
            .expect("second amador visible in shortlist");
        assert!(
            second_index > 0,
            "o segundo nome precisa aparecer atras do lider visivel do campeonato"
        );
    }

    #[test]
    fn test_special_window_eligible_candidates_use_regular_contract_category_when_driver_current_category_is_null(
    ) {
        let base_dir = create_test_base_dir("eligible_contract_fallback");
        seed_special_offer_career(&base_dir);

        let db_path = career_db_path(&base_dir, "career_001");
        let db = Database::open_existing(&db_path).expect("open db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("active season")
            .expect("season");

        let mut contracted_gt4 = first_unassigned_driver_in_category(&db.conn, &season.id, "gt4");
        contracted_gt4.categoria_atual = None;
        contracted_gt4.stats_temporada.pontos = 320.0;
        contracted_gt4.stats_temporada.vitorias = 5;
        driver_queries::update_driver(&db.conn, &contracted_gt4).expect("update gt4 driver");

        db.conn
            .execute(
                "INSERT OR REPLACE INTO special_window_candidate_pool (
                    season_id, driver_id, driver_name, origin_category, license_level,
                    desirability, production_eligible, endurance_eligible, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'Livre')",
                rusqlite::params![
                    &season.id,
                    &contracted_gt4.id,
                    &contracted_gt4.nome,
                    "bmw_m2",
                    1_i64,
                    95_i32,
                    1_i64,
                    0_i64,
                ],
            )
            .expect("upsert fallback candidate");

        let state = get_special_window_state_in_base_dir(&base_dir, "career_001")
            .expect("load special window state");

        let visible = state
            .eligible_candidates
            .iter()
            .find(|candidate| candidate.driver_id == contracted_gt4.id)
            .expect("driver visible in shortlist");

        assert_eq!(visible.origin_category, "gt4");
        assert!(visible.endurance_eligible);
        assert!(!visible.production_eligible);
    }
}
