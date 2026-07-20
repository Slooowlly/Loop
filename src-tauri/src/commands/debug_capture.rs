//! Comandos do GRAVADOR DE CORRIDA (debug — só pra calibração local, fora do fluxo comercial).
//! Liga/desliga a captura da telemetria crua + YAML + histórico num arquivo JSONL, pra a gente
//! rebalancear o app com dados REAIS de pista. Ver [`crate::iracing_sdk::race_capture`].

use serde::Serialize;
use tauri::Manager;

use crate::iracing_sdk::{race_capture, race_monitor};

/// Pasta onde os arquivos de captura são gravados: `<app_data>/debug/race_captures`.
fn capture_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    Ok(base.join("debug").join("race_captures"))
}

/// Começa a gravar uma corrida. Devolve o caminho do arquivo criado. O sampler do monitor
/// passa a despejar telemetria (subamostrada) + o YAML da sessão nele até `race_capture_stop`.
#[tauri::command]
pub fn race_capture_start(app: tauri::AppHandle) -> Result<String, String> {
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
