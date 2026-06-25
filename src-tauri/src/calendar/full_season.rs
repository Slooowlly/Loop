//! Gerador de calendário unificado — Modelo 9D.
//!
//! Distribui as 9 divisões da carreira na régua season_week (1–51),
//! inteiramente dentro da janela de corridas sw 10–51 (woy 6–47, fev–nov).
//!
//! Regras garantidas por construção:
//! - Pares conflitantes nunca compartilham a mesma season_week.
//! - Abertura de cada divisão: sw 10–13.
//! - Final escalonado por prestígio: rookie/cup/bmw terminam antes; o topo
//!   (production/gt4/endurance/gt3) fecha o fim de novembro em sw 48–51.
//! - Finais de gt3 (sw 51) e endurance (sw 50) em semanas distintas.
//! - ThematicSlot somente do grupo regular (nunca *Especial).
//! - Todas as entradas com season_phase = Temporada.

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

// ── API pública ───────────────────────────────────────────────────────────────

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

        let mut entries = super::generate_calendar_for_category_with_constraints(
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

// ── Gerador parcial (Etapa 10 / migração v34) ─────────────────────────────────
//
// Produz APENAS production_challenger e endurance para a janela restante do ano.
// Janelas canônicas (mesmo offset do gerador completo):
//   production_challenger: woy_end = 47  →  sw_end = 51
//   endurance:             woy_end = 45  →  sw_end = 49
//
// Regra de degradação (aplicada independentemente a cada divisão):
//   (a) contagem-alvo (10 prod / 6 end) + espaçamento canônico (prod≥3 sw, end≥5 sw);
//   (b) contagem mínima (3 prod / 2 end) + espaçamento canônico;
//   (c) contagem mínima + espaçamento relaxado (prod≥2 sw, end≥3 sw);
//   (d) o que couber com espaçamento relaxado (inclusive zero) + aviso em stderr.
//
// "Cabe" significa: span ≥ min_spacing × (N − 1), onde span = to_woy − from_woy.

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
fn partial_compute_count(
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
        let mut entries = super::generate_calendar_for_category_with_count(
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
        let mut entries = super::generate_calendar_for_category_with_count(
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

// ── Testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rusqlite::Connection;

    use super::{
        build_full_season_calendar, build_partial_special_divisions, generate_full_season_calendar,
        generate_partial_special_divisions, partial_compute_count,
    };
    use crate::constants::categories::CALENDAR_CONFLICTS;
    use crate::db::migrations;
    use crate::db::queries::seasons::insert_season;
    use crate::models::enums::{SeasonPhase, ThematicSlot};
    use crate::models::season::Season;

    // ── Testes básicos (seed fixo) ────────────────────────────────────────────

    #[test]
    fn total_de_74_entradas() {
        let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
        assert_eq!(
            entries.len(),
            74,
            "full season deve ter exatamente 74 entradas (5+5+8+8+8+10+10+14+6)"
        );
    }

    #[test]
    fn contagem_por_categoria_correta() {
        let expected: &[(&str, usize)] = &[
            ("mazda_rookie", 5),
            ("toyota_rookie", 5),
            ("mazda_amador", 8),
            ("toyota_amador", 8),
            ("bmw_m2", 8),
            ("production_challenger", 10),
            ("gt4", 10),
            ("gt3", 14),
            ("endurance", 6),
        ];
        let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
        for &(cat, expected_count) in expected {
            let actual = entries.iter().filter(|e| e.categoria == cat).count();
            assert_eq!(
                actual, expected_count,
                "categoria {cat}: esperado {expected_count}, encontrado {actual}"
            );
        }
    }

    #[test]
    fn todas_as_entradas_com_season_phase_temporada() {
        let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
        for entry in &entries {
            assert_eq!(
                entry.season_phase,
                SeasonPhase::Temporada,
                "entry {} ({}) tem phase {:?}",
                entry.id,
                entry.categoria,
                entry.season_phase
            );
        }
    }

    #[test]
    fn season_week_definida_e_no_intervalo_10_a_51() {
        let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
        for entry in &entries {
            let sw = entry
                .season_week
                .unwrap_or_else(|| panic!("entry {} sem season_week", entry.id));
            assert!(
                (10..=51).contains(&sw),
                "entry {} ({}) tem season_week={sw} (fora de 10–51)",
                entry.id,
                entry.categoria
            );
        }
    }

    #[test]
    fn thematic_slot_somente_grupo_regular() {
        let special_slots = [
            ThematicSlot::AberturaEspecial,
            ThematicSlot::RodadaEspecial,
            ThematicSlot::FinalEspecial,
        ];
        let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
        for entry in &entries {
            assert!(
                !special_slots.contains(&entry.thematic_slot),
                "entry {} ({}) tem slot especial {:?}",
                entry.id,
                entry.categoria,
                entry.thematic_slot
            );
        }
    }

    #[test]
    fn abertura_em_sw_10_a_13() {
        let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
        let mut by_cat: HashMap<&str, Vec<_>> = HashMap::new();
        for entry in &entries {
            by_cat
                .entry(entry.categoria.as_str())
                .or_default()
                .push(entry);
        }
        for (cat, mut rounds) in by_cat {
            rounds.sort_by_key(|e| e.rodada);
            let first_sw = rounds[0].season_week.unwrap();
            assert!(
                (10..=13).contains(&first_sw),
                "categoria {cat}: abertura sw={first_sw} fora de 10–13"
            );
        }
    }

    #[test]
    fn final_escalonado_por_prestigio() {
        // Prestígio fecha o fim de novembro (sw 48–51); as demais terminam antes.
        let prestige = ["production_challenger", "gt4", "endurance", "gt3"];
        let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
        let mut by_cat: HashMap<&str, Vec<_>> = HashMap::new();
        for entry in &entries {
            by_cat
                .entry(entry.categoria.as_str())
                .or_default()
                .push(entry);
        }
        for (cat, mut rounds) in by_cat {
            rounds.sort_by_key(|e| e.rodada);
            let last_sw = rounds.last().unwrap().season_week.unwrap();
            if prestige.contains(&cat) {
                assert!(
                    (48..=51).contains(&last_sw),
                    "prestígio {cat}: final sw={last_sw} fora de 48–51"
                );
            } else {
                assert!(
                    (40..=47).contains(&last_sw),
                    "{cat}: final sw={last_sw} deveria terminar antes do prestígio (40–47)"
                );
            }
        }
    }

    #[test]
    fn conflict_pairs_sem_season_week_compartilhada() {
        let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
        for &(cat_a, cat_b) in &CALENDAR_CONFLICTS {
            let weeks_a: std::collections::HashSet<u32> = entries
                .iter()
                .filter(|e| e.categoria == cat_a)
                .map(|e| e.season_week.unwrap())
                .collect();
            let weeks_b: std::collections::HashSet<u32> = entries
                .iter()
                .filter(|e| e.categoria == cat_b)
                .map(|e| e.season_week.unwrap())
                .collect();
            let shared: Vec<u32> = weeks_a.intersection(&weeks_b).copied().collect();
            assert!(
                shared.is_empty(),
                "par ({cat_a}, {cat_b}) compartilha season_weeks: {shared:?}"
            );
        }
    }

    #[test]
    fn gt3_e_endurance_final_em_semanas_distintas() {
        let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
        let gt3_final = entries
            .iter()
            .filter(|e| e.categoria == "gt3")
            .max_by_key(|e| e.rodada)
            .map(|e| e.season_week.unwrap())
            .expect("gt3 entries");
        let end_final = entries
            .iter()
            .filter(|e| e.categoria == "endurance")
            .max_by_key(|e| e.rodada)
            .map(|e| e.season_week.unwrap())
            .expect("endurance entries");
        assert_ne!(
            gt3_final, end_final,
            "gt3 e endurance não podem ter o final na mesma semana"
        );
    }

    #[test]
    fn spacing_minimo_por_categoria() {
        let min_spacing = |cat: &str| -> u32 {
            match cat {
                "endurance" => 5,
                "gt3" => 2,
                "gt4" | "production_challenger" => 3,
                "mazda_amador" | "toyota_amador" | "bmw_m2" => 4,
                "mazda_rookie" | "toyota_rookie" => 6,
                _ => 1,
            }
        };
        let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
        let mut by_cat: HashMap<&str, Vec<u32>> = HashMap::new();
        for entry in &entries {
            by_cat
                .entry(entry.categoria.as_str())
                .or_default()
                .push(entry.season_week.unwrap());
        }
        for (cat, mut weeks) in by_cat {
            weeks.sort_unstable();
            let min_sp = min_spacing(cat);
            for w in weeks.windows(2) {
                let sp = w[1] - w[0];
                assert!(
                    sp >= min_sp,
                    "categoria {cat}: spacing={sp} < min={min_sp} entre sw={} e sw={}",
                    w[0],
                    w[1]
                );
            }
        }
    }

    #[test]
    fn determinismo_mesmo_seed() {
        let a = build_full_season_calendar("S001", 2027, 77).unwrap();
        let b = build_full_season_calendar("S001", 2027, 77).unwrap();
        assert_eq!(a.len(), b.len());
        for (ea, eb) in a.iter().zip(b.iter()) {
            assert_eq!(
                ea.track_id, eb.track_id,
                "não-determinístico: rodada {} / {}",
                ea.rodada, ea.categoria
            );
            assert_eq!(ea.categoria, eb.categoria);
            assert_eq!(ea.season_week, eb.season_week);
            assert_eq!(ea.thematic_slot, eb.thematic_slot);
        }
    }

    #[test]
    fn seeds_diferentes_produzem_calendarios_diferentes() {
        let a = build_full_season_calendar("S001", 2027, 1).unwrap();
        let b = build_full_season_calendar("S001", 2027, 2).unwrap();
        let a_tracks: Vec<u32> = a.iter().map(|e| e.track_id).collect();
        let b_tracks: Vec<u32> = b.iter().map(|e| e.track_id).collect();
        assert_ne!(
            a_tracks, b_tracks,
            "seeds distintos devem produzir calendários distintos"
        );
    }

    // ── Validação de 100 seeds ────────────────────────────────────────────────

    #[test]
    fn validacao_100_seeds() {
        let special_slots = [
            ThematicSlot::AberturaEspecial,
            ThematicSlot::RodadaEspecial,
            ThematicSlot::FinalEspecial,
        ];
        let expected_counts: &[(&str, usize)] = &[
            ("mazda_rookie", 5),
            ("toyota_rookie", 5),
            ("mazda_amador", 8),
            ("toyota_amador", 8),
            ("bmw_m2", 8),
            ("production_challenger", 10),
            ("gt4", 10),
            ("gt3", 14),
            ("endurance", 6),
        ];

        for seed in 0u64..100 {
            let entries = build_full_season_calendar("S001", 2027, seed)
                .unwrap_or_else(|e| panic!("seed={seed} falhou: {e}"));

            // Contagem total
            assert_eq!(entries.len(), 74, "seed={seed}: total deve ser 74");

            // Contagem por categoria
            for &(cat, count) in expected_counts {
                let actual = entries.iter().filter(|e| e.categoria == cat).count();
                assert_eq!(actual, count, "seed={seed} cat={cat}");
            }

            // Todas Temporada
            for e in &entries {
                assert_eq!(e.season_phase, SeasonPhase::Temporada, "seed={seed} phase");
            }

            // Sem slots especiais
            for e in &entries {
                assert!(
                    !special_slots.contains(&e.thematic_slot),
                    "seed={seed}: slot especial em {}",
                    e.categoria
                );
            }

            // season_week em 10–51
            for e in &entries {
                let sw = e.season_week.unwrap();
                assert!((10..=51).contains(&sw), "seed={seed} sw={sw}");
            }

            // Conflict pairs disjuntos
            for &(cat_a, cat_b) in &CALENDAR_CONFLICTS {
                let wa: std::collections::HashSet<u32> = entries
                    .iter()
                    .filter(|e| e.categoria == cat_a)
                    .map(|e| e.season_week.unwrap())
                    .collect();
                let wb: std::collections::HashSet<u32> = entries
                    .iter()
                    .filter(|e| e.categoria == cat_b)
                    .map(|e| e.season_week.unwrap())
                    .collect();
                assert!(
                    wa.is_disjoint(&wb),
                    "seed={seed} par ({cat_a},{cat_b}) compartilha weeks"
                );
            }

            // Abertura (10–13) e final escalonado por prestígio (topo 48–51, resto antes)
            let prestige = ["production_challenger", "gt4", "endurance", "gt3"];
            let mut by_cat: HashMap<&str, Vec<_>> = HashMap::new();
            for e in &entries {
                by_cat.entry(e.categoria.as_str()).or_default().push(e);
            }
            for (cat, mut rounds) in by_cat {
                rounds.sort_by_key(|e| e.rodada);
                let first_sw = rounds[0].season_week.unwrap();
                let last_sw = rounds.last().unwrap().season_week.unwrap();
                assert!(
                    (10..=13).contains(&first_sw),
                    "seed={seed} {cat}: abertura sw={first_sw}"
                );
                let final_ok = if prestige.contains(&cat) {
                    (48..=51).contains(&last_sw)
                } else {
                    (40..=47).contains(&last_sw)
                };
                assert!(final_ok, "seed={seed} {cat}: final sw={last_sw}");
            }

            // gt3 final ≠ endurance final
            let gt3_final = entries
                .iter()
                .filter(|e| e.categoria == "gt3")
                .max_by_key(|e| e.rodada)
                .map(|e| e.season_week.unwrap())
                .unwrap();
            let end_final = entries
                .iter()
                .filter(|e| e.categoria == "endurance")
                .max_by_key(|e| e.rodada)
                .map(|e| e.season_week.unwrap())
                .unwrap();
            assert_ne!(
                gt3_final, end_final,
                "seed={seed}: gt3 e endurance final iguais"
            );
        }
    }

    // ── Testes de DB ──────────────────────────────────────────────────────────

    #[test]
    fn db_round_trip_conta_74() {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_all(&conn).unwrap();
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2027)).unwrap();

        let inserted = generate_full_season_calendar(&conn, "S001", 2027, 42).unwrap();
        assert_eq!(inserted, 74);

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar WHERE COALESCE(season_id,temporada_id)='S001'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 74);
    }

    #[test]
    fn db_round_trip_season_week_nao_nulo() {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_all(&conn).unwrap();
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2027)).unwrap();
        generate_full_season_calendar(&conn, "S001", 2027, 42).unwrap();

        let non_null: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar \
                 WHERE COALESCE(season_id,temporada_id)='S001' AND season_week IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            non_null, 74,
            "todas as 74 entradas devem ter season_week definido"
        );
    }

    #[test]
    fn db_round_trip_season_phase_temporada() {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_all(&conn).unwrap();
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2027)).unwrap();
        generate_full_season_calendar(&conn, "S001", 2027, 42).unwrap();

        let non_temporada: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar \
                 WHERE COALESCE(season_id,temporada_id)='S001' AND season_phase != 'Temporada'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            non_temporada, 0,
            "todas as entradas devem ter season_phase='Temporada'"
        );
    }

    #[test]
    fn db_round_trip_ids_unicos() {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_all(&conn).unwrap();
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2027)).unwrap();
        generate_full_season_calendar(&conn, "S001", 2027, 42).unwrap();

        let distinct: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT id) FROM calendar \
                 WHERE COALESCE(season_id,temporada_id)='S001'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(distinct, 74, "todos os 74 IDs devem ser únicos");
    }

    #[test]
    fn db_season_week_consistente_com_week_of_year() {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_all(&conn).unwrap();
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2027)).unwrap();
        generate_full_season_calendar(&conn, "S001", 2027, 42).unwrap();

        // Para a janela de corridas: season_week = week_of_year + 4
        let inconsistentes: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar \
                 WHERE COALESCE(season_id,temporada_id)='S001' \
                   AND season_week IS NOT NULL \
                   AND season_week != week_of_year + 4",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            inconsistentes, 0,
            "season_week deve ser exatamente week_of_year+4 para todas as entradas"
        );
    }

    // ── Testes do gerador parcial (Etapa 10) ──────────────────────────────────

    #[test]
    fn partial_degrada_regra_descrita() {
        // (a) alvo + canônico cabe: span=42, target=10, min_spacing=3 → 3*9=27 ≤ 42
        assert_eq!(partial_compute_count(5, 47, 10, 3, 3, 2), (10, false));
        // (b) alvo falha, mínimo + canônico cabe: span=8, prod: 3*9=27>8 falha; 3*2=6≤8 ok
        assert_eq!(partial_compute_count(39, 47, 10, 3, 3, 2), (3, false));
        // (c) alvo+canônico falha, mínimo+canônico falha, mínimo+relaxado cabe: end min=2, relax=3
        //     span=3, end: 5*(2-1)=5>3 falha; 3*(2-1)=3≤3 ok
        assert_eq!(partial_compute_count(42, 45, 6, 2, 5, 3), (2, false));
        // (d) nem mínimo com relaxado cabe → o que couber, com aviso
        //     span=1, end: 3*(2-1)=3>1 falha; max=1+1/3=1; warn=true
        assert_eq!(partial_compute_count(44, 45, 6, 2, 5, 3), (1, true));
        // janela negativa → 0 rounds
        assert_eq!(partial_compute_count(50, 45, 6, 2, 5, 3), (0, true));
    }

    #[test]
    fn partial_inicio_normal_gera_alvo_completo() {
        // from_sw=10: semana de abertura; janela completa → 10 prod + 6 end
        let entries = build_partial_special_divisions("S001", 2027, 10, 42).unwrap();
        let prod = entries
            .iter()
            .filter(|e| e.categoria == "production_challenger")
            .count();
        let end = entries
            .iter()
            .filter(|e| e.categoria == "endurance")
            .count();
        assert_eq!(prod, 10, "janela completa: production deve ter 10");
        assert_eq!(end, 6, "janela completa: endurance deve ter 6");
    }

    #[test]
    fn partial_meio_do_ano_degrada() {
        // from_sw=40: janela reduzida. prod span=47-(40-4)=47-36=11 → 3*9=27>11 → tenta mín
        // prod mín 3: 3*2=6≤11 ok → 3 rounds.
        // end span=45-(40-4)=45-36=9 → 5*5=25>9 → tenta mín
        // end mín 2: 5*1=5≤9 ok → 2 rounds.
        let entries = build_partial_special_divisions("S001", 2027, 40, 99).unwrap();
        let prod = entries
            .iter()
            .filter(|e| e.categoria == "production_challenger")
            .count();
        let end = entries
            .iter()
            .filter(|e| e.categoria == "endurance")
            .count();
        assert_eq!(prod, 3, "from_sw=40: production deve ter 3 (mínimo)");
        assert_eq!(end, 2, "from_sw=40: endurance deve ter 2 (mínimo)");
    }

    #[test]
    fn partial_fim_do_ano_gera_menos_que_minimo() {
        // from_sw=49: end span = 45-(49-4)=45-45=0 → end=1 (1 round cabe com span=0).
        // prod span = 47-(49-4)=47-45=2 → prod max=1+2/2=2; 3*2=6>2 → tenta mín 3: 3*2=6>2
        //   → tenta relaxado: 2*2=4>2 → o que couber: 1+2/2=2; warn
        let entries = build_partial_special_divisions("S001", 2027, 49, 1).unwrap();
        let end = entries
            .iter()
            .filter(|e| e.categoria == "endurance")
            .count();
        // end span = 45 - (49-4) = 0 → max_count(0, relax=3) = 1
        assert_eq!(end, 1, "from_sw=49: endurance deve ter 1 rodada no máximo");
        // prod deve ter ≤ 2 (ou 0 se from_sw>47)
        let prod = entries
            .iter()
            .filter(|e| e.categoria == "production_challenger")
            .count();
        assert!(prod <= 2, "from_sw=49: production deve ter ≤ 2 rodadas");
    }

    #[test]
    fn partial_alem_da_janela_end_zero_entries() {
        // from_sw=51: end span = 45-(51-4)=45-47<0 → 0 rounds de endurance
        let entries = build_partial_special_divisions("S001", 2027, 51, 7).unwrap();
        let end = entries
            .iter()
            .filter(|e| e.categoria == "endurance")
            .count();
        assert_eq!(end, 0, "from_sw>49: endurance deve ter 0 entradas");
    }

    #[test]
    fn partial_season_week_em_range_valido() {
        for from_sw in [10u32, 20, 30, 40, 48] {
            let entries = build_partial_special_divisions("S001", 2027, from_sw, from_sw as u64)
                .unwrap_or_else(|e| panic!("from_sw={from_sw} falhou: {e}"));
            for entry in &entries {
                let sw = entry.season_week.unwrap();
                let max_sw = if entry.categoria == "endurance" {
                    49
                } else {
                    51
                };
                assert!(
                    sw >= from_sw && sw <= max_sw,
                    "from_sw={from_sw} {}: sw={sw} fora de [{from_sw},{max_sw}]",
                    entry.categoria
                );
            }
        }
    }

    #[test]
    fn partial_season_phase_temporada() {
        let entries = build_partial_special_divisions("S001", 2027, 10, 42).unwrap();
        for entry in &entries {
            assert_eq!(
                entry.season_phase,
                crate::models::enums::SeasonPhase::Temporada,
                "entry {} tem phase {:?}",
                entry.id,
                entry.season_phase
            );
        }
    }

    #[test]
    fn partial_slots_somente_regulares() {
        let special = [
            crate::models::enums::ThematicSlot::AberturaEspecial,
            crate::models::enums::ThematicSlot::RodadaEspecial,
            crate::models::enums::ThematicSlot::FinalEspecial,
        ];
        let entries = build_partial_special_divisions("S001", 2027, 10, 42).unwrap();
        for entry in &entries {
            assert!(
                !special.contains(&entry.thematic_slot),
                "entry {} tem slot especial {:?}",
                entry.id,
                entry.thematic_slot
            );
        }
    }

    #[test]
    fn partial_sem_lmp2() {
        let entries = build_partial_special_divisions("S001", 2027, 10, 42).unwrap();
        assert!(
            entries.iter().all(|e| e.categoria != "lmp2"),
            "partial não deve gerar lmp2"
        );
    }

    #[test]
    fn partial_determinismo() {
        let a = build_partial_special_divisions("S001", 2027, 15, 77).unwrap();
        let b = build_partial_special_divisions("S001", 2027, 15, 77).unwrap();
        assert_eq!(a.len(), b.len(), "determinismo: tamanhos diferentes");
        for (ea, eb) in a.iter().zip(b.iter()) {
            assert_eq!(ea.track_id, eb.track_id, "determinismo: track_id diferente");
            assert_eq!(
                ea.season_week, eb.season_week,
                "determinismo: season_week diferente"
            );
            assert_eq!(
                ea.thematic_slot, eb.thematic_slot,
                "determinismo: slot diferente"
            );
        }
    }

    #[test]
    fn partial_seeds_diferentes_calendarios_diferentes() {
        let a = build_partial_special_divisions("S001", 2027, 10, 1).unwrap();
        let b = build_partial_special_divisions("S001", 2027, 10, 2).unwrap();
        let a_tracks: Vec<u32> = a.iter().map(|e| e.track_id).collect();
        let b_tracks: Vec<u32> = b.iter().map(|e| e.track_id).collect();
        assert_ne!(
            a_tracks, b_tracks,
            "seeds distintos devem gerar calendários distintos"
        );
    }

    #[test]
    fn partial_db_insere_e_ids_unicos() {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_all(&conn).unwrap();
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2027)).unwrap();

        let inserted = generate_partial_special_divisions(&conn, "S001", 2027, 10, 42).unwrap();
        assert_eq!(
            inserted, 16,
            "from_sw=10 deve inserir 16 entradas (10 prod + 6 end)"
        );

        let distinct: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT id) FROM calendar \
                 WHERE COALESCE(season_id,temporada_id)='S001'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(distinct, 16, "todos os IDs devem ser únicos");
    }

    #[test]
    fn partial_db_season_week_nao_nulo() {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_all(&conn).unwrap();
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2027)).unwrap();
        generate_partial_special_divisions(&conn, "S001", 2027, 10, 42).unwrap();

        let nulos: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar \
                 WHERE COALESCE(season_id,temporada_id)='S001' AND season_week IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(nulos, 0, "nenhuma entrada deve ter season_week NULL");
    }
}
