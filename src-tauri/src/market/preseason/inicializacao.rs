//! Abertura da pré-temporada: monta o plano, sorteia os atributos sazonais das
//! equipes e aplica o aporte de última chance de quem vai para o all-in.

use super::*;

pub fn initialize_preseason(
    conn: &Connection,
    season_number: i32,
    rng: &mut impl Rng,
) -> Result<PreSeasonPlan, String> {
    let season_id = get_season_id_by_number(conn, season_number)?
        .ok_or_else(|| format!("Temporada {season_number} nao encontrada"))?;
    reset_market_state(conn, &season_id, &PreSeasonPhase::Transfers)?;
    repair_missing_licenses_for_current_categories(conn)?;
    assign_seasonal_team_attributes(conn, season_number, &season_id)?;

    // Contratos ativos ANTES das pré-passes — p/ detectar as DISPENSAS (terminaram e
    // não foram renovados) e narrá-las no feed da 1ª semana.
    let contracts_before = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao carregar contratos antes das pre-passes: {e}"))?;

    // Snapshot das categorias ANTES das pré-passes (que limpam o categoria_atual dos
    // dispensados) — origem p/ inferir promovido/rebaixado nas assinaturas da escada.
    let category_snapshot: std::collections::HashMap<String, String> =
        driver_queries::get_all_drivers(conn)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|d| d.categoria_atual.map(|c| (d.id, c)))
            .collect();

    // Snapshot da EQUIPE anterior (+ tempo de casa) ANTES das pré-passes — origem p/ o
    // popup de detalhe da transferência (de qual equipe veio e há quantas temporadas).
    let previous_team: std::collections::HashMap<String, (String, i32)> = contracts_before
        .iter()
        .map(|c| {
            let tenure = crate::commands::career::calculate_consecutive_team_tenure(
                conn,
                &c.piloto_id,
                &c.equipe_id,
                season_number - 1,
            )
            .unwrap_or(1);
            (c.piloto_id.clone(), (c.equipe_nome.clone(), tenure))
        })
        .collect();

    // ── PRÉ-PASSES REAIS (sem rollback-replay): expira contratos que terminaram,
    // renova (slam-aware) e rebaixa por mérito — aplicadas DE VERDADE no banco. ──
    let prepass_report = crate::market::pipeline::run_market_prepasses(conn, season_number, rng)
        .map_err(|e| format!("Falha ao aplicar pre-passes do mercado: {e}"))?;

    // Feed da 1ª semana: dispensas (×) + promoções/rebaixamentos por mérito (↑/↓), que
    // acontecem nas pré-passes e antes ficavam invisíveis.
    let mut pending_departures =
        build_departure_events(conn, season_number, &contracts_before, &previous_team)?;
    pending_departures.extend(merit_move_events(&prepass_report, &previous_team));

    // O mercado ao vivo é a escada paginada conduzida por advance_week (sem motor de
    // janela persistido): cada semana preenche vagas em todos os tiers e oferta vagas
    // ao jogador. A pré-temporada fecha quando não há mais o que preencher.

    // O jogador já tem time se saiu da pré-passe com contrato regular ativo
    // (renovou / contrato plurianual); senão é agente livre dentro da janela.
    let player_has_team = driver_queries::get_player_driver(conn)
        .ok()
        .and_then(|player| {
            contract_queries::get_active_regular_contract_for_pilot(conn, &player.id).ok()
        })
        .flatten()
        .is_some();

    // Semanas VARIÁVEIS: a conclusão é dirigida pela janela (flag closed), não por um
    // total fixo. `total_weeks` fica só como teto p/ a data exibida não estourar.
    let total_weeks = i32::from(MARKET_DURATION_WEEKS);
    let mut state = PreSeasonState {
        season_number,
        current_week: 1,
        total_weeks,
        phase: PreSeasonPhase::Transfers,
        is_complete: false,
        player_has_pending_proposals: false,
        player_has_team,
        current_display_date: None,
    };
    refresh_preseason_state_display_date(conn, &season_id, &mut state)?;

    // Quebra de contrato do jogador (Fase 2b.3): se um time claramente melhor cobiça o
    // jogador CONTRATADO, guarda o leilão pra ele ver e decidir. Raro; None quase sempre.
    let player_poach_offer =
        crate::market::pipeline::compute_player_poach_offer(conn, season_number)?;

    Ok(PreSeasonPlan {
        state,
        planned_events: Vec::new(),
        executed_weeks: Vec::new(),
        pending_departures,
        category_snapshot,
        previous_team,
        player_poach_offer,
    })
}

