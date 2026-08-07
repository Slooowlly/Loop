//! Efeitos financeiros do fim de temporada: prêmio de construtores e o ciclo de
//! colapso → venda das equipes.

use super::*;

use crate::news::{NewsImportance, NewsItem, NewsType};

/// Ícone próprio da falência. O `NewsType` classifica (é assunto de mercado/gestão);
/// o ícone conta o que aconteceu — quebrar não é uma transferência qualquer.
const ICONE_FALENCIA: &str = "🏦";

/// Rótulo da categoria para a notícia, com recuo para o id cru quando o catálogo
/// não conhece a categoria (save meio migrado).
fn rotulo_de_categoria(categoria: &str) -> String {
    get_category_config(categoria)
        .map(|c| c.nome_curto.to_string())
        .unwrap_or_else(|| categoria.to_string())
}

/// Dinheiro em milhões, como a prosa das notícias usa.
fn em_milhoes(valor: f64) -> String {
    format!("{:.1} M", valor / 1_000_000.0)
}

/// Publica uma notícia da falência. Best-effort: a quebra já aconteceu no banco;
/// falhar ao publicar não pode derrubar a virada de temporada inteira.
fn publica_noticia_de_falencia(
    conn: &Connection,
    team: &Team,
    season: &Season,
    id_sufixo: &str,
    titulo: String,
    texto: String,
    importancia: NewsImportance,
) {
    let item = NewsItem {
        id: format!("NEWS-FAL-{}-{}-{}", season.numero, team.id, id_sufixo),
        tipo: NewsType::Mercado,
        icone: ICONE_FALENCIA.to_string(),
        titulo,
        texto,
        rodada: None,
        semana_pretemporada: None,
        temporada: season.numero,
        categoria_id: Some(team.categoria.clone()),
        categoria_nome: Some(rotulo_de_categoria(&team.categoria)),
        importancia,
        timestamp: chrono::Local::now().timestamp(),
        driver_id: None,
        driver_id_secondary: None,
        team_id: Some(team.id.clone()),
    };
    if let Err(e) = crate::db::queries::news::insert_news(conn, &item) {
        eprintln!("[news] Falha ao publicar falência de {}: {e:?}", team.id);
    }
}

/// A nova diretoria corta a folha: NÃO RENOVA o contrato mais caro da equipe vendida.
///
/// É o que faz a falência ser sentida no assento em vez de no balanço — quem fica
/// perde o companheiro de equipe. O JOGADOR nunca é cortado: tirar o assento dele
/// por evento do mundo seria decidir a carreira por ele, e a saída dele é uma
/// ESCOLHA (a proposta de fuga, em `market::pipeline::jogador`).
///
/// **Encurta a vigência em vez de rescindir na hora**, e isso não é detalhe. Este
/// código roda no fim da temporada, quando praticamente não existe agente livre —
/// esvaziar o assento aqui deixaria a equipe com um carro sem piloto que o mundo
/// fechado não tem como preencher (medido: 2 grids de gt3 incompletos no harness
/// histórico). Com `temporada_fim` na temporada que acaba, o contrato expira pela
/// via normal em `expire_ending_contracts`, o piloto entra no mercado da
/// pré-temporada junto com todos os outros que terminaram vínculo, e é o leilão —
/// que já é o dono da regra de grid cheio — quem repõe o assento.
///
/// Devolve o nome de quem não será renovado, se houve.
fn corta_a_folha(
    conn: &Connection,
    team: &Team,
    season: &Season,
) -> Result<Option<String>, String> {
    let contratos = contract_queries::get_active_regular_contracts_by_team(conn, &team.id)
        .map_err(|e| format!("Falha ao ler contratos da equipe vendida: {e}"))?;
    // Uma equipe com um piloto só não fica sem ninguém — não há folha a cortar.
    if contratos.len() < 2 {
        return Ok(None);
    }

    let mut candidatos: Vec<_> = contratos
        .into_iter()
        // Contrato que já termina nesta temporada não é corte nenhum: o mercado já
        // ia decidir sobre ele. Cortar aí seria anunciar uma consequência que não
        // existe.
        .filter(|c| c.temporada_fim > season.numero)
        .filter(|c| {
            driver_queries::get_driver(conn, &c.piloto_id)
                .map(|d| !d.is_jogador)
                .unwrap_or(true)
        })
        .collect();
    if candidatos.is_empty() {
        return Ok(None);
    }
    // O mais caro primeiro: é a folha que a diretoria não consegue pagar.
    candidatos.sort_by(|a, b| {
        b.salario_anual
            .total_cmp(&a.salario_anual)
            .then_with(|| a.piloto_id.cmp(&b.piloto_id))
    });
    let alvo = &candidatos[0];

    conn.execute(
        "UPDATE contracts SET temporada_fim = ?1 WHERE id = ?2",
        rusqlite::params![season.numero, alvo.id],
    )
    .map_err(|e| format!("Falha ao encurtar contrato na venda: {e}"))?;
    Ok(Some(alvo.piloto_nome.clone()))
}

