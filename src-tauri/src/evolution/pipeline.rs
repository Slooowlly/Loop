use std::collections::{HashMap, HashSet};
use std::path::Path;

use rand::{rngs::StdRng, Rng, SeedableRng};
use rusqlite::Connection;

use crate::constants::categories::get_category_config;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::evolution::context::StandingEntry;
use crate::evolution::decline::apply_age_decline;
use crate::evolution::growth::{calculate_growth, GrowthReport};
use crate::evolution::licenses::persist_licenses;
use crate::evolution::motivation::{
    adjust_end_of_season_motivation, MotivationContext, MotivationReport,
};
use crate::evolution::retirement::{check_retirement, process_retirement};
use crate::evolution::season_transition::{
    archive_driver_season, create_next_season_9d, reset_driver_season_stats,
    reset_team_season_stats, update_meta_for_new_season,
};
use crate::evolution::standings::build_and_persist_standings;
use crate::finance::prize::constructor_prize;
use crate::finance::rescue::apply_team_sale;
use crate::finance::state::refresh_team_financial_state;
use crate::market::preseason::{advance_week, initialize_preseason, save_preseason_plan};
use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::enums::{DriverStatus, SeasonPhase};
use crate::models::season::Season;
use crate::models::team::Team;
use crate::promotion::pipeline::run_promotion_relegation_for_year;
use crate::rivalry::apply_season_end_rivalry_decay;
use crate::world::team_archive::archive_team_season;

// Reexports para compatibilidade — callsites externos usam crate::evolution::pipeline::*
pub use crate::evolution::context::{EndOfSeasonResult, RetirementInfo, RookieInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndOfSeasonMode {
    Playable,
    HistoricalDraft,
}

pub fn run_end_of_season(
    conn: &mut Connection,
    season: &Season,
    save_path: &Path,
) -> Result<EndOfSeasonResult, String> {
    run_end_of_season_with_mode(conn, season, save_path, EndOfSeasonMode::Playable)
}

pub(crate) fn run_historical_end_of_season(
    conn: &mut Connection,
    season: &Season,
    save_path: &Path,
) -> Result<EndOfSeasonResult, String> {
    run_end_of_season_with_mode(conn, season, save_path, EndOfSeasonMode::HistoricalDraft)
}

fn run_end_of_season_with_mode(
    conn: &mut Connection,
    season: &Season,
    save_path: &Path,
    mode: EndOfSeasonMode,
) -> Result<EndOfSeasonResult, String> {
    let mut rng = StdRng::seed_from_u64(((season.numero as u64) << 32) | season.ano as u64);
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("Falha ao iniciar transacao de fim de temporada: {e}"))?;

    let (teams_by_id, contracts_by_driver) = build_context(&tx)?;

    let standings = build_and_persist_standings(&tx, season, &contracts_by_driver)?;
    // Pilotos de lmp2 aparecem em dois standings (lmp2 regular + classe lmp2 da
    // Endurance); o campeonato regular é o de referência para evolução/arquivo.
    let mut standings_by_driver: HashMap<String, StandingEntry> = HashMap::new();
    for entry in standings
        .iter()
        .filter(|entry| crate::constants::categories::is_multiclass_category(&entry.category))
    {
        standings_by_driver.insert(entry.driver_id.clone(), entry.clone());
    }
    for entry in standings
        .iter()
        .filter(|entry| !crate::constants::categories::is_multiclass_category(&entry.category))
    {
        standings_by_driver.insert(entry.driver_id.clone(), entry.clone());
    }

    let licenses_earned = persist_licenses(&tx, &standings, &standings_by_driver)
        .map_err(|e| format!("Falha ao persistir licencas: {e}"))?;

    season_queries::finalize_season(&tx, &season.id)
        .map_err(|e| format!("Falha ao finalizar temporada: {e}"))?;

    // Títulos contam por campeonato vencido: nas categorias especiais cada classe
    // (mazda/toyota/bmw, gt4/gt3/lmp2) tem o próprio campeão — não há título geral.
    let mut titles_by_driver: HashMap<String, i32> = HashMap::new();
    for entry in standings.iter().filter(|entry| entry.position == 1) {
        *titles_by_driver.entry(entry.driver_id.clone()).or_insert(0) += 1;
    }

    let (growth_reports, motivation_reports, retirements, _existing_names) =
        process_driver_evolution(
            &tx,
            season,
            &standings_by_driver,
            &titles_by_driver,
            &contracts_by_driver,
            &teams_by_id,
            &mut rng,
        )?;

    archive_driver_season(&tx, season, &standings_by_driver)
        .map_err(|e| format!("Falha ao arquivar temporada dos pilotos: {e}"))?;
    archive_team_season(&tx, season)
        .map_err(|e| format!("Falha ao arquivar temporada das equipes: {e}"))?;

    // Prêmio de fim de temporada por posição no campeonato de construtores.
    // Creditado após o arquivamento (que define posicao_campeonato) e antes da
    // promoção/rebaixamento, para que a equipe receba referente à categoria em
    // que de fato competiu nesta temporada.
    award_constructor_prizes(&tx, season)
        .map_err(|e| format!("Falha ao pagar prêmios de construtores: {e}"))?;

    // Ciclo de colapso → venda: equipes que fecham a temporada em colapso têm o
    // contador incrementado; ao chegar à 2ª temporada consecutiva em colapso (a
    // 2ª já em all-in), a equipe é vendida e renovada por uma nova diretoria.
    process_collapse_lifecycle(&tx, season, &mut rng)
        .map_err(|e| format!("Falha no ciclo de colapso/venda de equipes: {e}"))?;

    // Modelo fechado: nada de pré-geração de rookies aqui (era fonte de órfãos —
    // os excedentes não contratados viravam agentes livres eternos). Rookies nascem
    // sob demanda no mercado/cascata quando abre uma vaga de categoria de estreia.
    let rookies_generated: Vec<RookieInfo> = Vec::new();

    let promotion_result =
        run_promotion_relegation_for_year(&tx, season.numero, season.ano, &mut rng)
            .map_err(|e| format!("Erro na promocao/rebaixamento: {e}"))?;

    apply_season_end_rivalry_decay(&tx, season.numero)
        .map_err(|e| format!("Erro no decaimento de rivalidades: {e}"))?;

    let new_season = create_next_season_phase(&tx, season, &mut rng, mode)?;

    let (preseason_initialized, preseason_total_weeks) =
        initialize_preseason_phase(&tx, &new_season, save_path, &mut rng, mode)?;

    tx.commit().map_err(|e| {
        let _ = std::fs::remove_file(save_path.join("preseason_plan.json"));
        format!("Falha ao confirmar fim de temporada: {e}")
    })?;

    Ok(EndOfSeasonResult {
        growth_reports,
        motivation_reports,
        retirements,
        rookies_generated,
        new_season_id: new_season.id,
        new_year: new_season.ano,
        licenses_earned,
        promotion_result,
        preseason_initialized,
        preseason_total_weeks,
    })
}

