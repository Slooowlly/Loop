use std::collections::{HashMap, HashSet};
use std::path::Path;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::commands::race_history::append_race_result;
use crate::config::app_config::{AppConfig, SaveMeta};
use crate::constants::categories::{
    get_all_categories, get_category_config, is_multiclass_category, runs_in_special_phase,
    CategoryConfig,
};
use crate::constants::historical_timeline::is_team_active_in_year;
use crate::constants::scoring::{get_points_for_position, BONUS_FASTEST_LAP};
use crate::db::connection::Database;
use crate::db::connection::DbError;
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::special_team_entries as special_entry_queries;
use crate::db::queries::standings as standings_queries;
use crate::db::queries::standings::ChampionshipContext;
use crate::db::queries::teams as team_queries;
use crate::db::queries::track_history as track_history_queries;
use crate::event_interest::{
    calculate_expected_event_interest, calculate_realized_event_interest, EventInterestContext,
    InterestTier, RealizedEventInterest,
};
use crate::finance::cashflow::{apply_round_cashflow, TeamRoundFinanceContext};
use crate::finance::economy::{
    economy_cost_modifier, economy_income_modifier, global_economic_health_for_season,
    GlobalEconomicHealth,
};
use crate::finance::events::{apply_crisis_event_if_needed, debt_service_for_state};
use crate::finance::planning::{calculate_financial_plan, category_finance_scale};
use crate::finance::state::refresh_team_financial_state;
use crate::models::driver::Driver;
use crate::models::injury::Injury;
use crate::models::season::Season;
use crate::simulation::batch::{
    BriefRaceResult, CategorySimResult, SimHighlight, SimultaneousResults,
};
use crate::simulation::catalog::IncidentCatalog;
use crate::simulation::context::{SimDriver, SimulationContext};
use crate::simulation::engine::run_full_race_with_breakdowns;
use crate::simulation::incidents::IncidentResult;
use crate::simulation::race::RaceResult;
use crate::{calendar::CalendarEntry, models::team::Team};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceWeekendResult {
    pub player_race: RaceResult,
    pub other_categories: SimultaneousResults,
    /// Avaliação de carreira (expectativa vs resultado, nota, frases) — o MESMO
    /// cérebro do import do iRacing. `None` se não der para avaliar (tela trata).
    #[serde(default)]
    pub evaluation: Option<crate::race_eval::RaceEvaluation>,
    /// Fatura de manutenção do carro (gasolina/pneus; sim offline não tem batida).
    #[serde(default)]
    pub maintenance: MaintenanceBreakdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RacePersistenceMode {
    Playable,
    HistoricalDraft,
}

/// Coeficiente do termo de PATROCÍNIO por fama do lineup (presença pública 0–100).
/// Casado com o coeficiente da reputação (0.004) — a fama é uma 2ª moeda de receita
/// no mesmo patamar: contratar um rosto famoso capta patrocínio de verdade. Tunável.
const FAME_SPONSORSHIP_COEFF: f64 = 0.004;

fn calculate_team_round_finance_context(
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

fn compute_post_race_impact(
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
fn compute_venue_impact(race_entry: &CalendarEntry) -> Option<RealizedEventInterest> {
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
fn venue_prestige_score(race_entry: &CalendarEntry) -> f64 {
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

#[tauri::command]
pub async fn simulate_race_weekend(
    app: AppHandle,
    career_id: String,
    race_id: String,
) -> Result<RaceWeekendResult, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    simulate_race_weekend_in_base_dir(&base_dir, &career_id, &race_id)
}

#[tauri::command]
pub async fn simulate_special_block(
    app: AppHandle,
    career_id: String,
) -> Result<SimultaneousResults, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    simulate_special_block_in_base_dir(&base_dir, &career_id)
}

pub(crate) fn simulate_race_weekend_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    race_id: &str,
) -> Result<RaceWeekendResult, String> {
    let config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(career_id);
    let db_path = career_dir.join("career.db");
    let meta_path = career_dir.join("meta.json");

    if !career_dir.exists() {
        return Err("Save nao encontrado.".to_string());
    }
    if !db_path.exists() {
        return Err("Banco da carreira nao encontrado.".to_string());
    }

    let mut db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let mut race_entry = calendar_queries::get_calendar_entry_by_id(&db.conn, race_id)
        .map_err(|e| format!("Falha ao buscar corrida: {e}"))?
        .ok_or_else(|| "Corrida nao encontrada.".to_string())?;

    let active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

    // Validar que a corrida pertence à temporada atual e está pendente
    if race_entry.season_id != active_season.id {
        return Err("A corrida selecionada nao pertence a temporada atual.".to_string());
    }
    if race_entry.status.as_str() != "Pendente" {
        return Err("A corrida selecionada ja foi concluida ou simulada.".to_string());
    }
    let expected_player_race = get_next_player_race(&db.conn, &active_season)?;
    match expected_player_race {
        Some(expected) if expected.id == race_entry.id => {}
        Some(expected) => {
            return Err(format!(
                "A corrida selecionada nao e a proxima corrida valida do jogador. Proxima esperada: {} ({})",
                expected.track_name, expected.categoria
            ));
        }
        None => {
            return Err(
                "O jogador nao possui corrida pendente valida para simular nesta fase.".to_string(),
            );
        }
    }

    // FONTE ÚNICA do clima: resolve pelo MESMO generate_weather do export (e persiste no
    // entry.clima), pra a simulação offline produzir o resultado que o iRacing rodaria.
    if let Some(track) = crate::constants::tracks::get_track(race_entry.track_id) {
        let cal = calendar_queries::get_calendar(&db.conn, &race_entry.season_id, &race_entry.categoria)
            .unwrap_or_default();
        let career_first = cal.iter().all(|e| e.status.as_str() != "Concluida");
        let first_week = cal.iter().map(|e| e.week_of_year).min().unwrap_or(i32::MAX);
        let is_first = career_first && race_entry.week_of_year == first_week;
        race_entry.clima = crate::commands::iracing::resolve_and_persist_race_weather(
            &db.conn,
            career_id,
            track,
            race_entry.week_of_year,
            &race_entry.id,
            is_first,
        );
    }

    let (player_race, player_new_injuries) = simulate_category_race(&mut db, &race_entry, true)?;

    // Repercussão pós-corrida: fama (jogador + IA, modulada por carisma), deriva de
    // carisma e decaimento passivo. MESMA lógica da corrida importada do iRacing.
    let (post_race_bias, interest_tier) =
        apply_post_race_fame(&db.conn, &race_entry, &player_race, &player_new_injuries)?;

    warn_if_side_effect_fails(
        append_race_result(
            &career_dir,
            &race_entry.categoria,
            race_entry.rodada,
            &player_race.race_results,
        ),
        "Falha ao gravar race_results.json da corrida do jogador",
    );
    warn_if_side_effect_fails(
        track_history_queries::record_race_dnfs(
            &db.conn,
            &player_race.race_results,
            &race_entry.track_name,
            active_season.numero,
            race_entry.rodada,
        )
        .map_err(|e| format!("Falha ao registrar historico de DNF da corrida do jogador: {e}")),
        "Falha ao registrar historico de DNF da corrida do jogador",
    );
    match persist_race_news(
        &db.conn,
        &player_race,
        &active_season,
        race_entry.rodada,
        &race_entry.categoria,
        post_race_bias,
        race_entry.thematic_slot,
        &interest_tier,
        &player_race
            .race_results
            .iter()
            .flat_map(|r| r.incidents.clone())
            .collect::<Vec<_>>(),
        &player_new_injuries,
        &[], // sem telemetria real no fluxo simulado
    ) {
        // Corrida do jogador gerou notícia de Corrida → PRÉ-GERA o boletim de IA em
        // background agora, para já estar em cache quando o jogador abrir Notícias
        // (sem sentir a latência de ~5s do servidor).
        Ok(Some(news_id)) => {
            let mut cfg = AppConfig::load_or_default(base_dir);
            let install_id = cfg.get_or_create_install_id();
            let lang = cfg.language.clone();
            spawn_prewarm_boletim(db_path.clone(), news_id, lang, install_id);
        }
        Ok(None) => {}
        Err(e) => eprintln!("Falha ao persistir noticias da corrida do jogador: {e}"),
    }
    let other_categories = simulate_other_categories(
        &mut db,
        &career_dir,
        &race_entry.categoria,
        calendar_queries::calendar_entry_season_week(&race_entry),
        &race_entry.display_date,
        &active_season.id,
        active_season.numero,
    )?;
    warn_if_side_effect_fails(
        update_last_played(&meta_path),
        "Falha ao atualizar meta.json apos a corrida",
    );

    // Cérebro do pós-corrida: mesma avaliação do import do iRacing (expectativa vs
    // resultado, nota, frases). O sim offline NÃO tem telemetria ao vivo, então a
    // tela mostra a leitura de carreira + saldo de posições e esconde os gráficos.
    let evaluation = compute_race_evaluation(&db.conn, &player_race);

    // Conserto na simulação: DNF do jogador cobra como uma batida leve. "leve" cobra
    // 0 (reservado a batidas do iRacing que cruzaram a linha), então mapeamos o DNF de
    // sim pro tier mais leve que cobra ("moderado"). Debitado do caixa, igual ao import.
    let player_entry = player_race.race_results.iter().find(|r| r.is_jogador);
    let mut repair_cost = 0.0;
    let mut repair_severity = "nenhum";
    if player_entry.map(|r| r.is_dnf).unwrap_or(false) {
        repair_severity = "moderado";
        if let Some(pe) = player_entry {
            if let Ok(Some(mut team)) = team_queries::get_team_by_id(&db.conn, &pe.team_id) {
                let mut rng = rand::thread_rng();
                let cost = compute_repair_cost(
                    repair_severity,
                    &team.categoria,
                    team.car_performance,
                    &mut rng,
                );
                if cost > 0.0 {
                    team.cash_balance -= cost;
                    team.last_round_expenses += cost;
                    let _ = team_queries::update_team(&db.conn, &team);
                    repair_cost = cost;
                }
            }
        }
    }

    // Fatura de manutenção do carro (gasolina/pneus + conserto, se houve DNF).
    let maintenance = compute_maintenance_breakdown(
        &race_entry.categoria,
        player_entry.map(|r| r.final_tire_wear).unwrap_or(0.0),
        player_entry.map(|r| r.laps_completed).unwrap_or(0),
        repair_cost,
        repair_severity,
    );

    // Persiste a tela do pós-corrida para reabrir depois pela Home (offline não
    // tem telemetria ao vivo → sem gráficos, mas tem cérebro + saldo + manutenção).
    save_race_screen(
        &career_dir,
        race_id,
        &serde_json::json!({
            "race_result": &player_race,
            "evaluation": &evaluation,
            "telemetry": serde_json::Value::Null,
            "maintenance": &maintenance,
        }),
    );

    Ok(RaceWeekendResult {
        player_race,
        other_categories,
        evaluation,
        maintenance,
    })
}

pub(crate) fn simulate_special_block_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<SimultaneousResults, String> {
    let config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(career_id);
    let db_path = career_dir.join("career.db");
    let meta_path = career_dir.join("meta.json");

    if !career_dir.exists() {
        return Err("Save nao encontrado.".to_string());
    }
    if !db_path.exists() {
        return Err("Banco da carreira nao encontrado.".to_string());
    }

    let mut db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

    if active_season.fase != crate::models::enums::SeasonPhase::BlocoEspecial {
        return Err("O fast-sim do bloco especial so pode ocorrer em BlocoEspecial.".to_string());
    }

    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    if player.categoria_especial_ativa.is_some() {
        return Err(
            "O jogador participa do bloco especial ativo e deve correr essa fase normalmente."
                .to_string(),
        );
    }

    let mut categories_simulated = Vec::new();
    let mut total_races_simulated = 0;
    let mut highlights = Vec::new();

    for category_id in ["production_challenger", "endurance"] {
        let pending = calendar_queries::get_pending_races_for_category(
            &db.conn,
            &active_season.id,
            category_id,
        )
        .map_err(|e| format!("Falha ao buscar corridas pendentes de {}: {e}", category_id))?;

        if pending.is_empty() {
            continue;
        }

        let category = get_category_config(category_id)
            .ok_or_else(|| format!("Categoria '{}' nao encontrada.", category_id))?;

        let mut summaries = Vec::new();
        for entry in pending {
            let (result, special_injuries) = simulate_category_race(&mut db, &entry, false)?;
            // Fama do MUNDO nas categorias especiais também (jogador ou IA). Best-effort.
            let _ = apply_post_race_fame(&db.conn, &entry, &result, &special_injuries);
            warn_if_side_effect_fails(
                append_race_result(
                    &career_dir,
                    &entry.categoria,
                    entry.rodada,
                    &result.race_results,
                ),
                "Falha ao gravar race_results.json do bloco especial",
            );
            warn_if_side_effect_fails(
                track_history_queries::record_race_dnfs(
                    &db.conn,
                    &result.race_results,
                    &entry.track_name,
                    active_season.numero,
                    entry.rodada,
                )
                .map_err(|e| format!("Falha ao registrar historico de DNF do bloco especial: {e}")),
                "Falha ao registrar historico de DNF do bloco especial",
            );

            let winner = result
                .race_results
                .iter()
                .find(|driver| driver.finish_position == 1);
            summaries.push(BriefRaceResult {
                race_id: entry.id.clone(),
                track_name: entry.track_name.clone(),
                winner_name: winner
                    .map(|driver| driver.pilot_name.clone())
                    .unwrap_or_default(),
                winner_team: winner
                    .map(|driver| driver.team_name.clone())
                    .unwrap_or_default(),
            });
            total_races_simulated += 1;
        }

        if let Some(last) = summaries.last() {
            highlights.push(SimHighlight {
                headline: format!(
                    "{} vence em {} ({})",
                    last.winner_name, last.track_name, category.nome_curto
                ),
                category: category_id.to_string(),
            });
        }

        categories_simulated.push(CategorySimResult {
            category_id: category_id.to_string(),
            category_name: category.nome.to_string(),
            races_simulated: summaries.len() as i32,
            results: summaries,
        });
    }

    warn_if_side_effect_fails(
        persist_other_category_news(&db.conn, &highlights, active_season.numero),
        "Falha ao persistir noticias de outras categorias do bloco especial",
    );
    warn_if_side_effect_fails(
        update_last_played(&meta_path),
        "Falha ao atualizar meta.json apos o bloco especial",
    );

    Ok(SimultaneousResults {
        categories_simulated,
        total_races_simulated,
        highlights,
    })
}

pub(crate) fn simulate_category_race(
    db: &mut Database,
    race_entry: &CalendarEntry,
    advance_player_round: bool,
) -> Result<(RaceResult, Vec<Injury>), String> {
    simulate_category_race_with_mode(
        db,
        race_entry,
        advance_player_round,
        RacePersistenceMode::Playable,
    )
}

pub(crate) fn simulate_historical_category_race(
    db: &mut Database,
    race_entry: &CalendarEntry,
) -> Result<(RaceResult, Vec<Injury>), String> {
    simulate_category_race_with_mode(db, race_entry, false, RacePersistenceMode::HistoricalDraft)
}

/// `career_id` da carreira aberta, deduzido do caminho do banco (`<saves>/<career_id>/career.db`).
///
/// A quebra da corrida simulada precisa dele pra semear exatamente como o caminho AO VIVO
/// (`event_seed`): o id da etapa é SEQUENCIAL por save ("R001"), então sem a carreira na mistura
/// dois saves diferentes rolariam a MESMA sorte na mesma rodada. Vazio se o caminho não tiver a
/// forma esperada — aí a semente perde só o tempero entre saves, não o determinismo dentro de um.
fn career_id_from_db(db: &Database) -> String {
    db.path
        .parent()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Uma quebra pré-rolada de corrida simulada, com as três caras que ela precisa ter: o que a
/// SIMULAÇÃO cobra (tempo/abandono), o que o SAVE registra (peça, volta, severidade) e o que a
/// ECONOMIA recebe de volta (peça danificada do time). Juntas aqui pra o caller não remontar nada.
struct PrerolledBreakdown {
    outcome: crate::simulation::race::MechanicalOutcome,
    row: crate::db::queries::race_breakdowns::RaceBreakdownRow,
    team_id: String,
    part: crate::car::PartType,
    severity: crate::car::breakdown::Severity,
}

/// Pré-rola a QUEBRA DE PEÇA do grid inteiro de uma corrida SIMULADA — a Fase 7 do Sistema de
/// Quebra, o que faltava pra corrida não-dirigida ter culpado.
///
/// Usa o MESMO cérebro e os MESMOS inputs do disparo ao vivo: o desgaste REAL do `team_car` de
/// cada equipe, o pit crew dela, a demanda P/H/A da pista e o clima determinístico da etapa. É
/// isso que faz a quebra ser consequência da economia e não um dado jogado por cima — o time
/// pobre, que estica peça por falta de caixa, é o que larga na pista.
///
/// Piloto cujo time não tem carro no banco (save antigo, grid sintético) simplesmente não rola:
/// a corrida segue sem quebra pra ele, em vez de inventar um carro médio que não existe.
///
/// Devolve `None` quando NENHUM carro do grid pôde ser lido — aí o sistema não tem como rodar e
/// quem responde pela falha mecânica continua sendo a pane do catálogo de incidentes. `Some`
/// (mesmo vazio) significa "a quebra rodou": o catálogo sai de cena e esta é a fonte única.
fn preroll_simulated_breakdowns(
    conn: &rusqlite::Connection,
    career_id: &str,
    race_entry: &CalendarEntry,
    category: &CategoryConfig,
    total_laps: i32,
    sim_drivers: &[SimDriver],
    teams: &[Team],
) -> Option<Vec<PrerolledBreakdown>> {
    use crate::car::breakdown::{is_enduro_duration, roll_race_breakdowns_cfg};
    use crate::db::queries::race_breakdowns::RaceBreakdownRow;
    use crate::db::queries::team_car as tcq;
    use crate::market::car_maintenance::maintenance_demand;
    use crate::simulation::race::MechanicalOutcome;

    let ev_seed = crate::commands::iracing::event_seed(career_id, &race_entry.id);
    let weather = crate::commands::iracing::race_breakdown_weather(
        race_entry.track_id,
        race_entry.week_of_year,
        ev_seed,
        false,
    );
    let track_pha = maintenance_demand(&[race_entry.track_id]);
    // Enduro: severidade abrandada (o grid não esvazia) + rampa de desgaste no fim.
    let is_enduro = is_enduro_duration(category.duracao_corrida_min);
    // Tenda de durabilidade por nível só em categoria GERIDA (teto ≥ 3); spec fica de fora.
    let apply_tent = crate::car::cost::category_ceiling(&race_entry.categoria) > 2;
    let laps = total_laps.max(1) as u32;

    let pit_by_team: HashMap<&str, f64> = teams
        .iter()
        .map(|t| (t.id.as_str(), t.pit_crew_quality))
        .collect();
    // O carro é da EQUIPE (os dois pilotos partilham) — lido uma vez e reusado.
    let mut car_cache: HashMap<String, Option<crate::car::Car>> = HashMap::new();

    let mut out = Vec::new();
    let mut algum_carro_lido = false;
    for driver in sim_drivers {
        let car = car_cache
            .entry(driver.team_id.clone())
            .or_insert_with(|| tcq::get_team_car(conn, &driver.team_id).ok().flatten());
        let Some(car) = car.as_ref() else {
            continue;
        };
        algum_carro_lido = true;
        // Semente por PILOTO sobre a da etapa — o mesmo `seed_for` do disparo ao vivo. Os dois
        // pilotos de um time rolam sortes independentes a partir do MESMO desgaste do carro.
        let mut seed = ev_seed;
        for b in driver.id.bytes() {
            seed = seed.wrapping_mul(0x0000_0100_0000_01B3).wrapping_add(b as u64);
        }
        let pit = pit_by_team
            .get(driver.team_id.as_str())
            .copied()
            .unwrap_or(50.0);
        let events = roll_race_breakdowns_cfg(
            car, laps, seed, pit, track_pha, weather, &[], is_enduro, apply_tent,
        );
        for ev in events {
            let label = ev.problem_label().to_string();
            out.push(PrerolledBreakdown {
                outcome: MechanicalOutcome {
                    pilot_id: driver.id.clone(),
                    lap: ev.lap,
                    is_dnf: ev.is_dnf(),
                    penalty_secs: ev.penalty_secs.unwrap_or(0),
                    label: label.clone(),
                },
                row: RaceBreakdownRow {
                    driver_id: driver.id.clone(),
                    part: ev.part.as_str().to_string(),
                    problem: ev.problem,
                    lap: ev.lap,
                    severity: ev.severity.key().to_string(),
                    penalty_secs: ev.penalty_secs,
                    forced: ev.forced,
                    label,
                },
                team_id: driver.team_id.clone(),
                part: ev.part,
                severity: ev.severity,
            });
        }
    }
    algum_carro_lido.then_some(out)
}

fn simulate_category_race_with_mode(
    db: &mut Database,
    race_entry: &CalendarEntry,
    advance_player_round: bool,
    persistence_mode: RacePersistenceMode,
) -> Result<(RaceResult, Vec<Injury>), String> {
    let category = get_category_config(&race_entry.categoria)
        .ok_or_else(|| "Categoria da corrida nao encontrada.".to_string())?;
    let active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let uses_regular_special_grid = uses_regular_special_event_grid(&race_entry.categoria);
    let mut teams = if uses_regular_special_grid {
        get_regular_special_event_teams(&db.conn, &race_entry.categoria)
    } else if runs_in_special_phase(&race_entry.categoria) {
        special_entry_queries::get_entry_teams_for_category(
            &db.conn,
            &active_season.id,
            &race_entry.categoria,
        )
    } else {
        team_queries::get_teams_by_category(&db.conn, &race_entry.categoria)
    }
    .map_err(|e| format!("Falha ao buscar equipes da categoria: {e}"))?;
    if persistence_mode == RacePersistenceMode::HistoricalDraft {
        teams.retain(|team| is_team_active_in_year(team, active_season.ano));
    }
    let regular_special_contracts = if uses_regular_special_grid {
        let contracts =
            get_regular_special_event_contracts(&db.conn, &race_entry.categoria, &teams)?;
        if contracts.is_empty() {
            None
        } else {
            Some(contracts)
        }
    } else {
        None
    };
    let drivers = if let Some(contracts) = regular_special_contracts.as_ref() {
        get_drivers_for_contracts(&db.conn, contracts)?
    } else if uses_regular_special_grid {
        get_drivers_for_team_lineups(&db.conn, &teams)?
    } else {
        driver_queries::get_drivers_by_active_category(&db.conn, &race_entry.categoria)
            .map_err(|e| format!("Falha ao buscar pilotos da categoria: {e}"))?
    };

    let team_by_driver = if let Some(contracts) = regular_special_contracts.as_ref() {
        build_regular_contract_team_lookup(contracts, &teams)
    } else if runs_in_special_phase(&race_entry.categoria) {
        build_special_team_lookup(&db.conn, &teams, &race_entry.categoria)?
    } else {
        build_team_lookup(&teams)
    };
    // Lesionados CONTINUAM correndo (com penalidade de ritmo decrescente aplicada abaixo);
    // ninguém é mais removido da grade por lesão. Só ficam de fora os aposentados — inclusive
    // quem pendurou o capacete no meio da temporada por lesão grave, que segue congelado na
    // classificação mas não volta à pista.
    let driver_pool: Vec<_> = drivers
        .iter()
        .filter(|driver| driver.status != crate::models::enums::DriverStatus::Aposentado)
        .collect();
    // Lesões ativas da categoria, por piloto: alimentam a penalidade de ritmo do machucado.
    let injuries_by_driver: HashMap<String, Injury> =
        crate::db::queries::injuries::get_active_injuries_for_category(
            &db.conn,
            &race_entry.categoria,
        )
        .map_err(|e| format!("Falha ao buscar lesoes ativas da categoria: {e}"))?
        .into_iter()
        .map(|injury| (injury.pilot_id.clone(), injury))
        .collect();
    // Pressão de campeonato: standings da categoria (pontos de todos) + corridas
    // restantes + pontos do vencedor (P1 + volta rápida). Usado por piloto abaixo.
    let title_points: Vec<f64> = drivers.iter().map(|d| d.stats_temporada.pontos).collect();
    let races_left = (category.corridas_por_temporada as i32 - race_entry.rodada + 1).max(1) as u32;
    let max_race_points =
        (get_points_for_position(1, category.id == "endurance") + BONUS_FASTEST_LAP) as f64;

    // Casa cheia: interesse "de local" do evento (categoria + fase + rodada + papel
    // narrativo), SEM protagonismo do jogador nem drama de título — esses já entram
    // pela pressão de campeonato acima. Vale igual pra todos os pilotos do grid; a
    // variação por piloto vem só da forma recente ("algo a provar"). Calculado uma vez.
    let venue_ctx = EventInterestContext {
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
    let event_stakes = crate::simulation::pressure::event_stakes_from_score(
        calculate_expected_event_interest(&venue_ctx).score as f64,
    );

    // Pressão de Duelo (Nemesis): a rivalidade pessoal do jogador é a 3ª fonte de pressão
    // (clutch/choke via pressure.rs), SIMÉTRICA entre jogador e rival e só acesa quando os
    // dois vão brigar de verdade (gate de pace). Substitui o antigo bônus fixo de skill
    // (+2/+1), que não passava pela máquina de resiliência. Só vale com o jogador nesta
    // corrida. Mapa driver_id -> (percebida do par com o jogador, pace do oponente p/ o gate).
    let duel_by_driver: std::collections::HashMap<String, (f64, f64)> = {
        let mut m = std::collections::HashMap::new();
        if let Some(player) = driver_pool.iter().find(|d| d.is_jogador) {
            let player_id = player.id.clone();
            let player_skill = player.atributos.skill;
            let base_skill: std::collections::HashMap<String, f64> = driver_pool
                .iter()
                .map(|d| (d.id.clone(), d.atributos.skill))
                .collect();
            let current =
                crate::db::queries::player_nemesis::get_current_nemesis(&db.conn).unwrap_or(None);
            let interests =
                crate::commands::career::select_player_interests(&db.conn, current.as_deref());
            // Lado do jogador: sente o duelo contra o rival presente mais quente (o Nemesis
            // se estiver na corrida). Guardamos a percebida e o pace desse rival.
            let mut player_strongest: Option<(f64, f64)> = None;
            for r in interests.nemesis.into_iter().chain(interests.rivais) {
                // Só rivais realmente presentes nesta corrida.
                let Some(&rival_skill) = base_skill.get(&r.driver_id) else {
                    continue;
                };
                // Lado do rival: sente o duelo contra o jogador.
                m.insert(r.driver_id.clone(), (r.perceived, player_skill));
                if player_strongest.map_or(true, |(p, _)| r.perceived > p) {
                    player_strongest = Some((r.perceived, rival_skill));
                }
            }
            if let Some(info) = player_strongest {
                m.insert(player_id, info);
            }
        }
        m
    };

    let mut orphaned_drivers = Vec::new();
    let sim_drivers: Vec<SimDriver> = driver_pool
        .into_iter()
        .filter_map(|driver| match team_by_driver.get(&driver.id) {
            Some(team) => {
                let mut sd =
                    SimDriver::from_driver_team_and_track(driver, team, race_entry.track_id);
                // Conhecimento de pista: pista nova/pouco conhecida = mais lento.
                // Desconta do skill E do ritmo de classificação (corrida + quali).
                let pen =
                    track_penalty_for(driver, race_entry.track_id, active_season.numero as i32);
                sd.skill = (sd.skill as f64 - pen).max(5.0).round() as u8;
                sd.ritmo_classificacao =
                    (sd.ritmo_classificacao as f64 - pen).max(5.0).round() as u8;
                // Lesão: o machucado CORRE, mas com o ritmo reduzido. A penalidade é cheia
                // logo após a batida e DIMINUI a cada etapa até sarar (volta enferrujado e
                // vai melhorando). Fração do skill × (corridas restantes / total). Liga os
                // campos skill_penalty da lesão, antes só gravados e nunca lidos.
                if let Some(injury) = injuries_by_driver.get(&driver.id) {
                    let recovery = (injury.races_remaining as f64
                        / injury.races_total.max(1) as f64)
                        .clamp(0.0, 1.0);
                    let inj_pen = sd.skill as f64 * injury.skill_penalty * recovery;
                    sd.skill = (sd.skill as f64 - inj_pen).max(5.0).round() as u8;
                    sd.ritmo_classificacao =
                        (sd.ritmo_classificacao as f64 - inj_pen).max(5.0).round() as u8;
                }
                // (Chuva NÃO entra aqui: a sim já aplica rain_skill_penalty no score
                // em simulation/race.rs + qualifying.rs — aplicar de novo dobraria.)
                // Motivação: um piloto desmotivado corre ABAIXO do próprio skill (efeito
                // modesto e calibrável). Aplicado ao skill efetivo antes da pressão, para
                // o headroom já enxergar o rendimento real do piloto.
                let mot = crate::simulation::pressure::motivation_pace_delta(sd.motivacao);
                sd.skill = (sd.skill as f64 + mot).clamp(5.0, 100.0).round() as u8;
                sd.ritmo_classificacao =
                    (sd.ritmo_classificacao as f64 + mot).clamp(5.0, 100.0).round() as u8;
                // Pressão de campeonato (clutch/choke): ajusta ritmo + taxa de erro.
                let pctx = crate::simulation::pressure::title_context(
                    driver.stats_temporada.pontos,
                    &title_points,
                    races_left,
                    max_race_points,
                );
                let title_eff = crate::simulation::pressure::pressure_for(
                    &pctx,
                    races_left,
                    driver.atributos.mentalidade,
                    driver.atributos.experiencia,
                );
                // Pressão de casa cheia (universal): pesa mais em quem vem em má fase.
                let event_eff = crate::simulation::pressure::event_pressure_for(
                    event_stakes,
                    recent_avg_finish(&driver.ultimos_resultados),
                    driver.atributos.mentalidade,
                    driver.atributos.experiencia,
                );
                // Pressão de Duelo (Nemesis): 3ª fonte, acesa só quando jogador e rival
                // brigam de verdade (gate de pace). Herda mentalidade/experiência — não é
                // mais um bônus fixo. Percebida + pace do oponente vêm de `duel_by_driver`.
                let duel_eff = match duel_by_driver.get(&driver.id) {
                    Some(&(perceived, opponent_skill)) => {
                        crate::simulation::pressure::rivalry_pressure_for(
                            perceived,
                            sd.skill as f64,
                            opponent_skill,
                            driver.atributos.mentalidade,
                            driver.atributos.experiencia,
                            1.0,
                        )
                    }
                    None => crate::simulation::pressure::PressureEffect::NONE,
                };
                let peff = crate::simulation::pressure::combine(
                    crate::simulation::pressure::combine(title_eff, event_eff),
                    duel_eff,
                );
                // Headroom: converte o Δpace da pressão em pontos de skill conforme onde
                // o piloto está na curva (subir tem teto, cair tem chão). Usa o skill
                // efetivo do fim de semana (já com a penalidade de conhecimento de pista).
                let hr = crate::simulation::pressure::headroom_pace_mult(
                    sd.skill as f64,
                    peff.pace_delta >= 0.0,
                );
                let pace_delta = peff.pace_delta * hr;
                sd.skill = (sd.skill as f64 + pace_delta).clamp(5.0, 100.0).round() as u8;
                sd.ritmo_classificacao = (sd.ritmo_classificacao as f64 + pace_delta)
                    .clamp(5.0, 100.0)
                    .round() as u8;
                sd.pressure_error_mult = peff.error_mult;
                Some(sd)
            }
            None if persistence_mode == RacePersistenceMode::HistoricalDraft => None,
            None => {
                orphaned_drivers.push(format!("{} ({})", driver.nome, driver.id));
                None
            }
        })
        .collect();

    if !orphaned_drivers.is_empty() {
        return Err(format!(
            "Pilotos ativos sem equipe na categoria '{}': {}",
            race_entry.categoria,
            orphaned_drivers.join(", ")
        ));
    }

    if sim_drivers.is_empty() {
        return Err(format!(
            "Nenhum piloto ativo encontrado para a categoria '{}' na rodada {}. \
             A corrida nao foi disputada.",
            race_entry.categoria, race_entry.rodada
        ));
    }

    let ctx = SimulationContext::from_calendar_entry(
        race_entry,
        category.tier,
        race_entry.rodada >= category.corridas_por_temporada as i32,
    );
    let mut rng = rand::thread_rng();
    let catalog = IncidentCatalog::load(&db.conn).unwrap_or_else(|_| IncidentCatalog::empty());

    // QUEBRA DE PEÇA (Fase 7): pré-rola o grid inteiro sobre o desgaste REAL dos carros e deixa
    // a simulação cobrar o preço. Só na carreira jogável — o rascunho histórico reconstrói
    // temporadas antigas com grid sintético, onde não existe `team_car` pra ler. Quando roda,
    // ela vira a FONTE ÚNICA de pane: a mecânica genérica do catálogo é desligada na simulação.
    let prerolled = if persistence_mode == RacePersistenceMode::Playable {
        preroll_simulated_breakdowns(
            &db.conn,
            &career_id_from_db(db),
            race_entry,
            category,
            ctx.total_laps,
            &sim_drivers,
            &teams,
        )
    } else {
        None
    };
    let mechanicals: Option<Vec<crate::simulation::race::MechanicalOutcome>> = prerolled
        .as_ref()
        .map(|list| list.iter().map(|p| p.outcome.clone()).collect());

    let mut result = run_full_race_with_breakdowns(
        &sim_drivers,
        &ctx,
        category.id == "endurance",
        &catalog,
        mechanicals.as_deref(),
        &mut rng,
    );
    if is_multiclass_category(&race_entry.categoria) {
        apply_special_class_scoring(&mut result, &teams, category.id == "endurance");
    }
    let next_round = if advance_player_round {
        Some((active_season.rodada_atual + 1).min(category.corridas_por_temporada as i32))
    } else {
        None
    };

    // Só o que a corrida de fato COBROU: quem já tinha abandonado por batida antes da volta da
    // quebra não conta (ver `RaceResult::applied_mechanicals`). Daqui saem as três consequências.
    let applied: Vec<&PrerolledBreakdown> = match prerolled.as_ref() {
        Some(list) => result
            .applied_mechanicals
            .iter()
            .filter_map(|&i| list.get(i))
            .collect(),
        None => Vec::new(),
    };

    // Abandono POR QUEBRA precisa apontar pro catálogo `Mechanical`: é por esse id que o
    // histórico de DNF e o beat de Abandono da notícia reconhecem a pane. O `dnf_reason` a
    // simulação já gravou com a frase da peça. MESMO tratamento do import do iRacing.
    if applied.iter().any(|p| p.severity == crate::car::breakdown::Severity::Dnf) {
        let mech_id: Option<String> = db
            .conn
            .query_row(
                "SELECT id FROM incident_catalog WHERE incident_source = 'Mechanical' LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok();
        if let Some(id) = mech_id {
            for p in applied
                .iter()
                .filter(|p| p.severity == crate::car::breakdown::Severity::Dnf)
            {
                if let Some(dr) = result
                    .race_results
                    .iter_mut()
                    .find(|d| d.pilot_id == p.row.driver_id && d.is_dnf)
                {
                    dr.dnf_catalog_id = Some(id.clone());
                }
            }
        }
    }

    // Feedback físico da quebra (§4.6), por TIME: Leve segue, Grave vira fim de vida, DNF
    // destrói a peça (troca forçada a débito no fechamento da fatura). É o que fecha o laço
    // quebra → economia, igual à corrida ao vivo.
    let team_breakdowns: HashMap<
        String,
        Vec<(crate::car::PartType, crate::car::breakdown::Severity)>,
    > = {
        let mut map: HashMap<String, Vec<_>> = HashMap::new();
        for p in &applied {
            map.entry(p.team_id.clone()).or_default().push((p.part, p.severity));
        }
        map
    };

    let new_injuries_out = persist_race_result_tx(
        db,
        race_entry,
        &result,
        &teams,
        &active_season,
        category,
        next_round,
        persistence_mode,
        None, // sim offline: sem inputs do jogador → sem estilo
        0,    // sim offline: sem paradas reais (a IA modela pela duração)
        &team_breakdowns,
        &mut rng,
    )?;

    // Registra os desfechos pra tela pós-corrida e pra notícia. Best-effort: uma falha aqui não
    // desfaz o resultado já persistido — a corrida aconteceu, só o detalhe da peça se perde.
    if !applied.is_empty() {
        let rows: Vec<_> = applied.iter().map(|p| p.row.clone()).collect();
        warn_if_side_effect_fails(
            crate::db::queries::race_breakdowns::insert_breakdowns_batch(
                &db.conn,
                &race_entry.id,
                &rows,
            )
            .map_err(|e| e.to_string()),
            "Falha ao gravar race_breakdowns da corrida simulada",
        );
    }

    Ok((result, new_injuries_out))
}

/// Persiste um `RaceResult` na carreira dentro de UMA transação: standings/pontos,
/// recuperação+geração de lesões, resumo da corrida, avanço de rodada, hierarquia
/// e rivalidades. É o "rabo" compartilhado entre a simulação OFFLINE e o IMPORT do
/// iRacing — ambos produzem um `RaceResult` e caem aqui, então a carreira reage
/// igual nos dois caminhos. Retorna as lesões novas (pode incluir pilotos de IA).
#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_race_result_tx(
    db: &mut Database,
    race_entry: &CalendarEntry,
    result: &RaceResult,
    teams: &[Team],
    active_season: &Season,
    category: &CategoryConfig,
    next_round: Option<i32>,
    persistence_mode: RacePersistenceMode,
    // Estilo de pilotagem do JOGADOR (só quando ele correu no iRacing); `None` na sim offline.
    // Modula o desgaste SÓ do carro dele.
    player_style: Option<crate::car::driving_style::StyleFactors>,
    // Nº de paradas REAIS do jogador (do SDK) — alívio de gasto de peça do enduro (só no carro
    // dele; 10%/parada, teto 30%). 0 na sim offline (a IA modela as paradas pela duração).
    player_pits: u32,
    // Feedback físico da quebra (§4.6): peças que largaram nesta corrida, por time. Vazio na sim
    // offline (sem quebra) e nas corridas fora da categoria do jogador.
    team_breakdowns: &std::collections::HashMap<
        String,
        Vec<(crate::car::PartType, crate::car::breakdown::Severity)>,
    >,
    rng: &mut impl rand::Rng,
) -> Result<Vec<Injury>, String> {
    let mut new_injuries_out: Vec<Injury> = Vec::new();
    db.transaction(|tx| {
        // 0. Guarda de idempotência: o status foi checado fora da transação,
        // então uma invocação concorrente do mesmo comando pode ter concluído
        // esta corrida nesse meio-tempo. Re-checa sob o lock de escrita para
        // nunca persistir o mesmo resultado duas vezes.
        let current_status = calendar_queries::get_calendar_entry_by_id(tx, &race_entry.id)?
            .map(|entry| entry.status)
            .ok_or_else(|| {
                crate::db::connection::DbError::NotFound(format!(
                    "corrida '{}' nao encontrada ao persistir resultado",
                    race_entry.id
                ))
            })?;
        if current_status.as_str() != "Pendente" {
            return Err(crate::db::connection::DbError::InvalidData(format!(
                "corrida '{}' ja foi concluida por outra simulacao concorrente",
                race_entry.id
            )));
        }

        // 1. Processo de recuperação das lesões já ativas
        crate::evolution::injury::process_injury_recovery(tx, &race_entry.categoria)?;

        // 2. Aplica pontuações normais
        let economic_health = global_economic_health_for_season(active_season.numero as i32);
        // Próximas pistas da categoria (após esta rodada) — o cérebro do carro corta pelo
        // horizonte de cada time. Computado aqui e passado adiante.
        let upcoming_track_ids: Vec<u32> =
            calendar_queries::get_calendar(tx, &race_entry.season_id, &race_entry.categoria)?
                .into_iter()
                .filter(|entry| entry.rodada > race_entry.rodada)
                .map(|entry| entry.track_id)
                .collect();
        // Condições reais da corrida → o desgaste da grade toda responde à pista + clima
        // desta rodada (o cérebro de manutenção reage a corridas brutais). Todos os 4 canais
        // vêm dos campos AUTORITATIVOS persistidos da MESMA história que o iRacing roda: clima
        // e temperatura no `race_entry`; umidade e vento nas colunas da etapa (via A, gravadas
        // por `resolve_and_persist_race_weather`). Saves antigos → default neutro (45/25).
        let (humidity, wind_kmh) = tx
            .query_row(
                "SELECT umidade, vento FROM calendar WHERE id = ?1",
                [race_entry.id.as_str()],
                |row| Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?)),
            )
            .unwrap_or((
                crate::car::breakdown::Weather::NEUTRAL.humidity,
                crate::car::breakdown::Weather::NEUTRAL.wind_kmh,
            ));
        let weather = crate::car::breakdown::Weather {
            wetness: race_entry.clima.wetness(),
            temperature: race_entry.temperatura,
            humidity,
            wind_kmh,
        };
        // Duração da corrida (min) da categoria — acima do gate de enduro, o desgaste de peça
        // sobe pra grade toda. Fonte de verdade = config da categoria (mesma que a sim usa em
        // context.rs); categoria não resolvida → 30 (sprint/neutro).
        let duracao_min = crate::constants::categories::get_category_config(&race_entry.categoria)
            .map(|c| c.duracao_corrida_min)
            .unwrap_or(30);
        let wear_conditions = crate::market::car_maintenance::WearConditions::from_race(
            race_entry.track_id,
            weather,
            duracao_min,
        );
        // Time do JOGADOR (só se há estilo capturado) — o estilo modula o desgaste só do carro
        // dele. Piloto → contrato ativo → equipe.
        let player_team_id: Option<String> = if player_style.is_some() {
            crate::db::queries::drivers::get_player_driver(tx)
                .ok()
                .and_then(|p| {
                    crate::db::queries::contracts::get_active_contract_for_pilot(tx, &p.id)
                        .ok()
                        .flatten()
                })
                .map(|c| c.equipe_id)
        } else {
            None
        };
        // Peça 3 · desconfiança mecânica → menos desgaste: times com DNF MECÂNICO nas rodadas
        // recentes POUPAM as peças (o loop emergente da quebra: quebrou → desconfia → poupa →
        // quebra menos). Janela = as 3 rodadas ANTERIORES (a atual ainda não foi persistida).
        let team_cautions: std::collections::HashMap<
            String,
            crate::car::driving_style::StyleFactors,
        > = {
            let r = race_entry.rodada;
            crate::db::queries::race_history::mechanical_dnf_counts_by_team(
                tx,
                &active_season.id,
                &race_entry.categoria,
                (r - 3).max(1),
                r - 1,
            )
            .unwrap_or_default()
            .into_iter()
            .map(|(team, count)| {
                (
                    team,
                    crate::car::driving_style::StyleFactors::uniform(mechanical_caution_factor(
                        count,
                    )),
                )
            })
            .collect()
        };
        apply_race_result_to_database(
            tx,
            result,
            teams,
            economic_health,
            &race_entry.categoria,
            persistence_mode,
            race_entry.track_id,
            active_season.numero as i32,
            race_entry.rodada,
            &upcoming_track_ids,
            wear_conditions,
            venue_prestige_score(race_entry),
            player_team_id.as_deref(),
            player_style,
            player_pits,
            &team_cautions,
            team_breakdowns,
        )?;

        // 3. Verifica os incidentes recém-gerados e processa possíveis lesões
        let flat_incidents: Vec<_> = result
            .race_results
            .iter()
            .flat_map(|r| r.incidents.clone())
            .collect();
        let new_injuries = crate::evolution::injury::process_new_injuries(
            tx,
            active_season.numero as i32,
            &race_entry.id,
            &flat_incidents,
            &mut *rng,
        )?;
        new_injuries_out = new_injuries;

        // 3b. Lesão GRAVE pode (raramente, só IA) encerrar a carreira no meio da temporada.
        // Quem pendura o capacete fica congelado na classificação e a vaga é preenchida por
        // um substituto que entra como NOME NOVO (pilot_id próprio → começa do zero).
        process_severe_injury_retirements(tx, &new_injuries_out, active_season, &mut *rng)?;

        // 4. Salva o resumo da corrida e avança
        crate::db::queries::races::insert_race_results_batch(
            tx,
            &race_entry.id,
            &result.race_results,
        )?;
        calendar_queries::mark_race_completed(tx, &race_entry.id)?;
        if let Some(round) = next_round {
            season_queries::update_season_rodada(tx, &active_season.id, round)?;
        }
        season_queries::move_to_encerramento_if_completed(tx, active_season)?;

        // 5. Processa hierarquia interna das equipes da categoria
        if !runs_in_special_phase(&race_entry.categoria) {
            crate::hierarchy::orders::process_hierarchy_for_category(
                tx,
                &result.race_results,
                &race_entry.categoria,
                race_entry.rodada,
                category.corridas_por_temporada as i32,
                active_season.numero,
            )?;
        }

        // 6. Processa rivalidades por disputa de campeonato (últimas rodadas)
        crate::rivalry::process_championship_rivalry(
            tx,
            &race_entry.categoria,
            race_entry.rodada,
            category.corridas_por_temporada as i32,
            active_season.numero,
        )?;

        // 7. Processa rivalidades geradas por colisões bilaterais (fatos da corrida)
        crate::rivalry::process_collisions_rivalry(
            tx,
            &flat_incidents,
            &race_entry.categoria,
            race_entry.rodada,
            active_season.numero,
        )?;

        // 8. Rivalidade entre EQUIPES (Fontes 3+4 + moral de derby): reusa os mesmos fatos
        // da corrida. Mapa driver→time e melhor chegada por time, do próprio resultado.
        let team_by_driver: std::collections::HashMap<String, String> = result
            .race_results
            .iter()
            .map(|r| (r.pilot_id.clone(), r.team_id.clone()))
            .collect();
        // Fonte 3 — guerra na pista: agrega as colisões por par de times diferentes.
        crate::rivalry::team::process_team_collisions_rivalry(
            tx,
            &flat_incidents,
            &team_by_driver,
            &race_entry.categoria,
            race_entry.rodada,
            active_season.numero,
        )?;
        // Fonte 4 — transbordamento: rivalidades intensas de piloto cross-time pingam nos times.
        crate::rivalry::team::process_driver_rivalry_bleed(
            tx,
            &team_by_driver,
            &race_entry.categoria,
            race_entry.rodada,
            active_season.numero,
        )?;
        // Tier 2 — pulso de moral de derby: bater o rival empurra a moral (sutil, simétrico).
        let mut team_best_finish: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        for r in &result.race_results {
            team_best_finish
                .entry(r.team_id.clone())
                .and_modify(|p| {
                    if r.finish_position < *p {
                        *p = r.finish_position;
                    }
                })
                .or_insert(r.finish_position);
        }
        crate::rivalry::team::apply_derby_morale(tx, &team_best_finish)?;

        Ok(())
    })
    .map_err(|e| format!("Falha ao persistir resultado da corrida: {e}"))?;

    Ok(new_injuries_out)
}

/// Esvazia o assento do piloto no time (libera a vaga para reposição), mantendo o
/// companheiro no lugar.
fn vacate_team_seat(
    tx: &rusqlite::Transaction,
    team_id: &str,
    driver_id: &str,
) -> Result<(), DbError> {
    if let Some(team) = team_queries::get_team_by_id(tx, team_id)? {
        let p1 = team.piloto_1_id.filter(|id| id.as_str() != driver_id);
        let p2 = team.piloto_2_id.filter(|id| id.as_str() != driver_id);
        team_queries::update_team_pilots(tx, team_id, p1.as_deref(), p2.as_deref())?;
    }
    Ok(())
}

/// Aposentadoria por lesão grave no meio da temporada (só IA). Para cada lesão GRAVE
/// recém-gerada, rola a chance ponderada por idade; se aposentar: registra no hall dos
/// aposentados, desativa a lesão (para a recuperação por corrida não "ressuscitar" o
/// piloto para Ativo), marca Aposentado MANTENDO a categoria (fica congelado na
/// classificação da temporada), rescinde o contrato, esvazia o assento e contrata um
/// substituto que entra como nome novo (pilot_id próprio → não herda resultados).
fn process_severe_injury_retirements(
    tx: &rusqlite::Transaction,
    new_injuries: &[Injury],
    active_season: &Season,
    rng: &mut impl rand::Rng,
) -> Result<(), DbError> {
    use crate::models::enums::InjuryType;
    for injury in new_injuries {
        if injury.injury_type != InjuryType::Grave {
            continue;
        }
        let mut driver = driver_queries::get_driver(tx, &injury.pilot_id)?;
        // O piloto do jogador NUNCA é aposentado à força (decisão de design).
        if driver.is_jogador {
            continue;
        }
        let chance =
            crate::evolution::retirement::severe_injury_retirement_chance(driver.idade);
        if !rng.gen_bool(chance.clamp(0.0, 1.0)) {
            continue;
        }

        let final_category = driver
            .categoria_atual
            .clone()
            .unwrap_or_else(|| "SemCategoria".to_string());
        let reason = rust_i18n::t!("race.retirement.injury", age = driver.idade).to_string();

        // 1. Hall dos aposentados (snapshot de carreira).
        crate::evolution::pipeline::persist_retired_driver(
            tx,
            &driver,
            active_season,
            &final_category,
            &reason,
        )
        .map_err(DbError::InvalidData)?;

        // 2. Desativa a lesão: senão a recuperação por corrida a zeraria e devolveria o
        // piloto aposentado ao status Ativo.
        crate::db::queries::injuries::update_injury_status(tx, &injury.id, 0, false)?;

        // 3. Marca Aposentado, MANTENDO categoria_atual (fica congelado na classificação;
        // a grade filtra Aposentado e a virada de temporada pula quem não é Ativo).
        crate::evolution::retirement::process_retirement(&mut driver);
        driver_queries::update_driver(tx, &driver).map_err(|e| {
            DbError::InvalidData(format!("Falha ao aposentar piloto lesionado: {e}"))
        })?;

        // 4. Libera a vaga e contrata um substituto (agente livre licenciado ou novato).
        if let Some(contract) =
            contract_queries::get_active_regular_contract_for_pilot(tx, &driver.id)?
        {
            let team_id = contract.equipe_id.clone();
            contract_queries::update_contract_status(
                tx,
                &contract.id,
                &crate::models::enums::ContractStatus::Rescindido,
            )?;
            vacate_team_seat(tx, &team_id, &driver.id)?;
            crate::commands::career::backfill_team_vacancy(
                tx,
                &team_id,
                active_season.numero,
                active_season.ano,
            )
            .map_err(DbError::InvalidData)?;
        }
    }
    Ok(())
}

/// Resumo do que foi gravado ao importar uma corrida do iRacing — devolvido ao
/// front para o aviso/pop-up.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedRaceSummary {
    pub race_id: String,
    pub track_name: String,
    pub categoria: String,
    pub rodada: i32,
    /// Pilotos efetivamente gravados (após filtrar carros sem piloto da carreira).
    pub saved_drivers: usize,
    /// Carros da sessão descartados por não casarem com nenhum piloto da carreira.
    pub dropped_unmatched: usize,
    pub player_position: Option<i32>,
    pub player_points: Option<i32>,
    pub player_is_dnf: bool,
    pub winner_name: Option<String>,
    /// Custo do conserto do carro do jogador (0 se não houve batida cobrável).
    pub repair_cost: f64,
    /// Severidade da pior batida do jogador (leve/moderado/grave/destruído/catastrófico/nenhum).
    pub repair_severity: String,
    /// Quantas vezes o jogador bateu (de forma cobrável) NESTA temporada, incluindo esta.
    pub repair_count: i32,
    /// Frase pronta do pop-up (com valor e contagem já preenchidos). Vazia se cost=0.
    pub repair_message: String,
    /// Fatura de manutenção do carro (gasolina/pneus + itens do conserto). Sempre presente.
    #[serde(default)]
    pub maintenance: MaintenanceBreakdown,
}

/// Um item da fatura de manutenção do carro (gasolina, pneus, carroceria...). Só
/// itens com custo > 0 entram na lista.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceItem {
    pub key: String,
    pub label: String,
    pub cost: f64,
}

