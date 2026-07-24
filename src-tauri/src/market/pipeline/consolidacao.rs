//! Consolidação da grade: a última etapa da entressafra, onde nenhuma vaga pode
//! sobrar aberta.
//!
//! Rebaixa por mérito, garante a subida do campeão pela escada, faz o recrutamento
//! profundo nas categorias de baixo, gera rookies só na base quando falta gente e,
//! no fim, recalcula a hierarquia (N1/N2) de cada equipe.

use super::*;

/// (Experimento) Liga a "subida garantida do campeão do Rookie": quando o Amador
/// está cheio, força a troca do 1º do Rookie com o pior do Amador. Off por padrão;
/// ligue com `IRACER_ROOKIE_MERIT=1` (ou `=true`) para o A/B no harness sim_stats.
/// Ver `guarantee_rookie_champion_promotions`.
pub(super) fn rookie_merit_enabled() -> bool {
    std::env::var("IRACER_ROOKIE_MERIT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// (Anti-deflação da grade) Liga o "mercado realista" na escada viva de contratações,
/// em duas frentes complementares:
///  1. ORDEM DE ESCOLHA dos assentos passa a ponderar prestígio (reputação da equipe,
///     já carregada na vaga) além do carro — port do score de assento do motor de janela
///     (`transfer_window::driver_offer_score`, pesos `w_car`/`w_prestige`) — para o melhor
///     carro numa equipe prestigiada escolher do pool ANTES de um carro igual sem tradição.
///  2. SELEÇÃO do candidato passa a penalizar quem o assento NÃO PODE PAGAR: o preço de
///     mercado do piloto acima do teto salarial derivado do poder de gasto da equipe vira
///     penalidade, fazendo um time sem caixa DESCER para um piloto mais barato em vez de
///     assinar sempre o melhor agente livre (Problema 1: finanças limitavam o SALÁRIO, não
///     a SELEÇÃO). Penalidade SOFT (nunca filtra) para preservar a invariante de grid e
///     evitar re-scans em cascata que já travaram o sim multi-temporada.
///
/// LIGADO por padrão; desligue com `IRACER_MARKET_AFFORDABILITY=0` (ou `false`/`off`) para
/// o A/B no harness sim_stats (comparar a distribuição de skill do topo da grade com/sem).
pub(super) fn market_affordability_enabled() -> bool {
    std::env::var("IRACER_MARKET_AFFORDABILITY")
        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false") || v.eq_ignore_ascii_case("off")))
        .unwrap_or(true)
}

pub(super) fn fill_remaining_vacancies_with_rookies(
    conn: &Connection,
    teams: &[crate::models::team::Team],
    new_season_number: i32,
    report: &mut MarketReport,
    rng: &mut impl Rng,
    limit: Option<usize>,
    reserved: &HashSet<String>,
) -> Result<(), String> {
    let debut_year = get_season_by_number(conn, new_season_number)?
        .map(|season| season.ano)
        .unwrap_or_else(|| Local::now().year());

    // Necessidade financeira por time (Fase 2a): quanto o time pesa a fama de um
    // candidato. Carente pesa alto (precisa do patrocínio); dinastia rica pesa baixo.
    let team_need_by_id: HashMap<String, f64> = teams
        .iter()
        .map(|team| {
            let budget_index = crate::finance::planning::derive_budget_index_from_money(team);
            (
                team.id.clone(),
                crate::fame::team_need_factor(budget_index, team.reputacao),
            )
        })
        .collect();

    // Teto salarial por time (Item 1): quanto a folha de UM piloto do time comporta,
    // derivado do poder de gasto (`calculate_salary_ceiling` já pondera caixa, dívida,
    // estado financeiro e reputação). Alimenta a penalidade de affordability na seleção.
    // Vazio quando a flag está off → seleção volta ao comportamento antigo (sem penalidade).
    let team_ceiling_by_id: HashMap<String, f64> = if market_affordability_enabled() {
        teams
            .iter()
            .map(|team| {
                (
                    team.id.clone(),
                    crate::finance::salary::calculate_salary_ceiling(team),
                )
            })
            .collect()
    } else {
        HashMap::new()
    };

    loop {
        let current_drivers = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao recarregar pilotos: {e}"))?;
        let current_by_id: HashMap<String, Driver> = current_drivers
            .iter()
            .cloned()
            .map(|driver| (driver.id.clone(), driver))
            .collect();
        sync_team_slots(conn, teams, &current_by_id)?;
        let mut vacancies: Vec<_> = find_vacancies(conn)?
            .into_iter()
            .filter(is_regular_vacancy)
            .filter(|vacancy| is_category_active_in_year(&vacancy.categoria, debut_year))
            .filter(|v| {
                !reserved.contains(&format!("{}#{}", v.team_id, v.papel_necessario.as_str()))
            })
            .collect();
        if vacancies.is_empty() {
            break;
        }
        // Preenche as vagas de TOPO primeiro (tier decrescente). `find_vacancies`
        // devolve na ordem dos times; sem ordenar, um craque livre no pool de resgate
        // passa o piso de quase toda vaga e é assinado pela 1ª que aparece na
        // iteração (amador/gt4) ANTES de a vaga de GT3/endurance ser processada —
        // enterrando o talento num tier baixo. Ordenando por tier desc, o topo
        // escolhe do pool antes das categorias inferiores o capturarem.
        //
        // Desempate DENTRO do tier por DESEJABILIDADE do assento decrescente: cada vaga
        // pega o MELHOR candidato do pool (max por `compare_pool_fallback_candidates`),
        // logo processar o assento mais desejável primeiro faz o melhor assento ficar com
        // o melhor piloto disponível. Sem esse desempate, a ordem de times era arbitrária
        // e um assento pior do mesmo tier abocanhava o craque antes do melhor — a raiz do
        // "melhor carro ≠ melhor piloto" que deflaciona a grade.
        //
        // Com o mercado realista (flag), desejabilidade = carro + PRESTÍGIO (reputação da
        // equipe), port do score de assento do motor de janela — o melhor carro numa
        // equipe prestigiada escolhe antes de um carro igual sem tradição. Sem a flag,
        // desempata só por `car_performance` (comportamento antigo). `sort_by` é estável,
        // então assentos empatados (ex.: N1/N2 do mesmo time) preservam a ordem original.
        let use_market_realism = market_affordability_enabled();
        vacancies.sort_by(|a, b| {
            b.category_tier.cmp(&a.category_tier).then_with(|| {
                if use_market_realism {
                    seat_desirability(b)
                        .partial_cmp(&seat_desirability(a))
                        .unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    b.car_strength
                        .partial_cmp(&a.car_strength)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }
            })
        });

        let previous_season = get_season_by_number(conn, new_season_number - 1)?;
        let market_contexts = load_market_contexts(
            conn,
            previous_season.as_ref().map(|season| season.id.as_str()),
            &current_by_id,
            &HashMap::new(),
        )?;
        let mut available = find_available_drivers(conn, &market_contexts)?;
        let license_levels = load_max_license_levels(conn)?;
        let mut filled_any = false;
        for vacancy in vacancies {
            // Pacing: o chamador paginado passa um report novo, logo a contagem de
            // assinaturas deste relatório == as feitas nesta chamada. Ao atingir o
            // limite, encerra (as vagas restantes ficam pra próxima semana/fecho).
            if limit.is_some_and(|l| report.new_signings.len() >= l) {
                return Ok(());
            }
            let need_factor = team_need_by_id
                .get(&vacancy.team_id)
                .copied()
                .unwrap_or(crate::fame::TEAM_NEED_MIN);
            // `None` (flag off / time sem teto) → sem penalidade de affordability.
            let team_ceiling = team_ceiling_by_id.get(&vacancy.team_id).copied();
            let is_debut_vacancy = is_real_career_debut_category(&vacancy.categoria)
                || is_entry_category_for_year(&vacancy.categoria, debut_year);
            let fallback_index = available
                .iter()
                .enumerate()
                .filter(|(_, candidate)| is_pool_fallback_candidate(candidate, &vacancy))
                .max_by(|(_, a), (_, b)| {
                    compare_pool_fallback_candidates(a, b, &vacancy, need_factor, team_ceiling)
                })
                .map(|(index, _)| index);

            // O pool de resgate roda primeiro (mais barato que a cascata de promoção).
            // O item B (piso de skill em is_pool_fallback_candidate) já barra órfão fraco
            // aqui, então não é preciso reordenar antes da promoção meritória — reordenar
            // disparava re-scans em cascata a cada promoção e travava o sim multi-temporada.
            if let Some(index) = fallback_index {
                let candidate = available.remove(index);
                grant_driver_license_for_division_if_needed(
                    conn,
                    &candidate.driver.id,
                    &vacancy.categoria,
                    vacancy.classe.as_deref(),
                )?;
                sign_driver_to_team(
                    conn,
                    &candidate.driver,
                    &vacancy,
                    new_season_number,
                    calculate_offer_salary(&vacancy, &candidate.driver, rng),
                    1,
                    vacancy.papel_necessario.clone(),
                )?;
                let signing_type = if is_real_career_debut_category(&vacancy.categoria) {
                    report.rookies_placed += 1;
                    "rookie"
                } else {
                    "transferencia"
                };
                report.new_signings.push(SigningInfo {
                    driver_id: candidate.driver.id.clone(),
                    driver_name: candidate.driver.nome.clone(),
                    team_id: vacancy.team_id.clone(),
                    team_name: vacancy.team_name.clone(),
                    categoria: vacancy.categoria.clone(),
                    papel: vacancy.papel_necessario.as_str().to_string(),
                    tipo: signing_type.to_string(),
                });
                filled_any = true;
                continue;
            }

            if is_debut_vacancy {
                let rookie = generate_and_sign_rookie_for_vacancy(
                    conn,
                    &vacancy,
                    new_season_number,
                    debut_year,
                    rng,
                )?;
                report.rookies_placed += 1;
                report.new_signings.push(SigningInfo {
                    driver_id: rookie.id.clone(),
                    driver_name: rookie.nome.clone(),
                    team_id: vacancy.team_id.clone(),
                    team_name: vacancy.team_name.clone(),
                    categoria: vacancy.categoria.clone(),
                    papel: vacancy.papel_necessario.as_str().to_string(),
                    tipo: "rookie".to_string(),
                });
                filled_any = true;
                continue;
            }

            // Sistema de escada (modelo fechado): se o pool não cobre uma vaga
            // não-estreia, promovemos o melhor piloto da categoria de baixo em vez
            // de gerar um piloto novo do nada ou abortar. Promover abre o assento
            // dele lá embaixo, que será preenchido na próxima volta do loop — a
            // cascata desce até a categoria de estreia, onde aí sim nasce um rookie.
            // Portão de MÉRITO na escada regular. Categorias de fase especial
            // (endurance/production) hoje mantêm o comportamento antigo (concede a
            // licença ao assinar) — endurecer isso (gate de licença real) desestabilizou
            // o sim multi-temporada; fica para uma investigação à parte de #3.
            let is_special_vacancy = runs_in_special_phase(&vacancy.categoria);
            let required_license = if is_special_vacancy {
                None
            } else {
                required_license_for_division(&vacancy.categoria, vacancy.classe.as_deref())
            };
            let feeder_candidate = best_feeder_promotion_candidate(
                &vacancy,
                &current_by_id,
                &market_contexts,
                &license_levels,
                required_license,
            );
            // Recrutamento profundo (demanda de time + aceite): para uma vaga de topo
            // mal servida pelo feeder, o time busca o craque preso nas categorias
            // inferiores e lhe faz proposta; o piloto decide. Prefere o recrutado que
            // aceitou; senão, segue a escada normal com o candidato do feeder.
            let deep_candidate = deep_recruitment_candidate(
                conn,
                &vacancy,
                &current_by_id,
                &market_contexts,
                &license_levels,
                required_license,
                feeder_candidate
                    .as_ref()
                    .map(|driver| driver.atributos.skill),
                rng,
            )?;
            let was_deep = deep_candidate.is_some();
            if let Some(candidate) = deep_candidate.or(feeder_candidate) {
                // Rescinde o contrato atual do piloto na categoria de baixo antes de
                // promovê-lo (o índice único (piloto_id, tipo) impede dois contratos
                // regulares ativos). Isso abre o assento dele lá embaixo.
                for contract in contract_queries::get_all_active_regular_contracts(conn)
                    .map_err(|e| format!("Falha ao carregar contrato do promovido: {e}"))?
                    .into_iter()
                    .filter(|contract| contract.piloto_id == candidate.id)
                {
                    contract_queries::update_contract_status(
                        conn,
                        &contract.id,
                        &ContractStatus::Rescindido,
                    )
                    .map_err(|e| format!("Falha ao rescindir contrato do promovido: {e}"))?;
                }
                if is_special_vacancy {
                    // Especiais: concede a licença da divisão ao assinar.
                    grant_driver_license_for_division_if_needed(
                        conn,
                        &candidate.id,
                        &vacancy.categoria,
                        vacancy.classe.as_deref(),
                    )?;
                }
                // Escada regular: sem concessão — o candidato JÁ possui a licença
                // exigida (filtro de mérito em best_feeder_promotion_candidate).
                sign_driver_to_team(
                    conn,
                    &candidate,
                    &vacancy,
                    new_season_number,
                    calculate_offer_salary(&vacancy, &candidate, rng),
                    1,
                    vacancy.papel_necessario.clone(),
                )?;
                if was_deep {
                    // Ligou os dois cérebros: proposta feita e ACEITA pelo craque da
                    // várzea (o feeder míope nunca o alcançaria).
                    report.proposals_made += 1;
                    report.proposals_accepted += 1;
                }
                report.new_signings.push(SigningInfo {
                    driver_id: candidate.id.clone(),
                    driver_name: candidate.nome.clone(),
                    team_id: vacancy.team_id.clone(),
                    team_name: vacancy.team_name.clone(),
                    categoria: vacancy.categoria.clone(),
                    papel: vacancy.papel_necessario.as_str().to_string(),
                    tipo: if was_deep { "recrutamento" } else { "promocao" }.to_string(),
                });
                filled_any = true;
                // Reescaneia do zero: o assento aberto na categoria de baixo vira a
                // próxima vaga a preencher (continua a cascata) e evita reusar o
                // mesmo candidato com dados defasados nesta passada.
                break;
            }

            // Escassez numa vaga REGULAR de categoria superior, sem candidato
            // meritorio. Deixar o assento vazio violaria a invariante de grid
            // (validate_and_normalize_team_hierarchies aborta a temporada). As
            // categorias especiais (endurance/production) NAO entram aqui.
            if is_special_vacancy {
                continue;
            }

            if let Some(candidate) = best_feeder_promotion_candidate(
                &vacancy,
                &current_by_id,
                &market_contexts,
                &license_levels,
                None,
            ) {
                for contract in contract_queries::get_all_active_regular_contracts(conn)
                    .map_err(|e| {
                        format!("Falha ao carregar contrato do promovido (emergencia): {e}")
                    })?
                    .into_iter()
                    .filter(|contract| contract.piloto_id == candidate.id)
                {
                    contract_queries::update_contract_status(
                        conn,
                        &contract.id,
                        &ContractStatus::Rescindido,
                    )
                    .map_err(|e| {
                        format!("Falha ao rescindir contrato do promovido (emergencia): {e}")
                    })?;
                }
                grant_driver_license_for_division_if_needed(
                    conn,
                    &candidate.id,
                    &vacancy.categoria,
                    vacancy.classe.as_deref(),
                )?;
                sign_driver_to_team(
                    conn,
                    &candidate,
                    &vacancy,
                    new_season_number,
                    calculate_offer_salary(&vacancy, &candidate, rng),
                    1,
                    vacancy.papel_necessario.clone(),
                )?;
                report.new_signings.push(SigningInfo {
                    driver_id: candidate.id.clone(),
                    driver_name: candidate.nome.clone(),
                    team_id: vacancy.team_id.clone(),
                    team_name: vacancy.team_name.clone(),
                    categoria: vacancy.categoria.clone(),
                    papel: vacancy.papel_necessario.as_str().to_string(),
                    tipo: "promocao_emergencia".to_string(),
                });
                EMERGENCY_PROMOTIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let from_tier = candidate
                    .categoria_atual
                    .as_deref()
                    .and_then(get_category_config)
                    .map(|c| c.tier)
                    .unwrap_or(99);
                let to_tier = get_category_config(&vacancy.categoria)
                    .map(|c| c.tier)
                    .unwrap_or(99);
                if let Ok(mut paths) = EMERGENCY_PROMO_PATHS.lock() {
                    paths.push((from_tier, to_tier));
                }
                filled_any = true;
                break;
            }

            let rookie = generate_and_sign_rookie_for_vacancy(
                conn,
                &vacancy,
                new_season_number,
                debut_year,
                rng,
            )?;
            report.rookies_placed += 1;
            report.new_signings.push(SigningInfo {
                driver_id: rookie.id.clone(),
                driver_name: rookie.nome.clone(),
                team_id: vacancy.team_id.clone(),
                team_name: vacancy.team_name.clone(),
                categoria: vacancy.categoria.clone(),
                papel: vacancy.papel_necessario.as_str().to_string(),
                tipo: "rookie_emergencia".to_string(),
            });
            EMERGENCY_ROOKIES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            filled_any = true;
            break;
        }

        if !filled_any {
            // Nenhuma vaga preenchível nesta passada: para de tentar (evita loop
            // infinito); as vagas restantes ficam abertas até a próxima preseason.
            break;
        }
    }

    Ok(())
}

/// Wrapper paginado da escada (ladder fill): carrega as equipes e chama
/// `fill_remaining_vacancies_with_rookies` com um teto de assinaturas (`limit`) e um
/// conjunto de assentos reservados (não preenche). Usado pela Janela ao vivo —
/// `preseason.rs` não precisa carregar `teams`.
pub(crate) fn fill_vacancies_paced(
    conn: &Connection,
    season: i32,
    limit: Option<usize>,
    reserved: &HashSet<String>,
    report: &mut MarketReport,
    rng: &mut impl Rng,
) -> Result<(), String> {
    let teams = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao carregar equipes para a escada paginada: {e}"))?;
    fill_remaining_vacancies_with_rookies(conn, &teams, season, report, rng, limit, reserved)
}

/// Rebaixamento por MÉRITO (modelo fechado, conservação preservada): em cada
/// categoria regular, se o melhor piloto licenciado da categoria de baixo foi
/// campeão/vice e o pior piloto da categoria terminou no fundo (penúltimo/último),
/// os dois TROCAM de assento — um sobe, um desce. Conservador: no máximo 1 troca
/// por categoria por temporada e nunca mexe no piloto jogador.
pub(super) fn apply_merit_relegations(
    conn: &Connection,
    teams: &[crate::models::team::Team],
    new_season_number: i32,
    contexts: &HashMap<String, DriverMarketContext>,
    report: &mut MarketReport,
) -> Result<(), String> {
    let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos para rebaixamento: {e}"))?
        .into_iter()
        .map(|driver| (driver.id.clone(), driver))
        .collect();
    let license_levels = load_max_license_levels(conn)?;

    // Categorias regulares (não-estreia, não-especiais), do topo para a base.
    let mut categories: Vec<_> = get_all_categories()
        .iter()
        .filter(|category| {
            uses_regular_contracts(category.id)
                && !runs_in_special_phase(category.id)
                && !is_real_career_debut_category(category.id)
        })
        .collect();
    categories.sort_by(|a, b| b.tier.cmp(&a.tier));

    // O rebaixamento automático nunca mexe no time do jogador — ele controla o
    // próprio elenco (e mexer ali quebraria o plano de pré-temporada).
    let player_team_ids: HashSet<&str> = teams
        .iter()
        .filter(|team| team.is_player_team)
        .map(|team| team.id.as_str())
        .collect();
    let is_active_non_player = |id: &str| {
        drivers_by_id
            .get(id)
            .is_some_and(|driver| !driver.is_jogador && driver.status == DriverStatus::Ativo)
    };
    let position_of = |id: &str| contexts.get(id).map(|c| c.posicao_campeonato).unwrap_or(99);
    let skill_of = |id: &str| {
        drivers_by_id
            .get(id)
            .map(|d| d.atributos.skill)
            .unwrap_or(0.0)
    };

    for category in categories {
        let Some(required) = required_license_for_division(category.id, None) else {
            continue;
        };
        let active = contract_queries::get_all_active_regular_contracts(conn)
            .map_err(|e| format!("Falha ao carregar contratos para rebaixamento: {e}"))?;

        // Pior piloto da categoria: pior posição no campeonato, depois menor skill.
        let upper: Vec<&Contract> = active
            .iter()
            .filter(|c| {
                c.categoria == category.id
                    && c.classe.is_none()
                    && is_active_non_player(&c.piloto_id)
                    && !player_team_ids.contains(c.equipe_id.as_str())
            })
            .collect();
        if upper.len() < 2 {
            continue;
        }
        let Some(weakest) = upper.iter().max_by(|a, b| {
            position_of(&a.piloto_id)
                .cmp(&position_of(&b.piloto_id))
                .then_with(|| skill_of(&b.piloto_id).total_cmp(&skill_of(&a.piloto_id)))
        }) else {
            continue;
        };
        // Só rebaixa quem realmente foi mal: penúltimo ou último na sua categoria.
        let Some(weak_ctx) = contexts.get(&weakest.piloto_id) else {
            continue;
        };
        if weak_ctx.total_pilotos < 2 || weak_ctx.posicao_campeonato < weak_ctx.total_pilotos - 1 {
            continue;
        }

        // Melhor "subidor": campeão/vice de um feeder, já com a licença exigida.
        let feeders = get_feeder_categories(category.id);
        let Some(best_riser) = active
            .iter()
            .filter(|c| {
                feeders.iter().any(|feeder| *feeder == c.categoria)
                    && license_levels
                        .get(&c.piloto_id)
                        .is_some_and(|&owned| owned >= required)
                    && is_active_non_player(&c.piloto_id)
                    && !player_team_ids.contains(c.equipe_id.as_str())
            })
            .min_by(|a, b| {
                position_of(&a.piloto_id)
                    .cmp(&position_of(&b.piloto_id))
                    .then_with(|| skill_of(&b.piloto_id).total_cmp(&skill_of(&a.piloto_id)))
            })
        else {
            continue;
        };
        if position_of(&best_riser.piloto_id) > 2 {
            continue;
        }

        swap_contract_seats(conn, best_riser, weakest, new_season_number, report)?;
    }

    let refreshed: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao recarregar pilotos apos rebaixamento: {e}"))?
        .into_iter()
        .map(|driver| (driver.id.clone(), driver))
        .collect();
    sync_team_slots_from_active_regular_contracts(conn, teams, &refreshed)?;
    Ok(())
}

/// (Flag `IRACER_ROOKIE_MERIT`) Garante a subida do CAMPEÃO de cada categoria de
/// estreia (Rookie) para a categoria-alvo (Amador).
///
/// O fluxo normal já promove o melhor feeder quando o Amador tem vaga natural; esta
/// passada cobre o caso em que o Amador está CHEIO: força a troca do 1º do Rookie
/// com o pior do Amador, reusando a mesma máquina de `swap_contract_seats` do
/// rebaixamento por mérito (campeão sobe, pior desce ao Rookie — exatamente o que o
/// rebaixamento por mérito já faz, só que aqui o gatilho é "campeão" em vez de
/// "pior do Amador terminou em último"). Conservadora: no máximo 1 troca por
/// categoria de estreia, nunca mexe no jogador, e só dispara se o campeão de fato
/// possuir a licença exigida (a metade superior do Rookie a conquista).
pub(super) fn guarantee_rookie_champion_promotions(
    conn: &Connection,
    teams: &[crate::models::team::Team],
    new_season_number: i32,
    contexts: &HashMap<String, DriverMarketContext>,
    report: &mut MarketReport,
) -> Result<(), String> {
    if !rookie_merit_enabled() {
        return Ok(());
    }

    let debut_year = get_season_by_number(conn, new_season_number)?
        .map(|season| season.ano)
        .unwrap_or_else(|| Local::now().year());

    let player_team_ids: HashSet<&str> = teams
        .iter()
        .filter(|team| team.is_player_team)
        .map(|team| team.id.as_str())
        .collect();

    let rookie_cats: Vec<&'static str> = get_all_categories()
        .iter()
        .filter(|category| {
            is_real_career_debut_category(category.id)
                && is_category_active_in_year(category.id, debut_year)
        })
        .map(|category| category.id)
        .collect();

    let mut swapped_any = false;
    for rookie_cat in rookie_cats {
        // Alvo regular ativo (Amador) para onde o Rookie alimenta.
        let Some(target_cat) = get_target_categories(rookie_cat).into_iter().find(|target| {
            uses_regular_contracts(target)
                && !runs_in_special_phase(target)
                && is_category_active_in_year(target, debut_year)
        }) else {
            continue;
        };
        let Some(required) = required_license_for_division(target_cat, None) else {
            continue;
        };

        // Vaga natural no Amador → o fluxo normal (escada) já promove o melhor
        // feeder (o campeão). Nada a forçar.
        let target_has_vacancy = find_vacancies(conn)?
            .into_iter()
            .any(|vacancy| vacancy.categoria == target_cat && is_regular_vacancy(&vacancy));
        if target_has_vacancy {
            continue;
        }

        // Recarrega o estado a cada categoria (a troca anterior mexeu nos contratos).
        let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao carregar pilotos (promo campeao rookie): {e}"))?
            .into_iter()
            .map(|driver| (driver.id.clone(), driver))
            .collect();
        let license_levels = load_max_license_levels(conn)?;
        let is_active_non_player = |id: &str| {
            drivers_by_id
                .get(id)
                .is_some_and(|driver| !driver.is_jogador && driver.status == DriverStatus::Ativo)
        };
        let position_of = |id: &str| contexts.get(id).map(|c| c.posicao_campeonato).unwrap_or(99);
        let skill_of = |id: &str| {
            drivers_by_id
                .get(id)
                .map(|driver| driver.atributos.skill)
                .unwrap_or(0.0)
        };

        let active = contract_queries::get_all_active_regular_contracts(conn)
            .map_err(|e| format!("Falha ao carregar contratos (promo campeao rookie): {e}"))?;

        // Campeão do Rookie: 1º colocado, ativo, fora do time do jogador, COM a
        // licença exigida pelo Amador.
        let Some(champion) = active
            .iter()
            .filter(|c| {
                c.categoria == rookie_cat
                    && c.classe.is_none()
                    && is_active_non_player(&c.piloto_id)
                    && !player_team_ids.contains(c.equipe_id.as_str())
                    && license_levels
                        .get(&c.piloto_id)
                        .is_some_and(|&owned| owned >= required)
            })
            .min_by_key(|c| position_of(&c.piloto_id))
        else {
            continue;
        };
        if position_of(&champion.piloto_id) != 1 {
            continue; // só o 1º é garantido
        }

        // Pior piloto do Amador (pior posição, depois menor skill), fora do jogador.
        let Some(weakest) = active
            .iter()
            .filter(|c| {
                c.categoria == target_cat
                    && c.classe.is_none()
                    && is_active_non_player(&c.piloto_id)
                    && !player_team_ids.contains(c.equipe_id.as_str())
            })
            .max_by(|a, b| {
                position_of(&a.piloto_id)
                    .cmp(&position_of(&b.piloto_id))
                    .then_with(|| skill_of(&b.piloto_id).total_cmp(&skill_of(&a.piloto_id)))
            })
        else {
            continue;
        };

        swap_contract_seats(conn, champion, weakest, new_season_number, report)?;
        swapped_any = true;
    }

    if swapped_any {
        let refreshed: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao recarregar pilotos (pos promo campeao rookie): {e}"))?
            .into_iter()
            .map(|driver| (driver.id.clone(), driver))
            .collect();
        sync_team_slots_from_active_regular_contracts(conn, teams, &refreshed)?;
    }
    Ok(())
}

/// Categoria de ENTRADA da escada num ano: ativa e com nenhuma feeder ativa ainda
/// (a de menor tier existente). É onde nascem novos pilotos da época.
pub(super) fn is_entry_category_for_year(categoria: &str, year: i32) -> bool {
    is_category_active_in_year(categoria, year)
        && get_feeder_categories(categoria)
            .iter()
            .all(|feeder| !is_category_active_in_year(feeder, year))
}

/// Melhor candidato a PROMOÇÃO para uma vaga não-estreia (escada por MÉRITO).
///
/// Piloto ativo (não-jogador) atualmente numa categoria que alimenta a vaga
/// (`get_feeder_categories`) E que **já conquistou a licença exigida** pela divisão
/// (top-metade da categoria de baixo — mesma regra do jogador; nada de conceder
/// licença na hora). Entre os elegíveis, escolhe pela classificação no campeonato e,
/// em empate, pelo maior skill. Ver uso em `fill_remaining_vacancies_with_rookies`.
pub(super) fn best_feeder_promotion_candidate(
    vacancy: &Vacancy,
    drivers_by_id: &HashMap<String, Driver>,
    contexts: &HashMap<String, DriverMarketContext>,
    license_levels: &HashMap<String, u8>,
    required_license: Option<u8>,
) -> Option<Driver> {
    let feeders = get_feeder_categories(&vacancy.categoria);
    if feeders.is_empty() {
        return None;
    }

    drivers_by_id
        .values()
        .filter(|driver| {
            !driver.is_jogador
                && driver.status == DriverStatus::Ativo
                && driver
                    .categoria_atual
                    .as_deref()
                    .is_some_and(|categoria| feeders.iter().any(|feeder| *feeder == categoria))
                // Mérito: precisa POSSUIR de fato a licença exigida (linha real
                // >= nível), igual ao check de ensure_driver_can_join_division.
                // "Sem licença" não conta como nível 0.
                && match required_license {
                    Some(level) => license_levels
                        .get(&driver.id)
                        .is_some_and(|&owned| owned >= level),
                    None => true,
                }
        })
        .max_by(|a, b| {
            let score = |driver: &Driver| {
                let pos = contexts
                    .get(&driver.id)
                    .map(|context| context.posicao_campeonato)
                    .unwrap_or(99);
                feeder_promotion_score(driver.atributos.skill, pos)
            };
            score(a).total_cmp(&score(b))
        })
        .cloned()
}

/// Score de promoção em cascata: o TALENTO (skill) manda, com um empurrão
/// decrescente pelo desempenho na temporada. Antes a promoção ordenava SÓ por
/// `posicao_campeonato` (skill só desempatava), então o GT3 recebia os CAMPEÕES do
/// GT4 em vez dos mais HABILIDOSOS — um craque skill-80 em carro fraco (8º) perdia a
/// vaga para o campeão skill-60, e o topo deflacionava temporada após temporada. Com
/// o empurrão de campeonato preservamos o mérito da temporada (o campeão sobe na
/// frente de talentos até ~9 pts acima), sem enterrar o talento. 1º=+7.2, 5º=+4, 10º+=0.
pub(super) fn feeder_promotion_score(skill: f64, posicao_campeonato: i32) -> f64 {
    let championship_bonus = (10 - posicao_campeonato.clamp(1, 10)) as f64 * 0.8;
    skill + championship_bonus
}

/// Recrutamento profundo por DEMANDA DE TIME + ACEITE DO PILOTO. Liga os dois
/// cérebros prontos (`slam_ambition` e `driver_ai::evaluate_proposal`) na escada viva.
///
/// Para uma vaga de topo REGULAR (gt3) que o feeder imediato (só gt4) deixaria mal
/// servida, o time escaneia TODAS as categorias inferiores atrás do craque preso lá
/// (a elite não-ambiciosa que a cascata míope nunca alcança), prioriza quem AMBICIONA
/// a categoria (slam) e lhe faz proposta; o piloto decide via `evaluate_proposal`
/// (agência: Leal/Consolidador/oferta ruim recusam e ficam). Devolve o primeiro que
/// ACEITAR, ou `None` (cai na escada normal).
///
/// Gate anti-churn: só dispara para gt3 (tier ≥ 4 e NÃO fase especial — endurance e
/// production têm convocação própria e o feeder do endurance é só-gt3 de propósito) e
/// só quando o feeder não entrega o nível-alvo da categoria. A escada saudável do
/// miolo fica intacta.
#[allow(clippy::too_many_arguments)]
pub(super) fn deep_recruitment_candidate(
    conn: &Connection,
    vacancy: &Vacancy,
    drivers_by_id: &HashMap<String, Driver>,
    contexts: &HashMap<String, DriverMarketContext>,
    license_levels: &HashMap<String, u8>,
    required_license: Option<u8>,
    feeder_best_skill: Option<f64>,
    rng: &mut impl Rng,
) -> Result<Option<Driver>, String> {
    if vacancy.category_tier < 4 || runs_in_special_phase(&vacancy.categoria) {
        return Ok(None);
    }
    let target = skill_ranges::get_skill_range_by_tier(vacancy.category_tier.min(4))
        .map(|range| range.skill_media as f64)
        .unwrap_or(78.0);
    // Feeder já entrega o nível do topo → escada normal, sem escanear fundo.
    if feeder_best_skill.is_some_and(|skill| skill >= target) {
        return Ok(None);
    }

    let candidates: Vec<&Driver> = drivers_by_id
        .values()
        .filter(|driver| {
            !driver.is_jogador
                && driver.status == DriverStatus::Ativo
                // Só puxa craque de verdade (nível do topo) E só se for UPGRADE sobre
                // o melhor do feeder — nada de shuffle lateral.
                && driver.atributos.skill >= target
                && feeder_best_skill.map_or(true, |skill| driver.atributos.skill > skill)
                // Sentado numa categoria de tier ABAIXO da vaga (a "várzea").
                && driver.categoria_atual.as_deref().is_some_and(|cat| {
                    get_category_config(cat)
                        .is_some_and(|config| config.tier < vacancy.category_tier)
                })
                // Mérito de licença: idêntico ao da promoção regular (nada de conceder
                // na hora); barra naturalmente puxar rookie cru sem a licença do topo.
                && match required_license {
                    Some(level) => {
                        license_levels.get(&driver.id).is_some_and(|&owned| owned >= level)
                    }
                    None => true,
                }
        })
        .collect();
    if candidates.is_empty() {
        return Ok(None);
    }

    // Ranqueia UMA vez (slam consulta o archive por piloto — não repetir no comparador):
    // ambicioso que QUER esta categoria primeiro, depois pelo score de promoção.
    let mut ranked: Vec<(&Driver, bool, f64)> = candidates
        .into_iter()
        .map(|driver| {
            let wants_this = slam_target_category(conn, driver)
                .ok()
                .flatten()
                .is_some_and(|(category, _)| category == vacancy.categoria);
            let pos = contexts
                .get(&driver.id)
                .map(|context| context.posicao_campeonato)
                .unwrap_or(99);
            (driver, wants_this, feeder_promotion_score(driver.atributos.skill, pos))
        })
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.total_cmp(&a.2)));

    // O time faz proposta ao melhor; o piloto decide. Primeiro que aceita, ganha a
    // vaga; quem recusa (Leal/Consolidador/oferta ruim/quer ser N1) fica onde está.
    let contracts = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao carregar contratos p/ recrutamento profundo: {e}"))?;
    for (candidate, _, _) in ranked {
        let current_contract = contracts
            .iter()
            .find(|contract| contract.piloto_id == candidate.id);
        let current_tier = candidate
            .categoria_atual
            .as_deref()
            .and_then(get_category_config)
            .map(|config| config.tier)
            .unwrap_or(0);
        let proposal = MarketProposal {
            id: format!("deep-{}-{}", vacancy.team_id, candidate.id),
            equipe_id: vacancy.team_id.clone(),
            equipe_nome: vacancy.team_name.clone(),
            piloto_id: candidate.id.clone(),
            piloto_nome: candidate.nome.clone(),
            categoria: vacancy.categoria.clone(),
            papel: vacancy.papel_necessario.clone(),
            salario_oferecido: calculate_offer_salary(vacancy, candidate, rng),
            duracao_anos: 1,
            status: ProposalStatus::Pendente,
            motivo_recusa: None,
        };
        if evaluate_proposal(
            candidate,
            &proposal,
            current_contract,
            current_tier,
            vacancy.category_tier,
            vacancy.car_strength,
            vacancy.reputacao,
            rng,
        )
        .accepted
        {
            return Ok(Some(candidate.clone()));
        }
    }
    Ok(None)
}

