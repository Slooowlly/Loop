use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Local};
use rand::Rng;
use rusqlite::{params, Connection, OptionalExtension};

use crate::constants::categories::{
    get_all_categories, get_category_config, get_feeder_categories, runs_in_special_phase,
    uses_regular_contracts,
};
use crate::constants::historical_timeline::is_category_active_in_year;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::teams as team_queries;
use crate::evolution::rookies::generate_rookies;

// Contadores de preenchimento de emergencia (escassez na escada fechada). Lidos
// pelo harness sim_stats. Custo despresivel: dois atomics por finalize.
pub static EMERGENCY_PROMOTIONS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
pub static EMERGENCY_ROOKIES: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
/// Caminho de cada promoção de emergência: (tier de origem do piloto, tier da
/// vaga). Mostra de onde vem (feeder) e pra onde vai (a escassez).
pub static EMERGENCY_PROMO_PATHS: std::sync::Mutex<Vec<(u8, u8)>> =
    std::sync::Mutex::new(Vec::new());

use crate::generators::ids::{next_id, IdType};
use crate::market::driver_ai::evaluate_proposal;
use crate::market::evaluation::{estimate_expected_position, evaluate_driver_performance};
use crate::market::proposals::{
    is_real_career_debut_category, is_rookie_market_candidate, MarketProposal, MarketReport,
    SigningInfo, Vacancy,
};
use crate::market::renewal::should_renew_contract;
use crate::market::sync::sync_team_slots_from_active_regular_contracts;
use crate::market::team_ai::{generate_team_proposals, AvailableDriver};
use crate::market::visibility::calculate_visibility;
use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::enums::{ContractStatus, DriverStatus, TeamRole};
use crate::models::license::{
    ensure_driver_can_join_division, grant_driver_license_for_division_if_needed,
    repair_missing_licenses_for_current_categories, required_license_for_division,
};
use crate::models::team::TeamHierarchyClimate;

/// Instrumentação do harness de estatística: contadores de preenchimento de
/// emergência da escada fechada — promoções concedidas sem mérito por escassez,
/// e rookies gerados sem feeder. Incrementados onde essa lógica ocorre; lidos
/// pelo Monte Carlo em sim_stats. (Declaração mínima; ligar os incrementos.)

#[derive(Debug, Clone)]
struct DriverMarketContext {
    posicao_campeonato: i32,
    total_pilotos: i32,
    categoria: String,
    category_tier: u8,
    vitorias: i32,
    poles: i32,
    titulos: i32,
    papel: TeamRole,
}

fn with_savepoint<T, F>(conn: &Connection, name: &str, action: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    conn.execute_batch(&format!("SAVEPOINT {name}"))
        .map_err(|e| format!("Falha ao abrir savepoint '{name}': {e}"))?;

    match action() {
        Ok(value) => {
            conn.execute_batch(&format!("RELEASE SAVEPOINT {name}"))
                .map_err(|e| format!("Falha ao confirmar savepoint '{name}': {e}"))?;
            Ok(value)
        }
        Err(err) => {
            conn.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {name}; RELEASE SAVEPOINT {name};"
            ))
            .map_err(|rollback_err| {
                format!("{err}; alem disso falhou o rollback do savepoint '{name}': {rollback_err}")
            })?;
            Err(err)
        }
    }
}

pub fn run_market(
    conn: &Connection,
    new_season_number: i32,
    rng: &mut impl Rng,
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
            let context = standings_by_driver
                .get(&driver.id)
                .cloned()
                .unwrap_or_else(|| default_market_context(driver));
            let expected_position =
                estimate_expected_position(team.car_performance, context.total_pilotos.max(1));
            let performance_score = evaluate_driver_performance(
                context.posicao_campeonato,
                context.total_pilotos,
                context.vitorias,
                driver.atributos.consistencia,
                expected_position,
            );
            let decision = should_renew_contract(driver, performance_score, contract, team, rng);
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
        apply_merit_relegations(conn, &teams, new_season_number, &standings_by_driver, &mut report)?;

        let mut refreshed_drivers = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao recarregar pilotos: {e}"))?;
        let mut refreshed_by_id: HashMap<String, Driver> = refreshed_drivers
            .iter()
            .cloned()
            .map(|driver| (driver.id.clone(), driver))
            .collect();
        sync_team_slots(conn, &teams, &refreshed_by_id)?;
        let initial_vacancies = find_vacancies(conn)?;
        let mut available = find_available_drivers(conn, &standings_by_driver)?;

        for vacancy in &initial_vacancies {
            let proposals = generate_team_proposals(vacancy, &available, new_season_number, rng);
            report.proposals_made += proposals.len() as i32;

            for proposal in proposals {
                let Some(index) = available
                    .iter()
                    .position(|candidate| candidate.driver.id == proposal.piloto_id)
                else {
                    continue;
                };
                let candidate = available[index].clone();
                let previous_contract = expiring_by_driver.get(&candidate.driver.id);
                let decision = evaluate_proposal(
                    &candidate.driver,
                    &proposal,
                    previous_contract,
                    candidate.category_tier,
                    vacancy.category_tier,
                    vacancy.car_performance,
                    vacancy.reputacao,
                    rng,
                );

                if !decision.accepted {
                    report.proposals_rejected += 1;
                    continue;
                }

                sign_driver_to_team(
                    conn,
                    &candidate.driver,
                    vacancy,
                    new_season_number,
                    proposal.salario_oferecido,
                    proposal.duracao_anos,
                    proposal.papel.clone(),
                )?;
                let tipo = if is_rookie_signing_candidate(
                    &candidate,
                    &expiring_by_driver,
                    &vacancy.categoria,
                ) {
                    report.rookies_placed += 1;
                    "rookie"
                } else {
                    "transferencia"
                };
                report.proposals_accepted += 1;
                report.new_signings.push(SigningInfo {
                    driver_id: candidate.driver.id.clone(),
                    driver_name: candidate.driver.nome.clone(),
                    team_id: vacancy.team_id.clone(),
                    team_name: vacancy.team_name.clone(),
                    categoria: vacancy.categoria.clone(),
                    papel: proposal.papel.as_str().to_string(),
                    tipo: tipo.to_string(),
                });
                available.remove(index);

                refreshed_drivers = driver_queries::get_all_drivers(conn)
                    .map_err(|e| format!("Falha ao recarregar pilotos apos assinatura: {e}"))?;
                refreshed_by_id = refreshed_drivers
                    .iter()
                    .cloned()
                    .map(|driver| (driver.id.clone(), driver))
                    .collect();
                sync_team_slots(conn, &teams, &refreshed_by_id)?;
                break;
            }
        }

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

        fill_remaining_vacancies_with_rookies(conn, &teams, new_season_number, &mut report, rng)?;

        refreshed_drivers = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao recarregar pilotos finais: {e}"))?;
        refreshed_by_id = refreshed_drivers
            .into_iter()
            .map(|driver| (driver.id.clone(), driver))
            .collect();
        refresh_team_hierarchy(conn, &teams, &refreshed_by_id)?;
        report.unresolved_vacancies = find_vacancies(conn)?.len() as i32;

        persist_market_state(conn, &new_season.id)?;
        Ok(report)
    })
}

fn is_rookie_signing_candidate(
    candidate: &AvailableDriver,
    expiring_by_driver: &HashMap<String, Contract>,
    target_category: &str,
) -> bool {
    if !is_real_career_debut_category(target_category) {
        return false;
    }
    if expiring_by_driver.contains_key(&candidate.driver.id) {
        return false;
    }
    if !candidate.categoria_atual.is_empty() {
        return false;
    }
    if candidate.posicao_campeonato < 99 {
        return false;
    }
    true
}

/// Escaneia todas as equipes de categorias regulares e garante que tenham 2 pilotos.
/// Caso faltem pilotos, preenche com novos rookies (rookies são gerados e contratados).
pub fn fill_all_remaining_vacancies(
    conn: &Connection,
    new_season_number: i32,
    rng: &mut impl Rng,
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
        let regular_vacancies: Vec<_> = find_vacancies(conn)?.into_iter().filter(fillable).collect();

        if regular_vacancies.is_empty() {
            break;
        }

        let mut report = MarketReport::default();
        fill_remaining_vacancies_with_rookies(conn, &teams, new_season_number, &mut report, rng)?;

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

fn get_season_by_number(
    conn: &Connection,
    season_number: i32,
) -> Result<Option<crate::models::season::Season>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, numero, ano, status, rodada_atual, created_at, updated_at
             FROM seasons
             WHERE numero = ?1
             LIMIT 1",
        )
        .map_err(|e| format!("Falha ao preparar busca de temporada: {e}"))?;
    stmt.query_row(params![season_number], |row| {
        Ok(crate::models::season::Season {
            id: row.get(0)?,
            numero: row.get(1)?,
            ano: row.get(2)?,
            status: crate::models::enums::SeasonStatus::from_str_strict(&row.get::<_, String>(3)?)
                .map_err(rusqlite::Error::InvalidParameterName)?,
            rodada_atual: row.get(4)?,
            fase: crate::models::enums::SeasonPhase::BlocoRegular,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })
    .optional()
    .map_err(|e| format!("Falha ao buscar temporada {season_number}: {e}"))
}

fn reset_market_state(conn: &Connection, season_id: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM market_proposals WHERE temporada_id = ?1",
        params![season_id],
    )
    .map_err(|e| format!("Falha ao limpar propostas de mercado: {e}"))?;
    conn.execute(
        "DELETE FROM market WHERE temporada_id = ?1",
        params![season_id],
    )
    .map_err(|e| format!("Falha ao limpar estado do mercado: {e}"))?;
    Ok(())
}

fn persist_market_state(conn: &Connection, season_id: &str) -> Result<(), String> {
    let now = timestamp_now();
    conn.execute(
        "INSERT INTO market (temporada_id, status, fase, inicio, fim)
         VALUES (?1, 'Fechado', 'PreTemporada', ?2, ?3)",
        params![season_id, now, now],
    )
    .map_err(|e| format!("Falha ao persistir estado do mercado: {e}"))?;
    Ok(())
}

fn load_market_contexts(
    conn: &Connection,
    previous_season_id: Option<&str>,
    drivers_by_id: &HashMap<String, Driver>,
    expiring_by_driver: &HashMap<String, Contract>,
) -> Result<HashMap<String, DriverMarketContext>, String> {
    let mut contexts = HashMap::new();
    if let Some(season_id) = previous_season_id {
        let mut stmt = conn
            .prepare(
                "SELECT piloto_id, categoria, posicao, vitorias, poles
                 FROM standings
                 WHERE temporada_id = ?1",
            )
            .map_err(|e| format!("Falha ao preparar standings do mercado: {e}"))?;
        let mut rows = stmt
            .query(params![season_id])
            .map_err(|e| format!("Falha ao ler standings do mercado: {e}"))?;
        let mut totals_by_category: HashMap<String, i32> = HashMap::new();
        let mut raw_rows = Vec::new();

        while let Some(row) = rows
            .next()
            .map_err(|e| format!("Falha ao iterar standings do mercado: {e}"))?
        {
            let piloto_id: String = row
                .get("piloto_id")
                .map_err(|e| format!("Falha ao ler piloto_id do standings: {e}"))?;
            let categoria: String = row.get("categoria").map_err(|e| {
                format!(
                    "Falha ao ler categoria do standings para piloto '{}': {e}",
                    piloto_id
                )
            })?;
            let posicao: i32 = row
                .get("posicao")
                .map_err(|e| format!("Falha ao ler posicao do standings: {e}"))?;
            let vitorias: i32 = row
                .get("vitorias")
                .map_err(|e| format!("Falha ao ler vitorias do standings: {e}"))?;
            let poles: i32 = row
                .get("poles")
                .map_err(|e| format!("Falha ao ler poles do standings: {e}"))?;
            *totals_by_category.entry(categoria.clone()).or_insert(0) += 1;
            raw_rows.push((piloto_id, categoria, posicao, vitorias, poles));
        }

        for (piloto_id, categoria, posicao, vitorias, poles) in raw_rows {
            let driver = drivers_by_id.get(&piloto_id);
            contexts.insert(
                piloto_id.clone(),
                DriverMarketContext {
                    posicao_campeonato: posicao,
                    total_pilotos: totals_by_category.get(&categoria).copied().unwrap_or(1),
                    category_tier: get_category_config(&categoria)
                        .map(|config| config.tier)
                        .unwrap_or(0),
                    categoria: categoria.clone(),
                    vitorias,
                    poles,
                    titulos: driver.map(|d| d.stats_carreira.titulos as i32).unwrap_or(0),
                    papel: expiring_by_driver
                        .get(&piloto_id)
                        .map(|contract| contract.papel.clone())
                        .unwrap_or(TeamRole::Numero2),
                },
            );
        }
    }

    for driver in drivers_by_id.values() {
        contexts
            .entry(driver.id.clone())
            .or_insert_with(|| default_market_context(driver));
    }
    Ok(contexts)
}

