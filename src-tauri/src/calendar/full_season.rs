//! Gerador de calendário unificado — Modelo 9D.
//!
//! Distribui as 9 divisões da carreira na régua season_week (1–51),
//! inteiramente dentro da janela de corridas sw 10–51 (woy 6–47, fev–nov).
//!
//! Regras garantidas por construção:
//! - Pares conflitantes nunca compartilham a mesma season_week.
//! - Abertura de cada divisão: sw 10–13.
//! - Final escalonado por prestígio: rookie/cup/bmw terminam antes; o topo
//!   (production/gt4/endurance/gt3) fecha o fim de novembro em sw 48–51.
//! - Finais de gt3 (sw 51) e endurance (sw 50) em semanas distintas.
//! - ThematicSlot somente do grupo regular (nunca *Especial).
//! - Todas as entradas com season_phase = Temporada.
//!
//! Submódulos:
//! - [`completo`] — gerador 9D completo (74 entradas) + persistência.
//! - [`parcial`] — gerador parcial da Etapa 10 / migração v34 (production + endurance).

mod completo;
mod parcial;

#[cfg(test)]
mod tests;

// Re-exports para manter idênticos todos os caminhos públicos do módulo.
pub use completo::*;
pub(crate) use parcial::*;
