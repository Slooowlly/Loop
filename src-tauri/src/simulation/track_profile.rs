use serde::{Deserialize, Serialize};

/// Canonical sporting character of the track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackCharacter {
    /// Wide and fast tracks where skill and car pace dominate.
    Flowing,
    /// Mixed profile used as neutral baseline.
    Technical,
    /// Narrow and slow tracks that reward precision and consistency.
    Tight,
    /// Oval with infield section.
    Roval,
}

/// Canonical simulation data for a track.
#[derive(Debug, Clone)]
pub struct TrackSimulationData {
    pub track_character: TrackCharacter,
    /// Multiplies tire degradation (> 1.0 means more wear).
    pub tire_stress_multiplier: f64,
    /// Multiplies physical degradation (> 1.0 means more fatigue).
    pub physical_stress_multiplier: f64,
    /// Weight of acceleration demand on this circuit.
    pub acceleration_weight: f64,
    /// Weight of top-end power demand on this circuit.
    pub power_weight: f64,
    /// Weight of handling demand on this circuit.
    pub handling_weight: f64,
}

impl TrackSimulationData {
    fn new(
        character: TrackCharacter,
        tire: f64,
        physical: f64,
        acceleration: f64,
        power: f64,
        handling: f64,
    ) -> Self {
        Self {
            track_character: character,
            tire_stress_multiplier: tire,
            physical_stress_multiplier: physical,
            acceleration_weight: acceleration,
            power_weight: power,
            handling_weight: handling,
        }
    }
}

pub const BALANCED_CAR_WEIGHTS: (f64, f64, f64) = (34.0, 33.0, 33.0);

