//! Tipos que cruzam a fronteira do SDK: o erro, a sessão, o snapshot de
//! telemetria ao vivo e o estado por carro.

use serde::Serialize;
use thiserror::Error;

/// Erros possíveis ao falar com o SDK do iRacing.
#[derive(Debug, Error)]
pub enum IracingError {
    #[error("iRacing não está rodando (mapa de memória '{0}' não encontrado)")]
    NotRunning(String),
    #[error("iRacing conectado, mas a sessão ainda não está pronta (status={0})")]
    NotConnected(i32),
    #[error("cabeçalho do SDK inválido ou incompleto")]
    InvalidHeader,
    #[error("falha ao mapear a memória do iRacing (código {0})")]
    MapFailed(u32),
    // O SO recusou trazer o iRacing ao primeiro plano (fullscreen EXCLUSIVO ou trava
    // de foco). Sem foreground, o `SendInput` do chat cairia no vazio silenciosamente.
    // Só é construído no caminho Windows (`send_chat_text`); no stub é dead code.
    #[cfg_attr(not(windows), allow(dead_code))]
    #[error("não foi possível trazer o iRacing ao primeiro plano — rode o sim em modo JANELA ou BORDERLESS (o fullscreen exclusivo bloqueia o envio de comandos)")]
    ForegroundBlocked,
    // Só é construído no stub não-Windows; em Windows é "dead code" esperado.
    #[cfg_attr(windows, allow(dead_code))]
    #[error("o SDK do iRacing só está disponível no Windows")]
    Unsupported,
}

/// Resultado da leitura de uma sessão do iRacing.
#[derive(Debug, Clone, Serialize)]
pub struct IracingSession {
    /// Versão do header da API exposta pelo sim.
    pub api_version: i32,
    /// Ticks de telemetria por segundo (ex.: 60).
    pub tick_rate: i32,
    /// Contador que o iRacing incrementa a cada mudança na info de sessão.
    pub session_info_update: i32,
    /// Tamanho declarado (em bytes) da string YAML de sessão.
    pub session_info_len: i32,
    /// Pista lida do YAML, quando encontrada (`TrackDisplayName`).
    pub track_name: Option<String>,
    /// String YAML completa da sessão (pista, carros, pilotos, classes).
    pub session_yaml: String,
}

