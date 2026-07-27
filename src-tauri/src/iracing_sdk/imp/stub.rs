//! Stub fora do Windows: tudo devolve [`IracingError::Unsupported`], para a lib
//! continuar compilando em qualquer SO.

use crate::iracing_sdk::{
    DiagnosticoIracing, IracingError, IracingSession, IracingTelemetry, Veredito,
};

/// Fora do Windows não há SDK — o diagnóstico diz isso sem fingir que mediu algo.
pub fn diagnosticar() -> DiagnosticoIracing {
    DiagnosticoIracing {
        veredito: Veredito::NaoSuportado,
        memoria_ok: false,
        memoria_nome: None,
        memoria_erro: 0,
        janela_encontrada: false,
        janela_simulador: false,
        status: None,
        num_vars: None,
        session_info_len: None,
        elevado: false,
        ticks_observados: crate::iracing_sdk::ticks_observados(),
        log_caminho: crate::diagnostico::caminho().map(|p| p.to_string_lossy().to_string()),
    }
}

pub fn read_session() -> Result<IracingSession, IracingError> {
    Err(IracingError::Unsupported)
}

pub fn read_telemetry() -> Result<IracingTelemetry, IracingError> {
    Err(IracingError::Unsupported)
}

pub fn send_chat_macro(_macro_num: i32) -> Result<(), IracingError> {
    Err(IracingError::Unsupported)
}

pub fn send_chat_text(_text: &str) -> Result<(), IracingError> {
    Err(IracingError::Unsupported)
}

pub fn focus_iracing_window() -> Result<bool, IracingError> {
    Err(IracingError::Unsupported)
}

pub fn force_foreground_window(_hwnd_raw: isize) {}

pub fn foreground_is_iracing() -> bool {
    false
}
