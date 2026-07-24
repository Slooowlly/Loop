//! Ritmo, consistência, fluxo de posições e o RIVAL — os números "duros" da corrida,
//! mais os limiares anti-falso-positivo compartilhados pelos outros submódulos.

use std::collections::HashMap;

use crate::iracing_sdk::race_monitor::RaceHistory;

use super::tipos::{PaceAnalysis, PositionFlow, RivalCard};

pub(super) fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

/// Limiar de volta "limpa": dentro de 4% da melhor volta.
pub(super) const CLEAN_LAP_FACTOR: f64 = 1.04;

/// Voltas válidas mínimas para o card de RITMO aparecer.
const MIN_PACE_LAPS: i32 = 2;
/// Voltas válidas mínimas para a CONSISTÊNCIA ser confiável.
pub(super) const MIN_CONSISTENCY_LAPS: i32 = 3;
/// Voltas mínimas do campo para o "vs grid" valer.
const MIN_GRID_SAMPLE: i32 = 3;
/// Voltas mínimas ao lado de um piloto para chamá-lo de RIVAL.
const MIN_RIVAL_LAPS: i32 = 3;
/// Gap médio máximo (s) para considerar que houve disputa real.
const MAX_RIVAL_GAP_S: f64 = 3.0;

/// Conta os movimentos brutos de posição na trajetória do jogador. Posição MENOR
/// = subiu. Inclui ganhos herdados (alguém à frente abandona também sobe sua
/// posição), por isso é só ESTIMATIVA — o split fino fica com a tabela oficial.
pub(super) fn analyze_position_flow(history: &RaceHistory) -> Option<PositionFlow> {
    let positions: Vec<i32> = history
        .player_track
        .iter()
        .map(|p| p.position)
        .filter(|p| *p > 0)
        .collect();
    if positions.len() < 3 {
        return None;
    }
    let mut gained = 0;
    let mut lost = 0;
    let mut prev = positions[0];
    for &pos in &positions[1..] {
        if pos < prev {
            gained += prev - pos;
        } else if pos > prev {
            lost += pos - prev;
        }
        prev = pos;
    }
    Some(PositionFlow {
        gained_on_track: gained,
        lost_on_track: lost,
        samples: positions.len() as i32,
    })
}

/// Confiança da análise pela cobertura (voltas do jogador vs corrida).
/// Quando não sabemos a duração da corrida, caímos no número absoluto de voltas.
pub(super) fn confidence_label(laps_seen: i32, race_laps: i32) -> (String, bool) {
    if race_laps > 0 {
        let coverage = laps_seen as f64 / race_laps as f64;
        let conf = if coverage >= 0.9 {
            "alta"
        } else if coverage >= 0.6 {
            "media"
        } else {
            "baixa"
        };
        // Saiu bem antes do fim (faltaram >= 2 voltas).
        let partial = (race_laps - laps_seen) >= 2;
        (conf.to_string(), partial)
    } else {
        let conf = if laps_seen >= 8 {
            "alta"
        } else if laps_seen >= 4 {
            "media"
        } else {
            "baixa"
        };
        (conf.to_string(), false)
    }
}

