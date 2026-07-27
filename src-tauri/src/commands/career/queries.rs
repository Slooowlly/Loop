//! Consultas de leitura da carreira: pilotos, equipes, calendario, resultados e
//! noticias, com os helpers de resolucao de contrato/equipe que elas compartilham.

use super::*;

/// Dossiê de habilidade do JOGADOR: atributos inferidos do desempenho REAL na
/// pista (só visual — o mercado NÃO consulta). Reconstrói o grid de cada corrida
/// e roda o estimador puro (ver `crate::player_skill` e o spec de 2026-07-12).
pub(crate) fn get_player_dossier_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<crate::player_skill::PlayerDossier, String> {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let samples = crate::db::queries::race_history::get_player_race_samples(&db.conn, &player.id)
        .map_err(|e| format!("Falha ao reconstruir histórico do jogador: {e}"))?;

    Ok(crate::player_skill::build_dossier(
        &samples,
        player.atributos.midia,
    ))
}

pub(crate) fn get_driver_in_base_dir(
    base_dir: &Path,
    career_number: u32,
    driver_id: &str,
) -> Result<Driver, String> {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.career_db_path(career_number);
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    driver_queries::get_driver(&db.conn, driver_id)
        .map_err(|e| format!("Falha ao buscar piloto: {e}"))
}

pub(crate) fn get_news_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    season: Option<i32>,
    tipo: Option<&str>,
    limit: Option<i32>,
) -> Result<Vec<NewsItem>, String> {
    let (db, _career_dir, _meta) = open_career_resources(base_dir, career_id)?;
    let max_items = limit.unwrap_or(50).clamp(1, 400);
    let query_limit = if tipo.is_some() { 400 } else { max_items };
    let mut items = match season {
        Some(season_number) => {
            news_queries::get_news_by_season(&db.conn, season_number, query_limit)
                .map_err(|e| format!("Falha ao buscar noticias por temporada: {e}"))?
        }
        None => news_queries::get_recent_news(&db.conn, query_limit)
            .map_err(|e| format!("Falha ao buscar noticias recentes: {e}"))?,
    };

    if let Some(tipo) = tipo {
        let tipo_normalizado = NewsType::from_str_strict(tipo)
            .map_err(|e| format!("Filtro de noticia invalido: {e}"))?;
        items.retain(|item| item.tipo == tipo_normalizado);
    }

    items.truncate(max_items as usize);
    Ok(items)
}

