//! Modo janela do iRacing — o pré-requisito do overlay do Loop.
//!
//! A lógica mora em [`crate::iracing_sdk::modo_janela`]; aqui é só a casca.

use crate::iracing_sdk::modo_janela::{self, ModoJanelaStatus};

/// Ajusta os `rendererDX11*.ini` para janela sem borda maximizada.
///
/// Idempotente e sem pergunta: a exportação da etapa dispara e ignora o resultado,
/// cobrindo quem abriu o Loop com o simulador já rodando (aí o boot não conseguiu
/// escrever). Erra, em vez de mentir sucesso, com o simulador aberto.
#[tauri::command]
pub fn iracing_modo_janela_aplicar() -> Result<ModoJanelaStatus, String> {
    modo_janela::aplicar()
}