/// Tamanho do aporte de última chance, como fração do caixa-médio da categoria.
/// Calibrado para que ~20% das equipes em all-in escapem do colapso (o resto
/// ainda é vendido). Maior = mais escapam.
pub(super) const LAST_CHANCE_PACKAGE_FACTOR: f64 = 0.25;

/// Aplica o aporte de última chance a uma equipe entrando no ano de all-in:
/// abate a maior parte da dívida e reforça o caixa. Não recalcula o estado
/// financeiro (o chamador o faz).
pub(super) fn apply_last_chance_package(team: &mut crate::models::team::Team) {
    let scale = category_finance_scale(&team.categoria);
    let package = scale.expected_cash_midpoint() * LAST_CHANCE_PACKAGE_FACTOR;
    // 70% do pacote abate dívida, 30% vira capital de giro.
    team.debt_balance = (team.debt_balance - package * 0.70).max(0.0);
    team.cash_balance += package * 0.30;
}

pub(super) fn assign_seasonal_team_attributes(
    conn: &Connection,
    season_number: i32,
    season_id: &str,
) -> Result<(), String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao carregar equipes: {e}"))?;
    let previous_standings = load_previous_team_standings(conn, season_number)?;
    // Pilar D: as 3 elites por classe premium (Production auto por reputação;
    // GT3/Endurance com as marcas reais fixas). Recebem plano de dinastia + piso de
    // recursos abaixo, sustentando carro no topo temporada após temporada.
    let elite_ids = designate_elite_teams(&teams);

    // Ano de carreira (0 = primeiro ano), usado para esmaecer a penalidade fictícia
    // GT3 ao longo dos primeiros anos. Saves antigos NÃO têm a meta "career_start_year"
    // (semeada só em carreiras novas); para eles usamos PENALTY_FADE_YEARS, que zera a
    // penalidade (fator 1.0) e deixa esses saves intocados.
    let career_year: i32 = match meta_queries::get_meta_value(conn, "career_start_year")
        .map_err(|e| format!("Falha ao ler career_start_year: {e}"))?
        .and_then(|v| v.parse::<i32>().ok())
    {
        Some(career_start_year) => {
            let season_year: i32 = conn
                .query_row(
                    "SELECT ano FROM seasons WHERE numero = ?1",
                    rusqlite::params![season_number],
                    |row| row.get(0),
                )
                .map_err(|e| format!("Falha ao buscar ano da temporada {season_number}: {e}"))?;
            season_year - career_start_year
        }
        None => PENALTY_FADE_YEARS as i32,
    };

    let mut categories = teams
        .iter()
        .map(|team| team.categoria.clone())
        .collect::<Vec<_>>();
    categories.sort();
    categories.dedup();

    for category in categories {
        let category_teams = teams
            .iter()
            .filter(|team| team.categoria == category)
            .cloned()
            .collect::<Vec<_>>();
        if category_teams.is_empty() {
            continue;
        }

        let calendar = calendar_queries::get_calendar(conn, season_id, &category)
            .map_err(|e| format!("Falha ao carregar calendario de {category}: {e}"))?;
        if calendar.is_empty() {
            continue;
        }

        for team in &category_teams {
            let mut updated_team = team.clone();
            // O shape/nível do carro agora vive no Sistema de Nível do Carro (tabela
            // team_car), decidido pelo cérebro de manutenção por corrida — não mais por
            // um perfil discreto escolhido aqui. O custo do carro vira depreciação real
            // na fatura (chunk 6).
            updated_team.pit_strategy_risk = recalculate_pit_strategy_risk(team, &category_teams);
            updated_team.budget = derive_budget_index_from_money(&updated_team);
            refresh_team_financial_state(&mut updated_team);
            // Pilar D: elite recebe o piso de recursos (patrocínio de dinastia) antes
            // de recalcular o estado financeiro — assim nunca cai em colapso e sustenta
            // o investimento máximo no carro.
            let is_elite = elite_ids.contains(&updated_team.id);
            if is_elite {
                apply_elite_resource_floor(&mut updated_team);
                updated_team.budget = derive_budget_index_from_money(&updated_team);
                refresh_team_financial_state(&mut updated_team);
            }
            // Equipe entrando na 2ª temporada consecutiva de colapso vai de all-in
            // numa tentativa de se salvar antes de ser vendida. Recebe um aporte de
            // "última chance" (investidores injetam capital): abate parte da dívida
            // e reforça o caixa, dando uma chance real — mas só quem também render
            // na pista escapa do colapso; o resto ainda termina vendido.
            let collapse_streak =
                team_queries::get_collapse_streak(conn, &updated_team.id).unwrap_or(0);
            updated_team.season_strategy = if collapse_streak == 1 && !is_elite {
                apply_last_chance_package(&mut updated_team);
                refresh_team_financial_state(&mut updated_team);
                "all_in".to_string()
            } else {
                // Pilar C: a estratégia da temporada vem do plano de 3 temporadas
                // da equipe (arco sustentado), não de uma escolha reativa anual.
                // Pilar D: elites rodam Elite Dominance permanente (dinastia).
                advance_strategic_plan(conn, &updated_team, is_elite)
                    .map_err(|e| {
                        format!(
                            "Falha ao avançar plano estratégico da equipe {}: {e}",
                            updated_team.nome
                        )
                    })?
                    .to_string()
            };
            // Foco vigente (da temporada que passou) decide quanto o time canaliza
            // para o carro neste offseason — a consequência real do foco (ideia 4).
            // Lido ANTES do update_team_focus abaixo (que só recalcula a fase nova).
            let current_focus = crate::finance::focus::get_focus(conn, &updated_team.id)
                .map(|(foco, _)| foco)
                .unwrap_or(crate::finance::focus::TeamFocus::MeioDeGrid);
            apply_offseason_competitiveness_impact(&mut updated_team, career_year, current_focus);
            updated_team.pit_crew_quality = recalculate_pit_crew_quality(
                &updated_team,
                previous_standings.get(&team.id).copied(),
            );
            refresh_team_financial_state(&mut updated_team);
            updated_team.budget = derive_budget_index_from_money(&updated_team);
            team_queries::update_team(conn, &updated_team).map_err(|e| {
                format!(
                    "Falha ao salvar perfil sazonal do carro para equipe {}: {e}",
                    updated_team.nome
                )
            })?;

            // Foco da equipe (ideia 4): deriva a fase atual do estado já calculado,
            // com histerese; promoção/rebaixamento (categoria_anterior) é evento duro
            // que fura a histerese e troca na hora. O retorno (virada) alimentará a
            // notícia da mudança de foco.
            let (plan_type, _) = team_queries::get_strategic_plan(conn, &updated_team.id)
                .unwrap_or_else(|_| ("sustainable".to_string(), 0));
            let hard_event = updated_team.categoria_anterior.is_some();
            crate::finance::focus::update_team_focus(
                conn,
                &updated_team,
                is_elite,
                &plan_type,
                hard_event,
            )
            .map_err(|e| format!("Falha ao atualizar foco da equipe {}: {e}", updated_team.nome))?;
        }
    }

    Ok(())
}