pub(super) fn compare_pool_fallback_candidates(
    a: &AvailableDriver,
    b: &AvailableDriver,
    vacancy: &Vacancy,
    need_factor: f64,
    team_ceiling: Option<f64>,
) -> std::cmp::Ordering {
    // Gates duros (experiência/licença) mandam primeiro; o VALOR do time desempata:
    // mérito esportivo (skill) + apelo comercial da fama ponderado pela necessidade
    // do time, MENOS a penalidade de affordability (Item 1). Time carente pode preferir
    // um nome famoso a um rápido anônimo; numa dinastia a fama pesa pouco e a velocidade
    // decide (Fase 2a do estrelato). Time SEM CAIXA desce para um piloto mais barato.
    pool_fallback_candidate_rank(a, vacancy)
        .cmp(&pool_fallback_candidate_rank(b, vacancy))
        .then_with(|| {
            team_candidate_value(a, vacancy, need_factor, team_ceiling)
                .total_cmp(&team_candidate_value(b, vacancy, need_factor, team_ceiling))
        })
}

/// Valor de um candidato para o time: skill + apelo comercial da fama ponderado pela
/// necessidade do time (`fame_commercial_units × need_factor`), MENOS a penalidade de
/// affordability quando o time carrega um teto (`team_ceiling = Some`). `None` (flag off)
/// = comportamento antigo, sem penalidade.
pub(super) fn team_candidate_value(
    candidate: &AvailableDriver,
    vacancy: &Vacancy,
    need_factor: f64,
    team_ceiling: Option<f64>,
) -> f64 {
    let skill = candidate.driver.atributos.skill;
    let merit_and_appeal =
        skill + crate::fame::fame_commercial_units(candidate.driver.atributos.midia) * need_factor;
    match team_ceiling {
        // Item 1: penaliza o candidato que o assento NÃO PODE PAGAR. O preço de mercado do
        // piloto (por tier+papel do assento) acima do teto salarial da equipe vira uma
        // penalidade em "pontos de skill", empurrando um time sem caixa para um piloto mais
        // barato (ou mantendo o craque caro no pool para um assento que o comporte).
        Some(ceiling) => {
            let price = candidate_market_price(
                skill,
                vacancy.category_tier,
                matches!(vacancy.papel_necessario, TeamRole::Numero1),
            );
            merit_and_appeal - affordability_penalty(price, ceiling)
        }
        None => merit_and_appeal,
    }
}

