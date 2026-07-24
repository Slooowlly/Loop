//! Finanças da equipe: snapshot de caixa/dívida e o histórico financeiro por rodada.

use rusqlite::{params, Connection};

use crate::db::connection::DbError;
use crate::finance::cashflow::{RoundCashflowSummary, TeamRoundFinanceContext};
use crate::models::team::Team;

use super::mapeamento::ensure_team_rows_affected;

pub fn update_team_finance_snapshot(conn: &Connection, team: &Team) -> Result<(), DbError> {
    let affected = conn.execute(
        "UPDATE teams
         SET cash_balance = ?1,
             debt_balance = ?2,
             financial_state = ?3,
             season_strategy = ?4,
             last_round_income = ?5,
             last_round_expenses = ?6,
             last_round_net = ?7,
             parachute_payment_remaining = ?8
         WHERE id = ?9",
        params![
            team.cash_balance,
            team.debt_balance,
            &team.financial_state,
            &team.season_strategy,
            team.last_round_income,
            team.last_round_expenses,
            team.last_round_net,
            team.parachute_payment_remaining,
            &team.id,
        ],
    )?;
    ensure_team_rows_affected(
        affected,
        &team.id,
        "atualizar snapshot financeiro da equipe",
    )?;
    Ok(())
}

/// Ajusta o caixa de um time por um delta (positivo credita, negativo debita). Base
/// da 1ª transferência de dinheiro time→time (multa de rescisão do poaching, Fase 2b).
pub fn adjust_team_cash(conn: &Connection, team_id: &str, delta: f64) -> Result<(), DbError> {
    let affected = conn.execute(
        "UPDATE teams SET cash_balance = cash_balance + ?1, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?2",
        params![delta, team_id],
    )?;
    ensure_team_rows_affected(affected, team_id, "ajustar caixa da equipe")?;
    Ok(())
}

/// Uma linha do histórico financeiro por rodada (tabela `team_finance_history`).
/// Carrega a divisão REAL de receita/despesa da rodada + caixa/dívida resultantes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TeamFinanceHistoryEntry {
    pub season_number: i32,
    pub round: i32,
    pub category: String,
    pub sponsorship_income: f64,
    /// Bilheteria/portão da rodada (Fase 3 do Estrelato). 0 nas linhas legadas.
    pub gate_income: f64,
    pub result_bonus: f64,
    pub partial_prize_income: f64,
    pub aid_income: f64,
    pub salary_expense: f64,
    pub event_operations_cost: f64,
    pub structural_maintenance_cost: f64,
    pub technical_investment_cost: f64,
    pub debt_service_cost: f64,
    /// Prêmio de construtores creditado no encerramento da temporada (0 em rodadas de
    /// corrida; > 0 só na linha de fechamento gravada por `insert_team_finance_season_close`).
    pub constructor_prize_income: f64,
    pub income_total: f64,
    pub expenses_total: f64,
    pub net: f64,
    pub cash_balance: f64,
    pub debt_balance: f64,
}

/// Grava a divisão financeira REAL de uma rodada para uma equipe. `team` deve estar no
/// estado PÓS-rodada (já com `apply_round_cashflow` aplicado), para `cash_balance`/
/// `debt_balance` refletirem o resultado. `INSERT OR REPLACE` na chave
/// (team_id, season_number, round) torna a gravação idempotente contra re-simulação.
pub fn insert_team_finance_history(
    conn: &Connection,
    team: &Team,
    context: &TeamRoundFinanceContext,
    summary: &RoundCashflowSummary,
    season_number: i32,
    round: i32,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT OR REPLACE INTO team_finance_history (
            team_id, season_number, round, category,
            sponsorship_income, gate_income, result_bonus, partial_prize_income, aid_income,
            salary_expense, event_operations_cost, structural_maintenance_cost,
            technical_investment_cost, debt_service_cost,
            income_total, expenses_total, net, cash_balance, debt_balance
        ) VALUES (
            :team_id, :season_number, :round, :category,
            :sponsorship_income, :gate_income, :result_bonus, :partial_prize_income, :aid_income,
            :salary_expense, :event_operations_cost, :structural_maintenance_cost,
            :technical_investment_cost, :debt_service_cost,
            :income_total, :expenses_total, :net, :cash_balance, :debt_balance
        )",
        rusqlite::named_params! {
            ":team_id": &team.id,
            ":season_number": season_number,
            ":round": round,
            ":category": &team.categoria,
            ":sponsorship_income": context.sponsorship_income,
            ":gate_income": context.gate_income,
            ":result_bonus": context.result_bonus,
            ":partial_prize_income": context.partial_prize_income,
            ":aid_income": context.aid_income,
            ":salary_expense": context.salary_expense,
            ":event_operations_cost": context.event_operations_cost,
            ":structural_maintenance_cost": context.structural_maintenance_cost,
            ":technical_investment_cost": context.technical_investment_cost,
            ":debt_service_cost": context.debt_service_cost,
            ":income_total": summary.income,
            ":expenses_total": summary.expenses,
            ":net": summary.net,
            ":cash_balance": team.cash_balance,
            ":debt_balance": team.debt_balance,
        },
    )?;
    Ok(())
}

