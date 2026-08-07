//! Dinheiro e fama do fim de semana: contexto financeiro da rodada, impacto do resultado no interesse do evento e repasse de fama para patrocinio e bilheteria.

use super::*;

// ── Prêmio por resultado, em fração do custo operacional de UMA rodada ────────────────
//
// Estes cinco coeficientes eram valores ABSOLUTOS (650/ponto, 4.000/vitória, 1.250/pódio,
// 1.000 por top-5, 120/ponto de prêmio parcial) — os únicos desta função, onde todo o resto
// já é fração de `round_operating_base` ou do caixa-médio da categoria. Como a base da rodada
// vai de ~R$15 mil (rookie) a ~R$1,4 milhão (endurance), um número fixo significa coisas
// opostas nas pontas da escada: a vitória pagava 26% da operação de uma rodada no rookie e
// 0,3% no endurance.
//
// A consequência era um déficit que CRESCIA a cada degrau, medido em
// `commands::race::tests::medicao_financeira`: a despesa é plana em ~1,85× o custo operacional
// declarado da categoria, mas a receita caía de 2,19× (rookie) para 1,53× (lmp2) — e os ~19
// pontos percentuais que sumiam do caminho eram exatamente estes prêmios evaporando contra uma
// escala 89× maior. Daí o mundo com 20% das temporadas em colapso e a equipe mediana do topo
// vivendo logo depois de ter sido vendida.
//
// Os valores mantêm as PROPORÇÕES entre si (uma vitória vale ~6 pontos de bônus, um pódio ~2)
// e reproduzem, no rookie, o que os absolutos pagavam lá — que era a única ponta da escada
// onde a economia fechava.
const BONUS_POR_PONTO: f64 = 0.042;
const BONUS_POR_VITORIA: f64 = 0.26;
const BONUS_POR_PODIO: f64 = 0.081;
const BONUS_TOP5: f64 = 0.065;
const PREMIO_PARCIAL_POR_PONTO: f64 = 0.0078;

/// Botão único da magnitude do prêmio por resultado. Existe separado dos coeficientes para
/// que recalibrar o NÍVEL não mexa na FORMA (a proporção entre ponto, vitória e pódio, que é
/// decisão de design). Calibrado pelo harness de economia: alvo de receita/despesa ~1,0 em
/// toda a escada, com o campeão lucrando e o lanterna sangrando.
const ESCALA_DO_BONUS: f64 = 1.0;

/// Fração do caixa-médio da categoria que o patrocínio-base paga por rodada. É o termo
/// dominante da receita (~70%), então é o botão do NÍVEL da economia — enquanto os
/// coeficientes de bônus acima definem a INCLINAÇÃO por desempenho.
///
/// Histórico: nasceu 0,16, virou 0,32 quando a receita-base era metade da despesa e o grid
/// inteiro falia. Com o prêmio por resultado agora proporcional à categoria, 0,32 passou a
/// deixar o mundo folgado demais (`elite` virou o estado modal em quase toda categoria, o que
/// esvazia a pressão orçamentária tanto quanto a falência em massa esvaziava). Calibrado em
/// 0,27 pelo harness de economia: receita/despesa ~1,0 na escada inteira.
const PATROCINIO_BASE: f64 = 0.27;

/// Coeficiente do termo de PATROCÍNIO por fama do lineup (presença pública 0–100).
/// Casado com o coeficiente da reputação (0.004) — a fama é uma 2ª moeda de receita
/// no mesmo patamar: contratar um rosto famoso capta patrocínio de verdade. Tunável.
pub(super) const FAME_SPONSORSHIP_COEFF: f64 = 0.004;

