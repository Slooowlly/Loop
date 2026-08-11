//! Motor puro do leilão da Janela de Transferências (Fase 1, IA-only, sem DB).
//!
//! Roda o ciclo semanal de dois lados: OFERTAS (equipes ofertam à shortlist, sobem
//! o lance se perderam o alvo) → RESPOSTAS (piloto aceita só a melhor acima do
//! limiar) → RESULTADOS (equipe assina o nº1 entre os que aceitaram; resto = "resto")
//! → ROLLOVER. Fecha quando uma semana passa sem assinatura (ou teto duro).
//!
//! Pesos e regras seguem o design `docs/superpowers/specs/2026-06-20-weekly-transfer-market-design.md`.
//! O wiring (ler vagas/pilotos do DB, prestígio do archive, garantias do pódio e
//! entourage do jogador) é uma camada de cima — aqui é só o leilão puro.
//!
//! Fachada: os submódulos abaixo carregam a implementação e são re-exportados aqui,
//! então todos os caminhos públicos (`market::transfer_window::*`) seguem idênticos.
//!
//! - [`tipos`] — DTOs (`Seat`, `Candidate`, `Signing`, `WindowConfig`, `PlayerOffer`…)
//! - [`pontuacao`] — scores, dignidade, elegibilidade e regra de marca
//! - [`leilao`] — ofertas da semana e resolução (respostas + resultados)
//! - [`fechamento`] — passe de clearing e rede de segurança dos craques
//! - [`estado`] — `WindowState` stepável e `run_window`

#![allow(dead_code)] // wiring vem na próxima camada

mod estado;
mod fechamento;
mod leilao;
mod pontuacao;
mod tipos;

pub use estado::{run_window, WindowState};
pub use tipos::{
    Candidate, PlayerOffer, Seat, Signing, WindowConfig, SEAT_W_CAR, SEAT_W_PRESTIGE,
};

#[cfg(test)]
mod tests;
