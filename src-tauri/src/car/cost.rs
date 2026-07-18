//! Curva de custo de peça por categoria, com o **teto suave**.
//!
//! ```text
//! custo(cat, peça, nível) = base_peça(cat) · ∏_{k=2}^{nível} step(k)
//! step(k) = 1 + 23,85%                         se k ≤ teto_cat
//!         = 1 + 23,85% + 35%·(k − teto_cat)    se k >  teto_cat   (a "parede")
//! ```
//!
//! Abaixo do teto o custo cresce de forma geométrica mansa (+23,85%/nível). Acima,
//! cada nível fica mais caro que o anterior (parede que se ergue), sem cap rígido.
//! A escala absoluta por categoria é placeholder calibrável (chunk 8). Ver design em
//! `docs/superpowers/specs/2026-07-17-car-level-system-design.md`.
#![allow(dead_code)] // Chunk 1: curva pura; consumida por finanças/cérebro do time nos chunks 3+.

use crate::car::parts::PartType;

/// Crescimento geométrico base por nível (abaixo do teto): +23,85%.
pub const BASE_GROWTH: f64 = 0.2385;
/// Íngreme extra por nível acima do teto — a "parede" que compõe.
pub const WALL_EXTRA_PER_LEVEL: f64 = 0.35;

/// Base da categoria, ignorando a classe (ex.: `"endurance:gt3"` → `"endurance"`).
fn category_base(category_id: &str) -> &str {
    category_id.split(':').next().unwrap_or(category_id)
}

/// Teto suave de nível por categoria: o nível "natural" da categoria; acima dele o
/// custo dispara. Não é cap rígido — é onde a parede começa.
pub fn category_ceiling(category_id: &str) -> u8 {
    match category_base(category_id) {
        "mazda_rookie" | "toyota_rookie" => 1,
        "mazda_amador" | "toyota_amador" => 2,
        "bmw_m2" => 3,
        "production_challenger" => 4,
        "gt4" => 6,
        "gt3" => 7,
        "lmp2" | "endurance" => 8,
        _ => 4,
    }
}

/// Escala de custo por categoria. PRIMEIRA-PASSADA ancorada na economia de cada categoria
/// (`operating_cost_midpoint × ~0,00065`), pra que a manutenção recorrente fique numa
/// fração sustentável do orçamento (design §6) e o acoplamento orçamento↔custo funcione.
/// O Monte Carlo do chunk 8 refina esses números. Mantém as proporções relativas entre
/// peças (o custo relativo e a parede não mudam).
fn category_cost_scale(category_id: &str) -> f64 {
    match category_base(category_id) {
        "mazda_rookie" | "toyota_rookie" => 120.0,
        "mazda_amador" | "toyota_amador" => 280.0,
        "bmw_m2" | "production_challenger" => 715.0,
        "gt4" => 1_800.0,
        "gt3" => 5_200.0,
        "lmp2" => 8_800.0,
        "endurance" => 10_700.0,
        _ => 715.0,
    }
}

/// Custo da peça no nível 1 para a categoria.
fn part_base_cost(category_id: &str, part: PartType) -> f64 {
    part.relative_cost() * category_cost_scale(category_id)
}

/// Custo de uma peça no `level` dado, aplicando o teto suave da categoria.
pub fn part_cost(category_id: &str, part: PartType, level: u8) -> f64 {
    let ceiling = category_ceiling(category_id);
    let mut cost = part_base_cost(category_id, part);
    for k in 2..=level {
        let step = if k <= ceiling {
            1.0 + BASE_GROWTH
        } else {
            1.0 + BASE_GROWTH + WALL_EXTRA_PER_LEVEL * (k - ceiling) as f64
        };
        cost *= step;
    }
    cost
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tetos_batem_com_o_design() {
        assert_eq!(category_ceiling("mazda_rookie"), 1);
        assert_eq!(category_ceiling("mazda_amador"), 2);
        assert_eq!(category_ceiling("bmw_m2"), 3);
        assert_eq!(category_ceiling("production_challenger"), 4);
        assert_eq!(category_ceiling("gt4"), 6);
        assert_eq!(category_ceiling("gt3"), 7);
        assert_eq!(category_ceiling("endurance"), 8);
        // split por classe
        assert_eq!(category_ceiling("endurance:gt3"), 8);
    }

    #[test]
    fn abaixo_do_teto_cresce_23_85_por_cento() {
        // gt3 teto 7: L1→L2 está abaixo do teto.
        let c1 = part_cost("gt3", PartType::Engine, 1);
        let c2 = part_cost("gt3", PartType::Engine, 2);
        assert!(((c2 / c1) - 1.2385).abs() < 1e-6, "ratio={}", c2 / c1);
    }

    #[test]
    fn primeiro_nivel_acima_do_teto_e_59_por_cento() {
        // amador teto 2: L2→L3 é o primeiro acima → +23,85% +35% = +58,85%.
        let c2 = part_cost("mazda_amador", PartType::Engine, 2);
        let c3 = part_cost("mazda_amador", PartType::Engine, 3);
        assert!(((c3 / c2) - 1.5885).abs() < 1e-6, "ratio={}", c3 / c2);
    }

    #[test]
    fn a_parede_fica_mais_ingreme_a_cada_nivel() {
        // L3→L4 (2 acima do teto) → +23,85% +70% = +93,85%.
        let c3 = part_cost("mazda_amador", PartType::Engine, 3);
        let c4 = part_cost("mazda_amador", PartType::Engine, 4);
        assert!(((c4 / c3) - 1.9385).abs() < 1e-6, "ratio={}", c4 / c3);
    }

    #[test]
    fn nivel_5_dói_muito_mais_em_categoria_de_teto_baixo() {
        // Razão ao próprio L1 remove a escala de categoria → compara só a parede.
        let amador = part_cost("mazda_amador", PartType::Engine, 5)
            / part_cost("mazda_amador", PartType::Engine, 1);
        let gt3 =
            part_cost("gt3", PartType::Engine, 5) / part_cost("gt3", PartType::Engine, 1);
        assert!(
            amador > gt3 * 2.0,
            "L5 no amador ({amador:.2}×) deveria eclipsar o gt3 ({gt3:.2}×)"
        );
    }
}