/// Todos os botões da RECEITA de uma rodada num objeto só.
///
/// Existe para o harness de calibração (`commands::race::tests::medicao_financeira`) varrer
/// valores dentro de um processo só, comparando dez configurações lado a lado — o que uma
/// env var não permite, porque `gate_pot_coeff()` é `LazyLock` e lê uma vez. A PRODUÇÃO usa
/// `Default`, e cada campo do `Default` É exatamente a constante declarada acima dele; não
/// existe segundo lugar onde estes números morem. Mesmo padrão de
/// [`crate::car::crash::apply_contact_wear_with`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CoeficientesDeReceita {
    /// Fração do caixa-médio da categoria que o patrocínio paga por rodada. O NÍVEL.
    pub patrocinio_base: f64,
    /// Peso da fama do lineup no patrocínio.
    pub fama_patrocinio: f64,
    /// Bônus por ponto, vitória, pódio e top-5 — a INCLINAÇÃO por desempenho.
    pub bonus_por_ponto: f64,
    pub bonus_por_vitoria: f64,
    pub bonus_por_podio: f64,
    pub bonus_top5: f64,
    /// Multiplicador único do prêmio por resultado (mexe no nível sem mexer na forma).
    pub escala_do_bonus: f64,
    /// Prêmio parcial por ponto, o canal por-corrida separado do bônus.
    pub premio_parcial_por_ponto: f64,
    /// Bolo da bilheteria, em frações do custo operacional de UMA rodada de UMA equipe.
    pub portao_coef: f64,
    /// Quanto do portão é piso dividido igual (0) vs. prêmio por fama (1).
    pub portao_piso: f64,
    /// Prêmio de construtores: fração paga ao último e inclinação até o 1º.
    pub premio_base: f64,
    pub premio_inclinacao: f64,
}

impl Default for CoeficientesDeReceita {
    fn default() -> Self {
        Self {
            patrocinio_base: PATROCINIO_BASE,
            fama_patrocinio: FAME_SPONSORSHIP_COEFF,
            bonus_por_ponto: BONUS_POR_PONTO,
            bonus_por_vitoria: BONUS_POR_VITORIA,
            bonus_por_podio: BONUS_POR_PODIO,
            bonus_top5: BONUS_TOP5,
            escala_do_bonus: ESCALA_DO_BONUS,
            premio_parcial_por_ponto: PREMIO_PARCIAL_POR_PONTO,
            portao_coef: crate::finance::cashflow::gate_pot_coeff(),
            portao_piso: crate::finance::cashflow::GATE_FLOOR_WEIGHT,
            premio_base: crate::finance::prize::PRIZE_BASE_FACTOR,
            premio_inclinacao: crate::finance::prize::PRIZE_SLOPE_FACTOR,
        }
    }
}

/// O que uma etapa cobra de um time em campo, além da escala da categoria: onde ela
/// aconteceu e o quanto os carros dele efetivamente rodaram. Alimenta a fatura da etapa
/// (`super::despesa`) — a mesma para IA e para o jogador.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RoundOperationContext {
    pub track_id: u32,
    /// Fração da corrida completada pelos carros do time (0..1). Abandono cedo queima
    /// menos combustível e menos pneu.
    pub laps_ratio: f64,
    /// Desgaste final médio dos pneus dos carros do time (0..1).
    pub tire_wear: f64,
}

/// Média de voltas e desgaste dos carros de um time numa corrida, normalizada pela
/// distância que o vencedor completou. Sem carro do time no resultado → rodada cheia
/// com desgaste médio (o time pagou a etapa de qualquer jeito).
pub(crate) fn round_operation_context(
    result: &RaceResult,
    team_id: &str,
    track_id: u32,
) -> RoundOperationContext {
    let race_laps = result
        .race_results
        .iter()
        .map(|r| r.laps_completed)
        .max()
        .unwrap_or(0)
        .max(1) as f64;
    let carros: Vec<_> = result
        .race_results
        .iter()
        .filter(|r| r.team_id == team_id)
        .collect();
    if carros.is_empty() {
        return RoundOperationContext {
            track_id,
            laps_ratio: 1.0,
            tire_wear: 0.6,
        };
    }
    let n = carros.len() as f64;
    RoundOperationContext {
        track_id,
        laps_ratio: (carros.iter().map(|r| r.laps_completed as f64).sum::<f64>() / n / race_laps)
            .clamp(0.0, 1.0),
        tire_wear: (carros.iter().map(|r| r.final_tire_wear).sum::<f64>() / n).clamp(0.0, 1.0),
    }
}

/// Custo operacional médio de UMA rodada da categoria — a base de que todos os pesos
/// financeiros da rodada são fração.
pub(crate) fn round_operating_base(
    categoria: &str,
    classe: Option<&str>,
    rounds_in_season: f64,
) -> f64 {
    category_finance_scale_for(categoria, classe).operating_cost_midpoint()
        / rounds_in_season.max(1.0)
}

