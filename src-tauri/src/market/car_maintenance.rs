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
mod tests {
    use super::*;

    // -------- Horizonte de planejamento --------

    #[test]
    fn horizonte_distribuicao_bate_20_30_30_20() {
        let n = 3000;
        let (mut single, mut three, mut five, mut season) = (0, 0, 0, 0);
        for i in 0..n {
            match planning_horizon(&format!("T{i}"), 1) {
                PlanningHorizon::SingleTrack => single += 1,
                PlanningHorizon::ThreeRaces => three += 1,
                PlanningHorizon::FiveRaces => five += 1,
                PlanningHorizon::FullSeason => season += 1,
            }
        }
        let frac = |x: i32| x as f64 / n as f64;
        assert!((frac(single) - 0.20).abs() < 0.04, "single={}", frac(single));
        assert!((frac(three) - 0.30).abs() < 0.04, "three={}", frac(three));
        assert!((frac(five) - 0.30).abs() < 0.04, "five={}", frac(five));
        assert!((frac(season) - 0.20).abs() < 0.04, "season={}", frac(season));
    }

    #[test]
    fn horizonte_e_deterministico() {
        assert_eq!(planning_horizon("Team-42", 7), planning_horizon("Team-42", 7));
    }

    #[test]
    fn horizonte_re_rola_por_temporada() {
        let n = 500;
        let changed = (0..n)
            .filter(|i| {
                planning_horizon(&format!("T{i}"), 1) != planning_horizon(&format!("T{i}"), 2)
            })
            .count();
        assert!(changed > 200, "esperado muitos times re-rolando; mudaram {changed}");
    }

    // -------- Demanda de manutenção --------

    #[test]
    fn demanda_normaliza_e_le_as_pistas() {
        // Vazio → balanceado.
        let (p, h, a) = maintenance_demand(&[]);
        assert!((p + h + a - 1.0).abs() < 1e-9);
        assert!((p - 1.0 / 3.0).abs() < 1e-9);
        // Monza (239) é power-heavy → P domina.
        let (p2, h2, a2) = maintenance_demand(&[239]);
        assert!((p2 + h2 + a2 - 1.0).abs() < 1e-6);
        assert!(p2 > h2 && p2 > a2, "Monza deveria exigir Power: P={p2} H={h2} A={a2}");
    }

    // -------- Decisão de manutenção --------

    #[test]
    fn prioriza_peca_do_atributo_exigido_com_caixa_curto() {
        let mut car = Car::uniform(5);
        car.set_wear(PartType::Engine, 0.90); // fim de vida
        car.set_wear(PartType::Brakes, 0.90); // fim de vida
        let demand = (1.0, 0.0, 0.0); // power puro
        let budget = replace_cost("gt4", car.part(PartType::Engine).unwrap());

        let plan = decide_maintenance(&car, "gt4", budget, demand);

        // O motor (relevante em power) leva a única troca possível...
        assert_eq!(plan.actions.get(&PartType::Engine), Some(&PartAction::Replace));
        // ...e os freios (H puro, irrelevantes aqui, sem caixa) degradam.
        assert_eq!(plan.actions.get(&PartType::Brakes), Some(&PartAction::Degrade));
    }

    #[test]
    fn estica_quando_sem_caixa_mas_a_pista_exige() {
        let mut car = Car::uniform(5);
        car.set_wear(PartType::Engine, 0.90);
        let demand = (1.0, 0.0, 0.0);
        // Caixa só dá para esticar (40% de uma nova), não para trocar.
        let sc = stretch_cost("gt4", car.part(PartType::Engine).unwrap());

        let plan = decide_maintenance(&car, "gt4", sc, demand);

        assert_eq!(plan.actions.get(&PartType::Engine), Some(&PartAction::Stretch));
    }

    #[test]
    fn degrada_peca_irrelevante_para_a_proxima_pista() {
        let mut car = Car::uniform(5);
        car.set_wear(PartType::Brakes, 0.90);
        let demand = (1.0, 0.0, 0.0); // power; freios (H puro) irrelevantes

        let plan = decide_maintenance(&car, "gt4", 0.0, demand);

        assert_eq!(plan.actions.get(&PartType::Brakes), Some(&PartAction::Degrade));
    }

    // -------- Seed inicial --------

    #[test]
    fn seed_persiste_carros_correlacionados_com_a_categoria() {
        use crate::models::team::placeholder_team_from_db;
        let conn = Connection::open_in_memory().unwrap();
        let mk = |id: &str, cat: &str, cp: f64| {
            let mut t = placeholder_team_from_db(
                id.to_string(),
                format!("Team {id}"),
                cat.to_string(),
                "2026-01-01T00:00:00".to_string(),
            );
            t.car_performance = cp;
            t
        };
        let teams = vec![
            mk("A", "gt3", 15.0),          // topo do GT3
            mk("B", "gt3", 5.0),           // fundo do GT3
            mk("R", "mazda_rookie", 2.0),  // spec
        ];

        seed_and_persist_team_cars(&conn, &teams).unwrap();

        let a = team_car::get_team_car(&conn, "A").unwrap().unwrap();
        let b = team_car::get_team_car(&conn, "B").unwrap().unwrap();
        let r = team_car::get_team_car(&conn, "R").unwrap().unwrap();
        assert!(
            a.display_level() > b.display_level(),
            "topo do GT3 ({}) deveria ter carro melhor que o fundo ({})",
            a.display_level(),
            b.display_level()
        );
        assert!(a.display_level() <= 7, "respeita o teto do GT3");
        assert_eq!(r.display_level(), 1, "rookie é spec");
    }