/// Preço de mercado (independente do caixa do time) que um piloto de `skill` comanda num
/// assento deste `tier`/papel: a faixa salarial do tier posicionada pela skill, com fator
/// de papel (N1 titular custa mais). Mesma escala das ofertas ao jogador
/// (`player_offer_salary`) e dos contratos da IA (`salary_range_for_tier`) — não depende
/// da pobreza da equipe, senão o teto baixo de um time quebrado tornaria todo mundo
/// "barato" e a penalidade nunca dispararia.
pub(super) fn candidate_market_price(skill: f64, tier: u8, is_n1: bool) -> f64 {
    let (lo, hi) = crate::models::contract::salary_range_for_tier(tier);
    let t = (skill / 100.0).clamp(0.0, 1.0);
    let base = lo + (hi - lo) * t;
    let role = if is_n1 { 1.30 } else { 1.06 };
    base * role
}

/// Peso e teto da penalidade de affordability (em "pontos de skill", mesma unidade de
/// `team_candidate_value`, cujo base ≈ skill 0–100 + fama 0–63). A penalidade só ENTRA
/// como desempate DEPOIS dos gates duros (via `.then_with` em
/// `compare_pool_fallback_candidates`), então pode ser forte sem inverter licença/experiência
/// nem desestabilizar o sim (é comparador puro, sem re-scan). O `WEIGHT` alto garante que
/// "não posso pagar" supere uma diferença de skill relevante — um assento sobre orçamento
/// perde para um candidato que CABE, mesmo sendo este menos habilidoso. O `CAP` satura para
/// que, quando NINGUÉM cabe (time quebrado), todos fiquem igualmente penalizados e a skill
/// volte a decidir (ele assina o melhor disponível em vez de afundar num skill-20).
pub(super) const AFFORDABILITY_PENALTY_WEIGHT: f64 = 200.0;

