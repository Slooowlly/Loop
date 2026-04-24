#![allow(dead_code)]

use rand::Rng;

use crate::market::proposals::MarketProposal;
use crate::market::visibility::{
    derive_market_visibility_profile, MarketVisibilityProfile, MarketVisibilityTier,
};
use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::enums::{PrimaryPersonality, TeamRole};

#[derive(Debug, Clone)]
pub struct ProposalDecision {
    pub accepted: bool,
    pub reason: String,
}

pub fn evaluate_proposal(
    driver: &Driver,
    proposal: &MarketProposal,
    current_contract: Option<&Contract>,
    current_category_tier: u8,
    proposal_category_tier: u8,
    team_car_performance: f64,
    team_reputacao: f64,
    rng: &mut impl Rng,
) -> ProposalDecision {
    let car_perf_normalized = (((team_car_performance + 5.0) / 21.0) * 100.0).clamp(0.0, 100.0);
    let salary_minimum = calculate_salary_minimum(driver, current_category_tier);

    if proposal.salario_oferecido < salary_minimum * 0.80 {
        return reject("Salário abaixo do aceitável");
    }

    if proposal_category_tier < current_category_tier && current_contract.is_some() {
        if rng.gen_range(0.0..1.0) > 0.30 {
            return reject("Não quer descer de categoria");
        }
    }

    if proposal.papel == TeamRole::Numero2
        && driver.atributos.skill > 70.0
        && rng.gen_range(0.0..1.0) < 0.50
    {
        return reject("Quer ser N1");
    }

    let mut score = (car_perf_normalized / 100.0) * 30.0
        + (proposal_category_tier as f64 / 5.0) * 25.0
        + if proposal.papel == TeamRole::Numero1 {
            15.0
        } else {
            10.0
        }
        + (proposal.salario_oferecido / 300_000.0).min(1.0) * 15.0
        + (team_reputacao / 100.0) * 10.0;

    match &driver.personalidade_primaria {
        Some(PrimaryPersonality::Ambicioso) => {
            if proposal_category_tier > current_category_tier {
                score += 15.0;
            }
            if proposal_category_tier <= current_category_tier {
                score -= 10.0;
            }
        }
        Some(PrimaryPersonality::Mercenario) => {
            if proposal.salario_oferecido > salary_minimum * 1.3 {
                score += 20.0;
            }
        }
        Some(PrimaryPersonality::Consolidador) => {
            if proposal_category_tier == current_category_tier && car_perf_normalized > 60.0 {
                score += 10.0;
            }
        }
        Some(PrimaryPersonality::Leal) => {
            score -= 15.0;
        }
        _ => {}
    }

    // Ajuste leve por visibilidade pública — sensibilidade ao patamar da proposta.
    // Atua apenas na componente contínua de aceitação (score final).
    // Não substitui nem contorna hard rejections já avaliadas acima
    // (salary_floor, category_downgrade, n2_resistance, poor_car).
    let driver_profile = derive_market_visibility_profile(driver.atributos.midia);
    let step_up = proposal_category_tier > current_category_tier;
    let step_down = proposal_category_tier < current_category_tier;
    score += market_visibility_acceptance_adjustment(&driver_profile, step_up, step_down);

    let threshold = 50.0 + rng.gen_range(-10.0..=10.0);
    if score >= threshold {
        accept()
    } else {
        reject("Proposta não atrativa o suficiente")
    }
}

fn calculate_salary_minimum(driver: &Driver, current_category_tier: u8) -> f64 {
    let tier_base = match current_category_tier {
        0 => 10_000.0,
        1 => 22_000.0,
        2 => 40_000.0,
        3 => 80_000.0,
        4 => 140_000.0,
        _ => 160_000.0,
    };

    let skill_factor = (driver.atributos.skill / 70.0).clamp(0.6, 1.6);
    (tier_base * skill_factor).max(5_000.0)
}

/// Intensidade de sensibilidade a status por tier de visibilidade pública.
///
/// Representa o quanto o perfil público do piloto amplifica sua reação
/// ao patamar da proposta (step up / step down). Valores deliberadamente
/// pequenos — o ajuste é secundário a todos os fatores esportivos principais.
fn visibility_status_sensitivity(profile: &MarketVisibilityProfile) -> f64 {
    match profile.tier {
        MarketVisibilityTier::Baixa => 0.0,
        MarketVisibilityTier::Relevante => 0.02,
        MarketVisibilityTier::Alta => 0.05,
        MarketVisibilityTier::Elite => 0.08,
    }
}

