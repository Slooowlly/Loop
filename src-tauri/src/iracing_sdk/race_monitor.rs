//! Monitor de corrida UNIFICADO.
//!
//! Consolida os três detectores que antes eram silos isolados (tentativas, DNF e
//! crash) num único modelo onde a **tentativa é o container** e tudo conversa:
//!
//! ```text
//! Tentativa #2
//! ├─ crashes[]   ← batidas pontuadas (leve..catastrófico), com a volta
//! ├─ evidence    ← saiu da pista, garagem, bandeiras, incidentes, bandeirada
//! └─ outcome     ← finished | dnf | not_started, com motivo que cita a pior batida
//! ```
//!
//! Regra-chave do desfecho: se o carro **cruzou a bandeirada**, não foi perda
//! total — mesmo que o impacto tenha sido grave, o carro ainda estava em uso.
//! Então as batidas de uma tentativa FINALIZADA têm a severidade **rebaixada um
//! nível** (o impacto bruto fica guardado como referência).
//!
//! Tudo é alimentado por um único amostrador de ~60 Hz, para nunca perder o pico
//! de um impacto.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::Serialize;

use super::IracingTelemetry;

// ─── Constantes de tentativa/sessão ──────────────────────────────────────────
const STATE_RACING: i32 = 4;
const STATE_CHECKERED: i32 = 5;
const SURFACE_NOT_IN_WORLD: i32 = -1;
const SURFACE_OFF_TRACK: i32 = 0;
const SURFACE_IN_PIT_STALL: i32 = 1;
const SURFACE_ON_TRACK: i32 = 3;
const SESSION_TIME_DROP_TOLERANCE: f64 = 1.0;
/// Um reinício de verdade leva o SessionTime de volta para perto de ZERO (o
/// relógio do jogo zera, os carros voltam ao grid). Uma simples queda do tempo
/// (ex.: ao entrar no replay) NÃO é reinício.
const RESTART_RESET_MAX_SECS: f64 = 10.0;
const FLAG_CHECKERED: u32 = 0x0000_0001;
const FLAG_BLACK: u32 = 0x0001_0000;
const FLAG_DISQUALIFY: u32 = 0x0002_0000;

// ─── Constantes de pontuação de batida (calibradas nos testes) ───────────────
const GRAVITY: f64 = 9.81;
const SAMPLER_PERIOD_MS: u64 = 16;
const MERGE_WINDOW_SECS: f64 = 10.0;

const INCIDENT_1X: f64 = 10.0;
const INCIDENT_2X: f64 = 20.0;
const INCIDENT_4X: f64 = 35.0;

const G_THRESHOLD: f64 = 3.0;
const G_RATE: f64 = 1.0;
const G_CAP: f64 = 30.0;

const SPEED_LOST_THRESHOLD: f64 = 3.0;
const SPEED_LOST_RATE: f64 = 4.0;
const SPEED_LOST_CAP: f64 = 160.0;

const YAW_THRESHOLD: f64 = 3.0;
const YAW_RATE_W: f64 = 10.0;
const YAW_CAP: f64 = 18.0;

const ROT_THRESHOLD: f64 = 3.5;
const ROT_RATE_W: f64 = 10.0;
const ROT_CAP: f64 = 15.0;

const TOW_PTS: f64 = 25.0;
const OFFTRACK_PTS: f64 = 8.0;

const SEV_MINOR: f64 = 15.0;
const SEV_MODERATE: f64 = 50.0;
const SEV_SEVERE: f64 = 110.0;
const SEV_TOTALED: f64 = 170.0;
const SEV_CATASTROPHIC: f64 = 230.0;

/// Ordem das severidades, do menor para o maior. Índice usado para rebaixar.
const SEVERITIES: [&str; 5] = ["leve", "moderado", "grave", "destruído", "catastrófico"];

// ─── Pontuação por componente (com teto) ─────────────────────────────────────
#[derive(Clone, Copy)]
struct Components {
    incident: f64,
    g: f64,
    speed: f64,
    yaw: f64,
    rot: f64,
    tow: f64,
    offtrack: f64,
}

impl Components {
    const ZERO: Self = Self {
        incident: 0.0,
        g: 0.0,
        speed: 0.0,
        yaw: 0.0,
        rot: 0.0,
        tow: 0.0,
        offtrack: 0.0,
    };

