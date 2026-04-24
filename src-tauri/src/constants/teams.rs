#![allow(dead_code)]

pub struct TeamTemplate {
    pub nome: &'static str,
    pub nome_curto: &'static str,
    pub categoria: &'static str,
    pub cor_primaria: &'static str,
    pub cor_secundaria: &'static str,
    pub pais_sede: &'static str,
    pub marca: Option<&'static str>,
    pub classe: Option<&'static str>,
    pub car_performance_base: f64,
    pub budget_base: f64,
    pub reputacao_base: f64,
}

static TEAMS: &[TeamTemplate] = &[
    TeamTemplate {
        nome: "Racing Academy Red",
        nome_curto: "RAR",
        categoria: "mazda_rookie",
        cor_primaria: "#e63946",
        cor_secundaria: "#e63946",
        pais_sede: "🇺🇸 EUA",
        marca: None,
        classe: None,
        car_performance_base: 1.0,
        budget_base: 22.0,
        reputacao_base: 18.0,
    },
    TeamTemplate {
        nome: "Thunderline Academy",
        nome_curto: "TLA",
        categoria: "mazda_rookie",
        cor_primaria: "#2f3542",
        cor_secundaria: "#2f3542",
        pais_sede: "🇺🇸 EUA",
        marca: None,
        classe: None,
        car_performance_base: 2.0,
        budget_base: 25.0,
        reputacao_base: 20.0,
    },
    TeamTemplate {
        nome: "Grid Start Racing School",
        nome_curto: "GSR",
        categoria: "mazda_rookie",
        cor_primaria: "#f6c90e",
        cor_secundaria: "#f6c90e",
        pais_sede: "🇬🇧 Reino Unido",
        marca: None,
        classe: None,
        car_performance_base: 0.0,
        budget_base: 20.0,
        reputacao_base: 15.0,
    },
    TeamTemplate {
        nome: "First Gear Motorsport",
        nome_curto: "FGM",
        categoria: "mazda_rookie",
        cor_primaria: "#3a86ff",
        cor_secundaria: "#3a86ff",
        pais_sede: "🇩🇪 Alemanha",
        marca: None,
        classe: None,
        car_performance_base: 1.5,
        budget_base: 23.0,
        reputacao_base: 17.0,
    },
    TeamTemplate {
        nome: "Apex Academy Racing",
        nome_curto: "AAR",
        categoria: "mazda_rookie",
        cor_primaria: "#2ec4b6",
        cor_secundaria: "#2ec4b6",
        pais_sede: "🇫🇷 França",
        marca: None,
        classe: None,
        car_performance_base: 2.5,
        budget_base: 28.0,
        reputacao_base: 22.0,
    },
    TeamTemplate {
        nome: "Velocity Prep Motorsport",
        nome_curto: "VPM",
        categoria: "mazda_rookie",
        cor_primaria: "#9b5de5",
        cor_secundaria: "#9b5de5",
        pais_sede: "🇯🇵 Japão",
        marca: None,
        classe: None,
        car_performance_base: 0.5,
        budget_base: 21.0,
        reputacao_base: 16.0,
    },
    TeamTemplate {
        nome: "Sakura Driver Academy",
        nome_curto: "SDA",
        categoria: "toyota_rookie",
        cor_primaria: "#d90429",
        cor_secundaria: "#d90429",
        pais_sede: "🇯🇵 Japão",
        marca: None,
        classe: None,
        car_performance_base: 2.0,
        budget_base: 27.0,
        reputacao_base: 22.0,
    },
    TeamTemplate {
        nome: "Kanzen Racing School",
        nome_curto: "KRS",
        categoria: "toyota_rookie",
        cor_primaria: "#264653",
        cor_secundaria: "#264653",
        pais_sede: "🇯🇵 Japão",
        marca: None,
        classe: None,
        car_performance_base: 2.5,
        budget_base: 29.0,
        reputacao_base: 25.0,
    },
    TeamTemplate {
        nome: "Open Road Academy",
        nome_curto: "ORA",
        categoria: "toyota_rookie",
        cor_primaria: "#8b5e34",
        cor_secundaria: "#8b5e34",
        pais_sede: "🇺🇸 EUA",
        marca: None,
        classe: None,
        car_performance_base: 0.5,
        budget_base: 21.0,
        reputacao_base: 16.0,
    },
    TeamTemplate {
        nome: "Speed Lab Rookies",
        nome_curto: "SLR",
        categoria: "toyota_rookie",
        cor_primaria: "#fb5607",
        cor_secundaria: "#fb5607",
        pais_sede: "🇺🇸 EUA",
        marca: None,
        classe: None,
        car_performance_base: 1.0,
        budget_base: 22.0,
        reputacao_base: 18.0,
    },
    TeamTemplate {
        nome: "Rising Stars Motorsport",
        nome_curto: "RSM",
        categoria: "toyota_rookie",
        cor_primaria: "#ffbe0b",
        cor_secundaria: "#ffbe0b",
        pais_sede: "🇬🇧 Reino Unido",
        marca: None,
        classe: None,
        car_performance_base: 1.5,
        budget_base: 24.0,
        reputacao_base: 20.0,
    },
    TeamTemplate {
        nome: "Fresh Start Racing",
        nome_curto: "FSR",
        categoria: "toyota_rookie",
        cor_primaria: "#80b918",
        cor_secundaria: "#80b918",
        pais_sede: "🇩🇪 Alemanha",
        marca: None,
        classe: None,
        car_performance_base: 0.0,
        budget_base: 20.0,
        reputacao_base: 15.0,
    },
    TeamTemplate {
        nome: "Roadster Touring Club",
        nome_curto: "RTC",
        categoria: "mazda_amador",
        cor_primaria: "#8c1d40",
        cor_secundaria: "#8c1d40",
        pais_sede: "🇺🇸 EUA",
        marca: None,
        classe: None,
        car_performance_base: 4.0,
        budget_base: 42.0,
        reputacao_base: 38.0,
    },
    TeamTemplate {
        nome: "Weekend Warriors Racing",
        nome_curto: "WWR",
        categoria: "mazda_amador",
        cor_primaria: "#455a64",
        cor_secundaria: "#455a64",
        pais_sede: "🇺🇸 EUA",
        marca: None,
        classe: None,
        car_performance_base: 2.0,
        budget_base: 33.0,
        reputacao_base: 28.0,
    },
    TeamTemplate {
        nome: "Club Racer Motorsport",
        nome_curto: "CRM",
        categoria: "mazda_amador",
        cor_primaria: "#00a896",
        cor_secundaria: "#00a896",
        pais_sede: "🇬🇧 Reino Unido",
        marca: None,
        classe: None,
        car_performance_base: 3.0,
        budget_base: 38.0,
        reputacao_base: 33.0,
    },
    TeamTemplate {
        nome: "Dual Exit Racing",
        nome_curto: "DER",
        categoria: "mazda_amador",
        cor_primaria: "#7b2cbf",
        cor_secundaria: "#7b2cbf",
        pais_sede: "🇩🇪 Alemanha",
        marca: None,
        classe: None,
        car_performance_base: 5.0,
        budget_base: 50.0,
        reputacao_base: 45.0,
    },
    TeamTemplate {
        nome: "Sunday Speed Club",
        nome_curto: "SSC",
        categoria: "mazda_amador",
        cor_primaria: "#ff9f1c",
        cor_secundaria: "#ff9f1c",
        pais_sede: "🇫🇷 França",
        marca: None,
        classe: None,
        car_performance_base: 1.5,
        budget_base: 30.0,
        reputacao_base: 25.0,
    },
    TeamTemplate {
        nome: "Grassroots Racing Team",
        nome_curto: "GRT",
        categoria: "mazda_amador",
        cor_primaria: "#2d6a4f",
        cor_secundaria: "#2d6a4f",
        pais_sede: "🇺🇸 EUA",
        marca: None,
        classe: None,
        car_performance_base: 2.5,
        budget_base: 35.0,
        reputacao_base: 30.0,
    },
    TeamTemplate {
        nome: "Petrolhead United",
        nome_curto: "PHU",
        categoria: "mazda_amador",
        cor_primaria: "#b5651d",
        cor_secundaria: "#b5651d",
        pais_sede: "🇬🇧 Reino Unido",
        marca: None,
        classe: None,
        car_performance_base: 3.5,
        budget_base: 40.0,
        reputacao_base: 35.0,
    },
    TeamTemplate {
        nome: "Track Day Heroes",
        nome_curto: "TDH",
        categoria: "mazda_amador",
        cor_primaria: "#4cc9f0",
        cor_secundaria: "#4cc9f0",
        pais_sede: "🇳🇱 Holanda",
        marca: None,
        classe: None,
        car_performance_base: 1.0,
        budget_base: 31.0,
        reputacao_base: 26.0,
    },
    TeamTemplate {
        nome: "Late Apex Contenders",
        nome_curto: "LAC",
        categoria: "mazda_amador",
        cor_primaria: "#4361ee",
        cor_secundaria: "#4361ee",
        pais_sede: "🇯🇵 Japão",
        marca: None,
        classe: None,
        car_performance_base: 4.5,
        budget_base: 47.0,
        reputacao_base: 42.0,
    },
    TeamTemplate {
        nome: "Amateur Hour Racing",
        nome_curto: "AHR",
        categoria: "mazda_amador",
        cor_primaria: "#f4f1de",
        cor_secundaria: "#f4f1de",
        pais_sede: "🇺🇸 EUA",
        marca: None,
        classe: None,
        car_performance_base: 1.0,
        budget_base: 32.0,
        reputacao_base: 27.0,
    },
    TeamTemplate {
        nome: "Eastline Cup Racing",
        nome_curto: "ECR",
        categoria: "toyota_amador",
        cor_primaria: "#ef233c",
        cor_secundaria: "#ef233c",
        pais_sede: "🇯🇵 Japão",
        marca: None,
        classe: None,
        car_performance_base: 4.5,
        budget_base: 48.0,
        reputacao_base: 43.0,
    },
    TeamTemplate {
        nome: "Street to Track Team",
        nome_curto: "STT",
        categoria: "toyota_amador",
        cor_primaria: "#1d3557",
        cor_secundaria: "#1d3557",
        pais_sede: "🇺🇸 EUA",
        marca: None,
        classe: None,
        car_performance_base: 2.0,
        budget_base: 34.0,
        reputacao_base: 29.0,
    },
    TeamTemplate {
        nome: "Flat Six Motorsport",
        nome_curto: "FSM",
        categoria: "toyota_amador",
        cor_primaria: "#ff7a00",
        cor_secundaria: "#ff7a00",
        pais_sede: "🇩🇪 Alemanha",
        marca: None,
        classe: None,
        car_performance_base: 3.0,
        budget_base: 39.0,
        reputacao_base: 34.0,
    },
    TeamTemplate {
        nome: "Gentleman Driver Club",
        nome_curto: "GDC",
        categoria: "toyota_amador",
        cor_primaria: "#b8a1ff",
        cor_secundaria: "#b8a1ff",
        pais_sede: "🇧🇪 Bélgica",
        marca: None,
        classe: None,
        car_performance_base: 5.0,
        budget_base: 52.0,
        reputacao_base: 47.0,
    },
    TeamTemplate {
        nome: "Akagi Touring Challenge",
        nome_curto: "AKG",
        categoria: "toyota_amador",
        cor_primaria: "#e9c46a",
        cor_secundaria: "#e9c46a",
        pais_sede: "🇯🇵 Japão",
        marca: None,
        classe: None,
        car_performance_base: 3.5,
        budget_base: 43.0,
        reputacao_base: 38.0,
    },
    TeamTemplate {
        nome: "Rev Happy Racing",
        nome_curto: "RHR",
        categoria: "toyota_amador",
        cor_primaria: "#06d6a0",
        cor_secundaria: "#06d6a0",
        pais_sede: "🇦🇺 Austrália",
        marca: None,
        classe: None,
        car_performance_base: 2.5,
        budget_base: 36.0,
        reputacao_base: 31.0,
    },
    TeamTemplate {
        nome: "Corner Workers Racing",
        nome_curto: "CWR",
        categoria: "toyota_amador",
        cor_primaria: "#f8f9fa",
        cor_secundaria: "#f8f9fa",
        pais_sede: "🇺🇸 EUA",
        marca: None,
        classe: None,
        car_performance_base: 1.5,
        budget_base: 31.0,
        reputacao_base: 26.0,
    },
    TeamTemplate {
        nome: "Daily Driver Racing",
        nome_curto: "DDR",
        categoria: "toyota_amador",
        cor_primaria: "#00b4d8",
        cor_secundaria: "#00b4d8",
        pais_sede: "🇬🇧 Reino Unido",
        marca: None,
        classe: None,
        car_performance_base: 1.0,
        budget_base: 30.0,
        reputacao_base: 25.0,
    },
    TeamTemplate {
        nome: "Smooth Line Motorsport",
        nome_curto: "SLM",
        categoria: "toyota_amador",
        cor_primaria: "#3a0ca3",
        cor_secundaria: "#3a0ca3",
        pais_sede: "🇫🇷 França",
        marca: None,
        classe: None,
        car_performance_base: 4.0,
        budget_base: 45.0,
        reputacao_base: 40.0,
    },
    TeamTemplate {
        nome: "Over the Limit Racing",
        nome_curto: "OTL",
        categoria: "toyota_amador",
        cor_primaria: "#9d0208",
        cor_secundaria: "#9d0208",
        pais_sede: "🇳🇱 Holanda",
        marca: None,
        classe: None,
        car_performance_base: 2.0,
        budget_base: 33.0,
        reputacao_base: 28.0,
    },
    TeamTemplate {
        nome: "Bayern Division",
        nome_curto: "BYD",
        categoria: "bmw_m2",
        cor_primaria: "#004e98",
        cor_secundaria: "#004e98",
        pais_sede: "🇩🇪 Alemanha",
        marca: None,
        classe: None,
        car_performance_base: 7.0,
        budget_base: 58.0,
        reputacao_base: 54.0,
    },
    TeamTemplate {
        nome: "M Power",
        nome_curto: "MPR",
        categoria: "bmw_m2",
        cor_primaria: "#1b1b1e",
        cor_secundaria: "#1b1b1e",
        pais_sede: "🇩🇪 Alemanha",
        marca: None,
        classe: None,
        car_performance_base: 6.0,
        budget_base: 52.0,
        reputacao_base: 48.0,
    },
    TeamTemplate {
        nome: "Blue Propeller",
        nome_curto: "BPR",
        categoria: "bmw_m2",
        cor_primaria: "#0072ce",
        cor_secundaria: "#0072ce",
        pais_sede: "🇬🇧 Reino Unido",
        marca: None,
        classe: None,
        car_performance_base: 5.0,
        budget_base: 46.0,
        reputacao_base: 42.0,
    },
    TeamTemplate {
        nome: "Munich Speed Works",
        nome_curto: "MSW",
        categoria: "bmw_m2",
        cor_primaria: "#ff6b00",
        cor_secundaria: "#ff6b00",
        pais_sede: "🇩🇪 Alemanha",
        marca: None,
        classe: None,
        car_performance_base: 7.5,
        budget_base: 60.0,
        reputacao_base: 56.0,
    },
    TeamTemplate {
        nome: "Isar Track",
        nome_curto: "IST",
        categoria: "bmw_m2",
        cor_primaria: "#006b3c",
        cor_secundaria: "#006b3c",
        pais_sede: "🇩🇪 Alemanha",
        marca: None,
        classe: None,
        car_performance_base: 3.0,
        budget_base: 38.0,
        reputacao_base: 34.0,
    },
    TeamTemplate {
        nome: "Eifel Sprint",
        nome_curto: "EFS",
        categoria: "bmw_m2",
        cor_primaria: "#6a00f4",
        cor_secundaria: "#6a00f4",
        pais_sede: "🇫🇷 França",
        marca: None,
        classe: None,
        car_performance_base: 4.5,
        budget_base: 44.0,
        reputacao_base: 40.0,
    },
    TeamTemplate {
        nome: "Corporate Express",
        nome_curto: "CEX",
        categoria: "bmw_m2",
        cor_primaria: "#5c677d",
        cor_secundaria: "#5c677d",
        pais_sede: "🇨🇭 Suíça",
        marca: None,
        classe: None,
        car_performance_base: 2.5,
        budget_base: 36.0,
        reputacao_base: 32.0,
    },
    TeamTemplate {
        nome: "Roundel",
        nome_curto: "RND",
        categoria: "bmw_m2",
        cor_primaria: "#b08900",
        cor_secundaria: "#b08900",
        pais_sede: "🇬🇧 Reino Unido",
        marca: None,
        classe: None,
        car_performance_base: 6.5,
        budget_base: 55.0,
        reputacao_base: 51.0,
    },
    TeamTemplate {
        nome: "Southern Cross",
        nome_curto: "SCX",
        categoria: "bmw_m2",
        cor_primaria: "#c1121f",
        cor_secundaria: "#c1121f",
        pais_sede: "🇦🇺 Austrália",
        marca: None,
        classe: None,
        car_performance_base: 4.0,
        budget_base: 42.0,
        reputacao_base: 38.0,
    },
    TeamTemplate {
        nome: "Black Forest Works",
        nome_curto: "BFW",
        categoria: "bmw_m2",
        cor_primaria: "#8b5e34",
        cor_secundaria: "#8b5e34",
        pais_sede: "🇩🇪 Alemanha",
        marca: None,
        classe: None,
        car_performance_base: 3.5,
        budget_base: 40.0,
        reputacao_base: 36.0,
    },
    TeamTemplate {
        nome: "Rahal Letterman Racing",
        nome_curto: "RLR",
        categoria: "gt4",
        cor_primaria: "#0057b8",
        cor_secundaria: "#0057b8",
        pais_sede: "🇺🇸 EUA",
        marca: Some("BMW"),
        classe: None,
        car_performance_base: 8.0,
        budget_base: 62.0,
        reputacao_base: 57.0,
    },
    TeamTemplate {
        nome: "Toksport World Racing",
        nome_curto: "TKW",
        categoria: "gt4",
        cor_primaria: "#a4161a",
        cor_secundaria: "#a4161a",
        pais_sede: "🇩🇪 Alemanha",
        marca: Some("BMW"),
        classe: None,
        car_performance_base: 5.0,
        budget_base: 50.0,
        reputacao_base: 46.0,
    },
    TeamTemplate {
        nome: "Stuttgart Racing Academy",
        nome_curto: "SRA",
        categoria: "gt4",
        cor_primaria: "#111111",
        cor_secundaria: "#111111",
        pais_sede: "🇬🇧 Reino Unido",
        marca: Some("Porsche"),
        classe: None,
        car_performance_base: 9.0,
        budget_base: 67.0,
        reputacao_base: 62.0,
    },
    TeamTemplate {
        nome: "Grove Drive Racing",
        nome_curto: "GVR",
        categoria: "gt4",
        cor_primaria: "#ff7a00",
        cor_secundaria: "#ff7a00",
        pais_sede: "🇫🇷 França",
        marca: Some("Porsche"),
        classe: None,
        car_performance_base: 6.0,
        budget_base: 53.0,
        reputacao_base: 49.0,
    },
    TeamTemplate {
        nome: "Formosa Corsa",
        nome_curto: "FMC",
        categoria: "gt4",
        cor_primaria: "#c9a227",
        cor_secundaria: "#c9a227",
        pais_sede: "🇹🇼 Taiwan",
        marca: Some("Mercedes-AMG"),
        classe: None,
        car_performance_base: 7.0,
        budget_base: 58.0,
        reputacao_base: 53.0,
    },
    TeamTemplate {
        nome: "Silver Peak Performance",
        nome_curto: "SPP",
        categoria: "gt4",
        cor_primaria: "#bfc0c0",
        cor_secundaria: "#bfc0c0",
        pais_sede: "🇩🇪 Alemanha",
        marca: Some("Mercedes-AMG"),
        classe: None,
        car_performance_base: 4.0,
        budget_base: 44.0,
        reputacao_base: 40.0,
    },
    TeamTemplate {
        nome: "Heart of Racing",
        nome_curto: "HRT",
        categoria: "gt4",
        cor_primaria: "#006f52",
        cor_secundaria: "#006f52",
        pais_sede: "🇺🇸 EUA",
        marca: Some("Aston Martin"),
        classe: None,
        car_performance_base: 9.5,
        budget_base: 69.0,
        reputacao_base: 64.0,
    },
    TeamTemplate {
        nome: "North Sea Motorsport",
        nome_curto: "NSM",
        categoria: "gt4",
        cor_primaria: "#ff5500",
        cor_secundaria: "#ff5500",
        pais_sede: "🇬🇧 Reino Unido",
        marca: Some("McLaren"),
        classe: None,
        car_performance_base: 7.5,
        budget_base: 61.0,
        reputacao_base: 56.0,
    },
    TeamTemplate {
        nome: "Aures Racing",
        nome_curto: "ARS",
        categoria: "gt4",
        cor_primaria: "#00a6a6",
        cor_secundaria: "#00a6a6",
        pais_sede: "🇫🇷 França",
        marca: Some("McLaren"),
        classe: None,
        car_performance_base: 3.5,
        budget_base: 42.0,
        reputacao_base: 38.0,
    },
    TeamTemplate {
        nome: "Aichi Works",
        nome_curto: "AWK",
        categoria: "gt4",
        cor_primaria: "#e4002b",
        cor_secundaria: "#e4002b",
        pais_sede: "🇬🇧 Reino Unido",
        marca: Some("Toyota"),
        classe: None,
        car_performance_base: 6.0,
        budget_base: 54.0,
        reputacao_base: 50.0,
    },
    TeamTemplate {
        nome: "Mercedes-AMG",
        nome_curto: "AMG",
        categoria: "gt3",
        cor_primaria: "#00d2be",
        cor_secundaria: "#00d2be",
        pais_sede: "🇺🇸 EUA",
        marca: Some("Mercedes-AMG"),
        classe: None,
        car_performance_base: 15.0,
        budget_base: 83.0,
        reputacao_base: 87.0,
    },
    TeamTemplate {
        nome: "Lamborghini",
        nome_curto: "LAM",
        categoria: "gt3",
        cor_primaria: "#ffd100",
        cor_secundaria: "#ffd100",
        pais_sede: "🇺🇸 EUA",
        marca: Some("Lamborghini"),
        classe: None,
        car_performance_base: 14.0,
        budget_base: 80.0,
        reputacao_base: 84.0,
    },
    TeamTemplate {
        nome: "Porsche",
        nome_curto: "POR",
        categoria: "gt3",
        cor_primaria: "#111111",
        cor_secundaria: "#111111",
        pais_sede: "🇺🇸 EUA",
        marca: Some("Porsche"),
        classe: None,
        car_performance_base: 13.0,
        budget_base: 77.0,
        reputacao_base: 81.0,
    },
    TeamTemplate {
        nome: "Aston Martin",
        nome_curto: "AST",
        categoria: "gt3",
        cor_primaria: "#005f48",
        cor_secundaria: "#005f48",
        pais_sede: "🇺🇸 EUA",
        marca: Some("Aston Martin"),
        classe: None,
        car_performance_base: 11.0,
        budget_base: 72.0,
        reputacao_base: 75.0,
    },
    TeamTemplate {
        nome: "McLaren",
        nome_curto: "MCL",
        categoria: "gt3",
        cor_primaria: "#ff8700",
        cor_secundaria: "#ff8700",
        pais_sede: "🇺🇸 EUA",
        marca: Some("McLaren"),
        classe: None,
        car_performance_base: 10.5,
        budget_base: 70.0,
        reputacao_base: 73.0,
    },
    TeamTemplate {
        nome: "Audi",
        nome_curto: "AUD",
        categoria: "gt3",
        cor_primaria: "#bb0a21",
        cor_secundaria: "#bb0a21",
        pais_sede: "Alemanha",
        marca: Some("Audi"),
        classe: None,
        car_performance_base: 9.0,
        budget_base: 65.0,
        reputacao_base: 68.0,
    },
    TeamTemplate {
        nome: "BMW",
        nome_curto: "BMW",
        categoria: "gt3",
        cor_primaria: "#0057b8",
        cor_secundaria: "#0057b8",
        pais_sede: "🇺🇸 EUA",
        marca: Some("BMW"),
        classe: None,
        car_performance_base: 8.5,
        budget_base: 63.0,
        reputacao_base: 65.0,
    },
    TeamTemplate {
        nome: "Ferrari",
        nome_curto: "FER",
        categoria: "gt3",
        cor_primaria: "#dc0000",
        cor_secundaria: "#dc0000",
        pais_sede: "🇮🇹 Itália",
        marca: Some("Ferrari"),
        classe: None,
        car_performance_base: 6.5,
        budget_base: 55.0,
        reputacao_base: 57.0,
    },
    TeamTemplate {
        nome: "Chevrolet",
        nome_curto: "CHE",
        categoria: "gt3",
        cor_primaria: "#f9c80e",
        cor_secundaria: "#f9c80e",
        pais_sede: "EUA",
        marca: Some("Chevrolet"),
        classe: None,
        car_performance_base: 5.5,
        budget_base: 50.0,
        reputacao_base: 52.0,
    },
    TeamTemplate {
        nome: "Ford Mustang",
        nome_curto: "FRD",
        categoria: "gt3",
        cor_primaria: "#004b3a",
        cor_secundaria: "#004b3a",
        pais_sede: "EUA",
        marca: Some("Ford Mustang"),
        classe: None,
        car_performance_base: 4.5,
        budget_base: 46.0,
        reputacao_base: 48.0,
    },
    TeamTemplate {
        nome: "Acura",
        nome_curto: "ACR",
        categoria: "gt3",
        cor_primaria: "#c8102e",
        cor_secundaria: "#c8102e",
        pais_sede: "EUA",
        marca: Some("Acura"),
        classe: None,
        car_performance_base: 3.5,
        budget_base: 44.0,
        reputacao_base: 46.0,
    },
    TeamTemplate {
        nome: "Obsidian",
        nome_curto: "OBS",
        categoria: "gt3",
        cor_primaria: "#2b2d42",
        cor_secundaria: "#2b2d42",
        pais_sede: "Canada",
        marca: None,
        classe: None,
        car_performance_base: 2.0,
        budget_base: 39.0,
        reputacao_base: 36.0,
    },
    TeamTemplate {
        nome: "Kitsune",
        nome_curto: "KIT",
        categoria: "gt3",
        cor_primaria: "#3a0ca3",
        cor_secundaria: "#3a0ca3",
        pais_sede: "Japao",
        marca: None,
        classe: None,
        car_performance_base: 1.0,
        budget_base: 36.0,
        reputacao_base: 33.0,
    },
    TeamTemplate {
        nome: "Valkyrie",
        nome_curto: "VAL",
        categoria: "gt3",
        cor_primaria: "#6f4e37",
        cor_secundaria: "#6f4e37",
        pais_sede: "Noruega",
        marca: None,
        classe: None,
        car_performance_base: -1.0,
        budget_base: 35.0,
        reputacao_base: 31.0,
    },
    TeamTemplate {
        nome: "United Autosports",
        nome_curto: "UNT",
        categoria: "endurance",
        cor_primaria: "#1f4fff",
        cor_secundaria: "#1f4fff",
        pais_sede: "🇬🇧 Reino Unido",
        marca: None,
        classe: Some("lmp2"),
        car_performance_base: 16.0,
        budget_base: 90.0,
        reputacao_base: 88.0,
    },
    TeamTemplate {
        nome: "Jota Sport",
        nome_curto: "JOT",
        categoria: "endurance",
        cor_primaria: "#f5c400",
        cor_secundaria: "#f5c400",
        pais_sede: "🇬🇧 Reino Unido",
        marca: None,
        classe: Some("lmp2"),
        car_performance_base: 15.0,
        budget_base: 87.0,
        reputacao_base: 85.0,
    },
    TeamTemplate {
        nome: "Belgian Racing Team",
        nome_curto: "BRT",
        categoria: "endurance",
        cor_primaria: "#0b3d91",
        cor_secundaria: "#0b3d91",
        pais_sede: "🇧🇪 Bélgica",
        marca: None,
        classe: Some("lmp2"),
        car_performance_base: 14.5,
        budget_base: 85.0,
        reputacao_base: 83.0,
    },
    TeamTemplate {
        nome: "Prema Powerteam",
        nome_curto: "PRM",
        categoria: "endurance",
        cor_primaria: "#ff2e2e",
        cor_secundaria: "#ff2e2e",
        pais_sede: "🇮🇹 Itália",
        marca: None,
        classe: Some("lmp2"),
        car_performance_base: 14.0,
        budget_base: 84.0,
        reputacao_base: 82.0,
    },
    TeamTemplate {
        nome: "Cool Racing",
        nome_curto: "CRL",
        categoria: "endurance",
        cor_primaria: "#00bfff",
        cor_secundaria: "#00bfff",
        pais_sede: "🇨🇭 Suíça",
        marca: None,
        classe: Some("lmp2"),
        car_performance_base: 13.5,
        budget_base: 82.0,
        reputacao_base: 80.0,
    },
];

