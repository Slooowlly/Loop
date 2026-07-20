//! Ponte de VR (lado escritor).
//!
//! O front desenha os overlays num `<canvas>` e manda os pixels RGBA pra cá; aqui a
//! gente escreve numa **memória compartilhada nomeada** que a OpenXR API layer (dentro
//! do processo do iRacing) lê e sobe pra textura do quad em VR.
//!
//! São DOIS painéis independentes, cada um com seu mapeamento + config de pose:
//!   • TORRE   → `Local\iRacerOverlayFrame`  (retrato 1024×2048)
//!   • RÁDIO   → `Local\iRacerEngineerFrame`  (banner 1024×256)
//! O contrato (nome, cabeçalho, formato) casa com `vr-overlay/src/shared_frame.h`.
//!
//! Os bytes chegam como corpo RAW da IPC (ArrayBuffer no JS) — nada de JSON, que
//! transformaria MBs em milhões de números.

#[cfg(windows)]
mod imp {
    use std::sync::{Mutex, OnceLock};

    use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
    use winapi::um::memoryapi::{CreateFileMappingW, MapViewOfFile, FILE_MAP_WRITE};

    const MAGIC: u32 = 0x5241_4352; // 'RACR'
    // Cabeçalho estendido (casa com IracerFrameHeader em shared_frame.h):
    // [0]magic [1]w [2]h [3]frame | [4]epoch [5]lockMode [6]x [7]y [8]z [9]yaw
    // [10]pitch [11]scale [12]visible [13]recenterSeq [14]recenterKey
    // → 15 palavras de 4 bytes = 60 bytes.
    const HEADER: usize = 60;
    const PAGE_READWRITE: u32 = 0x04;

    /// Descrição de um painel: nome do mapeamento, resolução e pose padrão (espelham
    /// os defaults da layer). A TORRE é retrato cockpit-locked; o RÁDIO é banner
    /// head-locked (segue o olhar, no meio-alto da visão).
    pub struct Panel {
        idx: usize,
        name: &'static str,
        w: u32,
        h: u32,
        def_lock: i32,
        def_x: f32,
        def_y: f32,
        def_z: f32,
        def_yaw: f32,
        def_pitch: f32,
        def_scale: f32,
    }

    pub const TOWER: Panel = Panel {
        idx: 0,
        name: "Local\\iRacerOverlayFrame",
        w: 1024,
        h: 2048,
        def_lock: 1, // world/cockpit
        def_x: 0.0,
        def_y: 0.61,
        def_z: -1.29,
        def_yaw: 0.0,
        def_pitch: 30.0,
        def_scale: 1.7,
    };

    pub const ENGINEER: Panel = Panel {
        idx: 1,
        name: "Local\\iRacerEngineerFrame",
        w: 1024,
        h: 256,
        def_lock: 0, // head-locked: segue o olhar
        def_x: 0.0,
        def_y: 0.08,
        def_z: -1.0,
        def_yaw: 0.0,
        def_pitch: 0.0,
        def_scale: 1.0,
    };

    fn panel_size(p: &Panel) -> usize {
        HEADER + (p.w as usize) * (p.h as usize) * 4
    }

    /// Ponteiros brutos do mapeamento. Guardados como `usize` pra poderem cruzar
    /// threads; todo acesso é serializado pelo `Mutex` de fora.
    struct Bridge {
        handle: usize,
        base: usize,
    }
    unsafe impl Send for Bridge {}

