//! Evento de venda / nova diretoria para equipes cronicamente falidas.
//!
//! Quando uma equipe passa duas temporadas consecutivas em colapso financeiro
//! (a segunda já em modo all-in), ela é "vendida": entra uma nova diretoria que
//! assume o passivo e recoloca a equipe para rodar.
//!
//! **Falir não pode ser lucrativo.** A versão anterior disto era um prêmio: a
//! dívida sumia, entrava caixa equivalente a 45% do caixa-MÉDIO da categoria
//! (na GT3, ~7 M) e todos os atributos de performance eram RE-SORTEADOS numa
//! faixa ampla de 20 a 90 — ou seja, quebrar podia deixar a equipe melhor do que
//! ela era. Medido num save real: a T101 acumulou 24,6 M de dívida, foi vendida,
//! teve tudo perdoado e ainda RECEBEU 6,975 M.
//!
//! O que a nova diretoria entrega agora é o mínimo para OPERAR: **dois meses de
//! operação da divisão**, contra os seis meses da equipe mediana saudável — a mesma
//! unidade em que o jogo inteiro mede fôlego desde a troca da âncora de estoque. E o
//! que a equipe perde aparece onde importa — o carro regride, a estrutura
//! regride, a reputação regride. Nada é re-sorteado: tudo DECAI a partir do que
//! a equipe tinha, rumo a um piso operacional. Quem já estava no fundo não
//! afunda mais; quem tinha algo a perder perde.
//!
//! A IDENTIDADE é preservada — nome, cores, país, ano de fundação, histórico de
//! títulos e resultados continuam intactos.
//!
//! A dívida continua sendo quitada na venda (é o que um processo de recuperação
//! faz: o credor toma o prejuízo). Manter passivo aqui devolveria a equipe ao
//! colapso no mesmo instante, e o gatilho do colapso não é assunto deste módulo.

use rand::Rng;

use crate::finance::state::refresh_team_financial_state;
use crate::models::team::Team;

/// Caixa da nova diretoria, em **MESES DE OPERAÇÃO** da divisão.
///
/// Esta constante já foi reescrita duas vezes, e as duas vezes pelo mesmo motivo: ela estava
/// escrita numa unidade que não era a do resto do jogo, então mudar a âncora do jogo mudava
/// silenciosamente o que ela significa.
///
/// - Original: `45% do caixa-MÉDIO da categoria`. Na GT3 isso dava ~7 M — um prêmio.
/// - Primeira correção: `35% do custo operacional anual`, ou seja 4,2 meses de operação.
///   Parecia austero porque a referência de caixa da escada era de ~24 meses.
/// - **A âncora de estoque mudou**: o caixa de referência da divisão passou a ser
///   [`crate::economia::temporada::caixa_meses_de_referencia`] — 6 meses, numa faixa de 1 a
///   11. Os mesmos 4,2 meses viraram **70% do caixa de uma equipe mediana saudável**, e a
///   seção 2.5 ("falir é lucrativo") voltou inteira sem que uma linha desta função mudasse.
///
/// Agora ela está na MESMA unidade da âncora, então não há como a escala mudar debaixo dela
/// de novo: dois meses são dois meses em qualquer degrau, e são um terço do caixa da equipe
/// mediana. É a ponta de baixo da faixa declarada — a equipe que "vive de etapa em etapa,
/// qualquer imprevisto vira dívida", que é exatamente o que sai de uma recuperação judicial.
///
/// Um mês seria o piso literal da faixa e devolveria a equipe ao colapso na temporada
/// seguinte (venda em loop). Dois é o mínimo com que ela consegue operar sem que a falência
/// vire um evento recorrente por construção.
pub const SALE_CAIXA_MESES: f64 = 2.0;