/// Ajuste de aceitação de proposta baseado em visibilidade pública e patamar da oportunidade.
///
/// Aplica sensibilidade a status de forma direcional:
/// - step up (proposta em tier superior): pilotos mais públicos ficam levemente mais inclinados
/// - step down (proposta em tier inferior): pilotos mais públicos ficam levemente mais resistentes
/// - lateral (tier equivalente): ajuste zero, independente do tier de visibilidade
///
/// Efeito máximo absoluto: ±0.08 — secundário a qualquer fator esportivo principal.
/// Não afeta salário, thresholds estruturais nem hard rejections.
///
/// **v1 — patamar como tier de categoria:** nesta versão, "patamar da proposta" é
/// aproximado pela comparação entre `proposal_category_tier` e `current_category_tier`.
/// Não representa um modelo completo de status da oferta (reputação da equipe, carro,
/// package financeiro). Essa simplificação é intencional para v1.
fn market_visibility_acceptance_adjustment(
    profile: &MarketVisibilityProfile,
    step_up: bool,
    step_down: bool,
) -> f64 {
    let sensitivity = visibility_status_sensitivity(profile);
    if step_up {
        sensitivity
    } else if step_down {
        -sensitivity
    } else {
        0.0
    }
}

fn accept() -> ProposalDecision {
    ProposalDecision {
        accepted: true,
        reason: "Proposta aceita".to_string(),
    }
}

