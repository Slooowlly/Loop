//! Geração dos calendários por categoria e por fase da temporada
//! (extraída de `calendar/mod.rs`).

use std::collections::{HashMap, HashSet};

use rand::Rng;

use crate::constants::categories::{
    get_all_categories, get_category_config, has_calendar_conflict, runs_in_special_phase,
};
use crate::constants::tracks::{get_tracks_for_tier, TrackInfo};
use crate::db::queries::calendar as cal_queries;
use crate::generators::ids::{next_ids, IdType};
use crate::models::enums::{SeasonPhase, ThematicSlot};

use super::entry::CalendarEntry;
use super::generator;
use super::janela::{season_week_window, week_for_rodada};
use super::montagem::{build_calendar_entry, ensure_night_race};
use super::selecao::{parse_season_number, select_tracks, select_tracks_themed};

// ── Funções de produção (season_year obrigatório) ─────────────────────────────

/// Gera o calendário de uma categoria para uso em produção.
/// Requer o ano da temporada para calcular datas visuais.
#[allow(dead_code)]
pub fn generate_calendar_for_category_with_year(
    season_id: &str,
    season_year: i32,
    categoria: &str,
    rng: &mut impl Rng,
) -> Result<Vec<CalendarEntry>, String> {
    if categoria == "lmp2" {
        return Err("LMP2 e uma classe da Endurance; use o calendario de endurance".to_string());
    }

    let phase = if runs_in_special_phase(categoria) {
        SeasonPhase::BlocoEspecial
    } else {
        SeasonPhase::BlocoRegular
    };
    let (week_start, week_end) = season_week_window(season_year, phase);
    let mut next_id = 1_u32;
    generate_calendar_for_category_with_constraints(
        season_id,
        season_year,
        categoria,
        week_start,
        week_end,
        phase,
        &HashMap::new(),
        &mut || {
            let id = format!("R{:03}", next_id);
            next_id += 1;
            id
        },
        rng,
    )
}

/// LEGADO 9D - gera calendários regulares do fluxo pre-9D.
/// Mantido enquanto saves em voo ainda puderem acionar o fluxo legado.
#[allow(dead_code)]
pub fn generate_all_calendars_with_year(
    season_id: &str,
    season_year: i32,
    rng: &mut impl Rng,
) -> Result<HashMap<String, Vec<CalendarEntry>>, String> {
    let mut next_id = 1_u32;
    generate_all_calendars_with_id_factory(
        season_id,
        season_year,
        &mut || {
            let id = format!("R{:03}", next_id);
            next_id += 1;
            id
        },
        rng,
    )
}

pub(crate) fn generate_all_calendars_with_id_factory<F, R>(
    season_id: &str,
    season_year: i32,
    id_generator: &mut F,
    rng: &mut R,
) -> Result<HashMap<String, Vec<CalendarEntry>>, String>
where
    F: FnMut() -> String,
    R: Rng,
{
    let mut calendars: HashMap<String, Vec<CalendarEntry>> = HashMap::new();
    let (regular_week_start, regular_week_end) =
        season_week_window(season_year, SeasonPhase::BlocoRegular);

    for category in get_all_categories() {
        // Categorias especiais não têm calendário no BlocoRegular.
        // O calendário delas é gerado em iniciar_bloco_especial.
        if runs_in_special_phase(category.id) {
            continue;
        }

        let conflicts = calendars
            .iter()
            .filter(|(other_category, _)| {
                has_calendar_conflict(category.id, other_category.as_str())
            })
            .flat_map(|(_, entries)| entries.iter())
            .fold(
                HashMap::<i32, HashSet<u32>>::new(),
                |mut acc: HashMap<i32, HashSet<u32>>, entry| {
                    acc.entry(entry.rodada).or_default().insert(entry.track_id);
                    acc
                },
            );

        let calendar = generate_calendar_for_category_with_constraints(
            season_id,
            season_year,
            category.id,
            regular_week_start,
            regular_week_end,
            SeasonPhase::BlocoRegular,
            &conflicts,
            id_generator,
            rng,
        )?;
        calendars.insert(category.id.to_string(), calendar);
    }

    Ok(calendars)
}

