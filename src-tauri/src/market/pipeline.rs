use std::collections::{HashMap, HashSet};

use chrono::Local;
use rand::Rng;
use rusqlite::{params, Connection, OptionalExtension};

use crate::constants::categories::{get_all_categories, get_category_config, is_especial};
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::teams as team_queries;
use crate::generators::ids::{next_id, IdType};
use crate::market::driver_ai::evaluate_proposal;
use crate::market::evaluation::{estimate_expected_position, evaluate_driver_performance};
use crate::market::proposals::{
    is_real_career_debut_category, MarketProposal, MarketReport, SigningInfo, Vacancy,
};
use crate::market::renewal::should_renew_contract;
use crate::market::sync::sync_team_slots_from_active_regular_contracts;
use crate::market::team_ai::{generate_team_proposals, AvailableDriver};
use crate::market::visibility::calculate_visibility;
use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::enums::{ContractStatus, DriverStatus, TeamRole};
use crate::models::license::{
    ensure_driver_can_join_category, grant_driver_license_for_category_if_needed,
    repair_missing_licenses_for_current_categories,
};
use crate::models::team::TeamHierarchyClimate;

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
        seed_market_free_agent_pool(conn, rng)?;

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

            let new_contract = Contract::new(
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

    loop {
        let current_drivers = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao recarregar pilotos: {e}"))?;
        let current_by_id: HashMap<String, Driver> = current_drivers
            .iter()
            .cloned()
            .map(|driver| (driver.id.clone(), driver))
            .collect();

        sync_team_slots(conn, &teams, &current_by_id)?;
        let vacancies = find_vacancies(conn)?;

        // Filtra apenas vagas de categorias regulares (evita slots de convidados/especiais se houver)
        let regular_vacancies: Vec<_> = vacancies.into_iter().filter(is_regular_vacancy).collect();

        if regular_vacancies.is_empty() {
            break;
        }

        let mut report = MarketReport::default();
        fill_remaining_vacancies_with_rookies(conn, &teams, new_season_number, &mut report, rng)?;

        // Se após tentar preencher ainda persistirem as mesmas vagas (ex: erro na geração), quebra para evitar loop infinito
        let final_vacancies = find_vacancies(conn)?;
        if final_vacancies.len() >= regular_vacancies.len() {
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

fn seed_market_free_agent_pool(conn: &Connection, rng: &mut impl Rng) -> Result<usize, String> {
    let existing_drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos para pool de mercado: {e}"))?;
    let mut names: HashSet<String> = existing_drivers
        .into_iter()
        .map(|driver| driver.nome)
        .collect();
    let mut inserted = 0;

    for category in get_all_categories()
        .iter()
        .filter(|category| !is_especial(category.id))
    {
        let count = category.grid_total as usize;
        let mut ids = Vec::with_capacity(count);
        for _ in 0..count {
            ids.push(
                next_id(conn, IdType::Driver)
                    .map_err(|e| format!("Falha ao gerar ID de piloto do pool: {e}"))?,
            );
        }
        let mut id_iter = ids.into_iter();
        let mut category_drivers = Driver::generate_for_category_with_id_factory(
            category.id,
            category.tier,
            "medio",
            count,
            &mut names,
            &mut || id_iter.next().expect("pool id prealocado"),
            rng,
        );
        for driver in &mut category_drivers {
            driver.categoria_atual = None;
            driver_queries::insert_driver(conn, driver)
                .map_err(|e| format!("Falha ao inserir piloto do pool '{}': {e}", driver.nome))?;
            inserted += 1;
        }
    }

    Ok(inserted)
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
        if is_especial(&team.categoria) {
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
        ensure_driver_can_join_category(conn, &driver.id, &driver.nome, &vacancy.categoria)?;
        let new_contract = Contract::new(
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
        let team_proposals =
            generate_team_proposals(vacancy, &[player_available.clone()], new_season_number, rng);
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
    _rng: &mut impl Rng,
) -> Result<(), String> {
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
            .collect();
        if vacancies.is_empty() {
            break;
        }

        let mut available = find_available_drivers(conn, &HashMap::new())?;
        for vacancy in vacancies {
            let fallback_index = available
                .iter()
                .enumerate()
                .filter(|(_, candidate)| candidate.driver.categoria_atual.is_none())
                .max_by(|(_, a), (_, b)| compare_pool_fallback_candidates(a, b, &vacancy))
                .map(|(index, _)| index);

            if let Some(index) = fallback_index {
                let candidate = available.remove(index);
                grant_driver_license_for_category_if_needed(
                    conn,
                    &candidate.driver.id,
                    &vacancy.categoria,
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
                continue;
            }

            return Err(format!(
                "Sem pilotos desempregados no pool de pilotos para preencher a vaga {} da equipe '{}' em {}",
                vacancy.papel_necessario.as_str(),
                vacancy.team_name,
                vacancy.categoria
            ));
        }
    }

    Ok(())
}

fn is_regular_vacancy(vacancy: &Vacancy) -> bool {
    get_category_config(&vacancy.categoria)
        .map(|category| !is_especial(category.id))
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

fn pool_fallback_candidate_rank(candidate: &AvailableDriver, vacancy: &Vacancy) -> (u8, u8, u8) {
    let preferred_experience = if is_real_career_debut_category(&vacancy.categoria) {
        candidate.driver.stats_carreira.corridas == 0
    } else {
        candidate.driver.stats_carreira.corridas > 0
    };
    let required_license = get_category_config(&vacancy.categoria)
        .and_then(|category| category.licenca_necessaria)
        .unwrap_or(0);
    let has_required_license = candidate
        .max_license_level
        .map(|level| level >= required_license)
        .unwrap_or(required_license == 0);
    let license_level = candidate
        .max_license_level
        .unwrap_or(0)
        .min(required_license);

    (
        u8::from(preferred_experience),
        u8::from(has_required_license),
        license_level,
    )
}

fn calculate_fallback_salary(vacancy: &Vacancy, driver: &Driver) -> f64 {
    let tier_base = match vacancy.category_tier {
        0 => 9_000.0,
        1 => 18_000.0,
        2 => 35_000.0,
        3 => 70_000.0,
        4 => 120_000.0,
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
    fn test_final_vacancy_fill_ignores_special_category_slots() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(312);
        let mut team_rng = StdRng::seed_from_u64(313);
        let mut special_team = sample_team("gt4", "T900", &mut team_rng);
        special_team.categoria = "production_challenger".to_string();
        team_queries::insert_team(&conn, &special_team).expect("special team");

        fill_all_remaining_vacancies(&conn, 2, &mut rng).expect("fill regular vacancies");

        let regular_empty_slots: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM teams
                 WHERE categoria = 'gt4'
                   AND (piloto_1_id IS NULL OR piloto_2_id IS NULL)",
                [],
                |row| row.get(0),
            )
            .expect("regular empty slots");
        let special_empty_slots: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM teams
                 WHERE id = 'T900'
                   AND piloto_1_id IS NULL
                   AND piloto_2_id IS NULL",
                [],
                |row| row.get(0),
            )
            .expect("special empty slots");

        assert_eq!(regular_empty_slots, 0);
        assert_eq!(special_empty_slots, 1);
    }

    #[test]
    fn test_regular_market_vacancy_discovery_ignores_special_category_slots() {
        let conn = setup_market_fixture();
        let mut team_rng = StdRng::seed_from_u64(316);
        let mut special_team = sample_team("gt4", "T902", &mut team_rng);
        special_team.categoria = "endurance".to_string();
        special_team.piloto_1_id = None;
        special_team.piloto_2_id = None;
        team_queries::insert_team(&conn, &special_team).expect("special team");

        let vacancies = find_vacancies(&conn).expect("vacancies");

        assert!(vacancies
            .iter()
            .all(|vacancy| !is_especial(&vacancy.categoria)));
    }

    #[test]
    fn test_market_does_not_create_regular_contracts_for_special_category_slots() {
        let conn = setup_market_fixture();
        let mut rng = StdRng::seed_from_u64(314);
        let mut team_rng = StdRng::seed_from_u64(315);
        let mut special_team = sample_team("gt4", "T901", &mut team_rng);
        special_team.categoria = "endurance".to_string();
        special_team.piloto_1_id = None;
        special_team.piloto_2_id = None;
        team_queries::insert_team(&conn, &special_team).expect("special team");

        run_market(&conn, 2, &mut rng).expect("market should run");

        let active_regular_special_contracts: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM contracts
                 WHERE status = 'Ativo'
                   AND tipo = 'Regular'
                   AND categoria IN ('production_challenger', 'endurance')",
                [],
                |row| row.get(0),
            )
            .expect("regular special contracts");
        let special_team_after = team_queries::get_team_by_id(&conn, "T901")
            .expect("team query")
            .expect("special team after market");

        assert_eq!(active_regular_special_contracts, 0);
        assert!(special_team_after.piloto_1_id.is_none());
        assert!(special_team_after.piloto_2_id.is_none());
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
    fn test_final_vacancy_fill_never_generates_driver_when_pool_is_empty() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

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

        let error = fill_remaining_vacancies_with_rookies(&conn, &teams, 2, &mut report, &mut rng)
            .expect_err("pool exhaustion must not generate a new driver");
        let driver_count = driver_queries::count_drivers(&conn).expect("driver count");

        assert!(
            error.contains("pool de pilotos"),
            "erro deve explicar que o pool acabou: {error}"
        );
        assert_eq!(driver_count, 1);
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
