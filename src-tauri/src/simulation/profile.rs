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
    // Chaveado pelos ids reais do iRacing (ver constants/tracks.rs).
    match (car_family, track_id) {
        // --- Charlotte Roval (554) ---
        ("mx5", 554) => Some(101_000.0),
        ("gr86", 554) => Some(98_000.0),
        ("bmw_m2", 554) => Some(85_000.0),
        ("gt4", 554) => Some(79_000.0),
        ("gt3", 554) => Some(72_000.0),
        ("lmp2", 554) => Some(63_000.0),

        // --- Lime Rock GP (353) ---
        ("mx5", 353) => Some(60_000.0),
        ("gr86", 353) => Some(58_000.0),
        ("bmw_m2", 353) => Some(49_000.0),
        ("gt4", 353) => Some(45_000.0),
        ("gt3", 353) => Some(40_000.0),
        ("lmp2", 353) => Some(35_000.0),

        // --- Laguna Seca (586) ---
        ("mx5", 586) => Some(100_000.0),
        ("gr86", 586) => Some(97_000.0),
        ("bmw_m2", 586) => Some(84_000.0),
        ("gt4", 586) => Some(77_000.0),
        ("gt3", 586) => Some(71_000.0),
        ("lmp2", 586) => Some(62_000.0),

        // --- Okayama (166) ---
        ("mx5", 166) => Some(107_000.0),
        ("gr86", 166) => Some(102_000.0),
        ("bmw_m2", 166) => Some(86_000.0),
        ("gt4", 166) => Some(79_000.0),
        ("gt3", 166) => Some(72_000.0),
        ("lmp2", 166) => Some(63_000.0),

        // --- Oulton Park Fosters (181) ---
        ("mx5", 181) => Some(115_000.0),
        ("gr86", 181) => Some(111_000.0),
        ("bmw_m2", 181) => Some(100_000.0),
        ("gt4", 181) => Some(93_000.0),
        ("gt3", 181) => Some(85_000.0),
        ("lmp2", 181) => Some(74_000.0),

        // --- Oulton Park Intl (180) ---
        ("mx5", 180) => Some(115_000.0),
        ("gr86", 180) => Some(111_000.0),
        ("bmw_m2", 180) => Some(100_000.0),
        ("gt4", 180) => Some(93_000.0),
        ("gt3", 180) => Some(85_000.0),
        ("lmp2", 180) => Some(74_000.0),

        // --- Oulton Park Island (182) ---
        ("mx5", 182) => Some(110_000.0),
        ("gr86", 182) => Some(106_000.0),
        ("bmw_m2", 182) => Some(96_000.0),
        ("gt4", 182) => Some(89_000.0),
        ("gt3", 182) => Some(82_000.0),
        ("lmp2", 182) => Some(71_000.0),

        // --- Summit Point (9) ---
        ("mx5", 9) => Some(80_000.0),
        ("gr86", 9) => Some(78_000.0),
        ("bmw_m2", 9) => Some(65_000.0),
        ("gt4", 9) => Some(60_000.0),
        ("gt3", 9) => Some(54_000.0),
        ("lmp2", 9) => Some(46_000.0),

        // --- Tsukuba (324) ---
        ("mx5", 324) => Some(69_000.0),
        ("gr86", 324) => Some(65_000.0),
        ("bmw_m2", 324) => Some(57_000.0),
        ("gt4", 324) => Some(53_000.0),
        ("gt3", 324) => Some(46_000.0),
        ("lmp2", 324) => Some(44_000.0),

        // --- Brands Hatch GP (145) ---
        ("mx5", 145) => Some(108_000.0),
        ("gr86", 145) => Some(104_000.0),
        ("bmw_m2", 145) => Some(91_000.0),
        ("gt4", 145) => Some(84_000.0),
        ("gt3", 145) => Some(77_000.0),
        ("lmp2", 145) => Some(67_000.0),

        // --- Daytona Road (192) ---
        ("mx5", 192) => Some(139_000.0),
        ("gr86", 192) => Some(135_000.0),
        ("bmw_m2", 192) => Some(116_000.0),
        ("gt4", 192) => Some(106_000.0),
        ("gt3", 192) => Some(96_000.0),
        ("lmp2", 192) => Some(83_000.0),

        // --- Mid-Ohio (153) ---
        ("mx5", 153) => Some(102_000.0),
        ("gr86", 153) => Some(98_000.0),
        ("bmw_m2", 153) => Some(84_000.0),
        ("gt4", 153) => Some(78_000.0),
        ("gt3", 153) => Some(71_000.0),
        ("lmp2", 153) => Some(62_000.0),

        // --- Road America (18) ---
        ("mx5", 18) => Some(156_000.0),
        ("gr86", 18) => Some(151_000.0),
        ("bmw_m2", 18) => Some(132_000.0),
        ("gt4", 18) => Some(121_000.0),
        ("gt3", 18) => Some(110_000.0),
        ("lmp2", 18) => Some(94_000.0),

        // --- Sonoma (566) ---
        ("mx5", 566) => Some(109_000.0),
        ("gr86", 566) => Some(105_000.0),
        ("bmw_m2", 566) => Some(90_000.0),
        ("gt4", 566) => Some(83_000.0),
        ("gt3", 566) => Some(76_000.0),
        ("lmp2", 566) => Some(66_000.0),

        // --- VIR Full (465) ---
        ("mx5", 465) => Some(140_000.0),
        ("gr86", 465) => Some(135_000.0),
        ("bmw_m2", 465) => Some(113_000.0),
        ("gt4", 465) => Some(104_000.0),
        ("gt3", 465) => Some(97_000.0),
        ("lmp2", 465) => Some(84_000.0),

        // --- Watkins Glen Cup (433) ---
        ("mx5", 433) => Some(105_000.0),
        ("gr86", 433) => Some(101_000.0),
        ("bmw_m2", 433) => Some(88_000.0),
        ("gt4", 433) => Some(81_000.0),
        ("gt3", 433) => Some(74_000.0),
        ("lmp2", 433) => Some(64_000.0),

        // --- Monza (239) ---
        ("mx5", 239) => Some(134_000.0),
        ("gr86", 239) => Some(130_000.0),
        ("bmw_m2", 239) => Some(117_000.0),
        ("gt4", 239) => Some(107_000.0),
        ("gt3", 239) => Some(97_000.0),
        ("lmp2", 239) => Some(84_000.0),

        // --- Silverstone GP (341) ---
        ("mx5", 341) => Some(141_000.0),
        ("gr86", 341) => Some(137_000.0),
        ("bmw_m2", 341) => Some(119_000.0),
        ("gt4", 341) => Some(110_000.0),
        ("gt3", 341) => Some(99_000.0),
        ("lmp2", 341) => Some(86_000.0),

        // --- Bathurst / Mount Panorama (219) ---
        ("mx5", 219) => Some(172_000.0),
        ("gr86", 219) => Some(167_000.0),
        ("bmw_m2", 219) => Some(144_000.0),
        ("gt4", 219) => Some(134_000.0),
        ("gt3", 219) => Some(123_000.0),
        ("lmp2", 219) => Some(107_000.0),

        // --- Mosport / CTMP (144) ---
        ("mx5", 144) => Some(106_000.0),
        ("gr86", 144) => Some(102_000.0),
        ("bmw_m2", 144) => Some(92_000.0),
        ("gt4", 144) => Some(85_000.0),
        ("gt3", 144) => Some(78_000.0),
        ("lmp2", 144) => Some(68_000.0),

        // --- Suzuka (168) ---
        ("mx5", 168) => Some(152_000.0),
        ("gr86", 168) => Some(147_000.0),
        ("bmw_m2", 168) => Some(135_000.0),
        ("gt4", 168) => Some(126_000.0),
        ("gt3", 168) => Some(115_000.0),
        ("lmp2", 168) => Some(100_000.0),

        // --- Phillip Island (152) ---
        ("mx5", 152) => Some(110_000.0),
        ("gr86", 152) => Some(107_000.0),
        ("bmw_m2", 152) => Some(90_000.0),
        ("gt4", 152) => Some(83_000.0),
        ("gt3", 152) => Some(75_000.0),
        ("lmp2", 152) => Some(64_000.0),

        // --- Indy Road (448, ~1:33 GT3) ---
        ("mx5", 448) => Some(123_000.0),
        ("gr86", 448) => Some(119_000.0),
        ("bmw_m2", 448) => Some(109_000.0),
        ("gt4", 448) => Some(101_000.0),
        ("gt3", 448) => Some(93_000.0),
        ("lmp2", 448) => Some(81_000.0),

        // --- Spa (523) ---
        ("mx5", 523) => Some(168_000.0),
        ("gr86", 523) => Some(163_000.0),
        ("bmw_m2", 523) => Some(142_000.0),
        ("gt4", 523) => Some(130_000.0),
        ("gt3", 523) => Some(117_000.0),
        ("lmp2", 523) => Some(101_000.0),

        // --- Hockenheim GP (390) ---
        ("mx5", 390) => Some(113_000.0),
        ("gr86", 390) => Some(110_000.0),
        ("bmw_m2", 390) => Some(92_000.0),
        ("gt4", 390) => Some(85_000.0),
        ("gt3", 390) => Some(77_000.0),
        ("lmp2", 390) => Some(66_000.0),

        // --- Hungaroring (413) ---
        ("mx5", 413) => Some(134_000.0),
        ("gr86", 413) => Some(128_000.0),
        ("bmw_m2", 413) => Some(115_000.0),
        ("gt4", 413) => Some(107_000.0),
        ("gt3", 413) => Some(101_000.0),
        ("lmp2", 413) => Some(91_000.0),

        // --- Nordschleife (249) ---
        ("mx5", 249) => Some(586_000.0),
        ("gr86", 249) => Some(566_000.0),
        ("bmw_m2", 249) => Some(484_000.0),
        ("gt4", 249) => Some(446_000.0),
        ("gt3", 249) => Some(410_000.0),
        ("lmp2", 249) => Some(357_000.0),

        // --- Interlagos (212) ---
        ("mx5", 212) => Some(119_000.0),
        ("gr86", 212) => Some(115_000.0),
        ("bmw_m2", 212) => Some(100_000.0),
        ("gt4", 212) => Some(93_000.0),
        ("gt3", 212) => Some(85_000.0),
        ("lmp2", 212) => Some(73_000.0),

        // --- COTA (229) ---
        ("mx5", 229) => Some(144_000.0),
        ("gr86", 229) => Some(139_000.0),
        ("bmw_m2", 229) => Some(128_000.0),
        ("gt4", 229) => Some(119_000.0),
        ("gt3", 229) => Some(109_000.0),
        ("lmp2", 229) => Some(95_000.0),

        // --- Sebring (95) ---
        ("mx5", 95) => Some(164_000.0),
        ("gr86", 95) => Some(158_000.0),
        ("bmw_m2", 95) => Some(140_000.0),
        ("gt4", 95) => Some(130_000.0),
        ("gt3", 95) => Some(119_000.0),
        ("lmp2", 95) => Some(104_000.0),

        // --- Magny-Cours (463) ---
        ("mx5", 463) => Some(118_000.0),
        ("gr86", 463) => Some(114_000.0),
        ("bmw_m2", 463) => Some(102_000.0),
        ("gt4", 463) => Some(95_000.0),
        ("gt3", 463) => Some(87_000.0),
        ("lmp2", 463) => Some(76_000.0),

        // --- Road Atlanta (127) ---
        ("mx5", 127) => Some(109_000.0),
        ("gr86", 127) => Some(106_000.0),
        ("bmw_m2", 127) => Some(95_000.0),
        ("gt4", 127) => Some(88_000.0),
        ("gt3", 127) => Some(81_000.0),
        ("lmp2", 127) => Some(70_000.0),

        // --- Barcelona GP (345) ---
        ("mx5", 345) => Some(122_000.0),
        ("gr86", 345) => Some(118_000.0),
        ("bmw_m2", 345) => Some(108_000.0),
        ("gt4", 345) => Some(100_000.0),
        ("gt3", 345) => Some(93_000.0),
        ("lmp2", 345) => Some(80_000.0),

        // --- Le Mans (268) ---
        ("mx5", 268) => Some(316_000.0),
        ("gr86", 268) => Some(307_000.0),
        ("bmw_m2", 268) => Some(276_000.0),
        ("gt4", 268) => Some(253_000.0),
        ("gt3", 268) => Some(228_000.0),
        ("lmp2", 268) => Some(200_000.0),

        // --- Snetterton 300 (297) ---
        ("mx5", 297) => Some(119_000.0),
        ("gr86", 297) => Some(116_000.0),
        ("bmw_m2", 297) => Some(104_000.0),
        ("gt4", 297) => Some(95_000.0),
        ("gt3", 297) => Some(87_000.0),
        ("lmp2", 297) => Some(74_000.0),

        // --- Long Beach (179) ---
        ("mx5", 179) => Some(102_000.0),
        ("gr86", 179) => Some(97_000.0),
        ("bmw_m2", 179) => Some(88_000.0),
        ("gt4", 179) => Some(82_000.0),
        ("gt3", 179) => Some(73_000.0),
        ("lmp2", 179) => Some(65_000.0),

        // --- Thruxton (532) ---
        ("mx5", 532) => Some(92_000.0),
        ("gr86", 532) => Some(89_000.0),
        ("bmw_m2", 532) => Some(77_000.0),
        ("gt4", 532) => Some(70_000.0),
        ("gt3", 532) => Some(63_000.0),
        ("lmp2", 532) => Some(55_000.0),

        // --- Cadwell Park (527) ---
        ("mx5", 527) => Some(109_000.0),
        ("gr86", 527) => Some(104_000.0),
        ("bmw_m2", 527) => Some(96_000.0),
        ("gt4", 527) => Some(90_000.0),
        ("gt3", 527) => Some(86_000.0),
        ("lmp2", 527) => Some(76_000.0),

        // --- Zolder (199) ---
        ("mx5", 199) => Some(111_000.0),
        ("gr86", 199) => Some(107_000.0),
        ("bmw_m2", 199) => Some(93_000.0),
        ("gt4", 199) => Some(86_000.0),
        ("gt3", 199) => Some(79_000.0),
        ("lmp2", 199) => Some(69_000.0),

        // --- Misano (501) ---
        ("mx5", 501) => Some(119_000.0),
        ("gr86", 501) => Some(115_000.0),
        ("bmw_m2", 501) => Some(98_000.0),
        ("gt4", 501) => Some(91_000.0),
        ("gt3", 501) => Some(83_000.0),
        ("lmp2", 501) => Some(73_000.0),

        // --- Fuji (444) ---
        ("mx5", 444) => Some(111_000.0),
        ("gr86", 444) => Some(108_000.0),
        ("bmw_m2", 444) => Some(92_000.0),
        ("gt4", 444) => Some(84_000.0),
        ("gt3", 444) => Some(76_000.0),
        ("lmp2", 444) => Some(66_000.0),

        // --- Zandvoort (485) ---
        ("mx5", 485) => Some(118_000.0),
        ("gr86", 485) => Some(114_000.0),
        ("bmw_m2", 485) => Some(99_000.0),
        ("gt4", 485) => Some(91_000.0),
        ("gt3", 485) => Some(84_000.0),
        ("lmp2", 485) => Some(73_000.0),

        // --- Red Bull Ring (403) ---
        ("mx5", 403) => Some(105_000.0),
        ("gr86", 403) => Some(102_000.0),
        ("bmw_m2", 403) => Some(88_000.0),
        ("gt4", 403) => Some(80_000.0),
        ("gt3", 403) => Some(73_000.0),
        ("lmp2", 403) => Some(62_000.0),

        // --- Donington GP (233) ---
        ("mx5", 233) => Some(110_000.0),
        ("gr86", 233) => Some(106_000.0),
        ("bmw_m2", 233) => Some(93_000.0),
        ("gt4", 233) => Some(87_000.0),
        ("gt3", 233) => Some(79_000.0),
        ("lmp2", 233) => Some(69_000.0),

        // --- Mexico City / Hermanos Rodriguez (572) ---
        ("mx5", 572) => Some(115_000.0),
        ("gr86", 572) => Some(112_000.0),
        ("bmw_m2", 572) => Some(100_000.0),
        ("gt4", 572) => Some(93_000.0),
        ("gt3", 572) => Some(85_000.0),
        ("lmp2", 572) => Some(73_000.0),

        // --- Sandown (443) ---
        ("mx5", 443) => Some(87_000.0),
        ("gr86", 443) => Some(84_000.0),
        ("bmw_m2", 443) => Some(72_000.0),
        ("gt4", 443) => Some(67_000.0),
        ("gt3", 443) => Some(61_000.0),
        ("lmp2", 443) => Some(53_000.0),

        // --- Portimão / Algarve GP (509) ---
        ("mx5", 509) => Some(124_000.0),
        ("gr86", 509) => Some(120_000.0),
        ("bmw_m2", 509) => Some(108_000.0),
        ("gt4", 509) => Some(100_000.0),
        ("gt3", 509) => Some(91_000.0),
        ("lmp2", 509) => Some(80_000.0),

        // --- Mugello (498) ---
        ("mx5", 498) => Some(128_000.0),
        ("gr86", 498) => Some(124_000.0),
        ("bmw_m2", 498) => Some(106_000.0),
        ("gt4", 498) => Some(97_000.0),
        ("gt3", 498) => Some(88_000.0),
        ("lmp2", 498) => Some(76_000.0),

        // --- Imola (266) ---
        ("mx5", 266) => Some(128_000.0),
        ("gr86", 266) => Some(124_000.0),
        ("bmw_m2", 266) => Some(114_000.0),
        ("gt4", 266) => Some(105_000.0),
        ("gt3", 266) => Some(98_000.0),
        ("lmp2", 266) => Some(84_000.0),

        // --- Detroit (319) ---
        ("mx5", 319) => Some(107_000.0),
        ("gr86", 319) => Some(102_000.0),
        ("bmw_m2", 319) => Some(89_000.0),
        ("gt4", 319) => Some(82_000.0),
        ("gt3", 319) => Some(78_000.0),
        ("lmp2", 319) => Some(69_000.0),

        // --- Nürburgring Combined 24H (252) ---
        ("mx5", 252) => Some(703_000.0),
        ("gr86", 252) => Some(678_000.0),
        ("bmw_m2", 252) => Some(589_000.0),
        ("gt4", 252) => Some(543_000.0),
        ("gt3", 252) => Some(499_000.0),
        ("lmp2", 252) => Some(434_000.0),

        // --- Silverstone National (343) ---
        ("mx5", 343) => Some(107_000.0),
        ("gr86", 343) => Some(103_000.0),
        ("bmw_m2", 343) => Some(90_000.0),
        ("gt4", 343) => Some(83_000.0),
        ("gt3", 343) => Some(76_000.0),
        ("lmp2", 343) => Some(66_000.0),

        // ── Layouts secundários (escalados por comprimento do layout irmão) ──
        // --- Algarve GP Chicanes (510, ← 509) ---
        ("mx5", 510) => Some(121_000.0),
        ("gr86", 510) => Some(117_000.0),
        ("bmw_m2", 510) => Some(106_000.0),
        ("gt4", 510) => Some(98_000.0),
        ("gt3", 510) => Some(89_000.0),
        ("lmp2", 510) => Some(78_000.0),

        // --- Hermanos National (574, ← 572) ---
        ("mx5", 574) => Some(96_000.0),
        ("gr86", 574) => Some(94_000.0),
        ("bmw_m2", 574) => Some(84_000.0),
        ("gt4", 574) => Some(78_000.0),
        ("gt3", 574) => Some(71_000.0),
        ("lmp2", 574) => Some(61_000.0),

        // --- Mugello Short (499, ← 498) ---
        ("mx5", 499) => Some(106_000.0),
        ("gr86", 499) => Some(103_000.0),
        ("bmw_m2", 499) => Some(88_000.0),
        ("gt4", 499) => Some(80_000.0),
        ("gt3", 499) => Some(73_000.0),
        ("lmp2", 499) => Some(63_000.0),

        // --- Monza Combined (247, oval+road ~10 km) ---
        ("mx5", 247) => Some(231_000.0),
        ("gr86", 247) => Some(224_000.0),
        ("bmw_m2", 247) => Some(202_000.0),
        ("gt4", 247) => Some(184_000.0),
        ("gt3", 247) => Some(167_000.0),
        ("lmp2", 247) => Some(145_000.0),

        // --- Monza s/ 1ª chicane (244) ---
        ("mx5", 244) => Some(133_000.0),
        ("gr86", 244) => Some(129_000.0),
        ("bmw_m2", 244) => Some(116_000.0),
        ("gt4", 244) => Some(106_000.0),
        ("gt3", 244) => Some(96_000.0),
        ("lmp2", 244) => Some(83_000.0),

        // --- Monza s/ chicanes (242) ---
        ("mx5", 242) => Some(130_000.0),
        ("gr86", 242) => Some(126_000.0),
        ("bmw_m2", 242) => Some(113_000.0),
        ("gt4", 242) => Some(104_000.0),
        ("gt3", 242) => Some(94_000.0),
        ("lmp2", 242) => Some(81_000.0),

        // --- Spa Endurance (525, = 523) ---
        ("mx5", 525) => Some(168_000.0),
        ("gr86", 525) => Some(163_000.0),
        ("bmw_m2", 525) => Some(142_000.0),
        ("gt4", 525) => Some(130_000.0),
        ("gt3", 525) => Some(117_000.0),
        ("lmp2", 525) => Some(101_000.0),

        // --- Zandvoort Nationaal (487, ← 485) ---
        ("mx5", 487) => Some(82_000.0),
        ("gr86", 487) => Some(80_000.0),
        ("bmw_m2", 487) => Some(69_000.0),
        ("gt4", 487) => Some(64_000.0),
        ("gt3", 487) => Some(59_000.0),
        ("lmp2", 487) => Some(51_000.0),

        // --- Zandvoort w/chicane (486, ← 485) ---
        ("mx5", 486) => Some(120_000.0),
        ("gr86", 486) => Some(116_000.0),
        ("bmw_m2", 486) => Some(101_000.0),
        ("gt4", 486) => Some(93_000.0),
        ("gt3", 486) => Some(86_000.0),
        ("lmp2", 486) => Some(74_000.0),

        // --- Navarra Speed (515, root free) ---
        ("mx5", 515) => Some(108_000.0),
        ("gr86", 515) => Some(105_000.0),
        ("bmw_m2", 515) => Some(96_000.0),
        ("gt4", 515) => Some(89_000.0),
        ("gt3", 515) => Some(82_000.0),
        ("lmp2", 515) => Some(71_000.0),

        // --- Navarra Medium (516) ---
        ("mx5", 516) => Some(94_000.0),
        ("gr86", 516) => Some(91_000.0),
        ("bmw_m2", 516) => Some(83_000.0),
        ("gt4", 516) => Some(77_000.0),
        ("gt3", 516) => Some(71_000.0),
        ("lmp2", 516) => Some(62_000.0),

        // --- Navarra Short (517) ---
        ("mx5", 517) => Some(81_000.0),
        ("gr86", 517) => Some(78_000.0),
        ("bmw_m2", 517) => Some(71_000.0),
        ("gt4", 517) => Some(67_000.0),
        ("gt3", 517) => Some(61_000.0),
        ("lmp2", 517) => Some(53_000.0),

        // --- Fuji No Chicane (445, ← 444) ---
        ("mx5", 445) => Some(107_000.0),
        ("gr86", 445) => Some(104_000.0),
        ("bmw_m2", 445) => Some(88_000.0),
        ("gt4", 445) => Some(81_000.0),
        ("gt3", 445) => Some(73_000.0),
        ("lmp2", 445) => Some(63_000.0),

        // --- Lime Rock Classic (352, = 353) ---
        ("mx5", 352) => Some(60_000.0),
        ("gr86", 352) => Some(58_000.0),
        ("bmw_m2", 352) => Some(49_000.0),
        ("gt4", 352) => Some(45_000.0),
        ("gt3", 352) => Some(40_000.0),
        ("lmp2", 352) => Some(35_000.0),

        // --- Lime Rock Chicanes (354, ← 353) ---
        ("mx5", 354) => Some(63_000.0),
        ("gr86", 354) => Some(60_000.0),
        ("bmw_m2", 354) => Some(51_000.0),
        ("gt4", 354) => Some(47_000.0),
        ("gt3", 354) => Some(42_000.0),
        ("lmp2", 354) => Some(36_000.0),

        // --- Mid-Ohio Chicane (154, ← 153) ---
        ("mx5", 154) => Some(97_000.0),
        ("gr86", 154) => Some(93_000.0),
        ("bmw_m2", 154) => Some(80_000.0),
        ("gt4", 154) => Some(74_000.0),
        ("gt3", 154) => Some(67_000.0),
        ("lmp2", 154) => Some(59_000.0),

        // --- Oschersleben GP (449, root free) ---
        ("mx5", 449) => Some(107_000.0),
        ("gr86", 449) => Some(104_000.0),
        ("bmw_m2", 449) => Some(95_000.0),
        ("gt4", 449) => Some(88_000.0),
        ("gt3", 449) => Some(81_000.0),
        ("lmp2", 449) => Some(70_000.0),

        // --- Oschersleben Alt (454, = 449) ---
        ("mx5", 454) => Some(107_000.0),
        ("gr86", 454) => Some(104_000.0),
        ("bmw_m2", 454) => Some(95_000.0),
        ("gt4", 454) => Some(88_000.0),
        ("gt3", 454) => Some(81_000.0),
        ("lmp2", 454) => Some(70_000.0),

        // --- Oschersleben B (455, ← 449) ---
        ("mx5", 455) => Some(70_000.0),
        ("gr86", 455) => Some(68_000.0),
        ("bmw_m2", 455) => Some(62_000.0),
        ("gt4", 455) => Some(58_000.0),
        ("gt3", 455) => Some(53_000.0),
        ("lmp2", 455) => Some(46_000.0),

        // --- Oran Park GP (202, root free) ---
        ("mx5", 202) => Some(73_000.0),
        ("gr86", 202) => Some(70_000.0),
        ("bmw_m2", 202) => Some(64_000.0),
        ("gt4", 202) => Some(60_000.0),
        ("gt3", 202) => Some(55_000.0),
        ("lmp2", 202) => Some(48_000.0),

        // --- Oran Park South (208, ← 202) ---
        ("mx5", 208) => Some(55_000.0),
        ("gr86", 208) => Some(54_000.0),
        ("bmw_m2", 208) => Some(49_000.0),
        ("gt4", 208) => Some(46_000.0),
        ("gt3", 208) => Some(42_000.0),
        ("lmp2", 208) => Some(37_000.0),

        // --- Oran Park North (207, ← 202) ---
        ("mx5", 207) => Some(45_000.0),
        ("gr86", 207) => Some(44_000.0),
        ("bmw_m2", 207) => Some(40_000.0),
        ("gt4", 207) => Some(37_000.0),
        ("gt3", 207) => Some(34_000.0),
        ("lmp2", 207) => Some(30_000.0),

        // --- Oulton w/out Hislop (183, ← 180) ---
        ("mx5", 183) => Some(112_000.0),
        ("gr86", 183) => Some(108_000.0),
        ("bmw_m2", 183) => Some(98_000.0),
        ("gt4", 183) => Some(91_000.0),
        ("gt3", 183) => Some(83_000.0),
        ("lmp2", 183) => Some(72_000.0),

        // --- Oulton w/out Brittens (184, ← 180) ---
        ("mx5", 184) => Some(112_000.0),
        ("gr86", 184) => Some(108_000.0),
        ("bmw_m2", 184) => Some(98_000.0),
        ("gt4", 184) => Some(91_000.0),
        ("gt3", 184) => Some(83_000.0),
        ("lmp2", 184) => Some(72_000.0),

        // --- Oulton w/no Chicanes (185, ← 180) ---
        ("mx5", 185) => Some(112_000.0),
        ("gr86", 185) => Some(108_000.0),
        ("bmw_m2", 185) => Some(98_000.0),
        ("gt4", 185) => Some(91_000.0),
        ("gt3", 185) => Some(83_000.0),
        ("lmp2", 185) => Some(72_000.0),

        // --- Silverstone International (342, ← 341) ---
        ("mx5", 342) => Some(86_000.0),
        ("gr86", 342) => Some(84_000.0),
        ("bmw_m2", 342) => Some(73_000.0),
        ("gt4", 342) => Some(67_000.0),
        ("gt3", 342) => Some(60_000.0),
        ("lmp2", 342) => Some(52_000.0),

        // --- Snetterton 200 (298, ← 297) ---
        ("mx5", 298) => Some(79_000.0),
        ("gr86", 298) => Some(77_000.0),
        ("bmw_m2", 298) => Some(69_000.0),
        ("gt4", 298) => Some(63_000.0),
        ("gt3", 298) => Some(58_000.0),
        ("lmp2", 298) => Some(49_000.0),

        // --- Sonoma Cup Short (567, ← 566) ---
        ("mx5", 567) => Some(87_000.0),
        ("gr86", 567) => Some(84_000.0),
        ("bmw_m2", 567) => Some(72_000.0),
        ("gt4", 567) => Some(66_000.0),
        ("gt3", 567) => Some(61_000.0),
        ("lmp2", 567) => Some(53_000.0),

        // --- VIR North (467, ← 465) ---
        ("mx5", 467) => Some(95_000.0),
        ("gr86", 467) => Some(92_000.0),
        ("bmw_m2", 467) => Some(77_000.0),
        ("gt4", 467) => Some(71_000.0),
        ("gt3", 467) => Some(66_000.0),
        ("lmp2", 467) => Some(57_000.0),

        // --- VIR South (468, ← 465) ---
        ("mx5", 468) => Some(63_000.0),
        ("gr86", 468) => Some(61_000.0),
        ("bmw_m2", 468) => Some(51_000.0),
        ("gt4", 468) => Some(47_000.0),
        ("gt3", 468) => Some(44_000.0),
        ("lmp2", 468) => Some(38_000.0),

        // --- VIR Grand (466, ← 465) ---
        ("mx5", 466) => Some(180_000.0),
        ("gr86", 466) => Some(173_000.0),
        ("bmw_m2", 466) => Some(145_000.0),
        ("gt4", 466) => Some(133_000.0),
        ("gt3", 466) => Some(124_000.0),
        ("lmp2", 466) => Some(108_000.0),

        // --- Winton Club (440, root free) ---
        ("mx5", 440) => Some(58_000.0),
        ("gr86", 440) => Some(56_000.0),
        ("bmw_m2", 440) => Some(51_000.0),
        ("gt4", 440) => Some(48_000.0),
        ("gt3", 440) => Some(44_000.0),
        ("lmp2", 440) => Some(38_000.0),

        // --- Winton National (439, ← 440) ---
        ("mx5", 439) => Some(87_000.0),
        ("gr86", 439) => Some(84_000.0),
        ("bmw_m2", 439) => Some(77_000.0),
        ("gt4", 439) => Some(72_000.0),
        ("gt3", 439) => Some(66_000.0),
        ("lmp2", 439) => Some(57_000.0),

        // --- Nürburgring Combined Short (263) ---
        ("mx5", 263) => Some(135_000.0),
        ("gr86", 263) => Some(131_000.0),
        ("bmw_m2", 263) => Some(119_000.0),
        ("gt4", 263) => Some(111_000.0),
        ("gt3", 263) => Some(102_000.0),
        ("lmp2", 263) => Some(89_000.0),

        // ── Venues novos: lap-times pesquisados por circuito (âncora GT3) ──
        // --- Adelaide (580, ~1:16 GT3) ---
        ("mx5", 580) => Some(102_000.0),
        ("gr86", 580) => Some(99_000.0),
        ("bmw_m2", 580) => Some(90_000.0),
        ("gt4", 580) => Some(84_000.0),
        ("gt3", 580) => Some(77_000.0),
        ("lmp2", 580) => Some(67_000.0),

        // --- Barber (585, ~1:24 GT3) ---
        ("mx5", 585) => Some(111_000.0),
        ("gr86", 585) => Some(108_000.0),
        ("bmw_m2", 585) => Some(98_000.0),
        ("gt4", 585) => Some(92_000.0),
        ("gt3", 585) => Some(84_000.0),
        ("lmp2", 585) => Some(73_000.0),

        // --- Chicago Street (405, ~1:32 GT3 est.) ---
        ("mx5", 405) => Some(121_000.0),
        ("gr86", 405) => Some(118_000.0),
        ("bmw_m2", 405) => Some(108_000.0),
        ("gt4", 405) => Some(100_000.0),
        ("gt3", 405) => Some(92_000.0),
        ("lmp2", 405) => Some(80_000.0),

        // --- Gilles Villeneuve (218, ~1:32 GT3) ---
        ("mx5", 218) => Some(121_000.0),
        ("gr86", 218) => Some(118_000.0),
        ("bmw_m2", 218) => Some(108_000.0),
        ("gt4", 218) => Some(100_000.0),
        ("gt3", 218) => Some(92_000.0),
        ("lmp2", 218) => Some(80_000.0),

        // --- Jerez (473, ~1:42 GT3) ---
        ("mx5", 473) => Some(135_000.0),
        ("gr86", 473) => Some(131_000.0),
        ("bmw_m2", 473) => Some(119_000.0),
        ("gt4", 473) => Some(111_000.0),
        ("gt3", 473) => Some(102_000.0),
        ("lmp2", 473) => Some(89_000.0),

        // --- Knockhill (423, ~47s GT3) ---
        ("mx5", 423) => Some(62_000.0),
        ("gr86", 423) => Some(60_000.0),
        ("bmw_m2", 423) => Some(55_000.0),
        ("gt4", 423) => Some(51_000.0),
        ("gt3", 423) => Some(47_000.0),
        ("lmp2", 423) => Some(41_000.0),

        // --- Miami (539, ~1:54 GT3) ---
        ("mx5", 539) => Some(150_000.0),
        ("gr86", 539) => Some(146_000.0),
        ("bmw_m2", 539) => Some(133_000.0),
        ("gt4", 539) => Some(124_000.0),
        ("gt3", 539) => Some(114_000.0),
        ("lmp2", 539) => Some(99_000.0),

        // --- Motegi (195, ~1:49 GT3) ---
        ("mx5", 195) => Some(145_000.0),
        ("gr86", 195) => Some(141_000.0),
        ("bmw_m2", 195) => Some(129_000.0),
        ("gt4", 195) => Some(120_000.0),
        ("gt3", 195) => Some(110_000.0),
        ("lmp2", 195) => Some(96_000.0),

        // --- MotorLand Aragón GP (475, ~1:50 GT3) ---
        ("mx5", 475) => Some(145_000.0),
        ("gr86", 475) => Some(141_000.0),
        ("bmw_m2", 475) => Some(129_000.0),
        ("gt4", 475) => Some(120_000.0),
        ("gt3", 475) => Some(110_000.0),
        ("lmp2", 475) => Some(96_000.0),

        // --- MotorLand Aragón National (476, ← GP) ---
        ("mx5", 476) => Some(131_000.0),
        ("gr86", 476) => Some(127_000.0),
        ("bmw_m2", 476) => Some(116_000.0),
        ("gt4", 476) => Some(108_000.0),
        ("gt3", 476) => Some(99_000.0),
        ("lmp2", 476) => Some(86_000.0),

        // --- Portland (536, ~1:10 GT3) ---
        ("mx5", 536) => Some(92_000.0),
        ("gr86", 536) => Some(90_000.0),
        ("bmw_m2", 536) => Some(82_000.0),
        ("gt4", 536) => Some(76_000.0),
        ("gt3", 536) => Some(70_000.0),
        ("lmp2", 536) => Some(61_000.0),

        // --- Coronado (589, ~2:10 GT3, 5,5 km) ---
        ("mx5", 589) => Some(172_000.0),
        ("gr86", 589) => Some(166_000.0),
        ("bmw_m2", 589) => Some(152_000.0),
        ("gt4", 589) => Some(142_000.0),
        ("gt3", 589) => Some(130_000.0),
        ("lmp2", 589) => Some(113_000.0),

        // --- Sachsenring (521, ~1:19 GT3) ---
        ("mx5", 521) => Some(104_000.0),
        ("gr86", 521) => Some(101_000.0),
        ("bmw_m2", 521) => Some(92_000.0),
        ("gt4", 521) => Some(86_000.0),
        ("gt3", 521) => Some(79_000.0),
        ("lmp2", 521) => Some(69_000.0),

        // --- The Bend GT Circuit (540, ~1:50 GT3) ---
        ("mx5", 540) => Some(145_000.0),
        ("gr86", 540) => Some(141_000.0),
        ("bmw_m2", 540) => Some(129_000.0),
        ("gt4", 540) => Some(120_000.0),
        ("gt3", 540) => Some(110_000.0),
        ("lmp2", 540) => Some(96_000.0),

        // --- The Bend International (541, 7,7 km) ---
        ("mx5", 541) => Some(231_000.0),
        ("gr86", 541) => Some(224_000.0),
        ("bmw_m2", 541) => Some(205_000.0),
        ("gt4", 541) => Some(191_000.0),
        ("gt3", 541) => Some(175_000.0),
        ("lmp2", 541) => Some(152_000.0),

        // --- St. Petersburg (584, ~1:41 GT3) ---
        ("mx5", 584) => Some(133_000.0),
        ("gr86", 584) => Some(129_000.0),
        ("bmw_m2", 584) => Some(118_000.0),
        ("gt4", 584) => Some(110_000.0),
        ("gt3", 584) => Some(101_000.0),
        ("lmp2", 584) => Some(88_000.0),

        // --- Willow Springs Big Willow (481, ~1:14 GT3) ---
        ("mx5", 481) => Some(98_000.0),
        ("gr86", 481) => Some(95_000.0),
        ("bmw_m2", 481) => Some(87_000.0),
        ("gt4", 481) => Some(81_000.0),
        ("gt3", 481) => Some(74_000.0),
        ("lmp2", 481) => Some(64_000.0),

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Dificuldade de pista (mapa estático para pistas com identidade reconhecida)
// ---------------------------------------------------------------------------

fn track_difficulty_for(track_id: u32) -> f64 {
    match track_id {
        249 => 1.6, // Nordschleife
        219 => 1.5, // Mount Panorama / Bathurst
        252 => 1.5, // Nürburgring Combined 24h
        523 => 1.4, // Spa
        527 => 1.3, // Cadwell Park
        168 => 1.3, // Suzuka
        413 => 1.2, // Hungaroring (técnico/lento)
        268 => 1.2, // Le Mans (longo e exigente)
        554 => 0.9, // Charlotte Roval (mais fácil de ultrapassar)
        192 => 0.9, // Daytona Road (roval)
        // ── Venues novos (pesquisados) ──
        405 => 1.45, // Chicago Street (90°, muros, baixa aderência)
        580 => 1.35, // Adelaide (street, muros)
        589 => 1.35, // Coronado (airbase, muros de concreto)
        584 => 1.35, // St. Petersburg (bumpy, muros)
        218 => 1.30, // Gilles Villeneuve (Wall of Champions)
        521 => 1.30, // Sachsenring (waterfall cego, Omega)
        481 => 1.30, // Willow Springs (sweepers rápidos, cegos)
        539 => 1.25, // Miami (setor técnico murado)
        585 => 1.20, // Barber (ápices cegos, elevação)
        423 => 1.20, // Knockhill (ondulado, cego)
        475 => 1.15, // Aragón GP (esses cegos, comprometido)
        476 => 1.12, // Aragón National
        473 => 1.10, // Jerez
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
        let hungaroring =
            resolve_simulation_profile("gt4", 413, 22.0, WeatherCondition::Dry, 30, 12); // Tight
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
}
