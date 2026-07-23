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

    // Aplicar mudanças (Merge manual dos campos de settings)
    current_config.language = new_config.language;
    current_config.autosave_enabled = new_config.autosave_enabled;

    // Telemetria de produto: só sobrescreve se o front MANDOU um valor. `None`
    // vindo do front significa "não mexi nisso", e não "o jogador recusou" — a
    // diferença importa, porque `None` no disco é o que faz o aviso de primeira
    // execução aparecer. Aplicado nos estáticos na hora (sem exigir restart).
    if new_config.telemetry_enabled.is_some() {
        current_config.telemetry_enabled = new_config.telemetry_enabled;
        crate::telemetry::set_enabled(new_config.telemetry_enabled.unwrap_or(false));
    }

    // last_career, window_state e base_dir são preservados de current_config ou atualizados via eventos específicos.

    // Reflete a troca de idioma no locale do backend na hora (sem exigir restart).
    rust_i18n::set_locale(&current_config.language);

    current_config.save()
}
