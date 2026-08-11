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

/// O estado atual, sem escrever nada. É o que diz à tela se existe backup para
/// desfazer e se o simulador está aberto agora.
#[tauri::command]
pub fn iracing_modo_janela_status() -> ModoJanelaStatus {
    modo_janela::status()
}

/// Desfaz o ajuste: devolve os `rendererDX11*.ini` ao que eram antes de o Loop
/// tocar neles.
///
/// A contrapartida de aplicar sem perguntar. Erra com o simulador aberto pela mesma
/// razão do aplicar, e erra também quando não há backup nenhum — "desfiz" sobre um
/// arquivo intocado é uma mentira que só se descobre no jogo.
#[tauri::command]
pub fn iracing_modo_janela_restaurar() -> Result<ModoJanelaStatus, String> {
    modo_janela::restaurar()
}
