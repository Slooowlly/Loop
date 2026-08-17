//! O mercado visto pelo piloto do jogador: propostas recebidas, assentos reservados,
//! interesse das equipes e a assinatura escolhida na tela.
//!
//! O mundo da IA se resolve sozinho; aqui fica tudo que precisa esperar a decisão
//! humana — quem oferta, por quanto, por quantas semanas a oferta vive e onde o
//! jogador cai se nada for aceito.

use super::*;

/// GARANTIA DE PORTA (sem deslocar ninguém): ao fechar a janela, se o JOGADOR ficou sem
/// contrato, coloca-o numa vaga VAZIA que a carteira dele alcança. Como o mercado segura
/// 2-3 assentos vazios pra ele a semana toda (ver `player_reserved_seats`), sempre há
/// vaga natural aqui — nenhum piloto da IA é dispensado pra abrir espaço. Em um save
/// degenerado sem NENHUMA vaga licenciada, ele segue agente livre nesta temporada (a
/// finalização aceita jogador sem time); volta a ter reserva na janela seguinte.
pub(crate) fn ensure_player_seated(conn: &Connection, season: i32) -> Result<(), String> {
    let Ok(player) = driver_queries::get_player_driver(conn) else {
        return Ok(());
    };
    // Só o aposentado fica sem porta. Lesão e suspensão são temporárias, e o gate por
    // `!= Ativo` fazia o jogador lesionado atravessar a virada SEM equipe (medido: 5%
    // dos fechos de janela) — justamente quem não pode correr atrás de vaga. A IA nunca
    // teve essa exclusão: o filtro dela barra só `Aposentado`.
    if player.status == DriverStatus::Aposentado {
        return Ok(());
    }
    if contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao checar contrato do jogador: {e}"))?
        .is_some()
    {
        return Ok(()); // Já tem time (assinou na janela).
    }

    // 1. Vaga vazia na PRÓPRIA categoria / mesmo tier (sem demoção) — normalmente um dos
    //    assentos reservados. 2. Recurso final: qualquer vaga vazia licenciada ou estreia
    //    (rookie). Nunca dispensa um piloto pra abrir vaga.
    if !place_player_in_natural_vacancy(conn, &player, season, false)? {
        place_player_in_natural_vacancy(conn, &player, season, true)?;
    }
    Ok(())
}

/// Coloca o jogador na melhor vaga DISPONÍVEL. Com `allow_fallback=false`: só própria
/// categoria → mesmo tier (sem demoção). Com `true`: também desce p/ qualquer
/// licenciada e estreia (rookie) — recurso final. Pior carro primeiro. Devolve sucesso.
pub(super) fn place_player_in_natural_vacancy(
    conn: &Connection,
    player: &Driver,
    season: i32,
    allow_fallback: bool,
) -> Result<bool, String> {
    let player_tier = player_market_tier(conn, &player)?;
    let player_lic = load_max_license_levels(conn)?
        .get(&player.id)
        .copied()
        .unwrap_or(0)
        .max(
            crate::models::license::required_license_for_division(
                player.categoria_atual.as_deref().unwrap_or(""),
                None,
            )
            .unwrap_or(0),
        );
    // No fallback, a estreia (rookie) é sempre acessível (piso de recomeço).
    let licensed = |vac: &Vacancy| {
        (allow_fallback && is_real_career_debut_category(&vac.categoria))
            || crate::models::license::required_license_for_division(
                &vac.categoria,
                vac.classe.as_deref(),
            )
            .unwrap_or(0)
                <= player_lic
    };
    // `market/sync.rs` zera o `categoria_atual` de quem está sem contrato regular, então
    // o agente livre chegava aqui com string vazia e o passe 0 ("própria categoria")
    // nunca casava com vaga nenhuma — 0 acertos em 492 fechos medidos; quem resolvia era
    // sempre o passe 1 (mesmo tier), que ignora a preferência pela categoria de origem.
    // O resgate é o mesmo do `player_market_tier`: a categoria do último contrato.
    let player_cat = match player.categoria_atual.clone().filter(|cat| !cat.is_empty()) {
        Some(cat) => cat,
        None => find_last_player_category(conn, &player.id)?,
    };
    let vacancies = find_vacancies(conn)?;
    let mut pick: Option<&Vacancy> = None;
    // Sem fallback: só passes 0 (categoria) e 1 (mesmo tier). Com: + 2 (qualquer).
    let passes = if allow_fallback { 3 } else { 2 };
    for pass in 0..passes {
        let mut cands: Vec<&Vacancy> = vacancies
            .iter()
            .filter(|v| {
                licensed(v)
                    && match pass {
                        0 => v.categoria == player_cat,
                        1 => v.category_tier == player_tier,
                        _ => true,
                    }
            })
            .collect();
        if !cands.is_empty() {
            cands.sort_by(|a, b| a.car_strength.total_cmp(&b.car_strength));
            pick = cands.first().copied();
            break;
        }
    }
    let Some(vac) = pick.cloned() else {
        return Ok(false);
    };
    // Concede a licença da vaga se faltar (ex.: recomeço numa estreia / sua categoria).
    let _ = crate::licensing::grant_driver_license_for_division_if_needed(
        conn,
        &player.id,
        &vac.categoria,
        vac.classe.as_deref(),
    );
    // MESMA fonte salarial da oferta e da assinatura na tela (`player_market_offers` e
    // `sign_player_to_vacancy`): faixa por tier posicionada pela skill, fator de papel,
    // variação estável da equipe e o prêmio de interesse ativo. A garantia de porta usava
    // uma fórmula própria (`12k + skill*1.8k`) que ignorava a categoria e inflava o valor
    // — o jogador lia um número na Janela e o contrato gravava outro.
    let is_n1 = matches!(vac.papel_necessario, TeamRole::Numero1);
    let active_interest = player_active_interest_teams(conn, player)?
        .iter()
        .any(|(id, _, _)| id == &vac.team_id);
    let salary =
        player_offer_salary_with_interest(&vac, is_n1, player.atributos.skill, active_interest);
    sign_driver_to_team(
        conn,
        player,
        &vac,
        season,
        salary,
        1,
        vac.papel_necessario.clone(),
    )?;
    Ok(true)
}

