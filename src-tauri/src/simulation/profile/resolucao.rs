use crate::constants::tracks::get_track;
use crate::models::enums::WeatherCondition;
use crate::simulation::profile::base::{base_profile_for, car_family_for};
use crate::simulation::profile::lap_times::base_lap_time_ms_for;
use crate::simulation::profile::pista::{overtaking_difficulty_for, track_difficulty_for};
use crate::simulation::profile::tipos::SimulationProfile;
use crate::simulation::track_profile::get_track_simulation_data;

// ---------------------------------------------------------------------------
// Função principal de resolução
// ---------------------------------------------------------------------------

/// Resolve o perfil de simulação para uma corrida específica.
/// Ordem: base por category_id → base_lap_time por tabela → ajustes pista → ajustes clima/temp.
pub fn resolve_simulation_profile(
    category_id: &str,
    track_id: u32,
    temperature: f64,
    weather: WeatherCondition,
    _race_duration_minutes: i32,
    _total_laps: i32,
) -> SimulationProfile {
    let base = base_profile_for(category_id);
    let car_family = car_family_for(category_id);

    // Base lap time: tabela explícita primeiro, comprimento como fallback
    let base_lap_time_ms = base_lap_time_ms_for(car_family, track_id).unwrap_or_else(|| {
        get_track(track_id)
            .map(|t| t.comprimento_km * base.ms_per_km_fallback)
            .unwrap_or(90_000.0)
    });

    // Identidade esportiva da pista (character + stress multipliers)
    let track_sim = get_track_simulation_data(track_id);

    // Stress de pista aplicado à degradação de categoria
    let base_tire_degr = base.tire_degradation_rate * track_sim.tire_stress_multiplier;
    let base_phys_degr = base.physical_degradation_rate * track_sim.physical_stress_multiplier;

    // Dificuldade e overtaking baseados no character da pista
    let track_difficulty = track_difficulty_for(track_id);
    let overtaking_difficulty = overtaking_difficulty_for(track_sim.track_character);

    // Ajustes de clima/temperatura
    let mut rain_sensitivity = 1.0_f64;
    let mut incident_rate_multiplier = base.incident_rate_multiplier;
    let mut tire_degradation_rate = base_tire_degr;
    let mut physical_degradation_rate = base_phys_degr;

    match weather {
        WeatherCondition::Wet | WeatherCondition::HeavyRain => {
            rain_sensitivity *= 1.20;
            incident_rate_multiplier *= 1.15;
        }
        WeatherCondition::Damp => {
            rain_sensitivity *= 1.08;
            incident_rate_multiplier *= 1.05;
        }
        WeatherCondition::Dry => {}
    }

    if temperature > 35.0 {
        tire_degradation_rate *= 1.15;
    }
    if temperature < 10.0 {
        physical_degradation_rate *= 1.10;
    }

    SimulationProfile {
        base_lap_time_ms,
        tire_degradation_rate,
        physical_degradation_rate,
        incident_rate_multiplier,
        qualifying_variance_multiplier: base.qualifying_variance_multiplier,
        race_variance_multiplier: base.race_variance_multiplier,
        rain_sensitivity,
        start_chaos_multiplier: base.start_chaos_multiplier,
        track_difficulty_multiplier: track_difficulty,
        overtaking_difficulty_multiplier: overtaking_difficulty,
        race_pace_spread_multiplier: base.race_pace_spread_multiplier,
        track_character: track_sim.track_character,
    }
}