pub(super) fn calculate_team_round_finance_context(
    team: &Team,
    lineup_public_presence: f64,
    // ATRAÇÃO DE PÚBLICO desta equipe nesta etapa (`public_presence::atracao`). Vem
    // pronta de fora porque depende da classificação VIVA do campeonato, que só quem
    // tem o grid inteiro em mãos sabe calcular. É a cota da bilheteria; a presença
    // acima segue sendo a do patrocínio.
    team_audience_appeal: f64,
    added_points: i32,
    added_victories: i32,
    added_podiums: i32,
    best_result: i32,
    salary_expense: f64,
    rounds_in_season: f64,
    economic_health: GlobalEconomicHealth,
    car_maintenance_cost: f64,
    // O que esta etapa cobrou deste time em campo: onde foi e o que os carros dele fizeram.
    // Dimensiona a fatura de operação — cujo total É o `event_operations_cost`.
    round_operation: RoundOperationContext,
    // Os fatos físicos desta etapa (duração real sorteada, carros inscritos). Só o modelo
    // novo de despesa os consome — ver `despesa.rs`.
    etapa: EtapaFisica,
    // Bilheteria: prestígio do evento (score do event_interest, pré-vendido) + ATRAÇÃO
    // DE PÚBLICO somada do grid + nº de times, para dividir o bolo de público por cota.
    // O denominador é atração, não presença — ver o bloco da bilheteria mais abaixo.
    event_prestige_score: f64,
    grid_total_appeal: f64,
    grid_team_count: f64,
) -> TeamRoundFinanceContext {
    calculate_team_round_finance_context_modelo(
        team,
        lineup_public_presence,
        team_audience_appeal,
        added_points,
        added_victories,
        added_podiums,
        best_result,
        salary_expense,
        rounds_in_season,
        economic_health,
        car_maintenance_cost,
        round_operation,
        etapa,
        event_prestige_score,
        grid_total_appeal,
        grid_team_count,
        CoeficientesDeReceita::default(),
        despesa_da_rodada,
    )
}

/// Igual a [`calculate_team_round_finance_context`], com os coeficientes de receita
/// explícitos. Só o harness de calibração passa algo diferente do `Default`.
///
/// Usa a etapa de REFERÊNCIA da divisão (duração nominal, lotação declarada) e a conta de
/// despesa de produção. Quem precisa medir a conta antiga lado a lado com esta chama
/// [`calculate_team_round_finance_context_modelo`] diretamente.
pub(crate) fn calculate_team_round_finance_context_com(
    team: &Team,
    lineup_public_presence: f64,
    team_audience_appeal: f64,
    added_points: i32,
    added_victories: i32,
    added_podiums: i32,
    best_result: i32,
    salary_expense: f64,
    rounds_in_season: f64,
    economic_health: GlobalEconomicHealth,
    car_maintenance_cost: f64,
    round_operation: RoundOperationContext,
    event_prestige_score: f64,
    grid_total_appeal: f64,
    grid_team_count: f64,
    coef: CoeficientesDeReceita,
) -> TeamRoundFinanceContext {
    calculate_team_round_finance_context_modelo(
        team,
        lineup_public_presence,
        team_audience_appeal,
        added_points,
        added_victories,
        added_podiums,
        best_result,
        salary_expense,
        rounds_in_season,
        economic_health,
        car_maintenance_cost,
        round_operation,
        EtapaFisica::de_referencia(&team.categoria, team.classe.as_deref()),
        event_prestige_score,
        grid_total_appeal,
        grid_team_count,
        coef,
        despesa_da_rodada,
    )
}