    /// Uma célula de ponte por painel (índice 0 = torre, 1 = rádio).
    fn cell(idx: usize) -> &'static Mutex<Option<Bridge>> {
        static TOWER_C: OnceLock<Mutex<Option<Bridge>>> = OnceLock::new();
        static ENG_C: OnceLock<Mutex<Option<Bridge>>> = OnceLock::new();
        if idx == 0 {
            TOWER_C.get_or_init(|| Mutex::new(None))
        } else {
            ENG_C.get_or_init(|| Mutex::new(None))
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Garante que o mapeamento do painel existe, criando-o (com cabeçalho + pose
    /// padrão) na primeira vez. Chamado tanto pela escrita de frame quanto de pose.
    fn ensure_bridge(guard: &mut Option<Bridge>, p: &Panel) -> Result<(), String> {
        if guard.is_some() {
            return Ok(());
        }
        let size = panel_size(p);
        unsafe {
            let name = wide(p.name);
            let handle = CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                std::ptr::null_mut(),
                PAGE_READWRITE,
                ((size as u64) >> 32) as u32,
                ((size as u64) & 0xFFFF_FFFF) as u32,
                name.as_ptr(),
            );
            if handle.is_null() {
                return Err("CreateFileMappingW falhou".into());
            }
            let base = MapViewOfFile(handle, FILE_MAP_WRITE, 0, 0, size);
            if base.is_null() {
                CloseHandle(handle);
                return Err("MapViewOfFile falhou".into());
            }
            let up = base as *mut u32;
            let fp = base as *mut f32;
            *up.add(0) = MAGIC;
            *up.add(1) = p.w;
            *up.add(2) = p.h;
            *up.add(3) = 0; // frame
            *up.add(4) = 0; // epoch
            *(up.add(5) as *mut i32) = p.def_lock;
            *fp.add(6) = p.def_x;
            *fp.add(7) = p.def_y;
            *fp.add(8) = p.def_z;
            *fp.add(9) = p.def_yaw;
            *fp.add(10) = p.def_pitch;
            *fp.add(11) = p.def_scale;
            *up.add(12) = 1; // visible
            *up.add(13) = 0; // recenterSeq
            *up.add(14) = 0; // recenterKey
            *guard = Some(Bridge {
                handle: handle as usize,
                base: base as usize,
            });
        }
        Ok(())
    }

    pub fn write_frame(p: &Panel, bytes: &[u8]) -> Result<(), String> {
        let expected = (p.w as usize) * (p.h as usize) * 4;
        if bytes.len() != expected {
            return Err(format!(
                "frame com tamanho errado: {} bytes (esperado {})",
                bytes.len(),
                expected
            ));
        }
        let mut guard = cell(p.idx).lock().map_err(|e| e.to_string())?;
        ensure_bridge(&mut guard, p)?;
        let b = guard.as_ref().ok_or("bridge não inicializada")?;
        unsafe {
            let base = b.base as *mut u8;
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), base.add(HEADER), expected);
            let frame = (base as *mut u32).add(3);
            *frame = (*frame).wrapping_add(1);
        }
        Ok(())
    }

    /// Lê o bloco de POSE atual (pra sincronizar quando o teclado, na layer, mexeu).
    pub fn read_pose(p: &Panel) -> Option<(i32, f32, f32, f32, f32, f32, f32, bool)> {
        let guard = cell(p.idx).lock().ok()?;
        let b = guard.as_ref()?;
        unsafe {
            let up = b.base as *const u32;
            let fp = b.base as *const f32;
            Some((
                *(up.add(5) as *const i32),
                *fp.add(6),
                *fp.add(7),
                *fp.add(8),
                *fp.add(9),
                *fp.add(10),
                *fp.add(11),
                *up.add(12) != 0,
            ))
        }
    }

    /// Escreve o bloco de POSE (trava/posição/giro/inclinação/escala/visível). Toca só
    /// as palavras da config — nunca os pixels nem o contador de frame.
    #[allow(clippy::too_many_arguments)]
    pub fn write_pose(
        p: &Panel,
        lock_mode: i32,
        x: f32,
        y: f32,
        z: f32,
        yaw: f32,
        pitch: f32,
        scale: f32,
        visible: bool,
    ) -> Result<(), String> {
        let mut guard = cell(p.idx).lock().map_err(|e| e.to_string())?;
        ensure_bridge(&mut guard, p)?;
        let b = guard.as_ref().ok_or("bridge não inicializada")?;
        unsafe {
            let up = b.base as *mut u32;
            let fp = b.base as *mut f32;
            *(up.add(5) as *mut i32) = if lock_mode == 0 { 0 } else { 1 };
            *fp.add(6) = x;
            *fp.add(7) = y;
            *fp.add(8) = z;
            *fp.add(9) = yaw;
            *fp.add(10) = pitch;
            *fp.add(11) = if scale > 0.01 { scale } else { 1.0 };
            *up.add(12) = if visible { 1 } else { 0 };
            let epoch = up.add(4);
            *epoch = (*epoch).wrapping_add(1);
        }
        Ok(())
    }

    /// Pede um RECENTRO no painel: incrementa `recenterSeq` (word 13).
    pub fn bump_recenter(p: &Panel) -> Result<(), String> {
        let mut guard = cell(p.idx).lock().map_err(|e| e.to_string())?;
        ensure_bridge(&mut guard, p)?;
        let b = guard.as_ref().ok_or("bridge não inicializada")?;
        unsafe {
            let seq = (b.base as *mut u32).add(13);
            *seq = (*seq).wrapping_add(1);
        }
        Ok(())
    }

    /// Define a tecla (Virtual-Key) que recentra o painel dentro do VR (0 = desligada).
    pub fn set_recenter_key(p: &Panel, vk: u32) -> Result<(), String> {
        let mut guard = cell(p.idx).lock().map_err(|e| e.to_string())?;
        ensure_bridge(&mut guard, p)?;
        let b = guard.as_ref().ok_or("bridge não inicializada")?;
        unsafe {
            *(b.base as *mut u32).add(14) = vk;
        }
        Ok(())
    }
}

// ───────────────────────── Pose (serialização compartilhada) ─────────────────────────

