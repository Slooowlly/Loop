//! Análise da TELEMETRIA da corrida (Fase 2 do pós-corrida): ritmo, consistência
//! e rival, a partir do histórico ao vivo do monitor (`RaceHistory`).
//!
//! Esses dados só existem ENQUANTO o jogador esteve na pista — se ele saiu cedo,
//! a análise vem parcial ou vazia. Tudo aqui é tolerante: campos `Option`/flags
//! para a tela "mostrar quando tem, esconder quando não tem". Nunca quebra.
//!
//! Lógica pura e testável; o chamador resolve `car_idx → nome` (via cars_meta +
//! roster) e passa o mapa.

use std::collections::HashMap;

use serde::Serialize;

use crate::iracing_sdk::race_monitor::{PlayerLap, RaceHistory};

/// Análise de RITMO + consistência do jogador (tempos em ms).
#[derive(Debug, Clone, Default, Serialize)]
pub struct PaceAnalysis {
    pub best_lap_ms: f64,
    /// Ritmo médio REAL (todas as voltas).
    pub real_avg_ms: f64,
    /// Ritmo LIMPO (voltas dentro de 4% da melhor — sem erros grosseiros).
    pub clean_avg_ms: f64,
    /// Tempo perdido por erros/tráfego por volta (real − limpo).
    pub lost_per_lap_ms: f64,
    /// Ritmo médio do CAMPO (todos os carros) — para comparar.
    pub grid_avg_ms: f64,
    /// Você vs campo (limpo − campo); negativo = mais rápido que a média.
    pub vs_grid_ms: f64,
    /// Voltas "boas" (dentro de 4% da melhor) / total.
    pub good_laps: i32,
    pub total_laps: i32,
    /// A consistência só é confiável com voltas suficientes (>= 3 válidas).
    /// Abaixo disso a tela esconde o card de consistência.
    pub consistency_reliable: bool,
    /// Quantas voltas do CAMPO entraram na média do grid (confiabilidade do vs_grid).
    pub grid_sample: i32,
    /// O "vs grid" só é confiável com amostra suficiente do campo.
    pub vs_grid_reliable: bool,
}

/// Movimentação BRUTA de posição do jogador na pista (Nível 2 do breakdown).
/// É só a trajetória observada — o SALDO oficial (grid → chegada) e as "herdadas
/// por DNF" continuam vindo da tabela oficial. Tudo aqui é ESTIMADO (amostragem).
#[derive(Debug, Clone, Serialize)]
pub struct PositionFlow {
    /// Soma das SUBIDAS de posição observadas na pista (ganhos brutos).
    pub gained_on_track: i32,
    /// Soma das QUEDAS de posição observadas na pista (perdas brutas).
    pub lost_on_track: i32,
    /// Amostras de posição usadas — base da confiança.
    pub samples: i32,
}

/// Sinais de incidente/abandono do jogador que NÃO estão no `RaceHistory` —
/// vêm do monitor ao vivo (`Attempt.crashes`, DNF). O chamador preenche.
#[derive(Debug, Clone, Default)]
pub struct PlayerIncidents {
    /// Voltas em que o monitor flagrou batida/contato do jogador.
    pub crash_laps: Vec<i32>,
    /// O jogador abandonou a prova.
    pub is_dnf: bool,
    /// Volta em que a corrida do jogador encerrou (última volta / batida).
    pub dnf_lap: Option<i32>,
}

/// O "erro mais caro" da corrida (2b-2): a volta de maior custo estimado. Sempre
/// ESTIMADO — combina volta lenta vs ritmo limpo, posições perdidas e incidente.
/// `kind`: "incident" | "pace_drop" | "position_loss" | "dnf". A tela formata a
/// frase a partir destes números. Só existe com confiança >= média (baixa some).
#[derive(Debug, Clone, Serialize)]
pub struct CostlyMistake {
    pub lap: i32,
    pub kind: String,
    /// Tempo perdido estimado vs ritmo limpo (ms). 0 quando n/a.
    pub time_lost_ms: f64,
    /// Posições perdidas nessa volta. 0 quando n/a.
    pub positions_lost: i32,
    /// "alta" | "media" (baixa nunca chega aqui — escondemos).
    pub confidence: String,
}

