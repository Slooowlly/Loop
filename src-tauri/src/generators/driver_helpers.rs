use rand::Rng;

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

/// Ano de início de carreira estimado a partir da idade, ancorado no ano do MUNDO.
/// Convenção: carreira começa aos 16 anos.
///
/// `world_year` é o ano da temporada que está sendo gerada, e não o do relógio da
/// máquina. A diferença não é cosmética: num mundo histórico que começa em 2000, o
/// `Local::now()` dava a um piloto de 30 anos uma carreira iniciada em 2012 — doze anos
/// no futuro do próprio mundo. E, pior, o MESMO save gerado em anos civis diferentes
/// saía com históricos diferentes, o que tira a reprodutibilidade de qualquer teste ou
/// comparação entre execuções.
pub fn career_start_year_from_age(world_year: u32, age: u32) -> u32 {
    world_year.saturating_sub(age.saturating_sub(16))
}

#[cfg(test)]
mod tests {
    use super::career_start_year_from_age;

    /// O ano de início sai do mundo, não do relógio — e um mundo histórico prova isso
    /// melhor que qualquer outro: em 2000, ninguém pode ter começado em 2012.
    #[test]
    fn o_inicio_de_carreira_sai_do_ano_do_mundo() {
        assert_eq!(career_start_year_from_age(2000, 30), 1986);
        assert_eq!(career_start_year_from_age(2026, 30), 2012);
        // Aos 16 a carreira começa no próprio ano; abaixo disso não retrocede.
        assert_eq!(career_start_year_from_age(2000, 16), 2000);
        assert_eq!(career_start_year_from_age(2000, 14), 2000);
    }
}
