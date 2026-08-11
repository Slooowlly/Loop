//! **As condições da rodada** — pista e clima, o que decide QUAL peça sofre e QUANTO.
//!
//! Um dos quatro papéis de `car::breakdown`, e o mais autocontido: são funções puras de
//! `(peça, pista, clima)` para um multiplicador de desgaste, sem estado e sem sorte. Alimenta
//! os dois lados do sistema pela MESMA física — o risco por volta na corrida ao vivo
//! ([`super::LiveBreakdown`]) e o desgaste que persiste na economia
//! ([`conditions_wear_mults`]) —, e o teto de [`CONDITIONS_MAX_MULT`] é a fonte única da poda
//! que mantém os dois sincronizados.

use crate::car::PartType;

/// TETO do mult combinado de condições (pista × clima) por peça/volta. Sem isto, dia quente
/// e úmido em pista peaked chega a ~2× e consome ~65% da vida do motor numa corrida só —
/// esvaziando a grade. O teto poda só a CAUDA brutal: o neutro (~1.0) não muda, e a economia
/// continua respondendo às condições. Aplicado igual no risco ao vivo E na economia.
pub(super) const CONDITIONS_MAX_MULT: f64 = 1.5;

/// Mult combinado de condições (pista × clima) de uma peça, já com o [`CONDITIONS_MAX_MULT`].
/// Fonte ÚNICA da poda — usado pelo risco ao vivo ([`super::LiveBreakdown::advance_lap_at`])
/// e pela economia ([`conditions_wear_mults`]) pra os dois ficarem sincronizados.
pub(super) fn conditions_mult(
    pt: PartType,
    track_pha: (f64, f64, f64),
    mean_align: f64,
    weather: Weather,
) -> f64 {
    (track_wear_mult(pt, track_pha, mean_align) * weather_wear_mult(pt, weather))
        .min(CONDITIONS_MAX_MULT)
}

// ───────────────────────── Influência da pista (qual peça sofre) ─────────────────────────

/// Força "média" do efeito da pista (~±35% nas pistas mais peaked). Calibrável.
const TRACK_STRESS_K: f64 = 1.4;

/// Alinhamento (centrado) da peça com a demanda PHA da pista: positivo = a peça puxa PARA o
/// atributo que a pista cobra (é estressada ali); negativo = puxa pra longe. Ambos os vetores
/// em frações centradas em 1/3, então pista equilibrada → ~0 para todas as peças.
pub(super) fn track_alignment(pt: PartType, track_pha: (f64, f64, f64)) -> f64 {
    let dir = |(p, h, a): (f64, f64, f64)| {
        let t = p + h + a;
        if t > 0.0 {
            (p / t, h / t, a / t)
        } else {
            (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0)
        }
    };
    let (pp, ph, pa) = dir(pt.pha_per_level());
    let (dp, dh, da) = dir(track_pha);
    let t = 1.0 / 3.0;
    (pp - t) * (dp - t) + (ph - t) * (dh - t) + (pa - t) * (da - t)
}

/// Multiplicador de desgaste POR VOLTA da peça nesta pista, **centrado em ~1.0** (subtraindo
/// o `mean_align` das 11 peças) pra não inflar a taxa total do grid: peça alinhada com a
/// pista gasta mais (chega na zona de perigo antes), a contrária gasta menos. Clampado pra
/// não explodir em pistas extremas. É assim que a pista decide QUAL peça tende a quebrar.
pub(super) fn track_wear_mult(pt: PartType, track_pha: (f64, f64, f64), mean_align: f64) -> f64 {
    let a = track_alignment(pt, track_pha) - mean_align;
    (1.0 + TRACK_STRESS_K * a).clamp(0.6, 1.5)
}

// ────────────────── Influência do CLIMA (chuva/calor/umidade/vento) ──────────────────

/// Condições de clima da corrida que modulam o desgaste. Faixas REAIS do iRacing: temp
/// [18,32] °C, umidade [0,100] %, vento [2,48] km/h. Fonte única = a `WeatherStory` (mesma
/// que o iRacing roda), resolvida por corrida.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weather {
    /// Molhado da pista, 0..1 (da chuva).
    pub wetness: f64,
    /// Temperatura do ar, °C (18..32).
    pub temperature: f64,
    /// Umidade relativa, 0..100 %.
    pub humidity: f64,
    /// Velocidade do vento, km/h (2..48).
    pub wind_kmh: f64,
}

impl Weather {
    /// Clima NEUTRO → todos os mults = 1.0 (temp no meio da faixa, vento típico, seco).
    pub const NEUTRAL: Weather = Weather {
        wetness: 0.0,
        temperature: TEMP_MID_C,
        humidity: 45.0,
        wind_kmh: WIND_TYPICAL_KMH,
    };
}

