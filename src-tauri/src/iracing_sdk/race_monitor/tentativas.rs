//! Ciclo de vida da tentativa: abrir, garantir ativa, detectar restart e
//! fechar com o desfecho (finished/dnf/not_started).

use super::*;

impl RaceMonitor {
    // ── Tentativas ───────────────────────────────────────────────────────────
    pub(super) fn start_attempt(&mut self, session_time: f64) {
        self.current_attempt += 1;
        self.prev_surface = SURFACE_ON_TRACK;
        self.prev_incident = None;
        self.player_incidents.clear();
        self.race_started_emitted = false;
        self.race_finished_emitted = false;
        self.dnf_probable = false;
        self.player_yellow_rec_emitted = false;
        self.last_pit_cluster_alert = None;
        self.last_collective_alert = None;
        self.race_green_time = None;
        // Nova tentativa = grade resetada; zera o rastreamento de movimento da IA.
        self.car_monitors = [CarMonitor::DEFAULT; 64];
        // Grid recapturado na próxima largada (verde).
        self.grid_class_pos = [0; 64];
        // Restart = corrida fresca → descarta o log de quebras da tentativa anterior (Peça 3).
        self.breakdown_log.clear();
        self.player_risk_warned = [false; 11];
        self.player_warning_log.clear();
        self.poupar_avisado = false;
        self.ritmo.reiniciar();
        self.ritmo_log.clear();
        self.ritmo_ultima_volta = -1;
        self.attempts.push(Attempt {
            number: self.current_attempt,
            status: "active".to_string(),
            started_at_session_time: session_time,
            laps_completed: 0,
            ended_by: None,
            reason: None,
            worst_crash: None,
            evidence: AttemptEvidence::default(),
            crashes: Vec::new(),
            peak_crash_score: 0.0,
            collided_with_car_number: None,
            peak_impact_dir: None,
            style: crate::car::driving_style::StyleAccumulator::new(),
        });
    }

    pub(super) fn ensure_active(&mut self, session_time: f64) {
        let need = match self.attempts.last() {
            None => true,
            Some(a) => a.status != "active",
        };
        if need {
            self.start_attempt(session_time);
        }
    }

    pub(super) fn restarted(prev: &Snapshot, cur: &Snapshot) -> bool {
        let time_reset = cur.session_time + SESSION_TIME_DROP_TOLERANCE < prev.session_time
            && cur.session_time < RESTART_RESET_MAX_SECS;
        time_reset || cur.lap_completed < prev.lap_completed
    }

    /// Fecha a tentativa ativa, classificando o desfecho e (se finalizada)
    /// rebaixando a severidade das batidas. Retorna o texto do evento.
    /// Monta o desfecho da corrida para a telemetria de produto. Lê SÓ do que já foi
    /// acumulado para o painel pós-corrida (`self.history`) — nenhuma amostragem nova,
    /// nenhum custo por tick. Roda uma vez, no fim da corrida.
    ///
    /// Tudo é best-effort: campo que não dá para determinar sai zerado e o
    /// `telemetry::race_end` o omite do payload.
    pub(super) fn build_race_outcome(
        &self,
        ev: &AttemptEvidence,
        laps: i32,
        attempt_number: i32,
        worst_crash: Option<String>,
    ) -> crate::telemetry::RaceOutcome {
        let mut out = crate::telemetry::RaceOutcome {
            voltas: laps,
            incidentes: ev.incident_points,
            // Tentativa nº3 = duas largadas refeitas.
            restarts: (attempt_number - 1).max(0),
            off_track: ev.off_track,
            towed: ev.towed_to_pit,
            garage: ev.garage,
            black_flag: ev.black_flag,
            disqualified: ev.disqualified,
            pior_batida: worst_crash,
            carro: self.session_car_name.clone(),
            ..Default::default()
        };

        let idx = self.history.player_car_idx;
        let Some(me) = self.history.cars_meta.iter().find(|c| c.idx == idx) else {
            // Sem meta do jogador não há posição nem classe de referência; o resto do
            // desfecho (voltas, incidentes, carro) continua valendo.
            return out;
        };
        out.posicao_final = me.class_position.max(0);
        out.posicao_grid = me.grid_class_position.max(0);

        // Índice por car_idx, para uma passada só sobre as voltas (que são milhares).
        let mut class_of = [i64::MIN; 64];
        for c in self.history.cars_meta.iter() {
            if c.idx >= 0 && (c.idx as usize) < 64 && !c.is_pace {
                class_of[c.idx as usize] = c.class_id;
            }
        }
        out.carros_na_classe = class_of.iter().filter(|c| **c == me.class_id).count() as i32;

        let (mut best_me, mut best_class) = (f64::INFINITY, f64::INFINITY);
        for l in self.history.car_laps.iter() {
            if l.time <= 0.0 || l.car_idx < 0 || l.car_idx as usize >= 64 {
                continue;
            }
            if l.car_idx == idx {
                best_me = best_me.min(l.time);
            }
            if class_of[l.car_idx as usize] == me.class_id {
                best_class = best_class.min(l.time);
            }
        }
        if best_me.is_finite() {
            out.melhor_volta_s = best_me;
        }
        if best_class.is_finite() {
            out.melhor_volta_classe_s = best_class;
        }
        out
    }

