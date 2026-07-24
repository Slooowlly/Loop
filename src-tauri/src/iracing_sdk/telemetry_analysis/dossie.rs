//! Sinais COMPACTOS de telemetria para o **dossiê de habilidade** (Fase 2).
//!
//! O dossiê agrega POR CORRIDA, então guardamos escalares (não os pontos brutos):
//! consistência de ritmo, fração de tempo em briga + saldo de posições na pista
//! (racecraft avançado) e o ganho de posições na largada. Persistidos em
//! `player_race_telemetry` no import e lidos depois pelo estimador (`player_skill`).

use crate::iracing_sdk::race_monitor::RaceHistory;

use super::ritmo::{mean, MIN_CONSISTENCY_LAPS};
use super::tipos::TelemetryAnalysis;

/// Gap (s) abaixo do qual consideramos que havia um carro "em cima" — briga.
const BATTLE_GAP_S: f64 = 1.0;
/// Amostras mínimas da batalha para a fração fazer sentido.
const MIN_BATTLE_POINTS: i32 = 10;
/// Janela da largada: comparamos a posição inicial com a de +20s.
const START_WINDOW_S: f64 = 20.0;
/// Span mínimo de amostras para o `start_delta` valer numa corrida curta.
const START_MIN_SPAN_S: f64 = 15.0;
/// Escala do coeficiente de variação → nota de consistência (cv 0 → 100).
const CONSISTENCY_CV_SCALE: f64 = 2000.0;

/// Sinais COMPACTOS de telemetria de uma corrida do jogador, para o dossiê de
/// habilidade (Fase 2). Um valor por corrida; o estimador agrega. Sentinela
/// `-1.0` (ou `start_valid=false`) = não computável nesta corrida.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerRaceTelemetry {
    pub laps_seen: i32,
    pub race_laps: i32,
    /// 0–100: quão parelhos foram os tempos de volta. -1 = poucas voltas.
    pub consistency: f64,
    /// 0–1: fração das amostras com um carro a < 1s. -1 = poucas amostras.
    pub battle_fraction: f64,
    /// Posições SUBIDAS na pista (bruto, do fluxo de posição).
    pub on_track_gained: i32,
    /// Posições PERDIDAS na pista (bruto).
    pub on_track_lost: i32,
    /// Posições ganhas nos ~20s iniciais (posição no start − posição em +20s).
    pub start_delta: i32,
    /// `start_delta` é confiável (houve amostra cobrindo a janela de largada).
    pub start_valid: bool,
}

/// Extrai os sinais do dossiê do histórico ao vivo + a análise já feita. `None`
/// quando não há nada utilizável (jogador não correu / não foi monitorado).
pub fn extract_player_race_telemetry(
    history: &RaceHistory,
    telemetry: &TelemetryAnalysis,
) -> Option<PlayerRaceTelemetry> {
    let times: Vec<f64> = history
        .player_laps
        .iter()
        .map(|l| l.time)
        .filter(|t| *t > 0.0)
        .collect();
    let laps_seen = times.len() as i32;
    let has_track = !history.player_track.is_empty();
    if laps_seen == 0 && !has_track {
        return None;
    }

    // Consistência: coeficiente de variação dos tempos de volta (menor = melhor).
    let consistency = if laps_seen >= MIN_CONSISTENCY_LAPS {
        let m = mean(&times);
        if m > 0.0 {
            let var = times.iter().map(|t| (t - m).powi(2)).sum::<f64>() / times.len() as f64;
            let cv = var.sqrt() / m;
            (100.0 - cv * CONSISTENCY_CV_SCALE).clamp(1.0, 100.0)
        } else {
            -1.0
        }
    } else {
        -1.0
    };

    // Fração de tempo em briga: amostras com um vizinho a < BATTLE_GAP_S.
    let mut battle = 0;
    let mut pts = 0;
    for p in &history.player_track {
        let mut nearest = f64::INFINITY;
        if p.ahead_idx >= 0 && p.gap_ahead.is_finite() && p.gap_ahead >= 0.0 {
            nearest = nearest.min(p.gap_ahead);
        }
        if p.behind_idx >= 0 && p.gap_behind.is_finite() && p.gap_behind >= 0.0 {
            nearest = nearest.min(p.gap_behind);
        }
        if nearest.is_finite() {
            pts += 1;
            if nearest < BATTLE_GAP_S {
                battle += 1;
            }
        }
    }
    let battle_fraction = if pts >= MIN_BATTLE_POINTS {
        battle as f64 / pts as f64
    } else {
        -1.0
    };

    // Saldo bruto de posições na pista (do fluxo de posição já calculado).
    let (on_track_gained, on_track_lost) = telemetry
        .position_flow
        .as_ref()
        .map(|f| (f.gained_on_track, f.lost_on_track))
        .unwrap_or((0, 0));

    let (start_delta, start_valid) = compute_start_delta(history);

    Some(PlayerRaceTelemetry {
        laps_seen,
        race_laps: telemetry.race_laps,
        consistency,
        battle_fraction,
        on_track_gained,
        on_track_lost,
        start_delta,
        start_valid,
    })
}

/// Posições ganhas na largada: posição na 1ª amostra vs. a de ~20s depois
/// (positivo = subiu). Fallback: numa corrida curta, usa a última amostra se o
/// span cobrir ao menos [`START_MIN_SPAN_S`].
fn compute_start_delta(history: &RaceHistory) -> (i32, bool) {
    let mut pts: Vec<(f64, i32)> = history
        .player_track
        .iter()
        .filter(|p| p.position > 0)
        .map(|p| (p.session_time, p.position))
        .collect();
    if pts.len() < 2 {
        return (0, false);
    }
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (t0, pos0) = pts[0];
    if let Some(&(_, pos20)) = pts.iter().find(|(t, _)| *t >= t0 + START_WINDOW_S) {
        return (pos0 - pos20, true);
    }
    let (tl, posl) = *pts.last().unwrap();
    if tl - t0 >= START_MIN_SPAN_S {
        (pos0 - posl, true)
    } else {
        (0, false)
    }
}
