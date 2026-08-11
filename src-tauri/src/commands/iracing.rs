//! Comandos Tauri do teste do SDK do iRacing (ver [`crate::iracing_sdk`]).
//!
//! Este arquivo é só a FACHADA: cada área virou um submódulo em `iracing/`, e o
//! `pub use` abaixo mantém TODOS os caminhos públicos idênticos
//! (`commands::iracing::<nome>`), que é o que o `generate_handler!` do
//! [`crate::lib`] espera.

use crate::iracing_sdk::race_control::{self, YellowMacroStatus};
use crate::iracing_sdk::race_monitor::{self, EstadoAgora, RaceHistory, RaceStatus};
use crate::iracing_sdk::{self, IracingSession, IracingTelemetry};

#[path = "iracing/adaptativo.rs"]
mod adaptativo;
#[path = "iracing/clima.rs"]
mod clima;
#[path = "iracing/controle_corrida.rs"]
mod controle_corrida;
#[path = "iracing/corridas_salvas.rs"]
mod corridas_salvas;
#[path = "iracing/diagnostico.rs"]
mod diagnostico;
#[path = "iracing/importacao.rs"]
mod importacao;
#[path = "iracing/modo_janela.rs"]
mod modo_janela;
#[path = "iracing/pintura.rs"]
mod pintura;
#[path = "iracing/previsao_quebras.rs"]
mod previsao_quebras;
#[path = "iracing/resultado.rs"]
mod resultado;
#[path = "iracing/roster.rs"]
mod roster;
#[path = "iracing/sessao.rs"]
mod sessao;
#[path = "iracing/spotter.rs"]
mod spotter;
#[path = "iracing/temporada.rs"]
mod temporada;
#[path = "iracing/teste_chuva.rs"]
mod teste_chuva;

pub use adaptativo::*;
pub use clima::*;
pub use controle_corrida::*;
pub use corridas_salvas::*;
pub use diagnostico::*;
pub use importacao::*;
pub use modo_janela::*;
pub use pintura::*;
pub use previsao_quebras::*;
pub use resultado::*;
pub use roster::*;
pub use sessao::*;
pub use spotter::*;
pub use temporada::*;
pub use teste_chuva::*;

#[cfg(test)]
mod tests;
