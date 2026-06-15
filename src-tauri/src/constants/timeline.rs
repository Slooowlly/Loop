#![allow(dead_code)]

/// Semana do ano civil (week_of_year) em que o mercado de pré-temporada abre (dezembro).
pub const MARKET_START_WEEK_OF_YEAR: u8 = 49;

/// Duração total do mercado em semanas: 49, 50, 51, 52, 1, 2, 3, 4, 5 → 9 semanas.
pub const MARKET_DURATION_WEEKS: u8 = 9;

/// Primeira semana de corrida no ano civil (início de fevereiro).
pub const SEASON_FIRST_RACE_WEEK_OF_YEAR: u8 = 6;

/// Última semana de corrida no ano civil (fim de novembro).
pub const SEASON_LAST_RACE_WEEK_OF_YEAR: u8 = 47;

/// Primeira semana de corrida na régua season_week (season_week 10 = week_of_year 6).
pub const SEASON_FIRST_RACE_SEASON_WEEK: u8 = 10;

/// Total de semanas da temporada na régua season_week (1–51).
/// season_week 1 = abertura do mercado (woy 49); season_week 51 = última corrida (woy 47).
pub const SEASON_TOTAL_WEEKS: u8 = 51;
