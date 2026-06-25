//! Ponte sessão iRacing → [`RaceResult`] da simulação.
//!
//! O app de carreira é, no fundo, uma máquina que consome [`RaceResult`]: a
//! persistência de standings/pontos, as lesões, o filtro de narrativa e as
//! notícias TODOS leem essa struct. A simulação offline a PRODUZ; este módulo a
//! produz a partir de uma corrida REAL disputada no iRacing — assim o jogador
//! corre na pista e a carreira reage exatamente como se o motor offline tivesse
//! rodado a etapa.
//!
//! Duas fontes alimentam a ponte (ambas do `race_monitor`):
//! - [`RaceHistory`] (`get_history`): posições finais (`cars_meta`), voltas de
//!   cada carro (`car_laps` → melhor volta), voltas da quali (`qualy_laps` →
//!   grid) e o gap ao líder (`laps`).
//! - [`RaceStatus`] (`poll`): `attempts` (DNF + batida do jogador) e `events`
//!   (DNF e severidade da IA).
//!
//! Escopo desta fatia: reconstruir o resultado para VALIDAÇÃO (preview read-only).
//! A persistência na carreira, as lesões a partir da severidade e o conserto do
//! carro entram numa fatia seguinte — mas já consomem o [`RaceResult`] daqui.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::db::queries::{contracts, teams};
use crate::iracing_sdk::race_monitor::{
    severity_rank, Attempt, CarMeta, RaceEvent, RaceHistory, RaceStatus,
};
use crate::models::driver::Driver;
use crate::simulation::qualifying::QualifyingResult;
use crate::simulation::race::{ClassificationStatus, RaceDriverResult, RaceResult};

/// Identidade resolvida de um carro da sessão (piloto + equipe da carreira).
struct CarIdentity {
    driver_id: String,
    driver_name: String,
    team_id: String,
    team_name: String,
}

/// Resolve `driver_id`/nome/equipe de um carro. O jogador vem do
/// `player_driver`; a IA vem do mapa número→`driver_id` salvo na geração do
/// roster. Sem casamento, devolve um placeholder estável (não quebra o pipeline).
fn resolve_identity(
    conn: &Connection,
    car_number: i32,
    is_player: bool,
    player_driver: Option<&Driver>,
    by_number: &HashMap<i64, String>,
) -> CarIdentity {
    let driver_id = if is_player {
        player_driver.map(|d| d.id.clone())
    } else {
        by_number.get(&(car_number as i64)).cloned()
    };

    let driver_id = match driver_id {
        Some(id) => id,
        None => {
            return CarIdentity {
                driver_id: format!("car-{}", car_number),
                driver_name: format!("Carro #{}", car_number),
                team_id: String::new(),
                team_name: "—".to_string(),
            };
        }
    };

    // Nome: jogador já em mãos; IA busca no banco.
    let driver_name = if is_player {
        player_driver.map(|d| d.nome.clone())
    } else {
        crate::db::queries::drivers::get_driver(conn, &driver_id)
            .ok()
            .map(|d| d.nome)
    }
    .unwrap_or_else(|| format!("Carro #{}", car_number));

    // Equipe: contrato regular ativo → equipe.
    let (team_id, team_name) = contracts::get_active_regular_contract_for_pilot(conn, &driver_id)
        .ok()
        .flatten()
        .map(|c| {
            let name = teams::get_team_by_id(conn, &c.equipe_id)
                .ok()
                .flatten()
                .map(|t| t.nome)
                .unwrap_or_else(|| c.equipe_id.clone());
            (c.equipe_id, name)
        })
        .unwrap_or_else(|| (String::new(), "—".to_string()));

    CarIdentity {
        driver_id,
        driver_name,
        team_id,
        team_name,
    }
}

