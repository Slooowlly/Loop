//! Stub fora do Windows: tudo devolve [`IracingError::Unsupported`], para a lib
//! continuar compilando em qualquer SO.

use crate::iracing_sdk::{IracingError, IracingSession, IracingTelemetry};

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