/// Chuva: +% de risco na ELETRÔNICA (curto/sensores). Cheio no aguaceiro (wetness=1).
const RAIN_ELEC_STRESS: f64 = 0.60;
/// Chuva ALIVIA a térmica: −% no motor/arrefecimento (o carro esfria).
const RAIN_THERMAL_RELIEF: f64 = 0.25;
/// Faixa REAL de temperatura do iRacing (°C). O modelo é CENTRADO no MEIO: dia frio alivia
/// a térmica, dia quente a agrava — a média da temporada fica ~neutra (não infla a economia).
const TEMP_MIN_C: f64 = 18.0;
const TEMP_MID_C: f64 = 25.0;
const TEMP_MAX_C: f64 = 32.0;
/// Estresse térmico no motor/arrefecimento no dia mais QUENTE (32 °C) — agora ALCANÇÁVEL.
const HEAT_THERMAL_STRESS: f64 = 0.40;
/// Alívio térmico no dia mais FRIO (18 °C).
const COOL_THERMAL_RELIEF: f64 = 0.20;
/// A umidade AMPLIFICA o estresse do calor (ar úmido refrigera pior). A 100 %, o estresse
/// térmico fica ×(1 + este fator). Só morde o lado QUENTE — dia frio úmido não vira alívio.
const HUMIDITY_THERMAL_BOOST: f64 = 0.50;
/// Vento: estressa suspensão + asas (instabilidade + carga aero flutuante). Centrado na
/// MÉDIA do vento sorteado (`generate_wind` seco ~25 km/h) pra não inflar a economia na média;
/// acima estressa, abaixo alivia de leve.
pub(super) const WIND_TYPICAL_KMH: f64 = 25.0;
const WIND_MAX_KMH: f64 = 48.0;
const WIND_STRESS: f64 = 0.15;

/// Carga térmica ASSINADA da temperatura: +1 no calor máx (32°), 0 no meio (25°), −1 no
/// frio máx (18°). Multiplicada por STRESS (calor) ou RELIEF (frio).
fn thermal_load(temperature: f64) -> f64 {
    if temperature >= TEMP_MID_C {
        ((temperature - TEMP_MID_C) / (TEMP_MAX_C - TEMP_MID_C)).clamp(0.0, 1.0)
    } else {
        -((TEMP_MID_C - temperature) / (TEMP_MID_C - TEMP_MIN_C)).clamp(0.0, 1.0)
    }
}

/// Carga de vento CENTRADA no vento típico: >0 acima (estressa), <0 abaixo (alivia leve).
fn wind_load(wind_kmh: f64) -> f64 {
    ((wind_kmh - WIND_TYPICAL_KMH) / (WIND_MAX_KMH - WIND_TYPICAL_KMH)).clamp(-1.0, 1.0)
}

/// Multiplicador de desgaste por volta pelo CLIMA. Chuva: eletrônica ↑, motor/arrefecimento ↓
/// (resfria). Temperatura CENTRADA: dia quente agrava a térmica, frio alivia; a **umidade
/// amplifica o calor** (o dia quente+úmido é o matador de motor). Vento estressa suspensão +
/// asas. Centrado → não infla a economia na média; só redistribui e cria variância real.
pub(super) fn weather_wear_mult(pt: PartType, weather: Weather) -> f64 {
    let w = weather.wetness.clamp(0.0, 1.0);
    let hum = (weather.humidity / 100.0).clamp(0.0, 1.0);
    let thermal = thermal_load(weather.temperature);
    // Calor (thermal ≥ 0) é amplificado pela umidade; frio (thermal < 0) é alívio puro.
    let thermal_term = if thermal >= 0.0 {
        thermal * HEAT_THERMAL_STRESS * (1.0 + hum * HUMIDITY_THERMAL_BOOST)
    } else {
        thermal * COOL_THERMAL_RELIEF
    };
    let wind = wind_load(weather.wind_kmh);
    match pt {
        PartType::Electronics => 1.0 + w * RAIN_ELEC_STRESS,
        PartType::Engine | PartType::Cooling => {
            (1.0 - w * RAIN_THERMAL_RELIEF) * (1.0 + thermal_term)
        }
        PartType::Suspension | PartType::FrontWing | PartType::RearWing => 1.0 + wind * WIND_STRESS,
        _ => 1.0,
    }
}

/// Multiplicador de desgaste POR CORRIDA de cada peça pelas condições REAIS da rodada
/// (pista × clima), a MESMA física do risco de quebra ao vivo. Alimenta a **economia**: o
/// desgaste que PERSISTE no carro passa a responder à pista (qual peça sofre) e ao clima
/// (chuva → eletrônica; calor → térmica) daquela corrida — pra **grade toda**, de modo que o
/// cérebro de manutenção reaja a corridas brutais. O estilo de pilotagem (só o jogador)
/// multiplica isto por fora. Corrida neutra (pista equilibrada, seco, ≤25 °C) → todos ~1.0,
/// e a economia calibrada não muda. `mean_align` é subtraído dentro de [`track_wear_mult`]
/// pra a pista só REDISTRIBUIR (não inflar o total); só o calor infla.
pub fn conditions_wear_mults(
    track_pha: (f64, f64, f64),
    weather: Weather,
) -> std::collections::HashMap<PartType, f64> {
    let mean_align = PartType::ALL
        .iter()
        .map(|&p| track_alignment(p, track_pha))
        .sum::<f64>()
        / PartType::ALL.len() as f64;
    PartType::ALL
        .iter()
        .map(|&pt| (pt, conditions_mult(pt, track_pha, mean_align, weather)))
        .collect()
}
