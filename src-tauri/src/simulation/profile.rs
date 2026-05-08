use crate::constants::tracks::get_track;
use crate::models::enums::WeatherCondition;
use crate::simulation::track_profile::{get_track_simulation_data, TrackCharacter};

/// Perfil de simulação canônico para uma corrida.
/// Centraliza todos os multiplicadores e parâmetros de tuning.
/// Resolvido uma vez por corrida e injetado no SimulationContext.
#[derive(Debug, Clone)]
pub struct SimulationProfile {
    /// Tempo base de volta em ms (do pole sitter ideal).
    pub base_lap_time_ms: f64,
    /// Taxa de desgaste de pneu por segmento.
    pub tire_degradation_rate: f64,
    /// Taxa de desgaste físico por segmento.
    pub physical_degradation_rate: f64,
    /// Multiplicador global de taxa de incidentes.
    pub incident_rate_multiplier: f64,
    /// Escala da variância no qualifying.
    pub qualifying_variance_multiplier: f64,
    /// Escala da variância no score de corrida.
    pub race_variance_multiplier: f64,
    /// Amplifica (>1.0) ou atenua (<1.0) o efeito da chuva.
    pub rain_sensitivity: f64,
    /// Amplifica caos adicional na largada (colisões/erros no Start).
    pub start_chaos_multiplier: f64,
    /// Dificuldade da pista (>1.0 = mais exigente, adaptabilidade vale mais).
    pub track_difficulty_multiplier: f64,
    /// Dificuldade de ultrapassagem (>1.0 = mais difícil).
    pub overtaking_difficulty_multiplier: f64,
    /// Spread de pace entre pilotos (>1.0 = mais separação).
    pub race_pace_spread_multiplier: f64,
    /// Caráter esportivo da pista (determina pesos de atributos).
    pub track_character: TrackCharacter,
}

// ---------------------------------------------------------------------------
// Perfis base por família de carro / categoria
// ---------------------------------------------------------------------------

struct BaseProfile {
    tire_degradation_rate: f64,
    physical_degradation_rate: f64,
    incident_rate_multiplier: f64,
    qualifying_variance_multiplier: f64,
    race_variance_multiplier: f64,
    start_chaos_multiplier: f64,
    race_pace_spread_multiplier: f64,
    /// Velocidade média em ms por km usada para fallback de base_lap_time_ms.
    ms_per_km_fallback: f64,
}

