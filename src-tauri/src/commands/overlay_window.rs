//! Controle da janela de OVERLAY no monitor (por cima do iRacing).
//!
//! A janela é DECLARADA no `tauri.conf.json` (label "overlay"): transparente, sem
//! borda, sempre no topo, fora da barra de tarefas e OCULTA no boot. Declarar no
//! config (em vez de criar em runtime) é o caminho confiável pra transparência no
//! Windows — o WebView2 recebe o fundo transparente na ordem certa.
//!
//! Aqui só MOSTRAMOS/ESCONDEMOS conforme o app detecta uma sessão ao vivo, e
//! guardamos o careerId ativo pra a própria janela ler (`overlay_active_career`),
//! já que ela é um webview separado sem o store do app.

use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Manager};

const LABEL: &str = "overlay";

// careerId da sessão que o overlay está mostrando (a janela lê via comando + poll).
fn active_career() -> &'static Mutex<Option<String>> {
    static C: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
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
