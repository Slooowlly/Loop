//! Tipos que cruzam a ponte para a UI: o modelo de tentativa/batida, o status
//! ao vivo e o histórico volta a volta da corrida.

use super::*;

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
    /// PICO do score de batida visto ao vivo nesta tentativa — atualizado todo
    /// tick enquanto há batida em andamento, INDEPENDENTE de a batida "fechar".
    /// Captura o impacto mesmo se o jogador bate e sai na hora (a batida nunca
    /// fecha, então não entraria em `crashes`). Base do conserto do carro.
    #[serde(default)]
    pub peak_crash_score: f64,
    /// Número do carro que estava NO MESMO PONTO da pista quando o jogador levou
    /// a pancada de CONTATO — o provável culpado da colisão. `None` se a batida
    /// foi solo (sem carro perto). Identifica "quem bateu em mim".
    #[serde(default)]
    pub collided_with_car_number: Option<i32>,
    /// Direção do impacto no PICO da batida (front/rear/side/vertical), do sinal
    /// dominante do G-force. Base do dano por peça na batida (`car::crash`).
    #[serde(default)]
    pub peak_impact_dir: Option<String>,
    /// Estilo de pilotagem do JOGADOR acumulado ao longo da tentativa (inputs do SDK tick a
    /// tick). Vira fator de desgaste por peça (economizar → desconto; abusar → espiral). Só o
    /// jogador acumula; a IA fica no default neutro.
    #[serde(default)]
    pub style: crate::car::driving_style::StyleAccumulator,
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
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct CarGapPoint {
    pub idx: i32,
    pub position: i32,
    /// Gap ao líder em segundos (`CarIdxF2Time`); 0 para o líder.
    pub gap: f64,
    /// Progresso na volta (`CarIdxLapDistPct`, 0..1) — proximidade à prova de wrap
    /// entre carros. Default 0 em saves antigos (campo ausente).
    #[serde(default)]
    pub lap_dist_pct: f32,
    /// Tempo estimado desde a linha (`CarIdxEstTime`, s) — escala a fração de volta
    /// em segundos. Default 0 em saves antigos.
    #[serde(default)]
    pub est_time: f32,
}

/// Snapshot de todos os carros num instante do race trace. Antes só saía na virada
/// da volta do líder; agora também a cada TROCA de posição, pra mostrar a ultrapassagem
/// na hora. `lap` + `progress` dão o X fracionário (ex.: volta 6 a 40% → 6.4).
#[derive(Clone, Serialize, Deserialize)]
pub struct LapSnapshot {
    /// Voltas completas do líder no instante do snapshot (parte inteira do X).
    pub lap: i32,
    /// Progresso do líder DENTRO da volta (0..1) no instante — a parte fracionária
    /// do X. 0 nos snapshots de virada de volta.
    #[serde(default)]
    pub progress: f32,
    pub cars: Vec<CarGapPoint>,
}

/// Tempo de uma volta completa do jogador (consistência de ritmo).
#[derive(Clone, Serialize, Deserialize)]
pub struct PlayerLap {
    pub lap: i32,
    pub time: f64,
    /// Combustível restante (litros) ao COMPLETAR esta volta. A diferença entre
    /// voltas dá o consumo por volta. -1 = não capturado (dado ausente).
    #[serde(default = "neg_one")]
    pub fuel_remaining: f64,
}

fn neg_one() -> f64 {
    -1.0
}

