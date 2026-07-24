//! Controle de corrida: decide quando um carro parado vira PERIGO e recomenda
//! (ou dispara) a bandeira amarela.

use super::*;

impl RaceMonitor {
    /// RaceControlEngine: decide se recomenda bandeira. Um carro parado só vira
    /// bandeira se for PERIGO (há carros chegando na posição dele). Exceção: uma
    /// batida grave do próprio jogador (temos outros dados para confirmar).
    pub(super) fn evaluate_race_control(&mut self, t: &IracingTelemetry) {
        let now = t.session_time;
        let flags = t.session_flags as u32;
        let is_green =
            t.session_state == STATE_RACING && flags & (FLAG_CAUTION | FLAG_CAUTION_WAVING) == 0;
        // Cooldown de início: nos primeiros segundos após o verde, ninguém é
        // candidato (grid engarrafado, ritmos ainda se estabelecendo).
        let in_start_grace = self
            .race_green_time
            .map(|g| now - g < START_GRACE_SECS)
            .unwrap_or(false);

        if is_green && !in_start_grace {
            // 1) IA parada + confirmada por tempo + não pit + com carros chegando.
            for car in &t.cars {
                if car.is_player || !self.is_monitorable_ai(car.idx) {
                    continue;
                }
                if car.on_pit_road || car.lap_dist_pct < 0.0 {
                    continue;
                }
                let i = car.idx as usize;
                let cm = self.car_monitors[i];
                let stalled = now - cm.last_move_time;
                // Precisa ter largado (has_moved); parado no grid não conta.
                if !cm.has_moved
                    || cm.yellow_rec_emitted
                    || cm.last_dist_pct < 0.0
                    || stalled < YELLOW_MIN_STOP_SECS
                {
                    continue;
                }
                // Só é PERIGO se o carro parado está NA PISTA (na linha de corrida).
                // Parado no escape/grama (OffTrack) não ameaça quem vem atrás —
                // eles passam por ele tranquilos.
                if car.track_surface != SURFACE_ON_TRACK {
                    continue;
                }
                let approaching = count_approaching(&t.cars, car.lap_dist_pct, car.idx);
                if approaching >= DANGER_CARS_MIN {
                    self.car_monitors[i].yellow_rec_emitted = true;
                    let idx = car.idx;
                    let lap = if car.lap > 0 {
                        car.lap
                    } else {
                        car.lap_completed + 1
                    };
                    let detail = format!(
                        "Carro {idx} parado com {approaching} carro(s) chegando — bandeira recomendada"
                    );
                    self.recommend_yellow(now, lap, Some(idx), detail);
                }
            }

            // 2) Jogador: batida GRAVE EM ANDAMENTO → recomenda na hora (não
            // espera o evento fechar nem o carro parar; senão demora ~10s).
            if self.in_crash
                && self.crash_had_impact
                && !self.player_yellow_rec_emitted
                && !t.player_on_pit_road
                && t.track_surface > SURFACE_NOT_IN_WORLD
            {
                // Severidade ao vivo = componentes + velocidade já perdida até agora.
                let speed_lost = (self.crash_entry_speed_ms - self.crash_min_speed_ms).max(0.0);
                let speed_pts = if speed_lost > SPEED_LOST_THRESHOLD {
                    ((speed_lost - SPEED_LOST_THRESHOLD) * SPEED_LOST_RATE).min(SPEED_LOST_CAP)
                } else {
                    0.0
                };
                let live_score = self.crash_components.total() + speed_pts;
                if live_score >= SEV_SEVERE {
                    self.player_yellow_rec_emitted = true;
                    let detail = format!(
                        "Batida {} do jogador — bandeira recomendada",
                        severity_label(live_score)
                    );
                    self.recommend_yellow(now, t.lap_completed + 1, None, detail);
                }
            }

            // 3) Cluster de pits de incidente: vários carros reduziram o ritmo na
            // pista e foram ao box em pouco tempo = provável acidente coletivo.
            let pit_incidents = self
                .car_monitors
                .iter()
                .filter(|cm| {
                    cm.incident_pit_time
                        .map(|t| now - t <= PIT_CLUSTER_WINDOW_SECS)
                        .unwrap_or(false)
                })
                .count();
            let recent_alert = self
                .last_pit_cluster_alert
                .map(|t| now - t < PIT_CLUSTER_COOLDOWN_SECS)
                .unwrap_or(false);
            if pit_incidents >= PIT_CLUSTER_MIN && !recent_alert {
                self.last_pit_cluster_alert = Some(now);
                let detail = format!(
                    "{pit_incidents} carros reduziram o ritmo e foram ao pit — possível acidente"
                );
                self.recommend_yellow(now, t.lap_completed + 1, None, detail);
            }

            // 4) Acidente COLETIVO por setor: vários carros PARADOS no mesmo
            // trecho (setor ± 1) = bandeira com mais confiança e mais rápido.
            let mut trouble: Vec<i32> = Vec::new(); // setores dos carros parados
            for car in &t.cars {
                if car.is_player || !self.is_monitorable_ai(car.idx) || car.on_pit_road {
                    continue;
                }
                let cm = self.car_monitors[car.idx as usize];
                if !cm.has_moved {
                    continue;
                }
                let on_racing =
                    car.track_surface == SURFACE_ON_TRACK || car.track_surface == SURFACE_OFF_TRACK;
                let stalled = now - cm.last_move_time;
                // Acidente coletivo = carros PARADOS no mesmo trecho. NÃO conta
                // "lento": um pelotão em tráfego é lento (vs líder) mas não parou.
                if on_racing && cm.has_moved && stalled > YELLOW_MIN_STOP_SECS {
                    trouble.push((car.lap_dist_pct * NUM_SECTORS as f64).floor() as i32);
                }
            }
            // Existe um trecho (setor ± 1) com COLLECTIVE_MIN+ carros em apuros?
            let collective = trouble.iter().any(|sec_a| {
                trouble
                    .iter()
                    .filter(|sec_b| {
                        let d = (sec_a - *sec_b).abs();
                        d.min(NUM_SECTORS - d) <= 1 // vizinhos, com volta circular
                    })
                    .count()
                    >= COLLECTIVE_MIN
            });
            let recent_collective = self
                .last_collective_alert
                .map(|t| now - t < COLLECTIVE_COOLDOWN_SECS)
                .unwrap_or(false);
            if collective && !recent_collective {
                self.last_collective_alert = Some(now);
                let detail = format!(
                    "{} carros em apuros no mesmo trecho — acidente coletivo",
                    trouble.len()
                );
                self.recommend_yellow(now, t.lap_completed + 1, None, detail);
            }
        }

        // Expira uma recomendação não confirmada pelo SessionFlags.
        if let Some(rec) = self.pending_yellow_time {
            if now - rec > YELLOW_CONFIRM_WINDOW_SECS {
                self.pending_yellow_time = None;
            }
        }
    }

