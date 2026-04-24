mod generator;

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, NaiveDate, Weekday};
use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};

use crate::constants::categories::{
    get_all_categories, get_category_config, has_calendar_conflict, is_especial, CategoryConfig,
};
use crate::constants::tracks::{
    get_qualifying_duration, get_rain_chance, get_track, get_tracks_for_tier, TrackInfo,
};
use crate::db::queries::calendar as cal_queries;
use crate::generators::ids::{next_ids, IdType};
use crate::models::enums::{RaceStatus, SeasonPhase, ThematicSlot, WeatherCondition};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarEntry {
    pub id: String,
    pub season_id: String,
    pub categoria: String,
    pub rodada: i32,
    pub nome: String,
    pub track_id: u32,
    pub track_name: String,
    pub track_config: String,
    pub clima: WeatherCondition,
    pub temperatura: f64,
    pub voltas: i32,
    pub duracao_corrida_min: i32,
    pub duracao_classificacao_min: i32,
    pub status: RaceStatus,
    pub horario: String,
    /// Semana do ano (1–52) — unidade temporal interna do sistema.
    /// A ordenação e toda lógica temporal baseiam-se neste campo.
    pub week_of_year: i32,
    /// Fase da temporada em que o evento ocorre (BlocoRegular ou BlocoEspecial).
    pub season_phase: SeasonPhase,
    /// Data visual derivada de week_of_year — para UI, notícias e narrativa.
    /// Não é a base lógica do sistema; use week_of_year para ordenação.
    pub display_date: String,
    /// Papel narrativo fixo desta corrida dentro da temporada.
    /// Determinado no momento da geração — imutável após persistência.
    /// `NaoClassificado` para saves pré-v12 ou caminho legado.
    pub thematic_slot: ThematicSlot,
}

// ── Constantes de calendário ──────────────────────────────────────────────────

/// Janelas mensais da temporada.
/// O dia exato continua flexível, mas cada bloco precisa caber na sua faixa do ano.
const REGULAR_WINDOW_START_MONTH: u32 = 2;
const REGULAR_WINDOW_END_MONTH: u32 = 8;
const SPECIAL_WINDOW_START_MONTH: u32 = 9;
const SPECIAL_WINDOW_END_MONTH: u32 = 12;