/// A equipe é a do JOGADOR nesta temporada?
fn e_a_equipe_do_jogador(conn: &Connection, team_id: &str) -> bool {
    let Ok(player) = driver_queries::get_player_driver(conn) else {
        return false;
    };
    contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .ok()
        .flatten()
        .map(|c| c.equipe_id == team_id)
        .unwrap_or(false)
}

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
                // Descoberto ANTES de mexer no elenco — depois do corte de folha o
                // contrato do dispensado já não serve para responder a pergunta.
                let e_do_jogador = e_a_equipe_do_jogador(conn, &team.id);

                let outcome = apply_team_sale(&mut team, rng);
                team_queries::update_team(conn, &team)
                    .map_err(|e| format!("Falha ao renovar equipe vendida: {e}"))?;
                team_queries::set_collapse_streak(conn, &team.id, 0)
                    .map_err(|e| format!("Falha ao zerar streak pós-venda: {e}"))?;
                let _ = team_queries::incr_rescue_counter(conn, "sold");

                // A nova diretoria corta a folha: o piloto mais caro (nunca o jogador)
                // é dispensado e cai no mercado da pré-temporada, que roda adiante.
                let dispensado = corta_a_folha(conn, &team, season)?;

                // Registra o evento de venda/nova diretoria para a ficha da equipe.
                let _ = team_queries::insert_team_ownership_event(
                    conn,
                    &team.id,
                    season.numero,
                    season.ano,
                    "sale",
                    outcome.debt_cleared,
                    outcome.cash_injected,
                    &rust_i18n::t!("team_dossier.ownership.sale_detail"),
                );

                // ── A falência vira EVENTO VISÍVEL ──
                let categoria = rotulo_de_categoria(&team.categoria);
                let cleared = em_milhoes(outcome.debt_cleared);
                let (titulo, texto) = match &dispensado {
                    Some(nome) => (
                        rust_i18n::t!("bankruptcy.sale_title", team = team.nome.as_str()),
                        rust_i18n::t!(
                            "bankruptcy.sale_text_released",
                            team = team.nome.as_str(),
                            cleared = cleared.as_str(),
                            driver = nome.as_str(),
                            category = categoria.as_str()
                        ),
                    ),
                    None => (
                        rust_i18n::t!("bankruptcy.sale_title", team = team.nome.as_str()),
                        rust_i18n::t!(
                            "bankruptcy.sale_text",
                            team = team.nome.as_str(),
                            cleared = cleared.as_str(),
                            category = categoria.as_str()
                        ),
                    ),
                };
                publica_noticia_de_falencia(
                    conn,
                    &team,
                    season,
                    "sale",
                    titulo.to_string(),
                    texto.to_string(),
                    NewsImportance::Alta,
                );

                // Se for a equipe DO JOGADOR, a manchete é dele — e em Destaque. A
                // saída (proposta de outra equipe) é montada pelo mercado da
                // pré-temporada, que roda mais adiante nesta mesma virada.
                if e_do_jogador {
                    publica_noticia_de_falencia(
                        conn,
                        &team,
                        season,
                        "player",
                        rust_i18n::t!("bankruptcy.player_team_title").to_string(),
                        rust_i18n::t!("bankruptcy.player_team_text", team = team.nome.as_str())
                            .to_string(),
                        NewsImportance::Destaque,
                    );
                }
            } else {
                // 1ª temporada em colapso: aviso; all-in virá na próxima.
                team_queries::set_collapse_streak(conn, &team.id, new_streak)
                    .map_err(|e| format!("Falha ao gravar streak de colapso: {e}"))?;

                // O aviso também é público: sem ele a venda do ano seguinte chega
                // do nada. Registro na ficha + notícia, os dois tempos da história.
                let _ = team_queries::insert_team_ownership_event(
                    conn,
                    &team.id,
                    season.numero,
                    season.ano,
                    "collapse_warning",
                    team.debt_balance.max(0.0),
                    0.0,
                    &rust_i18n::t!("team_dossier.ownership.collapse_warning_detail"),
                );
                publica_noticia_de_falencia(
                    conn,
                    &team,
                    season,
                    "warning",
                    rust_i18n::t!("bankruptcy.warning_title", team = team.nome.as_str())
                        .to_string(),
                    rust_i18n::t!(
                        "bankruptcy.warning_text",
                        team = team.nome.as_str(),
                        season = season.numero,
                        category = rotulo_de_categoria(&team.categoria).as_str()
                    )
                    .to_string(),
                    NewsImportance::Media,
                );
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
