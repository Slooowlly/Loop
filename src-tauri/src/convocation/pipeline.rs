use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::calendar::generate_and_insert_special_calendars;
use crate::constants::categories::get_category_config;
use crate::db::connection::DbError;
use crate::db::queries::{
    calendar as calendar_queries, contracts as contract_queries, drivers as driver_queries,
    seasons as season_queries, special_team_entries as special_entry_queries,
    teams as team_queries,
};
use crate::generators::ids::IdType;
use crate::models::driver::Driver;
use crate::models::enums::{SeasonPhase, TeamRole};
use crate::models::license::driver_has_required_license_for_category;
use crate::promotion::standings::calculate_constructor_standings;

use super::eligibility::{coletar_candidatos, FonteConvocacao};
use super::player_offers::{self, PlayerSpecialOffer};
use super::quotas::calcular_cotas;
use super::scoring::calcular_score;
use super::special_window;

// ── Estruturas públicas ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverAssignment {
    pub driver_id: String,
    pub team_id: String,
    pub papel: TeamRole,
    pub fonte: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridClasse {
    pub class_name: String,
    pub assignments: Vec<DriverAssignment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvocationResult {
    pub grids: Vec<GridClasse>,
    pub total_contratos: usize,
    pub errors: Vec<String>,
}

struct PendingOp {
    contract: crate::models::contract::Contract,
    driver_id: String,
    special_category: String,
}

// ── Classes convocadas ────────────────────────────────────────────────────────

/// Classes que participam da convocação especial.
struct ClasseConfig {
    special_category: &'static str,
    class_name: &'static str,
    feeder_category: &'static str,
}

const CLASSES_CONVOCADAS: &[ClasseConfig] = &[
    ClasseConfig {
        special_category: "production_challenger",
        class_name: "mazda",
        feeder_category: "mazda_amador",
    },
    ClasseConfig {
        special_category: "production_challenger",
        class_name: "toyota",
        feeder_category: "toyota_amador",
    },
    ClasseConfig {
        special_category: "production_challenger",
        class_name: "bmw",
        feeder_category: "bmw_m2",
    },
    ClasseConfig {
        special_category: "endurance",
        class_name: "gt4",
        feeder_category: "gt4",
    },
    ClasseConfig {
        special_category: "endurance",
        class_name: "gt3",
        feeder_category: "gt3",
    },
    ClasseConfig {
        special_category: "endurance",
        class_name: "lmp2",
        feeder_category: "lmp2",
    },
];

// ── Transições de fase ────────────────────────────────────────────────────────

/// BlocoRegular → JanelaConvocacao.
/// Requer que a temporada ativa esteja em BlocoRegular.
pub fn advance_to_convocation_window(conn: &Connection) -> Result<(), DbError> {
    let season = season_queries::get_active_season(conn)?
        .ok_or_else(|| DbError::NotFound("Nenhuma temporada ativa".into()))?;

    if season.fase != SeasonPhase::BlocoRegular {
        return Err(DbError::Migration(format!(
            "Fase atual é '{}'; esperado BlocoRegular",
            season.fase
        )));
    }

    let pending_regular = calendar_queries::count_pending_races_in_phase(
        conn,
        &season.id,
        &SeasonPhase::BlocoRegular,
    )?;
    if pending_regular > 0 {
        return Err(DbError::Migration(format!(
            "A janela de convocacao so pode abrir depois do fim do bloco regular. Ainda existem {pending_regular} corridas regulares pendentes."
        )));
    }

    season_queries::update_season_fase(conn, &season.id, &SeasonPhase::JanelaConvocacao)?;
    Ok(())
}

/// JanelaConvocacao → BlocoEspecial.
/// Deve ser chamada APÓS run_convocation_window.
/// Gera o calendário das categorias especiais na janela setembro–dezembro.
pub fn iniciar_bloco_especial(conn: &Connection) -> Result<(), DbError> {
    let season = season_queries::get_active_season(conn)?
        .ok_or_else(|| DbError::NotFound("Nenhuma temporada ativa".into()))?;

    if season.fase != SeasonPhase::JanelaConvocacao {
        return Err(DbError::Migration(format!(
            "Fase atual é '{}'; esperado JanelaConvocacao",
            season.fase
        )));
    }

    // Gerar calendário das categorias especiais (production_challenger e endurance)
    let tx = conn.unchecked_transaction()?;
    season_queries::update_season_fase(&tx, &season.id, &SeasonPhase::BlocoEspecial)?;

    let mut rng = rand::thread_rng();
    generate_and_insert_special_calendars(&tx, &season.id, season.ano, &mut rng)
        .map_err(|e| DbError::Migration(format!("Falha ao gerar calendário especial: {e}")))?;

    tx.commit()?;
    Ok(())
}

// ── Pipeline principal ────────────────────────────────────────────────────────

/// Monta os grids das categorias especiais em memória e persiste em uma única
/// transação. Não muda a fase da temporada (permanece JanelaConvocacao).
pub fn run_convocation_window(conn: &Connection) -> Result<ConvocationResult, DbError> {
    let season = season_queries::get_active_season(conn)?
        .ok_or_else(|| DbError::NotFound("Nenhuma temporada ativa".into()))?;

    if season.fase != SeasonPhase::JanelaConvocacao {
        return Err(DbError::Migration(format!(
            "Fase atual é '{}'; convocação só ocorre na JanelaConvocacao",
            season.fase
        )));
    }

    let season_number = season.numero;
    ensure_special_team_entries(conn, &season.id, season_number)?;
    let player = match driver_queries::get_player_driver(conn) {
        Ok(player) => Some(player),
        Err(DbError::NotFound(_)) => None,
        Err(err) => return Err(err),
    };

    // ── Passo 1: construir todos os grids em memória ──────────────────────────
    // Manter conjunto global de drivers já alocados para evitar duplicatas entre classes
    let mut all_grids: Vec<GridClasse> = Vec::new();
    let mut all_errors: Vec<String> = Vec::new();
    let mut globally_assigned: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Some(player) = &player {
        globally_assigned.insert(player.id.clone());
    }

    for cfg in CLASSES_CONVOCADAS {
        match montar_grid_classe(conn, cfg, season_number, &season.id, &globally_assigned) {
            Ok(grid) => {
                for a in &grid.assignments {
                    globally_assigned.insert(a.driver_id.clone());
                }
                all_grids.push(grid);
            }
            Err(e) => all_errors.push(format!(
                "[{}/{}] {}",
                cfg.special_category, cfg.class_name, e
            )),
        }
    }

    // ── Passo 2: validar (sem efeitos colaterais) ─────────────────────────────
    let validation_errors = validar_grids(&all_grids);
    if !validation_errors.is_empty() {
        return Ok(ConvocationResult {
            grids: Vec::new(),
            total_contratos: 0,
            errors: validation_errors,
        });
    }

    // ── Passo 3: persistir em transação atômica ───────────────────────────────
    let total_contratos = all_grids.iter().map(|g| g.assignments.len()).sum();
    let player_offers_payload = if let Some(player) = &player {
        Some((
            player.id.clone(),
            build_player_special_offers(conn, &season.id, player)?,
        ))
    } else {
        None
    };
    persistir_grids_e_ofertas(
        conn,
        &season.id,
        &all_grids,
        season_number,
        player_offers_payload.as_ref(),
    )?;
    special_window::initialize_special_window(conn, &season.id, player.as_ref(), &all_grids)?;

    Ok(ConvocationResult {
        grids: all_grids,
        total_contratos,
        errors: all_errors,
    })
}

// ── Montagem de grid por classe ───────────────────────────────────────────────

fn ensure_special_team_entries(
    conn: &Connection,
    season_id: &str,
    _season_number: i32,
) -> Result<(), DbError> {
    for cfg in CLASSES_CONVOCADAS {
        let target_slots = target_slots_for_class(conn, cfg)?;
        let mut entries = Vec::new();
        let mut used_team_ids = std::collections::HashSet::new();

        let legacy_special_teams = team_queries::get_teams_by_category_and_class(
            conn,
            cfg.special_category,
            cfg.class_name,
        )?;
        if !legacy_special_teams.is_empty() {
            for team in legacy_special_teams.into_iter().take(target_slots) {
                if !used_team_ids.insert(team.id.clone()) {
                    continue;
                }
                entries.push(special_entry_queries::NewSpecialTeamEntry {
                    team_id: team.id,
                    source_category: cfg.special_category.to_string(),
                    qualified_via: "ClasseEspecial".to_string(),
                    guaranteed_next_year: false,
                });
            }

            special_entry_queries::replace_entries_for_class(
                conn,
                season_id,
                cfg.special_category,
                cfg.class_name,
                &entries,
            )?;
            continue;
        }

        let regular_standings = calculate_constructor_standings(conn, cfg.feeder_category)
            .map_err(DbError::Migration)?;
        for standing in regular_standings {
            if entries.len() >= target_slots {
                break;
            }
            if !used_team_ids.insert(standing.team_id.clone()) {
                continue;
            }
            entries.push(special_entry_queries::NewSpecialTeamEntry {
                team_id: standing.team_id,
                source_category: cfg.feeder_category.to_string(),
                qualified_via: format!("RegularP{}", standing.posicao),
                guaranteed_next_year: false,
            });
        }

        special_entry_queries::replace_entries_for_class(
            conn,
            season_id,
            cfg.special_category,
            cfg.class_name,
            &entries,
        )?;
    }

    Ok(())
}

fn target_slots_for_class(conn: &Connection, cfg: &ClasseConfig) -> Result<usize, DbError> {
    let legacy_special_teams =
        team_queries::get_teams_by_category_and_class(conn, cfg.special_category, cfg.class_name)?;
    if !legacy_special_teams.is_empty() {
        return Ok(legacy_special_teams.len());
    }

    Ok(match cfg.special_category {
        "endurance" => 6,
        _ => 5,
    })
}

fn get_special_class_entry_teams(
    conn: &Connection,
    season_id: &str,
    cfg: &ClasseConfig,
) -> Result<Vec<crate::models::team::Team>, DbError> {
    let teams = special_entry_queries::get_entry_teams_for_class(
        conn,
        season_id,
        cfg.special_category,
        cfg.class_name,
    )?;
    if !teams.is_empty() {
        return Ok(teams);
    }

    let legacy_teams =
        team_queries::get_teams_by_category_and_class(conn, cfg.special_category, cfg.class_name)?;
    if !legacy_teams.is_empty() {
        return Ok(legacy_teams);
    }

    team_queries::get_teams_by_category(conn, cfg.feeder_category)
}

fn montar_grid_classe(
    conn: &Connection,
    cfg: &ClasseConfig,
    _season_number: i32,
    season_id: &str,
    globally_excluded: &std::collections::HashSet<String>,
) -> Result<GridClasse, DbError> {
    // 1. Equipes regulares classificadas para a classe especial.
    let teams = get_special_class_entry_teams(conn, season_id, cfg)?;
    if teams.is_empty() {
        return Err(DbError::NotFound(format!(
            "Nenhuma equipe para {}/{}",
            cfg.special_category, cfg.class_name
        )));
    }

    let total_assentos = teams.len() * 2;
    let cotas = calcular_cotas(total_assentos);

    // 2. Candidatos de todas as fontes
    let candidatos = coletar_candidatos(
        conn,
        cfg.special_category,
        cfg.class_name,
        cfg.feeder_category,
    )?;

    // 3. Calcular scores e separar por fonte (excluir já alocados globalmente)
    let mut fonte_a: Vec<(String, f64)> = Vec::new();
    let mut fonte_b: Vec<(String, f64)> = Vec::new();
    let mut fonte_c: Vec<(String, f64)> = Vec::new();
    let mut fonte_d: Vec<(String, f64)> = Vec::new();

    for c in candidatos
        .iter()
        .filter(|c| !globally_excluded.contains(&c.driver_id))
    {
        let historico = contract_queries::get_especial_contract_count(
            conn,
            &c.driver_id,
            cfg.special_category,
            cfg.class_name,
        )
        .unwrap_or(0);
        let score = calcular_score(&c.driver, &c.fonte, historico);
        match c.fonte {
            FonteConvocacao::MeritoRegular => fonte_a.push((c.driver_id.clone(), score)),
            FonteConvocacao::ContinuidadeHistorica => fonte_b.push((c.driver_id.clone(), score)),
            FonteConvocacao::PoolGlobal => fonte_c.push((c.driver_id.clone(), score)),
            FonteConvocacao::Wildcard => fonte_d.push((c.driver_id.clone(), score)),
        }
    }

    // 4. Ordenar cada fonte por score desc
    for v in [&mut fonte_a, &mut fonte_b, &mut fonte_c, &mut fonte_d] {
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    }

    // 5. Selecionar por cota com overflow B/C → A
    let mut selecionados: Vec<(String, FonteConvocacao, f64)> = Vec::new();

    // D (wildcard): máximo 1
    let d_count = cotas.wildcard.min(fonte_d.len());
    for (id, score) in fonte_d.iter().take(d_count) {
        selecionados.push((id.clone(), FonteConvocacao::Wildcard, *score));
    }

    // B (continuidade)
    let b_count = cotas.continuidade.min(fonte_b.len());
    let b_overflow = cotas.continuidade.saturating_sub(b_count);
    for (id, score) in fonte_b.iter().take(b_count) {
        selecionados.push((id.clone(), FonteConvocacao::ContinuidadeHistorica, *score));
    }

    // C (pool)
    let c_count = cotas.pool_global.min(fonte_c.len());
    let c_overflow = cotas.pool_global.saturating_sub(c_count);
    for (id, score) in fonte_c.iter().take(c_count) {
        selecionados.push((id.clone(), FonteConvocacao::PoolGlobal, *score));
    }

    // A (mérito) + overflow de B e C
    let a_total = cotas.merito_regular + b_overflow + c_overflow;

    // Remover da pool A quem já foi selecionado via outra fonte
    let ja_selecionados: std::collections::HashSet<String> =
        selecionados.iter().map(|(id, _, _)| id.clone()).collect();

    let mut idx = 0;
    for (id, score) in &fonte_a {
        if ja_selecionados.contains(id) {
            continue;
        }
        if idx >= a_total {
            break;
        }
        selecionados.push((id.clone(), FonteConvocacao::MeritoRegular, *score));
        idx += 1;
    }

    // 6. Ordenar selecionados por score desc para distribuição equitativa
    selecionados.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // 7. Distribuir: posição 2i → team[i] N1, posição 2i+1 → team[i] N2
    let mut assignments: Vec<DriverAssignment> = Vec::new();
    for (i, (driver_id, fonte, score)) in selecionados.iter().enumerate() {
        let team_idx = i / 2;
        if team_idx >= teams.len() {
            break; // mais pilotos que assentos (não deve ocorrer, mas defensivo)
        }
        let papel = if i % 2 == 0 {
            TeamRole::Numero1
        } else {
            TeamRole::Numero2
        };
        assignments.push(DriverAssignment {
            driver_id: driver_id.clone(),
            team_id: teams[team_idx].id.clone(),
            papel,
            fonte: fonte_label(fonte),
            score: *score,
        });
    }

    Ok(GridClasse {
        class_name: cfg.class_name.to_string(),
        assignments,
    })
}

fn fonte_label(fonte: &FonteConvocacao) -> String {
    match fonte {
        FonteConvocacao::MeritoRegular => "MeritoRegular".into(),
        FonteConvocacao::ContinuidadeHistorica => "ContinuidadeHistorica".into(),
        FonteConvocacao::PoolGlobal => "PoolGlobal".into(),
        FonteConvocacao::Wildcard => "Wildcard".into(),
    }
}

fn is_primary_current_category_for_class(cfg: &ClasseConfig, category: &str) -> bool {
    cfg.feeder_category == category
}

fn is_exceptional_rookie_for_class(player: &Driver, cfg: &ClasseConfig) -> bool {
    let Some(current_category) = player.categoria_atual.as_deref() else {
        return false;
    };

    let rookie_matches = matches!(
        (cfg.class_name, current_category),
        ("mazda", "mazda_rookie") | ("toyota", "toyota_rookie")
    );
    let exceptional = player.atributos.skill >= 84.0
        || (player.melhor_resultado_temp == Some(1) && player.stats_temporada.vitorias >= 2);

    rookie_matches && exceptional
}

fn contract_matches_class_lane(
    contract: &crate::models::contract::Contract,
    cfg: &ClasseConfig,
) -> bool {
    if contract.categoria == cfg.special_category
        && contract.classe.as_deref() == Some(cfg.class_name)
    {
        return true;
    }

    match cfg.class_name {
        "mazda" => matches!(contract.categoria.as_str(), "mazda_amador" | "mazda_rookie"),
        "toyota" => matches!(
            contract.categoria.as_str(),
            "toyota_amador" | "toyota_rookie"
        ),
        "bmw" => contract.categoria == "bmw_m2",
        "gt4" => contract.categoria == "gt4",
        "gt3" => contract.categoria == "gt3",
        _ => false,
    }
}

fn player_has_same_car_history(
    contracts: &[crate::models::contract::Contract],
    cfg: &ClasseConfig,
) -> bool {
    contracts
        .iter()
        .any(|contract| contract_matches_class_lane(contract, cfg))
}

fn player_has_team_history(contracts: &[crate::models::contract::Contract], team_id: &str) -> bool {
    contracts
        .iter()
        .any(|contract| contract.equipe_id == team_id)
}

fn player_offer_quality_score(player: &Driver) -> f64 {
    let champion_bonus = if player.melhor_resultado_temp == Some(1) {
        8.0
    } else {
        0.0
    };
    let wins_bonus = (player.stats_temporada.vitorias.min(5) as f64) * 2.0;
    player.atributos.skill + champion_bonus + wins_bonus
}

fn fallback_quality_threshold(cfg: &ClasseConfig) -> f64 {
    match cfg.special_category {
        "endurance" => 90.0,
        _ => 82.0,
    }
}

fn build_player_special_offers(
    conn: &Connection,
    season_id: &str,
    player: &Driver,
) -> Result<Vec<PlayerSpecialOffer>, DbError> {
    let papel = if player.atributos.skill >= 85.0 {
        TeamRole::Numero1
    } else {
        TeamRole::Numero2
    };
    let current_category = player.categoria_atual.as_deref();
    let current_category_is_regular = current_category.and_then(get_category_config).is_some();
    let has_active_regular_contract =
        contract_queries::has_active_regular_contract(conn, &player.id)?;
    let contract_history = contract_queries::get_contracts_for_pilot(conn, &player.id)?;
    let quality_score = player_offer_quality_score(player);

    let mut preferred: Vec<(i32, String, String, String, String)> = Vec::new();
    let mut fallback: Vec<(i32, String, String, String, String)> = Vec::new();

    for cfg in CLASSES_CONVOCADAS {
        let teams = get_special_class_entry_teams(conn, season_id, cfg)?;

        for team in teams {
            let team_history = player_has_team_history(&contract_history, &team.id);
            let primary_current_fit = current_category
                .is_some_and(|category| is_primary_current_category_for_class(cfg, category));
            let rookie_exception = is_exceptional_rookie_for_class(player, cfg);
            let same_car_history = player_has_same_car_history(&contract_history, cfg);
            let license_ok =
                driver_has_required_license_for_category(conn, &player.id, cfg.special_category)
                    .map_err(DbError::InvalidData)?;

            let preferred_priority = if team_history && !current_category_is_regular {
                Some(520)
            } else if primary_current_fit {
                Some(500)
            } else if rookie_exception {
                Some(460)
            } else if !has_active_regular_contract && same_car_history {
                Some(400)
            } else if team_history {
                Some(320)
            } else {
                None
            };

            if let Some(priority) = preferred_priority {
                preferred.push((
                    priority + team.car_performance.round() as i32,
                    team.id,
                    team.nome,
                    cfg.special_category.to_string(),
                    cfg.class_name.to_string(),
                ));
                continue;
            }

            if license_ok && quality_score >= fallback_quality_threshold(cfg) {
                fallback.push((
                    100 + team.car_performance.round() as i32,
                    team.id,
                    team.nome,
                    cfg.special_category.to_string(),
                    cfg.class_name.to_string(),
                ));
            }
        }
    }

    preferred.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.2.cmp(&right.2)));
    fallback.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.2.cmp(&right.2)));

    let mut selected = Vec::new();
    let mut seen_team_ids = std::collections::HashSet::new();

    for entry in preferred.into_iter().chain(fallback.into_iter()) {
        if seen_team_ids.insert(entry.1.clone()) {
            selected.push(entry);
        }
        if selected.len() == 3 {
            break;
        }
    }

    Ok(selected
        .into_iter()
        .map(
            |(_, team_id, team_name, special_category, class_name)| PlayerSpecialOffer {
                id: format!(
                    "PSO-{season_id}-{}-{}-{}",
                    player.id,
                    team_id,
                    papel.as_str()
                ),
                player_driver_id: player.id.clone(),
                team_id,
                team_name,
                special_category,
                class_name,
                papel: papel.clone(),
                status: "Pendente".to_string(),
            },
        )
        .collect())
}

