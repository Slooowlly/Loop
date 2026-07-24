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

mod controle_corrida;
mod historico;
mod observacao;
mod pontuacao;
mod quebras;
mod resultado;
mod sessao;
mod tentativas;
mod tipos;

pub(crate) use observacao::*;
pub(crate) use pontuacao::*;
pub use resultado::*;
pub use tipos::*;
use sessao::*;

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
/// captura a briga sem peso (1h de corrida ≈ 7200 pontos minúsculos a 2Hz).
const NEIGHBOR_SAMPLE_SECS: f64 = 0.5;
/// Teto de amostras da batalha guardadas (≈ 41 min a 2Hz).
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
/// Intervalo mínimo (segundos) entre snapshots do trace disparados por TROCA de
/// posição. Evita que uma disputa lado-a-lado (posições trocando tick a tick) gere
/// um snapshot por frame. A virada de volta do líder ignora este throttle.
const MIN_TRACE_EVENT_GAP_SECS: f64 = 0.25;
/// Dwell mínimo (segundos) para uma passagem pela caixa contar como parada real.
/// `CarIdxTrackSurface == InPitStall` pode piscar por 1 frame (carro cruzando a
/// zona da caixa, ou leitura transiente do SDK) e gerar "pits" de ~0s. Uma parada
/// de verdade sempre fica estacionada por vários segundos.
const MIN_PIT_STALL_DWELL_SECS: f64 = 2.5;

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
pub(crate) const SEVERITIES: [&str; 5] = ["leve", "moderado", "grave", "destruído", "catastrófico"];

/// Distância máxima na volta (fração 0–1) para considerar outro carro "no mesmo
/// ponto" do jogador num contato — ~2% da pista (poucos comprimentos de carro).
const CONTACT_NEAR_PCT: f64 = 0.02;

/// Limite de voltas-por-carro guardadas (todos os carros × várias voltas).
const MAX_CAR_LAPS: usize = 4000;

