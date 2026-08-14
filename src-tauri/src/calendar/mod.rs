//! Calendário da temporada — índice do módulo.
//!
//! A lógica vive nos submódulos irmãos:
//! - `entry`: a entidade `CalendarEntry`
//! - `janela`: janelas de temporada, semanas e datas visuais
//! - `selecao`: escolha e ordenação das pistas (pool temático, fixas, conflitos)
//! - `montagem`: montagem de cada etapa (horário, clima, duração, voltas)
//! - `geracao`: geração dos calendários por categoria e por fase

pub(crate) mod full_season;
mod generator;

mod entry;
mod geracao;
mod janela;
mod montagem;
mod selecao;

// Reexportados só para a suíte `tests`, que enxerga este módulo via `use super::*`
// e depende desses nomes desde quando a lógica morava aqui.
#[cfg(test)]
use chrono::{NaiveDate, Weekday};
#[cfg(test)]
use std::collections::HashSet;

#[cfg(test)]
use crate::models::enums::{SeasonPhase, WeatherCondition};

/// O teto de voltas por duração (B19) — fonte única, reexportada para quem monta etapa
/// sintética (harness de quebra) precisar contar as MESMAS voltas que o calendário conta.
pub(crate) use montagem::teto_de_voltas;

/// `estimate_laps` para quem monta etapa sintética em teste/harness. Existe porque `montagem`
/// é privado e a alternativa seria replicar a conta — que é justamente como o teto de 50 do
/// B19 ficou desalinhado entre o calendário e o harness de quebra.
#[cfg(test)]
pub(crate) fn estimate_laps_de_teste(
    track: &crate::constants::tracks::TrackInfo,
    duracao_corrida_min: i32,
) -> i32 {
    montagem::estimate_laps(track, duracao_corrida_min)
}

pub use entry::*;
pub use geracao::*;
pub use janela::*;
pub use montagem::*;
pub use selecao::*;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
