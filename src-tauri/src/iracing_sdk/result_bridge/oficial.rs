//! Reconstrução do [`RaceResult`] a partir do RESULTADO OFICIAL do iRacing
//! (JSON do aiseason) — a fonte autoritativa que é persistida na carreira.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::models::driver::Driver;
use crate::simulation::qualifying::QualifyingResult;
use crate::simulation::race::{ClassificationStatus, RaceDriverResult, RaceResult};

use super::identidade::resolve_identity;

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
    // Marcadores de incidente do jogador vindos do monitor ao vivo (pontos do
    // PRÓPRIO iRacing: 1 = saída, 2 = rodada, 4 = contato). É o único sinal de
    // batida de quem TERMINOU a corrida — o JSON oficial zera os incidentes.
    player_incident_marks: &[crate::iracing_sdk::race_monitor::PlayerIncidentMark],
    // Pior severidade de batida do jogador (`player_worst_severity`): "nenhum" |
    // "leve" | "moderado" | "grave" | "destruído" | "catastrófico". É o MESMO sinal
    // que calcula o conserto do carro — usá-lo aqui mantém revista e debrief de acordo.
    player_crash_severity: &str,
    // Direção do impacto no pico: "front" | "rear" | "side" | "vertical" (dos Gs).
    // Diz ONDE o carro foi atingido — pancada na traseira conta uma história bem
    // diferente de bater de frente. Vazio quando não houve impacto medido.
    player_impact_dir: &str,
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

    // Sufixo ", com impacto na traseira" etc. Vazio quando o monitor não mediu
    // direção — é enfeite factual, nunca deve inventar um lado que não aconteceu.
    let impact_suffix = match player_impact_dir {
        "front" => rust_i18n::t!("narrative.beat.incident_dir_front").to_string(),
        "rear" => rust_i18n::t!("narrative.beat.incident_dir_rear").to_string(),
        "side" => rust_i18n::t!("narrative.beat.incident_dir_side").to_string(),
        "vertical" => rust_i18n::t!("narrative.beat.incident_dir_vertical").to_string(),
        _ => String::new(),
    };

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
                format!(
                    "{}{impact_suffix}",
                    rust_i18n::t!("narrative.beat.incident_crash_with", other = cname.as_str())
                ),
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
                // Sem sufixo de direção: o impacto medido é do carro do JOGADOR.
                rust_i18n::t!("narrative.beat.incident_crash_with", other = pname.as_str())
                    .to_string(),
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

    // ── Batida do JOGADOR que NÃO abandonou ─────────────────────────────────
    // Tudo acima só cria incidente para quem tem `is_dnf`. Quem bateu e TERMINOU
    // ficava invisível — e é justamente o caso do "toque leve" que a matéria deve
    // citar como leve. Aqui usamos os pontos de incidente do PRÓPRIO iRacing como
    // escala de impacto (é dado real, não inferência), pegando o PIOR evento.
    if let Some(pi) = race_results.iter().position(|r| r.is_jogador && !r.is_dnf) {
        if race_results[pi].incidents.is_empty() {
            let worst = player_incident_marks.iter().max_by_key(|m| m.points);

            // MAGNITUDE do impacto: vem do mesmo sinal que calcula o conserto do carro
            // (`peak_crash_score` → `severity_label`). É bem mais fino que os pontos do
            // iRacing, onde "contato" é 4 pts tanto para o encostão quanto para a
            // pancada que destrói o carro. Usar a MESMA escala do conserto impede a
            // revista de chamar de toque leve uma batida que o debrief cobrou como grave.
            let by_impact = match player_crash_severity {
                "catastrófico" | "destruído" | "grave" => Some(IncidentSeverity::Critical),
                "moderado" => Some(IncidentSeverity::Major),
                "leve" => Some(IncidentSeverity::Minor),
                _ => None,
            };
            // Sem impacto medido, os pontos do iRacing ainda pegam a rodada/contato
            // (o detector de batida pode não ter fechado, mas o sim contou o incidente).
            let by_points = worst.and_then(|m| match m.points {
                p if p >= 4 => Some(IncidentSeverity::Major),
                2 => Some(IncidentSeverity::Minor),
                // <= 1 ponto é só uma saída de pista: ruído, não vira nota.
                _ => None,
            });

            if let Some(severity) = by_impact.or(by_points) {
                // O TIPO vem dos pontos (4 = contato com OUTRO carro); a magnitude vem do
                // impacto. Sem pontos de contato, tratamos como erro de pilotagem.
                let contact = worst.map(|m| m.points >= 4).unwrap_or(false);
                let kind = if contact {
                    IncidentType::Collision
                } else {
                    IncidentType::DriverError
                };
                // A volta só existe se houve marcador; com impacto puro fica sem volta.
                let base = match worst {
                    Some(m) => {
                        let lap = m.lap_f.max(0.0).floor() as i64;
                        let key = if contact {
                            "narrative.beat.incident_sdk_contact"
                        } else {
                            "narrative.beat.incident_sdk_spin"
                        };
                        rust_i18n::t!(key, lap = lap).to_string()
                    }
                    None => rust_i18n::t!("narrative.beat.incident_sdk_impact").to_string(),
                };
                let desc = format!("{base}{impact_suffix}");
                let inc = make_incident(
                    race_results[pi].pilot_id.clone(),
                    kind,
                    severity,
                    "corrida",
                    0,
                    false,
                    desc,
                    None,
                    false,
                    None,
                    None,
                );
                race_results[pi].incidents_count = race_results[pi].incidents_count.max(1);
                race_results[pi].incidents.push(inc);
            }
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
        // Ver acima: a amarela da corrida importada vem do SDK, não daqui.
        caution_segments: Vec::new(),
        // Corrida AO VIVO: a quebra vem do log do disparo (`!black`/`!dq`), não da simulação.
        applied_mechanicals: Vec::new(),
    }
}
