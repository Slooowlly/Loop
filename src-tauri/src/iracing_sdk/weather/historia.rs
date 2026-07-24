//! Gerador da "história do clima" do fim de semana por pista + estação, com a
//! exceção roteirizada da 1ª corrida de todo save.

use serde::{Deserialize, Serialize};

use super::penalidade::RainIntensity;

/// Hemisfério da pista (define a estação a partir do mês).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hemisphere {
    North,
    South,
}

/// Estação do ano na pista.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Season {
    Winter,
    Spring,
    Summer,
    Autumn,
}

/// Tendência de chuva da pista (espelha o `rain_group` do TrackInfo).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClimateTendency {
    Dry,
    Normal,
    Rainy,
}

/// O cenário de clima sorteado para o fim de semana (vira keyframes no export).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherScenario {
    // ☀️ Secas (skill normal) — a maioria.
    ClearDry,        // sol/parcial o tempo todo
    Scare,           // começa limpo, céu fecha no meio, mas NÃO chove
    LastDrops,       // seco; pingos só nos últimos minutos
    PassingDrizzle,  // garoa de 2–3 min no meio, volta a secar
    ClearingUp,      // começa encoberto/ameaçando → vai limpando
    WetQualyDryRace, // choveu na quali; corrida seca
    // 🌧️ Molhadas (skill penalizado) — só com tendência alta.
    SteadyRain,          // chuva constante
    Improving,           // forte → afrouxa, mas nunca seca
    StormArrives,        // leve/decente → intensifica pro fim
    PulsingStorm,        // forte → afrouxa → forte
    LightQualyWorseRace, // garoa na quali, chuva pior na corrida
    // Roteiro fixo da 1ª corrida do save.
    FirstRaceScript, // limpo → nublado → encoberto → pingos na última volta
}

/// A história do clima decidida para uma etapa. Os keyframes da timeline são
/// montados no export a partir disto.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeatherStory {
    pub scenario: WeatherScenario,
    /// Se a CORRIDA é molhada (→ aplica a penalidade de skill, e fica molhada).
    pub is_wet_race: bool,
    /// Caráter geral da chuva da corrida (para a penalidade). None se seca.
    pub race_intensity: RainIntensity,
    /// Clima da QUALI (pode diferir da corrida).
    pub qualy_intensity: RainIntensity,
    pub season: Season,
    /// Tendência de chuva calculada (0–1), para diagnóstico/tuning.
    pub tendency: f64,
}

/// Estação a partir do mês (1–12) e do hemisfério.
pub fn season_for(month: u32, hemi: Hemisphere) -> Season {
    // Estação no hemisfério NORTE pelo mês; no SUL é o oposto.
    let north = match month {
        12 | 1 | 2 => Season::Winter,
        3..=5 => Season::Spring,
        6..=8 => Season::Summer,
        _ => Season::Autumn,
    };
    match hemi {
        Hemisphere::North => north,
        Hemisphere::South => match north {
            Season::Winter => Season::Summer,
            Season::Summer => Season::Winter,
            Season::Spring => Season::Autumn,
            Season::Autumn => Season::Spring,
        },
    }
}

/// Multiplicador sazonal da chance de chuva (inverno molhado, verão seco).
fn season_wetness(season: Season) -> f64 {
    match season {
        Season::Winter => 1.5,
        Season::Autumn => 1.15,
        Season::Spring => 1.0,
        Season::Summer => 0.5,
    }
}

/// Chance-base de corrida molhada por grupo de pista (referência primavera/neutra).
/// Diferente do modelo antigo (tendência × limiar): agora é probabilidade DIRETA, e
/// pistas Normal/Dry TAMBÉM podem molhar (raro), não só as Rainy.
fn group_base(group: ClimateTendency) -> f64 {
    match group {
        ClimateTendency::Dry => 0.04,
        ClimateTendency::Normal => 0.20,
        ClimateTendency::Rainy => 0.40,
    }
}

/// Probabilidade (0–1) de a CORRIDA ser molhada = base do grupo × multiplicador da
/// estação. (Mantém o nome histórico; hoje é a chance direta, sem limiar.)
pub fn rain_tendency(group: ClimateTendency, season: Season) -> f64 {
    (group_base(group) * season_wetness(season)).clamp(0.0, 1.0)
}

/// Tier de severidade da chuva de uma condição (grupo + estação): governa a
/// distribuição de intensidade quando molha. Só o tier ALTO permite temporal
/// (VeryHeavy); tiers menores ficam em Decent/Heavy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WetTier {
    High,
    Mid,
    Low,
}

fn wet_severity_tier(group: ClimateTendency, season: Season) -> WetTier {
    use ClimateTendency::*;
    use Season::*;
    match (group, season) {
        // Mais úmido: Rainy no inverno/outono → pode dar temporal.
        (Rainy, Winter) | (Rainy, Autumn) => WetTier::High,
        // Médio: Rainy quente, ou Normal frio.
        (Rainy, _) | (Normal, Winter) | (Normal, Autumn) => WetTier::Mid,
        // Marginal: Normal quente, ou qualquer Dry.
        _ => WetTier::Low,
    }
}