pub(crate) fn get_driver_detail_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    driver_id: &str,
) -> Result<DriverDetail, String> {
    let (db, career_dir, _) = open_career_resources_read_only(base_dir, career_id)?;
    let driver = driver_queries::get_driver(&db.conn, driver_id)
        .map_err(|e| format!("Falha ao buscar piloto: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let contract = preferred_active_contract_for_phase(&db.conn, driver_id, season.fase)?;
    let team = resolve_driver_team(&db.conn, driver_id, contract.as_ref())?;
    let role = resolve_driver_role(driver_id, contract.as_ref(), team.as_ref());

    build_driver_detail_payload(
        &db.conn,
        &career_dir,
        &season,
        &driver,
        contract.as_ref(),
        team.as_ref(),
        role,
    )
}

/// Inverte o favorito do piloto (watchlist) e devolve o NOVO estado (true = agora
/// favoritado). Puramente cosmético — alimenta a ênfase do feed do mercado e o filtro
/// "Favoritos" na aba de pilotos; não toca na simulação.
pub(crate) fn toggle_driver_favorite_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    driver_id: &str,
) -> Result<bool, String> {
    let (db, _, _) = open_career_resources_read_only(base_dir, career_id)?;
    crate::db::queries::favorites::toggle_favorite(&db.conn, driver_id)
        .map_err(|e| format!("Falha ao alternar favorito: {e}"))
}

// Melhor posição de chegada (ignorando DNF) usada como desempate de classificação.
// Quem não terminou nenhuma corrida fica com o pior valor possível, então cai para
// o fim do grupo empatado em vez de subir por não ter resultado.
pub(crate) fn best_finish_position(results: &[Option<RoundResult>]) -> i32 {
    results
        .iter()
        .filter_map(|result| result.as_ref())
        .filter(|result| !result.is_dnf)
        .map(|result| result.position)
        .min()
        .unwrap_or(i32::MAX)
}

pub(crate) fn get_drivers_by_category_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    category: &str,
) -> Result<Vec<DriverSummary>, String> {
    let category = category.trim().to_lowercase();
    let (db, career_dir, _) =
        open_career_resources_for_category_read(base_dir, career_id, &category)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let total_rounds = count_calendar_entries(&db.conn, &season.id, &category)
        .map_err(|e| format!("Falha ao contar corridas da categoria: {e}"))?
        as usize;

    if categories::is_multiclass_category(&category) {
        let special_standings = get_special_driver_standings_from_results(
            &db,
            &career_dir,
            &season,
            &category,
            total_rounds,
        )?;
        if !special_standings.is_empty() {
            return Ok(special_standings);
        }
    }

    let mut drivers = driver_queries::get_drivers_by_category(&db.conn, &category)
        .map_err(|e| format!("Falha ao buscar pilotos da categoria: {e}"))?;
    let participant_ids = get_regular_standings_participant_ids(&db.conn, &season.id, &category)?;
    if !participant_ids.is_empty() {
        drivers.retain(|driver| participant_ids.contains(&driver.id));
    }
    let driver_ids: Vec<String> = drivers.iter().map(|driver| driver.id.clone()).collect();
    let active_injuries_by_driver =
        injury_queries::get_active_injury_types_by_pilot(&db.conn, &driver_ids)
            .map_err(|e| format!("Falha ao buscar lesoes ativas dos pilotos: {e}"))?;
    let history_map: HashMap<String, Vec<Option<RoundResult>>> =
        build_driver_histories(&career_dir, &category, total_rounds, &driver_ids)?
            .into_iter()
            .map(|history| (history.driver_id, history.results))
            .collect();

    let mut standings: Vec<DriverSummary> = drivers
        .into_iter()
        .map(|driver| {
            let driver_id = driver.id.clone();
            let team = find_player_team(&db.conn, &driver.id, season.fase)
                .ok()
                .flatten();
            DriverSummary {
                id: driver_id.clone(),
                nome: driver.nome,
                nacionalidade: driver.nacionalidade,
                idade: driver.idade as i32,
                skill: driver.atributos.skill.round().clamp(0.0, 100.0) as u8,
                midia: driver.atributos.midia.round().clamp(0.0, 100.0) as u8,
                categoria_especial_ativa: driver.categoria_especial_ativa.clone(),
                equipe_id: team.as_ref().map(|value| value.id.clone()),
                equipe_nome: team.as_ref().map(|value| value.nome.clone()),
                equipe_nome_curto: team.as_ref().map(|value| value.nome_curto.clone()),
                equipe_cor: team
                    .as_ref()
                    .map(|value| value.cor_primaria.clone())
                    .unwrap_or_else(|| "#7d8590".to_string()),
                classe: team.as_ref().and_then(|value| value.classe.clone()),
                is_jogador: driver.is_jogador,
                is_estreante: driver.temporadas_na_categoria == 0,
                is_estreante_da_vida: driver.stats_carreira.corridas == 0,
                lesao_ativa_tipo: active_injuries_by_driver.get(&driver_id).cloned(),
                is_aposentado: driver.status == crate::models::enums::DriverStatus::Aposentado,
                pontos: driver.stats_temporada.pontos.round() as i32,
                vitorias: driver.stats_temporada.vitorias as i32,
                podios: driver.stats_temporada.podios as i32,
                posicao_campeonato: 0,
                results: merge_recent_results_fallback(
                    history_map.get(&driver_id).cloned().unwrap_or_default(),
                    &driver.ultimos_resultados,
                    total_rounds,
                    driver.stats_temporada.corridas as usize,
                ),
            }
        })
        .collect();

    standings.sort_by(|a, b| {
        b.pontos
            .cmp(&a.pontos)
            .then_with(|| b.vitorias.cmp(&a.vitorias))
            .then_with(|| b.podios.cmp(&a.podios))
            // Desempate por melhor chegada na pista: sem isso, pilotos empatados
            // (tipicamente todo o pelotão de 0 ponto) caíam direto no nome, então
            // o 20º podia aparecer atrás do 26º. Menor posição = melhor.
            .then_with(|| best_finish_position(&a.results).cmp(&best_finish_position(&b.results)))
            .then_with(|| a.nome.cmp(&b.nome))
    });

    for (index, driver) in standings.iter_mut().enumerate() {
        driver.posicao_campeonato = index as i32 + 1;
    }

    Ok(standings)
}

pub(crate) fn get_driver_slot_info(
    db: &Database,
    driver_id: Option<&String>,
    team_id: &str,
    active_season_number: i32,
) -> (Option<String>, Option<i32>) {
    let Some(driver_id) = driver_id else {
        return (None, None);
    };

    let driver_name = driver_queries::get_driver(&db.conn, driver_id)
        .ok()
        .map(|driver| driver.nome);
    let tenure_seasons =
        calculate_consecutive_team_tenure(&db.conn, driver_id, team_id, active_season_number);
    (driver_name, tenure_seasons)
}

