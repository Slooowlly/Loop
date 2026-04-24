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

static TRACKS: &[TrackInfo] = &[
    TrackInfo {
        track_id: 8,
        nome: "Summit Point - Jefferson Circuit",
        nome_curto: "Jefferson",
        pais: "🇺🇸 EUA",
        comprimento_km: 1.6,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 9,
        nome: "Summit Point - Summit Point Raceway",
        nome_curto: "Summit Point",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.2,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 14,
        nome: "Lime Rock Park - Full Course",
        nome_curto: "Lime Rock",
        pais: "🇺🇸 EUA",
        comprimento_km: 2.4,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 47,
        nome: "Laguna Seca - Full Course",
        nome_curto: "Laguna Seca",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.6,
        rain_group: RainGroup::Dry,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 166,
        nome: "Okayama International Circuit",
        nome_curto: "Okayama",
        pais: "🇯🇵 Japão",
        comprimento_km: 3.7,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 261,
        nome: "Oulton Park - Fosters",
        nome_curto: "Oulton Fosters",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 4.3,
        rain_group: RainGroup::Rainy,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 300,
        nome: "Brands Hatch - Grand Prix",
        nome_curto: "Brands GP",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 3.9,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 301,
        nome: "Brands Hatch - Indy",
        nome_curto: "Brands Indy",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 1.9,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 325,
        nome: "Tsukuba Circuit - 2000 Full Course",
        nome_curto: "Tsukuba",
        pais: "🇯🇵 Japão",
        comprimento_km: 2.0,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 341,
        nome: "Oulton Park - Island",
        nome_curto: "Oulton Island",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 3.6,
        rain_group: RainGroup::Rainy,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 554,
        nome: "Charlotte Motor Speedway - Roval",
        nome_curto: "Charlotte Roval",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.7,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Roval,
    },
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
        track_id: 451,
        nome: "Rudskogen Motorsenter",
        nome_curto: "Rudskogen",
        pais: "🇳🇴 Noruega",
        comprimento_km: 3.3,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
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
        track_id: 440,
        nome: "Winton Motor Raceway - Club Circuit",
        nome_curto: "Winton",
        pais: "🇦🇺 Austrália",
        comprimento_km: 2.0,
        rain_group: RainGroup::Normal,
        gratuita: true,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 45,
        nome: "Daytona International Speedway - Road Course",
        nome_curto: "Daytona Road",
        pais: "🇺🇸 EUA",
        comprimento_km: 5.7,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Roval,
    },
    TrackInfo {
        track_id: 51,
        nome: "Mid-Ohio Sports Car Course",
        nome_curto: "Mid-Ohio",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.8,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 52,
        nome: "Road America",
        nome_curto: "Road America",
        pais: "🇺🇸 EUA",
        comprimento_km: 6.5,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 53,
        nome: "Sonoma Raceway - Full Course",
        nome_curto: "Sonoma",
        pais: "🇺🇸 EUA",
        comprimento_km: 4.0,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 58,
        nome: "Virginia International Raceway - Full Course",
        nome_curto: "VIR Full",
        pais: "🇺🇸 EUA",
        comprimento_km: 5.3,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 67,
        nome: "Watkins Glen International - Boot",
        nome_curto: "Watkins Boot",
        pais: "🇺🇸 EUA",
        comprimento_km: 5.4,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 68,
        nome: "Watkins Glen International - Short",
        nome_curto: "Watkins Short",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.7,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 93,
        nome: "Autodromo Nazionale Monza",
        nome_curto: "Monza",
        pais: "🇮🇹 Itália",
        comprimento_km: 5.8,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 106,
        nome: "Silverstone Circuit - Grand Prix",
        nome_curto: "Silverstone GP",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 5.9,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 119,
        nome: "Mount Panorama Motor Racing Circuit",
        nome_curto: "Bathurst",
        pais: "🇦🇺 Austrália",
        comprimento_km: 6.2,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 125,
        nome: "Canadian Tire Motorsport Park (Mosport)",
        nome_curto: "Mosport",
        pais: "🇨🇦 Canadá",
        comprimento_km: 3.9,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 164,
        nome: "Suzuka International Racing Course",
        nome_curto: "Suzuka",
        pais: "🇯🇵 Japão",
        comprimento_km: 5.8,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 169,
        nome: "Philip Island Grand Prix Circuit",
        nome_curto: "Philip Island",
        pais: "🇦🇺 Austrália",
        comprimento_km: 4.4,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 185,
        nome: "Indianapolis Motor Speedway - Road Course",
        nome_curto: "Indy Road",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.9,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 188,
        nome: "Circuit de Spa-Francorchamps",
        nome_curto: "Spa",
        pais: "🇧🇪 Bélgica",
        comprimento_km: 7.0,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 192,
        nome: "Nürburgring - Grand Prix Strecke",
        nome_curto: "Nürburgring GP",
        pais: "🇩🇪 Alemanha",
        comprimento_km: 5.1,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 193,
        nome: "Hockenheimring Baden-Württemberg - GP",
        nome_curto: "Hockenheim GP",
        pais: "🇩🇪 Alemanha",
        comprimento_km: 4.6,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 194,
        nome: "Hungaroring",
        nome_curto: "Hungaroring",
        pais: "🇭🇺 Hungria",
        comprimento_km: 4.4,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 195,
        nome: "Hockenheimring Baden-Württemberg - Short",
        nome_curto: "Hockenheim Short",
        pais: "🇩🇪 Alemanha",
        comprimento_km: 2.3,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 196,
        nome: "Nürburgring - Nordschleife",
        nome_curto: "Nordschleife",
        pais: "🇩🇪 Alemanha",
        comprimento_km: 20.8,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 197,
        nome: "Nürburgring - Sprint Strecke",
        nome_curto: "Nürburgring Sprint",
        pais: "🇩🇪 Alemanha",
        comprimento_km: 3.6,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 199,
        nome: "Autodromo Jose Carlos Pace (Interlagos)",
        nome_curto: "Interlagos",
        pais: "🇧🇷 Brasil",
        comprimento_km: 4.3,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 212,
        nome: "Circuit of the Americas",
        nome_curto: "COTA",
        pais: "🇺🇸 EUA",
        comprimento_km: 5.5,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 238,
        nome: "Sebring International Raceway",
        nome_curto: "Sebring",
        pais: "🇺🇸 EUA",
        comprimento_km: 5.9,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 244,
        nome: "Circuit de Nevers Magny-Cours",
        nome_curto: "Magny-Cours",
        pais: "🇫🇷 França",
        comprimento_km: 4.4,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 249,
        nome: "Road Atlanta - Full Course",
        nome_curto: "Road Atlanta",
        pais: "🇺🇸 EUA",
        comprimento_km: 4.1,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 259,
        nome: "Virginia International Raceway - Patriot",
        nome_curto: "VIR Patriot",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.3,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 281,
        nome: "Circuit de Barcelona-Catalunya",
        nome_curto: "Barcelona",
        pais: "🇪🇸 Espanha",
        comprimento_km: 4.7,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 287,
        nome: "Circuit de la Sarthe - 24 Hours",
        nome_curto: "Le Mans",
        pais: "🇫🇷 França",
        comprimento_km: 13.6,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 316,
        nome: "Snetterton Circuit - 300",
        nome_curto: "Snetterton 300",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 4.8,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 318,
        nome: "Long Beach Street Circuit",
        nome_curto: "Long Beach",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.2,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 335,
        nome: "Thruxton Circuit",
        nome_curto: "Thruxton",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 3.8,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 339,
        nome: "Cadwell Park Circuit",
        nome_curto: "Cadwell Park",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 3.5,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 342,
        nome: "Oulton Park - International",
        nome_curto: "Oulton Intl",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 4.6,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 350,
        nome: "Circuit Zolder",
        nome_curto: "Zolder",
        pais: "🇧🇪 Bélgica",
        comprimento_km: 4.0,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 360,
        nome: "Circuit Paul Ricard",
        nome_curto: "Paul Ricard",
        pais: "🇫🇷 França",
        comprimento_km: 5.8,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 363,
        nome: "Misano World Circuit Marco Simoncelli",
        nome_curto: "Misano",
        pais: "🇮🇹 Itália",
        comprimento_km: 4.2,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 373,
        nome: "Fuji International Speedway",
        nome_curto: "Fuji",
        pais: "🇯🇵 Japão",
        comprimento_km: 4.6,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 382,
        nome: "Autodromo di Vallelunga",
        nome_curto: "Vallelunga",
        pais: "🇮🇹 Itália",
        comprimento_km: 4.1,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 389,
        nome: "Circuit Zandvoort",
        nome_curto: "Zandvoort",
        pais: "🇳🇱 Holanda",
        comprimento_km: 4.3,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 393,
        nome: "Bahrain International Circuit",
        nome_curto: "Bahrain",
        pais: "🇧🇭 Bahrein",
        comprimento_km: 5.4,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 397,
        nome: "Red Bull Ring",
        nome_curto: "Red Bull Ring",
        pais: "🇦🇹 Áustria",
        comprimento_km: 4.3,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 399,
        nome: "Donington Park - Grand Prix",
        nome_curto: "Donington GP",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 4.0,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 400,
        nome: "Donington Park - National",
        nome_curto: "Donington Natl",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 3.1,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 404,
        nome: "Automotodrom Brno",
        nome_curto: "Brno",
        pais: "🇨🇿 Tchéquia",
        comprimento_km: 5.4,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 409,
        nome: "TT Circuit Assen",
        nome_curto: "Assen",
        pais: "🇳🇱 Holanda",
        comprimento_km: 4.5,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 413,
        nome: "Autodromo Hermanos Rodriguez",
        nome_curto: "Mexico City",
        pais: "🇲🇽 México",
        comprimento_km: 4.3,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 420,
        nome: "Istanbul Park",
        nome_curto: "Istanbul",
        pais: "🇹🇷 Turquia",
        comprimento_km: 5.3,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 421,
        nome: "Sandown International Motor Raceway",
        nome_curto: "Sandown",
        pais: "🇦🇺 Austrália",
        comprimento_km: 3.1,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 425,
        nome: "Autodromo Internacional do Algarve (Portimão)",
        nome_curto: "Portimão",
        pais: "🇵🇹 Portugal",
        comprimento_km: 4.7,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 452,
        nome: "Autodromo Internazionale del Mugello",
        nome_curto: "Mugello",
        pais: "🇮🇹 Itália",
        comprimento_km: 5.2,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 455,
        nome: "Autodromo Enzo e Dino Ferrari (Imola)",
        nome_curto: "Imola",
        pais: "🇮🇹 Itália",
        comprimento_km: 4.9,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 504,
        nome: "Detroit Grand Prix Street Circuit",
        nome_curto: "Detroit",
        pais: "🇺🇸 EUA",
        comprimento_km: 3.0,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 513,
        nome: "Kyalami Grand Prix Circuit",
        nome_curto: "Kyalami",
        pais: "🇿🇦 África do Sul",
        comprimento_km: 4.5,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 516,
        nome: "Yas Marina Circuit",
        nome_curto: "Yas Marina",
        pais: "🇦🇪 EAU",
        comprimento_km: 5.5,
        rain_group: RainGroup::Dry,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 520,
        nome: "Autodromo di Modena",
        nome_curto: "Modena",
        pais: "🇮🇹 Itália",
        comprimento_km: 2.1,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 524,
        nome: "Circuit de Catalunya - National",
        nome_curto: "Barcelona Natl",
        pais: "🇪🇸 Espanha",
        comprimento_km: 3.0,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 528,
        nome: "Nürburgring - Combined (24H)",
        nome_curto: "Nürburgring 24H",
        pais: "🇩🇪 Alemanha",
        comprimento_km: 25.4,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 532,
        nome: "Silverstone Circuit - National",
        nome_curto: "Silverstone Natl",
        pais: "🇬🇧 Reino Unido",
        comprimento_km: 3.7,
        rain_group: RainGroup::Rainy,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 538,
        nome: "Suzuka International - East Short",
        nome_curto: "Suzuka East",
        pais: "🇯🇵 Japão",
        comprimento_km: 2.2,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 542,
        nome: "Okayama International Circuit - Short",
        nome_curto: "Okayama Short",
        pais: "🇯🇵 Japão",
        comprimento_km: 1.7,
        rain_group: RainGroup::Normal,
        gratuita: false,
        tipo: TrackType::Road,
    },
    TrackInfo {
        track_id: 548,
        nome: "Autodromo Nazionale Monza - Junior",
        nome_curto: "Monza Junior",
        pais: "🇮🇹 Itália",
        comprimento_km: 2.4,
        rain_group: RainGroup::Normal,
        gratuita: false,
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

pub fn get_qualifying_duration(track_id: u32) -> u8 {
    let Some(track) = get_track(track_id) else {
        return 15;
    };

    if matches!(track.track_id, 196 | 287 | 528) {
        20
    } else if track.comprimento_km > 5.0 {
        18
    } else {
        15
    }
}

pub fn duracao_classificacao_para(comprimento_km: f64, track_id: u32) -> u32 {
    if matches!(track_id, 196 | 287 | 528) {
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
        assert!(!get_track(300).expect("Brands GP").gratuita);
        assert!(!get_track(301).expect("Brands Indy").gratuita);
    }

    #[test]
    fn current_free_road_tracks_are_in_catalog() {
        for track_id in [202, 440, 449, 451, 489, 515] {
            let track = get_track(track_id).unwrap_or_else(|| panic!("missing track {track_id}"));
            assert!(track.gratuita, "track {track_id} should be free");
        }
    }

    #[test]
    fn test_tracks_for_tier_3_includes_paid() {
        let tracks = get_tracks_for_tier(3);
        assert!(tracks.iter().any(|track| !track.gratuita));
    }

    #[test]
    fn test_rain_chance_by_group() {
        assert_eq!(get_rain_chance(47), 0.05);
        assert_eq!(get_rain_chance(8), 0.15);
        assert_eq!(get_rain_chance(261), 0.30);
    }

    #[test]
    fn test_qualifying_duration_default() {
        assert_eq!(get_qualifying_duration(47), 15);
    }
}
