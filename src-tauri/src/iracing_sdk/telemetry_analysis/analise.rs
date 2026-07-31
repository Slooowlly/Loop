//! O orquestrador: `analyze` cruza os submódulos (ritmo, rival, fluxo, erro, melhor
//! momento, gráficos, pneu, combustível, setores) num único `TelemetryAnalysis`.

use std::collections::HashMap;

use crate::iracing_sdk::race_monitor::RaceHistory;

use super::combustivel::analyze_fuel;
use super::graficos::build_charts;
use super::momentos::{analyze_best_moment, analyze_mistake};
use super::ritmo::{analyze_pace, analyze_position_flow, analyze_rival, confidence_label};
use super::setores::analyze_sectors;
use super::tipos::{PlayerIncidents, TelemetryAnalysis};

/// Analisa o histórico. `name_by_idx`: car_idx → nome do piloto (resolvido fora).
/// `incidents`: sinais de batida/DNF do monitor (fora do `RaceHistory`).
pub fn analyze(
    history: &RaceHistory,
    name_by_idx: &HashMap<i32, String>,
    team_by_idx: &HashMap<i32, String>,
    incidents: &PlayerIncidents,
) -> TelemetryAnalysis {
    let player_idx = history.player_car_idx;
    let pace = analyze_pace(history, player_idx);
    let rival = analyze_rival(history, name_by_idx);
    let position_flow = analyze_position_flow(history);
    let mistake = analyze_mistake(history, incidents, pace.as_ref());
    let best_moment = analyze_best_moment(
        history,
        name_by_idx,
        pace.as_ref(),
        incidents,
        mistake.as_ref(),
    );
    let charts = build_charts(history, name_by_idx);
    let laps_seen = history.player_laps.len() as i32;
    let last_lap_seen = history.player_laps.iter().map(|l| l.lap).max().unwrap_or(0);
    // Voltas totais da corrida = última volta do líder no race trace.
    let race_laps = history.laps.iter().map(|s| s.lap).max().unwrap_or(0);

    let (confidence, is_partial) = confidence_label(laps_seen, race_laps);

    // Estratégia de pneu (todos os carros) a partir das paradas + clima da corrida.
    // Resolve os nomes dos pilotos aqui (o módulo puro só conhece o car_idx).
    let mut tire_strategies =
        crate::iracing_sdk::tire_strategy::infer_all(&history.pit_stops, history.weather);
    for s in &mut tire_strategies {
        s.pilot_name = name_by_idx
            .get(&s.car_idx)
            .cloned()
            .unwrap_or_else(|| format!("Carro {}", s.car_idx));
        s.team_name = team_by_idx.get(&s.car_idx).cloned().unwrap_or_default();
    }
    let player_tire = tire_strategies
        .iter()
        .find(|s| s.car_idx == player_idx)
        .cloned();

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
        tire_strategies,
        player_tire,
        fuel: analyze_fuel(history),
        sectors: analyze_sectors(history),
    }
}
