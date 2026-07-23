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
    // CALIBRADO no SIMULADOR: o offset 0 dos keyframes é a LARGADA DA CORRIDA (NÃO o
    // início da sessão!). Os offsets são ~minutos a partir da largada (dado real: chuva
    // @offset N caía ~N min depois da largada). Então a CORRIDA = offset 0..race_end; a
    // quali/prática ficam em offset NEGATIVO (QUALI=-90 é só a âncora de render).
    // MODELO AFIM MEDIDO no simulador (rookie; largada medida via "tempo restante na
    // sessão", VENTO CALMO p/ a frente seguir o keyframe): clock_min_após_largada =
    // 0.68×offset + C. Recalibrado em Navarra com vento calmo: offset −17 punha a chuva
    // 5 min ANTES da largada ⇒ C≈7. `off(m)` põe o minuto-de-corrida m no offset certo;
    // a CORRIDA ocupa off(0)≈−10 a off(race_end). QUALI=-90 é só âncora de render.
    const QUALI: i64 = -90;
    let off = |race_min: f64| ((race_min - 7.0) / 0.68).round() as i64;
    let at = |f: f64| off(r as f64 * f);
    let rend = at(1.0);
    // Água na PISTA na largada de uma corrida molhada. PISO = 4 ("Wet" firme): o
    // problema real do user foi uma corrida que "começou com chuva leve, tão leve que
    // a maioria largou de pneu seco e depois trocou pra chuva". Como a punição de
    // habilidade de chuva é aplicada ao pelotão INTEIRO, não pode haver dúvida de pneu
    // na largada — a pista tem que estar comprovadamente encharcada quando abre a
    // bandeira, pra o iRacing pôr TODOS de wet. `track_water` não afeta a UI (que usa
    // event_type) nem a punição do sim (que usa `race_intensity`): é só a alavanca da
    // escolha de pneu da largada. Corrida seca segue em 0.
    let iw = if story.is_wet_race {
        intensity_water(story.race_intensity).max(4)
    } else {
        intensity_water(story.race_intensity)
    };
    // Chuva na LARGADA de uma corrida molhada nunca é garoa (event 6): piso = 7 (chuva
    // de verdade). Garoa+pista-no-limiar é justamente o que faz o grid largar dividido.
    let start_iet = if story.is_wet_race {
        intensity_event_type(story.race_intensity).max(7)
    } else {
        intensity_event_type(story.race_intensity)
    };
    let iet = intensity_event_type(story.race_intensity);
    use WeatherScenario::*;
    let (skies, humidity, water, kf): (i64, i64, i64, Vec<(i64, i64)>) = match story.scenario {
        ClearDry => (1, 45, 0, vec![(1, QUALI), (1, at(0.0)), (0, at(0.4)), (1, rend)]),
        // O susto: céu FECHA durante a corrida (encoberto) e fica, mas NÃO chove.
        Scare => (
            2,
            55,
            0,
            vec![
                (1, QUALI),
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
            vec![(1, QUALI), (1, at(0.0)), (1, at(0.6)), (6, at(0.92)), (6, rend)],
        ),
        // Garoa passageira no meio da corrida, volta a abrir.
        PassingDrizzle => (
            2,
            55,
            1,
            vec![
                (1, QUALI),
                (1, at(0.0)),
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
                (3, at(0.0)),
                (2, at(0.4)),
                (1, at(0.7)),
                (0, rend),
            ],
        ),
        // Molhadas — ficam molhadas o tempo todo da CORRIDA (nunca seca).
        SteadyRain => (3, 88, iw, vec![(3, QUALI), (start_iet, at(0.0)), (iet, rend)]),
        Improving => (
            3,
            90,
            iw,
            vec![
                (3, QUALI),
                (8, at(0.0)),
                (7, at(0.4)),
                (6, at(0.8)),
                (6, rend),
            ],
        ),
        // "Tempestade chegando": ANTES abria de garoa (6) e crescia — mas garoa na
        // largada é exatamente o que dividia o grid. Abre já com chuva de verdade (7)
        // e adensa pro fim (8). O arco de "piorar" continua legível, sem ambiguidade.
        StormArrives => (
            2,
            85,
            iw,
            vec![
                (2, QUALI),
                (start_iet.max(7), at(0.0)),
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
                (8, at(0.0)),
                (7, at(0.35)),
                (6, at(0.5)),
                (7, at(0.7)),
                (8, rend),
            ],
        ),
        // Garoa na QUALI (6, aceitável — quali é sessão à parte), mas a CORRIDA já
        // larga molhada de verdade (7) e piora (`iet` ≥ 7). Nunca larga de garoa.
        LightQualyWorseRace => (
            3,
            85,
            iw,
            vec![(6, QUALI), (start_iet.max(7), at(0.0)), (7, at(0.5)), (iet.max(7), rend)],
        ),
        // 1ª corrida: largada LIMPA → céu fecha no MEIO (frente entrando) → garoa leve
        // CAINDO na 2ª metade (vento forte global ajuda a frente a chegar de verdade).
        FirstRaceScript => (
            0,
            55,
            1,
            // A corrida vive em offset NEGATIVO (offset 0 cai ~19 min depois da largada).
            // LIMPO na largada (at 0.0) → nuvem/encoberto fechando → GAROA na 2ª metade
            // (at 0.5) até a bandeirada (rend). Vento forte traz a frente a tempo.
            vec![
                (0, QUALI),
                (0, at(0.0)),
                (2, at(0.35)),
                (3, at(0.45)),
                (6, at(0.5)),
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

// ─── Timeline do clima para a UI (frações da corrida) ──────────────────────

/// Um ponto do timeline de clima da corrida (para a tela): fração da prova + tipo
/// de tempo. `event_type`: 0 limpo, 1 quase limpo, 2 parcial, 3 encoberto, 6 garoa,
/// 7 chuva, 8 chuva forte.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WeatherTimelinePoint {
    /// Fração da corrida: 0 = largada, 1 = bandeira.
    pub frac: f64,
    pub event_type: i64,
}

/// Timeline do clima da corrida em FRAÇÕES (0..1), derivado do MESMO arco que vai pro
/// export — assim a tela mostra o tempo que a prova de fato seguiu. Reaproveita
/// `story_to_profile` e inverte o modelo de offset de volta pra fração (a âncora de
/// QUALI, em offset negativo, é descartada). É só para exibição (erro de
/// arredondamento ~±0.005, irrelevante no gráfico).
pub fn story_to_timeline(story: &WeatherStory) -> Vec<WeatherTimelinePoint> {
    // Duração nominal só para inverter os offsets (o resultado é em fração, então o
    // valor não importa desde que seja o mesmo na ida e na volta).
    const R: i64 = 60;
    let profile = story_to_profile(story, R);
    profile
        .keyframes
        .into_iter()
        .filter_map(|(event_type, offset)| {
            // off(m) = round((m-7)/0.68) ⇒ m ≈ offset*0.68 + 7 ; frac = m / R.
            let race_min = offset as f64 * 0.68 + 7.0;
            let frac = race_min / R as f64;
            if frac < -0.001 {
                None // âncora de QUALI (offset muito negativo) — fora da corrida
            } else {
                Some(WeatherTimelinePoint {
                    frac: frac.clamp(0.0, 1.0),
                    event_type,
                })
            }
        })
        .collect()
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
        // Chance de molhar: Rainy no inverno alta (~0.60) >> Dry no verão mínima (~0.02).
        let molhada = rain_tendency(ClimateTendency::Rainy, Season::Winter);
        let seca = rain_tendency(ClimateTendency::Dry, Season::Summer);
        assert!(molhada >= 0.55, "{molhada}");
        assert!(seca < 0.05, "{seca}");
        // Normal agora TAMBÉM molha (não é mais zero): ~0.20 na primavera.
        let normal = rain_tendency(ClimateTendency::Normal, Season::Spring);
        assert!((0.15..0.30).contains(&normal), "{normal}");
    }

    #[test]
    fn primeira_corrida_e_roteirizada() {
        let w = generate_weather(1, Hemisphere::North, ClimateTendency::Rainy, 123, true);
        assert_eq!(w.scenario, WeatherScenario::FirstRaceScript);
        assert!(!w.is_wet_race);
        assert_eq!(w.race_intensity, RainIntensity::None);
    }

    #[test]
    fn dry_no_verao_quase_nunca_molha() {
        // Pista Dry no verão = chance mínima (~2%): rara, mas NÃO mais zero.
        let mut wet = 0;
        for seed in 0..1000u64 {
            let w = generate_weather(7, Hemisphere::North, ClimateTendency::Dry, seed, false);
            if w.is_wet_race {
                wet += 1;
            }
        }
        assert!(wet > 0, "Dry no verão deveria molhar RARAMENTE, não nunca");
        assert!(wet < 80, "Dry no verão molhou demais ({wet}/1000)");
    }

    #[test]
    fn normal_agora_molha_as_vezes() {
        // MUDANÇA de design: pista Normal (a maioria do catálogo) agora molha às vezes.
        let mut wet = 0;
        for seed in 0..500u64 {
            let w = generate_weather(1, Hemisphere::North, ClimateTendency::Normal, seed, false);
            if w.is_wet_race {
                wet += 1;
            }
        }
        // Normal no inverno ~30% → nem nunca nem maioria.
        assert!((60..220).contains(&wet), "Normal inverno molhou {wet}/500 (esperado ~30%)");
    }

    #[test]
    fn temporal_so_no_tier_alto() {
        // VeryHeavy só aparece em Rainy inverno/outono; nunca em Normal/Dry.
        let mut vh_rainy = 0;
        for seed in 0..2000u64 {
            let w = generate_weather(1, Hemisphere::North, ClimateTendency::Rainy, seed, false);
            if w.race_intensity == RainIntensity::VeryHeavy {
                vh_rainy += 1;
            }
            let n = generate_weather(1, Hemisphere::North, ClimateTendency::Normal, seed, false);
            assert_ne!(n.race_intensity, RainIntensity::VeryHeavy, "Normal deu temporal (seed {seed})");
        }
        assert!(vh_rainy > 0, "Rainy inverno nunca deu temporal");
    }

    #[test]
    fn bias_geral_ainda_seco_na_maioria() {
        // Mesmo com a chuva mais frequente, a MAIORIA das corridas segue seca (Normal
        // ao longo do ano < 30% molhado).
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
            (wet as f64) / (total as f64) < 0.30,
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

    const WET_SCENARIOS: [WeatherScenario; 5] = [
        WeatherScenario::SteadyRain,
        WeatherScenario::Improving,
        WeatherScenario::StormArrives,
        WeatherScenario::PulsingStorm,
        WeatherScenario::LightQualyWorseRace,
    ];

    #[test]
    fn corrida_molhada_nunca_e_so_garoa() {
        // O gerador NUNCA deve entregar `Light` como caráter de uma corrida molhada
        // (garoa = pista no limiar → grid larga dividido de pneu). Varre tendências e
        // seeds e confere o piso.
        for month in 1..=12u32 {
            for hemi in [Hemisphere::North, Hemisphere::South] {
                for group in [
                    ClimateTendency::Dry,
                    ClimateTendency::Normal,
                    ClimateTendency::Rainy,
                ] {
                    for seed in 0..300u64 {
                        let w = generate_weather(month, hemi, group, seed, false);
                        if w.is_wet_race {
                            assert_ne!(
                                w.race_intensity,
                                RainIntensity::Light,
                                "corrida molhada saiu como garoa (seed {seed})"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn corrida_molhada_larga_inequivocamente_molhada() {
        // Para TODO cenário molhado e TODA intensidade de corrida molhada, a largada
        // (keyframe logo após a âncora QUALI) tem chuva de verdade (≥7) e a pista já
        // está encharcada (track_water ≥4) → o iRacing põe TODOS de wet, sem dúvida.
        for scenario in WET_SCENARIOS {
            for intensity in [
                RainIntensity::Decent,
                RainIntensity::Heavy,
                RainIntensity::VeryHeavy,
            ] {
                let p = story_to_profile(&story(scenario, true, intensity), 30);
                assert!(
                    p.track_water >= 4,
                    "{scenario:?}/{intensity:?}: pista seca demais na largada (water {})",
                    p.track_water
                );
                // kf[0] é a âncora de QUALI (offset -90); kf[1] é a LARGADA da corrida.
                let start = p.keyframes[1];
                assert!(
                    start.0 >= 7,
                    "{scenario:?}/{intensity:?}: largada de garoa (event {}) — grid divide o pneu",
                    start.0
                );
            }
        }
    }

    #[test]
    fn timeline_comeca_na_largada_e_termina_na_bandeira() {
        // Qualquer cenário: frações ordenadas, começa ~0 e termina ~1, sem a QUALI.
        let p = story_to_timeline(&story(WeatherScenario::PulsingStorm, true, RainIntensity::Heavy));
        assert!(!p.is_empty());
        assert!(p[0].frac <= 0.05, "não começa na largada: {}", p[0].frac);
        assert!(p.last().unwrap().frac >= 0.95, "não termina na bandeira");
        assert!(
            p.windows(2).all(|w| w[0].frac <= w[1].frac + 1e-9),
            "frações fora de ordem"
        );
        assert!(p.iter().all(|pt| pt.frac >= 0.0), "tem ponto antes da largada (QUALI?)");
    }

    #[test]
    fn timeline_molhado_tem_chuva_seco_nao() {
        let wet = story_to_timeline(&story(WeatherScenario::SteadyRain, true, RainIntensity::VeryHeavy));
        assert!(wet.iter().any(|p| p.event_type >= 6), "corrida molhada sem chuva");
        let dry = story_to_timeline(&story(WeatherScenario::ClearDry, false, RainIntensity::None));
        assert!(dry.iter().all(|p| p.event_type < 6), "corrida seca com chuva");
    }

    #[test]
    fn temperatura_fica_na_faixa_do_iracing() {
        // Toda combinação estação × intensidade × seed cai em [18, 32].
        for season in SEASONS {
            for intensity in [
                RainIntensity::None,
                RainIntensity::Light,
                RainIntensity::Decent,
                RainIntensity::Heavy,
                RainIntensity::VeryHeavy,
            ] {
                for seed in 0..300u64 {
                    let mut s = story(WeatherScenario::SteadyRain, intensity != RainIntensity::None, intensity);
                    s.season = season;
                    let t = story_temperature(&s, seed);
                    assert!((18..=32).contains(&t), "temp fora da faixa: {t} ({season:?}/{intensity:?})");
                }
            }
        }
    }

    #[test]
    fn temperatura_chuva_esfria_e_verao_esquenta() {
        // Média: verão seco > inverno com temporal (a chuva alinha e esfria).
        let media = |season, intensity, wet| {
            let mut acc = 0i64;
            for seed in 0..500u64 {
                let mut s = story(WeatherScenario::SteadyRain, wet, intensity);
                s.season = season;
                acc += story_temperature(&s, seed);
            }
            acc as f64 / 500.0
        };
        let verao_seco = media(Season::Summer, RainIntensity::None, false);
        let inverno_temporal = media(Season::Winter, RainIntensity::VeryHeavy, true);
        assert!(verao_seco > inverno_temporal, "{verao_seco} vs {inverno_temporal}");
    }

    #[test]
    fn temperatura_deterministica() {
        let s = story(WeatherScenario::ClearDry, false, RainIntensity::None);
        assert_eq!(story_temperature(&s, 77), story_temperature(&s, 77));
    }

    #[test]
    fn vento_usa_a_escada_e_varia() {
        // Todo vento é um degrau da WIND_LADDER (2–48) e a direção fica em [0,359].
        // Seco deve exercitar vários degraus (varia entre corridas).
        let ladder: std::collections::HashSet<i64> = WIND_LADDER.iter().map(|(k, _)| *k).collect();
        let mut speeds = std::collections::HashSet::new();
        for seed in 0..500u64 {
            let s = story(WeatherScenario::ClearDry, false, RainIntensity::None);
            let w = generate_wind(&s, seed);
            assert!(ladder.contains(&w.speed_kmh), "vento fora da escada: {}", w.speed_kmh);
            assert!((0..=359).contains(&w.dir_deg), "direção fora da faixa: {}", w.dir_deg);
            speeds.insert(w.speed_kmh);
        }
        assert!(speeds.len() >= 4, "vento seco não varia o suficiente ({} degraus)", speeds.len());
    }

    #[test]
    fn temporal_venta_mais() {
        // Média de vento no temporal > média no seco.
        let media = |intensity, wet| {
            let mut acc = 0i64;
            for seed in 0..500u64 {
                let s = story(WeatherScenario::SteadyRain, wet, intensity);
                acc += generate_wind(&s, seed).speed_kmh;
            }
            acc as f64 / 500.0
        };
        assert!(
            media(RainIntensity::VeryHeavy, true) > media(RainIntensity::None, false),
            "temporal deveria ventar mais"
        );
    }

    #[test]
    fn vento_deterministico() {
        let s = story(WeatherScenario::ClearDry, false, RainIntensity::None);
        assert_eq!(generate_wind(&s, 42), generate_wind(&s, 42));
    }

    #[test]
    fn primeira_corrida_termina_com_pingos() {
        let p = story_to_profile(
            &story(WeatherScenario::FirstRaceScript, false, RainIntensity::None),
            20,
        );
        let last = p.keyframes.last().unwrap();
        assert_eq!(last.0, 6, "não terminou com pingos");
        // Estrutura (robusta ao C do modelo afim): largada LIMPA (keyframe após a âncora
        // QUALI), termina com CHUVA, e os offsets são crescentes (ordenados no tempo).
        assert_eq!(p.keyframes[1].0, 0, "largada não está limpa");
        assert!(
            p.keyframes.windows(2).all(|w| w[0].1 <= w[1].1),
            "offsets fora de ordem"
        );
    }
}
