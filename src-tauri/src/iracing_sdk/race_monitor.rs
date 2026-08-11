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

mod amostrador;
mod api;
mod controle_corrida;
mod estado_agora;
mod historico;
mod observacao;
mod pontuacao;
mod quali;
mod quebras;
mod resultado;
mod sessao;
mod tentativas;
mod tipos;
mod voltas;

pub use amostrador::*;
pub use api::*;
pub use estado_agora::*;
pub(crate) use observacao::*;
pub(crate) use pontuacao::*;
pub use resultado::*;
use sessao::*;
pub use tipos::*;

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
const FLAG_WHITE: u32 = 0x0000_0002;
const FLAG_YELLOW: u32 = 0x0000_0008;
const FLAG_RED: u32 = 0x0000_0010;
const FLAG_BLUE: u32 = 0x0000_0020;
const FLAG_YELLOW_WAVING: u32 = 0x0000_0100;
const FLAG_CAUTION: u32 = 0x0000_4000;
const FLAG_CAUTION_WAVING: u32 = 0x0000_8000;
const FLAG_BLACK: u32 = 0x0001_0000;
const FLAG_DISQUALIFY: u32 = 0x0002_0000;
const FLAG_REPAIR: u32 = 0x0010_0000;

/// De quanto em quanto tempo o [`EstadoAgora`] recolhe uma amostra. O sampler roda a 60 Hz
/// para o spotter e a pontuação de batida; o estado narrado não precisa disso — a fala mais
/// curta do engenheiro leva meio segundo, e clonar o vetor de carros 60 vezes por segundo
/// pagaria caro por uma resolução que ninguém consome.
const ESTADO_REFRESH_SECS: f64 = 0.25;

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

/// Aumento (s) em `PitRepairLeft + PitOptRepairLeft` que conta como dano NOVO. Só
/// separa ruído de ponto flutuante do que o sim de fato acrescentou.
const REPAIR_JUMP_SECS: f64 = 0.05;

// ─── Carro destruído na CLASSIFICAÇÃO ────────────────────────────────────────
// O iRacing devolve o carro inteiro para a corrida; a consequência é regra NOSSA, imposta
// por comando de admin, como já é o Sistema de Quebra.
//
// O castigo age em DOIS momentos. NA QUALI, ao vivo: batida "grave"+ manda `!dq` na hora —
// o jogador fica impedido de seguir classificando, e o rádio diz o porquê no instante em
// que o fim de semana dele muda. NA CORRIDA: "grave" vira largada do fundo (`!clear` +
// `!eol`, medido funcionando em 2026-08-10); "destruído"+ vira DQ reafirmado (`!dq`) — o
// carro não corre. "Catastrófico" só muda a fala do rádio ("você está inteiro?").
//
// A SEVERIDADE da batida gradua (G + velocidade perdida, impacto confirmado). O MEATBALL
// (`FLAG_REPAIR`) é piso de "grave": é o sim declarando reparo obrigatório, e cobre o
// score ficar curto em pista molhada/G subamostrado. Os canais `PitRepairLeft` são MUDOS
// fora do box (medido: carro destruído, meatball na tela, 0.0 em todos os frames — e na
// quali o botão de box conserta o carro, então eles nunca falam lá); quando falarem,
// também são piso. Rodada sem impacto não castiga nunca.
/// Severidade a partir da qual a quali TRAVA e a corrida sai do fundo.
const QUALI_WRECK_PENALTY_SEV: &str = "grave";
/// Severidade a partir da qual o carro não corre (DQ na quali E na corrida).
const QUALI_WRECK_DQ_SEV: &str = "destruído";
/// Reparo obrigatório (s) que também basta para o piso de "grave". Corroboração.
const QUALI_WRECK_PENALTY_S: f64 = 25.0;
/// Reparo obrigatório (s) que também basta para o piso de "destruído".
const QUALI_WRECK_DQ_S: f64 = 60.0;
/// Penalidade (s) da bandeira preta quando o `!eol` perde a janela da formação. O castigo
/// não pode evaporar só porque o YAML demorou a entregar o número do carro.
const QUALI_WRECK_FALLBACK_PENALTY_S: u32 = 15;