/// Credita o prêmio de fim de temporada do campeonato de construtores no caixa
/// de cada equipe, a partir das posições recém-arquivadas em
/// `team_season_archive`, e atualiza o estado financeiro resultante.
fn award_constructor_prizes(conn: &Connection, season: &Season) -> Result<(), String> {
    // (team_id, categoria, posição final, nº de equipes no grupo de campeonato).
    // O tamanho do grid é por grupo (categoria + classe), batendo com o
    // agrupamento usado em archive_team_season para categorias multi-classe.
    let mut stmt = conn
        .prepare(
            "SELECT team_id, categoria, posicao_campeonato,
                    COUNT(*) OVER (PARTITION BY categoria, COALESCE(classe, '')) AS grid_size
             FROM team_season_archive
             WHERE season_number = ?1 AND posicao_campeonato IS NOT NULL",
        )
        .map_err(|e| format!("Falha ao preparar consulta de prêmios: {e}"))?;
    let rows = stmt
        .query_map([season.numero], |row| {
            let team_id: String = row.get(0)?;
            let categoria: String = row.get(1)?;
            let position: i32 = row.get(2)?;
            let grid_size: i32 = row.get(3)?;
            Ok((team_id, categoria, position, grid_size))
        })
        .map_err(|e| format!("Falha ao consultar prêmios de construtores: {e}"))?;

    let mut awards: Vec<(String, f64)> = Vec::new();
    for row in rows {
        let (team_id, categoria, position, grid_size) =
            row.map_err(|e| format!("Falha ao mapear prêmio de construtores: {e}"))?;
        let prize = constructor_prize(&categoria, position, grid_size);
        if prize > 0.0 {
            awards.push((team_id, prize));
        }
    }
    drop(stmt);

    for (team_id, prize) in awards {
        let mut team = match team_queries::get_team_by_id(conn, &team_id) {
            Ok(Some(team)) => team,
            Ok(None) => continue,
            Err(e) => return Err(format!("Falha ao carregar equipe {team_id}: {e}")),
        };
        team.cash_balance += prize;
        refresh_team_financial_state(&mut team);
        team_queries::update_team_finance_snapshot(conn, &team)
            .map_err(|e| format!("Falha ao creditar prêmio à equipe {team_id}: {e}"))?;
    }

    Ok(())
}

