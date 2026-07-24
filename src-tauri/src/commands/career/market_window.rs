//! Janela de mercado e pre-temporada: avanco de semana, estado do plano, quebra de
//! contrato do jogador, propostas recebidas e o fecho da pre-temporada.

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerProposalView {
    pub proposal_id: String,
    pub equipe_id: String,
    pub equipe_nome: String,
    pub equipe_cor_primaria: String,
    pub equipe_cor_secundaria: String,
    pub categoria: String,
    pub categoria_nome: String,
    pub categoria_tier: u8,
    pub papel: String,
    pub salario_oferecido: f64,
    pub duracao_anos: i32,
    pub car_performance: f64,
    pub car_performance_rating: u8,
    pub reputacao: f64,
    pub companheiro_nome: Option<String>,
    pub companheiro_skill: Option<u8>,
    pub status: String,
    /// Semanas até a proposta expirar (Fase B). `None` = sem prazo (proposta de rollover).
    pub semanas_restantes: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalResponse {
    pub success: bool,
    pub action: String,
    pub message: String,
    pub new_team_name: Option<String>,
    pub remaining_proposals: i32,
    pub news_generated: Vec<String>,
}

pub(crate) fn advance_market_week_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    accepted_seat_id: Option<&str>,
) -> Result<WeekResult, String> {
    let _career_number =
        career_number_from_id(career_id).ok_or_else(|| "ID de carreira invalido.".to_string())?;
    let (db, career_dir, mut meta) = open_career_resources(base_dir, career_id)?;
    let meta_path = career_dir.join("meta.json");
    let mut plan = load_preseason_plan(&career_dir)?
        .ok_or_else(|| "Plano da pre-temporada nao encontrado.".to_string())?;
    let tx = db
        .conn
        .unchecked_transaction()
        .map_err(|e| format!("Falha ao iniciar transacao da semana de mercado: {e}"))?;
    let result = advance_week(&tx, &mut plan, accepted_seat_id)?;
    warn_if_noncritical(
        persist_market_week_news(&tx, &plan.state, &result),
        "Falha ao persistir noticias da semana de mercado",
    );
    crate::market::preseason::save_preseason_plan(&career_dir, &plan)?;
    tx.commit()
        .map_err(|e| format!("Falha ao confirmar semana de mercado: {e}"))?;

    meta.last_played = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    warn_if_noncritical(
        write_save_meta(&meta_path, &meta),
        "Falha ao atualizar meta.json apos avancar semana de mercado",
    );
    Ok(result)
}

pub(crate) fn get_preseason_state_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<PreSeasonState, String> {
    let (db, career_dir, _) = open_career_resources_read_only(base_dir, career_id)?;
    let mut plan = load_preseason_plan(&career_dir)?
        .ok_or_else(|| "Plano da pre-temporada nao encontrado.".to_string())?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada da pre-temporada: {e}"))?
        .ok_or_else(|| format!("Temporada {} nao encontrada", plan.state.season_number))?;
    if season.numero != plan.state.season_number {
        return Err(format!(
            "Plano de pre-temporada desatualizado para a temporada ativa {}.",
            season.numero
        ));
    }
    crate::market::preseason::refresh_preseason_state_display_date(
        &db.conn,
        &season.id,
        &mut plan.state,
    )?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    plan.state.player_has_team =
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
            .map(|c| c.is_some())
            .unwrap_or(false);
    Ok(plan.state)
}

/// Quebra de contrato do jogador (Fase 2b.3): a oferta guardada no plano da janela,
/// ou None. Enriquecida no setup (`compute_player_poach_offer`); só leitura aqui.
pub(crate) fn get_player_poach_offer_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<Option<crate::market::pipeline::PlayerPoachOffer>, String> {
    let (_, career_dir, _) = open_career_resources_read_only(base_dir, career_id)?;
    let plan = load_preseason_plan(&career_dir)?
        .ok_or_else(|| "Plano da pre-temporada nao encontrado.".to_string())?;
    Ok(plan.player_poach_offer)
}

