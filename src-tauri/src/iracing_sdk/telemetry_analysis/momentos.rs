//! O "erro mais caro" e o "melhor momento" da corrida — os dois cards narrativos que
//! espelham um ao outro, pontuando cada volta e escolhendo o destaque por prioridade.

use std::collections::HashMap;

use crate::iracing_sdk::race_monitor::{PlayerLap, RaceHistory};

use super::ritmo::{find_rival, rival_beaten, CLEAN_LAP_FACTOR};
use super::tipos::{BestMoment, CostlyMistake, PaceAnalysis, PlayerIncidents};

/// Peso de cada posição perdida no score do erro (em "segundos equivalentes").
const POS_WEIGHT: f64 = 3.0;
/// Bônus de score quando a volta teve incidente/contato.
const INCIDENT_BONUS: f64 = 2.0;
/// Score mínimo para o erro virar card (corrida limpa não mostra nada).
const MISTAKE_MIN_SCORE: f64 = 3.0;
/// Excesso de tempo (s) que conta como sinal de "volta lenta".
const SLOW_SIGNAL_S: f64 = 1.5;

/// Erro mais caro (2b-2). Pontua cada volta por tempo perdido vs ritmo limpo +
/// posições perdidas + incidente, e escolhe a de maior custo. Confiança baixa é
/// escondida (melhor não mostrar do que inventar drama). DNF domina o card.
pub(super) fn analyze_mistake(
    history: &RaceHistory,
    incidents: &PlayerIncidents,
    pace: Option<&PaceAnalysis>,
) -> Option<CostlyMistake> {
    // O abandono é, por definição, o momento mais caro da corrida.
    if incidents.is_dnf {
        return Some(CostlyMistake {
            lap: incidents.dnf_lap.unwrap_or(0),
            kind: "dnf".to_string(),
            time_lost_ms: 0.0,
            positions_lost: 0,
            confidence: "alta".to_string(),
        });
    }

    let clean = pace.map(|p| p.clean_avg_ms).unwrap_or(0.0);
    let yellow: std::collections::HashSet<i32> = history.yellow_laps.iter().copied().collect();
    let crash: std::collections::HashSet<i32> = incidents.crash_laps.iter().copied().collect();

    // Volta de largada: o campo inteiro larga junto e acelera do grid, então ela é
    // SEMPRE ~vários segundos mais lenta que o ritmo — sistêmico, não erro do piloto.
    // (O ritmo limpo já a ignora via CLEAN_LAP_FACTOR; aqui neutralizamos só o sinal
    // de RITMO dela pra não virar o "erro mais caro" falso.) Batida/perda de posição
    // na largada continuam contando — um incidente de 1ª volta ainda é flagrado.
    let opening_lap = history
        .player_laps
        .iter()
        .map(|l| l.lap)
        .filter(|lap| *lap > 0)
        .min();

    // Posição representativa por volta = última amostra daquela volta.
    let mut pos_by_lap: HashMap<i32, i32> = HashMap::new();
    for p in &history.player_track {
        if p.position > 0 && p.lap >= 0 {
            pos_by_lap.insert(p.lap, p.position);
        }
    }

    let mut best: Option<(f64, CostlyMistake)> = None;
    for l in &history.player_laps {
        let lap = l.lap;
        // Voltas sob amarela são naturalmente lentas — não punir.
        if lap <= 0 || yellow.contains(&lap) {
            continue;
        }
        let lap_ms = l.time * 1000.0;
        // Na largada, a lentidão é do procedimento (grid parado/pack), não erro —
        // zera o excesso de tempo dela. Perda de posição/batida abaixo ainda valem.
        let is_opening = Some(lap) == opening_lap;
        let slow_excess = if clean > 0.0 && lap_ms > 0.0 && !is_opening {
            (lap_ms - clean).max(0.0)
        } else {
            0.0
        };
        let positions_lost = match (pos_by_lap.get(&lap), pos_by_lap.get(&(lap - 1))) {
            (Some(cur), Some(prev)) => (cur - prev).max(0),
            _ => 0,
        };
        let is_incident = crash.contains(&lap);

        let slow_s = slow_excess / 1000.0;
        let score = slow_s
            + positions_lost as f64 * POS_WEIGHT
            + if is_incident { INCIDENT_BONUS } else { 0.0 };

        // Quantos sinais BATEM nessa volta → confiança.
        let sig_slow = slow_s >= SLOW_SIGNAL_S;
        let sig_pos = positions_lost >= 1;
        let n = sig_slow as i32 + sig_pos as i32 + is_incident as i32;
        let confidence = if n >= 3 {
            "alta"
        } else if n == 2 {
            "media"
        } else if is_incident || slow_s >= 3.0 || positions_lost >= 2 {
            // Um sinal forte sozinho ainda vale (média).
            "media"
        } else {
            "baixa"
        };
        if confidence == "baixa" {
            continue;
        }

        let kind = if is_incident {
            "incident"
        } else if positions_lost >= 1 && slow_s < 2.0 {
            "position_loss"
        } else {
            "pace_drop"
        };

        let cand = CostlyMistake {
            lap,
            kind: kind.to_string(),
            time_lost_ms: slow_excess,
            positions_lost,
            confidence: confidence.to_string(),
        };
        if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
            best = Some((score, cand));
        }
    }

    // Só mostra se foi minimamente relevante — mas qualquer contato vale o card.
    best.filter(|(s, m)| *s >= MISTAKE_MIN_SCORE || m.kind == "incident")
        .map(|(_, m)| m)
}

