//! Vagas e lineup das equipes: normalizacao dos contratos regulares, encaixe de
//! pilotos, propostas emergenciais ao jogador e reposicao de assentos vazios.

use super::*;

pub(crate) fn normalize_regular_contracts_for_team(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<bool, String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar equipe para normalizar contratos: {e}"))?
        .ok_or_else(errors::team_not_found_for_contracts)?;
    let mut active_regular_contracts =
        contract_queries::get_active_contracts_for_team(conn, team_id)
            .map_err(|e| format!("Falha ao carregar contratos ativos da equipe: {e}"))?
            .into_iter()
            .filter(|contract| contract.tipo == crate::models::enums::ContractType::Regular)
            .collect::<Vec<_>>();
    let drivers_by_id = active_regular_contracts
        .iter()
        .filter_map(|contract| {
            driver_queries::get_driver(conn, &contract.piloto_id)
                .ok()
                .map(|driver| (contract.piloto_id.clone(), driver))
        })
        .collect::<HashMap<_, _>>();
    active_regular_contracts.sort_by(|a, b| {
        let a_is_player = drivers_by_id
            .get(&a.piloto_id)
            .map(|driver| driver.is_jogador)
            .unwrap_or(false);
        let b_is_player = drivers_by_id
            .get(&b.piloto_id)
            .map(|driver| driver.is_jogador)
            .unwrap_or(false);
        b_is_player
            .cmp(&a_is_player)
            .then_with(|| b.temporada_inicio.cmp(&a.temporada_inicio))
            .then_with(|| b.created_at.cmp(&a.created_at))
            .then_with(|| b.id.cmp(&a.id))
    });

    let mut keep_n1 = None;
    let mut keep_n2 = None;
    let mut displaced_driver_ids = HashSet::new();
    let mut contract_ids_in_slots = HashSet::new();
    let mut role_fixed = false;

    for contract in active_regular_contracts {
        if contract_ids_in_slots.contains(&contract.id) {
            continue;
        }
        let slot = match contract.papel {
            TeamRole::Numero1 => &mut keep_n1,
            TeamRole::Numero2 => &mut keep_n2,
        };
        if slot.is_none() {
            contract_ids_in_slots.insert(contract.id.clone());
            *slot = Some(contract);
            continue;
        }

        if keep_n1.is_none() {
            contract_ids_in_slots.insert(contract.id.clone());
            keep_n1 = Some(contract);
        } else if keep_n2.is_none() {
            contract_ids_in_slots.insert(contract.id.clone());
            keep_n2 = Some(contract);
        } else {
            contract_queries::update_contract_status(
                conn,
                &contract.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato regular excedente '{}': {e}",
                    contract.id
                )
            })?;
            displaced_driver_ids.insert(contract.piloto_id);
        }
    }

    if let Some(contract) = &keep_n1 {
        if contract.papel != TeamRole::Numero1 {
            conn.execute(
                "UPDATE contracts SET papel = 'Numero1' WHERE id = ?1",
                rusqlite::params![&contract.id],
            )
            .map_err(|e| {
                format!(
                    "Falha ao alinhar papel Numero1 do contrato '{}': {e}",
                    contract.id
                )
            })?;
            role_fixed = true;
        }
    }

    if let Some(contract) = &keep_n2 {
        if contract.papel != TeamRole::Numero2 {
            conn.execute(
                "UPDATE contracts SET papel = 'Numero2' WHERE id = ?1",
                rusqlite::params![&contract.id],
            )
            .map_err(|e| {
                format!(
                    "Falha ao alinhar papel Numero2 do contrato '{}': {e}",
                    contract.id
                )
            })?;
            role_fixed = true;
        }
    }

    let piloto_1 = keep_n1.as_ref().map(|contract| contract.piloto_id.as_str());
    let piloto_2 = keep_n2.as_ref().map(|contract| contract.piloto_id.as_str());
    let changed = team.piloto_1_id.as_deref() != piloto_1
        || team.piloto_2_id.as_deref() != piloto_2
        || !displaced_driver_ids.is_empty()
        || role_fixed;

    if team.piloto_1_id.as_deref() != piloto_1 || team.piloto_2_id.as_deref() != piloto_2 {
        team_queries::update_team_pilots(conn, team_id, piloto_1, piloto_2)
            .map_err(|e| format!("Falha ao atualizar lineup da equipe '{}': {e}", team.nome))?;
    }

    for driver_id in displaced_driver_ids {
        if contract_queries::get_active_contract_for_pilot(conn, &driver_id)
            .map_err(|e| {
                format!(
                    "Falha ao verificar contrato remanescente de '{}': {e}",
                    driver_id
                )
            })?
            .is_some()
        {
            continue;
        }
        let mut driver = driver_queries::get_driver(conn, &driver_id)
            .map_err(|e| format!("Falha ao carregar piloto deslocado '{}': {e}", driver_id))?;
        if driver.categoria_atual.is_none() {
            continue;
        }
        driver.categoria_atual = None;
        driver_queries::update_driver(conn, &driver).map_err(|e| {
            format!(
                "Falha ao limpar categoria do piloto deslocado '{}': {e}",
                driver_id
            )
        })?;
    }

    Ok(changed)
}