pub fn get_team_templates(category_id: &str) -> Vec<&'static TeamTemplate> {
    TEAMS
        .iter()
        .filter(|team| team.categoria == category_id)
        .collect()
}

pub fn get_teams_for_category(category_id: &str) -> Vec<&'static TeamTemplate> {
    get_team_templates(category_id)
}

pub fn get_reference_team_template(
    category_id: &str,
    class_name: Option<&str>,
) -> Option<&'static TeamTemplate> {
    if let Some(template) = get_team_templates(category_id)
        .into_iter()
        .find(|team| class_name.is_none() || team.classe == class_name)
    {
        return Some(template);
    }

    let reference_category = match (category_id, class_name) {
        ("production_challenger", Some("mazda")) => Some("mazda_amador"),
        ("production_challenger", Some("toyota")) => Some("toyota_amador"),
        ("production_challenger", Some("bmw")) => Some("bmw_m2"),
        ("endurance", Some("gt4")) => Some("gt4"),
        ("endurance", Some("gt3")) => Some("gt3"),
        _ => None,
    }?;

    get_team_templates(reference_category).into_iter().next()
}

pub fn get_all_team_templates() -> &'static [TeamTemplate] {
    TEAMS
}

pub fn count_teams() -> usize {
    TEAMS.len()
}