/// Fatura de manutenção do carro da corrida — INFORMATIVA: itemiza dinheiro que já
/// sai do caixa (parcela de manutenção da operação da rodada + conserto da batida).
/// Não cria cobrança nova; só mostra onde o dinheiro foi.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaintenanceBreakdown {
    pub total: f64,
    pub items: Vec<MaintenanceItem>,
}

/// Como o custo do conserto se divide entre os itens mecânicos, por severidade. NÃO
/// simula o dano real — só fatia o valor total entre componentes plausíveis. Frações
/// somam ~1.0.
/// Divisão do custo de conserto por peça, como (key, proporção). A `key` é o token
/// da peça; o rótulo de display resolve em `maintenance_label`.
fn damage_split(severity: &str) -> Vec<(&'static str, f64)> {
    match severity {
        "moderado" => vec![("carroceria", 0.70), ("suspensao", 0.30)],
        "grave" => vec![
            ("carroceria", 0.50),
            ("suspensao", 0.25),
            ("freio", 0.15),
            ("embreagem", 0.10),
        ],
        "destruído" => vec![
            ("carroceria", 0.40),
            ("suspensao", 0.20),
            ("motor", 0.15),
            ("freio", 0.12),
            ("cambio", 0.08),
            ("embreagem", 0.05),
        ],
        "catastrófico" => vec![
            ("carroceria", 0.32),
            ("motor", 0.22),
            ("suspensao", 0.18),
            ("cambio", 0.12),
            ("freio", 0.08),
            ("embreagem", 0.08),
        ],
        _ => vec![],
    }
}

