//! Implementação por SO. O binding real só existe em Windows; em outros alvos
//! entra um stub que devolve [`super::IracingError::Unsupported`], para a lib
//! continuar compilando em qualquer SO.

#[cfg(windows)]
mod chat;
#[cfg(windows)]
mod janela;
#[cfg(windows)]
mod leitura;
#[cfg(not(windows))]
mod stub;
#[cfg(windows)]
mod util;

#[cfg(windows)]
pub use chat::{send_chat_macro, send_chat_text};
#[cfg(windows)]
pub use janela::{focus_iracing_window, force_foreground_window, foreground_is_iracing};
#[cfg(windows)]
pub use leitura::{read_session, read_telemetry};

#[cfg(not(windows))]
pub use stub::*;