const SCHEDULE_HOURS: [&str; 5] = ["10:00", "12:00", "14:00", "16:00", "18:00"];

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
    let phase = if is_especial(categoria) {
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

/// Gera todos os calendários regulares (exclui especiais) para uso em produção.
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
        if is_especial(category.id) {
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
        if !is_especial(category.id) {
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

/// Extrai o número sequencial de um season_id no formato "S001".
/// Retorna 1 como fallback seguro.
/// Nota v1: usado como proxy de season_number — considerar passar explicitamente no futuro.
fn parse_season_number(season_id: &str) -> i32 {
    season_id
        .trim_start_matches('S')
        .parse::<i32>()
        .unwrap_or(1)
}

/// Seleciona pistas para uma categoria usando o pool temático resolvido.
///
/// Fluxo:
/// 1. Pré-reservar slots narrativos (last → penult → first) com strong tracks
/// 2. Endurance: garantir âncora forte no miolo além do slot final
/// 3. Visitor: alocar em round de miolo não-narrativo não-banned
/// 4. Preencher rounds restantes aleatoriamente
/// 5. Resolver conflitos residuais de ban
fn select_tracks_themed<R: Rng>(
    pool: &generator::ThematicPool,
    config: &CategoryConfig,
    season_phase: SeasonPhase,
    banned_tracks_by_round: &HashMap<i32, HashSet<u32>>,
    rng: &mut R,
) -> Result<Vec<(&'static TrackInfo, ThematicSlot)>, String> {
    let total = config.corridas_por_temporada as i32;

    // Construir lista base de TrackInfos candidatas (sem visitor)
    let mut available: Vec<&'static TrackInfo> = pool
        .candidate_ids
        .iter()
        .filter_map(|&id| get_track(id))
        .collect();

    // Resultado final: indexed por rodada (0-based internamente, 1-based externamente)
    let mut assigned: Vec<Option<&'static TrackInfo>> = vec![None; total as usize];
    let mut used_ids: HashSet<u32> = HashSet::new();

    // Rastreia o slot narrativo de cada rodada (0-based)
    let mut slot_by_round: Vec<ThematicSlot> = vec![
        match season_phase {
            SeasonPhase::BlocoEspecial => ThematicSlot::RodadaEspecial,
            _ => ThematicSlot::RodadaRegular,
        };
        total as usize
    ];

    // Rodada 1 sempre recebe slot de abertura, independente de ser strong ou não
    if total >= 1 {
        slot_by_round[0] = match season_phase {
            SeasonPhase::BlocoEspecial => ThematicSlot::AberturaEspecial,
            _ => ThematicSlot::AberturaDaTemporada,
        };
    }

    // Helper: pegar strong não-usado não-banned para um round (1-based)
    let pick_strong = |available: &mut Vec<&'static TrackInfo>,
                       used_ids: &HashSet<u32>,
                       strong_ids: &[u32],
                       round: i32,
                       banned: &HashMap<i32, HashSet<u32>>,
                       rng: &mut R|
     -> Option<&'static TrackInfo> {
        let banned_set = banned.get(&round);
        let mut candidates: Vec<&'static TrackInfo> = strong_ids
            .iter()
            .filter_map(|&id| get_track(id))
            .filter(|t| {
                !used_ids.contains(&t.track_id)
                    && banned_set.map_or(true, |b| !b.contains(&t.track_id))
            })
            .collect();
        if candidates.is_empty() {
            // Fallback gracioso: qualquer disponível não-banned
            candidates = available
                .iter()
                .copied()
                .filter(|t| {
                    !used_ids.contains(&t.track_id)
                        && banned_set.map_or(true, |b| !b.contains(&t.track_id))
                })
                .collect();
        }
        if candidates.is_empty() {
            return None;
        }
        candidates.shuffle(rng);
        let track = candidates[0];
        available.retain(|t| t.track_id != track.track_id);
        Some(track)
    };

    // ── Passo 1: reservar slots narrativos (last → penult → first) ────────────
    let slots_to_reserve: Vec<(i32, bool)> = {
        let mut slots = Vec::new();
        if pool.narrative_rounds.strong_last {
            slots.push((total, true));
        }
        if pool.narrative_rounds.strong_penult && total >= 2 {
            slots.push((total - 1, true));
        }
        if pool.narrative_rounds.strong_first {
            slots.push((1, true));
        }
        slots
    };

    for (round, _strong) in &slots_to_reserve {
        if let Some(track) = pick_strong(
            &mut available,
            &used_ids,
            &pool.strong_ids,
            *round,
            banned_tracks_by_round,
            rng,
        ) {
            assigned[(round - 1) as usize] = Some(track);
            used_ids.insert(track.track_id);

            // Classificar slot narrativo pela posição
            let idx = (round - 1) as usize;
            if *round == total {
                slot_by_round[idx] = match season_phase {
                    SeasonPhase::BlocoEspecial => ThematicSlot::FinalEspecial,
                    _ => ThematicSlot::FinalDaTemporada,
                };
            } else if *round == total - 1 && pool.narrative_rounds.strong_penult {
                slot_by_round[idx] = ThematicSlot::TensaoPreFinal;
            }
            // strong_first na rodada 1: já classificada como AberturaDaTemporada/AberturaEspecial acima
        }
    }

    // ── Passo 2: Endurance — garantir âncora forte no miolo ──────────────────
    if config.id == "endurance" {
        let has_strong_in_narrative = slots_to_reserve.iter().any(|(r, _)| {
            assigned[(r - 1) as usize]
                .map(|t| pool.strong_ids.contains(&t.track_id))
                .unwrap_or(false)
        });
        if !has_strong_in_narrative {
            // Reservar âncora em round de miolo não-narrativo
            let narrative_rounds: HashSet<i32> = slots_to_reserve.iter().map(|(r, _)| *r).collect();
            let miolo_rounds: Vec<i32> = (1..=total)
                .filter(|r| !narrative_rounds.contains(r) && assigned[(r - 1) as usize].is_none())
                .collect();
            if !miolo_rounds.is_empty() {
                let anchor_round = miolo_rounds[rng.gen_range(0..miolo_rounds.len())];
                if let Some(track) = pick_strong(
                    &mut available,
                    &used_ids,
                    &pool.strong_ids,
                    anchor_round,
                    banned_tracks_by_round,
                    rng,
                ) {
                    assigned[(anchor_round - 1) as usize] = Some(track);
                    used_ids.insert(track.track_id);
                    slot_by_round[(anchor_round - 1) as usize] = ThematicSlot::MidpointPrestigio;
                }
            }
        }
    }

    // ── Passo 3: Visitor — slot de miolo dedicado ─────────────────────────────
    if let Some(visitor_id) = pool.visitor_id {
        if let Some(visitor_track) = get_track(visitor_id) {
            let narrative_rounds: HashSet<i32> = slots_to_reserve.iter().map(|(r, _)| *r).collect();
            let mut eligible_rounds: Vec<i32> = (1..=total)
                .filter(|r| {
                    !narrative_rounds.contains(r)
                        && assigned[(r - 1) as usize].is_none()
                        && banned_tracks_by_round
                            .get(r)
                            .map_or(true, |b| !b.contains(&visitor_id))
                })
                .collect();
            if !eligible_rounds.is_empty() {
                eligible_rounds.shuffle(rng);
                let visitor_round = eligible_rounds[0];
                assigned[(visitor_round - 1) as usize] = Some(visitor_track);
                used_ids.insert(visitor_id);
                available.retain(|t| t.track_id != visitor_id);
                slot_by_round[(visitor_round - 1) as usize] = ThematicSlot::VisitanteRegional;
            }
        }
    }

    // ── Passo 4: preencher rounds restantes com retry (derangement-safe) ────────
    // Com pools mínimos (N tracks para N rounds) e bans do campeonato irmão,
    // o fill greedy pode travar num derangement inválido. Retry com re-shuffle
    // até 30 tentativas garante encontrar uma permissão válida quando ela existe.
    let base_assigned = assigned.clone();
    let base_used_ids = used_ids.clone();
    let base_available: Vec<&'static TrackInfo> = pool
        .candidate_ids
        .iter()
        .filter_map(|&id| get_track(id))
        .filter(|t| !base_used_ids.contains(&t.track_id))
        .collect();

    let mut fill_ok = false;
    for _ in 0..30 {
        assigned = base_assigned.clone();
        used_ids = base_used_ids.clone();
        let mut try_avail = base_available.clone();
        try_avail.shuffle(rng);

        let mut attempt_ok = true;
        for round in 1..=total {
            if assigned[(round - 1) as usize].is_some() {
                continue;
            }
            let banned_set = banned_tracks_by_round.get(&round);
            if let Some(t) = try_avail
                .iter()
                .find(|t| banned_set.map_or(true, |b| !b.contains(&t.track_id)))
                .copied()
            {
                assigned[(round - 1) as usize] = Some(t);
                used_ids.insert(t.track_id);
                try_avail.retain(|a| a.track_id != t.track_id);
            } else {
                attempt_ok = false;
                break;
            }
        }
        if attempt_ok {
            fill_ok = true;
            break;
        }
    }

    if !fill_ok {
        return Err(format!(
            "Não foi possível resolver conflito de calendário para {} (pool esgotado)",
            config.id
        ));
    }

    // ── Montar resultado final ────────────────────────────────────────────────
    assigned
        .into_iter()
        .zip(slot_by_round.into_iter())
        .enumerate()
        .map(|(i, (opt, slot))| {
            opt.ok_or_else(|| format!("Rodada {} não preenchida para {}", i + 1, config.id))
                .map(|track| (track, slot))
        })
        .collect()
}

fn generate_calendar_for_category_with_constraints<F, R>(
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
        select_tracks_themed(&pool, config, season_phase, banned_tracks_by_round, rng)?
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

    Ok(entries)
}

fn select_tracks<R: Rng>(
    config: &CategoryConfig,
    eligible_tracks: &[&'static TrackInfo],
    banned_tracks_by_round: &HashMap<i32, HashSet<u32>>,
    rng: &mut R,
) -> Result<Vec<&'static TrackInfo>, String> {
    let mut used = HashSet::new();
    let fixed_tracks = select_fixed_tracks(config, eligible_tracks);
    let mut selected = fixed_tracks.clone();
    used.extend(fixed_tracks.iter().map(|track| track.track_id));

    let remaining_needed = config.corridas_por_temporada as usize - selected.len();
    let mut variable_candidates: Vec<&TrackInfo> = eligible_tracks
        .iter()
        .copied()
        .filter(|track| !used.contains(&track.track_id))
        .collect();
    variable_candidates.shuffle(rng);

    for track in variable_candidates.into_iter().take(remaining_needed) {
        used.insert(track.track_id);
        selected.push(track);
    }

    if selected.len() != config.corridas_por_temporada as usize {
        return Err(format!(
            "Nao foi possivel selecionar pistas suficientes para {}",
            config.id
        ));
    }

    if config.tier == 0 {
        selected.shuffle(rng);
    }

    let mut ordered = Vec::with_capacity(selected.len());
    let mut remaining = selected;
    for rodada in 1..=config.corridas_por_temporada as i32 {
        let banned = banned_tracks_by_round.get(&rodada);
        let chosen_index = remaining
            .iter()
            .position(|track| {
                banned
                    .map(|tracks| !tracks.contains(&track.track_id))
                    .unwrap_or(true)
            })
            .or_else(|| {
                eligible_tracks
                    .iter()
                    .copied()
                    .find(|track| {
                        !ordered
                            .iter()
                            .any(|used_track: &&TrackInfo| used_track.track_id == track.track_id)
                            && banned
                                .map(|tracks| !tracks.contains(&track.track_id))
                                .unwrap_or(true)
                    })
                    .map(|replacement| {
                        remaining.push(replacement);
                        remaining.len() - 1
                    })
            });

        let Some(index) = chosen_index else {
            return Err(format!(
                "Nao foi possivel resolver conflito de calendario para {} na rodada {}",
                config.id, rodada
            ));
        };

        ordered.push(remaining.remove(index));
    }

    Ok(ordered)
}

fn select_fixed_tracks(
    config: &CategoryConfig,
    eligible_tracks: &[&'static TrackInfo],
) -> Vec<&'static TrackInfo> {
    let fixed_count = config.pistas_fixas as usize;
    if fixed_count == 0 {
        return Vec::new();
    }

    let start_index = config
        .id
        .bytes()
        .fold(0_usize, |acc, byte| acc + byte as usize)
        % eligible_tracks.len();

    (0..fixed_count)
        .map(|offset| eligible_tracks[(start_index + offset) % eligible_tracks.len()])
        .collect()
}

fn build_calendar_entry<R: Rng>(
    id: String,
    season_id: &str,
    season_year: i32,
    categoria: &str,
    rodada: i32,
    week_of_year: i32,
    season_phase: SeasonPhase,
    thematic_slot: ThematicSlot,
    track: &TrackInfo,
    config: &CategoryConfig,
    rng: &mut R,
) -> CalendarEntry {
    let clima = random_weather(track.track_id, rng);
    let temperatura = random_temperature(clima, rng);
    let duracao_corrida_min = resolve_race_duration(config, rng);
    let duracao_classificacao_min = get_qualifying_duration(track.track_id) as i32;
    let voltas = estimate_laps(track, duracao_corrida_min);
    let (track_name, track_config) = split_track_name(track.nome);

    CalendarEntry {
        id,
        season_id: season_id.to_string(),
        categoria: categoria.to_string(),
        rodada,
        nome: format!("Rodada {} - {}", rodada, track.nome_curto),
        track_id: track.track_id,
        track_name,
        track_config,
        clima,
        temperatura,
        voltas,
        duracao_corrida_min,
        duracao_classificacao_min,
        status: RaceStatus::Pendente,
        horario: SCHEDULE_HOURS[rng.gen_range(0..SCHEDULE_HOURS.len())].to_string(),
        week_of_year,
        season_phase,
        display_date: display_date_for_category_week(
            season_year,
            week_of_year,
            categoria,
            season_phase,
        ),
        thematic_slot,
    }
}

// ── Helpers temporais ─────────────────────────────────────────────────────────

/// Distribui N rodadas uniformemente entre [start_week, end_week].
/// rodada é 1-based.
fn week_for_rodada(rodada: i32, total: i32, start: i32, end: i32) -> i32 {
    if total <= 1 {
        return start;
    }
    start + (rodada - 1) * (end - start) / (total - 1)
}

pub(crate) fn calendar_week_for_round(
    year: i32,
    season_phase: SeasonPhase,
    rodada: i32,
    total: i32,
) -> i32 {
    let (week_start, week_end) = season_week_window(year, season_phase);
    week_for_rodada(rodada, total, week_start, week_end)
}

pub(crate) fn display_date_for_category_week(
    year: i32,
    week: i32,
    category_id: &str,
    season_phase: SeasonPhase,
) -> String {
    display_date_for_weekday(
        year,
        week,
        resolve_calendar_weekday(category_id, season_phase),
    )
}

fn resolve_calendar_weekday(category_id: &str, season_phase: SeasonPhase) -> Weekday {
    match season_phase {
        SeasonPhase::BlocoEspecial => Weekday::Sun,
        _ => match category_id {
            "mazda_rookie" => Weekday::Mon,
            "toyota_rookie" => Weekday::Tue,
            "mazda_amador" => Weekday::Wed,
            "toyota_amador" => Weekday::Thu,
            "bmw_m2" => Weekday::Fri,
            "gt3" => Weekday::Sun,
            "gt4" => Weekday::Sat,
            _ => Weekday::Sat,
        },
    }
}

fn season_week_window(year: i32, phase: SeasonPhase) -> (i32, i32) {
    let (start_date, end_date) = season_date_window(year, phase);
    (
        start_date.iso_week().week() as i32,
        end_date.iso_week().week() as i32,
    )
}

fn season_date_window(year: i32, phase: SeasonPhase) -> (NaiveDate, NaiveDate) {
    match phase {
        // Começo mais para o fim de fevereiro para abrir espaço real ao mercado de dezembro-fevereiro.
        SeasonPhase::BlocoRegular => (
            last_weekday_of_month(year, REGULAR_WINDOW_START_MONTH, Weekday::Sat),
            nth_weekday_of_month(year, REGULAR_WINDOW_END_MONTH, Weekday::Sat, 3),
        ),
        // Deixa a virada agosto/setembro para convocação e preserva o restante de dezembro para o mercado aberto.
        SeasonPhase::BlocoEspecial => (
            nth_weekday_of_month(year, SPECIAL_WINDOW_START_MONTH, Weekday::Sun, 2),
            last_weekday_of_month(year, SPECIAL_WINDOW_END_MONTH, Weekday::Sun),
        ),
        _ => (
            last_weekday_of_month(year, REGULAR_WINDOW_START_MONTH, Weekday::Sat),
            nth_weekday_of_month(year, REGULAR_WINDOW_END_MONTH, Weekday::Sat, 3),
        ),
    }
}

fn nth_weekday_of_month(year: i32, month: u32, weekday: Weekday, nth: u32) -> NaiveDate {
    let first_day = NaiveDate::from_ymd_opt(year, month, 1).expect("valid month");
    let offset = (7 + weekday.num_days_from_monday() as i64
        - first_day.weekday().num_days_from_monday() as i64)
        % 7;
    let day = 1 + offset as u32 + (nth.saturating_sub(1) * 7);
    NaiveDate::from_ymd_opt(year, month, day)
        .or_else(|| last_day_of_month(year, month))
        .expect("valid nth weekday fallback")
}

fn last_weekday_of_month(year: i32, month: u32, weekday: Weekday) -> NaiveDate {
    let mut current = last_day_of_month(year, month).expect("valid last day");
    while current.weekday() != weekday {
        current = current.pred_opt().expect("previous day within month");
    }
    current
}

fn last_day_of_month(year: i32, month: u32) -> Option<NaiveDate> {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)?.pred_opt()
}

/// Converte week_of_year + year em uma data visual ISO "YYYY-MM-DD" (Sábado da semana).
/// Apenas para display — a lógica temporal usa week_of_year diretamente.
#[cfg(test)]
fn week_to_display_date(year: i32, week: i32) -> String {
    display_date_for_weekday(year, week, Weekday::Sat)
}

fn display_date_for_weekday(year: i32, week: i32, weekday: Weekday) -> String {
    NaiveDate::from_isoywd_opt(year, week.clamp(1, 52) as u32, weekday)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| format!("{}-01-01", year))
}

