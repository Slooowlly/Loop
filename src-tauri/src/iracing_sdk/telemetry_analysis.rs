//! Análise da TELEMETRIA da corrida (Fase 2 do pós-corrida): ritmo, consistência
//! e rival, a partir do histórico ao vivo do monitor (`RaceHistory`).
//!
//! Esses dados só existem ENQUANTO o jogador esteve na pista — se ele saiu cedo,
//! a análise vem parcial ou vazia. Tudo aqui é tolerante: campos `Option`/flags
//! para a tela "mostrar quando tem, esconder quando não tem". Nunca quebra.
//!
//! Lógica pura e testável; o chamador resolve `car_idx → nome` (via cars_meta +
//! roster) e passa o mapa.
//!
//! ```text
//! tipos        ← DTOs dos cards + o agregado TelemetryAnalysis
//! ritmo        ← ritmo/consistência, fluxo de posição, RIVAL + limiares comuns
//! momentos     ← erro mais caro + melhor momento (os cards narrativos)
//! graficos     ← séries dos gráficos (race trace, tempos, gap ao rival)
//! setores      ← parciais por setor do jogador
//! combustivel  ← consumo/autonomia do jogador
//! dossie       ← sinais compactos por corrida para o dossiê de habilidade
//! analise      ← analyze(): o orquestrador que cruza tudo
//! ```

mod analise;
mod combustivel;
mod dossie;
mod graficos;
mod momentos;
mod ritmo;
mod setores;
mod tipos;

pub use analise::analyze;
pub use dossie::{extract_player_race_telemetry, PlayerRaceTelemetry};
pub use tipos::*;

#[cfg(test)]
mod tests;
