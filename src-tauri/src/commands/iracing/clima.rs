//! Clima do evento: hemisfério, tendência climática, semente determinística e linha do tempo da corrida.

/// Hemisfério da pista pelo país (sul = Austrália, Argentina, Brasil, etc.).
pub(crate) fn track_hemisphere(pais: &str) -> crate::iracing_sdk::weather::Hemisphere {
    use crate::iracing_sdk::weather::Hemisphere;
    const SOUTH: [&str; 9] = [
        "🇦🇺",
        "🇦🇷",
        "🇧🇷",
        "🇿🇦",
        "🇳🇿",
        "🇨🇱",
        "🇺🇾",
        "Austrália",
        "Australia",
    ];
    if SOUTH.iter().any(|s| pais.contains(s)) {
        Hemisphere::South
    } else {
        Hemisphere::North
    }
}

/// `rain_group` da pista → tendência de clima do gerador.
pub(crate) fn climate_tendency(
    g: crate::models::enums::RainGroup,
) -> crate::iracing_sdk::weather::ClimateTendency {
    use crate::iracing_sdk::weather::ClimateTendency;
    use crate::models::enums::RainGroup;
    match g {
        RainGroup::Dry => ClimateTendency::Dry,
        RainGroup::Rainy => ClimateTendency::Rainy,
        _ => ClimateTendency::Normal,
    }
}

/// Mês (1–12) a partir da semana do ano (1–52).
pub(crate) fn month_from_week(week: i32) -> u32 {
    (((week.max(1) - 1) * 12 / 52) + 1).clamp(1, 12) as u32
}

/// Clima da etapa no formato do Sistema de Quebra (molhado/temperatura/umidade/vento).
///
/// FONTE ÚNICA dos quatro consumidores da quebra: o disparo AO VIVO (export do roster), o
/// aviso pré-corrida (forecast) e o pré-roll da corrida SIMULADA (Fase 7). Todos derivam da
/// MESMA história determinística (`generate_weather` sobre a `event_seed`), então o risco que
/// o jogador vê na Sala de Estratégia é o risco sob o tempo que a corrida de fato terá — não
/// importa se ela vai ser dirigida ou simulada. Pista desconhecida cai no clima NEUTRO.
pub(crate) fn race_breakdown_weather(
    track_id: u32,
    week_of_year: i32,
    ev_seed: u64,
    force_wet: bool,
) -> crate::car::breakdown::Weather {
    use crate::constants::tracks::get_track;
    use crate::iracing_sdk::weather;

    let Some(track) = get_track(track_id) else {
        return crate::car::breakdown::Weather::NEUTRAL;
    };
    let mut story = weather::generate_weather(
        month_from_week(week_of_year),
        track_hemisphere(track.pais),
        climate_tendency(track.rain_group),
        ev_seed,
        false,
    );
    if force_wet {
        story.is_wet_race = true;
        story.race_intensity = weather::RainIntensity::Heavy;
        story.scenario = weather::WeatherScenario::SteadyRain;
    }
    crate::car::breakdown::Weather {
        wetness: story_to_weather_condition(&story).wetness(),
        temperature: weather::story_temperature(&story, ev_seed) as f64,
        humidity: weather::story_to_profile(&story, 60).humidity as f64,
        wind_kmh: weather::generate_wind(&story, ev_seed).speed_kmh as f64,
    }
}

/// Semente estável por etapa (carreira + id da etapa) → clima/horário fixos
/// (não re-sorteia a cada export). A carreira entra na mistura porque o id da etapa é
/// SEQUENCIAL por save ("R001") — sem ela, dois saves rolariam a mesma sorte na mesma rodada.
pub(crate) fn event_seed(career_id: &str, event_id: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    career_id.hash(&mut h);
    event_id.hash(&mut h);
    h.finish()
}

/// Converte a história do clima (`generate_weather`) → `WeatherCondition` do
/// calendário/sim. FONTE ÚNICA: o mesmo gerador do export passa a alimentar o
/// `entry.clima` (UI + simulação offline batem com o iRacing). Mapa: seco→Dry,
/// Light→Damp, Decent→Wet, Heavy/VeryHeavy→HeavyRain.
pub(crate) fn story_to_weather_condition(
    story: &crate::iracing_sdk::weather::WeatherStory,
) -> crate::models::enums::WeatherCondition {
    use crate::iracing_sdk::weather::RainIntensity;
    use crate::models::enums::WeatherCondition as W;
    if !story.is_wet_race {
        return W::Dry;
    }
    match story.race_intensity {
        RainIntensity::Heavy | RainIntensity::VeryHeavy => W::HeavyRain,
        RainIntensity::Decent => W::Wet,
        _ => W::Damp, // Light (None não ocorre em corrida molhada)
    }
}