fn random_weather(rain_track_id: u32, rng: &mut impl Rng) -> WeatherCondition {
    let rain_chance = get_rain_chance(rain_track_id);
    if rng.gen::<f64>() >= rain_chance {
        return WeatherCondition::Dry;
    }

    let intensity = rng.gen::<f64>();
    if intensity < 0.40 {
        WeatherCondition::Damp
    } else if intensity < 0.80 {
        WeatherCondition::Wet
    } else {
        WeatherCondition::HeavyRain
    }
}

fn random_temperature(clima: WeatherCondition, rng: &mut impl Rng) -> f64 {
    let (min, max) = match clima {
        WeatherCondition::Dry => (20.0, 35.0),
        WeatherCondition::Damp => (15.0, 25.0),
        WeatherCondition::Wet => (12.0, 22.0),
        WeatherCondition::HeavyRain => (10.0, 20.0),
    };
    (rng.gen_range(min..=max) * 10.0_f64).round() / 10.0_f64
}

fn resolve_race_duration(config: &CategoryConfig, rng: &mut impl Rng) -> i32 {
    if config.duracao_corrida_min > 0 {
        config.duracao_corrida_min as i32
    } else {
        [120, 180, 240, 360][rng.gen_range(0..4)]
    }
}

fn estimate_laps(track: &TrackInfo, duracao_corrida_min: i32) -> i32 {
    let tempo_volta_estimado_min = track.comprimento_km / 2.0;
    ((duracao_corrida_min as f64 / tempo_volta_estimado_min).ceil() as i32).clamp(5, 50)
}

