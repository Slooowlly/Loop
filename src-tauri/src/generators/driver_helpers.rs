use rand::Rng;

use crate::common::time::current_year;
use crate::models::enums::{PrimaryPersonality, SecondaryPersonality};

pub fn random_primary_personality(rng: &mut impl Rng) -> PrimaryPersonality {
    match rng.gen_range(0_u8..4_u8) {
        0 => PrimaryPersonality::Ambicioso,
        1 => PrimaryPersonality::Consolidador,
        2 => PrimaryPersonality::Mercenario,
        _ => PrimaryPersonality::Leal,
    }
}

pub fn random_secondary_personality(rng: &mut impl Rng) -> SecondaryPersonality {
    match rng.gen_range(0_u8..8_u8) {
        0 => SecondaryPersonality::CabecaQuente,
        1 => SecondaryPersonality::SangueFrio,
        2 => SecondaryPersonality::Apostador,
        3 => SecondaryPersonality::Calculista,
        4 => SecondaryPersonality::Showman,
        5 => SecondaryPersonality::TeamPlayer,
        6 => SecondaryPersonality::Solitario,
        _ => SecondaryPersonality::Estudioso,
    }
}

/// Retorna o ano de início de carreira estimado a partir da idade atual.
/// Convenção: carreira começa aos 16 anos.
pub fn career_start_year_from_age(age: u32) -> u32 {
    current_year().saturating_sub(age.saturating_sub(16))
}