    /// Integração de ponta a ponta PELO BANCO: seed → carrega → cérebro decide →
    /// desgaste → persiste, por uma temporada. Todos começam iguais (nível 5); só o
    /// ORÇAMENTO os separa. Demonstra o spread da grade emergindo do dinheiro — a
    /// promessa central do sistema. Exercita seed + wear + brain + team_car juntos.
    #[test]
    fn integracao_temporada_o_orcamento_abre_o_spread_da_grade() {
        let conn = Connection::open_in_memory().unwrap();
        let rich = ["R1", "R2", "R3"];
        let poor = ["P1", "P2", "P3"];

        // Todos nascem no mesmo carro (gt3, qualidade média → nível 5).
        for id in rich.iter().chain(poor.iter()) {
            team_car::upsert_team_car(&conn, id, &seed_car("gt3", 0.5)).unwrap();
        }
        // Sanidade: largaram iguais.
        for id in rich.iter().chain(poor.iter()) {
            assert_eq!(team_car::get_team_car(&conn, id).unwrap().unwrap().display_level(), 5);
        }

        // Calendário misto de uma temporada: power → handling → accel, repetindo.
        let calendario = [(0.70, 0.20, 0.10), (0.15, 0.70, 0.15), (0.20, 0.15, 0.65)];

        for corrida in 0..24 {
            let demand = calendario[corrida % calendario.len()];
            for (&id, budget) in rich.iter().zip(std::iter::repeat(1e12)) {
                let mut car = team_car::get_team_car(&conn, id).unwrap().unwrap();
                let plan = decide_maintenance(&car, "gt3", budget, demand);
                apply_plan(&mut car, &plan);
                team_car::upsert_team_car(&conn, id, &car).unwrap();
            }
            for (&id, budget) in poor.iter().zip(std::iter::repeat(0.0)) {
                let mut car = team_car::get_team_car(&conn, id).unwrap().unwrap();
                let plan = decide_maintenance(&car, "gt3", budget, demand);
                apply_plan(&mut car, &plan);
                team_car::upsert_team_car(&conn, id, &car).unwrap();
            }
        }

        let nivel = |id: &str| team_car::get_team_car(&conn, id).unwrap().unwrap().display_level();
        let ricos: Vec<u8> = rich.iter().map(|id| nivel(id)).collect();
        let pobres: Vec<u8> = poor.iter().map(|id| nivel(id)).collect();

        // Os ricos sobem rumo ao teto (7); os pobres sangram; todos respeitam o teto.
        assert!(ricos.iter().all(|&l| l >= 6), "ricos deveriam chegar perto do teto: {ricos:?}");
        assert!(pobres.iter().all(|&l| l <= 3), "pobres deveriam sangrar: {pobres:?}");
        assert!(ricos.iter().all(|&l| l <= 7), "ninguém passa do teto do GT3: {ricos:?}");
        let melhor_pobre = *pobres.iter().max().unwrap();
        let pior_rico = *ricos.iter().min().unwrap();
        assert!(pior_rico > melhor_pobre + 2, "o spread deveria ser nítido: ricos={ricos:?} pobres={pobres:?}");
    }

