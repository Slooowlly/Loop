//! O modelo de despesa ANTIGO, congelado como dado histórico do harness.
//!
//! Isto era `finance::operacao` mais o braço `ModeloDeDespesa::Legado` de
//! `race::despesa::despesa_da_rodada`. Nenhum dos dois é mais código de produção: a rodada
//! debita `economia::evento` + `economia::temporada`, `quantidade × preço`, sem interruptor.
//!
//! **Por que não foi apagado.** O comparador (`comparar_modelos_de_despesa`) é o instrumento
//! que provou a troca, e ele só prova alguma coisa se tiver os DOIS lados dentro do mesmo
//! binário — confrontar dois relatórios de execuções diferentes não prova nada, porque a
//! árvore não é determinística entre elas. Apagar o modelo velho de vez transformaria o
//! comparador em tautologia: ele passaria a comparar o modelo novo com ele mesmo e daria
//! 1,00 em toda a escada. É exatamente o que já aconteceu uma vez com a tabela financeira
//! antiga, e o remédio é o mesmo de [`crate::economia::tests::legado`] — a régua vira dado.
//!
//! **Estes números não devem ser atualizados nunca.** Eles são o retrato de onde a economia
//! estava: nove pesos que são frações de `round_operating_base`, mais `0,18 × base` de
//! estrutura e `0,16 × base` de técnica. Somados, ~1,16 × base por rodada — a assinatura do
//! modelo velho, idêntica nas catorze divisões, porque quando toda linha é fração do mesmo
//! orçamento o total só pode ser uma constante vezes ele.
//!
//! O que este módulo AINDA divide com a produção, de propósito:
//!
//! - `OperationLine` e os tokens de bloco (`carro`/`logistica`/`equipe`), que são vocabulário
//!   de exibição e não aritmética do modelo;
//! - [`crate::constants::geografia::continente`], para que os dois lados do comparador vejam
//!   o mesmo mundo. Congelar a geografia junto faria a diferença medida misturar a troca de
//!   modelo com uma divergência de mapa.

use super::super::despesa::{
    DespesaDaRodada, EtapaFisica, OperationLine, GROUP_CAR, GROUP_CREW, GROUP_LOGISTICS,
};
use super::super::financas::RoundOperationContext;
use crate::constants::tracks::get_track;
use crate::finance::cashflow::LinhasDaDespesa;
use crate::models::team::Team;

/// Tudo que dimensionava a fatura de uma equipe numa rodada no modelo antigo.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OperationInputs {
    /// Custo operacional médio da rodada (escala da categoria / nº de rodadas).
    pub round_operating_base: f64,
    /// Instalações do time (0–100): motorhome/estrutura móvel maior custa mais no evento.
    pub facilities: f64,
    /// Qualidade do pit crew (0–100), proxy do tamanho da operação levada à etapa.
    pub pit_crew_quality: f64,
    /// Modulador de custo da economia global.
    pub cost_modifier: f64,
    /// Distância logística da etapa: casa / mesmo continente / intercontinental.
    pub travel_factor: f64,
    /// Fração da corrida que os carros do time completaram (0..1).
    pub laps_ratio: f64,
    /// Desgaste final médio dos pneus dos carros do time (0..1).
    pub tire_wear: f64,
}

/// Fator de logística por distância da etapa. Era o degrau que a fatura física trocou por
/// quilômetro de verdade.
pub(crate) const TRAVEL_HOME: f64 = 0.70;
pub(crate) const TRAVEL_CONTINENTAL: f64 = 1.00;
pub(crate) const TRAVEL_INTERCONTINENTAL: f64 = 1.45;

/// Pesos nominais de cada linha como fração do custo operacional da rodada. Calibrados para
/// que, num time mediano (`facilities` 50) e sob fatores médios (voltas ~0.95, desgaste ~0.65,
/// viagem 1.0, equipe 1.0), a soma desse ~0.62 — o que a fórmula achatada anterior a eles
/// cobrava (`0.42 + 0.004 × 50`).
const W_GASOLINA: f64 = 0.070;
const W_PNEUS: f64 = 0.073;
const W_PECAS: f64 = 0.041;
const W_FRETE: f64 = 0.125;
const W_VIAGEM: f64 = 0.095;
const W_ESTADIA: f64 = 0.080;
const W_DIARIAS: f64 = 0.045;
const W_ESTRUTURA: f64 = 0.051;
/// Inscrição era a única linha que ignorava TODOS os fatores: a mesma taxa para todo mundo,
/// em qualquer etapa.
const W_INSCRICAO: f64 = 0.045;

