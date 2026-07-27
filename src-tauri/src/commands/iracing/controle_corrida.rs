//! Race Control: macro de bandeira amarela e chat do iRacing.

use super::*;

// ─── Race Control: macro de bandeira amarela ─────────────────────────────────

/// Estado da macro de bandeira (app.ini achado, instalada, slot, original).
#[tauri::command]
pub fn iracing_yellow_macro_status() -> YellowMacroStatus {
    race_control::status()
}

/// Instala a macro `!y$` no slot "You're welcome" (com backup).
#[tauri::command]
pub fn iracing_install_yellow_macro() -> Result<YellowMacroStatus, String> {
    race_control::install()
}

/// Dispara a macro instalada (aciona a bandeira no iRacing).
#[tauri::command]
pub fn iracing_throw_yellow() -> Result<(), String> {
    race_control::throw_yellow()
}

/// Liga/desliga o envio AUTOMÁTICO de bandeira pelo RaceControl.
#[tauri::command]
pub fn iracing_set_auto_yellow(enabled: bool) {
    race_monitor::set_auto_yellow(enabled);
}

/// Estado do envio automático de bandeira.
#[tauri::command]
pub fn iracing_auto_yellow_enabled() -> bool {
    race_monitor::auto_yellow_enabled()
}

/// Dispara um macro de chat por número (teste cru — descobrir o slot certo).
#[tauri::command]
pub fn iracing_send_chat_macro(macro_num: i32) -> Result<(), String> {
    iracing_sdk::send_chat_macro(macro_num).map_err(|e| e.to_string())
}

/// Envia um comando de chat de TEXTO LIVRE ao iRacing (ex.: `!black #1 20`).
/// Teste do caminho parametrizado (foca a janela → abre o chat → digita + Enter),
/// sem depender de macro no `app.ini`.
#[tauri::command]
pub fn iracing_send_chat_text(text: String) -> Result<(), String> {
    iracing_sdk::send_chat_text(&text).map_err(|e| e.to_string())
}

/// DEBUG: arma uma quebra GARANTIDA no carro do jogador pra próxima volta cruzada (motor na
/// parede). Testa o disparo ao vivo ponta a ponta: ao cruzar a linha, o monitor manda o
/// `!black`/`!dq` sozinho. Requer estar numa sessão do iRacing (número do carro conhecido).
#[tauri::command]
pub fn iracing_arm_test_breakdown() -> Result<bool, String> {
    Ok(crate::iracing_sdk::race_monitor::arm_test_breakdown())
}

/// DEBUG: arma a GRADE TODA com uma peça perto de quebrar por carro. Ao longo das próximas
/// voltas, os carros vão largando peças (`!black`/`!dq`), estrangulado pra não spammar o chat.
#[tauri::command]
pub fn iracing_arm_test_breakdown_grid() -> Result<(), String> {
    crate::iracing_sdk::race_monitor::arm_test_breakdown_grid();
    Ok(())
}