// PRNG determinístico (splitmix64) — mesmo seed → mesmo clima (estável entre
// re-exports da mesma corrida).
fn next_rand(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub(super) fn roll01(state: &mut u64) -> f64 {
    (next_rand(state) >> 11) as f64 / (1u64 << 53) as f64
}

/// Decide a história do clima de uma etapa (determinístico pelo `seed`).
pub fn generate_weather(
    month: u32,
    hemi: Hemisphere,
    group: ClimateTendency,
    seed: u64,
    is_first_race: bool,
) -> WeatherStory {
    let season = season_for(month, hemi);
    let tendency = rain_tendency(group, season);

    // 1ª corrida do save: roteiro fixo, seco com arco visível, zero penalidade.
    if is_first_race {
        return WeatherStory {
            scenario: WeatherScenario::FirstRaceScript,
            is_wet_race: false,
            race_intensity: RainIntensity::None,
            qualy_intensity: RainIntensity::None,
            season,
            tendency,
        };
    }

    let mut state = seed ^ 0xA5A5_5A5A_DEAD_BEEF;

    // Chance DIRETA de molhar (base do grupo × estação). Sem limiar: Normal/Dry também
    // podem molhar (raro), garantindo variação e ~2+ chuvas por temporada de 10.
    let p_wet = tendency;
    let is_wet_race = roll01(&mut state) < p_wet;

    if is_wet_race {
        // Intensidade pela SEVERIDADE da condição (grupo+estação). PISO = Decente: uma
        // corrida "molhada" NUNCA é só garoa (`Light`) — garoa deixa a pista no limiar
        // seco/molhado e o iRacing larga metade do grid de SLICK, quebrando a punição
        // (aplicada ao pelotão INTEIRO). Só o tier ALTO (Rainy inverno/outono) libera
        // temporal (VeryHeavy). Garoa segue só na QUALI e como trecho tardio de arco.
        let r = roll01(&mut state);
        let race_intensity = match wet_severity_tier(group, season) {
            // Alto: 40% Decent · 40% Heavy · 20% VeryHeavy (temporal raro).
            WetTier::High => {
                if r < 0.40 {
                    RainIntensity::Decent
                } else if r < 0.80 {
                    RainIntensity::Heavy
                } else {
                    RainIntensity::VeryHeavy
                }
            }
            // Médio: 50% Decent · 50% Heavy · sem temporal (só o tier ALTO libera VeryHeavy,
            // como diz a doc de `wet_severity_tier` e o invariante `temporal_so_no_tier_alto`).
            WetTier::Mid => {
                if r < 0.50 {
                    RainIntensity::Decent
                } else {
                    RainIntensity::Heavy
                }
            }
            // Baixo: 70% Decent · 30% Heavy · sem temporal.
            WetTier::Low => {
                if r < 0.70 {
                    RainIntensity::Decent
                } else {
                    RainIntensity::Heavy
                }
            }
        };
        let s = roll01(&mut state);
        let scenario = if s < 0.35 {
            WeatherScenario::SteadyRain
        } else if s < 0.60 {
            WeatherScenario::Improving
        } else if s < 0.80 {
            WeatherScenario::StormArrives
        } else if s < 0.92 {
            WeatherScenario::PulsingStorm
        } else {
            WeatherScenario::LightQualyWorseRace
        };
        let qualy_intensity = match scenario {
            WeatherScenario::LightQualyWorseRace => RainIntensity::Light,
            _ => RainIntensity::Light, // quali úmida quando a corrida é molhada
        };
        WeatherStory {
            scenario,
            is_wet_race: true,
            race_intensity,
            qualy_intensity,
            season,
            tendency,
        }
    } else {
        // SECA — sustos/pingos mais comuns quando há "motivo" pra nuvens (chance de
        // molhar razoável). p_wet ≥ 0.15 = Normal+ ou Rainy.
        let s = roll01(&mut state);
        let scenario = if tendency > 0.15 {
            if s < 0.30 {
                WeatherScenario::Scare
            } else if s < 0.55 {
                WeatherScenario::LastDrops
            } else if s < 0.70 {
                WeatherScenario::ClearingUp
            } else if s < 0.82 {
                WeatherScenario::PassingDrizzle
            } else if s < 0.92 {
                WeatherScenario::WetQualyDryRace
            } else {
                WeatherScenario::ClearDry
            }
        } else if s < 0.55 {
            WeatherScenario::ClearDry
        } else if s < 0.75 {
            WeatherScenario::LastDrops
        } else if s < 0.90 {
            WeatherScenario::Scare
        } else {
            WeatherScenario::PassingDrizzle
        };
        let qualy_intensity = match scenario {
            WeatherScenario::WetQualyDryRace => RainIntensity::Decent,
            _ => RainIntensity::None,
        };
        WeatherStory {
            scenario,
            is_wet_race: false,
            race_intensity: RainIntensity::None,
            qualy_intensity,
            season,
            tendency,
        }
    }
}
