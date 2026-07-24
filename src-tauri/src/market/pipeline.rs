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

/// (Experimento) Liga a "subida garantida do campeão do Rookie": quando o Amador
/// está cheio, força a troca do 1º do Rookie com o pior do Amador. Off por padrão;
/// ligue com `IRACER_ROOKIE_MERIT=1` (ou `=true`) para o A/B no harness sim_stats.
/// Ver `guarantee_rookie_champion_promotions`.
fn rookie_merit_enabled() -> bool {
    std::env::var("IRACER_ROOKIE_MERIT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// (Anti-deflação da grade) Liga o "mercado realista" na escada viva de contratações,
/// em duas frentes complementares:
///  1. ORDEM DE ESCOLHA dos assentos passa a ponderar prestígio (reputação da equipe,
///     já carregada na vaga) além do carro — port do score de assento do motor de janela
///     (`transfer_window::driver_offer_score`, pesos `w_car`/`w_prestige`) — para o melhor
///     carro numa equipe prestigiada escolher do pool ANTES de um carro igual sem tradição.
///  2. SELEÇÃO do candidato passa a penalizar quem o assento NÃO PODE PAGAR: o preço de
///     mercado do piloto acima do teto salarial derivado do poder de gasto da equipe vira
///     penalidade, fazendo um time sem caixa DESCER para um piloto mais barato em vez de
///     assinar sempre o melhor agente livre (Problema 1: finanças limitavam o SALÁRIO, não
///     a SELEÇÃO). Penalidade SOFT (nunca filtra) para preservar a invariante de grid e
///     evitar re-scans em cascata que já travaram o sim multi-temporada.
///
/// LIGADO por padrão; desligue com `IRACER_MARKET_AFFORDABILITY=0` (ou `false`/`off`) para
/// o A/B no harness sim_stats (comparar a distribuição de skill do topo da grade com/sem).
fn market_affordability_enabled() -> bool {
    std::env::var("IRACER_MARKET_AFFORDABILITY")
        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false") || v.eq_ignore_ascii_case("off")))
        .unwrap_or(true)
}

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

#[allow(dead_code)] // superada pela Janela de Transferências (apply_weekly_market)
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

/// Mapa piloto → categoria do contrato Regular mais recente (por `temporada_fim`).
/// Serve para resgatar o nível de veteranos parados no leilão (ver `find_available_drivers`).
fn load_last_regular_categories(conn: &Connection) -> Result<HashMap<String, String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT c1.piloto_id, c1.categoria
             FROM contracts c1
             WHERE c1.tipo = 'Regular'
               AND CAST(c1.temporada_fim AS INTEGER) = (
                   SELECT MAX(CAST(c2.temporada_fim AS INTEGER))
                   FROM contracts c2
                   WHERE c2.piloto_id = c1.piloto_id AND c2.tipo = 'Regular'
               )",
        )
        .map_err(|e| format!("Falha ao preparar últimas categorias: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("Falha ao consultar últimas categorias: {e}"))?;
    let mut map = HashMap::new();
    for row in rows {
        let (piloto_id, categoria) =
            row.map_err(|e| format!("Falha ao ler última categoria: {e}"))?;
        map.insert(piloto_id, categoria);
    }
    Ok(map)
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
                    car_strength: team.car_strength(),
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
                    car_strength: team.car_strength(),
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
                car_strength: team.car_strength(),
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
                car_strength: team.car_strength(),
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
    // Última categoria contratada por piloto — para resgatar o nível de veteranos parados
    // (categoria_atual zerada por sync.rs) em vez de rebaixá-los a tier 0 no leilão.
    let last_categories = load_last_regular_categories(conn)?;
    let mut available = Vec::new();

    for driver in drivers {
        if driver.is_jogador
            || driver.status != DriverStatus::Ativo
            || contracted_ids.contains(&driver.id)
        {
            continue;
        }
        let mut context = standings_by_driver
            .get(&driver.id)
            .cloned()
            .unwrap_or_else(|| default_market_context(&driver));
        // Piloto parado (sem categoria atual nem standing da última temporada): ancora no
        // nível da última categoria que correu, espelhando `player_market_tier`. Sem isso,
        // um ex-GT3 vira candidato tier 0 (só recebe proposta de rookie).
        if context.categoria.is_empty() {
            if let Some(last_cat) = last_categories.get(&driver.id) {
                if let Some(config) = get_category_config(last_cat) {
                    context.categoria = last_cat.clone();
                    context.category_tier = config.tier;
                }
            }
        }
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
    })?;
    // Rivalidade entre EQUIPES — Fonte 2 (Elo 2) na TRANSFERÊNCIA NORMAL: assinar um piloto
    // que largou o rival na temporada passada deixa marca no par de times. Fora do savepoint
    // (best-effort — nunca desfaz a assinatura) e DEPOIS do commit, pra o histórico já incluir
    // o contrato novo. O poaching tem seu próprio site (rancor máximo), então não duplica.
    seed_ordinary_transfer_rivalry(conn, driver, &vacancy.team_id, new_season_number);
    Ok(())
}

/// Fonte 2 (Elo 2) para TRANSFERÊNCIAS NORMAIS (não-poaching): se o piloto correu na
/// temporada imediatamente anterior por um time DIFERENTE do novo, semeia rivalidade de
/// mercado LEVE (`is_poaching=false` — o peso final vem do calibre do piloto, dentro da
/// própria `seed_team_rivalry_from_transfer`). No-op para rookie (sem histórico), renovação
/// (mesmo time) ou quem voltou de um período parado (saída não-fresca, evita semear com um
/// time de anos atrás). Best-effort: um erro aqui nunca falha a assinatura.
fn seed_ordinary_transfer_rivalry(
    conn: &Connection,
    driver: &Driver,
    new_team_id: &str,
    new_season_number: i32,
) {
    let Ok(history) = contract_queries::get_contracts_for_pilot(conn, &driver.id) else {
        return;
    };
    // Histórico já vem por temporada DESC → o 1º contrato num time diferente do novo é o
    // último time real do piloto (o contrato recém-inserido é no time novo, então é ignorado).
    let Some(prev) = history.iter().find(|c| c.equipe_id != new_team_id) else {
        return;
    };
    // Só saída FRESCA: terminou na temporada imediatamente anterior à da assinatura.
    if prev.temporada_fim != new_season_number - 1 {
        return;
    }
    if let Err(e) = crate::rivalry::team::seed_team_rivalry_from_transfer(
        conn,
        &prev.equipe_id,
        new_team_id,
        driver,
        false, // transferência normal (não é assédio mid-contrato)
        new_season_number,
    ) {
        eprintln!("Aviso: falha ao semear rivalidade de equipe na transferência: {e}");
    }
}

/// Transfere `amount` do caixa do time `from` para o `to` — a 1ª mecânica de dinheiro
/// time→time (a multa de rescisão do poaching, Fase 2b). No-op se `amount ≤ 0` ou
/// mesma equipe. Debita o assediante e credita o vendedor.
pub(crate) fn transfer_between_teams(
    conn: &Connection,
    from_team: &str,
    to_team: &str,
    amount: f64,
) -> Result<(), String> {
    if amount <= 0.0 || from_team == to_team {
        return Ok(());
    }
    team_queries::adjust_team_cash(conn, from_team, -amount)
        .map_err(|e| format!("Falha ao debitar multa do assediante: {e}"))?;
    team_queries::adjust_team_cash(conn, to_team, amount)
        .map_err(|e| format!("Falha ao creditar multa ao vendedor: {e}"))?;
    Ok(())
}

/// DEBUG (Fase 2b): raio-x de um assédio — tudo que decidiu o leilão. O leilão só
/// roda entre IAs e não tem tela até o 2b.3; isto é o que o comando de debug
/// (dry-run) devolve pra dar pra ver acontecendo.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct PoachAuditBid {
    pub team_name: String,
    pub salary: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct PoachAudit {
    pub categoria: String,
    pub poacher_name: String,
    pub poacher_cash: f64,
    pub seller_name: String,
    pub target_name: String,
    pub target_skill: f64,
    pub target_fama: f64,
    pub incumbent_name: String,
    pub buyout: f64,
    pub salario_atual: f64,
    /// Vínculo do alvo com o assediante e com a casa (0–100).
    pub bond_poacher: f64,
    pub bond_holder: f64,
    pub ceiling_poacher: f64,
    pub ceiling_holder: f64,
    pub bids: Vec<PoachAuditBid>,
    pub poacher_wins: bool,
    pub salario_final: f64,
}

/// Valor de um piloto (por id) para o assediante: `poach_target_value` (skill +
/// apelo comercial × necessidade). 0 se o piloto não existe.
fn poach_value_of(drivers_by_id: &HashMap<String, Driver>, id: &str, need: f64) -> f64 {
    drivers_by_id
        .get(id)
        .map(|d| {
            crate::market::poaching::poach_target_value(
                d.atributos.skill,
                d.atributos.midia,
                need,
            )
        })
        .unwrap_or(0.0)
}

/// Executa uma quebra de contrato: o `poacher` arranca `target` (contratado no
/// `seller`) pagando a `multa`; o piloto dispensado (`incumbent`) vira agente livre
/// limpo (categoria zerada → a escada o repesca). A vaga aberta no vendedor é
/// preenchida depois pela escada. (Fase 2b.1, só IA.)
#[allow(clippy::too_many_arguments)]
fn execute_poach(
    conn: &Connection,
    poacher: &crate::models::team::Team,
    seller_team_id: &str,
    target: &Driver,
    target_contract: &Contract,
    incumbent_contract: &Contract,
    buyout: f64,
    salary: f64,
    new_season_number: i32,
    report: &mut MarketReport,
) -> Result<(), String> {
    // Rescinde o alvo (no vendedor) e o dispensado (no poacher).
    for cid in [&target_contract.id, &incumbent_contract.id] {
        contract_queries::update_contract_status(conn, cid, &ContractStatus::Rescindido)
            .map_err(|e| format!("Falha ao rescindir contrato no poaching: {e}"))?;
    }

    // O dispensado volta ao pool como agente livre LIMPO (categoria None) — senão
    // vira órfão que a escada não repesca.
    if let Some(mut incumbent) = driver_queries::get_driver(conn, &incumbent_contract.piloto_id).ok()
    {
        incumbent.categoria_atual = None;
        driver_queries::update_driver(conn, &incumbent)
            .map_err(|e| format!("Falha ao liberar dispensado no poaching: {e}"))?;
    }

    // Novo contrato: alvo → poacher, no papel do assento liberado.
    let mut contract = Contract::new(
        next_id(conn, IdType::Contract)
            .map_err(|e| format!("Falha ao gerar ID de contrato no poaching: {e}"))?,
        target.id.clone(),
        target.nome.clone(),
        poacher.id.clone(),
        poacher.nome.clone(),
        new_season_number,
        1,
        salary,
        incumbent_contract.papel.clone(),
        poacher.categoria.clone(),
    );
    contract.classe = poacher.classe.clone();
    contract_queries::insert_contract(conn, &contract)
        .map_err(|e| format!("Falha ao inserir contrato no poaching: {e}"))?;

    let mut moved = target.clone();
    moved.categoria_atual = Some(poacher.categoria.clone());
    driver_queries::update_driver(conn, &moved)
        .map_err(|e| format!("Falha ao atualizar piloto arrancado: {e}"))?;

    // A multa: dinheiro do assediante → vendedor.
    transfer_between_teams(conn, &poacher.id, seller_team_id, buyout)?;

    // Rivalidade entre EQUIPES — Fonte 2 (roubo de talento, o Elo 2): arrancar um astro
    // contratado do rival deixa marca duradoura no par de times. É o assédio mid-contrato
    // (rancor máximo). Best-effort — não desfaz o poaching se a semeadura falhar.
    if let Err(e) = crate::rivalry::team::seed_team_rivalry_from_transfer(
        conn,
        seller_team_id,
        &poacher.id,
        target,
        true, // is_poaching
        new_season_number,
    ) {
        eprintln!("Aviso: falha ao semear rivalidade de equipe no poaching: {e}");
    }

    report.new_signings.push(SigningInfo {
        driver_id: target.id.clone(),
        driver_name: target.nome.clone(),
        team_id: poacher.id.clone(),
        team_name: poacher.nome.clone(),
        categoria: poacher.categoria.clone(),
        papel: incumbent_contract.papel.as_str().to_string(),
        tipo: "poaching".to_string(),
    });
    Ok(())
}

/// Passe de POACHING / quebra de contrato (Fase 2b, só IA): na janela, times com
/// caixa arrancam astros CONTRATADOS de outros times da mesma categoria, pagando a
/// multa. Gatilho = fama + mérito (`poach_target_value`); conservador (poucos por
/// janela). NUNCA mexe no jogador nem no time dele (isso é 2b.3).
///
/// O time atual **reage** (2b.2): cada assédio vira um leilão de salário
/// (`resolve_salary_auction`) em que o vínculo do piloto com a casa é defesa. Se o
/// time segura, o aumento fica no contrato; se perde, a multa paga o consolo.
///
/// `audit` recebe o raio-x de cada assédio (só pro debug/dry-run; o jogo real
/// passa um vetor que descarta).
pub(crate) fn run_poaching_pass(
    conn: &Connection,
    teams: &[crate::models::team::Team],
    new_season_number: i32,
    rng: &mut impl Rng,
    report: &mut MarketReport,
    audit: &mut Vec<PoachAudit>,
) -> Result<(), String> {
    let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos p/ poaching: {e}"))?
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();
    let active = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao carregar contratos p/ poaching: {e}"))?;
    let contract_by_driver: HashMap<String, Contract> = active
        .iter()
        .cloned()
        .map(|c| (c.piloto_id.clone(), c))
        .collect();
    let player_seller_teams: HashSet<String> = teams
        .iter()
        .filter(|t| t.is_player_team)
        .map(|t| t.id.clone())
        .collect();

    let is_active_non_player = |id: &str| {
        drivers_by_id
            .get(id)
            .is_some_and(|d| !d.is_jogador && d.status == DriverStatus::Ativo)
    };

    // Poachers: times regulares, não do jogador, com AS DUAS vagas preenchidas (só
    // substituem quem já têm). Embaralha (determinístico via rng).
    let mut poachers: Vec<crate::models::team::Team> = teams
        .iter()
        .filter(|t| uses_regular_contracts(&t.categoria) && !t.is_player_team)
        .filter(|t| t.piloto_1_id.is_some() && t.piloto_2_id.is_some())
        .cloned()
        .collect();
    for i in (1..poachers.len()).rev() {
        let j = rng.gen_range(0..=i);
        poachers.swap(i, j);
    }

    let max_poaches = (teams.len() / 8).clamp(1, 3);
    let mut done = 0usize;
    let mut moved: HashSet<String> = HashSet::new();

    for poacher_stale in poachers {
        if done >= max_poaches {
            break;
        }
        let Some(poacher) = team_queries::get_team_by_id(conn, &poacher_stale.id)
            .ok()
            .flatten()
        else {
            continue;
        };
        let (Some(p1), Some(p2)) = (poacher.piloto_1_id.clone(), poacher.piloto_2_id.clone())
        else {
            continue; // perdeu uma vaga no meio do passe
        };
        let need = crate::fame::team_need_factor(
            crate::finance::planning::derive_budget_index_from_money(&poacher),
            poacher.reputacao,
        );

        // Incumbente = o mais fraco dos dois (por valor), não-jogador, ainda não movido.
        let mut inc_candidates: Vec<String> = [p1, p2]
            .into_iter()
            .filter(|id| is_active_non_player(id) && !moved.contains(id))
            .collect();
        inc_candidates.sort_by(|a, b| {
            poach_value_of(&drivers_by_id, a, need)
                .total_cmp(&poach_value_of(&drivers_by_id, b, need))
        });
        let Some(incumbent_id) = inc_candidates.first().cloned() else {
            continue;
        };
        let Some(incumbent_contract) = contract_by_driver.get(&incumbent_id) else {
            continue;
        };
        let incumbent_value = poach_value_of(&drivers_by_id, &incumbent_id, need);

        // Melhor alvo: contratado, mesma categoria/classe, outro time (não do jogador),
        // não-jogador, upgrade claro, multa que cabe no caixa.
        let mut best: Option<(Contract, Driver, f64)> = None;
        for c in &active {
            if c.categoria != poacher.categoria
                || c.classe != poacher.classe
                || c.equipe_id == poacher.id
                || moved.contains(&c.piloto_id)
                || player_seller_teams.contains(&c.equipe_id)
                || !is_active_non_player(&c.piloto_id)
            {
                continue;
            }
            let Some(target) = drivers_by_id.get(&c.piloto_id) else {
                continue;
            };
            let tv = crate::market::poaching::poach_target_value(
                target.atributos.skill,
                target.atributos.midia,
                need,
            );
            if !crate::market::poaching::is_clear_upgrade(tv, incumbent_value) {
                continue;
            }
            let years = (c.temporada_fim - new_season_number + 1).max(1);
            let buyout = crate::market::poaching::buyout_fee(
                c.salario_anual,
                years,
                target.atributos.skill,
                target.atributos.midia,
            );
            if !crate::market::poaching::can_afford_buyout(poacher.cash_balance, buyout) {
                continue;
            }
            let better = best.as_ref().is_none_or(|(_, bd, _)| {
                tv > crate::market::poaching::poach_target_value(
                    bd.atributos.skill,
                    bd.atributos.midia,
                    need,
                )
            });
            if better {
                best = Some((c.clone(), target.clone(), buyout));
            }
        }

        if let Some((target_contract, target, buyout)) = best {
            // Leilão de salário (Fase 2b.2): o time atual briga pra segurar. O status
            // quo é o lance de abertura — vínculo alto já é defesa, sem gastar nada.
            let Some(seller) = team_queries::get_team_by_id(conn, &target_contract.equipe_id)
                .ok()
                .flatten()
            else {
                continue;
            };
            let reference = target_contract.salario_anual;
            let poacher_side = crate::market::poaching::AuctionSide {
                team_id: poacher.id.clone(),
                team_quality: team_quality(&poacher),
                bond: crate::market::bond::get_bond(conn, &target.id, &poacher.id)?,
                // O caixa do assediante já conta a multa que ele vai desembolsar.
                ceiling: crate::market::poaching::salary_ceiling(
                    reference,
                    crate::market::poaching::poach_target_value(
                        target.atributos.skill,
                        target.atributos.midia,
                        need,
                    ),
                    poacher.cash_balance - buyout,
                ),
            };
            let seller_need = crate::fame::team_need_factor(
                crate::finance::planning::derive_budget_index_from_money(&seller),
                seller.reputacao,
            );
            let holder_side = crate::market::poaching::AuctionSide {
                team_id: seller.id.clone(),
                team_quality: team_quality(&seller),
                bond: crate::market::bond::get_bond(conn, &target.id, &seller.id)?,
                ceiling: crate::market::poaching::salary_ceiling(
                    reference,
                    crate::market::poaching::poach_target_value(
                        target.atributos.skill,
                        target.atributos.midia,
                        seller_need,
                    ),
                    seller.cash_balance,
                ),
            };
            let auction =
                crate::market::poaching::resolve_salary_auction(reference, &poacher_side, &holder_side);

            audit.push(PoachAudit {
                categoria: poacher.categoria.clone(),
                poacher_name: poacher.nome.clone(),
                poacher_cash: poacher.cash_balance,
                seller_name: seller.nome.clone(),
                target_name: target.nome.clone(),
                target_skill: target.atributos.skill,
                target_fama: target.atributos.midia,
                incumbent_name: drivers_by_id
                    .get(&incumbent_id)
                    .map(|d| d.nome.clone())
                    .unwrap_or_else(|| incumbent_id.clone()),
                buyout,
                salario_atual: reference,
                bond_poacher: poacher_side.bond,
                bond_holder: holder_side.bond,
                ceiling_poacher: poacher_side.ceiling,
                ceiling_holder: holder_side.ceiling,
                bids: auction
                    .bids
                    .iter()
                    .map(|b| PoachAuditBid {
                        team_name: if b.team_id == poacher.id {
                            poacher.nome.clone()
                        } else {
                            seller.nome.clone()
                        },
                        salary: b.salary,
                    })
                    .collect(),
                poacher_wins: auction.poacher_wins,
                salario_final: auction.salary,
            });

            if auction.poacher_wins {
                execute_poach(
                    conn,
                    &poacher,
                    &target_contract.equipe_id,
                    &target,
                    &target_contract,
                    incumbent_contract,
                    buyout,
                    auction.salary,
                    new_season_number,
                    report,
                )?;
                moved.insert(target.id.clone());
                moved.insert(incumbent_id);
                done += 1;
            } else if auction.bids.len() > 1 {
                // Houve assédio de verdade e o time atual cobriu: o aumento fica no
                // contrato (segurar um astro custa). Se ninguém chegou a dar lance,
                // não houve notícia — e o piloto segue livre pra outro assediante.
                if auction.salary > reference {
                    contract_queries::update_contract_salary(
                        conn,
                        &target_contract.id,
                        auction.salary,
                    )
                    .map_err(|e| format!("Falha ao reajustar salario na retencao: {e}"))?;
                }
                report.new_signings.push(SigningInfo {
                    driver_id: target.id.clone(),
                    driver_name: target.nome.clone(),
                    team_id: seller.id.clone(),
                    team_name: seller.nome.clone(),
                    categoria: seller.categoria.clone(),
                    papel: target_contract.papel.as_str().to_string(),
                    tipo: "retencao".to_string(),
                });
                moved.insert(target.id.clone());
            }
        }
    }

    if done > 0 {
        let refreshed: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao recarregar pilotos apos poaching: {e}"))?
            .into_iter()
            .map(|d| (d.id.clone(), d))
            .collect();
        sync_team_slots_from_active_regular_contracts(conn, teams, &refreshed)?;
    }
    Ok(())
}

/// Monta o histórico de slam de um piloto a partir do archive: todos os títulos
/// (categoria-base + classe) e o resultado campeão-ou-não por temporada na
/// categoria atual (antigo→recente). Vazio se não houver archive.
pub(crate) fn read_slam_history(
    conn: &Connection,
    driver: &Driver,
) -> Result<(Vec<slam_ambition::TitleWin>, Vec<bool>), String> {
    let current = driver.categoria_atual.clone().unwrap_or_default();
    let mut stmt = match conn.prepare(
        "SELECT categoria, posicao_campeonato, snapshot_json
         FROM driver_season_archive WHERE piloto_id = ?1 ORDER BY season_number ASC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Ok((Vec::new(), Vec::new())),
    };
    let rows = stmt
        .query_map(params![driver.id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<i32>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar historico de slam: {e}"))?;

    let mut history = Vec::new();
    let mut current_results = Vec::new();
    for row in rows {
        let (categoria, posicao, snapshot_json) =
            row.map_err(|e| format!("Falha ao ler historico de slam: {e}"))?;
        let snapshot: serde_json::Value = serde_json::from_str(&snapshot_json).unwrap_or_default();
        let cat_field = snapshot
            .get("categoria")
            .and_then(|value| value.as_str())
            .unwrap_or(&categoria);
        let base = cat_field.split(':').next().unwrap_or(cat_field).to_string();
        let class = snapshot
            .get("classe")
            .or_else(|| snapshot.get("class_name"))
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .or_else(|| {
                cat_field
                    .split_once(':')
                    .map(|(_, class)| class.to_string())
            });
        let titulos = snapshot
            .get("titulos")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        let champion = posicao == Some(1) || titulos > 0;
        if champion {
            history.push(slam_ambition::TitleWin {
                category: base.clone(),
                class: class.clone(),
            });
        }
        if base == current {
            current_results.push(champion);
        }
    }
    Ok((history, current_results))
}

/// Categoria-alvo do slam de um piloto, se ele é um slam-chaser ativo (Ambicioso
/// com slam alcançável). `Chase` → a base a coletar; `Stay` → a categoria atual;
/// `None` → não persegue slam (ou deve subir normal).
fn slam_target_category(
    conn: &Connection,
    driver: &Driver,
) -> Result<Option<(String, Option<String>)>, String> {
    if driver.personalidade_primaria != Some(PrimaryPersonality::Ambicioso) {
        return Ok(None);
    }
    let (history, current_results) = read_slam_history(conn, driver)?;
    let current = driver.categoria_atual.clone().unwrap_or_default();
    Ok(
        match slam_ambition::decide(
            &history,
            &current,
            driver.atributos.skill,
            true,
            &current_results,
        ) {
            Some(SlamDecision::Chase {
                category, class, ..
            }) => Some((category, class)),
            Some(SlamDecision::Stay { .. }) => Some((current, None)),
            None => None,
        },
    )
}

/// Passe prioritário do slam-chasing: pilotos ambiciosos (personalidade Ambicioso)
/// escolhem PRIMEIRO a melhor vaga (por car_performance) da categoria-alvo do seu
/// slam, antes da disputa normal. Remove as vagas/pilotos usados das listas.
#[allow(dead_code)] // superada pela Janela de Transferências (slam vira bônus no score)
fn apply_slam_priority_pass(
    conn: &Connection,
    vacancies: &mut Vec<Vacancy>,
    available: &mut Vec<AvailableDriver>,
    new_season_number: i32,
    rng: &mut impl Rng,
    report: &mut MarketReport,
) -> Result<(), String> {
    // (driver_id, categoria-alvo, classe-alvo, skill) de cada slam-chaser.
    let mut chasers: Vec<(String, String, Option<String>, f64)> = Vec::new();
    for candidate in available.iter() {
        if let Some((category, class)) = slam_target_category(conn, &candidate.driver)? {
            chasers.push((
                candidate.driver.id.clone(),
                category,
                class,
                candidate.driver.atributos.skill,
            ));
        }
    }
    // O mais qualificado escolhe primeiro.
    chasers.sort_by(|a, b| b.3.total_cmp(&a.3));

    for (driver_id, category, class, _) in chasers {
        let Some(driver_index) = available.iter().position(|c| c.driver.id == driver_id) else {
            continue;
        };
        // Melhor vaga (por car_performance) na categoria-alvo; classe deve bater se exigida.
        let best = vacancies
            .iter()
            .enumerate()
            .filter(|(_, vacancy)| {
                vacancy.categoria == category
                    && match &class {
                        Some(target) => vacancy.classe.as_deref() == Some(target.as_str()),
                        None => true,
                    }
            })
            .max_by(|(_, a), (_, b)| a.car_strength.total_cmp(&b.car_strength))
            .map(|(index, _)| index);
        let Some(vacancy_index) = best else {
            continue; // sem vaga na categoria-alvo → cai pro mercado normal
        };

        let candidate = available[driver_index].clone();
        let vacancy = vacancies[vacancy_index].clone();
        let salary = calculate_offer_salary(&vacancy, &candidate.driver, rng);
        let duration = if vacancy.category_tier >= 4 { 3 } else { 2 };
        sign_driver_to_team(
            conn,
            &candidate.driver,
            &vacancy,
            new_season_number,
            salary,
            duration,
            vacancy.papel_necessario.clone(),
        )?;
        report.proposals_made += 1;
        report.proposals_accepted += 1;
        report.new_signings.push(SigningInfo {
            driver_id: candidate.driver.id.clone(),
            driver_name: candidate.driver.nome.clone(),
            team_id: vacancy.team_id.clone(),
            team_name: vacancy.team_name.clone(),
            categoria: vacancy.categoria.clone(),
            papel: vacancy.papel_necessario.as_str().to_string(),
            tipo: "slam".to_string(),
        });
        vacancies.remove(vacancy_index);
        available.remove(driver_index);
    }
    Ok(())
}

/// Prestígio competitivo (0-100) de uma equipe pelos ÚLTIMOS 10 ANOS do campeonato
/// de construtores (título alto, pódio médio, com peso por recência). O que o
/// piloto mais confia (vs a promessa não-verificável do carro). Sem archive → 0.
fn team_prestige(conn: &Connection, team_id: &str, current_season: i32) -> Result<f64, String> {
    let mut stmt = match conn.prepare(
        "SELECT season_number, posicao_campeonato FROM team_season_archive
         WHERE team_id = ?1 AND season_number > ?2",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Ok(0.0),
    };
    let rows = stmt
        .query_map(params![team_id, current_season - 10], |row| {
            Ok((row.get::<_, i32>(0)?, row.get::<_, Option<i32>>(1)?))
        })
        .map_err(|e| format!("Falha ao consultar prestigio da equipe: {e}"))?;
    let mut raw = 0.0;
    for row in rows {
        let (season, pos) = row.map_err(|e| format!("Falha ao ler prestigio da equipe: {e}"))?;
        let Some(pos) = pos else { continue };
        let pts = match pos {
            1 => 10.0,
            2..=3 => 5.0,
            4..=6 => 2.0,
            _ => 0.0,
        };
        let age = (current_season - season).max(0) as f64;
        let recency = (1.0 - age / 10.0).clamp(0.1, 1.0);
        raw += pts * recency;
    }
    Ok((raw * 2.5).min(100.0))
}

/// Marca (mazda/toyota) derivada do id da categoria — só tiers 0-1.
fn brand_of_category(category: &str) -> Option<String> {
    if category.starts_with("mazda_") {
        Some("mazda".to_string())
    } else if category.starts_with("toyota_") {
        Some("toyota".to_string())
    } else {
        None
    }
}

/// Piloto-estrela sintético p/ derivar o teto de salário que a equipe comporta.
fn synthetic_star() -> Driver {
    let mut star = Driver::new(
        "STAR".to_string(),
        "Star".to_string(),
        "BR".to_string(),
        "M".to_string(),
        26,
        2000,
    );
    star.atributos.skill = 92.0;
    star
}

/// Constrói o `Seat` (vaga do motor) a partir de uma `Vacancy` do banco.
fn seat_from_vacancy(
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
fn candidate_from_available(
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
fn apply_weekly_market(
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

// ─── Janela de Transferências INTERATIVA (Fase 2): persistência + orquestração ───

fn persist_window(
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

fn load_window(
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
fn build_interactive_window(
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
fn apply_signings(
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
        let _ = crate::models::license::grant_driver_license_for_division_if_needed(
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

/// GARANTIA DE PORTA (sem deslocar ninguém): ao fechar a janela, se o JOGADOR ficou sem
/// contrato, coloca-o numa vaga VAZIA que a carteira dele alcança. Como o mercado segura
/// 2-3 assentos vazios pra ele a semana toda (ver `player_reserved_seats`), sempre há
/// vaga natural aqui — nenhum piloto da IA é dispensado pra abrir espaço. Em um save
/// degenerado sem NENHUMA vaga licenciada, ele segue agente livre nesta temporada (a
/// finalização aceita jogador sem time); volta a ter reserva na janela seguinte.
pub(crate) fn ensure_player_seated(conn: &Connection, season: i32) -> Result<(), String> {
    let Ok(player) = driver_queries::get_player_driver(conn) else {
        return Ok(());
    };
    if player.status != DriverStatus::Ativo {
        return Ok(());
    }
    if contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao checar contrato do jogador: {e}"))?
        .is_some()
    {
        return Ok(()); // Já tem time (assinou na janela).
    }

    // 1. Vaga vazia na PRÓPRIA categoria / mesmo tier (sem demoção) — normalmente um dos
    //    assentos reservados. 2. Recurso final: qualquer vaga vazia licenciada ou estreia
    //    (rookie). Nunca dispensa um piloto pra abrir vaga.
    if !place_player_in_natural_vacancy(conn, &player, season, false)? {
        place_player_in_natural_vacancy(conn, &player, season, true)?;
    }
    Ok(())
}

/// Coloca o jogador na melhor vaga DISPONÍVEL. Com `allow_fallback=false`: só própria
/// categoria → mesmo tier (sem demoção). Com `true`: também desce p/ qualquer
/// licenciada e estreia (rookie) — recurso final. Pior carro primeiro. Devolve sucesso.
fn place_player_in_natural_vacancy(
    conn: &Connection,
    player: &Driver,
    season: i32,
    allow_fallback: bool,
) -> Result<bool, String> {
    let player_tier = player_market_tier(conn, &player)?;
    let player_lic = load_max_license_levels(conn)?
        .get(&player.id)
        .copied()
        .unwrap_or(0)
        .max(
            crate::models::license::required_license_for_division(
                player.categoria_atual.as_deref().unwrap_or(""),
                None,
            )
            .unwrap_or(0),
        );
    // No fallback, a estreia (rookie) é sempre acessível (piso de recomeço).
    let licensed = |vac: &Vacancy| {
        (allow_fallback && is_real_career_debut_category(&vac.categoria))
            || crate::models::license::required_license_for_division(
                &vac.categoria,
                vac.classe.as_deref(),
            )
            .unwrap_or(0)
                <= player_lic
    };
    let player_cat = player.categoria_atual.clone().unwrap_or_default();
    let vacancies = find_vacancies(conn)?;
    let mut pick: Option<&Vacancy> = None;
    // Sem fallback: só passes 0 (categoria) e 1 (mesmo tier). Com: + 2 (qualquer).
    let passes = if allow_fallback { 3 } else { 2 };
    for pass in 0..passes {
        let mut cands: Vec<&Vacancy> = vacancies
            .iter()
            .filter(|v| {
                licensed(v)
                    && match pass {
                        0 => v.categoria == player_cat,
                        1 => v.category_tier == player_tier,
                        _ => true,
                    }
            })
            .collect();
        if !cands.is_empty() {
            cands.sort_by(|a, b| a.car_strength.total_cmp(&b.car_strength));
            pick = cands.first().copied();
            break;
        }
    }
    let Some(vac) = pick.cloned() else {
        return Ok(false);
    };
    // Concede a licença da vaga se faltar (ex.: recomeço numa estreia / sua categoria).
    let _ = crate::models::license::grant_driver_license_for_division_if_needed(
        conn,
        &player.id,
        &vac.categoria,
        vac.classe.as_deref(),
    );
    let salary = (12_000.0 + player.atributos.skill * 1_800.0).max(5_000.0);
    sign_driver_to_team(
        conn,
        player,
        &vac,
        season,
        salary,
        1,
        vac.papel_necessario.clone(),
    )?;
    Ok(true)
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
            persist_player_proposal(conn, season_id, &proposal, None)?;
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
                    salario_oferecido: calculate_offer_salary(&vacancy, &player, rng),
                    duracao_anos: 1,
                    status: crate::market::proposals::ProposalStatus::Pendente,
                    motivo_recusa: None,
                };
                persist_player_proposal(conn, season_id, &proposal, None)?;
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
        .min_by(|a, b| a.car_strength().total_cmp(&b.car_strength()))
        .copied()
}

fn best_team<'a>(teams: &[&'a crate::models::team::Team]) -> Option<&'a crate::models::team::Team> {
    teams
        .iter()
        .max_by(|a, b| a.car_strength().total_cmp(&b.car_strength()))
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
        car_strength: team.car_strength(),
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
    semana_limite: Option<i32>,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO market_proposals (
            id, temporada_id, equipe_id, piloto_id, papel, salario, status, motivo_recusa, criado_em, semana_limite
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
            semana_limite,
        ],
    )
    .map_err(|e| format!("Falha ao persistir proposta do jogador: {e}"))?;
    Ok(())
}

/// Teto do boost de pedigree e escala de referência (índice de carreira "forte").
const PEDIGREE_BOOST_MAX: f64 = 0.4;
const PEDIGREE_BOOST_SCALE: f64 = 4000.0;

/// Semanas que uma proposta formal fica de pé antes de expirar (Fase B).
const PROPOSAL_TTL_WEEKS: i32 = 3;

/// Teto de propostas formais simultâneas por PRESTÍGIO (índice do ranking). Rookie recebe
/// pouca atenção; estrela é disputada por muitas equipes. Fase B.
fn prestige_proposal_cap(index: f64) -> usize {
    match index {
        i if i >= 3.0 * PEDIGREE_BOOST_SCALE => 5,
        i if i >= 1.5 * PEDIGREE_BOOST_SCALE => 4,
        i if i >= 0.5 * PEDIGREE_BOOST_SCALE => 3,
        i if i >= 0.1 * PEDIGREE_BOOST_SCALE => 2,
        _ => 1,
    }
}

/// Curva saturante do pedigree: índice 0 → 0; cresce e satura em `PEDIGREE_BOOST_MAX`.
fn pedigree_boost_from_index(index: f64) -> f64 {
    let index = index.max(0.0);
    PEDIGREE_BOOST_MAX * (index / (index + PEDIGREE_BOOST_SCALE))
}

/// Fator de pedigree ∈ [0, `PEDIGREE_BOOST_MAX`] a partir do índice do ranking mundial
/// (currículo de carreira). Curva saturante: rookie ~0, campeão/estrela aproxima o teto.
/// Barato (índice de um piloto só). Percentil entre ativos fica como refino futuro.
fn pedigree_boost(conn: &Connection, driver: &Driver) -> Result<f64, String> {
    let index = crate::commands::global_driver_rankings::historical_index_for_driver(conn, driver)?;
    Ok(pedigree_boost_from_index(index))
}

/// FASE A das propostas formais ("Proposta recebida"): durante a janela semanal, gera
/// propostas nominais de MÉRITO pro jogador AGENTE LIVRE. Para cada vaga do mesmo tier,
/// roda a MESMA seleção da IA com o pool completo (jogador + agentes livres da IA); se a
/// equipe escolheria o jogador (ele entra no top-3 da shortlist dela), cria a proposta
/// formal. Isso é o que diferencia "Proposta recebida" (a equipe TE QUER) de "Suas
/// ofertas" (vaga aberta qualquer). Dedup por ID determinístico: não recria proposta já
/// criada e não reoferece assento recusado. Devolve os assentos das propostas PENDENTES,
/// pra a escada segurá-los. Sem fallback de piso — os assentos reservados já cobrem isso.
///
/// FASE B: propostas expiram após `PROPOSAL_TTL_WEEKS` semanas (varredura no início) e o
/// número de propostas simultâneas é limitado por PRESTÍGIO (`prestige_proposal_cap`).
pub(crate) fn generate_player_window_proposals(
    conn: &Connection,
    season: i32,
    week: i32,
    rng: &mut impl Rng,
) -> Result<Vec<String>, String> {
    let Ok(player) = driver_queries::get_player_driver(conn) else {
        return Ok(Vec::new());
    };
    if player.status != DriverStatus::Ativo {
        return Ok(Vec::new());
    }
    if contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao checar contrato do jogador: {e}"))?
        .is_some()
    {
        // Só agente livre recebe proposta formal nesta fase (poaching mid-contrato é Fase D).
        return Ok(Vec::new());
    }

    let season_row = get_season_by_number(conn, season)?
        .ok_or_else(|| format!("Temporada {season} nao encontrada"))?;
    let previous_season = get_season_by_number(conn, season - 1)?;

    // EXPIRAÇÃO (Fase B): propostas cujo prazo venceu deixam de ser pendentes → o assento
    // delas não é mais segurado e volta pra IA.
    conn.execute(
        "UPDATE market_proposals SET status = ?1
         WHERE temporada_id = ?2 AND piloto_id = ?3 AND status = ?4
           AND semana_limite IS NOT NULL AND semana_limite <= ?5",
        params![
            crate::market::proposals::ProposalStatus::Expirada.as_str(),
            season_row.id,
            player.id,
            crate::market::proposals::ProposalStatus::Pendente.as_str(),
            week,
        ],
    )
    .map_err(|e| format!("Falha ao expirar propostas do jogador: {e}"))?;

    // Contexto de visibilidade (mesmos dados usados pela IA).
    let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos: {e}"))?
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();
    let contexts = load_market_contexts(
        conn,
        previous_season.as_ref().map(|s| s.id.as_str()),
        &drivers_by_id,
        &HashMap::new(),
    )?;
    let context = contexts
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

    // Pedigree (Feature 1): o índice do ranking mundial (currículo de carreira) eleva o
    // valor de mercado — um veterano condecorado segue cortejado mesmo após temporada
    // morna. Aplica ao jogador E ao pool da IA pra a comparação de mérito ser justa.
    let player_index =
        crate::commands::global_driver_rankings::historical_index_for_driver(conn, &player)?;
    let visibility = visibility * (1.0 + pedigree_boost_from_index(player_index));

    // Teto de propostas simultâneas por prestígio (Fase B): conta as que já estão de pé
    // e só cria novas até o teto.
    let cap = prestige_proposal_cap(player_index);
    let pending_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM market_proposals
             WHERE temporada_id = ?1 AND piloto_id = ?2 AND status = ?3",
            params![
                season_row.id,
                player.id,
                crate::market::proposals::ProposalStatus::Pendente.as_str()
            ],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("Falha ao contar propostas pendentes: {e}"))?;
    let mut new_slots = cap.saturating_sub(pending_count as usize);

    // Jogador entra no pool com is_jogador=false pra ser avaliado como qualquer piloto.
    let license_levels = load_max_license_levels(conn)?;
    let mut player_as_driver = player.clone();
    player_as_driver.is_jogador = false;
    let player_available = AvailableDriver {
        driver: player_as_driver,
        visibility,
        posicao_campeonato: context.posicao_campeonato,
        categoria_atual: context.categoria.clone(),
        category_tier: context.category_tier,
        max_license_level: license_levels.get(&player.id).copied(),
    };
    let player_tier = player_market_tier(conn, &player)?;

    // Pool completo: jogador + agentes livres da IA — pra o mérito ser "a equipe te prefere
    // aos livres dela", não só "você serve". O pedigree também entra no valor de cada IA.
    let mut pool = find_available_drivers(conn, &contexts)?;
    for candidate in &mut pool {
        candidate.visibility *= 1.0 + pedigree_boost(conn, &candidate.driver)?;
    }
    pool.push(player_available);

    // Fase A: só o MESMO tier (proposta de promoção é Fase C).
    let vacancies: Vec<Vacancy> = find_vacancies(conn)?
        .into_iter()
        .filter(is_regular_vacancy)
        .filter(|v| v.category_tier == player_tier)
        .collect();

    let mut held = Vec::new();
    for vacancy in &vacancies {
        let shortlist = generate_team_proposals(vacancy, &pool, season, rng);
        // A equipe escolheria o jogador? (ele entrou no top-3 da shortlist dela)
        let Some(mut proposal) = shortlist.into_iter().find(|p| p.piloto_id == player.id) else {
            continue;
        };
        let seat = format!("{}#{}", vacancy.team_id, vacancy.papel_necessario.as_str());
        proposal.id = format!(
            "MP-{}-{}-{}-{}",
            season, vacancy.team_id, player.id, vacancy.papel_necessario.as_str()
        );
        // Dedup por status da proposta já existente com esse ID.
        let existing_status: Option<String> = conn
            .query_row(
                "SELECT status FROM market_proposals WHERE id = ?1 LIMIT 1",
                params![proposal.id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| format!("Falha ao checar proposta existente: {e}"))?;
        match existing_status.as_deref() {
            Some(s) if s == crate::market::proposals::ProposalStatus::Pendente.as_str() => {
                held.push(seat) // já pendente → segura
            }
            Some(_) => {} // recusada/aceita/expirada → não reoferece nem segura
            None => {
                if new_slots == 0 {
                    continue; // teto de prestígio atingido nesta janela
                }
                proposal.piloto_nome = player.nome.clone();
                persist_player_proposal(
                    conn,
                    &season_row.id,
                    &proposal,
                    Some(week + PROPOSAL_TTL_WEEKS),
                )?;
                held.push(seat);
                new_slots -= 1;
            }
        }
    }
    Ok(held)
}

fn fill_remaining_vacancies_with_rookies(
    conn: &Connection,
    teams: &[crate::models::team::Team],
    new_season_number: i32,
    report: &mut MarketReport,
    rng: &mut impl Rng,
    limit: Option<usize>,
    reserved: &HashSet<String>,
) -> Result<(), String> {
    let debut_year = get_season_by_number(conn, new_season_number)?
        .map(|season| season.ano)
        .unwrap_or_else(|| Local::now().year());

    // Necessidade financeira por time (Fase 2a): quanto o time pesa a fama de um
    // candidato. Carente pesa alto (precisa do patrocínio); dinastia rica pesa baixo.
    let team_need_by_id: HashMap<String, f64> = teams
        .iter()
        .map(|team| {
            let budget_index = crate::finance::planning::derive_budget_index_from_money(team);
            (
                team.id.clone(),
                crate::fame::team_need_factor(budget_index, team.reputacao),
            )
        })
        .collect();

    // Teto salarial por time (Item 1): quanto a folha de UM piloto do time comporta,
    // derivado do poder de gasto (`calculate_salary_ceiling` já pondera caixa, dívida,
    // estado financeiro e reputação). Alimenta a penalidade de affordability na seleção.
    // Vazio quando a flag está off → seleção volta ao comportamento antigo (sem penalidade).
    let team_ceiling_by_id: HashMap<String, f64> = if market_affordability_enabled() {
        teams
            .iter()
            .map(|team| {
                (
                    team.id.clone(),
                    crate::finance::salary::calculate_salary_ceiling(team),
                )
            })
            .collect()
    } else {
        HashMap::new()
    };

    loop {
        let current_drivers = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao recarregar pilotos: {e}"))?;
        let current_by_id: HashMap<String, Driver> = current_drivers
            .iter()
            .cloned()
            .map(|driver| (driver.id.clone(), driver))
            .collect();
        sync_team_slots(conn, teams, &current_by_id)?;
        let mut vacancies: Vec<_> = find_vacancies(conn)?
            .into_iter()
            .filter(is_regular_vacancy)
            .filter(|vacancy| is_category_active_in_year(&vacancy.categoria, debut_year))
            .filter(|v| {
                !reserved.contains(&format!("{}#{}", v.team_id, v.papel_necessario.as_str()))
            })
            .collect();
        if vacancies.is_empty() {
            break;
        }
        // Preenche as vagas de TOPO primeiro (tier decrescente). `find_vacancies`
        // devolve na ordem dos times; sem ordenar, um craque livre no pool de resgate
        // passa o piso de quase toda vaga e é assinado pela 1ª que aparece na
        // iteração (amador/gt4) ANTES de a vaga de GT3/endurance ser processada —
        // enterrando o talento num tier baixo. Ordenando por tier desc, o topo
        // escolhe do pool antes das categorias inferiores o capturarem.
        //
        // Desempate DENTRO do tier por DESEJABILIDADE do assento decrescente: cada vaga
        // pega o MELHOR candidato do pool (max por `compare_pool_fallback_candidates`),
        // logo processar o assento mais desejável primeiro faz o melhor assento ficar com
        // o melhor piloto disponível. Sem esse desempate, a ordem de times era arbitrária
        // e um assento pior do mesmo tier abocanhava o craque antes do melhor — a raiz do
        // "melhor carro ≠ melhor piloto" que deflaciona a grade.
        //
        // Com o mercado realista (flag), desejabilidade = carro + PRESTÍGIO (reputação da
        // equipe), port do score de assento do motor de janela — o melhor carro numa
        // equipe prestigiada escolhe antes de um carro igual sem tradição. Sem a flag,
        // desempata só por `car_performance` (comportamento antigo). `sort_by` é estável,
        // então assentos empatados (ex.: N1/N2 do mesmo time) preservam a ordem original.
        let use_market_realism = market_affordability_enabled();
        vacancies.sort_by(|a, b| {
            b.category_tier.cmp(&a.category_tier).then_with(|| {
                if use_market_realism {
                    seat_desirability(b)
                        .partial_cmp(&seat_desirability(a))
                        .unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    b.car_strength
                        .partial_cmp(&a.car_strength)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }
            })
        });

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
            // Pacing: o chamador paginado passa um report novo, logo a contagem de
            // assinaturas deste relatório == as feitas nesta chamada. Ao atingir o
            // limite, encerra (as vagas restantes ficam pra próxima semana/fecho).
            if limit.is_some_and(|l| report.new_signings.len() >= l) {
                return Ok(());
            }
            let need_factor = team_need_by_id
                .get(&vacancy.team_id)
                .copied()
                .unwrap_or(crate::fame::TEAM_NEED_MIN);
            // `None` (flag off / time sem teto) → sem penalidade de affordability.
            let team_ceiling = team_ceiling_by_id.get(&vacancy.team_id).copied();
            let is_debut_vacancy = is_real_career_debut_category(&vacancy.categoria)
                || is_entry_category_for_year(&vacancy.categoria, debut_year);
            let fallback_index = available
                .iter()
                .enumerate()
                .filter(|(_, candidate)| is_pool_fallback_candidate(candidate, &vacancy))
                .max_by(|(_, a), (_, b)| {
                    compare_pool_fallback_candidates(a, b, &vacancy, need_factor, team_ceiling)
                })
                .map(|(index, _)| index);

            // O pool de resgate roda primeiro (mais barato que a cascata de promoção).
            // O item B (piso de skill em is_pool_fallback_candidate) já barra órfão fraco
            // aqui, então não é preciso reordenar antes da promoção meritória — reordenar
            // disparava re-scans em cascata a cada promoção e travava o sim multi-temporada.
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
                    calculate_offer_salary(&vacancy, &candidate.driver, rng),
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

            if is_debut_vacancy {
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
            // Portão de MÉRITO na escada regular. Categorias de fase especial
            // (endurance/production) hoje mantêm o comportamento antigo (concede a
            // licença ao assinar) — endurecer isso (gate de licença real) desestabilizou
            // o sim multi-temporada; fica para uma investigação à parte de #3.
            let is_special_vacancy = runs_in_special_phase(&vacancy.categoria);
            let required_license = if is_special_vacancy {
                None
            } else {
                required_license_for_division(&vacancy.categoria, vacancy.classe.as_deref())
            };
            let feeder_candidate = best_feeder_promotion_candidate(
                &vacancy,
                &current_by_id,
                &market_contexts,
                &license_levels,
                required_license,
            );
            // Recrutamento profundo (demanda de time + aceite): para uma vaga de topo
            // mal servida pelo feeder, o time busca o craque preso nas categorias
            // inferiores e lhe faz proposta; o piloto decide. Prefere o recrutado que
            // aceitou; senão, segue a escada normal com o candidato do feeder.
            let deep_candidate = deep_recruitment_candidate(
                conn,
                &vacancy,
                &current_by_id,
                &market_contexts,
                &license_levels,
                required_license,
                feeder_candidate
                    .as_ref()
                    .map(|driver| driver.atributos.skill),
                rng,
            )?;
            let was_deep = deep_candidate.is_some();
            if let Some(candidate) = deep_candidate.or(feeder_candidate) {
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
                    // Especiais: concede a licença da divisão ao assinar.
                    grant_driver_license_for_division_if_needed(
                        conn,
                        &candidate.id,
                        &vacancy.categoria,
                        vacancy.classe.as_deref(),
                    )?;
                }
                // Escada regular: sem concessão — o candidato JÁ possui a licença
                // exigida (filtro de mérito em best_feeder_promotion_candidate).
                sign_driver_to_team(
                    conn,
                    &candidate,
                    &vacancy,
                    new_season_number,
                    calculate_offer_salary(&vacancy, &candidate, rng),
                    1,
                    vacancy.papel_necessario.clone(),
                )?;
                if was_deep {
                    // Ligou os dois cérebros: proposta feita e ACEITA pelo craque da
                    // várzea (o feeder míope nunca o alcançaria).
                    report.proposals_made += 1;
                    report.proposals_accepted += 1;
                }
                report.new_signings.push(SigningInfo {
                    driver_id: candidate.id.clone(),
                    driver_name: candidate.nome.clone(),
                    team_id: vacancy.team_id.clone(),
                    team_name: vacancy.team_name.clone(),
                    categoria: vacancy.categoria.clone(),
                    papel: vacancy.papel_necessario.as_str().to_string(),
                    tipo: if was_deep { "recrutamento" } else { "promocao" }.to_string(),
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
                    calculate_offer_salary(&vacancy, &candidate, rng),
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

/// Wrapper paginado da escada (ladder fill): carrega as equipes e chama
/// `fill_remaining_vacancies_with_rookies` com um teto de assinaturas (`limit`) e um
/// conjunto de assentos reservados (não preenche). Usado pela Janela ao vivo —
/// `preseason.rs` não precisa carregar `teams`.
pub(crate) fn fill_vacancies_paced(
    conn: &Connection,
    season: i32,
    limit: Option<usize>,
    reserved: &HashSet<String>,
    report: &mut MarketReport,
    rng: &mut impl Rng,
) -> Result<(), String> {
    let teams = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao carregar equipes para a escada paginada: {e}"))?;
    fill_remaining_vacancies_with_rookies(conn, &teams, season, report, rng, limit, reserved)
}

/// Tier de mercado do jogador para efeito de ofertas. Usa a `categoria_atual`; se ela
/// já foi limpa (agente livre há tempo — `sync.rs` zera o categoria_atual de quem não
/// tem contrato regular), cai na categoria do ÚLTIMO contrato. Assim um jogador que
/// ficou sem correr volta a receber ofertas NO NÍVEL DELE, e não rebaixado a rookie.
fn player_market_tier(conn: &Connection, player: &Driver) -> Result<u8, String> {
    if let Some(tier) = player
        .categoria_atual
        .as_deref()
        .and_then(crate::constants::categories::get_category_config)
        .map(|c| c.tier)
    {
        return Ok(tier);
    }
    let last = find_last_player_category(conn, &player.id)?;
    Ok(crate::constants::categories::get_category_config(&last)
        .map(|c| c.tier)
        .unwrap_or(0))
}

/// Nível de licença máximo do jogador (licença efetiva = maior entre a possuída e a
/// exigida pela categoria atual). Reaproveita o estilo de `place_player_in_natural_vacancy`.
fn player_effective_license(conn: &Connection, player: &Driver) -> Result<u8, String> {
    Ok(load_max_license_levels(conn)?
        .get(&player.id)
        .copied()
        .unwrap_or(0)
        .max(
            crate::models::license::required_license_for_division(
                player.categoria_atual.as_deref().unwrap_or(""),
                None,
            )
            .unwrap_or(0),
        ))
}

/// Salário ofertado ao jogador numa vaga. Mesma fórmula usada na garantia de porta.
/// Salário ofertado ao jogador, na MESMA escala dos contratos da IA: faixa por tier
/// (`salary_range_for_tier`) posicionada pela skill, com fator de papel (N1 titular
/// ganha mais que N2). Antes usava uma fórmula fixa `12k + skill*1.8k` que ignorava a
/// categoria e inflava o valor (ex.: ~100k no rookie, que na verdade paga ~5k–21k).
/// O `team_id` aplica uma variação ESTÁVEL (±7%, determinística) pra cada equipe ter
/// seu próprio número — sem isso todas ofereciam exatamente o mesmo valor.
fn player_offer_salary(tier: u8, is_n1: bool, skill: f64, team_id: &str) -> f64 {
    let (base_min, base_max) = crate::models::contract::salary_range_for_tier(tier);
    let t = (skill / 100.0).clamp(0.0, 1.0);
    let base = base_min + (base_max - base_min) * t;
    // Fatores no MEIO das faixas da IA (N1 1.20–1.40, N2 1.00–1.12).
    let role_mult = if is_n1 { 1.30 } else { 1.06 };
    (base * role_mult * team_salary_multiplier(team_id))
        .round()
        .max(5_000.0)
}

/// Multiplicador salarial ESTÁVEL por equipe (0.93–1.07 = ±7%), derivado de um hash
/// determinístico do id — mesmo time sempre dá o mesmo número (oferta = assinatura),
/// mas times diferentes variam. Dá "personalidade" às ofertas sem aleatoriedade.
fn team_salary_multiplier(team_id: &str) -> f64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a offset basis
    for b in team_id.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3); // FNV prime
    }
    let frac = (hash % 1000) as f64 / 999.0; // 0.0..1.0
    0.93 + frac * 0.14 // 0.93..1.07
}

/// Quantos assentos o mercado segura pro jogador agente livre. Segurar ALGUNS (não um
/// só) dá escolha real e, principalmente, garante que na última semana haja vaga VAZIA
/// pra ele — sem precisar dispensar nenhum piloto da IA pra abrir espaço.
const MAX_PLAYER_RESERVED_SEATS: usize = 3;

/// Se o jogador está ativo e SEM contrato regular ativo, SEGURA até
/// `MAX_PLAYER_RESERVED_SEATS` assentos regulares que a carteira dele alcança — os mais
/// acessíveis primeiro (tier dele → tier−1 → qualquer licenciada; pior carro primeiro
/// dentro de cada faixa). A escada poupa esses assentos toda semana, então eles seguem
/// VAZIOS até o fecho, quando o jogador escolhe um (ou é colocado) e os demais são
/// repostos pela IA. Devolve os `seat_id` `team#papel`, ou vazio se ele já tem contrato
/// / não está ativo. Nunca desloca ninguém: só reserva vagas já vazias.
pub(crate) fn player_reserved_seats(
    conn: &Connection,
    season: i32,
) -> Result<Vec<String>, String> {
    let _ = season;
    let Ok(player) = driver_queries::get_player_driver(conn) else {
        return Ok(Vec::new());
    };
    if player.status != DriverStatus::Ativo {
        return Ok(Vec::new());
    }
    if contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao checar contrato do jogador: {e}"))?
        .is_some()
    {
        return Ok(Vec::new());
    }

    let player_tier = player_market_tier(conn, &player)?;
    let player_lic = player_effective_license(conn, &player)?;
    // Vaga de estreia (rookie) é sempre acessível ao jogador (piso de recomeço).
    let licensed = |vac: &Vacancy| {
        is_real_career_debut_category(&vac.categoria)
            || crate::models::license::required_license_for_division(
                &vac.categoria,
                vac.classe.as_deref(),
            )
            .unwrap_or(0)
                <= player_lic
    };
    let vacancies: Vec<Vacancy> = find_vacancies(conn)?
        .into_iter()
        .filter(is_regular_vacancy)
        .collect();
    // Passes: tier do jogador → tier−1 → qualquer licenciada; pior carro primeiro.
    // Acumula até o teto, sem repetir assento entre as faixas.
    let mut seats: Vec<String> = Vec::new();
    for pass in 0..3 {
        let mut cands: Vec<&Vacancy> = vacancies
            .iter()
            .filter(|v| {
                licensed(v)
                    && match pass {
                        0 => v.category_tier == player_tier,
                        1 => player_tier > 0 && v.category_tier == player_tier - 1,
                        _ => true,
                    }
            })
            .collect();
        cands.sort_by(|a, b| a.car_strength.total_cmp(&b.car_strength));
        for vac in cands {
            let seat = format!("{}#{}", vac.team_id, vac.papel_necessario.as_str());
            if !seats.contains(&seat) {
                seats.push(seat);
                if seats.len() >= MAX_PLAYER_RESERVED_SEATS {
                    return Ok(seats);
                }
            }
        }
    }
    Ok(seats)
}

/// Ofertas do mercado pro JOGADOR nesta semana: toda vaga regular em que ele é
/// elegível+licenciado, no tier dele OU tier−1. Vazio se ele já tem contrato / não
/// está ativo. (`is_n1` = papel Numero1; salário = fórmula de garantia de porta.)
/// Posição final do jogador na temporada arquivada mais recente (menor número = melhor).
/// None se ainda não correu. Usado pra decidir ofertas de promoção (pódio). Lê do ARQUIVO
/// persistente (`driver_season_archive`) — NÃO do `standings`, que é recalculado do zero e
/// deletado a cada avanço de temporada (e exclui agentes livres que não correram).
fn player_last_finish_position(conn: &Connection, player_id: &str) -> Option<i32> {
    conn.query_row(
        "SELECT posicao_campeonato FROM driver_season_archive
         WHERE piloto_id = ?1 AND posicao_campeonato IS NOT NULL
         ORDER BY season_number DESC
         LIMIT 1",
        params![player_id],
        |row| row.get::<_, i32>(0),
    )
    .ok()
}

/// Os POUCOS MELHORES times que cobiçam o jogador pela fama — o "interesse ativo"
/// VISÍVEL ao jogador (badge + N1 + prêmio + e-mail). Retorna `(team_id, nome,
/// categoria)` dos melhores da categoria do jogador (exceto o dele), quantos =
/// `active_interest_team_count(fama)`. Decoplado da economia da IA (que dá apelo aos
/// times CARENTES): pro jogador, "me querem" tem que ler como "time bom". Fase 2a.
pub(crate) fn player_active_interest_teams(
    conn: &Connection,
    player: &Driver,
) -> Result<Vec<(String, String, String)>, String> {
    let count = crate::fame::active_interest_team_count(player.atributos.midia);
    if count == 0 {
        return Ok(Vec::new());
    }
    let categoria = match player.categoria_atual.as_deref() {
        Some(c) if !c.is_empty() => c.to_string(),
        _ => return Ok(Vec::new()),
    };
    let player_team_id = contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .ok()
        .flatten()
        .map(|c| c.equipe_id);
    let mut teams: Vec<_> = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao carregar times p/ interesse do astro: {e}"))?
        .into_iter()
        .filter(|team| team.ativa && team.categoria == categoria)
        .filter(|team| Some(&team.id) != player_team_id.as_ref())
        .collect();
    teams.sort_by(|a, b| {
        team_quality(b).total_cmp(&team_quality(a))
    });
    Ok(teams
        .into_iter()
        .take(count)
        .map(|team| (team.id, team.nome, team.categoria))
        .collect())
}

fn team_quality(team: &crate::models::team::Team) -> f64 {
    // `team_prestige_quality` sempre assumiu carro em 0–100 (ela clampa nisso, e os testes
    // passam 90). Recebia a coluna legada em 0–16: o carro pesava no máximo 9,6 contra 100 de
    // reputação, então o astro só olhava para prestígio. `car_strength` entrega a escala certa.
    crate::fame::team_prestige_quality(
        team.reputacao,
        team.car_strength(),
        team.historico_titulos_pilotos + team.historico_titulos_construtores,
    )
}

// ============================================================================
// Fase 2b.3 — QUEBRA DE CONTRATO DO JOGADOR (o leilão que o jogador VÊ e decide).
// ============================================================================
//
// O poaching IA-vs-IA (2b.1/2b.2) roda auto-resolvido nas pré-passes e NUNCA toca o
// jogador. Aqui é o inverso: um time claramente melhor bate à porta do jogador
// CONTRATADO, dispara o leilão de salário (mesmo motor), mas em vez de executar,
// devolve o negócio pra UI — e a PALAVRA FINAL é do jogador (Sair ou Ficar).

/// Fama mínima (Estrela+) pra o jogador virar alvo de quebra de contrato. RARO.
const PLAYER_POACH_MIN_FAMA: f64 = 70.0;
/// Gap MÍNIMO de qualidade pra um time cobiçar o jogador contratado — o assédio é
/// especial: só um time claramente MELHOR que o atual faz esse esforço.
const PLAYER_POACH_QUALITY_MARGIN: f64 = 15.0;

/// Um lance do leilão, já pronto pra tela (nome do time + de quem é o lance).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PoachBid {
    pub team_name: String,
    pub is_poacher: bool,
    pub salary: f64,
    /// "abertura" no 1º (status quo do time atual) | "lance N" nos seguintes.
    pub label: String,
}

/// Rótulo de um lance do leilão (display): 1º = "abertura", seguintes = "lance N".
fn bid_label(i: usize) -> String {
    if i == 0 {
        rust_i18n::t!("market.bid.opening").to_string()
    } else {
        rust_i18n::t!("market.bid.nth", n = i).to_string()
    }
}

/// Uma proposta de quebra de contrato dirigida ao JOGADOR — carrega os ids p/ a
/// resolução E os campos de exibição p/ a tela do leilão. Persistida no plano da
/// pré-temporada (uma por janela, no máx), consumida ao decidir.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlayerPoachOffer {
    // --- resolução ---
    pub current_contract_id: String,
    pub current_team_id: String,
    pub suitor_team_id: String,
    // --- números do negócio ---
    pub buyout: f64,
    pub current_salary: f64,
    /// Salário se SAIR (melhor lance do assediante).
    pub poacher_best: f64,
    /// Salário se FICAR (melhor cobertura do time atual; ≥ atual).
    pub holder_best: f64,
    // --- exibição ---
    pub suitor_name: String,
    pub suitor_color: String,
    pub suitor_car_rating: u8,
    pub current_team_name: String,
    pub current_team_color: String,
    pub category_label: String,
    /// Quem sairia da vaga do assediante pra dar lugar ao jogador (None se vaga aberta).
    pub incumbent_name: Option<String>,
    pub player_fama: u8,
    pub bids: Vec<PoachBid>,
    /// Quem venceu ECONOMICAMENTE (só narrativa — a palavra é do jogador).
    pub poacher_wins: bool,
}

/// Resultado de resolver a quebra de contrato do jogador.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlayerPoachOutcome {
    /// A decisão surtiu efeito? (false = a oferta estava velha/expirada.)
    pub applied: bool,
    /// O jogador saiu (true) ou ficou (false).
    pub left: bool,
    pub salary: f64,
    pub team_name: String,
    pub note: String,
}

/// Valor de um piloto (Driver) para um time, dado o need factor do time.
fn driver_poach_value(driver: &Driver, need: f64) -> f64 {
    crate::market::poaching::poach_target_value(
        driver.atributos.skill,
        driver.atributos.midia,
        need,
    )
}

/// Mínimo de lances mostrados na tela do jogador quando há disputa de verdade
/// (abertura + 4 turnos). Só afeta a EXIBIÇÃO — a economia real (poacher_best/
/// holder_best/vencedor) já está decidida.
const PLAYER_MIN_DISPLAY_BIDS: usize = 5;

/// Monta os lances a MOSTRAR pro jogador. Se os dois lados subiram de verdade e o
/// leilão real convergiu rápido (poucos lances), dramatiza numa sequência crescente
/// de ao menos [`PLAYER_MIN_DISPLAY_BIDS`] alternando os times e terminando no
/// vencedor pelo salário final. Se o time atual NÃO brigou (holder_best == atual),
/// não inventa disputa: devolve os lances reais.
fn build_player_display_bids(
    real: &[PoachBid],
    suitor_name: &str,
    holder_name: &str,
    current_salary: f64,
    poacher_best: f64,
    holder_best: f64,
) -> Vec<PoachBid> {
    let both_fought =
        poacher_best > current_salary + 1.0 && holder_best > current_salary + 1.0;
    if real.len() >= PLAYER_MIN_DISPLAY_BIDS || !both_fought {
        return real.to_vec();
    }

    // Candidatos crescentes: abertura (status quo) + subidas intermediárias de cada
    // lado + os finais reais dos dois. Ordena por valor → sobe monotônico, o maior
    // (do vencedor) fica por último.
    let mut vals: Vec<(bool, f64)> = vec![
        (false, current_salary),
        (true, current_salary + 0.5 * (poacher_best - current_salary)),
        (false, current_salary + 0.6 * (holder_best - current_salary)),
        (true, poacher_best),
        (false, holder_best),
    ];
    vals.sort_by(|a, b| a.1.total_cmp(&b.1));

    vals.into_iter()
        .enumerate()
        .map(|(i, (is_poacher, salary))| PoachBid {
            team_name: if is_poacher { suitor_name } else { holder_name }.to_string(),
            is_poacher,
            salary: salary.round(),
            label: bid_label(i),
        })
        .collect()
}

/// Detecta se o JOGADOR está sendo cobiçado por um time claramente melhor e monta o
/// leilão de salário SEM executar. Retorna None na esmagadora maioria das viradas —
/// é raro por design (fama Estrela+ E um pretendente muito acima do time atual).
pub(crate) fn compute_player_poach_offer(
    conn: &Connection,
    new_season_number: i32,
) -> Result<Option<PlayerPoachOffer>, String> {
    compute_player_poach_offer_inner(conn, new_season_number, false)
}

/// Variante de DEBUG: relaxa os portões (fama, gap de qualidade, upgrade, caixa) e
/// escolhe o pretendente mais RICO da categoria — pra forçar o leilão a aparecer e
/// dar pra testar a tela mesmo num save sem o cenário raro. Só usada pelo debug.
pub(crate) fn debug_build_player_poach_offer(
    conn: &Connection,
    new_season_number: i32,
) -> Result<Option<PlayerPoachOffer>, String> {
    compute_player_poach_offer_inner(conn, new_season_number, true)
}

fn compute_player_poach_offer_inner(
    conn: &Connection,
    new_season_number: i32,
    force: bool,
) -> Result<Option<PlayerPoachOffer>, String> {
    let Ok(player) = driver_queries::get_player_driver(conn) else {
        return Ok(None);
    };
    if player.status != DriverStatus::Ativo
        || (!force && player.atributos.midia <= PLAYER_POACH_MIN_FAMA)
    {
        return Ok(None);
    }
    let Some(current) = contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao checar contrato do jogador: {e}"))?
    else {
        return Ok(None); // agente livre não é "arrancado" — ele já escolhe pela escada
    };
    let Some(current_team) = team_queries::get_team_by_id(conn, &current.equipe_id)
        .map_err(|e| format!("Falha ao carregar time atual do jogador: {e}"))?
    else {
        return Ok(None);
    };
    let current_quality = team_quality(&current_team);

    let years = (current.temporada_fim - new_season_number + 1).max(1);
    let buyout = crate::market::poaching::buyout_fee(
        current.salario_anual,
        years,
        player.atributos.skill,
        player.atributos.midia,
    );

    let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos p/ poach do jogador: {e}"))?
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();
    let teams = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao carregar times p/ poach do jogador: {e}"))?;

    // Melhor pretendente: time regular, não-jogador, mesma categoria+classe, as DUAS
    // vagas cheias, qualidade claramente > a atual, que vê o jogador como upgrade e
    // consegue pagar a multa. Fica com o de MAIOR qualidade.
    let mut best: Option<(crate::models::team::Team, f64, Option<String>)> = None;
    for t in &teams {
        if t.is_player_team || t.id == current_team.id || !uses_regular_contracts(&t.categoria) {
            continue;
        }
        if t.categoria != current.categoria || t.classe != current.classe {
            continue;
        }
        let (Some(p1), Some(p2)) = (t.piloto_1_id.clone(), t.piloto_2_id.clone()) else {
            continue;
        };
        let q = team_quality(t);
        if !force && q <= current_quality + PLAYER_POACH_QUALITY_MARGIN {
            continue;
        }
        if !force && !crate::market::poaching::can_afford_buyout(t.cash_balance, buyout) {
            continue;
        }
        let need = crate::fame::team_need_factor(
            crate::finance::planning::derive_budget_index_from_money(t),
            t.reputacao,
        );
        let player_value =
            crate::market::poaching::poach_target_value(player.atributos.skill, player.atributos.midia, need);
        let mut occ: Vec<(String, f64)> = [p1, p2]
            .into_iter()
            .filter_map(|id| drivers_by_id.get(&id).map(|d| (id, driver_poach_value(d, need))))
            .collect();
        occ.sort_by(|a, b| a.1.total_cmp(&b.1));
        let Some((weak_id, weak_value)) = occ.first().cloned() else {
            continue;
        };
        if !force && !crate::market::poaching::is_clear_upgrade(player_value, weak_value) {
            continue;
        }
        // Normal: fica com o de maior QUALIDADE. Debug (force): o mais RICO, pra o
        // leilão ter fôlego de dar lances de verdade.
        let rank = if force { t.cash_balance } else { q };
        if best.as_ref().is_none_or(|(_, br, _)| rank > *br) {
            best = Some((t.clone(), rank, Some(weak_id)));
        }
    }
    let Some((suitor, _, weak_id)) = best else {
        return Ok(None);
    };

    // Leilão: assediante vs time atual (o status quo é o lance de abertura).
    let suitor_need = crate::fame::team_need_factor(
        crate::finance::planning::derive_budget_index_from_money(&suitor),
        suitor.reputacao,
    );
    let current_need = crate::fame::team_need_factor(
        crate::finance::planning::derive_budget_index_from_money(&current_team),
        current_team.reputacao,
    );
    let poacher_side = crate::market::poaching::AuctionSide {
        team_id: suitor.id.clone(),
        team_quality: team_quality(&suitor),
        bond: crate::market::bond::get_bond(conn, &player.id, &suitor.id)?,
        ceiling: crate::market::poaching::salary_ceiling(
            current.salario_anual,
            crate::market::poaching::poach_target_value(
                player.atributos.skill,
                player.atributos.midia,
                suitor_need,
            ),
            suitor.cash_balance - buyout,
        ),
    };
    let holder_side = crate::market::poaching::AuctionSide {
        team_id: current_team.id.clone(),
        team_quality: current_quality,
        bond: crate::market::bond::get_bond(conn, &player.id, &current_team.id)?,
        ceiling: crate::market::poaching::salary_ceiling(
            current.salario_anual,
            crate::market::poaching::poach_target_value(
                player.atributos.skill,
                player.atributos.midia,
                current_need,
            ),
            current_team.cash_balance,
        ),
    };
    let auction = crate::market::poaching::resolve_salary_auction(
        current.salario_anual,
        &poacher_side,
        &holder_side,
    );

    // Sem ao menos um lance do assediante (nem cobriu o status quo) → não há oferta.
    if !auction.bids.iter().any(|b| b.team_id == suitor.id) {
        return Ok(None);
    }
    let poacher_best = auction
        .bids
        .iter()
        .filter(|b| b.team_id == suitor.id)
        .map(|b| b.salary)
        .fold(0.0_f64, f64::max);
    let holder_best = auction
        .bids
        .iter()
        .filter(|b| b.team_id == current_team.id)
        .map(|b| b.salary)
        .fold(current.salario_anual, f64::max);
    // DEBUG: se o time atual não teria como brigar (pobre), finge uma cobertura
    // competitiva SÓ pra dar pra ver a disputa de 4 turnos na tela. Nunca no jogo real.
    let holder_best = if force && holder_best <= current.salario_anual + 1.0 {
        (current.salario_anual + 0.7 * (poacher_best - current.salario_anual)).round()
    } else {
        holder_best
    };

    let real_bids: Vec<PoachBid> = auction
        .bids
        .iter()
        .enumerate()
        .map(|(i, b)| PoachBid {
            team_name: if b.team_id == suitor.id {
                suitor.nome.clone()
            } else {
                current_team.nome.clone()
            },
            is_poacher: b.team_id == suitor.id,
            salary: b.salary,
            label: bid_label(i),
        })
        .collect();
    // Garante uma disputa com fôlego pra tela do jogador: quando os DOIS lados
    // sobem de verdade, mostra ao menos 4 turnos (a economia real — poacher_best/
    // holder_best/vencedor — não muda; isto é só a dramatização dos lances).
    let bids = build_player_display_bids(
        &real_bids,
        &suitor.nome,
        &current_team.nome,
        current.salario_anual,
        poacher_best,
        holder_best,
    );

    Ok(Some(PlayerPoachOffer {
        current_contract_id: current.id.clone(),
        current_team_id: current_team.id.clone(),
        suitor_team_id: suitor.id.clone(),
        buyout,
        current_salary: current.salario_anual,
        poacher_best,
        holder_best,
        suitor_name: suitor.nome.clone(),
        suitor_color: suitor.cor_primaria.clone(),
        suitor_car_rating: suitor.car_strength()
            .round()
            .clamp(0.0, 100.0) as u8,
        current_team_name: current_team.nome.clone(),
        current_team_color: current_team.cor_primaria.clone(),
        category_label: crate::constants::categories::get_category_config(&suitor.categoria)
            .map(|c| c.nome_curto.to_string())
            .unwrap_or_else(|| suitor.categoria.clone()),
        incumbent_name: weak_id
            .as_deref()
            .and_then(|id| drivers_by_id.get(id))
            .map(|d| d.nome.clone()),
        player_fama: player.atributos.midia.round().clamp(0.0, 100.0) as u8,
        bids,
        poacher_wins: auction.poacher_wins,
    }))
}

/// Aplica a decisão do jogador sobre a quebra de contrato. `accept = true` → SAIR
/// (assina no pretendente pelo melhor lance dele, a multa vai do pretendente ao time
/// atual, o mais fraco do pretendente é dispensado limpo). `accept = false` → FICAR
/// (se o time cobriu acima do salário atual, o aumento fica no contrato). Revalida o
/// contrato que o jogador VIU — se mudou (a escada mexeu), a oferta expirou.
pub(crate) fn resolve_player_poach(
    conn: &Connection,
    offer: &PlayerPoachOffer,
    accept: bool,
    new_season_number: i32,
) -> Result<PlayerPoachOutcome, String> {
    let player = driver_queries::get_player_driver(conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let current = contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao checar contrato do jogador: {e}"))?;
    if current.as_ref().is_none_or(|c| c.id != offer.current_contract_id) {
        return Ok(PlayerPoachOutcome {
            applied: false,
            left: false,
            salary: offer.current_salary,
            team_name: offer.current_team_name.clone(),
            note: rust_i18n::t!("market.poach_outcome.expired").to_string(),
        });
    }
    let current = current.unwrap();

    if !accept {
        // FICAR: se o time cobriu acima do atual, o aumento fica no contrato.
        if offer.holder_best > current.salario_anual {
            contract_queries::update_contract_salary(conn, &current.id, offer.holder_best)
                .map_err(|e| format!("Falha ao aplicar aumento de retencao do jogador: {e}"))?;
        }
        return Ok(PlayerPoachOutcome {
            applied: true,
            left: false,
            salary: offer.holder_best.max(current.salario_anual),
            team_name: offer.current_team_name.clone(),
            note: rust_i18n::t!("market.poach_outcome.stayed", team = offer.current_team_name.as_str())
                .to_string(),
        });
    }

    // SAIR: quebra de contrato de verdade.
    let Some(suitor) = team_queries::get_team_by_id(conn, &offer.suitor_team_id)
        .map_err(|e| format!("Falha ao carregar pretendente: {e}"))?
    else {
        return Ok(PlayerPoachOutcome {
            applied: false,
            left: false,
            salary: current.salario_anual,
            team_name: offer.current_team_name.clone(),
            note: rust_i18n::t!("market.poach_outcome.unavailable").to_string(),
        });
    };

    contract_queries::update_contract_status(conn, &current.id, &ContractStatus::Rescindido)
        .map_err(|e| format!("Falha ao rescindir contrato do jogador: {e}"))?;

    // Assento no pretendente: usa vaga aberta se houver; senão desloca o mais fraco
    // (recomputado AGORA, não o do momento da oferta — a escada pode ter mexido).
    let (papel, displaced_id): (TeamRole, Option<String>) =
        if suitor.piloto_1_id.is_none() {
            (TeamRole::Numero1, None)
        } else if suitor.piloto_2_id.is_none() {
            (TeamRole::Numero2, None)
        } else {
            let a = suitor.piloto_1_id.clone().unwrap();
            let b = suitor.piloto_2_id.clone().unwrap();
            let need = crate::fame::team_need_factor(
                crate::finance::planning::derive_budget_index_from_money(&suitor),
                suitor.reputacao,
            );
            let value_of = |id: &str| {
                driver_queries::get_driver(conn, id)
                    .ok()
                    .map(|d| driver_poach_value(&d, need))
                    .unwrap_or(f64::MAX)
            };
            if value_of(&a) <= value_of(&b) {
                (TeamRole::Numero1, Some(a))
            } else {
                (TeamRole::Numero2, Some(b))
            }
        };

    if let Some(did) = displaced_id.as_ref() {
        if let Some(dc) = contract_queries::get_active_regular_contract_for_pilot(conn, did)
            .map_err(|e| format!("Falha ao carregar contrato do deslocado: {e}"))?
        {
            contract_queries::update_contract_status(conn, &dc.id, &ContractStatus::Rescindido)
                .map_err(|e| format!("Falha ao rescindir deslocado: {e}"))?;
        }
        if let Ok(mut d) = driver_queries::get_driver(conn, did) {
            d.categoria_atual = None; // agente livre LIMPO (a escada o repesca)
            driver_queries::update_driver(conn, &d)
                .map_err(|e| format!("Falha ao liberar deslocado: {e}"))?;
        }
    }

    let mut contract = Contract::new(
        next_id(conn, IdType::Contract)
            .map_err(|e| format!("Falha ao gerar ID de contrato do jogador: {e}"))?,
        player.id.clone(),
        player.nome.clone(),
        suitor.id.clone(),
        suitor.nome.clone(),
        new_season_number,
        2,
        offer.poacher_best,
        papel,
        suitor.categoria.clone(),
    );
    contract.classe = suitor.classe.clone();
    contract_queries::insert_contract(conn, &contract)
        .map_err(|e| format!("Falha ao inserir novo contrato do jogador: {e}"))?;

    let mut moved = player.clone();
    moved.categoria_atual = Some(suitor.categoria.clone());
    driver_queries::update_driver(conn, &moved)
        .map_err(|e| format!("Falha ao mover jogador de equipe: {e}"))?;

    // A multa: pretendente → time atual (o time que perde o jogador é indenizado).
    transfer_between_teams(conn, &suitor.id, &offer.current_team_id, offer.buyout)?;

    let teams = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao recarregar times: {e}"))?;
    let refreshed: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao recarregar pilotos: {e}"))?
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();
    sync_team_slots_from_active_regular_contracts(conn, &teams, &refreshed)?;

    Ok(PlayerPoachOutcome {
        applied: true,
        left: true,
        salary: offer.poacher_best,
        team_name: suitor.nome.clone(),
        note: rust_i18n::t!("market.poach_outcome.signed", team = suitor.nome.as_str()).to_string(),
    })
}

/// Salário da oferta ao jogador, com o prêmio de interesse ativo aplicado quando o
/// time cobiça o nome dele — usado no MESMO ponto pela listagem e pela assinatura,
/// pra o que é mostrado bater com o que é assinado.
fn player_offer_salary_with_interest(
    tier: u8,
    is_n1: bool,
    skill: f64,
    team_id: &str,
    active_interest: bool,
) -> f64 {
    let base = player_offer_salary(tier, is_n1, skill, team_id);
    if active_interest {
        base * crate::fame::ACTIVE_INTEREST_SALARY_PREMIUM
    } else {
        base
    }
}

pub(crate) fn player_market_offers(
    conn: &Connection,
    season: i32,
) -> Result<Vec<crate::market::transfer_window::PlayerOffer>, String> {
    let Ok(player) = driver_queries::get_player_driver(conn) else {
        return Ok(Vec::new());
    };
    if player.status != DriverStatus::Ativo {
        return Ok(Vec::new());
    }
    if contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao checar contrato do jogador: {e}"))?
        .is_some()
    {
        return Ok(Vec::new());
    }

    let player_tier = player_market_tier(conn, &player)?;
    let player_lic = player_effective_license(conn, &player)?;
    // Pódio (1º–3º) na última temporada → habilita ofertas de PROMOÇÃO (tier acima).
    let player_podium = player_last_finish_position(conn, &player.id)
        .is_some_and(|pos| (1..=3).contains(&pos));
    // Interesse ativo (Fase 2a): os poucos MELHORES times que cobiçam o jogador.
    let interest_team_ids: std::collections::HashSet<String> =
        player_active_interest_teams(conn, &player)?
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();

    // Assentos que já têm proposta formal PENDENTE ("Proposta recebida") não aparecem
    // também em "Suas ofertas" — o mesmo assento não deve virar card E oferta passiva.
    let proposal_seats: std::collections::HashSet<String> =
        match get_season_by_number(conn, season)? {
            Some(s) => crate::db::queries::market_proposals::get_pending_player_proposals(
                conn, &s.id, &player.id,
            )
            .map_err(|e| format!("Falha ao carregar propostas pendentes: {e}"))?
            .into_iter()
            .map(|p| format!("{}#{}", p.equipe_id, p.papel.as_str()))
            .collect(),
            None => std::collections::HashSet::new(),
        };

    let mut offers: Vec<crate::market::transfer_window::PlayerOffer> = Vec::new();
    // Um time com as DUAS vagas abertas geraria duas ofertas (N1 e N2). O jogador só
    // deve receber UMA por time — fica com a de titular (N1) quando ambas existem.
    let mut offer_by_team: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for vac in find_vacancies(conn)?.into_iter().filter(is_regular_vacancy) {
        // Assento já ofertado como card formal → não duplica em "Suas ofertas".
        if proposal_seats.contains(&format!("{}#{}", vac.team_id, vac.papel_necessario.as_str())) {
            continue;
        }
        // Piso de RECOMEÇO: vaga de estreia (rookie) é sempre oferecida ao jogador,
        // mesmo fora do tier/licença — assim ele NUNCA fica sem nenhuma proposta.
        let is_debut = is_real_career_debut_category(&vac.categoria);
        // PROMOÇÃO: quem foi pódio recebe ofertas do tier ACIMA. A licença é concedida
        // ao assinar (grant_driver_license_for_division_if_needed), então o portão de
        // licença é dispensado aqui, como no caso de estreia.
        let is_promotion = player_podium && vac.category_tier == player_tier + 1;
        let required = crate::models::license::required_license_for_division(
            &vac.categoria,
            vac.classe.as_deref(),
        )
        .unwrap_or(0);
        if required > player_lic && !is_debut && !is_promotion {
            continue;
        }
        let in_tier = vac.category_tier == player_tier
            || (player_tier > 0 && vac.category_tier == player_tier - 1)
            || is_promotion;
        if !in_tier && !is_debut {
            continue;
        }
        let is_n1 = matches!(vac.papel_necessario, TeamRole::Numero1);
        let active_interest = interest_team_ids.contains(&vac.team_id);
        let salary = player_offer_salary_with_interest(
            vac.category_tier,
            is_n1,
            player.atributos.skill,
            &vac.team_id,
            active_interest,
        );
        let offer = crate::market::transfer_window::PlayerOffer {
            seat_id: format!("{}#{}", vac.team_id, vac.papel_necessario.as_str()),
            team_id: vac.team_id.clone(),
            category: vac.categoria.clone(),
            class: vac.classe.clone(),
            salary,
            is_n1,
            active_interest,
        };
        if let Some(&idx) = offer_by_team.get(&vac.team_id) {
            // Já há oferta deste time: só troca se a nova for titular (N1) e a atual for N2.
            if is_n1 && !offers[idx].is_n1 {
                offers[idx] = offer;
            }
            continue;
        }
        offer_by_team.insert(vac.team_id.clone(), offers.len());
        offers.push(offer);
    }

    // Piso de RECOMEÇO: se nenhuma vaga no tier/tier−1/estreia abriu, mostra as vagas que
    // a escada reservou pro jogador (`player_reserved_seats` usa o fallback "qualquer vaga
    // licenciada"). Sem isso, um agente livre há tempo teria vagas reservadas
    // silenciosamente pela escada e NUNCA as veria ofertadas → zero propostas.
    if offers.is_empty() {
        let held = player_reserved_seats(conn, season)?;
        if !held.is_empty() {
            let vacs = find_vacancies(conn)?;
            for seat_id in held {
                if let Some(vac) = vacs
                    .iter()
                    .find(|v| format!("{}#{}", v.team_id, v.papel_necessario.as_str()) == seat_id)
                {
                    let is_n1 = matches!(vac.papel_necessario, TeamRole::Numero1);
                    let active_interest = interest_team_ids.contains(&vac.team_id);
                    offers.push(crate::market::transfer_window::PlayerOffer {
                        seat_id,
                        team_id: vac.team_id.clone(),
                        category: vac.categoria.clone(),
                        class: vac.classe.clone(),
                        salary: player_offer_salary_with_interest(
                            vac.category_tier,
                            is_n1,
                            player.atributos.skill,
                            &vac.team_id,
                            active_interest,
                        ),
                        is_n1,
                        active_interest,
                    });
                }
            }
        }
    }

    Ok(offers)
}

/// Assina o jogador na vaga `seat_id` ("team#papel"): valida que a vaga existe e é
/// dele (elegível+licenciada), concede a licença e contrata (1 ano). Erro se a vaga
/// sumiu ou não é elegível.
pub(crate) fn sign_player_to_vacancy(
    conn: &Connection,
    season: i32,
    seat_id: &str,
) -> Result<(), String> {
    let player = driver_queries::get_player_driver(conn)
        .map_err(|e| format!("Falha ao carregar piloto do jogador: {e}"))?;
    let vac = find_vacancies(conn)?
        .into_iter()
        .find(|v| format!("{}#{}", v.team_id, v.papel_necessario.as_str()) == seat_id)
        .ok_or_else(|| format!("Vaga '{seat_id}' nao encontrada."))?;

    // A vaga deve ser do jogador: licenciada (ou de estreia). Não exigimos in_tier aqui
    // porque `player_market_offers` pode ofertar a vaga reservada fora do tier (piso de
    // recomeço, "qualquer vaga licenciada") — o sign precisa aceitar tudo que é ofertado.
    let player_lic = player_effective_license(conn, &player)?;
    // Vaga de estreia (rookie) é sempre aceitável (piso de recomeço), mesmo sem licença.
    let is_debut = is_real_career_debut_category(&vac.categoria);
    // Promoção do pódio (tier acima): também aceitável — a licença é concedida logo
    // abaixo. Espelha a elegibilidade de `player_market_offers`.
    let player_tier = player_market_tier(conn, &player)?;
    let is_promotion = vac.category_tier == player_tier + 1
        && player_last_finish_position(conn, &player.id).is_some_and(|pos| (1..=3).contains(&pos));
    let required = crate::models::license::required_license_for_division(
        &vac.categoria,
        vac.classe.as_deref(),
    )
    .unwrap_or(0);
    if !is_debut && !is_promotion && required > player_lic {
        return Err(format!("Vaga '{seat_id}' nao esta disponivel para o jogador."));
    }

    grant_driver_license_for_division_if_needed(
        conn,
        &player.id,
        &vac.categoria,
        vac.classe.as_deref(),
    )?;
    let is_n1 = matches!(vac.papel_necessario, TeamRole::Numero1);
    // Ideia 4: a duração honra o Foco do time + o Vínculo do jogador com ele — um
    // time-casa/leal assina o jogador num contrato de projeto plurianual (o mesmo
    // número mostrado na oferta). Falha na leitura → 1 ano (neutro).
    let vinculo = crate::market::bond::get_bond(conn, &player.id, &vac.team_id).unwrap_or(0.0);
    let foco = crate::finance::focus::get_focus(conn, &vac.team_id)
        .map(|(f, _)| f)
        .unwrap_or(crate::finance::focus::TeamFocus::MeioDeGrid);
    let duration = crate::market::renewal::player_offer_duration(foco, vinculo);
    // O salário assinado honra o prêmio de interesse ativo (o mesmo mostrado na oferta).
    let active_interest = player_active_interest_teams(conn, &player)?
        .iter()
        .any(|(id, _, _)| id == &vac.team_id);
    sign_driver_to_team(
        conn,
        &player,
        &vac,
        season,
        player_offer_salary_with_interest(
            vac.category_tier,
            is_n1,
            player.atributos.skill,
            &vac.team_id,
            active_interest,
        ),
        duration,
        vac.papel_necessario.clone(),
    )
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
    let skill_of = |id: &str| {
        drivers_by_id
            .get(id)
            .map(|d| d.atributos.skill)
            .unwrap_or(0.0)
    };

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
        if weak_ctx.total_pilotos < 2 || weak_ctx.posicao_campeonato < weak_ctx.total_pilotos - 1 {
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

/// (Flag `IRACER_ROOKIE_MERIT`) Garante a subida do CAMPEÃO de cada categoria de
/// estreia (Rookie) para a categoria-alvo (Amador).
///
/// O fluxo normal já promove o melhor feeder quando o Amador tem vaga natural; esta
/// passada cobre o caso em que o Amador está CHEIO: força a troca do 1º do Rookie
/// com o pior do Amador, reusando a mesma máquina de `swap_contract_seats` do
/// rebaixamento por mérito (campeão sobe, pior desce ao Rookie — exatamente o que o
/// rebaixamento por mérito já faz, só que aqui o gatilho é "campeão" em vez de
/// "pior do Amador terminou em último"). Conservadora: no máximo 1 troca por
/// categoria de estreia, nunca mexe no jogador, e só dispara se o campeão de fato
/// possuir a licença exigida (a metade superior do Rookie a conquista).
fn guarantee_rookie_champion_promotions(
    conn: &Connection,
    teams: &[crate::models::team::Team],
    new_season_number: i32,
    contexts: &HashMap<String, DriverMarketContext>,
    report: &mut MarketReport,
) -> Result<(), String> {
    if !rookie_merit_enabled() {
        return Ok(());
    }

    let debut_year = get_season_by_number(conn, new_season_number)?
        .map(|season| season.ano)
        .unwrap_or_else(|| Local::now().year());

    let player_team_ids: HashSet<&str> = teams
        .iter()
        .filter(|team| team.is_player_team)
        .map(|team| team.id.as_str())
        .collect();

    let rookie_cats: Vec<&'static str> = get_all_categories()
        .iter()
        .filter(|category| {
            is_real_career_debut_category(category.id)
                && is_category_active_in_year(category.id, debut_year)
        })
        .map(|category| category.id)
        .collect();

    let mut swapped_any = false;
    for rookie_cat in rookie_cats {
        // Alvo regular ativo (Amador) para onde o Rookie alimenta.
        let Some(target_cat) = get_target_categories(rookie_cat).into_iter().find(|target| {
            uses_regular_contracts(target)
                && !runs_in_special_phase(target)
                && is_category_active_in_year(target, debut_year)
        }) else {
            continue;
        };
        let Some(required) = required_license_for_division(target_cat, None) else {
            continue;
        };

        // Vaga natural no Amador → o fluxo normal (escada) já promove o melhor
        // feeder (o campeão). Nada a forçar.
        let target_has_vacancy = find_vacancies(conn)?
            .into_iter()
            .any(|vacancy| vacancy.categoria == target_cat && is_regular_vacancy(&vacancy));
        if target_has_vacancy {
            continue;
        }

        // Recarrega o estado a cada categoria (a troca anterior mexeu nos contratos).
        let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao carregar pilotos (promo campeao rookie): {e}"))?
            .into_iter()
            .map(|driver| (driver.id.clone(), driver))
            .collect();
        let license_levels = load_max_license_levels(conn)?;
        let is_active_non_player = |id: &str| {
            drivers_by_id
                .get(id)
                .is_some_and(|driver| !driver.is_jogador && driver.status == DriverStatus::Ativo)
        };
        let position_of = |id: &str| contexts.get(id).map(|c| c.posicao_campeonato).unwrap_or(99);
        let skill_of = |id: &str| {
            drivers_by_id
                .get(id)
                .map(|driver| driver.atributos.skill)
                .unwrap_or(0.0)
        };

        let active = contract_queries::get_all_active_regular_contracts(conn)
            .map_err(|e| format!("Falha ao carregar contratos (promo campeao rookie): {e}"))?;

        // Campeão do Rookie: 1º colocado, ativo, fora do time do jogador, COM a
        // licença exigida pelo Amador.
        let Some(champion) = active
            .iter()
            .filter(|c| {
                c.categoria == rookie_cat
                    && c.classe.is_none()
                    && is_active_non_player(&c.piloto_id)
                    && !player_team_ids.contains(c.equipe_id.as_str())
                    && license_levels
                        .get(&c.piloto_id)
                        .is_some_and(|&owned| owned >= required)
            })
            .min_by_key(|c| position_of(&c.piloto_id))
        else {
            continue;
        };
        if position_of(&champion.piloto_id) != 1 {
            continue; // só o 1º é garantido
        }

        // Pior piloto do Amador (pior posição, depois menor skill), fora do jogador.
        let Some(weakest) = active
            .iter()
            .filter(|c| {
                c.categoria == target_cat
                    && c.classe.is_none()
                    && is_active_non_player(&c.piloto_id)
                    && !player_team_ids.contains(c.equipe_id.as_str())
            })
            .max_by(|a, b| {
                position_of(&a.piloto_id)
                    .cmp(&position_of(&b.piloto_id))
                    .then_with(|| skill_of(&b.piloto_id).total_cmp(&skill_of(&a.piloto_id)))
            })
        else {
            continue;
        };

        swap_contract_seats(conn, champion, weakest, new_season_number, report)?;
        swapped_any = true;
    }

    if swapped_any {
        let refreshed: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao recarregar pilotos (pos promo campeao rookie): {e}"))?
            .into_iter()
            .map(|driver| (driver.id.clone(), driver))
            .collect();
        sync_team_slots_from_active_regular_contracts(conn, teams, &refreshed)?;
    }
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
    move_driver(
        conn,
        &riser.piloto_id,
        &riser.piloto_nome,
        weak,
        "promocao_merito",
    )?;
    move_driver(
        conn,
        &weak.piloto_id,
        &weak.piloto_nome,
        riser,
        "rebaixamento",
    )?;
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
        .max_by(|a, b| {
            let score = |driver: &Driver| {
                let pos = contexts
                    .get(&driver.id)
                    .map(|context| context.posicao_campeonato)
                    .unwrap_or(99);
                feeder_promotion_score(driver.atributos.skill, pos)
            };
            score(a).total_cmp(&score(b))
        })
        .cloned()
}

/// Score de promoção em cascata: o TALENTO (skill) manda, com um empurrão
/// decrescente pelo desempenho na temporada. Antes a promoção ordenava SÓ por
/// `posicao_campeonato` (skill só desempatava), então o GT3 recebia os CAMPEÕES do
/// GT4 em vez dos mais HABILIDOSOS — um craque skill-80 em carro fraco (8º) perdia a
/// vaga para o campeão skill-60, e o topo deflacionava temporada após temporada. Com
/// o empurrão de campeonato preservamos o mérito da temporada (o campeão sobe na
/// frente de talentos até ~9 pts acima), sem enterrar o talento. 1º=+7.2, 5º=+4, 10º+=0.
fn feeder_promotion_score(skill: f64, posicao_campeonato: i32) -> f64 {
    let championship_bonus = (10 - posicao_campeonato.clamp(1, 10)) as f64 * 0.8;
    skill + championship_bonus
}

/// Recrutamento profundo por DEMANDA DE TIME + ACEITE DO PILOTO. Liga os dois
/// cérebros prontos (`slam_ambition` e `driver_ai::evaluate_proposal`) na escada viva.
///
/// Para uma vaga de topo REGULAR (gt3) que o feeder imediato (só gt4) deixaria mal
/// servida, o time escaneia TODAS as categorias inferiores atrás do craque preso lá
/// (a elite não-ambiciosa que a cascata míope nunca alcança), prioriza quem AMBICIONA
/// a categoria (slam) e lhe faz proposta; o piloto decide via `evaluate_proposal`
/// (agência: Leal/Consolidador/oferta ruim recusam e ficam). Devolve o primeiro que
/// ACEITAR, ou `None` (cai na escada normal).
///
/// Gate anti-churn: só dispara para gt3 (tier ≥ 4 e NÃO fase especial — endurance e
/// production têm convocação própria e o feeder do endurance é só-gt3 de propósito) e
/// só quando o feeder não entrega o nível-alvo da categoria. A escada saudável do
/// miolo fica intacta.
#[allow(clippy::too_many_arguments)]
fn deep_recruitment_candidate(
    conn: &Connection,
    vacancy: &Vacancy,
    drivers_by_id: &HashMap<String, Driver>,
    contexts: &HashMap<String, DriverMarketContext>,
    license_levels: &HashMap<String, u8>,
    required_license: Option<u8>,
    feeder_best_skill: Option<f64>,
    rng: &mut impl Rng,
) -> Result<Option<Driver>, String> {
    if vacancy.category_tier < 4 || runs_in_special_phase(&vacancy.categoria) {
        return Ok(None);
    }
    let target = skill_ranges::get_skill_range_by_tier(vacancy.category_tier.min(4))
        .map(|range| range.skill_media as f64)
        .unwrap_or(78.0);
    // Feeder já entrega o nível do topo → escada normal, sem escanear fundo.
    if feeder_best_skill.is_some_and(|skill| skill >= target) {
        return Ok(None);
    }

    let candidates: Vec<&Driver> = drivers_by_id
        .values()
        .filter(|driver| {
            !driver.is_jogador
                && driver.status == DriverStatus::Ativo
                // Só puxa craque de verdade (nível do topo) E só se for UPGRADE sobre
                // o melhor do feeder — nada de shuffle lateral.
                && driver.atributos.skill >= target
                && feeder_best_skill.map_or(true, |skill| driver.atributos.skill > skill)
                // Sentado numa categoria de tier ABAIXO da vaga (a "várzea").
                && driver.categoria_atual.as_deref().is_some_and(|cat| {
                    get_category_config(cat)
                        .is_some_and(|config| config.tier < vacancy.category_tier)
                })
                // Mérito de licença: idêntico ao da promoção regular (nada de conceder
                // na hora); barra naturalmente puxar rookie cru sem a licença do topo.
                && match required_license {
                    Some(level) => {
                        license_levels.get(&driver.id).is_some_and(|&owned| owned >= level)
                    }
                    None => true,
                }
        })
        .collect();
    if candidates.is_empty() {
        return Ok(None);
    }

    // Ranqueia UMA vez (slam consulta o archive por piloto — não repetir no comparador):
    // ambicioso que QUER esta categoria primeiro, depois pelo score de promoção.
    let mut ranked: Vec<(&Driver, bool, f64)> = candidates
        .into_iter()
        .map(|driver| {
            let wants_this = slam_target_category(conn, driver)
                .ok()
                .flatten()
                .is_some_and(|(category, _)| category == vacancy.categoria);
            let pos = contexts
                .get(&driver.id)
                .map(|context| context.posicao_campeonato)
                .unwrap_or(99);
            (driver, wants_this, feeder_promotion_score(driver.atributos.skill, pos))
        })
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.total_cmp(&a.2)));

    // O time faz proposta ao melhor; o piloto decide. Primeiro que aceita, ganha a
    // vaga; quem recusa (Leal/Consolidador/oferta ruim/quer ser N1) fica onde está.
    let contracts = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao carregar contratos p/ recrutamento profundo: {e}"))?;
    for (candidate, _, _) in ranked {
        let current_contract = contracts
            .iter()
            .find(|contract| contract.piloto_id == candidate.id);
        let current_tier = candidate
            .categoria_atual
            .as_deref()
            .and_then(get_category_config)
            .map(|config| config.tier)
            .unwrap_or(0);
        let proposal = MarketProposal {
            id: format!("deep-{}-{}", vacancy.team_id, candidate.id),
            equipe_id: vacancy.team_id.clone(),
            equipe_nome: vacancy.team_name.clone(),
            piloto_id: candidate.id.clone(),
            piloto_nome: candidate.nome.clone(),
            categoria: vacancy.categoria.clone(),
            papel: vacancy.papel_necessario.clone(),
            salario_oferecido: calculate_offer_salary(vacancy, candidate, rng),
            duracao_anos: 1,
            status: ProposalStatus::Pendente,
            motivo_recusa: None,
        };
        if evaluate_proposal(
            candidate,
            &proposal,
            current_contract,
            current_tier,
            vacancy.category_tier,
            vacancy.car_strength,
            vacancy.reputacao,
            rng,
        )
        .accepted
        {
            return Ok(Some(candidate.clone()));
        }
    }
    Ok(None)
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
        calculate_offer_salary(vacancy, &rookie, rng),
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
    need_factor: f64,
    team_ceiling: Option<f64>,
) -> std::cmp::Ordering {
    // Gates duros (experiência/licença) mandam primeiro; o VALOR do time desempata:
    // mérito esportivo (skill) + apelo comercial da fama ponderado pela necessidade
    // do time, MENOS a penalidade de affordability (Item 1). Time carente pode preferir
    // um nome famoso a um rápido anônimo; numa dinastia a fama pesa pouco e a velocidade
    // decide (Fase 2a do estrelato). Time SEM CAIXA desce para um piloto mais barato.
    pool_fallback_candidate_rank(a, vacancy)
        .cmp(&pool_fallback_candidate_rank(b, vacancy))
        .then_with(|| {
            team_candidate_value(a, vacancy, need_factor, team_ceiling)
                .total_cmp(&team_candidate_value(b, vacancy, need_factor, team_ceiling))
        })
}

/// Valor de um candidato para o time: skill + apelo comercial da fama ponderado pela
/// necessidade do time (`fame_commercial_units × need_factor`), MENOS a penalidade de
/// affordability quando o time carrega um teto (`team_ceiling = Some`). `None` (flag off)
/// = comportamento antigo, sem penalidade.
fn team_candidate_value(
    candidate: &AvailableDriver,
    vacancy: &Vacancy,
    need_factor: f64,
    team_ceiling: Option<f64>,
) -> f64 {
    let skill = candidate.driver.atributos.skill;
    let merit_and_appeal =
        skill + crate::fame::fame_commercial_units(candidate.driver.atributos.midia) * need_factor;
    match team_ceiling {
        // Item 1: penaliza o candidato que o assento NÃO PODE PAGAR. O preço de mercado do
        // piloto (por tier+papel do assento) acima do teto salarial da equipe vira uma
        // penalidade em "pontos de skill", empurrando um time sem caixa para um piloto mais
        // barato (ou mantendo o craque caro no pool para um assento que o comporte).
        Some(ceiling) => {
            let price = candidate_market_price(
                skill,
                vacancy.category_tier,
                matches!(vacancy.papel_necessario, TeamRole::Numero1),
            );
            merit_and_appeal - affordability_penalty(price, ceiling)
        }
        None => merit_and_appeal,
    }
}

/// Preço de mercado (independente do caixa do time) que um piloto de `skill` comanda num
/// assento deste `tier`/papel: a faixa salarial do tier posicionada pela skill, com fator
/// de papel (N1 titular custa mais). Mesma escala das ofertas ao jogador
/// (`player_offer_salary`) e dos contratos da IA (`salary_range_for_tier`) — não depende
/// da pobreza da equipe, senão o teto baixo de um time quebrado tornaria todo mundo
/// "barato" e a penalidade nunca dispararia.
fn candidate_market_price(skill: f64, tier: u8, is_n1: bool) -> f64 {
    let (lo, hi) = crate::models::contract::salary_range_for_tier(tier);
    let t = (skill / 100.0).clamp(0.0, 1.0);
    let base = lo + (hi - lo) * t;
    let role = if is_n1 { 1.30 } else { 1.06 };
    base * role
}

/// Peso e teto da penalidade de affordability (em "pontos de skill", mesma unidade de
/// `team_candidate_value`, cujo base ≈ skill 0–100 + fama 0–63). A penalidade só ENTRA
/// como desempate DEPOIS dos gates duros (via `.then_with` em
/// `compare_pool_fallback_candidates`), então pode ser forte sem inverter licença/experiência
/// nem desestabilizar o sim (é comparador puro, sem re-scan). O `WEIGHT` alto garante que
/// "não posso pagar" supere uma diferença de skill relevante — um assento sobre orçamento
/// perde para um candidato que CABE, mesmo sendo este menos habilidoso. O `CAP` satura para
/// que, quando NINGUÉM cabe (time quebrado), todos fiquem igualmente penalizados e a skill
/// volte a decidir (ele assina o melhor disponível em vez de afundar num skill-20).
const AFFORDABILITY_PENALTY_WEIGHT: f64 = 200.0;
const AFFORDABILITY_PENALTY_CAP: f64 = 120.0;

/// Penalidade de affordability: 0 se o assento comporta o preço; senão cresce com o quanto
/// o preço excede o teto, saturando em `AFFORDABILITY_PENALTY_CAP`.
fn affordability_penalty(price: f64, ceiling: f64) -> f64 {
    if ceiling <= 0.0 || price <= ceiling {
        return 0.0;
    }
    (AFFORDABILITY_PENALTY_WEIGHT * (price / ceiling - 1.0)).min(AFFORDABILITY_PENALTY_CAP)
}

/// Desejabilidade de um assento para a ORDEM de escolha (port de
/// `transfer_window::driver_offer_score`): carro + prestígio (reputação da equipe, já na
/// vaga). Os pesos vêm da FONTE ÚNICA `transfer_window::{SEAT_W_CAR, SEAT_W_PRESTIGE}` — os
/// mesmos do motor de janela —, então não divergem. Assim o melhor carro numa equipe
/// prestigiada escolhe do pool antes de um carro igual sem tradição — o que o leilão dava de
/// graça, reproduzido na escada gulosa.
fn seat_desirability(vacancy: &Vacancy) -> f64 {
    use crate::market::transfer_window::{SEAT_W_CAR, SEAT_W_PRESTIGE};
    let car_norm = vacancy.car_strength;
    (car_norm / 100.0).min(1.2) * SEAT_W_CAR
        + (vacancy.reputacao.clamp(0.0, 100.0) / 100.0) * SEAT_W_PRESTIGE
}

/// Margem do piso de skill do pool de resgate: um órfão só preenche uma vaga
/// NÃO-estreia se o skill dele estiver, no máximo, esta distância ABAIXO da média
/// típica do tier da categoria. Sem isto, um lanterna (skill ~28) era resgatado
/// direto para GT3/Endurance só por estar sem categoria no momento. (Item B.)
///
/// Usamos SKILL (sinal confiável) e não o tier ancorado: um órfão sem histórico de
/// contrato ancora em tier 0 mesmo com skill alto, o que bloquearia resgates
/// legítimos (ex.: um skill-65 para a Production).
const POOL_FALLBACK_SKILL_MARGIN: f64 = 20.0;

/// Piso de skill exigido do órfão para uma vaga do tier dado (média do tier − margem).
fn pool_fallback_skill_floor(vacancy_tier: u8) -> f64 {
    let media = crate::constants::skill_ranges::get_skill_range_by_tier(vacancy_tier.min(4))
        .map(|range| range.skill_media as f64)
        .unwrap_or(60.0);
    (media - POOL_FALLBACK_SKILL_MARGIN).max(0.0)
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

    // Vaga NÃO-estreia: precisa ser órfão (sem categoria atual) E ter skill
    // compatível com o nível da categoria (piso = média do tier − margem). Item B.
    candidate.driver.categoria_atual.is_none()
        && candidate.driver.atributos.skill >= pool_fallback_skill_floor(vacancy.category_tier)
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
#[path = "pipeline/tests/mod.rs"]
mod tests;