/// Snapshot de telemetria ao vivo, lido do buffer mais recente. Um conjunto
/// curado dos canais mais úteis para um teste; o SDK expõe centenas de outros.
#[derive(Debug, Clone, Default, Serialize)]
pub struct IracingTelemetry {
    /// Velocidade em m/s (canal bruto `Speed`).
    pub speed_ms: f64,
    /// Velocidade em km/h, derivada de `speed_ms`.
    pub speed_kmh: f64,
    /// Rotação do motor (`RPM`).
    pub rpm: f64,
    /// Marcha (`Gear`): -1 = ré, 0 = neutro, 1..n.
    pub gear: i32,
    /// Acelerador, 0.0–1.0 (`Throttle`).
    pub throttle: f64,
    /// Freio, 0.0–1.0 (`Brake`).
    pub brake: f64,
    /// Embreagem, 0.0–1.0 (`Clutch`).
    pub clutch: f64,
    /// Ângulo do volante em radianos (`SteeringWheelAngle`).
    pub steering_angle_rad: f64,
    /// Volta atual (`Lap`).
    pub lap: i32,
    /// Progresso na volta, 0.0–1.0 (`LapDistPct`).
    pub lap_dist_pct: f64,
    /// Combustível em litros (`FuelLevel`).
    pub fuel_level: f64,
    /// Tempo da volta em andamento, em segundos (`LapCurrentLapTime`).
    pub lap_current_time: f64,
    /// Tempo da ÚLTIMA volta completa do jogador, em segundos (`LapLastLapTime`).
    /// -1 quando ainda não há volta válida. Base do gráfico de consistência.
    pub last_lap_time: f64,
    /// Posição do carro do jogador na sessão (`PlayerCarPosition`).
    pub position: i32,
    /// Se o carro do jogador está na pista (`IsOnTrack`).
    pub on_track: bool,
    /// Tempo decorrido da sessão em segundos (`SessionTime`). Cresce de forma
    /// monotônica durante a sessão e volta para ~0 quando a corrida reinicia.
    pub session_time: f64,
    /// Voltas completadas pelo jogador (`LapCompleted`).
    pub lap_completed: i32,
    /// Estado da sessão (`SessionState`): 0 Invalid, 1 GetInCar, 2 Warmup,
    /// 3 ParadeLaps, 4 Racing, 5 Checkered, 6 CoolDown.
    pub session_state: i32,
    /// Índice da sessão atual (`SessionNum`): practice/quali/race têm números
    /// diferentes.
    pub session_num: i32,
    /// Se o carro do jogador está na garagem (`IsInGarage`). Voltar à garagem no
    /// meio de uma corrida offline = abandono.
    pub is_in_garage: bool,
    /// Onde o carro do jogador está (`PlayerTrackSurface`, irsdk_TrkLoc):
    /// -1 NotInWorld, 0 OffTrack, 1 InPitStall, 2 ApproachingPits, 3 OnTrack.
    pub track_surface: i32,
    /// Bitfield de bandeiras da sessão (`SessionFlags`): contém DQ, bandeira
    /// preta, meatball, etc.
    pub session_flags: i32,
    /// Pontos de incidente do jogador na sessão (`PlayerCarMyIncidentCount`).
    /// O DELTA entre ticks é a classificação do iRacing: +1 saiu da pista,
    /// +2 perda de controle, +4 contato.
    pub incident_count: i32,
    /// Aceleração lateral em m/s² (`LatAccel`).
    pub lat_accel: f64,
    /// Aceleração longitudinal em m/s² (`LongAccel`).
    pub long_accel: f64,
    /// Aceleração vertical em m/s² (`VertAccel`) — zebra/impacto/capotagem.
    pub vert_accel: f64,
    /// Incidentes do piloto (`PlayerCarDriverIncidentCount`).
    pub driver_incident_count: i32,
    /// Incidentes do time (`PlayerCarTeamIncidentCount`).
    pub team_incident_count: i32,
    /// Taxa de guinada em rad/s (`YawRate`) — rodadas bruscas.
    pub yaw_rate: f64,
    /// Taxa de rolagem em rad/s (`RollRate`) — capotagem/impacto.
    pub roll_rate: f64,
    /// Taxa de arfagem em rad/s (`PitchRate`) — mergulho/impacto.
    pub pitch_rate: f64,
    /// Tempo de reboque acionado em segundos (`PlayerCarTowTime`).
    pub tow_time: f64,
    /// Tempo de reparo obrigatório no pit em segundos (`PitRepairNeeded`).
    pub pit_repair_needed: f64,
    /// Tempo de reparo OPCIONAL no pit em segundos (`PitOptRepairNeeded`) —
    /// reflete melhor o estrago total do carro após uma batida.
    pub pit_opt_repair_needed: f64,
    /// Se o carro do jogador ainda está ativo no mundo (`IsOnTrackCar`).
    pub is_on_track_car: bool,
    /// Se um replay está sendo reproduzido (`IsReplayPlaying`). Durante o replay
    /// o `SessionTime` reflete a posição do replay, não a corrida ao vivo.
    pub is_replay_playing: bool,
    /// Índice do carro do jogador (`PlayerCarIdx`) — qual entrada de `cars` é a dele.
    pub player_car_idx: i32,
    /// Índice do carro que a CÂMERA está assistindo (`CamCarIdx`). No replay/spectate
    /// segue quem você olha; dirigindo normal fica no seu carro (= `player_car_idx`).
    /// -1 = ainda não lido. Alimenta o destaque da torre "linha = quem você assiste".
    pub cam_car_idx: i32,
    /// Se o carro do jogador está no pit road (`OnPitRoad`).
    pub player_on_pit_road: bool,
    /// Umidade da pista (`TrackWetness`, irsdk_TrackWetness): 0 Unknown, 1 Dry,
    /// 2 MostlyDry, 3 VeryLightlyWet, 4 LightlyWet, 5 ModeratelyWet, 6 VeryWet,
    /// 7 ExtremelyWet. Base da inferência de composto (seco/chuva).
    pub track_wetness: i32,
    /// Temperatura do ar em °C (`AirTemp`).
    pub air_temp: f64,
    /// Temperatura da pista em °C (`TrackTemp`).
    pub track_temp: f64,
    /// Umidade relativa do ar, fração 0.0–1.0 (`RelativeHumidity`). Alimenta o Sistema de
    /// Quebra (umidade amplifica o calor no motor). 0 = ainda não lido.
    pub relative_humidity: f64,
    /// Velocidade do vento em m/s (`WindVel`). Alimenta o Sistema de Quebra (vento estressa
    /// suspensão + asas). 0 = ainda não lido.
    pub wind_ms: f64,
    /// Duração total da sessão em segundos (`SessionTimeTotal`) — corridas por tempo.
    pub session_time_total: f64,
    /// Tempo restante da sessão em segundos (`SessionTimeRemain`).
    pub session_time_remain: f64,
    /// Estimativa do iRacing de voltas restantes na sessão (`SessionLapsRemainEx`),
    /// válida INCLUSIVE em corrida por tempo. -1/0 quando indisponível.
    pub session_laps_remain_ex: i32,
    /// Snapshot de TODOS os carros na sessão (lido das variáveis de array
    /// `CarIdx*`). Só os carros presentes (no mundo) entram aqui.
    pub cars: Vec<CarSnapshot>,
}