/// Limite de paradas de box guardadas (todos os carros × várias paradas).
const MAX_PIT_STOPS: usize = 600;

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
    /// Classe (`CarClassID`) por carro — para a adaptação multiclasse.
    car_class_id: [i64; 64],
    /// Nome curto de cada classe (`CarClassShortName`) como `(class_id, nome)` —
    /// para as abas por categoria no overlay multiclasse. Vec (não HashMap) para
    /// caber no `const fn new()`.
    class_names: Vec<(i64, String)>,
    /// Nome do piloto (`UserName`) como `(car_idx, nome)` — para mostrar nomes em
    /// vez de números no overlay. Vec por causa do `const fn new()`.
    driver_names: Vec<(i32, String)>,
    /// Voltas do jogador que passaram pelo pit (in/out lap) — o ritmo as ignora.
    player_pit_laps: Vec<i32>,
    /// Running: o jogador passou pelo pit road na volta em andamento?
    player_pit_seen: bool,
    /// Pins do jogador no race trace (incidentes/saídas), com a posição na volta.
    player_incidents: Vec<PlayerIncidentMark>,
    /// Número do carro (`CarNumberRaw`) por carro — ponte p/ driver_id (Fase 3).
    car_number: [i32; 64],
    /// Redline do carro (`DriverInfo:DriverCarRedLine`) — referência do estilo de pilotagem
    /// (colado no limitador / short-shift). `None` até o YAML ser lido.
    car_redline: Option<f64>,
    /// Pista da sessão (`WeekendInfo:TrackID`) — copiada para o histórico.
    session_track_id: i64,
    /// Identidade única do evento (`WeekendInfo:SubSessionID`).
    session_subsession_id: i64,
    /// Carro do jogador nesta sessão (`CarScreenName`). Só telemetria de produto: sem
    /// ele, o tempo de volta não é comparável — 1:35 é rápido num carro e lento noutro.
    session_car_name: Option<String>,
    /// `SessionNum` da sessão de qualify (-1 se não houver) — detecta a quali.
    qualy_session_num: i32,
    /// Se estávamos em quali no tick anterior (detecta entrada numa quali nova).
    prev_in_qualy: bool,
    /// Voltas capturadas na sessão de quali (carregadas no histórico da corrida).
    qualy_laps: Vec<CarLap>,
    /// Última volta de quali já registrada por carro.
    qualy_car_lap_completed: [i32; 64],
    /// Último alerta de acidente coletivo por setor (cooldown).
    last_collective_alert: Option<f64>,
    /// Diagnóstico ao vivo por carro (para a UI) + se está verde.
    cars_debug: Vec<CarDebug>,
    live_is_green: bool,
    /// `session_time` do verde (largada) — para o cooldown de início.
    race_green_time: Option<f64>,

    // Histórico volta a volta (painel pós-corrida)
    history: RaceHistory,
    /// `session_num` que o histórico atual cobre. Muda (quali/treino → corrida) →
    /// o histórico é zerado pra corrida não herdar nada da sessão anterior. -1 = ainda
    /// não há sessão associada.
    hist_session_num: i32,
    /// Última volta do líder já registrada no histórico.
    hist_leader_lap: i32,
    /// Última volta do jogador já registrada no histórico.
    hist_player_lap: i32,
    /// Última volta completada já registrada POR CARRO (detecta fim de volta da IA).
    hist_car_lap_completed: [i32; 64],
    /// Última posição VÁLIDA (≥1) conhecida por carro. `CarIdxPosition` pisca 0
    /// quando o carro está no box/entre estados; guardamos a última boa pra ele não
    /// sumir do race trace num tick ruim. 0 = nunca teve posição válida.
    hist_car_last_pos: [i32; 64],
    /// Posição de cada carro no ÚLTIMO snapshot do trace — base pra detectar troca
    /// de posição (evento) e gravar o ponto na hora. 0 = ainda sem snapshot.
    hist_trace_pos: [i32; 64],
    /// `session_time` do último snapshot por EVENTO (troca de posição). Throttle
    /// leve pra oscilação lado-a-lado não gerar um snapshot por tick.
    hist_last_trace_event_time: f64,
    /// `session_time` da última amostra da batalha (à frente/atrás).
    hist_last_neighbor_time: f64,
    /// Posição na classe de cada carro no instante da LARGADA (bandeira verde) =
    /// o grid. 0 = ainda não capturado. Persiste entre resets de histórico; é
    /// gravado em `CarMeta.grid_class_position` no upsert do resumo por carro.
    grid_class_pos: [i32; 64],

    // ── Detector de pit (estratégia de pneu — todos os carros) ──────────────
    /// Carro está PARADO na caixa agora (`CarIdxTrackSurface == InPitStall`).
    pit_in_stall: [bool; 64],
    /// `session_time` em que o carro entrou na caixa (início do dwell).
    pit_stall_enter_time: [f64; 64],
    /// Volta em que o carro entrou na caixa.
    pit_stall_enter_lap: [i32; 64],
    /// Pista estava molhada no instante em que o carro parou na caixa.
    pit_stall_wet: [bool; 64],
    /// Já capturamos o clima da LARGADA nesta tentativa (uma vez, no verde).
    weather_start_captured: bool,

    // ── Parciais por setor do jogador (pista dividida em 3) ─────────────────
    /// Setor em que o jogador estava no tick anterior (0..2). -1 = ainda sem base.
    sec_prev: i32,
    /// `session_time` de quando o jogador entrou no setor atual.
    sec_enter_time: f64,
    /// Entrou NESTE setor no começo dele (não no meio) → o parcial é válido.
    sec_clean: bool,

    // ── Disparo de quebra AO VIVO (Sistema de Quebra) ───────────────────────
    /// Diretor da quebra da grade toda (por número de carro) — montado no verde/armado no
    /// debug. `None` = nada a disparar. A avaliação por volta produz comandos aqui.
    breakdown: Option<crate::car::breakdown::BreakdownDirector>,
    /// Comandos de admin (`!black`/`!dq`) a enviar — drenados FORA do lock e mandados via
    /// `send_chat_text` (que foca a janela + SendInput, não pode rodar segurando o lock).
    pending_breakdown_cmds: Vec<String>,
    /// DEBUG: pediu-se armar a GRADE TODA — montada no próximo tick (com `t.cars` em mãos,
    /// pra prender cada carro na volta atual). Uma peça perto de quebrar por carro.
    arm_grid_pending: bool,
    /// Clima FIXO da corrida (do export) que alimenta o `on_lap` do disparo REAL. `NEUTRAL`
    /// até um diretor de produção ser instalado. O clima vivo do SDK é o próximo refino (Peça 2).
    breakdown_weather: crate::car::breakdown::Weather,
    /// Estado de quebra do JOGADOR pendente de VÍNCULO: o número do jogador só é conhecido AO
    /// VIVO (`CarNumberRaw`), então o export guarda o `LiveBreakdown` dele aqui e o monitor o
    /// liga ao diretor no verde. `None` = jogador fora do disparo (ou já vinculado).
    pending_player_live: Option<crate::car::breakdown::LiveBreakdown>,
    /// Diretor de produção recém-instalado ainda NÃO preso à volta atual — o monitor faz o
    /// `prime_lap` de todos os carros no primeiro tick verde pra não retroagir voltas passadas.
    breakdown_needs_prime: bool,
    /// Estado de ALERTA de quebra por car_idx, pro overlay: quando uma peça larga o carro entra
    /// em alerta (leve/grave) até SAIR do box reparado; DNF fica persistente. Alimenta o
    /// triângulo laranja/vermelho e a bandeira preta da torre. Separado da fila de comandos
    /// (que é consumida e some) — este estado dura enquanto o problema existe.
    breakdown_alert: [Option<BreakdownAlert>; 64],
    /// `on_pit_road` do tick anterior por car_idx — detecta a SAÍDA do box (reparou → apaga o
    /// alerta de penalidade). Usado só pela máquina de estados do alerta.
    breakdown_prev_on_pit: [bool; 64],
    /// Log ESTRUTURADO dos desfechos de quebra da corrida (Peça 3) — acumula a corrida toda e é
    /// drenado no import → tabela `race_breakdowns` + debrief/notícia. Separado da fila de
    /// comandos (consumida e some) e do alerta do overlay (estado vivo).
    breakdown_log: Vec<BreakdownOutcome>,
    /// Paradas de REPARO: `(car_idx, volta de entrada)` — o carro entrou no box com peça
    /// quebrada (penalidade ativa). Alimenta o ícone de "peça" (triângulo) NO LUGAR do pneu na
    /// coluna de paradas do overlay. Zerado ao instalar um diretor novo.
    breakdown_repair_laps: Vec<(i32, u32)>,
    /// `session_time` da ÚLTIMA quebra de cada carro (car_idx). Alimenta o FLASH de 5 s na
    /// torre (a linha do piloto pisca quando o rádio anuncia o problema). 0 = nunca quebrou.
    breakdown_flash_at: [f64; 64],
    /// Progresso da corrida por TEMPO (0..1), atualizado a cada tick da grade a partir do
    /// tempo de sessão. Só o enduro usa (rampa de desgaste do fim); o tick do jogador reusa.
    breakdown_progress: f64,
    /// AVISO pessoal: peças do JOGADOR que já cruzaram o limiar de risco (`RISK_OPEN`) nesta
    /// corrida, por índice em `PartType::ALL`. Rearma quando a peça sai da zona (troca/reparo).
    player_risk_warned: [bool; 11],
    /// Log dos avisos pessoais (peça do jogador entrou na zona de risco) — o overlay mostra num
    /// card DISTINTO (voz em 2ª pessoa). Zerado ao instalar um diretor novo.
    player_warning_log: Vec<PlayerWarning>,
    /// Latch: um comando de quebra (`!black`/`!dq`) NÃO chegou ao iRacing nesta corrida
    /// (janela não encontrada / foreground recusado). Vira um aviso âmbar único no rádio
    /// ("os comandos não estão chegando; rode o sim em janela/borderless") em vez de a
    /// penalidade sumir em silêncio. Zerado ao instalar um diretor novo.
    chat_send_warned: bool,
}

