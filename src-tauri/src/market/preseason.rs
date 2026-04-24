use std::collections::HashSet;
use std::path::Path;
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{Duration, Local, NaiveDate};
use rand::Rng;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::teams as team_queries;
use crate::finance::cashflow::apply_offseason_competitiveness_impact;
use crate::finance::planning::{category_finance_scale, derive_budget_index_from_money};
use crate::finance::state::{choose_season_strategy, refresh_team_financial_state};
use crate::generators::ids::{next_id, IdType};
use crate::market::car_build_strategy::choose_car_build_profile;
use crate::market::pipeline::run_market;
use crate::market::pit_strategy::{
    recalculate_pit_crew_quality, recalculate_pit_strategy_risk, PreviousTeamStanding,
};
use crate::market::proposals::{is_real_career_debut_category, MarketProposal, ProposalStatus};
use crate::market::sync::sync_team_slots_from_active_regular_contracts;
use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::enums::{ContractStatus, DriverStatus, SeasonPhase, TeamRole};
use crate::models::license::{
    ensure_driver_can_join_category, grant_driver_license_for_category_if_needed,
    repair_missing_licenses_for_current_categories,
};
use crate::simulation::car_build::profile_money_cost;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PreSeasonPhase {
    ContractExpiry,
    Transfers,
    PlayerProposals,
    RookiePlacement,
    Finalization,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreSeasonState {
    pub season_number: i32,
    pub current_week: i32,
    pub total_weeks: i32,
    pub phase: PreSeasonPhase,
    pub is_complete: bool,
    pub player_has_pending_proposals: bool,
    /// Verdadeiro se o jogador já tem um contrato regular ativo para esta temporada.
    #[serde(default)]
    pub player_has_team: bool,
    #[serde(default)]
    pub current_display_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MarketEventType {
    ContractExpired,
    ContractRenewed,
    TransferCompleted,
    TransferRejected,
    RookieSigned,
    PlayerProposalReceived,
    HierarchyUpdated,
    PreSeasonComplete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEvent {
    pub event_type: MarketEventType,
    pub headline: String,
    pub description: String,
    pub driver_id: Option<String>,
    pub driver_name: Option<String>,
    pub team_id: Option<String>,
    pub team_name: Option<String>,
    pub from_team: Option<String>,
    pub to_team: Option<String>,
    pub categoria: Option<String>,
    #[serde(default)]
    pub from_categoria: Option<String>,
    #[serde(default)]
    pub movement_kind: Option<String>,
    #[serde(default)]
    pub championship_position: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekResult {
    pub week_number: i32,
    pub phase: PreSeasonPhase,
    pub events: Vec<MarketEvent>,
    pub is_last_week: bool,
    pub player_proposals: Vec<MarketProposal>,
    pub remaining_vacancies: i32,
    pub next_phase: PreSeasonPhase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreSeasonPlan {
    pub state: PreSeasonState,
    pub planned_events: Vec<PlannedEvent>,
    pub executed_weeks: Vec<WeekResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedEvent {
    pub week: i32,
    pub event: PendingAction,
    pub executed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PendingAction {
    PhaseMarker {
        phase: PreSeasonPhase,
    },
    ExpireContract {
        contract_id: String,
        driver_id: String,
        driver_name: String,
        team_id: String,
        team_name: String,
    },
    RenewContract {
        driver_id: String,
        driver_name: String,
        team_id: String,
        team_name: String,
        new_salary: f64,
        new_duration: i32,
        new_role: String,
    },
    Transfer {
        driver_id: String,
        driver_name: String,
        from_team_id: Option<String>,
        from_team_name: Option<String>,
        #[serde(default)]
        from_categoria: Option<String>,
        to_team_id: String,
        to_team_name: String,
        salary: f64,
        duration: i32,
        role: String,
    },
    PlayerProposal {
        proposal: MarketProposal,
    },
    PlaceRookie {
        driver: Driver,
        team_id: String,
        team_name: String,
        salary: f64,
        duration: i32,
        role: String,
    },
    UpdateHierarchy {
        team_id: String,
        team_name: String,
        n1_id: Option<String>,
        n1_name: String,
        n2_id: Option<String>,
        n2_name: String,
        // Estado hierárquico anterior (capturado no início da preseason).
        // #[serde(default)] para compatibilidade com saves anteriores que não têm esses campos.
        #[serde(default)]
        prev_n1_id: Option<String>,
        #[serde(default)]
        prev_n2_id: Option<String>,
        #[serde(default)]
        prev_tensao: f64,
        #[serde(default = "default_estavel")]
        prev_status: String,
        #[serde(default)]
        prev_categoria: String,
    },
}

fn default_estavel() -> String {
    "estavel".to_string()
}

#[cfg(test)]
static PRESEASON_CLONE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
struct TempPreseasonClone {
    path: std::path::PathBuf,
    conn: Option<Connection>,
}

#[cfg(test)]
impl TempPreseasonClone {
    fn new(source: &Connection) -> Result<Self, String> {
        let path = clone_connection_to_temp(source)?;
        let conn = Connection::open(&path)
            .map_err(|e| format!("Falha ao abrir clone temporario do banco: {e}"))?;
        Ok(Self {
            path,
            conn: Some(conn),
        })
    }

    fn connection(&self) -> &Connection {
        self.conn
            .as_ref()
            .expect("clone temporario da preseason ja foi liberado")
    }

    #[cfg(test)]
    fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
impl Drop for TempPreseasonClone {
    fn drop(&mut self) {
        let _ = self.conn.take();
        if let Err(err) = cleanup_temp_db(&self.path) {
            eprintln!("Falha ao limpar clone temporario da preseason: {err}");
        }
    }
}

pub fn initialize_preseason(
    conn: &Connection,
    season_number: i32,
    rng: &mut impl Rng,
) -> Result<PreSeasonPlan, String> {
    let season_id = get_season_id_by_number(conn, season_number)?
        .ok_or_else(|| format!("Temporada {season_number} nao encontrada"))?;
    reset_market_state(conn, &season_id, &PreSeasonPhase::ContractExpiry)?;
    repair_missing_licenses_for_current_categories(conn)?;
    assign_seasonal_team_attributes(conn, season_number, &season_id)?;

    let original_contracts = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao carregar contratos atuais: {e}"))?;
    let original_teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao carregar equipes: {e}"))?;
    let _original_drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos atuais: {e}"))?;
    let original_contracts_by_driver = original_contracts
        .iter()
        .cloned()
        .map(|contract| (contract.piloto_id.clone(), contract))
        .collect::<std::collections::HashMap<_, _>>();

    let (
        market_report,
        temp_teams,
        temp_drivers,
        simulated_contracts_by_driver,
        renewal_events,
        mut planned_events,
        transfer_events,
    ) = {
        conn.execute_batch("SAVEPOINT preseason_plan_simulation")
            .map_err(|e| format!("Falha ao iniciar savepoint do plano da pre-temporada: {e}"))?;
        let simulation_result = (|| -> Result<_, String> {
            let market_report = run_market(conn, season_number, rng)
                .map_err(|e| format!("Falha ao simular mercado para o plano: {e}"))?;
            let simulated_contracts = contract_queries::get_all_active_regular_contracts(conn)
                .map_err(|e| format!("Falha ao carregar contratos simulados: {e}"))?;
            let simulated_contracts_by_driver = simulated_contracts
                .into_iter()
                .map(|contract| (contract.piloto_id.clone(), contract))
                .collect::<std::collections::HashMap<_, _>>();
            let temp_teams = team_queries::get_all_teams(conn)
                .map_err(|e| format!("Falha ao carregar equipes simuladas: {e}"))?;
            let temp_drivers = driver_queries::get_all_drivers(conn)
                .map_err(|e| format!("Falha ao carregar pilotos simulados: {e}"))?;
            let renewal_events =
                build_renewal_events(&simulated_contracts_by_driver, &market_report.new_signings)?;
            let renewed_driver_ids: HashSet<String> = renewal_events
                .iter()
                .filter_map(|event| match &event.event {
                    PendingAction::RenewContract { driver_id, .. } => Some(driver_id.clone()),
                    _ => None,
                })
                .collect();
            let planned_events =
                build_expiry_events(conn, &original_contracts, &renewed_driver_ids)?;
            let transfer_events = build_transfer_events(
                &simulated_contracts_by_driver,
                &market_report.new_signings,
                &original_contracts_by_driver,
            )?;
            Ok((
                market_report,
                temp_teams,
                temp_drivers,
                simulated_contracts_by_driver,
                renewal_events,
                planned_events,
                transfer_events,
            ))
        })();
        let rollback_result =
            conn.execute_batch("ROLLBACK TO SAVEPOINT preseason_plan_simulation; RELEASE SAVEPOINT preseason_plan_simulation;");
        if let Err(e) = rollback_result {
            return Err(format!(
                "Falha ao reverter simulacao temporaria da pre-temporada: {e}"
            ));
        }
        simulation_result?
    };

    let original_teams_by_id = original_teams
        .iter()
        .cloned()
        .map(|team| (team.id.clone(), team))
        .collect::<std::collections::HashMap<_, _>>();
    let temp_drivers_by_id = temp_drivers
        .iter()
        .cloned()
        .map(|driver| (driver.id.clone(), driver))
        .collect::<std::collections::HashMap<_, _>>();

    apply_preseason_entry_contract_state(conn, season_number)?;
    apply_preseason_renewal_state(conn, season_number, &renewal_events)?;

    planned_events.extend(renewal_events);
    if !planned_events.iter().any(|event| {
        event.week == 2 && phase_for_action(&event.event) == PreSeasonPhase::ContractExpiry
    }) {
        planned_events.push(PlannedEvent {
            week: 2,
            event: PendingAction::PhaseMarker {
                phase: PreSeasonPhase::ContractExpiry,
            },
            executed: false,
        });
    }

    if transfer_events.is_empty() {
        planned_events.push(PlannedEvent {
            week: 3,
            event: PendingAction::PhaseMarker {
                phase: PreSeasonPhase::Transfers,
            },
            executed: false,
        });
    } else {
        planned_events.extend(transfer_events);
    }

    let mut current_week = planned_events
        .iter()
        .map(|event| event.week)
        .max()
        .unwrap_or(2)
        + 1;
    if market_report.player_proposals.is_empty() {
        planned_events.push(PlannedEvent {
            week: current_week,
            event: PendingAction::PhaseMarker {
                phase: PreSeasonPhase::PlayerProposals,
            },
            executed: false,
        });
        current_week += 1;
    } else {
        for proposal in market_report.player_proposals.iter().cloned() {
            planned_events.push(PlannedEvent {
                week: current_week,
                event: PendingAction::PlayerProposal { proposal },
                executed: false,
            });
        }
        current_week += 1;
    }

    let rookie_events = build_rookie_events(
        &simulated_contracts_by_driver,
        &market_report.new_signings,
        &temp_drivers_by_id,
        current_week,
    )?;
    if rookie_events.is_empty() {
        planned_events.push(PlannedEvent {
            week: current_week,
            event: PendingAction::PhaseMarker {
                phase: PreSeasonPhase::RookiePlacement,
            },
            executed: false,
        });
    } else {
        planned_events.extend(rookie_events);
    }
    current_week += 1;
    let hierarchy_events = build_hierarchy_events(
        &temp_teams,
        &original_teams_by_id,
        &temp_drivers_by_id,
        current_week,
    );
    if hierarchy_events.is_empty() {
        planned_events.push(PlannedEvent {
            week: current_week,
            event: PendingAction::PhaseMarker {
                phase: PreSeasonPhase::Finalization,
            },
            executed: false,
        });
    } else {
        planned_events.extend(hierarchy_events);
    }

    let total_weeks = current_week.max(3);

    let mut state = PreSeasonState {
        season_number,
        current_week: 1,
        total_weeks,
        phase: phase_for_week(1, &planned_events),
        is_complete: false,
        player_has_pending_proposals: market_report
            .player_proposals
            .iter()
            .any(|proposal| proposal.status == ProposalStatus::Pendente),
        player_has_team: false,
        current_display_date: None,
    };
    refresh_preseason_state_display_date(conn, &season_id, &mut state)?;

    Ok(PreSeasonPlan {
        state,
        planned_events,
        executed_weeks: Vec::new(),
    })
}

fn assign_seasonal_team_attributes(
    conn: &Connection,
    season_number: i32,
    season_id: &str,
) -> Result<(), String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao carregar equipes: {e}"))?;
    let previous_standings = load_previous_team_standings(conn, season_number)?;
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
            updated_team.car_build_profile =
                choose_car_build_profile(team, &category_teams, &calendar);
            updated_team.pit_strategy_risk = recalculate_pit_strategy_risk(team, &category_teams);
            let scale = category_finance_scale(&updated_team.categoria);
            let car_unit = scale.operating_cost_midpoint() * 0.30;
            let profile_cost = profile_money_cost(updated_team.car_build_profile, car_unit);
            updated_team.cash_balance -= profile_cost;
            updated_team.budget = derive_budget_index_from_money(&updated_team);
            refresh_team_financial_state(&mut updated_team);
            updated_team.season_strategy = choose_season_strategy(&updated_team).to_string();
            apply_offseason_competitiveness_impact(&mut updated_team);
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
        }
    }

    Ok(())
}

fn load_previous_team_standings(
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

pub fn advance_week(conn: &Connection, plan: &mut PreSeasonPlan) -> Result<WeekResult, String> {
    if plan.state.is_complete {
        return Err("Pre-temporada ja esta completa".to_string());
    }

    repair_missing_licenses_for_current_categories(conn)?;
    let week = plan.state.current_week;
    let season_id = get_season_id_by_number(conn, plan.state.season_number)?
        .ok_or_else(|| format!("Temporada {} nao encontrada", plan.state.season_number))?;
    let phase = phase_for_week(week, &plan.planned_events);
    let indices: Vec<usize> = plan
        .planned_events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            if event.week == week && !event.executed {
                Some(index)
            } else {
                None
            }
        })
        .collect();

    let mut events = Vec::new();
    let mut player_proposals = Vec::new();
    for index in indices {
        let action = plan.planned_events[index].event.clone();
        execute_action(
            conn,
            &season_id,
            plan.state.season_number,
            &action,
            &mut events,
            &mut player_proposals,
        )?;
        plan.planned_events[index].executed = true;
    }

    sync_team_slots_from_active_contracts(conn)?;
    let remaining_vacancies = count_remaining_vacancies(conn)?;

    let is_last_week = week >= plan.state.total_weeks;
    if is_last_week {
        plan.state.current_week = plan.state.total_weeks + 1;
        plan.state.phase = PreSeasonPhase::Complete;
        plan.state.is_complete = true;
        events.push(MarketEvent {
            event_type: MarketEventType::PreSeasonComplete,
            headline: "Pre-temporada encerrada".to_string(),
            description: "O mercado de transferencias foi finalizado.".to_string(),
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
        });
        update_market_state(conn, &season_id, "Fechado", &PreSeasonPhase::Complete, true)?;
    } else {
        plan.state.current_week += 1;
        plan.state.phase = phase_for_week(plan.state.current_week, &plan.planned_events);
        update_market_state(conn, &season_id, "Aberto", &plan.state.phase, false)?;
    }

    let next_phase = if plan.state.is_complete {
        PreSeasonPhase::Complete
    } else {
        phase_for_week(plan.state.current_week, &plan.planned_events)
    };
    refresh_preseason_state_display_date(conn, &season_id, &mut plan.state)?;
    let result = WeekResult {
        week_number: week,
        phase,
        events,
        is_last_week,
        player_proposals,
        remaining_vacancies,
        next_phase,
    };
    plan.executed_weeks.push(result.clone());
    Ok(result)
}

pub fn refresh_preseason_state_display_date(
    conn: &Connection,
    season_id: &str,
    state: &mut PreSeasonState,
) -> Result<(), String> {
    state.current_display_date =
        compute_preseason_display_date(conn, season_id, state.current_week, state.total_weeks)?;
    Ok(())
}

pub fn save_preseason_plan(save_path: &Path, plan: &PreSeasonPlan) -> Result<(), String> {
    std::fs::create_dir_all(save_path)
        .map_err(|e| format!("Falha ao criar diretorio da pre-temporada: {e}"))?;
    let json = serde_json::to_string_pretty(plan)
        .map_err(|e| format!("Falha ao serializar plano da pre-temporada: {e}"))?;
    std::fs::write(preseason_plan_path(save_path), json)
        .map_err(|e| format!("Falha ao salvar plano da pre-temporada: {e}"))
}

pub fn load_preseason_plan(save_path: &Path) -> Result<Option<PreSeasonPlan>, String> {
    let path = preseason_plan_path(save_path);
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Falha ao ler plano da pre-temporada: {e}"))?;
    let plan = serde_json::from_str(&content)
        .map_err(|e| format!("Falha ao parsear plano da pre-temporada: {e}"))?;
    Ok(Some(plan))
}

pub fn delete_preseason_plan(save_path: &Path) -> Result<(), String> {
    let path = preseason_plan_path(save_path);
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_file(path).map_err(|e| format!("Falha ao apagar plano da pre-temporada: {e}"))
}

fn preseason_plan_path(save_path: &Path) -> std::path::PathBuf {
    save_path.join("preseason_plan.json")
}

fn compute_preseason_display_date(
    conn: &Connection,
    season_id: &str,
    current_week: i32,
    total_weeks: i32,
) -> Result<Option<String>, String> {
    let Some(first_regular_event) =
        calendar_queries::get_next_any_race_in_phase(conn, season_id, &SeasonPhase::BlocoRegular)
            .map_err(|e| format!("Falha ao buscar primeira data da temporada regular: {e}"))?
    else {
        return Ok(None);
    };

    let anchor_date = NaiveDate::parse_from_str(&first_regular_event.display_date, "%Y-%m-%d")
        .map_err(|e| format!("Falha ao interpretar data da primeira corrida regular: {e}"))?;
    let effective_week = current_week.clamp(1, total_weeks.max(1));
    let weeks_before_first_event = i64::from(total_weeks.max(1) - effective_week + 1);
    let preseason_date = anchor_date - Duration::days(weeks_before_first_event * 7);
    Ok(Some(preseason_date.format("%Y-%m-%d").to_string()))
}

fn phase_for_week(week: i32, planned_events: &[PlannedEvent]) -> PreSeasonPhase {
    let mut phases = planned_events
        .iter()
        .filter(|event| event.week == week)
        .map(|event| phase_for_action(&event.event))
        .collect::<Vec<_>>();
    phases.sort_by_key(phase_order);
    phases
        .into_iter()
        .next()
        .or_else(|| {
            planned_events
                .iter()
                .filter(|event| event.week > week)
                .map(|event| phase_for_action(&event.event))
                .min_by_key(phase_order)
        })
        .unwrap_or(PreSeasonPhase::Complete)
}

fn phase_for_action(action: &PendingAction) -> PreSeasonPhase {
    match action {
        PendingAction::PhaseMarker { phase } => phase.clone(),
        PendingAction::ExpireContract { .. } | PendingAction::RenewContract { .. } => {
            PreSeasonPhase::ContractExpiry
        }
        PendingAction::Transfer { .. } => PreSeasonPhase::Transfers,
        PendingAction::PlayerProposal { .. } => PreSeasonPhase::PlayerProposals,
        PendingAction::PlaceRookie { .. } => PreSeasonPhase::RookiePlacement,
        PendingAction::UpdateHierarchy { .. } => PreSeasonPhase::Finalization,
    }
}

fn phase_order(phase: &PreSeasonPhase) -> i32 {
    match phase {
        PreSeasonPhase::ContractExpiry => 1,
        PreSeasonPhase::Transfers => 2,
        PreSeasonPhase::PlayerProposals => 3,
        PreSeasonPhase::RookiePlacement => 4,
        PreSeasonPhase::Finalization => 5,
        PreSeasonPhase::Complete => 6,
    }
}

fn build_expiry_events(
    temp_conn: &Connection,
    original_contracts: &[Contract],
    renewed_driver_ids: &HashSet<String>,
) -> Result<Vec<PlannedEvent>, String> {
    Ok(original_contracts
        .iter()
        .filter_map(|contract| {
            if renewed_driver_ids.contains(&contract.piloto_id) {
                return None;
            }
            let temp_contract = contract_queries::get_contract_by_id(temp_conn, &contract.id)
                .ok()
                .flatten()?;
            if temp_contract.status == ContractStatus::Ativo {
                return None;
            }
            Some(PlannedEvent {
                week: 1,
                event: PendingAction::ExpireContract {
                    contract_id: contract.id.clone(),
                    driver_id: contract.piloto_id.clone(),
                    driver_name: contract.piloto_nome.clone(),
                    team_id: contract.equipe_id.clone(),
                    team_name: contract.equipe_nome.clone(),
                },
                executed: false,
            })
        })
        .collect())
}

fn apply_preseason_entry_contract_state(
    conn: &Connection,
    season_number: i32,
) -> Result<(), String> {
    contract_queries::expire_ending_contracts(conn, season_number - 1)
        .map_err(|e| format!("Falha ao expirar contratos na entrada da pre-temporada: {e}"))?;
    sync_team_slots_from_active_contracts(conn)?;
    Ok(())
}

fn apply_preseason_renewal_state(
    conn: &Connection,
    season_number: i32,
    renewal_events: &[PlannedEvent],
) -> Result<(), String> {
    for event in renewal_events {
        let PendingAction::RenewContract {
            driver_id,
            driver_name,
            team_id,
            team_name,
            new_salary,
            new_duration,
            new_role,
        } = &event.event
        else {
            continue;
        };

        let existing_contract =
            contract_queries::get_active_regular_contract_for_pilot(conn, driver_id)
                .map_err(|e| format!("Falha ao buscar renovacao ativa pre-aplicada: {e}"))?;
        if existing_contract.as_ref().is_some_and(|contract| {
            contract.equipe_id == *team_id && contract.temporada_inicio == season_number
        }) {
            continue;
        }

        let team = team_queries::get_team_by_id(conn, team_id)
            .map_err(|e| format!("Falha ao buscar equipe da renovacao pre-aplicada: {e}"))?
            .ok_or_else(|| format!("Equipe '{}' nao encontrada", team_id))?;
        let role = TeamRole::from_str_strict(new_role)
            .map_err(|e| format!("Falha ao interpretar papel da renovacao pre-aplicada: {e}"))?;
        let contract = Contract::new(
            next_id(conn, IdType::Contract)
                .map_err(|e| format!("Falha ao gerar ID da renovacao pre-aplicada: {e}"))?,
            driver_id.clone(),
            driver_name.clone(),
            team_id.clone(),
            team_name.clone(),
            season_number,
            *new_duration,
            *new_salary,
            role,
            team.categoria.clone(),
        );
        contract_queries::insert_contract(conn, &contract).map_err(|e| {
            format!(
                "Falha ao inserir renovacao pre-aplicada '{}': {e}",
                driver_id
            )
        })?;
    }

    sync_team_slots_from_active_contracts(conn)?;
    Ok(())
}

fn build_renewal_events(
    simulated_contracts_by_driver: &std::collections::HashMap<String, Contract>,
    signings: &[crate::market::proposals::SigningInfo],
) -> Result<Vec<PlannedEvent>, String> {
    let renewals: Vec<_> = signings
        .iter()
        .filter(|signing| signing.tipo == "renovacao")
        .cloned()
        .collect();
    let split = renewals.len().div_ceil(2).max(1);
    let mut events = Vec::new();
    for (index, signing) in renewals.into_iter().enumerate() {
        let contract = simulated_contracts_by_driver
            .get(&signing.driver_id)
            .ok_or_else(|| {
                format!(
                    "Contrato renovado de '{}' nao encontrado",
                    signing.driver_id
                )
            })?;
        events.push(PlannedEvent {
            week: if index < split { 1 } else { 2 },
            event: PendingAction::RenewContract {
                driver_id: signing.driver_id,
                driver_name: signing.driver_name,
                team_id: signing.team_id,
                team_name: signing.team_name,
                new_salary: contract.salario_anual,
                new_duration: contract.duracao_anos,
                new_role: contract.papel.as_str().to_string(),
            },
            executed: false,
        });
    }
    Ok(events)
}

fn build_transfer_events(
    simulated_contracts_by_driver: &std::collections::HashMap<String, Contract>,
    signings: &[crate::market::proposals::SigningInfo],
    original_contracts_by_driver: &std::collections::HashMap<String, Contract>,
) -> Result<Vec<PlannedEvent>, String> {
    let mut events = Vec::new();
    for (index, signing) in signings
        .iter()
        .filter(|signing| signing.tipo == "transferencia")
        .cloned()
        .enumerate()
    {
        let contract = simulated_contracts_by_driver
            .get(&signing.driver_id)
            .ok_or_else(|| format!("Contrato de '{}' nao encontrado", signing.driver_id))?;
        let previous_team = original_contracts_by_driver.get(&signing.driver_id);
        events.push(PlannedEvent {
            week: 3 + (index / 3).min(2) as i32,
            event: PendingAction::Transfer {
                driver_id: signing.driver_id,
                driver_name: signing.driver_name,
                from_team_id: previous_team.map(|contract| contract.equipe_id.clone()),
                from_team_name: previous_team.map(|contract| contract.equipe_nome.clone()),
                from_categoria: previous_team.map(|contract| contract.categoria.clone()),
                to_team_id: signing.team_id,
                to_team_name: signing.team_name,
                salary: contract.salario_anual,
                duration: contract.duracao_anos,
                role: contract.papel.as_str().to_string(),
            },
            executed: false,
        });
    }
    Ok(events)
}

fn build_rookie_events(
    simulated_contracts_by_driver: &std::collections::HashMap<String, Contract>,
    signings: &[crate::market::proposals::SigningInfo],
    temp_drivers_by_id: &std::collections::HashMap<String, Driver>,
    week: i32,
) -> Result<Vec<PlannedEvent>, String> {
    let mut events = Vec::new();
    for signing in signings.iter().filter(|signing| signing.tipo == "rookie") {
        let contract = simulated_contracts_by_driver
            .get(&signing.driver_id)
            .ok_or_else(|| format!("Contrato de rookie '{}' nao encontrado", signing.driver_id))?;
        let driver = temp_drivers_by_id
            .get(&signing.driver_id)
            .cloned()
            .ok_or_else(|| format!("Rookie '{}' nao encontrado no clone", signing.driver_id))?;
        events.push(PlannedEvent {
            week,
            event: PendingAction::PlaceRookie {
                driver,
                team_id: signing.team_id.clone(),
                team_name: signing.team_name.clone(),
                salary: contract.salario_anual,
                duration: contract.duracao_anos,
                role: contract.papel.as_str().to_string(),
            },
            executed: false,
        });
    }
    Ok(events)
}

fn build_hierarchy_events(
    temp_teams: &[crate::models::team::Team],
    original_teams_by_id: &std::collections::HashMap<String, crate::models::team::Team>,
    temp_drivers_by_id: &std::collections::HashMap<String, Driver>,
    week: i32,
) -> Vec<PlannedEvent> {
    let mut events = Vec::new();
    for team in temp_teams {
        let changed = original_teams_by_id.get(&team.id).is_none_or(|current| {
            current.piloto_1_id != team.piloto_1_id
                || current.piloto_2_id != team.piloto_2_id
                || current.hierarquia_n1_id != team.hierarquia_n1_id
                || current.hierarquia_n2_id != team.hierarquia_n2_id
        });
        if !changed {
            continue;
        }
        let n1_name = team
            .piloto_1_id
            .as_ref()
            .and_then(|id| temp_drivers_by_id.get(id))
            .map(|driver| driver.nome.clone())
            .unwrap_or_else(|| "Sem piloto".to_string());
        let n2_name = team
            .piloto_2_id
            .as_ref()
            .and_then(|id| temp_drivers_by_id.get(id))
            .map(|driver| driver.nome.clone())
            .unwrap_or_else(|| "Sem piloto".to_string());
        let prev = original_teams_by_id.get(&team.id);
        events.push(PlannedEvent {
            week,
            event: PendingAction::UpdateHierarchy {
                team_id: team.id.clone(),
                team_name: team.nome.clone(),
                n1_id: team.piloto_1_id.clone(),
                n1_name,
                n2_id: team.piloto_2_id.clone(),
                n2_name,
                prev_n1_id: prev.and_then(|t| t.hierarquia_n1_id.clone()),
                prev_n2_id: prev.and_then(|t| t.hierarquia_n2_id.clone()),
                prev_tensao: prev.map(|t| t.hierarquia_tensao).unwrap_or(0.0),
                prev_status: prev
                    .map(|t| t.hierarquia_status.clone())
                    .unwrap_or_else(|| "estavel".to_string()),
                prev_categoria: prev.map(|t| t.categoria.clone()).unwrap_or_default(),
            },
            executed: false,
        });
    }
    events
}

fn get_season_id_by_number(
    conn: &Connection,
    season_number: i32,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT id FROM seasons WHERE numero = ?1 LIMIT 1",
        rusqlite::params![season_number],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| format!("Falha ao buscar temporada {season_number}: {e}"))
}

fn reset_market_state(
    conn: &Connection,
    season_id: &str,
    phase: &PreSeasonPhase,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM market_proposals WHERE temporada_id = ?1",
        rusqlite::params![season_id],
    )
    .map_err(|e| format!("Falha ao limpar propostas antigas da pre-temporada: {e}"))?;
    conn.execute(
        "DELETE FROM market WHERE temporada_id = ?1",
        rusqlite::params![season_id],
    )
    .map_err(|e| format!("Falha ao limpar estado antigo do mercado: {e}"))?;
    conn.execute(
        "INSERT INTO market (temporada_id, status, fase, inicio, fim)
         VALUES (?1, 'Aberto', ?2, ?3, '')",
        rusqlite::params![season_id, phase_label(phase), timestamp_now()],
    )
    .map_err(|e| format!("Falha ao inicializar estado do mercado: {e}"))?;
    Ok(())
}

fn update_market_state(
    conn: &Connection,
    season_id: &str,
    status: &str,
    phase: &PreSeasonPhase,
    completed: bool,
) -> Result<(), String> {
    let end_value = if completed {
        timestamp_now()
    } else {
        String::new()
    };
    conn.execute(
        "UPDATE market
         SET status = ?1, fase = ?2, fim = CASE WHEN ?3 = '' THEN fim ELSE ?3 END
         WHERE temporada_id = ?4",
        rusqlite::params![status, phase_label(phase), end_value, season_id],
    )
    .map_err(|e| format!("Falha ao atualizar estado do mercado: {e}"))?;
    Ok(())
}

fn phase_label(phase: &PreSeasonPhase) -> &'static str {
    match phase {
        PreSeasonPhase::ContractExpiry => "ContractExpiry",
        PreSeasonPhase::Transfers => "Transfers",
        PreSeasonPhase::PlayerProposals => "PlayerProposals",
        PreSeasonPhase::RookiePlacement => "RookiePlacement",
        PreSeasonPhase::Finalization => "Finalization",
        PreSeasonPhase::Complete => "Complete",
    }
}

fn timestamp_now() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

#[cfg(test)]
fn clone_connection_to_temp(conn: &Connection) -> Result<std::path::PathBuf, String> {
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|e| format!("Falha ao checkpointar banco antes do clone: {e}"))?;
    let temp_path = next_preseason_clone_path()?;
    let escaped = temp_path
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{escaped}';"))
        .map_err(|e| format!("Falha ao clonar banco para planejamento da pre-temporada: {e}"))?;
    Ok(temp_path)
}

