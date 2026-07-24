//! O ciclo semanal do leilão: OFERTAS (shortlist + escalada do lance) e
//! RESPOSTAS/RESULTADOS (piloto aceita a melhor; a vaga assina o nº1).

use std::collections::HashMap;

use super::pontuacao::{
    driver_offer_score, eligible, filter_brand, passes_dignity, shortlist_size, team_candidate_score,
};
use super::tipos::{Candidate, Seat, Signing, WindowConfig};

pub(crate) fn salkey(seat: &str, driver: &str) -> String {
    format!("{seat}\u{1}{driver}")
}

/// OFERTAS: cada vaga monta a shortlist e oferta (escalando o lance). Devolve
/// `driver_index -> [(seat_index, salário)]` e atualiza `current_salary`.
pub(crate) fn compute_offers(
    open: &[Seat],
    free: &[Candidate],
    current_salary: &mut HashMap<String, f64>,
    cfg: &WindowConfig,
) -> HashMap<usize, Vec<(usize, f64)>> {
    let mut offers_by_driver: HashMap<usize, Vec<(usize, f64)>> = HashMap::new();
    for (si, seat) in open.iter().enumerate() {
        let mut ranked: Vec<usize> = (0..free.len())
            .filter(|&ci| eligible(cfg, seat, &free[ci]))
            .collect();
        ranked.sort_by(|&a, &b| {
            team_candidate_score(&free[b])
                .total_cmp(&team_candidate_score(&free[a]))
                .then_with(|| free[a].id.cmp(&free[b].id))
        });
        for &ci in ranked.iter().take(shortlist_size(cfg, seat.tier)) {
            let key = salkey(&seat.id, &free[ci].id);
            let salary = match current_salary.get(&key) {
                Some(&prev) => (prev + cfg.bid_gap_close * (seat.salary_ceiling - prev))
                    .min(seat.salary_ceiling),
                None => free[ci]
                    .market_value
                    .clamp(seat.salary_floor, seat.salary_ceiling),
            };
            current_salary.insert(key, salary);
            offers_by_driver.entry(ci).or_default().push((si, salary));
        }
    }

    // ── GARANTIA DE OFERTAS AO JOGADOR ──────────────────────────────────────────
    // O jogador (único com ai_respects_brand=false) é rankeado por skill como todos;
    // um piloto fraco (rookie) nunca entraria na shortlist e ficaria SEM nenhuma
    // oferta. Garante ofertas das vagas do NÍVEL DELE (próprio tier), as de maior
    // prestígio primeiro, até um teto por semana — ele recebe das duas marcas.
    if let Some(pi) = free.iter().position(|c| !c.ai_respects_brand) {
        let cap = cfg.shortlist_top as usize + 1;
        let mut count = offers_by_driver.get(&pi).map_or(0, Vec::len);
        let mut tier_seats: Vec<usize> = (0..open.len())
            .filter(|&si| {
                open[si].tier == free[pi].tier
                    && eligible(cfg, &open[si], &free[pi])
                    && passes_dignity(cfg, &open[si], &free[pi])
            })
            .collect();
        tier_seats.sort_by(|&a, &b| open[b].prestige.total_cmp(&open[a].prestige));
        for si in tier_seats {
            if count >= cap {
                break;
            }
            let already = offers_by_driver
                .get(&pi)
                .is_some_and(|v| v.iter().any(|&(s, _)| s == si));
            if already {
                continue;
            }
            let key = salkey(&open[si].id, &free[pi].id);
            let salary = match current_salary.get(&key) {
                Some(&prev) => (prev + cfg.bid_gap_close * (open[si].salary_ceiling - prev))
                    .min(open[si].salary_ceiling),
                None => free[pi]
                    .market_value
                    .clamp(open[si].salary_floor, open[si].salary_ceiling),
            };
            current_salary.insert(key, salary);
            offers_by_driver.entry(pi).or_default().push((si, salary));
            count += 1;
        }
    }

    offers_by_driver
}

