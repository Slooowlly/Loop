//! Janela de transferências ao vivo: o mercado semana a semana que a pré-temporada
//! interativa desenha na tela.
//!
//! Converte vagas/pilotos no formato do motor de janela ([`crate::market::transfer_window`]),
//! avança uma semana por vez, persiste o estado entre chamadas e escritura as assinaturas
//! que a semana fechou.

use super::*;

/// Constrói o `Seat` (vaga do motor) a partir de uma `Vacancy` do banco.
pub(super) fn seat_from_vacancy(
    conn: &Connection,
    vac: &Vacancy,
    star: &Driver,
    season: i32,
    rng: &mut impl Rng,
) -> Result<crate::market::transfer_window::Seat, String> {
    let ceiling = calculate_offer_salary(vac, star, rng).max(20_000.0);
    Ok(crate::market::transfer_window::Seat {
        id: format!("{}#{}", vac.team_id, vac.papel_necessario.as_str()),
        team_id: vac.team_id.clone(),
        category: vac.categoria.clone(),
        class: vac.classe.clone(),
        tier: vac.category_tier,
        is_n1: matches!(vac.papel_necessario, TeamRole::Numero1),
        car_norm: vac.car_strength,
        prestige: team_prestige(conn, &vac.team_id, season)?,
        required_license: crate::models::license::required_license_for_division(
            &vac.categoria,
            vac.classe.as_deref(),
        )
        .unwrap_or(0),
        salary_floor: ceiling * 0.35,
        salary_ceiling: ceiling,
    })
}

/// Constrói o `Candidate` (piloto do motor) a partir de um agente livre.
/// `is_player`=true desliga o respeito à marca (jogador tem liberdade).
pub(super) fn candidate_from_available(
    conn: &Connection,
    cand: &AvailableDriver,
    is_player: bool,
) -> Result<crate::market::transfer_window::Candidate, String> {
    let driver = &cand.driver;
    let slam_target = slam_target_category(conn, driver)?.map(|(category, _)| category);
    let max_license = cand.max_license_level.unwrap_or(0).max(
        crate::models::license::required_license_for_division(&cand.categoria_atual, None)
            .unwrap_or(0),
    );
    Ok(crate::market::transfer_window::Candidate {
        id: driver.id.clone(),
        skill: driver.atributos.skill,
        tier: cand.category_tier,
        brand: brand_of_category(&cand.categoria_atual),
        slam_target,
        max_license,
        market_value: 12_000.0 + driver.atributos.skill * 1_800.0,
        ai_respects_brand: !is_player,
        category: cand.categoria_atual.clone(),
    })
}

