//! Controle da janela de OVERLAY no monitor (por cima do iRacing).
//!
//! A janela é DECLARADA no `tauri.conf.json` (label "overlay"): transparente, sem
//! borda, sempre no topo, fora da barra de tarefas e OCULTA no boot. Declarar no
//! config (em vez de criar em runtime) é o caminho confiável pra transparência no
//! Windows — o WebView2 recebe o fundo transparente na ordem certa.
//!
//! Além de mostrar/esconder, aqui mora o VIGIA DE CURSOR do "hover": travada, a
//! janela é clique-atravessa (o mouse vai pro jogo), então o webview NÃO enxerga o
//! mouse. Um loop lê a posição global do cursor (`GetCursorPos`) e, quando ele entra
//! na área da torre, torna a janela interativa e avisa o overlay (`overlay-hover`)
//! pra revelar cadeado+olho; ao sair, volta a clique-atravessa. Assim o mouse só é
//! "capturado" enquanto está EM CIMA da torre — fora dela vai direto pro iRacing.

use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

const LABEL: &str = "overlay";

// careerId da sessão que o overlay está mostrando (a janela lê via comando + poll).
fn active_career() -> &'static Mutex<Option<String>> {
    static C: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

/// Configuração do vigia de hover: se está vigiando (travado + visível) e a caixa
/// da torre em px LÓGICOS (o app reporta; o vigia converte pra físico com o
/// scale-factor da janela).
struct HoverCfg {
    watch: bool,
    w_css: f64,
    h_css: f64,
}

fn hover_cfg() -> &'static Mutex<HoverCfg> {
    static C: OnceLock<Mutex<HoverCfg>> = OnceLock::new();
    C.get_or_init(|| {
        Mutex::new(HoverCfg {
            watch: false,
            w_css: 512.0,
            h_css: 200.0,
        })
    })
}

#[cfg(windows)]
fn cursor_pos() -> Option<(i32, i32)> {
    use winapi::shared::windef::POINT;
    use winapi::um::winuser::GetCursorPos;
    unsafe {
        let mut p = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut p) != 0 {
            Some((p.x, p.y))
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
fn cursor_pos() -> Option<(i32, i32)> {
    None
}

/// Sobe o vigia de cursor (uma thread). Chamado uma vez no setup. Ocioso (30 ms)
/// enquanto `watch` está desligado — custo desprezível.
pub fn start_hover_watcher(app: AppHandle) {
    thread::spawn(move || {
        let mut prev_inside = false;
        loop {
            thread::sleep(Duration::from_millis(30));

            let (watch, w_css, h_css) = match hover_cfg().lock() {
                Ok(c) => (c.watch, c.w_css, c.h_css),
                Err(_) => (false, 512.0, 200.0),
            };

            let win = match app.get_webview_window(LABEL) {
                Some(w) => w,
                None => continue,
            };
            let shown = win.is_visible().unwrap_or(false);

            let inside = if watch && shown {
                match (win.outer_position(), cursor_pos()) {
                    (Ok(p), Some((cx, cy))) => {
                        let sf = win.scale_factor().unwrap_or(1.0);
                        let rw = (w_css * sf) as i32;
                        let rh = (h_css * sf) as i32;
                        cx >= p.x && cx < p.x + rw && cy >= p.y && cy < p.y + rh
                    }
                    _ => false,
                }
            } else {
                false
            };

            if inside != prev_inside {
                // Só alterna o clique-atravessa quando estamos VIGIANDO (travado). Se
                // watch=false (destravado/oculto), o app é dono da interatividade —
                // não mexemos, só emitimos o hover pra baixo (o overlay o ignora).
                if watch {
                    let _ = win.set_ignore_cursor_events(!inside);
                }
                let _ = app.emit_to(LABEL, "overlay-hover", inside);
                prev_inside = inside;
            }
        }
    });
}

/// Liga/desliga o vigia de hover. O app chama com `visível && travado`.
#[tauri::command]
pub fn overlay_set_hover_watch(active: bool) {
    if let Ok(mut c) = hover_cfg().lock() {
        c.watch = active;
    }
}

/// Reporta a caixa (px lógicos) que conta como "em cima da torre" — torre inteira
/// quando visível, ou o nub mínimo quando oculta.
#[tauri::command]
pub fn overlay_set_hover_rect(width: f64, height: f64) {
    if let Ok(mut c) = hover_cfg().lock() {
        c.w_css = width.max(1.0);
        c.h_css = height.max(1.0);
    }
}

/// Mostra o overlay de monitor pra `career_id`. Torna a janela clique-atravessa
/// (o mouse vai pro iRacing) e a exibe.
#[tauri::command]
pub fn overlay_window_show(app: AppHandle, career_id: String) -> Result<(), String> {
    *active_career().lock().map_err(|e| e.to_string())? = Some(career_id);
    if let Some(win) = app.get_webview_window(LABEL) {
        // NÃO mexe em ignore_cursor_events aqui: o padrão click-through é setado no
        // boot e o modo "mover" o alterna via set_interactive. Se re-travássemos no
        // show, o modo mover seria desfeito.
        win.show().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Alterna o overlay entre TRAVADO (clique-atravessa, display-only) e MÓVEL
/// (interativo: recebe o mouse pra poder arrastar). Chamado pelo botão "Mover" do
/// app; ao terminar, volta pra travado.
#[tauri::command]
pub fn overlay_window_set_interactive(app: AppHandle, interactive: bool) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(LABEL) {
        win.set_ignore_cursor_events(!interactive)
            .map_err(|e| format!("Falha ao alternar interatividade: {e}"))?;
        if interactive {
            let _ = win.set_focus();
        }
    }
    Ok(())
}

/// Esconde o overlay de monitor (mantém a janela viva pra reexibir instantâneo).
#[tauri::command]
pub fn overlay_window_hide(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(LABEL) {
        win.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// careerId que o overlay deve mostrar (a própria janela de overlay lê isto por
/// poll pra saber de quem puxar os dados ao vivo). `None` = nenhuma sessão ativa.
#[tauri::command]
pub fn overlay_active_career() -> Option<String> {
    active_career().lock().ok().and_then(|c| c.clone())
}
