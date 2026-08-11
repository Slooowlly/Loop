//! Cérebro de manutenção do carro por corrida (Sistema de Nível do Carro).
//!
//! Decide, para cada time e a cada corrida, o que fazer com as peças do carro
//! (trocar / esticar / degradar) e quando subir de nível — dentro do caixa e olhando o
//! calendário à frente. O jogador NÃO participa; seu time roda no mesmo cérebro.
//!
//! O tick pós-corrida JÁ está ligado (`commands/race/despesa.rs`,
//! `commands/career/lifecycle.rs`, `commands/iracing/roster.rs`) e o legado
//! `car_build_strategy` (perfil discreto) não existe mais no crate. Ver design §7 em
//! `docs/superpowers/specs/2026-07-17-car-level-system-design.md`.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::car::cost::{category_ceiling, upgrade_cost};
use crate::car::seed::seed_car;
use crate::car::wear::{
    advance_race, advance_race_scaled, can_stretch, replace_cost, stretch_cost, wear_per_race,
    PartAction,
};
use crate::car::{Car, CarPart, PartType};
use crate::db::connection::DbError;
use crate::db::queries::team_car;
use crate::finance::planning::calculate_financial_plan;
use crate::models::team::Team;
use crate::simulation::track_profile::get_track_simulation_data;
// Cada área do cérebro de manutenção mora no seu módulo e enxerga os imports
// acima via `use super::*`.
mod dna;
mod horizonte;
mod plano;
mod semeadura;
mod tick_corrida;

// Os caminhos públicos continuam saindo por `market::car_maintenance::*`.
pub use dna::*;
pub use horizonte::*;
pub use plano::*;
pub use semeadura::*;
pub use tick_corrida::*;

#[cfg(test)]
mod tests;