/// A equipe do jogador foi VENDIDA por falência no offseason que está rodando agora?
/// Devolve o id dela.
///
/// A venda é registrada em `team_ownership_events` no fim da temporada que acabou —
/// isto é, `new_season_number − 1` — por `evolution::pipeline::financas`. É a única
/// coisa que o mercado precisa saber sobre a quebra: que existe um jogador preso a um
/// projeto que acabou de ruir e que ele merece uma porta de saída.
pub(super) fn player_team_sold_in_offseason(
    conn: &Connection,
    player: &Driver,
    new_season_number: i32,
) -> Result<Option<String>, String> {
    let Some(contract) = contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao checar contrato do jogador: {e}"))?
    else {
        return Ok(None); // Sem time não há de onde fugir.
    };
    let evento = team_queries::get_latest_ownership_event(conn, &contract.equipe_id)
        .map_err(|e| format!("Falha ao ler eventos de propriedade da equipe: {e}"))?;
    match evento {
        Some((tipo, temporada)) if tipo == "sale" && temporada == new_season_number - 1 => {
            Ok(Some(contract.equipe_id))
        }
        _ => Ok(None),
    }
}

pub(super) fn generate_player_proposals(
    conn: &Connection,
    season_id: &str,
    new_season_number: i32,
    vacancies: &[Vacancy],
    player_was_expiring: bool,
    standings_by_driver: &HashMap<String, DriverMarketContext>,
    rng: &mut impl Rng,
) -> Result<Vec<MarketProposal>, String> {
    let player = match driver_queries::get_player_driver(conn) {
        Ok(p) => p,
        Err(crate::db::connection::DbError::NotFound(_)) => return Ok(Vec::new()),
        Err(e) => {
            return Err(format!(
                "Falha ao buscar piloto do jogador para o mercado: {e}"
            ))
        }
    };
    let player_active_contract =
        contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
            .map_err(|e| format!("Falha ao buscar contrato regular do jogador: {e}"))?;
    let player_is_free = player_active_contract.is_none();
    // FUGA DA FALÊNCIA: se a equipe do jogador acabou de ser vendida por colapso, ele
    // recebe propostas mesmo estando sob contrato. A quebra não pode ser só uma
    // notícia — precisa vir com uma porta. Aceitar rescinde o contrato atual
    // (`accept_player_proposal_tx` já faz isso); ignorar é a escolha de ficar e correr
    // com o que sobrou.
    let equipe_falida = player_team_sold_in_offseason(conn, &player, new_season_number)?;
    if !player_is_free && !player_was_expiring && equipe_falida.is_none() {
        return Ok(Vec::new());
    }

    // Calcula visibilidade real do jogador com os mesmos dados usados pela IA.
    let context = standings_by_driver
        .get(&player.id)
        .cloned()
        .unwrap_or_else(|| default_market_context(&player));
    let visibility = calculate_visibility(
        &player,
        context.posicao_campeonato,
        context.total_pilotos,
        context.category_tier,
        context.vitorias,
        context.titulos,
        context.poles,
        &context.papel,
        &context.categoria,
    );

    // Usa is_jogador=false para que generate_team_proposals avalie o jogador
    // com os mesmos critérios de qualquer piloto IA. A flag existe apenas para
    // impedir que o loop principal de mercado proponha ao jogador — aqui é intencional.
    let license_levels = load_max_license_levels(conn)?;
    let max_license_level = license_levels.get(&player.id).copied();
    let mut player_as_driver = player.clone();
    player_as_driver.is_jogador = false;
    let player_available = AvailableDriver {
        driver: player_as_driver,
        visibility,
        posicao_campeonato: context.posicao_campeonato,
        categoria_atual: context.categoria.clone(),
        category_tier: context.category_tier,
        max_license_level,
    };

    let mut proposals = Vec::new();
    for vacancy in vacancies {
        // A equipe que acabou de quebrar não oferta a fuga dela mesma.
        if equipe_falida.as_deref() == Some(vacancy.team_id.as_str()) {
            continue;
        }
        let team_proposals = generate_team_proposals(
            vacancy,
            std::slice::from_ref(&player_available),
            new_season_number,
            rng,
        );
        for mut proposal in team_proposals {
            // Restaura o ID correto do jogador e gera ID de proposta único por temporada.
            proposal.piloto_id = player.id.clone();
            proposal.piloto_nome = player.nome.clone();
            proposal.id = format!(
                "MP-{}-{}-{}-{}",
                new_season_number,
                vacancy.team_id,
                player.id,
                vacancy.papel_necessario.as_str(),
            );
            persist_player_proposal(conn, season_id, &proposal, None)?;
            proposals.push(proposal);
        }
    }

    // Garantia de proposta mínima: se o jogador estiver livre neste início de temporada,
    // a pipeline tenta preservar ao menos uma rota de continuidade para não deixá-lo
    // sem evento jogável na pré-temporada. Isso cobre tanto quem já estava livre
    // quanto quem acabou de ter o contrato encerrado.
    // Tenta: 1) equipe anterior na mesma categoria, 2) pior equipe da mesma categoria,
    // 3) melhor equipe de categoria inferior (salário menor naturalmente).
    // A garantia vale também para quem está fugindo da falência: se nenhuma vaga do
    // mercado gerou proposta, ele ainda tem que ver UMA porta — senão "há como sair"
    // vira promessa vazia e a única opção é ficar.
    if proposals.is_empty() && (player_is_free || equipe_falida.is_some()) {
        // Categoria do jogador: contexto dos standings ou último contrato no DB.
        let player_category = if !context.categoria.is_empty() {
            context.categoria.clone()
        } else {
            find_last_player_category(conn, &player.id)?
        };

        if !player_category.is_empty() {
            let all_teams = team_queries::get_all_teams(conn)
                .map_err(|e| format!("Falha ao carregar equipes para fallback do jogador: {e}"))?;
            let category_teams: Vec<&crate::models::team::Team> = all_teams
                .iter()
                .filter(|team| team.categoria == player_category)
                .filter(|team| equipe_falida.as_deref() != Some(team.id.as_str()))
                .collect();

            // Tenta primeiro a equipe anterior, depois a pior equipe da mesma categoria.
            let mut fallback_team =
                find_previous_team_for_player(conn, &player.id, &category_teams)?
                    .or_else(|| worst_team(&category_teams));

            // Se não há vaga na categoria atual, tenta a melhor vaga de tier inferior.
            if fallback_team.is_none() {
                let player_tier =
                    crate::constants::categories::get_category_config(&player_category)
                        .map(|c| c.tier)
                        .unwrap_or(99);
                let lower_teams: Vec<&crate::models::team::Team> = all_teams
                    .iter()
                    .filter(|team| {
                        crate::constants::categories::get_category_config(&team.categoria)
                            .map(|c| c.tier < player_tier)
                            .unwrap_or(false)
                    })
                    .collect();
                fallback_team = best_team(&lower_teams);
            }

            if let Some(team) = fallback_team {
                let vacancy = fallback_vacancy_from_team(team);
                let proposal = MarketProposal {
                    id: format!(
                        "MP-{}-{}-{}-fallback",
                        new_season_number, vacancy.team_id, player.id
                    ),
                    equipe_id: vacancy.team_id.clone(),
                    equipe_nome: vacancy.team_name.clone(),
                    piloto_id: player.id.clone(),
                    piloto_nome: player.nome.clone(),
                    categoria: vacancy.categoria.clone(),
                    papel: vacancy.papel_necessario.clone(),
                    salario_oferecido: calculate_offer_salary(&vacancy, &player, rng),
                    duracao_anos: 1,
                    status: crate::market::proposals::ProposalStatus::Pendente,
                    motivo_recusa: None,
                };
                persist_player_proposal(conn, season_id, &proposal, None)?;
                proposals.push(proposal);
            }
        }
    }

    Ok(proposals)
}

