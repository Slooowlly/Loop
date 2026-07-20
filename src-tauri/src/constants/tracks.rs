#![allow(dead_code)]

use crate::constants::categories::get_category_config;
use crate::models::enums::{RainGroup, TrackType};

pub struct TrackInfo {
    pub track_id: u32,
    pub nome: &'static str,
    pub nome_curto: &'static str,
    pub pais: &'static str,
    pub comprimento_km: f64,
    pub rain_group: RainGroup,
    pub gratuita: bool,
    pub tipo: TrackType,
}

pub type TrackDefinition = TrackInfo;

// ─────────────────────────────────────────────────────────────────────────────
// Catálogo de pistas — ids reais do iRacing (crava dos 108 eventos do usuário).
// Fonte autoritativa: WeekendInfo.TrackID das sessões do jogador (jul/2026).
// Cada entrada é um evento (venue + layout) que o jogador possui.
// `gratuita` preserva a curadoria anterior do conteúdo-base gratuito.
// ─────────────────────────────────────────────────────────────────────────────
static TRACKS: &[TrackInfo] = &[
    // ── Adelaide Street Circuit ──────────────────────────────────────────────
    TrackInfo {
        track_id: 580,
        nome: "Adelaide Street Circuit",
        nome_curto: "Adelaide",
        pais: "🇦🇺 Austrália",
        comprimento_km: 3.2,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Algarve International Circuit (Portimão) ─────────────────────────────
    TrackInfo {
        track_id: 509,
        nome: "Algarve International Circuit - Grand Prix",
        nome_curto: "Portimão",
        pais: "🇵🇹 Portugal",
        comprimento_km: 4.7,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 510,
        nome: "Algarve International Circuit - Grand Prix Chicanes",
        nome_curto: "Portimão Chicanes",
        pais: "🇵🇹 Portugal",
        comprimento_km: 4.6,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Autódromo Hermanos Rodríguez ─────────────────────────────────────────
    TrackInfo {
        track_id: 572,
        nome: "Autódromo Hermanos Rodríguez - Grand Prix",
        nome_curto: "Mexico City",
        pais: "🇲🇽 México",
        comprimento_km: 4.3,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 574,
        nome: "Autódromo Hermanos Rodríguez - National",
        nome_curto: "Mexico National",
        pais: "🇲🇽 México",
        comprimento_km: 3.6,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Autodromo Internazionale del Mugello ─────────────────────────────────
    TrackInfo {
        track_id: 498,
        nome: "Autodromo Internazionale del Mugello - Grand Prix",
        nome_curto: "Mugello",
        pais: "🇮🇹 Itália",
        comprimento_km: 5.2,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 499,
        nome: "Autodromo Internazionale del Mugello - Short",
        nome_curto: "Mugello Short",
        pais: "🇮🇹 Itália",
        comprimento_km: 4.3,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Autodromo Enzo e Dino Ferrari (Imola) ────────────────────────────────
    TrackInfo {
        track_id: 266,
        nome: "Autodromo Internazionale Enzo e Dino Ferrari - Grand Prix",
        nome_curto: "Imola",
        pais: "🇮🇹 Itália",
        comprimento_km: 4.9,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Autódromo José Carlos Pace (Interlagos) ──────────────────────────────
    TrackInfo {
        track_id: 212,
        nome: "Autódromo José Carlos Pace - Grand Prix",
        nome_curto: "Interlagos",
        pais: "🇧🇷 Brasil",
        comprimento_km: 4.3,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Autodromo Nazionale Monza ────────────────────────────────────────────
    TrackInfo {
        track_id: 239,
        nome: "Autodromo Nazionale Monza - Grand Prix",
        nome_curto: "Monza",
        pais: "🇮🇹 Itália",
        comprimento_km: 5.8,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 247,
        nome: "Autodromo Nazionale Monza - Combined",
        nome_curto: "Monza Combined",
        pais: "🇮🇹 Itália",
        comprimento_km: 10.0,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 244,
        nome: "Autodromo Nazionale Monza - GP without first chicane",
        nome_curto: "Monza s/ 1ª chicane",
        pais: "🇮🇹 Itália",
        comprimento_km: 5.8,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 242,
        nome: "Autodromo Nazionale Monza - GP without chicanes",
        nome_curto: "Monza s/ chicanes",
        pais: "🇮🇹 Itália",
        comprimento_km: 5.8,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Barber Motorsports Park ──────────────────────────────────────────────
    TrackInfo {
        track_id: 585,
        nome: "Barber Motorsports Park",
        nome_curto: "Barber",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.8,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Brands Hatch Circuit ─────────────────────────────────────────────────
    TrackInfo {
        track_id: 145,
        nome: "Brands Hatch Circuit - Grand Prix",
        nome_curto: "Brands GP",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 3.9,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Cadwell Park Circuit ─────────────────────────────────────────────────
    TrackInfo {
        track_id: 527,
        nome: "Cadwell Park Circuit - Full",
        nome_curto: "Cadwell Park",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 3.5,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Canadian Tire Motorsport Park (Mosport) ──────────────────────────────
    TrackInfo {
        track_id: 144,
        nome: "Canadian Tire Motorsport Park",
        nome_curto: "Mosport",
        pais: "🇨🇦 Canadá",
        comprimento_km: 3.9,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Charlotte Motor Speedway (Roval) ─────────────────────────────────────
    TrackInfo {
        track_id: 554,
        nome: "Charlotte Motor Speedway - Roval 2025",
        nome_curto: "Charlotte Roval",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.7,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Roval,
    },
    // ── Chicago Street Course ────────────────────────────────────────────────
    TrackInfo {
        track_id: 405,
        nome: "Chicago Street Course - Prototype",
        nome_curto: "Chicago Street",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.4,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Circuit de Barcelona-Catalunya ───────────────────────────────────────
    TrackInfo {
        track_id: 345,
        nome: "Circuit de Barcelona-Catalunya - Grand Prix",
        nome_curto: "Barcelona",
        pais: "🇪🇸 Espanha",
        comprimento_km: 4.7,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 346,
        nome: "Circuit de Barcelona-Catalunya - National",
        nome_curto: "Barcelona Natl",
        pais: "🇪🇸 Espanha",
        comprimento_km: 3.0,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Circuit de Lédenon ───────────────────────────────────────────────────
    TrackInfo {
        track_id: 489,
        nome: "Circuit de Lédenon",
        nome_curto: "Lédenon",
        pais: "🇫🇷 França",
        comprimento_km: 3.2,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    // ── Circuit de Nevers Magny-Cours ────────────────────────────────────────
    TrackInfo {
        track_id: 463,
        nome: "Circuit de Nevers Magny-Cours",
        nome_curto: "Magny-Cours",
        pais: "🇫🇷 França",
        comprimento_km: 4.4,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Circuit de Spa-Francorchamps ─────────────────────────────────────────
    TrackInfo {
        track_id: 523,
        nome: "Circuit de Spa-Francorchamps - Grand Prix Pits",
        nome_curto: "Spa",
        pais: "🇧🇪 Bélgica",
        comprimento_km: 7.0,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 525,
        nome: "Circuit de Spa-Francorchamps - Endurance",
        nome_curto: "Spa Endurance",
        pais: "🇧🇪 Bélgica",
        comprimento_km: 7.0,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Circuit des 24 Heures du Mans ────────────────────────────────────────
    TrackInfo {
        track_id: 268,
        nome: "Circuit des 24 Heures du Mans",
        nome_curto: "Le Mans",
        pais: "🇫🇷 França",
        comprimento_km: 13.6,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Circuit Gilles Villeneuve ────────────────────────────────────────────
    TrackInfo {
        track_id: 218,
        nome: "Circuit Gilles Villeneuve",
        nome_curto: "Montreal",
        pais: "🇨🇦 Canadá",
        comprimento_km: 4.4,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Circuit of the Americas ──────────────────────────────────────────────
    TrackInfo {
        track_id: 229,
        nome: "Circuit of the Americas - Grand Prix",
        nome_curto: "COTA",
        pais: "🇺🇸 EUA",
        comprimento_km: 5.5,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Circuit Zandvoort ────────────────────────────────────────────────────
    TrackInfo {
        track_id: 485,
        nome: "Circuit Zandvoort - Grand Prix",
        nome_curto: "Zandvoort",
        pais: "🇳🇱 Holanda",
        comprimento_km: 4.3,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 487,
        nome: "Circuit Zandvoort - Nationaal",
        nome_curto: "Zandvoort Natl",
        pais: "🇳🇱 Holanda",
        comprimento_km: 3.0,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 486,
        nome: "Circuit Zandvoort - Grand Prix w/chicane",
        nome_curto: "Zandvoort Chicane",
        pais: "🇳🇱 Holanda",
        comprimento_km: 4.3,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Circuit Zolder ───────────────────────────────────────────────────────
    TrackInfo {
        track_id: 199,
        nome: "Circuit Zolder - Grand Prix",
        nome_curto: "Zolder",
        pais: "🇧🇪 Bélgica",
        comprimento_km: 4.0,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Circuito de Jerez - Ángel Nieto ──────────────────────────────────────
    TrackInfo {
        track_id: 473,
        nome: "Circuito de Jerez - Ángel Nieto - Grand Prix",
        nome_curto: "Jerez",
        pais: "🇪🇸 Espanha",
        comprimento_km: 4.4,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Circuito de Navarra ──────────────────────────────────────────────────
    TrackInfo {
        track_id: 515,
        nome: "Circuito de Navarra - Speed Circuit",
        nome_curto: "Navarra",
        pais: "🇪🇸 Espanha",
        comprimento_km: 3.9,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 516,
        nome: "Circuito de Navarra - Speed Circuit Medium",
        nome_curto: "Navarra Medium",
        pais: "🇪🇸 Espanha",
        comprimento_km: 3.4,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 517,
        nome: "Circuito de Navarra - Speed Circuit Short",
        nome_curto: "Navarra Short",
        pais: "🇪🇸 Espanha",
        comprimento_km: 2.9,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    // ── Daytona International Speedway (Road Course) ─────────────────────────
    TrackInfo {
        track_id: 192,
        nome: "Daytona International Speedway - Road Course",
        nome_curto: "Daytona Road",
        pais: "🇺🇸 EUA",
        comprimento_km: 5.7,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Roval,
    },
    // ── Detroit Grand Prix at Belle Isle ─────────────────────────────────────
    TrackInfo {
        track_id: 319,
        nome: "Detroit Grand Prix at Belle Isle",
        nome_curto: "Belle Isle",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.8,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Donington Park Racing Circuit ────────────────────────────────────────
    TrackInfo {
        track_id: 233,
        nome: "Donington Park Racing Circuit - Grand Prix",
        nome_curto: "Donington GP",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 4.0,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Fuji International Speedway ───────────────────────────────────────────
    TrackInfo {
        track_id: 444,
        nome: "Fuji International Speedway - Grand Prix",
        nome_curto: "Fuji",
        pais: "🇯🇵 Japão",
        comprimento_km: 4.6,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 445,
        nome: "Fuji International Speedway - No Chicane",
        nome_curto: "Fuji No Chicane",
        pais: "🇯🇵 Japão",
        comprimento_km: 4.5,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Hockenheimring Baden-Württemberg ─────────────────────────────────────
    TrackInfo {
        track_id: 390,
        nome: "Hockenheimring Baden-Württemberg - Grand Prix",
        nome_curto: "Hockenheim",
        pais: "🇩🇪 Alemanha",
        comprimento_km: 4.6,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Hungaroring ──────────────────────────────────────────────────────────
    TrackInfo {
        track_id: 413,
        nome: "Hungaroring",
        nome_curto: "Hungaroring",
        pais: "🇭🇺 Hungria",
        comprimento_km: 4.4,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Indianapolis Motor Speedway (Road Course) ────────────────────────────
    TrackInfo {
        track_id: 448,
        nome: "Indianapolis Motor Speedway - Road Course",
        nome_curto: "Indy Road",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.9,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Roval,
    },
    // ── Knockhill Racing Circuit ─────────────────────────────────────────────
    TrackInfo {
        track_id: 423,
        nome: "Knockhill Racing Circuit - International",
        nome_curto: "Knockhill",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 2.0,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Lime Rock Park ───────────────────────────────────────────────────────
    TrackInfo {
        track_id: 353,
        nome: "Lime Rock Park - Grand Prix",
        nome_curto: "Lime Rock",
        pais: "🇺🇸 EUA",
        comprimento_km: 2.4,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 352,
        nome: "Lime Rock Park - Classic",
        nome_curto: "Lime Rock Classic",
        pais: "🇺🇸 EUA",
        comprimento_km: 2.4,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 354,
        nome: "Lime Rock Park - Chicanes",
        nome_curto: "Lime Rock Chicanes",
        pais: "🇺🇸 EUA",
        comprimento_km: 2.5,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    // ── Long Beach Street Circuit ────────────────────────────────────────────
    TrackInfo {
        track_id: 179,
        nome: "Long Beach Street Circuit",
        nome_curto: "Long Beach",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.2,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Miami International Autodrome ─────────────────────────────────────────
    TrackInfo {
        track_id: 539,
        nome: "Miami International Autodrome - Grand Prix",
        nome_curto: "Miami",
        pais: "🇺🇸 EUA",
        comprimento_km: 5.4,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Mid-Ohio Sports Car Course ───────────────────────────────────────────
    TrackInfo {
        track_id: 153,
        nome: "Mid-Ohio Sports Car Course - Full Course",
        nome_curto: "Mid-Ohio",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.8,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 154,
        nome: "Mid-Ohio Sports Car Course - Chicane",
        nome_curto: "Mid-Ohio Chicane",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.6,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Misano World Circuit Marco Simoncelli ────────────────────────────────
    TrackInfo {
        track_id: 501,
        nome: "Misano World Circuit Marco Simoncelli - Grand Prix",
        nome_curto: "Misano",
        pais: "🇮🇹 Itália",
        comprimento_km: 4.2,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Mobility Resort Motegi ───────────────────────────────────────────────
    TrackInfo {
        track_id: 195,
        nome: "Mobility Resort Motegi - Grand Prix",
        nome_curto: "Motegi",
        pais: "🇯🇵 Japão",
        comprimento_km: 4.8,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── MotorLand Aragón ─────────────────────────────────────────────────────
    TrackInfo {
        track_id: 475,
        nome: "MotorLand Aragón - Grand Prix",
        nome_curto: "Aragón",
        pais: "🇪🇸 Espanha",
        comprimento_km: 5.3,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 476,
        nome: "MotorLand Aragón - National",
        nome_curto: "Aragón Natl",
        pais: "🇪🇸 Espanha",
        comprimento_km: 4.7,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Motorsport Arena Oschersleben ────────────────────────────────────────
    TrackInfo {
        track_id: 449,
        nome: "Motorsport Arena Oschersleben - Grand Prix",
        nome_curto: "Oschersleben",
        pais: "🇩🇪 Alemanha",
        comprimento_km: 3.7,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 454,
        nome: "Motorsport Arena Oschersleben - Alternate",
        nome_curto: "Oschersleben Alt",
        pais: "🇩🇪 Alemanha",
        comprimento_km: 3.7,
        rain_group: RainGroup::Rainy,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 455,
        nome: "Motorsport Arena Oschersleben - B Course",
        nome_curto: "Oschersleben B",
        pais: "🇩🇪 Alemanha",
        comprimento_km: 2.4,
        rain_group: RainGroup::Rainy,
        gratuita: true,
        tipo: TrackType::Road,
    },
    // ── Mount Panorama Circuit (Bathurst) ────────────────────────────────────
    TrackInfo {
        track_id: 219,
        nome: "Mount Panorama Circuit",
        nome_curto: "Bathurst",
        pais: "🇦🇺 Austrália",
        comprimento_km: 6.2,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Nürburgring Combined ─────────────────────────────────────────────────
    TrackInfo {
        track_id: 263,
        nome: "Nürburgring Combined - Gesamtstrecke Short w/out Arena",
        nome_curto: "Nürburgring Combined",
        pais: "🇩🇪 Alemanha",
        comprimento_km: 5.1,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 252,
        nome: "Nürburgring Combined - Gesamtstrecke 24h",
        nome_curto: "Nürburgring 24h",
        pais: "🇩🇪 Alemanha",
        comprimento_km: 25.4,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Nürburgring Nordschleife ─────────────────────────────────────────────
    TrackInfo {
        track_id: 249,
        nome: "Nürburgring Nordschleife - Industriefahrten",
        nome_curto: "Nordschleife",
        pais: "🇩🇪 Alemanha",
        comprimento_km: 20.8,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Okayama International Circuit ─────────────────────────────────────────
    TrackInfo {
        track_id: 166,
        nome: "Okayama International Circuit - Full Course",
        nome_curto: "Okayama",
        pais: "🇯🇵 Japão",
        comprimento_km: 3.7,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 167,
        nome: "Okayama International Circuit - Short",
        nome_curto: "Okayama Short",
        pais: "🇯🇵 Japão",
        comprimento_km: 2.4,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    // ── Oran Park Raceway ────────────────────────────────────────────────────
    TrackInfo {
        track_id: 202,
        nome: "Oran Park Raceway - Grand Prix",
        nome_curto: "Oran Park",
        pais: "🇦🇺 Austrália",
        comprimento_km: 2.6,
        rain_group: RainGroup::Dry,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 208,
        nome: "Oran Park Raceway - South",
        nome_curto: "Oran Park South",
        pais: "🇦🇺 Austrália",
        comprimento_km: 2.0,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 207,
        nome: "Oran Park Raceway - North",
        nome_curto: "Oran Park North",
        pais: "🇦🇺 Austrália",
        comprimento_km: 1.6,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    // ── Oulton Park Circuit ──────────────────────────────────────────────────
    TrackInfo {
        track_id: 180,
        nome: "Oulton Park Circuit - International",
        nome_curto: "Oulton Intl",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 4.4,
        rain_group: RainGroup::Rainy,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 181,
        nome: "Oulton Park Circuit - Fosters",
        nome_curto: "Oulton Fosters",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 2.7,
        rain_group: RainGroup::Rainy,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 182,
        nome: "Oulton Park Circuit - Island",
        nome_curto: "Oulton Island",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 3.6,
        rain_group: RainGroup::Rainy,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 183,
        nome: "Oulton Park Circuit - Intl w/out Hislop",
        nome_curto: "Oulton w/out Hislop",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 4.3,
        rain_group: RainGroup::Rainy,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 184,
        nome: "Oulton Park Circuit - Intl w/out Brittens",
        nome_curto: "Oulton w/out Brittens",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 4.3,
        rain_group: RainGroup::Rainy,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 185,
        nome: "Oulton Park Circuit - Intl w/no Chicanes",
        nome_curto: "Oulton w/no Chicanes",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 4.3,
        rain_group: RainGroup::Rainy,
        gratuita: true,
        tipo: TrackType::Road,
    },
    // ── Phillip Island Circuit ───────────────────────────────────────────────
    TrackInfo {
        track_id: 152,
        nome: "Phillip Island Circuit",
        nome_curto: "Phillip Island",
        pais: "🇦🇺 Austrália",
        comprimento_km: 4.4,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Portland International Raceway ────────────────────────────────────────
    TrackInfo {
        track_id: 536,
        nome: "Portland International Raceway - Full Circuit",
        nome_curto: "Portland",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.2,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Qualcomm Circuit (Naval Base Coronado) ───────────────────────────────
    TrackInfo {
        track_id: 589,
        nome: "Qualcomm Circuit (Naval Base Coronado)",
        nome_curto: "Coronado",
        pais: "🇺🇸 EUA",
        comprimento_km: 5.5,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Red Bull Ring ────────────────────────────────────────────────────────
    TrackInfo {
        track_id: 403,
        nome: "Red Bull Ring - Grand Prix",
        nome_curto: "Red Bull Ring",
        pais: "🇦🇹 Áustria",
        comprimento_km: 4.3,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Road America ─────────────────────────────────────────────────────────
    TrackInfo {
        track_id: 18,
        nome: "Road America - Full Course",
        nome_curto: "Road America",
        pais: "🇺🇸 EUA",
        comprimento_km: 6.5,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Road Atlanta ─────────────────────────────────────────────────────────
    TrackInfo {
        track_id: 127,
        nome: "Road Atlanta - Full Course",
        nome_curto: "Road Atlanta",
        pais: "🇺🇸 EUA",
        comprimento_km: 4.1,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Rudskogen Motorsenter ────────────────────────────────────────────────
    TrackInfo {
        track_id: 451,
        nome: "Rudskogen Motorsenter",
        nome_curto: "Rudskogen",
        pais: "🇳🇴 Noruega",
        comprimento_km: 3.3,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    // ── Sachsenring ──────────────────────────────────────────────────────────
    TrackInfo {
        track_id: 521,
        nome: "Sachsenring",
        nome_curto: "Sachsenring",
        pais: "🇩🇪 Alemanha",
        comprimento_km: 3.7,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Sandown International Motor Raceway ───────────────────────────────────
    TrackInfo {
        track_id: 443,
        nome: "Sandown International Motor Raceway",
        nome_curto: "Sandown",
        pais: "🇦🇺 Austrália",
        comprimento_km: 3.1,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Sebring International Raceway ─────────────────────────────────────────
    TrackInfo {
        track_id: 95,
        nome: "Sebring International Raceway - International",
        nome_curto: "Sebring",
        pais: "🇺🇸 EUA",
        comprimento_km: 5.9,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Shell V-Power Motorsport Park at The Bend ────────────────────────────
    TrackInfo {
        track_id: 540,
        nome: "The Bend Motorsport Park - GT Circuit",
        nome_curto: "The Bend GT",
        pais: "🇦🇺 Austrália",
        comprimento_km: 4.9,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 541,
        nome: "The Bend Motorsport Park - International Circuit",
        nome_curto: "The Bend Intl",
        pais: "🇦🇺 Austrália",
        comprimento_km: 7.7,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Silverstone Circuit ──────────────────────────────────────────────────
    TrackInfo {
        track_id: 341,
        nome: "Silverstone Circuit - Grand Prix",
        nome_curto: "Silverstone GP",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 5.9,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 342,
        nome: "Silverstone Circuit - International",
        nome_curto: "Silverstone Intl",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 3.6,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 343,
        nome: "Silverstone Circuit - National",
        nome_curto: "Silverstone Natl",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 2.6,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Snetterton Circuit ───────────────────────────────────────────────────
    TrackInfo {
        track_id: 297,
        nome: "Snetterton Circuit - 300",
        nome_curto: "Snetterton 300",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 4.8,
        rain_group: RainGroup::Rainy,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 298,
        nome: "Snetterton Circuit - 200",
        nome_curto: "Snetterton 200",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 3.2,
        rain_group: RainGroup::Rainy,
        gratuita: true,
        tipo: TrackType::Road,
    },
    // ── Sonoma Raceway ───────────────────────────────────────────────────────
    TrackInfo {
        track_id: 566,
        nome: "Sonoma Raceway - Cup Long",
        nome_curto: "Sonoma",
        pais: "🇺🇸 EUA",
        comprimento_km: 4.0,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 567,
        nome: "Sonoma Raceway - Cup Short",
        nome_curto: "Sonoma Short",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.2,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── St. Petersburg Grand Prix ────────────────────────────────────────────
    TrackInfo {
        track_id: 584,
        nome: "St. Petersburg Grand Prix",
        nome_curto: "St. Pete",
        pais: "🇺🇸 EUA",
        comprimento_km: 2.9,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Summit Point Raceway ─────────────────────────────────────────────────
    TrackInfo {
        track_id: 9,
        nome: "Summit Point Raceway",
        nome_curto: "Summit Point",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.2,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    // ── Suzuka International Racing Course ────────────────────────────────────
    TrackInfo {
        track_id: 168,
        nome: "Suzuka International Racing Course - Grand Prix",
        nome_curto: "Suzuka",
        pais: "🇯🇵 Japão",
        comprimento_km: 5.8,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Thruxton Circuit ─────────────────────────────────────────────────────
    TrackInfo {
        track_id: 532,
        nome: "Thruxton Circuit",
        nome_curto: "Thruxton",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 3.8,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Tsukuba Circuit ──────────────────────────────────────────────────────
    TrackInfo {
        track_id: 324,
        nome: "Tsukuba Circuit - 2000 Full",
        nome_curto: "Tsukuba",
        pais: "🇯🇵 Japão",
        comprimento_km: 2.0,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    // ── Virginia International Raceway ────────────────────────────────────────
    TrackInfo {
        track_id: 465,
        nome: "Virginia International Raceway - Full Course",
        nome_curto: "VIR Full",
        pais: "🇺🇸 EUA",
        comprimento_km: 5.3,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 467,
        nome: "Virginia International Raceway - North Course",
        nome_curto: "VIR North",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.6,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 468,
        nome: "Virginia International Raceway - South Course",
        nome_curto: "VIR South",
        pais: "🇺🇸 EUA",
        comprimento_km: 2.4,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 466,
        nome: "Virginia International Raceway - Grand Course",
        nome_curto: "VIR Grand",
        pais: "🇺🇸 EUA",
        comprimento_km: 6.8,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    // ── Watkins Glen International ────────────────────────────────────────────
    TrackInfo {
        track_id: 433,
        nome: "Watkins Glen International - Cup",
        nome_curto: "Watkins Cup",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.7,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── WeatherTech Raceway at Laguna Seca ───────────────────────────────────
    TrackInfo {
        track_id: 586,
        nome: "WeatherTech Raceway at Laguna Seca - 2026",
        nome_curto: "Laguna Seca",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.6,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Willow Springs International Raceway ──────────────────────────────────
    TrackInfo {
        track_id: 481,
        nome: "Willow Springs International Raceway",
        nome_curto: "Willow Springs",
        pais: "🇺🇸 EUA",
        comprimento_km: 4.0,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    // ── Winton Motor Raceway ─────────────────────────────────────────────────
    TrackInfo {
        track_id: 439,
        nome: "Winton Motor Raceway - National Circuit",
        nome_curto: "Winton National",
        pais: "🇦🇺 Austrália",
        comprimento_km: 3.0,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 440,
        nome: "Winton Motor Raceway - Club Circuit",
        nome_curto: "Winton",
        pais: "🇦🇺 Austrália",
        comprimento_km: 2.0,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
];

pub fn get_track(track_id: u32) -> Option<&'static TrackInfo> {
    TRACKS.iter().find(|track| track.track_id == track_id)
}

pub fn get_all_tracks() -> &'static [TrackInfo] {
    TRACKS
}

pub fn get_free_tracks() -> Vec<&'static TrackInfo> {
    TRACKS.iter().filter(|track| track.gratuita).collect()
}

/// Fallback de TESTE: quando a pista pedida é conteúdo PAGO (que o jogador pode
/// não possuir), devolve uma pista GRÁTIS determinística no lugar, para o export
/// conseguir rodar mesmo sem a pista real. Pistas já grátis (e a própria escolha)
/// passam intactas.
///
/// A escolha é determinística e INDEPENDENTE de ordem (`track_id % pool`): a mesma
/// pista paga sempre mapeia para a mesma free em todo export — o import compara o
/// resultado do iRacing contra a pista que foi de fato exportada, então o mapa
/// precisa ser estável entre exports. Prefere free do mesmo tipo (road/roval).
///
/// TODO(design final): trocar por "pistas que o jogador realmente possui" em vez
/// de substituir todo conteúdo pago — ver backlog de posse de pistas.
pub fn free_or_substitute(track_id: u32) -> Option<&'static TrackInfo> {
    let original = get_track(track_id);
    if let Some(track) = original {
        if track.gratuita {
            return Some(track);
        }
    }

    let free = get_free_tracks();
    if free.is_empty() {
        return original;
    }

    // Prefere manter o tipo (road/roval) da pista original; se não houver free do
    // mesmo tipo, cai para o pool free inteiro.
    let pool: Vec<&'static TrackInfo> = match original.map(|t| t.tipo) {
        Some(tipo) => {
            let same: Vec<&'static TrackInfo> =
                free.iter().copied().filter(|t| t.tipo == tipo).collect();
            if same.is_empty() {
                free
            } else {
                same
            }
        }
        None => free,
    };

    Some(pool[(track_id as usize) % pool.len()])
}

pub fn get_tracks_for_tier(tier: u8) -> Vec<&'static TrackInfo> {
    if tier <= 2 {
        get_free_tracks()
    } else {
        TRACKS.iter().collect()
    }
}

pub fn get_tracks_for_category(category_id: &str) -> Vec<&'static TrackInfo> {
    let Some(category) = get_category_config(category_id) else {
        return Vec::new();
    };

    get_tracks_for_tier(category.tier)
}

pub fn get_rain_chance(track_id: u32) -> f64 {
    match get_track(track_id).map(|track| track.rain_group) {
        Some(RainGroup::Dry) => 0.05,
        Some(RainGroup::Normal) => 0.15,
        Some(RainGroup::Rainy) => 0.30,
        None => 0.15,
    }
}

// Ids longos/endurance que merecem quali estendida: Nordschleife (249),
// Le Mans (268), Nürburgring 24h (252), Spa Endurance (525).
const LONG_QUALI_TRACK_IDS: &[u32] = &[249, 268, 252, 525];

pub fn get_qualifying_duration(track_id: u32) -> u8 {
    let Some(track) = get_track(track_id) else {
        return 15;
    };

    if LONG_QUALI_TRACK_IDS.contains(&track.track_id) {
        20
    } else if track.comprimento_km > 5.0 {
        18
    } else {
        15
    }
}

pub fn duracao_classificacao_para(comprimento_km: f64, track_id: u32) -> u32 {
    if LONG_QUALI_TRACK_IDS.contains(&track_id) {
        20
    } else if comprimento_km > 5.0 {
        18
    } else {
        15
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracks_for_tier_0_only_free() {
        let tracks = get_tracks_for_tier(0);
        assert!(!tracks.is_empty());
        assert!(tracks.iter().all(|track| track.gratuita));
    }

    #[test]
    fn brands_hatch_is_paid_content() {
        assert!(!get_track(145).expect("Brands GP").gratuita);
    }

    #[test]
    fn free_or_substitute_swaps_paid_for_free_and_is_stable() {
        // Adelaide (580) é conteúdo pago no catálogo.
        assert!(!get_track(580).expect("Adelaide").gratuita);
        let sub = free_or_substitute(580).expect("deve devolver uma substituta");
        assert!(sub.gratuita, "a substituta precisa ser uma pista grátis");
        // Determinístico: a mesma pista paga sempre mapeia para a mesma free (o import
        // compara o resultado contra a pista exportada).
        assert_eq!(sub.track_id, free_or_substitute(580).expect("estável").track_id);
    }

    #[test]
    fn free_or_substitute_keeps_free_tracks_untouched() {
        // 451 é uma pista grátis do catálogo → passa intacta.
        assert!(get_track(451).expect("free 451").gratuita);
        assert_eq!(free_or_substitute(451).expect("free").track_id, 451);
    }

    #[test]
    fn free_or_substitute_prefers_same_track_type() {
        // Adelaide (580) é Road; a substituta também deve ser Road (todas as free são).
        let sub = free_or_substitute(580).expect("substituta");
        assert_eq!(sub.tipo, TrackType::Road);
    }

    #[test]
    fn current_free_road_tracks_are_in_catalog() {
        for track_id in [202, 440, 449, 451, 489, 515] {
            let track = get_track(track_id).unwrap_or_else(|| panic!("missing track {track_id}"));
            assert!(track.gratuita, "track {track_id} should be free");
        }
    }

    #[test]
    fn catalog_has_no_duplicate_ids() {
        let mut ids: Vec<u32> = TRACKS.iter().map(|t| t.track_id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "há track_ids duplicados no catálogo");
    }

    #[test]
    fn catalog_covers_all_owned_events() {
        assert_eq!(TRACKS.len(), 107, "catálogo deve ter exatamente os eventos possuídos");
    }

    #[test]
    fn corrected_ids_match_iracing() {
        // Amostra das correções críticas contra o esquema legado.
        assert_eq!(get_track(239).expect("Monza").nome_curto, "Monza");
        assert_eq!(get_track(229).expect("COTA").nome_curto, "COTA");
        assert_eq!(get_track(212).expect("Interlagos").nome_curto, "Interlagos");
        assert_eq!(get_track(523).expect("Spa").nome_curto, "Spa");
        assert_eq!(get_track(168).expect("Suzuka").nome_curto, "Suzuka");
        assert_eq!(get_track(219).expect("Bathurst").nome_curto, "Bathurst");
        assert_eq!(get_track(586).expect("Laguna Seca").nome_curto, "Laguna Seca");
    }

    #[test]
    fn test_tracks_for_tier_3_includes_paid() {
        let tracks = get_tracks_for_tier(3);
        assert!(tracks.iter().any(|track| !track.gratuita));
    }

    #[test]
    fn test_rain_chance_by_group() {
        assert_eq!(get_rain_chance(586), 0.05); // Laguna Seca (Dry)
        assert_eq!(get_rain_chance(9), 0.15); // Summit Point (Normal)
        assert_eq!(get_rain_chance(181), 0.30); // Oulton Park Fosters (Rainy)
    }

    #[test]
    fn test_qualifying_duration_default() {
        assert_eq!(get_qualifying_duration(586), 15); // Laguna Seca (3.6km)
    }

    #[test]
    fn test_qualifying_duration_long_tracks() {
        assert_eq!(get_qualifying_duration(249), 20); // Nordschleife
        assert_eq!(get_qualifying_duration(268), 20); // Le Mans
        assert_eq!(get_qualifying_duration(252), 20); // Nürburgring 24h
    }
}
