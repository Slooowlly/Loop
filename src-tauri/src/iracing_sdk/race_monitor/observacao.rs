//! Observação por tick: lê a telemetria, detecta os eventos do jogador e da IA
//! e acumula as evidências da tentativa.

use super::*;

/// Passo puro da máquina de apagar o alerta de PENALIDADE: dado o "já entrou no box desde a
/// quebra", o "está no box agora" e o "estava no box no tick anterior", devolve
/// `(novo_entered_since, apagar)`. Apaga quando o carro SAI do box (true→false) já tendo
/// entrado desde a quebra = serviu a penalidade / reparou. (DNF não passa por aqui — é fixo.)
pub(crate) fn pit_clear_step(entered_since: bool, on_pit: bool, prev_on_pit: bool) -> (bool, bool) {
    let entered = entered_since || on_pit;
    let clear = prev_on_pit && !on_pit && entered;
    (entered, clear)
}

impl RaceMonitor {
    /// Carro elegível para as regras de IA: é IA e NÃO é pace car.
    pub(super) fn is_monitorable_ai(&self, idx: i32) -> bool {
        idx >= 0
            && (idx as usize) < 64
            && self.car_is_ai[idx as usize]
            && !self.car_is_pace[idx as usize]
    }

    /// Registra um evento no log (mantém os últimos [`MAX_EVENTS`]).
    pub(super) fn emit(
        &mut self,
        session_time: f64,
        lap: i32,
        kind: &str,
        car_idx: Option<i32>,
        detail: String,
        severity: Option<String>,
    ) {
        self.events.push(RaceEvent {
            session_time,
            lap,
            kind: kind.to_string(),
            car_idx,
            detail,
            severity,
        });
        if self.events.len() > MAX_EVENTS {
            let excess = self.events.len() - MAX_EVENTS;
            self.events.drain(0..excess);
        }
    }

    pub(super) fn observe(&mut self, t: &IracingTelemetry) {
        let now = t.session_time;

        // Salto de tempo (rebobinar/avançar o replay): zera os relógios da IA e
        // o prev do jogador, para não virar falso "parado"/restart.
        let jumped = self.live_session_time != 0.0
            && (now - self.live_session_time).abs() > REPLAY_JUMP_SECS;
        if jumped {
            self.car_monitors = [CarMonitor::DEFAULT; 64];
            self.prev = None;
            self.race_green_time = None; // novo cooldown após o salto
        }

        // Marca o momento do verde (largada) para o cooldown de início. Reseta
        // fora de Racing (pré-largada/pós-corrida).
        if t.session_state == STATE_RACING {
            if self.race_green_time.is_none() {
                self.race_green_time = Some(now);
            }
        } else {
            self.race_green_time = None;
        }

        // Bandeira amarela da sessão (SessionFlags) — sempre, pois vale também
        // assistindo (inclusive para confirmar uma bandeira que enviamos).
        let flags = t.session_flags as u32;
        let caution = flags & (FLAG_CAUTION | FLAG_CAUTION_WAVING) != 0;
        if !self.prev_caution && caution {
            self.emit(
                now,
                t.lap_completed,
                "yellow_triggered",
                None,
                "Bandeira amarela".to_string(),
                None,
            );
            if let Some(rec) = self.pending_yellow_time {
                if now - rec <= YELLOW_CONFIRM_WINDOW_SECS {
                    self.emit(
                        now,
                        t.lap_completed,
                        "yellow_confirmed",
                        None,
                        "Amarela confirmada pelo SessionFlags".to_string(),
                        None,
                    );
                }
                self.pending_yellow_time = None;
            }
        }
        self.prev_caution = caution;

        // Lógica do JOGADOR (tentativa/batida/eventos) só AO VIVO. No replay ele
        // está apenas assistindo.
        if t.is_replay_playing {
            self.live_score = 0.0;
        } else {
            self.process_player(t);
        }

        // Monitoramento das IAs + decisão de bandeira + diagnóstico: SEMPRE (ao
        // vivo e no replay), pois os carros são reais em ambos os casos.
        self.process_ai_cars(t);
        // Disparo de quebra da GRADE TODA (usa a volta de cada carro do `t.cars`).
        self.tick_breakdown_grid(t);
        // Vitrine da 1ª corrida do save: quebra GARANTIDA num backmarker (nunca o jogador).
        self.tick_showcase_breakdown(t);
        self.evaluate_race_control(t);
        self.build_cars_debug(t);
        self.capture_qualy(t);
        self.record_history(t);

        // Snapshot ao vivo (display).
        self.connected = true;
        self.was_connected = true;
        self.live_g = g_force(t);
        self.live_speed_kmh = t.speed_kmh;
        self.live_tow = t.tow_time;
        self.live_state = t.session_state;
        self.live_surface = t.track_surface;
        self.live_lap = t.lap_completed;
        self.live_incident = t.incident_count;
        self.live_session_time = now;
        self.live_cars_count = t.cars.len() as i32;
    }