/// Rótulo de display de um item de manutenção (i18n) a partir da `key`.
fn maintenance_label(key: &str) -> String {
    let full = format!("maintenance.{key}");
    rust_i18n::t!(&full).to_string()
}

/// Fração do custo de OPERAÇÃO da categoria que vira "manutenção do carro" (gasolina,
/// pneus, revisão). O resto da operação é salário/estrutura/etc. — fora da fatura.
const CAR_UPKEEP_FRACTION: f64 = 0.30;

/// Monta a fatura de manutenção do carro da corrida. Piso (sempre): a parcela de
/// manutenção da operação da rodada, fatiada em gasolina (peso por voltas) + pneus
/// (peso por desgaste). Se houve batida, soma os itens mecânicos do conserto.
pub(crate) fn compute_maintenance_breakdown(
    category: &str,
    final_tire_wear: f64,
    laps_completed: i32,
    repair_cost: f64,
    repair_severity: &str,
) -> MaintenanceBreakdown {
    let rounds = get_category_config(category)
        .map(|c| c.corridas_por_temporada as i32)
        .unwrap_or(12)
        .max(1);
    let operating_per_round = category_finance_scale(category).operating_cost_midpoint() / rounds as f64;
    let floor = operating_per_round * CAR_UPKEEP_FRACTION;

    // Pesos dinâmicos: pneu cresce com desgaste, gasolina com voltas rodadas. Mantêm
    // a soma = piso (informativo), mas variam a divisão de corrida pra corrida.
    let wear = final_tire_wear.clamp(0.0, 1.0);
    let tire_w = 0.5 + wear; // 0.5..1.5
    let fuel_w = 0.6 + (laps_completed.max(0) as f64 / 18.0).min(1.0); // 0.6..1.6
    let sum_w = (tire_w + fuel_w).max(0.001);
    let gasolina = (floor * fuel_w / sum_w).round();
    let pneus = (floor * tire_w / sum_w).round();

    let mut items = Vec::new();
    if gasolina > 0.0 {
        items.push(MaintenanceItem { key: "gasolina".into(), label: maintenance_label("gasolina"), cost: gasolina });
    }
    if pneus > 0.0 {
        items.push(MaintenanceItem { key: "pneus".into(), label: maintenance_label("pneus"), cost: pneus });
    }
    if repair_cost > 0.0 {
        for (key, prop) in damage_split(repair_severity) {
            let cost = (repair_cost * prop).round();
            if cost > 0.0 {
                items.push(MaintenanceItem { key: key.into(), label: maintenance_label(key), cost });
            }
        }
    }
    let total = items.iter().map(|i| i.cost).sum();
    MaintenanceBreakdown { total, items }
}

/// Fração do custo de OPERAÇÃO da categoria que cada severidade de batida cobra em
/// conserto. A batida já chega rebaixada se o carro cruzou a linha (race_monitor).
fn repair_cost_fraction(severity: &str) -> f64 {
    match severity {
        "moderado" => 0.03,
        "grave" => 0.10,
        "destruído" => 0.25,
        "catastrófico" => 0.45,
        _ => 0.0, // leve / nenhum: sem cobrança
    }
}

/// Custo de conserto do carro do jogador: fração(severidade) × custo de operação da
/// categoria × fator do carro × margem aleatória (±35%). A margem dá variação dentro
/// do tier — duas batidas "graves" não custam idêntico. Calculado UMA vez e persistido
/// (caixa + summary + fatura), então não muda ao reabrir a tela. SÓ jogador.
fn compute_repair_cost(
    severity: &str,
    category: &str,
    car_performance: f64,
    rng: &mut impl rand::Rng,
) -> f64 {
    let frac = repair_cost_fraction(severity);
    if frac <= 0.0 {
        return 0.0;
    }
    let operating = category_finance_scale(category).operating_cost_midpoint();
    let car_factor =
        1.0 + crate::simulation::math::normalize_car_performance(car_performance) / 100.0 * 0.5;
    let variance = 0.65 + rng.gen::<f64>() * 0.70; // 0.65..1.35
    (frac * operating * car_factor * variance).round()
}

/// Formata um valor em dólar no estilo en-US: "$18,500" (negativo: "-$18,500").
fn format_brl(value: f64) -> String {
    let n = value.round() as i64;
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{}${}", if n < 0 { "-" } else { "" }, out)
}

/// Lê/incrementa a contagem de batidas COBRÁVEIS do jogador na temporada atual,
/// persistida em `iracing_repair_count.json` no diretório do save. Reinicia a cada
/// temporada. Devolve a contagem JÁ incluindo esta batida.
fn bump_repair_count(career_dir: &Path, season_number: i32) -> i32 {
    let path = career_dir.join("iracing_repair_count.json");
    let stored: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null);
    let same_season = stored.get("season").and_then(|v| v.as_i64()) == Some(season_number as i64);
    let prev = if same_season {
        stored.get("count").and_then(|v| v.as_i64()).unwrap_or(0)
    } else {
        0
    };
    let count = (prev + 1) as i32;
    let _ = std::fs::write(
        &path,
        serde_json::json!({ "season": season_number, "count": count }).to_string(),
    );
    count
}

/// Frase pronta do pop-up de conserto, variando por VALOR (severidade) e por
/// QUANTAS vezes já aconteceu nesta temporada. Sorteio dentro da célula.
fn pick_repair_message(
    rng: &mut impl rand::Rng,
    severity: &str,
    valor: &str,
    count: i32,
) -> String {
    const FIRST: &[&str] = &[
        "Acontece. O pessoal do box já está no conserto — {valor}. Cabeça erguida pra próxima.",
        "É assim que se aprende. {valor} no reparo; a gente absorve dessa vez.",
        "Bateu, mas trouxe lições. Custou {valor} — na próxima a gente traz inteiro.",
    ];
    const REPEAT: &[&str] = &[
        "De novo. O conserto foi {valor}. Precisamos trazer esse carro pra casa inteiro.",
        "Já são {n}. Mais {valor} que saíram do caixa — isso começa a pesar.",
        "Segunda chamada. {valor}. O orçamento não é infinito, combinado?",
    ];
    const CHRONIC: &[&str] = &[
        "Outra?! {valor}. Já são {n} carros destruídos — não pode virar hábito.",
        "{n} batidas nesta temporada. {valor} dessa. O conselho está perguntando o que acontece no cockpit.",
        "{valor}. Se continuar assim, vamos ter uma conversa séria sobre o seu lugar aqui.",
    ];
    const LIGHT: &[&str] = &[
        "Nada grave — {valor} e o carro tá novo de novo.",
        "Só um susto. {valor} no conserto, seguimos.",
    ];
    const CATASTROPHIC: &[&str] = &[
        "O carro voltou em pedaços. {valor}. Isso vai doer no orçamento o resto da temporada.",
        "Perda quase total. {valor}. Precisamos de um fim de semana limpo, urgente.",
        "{valor} num capote só. O caixa sentiu — e o chefe também.",
    ];

    let pool: &[&str] = if severity == "catastrófico" {
        CATASTROPHIC
    } else if severity == "moderado" && count <= 1 {
        LIGHT
    } else if count <= 1 {
        FIRST
    } else if count <= 3 {
        REPEAT
    } else {
        CHRONIC
    };
    let idx = rng.gen_range(0..pool.len());
    pool[idx]
        .replace("{valor}", valor)
        .replace("{n}", &count.to_string())
}

/// Média das chegadas recentes (do JSON `ultimos_resultados`) — menor = melhor
/// forma. `None` se não houver histórico.
fn recent_avg_finish(v: &serde_json::Value) -> Option<f64> {
    let positions: Vec<f64> = v
        .as_array()?
        .iter()
        .filter_map(|e| e.get("position").and_then(|p| p.as_i64()))
        .filter(|p| *p > 0)
        .map(|p| p as f64)
        .collect();
    if positions.is_empty() {
        None
    } else {
        Some(positions.iter().sum::<f64>() / positions.len() as f64)
    }
}

/// Roda o "cérebro" do pós-corrida (`race_eval`) sobre um `RaceResult` + o banco:
/// monta o mérito de cada piloto (skill + carro + forma recente) e avalia o
/// resultado do JOGADOR (expectativa vs resultado, nota, frases). `None` se não
/// houver jogador no resultado — a tela trata o `None` e nunca quebra.
/// Persiste o PAYLOAD da tela de pós-corrida (resultado + avaliação + telemetria)
/// por corrida, para o jogador reabrir a classificação final depois pela Home.
/// Efêmero vira durável: `career_dir/race_screens/<race_id>.json`. Best-effort.
pub(crate) fn save_race_screen(career_dir: &Path, race_id: &str, payload: &serde_json::Value) {
    let dir = career_dir.join("race_screens");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(s) = serde_json::to_string(payload) {
        let _ = std::fs::write(dir.join(format!("{race_id}.json")), s);
    }
}

/// Reabre a tela salva da corrida da rodada `rodada` na categoria. Resolve
/// rodada→race_id pelo calendário da temporada ATIVA e lê o arquivo. `None` se
/// não houver tela salva (corrida antiga / outra categoria / nunca jogada).
#[tauri::command]
pub fn get_saved_race_screen(
    app: AppHandle,
    career_id: String,
    category: String,
    rodada: i32,
) -> Result<Option<serde_json::Value>, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let career_dir = config.saves_dir().join(&career_id);
    let db_path = career_dir.join("career.db");
    if !db_path.exists() {
        return Ok(None);
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let Some(active_season) = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
    else {
        return Ok(None);
    };
    let entries = calendar_queries::get_calendar(&db.conn, &active_season.id, &category)
        .map_err(|e| format!("Falha ao buscar calendário: {e}"))?;
    let Some(entry) = entries.into_iter().find(|e| e.rodada == rodada) else {
        return Ok(None);
    };
    let path = career_dir.join("race_screens").join(format!("{}.json", entry.id));
    match std::fs::read_to_string(&path) {
        Ok(s) => {
            let mut v = match serde_json::from_str::<serde_json::Value>(&s) {
                Ok(v) => v,
                Err(_) => return Ok(None),
            };
            // Injeta o race_id (não estava no payload salvo) p/ o front reconstruir o clima.
            if let Some(obj) = v.as_object_mut() {
                obj.insert("race_id".into(), serde_json::Value::String(entry.id.clone()));
            }
            Ok(Some(v))
        }
        Err(_) => Ok(None),
    }
}

/// Uma quebra de peça pra UI pós-corrida (Peça 3): já resolvida com o nome do piloto e da peça.
#[derive(serde::Serialize)]
pub struct RaceBreakdownView {
    pub driver_id: String,
    pub driver_name: String,
    /// Chave da peça (`PartType::as_str`).
    pub part: String,
    /// Nome legível da peça na categoria (Motor/Câmbio/Asa dianteira…).
    pub part_name: String,
    pub lap: u32,
    /// "light" | "heavy" | "dnf".
    pub severity: String,
    /// Segundos perdidos no box; `null` = DNF.
    pub penalty_secs: Option<u32>,
    /// Frase do problema concreto (ex.: "motor fundiu por superaquecimento").
    pub label: String,
    pub is_player: bool,
}

/// Quebras de peça de uma corrida, pra tela pós-corrida (resumo no Debrief + detalhe por piloto
/// na Telemetria). Vazio se a corrida não teve quebra (ou é save antigo).
#[tauri::command]
pub fn get_race_breakdowns(
    app: AppHandle,
    career_id: String,
    race_id: String,
) -> Result<Vec<RaceBreakdownView>, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Ok(Vec::new());
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let rows = crate::db::queries::race_breakdowns::get_breakdowns_for_race(&db.conn, &race_id)
        .map_err(|e| format!("Falha ao ler quebras: {e}"))?;
    let categoria = calendar_queries::get_calendar_entry_by_id(&db.conn, &race_id)
        .ok()
        .flatten()
        .map(|e| e.categoria)
        .unwrap_or_default();
    let out = rows
        .into_iter()
        .map(|r| {
            let driver = driver_queries::get_driver(&db.conn, &r.driver_id).ok();
            let (driver_name, is_player) = driver
                .as_ref()
                .map(|d| (d.nome.clone(), d.is_jogador))
                .unwrap_or_else(|| (r.driver_id.clone(), false));
            let part_name = crate::car::PartType::from_str(&r.part)
                .map(|pt| pt.display_name(&categoria).to_string())
                .unwrap_or_else(|| r.part.clone());
            RaceBreakdownView {
                driver_id: r.driver_id,
                driver_name,
                part: r.part,
                part_name,
                lap: r.lap,
                severity: r.severity,
                penalty_secs: r.penalty_secs,
                label: r.label,
                is_player,
            }
        })
        .collect();
    Ok(out)
}

pub(crate) fn compute_race_evaluation(
    conn: &rusqlite::Connection,
    result: &RaceResult,
) -> Option<crate::race_eval::RaceEvaluation> {
    use crate::race_eval::{compute_merit, evaluate, DriverMerit, RaceEvalInput};

    let player = result.race_results.iter().find(|r| r.is_jogador)?;
    let field_size = result.race_results.len() as i32;
    let field: Vec<DriverMerit> = result
        .race_results
        .iter()
        .filter_map(|r| {
            let driver = driver_queries::get_driver(conn, &r.pilot_id).ok()?;
            let car = team_queries::get_team_by_id(conn, &r.team_id)
                .ok()
                .flatten()
                .map(|t| t.car_strength())
                .unwrap_or(0.0);
            let car_norm = car;
            let merit = compute_merit(
                driver.atributos.skill,
                car_norm,
                recent_avg_finish(&driver.ultimos_resultados),
                field_size,
                0, // corridas na pista entram na Fase 2 (afinamento)
            );
            Some(DriverMerit {
                pilot_id: r.pilot_id.clone(),
                merit,
            })
        })
        .collect();
    if field.is_empty() {
        return None;
    }
    Some(evaluate(&RaceEvalInput {
        player_id: player.pilot_id.clone(),
        grid_position: player.grid_position.max(1),
        finish_position: player.finish_position.max(1),
        is_dnf: player.is_dnf,
        incidents: player.incidents_count.max(0),
        field,
    }))
}

