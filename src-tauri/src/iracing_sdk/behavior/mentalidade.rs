//! Ganho e compostura pela **mentalidade** — a espinha dorsal do modelo — mais o
//! gerador determinístico que alimenta o sorteio do dia.

// --- Ganho pela mentalidade (espinha dorsal) ------------------------------------
const MALL_MIN: f64 = 0.6; // mental forte (100) → sinais amortecidos
const MALL_MAX: f64 = 1.4; // mental fraco (0) → sinais amplificados
/// O quanto o mais forte mentalmente consegue blindar do adverso no melhor dia —
/// nunca 100% (ninguém é imune). Compostura efetiva = mentalidade × isto.
const MAX_COMPOSURE: f64 = 0.75;

/// Quanto os sinais conseguem deformar o piloto (0.6 forte … 1.4 fraco).
pub fn malleability(mentality: f64) -> f64 {
    MALL_MAX - mentality.clamp(0.0, 100.0) / 100.0 * (MALL_MAX - MALL_MIN)
}

/// Fração do impacto ADVERSO que o piloto leva nesta corrida (0 = blindou tudo,
/// 1 = levou cheio). GRANULAR: a mentalidade puxa a média pra baixo e um sorteio do
/// dia dá a variação. Mental 0 → sempre 1.0; quanto mais forte, menor e mais variável.
pub fn adverse_multiplier(mentality: f64, seed: u64) -> f64 {
    let composure = mentality.clamp(0.0, 100.0) / 100.0 * MAX_COMPOSURE;
    (1.0 - composure * composure_roll(seed)).clamp(0.0, 1.0)
}

pub(super) fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Sorteio determinístico [0,1) do seed (descorrelacionado do wobble).
fn composure_roll(seed: u64) -> f64 {
    (splitmix(seed ^ 0xA5A5_A5A5_A5A5_A5A5) >> 11) as f64 / (1u64 << 53) as f64
}