/// Processa o ciclo de colapso financeiro das equipes no fim da temporada:
///   • Em colapso pela 1ª vez (streak 0→1): apenas registra (aviso). A próxima
///     temporada será forçada a all-in (ver preseason::choose).
///   • Em colapso pela 2ª vez seguida (streak →2): a equipe é VENDIDA — nova
///     diretoria quita a dívida, injeta caixa e re-sorteia atributos. Identidade
///     e histórico preservados. Contador zerado.
///   • Fora do colapso: contador zerado (recuperou-se).
fn process_collapse_lifecycle(
    conn: &Connection,
    season: &Season,
    rng: &mut impl Rng,
) -> Result<(), String> {
    let teams = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao buscar equipes para ciclo de colapso: {e}"))?;

    for mut team in teams {
        if !team.ativa {
            continue;
        }
        let streak = team_queries::get_collapse_streak(conn, &team.id)
            .map_err(|e| format!("Falha ao ler streak de colapso: {e}"))?;

        if team.financial_state == "collapse" {
            let new_streak = streak + 1;
            if new_streak >= 2 {
                // 2ª temporada consecutiva em colapso (a 2ª já em all-in): venda.
                let outcome = apply_team_sale(&mut team, rng);
                team_queries::update_team(conn, &team)
                    .map_err(|e| format!("Falha ao renovar equipe vendida: {e}"))?;
                team_queries::set_collapse_streak(conn, &team.id, 0)
                    .map_err(|e| format!("Falha ao zerar streak pós-venda: {e}"))?;
                let _ = team_queries::incr_rescue_counter(conn, "sold");
                // Registra o evento de venda/nova diretoria para a ficha da equipe.
                let _ = team_queries::insert_team_ownership_event(
                    conn,
                    &team.id,
                    season.numero,
                    season.ano,
                    "sale",
                    outcome.debt_cleared,
                    outcome.cash_injected,
                    "Nova diretoria assume após colapso financeiro crônico.",
                );
            } else {
                // 1ª temporada em colapso: aviso; all-in virá na próxima.
                team_queries::set_collapse_streak(conn, &team.id, new_streak)
                    .map_err(|e| format!("Falha ao gravar streak de colapso: {e}"))?;
            }
        } else if streak != 0 {
            // Tinha aviso (streak >= 1) e fechou a temporada FORA do colapso:
            // salvou-se sozinha no ano de all-in, sem precisar de venda.
            let _ = team_queries::incr_rescue_counter(conn, "self_rescued");
            team_queries::set_collapse_streak(conn, &team.id, 0)
                .map_err(|e| format!("Falha ao zerar streak de colapso: {e}"))?;
        }
    }

    Ok(())
}

fn build_context(
    conn: &Connection,
) -> Result<(HashMap<String, Team>, HashMap<String, Contract>), String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao buscar equipes: {e}"))?;
    let teams_by_id: HashMap<String, Team> = teams
        .into_iter()
        .map(|team| (team.id.clone(), team))
        .collect();
    let active_contracts = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao buscar contratos regulares ativos: {e}"))?;
    let contracts_by_driver: HashMap<String, Contract> = active_contracts
        .into_iter()
        .map(|contract| (contract.piloto_id.clone(), contract))
        .collect();
    Ok((teams_by_id, contracts_by_driver))
}

fn process_driver_evolution(
    conn: &Connection,
    season: &Season,
    standings_by_driver: &HashMap<String, StandingEntry>,
    titles_by_driver: &HashMap<String, i32>,
    contracts_by_driver: &HashMap<String, Contract>,
    teams_by_id: &HashMap<String, Team>,
    rng: &mut impl Rng,
) -> Result<
    (
        Vec<GrowthReport>,
        Vec<MotivationReport>,
        Vec<RetirementInfo>,
        HashSet<String>,
    ),
    String,