/// Retorna a categoria do contrato mais recente do jogador (qualquer status).
pub(super) fn find_last_player_category(
    conn: &Connection,
    player_id: &str,
) -> Result<String, String> {
    conn.query_row(
        // `temporada_fim` é coluna TEXT: sem o CAST a ordenação é lexicográfica e o
        // "mais recente" vira a temporada 9 em vez da 10.
        "SELECT categoria FROM contracts WHERE piloto_id = ?1
         ORDER BY CAST(temporada_fim AS INTEGER) DESC LIMIT 1",
        params![player_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map(|opt| opt.unwrap_or_default())
    .map_err(|e| format!("Falha ao buscar última categoria do jogador: {e}"))
}

pub(super) fn find_previous_team_for_player<'a>(
    conn: &Connection,
    player_id: &str,
    teams: &[&'a crate::models::team::Team],
) -> Result<Option<&'a crate::models::team::Team>, String> {
    let prev_team_id: Option<String> = conn
        .query_row(
            // Mesmo motivo do CAST em `find_last_player_category`: coluna TEXT.
            "SELECT equipe_id FROM contracts WHERE piloto_id = ?1
             ORDER BY CAST(temporada_fim AS INTEGER) DESC LIMIT 1",
            params![player_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Falha ao buscar equipe anterior do jogador: {e}"))?;

    let Some(team_id) = prev_team_id else {
        return Ok(None);
    };
    Ok(teams.iter().find(|team| team.id == team_id).copied())
}

pub(super) fn worst_team<'a>(
    teams: &[&'a crate::models::team::Team],
) -> Option<&'a crate::models::team::Team> {
    teams
        .iter()
        .min_by(|a, b| a.car_strength().total_cmp(&b.car_strength()))
        .copied()
}

pub(super) fn best_team<'a>(
    teams: &[&'a crate::models::team::Team],
) -> Option<&'a crate::models::team::Team> {
    teams
        .iter()
        .max_by(|a, b| a.car_strength().total_cmp(&b.car_strength()))
        .copied()
}

pub(super) fn fallback_vacancy_from_team(team: &crate::models::team::Team) -> Vacancy {
    let papel_necessario = if team.piloto_1_id.is_none() {
        TeamRole::Numero1
    } else {
        TeamRole::Numero2
    };
    let piloto_existente_id = match papel_necessario {
        TeamRole::Numero1 => team.piloto_1_id.clone(),
        TeamRole::Numero2 => team.piloto_2_id.clone(),
    };

    Vacancy {
        team_id: team.id.clone(),
        team_name: team.nome.clone(),
        categoria: team.categoria.clone(),
        classe: team.classe.clone(),
        category_tier: get_category_config(&team.categoria)
            .map(|config| config.tier)
            .unwrap_or(0),
        car_strength: team.car_strength(),
        budget: team.budget,
        cash_balance: team.cash_balance,
        debt_balance: team.debt_balance,
        financial_state: team.financial_state.clone(),
        reputacao: team.reputacao,
        papel_necessario,
        piloto_existente_id,
    }
}

pub(super) fn persist_player_proposal(
    conn: &Connection,
    season_id: &str,
    proposal: &MarketProposal,
    semana_limite: Option<i32>,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO market_proposals (
            id, temporada_id, equipe_id, piloto_id, papel, salario, status, motivo_recusa, criado_em, semana_limite
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            &proposal.id,
            season_id,
            &proposal.equipe_id,
            &proposal.piloto_id,
            proposal.papel.as_str(),
            proposal.salario_oferecido,
            proposal.status.as_str(),
            proposal.motivo_recusa.clone(),
            timestamp_now(),
            semana_limite,
        ],
    )
    .map_err(|e| format!("Falha ao persistir proposta do jogador: {e}"))?;
    Ok(())
}

/// Teto do boost de pedigree e escala de referência (índice de carreira "forte").
pub(super) const PEDIGREE_BOOST_MAX: f64 = 0.4;

pub(super) const PEDIGREE_BOOST_SCALE: f64 = 4000.0;

/// Semanas que uma proposta formal fica de pé antes de expirar (Fase B).
pub(super) const PROPOSAL_TTL_WEEKS: i32 = 3;

/// Teto de propostas formais simultâneas por PRESTÍGIO (índice do ranking). Rookie recebe
/// pouca atenção; estrela é disputada por muitas equipes. Fase B.
pub(super) fn prestige_proposal_cap(index: f64) -> usize {
    match index {
        i if i >= 3.0 * PEDIGREE_BOOST_SCALE => 5,
        i if i >= 1.5 * PEDIGREE_BOOST_SCALE => 4,
        i if i >= 0.5 * PEDIGREE_BOOST_SCALE => 3,
        i if i >= 0.1 * PEDIGREE_BOOST_SCALE => 2,
        _ => 1,
    }
}

/// Curva saturante do pedigree: índice 0 → 0; cresce e satura em `PEDIGREE_BOOST_MAX`.
pub(super) fn pedigree_boost_from_index(index: f64) -> f64 {
    let index = index.max(0.0);
    PEDIGREE_BOOST_MAX * (index / (index + PEDIGREE_BOOST_SCALE))
}

/// Fator de pedigree ∈ [0, `PEDIGREE_BOOST_MAX`] a partir do índice do ranking mundial
/// (currículo de carreira). Curva saturante: rookie ~0, campeão/estrela aproxima o teto.
/// Barato (índice de um piloto só). Percentil entre ativos fica como refino futuro.
pub(super) fn pedigree_boost(conn: &Connection, driver: &Driver) -> Result<f64, String> {
    let index = crate::commands::global_driver_rankings::historical_index_for_driver(conn, driver)?;
    Ok(pedigree_boost_from_index(index))
}