/// Resolve o clima de uma etapa pela FONTE ÚNICA (`generate_weather`) e PERSISTE em
/// `calendar.clima`, pra UI + sim offline baterem com o export. Devolve a condição.
pub(crate) fn resolve_and_persist_race_weather(
    conn: &rusqlite::Connection,
    career_id: &str,
    track: &crate::constants::tracks::TrackInfo,
    week_of_year: i32,
    race_id: &str,
    is_first_race: bool,
) -> crate::models::enums::WeatherCondition {
    let seed = event_seed(career_id, race_id);
    let story = crate::iracing_sdk::weather::generate_weather(
        month_from_week(week_of_year),
        track_hemisphere(track.pais),
        climate_tendency(track.rain_group),
        seed,
        is_first_race,
    );
    let wc = story_to_weather_condition(&story);
    // Temperatura alinhada à MESMA história (mesma fonte do export) → UI e sim batem.
    let temp_c = crate::iracing_sdk::weather::story_temperature(&story, seed) as f64;
    // Umidade e vento da MESMA história (o Sistema de Quebra usa: umidade amplifica o calor
    // no motor; vento estressa suspensão + asas). A umidade é constante por cenário no perfil.
    let humidity = crate::iracing_sdk::weather::story_to_profile(&story, 60).humidity as f64;
    let wind_kmh = crate::iracing_sdk::weather::generate_wind(&story, seed).speed_kmh as f64;
    let _ = conn.execute(
        "UPDATE calendar SET clima = ?1, temperatura = ?2, umidade = ?3, vento = ?4 WHERE id = ?5",
        rusqlite::params![wc.as_str(), temp_c, humidity, wind_kmh, race_id],
    );
    wc
}

/// Monta o `EventWeather` de uma etapa via o gerador de clima + horário (golden
/// hour por estação). Devolve também a `WeatherStory` (p/ a penalidade da chuva).
/// Ano SEGURO para o `simulated_start_time` do clima. O iRacing calcula sol/estação a
/// partir dessa data e ENGASGA com anos muito no futuro (a carreira pode estar em 2042+):
/// cada bloco de clima fica lento o bastante para, somado em muitas etapas, estourar o
/// watchdog de load do sim ("Simulator appeared to be unresponsive for more than 25
/// seconds"). Mapeia o ano da carreira para a janela recente [2024, 2027] preservando
/// mês/dia/hora (o que importa para estação e golden hour) e a fase de ano bissexto. Só o
/// iRacing vê o ano trocado — a carreira segue no ano real. Ver [[project_aiseason_weather_hang]].
pub(crate) fn sim_safe_year(year: i32) -> i32 {
    2024 + (year - 2024).rem_euclid(4)
}

pub(crate) fn build_event_weather(
    track: &crate::constants::tracks::TrackInfo,
    week_of_year: i32,
    year: i32,
    tier: u8,
    custid: i64,
    seed: u64,
    is_first_race: bool,
    race_end: i64,
    force_wet: bool,
    force_night: bool,
) -> (
    crate::iracing_sdk::season_gen::EventWeather,
    crate::iracing_sdk::weather::WeatherStory,
) {
    use crate::iracing_sdk::{season_gen, weather};
    let month = month_from_week(week_of_year);
    let mut story = weather::generate_weather(
        month,
        track_hemisphere(track.pais),
        climate_tendency(track.rain_group),
        seed,
        is_first_race,
    );
    // TESTE: força chuva forte nesta etapa (corrida molhada o tempo todo).
    if force_wet {
        story.is_wet_race = true;
        story.race_intensity = weather::RainIntensity::Heavy;
        story.scenario = weather::WeatherScenario::SteadyRain;
    }
    let is_lit = track.track_id == 556; // Charlotte Roval — única com iluminação
                                        // Etapa designada como noturna pelo calendário força a hora no escuro (sobrepõe
                                        // o sorteio por-pista, mas nunca em rookie — o calendário nunca designa tier 0).
    let hour = if force_night {
        weather::night_start_hour(story.season, seed ^ 0x55)
    } else {
        weather::generate_race_start_hour(story.season, tier, is_lit, seed ^ 0x55)
    };
    let profile = weather::story_to_profile(&story, race_end);
    // Temperatura ALINHADA à história de chuva (mesma fonte determinística) — nunca
    // uma temp "de chuva" numa corrida que roda seca. Presa em [18, 32] pelo gerador.
    let temp_c = weather::story_temperature(&story, seed);
    // Vento VARIÁVEL por corrida (2–48 km/h + direção).
    let wind = weather::generate_wind(&story, seed);
    // Umidade com pequeno jitter determinístico (varia por corrida, ±8), clamp [0,100].
    let hum_jitter = ((seed >> 17) % 17) as i64 - 8;
    let humidity = (profile.humidity + hum_jitter).clamp(0, 100);
    let hh = (hour.floor() as i64).clamp(0, 23);
    let mm = (((hour - hour.floor()) * 60.0).round() as i64).clamp(0, 59);
    let start_time = format!("{}-{month:02}-15T{hh:02}:{mm:02}:00", sim_safe_year(year));
    let ew = season_gen::EventWeather {
        skies: profile.skies,
        humidity,
        temp_c,
        track_water: profile.track_water,
        wind_kmh: wind.speed_kmh,
        wind_dir_deg: wind.dir_deg,
        keyframes: profile
            .keyframes
            .into_iter()
            .map(|(event_type, time_offset)| season_gen::WeatherKeyframe {
                event_type,
                time_offset,
            })
            .collect(),
        weather_id: format!("{custid}_{}", uuid::Uuid::new_v4()),
        start_time,
    };
    (ew, story)
}

