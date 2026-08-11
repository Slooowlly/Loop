//! Ciclo de vida da tentativa: abrir, garantir ativa, detectar restart e
//! fechar com o desfecho (finished/dnf/not_started).

use super::*;

impl RaceMonitor {
    // ── Tentativas ───────────────────────────────────────────────────────────
    pub(super) fn start_attempt(&mut self, session_time: f64) {
        self.current_attempt += 1;
        self.prev_surface = SURFACE_ON_TRACK;
        self.prev_incident = None;
        // Sem leitura anterior de reparo: o primeiro tick da tentativa nova só ancora o
        // valor. Senão o reparo pendente que sobrou da sessão passada viraria um "salto"
        // e confirmaria uma batida que não é desta corrida.
        self.prev_repair_needed_s = -1.0;
        self.player_incidents.clear();
        self.race_started_emitted = false;
        self.race_finished_emitted = false;
        // Restart = a bandeirada anterior não vale mais; o cabeçalho da torre volta a
        // contar a volta em curso normalmente.
        self.volta_final_lider = 0;
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
        self.avancar_radio_epoch();
        self.breakdown_log.clear();
        self.player_risk_warned = [false; 11];
        self.player_warning_log.clear();
        self.poupar_avisado = false;
        self.ritmo.reiniciar();
        self.ritmo_log.clear();
        self.ritmo_ultima_volta = -1;
        // O log de EVENTOS é da tentativa, como todo o resto acima. Ele é lido no import
        // (`ai_dnfs`/`ai_worst_incident` na ponte de sessão e o `extra_dnf_numbers` do
        // resultado de AI season), então um abandono ou um incidente flagrado na tentativa
        // que o jogador jogou fora entraria no resultado da corrida que valeu, e daí na
        // notícia, no resumo e na carreira.
        self.events.clear();
        // As voltas de pit do jogador moram fora do `history` (que já nasce zerado por
        // tentativa): sem isto, a in/out lap de uma tentativa descartada seguia sendo
        // excluída do ritmo e desenhada no overlay de uma corrida em que ela não existiu.
        self.player_pit_laps.clear();
        // O ciclo de volta também é da tentativa. Uma volta aberta na tentativa abandonada
        // fecharia na corrida nova, com o tempo e a marca de box de uma corrida que o jogador
        // jogou fora.
        self.voltas_jogador.reiniciar();
        for coletor in self.voltas_carro.iter_mut() {
            coletor.reiniciar();
        }
        // Comando de quebra ainda na fila é da tentativa que morreu. O disparo é
        // estrangulado (~1 a cada 1,5 s), então um `!black`/`!dq` enfileirado lá chegaria ao
        // sim já na tentativa nova, punindo um carro por uma quebra que não aconteceu nesta
        // corrida — e a penalidade é real, entra no resultado.
        self.pending_breakdown_cmds.clear();
        // Estado vivo do sistema de quebra: triângulo do overlay, flash da torre, parada de
        // reparo e o latch do aviso de comando não entregue. Tudo nasceu na tentativa
        // anterior e nada disso aconteceu nesta.
        self.breakdown_alert = [None; 64];
        self.breakdown_prev_on_pit = [false; 64];
        self.breakdown_repair_laps.clear();
        self.breakdown_flash_at = [0.0; 64];
        self.breakdown_progress = 0.0;
        self.chat_send_warned = false;
        // Corrida nova, largada nova: a carência de largada da quebra recomeça.
        self.breakdown_green_at = None;
        // E o DIRETOR volta ao estado em que foi instalado: as voltas da tentativa
        // abandonada consumiram vida de peça e podem ter quebrado carros, e esse desgaste
        // vira consequência de carreira (conserto, abandono, notícia) de uma corrida que o
        // jogador não correu.
        self.restaurar_diretor_de_quebra();
        // A tendência de gap compara duas amostras no tempo; atravessar o reinício com o
        // histórico intacto inventaria um ganho de dezenas de segundos entre dois instantes
        // que nunca foram vizinhos.
        self.gap_hist.clear();
        self.pending_yellow_time = None;
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
            sim_repair_needed_s: 0.0,
            sim_repair_required_s: 0.0,
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
        worst_crash: Option<String>,
    ) -> crate::telemetry::RaceOutcome {
        let mut out = crate::telemetry::RaceOutcome {
            voltas: laps,
            incidentes: ev.incident_points,
            // Contados por sessão, na borda do reinício. O número da tentativa saiu da
            // assinatura junto com o bug: ela também é recriada a cada troca de sessão,
            // então um fim de semana limpo (treino → quali → corrida) reportava dois
            // reinícios que não houve.
            restarts: self.restarts_corrida,
            restarts_quali: self.restarts_quali,
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
        // — o servidor reabre pelo subsession_id na largada seguinte. Troca de sessão
        // também não reporta: fechar o treino ou a classificação ali é bookkeeping do
        // container de dano, não uma corrida que terminou.
        //
        // Fica DEPOIS do borrow porque o desfecho lê `self.history`, e não antes como
        // era: o `attempt` emprestado mutavelmente bloquearia a leitura.
        if !matches!(ended_by, "restart" | "session_change") {
            let outcome = self.build_race_outcome(&ev, lap, worst.clone());
            crate::telemetry::race_end(&status, Some(outcome));
        }

        let now = self.live_session_time;
        // O marcador de reinício NÃO sai daqui: ele pertence ao log da tentativa nova, que
        // ainda nem abriu (ver `process_player`). Emitido aqui, cairia no log da tentativa
        // abandonada e morreria com ela.
        //
        // Trocar de sessão não é abandono: o jogador só saiu do treino/da quali para a
        // corrida. Anunciar "DNF confirmado" no feed no exato instante em que a corrida
        // abre seria mentira na cara do jogador.
        if status == "dnf" && ended_by != "session_change" {
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
