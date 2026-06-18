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

use serde::{Deserialize, Serialize};

use super::{CarSnapshot, IracingTelemetry};

// ─── Constantes de tentativa/sessão ──────────────────────────────────────────
const STATE_RACING: i32 = 4;
const STATE_CHECKERED: i32 = 5;
const SURFACE_NOT_IN_WORLD: i32 = -1;
const SURFACE_OFF_TRACK: i32 = 0;
const SURFACE_IN_PIT_STALL: i32 = 1;
const SURFACE_ON_TRACK: i32 = 3;
const SESSION_TIME_DROP_TOLERANCE: f64 = 1.0;
/// Salto de SessionTime (rebobinar/avançar replay) acima do qual zeramos os
/// relógios da IA para não interpretar como "parado"/restart.
const REPLAY_JUMP_SECS: f64 = 2.0;
/// Um reinício de verdade leva o SessionTime de volta para perto de ZERO (o
/// relógio do jogo zera, os carros voltam ao grid). Uma simples queda do tempo
/// (ex.: ao entrar no replay) NÃO é reinício.
const RESTART_RESET_MAX_SECS: f64 = 10.0;
const FLAG_CHECKERED: u32 = 0x0000_0001;
const FLAG_CAUTION: u32 = 0x0000_4000;
const FLAG_CAUTION_WAVING: u32 = 0x0000_8000;
const FLAG_BLACK: u32 = 0x0001_0000;
const FLAG_DISQUALIFY: u32 = 0x0002_0000;

// ─── RaceEventEngine ─────────────────────────────────────────────────────────
/// Quantos eventos manter no log (os mais antigos saem).
const MAX_EVENTS: usize = 60;
/// Teto de voltas guardadas no histórico (race trace). Corridas reais ficam bem
/// abaixo; é só um guarda contra crescimento ilimitado.
const MAX_HISTORY_LAPS: usize = 600;
/// Intervalo (s) entre amostras da batalha do jogador (à frente/atrás). ~1Hz
/// captura a briga sem peso (1h de corrida ≈ 3600 pontos minúsculos).
const NEIGHBOR_SAMPLE_SECS: f64 = 1.0;
/// Teto de amostras da batalha guardadas (≈ 83 min a 1Hz).
const MAX_TRACK_POINTS: usize = 5000;
/// Variação mínima de LapDistPct para considerar que um carro "andou".
const AI_PROGRESS_EPS: f64 = 0.0015;
/// Segundos sem progresso para sinalizar carro parado.
const AI_STOPPED_SECS: f64 = 10.0;
/// Segundos sem progresso para sinalizar provável DNF de IA.
const AI_DNF_SECS: f64 = 25.0;

// ─── RaceControlEngine ───────────────────────────────────────────────────────
/// Tempo mínimo com progresso ZERADO para um carro parado virar candidato a
/// bandeira. 2s sem progresso já é claramente anormal vs. o ritmo de corrida —
/// o carro realmente parou, então já consideramos quem vem atrás.
const YELLOW_MIN_STOP_SECS: f64 = 2.0;
/// Janela de pista ATRÁS do carro parado (fração da volta) dentro da qual outro
/// carro é considerado "chegando" — risco de colisão.
const DANGER_GAP: f64 = 0.10;
/// Quantos carros chegando configuram perigo (>= isto → recomenda bandeira).
const DANGER_CARS_MIN: usize = 1;
/// Janela para correlacionar a amarela do SessionFlags com nossa recomendação.
const YELLOW_CONFIRM_WINDOW_SECS: f64 = 12.0;
/// Cooldown após o verde: nos primeiros segundos da corrida o grid está
/// engarrafado (carros rápidos presos atrás dos lentos), então não decidimos
/// bandeira até o pelotão abrir.
const START_GRACE_SECS: f64 = 8.0;

// Cluster de pits = acidente: carros que reduziram muito o ritmo e foram ao box.
/// Janela de medição do ritmo (pace) de cada carro.
const PACE_WINDOW_SECS: f64 = 1.5;
/// Abaixo desta fração do ritmo do líder, o carro está "lento" (danificado).
const SLOW_PACE_FRACTION: f64 = 0.4;
/// Acima desta fração, consideramos que o carro JÁ atingiu ritmo de corrida.
/// "Lento" só conta depois disso — senão a largada (todos acelerando) vira
/// falso "carro lento".
const RACING_PACE_FRACTION: f64 = 0.6;
/// Pit logo após ficar lento NA PISTA conta como pit de incidente.
const SLOW_PIT_WINDOW_SECS: f64 = 6.0;
/// Janela e mínimo de pits de incidente para suspeitar de acidente coletivo.
const PIT_CLUSTER_WINDOW_SECS: f64 = 15.0;
const PIT_CLUSTER_MIN: usize = 2;
/// Não realertar sobre o mesmo cluster por este tempo.
const PIT_CLUSTER_COOLDOWN_SECS: f64 = 30.0;

