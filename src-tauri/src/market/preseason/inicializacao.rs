//! Abertura da pré-temporada: monta o plano, sorteia os atributos sazonais das
//! equipes e aplica o aporte de última chance de quem vai para o all-in.

use super::*;

/// Abre a janela na FOTO: o grid como a temporada terminou, sem ninguém dispensado
/// ainda. As pré-passes (o que esvazia assentos) caem ao sair da semana 1, em
/// `advance_week` — ver `MARKET_SIGNINGS_START_WEEK`.
///
/// `_rng` sobrou da época em que as pré-passes rodavam aqui. Fica na assinatura porque
/// a abertura é justamente onde um sorteio novo entraria, e trocá-la mexeria em ~18
/// callsites por nada.
pub fn initialize_preseason(
    conn: &Connection,
    season_number: i32,
    _rng: &mut impl Rng,
) -> Result<PreSeasonPlan, String> {
    let season_id = get_season_id_by_number(conn, season_number)?
        .ok_or_else(|| format!("Temporada {season_number} nao encontrada"))?;
    reset_market_state(conn, &season_id, &PreSeasonPhase::Transfers)?;
    repair_missing_licenses_for_current_categories(conn)?;
    assign_seasonal_team_attributes(conn, season_number, &season_id)?;

    // Contratos ativos na FOTO (semana 1) — as pré-passes só caem ao sair dela.
    let contracts_before = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao carregar contratos da foto do grid: {e}"))?;

    // Snapshot das categorias ANTES das pré-passes (que limpam o categoria_atual dos
    // dispensados) — origem p/ inferir promovido/rebaixado nas assinaturas da escada.
    let category_snapshot: std::collections::HashMap<String, String> =
        driver_queries::get_all_drivers(conn)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|d| d.categoria_atual.map(|c| (d.id, c)))
            .collect();

    // Snapshot da EQUIPE anterior (+ tempo de casa) ANTES das pré-passes — origem p/ o
    // popup de detalhe da transferência (de qual equipe veio e há quantas temporadas).
    let previous_team: std::collections::HashMap<String, (String, i32)> = contracts_before
        .iter()
        .map(|c| {
            let tenure = crate::commands::career::calculate_consecutive_team_tenure(
                conn,
                &c.piloto_id,
                &c.equipe_id,
                season_number - 1,
            )
            .unwrap_or(1);
            (c.piloto_id.clone(), (c.equipe_nome.clone(), tenure))
        })
        .collect();

    // ── As PRÉ-PASSES não rodam aqui. ──────────────────────────────────────────────
    // Elas expiram contratos, renovam (slam-aware), rebaixam por mérito e resolvem o
    // assédio — ou seja, esvaziam assentos. Rodando na abertura, a semana 1 já nasceria
    // com o grid furado e o jogador nunca veria a foto de como a temporada terminou. Por
    // isso caem ao SAIR da semana 1 (`advance_week`), com o feed narrando a mudança.
    //
    // O mercado ao vivo é a escada paginada conduzida por advance_week: da
    // `signings_start_week` em diante cada semana preenche vagas em todos os tiers e
    // oferta ao jogador. A pré-temporada fecha quando não há mais o que preencher.

    // Na foto o jogador tem time se tem contrato regular ativo — inclusive um que vence
    // nesta virada (ainda não expirou). Ele só descobre que ficou de fora na semana 2,
    // junto com o resto do grid, que é exatamente a intenção.
    let player_has_team = driver_queries::get_player_driver(conn)
        .ok()
        .and_then(|player| {
            contract_queries::get_active_regular_contract_for_pilot(conn, &player.id).ok()
        })
        .flatten()
        .is_some();

    // Semanas VARIÁVEIS: a conclusão é dirigida pela janela (flag closed), não por um
    // total fixo. `total_weeks` fica só como teto p/ a data exibida não estourar.
    let total_weeks = i32::from(MARKET_DURATION_WEEKS);
    let mut state = PreSeasonState {
        season_number,
        current_week: 1,
        total_weeks,
        phase: PreSeasonPhase::Transfers,
        is_complete: false,
        player_has_pending_proposals: false,
        player_has_team,
        current_display_date: None,
        signings_start_week: i32::from(MARKET_SIGNINGS_START_WEEK),
        player_interest_forecast: None,
    };
    refresh_preseason_state_display_date(conn, &season_id, &mut state)?;

    let mut plan = PreSeasonPlan {
        state,
        planned_events: Vec::new(),
        executed_weeks: Vec::new(),
        pending_departures: Vec::new(),
        pending_renewals: Vec::new(),
        prepasses_applied: false,
        movements_applied: false,
        category_snapshot,
        previous_team,
        // A quebra de contrato do jogador (Fase 2b.3) depende de quem sobrou com
        // assento, então só é computada junto das pré-passes, ao sair da semana 1.
        player_poach_offer: None,
    };
    // Expectativa da semana 1: as vagas ainda não existem, então a faixa sai dos
    // contratos que VENCEM. Ver `refresh_player_interest_forecast`.
    refresh_player_interest_forecast(conn, &mut plan, season_number, &contracts_before)?;
    Ok(plan)
}

