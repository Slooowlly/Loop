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

use crate::finance::planning::category_finance_scale_for;

/// Fração do custo operacional médio paga ao ÚLTIMO colocado.
pub(crate) const PRIZE_BASE_FACTOR: f64 = 0.15;
/// Fração adicional concedida linearmente até o 1º colocado.
pub(crate) const PRIZE_SLOPE_FACTOR: f64 = 0.50;

/// Prêmio de construtores (em dinheiro do jogo) para uma equipe que terminou
/// em `position` (1 = campeão) num grupo de campeonato com `grid_size` equipes.
///
/// Escala pela DIVISÃO — categoria mais classe: divisões mais caras pagam prêmios maiores,
/// na mesma proporção em que seus custos são maiores.
///
/// # Por que a classe é parâmetro, e não um detalhe
///
/// O campeonato de construtores de um multi-classe é disputado POR CLASSE — é assim que
/// `world::team_archive` numera as posições e que `evolution::pipeline` conta o grid. Um
/// prêmio que lê só `endurance` responde com a âncora da divisão de referência para as três
/// classes: a campeã GT4 recebia o mesmo que a campeã LMP2, num campeonato em que operar um
/// LMP2 custa múltiplas vezes mais. O prêmio deixava de cobrir a operação de quem estava em
/// cima e sobrecobria quem estava embaixo, dentro da mesma corrida.
///
/// `classe: None` numa categoria multi-classe continua resolvendo para a divisão de
/// referência ([`category_finance_scale_for`]) — é a aproximação com perda de sempre, e
/// existe só para as categorias de classe única, onde ela é exata.
pub fn constructor_prize(
    category: &str,
    classe: Option<&str>,
    position: i32,
    grid_size: i32,
) -> f64 {
    constructor_prize_with(
        category,
        classe,
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
    classe: Option<&str>,
    position: i32,
    grid_size: i32,
    base_factor: f64,
    slope_factor: f64,
) -> f64 {
    if grid_size <= 0 || position <= 0 || position > grid_size {
        return 0.0;
    }

    let operating = category_finance_scale_for(category, classe).operating_cost_midpoint();

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

    fn operacional(category: &str, classe: Option<&str>) -> f64 {
        category_finance_scale_for(category, classe).operating_cost_midpoint()
    }

    #[test]
    fn champion_earns_more_than_last_place() {
        let champ = constructor_prize("gt3", None, 1, 14);
        let last = constructor_prize("gt3", None, 14, 14);
        assert!(champ > last);
        assert!(last > 0.0, "até o último colocado recebe algo");
    }

    #[test]
    fn prize_scales_with_category_cost() {
        // Mesma posição relativa, categoria mais cara paga mais.
        let gt3_champ = constructor_prize("gt3", None, 1, 14);
        let rookie_champ = constructor_prize("mazda_rookie", None, 1, 14);
        assert!(gt3_champ > rookie_champ);
    }

    #[test]
    fn champion_prize_matches_base_plus_slope() {
        let operating = operacional("gt3", None);
        let champ = constructor_prize("gt3", None, 1, 10);
        let expected = operating * (PRIZE_BASE_FACTOR + PRIZE_SLOPE_FACTOR);
        assert!((champ - expected).abs() < 1.0);
    }

    #[test]
    fn last_place_prize_matches_base_only() {
        let operating = operacional("gt3", None);
        let last = constructor_prize("gt3", None, 10, 10);
        let expected = operating * PRIZE_BASE_FACTOR;
        assert!((last - expected).abs() < 1.0);
    }

    #[test]
    fn invalid_inputs_return_zero() {
        assert_eq!(constructor_prize("gt3", None, 0, 14), 0.0);
        assert_eq!(constructor_prize("gt3", None, 1, 0), 0.0);
        assert_eq!(constructor_prize("gt3", None, 20, 14), 0.0);
    }

    #[test]
    fn midfield_is_near_average() {
        let operating = operacional("gt3", None);
        // posição central de um grid de 11 (posição 6) ≈ média
        let mid = constructor_prize("gt3", None, 6, 11);
        let avg = operating * (PRIZE_BASE_FACTOR + PRIZE_SLOPE_FACTOR / 2.0);
        assert!((mid - avg).abs() < operating * 0.05);
    }

    /// **As três classes do Endurance pagam três prêmios diferentes.**
    ///
    /// A campeã GT4 e a campeã LMP2 recebiam o MESMO cheque, porque o prêmio lia só
    /// `endurance` e a escala caía na divisão de referência para as três. São três
    /// campeonatos separados dentro da mesma corrida, com custos de operar que não se
    /// parecem — o prêmio tem que segui-los.
    #[test]
    fn as_tres_classes_do_endurance_pagam_premios_diferentes() {
        let gt4 = constructor_prize("endurance", Some("gt4"), 1, 6);
        let gt3 = constructor_prize("endurance", Some("gt3"), 1, 6);
        let lmp2 = constructor_prize("endurance", Some("lmp2"), 1, 6);

        assert!(
            gt4 < gt3 && gt3 < lmp2,
            "o prêmio precisa subir com o custo de operar a classe: \
             gt4 {gt4:.0}, gt3 {gt3:.0}, lmp2 {lmp2:.0}"
        );

        for (classe, premio) in [("gt4", gt4), ("gt3", gt3), ("lmp2", lmp2)] {
            let esperado =
                operacional("endurance", Some(classe)) * (PRIZE_BASE_FACTOR + PRIZE_SLOPE_FACTOR);
            assert!(
                (premio - esperado).abs() < 1.0,
                "{classe}: a campeã recebeu {premio:.0} e a âncora da classe dá {esperado:.0}"
            );
        }
    }

    /// A âncora da categoria sozinha não descreve nenhuma das pontas: ela é a divisão de
    /// referência, e as outras duas classes ficam a uma distância que o prêmio não pode
    /// ignorar. Este teste é o que ficaria vermelho se alguém devolvesse o cálculo
    /// class-blind ao lugar.
    #[test]
    fn a_ancora_da_categoria_nao_serve_para_as_pontas_do_endurance() {
        let pela_categoria = constructor_prize("endurance", None, 1, 6);
        let gt4 = constructor_prize("endurance", Some("gt4"), 1, 6);
        let lmp2 = constructor_prize("endurance", Some("lmp2"), 1, 6);

        assert!(
            (gt4 - pela_categoria).abs() > 1.0 && (lmp2 - pela_categoria).abs() > 1.0,
            "a âncora da categoria ({pela_categoria:.0}) coincidiu com as pontas \
             (gt4 {gt4:.0}, lmp2 {lmp2:.0}) — o cálculo voltou a ser cego para a classe"
        );
    }

    /// Numa categoria de classe única a classe não tem o que dizer, e o prêmio é o mesmo
    /// que sempre foi. A mudança de assinatura não pode deslocar as oito divisões
    /// monoclasse da escada.
    #[test]
    fn categoria_monoclasse_paga_pela_propria_ancora() {
        for (categoria, classe) in crate::economia::ancora::DIVISOES {
            if classe.is_some() {
                continue;
            }
            let premio = constructor_prize(categoria, None, 1, 10);
            let esperado = operacional(categoria, None) * (PRIZE_BASE_FACTOR + PRIZE_SLOPE_FACTOR);
            assert!(
                (premio - esperado).abs() < 1.0,
                "{categoria}: campeã recebeu {premio:.0} contra {esperado:.0} da âncora"
            );
        }
    }
}