/// O material com que o mercado avalia o jogador numa semana: o pool completo (agentes
/// livres da IA + ele, todos com o pedigree aplicado) e as vagas do tier dele.
///
/// Existe pra a expectativa mostrada nas semanas de abertura e as propostas da semana em
/// que o mercado abre saírem da MESMA conta. Se cada uma montasse o seu pool, o número
/// que o jogador leu como promessa não bateria com o que ele recebe — e a expectativa
/// viraria enfeite.
pub(super) struct PlayerMarketScan {
    pub player: Driver,
    pub pool: Vec<AvailableDriver>,
    pub vacancies: Vec<Vacancy>,
    pub player_index: f64,
}

/// Monta o [`PlayerMarketScan`], ou `None` quando não há o que avaliar: jogador inativo
/// ou já contratado (só agente livre recebe proposta formal nesta fase — poaching
/// mid-contrato é Fase D).
pub(super) fn build_player_market_scan(
    conn: &Connection,
    season: i32,
) -> Result<Option<PlayerMarketScan>, String> {
    let Ok(player) = driver_queries::get_player_driver(conn) else {
        return Ok(None);
    };
    // Mesmo critério do pool da IA (`find_available_drivers` barra só `Aposentado`):
    // agente livre lesionado continua no mercado — a proposta é para a temporada que
    // vem, não para a corrida de amanhã. O gate por `!= Ativo` o deixava invisível
    // para o mercado inteiro enquanto ele mais precisava de contrato.
    if player.status == DriverStatus::Aposentado {
        return Ok(None);
    }
    if contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao checar contrato do jogador: {e}"))?
        .is_some()
    {
        return Ok(None);
    }

    let previous_season = get_season_by_number(conn, season - 1)?;

    // Contexto de visibilidade (mesmos dados usados pela IA).
    let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos: {e}"))?
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();
    let contexts = load_market_contexts(
        conn,
        previous_season.as_ref().map(|s| s.id.as_str()),
        &drivers_by_id,
        &HashMap::new(),
    )?;
    let context = contexts
        .get(&player.id)
        .cloned()
        .unwrap_or_else(|| default_market_context(&player));
    let visibility = calculate_visibility(
        &player,
        context.posicao_campeonato,
        context.total_pilotos,
        context.category_tier,
        context.vitorias,
        context.titulos,
        context.poles,
        &context.papel,
        &context.categoria,
    );

    // Pedigree (Feature 1): o índice do ranking mundial (currículo de carreira) eleva o
    // valor de mercado — um veterano condecorado segue cortejado mesmo após temporada
    // morna. Aplica ao jogador E ao pool da IA pra a comparação de mérito ser justa.
    let player_index =
        crate::commands::global_driver_rankings::historical_index_for_driver(conn, &player)?;
    let visibility = visibility * (1.0 + pedigree_boost_from_index(player_index));

    // Jogador entra no pool com is_jogador=false pra ser avaliado como qualquer piloto.
    let license_levels = load_max_license_levels(conn)?;
    let mut player_as_driver = player.clone();
    player_as_driver.is_jogador = false;
    let player_available = AvailableDriver {
        driver: player_as_driver,
        visibility,
        posicao_campeonato: context.posicao_campeonato,
        categoria_atual: context.categoria.clone(),
        category_tier: context.category_tier,
        max_license_level: license_levels.get(&player.id).copied(),
    };
    let player_tier = player_market_tier(conn, &player)?;

    // Pool completo: jogador + agentes livres da IA — pra o mérito ser "a equipe te prefere
    // aos livres dela", não só "você serve". O pedigree também entra no valor de cada IA.
    let mut pool = find_available_drivers(conn, &contexts)?;
    for candidate in &mut pool {
        candidate.visibility *= 1.0 + pedigree_boost(conn, &candidate.driver)?;
    }
    pool.push(player_available);

    // Fase A: só o MESMO tier (proposta de promoção é Fase C).
    let vacancies: Vec<Vacancy> = find_vacancies(conn)?
        .into_iter()
        .filter(is_regular_vacancy)
        .filter(|v| v.category_tier == player_tier)
        .collect();

    Ok(Some(PlayerMarketScan {
        player,
        pool,
        vacancies,
        player_index,
    }))
}

/// Quantas equipes colocariam o jogador na shortlist AGORA, com as vagas que existem
/// neste instante. É a resposta exata que as propostas dariam — usada pela expectativa
/// mostrada antes de o mercado abrir. Não escreve nada e não consome o `rng` da janela.
///
/// `None` quando não há o que contar (jogador inativo ou já contratado), que é diferente
/// de `Some(0)` — "ninguém te quer" é notícia, "você não está no mercado" não é.
pub(crate) fn count_interested_teams(
    conn: &Connection,
    season: i32,
    week: i32,
) -> Result<Option<i32>, String> {
    let Some(scan) = build_player_market_scan(conn, season)? else {
        return Ok(None);
    };
    // Semente própria: a contagem não pode deslocar o fluxo de aleatoriedade que a
    // escada da semana usa depois — senão só de olhar a expectativa o mercado mudaria.
    //
    // Mistura temporada E semana, o mesmo discriminante da `preseason::semana`: semeando
    // só com a temporada, toda semana de abertura consome a MESMA sequência e a
    // expectativa de semanas diferentes fica correlacionada com as decisões do mercado.
    // O XOR com a constante mantém a contagem num fluxo distinto do da janela.
    let mut rng = StdRng::seed_from_u64(
        (season as u64)
            .wrapping_mul(1_000)
            .wrapping_add(week.max(0) as u64)
            ^ 0x5EED_C0FF_EE15_6A11,
    );
    let count = scan
        .vacancies
        .iter()
        .filter(|vacancy| {
            generate_team_proposals(vacancy, &scan.pool, season, &mut rng)
                .iter()
                .any(|p| p.piloto_id == scan.player.id)
        })
        .count();
    Ok(Some(count as i32))
}