pub(crate) fn calculate_consecutive_team_tenure(
    conn: &rusqlite::Connection,
    driver_id: &str,
    team_id: &str,
    active_season_number: i32,
) -> Option<i32> {
    let contracts = contract_queries::get_contracts_for_pilot(conn, driver_id).ok()?;
    consecutive_team_seasons_up_to(&contracts, team_id, active_season_number)
}

pub(crate) fn consecutive_team_seasons_up_to(
    contracts: &[crate::models::contract::Contract],
    team_id: &str,
    active_season_number: i32,
) -> Option<i32> {
    let mut intervals: Vec<(i32, i32)> = contracts
        .iter()
        .filter(|contract| {
            contract.tipo == crate::models::enums::ContractType::Regular
                && contract.equipe_id == team_id
                && contract.status != crate::models::enums::ContractStatus::Pendente
        })
        .map(|contract| {
            (
                contract.temporada_inicio,
                contract.temporada_fim.min(active_season_number),
            )
        })
        .filter(|(start, end)| *start <= *end)
        .collect();

    intervals.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));

    let mut covered_until = active_season_number;
    let mut earliest_start = None;

    for (start, end) in intervals {
        if end < covered_until {
            if end + 1 != covered_until {
                continue;
            }
        } else if start > covered_until || end < covered_until {
            continue;
        }

        earliest_start = Some(start);
        covered_until = start - 1;
    }

    earliest_start.map(|start| active_season_number - start + 1)
}

pub(crate) fn team_founded_year_for_payload(team: &Team) -> i32 {
    if team.ano_fundacao > 1800 {
        return team.ano_fundacao;
    }

    let rank_index = team.meta_posicao.saturating_sub(1).max(0) as usize;
    historical_team_foundation_year(&team.nome, &team.categoria, rank_index, 10)
}

pub(crate) fn get_race_results_by_category_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    category: &str,
) -> Result<Vec<DriverRaceHistory>, String> {
    let category = category.trim().to_lowercase();
    let (db, career_dir, _) = open_career_resources_read_only(base_dir, career_id)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let drivers = driver_queries::get_drivers_by_category(&db.conn, &category)
        .map_err(|e| format!("Falha ao buscar pilotos da categoria: {e}"))?;
    let total_rounds = count_calendar_entries(&db.conn, &season.id, &category)
        .map_err(|e| format!("Falha ao contar corridas da categoria: {e}"))?
        as usize;
    let driver_ids: Vec<String> = drivers.into_iter().map(|driver| driver.id).collect();

    build_driver_histories(&career_dir, &category, total_rounds, &driver_ids)
}

/// Campeões da temporada passada nesta categoria. Alimenta o selo de campeão
/// reinante na classificação e o "trono vago" da prévia de temporada — ambos
/// ficavam mudos porque esta função devolvia vazio em qualquer temporada.
pub(crate) fn get_previous_champions_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    category: &str,
) -> Result<PreviousChampions, String> {
    let category = category.trim().to_lowercase();
    let (db, _, _) = open_career_resources_read_only(base_dir, career_id)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

    if season.numero <= 1 {
        return Ok(empty_previous_champions());
    }

    let seasons = season_queries::get_all_seasons(&db.conn)
        .map_err(|e| format!("Falha ao listar temporadas: {e}"))?;
    let Some(previous) = seasons.iter().find(|s| s.numero == season.numero - 1) else {
        return Ok(empty_previous_champions());
    };

    let driver_champion_id = crate::db::queries::race_history::get_category_champion_for_season(
        &db.conn,
        &previous.id,
        &category,
    )
    .map_err(|e| format!("Falha ao buscar campeao da temporada anterior: {e}"))?;

    Ok(PreviousChampions {
        driver_champion_id,
        // Títulos de construtores pedem contagem histórica por equipe (`titles`)
        // e o sinal de defesa; segue vazio até essa parte existir.
        constructor_champions: Vec::<ConstructorChampion>::new(),
    })
}