/// Alerta de quebra de UM carro pro overlay: a severidade (vira triângulo laranja/vermelho no
/// leve/grave, ou bandeira preta no DNF) + se o carro já ENTROU no box desde a quebra (pra
/// apagar o alerta de penalidade quando ele SAIR reparado).
#[derive(Clone, Copy)]
struct BreakdownAlert {
    severity: crate::car::breakdown::Severity,
    entered_pit_since: bool,
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
            car_class_id: [0; 64],
            class_names: Vec::new(),
            driver_names: Vec::new(),
            player_pit_laps: Vec::new(),
            player_pit_seen: false,
            player_incidents: Vec::new(),
            car_number: [0; 64],
            car_redline: None,
            session_track_id: 0,
            session_subsession_id: 0,
            session_car_name: None,
            qualy_session_num: -1,
            prev_in_qualy: false,
            qualy_laps: Vec::new(),
            qualy_car_lap_completed: [0; 64],
            last_collective_alert: None,
            cars_debug: Vec::new(),
            live_is_green: false,
            race_green_time: None,
            history: RaceHistory::empty(),
            hist_session_num: -1,
            hist_leader_lap: 0,
            hist_player_lap: 0,
            hist_car_lap_completed: [0; 64],
            hist_car_last_pos: [0; 64],
            hist_trace_pos: [0; 64],
            hist_last_trace_event_time: 0.0,
            hist_last_neighbor_time: 0.0,
            grid_class_pos: [0; 64],
            pit_in_stall: [false; 64],
            pit_stall_enter_time: [0.0; 64],
            pit_stall_enter_lap: [0; 64],
            pit_stall_wet: [false; 64],
            weather_start_captured: false,
            sec_prev: -1,
            sec_enter_time: 0.0,
            sec_clean: false,
            breakdown: None,
            pending_breakdown_cmds: Vec::new(),
            arm_grid_pending: false,
            breakdown_weather: crate::car::breakdown::Weather::NEUTRAL,
            pending_player_live: None,
            breakdown_needs_prime: false,
            breakdown_alert: [None; 64],
            player_risk_warned: [false; 11],
            player_warning_log: Vec::new(),
            breakdown_prev_on_pit: [false; 64],
            breakdown_log: Vec::new(),
            breakdown_repair_laps: Vec::new(),
            breakdown_flash_at: [0.0; 64],
            breakdown_progress: 0.0,
            chat_send_warned: false,
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

/// Janela de tempo em que o app deve "puxar" o foco para si depois que o iRacing
/// fecha. Necessária porque a UI/launcher do iRacing se traz para frente alguns
/// segundos APÓS o sim fechar — então focamos uma vez na hora (bring-up) e, pelo
/// resto da janela, só RECONQUISTAMOS o foco se o iRacing o roubar de volta.
const FOCUS_SELF_WINDOW_SECS: u64 = 6;

/// `true` enquanto estamos dentro da janela pós-fechamento do sim.
static FOCUS_DEADLINE: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
/// `true` só no primeiro poll após o fechamento (o bring-up incondicional).
static FOCUS_INITIAL: AtomicBool = AtomicBool::new(false);

fn focus_deadline() -> std::sync::MutexGuard<'static, Option<std::time::Instant>> {
    FOCUS_DEADLINE.lock().unwrap_or_else(|p| p.into_inner())
}