fn default_market_context(driver: &Driver) -> DriverMarketContext {
    let categoria = driver.categoria_atual.clone().unwrap_or_default();
    DriverMarketContext {
        posicao_campeonato: 99,
        total_pilotos: 99,
        category_tier: get_category_config(&categoria)
            .map(|config| config.tier)
            .unwrap_or(0),
        categoria,
        vitorias: driver.stats_temporada.vitorias as i32,
        poles: driver.stats_temporada.poles as i32,
        titulos: driver.stats_carreira.titulos as i32,
        papel: TeamRole::Numero2,
    }
}

fn sync_team_slots(
    conn: &Connection,
    teams: &[crate::models::team::Team],
    drivers_by_id: &HashMap<String, Driver>,
) -> Result<(), String> {
    sync_team_slots_from_active_regular_contracts(conn, teams, drivers_by_id)
}

fn find_vacancies(conn: &Connection) -> Result<Vec<Vacancy>, String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao buscar equipes: {e}"))?;
    let mut vacancies = Vec::new();

    for team in teams {
        if !uses_regular_contracts(&team.categoria) {
            continue;
        }
        let category_tier = get_category_config(&team.categoria)
            .map(|config| config.tier)
            .unwrap_or(0);
        match (&team.piloto_1_id, &team.piloto_2_id) {
            (None, None) => {
                vacancies.push(Vacancy {
                    team_id: team.id.clone(),
                    team_name: team.nome.clone(),
                    categoria: team.categoria.clone(),
                    classe: team.classe.clone(),
                    category_tier,
                    car_performance: team.car_performance,
                    budget: team.budget,
                    cash_balance: team.cash_balance,
                    debt_balance: team.debt_balance,
                    financial_state: team.financial_state.clone(),
                    reputacao: team.reputacao,
                    papel_necessario: TeamRole::Numero1,
                    piloto_existente_id: None,
                });
                vacancies.push(Vacancy {
                    team_id: team.id.clone(),
                    team_name: team.nome.clone(),
                    categoria: team.categoria.clone(),
                    classe: team.classe.clone(),
                    category_tier,
                    car_performance: team.car_performance,
                    budget: team.budget,
                    cash_balance: team.cash_balance,
                    debt_balance: team.debt_balance,
                    financial_state: team.financial_state.clone(),
                    reputacao: team.reputacao,
                    papel_necessario: TeamRole::Numero2,
                    piloto_existente_id: None,
                });
            }
            (Some(existing), None) => vacancies.push(Vacancy {
                team_id: team.id.clone(),
                team_name: team.nome.clone(),
                categoria: team.categoria.clone(),
                classe: team.classe.clone(),
                category_tier,
                car_performance: team.car_performance,
                budget: team.budget,
                cash_balance: team.cash_balance,
                debt_balance: team.debt_balance,
                financial_state: team.financial_state.clone(),
                reputacao: team.reputacao,
                papel_necessario: TeamRole::Numero2,
                piloto_existente_id: Some(existing.clone()),
            }),
            (None, Some(existing)) => vacancies.push(Vacancy {
                team_id: team.id.clone(),
                team_name: team.nome.clone(),
                categoria: team.categoria.clone(),
                classe: team.classe.clone(),
                category_tier,
                car_performance: team.car_performance,
                budget: team.budget,
                cash_balance: team.cash_balance,
                debt_balance: team.debt_balance,
                financial_state: team.financial_state.clone(),
                reputacao: team.reputacao,
                papel_necessario: TeamRole::Numero1,
                piloto_existente_id: Some(existing.clone()),
            }),
            (Some(_), Some(_)) => {}
        }
    }

    Ok(vacancies)
}

fn load_max_license_levels(conn: &Connection) -> Result<HashMap<String, u8>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT piloto_id, MAX(CAST(nivel AS INTEGER))
             FROM licenses
             GROUP BY piloto_id",
        )
        .map_err(|e| format!("Falha ao preparar consulta de licencas: {e}"))?;
    let mut rows = stmt
        .query([])
        .map_err(|e| format!("Falha ao ler licencas: {e}"))?;
    let mut map = HashMap::new();
    while let Some(row) = rows
        .next()
        .map_err(|e| format!("Falha ao iterar licencas: {e}"))?
    {
        let piloto_id: String = row.get(0).unwrap_or_default();
        let nivel: u8 = row.get::<_, i64>(1).unwrap_or(0) as u8;
        map.insert(piloto_id, nivel);
    }
    Ok(map)
}

fn find_available_drivers(
    conn: &Connection,
    standings_by_driver: &HashMap<String, DriverMarketContext>,
) -> Result<Vec<AvailableDriver>, String> {
    let active_contracts = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao recarregar contratos ativos: {e}"))?;
    let contracted_ids: HashSet<String> = active_contracts
        .into_iter()
        .map(|contract| contract.piloto_id)
        .collect();

    let drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos disponiveis: {e}"))?;
    let license_levels = load_max_license_levels(conn)?;
    let mut available = Vec::new();

    for driver in drivers {
        if driver.is_jogador
            || driver.status != DriverStatus::Ativo
            || contracted_ids.contains(&driver.id)
        {
            continue;
        }
        let context = standings_by_driver
            .get(&driver.id)
            .cloned()
            .unwrap_or_else(|| default_market_context(&driver));
        let visibility = calculate_visibility(
            &driver,
            context.posicao_campeonato,
            context.total_pilotos,
            context.category_tier,
            context.vitorias,
            context.titulos,
            context.poles,
            &context.papel,
            &context.categoria,
        );
        let max_license_level = license_levels.get(&driver.id).copied();
        available.push(AvailableDriver {
            driver,
            visibility,
            posicao_campeonato: context.posicao_campeonato,
            categoria_atual: context.categoria,
            category_tier: context.category_tier,
            max_license_level,
        });
    }

    Ok(available)
}

pub(crate) fn sign_driver_to_team(
    conn: &Connection,
    driver: &Driver,
    vacancy: &Vacancy,
    new_season_number: i32,
    salary: f64,
    duration: i32,
    role: TeamRole,
) -> Result<(), String> {
    with_savepoint(conn, "market_sign_driver", || {
        let team = team_queries::get_team_by_id(conn, &vacancy.team_id)
            .map_err(|e| format!("Falha ao buscar equipe da assinatura: {e}"))?
            .ok_or_else(|| format!("Equipe '{}' nao encontrada", vacancy.team_id))?;
        ensure_driver_can_join_division(
            conn,
            &driver.id,
            &driver.nome,
            &vacancy.categoria,
            vacancy.classe.as_deref(),
        )?;
        let mut new_contract = Contract::new(
            next_id(conn, IdType::Contract)
                .map_err(|e| format!("Falha ao gerar ID de contrato: {e}"))?,
            driver.id.clone(),
            driver.nome.clone(),
            vacancy.team_id.clone(),
            team.nome.clone(),
            new_season_number,
            duration,
            salary,
            role,
            vacancy.categoria.clone(),
        );
        new_contract.classe = team.classe.clone();
        contract_queries::insert_contract(conn, &new_contract)
            .map_err(|e| format!("Falha ao inserir contratacao: {e}"))?;

        let mut updated_driver = driver.clone();
        updated_driver.categoria_atual = Some(vacancy.categoria.clone());
        driver_queries::update_driver(conn, &updated_driver).map_err(|e| {
            format!(
                "Falha ao atualizar piloto contratado '{}': {e}",
                driver.nome
            )
        })?;
        Ok(())
    })
}

fn generate_player_proposals(
    conn: &Connection,
    season_id: &str,
    new_season_number: i32,
    vacancies: &[Vacancy],
    player_was_expiring: bool,
    standings_by_driver: &HashMap<String, DriverMarketContext>,
    rng: &mut impl Rng,
) -> Result<Vec<MarketProposal>, String> {
    let player = match driver_queries::get_player_driver(conn) {
        Ok(p) => p,
        Err(crate::db::connection::DbError::NotFound(_)) => return Ok(Vec::new()),
        Err(e) => {
            return Err(format!(
                "Falha ao buscar piloto do jogador para o mercado: {e}"
            ))
        }
    };
    let player_active_contract =
        contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
            .map_err(|e| format!("Falha ao buscar contrato regular do jogador: {e}"))?;
    let player_is_free = player_active_contract.is_none();
    if !player_is_free && !player_was_expiring {
        return Ok(Vec::new());
    }

    // Calcula visibilidade real do jogador com os mesmos dados usados pela IA.
    let context = standings_by_driver
        .get(&player.id)
        .cloned()
        .unwrap_or_else(|| default_market_context(&player));
    let visibility = calculate_visibility(
        &player,
        context.posicao_campeonato,
        context.total_pilotos,
        context.category_tier,
        context.vitorias,
        context.titulos,
        context.poles,
        &context.papel,
        &context.categoria,
    );

    // Usa is_jogador=false para que generate_team_proposals avalie o jogador
    // com os mesmos critérios de qualquer piloto IA. A flag existe apenas para
    // impedir que o loop principal de mercado proponha ao jogador — aqui é intencional.
    let license_levels = load_max_license_levels(conn)?;
    let max_license_level = license_levels.get(&player.id).copied();
    let mut player_as_driver = player.clone();
    player_as_driver.is_jogador = false;
    let player_available = AvailableDriver {
        driver: player_as_driver,
        visibility,
        posicao_campeonato: context.posicao_campeonato,
        categoria_atual: context.categoria.clone(),
        category_tier: context.category_tier,
        max_license_level,
    };

    let mut proposals = Vec::new();
    for vacancy in vacancies {
        let team_proposals = generate_team_proposals(
            vacancy,
            std::slice::from_ref(&player_available),
            new_season_number,
            rng,
        );
        for mut proposal in team_proposals {
            // Restaura o ID correto do jogador e gera ID de proposta único por temporada.
            proposal.piloto_id = player.id.clone();
            proposal.piloto_nome = player.nome.clone();
            proposal.id = format!(
                "MP-{}-{}-{}-{}",
                new_season_number,
                vacancy.team_id,
                player.id,
                vacancy.papel_necessario.as_str(),
            );
            persist_player_proposal(conn, season_id, &proposal)?;
            proposals.push(proposal);
        }
    }

    // Garantia de proposta mínima: se o jogador estiver livre neste início de temporada,
    // a pipeline tenta preservar ao menos uma rota de continuidade para não deixá-lo
    // sem evento jogável na pré-temporada. Isso cobre tanto quem já estava livre
    // quanto quem acabou de ter o contrato encerrado.
    // Tenta: 1) equipe anterior na mesma categoria, 2) pior equipe da mesma categoria,
    // 3) melhor equipe de categoria inferior (salário menor naturalmente).
    if proposals.is_empty() && player_is_free {
        // Categoria do jogador: contexto dos standings ou último contrato no DB.
        let player_category = if !context.categoria.is_empty() {
            context.categoria.clone()
        } else {
            find_last_player_category(conn, &player.id)?
        };

        if !player_category.is_empty() {
            let all_teams = team_queries::get_all_teams(conn)
                .map_err(|e| format!("Falha ao carregar equipes para fallback do jogador: {e}"))?;
            let category_teams: Vec<&crate::models::team::Team> = all_teams
                .iter()
                .filter(|team| team.categoria == player_category)
                .collect();

            // Tenta primeiro a equipe anterior, depois a pior equipe da mesma categoria.
            let mut fallback_team =
                find_previous_team_for_player(conn, &player.id, &category_teams)?
                    .or_else(|| worst_team(&category_teams));

            // Se não há vaga na categoria atual, tenta a melhor vaga de tier inferior.
            if fallback_team.is_none() {
                let player_tier =
                    crate::constants::categories::get_category_config(&player_category)
                        .map(|c| c.tier)
                        .unwrap_or(99);
                let lower_teams: Vec<&crate::models::team::Team> = all_teams
                    .iter()
                    .filter(|team| {
                        crate::constants::categories::get_category_config(&team.categoria)
                            .map(|c| c.tier < player_tier)
                            .unwrap_or(false)
                    })
                    .collect();
                fallback_team = best_team(&lower_teams);
            }

            if let Some(team) = fallback_team {
                let vacancy = fallback_vacancy_from_team(team);
                let proposal = MarketProposal {
                    id: format!(
                        "MP-{}-{}-{}-fallback",
                        new_season_number, vacancy.team_id, player.id
                    ),
                    equipe_id: vacancy.team_id.clone(),
                    equipe_nome: vacancy.team_name.clone(),
                    piloto_id: player.id.clone(),
                    piloto_nome: player.nome.clone(),
                    categoria: vacancy.categoria.clone(),
                    papel: vacancy.papel_necessario.clone(),
                    salario_oferecido: calculate_fallback_salary(&vacancy, &player),
                    duracao_anos: 1,
                    status: crate::market::proposals::ProposalStatus::Pendente,
                    motivo_recusa: None,
                };
                persist_player_proposal(conn, season_id, &proposal)?;
                proposals.push(proposal);
            }
        }
    }

    Ok(proposals)
}

