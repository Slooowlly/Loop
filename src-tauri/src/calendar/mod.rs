pub(crate) mod full_season;
mod generator;

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, NaiveDate, Weekday};
use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};

use crate::constants::categories::{
    get_all_categories, get_category_config, has_calendar_conflict, runs_in_special_phase,
    CategoryConfig,
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
    /// Não é a base lógica do sistema; use season_week para ordenação 9D.
    pub display_date: String,
    /// Papel narrativo fixo desta corrida dentro da temporada.
    /// Determinado no momento da geração — imutável após persistência.
    /// `NaoClassificado` para saves pré-v12 ou caminho legado.
    pub thematic_slot: ThematicSlot,
    /// Posição monotônica na régua 9D (1–51). None para saves pré-v33.
    /// Adicionado à coluna DB na migração v33 (Etapa 3).
    #[serde(default)]
    pub season_week: Option<u32>,
}

// ── Constantes de calendário ──────────────────────────────────────────────────

/// Janelas mensais da temporada.
/// O dia exato continua flexível, mas cada bloco precisa caber na sua faixa do ano.
const REGULAR_WINDOW_START_MONTH: u32 = 2;
const REGULAR_WINDOW_END_MONTH: u32 = 8;
const SPECIAL_WINDOW_START_MONTH: u32 = 9;
const SPECIAL_WINDOW_END_MONTH: u32 = 12;

const SCHEDULE_HOURS: [&str; 5] = ["10:00", "12:00", "14:00", "16:00", "18:00"];

/// Horário exibido na etapa NOTURNA garantida (a hora real de largada exportada
/// pro iRacing vem de `weather::night_start_hour`, atrelada à estação).
const NIGHT_SCHEDULE_HOUR: &str = "21:00";

/// Pista com iluminação (Charlotte Roval) — preferida para a corrida noturna.
const LIT_TRACK_ID: u32 = 556;

/// Uma etapa é noturna quando começa às 20h ou depois (distingue do 18h diurno).
pub fn is_night_horario(horario: &str) -> bool {
    horario
        .split(':')
        .next()
        .and_then(|hh| hh.trim().parse::<i32>().ok())
        .is_some_and(|h| h >= 20)
}

/// Garante ao menos UMA corrida noturna na temporada, NUNCA a primeira nem a
/// última rodada (decisão do user). Preserva a regra "rookie (tier 0) nunca de
/// noite" — pula categorias rookie. Precisa de ≥3 rodadas (para sobrar um miolo).
/// Prefere a pista iluminada (Charlotte) se ela cair no miolo; senão, escolhe uma
/// rodada do miolo de forma determinística pelo `rng`.
fn ensure_night_race<R: Rng>(entries: &mut [CalendarEntry], tier: u8, rng: &mut R) {
    if tier == 0 || entries.len() < 3 {
        return;
    }
    let max_rodada = entries.iter().map(|e| e.rodada).max().unwrap_or(0);
    // Já existe uma noturna no miolo? Então não força outra.
    let has_night = entries
        .iter()
        .any(|e| e.rodada != 1 && e.rodada != max_rodada && is_night_horario(&e.horario));
    if has_night {
        return;
    }
    // Índices elegíveis (miolo): nem a 1ª nem a última rodada.
    let eligible: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.rodada != 1 && e.rodada != max_rodada)
        .map(|(i, _)| i)
        .collect();
    if eligible.is_empty() {
        return;
    }
    // Preferir Charlotte (iluminada) se estiver no miolo; senão, uma rodada aleatória.
    let chosen = eligible
        .iter()
        .copied()
        .find(|&i| entries[i].track_id == LIT_TRACK_ID)
        .unwrap_or_else(|| eligible[rng.gen_range(0..eligible.len())]);
    entries[chosen].horario = NIGHT_SCHEDULE_HOUR.to_string();
}

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