/// FASE A das propostas formais ("Proposta recebida"): durante a janela semanal, gera
/// propostas nominais de MÉRITO pro jogador AGENTE LIVRE. Para cada vaga do mesmo tier,
/// roda a MESMA seleção da IA com o pool completo (jogador + agentes livres da IA); se a
/// equipe escolheria o jogador (ele entra no top-3 da shortlist dela), cria a proposta
/// formal. Isso é o que diferencia "Proposta recebida" (a equipe TE QUER) de "Suas
/// ofertas" (vaga aberta qualquer). Dedup por ID determinístico: não recria proposta já
/// criada e não reoferece assento recusado. Devolve os assentos das propostas PENDENTES,
/// pra a escada segurá-los. Sem fallback de piso — os assentos reservados já cobrem isso.
///
/// FASE B: propostas expiram após `PROPOSAL_TTL_WEEKS` semanas (varredura no início) e o
/// número de propostas simultâneas é limitado por PRESTÍGIO (`prestige_proposal_cap`).
///
/// Só é chamada da `signings_start_week` em diante: nas semanas de abertura o jogador vê
/// a expectativa (`count_interested_teams`), não propostas.
pub(crate) fn generate_player_window_proposals(
    conn: &Connection,
    season: i32,
    week: i32,
    rng: &mut impl Rng,
) -> Result<Vec<String>, String> {
    let season_row = get_season_by_number(conn, season)?
        .ok_or_else(|| format!("Temporada {season} nao encontrada"))?;

    // EXPIRAÇÃO (Fase B): propostas cujo prazo venceu deixam de ser pendentes → o assento
    // delas não é mais segurado e volta pra IA. Roda antes do scan, e mesmo quando o
    // jogador já assinou — proposta vencida não pode ficar pendente pra sempre.
    let Ok(player_id) = driver_queries::get_player_driver(conn).map(|p| p.id) else {
        return Ok(Vec::new());
    };
    conn.execute(
        "UPDATE market_proposals SET status = ?1
         WHERE temporada_id = ?2 AND piloto_id = ?3 AND status = ?4
           AND semana_limite IS NOT NULL AND semana_limite <= ?5",
        params![
            crate::market::proposals::ProposalStatus::Expirada.as_str(),
            season_row.id,
            player_id,
            crate::market::proposals::ProposalStatus::Pendente.as_str(),
            week,
        ],
    )
    .map_err(|e| format!("Falha ao expirar propostas do jogador: {e}"))?;

    let Some(scan) = build_player_market_scan(conn, season)? else {
        return Ok(Vec::new());
    };
    let PlayerMarketScan {
        player,
        pool,
        vacancies,
        player_index,
    } = scan;

    // Teto de propostas simultâneas por prestígio (Fase B): conta as que já estão de pé
    // e só cria novas até o teto.
    let cap = prestige_proposal_cap(player_index);
    let pending_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM market_proposals
             WHERE temporada_id = ?1 AND piloto_id = ?2 AND status = ?3",
            params![
                season_row.id,
                player.id,
                crate::market::proposals::ProposalStatus::Pendente.as_str()
            ],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("Falha ao contar propostas pendentes: {e}"))?;
    let mut new_slots = cap.saturating_sub(pending_count as usize);

    // MERCADO EM ONDAS: a proposta ao jogador obedece o mesmo relógio da IA. A equipe
    // que ainda não abriu contratação não propõe a ele — o que ela gera antes disso é
    // INTERESSE (`count_interested_teams`, que não filtra por onda de propósito: "tem
    // equipe de olho" aparece cedo, a proposta chega quando a equipe entra no mercado).
    // É daqui que sai o "fraco recebe nas semanas finais": a equipe do fundo, a única
    // onde ele é primeira escolha, só entra nas semanas finais.
    let metade_de_baixo = {
        let teams = team_queries::get_all_teams(conn)
            .map_err(|e| format!("Falha ao carregar equipes para a onda do mercado: {e}"))?;
        super::consolidacao::metade_de_baixo_por_categoria(&teams)
    };
    let equipe_liberada = |vacancy: &Vacancy| {
        !super::consolidacao::mercado_em_ondas()
            || week
                >= super::consolidacao::semana_de_liberacao(
                    vacancy.category_tier,
                    metade_de_baixo.contains(&vacancy.team_id),
                )
    };

    let mut held = Vec::new();
    for vacancy in &vacancies {
        // A shortlist roda para TODA vaga, sempre e na mesma ordem, mesmo quando o
        // desfecho já está decidido: ela consome o `rng`, e pular a chamada deslocaria a
        // sequência que a escada da semana usa em seguida.
        let shortlist = generate_team_proposals(vacancy, &pool, season, rng);
        let posicao = shortlist.iter().position(|p| p.piloto_id == player.id);
        let seat = format!("{}#{}", vacancy.team_id, vacancy.papel_necessario.as_str());
        let proposal_id = format!(
            "MP-{}-{}-{}-{}",
            season,
            vacancy.team_id,
            player.id,
            vacancy.papel_necessario.as_str()
        );
        // Dedup por status da proposta já existente com esse ID. É consultado ANTES de
        // olhar a posição: proposta pendente segura o assento mesmo que o jogador tenha
        // caído da shortlist nesta semana. Sem isso, a IA tomaria a vaga por baixo de uma
        // proposta que o jogador ainda pode aceitar — o que ficou provável quando o
        // nascimento passou a depender da posição.
        let existing_status: Option<String> = conn
            .query_row(
                "SELECT status FROM market_proposals WHERE id = ?1 LIMIT 1",
                params![proposal_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| format!("Falha ao checar proposta existente: {e}"))?;
        match existing_status.as_deref() {
            Some(s) if s == crate::market::proposals::ProposalStatus::Pendente.as_str() => {
                held.push(seat); // já pendente → segura
                continue;
            }
            Some(_) => continue, // recusada/aceita/expirada → não reoferece nem segura
            None => {}
        }

        // A equipe escolheria o jogador AGORA? Ver `jogador_e_a_escolha_da_vaga`.
        let Some(posicao) = posicao else { continue };
        if !jogador_e_a_escolha_da_vaga(posicao) {
            continue;
        }
        // ... e ela já pode contratar? A onda segura a proposta da equipe do fundo
        // até a semana de liberação dela — mesmo com o jogador sendo a escolha.
        if !equipe_liberada(vacancy) {
            continue;
        }
        if new_slots == 0 {
            continue; // teto de prestígio atingido nesta janela
        }
        let Some(mut proposal) = shortlist.into_iter().nth(posicao) else {
            continue;
        };
        proposal.id = proposal_id;
        proposal.piloto_nome = player.nome.clone();
        persist_player_proposal(
            conn,
            &season_row.id,
            &proposal,
            Some(week + PROPOSAL_TTL_WEEKS),
        )?;
        held.push(seat);
        new_slots -= 1;
    }
    Ok(held)
}