    /// Registra a recomendação de bandeira e, se o envio automático estiver
    /// ligado, dispara a macro `!y$` no iRacing.
    pub(super) fn recommend_yellow(&mut self, now: f64, lap: i32, car_idx: Option<i32>, detail: String) {
        self.pending_yellow_time = Some(now);
        self.emit(now, lap, "yellow_recommended", car_idx, detail, None);
        if AUTO_YELLOW.load(Ordering::Relaxed) {
            match crate::iracing_sdk::race_control::throw_yellow() {
                Ok(()) => self.emit(
                    now,
                    lap,
                    "yellow_sent",
                    car_idx,
                    "Bandeira !y$ enviada ao iRacing".to_string(),
                    None,
                ),
                Err(e) => self.emit(now, lap, "yellow_send_failed", car_idx, e, None),
            }
        }
    }
}
/// Conta carros "chegando" na posição `target_pct` (consciência espacial do
/// RaceControl): na pista, fora do pit, ATRÁS do carro parado e dentro da janela
/// de perigo — ou seja, vão passar pela posição dele em breve.
fn count_approaching(cars: &[CarSnapshot], target_pct: f64, target_idx: i32) -> usize {
    cars.iter()
        .filter(|c| {
            c.idx != target_idx
                && !c.on_pit_road
                && c.track_surface == SURFACE_ON_TRACK
                && c.lap_dist_pct >= 0.0
        })
        .filter(|c| {
            // Distância de pista que o carro precisa andar até a posição parada.
            let mut gap = target_pct - c.lap_dist_pct;
            if gap < 0.0 {
                gap += 1.0; // a pista é circular (0..1 por volta)
            }
            gap > 0.001 && gap <= DANGER_GAP
        })
        .count()
}
