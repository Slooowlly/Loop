//! Teste de chuva: tabela de referência de skill por cenário de chuva.

/// Uma linha da tabela de referência do teste de chuva (skill por cenário).
#[derive(serde::Serialize)]
pub struct RainTestRow {
    pub name: String,
    pub base_skill: i64,
    pub fator_chuva: i64,
    pub skill_seco: i64,
    pub skill_molhado: i64,
    pub skill_tempestade: i64,
}

#[derive(serde::Serialize)]
pub struct RainTestResult {
    /// Nomes das 3 seasons criadas (carregar no iRacing).
    pub seasons: Vec<String>,
    /// 16 pilotos com o skill efetivo em cada cenário (referência do esperado).
    pub rows: Vec<RainTestRow>,
}

/// TESTE DE CHUVA: gera 3 seasons (Seco / Molhado / Tempestade), cada uma com seu
/// próprio roster de 16 pilotos controlados (4 skills × 4 fatores de chuva), onde o
/// `driver_skill` já entra RE-RANKEADO pela penalidade de chuva POR PILOTO
/// (`rain_skill_penalty`). Valida que bons-de-chuva sobem no molhado/tempestade.
/// Como a IA do iRacing não tem atributo de chuva e o roster é fixo por season, cada
/// clima precisa do seu export — daí as 3 seasons.
#[tauri::command]
pub fn iracing_export_rain_test() -> Result<RainTestResult, String> {
    use crate::iracing_sdk::weather::{RainIntensity, Season, WeatherScenario, WeatherStory};
    use crate::iracing_sdk::{paths, roster_gen, season_gen, weather};

    let car = roster_gen::car_spec("mx5").ok_or("carro mx5 não encontrado")?;
    // Matriz: 4 níveis de skill × 4 níveis de fator_chuva. Cor do carro por skill (pra
    // achar na pista); o resultado vem pelo NOME ("Bom-Mestre" etc.).
    let skills = [
        ("Ruim", 45.0, "888888"),
        ("Mediano", 62.0, "2255FF"),
        ("Bom", 80.0, "22CC44"),
        ("Alien", 94.0, "FF2222"),
    ];
    let rains = [
        ("Pessimo", 10.0),
        ("Mediano", 50.0),
        ("Bom", 78.0),
        ("Mestre", 100.0),
    ];
    // (rótulo, cenário de clima, intensidade da corrida, molhada?)
    let scenarios = [
        (
            "Seco",
            WeatherScenario::FirstRaceScript,
            RainIntensity::None,
            false,
        ),
        (
            "Molhado",
            WeatherScenario::SteadyRain,
            RainIntensity::Decent,
            true,
        ),
        (
            "Tempestade",
            WeatherScenario::SteadyRain,
            RainIntensity::VeryHeavy,
            true,
        ),
    ];

    let track_id = 516; // Navarra (road, conteúdo grátis)
    let start_time = "2031-06-15T16:00:00".to_string();
    let race_end: i64 = 15;

    let airosters =
        paths::airosters_dir().ok_or("Não foi possível localizar a pasta airosters do iRacing.")?;
    let aiseasons =
        paths::aiseasons_dir().ok_or("Não foi possível localizar a pasta aiseasons do iRacing.")?;

    let mut seasons = Vec::new();
    for (sc_label, scenario, intensity, is_wet) in scenarios {
        // Roster: driver_skill = base − penalidade(fator, intensidade) deste cenário.
        let mut drivers = Vec::new();
        let mut row = 0i64;
        for (_sk_label, base, color) in skills {
            for (rn_label, fator) in rains {
                let _ = rn_label;
                let pen = weather::rain_skill_penalty(fator, intensity);
                let skill = (base - pen as f64).clamp(1.0, 100.0).round() as i64;
                let name = format!("{}-{}", skills_label(base), rains_label(fator));
                let design = format!("0,{color},111111,FFFFFF");
                drivers.push(roster_gen::RosterDriver {
                    driver_name: name,
                    car_number: (row + 1).to_string(),
                    car_design: design.clone(),
                    suit_design: design.clone(),
                    helmet_design: design,
                    car_path: car.car_path.to_string(),
                    car_id: car.car_id,
                    car_class_id: car.car_class_id,
                    sponsor1: car.sponsors[0],
                    sponsor2: car.sponsors.get(1).copied().unwrap_or(car.sponsors[0]),
                    number_design: "0,0,FFFFFF,777777,000000".to_string(),
                    driver_skill: skill,
                    driver_aggression: 50,
                    driver_optimism: 50,
                    driver_smoothness: 50,
                    pit_crew_skill: 50,
                    strategy_riskiness: 50,
                    driver_age: 28,
                    id: uuid::Uuid::new_v4().to_string(),
                    row_index: row,
                });
                row += 1;
            }
        }
        let roster = roster_gen::RosterFile { drivers };
        let roster_name = format!("Teste Chuva - {sc_label}");
        let safe: String = roster_name
            .chars()
            .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
            .collect();

        let rdir = airosters.join(&safe);
        std::fs::create_dir_all(&rdir).map_err(|e| format!("Falha ao criar pasta roster: {e}"))?;
        let rjson = serde_json::to_string_pretty(&roster)
            .map_err(|e| format!("Falha ao serializar roster: {e}"))?;
        std::fs::write(rdir.join("roster.json"), rjson)
            .map_err(|e| format!("Falha ao gravar roster: {e}"))?;

        // Clima do cenário via o modelo calibrado.
        let story = WeatherStory {
            scenario,
            is_wet_race: is_wet,
            race_intensity: intensity,
            qualy_intensity: RainIntensity::None,
            season: Season::Summer,
            tendency: 0.0,
        };
        let profile = weather::story_to_profile(&story, race_end);
        let wind = weather::generate_wind(&story, 0x5EED ^ story.race_intensity as u64);
        let ew = season_gen::EventWeather {
            skies: profile.skies,
            humidity: profile.humidity,
            temp_c: 18,
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
            weather_id: format!("0_{}", uuid::Uuid::new_v4()),
            start_time: start_time.clone(),
        };
        let global = season_gen::EventWeather {
            skies: 1,
            humidity: 45,
            temp_c: 18,
            track_water: 0,
            wind_kmh: 10,
            wind_dir_deg: 0,
            keyframes: vec![season_gen::WeatherKeyframe {
                event_type: 1,
                time_offset: 0,
            }],
            weather_id: format!("0_{}", uuid::Uuid::new_v4()),
            start_time: start_time.clone(),
        };
        let params = season_gen::SeasonParams {
            roster_name: roster_name.clone(),
            name: roster_name.clone(),
            car_id: car.car_id,
            car_class_id: car.car_class_id,
            race_length_min: race_end,
            max_drivers: 16,
            min_skill: 25,
            max_skill: 95,
            year: 2031,
            global_weather: global,
            events: vec![season_gen::EventInput {
                track_id,
                is_oval: false,
                event_id: uuid::Uuid::new_v4().to_string(),
                weather: ew,
                results: None,
            }],
        };
        let season_json = season_gen::build_season(&params);
        std::fs::create_dir_all(&aiseasons)
            .map_err(|e| format!("Falha ao criar pasta aiseasons: {e}"))?;
        let sjson = serde_json::to_string_pretty(&season_json)
            .map_err(|e| format!("Falha ao serializar season: {e}"))?;
        std::fs::write(aiseasons.join(format!("{safe}.json")), sjson)
            .map_err(|e| format!("Falha ao gravar season: {e}"))?;
        seasons.push(roster_name);
    }

    // Tabela de referência (skill efetivo de cada piloto por cenário).
    let mut rows = Vec::new();
    for (_sk, base, _color) in skills {
        for (_rn, fator) in rains {
            rows.push(RainTestRow {
                name: format!("{}-{}", skills_label(base), rains_label(fator)),
                base_skill: base as i64,
                fator_chuva: fator as i64,
                skill_seco: (base - weather::rain_skill_penalty(fator, RainIntensity::None) as f64)
                    .clamp(1.0, 100.0)
                    .round() as i64,
                skill_molhado: (base
                    - weather::rain_skill_penalty(fator, RainIntensity::Decent) as f64)
                    .clamp(1.0, 100.0)
                    .round() as i64,
                skill_tempestade: (base
                    - weather::rain_skill_penalty(fator, RainIntensity::VeryHeavy) as f64)
                    .clamp(1.0, 100.0)
                    .round() as i64,
            });
        }
    }
    Ok(RainTestResult { seasons, rows })
}

fn skills_label(base: f64) -> &'static str {
    match base as i64 {
        45 => "Ruim",
        62 => "Mediano",
        80 => "Bom",
        _ => "Alien",
    }
}
fn rains_label(fator: f64) -> &'static str {
    match fator as i64 {
        10 => "Pessimo",
        50 => "Mediano",
        78 => "Bom",
        _ => "Mestre",
    }
}