/// Piso operacional dos atributos de estrutura (0–100) na regressão da venda.
/// A equipe vendida chega perto do mínimo com que ainda se toca uma operação.
pub const SALE_STRUCTURE_FLOOR: f64 = 22.0;
/// Fração da distância até o piso que SOBREVIVE à venda. Faixa, não número fixo:
/// a nova diretoria pode ser mais ou menos competente, mas nunca é uma melhoria.
pub const SALE_STRUCTURE_KEEP: (f64, f64) = (0.55, 0.78);
/// Reputação cai mais fundo que a estrutura — quebrar é público.
pub const SALE_REPUTATION_FLOOR: f64 = 18.0;
pub const SALE_REPUTATION_KEEP: (f64, f64) = (0.50, 0.72);
/// Piso e retenção do `car_performance` (domínio [-5, 16]). O carro é o que o
/// jogador sente no assento, então é o eixo que cai mais.
pub const SALE_CAR_FLOOR: f64 = -3.0;
pub const SALE_CAR_KEEP: (f64, f64) = (0.40, 0.70);
/// Moral da equipe recém-vendida: abalada, nunca eufórica (antes era 0.9–1.3).
pub const SALE_MORALE: (f64, f64) = (0.80, 0.95);

/// O que a venda fez com a equipe — alimenta a notícia e o registro na ficha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamSaleOutcome {
    pub debt_cleared: f64,
    pub cash_injected: f64,
    /// Quanto o `car_performance` regrediu (positivo = perdeu).
    pub car_performance_lost: f64,
    /// Queda média dos atributos de estrutura (engenharia, instalações, box).
    pub structure_lost: f64,
}

/// O caixa que a nova diretoria põe na mesa, na unidade da âncora.
///
/// Aceita a categoria com a classe embutida (`"endurance:gt3"`) pela mesma convenção de
/// [`crate::finance::planning::category_finance_scale`] — a classe explícita de `Team::classe`
/// tem precedência, porque é onde ela de fato mora.
pub fn caixa_da_nova_diretoria(categoria: &str, classe: Option<&str>) -> f64 {
    let base = categoria.split(':').next().unwrap_or(categoria);
    let classe = classe.filter(|c| !c.is_empty()).or_else(|| {
        categoria
            .split_once(':')
            .map(|(_, c)| c)
            .filter(|c| !c.is_empty())
    });
    crate::economia::temporada::caixa_para_meses(base, classe, SALE_CAIXA_MESES)
}

/// Um passo de REGRESSÃO rumo a um piso: sobra `mantem` da distância até ele.
/// Quem já está no piso (ou abaixo) não se move — a queda tem fundo.
fn regride(atual: f64, piso: f64, mantem: f64) -> f64 {
    if atual <= piso {
        atual
    } else {
        piso + (atual - piso) * mantem.clamp(0.0, 1.0)
    }
}

