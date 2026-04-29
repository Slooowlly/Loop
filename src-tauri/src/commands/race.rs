use std::collections::{HashMap, HashSet};
use std::path::Path;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::commands::race_history::append_race_result;
use crate::config::app_config::{AppConfig, SaveMeta};
use crate::constants::categories::{get_all_categories, get_category_config, is_especial};
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
use crate::models::injury::Injury;
use crate::models::season::Season;
use crate::simulation::batch::{
    BriefRaceResult, CategorySimResult, SimHighlight, SimultaneousResults,
};
use crate::simulation::catalog::IncidentCatalog;
use crate::simulation::context::{SimDriver, SimulationContext};
use crate::simulation::engine::run_full_race;
use crate::simulation::incidents::IncidentResult;
use crate::simulation::race::RaceResult;
use crate::{calendar::CalendarEntry, models::team::Team};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceWeekendResult {
    pub player_race: RaceResult,
    pub other_categories: SimultaneousResults,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RacePersistenceMode {
    Playable,
    HistoricalDraft,
}

fn calculate_team_round_finance_context(
    team: &Team,
    added_points: i32,
    added_victories: i32,
    added_podiums: i32,
    best_result: i32,
    salary_expense: f64,
    rounds_in_season: f64,
    economic_health: GlobalEconomicHealth,
) -> TeamRoundFinanceContext {
    let income_modifier = economy_income_modifier(economic_health);
    let cost_modifier = economy_cost_modifier(economic_health);
    let scale = category_finance_scale(&team.categoria);
    let plan = calculate_financial_plan(team);
    let round_operating_base = scale.operating_cost_midpoint() / rounds_in_season.max(1.0);
    let sponsorship_income = (scale.expected_cash_midpoint() / rounds_in_season.max(1.0) * 0.16
        + team.reputacao * round_operating_base * 0.004
        + plan.budget_index * round_operating_base * 0.002)
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
    let technical_investment_cost = (round_operating_base * 0.16
        + team.car_performance.max(0.0) * round_operating_base * 0.015)
        * cost_modifier;
    let debt_service_cost = debt_service_for_state(team.debt_balance, &team.financial_state);

    TeamRoundFinanceContext {
        sponsorship_income,
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
    let race_entry = calendar_queries::get_calendar_entry_by_id(&db.conn, race_id)
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

    let (player_race, player_new_injuries) = simulate_category_race(&mut db, &race_entry, true)?;

    // Calcular repercussão pós-corrida e aplicar efeitos (fallback silencioso)
    // ID do jogador extraído para exclusão no bloco de world-facing media impact
    let excluded_driver_id = player_race
        .race_results
        .iter()
        .find(|r| r.is_jogador)
        .map(|r| r.pilot_id.clone())
        .unwrap_or_default();

    let (post_race_bias, ai_media_impacts, interest_tier) = if let Some(realized) =
        compute_post_race_impact(&db.conn, &race_entry, &player_race)
    {
        if let Ok(player) = driver_queries::get_player_driver(&db.conn) {
            let player_result = player_race.race_results.iter().find(|r| r.is_jogador);
            let base_midia_delta = if player_result.map_or(false, |r| r.finish_position == 1) {
                3.0
            } else if player_result.map_or(false, |r| r.finish_position <= 3 && !r.is_dnf) {
                2.0
            } else if player_result.map_or(false, |r| r.finish_position <= 5) {
                1.0
            } else if player_result.map_or(false, |r| r.is_dnf) {
                -2.0
            } else {
                -1.0
            };
            let new_midia = (player.atributos.midia
                + base_midia_delta * realized.media_delta_modifier as f64)
                .clamp(0.0, 100.0);
            let _ = driver_queries::update_driver_midia(&db.conn, &player.id, new_midia);

            let base_mot_delta = if player_result.map_or(false, |r| r.finish_position == 1) {
                4.0
            } else if player_result.map_or(false, |r| r.finish_position <= 3 && !r.is_dnf) {
                2.5
            } else if player_result.map_or(false, |r| r.finish_position <= 5) {
                1.0
            } else if player_result.map_or(false, |r| r.is_dnf) {
                -3.5
            } else {
                -0.5
            };
            let new_motivacao = (player.motivacao
                + base_mot_delta * realized.motivation_delta_modifier as f64)
                .clamp(0.0, 100.0);
            let _ = driver_queries::update_driver_motivation(&db.conn, &player.id, new_motivacao);
        }

        // World-facing media impact — pilotos AI relevantes.
        // Dependência semântica intencional: sem `realized`, este bloco não roda.
        // `excluded_driver_id` (jogador) omitido para evitar dupla aplicação com o pipeline player-facing.
        let main_incident_pilot: Option<String> = player_race
            .notable_incident_pilot_ids
            .iter()
            .find(|id| id.as_str() != excluded_driver_id.as_str())
            .cloned();

        // P2 e P3 elegíveis — winner explicitamente excluído (mutuidade Win/Podium)
        let podium_pilot_ids: Vec<&str> = player_race
            .race_results
            .iter()
            .filter(|r| {
                r.finish_position >= 2
                    && r.finish_position <= 3
                    && !r.is_dnf
                    && r.pilot_id != player_race.winner_id
            })
            .map(|r| r.pilot_id.as_str())
            .collect();

        let race_ctx = crate::event_interest::RaceEventContext {
            winner_id: &player_race.winner_id,
            pole_sitter_id: &player_race.pole_sitter_id,
            podium_ids: &podium_pilot_ids,
            main_incident_pilot_id: main_incident_pilot.as_deref(),
            excluded_driver_id: &excluded_driver_id,
        };

        // `player_new_injuries` contém lesões novas geradas na corrida (pode incluir pilotos AI).
        let impacts = crate::event_interest::compute_public_media_impacts(
            &race_ctx,
            &player_new_injuries,
            &realized,
        );

        (
            realized.news_importance_bias,
            impacts,
            realized.final_tier.clone(),
        )
    } else {
        (0, vec![], InterestTier::Baixo)
    };

    for impact in &ai_media_impacts {
        driver_queries::update_driver_midia_delta(&db.conn, &impact.driver_id, impact.delta)
            .map_err(|e| {
                format!(
                    "Falha ao aplicar impacto de mídia para '{}': {e}",
                    impact.driver_id
                )
            })?;
    }

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
    warn_if_side_effect_fails(
        persist_race_news(
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
        ),
        "Falha ao persistir noticias da corrida do jogador",
    );
    let other_categories = simulate_other_categories(
        &mut db,
        &career_dir,
        &race_entry.categoria,
        race_entry.week_of_year,
        &race_entry.display_date,
        &active_season.id,
        active_season.numero,
    )?;
    warn_if_side_effect_fails(
        update_last_played(&meta_path),
        "Falha ao atualizar meta.json apos a corrida",
    );

    Ok(RaceWeekendResult {
        player_race,
        other_categories,
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
            let (result, _) = simulate_category_race(&mut db, &entry, false)?;
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
    let mut teams = if is_especial(&race_entry.categoria) {
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
    let drivers = driver_queries::get_drivers_by_active_category(&db.conn, &race_entry.categoria)
        .map_err(|e| format!("Falha ao buscar pilotos da categoria: {e}"))?;

    let team_by_driver = if is_especial(&race_entry.categoria) {
        build_special_team_lookup(&db.conn, &teams, &race_entry.categoria)?
    } else {
        build_team_lookup(&teams)
    };
    let mut orphaned_drivers = Vec::new();
    let sim_drivers: Vec<SimDriver> = drivers
        .iter()
        .filter(|d| d.status != crate::models::enums::DriverStatus::Lesionado)
        .filter_map(|driver| match team_by_driver.get(&driver.id) {
            Some(team) => Some(SimDriver::from_driver_team_and_track(
                driver,
                team,
                race_entry.track_id,
            )),
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
    let mut result = run_full_race(
        &sim_drivers,
        &ctx,
        category.id == "endurance",
        &catalog,
        &mut rng,
    );
    if is_especial(&race_entry.categoria) {
        apply_special_class_scoring(&mut result, &teams, category.id == "endurance");
    }
    let next_round = if advance_player_round {
        Some((active_season.rodada_atual + 1).min(category.corridas_por_temporada as i32))
    } else {
        None
    };

    let mut new_injuries_out: Vec<Injury> = Vec::new();
    db.transaction(|tx| {
        // 1. Processo de recuperação das lesões já ativas
        crate::evolution::injury::process_injury_recovery(tx, &race_entry.categoria)?;

        // 2. Aplica pontuações normais
        let economic_health = global_economic_health_for_season(active_season.numero as i32);
        apply_race_result_to_database(
            tx,
            &result,
            &teams,
            economic_health,
            &race_entry.categoria,
            persistence_mode,
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
            &mut rng,
        )?;
        new_injuries_out = new_injuries;

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

        // 5. Processa hierarquia interna das equipes da categoria
        if !is_especial(&race_entry.categoria) {
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

        Ok(())
    })
    .map_err(|e| format!("Falha ao persistir resultado da corrida: {e}"))?;

    Ok((result, new_injuries_out))
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

        // Busca corridas pendentes com week_of_year <= target_week, em ordem cronológica.
        // Categorias especiais são excluídas naturalmente durante o BlocoRegular
        // até a abertura da janela especial de setembro, e incluídas no BlocoEspecial.
        let races_to_simulate = if is_last_player_race {
            calendar_queries::get_pending_races_up_to_week(&db.conn, season_id, category.id, 52)
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
            .into_iter()
            .filter(|entry| {
                !target_display_date.trim().is_empty() && entry.display_date == target_display_date
            })
            .collect()
        };

        if races_to_simulate.is_empty() {
            continue;
        }

        let mut summaries = Vec::new();
        for entry in races_to_simulate {
            let (result, _) = simulate_category_race(db, &entry, false)?;
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

            let is_visible_same_day =
                !target_display_date.trim().is_empty() && entry.display_date == target_display_date;
            if !is_visible_same_day {
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

fn apply_race_result_to_database(
    tx: &rusqlite::Transaction<'_>,
    result: &RaceResult,
    teams: &[Team],
    economic_health: GlobalEconomicHealth,
    race_category: &str,
    persistence_mode: RacePersistenceMode,
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
        driver.ultimos_resultados = append_recent_result(
            &driver.ultimos_resultados,
            race_driver.finish_position,
            race_driver.is_dnf,
        );

        driver_queries::update_driver(tx, &driver)?;
    }

    if is_especial(race_category) {
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
        let finance_context = calculate_team_round_finance_context(
            team,
            added_points,
            added_victories,
            added_podiums,
            best_result,
            salary_expense,
            rounds_in_season,
            economic_health,
        );

        let mut updated_team = team.clone();
        apply_round_cashflow(&mut updated_team, finance_context);
        apply_crisis_event_if_needed(&mut updated_team);
        refresh_team_financial_state(&mut updated_team);
        team_queries::update_team_finance_snapshot(tx, &updated_team)?;
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

fn build_team_lookup<'a>(
    teams: &'a [crate::models::team::Team],
) -> HashMap<String, &'a crate::models::team::Team> {
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

fn group_results_by_team<'a>(
    result: &'a RaceResult,
) -> HashMap<String, Vec<&'a crate::simulation::race::RaceDriverResult>> {
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

    if active_season.fase == crate::models::enums::SeasonPhase::BlocoEspecial {
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

    if active_season.fase == crate::models::enums::SeasonPhase::BlocoEspecial {
        if let Some(category) = player.categoria_especial_ativa {
            return Ok(Some(category));
        }
    }

    Ok(player.categoria_atual)
}

fn get_next_player_race(
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
) -> Result<(), String> {
    use crate::db::queries::news as news_queries;
    use crate::generators::ids::{next_id, IdType};
    use crate::news::{NewsImportance, NewsItem, NewsType};

    use crate::db::queries::drivers as driver_queries;

    let now = chrono::Local::now().timestamp();
    let mut items: Vec<NewsItem> = Vec::new();

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

        let (titulo, texto) = if fallback_races == 0 {
            (
                format!("{} vence a corrida final da temporada em {}", winner_name, race_result.track_name),
                format!("{} cruzou a linha de chegada em primeiro lugar na última rodada da temporada {}.", winner_name, active_season.numero),
            )
        } else if fallback_races <= 2 {
            (
                format!("{} conquista vitória crucial na reta final em {}", winner_name, race_result.track_name),
                format!("Com a temporada se aproximando do fim, {} garantiu o primeiro lugar na rodada {}.", winner_name, round),
            )
        } else {
            (
                format!("{} vence em {}", winner_name, race_result.track_name),
                format!(
                    "{} cruzou a linha de chegada em primeiro lugar na rodada {} da temporada {}.",
                    winner_name, round, active_season.numero
                ),
            )
        };

        let winner_team = race_result
            .race_results
            .iter()
            .find(|r| &r.pilot_id == winner_id)
            .map(|r| r.team_id.clone());
        let id = next_id(conn, IdType::News).map_err(|e| format!("next_id news: {e:?}"))?;
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
                        titulo: format!("{} é o grande campeão da temporada {}!", champion.pilot_name, active_season.numero),
                        texto: format!("Após {} rodadas intensas, {} conquista o título da categoria. Uma temporada inesquecível chegou ao fim.", total_rodadas, champion.pilot_name),
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

    Ok(())
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
            texto: format!("Resumo das outras categorias: {}.", highlight.headline),
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
mod tests {
    use std::fs;

    use super::*;
    use crate::commands::career::{create_career_in_base_dir, CreateCareerInput};
    use crate::db::queries::calendar::get_next_race;
    use crate::db::queries::news as news_queries;
    use crate::models::team::placeholder_team_from_db;
    use crate::simulation::race::{ClassificationStatus, RaceDriverResult};

    #[test]
    fn round_finance_context_uses_real_money_instead_of_raw_budget() {
        let mut rich = placeholder_team_from_db(
            "TRICH".to_string(),
            "Rich Team".to_string(),
            "gt4".to_string(),
            "2026-01-01".to_string(),
        );
        rich.cash_balance = 7_000_000.0;
        rich.debt_balance = 0.0;
        rich.budget = 1.0;
        rich.reputacao = 75.0;
        rich.financial_state = "healthy".to_string();

        let mut poor = rich.clone();
        poor.id = "TPOOR".to_string();
        poor.cash_balance = 150_000.0;
        poor.debt_balance = 3_000_000.0;
        poor.budget = 99.0;
        poor.reputacao = 40.0;
        poor.financial_state = "crisis".to_string();

        let rich_context = calculate_team_round_finance_context(
            &rich,
            4,
            0,
            0,
            8,
            35_000.0,
            8.0,
            GlobalEconomicHealth::Neutral,
        );
        let poor_context = calculate_team_round_finance_context(
            &poor,
            4,
            0,
            0,
            8,
            35_000.0,
            8.0,
            GlobalEconomicHealth::Neutral,
        );

        assert!(rich_context.sponsorship_income > poor_context.sponsorship_income);
        assert!(poor_context.debt_service_cost > rich_context.debt_service_cost);
    }

    fn sample_driver_result(
        pilot_id: &str,
        team_id: &str,
        finish_position: i32,
    ) -> RaceDriverResult {
        RaceDriverResult {
            pilot_id: pilot_id.to_string(),
            pilot_name: pilot_id.to_string(),
            team_id: team_id.to_string(),
            team_name: team_id.to_string(),
            grid_position: finish_position,
            finish_position,
            positions_gained: 0,
            best_lap_time_ms: 90_000.0 + finish_position as f64,
            total_race_time_ms: 900_000.0 + finish_position as f64,
            gap_to_winner_ms: if finish_position == 1 {
                0.0
            } else {
                finish_position as f64
            },
            is_dnf: false,
            dnf_reason: None,
            dnf_segment: None,
            incidents_count: 0,
            incidents: Vec::new(),
            has_fastest_lap: false,
            points_earned: 0,
            is_jogador: false,
            laps_completed: 20,
            final_tire_wear: 0.5,
            final_physical: 0.8,
            classification_status: ClassificationStatus::Finished,
            notable_incident: None,
            dnf_catalog_id: None,
            damage_origin_segment: None,
        }
    }

    fn sample_special_team(team_id: &str, class_name: &str) -> Team {
        let mut team = placeholder_team_from_db(
            team_id.to_string(),
            team_id.to_string(),
            "endurance".to_string(),
            "2026-01-01".to_string(),
        );
        team.classe = Some(class_name.to_string());
        team
    }

    #[test]
    fn special_results_are_scored_by_class_position() {
        let teams = vec![
            sample_special_team("LMP-A", "lmp2"),
            sample_special_team("LMP-B", "lmp2"),
            sample_special_team("GT3-A", "gt3"),
            sample_special_team("GT4-A", "gt4"),
        ];
        let mut result = RaceResult {
            qualifying_results: Vec::new(),
            race_results: vec![
                sample_driver_result("P-LMP-A", "LMP-A", 1),
                sample_driver_result("P-GT3-A", "GT3-A", 2),
                sample_driver_result("P-GT4-A", "GT4-A", 3),
                sample_driver_result("P-LMP-B", "LMP-B", 4),
            ],
            pole_sitter_id: "P-LMP-A".to_string(),
            winner_id: "P-LMP-A".to_string(),
            fastest_lap_id: String::new(),
            total_laps: 20,
            weather: "dry".to_string(),
            track_name: "Test".to_string(),
            total_incidents: 0,
            total_dnfs: 0,
            main_incident_count: 0,
            notable_incident_pilot_ids: Vec::new(),
            most_positions_gained_id: None,
        };

        apply_special_class_scoring(&mut result, &teams, true);

        let by_pilot: std::collections::HashMap<_, _> = result
            .race_results
            .iter()
            .map(|entry| (entry.pilot_id.as_str(), entry))
            .collect();

        assert_eq!(by_pilot["P-LMP-A"].finish_position, 1);
        assert_eq!(by_pilot["P-LMP-A"].points_earned, 35);
        assert_eq!(by_pilot["P-LMP-B"].finish_position, 2);
        assert_eq!(by_pilot["P-LMP-B"].points_earned, 28);
        assert_eq!(by_pilot["P-GT3-A"].finish_position, 1);
        assert_eq!(by_pilot["P-GT3-A"].points_earned, 35);
        assert_eq!(by_pilot["P-GT4-A"].finish_position, 1);
        assert_eq!(by_pilot["P-GT4-A"].points_earned, 35);
    }

    #[test]
    fn test_simulate_race_weekend_updates_state() {
        let base_dir = unique_test_dir("simulate_weekend");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");

        let result = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        assert_eq!(result.player_race.race_results.len(), 12);
        assert_eq!(result.other_categories.total_races_simulated, 0);

        let updated_db = Database::open_existing(&db_path).expect("reopen db");
        let season_after = season_queries::get_active_season(&updated_db.conn)
            .expect("season after")
            .expect("active season after");
        assert_eq!(season_after.rodada_atual, 2);

        let completed = calendar_queries::get_calendar_entry_by_id(&updated_db.conn, &next_race.id)
            .expect("race by id")
            .expect("calendar entry");
        assert_eq!(completed.status.as_str(), "Concluida");

        let driver = driver_queries::get_player_driver(&updated_db.conn).expect("player driver");
        assert!(driver.stats_temporada.corridas >= 1);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_updates_team_finance_snapshot() {
        let base_dir = unique_test_dir("simulate_team_finance");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let contract =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
                .expect("active contract")
                .expect("player contract");
        let team_before = team_queries::get_team_by_id(&db.conn, &contract.equipe_id)
            .expect("team before")
            .expect("existing team before");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        drop(db);

        simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let team_after = team_queries::get_team_by_id(&updated_db.conn, &contract.equipe_id)
            .expect("team after")
            .expect("existing team after");

        assert_ne!(team_after.cash_balance, team_before.cash_balance);
        assert!(
            team_after.last_round_income > 0.0,
            "team should record round income"
        );
        assert!(
            team_after.last_round_expenses > 0.0,
            "team should record round expenses"
        );
        assert_eq!(
            team_after.last_round_net,
            team_after.last_round_income - team_after.last_round_expenses
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_applies_crisis_finance_event() {
        let base_dir = unique_test_dir("simulate_crisis_finance");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let contract =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
                .expect("active contract")
                .expect("player contract");
        let mut team = team_queries::get_team_by_id(&db.conn, &contract.equipe_id)
            .expect("team before")
            .expect("existing team before");
        team.cash_balance = -100_000.0;
        team.debt_balance = 850_000.0;
        team.financial_state = "collapse".to_string();
        team_queries::update_team(&db.conn, &team).expect("update crisis team");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        drop(db);

        simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let team_after = team_queries::get_team_by_id(&updated_db.conn, &contract.equipe_id)
            .expect("team after")
            .expect("existing team after");

        assert!(team_after.cash_balance > -100_000.0);
        assert!(team_after.debt_balance > 850_000.0);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_rejects_completed_race() {
        let base_dir = unique_test_dir("simulate_completed");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");

        simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("first simulation");
        let error = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect_err("second simulation should fail");

        assert!(
            error.contains("ja foi concluida ou simulada"),
            "Erro inesperado: {}",
            error
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_rejects_out_of_order_race() {
        let base_dir = unique_test_dir("simulate_wrong_order");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let schedule =
            calendar_queries::get_calendar(&db.conn, &season.id, "mazda_rookie").expect("schedule");
        let later_race = schedule
            .into_iter()
            .find(|entry| entry.rodada == 2)
            .expect("round 2 race");

        let error = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &later_race.id)
            .expect_err("out of order race should fail");

        assert!(
            error.contains("proxima corrida valida"),
            "erro inesperado: {}",
            error
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_rejects_other_category_race() {
        let base_dir = unique_test_dir("simulate_wrong_category");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let other_category_race = get_next_race(&db.conn, &season.id, "gt3")
            .expect("next gt3 race")
            .expect("pending gt3 race");

        let error =
            simulate_race_weekend_in_base_dir(&base_dir, "career_001", &other_category_race.id)
                .expect_err("other category race should fail");

        assert!(
            error.contains("proxima corrida valida"),
            "erro inesperado: {}",
            error
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_rejects_active_driver_without_team() {
        let base_dir = unique_test_dir("simulate_orphan_driver");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let mut db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        let player = driver_queries::get_player_driver(&db.conn).expect("player driver");
        let player_team = team_queries::get_teams_by_category(&db.conn, "mazda_rookie")
            .expect("teams")
            .into_iter()
            .find(|team| {
                team.piloto_1_id.as_deref() == Some(player.id.as_str())
                    || team.piloto_2_id.as_deref() == Some(player.id.as_str())
            })
            .expect("player team");
        team_queries::remove_pilot_from_team(&db.conn, &player.id, &player_team.id)
            .expect("remove player from team");

        let error = simulate_category_race(&mut db, &next_race, true)
            .expect_err("active driver without team should fail");
        assert!(
            error.contains("Pilotos ativos sem equipe"),
            "erro inesperado: {}",
            error
        );
        assert!(
            error.contains(&player.id),
            "mensagem deveria apontar piloto orfao: {}",
            error
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_persists_news() {
        let base_dir = unique_test_dir("simulate_news");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        align_next_race_display_date(&db.conn, &season.id, "gt3", &next_race.display_date);
        news_queries::delete_all_news(&db.conn).expect("clear news");
        drop(db);

        simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let news = news_queries::get_recent_news(&updated_db.conn, 50).expect("recent news");
        assert!(
            news.iter()
                .any(|item| item.categoria_id.as_deref() == Some("mazda_rookie")),
            "deveria existir noticia da corrida do jogador"
        );
        assert!(
            news.iter()
                .any(|item| item.categoria_id.as_deref() == Some("gt3")),
            "deveria existir noticia de outra categoria simulada"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_ignores_invalid_meta_after_persisting_race() {
        let base_dir = unique_test_dir("simulate_invalid_meta");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let career_dir = config.saves_dir().join("career_001");
        let db_path = career_dir.join("career.db");
        let meta_path = career_dir.join("meta.json");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        drop(db);

        fs::write(&meta_path, "{meta invalida").expect("corrupt meta");

        let result = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id);
        assert!(
            result.is_ok(),
            "simulacao nao deveria falhar por meta invalida"
        );

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let completed = calendar_queries::get_calendar_entry_by_id(&updated_db.conn, &next_race.id)
            .expect("race by id")
            .expect("calendar entry");
        assert_eq!(completed.status.as_str(), "Concluida");

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_returns_other_results() {
        let base_dir = unique_test_dir("simulate_other_results");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        align_next_race_display_date(&db.conn, &season.id, "gt3", &next_race.display_date);

        let result = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        assert_eq!(result.player_race.track_name, next_race.track_name);
        assert_eq!(result.other_categories.total_races_simulated, 1);
        assert!(result
            .other_categories
            .categories_simulated
            .iter()
            .all(|category| category.category_id != "mazda_rookie"));
        assert_eq!(
            result
                .other_categories
                .categories_simulated
                .iter()
                .map(|category| category.category_id.as_str())
                .collect::<Vec<_>>(),
            vec!["gt3"]
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_only_processes_same_day_categories() {
        let base_dir = unique_test_dir("simulate_same_day_only");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        let toyota_race = get_next_race(&db.conn, &season.id, "toyota_rookie")
            .expect("toyota race")
            .expect("pending toyota race");
        let gt3_race = get_next_race(&db.conn, &season.id, "gt3")
            .expect("gt3 race")
            .expect("pending gt3 race");
        db.conn
            .execute(
                "UPDATE calendar SET data = ?1 WHERE id = ?2",
                rusqlite::params![next_race.display_date, toyota_race.id],
            )
            .expect("align toyota to same day");
        drop(db);

        let result = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        assert_eq!(result.other_categories.total_races_simulated, 1);
        assert_eq!(
            result
                .other_categories
                .categories_simulated
                .iter()
                .map(|category| category.category_id.as_str())
                .collect::<Vec<_>>(),
            vec!["toyota_rookie"]
        );

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let toyota_after =
            calendar_queries::get_calendar_entry_by_id(&updated_db.conn, &toyota_race.id)
                .expect("toyota by id")
                .expect("toyota entry");
        let gt3_after = calendar_queries::get_calendar_entry_by_id(&updated_db.conn, &gt3_race.id)
            .expect("gt3 by id")
            .expect("gt3 entry");

        assert_eq!(toyota_after.status.as_str(), "Concluida");
        assert_eq!(gt3_after.status.as_str(), "Pendente");

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_all_categories_complete_after_last_race() {
        let base_dir = unique_test_dir("simulate_all_categories");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");

        loop {
            let db = Database::open_existing(&db_path).expect("db");
            let season = season_queries::get_active_season(&db.conn)
                .expect("season")
                .expect("active season");
            let Some(next_race) =
                get_next_race(&db.conn, &season.id, "mazda_rookie").expect("next race")
            else {
                break;
            };
            drop(db);

            simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
                .expect("simulate round");
        }

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let season = season_queries::get_active_season(&updated_db.conn)
            .expect("season")
            .expect("active season");
        let pending =
            calendar_queries::get_pending_races(&updated_db.conn, &season.id).expect("pending");

        assert!(pending.is_empty(), "all categories should be complete");

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_last_player_race_drains_pending_races_without_showing_off_day_results() {
        let base_dir = unique_test_dir("simulate_last_race_hidden_drain");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let player_schedule = calendar_queries::get_calendar(&db.conn, &season.id, "mazda_rookie")
            .expect("player calendar");
        let last_player_race = player_schedule.last().expect("last player race").clone();

        for entry in player_schedule
            .iter()
            .filter(|entry| entry.id != last_player_race.id)
        {
            calendar_queries::mark_race_completed(&db.conn, &entry.id)
                .expect("complete previous player race");
        }

        db.conn
            .execute(
                "UPDATE calendar
                 SET data = '2024-02-01'
                 WHERE categoria != 'mazda_rookie'
                   AND status = 'Pendente'",
                [],
            )
            .expect("move other categories away from player race day");

        let pending_before = calendar_queries::get_pending_races(&db.conn, &season.id)
            .expect("pending before")
            .into_iter()
            .filter(|entry| entry.categoria != "mazda_rookie")
            .count();
        assert!(
            pending_before > 0,
            "test setup should leave other categories pending"
        );
        drop(db);

        let result =
            simulate_race_weekend_in_base_dir(&base_dir, "career_001", &last_player_race.id)
                .expect("simulate last player race");

        assert_eq!(result.other_categories.total_races_simulated, 0);
        assert!(result.other_categories.categories_simulated.is_empty());
        assert!(result.other_categories.highlights.is_empty());

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let pending_after =
            calendar_queries::get_pending_races(&updated_db.conn, &season.id).expect("pending");
        assert!(
            pending_after.is_empty(),
            "hidden drain should still complete the season calendar"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_stats_updated_for_other_categories() {
        let base_dir = unique_test_dir("simulate_other_stats");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        align_next_race_display_date(&db.conn, &season.id, "gt3", &next_race.display_date);
        drop(db);

        simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        let updated_db = Database::open_existing(&db_path).expect("updated db");
        let gt3_driver = driver_queries::get_drivers_by_category(&updated_db.conn, "gt3")
            .expect("gt3 drivers")
            .into_iter()
            .next()
            .expect("at least one gt3 driver");

        assert!(
            gt3_driver.stats_temporada.corridas > 0,
            "other categories should update driver stats"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_race_history_saved_for_other_categories() {
        let base_dir = unique_test_dir("simulate_other_history");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let career_dir = config.saves_dir().join("career_001");
        let db_path = career_dir.join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");
        align_next_race_display_date(&db.conn, &season.id, "gt3", &next_race.display_date);
        drop(db);

        simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
            .expect("simulate");

        let history_path = career_dir.join("race_results.json");
        let history =
            fs::read_to_string(history_path).expect("race history should be written to disk");

        assert!(history.contains("\"mazda_rookie\""));
        assert!(history.contains("\"gt3\""));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_race_weekend_uses_active_special_category_for_player() {
        let base_dir = unique_test_dir("simulate_special_player");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let mut player = driver_queries::get_player_driver(&db.conn).expect("player");
        player.categoria_atual = Some("gt4".to_string());
        player.atributos.skill = 98.0;
        driver_queries::update_driver(&db.conn, &player).expect("update player");

        mark_regular_races_completed(&db);
        crate::convocation::advance_to_convocation_window(&db.conn).expect("advance convocation");
        crate::convocation::run_convocation_window(&db.conn).expect("run convocation");
        let offers = crate::commands::convocation::get_player_special_offers_in_base_dir(
            &base_dir,
            "career_001",
        )
        .expect("special offers");
        crate::commands::convocation::respond_player_special_offer_in_base_dir(
            &base_dir,
            "career_001",
            &offers[0].id,
            true,
        )
        .expect("accept special offer");

        let db_after_accept = Database::open_existing(&db_path).expect("db after accept");
        let accepted_contract =
            crate::db::queries::contracts::get_active_especial_contract_for_pilot(
                &db_after_accept.conn,
                &player.id,
            )
            .expect("special contract query")
            .expect("player should have active special contract");
        assert_eq!(accepted_contract.categoria, "endurance");
        assert!(
            !accepted_contract.equipe_id.is_empty(),
            "o contrato especial deve apontar para a equipe regular classificada"
        );

        crate::convocation::iniciar_bloco_especial(&db_after_accept.conn)
            .expect("start special block");
        let season = season_queries::get_active_season(&db_after_accept.conn)
            .expect("season")
            .expect("active season");
        let endurance_teams = special_entry_queries::get_entry_teams_for_category(
            &db_after_accept.conn,
            &season.id,
            "endurance",
        )
        .expect("endurance entry teams");
        let endurance_drivers =
            driver_queries::get_drivers_by_active_category(&db_after_accept.conn, "endurance")
                .expect("endurance drivers");
        let endurance_lookup =
            build_special_team_lookup(&db_after_accept.conn, &endurance_teams, "endurance")
                .expect("special team lookup");
        let missing_before_sim = endurance_drivers
            .iter()
            .filter(|driver| !endurance_lookup.contains_key(&driver.id))
            .map(|driver| format!("{} ({})", driver.nome, driver.id))
            .collect::<Vec<_>>();
        assert!(
            endurance_drivers
                .iter()
                .any(|driver| driver.id == player.id),
            "o jogador deveria aparecer entre os pilotos ativos de endurance"
        );
        assert!(
            endurance_lookup.contains_key(&player.id),
            "o lookup de equipes de endurance deveria conter o jogador antes da simulacao"
        );
        assert!(
            missing_before_sim.is_empty(),
            "todos os pilotos ativos de endurance deveriam ter equipe antes da simulacao: {:?}",
            missing_before_sim
        );
        let next_special_race = get_next_race(&db_after_accept.conn, &season.id, "endurance")
            .expect("next special race")
            .expect("pending special race");
        drop(db_after_accept);

        let result =
            simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_special_race.id)
                .expect("simulate special race");

        assert!(
            result.player_race.race_results.iter().any(|entry| entry.is_jogador),
            "o grid especial deveria incluir o jogador quando categoria_especial_ativa estiver ativa"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_special_block_fast_forwards_when_player_stays_out() {
        let base_dir = unique_test_dir("simulate_special_block_skip");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let mut player = driver_queries::get_player_driver(&db.conn).expect("player");
        player.categoria_atual = Some("gt4".to_string());
        player.atributos.skill = 98.0;
        driver_queries::update_driver(&db.conn, &player).expect("update player");

        mark_regular_races_completed(&db);
        crate::convocation::advance_to_convocation_window(&db.conn).expect("advance convocation");
        crate::convocation::run_convocation_window(&db.conn).expect("run convocation");
        crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");
        drop(db);

        let result =
            simulate_special_block_in_base_dir(&base_dir, "career_001").expect("fast sim special");

        assert_eq!(result.total_races_simulated, 16);

        let refreshed_db = Database::open_existing(&db_path).expect("refreshed db");
        let season = season_queries::get_active_season(&refreshed_db.conn)
            .expect("season")
            .expect("active season");
        let pending_specials = calendar_queries::get_pending_races_for_category(
            &refreshed_db.conn,
            &season.id,
            "production_challenger",
        )
        .expect("production pending")
        .len()
            + calendar_queries::get_pending_races_for_category(
                &refreshed_db.conn,
                &season.id,
                "endurance",
            )
            .expect("endurance pending")
            .len();
        assert_eq!(
            pending_specials, 0,
            "nao deveria restar corrida especial pendente"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_simulate_special_block_rejects_player_inside_special_grid() {
        let base_dir = unique_test_dir("simulate_special_block_player_inside");
        fs::create_dir_all(&base_dir).expect("base dir");

        create_career_in_base_dir(
            &base_dir,
            CreateCareerInput {
                player_name: "Joao Silva".to_string(),
                player_nationality: "br".to_string(),
                player_age: Some(20),
                category: "mazda_rookie".to_string(),
                team_index: 0,
                difficulty: "medio".to_string(),
            },
        )
        .expect("career");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let mut player = driver_queries::get_player_driver(&db.conn).expect("player");
        player.categoria_atual = Some("gt4".to_string());
        player.atributos.skill = 98.0;
        driver_queries::update_driver(&db.conn, &player).expect("update player");

        mark_regular_races_completed(&db);
        crate::convocation::advance_to_convocation_window(&db.conn).expect("advance convocation");
        crate::convocation::run_convocation_window(&db.conn).expect("run convocation");
        let offers = crate::commands::convocation::get_player_special_offers_in_base_dir(
            &base_dir,
            "career_001",
        )
        .expect("special offers");
        crate::commands::convocation::respond_player_special_offer_in_base_dir(
            &base_dir,
            "career_001",
            &offers[0].id,
            true,
        )
        .expect("accept special offer");
        crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");
        drop(db);

        let error = simulate_special_block_in_base_dir(&base_dir, "career_001")
            .expect_err("player should not skip entered special block");
        assert!(error.contains("deve correr essa fase normalmente"));

        let _ = fs::remove_dir_all(base_dir);
    }

    fn unique_test_dir(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("iracerapp_{label}_{nanos}"))
    }

    fn mark_regular_races_completed(db: &Database) {
        db.conn
            .execute(
                "UPDATE calendar SET status = 'Concluida' WHERE season_phase = 'BlocoRegular'",
                [],
            )
            .expect("complete regular block");
    }

    fn align_next_race_display_date(
        conn: &rusqlite::Connection,
        season_id: &str,
        category_id: &str,
        display_date: &str,
    ) {
        let race = get_next_race(conn, season_id, category_id)
            .expect("next race for category")
            .expect("pending race for category");
        conn.execute(
            "UPDATE calendar SET data = ?1 WHERE id = ?2",
            rusqlite::params![display_date, race.id],
        )
        .expect("align next race display date");
    }
}
