//! Dinheiro e fama do fim de semana: contexto financeiro da rodada, impacto do resultado no interesse do evento e repasse de fama para patrocinio e bilheteria.

use super::*;

/// Coeficiente do termo de PATROCÍNIO por fama do lineup (presença pública 0–100).
/// Casado com o coeficiente da reputação (0.004) — a fama é uma 2ª moeda de receita
/// no mesmo patamar: contratar um rosto famoso capta patrocínio de verdade. Tunável.
pub(super) const FAME_SPONSORSHIP_COEFF: f64 = 0.004;

pub(super) fn calculate_team_round_finance_context(
    team: &Team,
    lineup_public_presence: f64,
    added_points: i32,
    added_victories: i32,
    added_podiums: i32,
    best_result: i32,
    salary_expense: f64,
    rounds_in_season: f64,
    economic_health: GlobalEconomicHealth,
    car_maintenance_cost: f64,
    // Bilheteria (Fase 3 do Estrelato): prestígio do evento (score do event_interest,
    // pré-vendido) + presença pública somada do grid + nº de times, para dividir o
    // bolo de público por cota de fama competitiva.
    event_prestige_score: f64,
    grid_total_presence: f64,
    grid_team_count: f64,
) -> TeamRoundFinanceContext {
    let income_modifier = economy_income_modifier(economic_health);
    let cost_modifier = economy_cost_modifier(economic_health);
    let scale = category_finance_scale(&team.categoria);
    let plan = calculate_financial_plan(team);
    let round_operating_base = scale.operating_cost_midpoint() / rounds_in_season.max(1.0);
    // Coeficiente de patrocínio elevado de 0.16 → 0.32 (rebalanceamento financeiro):
    // a receita-base por corrida era ~metade da despesa operacional, levando todo
    // o grid a déficit estrutural e falência em massa. Junto com o prêmio de
    // construtores de fim de temporada (ver finance::prize), aproxima o meio de
    // grid do equilíbrio.
    let sponsorship_income = (scale.expected_cash_midpoint() / rounds_in_season.max(1.0) * 0.32
        + team.reputacao * round_operating_base * 0.004
        + plan.budget_index * round_operating_base * 0.002
        + lineup_public_presence * round_operating_base * FAME_SPONSORSHIP_COEFF)
        * income_modifier;
    let result_bonus = (added_points as f64 * 650.0
        + added_victories as f64 * 4_000.0
        + added_podiums as f64 * 1_250.0
        + if best_result <= 5 { 1_000.0 } else { 0.0 })
        * income_modifier;
    let partial_prize_income = added_points as f64 * 120.0 * income_modifier;
    let aid_income = team.parachute_payment_remaining.min(25_000.0);
    let event_operations_cost = (round_operating_base * 0.42
        + team.facilities * round_operating_base * 0.004)
        * cost_modifier;
    let structural_maintenance_cost = (round_operating_base * 0.18
        + team.engineering * round_operating_base * 0.0025
        + team.pit_crew_quality * round_operating_base * 0.0015)
        * cost_modifier;
    // A depreciação REAL do carro (Sistema de Nível do Carro) substitui a proxy antiga
    // baseada em `car_performance`: o custo é o que o cérebro decidiu gastar em peças nesta
    // rodada. A base técnica (0.16) segue modulada pela economia; o custo de peças entra
    // cru (foi decidido com o preço cru).
    let technical_investment_cost =
        round_operating_base * 0.16 * cost_modifier + car_maintenance_cost.max(0.0);
    let debt_service_cost = debt_service_for_state(team.debt_balance, &team.financial_state);

    // Bilheteria: o público que a fama do lineup atrai, escalado pelo prestígio do
    // evento e dividido por cota competitiva (presença deste time / grid) + piso.
    // Canal distinto do patrocínio — fecha o loop "casa cheia → dinheiro".
    let gate_income = crate::finance::cashflow::calculate_gate_income(
        event_prestige_score,
        round_operating_base,
        lineup_public_presence,
        grid_total_presence,
        grid_team_count,
        income_modifier,
    );

    TeamRoundFinanceContext {
        sponsorship_income,
        gate_income,
        result_bonus,
        partial_prize_income,
        aid_income,
        salary_expense,
        event_operations_cost,
        structural_maintenance_cost,
        technical_investment_cost,
        debt_service_cost,
    }
}