pub fn get_teams_by_endurance_class(classe: &str) -> Vec<&'static TeamTemplate> {
    TEAMS
        .iter()
        .filter(|team| team.categoria == "endurance" && team.classe == Some(classe))
        .collect()
}

pub fn get_teams_by_brand(marca: &str) -> Vec<&'static TeamTemplate> {
    TEAMS
        .iter()
        .filter(|team| team.marca == Some(marca))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_team_templates_gt3_count() {
        assert_eq!(get_team_templates("gt3").len(), 14);
    }

    #[test]
    fn test_gt3_contains_required_manufacturers() {
        let brands: std::collections::HashSet<_> = get_team_templates("gt3")
            .into_iter()
            .filter_map(|team| team.marca)
            .collect();

        for brand in [
            "Ferrari",
            "Porsche",
            "Ford Mustang",
            "Chevrolet",
            "BMW",
            "Mercedes-AMG",
            "Lamborghini",
            "McLaren",
            "Acura",
            "Aston Martin",
            "Audi",
        ] {
            assert!(
                brands.contains(brand),
                "GT3 sem fabricante obrigatorio: {brand}"
            );
        }
    }

    #[test]
    fn test_gt3_manufacturers_are_not_repeated_and_exotics_are_weakest() {
        let teams = get_team_templates("gt3");
        let mut brands = std::collections::HashSet::new();
        let exotic_teams: Vec<_> = teams
            .iter()
            .copied()
            .filter(|team| team.marca.is_none())
            .collect();

        for team in teams.iter().copied().filter(|team| team.marca.is_some()) {
            let brand = team.marca.expect("brand");
            assert!(brands.insert(brand), "fabricante GT3 repetido: {brand}");
        }

        assert_eq!(brands.len(), 11);
        assert_eq!(exotic_teams.len(), 3);
        assert!(exotic_teams
            .iter()
            .all(|team| team.car_performance_base <= 2.0));
    }

    #[test]
    fn test_gt3_factory_colors_match_required_brands() {
        let teams = get_team_templates("gt3");
        let colors_for = |brand| {
            teams
                .iter()
                .find(|team| team.marca == Some(brand))
                .map(|team| (team.cor_primaria, team.cor_secundaria))
                .expect("GT3 brand should exist")
        };

        assert_eq!(colors_for("Ferrari"), ("#dc0000", "#dc0000"));
        assert_eq!(colors_for("Mercedes-AMG"), ("#00d2be", "#00d2be"));
        assert_eq!(colors_for("Lamborghini"), ("#ffd100", "#ffd100"));
        assert_eq!(colors_for("McLaren"), ("#ff8700", "#ff8700"));
        assert_eq!(colors_for("Porsche"), ("#111111", "#111111"));
    }

    #[test]
    fn test_gt3_team_names_are_simple() {
        let names: Vec<_> = get_team_templates("gt3")
            .into_iter()
            .map(|team| team.nome)
            .collect();

        assert_eq!(
            names,
            vec![
                "Mercedes-AMG",
                "Lamborghini",
                "Porsche",
                "Aston Martin",
                "McLaren",
                "Audi",
                "BMW",
                "Ferrari",
                "Chevrolet",
                "Ford Mustang",
                "Acura",
                "Obsidian",
                "Kitsune",
                "Valkyrie",
            ]
        );
    }

    #[test]
    fn test_bmw_m2_palette_has_one_unique_color_per_team() {
        let teams = get_team_templates("bmw_m2");
        let mut colors = std::collections::HashSet::new();

        for team in teams {
            assert_eq!(
                team.cor_primaria, team.cor_secundaria,
                "BMW M2 com primaria diferente da secundaria: {}",
                team.nome
            );
            assert!(
                colors.insert(team.cor_primaria),
                "cor primaria BMW M2 repetida: {} ({})",
                team.cor_primaria,
                team.nome
            );
        }

        assert_eq!(colors.len(), 10);
    }

    #[test]
    fn test_bmw_m2_team_names_are_distinct() {
        let names: Vec<_> = get_team_templates("bmw_m2")
            .into_iter()
            .map(|team| team.nome)
            .collect();

        assert_eq!(
            names,
            vec![
                "Bayern Division",
                "M Power",
                "Blue Propeller",
                "Munich Speed Works",
                "Isar Track",
                "Eifel Sprint",
                "Corporate Express",
                "Roundel",
                "Southern Cross",
                "Black Forest Works",
            ]
        );
    }

    #[test]
    fn test_team_templates_production_count() {
        assert_eq!(get_team_templates("production_challenger").len(), 0);
    }

    #[test]
    fn test_team_templates_endurance_only_has_lmp2_templates() {
        let endurance_templates = get_team_templates("endurance");
        assert_eq!(endurance_templates.len(), 5);
        assert!(endurance_templates
            .iter()
            .all(|team| team.classe == Some("lmp2")));
    }

    #[test]
    fn test_special_class_reference_templates_use_regular_feeders() {
        assert_eq!(
            get_reference_team_template("production_challenger", Some("mazda"))
                .map(|team| team.categoria),
            Some("mazda_amador")
        );
        assert_eq!(
            get_reference_team_template("endurance", Some("gt4")).map(|team| team.categoria),
            Some("gt4")
        );
        assert_eq!(
            get_reference_team_template("endurance", Some("lmp2")).map(|team| team.classe),
            Some(Some("lmp2"))
        );
    }
}