/// Resolve a decisão do jogador na quebra de contrato: `accept = true` sai pro
/// pretendente, `false` fica no time atual. Recebe a oferta que o jogador VIU (a UI a
/// tem em mãos), aplica no banco e — se existir um plano de janela — limpa a oferta
/// dele. Independente do plano, pra o debug funcionar de qualquer tela.
pub(crate) fn resolve_player_poach_offer_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    offer: &crate::market::pipeline::PlayerPoachOffer,
    accept: bool,
) -> Result<crate::market::pipeline::PlayerPoachOutcome, String> {
    let (db, career_dir, _) = open_career_resources(base_dir, career_id)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Nenhuma temporada ativa.".to_string())?;

    let tx = db
        .conn
        .unchecked_transaction()
        .map_err(|e| format!("Falha ao iniciar transacao da quebra de contrato: {e}"))?;
    let outcome = crate::market::pipeline::resolve_player_poach(&tx, offer, accept, season.numero)?;
    tx.commit()
        .map_err(|e| format!("Falha ao confirmar quebra de contrato: {e}"))?;

    // Consome a oferta do plano da janela, se houver (uma decisão por janela).
    if let Ok(Some(mut plan)) = load_preseason_plan(&career_dir) {
        if plan.player_poach_offer.is_some() {
            plan.player_poach_offer = None;
            plan.state.player_has_team = true;
            crate::market::preseason::save_preseason_plan(&career_dir, &plan)?;
        }
    }
    Ok(outcome)
}

/// DEBUG: relatório do dry-run do leilão de poaching (Fase 2b.2).
#[derive(Debug, Serialize)]
pub(crate) struct PoachDebugReport {
    /// `true` = o mundo foi TEMPERADO pra garantir briga (não é o estado real do save).
    pub forced: bool,
    pub nota: String,
    pub auctions: Vec<crate::market::pipeline::PoachAudit>,
}

pub(crate) fn get_player_proposals_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<Vec<PlayerProposalView>, String> {
    let (db, career_dir, _meta) = open_career_resources(base_dir, career_id)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let mut proposals =
        market_proposal_queries::get_pending_player_proposals(&db.conn, &season.id, &player.id)
            .map_err(|e| format!("Falha ao buscar propostas pendentes: {e}"))?
            .into_iter()
            .map(|proposal| build_player_proposal_view(&db.conn, &proposal))
            .collect::<Result<Vec<_>, _>>()?;
    proposals.sort_by(|a, b| b.car_performance.total_cmp(&a.car_performance));

    // Prazo no card (Fase B): semanas restantes = semana_limite − semana atual da janela.
    let current_week = load_preseason_plan(&career_dir)
        .ok()
        .flatten()
        .map(|plan| plan.state.current_week);
    if let Some(week) = current_week {
        let mut stmt = db
            .conn
            .prepare(
                "SELECT id, semana_limite FROM market_proposals
                 WHERE temporada_id = ?1 AND piloto_id = ?2 AND status = 'Pendente'
                   AND semana_limite IS NOT NULL",
            )
            .map_err(|e| format!("Falha ao preparar prazos das propostas: {e}"))?;
        let deadlines: std::collections::HashMap<String, i32> = stmt
            .query_map(rusqlite::params![season.id, player.id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
            })
            .map_err(|e| format!("Falha ao consultar prazos: {e}"))?
            .collect::<Result<_, _>>()
            .map_err(|e| format!("Falha ao ler prazos: {e}"))?;
        for view in &mut proposals {
            if let Some(limite) = deadlines.get(&view.proposal_id) {
                view.semanas_restantes = Some((limite - week).max(0));
            }
        }
    }
    Ok(proposals)
}

