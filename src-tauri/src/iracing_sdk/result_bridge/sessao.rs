//! Reconstrução do [`RaceResult`] a partir da SESSÃO ao vivo (histórico do
//! monitor + status), usada na validação/preview da corrida.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::iracing_sdk::race_monitor::{RaceHistory, RaceStatus};
use crate::models::driver::Driver;
use crate::simulation::qualifying::QualifyingResult;
use crate::simulation::race::{ClassificationStatus, RaceDriverResult, RaceResult};

use super::agregacao::{
    ai_dnfs, ai_worst_incident, best_lap_ms_by_car, grid_by_car, laps_completed_by_car,
    player_attempt,
};
use super::identidade::resolve_identity;

/// Constrói um [`RaceResult`] a partir da sessão do iRacing já encerrada.
///
/// `by_number`: número do carro → `driver_id` (mapa salvo na geração do roster).
/// `player_driver`: o piloto-jogador da carreira (excluído do roster da IA).
pub fn build_race_result_from_session(
    history: &RaceHistory,
    status: &RaceStatus,
    conn: &Connection,
    by_number: &HashMap<i64, String>,
    player_driver: Option<&Driver>,
    weather: &str,
    track_name: &str,
) -> RaceResult {
    let best_lap_ms = best_lap_ms_by_car(history);
    let laps_done = laps_completed_by_car(history);
    let grid = grid_by_car(history);
    let ai_dnf = ai_dnfs(&status.events);
    let ai_incident = ai_worst_incident(&status.events);

    // DNF + batida do jogador, da sua tentativa.
    let p_attempt = player_attempt(status, history.attempt_number);
    let player_dnf = p_attempt.map(|a| a.status == "dnf").unwrap_or(false);
    let player_dnf_reason = p_attempt.and_then(|a| a.reason.clone());
    let player_worst_crash = p_attempt.and_then(|a| a.worst_crash.clone());
    let player_incidents = p_attempt.map(|a| a.crashes.len() as i32).unwrap_or(0);

    // Carro com a melhor volta absoluta → autor da volta mais rápida.
    let fastest_idx = best_lap_ms
        .iter()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(idx, _)| *idx);

    // Classe do jogador (para definir vencedor/pole "do jogador" em multiclasse).
    let player_class_id = history
        .cars_meta
        .iter()
        .find(|m| m.idx == history.player_car_idx)
        .map(|m| m.class_id);

    // Gap ao líder (ms) por carro, do último snapshot de volta.
    let last_gaps: HashMap<i32, f64> = history
        .laps
        .last()
        .map(|snap| snap.cars.iter().map(|c| (c.idx, c.gap * 1000.0)).collect())
        .unwrap_or_default();

    let mut race_results: Vec<RaceDriverResult> = Vec::new();
    let mut qualifying_results: Vec<QualifyingResult> = Vec::new();

    for meta in history.cars_meta.iter().filter(|m| !m.is_pace) {
        let is_player = meta.idx == history.player_car_idx;
        let id = resolve_identity(conn, meta.car_number, is_player, player_driver, by_number);

        // Chegada na classe; não classificados (≤0) vão para o fundo (sentinela só
        // de ORDENAÇÃO — nunca aparece como número de posição ao usuário).
        let classified = meta.class_position >= 1;
        let finish_position = if classified {
            meta.class_position
        } else {
            i32::MAX / 2
        };
        // Grid: snapshot da largada (verde) > ranking da quali > desconhecido (0).
        let grid_position = if meta.grid_class_position > 0 {
            meta.grid_class_position
        } else {
            grid.get(&meta.idx).copied().unwrap_or(0)
        };
        // Posições ganhas só faz sentido com chegada classificada E grid conhecido.
        let positions_gained = if classified && grid_position > 0 {
            grid_position - finish_position
        } else {
            0
        };

        let is_dnf = if is_player {
            player_dnf
        } else {
            ai_dnf.contains_key(&meta.idx) || meta.class_position <= 0
        };
        let dnf_reason = if is_player {
            player_dnf_reason.clone()
        } else {
            ai_dnf.get(&meta.idx).cloned()
        };
        let notable_incident = if is_player {
            player_worst_crash.clone()
        } else {
            ai_incident.get(&meta.idx).map(|(_, d)| d.clone())
        };
        let incidents_count = if is_player {
            player_incidents
        } else {
            ai_incident.contains_key(&meta.idx) as i32
        };

        let has_fastest_lap = Some(meta.idx) == fastest_idx;

        race_results.push(RaceDriverResult {
            pilot_id: id.driver_id.clone(),
            pilot_name: id.driver_name.clone(),
            team_id: id.team_id.clone(),
            team_name: id.team_name.clone(),
            grid_position,
            finish_position,
            positions_gained,
            best_lap_time_ms: best_lap_ms.get(&meta.idx).copied().unwrap_or(0.0),
            total_race_time_ms: 0.0,
            gap_to_winner_ms: last_gaps.get(&meta.idx).copied().unwrap_or(0.0),
            is_dnf,
            dnf_reason,
            dnf_segment: None,
            incidents_count,
            incidents: Vec::new(),
            has_fastest_lap,
            points_earned: 0,
            is_jogador: is_player,
            laps_completed: laps_done.get(&meta.idx).copied().unwrap_or(0),
            final_tire_wear: 1.0,
            final_physical: 1.0,
            classification_status: if is_dnf {
                ClassificationStatus::Dnf
            } else {
                ClassificationStatus::Finished
            },
            notable_incident,
            dnf_catalog_id: None,
            damage_origin_segment: None,
            // Sessão REAL do iRacing: sem dado de posição na pista trecho a trecho.
            posicoes_por_segmento: Vec::new(),
            gaps_para_da_frente_ms: Vec::new(),
            segmentos_em_ar_sujo: 0,
            tentativas_ultrapassagem: 0,
            ultrapassagens_concluidas: 0,
            tentativas_sofridas: 0,
            maior_sequencia_preso: 0,
            // Corrida REAL do iRacing: estratégia de parada é subproduto da nossa simulação.
            volta_da_parada: Vec::new(),
            posicao_antes_da_parada: Vec::new(),
            posicao_depois: Vec::new(),
            estrategia_id: String::new(),
        });

        qualifying_results.push(QualifyingResult {
            pilot_id: id.driver_id,
            pilot_name: id.driver_name,
            team_id: id.team_id,
            team_name: id.team_name,
            position: grid_position,
            quali_score: 0.0,
            best_lap_time_ms: 0.0,
            gap_to_pole_ms: 0.0,
            is_pole: false,
            is_jogador: is_player,
            // Sessão REAL do iRacing: a grade é a de verdade, não a nossa simulada.
            volta_perdida: false,
        });
    }

    // Vencedor e pole DA CLASSE DO JOGADOR (em corrida de classe única, é o P1).
    let in_player_class = |idx_class: i64| player_class_id.map(|c| c == idx_class).unwrap_or(true);
    let class_of: HashMap<String, i64> = history
        .cars_meta
        .iter()
        .filter(|m| !m.is_pace)
        .filter_map(|m| {
            let is_player = m.idx == history.player_car_idx;
            let id = if is_player {
                player_driver.map(|d| d.id.clone())
            } else {
                by_number.get(&(m.car_number as i64)).cloned()
            };
            id.map(|id| (id, m.class_id))
        })
        .collect();

    let winner_id = race_results
        .iter()
        .filter(|r| !r.is_dnf && in_player_class(class_of.get(&r.pilot_id).copied().unwrap_or(0)))
        .min_by_key(|r| r.finish_position)
        .map(|r| r.pilot_id.clone())
        .unwrap_or_default();
    let pole_sitter_id = race_results
        .iter()
        .filter(|r| in_player_class(class_of.get(&r.pilot_id).copied().unwrap_or(0)))
        .min_by_key(|r| r.grid_position)
        .map(|r| r.pilot_id.clone())
        .unwrap_or_default();
    if let Some(q) = qualifying_results
        .iter_mut()
        .find(|q| q.pilot_id == pole_sitter_id)
    {
        q.is_pole = true;
    }

    let fastest_lap_id = fastest_idx
        .and_then(|idx| {
            history
                .cars_meta
                .iter()
                .find(|m| m.idx == idx)
                .map(|m| (m.idx == history.player_car_idx, m.car_number))
        })
        .and_then(|(is_player, num)| {
            if is_player {
                player_driver.map(|d| d.id.clone())
            } else {
                by_number.get(&(num as i64)).cloned()
            }
        })
        .unwrap_or_default();

    let total_dnfs = race_results.iter().filter(|r| r.is_dnf).count() as i32;
    let total_incidents = race_results.iter().map(|r| r.incidents_count).sum();
    let most_positions_gained_id = race_results
        .iter()
        .filter(|r| !r.is_dnf)
        .max_by_key(|r| r.positions_gained)
        .filter(|r| r.positions_gained > 0)
        .map(|r| r.pilot_id.clone());
    let notable_incident_pilot_ids: Vec<String> = race_results
        .iter()
        .filter(|r| r.notable_incident.is_some())
        .map(|r| r.pilot_id.clone())
        .collect();

    let total_laps = laps_done.values().copied().max().unwrap_or(0);

    RaceResult {
        qualifying_results,
        race_results,
        pole_sitter_id,
        winner_id,
        fastest_lap_id,
        total_laps,
        weather: weather.to_string(),
        track_name: track_name.to_string(),
        total_incidents,
        total_dnfs,
        main_incident_count: notable_incident_pilot_ids.len() as i32,
        notable_incident_pilot_ids,
        most_positions_gained_id,
        // Vazio de propósito: em corrida IMPORTADA a amarela não é derivada, ela é o
        // dado REAL do SessionFlags e entra pelos fatos de telemetria. Derivar aqui
        // faria a revista contar a mesma neutralização duas vezes.
        caution_segments: Vec::new(),
        // Corrida AO VIVO: a quebra vem do log do disparo (`!black`/`!dq`), não da simulação.
        applied_mechanicals: Vec::new(),
        safety_cars: Vec::new(),
        ordem_pre_safety_car: Vec::new(),
    }
}
