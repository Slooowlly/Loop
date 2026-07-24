//! Cérebro de manutenção do carro por corrida (Sistema de Nível do Carro).
//!
//! Decide, para cada time e a cada corrida, o que fazer com as peças do carro
//! (trocar / esticar / degradar) e quando subir de nível — dentro do caixa e olhando o
//! calendário à frente. O jogador NÃO participa; seu time roda no mesmo cérebro. Este
//! módulo é o novo motor: convive com o `car_build_strategy` legado (perfil discreto)
//! até o chunk 5 aposentá-lo. Ver design §7 em
//! `docs/superpowers/specs/2026-07-17-car-level-system-design.md`.
#![allow(dead_code)] // Chunk 3: cérebro puro; wiring no tick pós-corrida vem no chunk 4.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::car::cost::{category_ceiling, part_cost};
use crate::car::seed::seed_car;
use crate::car::wear::{
    advance_race, advance_race_scaled, can_stretch, replace_cost, stretch_cost, wear_per_race,
    PartAction,
};
use crate::car::{Car, CarPart, PartType};
use crate::db::connection::DbError;
use crate::db::queries::team_car;
use crate::finance::planning::calculate_financial_plan;
use crate::models::team::Team;
use crate::simulation::track_profile::get_track_simulation_data;

/// Uma peça é considerada "relevante" para a demanda se contribui com ao menos esta
/// fração do máximo que qualquer peça oferece para aquela demanda.
const RELEVANCE_FRACTION: f64 = 0.4;

/// Quão "peaked" a demanda do calendário precisa ser para o time ESPECIALIZAR (spread
/// máx−mín da demanda normalizada). Abaixo disso, sobe tudo (carro balanceado).
const DEMAND_PEAK_THRESHOLD: f64 = 0.15;

/// Quanto o teto das peças IRRELEVANTES fica abaixo do teto da categoria quando o time
/// especializa — é o que cria o FOCO. Calibrável (chunk 8).
const FOCUS_GAP: u8 = 3;

// ===================== Horizonte de planejamento =====================

/// Quão longe o time enxerga o calendário ao planejar o carro. Varia por temporada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanningHorizon {
    /// Míope: só a próxima pista.
    SingleTrack,
    ThreeRaces,
    FiveRaces,
    /// Enxerga a temporada inteira.
    FullSeason,
}

impl PlanningHorizon {
    /// Nº de corridas à frente que o time considera. `None` = temporada inteira.
    pub fn lookahead(self) -> Option<usize> {
        match self {
            PlanningHorizon::SingleTrack => Some(1),
            PlanningHorizon::ThreeRaces => Some(3),
            PlanningHorizon::FiveRaces => Some(5),
            PlanningHorizon::FullSeason => None,
        }
    }
}

/// Horizonte determinístico por `(time, temporada)` — re-rola a cada temporada.
/// Distribuição: 20% míope / 30% 3 corridas / 30% 5 corridas / 20% temporada.
pub fn planning_horizon(team_id: &str, season: i32) -> PlanningHorizon {
    let mut seed: u32 = 0x9E37_79B9;
    for byte in team_id.bytes() {
        seed = seed.wrapping_mul(31).wrapping_add(byte as u32);
    }
    seed = seed
        .wrapping_mul(2_654_435_761)
        .wrapping_add((season as u32).wrapping_mul(40_503));
    // avalanche (mistura bem os bits para o módulo 100 não correlacionar com o input)
    seed ^= seed >> 15;
    seed = seed.wrapping_mul(0x2c1b_3c6d);
    seed ^= seed >> 12;

    match seed % 100 {
        0..=19 => PlanningHorizon::SingleTrack,
        20..=49 => PlanningHorizon::ThreeRaces,
        50..=79 => PlanningHorizon::FiveRaces,
        _ => PlanningHorizon::FullSeason,
    }
}

// ===================== Identidade / DNA de carro do time =====================

/// Viés inato de construção de carro do time — **PERSISTENTE** (não muda por temporada).
/// É a fonte de foco que a média do calendário NÃO lava: um time "de potência" sempre puxa
/// o carro pra potência, independente das pistas do calendário. O jogador não vê (o shape
/// continua oculto; isto é identidade de bastidor). Relaciona com a identidade viva do time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarFocus {
    Balanced,
    Power,
    Handling,
    Acceleration,
}

