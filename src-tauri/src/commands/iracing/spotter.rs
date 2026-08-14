//! Spotter: silenciar o nativo do iRacing enquanto o Loop assume o papel, e ler
//! o que o nosso spotter está enxergando.
//!
//! A lógica mora em [`crate::iracing_sdk::spotter_control`] (o `app.ini`) e em
//! [`crate::iracing_sdk::spotter`] (a vizinhança lateral); aqui é só a casca.

use super::*;
use crate::iracing_sdk::spotter::{self, EstadoSpotter};
use crate::iracing_sdk::spotter_control::{self, SpotterStatus};

/// Estado atual: `app.ini` achado, se o nativo está calado e o que será devolvido.
#[tauri::command]
pub fn iracing_spotter_status() -> SpotterStatus {
    spotter_control::status()
}

/// Liga/desliga o Loop como spotter e aplica no `app.ini` na hora.
///
/// A preferência é persistida ANTES da escrita no `app.ini`, de propósito: se o
/// arquivo estiver bloqueado ou ausente, o jogador ainda vê o interruptor onde ele
/// o deixou, e o boot seguinte tenta de novo. Ao contrário, o interruptor voltaria
/// sozinho e pareceria um botão quebrado.
#[tauri::command]
pub fn iracing_spotter_set(app: tauri::AppHandle, enabled: bool) -> Result<SpotterStatus, String> {
    use tauri::Manager;

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let mut config = crate::config::app_config::AppConfig::load_or_default(&base_dir);
    config.spotter_takeover = enabled;
    config.save()?;

    spotter_control::sincronizar(enabled)
}

/// Devolve o spotter nativo AGORA, sem mexer na preferência.
///
/// É o escape para quem vai abrir o iRacing online sem fechar o Loop — o caso em que
/// a devolução automática do fechamento não teria acontecido ainda.
#[tauri::command]
pub fn iracing_spotter_restore() -> Result<SpotterStatus, String> {
    spotter_control::devolver()
}

/// Vizinhança lateral AGORA + os últimos anúncios confirmados.
///
/// Barato de propósito (um lock, sem tocar no SDK nem no banco): o amostrador de
/// fundo é quem lê a 60 Hz, então a UI pode consultar num ritmo próprio sem
/// arrastar a leitura da shared memory junto.
#[tauri::command]
pub fn iracing_spotter_vizinhanca() -> EstadoSpotter {
    spotter::estado()
}
