use tauri::Window;

#[tauri::command]
pub fn minimize_window(window: Window) -> Result<(), String> {
    window
        .minimize()
        .map_err(|error| format!("Falha ao minimizar janela: {error}"))
}

/// Inicia um arrasto da janela ao nível do SO (a partir do cursor atual).
/// Necessário porque a janela usa `decorations: false` (sem barra de título),
/// então não há superfície nativa para mover a janela — inclusive entre monitores.
#[tauri::command]
pub fn start_window_drag(window: Window) -> Result<(), String> {
    window
        .start_dragging()
        .map_err(|error| format!("Falha ao arrastar janela: {error}"))
}

#[tauri::command]
pub fn close_window(window: Window) -> Result<(), String> {
    window
        .close()
        .map_err(|error| format!("Falha ao fechar janela: {error}"))
}

// Não há comando de MAXIMIZAR aqui, e é de propósito. Os controles de janela do
// `WindowControlsDrawer` trabalham com TELA CHEIA: `toggle_fullscreen_window` e
// `get_window_fullscreen`. Os antigos `toggle_maximize_window` e `get_window_maximized`
// ficaram registrados sem nenhum chamador e saíram em 11/08/2026.

#[tauri::command]
pub fn toggle_fullscreen_window(window: Window) -> Result<bool, String> {
    let is_fullscreen = window
        .is_fullscreen()
        .map_err(|error| format!("Falha ao ler estado de tela cheia: {error}"))?;

    window
        .set_fullscreen(!is_fullscreen)
        .map_err(|error| format!("Falha ao alternar tela cheia: {error}"))?;

    window
        .is_fullscreen()
        .map_err(|error| format!("Falha ao ler estado de tela cheia: {error}"))
}

#[tauri::command]
pub fn get_window_fullscreen(window: Window) -> Result<bool, String> {
    window
        .is_fullscreen()
        .map_err(|error| format!("Falha ao ler estado de tela cheia: {error}"))
}
