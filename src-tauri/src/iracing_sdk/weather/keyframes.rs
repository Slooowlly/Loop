//! Cenário sorteado → timeline do iRacing (keyframes do export) e a timeline em
//! frações da corrida usada pela UI.

use serde::{Deserialize, Serialize};

use super::historia::{WeatherScenario, WeatherStory};
use super::penalidade::RainIntensity;

/// Fração da corrida a partir da qual uma prova SECA pode ter chuva (o "pingos no fim"
/// que o design permite). Antes disso o arco de uma corrida seca é seco de verdade.
pub const DRY_RAIN_ONSET: f64 = 0.95;

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
    // DOUTRINA: a corrida é 100% SECA ou 100% MOLHADA. A penalidade de skill da IA é
    // fixa no fim de semana inteiro, então um trecho seco numa prova molhada (ou chuva
    // no meio de uma prova seca) descola o que o jogador enfrenta do que foi cobrado.
    // Numa corrida SECA a chuva só pode aparecer no TRECHO FINAL (a "ameaça" que se
    // cumpre na última volta), nunca antes.
    let rain_on = at(DRY_RAIN_ONSET);
    // Segura o tempo SECO até uma unidade de offset antes do gatilho. Sem este ponto o
    // iRacing INTERPOLA do último keyframe seco até a garoa e a chuva chega bem no meio
    // da prova — era o furo do "começou seca e choveu depois".
    let hold_dry = rain_on - 1;
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
        // Corrida SECA larga em pista SECA. Água residual no início deixaria o grid em
        // dúvida de pneu numa prova em que a IA corre sem penalidade nenhuma.
        0
    };
    // TETO e PISO do arco de uma corrida molhada.
    // - Piso 7 (chuva de verdade) em TODOS os pontos, não só na largada: garoa deixa a
    //   pista no limiar seco/molhado, o grid volta pro slick no meio da prova e a
    //   punição — cobrada do pelotão inteiro, a corrida toda — deixa de valer.
    // - Teto = o `event_type` da MESMA intensidade que gerou a penalidade: uma prova
    //   `Decent` não pode rodar como temporal, nem um temporal terminar em garoa.
    // Efeito colateral aceito: com teto 7 (Decent/Heavy) o arco fica constante — só o
    // temporal (teto 8) tem faixa pra desenhar "afrouxa" e "adensa".
    let wet_top = intensity_event_type(story.race_intensity).max(7);
    let wet = |e: i64| e.clamp(7, wet_top);
    use WeatherScenario::*;
    let (skies, humidity, water, kf): (i64, i64, i64, Vec<(i64, i64)>) = match story.scenario {
        ClearDry => (
            1,
            45,
            0,
            vec![(1, QUALI), (1, at(0.0)), (0, at(0.4)), (1, rend)],
        ),
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
        // Céu limpo a prova toda; pingos só no TRECHO FINAL.
        LastDrops => (
            1,
            50,
            0,
            vec![
                (1, QUALI),
                (1, at(0.0)),
                (1, hold_dry),
                (6, rain_on),
                (6, rend),
            ],
        ),
        // A AMEAÇA que se cumpre no fim: o céu fecha no meio da prova (sem pingo) e a
        // garoa só cai na última volta. ANTES a garoa caía em 45%–60% da corrida — chuva
        // no meio de uma prova SECA, sem penalidade nenhuma na IA. Era o furo principal.
        PassingDrizzle => (
            2,
            55,
            0,
            vec![
                (1, QUALI),
                (1, at(0.0)),
                (2, at(0.45)),
                (3, at(0.7)),
                (3, hold_dry),
                (6, rain_on),
                (6, rend),
            ],
        ),
        // Começa encoberto na largada e vai LIMPANDO durante a corrida.
        ClearingUp => (
            3,
            60,
            0,
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
            0,
            vec![
                (7, QUALI),
                (3, at(0.0)),
                (2, at(0.4)),
                (1, at(0.7)),
                (0, rend),
            ],
        ),
        // Molhadas — ficam molhadas do verde à bandeira (nunca afrouxam pra garoa nem
        // passam do teto da intensidade que gerou a penalidade).
        SteadyRain => (
            3,
            88,
            iw,
            vec![(3, QUALI), (wet_top, at(0.0)), (wet_top, rend)],
        ),
        Improving => (
            3,
            90,
            iw,
            vec![
                (3, QUALI),
                (wet(8), at(0.0)),
                (wet(7), at(0.4)),
                (wet(7), at(0.8)),
                (wet(7), rend),
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
                (wet(7), at(0.0)),
                (wet(7), at(0.4)),
                (wet(8), at(0.8)),
                (wet(8), rend),
            ],
        ),
        PulsingStorm => (
            3,
            90,
            iw,
            vec![
                (3, QUALI),
                (wet(8), at(0.0)),
                (wet(7), at(0.35)),
                (wet(7), at(0.5)),
                (wet(7), at(0.7)),
                (wet(8), rend),
            ],
        ),
        // Garoa na QUALI (6, aceitável — quali é sessão à parte), mas a CORRIDA já
        // larga molhada de verdade e assim segue. Nunca larga de garoa.
        LightQualyWorseRace => (
            3,
            85,
            iw,
            vec![
                (6, QUALI),
                (wet(7), at(0.0)),
                (wet(7), at(0.5)),
                (wet_top, rend),
            ],
        ),
        // 1ª corrida: largada LIMPA → céu fecha no MEIO (frente entrando) → pingos só na
        // ÚLTIMA VOLTA. É o que o roteiro sempre disse fazer; os keyframes é que punham
        // a garoa na METADE da prova, molhando meia corrida de estreia sem penalidade.
        FirstRaceScript => (
            0,
            55,
            0,
            vec![
                (0, QUALI),
                (0, at(0.0)),
                (2, at(0.35)),
                (3, at(0.6)),
                (3, hold_dry),
                (6, rain_on),
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