// Setores: divide a pista para detectar acidente coletivo (carros em apuros no
// mesmo trecho ou em setores vizinhos).
const NUM_SECTORS: i32 = 20;
const COLLECTIVE_MIN: usize = 2;
const COLLECTIVE_COOLDOWN_SECS: f64 = 30.0;
/// A cada quantos ticks (~60 Hz) recarregar a classificação de carros do YAML.
const YAML_REFRESH_TICKS: u64 = 180;

// ─── Constantes de pontuação de batida (calibradas nos testes) ───────────────
const GRAVITY: f64 = 9.81;
/// Período do sampler com o iRacing CONECTADO (~60 Hz) — para não perder picos.
const SAMPLER_PERIOD_MS: u64 = 16;
/// Período OCIOSO (iRacing fechado): só espia a conexão devagar, custo ~zero.
const SAMPLER_IDLE_PERIOD_MS: u64 = 1000;
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

/// Um evento discreto da corrida (saída do RaceEventEngine).
#[derive(Clone, Serialize)]
pub struct RaceEvent {
    pub session_time: f64,
    pub lap: i32,
    /// race_started | race_restarted | race_finished | possible_dnf |
    /// dnf_confirmed | pit_entry | tow_detected | player_damage_detected |
    /// yellow_triggered | ai_offtrack | ai_stopped | ai_possible_dnf
    pub kind: String,
    /// Carro envolvido (eventos de IA); None para eventos do jogador/sessão.
    pub car_idx: Option<i32>,
    pub detail: String,
    pub severity: Option<String>,
}

/// Estado por carro que o RaceControl enxerga — para diagnóstico na UI.
#[derive(Clone, Serialize)]
pub struct CarDebug {
    pub idx: i32,
    pub is_player: bool,
    pub is_ai: bool,
    pub is_pace: bool,
    pub position: i32,
    pub lap_dist_pct: f64,
    pub sector: i32,
    pub track_surface: String,
    pub on_pit_road: bool,
    pub has_moved: bool,
    pub stalled_secs: f64,
    /// Ritmo como % do líder (100 = no ritmo; baixo = lento/danificado).
    pub pace_pct_of_leader: f64,
    /// Se está "em apuros" pelas regras (candidato a bandeira).
    pub in_trouble: bool,
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
    pub cars_count: i32,
    /// Batida ACONTECENDO agora (evento ainda aberto) — para feedback imediato.
    pub crash_in_progress: bool,
    /// Score acumulado da batida em andamento e sua severidade.
    pub crash_progress_score: f64,
    pub crash_progress_severity: String,
    /// Se a corrida está verde agora (gate das bandeiras).
    pub is_green: bool,
    /// Diagnóstico por carro (o que o RaceControl vê).
    pub cars_debug: Vec<CarDebug>,
    pub attempts: Vec<Attempt>,
    pub events: Vec<RaceEvent>,
}

// ─── Histórico volta a volta (painel pós-corrida) ───────────────────────────
/// O gap de um carro ao líder numa volta — um ponto do race trace.
#[derive(Clone, Serialize, Deserialize)]
pub struct CarGapPoint {
    pub idx: i32,
    pub position: i32,
    /// Gap ao líder em segundos (`CarIdxF2Time`); 0 para o líder.
    pub gap: f64,
}

/// Snapshot de todos os carros no instante em que o líder completou uma volta.
#[derive(Clone, Serialize, Deserialize)]
pub struct LapSnapshot {
    /// Volta do líder (1-based) — o eixo X do race trace.
    pub lap: i32,
    pub cars: Vec<CarGapPoint>,
}

/// Tempo de uma volta completa do jogador (consistência de ritmo).
#[derive(Clone, Serialize, Deserialize)]
pub struct PlayerLap {
    pub lap: i32,
    pub time: f64,
}