pub(super) fn compute_post_race_impact(
    conn: &rusqlite::Connection,
    race_entry: &CalendarEntry,
    player_race: &RaceResult,
) -> Option<RealizedEventInterest> {
    let category = get_category_config(&race_entry.categoria)?;
    let total_rodadas = category.corridas_por_temporada as i32;
    let player = driver_queries::get_player_driver(conn).ok()?;
    let champ = standings_queries::get_championship_context(conn, &race_entry.categoria).unwrap_or(
        ChampionshipContext {
            player_position: 0,
            gap_to_leader: 0,
        },
    );
    let player_result = player_race.race_results.iter().find(|r| r.is_jogador)?;

    let remaining = total_rodadas - race_entry.rodada;
    let is_title_decider = remaining <= 2 && champ.gap_to_leader <= 50 && champ.player_position > 0;
    let is_final_round_decider = race_entry.rodada == total_rodadas && is_title_decider;

    let ctx = EventInterestContext {
        categoria: race_entry.categoria.clone(),
        season_phase: race_entry.season_phase,
        rodada: race_entry.rodada,
        total_rodadas,
        week_of_year: race_entry.week_of_year,
        track_id: race_entry.track_id as i32,
        track_name: race_entry.track_name.clone(),
        is_player_event: true,
        player_championship_position: if champ.player_position > 0 {
            Some(champ.player_position)
        } else {
            None
        },
        player_media: Some(player.atributos.midia as f32),
        championship_gap_to_leader: if champ.gap_to_leader > 0 || champ.player_position == 1 {
            Some(champ.gap_to_leader)
        } else {
            None
        },
        is_title_decider_candidate: is_title_decider,
        thematic_slot: race_entry.thematic_slot,
    };
    let expected = calculate_expected_event_interest(&ctx);
    Some(calculate_realized_event_interest(
        &expected,
        &ctx,
        Some(player_result.finish_position),
        Some(player_result.grid_position),
        player_result.finish_position == 1,
        player_result.finish_position <= 3 && !player_result.is_dnf,
        player_result.is_dnf,
        is_final_round_decider,
    ))
}

/// Interesse do evento "de local" para uma corrida SEM o jogador (categorias da IA):
/// só o contexto do evento (categoria/fase/rodada/papel narrativo), sem protagonismo
/// nem resultado de um piloto específico. Escala o impacto de fama dos pilotos IA
/// quando o jogador não corre aquela categoria — é o que faz o astro nascer no mundo
/// todo, não só na categoria do jogador. `None` se a categoria não tem config.
pub(super) fn compute_venue_impact(race_entry: &CalendarEntry) -> Option<RealizedEventInterest> {
    let category = get_category_config(&race_entry.categoria)?;
    let ctx = EventInterestContext {
        categoria: race_entry.categoria.clone(),
        season_phase: race_entry.season_phase,
        rodada: race_entry.rodada,
        total_rodadas: category.corridas_por_temporada as i32,
        week_of_year: race_entry.week_of_year,
        track_id: race_entry.track_id as i32,
        track_name: race_entry.track_name.clone(),
        is_player_event: false,
        player_championship_position: None,
        player_media: None,
        championship_gap_to_leader: None,
        is_title_decider_candidate: false,
        thematic_slot: race_entry.thematic_slot,
    };
    let expected = calculate_expected_event_interest(&ctx);
    Some(calculate_realized_event_interest(
        &expected, &ctx, None, None, false, false, false, false,
    ))
}