/// A vaga faz proposta formal ao jogador, dada a posição dele na shortlist da equipe?
///
/// A regra é **primeira escolha ainda livre**: a proposta só nasce quando não sobrou
/// ninguém melhor disponível para aquele assento. Como a escada da IA consome os agentes
/// livres semana a semana, quem estava na frente some do pool ao assinar em outro lugar e
/// o jogador sobe sozinho — então a ESCASSEZ e o MOMENTO saem do mundo, sem constante de
/// calendário. Piloto medíocre é segunda opção de todo mundo e só recebe proposta lá pelo
/// fim da janela; piloto fora da curva recebe na primeira semana de assinaturas.
///
/// Isso separa os dois conceitos que a tela mostra: **interesse** é estar no top-3
/// (`count_interested_teams`, "tem equipe de olho, mas há gente na frente") e **proposta**
/// é ser a escolha. Antes os dois eram a mesma coisa, e por isso o rookie recebia a única
/// proposta da carreira dele na primeira semana possível — o oposto da escassez pretendida.
///
/// Desligar `IRACER_PROPOSTA_PRIMEIRA_ESCOLHA` devolve o critério antigo (qualquer lugar
/// do top-3), que é o braço "antes" do A/B — não é uma opção de jogo.
pub(super) fn jogador_e_a_escolha_da_vaga(posicao_na_shortlist: usize) -> bool {
    if !crate::constants::flags_experimentais::booleana("IRACER_PROPOSTA_PRIMEIRA_ESCOLHA") {
        return true; // qualquer posição do top-3 servia
    }
    posicao_na_shortlist == 0
}

/// Tier de mercado do jogador para efeito de ofertas. Usa a `categoria_atual`; se ela
/// já foi limpa (agente livre há tempo — `sync.rs` zera o categoria_atual de quem não
/// tem contrato regular), cai na categoria do ÚLTIMO contrato. Assim um jogador que
/// ficou sem correr volta a receber ofertas NO NÍVEL DELE, e não rebaixado a rookie.
pub(super) fn player_market_tier(conn: &Connection, player: &Driver) -> Result<u8, String> {
    if let Some(tier) = player
        .categoria_atual
        .as_deref()
        .and_then(crate::constants::categories::get_category_config)
        .map(|c| c.tier)
    {
        return Ok(tier);
    }
    let last = find_last_player_category(conn, &player.id)?;
    Ok(crate::constants::categories::get_category_config(&last)
        .map(|c| c.tier)
        .unwrap_or(0))
}

/// Nível de licença máximo do jogador (licença efetiva = maior entre a possuída e a
/// exigida pela categoria atual). Reaproveita o estilo de `place_player_in_natural_vacancy`.
pub(super) fn player_effective_license(conn: &Connection, player: &Driver) -> Result<u8, String> {
    Ok(load_max_license_levels(conn)?
        .get(&player.id)
        .copied()
        .unwrap_or(0)
        .max(
            crate::models::license::required_license_for_division(
                player.categoria_atual.as_deref().unwrap_or(""),
                None,
            )
            .unwrap_or(0),
        ))
}

/// Salário ofertado ao jogador numa vaga, na MESMA escala dos contratos da IA: faixa por tier
/// (`salary_range_for_tier`) posicionada pela skill, com fator de papel (N1 titular
/// ganha mais que N2). Antes usava uma fórmula fixa `12k + skill*1.8k` que ignorava a
/// categoria e inflava o valor (ex.: ~100k no rookie, que na verdade paga ~5k–21k).
/// O `team_id` aplica uma variação ESTÁVEL (±7%, determinística) pra cada equipe ter
/// seu próprio número — sem isso todas ofereciam exatamente o mesmo valor.
///
/// FONTE ÚNICA do salário do jogador: a listagem da Janela, a assinatura na tela e a
/// garantia de porta (`place_player_in_natural_vacancy`) passam todas por aqui, via
/// `player_offer_salary_with_interest`. Nenhum desses caminhos tem fórmula própria.
///
/// Isto aqui é o VALOR DE MERCADO do jogador, sem olhar o bolso de quem paga. O que a
/// equipe consegue sustentar entra depois, no teto financeiro de
/// `player_offer_salary_with_interest` — que é por onde todo mundo passa.
pub(super) fn player_offer_salary(tier: u8, is_n1: bool, skill: f64, team_id: &str) -> f64 {
    let (base_min, base_max) = crate::models::contract::salary_range_for_tier(tier);
    let t = (skill / 100.0).clamp(0.0, 1.0);
    let base = base_min + (base_max - base_min) * t;
    // Fatores no MEIO das faixas da IA (N1 1.20–1.40, N2 1.00–1.12).
    let role_mult = if is_n1 { 1.30 } else { 1.06 };
    (base * role_mult * team_salary_multiplier(team_id))
        .round()
        .max(5_000.0)
}

/// Multiplicador salarial ESTÁVEL por equipe (0.93–1.07 = ±7%), derivado de um hash
/// determinístico do id — mesmo time sempre dá o mesmo número (oferta = assinatura),
/// mas times diferentes variam. Dá "personalidade" às ofertas sem aleatoriedade.
pub(super) fn team_salary_multiplier(team_id: &str) -> f64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a offset basis
    for b in team_id.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3); // FNV prime
    }
    let frac = (hash % 1000) as f64 / 999.0; // 0.0..1.0
    0.93 + frac * 0.14 // 0.93..1.07
}

/// Quantos assentos o mercado segura pro jogador agente livre. Segurar ALGUNS (não um
/// só) dá escolha real e, principalmente, garante que na última semana haja vaga VAZIA
/// pra ele — sem precisar dispensar nenhum piloto da IA pra abrir espaço.
pub(super) const MAX_PLAYER_RESERVED_SEATS: usize = 3;