/// Tamanho do aporte de última chance, como fração do caixa-médio da categoria.
/// Calibrado para que ~20% das equipes em all-in escapem do colapso (o resto
/// ainda é vendido). Maior = mais escapam.
pub(super) const LAST_CHANCE_PACKAGE_FACTOR: f64 = 0.25;

/// Aplica o aporte de última chance a uma equipe entrando no ano de all-in:
/// abate a maior parte da dívida e reforça o caixa. Não recalcula o estado
/// financeiro (o chamador o faz).
pub(super) fn apply_last_chance_package(team: &mut crate::models::team::Team) {
    let scale = category_finance_scale_for(&team.categoria, team.classe.as_deref());
    let package = scale.expected_cash_midpoint() * LAST_CHANCE_PACKAGE_FACTOR;
    // 70% do pacote abate dívida, 30% vira capital de giro.
    team.debt_balance = (team.debt_balance - package * 0.70).max(0.0);
    team.cash_balance += package * 0.30;
}

/// **O offseason da equipe: ela investe o excedente de caixa e recebe estrutura.**
///
/// Substitui `finance::cashflow::apply_offseason_competitiveness_impact`, que ocupava esta
/// linha e **creditava engenharia, instalações e confiabilidade sem debitar nada** — medido
/// pelo harness de economia em ~2,76 pontos de estrutura por equipe por temporada, de graça.
/// Era uma das fontes de dinheiro do nada do redesign e a razão declarada de o superávit das
/// equipes não ter para onde ir: nenhum débito escalava com a riqueza, então o caixa
/// integrava para sempre.
///
/// Quem decide agora é [`crate::economia::desenvolvimento`], cujos parâmetros o harness
/// varreu (`varrer_ralo`, `varrer_a_reserva_do_ralo`): a equipe guarda uma reserva de
/// operação, investe uma fração do que sobra acima dela, e o que isso compra satura. Quem
/// não tem excedente não investe — e ainda deprecia, porque ficar parado tem preço.
///
/// O que se perdeu na troca, de propósito: a penalidade de marca fictícia e o "breakthrough
/// técnico" só alimentavam `car_performance_delta`, e esse delta **já não escrevia em lugar
/// nenhum** — quem constrói carro é o Sistema de Nível do Carro, pela tabela `team_car`. O
/// retorno da função antiga era descartado aqui e não tinha outro consumidor no crate.
fn aplicar_desenvolvimento_de_offseason(
    team: &mut crate::models::team::Team,
    foco: crate::finance::focus::TeamFocus,
) -> crate::economia::desenvolvimento::PlanoDeDesenvolvimento {
    use crate::economia::desenvolvimento::{
        planejar_desenvolvimento, EntradaDeDesenvolvimento, ParametrosDeDesenvolvimento,
    };

    let entrada = EntradaDeDesenvolvimento {
        caixa: team.cash_balance,
        divida: team.debt_balance,
        custo_operacional_anual: crate::economia::temporada::custo_operacional_anual_de_referencia(
            &team.categoria,
            team.classe.as_deref(),
        ),
        engenharia: team.engineering,
        instalacoes: team.facilities,
        confiabilidade: team.confiabilidade,
        apetite: foco.apetite_de_investimento(),
    };
    let plano = planejar_desenvolvimento(&entrada, &ParametrosDeDesenvolvimento::default());

    // Os deltas já saem limitados à faixa 0–100 pelo módulo; o clamp aqui é rede contra
    // uma coluna que chegou fora da faixa por outro caminho.
    team.engineering = (team.engineering + plano.delta_engenharia).clamp(0.0, 100.0);
    team.facilities = (team.facilities + plano.delta_instalacoes).clamp(0.0, 100.0);
    team.confiabilidade = (team.confiabilidade + plano.delta_confiabilidade).clamp(0.0, 100.0);
    team.cash_balance -= plano.investimento;

    plano
}

