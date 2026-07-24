//! O plano de manutenção de UMA corrida: demanda PHA das próximas pistas,
//! relevância das peças e a decisão de trocar / esticar / degradar / subir de
//! nível dentro do caixa.

use super::*;

/// Uma peça é considerada "relevante" para a demanda se contribui com ao menos esta
/// fração do máximo que qualquer peça oferece para aquela demanda.
pub(super) const RELEVANCE_FRACTION: f64 = 0.4;

/// Quão "peaked" a demanda do calendário precisa ser para o time ESPECIALIZAR (spread
/// máx−mín da demanda normalizada). Abaixo disso, sobe tudo (carro balanceado).
pub(super) const DEMAND_PEAK_THRESHOLD: f64 = 0.15;

/// Quanto o teto das peças IRRELEVANTES fica abaixo do teto da categoria quando o time
/// especializa — é o que cria o FOCO. Calibrável (chunk 8).
pub(super) const FOCUS_GAP: u8 = 3;

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
pub(super) fn demand_spread(demand: (f64, f64, f64)) -> f64 {
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
pub(super) fn part_relevance(part: PartType, demand: (f64, f64, f64)) -> f64 {
    let (pp, ph, pa) = part.pha_per_level();
    let (dp, dh, da) = demand;
    let third = 1.0 / 3.0;
    pp * (dp - third) + ph * (dh - third) + pa * (da - third)
}

/// Uma peça precisa de decisão nesta corrida? (esgotada, ou cruzaria 100% ao correr). O
/// incremento é ciente da vida EFETIVA da unidade (individual × tenda de nível, §4.1/§4.8):
/// peça durável/nível-5 gasta menos e é trocada mais tarde; limão/nível-extremo, antes.
pub(super) fn needs_decision(part: &CarPart, apply_tent: bool) -> bool {
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
pub(super) fn current_target(plan: &CarMaintenancePlan, car: &Car, part: PartType) -> u8 {
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
