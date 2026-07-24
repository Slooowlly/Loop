//! Agregações sobre o histórico e os eventos do monitor: melhor volta, voltas
//! completas, grid derivado da quali, DNFs e incidentes da IA, e a pior batida
//! do jogador.

use std::collections::HashMap;

use crate::iracing_sdk::race_monitor::{severity_rank, Attempt, RaceEvent, RaceHistory, RaceStatus};

/// Melhor volta (em ms) de cada carro, a partir de `car_laps` (tempos em segundos).
/// Slots com `time <= 0` (sem volta válida) são ignorados.
pub(super) fn best_lap_ms_by_car(history: &RaceHistory) -> HashMap<i32, f64> {
    let mut best: HashMap<i32, f64> = HashMap::new();
    for lap in &history.car_laps {
        if lap.time > 0.0 {
            best.entry(lap.car_idx)
                .and_modify(|t| *t = t.min(lap.time))
                .or_insert(lap.time);
        }
    }
    best.into_iter().map(|(idx, s)| (idx, s * 1000.0)).collect()
}

/// Voltas completas de cada carro (maior `lap` visto em `car_laps`).
pub(super) fn laps_completed_by_car(history: &RaceHistory) -> HashMap<i32, i32> {
    let mut laps: HashMap<i32, i32> = HashMap::new();
    for lap in &history.car_laps {
        laps.entry(lap.car_idx)
            .and_modify(|l| *l = (*l).max(lap.lap))
            .or_insert(lap.lap);
    }
    laps
}

/// Grid (posição de largada) por carro, RELATIVO À CLASSE — coerente com o
/// `class_position` de chegada que o iRacing reporta. Deriva da melhor volta da
/// quali (`qualy_laps`); carros sem volta de quali vão ao fundo do grid da classe.
/// Sem nenhuma quali capturada, devolve vazio (o chamador usa grid = chegada).
pub(super) fn grid_by_car(history: &RaceHistory) -> HashMap<i32, i32> {
    if history.qualy_laps.is_empty() {
        return HashMap::new();
    }

    // Melhor volta de quali por carro.
    let mut quali_best: HashMap<i32, f64> = HashMap::new();
    for lap in &history.qualy_laps {
        if lap.time > 0.0 {
            quali_best
                .entry(lap.car_idx)
                .and_modify(|t| *t = t.min(lap.time))
                .or_insert(lap.time);
        }
    }

    // Classe de cada carro (para rankear o grid dentro da classe).
    let class_of: HashMap<i32, i64> = history
        .cars_meta
        .iter()
        .filter(|m| !m.is_pace)
        .map(|m| (m.idx, m.class_id))
        .collect();

    // Agrupa carros (com tempo de quali) por classe e ordena por tempo.
    let mut by_class: HashMap<i64, Vec<(i32, f64)>> = HashMap::new();
    for (idx, time) in &quali_best {
        let class = class_of.get(idx).copied().unwrap_or(0);
        by_class.entry(class).or_default().push((*idx, *time));
    }

    let mut grid: HashMap<i32, i32> = HashMap::new();
    for cars in by_class.values_mut() {
        cars.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        for (pos, (idx, _)) in cars.iter().enumerate() {
            grid.insert(*idx, pos as i32 + 1);
        }
    }
    grid
}

/// Carros (idx) que abandonaram, com o motivo, a partir dos eventos do monitor.
/// IA: eventos `dnf_confirmed` carregam `car_idx` + `detail`. O jogador é tratado
/// à parte (via `attempts`), então eventos sem `car_idx` são ignorados aqui.
pub(super) fn ai_dnfs(events: &[RaceEvent]) -> HashMap<i32, String> {
    let mut out: HashMap<i32, String> = HashMap::new();
    for ev in events {
        if ev.kind == "dnf_confirmed" {
            if let Some(idx) = ev.car_idx {
                out.entry(idx).or_insert_with(|| ev.detail.clone());
            }
        }
    }
    out
}

/// Pior incidente da IA por carro (maior severidade entre os eventos do carro).
/// Devolve `(severidade, detalhe)`.
pub(super) fn ai_worst_incident(events: &[RaceEvent]) -> HashMap<i32, (String, String)> {
    let mut out: HashMap<i32, (String, String)> = HashMap::new();
    for ev in events {
        let Some(idx) = ev.car_idx else { continue };
        let Some(sev) = ev.severity.as_deref() else {
            continue;
        };
        let better = out
            .get(&idx)
            .map(|(cur, _)| severity_rank(sev) > severity_rank(cur))
            .unwrap_or(true);
        if better {
            out.insert(idx, (sev.to_string(), ev.detail.clone()));
        }
    }
    out
}

/// Pior severidade de batida do JOGADOR na corrida (label: leve/moderado/grave/
/// destruído/catastrófico, ou "nenhum"). Usa o PICO do score de batida da
/// tentativa (`peak_crash_score`) — atualizado ao vivo todo tick — em vez de
/// `crashes`, que só registra quando a batida "fecha" (e some se o jogador bate e
/// sai). Cruza com `crashes` por garantia (pega o que for maior). Base do conserto.
pub fn player_worst_severity(status: &RaceStatus, attempt_number: i32) -> String {
    use crate::iracing_sdk::race_monitor::severity_label;
    let Some(attempt) = player_attempt(status, attempt_number) else {
        return "nenhum".to_string();
    };
    // Severidade do pico ao vivo.
    let peak_sev = severity_label(attempt.peak_crash_score);
    // Maior severidade entre as batidas já fechadas (defensivo).
    let closed_sev = attempt
        .crashes
        .iter()
        .max_by_key(|c| severity_rank(&c.severity))
        .map(|c| c.severity.as_str())
        .unwrap_or("nenhum");
    if severity_rank(peak_sev) >= severity_rank(closed_sev) {
        peak_sev.to_string()
    } else {
        closed_sev.to_string()
    }
}

/// A tentativa do jogador que este histórico cobre (casa pelo número da
/// tentativa; cai para a última se não achar).
pub(super) fn player_attempt<'a>(
    status: &'a RaceStatus,
    attempt_number: i32,
) -> Option<&'a Attempt> {
    status
        .attempts
        .iter()
        .find(|a| a.number == attempt_number)
        .or_else(|| status.attempts.last())
}