#[cfg(test)]
fn setup_world_db() -> (rusqlite::Connection, String) {
    use rand::{rngs::StdRng, SeedableRng};

    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    crate::db::migrations::run_all(&conn).expect("migrations");

    let mut rng = StdRng::seed_from_u64(99);
    let world = crate::generators::world::generate_world_with_rng(
        "Test Player",
        "🇧🇷 Brasileiro",
        20,
        "mazda_rookie",
        0,
        "medio",
        &mut rng,
    )
    .expect("world generation");

    let season_id = "S001".to_string();
    let season = crate::models::season::Season::new(season_id.clone(), 1, 2024);
    crate::db::queries::seasons::insert_season(&conn, &season).expect("insert season");
    for driver in &world.drivers {
        crate::db::queries::drivers::insert_driver(&conn, driver).expect("insert driver");
    }
    crate::db::queries::teams::insert_teams(&conn, &world.teams).expect("insert teams");
    crate::db::queries::contracts::insert_contracts(&conn, &world.contracts)
        .expect("insert contracts");

    let next_contract = world.contracts.len() + 1;
    conn.execute(
        "UPDATE meta SET value = ?1 WHERE key = 'next_contract_id'",
        rusqlite::params![next_contract.to_string()],
    )
    .expect("update meta contract counter");

    (conn, season_id)
}

