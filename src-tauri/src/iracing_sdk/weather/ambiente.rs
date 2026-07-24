//! Temperatura e vento da etapa — ambos DERIVADOS da mesma história de clima.

use serde::{Deserialize, Serialize};

use super::historia::{roll01, Season, WeatherStory};
use super::penalidade::RainIntensity;

// ─── Temperatura da etapa (ALINHADA à história de chuva) ────────────────────

/// Temperatura do ar da corrida (°C), DERIVADA da MESMA história de clima — assim
/// ela SEMPRE bate com a chuva real (nunca uma temperatura "de chuva" numa corrida
/// que roda seca). Base pela estação, resfria com a intensidade da chuva, com um
/// jitter determinístico pelo `seed`. Presa em **[18, 32]** (faixa observada no
/// iRacing: máx 32, mín 18).
pub fn story_temperature(story: &WeatherStory, seed: u64) -> i64 {
    let base = match story.season {
        Season::Summer => 30.0,
        Season::Spring | Season::Autumn => 24.0,
        Season::Winter => 20.0,
    };
    // Chuva esfria o ar (quanto mais forte, mais frio).
    let rain_cool = match story.race_intensity {
        RainIntensity::None => 0.0,
        RainIntensity::Light => 2.0,
        RainIntensity::Decent => 4.0,
        RainIntensity::Heavy => 6.0,
        RainIntensity::VeryHeavy => 8.0,
    };
    let mut state = seed ^ 0x7EE0_17A1_9C0F_1234;
    let jitter = (roll01(&mut state) - 0.5) * 6.0; // ±3 °C
    (base - rain_cool + jitter).round().clamp(18.0, 32.0) as i64
}

// ─── Vento da etapa (VARIA entre corridas) ──────────────────────────────────

/// Escada de vento MEDIDA no iRacing: cada `wind_speed_option` (preset) rende uma
/// velocidade fixa — e o mapa NÃO é monotônico na ordem do preset. Aqui em ordem
/// CRESCENTE de km/h: `(km/h aproximado, wind_speed_option)`. O sim IGNORA o
/// `wind_value` contínuo; só o preset vale. Medido in-sim (todos os 7 presets):
/// 1≈2 · 2≈2 · 3≈10 · 0≈13 · 5≈21 · 6≈30 · 4≈48 km/h. Preset 1 descartado (duplica
/// o 2). 6 degraus úteis cobrindo 2–48.
pub const WIND_LADDER: [(i64, i64); 6] =
    [(2, 2), (10, 3), (13, 0), (21, 5), (30, 6), (48, 4)];

/// Vento sorteado para a corrida: velocidade em km/h (um dos degraus da `WIND_LADDER`,
/// 2–48) + direção em graus **[0, 359]**. Um valor por corrida (não muda no meio).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindState {
    pub speed_kmh: i64,
    pub dir_deg: i64,
}

/// Sorteia o vento da etapa (determinístico pelo `seed`). Como o iRacing só honra o
/// PRESET (5 degraus reais), escolhe um degrau da `WIND_LADDER` — tempestade puxa
/// pros degraus altos, seco/leve espalha do calmo ao forte com viés baixo. Testado
/// na pista: o vento **não** influencia a chegada da frente de chuva.
pub fn generate_wind(story: &WeatherStory, seed: u64) -> WindState {
    let mut state = seed ^ 0x011D_C0FF_EE15_600D;
    let r = roll01(&mut state);
    // Índice na escada crescente [calmo(0) 2 · 10 · 13 · 21 · 30 · forte(5) 48].
    let idx = match story.race_intensity {
        // Temporal: só forte/muito forte (degraus 30 e 48).
        RainIntensity::VeryHeavy => {
            if r < 0.5 {
                5
            } else {
                4
            }
        }
        // Chuva forte: médio a forte (21/30/48).
        RainIntensity::Heavy => {
            if r < 0.45 {
                3
            } else if r < 0.8 {
                4
            } else {
                5
            }
        }
        // Seco/leve: espalha do calmo ao forte, viés pro leve.
        _ => {
            if r < 0.28 {
                0
            } else if r < 0.50 {
                1
            } else if r < 0.68 {
                2
            } else if r < 0.83 {
                3
            } else if r < 0.94 {
                4
            } else {
                5
            }
        }
    };
    let speed = WIND_LADDER[idx].0;
    let dir = roll01(&mut state) * 360.0;
    WindState {
        speed_kmh: speed,
        dir_deg: (dir.floor() as i64).clamp(0, 359),
    }
}