pub(crate) fn place_driver_in_team(
    conn: &rusqlite::Connection,
    team_id: &str,
    driver_id: &str,
    role: TeamRole,
) -> Result<(), String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar equipe para encaixar jogador: {e}"))?
        .ok_or_else(errors::team_not_found_for_placement)?;
    let existing = [team.piloto_1_id.clone(), team.piloto_2_id.clone()]
        .into_iter()
        .flatten()
        .filter(|id| id != driver_id)
        .collect::<Vec<_>>();
    let (piloto_1, piloto_2) = match role {
        TeamRole::Numero1 => (Some(driver_id.to_string()), existing.first().cloned()),
        TeamRole::Numero2 => (existing.first().cloned(), Some(driver_id.to_string())),
    };
    team_queries::update_team_pilots(conn, team_id, piloto_1.as_deref(), piloto_2.as_deref())
        .map_err(|e| format!("Falha ao atualizar pilotos da nova equipe: {e}"))?;
    Ok(())
}

pub(crate) fn refresh_team_hierarchy_now(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<(), String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar equipe para hierarquia: {e}"))?
        .ok_or_else(errors::team_not_found_for_hierarchy)?;
    let mut candidates = [team.piloto_1_id.clone(), team.piloto_2_id.clone()]
        .into_iter()
        .flatten()
        .filter_map(|id| driver_queries::get_driver(conn, &id).ok())
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| b.atributos.skill.total_cmp(&a.atributos.skill));
    let n1_id = candidates.first().map(|driver| driver.id.as_str());
    let n2_id = candidates.get(1).map(|driver| driver.id.as_str());
    team_queries::update_team_hierarchy(
        conn,
        team_id,
        n1_id,
        n2_id,
        TeamHierarchyClimate::Estavel.as_str(),
        0.0,
    )
    .map_err(|e| format!("Falha ao atualizar hierarquia da equipe: {e}"))?;
    Ok(())
}

#[derive(Clone)]
pub(crate) struct TeamVacancy {
    team: Team,
    role: TeamRole,
}

pub(crate) fn list_team_vacancies(conn: &rusqlite::Connection) -> Result<Vec<TeamVacancy>, String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao listar equipes: {e}"))?;
    let mut vacancies = Vec::new();
    for team in teams {
        if team.piloto_1_id.is_none() {
            vacancies.push(TeamVacancy {
                team: team.clone(),
                role: TeamRole::Numero1,
            });
        }
        if team.piloto_2_id.is_none() {
            vacancies.push(TeamVacancy {
                team,
                role: TeamRole::Numero2,
            });
        }
    }
    Ok(vacancies)
}

