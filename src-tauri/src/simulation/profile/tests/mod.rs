use crate::models::enums::WeatherCondition;
use crate::simulation::profile::{resolve_simulation_profile, SimulationProfile};

fn profile_for(cat: &str) -> SimulationProfile {
    resolve_simulation_profile(cat, 586, 22.0, WeatherCondition::Dry, 30, 12)
}

#[test]
fn test_rookie_has_more_variance_than_gt3() {
    let rookie = profile_for("mazda_rookie");
    let gt3 = profile_for("gt3");
    assert!(
        rookie.qualifying_variance_multiplier > gt3.qualifying_variance_multiplier,
        "rookie qual_var={} should > gt3={}",
        rookie.qualifying_variance_multiplier,
        gt3.qualifying_variance_multiplier
    );
    assert!(
        rookie.race_variance_multiplier > gt3.race_variance_multiplier,
        "rookie race_var={} should > gt3={}",
        rookie.race_variance_multiplier,
        gt3.race_variance_multiplier
    );
}

#[test]
fn test_endurance_has_higher_tire_degradation_than_gt4() {
    let endurance = profile_for("endurance");
    let gt4 = profile_for("gt4");
    assert!(
        endurance.tire_degradation_rate > gt4.tire_degradation_rate,
        "endurance tire={} should > gt4={}",
        endurance.tire_degradation_rate,
        gt4.tire_degradation_rate
    );
    assert!(
        endurance.physical_degradation_rate > gt4.physical_degradation_rate,
        "endurance phys={} should > gt4={}",
        endurance.physical_degradation_rate,
        gt4.physical_degradation_rate
    );
}

#[test]
fn test_known_track_returns_explicit_lap_time() {
    // Laguna Seca (586) para GT4 deve retornar 77_000ms da tabela
    let profile = resolve_simulation_profile("gt4", 586, 22.0, WeatherCondition::Dry, 30, 12);
    assert_eq!(profile.base_lap_time_ms, 77_000.0);
}

#[test]
fn test_unknown_track_falls_back_to_length_based() {
    // track_id 9999 não existe na tabela nem em tracks.rs → usa 90_000 hardcoded
    let profile = resolve_simulation_profile("gt4", 9999, 22.0, WeatherCondition::Dry, 30, 12);
    assert!(profile.base_lap_time_ms > 0.0);
}

#[test]
fn test_rain_increases_incident_multiplier() {
    let dry = resolve_simulation_profile("gt4", 47, 22.0, WeatherCondition::Dry, 30, 12);
    let rain = resolve_simulation_profile("gt4", 47, 22.0, WeatherCondition::HeavyRain, 30, 12);
    assert!(
        rain.incident_rate_multiplier > dry.incident_rate_multiplier,
        "rain irm={} should > dry={}",
        rain.incident_rate_multiplier,
        dry.incident_rate_multiplier
    );
    assert!(
        rain.rain_sensitivity > dry.rain_sensitivity,
        "rain sensitivity={} should > dry={}",
        rain.rain_sensitivity,
        dry.rain_sensitivity
    );
}

#[test]
fn test_high_temp_increases_tire_degradation() {
    let normal = resolve_simulation_profile("gt4", 47, 22.0, WeatherCondition::Dry, 30, 12);
    let hot = resolve_simulation_profile("gt4", 47, 38.0, WeatherCondition::Dry, 30, 12);
    assert!(
        hot.tire_degradation_rate > normal.tire_degradation_rate,
        "hot tire_degr={} should > normal={}",
        hot.tire_degradation_rate,
        normal.tire_degradation_rate
    );
}

#[test]
fn test_unknown_category_returns_neutral_default_like_values() {
    let profile = resolve_simulation_profile(
        "categoria_inexistente",
        47,
        22.0,
        WeatherCondition::Dry,
        30,
        12,
    );
    // Deve retornar algo válido (não pânico, não zeros)
    assert!(profile.base_lap_time_ms > 0.0);
    assert!(profile.tire_degradation_rate > 0.0);
    assert!(profile.incident_rate_multiplier > 0.0);
}

#[test]
fn test_nordschleife_has_high_difficulty() {
    let profile = resolve_simulation_profile("gt3", 249, 22.0, WeatherCondition::Dry, 60, 5);
    assert!(
        profile.track_difficulty_multiplier >= 1.5,
        "Nordschleife should have difficulty >= 1.5, got {}",
        profile.track_difficulty_multiplier
    );
}

#[test]
fn test_roval_has_lower_overtaking_difficulty() {
    let roval = resolve_simulation_profile("gt4", 554, 22.0, WeatherCondition::Dry, 30, 12); // Charlotte Roval
    let road = resolve_simulation_profile("gt4", 212, 22.0, WeatherCondition::Dry, 30, 12); // Interlagos (Technical)
    assert!(
        roval.overtaking_difficulty_multiplier < road.overtaking_difficulty_multiplier,
        "roval={} should < road={}",
        roval.overtaking_difficulty_multiplier,
        road.overtaking_difficulty_multiplier
    );
}

#[test]
fn test_sebring_has_higher_tire_stress_than_tsukuba() {
    let sebring = resolve_simulation_profile("gt4", 95, 22.0, WeatherCondition::Dry, 30, 12);
    let tsukuba = resolve_simulation_profile("gt4", 324, 22.0, WeatherCondition::Dry, 30, 12);
    assert!(
        sebring.tire_degradation_rate > tsukuba.tire_degradation_rate,
        "Sebring tire={} should > Tsukuba={}",
        sebring.tire_degradation_rate,
        tsukuba.tire_degradation_rate
    );
}

#[test]
fn test_le_mans_has_higher_physical_stress_than_lime_rock() {
    let le_mans = resolve_simulation_profile("gt4", 268, 22.0, WeatherCondition::Dry, 30, 12);
    let lime_rock = resolve_simulation_profile("gt4", 353, 22.0, WeatherCondition::Dry, 30, 12);
    assert!(
        le_mans.physical_degradation_rate > lime_rock.physical_degradation_rate,
        "Le Mans phys={} should > Lime Rock={}",
        le_mans.physical_degradation_rate,
        lime_rock.physical_degradation_rate
    );
}

#[test]
fn test_tight_track_has_higher_overtaking_diff_than_flowing() {
    let hungaroring = resolve_simulation_profile("gt4", 413, 22.0, WeatherCondition::Dry, 30, 12); // Tight
    let spa = resolve_simulation_profile("gt4", 523, 22.0, WeatherCondition::Dry, 30, 12); // Flowing
    assert!(
        hungaroring.overtaking_difficulty_multiplier > spa.overtaking_difficulty_multiplier,
        "Tight={} should > Flowing={}",
        hungaroring.overtaking_difficulty_multiplier,
        spa.overtaking_difficulty_multiplier
    );
}

#[test]
fn test_gt3_has_lower_incident_rate_than_rookie() {
    let rookie = profile_for("mazda_rookie");
    let gt3 = profile_for("gt3");
    assert!(
        gt3.incident_rate_multiplier < rookie.incident_rate_multiplier,
        "gt3={} should < rookie={}",
        gt3.incident_rate_multiplier,
        rookie.incident_rate_multiplier
    );
}