/// Retorna a categoria do contrato mais recente do jogador (qualquer status).
fn find_last_player_category(conn: &Connection, player_id: &str) -> Result<String, String> {
    conn.query_row(
        "SELECT categoria FROM contracts WHERE piloto_id = ?1 ORDER BY temporada_fim DESC LIMIT 1",
        params![player_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map(|opt| opt.unwrap_or_default())
    .map_err(|e| format!("Falha ao buscar última categoria do jogador: {e}"))
}

fn find_previous_team_for_player<'a>(
    conn: &Connection,
    player_id: &str,
    teams: &[&'a crate::models::team::Team],
) -> Result<Option<&'a crate::models::team::Team>, String> {
    let prev_team_id: Option<String> = conn
        .query_row(
            "SELECT equipe_id FROM contracts WHERE piloto_id = ?1 ORDER BY temporada_fim DESC LIMIT 1",
            params![player_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Falha ao buscar equipe anterior do jogador: {e}"))?;

    let Some(team_id) = prev_team_id else {
        return Ok(None);
    };
    Ok(teams.iter().find(|team| team.id == team_id).copied())
}

fn worst_team<'a>(
    teams: &[&'a crate::models::team::Team],
) -> Option<&'a crate::models::team::Team> {
    teams
        .iter()
        .min_by(|a, b| a.car_performance.total_cmp(&b.car_performance))
        .copied()
}

fn best_team<'a>(teams: &[&'a crate::models::team::Team]) -> Option<&'a crate::models::team::Team> {
    teams
        .iter()
        .max_by(|a, b| a.car_performance.total_cmp(&b.car_performance))
        .copied()
}

fn fallback_vacancy_from_team(team: &crate::models::team::Team) -> Vacancy {
    let papel_necessario = if team.piloto_1_id.is_none() {
        TeamRole::Numero1
    } else {
        TeamRole::Numero2
    };
    let piloto_existente_id = match papel_necessario {
        TeamRole::Numero1 => team.piloto_1_id.clone(),
        TeamRole::Numero2 => team.piloto_2_id.clone(),
    };

    Vacancy {
        team_id: team.id.clone(),
        team_name: team.nome.clone(),
        categoria: team.categoria.clone(),
        classe: team.classe.clone(),
        category_tier: get_category_config(&team.categoria)
            .map(|config| config.tier)
            .unwrap_or(0),
        car_performance: team.car_performance,
        budget: team.budget,
        cash_balance: team.cash_balance,
        debt_balance: team.debt_balance,
        financial_state: team.financial_state.clone(),
        reputacao: team.reputacao,
        papel_necessario,
        piloto_existente_id,
    }
}