/// O piloto com quem você mais brigou.
#[derive(Debug, Clone, Serialize)]
pub struct RivalCard {
    pub pilot_name: String,
    /// Voltas distintas em que ele esteve ao seu lado (à frente ou atrás).
    pub laps_battled: i32,
    /// Gap médio para ele nesses momentos (segundos).
    pub avg_gap_s: f64,
}

/// O melhor momento da corrida (2b-3): o espelho positivo do erro mais caro.
/// `kind`: "position_gain" | "rival_beaten" | "recovery" | "clean_streak" |
/// "best_lap". Escolhido por PRIORIDADE (ganho de pos > rival > recuperação >
/// sequência > melhor volta como fallback), não só por score. Só com confiança
/// >= média; corrida sem destaque real → None (não força narrativa bonita).
#[derive(Debug, Clone, Serialize)]
pub struct BestMoment {
    /// Volta do momento (0 quando é multi-volta: sequência/rival).
    pub lap: i32,
    pub kind: String,
    pub positions_gained: i32,
    /// Ganho de tempo vs ritmo limpo (ms) — p/ volta forte / melhor volta.
    pub time_gain_ms: f64,
    /// Tamanho da sequência limpa / voltas de batalha com o rival.
    pub streak: i32,
    /// Nome do rival superado (kind "rival_beaten").
    pub rival_name: String,
    /// "alta" | "media".
    pub confidence: String,
}

/// Um ponto do race trace de um carro: gap ao líder + posição naquela volta.
#[derive(Debug, Clone, Serialize)]
pub struct ChartTracePoint {
    pub lap: i32,
    pub gap: f64,
    pub position: i32,
}

