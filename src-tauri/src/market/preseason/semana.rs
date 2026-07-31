//! Avanço de UMA semana da pré-temporada (a Janela de Transferências).

use super::*;

/// Avança UMA semana da pré-temporada. O mercado é a Janela de Transferências: a IA
/// assina e o jogador aceita `player_choice` (id da vaga) ou espera (`None`). A
/// pré-temporada fecha quando a janela fecha (semanas variáveis), não num total fixo.
pub fn advance_week(
    conn: &Connection,
    plan: &mut PreSeasonPlan,
    player_choice: Option<&str>,
) -> Result<WeekResult, String> {
    if plan.state.is_complete {
        return Err("Pre-temporada ja esta completa".to_string());
    }

    repair_missing_licenses_for_current_categories(conn)?;
    let week = plan.state.current_week;
    let season = plan.state.season_number;
    let season_id = get_season_id_by_number(conn, season)?
        .ok_or_else(|| format!("Temporada {season} nao encontrada"))?;
    let mut rng = StdRng::seed_from_u64(season as u64);

    // O jogador aceitou uma oferta nesta semana → assina antes da escada (libera o
    // assento dele da reserva e evita que a escada o preencha por baixo).
    if let Some(seat) = player_choice {
        crate::market::pipeline::sign_player_to_vacancy(conn, season, seat)?;
    }

    // Propostas formais de MÉRITO desta semana ("Proposta recebida"): equipes que
    // escolheriam o jogador o cortejam nominalmente. Os assentos dessas propostas também
    // são segurados (não podem ser preenchidos pela IA enquanto a proposta vive).
    let proposal_seats =
        crate::market::pipeline::generate_player_window_proposals(conn, season, week, &mut rng)?;

    // Reserva alguns assentos pro jogador (se agente livre ativo) — a escada não os
    // preenche, garantindo escolha real E que na última semana haja vaga vazia pra ele,
    // sem dispensar ninguém. Une os assentos das propostas formais.
    let reserved: std::collections::HashSet<String> =
        crate::market::pipeline::player_reserved_seats(conn, season)?
            .into_iter()
            .chain(proposal_seats)
            .collect();

    // Categorias de ORIGEM (snapshot do INÍCIO da pré-temporada, antes das pré-passes
    // limparem o categoria_atual dos dispensados) — pra inferir promovido/rebaixado.
    let category_snapshot = plan.category_snapshot.clone();
    // Equipe anterior (+ tempo de casa) p/ o popup de detalhe da transferência.
    let previous_team = plan.previous_team.clone();

    // Escada (ladder fill) paginada: preenche ~6 vagas em TODOS os tiers (agente
    // livre → rookie → promoção da categoria de baixo), poupando os assentos reservados.
    let mut report = crate::market::proposals::MarketReport::default();
    crate::market::pipeline::fill_vacancies_paced(
        conn,
        season,
        Some(6),
        &reserved,
        &mut report,
        &mut rng,
    )?;

    // Mapeia as assinaturas da escada → eventos de feed.
    let drivers = driver_queries::get_all_drivers(conn).unwrap_or_default();
    let driver_names: std::collections::HashMap<&str, &str> = drivers
        .iter()
        .map(|d| (d.id.as_str(), d.nome.as_str()))
        .collect();
    let teams = team_queries::get_all_teams(conn).unwrap_or_default();
    let team_names: std::collections::HashMap<&str, &str> = teams
        .iter()
        .map(|t| (t.id.as_str(), t.nome.as_str()))
        .collect();
    // Mapeia uma assinatura da escada → evento de feed. Closure reutilizada tanto
    // pelas assinaturas paginadas quanto pelo preenchimento final da última semana.
    let map_signing = |signing: &crate::market::proposals::SigningInfo| -> MarketEvent {
        let dname = driver_names
            .get(signing.driver_id.as_str())
            .copied()
            .unwrap_or(signing.driver_name.as_str());
        let tname = team_names
            .get(signing.team_id.as_str())
            .copied()
            .unwrap_or(signing.team_name.as_str());
        // Origem do piloto (categoria no início) → promovido/rebaixado/lateral.
        let from_cat = category_snapshot.get(signing.driver_id.as_str()).cloned();
        // Estreia (rookie) não tem equipe anterior por definição — não anexa snapshot.
        let is_rookie = matches!(signing.tipo.as_str(), "rookie" | "rookie_emergencia");
        let prev = if is_rookie {
            None
        } else {
            previous_team.get(signing.driver_id.as_str()).cloned()
        };
        let movement_kind = if is_rookie {
            "rookie".to_string()
        } else {
            let from_tier = from_cat
                .as_deref()
                .and_then(crate::constants::categories::get_category_config)
                .map(|c| c.tier);
            let to_tier = crate::constants::categories::get_category_config(&signing.categoria)
                .map(|c| c.tier);
            // Mesma equipe = RENOVAÇÃO (re-assinou o próprio assento), não troca lateral.
            let same_team = prev
                .as_ref()
                .is_some_and(|(team, _)| team.as_str() == tname);
            match (from_tier, to_tier) {
                (Some(f), Some(t)) if t > f => "promotion",
                (Some(f), Some(t)) if t < f => "relegation",
                _ if same_team => "renewal",
                (Some(_), Some(_)) => "lateral",
                _ => "signing",
            }
            .to_string()
        };
        let (from_team, seasons_at_previous) = match prev {
            Some((team, tenure)) => (Some(team), Some(tenure)),
            None => (None, None),
        };
        MarketEvent {
            event_type: MarketEventType::TransferCompleted,
            headline: format!("{dname} -> {tname}"),
            description: rust_i18n::t!("market.event.deal", category = signing.categoria.as_str())
                .to_string(),
            driver_id: Some(signing.driver_id.clone()),
            driver_name: Some(dname.to_string()),
            team_id: Some(signing.team_id.clone()),
            team_name: Some(tname.to_string()),
            from_team,
            to_team: Some(tname.to_string()),
            categoria: Some(signing.categoria.clone()),
            from_categoria: from_cat,
            movement_kind: Some(movement_kind),
            championship_position: None,
            seasons_at_previous,
            relation: None,
        }
    };
    let mut events: Vec<MarketEvent> = report.new_signings.iter().map(&map_signing).collect();

    // Na 1ª semana avançada, anexa (no topo) as DISPENSAS capturadas no início — o
    // jogador vê quem perdeu a vaga antes das contratações.
    if week == 1 && !plan.pending_departures.is_empty() {
        let mut feed = std::mem::take(&mut plan.pending_departures);
        feed.extend(events);
        events = feed;
    }

    sync_team_slots_from_active_contracts(conn)?;
    let remaining = count_remaining_vacancies(conn)?;

    // O jogador pode ter assinado nesta semana (aceitou uma oferta) — reflete no
    // estado pra a UI e o gate de finalização não ficarem defasados.
    plan.state.player_has_team = driver_queries::get_player_driver(conn)
        .ok()
        .and_then(|player| {
            contract_queries::get_active_regular_contract_for_pilot(conn, &player.id).ok()
        })
        .flatten()
        .is_some();

    // Fecha quando só restam os assentos reservados (nada mais a preencher) ou ao
    // bater o teto de semanas.
    let is_last_week =
        remaining <= reserved.len() as i32 || week >= i32::from(MARKET_DURATION_WEEKS);
    if is_last_week {
        // Garante porta ao jogador (num dos assentos vazios que a escada segurou pra ele,
        // sem dispensar ninguém) e preenche TODAS as vagas restantes — nenhum time corre
        // sem piloto.
        crate::market::pipeline::ensure_player_seated(conn, season)?;
        // O preenchimento final assina vários pilotos de uma vez. Captura essas
        // assinaturas num report e mapeia p/ feed — senão a última semana ficaria
        // muda (os pilotos preenchidos no fechamento sumiam do "fechamento da semana").
        let mut final_report = crate::market::proposals::MarketReport::default();
        crate::market::pipeline::fill_all_remaining_vacancies_reported(
            conn,
            season,
            &mut rng,
            &mut final_report,
        )?;
        events.extend(final_report.new_signings.iter().map(&map_signing));
        plan.state.current_week = week + 1;
        plan.state.phase = PreSeasonPhase::Complete;
        plan.state.is_complete = true;
        events.push(MarketEvent {
            event_type: MarketEventType::PreSeasonComplete,
            headline: rust_i18n::t!("market.event.window_closed_headline").to_string(),
            description: rust_i18n::t!("market.event.window_closed_desc").to_string(),
            driver_id: None,
            driver_name: None,
            team_id: None,
            team_name: None,
            from_team: None,
            to_team: None,
            categoria: None,
            from_categoria: None,
            movement_kind: None,
            championship_position: None,
            seasons_at_previous: None,
            relation: None,
        });
        update_market_state(conn, &season_id, "Fechado", &PreSeasonPhase::Complete, true)?;
    } else {
        plan.state.current_week += 1;
        plan.state.phase = PreSeasonPhase::Transfers;
        update_market_state(
            conn,
            &season_id,
            "Aberto",
            &PreSeasonPhase::Transfers,
            false,
        )?;
    }

    // Marca cada evento com seu vínculo ao jogador (rival / já-correu-contra) — o feed
    // mostra TODOS, mas dá ênfase aos marcados. Não filtra nada.
    tag_player_relations(conn, &mut events);

    let next_phase = plan.state.phase.clone();
    refresh_preseason_state_display_date(conn, &season_id, &mut plan.state)?;
    let result = WeekResult {
        week_number: week,
        phase: PreSeasonPhase::Transfers,
        events,
        is_last_week,
        player_proposals: Vec::new(),
        remaining_vacancies: remaining,
        next_phase,
    };
    plan.executed_weeks.push(result.clone());
    Ok(result)
}