/// Importa um `RaceResult` vindo da SESSÃO do iRacing para a carreira. Guarda pela
/// próxima corrida pendente do jogador (mesma pista), filtra carros fantasmas,
/// recalcula classe+pontos e persiste pelo mesmo `persist_race_result_tx` da
/// simulação offline — a carreira reage idêntico a uma corrida simulada.
pub(crate) fn import_iracing_race_result(
    db: &mut Database,
    career_dir: &Path,
    session_track_id: i64,
    player_crash_severity: &str,
    // Direção do impacto no pico (front/rear/side/vertical) — do monitor. Vazia = frontal.
    player_impact_dir: &str,
    mut result: RaceResult,
    // Telemetria REAL do SDK (ritmo/duelo/erro/melhor momento) — vira pano de fundo
    // do boletim de IA. Corrida real do iRacing tem; offline/sem monitor vem vazia.
    telemetry: &crate::iracing_sdk::telemetry_analysis::TelemetryAnalysis,
    // Histórico ao vivo (voltas + batalha) — fonte dos sinais do dossiê de habilidade.
    history: &crate::iracing_sdk::race_monitor::RaceHistory,
    // Estilo de pilotagem do jogador (fatores por peça do monitor) — modula o desgaste só do
    // carro dele. `None`/neutro numa corrida sem estilo capturado.
    player_style: Option<crate::car::driving_style::StyleFactors>,
    // Desfechos de quebra da corrida (Peça 3), já resolvidos para driver_id. Vazio numa corrida
    // sem quebra. Persistidos na `race_breakdowns` (debrief/notícia).
    breakdowns: Vec<crate::db::queries::race_breakdowns::RaceBreakdownRow>,
) -> Result<(ImportedRaceSummary, RaceResult), String> {
    let active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

    let race_entry = get_next_player_race(&db.conn, &active_season)?
        .ok_or_else(|| "O jogador nao possui corrida pendente para importar.".to_string())?;

    // Guarda: a sessão tem que ser da MESMA pista da próxima corrida pendente.
    if race_entry.track_id as i64 != session_track_id {
        return Err(format!(
            "A sessao do iRacing e de outra pista (id {}). A proxima corrida da carreira e em {} (id {}). Importacao cancelada.",
            session_track_id, race_entry.track_name, race_entry.track_id
        ));
    }

    if runs_in_special_phase(&race_entry.categoria) {
        return Err("Importacao de corridas de fase especial ainda nao e suportada.".to_string());
    }

    let category = get_category_config(&race_entry.categoria)
        .ok_or_else(|| "Categoria da corrida nao encontrada.".to_string())?;
    let teams = team_queries::get_teams_by_category(&db.conn, &race_entry.categoria)
        .map_err(|e| format!("Falha ao buscar equipes da categoria: {e}"))?;

    // Filtra carros que NÃO casaram com pilotos reais da carreira (fantasmas /
    // pace car / slots vazios). Persistir um pilot_id inexistente quebraria o
    // apply_race_result_to_database (get_driver falha).
    let valid_ids: HashSet<String> = result
        .race_results
        .iter()
        .filter(|r| driver_queries::get_driver(&db.conn, &r.pilot_id).is_ok())
        .map(|r| r.pilot_id.clone())
        .collect();
    let total_before = result.race_results.len();
    result.race_results.retain(|r| valid_ids.contains(&r.pilot_id));
    result.qualifying_results.retain(|r| valid_ids.contains(&r.pilot_id));
    let dropped = total_before - result.race_results.len();
    if result.race_results.is_empty() {
        return Err("Nenhum piloto da sessao casou com a carreira — nada a importar.".to_string());
    }
    // Zera referências de topo que possam ter caído no filtro.
    for id in [
        &mut result.winner_id,
        &mut result.pole_sitter_id,
        &mut result.fastest_lap_id,
    ] {
        if !valid_ids.contains(id.as_str()) {
            id.clear();
        }
    }
    if result
        .most_positions_gained_id
        .as_ref()
        .is_some_and(|id| !valid_ids.contains(id))
    {
        result.most_positions_gained_id = None;
    }

    // Peça 3 · Camada B (ponte do DNF perdido): se um comando de quebra do JOGADOR não chegou
    // ao sim (fullscreen exclusivo → `chat_send_blocked`), o `!dq` não abandonou o carro dele e
    // o resultado importado o mostra na pista — mas a carreira JÁ comprometeu a quebra (está no
    // breakdown_log com desfecho "dnf"). Fecha a inconsistência carimbando o DNF a partir do LOG
    // (não do `!dq`). Gate no latch: só quando há evidência de que o comando foi bloqueado, pra
    // não fabricar um abandono que de fato não deveria acontecer. Roda ANTES do rescore, então
    // `apply_special_class_scoring` reconcilia posição/pontos (DNF vai pro fim da classe, 0 ponto)
    // e o bloco de quebra abaixo carimba o `dnf_catalog_id`/motivo (o carro agora é `is_dnf`).
    if crate::iracing_sdk::race_monitor::chat_send_blocked() {
        let player_break_label: Option<String> = result
            .race_results
            .iter()
            .find(|d| d.is_jogador && !d.is_dnf)
            .and_then(|player| {
                breakdowns
                    .iter()
                    .find(|b| b.severity == "dnf" && b.driver_id == player.pilot_id)
                    .map(|b| b.label.clone())
            });
        if let Some(label) = player_break_label {
            if let Some(player) = result.race_results.iter_mut().find(|d| d.is_jogador) {
                player.is_dnf = true;
                player.classification_status =
                    crate::simulation::race::ClassificationStatus::Dnf;
                player.dnf_reason = Some(label);
            }
            // Mantém a contagem de abandonos coerente (narrativa "corrida mais caótica" etc.).
            result.total_dnfs = result.race_results.iter().filter(|r| r.is_dnf).count() as i32;
        }
    }

    // Recalcula classe + pontos a partir de grid/chegada (single e multiclasse).
    apply_special_class_scoring(&mut result, &teams, category.id == "endurance");
    // Grid desconhecido (0) → evita posições-ganhas negativas absurdas.
    for r in result.race_results.iter_mut() {
        if r.grid_position <= 0 {
            r.grid_position = r.finish_position;
            r.positions_gained = 0;
        }
    }

    // Peça 3 · Camada B: os DNFs de QUEBRA viram DNF MECÂNICO no resultado (antes de persistir).
    // Assim a revista (beat "Abandono"), a desconfiança mecânica da IA e o motor editorial acendem
    // de graça: `dnf_catalog_id` aponta pra uma entry `Mechanical` do catálogo (fonte do join) e a
    // frase do problema vira o `dnf_reason` (a narrativa cita a peça). Só carimba quem o iRacing
    // também marcou como DNF — nunca fabrica um abandono que não aconteceu.
    if breakdowns.iter().any(|b| b.severity == "dnf") {
        let mech_id: Option<String> = db
            .conn
            .query_row(
                "SELECT id FROM incident_catalog WHERE incident_source = 'Mechanical' LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok();
        for b in breakdowns.iter().filter(|b| b.severity == "dnf") {
            if let Some(dr) = result
                .race_results
                .iter_mut()
                .find(|d| d.pilot_id == b.driver_id && d.is_dnf)
            {
                if let Some(id) = &mech_id {
                    dr.dnf_catalog_id = Some(id.clone());
                }
                dr.dnf_reason = Some(b.label.clone());
            }
        }
    }

    // Enduro: paradas REAIS do jogador (pneu/combustível) → alívio de gasto de peça (10%/parada,
    // teto 30%, só em corrida >40 min, aplicado no cérebro). Conta só paradas de SERVIÇO genuínas
    // (dwell ≥ limiar) do carro do jogador — não passagem no pit lane. A IA modela pela duração.
    const GENUINE_PIT_MIN_SECS: f64 = 4.0;
    let player_pits = history
        .pit_stops
        .iter()
        .filter(|s| s.car_idx == history.player_car_idx && s.stationary_secs >= GENUINE_PIT_MIN_SECS)
        .count() as u32;

    // Feedback físico da quebra (§4.6): agrupa as peças que largaram por TIME (resolve driver→time
    // pelo resultado). No persist, Leve → segue; Grave → fim de vida; DNF → destruída (troca
    // forçada a débito). É o que recopla a quebra ao vivo com o estado persistido do carro.
    let team_breakdowns: std::collections::HashMap<
        String,
        Vec<(crate::car::PartType, crate::car::breakdown::Severity)>,
    > = {
        use std::collections::HashMap;
        let driver_team: HashMap<&str, &str> = result
            .race_results
            .iter()
            .map(|r| (r.pilot_id.as_str(), r.team_id.as_str()))
            .collect();
        let mut map: HashMap<String, Vec<(crate::car::PartType, crate::car::breakdown::Severity)>> =
            HashMap::new();
        for row in &breakdowns {
            let (Some(pt), Some(sev), Some(team)) = (
                crate::car::PartType::from_str(&row.part),
                crate::car::breakdown::Severity::from_key(&row.severity),
                driver_team.get(row.driver_id.as_str()),
            ) else {
                continue;
            };
            map.entry(team.to_string()).or_default().push((pt, sev));
        }
        map
    };

    let next_round =
        Some((active_season.rodada_atual + 1).min(category.corridas_por_temporada as i32));
    let mut rng = rand::thread_rng();
    let import_injuries = persist_race_result_tx(
        db,
        &race_entry,
        &result,
        &teams,
        &active_season,
        category,
        next_round,
        RacePersistenceMode::Playable,
        player_style,
        player_pits,
        &team_breakdowns,
        &mut rng,
    )?;

    // Peça 3: grava os desfechos de quebra da corrida (debrief/notícia). Best-effort — uma
    // falha aqui não desfaz o resultado já persistido.
    warn_if_side_effect_fails(
        crate::db::queries::race_breakdowns::insert_breakdowns_batch(
            &db.conn,
            &race_entry.id,
            &breakdowns,
        )
        .map_err(|e| e.to_string()),
        "Falha ao gravar race_breakdowns do import",
    );

    // Fama do resultado IMPORTADO do iRacing: MESMA lógica da corrida simulada — o
    // astro nasce igual correndo na pista. Vitória/pódio sobem fama (modulada por
    // carisma), incidente/remontada movem carisma, o grid decai. Best-effort: uma
    // falha aqui não desfaz o resultado já persistido.
    let _ = apply_post_race_fame(&db.conn, &race_entry, &result, &import_injuries);

    // Grava também o histórico por rodada (race_results.json) — é a fonte da grade
    // R1..R5 da classificação (build_driver_histories). O caminho offline faz o
    // mesmo via append_race_result; sem isso, os pontos entram mas a coluna da
    // rodada fica vazia.
    warn_if_side_effect_fails(
        append_race_result(
            career_dir,
            &race_entry.categoria,
            race_entry.rodada,
            &result.race_results,
        ),
        "Falha ao gravar race_results.json do import",
    );

    // ── Telemetria do jogador → dossiê de habilidade (Fase 2) ────────────────
    // Extrai os sinais compactos (consistência, briga, largada) do histórico ao
    // vivo e grava uma linha por corrida. Best-effort: só existe quando o jogador
    // DIRIGIU no iRacing e foi monitorado; falha aqui não desfaz o resultado.
    if let Some(dossier_row) =
        crate::iracing_sdk::telemetry_analysis::extract_player_race_telemetry(history, telemetry)
    {
        warn_if_side_effect_fails(
            crate::db::queries::race_history::upsert_player_race_telemetry(
                &db.conn,
                &race_entry.id,
                &dossier_row,
            )
            .map_err(|e| e.to_string()),
            "Falha ao gravar telemetria do jogador (dossiê de habilidade)",
        );
    }

    // ── Boletim de IA ────────────────────────────────────────────────────────
    // Gera a notícia de Corrida + guarda os fatos do boletim (igual ao fluxo
    // in-app `simulate_race_weekend_in_base_dir`). Sem isto, corridas IMPORTADAS do
    // iRacing não produziam boletim algum. Importância/tier neutros; sem lesões
    // (o import ainda não as computa). Prewarm em background para o boletim já
    // estar em cache quando o jogador abrir Notícias.
    {
        let flat_incidents: Vec<IncidentResult> = result
            .race_results
            .iter()
            .flat_map(|r| r.incidents.clone())
            .collect();
        // Telemetria REAL → fatos de cor sobre a corrida do jogador (ritmo, duelo,
        // erro mais caro, melhor momento). Só corrida real do iRacing tem isto.
        let player_name = result
            .race_results
            .iter()
            .find(|r| r.is_jogador)
            .map(|r| r.pilot_name.clone())
            .unwrap_or_default();
        let telemetry_facts = telemetry_context_facts(telemetry, &player_name);
        match persist_race_news(
            &db.conn,
            &result,
            &active_season,
            race_entry.rodada,
            &race_entry.categoria,
            0,
            race_entry.thematic_slot,
            &InterestTier::Baixo,
            &flat_incidents,
            &[],
            &telemetry_facts,
        ) {
            Ok(Some(news_id)) => {
                if let Some(base_dir) = career_dir.parent().and_then(|p| p.parent()) {
                    let mut cfg = AppConfig::load_or_default(base_dir);
                    let install_id = cfg.get_or_create_install_id();
                    let lang = cfg.language.clone();
                    spawn_prewarm_boletim(
                        career_dir.join("career.db"),
                        news_id,
                        lang,
                        install_id,
                    );
                }
            }
            Ok(None) => {}
            Err(e) => eprintln!("[narrative] Falha ao gerar boletim do import iRacing: {e}"),
        }
    }

    // Simula as OUTRAS categorias (IA) até a semana desta corrida — igual ao fluxo
    // in-app (`simulate_race_weekend_in_base_dir`). Sem isto, as corridas das demais
    // categorias acumulavam pendentes e a temporada não fechava (`advance_season`
    // rejeita corridas pendentes). Quando esta é a ÚLTIMA corrida do jogador na
    // temporada, simula todas as restantes até o fim, deixando o calendário pronto
    // para avançar.
    simulate_other_categories(
        db,
        career_dir,
        &race_entry.categoria,
        calendar_queries::calendar_entry_season_week(&race_entry),
        &race_entry.display_date,
        &active_season.id,
        active_season.numero,
    )?;

    let player = result.race_results.iter().find(|r| r.is_jogador);
    let winner_name = result
        .race_results
        .iter()
        .find(|r| !r.is_dnf && r.finish_position == 1)
        .map(|r| r.pilot_name.clone());

    // ── Conserto do carro (SÓ jogador) ──────────────────────────────────────
    // A batida do jogador (severidade já rebaixada se cruzou a linha) vira um
    // custo proporcional à categoria e ao carro, debitado do caixa da equipe.
    let mut repair_cost = 0.0;
    let mut repair_count = 0;
    let mut repair_message = String::new();
    let player_team_id = player.map(|r| r.team_id.clone()).unwrap_or_default();
    // Dano por PEÇA da batida (car::crash): amassa/destrói peças conforme a DIREÇÃO do
    // impacto e a CONDIÇÃO de cada uma; o custo vem das peças (não mais flat). O carro
    // danificado PERSISTE — e como `persist_race_result_tx` (que já rodou acima) mantém o
    // carro antes, aplicamos o dano por CIMA; o cérebro de manutenção responde na PRÓXIMA
    // corrida (trocar/degradar conforme o caixa). Só há dano se houve batida (≠ "nenhum").
    if !player_team_id.is_empty() && !player_crash_severity.eq_ignore_ascii_case("nenhum") {
        if let Ok(Some(mut team)) = team_queries::get_team_by_id(&db.conn, &player_team_id) {
            use crate::car::crash::{apply_crash_damage, CrashSeverity, ImpactDirection};
            use crate::db::queries::team_car;
            let severity = CrashSeverity::from_label(player_crash_severity);
            let direction = ImpactDirection::from_str(player_impact_dir);
            let mut car = team_car::get_team_car(&db.conn, &player_team_id)
                .ok()
                .flatten()
                .unwrap_or_else(|| crate::car::seed::seed_car(&team.categoria, 0.5));
            let damage = apply_crash_damage(&mut car, &team.categoria, severity, direction);
            let _ = team_car::upsert_team_car(&db.conn, &player_team_id, &car);
            let cost = damage.cost.round();
            if cost > 0.0 {
                team.cash_balance -= cost;
                team.last_round_expenses += cost;
                let _ = team_queries::update_team(&db.conn, &team);
                repair_cost = cost;
                repair_count = bump_repair_count(career_dir, active_season.numero);
                repair_message = pick_repair_message(
                    &mut rng,
                    player_crash_severity,
                    &format_brl(cost),
                    repair_count,
                );
            }
        }
    }

    // Fatura de manutenção do carro (sempre presente; soma o conserto se houve batida).
    let maintenance = compute_maintenance_breakdown(
        &race_entry.categoria,
        player.map(|r| r.final_tire_wear).unwrap_or(0.0),
        player.map(|r| r.laps_completed).unwrap_or(0),
        repair_cost,
        player_crash_severity,
    );

    let summary = ImportedRaceSummary {
        race_id: race_entry.id.clone(),
        track_name: race_entry.track_name.clone(),
        categoria: race_entry.categoria.clone(),
        rodada: race_entry.rodada,
        saved_drivers: result.race_results.len(),
        dropped_unmatched: dropped,
        player_position: player.map(|r| r.finish_position),
        player_points: player.map(|r| r.points_earned),
        player_is_dnf: player.map(|r| r.is_dnf).unwrap_or(false),
        winner_name,
        repair_cost,
        repair_severity: player_crash_severity.to_string(),
        repair_count,
        repair_message,
        maintenance,
    };
    Ok((summary, result))
}

fn simulate_other_categories(
    db: &mut Database,
    career_dir: &Path,
    player_category: &str,
    target_week: i32,
    target_display_date: &str,
    season_id: &str,
    season_number: i32,
) -> Result<SimultaneousResults, String> {
    let mut categories_simulated = Vec::new();
    let mut total_races_simulated = 0;
    let mut highlights = Vec::new();
    let is_last_player_race = calendar_queries::get_next_race(&db.conn, season_id, player_category)
        .map_err(|e| {
            format!(
                "Falha ao verificar proximas corridas restantes de {}: {e}",
                player_category
            )
        })?
        .is_none();

    for category in get_all_categories() {
        if category.id == player_category {
            continue;
        }

        // Busca corridas pendentes com season_week <= target_week, em ordem cronológica.
        // Categorias especiais são excluídas naturalmente durante o BlocoRegular
        // até a abertura da janela especial de setembro, e incluídas no BlocoEspecial.
        let races_to_simulate = if is_last_player_race {
            calendar_queries::get_pending_races_up_to_week(
                &db.conn,
                season_id,
                category.id,
                i32::MAX,
            )
            .map_err(|e| {
                format!(
                    "Falha ao buscar todas as corridas pendentes de {}: {e}",
                    category.id
                )
            })?
        } else {
            calendar_queries::get_pending_races_up_to_week(
                &db.conn,
                season_id,
                category.id,
                target_week,
            )
            .map_err(|e| format!("Falha ao buscar corridas pendentes de {}: {e}", category.id))?
        };

        if races_to_simulate.is_empty() {
            continue;
        }

        let mut summaries = Vec::new();
        for entry in races_to_simulate {
            let (result, ai_injuries) = simulate_category_race(db, &entry, false)?;
            // Fama do MUNDO: astros da IA nascem nas outras categorias também. Usa o
            // interesse "de local" (sem jogador). Best-effort — não quebra o sim.
            let _ = apply_post_race_fame(&db.conn, &entry, &result, &ai_injuries);
            warn_if_side_effect_fails(
                append_race_result(
                    career_dir,
                    &entry.categoria,
                    entry.rodada,
                    &result.race_results,
                ),
                "Falha ao gravar race_results.json de outra categoria",
            );
            warn_if_side_effect_fails(
                track_history_queries::record_race_dnfs(
                    &db.conn,
                    &result.race_results,
                    &entry.track_name,
                    season_number,
                    entry.rodada,
                )
                .map_err(|e| {
                    format!("Falha ao registrar historico de DNF de outra categoria: {e}")
                }),
                "Falha ao registrar historico de DNF de outra categoria",
            );

            let is_visible_to_player = if is_last_player_race {
                !target_display_date.trim().is_empty() && entry.display_date == target_display_date
            } else {
                true
            };
            if !is_visible_to_player {
                continue;
            }

            let winner = result
                .race_results
                .iter()
                .find(|driver| driver.finish_position == 1);
            summaries.push(BriefRaceResult {
                race_id: entry.id.clone(),
                track_name: entry.track_name.clone(),
                winner_name: winner
                    .map(|driver| driver.pilot_name.clone())
                    .unwrap_or_default(),
                winner_team: winner
                    .map(|driver| driver.team_name.clone())
                    .unwrap_or_default(),
            });
            total_races_simulated += 1;
        }

        if summaries.is_empty() {
            continue;
        }

        if let Some(last) = summaries.last() {
            highlights.push(SimHighlight {
                headline: format!(
                    "{} vence em {} ({})",
                    last.winner_name, last.track_name, category.nome_curto
                ),
                category: category.id.to_string(),
            });
        }

        let races_simulated = summaries.len() as i32;
        categories_simulated.push(CategorySimResult {
            category_id: category.id.to_string(),
            category_name: category.nome.to_string(),
            races_simulated,
            results: summaries,
        });
    }

    warn_if_side_effect_fails(
        persist_other_category_news(&db.conn, &highlights, season_number),
        "Falha ao persistir noticias de outras categorias",
    );

    Ok(SimultaneousResults {
        categories_simulated,
        total_races_simulated,
        highlights,
    })
}

/// Penalidade de skill (pontos) por não conhecer a pista, a partir do histórico do
/// piloto. Mesma lógica usada no export pro iRacing (fonte única).
fn track_penalty_for(driver: &Driver, track_id: u32, season: i32) -> f64 {
    use crate::simulation::track_knowledge;
    let knowledge = track_knowledge::from_history(&driver.historico_circuitos, track_id as i64);
    let length = crate::constants::tracks::get_track(track_id)
        .map(|t| t.comprimento_km)
        .unwrap_or(3.0);
    track_knowledge::track_knowledge_penalty(
        &knowledge,
        length,
        driver.atributos.adaptabilidade,
        season,
    )
}

#[allow(clippy::too_many_arguments)]
/// Fator de cautela mecânica por nº de DNFs mecânicos recentes do time (< 1.0 = poupa mais).
/// 1 DNF → 0.90 · 2 → 0.83 · 3+ → 0.78. Assimétrico com a economia do jogador (piso 0.75).
fn mechanical_caution_factor(mechanical_dnfs: u32) -> f64 {
    match mechanical_dnfs {
        0 => 1.0,
        1 => 0.90,
        2 => 0.83,
        _ => 0.78,
    }
}

fn apply_race_result_to_database(
    tx: &rusqlite::Transaction<'_>,
    result: &RaceResult,
    teams: &[Team],
    economic_health: GlobalEconomicHealth,
    race_category: &str,
    persistence_mode: RacePersistenceMode,
    track_id: u32,
    season_number: i32,
    round: i32,
    upcoming_track_ids: &[u32],
    wear_conditions: crate::market::car_maintenance::WearConditions,
    // Bilheteria (Fase 3 do Estrelato): prestígio "de local" do evento (pré-vendido),
    // usado pra dimensionar o bolo de público da rodada.
    event_prestige_score: f64,
    // Estilo de pilotagem do JOGADOR (só quando ele correu no iRacing) + o time dele — o
    // estilo modula o desgaste SÓ do carro do jogador. `None` = corrida sem estilo capturado.
    player_team_id: Option<&str>,
    player_style: Option<crate::car::driving_style::StyleFactors>,
    // Nº de paradas REAIS do jogador (SDK) — alívio de gasto de peça do enduro só no carro dele.
    player_pits: u32,
    // Desconfiança mecânica por time (DNF mecânico recente) → poupa as peças. Vazio = ninguém.
    team_cautions: &std::collections::HashMap<String, crate::car::driving_style::StyleFactors>,
    // Feedback físico da quebra (§4.6): peças que largaram nesta corrida, por time. Leve → segue;
    // Grave → fim de vida; DNF → destruída (troca forçada a débito). Vazio na sim offline.
    team_breakdowns: &std::collections::HashMap<
        String,
        Vec<(crate::car::PartType, crate::car::breakdown::Severity)>,
    >,
) -> Result<(), DbError> {
    for race_driver in &result.race_results {
        let mut driver = driver_queries::get_driver(tx, &race_driver.pilot_id)?;
        let mut season_stats = driver.stats_temporada.clone();
        let mut career_stats = driver.stats_carreira.clone();

        let previous_races = season_stats.corridas;
        season_stats.pontos += race_driver.points_earned as f64;
        season_stats.vitorias += u32::from(!race_driver.is_dnf && race_driver.finish_position == 1);
        season_stats.podios += u32::from(!race_driver.is_dnf && race_driver.finish_position <= 3);
        season_stats.poles += u32::from(race_driver.pilot_id == result.pole_sitter_id);
        season_stats.corridas += 1;
        season_stats.dnfs += u32::from(race_driver.is_dnf);
        season_stats.posicao_media = recalculate_average_position(
            season_stats.posicao_media,
            previous_races,
            race_driver.finish_position,
        );

        career_stats.pontos_total += race_driver.points_earned as f64;
        career_stats.vitorias += u32::from(!race_driver.is_dnf && race_driver.finish_position == 1);
        career_stats.podios += u32::from(!race_driver.is_dnf && race_driver.finish_position <= 3);
        career_stats.poles += u32::from(race_driver.pilot_id == result.pole_sitter_id);
        career_stats.corridas += 1;
        career_stats.dnfs += u32::from(race_driver.is_dnf);

        let better_result = driver
            .melhor_resultado_temp
            .map(|current| current.min(race_driver.finish_position as u32))
            .or(Some(race_driver.finish_position as u32));

        driver.stats_temporada = season_stats;
        driver.stats_carreira = career_stats;
        driver.melhor_resultado_temp = better_result;
        driver.corridas_na_categoria += 1;
        // Conhecimento de pista: registra a corrida no cache (largadas, melhor
        // resultado, temporada) — alimenta a penalidade de pista nova (sim + export).
        let track_finish = if race_driver.is_dnf {
            None
        } else {
            Some(race_driver.finish_position as u32)
        };
        crate::simulation::track_knowledge::record_race(
            &mut driver.historico_circuitos,
            track_id as i64,
            track_finish,
            season_number,
        );
        driver.ultimos_resultados = append_recent_result(
            &driver.ultimos_resultados,
            race_driver.finish_position,
            race_driver.is_dnf,
        );

        driver_queries::update_driver(tx, &driver)?;
    }

    if runs_in_special_phase(race_category) {
        return Ok(());
    }

    let active_contracts = if persistence_mode == RacePersistenceMode::Playable {
        Some(contract_queries::get_all_active_regular_contracts(tx)?)
    } else {
        None
    };
    let race_results_by_team = group_results_by_team(result);
    let category_id = teams
        .first()
        .map(|team| team.categoria.as_str())
        .unwrap_or("");
    let rounds_in_season = get_category_config(category_id)
        .map(|config| f64::from(config.corridas_por_temporada.max(1)))
        .unwrap_or(1.0);
    // Bilheteria (Fase 3 do Estrelato): presença pública (fama do lineup) de cada time do
    // grid, pré-computada 1× por rodada. A soma é o denominador da cota competitiva de
    // bilheteria; o loop abaixo reusa a presença por time (evita 2ª consulta ao lineup).
    let team_presences: std::collections::HashMap<String, f64> = teams
        .iter()
        .map(|team| {
            let medias = team_queries::get_team_lineup_medias(tx, &team.id).unwrap_or_default();
            let presence =
                crate::public_presence::team::derive_team_public_presence(&medias).raw_score;
            (team.id.clone(), presence)
        })
        .collect();
    let grid_total_presence: f64 = team_presences.values().sum();
    let grid_team_count = teams.len().max(1) as f64;
    for team in teams {
        let Some(team_results) = race_results_by_team.get(&team.id) else {
            continue;
        };

        let added_points: i32 = team_results.iter().map(|entry| entry.points_earned).sum();
        let added_victories: i32 = team_results
            .iter()
            .filter(|entry| entry.finish_position == 1)
            .count() as i32;
        let added_podiums: i32 = team_results
            .iter()
            .filter(|entry| entry.finish_position <= 3)
            .count() as i32;
        let added_poles: i32 = i32::from(
            team_results
                .iter()
                .any(|entry| entry.pilot_id == result.pole_sitter_id),
        );
        let best_result = team_results
            .iter()
            .map(|entry| entry.finish_position)
            .min()
            .unwrap_or(99);
        let current_best = if team.stats_melhor_resultado <= 0 {
            99
        } else {
            team.stats_melhor_resultado
        };

        team_queries::update_team_season_stats(
            tx,
            &team.id,
            team.stats_vitorias + added_victories,
            team.stats_podios + added_podiums,
            team.stats_poles + added_poles,
            team.stats_pontos + added_points,
            current_best.min(best_result),
        )?;

        let Some(active_contracts) = active_contracts.as_ref() else {
            continue;
        };

        let team_salary_total: f64 = active_contracts
            .iter()
            .filter(|contract| contract.equipe_id == team.id)
            .map(|contract| contract.salario_anual)
            .sum();
        let salary_expense = team_salary_total / rounds_in_season;
        // Fama de equipe → patrocínio + bilheteria: presença pública do lineup (fama dos
        // pilotos). É o motor da "2ª moeda" — um rosto famoso capta patrocínio pra construir
        // o carro E puxa público pro portão. Reusa o pré-cômputo do grid.
        let lineup_public_presence = team_presences.get(&team.id).copied().unwrap_or(0.0);
        // Sistema de Nível do Carro: o cérebro decide a manutenção, aplica o desgaste e
        // persiste o carro; o custo decidido vira depreciação REAL na fatura abaixo.
        // O estilo de pilotagem só incide no carro do JOGADOR (o time dele nesta corrida). Os
        // demais times podem POUPAR as peças por desconfiança mecânica (DNF mecânico recente).
        let is_player_car = player_team_id == Some(team.id.as_str());
        let team_style = if is_player_car {
            player_style
        } else {
            team_cautions.get(&team.id).copied()
        };
        // Carro do JOGADOR usa o pit REAL do SDK pro alívio de enduro; a IA modela pela duração.
        // Feedback físico da quebra (§4.6): peças DESTE time que largaram nesta corrida.
        let this_team_breakdowns: &[(crate::car::PartType, crate::car::breakdown::Severity)] =
            team_breakdowns.get(team.id.as_str()).map(|v| v.as_slice()).unwrap_or(&[]);
        let car_maintenance_cost = crate::market::car_maintenance::maintain_team_car_pits(
            tx,
            team,
            category_id,
            season_number,
            upcoming_track_ids,
            wear_conditions,
            team_style,
            is_player_car,
            player_pits,
            this_team_breakdowns,
        )?;

        let finance_context = calculate_team_round_finance_context(
            team,
            lineup_public_presence,
            added_points,
            added_victories,
            added_podiums,
            best_result,
            salary_expense,
            rounds_in_season,
            economic_health,
            car_maintenance_cost,
            event_prestige_score,
            grid_total_presence,
            grid_team_count,
        );

        let mut updated_team = team.clone();
        let cashflow_summary = apply_round_cashflow(&mut updated_team, finance_context);
        apply_crisis_event_if_needed(&mut updated_team);
        refresh_team_financial_state(&mut updated_team);
        team_queries::update_team_finance_snapshot(tx, &updated_team)?;
        // Grava a divisão REAL da rodada (as 9 linhas) no histórico — fonte única do
        // dossiê financeiro da aba My Team, no lugar dos números fabricados no front.
        team_queries::insert_team_finance_history(
            tx,
            &updated_team,
            &finance_context,
            &cashflow_summary,
            season_number,
            round,
        )?;
    }

    Ok(())
}

fn append_recent_result(
    existing: &serde_json::Value,
    finish_position: i32,
    is_dnf: bool,
) -> serde_json::Value {
    let mut results: Vec<serde_json::Value> = existing
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.as_object().cloned())
        .map(serde_json::Value::Object)
        .collect();

    results.push(serde_json::json!({
        "position": finish_position,
        "is_dnf": is_dnf,
        "has_fastest_lap": false,
        "grid_position": 0,
        "positions_gained": 0
    }));

    if results.len() > 5 {
        let keep_from = results.len() - 5;
        results.drain(0..keep_from);
    }

    serde_json::Value::Array(results)
}

