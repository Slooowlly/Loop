//! Ponte de VR (lado escritor).
//!
//! O front desenha a torre de tempos num `<canvas>` 512×1024 e manda os pixels
//! RGBA pra cá; aqui a gente escreve numa **memória compartilhada nomeada** que a
//! OpenXR API layer (dentro do processo do iRacing) lê e sobe pra textura do
//! overlay em VR.
//!
//! O contrato do bloco (nome, cabeçalho, formato) é o mesmo de
//! `vr-overlay/src/shared_frame.h`. Qualquer mudança tem que casar dos dois lados.
//!
//! Os bytes chegam como corpo RAW da IPC (ArrayBuffer no JS) — nada de JSON, que
//! transformaria 2 MB em milhões de números.

#[cfg(windows)]
mod imp {
    use std::sync::{Mutex, OnceLock};

    use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
    use winapi::um::memoryapi::{CreateFileMappingW, MapViewOfFile, FILE_MAP_WRITE};

    const MAGIC: u32 = 0x5241_4352; // 'RACR'
    // Resolução 2× do layout lógico (512×1024) — texto nítido no VR. Casa com
    // IRACER_OVERLAY_W/H em shared_frame.h e VR_W/VR_H em towerCanvas.js.
    const W: u32 = 1024;
    const H: u32 = 2048;
    // Cabeçalho estendido (casa com IracerFrameHeader em shared_frame.h):
    // [0]magic [1]w [2]h [3]frame | [4]epoch [5]lockMode [6]x [7]y [8]z [9]yaw
    // [10]pitch [11]scale [12]visible [13]recenterSeq [14]recenterKey
    // → 15 palavras de 4 bytes = 60 bytes.
    const HEADER: usize = 60;
    const SIZE: usize = HEADER + (W as usize) * (H as usize) * 4;
    const PAGE_READWRITE: u32 = 0x04;

    // Padrões de pose (espelham kDefaultConfig da layer): cockpit-locked, à frente.
    const DEF_LOCK: i32 = 1; // 1 = world/cockpit, 0 = cabeça
    const DEF_X: f32 = 0.30;
    const DEF_Y: f32 = -0.10;
    const DEF_Z: f32 = -1.0;
    const DEF_YAW: f32 = -12.0;
    const DEF_PITCH: f32 = 8.0;
    const DEF_SCALE: f32 = 1.0;

    /// Ponteiros brutos do mapeamento. Guardados como `usize` pra poderem cruzar
    /// threads; todo acesso é serializado pelo `Mutex` de fora.
    struct Bridge {
        handle: usize,
        base: usize,
    }
    unsafe impl Send for Bridge {}

    static BRIDGE: OnceLock<Mutex<Option<Bridge>>> = OnceLock::new();

