//! Módulo isolado de TESTE do SDK do iRacing.
//!
//! Diferente do resto do app (um simulador de carreira *offline*), este módulo
//! fala com o iRacing REAL rodando na máquina. O SDK do iRacing publica seus
//! dados num arquivo mapeado em memória (`Local\IRSDKMemMapFileName`); aqui
//! abrimos esse mapeamento "na mão" (sem crate intermediária) e lemos o bloco de
//! informação de sessão, que é uma string YAML com pista, carros, pilotos e
//! classes.
//!
//! Escopo deliberadamente pequeno: conectar, validar e devolver a string de
//! sessão (+ alguns campos do cabeçalho). Telemetria por tick fica de fora.
//!
//! O binding real só existe em Windows; em outros alvos há um stub que devolve
//! [`IracingError::Unsupported`], para a lib continuar compilando em qualquer SO.

use serde::Serialize;
use thiserror::Error;

pub mod adaptive;
pub mod aiseason_results;
pub mod behavior;
pub mod car_difficulty;
pub mod paint_gen;
pub mod paths;
pub mod race_capture;
pub mod race_control;
pub mod race_monitor;
pub mod result_bridge;
pub mod results_gen;
pub mod rivalry_perception;
pub mod session_results;
pub mod roster_gen;
pub mod season_gen;
pub mod telemetry_analysis;
pub mod tire_strategy;
pub mod weather;

/// Nome do arquivo mapeado em memória que o iRacing expõe enquanto roda.
const MEM_MAP_FILE_NAME: &str = "Local\\IRSDKMemMapFileName";

/// Bit de `status` no cabeçalho indicando que o sim está conectado/ativo.
const STATUS_CONNECTED: i32 = 1;

/// Layout do cabeçalho `irsdk_header` (offsets em bytes a partir do início do
/// mapeamento). Só os campos que este teste consome estão nomeados.
mod header {
    pub const VER: usize = 0; // i32: versão da API do header
    pub const STATUS: usize = 4; // i32: bitfield (bit 0 = conectado)
    pub const TICK_RATE: usize = 8; // i32: ticks por segundo
    pub const SESSION_INFO_UPDATE: usize = 12; // i32: incrementa quando a sessão muda
    pub const SESSION_INFO_LEN: usize = 16; // i32: tamanho em bytes da string YAML
    pub const SESSION_INFO_OFFSET: usize = 20; // i32: offset da string YAML no mapeamento
    /// Tamanho mínimo do cabeçalho que precisamos ler com segurança.
    pub const MIN_LEN: usize = 24;

    // --- Telemetria ao vivo (variable buffers) ---
    pub const NUM_VARS: usize = 24; // i32: quantidade de variáveis de telemetria
    pub const VAR_HEADER_OFFSET: usize = 28; // i32: offset do array de irsdk_varHeader
    pub const NUM_BUF: usize = 32; // i32: quantos buffers de dados existem (<= 4)
    pub const VAR_BUF: usize = 48; // início do array irsdk_varBuf[4]
    pub const VAR_BUF_STRIDE: usize = 16; // tamanho de cada irsdk_varBuf
    pub const VAR_BUF_OFFSET_FIELD: usize = 4; // dentro do varBuf: i32 bufOffset (após tickCount)

    pub const VAR_HEADER_SIZE: usize = 144; // tamanho de cada irsdk_varHeader
    pub const VAR_TYPE: usize = 0; // i32: irsdk_VarType
    pub const VAR_OFFSET: usize = 4; // i32: offset do valor dentro da linha do buffer
    pub const VAR_COUNT: usize = 8; // i32: nº de entradas (1 = escalar; arrays CarIdx* = nº de carros)
    pub const VAR_NAME: usize = 16; // char[32]: nome da variável
    pub const VAR_NAME_MAX: usize = 32;
}

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

/// Conecta no iRacing, valida o cabeçalho e devolve a info de sessão.
///
/// Falha com [`IracingError::NotRunning`] se o sim não estiver aberto.
pub fn read_session() -> Result<IracingSession, IracingError> {
    imp::read_session()
}

/// Lê um snapshot de telemetria ao vivo do buffer mais recente.
///
/// Pensado para ser chamado repetidamente (polling) pela UI. Falha com
/// [`IracingError::NotRunning`] se o sim não estiver aberto.
pub fn read_telemetry() -> Result<IracingTelemetry, IracingError> {
    imp::read_telemetry()
}