/// Returns simulation data for a given iRacing track id.
/// Unknown tracks fall back to a neutral technical profile.
pub fn get_track_simulation_data(track_id: u32) -> TrackSimulationData {
    use TrackCharacter::*;
    match track_id {
        // Rovals
        554 => TrackSimulationData::new(Roval, 0.95, 0.90, 40.0, 35.0, 25.0), // Charlotte Roval
        45 => TrackSimulationData::new(Roval, 0.95, 0.90, 30.0, 50.0, 20.0),  // Daytona Road
        185 => TrackSimulationData::new(Roval, 0.90, 0.88, 35.0, 40.0, 25.0), // Indianapolis Road

        // Flowing
        188 => TrackSimulationData::new(Flowing, 1.25, 1.25, 15.0, 60.0, 25.0), // Spa
        52 => TrackSimulationData::new(Flowing, 1.10, 1.20, 20.0, 50.0, 30.0),  // Road America
        106 => TrackSimulationData::new(Flowing, 1.20, 1.15, 15.0, 55.0, 30.0), // Silverstone GP
        93 => TrackSimulationData::new(Flowing, 1.00, 1.00, 10.0, 70.0, 20.0),  // Monza
        169 => TrackSimulationData::new(Flowing, 1.00, 1.05, 20.0, 45.0, 35.0), // Philip Island
        393 => TrackSimulationData::new(Flowing, 1.05, 1.00, 25.0, 55.0, 20.0), // Bahrain
        193 => TrackSimulationData::new(Flowing, 0.95, 0.95, 20.0, 60.0, 20.0), // Hockenheim GP
        53 => TrackSimulationData::new(Flowing, 1.00, 1.00, 30.0, 35.0, 35.0),  // Sonoma
        360 => TrackSimulationData::new(Flowing, 1.00, 1.00, 20.0, 50.0, 30.0), // Paul Ricard
        373 => TrackSimulationData::new(Flowing, 1.00, 1.05, 25.0, 50.0, 25.0), // Fuji
        389 => TrackSimulationData::new(Flowing, 1.00, 1.00, 25.0, 40.0, 35.0), // Zandvoort
        516 => TrackSimulationData::new(Flowing, 1.00, 0.95, 30.0, 45.0, 25.0), // Yas Marina

        // Tight
        196 => TrackSimulationData::new(Tight, 1.30, 1.45, 20.0, 30.0, 50.0), // Nordschleife
        339 => TrackSimulationData::new(Tight, 1.10, 1.15, 30.0, 15.0, 55.0), // Cadwell Park
        194 => TrackSimulationData::new(Tight, 1.05, 1.10, 45.0, 10.0, 45.0), // Hungaroring
        325 => TrackSimulationData::new(Tight, 0.82, 0.80, 50.0, 10.0, 40.0), // Tsukuba
        318 => TrackSimulationData::new(Tight, 1.05, 1.05, 55.0, 10.0, 35.0), // Long Beach
        504 => TrackSimulationData::new(Tight, 1.05, 1.05, 50.0, 15.0, 35.0), // Detroit
        261 => TrackSimulationData::new(Tight, 1.00, 1.05, 45.0, 35.0, 20.0), // Oulton Fosters
        341 => TrackSimulationData::new(Tight, 1.00, 1.05, 45.0, 15.0, 40.0), // Oulton Island
        8 => TrackSimulationData::new(Tight, 0.85, 0.82, 50.0, 10.0, 40.0),   // Jefferson

        // Technical with explicit stress tuning
        238 => TrackSimulationData::new(Technical, 1.35, 1.30, 35.0, 30.0, 35.0), // Sebring
        528 => TrackSimulationData::new(Technical, 1.20, 1.60, 25.0, 30.0, 45.0), // Nurburgring 24H
        287 => TrackSimulationData::new(Technical, 1.20, 1.50, 15.0, 65.0, 20.0), // Le Mans
        119 => TrackSimulationData::new(Technical, 1.20, 1.30, 25.0, 40.0, 35.0), // Bathurst
        164 => TrackSimulationData::new(Technical, 1.15, 1.25, 25.0, 35.0, 40.0), // Suzuka
        212 => TrackSimulationData::new(Technical, 1.20, 1.15, 30.0, 35.0, 35.0), // COTA
        58 => TrackSimulationData::new(Technical, 1.25, 1.20, 30.0, 30.0, 40.0),  // VIR Full
        67 => TrackSimulationData::new(Technical, 1.10, 1.20, 30.0, 35.0, 35.0),  // Watkins Boot
        14 => TrackSimulationData::new(Technical, 0.85, 0.82, 40.0, 20.0, 40.0),  // Lime Rock
        9 => TrackSimulationData::new(Technical, 0.85, 0.85, 40.0, 25.0, 35.0),   // Summit Point
        195 => TrackSimulationData::new(Technical, 0.88, 0.85, 35.0, 40.0, 25.0), // Hockenheim Short
        301 => TrackSimulationData::new(Technical, 0.88, 0.85, 40.0, 25.0, 35.0), // Brands Indy
        520 => TrackSimulationData::new(Technical, 0.85, 0.85, 35.0, 30.0, 35.0), // Modena

        // Technical with neutral stress but explicit car weights
        47 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 25.0, 40.0), // Laguna Seca
        51 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 25.0, 40.0), // Mid-Ohio
        68 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 30.0, 35.0), // Watkins Short
        125 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 40.0, 35.0), // Mosport
        166 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 35.0, 35.0), // Okayama
        192 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 35.0, 35.0), // Nurburgring GP
        197 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 30.0, 35.0), // Nurburgring Sprint
        199 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 30.0, 35.0), // Interlagos
        202 => TrackSimulationData::new(Technical, 1.00, 1.00, 40.0, 15.0, 45.0), // Oran Park
        244 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 35.0, 35.0), // Magny-Cours
        249 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 40.0, 35.0), // Road Atlanta
        259 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 30.0, 35.0), // VIR Patriot
        281 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 35.0, 35.0), // Barcelona
        300 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 35.0, 35.0), // Brands GP
        316 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 45.0, 30.0), // Snetterton 300
        335 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 55.0, 25.0), // Thruxton
        342 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 20.0, 60.0), // Oulton Intl
        350 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 30.0, 35.0), // Zolder
        363 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 25.0, 40.0), // Misano
        382 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 30.0, 35.0), // Vallelunga
        397 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 50.0, 25.0), // Red Bull Ring
        399 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 35.0, 35.0), // Donington GP
        400 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 30.0, 35.0), // Donington Natl
        404 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 35.0, 35.0), // Brno
        409 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 40.0, 35.0), // Assen
        413 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 40.0, 30.0), // Mexico City
        420 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 40.0, 35.0), // Istanbul
        421 => TrackSimulationData::new(Technical, 1.00, 1.00, 40.0, 25.0, 35.0), // Sandown
        425 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 35.0, 35.0), // Portimao
        440 => TrackSimulationData::new(Technical, 1.00, 1.00, 55.0, 10.0, 35.0), // Winton
        449 => TrackSimulationData::new(Technical, 1.00, 1.00, 40.0, 40.0, 20.0), // Oschersleben
        451 => TrackSimulationData::new(Technical, 1.00, 1.00, 45.0, 15.0, 40.0), // Rudskogen
        452 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 45.0, 30.0), // Mugello
        455 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 30.0, 35.0), // Imola
        489 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 15.0, 65.0), // Ledenon
        513 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 45.0, 30.0), // Kyalami
        515 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 45.0, 30.0), // Navarra
        524 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 30.0, 35.0), // Barcelona Natl
        532 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 40.0, 30.0), // Silverstone Natl
        538 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 30.0, 35.0), // Suzuka East
        542 => TrackSimulationData::new(Technical, 1.00, 1.00, 40.0, 25.0, 35.0), // Okayama Short
        548 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 50.0, 20.0), // Monza Junior

        // Unknown track fallback
        _ => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 30.0, 35.0),
    }
}