fn reject(reason: &str) -> ProposalDecision {
    ProposalDecision {
        accepted: false,
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;
    use crate::market::proposals::{MarketProposal, ProposalStatus};

    #[test]
    fn test_accept_good_proposal() {
        let driver = sample_driver(65.0, None);
        let proposal = sample_proposal(120_000.0, TeamRole::Numero1);
        let mut rng = StdRng::seed_from_u64(1);

        let result = evaluate_proposal(&driver, &proposal, None, 2, 3, 12.0, 80.0, &mut rng);

        assert!(result.accepted);
    }

    #[test]
    fn test_reject_low_salary() {
        let driver = sample_driver(68.0, None);
        let proposal = sample_proposal(5_000.0, TeamRole::Numero1);
        let mut rng = StdRng::seed_from_u64(2);

        let result = evaluate_proposal(&driver, &proposal, None, 3, 3, 10.0, 70.0, &mut rng);

        assert!(!result.accepted);
        assert!(result.reason.contains("Salário"));
    }

    #[test]
    fn test_reject_category_downgrade() {
        let driver = sample_driver(72.0, None);
        let proposal = sample_proposal(90_000.0, TeamRole::Numero1);
        let contract = sample_contract();
        let mut rng = StdRng::seed_from_u64(7);

        let result = evaluate_proposal(
            &driver,
            &proposal,
            Some(&contract),
            4,
            2,
            10.0,
            70.0,
            &mut rng,
        );

        assert!(!result.accepted);
    }

    #[test]
    fn test_ambitious_accepts_promotion() {
        let driver = sample_driver(66.0, Some(PrimaryPersonality::Ambicioso));
        let proposal = sample_proposal(80_000.0, TeamRole::Numero2);
        let mut rng = StdRng::seed_from_u64(3);

        let result = evaluate_proposal(&driver, &proposal, None, 1, 3, 8.0, 70.0, &mut rng);

        assert!(result.accepted);
    }

    #[test]
    fn test_mercenary_follows_money() {
        let driver = sample_driver(64.0, Some(PrimaryPersonality::Mercenario));
        let proposal = sample_proposal(220_000.0, TeamRole::Numero2);
        let mut rng = StdRng::seed_from_u64(4);

        let result = evaluate_proposal(&driver, &proposal, None, 2, 2, 6.0, 55.0, &mut rng);

        assert!(result.accepted);
    }

    #[test]
    fn test_weak_car_does_not_auto_reject_strong_offer() {
        let driver = sample_driver(70.0, None);
        let proposal = sample_proposal(300_000.0, TeamRole::Numero1);

        for seed in 1..=12 {
            let mut rng = StdRng::seed_from_u64(seed);
            let result = evaluate_proposal(&driver, &proposal, None, 3, 4, 0.0, 80.0, &mut rng);
            assert!(
                result.accepted,
                "carro fraco nao deve gerar trava absoluta quando a oferta e forte; seed={seed}"
            );
        }
    }

    #[test]
    fn test_loyal_resists_transfer() {
        let driver = sample_driver(64.0, Some(PrimaryPersonality::Leal));
        let proposal = sample_proposal(80_000.0, TeamRole::Numero1);
        let contract = sample_contract();
        let mut rng = StdRng::seed_from_u64(5);

        let result = evaluate_proposal(
            &driver,
            &proposal,
            Some(&contract),
            2,
            2,
            8.0,
            60.0,
            &mut rng,
        );

        assert!(!result.accepted);
    }

    fn sample_driver(skill: f64, personality: Option<PrimaryPersonality>) -> Driver {
        let mut driver = Driver::new(
            "P001".to_string(),
            "Piloto".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            25,
            2020,
        );
        driver.atributos.skill = skill;
        driver.personalidade_primaria = personality;
        driver
    }

    fn sample_driver_with_media(
        skill: f64,
        personality: Option<PrimaryPersonality>,
        midia: f64,
    ) -> Driver {
        let mut driver = sample_driver(skill, personality);
        driver.atributos.midia = midia;
        driver
    }

    fn sample_proposal(salary: f64, role: TeamRole) -> MarketProposal {
        MarketProposal {
            id: "MP001".to_string(),
            equipe_id: "T001".to_string(),
            equipe_nome: "Equipe".to_string(),
            piloto_id: "P001".to_string(),
            piloto_nome: "Piloto".to_string(),
            categoria: "gt4".to_string(),
            papel: role,
            salario_oferecido: salary,
            duracao_anos: 2,
            status: ProposalStatus::Pendente,
            motivo_recusa: None,
        }
    }

    // ── Testes de market_visibility_acceptance_adjustment ─────────────────────

    #[test]
    fn test_visibility_step_up_positive_adjustment() {
        let elite = derive_market_visibility_profile(90.0);
        let baixa = derive_market_visibility_profile(10.0);
        let adj_elite = market_visibility_acceptance_adjustment(&elite, true, false);
        let adj_baixa = market_visibility_acceptance_adjustment(&baixa, true, false);
        assert!((adj_elite - 0.08).abs() < 1e-9);
        assert!((adj_baixa - 0.0).abs() < 1e-9);
        assert!(adj_elite > adj_baixa);
    }

    #[test]
    fn test_visibility_step_down_negative_adjustment() {
        let elite = derive_market_visibility_profile(90.0);
        let baixa = derive_market_visibility_profile(10.0);
        let adj_elite = market_visibility_acceptance_adjustment(&elite, false, true);
        let adj_baixa = market_visibility_acceptance_adjustment(&baixa, false, true);
        assert!((adj_elite - (-0.08)).abs() < 1e-9);
        assert!((adj_baixa - 0.0).abs() < 1e-9);
        assert!(adj_elite < adj_baixa);
    }

    #[test]
    fn test_visibility_lateral_zero_adjustment() {
        for media in [0.0_f64, 30.0, 60.0, 90.0] {
            let profile = derive_market_visibility_profile(media);
            let adj = market_visibility_acceptance_adjustment(&profile, false, false);
            assert!(
                (adj - 0.0).abs() < 1e-9,
                "lateral deve ser 0.0 para midia={media}"
            );
        }
    }

    #[test]
    fn test_visibility_adjustment_secondary_to_dominant_factor() {
        // Sanity check numérico: ajuste máximo (0.08) < menor componente principal (10 pts)
        let elite = derive_market_visibility_profile(100.0);
        let max_adj = market_visibility_acceptance_adjustment(&elite, true, false);
        let min_main_component = 10.0; // team_reputacao contribui até 10 pts
        assert!(max_adj < min_main_component);
    }

    #[test]
    fn test_visibility_adjustment_does_not_flip_lateral_outcome() {
        // Comportamental: em proposta lateral (ajuste = 0.0 para ambos), piloto Elite
        // e piloto Baixa com mesmos stats esportivos produzem a mesma decisão.
        // Prova que a visibilidade não adultera o resultado quando não há step.
        let proposal = sample_proposal(120_000.0, TeamRole::Numero1);

        let driver_elite = sample_driver_with_media(70.0, None, 90.0); // Elite
        let driver_baixa = sample_driver_with_media(70.0, None, 10.0); // Baixa

        // Ambos tier 3 → lateral → ajuste = 0.0 → decisão idêntica com mesmo seed
        let mut rng_e = StdRng::seed_from_u64(42);
        let mut rng_b = StdRng::seed_from_u64(42);
        let dec_elite =
            evaluate_proposal(&driver_elite, &proposal, None, 3, 3, 14.0, 80.0, &mut rng_e);
        let dec_baixa =
            evaluate_proposal(&driver_baixa, &proposal, None, 3, 3, 14.0, 80.0, &mut rng_b);

        assert_eq!(dec_elite.accepted, dec_baixa.accepted);
    }

    fn sample_contract() -> Contract {
        Contract::new(
            "C001".to_string(),
            "P001".to_string(),
            "Piloto".to_string(),
            "T001".to_string(),
            "Equipe Atual".to_string(),
            1,
            2,
            100_000.0,
            TeamRole::Numero1,
            "gt4".to_string(),
        )
    }
}