/// O painel de mercado do jogador FORA da janela de pré-temporada: os assentos
/// vazios do mundo, cada um com o veredito de elegibilidade já resolvido.
///
/// A regra de elegibilidade é a MESMA de [`generate_emergency_player_proposals`] —
/// licença da divisão mais faixa de tier (o tier do jogador ou um degrau acima).
/// Ela não é reimplementada aqui: `licenca_ok` chama o mesmo
/// `driver_has_required_license_for_division`, e `tier_ok` repete o mesmo intervalo.
/// A diferença é o propósito: lá a regra FILTRA (o mercado só oferta o que cabe),
/// aqui ela ANOTA — o jogador tem o direito de ver a cadeira que abriu na categoria
/// de cima e saber que ela não é para ele ainda.
///
/// Read-only: nada aqui grava proposta, contrato ou assento.
pub(crate) fn get_season_market_board_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<SeasonMarketBoard, String> {
    let (db, _dir, _meta) = open_career_resources_read_only(base_dir, career_id)?;
    let conn = &db.conn;

    let player = driver_queries::get_player_driver(conn)
        .map_err(|e| format!("Falha ao carregar o piloto do jogador: {e}"))?;

    let player_categoria = player
        .categoria_atual
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let player_tier = player_categoria
        .as_deref()
        .and_then(categories::get_category_config)
        .map(|config| config.tier);

    let mut vagas = Vec::new();
    for vacancy in list_team_vacancies(conn)? {
        let team = &vacancy.team;
        let seat_tier = categories::get_category_config(&team.categoria).map(|config| config.tier);
        // Sem tier do jogador não há faixa a comparar: a vaga fica anotada como fora
        // da faixa em vez de a tela afirmar que um agente livre sem categoria pode
        // ocupar qualquer assento do mundo.
        let tier_ok = match (player_tier, seat_tier) {
            (Some(mine), Some(theirs)) => theirs >= mine && theirs <= mine + 1,
            _ => false,
        };
        let licenca_ok = driver_has_required_license_for_division(
            conn,
            &player.id,
            &team.categoria,
            team.classe.as_deref(),
        )?;
        let elegivel = tier_ok && licenca_ok;

        vagas.push(OpenSeat {
            team_id: team.id.clone(),
            team_name: team.nome.clone(),
            team_color: team.cor_primaria.clone(),
            categoria: team.categoria.clone(),
            classe: team.classe.clone(),
            categoria_tier: seat_tier,
            papel: vacancy.role.as_str().to_string(),
            car_performance_rating: normalize_car_performance(team.effective_car_performance()),
            licenca_ok,
            tier_ok,
            salario_estimado: if elegivel {
                Some(calculate_offer_salary_for_team(team, &player))
            } else {
                None
            },
        });
    }

    // Elegíveis primeiro e, dentro de cada grupo, o melhor carro na frente: a
    // ordem responde "o que eu posso pegar, e o que vale mais" na mesma varredura.
    vagas.sort_by(|a, b| {
        let a_elegivel = a.licenca_ok && a.tier_ok;
        let b_elegivel = b.licenca_ok && b.tier_ok;
        b_elegivel
            .cmp(&a_elegivel)
            .then_with(|| b.car_performance_rating.cmp(&a.car_performance_rating))
            .then_with(|| a.team_name.cmp(&b.team_name))
    });

    let vagas_elegiveis = vagas
        .iter()
        .filter(|vaga| vaga.licenca_ok && vaga.tier_ok)
        .count() as i32;

    Ok(SeasonMarketBoard {
        player_categoria,
        player_tier,
        vagas,
        vagas_elegiveis,
    })
}

/// Vagas em que o jogador cabe: as que passam no filtro de tier E em que ele tem a
/// licença da divisão. A licença nunca é dispensada — é ela que impede um assento
/// acima do que o piloto pode dirigir.
///
/// `tier_ok` recebe o tier da categoria da vaga (`None` quando a categoria não tem
/// config) — cada chamador define a própria faixa.
///
/// `fallback_sem_tier` refaz a passada ignorando o tier quando a primeira não achou
/// nada. Ligado só no encaixe forçado, onde deixar o jogador sem assento é pior que
/// dar um assento fora da faixa; a proposta emergencial NÃO usa, porque ali uma vaga
/// fora da faixa seria uma oferta que o mercado não faria.
fn player_eligible_vacancies(
    conn: &rusqlite::Connection,
    player: &Driver,
    fallback_sem_tier: bool,
    tier_ok: impl Fn(Option<u8>) -> bool,
) -> Result<Vec<TeamVacancy>, String> {
    let mut vacancies = collect_licensed_vacancies(conn, player, &tier_ok)?;
    if vacancies.is_empty() && fallback_sem_tier {
        vacancies = collect_licensed_vacancies(conn, player, |_| true)?;
    }
    Ok(vacancies)
}

fn collect_licensed_vacancies(
    conn: &rusqlite::Connection,
    player: &Driver,
    tier_ok: impl Fn(Option<u8>) -> bool,
) -> Result<Vec<TeamVacancy>, String> {
    let mut vacancies = Vec::new();
    for vacancy in list_team_vacancies(conn)? {
        let tier = categories::get_category_config(&vacancy.team.categoria).map(|c| c.tier);
        if tier_ok(tier)
            && driver_has_required_license_for_division(
                conn,
                &player.id,
                &vacancy.team.categoria,
                vacancy.team.classe.as_deref(),
            )?
        {
            vacancies.push(vacancy);
        }
    }
    Ok(vacancies)
}