fn select_tracks_themed<R: Rng>(
    pool: &generator::ThematicPool,
    config: &CategoryConfig,
    total: i32,
    season_phase: SeasonPhase,
    banned_tracks_by_round: &HashMap<i32, HashSet<u32>>,
    rng: &mut R,
) -> Result<Vec<(&'static TrackInfo, ThematicSlot)>, String> {
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
                    && banned_set.is_none_or(|b| !b.contains(&t.track_id))
            })
            .collect();
        if candidates.is_empty() {
            // Fallback gracioso: qualquer disponível não-banned
            candidates = available
                .iter()
                .copied()
                .filter(|t| {
                    !used_ids.contains(&t.track_id)
                        && banned_set.is_none_or(|b| !b.contains(&t.track_id))
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
    // A regra da família é "final forte MAIS ao menos uma âncora de miolo", ou
    // seja, no mínimo dois eventos fortes na temporada. Como `strong_last` já
    // reserva a final, aqui se conta quantos fortes existem e completa-se até
    // o mínimo — em vez de tratar o miolo como plano B para quando a final
    // falha, que era o efeito da condição antiga e deixava anos com um só.
    if config.id == "endurance" {
        const MIN_STRONG_EVENTS: usize = 2;
        let narrative_rounds: HashSet<i32> = slots_to_reserve.iter().map(|(r, _)| *r).collect();

        loop {
            let strong_reserved = assigned
                .iter()
                .filter(|slot| {
                    slot.map(|t| pool.strong_ids.contains(&t.track_id))
                        .unwrap_or(false)
                })
                .count();
            if strong_reserved >= MIN_STRONG_EVENTS {
                break;
            }

            let miolo_rounds: Vec<i32> = (1..=total)
                .filter(|r| !narrative_rounds.contains(r) && assigned[(r - 1) as usize].is_none())
                .collect();
            if miolo_rounds.is_empty() {
                break;
            }

            let anchor_round = miolo_rounds[rng.gen_range(0..miolo_rounds.len())];
            // Sem pista forte disponível pra este round, insistir não ajuda.
            let Some(track) = pick_strong(
                &mut available,
                &used_ids,
                &pool.strong_ids,
                anchor_round,
                banned_tracks_by_round,
                rng,
            ) else {
                break;
            };

            assigned[(anchor_round - 1) as usize] = Some(track);
            used_ids.insert(track.track_id);
            slot_by_round[(anchor_round - 1) as usize] = ThematicSlot::MidpointPrestigio;
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
                            .is_none_or(|b| !b.contains(&visitor_id))
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
                .find(|t| banned_set.is_none_or(|b| !b.contains(&t.track_id)))
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
        .zip(slot_by_round)
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
        display_date: display_date_for_category_round(
            season_year,
            week_of_year,
            categoria,
            season_phase,
            rodada,
        ),
        thematic_slot,
        season_week: None,
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

pub(crate) fn display_date_for_category_round(
    year: i32,
    week: i32,
    category_id: &str,
    season_phase: SeasonPhase,
    rodada: i32,
) -> String {
    if season_phase == SeasonPhase::BlocoRegular && category_id == "lmp2" {
        let weekday = if rodada % 2 == 1 {
            Weekday::Sat
        } else {
            Weekday::Sun
        };
        return display_date_for_weekday(year, week, weekday);
    }

    display_date_for_category_week(year, week, category_id, season_phase)
}

fn resolve_calendar_weekday(category_id: &str, _season_phase: SeasonPhase) -> Weekday {
    // Dia fixo por categoria. Mazda/Toyota compartilham o dia e alternam semanas
    // (a separação por semana é feita pela janela disjunta no gerador 9D).
    match category_id {
        "mazda_rookie" | "toyota_rookie" => Weekday::Mon,
        "mazda_amador" | "toyota_amador" => Weekday::Tue,
        "bmw_m2" => Weekday::Wed,
        "gt4" => Weekday::Thu,
        "gt3" => Weekday::Fri,
        "production_challenger" => Weekday::Sat,
        "endurance" | "lmp2" => Weekday::Sun,
        _ => Weekday::Sat,
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
/// Apenas para display legado — a lógica temporal 9D usa season_week.
#[cfg(test)]
fn week_to_display_date(year: i32, week: i32) -> String {
    display_date_for_weekday(year, week, Weekday::Sat)
}

fn display_date_for_weekday(year: i32, week: i32, weekday: Weekday) -> String {
    NaiveDate::from_isoywd_opt(year, week.clamp(1, 52) as u32, weekday)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| format!("{}-01-01", year))
}

/// Deriva uma data visual a partir da régua 9D (season_week) em vez de week_of_year.
/// Resolve o ano civil correto via season_week_to_calendar_year antes de delegar
/// a display_date_for_weekday — necessário porque sw 1–4 pertencem ao ano anterior.
#[allow(dead_code)]
pub(crate) fn display_date_for_season_week(
    season_week: u8,
    season_year: i32,
    weekday: Weekday,
) -> Result<String, String> {
    use crate::models::temporal::{season_week_to_calendar_year, season_week_to_week_of_year};
    let woy = season_week_to_week_of_year(season_week)?;
    let cal_year = season_week_to_calendar_year(season_week, season_year)?;
    Ok(display_date_for_weekday(cal_year, woy as i32, weekday))
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

/// Placeholder de temperatura na criação do calendário — a temperatura DEFINITIVA
/// é derivada da história de chuva no export (`weather::story_temperature`) e
/// persistida por cima. Mantida na faixa observada no iRacing [18, 32] pra não
/// destoar antes do 1º export.
fn random_temperature(clima: WeatherCondition, rng: &mut impl Rng) -> f64 {
    let (min, max) = match clima {
        WeatherCondition::Dry => (24.0, 32.0),
        WeatherCondition::Damp => (20.0, 26.0),
        WeatherCondition::Wet => (18.0, 23.0),
        WeatherCondition::HeavyRain => (18.0, 21.0),
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
#[path = "tests/mod.rs"]
mod tests;