/// Score mínimo para o melhor momento virar card (sem destaque → nada).
const BEST_MIN_SCORE: f64 = 3.0;

/// Melhor momento (2b-3) — espelho positivo do erro mais caro. Detecta vários
/// candidatos e escolhe por PRIORIDADE (não só score): ganho de posição > rival
/// superado > recuperação pós-erro > sequência limpa > melhor volta (fallback).
/// Cada candidato tem seu próprio portão de confiança; baixa nunca vira card.
pub(super) fn analyze_best_moment(
    history: &RaceHistory,
    name_by_idx: &HashMap<i32, String>,
    pace: Option<&PaceAnalysis>,
    incidents: &PlayerIncidents,
    mistake: Option<&CostlyMistake>,
) -> Option<BestMoment> {
    let clean = pace.map(|p| p.clean_avg_ms).unwrap_or(0.0);
    let yellow: std::collections::HashSet<i32> = history.yellow_laps.iter().copied().collect();
    let crash: std::collections::HashSet<i32> = incidents.crash_laps.iter().copied().collect();

    let mut pos_by_lap: HashMap<i32, i32> = HashMap::new();
    for p in &history.player_track {
        if p.position > 0 && p.lap >= 0 {
            pos_by_lap.insert(p.lap, p.position);
        }
    }
    // Volta da melhor marca pessoal.
    let best_lap_lap = history
        .player_laps
        .iter()
        .filter(|l| l.time > 0.0)
        .min_by(|a, b| {
            a.time
                .partial_cmp(&b.time)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|l| l.lap);

    // ── 1. Ganho de posição (prioridade máxima) ─────────────────────────────
    {
        let mut best: Option<(f64, i32, i32, f64)> = None; // score, lap, gain, time_gain
        for l in &history.player_laps {
            let lap = l.lap;
            if lap <= 0 || yellow.contains(&lap) || crash.contains(&lap) {
                continue;
            }
            let lap_ms = l.time * 1000.0;
            // Ataque de verdade não é volta lenta (ganho lento = herdado, não mérito).
            if clean > 0.0 && lap_ms > clean * 1.05 {
                continue;
            }
            let gain = match (pos_by_lap.get(&lap), pos_by_lap.get(&(lap - 1))) {
                (Some(cur), Some(prev)) => (prev - cur).max(0),
                _ => 0,
            };
            if gain < 1 {
                continue;
            }
            let time_gain = if clean > 0.0 && lap_ms > 0.0 {
                (clean - lap_ms).max(0.0)
            } else {
                0.0
            };
            let score = gain as f64 * POS_WEIGHT + time_gain / 1000.0;
            if best.map(|(s, ..)| score > s).unwrap_or(true) {
                best = Some((score, lap, gain, time_gain));
            }
        }
        if let Some((score, lap, gain, time_gain)) = best {
            let is_best = best_lap_lap == Some(lap);
            let n = (gain >= 1) as i32 + (time_gain >= 800.0) as i32 + is_best as i32;
            let conf = if n >= 2 {
                Some("alta")
            } else if gain >= 2 || is_best {
                Some("media")
            } else {
                None // +1 isolado não é "ganho relevante"
            };
            if let Some(conf) = conf {
                if score >= BEST_MIN_SCORE {
                    return Some(BestMoment {
                        lap,
                        kind: "position_gain".to_string(),
                        positions_gained: gain,
                        time_gain_ms: time_gain,
                        streak: 0,
                        rival_name: String::new(),
                        confidence: conf.to_string(),
                    });
                }
            }
        }
    }

    // ── 2. Rival superado ───────────────────────────────────────────────────
    if let Some((ridx, rlaps, _)) = find_rival(history) {
        if rival_beaten(history, ridx) {
            if let Some(name) = name_by_idx.get(&ridx) {
                let conf = if rlaps >= 6 { "alta" } else { "media" };
                return Some(BestMoment {
                    lap: 0,
                    kind: "rival_beaten".to_string(),
                    positions_gained: 0,
                    time_gain_ms: 0.0,
                    streak: rlaps,
                    rival_name: name.clone(),
                    confidence: conf.to_string(),
                });
            }
        }
    }

    // ── 3. Recuperação pós-erro ─────────────────────────────────────────────
    if let Some(m) = mistake {
        if m.kind != "dnf" && m.lap > 0 {
            let pos_at = pos_by_lap.get(&m.lap).copied();
            let regained = pos_at
                .and_then(|pa| {
                    history
                        .player_track
                        .iter()
                        .filter(|p| p.lap > m.lap && p.position > 0)
                        .map(|p| p.position)
                        .min()
                        .map(|best_after| (pa - best_after).max(0))
                })
                .unwrap_or(0);
            let pace_back = clean > 0.0
                && history
                    .player_laps
                    .iter()
                    .any(|l| l.lap > m.lap && l.time > 0.0 && l.time * 1000.0 <= clean * 1.02);
            let signals = (regained >= 1) as i32 + pace_back as i32;
            let score = 2.0 + regained as f64 * 1.5 + if pace_back { 1.0 } else { 0.0 };
            if signals >= 1 && score >= BEST_MIN_SCORE {
                let conf = if regained >= 2 && pace_back {
                    "alta"
                } else {
                    "media"
                };
                return Some(BestMoment {
                    lap: 0,
                    kind: "recovery".to_string(),
                    positions_gained: regained,
                    time_gain_ms: 0.0,
                    streak: 0,
                    rival_name: String::new(),
                    confidence: conf.to_string(),
                });
            }
        }
    }

    // ── 4. Sequência limpa ──────────────────────────────────────────────────
    if clean > 0.0 {
        let mut laps: Vec<&PlayerLap> = history.player_laps.iter().filter(|l| l.lap > 0).collect();
        laps.sort_by_key(|l| l.lap);
        let mut best_streak = 0;
        let mut cur = 0;
        let mut prev_lap: Option<i32> = None;
        for l in laps {
            let good = l.time > 0.0
                && l.time * 1000.0 <= clean * CLEAN_LAP_FACTOR
                && !yellow.contains(&l.lap)
                && !crash.contains(&l.lap);
            let consecutive = prev_lap.map(|p| l.lap == p + 1).unwrap_or(true);
            cur = if good && consecutive {
                cur + 1
            } else if good {
                1
            } else {
                0
            };
            best_streak = best_streak.max(cur);
            prev_lap = Some(l.lap);
        }
        if best_streak >= 3 {
            let score = 1.0 + best_streak as f64 * 0.7;
            if score >= BEST_MIN_SCORE {
                let conf = if best_streak >= 5 { "alta" } else { "media" };
                return Some(BestMoment {
                    lap: 0,
                    kind: "clean_streak".to_string(),
                    positions_gained: 0,
                    time_gain_ms: 0.0,
                    streak: best_streak,
                    rival_name: String::new(),
                    confidence: conf.to_string(),
                });
            }
        }
    }

    // ── 5. Melhor volta (fallback bom, não prioridade) ──────────────────────
    if let (Some(bl_lap), Some(p)) = (best_lap_lap, pace) {
        if clean > 0.0 && !yellow.contains(&bl_lap) && !crash.contains(&bl_lap) {
            let margin = clean - p.best_lap_ms; // quão melhor que o ritmo limpo
            if margin >= 800.0 {
                return Some(BestMoment {
                    lap: bl_lap,
                    kind: "best_lap".to_string(),
                    positions_gained: 0,
                    time_gain_ms: margin,
                    streak: 0,
                    rival_name: String::new(),
                    confidence: "media".to_string(),
                });
            }
        }
    }

    None
}