pub(super) const AFFORDABILITY_PENALTY_CAP: f64 = 120.0;

/// Penalidade de affordability: 0 se o assento comporta o preço; senão cresce com o quanto
/// o preço excede o teto, saturando em `AFFORDABILITY_PENALTY_CAP`.
pub(super) fn affordability_penalty(price: f64, ceiling: f64) -> f64 {
    if ceiling <= 0.0 || price <= ceiling {
        return 0.0;
    }
    (AFFORDABILITY_PENALTY_WEIGHT * (price / ceiling - 1.0)).min(AFFORDABILITY_PENALTY_CAP)
}

/// Margem do piso de skill do pool de resgate: um órfão só preenche uma vaga
/// NÃO-estreia se o skill dele estiver, no máximo, esta distância ABAIXO da média
/// típica do tier da categoria. Sem isto, um lanterna (skill ~28) era resgatado
/// direto para GT3/Endurance só por estar sem categoria no momento. (Item B.)
///
/// Usamos SKILL (sinal confiável) e não o tier ancorado: um órfão sem histórico de
/// contrato ancora em tier 0 mesmo com skill alto, o que bloquearia resgates
/// legítimos (ex.: um skill-65 para a Production).
pub(super) const POOL_FALLBACK_SKILL_MARGIN: f64 = 20.0;