pub(crate) fn respond_to_proposal_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    proposal_id: &str,
    accept: bool,
) -> Result<ProposalResponse, String> {
    let (mut db, career_dir, _meta) = open_career_resources(base_dir, career_id)?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let proposal =
        market_proposal_queries::get_market_proposal_by_id(&db.conn, &season.id, proposal_id)
            .map_err(|e| format!("Falha ao carregar proposta: {e}"))?
            .ok_or_else(|| "Proposta nao encontrada.".to_string())?;
    if proposal.piloto_id != player.id {
        return Err("A proposta nao pertence ao jogador.".to_string());
    }
    if proposal.status != ProposalStatus::Pendente {
        return Err("A proposta nao esta mais pendente.".to_string());
    }

    let mut news_items = Vec::new();
    let mut new_team_name = None;
    let action = if accept { "accepted" } else { "rejected" }.to_string();

    if accept {
        let tx = db
            .conn
            .transaction()
            .map_err(|e| format!("Falha ao iniciar transacao de aceite: {e}"))?;
        accept_player_proposal_tx(&tx, &player, &season, &proposal)?;
        tx.commit()
            .map_err(|e| format!("Falha ao confirmar aceite da proposta: {e}"))?;

        warn_if_noncritical(
            reconcile_plan_after_player_accept(&career_dir, &db.conn, &proposal),
            "Falha ao reconciliar plano apos aceite da proposta",
        );
        new_team_name = Some(proposal.equipe_nome.clone());
    } else {
        let tx = db
            .conn
            .transaction()
            .map_err(|e| format!("Falha ao iniciar transacao de recusa: {e}"))?;
        market_proposal_queries::update_proposal_status(
            &tx,
            &proposal.id,
            "Recusada",
            Some("Jogador recusou a proposta"),
        )
        .map_err(|e| format!("Falha ao recusar proposta: {e}"))?;
        tx.commit()
            .map_err(|e| format!("Falha ao confirmar recusa da proposta: {e}"))?;
    }

    let mut remaining =
        market_proposal_queries::count_pending_player_proposals(&db.conn, &season.id, &player.id)
            .map_err(|e| format!("Falha ao contar propostas pendentes: {e}"))?;

    if !accept
        && remaining == 0
        && contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
            .map_err(|e| format!("Falha ao verificar equipe regular do jogador: {e}"))?
            .is_none()
    {
        let emergency = generate_emergency_player_proposals(&db.conn, &player, &season)?;
        if emergency.is_empty() {
            if let Some(team_name) =
                force_place_player(&db.conn, &player, &season, &mut news_items)?
            {
                new_team_name = Some(team_name);
            }
        } else {
            remaining = emergency.len() as i32;
        }
    }

    warn_if_noncritical(
        sync_preseason_pending_flag(&career_dir, remaining > 0),
        "Falha ao sincronizar indicador de propostas pendentes",
    );
    let headlines = news_items
        .iter()
        .map(|item| item.titulo.clone())
        .collect::<Vec<_>>();

    let message = if accept {
        let role = if proposal.papel == TeamRole::Numero1 {
            "N1"
        } else {
            "N2"
        };
        rust_i18n::t!(
            "career.message.proposal_signed",
            team = proposal.equipe_nome.as_str(),
            role = role
        )
        .to_string()
    } else if let Some(team_name) = &new_team_name {
        rust_i18n::t!(
            "career.message.proposal_declined_reallocated",
            team = proposal.equipe_nome.as_str(),
            new_team = team_name.as_str()
        )
        .to_string()
    } else if remaining > 0 {
        rust_i18n::t!(
            "career.message.proposal_declined_emergency",
            team = proposal.equipe_nome.as_str()
        )
        .to_string()
    } else {
        rust_i18n::t!(
            "career.message.proposal_declined",
            team = proposal.equipe_nome.as_str()
        )
        .to_string()
    };

    Ok(ProposalResponse {
        success: true,
        action,
        message,
        new_team_name,
        remaining_proposals: remaining,
        news_generated: headlines,
    })
}