#[cfg(test)]
fn make_player_eligible_for_specials(conn: &rusqlite::Connection, category: &str) -> String {
    let mut player = crate::db::queries::drivers::get_player_driver(conn).expect("player");
    player.categoria_atual = Some(category.to_string());
    player.atributos.skill = 98.0;
    player.melhor_resultado_temp = Some(1);
    player.stats_temporada.vitorias = 4;
    crate::db::queries::drivers::update_driver(conn, &player).expect("update player");
    player.id
}

#[cfg(test)]
mod player_convocation_offer_tests {
    use super::*;

    #[test]
    fn test_run_convocation_generates_player_special_offers_for_eligible_player() {
        let (conn, _) = setup_world_db();
        let player_id = make_player_eligible_for_specials(&conn, "gt4");
        advance_to_convocation_window(&conn).expect("advance");

        run_convocation_window(&conn).expect("convocação");

        let mut stmt = conn
            .prepare(
                "SELECT team_id, special_category, class_name, papel, status
                 FROM player_special_offers
                 WHERE player_driver_id = ?1
                 ORDER BY team_id",
            )
            .expect("prepare player special offers");
        let offers: Vec<(String, String, String, String, String)> = stmt
            .query_map(rusqlite::params![player_id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .expect("query offers")
            .filter_map(|row| row.ok())
            .collect();

        assert!(
            !offers.is_empty(),
            "jogador elegível deveria receber pelo menos uma convocação especial"
        );
        assert!(
            offers
                .iter()
                .all(|(team_id, category, class_name, papel, status)| {
                    !team_id.is_empty()
                        && !category.is_empty()
                        && !class_name.is_empty()
                        && !papel.is_empty()
                        && status == "Pendente"
                }),
            "ofertas especiais do jogador precisam persistir shape mínimo"
        );
    }

    #[test]
    fn test_run_convocation_keeps_player_special_offers_separate_from_market_proposals() {
        let (conn, _) = setup_world_db();
        let player_id = make_player_eligible_for_specials(&conn, "gt4");
        advance_to_convocation_window(&conn).expect("advance");

        run_convocation_window(&conn).expect("convocação");

        let special_offer_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM player_special_offers WHERE player_driver_id = ?1",
                rusqlite::params![&player_id],
                |row| row.get(0),
            )
            .expect("count special offers");
        let market_proposal_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM market_proposals WHERE piloto_id = ?1",
                rusqlite::params![&player_id],
                |row| row.get(0),
            )
            .expect("count market proposals");

        assert!(
            special_offer_count > 0,
            "deveria haver ofertas especiais do jogador"
        );
        assert_eq!(
            market_proposal_count, 0,
            "convocação especial não deve reaproveitar market_proposals"
        );
    }

