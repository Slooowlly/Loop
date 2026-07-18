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
    advance_race, can_stretch, replace_cost, stretch_cost, wear_per_race, PartAction,
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

/// Quanto uma peça serve à demanda PHA (produto escalar do viés com a demanda).
fn part_relevance(part: PartType, demand: (f64, f64, f64)) -> f64 {
    let (pp, ph, pa) = part.pha_per_level();
    let (dp, dh, da) = demand;
    pp * dp + ph * dh + pa * da
}

/// Uma peça precisa de decisão nesta corrida? (esgotada, ou cruzaria 100% ao correr).
fn needs_decision(part: &CarPart) -> bool {
    part.spent || part.wear + wear_per_race(part.part_type) >= 1.0
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
    let mut plan = CarMaintenancePlan::default();
    let mut budget = budget.max(0.0);

    let max_rel = PartType::ALL
        .iter()
        .map(|&pt| part_relevance(pt, demand))
        .fold(0.0_f64, f64::max);
    let rel_threshold = RELEVANCE_FRACTION * max_rel;

    // 1) Peças no fim da vida, mais relevantes primeiro.
    let mut eol: Vec<CarPart> = car.parts.iter().copied().filter(needs_decision).collect();
    eol.sort_by(|a, b| {
        part_relevance(b.part_type, demand)
            .partial_cmp(&part_relevance(a.part_type, demand))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for part in &eol {
        let rc = replace_cost(category_id, part);
        let sc = stretch_cost(category_id, part);
        let relevant = part_relevance(part.part_type, demand) >= rel_threshold;
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

    // 2) Passe de upgrade: com caixa sobrando, sobe as peças rumo ao seu TETO EFETIVO.
    //    ESPECIALIZAÇÃO: numa janela peaked (o calendário puxa um atributo), as peças
    //    IRRELEVANTES têm o teto rebaixado (ficam abaixo do teto da categoria) → o carro
    //    FOCA no atributo exigido; numa janela equilibrada, todas vão ao teto (balanceado).
    //    O horizonte do time modula: míope vê 1 pista (peaked → foca), temporada inteira vê
    //    a média (equilibra). Peças esticadas (spent) ficam de fora.
    let demand_peaked = demand_spread(demand) >= DEMAND_PEAK_THRESHOLD;
    let part_cap = |pt: PartType| -> u8 {
        if !demand_peaked || part_relevance(pt, demand) >= rel_threshold {
            ceiling
        } else {
            ceiling.saturating_sub(FOCUS_GAP).max(1)
        }
    };
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
    let demand = maintenance_demand(upcoming_track_ids);
    decide_maintenance(car, category_id, budget, demand)
}

/// Aplica o plano ao carro: instala os upgrades e roda a corrida (desgaste + ações).
pub fn apply_plan(car: &mut Car, plan: &CarMaintenancePlan) {
    for (&part, &level) in &plan.target_levels {
        car.set_level(part, level);
    }
    advance_race(car, &plan.actions);
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

/// Manutenção do carro de UM time após a corrida: o cérebro decide (trocar/esticar/
/// degradar/upgrade) olhando o caixa e as próximas pistas (cortadas pelo horizonte do
/// time), aplica o desgaste e persiste. Devolve o **custo gasto** — que vira depreciação
/// REAL na fatura (technical_investment_cost). Chamado por time dentro do
/// `persist_race_result_tx` (que já tem guard de idempotência), 1×/rodada.
pub fn maintain_team_car(
    conn: &Connection,
    team: &Team,
    category_id: &str,
    season_number: i32,
    all_upcoming_track_ids: &[u32],
) -> Result<f64, DbError> {
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

    // A janela de planejamento é cortada pelo horizonte do time.
    let horizon = planning_horizon(&team.id, season_number);
    let window: &[u32] = match horizon.lookahead() {
        Some(n) => &all_upcoming_track_ids[..all_upcoming_track_ids.len().min(n)],
        None => all_upcoming_track_ids,
    };

    let plan = decide_car_maintenance(team, &car, category_id, window);
    let cost = plan.estimated_cost;
    apply_plan(&mut car, &plan);
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
        // Monza (93) é power-heavy → P domina.
        let (p2, h2, a2) = maintenance_demand(&[93]);
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
            let cost = maintain_team_car(&conn, &team, "gt3", 1, &[93, 489, 325]).unwrap();
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
        maintain_team_car(&conn, &team, "mazda_amador", 1, &[]).unwrap();

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
}