fn build_team_lookup(
    teams: &[crate::models::team::Team],
) -> HashMap<String, &crate::models::team::Team> {
    let mut lookup = HashMap::new();
    for team in teams {
        if let Some(driver_id) = &team.piloto_1_id {
            lookup.insert(driver_id.clone(), team);
        }
        if let Some(driver_id) = &team.piloto_2_id {
            lookup.insert(driver_id.clone(), team);
        }
    }
    lookup
}

fn uses_regular_special_event_grid(category: &str) -> bool {
    matches!(category, "production_challenger" | "endurance")
}

fn get_regular_special_event_teams(
    conn: &rusqlite::Connection,
    category: &str,
) -> Result<Vec<crate::models::team::Team>, DbError> {
    team_queries::get_teams_by_category(conn, category)
}

fn get_regular_special_event_contracts(
    conn: &rusqlite::Connection,
    category: &str,
    grid_teams: &[crate::models::team::Team],
) -> Result<Vec<crate::models::contract::Contract>, String> {
    let active_contracts = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao buscar contratos regulares ativos: {e}"))?;
    let active_contracts = filter_regular_special_event_contracts(active_contracts, category);
    if !active_contracts.is_empty() {
        return Ok(active_contracts);
    }

    // Safety fallback for old saves/history imports that predate active regular
    // contracts in these real special-phase divisions. Normal new saves should
    // return through the active-contract path above.
    let mut fallback_contracts = Vec::new();
    fallback_contracts.extend(
        contract_queries::get_contracts_by_category(conn, category)
            .map_err(|e| format!("Falha ao buscar historico regular de contratos: {e}"))?,
    );

    // O histórico inclui contratos rescindidos; após promoção/rebaixamento parte
    // dessas equipes já saiu da categoria e não pertence mais ao grid.
    let grid_team_ids: std::collections::HashSet<&str> =
        grid_teams.iter().map(|team| team.id.as_str()).collect();
    fallback_contracts.retain(|contract| grid_team_ids.contains(contract.equipe_id.as_str()));

    Ok(filter_regular_special_event_contracts(
        fallback_contracts,
        category,
    ))
}

fn filter_regular_special_event_contracts(
    contracts: Vec<crate::models::contract::Contract>,
    category: &str,
) -> Vec<crate::models::contract::Contract> {
    contracts
        .into_iter()
        .filter(|contract| match category {
            "production_challenger" => contract.categoria == "production_challenger",
            "endurance" => contract.categoria == "endurance",
            _ => contract.categoria == category,
        })
        .filter(|contract| contract.tipo == crate::models::enums::ContractType::Regular)
        .collect()
}

fn get_drivers_for_contracts(
    conn: &rusqlite::Connection,
    contracts: &[crate::models::contract::Contract],
) -> Result<Vec<Driver>, String> {
    let mut drivers = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for contract in contracts {
        if !seen.insert(contract.piloto_id.clone()) {
            continue;
        }
        let driver = driver_queries::get_driver(conn, &contract.piloto_id).map_err(|e| {
            format!(
                "Falha ao buscar piloto contratado '{}': {e}",
                contract.piloto_id
            )
        })?;
        drivers.push(driver);
    }

    Ok(drivers)
}

fn get_drivers_for_team_lineups(
    conn: &rusqlite::Connection,
    teams: &[crate::models::team::Team],
) -> Result<Vec<Driver>, String> {
    let mut drivers = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for team in teams {
        for pilot_id in [team.piloto_1_id.as_ref(), team.piloto_2_id.as_ref()]
            .into_iter()
            .flatten()
        {
            if !seen.insert(pilot_id.clone()) {
                continue;
            }
            let driver = driver_queries::get_driver(conn, pilot_id)
                .map_err(|e| format!("Falha ao buscar piloto do lineup '{}': {e}", pilot_id))?;
            drivers.push(driver);
        }
    }

    Ok(drivers)
}

fn build_regular_contract_team_lookup<'a>(
    contracts: &[crate::models::contract::Contract],
    teams: &'a [crate::models::team::Team],
) -> HashMap<String, &'a crate::models::team::Team> {
    let teams_by_id: HashMap<&str, &crate::models::team::Team> =
        teams.iter().map(|team| (team.id.as_str(), team)).collect();
    let mut lookup = HashMap::new();

    for contract in contracts {
        if let Some(team) = teams_by_id.get(contract.equipe_id.as_str()) {
            lookup.insert(contract.piloto_id.clone(), *team);
        }
    }

    lookup
}

fn build_special_team_lookup<'a>(
    conn: &rusqlite::Connection,
    teams: &'a [crate::models::team::Team],
    category: &str,
) -> Result<HashMap<String, &'a crate::models::team::Team>, String> {
    let teams_by_id: HashMap<&str, &crate::models::team::Team> =
        teams.iter().map(|team| (team.id.as_str(), team)).collect();
    let contracts = contract_queries::get_active_especial_contracts_by_category(conn, category)
        .map_err(|e| format!("Falha ao buscar contratos especiais ativos: {e}"))?;
    let mut lookup = HashMap::new();

    for contract in contracts {
        if let Some(team) = teams_by_id.get(contract.equipe_id.as_str()) {
            lookup.insert(contract.piloto_id, *team);
        }
    }

    Ok(lookup)
}

fn apply_special_class_scoring(
    result: &mut RaceResult,
    teams: &[crate::models::team::Team],
    is_endurance: bool,
) {
    let class_by_team: HashMap<&str, &str> = teams
        .iter()
        .map(|team| {
            (
                team.id.as_str(),
                team.classe.as_deref().unwrap_or(team.categoria.as_str()),
            )
        })
        .collect();
    let mut result_indexes_by_class: HashMap<String, Vec<usize>> = HashMap::new();

    for (index, entry) in result.race_results.iter().enumerate() {
        let class_name = class_by_team
            .get(entry.team_id.as_str())
            .copied()
            .unwrap_or("geral");
        result_indexes_by_class
            .entry(class_name.to_string())
            .or_default()
            .push(index);
    }

    let fastest_lap_id = result.fastest_lap_id.clone();
    for indexes in result_indexes_by_class.values_mut() {
        indexes.sort_by(|left, right| {
            let left_result = &result.race_results[*left];
            let right_result = &result.race_results[*right];
            left_result
                .is_dnf
                .cmp(&right_result.is_dnf)
                .then_with(|| {
                    left_result
                        .finish_position
                        .cmp(&right_result.finish_position)
                })
                .then_with(|| left_result.pilot_name.cmp(&right_result.pilot_name))
        });

        for (class_index, result_index) in indexes.iter().enumerate() {
            let class_position = class_index as i32 + 1;
            let entry = &mut result.race_results[*result_index];
            entry.finish_position = class_position;
            entry.positions_gained = entry.grid_position - class_position;
            entry.points_earned = if entry.is_dnf {
                0
            } else {
                get_points_for_position(class_position as u8, is_endurance) as i32
            };
            if !entry.is_dnf && entry.pilot_id == fastest_lap_id && class_position <= 10 {
                entry.points_earned += BONUS_FASTEST_LAP as i32;
            }
        }
    }
}

fn group_results_by_team(
    result: &RaceResult,
) -> HashMap<String, Vec<&crate::simulation::race::RaceDriverResult>> {
    let mut grouped: HashMap<String, Vec<&crate::simulation::race::RaceDriverResult>> =
        HashMap::new();
    for driver_result in &result.race_results {
        grouped
            .entry(driver_result.team_id.clone())
            .or_default()
            .push(driver_result);
    }
    grouped
}

fn recalculate_average_position(
    current_average: f64,
    previous_races: u32,
    finish_position: i32,
) -> f64 {
    let total = current_average * previous_races as f64 + finish_position as f64;
    total / (previous_races as f64 + 1.0)
}

fn update_last_played(meta_path: &Path) -> Result<(), String> {
    let content =
        std::fs::read_to_string(meta_path).map_err(|e| format!("Falha ao ler meta.json: {e}"))?;
    let mut meta: SaveMeta =
        serde_json::from_str(&content).map_err(|e| format!("Falha ao parsear meta.json: {e}"))?;
    meta.last_played = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let json = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Falha ao serializar meta.json: {e}"))?;
    std::fs::write(meta_path, json).map_err(|e| format!("Falha ao gravar meta.json: {e}"))
}

fn warn_if_side_effect_fails<T>(result: Result<T, String>, context: &str) {
    if let Err(error) = result {
        eprintln!("Aviso: {context}: {error}");
    }
}