pub(crate) fn get_calendar_for_category_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    category: &str,
) -> Result<Vec<RaceSummary>, String> {
    let category = category.trim().to_lowercase();
    let (db, _, _) = open_career_resources_for_category_read(base_dir, career_id, &category)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    calendar_queries::normalize_calendar_display_dates_for_weekday_policy(
        &db.conn, &season.id, season.ano,
    )
    .map_err(|e| format!("Falha ao normalizar datas do calendario: {e}"))?;
    let entries = calendar_queries::get_calendar(&db.conn, &season.id, &category)
        .map_err(|e| format!("Falha ao buscar calendario da categoria: {e}"))?;

    Ok(entries
        .into_iter()
        .map(|race| RaceSummary {
            id: race.id,
            rodada: race.rodada,
            track_name: race.track_name,
            clima: race.clima.as_str().to_string(),
            duracao_corrida_min: race.duracao_corrida_min,
            status: race.status.as_str().to_string(),
            temperatura: race.temperatura,
            horario: race.horario.clone(),
            week_of_year: race.week_of_year,
            season_phase: race.season_phase.as_str().to_string(),
            display_date: race.display_date.clone(),
            thematic_slot: race.thematic_slot.as_str().to_string(),
            event_interest: None,
            public_fame_share: None,
        })
        .collect())
}

pub(crate) fn preferred_active_contract_for_phase(
    conn: &rusqlite::Connection,
    driver_id: &str,
    season_phase: SeasonPhase,
) -> Result<Option<crate::models::contract::Contract>, String> {
    if season_phase == SeasonPhase::BlocoEspecial {
        let special_contract =
            contract_queries::get_active_especial_contract_for_pilot(conn, driver_id)
                .map_err(|e| format!("Falha ao buscar contrato especial ativo: {e}"))?;
        if special_contract.is_some() {
            return Ok(special_contract);
        }
    }

    contract_queries::get_active_regular_contract_for_pilot(conn, driver_id)
        .map_err(|e| format!("Falha ao buscar contrato regular ativo: {e}"))
}

pub(crate) fn find_player_team(
    conn: &rusqlite::Connection,
    player_id: &str,
    season_phase: SeasonPhase,
) -> Result<Option<Team>, String> {
    let contract = preferred_active_contract_for_phase(conn, player_id, season_phase)?;
    resolve_driver_team(conn, player_id, contract.as_ref())
}

pub(crate) fn resolve_driver_team(
    conn: &rusqlite::Connection,
    driver_id: &str,
    contract: Option<&crate::models::contract::Contract>,
) -> Result<Option<Team>, String> {
    if let Some(contract) = contract {
        if let Some(mut team) = team_queries::get_team_by_id(conn, &contract.equipe_id)
            .map_err(|e| format!("Falha ao buscar equipe do contrato: {e}"))?
        {
            if contract.tipo.as_str() == "Especial" {
                team.categoria = contract.categoria.clone();
                team.classe = contract.classe.clone();
                let special_contracts =
                    contract_queries::get_active_especial_contracts_by_category(
                        conn,
                        &contract.categoria,
                    )
                    .map_err(|e| format!("Falha ao buscar contratos especiais ativos: {e}"))?;
                team.piloto_1_id = special_contracts
                    .iter()
                    .find(|value| {
                        value.equipe_id == contract.equipe_id && value.papel.as_str() == "Numero1"
                    })
                    .map(|value| value.piloto_id.clone());
                team.piloto_2_id = special_contracts
                    .iter()
                    .find(|value| {
                        value.equipe_id == contract.equipe_id && value.papel.as_str() == "Numero2"
                    })
                    .map(|value| value.piloto_id.clone());
            }
            return Ok(Some(team));
        }
    }

    let mut stmt = conn
        .prepare("SELECT id FROM teams WHERE piloto_1_id = ?1 OR piloto_2_id = ?1 LIMIT 1")
        .map_err(|e| format!("Falha ao procurar equipe do piloto: {e}"))?;
    let team_id: Option<String> = stmt
        .query_row(rusqlite::params![driver_id], |row| row.get(0))
        .optional()
        .map_err(|e| format!("Falha ao procurar equipe do piloto: {e}"))?;

    match team_id {
        Some(id) => team_queries::get_team_by_id(conn, &id)
            .map_err(|e| format!("Falha ao carregar equipe do piloto: {e}")),
        None => Ok(None),
    }
}

pub(crate) fn resolve_driver_role(
    driver_id: &str,
    contract: Option<&crate::models::contract::Contract>,
    team: Option<&Team>,
) -> Option<String> {
    if let Some(contract) = contract {
        return Some(contract.papel.as_str().to_string());
    }

    team.and_then(|value| {
        if value.piloto_1_id.as_deref() == Some(driver_id) {
            Some("Numero1".to_string())
        } else if value.piloto_2_id.as_deref() == Some(driver_id) {
            Some("Numero2".to_string())
        } else {
            None
        }
    })
}