#[cfg(test)]
fn next_preseason_clone_path() -> Result<std::path::PathBuf, String> {
    let pid = std::process::id();
    for _ in 0..64 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("Falha ao gerar timestamp do clone: {e}"))?
            .as_nanos();
        let counter = PRESEASON_CLONE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = std::env::temp_dir().join(format!(
            "iracerapp_preseason_clone_{pid}_{nanos}_{counter}.db"
        ));

        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err("Falha ao reservar caminho unico para clone temporario da pre-temporada".to_string())
}

#[cfg(test)]
fn cleanup_temp_db(path: &Path) -> Result<(), String> {
    fn remove_if_exists(path: &Path) -> Result<(), String> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(format!(
                "Falha ao remover arquivo temporario '{}': {err}",
                path.display()
            )),
        }
    }

    remove_if_exists(path)?;
    let wal = std::path::PathBuf::from(format!("{}-wal", path.to_string_lossy()));
    let shm = std::path::PathBuf::from(format!("{}-shm", path.to_string_lossy()));
    remove_if_exists(&wal)?;
    remove_if_exists(&shm)?;
    Ok(())
}

fn execute_action(
    conn: &Connection,
    season_id: &str,
    season_number: i32,
    action: &PendingAction,
    events: &mut Vec<MarketEvent>,
    player_proposals: &mut Vec<MarketProposal>,
) -> Result<(), String> {
    match action {
        PendingAction::PhaseMarker { .. } => {}
        PendingAction::ExpireContract {
            contract_id,
            driver_id,
            driver_name,
            team_id,
            team_name,
        } => {
            let status = driver_queries::get_driver(conn, driver_id)
                .map(|driver| {
                    if driver.status == DriverStatus::Aposentado {
                        ContractStatus::Rescindido
                    } else {
                        ContractStatus::Expirado
                    }
                })
                .unwrap_or(ContractStatus::Expirado);
            contract_queries::update_contract_status(conn, contract_id, &status)
                .map_err(|e| format!("Falha ao encerrar contrato '{}': {e}", contract_id))?;
            clear_driver_from_team(conn, team_id, driver_id)?;
            let category = team_queries::get_team_by_id(conn, team_id)
                .ok()
                .flatten()
                .map(|team| team.categoria);
            events.push(MarketEvent {
                event_type: MarketEventType::ContractExpired,
                headline: format!("{driver_name} deixa {team_name}"),
                description: format!("O vinculo de {driver_name} com {team_name} foi encerrado."),
                driver_id: Some(driver_id.clone()),
                driver_name: Some(driver_name.clone()),
                team_id: Some(team_id.clone()),
                team_name: Some(team_name.clone()),
                from_team: Some(team_name.clone()),
                to_team: None,
                categoria: category.clone(),
                from_categoria: category.clone(),
                movement_kind: classify_market_movement(
                    &MarketEventType::ContractExpired,
                    category.as_deref(),
                    None,
                ),
                championship_position: market_event_championship_position(
                    conn,
                    driver_id,
                    category.as_deref(),
                    season_number,
                ),
            });
        }
        PendingAction::RenewContract {
            driver_id,
            driver_name,
            team_id,
            team_name,
            new_salary,
            new_duration,
            new_role,
        } => {
            let team = team_queries::get_team_by_id(conn, team_id)
                .map_err(|e| format!("Falha ao buscar equipe da renovacao: {e}"))?
                .ok_or_else(|| format!("Equipe '{}' nao encontrada", team_id))?;
            let existing_contract =
                contract_queries::get_active_regular_contract_for_pilot(conn, driver_id)
                    .map_err(|e| format!("Falha ao buscar renovacao ja aplicada: {e}"))?;
            if !existing_contract.as_ref().is_some_and(|contract| {
                contract.equipe_id == *team_id && contract.temporada_inicio == season_number
            }) {
                let role = TeamRole::from_str_strict(new_role)
                    .map_err(|e| format!("Falha ao interpretar papel da renovacao: {e}"))?;
                let contract = Contract::new(
                    next_id(conn, IdType::Contract)
                        .map_err(|e| format!("Falha ao gerar ID da renovacao: {e}"))?,
                    driver_id.clone(),
                    driver_name.clone(),
                    team_id.clone(),
                    team_name.clone(),
                    season_number,
                    *new_duration,
                    *new_salary,
                    role,
                    team.categoria.clone(),
                );
                contract_queries::insert_contract(conn, &contract)
                    .map_err(|e| format!("Falha ao inserir renovacao '{}': {e}", driver_id))?;
            }
            let category = team.categoria.clone();
            events.push(MarketEvent {
                event_type: MarketEventType::ContractRenewed,
                headline: format!("{driver_name} renova com {team_name}"),
                description: format!(
                    "{driver_name} renovou com {team_name} por {} ano(s).",
                    new_duration
                ),
                driver_id: Some(driver_id.clone()),
                driver_name: Some(driver_name.clone()),
                team_id: Some(team_id.clone()),
                team_name: Some(team_name.clone()),
                from_team: Some(team_name.clone()),
                to_team: Some(team_name.clone()),
                categoria: Some(category.clone()),
                from_categoria: Some(category.clone()),
                movement_kind: classify_market_movement(
                    &MarketEventType::ContractRenewed,
                    Some(&category),
                    Some(&category),
                ),
                championship_position: market_event_championship_position(
                    conn,
                    driver_id,
                    Some(&category),
                    season_number,
                ),
            });
        }
        PendingAction::Transfer {
            driver_id,
            driver_name,
            from_team_name,
            from_categoria,
            to_team_id,
            to_team_name,
            salary,
            duration,
            role,
            ..
        } => {
            sign_driver_to_team(
                conn,
                driver_id,
                driver_name,
                to_team_id,
                season_number,
                *salary,
                *duration,
                role,
            )?;
            let to_category = team_queries::get_team_by_id(conn, to_team_id)
                .ok()
                .flatten()
                .map(|team| team.categoria);
            events.push(MarketEvent {
                event_type: MarketEventType::TransferCompleted,
                headline: format!("{driver_name} assina com {to_team_name}"),
                description: format!(
                    "{driver_name} deixa {} e assina com {to_team_name}.",
                    from_team_name
                        .clone()
                        .unwrap_or_else(|| "o mercado livre".to_string())
                ),
                driver_id: Some(driver_id.clone()),
                driver_name: Some(driver_name.clone()),
                team_id: Some(to_team_id.clone()),
                team_name: Some(to_team_name.clone()),
                from_team: from_team_name.clone(),
                to_team: Some(to_team_name.clone()),
                categoria: to_category.clone(),
                from_categoria: from_categoria.clone(),
                movement_kind: classify_market_movement(
                    &MarketEventType::TransferCompleted,
                    from_categoria.as_deref(),
                    to_category.as_deref(),
                ),
                championship_position: market_event_championship_position(
                    conn,
                    driver_id,
                    from_categoria.as_deref().or(to_category.as_deref()),
                    season_number,
                ),
            });
        }
        PendingAction::PlayerProposal { proposal } => {
            persist_player_proposal(conn, season_id, proposal)?;
            player_proposals.push(proposal.clone());
            events.push(MarketEvent {
                event_type: MarketEventType::PlayerProposalReceived,
                headline: format!(
                    "{} recebe proposta de {}",
                    proposal.piloto_nome, proposal.equipe_nome
                ),
                description: format!(
                    "{} oferece {} por {} ano(s).",
                    proposal.equipe_nome,
                    proposal.papel.as_str(),
                    proposal.duracao_anos
                ),
                driver_id: Some(proposal.piloto_id.clone()),
                driver_name: Some(proposal.piloto_nome.clone()),
                team_id: Some(proposal.equipe_id.clone()),
                team_name: Some(proposal.equipe_nome.clone()),
                from_team: None,
                to_team: Some(proposal.equipe_nome.clone()),
                categoria: Some(proposal.categoria.clone()),
                from_categoria: None,
                movement_kind: None,
                championship_position: market_event_championship_position(
                    conn,
                    &proposal.piloto_id,
                    Some(&proposal.categoria),
                    season_number,
                ),
            });
        }
        PendingAction::PlaceRookie {
            driver,
            team_id,
            team_name,
            salary,
            duration,
            role,
        } => {
            ensure_driver_exists(conn, driver)?;
            let team = team_queries::get_team_by_id(conn, team_id)
                .map_err(|e| format!("Falha ao buscar equipe '{}' para rookie: {e}", team_id))?
                .ok_or_else(|| format!("Equipe '{}' nao encontrada", team_id))?;
            grant_driver_license_for_category_if_needed(conn, &driver.id, &team.categoria)?;
            sign_driver_to_team(
                conn,
                &driver.id,
                &driver.nome,
                team_id,
                season_number,
                *salary,
                *duration,
                role,
            )?;
            let category = team.categoria.clone();
            let event_type = if is_real_career_debut_category(&category) {
                MarketEventType::RookieSigned
            } else {
                MarketEventType::TransferCompleted
            };
            events.push(MarketEvent {
                event_type: event_type.clone(),
                headline: format!("Rookie {} assina com {team_name}", driver.nome),
                description: format!("{} e o novo piloto da {team_name}.", driver.nome),
                driver_id: Some(driver.id.clone()),
                driver_name: Some(driver.nome.clone()),
                team_id: Some(team_id.clone()),
                team_name: Some(team_name.clone()),
                from_team: None,
                to_team: Some(team_name.clone()),
                categoria: Some(category.clone()),
                from_categoria: None,
                movement_kind: classify_market_movement(&event_type, None, Some(&category)),
                championship_position: market_event_championship_position(
                    conn,
                    &driver.id,
                    Some(&category),
                    season_number,
                ),
            });
        }
        PendingAction::UpdateHierarchy {
            team_id,
            team_name,
            n1_id,
            n1_name,
            n2_id,
            n2_name,
            prev_n1_id,
            prev_n2_id,
            prev_tensao,
            prev_status,
            prev_categoria,
        } => {
            use crate::hierarchy::transition::{
                decide_hierarchy_transition, resolve_transition_values, NewSeasonSetup,
                PrevHierarchyState, ResolvedTeamLineup,
            };

            // Valida o lineup final antes de qualquer leitura de DB ou persistência
            let resolved_lineup =
                ResolvedTeamLineup::new(team_id, n1_id.as_deref(), n2_id.as_deref())
                    .map_err(|e| format!("Lineup inválido para UpdateHierarchy: {e}"))?;

            // Ler categoria atual da equipe no DB (pode ter sido atualizada durante o ciclo)
            let current_team = team_queries::get_team_by_id(conn, team_id)
                .map_err(|e| format!("Falha ao ler equipe '{}': {e}", team_id))?
                .ok_or_else(|| format!("Equipe '{}' nao encontrada", team_id))?;

            let prev_state = PrevHierarchyState {
                n1_id: prev_n1_id.as_deref(),
                n2_id: prev_n2_id.as_deref(),
                tensao: *prev_tensao,
                status: prev_status.as_str(),
                categoria: prev_categoria.as_str(),
            };
            let new_setup = NewSeasonSetup {
                n1_id: Some(resolved_lineup.n1_id.as_str()),
                n2_id: Some(resolved_lineup.n2_id.as_str()),
                categoria: &current_team.categoria,
            };
            let decision = decide_hierarchy_transition(&prev_state, &new_setup);
            let (new_tensao, new_status) =
                resolve_transition_values(&decision, *prev_tensao, prev_status.as_str());

            team_queries::update_team_pilots(
                conn,
                team_id,
                Some(resolved_lineup.n1_id.as_str()),
                Some(resolved_lineup.n2_id.as_str()),
            )
            .map_err(|e| format!("Falha ao atualizar pilotos da equipe '{}': {e}", team_id))?;
            team_queries::update_team_hierarchy(
                conn,
                team_id,
                Some(resolved_lineup.n1_id.as_str()),
                Some(resolved_lineup.n2_id.as_str()),
                new_status,
                new_tensao,
            )
            .map_err(|e| format!("Falha ao atualizar hierarquia da equipe '{}': {e}", team_id))?;
            // Contadores de duelo são sempre resetados — são temporais por temporada
            team_queries::update_team_duel_counters(conn, team_id, 0, 0, 0, 0, 0)
                .map_err(|e| format!("Falha ao resetar contadores da equipe '{}': {e}", team_id))?;
            events.push(MarketEvent {
                event_type: MarketEventType::HierarchyUpdated,
                headline: format!("{team_name}: {n1_name} e N1, {n2_name} e N2"),
                description: format!("{team_name} definiu sua hierarquia para a temporada."),
                driver_id: None,
                driver_name: None,
                team_id: Some(team_id.clone()),
                team_name: Some(team_name.clone()),
                from_team: None,
                to_team: None,
                categoria: team_queries::get_team_by_id(conn, team_id)
                    .ok()
                    .flatten()
                    .map(|team| team.categoria),
                from_categoria: None,
                movement_kind: None,
                championship_position: None,
            });
        }
    }
    Ok(())
}