/// A conta da rodada com TUDO explícito: coeficientes de receita e o cálculo da despesa.
///
/// O último parâmetro é sempre [`despesa_da_rodada`] em produção. Ele é um ponteiro, e não
/// uma chamada direta, por um motivo só: o comparador do harness precisa rodar a conta ANTIGA
/// (congelada em `super::tests::despesa_legada`) contra esta sob a mesma semente, dentro do
/// mesmo binário. Confrontar dois relatórios de execuções diferentes não prova nada.
pub(crate) fn calculate_team_round_finance_context_modelo(
    team: &Team,
    lineup_public_presence: f64,
    // ATRAÇÃO DE PÚBLICO desta equipe nesta etapa (`public_presence::atracao`). Vem
    // pronta de fora porque depende da classificação VIVA do campeonato, que só quem
    // tem o grid inteiro em mãos sabe calcular. É a cota da bilheteria; a presença
    // acima segue sendo a do patrocínio.
    team_audience_appeal: f64,
    added_points: i32,
    added_victories: i32,
    added_podiums: i32,
    best_result: i32,
    salary_expense: f64,
    rounds_in_season: f64,
    economic_health: GlobalEconomicHealth,
    car_maintenance_cost: f64,
    round_operation: RoundOperationContext,
    etapa: EtapaFisica,
    event_prestige_score: f64,
    grid_total_appeal: f64,
    grid_team_count: f64,
    coef: CoeficientesDeReceita,
    calculo_da_despesa: CalculoDaDespesa,
) -> TeamRoundFinanceContext {
    let income_modifier = economy_income_modifier(economic_health);
    let cost_modifier = economy_cost_modifier(economic_health);
    let scale = category_finance_scale_for(&team.categoria, team.classe.as_deref());
    let plan = calculate_financial_plan(team);
    let round_operating_base = scale.operating_cost_midpoint() / rounds_in_season.max(1.0);
    // ── O patrocínio saiu do ESTOQUE ────────────────────────────────────────────────
    //
    // O termo-base era `expected_cash_midpoint / rodadas × 0,27`: o caixa que a equipe
    // deveria TER no banco, dividido por rodadas, virando receita. É a seção 2.1 aplicada
    // ao canal mais importante do jogo — o patrocínio é ~70% da receita, e ele escalava com
    // a riqueza esperada da categoria em vez de escalar com o custo de operar nela.
    //
    // Agora a base é o custo operacional da rodada, que é o que o patrocínio de fato
    // acompanha no mundo real: um patrocinador banca uma fração da operação, não uma fração
    // do patrimônio.
    //
    // **O coeficiente NÃO foi mexido.** `patrocinio_base` continua 0,27, e é isso que o
    // torna um alvo parado para a sessão de receita recalibrar em cima. O que muda é o
    // significado: antes 0,27 do caixa-médio cobria ~52% do custo operacional anual; agora
    // 0,27 do operacional cobre 27%. A diferença é o buraco que a recalibração precisa
    // fechar, e ela é deliberada, não um efeito colateral.
    let sponsorship_income = (round_operating_base * coef.patrocinio_base
        + team.reputacao * round_operating_base * 0.004
        + plan.budget_index * round_operating_base * 0.002
        + lineup_public_presence * round_operating_base * coef.fama_patrocinio)
        * income_modifier;
    let result_bonus = (added_points as f64 * coef.bonus_por_ponto
        + added_victories as f64 * coef.bonus_por_vitoria
        + added_podiums as f64 * coef.bonus_por_podio
        + if best_result <= 5 {
            coef.bonus_top5
        } else {
            0.0
        })
        * round_operating_base
        * coef.escala_do_bonus
        * income_modifier;
    let partial_prize_income = added_points as f64
        * coef.premio_parcial_por_ponto
        * round_operating_base
        * income_modifier;
    let aid_income = team.parachute_payment_remaining.min(25_000.0);
    // ── A DESPESA saiu do orçamento e virou conta física ─────────────────────────────
    //
    // As três linhas abaixo eram frações de `round_operating_base` — 0,62 de operação, 0,38
    // de estrutura, 0,16 de técnica — somando ~1,16× a base POR RODADA contra uma âncora que
    // diz que o ano inteiro custa 1,00× a base × rodadas. Era esse descompasso que fazia a
    // despesa medida ficar em 1,65–2,27× o custo operacional declarado da categoria, e é ele
    // que está embutido como fator ~1,96 em todo coeficiente de receita calibrado até agora.
    //
    // Agora a operação é `quantidade × preço` da etapa concreta e a estrutura é a fatia desta
    // rodada nos recorrentes do ano. O modelo antigo saiu da produção; ele só continua
    // alcançável pelo harness, congelado em `super::tests::despesa_legada`.
    let despesa = calculo_da_despesa(
        team,
        round_operating_base,
        cost_modifier,
        rounds_in_season,
        round_operation,
        etapa,
    );
    let event_operations_cost = despesa.operacao;
    let structural_maintenance_cost = despesa.estrutura;
    // A depreciação REAL do carro (Sistema de Nível do Carro): o custo é o que o cérebro
    // decidiu gastar em peças nesta rodada. Entra cru — foi decidido com o preço cru — e no
    // modelo físico ele é a ÚNICA parcela desta linha: o antigo `0,16 × base` era um
    // investimento técnico sem referente, e melhoria de estrutura é outro botão
    // (`economia::desenvolvimento`).
    let technical_investment_cost = despesa.tecnica + car_maintenance_cost.max(0.0);
    let debt_service_cost = debt_service_for_state(team.debt_balance, &team.financial_state);

    // Bilheteria: a cota do time no bolo de público. A grandeza NÃO é mais a presença
    // do lineup — é a ATRAÇÃO DE PÚBLICO (`public_presence::atracao`): fama do lineup
    // × competitividade + vínculo local + história. A troca existe porque a presença
    // sozinha é comprimida (todo grid ~40 ± 8), então `presença/total` colapsava em
    // `1/N` e a bilheteria pagava igual para todo mundo — era por isso que
    // `portao_piso` era um botão morto. A posição no campeonato tem espalhamento total
    // por construção, então a atração separa o grid mesmo com a fama ainda subindo.
    //
    // O PATROCÍNIO segue lendo `lineup_public_presence` logo acima, e isso é
    // deliberado: é a fama do ROSTO que capta patrocinador, enquanto é a CORRIDA que
    // enche arquibancada. São dois canais com causas diferentes.
    let gate_income = crate::finance::cashflow::calculate_gate_income_with(
        event_prestige_score,
        round_operating_base,
        team_audience_appeal,
        grid_total_appeal,
        grid_team_count,
        income_modifier,
        coef.portao_coef,
        coef.portao_piso,
    );

    TeamRoundFinanceContext {
        sponsorship_income,
        gate_income,
        result_bonus,
        partial_prize_income,
        aid_income,
        salary_expense,
        event_operations_cost,
        structural_maintenance_cost,
        technical_investment_cost,
        debt_service_cost,
        // A decomposição viaja JUNTO com o dinheiro. O ledger grava esta struct, não uma
        // segunda conta feita na hora de gravar — é o que impede a fatura do dossiê de
        // divergir do caixa.
        linhas: despesa.linhas_do_ledger,
    }
}

