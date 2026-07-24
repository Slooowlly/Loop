//! Gerador parcial (Etapa 10 / migração v34).
//!
//! Produz APENAS production_challenger e endurance para a janela restante do ano.
//! Janelas canônicas (mesmo offset do gerador completo):
//!   production_challenger: woy_end = 47  →  sw_end = 51
//!   endurance:             woy_end = 45  →  sw_end = 49
//!
//! Regra de degradação (aplicada independentemente a cada divisão):
//!   (a) contagem-alvo (10 prod / 6 end) + espaçamento canônico (prod≥3 sw, end≥5 sw);
//!   (b) contagem mínima (3 prod / 2 end) + espaçamento canônico;
//!   (c) contagem mínima + espaçamento relaxado (prod≥2 sw, end≥3 sw);
//!   (d) o que couber com espaçamento relaxado (inclusive zero) + aviso em stderr.
//!
//! "Cabe" significa: span ≥ min_spacing × (N − 1), onde span = to_woy − from_woy.

use std::collections::HashMap;

use rand::{rngs::StdRng, SeedableRng};
use rusqlite::Connection;

use crate::calendar::CalendarEntry;
use crate::db::connection::DbError;
use crate::db::queries::calendar as cal_queries;
use crate::generators::ids::{next_ids, IdType};
use crate::models::enums::SeasonPhase;

const PARTIAL_PROD_WOY_END: i32 = 47; // = sw 51
const PARTIAL_END_WOY_END: i32 = 45; // = sw 49

/// `true` se N rounds cabem em [from_woy, to_woy] com o espaçamento mínimo dado.
fn partial_fits(n: usize, from_woy: i32, to_woy: i32, min_spacing: i32) -> bool {
    if to_woy < from_woy {
        return n == 0;
    }
    if n <= 1 {
        return true;
    }
    (to_woy - from_woy) >= min_spacing * (n as i32 - 1)
}

/// Número máximo de rounds que cabem em [from_woy, to_woy] com min_spacing.
fn partial_max_count(from_woy: i32, to_woy: i32, min_spacing: i32) -> usize {
    if to_woy < from_woy {
        return 0;
    }
    1 + ((to_woy - from_woy) / min_spacing) as usize
}

/// Determina quantas corridas gerar aplicando a regra de degradação documentada acima.
/// Retorna (count, deve_avisar).
pub(super) fn partial_compute_count(
    from_woy: i32,
    to_woy: i32,
    target: usize,
    minimum: usize,
    canon_spacing: i32,
    relax_spacing: i32,
) -> (usize, bool) {
    if partial_fits(target, from_woy, to_woy, canon_spacing) {
        return (target, false);
    }
    if partial_fits(minimum, from_woy, to_woy, canon_spacing) {
        return (minimum, false);
    }
    if partial_fits(minimum, from_woy, to_woy, relax_spacing) {
        return (minimum, false);
    }
    let n = partial_max_count(from_woy, to_woy, relax_spacing);
    (n, true)
}

/// Gera entradas parciais para production_challenger e endurance nas semanas
/// restantes `[max(from_season_week, 10), 51]`.
///
/// Função pura e determinística para o mesmo `seed`.
/// IDs são placeholders sequenciais (PS00001…); o wrapper DB substitui pelos IDs reais.
pub(crate) fn build_partial_special_divisions(
    season_id: &str,
    season_year: i32,
    from_season_week: u32,
    seed: u64,
) -> Result<Vec<CalendarEntry>, String> {
    if season_id == "lmp2" {
        return Err("lmp2 não é uma divisão standalone".to_string());
    }

    let from_sw = (from_season_week as i32).max(10);
    let from_woy = from_sw - 4; // sw = woy + 4

    let mut rng = StdRng::seed_from_u64(seed);
    let mut all_entries: Vec<CalendarEntry> = Vec::new();
    let mut next_id = 1_u32;
    let mut id_gen = || {
        let id = format!("PS{:05}", next_id);
        next_id += 1;
        id
    };

    // ── Production Challenger ─────────────────────────────────────────────────
    let (prod_count, prod_warn) =
        partial_compute_count(from_woy, PARTIAL_PROD_WOY_END, 10, 3, 3, 2);
    if prod_warn {
        eprintln!(
            "[v34 partial] AVISO: production_challenger: {prod_count} rodada(s) na janela \
             sw {from_sw}–51 (mínimo esperado: 3)"
        );
    }
    if prod_count > 0 {
        let mut entries = crate::calendar::generate_calendar_for_category_with_count(
            season_id,
            season_year,
            "production_challenger",
            from_woy,
            PARTIAL_PROD_WOY_END,
            prod_count,
            SeasonPhase::Temporada,
            &HashMap::new(),
            &mut id_gen,
            &mut rng,
        )?;
        for entry in &mut entries {
            let sw = entry.week_of_year + 4;
            if !(10..=51).contains(&sw) {
                return Err(format!(
                    "season_week {sw} fora de 10–51 para production_challenger (parcial, \
                     week_of_year={})",
                    entry.week_of_year
                ));
            }
            entry.season_week = Some(sw as u32);
        }
        all_entries.extend(entries);
    }

    // ── Endurance ─────────────────────────────────────────────────────────────
    let (end_count, end_warn) = partial_compute_count(from_woy, PARTIAL_END_WOY_END, 6, 2, 5, 3);
    if end_warn {
        eprintln!(
            "[v34 partial] AVISO: endurance: {end_count} rodada(s) na janela \
             sw {from_sw}–49 (mínimo esperado: 2)"
        );
    }
    if end_count > 0 {
        let mut entries = crate::calendar::generate_calendar_for_category_with_count(
            season_id,
            season_year,
            "endurance",
            from_woy,
            PARTIAL_END_WOY_END,
            end_count,
            SeasonPhase::Temporada,
            &HashMap::new(),
            &mut id_gen,
            &mut rng,
        )?;
        for entry in &mut entries {
            let sw = entry.week_of_year + 4;
            if !(10..=49).contains(&sw) {
                return Err(format!(
                    "season_week {sw} fora de 10–49 para endurance (parcial, \
                     week_of_year={})",
                    entry.week_of_year
                ));
            }
            entry.season_week = Some(sw as u32);
        }
        all_entries.extend(entries);
    }

    Ok(all_entries)
}

/// Gera e persiste as divisões parciais para a temporada indicada.
///
/// IDs são alocados via sequenciador do DB (IdType::Race).
/// Retorna o número de entradas inseridas (0 quando a janela não comporta nenhuma corrida).
pub(crate) fn generate_partial_special_divisions(
    conn: &Connection,
    season_id: &str,
    season_year: i32,
    from_season_week: u32,
    seed: u64,
) -> Result<usize, DbError> {
    let mut entries =
        build_partial_special_divisions(season_id, season_year, from_season_week, seed)
            .map_err(DbError::InvalidData)?;

    if entries.is_empty() {
        return Ok(0);
    }

    let ids = next_ids(conn, IdType::Race, entries.len() as u32)?;
    for (entry, id) in entries.iter_mut().zip(ids) {
        entry.id = id;
    }

    cal_queries::insert_calendar_entries(conn, &entries)?;
    Ok(entries.len())
}
