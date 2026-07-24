//! Perfil de simulação canônico de uma corrida.
//!
//! Fachada do módulo: mantém os caminhos públicos históricos
//! (`SimulationProfile` e `resolve_simulation_profile`) enquanto o conteúdo
//! vive nos submódulos de `profile/`:
//!
//! | submódulo | conteúdo |
//! |---|---|
//! | `tipos` | struct `SimulationProfile` |
//! | `base` | perfis base por categoria e família de carro |
//! | `lap_times` | tabela de tempos base por (família, track_id) |
//! | `pista` | dificuldade de pista e de ultrapassagem |
//! | `resolucao` | `resolve_simulation_profile` (função principal) |

mod base;
mod lap_times;
mod pista;
mod resolucao;
mod tipos;

#[cfg(test)]
mod tests;

pub use resolucao::*;
pub use tipos::*;
