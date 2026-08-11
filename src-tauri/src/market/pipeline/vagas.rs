//! Levantamento das vagas abertas na grade e do pool de candidatos.
//!
//! Etapa de AVALIAÇÃO da entressafra: quem tem assento sobrando (`find_vacancies`),
//! quem está disponível para ocupá-lo (`find_available_drivers`) e o quão desejável
//! é cada assento (`seat_desirability`) — a ordem em que os assentos escolhem.

use super::*;

pub(super) fn find_vacancies(conn: &Connection) -> Result<Vec<Vacancy>, String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao buscar equipes: {e}"))?;
    let mut vacancies = Vec::new();

    // ANO da temporada corrente, para não abrir vaga numa DIVISÃO que ainda não existe.
    //
    // Sem isto, o levantamento varria TODAS as equipes do banco e ignorava a linha do tempo:
    // em 2005, as equipes de `mazda_rookie` — categoria que só nasce em 2020 — já geravam vaga.
    // A vaga era preenchida com um rookie recém-gerado que nunca corria (a categoria não é
    // simulada naquele ano) e que a escada depois promovia para cima, chegando ao gt3 com zero
    // corridas na conta. É a cadeia que `historical_draft` já descrevia em prosa e compensava
    // depois, com uma purga que só alcançava quem tinha ficado sem contrato.
    //
    // O corte é pela DIVISÃO (categoria + classe), NÃO pelo ano de fundação da equipe. A
    // geração histórica escala N1/N2 em todas as equipes, inclusive nas que só entram no
    // campeonato anos depois (Obsidian no gt3 nasce em 2004; a categoria roda desde 1999).
    // Cortando por fundação, essas equipes viravam um ralo de mão única: perdiam piloto por
    // fim de contrato — o mercado leva o piloto para uma equipe que corre — e nunca repunham,
    // porque nenhuma vaga era aberta para elas. Ficavam anos com MEIO elenco, que é o estado
    // que a auditoria do mundo histórico reprova (`active_team_without_two_drivers`) e que
    // travava a finalização do draft. Quem vai ao grid continua sendo só a equipe já fundada
    // (`is_team_active_in_year`, em `commands::race::simulacao`); manter o elenco é outra
    // pergunta.
    //
    // No jogo moderno o filtro é inerte (todas as categorias estão ativas), e se não houver
    // temporada ativa o comportamento é o de antes — não filtrar — em vez de esvaziar a grade.
    let ano_corrente = crate::db::queries::seasons::get_active_season(conn)
        .ok()
        .flatten()
        .map(|season| season.ano);

    for team in teams {
        if !uses_regular_contracts(&team.categoria) {
            continue;
        }
        if ano_corrente.is_some_and(|ano| {
            !crate::constants::historical_timeline::is_team_division_active_in_year(&team, ano)
        }) {
            continue;
        }
        let category_tier = get_category_config(&team.categoria)
            .map(|config| config.tier)
            .unwrap_or(0);
        match (&team.piloto_1_id, &team.piloto_2_id) {
            (None, None) => {
                vacancies.push(Vacancy {
                    team_id: team.id.clone(),
                    team_name: team.nome.clone(),
                    categoria: team.categoria.clone(),
                    classe: team.classe.clone(),
                    category_tier,
                    car_strength: team.car_strength(),
                    budget: team.budget,
                    cash_balance: team.cash_balance,
                    debt_balance: team.debt_balance,
                    financial_state: team.financial_state.clone(),
                    reputacao: team.reputacao,
                    papel_necessario: TeamRole::Numero1,
                    piloto_existente_id: None,
                });
                vacancies.push(Vacancy {
                    team_id: team.id.clone(),
                    team_name: team.nome.clone(),
                    categoria: team.categoria.clone(),
                    classe: team.classe.clone(),
                    category_tier,
                    car_strength: team.car_strength(),
                    budget: team.budget,
                    cash_balance: team.cash_balance,
                    debt_balance: team.debt_balance,
                    financial_state: team.financial_state.clone(),
                    reputacao: team.reputacao,
                    papel_necessario: TeamRole::Numero2,
                    piloto_existente_id: None,
                });
            }
            (Some(existing), None) => vacancies.push(Vacancy {
                team_id: team.id.clone(),
                team_name: team.nome.clone(),
                categoria: team.categoria.clone(),
                classe: team.classe.clone(),
                category_tier,
                car_strength: team.car_strength(),
                budget: team.budget,
                cash_balance: team.cash_balance,
                debt_balance: team.debt_balance,
                financial_state: team.financial_state.clone(),
                reputacao: team.reputacao,
                papel_necessario: TeamRole::Numero2,
                piloto_existente_id: Some(existing.clone()),
            }),
            (None, Some(existing)) => vacancies.push(Vacancy {
                team_id: team.id.clone(),
                team_name: team.nome.clone(),
                categoria: team.categoria.clone(),
                classe: team.classe.clone(),
                category_tier,
                car_strength: team.car_strength(),
                budget: team.budget,
                cash_balance: team.cash_balance,
                debt_balance: team.debt_balance,
                financial_state: team.financial_state.clone(),
                reputacao: team.reputacao,
                papel_necessario: TeamRole::Numero1,
                piloto_existente_id: Some(existing.clone()),
            }),
            (Some(_), Some(_)) => {}
        }
    }

    Ok(vacancies)
}

