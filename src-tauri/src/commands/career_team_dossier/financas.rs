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
    let (db, _, _) = open_career_resources_for_category_read(base_dir, career_id, &category)?;

    let entries = team_queries::get_team_finance_history_recent(&db.conn, team_id, HISTORY_WINDOW)
        .map_err(|e| format!("Falha ao carregar historico financeiro da equipe: {e}"))?;

    let Some(latest_entry) = entries.last().cloned() else {
        return Ok(TeamFinanceReport::default());
    };

    let current_season = latest_entry.season_number;

    // Acumulado da temporada corrente (rosca de custos + leitura de receita).
    let mut season = TeamFinanceRound {
        season_number: current_season,
        ..Default::default()
    };
    let mut season_rounds = 0;
    for entry in entries.iter().filter(|e| e.season_number == current_season) {
        season.sponsorship_income += entry.sponsorship_income;
        season.gate_income += entry.gate_income;
        season.result_bonus += entry.result_bonus;
        season.partial_prize_income += entry.partial_prize_income;
        season.aid_income += entry.aid_income;
        season.salary_expense += entry.salary_expense;
        season.event_operations_cost += entry.event_operations_cost;
        season.structural_maintenance_cost += entry.structural_maintenance_cost;
        season.technical_investment_cost += entry.technical_investment_cost;
        season.debt_service_cost += entry.debt_service_cost;
        season.constructor_prize_income += entry.constructor_prize_income;
        season.income_total += entry.income_total;
        season.expenses_total += entry.expenses_total;
        season.net += entry.net;
        season_rounds += 1;
    }
    season.round = season_rounds; // aqui `round` carrega a CONTAGEM de rodadas somadas.

    let latest = TeamFinanceRound {
        season_number: latest_entry.season_number,
        round: latest_entry.round,
        sponsorship_income: latest_entry.sponsorship_income,
        gate_income: latest_entry.gate_income,
        result_bonus: latest_entry.result_bonus,
        partial_prize_income: latest_entry.partial_prize_income,
        aid_income: latest_entry.aid_income,
        salary_expense: latest_entry.salary_expense,
        event_operations_cost: latest_entry.event_operations_cost,
        structural_maintenance_cost: latest_entry.structural_maintenance_cost,
        technical_investment_cost: latest_entry.technical_investment_cost,
        debt_service_cost: latest_entry.debt_service_cost,
        constructor_prize_income: latest_entry.constructor_prize_income,
        income_total: latest_entry.income_total,
        expenses_total: latest_entry.expenses_total,
        net: latest_entry.net,
    };

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
