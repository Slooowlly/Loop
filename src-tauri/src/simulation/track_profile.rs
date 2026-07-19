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
    // Chaveado pelos ids reais do iRacing (ver constants/tracks.rs).
    match track_id {
        // Rovals
        554 => TrackSimulationData::new(Roval, 0.95, 0.90, 50.0, 30.0, 20.0), // Charlotte Roval
        192 => TrackSimulationData::new(Roval, 0.95, 0.90, 30.0, 50.0, 20.0), // Daytona Road
        448 => TrackSimulationData::new(Roval, 1.05, 0.95, 30.0, 50.0, 20.0), // Indianapolis Road (retas → potência)

        // Flowing
        523 => TrackSimulationData::new(Flowing, 1.25, 1.25, 15.0, 60.0, 25.0), // Spa
        18 => TrackSimulationData::new(Flowing, 1.10, 1.20, 20.0, 50.0, 30.0),  // Road America
        341 => TrackSimulationData::new(Flowing, 1.20, 1.15, 15.0, 55.0, 30.0), // Silverstone GP
        239 => TrackSimulationData::new(Flowing, 1.00, 1.00, 10.0, 70.0, 20.0), // Monza
        152 => TrackSimulationData::new(Flowing, 1.00, 1.05, 15.0, 50.0, 35.0), // Phillip Island
        390 => TrackSimulationData::new(Flowing, 0.95, 0.95, 20.0, 60.0, 20.0), // Hockenheim GP
        566 => TrackSimulationData::new(Flowing, 1.00, 1.00, 30.0, 25.0, 45.0), // Sonoma
        444 => TrackSimulationData::new(Flowing, 1.00, 1.05, 25.0, 50.0, 25.0), // Fuji
        485 => TrackSimulationData::new(Flowing, 1.00, 1.00, 20.0, 50.0, 30.0), // Zandvoort

        // Tight
        249 => TrackSimulationData::new(Tight, 1.30, 1.45, 20.0, 30.0, 50.0), // Nordschleife
        527 => TrackSimulationData::new(Tight, 1.10, 1.15, 30.0, 15.0, 55.0), // Cadwell Park
        413 => TrackSimulationData::new(Tight, 1.05, 1.10, 45.0, 10.0, 45.0), // Hungaroring
        324 => TrackSimulationData::new(Tight, 0.82, 0.80, 50.0, 10.0, 40.0), // Tsukuba
        179 => TrackSimulationData::new(Tight, 1.05, 1.05, 55.0, 10.0, 35.0), // Long Beach
        319 => TrackSimulationData::new(Tight, 1.05, 1.05, 50.0, 15.0, 35.0), // Detroit
        181 => TrackSimulationData::new(Tight, 1.00, 1.05, 55.0, 25.0, 20.0), // Oulton Fosters
        182 => TrackSimulationData::new(Tight, 1.00, 1.05, 50.0, 15.0, 35.0), // Oulton Island

        // Technical with explicit stress tuning
        95 => TrackSimulationData::new(Technical, 1.35, 1.30, 35.0, 30.0, 35.0), // Sebring (all-rounder de referência)
        252 => TrackSimulationData::new(Technical, 1.20, 1.60, 20.0, 25.0, 55.0), // Nurburgring 24H
        268 => TrackSimulationData::new(Technical, 1.20, 1.50, 15.0, 65.0, 20.0), // Le Mans
        219 => TrackSimulationData::new(Technical, 1.20, 1.30, 20.0, 50.0, 30.0), // Bathurst
        168 => TrackSimulationData::new(Technical, 1.15, 1.25, 20.0, 30.0, 50.0), // Suzuka
        229 => TrackSimulationData::new(Technical, 1.20, 1.15, 25.0, 45.0, 30.0), // COTA
        465 => TrackSimulationData::new(Technical, 1.25, 1.20, 25.0, 25.0, 50.0), // VIR Full
        353 => TrackSimulationData::new(Technical, 0.85, 0.82, 50.0, 20.0, 30.0), // Lime Rock
        9 => TrackSimulationData::new(Technical, 0.85, 0.85, 50.0, 20.0, 30.0),   // Summit Point

        // Technical with neutral stress but explicit car weights
        586 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 20.0, 50.0), // Laguna Seca
        153 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 20.0, 50.0), // Mid-Ohio
        433 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 45.0, 30.0), // Watkins Cup
        144 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 50.0, 30.0), // Mosport
        166 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 25.0, 45.0), // Okayama
        212 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 45.0, 30.0), // Interlagos
        202 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 15.0, 50.0), // Oran Park
        463 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 25.0, 45.0), // Magny-Cours
        127 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 50.0, 30.0), // Road Atlanta
        345 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 25.0, 45.0), // Barcelona
        145 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 45.0, 30.0), // Brands GP
        297 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 50.0, 30.0), // Snetterton 300
        532 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 55.0, 25.0), // Thruxton
        180 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 20.0, 60.0), // Oulton Intl
        199 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 25.0, 40.0), // Zolder
        501 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 20.0, 50.0), // Misano
        403 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 50.0, 25.0), // Red Bull Ring
        233 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 25.0, 45.0), // Donington GP
        572 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 50.0, 25.0), // Mexico City
        443 => TrackSimulationData::new(Technical, 1.00, 1.00, 50.0, 20.0, 30.0), // Sandown
        509 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 25.0, 45.0), // Portimao / Algarve GP
        440 => TrackSimulationData::new(Technical, 1.00, 1.00, 55.0, 10.0, 35.0), // Winton
        449 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 50.0, 20.0), // Oschersleben
        451 => TrackSimulationData::new(Technical, 1.00, 1.00, 50.0, 15.0, 35.0), // Rudskogen
        498 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 50.0, 30.0), // Mugello
        266 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 25.0, 40.0), // Imola
        489 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 15.0, 65.0), // Ledenon
        515 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 50.0, 30.0), // Navarra
        346 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 25.0, 45.0), // Barcelona Natl
        343 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 50.0, 25.0), // Silverstone Natl
        167 => TrackSimulationData::new(Technical, 1.00, 1.00, 50.0, 20.0, 30.0), // Okayama Short

        // ── Layouts secundários: herdam o caráter do layout principal do venue ──
        510 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 25.0, 45.0), // Algarve GP Chicanes (← 509)
        574 => TrackSimulationData::new(Technical, 1.00, 1.00, 25.0, 50.0, 25.0), // Hermanos National (← 572)
        499 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 50.0, 30.0), // Mugello Short (← 498)
        247 => TrackSimulationData::new(Flowing, 1.00, 1.00, 10.0, 70.0, 20.0), // Monza Combined (← 239)
        244 => TrackSimulationData::new(Flowing, 1.00, 1.00, 10.0, 70.0, 20.0), // Monza s/ 1ª chicane (← 239)
        242 => TrackSimulationData::new(Flowing, 1.00, 1.00, 10.0, 70.0, 20.0), // Monza s/ chicanes (← 239)
        525 => TrackSimulationData::new(Flowing, 1.25, 1.25, 15.0, 60.0, 25.0), // Spa Endurance (← 523)
        487 => TrackSimulationData::new(Flowing, 1.00, 1.00, 20.0, 50.0, 30.0), // Zandvoort Nationaal (← 485)
        486 => TrackSimulationData::new(Flowing, 1.00, 1.00, 20.0, 50.0, 30.0), // Zandvoort w/chicane (← 485)
        516 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 50.0, 30.0), // Navarra Medium (← 515)
        517 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 50.0, 30.0), // Navarra Short (← 515)
        445 => TrackSimulationData::new(Flowing, 1.00, 1.05, 25.0, 50.0, 25.0), // Fuji No Chicane (← 444)
        352 => TrackSimulationData::new(Technical, 0.85, 0.82, 50.0, 20.0, 30.0), // Lime Rock Classic (← 353)
        354 => TrackSimulationData::new(Technical, 0.85, 0.82, 50.0, 20.0, 30.0), // Lime Rock Chicanes (← 353)
        154 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 20.0, 50.0), // Mid-Ohio Chicane (← 153)
        454 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 50.0, 20.0), // Oschersleben Alt (← 449)
        455 => TrackSimulationData::new(Technical, 1.00, 1.00, 30.0, 50.0, 20.0), // Oschersleben B (← 449)
        208 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 15.0, 50.0), // Oran Park South (← 202)
        207 => TrackSimulationData::new(Technical, 1.00, 1.00, 35.0, 15.0, 50.0), // Oran Park North (← 202)
        183 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 20.0, 60.0), // Oulton w/out Hislop (← 180)
        184 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 20.0, 60.0), // Oulton w/out Brittens (← 180)
        185 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 20.0, 60.0), // Oulton w/no Chicanes (← 180)
        342 => TrackSimulationData::new(Flowing, 1.20, 1.15, 15.0, 55.0, 30.0), // Silverstone Intl (← 341)
        298 => TrackSimulationData::new(Technical, 1.00, 1.00, 20.0, 50.0, 30.0), // Snetterton 200 (← 297)
        567 => TrackSimulationData::new(Flowing, 1.00, 1.00, 30.0, 25.0, 45.0), // Sonoma Cup Short (← 566)
        467 => TrackSimulationData::new(Technical, 1.25, 1.20, 25.0, 25.0, 50.0), // VIR North (← 465)
        468 => TrackSimulationData::new(Technical, 1.25, 1.20, 25.0, 25.0, 50.0), // VIR South (← 465)
        466 => TrackSimulationData::new(Technical, 1.25, 1.20, 25.0, 25.0, 50.0), // VIR Grand (← 465)
        439 => TrackSimulationData::new(Technical, 1.00, 1.00, 55.0, 10.0, 35.0), // Winton National (← 440)
        263 => TrackSimulationData::new(Technical, 1.05, 1.10, 20.0, 50.0, 30.0), // Nürburgring Combined Short (GP-like)

        // ── Venues novos: perfis pesquisados por circuito (jul/2026) ──
        580 => TrackSimulationData::new(Technical, 1.05, 1.15, 35.0, 45.0, 20.0), // Adelaide (retas longas → potência)
        585 => TrackSimulationData::new(Flowing, 1.10, 1.25, 25.0, 15.0, 60.0), // Barber (sweepers → dirigibilidade)
        405 => TrackSimulationData::new(Tight, 0.95, 1.10, 60.0, 25.0, 15.0), // Chicago Street (90° → aceleração)
        218 => TrackSimulationData::new(Technical, 1.05, 1.05, 25.0, 60.0, 15.0), // Gilles Villeneuve (retas → potência)
        473 => TrackSimulationData::new(Technical, 1.20, 1.15, 30.0, 15.0, 55.0), // Jerez (fluído MotoGP → dirigibilidade)
        423 => TrackSimulationData::new(Technical, 1.05, 1.15, 38.0, 25.0, 37.0), // Knockhill (curto, ondulado)
        539 => TrackSimulationData::new(Technical, 1.00, 1.10, 30.0, 45.0, 25.0), // Miami (retas → potência)
        195 => TrackSimulationData::new(Technical, 1.15, 1.10, 60.0, 25.0, 15.0), // Motegi (stop-and-go → aceleração)
        475 => TrackSimulationData::new(Flowing, 1.10, 1.20, 20.0, 30.0, 50.0), // Aragón GP (fluído → dirigibilidade)
        476 => TrackSimulationData::new(Flowing, 1.08, 1.12, 20.0, 30.0, 50.0), // Aragón National (← GP, dirigibilidade)
        536 => TrackSimulationData::new(Technical, 0.95, 0.90, 35.0, 50.0, 15.0), // Portland (reta longa → potência)
        589 => TrackSimulationData::new(Technical, 1.05, 1.10, 30.0, 55.0, 15.0), // Coronado (retas de pista → potência)
        521 => TrackSimulationData::new(Tight, 1.10, 1.05, 25.0, 10.0, 65.0), // Sachsenring (twisty → dirigibilidade)
        540 => TrackSimulationData::new(Flowing, 1.10, 1.00, 15.0, 30.0, 55.0), // The Bend GT (fluído → dirigibilidade)
        541 => TrackSimulationData::new(Flowing, 1.15, 1.30, 20.0, 45.0, 35.0), // The Bend Intl (7,7 km, retas → potência)
        584 => TrackSimulationData::new(Tight, 1.05, 1.20, 35.0, 50.0, 15.0), // St. Petersburg (runway → potência)
        481 => TrackSimulationData::new(Flowing, 1.30, 1.25, 10.0, 35.0, 55.0), // Willow Springs (sweepers → dirigibilidade)

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
        let data = get_track_simulation_data(95);
        assert!(
            data.tire_stress_multiplier >= 1.30,
            "Sebring tire stress={} should be >= 1.30",
            data.tire_stress_multiplier
        );
    }

    #[test]
    fn test_tsukuba_low_tire_stress() {
        let data = get_track_simulation_data(324);
        assert!(
            data.tire_stress_multiplier <= 0.85,
            "Tsukuba tire stress={} should be <= 0.85",
            data.tire_stress_multiplier
        );
    }

    #[test]
    fn test_le_mans_high_physical_stress() {
        let data = get_track_simulation_data(268);
        assert!(
            data.physical_stress_multiplier >= 1.45,
            "Le Mans physical stress={} should be >= 1.45",
            data.physical_stress_multiplier
        );
    }

    #[test]
    fn test_lime_rock_low_physical_stress() {
        let data = get_track_simulation_data(353);
        assert!(
            data.physical_stress_multiplier <= 0.85,
            "Lime Rock physical stress={} should be <= 0.85",
            data.physical_stress_multiplier
        );
    }

    #[test]
    fn test_tight_has_higher_overtaking_diff_than_flowing() {
        let hungaroring = get_track_simulation_data(413);
        let spa = get_track_simulation_data(523);
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
        let data = get_track_simulation_data(249);
        assert_eq!(data.track_character, TrackCharacter::Tight);
        assert!(data.tire_stress_multiplier >= 1.25);
        assert!(data.physical_stress_multiplier >= 1.40);
    }

    #[test]
    fn test_roval_character() {
        let charlotte = get_track_simulation_data(554);
        let daytona = get_track_simulation_data(192);
        assert_eq!(charlotte.track_character, TrackCharacter::Roval);
        assert_eq!(daytona.track_character, TrackCharacter::Roval);
    }

    #[test]
    fn test_monza_power_weights() {
        let data = get_track_simulation_data(239);
        assert_eq!(data.acceleration_weight, 10.0);
        assert_eq!(data.power_weight, 70.0);
        assert_eq!(data.handling_weight, 20.0);
    }

    #[test]
    fn test_tsukuba_acceleration_weights() {
        let data = get_track_simulation_data(324);
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
        let data = get_track_simulation_data(95);
        assert_eq!(data.acceleration_weight, 35.0);
        assert_eq!(data.power_weight, 30.0);
        assert_eq!(data.handling_weight, 35.0);
    }

    /// Depois do retune, o grid tem que ser majoritariamente peaked (foco importa);
    /// balanceado é minoria. Varre todas as pistas nomeadas e conta o spread.
    #[test]
    fn test_peaked_tracks_are_the_majority() {
        let ids = [
            554, 192, 448, 523, 18, 341, 239, 152, 390, 566, 444, 485, 249, 527, 413, 324, 179,
            319, 181, 182, 95, 252, 268, 219, 168, 229, 465, 353, 9, 586, 153, 433, 144, 166, 212,
            202, 463, 127, 345, 145, 297, 532, 180, 199, 501, 403, 233, 572, 443, 509, 440, 449,
            451, 498, 266, 489, 515, 346, 343, 167,
        ];
        let mut peaked = 0usize;
        let mut balanced = 0usize;
        for id in ids {
            let d = get_track_simulation_data(id);
            let mx = d
                .acceleration_weight
                .max(d.power_weight)
                .max(d.handling_weight);
            let mn = d
                .acceleration_weight
                .min(d.power_weight)
                .min(d.handling_weight);
            if mx - mn >= 15.0 {
                peaked += 1;
            } else {
                balanced += 1;
            }
        }
        assert!(
            peaked > balanced * 4,
            "peaked={} deveria dominar balanced={} (balanceado é secundário)",
            peaked,
            balanced
        );
    }

    /// Empates 30/35/35 antigos foram resolvidos por caráter real do circuito.
    #[test]
    fn test_former_balanced_tracks_now_lean() {
        // Interlagos: reta oposta + altitude => potência.
        let inter = get_track_simulation_data(212);
        assert!(inter.power_weight > inter.acceleration_weight);
        assert!(inter.power_weight > inter.handling_weight);
        // Laguna Seca: técnica lenta => handling.
        let laguna = get_track_simulation_data(586);
        assert!(laguna.handling_weight > laguna.power_weight);
        assert!(laguna.handling_weight > laguna.acceleration_weight);
        // Okayama Short: clube apertado => aceleração.
        let okayama = get_track_simulation_data(167);
        assert!(okayama.acceleration_weight > okayama.power_weight);
        assert!(okayama.acceleration_weight > okayama.handling_weight);
    }
}
