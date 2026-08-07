//! Gestao da equipe: saude da operacao, caixa/divida, eficiencia por temporada e os
//! eventos de propriedade/diretoria.

use super::*;

/// Carrega os eventos de propriedade/diretoria da equipe (ex.: venda por colapso).
/// Degrada graciosamente para vazio se a tabela ainda não existe (saves antigos
/// abertos somente-leitura antes da migração v36).
pub(super) fn load_team_ownership_events(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<Vec<TeamHistoryOwnershipEvent>, String> {
    let mut stmt = match conn.prepare(
        "SELECT ano, event_type, debt_cleared, cash_injected, detail
         FROM team_ownership_events
         WHERE team_id = ?1
         ORDER BY ano ASC, id ASC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Ok(Vec::new()),
    };
    let rows = stmt
        .query_map(rusqlite::params![team_id], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar eventos de propriedade: {e}"))?;

    let mut events = Vec::new();
    for row in rows {
        let (ano, event_type, debt_cleared, cash_injected, detail) =
            row.map_err(|e| format!("Falha ao mapear evento de propriedade: {e}"))?;
        let title = match event_type.as_str() {
            "sale" => rust_i18n::t!("team_dossier.ownership.sale_title").to_string(),
            "collapse_warning" => {
                rust_i18n::t!("team_dossier.ownership.collapse_warning_title").to_string()
            }
            _ => rust_i18n::t!("team_dossier.ownership.change_title").to_string(),
        };
        // No alerta de insolvência não houve venda: o número que importa é a dívida
        // acumulada. A nota de "dívida zerada · aporte" mentiria dizendo que entrou
        // dinheiro — nesse evento nada entrou, é justamente esse o problema.
        let financial_note = if event_type == "collapse_warning" {
            rust_i18n::t!(
                "team_dossier.ownership.debt_note",
                debt = format_brl(debt_cleared)
            )
            .to_string()
        } else {
            rust_i18n::t!(
                "team_dossier.ownership.financial_note",
                cleared = format_brl(debt_cleared),
                injected = format_brl(cash_injected)
            )
            .to_string()
        };
        events.push(TeamHistoryOwnershipEvent {
            year: ano.to_string(),
            event_type,
            title,
            detail,
            financial_note,
        });
    }
    Ok(events)
}

/// Linhas de receita e de custo de `team_finance_history`, na ordem em que são
/// somadas. O `&str` é o nome da coluna e vira o `id` do DTO — o front usa a mesma
/// chave para achar o rótulo em `myTeamTab.finance.lines.*`.
const INCOME_COLUMNS: [&str; 6] = [
    "sponsorship_income",
    "gate_income",
    "result_bonus",
    "partial_prize_income",
    "constructor_prize_income",
    "aid_income",
];
const EXPENSE_COLUMNS: [&str; 5] = [
    "salary_expense",
    "event_operations_cost",
    "structural_maintenance_cost",
    "technical_investment_cost",
    "debt_service_cost",
];

/// Teto de pontos da curva de caixa. Acima disso a série é amostrada por passo —
/// a curva continua cobrindo a janela INTEIRA (não é uma janela da cauda), só com
/// menos vértices. Uma carreira de ~12 rodadas por temporada cabe inteira até a 8ª.
const CASH_CURVE_MAX_POINTS: usize = 96;

/// Recorte das temporadas com livro-caixa DE VERDADE: as que têm pelo menos uma
/// rodada de CORRIDA gravada. Placeholders `?1` = `team_id`, `?2` =
/// [`team_queries::SEASON_CLOSE_ROUND`].
///
/// É o filtro que separa a carreira jogada do backstory. O sorteio histórico grava
/// UMA linha por ano — só o prêmio de construtores, sem patrocínio, sem salário, sem
/// operação — porque ele não simula economia, só resultados. Ler essas linhas como
/// finanças produz uma equipe que fatura sem gastar: ~26 anos de receita pura, uma
/// rampa monotônica de caixa e um "maior saldo histórico" de dezenas de milhões que
/// nunca existiu — e que o próprio início da carreira apaga, porque
/// `reset_historical_finance_for_playable_start` reescreve o caixa de todo mundo.
const MEASURED_SEASONS_FILTER: &str = "season_number IN (
             SELECT DISTINCT season_number FROM team_finance_history
             WHERE team_id = ?1 AND round < ?2
         )";

/// Livro-caixa agregado da equipe, restrito às temporadas medidas
/// ([`MEASURED_SEASONS_FILTER`]).
///
/// Tudo aqui sai de `team_finance_history`, a tabela que a simulação grava por
/// rodada para toda equipe do grid. Os cards de Gestão liam `teams.cash_balance` —
/// o saldo de HOJE — e o chamavam de "maior saldo histórico"; daqui em diante o
/// superlativo é superlativo de verdade, com a temporada em que aconteceu.
///
/// O recorte é único de propósito: pico, fundo do poço, temporadas no azul, curva e
/// repartição respondem todos sobre a MESMA janela. Antes só a repartição pulava o
/// backstory, e as outras quatro leituras contavam anos que nunca tiveram despesa.
///
/// Save sem nenhuma linha gravada (save antigo) devolve `None`. Carreira que ainda
/// não correu devolve `Some` com a janela ZERADA: é o que faz a aba explicar a
/// ausência em vez de sumir — e os cards caem no retrato atual da equipe.
pub(super) fn load_team_ledger(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<Option<TeamHistoryLedger>, String> {
    let has_history = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM team_finance_history WHERE team_id = ?1)",
            rusqlite::params![team_id],
            |row| row.get::<_, i32>(0),
        )
        .map_err(|e| format!("Falha ao verificar histórico financeiro da equipe: {e}"))?;
    if has_history == 0 {
        return Ok(None);
    }

    let (rounds, seasons, first_season, last_season) = conn
        .query_row(
            &format!(
                "SELECT COUNT(*), COUNT(DISTINCT season_number),
                        COALESCE(MIN(season_number), 0), COALESCE(MAX(season_number), 0)
                 FROM team_finance_history
                 WHERE team_id = ?1 AND {MEASURED_SEASONS_FILTER}"
            ),
            rusqlite::params![team_id, team_queries::SEASON_CLOSE_ROUND],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| format!("Falha ao agregar histórico financeiro da equipe: {e}"))?;
    let rounds: i32 = rounds;

    let flow = load_ledger_flow(conn, team_id)?;

    let (peak_cash, peak_cash_season, peak_cash_round) =
        extreme_row(conn, team_id, "cash_balance")?;
    let (worst_debt, worst_debt_season, worst_debt_round) =
        extreme_row(conn, team_id, "debt_balance")?;

    // Temporada saudável = a que FECHOU sem dívida, olhando a última rodada gravada
    // dela. A regra antiga era um binário sobre o estado atual da equipe aplicado
    // retroativamente a todas as temporadas — uma equipe com caixa cheio e zero
    // dívida marcava 0 só porque o estado do momento não era exatamente "healthy".
    let healthy_seasons = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM team_finance_history h
                 WHERE h.team_id = ?1
                   AND h.debt_balance <= 0.0
                   AND h.{MEASURED_SEASONS_FILTER}
                   AND h.round = (SELECT MAX(i.round) FROM team_finance_history i
                                  WHERE i.team_id = h.team_id
                                    AND i.season_number = h.season_number)"
            ),
            rusqlite::params![team_id, team_queries::SEASON_CLOSE_ROUND],
            |row| row.get::<_, i32>(0),
        )
        .map_err(|e| format!("Falha ao contar temporadas saudáveis: {e}"))?;

    Ok(Some(TeamHistoryLedger {
        seasons,
        rounds,
        first_season,
        last_season,
        peak_cash,
        peak_cash_season,
        peak_cash_round,
        worst_debt,
        worst_debt_season,
        worst_debt_round,
        healthy_seasons,
        flow_seasons: flow.seasons,
        flow_first_season: flow.first_season,
        flow_last_season: flow.last_season,
        flow_note: flow_absence_note(flow.seasons),
        income_total: flow.income_total,
        expenses_total: flow.expenses_total,
        income_lines: flow.income_lines,
        expense_lines: flow.expense_lines,
        cash_curve: load_cash_curve(conn, team_id)?,
    }))
}

