//! Utilitários compartilhados da matéria: ruído determinístico, percentil e i18n.

use super::*;

/// Ruído estável por piloto (hash FNV-1a do id). Determinístico: a mesma carreira gera
/// sempre a mesma percepção, mas ela não é uma função limpa do skill.
pub(super) fn jitter(id: &str) -> f64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in id.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    ((h % 1000) as f64 / 1000.0 - 0.5) * JITTER_RANGE
}

/// Percentil de um valor dentro do grid (0 = pior, 1 = melhor).
pub(super) fn percentile(value: f64, all: &[f64]) -> f64 {
    if all.is_empty() {
        return 0.5;
    }
    let at_or_below = all.iter().filter(|v| **v <= value).count() as f64;
    at_or_below / all.len() as f64
}

pub(super) fn tk(key: &str) -> String {
    // A chave precisa viver enquanto o macro a empresta (não pode ser temporário).
    let full = format!("season_preview.{key}");
    rust_i18n::t!(&full).to_string()
}