/// O que o bloco de fama do fim de semana devolve ao caller: o peso da corrida na
/// cobertura (notícias) e a repercussão PÚBLICA que a tela de resultado mostra.
pub(crate) struct PostRaceFameOutcome {
    pub news_importance_bias: i32,
    pub interest_tier: InterestTier,
    /// Repercussão ancorada no JOGADOR — `None` quando ele não correu esta corrida
    /// (fim de semana 100% IA). É o que alimenta o bloco "Repercussão" da tela.
    pub player_repercussion: Option<EventRepercussionSummary>,
}

pub(super) fn compute_post_race_impact(
    conn: &rusqlite::Connection,
    race_entry: &CalendarEntry,
    player_race: &RaceResult,
) -> Option<RealizedEventInterest> {
    let category = get_category_config(&race_entry.categoria)?;
    let total_rodadas = category.corridas_por_temporada as i32;
    let player = driver_queries::get_player_driver(conn).ok()?;
    let champ = standings_queries::get_championship_context(conn, &race_entry.categoria).unwrap_or(
        ChampionshipContext {
            player_position: 0,
            gap_to_leader: 0,
        },
    );
    let player_result = player_race.race_results.iter().find(|r| r.is_jogador)?;

    let remaining = total_rodadas - race_entry.rodada;
    let is_title_decider = remaining <= 2 && champ.gap_to_leader <= 50 && champ.player_position > 0;
    let is_final_round_decider = race_entry.rodada == total_rodadas && is_title_decider;

    let ctx = EventInterestContext {
        categoria: race_entry.categoria.clone(),
        season_phase: race_entry.season_phase,
        rodada: race_entry.rodada,
        total_rodadas,
        week_of_year: race_entry.week_of_year,
        track_id: race_entry.track_id as i32,
        track_name: race_entry.track_name.clone(),
        is_player_event: true,
        player_championship_position: if champ.player_position > 0 {
            Some(champ.player_position)
        } else {
            None
        },
        player_media: Some(player.atributos.midia as f32),
        championship_gap_to_leader: if champ.gap_to_leader > 0 || champ.player_position == 1 {
            Some(champ.gap_to_leader)
        } else {
            None
        },
        is_title_decider_candidate: is_title_decider,
        thematic_slot: race_entry.thematic_slot,
    };
    let expected = calculate_expected_event_interest(&ctx);
    Some(calculate_realized_event_interest(
        &expected,
        &ctx,
        Some(player_result.finish_position),
        Some(player_result.grid_position),
        player_result.finish_position == 1,
        player_result.finish_position <= 3 && !player_result.is_dnf,
        player_result.is_dnf,
        is_final_round_decider,
    ))
}