/// Frase que explica a ausência da repartição. Vazia quando há temporada medida.
///
/// Só restou uma causa: nenhuma temporada JOGADA. As temporadas anteriores à carreira
/// registram apenas o prêmio de construtores — o sorteio histórico não simula
/// economia — e por isso não têm repartição a mostrar.
///
/// Houve uma segunda causa, que a correção do bloco especial eliminou: Endurance e
/// Production não faturavam nada às próprias equipes, e para elas o gráfico ficaria
/// vazio para sempre. Hoje faturam, então a frase seria mentira.
fn flow_absence_note(flow_seasons: i32) -> String {
    if flow_seasons > 0 {
        return String::new();
    }
    rust_i18n::t!("team_dossier.management.flow_note_unplayed").to_string()
}

struct LedgerFlow {
    seasons: i32,
    first_season: i32,
    last_season: i32,
    income_total: f64,
    expenses_total: f64,
    income_lines: Vec<TeamHistoryLedgerLine>,
    expense_lines: Vec<TeamHistoryLedgerLine>,
}

/// Receita e custo repartidos por linha, no recorte de [`MEASURED_SEASONS_FILTER`].
///
/// A restrição não é conservadorismo, é a única leitura honesta possível: somar o
/// backstory junto com a temporada jogada produzia um gráfico em que 96% da "receita
/// da carreira" era prêmio e a equipe aparecia sem nenhum custo — verdade sobre a
/// tabela, mentira sobre a equipe.
///
/// Uma temporada medida entra INTEIRA, com o fechamento junto, porque o prêmio
/// daquele ano é receita real dela. Nenhuma temporada medida → sem repartição, e a
/// aba omite o gráfico e explica a ausência.
fn load_ledger_flow(conn: &rusqlite::Connection, team_id: &str) -> Result<LedgerFlow, String> {
    let sums: String = INCOME_COLUMNS
        .iter()
        .chain(EXPENSE_COLUMNS.iter())
        .map(|column| format!("COALESCE(SUM({column}), 0)"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT COUNT(DISTINCT season_number),
                COALESCE(MIN(season_number), 0), COALESCE(MAX(season_number), 0),
                COALESCE(SUM(income_total), 0), COALESCE(SUM(expenses_total), 0), {sums}
         FROM team_finance_history
         WHERE team_id = ?1 AND {MEASURED_SEASONS_FILTER}"
    );
    let (seasons, first_season, last_season, income_total, expenses_total, values) = conn
        .query_row(
            &sql,
            rusqlite::params![team_id, team_queries::SEASON_CLOSE_ROUND],
            |row| {
                let mut values = Vec::with_capacity(INCOME_COLUMNS.len() + EXPENSE_COLUMNS.len());
                for index in 0..(INCOME_COLUMNS.len() + EXPENSE_COLUMNS.len()) {
                    values.push(row.get::<_, f64>(5 + index)?);
                }
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, i32>(2)?,
                    row.get::<_, f64>(3)?,
                    row.get::<_, f64>(4)?,
                    values,
                ))
            },
        )
        .map_err(|e| format!("Falha ao agregar fluxo financeiro da equipe: {e}"))?;

    let (income_values, expense_values) = values.split_at(INCOME_COLUMNS.len());
    Ok(LedgerFlow {
        seasons,
        first_season,
        last_season,
        income_total,
        expenses_total,
        income_lines: ledger_lines(&INCOME_COLUMNS, income_values),
        expense_lines: ledger_lines(&EXPENSE_COLUMNS, expense_values),
    })
}