/// Limiar de `TrackWetness` a partir do qual a pista conta como MOLHADA para fins de
/// pneu (≥ LightlyWet). Tunável.
pub const WET_SURFACE_MIN: i32 = 4;

impl IracingTelemetry {
    /// Pista molhada o bastante para exigir pneu de chuva (`TrackWetness` ≥ limiar).
    pub fn track_is_wet(&self) -> bool {
        self.track_wetness >= WET_SURFACE_MIN
    }
}

/// Estado de um carro qualquer na sessão (jogador ou IA), lido das variáveis de
/// array `CarIdx*`. Base para o AiIncidentAnalyzer e a visão do campo todo.
#[derive(Debug, Clone, Serialize)]
pub struct CarSnapshot {
    /// Índice do carro (`CarIdx`), 0..63.
    pub idx: i32,
    /// Se é o carro do jogador.
    pub is_player: bool,
    /// Progresso na volta, 0.0–1.0 (`CarIdxLapDistPct`). -1 quando fora do mundo.
    pub lap_dist_pct: f64,
    /// Voltas completadas (`CarIdxLapCompleted`).
    pub lap_completed: i32,
    /// Volta atual (`CarIdxLap`).
    pub lap: i32,
    /// Posição geral (`CarIdxPosition`).
    pub position: i32,
    /// Posição na classe (`CarIdxClassPosition`).
    pub class_position: i32,
    /// Se está no pit road (`CarIdxOnPitRoad`).
    pub on_pit_road: bool,
    /// Onde o carro está (`CarIdxTrackSurface`, irsdk_TrkLoc).
    pub track_surface: i32,
    /// Marcha (`CarIdxGear`).
    pub gear: i32,
    /// Tempo atrás do líder em segundos (`CarIdxF2Time`) — o gap AO LÍDER. Só é
    /// confiável em sessões hosted/multiplayer; em corrida de IA costuma vir 0.
    /// NÃO usar como proximidade entre carros (ver `est_time`).
    pub f2_time: f64,
    /// Tempo estimado até a posição atual na pista (`CarIdxEstTime`, segundos desde a
    /// linha). Populado em qualquer sessão (inclusive IA). A diferença de `est_time`
    /// entre dois carros na MESMA volta ≈ o gap na pista entre eles — a fonte certa
    /// para proximidade/duelo.
    pub est_time: f64,
    /// Última volta completa do carro (`CarIdxLastLapTime`); ≤0 = sem volta válida.
    pub last_lap_time: f64,
    /// Melhor volta do carro na sessão (`CarIdxBestLapTime`); ≤0 = nenhuma.
    pub best_lap_time: f64,
    /// Composto de pneu ESCOLHIDO pelo carro (`CarIdxTireCompound`, irsdk). Índice
    /// 0-based por série (0 = 1º composto, 1 = 2º…); -1 = desconhecido/indisponível.
    /// É a MESMA info que o RaceLab mostra — e vem preenchida inclusive pra IA e ANTES
    /// da largada (assim que os carros escolhem o pneu). O mapa índice→nome é por série.
    pub tire_compound: i32,
}

impl Default for CarSnapshot {
    fn default() -> Self {
        // Padrões "ausente": fora do mundo / sem progresso, para filtrar slots vazios.
        Self {
            idx: 0,
            is_player: false,
            lap_dist_pct: -1.0,
            lap_completed: 0,
            lap: 0,
            position: 0,
            class_position: 0,
            on_pit_road: false,
            track_surface: -1,
            gear: 0,
            f2_time: 0.0,
            est_time: 0.0,
            last_lap_time: 0.0,
            best_lap_time: 0.0,
            tire_compound: -1,
        }
    }
}