/// Melhor volta (em ms) de cada carro, a partir de `car_laps` (tempos em segundos).
/// Slots com `time <= 0` (sem volta válida) são ignorados.
fn best_lap_ms_by_car(history: &RaceHistory) -> HashMap<i32, f64> {
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
fn laps_completed_by_car(history: &RaceHistory) -> HashMap<i32, i32> {
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
fn grid_by_car(history: &RaceHistory) -> HashMap<i32, i32> {
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
fn ai_dnfs(events: &[RaceEvent]) -> HashMap<i32, String> {
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
fn ai_worst_incident(events: &[RaceEvent]) -> HashMap<i32, (String, String)> {
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
fn player_attempt<'a>(status: &'a RaceStatus, attempt_number: i32) -> Option<&'a Attempt> {
    status
        .attempts
        .iter()
        .find(|a| a.number == attempt_number)
        .or_else(|| status.attempts.last())
}

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
        .map(|snap| {
            snap.cars
                .iter()
                .map(|c| (c.idx, c.gap * 1000.0))
                .collect()
        })
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
        .filter(|r| {
            !r.is_dnf && in_player_class(class_of.get(&r.pilot_id).copied().unwrap_or(0))
        })
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
    }
}

/// Constrói um [`RaceResult`] a partir do RESULTADO OFICIAL do iRacing (JSON do
/// aiseason). É a fonte autoritativa e persistida — robusta a o jogador sair cedo.
///
/// `player_custid`: o `cust_id` do jogador (de `cached_custid`) — casa a sua linha.
/// `by_number`: número do carro → `driver_id` (mapa do roster) para a IA.
pub fn build_race_result_from_aiseason(
    event: &crate::iracing_sdk::aiseason_results::AiEventResult,
    conn: &Connection,
    by_number: &HashMap<i64, String>,
    player_custid: i64,
    player_driver: Option<&Driver>,
    player_dnf: bool,
    player_collided_with_id: Option<&str>,
    extra_dnf_numbers: &std::collections::HashSet<i32>,
    weather: &str,
    track_name: &str,
) -> RaceResult {
    // Volta mais rápida = menor best_lap_time válido (cust_id é único por carro).
    let fastest_cust = event
        .rows
        .iter()
        .filter(|r| r.best_lap_time_ms > 0.0)
        .min_by(|a, b| {
            a.best_lap_time_ms
                .partial_cmp(&b.best_lap_time_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|r| r.cust_id);
    // Tempo da quali por piloto (cust_id) — fonte do tempo da pole no painel.
    let quali_best: HashMap<i64, f64> = event
        .qualify
        .iter()
        .filter(|r| r.best_lap_time_ms > 0.0)
        .map(|r| (r.cust_id, r.best_lap_time_ms))
        .collect();

    // Voltas do líder (referência da classificação). Quem completou MENOS DA
    // METADE disso não "terminou" — bateu/saiu e foi AI-completado lá atrás. É a
    // regra padrão de classificação e pega o caso óbvio (8 voltas atrás = DNF),
    // direto do JSON, sem depender do monitor ao vivo.
    let leader_laps = event
        .laps_complete
        .max(event.rows.iter().map(|r| r.laps_complete).max().unwrap_or(0));

    let mut race_results: Vec<RaceDriverResult> = Vec::new();
    let mut qualifying_results: Vec<QualifyingResult> = Vec::new();
    let mut fastest_lap_id = String::new();

    for row in &event.rows {
        let is_player = player_custid != 0 && row.cust_id == player_custid;
        let id = resolve_identity(conn, row.car_number, is_player, player_driver, by_number);

        let grid_position = row.grid_position;
        let finish_position = row.class_position.max(1);
        let positions_gained = if grid_position > 0 {
            grid_position - finish_position
        } else {
            0
        };
        let has_fastest_lap = fastest_cust == Some(row.cust_id);
        if has_fastest_lap {
            fastest_lap_id = id.driver_id.clone();
        }
        // DNF: o JSON marca todo mundo "Running" (o iRacing AI-completa a corrida
        // quando o jogador sai). Marcamos abandono por VÁRIOS sinais:
        //  - reason_out do JSON (raro, mas existe);
        //  - menos da metade das voltas do líder (bateu/saiu — pega o caso óbvio);
        //  - jogador que correu mas não cruzou a bandeira (monitor ao vivo);
        //  - carro que o monitor confirmou ter abandonado.
        let laps_down_dnf = leader_laps > 0 && row.laps_complete * 2 < leader_laps;
        let is_dnf = row.is_dnf()
            || laps_down_dnf
            || (is_player && player_dnf)
            || extra_dnf_numbers.contains(&row.car_number);
        let dnf_reason = if !is_dnf {
            None
        } else if row.is_dnf() {
            Some(row.reason_out.clone())
        } else if is_player && player_dnf || extra_dnf_numbers.contains(&row.car_number) {
            Some("Batida".to_string())
        } else {
            Some("Voltas atrás".to_string())
        };

        race_results.push(RaceDriverResult {
            pilot_id: id.driver_id.clone(),
            pilot_name: id.driver_name.clone(),
            team_id: id.team_id.clone(),
            team_name: id.team_name.clone(),
            grid_position,
            finish_position,
            positions_gained,
            best_lap_time_ms: row.best_lap_time_ms.max(0.0),
            total_race_time_ms: 0.0,
            gap_to_winner_ms: row.interval_ms.max(0.0),
            is_dnf,
            dnf_reason,
            dnf_segment: None,
            incidents_count: row.incidents,
            incidents: Vec::new(),
            has_fastest_lap,
            points_earned: 0,
            is_jogador: is_player,
            laps_completed: row.laps_complete,
            final_tire_wear: 1.0,
            final_physical: 1.0,
            classification_status: if is_dnf {
                ClassificationStatus::Dnf
            } else {
                ClassificationStatus::Finished
            },
            notable_incident: None,
            dnf_catalog_id: None,
            damage_origin_segment: None,
        });

        qualifying_results.push(QualifyingResult {
            pilot_id: id.driver_id,
            pilot_name: id.driver_name,
            team_id: id.team_id,
            team_name: id.team_name,
            position: grid_position.max(finish_position),
            quali_score: 0.0,
            best_lap_time_ms: quali_best.get(&row.cust_id).copied().unwrap_or(0.0),
            gap_to_pole_ms: 0.0,
            is_pole: grid_position == 1,
            is_jogador: is_player,
        });
    }

    // ── Inferência de incidentes a partir das VOLTAS-ATRÁS ──────────────────
    // O JSON zera os incidentes (o iRacing AI-completa a corrida), então a única
    // pegada do que aconteceu é quantas voltas cada um perdeu. Quem abandonou e
    // parou no MESMO ponto (±1 volta) provavelmente se BATEU JUNTO; quem ficou
    // sozinho lá atrás teve um problema solo. Vira `IncidentResult` para o filtro
    // de narrativa (e gera rivalidade de colisão no pipeline). É inferência — não
    // certeza —, mas é o melhor sinal que o resultado oficial deixa.
    use crate::simulation::incidents::{make_incident, IncidentSeverity, IncidentType};

    let attach = |results: &mut [RaceDriverResult], i: usize, inc: crate::simulation::incidents::IncidentResult| {
        results[i].notable_incident = Some(inc.description.clone());
        results[i].incidents_count = results[i].incidents_count.max(1);
        results[i].incidents.push(inc);
    };

    // ── Caso especial: "QUEM bateu no JOGADOR" ──────────────────────────────
    // O monitor ao vivo identifica o carro que colidiu com o jogador (mesmo que
    // ele estivesse parado e fosse atropelado depois — a inferência por voltas
    // não pega isso). Cria a colisão mútua jogador ↔ culpado.
    if let (Some(pi), Some(cid)) = (
        race_results.iter().position(|r| r.is_jogador && r.is_dnf),
        player_collided_with_id,
    ) {
        if let Some(ci) = race_results.iter().position(|r| r.pilot_id == cid) {
            let pname = race_results[pi].pilot_name.clone();
            let cname = race_results[ci].pilot_name.clone();
            let pid = race_results[pi].pilot_id.clone();
            let player_inc = make_incident(
                pid.clone(),
                IncidentType::Collision,
                IncidentSeverity::Critical,
                "corrida",
                0,
                true,
                format!("Batida com {cname}"),
                Some(cid.to_string()),
                true,
                None,
                None,
            );
            attach(&mut race_results, pi, player_inc);
            let culprit_dnf = race_results[ci].is_dnf;
            let culprit_inc = make_incident(
                cid.to_string(),
                IncidentType::Collision,
                if culprit_dnf {
                    IncidentSeverity::Critical
                } else {
                    IncidentSeverity::Major
                },
                "corrida",
                0,
                culprit_dnf,
                format!("Batida com {pname}"),
                Some(pid),
                true,
                None,
                None,
            );
            attach(&mut race_results, ci, culprit_inc);
        }
    }

    // ── Inferência por voltas-atrás para QUEM AINDA NÃO TEM incidente ───────
    let mut dnf_idx: Vec<usize> = (0..race_results.len())
        .filter(|&i| race_results[i].is_dnf && race_results[i].incidents.is_empty())
        .collect();
    dnf_idx.sort_by_key(|&i| race_results[i].laps_completed);
    // Agrupa por proximidade de voltas completas (±1 do âncora do grupo).
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for &i in &dnf_idx {
        match groups.last_mut() {
            Some(g)
                if (race_results[i].laps_completed - race_results[g[0]].laps_completed).abs()
                    <= 1 =>
            {
                g.push(i)
            }
            _ => groups.push(vec![i]),
        }
    }
    for group in &groups {
        if group.len() >= 2 {
            // Colisão: linka cada um ao próximo do grupo (rivalidade + narrativa).
            for (k, &i) in group.iter().enumerate() {
                let other = group[(k + 1) % group.len()];
                let other_name = race_results[other].pilot_name.clone();
                let other_id = race_results[other].pilot_id.clone();
                let inc = make_incident(
                    race_results[i].pilot_id.clone(),
                    IncidentType::Collision,
                    IncidentSeverity::Critical,
                    "corrida",
                    0,
                    true,
                    format!("Colisão com {other_name}"),
                    Some(other_id),
                    true,
                    None,
                    None,
                );
                attach(&mut race_results, i, inc);
            }
        } else {
            let i = group[0];
            let inc = make_incident(
                race_results[i].pilot_id.clone(),
                IncidentType::DriverError,
                IncidentSeverity::Major,
                "corrida",
                0,
                true,
                "Abandonou após incidente".to_string(),
                None,
                false,
                None,
                None,
            );
            attach(&mut race_results, i, inc);
        }
    }

    let winner_id = race_results
        .iter()
        .filter(|r| !r.is_dnf)
        .min_by_key(|r| r.finish_position)
        .map(|r| r.pilot_id.clone())
        .unwrap_or_default();
    let pole_sitter_id = race_results
        .iter()
        .filter(|r| r.grid_position >= 1)
        .min_by_key(|r| r.grid_position)
        .map(|r| r.pilot_id.clone())
        .unwrap_or_default();

    let total_dnfs = race_results.iter().filter(|r| r.is_dnf).count() as i32;
    let total_incidents = race_results.iter().map(|r| r.incidents_count).sum();
    let main_incident_count = race_results
        .iter()
        .flat_map(|r| &r.incidents)
        .filter(|i| i.narrative_importance_hint >= 1)
        .count() as i32;
    let notable_incident_pilot_ids: Vec<String> = race_results
        .iter()
        .filter(|r| r.notable_incident.is_some())
        .map(|r| r.pilot_id.clone())
        .collect();
    let most_positions_gained_id = race_results
        .iter()
        .filter(|r| !r.is_dnf)
        .max_by_key(|r| r.positions_gained)
        .filter(|r| r.positions_gained > 0)
        .map(|r| r.pilot_id.clone());
    let total_laps = event.rows.iter().map(|r| r.laps_complete).max().unwrap_or(0);

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
        main_incident_count,
        notable_incident_pilot_ids,
        most_positions_gained_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iracing_sdk::race_monitor::{CarGapPoint, CarLap, LapSnapshot};

    fn meta(idx: i32, car_number: i32, class_position: i32, class_id: i64) -> CarMeta {
        CarMeta {
            idx,
            is_ai: idx != 0,
            is_pace: false,
            class_id,
            class_position,
            car_number,
            grid_class_position: 0,
        }
    }

    fn history() -> RaceHistory {
        let mut h = RaceHistory {
            laps: vec![LapSnapshot {
                lap: 10,
                cars: vec![
                    CarGapPoint { idx: 0, position: 2, gap: 1.5 },
                    CarGapPoint { idx: 1, position: 1, gap: 0.0 },
                    CarGapPoint { idx: 2, position: 3, gap: 4.0 },
                ],
            }],
            player_laps: vec![],
            player_track: vec![],
            yellow_laps: vec![],
            player_car_idx: 0,
            attempt_number: 1,
            finished: true,
            outcome: "Finalizada".into(),
            car_laps: vec![],
            cars_meta: vec![
                meta(0, 10, 2, 100), // jogador, P2
                meta(1, 21, 1, 100), // IA vencedora
                meta(2, 33, 3, 100), // IA P3
            ],
            track_id: 1,
            qualy_laps: vec![],
            pit_stops: vec![],
            weather: Default::default(),
        };
        // Tempos de volta: idx1 mais rápido (90s), jogador 91s, idx2 92s.
        for (idx, t) in [(0, 91.0), (1, 90.0), (2, 92.0)] {
            for lap in 1..=10 {
                h.car_laps.push(CarLap { car_idx: idx, lap, time: t });
            }
        }
        // Quali: jogador foi pole (89s), idx1 90s, idx2 91s.
        for (idx, t) in [(0, 89.0), (1, 90.0), (2, 91.0)] {
            h.qualy_laps.push(CarLap { car_idx: idx, lap: 1, time: t });
        }
        h
    }

    fn empty_status() -> RaceStatus {
        RaceStatus {
            connected: true,
            attempt_number: 1,
            event: None,
            session_state_label: String::new(),
            track_surface_label: String::new(),
            lap_completed: 10,
            incident_count: 0,
            crash_score: 0.0,
            crash_severity_now: String::new(),
            g_force: 0.0,
            speed_kmh: 0.0,
            tow_time: 0.0,
            cars_count: 3,
            crash_in_progress: false,
            crash_progress_score: 0.0,
            crash_progress_severity: String::new(),
            is_green: true,
            cars_debug: vec![],
            attempts: vec![],
            events: vec![],
        }
    }

    #[test]
    fn reconstructs_positions_grid_and_fastest() {
        let h = history();
        let s = empty_status();
        let by_number: HashMap<i64, String> =
            [(21, "ai-winner".to_string()), (33, "ai-third".to_string())]
                .into_iter()
                .collect();
        let conn = Connection::open_in_memory().unwrap();

        let r = build_race_result_from_session(&h, &s, &conn, &by_number, None, "Seco", "Test");

        // 3 carros classificados.
        assert_eq!(r.race_results.len(), 3);
        // Volta rápida é do idx1 (90s) → "ai-winner".
        assert_eq!(r.fastest_lap_id, "ai-winner");
        assert_eq!(r.winner_id, "ai-winner");
        // Grid pole foi do jogador (quali 89s), mas sem player_driver o id é placeholder.
        let player = r.race_results.iter().find(|x| x.is_jogador).unwrap();
        assert_eq!(player.grid_position, 1);
        assert_eq!(player.finish_position, 2);
        assert_eq!(player.positions_gained, -1);
        assert_eq!(r.total_laps, 10);
    }

    #[test]
    fn ai_dnf_marks_driver_out() {
        let h = history();
        let mut s = empty_status();
        s.events.push(RaceEvent {
            session_time: 100.0,
            lap: 5,
            kind: "dnf_confirmed".into(),
            car_idx: Some(2),
            detail: "Acidente na curva 3".into(),
            severity: Some("grave".into()),
        });
        let by_number: HashMap<i64, String> =
            [(21, "ai-winner".to_string()), (33, "ai-third".to_string())]
                .into_iter()
                .collect();
        let conn = Connection::open_in_memory().unwrap();

        let r = build_race_result_from_session(&h, &s, &conn, &by_number, None, "Seco", "Test");
        let third = r.race_results.iter().find(|x| x.pilot_id == "ai-third").unwrap();
        assert!(third.is_dnf);
        assert_eq!(third.dnf_reason.as_deref(), Some("Acidente na curva 3"));
        assert_eq!(third.notable_incident.as_deref(), Some("Acidente na curva 3"));
        assert_eq!(r.total_dnfs, 1);
    }

    #[test]
    fn aiseason_marks_laps_down_and_gap_and_pole() {
        use crate::iracing_sdk::aiseason_results::{AiEventResult, AiResultRow};
        let row = |laps: i32, num: i32, interval: f64| AiResultRow {
            laps_complete: laps,
            car_number: num,
            reason_out: "Running".into(),
            best_lap_time_ms: 80000.0,
            interval_ms: interval,
            grid_position: num, // só pra ter pole no #1
            ..Default::default()
        };
        let event = AiEventResult {
            track_id: 1,
            laps_complete: 11,
            // P1 líder; DOIS carros parados juntos lá atrás (3/11 — colisão); um lapeado.
            rows: vec![
                row(11, 1, 0.0),
                row(3, 6, 0.0),
                row(3, 7, 0.0),
                row(10, 2, 25000.0),
            ],
            qualify: vec![AiResultRow {
                car_number: 1,
                cust_id: 0,
                best_lap_time_ms: 84000.0,
                grid_position: 1,
                ..Default::default()
            }],
        };
        let conn = Connection::open_in_memory().unwrap();
        let empty = std::collections::HashSet::new();
        let r = build_race_result_from_aiseason(
            &event,
            &conn,
            &HashMap::new(),
            0,
            None,
            false,
            None,
            &empty,
            "Seco",
            "T",
        );
        let leader = r.race_results.iter().find(|x| x.laps_completed == 11).unwrap();
        let lapped = r.race_results.iter().find(|x| x.laps_completed == 10).unwrap();
        let back: Vec<_> = r.race_results.iter().filter(|x| x.laps_completed == 3).collect();
        assert!(!leader.is_dnf, "líder não é DNF");
        assert!(!lapped.is_dnf, "lapeado (10/11) terminou");
        assert_eq!(back.len(), 2);
        assert!(back.iter().all(|x| x.is_dnf), "3/11 voltas = DNF");
        // Gap do carro lapeado vem do interval.
        assert!((lapped.gap_to_winner_ms - 25000.0).abs() < 1.0);
        // Tempo da pole (#1) veio da quali.
        let pole = r.qualifying_results.iter().find(|q| q.is_pole).unwrap();
        assert!((pole.best_lap_time_ms - 84000.0).abs() < 1.0);
        // Os dois parados juntos lá atrás → inferido como COLISÃO entre si.
        for car in &back {
            let inc = car.incidents.first().expect("incidente inferido");
            assert!(inc.is_two_car_incident, "colisão entre os dois");
            assert!(inc.linked_pilot_id.is_some());
        }
    }

    #[test]
    fn aiseason_links_player_to_who_hit_them() {
        use crate::iracing_sdk::aiseason_results::{AiEventResult, AiResultRow};
        let row = |laps: i32, num: i32, cust: i64| AiResultRow {
            laps_complete: laps,
            car_number: num,
            cust_id: cust,
            reason_out: "Running".into(),
            best_lap_time_ms: 80000.0,
            ..Default::default()
        };
        let event = AiEventResult {
            track_id: 1,
            laps_complete: 11,
            rows: vec![
                row(11, 1, 100),  // líder
                row(0, 64, 99),   // jogador parado (0 voltas) — DNF, cust 99
                row(11, 7, 700),  // culpado, que TERMINOU (11 voltas)
            ],
            qualify: vec![],
        };
        let conn = Connection::open_in_memory().unwrap();
        let by_number: HashMap<i64, String> =
            [(7i64, "ai-7".to_string())].into_iter().collect();
        let empty = std::collections::HashSet::new();
        let r = build_race_result_from_aiseason(
            &event,
            &conn,
            &by_number,
            99,
            None,
            true,
            Some("ai-7"),
            &empty,
            "Seco",
            "T",
        );
        let player = r.race_results.iter().find(|x| x.is_jogador).unwrap();
        assert!(player.is_dnf);
        let pi = player.incidents.first().expect("incidente do jogador");
        assert!(pi.is_two_car_incident);
        assert_eq!(pi.linked_pilot_id.as_deref(), Some("ai-7"));
        // O culpado ganha o incidente recíproco, MESMO tendo terminado a corrida.
        let culprit = r.race_results.iter().find(|x| x.pilot_id == "ai-7").unwrap();
        assert_eq!(
            culprit.incidents.first().and_then(|i| i.linked_pilot_id.clone()),
            Some(player.pilot_id.clone())
        );
    }
}