/// Reparte um lado do livro-caixa em linhas ordenadas da maior para a menor. Linha
/// zerada some: um "Auxílios $0" no gráfico só ocupa espaço para dizer que não
/// aconteceu.
///
/// A FATIA fica com o front de propósito: o denominador é escolha de exibição — o
/// Sankey mostra cada linha contra a receita total, não contra o seu próprio lado —
/// e um percentual pré-calculado aqui significaria outra coisa que a tela mostra.
fn ledger_lines(columns: &[&str], values: &[f64]) -> Vec<TeamHistoryLedgerLine> {
    let mut lines: Vec<TeamHistoryLedgerLine> = columns
        .iter()
        .zip(values.iter())
        .filter(|(_, value)| **value > 0.0)
        .map(|(id, value)| TeamHistoryLedgerLine {
            id: (*id).to_string(),
            value: *value,
        })
        .collect();
    lines.sort_by(|left, right| {
        right
            .value
            .total_cmp(&left.value)
            .then_with(|| left.id.cmp(&right.id))
    });
    lines
}

/// Rodada em que uma coluna de saldo bateu o máximo, com temporada e rodada. Os
/// empates caem na rodada mais antiga: o pico é quando ele foi ALCANÇADO.
///
/// Sem temporada medida não há extremo: devolve zeros, e quem chama trata isso como
/// "não há superlativo a contar" (ver [`build_real_team_management`]).
fn extreme_row(
    conn: &rusqlite::Connection,
    team_id: &str,
    column: &str,
) -> Result<(f64, i32, i32), String> {
    let sql = format!(
        "SELECT {column}, season_number, round FROM team_finance_history
         WHERE team_id = ?1 AND {MEASURED_SEASONS_FILTER}
         ORDER BY {column} DESC, season_number ASC, round ASC
         LIMIT 1"
    );
    conn.query_row(
        &sql,
        rusqlite::params![team_id, team_queries::SEASON_CLOSE_ROUND],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .or_else(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => Ok((0.0, 0, 0)),
        other => Err(format!("Falha ao localizar extremo de {column}: {other}")),
    })
}