fn market_category_tier(category: &str) -> i32 {
    match category {
        "mazda_rookie" | "toyota_rookie" => 1,
        "mazda_amador" | "toyota_amador" | "bmw_m2" | "mazda" | "toyota" | "bmw" => 2,
        "production_challenger" => 3,
        "gt4" => 4,
        "gt3" => 5,
        "endurance" => 6,
        _ => 0,
    }
}

fn classify_market_movement(
    event_type: &MarketEventType,
    from_category: Option<&str>,
    to_category: Option<&str>,
) -> Option<String> {
    match event_type {
        MarketEventType::ContractExpired => Some("departure".to_string()),
        MarketEventType::RookieSigned => {
            if to_category.is_some_and(is_real_career_debut_category) {
                Some("rookie".to_string())
            } else {
                Some("signing".to_string())
            }
        }
        MarketEventType::ContractRenewed => Some("renewal".to_string()),
        MarketEventType::TransferCompleted => {
            let Some(to_category) = to_category else {
                return Some("signing".to_string());
            };
            let Some(from_category) = from_category else {
                return Some("signing".to_string());
            };
            let from_tier = market_category_tier(from_category);
            let to_tier = market_category_tier(to_category);
            if from_tier == 0 || to_tier == 0 || from_tier == to_tier {
                return Some("lateral".to_string());
            }
            if to_tier > from_tier {
                Some("promotion".to_string())
            } else {
                Some("relegation".to_string())
            }
        }
        _ => None,
    }
}

