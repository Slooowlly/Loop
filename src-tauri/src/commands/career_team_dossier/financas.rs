//! Relatorio financeiro da equipe: ledger da ultima rodada, acumulado da temporada
//! corrente, grafico de caixa e projecao do premio de construtores.

use super::*;

/// Dossiê financeiro REAL de uma equipe, lido da tabela `team_finance_history`. Fonte
/// única da aba My Team: ledgers da última rodada, rosca de custos acumulados da
/// temporada e gráfico de caixa — substituindo os números fabricados no front. Save
/// sem histórico (antigo / sem corridas ainda) devolve um relatório vazio (o front
/// mostra estado honesto).
pub(crate) fn get_team_finance_report_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    category: &str,
    team_id: &str,
) -> Result<TeamFinanceReport, String> {
    /// Máximo de rodadas exibidas no gráfico de caixa.
    const TIMELINE_MAX: usize = 12;
    /// Janela lida do histórico: cobre a temporada corrente inteira + a cauda do gráfico.
    const HISTORY_WINDOW: i64 = 200;

    let category = category.trim().to_lowercase();
    let (db, _, _) = open_career_resources_read_only(base_dir, career_id)?;

    let entries = team_queries::get_team_finance_history_recent(&db.conn, team_id, HISTORY_WINDOW)
        .map_err(|e| format!("Falha ao carregar historico financeiro da equipe: {e}"))?;

    // Só as temporadas com rodada de CORRIDA gravada. As temporadas de backstory
    // gravam uma linha por ano com o prêmio de construtores e mais nada — receita
    // sem despesa, porque o sorteio histórico não simula economia. Somadas aqui,
    // viravam uma "temporada" de lucro puro e uma curva de caixa que só sobe. É o
    // mesmo recorte que o livro-caixa do dossiê aplica (`MEASURED_SEASONS_FILTER`).
    let temporadas_medidas: std::collections::HashSet<i32> = entries
        .iter()
        .filter(|entry| entry.round < team_queries::SEASON_CLOSE_ROUND)
        .map(|entry| entry.season_number)
        .collect();
    let entries: Vec<_> = entries
        .into_iter()
        .filter(|entry| temporadas_medidas.contains(&entry.season_number))
        .collect();

    let Some(latest_entry) = entries.last().cloned() else {
        return Ok(TeamFinanceReport::default());
    };

    let current_season = latest_entry.season_number;

    // Acumulado da temporada corrente (rosca de custos + leitura de receita).
    let season = acumula_temporada(&entries, current_season);
    let latest = rodada_do_ledger(&latest_entry);

    let start = entries.len().saturating_sub(TIMELINE_MAX);
    let cash_timeline = entries[start..]
        .iter()
        .map(|entry| TeamFinanceCashPoint {
            season_number: entry.season_number,
            round: entry.round,
            cash_balance: entry.cash_balance,
            net: entry.net,
            is_season_close: entry.constructor_prize_income > 0.0,
        })
        .collect();

    // Projeção: prêmio de construtores ESTIMADO se a temporada terminasse agora, pela
    // posição atual no campeonato. Reusa as classificações (fórmula única em `prize`); é
    // só exibição — não toca caixa nem IA. Falha nas classificações → projeção neutra (0).
    let (expected_constructor_prize, current_position, grid_size) =
        match get_teams_standings_in_base_dir(base_dir, career_id, &category) {
            Ok(standings) => {
                let grid_size = standings.len() as i32;
                match standings.iter().find(|s| s.id == team_id) {
                    Some(standing) => (
                        crate::finance::prize::constructor_prize(
                            &category,
                            standing.posicao,
                            grid_size,
                        ),
                        standing.posicao,
                        grid_size,
                    ),
                    None => (0.0, 0, grid_size),
                }
            }
            Err(_) => (0.0, 0, 0),
        };

    Ok(TeamFinanceReport {
        rounds_recorded: entries.len() as i32,
        latest: Some(latest),
        season: Some(season),
        cash_timeline,
        expected_constructor_prize,
        current_position,
        grid_size,
    })
}

/// Uma linha do ledger virando a linha que a tela lê.
///
/// A DESPESA sai decomposta nas oito linhas físicas que o caixa debitou — combustível,
/// pneus, desgaste de peça, frete, viagem e estadia, inscrição, diárias e o rateio da
/// estrutura. Antes o dossiê mostrava dois agregados (`event_operations_cost` e
/// `structural_maintenance_cost`) produzidos por pesos sobre um orçamento abstrato, e o
/// jogador lia "gasolina" num número que não tinha relação com combustível.
///
/// Os dois agregados continuam no struct porque a tela ainda os usa nos totais, e eles
/// **são** a soma das linhas: gravados de uma origem só, nunca recalculados aqui.
fn rodada_do_ledger(entry: &team_queries::TeamFinanceHistoryEntry) -> TeamFinanceRound {
    TeamFinanceRound {
        season_number: entry.season_number,
        round: entry.round,
        sponsorship_income: entry.sponsorship_income,
        gate_income: entry.gate_income,
        result_bonus: entry.result_bonus,
        partial_prize_income: entry.partial_prize_income,
        aid_income: entry.aid_income,
        salary_expense: entry.salary_expense,
        custo_combustivel: entry.linhas.combustivel,
        custo_pneus: entry.linhas.pneus,
        custo_desgaste_de_peca: entry.linhas.desgaste_de_peca,
        custo_frete: entry.linhas.frete,
        custo_viagem_e_estadia: entry.linhas.viagem_e_estadia,
        custo_inscricao: entry.linhas.inscricao,
        custo_diarias: entry.linhas.diarias,
        custo_estrutura: entry.linhas.estrutura,
        event_operations_cost: entry.event_operations_cost,
        structural_maintenance_cost: entry.structural_maintenance_cost,
        technical_investment_cost: entry.technical_investment_cost,
        debt_service_cost: entry.debt_service_cost,
        constructor_prize_income: entry.constructor_prize_income,
        income_total: entry.income_total,
        expenses_total: entry.expenses_total,
        net: entry.net,
    }
}