/// Instalações do time escalavam a operação levada à etapa (frete, comitiva, hospedagem).
/// Calibrado em 1.0 para `facilities` 50 (0 → 0.49, 100 → 1.51).
fn facilities_factor(facilities: f64) -> f64 {
    let f = facilities.clamp(0.0, 100.0);
    (0.196 + 0.004 * f) / 0.396
}

impl OperationInputs {
    /// Monta os fatores de uma equipe nesta etapa.
    pub(crate) fn for_round(
        round_operating_base: f64,
        facilities: f64,
        pit_crew_quality: f64,
        cost_modifier: f64,
        team_pais_sede: &str,
        track_id: u32,
        laps_ratio: f64,
        tire_wear: f64,
    ) -> Self {
        Self {
            round_operating_base,
            facilities,
            pit_crew_quality,
            cost_modifier,
            travel_factor: travel_factor(team_pais_sede, track_id),
            laps_ratio: laps_ratio.clamp(0.0, 1.0),
            tire_wear: tire_wear.clamp(0.0, 1.0),
        }
    }

    /// Fator de operação da equipe (0.80–1.20 pelo pit crew) — proxy do tamanho da comitiva
    /// que come e dorme na etapa.
    fn crew_factor(&self) -> f64 {
        0.80 + self.pit_crew_quality.clamp(0.0, 100.0) / 250.0
    }
}

/// Fator de logística da etapa para uma equipe, em três degraus. Pista desconhecida ou país
/// fora do mapa → neutro (1.0), nunca penaliza por falta de dado.
pub(crate) fn travel_factor(team_pais_sede: &str, track_id: u32) -> f64 {
    use crate::constants::geografia;

    let Some(track) = get_track(track_id) else {
        return TRAVEL_CONTINENTAL;
    };
    if pais_igual(team_pais_sede, track.pais) {
        return TRAVEL_HOME;
    }
    match (
        geografia::continente(team_pais_sede),
        geografia::continente(track.pais),
    ) {
        (Some(a), Some(b)) if a != b => TRAVEL_INTERCONTINENTAL,
        _ => TRAVEL_CONTINENTAL,
    }
}

/// Compara dois países tolerando a bandeira emoji na frente. **Não** dobra acento — é o
/// critério exato que o modelo antigo usava, e trocá-lo por
/// [`crate::constants::geografia::mesmo_pais`] moveria números que precisam ficar parados.
fn pais_igual(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.chars()
            .filter(|c| c.is_alphabetic() || c.is_whitespace())
            .collect::<String>()
            .trim()
            .to_lowercase()
    };
    let (a, b) = (norm(a), norm(b));
    !a.is_empty() && a == b
}

/// As linhas da fatura de operação da rodada no modelo antigo, já em dinheiro.
pub(crate) fn compute_operation_lines(input: &OperationInputs) -> Vec<OperationLine> {
    let base = input.round_operating_base.max(0.0) * input.cost_modifier;
    let travel = input.travel_factor;
    let crew = input.crew_factor();
    let infra = facilities_factor(input.facilities);
    // Combustível seguia as voltas rodadas; pneu seguia o desgaste com que o carro voltou.
    let fuel = 0.55 + 0.45 * input.laps_ratio;
    let tire = 0.60 + 0.55 * input.tire_wear;
    // Estadia sentia a viagem de forma amortecida.
    let stay = 0.55 + 0.45 * travel;

    let bruto = [
        (GROUP_CAR, "gasolina", W_GASOLINA * fuel),
        (GROUP_CAR, "pneus", W_PNEUS * tire),
        (GROUP_CAR, "pecas", W_PECAS),
        (GROUP_LOGISTICS, "frete", W_FRETE * travel * infra),
        (GROUP_LOGISTICS, "viagem", W_VIAGEM * travel * infra),
        (GROUP_LOGISTICS, "estadia", W_ESTADIA * stay * infra),
        (GROUP_LOGISTICS, "inscricao", W_INSCRICAO),
        (GROUP_CREW, "diarias", W_DIARIAS * crew * infra),
        (GROUP_CREW, "estrutura", W_ESTRUTURA * infra),
    ];

    bruto
        .into_iter()
        .map(|(group, key, peso)| OperationLine {
            key,
            group,
            cost: base * peso,
        })
        .filter(|l| l.cost > 0.0)
        .collect()
}

