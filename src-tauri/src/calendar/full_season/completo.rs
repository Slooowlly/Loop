//! Gerador do calendário 9D completo (todas as 9 divisões da temporada).

use std::collections::{HashMap, HashSet};

use rand::{rngs::StdRng, SeedableRng};
use rusqlite::Connection;

use crate::calendar::CalendarEntry;
use crate::constants::categories::has_calendar_conflict;
use crate::db::connection::DbError;
use crate::db::queries::calendar as cal_queries;
use crate::generators::ids::{next_ids, IdType};
use crate::models::enums::SeasonPhase;

// ── Janelas de semana por categoria ──────────────────────────────────────────
//
// Formato: (category_id, woy_start, woy_end)
//
// Conversão: season_week = week_of_year + 4  (para woy 1–47 → sw 5–51)
//   woy 6  = sw 10   woy 47 = sw 51
//   woy 46 = sw 50   woy 45 = sw 49   woy  7 = sw 11
//
// Pares conflitantes (Mazda/Toyota) usam o mesmo intervalo deslocado por 1 semana
// (start 6 vs 7, end 38 vs 39 / 40 vs 41) → conjuntos de season_week disjuntos ✓.
//
// Escalonamento por prestígio (todos abrem em sw 10–13; o final é que muda):
//   rookie    → sw 42/43   (termina antes)
//   cup       → sw 44/45
//   bmw       → sw 46
//   production→ sw 48   gt4 → sw 49   endurance → sw 50   gt3 → sw 51 (última)
//   endurance (sw 50) ≠ gt3 (sw 51).

const FULL_SEASON_WINDOWS: [(&str, i32, i32); 9] = [
    ("mazda_rookie", 6, 38),
    ("toyota_rookie", 7, 39),
    ("mazda_amador", 6, 40),
    ("toyota_amador", 7, 41),
    ("bmw_m2", 6, 42),
    ("production_challenger", 6, 44),
    ("gt4", 6, 45),
    ("endurance", 6, 46),
    ("gt3", 6, 47),
];

/// Gera o calendário unificado 9D para todas as 9 divisões da temporada.
///
/// Retorna exatamente 74 entradas (5+5+8+8+8+10+10+14+6), todas com
/// `season_phase = Temporada` e `season_week = Some(10..=51)`.
///
/// Função pura: sem I/O. Totalmente determinística para o mesmo `seed`.
pub fn build_full_season_calendar(
    season_id: &str,
    season_year: i32,
    seed: u64,
) -> Result<Vec<CalendarEntry>, String> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut all_entries: Vec<CalendarEntry> = Vec::with_capacity(74);
    let mut next_id = 1_u32;
    let mut id_gen = || {
        let id = format!("FS{:05}", next_id);
        next_id += 1;
        id
    };

    // Mantém as entradas já geradas para resolução de conflitos de pistas.
    let mut by_category: HashMap<String, Vec<CalendarEntry>> = HashMap::new();

    for &(category_id, woy_start, woy_end) in &FULL_SEASON_WINDOWS {
        // Montar banned_tracks_by_round a partir do parceiro conflitante (se houver).
        let banned: HashMap<i32, HashSet<u32>> = by_category
            .iter()
            .filter(|(other_id, _)| has_calendar_conflict(category_id, other_id.as_str()))
            .flat_map(|(_, entries)| entries.iter())
            .fold(HashMap::new(), |mut acc, entry| {
                acc.entry(entry.rodada).or_default().insert(entry.track_id);
                acc
            });

        let mut entries = crate::calendar::generate_calendar_for_category_with_constraints(
            season_id,
            season_year,
            category_id,
            woy_start,
            woy_end,
            SeasonPhase::Temporada,
            &banned,
            &mut id_gen,
            &mut rng,
        )?;

        for entry in &mut entries {
            // Janela de corridas: woy 1–47 → sw = woy + 4 (faixa 5–51).
            // Na janela de corridas woy 6–47 → sw 10–51.
            let sw = entry.week_of_year + 4;
            if !(10..=51).contains(&sw) {
                return Err(format!(
                    "season_week {sw} fora do intervalo válido (10–51) para {category_id} \
                     (week_of_year={})",
                    entry.week_of_year
                ));
            }
            entry.season_week = Some(sw as u32);
        }

        by_category.insert(category_id.to_string(), entries.clone());
        all_entries.extend(entries);
    }

    Ok(all_entries)
}

/// Gera e persiste o calendário 9D completo para a temporada indicada.
///
/// IDs são alocados via sequenciador do DB (IdType::Race).
/// Retorna o número de entradas inseridas (deve ser 74).
pub fn generate_full_season_calendar(
    conn: &Connection,
    season_id: &str,
    season_year: i32,
    seed: u64,
) -> Result<usize, DbError> {
    let mut entries =
        build_full_season_calendar(season_id, season_year, seed).map_err(DbError::InvalidData)?;

    let ids = next_ids(conn, IdType::Race, entries.len() as u32)?;
    for (entry, id) in entries.iter_mut().zip(ids) {
        entry.id = id;
    }

    cal_queries::insert_calendar_entries(conn, &entries)?;
    Ok(entries.len())
}
