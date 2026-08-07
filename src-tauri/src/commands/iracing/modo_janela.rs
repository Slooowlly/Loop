//! Modo janela do iRacing — o pré-requisito do overlay do Loop.
//!
//! A lógica mora em [`crate::iracing_sdk::modo_janela`]; aqui é só a casca.

use crate::iracing_sdk::modo_janela::{self, ModoJanelaStatus};

/// O iRacing já está em modo janela? Traz junto o `deve_perguntar`, que é o
/// único campo que a exportação da etapa precisa olhar.
#[tauri::command]
pub fn iracing_modo_janela_status() -> ModoJanelaStatus {
    modo_janela::status()
}

/// Ajusta os `rendererDX11*.ini` para janela sem borda maximizada.
///
/// Só é chamado depois de um "sim" explícito do jogador: é a configuração gráfica
/// DELE, não a nossa. Erra (em vez de mentir sucesso) com o simulador aberto.
#[tauri::command]
pub fn iracing_modo_janela_aplicar() -> Result<ModoJanelaStatus, String> {
    modo_janela::aplicar()
}