/// Score de PRESTÍGIO "de local" do evento (pré-vendido / esperado), sem protagonismo
/// nem resultado — a mesma base que dimensiona a pressão de "casa cheia". Alimenta a
/// BILHETERIA (Fase 3 do Estrelato): evento maior (endurance, final, decisão) lota mais
/// → bolo de público maior. `0.0` se a categoria não tem config (bilheteria some).
pub(super) fn venue_prestige_score(race_entry: &CalendarEntry) -> f64 {
    let Some(category) = get_category_config(&race_entry.categoria) else {
        return 0.0;
    };
    let ctx = EventInterestContext {
        categoria: race_entry.categoria.clone(),
        season_phase: race_entry.season_phase,
        rodada: race_entry.rodada,
        total_rodadas: category.corridas_por_temporada as i32,
        week_of_year: race_entry.week_of_year,
        track_id: race_entry.track_id as i32,
        track_name: race_entry.track_name.clone(),
        is_player_event: false,
        player_championship_position: None,
        player_media: None,
        championship_gap_to_leader: None,
        is_title_decider_candidate: false,
        thematic_slot: race_entry.thematic_slot,
    };
    calculate_expected_event_interest(&ctx).score as f64
}

/// Aplica TODOS os efeitos de FAMA de uma corrida concluída. Vale IGUAL pra corrida
/// SIMULADA offline e pra corrida IMPORTADA do iRacing — ambas produzem um `RaceResult`
/// e persistem pelo mesmo caminho, então a fama reage idêntico. Efeitos:
/// - mídia/motivação do JOGADOR (modulada pelo carisma dele),
/// - impacto de mídia nos pilotos IA notáveis (vencedor/pole/pódio/incidente/lesão,
///   modulado pelo carisma de cada um),
/// - deriva leve de CARISMA por marcos do fim de semana ("drama vende"),
/// - decaimento passivo da fama de todo o grid que correu.
/// Retorna `(news_importance_bias, interest_tier)` pro caller alimentar as notícias.
pub(crate) fn apply_post_race_fame(
    conn: &rusqlite::Connection,
    race_entry: &CalendarEntry,
    result: &RaceResult,
    new_injuries: &[Injury],
) -> Result<(i32, InterestTier), String> {
    // ID do jogador — excluído do bloco world-facing (já tratado no player-facing).
    // Vazio quando o jogador não corre esta categoria (corrida 100% IA).
    let excluded_driver_id = result
        .race_results
        .iter()
        .find(|r| r.is_jogador)
        .map(|r| r.pilot_id.clone())
        .unwrap_or_default();

    // Interesse REALIZADO ancorado no jogador — `Some` só se ele correu esta corrida.
    let player_realized = compute_post_race_impact(conn, race_entry, result);

    // ── Player-facing: mídia/motivação do JOGADOR (só quando ele correu) ──
    if let Some(realized) = &player_realized {
        if let Ok(player) = driver_queries::get_player_driver(conn) {
            let player_result = result.race_results.iter().find(|r| r.is_jogador);
            let base_midia_delta = if player_result.is_some_and(|r| r.finish_position == 1) {
                3.0
            } else if player_result.is_some_and(|r| r.finish_position <= 3 && !r.is_dnf) {
                2.0
            } else if player_result.is_some_and(|r| r.finish_position <= 5) {
                1.0
            } else if player_result.is_some_and(|r| r.is_dnf) {
                -2.0
            } else {
                -1.0
            };
            // Carisma modula o delta de fama: estrela ganha mais e amortece a perda
            // (mal cai terminando mal); apagado ganha menos e sangra.
            let raw_midia_delta = base_midia_delta * realized.media_delta_modifier as f64;
            let carisma_midia_delta =
                crate::fame::apply_carisma_to_fame_delta(raw_midia_delta, player.atributos.carisma);
            let new_midia = (player.atributos.midia + carisma_midia_delta).clamp(0.0, 100.0);
            let _ = driver_queries::update_driver_midia(conn, &player.id, new_midia);

            let base_mot_delta = if player_result.is_some_and(|r| r.finish_position == 1) {
                4.0
            } else if player_result.is_some_and(|r| r.finish_position <= 3 && !r.is_dnf) {
                2.5
            } else if player_result.is_some_and(|r| r.finish_position <= 5) {
                1.0
            } else if player_result.is_some_and(|r| r.is_dnf) {
                -3.5
            } else {
                -0.5
            };
            let new_motivacao = (player.motivacao
                + base_mot_delta * realized.motivation_delta_modifier as f64)
                .clamp(0.0, 100.0);
            let _ = driver_queries::update_driver_motivation(conn, &player.id, new_motivacao);
        }
    }

    // Interesse do evento para o mundo: do jogador se ele correu; senão, "de local"
    // (categoria da IA). É isto que faz a fama valer em TODAS as categorias.
    let realized = match player_realized.or_else(|| compute_venue_impact(race_entry)) {
        Some(r) => r,
        None => return Ok((0, InterestTier::Baixo)), // sem config de categoria → nada a fazer
    };
    let post_race_bias = realized.news_importance_bias;
    let interest_tier = realized.final_tier.clone();

    // ── World-facing: impacto de mídia nos pilotos IA notáveis (SEMPRE) ──
    // `excluded_driver_id` (jogador) omitido; vazio nas corridas 100% IA (ninguém excluído).
    let main_incident_pilot: Option<String> = result
        .notable_incident_pilot_ids
        .iter()
        .find(|id| id.as_str() != excluded_driver_id.as_str())
        .cloned();
    let podium_pilot_ids: Vec<&str> = result
        .race_results
        .iter()
        .filter(|r| {
            r.finish_position >= 2
                && r.finish_position <= 3
                && !r.is_dnf
                && r.pilot_id != result.winner_id
        })
        .map(|r| r.pilot_id.as_str())
        .collect();
    let race_ctx = crate::event_interest::RaceEventContext {
        winner_id: &result.winner_id,
        pole_sitter_id: &result.pole_sitter_id,
        podium_ids: &podium_pilot_ids,
        main_incident_pilot_id: main_incident_pilot.as_deref(),
        excluded_driver_id: &excluded_driver_id,
    };
    let ai_media_impacts =
        crate::event_interest::compute_public_media_impacts(&race_ctx, new_injuries, &realized);

    for impact in &ai_media_impacts {
        // Carisma do piloto IA modula quanto de fama aquele feito rende. Neutro 50 sem leitura.
        let carisma = driver_queries::get_driver_carisma(conn, &impact.driver_id)
            .ok()
            .flatten()
            .unwrap_or(50.0);
        let delta = crate::fame::apply_carisma_to_fame_delta(impact.delta, carisma);
        driver_queries::update_driver_midia_delta(conn, &impact.driver_id, delta).map_err(|e| {
            format!(
                "Falha ao aplicar impacto de mídia para '{}': {e}",
                impact.driver_id
            )
        })?;
    }

    // ── Deriva leve do CARISMA por marcos do fim de semana ("drama vende") ──
    for pid in &result.notable_incident_pilot_ids {
        let _ = driver_queries::bump_driver_carisma(conn, pid, crate::fame::CARISMA_DRIFT_INCIDENT);
    }
    if matches!(
        interest_tier,
        InterestTier::MuitoAlto | InterestTier::EventoPrincipal
    ) && !result.winner_id.is_empty()
    {
        let _ = driver_queries::bump_driver_carisma(
            conn,
            &result.winner_id,
            crate::fame::CARISMA_DRIFT_BIG_WIN,
        );
    }
    // Remontada dos protagonistas (jogador e vencedor): subir muitas posições é magnético.
    for r in result
        .race_results
        .iter()
        .filter(|r| r.is_jogador || r.pilot_id == result.winner_id)
    {
        if !r.is_dnf && r.grid_position - r.finish_position >= crate::fame::COMEBACK_MIN_POSITIONS {
            let _ =
                driver_queries::bump_driver_carisma(conn, &r.pilot_id, crate::fame::CARISMA_DRIFT_COMEBACK);
        }
    }

    // ── Decaimento passivo da FAMA de todo o grid que correu ──
    for r in &result.race_results {
        let _ = driver_queries::decay_driver_fame(
            conn,
            &r.pilot_id,
            crate::fame::FAME_DECAY_FLOOR,
            crate::fame::FAME_DECAY_BASE_RATE,
        );
    }

    Ok((post_race_bias, interest_tier))
}
