//! Pontuação de batida: os componentes do impacto, os rótulos de severidade e
//! o ciclo de abrir/fechar uma batida dentro da tentativa.

use super::*;

// ─── Pontuação por componente (com teto) ─────────────────────────────────────
#[derive(Clone, Copy)]
pub(crate) struct Components {
    pub(super) incident: f64,
    pub(super) g: f64,
    pub(super) speed: f64,
    pub(super) yaw: f64,
    pub(super) rot: f64,
    pub(super) tow: f64,
    pub(super) offtrack: f64,
}

impl Components {
    pub(super) const ZERO: Self = Self {
        incident: 0.0,
        g: 0.0,
        speed: 0.0,
        yaw: 0.0,
        rot: 0.0,
        tow: 0.0,
        offtrack: 0.0,
    };

    pub(super) fn total(&self) -> f64 {
        self.incident + self.g + self.speed + self.yaw + self.rot + self.tow + self.offtrack
    }

    pub(super) fn merge_max(&mut self, o: &Components) {
        self.incident = self.incident.max(o.incident);
        self.g = self.g.max(o.g);
        self.speed = self.speed.max(o.speed);
        self.yaw = self.yaw.max(o.yaw);
        self.rot = self.rot.max(o.rot);
        self.tow = self.tow.max(o.tow);
        self.offtrack = self.offtrack.max(o.offtrack);
    }
}

pub(crate) fn severity_label(score: f64) -> Severidade {
    if score >= SEV_CATASTROPHIC {
        Severidade::Catastrofico
    } else if score >= SEV_TOTALED {
        Severidade::Destruido
    } else if score >= SEV_SEVERE {
        Severidade::Grave
    } else if score >= SEV_MODERATE {
        Severidade::Moderado
    } else if score >= SEV_MINOR {
        Severidade::Leve
    } else {
        Severidade::Nenhum
    }
}

pub(crate) fn state_label(state: i32) -> String {
    match state {
        0 => "Invalid",
        1 => "GetInCar",
        2 => "Warmup",
        3 => "ParadeLaps",
        4 => "Racing",
        5 => "Checkered",
        6 => "CoolDown",
        _ => "Desconhecido",
    }
    .to_string()
}

pub(crate) fn surface_label(surface: i32) -> String {
    match surface {
        -1 => "NotInWorld",
        0 => "OffTrack",
        1 => "InPitStall",
        2 => "ApproachingPits",
        3 => "OnTrack",
        _ => "Desconhecido",
    }
    .to_string()
}

pub(crate) fn g_force(t: &IracingTelemetry) -> f64 {
    (t.lat_accel * t.lat_accel + t.long_accel * t.long_accel + t.vert_accel * t.vert_accel).sqrt()
        / GRAVITY
}