/// Curva de caixa das temporadas medidas ([`MEASURED_SEASONS_FILTER`]), amostrada
/// para caber em [`CASH_CURVE_MAX_POINTS`]. Quando o passo é maior que 1, a janela
/// prefere a rodada de encerramento se houver uma: o degrau do prêmio de
/// construtores é o acidente mais informativo do desenho e é justamente o que uma
/// amostragem ingênua descartaria. A última rodada entra sempre — a curva tem que
/// terminar no presente.
///
/// O backstory fica de fora. Com ele a curva desenhava ~26 anos de subida
/// monotônica seguidos de um penhasco no primeiro ano jogado — e o penhasco não era
/// uma quebra da equipe, era o reset de caixa do início da carreira. Duas leis
/// financeiras diferentes no mesmo traço, sem nada dizendo onde uma vira a outra.
fn load_cash_curve(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<Vec<TeamHistoryCashPoint>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT season_number, round, cash_balance, debt_balance, constructor_prize_income
             FROM team_finance_history
             WHERE team_id = ?1 AND {MEASURED_SEASONS_FILTER}
             ORDER BY season_number ASC, round ASC"
        ))
        .map_err(|e| format!("Falha ao preparar curva de caixa: {e}"))?;
    let rows = stmt
        .query_map(
            rusqlite::params![team_id, team_queries::SEASON_CLOSE_ROUND],
            |row| {
                Ok(TeamHistoryCashPoint {
                    season_number: row.get(0)?,
                    round: row.get(1)?,
                    cash_balance: row.get(2)?,
                    debt_balance: row.get(3)?,
                    is_season_close: row.get::<_, f64>(4)? > 0.0,
                })
            },
        )
        .map_err(|e| format!("Falha ao consultar curva de caixa: {e}"))?;
    let mut all = Vec::new();
    for row in rows {
        all.push(row.map_err(|e| format!("Falha ao mapear curva de caixa: {e}"))?);
    }
    if all.len() <= CASH_CURVE_MAX_POINTS {
        return Ok(all);
    }

    let stride = all.len().div_ceil(CASH_CURVE_MAX_POINTS);
    let mut sampled: Vec<TeamHistoryCashPoint> = all
        .chunks(stride)
        .map(|window| {
            window
                .iter()
                .find(|point| point.is_season_close)
                .unwrap_or(&window[0])
                .clone()
        })
        .collect();
    if let Some(last) = all.last() {
        let tail_is_last = sampled.last().is_some_and(|point| {
            point.season_number == last.season_number && point.round == last.round
        });
        if !tail_is_last {
            sampled.push(last.clone());
        }
    }
    Ok(sampled)
}

