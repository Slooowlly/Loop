//! Efeitos financeiros do fim de temporada: prêmio de construtores e o ciclo de
//! colapso → venda das equipes.

use super::*;

/// Credita o prêmio de fim de temporada do campeonato de construtores no caixa
/// de cada equipe, a partir das posições recém-arquivadas em
/// `team_season_archive`, e atualiza o estado financeiro resultante.
pub(super) fn award_constructor_prizes(conn: &Connection, season: &Season) -> Result<(), String> {
    // (team_id, categoria, posição final, nº de equipes no grupo de campeonato).
    // O tamanho do grid é por grupo (categoria + classe), batendo com o
    // agrupamento usado em archive_team_season para categorias multi-classe.
    let mut stmt = conn
        .prepare(
            "SELECT team_id, categoria, posicao_campeonato,
                    COUNT(*) OVER (PARTITION BY categoria, COALESCE(classe, '')) AS grid_size
             FROM team_season_archive
             WHERE season_number = ?1 AND posicao_campeonato IS NOT NULL",
        )
        .map_err(|e| format!("Falha ao preparar consulta de prêmios: {e}"))?;
    let rows = stmt
        .query_map([season.numero], |row| {
            let team_id: String = row.get(0)?;
            let categoria: String = row.get(1)?;
            let position: i32 = row.get(2)?;
            let grid_size: i32 = row.get(3)?;
            Ok((team_id, categoria, position, grid_size))
        })
        .map_err(|e| format!("Falha ao consultar prêmios de construtores: {e}"))?;

    let mut awards: Vec<(String, f64)> = Vec::new();
    for row in rows {
        let (team_id, categoria, position, grid_size) =
            row.map_err(|e| format!("Falha ao mapear prêmio de construtores: {e}"))?;
        let prize = constructor_prize(&categoria, position, grid_size);
        if prize > 0.0 {
            awards.push((team_id, prize));
        }
    }
    drop(stmt);

    for (team_id, prize) in awards {
        let mut team = match team_queries::get_team_by_id(conn, &team_id) {
            Ok(Some(team)) => team,
            Ok(None) => continue,
            Err(e) => return Err(format!("Falha ao carregar equipe {team_id}: {e}")),
        };
        team.cash_balance += prize;
        refresh_team_financial_state(&mut team);
        team_queries::update_team_finance_snapshot(conn, &team)
            .map_err(|e| format!("Falha ao creditar prêmio à equipe {team_id}: {e}"))?;
        // Grava o prêmio como linha de receita REAL de encerramento — é o que o faz
        // aparecer no gráfico de caixa e nos ledgers do dossiê (antes era invisível).
        team_queries::insert_team_finance_season_close(conn, &team, season.numero, prize)
            .map_err(|e| format!("Falha ao gravar linha de prêmio da equipe {team_id}: {e}"))?;
    }

    Ok(())
}

/// Processa o ciclo de colapso financeiro das equipes no fim da temporada:
///   • Em colapso pela 1ª vez (streak 0→1): apenas registra (aviso). A próxima
///     temporada será forçada a all-in (ver preseason::choose).
///   • Em colapso pela 2ª vez seguida (streak →2): a equipe é VENDIDA — nova
///     diretoria quita a dívida, injeta caixa e re-sorteia atributos. Identidade
///     e histórico preservados. Contador zerado.
///   • Fora do colapso: contador zerado (recuperou-se).
pub(super) fn process_collapse_lifecycle(
    conn: &Connection,
    season: &Season,
    rng: &mut impl Rng,
) -> Result<(), String> {
    let teams = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao buscar equipes para ciclo de colapso: {e}"))?;

    for mut team in teams {
        if !team.ativa {
            continue;
        }
        let streak = team_queries::get_collapse_streak(conn, &team.id)
            .map_err(|e| format!("Falha ao ler streak de colapso: {e}"))?;

        if team.financial_state == "collapse" {
            let new_streak = streak + 1;
            if new_streak >= 2 {
                // 2ª temporada consecutiva em colapso (a 2ª já em all-in): venda.
                let outcome = apply_team_sale(&mut team, rng);
                team_queries::update_team(conn, &team)
                    .map_err(|e| format!("Falha ao renovar equipe vendida: {e}"))?;
                team_queries::set_collapse_streak(conn, &team.id, 0)
                    .map_err(|e| format!("Falha ao zerar streak pós-venda: {e}"))?;
                let _ = team_queries::incr_rescue_counter(conn, "sold");
                // Registra o evento de venda/nova diretoria para a ficha da equipe.
                let _ = team_queries::insert_team_ownership_event(
                    conn,
                    &team.id,
                    season.numero,
                    season.ano,
                    "sale",
                    outcome.debt_cleared,
                    outcome.cash_injected,
                    "Nova diretoria assume após colapso financeiro crônico.",
                );
            } else {
                // 1ª temporada em colapso: aviso; all-in virá na próxima.
                team_queries::set_collapse_streak(conn, &team.id, new_streak)
                    .map_err(|e| format!("Falha ao gravar streak de colapso: {e}"))?;
            }
        } else if streak != 0 {
            // Tinha aviso (streak >= 1) e fechou a temporada FORA do colapso:
            // salvou-se sozinha no ano de all-in, sem precisar de venda.
            let _ = team_queries::incr_rescue_counter(conn, "self_rescued");
            team_queries::set_collapse_streak(conn, &team.id, 0)
                .map_err(|e| format!("Falha ao zerar streak de colapso: {e}"))?;
        }
    }

    Ok(())
}