/// Renova uma equipe falida sob nova diretoria. Preserva identidade e histórico;
/// quita o passivo, entrega caixa de giro e REGRIDE a capacidade competitiva.
/// Retorna o impacto para a notícia e o registro histórico.
pub fn apply_team_sale(team: &mut Team, rng: &mut impl Rng) -> TeamSaleOutcome {
    let debt_cleared = team.debt_balance.max(0.0);
    let cash_injected = caixa_da_nova_diretoria(&team.categoria, team.classe.as_deref());

    // ── Finanças: passivo quitado, caixa de giro ──
    team.debt_balance = 0.0;
    team.cash_balance = cash_injected;
    team.parachute_payment_remaining = 0.0;
    team.last_round_income = 0.0;
    team.last_round_expenses = 0.0;
    team.last_round_net = 0.0;
    // A nova diretoria entra cortando: austeridade, não projeto novo.
    team.season_strategy = "austerity".to_string();

    // ── Regressão: cada eixo perde parte da distância até o seu piso ──
    // Um sorteio por eixo (a queda não é uniforme entre departamentos), mas todos
    // no MESMO sentido: para baixo. Nada aqui pode melhorar a equipe.
    let mut cai = |atual: f64, piso: f64, faixa: (f64, f64)| {
        regride(atual, piso, rng.gen_range(faixa.0..=faixa.1))
    };

    let estrutura_antes = (team.engineering + team.facilities + team.pit_crew_quality) / 3.0;
    team.engineering = cai(team.engineering, SALE_STRUCTURE_FLOOR, SALE_STRUCTURE_KEEP);
    team.facilities = cai(team.facilities, SALE_STRUCTURE_FLOOR, SALE_STRUCTURE_KEEP);
    team.pit_crew_quality = cai(
        team.pit_crew_quality,
        SALE_STRUCTURE_FLOOR,
        SALE_STRUCTURE_KEEP,
    );
    let estrutura_depois = (team.engineering + team.facilities + team.pit_crew_quality) / 3.0;

    team.confiabilidade = cai(
        team.confiabilidade,
        SALE_STRUCTURE_FLOOR,
        SALE_STRUCTURE_KEEP,
    );
    team.aerodinamica = cai(team.aerodinamica, SALE_STRUCTURE_FLOOR, SALE_STRUCTURE_KEEP);
    team.motor = cai(team.motor, SALE_STRUCTURE_FLOOR, SALE_STRUCTURE_KEEP);
    team.chassi = cai(team.chassi, SALE_STRUCTURE_FLOOR, SALE_STRUCTURE_KEEP);
    team.reputacao = cai(team.reputacao, SALE_REPUTATION_FLOOR, SALE_REPUTATION_KEEP);

    let carro_antes = team.car_performance;
    team.car_performance = cai(team.car_performance, SALE_CAR_FLOOR, SALE_CAR_KEEP);

    // Moral abalada — a equipe inteira sabe o que aconteceu.
    team.morale = rng.gen_range(SALE_MORALE.0..=SALE_MORALE.1);

    // Recalcula o estado financeiro. Com o passivo quitado e caixa de giro a
    // equipe SAI do colapso: a punição é esportiva, não uma sentença de morte
    // financeira que a revenderia na temporada seguinte.
    refresh_team_financial_state(team);

    TeamSaleOutcome {
        debt_cleared,
        cash_injected,
        car_performance_lost: (carro_antes - team.car_performance).max(0.0),
        structure_lost: (estrutura_antes - estrutura_depois).max(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::team::placeholder_team_from_db;
    use rand::{rngs::StdRng, SeedableRng};

    /// Equipe quebrada, mas EXISTENTE: carro ruim e estrutura sucateada, não zerada.
    ///
    /// O `placeholder_team_from_db` nasce com `engineering`/`facilities` em 0 — é um
    /// stub de leitura, não uma equipe do mundo. Com o modelo antigo isso não
    /// aparecia porque a venda RE-SORTEAVA os atributos e fabricava estrutura do
    /// nada; agora que nada é fabricado, medir a venda num time de estrutura zero
    /// mediria um time que não existe em nenhum grid.
    fn collapsed_team() -> Team {
        let mut team = placeholder_team_from_db(
            "T001".to_string(),
            "Escuderia Histórica".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = -100_000.0;
        team.debt_balance = 40_000_000.0;
        team.financial_state = "collapse".to_string();
        team.car_performance = -4.0;
        team.engineering = 34.0;
        team.facilities = 30.0;
        team.pit_crew_quality = 32.0;
        team.confiabilidade = 40.0;
        team.aerodinamica = 33.0;
        team.motor = 35.0;
        team.chassi = 31.0;
        team.reputacao = 35.0;
        team
    }

    #[test]
    fn sale_clears_debt_and_injects_cash() {
        let mut team = collapsed_team();
        let mut rng = StdRng::seed_from_u64(1);
        apply_team_sale(&mut team, &mut rng);

        assert_eq!(team.debt_balance, 0.0, "dívida deve ser quitada");
        assert!(team.cash_balance > 0.0, "caixa deve ser injetado");
        assert_ne!(
            team.financial_state, "collapse",
            "deve sair do colapso após a venda"
        );
    }

    #[test]
    fn sale_preserves_identity_and_history() {
        let mut team = collapsed_team();
        team.historico_vitorias = 42;
        team.historico_titulos_construtores = 3;
        let nome = team.nome.clone();
        let cor = team.cor_primaria.clone();
        let fundacao = team.ano_fundacao;

        let mut rng = StdRng::seed_from_u64(2);
        apply_team_sale(&mut team, &mut rng);

        assert_eq!(team.nome, nome, "nome preservado");
        assert_eq!(team.cor_primaria, cor, "cor preservada");
        assert_eq!(team.ano_fundacao, fundacao, "ano de fundação preservado");
        assert_eq!(team.historico_vitorias, 42, "histórico preservado");
        assert_eq!(
            team.historico_titulos_construtores, 3,
            "títulos preservados"
        );
    }

    #[test]
    fn sale_attributes_stay_in_valid_ranges() {
        let mut team = collapsed_team();
        let mut rng = StdRng::seed_from_u64(3);
        apply_team_sale(&mut team, &mut rng);

        assert!((-5.0..=16.0).contains(&team.car_performance));
        assert!((0.0..=100.0).contains(&team.engineering));
        assert!((0.0..=100.0).contains(&team.reputacao));
    }

    /// Equipe com algo a perder: estrutura e carro acima do piso operacional.
    fn team_with_something_to_lose() -> Team {
        let mut team = collapsed_team();
        team.engineering = 62.0;
        team.facilities = 58.0;
        team.pit_crew_quality = 55.0;
        team.confiabilidade = 60.0;
        team.aerodinamica = 55.0;
        team.motor = 57.0;
        team.chassi = 54.0;
        team.reputacao = 50.0;
        team.car_performance = 6.0;
        team
    }

    #[test]
    fn falir_nao_paga_mais_que_o_custo_de_operar() {
        // O defeito medido: a venda entregava 45% do caixa-MÉDIO da categoria
        // (GT3 ≈ 6,975 M). Agora o aporte é caixa de giro sobre o custo
        // operacional — tem que ser bem menor que o prêmio antigo.
        let mut team = team_with_something_to_lose();
        let mut rng = StdRng::seed_from_u64(7);
        let outcome = apply_team_sale(&mut team, &mut rng);

        // O prêmio antigo é um número HISTÓRICO, congelado: 45% do caixa-médio que a tabela
        // escrita à mão declarava para a GT3 (15,5 milhões). Calculá-lo com a escala viva
        // deixou de medir alguma coisa quando o caixa esperado virou consequência — os dois
        // lados passaram a andar juntos e a comparação virou tautologia.
        const PREMIO_ANTIGO: f64 = 15_500_000.0 * 0.45;
        let premio_antigo = PREMIO_ANTIGO;
        assert!(
            outcome.cash_injected < premio_antigo * 0.5,
            "aporte {} deveria ser menos da metade do prêmio antigo {premio_antigo}",
            outcome.cash_injected
        );
        // E tem que ser suficiente para rodar: dois meses inteiros de operação.
        assert!(
            outcome.cash_injected
                >= crate::finance::state::custo_operacional_mensal("gt3", None) * 1.9
        );
    }

    /// GUARD DE UNIDADE. Este é o teste que faltava nas duas versões anteriores da
    /// constante: as duas foram calibradas contra uma âncora que depois mudou embaixo
    /// delas, e nada avisou. O aporte da venda tem que continuar sendo uma fração
    /// declarada do caixa da equipe MEDIANA, seja qual for a âncora da vez.
    #[test]
    fn a_venda_paga_uma_fracao_declarada_do_caixa_mediano() {
        use crate::economia::temporada::{caixa_de_referencia, caixa_meses_de_referencia};
        for (categoria, classe) in [
            ("mazda_rookie", None),
            ("bmw_m2", None),
            ("gt4", None),
            ("gt3", None),
            ("lmp2", None),
            ("endurance", Some("gt4")),
            ("endurance", Some("lmp2")),
        ] {
            let aporte = caixa_da_nova_diretoria(categoria, classe);
            let mediano = caixa_de_referencia(categoria, classe);
            let razao = aporte / mediano;
            let esperado = SALE_CAIXA_MESES / caixa_meses_de_referencia();
            assert!(
                (razao - esperado).abs() < 0.01,
                "{categoria}/{classe:?}: venda paga {:.0}% do caixa mediano, esperado {:.0}%",
                razao * 100.0,
                esperado * 100.0
            );
            // E a fração declarada tem que ser MINORITÁRIA — falir não pode aproximar a
            // equipe do caixa de quem não faliu.
            assert!(
                razao < 0.5,
                "{categoria}: falir entrega {razao:.2} do mediano"
            );
        }
    }

    /// A chave com classe embutida (`"endurance:gt3"`) tem que resolver igual à classe
    /// explícita — é a mesma convenção de `category_finance_scale`, e o Endurance é onde
    /// confundir as classes custa 3× no caixa.
    #[test]
    fn a_classe_embutida_resolve_como_a_explicita() {
        let embutida = caixa_da_nova_diretoria("endurance:lmp2", None);
        let explicita = caixa_da_nova_diretoria("endurance", Some("lmp2"));
        assert!((embutida - explicita).abs() < 1.0);
        assert!(explicita > caixa_da_nova_diretoria("endurance", Some("gt4")) * 2.0);
    }

    #[test]
    fn venda_nunca_melhora_a_equipe() {
        // O pior do modelo antigo: os atributos eram RE-SORTEADOS em 20–90, então
        // quebrar podia ser um upgrade. Nenhum eixo pode subir na venda.
        let antes = team_with_something_to_lose();
        for semente in 0..40u64 {
            let mut team = antes.clone();
            let mut rng = StdRng::seed_from_u64(semente);
            apply_team_sale(&mut team, &mut rng);

            assert!(team.engineering <= antes.engineering, "semente {semente}");
            assert!(team.facilities <= antes.facilities, "semente {semente}");
            assert!(
                team.pit_crew_quality <= antes.pit_crew_quality,
                "semente {semente}"
            );
            assert!(team.reputacao <= antes.reputacao, "semente {semente}");
            assert!(
                team.car_performance <= antes.car_performance,
                "semente {semente}"
            );
            assert!(team.morale <= 1.0, "moral abalada: {}", team.morale);
        }
    }

    #[test]
    fn a_queda_aparece_no_que_o_jogador_sente() {
        // O carro e a estrutura têm que cair de forma PERCEPTÍVEL, senão a
        // falência volta a ser um número no balanço.
        let antes = team_with_something_to_lose();
        let mut team = antes.clone();
        let mut rng = StdRng::seed_from_u64(11);
        let outcome = apply_team_sale(&mut team, &mut rng);

        assert!(
            outcome.car_performance_lost > 2.0,
            "carro caiu pouco: {}",
            outcome.car_performance_lost
        );
        assert!(
            outcome.structure_lost > 8.0,
            "estrutura caiu pouco: {}",
            outcome.structure_lost
        );
    }

    #[test]
    fn quem_ja_esta_no_fundo_nao_afunda_mais() {
        // A queda tem fundo: uma equipe já no piso operacional não é empurrada
        // para baixo dele a cada venda (senão vendas repetidas viram espiral).
        let mut team = collapsed_team();
        team.engineering = SALE_STRUCTURE_FLOOR - 5.0;
        team.facilities = SALE_STRUCTURE_FLOOR;
        team.car_performance = SALE_CAR_FLOOR - 1.0;
        let engenharia = team.engineering;
        let instalacoes = team.facilities;
        let carro = team.car_performance;

        let mut rng = StdRng::seed_from_u64(13);
        apply_team_sale(&mut team, &mut rng);

        assert_eq!(team.engineering, engenharia);
        assert_eq!(team.facilities, instalacoes);
        assert_eq!(team.car_performance, carro);
    }

    #[test]
    fn vendida_sai_do_colapso_para_poder_operar() {
        // A punição é esportiva, não uma sentença: a equipe tem que conseguir
        // rodar a temporada seguinte, senão seria revendida em loop.
        for semente in 0..20u64 {
            let mut team = team_with_something_to_lose();
            let mut rng = StdRng::seed_from_u64(semente);
            apply_team_sale(&mut team, &mut rng);
            assert_ne!(
                team.financial_state, "collapse",
                "semente {semente} deixou a equipe em colapso"
            );
        }
    }
}
