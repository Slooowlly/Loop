//! Desfecho e agregação do resultado: a ponte para a camada adaptativa e os
//! helpers que traduzem severidade/status em prosa de DNF.

use super::*;

/// Converte o histórico capturado no [`RaceResult`](crate::iracing_sdk::adaptive::RaceResult)
/// que a camada adaptativa consome (a ponte Fase A → Fase B). `track_id` vem de
/// quem chama (o export sabe a pista). As voltas de TODOS os carros já estão em
/// `car_laps`; aqui só agrupamos por carro e marcamos jogador/DNF.
pub fn build_adaptive_result(history: &RaceHistory, track_id: i64) -> crate::iracing_sdk::adaptive::RaceResult {
    use crate::iracing_sdk::adaptive::{DriverData, Lap, RaceResult};
    let player_idx = history.player_car_idx;
    let player_dnf = history.outcome.to_lowercase().contains("dnf");
    // Monta os pilotos a partir de um conjunto de voltas (corrida OU quali),
    // reusando o resumo por carro (classe/IA/posição). dnf só vale na corrida.
    let build = |laps_src: &[CarLap], dnf_applies: bool| -> Vec<DriverData> {
        history
            .cars_meta
            .iter()
            .filter(|m| !m.is_pace)
            .map(|m| {
                let laps: Vec<Lap> = laps_src
                    .iter()
                    .filter(|l| l.car_idx == m.idx)
                    .map(|l| Lap {
                        lap: l.lap,
                        time: l.time,
                    })
                    .collect();
                let is_player = m.idx == player_idx;
                DriverData {
                    car_idx: m.idx,
                    is_player,
                    is_ai: m.is_ai,
                    car_class_id: m.class_id,
                    finish_pos_in_class: m.class_position,
                    dnf: is_player && dnf_applies && player_dnf,
                    laps,
                }
            })
            .collect()
    };
    let race = build(&history.car_laps, true);
    let qualy = if history.qualy_laps.is_empty() {
        None
    } else {
        Some(build(&history.qualy_laps, false))
    };
    RaceResult {
        track_id,
        yellow_laps: history.yellow_laps.clone(),
        race,
        qualy,
    }
}

// ─── Helpers de desfecho ─────────────────────────────────────────────────────
/// Posto da severidade (0 = "nenhum", 5 = "catastrófico"). Usado para comparar
/// batidas — pela ponte de resultado e pelo cálculo de conserto do carro.
pub(crate) fn severity_rank(severity: &str) -> usize {
    SEVERITIES
        .iter()
        .position(|s| *s == severity)
        .map(|i| i + 1)
        .unwrap_or(0)
}

pub(crate) fn status_pt(status: &str) -> &'static str {
    match status {
        "active" => "Ativa",
        "finished" => "Finalizada",
        "dnf" => "DNF",
        "not_started" => "Não largou",
        _ => "Desconhecido",
    }
}

/// Motivo do DNF: cita a PIOR batida (se houve) + como encerrou.
pub(crate) fn build_dnf_reason(attempt: &Attempt, ev: &AttemptEvidence, ended_by: &str) -> String {
    let how = match ended_by {
        "restart" => "reiniciou sem terminar",
        "sim_closed" => "fechou o jogo / saiu sem terminar",
        _ => "encerrou sem terminar",
    };
    let worst = attempt
        .crashes
        .iter()
        .max_by_key(|c| severity_rank(&c.severity));
    if let Some(crash) = worst {
        let detail = crash
            .factors
            .iter()
            .find(|f| f.starts_with("perdeu"))
            .cloned()
            .unwrap_or_else(|| crash.factors.join(", "));
        format!(
            "Abandonou após batida {} na volta {} ({}); {how}.",
            crash.severity.to_uppercase(),
            crash.lap,
            detail
        )
    } else {
        // Sem batida: descreve pela evidência.
        let mut parts: Vec<&str> = Vec::new();
        if ev.disqualified {
            parts.push("desqualificado");
        }
        if ev.garage {
            parts.push("foi para a garagem");
        }
        if ev.off_track || ev.not_in_world {
            parts.push("saiu da pista");
        }
        if parts.is_empty() {
            format!("DNF — {how} (sem batida registrada).")
        } else {
            format!("DNF — {how}. {}.", parts.join(", "))
        }
    }
}
