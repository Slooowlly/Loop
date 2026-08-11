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
    let diag = serde_json::to_value(iracing_sdk::diagnosticar()).unwrap_or(serde_json::Value::Null);
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

// ── Registro do RÁDIO ───────────────────────────────────────────────────────
//
// Canal separado do `loop.log`, e não uma categoria dentro dele. O log de diagnóstico é
// pequeno, rotativo e feito para caber num anexo; o registro do rádio é uma linha por fala e
// existe para ser LIDO por script. Misturar os dois faria o log encher de rádio e perder o
// que ele existe para mostrar. Ver [`crate::radio_registro`].

/// **Registra uma fala que acabou de tocar** (ou que foi engolida no caminho).
///
/// Quem chama é o front, no instante em que o áudio começa — é a única ponta que sabe o que o
/// jogador de fato ouviu, e qual variação do rodízio saiu. O tempo de sessão é anexado aqui,
/// do último tique do amostrador, e não vem do JavaScript: o front não tem relógio do sim, e
/// mandá-lo daqui é o que faz as duas fases da linha do tempo serem comparáveis.
///
/// Nunca falha. Um registro que devolve erro faria o `catch` do front virar ruído no console
/// no meio de uma corrida, e o áudio não pode pagar nada por causa da medição.
#[tauri::command]
pub fn radio_registrar(registro: crate::radio_registro::Registro) {
    crate::radio_registro::registrar(&registro);
}

/// Caminho do registro desta execução. `None` até a primeira fala.
#[tauri::command]
pub fn radio_log_caminho() -> Option<String> {
    crate::radio_registro::caminho().map(|p| p.to_string_lossy().to_string())
}

/// Abre o Explorer na pasta dos registros, com o arquivo desta execução selecionado quando ele
/// já existe. Sem fala nenhuma ainda, abre a pasta — que é onde estão os das sessões anteriores.
#[tauri::command]
pub fn radio_log_revelar() -> Result<(), String> {
    let alvo = crate::radio_registro::caminho();
    let pasta = crate::radio_registro::pasta();
    #[cfg(windows)]
    {
        let arg = match (&alvo, &pasta) {
            (Some(p), _) => format!("/select,{}", p.display()),
            (None, Some(d)) => d.display().to_string(),
            (None, None) => return Err("A pasta de registros do rádio não foi criada.".to_string()),
        };
        std::process::Command::new("explorer.exe")
            .arg(arg)
            .spawn()
            .map_err(|e| format!("Falha ao abrir o Explorer: {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (alvo, pasta);
        Err("Disponível apenas no Windows.".to_string())
    }
}
