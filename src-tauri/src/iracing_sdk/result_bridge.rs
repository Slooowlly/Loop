//! Ponte sessão iRacing → [`RaceResult`] da simulação.
//!
//! O app de carreira é, no fundo, uma máquina que consome [`RaceResult`]: a
//! persistência de standings/pontos, as lesões, o filtro de narrativa e as
//! notícias TODOS leem essa struct. A simulação offline a PRODUZ; este módulo a
//! produz a partir de uma corrida REAL disputada no iRacing — assim o jogador
//! corre na pista e a carreira reage exatamente como se o motor offline tivesse
//! rodado a etapa.
//!
//! Duas fontes alimentam a ponte (ambas do `race_monitor`):
//! - [`RaceHistory`](crate::iracing_sdk::race_monitor::RaceHistory) (`get_history`):
//!   posições finais (`cars_meta`), voltas de cada carro (`car_laps` → melhor
//!   volta), voltas da quali (`qualy_laps` → grid) e o gap ao líder (`laps`).
//! - [`RaceStatus`](crate::iracing_sdk::race_monitor::RaceStatus) (`poll`):
//!   `attempts` (DNF + batida do jogador) e `events` (DNF e severidade da IA).
//!
//! Escopo desta fatia: reconstruir o resultado para VALIDAÇÃO (preview read-only).
//! A persistência na carreira, as lesões a partir da severidade e o conserto do
//! carro entram numa fatia seguinte — mas já consomem o [`RaceResult`] daqui.
//!
//! Organização dos submódulos:
//! - [`identidade`] — quem é cada carro na carreira (piloto + equipe).
//! - [`agregacao`] — agregações do histórico/eventos (melhor volta, grid, DNFs).
//! - [`sessao`] — resultado a partir da SESSÃO ao vivo.
//! - [`oficial`] — resultado a partir do JSON OFICIAL do aiseason.

mod agregacao;
mod identidade;
mod oficial;
mod sessao;

pub use agregacao::*;
pub use oficial::*;
pub use sessao::*;

#[cfg(test)]
mod tests;