/// Custo de operação da rodada no modelo antigo = soma da fatura.
pub(crate) fn compute_operation_cost(input: &OperationInputs) -> f64 {
    compute_operation_lines(input).iter().map(|l| l.cost).sum()
}

/// Junta time + etapa nos fatores da fatura antiga. Vivia em `race::financas`, e saiu de lá
/// junto com o resto: hoje o único caminho que monta estes fatores é o comparador.
pub(crate) fn operation_inputs(
    team: &Team,
    round_operating_base: f64,
    cost_modifier: f64,
    ctx: RoundOperationContext,
) -> OperationInputs {
    OperationInputs::for_round(
        round_operating_base,
        team.facilities,
        team.pit_crew_quality,
        cost_modifier,
        &team.pais_sede,
        ctx.track_id,
        ctx.laps_ratio,
        ctx.tire_wear,
    )
}

/// **A despesa de UMA rodada de UMA equipe no modelo antigo.** Assinatura idêntica à de
/// [`super::super::despesa::despesa_da_rodada`] de propósito: é o que permite o comparador
/// trocar um pelo outro como ponteiro de função, sem que o resto do harness saiba qual está
/// rodando.
///
/// `rounds_in_season` e `etapa` entram e não são lidos — o modelo antigo não sabia a duração
/// real da prova nem quantos carros a equipe inscreveu, e é justamente essa cegueira que a
/// troca corrigiu.
pub(crate) fn despesa_legada_da_rodada(
    team: &Team,
    round_operating_base: f64,
    cost_modifier: f64,
    _rounds_in_season: f64,
    ctx: RoundOperationContext,
    _etapa: EtapaFisica,
) -> DespesaDaRodada {
    let linhas = compute_operation_lines(&operation_inputs(
        team,
        round_operating_base,
        cost_modifier,
        ctx,
    ));
    let estrutura = (round_operating_base * 0.18
        + team.engineering * round_operating_base * 0.0025
        + team.pit_crew_quality * round_operating_base * 0.0015)
        * cost_modifier;
    let peso = |chave: &str| {
        linhas
            .iter()
            .filter(|l| l.key == chave)
            .map(|l| l.cost)
            .sum::<f64>()
    };
    DespesaDaRodada {
        operacao: linhas.iter().map(|l| l.cost).sum(),
        estrutura,
        tecnica: round_operating_base * 0.16 * cost_modifier,
        // O modelo velho tem uma linha a mais que o ledger — `estrutura`, a estrutura MÓVEL
        // levada à etapa. Ela entra em `frete`, que é o bloco que de fato a transporta. O
        // mapeamento é aproximado e pode ser: este caminho existe para o comparador, que lê
        // os agregados, e nunca escreveu no ledger depois que a troca entrou.
        linhas_do_ledger: LinhasDaDespesa {
            combustivel: peso("gasolina"),
            pneus: peso("pneus"),
            desgaste_de_peca: peso("pecas"),
            frete: peso("frete") + peso("estrutura"),
            viagem_e_estadia: peso("viagem") + peso("estadia"),
            inscricao: peso("inscricao"),
            diarias: peso("diarias"),
            estrutura,
        },
        linhas,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> OperationInputs {
        OperationInputs {
            round_operating_base: 100_000.0,
            facilities: 50.0,
            pit_crew_quality: 50.0,
            cost_modifier: 1.0,
            travel_factor: TRAVEL_CONTINENTAL,
            laps_ratio: 0.95,
            tire_wear: 0.65,
        }
    }

    /// Os testes abaixo são os que acompanhavam `finance::operacao` e vieram junto. Eles não
    /// defendem mais nenhum comportamento de produção — defendem a INTEGRIDADE DA CÓPIA: se
    /// alguém mexer num peso deste arquivo, a base de comparação do comparador se move em
    /// silêncio e todo número medido contra ela vira folclore.
    #[test]
    fn total_medio_fica_perto_da_formula_achatada() {
        let i = inputs();
        let antigo = i.round_operating_base * 0.42 + i.facilities * i.round_operating_base * 0.004;
        let novo = compute_operation_cost(&i);
        let desvio = (novo - antigo).abs() / antigo;
        assert!(
            desvio < 0.05,
            "fatura itemizada desviou {:.1}% do custo achatado (antigo {antigo:.0}, novo {novo:.0})",
            desvio * 100.0
        );
    }

    /// Nenhuma linha concentrava mais de 25% da fatura no ponto de calibração e nos extremos
    /// de `facilities`.
    #[test]
    fn nenhuma_linha_domina_a_fatura() {
        for facilities in [0.0, 50.0, 100.0] {
            let i = OperationInputs {
                facilities,
                ..inputs()
            };
            let total = compute_operation_cost(&i);
            for linha in compute_operation_lines(&i) {
                let fatia = linha.cost / total;
                assert!(
                    fatia < 0.25,
                    "linha '{}' ficou com {:.0}% da fatura (facilities {facilities})",
                    linha.key,
                    fatia * 100.0
                );
            }
        }
    }

    /// A equivalência com o termo linear de instalações valia na faixa inteira, não só no
    /// time mediano.
    #[test]
    fn escala_por_instalacoes_bate_com_a_formula_antiga() {
        for facilities in [0.0, 25.0, 50.0, 75.0, 100.0] {
            let i = OperationInputs {
                facilities,
                ..inputs()
            };
            let antigo = i.round_operating_base * (0.42 + 0.004 * facilities);
            let novo = compute_operation_cost(&i);
            let desvio = (novo - antigo).abs() / antigo;
            assert!(
                desvio < 0.05,
                "facilities {facilities}: desvio de {:.1}% (antigo {antigo:.0}, novo {novo:.0})",
                desvio * 100.0
            );
        }
    }

    /// Casa < continental < intercontinental.
    #[test]
    fn distancia_da_etapa_move_o_custo() {
        let casa = compute_operation_cost(&OperationInputs {
            travel_factor: TRAVEL_HOME,
            ..inputs()
        });
        let cont = compute_operation_cost(&inputs());
        let longe = compute_operation_cost(&OperationInputs {
            travel_factor: TRAVEL_INTERCONTINENTAL,
            ..inputs()
        });
        assert!(
            casa < cont && cont < longe,
            "casa {casa:.0} / cont {cont:.0} / longe {longe:.0}"
        );
    }

    /// Abandonar cedo com pneu novo custava menos combustível e menos borracha.
    #[test]
    fn corrida_curta_queima_menos() {
        let inteira = compute_operation_cost(&OperationInputs {
            laps_ratio: 1.0,
            tire_wear: 0.9,
            ..inputs()
        });
        let abandono = compute_operation_cost(&OperationInputs {
            laps_ratio: 0.1,
            tire_wear: 0.1,
            ..inputs()
        });
        assert!(abandono < inteira);
    }

    /// A geografia que o modelo antigo enxergava continua sendo a mesma que a produção
    /// enxerga: etapa no país-sede é "em casa" mesmo com o dado legado sem bandeira, e a
    /// Austrália não cai na Europa por conter "Austria" sem acento.
    #[test]
    fn geografia_resolve_casa_e_continente() {
        use crate::constants::geografia::continente;
        assert_eq!(continente("🇦🇺 Austrália"), Some("oceania"));
        assert_eq!(continente("Australia"), Some("oceania"));
        assert_eq!(continente("🇦🇹 Áustria"), Some("europa"));
        assert!(pais_igual("🇧🇷 Brasil", "Brasil"));
        assert!(!pais_igual("🇧🇷 Brasil", "🇵🇹 Portugal"));
    }
}