/// Piso de skill exigido do órfão para uma vaga do tier dado (média do tier − margem).
pub(super) fn pool_fallback_skill_floor(vacancy_tier: u8) -> f64 {
    let media = crate::constants::skill_ranges::get_skill_range_by_tier(vacancy_tier.min(4))
        .map(|range| range.skill_media as f64)
        .unwrap_or(60.0);
    (media - POOL_FALLBACK_SKILL_MARGIN).max(0.0)
}

pub(super) fn is_pool_fallback_candidate(candidate: &AvailableDriver, vacancy: &Vacancy) -> bool {
    if is_real_career_debut_category(&vacancy.categoria) {
        return is_rookie_market_candidate(
            &vacancy.categoria,
            candidate_category_for_rookie(candidate),
            candidate.driver.stats_carreira.corridas,
            candidate.driver.stats_carreira.temporadas,
        );
    }

    // Vaga NÃO-estreia: precisa ser órfão (sem categoria atual) E ter skill
    // compatível com o nível da categoria (piso = média do tier − margem). Item B.
    candidate.driver.categoria_atual.is_none()
        && candidate.driver.atributos.skill >= pool_fallback_skill_floor(vacancy.category_tier)
}

pub(super) fn pool_fallback_candidate_rank(candidate: &AvailableDriver, vacancy: &Vacancy) -> (u8, u8, u8) {
    let preferred_experience = if is_real_career_debut_category(&vacancy.categoria) {
        if candidate.driver.stats_carreira.corridas == 0
            && candidate.driver.stats_carreira.temporadas == 0
        {
            2
        } else if candidate_category_for_rookie(candidate) == vacancy.categoria {
            1
        } else {
            0
        }
    } else {
        u8::from(candidate.driver.stats_carreira.corridas > 0)
    };
    let required_license =
        required_license_for_division(&vacancy.categoria, vacancy.classe.as_deref()).unwrap_or(0);
    let has_required_license = candidate
        .max_license_level
        .map(|level| level >= required_license)
        .unwrap_or(required_license == 0);
    let license_level = candidate
        .max_license_level
        .unwrap_or(0)
        .min(required_license);

    (
        preferred_experience,
        u8::from(has_required_license),
        license_level,
    )
}