/// Rótulo PT do cenário de clima (para a tela do timeline).
fn scenario_label_pt(s: crate::iracing_sdk::weather::WeatherScenario) -> String {
    use crate::iracing_sdk::weather::WeatherScenario::*;
    match s {
        ClearDry => "Seco e limpo",
        Scare => "Céu fecha (sem chuva)",
        LastDrops => "Pingos no fim",
        PassingDrizzle => "Garoa passageira",
        ClearingUp => "Abrindo o tempo",
        WetQualyDryRace => "Secou para a corrida",
        SteadyRain => "Chuva constante",
        Improving => "Chuva afrouxando",
        StormArrives => "Tempestade chegando",
        PulsingStorm => "Tempestade pulsante",
        LightQualyWorseRace => "Piora na corrida",
        FirstRaceScript => "Nublado, pingos no fim",
    }
    .to_string()
}

/// Rótulo PT da intensidade da chuva.
fn intensity_label_pt(i: crate::iracing_sdk::weather::RainIntensity) -> String {
    use crate::iracing_sdk::weather::RainIntensity::*;
    match i {
        None => "Seco",
        Light => "Garoa",
        Decent => "Chuva",
        Heavy => "Chuva forte",
        VeryHeavy => "Temporal",
    }
    .to_string()
}

/// Timeline do clima de uma corrida (frações 0..1) — para a tela de clima (previsão
/// na sala de estratégia + revisão na pós-corrida). Reconstrói o MESMO clima
/// determinístico do export (pista + estação + seed), idêntico ao que a prova seguiu.
#[derive(serde::Serialize)]
pub struct RaceWeatherTimeline {
    pub scenario: String,
    pub is_wet_race: bool,
    pub intensity: String,
    pub points: Vec<crate::iracing_sdk::weather::WeatherTimelinePoint>,
}

#[tauri::command]
pub fn get_race_weather_timeline(
    app: tauri::AppHandle,
    career_id: String,
    race_id: String,
) -> Result<RaceWeatherTimeline, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use tauri::Manager;

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    build_race_weather_timeline(&db.conn, &career_id, &race_id)
}

/// Núcleo de [`get_race_weather_timeline`] sem depender do `AppHandle` — recebe a conexão
/// direto, para ser reusado pelo overlay ao vivo (torre) além da tela de clima. Reconstrói o
/// MESMO clima determinístico (pista + estação + seed) que a prova seguiu.
pub(crate) fn build_race_weather_timeline(
    conn: &rusqlite::Connection,
    career_id: &str,
    race_id: &str,
) -> Result<RaceWeatherTimeline, String> {
    let entry = crate::db::queries::calendar::get_calendar_entry_by_id(conn, race_id)
        .map_err(|e| format!("Falha ao buscar corrida: {e}"))?
        .ok_or_else(|| "Corrida não encontrada".to_string())?;
    let track = crate::constants::tracks::get_track(entry.track_id)
        .ok_or_else(|| "Pista não encontrada".to_string())?;

    // É a corrida de ESTREIA do save? (única que usa o roteiro fixo do 1º clima.)
    let first_id: Option<String> = conn
        .query_row(
            "SELECT c.id FROM calendar c JOIN seasons s ON c.season_id = s.id \
             ORDER BY s.numero ASC, c.week_of_year ASC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok();
    let is_first = first_id.as_deref() == Some(race_id);

    let story = crate::iracing_sdk::weather::generate_weather(
        month_from_week(entry.week_of_year),
        track_hemisphere(track.pais),
        climate_tendency(track.rain_group),
        event_seed(career_id, race_id),
        is_first,
    );
    Ok(RaceWeatherTimeline {
        scenario: scenario_label_pt(story.scenario),
        is_wet_race: story.is_wet_race,
        intensity: intensity_label_pt(story.race_intensity),
        points: crate::iracing_sdk::weather::story_to_timeline(&story),
    })
}