/// Se o jogador está ativo e SEM contrato regular ativo, SEGURA até
/// `MAX_PLAYER_RESERVED_SEATS` assentos regulares que a carteira dele alcança — os mais
/// acessíveis primeiro (tier dele → tier−1 → qualquer licenciada; pior carro primeiro
/// dentro de cada faixa). A escada poupa esses assentos toda semana, então eles seguem
/// VAZIOS até o fecho, quando o jogador escolhe um (ou é colocado) e os demais são
/// repostos pela IA. Devolve os `seat_id` `team#papel`, ou vazio se ele já tem contrato
/// / não está ativo. Nunca desloca ninguém: só reserva vagas já vazias.
pub(crate) fn player_reserved_seats(conn: &Connection, season: i32) -> Result<Vec<String>, String> {
    let _ = season;
    let Ok(player) = driver_queries::get_player_driver(conn) else {
        return Ok(Vec::new());
    };
    if player.status != DriverStatus::Ativo {
        return Ok(Vec::new());
    }
    if contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao checar contrato do jogador: {e}"))?
        .is_some()
    {
        return Ok(Vec::new());
    }

    let player_tier = player_market_tier(conn, &player)?;
    let player_lic = player_effective_license(conn, &player)?;
    // Vaga de estreia (rookie) é sempre acessível ao jogador (piso de recomeço).
    let licensed = |vac: &Vacancy| {
        is_real_career_debut_category(&vac.categoria)
            || crate::models::license::required_license_for_division(
                &vac.categoria,
                vac.classe.as_deref(),
            )
            .unwrap_or(0)
                <= player_lic
    };
    let vacancies: Vec<Vacancy> = find_vacancies(conn)?
        .into_iter()
        .filter(is_regular_vacancy)
        .collect();
    // Passes: tier do jogador → tier−1 → qualquer licenciada; pior carro primeiro.
    // Acumula até o teto, sem repetir assento entre as faixas.
    let mut seats: Vec<String> = Vec::new();
    for pass in 0..3 {
        let mut cands: Vec<&Vacancy> = vacancies
            .iter()
            .filter(|v| {
                licensed(v)
                    && match pass {
                        0 => v.category_tier == player_tier,
                        1 => player_tier > 0 && v.category_tier == player_tier - 1,
                        _ => true,
                    }
            })
            .collect();
        cands.sort_by(|a, b| a.car_strength.total_cmp(&b.car_strength));
        for vac in cands {
            let seat = format!("{}#{}", vac.team_id, vac.papel_necessario.as_str());
            if !seats.contains(&seat) {
                seats.push(seat);
                if seats.len() >= MAX_PLAYER_RESERVED_SEATS {
                    return Ok(seats);
                }
            }
        }
    }
    Ok(seats)
}

/// Ofertas do mercado pro JOGADOR nesta semana: toda vaga regular em que ele é
/// elegível+licenciado, no tier dele OU tier−1. Vazio se ele já tem contrato / não
/// está ativo. (`is_n1` = papel Numero1; salário = fórmula de garantia de porta.)
/// Posição final do jogador na temporada arquivada mais recente (menor número = melhor).
/// None se ainda não correu. Usado pra decidir ofertas de promoção (pódio). Lê do ARQUIVO
/// persistente (`driver_season_archive`) — NÃO do `standings`, que é recalculado do zero e
/// deletado a cada avanço de temporada (e exclui agentes livres que não correram).
pub(super) fn player_last_finish_position(conn: &Connection, player_id: &str) -> Option<i32> {
    conn.query_row(
        "SELECT posicao_campeonato FROM driver_season_archive
         WHERE piloto_id = ?1 AND posicao_campeonato IS NOT NULL
         ORDER BY season_number DESC
         LIMIT 1",
        params![player_id],
        |row| row.get::<_, i32>(0),
    )
    .ok()
}

/// Os POUCOS MELHORES times que cobiçam o jogador pela fama — o "interesse ativo"
/// VISÍVEL ao jogador (badge + N1 + prêmio + e-mail). Retorna `(team_id, nome,
/// categoria)` dos melhores da categoria do jogador (exceto o dele), quantos =
/// `active_interest_team_count(fama)`. Decoplado da economia da IA (que dá apelo aos
/// times CARENTES): pro jogador, "me querem" tem que ler como "time bom". Fase 2a.
pub(crate) fn player_active_interest_teams(
    conn: &Connection,
    player: &Driver,
) -> Result<Vec<(String, String, String)>, String> {
    let count = crate::fame::active_interest_team_count(player.atributos.midia);
    if count == 0 {
        return Ok(Vec::new());
    }
    let categoria = match player.categoria_atual.as_deref() {
        Some(c) if !c.is_empty() => c.to_string(),
        _ => return Ok(Vec::new()),
    };
    let player_team_id = contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .ok()
        .flatten()
        .map(|c| c.equipe_id);
    let mut teams: Vec<_> = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao carregar times p/ interesse do astro: {e}"))?
        .into_iter()
        .filter(|team| team.ativa && team.categoria == categoria)
        .filter(|team| Some(&team.id) != player_team_id.as_ref())
        .collect();
    teams.sort_by(|a, b| team_quality(b).total_cmp(&team_quality(a)));
    Ok(teams
        .into_iter()
        .take(count)
        .map(|team| (team.id, team.nome, team.categoria))
        .collect())
}