/// Interesse do evento "de local" para uma corrida SEM o jogador (categorias da IA):
/// só o contexto do evento (categoria/fase/rodada/papel narrativo), sem protagonismo
/// nem resultado de um piloto específico. Escala o impacto de fama dos pilotos IA
/// quando o jogador não corre aquela categoria — é o que faz o astro nascer no mundo
/// todo, não só na categoria do jogador. `None` se a categoria não tem config.
pub(super) fn compute_venue_impact(race_entry: &CalendarEntry) -> Option<RealizedEventInterest> {
    let category = get_category_config(&race_entry.categoria)?;
    let ctx = EventInterestContext {
        categoria: race_entry.categoria.clone(),
        season_phase: race_entry.season_phase,
        rodada: race_entry.rodada,
        total_rodadas: category.corridas_por_temporada as i32,
        week_of_year: race_entry.week_of_year,
        track_id: race_entry.track_id as i32,
        track_name: race_entry.track_name.clone(),
        is_player_event: false,
        player_championship_position: None,
        player_media: None,
        championship_gap_to_leader: None,
        is_title_decider_candidate: false,
        thematic_slot: race_entry.thematic_slot,
    };
    let expected = calculate_expected_event_interest(&ctx);
    Some(calculate_realized_event_interest(
        &expected, &ctx, None, None, false, false, false, false,
    ))
}

/// Score de PRESTÍGIO "de local" do evento (pré-vendido / esperado), sem protagonismo
/// nem resultado — a mesma base que dimensiona a pressão de "casa cheia". Alimenta a
/// BILHETERIA (Fase 3 do Estrelato): evento maior (endurance, final, decisão) lota mais
/// → bolo de público maior. `0.0` se a categoria não tem config (bilheteria some).
pub(super) fn venue_prestige_score(race_entry: &CalendarEntry) -> f64 {
    let Some(category) = get_category_config(&race_entry.categoria) else {
        return 0.0;
    };
    let ctx = EventInterestContext {
        categoria: race_entry.categoria.clone(),
        season_phase: race_entry.season_phase,
        rodada: race_entry.rodada,
        total_rodadas: category.corridas_por_temporada as i32,
        week_of_year: race_entry.week_of_year,
        track_id: race_entry.track_id as i32,
        track_name: race_entry.track_name.clone(),
        is_player_event: false,
        player_championship_position: None,
        player_media: None,
        championship_gap_to_leader: None,
        is_title_decider_candidate: false,
        thematic_slot: race_entry.thematic_slot,
    };
    calculate_expected_event_interest(&ctx).score as f64
}