/// Dispara um MACRO de chat do iRacing (1-15) via broadcast do Windows. O texto
/// do macro é configurado dentro do iRacing — ex.: um macro com `!yellow`. É o
/// único jeito do SDK "digitar" um comando de chat como `!yellow`.
pub fn send_chat_macro(macro_num: i32) -> Result<(), IracingError> {
    imp::send_chat_macro(macro_num)
}

/// Envia um comando de chat de TEXTO LIVRE ao iRacing (foca a janela, abre o
/// chat e digita o texto + Enter). Ao contrário do macro, o texto é montado em
/// runtime — é o caminho para comandos parametrizados como `!black #1 20`.
/// Requer a janela do sim em foreground; falha com [`IracingError::NotRunning`]
/// se o iRacing não estiver aberto. Fora do Windows, [`IracingError::Unsupported`].
pub fn send_chat_text(text: &str) -> Result<(), IracingError> {
    imp::send_chat_text(text)
}

/// Traz a janela do iRacing para frente (para o jogador "cair" no sim logo após
/// exportar os dados). Prefere o SIMULADOR ("iRacing.com Simulator"); se não
/// houver, foca a UI/launcher ("iRacing"). Devolve `Ok(true)` se achou e focou
/// alguma janela, `Ok(false)` se o iRacing não está aberto. Fora do Windows
/// devolve [`IracingError::Unsupported`].
pub fn focus_iracing_window() -> Result<bool, IracingError> {
    imp::focus_iracing_window()
}

/// Força NOSSA janela (handle bruto `HWND` como `isize`) para o primeiro plano,
/// contornando a trava anti-roubo-de-foco do Windows via `AttachThreadInput` com
/// a thread da janela atualmente em foco. Usado quando o iRacing fecha e o app
/// precisa subir sozinho (sem o usuário ter clicado nele). No-op fora do Windows.
pub fn force_foreground_window(hwnd_raw: isize) {
    imp::force_foreground_window(hwnd_raw);
}

/// `true` se a janela em primeiro plano AGORA pertence ao iRacing (título contém
/// "iracing"). Usado para reconquistar o foco só quando o iRacing o rouba, sem
/// incomodar o usuário caso ele tenha aberto outra coisa. `false` fora do Windows.
pub fn foreground_is_iracing() -> bool {
    imp::foreground_is_iracing()
}