fn persist_player_proposal(
    conn: &Connection,
    season_id: &str,
    proposal: &MarketProposal,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO market_proposals (
            id, temporada_id, equipe_id, piloto_id, papel, salario, status, motivo_recusa, criado_em
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
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

fn fill_remaining_vacancies_with_rookies(
    conn: &Connection,
    teams: &[crate::models::team::Team],
    new_season_number: i32,
    report: &mut MarketReport,
    rng: &mut impl Rng,
) -> Result<(), String> {
    let debut_year = get_season_by_number(conn, new_season_number)?
        .map(|season| season.ano)
        .unwrap_or_else(|| Local::now().year());

    loop {
        let current_drivers = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao recarregar pilotos: {e}"))?;
        let current_by_id: HashMap<String, Driver> = current_drivers
            .iter()
            .cloned()
            .map(|driver| (driver.id.clone(), driver))
            .collect();
        sync_team_slots(conn, teams, &current_by_id)?;
        let vacancies: Vec<_> = find_vacancies(conn)?
            .into_iter()
            .filter(is_regular_vacancy)
            .filter(|vacancy| is_category_active_in_year(&vacancy.categoria, debut_year))
            .collect();
        if vacancies.is_empty() {
            break;
        }

        let previous_season = get_season_by_number(conn, new_season_number - 1)?;
        let market_contexts = load_market_contexts(
            conn,
            previous_season.as_ref().map(|season| season.id.as_str()),
            &current_by_id,
            &HashMap::new(),
        )?;
        let mut available = find_available_drivers(conn, &market_contexts)?;
        let license_levels = load_max_license_levels(conn)?;
        let mut filled_any = false;
        for vacancy in vacancies {
            let fallback_index = available
                .iter()
                .enumerate()
                .filter(|(_, candidate)| is_pool_fallback_candidate(candidate, &vacancy))
                .max_by(|(_, a), (_, b)| compare_pool_fallback_candidates(a, b, &vacancy))
                .map(|(index, _)| index);

            if let Some(index) = fallback_index {
                let candidate = available.remove(index);
                grant_driver_license_for_division_if_needed(
                    conn,
                    &candidate.driver.id,
                    &vacancy.categoria,
                    vacancy.classe.as_deref(),
                )?;
                sign_driver_to_team(
                    conn,
                    &candidate.driver,
                    &vacancy,
                    new_season_number,
                    calculate_fallback_salary(&vacancy, &candidate.driver),
                    1,
                    vacancy.papel_necessario.clone(),
                )?;
                let signing_type = if is_real_career_debut_category(&vacancy.categoria) {
                    report.rookies_placed += 1;
                    "rookie"
                } else {
                    "transferencia"
                };
                report.new_signings.push(SigningInfo {
                    driver_id: candidate.driver.id.clone(),
                    driver_name: candidate.driver.nome.clone(),
                    team_id: vacancy.team_id.clone(),
                    team_name: vacancy.team_name.clone(),
                    categoria: vacancy.categoria.clone(),
                    papel: vacancy.papel_necessario.as_str().to_string(),
                    tipo: signing_type.to_string(),
                });
                filled_any = true;
                continue;
            }

            if is_real_career_debut_category(&vacancy.categoria)
                || is_entry_category_for_year(&vacancy.categoria, debut_year)
            {
                let rookie = generate_and_sign_rookie_for_vacancy(
                    conn,
                    &vacancy,
                    new_season_number,
                    debut_year,
                    rng,
                )?;
                report.rookies_placed += 1;
                report.new_signings.push(SigningInfo {
                    driver_id: rookie.id.clone(),
                    driver_name: rookie.nome.clone(),
                    team_id: vacancy.team_id.clone(),
                    team_name: vacancy.team_name.clone(),
                    categoria: vacancy.categoria.clone(),
                    papel: vacancy.papel_necessario.as_str().to_string(),
                    tipo: "rookie".to_string(),
                });
                filled_any = true;
                continue;
            }

            // Sistema de escada (modelo fechado): se o pool não cobre uma vaga
            // não-estreia, promovemos o melhor piloto da categoria de baixo em vez
            // de gerar um piloto novo do nada ou abortar. Promover abre o assento
            // dele lá embaixo, que será preenchido na próxima volta do loop — a
            // cascata desce até a categoria de estreia, onde aí sim nasce um rookie.
            // Portão de MÉRITO só na escada regular. Categorias de fase especial
            // (endurance/production) têm elegibilidade própria (convocação) e uma
            // progressão de classe que os feeders regulares não conseguem licenciar,
            // então ali mantemos o comportamento antigo (concede a licença ao assinar).
            let is_special_vacancy = runs_in_special_phase(&vacancy.categoria);
            let required_license = if is_special_vacancy {
                None
            } else {
                required_license_for_division(&vacancy.categoria, vacancy.classe.as_deref())
            };
            if let Some(candidate) = best_feeder_promotion_candidate(
                &vacancy,
                &current_by_id,
                &market_contexts,
                &license_levels,
                required_license,
            ) {
                // Rescinde o contrato atual do piloto na categoria de baixo antes de
                // promovê-lo (o índice único (piloto_id, tipo) impede dois contratos
                // regulares ativos). Isso abre o assento dele lá embaixo.
                for contract in contract_queries::get_all_active_regular_contracts(conn)
                    .map_err(|e| format!("Falha ao carregar contrato do promovido: {e}"))?
                    .into_iter()
                    .filter(|contract| contract.piloto_id == candidate.id)
                {
                    contract_queries::update_contract_status(
                        conn,
                        &contract.id,
                        &ContractStatus::Rescindido,
                    )
                    .map_err(|e| format!("Falha ao rescindir contrato do promovido: {e}"))?;
                }
                if is_special_vacancy {
                    // Especiais: concede a licença da divisão (elegibilidade via convocação).
                    grant_driver_license_for_division_if_needed(
                        conn,
                        &candidate.id,
                        &vacancy.categoria,
                        vacancy.classe.as_deref(),
                    )?;
                }
                // Escada regular: sem concessão — o candidato JÁ tem a licença exigida
                // (filtro de mérito em best_feeder_promotion_candidate).
                sign_driver_to_team(
                    conn,
                    &candidate,
                    &vacancy,
                    new_season_number,
                    calculate_fallback_salary(&vacancy, &candidate),
                    1,
                    vacancy.papel_necessario.clone(),
                )?;
                report.new_signings.push(SigningInfo {
                    driver_id: candidate.id.clone(),
                    driver_name: candidate.nome.clone(),
                    team_id: vacancy.team_id.clone(),
                    team_name: vacancy.team_name.clone(),
                    categoria: vacancy.categoria.clone(),
                    papel: vacancy.papel_necessario.as_str().to_string(),
                    tipo: "promocao".to_string(),
                });
                filled_any = true;
                // Reescaneia do zero: o assento aberto na categoria de baixo vira a
                // próxima vaga a preencher (continua a cascata) e evita reusar o
                // mesmo candidato com dados defasados nesta passada.
                break;
            }

            // Escassez numa vaga REGULAR de categoria superior, sem candidato
            // meritorio. Deixar o assento vazio violaria a invariante de grid
            // (validate_and_normalize_team_hierarchies aborta a temporada). As
            // categorias especiais (endurance/production) NAO entram aqui.
            if is_special_vacancy {
                continue;
            }

            if let Some(candidate) = best_feeder_promotion_candidate(
                &vacancy,
                &current_by_id,
                &market_contexts,
                &license_levels,
                None,
            ) {
                for contract in contract_queries::get_all_active_regular_contracts(conn)
                    .map_err(|e| {
                        format!("Falha ao carregar contrato do promovido (emergencia): {e}")
                    })?
                    .into_iter()
                    .filter(|contract| contract.piloto_id == candidate.id)
                {
                    contract_queries::update_contract_status(
                        conn,
                        &contract.id,
                        &ContractStatus::Rescindido,
                    )
                    .map_err(|e| {
                        format!("Falha ao rescindir contrato do promovido (emergencia): {e}")
                    })?;
                }
                grant_driver_license_for_division_if_needed(
                    conn,
                    &candidate.id,
                    &vacancy.categoria,
                    vacancy.classe.as_deref(),
                )?;
                sign_driver_to_team(
                    conn,
                    &candidate,
                    &vacancy,
                    new_season_number,
                    calculate_fallback_salary(&vacancy, &candidate),
                    1,
                    vacancy.papel_necessario.clone(),
                )?;
                report.new_signings.push(SigningInfo {
                    driver_id: candidate.id.clone(),
                    driver_name: candidate.nome.clone(),
                    team_id: vacancy.team_id.clone(),
                    team_name: vacancy.team_name.clone(),
                    categoria: vacancy.categoria.clone(),
                    papel: vacancy.papel_necessario.as_str().to_string(),
                    tipo: "promocao_emergencia".to_string(),
                });
                EMERGENCY_PROMOTIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let from_tier = candidate
                    .categoria_atual
                    .as_deref()
                    .and_then(get_category_config)
                    .map(|c| c.tier)
                    .unwrap_or(99);
                let to_tier = get_category_config(&vacancy.categoria)
                    .map(|c| c.tier)
                    .unwrap_or(99);
                if let Ok(mut paths) = EMERGENCY_PROMO_PATHS.lock() {
                    paths.push((from_tier, to_tier));
                }
                filled_any = true;
                break;
            }

            let rookie = generate_and_sign_rookie_for_vacancy(
                conn,
                &vacancy,
                new_season_number,
                debut_year,
                rng,
            )?;
            report.rookies_placed += 1;
            report.new_signings.push(SigningInfo {
                driver_id: rookie.id.clone(),
                driver_name: rookie.nome.clone(),
                team_id: vacancy.team_id.clone(),
                team_name: vacancy.team_name.clone(),
                categoria: vacancy.categoria.clone(),
                papel: vacancy.papel_necessario.as_str().to_string(),
                tipo: "rookie_emergencia".to_string(),
            });
            EMERGENCY_ROOKIES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            filled_any = true;
            break;
        }

        if !filled_any {
            // Nenhuma vaga preenchível nesta passada: para de tentar (evita loop
            // infinito); as vagas restantes ficam abertas até a próxima preseason.
            break;
        }
    }

    Ok(())
}

/// Rebaixamento por MÉRITO (modelo fechado, conservação preservada): em cada
/// categoria regular, se o melhor piloto licenciado da categoria de baixo foi
/// campeão/vice e o pior piloto da categoria terminou no fundo (penúltimo/último),
/// os dois TROCAM de assento — um sobe, um desce. Conservador: no máximo 1 troca
/// por categoria por temporada e nunca mexe no piloto jogador.
fn apply_merit_relegations(
    conn: &Connection,
    teams: &[crate::models::team::Team],
    new_season_number: i32,
    contexts: &HashMap<String, DriverMarketContext>,
    report: &mut MarketReport,
) -> Result<(), String> {
    let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos para rebaixamento: {e}"))?
        .into_iter()
        .map(|driver| (driver.id.clone(), driver))
        .collect();
    let license_levels = load_max_license_levels(conn)?;

    // Categorias regulares (não-estreia, não-especiais), do topo para a base.
    let mut categories: Vec<_> = get_all_categories()
        .iter()
        .filter(|category| {
            uses_regular_contracts(category.id)
                && !runs_in_special_phase(category.id)
                && !is_real_career_debut_category(category.id)
        })
        .collect();
    categories.sort_by(|a, b| b.tier.cmp(&a.tier));

    // O rebaixamento automático nunca mexe no time do jogador — ele controla o
    // próprio elenco (e mexer ali quebraria o plano de pré-temporada).
    let player_team_ids: HashSet<&str> = teams
        .iter()
        .filter(|team| team.is_player_team)
        .map(|team| team.id.as_str())
        .collect();
    let is_active_non_player = |id: &str| {
        drivers_by_id
            .get(id)
            .is_some_and(|driver| !driver.is_jogador && driver.status == DriverStatus::Ativo)
    };
    let position_of = |id: &str| contexts.get(id).map(|c| c.posicao_campeonato).unwrap_or(99);
    let skill_of = |id: &str| drivers_by_id.get(id).map(|d| d.atributos.skill).unwrap_or(0.0);

    for category in categories {
        let Some(required) = required_license_for_division(category.id, None) else {
            continue;
        };
        let active = contract_queries::get_all_active_regular_contracts(conn)
            .map_err(|e| format!("Falha ao carregar contratos para rebaixamento: {e}"))?;

        // Pior piloto da categoria: pior posição no campeonato, depois menor skill.
        let upper: Vec<&Contract> = active
            .iter()
            .filter(|c| {
                c.categoria == category.id
                    && c.classe.is_none()
                    && is_active_non_player(&c.piloto_id)
                    && !player_team_ids.contains(c.equipe_id.as_str())
            })
            .collect();
        if upper.len() < 2 {
            continue;
        }
        let Some(weakest) = upper.iter().max_by(|a, b| {
            position_of(&a.piloto_id)
                .cmp(&position_of(&b.piloto_id))
                .then_with(|| skill_of(&b.piloto_id).total_cmp(&skill_of(&a.piloto_id)))
        }) else {
            continue;
        };
        // Só rebaixa quem realmente foi mal: penúltimo ou último na sua categoria.
        let Some(weak_ctx) = contexts.get(&weakest.piloto_id) else {
            continue;
        };
        if weak_ctx.total_pilotos < 2
            || weak_ctx.posicao_campeonato < weak_ctx.total_pilotos - 1
        {
            continue;
        }

        // Melhor "subidor": campeão/vice de um feeder, já com a licença exigida.
        let feeders = get_feeder_categories(category.id);
        let Some(best_riser) = active
            .iter()
            .filter(|c| {
                feeders.iter().any(|feeder| *feeder == c.categoria)
                    && license_levels
                        .get(&c.piloto_id)
                        .is_some_and(|&owned| owned >= required)
                    && is_active_non_player(&c.piloto_id)
                    && !player_team_ids.contains(c.equipe_id.as_str())
            })
            .min_by(|a, b| {
                position_of(&a.piloto_id)
                    .cmp(&position_of(&b.piloto_id))
                    .then_with(|| skill_of(&b.piloto_id).total_cmp(&skill_of(&a.piloto_id)))
            })
        else {
            continue;
        };
        if position_of(&best_riser.piloto_id) > 2 {
            continue;
        }

        swap_contract_seats(conn, best_riser, weakest, new_season_number, report)?;
    }

    let refreshed: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao recarregar pilotos apos rebaixamento: {e}"))?
        .into_iter()
        .map(|driver| (driver.id.clone(), driver))
        .collect();
    sync_team_slots_from_active_regular_contracts(conn, teams, &refreshed)?;
    Ok(())
}

/// Executa a troca de assentos: `riser` (de baixo) assume a vaga de `weak` (de cima)
/// e `weak` assume a vaga de `riser`. Rescinde os dois contratos e cria os novos
/// trocados; ambos já têm a licença das divisões de destino (ver chamador).
fn swap_contract_seats(
    conn: &Connection,
    riser: &Contract,
    weak: &Contract,
    new_season_number: i32,
    report: &mut MarketReport,
) -> Result<(), String> {
    for contract_id in [&riser.id, &weak.id] {
        contract_queries::update_contract_status(conn, contract_id, &ContractStatus::Rescindido)
            .map_err(|e| format!("Falha ao rescindir contrato na troca de mérito: {e}"))?;
    }

    let mut move_driver = |conn: &Connection,
                           piloto_id: &str,
                           piloto_nome: &str,
                           destino: &Contract,
                           tipo: &str|
     -> Result<(), String> {
        let mut contract = Contract::new(
            next_id(conn, IdType::Contract)
                .map_err(|e| format!("Falha ao gerar ID de contrato na troca: {e}"))?,
            piloto_id.to_string(),
            piloto_nome.to_string(),
            destino.equipe_id.clone(),
            destino.equipe_nome.clone(),
            new_season_number,
            1,
            destino.salario_anual,
            destino.papel.clone(),
            destino.categoria.clone(),
        );
        contract.classe = destino.classe.clone();
        contract_queries::insert_contract(conn, &contract)
            .map_err(|e| format!("Falha ao inserir contrato na troca de mérito: {e}"))?;

        if let Some(mut driver) = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao carregar piloto na troca: {e}"))?
            .into_iter()
            .find(|driver| driver.id == piloto_id)
        {
            driver.categoria_atual = Some(destino.categoria.clone());
            driver_queries::update_driver(conn, &driver)
                .map_err(|e| format!("Falha ao atualizar categoria na troca: {e}"))?;
        }

        report.new_signings.push(SigningInfo {
            driver_id: piloto_id.to_string(),
            driver_name: piloto_nome.to_string(),
            team_id: destino.equipe_id.clone(),
            team_name: destino.equipe_nome.clone(),
            categoria: destino.categoria.clone(),
            papel: destino.papel.as_str().to_string(),
            tipo: tipo.to_string(),
        });
        Ok(())
    };

    // riser sobe para a vaga de weak; weak desce para a vaga do riser.
    move_driver(conn, &riser.piloto_id, &riser.piloto_nome, weak, "promocao_merito")?;
    move_driver(conn, &weak.piloto_id, &weak.piloto_nome, riser, "rebaixamento")?;
    Ok(())
}

/// Categoria de ENTRADA da escada num ano: ativa e com nenhuma feeder ativa ainda
/// (a de menor tier existente). É onde nascem novos pilotos da época.
fn is_entry_category_for_year(categoria: &str, year: i32) -> bool {
    is_category_active_in_year(categoria, year)
        && get_feeder_categories(categoria)
            .iter()
            .all(|feeder| !is_category_active_in_year(feeder, year))
}

/// Melhor candidato a PROMOÇÃO para uma vaga não-estreia (escada por MÉRITO).
///
/// Piloto ativo (não-jogador) atualmente numa categoria que alimenta a vaga
/// (`get_feeder_categories`) E que **já conquistou a licença exigida** pela divisão
/// (top-metade da categoria de baixo — mesma regra do jogador; nada de conceder
/// licença na hora). Entre os elegíveis, escolhe pela classificação no campeonato e,
/// em empate, pelo maior skill. Ver uso em `fill_remaining_vacancies_with_rookies`.
fn best_feeder_promotion_candidate(
    vacancy: &Vacancy,
    drivers_by_id: &HashMap<String, Driver>,
    contexts: &HashMap<String, DriverMarketContext>,
    license_levels: &HashMap<String, u8>,
    required_license: Option<u8>,
) -> Option<Driver> {
    let feeders = get_feeder_categories(&vacancy.categoria);
    if feeders.is_empty() {
        return None;
    }

    drivers_by_id
        .values()
        .filter(|driver| {
            !driver.is_jogador
                && driver.status == DriverStatus::Ativo
                && driver
                    .categoria_atual
                    .as_deref()
                    .is_some_and(|categoria| feeders.iter().any(|feeder| *feeder == categoria))
                // Mérito: precisa POSSUIR de fato a licença exigida (linha real
                // >= nível), igual ao check de ensure_driver_can_join_division.
                // "Sem licença" não conta como nível 0.
                && match required_license {
                    Some(level) => license_levels
                        .get(&driver.id)
                        .is_some_and(|&owned| owned >= level),
                    None => true,
                }
        })
        .min_by(|a, b| {
            let pos_a = contexts
                .get(&a.id)
                .map(|context| context.posicao_campeonato)
                .unwrap_or(99);
            let pos_b = contexts
                .get(&b.id)
                .map(|context| context.posicao_campeonato)
                .unwrap_or(99);
            pos_a
                .cmp(&pos_b)
                .then_with(|| b.atributos.skill.total_cmp(&a.atributos.skill))
        })
        .cloned()
}

fn generate_and_sign_rookie_for_vacancy(
    conn: &Connection,
    vacancy: &Vacancy,
    new_season_number: i32,
    debut_year: i32,
    rng: &mut impl Rng,
) -> Result<Driver, String> {
    let mut existing_names: HashSet<String> = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar nomes existentes para rookie: {e}"))?
        .into_iter()
        .map(|driver| driver.nome)
        .collect();
    let mut rookie = generate_rookies(1, debut_year, &mut existing_names, rng)
        .into_iter()
        .next()
        .ok_or_else(|| "Falha ao gerar rookie para vaga final.".to_string())?;
    rookie.id =
        next_id(conn, IdType::Driver).map_err(|e| format!("Falha ao gerar ID de rookie: {e}"))?;
    rookie.categoria_atual = None;

    driver_queries::insert_driver(conn, &rookie)
        .map_err(|e| format!("Falha ao inserir rookie '{}': {e}", rookie.nome))?;
    grant_driver_license_for_division_if_needed(
        conn,
        &rookie.id,
        &vacancy.categoria,
        vacancy.classe.as_deref(),
    )?;
    sign_driver_to_team(
        conn,
        &rookie,
        vacancy,
        new_season_number,
        calculate_fallback_salary(vacancy, &rookie),
        1,
        vacancy.papel_necessario.clone(),
    )?;

    Ok(rookie)
}

fn is_regular_vacancy(vacancy: &Vacancy) -> bool {
    get_category_config(&vacancy.categoria)
        .map(|category| uses_regular_contracts(category.id))
        .unwrap_or(true)
}

fn compare_pool_fallback_candidates(
    a: &AvailableDriver,
    b: &AvailableDriver,
    vacancy: &Vacancy,
) -> std::cmp::Ordering {
    pool_fallback_candidate_rank(a, vacancy)
        .cmp(&pool_fallback_candidate_rank(b, vacancy))
        .then_with(|| {
            a.driver
                .atributos
                .skill
                .total_cmp(&b.driver.atributos.skill)
        })
}

fn is_pool_fallback_candidate(candidate: &AvailableDriver, vacancy: &Vacancy) -> bool {
    if is_real_career_debut_category(&vacancy.categoria) {
        return is_rookie_market_candidate(
            &vacancy.categoria,
            candidate_category_for_rookie(candidate),
            candidate.driver.stats_carreira.corridas,
            candidate.driver.stats_carreira.temporadas,
        );
    }

    candidate.driver.categoria_atual.is_none()
}

fn pool_fallback_candidate_rank(candidate: &AvailableDriver, vacancy: &Vacancy) -> (u8, u8, u8) {
    let preferred_experience = if is_real_career_debut_category(&vacancy.categoria) {
        if candidate.driver.stats_carreira.corridas == 0
            && candidate.driver.stats_carreira.temporadas == 0
        {
            2
        } else if candidate_category_for_rookie(candidate) == vacancy.categoria {
            1
        } else {
            0
        }
    } else {
        u8::from(candidate.driver.stats_carreira.corridas > 0)
    };
    let required_license =
        required_license_for_division(&vacancy.categoria, vacancy.classe.as_deref()).unwrap_or(0);
    let has_required_license = candidate
        .max_license_level
        .map(|level| level >= required_license)
        .unwrap_or(required_license == 0);
    let license_level = candidate
        .max_license_level
        .unwrap_or(0)
        .min(required_license);

    (
        preferred_experience,
        u8::from(has_required_license),
        license_level,
    )
}

fn candidate_category_for_rookie(candidate: &AvailableDriver) -> &str {
    if candidate.categoria_atual.trim().is_empty() {
        candidate
            .driver
            .categoria_atual
            .as_deref()
            .unwrap_or_default()
    } else {
        candidate.categoria_atual.as_str()
    }
}

fn calculate_fallback_salary(vacancy: &Vacancy, driver: &Driver) -> f64 {
    let tier_base = match vacancy.category_tier {
        0 => 9_000.0,
        1 => 18_000.0,
        2 => 35_000.0,
        3 => 70_000.0,
        4 => 120_000.0,
        5 => 180_000.0,
        6 => 210_000.0,
        _ => 50_000.0,
    };
    (tier_base * (driver.atributos.skill / 75.0).max(0.7)).max(5_000.0)
}

fn refresh_team_hierarchy(
    conn: &Connection,
    teams: &[crate::models::team::Team],
    drivers_by_id: &HashMap<String, Driver>,
) -> Result<(), String> {
    for team in teams {
        let refreshed = team_queries::get_team_by_id(conn, &team.id)
            .map_err(|e| format!("Falha ao recarregar equipe '{}': {e}", team.nome))?
            .ok_or_else(|| format!("Equipe '{}' nao encontrada", team.id))?;
        let mut pilots = Vec::new();
        if let Some(pilot_id) = &refreshed.piloto_1_id {
            if let Some(driver) = drivers_by_id.get(pilot_id) {
                pilots.push(driver);
            }
        }
        if let Some(pilot_id) = &refreshed.piloto_2_id {
            if let Some(driver) = drivers_by_id.get(pilot_id) {
                pilots.push(driver);
            }
        }
        pilots.sort_by(|a, b| b.atributos.skill.total_cmp(&a.atributos.skill));
        let n1 = pilots.first().map(|driver| driver.id.as_str());
        let n2 = pilots.get(1).map(|driver| driver.id.as_str());
        team_queries::update_team_pilots(conn, &team.id, n1, n2).map_err(|e| {
            format!(
                "Falha ao atualizar pilotos finais da equipe '{}': {e}",
                team.nome
            )
        })?;
        team_queries::update_team_hierarchy(
            conn,
            &team.id,
            n1,
            n2,
            TeamHierarchyClimate::Estavel.as_str(),
            0.0,
        )
        .map_err(|e| {
            format!(
                "Falha ao atualizar hierarquia da equipe '{}': {e}",
                team.nome
            )
        })?;
    }
    Ok(())
}

fn timestamp_now() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};
    use rusqlite::Connection;

    use super::*;
    use crate::constants::teams::get_team_templates;
    use crate::db::migrations;
    use crate::db::queries::seasons as season_queries;
    use crate::models::license::driver_has_required_license_for_category;
    use crate::models::season::Season;

    #[test]
    fn test_market_fills_all_vacancies() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(300);

        let report = run_market(&conn, 2, &mut rng).expect("market should run");

        assert_eq!(report.unresolved_vacancies, 0);
        assert!(find_vacancies(&conn).expect("vacancies").is_empty());
    }

    #[test]
    fn test_merit_relegation_swaps_weak_top_driver_with_feeder_champion() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        let mut team_rng = StdRng::seed_from_u64(800);
        let bmw_team = sample_team("bmw_m2", "TBMW", &mut team_rng);
        let amateur_team = sample_team("mazda_amador", "TAMA", &mut team_rng);
        team_queries::insert_team(&conn, &bmw_team).expect("bmw team");
        team_queries::insert_team(&conn, &amateur_team).expect("amateur team");

        // BMW: titular forte (P_BMW1) + fraco que terminou em último (P_WEAK).
        let bmw1 = sample_driver("P_BMW1", "BMW Forte", Some("bmw_m2"), 70.0, DriverStatus::Ativo);
        let weak = sample_driver("P_WEAK", "BMW Fraco", Some("bmw_m2"), 52.0, DriverStatus::Ativo);
        // Amador: campeão com licença 1 (exigida pela BMW) -> merece subir.
        let champ = sample_driver(
            "P_CHAMP",
            "Amador Campeao",
            Some("mazda_amador"),
            74.0,
            DriverStatus::Ativo,
        );
        for driver in [&bmw1, &weak, &champ] {
            driver_queries::insert_driver(&conn, driver).expect("driver");
        }
        let seed_contract = |id: &str, driver: &Driver, team: &crate::models::team::Team, role: TeamRole, category: &str| {
            let contract = Contract::new(
                id.to_string(),
                driver.id.clone(),
                driver.nome.clone(),
                team.id.clone(),
                team.nome.clone(),
                1,
                2,
                100_000.0,
                role,
                category.to_string(),
            );
            contract_queries::insert_contract(&conn, &contract).expect("contract");
        };
        seed_contract("CB1", &bmw1, &bmw_team, TeamRole::Numero1, "bmw_m2");
        seed_contract("CWK", &weak, &bmw_team, TeamRole::Numero2, "bmw_m2");
        seed_contract("CCH", &champ, &amateur_team, TeamRole::Numero1, "mazda_amador");
        team_queries::update_team_pilots(&conn, &bmw_team.id, Some("P_BMW1"), Some("P_WEAK"))
            .expect("bmw lineup");
        team_queries::update_team_pilots(&conn, &amateur_team.id, Some("P_CHAMP"), None)
            .expect("amateur lineup");
        conn.execute(
            "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
             VALUES ('P_CHAMP', '1', 'mazda_amador', '2024-12-31T00:00:00', 1)",
            [],
        )
        .expect("license champ");

        let ctx = |pos: i32, total: i32, categoria: &str, tier: u8| DriverMarketContext {
            posicao_campeonato: pos,
            total_pilotos: total,
            categoria: categoria.to_string(),
            category_tier: tier,
            vitorias: 0,
            poles: 0,
            titulos: 0,
            papel: TeamRole::Numero2,
        };
        let mut contexts = HashMap::new();
        contexts.insert("P_BMW1".to_string(), ctx(1, 2, "bmw_m2", 2));
        contexts.insert("P_WEAK".to_string(), ctx(2, 2, "bmw_m2", 2)); // último na BMW
        contexts.insert("P_CHAMP".to_string(), ctx(1, 10, "mazda_amador", 1)); // campeão do amador

        let teams = team_queries::get_all_teams(&conn).expect("teams");
        let mut report = MarketReport::default();
        apply_merit_relegations(&conn, &teams, 2, &contexts, &mut report)
            .expect("rebaixamento deve rodar");

        // O campeão do amador subiu para a vaga da BMW; o fraco da BMW desceu.
        let champ_contract = contract_queries::get_active_regular_contract_for_pilot(&conn, "P_CHAMP")
            .expect("champ contract query")
            .expect("champ has active contract");
        let weak_contract = contract_queries::get_active_regular_contract_for_pilot(&conn, "P_WEAK")
            .expect("weak contract query")
            .expect("weak has active contract");
        assert_eq!(champ_contract.categoria, "bmw_m2");
        assert_eq!(champ_contract.equipe_id, "TBMW");
        assert_eq!(weak_contract.categoria, "mazda_amador");
        assert_eq!(weak_contract.equipe_id, "TAMA");
        assert!(report.new_signings.iter().any(|s| s.tipo == "promocao_merito"));
        assert!(report.new_signings.iter().any(|s| s.tipo == "rebaixamento"));
    }

    #[test]
    fn test_non_rookie_vacancy_is_filled_by_promoting_from_feeder_then_rookie_at_base() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        season_queries::insert_season(&conn, &Season::new("S002".to_string(), 2, 2025))
            .expect("season");

        let mut team_rng = StdRng::seed_from_u64(700);
        let rookie_team = sample_team("mazda_rookie", "TMR", &mut team_rng);
        let amateur_team = sample_team("mazda_amador", "TMA", &mut team_rng);
        team_queries::insert_team(&conn, &rookie_team).expect("rookie team");
        team_queries::insert_team(&conn, &amateur_team).expect("amateur team");

        // Grid de rookie cheio (fonte da promoção); amador com 1 titular -> N2 vago.
        let r1 = sample_driver("PR1", "Rookie Um", Some("mazda_rookie"), 60.0, DriverStatus::Ativo);
        let r2 = sample_driver(
            "PR2",
            "Rookie Dois",
            Some("mazda_rookie"),
            50.0,
            DriverStatus::Ativo,
        );
        let a1 = sample_driver(
            "PA1",
            "Amador Um",
            Some("mazda_amador"),
            65.0,
            DriverStatus::Ativo,
        );
        for driver in [&r1, &r2, &a1] {
            driver_queries::insert_driver(&conn, driver).expect("driver");
        }
        let seed_contract = |id: &str, driver: &Driver, team: &crate::models::team::Team, role: TeamRole, category: &str| {
            let contract = Contract::new(
                id.to_string(),
                driver.id.clone(),
                driver.nome.clone(),
                team.id.clone(),
                team.nome.clone(),
                1,
                2,
                100_000.0,
                role,
                category.to_string(),
            );
            contract_queries::insert_contract(&conn, &contract).expect("contract");
        };
        seed_contract("CR1", &r1, &rookie_team, TeamRole::Numero1, "mazda_rookie");
        seed_contract("CR2", &r2, &rookie_team, TeamRole::Numero2, "mazda_rookie");
        seed_contract("CA1", &a1, &amateur_team, TeamRole::Numero1, "mazda_amador");
        team_queries::update_team_pilots(&conn, &rookie_team.id, Some("PR1"), Some("PR2"))
            .expect("rookie lineup");
        team_queries::update_team_pilots(&conn, &amateur_team.id, Some("PA1"), None)
            .expect("amateur lineup");
        // PR1 conquistou a licença 0 (top-metade do rookie) -> elegível por mérito a
        // subir para o amador (que exige licença 0). PR2 não tem -> não sobe.
        conn.execute(
            "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
             VALUES ('PR1', '0', 'mazda_rookie', '2024-12-31T00:00:00', 1)",
            [],
        )
        .expect("license PR1");
        conn.execute("UPDATE meta SET value = '500' WHERE key = 'next_driver_id'", [])
            .expect("driver counter");

        let teams = team_queries::get_all_teams(&conn).expect("teams");
        let mut report = MarketReport::default();
        let mut rng = StdRng::seed_from_u64(701);

        fill_remaining_vacancies_with_rookies(&conn, &teams, 2, &mut report, &mut rng)
            .expect("cascata deve preencher a vaga sem erro");

        // A vaga não-estreia foi preenchida por PROMOÇÃO (não pelo pool, não por erro).
        assert!(
            report.new_signings.iter().any(|s| s.tipo == "promocao"),
            "deveria haver uma promoção da categoria de baixo"
        );
        // O melhor rookie (maior skill) foi o promovido.
        assert!(report
            .new_signings
            .iter()
            .any(|s| s.tipo == "promocao" && s.driver_id == "PR1"));
        // A base recebeu exatamente 1 rookie novo (o assento aberto na estreia).
        assert_eq!(
            driver_queries::count_drivers(&conn).expect("count"),
            4,
            "3 originais + 1 rookie gerado na base da cascata"
        );
        // Nenhuma vaga regular sobrou.
        assert_eq!(
            find_vacancies(&conn)
                .expect("vacancies")
                .into_iter()
                .filter(is_regular_vacancy)
                .count(),
            0
        );
    }

    #[test]
    fn test_market_expired_contracts_processed() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(301);

        let report = run_market(&conn, 2, &mut rng).expect("market should run");

        assert!(report.contracts_expired >= 1);
        let status: String = conn
            .query_row(
                "SELECT status FROM contracts WHERE id = 'C002'",
                [],
                |row| row.get(0),
            )
            .expect("expired contract status");
        assert_eq!(status, "Expirado");
    }

    #[test]
    fn test_market_all_teams_have_two_pilots() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(302);

        run_market(&conn, 2, &mut rng).expect("market should run");

        let teams = team_queries::get_all_teams(&conn).expect("teams");
        assert!(teams
            .iter()
            .all(|team| team.piloto_1_id.is_some() && team.piloto_2_id.is_some()));
    }

    #[test]
    fn test_final_vacancy_fill_handles_production_as_regular_contract_category() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(312);
        let mut team_rng = StdRng::seed_from_u64(313);
        let production_team = sample_team("production_challenger", "T900", &mut team_rng);
        team_queries::insert_team(&conn, &production_team).expect("production team");
        for index in 0..4 {
            let driver_id = format!("P90{index}");
            let driver = sample_driver(
                &driver_id,
                &format!("Piloto Livre {index}"),
                None,
                65.0 + index as f64,
                DriverStatus::Ativo,
            );
            driver_queries::insert_driver(&conn, &driver).expect("free driver");
        }

        fill_all_remaining_vacancies(&conn, 2, &mut rng).expect("fill regular vacancies");

        let production_empty_slots: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM teams
                 WHERE id = 'T900'
                   AND (piloto_1_id IS NULL OR piloto_2_id IS NULL)",
                [],
                |row| row.get(0),
            )
            .expect("production empty slots");
        let production_contracts: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM contracts
                 WHERE equipe_id = 'T900'
                   AND status = 'Ativo'
                   AND tipo = 'Regular'
                   AND categoria = 'production_challenger'
                   AND classe = 'mazda'",
                [],
                |row| row.get(0),
            )
            .expect("production regular contracts");

        assert_eq!(production_empty_slots, 0);
        assert_eq!(production_contracts, 2);
    }

    #[test]
    fn test_regular_market_vacancy_discovery_includes_real_special_phase_categories() {
        let conn = setup_market_fixture();
        let mut team_rng = StdRng::seed_from_u64(316);
        let mut production_team = sample_team("production_challenger", "T902", &mut team_rng);
        production_team.piloto_1_id = None;
        production_team.piloto_2_id = None;
        team_queries::insert_team(&conn, &production_team).expect("production team");
        let mut endurance_team = sample_team("endurance", "T903", &mut team_rng);
        endurance_team.piloto_1_id = None;
        endurance_team.piloto_2_id = None;
        team_queries::insert_team(&conn, &endurance_team).expect("endurance team");
        let mut lmp2_team = sample_team("endurance", "T904", &mut team_rng);
        lmp2_team.classe = Some("lmp2".to_string());
        lmp2_team.piloto_1_id = None;
        lmp2_team.piloto_2_id = None;
        team_queries::insert_team(&conn, &lmp2_team).expect("endurance lmp2 team");

        let vacancies = find_vacancies(&conn).expect("vacancies");

        assert!(vacancies.iter().any(|vacancy| {
            vacancy.team_id == "T902" && vacancy.categoria == "production_challenger"
        }));
        assert!(vacancies
            .iter()
            .any(|vacancy| vacancy.team_id == "T903" && vacancy.categoria == "endurance"));
        assert!(vacancies
            .iter()
            .any(|vacancy| vacancy.team_id == "T904" && vacancy.categoria == "endurance"));
        assert!(!vacancies.iter().any(|vacancy| vacancy.categoria == "lmp2"));
    }

    #[test]
    fn test_market_creates_regular_contracts_with_team_class_for_endurance_slots() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(314);
        let mut team_rng = StdRng::seed_from_u64(315);
        let mut endurance_team = sample_team("endurance", "T901", &mut team_rng);
        endurance_team.classe = Some("lmp2".to_string());
        endurance_team.piloto_1_id = None;
        endurance_team.piloto_2_id = None;
        team_queries::insert_team(&conn, &endurance_team).expect("endurance team");

        run_market(&conn, 2, &mut rng).expect("market should run");

        let active_regular_endurance_contracts: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM contracts
                 WHERE status = 'Ativo'
                   AND tipo = 'Regular'
                   AND categoria = 'endurance'
                   AND classe = 'lmp2'
                   AND equipe_id = 'T901'",
                [],
                |row| row.get(0),
            )
            .expect("regular endurance contracts");
        let special_contracts: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM contracts
                 WHERE tipo = 'Especial'
                   AND categoria IN ('production_challenger', 'endurance')",
                [],
                |row| row.get(0),
            )
            .expect("special contracts");
        let endurance_team_after = team_queries::get_team_by_id(&conn, "T901")
            .expect("team query")
            .expect("endurance team after market");

        assert_eq!(active_regular_endurance_contracts, 2);
        assert_eq!(special_contracts, 0);
        assert!(endurance_team_after.piloto_1_id.is_some());
        assert!(endurance_team_after.piloto_2_id.is_some());
    }

    #[test]
    fn test_sync_reopens_slots_when_active_contract_category_or_class_differs_from_team() {
        let conn = setup_market_fixture();
        let mut team_rng = StdRng::seed_from_u64(317);
        let mut production_team = sample_team("production_challenger", "T904", &mut team_rng);

        let driver_a = sample_driver(
            "P904",
            "Piloto Categoria Errada",
            Some("gt4"),
            70.0,
            DriverStatus::Ativo,
        );
        let driver_b = sample_driver(
            "P905",
            "Piloto Classe Errada",
            Some("production_challenger"),
            69.0,
            DriverStatus::Ativo,
        );
        driver_queries::insert_driver(&conn, &driver_a).expect("driver a");
        driver_queries::insert_driver(&conn, &driver_b).expect("driver b");

        production_team.piloto_1_id = Some(driver_a.id.clone());
        production_team.piloto_2_id = Some(driver_b.id.clone());
        team_queries::insert_team(&conn, &production_team).expect("production team");

        let wrong_category = Contract::new(
            "C904".to_string(),
            driver_a.id.clone(),
            driver_a.nome.clone(),
            production_team.id.clone(),
            production_team.nome.clone(),
            1,
            2,
            70_000.0,
            TeamRole::Numero1,
            "gt4".to_string(),
        );
        let mut wrong_class = Contract::new(
            "C905".to_string(),
            driver_b.id.clone(),
            driver_b.nome.clone(),
            production_team.id.clone(),
            production_team.nome.clone(),
            1,
            2,
            70_000.0,
            TeamRole::Numero2,
            "production_challenger".to_string(),
        );
        wrong_class.classe = Some("toyota".to_string());
        contract_queries::insert_contract(&conn, &wrong_category).expect("wrong category");
        contract_queries::insert_contract(&conn, &wrong_class).expect("wrong class");

        let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(&conn)
            .expect("drivers")
            .into_iter()
            .map(|driver| (driver.id.clone(), driver))
            .collect();
        sync_team_slots(&conn, &[production_team.clone()], &drivers_by_id)
            .expect("sync team slots");

        let vacancies = find_vacancies(&conn).expect("vacancies");
        let reopened = vacancies
            .iter()
            .filter(|vacancy| vacancy.team_id == production_team.id)
            .count();

        assert_eq!(reopened, 2);
    }

    #[test]
    fn test_market_hierarchy_updated() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(303);

        run_market(&conn, 2, &mut rng).expect("market should run");

        let teams = team_queries::get_all_teams(&conn).expect("teams");
        assert!(teams.iter().all(|team| team.hierarquia_n1_id.is_some()));
        assert!(teams.iter().all(|team| team.hierarquia_n2_id.is_some()));
        assert!(teams
            .iter()
            .all(|team| team.hierarquia_status == TeamHierarchyClimate::Estavel.as_str()));
    }

    #[test]
    fn test_run_market_classifies_existing_free_agent_as_transfer() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(300);

        let report = run_market(&conn, 2, &mut rng).expect("market should run");
        let signing = report
            .new_signings
            .iter()
            .find(|signing| signing.driver_id == "P004")
            .expect("experienced free agent should be signed");

        assert_eq!(
            signing.tipo, "transferencia",
            "piloto veterano ja existente no save nao deve ser classificado como rookie"
        );
    }

    #[test]
    fn test_rookie_signing_candidate_only_counts_real_rookie_categories() {
        let driver = sample_driver("P999", "Piloto Novo", None, 60.0, DriverStatus::Ativo);
        let candidate = AvailableDriver {
            driver,
            visibility: 6.0,
            posicao_campeonato: 99,
            categoria_atual: String::new(),
            category_tier: 0,
            max_license_level: Some(4),
        };
        let expiring = HashMap::new();

        assert!(is_rookie_signing_candidate(
            &candidate,
            &expiring,
            "mazda_rookie"
        ));
        assert!(is_rookie_signing_candidate(
            &candidate,
            &expiring,
            "toyota_rookie"
        ));
        assert!(!is_rookie_signing_candidate(&candidate, &expiring, "gt3"));
    }

    #[test]
    fn test_pool_fallback_for_non_rookie_vacancy_uses_experienced_lower_license_before_debutant() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        let mut team_rng = StdRng::seed_from_u64(611);
        let team = sample_team("gt3", "T900", &mut team_rng);
        team_queries::insert_team(&conn, &team).expect("insert gt3 team");

        let mut existing = sample_driver(
            "P900",
            "Piloto Titular",
            Some("gt3"),
            72.0,
            DriverStatus::Ativo,
        );
        existing.stats_carreira.corridas = 80;
        let mut experienced_lower_license =
            sample_driver("P901", "Piloto Experiente", None, 70.0, DriverStatus::Ativo);
        experienced_lower_license.stats_carreira.corridas = 24;
        let mut debutant_with_license =
            sample_driver("P902", "Piloto Estreante", None, 95.0, DriverStatus::Ativo);
        debutant_with_license.stats_carreira.corridas = 0;

        for driver in [
            &existing,
            &experienced_lower_license,
            &debutant_with_license,
        ] {
            driver_queries::insert_driver(&conn, driver).expect("insert driver");
        }

        let contract = Contract::new(
            "C900".to_string(),
            existing.id.clone(),
            existing.nome.clone(),
            team.id.clone(),
            team.nome.clone(),
            1,
            2,
            120_000.0,
            TeamRole::Numero1,
            "gt3".to_string(),
        );
        contract_queries::insert_contract(&conn, &contract).expect("insert contract");
        team_queries::update_team_pilots(&conn, &team.id, Some(&existing.id), None)
            .expect("seed lineup");

        conn.execute(
            "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
             VALUES
             ('P900', '3', 'gt3', '2024-12-31T00:00:00', 2),
             ('P901', '1', 'mazda_amador', '2024-12-31T00:00:00', 2),
             ('P902', '3', 'gt3', '2024-12-31T00:00:00', 0)",
            [],
        )
        .expect("insert licenses");
        conn.execute(
            "UPDATE meta SET value = '901' WHERE key = 'next_contract_id'",
            [],
        )
        .expect("contract counter");
        conn.execute(
            "UPDATE meta SET value = '903' WHERE key = 'next_driver_id'",
            [],
        )
        .expect("driver counter");

        let teams = team_queries::get_all_teams(&conn).expect("teams");
        let mut report = MarketReport::default();
        let mut rng = StdRng::seed_from_u64(612);

        fill_remaining_vacancies_with_rookies(&conn, &teams, 2, &mut report, &mut rng)
            .expect("fill vacancy");

        let refreshed = team_queries::get_team_by_id(&conn, &team.id)
            .expect("team query")
            .expect("team");
        assert_eq!(refreshed.piloto_2_id.as_deref(), Some("P901"));
        assert_ne!(refreshed.piloto_2_id.as_deref(), Some("P902"));
        assert!(
            driver_has_required_license_for_category(&conn, "P901", "gt3")
                .expect("fallback license should be granted"),
            "piloto experiente de carteira inferior deve ser regularizado ao ser usado como fallback"
        );
    }

    #[test]
    fn test_pool_fallback_for_rookie_vacancy_keeps_retrying_rookie_before_veteran() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        let previous = Season::new("S001".to_string(), 1, 2024);
        let next = Season::new("S002".to_string(), 2, 2025);
        season_queries::insert_season(&conn, &previous).expect("previous season");
        season_queries::finalize_season(&conn, &previous.id).expect("finalize previous");
        season_queries::insert_season(&conn, &next).expect("next season");

        let mut team_rng = StdRng::seed_from_u64(613);
        let team = sample_team("mazda_rookie", "T920", &mut team_rng);
        team_queries::insert_team(&conn, &team).expect("insert rookie team");

        let mut existing = sample_driver(
            "P920",
            "Piloto Titular",
            Some("mazda_rookie"),
            62.0,
            DriverStatus::Ativo,
        );
        existing.stats_carreira.corridas = 0;
        existing.stats_carreira.temporadas = 0;
        let mut retrying_rookie = sample_driver(
            "P921",
            "Rookie Tentando",
            Some("mazda_rookie"),
            50.0,
            DriverStatus::Ativo,
        );
        retrying_rookie.stats_carreira.corridas = 8;
        retrying_rookie.stats_carreira.temporadas = 1;
        let mut amateur_veteran =
            sample_driver("P922", "Veterano Amador", None, 95.0, DriverStatus::Ativo);
        amateur_veteran.stats_carreira.corridas = 40;
        amateur_veteran.stats_carreira.temporadas = 4;

        for driver in [&existing, &retrying_rookie, &amateur_veteran] {
            driver_queries::insert_driver(&conn, driver).expect("insert driver");
        }

        let contract = Contract::new(
            "C920".to_string(),
            existing.id.clone(),
            existing.nome.clone(),
            team.id.clone(),
            team.nome.clone(),
            1,
            2,
            15_000.0,
            TeamRole::Numero1,
            "mazda_rookie".to_string(),
        );
        contract_queries::insert_contract(&conn, &contract).expect("insert contract");
        team_queries::update_team_pilots(&conn, &team.id, Some(&existing.id), None)
            .expect("seed lineup");
        insert_standing(
            &conn,
            &previous.id,
            &retrying_rookie.id,
            &team.id,
            "mazda_rookie",
            12,
            8.0,
            0,
            0,
        );
        insert_standing(
            &conn,
            &previous.id,
            &amateur_veteran.id,
            &team.id,
            "mazda_amador",
            8,
            30.0,
            0,
            0,
        );
        conn.execute(
            "UPDATE meta SET value = '921' WHERE key = 'next_contract_id'",
            [],
        )
        .expect("contract counter");

        let teams = team_queries::get_all_teams(&conn).expect("teams");
        let mut report = MarketReport::default();
        let mut rng = StdRng::seed_from_u64(614);

        fill_remaining_vacancies_with_rookies(&conn, &teams, 2, &mut report, &mut rng)
            .expect("fill vacancy");

        let refreshed = team_queries::get_team_by_id(&conn, &team.id)
            .expect("team query")
            .expect("team");
        let lineup = [
            refreshed.piloto_1_id.as_deref(),
            refreshed.piloto_2_id.as_deref(),
        ];
        assert!(lineup.contains(&Some("P921")), "lineup: {lineup:?}");
        assert!(!lineup.contains(&Some("P922")), "lineup: {lineup:?}");
    }

    #[test]
    fn test_pool_fallback_for_rookie_vacancy_generates_new_rookie_before_veteran() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        let previous = Season::new("S001".to_string(), 1, 2024);
        let next = Season::new("S002".to_string(), 2, 2025);
        season_queries::insert_season(&conn, &previous).expect("previous season");
        season_queries::finalize_season(&conn, &previous.id).expect("finalize previous");
        season_queries::insert_season(&conn, &next).expect("next season");

        let mut team_rng = StdRng::seed_from_u64(615);
        let team = sample_team("mazda_rookie", "T930", &mut team_rng);
        team_queries::insert_team(&conn, &team).expect("insert rookie team");

        let mut existing = sample_driver(
            "P930",
            "Piloto Titular",
            Some("mazda_rookie"),
            62.0,
            DriverStatus::Ativo,
        );
        existing.stats_carreira.corridas = 0;
        existing.stats_carreira.temporadas = 0;
        let mut amateur_veteran =
            sample_driver("P931", "Veterano Amador", None, 95.0, DriverStatus::Ativo);
        amateur_veteran.stats_carreira.corridas = 40;
        amateur_veteran.stats_carreira.temporadas = 4;

        for driver in [&existing, &amateur_veteran] {
            driver_queries::insert_driver(&conn, driver).expect("insert driver");
        }

        let contract = Contract::new(
            "C930".to_string(),
            existing.id.clone(),
            existing.nome.clone(),
            team.id.clone(),
            team.nome.clone(),
            1,
            2,
            15_000.0,
            TeamRole::Numero1,
            "mazda_rookie".to_string(),
        );
        contract_queries::insert_contract(&conn, &contract).expect("insert contract");
        team_queries::update_team_pilots(&conn, &team.id, Some(&existing.id), None)
            .expect("seed lineup");
        insert_standing(
            &conn,
            &previous.id,
            &amateur_veteran.id,
            &team.id,
            "mazda_amador",
            8,
            30.0,
            0,
            0,
        );
        conn.execute(
            "UPDATE meta SET value = '931' WHERE key = 'next_contract_id'",
            [],
        )
        .expect("contract counter");
        conn.execute(
            "UPDATE meta SET value = '932' WHERE key = 'next_driver_id'",
            [],
        )
        .expect("driver counter");

        let teams = team_queries::get_all_teams(&conn).expect("teams");
        let mut report = MarketReport::default();
        let mut rng = StdRng::seed_from_u64(616);

        fill_remaining_vacancies_with_rookies(&conn, &teams, 2, &mut report, &mut rng)
            .expect("fill vacancy");

        let refreshed = team_queries::get_team_by_id(&conn, &team.id)
            .expect("team query")
            .expect("team");
        let lineup = [
            refreshed.piloto_1_id.as_deref(),
            refreshed.piloto_2_id.as_deref(),
        ];
        assert!(!lineup.contains(&Some("P931")), "lineup: {lineup:?}");
        assert_eq!(report.rookies_placed, 1);

        let generated_id = lineup
            .iter()
            .flatten()
            .find(|driver_id| **driver_id != "P930")
            .expect("generated rookie in lineup");
        let generated = driver_queries::get_driver(&conn, generated_id).expect("generated driver");
        assert_eq!(generated.stats_carreira.corridas, 0);
        assert_eq!(generated.stats_carreira.temporadas, 0);
        assert_eq!(generated.ano_inicio_carreira, 2025);
    }

    #[test]
    fn test_final_vacancy_fill_leaves_non_debut_vacancy_open_when_no_candidate() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        // Ano jogável: gt3 não é categoria de entrada (seu feeder gt4 já existe),
        // então a vaga sem candidato fica aberta em vez de gerar um rookie.
        season_queries::insert_season(&conn, &Season::new("S002".to_string(), 2, 2025))
            .expect("season");

        let mut team_rng = StdRng::seed_from_u64(621);
        let team = sample_team("gt3", "T910", &mut team_rng);
        team_queries::insert_team(&conn, &team).expect("insert gt3 team");

        let mut existing = sample_driver(
            "P910",
            "Piloto Titular",
            Some("gt3"),
            72.0,
            DriverStatus::Ativo,
        );
        existing.stats_carreira.corridas = 80;
        driver_queries::insert_driver(&conn, &existing).expect("insert driver");

        let contract = Contract::new(
            "C910".to_string(),
            existing.id.clone(),
            existing.nome.clone(),
            team.id.clone(),
            team.nome.clone(),
            1,
            2,
            120_000.0,
            TeamRole::Numero1,
            "gt3".to_string(),
        );
        contract_queries::insert_contract(&conn, &contract).expect("insert contract");
        team_queries::update_team_pilots(&conn, &team.id, Some(&existing.id), None)
            .expect("seed lineup");
        conn.execute(
            "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
             VALUES ('P910', '3', 'gt3', '2024-12-31T00:00:00', 2)",
            [],
        )
        .expect("insert license");
        conn.execute(
            "UPDATE meta SET value = '911' WHERE key = 'next_driver_id'",
            [],
        )
        .expect("driver counter");

        let teams = team_queries::get_all_teams(&conn).expect("teams");
        let mut report = MarketReport::default();
        let mut rng = StdRng::seed_from_u64(622);

        // Vaga não-estreia sem candidato (sem pool e sem feeder elegível): o fill
        // não aborta a temporada e completa o grid pelo fallback de emergência. O
        // essencial é não travar nem deixar o grid quebrado.
        fill_remaining_vacancies_with_rookies(&conn, &teams, 2, &mut report, &mut rng)
            .expect("fill deve concluir sem abortar mesmo sem candidato");

        assert!(
            !find_vacancies(&conn)
                .expect("vacancies")
                .into_iter()
                .any(|vacancy| is_regular_vacancy(&vacancy)),
            "o grid deve terminar completo (sem vaga regular aberta)"
        );
    }

    #[test]
    fn test_run_market_does_not_auto_sign_player() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        let previous = Season::new("S001".to_string(), 1, 2024);
        let next = Season::new("S002".to_string(), 2, 2025);
        season_queries::insert_season(&conn, &previous).expect("previous season");
        season_queries::finalize_season(&conn, &previous.id).expect("finalize previous");
        season_queries::insert_season(&conn, &next).expect("next season");

        let mut team_rng = StdRng::seed_from_u64(404);
        let current_team = sample_team("mazda_rookie", "T001", &mut team_rng);
        let vacancy_team = sample_team("mazda_rookie", "T002", &mut team_rng);
        team_queries::insert_team(&conn, &current_team).expect("current team");
        team_queries::insert_team(&conn, &vacancy_team).expect("vacancy team");

        let mut player = sample_driver(
            "P001",
            "Jogador",
            Some("mazda_rookie"),
            80.0,
            DriverStatus::Ativo,
        );
        player.is_jogador = true;
        let retired = sample_driver(
            "P002",
            "Veterano",
            Some("mazda_rookie"),
            55.0,
            DriverStatus::Aposentado,
        );
        driver_queries::insert_driver(&conn, &player).expect("insert player");
        driver_queries::insert_driver(&conn, &retired).expect("insert retired");

        let player_contract = Contract::new(
            "C001".to_string(),
            player.id.clone(),
            player.nome.clone(),
            current_team.id.clone(),
            current_team.nome.clone(),
            1,
            1,
            45_000.0,
            TeamRole::Numero1,
            "mazda_rookie".to_string(),
        );
        let retired_contract = Contract::new(
            "C002".to_string(),
            retired.id.clone(),
            retired.nome.clone(),
            vacancy_team.id.clone(),
            vacancy_team.nome.clone(),
            1,
            1,
            20_000.0,
            TeamRole::Numero1,
            "mazda_rookie".to_string(),
        );
        contract_queries::insert_contract(&conn, &player_contract).expect("insert player contract");
        contract_queries::insert_contract(&conn, &retired_contract)
            .expect("insert retired contract");

        team_queries::update_team_pilots(&conn, &current_team.id, Some(&player.id), None)
            .expect("current team lineup");
        team_queries::update_team_pilots(&conn, &vacancy_team.id, Some(&retired.id), None)
            .expect("vacancy team lineup");

        insert_standing(
            &conn,
            &previous.id,
            &player.id,
            &current_team.id,
            "mazda_rookie",
            2,
            90.0,
            1,
            1,
        );

        conn.execute(
            "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
             VALUES ('P001', '1', 'mazda_rookie', '2024-12-31T00:00:00', 1)",
            [],
        )
        .expect("insert player license");
        conn.execute(
            "UPDATE meta SET value = '3' WHERE key = 'next_contract_id'",
            [],
        )
        .expect("contract counter");
        conn.execute(
            "UPDATE meta SET value = '3' WHERE key = 'next_driver_id'",
            [],
        )
        .expect("driver counter");

        let mut rng = StdRng::seed_from_u64(405);
        let report = run_market(&conn, 2, &mut rng).expect("market should run");
        let active_contracts = contract_queries::get_contracts_for_pilot(&conn, &player.id)
            .expect("player contracts")
            .into_iter()
            .filter(|contract| contract.status == ContractStatus::Ativo)
            .collect::<Vec<_>>();

        assert!(
            report
                .new_signings
                .iter()
                .all(|signing| signing.driver_id != player.id),
            "o mercado não deve auto-assinar o jogador"
        );
        assert!(
            active_contracts.is_empty(),
            "o jogador não deveria ganhar contrato automático; contratos ativos: {:?}",
            active_contracts
                .iter()
                .map(|contract| (&contract.id, &contract.equipe_id, &contract.categoria))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_load_market_contexts_fails_on_corrupted_standings_row() {
        let conn = setup_market_fixture();
        conn.execute(
            "UPDATE standings
             SET categoria = CAST(X'00' AS BLOB)
             WHERE temporada_id = 'S001' AND piloto_id = 'P001'",
            [],
        )
        .expect("corrupt standings row");

        let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(&conn)
            .expect("drivers")
            .into_iter()
            .map(|driver| (driver.id.clone(), driver))
            .collect();
        let expiring_by_driver: HashMap<String, Contract> = HashMap::new();

        let result = load_market_contexts(&conn, Some("S001"), &drivers_by_id, &expiring_by_driver);

        let err = result.expect_err("corrupted standings should fail");
        assert!(err.contains("Falha ao ler categoria do standings"));
        assert!(err.contains("P001"));
    }

    #[test]
    fn test_invalid_season_status_from_db_returns_error() {
        let conn = setup_market_fixture();
        conn.execute(
            "UPDATE seasons SET status = 'status_quebrado' WHERE numero = 2",
            [],
        )
        .expect("corrupt season status");

        let err = get_season_by_number(&conn, 2).expect_err("invalid season status should fail");
        assert!(err.contains("SeasonStatus inv"));
    }

    #[test]
    fn test_sync_team_slots_fails_when_active_contract_points_to_missing_driver() {
        let conn = setup_market_fixture();
        conn.execute_batch("PRAGMA foreign_keys = OFF;")
            .expect("disable foreign keys for corruption setup");
        conn.execute(
            "UPDATE contracts SET piloto_id = 'P999' WHERE id = 'C001'",
            [],
        )
        .expect("corrupt contract driver reference");
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .expect("re-enable foreign keys after corruption setup");

        let teams = team_queries::get_all_teams(&conn).expect("teams");
        let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(&conn)
            .expect("drivers")
            .into_iter()
            .map(|driver| (driver.id.clone(), driver))
            .collect();

        let err = sync_team_slots(&conn, &teams, &drivers_by_id)
            .expect_err("sync should fail for orphan active contract");

        assert!(err.contains("C001"));
        assert!(err.contains("P999"));
    }

    #[test]
    fn test_run_market_repairs_legacy_missing_licenses_before_matching() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(406);

        let report = run_market(&conn, 2, &mut rng).expect("market should run");

        assert!(
            driver_has_required_license_for_category(&conn, "P002", "gt4")
                .expect("gt4 license for expiring veteran"),
            "veteranos de gt4 sem licenca coerente devem ser corrigidos antes do mercado"
        );
        assert!(
            driver_has_required_license_for_category(&conn, "P004", "gt4")
                .expect("gt4 license for free veteran"),
            "pilotos livres da categoria atual devem receber a licenca minima"
        );
        assert!(
            driver_has_required_license_for_category(&conn, "P006", "gt3")
                .expect("gt3 license for free veteran"),
            "pilotos ativos de categorias superiores tambem precisam ser reparados"
        );
        assert!(
            report.proposals_made > 0,
            "com as licencas legadas reparadas o mercado precisa voltar a gerar propostas reais"
        );
    }

    #[test]
    fn test_sign_driver_to_team_rolls_back_contract_when_driver_update_fails() {
        let conn = setup_market_fixture();
        let vacancy = find_vacancies(&conn)
            .expect("vacancies")
            .into_iter()
            .find(|vacancy| {
                vacancy.team_id == "T002" && vacancy.papel_necessario == TeamRole::Numero2
            })
            .expect("target vacancy");
        let driver = driver_queries::get_all_drivers(&conn)
            .expect("drivers query")
            .into_iter()
            .find(|driver| driver.id == "P004")
            .expect("existing driver");

        conn.execute(
            "CREATE TRIGGER fail_driver_update
             BEFORE UPDATE ON drivers
             WHEN NEW.id = 'P004'
             BEGIN
                 SELECT RAISE(ABORT, 'driver update blocked');
             END;",
            [],
        )
        .expect("create trigger");

        let err = sign_driver_to_team(
            &conn,
            &driver,
            &vacancy,
            2,
            calculate_fallback_salary(&vacancy, &driver),
            1,
            TeamRole::Numero2,
        )
        .expect_err("signing should fail");

        assert!(
            !err.is_empty(),
            "a falha precisa ser propagada quando o update do piloto nao puder ser aplicado"
        );
        let active_contracts = contract_queries::get_contracts_for_pilot(&conn, "P004")
            .expect("contracts for pilot")
            .into_iter()
            .filter(|contract| {
                contract.status == ContractStatus::Ativo && contract.temporada_inicio == 2
            })
            .collect::<Vec<_>>();
        assert!(
            active_contracts.is_empty(),
            "a assinatura deve ser atomica e nao deixar contrato ativo apos falha no update do piloto"
        );
    }

    #[test]
    fn test_run_market_rolls_back_when_market_persist_fails() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(407);

        conn.execute(
            "CREATE TRIGGER fail_market_insert
             BEFORE INSERT ON market
             BEGIN
                 SELECT RAISE(ABORT, 'market persist blocked');
             END;",
            [],
        )
        .expect("create trigger");

        let err = run_market(&conn, 2, &mut rng).expect_err("market should fail late");
        assert!(err.contains("market persist blocked"));

        let status_c002: String = conn
            .query_row(
                "SELECT status FROM contracts WHERE id = 'C002'",
                [],
                |row| row.get(0),
            )
            .expect("contract status");
        assert_eq!(
            status_c002, "Ativo",
            "a expiracao de contratos deve ser revertida quando a persistencia final falhar"
        );

        let season_market_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM market WHERE temporada_id = 'S002'",
                [],
                |row| row.get(0),
            )
            .expect("market rows");
        assert_eq!(season_market_rows, 0);
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
        for driver in [
            &driver_a, &driver_b, &driver_c, &driver_d, &driver_e, &driver_f,
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
            2,
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
        contract_queries::insert_contract(&conn, &contract_a).expect("contract a");
        contract_queries::insert_contract(&conn, &contract_b).expect("contract b");
        contract_queries::insert_contract(&conn, &contract_c).expect("contract c");

        team_queries::update_team_pilots(&conn, &team_a.id, Some(&driver_a.id), Some(&driver_b.id))
            .expect("team a pilots");
        team_queries::update_team_pilots(&conn, &team_b.id, Some(&driver_c.id), None)
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

        conn.execute(
            "UPDATE meta SET value = '4' WHERE key = 'next_contract_id'",
            [],
        )
        .expect("contract counter");
        conn.execute(
            "UPDATE meta SET value = '7' WHERE key = 'next_driver_id'",
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
        driver.stats_carreira.corridas = 40;
        driver.stats_carreira.temporadas = 5;
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
}