    #[test]
    fn tick_por_rodada_evolui_e_persiste_o_carro_pelo_caixa() {
        use crate::models::team::placeholder_team_from_db;
        let conn = Connection::open_in_memory().unwrap();
        let mut team = placeholder_team_from_db(
            "T1".to_string(),
            "Team 1".to_string(),
            "gt3".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.cash_balance = 1e12;
        team.debt_balance = 0.0;
        team.financial_state = "healthy".to_string();

        // Carro inicial no piso do GT3 (nível 3), persistido e anexado.
        let car = seed_car("gt3", 0.0);
        let start = car.display_level();
        team_car::upsert_team_car(&conn, "T1", &car).unwrap();
        team.car = Some(car);

        // Várias rodadas com caixa alto → o carro sobe rumo ao teto, e cada rodada tem custo.
        for _ in 0..20 {
            let cost =
                maintain_team_car(&conn, &team, "gt3", 1, &[239, 489, 324], WearConditions::neutral(), None)
                    .unwrap();
            assert!(cost >= 0.0);
            team.car = team_car::get_team_car(&conn, "T1").unwrap();
        }

        let end = team.car.as_ref().unwrap().display_level();
        assert!(end > start, "carro de time rico deveria melhorar (start={start}, end={end})");
    }

    #[test]
    fn carro_acima_do_teto_regride_ao_entrar_na_categoria() {
        use crate::models::team::placeholder_team_from_db;
        let conn = Connection::open_in_memory().unwrap();
        let mut team = placeholder_team_from_db(
            "T1".to_string(),
            "Rebaixado".to_string(),
            "mazda_amador".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        // Carro alto (nível 6), como se o time tivesse caído de uma categoria superior.
        let high = Car::uniform(6);
        team_car::upsert_team_car(&conn, "T1", &high).unwrap();
        team.car = Some(high);

        // Amador tem teto 2 → o carro deve regredir a ≤ 2.
        maintain_team_car(&conn, &team, "mazda_amador", 1, &[], WearConditions::neutral(), None).unwrap();

        let after = team_car::get_team_car(&conn, "T1").unwrap().unwrap();
        assert!(
            after.display_level() <= 2,
            "carro deveria regredir ao teto do amador, ficou {}",
            after.display_level()
        );
    }

    #[test]
    fn calendario_peaked_faz_o_carro_especializar() {
        // Demanda (P, H, A) fortemente de POWER → o carro foca em power.
        let power_demand = (0.70, 0.15, 0.15);
        let mut car = Car::uniform(1);
        for _ in 0..30 {
            let plan = decide_maintenance(&car, "gt3", 1e12, power_demand);
            apply_plan(&mut car, &plan);
        }
        let engine = car.part(PartType::Engine).unwrap().level; // relevante (power)
        let brakes = car.part(PartType::Brakes).unwrap().level; // irrelevante (H puro)
        assert!(
            engine > brakes + 1,
            "carro deveria focar em power (motor {engine} vs freios {brakes})"
        );
        let (p, h, _a) = car.pha();
        assert!(p > h, "o shape deveria pesar power: P={p:.1} H={h:.1}");
    }

    // -------- DNA / identidade de carro do time --------

    #[test]
    fn dna_e_estavel_e_nao_depende_de_temporada() {
        // Determinístico e permanente (a assinatura nem recebe temporada).
        assert_eq!(team_car_focus("Team-42"), team_car_focus("Team-42"));
    }

    #[test]
    fn dna_distribuicao_40_20_20_20() {
        let n = 3000;
        let (mut b, mut p, mut h, mut a) = (0, 0, 0, 0);
        for i in 0..n {
            match team_car_focus(&format!("T{i}")) {
                CarFocus::Balanced => b += 1,
                CarFocus::Power => p += 1,
                CarFocus::Handling => h += 1,
                CarFocus::Acceleration => a += 1,
            }
        }
        let frac = |x: i32| x as f64 / n as f64;
        assert!((frac(b) - 0.40).abs() < 0.05, "balanced={}", frac(b));
        assert!((frac(p) - 0.20).abs() < 0.05, "power={}", frac(p));
        assert!((frac(h) - 0.20).abs() < 0.05, "handling={}", frac(h));
        assert!((frac(a) - 0.20).abs() < 0.05, "accel={}", frac(a));
    }

    #[test]
    fn dna_pica_a_demanda_que_o_calendario_diverso_lavaria() {
        // Calendário diverso (2 P + 1 H + 1 A) → média ~balanceada, spread abaixo do gatilho.
        let diverse = maintenance_demand(&[239, 523, 489, 179]); // Monza+Spa(P) Ledenon(H) LongBeach(A)
        assert!(
            demand_spread(diverse) < DEMAND_PEAK_THRESHOLD,
            "calendário diverso deveria lavar pra balanceado: spread={}",
            demand_spread(diverse)
        );
        // DNA de potência empurra a demanda efetiva acima do gatilho.
        let blended = blend_with_focus(diverse, CarFocus::Power);
        let (p, h, a) = blended;
        assert!(p > h && p > a, "DNA deveria puxar power: P={p:.2} H={h:.2} A={a:.2}");
        assert!(
            demand_spread(blended) >= DEMAND_PEAK_THRESHOLD,
            "DNA deveria peakar a demanda: spread={}",
            demand_spread(blended)
        );
        // DNA balanceado NÃO peaka (generalista continua generalista).
        assert!(
            demand_spread(blend_with_focus(diverse, CarFocus::Balanced)) < DEMAND_PEAK_THRESHOLD
        );
    }

    #[test]
    fn time_com_dna_de_potencia_foca_mesmo_em_calendario_diverso() {
        use crate::models::team::placeholder_team_from_db;
        // Acha um id cujo DNA é potência.
        let team_id = (0..10_000)
            .map(|i| format!("P{i}"))
            .find(|id| team_car_focus(id) == CarFocus::Power)
            .expect("deveria existir id com DNA de potência");

        let conn = Connection::open_in_memory().unwrap();
        let mut team = placeholder_team_from_db(
            team_id.clone(),
            "Power DNA".to_string(),
            "gt3".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.cash_balance = 1e12;
        team.debt_balance = 0.0;
        team.financial_state = "healthy".to_string();
        let car = seed_car("gt3", 0.5);
        team_car::upsert_team_car(&conn, &team_id, &car).unwrap();
        team.car = Some(car);

        // Calendário DIVERSO cuja média até pende de leve pra HANDLING (contra o DNA):
        // se mesmo assim o carro pesa power, é o DNA sustentando o foco, não o calendário.
        let diverse = [489, 325, 180, 318, 93, 188];
        for season in 1..=4 {
            for _ in 0..15 {
                maintain_team_car(&conn, &team, "gt3", season, &diverse, WearConditions::neutral(), None)
                    .unwrap();
                team.car = team_car::get_team_car(&conn, &team_id).unwrap();
            }
        }

        let car = team.car.as_ref().unwrap();
        let (p, h, a) = car.pha();
        assert!(
            p > h && p > a,
            "carro de DNA-power deveria pesar power num calendário diverso: P={p:.1} H={h:.1} A={a:.1}"
        );
        let engine = car.part(PartType::Engine).unwrap().level;
        let brakes = car.part(PartType::Brakes).unwrap().level;
        assert!(
            engine > brakes,
            "peça de power (motor {engine}) deveria superar a de handling (freios {brakes})"
        );
    }

    // -------- Feedback físico da quebra (§4.6) --------

    /// Roda a manutenção de UM carro com um desgaste inicial no motor e uma lista de quebras
    /// desta corrida; devolve `(custo, peça-motor persistida)`.
    fn maintain_com_quebra(
        cash: f64,
        debt: f64,
        state: &str,
        engine_wear: f64,
        events: &[(PartType, crate::car::breakdown::Severity)],
    ) -> (f64, CarPart) {
        // Time de DNA BALANCEADO (sem foco) → o cérebro não de-investe nenhuma peça: uma peça no
        // fim de vida é trocada (com caixa) em vez de degradada. Isola o efeito do feedback.
        let team_id = (0..2000)
            .map(|i| format!("T{i}"))
            .find(|id| team_car_focus(id) == CarFocus::Balanced)
            .unwrap();
        maintain_com_quebra_team(&team_id, cash, debt, state, engine_wear, events)
    }

    fn maintain_com_quebra_team(
        team_id: &str,
        cash: f64,
        debt: f64,
        state: &str,
        engine_wear: f64,
        events: &[(PartType, crate::car::breakdown::Severity)],
    ) -> (f64, CarPart) {
        use crate::models::team::placeholder_team_from_db;
        let conn = Connection::open_in_memory().unwrap();
        let mut team = placeholder_team_from_db(
            team_id.to_string(),
            team_id.to_string(),
            "gt3".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.cash_balance = cash;
        team.debt_balance = debt;
        team.financial_state = state.to_string();
        let mut car = Car::uniform(5);
        car.set_wear(PartType::Engine, engine_wear);
        team_car::upsert_team_car(&conn, team_id, &car).unwrap();
        team.car = Some(car);
        let cost = maintain_team_car_pits(
            &conn,
            &team,
            "gt3",
            1,
            &[],
            WearConditions::neutral(),
            None,
            false,
            0,
            events,
        )
        .unwrap();
        let after = team_car::get_team_car(&conn, team_id).unwrap().unwrap();
        let engine = *after.part(PartType::Engine).unwrap();
        (cost, engine)
    }

    #[test]
    fn dnf_destroi_e_repoe_a_peca_mesmo_sem_caixa() {
        use crate::car::breakdown::Severity;
        // Time POBRE (sem caixa), motor a MEIA-VIDA (não estava no fim). DNF destrói → troca
        // FORÇADA a débito: a peça vira NOVA (não fica presa em sobreuso) e há custo cobrado.
        let (cost, engine) =
            maintain_com_quebra(0.0, 1e9, "critical", 0.30, &[(PartType::Engine, Severity::Dnf)]);
        assert!(engine.wear < 0.5, "motor destruído deveria virar NOVO (wear baixo), deu {}", engine.wear);
        assert!(cost > 0.0, "a troca forçada do DNF deveria cobrar custo (a débito), deu {cost}");
    }

    #[test]
    fn sem_feedback_a_peca_do_pobre_so_acumula() {
        // Contraste: MESMO cenário, sem evento → o motor a 0.30 num time pobre só acumula, NÃO
        // vira novo. Prova que é o FEEDBACK (não o cérebro) que reseta a peça no DNF.
        let (_c, engine) = maintain_com_quebra(0.0, 1e9, "critical", 0.30, &[]);
        assert!(engine.wear > 0.4, "sem quebra, o motor só acumula (não reseta): {}", engine.wear);
    }

    #[test]
    fn leve_nao_altera_a_peca() {
        use crate::car::breakdown::Severity;
        // Leve = mesmo desfecho que SEM quebra (a peça só perdeu rendimento na corrida).
        let (_c1, com) =
            maintain_com_quebra(0.0, 1e9, "critical", 0.30, &[(PartType::Engine, Severity::Light)]);
        let (_c2, sem) = maintain_com_quebra(0.0, 1e9, "critical", 0.30, &[]);
        assert!(
            (com.wear - sem.wear).abs() < 1e-9,
            "Leve não deveria mudar a peça (com {} vs sem {})",
            com.wear,
            sem.wear
        );
    }

    #[test]
    fn grave_forca_troca_ate_sem_caixa() {
        use crate::car::breakdown::Severity;
        // GRAVE também força a troca (variante simples) — inclusive no time POBRE, a débito: a
        // peça que custou tempo vira NOVA e não requebra; o buraco é financeiro (custo cobrado).
        let (cost, engine) =
            maintain_com_quebra(0.0, 1e9, "critical", 0.30, &[(PartType::Engine, Severity::Heavy)]);
        assert!(engine.wear < 0.5, "Grave deveria trocar a peça (nova) mesmo sem caixa: {}", engine.wear);
        assert!(cost > 0.0, "a troca forçada do Grave deveria cobrar custo (a débito): {cost}");
    }

    // -------- Economia do enduro (custo por duração + alívio de parada) --------

    /// Um time pobre (sem caixa pra trocar) só ACUMULA desgaste. No enduro (60 min) o desgaste
    /// persistido é bem maior que num sprint (30 min); paradas reais aliviam, mas o enduro ainda
    /// custa mais. É a conta do enduro fluindo pela economia calibrada, atrás do gate de 40 min.
    #[test]
    fn enduro_desgasta_mais_o_carro_e_a_parada_alivia() {
        use crate::models::team::placeholder_team_from_db;
        let total_wear = |duracao_min: u8, pits: u32| -> f64 {
            let conn = Connection::open_in_memory().unwrap();
            let mut team = placeholder_team_from_db(
                "T".to_string(),
                "T".to_string(),
                "gt3".to_string(),
                "2026-01-01T00:00:00".to_string(),
            );
            team.cash_balance = 0.0; // sem caixa → não troca, só acumula desgaste
            team.debt_balance = 1e9;
            team.financial_state = "critical".to_string();
            let car = Car::uniform(5);
            team_car::upsert_team_car(&conn, "T", &car).unwrap();
            team.car = Some(car);
            let cond = WearConditions {
                track_pha: (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0),
                weather: crate::car::breakdown::Weather::NEUTRAL,
                duracao_min,
            };
            // Carro do JOGADOR (Some) com paradas reais; estilo neutro (fator 1.0).
            maintain_team_car_pits(
                &conn, &team, "gt3", 1, &[], cond,
                Some(crate::car::driving_style::StyleFactors::uniform(1.0)), true, pits, &[],
            )
            .unwrap();
            let after = team_car::get_team_car(&conn, "T").unwrap().unwrap();
            after.parts.iter().map(|p| p.wear).sum()
        };
        let sprint = total_wear(30, 0);
        let enduro = total_wear(60, 0);
        let enduro_pit = total_wear(60, 3); // teto de alívio (−30% do sobrecusto)
        assert!(enduro > sprint * 1.5, "enduro deveria desgastar bem mais (sprint={sprint:.4} enduro={enduro:.4})");
        assert!(enduro_pit < enduro, "paradas deveriam aliviar o enduro ({enduro_pit:.4} < {enduro:.4})");
        assert!(enduro_pit > sprint, "mesmo com paradas o enduro custa mais que o sprint");
    }

    /// A IA (player_style = None) modela as paradas pela duração — recebe o alívio SOZINHA, sem
    /// receber contagem de pit. Enduro da IA custa mais que sprint, mas menos que enduro sem alívio.
    #[test]
    fn ia_recebe_alivio_modelado_no_enduro() {
        use crate::models::team::placeholder_team_from_db;
        let ai_wear = |duracao_min: u8| -> f64 {
            let conn = Connection::open_in_memory().unwrap();
            let mut team = placeholder_team_from_db(
                "T".to_string(), "T".to_string(), "gt3".to_string(), "2026-01-01T00:00:00".to_string(),
            );
            team.cash_balance = 0.0;
            team.debt_balance = 1e9;
            team.financial_state = "critical".to_string();
            let car = Car::uniform(5);
            team_car::upsert_team_car(&conn, "T", &car).unwrap();
            team.car = Some(car);
            let cond = WearConditions {
                track_pha: (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0),
                weather: crate::car::breakdown::Weather::NEUTRAL,
                duracao_min,
            };
            maintain_team_car(&conn, &team, "gt3", 1, &[], cond, None).unwrap();
            team_car::get_team_car(&conn, "T").unwrap().unwrap().parts.iter().map(|p| p.wear).sum()
        };
        let sprint = ai_wear(30);
        let enduro = ai_wear(60); // 2 paradas modeladas → −20% do sobrecusto
        assert!(enduro > sprint, "enduro da IA deveria custar mais ({enduro:.4} > {sprint:.4})");
        // Sobrecusto 60min = 1.0; com 2 paradas modeladas (−20%) → mult 1.8 → 1.8× o sprint.
        assert!((enduro / sprint - 1.8).abs() < 0.02, "IA 60min deveria ser ~1.8× o sprint, deu {:.3}", enduro / sprint);
    }

    #[test]
    fn calendario_equilibrado_mantem_carro_parelho() {
        let balanced = (0.34, 0.33, 0.33);
        let mut car = Car::uniform(1);
        for _ in 0..30 {
            let plan = decide_maintenance(&car, "gt3", 1e12, balanced);
            apply_plan(&mut car, &plan);
        }
        let levels: Vec<u8> = car.parts.iter().map(|p| p.level).collect();
        let max = *levels.iter().max().unwrap();
        let min = *levels.iter().min().unwrap();
        assert!(max - min <= 1, "equilibrado deveria manter o carro parelho: {levels:?}");
    }

    #[test]
    fn time_rico_atinge_o_teto_e_pobre_sangra() {
        // Calendário equilibrado → sem especialização; testa a magnitude/nível puro.
        let demand = (0.34, 0.33, 0.33);

        let mut rich = Car::uniform(3);
        for _ in 0..25 {
            let plan = decide_maintenance(&rich, "gt3", 1e12, demand);
            apply_plan(&mut rich, &plan);
        }
        assert!(
            rich.display_level() >= 6,
            "time rico deveria chegar perto do teto 7, ficou {}",
            rich.display_level()
        );

        let mut poor = Car::uniform(3);
        for _ in 0..25 {
            let plan = decide_maintenance(&poor, "gt3", 0.0, demand);
            apply_plan(&mut poor, &plan);
        }
        assert!(
            poor.display_level() < 3,
            "time pobre deveria sangrar abaixo de 3, ficou {}",
            poor.display_level()
        );
    }

    // -------- Recorrência da quebra ENTRE corridas (Pergunta 2) --------

    /// DIAGNÓSTICO — Pergunta 2 (rode com:
    /// `cargo test analise_recorrencia_entre_corridas -- --ignored --nocapture`).
    ///
    /// "Quando uma peça quebra, na PRÓXIMA corrida quebra de novo com a MESMA peça?"
    ///
    /// FATO ARQUITETURAL que o teste torna visível: a quebra AO VIVO e a persistência do
    /// desgaste são DESACOPLADAS. O pré-roll de quebra lê o desgaste de ENTRADA (persistido) e,
    /// quando uma peça larga, zera o desgaste dela só NA SIMULAÇÃO (é descartado). O desgaste
    /// que fica no save é avançado SÓ pelo cérebro de manutenção (`maintain_team_car` →
    /// `advance_race`). Logo, "quebrar de novo" NÃO é causado pela quebra — é decidido pelo
    /// ORÇAMENTO: time rico repõe a peça (desgaste zera → não repete); time pobre só degrada
    /// (o desgaste passa da parede e a peça FORÇA falha toda corrida).
    ///
    /// Roda o pipeline REAL por temporadas, para muitos times independentes de cada tier, e mede
    /// a recorrência da MESMA peça em corridas consecutivas vs a taxa-base por peça.
    #[test]
    #[ignore]
    fn analise_recorrencia_entre_corridas() {
        use crate::car::breakdown::{roll_race_breakdowns, Weather};
        use crate::car::PartType;
        use crate::models::team::placeholder_team_from_db;
        use std::collections::HashSet;

        const TIMES: usize = 600;
        const CORRIDAS: usize = 16;
        const CAT: &str = "gt3";
        let track_pha = (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0); // pista equilibrada → mults ~1.0
        let weather = Weather::NEUTRAL;

        // Um tier = (nome, caixa, dívida, estado financeiro).
        // NB: o comportamento é um PENHASCO, não uma rampa. Caixas de 5e4 a 4e5 (times
        // placeholder SEM receita) deram todos idênticos ao POBRE — um carro GT3 completo custa
        // mais que isso pra repor, então só o topo escapa. Times reais têm receita recorrente e
        // caem entre RICO e POBRE; aqui o sinal honesto é o CONTRASTE rico↔degrada.
        let tiers: [(&str, f64, f64, &str); 3] = [
            ("RICO   (repõe tudo)", 1e12, 0.0, "healthy"),
            ("MÉDIO  (caixa 1.5e5)", 1.5e5, 0.0, "healthy"),
            ("POBRE  (só degrada)", 0.0, 1e9, "critical"),
        ];

        println!("\n================ RECORRÊNCIA DA MESMA PEÇA ENTRE CORRIDAS ================");
        println!(
            "  {} times × {} corridas cada, por tier. Pré-roll de quebra sobre o desgaste",
            TIMES, CORRIDAS
        );
        println!("  persistido; entre corridas o cérebro de manutenção avança/persiste o desgaste.\n");
        println!(
            "  {:<22} {:>10} {:>12} {:>9} {:>11} {:>10}",
            "tier", "base/peça", "recorrência", "razão", "DNF→carro", "forçada"
        );
        println!(
            "  {:<22} {:>10} {:>12} {:>9} {:>11} {:>10}",
            "", "P(quebra)", "P(mesma|N)", "rec/base", "some grid", "(parede)"
        );

        for (nome, cash, debt, estado) in tiers {
            let conn = Connection::open_in_memory().unwrap();

            // Contadores agregados.
            let mut breaks_partlevel = 0u64; // total de eventos (peça-nível) somados
            let mut race_slots = 0u64; // corridas × 11 peças (denominador da base)
            let mut prev_pairs = 0u64; // peças que quebraram numa corrida COM próxima corrida
            let mut recurred = 0u64; // ...dessas, quantas quebraram DE NOVO na seguinte
            let mut dnf_races = 0u64; // corridas em que o carro saiu (DNF)
            let mut total_races = 0u64;
            let mut forced_events = 0u64; // eventos por PAREDE (falha forçada, >HARD_WALL)
            let mut all_events = 0u64; // total de eventos (pra a fração forçada)

            for t in 0..TIMES {
                let team_id = format!("{}-{t}", nome.trim());
                let mut team = placeholder_team_from_db(
                    team_id.clone(),
                    team_id.clone(),
                    CAT.to_string(),
                    "2026-01-01T00:00:00".to_string(),
                );
                team.cash_balance = cash;
                team.debt_balance = debt;
                team.financial_state = estado.to_string();

                // Carro inicial: qualidade correlacionada ao tier (rico começa melhor), mas a
                // dinâmica de recorrência vem da manutenção corrida a corrida, não do seed.
                let q = if cash > 1e6 { 0.7 } else if cash > 0.0 { 0.5 } else { 0.35 };
                let car = seed_car(CAT, q);
                team_car::upsert_team_car(&conn, &team_id, &car).unwrap();
                team.car = Some(car);

                let mut prev: Option<HashSet<PartType>> = None;

                for r in 0..CORRIDAS {
                    let car = team_car::get_team_car(&conn, &team_id).unwrap().unwrap();

                    // Semente única por (time, corrida) — como o disparo ao vivo do jogo (1 sorte).
                    let mut seed = 0xC0FF_EE00_u64 ^ (r as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                    for b in team_id.bytes() {
                        seed = seed.wrapping_mul(0x0000_0100_0000_01B3).wrapping_add(b as u64);
                    }
                    let evs = roll_race_breakdowns(&car, 18, seed, 50.0, track_pha, weather, &[]);

                    let cur: HashSet<PartType> = evs.iter().map(|e| e.part).collect();
                    total_races += 1;
                    race_slots += 11;
                    breaks_partlevel += cur.len() as u64;
                    all_events += evs.len() as u64;
                    forced_events += evs.iter().filter(|e| e.forced).count() as u64;
                    if evs.iter().any(|e| e.is_dnf()) {
                        dnf_races += 1;
                    }

                    if let Some(prev_set) = &prev {
                        for &p in prev_set {
                            prev_pairs += 1;
                            if cur.contains(&p) {
                                recurred += 1;
                            }
                        }
                    }
                    prev = Some(cur);

                    // FASE 5 — feedback físico: as peças que largaram viram consequência no save
                    // (Leve segue; Grave→fim de vida; DNF→destruída/troca forçada). É o que corta
                    // a recorrência (peça quebrada vira nova) e o runaway do pobre (vira dívida).
                    let events: Vec<(PartType, crate::car::breakdown::Severity)> =
                        evs.iter().map(|e| (e.part, e.severity)).collect();
                    // Entre corridas: o cérebro de manutenção avança/persiste o desgaste (neutro).
                    maintain_team_car_pits(
                        &conn,
                        &team,
                        CAT,
                        1,
                        &[],
                        WearConditions::neutral(),
                        None,
                        false,
                        0,
                        &events,
                    )
                    .unwrap();
                    team.car = team_car::get_team_car(&conn, &team_id).unwrap();
                }
            }

            let base = breaks_partlevel as f64 / race_slots as f64;
            let recor = if prev_pairs > 0 {
                recurred as f64 / prev_pairs as f64
            } else {
                0.0
            };
            let razao = if base > 1e-9 { recor / base } else { 0.0 };
            let dnf = dnf_races as f64 / total_races as f64;
            let forced = if all_events > 0 {
                forced_events as f64 / all_events as f64
            } else {
                0.0
            };
            let recor_str = if prev_pairs > 0 {
                format!("{:>10.1}%", recor * 100.0)
            } else {
                "     —    ".to_string()
            };
            println!(
                "  {:<22} {:>9.1}% {} {:>7.1}× {:>10.1}% {:>9.1}%",
                nome,
                base * 100.0,
                recor_str,
                razao,
                dnf * 100.0,
                forced * 100.0,
            );
        }
        println!(
            "\n  Leitura: 'base/peça' = chance de UMA peça qualquer quebrar numa corrida."
        );
        println!(
            "  'recorrência' = dado que a peça quebrou, chance de a MESMA quebrar na próxima."
        );
        println!(
            "  'razão' ≫ 1 = a quebra é PEGAJOSA (a mesma peça repete muito acima do acaso).\n"
        );
    }

    // -------- As 11 peças desgastam de forma diferente? (staggering) --------

    /// DIAGNÓSTICO (rode com:
    /// `cargo test analise_desgaste_por_peca -- --ignored --nocapture`).
    ///
    /// "As 11 peças deveriam desgastar de forma diferente." Este teste TORNA VISÍVEL o
    /// desgaste PERSISTIDO peça a peça, corrida a corrida, num carro rico no teto (que só cicla
    /// por fim-de-vida). Imprime o wear de cada peça e marca `*` quando entra na zona de risco
    /// (≥ 87%, quebraria na próxima). Compara calendário NEUTRO vs VARIADO.
    ///
    /// O que ele expõe: no persistido NÃO há ruído (só `wear_per_race × pista × clima`); todas
    /// largam em wear 0 iguais. Logo peças de MESMA durabilidade (há 6 de durab 3!) só se
    /// separam pela pista/clima. Num calendário neutro elas marcham em LOCKSTEP e chegam ao
    /// fim-de-vida JUNTAS — a origem do "várias peças quebram na mesma corrida".
    #[test]
    #[ignore]
    fn analise_desgaste_por_peca() {
        // Abreviações de 3 letras, na ordem de PartType::ALL.
        let abbr = |pt: PartType| match pt {
            PartType::Chassis => "Cha",
            PartType::Engine => "Eng",
            PartType::FrontWing => "AsD",
            PartType::RearWing => "AsT",
            PartType::Underbody => "Ass",
            PartType::Sidepods => "Sid",
            PartType::Cooling => "Arr",
            PartType::Gearbox => "Cbx",
            PartType::Brakes => "Fre",
            PartType::Suspension => "Sus",
            PartType::Electronics => "Ele",
        };
        const RISK_OPEN: f64 = 0.87; // espelha breakdown::RISK_OPEN

        // Calendário neutro (tudo equilibrado) vs variado (potência→handling→aceleração).
        let neutro = [(1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0)];
        let variado = [(0.70, 0.15, 0.15), (0.15, 0.70, 0.15), (0.20, 0.15, 0.65)];
        let quente = crate::car::breakdown::Weather {
            wetness: 0.0,
            temperature: 32.0,
            humidity: 80.0,
            wind_kmh: 30.0,
        };

        for (nome, calendario, usar_clima) in [
            ("NEUTRO  (pista equilibrada, clima neutro)", &neutro[..], false),
            ("VARIADO (P→H→A rotando, +1 dia quente)", &variado[..], true),
        ] {
            println!("\n================ DESGASTE PERSISTIDO POR PEÇA — {nome} ================");
            // Cabeçalho: durabilidade de cada peça (o diferenciador principal).
            print!("  {:>7}", "durab:");
            for &pt in &PartType::ALL {
                print!(" {:>3}", pt.durability());
            }
            println!();
            print!("  {:>7}", "corrida");
            for &pt in &PartType::ALL {
                print!(" {:>3}", abbr(pt));
            }
            println!("     (peças na zona de risco ≥87%)");

            let mut car = Car::uniform(7); // GT3 no teto → só cicla por fim-de-vida
            for r in 0..14 {
                let track = calendario[r % calendario.len()];
                let demand = track;
                // Clima quente numa corrida a cada 3 (só no cenário variado).
                let weather = if usar_clima && r % 3 == 2 {
                    quente
                } else {
                    crate::car::breakdown::Weather::NEUTRAL
                };

                // Estado PERSISTIDO no INÍCIO desta corrida = o que o pré-roll de quebra leria.
                // "Em risco" = a peça CRUZA a zona (≥87%) DURANTE esta corrida (entrada +
                // desgaste da corrida), não só se já entrou acima de 87%.
                let cruza_zona =
                    |pt: PartType, w: f64| w + wear_per_race(pt) >= RISK_OPEN;
                let em_risco: Vec<&str> = PartType::ALL
                    .iter()
                    .filter(|&&pt| {
                        car.part(pt).map(|p| cruza_zona(pt, p.wear)).unwrap_or(false)
                    })
                    .map(|&pt| abbr(pt))
                    .collect();
                print!("  {:>7}", format!("→{}", r + 1));
                for &pt in &PartType::ALL {
                    let w = car.part(pt).map(|p| p.wear).unwrap_or(0.0);
                    let mark = if cruza_zona(pt, w) { "*" } else { " " };
                    print!(" {:>2.0}{}", w * 100.0, mark);
                }
                if em_risco.is_empty() {
                    println!("   —");
                } else {
                    println!("   {} ({})", em_risco.join(","), em_risco.len());
                }

                // Avança a corrida (cérebro rico repõe no fim-de-vida; clima/pista modulam).
                let plan = decide_maintenance(&car, "gt3", 1e12, demand);
                let wear_mults =
                    crate::car::breakdown::conditions_wear_mults(track, weather);
                apply_plan_scaled(&mut car, &plan, &wear_mults, true, 1.0);
            }
        }
        println!(
            "\n  Leitura: peças de MESMA durabilidade e MESMO perfil (ex.: AsD/AsT, ambas durab 3)"
        );
        println!(
            "  entram na zona (*) JUNTAS no neutro. A pista/clima é o ÚNICO desempate — sem ela,"
        );
        println!("  o desgaste persistido não diferencia peças de mesma durabilidade.\n");
    }
}
