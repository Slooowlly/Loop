//! Geração de **AI season** do iRacing a partir do calendário da carreira.
//!
//! Espelha o aiseason de referência do usuário (regras de corrida/incidente),
//! mudando: o `rosterName`, a classe/carro, a duração da corrida (por categoria)
//! e os `events[]` (= calendário da carreira, cujos `track_id` JÁ são os trackIds
//! do iRacing). O clima é **POR EVENTO** — cada etapa carrega o clima da sua
//! corrida no calendário (o iRacing aceita um bloco `weather` dentro de cada
//! evento). Sai em `Documentos/iRacing/aiseasons/<série> - <ano>.json`.

use serde_json::{json, Value};

/// Um quadro-chave da linha do tempo de clima (clima dinâmico, version 3).
/// `event_type`: 0 Limpo · 1 Parcialmente · 2 Predominantemente · 3 Encoberto ·
/// 4 Neblina leve · 5 Neblina intensa · 6 Chuva leve · 7 Chuva · 8 Chuva intensa ·
/// 9 Chuva esporádica. `time_offset` em minutos (− antes / + durante a corrida).
pub struct WeatherKeyframe {
    pub event_type: i64,
    pub time_offset: i64,
}

/// Clima de uma etapa, derivado do clima/temperatura do calendário.
pub struct EventWeather {
    /// Céu (0 limpo … 3 encoberto).
    pub skies: i64,
    /// Umidade relativa (%).
    pub humidity: i64,
    /// Temperatura em °C.
    pub temp_c: i64,
    /// Molhado da pista: 0 seco … 5 muito intenso.
    pub track_water: i64,
    /// Linha do tempo do clima (quadros-chave). Clima DINÂMICO (version 3).
    pub keyframes: Vec<WeatherKeyframe>,
    /// `{custid}_{uuid}` que o iRacing usa para identificar o clima.
    pub weather_id: String,
}

/// Uma etapa do calendário para a season.
pub struct EventInput {
    pub track_id: i64,
    /// Oval/roval → paceCar de oval e largada lançada.
    pub is_oval: bool,
    pub event_id: String,
    /// Clima específico desta etapa.
    pub weather: EventWeather,
}

/// Parâmetros para montar a season (preparados a partir da carreira).
pub struct SeasonParams {
    pub roster_name: String,
    /// Nome do arquivo/série, ex.: "Mazda Rookie - 2025".
    pub name: String,
    pub car_id: i64,
    pub car_class_id: i64,
    /// Duração da corrida em minutos (da categoria).
    pub race_length_min: i64,
    pub max_drivers: i64,
    pub min_skill: i64,
    pub max_skill: i64,
    /// Ano (para a data simulada do clima).
    pub year: i32,
    /// Clima global (fallback) — id próprio, sem `eventId`.
    pub global_weather: EventWeather,
    pub events: Vec<EventInput>,
}

/// Categoria de temperatura do iRacing: 1 Frio · 2 Moderado · 3 Calor · 4 Quente.
fn temp_option(temp_c: i64) -> i64 {
    match temp_c {
        t if t < 16 => 1,
        t if t < 23 => 2,
        t if t < 29 => 3,
        _ => 4,
    }
}

/// Monta o bloco `weather` DINÂMICO (version 3) com `weather_timeline` +
/// `keyframes`. `event_id` presente → timeline da etapa; `None` → clima global.
fn weather_value(w: &EventWeather, year: i32, event_id: Option<&str>) -> Value {
    let keyframes: Vec<Value> = w
        .keyframes
        .iter()
        .enumerate()
        .map(|(i, k)| {
            json!({ "event_type": k.event_type, "index": i as i64, "time_offset": k.time_offset })
        })
        .collect();

    let mut timeline = json!({
        "keyframes": keyframes,
        "wind_direction_option": 0,
        "wind_speed_option": 2,
        "temperature_option": temp_option(w.temp_c),
        "weatherId": w.weather_id,
    });
    if let Some(eid) = event_id {
        timeline["eventId"] = json!(eid);
    }

    json!({
        "type": 3,
        "temp_units": 1,
        "temp_value": w.temp_c,
        "rel_humidity": w.humidity,
        "fog": 0,
        "wind_dir": 0,
        "wind_units": 1,
        "wind_value": 3,
        "skies": w.skies,
        "simulated_start_time": format!("{year}-06-01T14:00:00"),
        "simulated_time_multiplier": 1,
        "simulated_time_offsets": [20],
        "version": 3,
        "weather_var_initial": 0,
        "weather_var_ongoing": 0,
        "time_of_day": 4,
        "track_water": w.track_water,
        "weather_id": w.weather_id,
        "weather_timeline": timeline,
        "keyframes": timeline["keyframes"].clone()
    })
}