/// Janela de Transferências (motor IA-only): leilão de dois lados que substitui o
/// casamento guloso vaga-por-vaga. Constrói as vagas/candidatos do banco, roda o
/// motor (`transfer_window::run_window`) e aplica as assinaturas. Absorve o
/// slam-chasing (categoria-alvo vira bônus no score do piloto).
pub(super) fn apply_weekly_market(
    conn: &Connection,
    vacancies: &[Vacancy],
    available: &mut Vec<AvailableDriver>,
    new_season_number: i32,
    rng: &mut impl Rng,
    report: &mut MarketReport,
) -> Result<(), String> {
    use crate::market::transfer_window::{run_window, WindowConfig};

    let star = synthetic_star();
    let mut seats = Vec::new();
    let mut seat_to_vacancy: HashMap<String, &Vacancy> = HashMap::new();
    for vac in vacancies {
        let seat = seat_from_vacancy(conn, vac, &star, new_season_number, rng)?;
        seat_to_vacancy.insert(seat.id.clone(), vac);
        seats.push(seat);
    }

    let mut candidates = Vec::new();
    for cand in available.iter() {
        candidates.push(candidate_from_available(conn, cand, false)?);
    }

    let result = run_window(seats, candidates, &WindowConfig::default(), rng);

    for signing in &result.signings {
        let Some(&vac) = seat_to_vacancy.get(&signing.seat_id) else {
            continue;
        };
        let Some(idx) = available
            .iter()
            .position(|c| c.driver.id == signing.driver_id)
        else {
            continue;
        };
        let candidate = available[idx].clone();
        let duration = if vac.category_tier >= 4 { 3 } else { 2 };
        // Rede de segurança: se a assinatura falhar num caso de borda (licença/classe
        // não previstos), pula em vez de abortar o mercado — a vaga vai pra rookie.
        if sign_driver_to_team(
            conn,
            &candidate.driver,
            vac,
            new_season_number,
            signing.salary,
            duration,
            vac.papel_necessario.clone(),
        )
        .is_err()
        {
            continue;
        }
        report.proposals_made += 1;
        report.proposals_accepted += 1;
        report.new_signings.push(SigningInfo {
            driver_id: candidate.driver.id.clone(),
            driver_name: candidate.driver.nome.clone(),
            team_id: vac.team_id.clone(),
            team_name: vac.team_name.clone(),
            categoria: vac.categoria.clone(),
            papel: vac.papel_necessario.as_str().to_string(),
            tipo: "transferencia".to_string(),
        });
        available.remove(idx);
    }
    Ok(())
}

pub(super) fn persist_window(
    conn: &Connection,
    season: i32,
    state: &crate::market::transfer_window::WindowState,
    status: &str,
) -> Result<(), String> {
    let json =
        serde_json::to_string(state).map_err(|e| format!("Falha ao serializar janela: {e}"))?;
    conn.execute(
        "INSERT OR REPLACE INTO transfer_window (season_number, state_json, status, updated_at)
         VALUES (?1, ?2, ?3, datetime('now'))",
        params![season, json, status],
    )
    .map_err(|e| format!("Falha ao salvar janela: {e}"))?;
    Ok(())
}

pub(super) fn load_window(
    conn: &Connection,
    season: i32,
) -> Result<Option<crate::market::transfer_window::WindowState>, String> {
    let row = conn
        .query_row(
            "SELECT state_json FROM transfer_window WHERE season_number = ?1",
            params![season],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| format!("Falha ao ler janela: {e}"))?;
    match row {
        Some(json) => serde_json::from_str(&json)
            .map(Some)
            .map_err(|e| format!("Falha ao interpretar janela: {e}")),
        None => Ok(None),
    }
}

/// Constrói uma janela interativa do estado atual do banco (vagas + agentes livres
/// + o JOGADOR como candidate, se agente livre). Não persiste.
pub(super) fn build_interactive_window(
    conn: &Connection,
    season: i32,
    rng: &mut impl Rng,
) -> Result<crate::market::transfer_window::WindowState, String> {
    use crate::market::transfer_window::{Candidate, WindowConfig, WindowState};
    let star = synthetic_star();
    let vacancies = find_vacancies(conn)?;
    let mut seats = Vec::new();
    for vac in &vacancies {
        seats.push(seat_from_vacancy(conn, vac, &star, season, rng)?);
    }
    let standings: HashMap<String, DriverMarketContext> = HashMap::new();
    let available = find_available_drivers(conn, &standings)?;
    let mut candidates = Vec::new();
    for cand in &available {
        candidates.push(candidate_from_available(conn, cand, false)?);
    }
    // Jogador como candidate, se for agente livre (sem contrato regular ativo).
    let mut player_id = None;
    if let Ok(player) = driver_queries::get_player_driver(conn) {
        let free = contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
            .map_err(|e| format!("Falha ao checar contrato do jogador: {e}"))?
            .is_none();
        if player.status == DriverStatus::Ativo && free {
            let cat = player.categoria_atual.clone().unwrap_or_default();
            let tier = crate::constants::categories::get_category_config(&cat)
                .map(|c| c.tier)
                .unwrap_or(0);
            let lic =
                crate::models::license::required_license_for_division(&cat, None).unwrap_or(0);
            candidates.push(Candidate {
                id: player.id.clone(),
                skill: player.atributos.skill,
                tier,
                brand: brand_of_category(&cat),
                slam_target: None,
                max_license: lic,
                market_value: 12_000.0 + player.atributos.skill * 1_800.0,
                ai_respects_brand: false,
                category: cat.clone(),
            });
            player_id = Some(player.id.clone());
        }
    }
    Ok(WindowState::start(
        seats,
        candidates,
        WindowConfig::default(),
        player_id,
    ))
}

