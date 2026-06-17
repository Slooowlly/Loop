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

/// Normaliza car_performance para a escala de simulação. Linear: cp=−5 → 0,
/// cp=16 → 100. **Sem teto superior** (Pilar B): carros acima de 16 passam de
/// 100 e têm impacto proporcionalmente maior — é assim que a elite se separa.
/// Piso em 0 (cp ≤ −5).
pub fn normalize_car_performance(car_performance: f64) -> f64 {
    ((car_performance + 5.0) / 21.0 * 100.0).max(0.0)
}

/// Valor de car_performance "spec" usado nas categorias rookie: todos os carros
/// são idênticos (peças padrão), removendo qualquer vantagem de carro para que o
/// resultado dependa de talento + sorte. O valor absoluto é neutro (some igual a
/// todos os pilotos) — só precisa ser estável.
pub const ROOKIE_SPEC_CAR_PERFORMANCE: f64 = 2.0;

/// Base da categoria, ignorando a classe (ex.: "endurance:gt3" -> "endurance").
fn category_base(category_id: &str) -> &str {
    category_id.split(':').next().unwrap_or(category_id)
}

/// `car_performance` efetivo na simulação para a categoria. Categorias spec
/// (rookie) usam um valor fixo (carros idênticos); as demais usam o valor real
/// da equipe. Pilar A do redesign carro/dinastias.
pub fn category_car_performance(category_id: &str, car_performance: f64) -> f64 {
    match category_base(category_id) {
        "mazda_rookie" | "toyota_rookie" => ROOKIE_SPEC_CAR_PERFORMANCE,
        _ => car_performance,
    }
}

/// Multiplicador do peso do carro por categoria. Rookie quase não depende do
/// carro (talento decide); as categorias de topo dependem muito mais. O delta
/// é redistribuído aos atributos de piloto pelo chamador (mantendo a soma dos
/// pesos = 1.0). Pilar A do redesign carro/dinastias.
pub fn car_weight_scale(category_id: &str) -> f64 {
    match category_base(category_id) {
        "mazda_rookie" | "toyota_rookie" => 0.15,
        "mazda_amador" | "toyota_amador" => 0.50,
        "bmw_m2" => 0.70,
        "gt4" => 1.00,
        "gt3" => 1.30,
        "production_challenger" => 1.40,
        "lmp2" | "endurance" => 1.60,
        _ => 1.00,
    }
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
    fn test_normalize_car_performance_floor_and_no_ceiling() {
        // Piso em 0.
        assert_eq!(normalize_car_performance(-10.0), 0.0);
        // cp=16 ainda mapeia para 100 (preserva a calibração do Pilar A).
        assert!((normalize_car_performance(16.0) - 100.0).abs() < 0.01);
        // Sem teto: carros acima de 16 passam de 100 e continuam respondendo.
        assert!(normalize_car_performance(25.0) > 100.0, "sem teto rígido");
        assert!(
            normalize_car_performance(30.0) > normalize_car_performance(20.0),
            "continua diferenciando acima do antigo teto"
        );
    }

    #[test]
    fn test_rookie_car_is_spec_regardless_of_team_value() {
        // Dois carros bem diferentes na rookie viram o mesmo valor spec.
        assert_eq!(category_car_performance("mazda_rookie", 12.0), ROOKIE_SPEC_CAR_PERFORMANCE);
        assert_eq!(category_car_performance("toyota_rookie", -4.0), ROOKIE_SPEC_CAR_PERFORMANCE);
        assert_eq!(
            category_car_performance("mazda_rookie", 12.0),
            category_car_performance("mazda_rookie", -4.0),
        );
    }

    #[test]
    fn test_non_rookie_car_passes_through() {
        assert_eq!(category_car_performance("gt3", 9.5), 9.5);
        assert_eq!(category_car_performance("endurance:gt3", 14.0), 14.0);
        assert_eq!(category_car_performance("mazda_amador", 7.0), 7.0);
    }

    #[test]
    fn test_car_weight_scale_low_for_rookie_high_for_top() {
        assert!(car_weight_scale("mazda_rookie") < 0.25, "rookie deve ter peso de carro baixo");
        assert!(car_weight_scale("endurance") > 1.3, "endurance deve ter peso de carro alto");
        assert!(car_weight_scale("endurance:gt3") > 1.3, "classe de endurance herda o peso alto");
        assert!(car_weight_scale("production_challenger") > 1.0);
        assert!(car_weight_scale("mazda_rookie") < car_weight_scale("mazda_amador"));
        assert!(car_weight_scale("gt4") < car_weight_scale("gt3"));
    }
}