/// Extrai `TrackDisplayName: ...` da string YAML de sessão, se presente.
/// O YAML do iRacing é raso o suficiente para uma varredura por linha bastar
/// neste teste — sem precisar de um parser YAML completo.
fn parse_track_name(yaml: &str) -> Option<String> {
    yaml.lines()
        .find_map(|line| line.trim().strip_prefix("TrackDisplayName:"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Extrai o `custid` (id de cliente iRacing) do JOGADOR do YAML de sessão:
/// `DriverInfo.DriverCarIdx` diz qual carro é o do jogador, e o `UserID` do
/// piloto com aquele `CarIdx` é o custid. Varredura por linha (sem parser YAML).
pub fn parse_player_custid(yaml: &str) -> Option<i64> {
    let target = yaml
        .lines()
        .find_map(|line| line.trim().strip_prefix("DriverCarIdx:"))
        .and_then(|v| v.trim().parse::<i64>().ok())?;

    let mut current: Option<i64> = None;
    for line in yaml.lines() {
        let t = line.trim();
        let t = t.strip_prefix("- ").unwrap_or(t);
        if let Some(rest) = t.strip_prefix("CarIdx:") {
            current = rest.trim().parse::<i64>().ok();
        } else if let Some(rest) = t.strip_prefix("UserID:") {
            if current == Some(target) {
                return rest.trim().parse::<i64>().ok().filter(|id| *id > 0);
            }
        }
    }
    None
}

/// Redline do carro do jogador (RPM) do YAML de sessão (`DriverInfo.DriverCarRedLine`).
/// Referência pro estilo de pilotagem (colado no limitador / short-shift). Varredura por
/// linha. `None` se ausente (o consumidor trata como redline desconhecido → ignora rotação).
pub fn parse_car_redline(yaml: &str) -> Option<f64> {
    yaml.lines()
        .find_map(|line| line.trim().strip_prefix("DriverCarRedLine:"))
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|rpm| *rpm > 0.0)
}

// ─── Custid do jogador: capturado automaticamente e persistido ───────────────
static PLAYER_CUSTID: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
static CUSTID_LOADED: std::sync::Once = std::sync::Once::new();

fn custid_file() -> std::path::PathBuf {
    std::env::temp_dir().join("loop_player_custid.txt")
}

fn ensure_custid_loaded() {
    CUSTID_LOADED.call_once(|| {
        if let Ok(s) = std::fs::read_to_string(custid_file()) {
            if let Ok(id) = s.trim().parse::<i64>() {
                if id > 0 {
                    PLAYER_CUSTID.store(id, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
    });
}

/// Custid do jogador já conhecido (capturado nesta sessão ou persistido de antes).
/// `None` enquanto o jogador não tiver entrado em nenhuma sessão.
pub fn cached_custid() -> Option<i64> {
    ensure_custid_loaded();
    let v = PLAYER_CUSTID.load(std::sync::atomic::Ordering::Relaxed);
    (v > 0).then_some(v)
}

/// Captura o custid do YAML de sessão e persiste — chamado pelo sampler de fundo
/// enquanto o jogador corre. Grava uma única vez; ignora chamadas seguintes.
pub fn note_session_custid(yaml: &str) {
    ensure_custid_loaded();
    if PLAYER_CUSTID.load(std::sync::atomic::Ordering::Relaxed) > 0 {
        return;
    }
    if let Some(id) = parse_player_custid(yaml) {
        PLAYER_CUSTID.store(id, std::sync::atomic::Ordering::Relaxed);
        let _ = std::fs::write(custid_file(), id.to_string());
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::ffi::OsStrExt;

    use winapi::um::handleapi::CloseHandle;
    use winapi::um::memoryapi::{MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ};

    /// Converte uma `&str` para uma string larga (UTF-16) terminada em NUL,
    /// como as APIs `...W` do Windows esperam.
    fn wide_null(value: &str) -> Vec<u16> {
        std::ffi::OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    /// Lê um `i32` little-endian de `base + offset` sem assumir alinhamento.
    ///
    /// # Safety
    /// `base` deve apontar para um mapeamento válido com pelo menos
    /// `offset + 4` bytes legíveis.
    unsafe fn read_i32(base: *const u8, offset: usize) -> i32 {
        std::ptr::read_unaligned(base.add(offset) as *const i32)
    }

    /// Decodifica os bytes do YAML de sessão. O iRacing usa Latin-1
    /// (ISO-8859-1) para nomes; mapear byte→char preserva tudo sem panic.
    fn decode_latin1(bytes: &[u8]) -> String {
        bytes.iter().map(|&b| b as char).collect()
    }

    pub fn read_session() -> Result<IracingSession, IracingError> {
        unsafe { with_view(extract_session) }
    }

    pub fn read_telemetry() -> Result<IracingTelemetry, IracingError> {
        unsafe { with_view(extract_telemetry) }
    }

    /// Abre o mapa de memória, mapeia a view, roda `extract` e garante o
    /// desmapeamento/fechamento mesmo em erro. Compartilhado por sessão e
    /// telemetria para não duplicar o ciclo de vida dos handles.
    ///
    /// # Safety
    /// `extract` só recebe um ponteiro válido para o início da view mapeada.
    unsafe fn with_view<T>(
        extract: unsafe fn(*const u8) -> Result<T, IracingError>,
    ) -> Result<T, IracingError> {
        let name = wide_null(MEM_MAP_FILE_NAME);

        let mapping = OpenFileMappingW(FILE_MAP_READ, 0, name.as_ptr());
        if mapping.is_null() {
            return Err(IracingError::NotRunning(MEM_MAP_FILE_NAME.to_string()));
        }

        let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
        if view.is_null() {
            let code = winapi::um::errhandlingapi::GetLastError();
            CloseHandle(mapping);
            return Err(IracingError::MapFailed(code));
        }

        let result = extract(view as *const u8);
        UnmapViewOfFile(view);
        CloseHandle(mapping);
        result
    }

    /// Lê os campos do cabeçalho e copia a string de sessão.
    ///
    /// # Safety
    /// `base` deve ser uma view válida do mapeamento do iRacing.
    unsafe fn extract_session(base: *const u8) -> Result<IracingSession, IracingError> {
        let status = read_i32(base, header::STATUS);
        if status & STATUS_CONNECTED == 0 {
            return Err(IracingError::NotConnected(status));
        }

        let api_version = read_i32(base, header::VER);
        let tick_rate = read_i32(base, header::TICK_RATE);
        let session_info_update = read_i32(base, header::SESSION_INFO_UPDATE);
        let session_info_len = read_i32(base, header::SESSION_INFO_LEN);
        let session_info_offset = read_i32(base, header::SESSION_INFO_OFFSET);

        if session_info_len <= 0 || session_info_offset < header::MIN_LEN as i32 {
            return Err(IracingError::InvalidHeader);
        }

        // Copia os bytes do YAML, cortando no primeiro NUL (o iRacing dimensiona
        // o buffer com folga e preenche o resto com zeros).
        let yaml_ptr = base.add(session_info_offset as usize);
        let raw = std::slice::from_raw_parts(yaml_ptr, session_info_len as usize);
        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        let session_yaml = decode_latin1(&raw[..end]);
        let track_name = parse_track_name(&session_yaml);

        Ok(IracingSession {
            api_version,
            tick_rate,
            session_info_update,
            session_info_len,
            track_name,
            session_yaml,
        })
    }

    /// Decodifica um valor de telemetria como `f64`, conforme o `irsdk_VarType`.
    ///
    /// # Safety
    /// `ptr` deve apontar para um valor válido do tamanho correspondente ao tipo.
    unsafe fn read_value(ptr: *const u8, var_type: i32) -> f64 {
        match var_type {
            0 => *(ptr as *const i8) as f64,                             // char
            1 => (*ptr != 0) as i32 as f64,                              // bool
            2 | 3 => std::ptr::read_unaligned(ptr as *const i32) as f64, // int / bitField
            4 => std::ptr::read_unaligned(ptr as *const f32) as f64,     // float
            5 => std::ptr::read_unaligned(ptr as *const f64),            // double
            _ => 0.0,
        }
    }

    /// Tamanho em bytes de um `irsdk_VarType` (para indexar arrays).
    fn type_size(var_type: i32) -> usize {
        match var_type {
            0 | 1 => 1,     // char, bool
            2 | 3 | 4 => 4, // int, bitField, float
            5 => 8,         // double
            _ => 4,
        }
    }

    /// Máximo de carros que o iRacing expõe nas variáveis de array.
    const IRSDK_MAX_CARS: usize = 64;

    /// Varre os cabeçalhos de variáveis uma vez e extrai os canais curados do
    /// buffer de telemetria mais recente.
    ///
    /// # Safety
    /// `base` deve ser uma view válida do mapeamento do iRacing.
    unsafe fn extract_telemetry(base: *const u8) -> Result<IracingTelemetry, IracingError> {
        let status = read_i32(base, header::STATUS);
        if status & STATUS_CONNECTED == 0 {
            return Err(IracingError::NotConnected(status));
        }

        let num_vars = read_i32(base, header::NUM_VARS);
        let var_header_offset = read_i32(base, header::VAR_HEADER_OFFSET);
        let num_buf = read_i32(base, header::NUM_BUF).clamp(0, 4);
        if num_vars <= 0 || var_header_offset <= 0 {
            return Err(IracingError::InvalidHeader);
        }

        // Escolhe o buffer com o maior tickCount (o snapshot mais recente).
        let mut best_tick = i32::MIN;
        let mut buf_offset = 0i32;
        for i in 0..num_buf as usize {
            let entry = header::VAR_BUF + i * header::VAR_BUF_STRIDE;
            let tick = read_i32(base, entry);
            if tick > best_tick {
                best_tick = tick;
                buf_offset = read_i32(base, entry + header::VAR_BUF_OFFSET_FIELD);
            }
        }
        if buf_offset <= 0 {
            return Err(IracingError::InvalidHeader);
        }
        let buffer = base.add(buf_offset as usize);

        let mut t = IracingTelemetry::default();
        // -1 = "não lido" (Default dá 0, que é um idx válido). Se o `CamCarIdx` não
        // vier no frame, o overlay cai no carro do jogador em vez de destacar o carro 0.
        t.cam_car_idx = -1;
        // Pré-aloca todos os slots de carro (idx correto); slots não preenchidos
        // ficam com os padrões "ausente" e são filtrados no fim.
        let mut cars: Vec<CarSnapshot> = (0..IRSDK_MAX_CARS)
            .map(|i| CarSnapshot {
                idx: i as i32,
                ..CarSnapshot::default()
            })
            .collect();

        // Uma única passada pelos cabeçalhos; cada canal de interesse é casado
        // pelo nome e lido do buffer mais recente.
        for i in 0..num_vars as usize {
            let head = base.add(var_header_offset as usize + i * header::VAR_HEADER_SIZE);
            let var_type = read_i32(head, header::VAR_TYPE);
            let var_offset = read_i32(head, header::VAR_OFFSET);
            let var_count = read_i32(head, header::VAR_COUNT).max(1) as usize;

            let name_bytes =
                std::slice::from_raw_parts(head.add(header::VAR_NAME), header::VAR_NAME_MAX);
            let end = name_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(name_bytes.len());
            let name = match std::str::from_utf8(&name_bytes[..end]) {
                Ok(name) => name,
                Err(_) => continue,
            };

            // Variáveis de array por carro (`CarIdx*`): lê cada entrada e
            // distribui para o slot correspondente.
            if name.starts_with("CarIdx") {
                let size = type_size(var_type);
                let n = var_count.min(IRSDK_MAX_CARS);
                for idx in 0..n {
                    let v = read_value(buffer.add(var_offset as usize + idx * size), var_type);
                    let car = &mut cars[idx];
                    match name {
                        "CarIdxLapDistPct" => car.lap_dist_pct = v,
                        "CarIdxLapCompleted" => car.lap_completed = v as i32,
                        "CarIdxLap" => car.lap = v as i32,
                        "CarIdxPosition" => car.position = v as i32,
                        "CarIdxClassPosition" => car.class_position = v as i32,
                        "CarIdxOnPitRoad" => car.on_pit_road = v != 0.0,
                        "CarIdxTrackSurface" => car.track_surface = v as i32,
                        "CarIdxGear" => car.gear = v as i32,
                        "CarIdxF2Time" => car.f2_time = v,
                        "CarIdxEstTime" => car.est_time = v,
                        "CarIdxLastLapTime" => car.last_lap_time = v,
                        "CarIdxBestLapTime" => car.best_lap_time = v,
                        "CarIdxTireCompound" => car.tire_compound = v as i32,
                        _ => {}
                    }
                }
                continue;
            }

            let value = read_value(buffer.add(var_offset as usize), var_type);
            match name {
                "PlayerCarIdx" => t.player_car_idx = value as i32,
                "CamCarIdx" => t.cam_car_idx = value as i32,
                "OnPitRoad" => t.player_on_pit_road = value != 0.0,
                "Speed" => t.speed_ms = value,
                "RPM" => t.rpm = value,
                "Gear" => t.gear = value as i32,
                "Throttle" => t.throttle = value,
                "Brake" => t.brake = value,
                "Clutch" => t.clutch = value,
                "SteeringWheelAngle" => t.steering_angle_rad = value,
                "Lap" => t.lap = value as i32,
                "LapDistPct" => t.lap_dist_pct = value,
                "FuelLevel" => t.fuel_level = value,
                "LapCurrentLapTime" => t.lap_current_time = value,
                "LapLastLapTime" => t.last_lap_time = value,
                "PlayerCarPosition" => t.position = value as i32,
                "IsOnTrack" => t.on_track = value != 0.0,
                "SessionTime" => t.session_time = value,
                "LapCompleted" => t.lap_completed = value as i32,
                "SessionState" => t.session_state = value as i32,
                "SessionNum" => t.session_num = value as i32,
                "IsInGarage" => t.is_in_garage = value != 0.0,
                "PlayerTrackSurface" => t.track_surface = value as i32,
                "SessionFlags" => t.session_flags = value as i32,
                "PlayerCarMyIncidentCount" => t.incident_count = value as i32,
                "LatAccel" => t.lat_accel = value,
                "LongAccel" => t.long_accel = value,
                "VertAccel" => t.vert_accel = value,
                "PlayerCarDriverIncidentCount" => t.driver_incident_count = value as i32,
                "PlayerCarTeamIncidentCount" => t.team_incident_count = value as i32,
                "YawRate" => t.yaw_rate = value,
                "RollRate" => t.roll_rate = value,
                "PitchRate" => t.pitch_rate = value,
                "PlayerCarTowTime" => t.tow_time = value,
                "IsReplayPlaying" => t.is_replay_playing = value != 0.0,
                "PitRepairNeeded" => t.pit_repair_needed = value,
                "PitOptRepairNeeded" => t.pit_opt_repair_needed = value,
                "IsOnTrackCar" => t.is_on_track_car = value != 0.0,
                "TrackWetness" => t.track_wetness = value as i32,
                "AirTemp" => t.air_temp = value,
                "TrackTemp" => t.track_temp = value,
                "RelativeHumidity" => t.relative_humidity = value,
                "WindVel" => t.wind_ms = value,
                "SessionTimeTotal" => t.session_time_total = value,
                "SessionTimeRemain" => t.session_time_remain = value,
                "SessionLapsRemainEx" => t.session_laps_remain_ex = value as i32,
                _ => {}
            }
        }

        // Marca o carro do jogador e mantém só os carros presentes (no mundo
        // ou com progresso válido) — descarta os slots vazios.
        if let Some(player) = cars.get_mut(t.player_car_idx.max(0) as usize) {
            player.is_player = true;
        }
        cars.retain(|car| car.track_surface > -1 || car.lap_dist_pct >= 0.0);
        t.cars = cars;

        t.speed_kmh = t.speed_ms * 3.6;
        Ok(t)
    }

    /// Dispara um macro de chat do iRacing via a broadcast message documentada
    /// do SDK (`IRSDK_BROADCASTMSG`). Não usa a shared memory — é uma mensagem
    /// de janela. O iRacing então "digita" o texto do macro (ex.: `!yellow`).
    pub fn send_chat_macro(macro_num: i32) -> Result<(), IracingError> {
        use winapi::um::winuser::{RegisterWindowMessageW, SendNotifyMessageW, HWND_BROADCAST};

        // irsdk_BroadcastChatComand = 8; irsdk_ChatCommand_Macro = 0.
        const BROADCAST_CHAT_COMMAND: i32 = 8;
        const CHAT_COMMAND_MACRO: i32 = 0;

        let name = wide_null("IRSDK_BROADCASTMSG");
        unsafe {
            let msg_id = RegisterWindowMessageW(name.as_ptr());
            if msg_id == 0 {
                return Err(IracingError::MapFailed(
                    winapi::um::errhandlingapi::GetLastError(),
                ));
            }
            // wParam = MAKELONG(msg, var1); lParam = MAKELONG(var2, var3).
            let wparam = (BROADCAST_CHAT_COMMAND as usize & 0xffff)
                | ((CHAT_COMMAND_MACRO as usize & 0xffff) << 16);
            let lparam = (macro_num & 0xffff) as isize;
            SendNotifyMessageW(HWND_BROADCAST, msg_id, wparam, lparam);
        }
        Ok(())
    }

    /// Abre a linha de chat do iRacing via broadcast `ChatCommand_BeginChat`
    /// (subcomando 1 do `BroadcastChatCommand`). Diferente do macro, isto só
    /// ABRE a caixa de digitação — o texto vem depois, por [`type_unicode`].
    fn begin_chat() -> Result<(), IracingError> {
        use winapi::um::winuser::{RegisterWindowMessageW, SendNotifyMessageW, HWND_BROADCAST};

        // irsdk_BroadcastChatComand = 8; irsdk_ChatCommand_BeginChat = 1.
        const BROADCAST_CHAT_COMMAND: i32 = 8;
        const CHAT_COMMAND_BEGIN: i32 = 1;

        let name = wide_null("IRSDK_BROADCASTMSG");
        unsafe {
            let msg_id = RegisterWindowMessageW(name.as_ptr());
            if msg_id == 0 {
                return Err(IracingError::MapFailed(
                    winapi::um::errhandlingapi::GetLastError(),
                ));
            }
            let wparam = (BROADCAST_CHAT_COMMAND as usize & 0xffff)
                | ((CHAT_COMMAND_BEGIN as usize & 0xffff) << 16);
            SendNotifyMessageW(HWND_BROADCAST, msg_id, wparam, 0);
        }
        Ok(())
    }

    /// Digita `text` (e Enter no fim) via `SendInput` a nível de SO — cada char
    /// vai como evento Unicode, então funciona com qualquer caractere sem depender
    /// de layout de teclado. Requer que a janela ALVO esteja em foreground (é o
    /// papel do `focus_iracing_window` antes desta chamada). Injeta tudo num único
    /// `SendInput` para preservar a ordem.
    fn type_unicode(text: &str) -> Result<(), IracingError> {
        use winapi::um::winuser::{
            SendInput, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_RETURN,
        };

        unsafe fn key_event(scan: u16, vk: u16, flags: u32) -> INPUT {
            let mut input: INPUT = std::mem::zeroed();
            input.type_ = INPUT_KEYBOARD;
            let ki = input.u.ki_mut();
            ki.wVk = vk;
            ki.wScan = scan;
            ki.dwFlags = flags;
            input
        }

        let mut inputs: Vec<INPUT> = Vec::new();
        unsafe {
            // Texto: cada unidade UTF-16 como key-down/key-up Unicode.
            for unit in text.encode_utf16() {
                inputs.push(key_event(unit, 0, KEYEVENTF_UNICODE));
                inputs.push(key_event(unit, 0, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
            }
            // Enter: por virtual-key (mais confiável que '\n' num jogo).
            inputs.push(key_event(0, VK_RETURN as u16, 0));
            inputs.push(key_event(0, VK_RETURN as u16, KEYEVENTF_KEYUP));

            let sent = SendInput(
                inputs.len() as u32,
                inputs.as_mut_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );
            if sent as usize != inputs.len() {
                return Err(IracingError::MapFailed(
                    winapi::um::errhandlingapi::GetLastError(),
                ));
            }
        }
        Ok(())
    }

    /// Envia um comando de chat de TEXTO LIVRE ao iRacing (ex.: `!black #1 20`).
    /// Foca a janela do sim → abre o chat (`begin_chat`) → digita o texto + Enter
    /// (`type_unicode`). É o caminho para comandos PARAMETRIZADOS, que o macro
    /// (texto fixo em `app.ini`, cacheado pelo sim) não cobre. Os `sleep` dão
    /// tempo do foco assentar e da caixa de chat abrir antes de digitar.
    pub fn send_chat_text(text: &str) -> Result<(), IracingError> {
        use std::thread::sleep;
        use std::time::Duration;

        let Some(hwnd) = find_iracing_hwnd() else {
            return Err(IracingError::NotRunning("iRacing".to_string()));
        };
        // Traz o sim ao foreground E VERIFICA que o SO aceitou. Sem essa checagem, o
        // `SendInput` seguinte seria disparado contra uma janela que nunca virou foco
        // (fullscreen exclusivo / trava de foco) e o comando (`!black`, `!dq`) sumiria
        // sem rastro. Reportando `ForegroundBlocked`, o chamador ao vivo consegue avisar
        // o jogador em vez de o sistema de quebra falhar em silêncio.
        //
        // LIMITE conhecido: se o sim JÁ é o foreground (jogador dirigindo) mas está em
        // fullscreen EXCLUSIVO, o `SetForegroundWindow` devolve sucesso e mesmo assim o
        // `SendInput` pode não chegar — isso não é detectável daqui. A checagem cobre o
        // caso comum (nosso app em segundo plano e o SO recusando o roubo de foco).
        if !bring_iracing_to_front(hwnd) {
            return Err(IracingError::ForegroundBlocked);
        }
        sleep(Duration::from_millis(150));
        begin_chat()?;
        sleep(Duration::from_millis(150));
        type_unicode(text)
    }

    /// Acha a janela do iRacing por título (varrendo as janelas de topo). Prefere o
    /// simulador (score 2) sobre a UI/launcher (score 1). Devolve o `HWND` como
    /// `isize` (bruto, `Copy`) ou `None` quando nada casa (sim fechado).
    fn find_iracing_hwnd() -> Option<isize> {
        use winapi::shared::minwindef::{BOOL, LPARAM, TRUE};
        use winapi::shared::windef::HWND;
        use winapi::um::winuser::{
            EnumWindows, GetWindowTextLengthW, GetWindowTextW, IsWindowVisible,
        };

        // Melhor candidato encontrado: o simulador (score 2) ganha da UI (score 1).
        struct Found {
            hwnd: HWND,
            score: i32,
        }

        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let found = &mut *(lparam as *mut Option<Found>);
            if IsWindowVisible(hwnd) == 0 {
                return TRUE;
            }
            let len = GetWindowTextLengthW(hwnd);
            if len <= 0 {
                return TRUE;
            }
            let mut buf: Vec<u16> = vec![0; (len + 1) as usize];
            let got = GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
            if got <= 0 {
                return TRUE;
            }
            buf.truncate(got as usize);
            let title = String::from_utf16_lossy(&buf).to_lowercase();
            let score = if title.contains("iracing.com simulator") {
                2
            } else if title.contains("iracing") {
                1
            } else {
                0
            };
            if score > 0 && found.as_ref().map_or(true, |f| score > f.score) {
                *found = Some(Found { hwnd, score });
            }
            TRUE
        }

        let mut found: Option<Found> = None;
        unsafe {
            EnumWindows(Some(enum_proc), &mut found as *mut _ as LPARAM);
        }
        found.map(|f| f.hwnd as isize)
    }

    /// Restaura (se minimizada) e traz a janela `hwnd_raw` ao primeiro plano. Devolve
    /// `true` se o SO aceitou a troca de foreground (`SetForegroundWindow` != 0); em
    /// fullscreen EXCLUSIVO ou com trava de foco o Windows recusa e devolve `false`.
    fn bring_iracing_to_front(hwnd_raw: isize) -> bool {
        use winapi::shared::windef::HWND;
        use winapi::um::winuser::{
            BringWindowToTop, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
        };

        let hwnd = hwnd_raw as HWND;
        unsafe {
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            BringWindowToTop(hwnd);
            SetForegroundWindow(hwnd) != 0
        }
    }

    /// Acha a janela do iRacing (simulador ou launcher) e a traz para frente, para o
    /// jogador "cair" no iRacing logo após exportar os dados. Como o app é a janela em
    /// foco quando o usuário clica no toast, o Windows permite passar o foreground.
    /// `Ok(false)` = iRacing fechado (janela não encontrada).
    pub fn focus_iracing_window() -> Result<bool, IracingError> {
        match find_iracing_hwnd() {
            Some(hwnd) => {
                bring_iracing_to_front(hwnd);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Força a janela `hwnd_raw` para o primeiro plano. O Windows só deixa o
    /// processo em foco trocar o foreground; quando o app está em segundo plano
    /// (caso do iRacing fechando), anexamos o input da nossa thread à thread da
    /// janela em foco (`AttachThreadInput`) para o `SetForegroundWindow` valer.
    pub fn force_foreground_window(hwnd_raw: isize) {
        use winapi::shared::windef::HWND;
        use winapi::um::processthreadsapi::GetCurrentThreadId;
        use winapi::um::winuser::{
            AttachThreadInput, BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId,
            IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
        };

        let hwnd = hwnd_raw as HWND;
        if hwnd.is_null() {
            return;
        }
        unsafe {
            let foreground = GetForegroundWindow();
            let fg_thread = GetWindowThreadProcessId(foreground, std::ptr::null_mut());
            let our_thread = GetCurrentThreadId();
            let attached = fg_thread != 0
                && fg_thread != our_thread
                && AttachThreadInput(our_thread, fg_thread, 1) != 0;

            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            } else {
                ShowWindow(hwnd, SW_SHOW);
            }
            BringWindowToTop(hwnd);
            SetForegroundWindow(hwnd);

            if attached {
                AttachThreadInput(our_thread, fg_thread, 0);
            }
        }
    }

    /// `true` se a janela em foco agora tem "iracing" no título.
    pub fn foreground_is_iracing() -> bool {
        use winapi::um::winuser::{GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW};
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return false;
            }
            let len = GetWindowTextLengthW(hwnd);
            if len <= 0 {
                return false;
            }
            let mut buf: Vec<u16> = vec![0; (len + 1) as usize];
            let got = GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
            if got <= 0 {
                return false;
            }
            buf.truncate(got as usize);
            String::from_utf16_lossy(&buf)
                .to_lowercase()
                .contains("iracing")
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn read_session() -> Result<IracingSession, IracingError> {
        Err(IracingError::Unsupported)
    }

    pub fn read_telemetry() -> Result<IracingTelemetry, IracingError> {
        Err(IracingError::Unsupported)
    }

    pub fn send_chat_macro(_macro_num: i32) -> Result<(), IracingError> {
        Err(IracingError::Unsupported)
    }

    pub fn send_chat_text(_text: &str) -> Result<(), IracingError> {
        Err(IracingError::Unsupported)
    }

    pub fn focus_iracing_window() -> Result<bool, IracingError> {
        Err(IracingError::Unsupported)
    }

    pub fn force_foreground_window(_hwnd_raw: isize) {}

    pub fn foreground_is_iracing() -> bool {
        false
    }
}