/// Monta o JSON da AI season espelhando o exemplo de referência.
pub fn build_season(p: &SeasonParams) -> Value {
    let events: Vec<Value> = p
        .events
        .iter()
        .map(|e| {
            // paceCar: oval (category 1, largada lançada) vs road (category 2).
            let (cat, order) = if e.is_oval { (1, 3) } else { (2, 5) };
            let mut ev = json!({
                "trackId": e.track_id,
                "num_opt_laps": 0,
                "paceCar": {
                    "category_id": cat,
                    "car_id": 90,
                    "is_oval": e.is_oval,
                    "is_dirt": false,
                    "car_name": "Pace Car - Truck",
                    "car_class_id": 11,
                    "order": order
                },
                "short_parade_lap": false,
                "must_use_diff_tire_types_in_race": false,
                "subsessions": [5, 6],
                "eventId": e.event_id
            });
            if e.is_oval {
                ev["rolling_starts"] = json!(true);
            }
            // Clima DINÂMICO por etapa (timeline com a eventId desta corrida).
            ev["weather"] = weather_value(&e.weather, p.year, Some(&e.event_id));
            // track_state por etapa (forma que o iRacing usa no clima dinâmico).
            ev["track_state"] = json!({
                "leave_marbles": true,
                "practice_rubber": -1,
                "qualify_rubber": -1,
                "race_rubber": -1,
                "warmup_rubber": -1,
                "practice_grip_compound": null,
                "qualify_grip_compound": null,
                "warmup_grip_compound": null,
                "race_grip_compound": null
            });
            ev
        })
        .collect();

    // Clima global (fallback) — id próprio, sem eventId.
    let global_weather = weather_value(&p.global_weather, p.year, None);

    json!({
        "rosterName": p.roster_name,
        "adaptiveAIEnabled": false,
        "adaptiveAIDifficulty": 0,
        "aiCarClassId": p.car_class_id,
        "aiCarClassIds": [p.car_class_id],
        "avoidUser": false,
        "carId": p.car_id,
        "carSettings": [
            { "car_id": p.car_id, "max_pct_fuel_fill": 100, "max_dry_tire_sets": 0 }
        ],
        "category_id": 5,
        "damage_model": 0,
        "do_not_count_caution_laps": true,
        "full_course_cautions": true,
        "gridPosition": 1,
        "heatID": null,
        "incident_limit": 0,
        "incident_warn_mode": 1,
        "incident_warn_param1": 17,
        "incident_warn_param2": 0,
        "lucky_dog": false,
        "max_drivers": p.max_drivers,
        "maxSkill": p.max_skill,
        "minSkill": p.min_skill,
        "max_visor_tearoffs": -1,
        "multiclassType": 0,
        "must_use_diff_tire_types_in_race": false,
        "no_lapper_wave_arounds": false,
        "num_fast_tows": 0,
        "practice_length": 5,
        "qualify_laps": 3,
        "qualify_length": 10,
        "race_laps": 0,
        "race_length_type": 2,
        "race_length": p.race_length_min,
        "restarts": 2,
        "rolling_starts": false,
        "short_parade_lap": false,
        "start_on_qual_tire": false,
        "startZone": 0,
        "subsessions": [5, 6],
        "time_of_day": 0,
        "track_state": {
            "leave_marbles": true,
            "practice_rubber": -1,
            "qualify_rubber": -1,
            "race_rubber": -1,
            "warmup_rubber": -1
        },
        "heatRacing": false,
        "unsport_conduct_rule_mode": 0,
        "userCarClassId": p.car_class_id,
        "weather": global_weather,
        "events": events,
        "points_system_id": 4,
        "name": p.name
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monta_season_com_eventos_e_duracao() {
        let p = SeasonParams {
            roster_name: "Mazda Rookie - 2025".to_string(),
            name: "Mazda Rookie - 2025".to_string(),
            car_id: 67,
            car_class_id: 74,
            race_length_min: 15,
            max_drivers: 13,
            min_skill: 30,
            max_skill: 80,
            year: 2025,
            global_weather: EventWeather {
                skies: 1,
                humidity: 45,
                temp_c: 22,
                track_water: 0,
                keyframes: vec![WeatherKeyframe { event_type: 1, time_offset: -120 }],
                weather_id: "747998_global".to_string(),
            },
            events: vec![
                EventInput {
                    track_id: 166,
                    is_oval: false,
                    event_id: "e1".to_string(),
                    weather: EventWeather {
                        skies: 1,
                        humidity: 45,
                        temp_c: 22,
                        track_water: 0,
                        keyframes: vec![WeatherKeyframe { event_type: 1, time_offset: -120 }],
                        weather_id: "747998_a".to_string(),
                    },
                },
                EventInput {
                    track_id: 554,
                    is_oval: true,
                    event_id: "e2".to_string(),
                    weather: EventWeather {
                        skies: 3,
                        humidity: 96,
                        temp_c: 18,
                        track_water: 5,
                        keyframes: vec![
                            WeatherKeyframe { event_type: 3, time_offset: -120 },
                            WeatherKeyframe { event_type: 8, time_offset: 0 },
                        ],
                        weather_id: "747998_b".to_string(),
                    },
                },
            ],
        };
        let v = build_season(&p);
        assert_eq!(v["rosterName"], "Mazda Rookie - 2025");
        assert_eq!(v["race_length"], 15);
        assert_eq!(v["race_length_type"], 2);
        assert_eq!(v["aiCarClassIds"][0], 74);
        assert_eq!(v["events"].as_array().unwrap().len(), 2);
        // Evento road: paceCar category 2, sem rolling_starts.
        assert_eq!(v["events"][0]["trackId"], 166);
        assert_eq!(v["events"][0]["paceCar"]["category_id"], 2);
        assert!(v["events"][0].get("rolling_starts").is_none());
        // Evento oval (Charlotte Roval): category 1 + rolling_starts.
        assert_eq!(v["events"][1]["paceCar"]["is_oval"], true);
        assert_eq!(v["events"][1]["rolling_starts"], true);
        // Clima POR EVENTO: etapa 2 molhada (track_water 5), etapa 1 seca.
        assert_eq!(v["events"][0]["weather"]["track_water"], 0);
        assert_eq!(v["events"][1]["weather"]["track_water"], 5);
        assert_eq!(v["events"][1]["weather"]["temp_value"], 18);
        // Clima DINÂMICO (version 3) + timeline.
        assert_eq!(v["events"][1]["weather"]["version"], 3);
        assert_eq!(v["events"][1]["weather"]["weather_id"], "747998_b");
        assert_eq!(v["events"][1]["weather"]["weather_timeline"]["eventId"], "e2");
        // Keyframes: etapa 2 vira chuva intensa (event_type 8) em time_offset 0.
        let kfs = v["events"][1]["weather"]["keyframes"].as_array().unwrap();
        assert_eq!(kfs.len(), 2);
        assert_eq!(kfs[1]["event_type"], 8);
        assert_eq!(kfs[1]["time_offset"], 0);
        // Global tem timeline sem eventId.
        assert_eq!(v["weather"]["version"], 3);
        assert!(v["weather"]["weather_timeline"].get("eventId").is_none());
    }
}
