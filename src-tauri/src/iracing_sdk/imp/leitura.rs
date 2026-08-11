//! Leitura da shared memory do iRacing: abre o mapeamento, escolhe o buffer mais
//! recente e extrai a info de sessão (YAML) e o snapshot de telemetria.

use winapi::um::handleapi::CloseHandle;
use winapi::um::memoryapi::{MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ};

use super::util::{decode_latin1, read_i32, read_value, type_size, wide_null, IRSDK_MAX_CARS};
use crate::iracing_sdk::{
    header, parse_track_name, CarSnapshot, IracingError, IracingSession, IracingTelemetry,
    MEM_MAP_FILE_NAME, MEM_MAP_FILE_NAME_NU, STATUS_CONNECTED, VarDoSdk,
};

/// `ERROR_FILE_NOT_FOUND` — o mapeamento não existe: o sim está fechado (ou não
/// publicou o SDK). É o "erro" esperado e silencioso na maior parte do tempo.
pub(super) const ERRO_NAO_ENCONTRADO: u32 = 2;
/// `ERROR_ACCESS_DENIED` — o mapeamento EXISTE e o Windows recusou abrir. Estado
/// completamente diferente do anterior, e o que antes se disfarçava de "sim
/// fechado" deixando a telemetria zerada sem explicação.
pub(super) const ERRO_ACESSO_NEGADO: u32 = 5;

pub fn read_session() -> Result<IracingSession, IracingError> {
    unsafe { with_view(extract_session) }
}

pub fn read_telemetry() -> Result<IracingTelemetry, IracingError> {
    unsafe { with_view(extract_telemetry) }
}

/// Tudo que o SDK publica nesta build, como ele mesmo se descreve.
///
/// Não alimenta nenhuma lógica do jogo — vai para a captura de corrida, uma vez por
/// gravação. O motivo é simples: `extract_telemetry` casa nomes num `match`, e um nome
/// que não existe cai no `_ => {}` calado. Sem o inventário, um canal ausente e um
/// canal zerado são a mesma coisa vista de fora, e a diferença entre os dois muda o
/// que dá para construir.
pub fn read_var_inventory() -> Result<Vec<VarDoSdk>, IracingError> {
    unsafe { with_view(extract_var_inventory) }
}

/// Lê um `char[]` de tamanho fixo do cabeçalho, cortando no primeiro `NUL`.
///
/// # Safety
/// `ptr` deve apontar para pelo menos `max` bytes válidos.
unsafe fn texto_fixo(ptr: *const u8, max: usize) -> String {
    let bytes = std::slice::from_raw_parts(ptr, max);
    let fim = bytes.iter().position(|&b| b == 0).unwrap_or(max);
    String::from_utf8_lossy(&bytes[..fim]).into_owned()
}

