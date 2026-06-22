//! Evolução do clima na carreira → keyframes da timeline dinâmica do iRacing.
//! **Lógica pura, testável.** Duas peças:
//!
//! 1. [`rain_skill_penalty`] — quanto de skill a IA PERDE numa corrida molhada.
//!    Insight do design (user): na chuva NÃO se sobe a IA, BAIXA-SE o pelotão todo
//!    (chuva no iRacing é punitiva; subir a IA faria o humano forçar/rodar e ter
//!    medo da chuva). Quem é bom na chuva (`fator_chuva` alto) perde menos. Como o
//!    skill da IA é FIXO na corrida, uma corrida molhada fica molhada o tempo todo
//!    (senão o trecho seco entregaria que baixamos o nível) — mas a INTENSIDADE
//!    pode variar (forte→afrouxa→forte) pra dar dinamismo.
//!
//! 2. (a seguir) gerador da "história do clima" do fim de semana por pista+estação,
//!    com a exceção roteirizada da 1ª corrida de todo save.

use serde::{Deserialize, Serialize};

/// Intensidade da chuva na corrida (caráter geral, mesmo que varie no tempo).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RainIntensity {
    /// Seco — sem penalidade.
    None,
    /// Garoa/leve.
    Light,
    /// Chuva "decente" (a referência de 30→8).
    Decent,
    /// Chuva forte.
    Heavy,
    /// Chuva MUITO forte — penaliza até os bons (40→14).
    VeryHeavy,
}

impl RainIntensity {
    /// Âncoras `(fator_chuva, penalidade)` que definem a curva da penalidade, em
    /// ordem de fator. Interpolação LINEAR POR PARTES entre elas (assim cravamos os
    /// números que o user deu, mesmo quando não caem numa reta). Os de chuva forte
    /// têm um joelho em fator 90 (o "ás da chuva" só vira resiliente lá no topo).
    fn anchors(self) -> &'static [(f64, f64)] {
        match self {
            RainIntensity::None => &[(0.0, 0.0), (100.0, 0.0)],
            RainIntensity::Light => &[(0.0, 18.0), (100.0, 5.0)],
            // Decente: 0→30, 90→10, 100→8 (reta — os três são colineares).
            RainIntensity::Decent => &[(0.0, 30.0), (100.0, 8.0)],
            // Forte: intermediária (interpolada — user só fixou decente e muito forte).
            RainIntensity::Heavy => &[(0.0, 35.0), (90.0, 15.0), (100.0, 11.0)],
            // Muito forte: 0→40, 90→20, 100→14 (pontos exatos do user; curva).
            RainIntensity::VeryHeavy => &[(0.0, 40.0), (90.0, 20.0), (100.0, 14.0)],
        }
    }
}

/// Interpolação linear por partes de `y` num conjunto de âncoras `(x, y)` ordenado.
fn interp(anchors: &[(f64, f64)], x: f64) -> f64 {
    if x <= anchors[0].0 {
        return anchors[0].1;
    }
    for w in anchors.windows(2) {
        let (x0, y0) = w[0];
        let (x1, y1) = w[1];
        if x <= x1 {
            return y0 + (y1 - y0) * (x - x0) / (x1 - x0);
        }
    }
    anchors[anchors.len() - 1].1
}

/// Penalidade de skill (pontos a SUBTRAIR) de um piloto numa corrida molhada, dada
/// a habilidade dele na chuva (`fator_chuva` 0–100) e a intensidade. Interpola entre
/// as âncoras da intensidade. Seco → 0.
pub fn rain_skill_penalty(fator_chuva: f64, intensity: RainIntensity) -> i64 {
    let f = fator_chuva.clamp(0.0, 100.0);
    interp(intensity.anchors(), f).round() as i64
}

// ─── Gerador da "história do clima" do fim de semana ────────────────────────

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

/// Abaixo deste limiar de tendência, a chuva só vira "susto" — nunca molha de
/// verdade (bias forte pra seco; chuva no iRacing é punitiva).
const WET_THRESHOLD: f64 = 0.45;

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

fn season_wetness(season: Season) -> f64 {
    match season {
        Season::Winter => 1.35,
        Season::Autumn => 1.15,
        Season::Spring => 1.0,
        Season::Summer => 0.65,
    }
}

