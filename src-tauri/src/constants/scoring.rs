#![allow(dead_code)]

use crate::models::enums::WeatherCondition;

/// Converte gap de score de qualificação em gap de tempo de volta (ms).
/// O quali_score é calculado uma única vez por piloto (escala ~55–85).
/// Gap típico entre pole e 5º: 5–10 pontos → 250–500 ms.
pub const QUALI_SCORE_TO_LAP_MS: f64 = 50.0;

/// Converte gap de cumulative_score de corrida em gap de tempo de volta (ms).
/// O cumulative_score acumula 5 segmentos + componente de posição inicial,
/// resultando em gaps absolutos ~3–5× maiores que o quali_score para a mesma
/// diferença de skill. O coeficiente menor compensa essa diferença de escala.
/// Gap típico entre P1 e P2 (skills próximos): 12–17 pontos → 360–510 ms/volta.
pub const RACE_SCORE_TO_LAP_MS: f64 = 30.0;

pub const POINTS_STANDARD: [u8; 10] = [25, 18, 15, 12, 10, 8, 6, 4, 2, 1];
pub const POINTS_ENDURANCE: [u8; 10] = [35, 28, 23, 19, 16, 13, 10, 7, 4, 2];

pub const BONUS_FASTEST_LAP: u8 = 1;
pub const BONUS_OVERALL_1ST: u8 = 5;
pub const BONUS_OVERALL_2ND: u8 = 3;
pub const BONUS_OVERALL_3RD: u8 = 1;

pub struct DifficultyConfig {
    pub id: &'static str,
    pub nome: &'static str,
    pub skill_min_ia: u8,
    pub skill_max_ia: u8,
}

pub struct WeatherPenalty {
    pub condition: WeatherCondition,
    pub base_penalty: f64,
    pub difficulty_multiplier: f64,
}

static DIFFICULTIES: [DifficultyConfig; 4] = [
    DifficultyConfig {
        id: "facil",
        nome: "Fácil",
        skill_min_ia: 20,
        skill_max_ia: 60,
    },
    DifficultyConfig {
        id: "medio",
        nome: "Médio",
        skill_min_ia: 30,
        skill_max_ia: 80,
    },
    DifficultyConfig {
        id: "dificil",
        nome: "Difícil",
        skill_min_ia: 50,
        skill_max_ia: 90,
    },
    DifficultyConfig {
        id: "lendario",
        nome: "Lendário",
        skill_min_ia: 70,
        skill_max_ia: 100,
    },
];

pub const RAIN_INTENSITY_DISTRIBUTION: [(WeatherCondition, f64); 3] = [
    (WeatherCondition::Damp, 0.40),
    (WeatherCondition::Wet, 0.40),
    (WeatherCondition::HeavyRain, 0.20),
];

pub fn get_points_for_position(position: u8, is_endurance: bool) -> u8 {
    let points_table = if is_endurance {
        &POINTS_ENDURANCE
    } else {
        &POINTS_STANDARD
    };

    if !(1..=10).contains(&position) {
        return 0;
    }

    points_table[(position - 1) as usize]
}

pub fn get_points(position: u8, is_endurance: bool) -> u8 {
    get_points_for_position(position, is_endurance)
}

pub fn get_fastest_lap_bonus(position: u8) -> u8 {
    if (1..=10).contains(&position) {
        BONUS_FASTEST_LAP
    } else {
        0
    }
}

pub fn get_overall_bonus(overall_position: u8) -> u8 {
    match overall_position {
        1 => BONUS_OVERALL_1ST,
        2 => BONUS_OVERALL_2ND,
        3 => BONUS_OVERALL_3RD,
        _ => 0,
    }
}

pub fn get_difficulty_config(difficulty: &str) -> Option<&'static DifficultyConfig> {
    DIFFICULTIES.iter().find(|config| config.id == difficulty)
}

pub fn get_difficulty(id: &str) -> (i32, i32) {
    let config = get_difficulty_config(id).or_else(|| get_difficulty_config("medio"));
    if let Some(config) = config {
        (config.skill_min_ia as i32, config.skill_max_ia as i32)
    } else {
        (30, 80)
    }
}

pub fn get_all_difficulties() -> &'static [DifficultyConfig] {
    &DIFFICULTIES
}

pub fn rain_intensity_distribution() -> &'static [(WeatherCondition, f64); 3] {
    &RAIN_INTENSITY_DISTRIBUTION
}

pub fn get_weather_penalty(condition: &WeatherCondition) -> (f64, f64) {
    match condition {
        WeatherCondition::Dry => (0.00, 1.00),
        WeatherCondition::Damp => (0.06, 1.15),
        WeatherCondition::Wet => (0.12, 1.35),
        WeatherCondition::HeavyRain => (0.18, 1.60),
    }
}

pub fn get_rain_penalty(condition: &WeatherCondition) -> (f64, f64) {
    get_weather_penalty(condition)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::enums::WeatherCondition;

    #[test]
    fn test_points_standard_p1() {
        assert_eq!(get_points_for_position(1, false), 25);
    }

    #[test]
    fn test_points_endurance_p1() {
        assert_eq!(get_points_for_position(1, true), 35);
    }

    #[test]
    fn test_points_p11() {
        assert_eq!(get_points_for_position(11, false), 0);
    }

    #[test]
    fn test_weather_penalty_dry() {
        assert_eq!(get_weather_penalty(&WeatherCondition::Dry), (0.0, 1.0));
    }

    #[test]
    fn test_weather_penalty_heavy_rain() {
        assert_eq!(
            get_weather_penalty(&WeatherCondition::HeavyRain),
            (0.18, 1.60)
        );
    }

    #[test]
    fn test_difficulty_config_lendario() {
        let config = get_difficulty_config("lendario").expect("lendario should exist");
        assert_eq!(config.skill_min_ia, 70);
        assert_eq!(config.skill_max_ia, 100);
    }
}