fn get_player_active_category(
    conn: &rusqlite::Connection,
    active_season: &Season,
) -> Result<Option<String>, String> {
    let player = driver_queries::get_player_driver(conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;

    if active_season.fase.is_racing() {
        if let Some(contract) =
            contract_queries::get_active_especial_contract_for_pilot(conn, &player.id)
                .map_err(|e| format!("Falha ao buscar contrato especial ativo: {e}"))?
        {
            return Ok(Some(contract.categoria));
        }
    }

    if let Some(contract) =
        contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
            .map_err(|e| format!("Falha ao buscar contrato regular ativo: {e}"))?
    {
        return Ok(Some(contract.categoria));
    }

    if active_season.fase.is_racing() {
        if let Some(category) = player.categoria_especial_ativa {
            return Ok(Some(category));
        }
    }

    Ok(player.categoria_atual)
}

pub(crate) fn get_next_player_race(
    conn: &rusqlite::Connection,
    active_season: &Season,
) -> Result<Option<CalendarEntry>, String> {
    let Some(category_id) = get_player_active_category(conn, active_season)? else {
        return Ok(None);
    };

    calendar_queries::get_next_race(conn, &active_season.id, &category_id)
        .map_err(|e| format!("Falha ao buscar proxima corrida do jogador: {e}"))
}

fn race_news_importance(
    bias: i32,
    tier: &InterestTier,
    finish_position: i32,
) -> crate::news::NewsImportance {
    use crate::event_interest::InterestTier;
    use crate::news::NewsImportance;
    let tier_score = match tier {
        InterestTier::Baixo => 0,
        InterestTier::Moderado => 1,
        InterestTier::Alto => 2,
        InterestTier::MuitoAlto => 3,
        InterestTier::EventoPrincipal => 4,
    };
    let position_bonus = if finish_position == 1 {
        2
    } else if finish_position <= 3 {
        1
    } else {
        0
    };
    let total = bias + tier_score + position_bonus;
    let importance = if total >= 5 {
        NewsImportance::Destaque
    } else if total >= 3 {
        NewsImportance::Alta
    } else if total >= 1 {
        NewsImportance::Media
    } else {
        NewsImportance::Baixa
    };
    // Vitória sempre dispara pelo menos Alta para que detect_race_trigger acione LeaderWon/ShockWin/etc.
    if finish_position == 1 && matches!(importance, NewsImportance::Baixa | NewsImportance::Media) {
        NewsImportance::Alta
    } else {
        importance
    }
}

fn persist_race_news(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    active_season: &Season,
    round: i32,
    category_id: &str,
    news_importance_bias: i32,
    _thematic_slot: crate::models::enums::ThematicSlot,
    interest_tier: &InterestTier,
    flat_incidents: &[IncidentResult],
    new_injuries: &[Injury],
    // Fatos extras de pano de fundo (ex.: telemetria REAL do SDK numa corrida
    // importada do iRacing). Vazio no fluxo simulado. Entram na seção "Contexto".
    extra_context_facts: &[String],
) -> Result<Option<String>, String> {
    use crate::db::queries::news as news_queries;
    use crate::generators::ids::{next_id, IdType};
    use crate::news::{NewsImportance, NewsItem, NewsType};

    use crate::db::queries::drivers as driver_queries;

    let now = chrono::Local::now().timestamp();
    let mut items: Vec<NewsItem> = Vec::new();
    // Id da notícia de Corrida do jogador — usado para atrelar os fatos do boletim de IA.
    let mut corrida_news_id: Option<String> = None;

    // 1. Corrida — notícia sobre o VENCEDOR da corrida (não o jogador)
    // O sistema editorial foi projetado para compor histórias sobre quem ganhou.
    // A importância Alta garante que detect_race_trigger gera algo além do FallbackRaceResult.
    {
        let winner_id = &race_result.winner_id;
        let winner_name = driver_queries::get_driver(conn, winner_id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| winner_id.clone());
        let importance = race_news_importance(news_importance_bias, interest_tier, 1);

        let total_rodadas = crate::constants::categories::get_category_config(category_id)
            .map(|c| c.corridas_por_temporada as i32)
            .unwrap_or(round);
        let fallback_races = total_rodadas - round;

        let track = race_result.track_name.as_str();
        let (titulo, texto) = if fallback_races == 0 {
            (
                rust_i18n::t!("race.news.win_final_title", name = winner_name, track = track).to_string(),
                rust_i18n::t!("race.news.win_final_text", name = winner_name, season = active_season.numero).to_string(),
            )
        } else if fallback_races <= 2 {
            (
                rust_i18n::t!("race.news.win_crucial_title", name = winner_name, track = track).to_string(),
                rust_i18n::t!("race.news.win_crucial_text", name = winner_name, round = round).to_string(),
            )
        } else {
            (
                rust_i18n::t!("race.news.win_title", name = winner_name, track = track).to_string(),
                rust_i18n::t!(
                    "race.news.win_text",
                    name = winner_name,
                    round = round,
                    season = active_season.numero
                )
                .to_string(),
            )
        };

        let winner_team = race_result
            .race_results
            .iter()
            .find(|r| &r.pilot_id == winner_id)
            .map(|r| r.team_id.clone());
        let id = next_id(conn, IdType::News).map_err(|e| format!("next_id news: {e:?}"))?;
        corrida_news_id = Some(id.clone());
        items.push(NewsItem {
            id,
            tipo: NewsType::Corrida,
            icone: NewsType::Corrida.icone().to_string(),
            titulo,
            texto,
            rodada: Some(round),
            semana_pretemporada: None,
            temporada: active_season.numero,
            categoria_id: Some(category_id.to_string()),
            categoria_nome: None,
            importancia: importance,
            timestamp: now,
            driver_id: Some(winner_id.clone()),
            driver_id_secondary: None,
            team_id: winner_team.map(Some).unwrap_or(None),
        });

        if fallback_races == 0 {
            if let Ok(standings) = crate::db::queries::race_history::get_category_standings(
                conn,
                &active_season.id,
                category_id,
            ) {
                if let Some(champion) = standings.into_iter().next() {
                    let champ_id =
                        next_id(conn, IdType::News).unwrap_or_else(|_| "news_champ".to_string());
                    items.push(NewsItem {
                        id: champ_id,
                        tipo: NewsType::FramingSazonal,
                        icone: NewsType::FramingSazonal.icone().to_string(),
                        titulo: rust_i18n::t!("race.news.champion_title", name = champion.pilot_name.as_str(), season = active_season.numero).to_string(),
                        texto: rust_i18n::t!("race.news.champion_text", rounds = total_rodadas, name = champion.pilot_name.as_str()).to_string(),
                        rodada: Some(round),
                        semana_pretemporada: None,
                        temporada: active_season.numero,
                        categoria_id: Some(category_id.to_string()),
                        categoria_nome: None,
                        importancia: NewsImportance::Destaque,
                        timestamp: now,
                        driver_id: Some(champion.pilot_id),
                        driver_id_secondary: None,
                        team_id: None,
                    });
                }
            }
        }
    }

    // 2. Incidentes — um item por DNF + incidentes de hint >= 2 não-DNF
    // Evita duplicatas: se um piloto já tem DNF, não gera segundo item por hint >= 2 dele.
    let mut seen_incident_pilots: HashSet<String> = HashSet::new();
    let mut noticiable: Vec<&IncidentResult> = flat_incidents
        .iter()
        .filter(|i| i.is_dnf || i.narrative_importance_hint >= 2)
        .collect();
    // DNFs primeiro, depois por hint decrescente
    noticiable.sort_by_key(|i| {
        (
            std::cmp::Reverse(i.is_dnf as u8),
            std::cmp::Reverse(i.narrative_importance_hint),
        )
    });

    for inc in noticiable {
        if !seen_incident_pilots.insert(inc.pilot_id.clone()) {
            continue; // piloto já tem notícia nesta rodada
        }
        let driver_name = driver_queries::get_driver(conn, &inc.pilot_id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| inc.pilot_id.clone());
        let id = next_id(conn, IdType::News).map_err(|e| format!("next_id incident: {e:?}"))?;
        let titulo = if inc.is_dnf {
            format!("{} abandona a corrida após incidente", driver_name)
        } else {
            format!("{} envolvido em incidente durante a prova", driver_name)
        };
        let texto = inc.description.clone();
        let inc_importance = if inc.narrative_importance_hint >= 3 {
            NewsImportance::Destaque
        } else {
            NewsImportance::Alta
        };
        items.push(NewsItem {
            id,
            tipo: NewsType::Incidente,
            icone: NewsType::Incidente.icone().to_string(),
            titulo,
            texto,
            rodada: Some(round),
            semana_pretemporada: None,
            temporada: active_season.numero,
            categoria_id: Some(category_id.to_string()),
            categoria_nome: None,
            importancia: inc_importance,
            timestamp: now,
            driver_id: Some(inc.pilot_id.clone()),
            driver_id_secondary: inc.linked_pilot_id.clone(),
            team_id: None,
        });
    }

    // 3. Lesão — uma notícia por piloto lesionado
    for injury in new_injuries {
        let driver_name = driver_queries::get_driver(conn, &injury.pilot_id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| injury.pilot_id.clone());
        let id = next_id(conn, IdType::News).map_err(|e| format!("next_id injury: {e:?}"))?;
        let titulo = "desfalque confirmado".to_string();
        let texto = format!(
            "{} está fora da próxima etapa após lesão confirmada. Situação será reavaliada nos próximos dias.",
            driver_name
        );
        items.push(NewsItem {
            id,
            tipo: NewsType::Lesao,
            icone: NewsType::Lesao.icone().to_string(),
            titulo,
            texto,
            rodada: Some(round),
            semana_pretemporada: None,
            temporada: active_season.numero,
            categoria_id: Some(category_id.to_string()),
            categoria_nome: None,
            importancia: NewsImportance::Alta,
            timestamp: now,
            driver_id: Some(injury.pilot_id.clone()),
            driver_id_secondary: None,
            team_id: None,
        });
    }

    if !items.is_empty() {
        news_queries::insert_news_batch(conn, &items)
            .map_err(|e| format!("insert_news_batch: {e:?}"))?;
    }

    // Boletim de IA (teste via simulação): monta os fatos curados da corrida do
    // jogador e os guarda atrelados à notícia de Corrida, para o comando lazy
    // enviá-los ao servidor quando o jogador abrir a notícia. A fonte trocará
    // para os dados reais do SDK quando a integração corrida-real→carreira existir.
    let returned_news_id = corrida_news_id.clone();
    if let Some(news_id) = corrida_news_id {
        let category_name: &str = match crate::constants::categories::get_category_config(category_id)
        {
            Some(c) => c.nome,
            None => category_id,
        };
        // Lesões ocorridas nesta corrida → viram fatos do boletim (nome resolvido).
        let injury_facts: Vec<String> = new_injuries
            .iter()
            .map(|inj| {
                let name = driver_queries::get_driver(conn, &inj.pilot_id)
                    .map(|d| d.nome)
                    .unwrap_or_else(|_| inj.pilot_id.clone());
                rust_i18n::t!("briefing.ctx.injury", name = name.as_str()).to_string()
            })
            .collect();

        // Contexto de carreira (pano de fundo) dos pilotos em DESTAQUE: vencedor,
        // pódio (2º/3º), maior recuperação e o nosso piloto. Atributos do piloto +
        // histórico de pista — sem dependência de ordem de inserção.
        let mut context_facts: Vec<String> = Vec::new();
        let mut featured: Vec<String> = vec![race_result.winner_id.clone()];
        for d in &race_result.race_results {
            if !d.is_dnf && (d.finish_position == 2 || d.finish_position == 3) {
                featured.push(d.pilot_id.clone());
            }
        }
        if let Some(id) = &race_result.most_positions_gained_id {
            featured.push(id.clone());
        }
        if let Some(p) = race_result.race_results.iter().find(|d| d.is_jogador) {
            featured.push(p.pilot_id.clone());
        }
        featured.sort();
        featured.dedup();

        for pilot_id in &featured {
            let driver = match driver_queries::get_driver(conn, pilot_id) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let is_winner = *pilot_id == race_result.winner_id;

            // Rookie em destaque → valoriza a estreia. Veterano → só o vencedor (evita poluir).
            if driver.temporadas_na_categoria == 0 {
                context_facts.push(
                    rust_i18n::t!("briefing.ctx.rookie_debut", name = driver.nome.as_str())
                        .to_string(),
                );
            } else if is_winner && driver.temporadas_na_categoria >= 5 {
                context_facts.push(
                    rust_i18n::t!(
                        "briefing.ctx.veteran",
                        name = driver.nome.as_str(),
                        season = driver.temporadas_na_categoria + 1
                    )
                    .to_string(),
                );
            }

            // Histórico de pista: já abandonou aqui antes? (gosto de superação — só
            // para quem TERMINOU hoje, senão seria o abandono desta própria corrida).
            let dnfd_this_race = race_result
                .race_results
                .iter()
                .any(|d| d.pilot_id == *pilot_id && d.is_dnf);
            if !dnfd_this_race {
                if let Ok(Some(_)) = crate::db::queries::track_history::get_pilot_dnf_at_track(
                    conn,
                    pilot_id,
                    &race_result.track_name,
                ) {
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.overcame_dnf_here",
                            name = driver.nome.as_str()
                        )
                        .to_string(),
                    );
                }
            }
        }

        // Grava os DNFs desta corrida no histórico por pista — SÓ AGORA (depois de
        // ler os abandonos ANTERIORES acima), senão o abandono de hoje contaria como
        // "visita anterior" e a narrativa de superação dispararia errado. Camada
        // narrativa, não factual: erro (ex.: reprocessar a mesma etapa) é silencioso.
        let _ = crate::db::queries::track_history::record_race_dnfs(
            conn,
            &race_result.race_results,
            &race_result.track_name,
            active_season.numero,
            round,
        );

        // --- Recordes e marcos da categoria (todas as temporadas) — peso histórico. ---
        // Os agregados já incluem a corrida atual (persistida antes daqui).
        {
            let winner_id = &race_result.winner_id;
            let winner_name = driver_queries::get_driver(conn, winner_id)
                .map(|d| d.nome)
                .unwrap_or_else(|_| winner_id.clone());
            let records = crate::db::queries::race_history::get_category_records(conn, category_id)
                .ok();

            // Sequência de vitórias do vencedor (feito em destaque).
            if let Ok(streak) = crate::db::queries::race_history::get_win_streak(
                conn,
                winner_id,
                &active_season.id,
                category_id,
            ) {
                if streak >= 3 {
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.win_streak",
                            name = winner_name.as_str(),
                            n = streak
                        )
                        .to_string(),
                    );
                }
            }

            // Carreira do vencedor na categoria (para caça a rival e recorde batido).
            let winner_career = crate::db::queries::race_history::get_driver_category_career(
                conn,
                winner_id,
                category_id,
            )
            .ok();

            // Caça a um rival que AINDA está no grid: vencedor a poucas vitórias de
            // igualar alguém logo acima dele no total histórico da categoria.
            if let Some(wc) = &winner_career {
                if let Ok(actives) =
                    crate::db::queries::race_history::get_active_category_win_counts(
                        conn,
                        category_id,
                    )
                {
                    let target = actives
                        .iter()
                        .filter(|a| {
                            a.pilot_id != *winner_id
                                && a.value > wc.wins
                                && a.value - wc.wins <= 3
                        })
                        .min_by_key(|a| a.value - wc.wins);
                    if let Some(t) = target {
                        let diff = t.value - wc.wins;
                        let plural = if diff == 1 {
                            rust_i18n::t!("briefing.ctx.win_singular")
                        } else {
                            rust_i18n::t!("briefing.ctx.win_plural")
                        };
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.chasing_rival",
                                wins = wc.wins,
                                name = winner_name.as_str(),
                                diff = diff,
                                word = plural,
                                target = t.pilot_name.as_str(),
                                value = t.value
                            )
                            .to_string(),
                        );
                    }
                }
            }

            // Recorde de vitórias da categoria SUPERADO hoje: o vencedor passou a marca
            // anterior (era o 2º+1). Diz há quanto tempo a marca resistia, sem nomear o
            // dono anterior. Só vale se a marca era antiga (>= 2 anos) e não-trivial.
            if let (Some(recs), Some(wc)) = (records.as_ref(), winner_career.as_ref()) {
                let is_new_record = recs
                    .most_wins
                    .as_ref()
                    .map_or(false, |m| m.pilot_id == *winner_id && m.value == wc.wins);
                if is_new_record
                    && wc.wins >= 3
                    && recs.second_most_wins == Some(wc.wins - 1)
                {
                    if let Ok(Some(year)) =
                        crate::db::queries::race_history::first_year_reaching_wins(
                            conn,
                            category_id,
                            wc.wins - 1,
                        )
                    {
                        let dur = active_season.ano - year;
                        if dur >= 2 {
                            context_facts.push(
                                rust_i18n::t!(
                                    "briefing.ctx.new_win_record",
                                    name = winner_name.as_str(),
                                    years = dur
                                )
                                .to_string(),
                            );
                        }
                    }
                }
            }

            // Vencedor é quem mais venceu pela própria equipe na categoria.
            if let Some(cur) = race_result
                .race_results
                .iter()
                .find(|d| d.pilot_id == *winner_id)
            {
                if let Ok(Some(top)) =
                    crate::db::queries::race_history::get_team_top_winner_in_category(
                        conn,
                        &cur.team_id,
                        category_id,
                    )
                {
                    if top.pilot_id == *winner_id && top.value >= 2 {
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.team_top_winner",
                                name = winner_name.as_str(),
                                team = cur.team_name.as_str(),
                                wins = top.value
                            )
                            .to_string(),
                        );
                    }
                }
            }

            // Memória temporal dos recordes (vitórias e pódios): registra QUANDO o
            // recorde all-time da categoria é batido, para notícias de "recorde
            // quebrado" com data e o rodapé do mundo. Condição: o recordista PONTUOU
            // hoje na métrica (então o recorde avançou nesta corrida) e é dono ISOLADO
            // do topo (2º colocado == valor-1). `previous = valor-1` (cada corrida soma
            // no máximo 1). Pisos evitam marcos triviais. Idempotente por valor.
            if let Some(recs) = records.as_ref() {
                let record_milestone = |metric: &str, r: &crate::db::queries::race_history::CategoryRecord| {
                    let _ = crate::db::queries::milestones::insert_milestone(
                        conn,
                        category_id,
                        &crate::db::queries::milestones::RecordMilestone {
                            metric: metric.to_string(),
                            pilot_id: r.pilot_id.clone(),
                            pilot_name: r.pilot_name.clone(),
                            value: r.value,
                            previous_value: Some(r.value - 1),
                            context: String::new(),
                            season_number: active_season.numero,
                            ano: active_season.ano,
                            round,
                        },
                    );
                };
                // Vitórias: o recordista é o vencedor de hoje e abriu a marca.
                if let Some(r) = recs.most_wins.as_ref() {
                    if r.pilot_id == *winner_id
                        && r.value >= 5
                        && recs.second_most_wins == Some(r.value - 1)
                    {
                        record_milestone("wins", r);
                    }
                }
                // Pódios: o recordista subiu ao pódio hoje e abriu a marca.
                if let Some(r) = recs.most_podiums.as_ref() {
                    let podium_today = race_result.race_results.iter().any(|d| {
                        d.pilot_id == r.pilot_id && !d.is_dnf && (1..=3).contains(&d.finish_position)
                    });
                    if podium_today
                        && r.value >= 10
                        && recs.second_most_podiums == Some(r.value - 1)
                    {
                        record_milestone("podiums", r);
                    }
                }
                // Poles: o recordista fez a pole de hoje e abriu a marca.
                if let Some(r) = recs.most_poles.as_ref() {
                    if r.pilot_id == race_result.pole_sitter_id
                        && r.value >= 5
                        && recs.second_most_poles == Some(r.value - 1)
                    {
                        record_milestone("poles", r);
                    }
                }
            }

            // Recordes de VITÓRIA do vencedor de hoje: mais vitórias numa temporada,
            // dono da pista (mais vitórias no circuito) e maior sequência de vitórias.
            {
                use crate::db::queries::{milestones, race_history};
                let win_milestone =
                    |metric: &str, value: i32, previous: Option<i32>, context: String| {
                        let _ = milestones::insert_milestone(
                            conn,
                            category_id,
                            &milestones::RecordMilestone {
                                metric: metric.to_string(),
                                pilot_id: winner_id.clone(),
                                pilot_name: winner_name.clone(),
                                value,
                                previous_value: previous,
                                context,
                                season_number: active_season.numero,
                                ano: active_season.ano,
                                round,
                            },
                        );
                    };

                // (a) Mais vitórias numa única temporada (recorde de `standings`, que só
                // tem temporadas encerradas → a atual não entra e não conta duplicado).
                if let Ok(season_wins) = race_history::get_category_wins_this_season(
                    conn,
                    winner_id,
                    &active_season.id,
                    category_id,
                ) {
                    let prev = race_history::get_category_single_season_win_record(conn, category_id)
                        .ok()
                        .flatten()
                        .map(|r| r.value);
                    if season_wins >= 5 && prev.map_or(true, |p| season_wins > p) {
                        win_milestone("season_wins", season_wins, prev, String::new());
                    }
                }

                // (b) Dono da pista: vencedor virou o maior vencedor isolado do circuito.
                if let Ok(track_wins) = race_history::get_pilot_track_wins(
                    conn,
                    winner_id,
                    category_id,
                    &race_result.track_name,
                ) {
                    let others = race_history::get_track_win_leader_excluding(
                        conn,
                        category_id,
                        &race_result.track_name,
                        winner_id,
                    )
                    .unwrap_or(0);
                    if track_wins >= 3 && track_wins > others {
                        win_milestone(
                            "track_wins",
                            track_wins,
                            Some(track_wins - 1),
                            race_result.track_name.clone(),
                        );
                    }
                }

                // (c) Maior sequência de vitórias (na temporada). O "recorde atual" vive
                // nos próprios marcos → só anuncia quando supera o maior já registrado.
                if let Ok(streak) =
                    race_history::get_win_streak(conn, winner_id, &active_season.id, category_id)
                {
                    let streak = streak as i32;
                    let prev = milestones::get_max_milestone_value(conn, category_id, "win_streak")
                        .ok()
                        .flatten();
                    if streak >= 4 && prev.map_or(true, |p| streak > p) {
                        win_milestone("win_streak", streak, prev, String::new());
                    }
                }

                // (d) Maior VENCEDORA da história — a EQUIPE do vencedor de hoje virou a
                // dona isolada do recorde de vitórias da categoria. Guarda a equipe nos
                // campos pilot_* (o rodapé trata a métrica `team_wins` como time).
                if let Some(w) = race_result
                    .race_results
                    .iter()
                    .find(|d| d.pilot_id == *winner_id)
                {
                    let team_wins =
                        crate::db::queries::teams::get_team_category_wins(conn, &w.team_id, category_id)
                            .unwrap_or(0);
                    let others = crate::db::queries::teams::get_category_team_win_leader_excluding(
                        conn,
                        category_id,
                        &w.team_id,
                    )
                    .unwrap_or(0);
                    if team_wins >= 5 && team_wins > others {
                        let _ = milestones::insert_milestone(
                            conn,
                            category_id,
                            &milestones::RecordMilestone {
                                metric: "team_wins".to_string(),
                                pilot_id: w.team_id.clone(),
                                pilot_name: w.team_name.clone(),
                                value: team_wins,
                                previous_value: Some(team_wins - 1),
                                context: String::new(),
                                season_number: active_season.numero,
                                ano: active_season.ano,
                                round,
                            },
                        );
                    }

                    // (e) Recorde de DOBRADINHAS (1-2): só conta quando HOJE foi uma
                    // dobradinha da equipe do vencedor (ele em 1º + outro carro em 2º).
                    let one_two_today = race_result.race_results.iter().any(|d| {
                        d.team_id == w.team_id && !d.is_dnf && d.finish_position == 2
                    });
                    if one_two_today {
                        let count = crate::db::queries::teams::get_team_category_one_two(
                            conn,
                            &w.team_id,
                            category_id,
                        )
                        .unwrap_or(0);
                        let others = crate::db::queries::teams::get_category_one_two_leader_excluding(
                            conn,
                            category_id,
                            &w.team_id,
                        )
                        .unwrap_or(0);
                        if count >= 3 && count > others {
                            let _ = milestones::insert_milestone(
                                conn,
                                category_id,
                                &milestones::RecordMilestone {
                                    metric: "one_two".to_string(),
                                    pilot_id: w.team_id.clone(),
                                    pilot_name: w.team_name.clone(),
                                    value: count,
                                    previous_value: Some(count - 1),
                                    context: String::new(),
                                    season_number: active_season.numero,
                                    ano: active_season.ano,
                                    round,
                                },
                            );
                        }
                    }
                }
            }

            // Recorde de VOLTA da pista: o tempo de volta não é histórico, então o
            // recorde vive em `track_lap_records` (atualizado a cada corrida). Compara a
            // volta mais rápida de HOJE (em memória) com o recorde guardado; se for mais
            // rápida (ou não houver recorde), atualiza. Só emite MARCO quando SUPERA um
            // recorde existente — o inaugural fica guardado em silêncio.
            if !race_result.fastest_lap_id.is_empty() {
                if let Some(fl) = race_result
                    .race_results
                    .iter()
                    .find(|d| d.pilot_id == race_result.fastest_lap_id && d.best_lap_time_ms > 0.0)
                {
                    let lap_ms = fl.best_lap_time_ms.round() as i32;
                    let prev = crate::db::queries::milestones::get_track_lap_record(
                        conn,
                        category_id,
                        &race_result.track_name,
                    )
                    .ok()
                    .flatten();
                    let is_record = prev.as_ref().map_or(true, |(_, _, pms)| lap_ms < *pms);
                    if is_record {
                        let _ = crate::db::queries::milestones::upsert_track_lap_record(
                            conn,
                            category_id,
                            &race_result.track_name,
                            &fl.pilot_id,
                            &fl.pilot_name,
                            lap_ms,
                            active_season.numero,
                            round,
                        );
                        if let Some((_, _, pms)) = prev {
                            let _ = crate::db::queries::milestones::insert_milestone(
                                conn,
                                category_id,
                                &crate::db::queries::milestones::RecordMilestone {
                                    metric: "lap_record".to_string(),
                                    pilot_id: fl.pilot_id.clone(),
                                    pilot_name: fl.pilot_name.clone(),
                                    value: lap_ms,
                                    previous_value: Some(pms),
                                    context: race_result.track_name.clone(),
                                    season_number: active_season.numero,
                                    ano: active_season.ano,
                                    round,
                                },
                            );
                        }
                    }
                }
            }

            // Maior RECUPERAÇÃO da categoria numa corrida (o sim já aponta quem mais
            // ganhou posições hoje). Compara com o recorde histórico ANTES de hoje; só
            // anuncia quando SUPERA um recorde existente (inaugural fica só no histórico).
            if let Some(gain_id) = race_result.most_positions_gained_id.as_ref() {
                if let Some(cur) = race_result
                    .race_results
                    .iter()
                    .find(|d| d.pilot_id == *gain_id && !d.is_dnf && d.grid_position > 0)
                {
                    let gained = cur.grid_position - cur.finish_position;
                    if gained >= 6 {
                        let prev = crate::db::queries::race_history::get_category_comeback_record(
                            conn,
                            category_id,
                            &active_season.id,
                            round,
                        )
                        .ok()
                        .flatten();
                        if let Some(p) = prev {
                            if gained > p.value {
                                let _ = crate::db::queries::milestones::insert_milestone(
                                    conn,
                                    category_id,
                                    &crate::db::queries::milestones::RecordMilestone {
                                        metric: "comeback".to_string(),
                                        pilot_id: cur.pilot_id.clone(),
                                        pilot_name: cur.pilot_name.clone(),
                                        value: gained,
                                        previous_value: Some(p.value),
                                        context: String::new(),
                                        season_number: active_season.numero,
                                        ano: active_season.ano,
                                        round,
                                    },
                                );
                            }
                        }
                    }
                }
            }

            // Recordes escalares (idade/jejum/caóticos) e "de azar" (coroas cumulativas).
            {
                use crate::db::queries::{milestones, race_history};

                // Emite um marco escalar quando o candidato supera o recorde existente.
                let scalar = |kind: &str,
                              subj_id: &str,
                              subj_name: &str,
                              value: i32,
                              context: &str,
                              higher: bool| {
                    if let Ok(Some(prev)) = milestones::update_scalar_and_check(
                        conn,
                        category_id,
                        kind,
                        subj_id,
                        subj_name,
                        value,
                        context,
                        active_season.numero,
                        round,
                        higher,
                    ) {
                        let _ = milestones::insert_milestone(
                            conn,
                            category_id,
                            &milestones::RecordMilestone {
                                metric: kind.to_string(),
                                pilot_id: subj_id.to_string(),
                                pilot_name: subj_name.to_string(),
                                value,
                                previous_value: Some(prev),
                                context: context.to_string(),
                                season_number: active_season.numero,
                                ano: active_season.ano,
                                round,
                            },
                        );
                    }
                };

                // Piloto mais jovem / mais velho a vencer (idade do vencedor hoje).
                let winner_age = driver_queries::get_driver(conn, winner_id)
                    .map(|d| d.idade as i32)
                    .unwrap_or(0);
                if winner_age > 0 {
                    scalar("youngest_winner", winner_id, &winner_name, winner_age, "", false);
                    scalar("oldest_winner", winner_id, &winner_name, winner_age, "", true);
                }

                // Corrida mais caótica da história (mais abandonos numa etapa).
                if race_result.total_dnfs > 0 {
                    scalar(
                        "most_chaotic_race",
                        &race_result.track_name,
                        &race_result.track_name,
                        race_result.total_dnfs,
                        &race_result.track_name,
                        true,
                    );
                }

                // Maior jejum quebrado: o vencedor voltou a vencer após anos sem ganhar.
                if let Ok(Some(prev_win)) = race_history::get_pilot_previous_win_season(
                    conn,
                    winner_id,
                    category_id,
                    active_season.numero,
                    round,
                ) {
                    let drought = active_season.numero - prev_win;
                    if drought >= 3 {
                        scalar("drought_broken", winner_id, &winner_name, drought, "", true);
                    }
                }

                // "De azar" (coroas que trocam de dono): azarão, batedor, poleiro sem
                // título, maior pontuador. Só anuncia quando o dono muda e passa do piso.
                let crown = |kind: &str, leader: Option<race_history::CategoryRecord>, floor: i32| {
                    if let Some(l) = leader {
                        if let Ok(Some((prev_name, prev_val))) =
                            milestones::update_leader_and_check_crown(
                                conn,
                                category_id,
                                kind,
                                &l.pilot_id,
                                &l.pilot_name,
                                l.value,
                                floor,
                                active_season.numero,
                                round,
                            )
                        {
                            let _ = milestones::insert_milestone(
                                conn,
                                category_id,
                                &milestones::RecordMilestone {
                                    metric: kind.to_string(),
                                    pilot_id: l.pilot_id,
                                    pilot_name: l.pilot_name,
                                    value: l.value,
                                    previous_value: Some(prev_val),
                                    context: prev_name,
                                    season_number: active_season.numero,
                                    ano: active_season.ano,
                                    round,
                                },
                            );
                        }
                    }
                };
                crown(
                    "most_starts_no_win",
                    race_history::get_category_most_starts_no_win(conn, category_id).ok().flatten(),
                    30,
                );
                crown(
                    "most_career_dnfs",
                    race_history::get_category_most_career_dnfs(conn, category_id).ok().flatten(),
                    20,
                );
                crown(
                    "most_poles_no_title",
                    race_history::get_category_most_poles_no_title(conn, category_id).ok().flatten(),
                    5,
                );
                crown(
                    "most_career_points",
                    race_history::get_category_most_career_points(conn, category_id).ok().flatten(),
                    300,
                );
            }

            // Por destaque: o RECORDISTA aparece sempre que está em evidência (descreve
            // quem ele é, independe do resultado de hoje). Marcos de número redondo só
            // para quem REALMENTE fez aquilo hoje (venceu / subiu ao pódio / largou).
            for pilot_id in &featured {
                let is_winner = pilot_id == winner_id;
                let is_player = race_result
                    .race_results
                    .iter()
                    .any(|d| d.pilot_id == *pilot_id && d.is_jogador);
                let Ok(career) = crate::db::queries::race_history::get_driver_category_career(
                    conn,
                    pilot_id,
                    category_id,
                ) else {
                    continue;
                };
                let name = driver_queries::get_driver(conn, pilot_id)
                    .map(|d| d.nome)
                    .unwrap_or_else(|_| pilot_id.clone());
                let recs = records.as_ref();
                let holds = |rec: fn(&crate::db::queries::race_history::CategoryRecords) -> &Option<crate::db::queries::race_history::CategoryRecord>| {
                    recs.and_then(|r| rec(r).as_ref())
                        .map_or(false, |m| m.pilot_id == *pilot_id)
                };
                let is_wins_record = holds(|r| &r.most_wins);
                let is_podiums_record = holds(|r| &r.most_podiums);
                let is_starts_record = holds(|r| &r.most_starts);

                // Recordes históricos da categoria (estado — vale sempre que aparece).
                if is_wins_record {
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.record_wins",
                            name = name.as_str(),
                            wins = career.wins
                        )
                        .to_string(),
                    );
                }
                if is_podiums_record {
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.record_podiums",
                            name = name.as_str(),
                            podiums = career.podiums
                        )
                        .to_string(),
                    );
                }
                if is_starts_record {
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.record_starts",
                            name = name.as_str(),
                            starts = career.starts
                        )
                        .to_string(),
                    );
                }

                // Marcos de número redondo — só vencedor e jogador, e só se fez hoje.
                if is_winner || is_player {
                    let podium_today = race_result.race_results.iter().any(|d| {
                        d.pilot_id == *pilot_id
                            && !d.is_dnf
                            && (1..=3).contains(&d.finish_position)
                    });
                    if is_winner && !is_wins_record && [5, 10, 25, 50, 75, 100].contains(&career.wins)
                    {
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.nth_win",
                                n = career.wins,
                                name = name.as_str()
                            )
                            .to_string(),
                        );
                    }
                    if podium_today
                        && !is_podiums_record
                        && [25, 50, 100, 150].contains(&career.podiums)
                    {
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.nth_podium",
                                name = name.as_str(),
                                n = career.podiums
                            )
                            .to_string(),
                        );
                    }
                    if !is_starts_record && [50, 100, 150, 200, 250].contains(&career.starts) {
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.nth_start",
                                n = career.starts,
                                name = name.as_str()
                            )
                            .to_string(),
                        );
                    }
                }
            }
        }

        // --- Duelo interno: quem levou a melhor sobre o companheiro de equipe. ---
        // Só para o vencedor e o jogador (foco), lendo o próprio resultado — o "carro
        // irmão" é a referência mais justa da corrida do piloto. Par deduplicado.
        {
            let mut focus: Vec<&str> = vec![race_result.winner_id.as_str()];
            if let Some(p) = race_result.race_results.iter().find(|d| d.is_jogador) {
                focus.push(p.pilot_id.as_str());
            }
            let mut seen_pairs: std::collections::HashSet<(String, String)> =
                std::collections::HashSet::new();
            for pid in focus {
                let Some(me) = race_result.race_results.iter().find(|d| d.pilot_id == pid) else {
                    continue;
                };
                if me.team_id.is_empty() {
                    continue;
                }
                let Some(mate) = race_result
                    .race_results
                    .iter()
                    .find(|d| d.team_id == me.team_id && d.pilot_id != me.pilot_id)
                else {
                    continue;
                };
                let key = if me.pilot_id <= mate.pilot_id {
                    (me.pilot_id.clone(), mate.pilot_id.clone())
                } else {
                    (mate.pilot_id.clone(), me.pilot_id.clone())
                };
                if !seen_pairs.insert(key) {
                    continue;
                }
                // Quem terminou à frente: melhor posição, ou o único a completar.
                let me_ahead = match (me.is_dnf, mate.is_dnf) {
                    (false, true) => true,
                    (true, false) => false,
                    (false, false) => me.finish_position < mate.finish_position,
                    (true, true) => continue, // ambos fora → sem duelo interno
                };
                let (ahead, team, behind) = if me_ahead {
                    (&me.pilot_name, &me.team_name, &mate.pilot_name)
                } else {
                    (&mate.pilot_name, &mate.team_name, &me.pilot_name)
                };
                context_facts.push(
                    rust_i18n::t!(
                        "briefing.ctx.internal_duel",
                        team = team.as_str(),
                        ahead = ahead.as_str(),
                        behind = behind.as_str()
                    )
                    .to_string(),
                );
            }
        }

        // --- Quadro do campeonato: o que o resultado significa para a temporada. ---
        // Os resultados desta corrida já estão em `race_results` (persistidos em
        // simulate_category_race, antes daqui), então os standings já a incluem.
        let total_rodadas = crate::constants::categories::get_category_config(category_id)
            .map(|c| c.corridas_por_temporada as i32)
            .unwrap_or(round);
        let races_left = (total_rodadas - round).max(0);

        // "Valor de uma vitória" nesta categoria = pontos do vencedor desta corrida.
        // Vira o limiar de "briga apertada" sem depender da escala de pontos: um gap
        // menor que isso é recuperável numa única corrida → a disputa segue viva.
        let win_value = race_result
            .race_results
            .iter()
            .find(|d| d.pilot_id == race_result.winner_id)
            .map(|d| d.points_earned)
            .unwrap_or(0)
            .max(1) as f64;

        // Reta final / próxima é a decisiva.
        match races_left {
            0 => context_facts.push(rust_i18n::t!("briefing.ctx.season_last").to_string()),
            1 => context_facts.push(rust_i18n::t!("briefing.ctx.season_one_left").to_string()),
            2 => context_facts.push(rust_i18n::t!("briefing.ctx.season_two_left").to_string()),
            // "Reta final" só faz sentido quando a temporada de fato já passou da
            // metade. Numa temporada curta (ex.: 5 etapas), 4 restantes significa
            // que só a 1ª rodada foi disputada — não é reta final.
            n if n <= 4 && round * 2 > total_rodadas => context_facts
                .push(rust_i18n::t!("briefing.ctx.season_final_stretch", n = n).to_string()),
            _ => {}
        }

        // Brigas no campeonato só fazem sentido depois de algumas corridas (gap real).
        if round >= 2 {
            // Pilotos: título em aberto (P1×P2) OU, com o líder encaminhado, o vice (P2×P3).
            if let Ok(st) = crate::db::queries::race_history::get_category_standings(
                conn,
                &active_season.id,
                category_id,
            ) {
                if st.len() >= 2 {
                    let gap12 = (st[0].points - st[1].points).round();
                    if gap12 <= win_value {
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.title_fight",
                                leader = st[0].pilot_name.as_str(),
                                gap = gap12 as i32,
                                second = st[1].pilot_name.as_str()
                            )
                            .to_string(),
                        );
                    } else if st.len() >= 3 {
                        let gap23 = (st[1].points - st[2].points).round();
                        if gap23 <= win_value {
                            context_facts.push(
                                rust_i18n::t!(
                                    "briefing.ctx.vice_fight",
                                    leader = st[0].pilot_name.as_str(),
                                    second = st[1].pilot_name.as_str(),
                                    third = st[2].pilot_name.as_str(),
                                    gap = gap23 as i32
                                )
                                .to_string(),
                            );
                        }
                    }
                }
            }

            // Equipes: mesma lógica (ponta OU vice).
            if let Ok(ts) = crate::db::queries::race_history::get_team_standings(
                conn,
                &active_season.id,
                category_id,
            ) {
                if ts.len() >= 2 {
                    let gap12 = (ts[0].points - ts[1].points).round();
                    if gap12 <= win_value {
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.teams_top_fight",
                                a = ts[0].team_name.as_str(),
                                b = ts[1].team_name.as_str(),
                                gap = gap12 as i32
                            )
                            .to_string(),
                        );
                    } else if ts.len() >= 3 {
                        let gap23 = (ts[1].points - ts[2].points).round();
                        if gap23 <= win_value {
                            context_facts.push(
                                rust_i18n::t!(
                                    "briefing.ctx.teams_vice_fight",
                                    a = ts[1].team_name.as_str(),
                                    b = ts[2].team_name.as_str(),
                                    gap = gap23 as i32
                                )
                                .to_string(),
                            );
                        }
                    }
                }
            }
        }

        // Final da temporada: piloto em destaque que trocou de equipe e foi parar
        // num time que terminou ATRÁS do que ele deixou — "a aposta saiu cara".
        // Só na última etapa, e só na mesma categoria (troca lateral, não promoção).
        if races_left == 0 {
            if let Ok(ts) = crate::db::queries::race_history::get_team_standings(
                conn,
                &active_season.id,
                category_id,
            ) {
                let pos_of = |team_id: &str| {
                    ts.iter().find(|t| t.team_id == team_id).map(|t| t.position)
                };
                let prev_season = active_season.numero - 1;

                // Candidatos (jogador tem prioridade; senão, a virada mais dramática).
                struct SwitchRegret {
                    pilot_name: String,
                    old_team: String,
                    new_team: String,
                    old_pos: i32,
                    new_pos: i32,
                    is_player: bool,
                }
                let mut candidates: Vec<SwitchRegret> = Vec::new();

                for pilot_id in &featured {
                    let Some(cur) = race_result
                        .race_results
                        .iter()
                        .find(|d| d.pilot_id == *pilot_id)
                    else {
                        continue;
                    };
                    let contracts = match crate::db::queries::contracts::get_contracts_for_pilot(
                        conn, pilot_id,
                    ) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };
                    // Equipe na temporada passada, mesma categoria, time diferente do atual.
                    let Some(prev) = contracts.iter().find(|c| {
                        c.categoria.as_str() == category_id
                            && c.temporada_inicio <= prev_season
                            && c.temporada_fim >= prev_season
                            && c.equipe_id != cur.team_id
                    }) else {
                        continue;
                    };
                    if let (Some(new_pos), Some(old_pos)) =
                        (pos_of(&cur.team_id), pos_of(&prev.equipe_id))
                    {
                        // O time que ele DEIXOU terminou À FRENTE do que ele escolheu.
                        if old_pos < new_pos {
                            candidates.push(SwitchRegret {
                                pilot_name: cur.pilot_name.clone(),
                                old_team: prev.equipe_nome.clone(),
                                new_team: cur.team_name.clone(),
                                old_pos,
                                new_pos,
                                is_player: cur.is_jogador,
                            });
                        }
                    }
                }

                // Emite no máximo um (evita poluir): jogador primeiro, senão maior virada.
                if let Some(r) = candidates
                    .iter()
                    .find(|c| c.is_player)
                    .or_else(|| candidates.iter().max_by_key(|c| c.new_pos - c.old_pos))
                {
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.switch_regret",
                            pilot = r.pilot_name.as_str(),
                            old_team = r.old_team.as_str(),
                            new_team = r.new_team.as_str(),
                            old_pos = r.old_pos,
                            new_pos = r.new_pos
                        )
                        .to_string(),
                    );
                }
            }
        }

        // --- Arco de rivalidade (a "novela"): registra o capítulo de hoje no log de
        // episódios e recapitula o arco para os destaques que se cruzaram na pista. ---
        record_rivalry_episodes(
            conn,
            race_result,
            flat_incidents,
            category_id,
            round,
            active_season.numero,
            active_season.ano,
        );
        for fact in rivalry_arc_facts(conn, race_result, &featured, active_season.numero, round) {
            context_facts.push(fact);
        }

        // Desempenho e forma: esperado×real, forma recente, histórico de pista e
        // confronto entre companheiros (pano de fundo dos destaques).
        for fact in
            performance_context_facts(conn, race_result, &featured, active_season, round, category_id)
        {
            context_facts.push(fact);
        }

        // Telemetria REAL do SDK (só corrida importada do iRacing): ritmo, duelo,
        // erro mais caro, melhor momento — cor sobre a corrida do próprio jogador.
        for fact in extra_context_facts {
            context_facts.push(fact.clone());
        }

        // Peça 3 · notícia: PENALIDADES de quebra (não-DNF) — "perdeu tempo arrumando a peça X,
        // problema leve/grave". Os DNFs de quebra já entram pelo beat Abandono (Camada B); aqui
        // entram as paradas `!black`. Vazio no sim offline (só corrida ao vivo dispara quebra).
        let race_id_for_breakdowns = crate::db::queries::calendar::get_calendar(
            conn,
            &active_season.id,
            category_id,
        )
        .ok()
        .and_then(|entries| entries.into_iter().find(|e| e.rodada == round).map(|e| e.id));
        if let Some(rid) = &race_id_for_breakdowns {
            if let Ok(bds) = crate::db::queries::race_breakdowns::get_breakdowns_for_race(conn, rid) {
                let mut count = 0;
                for b in bds.iter().filter(|b| b.severity != "dnf") {
                    if count >= 6 {
                        break;
                    }
                    let Some(dr) = race_result
                        .race_results
                        .iter()
                        .find(|d| d.pilot_id == b.driver_id)
                    else {
                        continue;
                    };
                    let part_name = crate::car::PartType::from_str(&b.part)
                        .map(|pt| pt.display_name(category_id).to_string())
                        .unwrap_or_else(|| b.part.clone());
                    let grav = if b.severity == "heavy" {
                        rust_i18n::t!("briefing.ctx.severity_heavy")
                    } else {
                        rust_i18n::t!("briefing.ctx.severity_light")
                    };
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.breakdown_pit",
                            name = dr.pilot_name.as_str(),
                            team = dr.team_name.as_str(),
                            secs = b.penalty_secs.unwrap_or(0),
                            part = part_name.as_str(),
                            label = b.label.as_str(),
                            severity = grav
                        )
                        .to_string(),
                    );
                    count += 1;
                }
            }
        }

        let ctx = crate::narrative::build_race_context(
            race_result,
            &crate::narrative::RaceContextInput {
                category_name,
                year: active_season.ano,
                round,
                injuries: &injury_facts,
                incidents: flat_incidents,
                context_facts: &context_facts,
            },
        );
        // Mapa nome da equipe → cor primária das equipes desta corrida. O front usa
        // para colorir os nomes das equipes citadas no boletim. Dedup por team_id.
        let mut team_colors = serde_json::Map::new();
        let mut seen_teams: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for d in &race_result.race_results {
            if d.team_name.is_empty() || !seen_teams.insert(d.team_id.as_str()) {
                continue;
            }
            let color: Option<String> = conn
                .query_row(
                    "SELECT cor_primaria FROM teams WHERE id = ?1",
                    rusqlite::params![d.team_id],
                    |r| r.get::<_, String>(0),
                )
                .ok();
            if let Some(c) = color {
                if !c.trim().is_empty() {
                    team_colors.insert(d.team_name.clone(), serde_json::Value::String(c));
                }
            }
        }
        let teams_json = serde_json::Value::Object(team_colors).to_string();

        if let Err(e) =
            crate::db::queries::ai_story::store_race_facts(conn, &news_id, &ctx.facts, &teams_json)
        {
            eprintln!("[narrative] Falha ao guardar fatos do boletim de IA: {e:?}");
        }
    }

    Ok(returned_news_id)
}

