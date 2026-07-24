//! Ferramentas de depuracao do mercado e da carreira, usadas so pelos comandos de debug e pelos testes.

use super::*;

/// DEBUG: força uma proposta de quebra de contrato pro jogador (Fase 2b.3), pra testar
/// a tela do leilão mesmo num save sem o cenário raro. Relaxa os portões e escolhe o
/// pretendente mais rico da categoria. NÃO exige a janela de mercado — se houver um
/// plano, guarda a oferta nele; senão, só devolve pra UI mostrar. Exige o jogador sob
/// contrato regular (agente livre não é "arrancado").
pub(crate) fn debug_force_player_poach_offer_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<Option<crate::market::pipeline::PlayerPoachOffer>, String> {
    let (db, career_dir, _) = open_career_resources(base_dir, career_id)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Nenhuma temporada ativa.".to_string())?;
    let offer = crate::market::pipeline::debug_build_player_poach_offer(&db.conn, season.numero)?;
    // Se estamos numa janela de mercado, persiste no plano (fluxo real); senão só devolve.
    if let Ok(Some(mut plan)) = load_preseason_plan(&career_dir) {
        plan.player_poach_offer = offer.clone();
        crate::market::preseason::save_preseason_plan(&career_dir, &plan)?;
    }
    Ok(offer)
}

/// DEBUG: prepara o mercado num cenário específico. Simula as corridas restantes (encerra
/// a temporada), torna o jogador AGENTE LIVRE (pra as propostas formais aparecerem) e força
/// a posição final no campeonato (visibilidade → mérito). Cenários: "no_team" (livre, sem
/// forçar posição), "first" (campeão), "fifth" (meio do pelotão). O chamador avança a
/// temporada depois. Só usado pelos controles de debug da UI.
pub(crate) fn debug_prepare_market_scenario_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    scenario: &str,
) -> Result<(), String> {
    skip_all_pending_races_in_base_dir(base_dir, career_id)?;

    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;

    // Todos os cenários entram no mercado como agente livre (as propostas formais só são
    // geradas pra quem está sem contrato nesta fase).
    if let Some(contract) = contract_queries::get_active_contract_for_pilot(&db.conn, &player.id)
        .map_err(|e| format!("Falha ao buscar contrato do jogador: {e}"))?
    {
        contract_queries::update_contract_status(
            &db.conn,
            &contract.id,
            &ContractStatus::Rescindido,
        )
        .map_err(|e| format!("Falha ao rescindir contrato (debug): {e}"))?;
        team_queries::remove_pilot_from_team(&db.conn, &player.id, &contract.equipe_id)
            .map_err(|e| format!("Falha ao remover jogador da equipe (debug): {e}"))?;
    }

    // Força a posição final no campeonato (posicao, pontos, vitorias, podios).
    let forced: Option<(i32, f64, i32)> = match scenario {
        "first" => Some((1, 420.0, 10)),
        "fifth" => Some((5, 180.0, 2)),
        _ => None, // "no_team": não força posição
    };
    if let Some((pos, pts, wins)) = forced {
        // Categoria do "campeão/mediano": usa a atual; se o jogador nunca teve categoria
        // (ex.: save iniciado via "sem time"), assume rookie — assim a promoção mira a Cup.
        let categoria = player
            .categoria_atual
            .clone()
            .unwrap_or_else(|| "mazda_rookie".to_string());

        // Garante categoria_atual no jogador (o tier de mercado deriva dela).
        if player.categoria_atual.is_none() {
            db.conn
                .execute(
                    "UPDATE drivers SET categoria_atual = ?1 WHERE id = ?2",
                    rusqlite::params![categoria, player.id],
                )
                .map_err(|e| format!("Falha ao definir categoria do jogador (debug): {e}"))?;
        }

        // UPSERT do standings: atualiza a linha se existir, senão CRIA — sem isso o
        // UPDATE não afeta nada quando o jogador nunca correu (standings vazio) e o
        // cenário de campeão não surtia efeito (posição/pódio ausentes).
        let updated = db
            .conn
            .execute(
                "UPDATE standings SET posicao = ?1, pontos = ?2, vitorias = ?3, podios = ?4,
                     categoria = ?5
                 WHERE temporada_id = ?6 AND piloto_id = ?7",
                rusqlite::params![pos, pts, wins, wins + 3, categoria, season.id, player.id],
            )
            .map_err(|e| format!("Falha ao forcar classificacao (debug): {e}"))?;
        if updated == 0 {
            db.conn
                .execute(
                    "INSERT INTO standings
                        (temporada_id, piloto_id, categoria, posicao, pontos, vitorias, podios, poles, corridas)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 14)",
                    rusqlite::params![season.id, player.id, categoria, pos, pts, wins, wins + 3],
                )
                .map_err(|e| format!("Falha ao criar classificacao (debug): {e}"))?;
        }
    }

    // DEBUG: um campeão/mediano chega FAMOSO — a fama que ele acumularia na temporada
    // (que o atalho de debug pula ao não correr) é forçada aqui, pra dar pra testar o
    // interesse de mercado do estrelato (Fase 2a). Só no debug; não afeta o jogo real.
    let forced_fama: Option<f64> = match scenario {
        "first" => Some(82.0), // Estrela
        "fifth" => Some(55.0), // Nome forte
        _ => None,             // "no_team": não mexe
    };
    if let Some(fama) = forced_fama {
        db.conn
            .execute(
                "UPDATE drivers SET midia = ?1 WHERE id = ?2",
                rusqlite::params![fama, player.id],
            )
            .map_err(|e| format!("Falha ao forcar fama do jogador (debug): {e}"))?;
    }
    Ok(())
}