pub(super) fn load_max_license_levels(conn: &Connection) -> Result<HashMap<String, u8>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT piloto_id, MAX(CAST(nivel AS INTEGER))
             FROM licenses
             GROUP BY piloto_id",
        )
        .map_err(|e| format!("Falha ao preparar consulta de licencas: {e}"))?;
    let mut rows = stmt
        .query([])
        .map_err(|e| format!("Falha ao ler licencas: {e}"))?;
    let mut map = HashMap::new();
    while let Some(row) = rows
        .next()
        .map_err(|e| format!("Falha ao iterar licencas: {e}"))?
    {
        let piloto_id: String = row.get(0).unwrap_or_default();
        let nivel: u8 = row.get::<_, i64>(1).unwrap_or(0) as u8;
        map.insert(piloto_id, nivel);
    }
    Ok(map)
}

pub(super) fn find_available_drivers(
    conn: &Connection,
    standings_by_driver: &HashMap<String, DriverMarketContext>,
) -> Result<Vec<AvailableDriver>, String> {
    let active_contracts = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao recarregar contratos ativos: {e}"))?;
    let contracted_ids: HashSet<String> = active_contracts
        .into_iter()
        .map(|contract| contract.piloto_id)
        .collect();

    let drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos disponiveis: {e}"))?;
    let license_levels = load_max_license_levels(conn)?;
    // Última categoria contratada por piloto — para resgatar o nível de veteranos parados
    // (categoria_atual zerada por sync.rs) em vez de rebaixá-los a tier 0 no leilão.
    let last_categories = load_last_regular_categories(conn)?;
    let mut available = Vec::new();

    for driver in drivers {
        if driver.is_jogador
            || driver.status != DriverStatus::Ativo
            || contracted_ids.contains(&driver.id)
        {
            continue;
        }
        let mut context = standings_by_driver
            .get(&driver.id)
            .cloned()
            .unwrap_or_else(|| default_market_context(&driver));
        // Piloto parado (sem categoria atual nem standing da última temporada): ancora no
        // nível da última categoria que correu, espelhando `player_market_tier`. Sem isso,
        // um ex-GT3 vira candidato tier 0 (só recebe proposta de rookie).
        if context.categoria.is_empty() {
            if let Some(last_cat) = last_categories.get(&driver.id) {
                if let Some(config) = get_category_config(last_cat) {
                    context.categoria = last_cat.clone();
                    context.category_tier = config.tier;
                }
            }
        }
        let visibility = calculate_visibility(
            &driver,
            context.posicao_campeonato,
            context.total_pilotos,
            context.category_tier,
            context.vitorias,
            context.titulos,
            context.poles,
            &context.papel,
            &context.categoria,
        );
        let max_license_level = license_levels.get(&driver.id).copied();
        available.push(AvailableDriver {
            driver,
            visibility,
            posicao_campeonato: context.posicao_campeonato,
            categoria_atual: context.categoria,
            category_tier: context.category_tier,
            max_license_level,
        });
    }

    Ok(available)
}

pub(super) fn is_regular_vacancy(vacancy: &Vacancy) -> bool {
    get_category_config(&vacancy.categoria)
        .map(|category| uses_regular_contracts(category.id))
        .unwrap_or(true)
}

#[allow(dead_code)] // superada pela Janela de Transferências (apply_weekly_market)
pub(super) fn is_rookie_signing_candidate(
    candidate: &AvailableDriver,
    expiring_by_driver: &HashMap<String, Contract>,
    target_category: &str,
) -> bool {
    if !is_real_career_debut_category(target_category) {
        return false;
    }
    if expiring_by_driver.contains_key(&candidate.driver.id) {
        return false;
    }
    if !candidate.categoria_atual.is_empty() {
        return false;
    }
    if candidate.posicao_campeonato < 99 {
        return false;
    }
    true
}

/// Desejabilidade de um assento para a ORDEM de escolha (port de
/// `transfer_window::driver_offer_score`): carro + prestígio (reputação da equipe, já na
/// vaga). Os pesos vêm da FONTE ÚNICA `transfer_window::{SEAT_W_CAR, SEAT_W_PRESTIGE}` — os
/// mesmos do motor de janela —, então não divergem. Assim o melhor carro numa equipe
/// prestigiada escolhe do pool antes de um carro igual sem tradição — o que o leilão dava de
/// graça, reproduzido na escada gulosa.
pub(super) fn seat_desirability(vacancy: &Vacancy) -> f64 {
    use crate::market::transfer_window::{SEAT_W_CAR, SEAT_W_PRESTIGE};
    let car_norm = vacancy.car_strength;
    (car_norm / 100.0).min(1.2) * SEAT_W_CAR
        + (vacancy.reputacao.clamp(0.0, 100.0) / 100.0) * SEAT_W_PRESTIGE
}
