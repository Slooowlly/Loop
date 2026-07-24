use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Local};
use rand::Rng;
use rusqlite::{params, Connection, OptionalExtension};

use crate::constants::categories::{
    get_all_categories, get_category_config, get_feeder_categories, get_target_categories,
    runs_in_special_phase, uses_regular_contracts,
};
use crate::constants::historical_timeline::is_category_active_in_year;
use crate::constants::skill_ranges;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::teams as team_queries;
use crate::evolution::rookies::generate_rookies;

// Contadores de preenchimento de emergencia (escassez na escada fechada). Lidos
// pelo harness sim_stats. Custo despresivel: dois atomics por finalize.
pub static EMERGENCY_PROMOTIONS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
pub static EMERGENCY_ROOKIES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Caminho de cada promoção de emergência: (tier de origem do piloto, tier da
/// vaga). Mostra de onde vem (feeder) e pra onde vai (a escassez).
pub static EMERGENCY_PROMO_PATHS: std::sync::Mutex<Vec<(u8, u8)>> =
    std::sync::Mutex::new(Vec::new());

use crate::generators::ids::{next_id, IdType};
use crate::market::driver_ai::evaluate_proposal;
use crate::market::evaluation::{estimate_expected_position, evaluate_driver_performance};
use crate::market::proposals::{
    is_real_career_debut_category, is_rookie_market_candidate, MarketProposal, MarketReport,
    ProposalStatus, SigningInfo, Vacancy,
};
use crate::market::renewal::should_renew_contract;
use crate::market::slam_ambition::{self, SlamDecision};
use crate::market::sync::sync_team_slots_from_active_regular_contracts;
use crate::market::team_ai::{calculate_offer_salary, generate_team_proposals, AvailableDriver};
use crate::market::visibility::calculate_visibility;
use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::enums::{ContractStatus, DriverStatus, PrimaryPersonality, TeamRole};
use crate::models::license::{
    ensure_driver_can_join_division, grant_driver_license_for_division_if_needed,
    repair_missing_licenses_for_current_categories, required_license_for_division,
};
use crate::models::team::TeamHierarchyClimate;

// Etapas da entressafra. Este arquivo guarda só a orquestração de alto nível
// (`run_market_inner` + o preenchimento final); cada etapa mora no seu módulo e
// enxerga os imports acima via `use super::*`.
mod assedio;
mod assedio_jogador;
mod comum;
mod consolidacao;
mod contratacao;
mod estado;
mod janela;
mod jogador;
mod slam;
mod vagas;

// Onde há callsite fora do pipeline o glob é `pub(crate)`: assim
// `market::pipeline::run_poaching_pass` (e afins) continua resolvendo.
pub(crate) use assedio::*;
pub(crate) use assedio_jogador::*;
pub(crate) use consolidacao::*;
pub(crate) use contratacao::*;
pub(crate) use jogador::*;
pub(crate) use slam::*;
use comum::*;
use estado::*;
use janela::*;
use vagas::*;

/// Instrumentação do harness de estatística: contadores de preenchimento de
/// emergência da escada fechada — promoções concedidas sem mérito por escassez,
/// e rookies gerados sem feeder. Incrementados onde essa lógica ocorre; lidos
/// pelo Monte Carlo em sim_stats. (Declaração mínima; ligar os incrementos.)

/// Mercado completo (pré-passes + Janela IA + propostas + rookies). Resolve tudo de
/// uma vez. A pré-temporada interativa NÃO usa isto (usa `run_market_prepasses` + a
/// Janela ao vivo); fica como cobertura de teste do wiring completo do mercado.
#[cfg_attr(not(test), allow(dead_code))]
pub fn run_market(
    conn: &Connection,
    new_season_number: i32,
    rng: &mut impl Rng,
) -> Result<MarketReport, String> {
    run_market_inner(conn, new_season_number, rng, true)
}

/// Apenas as PRÉ-PASSES aplicadas DE VERDADE (expira contratos, renova slam-aware,
/// rebaixa por mérito) — NÃO resolve o mercado. A pré-temporada interativa usa isto
/// e inicia a Janela de Transferências no lugar do mercado instantâneo.
pub(crate) fn run_market_prepasses(
    conn: &Connection,
    new_season_number: i32,
    rng: &mut impl Rng,
) -> Result<MarketReport, String> {
    run_market_inner(conn, new_season_number, rng, false)
}

fn run_market_inner(
    conn: &Connection,
    new_season_number: i32,
    rng: &mut impl Rng,
    resolve_market: bool,
) -> Result<MarketReport, String> {
    with_savepoint(conn, "market_run", || {
        let new_season = get_season_by_number(conn, new_season_number)?
            .ok_or_else(|| format!("Temporada {new_season_number} nao encontrada"))?;
        let previous_season = get_season_by_number(conn, new_season_number - 1)?;

        let mut report = MarketReport::default();
        reset_market_state(conn, &new_season.id)?;
        repair_missing_licenses_for_current_categories(conn)?;
        // Modelo fechado: o pool de agentes livres NÃO é reabastecido. As vagas são
        // preenchidas pela escada (promoção da categoria de baixo) com rookies gerados
        // só na base — ver fill_remaining_vacancies_with_rookies.

        let all_drivers = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao carregar pilotos: {e}"))?;
        let drivers_by_id: HashMap<String, Driver> = all_drivers
            .iter()
            .cloned()
            .map(|driver| (driver.id.clone(), driver))
            .collect();
        let teams = team_queries::get_all_teams(conn)
            .map_err(|e| format!("Falha ao carregar equipes: {e}"))?;
        let teams_by_id: HashMap<String, crate::models::team::Team> = teams
            .iter()
            .cloned()
            .map(|team| (team.id.clone(), team))
            .collect();
        let active_contracts_before = contract_queries::get_all_active_regular_contracts(conn)
            .map_err(|e| format!("Falha ao carregar contratos ativos: {e}"))?;
        let expiring_contracts: Vec<Contract> = active_contracts_before
            .iter()
            .filter(|contract| contract.temporada_fim < new_season_number)
            .cloned()
            .collect();
        let expiring_by_driver: HashMap<String, Contract> = expiring_contracts
            .iter()
            .cloned()
            .map(|contract| (contract.piloto_id.clone(), contract))
            .collect();

        report.contracts_expired =
            contract_queries::expire_ending_contracts(conn, new_season_number - 1)
                .map_err(|e| format!("Falha ao expirar contratos: {e}"))?;

        let retired_contract_ids: Vec<String> = active_contracts_before
            .iter()
            .filter(|contract| {
                drivers_by_id
                    .get(&contract.piloto_id)
                    .is_some_and(|driver| driver.status == DriverStatus::Aposentado)
            })
            .map(|contract| contract.id.clone())
            .collect();
        for contract_id in &retired_contract_ids {
            contract_queries::update_contract_status(
                conn,
                contract_id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| format!("Falha ao rescindir contrato de aposentado: {e}"))?;
        }
        report.retirements_replaced = retired_contract_ids.len() as i32;

        let standings_by_driver = load_market_contexts(
            conn,
            previous_season.as_ref().map(|season| season.id.as_str()),
            &drivers_by_id,
            &expiring_by_driver,
        )?;
        let mut player_was_expiring = false;

        for contract in &expiring_contracts {
            let Some(driver) = drivers_by_id.get(&contract.piloto_id) else {
                continue;
            };
            if driver.status != DriverStatus::Ativo {
                continue;
            }
            if driver.is_jogador {
                player_was_expiring = true;
                continue;
            }

            let Some(team) = teams_by_id.get(&contract.equipe_id) else {
                continue;
            };
            // Slam-chaser que mira OUTRA categoria recusa a renovação → vira agente
            // livre para o passe prioritário colocá-lo na categoria-alvo. (Stay ou
            // alvo = categoria atual → renova normalmente.)
            if let Some((target_category, _)) = slam_target_category(conn, driver)? {
                if target_category != team.categoria {
                    continue;
                }
            }
            let context = standings_by_driver
                .get(&driver.id)
                .cloned()
                .unwrap_or_else(|| default_market_context(driver));
            let expected_position =
                estimate_expected_position(team.car_strength(), context.total_pilotos.max(1));
            let performance_score = evaluate_driver_performance(
                context.posicao_campeonato,
                context.total_pilotos,
                context.vitorias,
                driver.atributos.consistencia,
                expected_position,
            );
            let decision = should_renew_contract(driver, performance_score, contract, team, rng);
            // Ideia 4: Vínculo + Foco por cima da decisão-base (buffer de confiança da
            // relação + contrato de projeto plurianual). É o "segurar pra fazer história"
            // rodando na grade da IA (o jogador decide na janela). Falha na leitura →
            // neutro (vínculo 0 / foco meio-de-grid), sem alterar o comportamento base.
            let vinculo = crate::market::bond::get_bond(conn, &driver.id, &team.id).unwrap_or(0.0);
            let foco = crate::finance::focus::get_focus(conn, &team.id)
                .map(|(f, _)| f)
                .unwrap_or(crate::finance::focus::TeamFocus::MeioDeGrid);
            let decision = crate::market::renewal::apply_bond_and_focus_to_renewal(
                decision,
                driver,
                performance_score,
                contract,
                team,
                vinculo,
                foco,
            );
            if !decision.should_renew {
                continue;
            }

            let mut new_contract = Contract::new(
                next_id(conn, IdType::Contract)
                    .map_err(|e| format!("Falha ao gerar ID de contrato: {e}"))?,
                driver.id.clone(),
                driver.nome.clone(),
                team.id.clone(),
                team.nome.clone(),
                new_season_number,
                decision.new_duration.unwrap_or(1),
                decision.new_salary.unwrap_or(contract.salario_anual),
                decision
                    .new_role
                    .clone()
                    .unwrap_or_else(|| contract.papel.clone()),
                team.categoria.clone(),
            );
            new_contract.classe = team.classe.clone();
            contract_queries::insert_contract(conn, &new_contract)
                .map_err(|e| format!("Falha ao inserir renovacao: {e}"))?;
            report.contracts_renewed += 1;
            report.new_signings.push(SigningInfo {
                driver_id: driver.id.clone(),
                driver_name: driver.nome.clone(),
                team_id: team.id.clone(),
                team_name: team.nome.clone(),
                categoria: team.categoria.clone(),
                papel: new_contract.papel.as_str().to_string(),
                tipo: "renovacao".to_string(),
            });
        }

        // Rebaixamento por mérito (troca conservadora) antes de preencher vagas.
        apply_merit_relegations(
            conn,
            &teams,
            new_season_number,
            &standings_by_driver,
            &mut report,
        )?;

        // Poaching / quebra de contrato (Fase 2b.1, só IA): times com caixa arrancam
        // astros contratados da mesma categoria pagando a multa. Abre vagas que a
        // escada preenche depois.
        run_poaching_pass(
            conn,
            &teams,
            new_season_number,
            rng,
            &mut report,
            &mut Vec::new(),
        )?;

        // (Flag IRACER_ROOKIE_MERIT) Subida garantida do campeão do Rookie: cobre o
        // caso "Amador cheio" forçando a troca com o pior do Amador. Sem a flag é
        // no-op; com vaga natural no Amador, o fluxo normal já promove o campeão.
        guarantee_rookie_champion_promotions(
            conn,
            &teams,
            new_season_number,
            &standings_by_driver,
            &mut report,
        )?;

        let mut refreshed_drivers = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao recarregar pilotos: {e}"))?;
        let mut refreshed_by_id: HashMap<String, Driver> = refreshed_drivers
            .iter()
            .cloned()
            .map(|driver| (driver.id.clone(), driver))
            .collect();
        sync_team_slots(conn, &teams, &refreshed_by_id)?;
        if resolve_market {
            let initial_vacancies = find_vacancies(conn)?;
            let mut available = find_available_drivers(conn, &standings_by_driver)?;

            // ── Janela de Transferências (motor IA-only): leilão de dois lados que
            // substitui o casamento guloso vaga-por-vaga. Preenche as vagas com os
            // agentes livres; o que sobrar vai pra rookies/promoção depois. Absorve o
            // slam-chasing (a categoria-alvo vira bônus no score do piloto). ──
            apply_weekly_market(
                conn,
                &initial_vacancies,
                &mut available,
                new_season_number,
                rng,
                &mut report,
            )?;
            refreshed_drivers = driver_queries::get_all_drivers(conn)
                .map_err(|e| format!("Falha ao recarregar pilotos apos a janela: {e}"))?;
            refreshed_by_id = refreshed_drivers
                .iter()
                .cloned()
                .map(|driver| (driver.id.clone(), driver))
                .collect();
            sync_team_slots(conn, &teams, &refreshed_by_id)?;

            let player_proposals = generate_player_proposals(
                conn,
                &new_season.id,
                new_season_number,
                &find_vacancies(conn)?,
                player_was_expiring,
                &standings_by_driver,
                rng,
            )?;
            report.proposals_made += player_proposals.len() as i32;
            report.player_proposals = player_proposals;

            fill_remaining_vacancies_with_rookies(
                conn,
                &teams,
                new_season_number,
                &mut report,
                rng,
                None,
                &HashSet::new(),
            )?;

            refreshed_drivers = driver_queries::get_all_drivers(conn)
                .map_err(|e| format!("Falha ao recarregar pilotos finais: {e}"))?;
            refreshed_by_id = refreshed_drivers
                .into_iter()
                .map(|driver| (driver.id.clone(), driver))
                .collect();
            refresh_team_hierarchy(conn, &teams, &refreshed_by_id)?;
            report.unresolved_vacancies = find_vacancies(conn)?.len() as i32;
        }

        persist_market_state(conn, &new_season.id)?;
        Ok(report)
    })
}

/// Escaneia todas as equipes de categorias regulares e garante que tenham 2 pilotos.
/// Caso faltem pilotos, preenche com novos rookies (rookies são gerados e contratados).
pub fn fill_all_remaining_vacancies(
    conn: &Connection,
    new_season_number: i32,
    rng: &mut impl Rng,
) -> Result<(), String> {
    let mut report = MarketReport::default();
    fill_all_remaining_vacancies_reported(conn, new_season_number, rng, &mut report)
}

/// Idêntica a [`fill_all_remaining_vacancies`], mas acumula as assinaturas no
/// `report` do chamador — necessário para que o preenchimento final (última
/// semana do mercado) apareça no feed em vez de sumir num report descartado.
pub(crate) fn fill_all_remaining_vacancies_reported(
    conn: &Connection,
    new_season_number: i32,
    rng: &mut impl Rng,
    report: &mut MarketReport,
) -> Result<(), String> {
    let teams = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao carregar equipes para preenchimento final: {e}"))?;
    let debut_year = get_season_by_number(conn, new_season_number)?
        .map(|season| season.ano)
        .unwrap_or_else(|| Local::now().year());
    let fillable = |vacancy: &Vacancy| {
        is_regular_vacancy(vacancy) && is_category_active_in_year(&vacancy.categoria, debut_year)
    };

    loop {
        let current_drivers = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao recarregar pilotos: {e}"))?;
        let current_by_id: HashMap<String, Driver> = current_drivers
            .iter()
            .cloned()
            .map(|driver| (driver.id.clone(), driver))
            .collect();

        sync_team_slots(conn, &teams, &current_by_id)?;
        let regular_vacancies: Vec<_> =
            find_vacancies(conn)?.into_iter().filter(fillable).collect();

        if regular_vacancies.is_empty() {
            break;
        }

        // Acumula no report do chamador (cada iteração apenda novas assinaturas).
        fill_remaining_vacancies_with_rookies(
            conn,
            &teams,
            new_season_number,
            report,
            rng,
            None,
            &HashSet::new(),
        )?;

        // Se após tentar preencher ainda persistirem as mesmas vagas (ex: erro na geração), quebra para evitar loop infinito
        let final_vacancies = find_vacancies(conn)?.into_iter().filter(fillable).count();
        if final_vacancies >= regular_vacancies.len() {
            break;
        }
    }

    let current_drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao recarregar pilotos para sincronizacao final: {e}"))?;
    let current_by_id: HashMap<String, Driver> = current_drivers
        .iter()
        .cloned()
        .map(|driver| (driver.id.clone(), driver))
        .collect();
    sync_team_slots(conn, &teams, &current_by_id)?;

    Ok(())
}

// ─── Janela de Transferências INTERATIVA (Fase 2): persistência + orquestração ───

// ============================================================================
// Fase 2b.3 — QUEBRA DE CONTRATO DO JOGADOR (o leilão que o jogador VÊ e decide).
// ============================================================================
//
// O poaching IA-vs-IA (2b.1/2b.2) roda auto-resolvido nas pré-passes e NUNCA toca o
// jogador. Aqui é o inverso: um time claramente melhor bate à porta do jogador
// CONTRATADO, dispara o leilão de salário (mesmo motor), mas em vez de executar,
// devolve o negócio pra UI — e a PALAVRA FINAL é do jogador (Sair ou Ficar).

#[cfg(test)]
#[path = "pipeline/tests/mod.rs"]
mod tests;