pub(crate) fn build_team_summary(
    conn: &rusqlite::Connection,
    team: &Team,
) -> Result<TeamSummary, String> {
    let piloto_1_nome = match &team.piloto_1_id {
        Some(id) => Some(
            driver_queries::get_driver(conn, id)
                .map_err(|e| format!("Falha ao carregar piloto 1 da equipe: {e}"))?
                .nome,
        ),
        None => None,
    };

    let piloto_2_nome = match &team.piloto_2_id {
        Some(id) => Some(
            driver_queries::get_driver(conn, id)
                .map_err(|e| format!("Falha ao carregar piloto 2 da equipe: {e}"))?
                .nome,
        ),
        None => None,
    };

    let financial_plan = calculate_financial_plan(team);
    let salary_ceiling = calculate_salary_ceiling(team);
    let active_contracts = contract_queries::get_active_contracts_for_team(conn, &team.id)
        .map_err(|e| format!("Falha ao carregar contratos ativos da equipe: {e}"))?;
    let piloto_1_salario_anual = salary_for_driver(&active_contracts, team.piloto_1_id.as_deref());
    let piloto_2_salario_anual = salary_for_driver(&active_contracts, team.piloto_2_id.as_deref());

    Ok(TeamSummary {
        id: team.id.clone(),
        nome: team.nome.clone(),
        nome_curto: team.nome_curto.clone(),
        cor_primaria: team.cor_primaria.clone(),
        cor_secundaria: team.cor_secundaria.clone(),
        categoria: team.categoria.clone(),
        classe: team.classe.clone(),
        car_performance: team.effective_car_performance(),
        car_level: team.car.as_ref().map(|c| c.display_level()).unwrap_or(1),
        confiabilidade: team.confiabilidade,
        pit_strategy_risk: team.pit_strategy_risk,
        pit_crew_quality: team.pit_crew_quality,
        budget: team.budget,
        spending_power: financial_plan.spending_power,
        salary_ceiling,
        budget_index: financial_plan.budget_index,
        cash_balance: team.cash_balance,
        debt_balance: team.debt_balance,
        financial_state: team.financial_state.clone(),
        season_strategy: team.season_strategy.clone(),
        last_round_income: team.last_round_income,
        last_round_expenses: team.last_round_expenses,
        last_round_net: team.last_round_net,
        parachute_payment_remaining: team.parachute_payment_remaining,
        piloto_1_id: team.piloto_1_id.clone(),
        piloto_1_nome,
        piloto_1_salario_anual,
        piloto_2_id: team.piloto_2_id.clone(),
        piloto_2_nome,
        piloto_2_salario_anual,
        hierarquia_n1_id: team.hierarquia_n1_id.clone(),
        hierarquia_n2_id: team.hierarquia_n2_id.clone(),
        hierarquia_status: team.hierarquia_status.clone(),
        hierarquia_tensao: team.hierarquia_tensao,
        hierarquia_inversoes_temporada: team.hierarquia_inversoes_temporada,
    })
}

pub(crate) fn salary_for_driver(
    contracts: &[crate::models::contract::Contract],
    driver_id: Option<&str>,
) -> Option<f64> {
    let driver_id = driver_id?;
    contracts
        .iter()
        .find(|contract| contract.piloto_id == driver_id)
        .map(|contract| contract.salario_anual)
}

pub(crate) fn build_accepted_special_offer_summary(
    conn: &rusqlite::Connection,
    player: &crate::models::driver::Driver,
) -> Result<Option<AcceptedSpecialOfferSummary>, String> {
    if player.categoria_especial_ativa.is_none() {
        return Ok(None);
    }

    let Some(contract) = contract_queries::get_active_especial_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao buscar contrato especial ativo: {e}"))?
    else {
        return Ok(None);
    };

    Ok(Some(AcceptedSpecialOfferSummary {
        id: contract.id,
        team_id: contract.equipe_id,
        team_name: contract.equipe_nome,
        special_category: contract.categoria,
        class_name: contract.classe.unwrap_or_default(),
        papel: contract.papel.as_str().to_string(),
    }))
}

pub(crate) fn count_calendar_entries(
    conn: &rusqlite::Connection,
    season_id: &str,
    categoria: &str,
) -> Result<i32, rusqlite::Error> {
    conn.query_row(
        "SELECT COUNT(*) FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND categoria = ?2",
        rusqlite::params![season_id, categoria],
        |row| row.get(0),
    )
}