/// RESPOSTAS + RESULTADOS de uma semana: cada piloto aceita sua melhor oferta (o
/// JOGADOR usa `player_choice`); cada vaga assina o nº1 entre os que aceitaram.
/// Mutaciona open/free/signings. Devolve se houve alguma assinatura.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_week(
    open: &mut Vec<Seat>,
    free: &mut Vec<Candidate>,
    signings: &mut Vec<Signing>,
    offers_by_driver: &HashMap<usize, Vec<(usize, f64)>>,
    week: u32,
    threshold: f64,
    cfg: &WindowConfig,
    player_id: Option<&str>,
    player_choice: Option<&str>,
) -> bool {
    let mut accepted_by_seat: HashMap<usize, Vec<(usize, f64)>> = HashMap::new();
    for (&ci, raw_offers) in offers_by_driver.iter() {
        let chosen = if player_id == Some(free[ci].id.as_str()) {
            // JOGADOR: aceita a vaga que escolheu (por id), se estiver entre as ofertas;
            // senão (None) espera (não aceita nada nesta semana).
            player_choice.and_then(|seat_id| {
                raw_offers
                    .iter()
                    .copied()
                    .find(|&(si, _)| open[si].id == seat_id)
            })
        } else {
            let offers = filter_brand(&free[ci], raw_offers, open);
            offers
                .iter()
                .copied()
                .filter(|&(si, salary)| {
                    passes_dignity(cfg, &open[si], &free[ci])
                        && driver_offer_score(cfg, &open[si], &free[ci], salary) >= threshold
                })
                .max_by(|&(sa, salary_a), &(sb, salary_b)| {
                    driver_offer_score(cfg, &open[sa], &free[ci], salary_a)
                        .total_cmp(&driver_offer_score(cfg, &open[sb], &free[ci], salary_b))
                })
        };
        if let Some((si, salary)) = chosen {
            accepted_by_seat.entry(si).or_default().push((ci, salary));
        }
    }

    let mut signed_seat_indices: Vec<usize> = Vec::new();
    let mut signed_driver_indices: Vec<usize> = Vec::new();
    let mut any_signed = false;
    let mut seat_order: Vec<usize> = (0..open.len()).collect();
    seat_order.sort_by(|&a, &b| open[b].prestige.total_cmp(&open[a].prestige));
    for si in seat_order {
        if signed_seat_indices.contains(&si) {
            continue;
        }
        let Some(accepters) = accepted_by_seat.get(&si) else {
            continue;
        };
        let available: Vec<(usize, f64)> = accepters
            .iter()
            .copied()
            .filter(|&(ci, _)| !signed_driver_indices.contains(&ci))
            .collect();
        // A aceitação do JOGADOR é honrada: o time que o ofertou assina ELE (não um
        // IA mais forte que também aceitou) — senão um rookie nunca venceria a vaga.
        let pick = available
            .iter()
            .copied()
            .find(|&(ci, _)| !free[ci].ai_respects_brand)
            .or_else(|| {
                available.iter().copied().max_by(|&(ca, _), &(cb, _)| {
                    team_candidate_score(&free[ca]).total_cmp(&team_candidate_score(&free[cb]))
                })
            });
        if let Some((ci, salary)) = pick {
            let seat = &open[si];
            signings.push(Signing {
                seat_id: seat.id.clone(),
                team_id: seat.team_id.clone(),
                driver_id: free[ci].id.clone(),
                category: seat.category.clone(),
                class: seat.class.clone(),
                salary,
                week,
                from_category: (!free[ci].category.is_empty()).then(|| free[ci].category.clone()),
            });
            signed_seat_indices.push(si);
            signed_driver_indices.push(ci);
            any_signed = true;
        }
    }
    signed_seat_indices.sort_unstable_by(|a, b| b.cmp(a));
    for si in signed_seat_indices {
        open.remove(si);
    }
    signed_driver_indices.sort_unstable_by(|a, b| b.cmp(a));
    for ci in signed_driver_indices {
        free.remove(ci);
    }
    any_signed
}
