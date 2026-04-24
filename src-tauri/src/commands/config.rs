use crate::config::app_config::AppConfig;
use tauri::AppHandle;
use tauri::Manager;

#[tauri::command]
pub fn get_config(app: AppHandle) -> Result<AppConfig, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    Ok(AppConfig::load_or_default(&base_dir))
}

#[tauri::command]
pub fn update_config(app: AppHandle, new_config: AppConfig) -> Result<(), String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    // Carregar config atual para preservar metadados (merge)
    let mut current_config = AppConfig::load_or_default(&base_dir);

    // Validação de caminhos se presentes
    if let Some(ref path) = new_config.airosters_path {
        if !path.exists() {
            return Err("Caminho AI Rosters não existe no disco.".to_string());
        }
    }
    if let Some(ref path) = new_config.aiseasons_path {
        if !path.exists() {
            return Err("Caminho AI Seasons não existe no disco.".to_string());
        }
    }

    // Aplicar mudanças (Merge manual dos campos de settings)
    current_config.language = new_config.language;
    current_config.autosave_enabled = new_config.autosave_enabled;
    current_config.airosters_path = new_config.airosters_path;
    current_config.aiseasons_path = new_config.aiseasons_path;

    // last_career, window_state e base_dir são preservados de current_config ou atualizados via eventos específicos.

    current_config.save()
}