/// Últimas `limit` rodadas do histórico financeiro de uma equipe, em ordem cronológica
/// (season_number, round) ASC. Fonte do dossiê financeiro real da aba My Team.
pub fn get_team_finance_history_recent(
    conn: &Connection,
    team_id: &str,
    limit: i64,
) -> Result<Vec<TeamFinanceHistoryEntry>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT season_number, round, category,
                sponsorship_income, result_bonus, partial_prize_income, aid_income,
                salary_expense, event_operations_cost, structural_maintenance_cost,
                technical_investment_cost, debt_service_cost,
                income_total, expenses_total, net, cash_balance, debt_balance,
                constructor_prize_income, gate_income
         FROM team_finance_history
         WHERE team_id = ?1
         ORDER BY season_number DESC, round DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![team_id, limit], |row| {
        Ok(TeamFinanceHistoryEntry {
            season_number: row.get(0)?,
            round: row.get(1)?,
            category: row.get(2)?,
            sponsorship_income: row.get(3)?,
            result_bonus: row.get(4)?,
            partial_prize_income: row.get(5)?,
            aid_income: row.get(6)?,
            salary_expense: row.get(7)?,
            event_operations_cost: row.get(8)?,
            structural_maintenance_cost: row.get(9)?,
            technical_investment_cost: row.get(10)?,
            debt_service_cost: row.get(11)?,
            income_total: row.get(12)?,
            expenses_total: row.get(13)?,
            net: row.get(14)?,
            cash_balance: row.get(15)?,
            debt_balance: row.get(16)?,
            constructor_prize_income: row.get(17)?,
            gate_income: row.get(18)?,
        })
    })?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    // Consultado DESC (para o LIMIT pegar as mais recentes); devolve ASC (cronológico).
    entries.reverse();
    Ok(entries)
}

/// Rodada "sintética" da linha de encerramento (prêmio de construtores). Alta o bastante
/// para nenhuma temporada real alcançar, então ordena SEMPRE depois da última corrida da
/// temporada no gráfico de caixa.
pub const SEASON_CLOSE_ROUND: i32 = 1000;

/// Grava a linha de ENCERRAMENTO da temporada com o prêmio de construtores como receita
/// REAL. `team` deve estar no estado PÓS-prêmio (caixa já creditado). `INSERT OR REPLACE`
/// na chave (team_id, season_number, round = SEASON_CLOSE_ROUND) — idempotente contra
/// reprocessamento do encerramento. As 9 linhas de corrida ficam 0; só o prêmio entra.
pub fn insert_team_finance_season_close(
    conn: &Connection,
    team: &Team,
    season_number: i32,
    prize: f64,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT OR REPLACE INTO team_finance_history (
            team_id, season_number, round, category,
            constructor_prize_income, income_total, expenses_total, net,
            cash_balance, debt_balance
        ) VALUES (
            :team_id, :season_number, :round, :category,
            :prize, :prize, 0.0, :prize, :cash_balance, :debt_balance
        )",
        rusqlite::named_params! {
            ":team_id": &team.id,
            ":season_number": season_number,
            ":round": SEASON_CLOSE_ROUND,
            ":category": &team.categoria,
            ":prize": prize,
            ":cash_balance": team.cash_balance,
            ":debt_balance": team.debt_balance,
        },
    )?;
    Ok(())
}
