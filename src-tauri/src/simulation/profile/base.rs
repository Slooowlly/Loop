// ---------------------------------------------------------------------------
// Perfis base por família de carro / categoria
// ---------------------------------------------------------------------------

pub(super) struct BaseProfile {
    pub(super) tire_degradation_rate: f64,
    pub(super) physical_degradation_rate: f64,
    pub(super) incident_rate_multiplier: f64,
    pub(super) qualifying_variance_multiplier: f64,
    pub(super) race_variance_multiplier: f64,
    pub(super) start_chaos_multiplier: f64,
    pub(super) race_pace_spread_multiplier: f64,
    /// Velocidade média em ms por km usada para fallback de base_lap_time_ms.
    pub(super) ms_per_km_fallback: f64,
}

pub(super) fn base_profile_for(category_id: &str) -> BaseProfile {
    match category_id {
        // --- Rookie (MX-5 e GR86 entry level) ---
        "mazda_rookie" | "toyota_rookie" => BaseProfile {
            tire_degradation_rate: 0.025,
            physical_degradation_rate: 0.012,
            incident_rate_multiplier: 1.30,
            qualifying_variance_multiplier: 1.40,
            race_variance_multiplier: 1.40,
            start_chaos_multiplier: 1.50,
            race_pace_spread_multiplier: 1.30,
            ms_per_km_fallback: 27_500.0,
        },
        // --- Amador (MX-5 e GR86 championship, mais rodadas) ---
        "mazda_amador" | "toyota_amador" => BaseProfile {
            tire_degradation_rate: 0.023,
            physical_degradation_rate: 0.011,
            incident_rate_multiplier: 1.15,
            qualifying_variance_multiplier: 1.20,
            race_variance_multiplier: 1.20,
            start_chaos_multiplier: 1.30,
            race_pace_spread_multiplier: 1.15,
            ms_per_km_fallback: 27_000.0,
        },
        // --- BMW M2 CS (pro monomarca) ---
        "bmw_m2" => BaseProfile {
            tire_degradation_rate: 0.020,
            physical_degradation_rate: 0.010,
            incident_rate_multiplier: 1.05,
            qualifying_variance_multiplier: 1.00,
            race_variance_multiplier: 1.00,
            start_chaos_multiplier: 1.05,
            race_pace_spread_multiplier: 1.00,
            ms_per_km_fallback: 24_000.0,
        },
        // --- Production Challenger (multi-classe, usar BMW M2 como referência) ---
        "production_challenger" => BaseProfile {
            tire_degradation_rate: 0.021,
            physical_degradation_rate: 0.010,
            incident_rate_multiplier: 1.10,
            qualifying_variance_multiplier: 1.10,
            race_variance_multiplier: 1.10,
            start_chaos_multiplier: 1.20,
            race_pace_spread_multiplier: 1.10,
            ms_per_km_fallback: 24_000.0,
        },
        // --- GT4 ---
        "gt4" => BaseProfile {
            tire_degradation_rate: 0.020,
            physical_degradation_rate: 0.010,
            incident_rate_multiplier: 1.00,
            qualifying_variance_multiplier: 1.00,
            race_variance_multiplier: 1.00,
            start_chaos_multiplier: 1.00,
            race_pace_spread_multiplier: 1.00,
            ms_per_km_fallback: 22_000.0,
        },
        // --- GT3 ---
        "gt3" => BaseProfile {
            tire_degradation_rate: 0.018,
            physical_degradation_rate: 0.009,
            incident_rate_multiplier: 0.85,
            qualifying_variance_multiplier: 0.80,
            race_variance_multiplier: 0.80,
            start_chaos_multiplier: 0.80,
            race_pace_spread_multiplier: 0.85,
            ms_per_km_fallback: 20_000.0,
        },
        // --- Endurance (multi-classe, referência LMP2) ---
        "endurance" => BaseProfile {
            tire_degradation_rate: 0.030,
            physical_degradation_rate: 0.020,
            incident_rate_multiplier: 1.10,
            qualifying_variance_multiplier: 0.90,
            race_variance_multiplier: 0.90,
            start_chaos_multiplier: 0.70,
            race_pace_spread_multiplier: 0.90,
            ms_per_km_fallback: 17_000.0,
        },
        // Fallback neutro — não representa categoria real
        _ => BaseProfile {
            tire_degradation_rate: 0.020,
            physical_degradation_rate: 0.010,
            incident_rate_multiplier: 1.00,
            qualifying_variance_multiplier: 1.00,
            race_variance_multiplier: 1.00,
            start_chaos_multiplier: 1.00,
            race_pace_spread_multiplier: 1.00,
            ms_per_km_fallback: 22_000.0,
        },
    }
}

// ---------------------------------------------------------------------------
// Família de carro para lookup de tempos
// ---------------------------------------------------------------------------

pub(super) fn car_family_for(category_id: &str) -> &'static str {
    match category_id {
        "mazda_rookie" | "mazda_amador" => "mx5",
        "toyota_rookie" | "toyota_amador" => "gr86",
        "bmw_m2" | "production_challenger" => "bmw_m2",
        "gt4" => "gt4",
        "gt3" => "gt3",
        "lmp2" => "lmp2",
        "endurance" => "gt3",
        _ => "gt4",
    }
}