pub(crate) fn generate_emergency_player_proposals(
    conn: &rusqlite::Connection,
    player: &Driver,
    season: &Season,
) -> Result<Vec<MarketProposal>, String> {
    let player_tier = player
        .categoria_atual
        .as_deref()
        .and_then(categories::get_category_config)
        .map(|config| config.tier)
        .unwrap_or(0);
    // A categoria do próprio tier ou UM degrau acima. Sem fallback: se nada cabe na
    // faixa, o jogador não recebe proposta emergencial.
    let mut vacancies = player_eligible_vacancies(conn, player, false, |tier| {
        let tier = tier.unwrap_or(0);
        tier >= player_tier && tier <= player_tier + 1
    })?;
    // Melhor vaga = melhor CARRO efetivo (peças > coluna legada). Num grid spec ninguém
    // desempata pelo pacote e a ordem de entrada manda — que é a verdade da pista.
    vacancies.sort_by(|a, b| {
        b.team
            .effective_car_performance()
            .total_cmp(&a.team.effective_car_performance())
    });

    let mut created = Vec::new();
    for (index, vacancy) in vacancies.into_iter().take(2).enumerate() {
        let proposal = MarketProposal {
            id: format!(
                "MP-{}-{}-{}-EM-{}",
                season.numero, vacancy.team.id, player.id, index
            ),
            equipe_id: vacancy.team.id.clone(),
            equipe_nome: vacancy.team.nome.clone(),
            piloto_id: player.id.clone(),
            piloto_nome: player.nome.clone(),
            categoria: vacancy.team.categoria.clone(),
            papel: vacancy.role.clone(),
            salario_oferecido: calculate_offer_salary_for_team(&vacancy.team, player),
            duracao_anos: if categories::get_category_config(&vacancy.team.categoria)
                .map(|config| config.tier >= 3)
                .unwrap_or(false)
            {
                2
            } else {
                1
            },
            status: ProposalStatus::Pendente,
            motivo_recusa: None,
        };
        market_proposal_queries::insert_player_proposal(conn, &season.id, &proposal)
            .map_err(|e| format!("Falha ao persistir proposta emergencial: {e}"))?;
        created.push(proposal);
    }

    Ok(created)
}

pub(crate) fn force_place_player(
    conn: &rusqlite::Connection,
    player: &Driver,
    season: &Season,
    _news_items: &mut Vec<NewsItem>,
) -> Result<Option<String>, String> {
    let player_tier = player
        .categoria_atual
        .as_deref()
        .and_then(categories::get_category_config)
        .map(|config| config.tier)
        .unwrap_or(0);
    // Encaixe forçado: só o MESMO tier (categoria sem config não serve) e, se não houver
    // nenhuma, qualquer vaga licenciada — o jogador não pode ficar de fora do grid.
    let mut vacancies =
        player_eligible_vacancies(conn, player, true, |tier| tier == Some(player_tier))?;
    vacancies.sort_by(|a, b| {
        a.team
            .effective_car_performance()
            .total_cmp(&b.team.effective_car_performance())
    });
    let Some(vacancy) = vacancies.into_iter().next() else {
        return Ok(None);
    };
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("Falha ao iniciar transacao de alocacao forcada: {e}"))?;
    ensure_driver_can_join_division(
        &tx,
        &player.id,
        &player.nome,
        &vacancy.team.categoria,
        vacancy.team.classe.as_deref(),
    )?;

    let mut contract = crate::models::contract::Contract::new(
        next_id(&tx, IdType::Contract)
            .map_err(|e| format!("Falha ao gerar contrato forçado: {e}"))?,
        player.id.clone(),
        player.nome.clone(),
        vacancy.team.id.clone(),
        vacancy.team.nome.clone(),
        season.numero,
        1,
        calculate_offer_salary_for_team(&vacancy.team, player).max(5_000.0),
        vacancy.role.clone(),
        vacancy.team.categoria.clone(),
    );
    contract.classe = vacancy.team.classe.clone();
    contract_queries::insert_contract(&tx, &contract)
        .map_err(|e| format!("Falha ao inserir contrato forçado: {e}"))?;
    place_driver_in_team(&tx, &vacancy.team.id, &player.id, vacancy.role.clone())?;
    refresh_team_hierarchy_now(&tx, &vacancy.team.id)?;
    let mut updated_player = player.clone();
    updated_player.mover_para_categoria(Some(vacancy.team.categoria.clone()));
    updated_player.status = crate::models::enums::DriverStatus::Ativo;
    driver_queries::update_driver(&tx, &updated_player)
        .map_err(|e| format!("Falha ao atualizar jogador apos alocacao forcada: {e}"))?;
    tx.commit()
        .map_err(|e| format!("Falha ao confirmar alocacao forcada: {e}"))?;
    Ok(Some(vacancy.team.nome))
}

