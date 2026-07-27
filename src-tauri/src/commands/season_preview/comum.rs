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

/// **Percepção pública** de um piloto: o quanto o grid/a imprensa o cotam ANTES de a
/// temporada começar. É o que ordena os favoritos da matéria "O Que Esperar" — e, por
/// isso, também a expectativa que a torre do overlay usa pra ordenar a classificatória
/// da 1ª etapa, quando ainda não existe tempo nem campeonato.
///
/// Vale o que é PÚBLICO (títulos, vitórias, pódios, fama, carisma, rodagem); o skill
/// entra só como um vazamento fraco, porque ninguém o enxerga direto. O jitter impede
/// que a ordem vire um espelho exato do skill num grid sem resultado nenhum.
pub(crate) fn perception_score(d: &Driver) -> f64 {
    let c = &d.stats_carreira;
    W_TITLE * c.titulos as f64
        + W_WIN * c.vitorias as f64
        + W_PODIUM * c.podios as f64
        + W_FAME * d.atributos.midia
        + W_CHARISMA * d.atributos.carisma
        + W_EXPERIENCE * (c.corridas as f64).min(EXPERIENCE_CAP)
        + W_SKILL_HINT * d.atributos.skill
        + jitter(&d.id)
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