pub(super) fn build_real_team_management(
    conn: &rusqlite::Connection,
    team_id: &str,
    facts: &[TeamRaceFact],
) -> Result<TeamHistoryManagement, String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar gestão histórica da equipe: {e}"))?
        .ok_or_else(|| format!("Equipe '{team_id}' não encontrada para gestão histórica"))?;
    let ledger = load_team_ledger(conn, team_id)?;
    let cash = team.cash_balance.max(0.0);
    let debt = team.debt_balance.max(0.0);
    let points: f64 = facts.iter().map(|fact| fact.points).sum();
    let seasons = distinct_seasons(facts);
    let seasons_count = seasons.len().max(1) as f64;
    let points_per_season = points / seasons_count;
    // Pico, fundo do poço e temporadas no azul vêm do LIVRO-CAIXA, não do saldo de
    // hoje. Sem temporada medida o retrato atual é tudo que existe — e aí os
    // detalhes abaixo dizem isso em vez de fingir que é a série completa.
    //
    // `medido` é o livro-caixa só quando ele tem temporada JOGADA. Um livro que só
    // enxerga backstory não tem superlativo a contar: o pico dele seria caixa que a
    // simulação nunca cobrou despesa nenhuma para formar.
    let medido = ledger.as_ref().filter(|l| l.rounds > 0);
    let peak_cash = medido.map(|l| l.peak_cash).unwrap_or(cash);
    let worst_debt = medido.map(|l| l.worst_debt).unwrap_or(debt);
    let healthy_years = match (medido, ledger.as_ref()) {
        (Some(l), _) => l.healthy_seasons,
        // Há histórico, mas nenhuma temporada com economia medida: zero anos
        // AFERIDOS, não zero anos saudáveis — é o que o detalhe genérico diz.
        (None, Some(_)) => 0,
        // Save sem nenhuma linha financeira: sobra a heurística do estado de hoje.
        (None, None) => {
            if debt <= 0.0 {
                seasons.len() as i32
            } else {
                0
            }
        }
    };
    let state_label = financial_state_label_for_dossier(&team.financial_state);
    // "Nível do pacote" aqui é o Nível do Carro (1–10) — a MESMA leitura de carro que o
    // jogador vê no ranking e na aba da equipe. Antes era o escalar legado arredondado numa
    // faixa 0–16: outro número, outra escala, e ainda por cima cego ao sistema de peças.
    let technical_level = team
        .car
        .as_ref()
        .map(|car| car.display_level())
        .unwrap_or(1) as i32;

    let efficiency_value = format_decimal_pt(points_per_season, 1);
    let points_int = points.round() as i32;
    // Os detalhes carregam a DATA do superlativo. Sem ela o card volta a ser um
    // número solto que o jogador não sabe se aconteceu ano passado ou na fundação.
    let peak_cash_detail = match medido {
        Some(l) => rust_i18n::t!(
            "team_dossier.management.peak_cash_detail_season",
            season = l.peak_cash_season,
            round = l.peak_cash_round
        )
        .to_string(),
        None => rust_i18n::t!("team_dossier.management.peak_cash_detail").to_string(),
    };
    let worst_crisis_detail = match medido {
        Some(l) if l.worst_debt > 0.0 => rust_i18n::t!(
            "team_dossier.management.worst_crisis_detail_season",
            season = l.worst_debt_season
        )
        .to_string(),
        Some(l) if l.rounds == 1 => {
            rust_i18n::t!("team_dossier.management.worst_crisis_detail_clean_one").to_string()
        }
        Some(l) => rust_i18n::t!(
            "team_dossier.management.worst_crisis_detail_clean_other",
            count = l.rounds
        )
        .to_string(),
        None if debt > 0.0 => {
            rust_i18n::t!("team_dossier.management.worst_crisis_detail_debt").to_string()
        }
        None => rust_i18n::t!("team_dossier.management.worst_crisis_detail_none").to_string(),
    };
    // Com uma temporada só, "1 de 1 temporadas fecharam sem dívida" é uma fração que
    // não informa nada e ainda erra a concordância. A frase singular diz o mesmo em
    // linguagem de gente — e o desfecho vem da própria contagem.
    let healthy_years_detail = match medido {
        Some(l) if l.seasons == 1 => {
            let outcome = if l.healthy_seasons == 1 {
                rust_i18n::t!("team_dossier.management.healthy_years_outcome_clean")
            } else {
                rust_i18n::t!("team_dossier.management.healthy_years_outcome_debt")
            };
            rust_i18n::t!(
                "team_dossier.management.healthy_years_detail_season_one",
                outcome = outcome
            )
            .to_string()
        }
        Some(l) => rust_i18n::t!(
            "team_dossier.management.healthy_years_detail_season_other",
            healthy = l.healthy_seasons,
            total = l.seasons
        )
        .to_string(),
        None => rust_i18n::t!("team_dossier.management.healthy_years_detail").to_string(),
    };
    Ok(TeamHistoryManagement {
        operation_health: state_label.clone(),
        peak_cash: format_brl(peak_cash),
        worst_crisis: if worst_debt > 0.0 {
            rust_i18n::t!(
                "team_dossier.management.worst_crisis_debt",
                debt = format_brl(worst_debt)
            )
            .to_string()
        } else {
            rust_i18n::t!("team_dossier.management.worst_crisis_none").to_string()
        },
        healthy_years: if healthy_years == 1 {
            rust_i18n::t!("team_dossier.management.healthy_years_one").to_string()
        } else {
            rust_i18n::t!(
                "team_dossier.management.healthy_years_other",
                count = healthy_years
            )
            .to_string()
        },
        efficiency: rust_i18n::t!(
            "team_dossier.management.efficiency",
            value = efficiency_value.as_str()
        )
        .to_string(),
        biggest_investment: rust_i18n::t!(
            "team_dossier.management.biggest_investment",
            level = technical_level
        )
        .to_string(),
        summary: rust_i18n::t!(
            "team_dossier.management.summary",
            state = state_label.as_str(),
            cash = format_brl(cash),
            debt = format_brl(debt),
            points = points_int
        )
        .to_string(),
        peak_cash_detail,
        worst_crisis_detail,
        healthy_years_detail,
        efficiency_detail: rust_i18n::t!(
            "team_dossier.management.efficiency_detail",
            points = points_int,
            avg = efficiency_value.as_str()
        )
        .to_string(),
        investment_detail: rust_i18n::t!("team_dossier.management.investment_detail").to_string(),
        ledger,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sem temporada medida o bloco EXPLICA em vez de sumir — sumir não distingue
    /// "não há o que mostrar" de "quebrou". Com temporada medida a nota some, porque
    /// aí o gráfico fala por si.
    ///
    /// A nota não menciona mais o bloco especial: desde que Endurance e Production
    /// passaram a faturar as próprias equipes, a única causa restante é a carreira
    /// que ainda não correu.
    #[test]
    #[serial_test::serial]
    fn ausencia_de_fluxo_explica_a_carreira_sem_temporada_jogada() {
        rust_i18n::set_locale("pt-BR");
        assert!(flow_absence_note(0).contains("Nenhuma temporada jogada"));
        assert!(flow_absence_note(2).is_empty());
    }
}

fn financial_state_label_for_dossier(state: &str) -> String {
    let key = match state {
        "dominant" | "healthy" => "healthy",
        "stable" => "stable",
        "pressured" => "pressured",
        "critical" => "critical",
        _ => "monitored",
    };
    let full = format!("team_dossier.state.{key}");
    rust_i18n::t!(&full).to_string()
}