/// DEBUG: roda o passe de poaching **de verdade** e desfaz tudo (transação com
/// rollback), devolvendo o raio-x de cada leilão. O leilão só acontece entre IAs e
/// não tem tela até o 2b.3 — isto é a janela pra vê-lo. Não altera o save.
///
/// Se o save, como está, não gera nenhum assédio (ninguém tem caixa pra multa ou
/// não há upgrade claro), roda de novo TEMPERANDO o mundo (`forced`): fama 95 nos
/// melhores pilotos da categoria do jogador + caixa gordo nos times dela. Como tudo
/// é desfeito, é só uma simulação — mas mostra o leilão brigando de verdade.
pub(crate) fn debug_poaching_auctions_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<PoachDebugReport, String> {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let categoria = player
        .categoria_atual
        .clone()
        .unwrap_or_else(|| "gt3".to_string());

    // Passada 1: o mundo como ele está.
    let real = debug_run_poaching_dry(&db, season.numero + 1, None)?;
    if !real.is_empty() {
        return Ok(PoachDebugReport {
            forced: false,
            nota: format!(
                "{} assédio(s) que aconteceriam AGORA, do seu save como está. \
                 Nada foi salvo (a simulação é desfeita).",
                real.len()
            ),
            auctions: real,
        });
    }

    // Passada 2: temperando o mundo, já que o save não tinha briga nenhuma.
    let forced = debug_run_poaching_dry(&db, season.numero + 1, Some(&categoria))?;
    let nota = if forced.is_empty() {
        format!(
            "Nem temperando saiu briga em '{categoria}' — provavelmente não há dois times \
             regulares com as duas vagas cheias na categoria. Nada foi salvo."
        )
    } else {
        format!(
            "Seu save, como está, não gera assédio nenhum agora. Então TEMPEREI o mundo \
             (fama 95 nos melhores de '{categoria}' + caixa gordo nos times de lá) só pra \
             te mostrar o leilão brigando: {} assédio(s). É simulação — nada foi salvo.",
            forced.len()
        )
    };
    Ok(PoachDebugReport {
        forced: true,
        nota,
        auctions: forced,
    })
}

