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

/// Carisma INATO (0–100): magnetismo de estrela ancorado na personalidade + um
/// sorteio pessoal ±10. Base neutra 50. Showman/Apostador/Cabeça Quente puxam pra
/// cima (espetáculo, ousadia, drama — "vilão vende"); Calculista/Solitário/Estudioso/
/// Team Player/Sangue Frio puxam pra baixo (clínico, introvertido, discreto).
/// Ambicioso/Mercenário (primárias) dão um empurrão leve. Deriva de carreira ajusta
/// depois; isto é só o ponto de partida.
pub fn roll_carisma(
    primary: Option<&PrimaryPersonality>,
    secondary: Option<&SecondaryPersonality>,
    rng: &mut impl Rng,
) -> f64 {
    let mut base = 50.0_f64;
    base += match secondary {
        Some(SecondaryPersonality::Showman) => 25.0,
        Some(SecondaryPersonality::Apostador) => 12.0,
        Some(SecondaryPersonality::CabecaQuente) => 8.0,
        Some(SecondaryPersonality::SangueFrio) => -4.0,
        Some(SecondaryPersonality::Estudioso) => -6.0,
        Some(SecondaryPersonality::TeamPlayer) => -6.0,
        Some(SecondaryPersonality::Solitario) => -8.0,
        Some(SecondaryPersonality::Calculista) => -8.0,
        _ => 0.0,
    };
    base += match primary {
        Some(PrimaryPersonality::Ambicioso) => 6.0,
        Some(PrimaryPersonality::Mercenario) => 2.0,
        _ => 0.0,
    };
    base += rng.gen_range(-10.0..=10.0);
    base.clamp(0.0, 100.0)
}

/// Retorna o ano de início de carreira estimado a partir da idade atual.
/// Convenção: carreira começa aos 16 anos.
pub fn career_start_year_from_age(age: u32) -> u32 {
    current_year().saturating_sub(age.saturating_sub(16))
}
