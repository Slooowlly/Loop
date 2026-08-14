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
use crate::licensing::driver_has_required_license_for_category;
use crate::models::driver::Driver;
use crate::models::enums::{SeasonPhase, TeamRole};
use crate::promotion::standings::calculate_constructor_standings;

use super::eligibility::{coletar_candidatos, FonteConvocacao};
use super::player_offers::{self, PlayerSpecialOffer};
use super::quotas::calcular_cotas;
use super::scoring::calcular_score;
use super::special_window;

// Etapas do bloco especial. Este arquivo guarda só a orquestração de alto nível
// (`run_convocation_window`); cada etapa mora no seu módulo e enxerga os imports
// acima via `use super::*`.
mod comum;
mod fases;
mod grid;
mod ofertas;
mod persistencia;
mod pos_especial;
mod validacao;

// O glob é `pub` onde há caminho público a preservar: `convocation::pipeline::…`
// (e o re-export em `convocation/mod.rs`) continua resolvendo igual.
pub use comum::*;
pub use fases::*;
use grid::*;
use ofertas::*;
use persistencia::*;
pub use pos_especial::*;
use validacao::*;

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

#[cfg(test)]
#[path = "pipeline/tests/mod.rs"]
mod tests;
