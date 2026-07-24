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

mod pontuacao;
mod quebras;
mod resultado;
mod sessao;
mod tentativas;
mod tipos;

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

/// Passo puro da máquina de apagar o alerta de PENALIDADE: dado o "já entrou no box desde a
/// quebra", o "está no box agora" e o "estava no box no tick anterior", devolve
/// `(novo_entered_since, apagar)`. Apaga quando o carro SAI do box (true→false) já tendo
/// entrado desde a quebra = serviu a penalidade / reparou. (DNF não passa por aqui — é fixo.)
fn pit_clear_step(entered_since: bool, on_pit: bool, prev_on_pit: bool) -> (bool, bool) {
    let entered = entered_since || on_pit;
    let clear = prev_on_pit && !on_pit && entered;
    (entered, clear)
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

    /// Monta o diagnóstico por carro (o que o RaceControl enxerga de cada um).
    fn build_cars_debug(&mut self, t: &IracingTelemetry) {
        let now = t.session_time;
        let flags = t.session_flags as u32;
        self.live_is_green =
            t.session_state == STATE_RACING && flags & (FLAG_CAUTION | FLAG_CAUTION_WAVING) == 0;
        let ref_pace = self
            .car_monitors
            .iter()
            .map(|cm| cm.pace)
            .fold(0.0_f64, f64::max);

        let mut out = Vec::with_capacity(t.cars.len());
        for car in &t.cars {
            let valid = car.idx >= 0 && (car.idx as usize) < 64;
            let cm = if valid {
                self.car_monitors[car.idx as usize]
            } else {
                CarMonitor::DEFAULT
            };
            let stalled = if cm.has_moved {
                now - cm.last_move_time
            } else {
                0.0
            };
            let pace_pct = if ref_pace > 0.0 {
                (cm.pace / ref_pace * 100.0).clamp(0.0, 999.0)
            } else {
                0.0
            };
            let on_racing =
                car.track_surface == SURFACE_ON_TRACK || car.track_surface == SURFACE_OFF_TRACK;
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

    /// Carro elegível para as regras de IA: é IA e NÃO é pace car.
    fn is_monitorable_ai(&self, idx: i32) -> bool {
        idx >= 0
            && (idx as usize) < 64
            && self.car_is_ai[idx as usize]
            && !self.car_is_pace[idx as usize]
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

    // ── Loop principal ───────────────────────────────────────────────────────
    /// Grava o histórico volta a volta da tentativa ativa: a cada volta do líder
    /// um snapshot de gaps/posições, o tempo de cada volta do jogador e as voltas
    /// em que a amarela esteve ativa. Reinicia quando começa uma nova tentativa.
    /// Captura, a cada tick ATIVO, o contexto de clima da corrida e as PARADAS de box
    /// de todos os carros (entrada/saída do `InPitStall` + dwell + pista molhada no
    /// instante). Alimenta a inferência de estratégia de pneu (`tire_strategy`).
    fn capture_tire_strategy(&mut self, t: &IracingTelemetry, now: f64) {
        let wet = t.track_is_wet();

        // ── Contexto de clima ──
        // Largada: capturada uma vez quando a corrida já está em "Racing".
        if !self.weather_start_captured && t.session_state == STATE_RACING {
            self.history.weather.wet_at_start = wet;
            self.weather_start_captured = true;
        }
        if wet {
            self.history.weather.ever_wet = true;
        }
        // wet_at_finish = wetness do último tick ativo (record_history só roda enquanto
        // a tentativa está ativa; o último valor escrito ≈ a condição na bandeirada).
        if t.session_state == STATE_RACING {
            self.history.weather.wet_at_finish = wet;
        }

        // ── Paradas de box (todos os carros, menos o pace car) ──
        // Só rastreia durante a corrida VERDE: a espera no box antes da largada
        // (grid) ou numa classificatória não pode virar uma parada. Fora do verde,
        // zera o rastreio pra uma caixa pré-largada não abrir uma "entrada" que
        // fecharia (com dwell enorme) no instante da largada.
        if t.session_state < STATE_RACING {
            self.pit_in_stall = [false; 64];
            return;
        }
        for car in &t.cars {
            let i = car.idx as usize;
            if i >= 64 || self.car_is_pace[i] {
                continue;
            }
            let in_stall = car.track_surface == SURFACE_IN_PIT_STALL;
            if in_stall && !self.pit_in_stall[i] {
                // Entrou na caixa → abre o cronômetro de dwell.
                self.pit_in_stall[i] = true;
                self.pit_stall_enter_time[i] = now;
                self.pit_stall_enter_lap[i] = car.lap.max(car.lap_completed + 1).max(1);
                self.pit_stall_wet[i] = wet;
            } else if !in_stall && self.pit_in_stall[i] {
                // Saiu da caixa → fecha a parada.
                self.pit_in_stall[i] = false;
                let dwell = (now - self.pit_stall_enter_time[i]).max(0.0);
                // Ignora blips transientes de `InPitStall` (dwell ~0s): o carro
                // apenas cruzou a zona da caixa, não parou. Só conta parada real.
                if dwell < MIN_PIT_STALL_DWELL_SECS {
                    continue;
                }
                self.history.pit_stops.push(super::tire_strategy::PitStop {
                    car_idx: car.idx,
                    lap: self.pit_stall_enter_lap[i],
                    stationary_secs: dwell,
                    track_wet_at_stop: self.pit_stall_wet[i],
                });
                if self.history.pit_stops.len() > MAX_PIT_STOPS {
                    self.history.pit_stops.remove(0);
                }
            }
        }
    }

    /// Cronometra os 3 setores do jogador dividindo `lap_dist_pct` em terços. Só na
    /// corrida verde e com o carro na pista; sair da pista ou pular setor (teleporte/
    /// reset) invalida o parcial em andamento pra não gravar tempo-lixo.
    fn capture_player_sectors(&mut self, t: &IracingTelemetry) {
        if t.session_state < STATE_RACING || t.track_surface != SURFACE_ON_TRACK {
            self.sec_prev = -1;
            return;
        }
        let pct = t.lap_dist_pct;
        if !(0.0..=1.0).contains(&pct) {
            return;
        }
        let sec = if pct < 1.0 / 3.0 {
            0
        } else if pct < 2.0 / 3.0 {
            1
        } else {
            2
        };
        let now = t.session_time;
        if self.sec_prev < 0 {
            // Primeira base: começa a cronometrar, mas ESTE setor é parcial (entramos
            // no meio dele) → não grava ao fechá-lo.
            self.sec_prev = sec;
            self.sec_enter_time = now;
            self.sec_clean = false;
            return;
        }
        if sec == self.sec_prev {
            return;
        }
        let expected = (self.sec_prev + 1) % 3;
        if sec == expected {
            if self.sec_clean {
                let dur = now - self.sec_enter_time;
                if dur > 0.0 && dur < 600.0 {
                    // O S3 fecha na linha (2→0) → pertence à volta recém-completada.
                    let lap = if self.sec_prev == 2 {
                        (t.lap - 1).max(1)
                    } else {
                        t.lap.max(1)
                    };
                    self.history.player_sectors.push(SectorSplit {
                        lap,
                        sector: self.sec_prev + 1,
                        time: dur,
                    });
                    if self.history.player_sectors.len() > MAX_HISTORY_LAPS {
                        self.history.player_sectors.remove(0);
                    }
                }
            }
            self.sec_prev = sec;
            self.sec_enter_time = now;
            self.sec_clean = true;
        } else {
            // Pulou setor (teleporte/reset/volta anulada) → invalida o parcial.
            self.sec_prev = sec;
            self.sec_enter_time = now;
            self.sec_clean = false;
        }
    }

    fn record_history(&mut self, t: &IracingTelemetry) {
        let now = t.session_time;

        // A CLASSIFICATÓRIA nunca entra no histórico da CORRIDA. Ela roda na mesma
        // conexão de telemetria (com outro `session_num`) e, sem este gate, suas
        // voltas + o retorno ao box no fim viravam voltas/paradas "da corrida" no
        // pós-corrida. As voltas de quali que importam (grid) já são capturadas à
        // parte em `capture_qualy` → `qualy_laps`.
        if self.qualy_session_num >= 0 && t.session_num == self.qualy_session_num {
            return;
        }

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
        // Tentativa nova OU sessão de telemetria nova (quali/treino → corrida) →
        // começa um histórico limpo. O gate por sessão é a rede de segurança caso
        // a quali não tenha sido identificada a tempo pelo YAML.
        let subsession_changed = self.session_subsession_id > 0
            && self.history.subsession_id != self.session_subsession_id;
        if self.history.attempt_number != attempt
            || self.hist_session_num != t.session_num
            || subsession_changed
        {
            self.history = RaceHistory::empty();
            self.history.attempt_number = attempt;
            self.history.subsession_id = self.session_subsession_id;
            self.hist_session_num = t.session_num;
            self.hist_leader_lap = 0;
            self.hist_player_lap = 0;
            self.hist_car_lap_completed = [0; 64];
            self.hist_car_last_pos = [0; 64];
            self.hist_trace_pos = [0; 64];
            self.hist_last_trace_event_time = 0.0;
            self.hist_last_neighbor_time = 0.0;
            // A quali precede a corrida → carrega as voltas dela no histórico (uma vez).
            self.history.qualy_laps = self.qualy_laps.clone();
            // Reseta o detector de pit / clima / setor para a tentativa nova.
            self.pit_in_stall = [false; 64];
            self.weather_start_captured = false;
            self.sec_prev = -1;
            self.sec_enter_time = 0.0;
            self.sec_clean = false;
        }
        self.history.player_car_idx = t.player_car_idx;

        // Clima da corrida + paradas de box (estratégia de pneu de todos os carros).
        self.capture_tire_strategy(t, now);
        self.capture_player_sectors(t);

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

        // Guarda a última posição VÁLIDA de cada carro a cada tick. `CarIdxPosition`
        // pisca 0 quando o carro está no box/entre estados; sem isso, um único tick
        // ruim no instante do snapshot faria o carro sumir daquela volta do trace.
        for c in &t.cars {
            let i = c.idx;
            if i >= 0 && (i as usize) < 64 && c.position >= 1 {
                self.hist_car_last_pos[i as usize] = c.position;
            }
        }

        // Snapshot do trace: na VIRADA de volta do líder (âncora, garante 1 ponto por
        // volta) OU quando QUALQUER posição muda — aí a ultrapassagem entra no gráfico
        // NA HORA, não só quando o líder fecha a volta. O X vira fracionário
        // (volta do líder + progresso dele na volta).
        let new_leader_lap = leader_lap > self.hist_leader_lap && leader_lap >= 1;
        let positions_changed = t.session_state >= STATE_RACING
            && leader_lap >= 1
            && now - self.hist_last_trace_event_time >= MIN_TRACE_EVENT_GAP_SECS
            && t.cars.iter().any(|c| {
                let i = c.idx;
                i >= 0
                    && (i as usize) < 64
                    && c.position >= 1
                    && c.position != self.hist_trace_pos[i as usize]
            });
        if new_leader_lap || positions_changed {
            if new_leader_lap {
                self.hist_leader_lap = leader_lap;
            }
            if positions_changed {
                self.hist_last_trace_event_time = now;
            }
            // Progresso do líder DENTRO da volta (0..1) → parte fracionária do X.
            let leader_progress = t
                .cars
                .iter()
                .find(|c| c.position == 1)
                .map(|c| c.lap_dist_pct.clamp(0.0, 1.0))
                .unwrap_or(0.0);
            // Tempo de volta de referência do líder — usado pra "empilhar" as voltas
            // atrás no gap de quem está lapado/parado. Sem isso, um carro parado no box
            // (F2Time ~0, ver CarSnapshot::f2_time) fica colado no líder no gráfico.
            // Preferência: volta do líder → melhor volta do líder → volta mais rápida
            // do campo → fallback 90s (só relevante nos primeiros instantes).
            let leader_lap_ref = t
                .cars
                .iter()
                .find(|c| c.position == 1)
                .map(|c| if c.last_lap_time > 0.0 { c.last_lap_time } else { c.best_lap_time })
                .filter(|&v| v > 0.0)
                .or_else(|| {
                    t.cars
                        .iter()
                        .map(|c| c.last_lap_time)
                        .filter(|&v| v > 0.0)
                        .fold(None, |acc: Option<f64>, v| Some(acc.map_or(v, |a| a.min(v))))
                })
                .unwrap_or(90.0);
            let cars: Vec<CarGapPoint> = t
                .cars
                .iter()
                .filter_map(|c| {
                    let i = c.idx;
                    if i < 0 || i as usize >= 64 {
                        return None;
                    }
                    // Posição atual (≥1), ou a última válida conhecida (carro num
                    // blip de box). Sem nenhuma → pace car / nunca classificado.
                    let position = if c.position >= 1 {
                        c.position
                    } else {
                        self.hist_car_last_pos[i as usize]
                    };
                    if position < 1 {
                        return None;
                    }
                    // Gap ao líder: F2Time do SDK, mas com um PISO por voltas atrás.
                    // Um carro lapado (ou parado no box, sem volta completa) vem com
                    // F2Time ~0 e ficaria colado no líder; o piso `voltas_atrás × tempo
                    // de volta do líder` o empurra pro fim. O `max` preserva um F2Time
                    // já válido (que em corrida real JÁ inclui o tempo lapado) sem
                    // duplicar a contagem.
                    let base_gap = if c.f2_time.is_finite() {
                        c.f2_time.max(0.0)
                    } else {
                        0.0
                    };
                    let laps_behind = (leader_lap - c.lap_completed).max(0);
                    Some(CarGapPoint {
                        idx: c.idx,
                        position,
                        gap: base_gap.max(laps_behind as f64 * leader_lap_ref),
                        lap_dist_pct: if c.lap_dist_pct.is_finite() {
                            c.lap_dist_pct.clamp(0.0, 1.0) as f32
                        } else {
                            0.0
                        },
                        est_time: if c.est_time.is_finite() {
                            c.est_time.max(0.0) as f32
                        } else {
                            0.0
                        },
                    })
                })
                .collect();
            // Memoriza as posições gravadas pra detectar a PRÓXIMA troca.
            for c in &t.cars {
                let i = c.idx;
                if i >= 0 && (i as usize) < 64 && c.position >= 1 {
                    self.hist_trace_pos[i as usize] = c.position;
                }
            }
            self.history.laps.push(LapSnapshot {
                lap: leader_lap,
                progress: leader_progress as f32,
                cars,
            });
            if self.history.laps.len() > MAX_HISTORY_LAPS {
                self.history.laps.remove(0);
            }
        }

        // Pit do jogador: acumula se passou pelo pit road na volta em andamento.
        if t.player_on_pit_road {
            self.player_pit_seen = true;
        }

        // Jogador completou uma volta nova → registra o tempo dela.
        if t.lap_completed > self.hist_player_lap {
            self.hist_player_lap = t.lap_completed;
            if t.last_lap_time > 0.0 {
                self.history.player_laps.push(PlayerLap {
                    lap: t.lap_completed,
                    time: t.last_lap_time,
                    // Combustível restante ao fechar a volta (litros); consumo/volta
                    // sai da diferença entre voltas. <0 no SDK = ignora.
                    fuel_remaining: if t.fuel_level >= 0.0 { t.fuel_level } else { -1.0 },
                });
                if self.history.player_laps.len() > MAX_HISTORY_LAPS {
                    self.history.player_laps.remove(0);
                }
            }
            // A volta recém-completada teve pit? Marca e zera para a próxima.
            if self.player_pit_seen {
                self.player_pit_laps.push(t.lap_completed);
                if self.player_pit_laps.len() > MAX_HISTORY_LAPS {
                    self.player_pit_laps.remove(0);
                }
            }
            self.player_pit_seen = false;
        }

        // Cada carro (jogador + IA) completou uma volta → registra o tempo dela.
        // Base da adaptação (ritmo da frente da classe). Evento por carro, barato.
        for car in &t.cars {
            let i = car.idx;
            if i < 0 || i as usize >= 64 || self.car_is_pace[i as usize] {
                continue;
            }
            if car.lap_completed > self.hist_car_lap_completed[i as usize] {
                self.hist_car_lap_completed[i as usize] = car.lap_completed;
                if car.last_lap_time > 0.0 && car.lap_completed >= 1 {
                    self.history.car_laps.push(CarLap {
                        car_idx: i,
                        lap: car.lap_completed,
                        time: car.last_lap_time,
                    });
                    if self.history.car_laps.len() > MAX_CAR_LAPS {
                        self.history.car_laps.remove(0);
                    }
                }
            }
        }

        // Resumo por carro (classe, IA, posição na classe) — ACUMULADO por idx
        // (upsert), nunca encolhe. Um carro que sai do mundo (DNF, ou o cooldown
        // pós-bandeira em que todos voltam ao menu) MANTÉM a última amostra; assim
        // as posições finais não são apagadas no fim da corrida. (Antes era um
        // replace total, que zerava cars_meta quando o campo esvaziava no cooldown.)
        for c in t.cars.iter().filter(|c| c.idx >= 0 && (c.idx as usize) < 64) {
            let i = c.idx as usize;
            // Grid ROBUSTO: além do snapshot exato do verde, fixa a PRIMEIRA posição
            // na classe já observada (set-once). Se o monitor só começou a amostrar
            // depois da largada (transição do verde perdida), o grid ainda é a
            // posição mais antiga vista — bem melhor que vazio.
            if self.grid_class_pos[i] == 0 && c.class_position >= 1 {
                self.grid_class_pos[i] = c.class_position;
            }
            let meta = CarMeta {
                idx: c.idx,
                is_ai: self.car_is_ai[i],
                is_pace: self.car_is_pace[i],
                class_id: self.car_class_id[i],
                class_position: c.class_position,
                car_number: self.car_number[i],
                grid_class_position: self.grid_class_pos[i],
            };
            match self.history.cars_meta.iter_mut().find(|m| m.idx == c.idx) {
                // Preserva o grid já capturado se este tick ainda não o tem.
                Some(existing) => {
                    let grid = if meta.grid_class_position > 0 {
                        meta.grid_class_position
                    } else {
                        existing.grid_class_position
                    };
                    *existing = CarMeta {
                        grid_class_position: grid,
                        ..meta
                    };
                }
                None => self.history.cars_meta.push(meta),
            }
        }
        self.history.track_id = self.session_track_id;

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
                        if d.is_finite() {
                            d
                        } else {
                            0.0
                        }
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
        if self.prev_session_state < STATE_RACING
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
