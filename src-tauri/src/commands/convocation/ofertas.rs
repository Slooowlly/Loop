//! Ofertas especiais do jogador: listagem das pendentes, resposta (aceite/recusa) e
//! a transacao que efetiva o contrato especial e reorganiza o lineup da equipe.

use super::*;

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
        .map(|offers| {
            offers
                .into_iter()
                .filter(|offer| {
                    !matches!(
                        offer.special_category.as_str(),
                        "production_challenger" | "endurance"
                    )
                })
                .collect()
        })
        .map_err(|e| format!("Falha ao carregar ofertas especiais: {e}"))
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

pub(super) fn accept_player_special_offer_tx(
    tx: &rusqlite::Transaction<'_>,
    player: &Driver,
    season: &Season,
    offer: &PlayerSpecialOffer,
) -> Result<(), String> {
    if matches!(
        offer.special_category.as_str(),
        "production_challenger" | "endurance"
    ) {
        return Err(
            "Production/Endurance agora usam contratos regulares no BlocoEspecial.".to_string(),
        );
    }

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