/// Marcador de evento do JOGADOR no race trace (pin): posição = volta + fração
/// (via `lap_dist_pct`), com os pontos de incidente e se foi saída de pista.
#[derive(Clone, Serialize, Deserialize)]
pub struct PlayerIncidentMark {
    /// Volta + fração da volta (ex.: 3.42).
    pub lap_f: f64,
    /// Pontos do incidente: 0 (só saída), 1 (saída), 2 (rodada), 4 (contato).
    pub points: i32,
    /// Saída de pista no instante do evento.
    pub off_track: bool,
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

/// Uma volta completa de um carro qualquer (jogador ou IA) — base da adaptação.
#[derive(Clone, Serialize, Deserialize)]
pub struct CarLap {
    pub car_idx: i32,
    pub lap: i32,
    pub time: f64,
}

/// Resumo de um carro (classe, IA/pace, posição na classe) — para a adaptação
/// achar a referência da classe do jogador. A última amostra vale (posição final).
#[derive(Clone, Serialize, Deserialize)]
pub struct CarMeta {
    pub idx: i32,
    pub is_ai: bool,
    pub is_pace: bool,
    pub class_id: i64,
    pub class_position: i32,
    /// Número do carro (`CarNumberRaw`) — a ponte p/ nosso `driver_id` (Fase 3).
    #[serde(default)]
    pub car_number: i32,
    /// Posição na classe na LARGADA (grid). 0 = desconhecida (sem captura de verde).
    #[serde(default)]
    pub grid_class_position: i32,
}

/// Identidade do carro DIRETO do YAML da sessão — sem os gates de tentativa/quali
/// do histórico de corrida (`history.cars_meta` só enche com tentativa ativa e
/// fora da quali). É a fonte certa pra consumo AO VIVO (overlay VR): existe em
/// qualquer sessão segundos após o sampler ler o YAML.
#[derive(Clone, Serialize)]
pub struct YamlCarMeta {
    pub idx: i32,
    pub is_ai: bool,
    pub is_pace: bool,
    pub class_id: i64,
    /// Número do carro (`CarNumberRaw`) — ponte pro `driver_id` da carreira.
    pub car_number: i32,
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
    /// Voltas completas de CADA carro (jogador + IA) — base da adaptação (ritmo da
    /// frente da classe). Capturado por evento de fim de volta.
    #[serde(default)]
    pub car_laps: Vec<CarLap>,
    /// Resumo por carro (classe, IA/pace, posição na classe).
    #[serde(default)]
    pub cars_meta: Vec<CarMeta>,
    /// Pista da sessão (`WeekendInfo:TrackID`) — a corrida que foi disputada.
    #[serde(default)]
    pub track_id: i64,
    /// Identidade única do evento (`WeekendInfo:SubSessionID`).
    #[serde(default)]
    pub subsession_id: i64,
    /// Voltas da QUALI (capturadas na sessão de qualify que precede a corrida) —
    /// reforço do escudo anti-trânsito na adaptação. Vazio se não houve quali.
    #[serde(default)]
    pub qualy_laps: Vec<CarLap>,
    /// Paradas de box detectadas (todos os carros) — base da inferência de pneu.
    #[serde(default)]
    pub pit_stops: Vec<crate::iracing_sdk::tire_strategy::PitStop>,
    /// Contexto de clima da corrida (molhada na largada / em algum momento / no fim).
    #[serde(default)]
    pub weather: crate::iracing_sdk::tire_strategy::RaceWeatherContext,
    /// Parciais por setor do JOGADOR (pista dividida em 3). Base do "seu setor fraco".
    #[serde(default)]
    pub player_sectors: Vec<SectorSplit>,
}

/// Parcial de um setor da volta do jogador (pista dividida em 3 por `lap_dist_pct`).
#[derive(Clone, Serialize, Deserialize)]
pub struct SectorSplit {
    pub lap: i32,
    /// Setor 1..3.
    pub sector: i32,
    /// Tempo do setor em segundos.
    pub time: f64,
}

impl RaceHistory {
    pub(super) const fn empty() -> Self {
        Self {
            laps: Vec::new(),
            player_laps: Vec::new(),
            player_track: Vec::new(),
            yellow_laps: Vec::new(),
            player_car_idx: -1,
            attempt_number: 0,
            finished: false,
            outcome: String::new(),
            car_laps: Vec::new(),
            cars_meta: Vec::new(),
            track_id: 0,
            subsession_id: 0,
            qualy_laps: Vec::new(),
            pit_stops: Vec::new(),
            weather: crate::iracing_sdk::tire_strategy::RaceWeatherContext::DRY,
            player_sectors: Vec::new(),
        }
    }
}

/// Desfecho de UMA quebra disparada ao vivo (registro estruturado, além do comando `!black`/
/// `!dq`). Acumula no `breakdown_log` a corrida toda e é drenado no import → tabela
/// `race_breakdowns` + debrief/notícia. `part`/`severity` como chave estável; `label` = a frase
/// do problema concreto (peça + modo + severidade).
#[derive(Clone, Debug, Serialize)]
pub struct BreakdownOutcome {
    pub car_number: u32,
    pub part: String,
    pub problem: u8,
    pub lap: u32,
    pub severity: String,
    pub penalty_secs: Option<u32>,
    pub forced: bool,
    pub label: String,
}

impl BreakdownOutcome {
    pub(super) fn from_event(car_number: u32, ev: &crate::car::breakdown::BreakdownEvent) -> Self {
        Self {
            car_number,
            part: ev.part.as_str().to_string(),
            problem: ev.problem,
            lap: ev.lap,
            severity: ev.severity.key().to_string(),
            penalty_secs: ev.penalty_secs,
            forced: ev.forced,
            label: ev.problem_label().to_string(),
        }
    }
}

/// Aviso pessoal ao jogador: uma peça DELE entrou na janela de risco (já pode falhar).
#[derive(Clone)]
pub struct PlayerWarning {
    /// Chave da peça (`PartType::as_str`, ex.: "engine").
    pub part: &'static str,
    /// Desgaste no momento do aviso, em % (≥ 95).
    pub wear_pct: u8,
}

/// Versão ENXUTA do histórico para o overlay "iRacing Conectado": só o que os
/// gráficos ao vivo usam (sem `qualy_laps`). Inclui `car_laps` para o seletor de
/// ritmo por piloto. Leve o suficiente para o polling a 1Hz.
#[derive(Clone, Serialize)]
pub struct RaceFeedback {
    pub laps: Vec<LapSnapshot>,
    pub player_laps: Vec<PlayerLap>,
    pub player_track: Vec<PlayerTrackPoint>,
    pub yellow_laps: Vec<i32>,
    pub cars_meta: Vec<CarMeta>,
    /// Identidade por carro DIRETO do YAML (sem gates de tentativa/quali) — para
    /// consumidores AO VIVO como o overlay de VR. `cars_meta` continua sendo a
    /// visão do histórico (grid, posições finais).
    pub cars_yaml_meta: Vec<YamlCarMeta>,
    pub player_car_idx: i32,
    /// `class_id -> nome curto` para rotular as abas por categoria.
    pub class_names: std::collections::HashMap<i64, String>,
    /// `car_idx -> nome do piloto` para mostrar nomes em vez de números.
    pub driver_names: std::collections::HashMap<i32, String>,
    /// Voltas do jogador que passaram pelo pit (o ritmo as ignora).
    pub player_pit_laps: Vec<i32>,
    /// Voltas de TODOS os carros (idx, lap, time) — base do seletor de ritmo.
    pub car_laps: Vec<CarLap>,
    /// Pins do jogador (incidentes/saídas) para o race trace.
    pub player_incidents: Vec<PlayerIncidentMark>,
}