/// Gera e insere as entradas de calendário para as categorias especiais.
/// Chamada durante `iniciar_bloco_especial`, após a transição de fase.
///
/// Janela setembro–dezembro (bloco especial):
/// - production_challenger: 10 rodadas
/// - endurance: 6 rodadas
///
/// Retorna `Err` se já existir calendário especial para a temporada
/// (proteção contra duplicação).
pub fn generate_and_insert_special_calendars(
    conn: &rusqlite::Connection,
    season_id: &str,
    season_year: i32,
    rng: &mut impl Rng,
) -> Result<(), String> {
    let (special_week_start, special_week_end) =
        season_week_window(season_year, SeasonPhase::BlocoEspecial);

    // Guard: verificar por categoria especial (não por season_phase, para não
    // bloquear futuros eventos não-corrida dentro do mesmo bloco).
    let existing: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM calendar
             WHERE COALESCE(season_id, temporada_id) = ?1
               AND categoria IN ('production_challenger', 'endurance')",
            rusqlite::params![season_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Falha ao verificar calendário especial: {e}"))?;
    if existing > 0 {
        return Err("Calendário especial já gerado para esta temporada".to_string());
    }

    let mut all_entries: Vec<CalendarEntry> = Vec::new();

    for category in get_all_categories() {
        if !runs_in_special_phase(category.id) {
            continue;
        }
        let total = category.corridas_por_temporada as u32;
        let ids = next_ids(conn, IdType::Race, total)
            .map_err(|e| format!("Falha ao gerar IDs de corrida: {e}"))?;
        let mut ids_iter = ids.into_iter();

        let entries = generate_calendar_for_category_with_constraints(
            season_id,
            season_year,
            category.id,
            special_week_start,
            special_week_end,
            SeasonPhase::BlocoEspecial,
            &HashMap::new(),
            &mut || ids_iter.next().expect("race id"),
            rng,
        )?;
        all_entries.extend(entries);
    }

    cal_queries::insert_calendar_entries(conn, &all_entries)
        .map_err(|e| format!("Falha ao inserir calendário especial: {e}"))
}

// ── Wrappers de teste (NÃO usar em produção) ──────────────────────────────────

/// Wrapper legado para testes — usa year=2024 como padrão.
/// Em produção use generate_calendar_for_category_with_year.
#[cfg(test)]
pub fn generate_calendar_for_category(
    season_id: &str,
    categoria: &str,
    rng: &mut impl Rng,
) -> Result<Vec<CalendarEntry>, String> {
    generate_calendar_for_category_with_year(season_id, 2024, categoria, rng)
}

/// Wrapper legado para testes — usa year=2024 como padrão.
/// Em produção use generate_all_calendars_with_year.
#[cfg(test)]
pub fn generate_all_calendars(
    season_id: &str,
    rng: &mut impl Rng,
) -> Result<HashMap<String, Vec<CalendarEntry>>, String> {
    generate_all_calendars_with_year(season_id, 2024, rng)
}

// ── Geração temática ──────────────────────────────────────────────────────────

/// Variante de `generate_calendar_for_category_with_constraints` com contagem explícita.
///
/// Usada pelo gerador parcial de Production/Endurance (Etapa 10 / migração v34) quando
/// a janela restante do ano não comporta a contagem-alvo da config da categoria.
///
/// NOTA: A branch `else` (sem pool temático) delega a `select_tracks`, que internamente
/// usa `config.corridas_por_temporada`. Para production_challenger e endurance — único
/// uso real deste helper — essa branch nunca é ativada (ambas têm pool temático).
pub(crate) fn generate_calendar_for_category_with_count<F, R>(
    season_id: &str,
    season_year: i32,
    categoria: &str,
    week_start: i32,
    week_end: i32,
    count: usize,
    season_phase: SeasonPhase,
    banned_tracks_by_round: &HashMap<i32, HashSet<u32>>,
    id_generator: &mut F,
    rng: &mut R,
) -> Result<Vec<CalendarEntry>, String>
where
    F: FnMut() -> String,
    R: Rng,
{
    if count == 0 {
        return Ok(Vec::new());
    }

    let config = get_category_config(categoria)
        .ok_or_else(|| format!("Categoria desconhecida: {categoria}"))?;
    let total = count as i32;
    let season_number = parse_season_number(season_id);
    let themed = generator::resolve_thematic_pool(categoria, season_number, count, rng);

    let ordered_tracks: Vec<(&'static TrackInfo, ThematicSlot)> = if let Some(pool) = themed {
        let available_count = pool.candidate_ids.len() + pool.visitor_id.map_or(0, |_| 1);
        if available_count < count {
            return Err(format!(
                "Pool temático insuficiente para {categoria}: {available_count} disponíveis, \
                 {count} necessárias"
            ));
        }
        select_tracks_themed(
            &pool,
            config,
            total,
            season_phase,
            banned_tracks_by_round,
            rng,
        )?
    } else {
        let eligible_tracks = get_tracks_for_tier(config.tier);
        if eligible_tracks.len() < count {
            return Err(format!(
                "Pistas insuficientes para gerar calendario de {categoria}"
            ));
        }
        select_tracks(config, &eligible_tracks, banned_tracks_by_round, rng)?
            .into_iter()
            .take(count)
            .map(|t| (t, ThematicSlot::NaoClassificado))
            .collect()
    };

    let entries = ordered_tracks
        .into_iter()
        .enumerate()
        .map(|(index, (track, thematic_slot))| {
            let rodada = (index + 1) as i32;
            let week = week_for_rodada(rodada, total, week_start, week_end);
            build_calendar_entry(
                id_generator(),
                season_id,
                season_year,
                categoria,
                rodada,
                week,
                season_phase,
                thematic_slot,
                track,
                config,
                rng,
            )
        })
        .collect();

    let mut entries: Vec<CalendarEntry> = entries;
    ensure_night_race(&mut entries, config.tier, rng);

    Ok(entries)
}

pub(crate) fn generate_calendar_for_category_with_constraints<F, R>(
    season_id: &str,
    season_year: i32,
    categoria: &str,
    week_start: i32,
    week_end: i32,
    season_phase: SeasonPhase,
    banned_tracks_by_round: &HashMap<i32, HashSet<u32>>,
    id_generator: &mut F,
    rng: &mut R,
) -> Result<Vec<CalendarEntry>, String>
where
    F: FnMut() -> String,
    R: Rng,
{
    let config = get_category_config(categoria)
        .ok_or_else(|| format!("Categoria desconhecida: {categoria}"))?;

    let total = config.corridas_por_temporada as i32;
    let season_number = parse_season_number(season_id);
    let themed = generator::resolve_thematic_pool(
        categoria,
        season_number,
        config.corridas_por_temporada as usize,
        rng,
    );

    let ordered_tracks: Vec<(&'static TrackInfo, ThematicSlot)> = if let Some(pool) = themed {
        let available_count = pool.candidate_ids.len() + pool.visitor_id.map_or(0, |_| 1);
        if available_count < config.corridas_por_temporada as usize {
            return Err(format!(
                "Pool temático insuficiente para {categoria}: {available_count} disponíveis, {} necessárias",
                config.corridas_por_temporada
            ));
        }
        select_tracks_themed(
            &pool,
            config,
            total,
            season_phase,
            banned_tracks_by_round,
            rng,
        )?
    } else {
        let eligible_tracks = get_tracks_for_tier(config.tier);
        if eligible_tracks.len() < config.corridas_por_temporada as usize {
            return Err(format!(
                "Pistas insuficientes para gerar calendario de {categoria}"
            ));
        }
        select_tracks(config, &eligible_tracks, banned_tracks_by_round, rng)?
            .into_iter()
            .map(|t| (t, ThematicSlot::NaoClassificado))
            .collect()
    };

    let entries = ordered_tracks
        .into_iter()
        .enumerate()
        .map(|(index, (track, thematic_slot))| {
            let rodada = (index + 1) as i32;
            let week = week_for_rodada(rodada, total, week_start, week_end);
            build_calendar_entry(
                id_generator(),
                season_id,
                season_year,
                categoria,
                rodada,
                week,
                season_phase,
                thematic_slot,
                track,
                config,
                rng,
            )
        })
        .collect();

    let mut entries: Vec<CalendarEntry> = entries;
    ensure_night_race(&mut entries, config.tier, rng);

    Ok(entries)
}
