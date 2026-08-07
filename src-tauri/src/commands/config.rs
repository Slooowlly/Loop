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

    // Chaves de overlay — aplicadas no espelho volátil na hora, então os escritores do
    // front (que leem por poll) param/voltam sem exigir restart.
    current_config.vr_overlay_mode = new_config.vr_overlay_mode;
    current_config.monitor_overlay_in_vr = new_config.monitor_overlay_in_vr;
    crate::commands::vr_overlay::set_vr_mode(&current_config.vr_overlay_mode);
    crate::commands::vr_overlay::set_monitor_in_vr(current_config.monitor_overlay_in_vr);

    // `spotter_takeover` NÃO é mesclado aqui de propósito: quem escreve essa chave é
    // `iracing_spotter_set`, que também precisa sincronizar o `app.ini` do iRacing.
    // Mesclar aqui deixaria a preferência e o arquivo divergirem sempre que a tela de
    // Configurações salvasse qualquer outra coisa.

    // last_career, window_state e base_dir são preservados de current_config ou atualizados via eventos específicos.

    // Reflete a troca de idioma no locale do backend na hora (sem exigir restart).
    rust_i18n::set_locale(&current_config.language);

    current_config.save()
}