pub(super) fn candidate_category_for_rookie(candidate: &AvailableDriver) -> &str {
    if candidate.categoria_atual.trim().is_empty() {
        candidate
            .driver
            .categoria_atual
            .as_deref()
            .unwrap_or_default()
    } else {
        candidate.categoria_atual.as_str()
    }
}

pub(super) fn refresh_team_hierarchy(
    conn: &Connection,
    teams: &[crate::models::team::Team],
    drivers_by_id: &HashMap<String, Driver>,
) -> Result<(), String> {
    for team in teams {
        let refreshed = team_queries::get_team_by_id(conn, &team.id)
            .map_err(|e| format!("Falha ao recarregar equipe '{}': {e}", team.nome))?
            .ok_or_else(|| format!("Equipe '{}' nao encontrada", team.id))?;
        let mut pilots = Vec::new();
        if let Some(pilot_id) = &refreshed.piloto_1_id {
            if let Some(driver) = drivers_by_id.get(pilot_id) {
                pilots.push(driver);
            }
        }
        if let Some(pilot_id) = &refreshed.piloto_2_id {
            if let Some(driver) = drivers_by_id.get(pilot_id) {
                pilots.push(driver);
            }
        }
        pilots.sort_by(|a, b| b.atributos.skill.total_cmp(&a.atributos.skill));
        let n1 = pilots.first().map(|driver| driver.id.as_str());
        let n2 = pilots.get(1).map(|driver| driver.id.as_str());
        team_queries::update_team_pilots(conn, &team.id, n1, n2).map_err(|e| {
            format!(
                "Falha ao atualizar pilotos finais da equipe '{}': {e}",
                team.nome
            )
        })?;
        team_queries::update_team_hierarchy(
            conn,
            &team.id,
            n1,
            n2,
            TeamHierarchyClimate::Estavel.as_str(),
            0.0,
        )
        .map_err(|e| {
            format!(
                "Falha ao atualizar hierarquia da equipe '{}': {e}",
                team.nome
            )
        })?;
    }
    Ok(())
}