/// Pré-gera o boletim de IA em BACKGROUND logo após a corrida, para que ele já
/// esteja em cache quando o jogador abrir a aba de Notícias (sem sentir a latência
/// do servidor). Roda numa thread própria com conexão própria ao banco. Silencioso:
/// se falhar (rede/cooldown), o caminho lazy de abrir a notícia tenta de novo.
fn spawn_prewarm_boletim(
    db_path: std::path::PathBuf,
    news_id: String,
    lang: String,
    install_id: String,
) {
    std::thread::spawn(move || {
        let db = match Database::open_existing(&db_path) {
            Ok(d) => d,
            Err(_) => return,
        };
        let row = match crate::db::queries::ai_story::get_story(&db.conn, &news_id) {
            Ok(Some(r)) => r,
            _ => return,
        };
        if row.story.is_some() {
            return; // já gerado em algum momento — nada a fazer
        }
        // reading_seconds = None → tamanho padrão do servidor (a adaptação por
        // engajamento continua valendo no caminho lazy, se ainda não houver cache).
        if let Ok(story) =
            crate::narrative::client::fetch_story(&row.facts, &lang, &install_id, None)
        {
            let _ = crate::db::queries::ai_story::set_story(&db.conn, &news_id, &story);
        }
    });
}

/// Registra um CAPÍTULO de rivalidade por corrida em que dois rivais interagiram
/// (colisão, duelo de posições coladas, ou briga na ponta). Constrói a memória que o
/// boletim recapitula depois. As intensidades já foram atualizadas na transação da
/// corrida, então aqui só lemos o estado e gravamos o episódio.
fn record_rivalry_episodes(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    flat_incidents: &[IncidentResult],
    categoria: &str,
    rodada: i32,
    temporada: i32,
    ano: i32,
) {
    use crate::db::queries::drivers as driver_queries;
    use crate::models::rivalry::normalize_pair;
    use crate::simulation::incidents::IncidentType;
    use std::collections::{HashMap, HashSet};

    // pilot -> (posição final, dnf)
    let mut info: HashMap<&str, (i32, bool)> = HashMap::new();
    for d in &race_result.race_results {
        info.insert(d.pilot_id.as_str(), (d.finish_position, d.is_dnf));
    }

    // pares que colidiram nesta corrida (normalizados)
    let mut collided: HashSet<(String, String)> = HashSet::new();
    for inc in flat_incidents {
        if inc.incident_type == IncidentType::Collision {
            if let Some(other) = &inc.linked_pilot_id {
                if let Some(pair) = normalize_pair(&inc.pilot_id, other) {
                    collided.insert((pair.piloto1_id, pair.piloto2_id));
                }
            }
        }
    }

    let rivalries = match crate::db::queries::rivalries::get_all_rivalries(conn) {
        Ok(r) => r,
        Err(_) => return,
    };

    for riv in rivalries {
        let Some((pos_a, dnf_a)) = info.get(riv.piloto1_id.as_str()).copied() else {
            continue;
        };
        let Some((pos_b, dnf_b)) = info.get(riv.piloto2_id.as_str()).copied() else {
            continue;
        };

        let perceived = riv.perceived_intensity();
        let pair_key = (riv.piloto1_id.clone(), riv.piloto2_id.clone());
        let did_collide = collided.contains(&pair_key);

        // Colisão sempre vira capítulo (é a origem); duelo/ponta só se já é notável.
        if !did_collide && perceived < 30.0 {
            continue;
        }

        let both_finished = !dnf_a && !dnf_b;
        let close = both_finished && (pos_a - pos_b).abs() <= 3;
        let top_front = both_finished && pos_a <= 5 && pos_b <= 5;

        let interaction = if did_collide {
            "colisao"
        } else if close {
            "duelo"
        } else if top_front {
            "campeonato"
        } else {
            continue; // sem interação de verdade hoje
        };

        // Quem levou a melhor: melhor posição, ou o único a completar.
        let winner_id = if both_finished {
            match pos_a.cmp(&pos_b) {
                std::cmp::Ordering::Less => Some(riv.piloto1_id.clone()),
                std::cmp::Ordering::Greater => Some(riv.piloto2_id.clone()),
                std::cmp::Ordering::Equal => None,
            }
        } else if !dnf_a {
            Some(riv.piloto1_id.clone())
        } else if !dnf_b {
            Some(riv.piloto2_id.clone())
        } else {
            None
        };

        let name = |id: &str| {
            driver_queries::get_driver(conn, id)
                .map(|d| d.nome)
                .unwrap_or_else(|_| id.to_string())
        };
        let na = name(&riv.piloto1_id);
        let nb = name(&riv.piloto2_id);
        let summary = match interaction {
            "colisao" => format!("contato entre {na} e {nb} em {}", race_result.track_name),
            "duelo" => match &winner_id {
                Some(w) => {
                    let (wn, wp, lp) = if *w == riv.piloto1_id {
                        (&na, pos_a, pos_b)
                    } else {
                        (&nb, pos_b, pos_a)
                    };
                    format!("{wn} levou a melhor no duelo direto, {wp}º contra {lp}º")
                }
                None => format!("duelo parelho entre {na} e {nb}"),
            },
            _ => format!("{na} e {nb} brigaram por posições de ponta ({pos_a}º e {pos_b}º)"),
        };

        let ep = crate::db::queries::rivalry_episodes::RivalryEpisode {
            piloto1_id: riv.piloto1_id.clone(),
            piloto2_id: riv.piloto2_id.clone(),
            temporada,
            rodada,
            ano,
            categoria: categoria.to_string(),
            track_name: race_result.track_name.clone(),
            interaction: interaction.to_string(),
            winner_id,
            summary,
            perceived,
        };
        let _ = crate::db::queries::rivalry_episodes::insert_episode(conn, &ep);
    }
}

