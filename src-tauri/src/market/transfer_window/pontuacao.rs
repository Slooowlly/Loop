//! Scores e filtros do leilão: quanto o piloto valoriza uma oferta, quanto a equipe
//! valoriza um candidato, dignidade, elegibilidade e a regra de marca.

use super::tipos::{Candidate, Seat, WindowConfig};

pub(crate) const MAX_TIER: f64 = 6.0;
pub(crate) const SALARY_REF: f64 = 300_000.0;

/// Score do PILOTO sobre uma oferta (qual ele aceita). Confia mais no histórico
/// (prestígio) que na promessa do carro.
pub(crate) fn driver_offer_score(
    cfg: &WindowConfig,
    seat: &Seat,
    cand: &Candidate,
    salary: f64,
) -> f64 {
    // Tier RELATIVO: subir soma, descer subtrai (não é desejabilidade absoluta —
    // senão um piloto de tier baixo nunca aceitaria uma vaga boa do seu tier).
    let tier_delta = seat.tier as f64 - cand.tier as f64;
    let tier_component = (cfg.tier_base + cfg.tier_step * tier_delta).max(0.0);
    let mut score = tier_component
        + (seat.prestige / 100.0) * cfg.w_prestige
        + (seat.car_norm / 100.0).min(1.2) * cfg.w_car
        + (salary / SALARY_REF).min(1.0) * cfg.w_salary
        + if seat.is_n1 {
            cfg.w_role_n1
        } else {
            cfg.w_role_n2
        };
    if cand.slam_target.as_deref() == Some(seat.category.as_str()) {
        score += cfg.slam_bonus;
    }
    score
}

/// O piloto recusa cair 2+ tiers abaixo do seu nível, por qualquer salário (piso
/// de dignidade). Subir/lateral/descer 1 tier é permitido.
pub(crate) fn passes_dignity(cfg: &WindowConfig, seat: &Seat, cand: &Candidate) -> bool {
    cand.tier <= seat.tier + cfg.dignity_tier_gap.saturating_sub(1)
        || cand.tier.saturating_sub(seat.tier) < cfg.dignity_tier_gap
}

/// Score da EQUIPE sobre um candidato (quem entra na shortlist / quem ela assina).
/// Mérito esportivo: skill manda.
pub(crate) fn team_candidate_score(cand: &Candidate) -> f64 {
    cand.skill
}

/// Elegibilidade básica de um candidato pra uma vaga: tier dentro de ±1, OU craque
/// (pode descer pra caçar slam / a equipe abre exceção pelo super-qualificado).
pub(crate) fn eligible(cfg: &WindowConfig, seat: &Seat, cand: &Candidate) -> bool {
    // PROMOÇÃO de exatamente 1 tier (ex.: gt4→gt3): a licença é CONCEDIDA na
    // assinatura (igual ao ladder fill), senão as categorias de cima nunca abririam
    // pra quem vem de baixo e ficariam vazias na janela.
    let is_promotion = cand.tier + 1 == seat.tier;
    // Caso contrário, a licença é obrigatória.
    if cand.max_license < seat.required_license && !is_promotion {
        return false;
    }
    let near = (cand.tier as i16 - seat.tier as i16).abs() <= 1;
    near || cand.skill >= cfg.craque_skill
}

/// Regra de marca (IA, tiers 0-1): se o piloto-IA recebeu alguma oferta da MESMA
/// marca, ele ignora as cross-brand. Jogador (ai_respects_brand=false) não filtra.
pub(crate) fn filter_brand<'a>(
    cand: &Candidate,
    offers: &'a [(usize, f64)],
    seats: &[Seat],
) -> Vec<(usize, f64)> {
    let Some(brand) = cand.brand.as_deref() else {
        return offers.to_vec();
    };
    if !cand.ai_respects_brand {
        return offers.to_vec();
    }
    let same_brand: Vec<(usize, f64)> = offers
        .iter()
        .copied()
        .filter(|(si, _)| seat_brand(&seats[*si].category) == Some(brand))
        .collect();
    if same_brand.is_empty() {
        offers.to_vec()
    } else {
        same_brand
    }
}

/// Marca derivada do id da categoria (só tiers 0-1).
pub(crate) fn seat_brand(category: &str) -> Option<&'static str> {
    if category.starts_with("mazda_") {
        Some("mazda")
    } else if category.starts_with("toyota_") {
        Some("toyota")
    } else {
        None
    }
}

pub(crate) fn shortlist_size(cfg: &WindowConfig, tier: u8) -> usize {
    if tier <= cfg.low_tier_cap {
        cfg.shortlist_low
    } else {
        cfg.shortlist_top
    }
}
