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
mod grid;
mod ofertas;
mod persistencia;
mod pos_especial;
mod validacao;

// O glob é `pub` onde há caminho público a preservar: `convocation::pipeline::…`
// (e o re-export em `convocation/mod.rs`) continua resolvendo igual.
pub use pos_especial::*;
use grid::*;
use ofertas::*;
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