/// O acumulado de uma temporada: soma linha a linha das rodadas dela.
///
/// O campo `round` do resultado carrega a CONTAGEM de rodadas somadas, não uma rodada — é
/// como a tela lê o denominador da média.
///
/// Somar cada linha separadamente (em vez de somar os agregados e ratear depois) é o que
/// mantém a decomposição verdadeira ao longo da temporada: uma etapa intercontinental pesa
/// no frete e não no combustível, e o acumulado tem que mostrar isso.
fn acumula_temporada(
    entries: &[team_queries::TeamFinanceHistoryEntry],
    season_number: i32,
) -> TeamFinanceRound {
    let mut total = TeamFinanceRound {
        season_number,
        ..Default::default()
    };
    for entry in entries.iter().filter(|e| e.season_number == season_number) {
        let r = rodada_do_ledger(entry);
        total.sponsorship_income += r.sponsorship_income;
        total.gate_income += r.gate_income;
        total.result_bonus += r.result_bonus;
        total.partial_prize_income += r.partial_prize_income;
        total.aid_income += r.aid_income;
        total.salary_expense += r.salary_expense;
        total.custo_combustivel += r.custo_combustivel;
        total.custo_pneus += r.custo_pneus;
        total.custo_desgaste_de_peca += r.custo_desgaste_de_peca;
        total.custo_frete += r.custo_frete;
        total.custo_viagem_e_estadia += r.custo_viagem_e_estadia;
        total.custo_inscricao += r.custo_inscricao;
        total.custo_diarias += r.custo_diarias;
        total.custo_estrutura += r.custo_estrutura;
        total.event_operations_cost += r.event_operations_cost;
        total.structural_maintenance_cost += r.structural_maintenance_cost;
        total.technical_investment_cost += r.technical_investment_cost;
        total.debt_service_cost += r.debt_service_cost;
        total.constructor_prize_income += r.constructor_prize_income;
        total.income_total += r.income_total;
        total.expenses_total += r.expenses_total;
        total.net += r.net;
        total.round += 1;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finance::cashflow::LinhasDaDespesa;

    fn linha(season: i32, round: i32, escala: f64) -> team_queries::TeamFinanceHistoryEntry {
        let linhas = LinhasDaDespesa {
            combustivel: 1_200.0 * escala,
            pneus: 9_400.0 * escala,
            desgaste_de_peca: 5_100.0 * escala,
            frete: 14_000.0 * escala,
            viagem_e_estadia: 8_300.0 * escala,
            inscricao: 2_500.0 * escala,
            diarias: 6_700.0 * escala,
            estrutura: 31_000.0 * escala,
        };
        team_queries::TeamFinanceHistoryEntry {
            season_number: season,
            round,
            event_operations_cost: linhas.total_da_etapa(),
            structural_maintenance_cost: linhas.estrutura,
            linhas,
            ..Default::default()
        }
    }

    /// **A invariante, no lado que o jogador lê.** A decomposição que o dossiê mostra soma
    /// exatamente a despesa de operação e de estrutura que saiu do caixa — na rodada e no
    /// acumulado da temporada. Sem isto o dossiê volta a ser o que era: uma fatura paralela
    /// com números plausíveis que não são os do caixa.
    #[test]
    fn a_decomposicao_do_dossie_fecha_com_o_que_o_caixa_debitou() {
        let entries = [linha(3, 1, 1.0), linha(3, 2, 0.6), linha(3, 3, 1.4)];

        for r in [
            rodada_do_ledger(&entries[0]),
            acumula_temporada(&entries, 3),
        ] {
            let soma_das_sete = r.custo_combustivel
                + r.custo_pneus
                + r.custo_desgaste_de_peca
                + r.custo_frete
                + r.custo_viagem_e_estadia
                + r.custo_inscricao
                + r.custo_diarias;
            assert!(
                (soma_das_sete - r.event_operations_cost).abs() < 1e-6,
                "as sete linhas somam {soma_das_sete:.2} e a operação debitada foi {:.2}",
                r.event_operations_cost
            );
            assert!(
                (r.custo_estrutura - r.structural_maintenance_cost).abs() < 1e-6,
                "estrutura mostrada {:.2} vs debitada {:.2}",
                r.custo_estrutura,
                r.structural_maintenance_cost
            );
        }
    }

    /// O acumulado é a soma das rodadas DA TEMPORADA pedida, e `round` carrega a contagem.
    /// Uma temporada vizinha no mesmo histórico não pode vazar para dentro da rosca.
    #[test]
    fn o_acumulado_soma_so_a_temporada_pedida() {
        let entries = [linha(2, 9, 5.0), linha(3, 1, 1.0), linha(3, 2, 1.0)];
        let season = acumula_temporada(&entries, 3);

        assert_eq!(season.round, 2, "duas rodadas da temporada 3");
        assert!(
            (season.custo_pneus - 2.0 * 9_400.0).abs() < 1e-6,
            "a temporada 2 vazou para o acumulado: pneus {:.2}",
            season.custo_pneus
        );
    }
}