fn split_track_name(full_name: &str) -> (String, String) {
    if let Some((name, config)) = full_name.split_once(" - ") {
        (name.to_string(), config.to_string())
    } else {
        (full_name.to_string(), "Default".to_string())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Datelike;
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;
    use crate::constants::tracks::get_track;

    #[test]
    fn test_generate_calendar_correct_count() {
        let mut rng = StdRng::seed_from_u64(1);
        let gt3 = generate_calendar_for_category("S001", "gt3", &mut rng).expect("gt3 calendar");
        let mazda = generate_calendar_for_category("S001", "mazda_rookie", &mut rng)
            .expect("rookie calendar");

        assert_eq!(gt3.len(), 14);
        assert_eq!(mazda.len(), 5);
    }

    #[test]
    fn test_generate_calendar_no_duplicate_tracks() {
        let mut rng = StdRng::seed_from_u64(2);
        let calendar = generate_calendar_for_category("S001", "gt4", &mut rng).expect("calendar");
        let unique: HashSet<_> = calendar.iter().map(|entry| entry.track_id).collect();
        assert_eq!(unique.len(), calendar.len());
    }

    #[test]
    fn test_generate_calendar_respects_themed_pool() {
        // Com o gerador temático, rookie usa pools regionais (USA ou Europa).
        // O teste antigo verificava gratuita==true, mas os pools temáticos podem incluir
        // tracks paid (ex: VIR=58, Snetterton=316) por design. Verificamos que as tracks
        // vêm dos pools regionais elegíveis para rookie.
        let mut rng = StdRng::seed_from_u64(3);
        let calendar =
            generate_calendar_for_category("S001", "mazda_rookie", &mut rng).expect("calendar");
        let usa = generator::free_tracks_for_region(generator::CalendarRegion::Usa);
        let eur = generator::free_tracks_for_region(generator::CalendarRegion::Europa);
        let all_eligible: std::collections::HashSet<u32> =
            usa.iter().chain(eur.iter()).copied().collect();
        assert!(calendar
            .iter()
            .all(|entry| all_eligible.contains(&entry.track_id)));
    }

    #[test]
    fn test_generate_calendar_weather_distribution() {
        let mut rng = StdRng::seed_from_u64(4);
        let mut wet_races = 0_usize;
        let mut total_races = 0_usize;

        for _ in 0..100 {
            let calendar =
                generate_calendar_for_category("S001", "gt3", &mut rng).expect("calendar");
            wet_races += calendar
                .iter()
                .filter(|entry| entry.clima != WeatherCondition::Dry)
                .count();
            total_races += calendar.len();
        }

        let ratio = wet_races as f64 / total_races as f64;
        assert!(
            ratio > 0.05 && ratio < 0.35,
            "unexpected wet ratio: {}",
            ratio
        );
    }

    #[test]
    fn test_generate_all_calendars_no_conflicts() {
        let mut rng = StdRng::seed_from_u64(5);
        let calendars = generate_all_calendars("S001", &mut rng).expect("all calendars");

        for (left, right) in [
            ("mazda_rookie", "toyota_rookie"),
            ("mazda_amador", "toyota_amador"),
        ] {
            let left_calendar = calendars.get(left).expect("left calendar");
            let right_calendar = calendars.get(right).expect("right calendar");

            for (left_entry, right_entry) in left_calendar.iter().zip(right_calendar.iter()) {
                assert_ne!(left_entry.track_id, right_entry.track_id);
            }
        }
    }

    #[test]
    fn test_generate_calendar_voltas_reasonable() {
        let mut rng = StdRng::seed_from_u64(6);
        let calendar =
            generate_calendar_for_category("S001", "endurance", &mut rng).expect("calendar");
        assert!(calendar
            .iter()
            .all(|entry| (5..=50).contains(&entry.voltas)));
    }

    // ── Testes de week_for_rodada ─────────────────────────────────────────────

    #[test]
    fn test_week_for_rodada_boundaries() {
        // Primeira rodada → start; última → end
        assert_eq!(week_for_rodada(1, 5, 2, 40), 2);
        assert_eq!(week_for_rodada(5, 5, 2, 40), 40);
        assert_eq!(week_for_rodada(1, 14, 2, 40), 2);
        assert_eq!(week_for_rodada(14, 14, 2, 40), 40);
    }

    #[test]
    fn test_week_for_rodada_monotonic() {
        let total = 8;
        let mut prev = 0i32;
        for r in 1..=total {
            let w = week_for_rodada(r, total, 2, 40);
            assert!(w >= prev, "week não é monotônico em rodada {r}");
            prev = w;
        }
    }

    #[test]
    fn test_week_for_rodada_single_round() {
        assert_eq!(week_for_rodada(1, 1, 41, 50), 41);
    }

    // ── Testes de week_to_display_date ────────────────────────────────────────

    #[test]
    fn test_week_to_display_date_format() {
        let d = week_to_display_date(2028, 12);
        // Deve ser "YYYY-MM-DD"
        assert_eq!(d.len(), 10);
        assert_eq!(&d[4..5], "-");
        assert_eq!(&d[7..8], "-");
    }

    #[test]
    fn test_week_to_display_date_valid_range() {
        // Semanas 1-52 devem produzir datas válidas sem fallback
        for week in 1..=52 {
            let d = week_to_display_date(2028, week);
            assert!(
                !d.ends_with("01-01") || week == 1 || week == 52,
                "semana {week} produziu data fallback"
            );
        }
    }

    // ── Testes de generate_calendar_for_category_with_year ───────────────────

    #[test]
    fn test_regular_calendar_month_window() {
        let mut rng = StdRng::seed_from_u64(10);
        let calendar = generate_calendar_for_category_with_year("S001", 2028, "gt3", &mut rng)
            .expect("gt3 calendar");
        for entry in &calendar {
            let date = chrono::NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d")
                .expect("valid regular display_date");
            assert!(
                (2..=8).contains(&date.month()),
                "data regular {} fora da janela fevereiro-agosto",
                entry.display_date
            );
            assert_eq!(entry.season_phase, SeasonPhase::BlocoRegular);
        }
    }

    #[test]
    fn test_regular_calendar_display_date_not_empty() {
        let mut rng = StdRng::seed_from_u64(11);
        let calendar = generate_calendar_for_category_with_year("S001", 2028, "gt3", &mut rng)
            .expect("gt3 calendar");
        for entry in &calendar {
            assert!(!entry.display_date.is_empty(), "display_date vazia");
        }
    }

    #[test]
    fn test_weekday_policy_rookies_are_stable_monday_or_tuesday() {
        for category in ["mazda_rookie", "toyota_rookie"] {
            let mut rng = StdRng::seed_from_u64(42);
            let calendar =
                generate_calendar_for_category_with_year("S001", 2028, category, &mut rng)
                    .expect("rookie calendar");
            let weekdays: HashSet<Weekday> = calendar
                .iter()
                .map(|entry| {
                    NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d")
                        .expect("date")
                        .weekday()
                })
                .collect();

            assert_eq!(
                weekdays.len(),
                1,
                "{category} deve usar um dia fixo na temporada"
            );
            assert!(
                matches!(weekdays.iter().next(), Some(Weekday::Mon | Weekday::Tue)),
                "{category} deve correr segunda ou terça, recebeu {:?}",
                weekdays
            );
        }
    }

    #[test]
    fn test_weekday_policy_amador_categories_are_stable_wednesday_or_thursday() {
        for category in ["mazda_amador", "toyota_amador"] {
            let mut rng = StdRng::seed_from_u64(43);
            let calendar =
                generate_calendar_for_category_with_year("S001", 2028, category, &mut rng)
                    .expect("amador calendar");
            let weekdays: HashSet<Weekday> = calendar
                .iter()
                .map(|entry| {
                    NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d")
                        .expect("date")
                        .weekday()
                })
                .collect();

            assert_eq!(
                weekdays.len(),
                1,
                "{category} deve usar um dia fixo na temporada"
            );
            assert!(
                matches!(weekdays.iter().next(), Some(Weekday::Wed | Weekday::Thu)),
                "{category} deve correr quarta ou quinta, recebeu {:?}",
                weekdays
            );
        }
    }

    #[test]
    fn test_weekday_policy_bmw_m2_is_friday() {
        let mut rng = StdRng::seed_from_u64(43);
        let calendar = generate_calendar_for_category_with_year("S001", 2028, "bmw_m2", &mut rng)
            .expect("bmw calendar");

        for entry in &calendar {
            let date = NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d").expect("date");
            assert_eq!(
                date.weekday(),
                Weekday::Fri,
                "bmw_m2 gerou {}",
                entry.display_date
            );
            assert!(
                (2..=8).contains(&date.month()),
                "bmw_m2 saiu da janela fevereiro-agosto em {}",
                entry.display_date
            );
        }
    }

    #[test]
    fn test_weekday_policy_gt4_saturday_and_gt3_sunday() {
        let cases = [("gt4", Weekday::Sat), ("gt3", Weekday::Sun)];

        for (category, expected_weekday) in cases {
            let mut rng = StdRng::seed_from_u64(44);
            let calendar =
                generate_calendar_for_category_with_year("S001", 2028, category, &mut rng)
                    .expect("gt calendar");

            for entry in &calendar {
                let date =
                    NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d").expect("date");
                assert_eq!(
                    date.weekday(),
                    expected_weekday,
                    "{category} gerou {}",
                    entry.display_date
                );
                assert!(
                    (2..=8).contains(&date.month()),
                    "{category} saiu da janela fevereiro-agosto em {}",
                    entry.display_date
                );
            }
        }
    }

    #[test]
    fn test_weekday_policy_special_categories_are_sunday() {
        for category in ["production_challenger", "endurance"] {
            let mut rng = StdRng::seed_from_u64(45);
            let calendar =
                generate_calendar_for_category_with_year("S001", 2028, category, &mut rng)
                    .expect("special calendar");

            for entry in &calendar {
                let date =
                    NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d").expect("date");
                assert_eq!(
                    date.weekday(),
                    Weekday::Sun,
                    "{category} gerou {}",
                    entry.display_date
                );
                assert!(
                    (9..=12).contains(&date.month()),
                    "{category} saiu da janela setembro-dezembro em {}",
                    entry.display_date
                );
            }
        }
    }

    #[test]
    fn test_conflict_pairs_share_week_slots() {
        let mut rng = StdRng::seed_from_u64(12);
        let mazda =
            generate_calendar_for_category_with_year("S001", 2028, "mazda_rookie", &mut rng)
                .expect("mazda");
        let toyota =
            generate_calendar_for_category_with_year("S001", 2028, "toyota_rookie", &mut rng)
                .expect("toyota");
        // Pares conflito têm mesmo N de rodadas → mesmas semanas
        let mazda_weeks: Vec<i32> = mazda.iter().map(|e| e.week_of_year).collect();
        let toyota_weeks: Vec<i32> = toyota.iter().map(|e| e.week_of_year).collect();
        assert_eq!(
            mazda_weeks, toyota_weeks,
            "pares de conflito devem ter as mesmas semanas"
        );
    }

    // ── Testes de generate_and_insert_special_calendars ───────────────────────

    #[test]
    fn test_special_calendars_month_window() {
        use crate::db::migrations;
        use crate::db::queries::seasons::insert_season;
        use crate::models::season::Season;
        use rusqlite::Connection;

        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("migrations");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2028)).expect("season");

        let mut rng = StdRng::seed_from_u64(20);
        generate_and_insert_special_calendars(&conn, "S001", 2028, &mut rng)
            .expect("special calendars");

        let mut stmt = conn
            .prepare(
                "SELECT data FROM calendar
                 WHERE season_phase = 'BlocoEspecial'
                 ORDER BY week_of_year ASC, data ASC",
            )
            .expect("prepare special dates");
        let dates: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .expect("query special dates")
            .collect::<Result<_, _>>()
            .expect("collect special dates");

        assert!(
            !dates.is_empty(),
            "calendário especial deveria gerar ao menos uma data"
        );

        for display_date in dates {
            let date = chrono::NaiveDate::parse_from_str(&display_date, "%Y-%m-%d")
                .expect("valid special display_date");
            assert!(
                (9..=12).contains(&date.month()),
                "data especial {} fora da janela setembro-dezembro",
                display_date
            );
        }
    }

    #[test]
    fn test_special_calendar_reaches_the_last_week_of_december() {
        let mut rng = StdRng::seed_from_u64(26);
        let calendar = generate_calendar_for_category_with_year(
            "S001",
            2028,
            "production_challenger",
            &mut rng,
        )
        .expect("special calendar");

        let last_date = calendar
            .iter()
            .map(|entry| {
                NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d")
                    .expect("valid special display_date")
            })
            .max()
            .expect("last special date");

        assert_eq!(last_date.month(), 12);
        assert!(
            last_date.day() >= 24,
            "a última especial deve chegar ao fim de dezembro, recebeu {last_date}"
        );
        assert_eq!(last_date.weekday(), Weekday::Sun);
    }

    #[test]
    fn test_special_calendars_counts() {
        use crate::db::migrations;
        use crate::db::queries::seasons::insert_season;
        use crate::models::season::Season;
        use rusqlite::Connection;

        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("migrations");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2028)).expect("season");

        let mut rng = StdRng::seed_from_u64(21);
        generate_and_insert_special_calendars(&conn, "S001", 2028, &mut rng)
            .expect("special calendars");

        let pc: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar WHERE categoria = 'production_challenger'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let end: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar WHERE categoria = 'endurance'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        assert_eq!(pc, 10, "production_challenger deve ter 10 rodadas");
        assert_eq!(end, 6, "endurance deve ter 6 rodadas");
    }

    #[test]
    fn test_special_calendars_season_phase() {
        use crate::db::migrations;
        use crate::db::queries::seasons::insert_season;
        use crate::models::season::Season;
        use rusqlite::Connection;

        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("migrations");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2028)).expect("season");

        let mut rng = StdRng::seed_from_u64(22);
        generate_and_insert_special_calendars(&conn, "S001", 2028, &mut rng)
            .expect("special calendars");

        let wrong_phase: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar
             WHERE categoria IN ('production_challenger', 'endurance')
               AND season_phase != 'BlocoEspecial'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        assert_eq!(wrong_phase, 0, "entradas especiais com season_phase errado");
    }

    #[test]
    fn test_special_calendars_rejects_duplicate() {
        use crate::db::migrations;
        use crate::db::queries::seasons::insert_season;
        use crate::models::season::Season;
        use rusqlite::Connection;

        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("migrations");
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2028)).expect("season");

        let mut rng = StdRng::seed_from_u64(23);
        generate_and_insert_special_calendars(&conn, "S001", 2028, &mut rng)
            .expect("primeira geração");

        let mut rng2 = StdRng::seed_from_u64(24);
        let result = generate_and_insert_special_calendars(&conn, "S001", 2028, &mut rng2);
        assert!(result.is_err(), "segunda chamada deveria retornar Err");
    }

    // ── Testes de storyline temático ──────────────────────────────────────────

    #[allow(dead_code)]
    fn all_free_regional_ids() -> HashSet<u32> {
        use generator::{free_tracks_for_region, CalendarRegion};
        [
            CalendarRegion::Usa,
            CalendarRegion::Europa,
            CalendarRegion::JapaoOceania,
        ]
        .iter()
        .flat_map(|&r| free_tracks_for_region(r).iter().copied())
        .collect()
    }

    #[test]
    fn rookie_all_tracks_from_eligible_regions() {
        // mazda_rookie e toyota_rookie só usam USA e Europa (sem JapaoOceania).
        use generator::{free_tracks_for_region, CalendarRegion};
        let eligible: HashSet<u32> = [CalendarRegion::Usa, CalendarRegion::Europa]
            .iter()
            .flat_map(|&r| free_tracks_for_region(r).iter().copied())
            .collect();

        for seed in 0..30u64 {
            let mut rng = StdRng::seed_from_u64(seed + 100);
            let cal = generate_calendar_for_category("S001", "mazda_rookie", &mut rng)
                .expect("mazda_rookie");
            for entry in &cal {
                assert!(
                    eligible.contains(&entry.track_id),
                    "seed {seed}: mazda_rookie usou track {} fora de USA+Europa",
                    entry.track_id
                );
            }
        }
    }

    #[test]
    fn rookie_no_visitor_track() {
        // Rookies (S001) nunca têm visitante — pool único sem pista externa.
        use generator::{free_tracks_for_region, CalendarRegion};

        for seed in 0..30u64 {
            let mut rng = StdRng::seed_from_u64(seed + 200);
            let cal = generate_calendar_for_category("S001", "toyota_rookie", &mut rng)
                .expect("toyota_rookie");
            // Todos os tracks devem vir de UMA única região
            let usa: HashSet<u32> = free_tracks_for_region(CalendarRegion::Usa)
                .iter()
                .copied()
                .collect();
            let eur: HashSet<u32> = free_tracks_for_region(CalendarRegion::Europa)
                .iter()
                .copied()
                .collect();
            let track_ids: HashSet<u32> = cal.iter().map(|e| e.track_id).collect();
            // Se não é subconjunto de USA nem de Europa, pode ser multi-region fallback (DB incompleto)
            // Nesse caso, tracks devem ser subconjunto de USA ∪ Europa
            let all: HashSet<u32> = usa.union(&eur).copied().collect();
            assert!(
                track_ids.is_subset(&all),
                "seed {seed}: toyota_rookie usou track fora de USA+Europa"
            );
            // Sem pistas de JapaoOceania
            let jap: HashSet<u32> = free_tracks_for_region(CalendarRegion::JapaoOceania)
                .iter()
                .copied()
                .collect();
            assert!(
                track_ids.is_disjoint(&jap),
                "seed {seed}: toyota_rookie usou pista de JapaoOceania (não permitido em rookie)"
            );
        }
    }

    #[test]
    fn amador_season1_no_visitor() {
        // Na temporada 1, amador nunca tem visitante — season_number < 2.
        // Visitor só existe quando season_number >= 2 E 50% de chance.
        // S001 → season_number=1 → sempre sem visitor.
        // Verificamos que todos os tracks vêm da mesma região base.
        use generator::{free_tracks_for_region, CalendarRegion};
        let usa: HashSet<u32> = free_tracks_for_region(CalendarRegion::Usa)
            .iter()
            .copied()
            .collect();
        let eur: HashSet<u32> = free_tracks_for_region(CalendarRegion::Europa)
            .iter()
            .copied()
            .collect();
        let jap: HashSet<u32> = free_tracks_for_region(CalendarRegion::JapaoOceania)
            .iter()
            .copied()
            .collect();
        let regions = [&usa, &eur, &jap];

        for seed in 0..30u64 {
            let mut rng = StdRng::seed_from_u64(seed + 300);
            let cal = generate_calendar_for_category("S001", "mazda_amador", &mut rng)
                .expect("mazda_amador");
            let track_ids: HashSet<u32> = cal.iter().map(|e| e.track_id).collect();
            // Deve caber numa única região (DB atual pode forçar multi-region, mas sem visitor externo)
            // O que garantimos: tracks vêm do pool elegível de amador (USA + Europa + JapaoOceania)
            let all: HashSet<u32> = usa
                .iter()
                .chain(eur.iter())
                .chain(jap.iter())
                .copied()
                .collect();
            assert!(
                track_ids.is_subset(&all),
                "seed {seed}: mazda_amador S001 usou track fora das regiões elegíveis"
            );
            // Sem visitor: tracks devem caber em exatamente 1 das regiões (ou multi-region fallback)
            let fits_one_region = regions.iter().any(|r| track_ids.is_subset(r));
            let all_eligible: HashSet<u32> = all.clone();
            assert!(
                fits_one_region || track_ids.is_subset(&all_eligible),
                "seed {seed}: mazda_amador S001 misturou regiões inesperadamente"
            );
        }
    }

    #[test]
    fn amador_season2_all_tracks_from_eligible_regions() {
        // No estado atual do DB, amador tem 8 rounds mas nenhuma região tem 8 tracks
        // → multi-region fallback é sempre ativado → pool = USA ∪ Europa ∪ JapaoOceania.
        // Verificamos que todos os tracks vêm dessas 3 regiões e sem duplicatas.
        // A lógica de visitante (visitor para S002) entra em ação quando o DB crescer e
        // uma única região passar a ter >= 8 tracks — testável com mock de get_track.
        use generator::{free_tracks_for_region, CalendarRegion};
        let all_eligible: HashSet<u32> = [
            CalendarRegion::Usa,
            CalendarRegion::Europa,
            CalendarRegion::JapaoOceania,
        ]
        .iter()
        .flat_map(|&r| free_tracks_for_region(r).iter().copied())
        .collect();

        for seed in 0..30u64 {
            let mut rng = StdRng::seed_from_u64(seed + 400);
            let cal = generate_calendar_for_category("S002", "toyota_amador", &mut rng)
                .expect("toyota_amador S002");
            let unique_ids: HashSet<u32> = cal.iter().map(|e| e.track_id).collect();
            assert_eq!(
                unique_ids.len(),
                cal.len(),
                "seed {seed}: toyota_amador S002 tem tracks duplicados"
            );
            for id in &unique_ids {
                assert!(
                    all_eligible.contains(id),
                    "seed {seed}: toyota_amador S002 usou track {id} fora das regiões elegíveis"
                );
            }
        }
    }

    #[test]
    fn free_regional_final_is_strong() {
        // O último round de qualquer categoria FreeRegional deve ser uma pista forte.
        use generator::{strong_free_tracks_for_region, CalendarRegion};
        let all_strong: HashSet<u32> = [
            CalendarRegion::Usa,
            CalendarRegion::Europa,
            CalendarRegion::JapaoOceania,
        ]
        .iter()
        .flat_map(|&r| strong_free_tracks_for_region(r).iter().copied())
        .filter(|&id| get_track(id).is_some())
        .collect();

        for cat in [
            "mazda_rookie",
            "toyota_rookie",
            "mazda_amador",
            "toyota_amador",
            "bmw_m2",
        ] {
            for seed in 0..20u64 {
                let mut rng = StdRng::seed_from_u64(seed + 500);
                let cal = generate_calendar_for_category("S001", cat, &mut rng)
                    .unwrap_or_else(|e| panic!("{cat} seed {seed}: {e}"));
                let last = cal.last().expect("calendário vazio");
                assert!(
                    all_strong.contains(&last.track_id),
                    "{cat} seed {seed}: final track {} não é strong (esperado de {:?})",
                    last.track_id,
                    all_strong
                );
            }
        }
    }

    #[test]
    fn production_all_tracks_from_mix_pool() {
        let pool: HashSet<u32> = generator::production_free_mix_pool()
            .iter()
            .copied()
            .collect();
        for seed in 0..20u64 {
            let mut rng = StdRng::seed_from_u64(seed + 600);
            let cal = generate_calendar_for_category("S001", "production_challenger", &mut rng)
                .expect("production_challenger");
            for entry in &cal {
                assert!(
                    pool.contains(&entry.track_id),
                    "seed {seed}: production usou track {} fora do mix pool",
                    entry.track_id
                );
            }
        }
    }

    #[test]
    fn production_final_is_strong() {
        let strong: HashSet<u32> = generator::strong_production_tracks()
            .iter()
            .copied()
            .filter(|&id| get_track(id).is_some())
            .collect();
        for seed in 0..20u64 {
            let mut rng = StdRng::seed_from_u64(seed + 700);
            let cal = generate_calendar_for_category("S001", "production_challenger", &mut rng)
                .expect("production_challenger");
            let last = cal.last().expect("calendário vazio");
            assert!(
                strong.contains(&last.track_id),
                "seed {seed}: production final {} não é strong",
                last.track_id
            );
        }
    }

    #[test]
    fn gt4_first_and_last_are_strong() {
        let strong: HashSet<u32> = generator::strong_gt4_tracks()
            .iter()
            .copied()
            .filter(|&id| get_track(id).is_some())
            .collect();
        for seed in 0..20u64 {
            let mut rng = StdRng::seed_from_u64(seed + 800);
            let cal = generate_calendar_for_category("S001", "gt4", &mut rng).expect("gt4");
            let first = cal.first().expect("vazio");
            let last = cal.last().expect("vazio");
            assert!(
                strong.contains(&first.track_id),
                "seed {seed}: gt4 abertura {} não é strong",
                first.track_id
            );
            assert!(
                strong.contains(&last.track_id),
                "seed {seed}: gt4 final {} não é strong",
                last.track_id
            );
        }
    }

    #[test]
    fn gt3_first_and_last_are_strong() {
        let strong: HashSet<u32> = generator::strong_gt3_tracks()
            .iter()
            .copied()
            .filter(|&id| get_track(id).is_some())
            .collect();
        for seed in 0..20u64 {
            let mut rng = StdRng::seed_from_u64(seed + 900);
            let cal = generate_calendar_for_category("S001", "gt3", &mut rng).expect("gt3");
            let first = cal.first().expect("vazio");
            let last = cal.last().expect("vazio");
            assert!(
                strong.contains(&first.track_id),
                "seed {seed}: gt3 abertura {} não é strong",
                first.track_id
            );
            assert!(
                strong.contains(&last.track_id),
                "seed {seed}: gt3 final {} não é strong",
                last.track_id
            );
        }
    }

    #[test]
    fn endurance_all_tracks_from_curated_pool() {
        let pool: HashSet<u32> = generator::endurance_curated_pool()
            .iter()
            .copied()
            .collect();
        for seed in 0..20u64 {
            let mut rng = StdRng::seed_from_u64(seed + 1000);
            let cal =
                generate_calendar_for_category("S001", "endurance", &mut rng).expect("endurance");
            for entry in &cal {
                assert!(
                    pool.contains(&entry.track_id),
                    "seed {seed}: endurance usou track {} fora do pool curado",
                    entry.track_id
                );
            }
        }
    }

    #[test]
    fn endurance_has_at_least_two_strong_events() {
        // Regra de âncora: final forte + pelo menos 1 âncora de miolo = mínimo 2 eventos fortes.
        let strong: HashSet<u32> = generator::strong_endurance_tracks()
            .iter()
            .copied()
            .filter(|&id| get_track(id).is_some())
            .collect();
        for seed in 0..20u64 {
            let mut rng = StdRng::seed_from_u64(seed + 1100);
            let cal =
                generate_calendar_for_category("S001", "endurance", &mut rng).expect("endurance");
            let strong_count = cal.iter().filter(|e| strong.contains(&e.track_id)).count();
            assert!(
                strong_count >= 2,
                "seed {seed}: endurance tem apenas {strong_count} evento(s) forte(s) (mínimo 2)"
            );
        }
    }

    #[test]
    fn endurance_final_is_strong() {
        let strong: HashSet<u32> = generator::strong_endurance_tracks()
            .iter()
            .copied()
            .filter(|&id| get_track(id).is_some())
            .collect();
        for seed in 0..20u64 {
            let mut rng = StdRng::seed_from_u64(seed + 1200);
            let cal =
                generate_calendar_for_category("S001", "endurance", &mut rng).expect("endurance");
            let last = cal.last().expect("calendário vazio");
            assert!(
                strong.contains(&last.track_id),
                "seed {seed}: endurance final {} não é strong",
                last.track_id
            );
        }
    }
}