/// Um instante da "batalha" do jogador: sua posição/velocidade e quem está
/// imediatamente à frente e atrás, com os gaps. Amostrado num ritmo leve (~1Hz)
/// ao longo de toda a corrida, para mostrar a briga se desenvolvendo.
#[derive(Clone, Serialize, Deserialize)]
pub struct PlayerTrackPoint {
    /// Tempo de sessão do instante.
    pub session_time: f64,
    /// Volta do jogador.
    pub lap: i32,
    /// Posição do jogador.
    pub position: i32,
    /// Velocidade do jogador em km/h.
    pub speed_kmh: f64,
    /// Índice do carro à frente (-1 = ninguém / é líder).
    pub ahead_idx: i32,
    /// Gap para o carro à frente, em segundos.
    pub gap_ahead: f64,
    /// Índice do carro atrás (-1 = ninguém / é último).
    pub behind_idx: i32,
    /// Gap para o carro atrás, em segundos.
    pub gap_behind: f64,
}

/// Histórico volta a volta da tentativa atual, montado ao vivo para o painel
/// pós-corrida: race trace de posições, gap ao líder, ritmo do jogador e a
/// batalha (à frente/atrás) ao longo da corrida.
#[derive(Clone, Serialize, Deserialize)]
pub struct RaceHistory {
    /// Snapshots por volta do líder (race trace + gap ao líder).
    pub laps: Vec<LapSnapshot>,
    /// Tempos de volta do jogador.
    pub player_laps: Vec<PlayerLap>,
    /// A batalha do jogador (carro à frente/atrás) amostrada ao longo da corrida.
    pub player_track: Vec<PlayerTrackPoint>,
    /// Voltas (do líder) em que a bandeira amarela esteve ativa — a faixa amarela.
    pub yellow_laps: Vec<i32>,
    /// Índice do carro do jogador (destaca a linha dele no trace).
    pub player_car_idx: i32,
    /// Número da tentativa que este histórico cobre.
    pub attempt_number: i32,
    /// Se a tentativa que este histórico cobre já encerrou.
    pub finished: bool,
    /// Desfecho da tentativa (em PT) quando encerrada: "Finalizada", "DNF", etc.
    pub outcome: String,
}

impl RaceHistory {
    const fn empty() -> Self {
        Self {
            laps: Vec::new(),
            player_laps: Vec::new(),
            player_track: Vec::new(),
            yellow_laps: Vec::new(),
            player_car_idx: -1,
            attempt_number: 0,
            finished: false,
            outcome: String::new(),
        }
    }
}

/// Rastreamento por carro para os eventos de IA (parado/offtrack/DNF).
#[derive(Clone, Copy)]
struct CarMonitor {
    last_dist_pct: f64,
    last_move_time: f64,
    /// Se o carro JÁ se moveu de verdade (largou). Antes disso, ficar parado é
    /// só esperar a largada — não conta como incidente.
    has_moved: bool,
    /// Se o carro JÁ atingiu ritmo de corrida ao menos uma vez. "Lento" só conta
    /// depois disso (evita falso lento na aceleração da largada).
    has_raced: bool,
    stopped_emitted: bool,
    offtrack_emitted: bool,
    dnf_emitted: bool,
    yellow_rec_emitted: bool,
    // Ritmo (pace) e detecção de pit de incidente.
    pace_anchor_pct: f64,
    pace_anchor_time: f64,
    pace: f64,
    last_slow_time: f64,
    was_on_pit: bool,
    incident_pit_time: Option<f64>,
}

impl CarMonitor {
    const DEFAULT: Self = Self {
        last_dist_pct: -1.0,
        last_move_time: 0.0,
        has_moved: false,
        has_raced: false,
        stopped_emitted: false,
        offtrack_emitted: false,
        dnf_emitted: false,
        yellow_rec_emitted: false,
        pace_anchor_pct: -1.0,
        pace_anchor_time: 0.0,
        pace: 0.0,
        last_slow_time: -1000.0,
        was_on_pit: false,
        incident_pit_time: None,
    };
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
    live_cars_count: i32,
    live_session_time: f64,

    // RaceEventEngine
    events: Vec<RaceEvent>,
    prev_session_state: i32,
    prev_on_pit_road: bool,
    prev_caution: bool,
    race_started_emitted: bool,
    race_finished_emitted: bool,
    dnf_probable: bool,
    car_monitors: [CarMonitor; 64],

