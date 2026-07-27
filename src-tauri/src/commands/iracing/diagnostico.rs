//! Comandos de DIAGNÓSTICO da conexão com o iRacing.
//!
//! Existem para um cenário específico: o problema acontece no PC de outra pessoa
//! e a única coisa que se tem dela é uma mensagem de texto. Estes comandos
//! transformam "não está puxando nada" num relato com código de erro do Windows,
//! estado das janelas e um arquivo de log anexável.

use super::*;

/// Retrato cruzado da conexão AGORA (memória do SDK × janela do sim × elevação).
///
/// Barato: uma abertura de mapeamento e um `EnumWindows`. Pensado para um botão
/// que o jogador aperta, não para polling.
#[tauri::command]
pub fn iracing_diagnostico() -> crate::iracing_sdk::DiagnosticoIracing {
    let d = iracing_sdk::diagnosticar();
    // O que o jogador vê na tela também fica no arquivo: se ele mandar só o log,
    // o diagnóstico está lá; se mandar só o print, o veredito está no print.
    crate::diagnostico::linha(
        "diagnostico",
        &format!(
            "solicitado pela UI: veredito={:?} memoria_ok={} erro={} janela={} simulador={} elevado={} ticks={}",
            d.veredito,
            d.memoria_ok,
            d.memoria_erro,
            d.janela_encontrada,
            d.janela_simulador,
            d.elevado,
            d.ticks_observados
        ),
    );
    d
}

/// Final do log (últimos ~60 KB), para exibir e copiar. É o fim do arquivo porque
/// é onde está o que acabou de acontecer.
#[tauri::command]
pub fn iracing_log_ler() -> String {
    crate::diagnostico::ler_final(60 * 1024)
}

/// Caminho do arquivo de log, para o jogador achar e anexar. `None` se o log não
/// pôde ser criado (pasta sem permissão de escrita).
#[tauri::command]
pub fn iracing_log_caminho() -> Option<String> {
    crate::diagnostico::caminho().map(|p| p.to_string_lossy().to_string())
}

/// Envia o log ao desenvolvedor e devolve o TICKET que o jogador informa no
/// relato. Só roda por clique explícito — ver [`crate::diagnostico::enviar`].
///
/// `async` + `spawn_blocking` porque o envio é HTTP bloqueante e pode pagar o
/// cold start do servidor: num comando síncrono isso congelaria a janela.
#[tauri::command]
pub async fn iracing_log_enviar(nota: Option<String>) -> Result<String, String> {
    // O diagnóstico vai junto: quase sempre é ele que responde a pergunta, e
    // colhê-lo aqui garante que retrata o momento do envio.
    let diag = serde_json::to_value(iracing_sdk::diagnosticar())
        .unwrap_or(serde_json::Value::Null);
    tauri::async_runtime::spawn_blocking(move || crate::diagnostico::enviar(nota, diag))
        .await
        .map_err(|e| format!("Falha ao executar o envio: {e}"))?
}

/// Abre o Explorer já com o arquivo de log selecionado. Um clique a menos entre
/// "meu jogo está estranho" e o arquivo anexado na conversa.
#[tauri::command]
pub fn iracing_log_revelar() -> Result<(), String> {
    let Some(path) = crate::diagnostico::caminho() else {
        return Err("O arquivo de log ainda não foi criado.".to_string());
    };
    #[cfg(windows)]
    {
        std::process::Command::new("explorer.exe")
            .arg(format!("/select,{}", path.display()))
            .spawn()
            .map_err(|e| format!("Falha ao abrir o Explorer: {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("Disponível apenas no Windows.".to_string())
    }
}