pub(super) fn analyze_pace(history: &RaceHistory, player_idx: i32) -> Option<PaceAnalysis> {
    // Tempos do jogador (segundos → ms).
    let times: Vec<f64> = history
        .player_laps
        .iter()
        .map(|l| l.time)
        .filter(|t| *t > 0.0)
        .map(|t| t * 1000.0)
        .collect();
    // Precisa de um mínimo de voltas válidas para o card de ritmo ter sentido.
    if (times.len() as i32) < MIN_PACE_LAPS {
        return None;
    }
    let best = times.iter().cloned().fold(f64::INFINITY, f64::min);
    let real_avg = mean(&times);
    let clean: Vec<f64> = times
        .iter()
        .cloned()
        .filter(|t| *t <= best * CLEAN_LAP_FACTOR)
        .collect();
    let clean_avg = if clean.is_empty() { real_avg } else { mean(&clean) };

    // Ritmo do campo (todos os carros menos o jogador/pace), em ms.
    let grid_times: Vec<f64> = history
        .car_laps
        .iter()
        .filter(|l| l.car_idx != player_idx && l.time > 0.0)
        .map(|l| l.time * 1000.0)
        .collect();
    let grid_sample = grid_times.len() as i32;
    let grid_avg = mean(&grid_times);
    let vs_grid_reliable = grid_sample >= MIN_GRID_SAMPLE && grid_avg > 0.0;

    Some(PaceAnalysis {
        best_lap_ms: best,
        real_avg_ms: real_avg,
        clean_avg_ms: clean_avg,
        lost_per_lap_ms: (real_avg - clean_avg).max(0.0),
        grid_avg_ms: grid_avg,
        vs_grid_ms: if grid_avg > 0.0 {
            clean_avg - grid_avg
        } else {
            0.0
        },
        good_laps: clean.len() as i32,
        total_laps: times.len() as i32,
        consistency_reliable: (times.len() as i32) >= MIN_CONSISTENCY_LAPS,
        grid_sample,
        vs_grid_reliable,
    })
}

/// Acha o rival (car_idx, voltas ao lado, gap médio) com as mesmas regras
/// anti-falso-positivo do card. Compartilhado pelo card e pelo "melhor momento".
pub(super) fn find_rival(history: &RaceHistory) -> Option<(i32, i32, f64)> {
    if history.player_track.is_empty() {
        return None;
    }
    // Para cada vizinho (à frente/atrás), junta as voltas vistas e os gaps.
    let mut laps_by_idx: HashMap<i32, std::collections::HashSet<i32>> = HashMap::new();
    let mut gaps_by_idx: HashMap<i32, Vec<f64>> = HashMap::new();
    for p in &history.player_track {
        for (idx, gap) in [(p.ahead_idx, p.gap_ahead), (p.behind_idx, p.gap_behind)] {
            if idx < 0 {
                continue;
            }
            laps_by_idx.entry(idx).or_default().insert(p.lap.max(0));
            if gap.is_finite() && gap >= 0.0 {
                gaps_by_idx.entry(idx).or_default().push(gap);
            }
        }
    }
    // Rival = quem apareceu em MAIS voltas ao seu lado.
    let (rival_idx, laps) = laps_by_idx
        .iter()
        .map(|(idx, laps)| (*idx, laps.len() as i32))
        .max_by_key(|(_, n)| *n)?;
    // Anti-falso-positivo: só é "rival" com disputa real — voltas suficientes
    // ao lado E gap médio pequeno. Caso contrário, sem rival claro (None).
    if laps < MIN_RIVAL_LAPS {
        return None;
    }
    let gaps = gaps_by_idx.get(&rival_idx)?;
    if gaps.is_empty() {
        return None;
    }
    let avg_gap = mean(gaps);
    if avg_gap > MAX_RIVAL_GAP_S {
        return None;
    }
    Some((rival_idx, laps, avg_gap))
}

pub(super) fn analyze_rival(
    history: &RaceHistory,
    name_by_idx: &HashMap<i32, String>,
) -> Option<RivalCard> {
    let (rival_idx, laps, avg_gap) = find_rival(history)?;
    let name = name_by_idx.get(&rival_idx)?.clone();
    Some(RivalCard {
        pilot_name: name,
        laps_battled: laps,
        avg_gap_s: avg_gap,
    })
}

/// Você terminou À FRENTE do rival? Última adjacência vence: se na última vez que
/// ele apareceu ao seu lado estava ATRÁS (behind_idx), você venceu a disputa.
pub(super) fn rival_beaten(history: &RaceHistory, rival_idx: i32) -> bool {
    let mut beaten = None;
    for p in &history.player_track {
        if p.ahead_idx == rival_idx {
            beaten = Some(false);
        } else if p.behind_idx == rival_idx {
            beaten = Some(true);
        }
    }
    beaten.unwrap_or(false)
}