/// Arma a janela de foco (borda conectado→fechado).
fn arm_focus_self() {
    *focus_deadline() =
        Some(std::time::Instant::now() + std::time::Duration::from_secs(FOCUS_SELF_WINDOW_SECS));
    FOCUS_INITIAL.store(true, Ordering::SeqCst);
}

/// Desarma a janela de foco (sim reconectou).
fn clear_focus_self() {
    if focus_deadline().take().is_some() {
        FOCUS_INITIAL.store(false, Ordering::SeqCst);
    }
}

/// Estado da janela de foco para o front: `(dentro_da_janela, é_o_bring_up)`.
/// `é_o_bring_up` só vem `true` no primeiro poll (consome). Fora da janela
/// devolve `(false, false)`.
pub fn poll_focus_self() -> (bool, bool) {
    let in_window = focus_deadline().map_or(false, |d| std::time::Instant::now() < d);
    if !in_window {
        FOCUS_INITIAL.store(false, Ordering::SeqCst);
        return (false, false);
    }
    (true, FOCUS_INITIAL.swap(false, Ordering::SeqCst))
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
                                let class_names = parse_class_names(&session.session_yaml);
                                let driver_names = parse_driver_names(&session.session_yaml);
                                let track_id = parse_track_id(&session.session_yaml);
                                let subsession_id = parse_subsession_id(&session.session_yaml);
                                let qualy_num = parse_qualy_session_num(&session.session_yaml);
                                let numbers = parse_car_numbers(&session.session_yaml);
                                let redline = super::parse_car_redline(&session.session_yaml);
                                let car_name = parse_player_car_name(&session.session_yaml);
                                {
                                    let mut m = lock();
                                    m.set_car_classes(&classes);
                                    m.set_class_names(class_names);
                                    m.set_driver_names(driver_names);
                                    m.set_session_track_id(track_id);
                                    m.set_session_subsession_id(subsession_id);
                                    m.set_qualy_session_num(qualy_num);
                                    m.set_car_numbers(&numbers);
                                    m.set_car_redline(redline);
                                    m.set_session_car_name(car_name);
                                }
                                // Captura o custid do jogador automaticamente (uma vez).
                                super::note_session_custid(&session.session_yaml);
                                // DEBUG: se a gravação de corrida está ligada, salva o YAML.
                                super::race_capture::record_session(&session.session_yaml);
                            }
                        }
                        lock().observe(&t);
                        // DEBUG: grava o frame de telemetria (subamostrado) pra calibração.
                        super::race_capture::record_frame(&t);
                        // Disparo de quebra ESTRANGULADO: 1 comando a cada ~1,5s (a ~60 Hz),
                        // FORA do lock (o send_chat_text foca a janela + SendInput; não pode
                        // segurar o lock). Espaça o roubo de foco pra o jogador seguir dirigindo.
                        if tick % 90 == 0 {
                            if let Some(cmd) = lock().take_one_breakdown_cmd() {
                                // NÃO engole o erro: se o comando não chegou ao sim (janela
                                // não encontrada / foreground recusado / SendInput bloqueado),
                                // a penalidade sumiria em silêncio. Loga e arma UM aviso âmbar
                                // no rádio (latch por corrida) pra o jogador saber que precisa
                                // rodar o sim em janela/borderless.
                                if let Err(err) = super::send_chat_text(&cmd) {
                                    if lock().note_chat_send_failure() {
                                        eprintln!(
                                            "[breakdown] comando '{cmd}' não chegou ao iRacing: {err}"
                                        );
                                    }
                                }
                            }
                        }
                        // Sim conectado de novo: cancela qualquer janela de foco pendente.
                        clear_focus_self();
                        // Telemetria: ping de vida da corrida aberta (30 min). Sai
                        // FORA do lock e é ~grátis quando não há corrida rolando.
                        crate::telemetry::maybe_ping();
                        true
                    }
                    Err(error) => {
                        let mut m = lock();
                        // Sim fechado com tentativa ativa = DNF.
                        let sim_closed = matches!(error, super::IracingError::NotRunning(_));
                        if sim_closed && m.was_connected {
                            let active = m
                                .attempts
                                .last()
                                .map(|a| a.status == "active")
                                .unwrap_or(false);
                            if active {
                                m.pending_event = m.finalize_attempt("sim_closed");
                            }
                            m.was_connected = false;
                            m.prev = None;
                            m.reset_qualy_state();
                            // Telemetria: sim fechado fecha a corrida aberta. No-op
                            // se não havia nenhuma (finalize_attempt acima já pode
                            // ter fechado). Sem isso a corrida viraria fantasma e
                            // só sumiria do contador na expiração de 35 min.
                            // Sem desfecho: a conexão caiu, então não há posição final
                            // nem volta confiável pra reportar.
                            crate::telemetry::race_end("sim_closed", None);
                            // Borda de descida: arma a janela de foco da nossa janela.
                            arm_focus_self();
                        }
                        m.connected = false;
                        false
                    }
                }
            }))
            .unwrap_or_else(|_| {
                eprintln!(
                    "[race_monitor] sampler: panic num tick (recuperado, sampler segue vivo)"
                );
                false
            });
            tick = tick.wrapping_add(1);
            let period = if connected {
                SAMPLER_PERIOD_MS
            } else {
                SAMPLER_IDLE_PERIOD_MS
            };
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