fn latest_standing_position_for_market_event(
    conn: &Connection,
    driver_id: &str,
    category: Option<&str>,
    season_number: i32,
) -> Option<i32> {
    if let Some(category) = category {
        return conn
            .query_row(
                "SELECT st.posicao
                 FROM standings st
                 JOIN seasons s ON s.id = st.temporada_id
                 WHERE st.piloto_id = ?1
                   AND st.categoria = ?2
                   AND st.posicao > 0
                   AND s.numero < ?3
                 ORDER BY s.numero DESC
                 LIMIT 1",
                rusqlite::params![driver_id, category, season_number],
                |row| row.get::<_, i32>(0),
            )
            .optional()
            .ok()
            .flatten();
    }

    conn.query_row(
        "SELECT st.posicao
         FROM standings st
         JOIN seasons s ON s.id = st.temporada_id
         WHERE st.piloto_id = ?1
           AND st.posicao > 0
           AND s.numero < ?2
         ORDER BY s.numero DESC
         LIMIT 1",
        rusqlite::params![driver_id, season_number],
        |row| row.get::<_, i32>(0),
    )
    .optional()
    .ok()
    .flatten()
}

fn market_event_championship_position(
    conn: &Connection,
    driver_id: &str,
    result_category: Option<&str>,
    season_number: i32,
) -> Option<i32> {
    if let Some(position) =
        latest_standing_position_for_market_event(conn, driver_id, result_category, season_number)
    {
        return Some(position);
    }

    let driver = driver_queries::get_driver(conn, driver_id).ok()?;

    let contract_category =
        contract_queries::get_active_regular_contract_for_pilot(conn, driver_id)
            .ok()
            .flatten()
            .map(|contract| contract.categoria);
    let category = result_category.or_else(|| {
        driver
            .categoria_atual
            .as_deref()
            .or(contract_category.as_deref())
    })?;
    let mut drivers = driver_queries::get_drivers_by_category(conn, category).ok()?;
    drivers.sort_by(|left, right| {
        right
            .stats_temporada
            .pontos
            .total_cmp(&left.stats_temporada.pontos)
            .then_with(|| {
                right
                    .stats_temporada
                    .vitorias
                    .cmp(&left.stats_temporada.vitorias)
            })
            .then_with(|| {
                right
                    .stats_temporada
                    .podios
                    .cmp(&left.stats_temporada.podios)
            })
            .then_with(|| {
                left.stats_temporada
                    .posicao_media
                    .total_cmp(&right.stats_temporada.posicao_media)
            })
            .then_with(|| left.nome.cmp(&right.nome))
    });

    drivers
        .iter()
        .position(|candidate| candidate.id == driver_id)
        .map(|index| index as i32 + 1)
        .or_else(|| driver.melhor_resultado_temp.map(|position| position as i32))
}