> {
    let mut all_drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao buscar pilotos: {e}"))?;
    let existing_names: HashSet<String> = all_drivers
        .iter()
        .map(|driver| driver.nome.clone())
        .collect();
    let mut growth_reports = Vec::new();
    let mut motivation_reports = Vec::new();
    let mut retirements = Vec::new();

    for driver in &mut all_drivers {
        if driver.status != DriverStatus::Ativo {
            continue;
        }

        let standing = standings_by_driver.get(&driver.id).cloned();
        if let Some(standing) = standing {
            let team_car_performance = contracts_by_driver
                .get(&driver.id)
                .and_then(|contract| teams_by_id.get(&contract.equipe_id))
                .map(|team| team.car_performance)
                .unwrap_or(0.0);

            let category_tier = get_category_config(&standing.category)
                .map(|config| config.tier)
                .unwrap_or(0);
            let growth_report = calculate_growth(
                driver,
                &standing.stats,
                team_car_performance,
                category_tier,
                rng,
            );
            if !growth_report.changes.is_empty() {
                growth_reports.push(growth_report);
            }

            let _decline_changes = apply_age_decline(driver, rng);
            let seasons_in_category = driver.temporadas_na_categoria as i32 + 1;
            // Superou a maquina: terminou bem acima do que a forca do carro previa.
            // Expectativa ~ (carros melhores na categoria) * 2 pilotos + 1.
            let cars_ahead = teams_by_id
                .values()
                .filter(|team| {
                    team.categoria == standing.category
                        && team.car_performance > team_car_performance
                })
                .count() as i32;
            let expected_position = cars_ahead * 2 + 1;
            let outperformed_machinery =
                standing.stats.posicao_campeonato + 3 <= expected_position;
            let motivation_ctx = MotivationContext {
                was_champion: standing.position == 1,
                was_promoted: false,
                was_relegated: false,
                contract_renewed: false,
                lost_seat: false,
                seasons_in_category,
                outperformed_machinery,
            };
            let motivation_report =
                adjust_end_of_season_motivation(driver, &standing.stats, &motivation_ctx, rng);
            motivation_reports.push(motivation_report);

            driver.temporadas_na_categoria += 1;
            driver.corridas_na_categoria += standing.stats.corridas.max(0) as u32;
        }

        driver.idade += 1;
        if driver.motivacao < 20.0 {
            driver.temporadas_motivacao_baixa += 1;
        } else {
            driver.temporadas_motivacao_baixa = 0;
        }

        driver.accumulate_career_stats();
        if let Some(titles) = titles_by_driver.get(&driver.id) {
            driver.stats_carreira.titulos += *titles as u32;
        }

        let retirement =
            check_retirement(driver, driver.temporadas_motivacao_baixa as i32, false, rng);
        if retirement.should_retire {
            let reason = retirement
                .reason
                .clone()
                .unwrap_or_else(|| "Aposentadoria".to_string());
            let final_category = driver.categoria_atual.clone().or_else(|| {
                standings_by_driver
                    .get(&driver.id)
                    .map(|standing| standing.category.clone())
            });
            let retired_category = final_category
                .clone()
                .unwrap_or_else(|| "SemCategoria".to_string());
            persist_retired_driver(conn, driver, season, &retired_category, &reason)
                .map_err(|e| format!("Falha ao registrar aposentadoria: {e}"))?;
            process_retirement(driver);
            driver.categoria_atual = None;
            retirements.push(RetirementInfo {
                driver_id: driver.id.clone(),
                driver_name: driver.nome.clone(),
                age: driver.idade as i32,
                reason,
                categoria: final_category,
            });
        }
        driver_queries::update_driver(conn, driver)
            .map_err(|e| format!("Falha ao salvar piloto '{}': {e}", driver.nome))?;
    }

    Ok((
        growth_reports,
        motivation_reports,
        retirements,
        existing_names,
    ))
}

fn create_next_season_phase(
    conn: &Connection,
    season: &Season,
    rng: &mut impl Rng,
    mode: EndOfSeasonMode,
) -> Result<Season, String> {
    let fase_inicial = match mode {
        EndOfSeasonMode::Playable => SeasonPhase::PreTemporada,
        EndOfSeasonMode::HistoricalDraft => SeasonPhase::Temporada,
    };
    let seed: u64 = rng.gen();
    let new_season = create_next_season_9d(conn, season, fase_inicial, seed)?;
    reset_driver_season_stats(conn)?;
    reset_team_season_stats(conn, new_season.numero)?;
    update_meta_for_new_season(conn, new_season.numero, new_season.ano)?;
    Ok(new_season)
}

fn initialize_preseason_phase(
    conn: &Connection,
    new_season: &Season,
    save_path: &Path,
    rng: &mut impl Rng,
    mode: EndOfSeasonMode,
) -> Result<(bool, i32), String> {
    let mut preseason_plan = initialize_preseason(conn, new_season.numero, rng)
        .map_err(|e| format!("Erro ao inicializar pre-temporada: {e}"))?;
    if mode == EndOfSeasonMode::Playable {
        save_preseason_plan(save_path, &preseason_plan)
            .map_err(|e| format!("Erro ao salvar plano da pre-temporada: {e}"))?;
    } else {
        while !preseason_plan.state.is_complete {
            advance_week(conn, &mut preseason_plan)
                .map_err(|e| format!("Erro ao executar pre-temporada historica: {e}"))?;
        }
    }
    Ok((true, preseason_plan.state.total_weeks))
}