/// Clima EFETIVO do Sistema de Quebra: blend do baseline FIXO da corrida (do calendário, Peça 1)
/// com os canais VIVOS do SDK (Peça 2). Cada canal usa o vivo quando plausível/presente, senão
/// cai no baseline — captura o arco de chuva e a deriva de temperatura sem "sumir" o clima nos
/// primeiros segundos (antes da sessão popular os canais) nem em conteúdo que não reporte algum.
/// Progresso da corrida por TEMPO (0..1), do tempo de sessão do SDK. `(total − restante)/total`.
/// Fora de uma corrida com relógio (total ≤ 0) → 0. Só o enduro usa (rampa de desgaste do fim).
pub(crate) fn session_progress(t: &IracingTelemetry) -> f64 {
    if t.session_time_total > 0.0 {
        ((t.session_time_total - t.session_time_remain) / t.session_time_total).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

pub(crate) fn effective_weather(
    t: &IracingTelemetry,
    baseline: crate::car::breakdown::Weather,
) -> crate::car::breakdown::Weather {
    let mut w = baseline;
    // Temperatura do ar: usa o vivo se plausível (> 0 °C).
    if t.air_temp > 0.0 {
        w.temperature = t.air_temp;
    }
    // Molhado: `TrackWetness` 0=Unknown; 1=Dry .. 7=ExtremelyWet → 0..1. Só sobrepõe se conhecido.
    if t.track_wetness >= 1 {
        w.wetness = (((t.track_wetness - 1) as f64) / 6.0).clamp(0.0, 1.0);
    }
    // Umidade relativa (fração 0..1) → %. Só se lida (> 0).
    if t.relative_humidity > 0.0 {
        w.humidity = (t.relative_humidity * 100.0).clamp(0.0, 100.0);
    }
    // Vento m/s → km/h. Só se lido (> 0).
    if t.wind_ms > 0.0 {
        w.wind_kmh = t.wind_ms * 3.6;
    }
    w
}

impl RaceMonitor {
    // ── Batidas (scorer) ─────────────────────────────────────────────────────
    /// Pontua os sinais de batida deste tick (incidente, G, yaw, rotação, fora
    /// da pista). Reboque e velocidade perdida são tratados no `observe`/`close`.
    pub(super) fn score_tick(
        t: &IracingTelemetry,
        prev_incident: Option<i32>,
    ) -> (Components, Vec<String>) {
        let mut c = Components::ZERO;
        let mut factors: Vec<String> = Vec::new();

        if let Some(prev) = prev_incident {
            let delta = t.incident_count - prev;
            if delta > 0 {
                let (pts, kind) = if delta >= 4 {
                    (INCIDENT_4X, "contato (+4x)")
                } else if delta >= 2 {
                    (INCIDENT_2X, "rodada (+2x)")
                } else {
                    (INCIDENT_1X, "saída (+1x)")
                };
                c.incident = pts;
                factors.push(format!("incidente: {kind}"));
            }
        }

        if t.track_surface > SURFACE_NOT_IN_WORLD {
            let g_total = g_force(t);
            let g_impact = g_total - 1.0;
            if g_impact > G_THRESHOLD {
                c.g = ((g_impact - G_THRESHOLD) * G_RATE).min(G_CAP);
                factors.push(format!("impacto {g_total:.1}g"));
            }
            if t.yaw_rate.abs() > YAW_THRESHOLD {
                c.yaw = ((t.yaw_rate.abs() - YAW_THRESHOLD) * YAW_RATE_W).min(YAW_CAP);
                factors.push("guinada brusca (yaw)".to_string());
            }
            let rot = t.roll_rate.abs().max(t.pitch_rate.abs());
            if rot > ROT_THRESHOLD {
                c.rot = ((rot - ROT_THRESHOLD) * ROT_RATE_W).min(ROT_CAP);
                factors.push("rotação violenta (roll/pitch)".to_string());
            }
            if t.track_surface == SURFACE_OFF_TRACK {
                c.offtrack = OFFTRACK_PTS;
                factors.push("fora da pista".to_string());
            }
        }

        (c, factors)
    }

    pub(super) fn merge_crash_factors(&mut self, factors: Vec<String>) {
        for factor in factors {
            let cat = factor.split([':', ' ']).next().unwrap_or("");
            let dup = self
                .crash_factors
                .iter()
                .any(|e| e.split([':', ' ']).next() == Some(cat));
            if !dup {
                self.crash_factors.push(factor);
            }
        }
    }

    /// Carro (número) MAIS PRÓXIMO do jogador na pista neste tick — o provável
    /// culpado de um contato. Ignora o próprio jogador, o pace car e quem está no
    /// pit; só conta se estiver bem perto (mesmo ponto da volta, com wrap no 0/1).
    pub(super) fn nearest_contact_car(&self, t: &IracingTelemetry) -> Option<i32> {
        let me = t.lap_dist_pct;
        if me < 0.0 {
            return None;
        }
        t.cars
            .iter()
            .filter(|c| {
                c.idx != t.player_car_idx
                    && c.lap_dist_pct >= 0.0
                    && !c.on_pit_road
                    && (c.idx as usize) < 64
                    && !self.car_is_pace[c.idx as usize]
            })
            .map(|c| {
                let d = (c.lap_dist_pct - me).abs();
                (c.idx, d.min(1.0 - d)) // distância na volta, com wrap 0/1
            })
            .filter(|(_, d)| *d < CONTACT_NEAR_PCT)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .and_then(|(idx, _)| {
                let num = self.car_number[idx as usize];
                (num > 0).then_some(num)
            })
    }

    /// Pontos da VELOCIDADE PERDIDA na batida em curso, como ela está AGORA. Só conta com
    /// impacto (rodada/freada sem bater não).
    ///
    /// É o componente que separa o encostão da destruição — vale até
    /// [`SPEED_LOST_CAP`] pontos, mais que todos os outros somados. Vive à parte porque só o
    /// fechamento da batida o gravava, e quem precisa julgar o estrago NA HORA (o castigo da
    /// classificação) não pode esperar dez segundos de silêncio que podem nunca vir.
    pub(super) fn pontos_de_velocidade_perdida(&self) -> f64 {
        if !self.crash_had_impact {
            return 0.0;
        }
        let perdida = (self.crash_entry_speed_ms - self.crash_min_speed_ms).max(0.0);
        if perdida <= SPEED_LOST_THRESHOLD {
            return 0.0;
        }
        ((perdida - SPEED_LOST_THRESHOLD) * SPEED_LOST_RATE).min(SPEED_LOST_CAP)
    }

    /// O score da batida EM CURSO com a conta inteira (componentes + velocidade já perdida),
    /// sem esperar ela fechar. 0 quando não há batida aberta ou não houve impacto.
    pub(super) fn score_da_batida_em_curso(&self) -> f64 {
        if !self.in_crash || !self.crash_had_impact {
            return 0.0;
        }
        self.crash_components.total() + self.pontos_de_velocidade_perdida()
    }

    /// Fecha a batida em andamento e a anexa à tentativa atual.
    pub(super) fn close_crash(&mut self) {
        let speed_lost = (self.crash_entry_speed_ms - self.crash_min_speed_ms).max(0.0);
        let pontos = self.pontos_de_velocidade_perdida();
        if pontos > 0.0 {
            self.crash_components.speed = pontos;
            self.crash_factors
                .push(format!("perdeu {:.0} km/h", speed_lost * 3.6));
        }
        let score = self.crash_components.total();
        let sev = severity_label(score);
        let start_time = self.crash_start_time;
        let start_lap = self.crash_start_lap;
        let crash = CrashEvent {
            session_time: start_time,
            lap: start_lap,
            score,
            severity: sev,
            impact_severity: sev,
            had_impact: self.crash_had_impact,
            factors: std::mem::take(&mut self.crash_factors),
        };
        if let Some(attempt) = self.attempts.last_mut() {
            attempt.crashes.push(crash);
        }
        self.in_crash = false;

        // Batida grave+ = dano provável e suspeita de DNF (1º estágio).
        if sev >= Severidade::Grave {
            let rotulo = sev.as_str();
            self.emit(
                start_time,
                start_lap,
                "player_damage_detected",
                None,
                format!("Dano provável após batida {rotulo}"),
                Some(rotulo.to_string()),
            );
            if !self.dnf_probable {
                self.dnf_probable = true;
                self.emit(
                    start_time,
                    start_lap,
                    "possible_dnf",
                    None,
                    format!("Possível DNF após batida {rotulo}"),
                    Some(rotulo.to_string()),
                );
            }
        }
    }
}