    // RaceControlEngine
    /// `session_time` da última recomendação de bandeira (para confirmar pelo flag).
    pending_yellow_time: Option<f64>,
    /// Já recomendamos bandeira pelo carro do jogador nesta tentativa.
    player_yellow_rec_emitted: bool,
    /// Último alerta de cluster de pits (para o cooldown).
    last_pit_cluster_alert: Option<f64>,
    /// Classificação dos carros (do YAML `DriverInfo`): é IA? é pace car?
    car_is_ai: [bool; 64],
    car_is_pace: [bool; 64],
    /// Último alerta de acidente coletivo por setor (cooldown).
    last_collective_alert: Option<f64>,
    /// Diagnóstico ao vivo por carro (para a UI) + se está verde.
    cars_debug: Vec<CarDebug>,
    live_is_green: bool,
    /// `session_time` do verde (largada) — para o cooldown de início.
    race_green_time: Option<f64>,

    // Histórico volta a volta (painel pós-corrida)
    history: RaceHistory,
    /// Última volta do líder já registrada no histórico.
    hist_leader_lap: i32,
    /// Última volta do jogador já registrada no histórico.
    hist_player_lap: i32,
    /// `session_time` da última amostra da batalha (à frente/atrás).
    hist_last_neighbor_time: f64,
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
            live_cars_count: 0,
            live_session_time: 0.0,
            events: Vec::new(),
            prev_session_state: 0,
            prev_on_pit_road: false,
            prev_caution: false,
            race_started_emitted: false,
            race_finished_emitted: false,
            dnf_probable: false,
            car_monitors: [CarMonitor::DEFAULT; 64],
            pending_yellow_time: None,
            player_yellow_rec_emitted: false,
            last_pit_cluster_alert: None,
            car_is_ai: [true; 64],
            car_is_pace: [false; 64],
            last_collective_alert: None,
            cars_debug: Vec::new(),
            live_is_green: false,
            race_green_time: None,
            history: RaceHistory::empty(),
            hist_leader_lap: 0,
            hist_player_lap: 0,
            hist_last_neighbor_time: 0.0,
        }
    }

    /// Monta o diagnóstico por carro (o que o RaceControl enxerga de cada um).
    fn build_cars_debug(&mut self, t: &IracingTelemetry) {
        let now = t.session_time;
        let flags = t.session_flags as u32;
        self.live_is_green =
            t.session_state == STATE_RACING && flags & (FLAG_CAUTION | FLAG_CAUTION_WAVING) == 0;
        let ref_pace = self.car_monitors.iter().map(|cm| cm.pace).fold(0.0_f64, f64::max);

        let mut out = Vec::with_capacity(t.cars.len());
        for car in &t.cars {
            let valid = car.idx >= 0 && (car.idx as usize) < 64;
            let cm = if valid {
                self.car_monitors[car.idx as usize]
            } else {
                CarMonitor::DEFAULT
            };
            let stalled = if cm.has_moved { now - cm.last_move_time } else { 0.0 };
            let pace_pct = if ref_pace > 0.0 {
                (cm.pace / ref_pace * 100.0).clamp(0.0, 999.0)
            } else {
                0.0
            };
            let on_racing = car.track_surface == SURFACE_ON_TRACK
                || car.track_surface == SURFACE_OFF_TRACK;
            let monitorable = !car.is_player && self.is_monitorable_ai(car.idx);
            // "Em apuros" = PARADO na pista (mesmo critério dos gatilhos de
            // bandeira). Lento-vs-líder é tráfego, não conta.
            let in_trouble = monitorable
                && !car.on_pit_road
                && cm.has_moved
                && on_racing
                && stalled > YELLOW_MIN_STOP_SECS;
            out.push(CarDebug {
                idx: car.idx,
                is_player: car.is_player,
                is_ai: valid && self.car_is_ai[car.idx as usize],
                is_pace: valid && self.car_is_pace[car.idx as usize],
                position: car.position,
                lap_dist_pct: car.lap_dist_pct,
                sector: (car.lap_dist_pct * NUM_SECTORS as f64).floor() as i32,
                track_surface: surface_label(car.track_surface),
                on_pit_road: car.on_pit_road,
                has_moved: cm.has_moved,
                stalled_secs: stalled,
                pace_pct_of_leader: pace_pct,
                in_trouble,
            });
        }
        self.cars_debug = out;
    }

    /// Atualiza a classificação dos carros a partir do `DriverInfo` do YAML.
    fn set_car_classes(&mut self, classes: &[(bool, bool); 64]) {
        for i in 0..64 {
            self.car_is_ai[i] = classes[i].0;
            self.car_is_pace[i] = classes[i].1;
        }
    }

    /// Carro elegível para as regras de IA: é IA e NÃO é pace car.
    fn is_monitorable_ai(&self, idx: i32) -> bool {
        idx >= 0 && (idx as usize) < 64 && self.car_is_ai[idx as usize] && !self.car_is_pace[idx as usize]
    }

    /// Registra um evento no log (mantém os últimos [`MAX_EVENTS`]).
    fn emit(
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

    // ── Tentativas ───────────────────────────────────────────────────────────
    fn start_attempt(&mut self, session_time: f64) {
        self.current_attempt += 1;
        self.prev_surface = SURFACE_ON_TRACK;
        self.prev_incident = None;
        self.race_started_emitted = false;
        self.race_finished_emitted = false;
        self.dnf_probable = false;
        self.player_yellow_rec_emitted = false;
        self.last_pit_cluster_alert = None;
        self.last_collective_alert = None;
        self.race_green_time = None;
        // Nova tentativa = grade resetada; zera o rastreamento de movimento da IA.
        self.car_monitors = [CarMonitor::DEFAULT; 64];
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

        let number = attempt.number;
        let status = attempt.status.clone();
        let lap = attempt.laps_completed;
        let worst = attempt.worst_crash.clone();
        // (o borrow de `attempt` termina aqui; a partir daqui pode emitir)

        let now = self.live_session_time;
        if ended_by == "restart" {
            self.emit(now, lap, "race_restarted", None, format!("Corrida reiniciada (#{number})"), None);
        }
        if status == "dnf" {
            self.emit(now, lap, "dnf_confirmed", None, format!("DNF confirmado (#{number})"), worst);
        }

        Some(format!("Tentativa #{} encerrada: {}", number, status_pt(&status)))
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
        let start_time = self.crash_start_time;
        let start_lap = self.crash_start_lap;
        let crash = CrashEvent {
            session_time: start_time,
            lap: start_lap,
            score,
            severity: sev.clone(),
            impact_severity: sev.clone(),
            factors: std::mem::take(&mut self.crash_factors),
        };
        if let Some(attempt) = self.attempts.last_mut() {
            attempt.crashes.push(crash);
        }
        self.in_crash = false;

        // Batida grave+ = dano provável e suspeita de DNF (1º estágio).
        if severity_rank(&sev) >= severity_rank("grave") {
            self.emit(
                start_time,
                start_lap,
                "player_damage_detected",
                None,
                format!("Dano provável após batida {sev}"),
                Some(sev.clone()),
            );
            if !self.dnf_probable {
                self.dnf_probable = true;
                self.emit(
                    start_time,
                    start_lap,
                    "possible_dnf",
                    None,
                    format!("Possível DNF após batida {sev}"),
                    Some(sev),
                );
            }
        }
    }

    // ── Loop principal ───────────────────────────────────────────────────────
    /// Grava o histórico volta a volta da tentativa ativa: a cada volta do líder
    /// um snapshot de gaps/posições, o tempo de cada volta do jogador e as voltas
    /// em que a amarela esteve ativa. Reinicia quando começa uma nova tentativa.
    fn record_history(&mut self, t: &IracingTelemetry) {
        let now = t.session_time;

        // Marca o histórico como encerrado quando a tentativa que ele cobre fecha
        // (checkered/DNF/etc) — sinaliza ao painel que pode salvar/auto-salvar.
        let closed = self
            .attempts
            .last()
            .filter(|a| a.number == self.history.attempt_number && a.status != "active")
            .map(|a| status_pt(&a.status));
        if let Some(label) = closed {
            self.history.finished = true;
            self.history.outcome = label.to_string();
        }

        // Só grava com uma tentativa ATIVA (corrida em andamento ao vivo). Após o
        // fim (finished/dnf) paramos, mas mantemos o que já foi capturado.
        let attempt = match self.attempts.last() {
            Some(a) if a.status == "active" => a.number,
            _ => return,
        };
        // Tentativa nova → começa um histórico limpo.
        if self.history.attempt_number != attempt {
            self.history = RaceHistory::empty();
            self.history.attempt_number = attempt;
            self.hist_leader_lap = 0;
            self.hist_player_lap = 0;
            self.hist_last_neighbor_time = 0.0;
        }
        self.history.player_car_idx = t.player_car_idx;

        // Volta do líder = voltas do carro em P1 (ou o maior valor, na largada
        // quando as posições ainda não assentaram).
        let leader_lap = t
            .cars
            .iter()
            .filter(|c| c.position == 1)
            .map(|c| c.lap_completed)
            .max()
            .or_else(|| t.cars.iter().map(|c| c.lap_completed).max())
            .unwrap_or(0);

        // Amarela ativa nesta volta do líder → pinta a volta (faixa amarela).
        let caution = (t.session_flags as u32) & (FLAG_CAUTION | FLAG_CAUTION_WAVING) != 0;
        if caution && leader_lap >= 1 && !self.history.yellow_laps.contains(&leader_lap) {
            self.history.yellow_laps.push(leader_lap);
        }

        // Líder completou uma volta nova → snapshot de todos os carros.
        if leader_lap > self.hist_leader_lap && leader_lap >= 1 {
            self.hist_leader_lap = leader_lap;
            let cars: Vec<CarGapPoint> = t
                .cars
                .iter()
                .filter(|c| c.position >= 1) // ignora pace car / não classificados
                .map(|c| CarGapPoint {
                    idx: c.idx,
                    position: c.position,
                    gap: if c.f2_time.is_finite() { c.f2_time.max(0.0) } else { 0.0 },
                })
                .collect();
            self.history.laps.push(LapSnapshot { lap: leader_lap, cars });
            if self.history.laps.len() > MAX_HISTORY_LAPS {
                self.history.laps.remove(0);
            }
        }

        // Jogador completou uma volta nova → registra o tempo dela.
        if t.lap_completed > self.hist_player_lap {
            self.hist_player_lap = t.lap_completed;
            if t.last_lap_time > 0.0 {
                self.history.player_laps.push(PlayerLap {
                    lap: t.lap_completed,
                    time: t.last_lap_time,
                });
                if self.history.player_laps.len() > MAX_HISTORY_LAPS {
                    self.history.player_laps.remove(0);
                }
            }
        }

        // Batalha do jogador (carro à frente/atrás) — amostra leve a ~1Hz,
        // capturando a briga se desenvolvendo ao longo da corrida.
        if now - self.hist_last_neighbor_time >= NEIGHBOR_SAMPLE_SECS {
            if let Some(me) = t.cars.iter().find(|c| c.idx == t.player_car_idx) {
                if me.position >= 1 {
                    self.hist_last_neighbor_time = now;
                    let ahead = t.cars.iter().find(|c| c.position == me.position - 1);
                    let behind = t.cars.iter().find(|c| c.position == me.position + 1);
                    let gap_to = |other: &CarSnapshot| {
                        let d = (other.f2_time - me.f2_time).abs();
                        if d.is_finite() { d } else { 0.0 }
                    };
                    self.history.player_track.push(PlayerTrackPoint {
                        session_time: now,
                        lap: me.lap_completed,
                        position: me.position,
                        speed_kmh: t.speed_kmh,
                        ahead_idx: ahead.map(|c| c.idx).unwrap_or(-1),
                        gap_ahead: ahead.map(gap_to).unwrap_or(0.0),
                        behind_idx: behind.map(|c| c.idx).unwrap_or(-1),
                        gap_behind: behind.map(gap_to).unwrap_or(0.0),
                    });
                    if self.history.player_track.len() > MAX_TRACK_POINTS {
                        self.history.player_track.remove(0);
                    }
                }
            }
        }
    }

    fn observe(&mut self, t: &IracingTelemetry) {
        let now = t.session_time;

        // Salto de tempo (rebobinar/avançar o replay): zera os relógios da IA e
        // o prev do jogador, para não virar falso "parado"/restart.
        let jumped =
            self.live_session_time != 0.0 && (now - self.live_session_time).abs() > REPLAY_JUMP_SECS;
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
            self.emit(now, t.lap_completed, "yellow_triggered", None, "Bandeira amarela".to_string(), None);
            if let Some(rec) = self.pending_yellow_time {
                if now - rec <= YELLOW_CONFIRM_WINDOW_SECS {
                    self.emit(now, t.lap_completed, "yellow_confirmed", None, "Amarela confirmada pelo SessionFlags".to_string(), None);
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
        self.evaluate_race_control(t);
        self.build_cars_debug(t);
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
    fn process_player(&mut self, t: &IracingTelemetry) {
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

        // 4.5) Eventos de sessão/jogador.
        let lap = t.lap_completed;
        if self.prev_session_state < STATE_RACING
            && t.session_state >= STATE_RACING
            && t.session_state < STATE_CHECKERED
            && !self.race_started_emitted
        {
            self.race_started_emitted = true;
            self.emit(now, lap, "race_started", None, "Largada (verde)".to_string(), None);
        }

        let finished = self
            .attempts
            .last()
            .map(|a| a.evidence.reached_checkered)
            .unwrap_or(false);
        if finished && !self.race_finished_emitted {
            self.race_finished_emitted = true;
            self.emit(now, lap, "race_finished", None, "Cruzou a bandeirada".to_string(), None);
        }

        if !self.prev_on_pit_road && t.player_on_pit_road {
            self.emit(now, lap, "pit_entry", None, "Entrou no pit".to_string(), None);
        }
        if self.live_tow <= 0.0 && t.tow_time > 0.0 {
            self.emit(now, lap, "tow_detected", None, "Reboque acionado".to_string(), None);
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
    fn process_ai_cars(&mut self, t: &IracingTelemetry) {
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
            // Lento NA PISTA — só conta se o carro JÁ atingiu ritmo (não na largada).
            let on_racing_area =
                car.track_surface == SURFACE_ON_TRACK || car.track_surface == SURFACE_OFF_TRACK;
            if cm.has_raced && on_racing_area && ref_pace > 0.0 && cm.pace < SLOW_PACE_FRACTION * ref_pace
            {
                cm.last_slow_time = now;
            }
            // Pit de incidente: entrou no pit logo após ter ficado lento na pista.
            if car.on_pit_road && !cm.was_on_pit && now - cm.last_slow_time <= SLOW_PIT_WINDOW_SECS {
                cm.incident_pit_time = Some(now);
            }
            cm.was_on_pit = car.on_pit_road;

            self.car_monitors[i] = cm;

            let idx = car.idx;
            // Volta ATUAL do carro (CarIdxLap); fallback para completadas + 1.
            let car_lap = if car.lap > 0 { car.lap } else { car.lap_completed + 1 };
            if ev_offtrack {
                self.emit(now, car_lap, "ai_offtrack", Some(idx), format!("Carro {idx} saiu da pista"), None);
            }
            if ev_stopped {
                self.emit(now, car_lap, "ai_stopped", Some(idx), format!("Carro {idx} parado (~{stalled:.0}s)"), None);
            }
            if ev_dnf {
                self.emit(now, car_lap, "ai_possible_dnf", Some(idx), format!("Carro {idx} provável DNF (parado {stalled:.0}s)"), None);
            }
        }
    }

    /// RaceControlEngine: decide se recomenda bandeira. Um carro parado só vira
    /// bandeira se for PERIGO (há carros chegando na posição dele). Exceção: uma
    /// batida grave do próprio jogador (temos outros dados para confirmar).
    fn evaluate_race_control(&mut self, t: &IracingTelemetry) {
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
                    let lap = if car.lap > 0 { car.lap } else { car.lap_completed + 1 };
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
                let on_racing = car.track_surface == SURFACE_ON_TRACK
                    || car.track_surface == SURFACE_OFF_TRACK;
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
    fn recommend_yellow(&mut self, now: f64, lap: i32, car_idx: Option<i32>, detail: String) {
        self.pending_yellow_time = Some(now);
        self.emit(now, lap, "yellow_recommended", car_idx, detail, None);
        if AUTO_YELLOW.load(Ordering::Relaxed) {
            match super::race_control::throw_yellow() {
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

/// Lê `CarIsAI`/`CarIsPaceCar` por `CarIdx` do `DriverInfo` no YAML de sessão.
/// Retorna `[(is_ai, is_pace); 64]`. Varredura por linha (sem parser YAML).
fn parse_driver_classes(yaml: &str) -> [(bool, bool); 64] {
    let mut out = [(true, false); 64]; // padrão: IA, não pace (até o YAML dizer)
    let mut current: Option<usize> = None;
    for line in yaml.lines() {
        let t = line.trim();
        let t = t.strip_prefix("- ").unwrap_or(t);
        if let Some(rest) = t.strip_prefix("CarIdx:") {
            current = rest.trim().parse::<usize>().ok().filter(|n| *n < 64);
        } else if let Some(rest) = t.strip_prefix("CarIsAI:") {
            if let Some(i) = current {
                out[i].0 = rest.trim() == "1";
            }
        } else if let Some(rest) = t.strip_prefix("CarIsPaceCar:") {
            if let Some(i) = current {
                out[i].1 = rest.trim() == "1";
            }
        }
    }
    out
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

/// Quando ligado, o RaceControl não só recomenda como DISPARA a bandeira (envia
/// a macro `!y$` ao iRacing) automaticamente. Opt-in pelo usuário.
///
/// A preferência é PERSISTIDA em disco (um flag em `%TEMP%`), então sobrevive a
/// fechar o app / reiniciar o backend — sem isso o toggle "desmarcava sozinho".
static AUTO_YELLOW: AtomicBool = AtomicBool::new(false);
static AUTO_YELLOW_LOADED: std::sync::Once = std::sync::Once::new();

fn auto_yellow_file() -> std::path::PathBuf {
    std::env::temp_dir().join("loop_auto_yellow.flag")
}

/// Carrega a preferência salva para o AtomicBool, uma única vez por processo.
fn ensure_auto_yellow_loaded() {
    AUTO_YELLOW_LOADED.call_once(|| {
        if let Ok(s) = std::fs::read_to_string(auto_yellow_file()) {
            AUTO_YELLOW.store(s.trim() == "1", Ordering::Relaxed);
        }
    });
}

/// Liga/desliga o envio automático de bandeira (e persiste a escolha).
pub fn set_auto_yellow(enabled: bool) {
    ensure_auto_yellow_loaded();
    AUTO_YELLOW.store(enabled, Ordering::Relaxed);
    let _ = std::fs::write(auto_yellow_file(), if enabled { "1" } else { "0" });
}

/// Estado atual do envio automático (restaura do disco no primeiro acesso).
pub fn auto_yellow_enabled() -> bool {
    ensure_auto_yellow_loaded();
    AUTO_YELLOW.load(Ordering::Relaxed)
}

fn lock() -> std::sync::MutexGuard<'static, RaceMonitor> {
    MONITOR.lock().unwrap_or_else(|p| p.into_inner())
}

fn start_sampler() {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    ensure_auto_yellow_loaded();
    std::thread::spawn(|| {
        let mut tick = 0u64;
        loop {
        // O tick devolve se o iRacing estava CONECTADO — controla a cadência:
        // 60 Hz conectado (não perde picos), 1 Hz ocioso (só espia a conexão).
        let connected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        match super::read_telemetry() {
            Ok(t) => {
                // Recarrega a classificação de carros (IA/pace car) do YAML de
                // tempos em tempos — ela muda raramente, então não precisa ser
                // a cada tick.
                if tick % YAML_REFRESH_TICKS == 0 {
                    if let Ok(session) = super::read_session() {
                        let classes = parse_driver_classes(&session.session_yaml);
                        lock().set_car_classes(&classes);
                        // Captura o custid do jogador automaticamente (uma vez).
                        super::note_session_custid(&session.session_yaml);
                    }
                }
                lock().observe(&t);
                true
            }
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
                false
            }
        }
        }))
        .unwrap_or_else(|_| {
            eprintln!("[race_monitor] sampler: panic num tick (recuperado, sampler segue vivo)");
            false
        });
        tick = tick.wrapping_add(1);
        let period = if connected { SAMPLER_PERIOD_MS } else { SAMPLER_IDLE_PERIOD_MS };
        std::thread::sleep(std::time::Duration::from_millis(period));
        }
    });
}

/// Liga o sampler de fundo (idempotente). Chamado no boot do app e ao exportar
/// para o iRacing — assim o monitoramento e a captura do custid ligam sozinhos,
/// sem depender de nenhum toggle. Ocioso quando o sim está fechado.
pub fn start_watching() {
    start_sampler();
}

/// Se o iRacing está conectado agora (último tick do sampler).
pub fn is_connected() -> bool {
    start_sampler();
    lock().connected
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
        cars_count: m.live_cars_count,
        crash_in_progress: m.in_crash,
        crash_progress_score: if m.in_crash { m.crash_components.total() } else { 0.0 },
        crash_progress_severity: if m.in_crash {
            severity_label(m.crash_components.total()).to_string()
        } else {
            "nenhum".to_string()
        },
        is_green: m.live_is_green,
        cars_debug: m.cars_debug.clone(),
        attempts: m.attempts.clone(),
        events: m.events.clone(),
    }
}

/// Lê o histórico volta a volta acumulado (race trace + ritmo) para o painel
/// pós-corrida. Alimentado pelo mesmo sampler de ~60 Hz.
pub fn get_history() -> RaceHistory {
    start_sampler();
    lock().history.clone()
}

/// Zera o monitor para começar um novo teste.
pub fn reset() {
    *lock() = RaceMonitor::new();
}
