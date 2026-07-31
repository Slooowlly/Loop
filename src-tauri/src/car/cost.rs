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

/// Classe embutida na chave, quando há (ex.: `"endurance:gt3"` → `Some("gt3")`).
fn category_class(category_id: &str) -> Option<&str> {
    category_id
        .split_once(':')
        .map(|(_, classe)| classe)
        .filter(|classe| !classe.is_empty())
}

/// Teto suave de nível por categoria: o nível "natural" da categoria; acima dele o
/// custo dispara. Não é cap rígido — é onde a parede começa.
///
/// Aceita a chave com classe (`"endurance:gt3"`); sem classe, cai no teto da categoria.
pub fn category_ceiling(category_id: &str) -> u8 {
    category_ceiling_for(category_id, category_class(category_id))
}

/// Teto suave com a classe explícita. **Nas categorias multiclasse o teto é o da CLASSE**,
/// não o da categoria: o carro que a equipe leva ao Endurance é o mesmo tipo de carro da
/// classe correspondente. Sem isto, um GT4 do Endurance corria com teto 8 — carro melhor
/// que o de um GT3 do sprint (7) — e o Endurance inteiro herdava o teto do LMP2.
pub fn category_ceiling_for(category_id: &str, class_id: Option<&str>) -> u8 {
    let class_id = class_id.filter(|classe| !classe.is_empty());
    match (category_base(category_id), class_id) {
        ("endurance", Some("gt4")) => 6,
        ("endurance", Some("gt3")) => 7,
        ("endurance", Some("lmp2")) => 8,
        // Endurance sem classe é grid sintético/legado: mantém o teto histórico.
        ("endurance", None) => 8,
        ("mazda_rookie" | "toyota_rookie", _) => 1,
        ("mazda_amador" | "toyota_amador", _) => 2,
        ("bmw_m2", _) => 3,
        ("production_challenger", _) => 4,
        ("gt4", _) => 6,
        ("gt3", _) => 7,
        ("lmp2", _) => 8,
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

// ── Preço de DESENVOLVIMENTO (subir de nível) ─────────────────────────────────

/// Fração do custo operacional de UMA temporada que a escada completa de desenvolvimento
/// — as 11 peças, do nível 1 até o teto da categoria — deve custar.
///
/// Por que existe: [`part_cost`] é o preço de uma UNIDADE da peça, calibrado para que a
/// manutenção recorrente (reposição por desgaste, amassado, batida) caiba no orçamento.
/// Esse mesmo número, usado como preço de UPGRADE, deixava construir o carro inteiro
/// praticamente de graça: na Production o jogo completo no teto custava ~45k contra um
/// caixa típico de ~3M (1,5%), então qualquer equipe recém-promovida igualava o campo em
/// meia temporada e a escada virava catraca. Construir carro é P&D, não reposição — tem
/// preço próprio, e este é o parâmetro que o ancora.
pub const UPGRADE_BUDGET_FRACTION: f64 = 0.5;

/// Custo BASE (sem o preço de P&D) de sair de um carro todo no nível 1 e chegar ao teto da
/// categoria: soma de [`part_cost`] das 11 peças em cada nível de 2 até o teto — exatamente
/// o que o cérebro de manutenção paga, nível a nível, ao longo do caminho.
///
/// Zero nas categorias spec (teto 1): não há o que desenvolver.
pub fn full_build_base_cost(category_id: &str) -> f64 {
    let ceiling = category_ceiling(category_id);
    PartType::ALL
        .iter()
        .map(|&part| {
            (2..=ceiling)
                .map(|level| part_cost(category_id, part, level))
                .sum::<f64>()
        })
        .sum()
}

/// Multiplicador do preço de P&D da categoria — DERIVADO da economia dela, não escrito à
/// mão: é quanto o custo base da escada precisa ser multiplicado para que construir o carro
/// inteiro custe [`UPGRADE_BUDGET_FRACTION`] do custo operacional de uma temporada.
///
/// Nunca desce de `1.0`: desenvolver jamais sai mais barato que repor a mesma peça.
pub fn upgrade_price_multiplier(category_id: &str) -> f64 {
    let base = full_build_base_cost(category_id);
    if base <= 0.0 {
        return 1.0;
    }
    let alvo = UPGRADE_BUDGET_FRACTION
        * crate::finance::planning::category_finance_scale(category_base(category_id))
            .operating_cost_midpoint();
    (alvo / base).max(1.0)
}

/// Custo de DESENVOLVER uma peça até `level` — o preço de subir o carro, distinto do preço
/// de repor a mesma peça no nível em que ela já está ([`part_cost`], que segue valendo para
/// desgaste, esticada e batida).
pub fn upgrade_cost(category_id: &str, part: PartType, level: u8) -> f64 {
    part_cost(category_id, part, level) * upgrade_price_multiplier(category_id)
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
        // Endurance sem classe (grid sintético/legado) mantém o teto histórico.
        assert_eq!(category_ceiling("endurance"), 8);
    }

    #[test]
    fn no_endurance_o_teto_e_o_da_classe_e_nao_o_da_categoria() {
        // O carro que a equipe leva ao Endurance é o da CLASSE. Sem isto o GT4 do
        // Endurance corria com teto 8 — carro melhor que o do GT3 do sprint (7).
        assert_eq!(category_ceiling("endurance:gt4"), category_ceiling("gt4"));
        assert_eq!(category_ceiling("endurance:gt3"), category_ceiling("gt3"));
        assert_eq!(category_ceiling("endurance:lmp2"), category_ceiling("lmp2"));

        // A forma explícita concorda com a chave `categoria:classe`.
        assert_eq!(category_ceiling_for("endurance", Some("gt3")), 7);
        assert_eq!(category_ceiling_for("endurance", None), 8);
        assert_eq!(category_ceiling_for("gt3", None), 7);
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

    /// A âncora do preço de P&D: construir o carro inteiro custa metade do custo
    /// operacional de uma temporada — em TODA categoria com o que desenvolver.
    #[test]
    fn construir_o_carro_inteiro_custa_meia_temporada_de_operacao() {
        for categoria in [
            "mazda_amador",
            "bmw_m2",
            "production_challenger",
            "gt4",
            "gt3",
            "lmp2",
            "endurance",
        ] {
            let escada: f64 = PartType::ALL
                .iter()
                .map(|&part| {
                    (2..=category_ceiling(categoria))
                        .map(|level| upgrade_cost(categoria, part, level))
                        .sum::<f64>()
                })
                .sum();
            let operacao = crate::finance::planning::category_finance_scale(categoria)
                .operating_cost_midpoint();
            let fracao = escada / operacao;
            assert!(
                (fracao - UPGRADE_BUDGET_FRACTION).abs() < 0.02,
                "{categoria}: escada completa custa {fracao:.3} da operação (esperado {UPGRADE_BUDGET_FRACTION})"
            );
        }
    }

    /// Categoria spec não tem escada — o multiplicador não pode explodir nem dividir por zero.
    #[test]
    fn categoria_spec_nao_tem_preco_de_desenvolvimento() {
        assert_eq!(full_build_base_cost("mazda_rookie"), 0.0);
        assert_eq!(upgrade_price_multiplier("mazda_rookie"), 1.0);
    }

    /// Desenvolver nunca sai mais barato que repor: o piso de `1.0` vale em toda categoria.
    #[test]
    fn desenvolver_nunca_e_mais_barato_que_repor() {
        for categoria in [
            "mazda_rookie",
            "mazda_amador",
            "bmw_m2",
            "production_challenger",
            "gt4",
            "gt3",
            "lmp2",
            "endurance",
        ] {
            assert!(
                upgrade_cost(categoria, PartType::Engine, 3)
                    >= part_cost(categoria, PartType::Engine, 3),
                "{categoria}: upgrade saiu mais barato que a reposição"
            );
        }
    }

    /// O buraco que abriu o caso: na Production o jogo completo no teto custava ~1,5% do
    /// caixa típico. Com o preço de P&D tem que doer de verdade.
    #[test]
    fn construir_na_production_pesa_no_caixa_tipico() {
        let escada: f64 = PartType::ALL
            .iter()
            .map(|&part| {
                (2..=category_ceiling("production_challenger"))
                    .map(|level| upgrade_cost("production_challenger", part, level))
                    .sum::<f64>()
            })
            .sum();
        let caixa_tipico = 3_000_000.0;
        assert!(
            escada / caixa_tipico > 0.15,
            "escada da Production custa só {:.1}% de um caixa de 3M",
            escada / caixa_tipico * 100.0
        );
    }

    #[test]
    fn nivel_5_dói_muito_mais_em_categoria_de_teto_baixo() {
        // Razão ao próprio L1 remove a escala de categoria → compara só a parede.
        let amador = part_cost("mazda_amador", PartType::Engine, 5)
            / part_cost("mazda_amador", PartType::Engine, 1);
        let gt3 = part_cost("gt3", PartType::Engine, 5) / part_cost("gt3", PartType::Engine, 1);
        assert!(
            amador > gt3 * 2.0,
            "L5 no amador ({amador:.2}×) deveria eclipsar o gt3 ({gt3:.2}×)"
        );
    }
}
