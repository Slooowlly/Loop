//! Fecho da janela: o passe de clearing (esvazia o mercado) e a rede de segurança
//! dos craques (backstop final).

use super::pontuacao::{eligible, passes_dignity, team_candidate_score};
use super::tipos::{Candidate, Seat, Signing, WindowConfig};

/// Passe de fechamento: cada vaga pega o melhor piloto elegível+dignidade que
/// sobrou (o mercado "esvazia"). Não força ninguém 2+ tiers abaixo.
pub(crate) fn clearing_pass(
    open: &mut Vec<Seat>,
    free: &mut Vec<Candidate>,
    signings: &mut Vec<Signing>,
    cfg: &WindowConfig,
    week: u32,
    limit: Option<usize>, // None = esvazia tudo; Some(n) = no máx. n assinaturas
) {
    let mut filled = 0usize;
    loop {
        if limit.is_some_and(|max| filled >= max) {
            break;
        }
        let mut best_pair: Option<(usize, usize)> = None;
        let mut best_prestige = f64::NEG_INFINITY;
        for si in 0..open.len() {
            let pick = (0..free.len())
                .filter(|&ci| {
                    // O JOGADOR nunca é auto-colocado pelo clearing — só assina via
                    // própria aceitação ou pela garantia de porta no fecho.
                    free[ci].ai_respects_brand
                        && eligible(cfg, &open[si], &free[ci])
                        && passes_dignity(cfg, &open[si], &free[ci])
                })
                .max_by(|&a, &b| {
                    team_candidate_score(&free[a]).total_cmp(&team_candidate_score(&free[b]))
                });
            if let Some(ci) = pick {
                if open[si].prestige > best_prestige {
                    best_prestige = open[si].prestige;
                    best_pair = Some((si, ci));
                }
            }
        }
        let Some((si, ci)) = best_pair else { break };
        let seat = open.remove(si);
        let salary = free[ci]
            .market_value
            .clamp(seat.salary_floor, seat.salary_ceiling);
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
        free.remove(ci);
        filled += 1;
        if open.is_empty() || free.is_empty() {
            break;
        }
    }
}

/// Rede de segurança por skill: craque (skill ≥ craque_skill) sempre acha vaga (a
/// melhor que sobrar), IGNORANDO dignidade — backstop final.
pub(crate) fn safety_net(
    open: &mut Vec<Seat>,
    free: &mut Vec<Candidate>,
    signings: &mut Vec<Signing>,
    cfg: &WindowConfig,
    week: u32,
) {
    let mut craques: Vec<usize> = (0..free.len())
        .filter(|&ci| free[ci].ai_respects_brand && free[ci].skill >= cfg.craque_skill)
        .collect();
    craques.sort_by(|&a, &b| free[b].skill.total_cmp(&free[a].skill));
    for ci in craques {
        if open.is_empty() {
            break;
        }
        let best_si = open
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.prestige.total_cmp(&b.prestige))
            .map(|(i, _)| i)
            .unwrap();
        let seat = open.remove(best_si);
        let salary = free[ci]
            .market_value
            .clamp(seat.salary_floor, seat.salary_ceiling);
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
    }
    let signed_ids: std::collections::HashSet<&str> =
        signings.iter().map(|s| s.driver_id.as_str()).collect();
    free.retain(|c| !signed_ids.contains(c.id.as_str()));
}
