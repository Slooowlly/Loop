use crate::constants::scoring::get_weather_penalty;
use crate::models::enums::WeatherCondition;

/// Multiplicador de chuva por piloto.
/// Seco = 1.0; outros: penalidade base * (1 - absorção do fator_chuva).
pub fn weather_multiplier(weather: WeatherCondition, fator_chuva: u8) -> f64 {
    if weather == WeatherCondition::Dry {
        return 1.0;
    }
    let (base_penalty, _) = get_weather_penalty(&weather);
    let absorption = fator_chuva as f64 / 100.0 * 0.90;
    let rain_penalty = base_penalty * (1.0 - absorption);
    1.0 - rain_penalty
}

/// Versão com sensibilidade de chuva do contexto.
/// rain_sensitivity > 1.0 amplifica o efeito; < 1.0 atenua.
pub fn adjusted_weather_multiplier(
    weather: WeatherCondition,
    fator_chuva: u8,
    rain_sensitivity: f64,
) -> f64 {
    let base = weather_multiplier(weather, fator_chuva);
    1.0 - ((1.0 - base) * rain_sensitivity)
}

/// Normaliza car_performance (0–16) para escala 0–100.
pub fn normalize_car_performance(car_performance: f64) -> f64 {
    ((car_performance + 5.0) / 21.0 * 100.0).clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dry_weather_multiplier_is_one() {
        assert_eq!(weather_multiplier(WeatherCondition::Dry, 50), 1.0);
    }

    #[test]
    fn test_rain_specialist_less_penalized() {
        let specialist = weather_multiplier(WeatherCondition::HeavyRain, 95);
        let weak = weather_multiplier(WeatherCondition::HeavyRain, 20);
        assert!(
            specialist > weak,
            "specialist={specialist:.3} should > weak={weak:.3}"
        );
    }

    #[test]
    fn test_adjusted_amplifies_with_high_sensitivity() {
        let base = weather_multiplier(WeatherCondition::Wet, 50);
        let amplified = adjusted_weather_multiplier(WeatherCondition::Wet, 50, 1.5);
        assert!(
            amplified < base,
            "higher sensitivity should amplify penalty"
        );
    }

    #[test]
    fn test_normalize_car_performance_midrange() {
        // car=8.0 → (13/21)*100 ≈ 61.9
        let norm = normalize_car_performance(8.0);
        assert!((norm - 61.9).abs() < 1.0);
    }

    #[test]
    fn test_normalize_car_performance_clamps() {
        assert_eq!(normalize_car_performance(-10.0), 0.0);
        assert_eq!(normalize_car_performance(100.0), 100.0);
    }
}