pub(super) fn assign_seasonal_team_attributes(
    conn: &Connection,
    season_number: i32,
    season_id: &str,
) -> Result<(), String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao carregar equipes: {e}"))?;
    let previous_standings = load_previous_team_standings(conn, season_number)?;
    // Pilar D: as 3 elites por classe premium (Production auto por reputação;
    // GT3/Endurance com as marcas reais fixas). Recebem plano de dinastia + piso de
    // recursos abaixo, sustentando carro no topo temporada após temporada.
    let elite_ids = designate_elite_teams(&teams);

    // O "ano de carreira" era lido aqui só para esmaecer a penalidade de marca fictícia da
    // GT3 dentro de `apply_offseason_competitiveness_impact`. Essa penalidade modulava
    // exclusivamente o `car_performance_delta`, que já não escrevia em coluna nenhuma
    // (quem constrói carro é o Sistema de Nível do Carro), e a chamada saiu daqui junto com
    // a regalia de estrutura — ver `aplicar_desenvolvimento_de_offseason`. A meta
    // `career_start_year` continua sendo escrita pelas carreiras novas e lida por quem
    // precisa dela; o que sumiu foi esta leitura, que não alimentava mais nada.

    let mut categories = teams
        .iter()
        .map(|team| team.categoria.clone())
        .collect::<Vec<_>>();
    categories.sort();
    categories.dedup();

    for category in categories {
        let category_teams = teams
            .iter()
            .filter(|team| team.categoria == category)
            .cloned()
            .collect::<Vec<_>>();
        if category_teams.is_empty() {
            continue;
        }

        let calendar = calendar_queries::get_calendar(conn, season_id, &category)
            .map_err(|e| format!("Falha ao carregar calendario de {category}: {e}"))?;
        if calendar.is_empty() {
            continue;
        }

        for team in &category_teams {
            let mut updated_team = team.clone();
            // O shape/nível do carro agora vive no Sistema de Nível do Carro (tabela
            // team_car), decidido pelo cérebro de manutenção por corrida — não mais por
            // um perfil discreto escolhido aqui. O custo do carro vira depreciação real
            // na fatura (chunk 6).
            updated_team.pit_strategy_risk = recalculate_pit_strategy_risk(team, &category_teams);
            updated_team.budget = derive_budget_index_from_money(&updated_team);
            refresh_team_financial_state(&mut updated_team);
            // Pilar D: elite recebe o piso de recursos (patrocínio de dinastia) antes
            // de recalcular o estado financeiro — assim nunca cai em colapso e sustenta
            // o investimento máximo no carro.
            let is_elite = elite_ids.contains(&updated_team.id);
            if is_elite {
                apply_elite_resource_floor(&mut updated_team);
                updated_team.budget = derive_budget_index_from_money(&updated_team);
                refresh_team_financial_state(&mut updated_team);
            }
            // Equipe entrando na 2ª temporada consecutiva de colapso vai de all-in
            // numa tentativa de se salvar antes de ser vendida. Recebe um aporte de
            // "última chance" (investidores injetam capital): abate parte da dívida
            // e reforça o caixa, dando uma chance real — mas só quem também render
            // na pista escapa do colapso; o resto ainda termina vendido.
            let collapse_streak =
                team_queries::get_collapse_streak(conn, &updated_team.id).unwrap_or(0);
            updated_team.season_strategy = if collapse_streak == 1 && !is_elite {
                apply_last_chance_package(&mut updated_team);
                refresh_team_financial_state(&mut updated_team);
                "all_in".to_string()
            } else {
                // Pilar C: a estratégia da temporada vem do plano de 3 temporadas
                // da equipe (arco sustentado), não de uma escolha reativa anual.
                // Pilar D: elites rodam Elite Dominance permanente (dinastia).
                advance_strategic_plan(conn, &updated_team, is_elite)
                    .map_err(|e| {
                        format!(
                            "Falha ao avançar plano estratégico da equipe {}: {e}",
                            updated_team.nome
                        )
                    })?
                    .to_string()
            };
            // Foco vigente (da temporada que passou) decide quanto o time canaliza
            // para a estrutura neste offseason — a consequência real do foco (ideia 4).
            // Lido ANTES do update_team_focus abaixo (que só recalcula a fase nova).
            let current_focus = crate::finance::focus::get_focus(conn, &updated_team.id)
                .map(|(foco, _)| foco)
                .unwrap_or(crate::finance::focus::TeamFocus::MeioDeGrid);
            aplicar_desenvolvimento_de_offseason(&mut updated_team, current_focus);
            updated_team.pit_crew_quality = recalculate_pit_crew_quality(
                &updated_team,
                previous_standings.get(&team.id).copied(),
            );
            refresh_team_financial_state(&mut updated_team);
            updated_team.budget = derive_budget_index_from_money(&updated_team);
            team_queries::update_team(conn, &updated_team).map_err(|e| {
                format!(
                    "Falha ao salvar perfil sazonal do carro para equipe {}: {e}",
                    updated_team.nome
                )
            })?;

            // Foco da equipe (ideia 4): deriva a fase atual do estado já calculado,
            // com histerese; promoção/rebaixamento (categoria_anterior) é evento duro
            // que fura a histerese e troca na hora. O retorno (virada) alimentará a
            // notícia da mudança de foco.
            let (plan_type, _) = team_queries::get_strategic_plan(conn, &updated_team.id)
                .unwrap_or_else(|_| ("sustainable".to_string(), 0));
            let hard_event = updated_team.categoria_anterior.is_some();
            crate::finance::focus::update_team_focus(
                conn,
                &updated_team,
                is_elite,
                &plan_type,
                hard_event,
            )
            .map_err(|e| {
                format!(
                    "Falha ao atualizar foco da equipe {}: {e}",
                    updated_team.nome
                )
            })?;
        }
    }

    Ok(())
}

