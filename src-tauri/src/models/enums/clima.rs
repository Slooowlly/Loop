//! Enums de condição climática: agrupamento de chuva e condição de pista.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RainGroup {
    Dry,
    Normal,
    Rainy,
}

impl RainGroup {
    pub fn as_str(&self) -> &str {
        match self {
            RainGroup::Dry => "Dry",
            RainGroup::Normal => "Normal",
            RainGroup::Rainy => "Rainy",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Dry" => RainGroup::Dry,
            "Rainy" => RainGroup::Rainy,
            _ => RainGroup::Normal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherCondition {
    Dry,
    Damp,
    Wet,
    HeavyRain,
}

impl WeatherCondition {
    pub fn as_str(&self) -> &str {
        match self {
            WeatherCondition::Dry => "Dry",
            WeatherCondition::Damp => "Damp",
            WeatherCondition::Wet => "Wet",
            WeatherCondition::HeavyRain => "HeavyRain",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Damp" => WeatherCondition::Damp,
            "Wet" => WeatherCondition::Wet,
            "HeavyRain" => WeatherCondition::HeavyRain,
            _ => WeatherCondition::Dry,
        }
    }

    pub fn from_str_strict(s: &str) -> Result<Self, String> {
        match s.trim() {
            "Dry" => Ok(WeatherCondition::Dry),
            "Damp" => Ok(WeatherCondition::Damp),
            "Wet" => Ok(WeatherCondition::Wet),
            "HeavyRain" => Ok(WeatherCondition::HeavyRain),
            other => Err(format!("WeatherCondition invalido: '{other}'")),
        }
    }

    /// Fração de "molhado" da pista (0 = seco, 1 = aguaceiro) que a condição representa para o
    /// desgaste das peças: a chuva estressa a eletrônica e alivia a térmica (motor/arrefecimento).
    /// Escala com a severidade da condição. Consumido pelo Sistema de Quebra.
    pub fn wetness(&self) -> f64 {
        match self {
            WeatherCondition::Dry => 0.0,
            WeatherCondition::Damp => 0.35,
            WeatherCondition::Wet => 0.70,
            WeatherCondition::HeavyRain => 1.0,
        }
    }
}