pub(crate) fn finalize_preseason_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<(), String> {
    let (db, career_dir, mut meta) = open_career_resources(base_dir, career_id)?;
    let meta_path = career_dir.join("meta.json");
    let plan = load_preseason_plan(&career_dir)?
        .ok_or_else(|| "Plano da pre-temporada nao encontrado.".to_string())?;
    if !plan.state.is_complete {
        return Err("Pre-temporada ainda nao foi concluida.".to_string());
    }

    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    // Gate da Janela de Transferências: a pré-temporada só finaliza quando a janela
    // fechou (plan.state.is_complete, garantido acima). O jogador já está garantido
    // num assento pela garantia de porta no fecho — não há mais "propostas pendentes".

    let mut rng = rand::thread_rng();

    // 1. Invariante: Garantir que todas as equipes regulares tenham lineup completo antes de iniciar
    fill_all_remaining_vacancies(&db.conn, season.numero, &mut rng)
        .map_err(|e| format!("Falha ao preencher vagas remanescentes: {e}"))?;

    // 1b. Invariante: Garantir que N1/N2 de toda equipe regular está alinhado com o lineup final.
    // Normaliza equipes preenchidas por fallback que não passaram pelo UpdateHierarchy do mercado.
    crate::hierarchy::transition::validate_and_normalize_team_hierarchies(&db.conn)?;

    if season.fase == SeasonPhase::PreTemporada {
        season_queries::update_season_fase(&db.conn, &season.id, &SeasonPhase::Temporada)
            .map_err(|e| format!("Falha ao iniciar temporada apos pre-temporada: {e}"))?;
    }

    // 2. Limpar artefatos da corrida anterior (cache do dashboard)
    let results_path = career_dir.join("race_results.json");
    if results_path.exists() {
        let _ = std::fs::remove_file(&results_path);
    }

    delete_preseason_plan(&career_dir)?;
    delete_resume_context(&career_dir)?;
    meta.last_played = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    write_save_meta(&meta_path, &meta)?;

    Ok(())
}

/// Categorias onde um piloto pode realmente pegar vaga, espelhando o predicado
/// `eligible` do motor de transferências: tier do assento em [tier−1, tier+1] E licença
/// suficiente (com a exceção do +1 tier, cuja licença é concedida na assinatura).
/// Não considera o bônus de craque (skill≥80 pula 2 tiers) — sem skill neste payload.
pub(crate) fn eligible_categories_for(driver_tier: u8, license: u8) -> Vec<String> {
    crate::constants::categories::get_all_categories()
        .iter()
        .filter(|cat| {
            let seat_tier = cat.tier;
            let required = cat.licenca_necessaria.unwrap_or(0);
            let close = (seat_tier as i16 - driver_tier as i16).abs() <= 1;
            let promotion = seat_tier == driver_tier + 1;
            close && (license >= required || promotion)
        })
        .map(|cat| cat.id.to_string())
        .collect()
}

pub(crate) fn get_preseason_free_agents_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<Vec<crate::commands::career_types::FreeAgentPreview>, String> {
    let (db, _, _) = open_career_resources_read_only(base_dir, career_id)?;
    let raw = contract_queries::get_free_agents_for_preseason(&db.conn)
        .map_err(|e| format!("Falha ao buscar agentes livres: {e}"))?;

    let result = raw
        .into_iter()
        .map(|r| {
            let abbr = r
                .previous_team_name
                .as_deref()
                .map(|name| name.chars().take(3).collect::<String>().to_uppercase());
            let (license_nivel, license_sigla) = match r.max_license_level {
                Some(0) => ("Rookie", "R"),
                Some(1) => ("Amador", "A"),
                Some(2) => ("Pro", "P"),
                Some(3) => ("Super Pro", "SP"),
                Some(4) => ("Elite", "E"),
                Some(_) => ("Super Elite", "SE"),
                None => ("Rookie", "R"),
            };
            // Faixa de nível (tier da categoria onde corre hoje) — chave do agrupamento.
            let market_tier = crate::constants::categories::get_category_config(&r.categoria)
                .map(|config| config.tier);
            let eligible_categories =
                eligible_categories_for(market_tier.unwrap_or(0), r.max_license_level.unwrap_or(0));
            crate::commands::career_types::FreeAgentPreview {
                driver_id: r.driver_id,
                driver_name: r.driver_name,
                categoria: r.categoria,
                is_rookie: r.is_rookie,
                previous_team_name: r.previous_team_name,
                previous_team_color: r.previous_team_color,
                previous_team_abbr: abbr,
                seasons_at_last_team: r.seasons_at_last_team,
                total_career_seasons: r.total_career_seasons,
                license_nivel: license_nivel.to_string(),
                license_sigla: license_sigla.to_string(),
                last_championship_position: r.last_championship_position,
                last_championship_total_drivers: r.last_championship_total_drivers,
                market_tier,
                seasons_idle: r.seasons_idle,
                eligible_categories,
            }
        })
        .collect();

    Ok(result)
}