/// Variável de ambiente que arma a regra do carro destruído na classificação.
const QUALI_WRECK_ENV: &str = "IRACER_QUALI_WRECK";

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
    /// Segundos de reparo (obrigatório + opcional) que o sim reportava no tick anterior.
    /// O SALTO desse valor é o próprio iRacing dizendo "o carro quebrou agora", e serve de
    /// segunda confirmação do impacto. -1 = ainda sem leitura nesta tentativa.
    prev_repair_needed_s: f64,
    /// Número da tentativa que cobriu a CLASSIFICAÇÃO deste fim de semana. 0 = não houve (ou
    /// ainda não fechou). Serve para o import cobrar o conserto da batida da quali com a
    /// MESMA régua da corrida (`player_worst_severity` sobre esta tentativa).
    quali_attempt_number: i32,
    /// Castigo por carro destruído na classificação, decidido na virada para a corrida e
    /// pendente de envio: `"eol"` (larga do fundo) ou `"dq"` (não larga). Fica latch porque o
    /// comando precisa do NÚMERO do carro, que só é conhecido depois que a sessão de corrida
    /// popula o YAML — mandar no mesmo tick da virada acertaria o vazio.
    quali_wreck_pending: Option<&'static str>,
    /// O lockout AO VIVO da quali já saiu (`!dq` no meio da classificação). Latch por fim de
    /// semana: o comando é um só, e é ele que decide se o despacho da corrida precisa de um
    /// `!clear` antes do `!eol`.
    quali_lockout_sent: bool,
    /// Quantas falas de rádio já foram DESCARTADAS por reinícios/corridas anteriores.
    ///
    /// O overlay mostra "a mais nova por id" e ignora id menor ou igual ao último visto. O id
    /// é a posição no log — e os logs são esvaziados a cada tentativa nova. Sem esta base, o
    /// primeiro reinício fazia os ids voltarem a zero e o rádio emudecia PARA SEMPRE naquela
    /// sessão: as falas chegavam ao overlay e eram descartadas por "já vi essa", sem erro em
    /// lugar nenhum. Medido em 2026-08-10, com o jogador reiniciando a quali várias vezes.
    ///
    /// Uma base só para todos os canais: ela nunca decresce, então cada canal continua
    /// monotônico, e o desperdício de espaço de id é irrelevante.
    radio_epoch: usize,
    /// A regra do carro destruído na classificação está armada? `None` = ainda não resolvida
    /// (lida de [`QUALI_WRECK_ENV`] no primeiro fim de semana).
    ///
    /// Fica atrás de flag porque o comando da faixa do meio (`!eol`, "larga do fundo") ainda
    /// não foi confirmado numa etapa de verdade: se ele não pegar em sessão de IA, o castigo
    /// some em silêncio, e é melhor isso acontecer num teste do que na carreira de quem joga.
    quali_wreck_on: Option<bool>,

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

    // Estado narrado (ver `estado_agora`)
    /// Telemetria completa mais recente, refrescada a `ESTADO_REFRESH_SECS`. É a fonte do
    /// [`EstadoAgora`] — guardada inteira porque o retrato precisa do vetor de carros, e
    /// espelhar duas dúzias de escalares aqui só criaria uma segunda verdade para divergir.
    ultima_telemetria: Option<IracingTelemetry>,
    /// `session_time` do último refresh — o estrangulamento do clone.
    estado_ultimo_refresh: f64,
    /// Amostras de gap para frente e para trás. É o que separa "ele está a oito décimos"
    /// de "ele está a oito décimos e vindo" — a segunda é a que muda o que o piloto faz.
    gap_hist: Vec<estado_agora::AmostraGap>,

    // RaceEventEngine
    events: Vec<RaceEvent>,
    prev_session_state: i32,
    /// `SessionNum` do tick anterior — a fronteira entre treino, classificação e corrida
    /// dentro da MESMA conexão. Trocou de sessão ⇒ a tentativa anterior fecha, para que a
    /// batida de um treino não conte como dano da corrida. -1 = ainda sem sessão vista.
    prev_session_num: i32,
    prev_on_pit_road: bool,
    prev_caution: bool,
    race_started_emitted: bool,
    race_finished_emitted: bool,
    /// Voltas completas do líder no instante EXATO da bandeirada. 0 = a bandeirada ainda
    /// não caiu (ou o monitor só começou a olhar depois dela).
    ///
    /// Existe porque `CarIdxLapCompleted` não para no fim da prova: quem continua girando
    /// na volta de desaceleração fecha mais uma volta, e o maior valor da grade sobe
    /// depois de a corrida ter acabado. Em prova por VOLTAS o total da prova segura o
    /// número no cabeçalho da torre; em prova por TEMPO não existe total previsto, e este
    /// congelamento é o único teto possível. Por isso é capturado na BORDA de entrada em
    /// `STATE_CHECKERED`, no sampler a 60 Hz: um frame depois o valor já pode ter subido.
    volta_final_lider: i32,
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
    /// Reinícios da sessão de CORRIDA neste fim de semana, contados na borda em que o
    /// `restarted()` fecha a tentativa.
    ///
    /// Existe porque o número que ia para a telemetria de produto era
    /// `attempt_number - 1`, e a tentativa é criada a cada troca de sessão: um fim de
    /// semana normal (treino → quali → corrida, zero reinícios) reportava dois
    /// "restarts". Ver `build_race_outcome`.
    restarts_corrida: i32,
    /// Reinícios da sessão de CLASSIFICAÇÃO. Contados à parte porque respondem outra
    /// pergunta: refazer a quali é o jogador caçando uma volta boa, refazer a corrida é
    /// ele fugindo de um resultado.
    restarts_quali: i32,
    /// Carro do jogador nesta sessão (`CarScreenName`). Só telemetria de produto: sem
    /// ele, o tempo de volta não é comparável — 1:35 é rápido num carro e lento noutro.
    session_car_name: Option<String>,
    /// `SessionNum` da sessão de qualify (-1 se não houver) — detecta a quali.
    qualy_session_num: i32,
    /// `SessionNum` da sessão de CORRIDA (-1 se não houver) — o gate do snapshot de
    /// grid. Treino e quali também chegam a `SessionState = Racing`, então sem ele a
    /// primeira sessão do fim de semana roubava a captura do grid.
    race_session_num: i32,
    /// Se estávamos em quali no tick anterior (detecta entrada numa quali nova).
    prev_in_qualy: bool,
    /// Voltas capturadas na sessão de quali (carregadas no histórico da corrida).
    qualy_laps: Vec<CarLap>,
    /// Ciclo de vida da volta na QUALI, por carro. Separado do da corrida porque os dois
    /// rodam em gates diferentes e são zerados em momentos diferentes.
    voltas_quali: [voltas::ColetorDeVoltas; 64],
    /// Melhor volta VÁLIDA da quali por carro (segundos; 0 = nenhuma), travada do
    /// `CarIdxBestLapTime` a cada tick.
    ///
    /// Existe por causa de duas coisas que só valem na classificatória. A primeira: o
    /// `CarIdxBestLapTime` é o único canal que já vem com a volta anulada por limite de
    /// pista descartada — `qualy_laps` guarda o `CarIdxLastLapTime` cru, que registra a
    /// volta cortada como se valesse. A segunda: carro na garagem SAI de `cars`, e com
    /// ele sairia o tempo. Travando o valor enquanto o carro está no mundo, a melhor
    /// volta válida sobrevive à ida ao box.
    qualy_best_valid: [f64; 64],
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
    /// Ciclo de vida da volta do JOGADOR: quando ela abre, quando fecha e com que tempo entra
    /// no histórico. Ver [`voltas`] — ler `LapCompleted` e `LapLastLapTime` no mesmo tique
    /// gravava cada volta com o tempo da anterior.
    voltas_jogador: voltas::ColetorDeVoltas,
    /// O mesmo, por carro, na CORRIDA (base do ritmo do campo).
    voltas_carro: [voltas::ColetorDeVoltas; 64],
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
    /// O carro já foi visto FORA da caixa nesta sessão. Todo mundo NASCE parado na
    /// caixa; sem este selo, a ida inicial pra pista fecharia uma "parada" com dwell
    /// gigante e a torre mostraria um pit-stop fantasma antes da primeira volta.
    pit_left_stall: [bool; 64],
    /// A parada em andamento conta (o carro já tinha saído da caixa antes de entrar).
    pit_stall_valid: [bool; 64],
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
    /// Cópia PRISTINA do diretor no instante da instalação, com o estado do jogador (que só é
    /// vinculado no verde) e o pedido de vitrine guardados junto. É o que a tentativa nova
    /// recebe de volta num reinício: o desgaste que as voltas da tentativa abandonada
    /// consumiram, e as peças que largaram nela, são de uma corrida que não aconteceu — e
    /// desgaste e quebra viram consequência de carreira. `None` = nenhum diretor de PRODUÇÃO
    /// instalado (arme de debug ou nada), e aí não há a que voltar.
    breakdown_base: Option<crate::car::breakdown::BreakdownDirector>,
    breakdown_player_base: Option<crate::car::breakdown::LiveBreakdown>,
    showcase_armed: bool,
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
    /// `prime_lap` de todos os carros no primeiro tick verde da CORRIDA pra não retroagir voltas
    /// passadas (nem prender o diretor nas voltas do treino/quali).
    breakdown_needs_prime: bool,
    /// O diretor atual veio de um ARME DE DEBUG (`arm_test_breakdown`/`request_arm_grid`), e não
    /// do export. Ferramenta de teste na pista: ignora o gate de sessão e a carência de largada,
    /// senão testar a quebra exigiria uma corrida de verdade e três minutos de espera.
    breakdown_debug: bool,
    /// `session_time` do primeiro tick VERDE da sessão de corrida — âncora da carência de
    /// largada (ver `BREAKDOWN_GRACE_SECS`). `None` = a corrida ainda não largou.
    breakdown_green_at: Option<f64>,
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
    /// VITRINE da PRIMEIRA corrida do save: enquanto `true`, o monitor GARANTE que o penúltimo
    /// carro (nunca o jogador) sofra uma quebra de peça GRAVE e pare pra arrumar, pra mostrar o
    /// sistema logo de cara. Ligado só quando o export detecta a 1ª corrida (temporada 1, rodada
    /// 1) e apagado assim que dispara (uma vez por install).
    showcase_pending: bool,
    /// AVISO pessoal: peças do JOGADOR que já cruzaram o limiar de risco (`RISK_OPEN`) nesta
    /// corrida, por índice em `PartType::ALL`. Rearma quando a peça sai da zona (troca/reparo).
    player_risk_warned: [bool; 11],
    /// Log dos avisos pessoais (peça do jogador entrou na zona de risco) — o overlay mostra num
    /// card DISTINTO (voz em 2ª pessoa). Zerado ao instalar um diretor novo.
    player_warning_log: Vec<PlayerWarning>,
    /// O rádio de RITMO: observador da volta mais rápida (ver `engenheiro::ritmo`) e o log do
    /// que ele decidiu dizer. Zerados a cada corrida nova, como o log de quebras.
    ritmo: crate::engenheiro::ritmo::Observador,
    ritmo_log: Vec<FalaDeRitmo>,
    /// Última volta em que o rádio de ritmo foi consultado — o observador é POR VOLTA, e o
    /// tick roda a 60 Hz.
    ritmo_ultima_volta: i32,
    /// O engenheiro na CLASSIFICAÇÃO: o observador da sessão, a curva da melhor volta e o log do
    /// que ele decidiu dizer. Ver `engenheiro::classificacao` e `engenheiro::volta_referencia`.
    classificacao: crate::engenheiro::classificacao::Observador,
    /// A fala do observador entra no log como ela sai — peças e texto. Um tipo espelho aqui só
    /// criaria duas redações da mesma coisa, que é o defeito que a família de quebra já pagou.
    classificacao_log: Vec<crate::engenheiro::classificacao::Fala>,
    volta_ref: crate::engenheiro::volta_referencia::VoltaReferencia,
    /// Bookkeeping da volta de classificação, que é o que o observador NÃO faz — ele decide o
    /// que dizer, não onde estamos.
    ///
    /// `quali_volta` é a última volta cruzada; `quali_saiu_do_box` marca que a volta em curso
    /// começou saindo do box (é a de preparação); `quali_sujou` marca que a volta em curso
    /// tocou fora da pista e por isso não serve nem de referência nem de tentativa válida.
    quali_volta: i32,
    quali_saiu_do_box: bool,
    quali_sujou: bool,
    quali_sessao: i32,
    /// Latch do conselho de POUPAR o carro (peça nossa no limite + corrida derrubando gente).
    /// Uma vez por corrida: ele pede mudança de pilotagem, e repetido vira ruído.
    poupar_avisado: bool,
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
            prev_repair_needed_s: -1.0,
            quali_attempt_number: 0,
            quali_wreck_pending: None,
            quali_lockout_sent: false,
            radio_epoch: 0,
            quali_wreck_on: None,
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
            ultima_telemetria: None,
            estado_ultimo_refresh: f64::NEG_INFINITY,
            gap_hist: Vec::new(),
            events: Vec::new(),
            prev_session_state: 0,
            prev_session_num: -1,
            prev_on_pit_road: false,
            prev_caution: false,
            race_started_emitted: false,
            race_finished_emitted: false,
            volta_final_lider: 0,
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
            player_incidents: Vec::new(),
            car_number: [0; 64],
            car_redline: None,
            session_track_id: 0,
            session_subsession_id: 0,
            restarts_corrida: 0,
            restarts_quali: 0,
            session_car_name: None,
            qualy_session_num: -1,
            race_session_num: -1,
            prev_in_qualy: false,
            qualy_laps: Vec::new(),
            voltas_quali: [voltas::ColetorDeVoltas::DEFAULT; 64],
            qualy_best_valid: [0.0; 64],
            last_collective_alert: None,
            cars_debug: Vec::new(),
            live_is_green: false,
            race_green_time: None,
            history: RaceHistory::empty(),
            hist_session_num: -1,
            hist_leader_lap: 0,
            voltas_jogador: voltas::ColetorDeVoltas::DEFAULT,
            voltas_carro: [voltas::ColetorDeVoltas::DEFAULT; 64],
            hist_car_last_pos: [0; 64],
            hist_trace_pos: [0; 64],
            hist_last_trace_event_time: 0.0,
            hist_last_neighbor_time: 0.0,
            grid_class_pos: [0; 64],
            pit_in_stall: [false; 64],
            pit_stall_enter_time: [0.0; 64],
            pit_stall_enter_lap: [0; 64],
            pit_stall_wet: [false; 64],
            pit_left_stall: [false; 64],
            pit_stall_valid: [false; 64],
            weather_start_captured: false,
            sec_prev: -1,
            sec_enter_time: 0.0,
            sec_clean: false,
            breakdown: None,
            breakdown_base: None,
            breakdown_player_base: None,
            showcase_armed: false,
            pending_breakdown_cmds: Vec::new(),
            arm_grid_pending: false,
            breakdown_weather: crate::car::breakdown::Weather::NEUTRAL,
            pending_player_live: None,
            breakdown_needs_prime: false,
            breakdown_debug: false,
            breakdown_green_at: None,
            breakdown_alert: [None; 64],
            player_risk_warned: [false; 11],
            player_warning_log: Vec::new(),
            ritmo: crate::engenheiro::ritmo::Observador::novo(),
            ritmo_log: Vec::new(),
            ritmo_ultima_volta: -1,
            classificacao: crate::engenheiro::classificacao::Observador::novo(),
            classificacao_log: Vec::new(),
            volta_ref: crate::engenheiro::volta_referencia::VoltaReferencia::novo(),
            quali_volta: -1,
            quali_saiu_do_box: false,
            quali_sujou: false,
            quali_sessao: -1,
            poupar_avisado: false,
            breakdown_prev_on_pit: [false; 64],
            breakdown_log: Vec::new(),
            breakdown_repair_laps: Vec::new(),
            breakdown_flash_at: [0.0; 64],
            breakdown_progress: 0.0,
            showcase_pending: false,
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

#[cfg(test)]
mod tests;