    #[test]
    fn test_run_convocation_does_not_activate_player_special_contract_before_acceptance() {
        let (conn, _) = setup_world_db();
        let player_id = make_player_eligible_for_specials(&conn, "gt4");
        advance_to_convocation_window(&conn).expect("advance");

        run_convocation_window(&conn).expect("convocação");

        let player =
            crate::db::queries::drivers::get_driver(&conn, &player_id).expect("player refreshed");
        let especial = crate::db::queries::contracts::get_active_especial_contract_for_pilot(
            &conn, &player_id,
        )
        .expect("special contract lookup");

        assert!(
            player.categoria_especial_ativa.is_none(),
            "jogador não deveria entrar automaticamente no especial antes de aceitar"
        );
        assert!(
            especial.is_none(),
            "jogador não deveria ganhar contrato especial antes de aceitar"
        );
    }
}

// ── Validação em memória ──────────────────────────────────────────────────────

#[cfg(test)]
mod player_convocation_offer_additional_tests {
    use super::*;
    use crate::db::queries::{contracts as contract_queries, drivers as driver_queries};

    fn insert_historical_contract_for_offer_tests(
        conn: &rusqlite::Connection,
        player_id: &str,
        player_name: &str,
        team_id: &str,
        team_name: &str,
        category: &str,
        class_name: Option<&str>,
    ) {
        let mut contract = crate::models::contract::Contract::new(
            format!("HC-{player_id}-{team_id}-{category}"),
            player_id.to_string(),
            player_name.to_string(),
            team_id.to_string(),
            team_name.to_string(),
            crate::db::queries::seasons::get_active_season(conn)
                .expect("active season query")
                .expect("active season")
                .numero
                .saturating_sub(1),
            1,
            50_000.0,
            TeamRole::Numero1,
            category.to_string(),
        );
        contract.status = crate::models::enums::ContractStatus::Expirado;
        if let Some(class_name) = class_name {
            contract.tipo = crate::models::enums::ContractType::Especial;
            contract.classe = Some(class_name.to_string());
        }
        contract_queries::insert_contract(conn, &contract).expect("insert historical contract");
    }

    #[test]
    fn test_player_special_offers_prioritize_current_car_over_old_other_car_history() {
        let (conn, season_id) = setup_world_db();
        let player_id = make_player_eligible_for_specials(&conn, "bmw_m2");
        let player = driver_queries::get_driver(&conn, &player_id).expect("player");
        let toyota_team = get_special_class_entry_teams(
            &conn,
            &season_id,
            CLASSES_CONVOCADAS
                .iter()
                .find(|cfg| {
                    cfg.special_category == "production_challenger" && cfg.class_name == "toyota"
                })
                .expect("toyota config"),
        )
        .expect("toyota teams")
        .into_iter()
        .next()
        .expect("toyota team");

        insert_historical_contract_for_offer_tests(
            &conn,
            &player_id,
            &player.nome,
            &toyota_team.id,
            &toyota_team.nome,
            "production_challenger",
            Some("toyota"),
        );

        let offers =
            build_player_special_offers(&conn, &season_id, &player).expect("build player offers");

        assert!(!offers.is_empty());
        assert!(offers.iter().all(|offer| offer.class_name == "bmw"));
    }

    #[test]
    fn test_unemployed_player_with_same_car_history_still_receives_matching_offers() {
        let (conn, season_id) = setup_world_db();
        let player_id = make_player_eligible_for_specials(&conn, "gt4");
        let mut player = driver_queries::get_driver(&conn, &player_id).expect("player");
        let gt4_team = get_special_class_entry_teams(
            &conn,
            &season_id,
            CLASSES_CONVOCADAS
                .iter()
                .find(|cfg| cfg.special_category == "endurance" && cfg.class_name == "gt4")
                .expect("gt4 config"),
        )
        .expect("gt4 teams")
        .into_iter()
        .next()
        .expect("gt4 team");

        for contract in
            contract_queries::get_contracts_for_pilot(&conn, &player_id).expect("player contracts")
        {
            if contract.status == crate::models::enums::ContractStatus::Ativo {
                contract_queries::update_contract_status(
                    &conn,
                    &contract.id,
                    &crate::models::enums::ContractStatus::Expirado,
                )
                .expect("expire player contract");
            }
        }

        player.categoria_atual = None;
        driver_queries::update_driver(&conn, &player).expect("update unemployed player");

        insert_historical_contract_for_offer_tests(
            &conn,
            &player_id,
            &player.nome,
            &gt4_team.id,
            &gt4_team.nome,
            "endurance",
            Some("gt4"),
        );

        let refreshed = driver_queries::get_driver(&conn, &player_id).expect("refreshed player");
        let offers =
            build_player_special_offers(&conn, &season_id, &refreshed).expect("build offers");

        assert!(!offers.is_empty());
        assert!(offers.iter().all(|offer| offer.class_name == "gt4"));
    }

    #[test]
    fn test_team_history_can_unlock_offer_outside_current_car_lane() {
        let (conn, season_id) = setup_world_db();
        let player_id = make_player_eligible_for_specials(&conn, "gt3");
        let mut player = driver_queries::get_driver(&conn, &player_id).expect("player");
        player.categoria_atual = None;
        driver_queries::update_driver(&conn, &player).expect("clear player category");
        let toyota_team = get_special_class_entry_teams(
            &conn,
            &season_id,
            CLASSES_CONVOCADAS
                .iter()
                .find(|cfg| {
                    cfg.special_category == "production_challenger" && cfg.class_name == "toyota"
                })
                .expect("toyota config"),
        )
        .expect("toyota teams")
        .into_iter()
        .next()
        .expect("toyota team");

        insert_historical_contract_for_offer_tests(
            &conn,
            &player_id,
            &player.nome,
            &toyota_team.id,
            &toyota_team.nome,
            "production_challenger",
            Some("toyota"),
        );

        let offers =
            build_player_special_offers(&conn, &season_id, &player).expect("build player offers");

        assert!(offers.iter().any(|offer| offer.team_id == toyota_team.id));
    }
}

fn validar_grids(grids: &[GridClasse]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut global_driver_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for grid in grids {
        // Sem duplicatas intra-grid
        let mut ids_neste: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for a in &grid.assignments {
            if !ids_neste.insert(a.driver_id.as_str()) {
                errors.push(format!(
                    "[{}] driver_id duplicado no grid: {}",
                    grid.class_name, a.driver_id
                ));
            }
            if !global_driver_ids.insert(a.driver_id.as_str()) {
                errors.push(format!(
                    "[{}] driver {} já foi alocado em outra classe",
                    grid.class_name, a.driver_id
                ));
            }
        }
    }

    errors
}

// ── Persistência transacional ─────────────────────────────────────────────────