fn group_base(group: ClimateTendency) -> f64 {
    match group {
        ClimateTendency::Dry => 0.12,
        ClimateTendency::Normal => 0.30,
        ClimateTendency::Rainy => 0.55,
    }
}

/// Tendência de chuva (0–1) = base da pista × modificador da estação.
pub fn rain_tendency(group: ClimateTendency, season: Season) -> f64 {
    (group_base(group) * season_wetness(season)).clamp(0.0, 1.0)
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

fn roll01(state: &mut u64) -> f64 {
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

    // Só molha de verdade acima do limiar; a chance cresce com a tendência.
    let p_wet = if tendency < WET_THRESHOLD {
        0.0
    } else {
        (tendency - WET_THRESHOLD) / (1.0 - WET_THRESHOLD)
    };
    let is_wet_race = roll01(&mut state) < p_wet;

    if is_wet_race {
        // Intensidade cresce com a tendência.
        let r = roll01(&mut state);
        let race_intensity = if tendency > 0.8 {
            if r < 0.5 {
                RainIntensity::VeryHeavy
            } else {
                RainIntensity::Heavy
            }
        } else if tendency > 0.6 {
            if r < 0.5 {
                RainIntensity::Heavy
            } else {
                RainIntensity::Decent
            }
        } else if r < 0.5 {
            RainIntensity::Decent
        } else {
            RainIntensity::Light
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
        // SECA — sustos/pingos mais comuns quando há "motivo" pra nuvens.
        let s = roll01(&mut state);
        let scenario = if tendency > 0.30 {
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

// ─── Keyframes (cenário → timeline do iRacing) ──────────────────────────────

/// Perfil de clima pronto pra virar o bloco do iRacing: céu/umidade/água + os
/// keyframes `(event_type, time_offset)` (offset em min relativo ao início; quali
/// em offsets negativos, corrida de 0 a `race_end`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeatherProfile {
    pub skies: i64,
    pub humidity: i64,
    pub track_water: i64,
    pub keyframes: Vec<(i64, i64)>,
}

/// event_type do iRacing por intensidade de chuva (0 limpo … 6 leve, 7 chuva, 8 intensa).
fn intensity_event_type(i: RainIntensity) -> i64 {
    match i {
        RainIntensity::None => 0,
        RainIntensity::Light => 6,
        RainIntensity::Decent => 7,
        RainIntensity::Heavy => 7,
        RainIntensity::VeryHeavy => 8,
    }
}

/// track_water (0 seco … 5 encharcado) por intensidade.
fn intensity_water(i: RainIntensity) -> i64 {
    match i {
        RainIntensity::None => 0,
        RainIntensity::Light => 2,
        RainIntensity::Decent => 3,
        RainIntensity::Heavy => 4,
        RainIntensity::VeryHeavy => 5,
    }
}

/// Constrói o perfil de keyframes a partir do cenário sorteado e da duração da
/// corrida (min). É a "história" virando a timeline.
pub fn story_to_profile(story: &WeatherStory, race_end_min: i64) -> WeatherProfile {
    let r = race_end_min.max(1);
    // LINHA DO TEMPO: âncora QUALI em -90 (mantém a renderização certa, como no export
    // que funcionou), depois clima na largada (@0), e o arco da CORRIDA em offsets
    // absolutos via at(f) = RACE_START + fração × duração (a corrida começa ~RACE_START
    // min depois do início da timeline; antes disso é prática/quali).
    const QUALI: i64 = -90;
    // CALIBRADO (user): corrida começa às 4:20 e simulated_start = 3:45 → offset 35.
    // (Offset 25 caía na QUALY, não na corrida.) ~prática 5 + quali 10 + grid 20.
    const RACE_START: i64 = 35;
    let at = |f: f64| RACE_START + (r as f64 * f).round() as i64;
    let rend = at(1.0);
    let iw = intensity_water(story.race_intensity);
    let iet = intensity_event_type(story.race_intensity);
    use WeatherScenario::*;
    let (skies, humidity, water, kf): (i64, i64, i64, Vec<(i64, i64)>) = match story.scenario {
        ClearDry => (1, 45, 0, vec![(1, QUALI), (1, 0), (0, at(0.4)), (1, rend)]),
        // O susto: céu FECHA durante a corrida (encoberto) e fica, mas NÃO chove.
        Scare => (
            2,
            55,
            0,
            vec![
                (1, QUALI),
                (1, 0),
                (1, at(0.0)),
                (2, at(0.4)),
                (3, at(0.75)),
                (3, rend),
            ],
        ),
        // Pingos só no finzinho da corrida.
        LastDrops => (
            1,
            50,
            1,
            vec![(1, QUALI), (1, 0), (1, at(0.6)), (6, at(0.92)), (6, rend)],
        ),
        // Garoa passageira no meio da corrida, volta a abrir.
        PassingDrizzle => (
            2,
            55,
            1,
            vec![
                (1, QUALI),
                (1, 0),
                (1, at(0.3)),
                (6, at(0.45)),
                (2, at(0.6)),
                (0, rend),
            ],
        ),
        // Começa encoberto na largada e vai LIMPANDO durante a corrida.
        ClearingUp => (
            3,
            60,
            1,
            vec![
                (3, QUALI),
                (3, 0),
                (3, at(0.0)),
                (2, at(0.4)),
                (1, at(0.75)),
                (0, rend),
            ],
        ),
        // Choveu na quali; a corrida começa nublada e ABRE durante (pista secou).
        WetQualyDryRace => (
            2,
            60,
            1,
            vec![
                (7, QUALI),
                (3, 0),
                (3, at(0.0)),
                (2, at(0.4)),
                (1, at(0.7)),
                (0, rend),
            ],
        ),
        // Molhadas — ficam molhadas o tempo todo da CORRIDA (nunca seca).
        SteadyRain => (
            3,
            88,
            iw,
            vec![(3, QUALI), (3, 0), (iet, at(0.0)), (iet, rend)],
        ),
        Improving => (
            3,
            90,
            iw,
            vec![
                (3, QUALI),
                (3, 0),
                (8, at(0.0)),
                (7, at(0.4)),
                (6, at(0.8)),
                (6, rend),
            ],
        ),
        StormArrives => (
            2,
            85,
            iw,
            vec![
                (2, QUALI),
                (2, 0),
                (6, at(0.0)),
                (7, at(0.4)),
                (8, at(0.8)),
                (8, rend),
            ],
        ),
        PulsingStorm => (
            3,
            90,
            iw,
            vec![
                (3, QUALI),
                (3, 0),
                (8, at(0.0)),
                (7, at(0.35)),
                (6, at(0.5)),
                (7, at(0.7)),
                (8, rend),
            ],
        ),
        LightQualyWorseRace => (
            3,
            85,
            iw,
            vec![(6, QUALI), (3, 0), (6, at(0.0)), (7, at(0.5)), (iet, rend)],
        ),
        // 1ª corrida: pista SECA (track_water 0), céu LIMPO ancorado na largada e por
        // quase toda a corrida → garoa leve SÓ na última volta (cosmética).
        FirstRaceScript => (
            0,
            50,
            0,
            vec![
                (0, QUALI),
                (0, 0),
                (0, at(0.0)),
                (0, at(0.92)),
                (6, at(0.97)),
                (6, rend),
            ],
        ),
    };
    WeatherProfile {
        skies,
        humidity,
        track_water: water,
        keyframes: kf,
    }
}

// ─── Horário da corrida (golden hour, noite, sem meio-dia) ──────────────────

/// Horários do sol (aproximados, latitude média) por estação: (nascer, pôr) em h.
fn sun_times(season: Season) -> (f64, f64) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seco_sem_penalidade() {
        assert_eq!(rain_skill_penalty(0.0, RainIntensity::None), 0);
        assert_eq!(rain_skill_penalty(100.0, RainIntensity::None), 0);
    }

    #[test]
    fn chuva_decente_bate_os_exemplos() {
        // fator 100 → 8, fator 90 → 10, fator 0 → 30.
        assert_eq!(rain_skill_penalty(100.0, RainIntensity::Decent), 8);
        assert_eq!(rain_skill_penalty(90.0, RainIntensity::Decent), 10);
        assert_eq!(rain_skill_penalty(0.0, RainIntensity::Decent), 30);
    }

    #[test]
    fn chuva_muito_forte_bate_os_exemplos() {
        // Números EXATOS do user: fator 0 → 40, fator 90 → 20, fator 100 → 14.
        assert_eq!(rain_skill_penalty(0.0, RainIntensity::VeryHeavy), 40);
        assert_eq!(rain_skill_penalty(90.0, RainIntensity::VeryHeavy), 20);
        assert_eq!(rain_skill_penalty(100.0, RainIntensity::VeryHeavy), 14);
    }

    #[test]
    fn intensidade_sobe_a_reta_toda() {
        // Pro MESMO piloto, mais chuva = mais penalidade (até pros bons).
        let bom = 95.0;
        let leve = rain_skill_penalty(bom, RainIntensity::Light);
        let dec = rain_skill_penalty(bom, RainIntensity::Decent);
        let forte = rain_skill_penalty(bom, RainIntensity::Heavy);
        let mf = rain_skill_penalty(bom, RainIntensity::VeryHeavy);
        assert!(
            leve < dec && dec < forte && forte < mf,
            "{leve} {dec} {forte} {mf}"
        );
    }

    #[test]
    fn fator_chuva_alto_perde_menos() {
        // Na mesma chuva, quem é melhor na chuva perde menos.
        let i = RainIntensity::Heavy;
        assert!(rain_skill_penalty(100.0, i) < rain_skill_penalty(50.0, i));
        assert!(rain_skill_penalty(50.0, i) < rain_skill_penalty(0.0, i));
    }

    #[test]
    fn fator_clampa() {
        assert_eq!(
            rain_skill_penalty(150.0, RainIntensity::Decent),
            rain_skill_penalty(100.0, RainIntensity::Decent)
        );
    }

    #[test]
    fn estacao_por_hemisferio() {
        // Janeiro: inverno no norte, verão no sul.
        assert_eq!(season_for(1, Hemisphere::North), Season::Winter);
        assert_eq!(season_for(1, Hemisphere::South), Season::Summer);
        // Julho: verão no norte, inverno no sul.
        assert_eq!(season_for(7, Hemisphere::North), Season::Summer);
        assert_eq!(season_for(7, Hemisphere::South), Season::Winter);
    }

    #[test]
    fn tendencia_pista_e_estacao() {
        // Pista chuvosa no inverno >> pista seca no verão.
        let molhada = rain_tendency(ClimateTendency::Rainy, Season::Winter);
        let seca = rain_tendency(ClimateTendency::Dry, Season::Summer);
        assert!(molhada > 0.6, "{molhada}");
        assert!(seca < 0.15, "{seca}");
    }

    #[test]
    fn primeira_corrida_e_roteirizada() {
        let w = generate_weather(1, Hemisphere::North, ClimateTendency::Rainy, 123, true);
        assert_eq!(w.scenario, WeatherScenario::FirstRaceScript);
        assert!(!w.is_wet_race);
        assert_eq!(w.race_intensity, RainIntensity::None);
    }

    #[test]
    fn tendencia_baixa_nunca_molha_de_verdade() {
        // Pista Normal no outono = ~0.345 < limiar → só sustos, nunca molha.
        for seed in 0..500u64 {
            let w = generate_weather(10, Hemisphere::North, ClimateTendency::Normal, seed, false);
            assert!(!w.is_wet_race, "molhou com tendência baixa (seed {seed})");
        }
    }

    #[test]
    fn tendencia_alta_molha_as_vezes() {
        // Pista chuvosa no inverno = tendência alta → algumas molham, mas NÃO a maioria.
        let mut wet = 0;
        for seed in 0..500u64 {
            let w = generate_weather(1, Hemisphere::North, ClimateTendency::Rainy, seed, false);
            if w.is_wet_race {
                wet += 1;
            }
        }
        assert!(wet > 0, "nunca molhou mesmo com tendência alta");
        assert!(
            wet < 400,
            "molhou demais ({wet}/500) — bias pra seco quebrou"
        );
    }

    #[test]
    fn bias_geral_pra_seco() {
        // Numa pista Normal ao longo do ano, a grande maioria deve ser seca.
        let mut wet = 0;
        let total = 12 * 200;
        for month in 1..=12u32 {
            for seed in 0..200u64 {
                let w = generate_weather(
                    month,
                    Hemisphere::North,
                    ClimateTendency::Normal,
                    seed,
                    false,
                );
                if w.is_wet_race {
                    wet += 1;
                }
            }
        }
        assert!(
            (wet as f64) / (total as f64) < 0.15,
            "molhou demais: {wet}/{total}"
        );
    }

    #[test]
    fn deterministico_mesmo_seed() {
        let a = generate_weather(1, Hemisphere::North, ClimateTendency::Rainy, 999, false);
        let b = generate_weather(1, Hemisphere::North, ClimateTendency::Rainy, 999, false);
        assert_eq!(a.scenario, b.scenario);
        assert_eq!(a.is_wet_race, b.is_wet_race);
        assert_eq!(a.race_intensity, b.race_intensity);
    }

    const SEASONS: [Season; 4] = [
        Season::Winter,
        Season::Spring,
        Season::Summer,
        Season::Autumn,
    ];

    #[test]
    fn dia_nunca_no_meio_dia() {
        for season in SEASONS {
            for seed in 0..300u64 {
                // Rookie = sempre de dia; nenhum horário cai em 11–14h.
                let h = generate_race_start_hour(season, 0, false, seed);
                assert!(!(11.0..14.0).contains(&h), "meio-dia: {h} ({season:?})");
            }
        }
    }

    #[test]
    fn rookie_nunca_de_noite() {
        for season in SEASONS {
            let (_, ss) = sun_times(season);
            for seed in 0..300u64 {
                // Mesmo no Charlotte (lit), rookie nunca de noite.
                let h = generate_race_start_hour(season, 0, true, seed);
                assert!(h < ss, "rookie de noite: {h} ({season:?})");
            }
        }
    }

    #[test]
    fn charlotte_corre_muito_de_noite() {
        let (_, ss) = sun_times(Season::Spring);
        let night = (0..1000u64)
            .filter(|&s| generate_race_start_hour(Season::Spring, 1, true, s) > ss + 0.5)
            .count();
        let frac = night as f64 / 1000.0;
        assert!((0.72..0.88).contains(&frac), "Charlotte noite {frac}");
    }

    #[test]
    fn pista_sem_luz_raramente_de_noite() {
        let (_, ss) = sun_times(Season::Spring);
        let night = (0..1000u64)
            .filter(|&s| generate_race_start_hour(Season::Spring, 1, false, s) > ss + 0.5)
            .count();
        let frac = night as f64 / 1000.0;
        assert!((0.06..0.17).contains(&frac), "noite sem luz {frac}");
    }

    #[test]
    fn golden_segue_a_estacao() {
        // Pôr do sol mais cedo no inverno → golden hour da tarde mais cedo.
        assert!(sun_times(Season::Winter).1 < sun_times(Season::Summer).1);
    }

    #[test]
    fn horario_deterministico() {
        let a = generate_race_start_hour(Season::Summer, 1, false, 42);
        let b = generate_race_start_hour(Season::Summer, 1, false, 42);
        assert_eq!(a.to_bits(), b.to_bits());
    }

    fn story(scenario: WeatherScenario, wet: bool, intensity: RainIntensity) -> WeatherStory {
        WeatherStory {
            scenario,
            is_wet_race: wet,
            race_intensity: intensity,
            qualy_intensity: RainIntensity::None,
            season: Season::Spring,
            tendency: 0.5,
        }
    }

    #[test]
    fn perfil_molhado_tem_chuva() {
        let p = story_to_profile(
            &story(WeatherScenario::SteadyRain, true, RainIntensity::VeryHeavy),
            20,
        );
        assert!(p.track_water > 0);
        assert!(p.keyframes.iter().any(|(et, _)| *et >= 6), "sem chuva");
    }

    #[test]
    fn perfil_seco_sem_chuva_no_grosso() {
        let p = story_to_profile(
            &story(WeatherScenario::ClearDry, false, RainIntensity::None),
            20,
        );
        assert_eq!(p.track_water, 0);
        assert!(p.keyframes.iter().all(|(et, _)| *et < 6));
    }

    #[test]
    fn primeira_corrida_termina_com_pingos() {
        let p = story_to_profile(
            &story(WeatherScenario::FirstRaceScript, false, RainIntensity::None),
            20,
        );
        let last = p.keyframes.last().unwrap();
        assert_eq!(last.0, 6, "não terminou com pingos");
        // Fim = RACE_START (35) + duração (20). A chuva tem que cair DURANTE a corrida
        // (offset > 35 = largada às 4:20), não na prática/quali.
        assert_eq!(last.1, 55, "pingos não no fim da corrida");
        assert!(
            p.keyframes.iter().any(|(et, off)| *et >= 6 && *off > 35),
            "chuva fora da corrida"
        );
    }
}