/// Aplica um conjunto de assinaturas da janela ao banco. Chamado a cada semana com
/// as assinaturas NOVAS (incremental) — o feed e os elencos ficam em sincronia.
pub(super) fn apply_signings(
    conn: &Connection,
    signings: &[crate::market::transfer_window::Signing],
    season: i32,
) -> Result<(), String> {
    if signings.is_empty() {
        return Ok(());
    }
    let vacancies = find_vacancies(conn)?;
    let by_seat: HashMap<String, &Vacancy> = vacancies
        .iter()
        .map(|v| (format!("{}#{}", v.team_id, v.papel_necessario.as_str()), v))
        .collect();
    let drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos p/ assinar janela: {e}"))?;
    let by_id: HashMap<&str, &Driver> = drivers.iter().map(|d| (d.id.as_str(), d)).collect();
    for signing in signings {
        let (Some(&vac), Some(&driver)) = (
            by_seat.get(&signing.seat_id),
            by_id.get(signing.driver_id.as_str()),
        ) else {
            continue;
        };
        let duration = if vac.category_tier >= 4 { 3 } else { 2 };
        // PROMOÇÃO: concede a licença da categoria/classe se faltar (a janela aceita
        // subir 1 tier; a licença é dada aqui, igual ao ladder fill).
        let _ = crate::licensing::grant_driver_license_for_division_if_needed(
            conn,
            &driver.id,
            &vac.categoria,
            vac.classe.as_deref(),
        );
        // skip-on-error (rede de segurança contra casos de borda).
        let _ = sign_driver_to_team(
            conn,
            driver,
            vac,
            season,
            signing.salary,
            duration,
            vac.papel_necessario.clone(),
        );
    }
    Ok(())
}

/// Carrega a janela persistida ou inicia uma nova (e persiste).
pub(crate) fn window_get_or_init(
    conn: &Connection,
    season: i32,
    rng: &mut impl Rng,
) -> Result<crate::market::transfer_window::WindowState, String> {
    if let Some(state) = load_window(conn, season)? {
        return Ok(state);
    }
    let state = build_interactive_window(conn, season, rng)?;
    let status = if state.is_closed() { "closed" } else { "open" };
    persist_window(conn, season, &state, status)?;
    Ok(state)
}

/// Avança uma semana da janela com a escolha do jogador (`Some(seat_id)` aceita,
/// `None` espera), persiste, e ao FECHAR aplica todas as assinaturas no banco.
pub(crate) fn window_advance(
    conn: &Connection,
    season: i32,
    player_choice: Option<&str>,
    rng: &mut impl Rng,
) -> Result<crate::market::transfer_window::WindowState, String> {
    let mut state = window_get_or_init(conn, season, rng)?;
    if state.is_closed() {
        return Ok(state);
    }
    state.advance(player_choice);
    // Aplica as assinaturas NOVAS desta semana (incremental) — banco e feed em
    // sincronia, sem contratações "fantasma" até o fecho.
    apply_signings(conn, state.unapplied_signings(), season)?;
    state.mark_applied();
    if state.is_closed() {
        ensure_player_seated(conn, season)?;
        persist_window(conn, season, &state, "closed")?;
    } else {
        persist_window(conn, season, &state, "open")?;
    }
    Ok(state)
}