pub(super) fn load_previous_team_standings(
    conn: &Connection,
    season_number: i32,
) -> Result<std::collections::HashMap<String, PreviousTeamStanding>, String> {
    if season_number <= 1 {
        return Ok(std::collections::HashMap::new());
    }

    let Some(previous_season_id) = get_season_id_by_number(conn, season_number - 1)? else {
        return Ok(std::collections::HashMap::new());
    };

    let mut stmt = conn
        .prepare(
            "SELECT equipe_id, categoria, SUM(pontos) AS total_pontos
             FROM standings
             WHERE temporada_id = ?1 AND equipe_id IS NOT NULL AND TRIM(equipe_id) <> ''
             GROUP BY equipe_id, categoria
             ORDER BY categoria ASC, total_pontos DESC, equipe_id ASC",
        )
        .map_err(|e| format!("Falha ao preparar standings anteriores por equipe: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![previous_season_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar standings anteriores por equipe: {e}"))?;

    let mut grouped = std::collections::HashMap::<String, Vec<(String, f64)>>::new();
    for row in rows {
        let (team_id, category, total_points) =
            row.map_err(|e| format!("Falha ao ler standings anteriores por equipe: {e}"))?;
        grouped
            .entry(category)
            .or_default()
            .push((team_id, total_points));
    }

    let mut result = std::collections::HashMap::new();
    for teams_in_category in grouped.into_values() {
        let total_teams = teams_in_category.len();
        for (index, (team_id, _)) in teams_in_category.into_iter().enumerate() {
            result.insert(
                team_id,
                PreviousTeamStanding {
                    position: index as i32 + 1,
                    total_teams,
                },
            );
        }
    }

    Ok(result)
}