/// Peso do DNA na demanda efetiva (o resto vem do calendário). Alto o bastante para um time
/// focado cruzar o gatilho de especialização mesmo num calendário diverso (que lava pra
/// balanceado). Calibrável.
const DNA_DEMAND_WEIGHT: f64 = 0.6;

/// Intensidade do pico do DNA focado (fração do eixo dominante; o resto é dividido igual).
const DNA_PEAK: f64 = 0.70;

/// DNA determinístico e **estável** por time (sem temporada → não re-rola; é permanente).
/// Distribuição: 40% balanceado / 20% potência / 20% handling / 20% aceleração — foco é
/// maioria, mas generalistas continuam existindo.
pub fn team_car_focus(team_id: &str) -> CarFocus {
    let mut seed: u32 = 0x85EB_CA6B;
    for byte in team_id.bytes() {
        seed = seed.wrapping_mul(31).wrapping_add(byte as u32);
    }
    // avalanche (descorrelaciona o módulo 100 do input)
    seed ^= seed >> 15;
    seed = seed.wrapping_mul(0x2c1b_3c6d);
    seed ^= seed >> 13;

    match seed % 100 {
        0..=39 => CarFocus::Balanced,
        40..=59 => CarFocus::Power,
        60..=79 => CarFocus::Handling,
        _ => CarFocus::Acceleration,
    }
}

/// Demanda PHA `(P, H, A)` que o DNA sozinho pediria.
fn focus_demand(focus: CarFocus) -> (f64, f64, f64) {
    let lo = (1.0 - DNA_PEAK) / 2.0;
    match focus {
        CarFocus::Balanced => (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0),
        CarFocus::Power => (DNA_PEAK, lo, lo),
        CarFocus::Handling => (lo, DNA_PEAK, lo),
        CarFocus::Acceleration => (lo, lo, DNA_PEAK),
    }
}

/// Mistura a demanda do calendário com o DNA (persistente) do time. O DNA domina; o
/// calendário só modula. Para times balanceados, o resultado fica ~equilibrado (não foca).
fn blend_with_focus(calendar: (f64, f64, f64), focus: CarFocus) -> (f64, f64, f64) {
    let (cp, ch, ca) = calendar;
    let (fp, fh, fa) = focus_demand(focus);
    let w = DNA_DEMAND_WEIGHT;
    (
        w * fp + (1.0 - w) * cp,
        w * fh + (1.0 - w) * ch,
        w * fa + (1.0 - w) * ca,
    )
}

// ===================== Plano de manutenção =====================

/// Plano de manutenção do carro para UMA corrida.
#[derive(Debug, Clone, Default)]
pub struct CarMaintenancePlan {
    /// Ação por peça (peças ausentes = `Keep` no `advance_race`).
    pub actions: HashMap<PartType, PartAction>,
    /// Novo nível-alvo por peça (upgrade). Ausente = mantém o nível atual.
    pub target_levels: HashMap<PartType, u8>,
    /// Custo total estimado da manutenção nesta corrida.
    pub estimated_cost: f64,
}

/// Demanda PHA agregada das próximas pistas, normalizada em `(P, H, A)`.
pub fn maintenance_demand(upcoming_track_ids: &[u32]) -> (f64, f64, f64) {
    let (mut p, mut h, mut a) = (0.0, 0.0, 0.0);
    for &track_id in upcoming_track_ids {
        let data = get_track_simulation_data(track_id);
        p += data.power_weight;
        h += data.handling_weight;
        a += data.acceleration_weight;
    }
    let total = p + h + a;
    if total <= 0.0 {
        return (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0);
    }
    (p / total, h / total, a / total)
}

/// Spread da demanda (máx − mín das frações PHA) — 0 = equilibrada, ~1 = de ponto único.
fn demand_spread(demand: (f64, f64, f64)) -> f64 {
    let (p, h, a) = demand;
    let total = p + h + a;
    if total <= 0.0 {
        return 0.0;
    }
    let (p, h, a) = (p / total, h / total, a / total);
    p.max(h).max(a) - p.min(h).min(a)
}

