//! Horário de largada da corrida (golden hour, noite, sem meio-dia).

use super::historia::{roll01, Season};

/// Horários do sol (aproximados, latitude média) por estação: (nascer, pôr) em h.
pub(super) fn sun_times(season: Season) -> (f64, f64) {
    match season {
        Season::Summer => (5.5, 20.5),
        Season::Spring | Season::Autumn => (7.0, 18.5),
        Season::Winter => (8.0, 17.0),
    }
}

/// Decide a HORA de largada da corrida (0–24; ex.: 18.25 = 18h15), amarrada à
/// estação (golden hour de verdade) e determinística pelo `seed`.
///
/// Regras (decisão do user): nunca 11–14h (sombras chapadas); **rookie (tier 0)
/// nunca de noite**; **Charlotte** (única com iluminação, `is_lit_track`) corre de
/// noite **80%**; demais pistas ~**1 a cada 9** (a noite do Charlotte não desconta).
pub fn generate_race_start_hour(season: Season, tier: u8, is_lit_track: bool, seed: u64) -> f64 {
    let (sr, ss) = sun_times(season);
    let mut state = seed ^ 0x7A6F_4B1D_2E3C_9F08;

    let night = if tier == 0 {
        false
    } else if is_lit_track {
        roll01(&mut state) < 0.80
    } else {
        roll01(&mut state) < 0.11
    };

    let window = |state: &mut u64, a: f64, b: f64| a + roll01(state) * (b - a);

    if night {
        // Depois do escuro (pôr do sol + 1h) até ~22h30.
        let a = ss + 1.0;
        let b = 22.5_f64.max(a + 0.5);
        return window(&mut state, a, b);
    }

    // De dia: golden tarde (alto) · tarde · golden manhã · manhã. Sem meio-dia.
    let pick = roll01(&mut state);
    if pick < 0.45 {
        window(&mut state, ss - 1.5, ss - 0.5) // golden hour da tarde
    } else if pick < 0.65 {
        let a = 14.5_f64.max(sr + 1.5);
        window(&mut state, a, (ss - 1.6).max(a + 0.2)) // tarde (pós meio-dia)
    } else if pick < 0.85 {
        window(&mut state, sr + 0.25, sr + 1.25) // golden hour da manhã
    } else {
        let a = sr + 1.5;
        window(&mut state, a, 10.75_f64.max(a + 0.2)) // manhã (pré meio-dia)
    }
}

/// Hora de largada NOTURNA garantida (após o escuro), determinística pelo `seed`.
/// Usada quando o calendário DESIGNA uma etapa como noturna (regra: ao menos 1
/// corrida de noite por temporada, nunca a 1ª/última). Reusa exatamente a mesma
/// janela do ramo `night` de [`generate_race_start_hour`].
pub fn night_start_hour(season: Season, seed: u64) -> f64 {
    let (_, ss) = sun_times(season);
    let mut state = seed ^ 0x7A6F_4B1D_2E3C_9F08;
    let a = ss + 1.0; // pôr do sol + 1h (depois do escuro)
    let b = 22.5_f64.max(a + 0.5); // até ~22h30
    a + roll01(&mut state) * (b - a)
}