/// DEBUG: arma uma quebra garantida no carro do jogador pra próxima volta cruzada (testa o
/// disparo ao vivo na pista). `true` = armado (jogador em sessão + número conhecido).
pub fn arm_test_breakdown() -> bool {
    start_sampler();
    lock().arm_test_breakdown()
}

/// DEBUG: pede armar a GRADE TODA com uma peça perto de quebrar por carro (montada no próximo
/// tick, correndo). As quebras pingam ao longo das voltas, estranguladas pra não spammar.
pub fn arm_test_breakdown_grid() {
    start_sampler();
    lock().request_arm_grid();
}

/// PRODUÇÃO: instala o diretor de quebra da corrida montado com o DESGASTE REAL de cada time
/// (chamado pelo export, que tem DB). `player_live` = estado do jogador (vinculado ao número
/// dele no verde). `weather` = clima fixo da corrida. Prende os carros na volta atual no
/// primeiro tick verde e passa a disparar `!black`/`!dq` conforme cada carro cruza.
pub fn install_breakdown_director(
    dir: crate::car::breakdown::BreakdownDirector,
    player_live: Option<crate::car::breakdown::LiveBreakdown>,
    weather: crate::car::breakdown::Weather,
) {
    start_sampler();
    lock().install_breakdown_director(dir, player_live, weather);
}