/// Quão ALINHADA uma peça está com o eixo exigido pela demanda (não a magnitude bruta).
///
/// Usa a demanda **centrada** (subtrai o equilíbrio 1/3 de cada eixo): mede se a peça puxa
/// PARA o atributo exigido ou PARA LONGE dele. Sem centrar, peças de coeficiente enorme (o
/// motor pesa 5,78 em potência) pareceriam "relevantes" a qualquer demanda — e o carro só
/// inclinaria pra potência, nunca pra handling/aceleração. Centrado, o motor fica NEGATIVO
/// sob demanda de handling (ele puxa pra potência) e é corretamente de-investido.
fn part_relevance(part: PartType, demand: (f64, f64, f64)) -> f64 {
    let (pp, ph, pa) = part.pha_per_level();
    let (dp, dh, da) = demand;
    let third = 1.0 / 3.0;
    pp * (dp - third) + ph * (dh - third) + pa * (da - third)
}

/// Uma peça precisa de decisão nesta corrida? (esgotada, ou cruzaria 100% ao correr). O
/// incremento é ciente da vida EFETIVA da unidade (individual × tenda de nível, §4.1/§4.8):
/// peça durável/nível-5 gasta menos e é trocada mais tarde; limão/nível-extremo, antes.
fn needs_decision(part: &CarPart, apply_tent: bool) -> bool {
    part.spent
        || part.wear + wear_per_race(part.part_type) / crate::car::wear::part_effective_life(part, apply_tent)
            >= 1.0
}