/// Recapitula o ARCO de rivalidade para os destaques que se cruzaram HOJE: origem,
/// número de capítulos, retrospecto direto (h2h), o capítulo de hoje e revanche.
/// Só para rivalidades já claras (percebida ≥ 40) com capítulo registrado nesta corrida.
fn rivalry_arc_facts(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    featured: &[String],
    temporada: i32,
    rodada: i32,
) -> Vec<String> {
    use crate::db::queries::drivers as driver_queries;
    use crate::models::rivalry::RivalryType;
    use std::collections::HashSet;

    let mut out = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let name = |id: &str| {
        driver_queries::get_driver(conn, id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| id.to_string())
    };

    for pilot_id in featured {
        let rivs = match crate::rivalry::get_pilot_rivalries(conn, pilot_id) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for r in rivs {
            if r.perceived_intensity < 40.0 {
                continue;
            }
            // O rival precisa ter corrido hoje.
            if !race_result
                .race_results
                .iter()
                .any(|d| d.pilot_id == r.rival_id)
            {
                continue;
            }
            // Par normalizado para deduplicar (o par pode vir por ambos os lados).
            let (a, b) = if *pilot_id <= r.rival_id {
                (pilot_id.clone(), r.rival_id.clone())
            } else {
                (r.rival_id.clone(), pilot_id.clone())
            };
            if !seen.insert((a.clone(), b.clone())) {
                continue;
            }

            let eps = match crate::db::queries::rivalry_episodes::get_episodes_for_pair(conn, &a, &b)
            {
                Ok(e) => e,
                Err(_) => continue,
            };
            // Só recapitula se houve capítulo HOJE (a rivalidade se manifestou na corrida).
            let Some(today) = eps
                .last()
                .filter(|e| e.temporada == temporada && e.rodada == rodada)
            else {
                continue;
            };

            let na = name(&a);
            let nb = name(&b);
            let nivel = crate::rivalry::intensity_level(r.perceived_intensity).label();
            let chapters = eps.len();
            let ano_origem = eps.first().map(|e| e.ano).unwrap_or(0);

            // Retrospecto direto (h2h).
            let mut wins_a = 0;
            let mut wins_b = 0;
            for e in &eps {
                match e.winner_id.as_deref() {
                    Some(w) if w == a => wins_a += 1,
                    Some(w) if w == b => wins_b += 1,
                    _ => {}
                }
            }

            // Revanche: hoje X venceu e no capítulo ANTERIOR quem venceu foi o outro.
            let revenge = eps.len() >= 2
                && today.winner_id.is_some()
                && eps
                    .get(eps.len() - 2)
                    .and_then(|p| p.winner_id.as_deref())
                    .zip(today.winner_id.as_deref())
                    .map_or(false, |(prev_w, today_w)| prev_w != today_w);

            let origem = match r.tipo {
                RivalryType::Colisao => rust_i18n::t!("briefing.rivalry.origin_collision"),
                RivalryType::Companheiros => rust_i18n::t!("briefing.rivalry.origin_teammates"),
                RivalryType::Campeonato => rust_i18n::t!("briefing.rivalry.origin_championship"),
                RivalryType::Pista => rust_i18n::t!("briefing.rivalry.origin_track"),
            };

            let mut s = rust_i18n::t!(
                "briefing.rivalry.opener",
                a = na.as_str(),
                b = nb.as_str(),
                level = nivel,
                origin = origem
            )
            .to_string();
            if ano_origem > 0 && chapters > 1 {
                s.push_str(&rust_i18n::t!(
                    "briefing.rivalry.chapters",
                    chapters = chapters,
                    year = ano_origem
                ));
            }
            s.push_str(&rust_i18n::t!(
                "briefing.rivalry.today",
                summary = today.summary.as_str()
            ));
            if wins_a > 0 || wins_b > 0 {
                if wins_a == wins_b {
                    s.push_str(&rust_i18n::t!(
                        "briefing.rivalry.h2h_tied",
                        a = wins_a,
                        b = wins_b
                    ));
                } else {
                    let (leader, hi, lo) = if wins_a > wins_b {
                        (&na, wins_a, wins_b)
                    } else {
                        (&nb, wins_b, wins_a)
                    };
                    s.push_str(&rust_i18n::t!(
                        "briefing.rivalry.h2h_leader",
                        leader = leader.as_str(),
                        hi = hi,
                        lo = lo
                    ));
                }
            }
            if revenge {
                if let Some(tw) = today.winner_id.as_deref() {
                    let twn = if tw == a { &na } else { &nb };
                    s.push_str(&rust_i18n::t!("briefing.rivalry.revenge", name = twn.as_str()));
                }
            }
            out.push(s);
        }
    }
    out
}

/// Fatos de DESEMPENHO e FORMA para os destaques, como pano de fundo do boletim:
/// 1) esperado×real (reaproveita o cérebro `race_eval`: largada + mérito do conjunto);
/// 2) forma recente (últimas 5 corridas na categoria);
/// 3) histórico no circuito (já venceu aqui);
/// 4) confronto entre companheiros de equipe.
/// Tudo gated com folga para não inundar o contexto. Lógica de DB aqui; o módulo
/// `narrative` permanece puro.
fn performance_context_facts(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    featured: &[String],
    active_season: &Season,
    round: i32,
    category_id: &str,
) -> Vec<String> {
    use crate::db::queries::drivers as driver_queries;
    use crate::db::queries::race_history as rh;
    use crate::race_eval::{compute_merit, evaluate, Assessment, DriverMerit, RaceEvalInput};
    use std::collections::HashMap;

    let mut out: Vec<String> = Vec::new();
    let rows = &race_result.race_results;
    let field_size = rows.len().max(1) as i32;
    if rows.is_empty() {
        return out;
    }

    // Carrega cada participante uma vez (skill + nome) e o car_performance por equipe.
    let mut drivers: HashMap<String, Driver> = HashMap::new();
    for d in rows {
        if let std::collections::hash_map::Entry::Vacant(e) = drivers.entry(d.pilot_id.clone()) {
            if let Ok(drv) = driver_queries::get_driver(conn, &d.pilot_id) {
                e.insert(drv);
            }
        }
    }
    let mut car_perf: HashMap<String, f64> = HashMap::new();
    for d in rows {
        if let std::collections::hash_map::Entry::Vacant(e) = car_perf.entry(d.team_id.clone()) {
            let cp: f64 = conn
                .query_row(
                    "SELECT car_performance FROM teams WHERE id = ?1",
                    rusqlite::params![d.team_id],
                    |r| r.get(0),
                )
                .unwrap_or(50.0);
            e.insert(cp);
        }
    }
    let name_of = |id: &str| -> String {
        drivers
            .get(id)
            .map(|d| d.nome.clone())
            .unwrap_or_else(|| id.to_string())
    };

    // Campo de mérito (skill + carro pesam igual) — base da posição ESPERADA.
    let field: Vec<DriverMerit> = rows
        .iter()
        .map(|d| {
            let skill = drivers.get(&d.pilot_id).map(|x| x.atributos.skill).unwrap_or(50.0);
            let car = car_perf.get(&d.team_id).copied().unwrap_or(50.0);
            DriverMerit {
                pilot_id: d.pilot_id.clone(),
                merit: compute_merit(skill, car, None, field_size, 0),
            }
        })
        .collect();

    // ── 1) Esperado×real: só sinais FORTES (muito acima / muito abaixo). ──────────
    // O pole que floppou já é coberto pelo beat de Decepção; o DNF, pelo de Abandono.
    struct ExpCand {
        text: String,
        is_player: bool,
    }
    let mut exp: Vec<ExpCand> = Vec::new();
    for pilot_id in featured {
        let Some(d) = rows.iter().find(|x| &x.pilot_id == pilot_id) else {
            continue;
        };
        if d.is_dnf {
            continue;
        }
        let ev = evaluate(&RaceEvalInput {
            player_id: pilot_id.clone(),
            grid_position: d.grid_position,
            finish_position: d.finish_position,
            is_dnf: false,
            incidents: d.incidents_count,
            field: field.clone(),
        });
        let is_pole = *pilot_id == race_result.pole_sitter_id;
        let name = name_of(pilot_id);
        let text = match ev.assessment {
            Assessment::MuitoAcima => rust_i18n::t!(
                "briefing.perf.much_above",
                name = name.as_str(),
                grid = d.grid_position,
                finish = d.finish_position
            )
            .to_string(),
            Assessment::MuitoAbaixo if !is_pole => rust_i18n::t!(
                "briefing.perf.much_below",
                name = name.as_str(),
                grid = d.grid_position,
                finish = d.finish_position
            )
            .to_string(),
            _ => continue,
        };
        exp.push(ExpCand { text, is_player: d.is_jogador });
    }
    // No máximo 2, com o jogador tendo prioridade.
    exp.sort_by_key(|c| std::cmp::Reverse(c.is_player));
    for c in exp.into_iter().take(2) {
        out.push(c.text);
    }

    // ── 2) Forma recente (últimas 5 na categoria, antes de hoje). ────────────────
    // prioridade: fim de jejum (3) > sequência de pódios (2) > reação (1).
    let mut form: Vec<(i32, String)> = Vec::new();
    for pilot_id in featured {
        let Some(d) = rows.iter().find(|x| &x.pilot_id == pilot_id) else {
            continue;
        };
        if d.is_dnf {
            continue;
        }
        let recent = rh::get_recent_finishes_before(
            conn,
            pilot_id,
            category_id,
            active_season.numero,
            round,
            5,
        )
        .unwrap_or_default();
        if recent.len() < 3 {
            continue; // pouca história → sem leitura de forma confiável
        }
        let name = name_of(pilot_id);
        let recent_wins = recent.iter().filter(|r| r.finish == 1).count();
        let last_two_podiums = recent
            .iter()
            .take(2)
            .filter(|r| !r.is_dnf && r.finish <= 3)
            .count();

        if d.finish_position == 1 && recent_wins == 0 && recent.len() >= 5 {
            form.push((
                3,
                rust_i18n::t!("briefing.perf.end_drought", name = name.as_str()).to_string(),
            ));
        } else if d.finish_position <= 3 && last_two_podiums == 2 {
            form.push((
                2,
                rust_i18n::t!("briefing.perf.podium_streak", name = name.as_str()).to_string(),
            ));
        } else if d.finish_position <= 5 {
            let valid: Vec<i32> = recent.iter().filter(|r| !r.is_dnf).map(|r| r.finish).collect();
            if valid.len() >= 3 {
                let avg = valid.iter().sum::<i32>() as f64 / valid.len() as f64;
                if avg >= field_size as f64 * 0.5 {
                    form.push((
                        1,
                        rust_i18n::t!("briefing.perf.reaction", name = name.as_str()).to_string(),
                    ));
                }
            }
        }
    }
    form.sort_by_key(|(p, _)| std::cmp::Reverse(*p));
    for (_, t) in form.into_iter().take(2) {
        out.push(t);
    }

    // ── 3) Histórico no circuito: destaque que já venceu aqui antes. ─────────────
    if let Ok(Some(track_id)) =
        rh::get_round_track_id(conn, &active_season.id, category_id, round)
    {
        let mut track_facts: Vec<(i32, String)> = Vec::new();
        for pilot_id in featured {
            let Some(d) = rows.iter().find(|x| &x.pilot_id == pilot_id) else {
                continue;
            };
            if d.is_dnf {
                continue;
            }
            let th = rh::get_pilot_track_history(conn, pilot_id, track_id, &active_season.id, round)
                .unwrap_or_default();
            if th.wins < 1 {
                continue;
            }
            let name = name_of(pilot_id);
            let vez = if th.wins == 1 {
                rust_i18n::t!("briefing.perf.time_singular")
            } else {
                rust_i18n::t!("briefing.perf.time_plural")
            };
            let text = if d.finish_position == 1 {
                rust_i18n::t!(
                    "briefing.perf.track_specialist",
                    name = name.as_str(),
                    wins = th.wins,
                    times = vez
                )
                .to_string()
            } else if d.finish_position <= 3 {
                rust_i18n::t!(
                    "briefing.perf.track_good_history",
                    name = name.as_str(),
                    wins = th.wins,
                    times = vez
                )
                .to_string()
            } else {
                continue;
            };
            track_facts.push((th.wins, text));
        }
        track_facts.sort_by_key(|(w, _)| std::cmp::Reverse(*w));
        for (_, t) in track_facts.into_iter().take(2) {
            out.push(t);
        }
    }

    // ── 4) Confronto entre companheiros: par de destaques na MESMA equipe. ───────
    // Emite no máximo 1 (jogador tem prioridade). Exige ambos classificados hoje.
    let mut h2h: Option<(bool, String)> = None;
    'pairs: for i in 0..featured.len() {
        for j in (i + 1)..featured.len() {
            let (Some(a), Some(b)) = (
                rows.iter().find(|x| x.pilot_id == featured[i]),
                rows.iter().find(|x| x.pilot_id == featured[j]),
            ) else {
                continue;
            };
            if a.team_id != b.team_id || a.is_dnf || b.is_dnf {
                continue;
            }
            // Placar do confronto interno na temporada (rodadas em que ambos completaram).
            let ra = rh::get_pilot_season_results(conn, &a.pilot_id, &active_season.id, category_id)
                .unwrap_or_default();
            let rb = rh::get_pilot_season_results(conn, &b.pilot_id, &active_season.id, category_id)
                .unwrap_or_default();
            let rb_map: HashMap<i32, (i32, bool)> =
                rb.iter().map(|(r, f, dnf)| (*r, (*f, *dnf))).collect();
            let (mut wa, mut wb) = (0, 0);
            for (rnd, fa, da) in &ra {
                if let Some((fb, db)) = rb_map.get(rnd) {
                    if *da || *db {
                        continue;
                    }
                    if fa < fb {
                        wa += 1;
                    } else if fb < fa {
                        wb += 1;
                    }
                }
            }
            let (ahead, behind) = if a.finish_position < b.finish_position {
                (a, b)
            } else {
                (b, a)
            };
            let (an, bn) = (name_of(&ahead.pilot_id), name_of(&behind.pilot_id));
            let mut s = rust_i18n::t!(
                "briefing.perf.teammate_h2h",
                ahead = an.as_str(),
                behind = bn.as_str(),
                ap = ahead.finish_position,
                bp = behind.finish_position
            )
            .to_string();
            if wa + wb >= 2 {
                if wa == wb {
                    s.push_str(&rust_i18n::t!("briefing.perf.teammate_tied", a = wa, b = wb));
                } else {
                    let (ln, hi, lo) = if wa > wb {
                        (name_of(&a.pilot_id), wa, wb)
                    } else {
                        (name_of(&b.pilot_id), wb, wa)
                    };
                    s.push_str(&rust_i18n::t!(
                        "briefing.perf.teammate_leader",
                        leader = ln.as_str(),
                        hi = hi,
                        lo = lo
                    ));
                }
            } else {
                s.push('.');
            }
            let involves_player = ahead.is_jogador || behind.is_jogador;
            if h2h.as_ref().map_or(true, |(p, _)| involves_player && !p) {
                h2h = Some((involves_player, s));
                if involves_player {
                    break 'pairs;
                }
            }
        }
    }
    if let Some((_, s)) = h2h {
        out.push(s);
    }

    out
}

/// Converte a TELEMETRIA REAL do SDK (ritmo, duelo, erro mais caro, melhor momento)
/// em fatos de pano de fundo sobre a corrida do JOGADOR — a cor que só existe quando
/// ele correu de verdade no iRacing. O jogador é CITADO (subtrama), nunca protagonista;
/// estes fatos entram na seção "Contexto" e a IA tece quando fizer sentido. Tolerante:
/// cada item só sai se o sinal for confiável (o motor de telemetria já gateia isso).
fn telemetry_context_facts(
    telemetry: &crate::iracing_sdk::telemetry_analysis::TelemetryAnalysis,
    player_name: &str,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if !telemetry.has_telemetry || player_name.trim().is_empty() {
        return out;
    }
    let who = player_name;

    if let Some(p) = &telemetry.pace {
        // Ritmo vs campo (só se a amostra do grid for confiável).
        if p.vs_grid_reliable {
            let delta_s = (p.vs_grid_ms.abs() / 1000.0 * 10.0).round() / 10.0;
            if p.vs_grid_ms <= -200.0 {
                out.push(
                    rust_i18n::t!(
                        "briefing.tel.faster_than_grid",
                        who = who,
                        delta = format!("{delta_s:.1}")
                    )
                    .to_string(),
                );
            } else if p.vs_grid_ms >= 200.0 {
                out.push(
                    rust_i18n::t!(
                        "briefing.tel.slower_than_grid",
                        who = who,
                        delta = format!("{delta_s:.1}")
                    )
                    .to_string(),
                );
            }
        }
        // Consistência (só com voltas suficientes).
        if p.consistency_reliable && p.total_laps >= 4 {
            let ratio = p.good_laps as f64 / p.total_laps as f64;
            if ratio >= 0.85 {
                out.push(
                    rust_i18n::t!(
                        "briefing.tel.consistent",
                        who = who,
                        good = p.good_laps,
                        total = p.total_laps
                    )
                    .to_string(),
                );
            } else if ratio <= 0.5 {
                out.push(
                    rust_i18n::t!(
                        "briefing.tel.inconsistent",
                        who = who,
                        good = p.good_laps,
                        total = p.total_laps
                    )
                    .to_string(),
                );
            }
        }
    }

    // Duelo com o rival mais constante.
    if let Some(r) = &telemetry.rival {
        if !r.pilot_name.trim().is_empty() {
            let gap = (r.avg_gap_s * 10.0).round() / 10.0;
            out.push(
                rust_i18n::t!(
                    "briefing.tel.duel",
                    who = who,
                    laps = r.laps_battled,
                    rival = r.pilot_name.as_str(),
                    gap = format!("{gap:.1}")
                )
                .to_string(),
            );
        }
    }

    // Melhor momento da corrida do jogador.
    if let Some(b) = &telemetry.best_moment {
        let phrase = match b.kind.as_str() {
            "rival_beaten" if !b.rival_name.trim().is_empty() => Some(
                rust_i18n::t!(
                    "briefing.tel.best_rival_beaten",
                    who = who,
                    rival = b.rival_name.as_str()
                )
                .to_string(),
            ),
            "position_gain" if b.positions_gained >= 1 => Some(
                rust_i18n::t!(
                    "briefing.tel.best_position_gain",
                    who = who,
                    n = b.positions_gained
                )
                .to_string(),
            ),
            "recovery" => {
                Some(rust_i18n::t!("briefing.tel.best_recovery", who = who).to_string())
            }
            "clean_streak" if b.streak >= 3 => Some(
                rust_i18n::t!("briefing.tel.best_clean_streak", who = who, n = b.streak)
                    .to_string(),
            ),
            _ => None,
        };
        if let Some(phrase) = phrase {
            out.push(phrase);
        }
    }

    // Erro mais caro (DNF não entra: o beat de Abandono já cobre).
    if let Some(m) = &telemetry.mistake {
        let phrase = match m.kind.as_str() {
            "incident" => Some(
                rust_i18n::t!(
                    "briefing.tel.mistake_incident",
                    who = who,
                    lap = m.lap,
                    n = m.positions_lost.max(0)
                )
                .to_string(),
            ),
            "position_loss" if m.positions_lost >= 1 => Some(
                rust_i18n::t!(
                    "briefing.tel.mistake_position_loss",
                    who = who,
                    n = m.positions_lost,
                    lap = m.lap
                )
                .to_string(),
            ),
            "pace_drop" if m.time_lost_ms >= 1500.0 => Some(
                rust_i18n::t!(
                    "briefing.tel.mistake_pace_drop",
                    who = who,
                    lap = m.lap,
                    secs = format!("{:.0}", m.time_lost_ms / 1000.0)
                )
                .to_string(),
            ),
            _ => None,
        };
        if let Some(phrase) = phrase {
            out.push(phrase);
        }
    }

    // ── Bandeira amarela REAL (SessionFlags do SDK) ──────────────────────────
    // `yellow_laps` são as voltas do LÍDER em que a corrida esteve sob amarela.
    // Voltas consecutivas viram UM acionamento: é assim que a corrida é vivida e
    // narrada, não como uma lista solta de voltas. Só corrida importada tem isto —
    // no sim offline a amarela não é modelada, então este bloco fica vazio.
    if let Some(charts) = &telemetry.charts {
        let mut yellow = charts.yellow_laps.clone();
        yellow.retain(|l| *l > 0);
        yellow.sort_unstable();
        yellow.dedup();
        if let Some(&first) = yellow.first() {
            let periods = 1 + yellow.windows(2).filter(|w| w[1] - w[0] > 1).count();
            let key = if periods > 1 {
                "briefing.tel.yellow_multi"
            } else {
                "briefing.tel.yellow_single"
            };
            out.push(
                rust_i18n::t!(key, periods = periods, laps = yellow.len(), first = first)
                    .to_string(),
            );
        }
    }

    out
}

fn persist_other_category_news(
    conn: &rusqlite::Connection,
    highlights: &[SimHighlight],
    season_number: i32,
) -> Result<(), String> {
    use crate::db::queries::news as news_queries;
    use crate::generators::ids::{next_ids, IdType};
    use crate::news::{NewsImportance, NewsItem, NewsType};

    if highlights.is_empty() {
        return Ok(());
    }

    let ids = next_ids(conn, IdType::News, highlights.len() as u32)
        .map_err(|e| format!("next_ids news: {e:?}"))?;
    let now = chrono::Local::now().timestamp();
    let items = highlights
        .iter()
        .zip(ids)
        .map(|(highlight, id)| NewsItem {
            id,
            tipo: NewsType::Corrida,
            icone: NewsType::Corrida.icone().to_string(),
            titulo: highlight.headline.clone(),
            texto: rust_i18n::t!(
                "race.news.other_categories_summary",
                headline = highlight.headline.as_str()
            )
            .to_string(),
            rodada: None,
            semana_pretemporada: None,
            temporada: season_number,
            categoria_id: Some(highlight.category.clone()),
            categoria_nome: get_category_config(&highlight.category)
                .map(|category| category.nome.to_string()),
            importancia: NewsImportance::Media,
            timestamp: now,
            driver_id: None,
            driver_id_secondary: None,
            team_id: None,
        })
        .collect::<Vec<_>>();

    news_queries::insert_news_batch(conn, &items)
        .map_err(|e| format!("insert_news_batch outras categorias: {e:?}"))?;
    Ok(())
}

#[cfg(test)]
#[path = "race/tests/mod.rs"]
mod tests;