pub(crate) fn persist_market_week_news(
    _conn: &rusqlite::Connection,
    _state: &PreSeasonState,
    _week_result: &WeekResult,
) -> Result<(), String> {
    Ok(())
}

pub(crate) fn build_player_proposal_view(
    conn: &rusqlite::Connection,
    proposal: &MarketProposal,
) -> Result<PlayerProposalView, String> {
    let team = team_queries::get_team_by_id(conn, &proposal.equipe_id)
        .map_err(|e| format!("Falha ao carregar equipe da proposta: {e}"))?
        .ok_or_else(|| "Equipe da proposta nao encontrada.".to_string())?;
    let category = categories::get_category_config(&team.categoria)
        .ok_or_else(|| format!("Categoria '{}' nao encontrada", team.categoria))?;
    let companion_id = match proposal.papel {
        TeamRole::Numero1 => team
            .piloto_2_id
            .clone()
            .or_else(|| team.piloto_1_id.clone()),
        TeamRole::Numero2 => team
            .piloto_1_id
            .clone()
            .or_else(|| team.piloto_2_id.clone()),
    };
    let companion = companion_id
        .as_deref()
        .map(|id| driver_queries::get_driver(conn, id))
        .transpose()
        .map_err(|e| format!("Falha ao carregar companheiro de equipe: {e}"))?;
    Ok(PlayerProposalView {
        proposal_id: proposal.id.clone(),
        equipe_id: team.id.clone(),
        equipe_nome: team.nome.clone(),
        equipe_cor_primaria: team.cor_primaria.clone(),
        equipe_cor_secundaria: team.cor_secundaria.clone(),
        categoria: team.categoria.clone(),
        categoria_nome: category.nome_curto.to_string(),
        categoria_tier: category.tier,
        papel: if proposal.papel == TeamRole::Numero1 {
            "N1".to_string()
        } else {
            "N2".to_string()
        },
        salario_oferecido: proposal.salario_oferecido,
        duracao_anos: proposal.duracao_anos,
        car_performance: team.effective_car_performance(),
        car_performance_rating: normalize_car_performance(team.effective_car_performance()),
        reputacao: team.reputacao,
        companheiro_nome: companion.as_ref().map(|driver| driver.nome.clone()),
        companheiro_skill: companion
            .as_ref()
            .map(|driver| driver.atributos.skill.round().clamp(0.0, 100.0) as u8),
        status: proposal.status.as_str().to_string(),
        semanas_restantes: None, // preenchido pelo chamador que conhece a semana atual
    })
}