    /// Lógica do jogador AO VIVO: restart, evidências da tentativa, pontuação de
    /// batida e eventos de sessão/jogador.
    pub(super) fn process_player(&mut self, t: &IracingTelemetry) {
        let now = t.session_time;
        let cur = Snapshot {
            session_time: now,
            lap_completed: t.lap_completed,
        };

        // 1) Restart contra uma tentativa ativa que já largou.
        if let Some(prev) = self.prev {
            let active_raced = self
                .attempts
                .last()
                .map(|a| a.status == "active" && a.evidence.raced)
                .unwrap_or(false);
            if active_raced && Self::restarted(&prev, &cur) {
                self.pending_event = self.finalize_attempt("restart");
            }
        }
        self.ensure_active(now);
        self.prev = Some(cur);

        // 1.5) Estilo de pilotagem: acumula os inputs do jogador SÓ na pista e correndo
        // (pit/garagem/quali não contam). Vira fator de desgaste por peça no import — só o
        // jogador; a IA nunca. Redline desconhecido → o acumulador ignora a rotação.
        if t.track_surface == 3 && t.session_state == 4 {
            let redline = self.car_redline.unwrap_or(0.0);
            if let Some(attempt) = self.attempts.last_mut() {
                attempt.style.ingest(crate::car::driving_style::StyleSample {
                    throttle: t.throttle,
                    brake: t.brake,
                    rpm: t.rpm,
                    redline,
                    gear: t.gear,
                    steering_rad: t.steering_angle_rad,
                    vert_accel: t.vert_accel,
                });
            }
            // 1.6) Disparo de quebra AO VIVO: avalia o carro do jogador nesta volta e enfileira
            // os comandos (só correndo na pista). O diretor deduplica por volta.
            self.tick_breakdown_player(t);
        }

        // 2) Evidências da tentativa.
        self.accumulate_evidence(t);

        // 3) Scorer de batida.
        let prev_incident = self.prev_incident;
        let (mut components, mut factors) = Self::score_tick(t, prev_incident);
        if self.live_tow <= 0.0 && t.tow_time > 0.0 {
            components.tow = TOW_PTS;
            factors.push("reboque acionado".to_string());
        }
        let tick_score = components.total();

        // 4) Abre/funde/fecha a batida.
        if tick_score >= SEV_MINOR {
            if !self.in_crash {
                self.in_crash = true;
                self.crash_components = Components::ZERO;
                self.crash_factors = Vec::new();
                self.crash_start_time = now;
                self.crash_start_lap = t.lap_completed;
                self.crash_had_impact = false;
                self.crash_entry_speed_ms = self.cruise_speed_ms;
                self.crash_min_speed_ms = t.speed_ms;
            }
            if components.g > 0.0 || components.incident >= INCIDENT_4X {
                self.crash_had_impact = true;
                // CONTATO: o carro no mesmo ponto da pista que o jogador é o
                // provável culpado ("quem bateu em mim"). Último contato vence.
                let culprit = self.nearest_contact_car(t);
                if let Some(num) = culprit {
                    if let Some(a) = self.attempts.last_mut() {
                        a.collided_with_car_number = Some(num);
                    }
                }
            }
            self.crash_components.merge_max(&components);
            self.merge_crash_factors(factors);
            self.crash_last_above = Some(now);
            // PICO ao vivo: registra o maior impacto na tentativa mesmo que a
            // batida nunca "feche" (jogador bate e sai). Base do conserto.
            let peak = self.crash_components.total();
            if let Some(attempt) = self.attempts.last_mut() {
                if peak > attempt.peak_crash_score {
                    attempt.peak_crash_score = peak;
                    // Direção do impacto no instante do maior pico — para o dano por peça.
                    attempt.peak_impact_dir = Some(
                        crate::car::crash::impact_direction(
                            t.lat_accel,
                            t.long_accel,
                            t.vert_accel,
                        )
                        .as_str()
                        .to_string(),
                    );
                }
            }
        } else if self.in_crash {
            if let Some(last) = self.crash_last_above {
                if now - last > MERGE_WINDOW_SECS {
                    self.close_crash();
                }
            }
        } else {
            self.cruise_speed_ms = t.speed_ms;
        }
        if self.in_crash {
            self.crash_min_speed_ms = self.crash_min_speed_ms.min(t.speed_ms);
        }

        // 4.5) Eventos de sessão/jogador.
        let lap = t.lap_completed;
        // O verde SÓ conta na sessão de corrida. Treino livre e classificatória também
        // passam por `SessionState = Racing`, e como `race_started_emitted` só reabre em
        // `start_attempt`, a primeira sessão do fim de semana consumia o gate: a corrida
        // largava sem tirar snapshot de grid e o `delta` da torre ficava ancorado na
        // ordem do treino. De quebra, o evento de produto `race_start` também disparava
        // em treino.
        if self.in_race_session(t)
            && self.prev_session_state < STATE_RACING
            && t.session_state >= STATE_RACING
            && t.session_state < STATE_CHECKERED
            && !self.race_started_emitted
        {
            self.race_started_emitted = true;
            // Telemetria de produto: bandeira verde = corrida rolando. UPSERT por
            // subsession no servidor, então restart não vira duas corridas.
            crate::telemetry::race_start(self.session_subsession_id, self.session_track_id);
            // Snapshot do GRID: a posição na classe no instante da largada (ainda
            // não houve ultrapassagem) = a ordem de largada. Fonte do grid quando
            // não há quali voadora (AI season larga de grade fixa).
            for car in &t.cars {
                let i = car.idx;
                if (0..64).contains(&i) && car.class_position >= 1 {
                    self.grid_class_pos[i as usize] = car.class_position;
                }
            }
            self.emit(
                now,
                lap,
                "race_started",
                None,
                "Largada (verde)".to_string(),
                None,
            );
        }

        let finished = self
            .attempts
            .last()
            .map(|a| a.evidence.reached_checkered)
            .unwrap_or(false);
        if finished && !self.race_finished_emitted {
            self.race_finished_emitted = true;
            self.emit(
                now,
                lap,
                "race_finished",
                None,
                "Cruzou a bandeirada".to_string(),
                None,
            );
        }

        if !self.prev_on_pit_road && t.player_on_pit_road {
            self.emit(
                now,
                lap,
                "pit_entry",
                None,
                "Entrou no pit".to_string(),
                None,
            );
        }
        if self.live_tow <= 0.0 && t.tow_time > 0.0 {
            self.emit(
                now,
                lap,
                "tow_detected",
                None,
                "Reboque acionado".to_string(),
                None,
            );
        }

        // Pins do jogador no race trace: incidentes (com pontos) e saídas de
        // pista, posicionados pela fração da volta. Só durante a corrida.
        if t.session_state >= STATE_RACING {
            let lap_f = t.lap_completed as f64 + t.lap_dist_pct.clamp(0.0, 1.0);
            let delta = self
                .prev_incident
                .map(|p| t.incident_count - p)
                .unwrap_or(0);
            if delta > 0 {
                self.player_incidents.push(PlayerIncidentMark {
                    lap_f,
                    points: delta,
                    off_track: t.track_surface == SURFACE_OFF_TRACK,
                });
            } else if t.track_surface == SURFACE_OFF_TRACK
                && self.prev_surface != SURFACE_OFF_TRACK
            {
                // Excursão de pista sem ponto de incidente (0x).
                self.player_incidents.push(PlayerIncidentMark {
                    lap_f,
                    points: 0,
                    off_track: true,
                });
            }
            if self.player_incidents.len() > MAX_HISTORY_LAPS {
                self.player_incidents.remove(0);
            }
        }

        // Atualiza prev_* (transições do jogador) + score ao vivo.
        self.prev_surface = t.track_surface;
        self.prev_incident = Some(t.incident_count);
        self.prev_session_state = t.session_state;
        self.prev_on_pit_road = t.player_on_pit_road;
        self.live_score = tick_score;
    }

