//! Dados AO VIVO para a torre de tempos (overlay do iRacing).
//!
//! Cruza três mundos:
//!   • PISTA (SDK): posição na classe, pit, melhor volta, pneus — `read_telemetry`
//!     + `race_monitor` (identidade CarIdx→número/classe) + `tire_strategy`.
//!   • CARREIRA (save): nome do piloto, time, cor do time, pontos de campeonato.
//!   • SESSÃO: tipo, voltas, bandeira, categoria, clima.
//!
//! A ponte CarIdx→nosso piloto é o NÚMERO do carro (nós geramos o roster). Mesma
//! resolução de `build_session_race_result`/`iracing_car_colors`, mas ao vivo.
//!
//! Devolve `None` quando não há sessão ativa no iRacing (overlay fica oculto).
//!
//! Este arquivo é só a FACHADA: cada área virou um submódulo em `overlay/`, e o
//! `pub use` abaixo mantém TODOS os caminhos públicos idênticos
//! (`commands::overlay::<nome>`), que é o que o `generate_handler!` do
//! [`crate::lib`] espera.

#[path = "overlay/avisos.rs"]
mod avisos;
#[path = "overlay/demo.rs"]
mod demo;
#[path = "overlay/formato.rs"]
mod formato;
#[path = "overlay/radio.rs"]
mod radio;
#[path = "overlay/tipos.rs"]
mod tipos;
#[path = "overlay/torre.rs"]
mod torre;

pub use avisos::*;
pub use demo::*;
pub use radio::*;
pub use tipos::*;
pub use torre::*;

#[cfg(test)]
#[path = "overlay/tests/mod.rs"]
mod tests;