    fn total(&self) -> f64 {
        self.incident + self.g + self.speed + self.yaw + self.rot + self.tow + self.offtrack
    }

    fn merge_max(&mut self, o: &Components) {
        self.incident = self.incident.max(o.incident);
        self.g = self.g.max(o.g);
        self.speed = self.speed.max(o.speed);
        self.yaw = self.yaw.max(o.yaw);
        self.rot = self.rot.max(o.rot);
        self.tow = self.tow.max(o.tow);
        self.offtrack = self.offtrack.max(o.offtrack);
    }
}

fn severity_label(score: f64) -> &'static str {
    if score >= SEV_CATASTROPHIC {
        "catastrófico"
    } else if score >= SEV_TOTALED {
        "destruído"
    } else if score >= SEV_SEVERE {
        "grave"
    } else if score >= SEV_MODERATE {
        "moderado"
    } else if score >= SEV_MINOR {
        "leve"
    } else {
        "nenhum"
    }
}

/// Rebaixa uma severidade em um nível (carro sobreviveu/completou).
fn downgrade(severity: &str) -> &'static str {
    match SEVERITIES.iter().position(|s| *s == severity) {
        Some(0) | None => "leve",
        Some(i) => SEVERITIES[i - 1],
    }
}

fn state_label(state: i32) -> String {
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

fn surface_label(surface: i32) -> String {
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

fn g_force(t: &IracingTelemetry) -> f64 {
    (t.lat_accel * t.lat_accel + t.long_accel * t.long_accel + t.vert_accel * t.vert_accel)
        .sqrt()
        / GRAVITY
}

// ─── Modelo exposto à UI ─────────────────────────────────────────────────────
/// Uma batida registrada numa tentativa.
#[derive(Clone, Serialize)]
pub struct CrashEvent {
    pub session_time: f64,
    pub lap: i32,
    pub score: f64,
    /// Severidade final (pode ter sido rebaixada por a tentativa ter completado).
    pub severity: String,
    /// Severidade do impacto bruto, antes de qualquer rebaixamento.
    pub impact_severity: String,
    pub factors: Vec<String>,
}

/// Evidências (fora as batidas) acumuladas durante a tentativa.
#[derive(Clone, Default, Serialize)]
pub struct AttemptEvidence {
    pub raced: bool,
    pub reached_checkered: bool,
    pub off_track: bool,
    pub not_in_world: bool,
    pub towed_to_pit: bool,
    pub garage: bool,
    pub black_flag: bool,
    pub disqualified: bool,
    pub incident_points: i32,
}

/// Uma tentativa de corrida e seu desfecho.
#[derive(Clone, Serialize)]
pub struct Attempt {
    pub number: i32,
    pub status: String, // active | finished | dnf | not_started
    pub started_at_session_time: f64,
    pub laps_completed: i32,
    pub ended_by: Option<String>, // restart | sim_closed | checkered
    pub reason: Option<String>,
    pub worst_crash: Option<String>,
    pub evidence: AttemptEvidence,
    pub crashes: Vec<CrashEvent>,
}

/// Status completo devolvido ao frontend.
#[derive(Clone, Serialize)]
pub struct RaceStatus {
    pub connected: bool,
    pub attempt_number: i32,
    pub event: Option<String>,
    // Sinais ao vivo
    pub session_state_label: String,
    pub track_surface_label: String,
    pub lap_completed: i32,
    pub incident_count: i32,
    pub crash_score: f64,
    pub crash_severity_now: String,
    pub g_force: f64,
    pub speed_kmh: f64,
    pub tow_time: f64,
    pub attempts: Vec<Attempt>,
}

// ─── Estado interno do monitor ───────────────────────────────────────────────
#[derive(Clone, Copy)]
struct Snapshot {
    session_time: f64,
    lap_completed: i32,
}

struct RaceMonitor {
    // Tentativas
    prev: Option<Snapshot>,
    prev_surface: i32,
    prev_incident: Option<i32>,
    current_attempt: i32,
    attempts: Vec<Attempt>,
    was_connected: bool,
    pending_event: Option<String>,

    // Batida em andamento (escopada à tentativa atual)
    in_crash: bool,
    crash_components: Components,
    crash_factors: Vec<String>,
    crash_start_time: f64,
    crash_start_lap: i32,
    crash_last_above: Option<f64>,
    cruise_speed_ms: f64,
    crash_entry_speed_ms: f64,
    crash_min_speed_ms: f64,
    crash_had_impact: bool,

    // Snapshot ao vivo
    connected: bool,
    live_score: f64,
    live_g: f64,
    live_speed_kmh: f64,
    live_tow: f64,
    live_state: i32,
    live_surface: i32,
    live_lap: i32,
    live_incident: i32,
}

impl RaceMonitor {
    const fn new() -> Self {
        Self {
            prev: None,
            prev_surface: SURFACE_ON_TRACK,
            prev_incident: None,
            current_attempt: 0,
            attempts: Vec::new(),
            was_connected: false,
            pending_event: None,
            in_crash: false,
            crash_components: Components::ZERO,
            crash_factors: Vec::new(),
            crash_start_time: 0.0,
            crash_start_lap: 0,
            crash_last_above: None,
            cruise_speed_ms: 0.0,
            crash_entry_speed_ms: 0.0,
            crash_min_speed_ms: 0.0,
            crash_had_impact: false,
            connected: false,
            live_score: 0.0,
            live_g: 0.0,
            live_speed_kmh: 0.0,
            live_tow: 0.0,
            live_state: 0,
            live_surface: 0,
            live_lap: 0,
            live_incident: 0,
        }
    }

    // ── Tentativas ───────────────────────────────────────────────────────────
    fn start_attempt(&mut self, session_time: f64) {
        self.current_attempt += 1;
        self.prev_surface = SURFACE_ON_TRACK;
        self.prev_incident = None;
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
        });
    }

    fn ensure_active(&mut self, session_time: f64) {
        let need = match self.attempts.last() {
            None => true,
            Some(a) => a.status != "active",
        };
        if need {
            self.start_attempt(session_time);
        }
    }

    fn restarted(prev: &Snapshot, cur: &Snapshot) -> bool {
        let time_reset = cur.session_time + SESSION_TIME_DROP_TOLERANCE < prev.session_time
            && cur.session_time < RESTART_RESET_MAX_SECS;
        time_reset || cur.lap_completed < prev.lap_completed
    }

    /// Fecha a tentativa ativa, classificando o desfecho e (se finalizada)
    /// rebaixando a severidade das batidas. Retorna o texto do evento.
    fn finalize_attempt(&mut self, ended_by: &str) -> Option<String> {
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

        Some(format!(
            "Tentativa #{} encerrada: {}",
            attempt.number,
            status_pt(&attempt.status)
        ))
    }

    // ── Batidas (scorer) ─────────────────────────────────────────────────────
    /// Pontua os sinais de batida deste tick (incidente, G, yaw, rotação, fora
    /// da pista). Reboque e velocidade perdida são tratados no `observe`/`close`.
    fn score_tick(t: &IracingTelemetry, prev_incident: Option<i32>) -> (Components, Vec<String>) {
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

    fn merge_crash_factors(&mut self, factors: Vec<String>) {
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

    /// Fecha a batida em andamento e a anexa à tentativa atual.
    fn close_crash(&mut self) {
        // Velocidade perdida só conta com impacto (rodada/freada sem bater não).
        let speed_lost = (self.crash_entry_speed_ms - self.crash_min_speed_ms).max(0.0);
        if self.crash_had_impact && speed_lost > SPEED_LOST_THRESHOLD {
            self.crash_components.speed =
                ((speed_lost - SPEED_LOST_THRESHOLD) * SPEED_LOST_RATE).min(SPEED_LOST_CAP);
            self.crash_factors
                .push(format!("perdeu {:.0} km/h", speed_lost * 3.6));
        }
        let score = self.crash_components.total();
        let sev = severity_label(score).to_string();
        let crash = CrashEvent {
            session_time: self.crash_start_time,
            lap: self.crash_start_lap,
            score,
            severity: sev.clone(),
            impact_severity: sev,
            factors: std::mem::take(&mut self.crash_factors),
        };
        if let Some(attempt) = self.attempts.last_mut() {
            attempt.crashes.push(crash);
        }
        self.in_crash = false;
    }

    // ── Loop principal ───────────────────────────────────────────────────────
    fn observe(&mut self, t: &IracingTelemetry) {
        // Durante o replay o jogador está apenas assistindo: congela tudo. O
        // SessionTime do replay não pode ser confundido com um reinício, e nada
        // que aparece no replay conta como evidência/batida.
        if t.is_replay_playing {
            self.connected = true;
            self.was_connected = true;
            return;
        }

        let cur = Snapshot {
            session_time: t.session_time,
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
        self.ensure_active(t.session_time);
        self.prev = Some(cur);

        // 2) Evidências da tentativa.
        self.accumulate_evidence(t);

        // 3) Scorer de batida.
        let prev_incident = self.prev_incident;
        let (mut components, mut factors) = Self::score_tick(t, prev_incident);
        // Reboque acionado (transição 0 -> >0): trata aqui pois precisa do live_tow.
        if self.live_tow <= 0.0 && t.tow_time > 0.0 {
            components.tow = TOW_PTS;
            factors.push("reboque acionado".to_string());
        }
        let tick_score = components.total();

        // 4) Abre/funde/fecha a batida.
        let now = t.session_time;
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
            }
            self.crash_components.merge_max(&components);
            self.merge_crash_factors(factors);
            self.crash_last_above = Some(now);
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

        // 5) Atualiza estado para o próximo tick + snapshot ao vivo.
        self.prev_surface = t.track_surface;
        self.prev_incident = Some(t.incident_count);
        self.connected = true;
        self.was_connected = true;
        self.live_score = tick_score;
        self.live_g = g_force(t);
        self.live_speed_kmh = t.speed_kmh;
        self.live_tow = t.tow_time;
        self.live_state = t.session_state;
        self.live_surface = t.track_surface;
        self.live_lap = t.lap_completed;
        self.live_incident = t.incident_count;
    }

    fn accumulate_evidence(&mut self, t: &IracingTelemetry) {
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

// ─── Helpers de desfecho ─────────────────────────────────────────────────────
fn severity_rank(severity: &str) -> usize {
    SEVERITIES.iter().position(|s| *s == severity).map(|i| i + 1).unwrap_or(0)
}

fn status_pt(status: &str) -> &'static str {
    match status {
        "active" => "Ativa",
        "finished" => "Finalizada",
        "dnf" => "DNF",
        "not_started" => "Não largou",
        _ => "Desconhecido",
    }
}

/// Motivo do DNF: cita a PIOR batida (se houve) + como encerrou.
fn build_dnf_reason(attempt: &Attempt, ev: &AttemptEvidence, ended_by: &str) -> String {
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

// ─── Estado global + sampler ─────────────────────────────────────────────────
static MONITOR: Mutex<RaceMonitor> = Mutex::new(RaceMonitor::new());

fn lock() -> std::sync::MutexGuard<'static, RaceMonitor> {
    MONITOR.lock().unwrap_or_else(|p| p.into_inner())
}

fn start_sampler() {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| loop {
        match super::read_telemetry() {
            Ok(t) => lock().observe(&t),
            Err(error) => {
                let mut m = lock();
                // Sim fechado com tentativa ativa = DNF.
                let sim_closed = matches!(error, super::IracingError::NotRunning(_));
                if sim_closed && m.was_connected {
                    let active = m.attempts.last().map(|a| a.status == "active").unwrap_or(false);
                    if active {
                        m.pending_event = m.finalize_attempt("sim_closed");
                    }
                    m.was_connected = false;
                    m.prev = None;
                }
                m.connected = false;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(SAMPLER_PERIOD_MS));
    });
}

/// Lê o snapshot atual do monitor (alimentado a ~60 Hz pelo sampler).
pub fn poll() -> RaceStatus {
    start_sampler();
    let mut m = lock();
    let event = m.pending_event.take();
    RaceStatus {
        connected: m.connected,
        attempt_number: m.current_attempt,
        event,
        session_state_label: state_label(m.live_state),
        track_surface_label: surface_label(m.live_surface),
        lap_completed: m.live_lap,
        incident_count: m.live_incident,
        crash_score: m.live_score,
        crash_severity_now: severity_label(m.live_score).to_string(),
        g_force: m.live_g,
        speed_kmh: m.live_speed_kmh,
        tow_time: m.live_tow,
        attempts: m.attempts.clone(),
    }
}

/// Zera o monitor para começar um novo teste.
pub fn reset() {
    *lock() = RaceMonitor::new();
}