fn base_profile_for(category_id: &str) -> BaseProfile {
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

fn car_family_for(category_id: &str) -> &'static str {
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

// ---------------------------------------------------------------------------
// Tabela de tempos base por (família de carro, track_id)
// Fonte: tabela de referência do projeto (convertida de MM:SS para ms)
//
// Categorias mapeadas:
//   mx5    → mazda_rookie / mazda_amador
//   gr86   → toyota_rookie / toyota_amador
//   bmw_m2 → bmw_m2 / production_challenger
//   gt4    → gt4
//   gt3    → gt3
//   lmp2   → lmp2
// ---------------------------------------------------------------------------

fn base_lap_time_ms_for(car_family: &str, track_id: u32) -> Option<f64> {
    match (car_family, track_id) {
        // --- Charlotte Roval (554) ---
        ("mx5", 554) => Some(101_000.0),
        ("gr86", 554) => Some(98_000.0),
        ("bmw_m2", 554) => Some(85_000.0),
        ("gt4", 554) => Some(79_000.0),
        ("gt3", 554) => Some(72_000.0),
        ("lmp2", 554) => Some(63_000.0),

        // --- Lime Rock (14) ---
        ("mx5", 14) => Some(60_000.0),
        ("gr86", 14) => Some(58_000.0),
        ("bmw_m2", 14) => Some(49_000.0),
        ("gt4", 14) => Some(45_000.0),
        ("gt3", 14) => Some(40_000.0),
        ("lmp2", 14) => Some(35_000.0),

        // --- Laguna Seca (47) ---
        ("mx5", 47) => Some(100_000.0),
        ("gr86", 47) => Some(97_000.0),
        ("bmw_m2", 47) => Some(84_000.0),
        ("gt4", 47) => Some(77_000.0),
        ("gt3", 47) => Some(71_000.0),
        ("lmp2", 47) => Some(62_000.0),

        // --- Okayama (166) ---
        ("mx5", 166) => Some(107_000.0),
        ("gr86", 166) => Some(102_000.0),
        ("bmw_m2", 166) => Some(86_000.0),
        ("gt4", 166) => Some(79_000.0),
        ("gt3", 166) => Some(72_000.0),
        ("lmp2", 166) => Some(63_000.0),

        // --- Oulton Park Fosters (261) ---
        ("mx5", 261) => Some(115_000.0),
        ("gr86", 261) => Some(111_000.0),
        ("bmw_m2", 261) => Some(100_000.0),
        ("gt4", 261) => Some(93_000.0),
        ("gt3", 261) => Some(85_000.0),
        ("lmp2", 261) => Some(74_000.0),

        // --- Oulton Park Intl (342) ---
        ("mx5", 342) => Some(115_000.0),
        ("gr86", 342) => Some(111_000.0),
        ("bmw_m2", 342) => Some(100_000.0),
        ("gt4", 342) => Some(93_000.0),
        ("gt3", 342) => Some(85_000.0),
        ("lmp2", 342) => Some(74_000.0),

        // --- Oulton Park Island (341) ---
        ("mx5", 341) => Some(110_000.0),
        ("gr86", 341) => Some(106_000.0),
        ("bmw_m2", 341) => Some(96_000.0),
        ("gt4", 341) => Some(89_000.0),
        ("gt3", 341) => Some(82_000.0),
        ("lmp2", 341) => Some(71_000.0),

        // --- Summit Point (9) ---
        ("mx5", 9) => Some(80_000.0),
        ("gr86", 9) => Some(78_000.0),
        ("bmw_m2", 9) => Some(65_000.0),
        ("gt4", 9) => Some(60_000.0),
        ("gt3", 9) => Some(54_000.0),
        ("lmp2", 9) => Some(46_000.0),

        // --- Summit Point Jefferson (8) — pista menor, ~60% do circuito principal ---
        ("mx5", 8) => Some(52_000.0),
        ("gr86", 8) => Some(50_000.0),
        ("bmw_m2", 8) => Some(43_000.0),
        ("gt4", 8) => Some(39_000.0),
        ("gt3", 8) => Some(35_000.0),
        ("lmp2", 8) => Some(30_000.0),

        // --- Tsukuba (325) ---
        ("mx5", 325) => Some(69_000.0),
        ("gr86", 325) => Some(65_000.0),
        ("bmw_m2", 325) => Some(57_000.0),
        ("gt4", 325) => Some(53_000.0),
        ("gt3", 325) => Some(46_000.0),
        ("lmp2", 325) => Some(44_000.0),

        // --- Brands Hatch GP (300) ---
        ("mx5", 300) => Some(108_000.0),
        ("gr86", 300) => Some(104_000.0),
        ("bmw_m2", 300) => Some(91_000.0),
        ("gt4", 300) => Some(84_000.0),
        ("gt3", 300) => Some(77_000.0),
        ("lmp2", 300) => Some(67_000.0),

        // --- Brands Hatch Indy (301) ---
        ("mx5", 301) => Some(68_000.0),
        ("gr86", 301) => Some(66_000.0),
        ("bmw_m2", 301) => Some(57_000.0),
        ("gt4", 301) => Some(53_000.0),
        ("gt3", 301) => Some(48_000.0),
        ("lmp2", 301) => Some(42_000.0),

        // --- Daytona Road (45) ---
        ("mx5", 45) => Some(139_000.0),
        ("gr86", 45) => Some(135_000.0),
        ("bmw_m2", 45) => Some(116_000.0),
        ("gt4", 45) => Some(106_000.0),
        ("gt3", 45) => Some(96_000.0),
        ("lmp2", 45) => Some(83_000.0),

        // --- Mid-Ohio (51) ---
        ("mx5", 51) => Some(102_000.0),
        ("gr86", 51) => Some(98_000.0),
        ("bmw_m2", 51) => Some(84_000.0),
        ("gt4", 51) => Some(78_000.0),
        ("gt3", 51) => Some(71_000.0),
        ("lmp2", 51) => Some(62_000.0),

        // --- Road America (52) ---
        ("mx5", 52) => Some(156_000.0),
        ("gr86", 52) => Some(151_000.0),
        ("bmw_m2", 52) => Some(132_000.0),
        ("gt4", 52) => Some(121_000.0),
        ("gt3", 52) => Some(110_000.0),
        ("lmp2", 52) => Some(94_000.0),

        // --- Sonoma (53) ---
        ("mx5", 53) => Some(109_000.0),
        ("gr86", 53) => Some(105_000.0),
        ("bmw_m2", 53) => Some(90_000.0),
        ("gt4", 53) => Some(83_000.0),
        ("gt3", 53) => Some(76_000.0),
        ("lmp2", 53) => Some(66_000.0),

        // --- VIR Full (58) ---
        ("mx5", 58) => Some(140_000.0),
        ("gr86", 58) => Some(135_000.0),
        ("bmw_m2", 58) => Some(113_000.0),
        ("gt4", 58) => Some(104_000.0),
        ("gt3", 58) => Some(97_000.0),
        ("lmp2", 58) => Some(84_000.0),

        // --- Watkins Glen Boot (67) ---
        ("mx5", 67) => Some(132_000.0),
        ("gr86", 67) => Some(128_000.0),
        ("bmw_m2", 67) => Some(110_000.0),
        ("gt4", 67) => Some(101_000.0),
        ("gt3", 67) => Some(91_000.0),
        ("lmp2", 67) => Some(79_000.0),

        // --- Watkins Glen Short (68) ---
        ("mx5", 68) => Some(105_000.0),
        ("gr86", 68) => Some(101_000.0),
        ("bmw_m2", 68) => Some(88_000.0),
        ("gt4", 68) => Some(81_000.0),
        ("gt3", 68) => Some(74_000.0),
        ("lmp2", 68) => Some(64_000.0),

        // --- Monza (93) ---
        ("mx5", 93) => Some(134_000.0),
        ("gr86", 93) => Some(130_000.0),
        ("bmw_m2", 93) => Some(117_000.0),
        ("gt4", 93) => Some(107_000.0),
        ("gt3", 93) => Some(97_000.0),
        ("lmp2", 93) => Some(84_000.0),

        // --- Silverstone GP (106) ---
        ("mx5", 106) => Some(141_000.0),
        ("gr86", 106) => Some(137_000.0),
        ("bmw_m2", 106) => Some(119_000.0),
        ("gt4", 106) => Some(110_000.0),
        ("gt3", 106) => Some(99_000.0),
        ("lmp2", 106) => Some(86_000.0),

        // --- Bathurst / Mount Panorama (119) ---
        ("mx5", 119) => Some(172_000.0),
        ("gr86", 119) => Some(167_000.0),
        ("bmw_m2", 119) => Some(144_000.0),
        ("gt4", 119) => Some(134_000.0),
        ("gt3", 119) => Some(123_000.0),
        ("lmp2", 119) => Some(107_000.0),

        // --- Mosport / CTMP (125) ---
        ("mx5", 125) => Some(106_000.0),
        ("gr86", 125) => Some(102_000.0),
        ("bmw_m2", 125) => Some(92_000.0),
        ("gt4", 125) => Some(85_000.0),
        ("gt3", 125) => Some(78_000.0),
        ("lmp2", 125) => Some(68_000.0),

        // --- Suzuka (164) ---
        ("mx5", 164) => Some(152_000.0),
        ("gr86", 164) => Some(147_000.0),
        ("bmw_m2", 164) => Some(135_000.0),
        ("gt4", 164) => Some(126_000.0),
        ("gt3", 164) => Some(115_000.0),
        ("lmp2", 164) => Some(100_000.0),

        // --- Philip Island (169) ---
        ("mx5", 169) => Some(110_000.0),
        ("gr86", 169) => Some(107_000.0),
        ("bmw_m2", 169) => Some(90_000.0),
        ("gt4", 169) => Some(83_000.0),
        ("gt3", 169) => Some(75_000.0),
        ("lmp2", 169) => Some(64_000.0),

        // --- Indy Road (185) ---
        ("mx5", 185) => Some(105_000.0),
        ("gr86", 185) => Some(101_000.0),
        ("bmw_m2", 185) => Some(90_000.0),
        ("gt4", 185) => Some(84_000.0),
        ("gt3", 185) => Some(77_000.0),
        ("lmp2", 185) => Some(67_000.0),

        // --- Spa (188) ---
        ("mx5", 188) => Some(168_000.0),
        ("gr86", 188) => Some(163_000.0),
        ("bmw_m2", 188) => Some(142_000.0),
        ("gt4", 188) => Some(130_000.0),
        ("gt3", 188) => Some(117_000.0),
        ("lmp2", 188) => Some(101_000.0),

        // --- Nürburgring GP (192) ---
        ("mx5", 192) => Some(137_000.0),
        ("gr86", 192) => Some(133_000.0),
        ("bmw_m2", 192) => Some(120_000.0),
        ("gt4", 192) => Some(111_000.0),
        ("gt3", 192) => Some(102_000.0),
        ("lmp2", 192) => Some(88_000.0),

        // --- Hockenheim GP (193) ---
        ("mx5", 193) => Some(113_000.0),
        ("gr86", 193) => Some(110_000.0),
        ("bmw_m2", 193) => Some(92_000.0),
        ("gt4", 193) => Some(85_000.0),
        ("gt3", 193) => Some(77_000.0),
        ("lmp2", 193) => Some(66_000.0),

        // --- Hungaroring (194) ---
        ("mx5", 194) => Some(134_000.0),
        ("gr86", 194) => Some(128_000.0),
        ("bmw_m2", 194) => Some(115_000.0),
        ("gt4", 194) => Some(107_000.0),
        ("gt3", 194) => Some(101_000.0),
        ("lmp2", 194) => Some(91_000.0),

        // --- Hockenheim Short (195) ---
        ("mx5", 195) => Some(80_000.0),
        ("gr86", 195) => Some(77_000.0),
        ("bmw_m2", 195) => Some(67_000.0),
        ("gt4", 195) => Some(62_000.0),
        ("gt3", 195) => Some(56_000.0),
        ("lmp2", 195) => Some(49_000.0),

        // --- Nordschleife (196) ---
        ("mx5", 196) => Some(586_000.0),
        ("gr86", 196) => Some(566_000.0),
        ("bmw_m2", 196) => Some(484_000.0),
        ("gt4", 196) => Some(446_000.0),
        ("gt3", 196) => Some(410_000.0),
        ("lmp2", 196) => Some(357_000.0),

        // --- Nürburgring Sprint (197) ---
        ("mx5", 197) => Some(103_000.0),
        ("gr86", 197) => Some(99_000.0),
        ("bmw_m2", 197) => Some(87_000.0),
        ("gt4", 197) => Some(80_000.0),
        ("gt3", 197) => Some(73_000.0),
        ("lmp2", 197) => Some(63_000.0),

        // --- Interlagos (199) ---
        ("mx5", 199) => Some(119_000.0),
        ("gr86", 199) => Some(115_000.0),
        ("bmw_m2", 199) => Some(100_000.0),
        ("gt4", 199) => Some(93_000.0),
        ("gt3", 199) => Some(85_000.0),
        ("lmp2", 199) => Some(73_000.0),

        // --- COTA (212) ---
        ("mx5", 212) => Some(144_000.0),
        ("gr86", 212) => Some(139_000.0),
        ("bmw_m2", 212) => Some(128_000.0),
        ("gt4", 212) => Some(119_000.0),
        ("gt3", 212) => Some(109_000.0),
        ("lmp2", 212) => Some(95_000.0),

        // --- Sebring (238) ---
        ("mx5", 238) => Some(164_000.0),
        ("gr86", 238) => Some(158_000.0),
        ("bmw_m2", 238) => Some(140_000.0),
        ("gt4", 238) => Some(130_000.0),
        ("gt3", 238) => Some(119_000.0),
        ("lmp2", 238) => Some(104_000.0),

        // --- Magny-Cours (244) ---
        ("mx5", 244) => Some(118_000.0),
        ("gr86", 244) => Some(114_000.0),
        ("bmw_m2", 244) => Some(102_000.0),
        ("gt4", 244) => Some(95_000.0),
        ("gt3", 244) => Some(87_000.0),
        ("lmp2", 244) => Some(76_000.0),

        // --- Road Atlanta (249) ---
        ("mx5", 249) => Some(109_000.0),
        ("gr86", 249) => Some(106_000.0),
        ("bmw_m2", 249) => Some(95_000.0),
        ("gt4", 249) => Some(88_000.0),
        ("gt3", 249) => Some(81_000.0),
        ("lmp2", 249) => Some(70_000.0),

        // --- VIR Patriot (259) ---
        ("mx5", 259) => Some(96_000.0),
        ("gr86", 259) => Some(92_000.0),
        ("bmw_m2", 259) => Some(80_000.0),
        ("gt4", 259) => Some(74_000.0),
        ("gt3", 259) => Some(67_000.0),
        ("lmp2", 259) => Some(58_000.0),

        // --- Barcelona GP (281) ---
        ("mx5", 281) => Some(122_000.0),
        ("gr86", 281) => Some(118_000.0),
        ("bmw_m2", 281) => Some(108_000.0),
        ("gt4", 281) => Some(100_000.0),
        ("gt3", 281) => Some(93_000.0),
        ("lmp2", 281) => Some(80_000.0),

        // --- Le Mans (287) ---
        ("mx5", 287) => Some(316_000.0),
        ("gr86", 287) => Some(307_000.0),
        ("bmw_m2", 287) => Some(276_000.0),
        ("gt4", 287) => Some(253_000.0),
        ("gt3", 287) => Some(228_000.0),
        ("lmp2", 287) => Some(200_000.0),

        // --- Snetterton 300 (316) ---
        ("mx5", 316) => Some(119_000.0),
        ("gr86", 316) => Some(116_000.0),
        ("bmw_m2", 316) => Some(104_000.0),
        ("gt4", 316) => Some(95_000.0),
        ("gt3", 316) => Some(87_000.0),
        ("lmp2", 316) => Some(74_000.0),

        // --- Long Beach (318) ---
        ("mx5", 318) => Some(102_000.0),
        ("gr86", 318) => Some(97_000.0),
        ("bmw_m2", 318) => Some(88_000.0),
        ("gt4", 318) => Some(82_000.0),
        ("gt3", 318) => Some(73_000.0),
        ("lmp2", 318) => Some(65_000.0),

        // --- Thruxton (335) ---
        ("mx5", 335) => Some(92_000.0),
        ("gr86", 335) => Some(89_000.0),
        ("bmw_m2", 335) => Some(77_000.0),
        ("gt4", 335) => Some(70_000.0),
        ("gt3", 335) => Some(63_000.0),
        ("lmp2", 335) => Some(55_000.0),

        // --- Cadwell Park (339) ---
        ("mx5", 339) => Some(109_000.0),
        ("gr86", 339) => Some(104_000.0),
        ("bmw_m2", 339) => Some(96_000.0),
        ("gt4", 339) => Some(90_000.0),
        ("gt3", 339) => Some(86_000.0),
        ("lmp2", 339) => Some(76_000.0),

        // --- Zolder (350) ---
        ("mx5", 350) => Some(111_000.0),
        ("gr86", 350) => Some(107_000.0),
        ("bmw_m2", 350) => Some(93_000.0),
        ("gt4", 350) => Some(86_000.0),
        ("gt3", 350) => Some(79_000.0),
        ("lmp2", 350) => Some(69_000.0),

        // --- Paul Ricard (360) ---
        ("mx5", 360) => Some(135_000.0),
        ("gr86", 360) => Some(131_000.0),
        ("bmw_m2", 360) => Some(114_000.0),
        ("gt4", 360) => Some(105_000.0),
        ("gt3", 360) => Some(96_000.0),
        ("lmp2", 360) => Some(83_000.0),

        // --- Misano (363) ---
        ("mx5", 363) => Some(119_000.0),
        ("gr86", 363) => Some(115_000.0),
        ("bmw_m2", 363) => Some(98_000.0),
        ("gt4", 363) => Some(91_000.0),
        ("gt3", 363) => Some(83_000.0),
        ("lmp2", 363) => Some(73_000.0),

        // --- Fuji (373) ---
        ("mx5", 373) => Some(111_000.0),
        ("gr86", 373) => Some(108_000.0),
        ("bmw_m2", 373) => Some(92_000.0),
        ("gt4", 373) => Some(84_000.0),
        ("gt3", 373) => Some(76_000.0),
        ("lmp2", 373) => Some(66_000.0),

        // --- Vallelunga (382) ---
        ("mx5", 382) => Some(113_000.0),
        ("gr86", 382) => Some(109_000.0),
        ("bmw_m2", 382) => Some(96_000.0),
        ("gt4", 382) => Some(89_000.0),
        ("gt3", 382) => Some(81_000.0),
        ("lmp2", 382) => Some(70_000.0),

        // --- Zandvoort (389) ---
        ("mx5", 389) => Some(118_000.0),
        ("gr86", 389) => Some(114_000.0),
        ("bmw_m2", 389) => Some(99_000.0),
        ("gt4", 389) => Some(91_000.0),
        ("gt3", 389) => Some(84_000.0),
        ("lmp2", 389) => Some(73_000.0),

        // --- Bahrain (393) ---
        ("mx5", 393) => Some(137_000.0),
        ("gr86", 393) => Some(133_000.0),
        ("bmw_m2", 393) => Some(118_000.0),
        ("gt4", 393) => Some(109_000.0),
        ("gt3", 393) => Some(99_000.0),
        ("lmp2", 393) => Some(86_000.0),

        // --- Red Bull Ring (397) ---
        ("mx5", 397) => Some(105_000.0),
        ("gr86", 397) => Some(102_000.0),
        ("bmw_m2", 397) => Some(88_000.0),
        ("gt4", 397) => Some(80_000.0),
        ("gt3", 397) => Some(73_000.0),
        ("lmp2", 397) => Some(62_000.0),

        // --- Donington GP (399) ---
        ("mx5", 399) => Some(110_000.0),
        ("gr86", 399) => Some(106_000.0),
        ("bmw_m2", 399) => Some(93_000.0),
        ("gt4", 399) => Some(87_000.0),
        ("gt3", 399) => Some(79_000.0),
        ("lmp2", 399) => Some(69_000.0),

        // --- Donington National (400) ---
        ("mx5", 400) => Some(93_000.0),
        ("gr86", 400) => Some(90_000.0),
        ("bmw_m2", 400) => Some(79_000.0),
        ("gt4", 400) => Some(73_000.0),
        ("gt3", 400) => Some(67_000.0),
        ("lmp2", 400) => Some(58_000.0),

        // --- Brno (404) ---
        ("mx5", 404) => Some(137_000.0),
        ("gr86", 404) => Some(133_000.0),
        ("bmw_m2", 404) => Some(118_000.0),
        ("gt4", 404) => Some(109_000.0),
        ("gt3", 404) => Some(99_000.0),
        ("lmp2", 404) => Some(86_000.0),

        // --- Assen (409) ---
        ("mx5", 409) => Some(121_000.0),
        ("gr86", 409) => Some(117_000.0),
        ("bmw_m2", 409) => Some(102_000.0),
        ("gt4", 409) => Some(95_000.0),
        ("gt3", 409) => Some(87_000.0),
        ("lmp2", 409) => Some(75_000.0),

        // --- Mexico City / Hermanos Rodriguez (413) ---
        ("mx5", 413) => Some(115_000.0),
        ("gr86", 413) => Some(112_000.0),
        ("bmw_m2", 413) => Some(100_000.0),
        ("gt4", 413) => Some(93_000.0),
        ("gt3", 413) => Some(85_000.0),
        ("lmp2", 413) => Some(73_000.0),

        // --- Istanbul (420) ---
        ("mx5", 420) => Some(134_000.0),
        ("gr86", 420) => Some(130_000.0),
        ("bmw_m2", 420) => Some(116_000.0),
        ("gt4", 420) => Some(107_000.0),
        ("gt3", 420) => Some(97_000.0),
        ("lmp2", 420) => Some(84_000.0),

        // --- Sandown (421) ---
        ("mx5", 421) => Some(87_000.0),
        ("gr86", 421) => Some(84_000.0),
        ("bmw_m2", 421) => Some(72_000.0),
        ("gt4", 421) => Some(67_000.0),
        ("gt3", 421) => Some(61_000.0),
        ("lmp2", 421) => Some(53_000.0),

        // --- Portimão (425) ---
        ("mx5", 425) => Some(124_000.0),
        ("gr86", 425) => Some(120_000.0),
        ("bmw_m2", 425) => Some(108_000.0),
        ("gt4", 425) => Some(100_000.0),
        ("gt3", 425) => Some(91_000.0),
        ("lmp2", 425) => Some(80_000.0),

        // --- Mugello (452) ---
        ("mx5", 452) => Some(128_000.0),
        ("gr86", 452) => Some(124_000.0),
        ("bmw_m2", 452) => Some(106_000.0),
        ("gt4", 452) => Some(97_000.0),
        ("gt3", 452) => Some(88_000.0),
        ("lmp2", 452) => Some(76_000.0),

        // --- Imola (455) ---
        ("mx5", 455) => Some(128_000.0),
        ("gr86", 455) => Some(124_000.0),
        ("bmw_m2", 455) => Some(114_000.0),
        ("gt4", 455) => Some(105_000.0),
        ("gt3", 455) => Some(98_000.0),
        ("lmp2", 455) => Some(84_000.0),

        // --- Detroit (504) ---
        ("mx5", 504) => Some(107_000.0),
        ("gr86", 504) => Some(102_000.0),
        ("bmw_m2", 504) => Some(89_000.0),
        ("gt4", 504) => Some(82_000.0),
        ("gt3", 504) => Some(78_000.0),
        ("lmp2", 504) => Some(69_000.0),

        // --- Kyalami (513) ---
        ("mx5", 513) => Some(121_000.0),
        ("gr86", 513) => Some(117_000.0),
        ("bmw_m2", 513) => Some(102_000.0),
        ("gt4", 513) => Some(95_000.0),
        ("gt3", 513) => Some(87_000.0),
        ("lmp2", 513) => Some(75_000.0),

        // --- Yas Marina (516) ---
        ("mx5", 516) => Some(140_000.0),
        ("gr86", 516) => Some(136_000.0),
        ("bmw_m2", 516) => Some(121_000.0),
        ("gt4", 516) => Some(112_000.0),
        ("gt3", 516) => Some(102_000.0),
        ("lmp2", 516) => Some(88_000.0),

        // --- Nürburgring Combined 24H (528) ---
        ("mx5", 528) => Some(703_000.0),
        ("gr86", 528) => Some(678_000.0),
        ("bmw_m2", 528) => Some(589_000.0),
        ("gt4", 528) => Some(543_000.0),
        ("gt3", 528) => Some(499_000.0),
        ("lmp2", 528) => Some(434_000.0),

        // --- Silverstone National (532) ---
        ("mx5", 532) => Some(107_000.0),
        ("gr86", 532) => Some(103_000.0),
        ("bmw_m2", 532) => Some(90_000.0),
        ("gt4", 532) => Some(83_000.0),
        ("gt3", 532) => Some(76_000.0),
        ("lmp2", 532) => Some(66_000.0),

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Dificuldade de pista (mapa estático para pistas com identidade reconhecida)
// ---------------------------------------------------------------------------

fn track_difficulty_for(track_id: u32) -> f64 {
    match track_id {
        196 => 1.6, // Nordschleife
        119 => 1.5, // Mount Panorama / Bathurst
        528 => 1.5, // Nürburgring Combined
        188 => 1.4, // Spa
        339 => 1.3, // Cadwell Park
        164 => 1.3, // Suzuka
        194 => 1.2, // Hungaroring (técnico/lento)
        287 => 1.2, // Le Mans (longo e exigente)
        554 => 0.9, // Charlotte Roval (mais fácil de ultrapassar)
        45 => 0.9,  // Daytona Road (roval)
        _ => 1.0,   // baseline
    }
}

fn overtaking_difficulty_for(character: TrackCharacter) -> f64 {
    match character {
        TrackCharacter::Roval => 0.80,
        TrackCharacter::Flowing => 0.90,
        TrackCharacter::Technical => 1.00,
        TrackCharacter::Tight => 1.15,
    }
}

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

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_for(cat: &str) -> SimulationProfile {
        resolve_simulation_profile(cat, 47, 22.0, WeatherCondition::Dry, 30, 12)
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
        // Laguna Seca (47) para GT4 deve retornar 77_000ms da tabela
        let profile = resolve_simulation_profile("gt4", 47, 22.0, WeatherCondition::Dry, 30, 12);
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
        let profile = resolve_simulation_profile("gt3", 196, 22.0, WeatherCondition::Dry, 60, 5);
        assert!(
            profile.track_difficulty_multiplier >= 1.5,
            "Nordschleife should have difficulty >= 1.5, got {}",
            profile.track_difficulty_multiplier
        );
    }

    #[test]
    fn test_roval_has_lower_overtaking_difficulty() {
        let roval = resolve_simulation_profile("gt4", 554, 22.0, WeatherCondition::Dry, 30, 12); // Charlotte Roval
        let road = resolve_simulation_profile("gt4", 199, 22.0, WeatherCondition::Dry, 30, 12); // Interlagos (Technical)
        assert!(
            roval.overtaking_difficulty_multiplier < road.overtaking_difficulty_multiplier,
            "roval={} should < road={}",
            roval.overtaking_difficulty_multiplier,
            road.overtaking_difficulty_multiplier
        );
    }

    #[test]
    fn test_sebring_has_higher_tire_stress_than_tsukuba() {
        let sebring = resolve_simulation_profile("gt4", 238, 22.0, WeatherCondition::Dry, 30, 12);
        let tsukuba = resolve_simulation_profile("gt4", 325, 22.0, WeatherCondition::Dry, 30, 12);
        assert!(
            sebring.tire_degradation_rate > tsukuba.tire_degradation_rate,
            "Sebring tire={} should > Tsukuba={}",
            sebring.tire_degradation_rate,
            tsukuba.tire_degradation_rate
        );
    }

    #[test]
    fn test_le_mans_has_higher_physical_stress_than_lime_rock() {
        let le_mans = resolve_simulation_profile("gt4", 287, 22.0, WeatherCondition::Dry, 30, 12);
        let lime_rock = resolve_simulation_profile("gt4", 14, 22.0, WeatherCondition::Dry, 30, 12);
        assert!(
            le_mans.physical_degradation_rate > lime_rock.physical_degradation_rate,
            "Le Mans phys={} should > Lime Rock={}",
            le_mans.physical_degradation_rate,
            lime_rock.physical_degradation_rate
        );
    }

    #[test]
    fn test_tight_track_has_higher_overtaking_diff_than_flowing() {
        let hungaroring =
            resolve_simulation_profile("gt4", 194, 22.0, WeatherCondition::Dry, 30, 12); // Tight
        let spa = resolve_simulation_profile("gt4", 188, 22.0, WeatherCondition::Dry, 30, 12); // Flowing
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
}
