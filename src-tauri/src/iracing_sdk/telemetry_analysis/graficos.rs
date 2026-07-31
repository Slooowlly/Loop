//! Montagem das séries dos gráficos (race trace, tempos de volta, gap ao rival) a
//! partir do histórico ao vivo do monitor.

use std::collections::HashMap;

use crate::iracing_sdk::race_monitor::RaceHistory;

use super::ritmo::find_rival;
use super::tipos::{
    ChartCar, ChartCarLapTime, ChartGap, ChartLapTime, ChartTracePoint, RaceCharts,
};

/// Monta as séries dos gráficos a partir do histórico ao vivo. Resolve nomes via
/// `name_by_idx` (fallback "Carro N"). None se não há trace nem voltas.
pub(super) fn build_charts(
    history: &RaceHistory,
    name_by_idx: &HashMap<i32, String>,
) -> Option<RaceCharts> {
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
                    snap.cars
                        .iter()
                        .find(|c| c.idx == idx)
                        .map(|c| ChartTracePoint {
                            lap: snap.lap as f64 + snap.progress as f64,
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