pub(crate) fn player_market_offers(
    conn: &Connection,
    season: i32,
) -> Result<Vec<crate::market::transfer_window::PlayerOffer>, String> {
    let Ok(player) = driver_queries::get_player_driver(conn) else {
        return Ok(Vec::new());
    };
    if player.status != DriverStatus::Ativo {
        return Ok(Vec::new());
    }
    if contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao checar contrato do jogador: {e}"))?
        .is_some()
    {
        return Ok(Vec::new());
    }

    let player_tier = player_market_tier(conn, &player)?;
    let player_lic = player_effective_license(conn, &player)?;
    // Pódio (1º–3º) na última temporada → habilita ofertas de PROMOÇÃO (tier acima).
    let player_podium =
        player_last_finish_position(conn, &player.id).is_some_and(|pos| (1..=3).contains(&pos));
    // Interesse ativo (Fase 2a): os poucos MELHORES times que cobiçam o jogador.
    let interest_team_ids: std::collections::HashSet<String> =
        player_active_interest_teams(conn, &player)?
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();

    // Assentos que já têm proposta formal PENDENTE ("Proposta recebida") não aparecem
    // também em "Suas ofertas" — o mesmo assento não deve virar card E oferta passiva.
    let proposal_seats: std::collections::HashSet<String> =
        match get_season_by_number(conn, season)? {
            Some(s) => crate::db::queries::market_proposals::get_pending_player_proposals(
                conn, &s.id, &player.id,
            )
            .map_err(|e| format!("Falha ao carregar propostas pendentes: {e}"))?
            .into_iter()
            .map(|p| format!("{}#{}", p.equipe_id, p.papel.as_str()))
            .collect(),
            None => std::collections::HashSet::new(),
        };

    let mut offers: Vec<crate::market::transfer_window::PlayerOffer> = Vec::new();
    // Um time com as DUAS vagas abertas geraria duas ofertas (N1 e N2). O jogador só
    // deve receber UMA por time — fica com a de titular (N1) quando ambas existem.
    let mut offer_by_team: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for vac in find_vacancies(conn)?.into_iter().filter(is_regular_vacancy) {
        // Assento já ofertado como card formal → não duplica em "Suas ofertas".
        if proposal_seats.contains(&format!(
            "{}#{}",
            vac.team_id,
            vac.papel_necessario.as_str()
        )) {
            continue;
        }
        // Piso de RECOMEÇO: vaga de estreia (rookie) é sempre oferecida ao jogador,
        // mesmo fora do tier/licença — assim ele NUNCA fica sem nenhuma proposta.
        let is_debut = is_real_career_debut_category(&vac.categoria);
        // PROMOÇÃO: quem foi pódio recebe ofertas do tier ACIMA. A licença é concedida
        // ao assinar (grant_driver_license_for_division_if_needed), então o portão de
        // licença é dispensado aqui, como no caso de estreia.
        let is_promotion = player_podium && vac.category_tier == player_tier + 1;
        let required = crate::models::license::required_license_for_division(
            &vac.categoria,
            vac.classe.as_deref(),
        )
        .unwrap_or(0);
        if required > player_lic && !is_debut && !is_promotion {
            continue;
        }
        let in_tier = vac.category_tier == player_tier
            || (player_tier > 0 && vac.category_tier == player_tier - 1)
            || is_promotion;
        if !in_tier && !is_debut {
            continue;
        }
        let is_n1 = matches!(vac.papel_necessario, TeamRole::Numero1);
        let active_interest = interest_team_ids.contains(&vac.team_id);
        let salary =
            player_offer_salary_with_interest(&vac, is_n1, player.atributos.skill, active_interest);
        let offer = crate::market::transfer_window::PlayerOffer {
            seat_id: format!("{}#{}", vac.team_id, vac.papel_necessario.as_str()),
            team_id: vac.team_id.clone(),
            category: vac.categoria.clone(),
            class: vac.classe.clone(),
            salary,
            is_n1,
            active_interest,
        };
        if let Some(&idx) = offer_by_team.get(&vac.team_id) {
            // Já há oferta deste time: só troca se a nova for titular (N1) e a atual for N2.
            if is_n1 && !offers[idx].is_n1 {
                offers[idx] = offer;
            }
            continue;
        }
        offer_by_team.insert(vac.team_id.clone(), offers.len());
        offers.push(offer);
    }

    // Piso de RECOMEÇO: se nenhuma vaga no tier/tier−1/estreia abriu, mostra as vagas que
    // a escada reservou pro jogador (`player_reserved_seats` usa o fallback "qualquer vaga
    // licenciada"). Sem isso, um agente livre há tempo teria vagas reservadas
    // silenciosamente pela escada e NUNCA as veria ofertadas → zero propostas.
    if offers.is_empty() {
        let held = player_reserved_seats(conn, season)?;
        if !held.is_empty() {
            let vacs = find_vacancies(conn)?;
            for seat_id in held {
                if let Some(vac) = vacs
                    .iter()
                    .find(|v| format!("{}#{}", v.team_id, v.papel_necessario.as_str()) == seat_id)
                {
                    let is_n1 = matches!(vac.papel_necessario, TeamRole::Numero1);
                    let active_interest = interest_team_ids.contains(&vac.team_id);
                    offers.push(crate::market::transfer_window::PlayerOffer {
                        seat_id,
                        team_id: vac.team_id.clone(),
                        category: vac.categoria.clone(),
                        class: vac.classe.clone(),
                        salary: player_offer_salary_with_interest(
                            vac,
                            is_n1,
                            player.atributos.skill,
                            active_interest,
                        ),
                        is_n1,
                        active_interest,
                    });
                }
            }
        }
    }

    Ok(offers)
}

/// Assina o jogador na vaga `seat_id` ("team#papel"): valida que a vaga existe e é
/// dele (elegível+licenciada), concede a licença e contrata (1 ano). Erro se a vaga
/// sumiu ou não é elegível.
pub(crate) fn sign_player_to_vacancy(
    conn: &Connection,
    season: i32,
    seat_id: &str,
) -> Result<(), String> {
    let player = driver_queries::get_player_driver(conn)
        .map_err(|e| format!("Falha ao carregar piloto do jogador: {e}"))?;
    let vac = find_vacancies(conn)?
        .into_iter()
        .find(|v| format!("{}#{}", v.team_id, v.papel_necessario.as_str()) == seat_id)
        .ok_or_else(|| format!("Vaga '{seat_id}' nao encontrada."))?;

    // A vaga deve ser do jogador: licenciada (ou de estreia). Não exigimos in_tier aqui
    // porque `player_market_offers` pode ofertar a vaga reservada fora do tier (piso de
    // recomeço, "qualquer vaga licenciada") — o sign precisa aceitar tudo que é ofertado.
    let player_lic = player_effective_license(conn, &player)?;
    // Vaga de estreia (rookie) é sempre aceitável (piso de recomeço), mesmo sem licença.
    let is_debut = is_real_career_debut_category(&vac.categoria);
    // Promoção do pódio (tier acima): também aceitável — a licença é concedida logo
    // abaixo. Espelha a elegibilidade de `player_market_offers`.
    let player_tier = player_market_tier(conn, &player)?;
    let is_promotion = vac.category_tier == player_tier + 1
        && player_last_finish_position(conn, &player.id).is_some_and(|pos| (1..=3).contains(&pos));
    let required = crate::models::license::required_license_for_division(
        &vac.categoria,
        vac.classe.as_deref(),
    )
    .unwrap_or(0);
    if !is_debut && !is_promotion && required > player_lic {
        return Err(format!(
            "Vaga '{seat_id}' nao esta disponivel para o jogador."
        ));
    }

    grant_driver_license_for_division_if_needed(
        conn,
        &player.id,
        &vac.categoria,
        vac.classe.as_deref(),
    )?;
    let is_n1 = matches!(vac.papel_necessario, TeamRole::Numero1);
    // Ideia 4: a duração honra o Foco do time + o Vínculo do jogador com ele — um
    // time-casa/leal assina o jogador num contrato de projeto plurianual (o mesmo
    // número mostrado na oferta). Falha na leitura → 1 ano (neutro).
    let vinculo = crate::market::bond::get_bond(conn, &player.id, &vac.team_id).unwrap_or(0.0);
    let foco = crate::finance::focus::get_focus(conn, &vac.team_id)
        .map(|(f, _)| f)
        .unwrap_or(crate::finance::focus::TeamFocus::MeioDeGrid);
    let duration = crate::market::renewal::player_offer_duration(foco, vinculo);
    // O salário assinado honra o prêmio de interesse ativo (o mesmo mostrado na oferta).
    let active_interest = player_active_interest_teams(conn, &player)?
        .iter()
        .any(|(id, _, _)| id == &vac.team_id);
    sign_driver_to_team(
        conn,
        &player,
        &vac,
        season,
        player_offer_salary_with_interest(&vac, is_n1, player.atributos.skill, active_interest),
        duration,
        vac.papel_necessario.clone(),
    )
}