pub(crate) fn accept_player_proposal_tx(
    tx: &rusqlite::Transaction<'_>,
    player: &Driver,
    season: &Season,
    proposal: &MarketProposal,
) -> Result<(), String> {
    let previous_contract = contract_queries::get_active_regular_contract_for_pilot(tx, &player.id)
        .map_err(|e| format!("Falha ao buscar contrato regular atual do jogador: {e}"))?;
    let previous_team_id = previous_contract
        .as_ref()
        .map(|contract| contract.equipe_id.clone());

    if let Some(contract) = previous_contract {
        contract_queries::update_contract_status(tx, &contract.id, &ContractStatus::Rescindido)
            .map_err(|e| format!("Falha ao rescindir contrato atual: {e}"))?;
        team_queries::remove_pilot_from_team(tx, &player.id, &contract.equipe_id)
            .map_err(|e| format!("Falha ao remover jogador da equipe antiga: {e}"))?;
        refresh_team_hierarchy_now(tx, &contract.equipe_id)?;
    }

    let team = team_queries::get_team_by_id(tx, &proposal.equipe_id)
        .map_err(|e| format!("Falha ao carregar equipe da proposta: {e}"))?
        .ok_or_else(|| "Equipe da proposta nao encontrada.".to_string())?;
    ensure_driver_can_join_division(
        tx,
        &player.id,
        &player.nome,
        &team.categoria,
        team.classe.as_deref(),
    )?;
    let mut contract = crate::models::contract::Contract::new(
        next_id(tx, IdType::Contract).map_err(|e| format!("Falha ao gerar ID de contrato: {e}"))?,
        player.id.clone(),
        player.nome.clone(),
        team.id.clone(),
        team.nome.clone(),
        season.numero,
        proposal.duracao_anos,
        proposal.salario_oferecido,
        proposal.papel.clone(),
        team.categoria.clone(),
    );
    contract.classe = team.classe.clone();
    contract_queries::insert_contract(tx, &contract)
        .map_err(|e| format!("Falha ao criar novo contrato do jogador: {e}"))?;
    normalize_regular_contracts_for_team(tx, &team.id)?;
    refresh_team_hierarchy_now(tx, &team.id)?;

    let mut updated_player = player.clone();
    updated_player.categoria_atual = Some(team.categoria.clone());
    updated_player.status = crate::models::enums::DriverStatus::Ativo;
    driver_queries::update_driver(tx, &updated_player)
        .map_err(|e| format!("Falha ao atualizar categoria do jogador: {e}"))?;

    market_proposal_queries::update_proposal_status(tx, &proposal.id, "Aceita", None)
        .map_err(|e| format!("Falha ao marcar proposta como aceita: {e}"))?;
    market_proposal_queries::expire_remaining_proposals(tx, &season.id, &player.id, &proposal.id)
        .map_err(|e| format!("Falha ao expirar demais propostas: {e}"))?;

    if let Some(previous_team_id) = previous_team_id.filter(|old_team| old_team != &team.id) {
        backfill_team_vacancy(tx, &previous_team_id, season.numero, season.ano)?;
        refresh_team_hierarchy_now(tx, &previous_team_id)?;
    }

    Ok(())
}

pub(crate) fn pending_player_event_team_ids(
    event: &PendingAction,
    player_id: &str,
) -> Option<Vec<String>> {
    match event {
        PendingAction::ExpireContract {
            driver_id, team_id, ..
        } if driver_id == player_id => Some(vec![team_id.clone()]),
        PendingAction::RenewContract {
            driver_id, team_id, ..
        } if driver_id == player_id => Some(vec![team_id.clone()]),
        PendingAction::Transfer {
            driver_id,
            from_team_id,
            to_team_id,
            ..
        } if driver_id == player_id => {
            let mut team_ids = Vec::new();
            if let Some(from_team_id) = from_team_id {
                team_ids.push(from_team_id.clone());
            }
            team_ids.push(to_team_id.clone());
            Some(team_ids)
        }
        PendingAction::PlayerProposal { proposal } if proposal.piloto_id == player_id => {
            Some(vec![proposal.equipe_id.clone()])
        }
        PendingAction::PlaceRookie {
            driver, team_id, ..
        } if driver.id == player_id => Some(vec![team_id.clone()]),
        _ => None,
    }
}

pub(crate) fn is_team_role_vacant(
    conn: &rusqlite::Connection,
    team_id: &str,
    role: &str,
) -> Result<bool, String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar equipe para validar vaga: {e}"))?
        .ok_or_else(|| "Equipe nao encontrada para validar vaga.".to_string())?;
    let is_vacant = match TeamRole::from_str_strict(role)
        .map_err(|e| format!("Papel de equipe invalido ao validar vaga: {e}"))?
    {
        TeamRole::Numero1 => team.piloto_1_id.is_none(),
        TeamRole::Numero2 => team.piloto_2_id.is_none(),
    };
    Ok(is_vacant)
}