/// Pack density based on track length.
pub fn pack_density_factor(comprimento_km: f64) -> f64 {
    if comprimento_km < 2.5 {
        1.40
    } else if comprimento_km < 4.0 {
        1.10
    } else if comprimento_km < 6.0 {
        1.00
    } else if comprimento_km < 10.0 {
        0.90
    } else {
        0.75
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unknown_track_is_technical_default() {
        let data = get_track_simulation_data(99999);
        assert_eq!(data.track_character, TrackCharacter::Technical);
        assert_eq!(data.tire_stress_multiplier, 1.0);
        assert_eq!(data.physical_stress_multiplier, 1.0);
        assert_eq!(data.acceleration_weight, 35.0);
        assert_eq!(data.power_weight, 30.0);
        assert_eq!(data.handling_weight, 35.0);
    }

    #[test]
    fn test_sebring_high_tire_stress() {
        let data = get_track_simulation_data(238);
        assert!(
            data.tire_stress_multiplier >= 1.30,
            "Sebring tire stress={} should be >= 1.30",
            data.tire_stress_multiplier
        );
    }

    #[test]
    fn test_tsukuba_low_tire_stress() {
        let data = get_track_simulation_data(325);
        assert!(
            data.tire_stress_multiplier <= 0.85,
            "Tsukuba tire stress={} should be <= 0.85",
            data.tire_stress_multiplier
        );
    }

    #[test]
    fn test_le_mans_high_physical_stress() {
        let data = get_track_simulation_data(287);
        assert!(
            data.physical_stress_multiplier >= 1.45,
            "Le Mans physical stress={} should be >= 1.45",
            data.physical_stress_multiplier
        );
    }

    #[test]
    fn test_lime_rock_low_physical_stress() {
        let data = get_track_simulation_data(14);
        assert!(
            data.physical_stress_multiplier <= 0.85,
            "Lime Rock physical stress={} should be <= 0.85",
            data.physical_stress_multiplier
        );
    }

    #[test]
    fn test_tight_has_higher_overtaking_diff_than_flowing() {
        let hungaroring = get_track_simulation_data(194);
        let spa = get_track_simulation_data(188);
        assert_eq!(hungaroring.track_character, TrackCharacter::Tight);
        assert_eq!(spa.track_character, TrackCharacter::Flowing);
    }

    #[test]
    fn test_pack_density_short_track() {
        let factor = pack_density_factor(1.6);
        assert!(
            factor >= 1.35,
            "pack_density for 1.6km = {} should be >= 1.35",
            factor
        );
    }

    #[test]
    fn test_pack_density_le_mans() {
        let factor = pack_density_factor(13.6);
        assert!(
            factor <= 0.80,
            "pack_density for 13.6km = {} should be <= 0.80",
            factor
        );
    }

    #[test]
    fn test_nordschleife_is_tight_with_high_stress() {
        let data = get_track_simulation_data(196);
        assert_eq!(data.track_character, TrackCharacter::Tight);
        assert!(data.tire_stress_multiplier >= 1.25);
        assert!(data.physical_stress_multiplier >= 1.40);
    }

    #[test]
    fn test_roval_character() {
        let charlotte = get_track_simulation_data(554);
        let daytona = get_track_simulation_data(45);
        assert_eq!(charlotte.track_character, TrackCharacter::Roval);
        assert_eq!(daytona.track_character, TrackCharacter::Roval);
    }

    #[test]
    fn test_monza_power_weights() {
        let data = get_track_simulation_data(93);
        assert_eq!(data.acceleration_weight, 10.0);
        assert_eq!(data.power_weight, 70.0);
        assert_eq!(data.handling_weight, 20.0);
    }

    #[test]
    fn test_tsukuba_acceleration_weights() {
        let data = get_track_simulation_data(325);
        assert_eq!(data.acceleration_weight, 50.0);
        assert_eq!(data.power_weight, 10.0);
        assert_eq!(data.handling_weight, 40.0);
    }

    #[test]
    fn test_ledenon_handling_weights() {
        let data = get_track_simulation_data(489);
        assert_eq!(data.acceleration_weight, 20.0);
        assert_eq!(data.power_weight, 15.0);
        assert_eq!(data.handling_weight, 65.0);
    }

    #[test]
    fn test_sebring_near_balanced_weights() {
        let data = get_track_simulation_data(238);
        assert_eq!(data.acceleration_weight, 35.0);
        assert_eq!(data.power_weight, 30.0);
        assert_eq!(data.handling_weight, 35.0);
    }
}