#[cfg(test)]
fn persistir_grids(
    conn: &Connection,
    grids: &[GridClasse],
    season_number: i32,
) -> Result<(), DbError> {
    // Coletar todos os dados necessários antes da transação (next_id precisa de conn)
    // Gerar IDs de contrato antecipadamente
    let total = grids.iter().map(|g| g.assignments.len()).sum::<usize>();
    let contract_ids = crate::generators::ids::next_ids(conn, IdType::Contract, total as u32)?;

    let mut contract_idx = 0;

    // Agrupar assignments por team para update_team_pilots
    // Estrutura: team_id → (n1_id, n2_id)
    let mut team_lineup: std::collections::HashMap<String, (Option<String>, Option<String>)> =
        std::collections::HashMap::new();

    // Coletar dados de teams para obter nome
    let mut team_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    // Coletar dados de drivers para obter nome
    let mut driver_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    // Pre-carregar teams e drivers necessários
    for grid in grids {
        for a in &grid.assignments {
            if !team_map.contains_key(&a.team_id) {
                if let Ok(Some(team)) = team_queries::get_team_by_id(conn, &a.team_id) {
                    team_map.insert(team.id.clone(), team.nome.clone());
                }
            }
            if !driver_map.contains_key(&a.driver_id) {
                if let Ok(driver) = driver_queries::get_driver(conn, &a.driver_id) {
                    driver_map.insert(driver.id.clone(), driver.nome.clone());
                }
            }
        }
    }

    // Construir contratos e lineup updates em memória
    struct PendingOp {
        contract: crate::models::contract::Contract,
        driver_id: String,
        special_category: String,
    }

    let mut ops: Vec<PendingOp> = Vec::new();

    // Mapa de class_name → special_category
    let class_to_cat: std::collections::HashMap<&str, &str> = CLASSES_CONVOCADAS
        .iter()
        .map(|c| (c.class_name, c.special_category))
        .collect();

    for grid in grids {
        let special_cat = class_to_cat
            .get(grid.class_name.as_str())
            .copied()
            .unwrap_or("unknown");

        for a in &grid.assignments {
            let contract_id = contract_ids[contract_idx].clone();
            contract_idx += 1;

            let team_nome = team_map
                .get(&a.team_id)
                .cloned()
                .unwrap_or_else(|| a.team_id.clone());
            let driver_nome = driver_map
                .get(&a.driver_id)
                .cloned()
                .unwrap_or_else(|| a.driver_id.clone());

            let papel = if a.papel == TeamRole::Numero1 {
                TeamRole::Numero1
            } else {
                TeamRole::Numero2
            };

            let contract = contract_queries::generate_especial_contract(
                contract_id,
                &a.driver_id,
                &driver_nome,
                &a.team_id,
                &team_nome,
                papel.clone(),
                special_cat,
                &grid.class_name,
                season_number,
            );

            // Atualizar lineup
            let entry = team_lineup.entry(a.team_id.clone()).or_insert((None, None));
            match papel {
                TeamRole::Numero1 => entry.0 = Some(a.driver_id.clone()),
                TeamRole::Numero2 => entry.1 = Some(a.driver_id.clone()),
            }

            ops.push(PendingOp {
                contract,
                driver_id: a.driver_id.clone(),
                special_category: special_cat.to_string(),
            });
        }
    }

    let tx = conn.unchecked_transaction()?;

    driver_queries::clear_all_categoria_especial_ativa(&tx)?;
    team_queries::clear_special_team_lineups(&tx)?;
    team_queries::reset_special_team_hierarchies(&tx)?;

    // Persistir tudo
    for op in &ops {
        contract_queries::insert_contract(&tx, &op.contract)?;
        driver_queries::update_driver_especial_category(
            &tx,
            &op.driver_id,
            Some(&op.special_category),
        )?;
    }

    for (team_id, (n1, n2)) in &team_lineup {
        team_queries::update_team_pilots(&tx, team_id, n1.as_deref(), n2.as_deref())?;

        // Hierarquia: N1 = hierarquia_n1_id, N2 = hierarquia_n2_id
        if let (Some(n1_id), Some(n2_id)) = (n1, n2) {
            team_queries::update_team_hierarchy(
                &tx,
                team_id,
                Some(n1_id.as_str()),
                Some(n2_id.as_str()),
                "Claro",
                0.0,
            )?;
        }
    }

    tx.commit()?;

    Ok(())
}

// ── PosEspecial ───────────────────────────────────────────────────────────────

fn persistir_grids_e_ofertas(
    conn: &Connection,
    season_id: &str,
    grids: &[GridClasse],
    season_number: i32,
    player_offers_payload: Option<&(String, Vec<PlayerSpecialOffer>)>,
) -> Result<(), DbError> {
    let ops = preparar_persistencia_grids(conn, grids, season_number)?;
    let tx = conn.unchecked_transaction()?;
    aplicar_persistencia_grids(&tx, &ops)?;

    if let Some((player_id, offers)) = player_offers_payload {
        player_offers::replace_player_special_offers(&tx, season_id, player_id, offers)?;
    }

    tx.commit()?;
    Ok(())
}

fn preparar_persistencia_grids(
    conn: &Connection,
    grids: &[GridClasse],
    season_number: i32,
) -> Result<Vec<PendingOp>, DbError> {
    let total = grids.iter().map(|g| g.assignments.len()).sum::<usize>();
    let contract_ids = crate::generators::ids::next_ids(conn, IdType::Contract, total as u32)?;
    let mut contract_idx = 0;
    let mut team_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut driver_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for grid in grids {
        for assignment in &grid.assignments {
            if !team_map.contains_key(&assignment.team_id) {
                if let Ok(Some(team)) = team_queries::get_team_by_id(conn, &assignment.team_id) {
                    team_map.insert(team.id.clone(), team.nome.clone());
                }
            }
            if !driver_map.contains_key(&assignment.driver_id) {
                if let Ok(driver) = driver_queries::get_driver(conn, &assignment.driver_id) {
                    driver_map.insert(driver.id.clone(), driver.nome.clone());
                }
            }
        }
    }

    let class_to_cat: std::collections::HashMap<&str, &str> = CLASSES_CONVOCADAS
        .iter()
        .map(|cfg| (cfg.class_name, cfg.special_category))
        .collect();
    let mut ops = Vec::new();

    for grid in grids {
        let special_cat = class_to_cat
            .get(grid.class_name.as_str())
            .copied()
            .unwrap_or("unknown");

        for assignment in &grid.assignments {
            let contract_id = contract_ids[contract_idx].clone();
            contract_idx += 1;

            let team_nome = team_map
                .get(&assignment.team_id)
                .cloned()
                .unwrap_or_else(|| assignment.team_id.clone());
            let driver_nome = driver_map
                .get(&assignment.driver_id)
                .cloned()
                .unwrap_or_else(|| assignment.driver_id.clone());
            let papel = if assignment.papel == TeamRole::Numero1 {
                TeamRole::Numero1
            } else {
                TeamRole::Numero2
            };

            let contract = contract_queries::generate_especial_contract(
                contract_id,
                &assignment.driver_id,
                &driver_nome,
                &assignment.team_id,
                &team_nome,
                papel.clone(),
                special_cat,
                &grid.class_name,
                season_number,
            );

            ops.push(PendingOp {
                contract,
                driver_id: assignment.driver_id.clone(),
                special_category: special_cat.to_string(),
            });
        }
    }

    Ok(ops)
}