/// Alertas de quebra ativos por car_idx, pro overlay: `(car_idx, kind)` com
/// kind ∈ "light" | "heavy" | "dnf". Vazio quando não há quebra em andamento.
pub fn get_breakdown_alerts() -> Vec<(i32, &'static str)> {
    lock().breakdown_alerts_snapshot()
}

/// Paradas de REPARO de peça: `(car_idx, volta de entrada no box)`. O overlay marca o ícone
/// de "peça" no lugar do pneu na parada daquela volta.
pub fn get_breakdown_repair_laps() -> Vec<(i32, u32)> {
    lock().breakdown_repair_laps.clone()
}

/// Espia (sem drenar) o log de quebras da corrida em andamento — pro overlay do RÁDIO DA
/// EQUIPE mostrar cada quebra ao vivo. O drain de verdade (→ tabela/debrief) só acontece no
/// import; aqui é leitura pura, acumulativa durante a corrida.
pub fn peek_breakdown_log() -> Vec<BreakdownOutcome> {
    lock().breakdown_log.clone()
}

/// Espia (sem drenar) os AVISOS pessoais do jogador (peça entrou na zona de risco) — pro
/// overlay do rádio mostrar num card DISTINTO. Leitura pura, acumulativa durante a corrida.
pub fn peek_player_warnings() -> Vec<PlayerWarning> {
    lock().player_warning_log.clone()
}

