//! Tick de manutenção pós-corrida: as condições reais que modulam o desgaste
//! persistido e a manutenção do carro de um time depois da corrida.

use super::*;

// ===================== Tick de manutenção por corrida =====================

/// Condições REAIS da corrida que acabou, que modulam o desgaste PERSISTIDO (grade toda): a
/// pista (qual peça sofre) e o clima (chuva → eletrônica; calor+umidade → térmica; vento →
/// suspensão/asas). O estilo de pilotagem do jogador é aplicado por cima, só no carro dele
/// (ver wiring do import).
#[derive(Debug, Clone, Copy)]
pub struct WearConditions {
    /// Demanda PHA normalizada da pista corrida (via [`maintenance_demand`]).
    pub track_pha: (f64, f64, f64),
    /// Clima da rodada (mesma `WeatherStory` que o iRacing roda).
    pub weather: crate::car::breakdown::Weather,
    /// Duração da corrida (min) da categoria. Acima do gate de enduro, o desgaste de peça (→
    /// custo) sobe pra grade toda; parada real alivia. Sprint (≤ gate) → sem efeito.
    pub duracao_min: u8,
}

impl WearConditions {
    /// Corrida de referência neutra → todos os mults = 1.0 (economia inalterada). Usada em
    /// caminhos sem pista/clima resolvidos (testes, robustez). Duração de sprint (30 min).
    pub fn neutral() -> Self {
        Self {
            track_pha: (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0),
            weather: crate::car::breakdown::Weather::NEUTRAL,
            duracao_min: 30,
        }
    }

    /// Resolve a partir da pista corrida + clima da rodada + duração da categoria.
    pub fn from_race(
        track_id: u32,
        weather: crate::car::breakdown::Weather,
        duracao_min: u8,
    ) -> Self {
        Self {
            track_pha: maintenance_demand(&[track_id]),
            weather,
            duracao_min,
        }
    }
}

/// Manutenção do carro de UM time após a corrida: o cérebro decide (trocar/esticar/
/// degradar/upgrade) olhando o caixa e as próximas pistas (cortadas pelo horizonte do
/// time), aplica o desgaste (modulado pelas `conditions` reais da corrida) e persiste.
/// Devolve o **custo gasto** — que vira depreciação REAL na fatura (technical_investment_cost).
/// Chamado por time dentro do `persist_race_result_tx` (que já tem guard de idempotência),
/// 1×/rodada.
/// Manutenção do carro de UM time após a corrida. Ver [`maintain_team_car`]; aqui o
/// `player_pits` é o nº de paradas REAIS do jogador (do SDK) — só usado quando `player_style`
/// é `Some` (o carro do jogador). A IA modela as paradas pela duração.
pub fn maintain_team_car(
    conn: &Connection,
    team: &Team,
    category_id: &str,
    season_number: i32,
    all_upcoming_track_ids: &[u32],
    conditions: WearConditions,
    player_style: Option<crate::car::driving_style::StyleFactors>,
) -> Result<f64, DbError> {
    maintain_team_car_pits(
        conn,
        team,
        category_id,
        season_number,
        all_upcoming_track_ids,
        conditions,
        player_style,
        false,
        0,
        &[],
        0,
    )
}