pub(super) fn load_previous_team_standings(
    conn: &Connection,
    season_number: i32,
) -> Result<std::collections::HashMap<String, PreviousTeamStanding>, String> {
    if season_number <= 1 {
        return Ok(std::collections::HashMap::new());
    }

    let Some(previous_season_id) = get_season_id_by_number(conn, season_number - 1)? else {
        return Ok(std::collections::HashMap::new());
    };

    let mut stmt = conn
        .prepare(
            "SELECT equipe_id, categoria, SUM(pontos) AS total_pontos
             FROM standings
             WHERE temporada_id = ?1 AND equipe_id IS NOT NULL AND TRIM(equipe_id) <> ''
             GROUP BY equipe_id, categoria
             ORDER BY categoria ASC, total_pontos DESC, equipe_id ASC",
        )
        .map_err(|e| format!("Falha ao preparar standings anteriores por equipe: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![previous_season_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar standings anteriores por equipe: {e}"))?;

    let mut grouped = std::collections::HashMap::<String, Vec<(String, f64)>>::new();
    for row in rows {
        let (team_id, category, total_points) =
            row.map_err(|e| format!("Falha ao ler standings anteriores por equipe: {e}"))?;
        grouped
            .entry(category)
            .or_default()
            .push((team_id, total_points));
    }

    let mut result = std::collections::HashMap::new();
    for teams_in_category in grouped.into_values() {
        let total_teams = teams_in_category.len();
        for (index, (team_id, _)) in teams_in_category.into_iter().enumerate() {
            result.insert(
                team_id,
                PreviousTeamStanding {
                    position: index as i32 + 1,
                    total_teams,
                },
            );
        }
    }

    Ok(result)
}