    /// Eventos de IA: saída de pista, carro parado e provável DNF, a partir do
    /// progresso (`lap_dist_pct`) de cada carro entre ticks.
    pub(super) fn process_ai_cars(&mut self, t: &IracingTelemetry) {
        let now = t.session_time;
        let flags = t.session_flags as u32;
        let is_green =
            t.session_state == STATE_RACING && flags & (FLAG_CAUTION | FLAG_CAUTION_WAVING) == 0;
        // Ritmo de referência = o carro mais rápido (líder) no tick anterior.
        let ref_pace = self
            .car_monitors
            .iter()
            .map(|cm| cm.pace)
            .fold(0.0_f64, f64::max);

        for car in &t.cars {
            // Só carros de IA (não o jogador, não o pace car, não outros humanos).
            if car.is_player || !self.is_monitorable_ai(car.idx) {
                continue;
            }
            let i = car.idx as usize;
            let mut cm = self.car_monitors[i];

            if cm.last_dist_pct < 0.0 {
                // Primeira leitura: estabelece baseline, SEM marcar movimento.
                cm.last_dist_pct = car.lap_dist_pct;
                cm.last_move_time = now;
            } else if (car.lap_dist_pct - cm.last_dist_pct).abs() > AI_PROGRESS_EPS {
                // Andou de verdade → largou; reseta o relógio de parado.
                cm.last_move_time = now;
                cm.last_dist_pct = car.lap_dist_pct;
                cm.has_moved = true;
                cm.stopped_emitted = false;
                cm.dnf_emitted = false;
                cm.yellow_rec_emitted = false;
            }
            let stalled = now - cm.last_move_time;

            let (mut ev_offtrack, mut ev_stopped, mut ev_dnf) = (false, false, false);
            // "Parado" só vale se o carro JÁ largou (has_moved) — senão é grid.
            if is_green && !car.on_pit_road {
                if car.track_surface == SURFACE_OFF_TRACK && !cm.offtrack_emitted {
                    cm.offtrack_emitted = true;
                    ev_offtrack = true;
                }
                if car.track_surface == SURFACE_ON_TRACK {
                    cm.offtrack_emitted = false;
                }
                if cm.has_moved && stalled > AI_STOPPED_SECS && !cm.stopped_emitted {
                    cm.stopped_emitted = true;
                    ev_stopped = true;
                }
                if cm.has_moved && stalled > AI_DNF_SECS && !cm.dnf_emitted {
                    cm.dnf_emitted = true;
                    ev_dnf = true;
                }
            }

            // Ritmo (pace) — atualizado a cada PACE_WINDOW_SECS.
            if cm.pace_anchor_pct < 0.0 {
                cm.pace_anchor_pct = car.lap_dist_pct;
                cm.pace_anchor_time = now;
            } else if now - cm.pace_anchor_time >= PACE_WINDOW_SECS {
                let mut d = car.lap_dist_pct - cm.pace_anchor_pct;
                if d < 0.0 {
                    d += 1.0; // volta circular
                }
                cm.pace = d / (now - cm.pace_anchor_time);
                cm.pace_anchor_pct = car.lap_dist_pct;
                cm.pace_anchor_time = now;
                // Atingiu ritmo de corrida? Marca que já "correu" de verdade.
                if ref_pace > 0.0 && cm.pace >= RACING_PACE_FRACTION * ref_pace {
                    cm.has_raced = true;
                }
            }
            // "Lento por incidente" (alimenta o cluster de pits = acidente coletivo):
            // o ritmo despencou E o carro SAIU DA PISTA (rodada/excursão).
            // Exigir o off-track é o que separa um ACIDENTE de um carro só
            // DESACELERANDO PARA ABASTECER: a fila/rastejo na entrada do box (ON_TRACK,
            // porém quase parado por 2s+) era contada como "quase parado" e um grupo de
            // pits normais virava falso acidente coletivo. Carros que PARAM na pista por
            // batida (sem sair dela) seguem cobertos pela detecção por setor (caminho 4).
            let went_off = car.track_surface == SURFACE_OFF_TRACK;
            if cm.has_raced && ref_pace > 0.0 && cm.pace < SLOW_PACE_FRACTION * ref_pace && went_off {
                cm.last_slow_time = now;
            }
            // Pit de incidente: entrou no pit logo após ter ficado lento na pista.
            if car.on_pit_road && !cm.was_on_pit && now - cm.last_slow_time <= SLOW_PIT_WINDOW_SECS
            {
                cm.incident_pit_time = Some(now);
            }
            cm.was_on_pit = car.on_pit_road;

            self.car_monitors[i] = cm;

            let idx = car.idx;
            // Volta ATUAL do carro (CarIdxLap); fallback para completadas + 1.
            let car_lap = if car.lap > 0 {
                car.lap
            } else {
                car.lap_completed + 1
            };
            if ev_offtrack {
                self.emit(
                    now,
                    car_lap,
                    "ai_offtrack",
                    Some(idx),
                    format!("Carro {idx} saiu da pista"),
                    None,
                );
            }
            if ev_stopped {
                self.emit(
                    now,
                    car_lap,
                    "ai_stopped",
                    Some(idx),
                    format!("Carro {idx} parado (~{stalled:.0}s)"),
                    None,
                );
            }
            if ev_dnf {
                self.emit(
                    now,
                    car_lap,
                    "ai_possible_dnf",
                    Some(idx),
                    format!("Carro {idx} provável DNF (parado {stalled:.0}s)"),
                    None,
                );
            }
        }
    }