fn persist_player_proposal(
    conn: &Connection,
    season_id: &str,
    proposal: &MarketProposal,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO market_proposals (
            id, temporada_id, equipe_id, piloto_id, papel, salario, status, motivo_recusa, criado_em
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            &proposal.id,
            season_id,
            &proposal.equipe_id,
            &proposal.piloto_id,
            proposal.papel.as_str(),
            proposal.salario_oferecido,
            proposal.status.as_str(),
            proposal.motivo_recusa.clone(),
            timestamp_now(),
        ],
    )
    .map_err(|e| format!("Falha ao persistir proposta do jogador: {e}"))?;
    Ok(())
}

fn ensure_driver_exists(conn: &Connection, driver: &Driver) -> Result<(), String> {
    if driver_queries::get_driver(conn, &driver.id).is_ok() {
        return Ok(());
    }
    driver_queries::insert_driver(conn, driver)
        .map_err(|e| format!("Falha ao inserir rookie planejado '{}': {e}", driver.id))
}

fn sign_driver_to_team(
    conn: &Connection,
    driver_id: &str,
    driver_name: &str,
    team_id: &str,
    season_number: i32,
    salary: f64,
    duration: i32,
    role: &str,
) -> Result<(), String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao buscar equipe '{}' para assinatura: {e}", team_id))?
        .ok_or_else(|| format!("Equipe '{}' nao encontrada", team_id))?;
    let driver = driver_queries::get_driver(conn, driver_id).map_err(|e| {
        format!(
            "Falha ao buscar piloto '{}' para assinatura: {e}",
            driver_id
        )
    })?;
    ensure_driver_can_join_category(conn, driver_id, driver_name, &team.categoria)?;
    let role = TeamRole::from_str_strict(role)
        .map_err(|e| format!("Falha ao interpretar papel da assinatura: {e}"))?;
    let contract = Contract::new(
        next_id(conn, IdType::Contract)
            .map_err(|e| format!("Falha ao gerar ID de contrato: {e}"))?,
        driver_id.to_string(),
        driver_name.to_string(),
        team_id.to_string(),
        team.nome.clone(),
        season_number,
        duration,
        salary,
        role,
        team.categoria.clone(),
    );
    contract_queries::insert_contract(conn, &contract)
        .map_err(|e| format!("Falha ao assinar contrato de '{}': {e}", driver_id))?;

    let mut updated_driver = driver;
    updated_driver.categoria_atual = Some(team.categoria.clone());
    driver_queries::update_driver(conn, &updated_driver)
        .map_err(|e| format!("Falha ao atualizar piloto contratado '{}': {e}", driver_id))?;
    Ok(())
}

fn clear_driver_from_team(conn: &Connection, team_id: &str, driver_id: &str) -> Result<(), String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao buscar equipe para liberar piloto: {e}"))?
        .ok_or_else(|| format!("Equipe '{}' nao encontrada", team_id))?;
    let piloto_1 = if team.piloto_1_id.as_deref() == Some(driver_id) {
        None
    } else {
        team.piloto_1_id.as_deref()
    };
    let piloto_2 = if team.piloto_2_id.as_deref() == Some(driver_id) {
        None
    } else {
        team.piloto_2_id.as_deref()
    };
    team_queries::update_team_pilots(conn, team_id, piloto_1, piloto_2)
        .map_err(|e| format!("Falha ao remover piloto da equipe '{}': {e}", team_id))
}

fn sync_team_slots_from_active_contracts(conn: &Connection) -> Result<(), String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao carregar equipes: {e}"))?;
    let drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos: {e}"))?;
    let drivers_by_id = drivers
        .into_iter()
        .map(|driver| (driver.id.clone(), driver))
        .collect::<std::collections::HashMap<_, _>>();
    sync_team_slots_from_active_regular_contracts(conn, &teams, &drivers_by_id)
}

