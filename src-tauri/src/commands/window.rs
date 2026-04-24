use tauri::Window;

#[tauri::command]
pub fn minimize_window(window: Window) -> Result<(), String> {
    window
        .minimize()
        .map_err(|error| format!("Falha ao minimizar janela: {error}"))
}

#[tauri::command]
pub fn toggle_maximize_window(window: Window) -> Result<bool, String> {
    let is_maximized = window
        .is_maximized()
        .map_err(|error| format!("Falha ao ler estado maximizado: {error}"))?;

    if is_maximized {
        window
            .unmaximize()
            .map_err(|error| format!("Falha ao restaurar janela: {error}"))?;
    } else {
        window
            .maximize()
            .map_err(|error| format!("Falha ao maximizar janela: {error}"))?;
    }

    window
        .is_maximized()
        .map_err(|error| format!("Falha ao ler estado maximizado: {error}"))
}

#[tauri::command]
pub fn close_window(window: Window) -> Result<(), String> {
    window
        .close()
        .map_err(|error| format!("Falha ao fechar janela: {error}"))
}

#[tauri::command]
pub fn get_window_maximized(window: Window) -> Result<bool, String> {
    window
        .is_maximized()
        .map_err(|error| format!("Falha ao ler estado maximizado: {error}"))
}

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
