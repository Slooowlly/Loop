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

// Etapas do bloco especial. Este arquivo guarda só a orquestração de alto nível
// (`run_convocation_window`); cada etapa mora no seu módulo e enxerga os imports
// acima via `use super::*`.
mod persistencia;
mod pos_especial;
mod validacao;

// O glob é `pub` onde há caminho público a preservar: `convocation::pipeline::…`
// (e o re-export em `convocation/mod.rs`) continua resolvendo igual.
pub use pos_especial::*;
use persistencia::*;
use validacao::*;

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
        feeder_category: "endurance",
    },
];

fn uses_regular_special_event_grid(category: &str) -> bool {
    matches!(category, "production_challenger" | "endurance")
}

fn legacy_convocation_classes() -> impl Iterator<Item = &'static ClasseConfig> {
    CLASSES_CONVOCADAS
        .iter()
        .filter(|cfg| !uses_regular_special_event_grid(cfg.special_category))
}

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

    for cfg in legacy_convocation_classes() {
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
    for cfg in legacy_convocation_classes() {
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

    for cfg in legacy_convocation_classes() {
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
                    priority + (team.car_strength() * 0.16).round() as i32,
                    team.id,
                    team.nome,
                    cfg.special_category.to_string(),
                    cfg.class_name.to_string(),
                ));
                continue;
            }

            if license_ok && quality_score >= fallback_quality_threshold(cfg) {
                fallback.push((
                    100 + (team.car_strength() * 0.16).round() as i32,
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

#[cfg(test)]
#[path = "pipeline/tests/mod.rs"]
mod tests;