    pub(super) fn accumulate_evidence(&mut self, t: &IracingTelemetry) {
        let surface = t.track_surface;
        let prev_surface = self.prev_surface;
        let prev_incident = self.prev_incident;
        let flags = t.session_flags as u32;
        let prev_laps = self.attempts.last().map(|a| a.laps_completed).unwrap_or(0);
        let attempt = match self.attempts.last_mut() {
            Some(a) if a.status == "active" => a,
            _ => return,
        };
        let ev = &mut attempt.evidence;

        if t.session_state >= STATE_RACING && surface == SURFACE_ON_TRACK {
            ev.raced = true;
        }
        if ev.raced {
            if surface == SURFACE_OFF_TRACK {
                ev.off_track = true;
            }
            if surface == SURFACE_NOT_IN_WORLD {
                ev.not_in_world = true;
            }
            if surface == SURFACE_IN_PIT_STALL
                && (prev_surface == SURFACE_OFF_TRACK || prev_surface == SURFACE_NOT_IN_WORLD)
            {
                ev.towed_to_pit = true;
            }
            if flags & FLAG_BLACK != 0 {
                ev.black_flag = true;
            }
            if flags & FLAG_DISQUALIFY != 0 {
                ev.disqualified = true;
            }
            if t.is_in_garage {
                ev.garage = true;
            }
            let checkered_shown = flags & FLAG_CHECKERED != 0 || t.session_state >= STATE_CHECKERED;
            if checkered_shown && t.lap_completed > prev_laps {
                ev.reached_checkered = true;
            }
        }
        if let Some(prev) = prev_incident {
            let delta = t.incident_count - prev;
            if delta > 0 {
                ev.incident_points += delta;
            }
        }
        attempt.laps_completed = attempt.laps_completed.max(t.lap_completed);
    }
}
