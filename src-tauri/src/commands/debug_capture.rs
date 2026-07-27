//! Comandos do GRAVADOR DE CORRIDA (debug — só pra calibração local, fora do fluxo comercial).
//! Liga/desliga a captura da telemetria crua + YAML + histórico num arquivo JSONL, pra a gente
//! rebalancear o app com dados REAIS de pista. Ver [`crate::iracing_sdk::race_capture`].

use serde::Serialize;
use tauri::Manager;

use crate::iracing_sdk::{race_capture, race_monitor};

/// Pasta onde os arquivos de captura são gravados: `<app_data>/debug/race_captures`.
///
/// Fonte única é o `race_capture::init` do boot — se esta função montasse o caminho por
/// conta própria e os dois divergissem, a rotação limparia uma pasta e o botão de debug
/// gravaria noutra. O `AppHandle` só entra como saída de emergência, se o boot não passou
/// pelo `init`.
fn capture_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    if let Some(dir) = race_capture::pasta() {
        return Ok(dir.clone());
    }
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    Ok(base.join("debug").join("race_captures"))
}

/// Começa a gravar uma corrida. Devolve o caminho do arquivo criado.
///
/// A gravação hoje é AUTOMÁTICA (o sampler abre na borda de conexão do sim), então isto só
/// serve pra forçar um arquivo novo com o sim fechado. Se já há uma captura em curso, devolve
/// o caminho dela em vez de abrir outra: chamar `start` por cima descartaria a corrida que
/// está sendo gravada — sem o bloco `history`, que só é anexado no `stop`.
#[tauri::command]
pub fn race_capture_start(app: tauri::AppHandle) -> Result<String, String> {
    if let Some(atual) = race_capture::caminho_atual() {
        return Ok(atual.display().to_string());
    }
    let dir = capture_dir(&app)?;
    let path = race_capture::start(dir)?;
    Ok(path.display().to_string())
}

/// Para a gravação: anexa um resumo do HISTÓRICO (voltas, paradas, posições, clima, incidentes)
/// e fecha o arquivo. Devolve o caminho salvo (ou None se não havia gravação).
#[tauri::command]
pub fn race_capture_stop() -> Result<Option<String>, String> {
    // Resumo dos dados DERIVADOS acumulados na corrida, antes de fechar.
    let hist = race_monitor::get_history();
    if let Ok(v) = serde_json::to_value(&hist) {
        race_capture::record_block("history", v);
    }
    Ok(race_capture::stop().map(|p| p.display().to_string()))
}

/// Estado da captura: se está ativa, quantos frames já foram gravados e a pasta de saída.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatus {
    active: bool,
    frames: u64,
    dir: String,
}

#[tauri::command]
pub fn race_capture_status(app: tauri::AppHandle) -> CaptureStatus {
    CaptureStatus {
        active: race_capture::is_active(),
        frames: race_capture::frame_count(),
        dir: capture_dir(&app)
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    }
}