/// Roda o passe numa transação e SEMPRE desfaz. `spice` = categoria a temperar
/// (fama alta + caixa) antes de rodar, ou `None` pra usar o mundo como está.
pub(crate) fn debug_run_poaching_dry(
    db: &Database,
    new_season_number: i32,
    spice: Option<&str>,
) -> Result<Vec<crate::market::pipeline::PoachAudit>, String> {
    let tx = db
        .conn
        .unchecked_transaction()
        .map_err(|e| format!("Falha ao abrir transacao de debug: {e}"))?;

    if let Some(categoria) = spice {
        // Caixa gordo nos times da categoria (pra terem como pagar multa e salário).
        tx.execute(
            "UPDATE teams SET cash_balance = cash_balance + 8000000 WHERE categoria = ?1",
            rusqlite::params![categoria],
        )
        .map_err(|e| format!("Falha ao temperar caixa (debug): {e}"))?;
        // Os 3 melhores pilotos da categoria viram ídolos (fama 95).
        tx.execute(
            "UPDATE drivers SET midia = 95 WHERE id IN (
                 SELECT id FROM drivers
                 WHERE categoria_atual = ?1 AND is_jogador = 0 AND status = 'Ativo'
                 ORDER BY skill DESC LIMIT 3
             )",
            rusqlite::params![categoria],
        )
        .map_err(|e| format!("Falha ao temperar fama (debug): {e}"))?;
    }

    let teams =
        team_queries::get_all_teams(&tx).map_err(|e| format!("Falha ao carregar times: {e}"))?;
    let mut rng = StdRng::seed_from_u64(2026);
    let mut report = crate::market::proposals::MarketReport::default();
    let mut audit = Vec::new();
    let result = crate::market::pipeline::run_poaching_pass(
        &tx,
        &teams,
        new_season_number,
        &mut rng,
        &mut report,
        &mut audit,
    );
    // Desfaz SEMPRE — inclusive se o passe falhou no meio.
    tx.rollback()
        .map_err(|e| format!("Falha ao desfazer simulacao de debug: {e}"))?;
    result?;
    Ok(audit)
}

/// DEBUG: carimba a posição final do jogador no ARQUIVO da temporada mais recente.
/// Roda DEPOIS do avanço (o avanço recalcula standings só de quem correu e exclui o
/// jogador agente livre, gravando posição `null` no arquivo). Sem isso, o cenário de
/// campeão/mediano nunca vira pódio e o mercado não oferta promoção.
pub(crate) fn debug_stamp_player_championship_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    scenario: &str,
) -> Result<(), String> {
    let position: i32 = match scenario {
        "first" => 1,
        "fifth" => 5,
        _ => return Ok(()), // "no_team": não carimba
    };
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;

    // Categoria de exibição: última por contrato, senão rookie.
    let categoria: String = db
        .conn
        .query_row(
            "SELECT categoria FROM contracts WHERE piloto_id = ?1 ORDER BY temporada_fim DESC LIMIT 1",
            rusqlite::params![player.id],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| format!("Falha ao buscar categoria do jogador (debug): {e}"))?
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(|| "mazda_rookie".to_string());

    // Temporada arquivada mais recente do jogador (agregado → sempre 1 linha, valor Option).
    let latest: Option<i32> = db
        .conn
        .query_row(
            "SELECT MAX(season_number) FROM driver_season_archive WHERE piloto_id = ?1",
            rusqlite::params![player.id],
            |r| r.get::<_, Option<i32>>(0),
        )
        .map_err(|e| format!("Falha ao buscar arquivo do jogador (debug): {e}"))?;

    match latest {
        Some(season_number) => {
            db.conn
                .execute(
                    "UPDATE driver_season_archive
                     SET posicao_campeonato = ?1, categoria = ?2
                     WHERE piloto_id = ?3 AND season_number = ?4",
                    rusqlite::params![position, categoria, player.id, season_number],
                )
                .map_err(|e| format!("Falha ao carimbar posicao do jogador (debug): {e}"))?;
        }
        None => {
            // Sem arquivo ainda: cria uma linha pro ano da temporada ativa (edge case).
            let season = season_queries::get_active_season(&db.conn)
                .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
                .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
            db.conn
                .execute(
                    "INSERT INTO driver_season_archive
                        (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '{}')",
                    rusqlite::params![
                        player.id,
                        (season.numero - 1).max(1),
                        season.ano,
                        player.nome,
                        categoria,
                        position,
                        0.0
                    ],
                )
                .map_err(|e| format!("Falha ao criar arquivo do jogador (debug): {e}"))?;
        }
    }
    Ok(())
}