pub(crate) fn reconcile_plan_after_player_accept(
    career_dir: &Path,
    conn: &rusqlite::Connection,
    proposal: &MarketProposal,
) -> Result<(), String> {
    let Some(mut plan) = load_preseason_plan(career_dir)? else {
        return Ok(());
    };
    let mut affected_team_ids = HashSet::from([proposal.equipe_id.clone()]);
    plan.planned_events.retain(|event| {
        if event.executed {
            return true;
        }
        if let Some(team_ids) = pending_player_event_team_ids(&event.event, &proposal.piloto_id) {
            affected_team_ids.extend(team_ids);
            return false;
        }
        true
    });

    let stale_rookie_indices = plan
        .planned_events
        .iter()
        .enumerate()
        .filter(|(_, event)| !event.executed)
        .filter_map(|(index, event)| match &event.event {
            PendingAction::PlaceRookie { team_id, role, .. }
                if affected_team_ids.contains(team_id) =>
            {
                Some(
                    is_team_role_vacant(conn, team_id, role)
                        .map(|is_vacant| (!is_vacant).then_some(index)),
                )
            }
            _ => None,
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    for index in stale_rookie_indices.into_iter().rev() {
        plan.planned_events.remove(index);
    }
    for team_id in affected_team_ids {
        refresh_planned_hierarchy_for_team(&mut plan, conn, &team_id)?;
    }
    plan.state.player_has_pending_proposals = false;
    save_preseason_plan(career_dir, &plan)
}

pub(crate) fn sync_preseason_pending_flag(
    career_dir: &Path,
    has_pending: bool,
) -> Result<(), String> {
    let Some(mut plan) = load_preseason_plan(career_dir)? else {
        return Ok(());
    };
    plan.state.player_has_pending_proposals = has_pending;
    save_preseason_plan(career_dir, &plan)
}

pub(crate) fn refresh_planned_hierarchy_for_team(
    plan: &mut PreSeasonPlan,
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<(), String> {
    let hierarchy_week = plan
        .planned_events
        .iter()
        .filter_map(|event| match &event.event {
            PendingAction::UpdateHierarchy {
                team_id: current, ..
            } if current == team_id => Some(event.week),
            PendingAction::UpdateHierarchy { .. } => Some(event.week),
            _ => None,
        })
        .max()
        .unwrap_or(plan.state.total_weeks);
    plan.planned_events.retain(|event| {
        event.executed
            || !matches!(&event.event, PendingAction::UpdateHierarchy { team_id: current, .. } if current == team_id)
    });

    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar equipe para atualizar plano: {e}"))?
        .ok_or_else(|| "Equipe nao encontrada para atualizar plano.".to_string())?;
    let mut candidates = Vec::new();
    for driver_id in [team.piloto_1_id.clone(), team.piloto_2_id.clone()]
        .into_iter()
        .flatten()
    {
        let driver = driver_queries::get_driver(conn, &driver_id)
            .map_err(|e| format!("Falha ao carregar piloto da equipe para plano: {e}"))?;
        candidates.push((driver.id, driver.nome, driver.atributos.skill));
    }
    for event in plan.planned_events.iter() {
        if event.executed {
            continue;
        }
        if let PendingAction::PlaceRookie {
            driver,
            team_id: current,
            ..
        } = &event.event
        {
            if current == team_id {
                candidates.push((
                    driver.id.clone(),
                    driver.nome.clone(),
                    driver.atributos.skill,
                ));
            }
        }
    }
    candidates.sort_by(|a, b| b.2.total_cmp(&a.2));
    candidates.dedup_by(|a, b| a.0 == b.0);
    let n1 = candidates.first().cloned();
    let n2 = candidates.get(1).cloned();
    plan.planned_events.push(PlannedEvent {
        week: hierarchy_week,
        executed: false,
        event: PendingAction::UpdateHierarchy {
            team_id: team.id.clone(),
            team_name: team.nome.clone(),
            n1_id: n1.as_ref().map(|candidate| candidate.0.clone()),
            n1_name: n1
                .as_ref()
                .map(|candidate| candidate.1.clone())
                .unwrap_or_else(|| rust_i18n::t!("career.message.no_driver").to_string()),
            n2_id: n2.as_ref().map(|candidate| candidate.0.clone()),
            n2_name: n2
                .as_ref()
                .map(|candidate| candidate.1.clone())
                .unwrap_or_else(|| rust_i18n::t!("career.message.no_driver").to_string()),
            prev_n1_id: team.hierarquia_n1_id.clone(),
            prev_n2_id: team.hierarquia_n2_id.clone(),
            prev_tensao: team.hierarquia_tensao,
            prev_status: team.hierarquia_status.clone(),
            prev_categoria: team.categoria.clone(),
        },
    });
    Ok(())
}
