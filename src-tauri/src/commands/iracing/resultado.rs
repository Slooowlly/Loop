//! Ponte resultado do iRacing → carreira: mapeamento, preview e percepção de rivalidades na pista.

use super::*;

/// Uma entrada do resultado de corrida já mapeado para a carreira (Fase 3).
#[derive(serde::Serialize)]
pub struct CareerRaceEntry {
    /// Nosso `driver_id` (None se o número não casou — ex.: carro do jogador).
    pub driver_id: Option<String>,
    /// Nome do nosso piloto (do banco).
    pub name: Option<String>,
    pub car_number: i32,
    pub class_position: i32,
    pub is_player: bool,
}

/// Resultado da última corrida, mapeado de volta para os nossos pilotos.
#[derive(serde::Serialize)]
pub struct CareerRaceResult {
    pub track_id: i64,
    pub track_name: Option<String>,
    pub finished: bool,
    pub entries: Vec<CareerRaceEntry>,
}

/// Casa o resultado da última corrida (números de carro da IA) de volta com os
/// nossos pilotos (Fase 3). Como NÓS geramos o roster, o número do carro → nosso
/// `driver_id` via o mapa de números fixos salvo na geração do roster.
#[tauri::command]
pub fn iracing_career_race_result(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<CareerRaceResult, String> {
    use crate::config::app_config::AppConfig;
    use crate::constants::tracks::get_track;
    use crate::db::connection::Database;
    use crate::db::queries::drivers as dq;
    use crate::iracing_sdk::race_monitor;
    use tauri::Manager;

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    // Mapa de números: driver_id → número (salvo na geração do roster) → reverte.
    let numbers: std::collections::HashMap<String, i64> =
        std::fs::read_to_string(numbers_path(&base_dir, &career_id))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
    let by_number: std::collections::HashMap<i64, String> =
        numbers.into_iter().map(|(id, n)| (n, id)).collect();

    // Banco da carreira (para os nomes).
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let history = race_monitor::get_history();
    let player_idx = history.player_car_idx;

    // O jogador é excluído do roster da IA, então o número dele não está no mapa —
    // resolvemos pelo piloto-jogador da carreira (is_jogador).
    let player_driver = dq::get_player_driver(&db.conn).ok();

    let mut entries: Vec<CareerRaceEntry> = history
        .cars_meta
        .iter()
        .filter(|m| !m.is_pace)
        .map(|m| {
            let is_player = m.idx == player_idx;
            let (driver_id, name) = if is_player {
                match &player_driver {
                    Some(d) => (Some(d.id.clone()), Some(d.nome.clone())),
                    None => (None, None),
                }
            } else {
                let did = by_number.get(&(m.car_number as i64)).cloned();
                let nm = did
                    .as_deref()
                    .and_then(|id| dq::get_driver(&db.conn, id).ok())
                    .map(|d| d.nome);
                (did, nm)
            };
            CareerRaceEntry {
                driver_id,
                name,
                car_number: m.car_number,
                class_position: m.class_position,
                is_player,
            }
        })
        .collect();
    // Ordena pela posição na classe (não classificados ao fim).
    entries.sort_by_key(|e| {
        if e.class_position >= 1 {
            e.class_position
        } else {
            i32::MAX
        }
    });

    Ok(CareerRaceResult {
        track_id: history.track_id,
        track_name: get_track(history.track_id as u32).map(|t| t.nome.to_string()),
        finished: history.finished,
        entries,
    })
}

/// Setup compartilhado pelo preview e pelo import. Lê o RESULTADO OFICIAL do
/// iRacing (JSON do aiseason, persistido) — não reconstrói ao vivo. Acha o arquivo
/// e o evento pelo "post-it" gravado no export + a próxima corrida pendente da
/// carreira. A batida do jogador (custo de conserto) ainda vem do monitor ao vivo.
/// Devolve `(banco, career_dir, track_id, pior severidade do jogador, resultado)`.
pub(crate) fn build_session_race_result(
    app: &tauri::AppHandle,
    career_id: &str,
) -> Result<
    (
        crate::db::connection::Database,
        std::path::PathBuf,
        i64,
        String, // pior severidade de batida do jogador (base do conserto)
        crate::simulation::race::RaceResult,
        crate::iracing_sdk::telemetry_analysis::TelemetryAnalysis,
        crate::iracing_sdk::race_monitor::RaceHistory,
        // Mapa número do carro → driver_id (para resolver a identidade dos rivais
        // percebidos do SDK na ponte de rivalidade).
        std::collections::HashMap<i64, String>,
        // Direção do impacto no pico do jogador (front/rear/side/vertical; vazia se sem batida).
        String,
        // Estilo de pilotagem do jogador (fatores de desgaste por peça; neutro se sem estilo).
        crate::car::driving_style::StyleFactors,
    ),
    String,
> {
    use crate::config::app_config::AppConfig;
    use crate::constants::tracks::get_track;
    use crate::db::connection::Database;
    use crate::db::queries::drivers as dq;
    use crate::db::queries::seasons as sq;
    use crate::iracing_sdk::{aiseason_results, race_monitor, result_bridge, telemetry_analysis};
    use tauri::Manager;

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    // Mapa de números: driver_id → número → reverte para número → driver_id.
    let numbers: std::collections::HashMap<String, i64> =
        std::fs::read_to_string(numbers_path(&base_dir, career_id))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
    let by_number: std::collections::HashMap<i64, String> =
        numbers.into_iter().map(|(id, n)| (n, id)).collect();

    let config = AppConfig::load_or_default(&base_dir);
    let career_dir = config.saves_dir().join(career_id);
    let db_path = career_dir.join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;
    let player_driver = dq::get_player_driver(&db.conn).ok();

    // ── Post-it: arquivo de aiseason + mapa evento→corrida ──────────────────
    let pointer: serde_json::Value = season_pointer_path(&base_dir, career_id)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .ok_or_else(|| {
            "Não achei o registro do aiseason exportado. Exporte os dados (gerar AI Season) antes de importar.".to_string()
        })?;
    let aiseason_file = pointer
        .get("aiseason_file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Registro do aiseason inválido.".to_string())?;

    // ── Próxima corrida pendente da carreira → índice do evento ─────────────
    let active_season = sq::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao ler temporada: {e}"))?
        .ok_or("Nenhuma temporada ativa.")?;
    let next_race = crate::commands::race::get_next_player_race(&db.conn, &active_season)?
        .ok_or("O jogador não possui corrida pendente.")?;
    let events = pointer.get("events").and_then(|v| v.as_array());
    let event_index = events
        .and_then(|evs| {
            evs.iter().position(|e| {
                e.get("race_id").and_then(|v| v.as_str()) == Some(next_race.id.as_str())
            })
        })
        .ok_or_else(|| {
            format!(
                "A próxima corrida ({}) não está no aiseason exportado. Exporte novamente.",
                next_race.track_name
            )
        })?;

    // ── Lê o resultado oficial do JSON ──────────────────────────────────────
    let season_json: serde_json::Value = std::fs::read_to_string(aiseason_file)
        .map_err(|e| format!("Falha ao ler o aiseason: {e}"))
        .and_then(|s| serde_json::from_str(&s).map_err(|e| format!("aiseason inválido: {e}")))?;
    let event = aiseason_results::parse_event_result(&season_json, event_index)
        .ok_or("Evento sem resultado no aiseason.")?;
    if !event.is_final() {
        return Err(
            "O iRacing ainda não gravou o resultado dessa corrida. Termine/saia da corrida no iRacing (ele simula o resto e salva) e tente de novo."
                .to_string(),
        );
    }
    // Pista de fato EXPORTADA para este evento — pode ser uma FREE substituta quando a
    // pista real do calendário é conteúdo pago (ver `free_or_substitute` no export).
    // Comparamos o resultado do iRacing contra o que foi exportado, não contra a pista
    // original da carreira (senão a substituição de teste tropeçaria aqui).
    let exported_track_id = events
        .and_then(|evs| evs.get(event_index))
        .and_then(|e| e.get("track_id"))
        .and_then(|v| v.as_i64())
        .unwrap_or(next_race.track_id as i64);
    if event.track_id != exported_track_id {
        return Err(format!(
            "A pista do resultado (id {}) não bate com a corrida exportada ({}, id {}).",
            event.track_id, next_race.track_name, exported_track_id
        ));
    }

    let track_name = get_track(next_race.track_id)
        .map(|t| t.nome.to_string())
        .unwrap_or_else(|| next_race.track_name.clone());
    let player_custid = crate::iracing_sdk::cached_custid().unwrap_or(0);

    // ── Sinais do monitor AO VIVO (o JSON do iRacing não tem) ───────────────
    // O iRacing AI-completa a corrida e marca todo mundo "Running"; sobrepomos a
    // batida do jogador (conserto + DNF) e os abandonos que o monitor flagrou.
    let history = race_monitor::get_history();
    let status = race_monitor::poll();
    let player_crash = result_bridge::player_worst_severity(&status, status.attempt_number);
    let player_attempt = status
        .attempts
        .iter()
        .find(|a| a.number == status.attempt_number)
        .or_else(|| status.attempts.last());
    // Jogador correu mas não cruzou a bandeira (saiu/bateu) → DNF. Inclui o caso
    // de ficar parado na pista (raced fica true assim que ele entra na corrida).
    let player_dnf = player_attempt
        .map(|a| a.evidence.raced && !a.evidence.reached_checkered)
        .unwrap_or(false);
    // "Quem bateu em mim": o monitor guardou o carro mais próximo no contato.
    let player_collided_with_id: Option<String> = player_attempt
        .and_then(|a| a.collided_with_car_number)
        .and_then(|num| by_number.get(&(num as i64)).cloned());
    // Direção do impacto no pico (front/rear/side/vertical) — base do dano por peça no import.
    let player_impact_dir: String = player_attempt
        .and_then(|a| a.peak_impact_dir.clone())
        .unwrap_or_default();
    // Estilo de pilotagem do jogador (fatores de desgaste por peça) — do acumulador ao vivo.
    let player_style: crate::car::driving_style::StyleFactors = player_attempt
        .map(|a| a.style.factors())
        .unwrap_or_default();
    // Carros que o monitor confirmou ter abandonado → número do carro.
    let num_by_idx: std::collections::HashMap<i32, i32> = history
        .cars_meta
        .iter()
        .map(|m| (m.idx, m.car_number))
        .collect();
    let extra_dnf_numbers: std::collections::HashSet<i32> = status
        .events
        .iter()
        .filter(|e| e.kind == "dnf_confirmed")
        .filter_map(|e| e.car_idx)
        .filter_map(|idx| num_by_idx.get(&idx).copied())
        .filter(|n| *n > 0)
        .collect();

    let result = result_bridge::build_race_result_from_aiseason(
        &event,
        &db.conn,
        &by_number,
        player_custid,
        player_driver.as_ref(),
        player_dnf,
        player_collided_with_id.as_deref(),
        &extra_dnf_numbers,
        "", // clima resolvido na persistência
        &track_name,
        &race_monitor::get_player_incidents(),
        &player_crash,
        &player_impact_dir,
    );

    // TELEMETRIA (Fase 2): ritmo/consistência/rival do histórico ao vivo. Só vem
    // completa se o jogador correu; resolve car_idx→nome pelo cars_meta + roster.
    let name_by_idx: std::collections::HashMap<i32, String> = history
        .cars_meta
        .iter()
        .filter_map(|m| {
            let is_player = m.idx == history.player_car_idx;
            let name = if is_player {
                player_driver.as_ref().map(|d| d.nome.clone())
            } else {
                by_number
                    .get(&(m.car_number as i64))
                    .and_then(|id| dq::get_driver(&db.conn, id).ok())
                    .map(|d| d.nome)
            };
            name.map(|n| (m.idx, n))
        })
        .collect();
    // Equipe por car_idx (mesma fonte do resultado oficial: contrato regular ativo →
    // equipe). Resolve o driver_id por número e busca o time do contrato vigente.
    let team_by_idx: std::collections::HashMap<i32, String> = history
        .cars_meta
        .iter()
        .filter_map(|m| {
            let driver_id = if m.idx == history.player_car_idx {
                player_driver.as_ref().map(|d| d.id.clone())
            } else {
                by_number.get(&(m.car_number as i64)).cloned()
            }?;
            let team = crate::db::queries::contracts::get_active_regular_contract_for_pilot(
                &db.conn, &driver_id,
            )
            .ok()
            .flatten()
            .map(|c| {
                crate::db::queries::teams::get_team_by_id(&db.conn, &c.equipe_id)
                    .ok()
                    .flatten()
                    .map(|t| t.nome)
                    .unwrap_or(c.equipe_id)
            })?;
            Some((m.idx, team))
        })
        .collect();
    // Sinais de batida/DNF do jogador (não estão no RaceHistory) para o "erro
    // mais caro": voltas com contato + se abandonou + a volta em que parou.
    let crash_laps: Vec<i32> = player_attempt
        .map(|a| a.crashes.iter().map(|c| c.lap).filter(|l| *l > 0).collect())
        .unwrap_or_default();
    let dnf_lap = if player_dnf {
        history
            .player_laps
            .iter()
            .map(|l| l.lap)
            .max()
            .or_else(|| crash_laps.iter().max().copied())
    } else {
        None
    };
    let player_incidents = telemetry_analysis::PlayerIncidents {
        crash_laps,
        is_dnf: player_dnf,
        dnf_lap,
    };
    let telemetry =
        telemetry_analysis::analyze(&history, &name_by_idx, &team_by_idx, &player_incidents);

    Ok((
        db,
        career_dir,
        next_race.track_id as i64,
        player_crash,
        result,
        telemetry,
        history,
        by_number,
        player_impact_dir,
        player_style,
    ))
}

/// Peça 3: converte o log de quebras drenado do monitor em linhas prontas pra persistir,
/// resolvendo car_number → driver_id. O número do JOGADOR não está no `by_number` (só IAs) →
/// resolve pelo idx dele (`num_by_idx` do histórico) + o driver do save. Chamado SÓ no import
/// (one-shot); o `drain` esvazia o log, então nunca no preview (que repete).
pub(crate) fn resolve_breakdown_rows(
    conn: &rusqlite::Connection,
    history: &crate::iracing_sdk::race_monitor::RaceHistory,
    by_number: &std::collections::HashMap<i64, String>,
) -> Vec<crate::db::queries::race_breakdowns::RaceBreakdownRow> {
    use crate::db::queries::drivers as dq;
    let player_id: Option<String> = dq::get_player_driver(conn).ok().map(|p| p.id);
    let num_by_idx: std::collections::HashMap<i32, i32> = history
        .cars_meta
        .iter()
        .map(|m| (m.idx, m.car_number))
        .collect();
    let player_number: Option<i32> = num_by_idx
        .get(&history.player_car_idx)
        .copied()
        .filter(|n| *n > 0);
    crate::iracing_sdk::race_monitor::drain_breakdown_log()
        .into_iter()
        .filter_map(|o| {
            let driver_id = match (player_number, player_id.as_ref()) {
                (Some(pnum), Some(pid)) if o.car_number as i32 == pnum => Some(pid.clone()),
                _ => by_number.get(&(o.car_number as i64)).cloned(),
            }?;
            Some(crate::db::queries::race_breakdowns::RaceBreakdownRow {
                driver_id,
                part: o.part,
                problem: o.problem,
                lap: o.lap,
                severity: o.severity,
                penalty_secs: o.penalty_secs,
                forced: o.forced,
                label: o.label,
            })
        })
        .collect()
}

/// PREVIEW (read-only) da ponte sessão→`RaceResult`: reconstrói o resultado da
/// última corrida disputada no iRacing como o `RaceResult` que a carreira
/// consome — SEM gravar nada no banco. Serve para validar o mapeamento (posições,
/// grid, volta rápida, DNFs) contra a tela do iRacing antes de importar.
#[tauri::command]
pub fn iracing_preview_race_result(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<crate::simulation::race::RaceResult, String> {
    let (_db, _dir, _track_id, _sev, result, _tel, _hist, _by_number, _impact_dir, _style) =
        build_session_race_result(&app, &career_id)?;
    Ok(result)
}

/// Ponte de rivalidade de PISTA: aplica no motor de rivalidade as rivalidades que a
/// percepção do SDK detectou na corrida importada, resolvendo a identidade dos
/// oponentes (car_number → driver_id via `by_number`). Só o jogador é o probe.
///
/// Best-effort: qualquer falha é engolida (o import já foi persistido). Atualiza os
/// eixos (histórico/recente) E grava um CAPÍTULO do arco por rival (a "novela" que a
/// IA recapitula), com a interação REAL da percepção. Idempotente por rodada (o
/// `insert_episode` deduplica com o `record_rivalry_episodes` do boletim).
/// Ver `docs/superpowers/specs/2026-07-18-track-rivalry-perception-design.md` §10.
pub(crate) fn apply_track_rivalries(
    conn: &rusqlite::Connection,
    history: &crate::iracing_sdk::race_monitor::RaceHistory,
    by_number: &std::collections::HashMap<i64, String>,
    race_result: &crate::simulation::race::RaceResult,
    rodada: i32,
    categoria: &str,
) {
    use crate::iracing_sdk::rivalry_perception::{
        perceive_rivalries, ContactSeed, ContactTier, PerceptionParams,
    };
    use crate::models::rivalry::RivalryType;
    use crate::rivalry::{apply_rivalry_event, RivalryEvent};

    // driver_id do jogador (o probe).
    let Some(player_id) = race_result
        .race_results
        .iter()
        .find(|r| r.is_jogador)
        .map(|r| r.pilot_id.clone())
    else {
        return;
    };

    // car_idx → driver_id, via car_number (cars_meta) + by_number. Só carros mapeados.
    let driver_by_idx: std::collections::HashMap<i32, String> = history
        .cars_meta
        .iter()
        .filter_map(|m| {
            by_number
                .get(&(m.car_number as i64))
                .map(|id| (m.idx, id.clone()))
        })
        .collect();

    // Contato atribuído ("quem bateu em mim"), do monitor ao vivo → semente da percepção.
    let contact: Option<ContactSeed> = {
        let status = race_monitor::poll();
        let attempt = status
            .attempts
            .iter()
            .find(|a| a.number == status.attempt_number)
            .or_else(|| status.attempts.last());
        attempt.and_then(|a| {
            a.collided_with_car_number.and_then(|num| {
                history
                    .cars_meta
                    .iter()
                    .find(|m| m.car_number == num)
                    .map(|m| {
                        let tier = if a.evidence.raced && !a.evidence.reached_checkered {
                            ContactTier::Dnf
                        } else {
                            ContactTier::Major
                        };
                        ContactSeed {
                            opponent_car_idx: m.idx,
                            tier,
                        }
                    })
            })
        })
    };

    let season = crate::db::queries::seasons::get_active_season(conn)
        .ok()
        .flatten();
    let temporada = season.as_ref().map(|s| s.numero).unwrap_or(0);
    let ano = season.as_ref().map(|s| s.ano).unwrap_or(0);

    // Posição final do jogador (para decidir quem levou a melhor em cada capítulo).
    let player_res = race_result
        .race_results
        .iter()
        .find(|d| d.pilot_id == player_id);

    let perception = perceive_rivalries(
        history,
        history.player_car_idx,
        contact,
        &PerceptionParams::default(),
    );

    for opp in &perception.opponents {
        let Some(opp_id) = driver_by_idx.get(&opp.car_idx) else {
            continue;
        };
        if *opp_id == player_id {
            continue;
        }
        let is_contact = opp.hits.iter().any(|h| h.kind == "contato");
        let tipo = if is_contact {
            RivalryType::Colisao
        } else {
            RivalryType::Pista
        };
        let applied = match apply_rivalry_event(
            conn,
            &RivalryEvent {
                piloto_a: player_id.clone(),
                piloto_b: opp_id.clone(),
                tipo,
                historical_delta: opp.historical_delta,
                recent_delta: opp.recent_delta,
                temporada,
            },
        ) {
            Ok(a) => a,
            Err(_) => continue,
        };

        // Capítulo do arco: interação real + quem levou a melhor hoje.
        let opp_res = race_result
            .race_results
            .iter()
            .find(|d| &d.pilot_id == opp_id);
        let winner_id = match (player_res, opp_res) {
            (Some(p), Some(o)) => {
                let (p_ok, o_ok) = (!p.is_dnf, !o.is_dnf);
                if p_ok && o_ok {
                    match p.finish_position.cmp(&o.finish_position) {
                        std::cmp::Ordering::Less => Some(player_id.clone()),
                        std::cmp::Ordering::Greater => Some(opp_id.clone()),
                        std::cmp::Ordering::Equal => None,
                    }
                } else if p_ok {
                    Some(player_id.clone())
                } else if o_ok {
                    Some(opp_id.clone())
                } else {
                    None
                }
            }
            _ => None,
        };
        let opp_name = opp_res
            .map(|o| o.pilot_name.clone())
            .unwrap_or_else(|| opp_id.clone());
        let interaction = if is_contact { "colisao" } else { "duelo" };
        let summary = if is_contact {
            format!("contato com {opp_name} em {}", race_result.track_name)
        } else {
            format!(
                "duelo de {} voltas com {opp_name} em {}",
                opp.duel_laps, race_result.track_name
            )
        };
        let _ = crate::db::queries::rivalry_episodes::insert_episode(
            conn,
            &crate::db::queries::rivalry_episodes::RivalryEpisode {
                piloto1_id: player_id.clone(),
                piloto2_id: opp_id.clone(),
                temporada,
                rodada,
                ano,
                categoria: categoria.to_string(),
                track_name: race_result.track_name.clone(),
                interaction: interaction.to_string(),
                winner_id,
                summary,
                perceived: applied.new_perceived,
            },
        );
    }

    // Rivalidade IA-vs-IA: o motor de percepção aceita QUALQUER carro-sonda, não só o jogador.
    // Alimenta o ledger de rivalidade também entre as IAs, pra a "novela" emergir do grid
    // inteiro (piloto larga → vira rival → dá título → ex-time em crise), e não só ao redor do
    // jogador. Sem contato atribuído (só o jogador tem a semente do monitor) e SEM episódio (o
    // arco recapitulado é player-facing). Dedupe por par normalizado — o ledger é simétrico
    // (`normalize_pair`), então cada par de IA é aplicado UMA vez (o outro lado repetiria e
    // dobraria os deltas). Pares que envolvem o jogador já vieram do probe-jogador acima. O
    // custo é O(n²) sobre os snapshots, mas roda uma única vez no import.
    let mut seen_ai_pairs: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    let ai_idxs: Vec<i32> = driver_by_idx
        .keys()
        .copied()
        .filter(|&idx| idx != history.player_car_idx)
        .collect();
    for probe_idx in ai_idxs {
        let Some(probe_id) = driver_by_idx.get(&probe_idx) else {
            continue;
        };
        if *probe_id == player_id {
            continue;
        }
        let probe_perception =
            perceive_rivalries(history, probe_idx, None, &PerceptionParams::default());
        for opp in &probe_perception.opponents {
            let Some(opp_id) = driver_by_idx.get(&opp.car_idx) else {
                continue;
            };
            if opp_id == probe_id || *opp_id == player_id {
                continue; // par com o jogador já tratado pelo probe-jogador
            }
            // Chave canônica (ordem estável) → aplica o par só uma vez.
            let key = if probe_id <= opp_id {
                (probe_id.clone(), opp_id.clone())
            } else {
                (opp_id.clone(), probe_id.clone())
            };
            if !seen_ai_pairs.insert(key) {
                continue;
            }
            let is_contact = opp.hits.iter().any(|h| h.kind == "contato");
            let tipo = if is_contact {
                RivalryType::Colisao
            } else {
                RivalryType::Pista
            };
            let _ = apply_rivalry_event(
                conn,
                &RivalryEvent {
                    piloto_a: probe_id.clone(),
                    piloto_b: opp_id.clone(),
                    tipo,
                    historical_delta: opp.historical_delta,
                    recent_delta: opp.recent_delta,
                    temporada,
                },
            );
        }
    }
}