/// A linha de um carro no race trace (legenda + destaque do jogador).
#[derive(Debug, Clone, Serialize)]
pub struct ChartCar {
    pub idx: i32,
    pub name: String,
    pub is_player: bool,
    pub points: Vec<ChartTracePoint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChartLapTime {
    pub lap: i32,
    pub time_s: f64,
}

/// Tempo de volta de QUALQUER carro (para o seletor de ritmo por piloto).
#[derive(Debug, Clone, Serialize)]
pub struct ChartCarLapTime {
    pub idx: i32,
    pub lap: i32,
    pub time_s: f64,
}

/// Gap ao rival por volta, COM SINAL: + rival à frente (você caçando), − rival
/// atrás (você liderando a disputa).
#[derive(Debug, Clone, Serialize)]
pub struct ChartGap {
    pub lap: i32,
    pub gap_s: f64,
}

/// Séries para os gráficos da tela (2b — gráficos). Capturadas no import, então
/// não dependem do monitor ainda estar vivo. Vazio → a seção de gráficos some.
#[derive(Debug, Clone, Serialize)]
pub struct RaceCharts {
    /// Race trace: uma linha por carro (gap ao líder + posição por volta).
    pub cars: Vec<ChartCar>,
    /// Voltas sob bandeira amarela (faixas no gráfico).
    pub yellow_laps: Vec<i32>,
    /// Tempos de volta do jogador.
    pub lap_times: Vec<ChartLapTime>,
    /// Tempos de volta de TODOS os carros — para o seletor de ritmo por piloto.
    pub car_lap_times: Vec<ChartCarLapTime>,
    /// Gap ao rival por volta (vazio se não houve rival claro).
    pub rival_gap: Vec<ChartGap>,
    pub rival_name: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TelemetryAnalysis {
    /// Houve telemetria utilizável (o jogador correu e foi monitorado).
    pub has_telemetry: bool,
    /// Quantas voltas do jogador o monitor capturou.
    pub laps_seen: i32,
    /// Voltas TOTAIS da corrida (do líder) — base para a confiança/cobertura.
    pub race_laps: i32,
    /// Última volta do jogador efetivamente capturada (p/ "telemetria até a volta X").
    pub last_lap_seen: i32,
    /// Confiança da análise: "alta" | "media" | "baixa".
    pub confidence: String,
    /// Cobertura incompleta — o jogador saiu bem antes do fim.
    pub is_partial: bool,
    pub pace: Option<PaceAnalysis>,
    pub rival: Option<RivalCard>,
    /// Fluxo de posições na pista (Nível 2 do breakdown) — None se faltam amostras.
    pub position_flow: Option<PositionFlow>,
    /// Erro mais caro (2b-2) — None numa corrida limpa (nada de inventar drama).
    pub mistake: Option<CostlyMistake>,
    /// Melhor momento (2b-3) — None se não houve destaque real.
    pub best_moment: Option<BestMoment>,
    /// Séries para os gráficos (race trace, tempos, gap ao rival). None se vazio.
    pub charts: Option<RaceCharts>,
}

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

/// Limiar de volta "limpa": dentro de 4% da melhor volta.
const CLEAN_LAP_FACTOR: f64 = 1.04;

/// Voltas válidas mínimas para o card de RITMO aparecer.
const MIN_PACE_LAPS: i32 = 2;
/// Voltas válidas mínimas para a CONSISTÊNCIA ser confiável.
const MIN_CONSISTENCY_LAPS: i32 = 3;
/// Voltas mínimas do campo para o "vs grid" valer.
const MIN_GRID_SAMPLE: i32 = 3;
/// Voltas mínimas ao lado de um piloto para chamá-lo de RIVAL.
const MIN_RIVAL_LAPS: i32 = 3;
/// Gap médio máximo (s) para considerar que houve disputa real.
const MAX_RIVAL_GAP_S: f64 = 3.0;

/// Analisa o histórico. `name_by_idx`: car_idx → nome do piloto (resolvido fora).
/// `incidents`: sinais de batida/DNF do monitor (fora do `RaceHistory`).
pub fn analyze(
    history: &RaceHistory,
    name_by_idx: &HashMap<i32, String>,
    incidents: &PlayerIncidents,
) -> TelemetryAnalysis {
    let player_idx = history.player_car_idx;
    let pace = analyze_pace(history, player_idx);
    let rival = analyze_rival(history, name_by_idx);
    let position_flow = analyze_position_flow(history);
    let mistake = analyze_mistake(history, incidents, pace.as_ref());
    let best_moment =
        analyze_best_moment(history, name_by_idx, pace.as_ref(), incidents, mistake.as_ref());
    let charts = build_charts(history, name_by_idx);
    let laps_seen = history.player_laps.len() as i32;
    let last_lap_seen = history
        .player_laps
        .iter()
        .map(|l| l.lap)
        .max()
        .unwrap_or(0);
    // Voltas totais da corrida = última volta do líder no race trace.
    let race_laps = history.laps.iter().map(|s| s.lap).max().unwrap_or(0);

    let (confidence, is_partial) = confidence_label(laps_seen, race_laps);

    TelemetryAnalysis {
        has_telemetry: pace.is_some()
            || rival.is_some()
            || position_flow.is_some()
            || mistake.is_some()
            || best_moment.is_some(),
        laps_seen,
        race_laps,
        last_lap_seen,
        confidence,
        is_partial,
        pace,
        rival,
        position_flow,
        mistake,
        best_moment,
        charts,
    }
}

/// Monta as séries dos gráficos a partir do histórico ao vivo. Resolve nomes via
/// `name_by_idx` (fallback "Carro N"). None se não há trace nem voltas.
fn build_charts(history: &RaceHistory, name_by_idx: &HashMap<i32, String>) -> Option<RaceCharts> {
    // Race trace: um conjunto de pontos por carro presente nos snapshots.
    let mut idx_set: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
    for snap in &history.laps {
        for c in &snap.cars {
            idx_set.insert(c.idx);
        }
    }
    let cars: Vec<ChartCar> = idx_set
        .iter()
        .map(|&idx| {
            let points: Vec<ChartTracePoint> = history
                .laps
                .iter()
                .filter_map(|snap| {
                    snap.cars.iter().find(|c| c.idx == idx).map(|c| ChartTracePoint {
                        lap: snap.lap,
                        gap: c.gap,
                        position: c.position,
                    })
                })
                .collect();
            let is_player = idx == history.player_car_idx;
            let name = name_by_idx.get(&idx).cloned().unwrap_or_else(|| {
                if is_player {
                    "Você".to_string()
                } else {
                    format!("Carro {idx}")
                }
            });
            ChartCar {
                idx,
                name,
                is_player,
                points,
            }
        })
        .collect();

    let lap_times: Vec<ChartLapTime> = history
        .player_laps
        .iter()
        .filter(|l| l.time > 0.0)
        .map(|l| ChartLapTime {
            lap: l.lap,
            time_s: l.time,
        })
        .collect();

    // Tempos de volta de todos os carros (para comparar ritmo entre pilotos).
    let car_lap_times: Vec<ChartCarLapTime> = history
        .car_laps
        .iter()
        .filter(|l| l.time > 0.0)
        .map(|l| ChartCarLapTime {
            idx: l.car_idx,
            lap: l.lap,
            time_s: l.time,
        })
        .collect();

    // Gap ao rival por volta (assinado). Última amostra da volta com o rival
    // adjacente vence.
    let (rival_gap, rival_name) = if let Some((ridx, _, _)) = find_rival(history) {
        let mut by_lap: std::collections::BTreeMap<i32, f64> = std::collections::BTreeMap::new();
        for p in &history.player_track {
            if p.ahead_idx == ridx && p.gap_ahead.is_finite() {
                by_lap.insert(p.lap, p.gap_ahead);
            } else if p.behind_idx == ridx && p.gap_behind.is_finite() {
                by_lap.insert(p.lap, -p.gap_behind);
            }
        }
        let v: Vec<ChartGap> = by_lap
            .into_iter()
            .map(|(lap, g)| ChartGap { lap, gap_s: g })
            .collect();
        let name = name_by_idx.get(&ridx).cloned().unwrap_or_default();
        (v, name)
    } else {
        (Vec::new(), String::new())
    };

    if cars.is_empty() && lap_times.is_empty() {
        return None;
    }
    Some(RaceCharts {
        cars,
        yellow_laps: history.yellow_laps.clone(),
        lap_times,
        car_lap_times,
        rival_gap,
        rival_name,
    })
}

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
fn analyze_mistake(
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
        let slow_excess = if clean > 0.0 && lap_ms > 0.0 {
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
        let score = slow_s + positions_lost as f64 * POS_WEIGHT
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
fn analyze_best_moment(
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
        .min_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal))
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
                let conf = if regained >= 2 && pace_back { "alta" } else { "media" };
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

/// Conta os movimentos brutos de posição na trajetória do jogador. Posição MENOR
/// = subiu. Inclui ganhos herdados (alguém à frente abandona também sobe sua
/// posição), por isso é só ESTIMATIVA — o split fino fica com a tabela oficial.
fn analyze_position_flow(history: &RaceHistory) -> Option<PositionFlow> {
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
fn confidence_label(laps_seen: i32, race_laps: i32) -> (String, bool) {
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

fn analyze_pace(history: &RaceHistory, player_idx: i32) -> Option<PaceAnalysis> {
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
fn find_rival(history: &RaceHistory) -> Option<(i32, i32, f64)> {
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

fn analyze_rival(history: &RaceHistory, name_by_idx: &HashMap<i32, String>) -> Option<RivalCard> {
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
fn rival_beaten(history: &RaceHistory, rival_idx: i32) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iracing_sdk::race_monitor::{
        CarLap, LapSnapshot, PlayerLap, PlayerTrackPoint, RaceHistory,
    };

    fn base_history() -> RaceHistory {
        let mut h = RaceHistory::clone(&empty());
        h.player_car_idx = 0;
        h
    }
    // RaceHistory não expõe Default; montamos pelo serde de um JSON mínimo.
    fn empty() -> RaceHistory {
        serde_json::from_value(serde_json::json!({
            "laps": [], "player_laps": [], "player_track": [], "yellow_laps": [],
            "player_car_idx": 0, "attempt_number": 1, "finished": true, "outcome": "Finalizada",
            "car_laps": [], "cars_meta": [], "track_id": 1, "qualy_laps": []
        }))
        .unwrap()
    }

    #[test]
    fn pace_e_consistencia() {
        let mut h = base_history();
        // Voltas do jogador: 90, 90.5, 91, e uma ruim 96 (erro).
        h.player_laps = vec![
            PlayerLap { lap: 1, time: 90.0 },
            PlayerLap { lap: 2, time: 90.5 },
            PlayerLap { lap: 3, time: 91.0 },
            PlayerLap { lap: 4, time: 96.0 },
        ];
        // Campo um pouco mais lento.
        for lap in 1..=4 {
            h.car_laps.push(CarLap { car_idx: 1, lap, time: 92.0 });
        }
        let a = analyze(&h, &HashMap::new(), &PlayerIncidents::default());
        let p = a.pace.expect("tem ritmo");
        assert!((p.best_lap_ms - 90_000.0).abs() < 1.0);
        // Volta limpa exclui a de 96 (96000 > 90000*1.04=93600).
        assert_eq!(p.good_laps, 3);
        assert_eq!(p.total_laps, 4);
        // Perdeu tempo por causa da volta ruim.
        assert!(p.lost_per_lap_ms > 0.0);
        // Mais rápido que o campo (limpo ~90.5s vs campo 92s).
        assert!(p.vs_grid_ms < 0.0);
        // 4 voltas → consistência confiável; 4 voltas do campo → vs_grid confiável.
        assert!(p.consistency_reliable);
        assert!(p.vs_grid_reliable);
        assert_eq!(p.grid_sample, 4);
    }

    #[test]
    fn ritmo_some_mas_consistencia_nao_confiavel_com_2_voltas() {
        let mut h = base_history();
        h.player_laps = vec![
            PlayerLap { lap: 1, time: 90.0 },
            PlayerLap { lap: 2, time: 90.5 },
        ];
        let a = analyze(&h, &HashMap::new(), &PlayerIncidents::default());
        let p = a.pace.expect("2 voltas já dá ritmo");
        assert_eq!(p.total_laps, 2);
        // < 3 voltas → consistência não confiável (a tela esconde o card).
        assert!(!p.consistency_reliable);
        // sem voltas do campo → vs_grid não confiável.
        assert!(!p.vs_grid_reliable);
    }

    #[test]
    fn uma_volta_so_nao_gera_ritmo() {
        let mut h = base_history();
        h.player_laps = vec![PlayerLap { lap: 1, time: 90.0 }];
        let a = analyze(&h, &HashMap::new(), &PlayerIncidents::default());
        assert!(a.pace.is_none());
    }

    #[test]
    fn rival_e_quem_mais_brigou() {
        let mut h = base_history();
        // O carro idx 5 fica à frente/atrás por 3 voltas; o 9 só 1 volta.
        h.player_track = vec![
            PlayerTrackPoint { session_time: 1.0, lap: 1, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 0.5, behind_idx: 9, gap_behind: 0.8 },
            PlayerTrackPoint { session_time: 2.0, lap: 2, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 0.7, behind_idx: -1, gap_behind: 0.0 },
            PlayerTrackPoint { session_time: 3.0, lap: 3, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 0.6, behind_idx: -1, gap_behind: 0.0 },
        ];
        let mut names = HashMap::new();
        names.insert(5, "Lucas Silva".to_string());
        names.insert(9, "Rafael Costa".to_string());
        let a = analyze(&h, &names, &PlayerIncidents::default());
        let r = a.rival.expect("tem rival");
        assert_eq!(r.pilot_name, "Lucas Silva");
        assert_eq!(r.laps_battled, 3);
        assert!((r.avg_gap_s - 0.6).abs() < 0.05);
    }

    #[test]
    fn rival_rejeitado_com_poucas_voltas() {
        let mut h = base_history();
        // Só 2 voltas ao lado do idx 5 — abaixo do mínimo de rival.
        h.player_track = vec![
            PlayerTrackPoint { session_time: 1.0, lap: 1, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 0.5, behind_idx: -1, gap_behind: 0.0 },
            PlayerTrackPoint { session_time: 2.0, lap: 2, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 0.6, behind_idx: -1, gap_behind: 0.0 },
        ];
        let mut names = HashMap::new();
        names.insert(5, "Lucas Silva".to_string());
        let a = analyze(&h, &names, &PlayerIncidents::default());
        assert!(a.rival.is_none(), "2 voltas não é rival");
    }

    #[test]
    fn rival_rejeitado_com_gap_grande() {
        let mut h = base_history();
        // 3 voltas, mas sempre muito longe (gap > 3s) — não é disputa.
        h.player_track = vec![
            PlayerTrackPoint { session_time: 1.0, lap: 1, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 8.0, behind_idx: -1, gap_behind: 0.0 },
            PlayerTrackPoint { session_time: 2.0, lap: 2, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 9.0, behind_idx: -1, gap_behind: 0.0 },
            PlayerTrackPoint { session_time: 3.0, lap: 3, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 7.5, behind_idx: -1, gap_behind: 0.0 },
        ];
        let mut names = HashMap::new();
        names.insert(5, "Lucas Silva".to_string());
        let a = analyze(&h, &names, &PlayerIncidents::default());
        assert!(a.rival.is_none(), "gap grande não é rival");
    }

    #[test]
    fn confianca_alta_quando_cobre_a_corrida_toda() {
        let mut h = base_history();
        // Corrida de 10 voltas (líder), jogador fez 10.
        for lap in 1..=10 {
            h.laps.push(LapSnapshot { lap, cars: vec![] });
            h.player_laps.push(PlayerLap { lap, time: 90.0 });
        }
        let a = analyze(&h, &HashMap::new(), &PlayerIncidents::default());
        assert_eq!(a.race_laps, 10);
        assert_eq!(a.laps_seen, 10);
        assert_eq!(a.confidence, "alta");
        assert!(!a.is_partial);
    }

    #[test]
    fn confianca_baixa_e_parcial_quando_saiu_cedo() {
        let mut h = base_history();
        // Corrida de 12 voltas, jogador fez só 3 → saiu cedo.
        for lap in 1..=12 {
            h.laps.push(LapSnapshot { lap, cars: vec![] });
        }
        for lap in 1..=3 {
            h.player_laps.push(PlayerLap { lap, time: 90.0 });
        }
        let a = analyze(&h, &HashMap::new(), &PlayerIncidents::default());
        assert_eq!(a.race_laps, 12);
        assert_eq!(a.last_lap_seen, 3);
        assert_eq!(a.confidence, "baixa");
        assert!(a.is_partial);
    }

    #[test]
    fn position_flow_conta_subidas_e_quedas() {
        let mut h = base_history();
        // Trajetória de posição: P14 → P12 (subiu 2) → P13 (caiu 1) → P8 (subiu 5).
        let pos_seq = [14, 14, 12, 12, 13, 8, 8];
        h.player_track = pos_seq
            .iter()
            .enumerate()
            .map(|(i, &pos)| PlayerTrackPoint {
                session_time: i as f64,
                lap: i as i32,
                position: pos,
                speed_kmh: 200.0,
                ahead_idx: -1,
                gap_ahead: 0.0,
                behind_idx: -1,
                gap_behind: 0.0,
            })
            .collect();
        let a = analyze(&h, &HashMap::new(), &PlayerIncidents::default());
        let f = a.position_flow.expect("tem fluxo de posição");
        assert_eq!(f.gained_on_track, 7); // 2 + 5
        assert_eq!(f.lost_on_track, 1);
        assert_eq!(f.samples, 7);
    }

    #[test]
    fn erro_mais_caro_incidente_com_perda() {
        let mut h = base_history();
        // Ritmo limpo ~90s; volta 7 explode para 95s (perdeu ~5s).
        h.player_laps = vec![
            PlayerLap { lap: 5, time: 90.0 },
            PlayerLap { lap: 6, time: 90.0 },
            PlayerLap { lap: 7, time: 95.0 },
            PlayerLap { lap: 8, time: 90.0 },
        ];
        // Caiu de P7 (fim da volta 6) para P9 (fim da volta 7).
        h.player_track = vec![
            PlayerTrackPoint { session_time: 6.0, lap: 6, position: 7, speed_kmh: 200.0, ahead_idx: -1, gap_ahead: 0.0, behind_idx: -1, gap_behind: 0.0 },
            PlayerTrackPoint { session_time: 7.0, lap: 7, position: 9, speed_kmh: 150.0, ahead_idx: -1, gap_ahead: 0.0, behind_idx: -1, gap_behind: 0.0 },
        ];
        let inc = PlayerIncidents { crash_laps: vec![7], is_dnf: false, dnf_lap: None };
        let a = analyze(&h, &HashMap::new(), &inc);
        let m = a.mistake.expect("tem erro mais caro");
        assert_eq!(m.lap, 7);
        assert_eq!(m.kind, "incident");
        assert_eq!(m.positions_lost, 2);
        assert!(m.time_lost_ms > 3000.0);
        assert_eq!(m.confidence, "alta"); // lenta + perda + incidente
    }

    #[test]
    fn erro_mais_caro_dnf_domina() {
        let mut h = base_history();
        h.player_laps = vec![PlayerLap { lap: 1, time: 90.0 }, PlayerLap { lap: 2, time: 90.0 }];
        let inc = PlayerIncidents { crash_laps: vec![], is_dnf: true, dnf_lap: Some(9) };
        let a = analyze(&h, &HashMap::new(), &inc);
        let m = a.mistake.expect("DNF é o erro mais caro");
        assert_eq!(m.kind, "dnf");
        assert_eq!(m.lap, 9);
    }

    #[test]
    fn corrida_limpa_nao_mostra_erro() {
        let mut h = base_history();
        // Voltas consistentes, sem incidente nem perda de posição.
        h.player_laps = (1..=8)
            .map(|lap| PlayerLap { lap, time: 90.0 + (lap as f64 % 2.0) * 0.2 })
            .collect();
        let a = analyze(&h, &HashMap::new(), &PlayerIncidents::default());
        assert!(a.mistake.is_none(), "corrida limpa não inventa erro");
    }

    #[test]
    fn melhor_momento_ataque_decisivo() {
        let mut h = base_history();
        // Ritmo ~90s; volta 2 é a melhor (89s) e ganhou 2 posições.
        h.player_laps = vec![
            PlayerLap { lap: 1, time: 90.0 },
            PlayerLap { lap: 2, time: 89.0 },
            PlayerLap { lap: 3, time: 90.0 },
            PlayerLap { lap: 4, time: 90.0 },
        ];
        // P10 no fim da volta 1 → P8 no fim da volta 2 (ganho de 2).
        h.player_track = vec![
            PlayerTrackPoint { session_time: 1.0, lap: 1, position: 10, speed_kmh: 200.0, ahead_idx: -1, gap_ahead: 0.0, behind_idx: -1, gap_behind: 0.0 },
            PlayerTrackPoint { session_time: 2.0, lap: 2, position: 8, speed_kmh: 205.0, ahead_idx: -1, gap_ahead: 0.0, behind_idx: -1, gap_behind: 0.0 },
        ];
        let a = analyze(&h, &HashMap::new(), &PlayerIncidents::default());
        let b = a.best_moment.expect("tem melhor momento");
        assert_eq!(b.kind, "position_gain");
        assert_eq!(b.lap, 2);
        assert_eq!(b.positions_gained, 2);
        assert_eq!(b.confidence, "alta"); // melhor volta + ganho
    }

    #[test]
    fn melhor_momento_rival_superado() {
        let mut h = base_history();
        // Sem voltas registradas (foco no rival); idx 7 fica ATRÁS por 6 voltas.
        h.player_track = (1..=6)
            .map(|lap| PlayerTrackPoint {
                session_time: lap as f64,
                lap,
                position: 5,
                speed_kmh: 200.0,
                ahead_idx: -1,
                gap_ahead: 0.0,
                behind_idx: 7,
                gap_behind: 0.5,
            })
            .collect();
        let mut names = HashMap::new();
        names.insert(7, "Carlos Mendes".to_string());
        let a = analyze(&h, &names, &PlayerIncidents::default());
        let b = a.best_moment.expect("tem melhor momento");
        assert_eq!(b.kind, "rival_beaten");
        assert_eq!(b.rival_name, "Carlos Mendes");
        assert_eq!(b.streak, 6);
        assert_eq!(b.confidence, "alta");
    }

    #[test]
    fn charts_monta_trace_e_tempos() {
        use crate::iracing_sdk::race_monitor::CarGapPoint;
        let mut h = base_history();
        // 2 voltas de trace com 2 carros (jogador idx 0 + idx 1).
        for lap in 1..=2 {
            h.laps.push(LapSnapshot {
                lap,
                cars: vec![
                    CarGapPoint { idx: 0, position: 3, gap: 1.2 },
                    CarGapPoint { idx: 1, position: 1, gap: 0.0 },
                ],
            });
            h.player_laps.push(PlayerLap { lap, time: 90.0 });
        }
        let mut names = HashMap::new();
        names.insert(1, "Lider Silva".to_string());
        let a = analyze(&h, &names, &PlayerIncidents::default());
        let c = a.charts.expect("tem gráficos");
        assert_eq!(c.cars.len(), 2);
        assert!(c.cars.iter().any(|car| car.is_player && car.points.len() == 2));
        assert_eq!(c.lap_times.len(), 2);
    }

    #[test]
    fn sem_telemetria_nao_quebra() {
        let h = base_history();
        let a = analyze(&h, &HashMap::new(), &PlayerIncidents::default());
        assert!(!a.has_telemetry);
        assert!(a.pace.is_none());
        assert!(a.rival.is_none());
    }
}
