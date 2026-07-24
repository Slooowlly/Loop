//! Seleção e ordenação das pistas de um calendário — pool temático, pistas
//! fixas e resolução de conflitos (extraído de `calendar/mod.rs`).

use std::collections::{HashMap, HashSet};

use rand::{seq::SliceRandom, Rng};

use crate::constants::categories::CategoryConfig;
use crate::constants::tracks::{get_track, TrackInfo};
use crate::models::enums::{SeasonPhase, ThematicSlot};

use super::generator;

/// Extrai o número sequencial de um season_id no formato "S001".
/// Retorna 1 como fallback seguro.
/// Nota v1: usado como proxy de season_number — considerar passar explicitamente no futuro.
pub(crate) fn parse_season_number(season_id: &str) -> i32 {
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
pub(crate) fn select_tracks_themed<R: Rng>(
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

pub(crate) fn select_tracks<R: Rng>(
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

pub(crate) fn select_fixed_tracks(
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