/// Igual a [`maintain_team_car`], recebendo o alívio de gasto de peça do enduro do carro do
/// JOGADOR: `is_player_car` marca o carro dele (o único que usa o pit REAL do SDK, `player_pits`,
/// 10%/parada, teto 30%); todo o resto (IA) modela as paradas pela duração. NÃO confundir com
/// `player_style` (que também vem `Some` na desconfiança mecânica de times de IA).
#[allow(clippy::too_many_arguments)]
pub fn maintain_team_car_pits(
    conn: &Connection,
    team: &Team,
    category_id: &str,
    season_number: i32,
    all_upcoming_track_ids: &[u32],
    conditions: WearConditions,
    player_style: Option<crate::car::driving_style::StyleFactors>,
    is_player_car: bool,
    player_pits: u32,
    // FEEDBACK FÍSICO DA QUEBRA (§4.6): peças DESTE carro que largaram na corrida e a severidade.
    // Leve → segue; Grave → fim de vida; DNF → destruída (troca forçada, a débito). Vazio = sem
    // quebra (sim offline / corridas fora da categoria do jogador).
    race_breakdowns: &[(PartType, crate::car::breakdown::Severity)],
    // CONTATO DE DISPUTA: quantos roda-a-roda os carros DESTE time levaram na corrida. Vem da
    // sim offline (é o único lugar que sabe disso) e é o que finalmente dá consequência física
    // à batida da IA: castiga aero/laterais/suspensão e, se a peça já estava no fim, força a
    // troca a débito. 0 = corrida sem contato, ou caminho que não conta contato.
    contact_hits: u32,
) -> Result<f64, DbError> {
    use crate::car::breakdown::Severity;

    // Usa o carro anexado; senão carrega; senão semeia neutro (save antigo/robustez).
    // O seed de emergência usa a chave COM classe — um GT4 do Endurance não pode nascer
    // com o teto do LMP2 só porque a categoria é a mesma.
    let mut car = match &team.car {
        Some(car) => car.clone(),
        None => team_car::get_team_car(conn, &team.id)?
            .unwrap_or_else(|| seed_car(&team.car_category_key(), 0.5)),
    };

    // Um carro NUNCA pode estar acima do teto da EQUIPE (classe + natureza) — regride a ele
    // ao entrar na categoria. Sem isto, um time REBAIXADO carregaria o carro alto da
    // categoria anterior pra sempre (Replace mantém o nível; o teto só bloqueia upgrade,
    // não reposição), e uma privateer promovida manteria o nível de fábrica.
    let ceiling = team.car_ceiling();
    for part in car.parts.iter_mut() {
        if part.level > ceiling {
            part.level = ceiling;
        }
    }

    // FEEDBACK FÍSICO DA QUEBRA (§4.6), ANTES do cérebro decidir: a quebra ao vivo tem
    // consequência no save (recopla o desacoplamento). Regra escolhida (variante simples, medida
    // como a que fecha o runaway): LEVE → a peça segue (só perdeu rendimento); GRAVE ou DNF → a
    // peça é TROCADA à força, mesmo sem caixa (a débito). Assim uma peça que custou tempo/tirou o
    // carro vira NOVA na próxima — NÃO requebra — e o buraco do time pobre vira DÍVIDA, não o
    // mesmo defeito eterno. (A variante graduada deixava o Grave do pobre requebrar; descartada.)
    let mut forced_parts: Vec<PartType> = Vec::new();

    // CONTATO DE DISPUTA, antes de tudo: o castigo dos roda-a-roda entra como desgaste nas
    // peças e ENTÃO o cérebro decide olhando o carro já castigado. A peça que já estava no fim
    // da vida quando levou a pancada é destruída — cai em `forced_parts` junto com a quebra ao
    // vivo e segue a mesma regra dura: trocada mesmo sem caixa, a débito.
    let contato = crate::car::crash::apply_contact_wear(&mut car, category_id, contact_hits);
    for &pt in &contato.destroyed {
        if let Some(p) = car.parts.iter_mut().find(|p| p.part_type == pt) {
            p.wear = p.wear.max(1.0); // o cérebro tem de vê-la no fim de vida
        }
        forced_parts.push(pt);
    }

    for &(pt, sev) in race_breakdowns {
        if matches!(sev, Severity::Heavy | Severity::Dnf) {
            if let Some(p) = car.parts.iter_mut().find(|p| p.part_type == pt) {
                p.wear = p.wear.max(1.0); // garante que o cérebro a veja no fim de vida
            }
            forced_parts.push(pt);
        }
    }

    // A janela de planejamento é cortada pelo horizonte do time.
    let horizon = planning_horizon(&team.id, season_number);
    let window: &[u32] = match horizon.lookahead() {
        Some(n) => &all_upcoming_track_ids[..all_upcoming_track_ids.len().min(n)],
        None => all_upcoming_track_ids,
    };

    // Cota de DESENVOLVIMENTO desta corrida: conta as etapas que ainda restam na temporada
    // (a lista INTEIRA, não a cortada pelo horizonte — a cadência é do calendário, não da
    // miopia do time). Fora da janela o time faz manutenção e não sobe nível nenhum.
    let max_upgrades =
        upgrades_permitidos_nesta_corrida(&team.id, all_upcoming_track_ids.len());

    let mut plan = decide_car_maintenance(team, &car, category_id, window, Some(max_upgrades));
    // Grave/DNF = troca OBRIGATÓRIA, mesmo se o cérebro não teve caixa (nem sempre por Replace: o
    // pobre teria Degradado). O custo extra estoura o orçamento → cai na fatura → o time paga ou
    // vai a DÍVIDA. É isto que transforma o runaway do pobre em espiral de dívida (não o mesmo
    // defeito eterno): a peça vira NOVA na próxima, e o buraco é financeiro.
    let mut forced_cost = 0.0;
    for &pt in &forced_parts {
        if plan.actions.get(&pt) != Some(&PartAction::Replace) {
            if let Some(part) = car.part(pt) {
                forced_cost += crate::car::wear::replace_cost(category_id, part);
            }
            plan.actions.insert(pt, PartAction::Replace);
        }
    }
    // `contato.cost` é o reparo dos amassados, cobrado na fatura DESTA rodada. É o que faz a
    // batida pesar no caixa: o desgaste que ela deixa é lento demais para ser sentido.
    let cost = plan.estimated_cost + forced_cost + contato.cost;
    // O desgaste desta corrida responde à pista + clima reais (grade toda). Corrida neutra
    // → mults ~1.0, e a economia calibrada não muda.
    let mut wear_mults =
        crate::car::breakdown::conditions_wear_mults(conditions.track_pha, conditions.weather);
    // ENDURO (corrida longa): o desgaste de peça (→ custo) sobe com a duração pra GRADE TODA e
    // é aliviado por paradas reais. O jogador usa suas paradas do SDK; a IA modela pela duração.
    // Sprint → mult 1.0, economia inalterada. "Todos sentem" (o jogador também paga o sobrecusto).
    let genuine_pits = if is_player_car {
        player_pits
    } else {
        crate::car::breakdown::modeled_ai_pits(conditions.duracao_min)
    };
    let enduro_mult =
        crate::car::breakdown::enduro_economy_wear_mult(conditions.duracao_min, genuine_pits);
    if (enduro_mult - 1.0).abs() > f64::EPSILON {
        for mult in wear_mults.values_mut() {
            *mult *= enduro_mult;
        }
    }
    // Só o carro do JOGADOR: o estilo de pilotagem multiplica os mults por peça (economizar
    // → desconto; abusar → mais desgaste). A IA passa `None`. Peça sem estilo → fator 1.0.
    if let Some(style) = player_style {
        for (&pt, mult) in wear_mults.iter_mut() {
            *mult *= style.factor_for(pt);
        }
    }
    // Tenda de nível só em categoria gerida (teto ≥ 3); spec fica de fora (§4.8).
    let apply_tent = category_ceiling(category_id) > 2;
    // Confiabilidade do time (§4.2): a qualidade do box faz o carro durar mais/menos.
    let rel_mult = crate::car::wear::reliability_life_mult(team.pit_crew_quality);
    apply_plan_scaled(&mut car, &plan, &wear_mults, apply_tent, rel_mult);
    team_car::upsert_team_car(conn, &team.id, &car)?;
    Ok(cost)
}