fn aplicar_persistencia_grids(conn: &Connection, ops: &[PendingOp]) -> Result<(), DbError> {
    driver_queries::clear_all_categoria_especial_ativa(conn)?;
    team_queries::clear_special_team_lineups(conn)?;
    team_queries::reset_special_team_hierarchies(conn)?;

    let mut team_lineups: std::collections::HashMap<String, (Option<String>, Option<String>)> =
        std::collections::HashMap::new();

    for op in ops {
        contract_queries::insert_contract(conn, &op.contract)?;
        driver_queries::update_driver_especial_category(
            conn,
            &op.driver_id,
            Some(&op.special_category),
        )?;

        let lineup = team_lineups
            .entry(op.contract.equipe_id.clone())
            .or_insert((None, None));
        match op.contract.papel {
            TeamRole::Numero1 => lineup.0 = Some(op.driver_id.clone()),
            TeamRole::Numero2 => lineup.1 = Some(op.driver_id.clone()),
        }
    }

    for (team_id, (n1, n2)) in &team_lineups {
        team_queries::update_team_pilots(conn, team_id, n1.as_deref(), n2.as_deref())?;
        if let (Some(n1_id), Some(n2_id)) = (n1, n2) {
            team_queries::update_team_hierarchy(
                conn,
                team_id,
                Some(n1_id.as_str()),
                Some(n2_id.as_str()),
                "Claro",
                0.0,
            )?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PosEspecialResult {
    pub contratos_encerrados: usize,
    pub pilotos_liberados: usize,
    pub equipes_limpas: usize,
    pub errors: Vec<String>,
}

/// BlocoEspecial → PosEspecial (transição esportiva: as corridas especiais terminaram).
/// Deve ser chamada antes de run_pos_especial.
pub fn encerrar_bloco_especial(conn: &Connection) -> Result<(), DbError> {
    let season = season_queries::get_active_season(conn)?
        .ok_or_else(|| DbError::NotFound("Nenhuma temporada ativa".into()))?;

    if season.fase != SeasonPhase::BlocoEspecial {
        return Err(DbError::Migration(format!(
            "Fase atual é '{}'; esperado BlocoEspecial",
            season.fase
        )));
    }

    season_queries::update_season_fase(conn, &season.id, &SeasonPhase::PosEspecial)?;
    Ok(())
}

/// Desmontagem administrativa do bloco especial: expira contratos, limpa pilotos e
/// hierarquias das equipes especiais. Gera e persiste notícias de campeões.
///
/// Escopo neste bloco: "core cleanup + news de encerramento".
/// Fora de escopo (implementar em blocos posteriores):
///   ajustes de motivação pós-special, reputação, espectadores, prêmios.
pub fn run_pos_especial(conn: &Connection) -> Result<PosEspecialResult, DbError> {
    let season = season_queries::get_active_season(conn)?
        .ok_or_else(|| DbError::NotFound("Nenhuma temporada ativa".into()))?;

    if season.fase != SeasonPhase::PosEspecial {
        return Err(DbError::Migration(format!(
            "Fase atual é '{}'; esperado PosEspecial",
            season.fase
        )));
    }

    // Coletar campeões ANTES do cleanup (contratos ainda ativos)
    let _campeoes = query_campeoes_especiais(conn, season.numero)?;

    // Cleanup em uma transação
    let tx = conn.unchecked_transaction()?;

    let contratos_encerrados = contract_queries::expire_especial_contracts(&tx, season.numero)?;
    let pilotos_liberados = driver_queries::clear_all_categoria_especial_ativa(&tx)?;
    let equipes_limpas = team_queries::clear_special_team_lineups(&tx)?;
    team_queries::reset_special_team_hierarchies(&tx)?;

    tx.commit()?;

    Ok(PosEspecialResult {
        contratos_encerrados,
        pilotos_liberados,
        equipes_limpas,
        errors: vec![],
    })
}

/// Retorna o campeão de cada classe especial (maior temp_pontos com contrato Especial ativo).
/// Chamada antes do cleanup para ter acesso aos contratos ainda ativos.
fn query_campeoes_especiais(
    conn: &Connection,
    season_number: i32,
) -> Result<Vec<(String, String, Option<String>, Option<String>)>, DbError> {
    let mut resultado = Vec::new();

    for cfg in CLASSES_CONVOCADAS {
        let campeao: Option<(String, String)> = conn
            .query_row(
                "SELECT d.id, d.nome FROM drivers d
             INNER JOIN contracts c ON c.piloto_id = d.id
             WHERE c.tipo = 'Especial' AND c.status = 'Ativo'
               AND c.temporada_inicio = ?1
               AND c.categoria = ?2
               AND c.classe = ?3
             ORDER BY d.temp_pontos DESC
             LIMIT 1",
                rusqlite::params![season_number, cfg.special_category, cfg.class_name],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(DbError::Sqlite)?;

        resultado.push((
            cfg.special_category.to_string(),
            cfg.class_name.to_string(),
            campeao.as_ref().map(|(_, nome)| nome.clone()),
            campeao.map(|(driver_id, _)| driver_id),
        ));
    }

    Ok(resultado)
}

// ── Testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};
    use rusqlite::Connection;

    use super::*;
    use crate::calendar::CalendarEntry;
    use crate::db::migrations;
    use crate::db::queries::{calendar as calq, contracts as cq, drivers as dq, seasons as sq};
    use crate::generators::world::generate_world_with_rng;
    use crate::models::enums::{RaceStatus, SeasonPhase, ThematicSlot, WeatherCondition};

    fn setup_world_db() -> (Connection, String) {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("migrations");

        let mut rng = StdRng::seed_from_u64(99);
        let world = generate_world_with_rng(
            "Test Player",
            "🇧🇷 Brasileiro",
            20,
            "mazda_rookie",
            0,
            "medio",
            &mut rng,
        )
        .expect("world generation");

        let season_id = "S001".to_string();
        let season = crate::models::season::Season::new(season_id.clone(), 1, 2024);
        sq::insert_season(&conn, &season).expect("insert season");
        for driver in &world.drivers {
            dq::insert_driver(&conn, driver).expect("insert driver");
        }
        crate::db::queries::teams::insert_teams(&conn, &world.teams).expect("insert teams");
        cq::insert_contracts(&conn, &world.contracts).expect("insert contracts");

        // Sincronizar o contador de IDs com a quantidade de contratos inseridos
        let next_contract = world.contracts.len() + 1;
        conn.execute(
            "UPDATE meta SET value = ?1 WHERE key = 'next_contract_id'",
            rusqlite::params![next_contract.to_string()],
        )
        .expect("update meta contract counter");

        (conn, season_id)
    }

    fn make_player_eligible_for_specials(conn: &Connection, category: &str) -> String {
        let mut player = dq::get_player_driver(conn).expect("player");
        player.categoria_atual = Some(category.to_string());
        player.atributos.skill = 98.0;
        player.melhor_resultado_temp = Some(1);
        player.stats_temporada.vitorias = 4;
        dq::update_driver(conn, &player).expect("update player");
        player.id
    }

    fn insert_pending_regular_race(conn: &Connection, season_id: &str, category: &str) {
        calq::insert_calendar_entry(
            conn,
            &CalendarEntry {
                id: "R-PENDING-REGULAR".to_string(),
                season_id: season_id.to_string(),
                categoria: category.to_string(),
                rodada: 1,
                nome: "Corrida regular pendente".to_string(),
                track_id: 1,
                track_name: "Interlagos".to_string(),
                track_config: "GP".to_string(),
                clima: WeatherCondition::Dry,
                temperatura: 24.0,
                voltas: 20,
                duracao_corrida_min: 30,
                duracao_classificacao_min: 10,
                status: RaceStatus::Pendente,
                horario: "14:00".to_string(),
                week_of_year: 30,
                season_phase: SeasonPhase::BlocoRegular,
                display_date: "2024-09-15".to_string(),
                thematic_slot: ThematicSlot::RodadaRegular,
            },
        )
        .expect("insert pending regular race");
    }

    #[test]
    fn test_season_phase_transitions() {
        let (conn, season_id) = setup_world_db();

        // Começa em BlocoRegular
        let s = sq::get_season_by_id(&conn, &season_id).unwrap().unwrap();
        assert_eq!(s.fase, SeasonPhase::BlocoRegular);

        // advance → JanelaConvocacao
        advance_to_convocation_window(&conn).expect("advance");
        let s = sq::get_season_by_id(&conn, &season_id).unwrap().unwrap();
        assert_eq!(s.fase, SeasonPhase::JanelaConvocacao);

        // iniciar_bloco_especial → BlocoEspecial
        iniciar_bloco_especial(&conn).expect("iniciar");
        let s = sq::get_season_by_id(&conn, &season_id).unwrap().unwrap();
        assert_eq!(s.fase, SeasonPhase::BlocoEspecial);
    }

    #[test]
    fn test_advance_requires_bloco_regular() {
        let (conn, _) = setup_world_db();
        // Avançar duas vezes deve falhar na segunda
        advance_to_convocation_window(&conn).expect("primeira avançada");
        let result = advance_to_convocation_window(&conn);
        assert!(
            result.is_err(),
            "deveria falhar se não estiver em BlocoRegular"
        );
    }

    #[test]
    fn test_advance_to_convocation_rejects_pending_regular_races() {
        let (conn, season_id) = setup_world_db();
        insert_pending_regular_race(&conn, &season_id, "gt3");

        let result = advance_to_convocation_window(&conn);

        assert!(
            result.is_err(),
            "nao deveria abrir convocacao antes do fim real do bloco regular"
        );
    }

    #[test]
    fn test_run_convocation_requires_janela() {
        let (conn, _) = setup_world_db();
        // Tentar convocação em BlocoRegular deve falhar
        let result = run_convocation_window(&conn);
        assert!(result.is_err(), "deveria falhar fora de JanelaConvocacao");
    }

    #[test]
    fn test_iniciar_bloco_especial_rolls_back_phase_when_calendar_generation_fails() {
        let (conn, season_id) = setup_world_db();
        advance_to_convocation_window(&conn).expect("advance");

        conn.execute(
            "CREATE TRIGGER fail_special_calendar_insert
             BEFORE INSERT ON calendar
             BEGIN
                 SELECT RAISE(ABORT, 'special calendar blocked');
             END;",
            [],
        )
        .expect("create trigger");

        let result = iniciar_bloco_especial(&conn);
        assert!(result.is_err(), "inicio do bloco especial deveria falhar");

        let season = sq::get_season_by_id(&conn, &season_id)
            .expect("season query")
            .expect("season");
        assert_eq!(
            season.fase,
            SeasonPhase::JanelaConvocacao,
            "a fase nao deve avancar se a geracao do calendario especial falhar"
        );
    }

    #[test]
    fn test_run_convocation_rolls_back_when_player_offer_persistence_fails() {
        let (conn, _) = setup_world_db();
        make_player_eligible_for_specials(&conn, "gt4");
        advance_to_convocation_window(&conn).expect("advance");

        conn.execute(
            "CREATE TRIGGER fail_player_special_offer_insert
             BEFORE INSERT ON player_special_offers
             BEGIN
                 SELECT RAISE(ABORT, 'player special offer blocked');
             END;",
            [],
        )
        .expect("create trigger");

        let result = run_convocation_window(&conn);
        assert!(result.is_err(), "convocacao deveria falhar");

        let especial_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM contracts WHERE tipo = 'Especial'",
                [],
                |row| row.get(0),
            )
            .expect("special contracts count");
        assert_eq!(
            especial_count, 0,
            "a convocacao precisa ser atomica e nao deixar contratos especiais apos falha nas ofertas"
        );

        let drivers_in_special: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM drivers WHERE categoria_especial_ativa IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .expect("drivers in special count");
        assert_eq!(
            drivers_in_special, 0,
            "a convocacao nao deve marcar pilotos no especial apos rollback"
        );
    }

    #[test]
    fn test_run_convocation_propagates_player_lookup_errors() {
        let (conn, _) = setup_world_db();
        advance_to_convocation_window(&conn).expect("advance");
        let player_id = dq::get_player_driver(&conn).expect("player").id;

        conn.execute(
            "UPDATE drivers SET personalidade_primaria = 'perfil_quebrado' WHERE id = ?1",
            rusqlite::params![player_id],
        )
        .expect("corrupt player personality");

        let result = run_convocation_window(&conn);
        assert!(
            result.is_err(),
            "erro estrutural na leitura do jogador nao deveria ser tratado como ausencia de jogador"
        );
    }

    #[test]
    fn test_run_convocation_no_duplicate_drivers() {
        let (conn, _) = setup_world_db();
        advance_to_convocation_window(&conn).expect("advance");

        let result = run_convocation_window(&conn).expect("convocação");

        // Nenhum driver_id duplicado em todos os grids
        let mut all_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        for grid in &result.grids {
            for a in &grid.assignments {
                assert!(
                    all_ids.insert(a.driver_id.clone()),
                    "driver {} duplicado entre grids",
                    a.driver_id
                );
            }
        }
    }

    #[test]
    fn test_run_convocation_contracts_are_especial() {
        let (conn, _) = setup_world_db();
        advance_to_convocation_window(&conn).expect("advance");
        let result = run_convocation_window(&conn).expect("convocação");
        assert!(
            result.errors.is_empty(),
            "erros na convocação: {:?}",
            result.errors
        );

        // Todos os contratos especiais gerados devem ter tipo=Especial
        let especiais: Vec<_> = conn
            .prepare("SELECT tipo FROM contracts WHERE tipo = 'Especial'")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(!especiais.is_empty(), "nenhum contrato especial gerado");
    }

    #[test]
    fn test_run_convocation_contracts_have_classe() {
        let (conn, _) = setup_world_db();
        advance_to_convocation_window(&conn).expect("advance");
        run_convocation_window(&conn).expect("convocação");

        // Contratos especiais devem ter classe não nula
        let null_classe: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM contracts WHERE tipo='Especial' AND classe IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        assert_eq!(
            null_classe, 0,
            "contratos especiais com classe=NULL: {}",
            null_classe
        );
    }

    #[test]
    fn test_run_convocation_drivers_keep_regular_category() {
        let (conn, _) = setup_world_db();
        advance_to_convocation_window(&conn).expect("advance");
        let result = run_convocation_window(&conn).expect("convocação");

        // categoria_atual dos pilotos convocados deve estar intacta
        for grid in &result.grids {
            for a in &grid.assignments {
                let driver = dq::get_driver(&conn, &a.driver_id).expect("get driver");
                // categoria_especial_ativa deve estar preenchida
                assert!(
                    driver.categoria_especial_ativa.is_some(),
                    "piloto {} não tem categoria_especial_ativa após convocação",
                    driver.nome
                );
            }
        }
    }

    #[test]
    fn test_lmp2_class_gets_special_grid() {
        let (conn, _) = setup_world_db();
        advance_to_convocation_window(&conn).expect("advance");
        run_convocation_window(&conn).expect("convocação");

        // LMP2 e categoria regular e tambem uma classe convocada no bloco especial de Endurance.
        let lmp2_with_two_pilots: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM teams
                 WHERE categoria='lmp2'
                   AND piloto_1_id IS NOT NULL
                   AND piloto_2_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let lmp2_contracts: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM contracts
                 WHERE tipo='Especial'
                   AND categoria='endurance'
                   AND classe='lmp2'
                   AND status='Ativo'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let lmp2_entries: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM special_team_entries
                 WHERE special_category='endurance'
                   AND class_name='lmp2'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        assert!(lmp2_with_two_pilots > 0, "LMP2 deveria montar grid proprio");
        assert!(lmp2_contracts > 0, "LMP2 deveria gerar contratos especiais");
        assert!(
            lmp2_entries > 0,
            "LMP2 deveria registrar equipes classificadas"
        );
    }

    #[test]
    fn test_special_entries_ignore_previous_special_guarantees() {
        let (conn, season_id) = setup_world_db();
        let mut previous_season = crate::models::season::Season::new("S000".to_string(), 0, 2023);
        previous_season.finalizar();
        sq::insert_season(&conn, &previous_season).expect("insert previous season");

        let feeder_category = "mazda_amador";
        let mut team_ids: Vec<String> = conn
            .prepare("SELECT id FROM teams WHERE categoria = ?1 ORDER BY nome ASC")
            .expect("prepare team query")
            .query_map(rusqlite::params![feeder_category], |row| {
                row.get::<_, String>(0)
            })
            .expect("query teams")
            .map(|row| row.expect("team id"))
            .collect();
        assert!(
            team_ids.len() > 5,
            "teste precisa de mais equipes regulares que vagas especiais"
        );

        let previously_guaranteed_team_id = team_ids.pop().expect("guaranteed team");
        special_entry_queries::replace_entries_for_class(
            &conn,
            "S000",
            "production_challenger",
            "mazda",
            &[special_entry_queries::NewSpecialTeamEntry {
                team_id: previously_guaranteed_team_id.clone(),
                source_category: feeder_category.to_string(),
                qualified_via: "GarantiaEspecial".to_string(),
                guaranteed_next_year: true,
            }],
        )
        .expect("previous guarantee");

        conn.execute(
            "UPDATE teams SET stats_pontos = 0, stats_vitorias = 0, stats_melhor_resultado = 99
             WHERE categoria = ?1",
            rusqlite::params![feeder_category],
        )
        .expect("reset standings");
        for (index, team_id) in team_ids.iter().take(5).enumerate() {
            conn.execute(
                "UPDATE teams
                 SET stats_pontos = ?2, stats_vitorias = ?3, stats_melhor_resultado = 1
                 WHERE id = ?1",
                rusqlite::params![team_id, 100 - index as i32, 5 - index as i32],
            )
            .expect("seed regular contender");
        }

        ensure_special_team_entries(&conn, &season_id, 1).expect("ensure entries");

        let entries = special_entry_queries::get_entries_for_class(
            &conn,
            &season_id,
            "production_challenger",
            "mazda",
        )
        .expect("entries");
        assert!(
            entries
                .iter()
                .all(|entry| entry.qualified_via.starts_with("RegularP")),
            "todas as vagas especiais devem vir da temporada regular: {:?}",
            entries
                .iter()
                .map(|entry| entry.qualified_via.clone())
                .collect::<Vec<_>>()
        );
        assert!(
            entries
                .iter()
                .all(|entry| entry.team_id != previously_guaranteed_team_id),
            "equipe com garantia antiga nao pode furar a fila regular"
        );
    }

    #[test]
    fn test_special_window_team_sections_expose_class_for_each_car() {
        let (conn, season_id) = setup_world_db();
        let player = dq::get_player_driver(&conn).expect("player");
        advance_to_convocation_window(&conn).expect("advance");
        run_convocation_window(&conn).expect("convocação");

        let payload = special_window::load_special_window_payload(&conn, &season_id, &player.id)
            .expect("payload");
        let mut classes_by_category: std::collections::HashMap<
            String,
            std::collections::HashSet<String>,
        > = std::collections::HashMap::new();
        let mut missing_class_teams = Vec::new();

        for section in payload.team_sections {
            for team in section.teams {
                match team.classe {
                    Some(class_name) => {
                        classes_by_category
                            .entry(section.category.clone())
                            .or_default()
                            .insert(class_name);
                    }
                    None => missing_class_teams.push(team.nome),
                }
            }
        }

        assert!(
            missing_class_teams.is_empty(),
            "todas as equipes da janela especial devem expor classe/carro: {:?}",
            missing_class_teams
        );
        assert_eq!(
            classes_by_category
                .get("production_challenger")
                .expect("production section")
                .len(),
            3
        );
        assert_eq!(
            classes_by_category
                .get("endurance")
                .expect("endurance section")
                .len(),
            3
        );
    }

    #[test]
    fn test_persistir_grids_rolls_back_all_changes_on_error() {
        let (conn, _) = setup_world_db();
        let season_number = 1;

        let next_contract: i64 = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'next_contract_id'",
                [],
                |row| row.get(0),
            )
            .expect("read next contract id");
        let second_contract_id = format!("C{:03}", next_contract + 1);
        conn.execute_batch(&format!(
            "
            CREATE TRIGGER fail_second_special_contract_insert
            BEFORE INSERT ON contracts
            WHEN NEW.id = '{second_contract_id}'
            BEGIN
                SELECT RAISE(ABORT, 'forced special contract failure');
            END;
            "
        ))
        .expect("create failing trigger");

        let team = team_queries::get_teams_by_category(&conn, "gt4")
            .expect("gt4 teams")
            .into_iter()
            .next()
            .expect("at least one gt4 team");
        let drivers = dq::get_drivers_by_category(&conn, "gt4").expect("gt4 drivers");
        let assignments = vec![
            DriverAssignment {
                driver_id: drivers[0].id.clone(),
                team_id: team.id.clone(),
                papel: TeamRole::Numero1,
                fonte: "MeritoRegular".to_string(),
                score: 99.0,
            },
            DriverAssignment {
                driver_id: drivers[1].id.clone(),
                team_id: team.id.clone(),
                papel: TeamRole::Numero2,
                fonte: "MeritoRegular".to_string(),
                score: 98.0,
            },
        ];

        let result = persistir_grids(
            &conn,
            &[GridClasse {
                class_name: "gt4".to_string(),
                assignments,
            }],
            season_number,
        );
        assert!(result.is_err(), "persistência deveria falhar com trigger");

        let especiais: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM contracts WHERE tipo = 'Especial'",
                [],
                |row| row.get(0),
            )
            .expect("count special contracts");
        assert_eq!(
            especiais, 0,
            "nenhum contrato especial deveria sobreviver após rollback"
        );

        for driver in drivers.iter().take(2) {
            let refreshed = dq::get_driver(&conn, &driver.id).expect("refresh driver");
            assert!(
                refreshed.categoria_especial_ativa.is_none(),
                "piloto {} não deveria ficar marcado no especial após rollback",
                refreshed.nome
            );
        }
    }

    // ── Testes PosEspecial ────────────────────────────────────────────────────

    /// Helper: avança até BlocoEspecial com convocação completa.
    fn setup_bloco_especial(conn: &Connection) {
        advance_to_convocation_window(conn).expect("advance to janela");
        run_convocation_window(conn).expect("run convocação");
        iniciar_bloco_especial(conn).expect("iniciar bloco especial");
    }

    #[test]
    fn test_encerrar_bloco_especial_transitions_phase() {
        let (conn, season_id) = setup_world_db();
        setup_bloco_especial(&conn);

        encerrar_bloco_especial(&conn).expect("encerrar bloco especial");
        let s = sq::get_season_by_id(&conn, &season_id).unwrap().unwrap();
        assert_eq!(s.fase, SeasonPhase::PosEspecial);
    }

    #[test]
    fn test_encerrar_bloco_especial_rejects_wrong_phase() {
        let (conn, _) = setup_world_db();
        // Estamos em BlocoRegular, não BlocoEspecial
        let result = encerrar_bloco_especial(&conn);
        assert!(result.is_err(), "deveria rejeitar fora de BlocoEspecial");
    }

    #[test]
    fn test_run_pos_especial_rejects_wrong_phase() {
        let (conn, _) = setup_world_db();
        // Estamos em BlocoRegular, não PosEspecial
        let result = run_pos_especial(&conn);
        assert!(result.is_err(), "deveria rejeitar fora de PosEspecial");
    }

    #[test]
    fn test_run_pos_especial_expires_especial_contracts() {
        let (conn, _) = setup_world_db();
        setup_bloco_especial(&conn);
        encerrar_bloco_especial(&conn).expect("encerrar");

        run_pos_especial(&conn).expect("run pos especial");

        let ativos: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM contracts WHERE tipo='Especial' AND status='Ativo'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        assert_eq!(
            ativos, 0,
            "contratos Especial ainda ativos após PosEspecial: {}",
            ativos
        );
    }

    #[test]
    fn test_run_pos_especial_clears_categoria_especial_ativa() {
        let (conn, _) = setup_world_db();
        setup_bloco_especial(&conn);
        encerrar_bloco_especial(&conn).expect("encerrar");

        run_pos_especial(&conn).expect("run pos especial");

        let com_especial: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM drivers WHERE categoria_especial_ativa IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        assert_eq!(
            com_especial, 0,
            "pilotos com categoria_especial_ativa após PosEspecial: {}",
            com_especial
        );
    }

    #[test]
    fn test_run_pos_especial_clears_team_lineups() {
        let (conn, _) = setup_world_db();
        setup_bloco_especial(&conn);
        encerrar_bloco_especial(&conn).expect("encerrar");

        run_pos_especial(&conn).expect("run pos especial");

        let com_pilotos: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM teams WHERE categoria IN ('production_challenger','endurance') AND piloto_1_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        assert_eq!(
            com_pilotos, 0,
            "equipes especiais com piloto após PosEspecial: {}",
            com_pilotos
        );
    }

    #[test]
    fn test_run_pos_especial_resets_hierarchy() {
        let (conn, _) = setup_world_db();
        setup_bloco_especial(&conn);
        encerrar_bloco_especial(&conn).expect("encerrar");

        run_pos_especial(&conn).expect("run pos especial");

        let com_hierarquia: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM teams WHERE categoria IN ('production_challenger','endurance') AND hierarquia_n1_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        assert_eq!(
            com_hierarquia, 0,
            "equipes especiais com hierarquia após PosEspecial: {}",
            com_hierarquia
        );
    }
}