/// Pose atual de um painel (pra sincronizar o app quando o teclado, lido pela layer,
/// mexeu na posição). Devolve `None` se a ponte ainda não foi criada.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayPose {
    lock_mode: i32,
    x: f32,
    y: f32,
    z: f32,
    yaw: f32,
    pitch: f32,
    scale: f32,
    visible: bool,
}

#[cfg(windows)]
fn to_pose(t: (i32, f32, f32, f32, f32, f32, f32, bool)) -> OverlayPose {
    OverlayPose {
        lock_mode: t.0,
        x: t.1,
        y: t.2,
        z: t.3,
        yaw: t.4,
        pitch: t.5,
        scale: t.6,
        visible: t.7,
    }
}

// ───────────────────────── Comandos: TORRE (painel 1) ─────────────────────────

#[cfg(windows)]
#[tauri::command]
pub fn vr_overlay_write_frame(request: tauri::ipc::Request<'_>) -> Result<(), String> {
    match request.body() {
        tauri::ipc::InvokeBody::Raw(data) => imp::write_frame(&imp::TOWER, data),
        _ => Err("esperado corpo raw (ArrayBuffer)".into()),
    }
}

#[cfg(windows)]
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn vr_overlay_set_pose(
    lock_mode: i32,
    x: f32,
    y: f32,
    z: f32,
    yaw: f32,
    pitch: f32,
    scale: f32,
    visible: bool,
) -> Result<(), String> {
    imp::write_pose(&imp::TOWER, lock_mode, x, y, z, yaw, pitch, scale, visible)
}

#[cfg(windows)]
#[tauri::command]
pub fn vr_overlay_get_pose() -> Option<OverlayPose> {
    imp::read_pose(&imp::TOWER).map(to_pose)
}

#[cfg(windows)]
#[tauri::command]
pub fn vr_overlay_recenter() -> Result<(), String> {
    imp::bump_recenter(&imp::TOWER)
}

#[cfg(windows)]
#[tauri::command]
pub fn vr_overlay_set_recenter_key(vk: u32) -> Result<(), String> {
    imp::set_recenter_key(&imp::TOWER, vk)
}

// ───────────────────────── Comandos: RÁDIO (painel 2) ─────────────────────────

#[cfg(windows)]
#[tauri::command]
pub fn vr_engineer_write_frame(request: tauri::ipc::Request<'_>) -> Result<(), String> {
    match request.body() {
        tauri::ipc::InvokeBody::Raw(data) => imp::write_frame(&imp::ENGINEER, data),
        _ => Err("esperado corpo raw (ArrayBuffer)".into()),
    }
}

#[cfg(windows)]
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn vr_engineer_set_pose(
    lock_mode: i32,
    x: f32,
    y: f32,
    z: f32,
    yaw: f32,
    pitch: f32,
    scale: f32,
    visible: bool,
) -> Result<(), String> {
    imp::write_pose(&imp::ENGINEER, lock_mode, x, y, z, yaw, pitch, scale, visible)
}

#[cfg(windows)]
#[tauri::command]
pub fn vr_engineer_get_pose() -> Option<OverlayPose> {
    imp::read_pose(&imp::ENGINEER).map(to_pose)
}

#[cfg(windows)]
#[tauri::command]
pub fn vr_engineer_recenter() -> Result<(), String> {
    imp::bump_recenter(&imp::ENGINEER)
}

#[cfg(windows)]
#[tauri::command]
pub fn vr_engineer_set_recenter_key(vk: u32) -> Result<(), String> {
    imp::set_recenter_key(&imp::ENGINEER, vk)
}

// ───────────────────────── Stubs não-Windows ─────────────────────────

#[cfg(not(windows))]
#[tauri::command]
pub fn vr_overlay_write_frame(_request: tauri::ipc::Request<'_>) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn vr_overlay_set_pose(
    _lock_mode: i32,
    _x: f32,
    _y: f32,
    _z: f32,
    _yaw: f32,
    _pitch: f32,
    _scale: f32,
    _visible: bool,
) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
#[tauri::command]
pub fn vr_overlay_get_pose() -> Option<OverlayPose> {
    None
}

#[cfg(not(windows))]
#[tauri::command]
pub fn vr_overlay_recenter() -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
#[tauri::command]
pub fn vr_overlay_set_recenter_key(_vk: u32) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
#[tauri::command]
pub fn vr_engineer_write_frame(_request: tauri::ipc::Request<'_>) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn vr_engineer_set_pose(
    _lock_mode: i32,
    _x: f32,
    _y: f32,
    _z: f32,
    _yaw: f32,
    _pitch: f32,
    _scale: f32,
    _visible: bool,
) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
#[tauri::command]
pub fn vr_engineer_get_pose() -> Option<OverlayPose> {
    None
}

#[cfg(not(windows))]
#[tauri::command]
pub fn vr_engineer_recenter() -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
#[tauri::command]
pub fn vr_engineer_set_recenter_key(_vk: u32) -> Result<(), String> {
    Ok(())
}