/// # Safety
/// `base` deve ser uma view válida do mapeamento do iRacing.
unsafe fn extract_var_inventory(base: *const u8) -> Result<Vec<VarDoSdk>, IracingError> {
    let status = read_i32(base, header::STATUS);
    if status & STATUS_CONNECTED == 0 {
        return Err(IracingError::NotConnected(status));
    }
    let num_vars = read_i32(base, header::NUM_VARS);
    let var_header_offset = read_i32(base, header::VAR_HEADER_OFFSET);
    if num_vars <= 0 || var_header_offset <= 0 {
        return Err(IracingError::InvalidHeader);
    }

    let mut saida = Vec::with_capacity(num_vars as usize);
    for i in 0..num_vars as usize {
        let head = base.add(var_header_offset as usize + i * header::VAR_HEADER_SIZE);
        let nome = texto_fixo(head.add(header::VAR_NAME), header::VAR_NAME_MAX);
        if nome.is_empty() {
            continue;
        }
        saida.push(VarDoSdk {
            nome,
            tipo: read_i32(head, header::VAR_TYPE),
            quantidade: read_i32(head, header::VAR_COUNT).max(1),
            unidade: texto_fixo(head.add(header::VAR_UNIT), header::VAR_UNIT_MAX),
            descricao: texto_fixo(head.add(header::VAR_DESC), header::VAR_DESC_MAX),
        });
    }
    saida.sort_by(|a, b| a.nome.cmp(&b.nome));
    Ok(saida)
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
    let (mapping, _) = abrir_mapeamento()?;

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

/// Abre o mapeamento do SDK, com uma SEGUNDA CHANCE e sem perder o motivo da
/// falha. Devolve o handle (que o chamador fecha) e o nome que funcionou.
///
/// Duas tentativas porque `Local\` resolve no namespace da sessão do Windows:
/// se o Loop e o sim caírem em sessões distintas, o nome canônico não acha nada
/// mesmo com o iRacing aberto, e o nome nu ainda acha. A segunda chamada só
/// acontece depois de a primeira já ter falhado, então não custa nada no caminho
/// feliz.
///
/// E o `GetLastError()` é PRESERVADO: distinguir "não encontrado" (sim fechado,
/// silêncio) de "acesso negado" (elevação, precisa avisar o jogador) é a única
/// forma de a UI dizer algo útil em vez de mostrar tudo zerado.
///
/// # Safety
/// Chama a API do Windows; o handle devolvido precisa de `CloseHandle`.
pub(super) unsafe fn abrir_mapeamento(
) -> Result<(winapi::shared::ntdef::HANDLE, &'static str), IracingError> {
    let mut ultimo_erro = ERRO_NAO_ENCONTRADO;

    for nome in [MEM_MAP_FILE_NAME, MEM_MAP_FILE_NAME_NU] {
        let wide = wide_null(nome);
        let mapping = OpenFileMappingW(FILE_MAP_READ, 0, wide.as_ptr());
        if !mapping.is_null() {
            return Ok((mapping, nome));
        }
        ultimo_erro = winapi::um::errhandlingapi::GetLastError();
        // Acesso negado é conclusivo: o objeto EXISTE e a permissão é que falta.
        // Tentar o outro nome não muda nada e só embaralharia o código de erro.
        if ultimo_erro == ERRO_ACESSO_NEGADO {
            return Err(IracingError::AccessDenied(ultimo_erro));
        }
    }

    if ultimo_erro == ERRO_ACESSO_NEGADO {
        return Err(IracingError::AccessDenied(ultimo_erro));
    }
    Err(IracingError::NotRunning(MEM_MAP_FILE_NAME.to_string()))
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
                    "CarIdxTrackSurfaceMaterial" => car.track_surface_material = v as i32,
                    "CarIdxRPM" => car.rpm = v,
                    "CarIdxSteer" => car.steer = v,
                    "CarIdxSessionFlags" => car.session_flags = v as i32,
                    "CarIdxPaceLine" => car.pace_line = v as i32,
                    "CarIdxPaceRow" => car.pace_row = v as i32,
                    "CarIdxPaceFlags" => car.pace_flags = v as i32,
                    "CarIdxBestLapNum" => car.best_lap_num = v as i32,
                    "CarIdxFastRepairsUsed" => car.fast_repairs_used = v as i32,
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
            // Os canais de dano chamam-se `…Left` no SDK (tempo de reparo que FALTA). Os
            // nomes `PitRepairNeeded`/`PitOptRepairNeeded`, que estavam aqui, não existem no
            // inventário de variáveis: nunca casavam, e o dano do carro lia zero para sempre
            // — inclusive com o meatball na tela.
            "PitRepairLeft" => t.pit_repair_needed = value,
            "PitOptRepairLeft" => t.pit_opt_repair_needed = value,
            "IsOnTrackCar" => t.is_on_track_car = value != 0.0,
            "TrackWetness" => t.track_wetness = value as i32,
            "AirTemp" => t.air_temp = value,
            "TrackTemp" => t.track_temp = value,
            "RelativeHumidity" => t.relative_humidity = value,
            "WindVel" => t.wind_ms = value,
            "SessionTimeTotal" => t.session_time_total = value,
            "SessionTimeRemain" => t.session_time_remain = value,
            "SessionLapsRemainEx" => t.session_laps_remain_ex = value as i32,
            "CarLeftRight" => t.car_left_right = value as i32,
            "SessionTick" => t.session_tick = value as i32,
            "SessionLapsRemain" => t.session_laps_remain = value as i32,
            "SessionLapsTotal" => t.session_laps_total = value as i32,
            "PaceMode" => t.pace_mode = value as i32,
            "PitsOpen" => t.pits_open = value != 0.0,
            "Precipitation" => t.precipitation = value,
            "WeatherDeclaredWet" => t.weather_declared_wet = value != 0.0,
            "TrackTempCrew" => t.track_temp_crew = value,
            "Skies" => t.skies = value as i32,
            "FogLevel" => t.fog_level = value,
            "WindDir" => t.wind_dir = value,
            "CamCameraNumber" => t.cam_camera_number = value as i32,
            "CamGroupNumber" => t.cam_group_number = value as i32,
            "CamCameraState" => t.cam_camera_state = value as i32,
            "ReplayFrameNum" => t.replay_frame_num = value as i32,
            "ReplaySessionNum" => t.replay_session_num = value as i32,
            "ReplaySessionTime" => t.replay_session_time = value,
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