    pub(super) fn finalize_attempt(&mut self, ended_by: &str) -> Option<String> {
        // Uma batida em aberto pertence a esta tentativa: fecha primeiro.
        if self.in_crash {
            self.close_crash();
        }
        let attempt = self.attempts.last_mut()?;
        if attempt.status != "active" {
            return None;
        }
        attempt.ended_by = Some(ended_by.to_string());
        let ev = attempt.evidence.clone();

        if ev.reached_checkered {
            attempt.status = "finished".to_string();
            attempt.reason = Some("Cruzou a bandeira quadriculada".to_string());
            // Carro completou ⇒ dano não foi terminal: rebaixa as batidas.
            for crash in attempt.crashes.iter_mut() {
                crash.severity = downgrade(&crash.severity).to_string();
            }
        } else if !ev.raced {
            attempt.status = "not_started".to_string();
            attempt.reason = Some("Não chegou a largar".to_string());
        } else {
            attempt.status = "dnf".to_string();
            attempt.reason = Some(build_dnf_reason(attempt, &ev, ended_by));
        }

        // Pior batida (pela severidade FINAL já ajustada).
        attempt.worst_crash = attempt
            .crashes
            .iter()
            .max_by_key(|c| severity_rank(&c.severity))
            .map(|c| c.severity.clone());

        let number = attempt.number;
        let status = attempt.status.clone();
        let lap = attempt.laps_completed;
        let worst = attempt.worst_crash.clone();

        // (o borrow de `attempt` termina aqui; a partir daqui pode emitir)

        // Telemetria de produto: fecha a corrida com o status JÁ classificado acima
        // (finished | dnf | not_started) e o desfecho. Restart mantém a corrida aberta
        // — o servidor reabre pelo subsession_id na largada seguinte.
        //
        // Fica DEPOIS do borrow porque o desfecho lê `self.history`, e não antes como
        // era: o `attempt` emprestado mutavelmente bloquearia a leitura.
        if ended_by != "restart" {
            let outcome = self.build_race_outcome(&ev, lap, number, worst.clone());
            crate::telemetry::race_end(&status, Some(outcome));
        }

        let now = self.live_session_time;
        if ended_by == "restart" {
            self.emit(
                now,
                lap,
                "race_restarted",
                None,
                format!("Corrida reiniciada (#{number})"),
                None,
            );
        }
        if status == "dnf" {
            self.emit(
                now,
                lap,
                "dnf_confirmed",
                None,
                format!("DNF confirmado (#{number})"),
                worst,
            );
        }

        Some(format!(
            "Tentativa #{} encerrada: {}",
            number,
            status_pt(&status)
        ))
    }
}
