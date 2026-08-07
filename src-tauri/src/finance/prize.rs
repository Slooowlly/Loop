//! Premiação de fim de temporada por posição no campeonato de construtores.
//!
//! Até a Fase de balanceamento financeiro, equipes só recebiam prêmios por
//! corrida (patrocínio + bônus de pontos), o que deixava a posição final no
//! campeonato sem valor financeiro e o grid inteiro em déficit estrutural.
//!
//! Este módulo adiciona um prêmio único, pago no encerramento da temporada,
//! escalonado linearmente pela posição final: o último colocado recebe
//! `BASE × O`, o campeão recebe `(BASE + SLOPE) × O`, onde `O` é o custo
//! operacional médio da categoria. A média por equipe ≈ `(BASE + SLOPE/2) × O`,
//! calibrada para deixar o meio de grid perto do equilíbrio (junto com o
//! patrocínio por corrida), com o topo lucrando e o fundo no vermelho.

use crate::finance::planning::category_finance_scale;

/// Fração do custo operacional médio paga ao ÚLTIMO colocado.
pub(crate) const PRIZE_BASE_FACTOR: f64 = 0.15;
/// Fração adicional concedida linearmente até o 1º colocado.
pub(crate) const PRIZE_SLOPE_FACTOR: f64 = 0.50;

/// Prêmio de construtores (em dinheiro do jogo) para uma equipe que terminou
/// em `position` (1 = campeão) num grupo de campeonato com `grid_size` equipes.
///
/// Escala pela categoria: categorias mais caras pagam prêmios maiores, na mesma
/// proporção em que seus custos são maiores.
pub fn constructor_prize(category: &str, position: i32, grid_size: i32) -> f64 {
    constructor_prize_with(
        category,
        position,
        grid_size,
        PRIZE_BASE_FACTOR,
        PRIZE_SLOPE_FACTOR,
    )
}

/// Igual a [`constructor_prize`], com os dois fatores explícitos. Existe para o harness de
/// calibração (`commands::race::tests::medicao_financeira`) varrer o peso do prêmio de
/// fechamento sem recompilar — a produção usa as constantes acima. Mesmo padrão de
/// [`crate::car::crash::apply_contact_wear_with`].
pub fn constructor_prize_with(
    category: &str,
    position: i32,
    grid_size: i32,
    base_factor: f64,
    slope_factor: f64,
) -> f64 {
    if grid_size <= 0 || position <= 0 || position > grid_size {
        return 0.0;
    }

    let operating = category_finance_scale(category).operating_cost_midpoint();

    // frac = 1.0 para o 1º colocado, 0.0 para o último.
    let frac = if grid_size == 1 {
        1.0
    } else {
        (grid_size - position) as f64 / (grid_size - 1) as f64
    };

    operating * (base_factor + slope_factor * frac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn champion_earns_more_than_last_place() {
        let champ = constructor_prize("gt3", 1, 14);
        let last = constructor_prize("gt3", 14, 14);
        assert!(champ > last);
        assert!(last > 0.0, "até o último colocado recebe algo");
    }

    #[test]
    fn prize_scales_with_category_cost() {
        // Mesma posição relativa, categoria mais cara paga mais.
        let gt3_champ = constructor_prize("gt3", 1, 14);
        let rookie_champ = constructor_prize("mazda_rookie", 1, 14);
        assert!(gt3_champ > rookie_champ);
    }

    #[test]
    fn champion_prize_matches_base_plus_slope() {
        let operating = category_finance_scale("gt3").operating_cost_midpoint();
        let champ = constructor_prize("gt3", 1, 10);
        let expected = operating * (PRIZE_BASE_FACTOR + PRIZE_SLOPE_FACTOR);
        assert!((champ - expected).abs() < 1.0);
    }

    #[test]
    fn last_place_prize_matches_base_only() {
        let operating = category_finance_scale("gt3").operating_cost_midpoint();
        let last = constructor_prize("gt3", 10, 10);
        let expected = operating * PRIZE_BASE_FACTOR;
        assert!((last - expected).abs() < 1.0);
    }

    #[test]
    fn invalid_inputs_return_zero() {
        assert_eq!(constructor_prize("gt3", 0, 14), 0.0);
        assert_eq!(constructor_prize("gt3", 1, 0), 0.0);
        assert_eq!(constructor_prize("gt3", 20, 14), 0.0);
    }

    #[test]
    fn midfield_is_near_average() {
        let operating = category_finance_scale("gt3").operating_cost_midpoint();
        // posição central de um grid de 11 (posição 6) ≈ média
        let mid = constructor_prize("gt3", 6, 11);
        let avg = operating * (PRIZE_BASE_FACTOR + PRIZE_SLOPE_FACTOR / 2.0);
        assert!((mid - avg).abs() < operating * 0.05);
    }
}
