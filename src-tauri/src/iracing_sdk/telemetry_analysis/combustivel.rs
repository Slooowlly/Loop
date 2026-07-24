//! Consumo de combustível do jogador na corrida, a partir das voltas com o tanque captado.

use serde::Serialize;

use crate::iracing_sdk::race_monitor::RaceHistory;

/// Consumo de combustível do jogador na corrida (litros).
#[derive(Debug, Clone, Serialize)]
pub struct FuelSummary {
    /// Consumo médio por volta (L).
    pub used_per_lap_l: f64,
    /// Combustível restante na última volta captada (L).
    pub remaining_l: f64,
    /// Voltas de autonomia restantes no ritmo atual (remaining / used_per_lap).
    pub laps_left: f64,
}

/// Consumo do jogador a partir das voltas com combustível captado. Precisa de ≥2
/// voltas com `fuel_remaining >= 0` e consumo positivo (ruído/reabastecimento fora).
pub(super) fn analyze_fuel(history: &RaceHistory) -> Option<FuelSummary> {
    let mut pts: Vec<(f64, f64)> = history
        .player_laps
        .iter()
        .filter(|l| l.fuel_remaining >= 0.0)
        .map(|l| (l.lap as f64, l.fuel_remaining))
        .collect();
    if pts.len() < 2 {
        return None;
    }
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (lap0, fuel0) = pts[0];
    let (lap1, fuel1) = *pts.last().unwrap();
    let lap_span = lap1 - lap0;
    let burned = fuel0 - fuel1;
    if lap_span < 1.0 || burned <= 0.0 {
        return None;
    }
    let used_per_lap_l = burned / lap_span;
    let laps_left = if used_per_lap_l > 0.0 {
        fuel1 / used_per_lap_l
    } else {
        0.0
    };
    Some(FuelSummary {
        used_per_lap_l,
        remaining_l: fuel1,
        laps_left,
    })
}