/// `true` se algum comando de quebra falhou em chegar ao iRacing nesta corrida (janela não
/// encontrada / foreground recusado / `SendInput` bloqueado). O overlay do rádio transforma
/// isso num aviso âmbar único orientando o jogador a rodar o sim em janela/borderless.
pub fn chat_send_blocked() -> bool {
    lock().chat_send_warned
}

/// car_idx que devem PISCAR na torre agora (quebraram nos últimos 5 s) — sincroniza o flash
/// da linha do piloto com o anúncio do rádio do engenheiro.
pub fn get_breakdown_flashes() -> Vec<i32> {
    lock().breakdown_flash_idxs()
}

/// PEÇA 3: DRENA o log estruturado de desfechos de quebra da corrida (esvazia). Chamado UMA vez
/// no import (`build_session_race_result`) → resolve car_number→driver_id e persiste na
/// `race_breakdowns`. `std::mem::take` garante que cada desfecho seja importado uma só vez.
pub fn drain_breakdown_log() -> Vec<BreakdownOutcome> {
    start_sampler();
    std::mem::take(&mut lock().breakdown_log)
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
        crash_progress_score: if m.in_crash {
            m.crash_components.total()
        } else {
            0.0
        },
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

/// Marcadores de incidente do JOGADOR (pontos do próprio iRacing + volta). Moram no
/// monitor, não no `RaceHistory`, daí o acessor próprio — é o ÚNICO sinal de batida de
/// quem TERMINOU a corrida, já que o resultado oficial zera os incidentes.
pub fn get_player_incidents() -> Vec<PlayerIncidentMark> {
    start_sampler();
    lock().player_incidents.clone()
}

/// Lê as voltas de qualify capturadas ao vivo, sem misturá-las ao histórico da corrida.
pub fn get_qualy_laps() -> Vec<CarLap> {
    start_sampler();
    lock().qualy_laps_snapshot()
}

/// Lê a identidade única do evento atualmente observado pelo monitor.
pub fn get_subsession_id() -> i64 {
    start_sampler();
    lock().session_subsession_id
}

pub fn get_feedback() -> RaceFeedback {
    start_sampler();
    let m = lock();
    // Identidade "ao vivo": todo carro que o YAML da sessão conhece (tem nome de
    // piloto ou número). Independe de tentativa ativa / não-quali.
    let named: std::collections::HashSet<i32> =
        m.driver_names.iter().map(|(idx, _)| *idx).collect();
    let cars_yaml_meta = (0..64i32)
        .filter(|&i| named.contains(&i) || m.car_number[i as usize] > 0)
        .map(|i| YamlCarMeta {
            idx: i,
            is_ai: m.car_is_ai[i as usize],
            is_pace: m.car_is_pace[i as usize],
            class_id: m.car_class_id[i as usize],
            car_number: m.car_number[i as usize],
        })
        .collect();
    RaceFeedback {
        laps: m.history.laps.clone(),
        player_laps: m.history.player_laps.clone(),
        player_track: m.history.player_track.clone(),
        yellow_laps: m.history.yellow_laps.clone(),
        cars_meta: m.history.cars_meta.clone(),
        cars_yaml_meta,
        player_car_idx: m.history.player_car_idx,
        class_names: m.class_names.iter().cloned().collect(),
        driver_names: m.driver_names.iter().cloned().collect(),
        player_pit_laps: m.player_pit_laps.clone(),
        car_laps: m.history.car_laps.clone(),
        player_incidents: m.player_incidents.clone(),
    }
}

/// Zera o monitor para começar um novo teste.
pub fn reset() {
    *lock() = RaceMonitor::new();
}

#[cfg(test)]
mod tests;
