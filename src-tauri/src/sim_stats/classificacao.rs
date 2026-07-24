//! Parâmetros de classificação do harness: os atributos que contam como
//! "evolução", os limiares de estagnação e os rótulos de faixa etária, tier e
//! banda de habilidade usados no relatório.

// ── Parâmetros de classificação ───────────────────────────────────────────────

/// 10 atributos "treináveis" — média deles é o nosso indicador de evolução.
pub(super) const CORE_ATTRS: &[&str] = &[
    "skill",
    "consistencia",
    "racecraft",
    "defesa",
    "ritmo_classificacao",
    "gestao_pneus",
    "adaptabilidade",
    "mentalidade",
    "confianca",
    "smoothness",
];

/// Margem (em pontos de atributo médio) abaixo da qual consideramos "estagnou"
/// numa única temporada (mudanças são graduais).
pub(super) const STAGNATION_THRESHOLD: f64 = 0.5;

/// Margem para a trajetória de CARREIRA inteira (primeiro vs último ano).
pub(super) const CAREER_THRESHOLD: f64 = 1.0;

pub(super) fn age_bucket(age: i32) -> &'static str {
    match age {
        i32::MIN..=20 => "<=20",
        21..=24 => "21-24",
        25..=28 => "25-28",
        29..=32 => "29-32",
        33..=36 => "33-36",
        37..=39 => "37-39",
        40..=42 => "40-42",
        _ => "43+",
    }
}

/// Ranking de estados financeiros (0 = pior). Usado para detectar recuperação.
pub(super) fn state_rank(state: &str) -> u8 {
    match state {
        "collapse" => 0,
        "crisis" => 1,
        "pressured" => 2,
        "stable" => 3,
        "healthy" => 4,
        "elite" => 5,
        _ => 3, // desconhecido tratado como neutro
    }
}

pub(super) fn tier_of(cat: &str) -> u8 {
    crate::constants::categories::get_category_config(cat)
        .map(|c| c.tier)
        .unwrap_or(99)
}

/// Rótulo do tier (nível) para o relatório de funil de carreira.
pub(super) fn tier_label(tier: u8) -> &'static str {
    match tier {
        0 => "Rookie",
        1 => "Amador",
        2 => "Pro",
        3 => "Super Pro",
        4 => "Master",
        5 => "Elite (LMP2)",
        6 => "Endurance",
        _ => "?",
    }
}

/// Banda pelo PICO de habilidade na carreira (o quão bom o piloto chegou a ser).
pub(super) fn skill_band_of(overall: f64) -> &'static str {
    if overall >= 65.0 {
        "Elite (65+)"
    } else if overall >= 57.0 {
        "Bom (57-65)"
    } else {
        "Comum (<57)"
    }
}