pub(crate) fn backfill_team_vacancy(
    conn: &rusqlite::Connection,
    team_id: &str,
    season_number: i32,
    season_year: i32,
) -> Result<(), String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar equipe para reposicao: {e}"))?
        .ok_or_else(errors::team_not_found_for_replacement)?;
    let role = if team.piloto_1_id.is_none() {
        TeamRole::Numero1
    } else if team.piloto_2_id.is_none() {
        TeamRole::Numero2
    } else {
        return Ok(());
    };

    let free_driver = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos para reposicao: {e}"))?
        .into_iter()
        .filter(|driver| driver.status == crate::models::enums::DriverStatus::Ativo)
        .filter(|driver| {
            contract_queries::get_active_regular_contract_for_pilot(conn, &driver.id)
                .ok()
                .flatten()
                .is_none()
        })
        .filter(|driver| {
            driver_has_required_license_for_division(
                conn,
                &driver.id,
                &team.categoria,
                team.classe.as_deref(),
            )
            .unwrap_or(false)
        })
        .max_by(|a, b| a.atributos.skill.total_cmp(&b.atributos.skill));

    let replacement = if let Some(driver) = free_driver {
        driver
    } else {
        let mut existing_names = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao carregar nomes existentes: {e}"))?
            .into_iter()
            .map(|driver| driver.nome)
            .collect::<HashSet<_>>();
        let mut rng = rand::thread_rng();
        let mut rookie = crate::evolution::rookies::generate_rookies(
            1,
            season_year,
            &mut existing_names,
            &mut rng,
        )
        .into_iter()
        .next()
        .ok_or_else(errors::rookie_generation_failed)?;
        rookie.id = format!(
            "P-EM-{}",
            next_id(conn, IdType::Driver)
                .map_err(|e| format!("Falha ao gerar ID emergencial: {e}"))?
        );
        driver_queries::insert_driver(conn, &rookie)
            .map_err(|e| format!("Falha ao inserir rookie emergencial: {e}"))?;
        grant_driver_license_for_division_if_needed(
            conn,
            &rookie.id,
            &team.categoria,
            team.classe.as_deref(),
        )?;
        rookie
    };
    ensure_driver_can_join_division(
        conn,
        &replacement.id,
        &replacement.nome,
        &team.categoria,
        team.classe.as_deref(),
    )?;

    let mut contract = crate::models::contract::Contract::new(
        next_id(conn, IdType::Contract)
            .map_err(|e| format!("Falha ao gerar contrato de reposicao: {e}"))?,
        replacement.id.clone(),
        replacement.nome.clone(),
        team.id.clone(),
        team.nome.clone(),
        season_number,
        1,
        calculate_offer_salary_for_team(&team, &replacement).max(5_000.0),
        role.clone(),
        team.categoria.clone(),
    );
    contract.classe = team.classe.clone();
    contract_queries::insert_contract(conn, &contract)
        .map_err(|e| format!("Falha ao inserir contrato de reposicao: {e}"))?;
    place_driver_in_team(conn, &team.id, &replacement.id, role)?;
    let mut updated_driver = replacement.clone();
    updated_driver.mover_para_categoria(Some(team.categoria.clone()));
    driver_queries::update_driver(conn, &updated_driver)
        .map_err(|e| format!("Falha ao atualizar piloto de reposicao: {e}"))?;
    Ok(())
}

pub(crate) fn calculate_offer_salary_for_team(team: &Team, player: &Driver) -> f64 {
    calculate_offer_salary_from_money(team, player.atributos.skill)
}

pub(crate) fn normalize_car_performance(car_performance: f64) -> u8 {
    (((car_performance + 5.0) / 21.0) * 100.0)
        .round()
        .clamp(0.0, 100.0) as u8
}