    fn cell() -> &'static Mutex<Option<Bridge>> {
        BRIDGE.get_or_init(|| Mutex::new(None))
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Garante que o mapeamento existe, criando-o (com cabeçalho + pose padrão) na
    /// primeira vez. Chamado tanto pela escrita de frame quanto pela de pose —
    /// qualquer um dos dois pode ser o primeiro a rodar.
    fn ensure_bridge(guard: &mut Option<Bridge>) -> Result<(), String> {
        if guard.is_some() {
            return Ok(());
        }
        unsafe {
            let name = wide("Local\\iRacerOverlayFrame");
            let handle = CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                std::ptr::null_mut(),
                PAGE_READWRITE,
                ((SIZE as u64) >> 32) as u32,
                ((SIZE as u64) & 0xFFFF_FFFF) as u32,
                name.as_ptr(),
            );
            if handle.is_null() {
                return Err("CreateFileMappingW falhou".into());
            }
            let base = MapViewOfFile(handle, FILE_MAP_WRITE, 0, 0, SIZE);
            if base.is_null() {
                CloseHandle(handle);
                return Err("MapViewOfFile falhou".into());
            }
            // Cabeçalho + config de pose padrão (a layer usa até o app mandar a sua).
            let up = base as *mut u32;
            let fp = base as *mut f32;
            *up.add(0) = MAGIC;
            *up.add(1) = W;
            *up.add(2) = H;
            *up.add(3) = 0; // frame
            *up.add(4) = 0; // epoch
            *(up.add(5) as *mut i32) = DEF_LOCK;
            *fp.add(6) = DEF_X;
            *fp.add(7) = DEF_Y;
            *fp.add(8) = DEF_Z;
            *fp.add(9) = DEF_YAW;
            *fp.add(10) = DEF_PITCH;
            *fp.add(11) = DEF_SCALE;
            *up.add(12) = 1; // visible
            *up.add(13) = 0; // recenterSeq
            *up.add(14) = 0; // recenterKey (0 = tecla de recentro desligada)
            *guard = Some(Bridge {
                handle: handle as usize,
                base: base as usize,
            });
        }
        Ok(())
    }

    pub fn write_frame(bytes: &[u8]) -> Result<(), String> {
        let expected = (W as usize) * (H as usize) * 4;
        if bytes.len() != expected {
            return Err(format!(
                "frame com tamanho errado: {} bytes (esperado {})",
                bytes.len(),
                expected
            ));
        }

        let mut guard = cell().lock().map_err(|e| e.to_string())?;
        ensure_bridge(&mut guard)?;

        let b = guard.as_ref().ok_or("bridge não inicializada")?;
        unsafe {
            let base = b.base as *mut u8;
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), base.add(HEADER), expected);
            // Incrementa o contador de frame (leitor usa pra saber que mudou).
            let frame = (base as *mut u32).add(3);
            *frame = (*frame).wrapping_add(1);
        }
        Ok(())
    }

    /// Lê o bloco de POSE atual da memória compartilhada. Serve pro app sincronizar
    /// quando o AJUSTE POR TECLADO (na layer) mexeu na pose. `None` se a ponte ainda
    /// não existe (ninguém escreveu nada).
    pub fn read_pose() -> Option<(i32, f32, f32, f32, f32, f32, f32, bool)> {
        let guard = cell().lock().ok()?;
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

    /// Escreve o bloco de POSE (trava/posição/giro/escala/visível). Toca só as
    /// palavras da config — nunca os pixels nem o contador de frame.
    pub fn write_pose(
        lock_mode: i32,
        x: f32,
        y: f32,
        z: f32,
        yaw: f32,
        pitch: f32,
        scale: f32,
        visible: bool,
    ) -> Result<(), String> {
        let mut guard = cell().lock().map_err(|e| e.to_string())?;
        ensure_bridge(&mut guard)?;

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
            // epoch por último (bump = "config nova"); a layer nem precisa, mas
            // ajuda a diagnosticar pelo log.
            let epoch = up.add(4);
            *epoch = (*epoch).wrapping_add(1);
        }
        Ok(())
    }

    /// Pede um RECENTRO: incrementa `recenterSeq` (word 13). A layer vê a mudança e
    /// reancora o overlay na cabeça atual. Toca só essa palavra.
    pub fn bump_recenter() -> Result<(), String> {
        let mut guard = cell().lock().map_err(|e| e.to_string())?;
        ensure_bridge(&mut guard)?;
        let b = guard.as_ref().ok_or("bridge não inicializada")?;
        unsafe {
            let seq = (b.base as *mut u32).add(13);
            *seq = (*seq).wrapping_add(1);
        }
        Ok(())
    }

    /// Define a tecla (Virtual-Key do Windows) que recentra o overlay dentro do VR.
    /// 0 = desligada. Escreve só a word 14.
    pub fn set_recenter_key(vk: u32) -> Result<(), String> {
        let mut guard = cell().lock().map_err(|e| e.to_string())?;
        ensure_bridge(&mut guard)?;
        let b = guard.as_ref().ok_or("bridge não inicializada")?;
        unsafe {
            *(b.base as *mut u32).add(14) = vk;
        }
        Ok(())
    }
}

#[cfg(windows)]
#[tauri::command]
pub fn vr_overlay_write_frame(request: tauri::ipc::Request<'_>) -> Result<(), String> {
    match request.body() {
        tauri::ipc::InvokeBody::Raw(data) => imp::write_frame(data),
        _ => Err("esperado corpo raw (ArrayBuffer)".into()),
    }
}

#[cfg(not(windows))]
#[tauri::command]
pub fn vr_overlay_write_frame(_request: tauri::ipc::Request<'_>) -> Result<(), String> {
    Ok(())
}

/// Ajusta a POSE do painel de VR (trava cockpit/cabeça, posição em metros, giro,
/// escala, visível). O app é a fonte da verdade e persiste no front; aqui só
/// empurramos os valores pra memória compartilhada que a layer lê.
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
    imp::write_pose(lock_mode, x, y, z, yaw, pitch, scale, visible)
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

/// Pose atual do painel (pra sincronizar o app quando o teclado, lido pela layer,
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
#[tauri::command]
pub fn vr_overlay_get_pose() -> Option<OverlayPose> {
    imp::read_pose().map(|(lock_mode, x, y, z, yaw, pitch, scale, visible)| OverlayPose {
        lock_mode,
        x,
        y,
        z,
        yaw,
        pitch,
        scale,
        visible,
    })
}

#[cfg(not(windows))]
#[tauri::command]
pub fn vr_overlay_get_pose() -> Option<OverlayPose> {
    None
}

/// Recentra o overlay: reancora o world-lock na cabeça atual (mesma posição sempre).
#[cfg(windows)]
#[tauri::command]
pub fn vr_overlay_recenter() -> Result<(), String> {
    imp::bump_recenter()
}

#[cfg(not(windows))]
#[tauri::command]
pub fn vr_overlay_recenter() -> Result<(), String> {
    Ok(())
}

/// Define a tecla de recentro dentro do VR (Virtual-Key do Windows; 0 = desligada).
#[cfg(windows)]
#[tauri::command]
pub fn vr_overlay_set_recenter_key(vk: u32) -> Result<(), String> {
    imp::set_recenter_key(vk)
}

#[cfg(not(windows))]
#[tauri::command]
pub fn vr_overlay_set_recenter_key(_vk: u32) -> Result<(), String> {
    Ok(())
}