/// Aplica TODOS os efeitos de FAMA de uma corrida concluída. Vale IGUAL pra corrida
/// SIMULADA offline e pra corrida IMPORTADA do iRacing — ambas produzem um `RaceResult`
/// e persistem pelo mesmo caminho, então a fama reage idêntico. Efeitos:
/// - mídia/motivação do JOGADOR (modulada pelo carisma dele),
/// - impacto de mídia nos pilotos IA notáveis (vencedor/pole/pódio/incidente/lesão,
///   modulado pelo carisma de cada um),
/// - deriva leve de CARISMA por marcos do fim de semana ("drama vende"),
/// - decaimento passivo da fama de todo o grid que correu.
/// Retorna o [`PostRaceFameOutcome`] pro caller alimentar as notícias e a tela.
pub(crate) fn apply_post_race_fame(
    conn: &rusqlite::Connection,
    race_entry: &CalendarEntry,
    result: &RaceResult,
    new_injuries: &[Injury],
) -> Result<PostRaceFameOutcome, String> {
    // ID do jogador — excluído do bloco world-facing (já tratado no player-facing).
    // Vazio quando o jogador não corre esta categoria (corrida 100% IA).
    let excluded_driver_id = result
        .race_results
        .iter()
        .find(|r| r.is_jogador)
        .map(|r| r.pilot_id.clone())
        .unwrap_or_default();

    // Interesse REALIZADO ancorado no jogador — `Some` só se ele correu esta corrida.
    let player_realized = compute_post_race_impact(conn, race_entry, result);

    // ── Player-facing: mídia/motivação do JOGADOR (só quando ele correu) ──
    if let Some(realized) = &player_realized {
        if let Ok(player) = driver_queries::get_player_driver(conn) {
            let player_result = result.race_results.iter().find(|r| r.is_jogador);
            // **A MESMA escada do mundo**, literalmente a mesma função
            // ([`crate::fame::fame_finish_base_delta`]). Havia duas tabelas escritas em
            // lugares diferentes e elas não batiam: aqui o pódio pagava +2,0 e o top-5
            // +1,0, onde `event_interest::public_impact` pagava +1,0 e +0,5 para o mesmo
            // resultado na mesma corrida.
            let base_midia_delta = player_result
                .map(|r| {
                    crate::fame::fame_finish_base_delta(
                        r.finish_position,
                        r.is_dnf,
                        crate::fame::fame_visibility_last_position(result.race_results.len()),
                    )
                })
                .unwrap_or(crate::fame::FAME_FINISH_FORA);
            // Carisma modula o delta de fama: estrela ganha mais e amortece a perda
            // (mal cai terminando mal); apagado ganha menos e sangra. O tier da categoria
            // escala tudo — a mesma vitória vale mais no Endurance que na Rookie.
            let tier_categoria =
                crate::constants::categories::get_category_config(&race_entry.categoria)
                    .map(|c| c.tier)
                    .unwrap_or(0);
            // E o MESMO multiplicador de importância do evento. Era
            // `realized.media_delta_modifier` (0,75–1,60, quase sempre ~1,0) contra o
            // `fame_event_interest_mult` (0,30–2,50) do mundo — duas modulações
            // diferentes do mesmo `final_score`. Ver a tabela em
            // [`crate::event_interest::fame_event_interest_mult`].
            let raw_midia_delta = base_midia_delta
                * crate::event_interest::fame_event_interest_mult(&realized.final_tier)
                * crate::fame::fame_category_tier_mult(tier_categoria);
            let carisma_midia_delta =
                crate::fame::apply_carisma_to_fame_delta(raw_midia_delta, player.atributos.carisma);
            // O mesmo portão da IA: subir de 90 para 95 custa muito mais que subir de 50
            // para 55, e é o que impede a fama do jogador de saturar na metade da carreira.
            let new_midia = crate::fame::apply_fame_gain(player.atributos.midia, carisma_midia_delta);
            let _ = driver_queries::update_driver_midia(conn, &player.id, new_midia);

            let base_mot_delta = if player_result.is_some_and(|r| r.finish_position == 1) {
                4.0
            } else if player_result.is_some_and(|r| r.finish_position <= 3 && !r.is_dnf) {
                2.5
            } else if player_result.is_some_and(|r| r.finish_position <= 5) {
                1.0
            } else if player_result.is_some_and(|r| r.is_dnf) {
                -3.5
            } else {
                -0.5
            };
            let new_motivacao = (player.motivacao
                + base_mot_delta * realized.motivation_delta_modifier as f64)
                .clamp(0.0, 100.0);
            let _ = driver_queries::update_driver_motivation(conn, &player.id, new_motivacao);
        }
    }

    // Interesse do evento para o mundo: do jogador se ele correu; senão, "de local"
    // (categoria da IA). É isto que faz a fama valer em TODAS as categorias.
    // Repercussão pública do JOGADOR — calculada aqui porque `player_realized` é
    // consumido logo abaixo (o `or_else` do fallback "de local" o move).
    let player_repercussion = player_realized.as_ref().map(to_repercussion_summary);
    let realized = match player_realized.or_else(|| compute_venue_impact(race_entry)) {
        Some(r) => r,
        // Sem config de categoria → nada a fazer.
        None => {
            return Ok(PostRaceFameOutcome {
                news_importance_bias: 0,
                interest_tier: InterestTier::Baixo,
                player_repercussion,
            })
        }
    };
    let post_race_bias = realized.news_importance_bias;
    let interest_tier = realized.final_tier.clone();

    // ── World-facing: impacto de mídia nos pilotos IA notáveis (SEMPRE) ──
    // `excluded_driver_id` (jogador) omitido; vazio nas corridas 100% IA (ninguém excluído).
    let main_incident_pilot: Option<String> = result
        .notable_incident_pilot_ids
        .iter()
        .find(|id| id.as_str() != excluded_driver_id.as_str())
        .cloned();
    // Papéis de chegada, em faixas exclusivas: cada piloto entra na melhor que alcançou.
    // O DNF nunca conta — terminar no top 10 andando é o que constrói nome.
    let faixa_de_chegada = |min: i32, max: i32| -> Vec<&str> {
        result
            .race_results
            .iter()
            .filter(|r| {
                r.finish_position >= min
                    && r.finish_position <= max
                    && !r.is_dnf
                    && r.pilot_id != result.winner_id
            })
            .map(|r| r.pilot_id.as_str())
            .collect()
    };
    // A faixa de "aparece" é relativa ao tamanho do campo — ver `fame_visibility_last_position`.
    let ultima_posicao_visivel =
        crate::fame::fame_visibility_last_position(result.race_results.len());
    let podium_pilot_ids = faixa_de_chegada(2, 3);
    let top5_pilot_ids = faixa_de_chegada(4, 5.min(ultima_posicao_visivel));
    let top10_pilot_ids = faixa_de_chegada(6, ultima_posicao_visivel);
    let category_tier = crate::constants::categories::get_category_config(&race_entry.categoria)
        .map(|c| c.tier)
        .unwrap_or(0);
    let race_ctx = crate::event_interest::RaceEventContext {
        winner_id: &result.winner_id,
        pole_sitter_id: &result.pole_sitter_id,
        podium_ids: &podium_pilot_ids,
        top5_ids: &top5_pilot_ids,
        top10_ids: &top10_pilot_ids,
        main_incident_pilot_id: main_incident_pilot.as_deref(),
        excluded_driver_id: &excluded_driver_id,
        category_tier,
    };
    let ai_media_impacts =
        crate::event_interest::compute_public_media_impacts(&race_ctx, new_injuries, &realized);

    for impact in &ai_media_impacts {
        // Carisma do piloto IA modula quanto de fama aquele feito rende. Neutro 50 sem leitura.
        let carisma = driver_queries::get_driver_carisma(conn, &impact.driver_id)
            .ok()
            .flatten()
            .unwrap_or(50.0);
        let delta = crate::fame::apply_carisma_to_fame_delta(impact.delta, carisma);
        driver_queries::update_driver_midia_delta(conn, &impact.driver_id, delta).map_err(|e| {
            format!(
                "Falha ao aplicar impacto de mídia para '{}': {e}",
                impact.driver_id
            )
        })?;
    }

    // ── Deriva leve do CARISMA por marcos do fim de semana ("drama vende") ──
    for pid in &result.notable_incident_pilot_ids {
        let _ = driver_queries::bump_driver_carisma(conn, pid, crate::fame::CARISMA_DRIFT_INCIDENT);
    }
    if matches!(
        interest_tier,
        InterestTier::MuitoAlto | InterestTier::EventoPrincipal
    ) && !result.winner_id.is_empty()
    {
        let _ = driver_queries::bump_driver_carisma(
            conn,
            &result.winner_id,
            crate::fame::CARISMA_DRIFT_BIG_WIN,
        );
    }
    // Remontada dos protagonistas (jogador e vencedor): subir muitas posições é magnético.
    for r in result
        .race_results
        .iter()
        .filter(|r| r.is_jogador || r.pilot_id == result.winner_id)
    {
        if !r.is_dnf && r.grid_position - r.finish_position >= crate::fame::COMEBACK_MIN_POSITIONS {
            let _ = driver_queries::bump_driver_carisma(
                conn,
                &r.pilot_id,
                crate::fame::CARISMA_DRIFT_COMEBACK,
            );
        }
    }

    // ── Decaimento passivo da FAMA de todo o grid que correu ──
    // O piso é PESSOAL: quem tem títulos, vitórias e anos na elite não decai para o
    // mesmo lugar de um estreante. É essa trava que impede a população de colapsar
    // toda na mesma faixa (e a presença pública das equipes de ficar idêntica).
    let ids_do_grid: Vec<&str> = result
        .race_results
        .iter()
        .map(|r| r.pilot_id.as_str())
        .collect();
    let pedigrees = driver_queries::get_fame_pedigrees(conn, &ids_do_grid).unwrap_or_default();
    for r in &result.race_results {
        let piso = pedigrees
            .get(&r.pilot_id)
            .map(|p| crate::fame::personal_fame_floor(p.titulos, p.vitorias, p.temporadas_elite))
            .unwrap_or(crate::fame::FAME_DECAY_FLOOR);
        let _ = driver_queries::decay_driver_fame(
            conn,
            &r.pilot_id,
            piso,
            crate::fame::FAME_DECAY_BASE_RATE,
        );
    }

    Ok(PostRaceFameOutcome {
        news_importance_bias: post_race_bias,
        interest_tier,
        player_repercussion,
    })
}