/// Decide a manutenção do carro para a próxima corrida, dada a demanda PHA e o caixa.
///
/// 1) Peças no fim da vida são resolvidas por relevância: **trocar** (se cabe no caixa)
///    > **esticar** (relevante, elegível e cabe o custo reduzido) > **degradar**.
/// 2) Com caixa sobrando, um passe de **upgrade** sobe as peças abaixo do teto rumo a
///    ele (mais relevantes primeiro), +1 nível por corrida.
pub fn decide_maintenance(
    car: &Car,
    category_id: &str,
    budget: f64,
    demand: (f64, f64, f64),
) -> CarMaintenancePlan {
    let ceiling = category_ceiling(category_id);
    // Tenda de nível (§4.8) só em categoria GERIDA (teto ≥ 3); spec (rookie/amador) fica de fora.
    let apply_tent = ceiling > 2;
    let mut plan = CarMaintenancePlan::default();
    let mut budget = budget.max(0.0);

    let max_rel = PartType::ALL
        .iter()
        .map(|&pt| part_relevance(pt, demand))
        .fold(0.0_f64, f64::max);
    let rel_threshold = RELEVANCE_FRACTION * max_rel;

    // Numa janela peaked (o calendário/DNA puxa um atributo), o time ESPECIALIZA: as peças
    // IRRELEVANTES têm o teto rebaixado `FOCUS_GAP` abaixo do teto da categoria; as relevantes
    // vão ao teto. Numa janela equilibrada, todas vão ao teto (carro balanceado). O horizonte
    // modula quais pistas entram na demanda; o DNA do time (em `decide_car_maintenance`)
    // sustenta o pico que a média do calendário lavaria.
    let demand_peaked = demand_spread(demand) >= DEMAND_PEAK_THRESHOLD;
    let part_cap = |pt: PartType| -> u8 {
        if !demand_peaked || part_relevance(pt, demand) >= rel_threshold {
            ceiling
        } else {
            ceiling.saturating_sub(FOCUS_GAP).max(1)
        }
    };

    // 1) Peças no fim da vida, mais relevantes primeiro.
    let mut eol: Vec<CarPart> =
        car.parts.iter().copied().filter(|p| needs_decision(p, apply_tent)).collect();
    eol.sort_by(|a, b| {
        part_relevance(b.part_type, demand)
            .partial_cmp(&part_relevance(a.part_type, demand))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for part in &eol {
        let rc = replace_cost(category_id, part);
        let sc = stretch_cost(category_id, part);
        let relevant = part_relevance(part.part_type, demand) >= rel_threshold;
        // FOCO REAL: peça irrelevante ACIMA do teto de foco é DEIXADA degradar de propósito
        // (de-investimento no eixo errado). É isto — e não só "não subir" — que inclina o
        // shape; sem, o time rico repõe tudo e mantém o carro parelho, e o foco nunca emerge.
        if demand_peaked && !relevant && part.level > part_cap(part.part_type) {
            plan.actions.insert(part.part_type, PartAction::Degrade);
            continue;
        }
        if budget >= rc {
            plan.actions.insert(part.part_type, PartAction::Replace);
            budget -= rc;
            plan.estimated_cost += rc;
        } else if relevant && can_stretch(part) && budget >= sc {
            plan.actions.insert(part.part_type, PartAction::Stretch);
            budget -= sc;
            plan.estimated_cost += sc;
        } else {
            plan.actions.insert(part.part_type, PartAction::Degrade);
        }
    }

    // 1b) Passe PROATIVO (redesign 2026-07-22 §4.5): com caixa, o time troca a peça UMA corrida
    //     ANTES do fim da vida — assim ela não entra na próxima corrida perto de 100% nem roda
    //     um trecho em SOBREUSO (onde o risco de quebra dispara). É o que dá CONFIABILIDADE ao
    //     time rico (paga mais trocas por menos quebras); o pobre não tem caixa e deixa a peça ir
    //     ao sobreuso — a consequência. Peça já resolvida no passe 1, esticada, ou irrelevante
    //     acima do teto de foco (que queremos degradar) ficam de fora.
    let mut proactive: Vec<CarPart> = car
        .parts
        .iter()
        .copied()
        .filter(|p| {
            if plan.actions.contains_key(&p.part_type) || p.spent {
                return false;
            }
            if p.level > part_cap(p.part_type) {
                return false; // irrelevante em foco → deixa degradar, não repõe
            }
            // "Precisaria de troca na PRÓXIMA corrida" = desgaste + 2 incrementos ≥ 100%.
            let incr = wear_per_race(p.part_type) / crate::car::wear::part_effective_life(p, apply_tent);
            p.wear + 2.0 * incr >= 1.0
        })
        .collect();
    proactive.sort_by(|a, b| {
        part_relevance(b.part_type, demand)
            .partial_cmp(&part_relevance(a.part_type, demand))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for part in &proactive {
        let rc = replace_cost(category_id, part);
        if budget >= rc {
            plan.actions.insert(part.part_type, PartAction::Replace);
            budget -= rc;
            plan.estimated_cost += rc;
        }
    }

    // 2) Passe de upgrade: com caixa sobrando, sobe as peças rumo ao seu TETO EFETIVO
    //    (relevantes → teto da categoria; irrelevantes → teto de foco). Esticadas ficam fora.
    let mut upgradable: Vec<PartType> = PartType::ALL
        .iter()
        .copied()
        .filter(|&pt| {
            current_target(&plan, car, pt) < part_cap(pt)
                && car.part(pt).map(|p| !p.spent).unwrap_or(true)
        })
        .collect();
    upgradable.sort_by(|a, b| {
        part_relevance(*b, demand)
            .partial_cmp(&part_relevance(*a, demand))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for pt in upgradable {
        let new_level = current_target(&plan, car, pt) + 1;
        let up_cost = part_cost(category_id, pt, new_level);
        if budget >= up_cost {
            plan.target_levels.insert(pt, new_level);
            plan.actions.entry(pt).or_insert(PartAction::Replace);
            budget -= up_cost;
            plan.estimated_cost += up_cost;
        }
    }

    plan
}

/// Nível-alvo atual de uma peça no plano (upgrade pendente, senão o nível instalado).
fn current_target(plan: &CarMaintenancePlan, car: &Car, part: PartType) -> u8 {
    plan.target_levels
        .get(&part)
        .copied()
        .or_else(|| car.part(part).map(|p| p.level))
        .unwrap_or(1)
}

/// Decide a manutenção usando o caixa real do time e as próximas pistas do calendário.
pub fn decide_car_maintenance(
    team: &Team,
    car: &Car,
    category_id: &str,
    upcoming_track_ids: &[u32],
) -> CarMaintenancePlan {
    let budget = calculate_financial_plan(team).spending_power.max(0.0);
    // Demanda efetiva = calendário misturado com o DNA persistente do time. É o que faz o
    // foco EMERGIR: a média do calendário sozinha lava pra balanceado; o DNA sustenta o pico.
    let calendar_demand = maintenance_demand(upcoming_track_ids);
    let demand = blend_with_focus(calendar_demand, team_car_focus(&team.id));
    decide_maintenance(car, category_id, budget, demand)
}

/// Aplica o plano ao carro: instala os upgrades e roda a corrida (desgaste + ações) com
/// desgaste NEUTRO (corrida de referência).
pub fn apply_plan(car: &mut Car, plan: &CarMaintenancePlan) {
    for (&part, &level) in &plan.target_levels {
        car.set_level(part, level);
    }
    advance_race(car, &plan.actions);
}

/// Igual a [`apply_plan`], mas o desgaste desta corrida é escalado por peça pelas condições
/// REAIS (pista × clima via `wear_mults`; peças ausentes no mapa → 1.0). É o que faz o
/// desgaste persistido responder à corrida.
pub fn apply_plan_scaled(
    car: &mut Car,
    plan: &CarMaintenancePlan,
    wear_mults: &std::collections::HashMap<PartType, f64>,
    apply_tent: bool,
    rel_mult: f64,
) {
    for (&part, &level) in &plan.target_levels {
        car.set_level(part, level);
    }
    advance_race_scaled(
        car,
        &plan.actions,
        |pt| wear_mults.get(&pt).copied().unwrap_or(1.0),
        apply_tent,
        rel_mult,
    );
}

// ===================== Seed inicial dos carros =====================

/// Qualidade relativa (0..1) de cada time DENTRO da sua categoria, medida pelo
/// `car_performance` (o escalar legado que já reflete orçamento/prestígio no seed).
fn category_quality(teams: &[Team]) -> HashMap<String, f64> {
    // Min/max de car_performance por categoria.
    let mut bounds: HashMap<&str, (f64, f64)> = HashMap::new();
    for team in teams {
        let entry = bounds
            .entry(team.categoria.as_str())
            .or_insert((f64::INFINITY, f64::NEG_INFINITY));
        entry.0 = entry.0.min(team.car_performance);
        entry.1 = entry.1.max(team.car_performance);
    }

    let mut quality = HashMap::new();
    for team in teams {
        let (min, max) = bounds
            .get(team.categoria.as_str())
            .copied()
            .unwrap_or((0.0, 0.0));
        let spread = max - min;
        let q = if spread.abs() < f64::EPSILON {
            0.5
        } else {
            ((team.car_performance - min) / spread).clamp(0.0, 1.0)
        };
        quality.insert(team.id.clone(), q);
    }
    quality
}

/// Semeia e persiste o carro inicial de cada time (correlacionado com a qualidade na
/// categoria; rookie = spec). Chamado uma vez na criação da carreira, logo após inserir
/// os times.
pub fn seed_and_persist_team_cars(conn: &Connection, teams: &[Team]) -> Result<(), DbError> {
    let quality = category_quality(teams);
    for team in teams {
        let q = quality.get(&team.id).copied().unwrap_or(0.5);
        let car = seed_car(&team.categoria, q);
        team_car::upsert_team_car(conn, &team.id, &car)?;
    }
    Ok(())
}

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
    pub fn from_race(track_id: u32, weather: crate::car::breakdown::Weather, duracao_min: u8) -> Self {
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
) -> Result<f64, DbError> {
    use crate::car::breakdown::Severity;

    // Usa o carro anexado; senão carrega; senão semeia neutro (save antigo/robustez).
    let mut car = match &team.car {
        Some(car) => car.clone(),
        None => team_car::get_team_car(conn, &team.id)?
            .unwrap_or_else(|| seed_car(category_id, 0.5)),
    };

    // Um carro NUNCA pode estar acima do teto da sua categoria — regride ao teto ao entrar
    // nela. Sem isto, um time REBAIXADO carregaria o carro alto da categoria anterior pra
    // sempre (Replace mantém o nível; o teto só bloqueia upgrade, não reposição).
    let ceiling = category_ceiling(category_id);
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

    let mut plan = decide_car_maintenance(team, &car, category_id, window);
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
    let cost = plan.estimated_cost + forced_cost;
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

#[cfg(test)]
mod tests;