fn count_remaining_vacancies(conn: &Connection) -> Result<i32, String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao contar vagas: {e}"))?;
    Ok(teams
        .iter()
        .map(|team| {
            let mut open = 0;
            if team.piloto_1_id.is_none() {
                open += 1;
            }
            if team.piloto_2_id.is_none() {
                open += 1;
            }
            open
        })
        .sum())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use rand::{rngs::StdRng, SeedableRng};
    use rusqlite::{params, Connection};

    use super::*;
    use crate::calendar::CalendarEntry;
    use crate::constants::teams::get_team_templates;
    use crate::db::migrations;
    use crate::db::queries::calendar as calendar_queries;
    use crate::db::queries::contracts as contract_queries;
    use crate::db::queries::drivers as driver_queries;
    use crate::db::queries::seasons as season_queries;
    use crate::db::queries::teams as team_queries;
    use crate::finance::planning::derive_budget_index_from_money;
    use crate::models::contract::Contract;
    use crate::models::driver::Driver;
    use crate::models::enums::{
        DriverStatus, RaceStatus, SeasonPhase, TeamRole, ThematicSlot, WeatherCondition,
    };
    use crate::models::license::driver_has_required_license_for_category;
    use crate::models::season::Season;
    use crate::simulation::car_build::CarBuildProfile;

    fn sample_calendar_entry(
        id: &str,
        season_id: &str,
        category: &str,
        rodada: i32,
        track_id: u32,
    ) -> CalendarEntry {
        CalendarEntry {
            id: id.to_string(),
            season_id: season_id.to_string(),
            categoria: category.to_string(),
            rodada,
            nome: format!("Round {rodada}"),
            track_id,
            track_name: format!("Track {track_id}"),
            track_config: "Full".to_string(),
            clima: WeatherCondition::Dry,
            temperatura: 22.0,
            voltas: 20,
            duracao_corrida_min: 30,
            duracao_classificacao_min: 15,
            status: RaceStatus::Pendente,
            horario: "14:00".to_string(),
            week_of_year: rodada,
            season_phase: SeasonPhase::BlocoRegular,
            display_date: "2025-02-01".to_string(),
            thematic_slot: ThematicSlot::NaoClassificado,
        }
    }

    #[test]
    fn test_initialize_preseason_creates_plan() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(500);

        let plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

        assert_eq!(plan.state.season_number, 2);
        assert_eq!(plan.state.current_week, 1);
        assert!(plan.state.total_weeks >= 3);
        assert!(!plan.planned_events.is_empty());
    }

    #[test]
    fn test_initialize_preseason_has_all_phases() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(501);

        let plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");
        let phases: Vec<_> = (1..=plan.state.total_weeks)
            .map(|week| phase_for_week(week, &plan.planned_events))
            .collect();

        assert!(phases.contains(&PreSeasonPhase::ContractExpiry));
        assert!(phases.contains(&PreSeasonPhase::Transfers));
        assert!(phases.contains(&PreSeasonPhase::PlayerProposals));
        assert!(phases.contains(&PreSeasonPhase::RookiePlacement));
        assert!(phases.contains(&PreSeasonPhase::Finalization));
    }

    #[test]
    fn test_plan_total_weeks_reasonable() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(502);

        let plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

        assert!((3..=12).contains(&plan.state.total_weeks));
    }

    #[test]
    fn test_initialize_preseason_assigns_power_profile_for_weak_team_on_power_calendar() {
        let conn = setup_market_fixture();
        for entry in [
            sample_calendar_entry("R101", "S002", "gt4", 1, 93),
            sample_calendar_entry("R102", "S002", "gt4", 2, 287),
            sample_calendar_entry("R103", "S002", "gt4", 3, 188),
            sample_calendar_entry("R104", "S002", "gt4", 4, 397),
        ] {
            calendar_queries::insert_calendar_entry(&conn, &entry).expect("insert calendar entry");
        }

        let mut team_a = team_queries::get_team_by_id(&conn, "T001")
            .expect("load team a")
            .expect("team a exists");
        team_a.car_performance = 12.0;
        team_a.budget = 85.0;
        team_a.engineering = 82.0;
        team_a.facilities = 80.0;
        team_queries::update_team(&conn, &team_a).expect("update team a");

        let mut team_b = team_queries::get_team_by_id(&conn, "T002")
            .expect("load team b")
            .expect("team b exists");
        team_b.car_performance = 4.0;
        team_b.budget = 18.0;
        team_b.cash_balance = 2_500_000.0;
        team_b.engineering = 35.0;
        team_b.facilities = 30.0;
        let team_b_cash_before = team_b.cash_balance;
        team_queries::update_team(&conn, &team_b).expect("update team b");

        let mut rng = StdRng::seed_from_u64(502);
        let _plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

        let updated_team_b = team_queries::get_team_by_id(&conn, "T002")
            .expect("reload team b")
            .expect("team b exists after preseason");
        let updated_team_a = team_queries::get_team_by_id(&conn, "T001")
            .expect("reload team a")
            .expect("team a exists after preseason");
        assert!(matches!(
            updated_team_b.car_build_profile,
            CarBuildProfile::PowerIntermediate | CarBuildProfile::PowerExtreme
        ));
        assert!(
            updated_team_b.cash_balance < team_b_cash_before,
            "expected car profile cost to consume cash: before={}, after={}",
            team_b_cash_before,
            updated_team_b.cash_balance
        );
        let expected_budget = derive_budget_index_from_money(&updated_team_b);
        assert!(
            (updated_team_b.budget - expected_budget).abs() < 0.0001,
            "expected derived budget {expected_budget}, got {}",
            updated_team_b.budget
        );
        assert!(
            updated_team_b.pit_strategy_risk > updated_team_a.pit_strategy_risk,
            "backmarker should carry more pit risk: weak={} strong={}",
            updated_team_b.pit_strategy_risk,
            updated_team_a.pit_strategy_risk
        );
        assert!(
            updated_team_a.pit_crew_quality > updated_team_b.pit_crew_quality,
            "richer team should keep stronger pit crew: strong={} weak={}",
            updated_team_a.pit_crew_quality,
            updated_team_b.pit_crew_quality
        );
        assert_eq!(updated_team_a.season_strategy, "balanced");
        assert!(matches!(
            updated_team_b.season_strategy.as_str(),
            "all_in" | "survival" | "austerity"
        ));
    }

    #[test]
    fn test_initialize_preseason_applies_financial_crisis_drag_to_team_quality() {
        let conn = setup_market_fixture();
        for entry in [
            sample_calendar_entry("R201", "S002", "gt4", 1, 93),
            sample_calendar_entry("R202", "S002", "gt4", 2, 397),
        ] {
            calendar_queries::insert_calendar_entry(&conn, &entry).expect("insert calendar entry");
        }

        let mut team = team_queries::get_team_by_id(&conn, "T002")
            .expect("load team")
            .expect("team exists");
        team.confiabilidade = 70.0;
        team.engineering = 45.0;
        team.facilities = 45.0;
        team.cash_balance = -100_000.0;
        team.debt_balance = 900_000.0;
        team.financial_state = "collapse".to_string();
        team.season_strategy = "survival".to_string();
        team_queries::update_team(&conn, &team).expect("update crisis team");

        let mut rng = StdRng::seed_from_u64(506);
        let _plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

        let updated_team = team_queries::get_team_by_id(&conn, "T002")
            .expect("reload team")
            .expect("team exists after preseason");

        assert!(updated_team.confiabilidade < 70.0);
        assert!(updated_team.engineering < 45.0);
        assert!(updated_team.facilities < 45.0);
    }

    #[test]
    fn test_advance_week_executes_events() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(503);
        let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

        let result = advance_week(&conn, &mut plan).expect("week should advance");

        assert_eq!(result.week_number, 1);
        assert!(!result.events.is_empty());
        assert!(plan.planned_events.iter().any(|event| event.executed));
    }

    #[test]
    fn test_advance_week_increments_week() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(504);
        let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

        advance_week(&conn, &mut plan).expect("week should advance");

        assert_eq!(plan.state.current_week, 2);
    }

    #[test]
    fn test_advance_week_phase_transitions() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(505);
        let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

        let mut seen_player_phase = false;
        while !plan.state.is_complete {
            let result = advance_week(&conn, &mut plan).expect("week should advance");
            if result.next_phase == PreSeasonPhase::PlayerProposals
                || result.phase == PreSeasonPhase::PlayerProposals
            {
                seen_player_phase = true;
            }
        }

        assert!(seen_player_phase);
        assert_eq!(plan.state.phase, PreSeasonPhase::Complete);
    }

    #[test]
    fn test_contract_expiry_week() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(506);
        let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

        let week_one_expiries: Vec<String> = plan
            .planned_events
            .iter()
            .filter_map(|event| match &event.event {
                PendingAction::ExpireContract { contract_id, .. } if event.week == 1 => {
                    Some(contract_id.clone())
                }
                _ => None,
            })
            .collect();
        assert!(!week_one_expiries.is_empty());

        let result = advance_week(&conn, &mut plan).expect("week should advance");
        assert_eq!(result.phase, PreSeasonPhase::ContractExpiry);
        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == MarketEventType::ContractExpired));

        for contract_id in week_one_expiries {
            let status: String = conn
                .query_row(
                    "SELECT status FROM contracts WHERE id = ?1",
                    [&contract_id],
                    |row| row.get(0),
                )
                .expect("contract status");
            assert_ne!(status, "Ativo");
        }
    }

    #[test]
    fn test_initialize_preseason_expires_ending_contracts_immediately() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(5061);

        let active_before = contract_queries::get_active_regular_contract_for_pilot(&conn, "P007")
            .expect("active contract query before preseason")
            .expect("player should start with active contract");
        assert_eq!(active_before.id, "C004");

        let plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

        let active_after = contract_queries::get_active_regular_contract_for_pilot(&conn, "P007")
            .expect("active contract query after preseason");
        assert!(
            active_after.is_none(),
            "piloto com contrato encerrado na temporada anterior deve entrar na pre-temporada sem contrato ativo"
        );

        let player_contract_status: String = conn
            .query_row(
                "SELECT status FROM contracts WHERE id = 'C004'",
                [],
                |row| row.get(0),
            )
            .expect("player contract status");
        assert_ne!(player_contract_status, "Ativo");

        assert!(
            !plan.planned_events.iter().any(|event| {
                event.week > 1
                    && matches!(event.event, PendingAction::ExpireContract { .. })
            }),
            "expiracoes de contrato nao devem ficar adiadas para semanas posteriores ao inicio da janela"
        );
    }

    #[test]
    fn test_renewal_week() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(507);
        let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

        while !plan.state.is_complete {
            let result = advance_week(&conn, &mut plan).expect("week should advance");
            if result
                .events
                .iter()
                .any(|event| event.event_type == MarketEventType::ContractRenewed)
            {
                break;
            }
        }

        let renewed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM contracts WHERE piloto_id = 'P001' AND temporada_inicio = 2 AND status = 'Ativo'",
                [],
                |row| row.get(0),
            )
            .expect("renewed count");
        assert_eq!(renewed, 1);
    }

    #[test]
    fn test_build_expiry_events_skips_driver_with_planned_renewal() {
        let conn = setup_market_fixture();
        let original_contracts =
            contract_queries::get_all_active_regular_contracts(&conn).expect("active contracts");
        apply_preseason_entry_contract_state(&conn, 2).expect("entry state");
        let renewed_driver_ids = HashSet::from(["P001".to_string()]);

        let events =
            build_expiry_events(&conn, &original_contracts, &renewed_driver_ids).expect("events");

        assert!(
            !events.iter().any(|event| matches!(
                &event.event,
                PendingAction::ExpireContract { driver_id, .. } if driver_id == "P001"
            )),
            "piloto com renovacao planejada nao deve ser tratado como saida de equipe"
        );
    }

    #[test]
    fn test_apply_preseason_renewal_state_keeps_driver_linked_to_team() {
        let conn = setup_market_fixture();
        apply_preseason_entry_contract_state(&conn, 2).expect("entry state");

        let renewal_events = vec![PlannedEvent {
            week: 1,
            executed: false,
            event: PendingAction::RenewContract {
                driver_id: "P001".to_string(),
                driver_name: "Piloto A".to_string(),
                team_id: "T001".to_string(),
                team_name: "Equipe A".to_string(),
                new_salary: 140_000.0,
                new_duration: 2,
                new_role: TeamRole::Numero1.as_str().to_string(),
            },
        }];

        apply_preseason_renewal_state(&conn, 2, &renewal_events).expect("renewal state");

        let renewed_contract =
            contract_queries::get_active_regular_contract_for_pilot(&conn, "P001")
                .expect("renewed contract query")
                .expect("renewed driver should remain under active contract");
        assert_eq!(renewed_contract.temporada_inicio, 2);
        assert_eq!(renewed_contract.equipe_id, "T001");

        let team = team_queries::get_team_by_id(&conn, "T001")
            .expect("team query")
            .expect("team exists");
        assert!(
            team.piloto_1_id.as_deref() == Some("P001")
                || team.piloto_2_id.as_deref() == Some("P001"),
            "renovacao deve manter o piloto vinculado a equipe desde a entrada da pre-temporada"
        );
    }

    #[test]
    fn test_initialize_preseason_does_not_schedule_automatic_move_for_player() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(507);
        let plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");
        let player = driver_queries::get_player_driver(&conn).expect("player");

        assert!(
            plan.planned_events.iter().any(|event| matches!(
                &event.event,
                PendingAction::PlayerProposal { proposal } if proposal.piloto_id == player.id
            )),
            "o jogador deveria receber ao menos uma proposta planejada"
        );
        assert!(
            !plan.planned_events.iter().any(|event| matches!(
                &event.event,
                PendingAction::RenewContract { driver_id, .. } if driver_id == &player.id
            )),
            "o plano não deve renovar automaticamente o contrato do jogador"
        );
        assert!(
            !plan.planned_events.iter().any(|event| matches!(
                &event.event,
                PendingAction::Transfer { driver_id, .. } if driver_id == &player.id
            )),
            "o plano não deve agendar transferência automática para o jogador"
        );
        assert!(
            !plan.planned_events.iter().any(|event| matches!(
                &event.event,
                PendingAction::PlaceRookie { driver, .. } if driver.id == player.id
            )),
            "o plano não deve tratar o jogador como rookie para preencher vagas"
        );
    }

    #[test]
    fn test_sign_driver_to_team_rejects_driver_without_required_license() {
        let conn = setup_market_fixture();
        let free_driver = driver_queries::get_driver(&conn, "P005").expect("free driver");
        let team = team_queries::get_team_by_id(&conn, "T001")
            .expect("team query")
            .expect("team exists");

        let error = sign_driver_to_team(
            &conn,
            &free_driver.id,
            &free_driver.nome,
            &team.id,
            2,
            80_000.0,
            1,
            TeamRole::Numero2.as_str(),
        )
        .expect_err("signing should fail without required license");

        assert!(error.to_lowercase().contains("licenc"));

        let active_contracts: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM contracts WHERE piloto_id = ?1 AND status = 'Ativo'",
                [&free_driver.id],
                |row| row.get(0),
            )
            .expect("active contracts count");
        assert_eq!(active_contracts, 0);

        let refreshed = driver_queries::get_driver(&conn, &free_driver.id).expect("driver query");
        assert!(refreshed.categoria_atual.is_none());
    }

    #[test]
    fn test_sign_driver_to_team_rejects_invalid_role() {
        let conn = setup_market_fixture();
        let free_driver = driver_queries::get_driver(&conn, "P004").expect("free driver");
        let team = team_queries::get_team_by_id(&conn, "T002")
            .expect("team query")
            .expect("team exists");

        let error = sign_driver_to_team(
            &conn,
            &free_driver.id,
            &free_driver.nome,
            &team.id,
            2,
            80_000.0,
            1,
            "PapelInvalido",
        )
        .expect_err("signing should fail with invalid role");

        assert!(error.contains("TeamRole"));
    }

    #[test]
    fn test_transfer_week() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(508);
        let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

        while !plan.state.is_complete {
            let result = advance_week(&conn, &mut plan).expect("week should advance");
            if result
                .events
                .iter()
                .any(|event| event.event_type == MarketEventType::TransferCompleted)
            {
                break;
            }
        }

        let team = team_queries::get_team_by_id(&conn, "T002")
            .expect("team query")
            .expect("team");
        assert!(team.piloto_1_id.is_some() || team.piloto_2_id.is_some());
    }

    #[test]
    fn test_classify_market_movement_distinguishes_weekly_closing_types() {
        assert_eq!(
            classify_market_movement(&MarketEventType::RookieSigned, None, Some("mazda_rookie")),
            Some("rookie".to_string())
        );
        assert_eq!(
            classify_market_movement(&MarketEventType::RookieSigned, None, Some("gt4")),
            Some("signing".to_string())
        );
        assert_eq!(
            classify_market_movement(&MarketEventType::ContractExpired, Some("gt4"), None),
            Some("departure".to_string())
        );
        assert_eq!(
            classify_market_movement(&MarketEventType::TransferCompleted, None, Some("gt3")),
            Some("signing".to_string())
        );
        assert_eq!(
            classify_market_movement(
                &MarketEventType::TransferCompleted,
                Some("gt4"),
                Some("gt4")
            ),
            Some("lateral".to_string())
        );
        assert_eq!(
            classify_market_movement(
                &MarketEventType::TransferCompleted,
                Some("bmw_m2"),
                Some("gt4")
            ),
            Some("promotion".to_string())
        );
        assert_eq!(
            classify_market_movement(
                &MarketEventType::TransferCompleted,
                Some("gt4"),
                Some("bmw_m2")
            ),
            Some("relegation".to_string())
        );
        assert_eq!(
            classify_market_movement(&MarketEventType::ContractRenewed, Some("gt4"), Some("gt4")),
            Some("renewal".to_string())
        );
    }

    #[test]
    fn test_build_transfer_events_preserves_previous_category_for_weekly_closing() {
        let mut simulated_contracts_by_driver = std::collections::HashMap::new();
        let mut original_contracts_by_driver = std::collections::HashMap::new();
        let original = Contract::new(
            "C100".to_string(),
            "P100".to_string(),
            "Piloto Transfer".to_string(),
            "T100".to_string(),
            "Equipe Antiga".to_string(),
            1,
            1,
            80_000.0,
            TeamRole::Numero1,
            "gt4".to_string(),
        );
        let simulated = Contract::new(
            "C101".to_string(),
            "P100".to_string(),
            "Piloto Transfer".to_string(),
            "T200".to_string(),
            "Equipe Nova".to_string(),
            2,
            1,
            120_000.0,
            TeamRole::Numero1,
            "gt3".to_string(),
        );
        original_contracts_by_driver.insert("P100".to_string(), original);
        simulated_contracts_by_driver.insert("P100".to_string(), simulated);

        let signings = vec![crate::market::proposals::SigningInfo {
            driver_id: "P100".to_string(),
            driver_name: "Piloto Transfer".to_string(),
            team_id: "T200".to_string(),
            team_name: "Equipe Nova".to_string(),
            categoria: "gt3".to_string(),
            papel: TeamRole::Numero1.as_str().to_string(),
            tipo: "transferencia".to_string(),
        }];

        let events = build_transfer_events(
            &simulated_contracts_by_driver,
            &signings,
            &original_contracts_by_driver,
        )
        .expect("transfer events");

        assert!(matches!(
            &events[0].event,
            PendingAction::Transfer {
                from_categoria,
                ..
            } if from_categoria.as_deref() == Some("gt4")
        ));
    }

    #[test]
    fn test_market_event_position_uses_latest_category_standing_before_cached_best_result() {
        let conn = setup_market_fixture();
        conn.execute(
            "UPDATE drivers
             SET melhor_resultado_temp = 3
             WHERE id IN ('P001', 'P002')",
            [],
        )
        .expect("set duplicated cached best result");

        assert_eq!(
            market_event_championship_position(&conn, "P001", Some("gt4"), 2),
            Some(1)
        );
        assert_eq!(
            market_event_championship_position(&conn, "P002", Some("gt4"), 2),
            Some(4)
        );
    }

    #[test]
    fn test_preseason_reduces_vacancies_before_final_week() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(508);
        let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

        advance_week(&conn, &mut plan).expect("contract expiry week should advance");
        let second_week = advance_week(&conn, &mut plan).expect("second week should advance");
        let vacancies_before_transfers = second_week.remaining_vacancies;

        let transfer_week = advance_week(&conn, &mut plan).expect("transfer week should advance");

        assert!(
            transfer_week
                .events
                .iter()
                .any(|event| event.event_type == MarketEventType::TransferCompleted),
            "a pre-temporada deveria ter pelo menos uma contratacao antes da semana final"
        );
        assert!(
            transfer_week.remaining_vacancies < vacancies_before_transfers,
            "as vagas devem comecar a cair antes da ultima semana"
        );
    }

    #[test]
    fn test_rookie_placement_week() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(509);
        let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

        while !plan.state.is_complete {
            let result = advance_week(&conn, &mut plan).expect("week should advance");
            if result
                .events
                .iter()
                .any(|event| event.event_type == MarketEventType::RookieSigned)
            {
                break;
            }
        }

        let rookie_contracts: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM contracts WHERE temporada_inicio = 2 AND status = 'Ativo'",
                [],
                |row| row.get(0),
            )
            .expect("rookie contracts");
        assert!(rookie_contracts >= 2);
    }

    #[test]
    fn test_place_rookie_grants_required_license_before_signing() {
        let conn = setup_market_fixture();
        let rookie = sample_driver(
            "P999",
            "Rookie Planejado",
            Some("gt4"),
            55.0,
            DriverStatus::Ativo,
        );
        let mut plan = PreSeasonPlan {
            state: PreSeasonState {
                season_number: 2,
                current_week: 1,
                total_weeks: 1,
                phase: PreSeasonPhase::RookiePlacement,
                is_complete: false,
                player_has_pending_proposals: false,
                player_has_team: false,
                current_display_date: None,
            },
            planned_events: vec![PlannedEvent {
                week: 1,
                executed: false,
                event: PendingAction::PlaceRookie {
                    driver: rookie.clone(),
                    team_id: "T001".to_string(),
                    team_name: "Equipe A".to_string(),
                    salary: 80_000.0,
                    duration: 1,
                    role: TeamRole::Numero2.as_str().to_string(),
                },
            }],
            executed_weeks: Vec::new(),
        };

        let result = advance_week(&conn, &mut plan).expect("rookie placement should succeed");

        assert!(result.events.iter().any(|event| {
            event.event_type == MarketEventType::TransferCompleted
                && event.movement_kind.as_deref() == Some("signing")
        }));
        assert!(!result
            .events
            .iter()
            .any(|event| event.event_type == MarketEventType::RookieSigned));

        let license_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM licenses WHERE piloto_id = ?1 AND CAST(nivel AS INTEGER) >= 2",
                [&rookie.id],
                |row| row.get(0),
            )
            .expect("rookie license count");
        assert_eq!(license_count, 1);

        let active_contracts: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM contracts WHERE piloto_id = ?1 AND status = 'Ativo'",
                [&rookie.id],
                |row| row.get(0),
            )
            .expect("rookie active contract count");
        assert_eq!(active_contracts, 1);
    }

    #[test]
    fn test_all_teams_filled_after_complete() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(510);
        let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

        while !plan.state.is_complete {
            advance_week(&conn, &mut plan).expect("week should advance");
        }

        let teams = team_queries::get_all_teams(&conn).expect("teams");
        assert!(teams
            .iter()
            .all(|team| team.piloto_1_id.is_some() && team.piloto_2_id.is_some()));
    }

    #[test]
    fn test_plan_persistence() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(511);
        let plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");
        let temp_dir = unique_test_dir("preseason_persistence");
        fs::create_dir_all(&temp_dir).expect("temp dir");

        save_preseason_plan(&temp_dir, &plan).expect("plan should save");
        let loaded = load_preseason_plan(&temp_dir)
            .expect("plan should load")
            .expect("plan should exist");

        assert_eq!(loaded.state.season_number, plan.state.season_number);
        assert_eq!(loaded.state.total_weeks, plan.state.total_weeks);
        assert_eq!(loaded.planned_events.len(), plan.planned_events.len());

        delete_preseason_plan(&temp_dir).expect("delete plan");
        assert!(load_preseason_plan(&temp_dir)
            .expect("load after delete")
            .is_none());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_temp_preseason_clone_cleans_up_file_on_drop() {
        let conn = setup_market_fixture();
        let temp_path;
        let wal_path;
        let shm_path;

        {
            let clone = TempPreseasonClone::new(&conn).expect("temp clone");
            temp_path = clone.path().to_path_buf();
            wal_path = PathBuf::from(format!("{}-wal", temp_path.to_string_lossy()));
            shm_path = PathBuf::from(format!("{}-shm", temp_path.to_string_lossy()));

            assert!(
                temp_path.exists(),
                "temp clone should exist while guard is alive"
            );

            let contract_count: i64 = clone
                .connection()
                .query_row("SELECT COUNT(*) FROM contracts", [], |row| row.get(0))
                .expect("count contracts from temp clone");
            assert!(contract_count > 0, "temp clone should be readable");
        }

        assert!(
            !temp_path.exists(),
            "temp clone file should be removed after guard drop: {}",
            temp_path.display()
        );
        assert!(
            !wal_path.exists(),
            "temp clone wal file should be removed after guard drop: {}",
            wal_path.display()
        );
        assert!(
            !shm_path.exists(),
            "temp clone shm file should be removed after guard drop: {}",
            shm_path.display()
        );
    }

    #[test]
    fn test_next_preseason_clone_path_is_unique_on_rapid_calls() {
        let mut seen = std::collections::HashSet::new();

        for _ in 0..128 {
            let path = next_preseason_clone_path().expect("unique clone path");
            assert!(
                seen.insert(path.clone()),
                "clone path duplicado gerado em chamadas rapidas: {}",
                path.display()
            );
        }
    }

    #[test]
    fn test_advance_week_repairs_legacy_license_before_transfer() {
        let conn = setup_market_fixture();
        let mut team_rng = StdRng::seed_from_u64(513);
        let extra_team = sample_team("gt4", "T003", &mut team_rng);
        team_queries::insert_team(&conn, &extra_team).expect("extra team");
        conn.execute("DELETE FROM licenses WHERE piloto_id = 'P004'", [])
            .expect("remove legacy-corrected license");

        let mut plan = PreSeasonPlan {
            state: PreSeasonState {
                season_number: 2,
                current_week: 1,
                total_weeks: 1,
                phase: PreSeasonPhase::Transfers,
                is_complete: false,
                player_has_pending_proposals: false,
                player_has_team: false,
                current_display_date: None,
            },
            planned_events: vec![PlannedEvent {
                week: 1,
                executed: false,
                event: PendingAction::Transfer {
                    driver_id: "P004".to_string(),
                    driver_name: "Piloto D".to_string(),
                    from_team_id: None,
                    from_team_name: None,
                    from_categoria: None,
                    to_team_id: extra_team.id.clone(),
                    to_team_name: extra_team.nome.clone(),
                    salary: 110_000.0,
                    duration: 1,
                    role: TeamRole::Numero1.as_str().to_string(),
                },
            }],
            executed_weeks: Vec::new(),
        };

        let result = advance_week(&conn, &mut plan).expect("legacy transfer should succeed");

        assert!(result
            .events
            .iter()
            .any(|event| event.event_type == MarketEventType::TransferCompleted));
        assert!(
            driver_has_required_license_for_category(&conn, "P004", "gt4")
                .expect("repaired gt4 license"),
            "a execucao da pre-temporada deve recuperar saves legados antes de assinar"
        );
        let active_contracts: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM contracts
                 WHERE piloto_id = 'P004' AND equipe_id = 'T003' AND status = 'Ativo'",
                [],
                |row| row.get(0),
            )
            .expect("signed contract count");
        assert_eq!(active_contracts, 1);
    }

    #[test]
    fn test_cannot_advance_after_complete() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(512);
        let mut plan = initialize_preseason(&conn, 2, &mut rng).expect("plan should be created");

        while !plan.state.is_complete {
            advance_week(&conn, &mut plan).expect("week should advance");
        }

        let error = advance_week(&conn, &mut plan).expect_err("should reject after complete");
        assert!(error.contains("completa"));
    }

    fn setup_market_fixture() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        let previous = Season::new("S001".to_string(), 1, 2024);
        let next = Season::new("S002".to_string(), 2, 2025);
        season_queries::insert_season(&conn, &previous).expect("previous season");
        season_queries::finalize_season(&conn, &previous.id).expect("finalize previous");
        season_queries::insert_season(&conn, &next).expect("next season");

        let mut team_rng = StdRng::seed_from_u64(200);
        let team_a = sample_team("gt4", "T001", &mut team_rng);
        let team_b = sample_team("gt4", "T002", &mut team_rng);
        team_queries::insert_team(&conn, &team_a).expect("team a");
        team_queries::insert_team(&conn, &team_b).expect("team b");

        let driver_a = sample_driver("P001", "Piloto A", Some("gt4"), 78.0, DriverStatus::Ativo);
        let driver_b = sample_driver("P002", "Piloto B", Some("gt4"), 66.0, DriverStatus::Ativo);
        let driver_c = sample_driver(
            "P003",
            "Piloto C",
            Some("gt4"),
            62.0,
            DriverStatus::Aposentado,
        );
        let driver_d = sample_driver("P004", "Piloto D", Some("gt4"), 74.0, DriverStatus::Ativo);
        let driver_e = sample_driver("P005", "Piloto E", None, 59.0, DriverStatus::Ativo);
        let driver_f = sample_driver("P006", "Piloto F", Some("gt3"), 76.0, DriverStatus::Ativo);
        let mut player = sample_driver("P007", "Jogador", Some("gt4"), 72.0, DriverStatus::Ativo);
        player.is_jogador = true;
        for driver in [
            &driver_a, &driver_b, &driver_c, &driver_d, &driver_e, &driver_f, &player,
        ] {
            driver_queries::insert_driver(&conn, driver).expect("insert driver");
        }

        let contract_a = Contract::new(
            "C001".to_string(),
            driver_a.id.clone(),
            driver_a.nome.clone(),
            team_a.id.clone(),
            team_a.nome.clone(),
            1,
            1,
            140_000.0,
            TeamRole::Numero1,
            "gt4".to_string(),
        );
        let contract_b = Contract::new(
            "C002".to_string(),
            driver_b.id.clone(),
            driver_b.nome.clone(),
            team_a.id.clone(),
            team_a.nome.clone(),
            1,
            1,
            95_000.0,
            TeamRole::Numero2,
            "gt4".to_string(),
        );
        let contract_c = Contract::new(
            "C003".to_string(),
            driver_c.id.clone(),
            driver_c.nome.clone(),
            team_b.id.clone(),
            team_b.nome.clone(),
            1,
            2,
            85_000.0,
            TeamRole::Numero1,
            "gt4".to_string(),
        );
        let contract_d = Contract::new(
            "C004".to_string(),
            player.id.clone(),
            player.nome.clone(),
            team_b.id.clone(),
            team_b.nome.clone(),
            1,
            1,
            90_000.0,
            TeamRole::Numero2,
            "gt4".to_string(),
        );
        contract_queries::insert_contract(&conn, &contract_a).expect("contract a");
        contract_queries::insert_contract(&conn, &contract_b).expect("contract b");
        contract_queries::insert_contract(&conn, &contract_c).expect("contract c");
        contract_queries::insert_contract(&conn, &contract_d).expect("contract d");

        team_queries::update_team_pilots(&conn, &team_a.id, Some(&driver_a.id), Some(&driver_b.id))
            .expect("team a pilots");
        team_queries::update_team_pilots(&conn, &team_b.id, Some(&driver_c.id), Some(&player.id))
            .expect("team b pilots");

        insert_standing(
            &conn,
            &previous.id,
            &driver_a.id,
            &team_a.id,
            "gt4",
            1,
            120.0,
            3,
            2,
        );
        insert_standing(
            &conn,
            &previous.id,
            &driver_b.id,
            &team_a.id,
            "gt4",
            4,
            72.0,
            1,
            1,
        );
        insert_standing(
            &conn,
            &previous.id,
            &driver_c.id,
            &team_b.id,
            "gt4",
            6,
            40.0,
            0,
            0,
        );
        insert_standing(
            &conn,
            &previous.id,
            &driver_d.id,
            &team_b.id,
            "gt4",
            2,
            96.0,
            2,
            1,
        );
        insert_standing(
            &conn,
            &previous.id,
            &driver_f.id,
            &team_a.id,
            "gt3",
            3,
            88.0,
            1,
            2,
        );
        insert_standing(
            &conn,
            &previous.id,
            &player.id,
            &team_b.id,
            "gt4",
            5,
            60.0,
            0,
            0,
        );

        // Licenças — necessárias para que o filtro de mercado não bloqueie os pilotos.
        // gt4 exige nível 2, gt3 exige nível 3.
        for (piloto_id, nivel) in [
            ("P001", 2),
            ("P002", 2),
            ("P003", 2),
            ("P004", 2),
            ("P005", 0),
            ("P006", 3),
            ("P007", 2),
        ] {
            conn.execute(
                "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
                 VALUES (?1, ?2, 'gt4', '2024', 3)",
                params![piloto_id, nivel.to_string()],
            )
            .expect("insert license");
        }

        conn.execute(
            "UPDATE meta SET value = '5' WHERE key = 'next_contract_id'",
            [],
        )
        .expect("contract counter");
        conn.execute(
            "UPDATE meta SET value = '8' WHERE key = 'next_driver_id'",
            [],
        )
        .expect("driver counter");

        conn
    }

    fn sample_team(category: &str, id: &str, rng: &mut StdRng) -> crate::models::team::Team {
        let template = get_team_templates(category)[0];
        crate::models::team::Team::from_template_with_rng(
            template,
            category,
            id.to_string(),
            2025,
            rng,
        )
    }

    fn sample_driver(
        id: &str,
        name: &str,
        category: Option<&str>,
        skill: f64,
        status: DriverStatus,
    ) -> Driver {
        let mut driver = Driver::new(
            id.to_string(),
            name.to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2020,
        );
        driver.categoria_atual = category.map(str::to_string);
        driver.status = status;
        driver.atributos.skill = skill;
        driver.atributos.consistencia = 68.0;
        driver.stats_temporada.vitorias = 1;
        driver.stats_temporada.poles = 1;
        driver.stats_carreira.titulos = 1;
        driver
    }

    fn insert_standing(
        conn: &Connection,
        season_id: &str,
        driver_id: &str,
        team_id: &str,
        category: &str,
        position: i32,
        points: f64,
        wins: i32,
        poles: i32,
    ) {
        conn.execute(
            "INSERT INTO standings (
                temporada_id, piloto_id, equipe_id, categoria, posicao, pontos, vitorias, podios, poles, corridas
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![season_id, driver_id, team_id, category, position, points, wins, wins + 1, poles, 8],
        )
        .expect("insert standing");
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("iracerapp_{label}_{nanos}"))
    }

    // ── Testes do handler UpdateHierarchy ──

    fn update_hierarchy_action(
        team_id: &str,
        n1_id: Option<&str>,
        n2_id: Option<&str>,
    ) -> PendingAction {
        PendingAction::UpdateHierarchy {
            team_id: team_id.to_string(),
            team_name: "Equipe Teste".to_string(),
            n1_id: n1_id.map(str::to_string),
            n1_name: n1_id.unwrap_or("").to_string(),
            n2_id: n2_id.map(str::to_string),
            n2_name: n2_id.unwrap_or("").to_string(),
            prev_n1_id: None,
            prev_n2_id: None,
            prev_tensao: 0.0,
            prev_status: "estavel".to_string(),
            prev_categoria: "gt4".to_string(),
        }
    }

    #[test]
    fn test_update_hierarchy_valid_lineup_persists() {
        let conn = setup_market_fixture();
        let action = update_hierarchy_action("T001", Some("P001"), Some("P002"));
        let mut events = Vec::new();
        let mut proposals = Vec::new();

        execute_action(&conn, "S002", 2, &action, &mut events, &mut proposals)
            .expect("ação deve executar com lineup válido");

        let team = team_queries::get_team_by_id(&conn, "T001")
            .unwrap()
            .unwrap();
        assert_eq!(team.hierarquia_n1_id.as_deref(), Some("P001"));
        assert_eq!(team.hierarquia_n2_id.as_deref(), Some("P002"));
        assert_eq!(team.piloto_1_id.as_deref(), Some("P001"));
        assert_eq!(team.piloto_2_id.as_deref(), Some("P002"));
        // Contadores devem ter sido resetados
        assert_eq!(team.hierarquia_duelos_total, 0);
    }

    #[test]
    fn test_update_hierarchy_rejects_missing_n1() {
        let conn = setup_market_fixture();
        let action = update_hierarchy_action("T001", None, Some("P002"));
        let mut events = Vec::new();
        let mut proposals = Vec::new();

        let err =
            execute_action(&conn, "S002", 2, &action, &mut events, &mut proposals).unwrap_err();
        assert!(err.contains("N1 ausente"), "erro inesperado: {err}");
    }

    #[test]
    fn test_update_hierarchy_rejects_same_driver_for_n1_n2() {
        let conn = setup_market_fixture();
        let action = update_hierarchy_action("T001", Some("P001"), Some("P001"));
        let mut events = Vec::new();
        let mut proposals = Vec::new();

        let err =
            execute_action(&conn, "S002", 2, &action, &mut events, &mut proposals).unwrap_err();
        assert!(err.contains("mesmo piloto"), "erro inesperado: {err}");
    }

    // ── Testes de fechamento do macrobloco (contrato completo) ──

    #[test]
    fn test_same_pair_preserves_tensao_and_resets_counters() {
        let conn = setup_market_fixture();
        // Configurar estado hierárquico pré-existente com valores não-zero
        conn.execute(
            "UPDATE teams SET hierarquia_n1_id='P001', hierarquia_n2_id='P002',
             hierarquia_tensao=45.0, hierarquia_status='tensao',
             hierarquia_duelos_total=5, hierarquia_duelos_n2_vencidos=3,
             hierarquia_sequencia_n2=2, hierarquia_sequencia_n1=1,
             hierarquia_inversoes_temporada=1 WHERE id='T001'",
            [],
        )
        .expect("setup hierarchy state");

        let action = PendingAction::UpdateHierarchy {
            team_id: "T001".to_string(),
            team_name: "Equipe Teste".to_string(),
            n1_id: Some("P001".to_string()),
            n1_name: "Piloto A".to_string(),
            n2_id: Some("P002".to_string()),
            n2_name: "Piloto B".to_string(),
            // Mesma dupla, mesma categoria → PartialPreserve
            prev_n1_id: Some("P001".to_string()),
            prev_n2_id: Some("P002".to_string()),
            prev_tensao: 45.0,
            prev_status: "tensao".to_string(),
            prev_categoria: "gt4".to_string(),
        };
        let mut events = Vec::new();
        let mut proposals = Vec::new();
        execute_action(&conn, "S002", 2, &action, &mut events, &mut proposals)
            .expect("ação deve executar");

        let team = team_queries::get_team_by_id(&conn, "T001")
            .unwrap()
            .unwrap();
        // Tensao e status preservados
        assert!(
            (team.hierarquia_tensao - 45.0).abs() < f64::EPSILON,
            "tensao deve ser 45.0, foi {}",
            team.hierarquia_tensao
        );
        assert_eq!(team.hierarquia_status, "tensao");
        // Todos os 5 counters sazonais resetados
        assert_eq!(team.hierarquia_duelos_total, 0);
        assert_eq!(team.hierarquia_duelos_n2_vencidos, 0);
        assert_eq!(team.hierarquia_sequencia_n2, 0);
        assert_eq!(team.hierarquia_sequencia_n1, 0);
        assert_eq!(team.hierarquia_inversoes_temporada, 0);
    }

    #[test]
    fn test_changed_pilot_resets_hierarchy_to_defaults() {
        let conn = setup_market_fixture();
        // Configurar estado hierárquico pré-existente com tensao não-zero
        conn.execute(
            "UPDATE teams SET hierarquia_n1_id='P001', hierarquia_n2_id='P002',
             hierarquia_tensao=60.0, hierarquia_status='crise',
             hierarquia_duelos_total=8 WHERE id='T001'",
            [],
        )
        .expect("setup hierarchy state");

        let action = PendingAction::UpdateHierarchy {
            team_id: "T001".to_string(),
            team_name: "Equipe Teste".to_string(),
            n1_id: Some("P001".to_string()),
            n1_name: "Piloto A".to_string(),
            n2_id: Some("P004".to_string()), // piloto N2 mudou: P002 → P004
            n2_name: "Piloto D".to_string(),
            prev_n1_id: Some("P001".to_string()),
            prev_n2_id: Some("P002".to_string()),
            prev_tensao: 60.0,
            prev_status: "crise".to_string(),
            prev_categoria: "gt4".to_string(),
        };
        let mut events = Vec::new();
        let mut proposals = Vec::new();
        execute_action(&conn, "S002", 2, &action, &mut events, &mut proposals)
            .expect("ação deve executar");

        let team = team_queries::get_team_by_id(&conn, "T001")
            .unwrap()
            .unwrap();
        // FullReset: tensao e status resetados para defaults seguros
        assert!(
            (team.hierarquia_tensao - 0.0).abs() < f64::EPSILON,
            "tensao deve ser 0.0, foi {}",
            team.hierarquia_tensao
        );
        assert_eq!(team.hierarquia_status, "estavel");
        assert_eq!(team.hierarquia_duelos_total, 0);
    }

    #[test]
    fn test_rivalry_unchanged_after_hierarchy_update() {
        let conn = setup_market_fixture();
        // Inserir rivalidade entre P001 e P002 com intensidade conhecida
        conn.execute(
            "INSERT INTO rivalries (id, piloto1_id, piloto2_id, intensidade, tipo, criado_em, ultima_atualizacao)
             VALUES ('R001', 'P001', 'P002', 50.0, 'Normal', '2024', '2024')",
            [],
        )
        .expect("insert rivalry");

        // Executar transição de hierarquia para equipe com P001+P002
        let action = update_hierarchy_action("T001", Some("P001"), Some("P002"));
        let mut events = Vec::new();
        let mut proposals = Vec::new();
        execute_action(&conn, "S002", 2, &action, &mut events, &mut proposals)
            .expect("ação deve executar");

        // Rivalidade deve permanecer intacta — hierarquia não toca a tabela rivalries
        let intensidade: f64 = conn
            .query_row(
                "SELECT intensidade FROM rivalries WHERE id='R001'",
                [],
                |row| row.get(0),
            )
            .expect("rivalidade deve ainda existir");
        assert!(
            (intensidade - 50.0).abs() < f64::EPSILON,
            "intensidade deve ser 50.0, foi {}",
            intensidade
        );
    }
}