fn persist_retired_driver(
    conn: &Connection,
    driver: &Driver,
    season: &Season,
    final_category: &str,
    reason: &str,
) -> Result<(), String> {
    let stats_json = serde_json::to_string(&driver.stats_carreira).map_err(|e| {
        format!(
            "Falha ao serializar estatisticas do piloto aposentado '{}': {e}",
            driver.nome
        )
    })?;
    conn.execute(
        "INSERT OR REPLACE INTO retired (
            piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            &driver.id,
            &driver.nome,
            season.ano.to_string(),
            final_category,
            stats_json,
            reason,
        ],
    )
    .map_err(|e| format!("Falha ao salvar piloto aposentado '{}': {e}", driver.nome))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};
    use rusqlite::Connection;

    use super::*;
    use crate::calendar::generate_calendar_for_category;
    use crate::constants::teams::get_team_templates;
    use crate::db::migrations;
    use crate::db::queries::calendar as calendar_queries;
    use crate::models::contract::Contract;
    use crate::models::driver::Driver;
    use crate::models::enums::{ContractType, DriverStatus, TeamRole};
    use crate::models::team::Team;

    #[test]
    fn test_end_of_season_increments_year() {
        let (mut conn, season) = setup_pipeline_fixture();
        let save_path = unique_test_dir("eos_year");

        let result =
            run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

        assert_eq!(result.new_year, season.ano + 1);
        assert!(
            result.promotion_result.errors.is_empty(),
            "promotion/relegation should keep invariants in fixture: {:?}",
            result.promotion_result.errors
        );
        assert!(result.preseason_initialized);
        assert!(result.preseason_total_weeks >= 3);
        let meta_year: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'current_year'",
                [],
                |row| row.get(0),
            )
            .expect("meta current year");
        assert_eq!(meta_year, (season.ano + 1).to_string());
        assert!(save_path.join("preseason_plan.json").exists());
        let _ = std::fs::remove_dir_all(save_path);
    }

    #[test]
    fn test_end_of_season_creates_new_season() {
        let (mut conn, season) = setup_pipeline_fixture();
        let save_path = unique_test_dir("eos_new_season");

        let result =
            run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

        let active = season_queries::get_active_season(&conn)
            .expect("active season query")
            .expect("new active season");
        assert_eq!(active.id, result.new_season_id);
        assert_eq!(active.numero, season.numero + 1);
        assert_eq!(active.ano, season.ano + 1);
        let _ = std::fs::remove_dir_all(save_path);
    }

    #[test]
    fn test_end_of_season_resets_stats() {
        let (mut conn, season) = setup_pipeline_fixture();
        let save_path = unique_test_dir("eos_reset_stats");

        run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

        let drivers = driver_queries::get_drivers_by_category(&conn, "mazda_rookie")
            .expect("drivers should load");
        assert!(drivers
            .iter()
            .all(|driver| driver.stats_temporada.corridas == 0));
        assert!(drivers
            .iter()
            .all(|driver| driver.stats_temporada.pontos == 0.0));

        let teams =
            team_queries::get_teams_by_category(&conn, "mazda_rookie").expect("teams should load");
        assert!(teams.iter().all(|team| team.stats_pontos == 0));
        assert!(teams.iter().all(|team| team.stats_vitorias == 0));
        let _ = std::fs::remove_dir_all(save_path);
    }

    #[test]
    fn test_end_of_season_retirement_report_keeps_final_category() {
        let (mut conn, season) = setup_pipeline_fixture();
        let save_path = unique_test_dir("eos_retirement_category");

        let mut driver = driver_queries::get_driver(&conn, "P001").expect("retiring driver");
        driver.idade = 47;
        driver_queries::update_driver(&conn, &driver).expect("update retiring driver");

        let result =
            run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

        let retirement = result
            .retirements
            .iter()
            .find(|entry| entry.driver_id == "P001")
            .expect("driver should retire");
        assert_eq!(retirement.categoria.as_deref(), Some("mazda_rookie"));

        let _ = std::fs::remove_dir_all(save_path);
    }

    #[test]
    fn test_end_of_season_archive_excludes_newly_generated_rookies() {
        let (mut conn, season) = setup_pipeline_fixture();
        let save_path = unique_test_dir("eos_archive_excludes_rookies");

        let result =
            run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

        let archived_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM driver_season_archive WHERE season_number = ?1",
                rusqlite::params![season.numero],
                |row| row.get(0),
            )
            .expect("archive count");
        assert_eq!(
            archived_count, 2,
            "only season participants should be archived"
        );

        for rookie in &result.rookies_generated {
            let rookie_archived: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM driver_season_archive WHERE piloto_id = ?1 AND season_number = ?2",
                    rusqlite::params![&rookie.driver_id, season.numero],
                    |row| row.get(0),
                )
                .expect("rookie archive count");
            assert_eq!(
                rookie_archived, 0,
                "rookie '{}' should not be archived for the previous season",
                rookie.driver_id
            );
        }

        let _ = std::fs::remove_dir_all(save_path);
    }

    #[test]
    fn test_end_of_season_standings_keep_regular_team_when_special_contract_is_active() {
        let (mut conn, season) = setup_pipeline_fixture();
        let save_path = unique_test_dir("eos_regular_contract_priority");

        let special_team = sample_named_team(
            "production_challenger",
            "SP001",
            "Special Team",
            Some("mazda"),
            1234,
        );
        team_queries::insert_team(&conn, &special_team).expect("insert special team");

        let mut special_contract = Contract::new(
            "C900".to_string(),
            "P001".to_string(),
            "Piloto A".to_string(),
            special_team.id.clone(),
            special_team.nome.clone(),
            1,
            1,
            50_000.0,
            TeamRole::Numero1,
            "production_challenger".to_string(),
        );
        special_contract.tipo = ContractType::Especial;
        special_contract.classe = Some("mazda".to_string());
        contract_queries::insert_contract(&conn, &special_contract)
            .expect("insert special contract");

        run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

        let standings_team_id: String = conn
            .query_row(
                "SELECT equipe_id FROM standings
                 WHERE temporada_id = ?1 AND piloto_id = ?2",
                rusqlite::params![&season.id, "P001"],
                |row| row.get(0),
            )
            .expect("standing for driver");
        assert_eq!(standings_team_id, "T001");

        let _ = std::fs::remove_dir_all(save_path);
    }

    #[test]
    fn test_promotion_initializes_preseason_after_movements() {
        let (mut conn, season, promoted_team_id, _second_driver_id) =
            setup_promotion_order_fixture();
        let save_path = unique_test_dir("eos_preseason_order");

        let result =
            run_end_of_season(&mut conn, &season, &save_path).expect("pipeline should run");

        // Fase 4B: a campea do gt4 sobe para a Endurance classe gt4 levando os
        // pilotos; nenhum movimento toca LMP2.
        assert!(result
            .promotion_result
            .movements
            .iter()
            .all(|movement| movement.from_category != "lmp2" && movement.to_category != "lmp2"));
        assert!(result.preseason_initialized);
        assert!(result.preseason_total_weeks >= 3);

        let promoted_team = team_queries::get_team_by_id(&conn, &promoted_team_id)
            .expect("team query")
            .expect("promoted team");
        assert_eq!(promoted_team.categoria, "endurance");
        assert_eq!(promoted_team.classe.as_deref(), Some("gt4"));
        assert!(promoted_team.piloto_1_id.is_some() || promoted_team.piloto_2_id.is_some());

        assert!(save_path.join("preseason_plan.json").exists());
        let _ = std::fs::remove_dir_all(save_path);
    }

    #[test]
    fn test_end_of_season_rolls_back_when_preseason_plan_save_fails() {
        let (mut conn, season) = setup_pipeline_fixture();
        let blocked_path = unique_test_dir("eos_save_failure").join("blocked_path");
        std::fs::write(&blocked_path, "not a directory").expect("blocker file");
        let mut retiring_driver =
            driver_queries::get_driver(&conn, "P001").expect("retiring driver");
        retiring_driver.idade = 47;
        driver_queries::update_driver(&conn, &retiring_driver).expect("update retiring driver");

        let result = run_end_of_season(&mut conn, &season, &blocked_path);

        assert!(
            result.is_err(),
            "pipeline should fail when save path is invalid"
        );
        let active = season_queries::get_active_season(&conn)
            .expect("active season query")
            .expect("original season should remain active");
        assert_eq!(active.id, season.id);
        let all_seasons = season_queries::get_all_seasons(&conn).expect("all seasons");
        assert_eq!(all_seasons.len(), 1, "new season should not be persisted");

        let retired_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM retired", [], |row| row.get(0))
            .expect("retired count");
        assert_eq!(retired_count, 0, "retirement snapshot should rollback");
        let driver = driver_queries::get_driver(&conn, "P001").expect("driver after rollback");
        assert_eq!(driver.status, DriverStatus::Ativo);
        assert_eq!(driver.categoria_atual.as_deref(), Some("mazda_rookie"));
        assert_eq!(driver.idade, 47);

        let _ = std::fs::remove_dir_all(blocked_path.parent().expect("parent"));
    }

    fn setup_pipeline_fixture() -> (Connection, Season) {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        let season = Season::new("S001".to_string(), 1, 2024);
        season_queries::insert_season(&conn, &season).expect("season insert");
        seed_pipeline_supporting_teams(&conn);

        let mut rng = StdRng::seed_from_u64(10);
        let team_a = sample_team("mazda_rookie", "T001", &mut rng);
        let team_b = sample_team("mazda_rookie", "T002", &mut rng);
        team_queries::insert_team(&conn, &team_a).expect("team a");
        team_queries::insert_team(&conn, &team_b).expect("team b");

        let driver_a = sample_driver("P001", "Piloto A", "mazda_rookie", 120.0, 3, 5, 0);
        let driver_b = sample_driver("P002", "Piloto B", "mazda_rookie", 90.0, 1, 4, 1);
        driver_queries::insert_driver(&conn, &driver_a).expect("driver a");
        driver_queries::insert_driver(&conn, &driver_b).expect("driver b");

        let contract_a = Contract::new(
            "C001".to_string(),
            driver_a.id.clone(),
            driver_a.nome.clone(),
            team_a.id.clone(),
            team_a.nome.clone(),
            1,
            2,
            100_000.0,
            TeamRole::Numero1,
            "mazda_rookie".to_string(),
        );
        let contract_b = Contract::new(
            "C002".to_string(),
            driver_b.id.clone(),
            driver_b.nome.clone(),
            team_b.id.clone(),
            team_b.nome.clone(),
            1,
            2,
            90_000.0,
            TeamRole::Numero1,
            "mazda_rookie".to_string(),
        );
        contract_queries::insert_contract(&conn, &contract_a).expect("contract a");
        contract_queries::insert_contract(&conn, &contract_b).expect("contract b");

        let mut calendar_rng = StdRng::seed_from_u64(20);
        let entry = generate_calendar_for_category(&season.id, "mazda_rookie", &mut calendar_rng)
            .expect("calendar")
            .into_iter()
            .next()
            .expect("calendar entry");
        calendar_queries::insert_calendar_entry(&conn, &entry).expect("calendar insert");
        calendar_queries::mark_race_completed(&conn, &entry.id).expect("mark complete");
        conn.execute(
            "UPDATE meta SET value = '3' WHERE key = 'next_driver_id'",
            [],
        )
        .expect("meta driver counter");
        conn.execute(
            "UPDATE meta SET value = '3' WHERE key = 'next_contract_id'",
            [],
        )
        .expect("meta contract counter");
        conn.execute(
            "UPDATE meta SET value = '2' WHERE key = 'next_season_id'",
            [],
        )
        .expect("meta season counter");
        conn.execute("UPDATE meta SET value = '2' WHERE key = 'next_race_id'", [])
            .expect("meta race counter");

        (conn, season)
    }

    fn setup_promotion_order_fixture() -> (Connection, Season, String, String) {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        let previous = Season::new("OLD1".to_string(), 1, 2024);
        season_queries::insert_season(&conn, &previous).expect("previous season");
        season_queries::finalize_season(&conn, &previous.id).expect("finalize previous season");

        let season = Season::new("CUR2".to_string(), 2, 2025);
        season_queries::insert_season(&conn, &season).expect("current season");

        seed_promotion_teams(&conn);
        seed_gt4_promotion_drivers(&conn);

        conn.execute(
            "UPDATE meta SET value = '2' WHERE key = 'current_season'",
            [],
        )
        .expect("meta current season");
        conn.execute(
            "UPDATE meta SET value = '2025' WHERE key = 'current_year'",
            [],
        )
        .expect("meta current year");

        (conn, season, "GT4PROMO".to_string(), "GT4LOW".to_string())
    }

    fn seed_promotion_teams(conn: &Connection) {
        insert_ranked_teams(conn, "mazda_rookie", "MR", 6, None);
        insert_ranked_teams(conn, "toyota_rookie", "TR", 6, None);
        insert_ranked_teams(conn, "mazda_amador", "MA", 10, None);
        insert_ranked_teams(conn, "toyota_amador", "TA", 10, None);
        insert_ranked_teams(conn, "bmw_m2", "BM", 10, None);
        insert_ranked_teams(conn, "production_challenger", "PM", 6, Some("mazda"));
        insert_ranked_teams(conn, "production_challenger", "PT", 6, Some("toyota"));
        insert_ranked_teams(conn, "production_challenger", "PB", 6, Some("bmw"));
        insert_ranked_teams(conn, "gt4", "GT4", 9, None);
        insert_ranked_teams(conn, "gt3", "GT3", 14, None);
        insert_ranked_teams(conn, "endurance", "EG4", 6, Some("gt4"));
        insert_ranked_teams(conn, "endurance", "EG3", 6, Some("gt3"));
        insert_ranked_teams(conn, "endurance", "LMP", 6, Some("lmp2"));

        let mut promoted_team = sample_named_team("gt4", "GT4PROMO", "GT4 Promo Team", None, 9001);
        promoted_team.stats_pontos = 999;
        promoted_team.stats_vitorias = 8;
        promoted_team.stats_melhor_resultado = 1;
        team_queries::insert_team(conn, &promoted_team).expect("insert promoted gt4 team");
    }

    fn seed_pipeline_supporting_teams(conn: &Connection) {
        insert_ranked_teams(conn, "mazda_rookie", "MR", 4, None);
        insert_ranked_teams(conn, "toyota_rookie", "TR", 6, None);
        insert_ranked_teams(conn, "mazda_amador", "MA", 10, None);
        insert_ranked_teams(conn, "toyota_amador", "TA", 10, None);
        insert_ranked_teams(conn, "bmw_m2", "BM", 10, None);
        insert_ranked_teams(conn, "production_challenger", "PM", 6, Some("mazda"));
        insert_ranked_teams(conn, "production_challenger", "PT", 6, Some("toyota"));
        insert_ranked_teams(conn, "production_challenger", "PB", 6, Some("bmw"));
        insert_ranked_teams(conn, "gt4", "GT4", 10, None);
        insert_ranked_teams(conn, "gt3", "GT3", 14, None);
        insert_ranked_teams(conn, "endurance", "EG4", 6, Some("gt4"));
        insert_ranked_teams(conn, "endurance", "EG3", 6, Some("gt3"));
        insert_ranked_teams(conn, "endurance", "LMP", 6, Some("lmp2"));
    }

    fn seed_gt4_promotion_drivers(conn: &Connection) {
        let licensed_driver = sample_driver("GT4TOP", "Piloto Licenciado", "gt4", 200.0, 4, 10, 0);
        let unlicensed_driver = sample_driver("GT4LOW", "Piloto Sem Licenca", "gt4", 5.0, 0, 10, 2);
        let support_drivers = [
            sample_driver("GT4D1", "GT4 Driver 1", "gt4", 150.0, 3, 10, 0),
            sample_driver("GT4D2", "GT4 Driver 2", "gt4", 130.0, 2, 10, 0),
            sample_driver("GT4D3", "GT4 Driver 3", "gt4", 110.0, 2, 10, 0),
            sample_driver("GT4D4", "GT4 Driver 4", "gt4", 90.0, 1, 10, 1),
            sample_driver("GT4D5", "GT4 Driver 5", "gt4", 70.0, 1, 10, 1),
            sample_driver("GT4D6", "GT4 Driver 6", "gt4", 50.0, 0, 10, 1),
        ];

        for driver in [&licensed_driver, &unlicensed_driver] {
            driver_queries::insert_driver(conn, driver).expect("insert promoted team driver");
        }
        for driver in &support_drivers {
            driver_queries::insert_driver(conn, driver).expect("insert support driver");
        }

        let contract_1 = Contract::new(
            "KGT401".to_string(),
            licensed_driver.id.clone(),
            licensed_driver.nome.clone(),
            "GT4PROMO".to_string(),
            "GT4 Promo Team".to_string(),
            2,
            2,
            150_000.0,
            TeamRole::Numero1,
            "gt4".to_string(),
        );
        let contract_2 = Contract::new(
            "KGT402".to_string(),
            unlicensed_driver.id.clone(),
            unlicensed_driver.nome.clone(),
            "GT4PROMO".to_string(),
            "GT4 Promo Team".to_string(),
            2,
            2,
            120_000.0,
            TeamRole::Numero2,
            "gt4".to_string(),
        );
        contract_queries::insert_contract(conn, &contract_1).expect("insert contract 1");
        contract_queries::insert_contract(conn, &contract_2).expect("insert contract 2");
        team_queries::update_team_pilots(
            conn,
            "GT4PROMO",
            Some(&licensed_driver.id),
            Some(&unlicensed_driver.id),
        )
        .expect("assign promoted team pilots");
    }

    fn insert_ranked_teams(
        conn: &Connection,
        category: &str,
        prefix: &str,
        count: usize,
        class: Option<&str>,
    ) {
        for index in 0..count {
            let rank = index + 1;
            let mut team = sample_named_team(
                category,
                &format!("{prefix}{rank}"),
                &format!("{prefix} Team {rank}"),
                class,
                rank as u64 + prefix.bytes().map(u64::from).sum::<u64>(),
            );
            team.stats_pontos = ((count - index) * 10) as i32;
            team.stats_vitorias = (count - index) as i32;
            team.stats_melhor_resultado = rank as i32;
            team_queries::insert_team(conn, &team).expect("insert ranked team");
        }
    }

    fn sample_driver(
        id: &str,
        name: &str,
        category: &str,
        points: f64,
        wins: u32,
        races: u32,
        dnfs: u32,
    ) -> Driver {
        let mut driver = Driver::new(
            id.to_string(),
            name.to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2020,
        );
        driver.categoria_atual = Some(category.to_string());
        driver.stats_temporada.pontos = points;
        driver.stats_temporada.vitorias = wins;
        driver.stats_temporada.podios = wins + 1;
        driver.stats_temporada.corridas = races;
        driver.stats_temporada.dnfs = dnfs;
        driver.stats_temporada.poles = wins;
        driver.stats_temporada.posicao_media = 4.0;
        driver
    }

    fn sample_team(category: &str, id: &str, rng: &mut StdRng) -> Team {
        let template = get_team_templates(category)[0];
        Team::from_template_with_rng(template, category, id.to_string(), 2024, rng)
    }

    fn sample_named_team(
        category: &str,
        id: &str,
        name: &str,
        class: Option<&str>,
        seed: u64,
    ) -> Team {
        let template = crate::constants::teams::get_reference_team_template(category, class)
            .expect("team template");
        let mut rng = StdRng::seed_from_u64(seed);
        let mut team =
            Team::from_template_with_rng(template, category, id.to_string(), 2025, &mut rng);
        team.nome = name.to_string();
        team.nome_curto = name.to_string();
        team.classe = class.map(str::to_string);
        team
    }

    fn unique_test_dir(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("iracerapp_eos_{label}_{nanos}"));
        std::fs::create_dir_all(&path).expect("temp dir");
        path
    }
}
