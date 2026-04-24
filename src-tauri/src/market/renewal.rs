use rand::Rng;

use crate::finance::salary::{calculate_renewal_pressure_from_money, calculate_salary_ceiling};
use crate::market::visibility::{
    derive_market_visibility_profile, MarketVisibilityProfile, MarketVisibilityTier,
};
use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::enums::{PrimaryPersonality, TeamRole};
use crate::models::team::Team;

/// Contexto de continuidade percebido pelo piloto ao avaliar renovação.
///
/// Derivado de sinais disponíveis localmente — v1 usa `performance_score` e `papel`.
/// Não representa um modelo completo de qualidade do ambiente (carro, reputação, etc.),
/// que não são parâmetros desta função. Simplificação intencional para v1.
#[derive(Debug, Clone, PartialEq)]
enum RenewalContinuityContext {
    Forte,  // Continuidade claramente boa — performance alta + N1
    Neutro, // Continuidade razoável — situação intermediária
    Fraco,  // Continuidade abaixo do patamar — performance baixa ou N2 fraco
}

#[derive(Debug, Clone)]
pub struct RenewalDecision {
    pub should_renew: bool,
    pub reason: String,
    pub new_salary: Option<f64>,
    pub new_duration: Option<i32>,
    pub new_role: Option<TeamRole>,
}

#[derive(Debug, Clone, Copy)]
struct N2ConsistencyGuard {
    protects_hard_rejection: bool,
    non_renewal_risk: f64,
}

pub fn should_renew_contract(
    driver: &Driver,
    performance_score: f64,
    contract: &Contract,
    team: &Team,
    rng: &mut impl Rng,
) -> RenewalDecision {
    let n2_consistency_guard = n2_consistency_retention_guard(driver, performance_score);
    let mut decision = if driver.idade > 36 && performance_score < 60.0 {
        no_renewal("Veterano com desempenho abaixo da média")
    } else if driver.idade > 36 && performance_score < 75.0 && rng.gen_range(0.0..1.0) < 0.50 {
        no_renewal("Veterano, equipe busca sangue novo")
    } else if performance_score < 35.0 {
        no_renewal("Desempenho muito fraco")
    } else if performance_score < 50.0 && rng.gen_range(0.0..1.0) < 0.60 {
        no_renewal("Desempenho abaixo da média")
    } else {
        let salary_pressure = calculate_renewal_pressure_from_money(team, contract.salario_anual);
        if salary_pressure > 0.75 && performance_score < 70.0 {
            no_renewal("Salário desproporcional ao desempenho")
        } else if contract.papel == TeamRole::Numero2
            && performance_score < 55.0
            && !n2_consistency_guard.protects_hard_rejection
        {
            no_renewal("N2 fraco, equipe busca jovem promessa")
        } else if contract.papel == TeamRole::Numero2
            && performance_score < 65.0
            && rng.gen_range(0.0..1.0) < n2_consistency_guard.non_renewal_risk
        {
            no_renewal("N2 mediano, chance de buscar melhor")
        } else {
            let new_salary = calculate_renewal_salary(contract, performance_score, driver, team);
            let new_duration = if performance_score > 80.0 {
                rng.gen_range(2..=3)
            } else if performance_score > 60.0 {
                rng.gen_range(1..=2)
            } else {
                1
            };
            RenewalDecision {
                should_renew: true,
                reason: "Desempenho satisfatório".into(),
                new_salary: Some(new_salary),
                new_duration: Some(new_duration),
                new_role: Some(contract.papel.clone()),
            }
        }
    };

    // Resistência leve ao patamar de continuidade (visibilidade pública).
    // Aplicada apenas a decisões de aceitação que passaram todos os gates estruturais.
    // Não substitui hard rejections já avaliadas acima.
    // Não altera salário. Efeito máximo: 8% (Elite + Fraco).
    // Personalidade (Leal) pode sobrescrever esta resistência — posicionamento intencional.
    let driver_profile = derive_market_visibility_profile(driver.atributos.midia);
    let continuity_ctx = classify_renewal_continuity(performance_score, &contract.papel);
    let resistance = market_visibility_renewal_resistance(&driver_profile, continuity_ctx);
    if decision.should_renew && rng.gen_range(0.0..1.0) < resistance {
        decision = no_renewal("Piloto questiona continuidade");
    }

    match &driver.personalidade_primaria {
        Some(PrimaryPersonality::Leal) => {
            if !decision.should_renew && performance_score > 40.0 {
                decision.should_renew = true;
                decision.reason = "Leal — equipe dá outra chance".into();
                decision.new_salary = Some(
                    calculate_renewal_salary(contract, performance_score, driver, team) * 0.90,
                );
                decision.new_duration = Some(1);
                decision.new_role = Some(contract.papel.clone());
            } else if let Some(ref mut salary) = decision.new_salary {
                *salary *= 0.90;
            }
        }
        Some(PrimaryPersonality::Mercenario) => {
            if let Some(ref mut salary) = decision.new_salary {
                *salary *= 1.15;
            }
        }
        _ => {}
    }

    if let Some(ref mut salary) = decision.new_salary {
        *salary = salary
            .min(calculate_salary_ceiling(team))
            .max(5_000.0)
            .round();
    }

    decision
}

fn n2_consistency_retention_guard(driver: &Driver, performance_score: f64) -> N2ConsistencyGuard {
    if performance_score >= 65.0 {
        return N2ConsistencyGuard {
            protects_hard_rejection: true,
            non_renewal_risk: 0.0,
        };
    }

    if driver.atributos.consistencia >= 78.0 && performance_score >= 50.0 {
        N2ConsistencyGuard {
            protects_hard_rejection: true,
            non_renewal_risk: 0.20,
        }
    } else if driver.atributos.consistencia >= 70.0 && performance_score >= 50.0 {
        N2ConsistencyGuard {
            protects_hard_rejection: true,
            non_renewal_risk: 0.35,
        }
    } else {
        N2ConsistencyGuard {
            protects_hard_rejection: false,
            non_renewal_risk: 0.55,
        }
    }
}

/// Infere o contexto de continuidade a partir de sinais locais da renovação.
///
/// Usa apenas os parâmetros disponíveis em `should_renew_contract`: performance_score
/// e papel. Thresholds alinhados com os gates existentes (performance < 50, < 65 para N2).
fn classify_renewal_continuity(
    performance_score: f64,
    papel: &TeamRole,
) -> RenewalContinuityContext {
    if performance_score >= 70.0 && *papel == TeamRole::Numero1 {
        RenewalContinuityContext::Forte
    } else if performance_score < 50.0 || (*papel == TeamRole::Numero2 && performance_score < 65.0)
    {
        RenewalContinuityContext::Fraco
    } else {
        RenewalContinuityContext::Neutro
    }
}

/// Intensidade de sensibilidade ao patamar de continuidade por tier de visibilidade pública.
///
/// Espelha os valores de `visibility_status_sensitivity` em `driver_ai.rs` — escala
/// consistente e legível no sistema de mercado. Secundário a todos os fatores centrais.
fn visibility_continuity_sensitivity(profile: &MarketVisibilityProfile) -> f64 {
    match profile.tier {
        MarketVisibilityTier::Baixa => 0.0,
        MarketVisibilityTier::Relevante => 0.02,
        MarketVisibilityTier::Alta => 0.05,
        MarketVisibilityTier::Elite => 0.08,
    }
}

/// Resistência soft à renovação baseada em visibilidade pública e contexto de continuidade.
///
/// Retorna a probabilidade de resistência leve do piloto à renovação:
/// - Forte: 0.0 — sem resistência adicional (piloto confortável com a continuidade)
/// - Neutro: 0.0 — sem efeito
/// - Fraco: sensitivity — leve resistência proporcional ao perfil público
///
/// Efeito máximo: 8% (Elite + Fraco). Secundário a qualquer gate estrutural existente.
/// Não altera salário. Não cria rejeição automática.
///
/// Semântica honesta para v1: o helper modela apenas o lado da resistência à continuidade
/// fraca. Em contexto Forte, a ausência de resistência é a representação correta do conforto.
fn market_visibility_renewal_resistance(
    profile: &MarketVisibilityProfile,
    ctx: RenewalContinuityContext,
) -> f64 {
    match ctx {
        RenewalContinuityContext::Forte => 0.0,
        RenewalContinuityContext::Neutro => 0.0,
        RenewalContinuityContext::Fraco => visibility_continuity_sensitivity(profile),
    }
}

fn no_renewal(reason: &str) -> RenewalDecision {
    RenewalDecision {
        should_renew: false,
        reason: reason.to_string(),
        new_salary: None,
        new_duration: None,
        new_role: None,
    }
}

fn calculate_renewal_salary(
    contract: &Contract,
    performance: f64,
    driver: &Driver,
    team: &Team,
) -> f64 {
    let base = contract.salario_anual;
    let perf_modifier = if performance > 80.0 {
        1.20
    } else if performance > 60.0 {
        1.05
    } else {
        0.90
    };

    let age_modifier = if driver.idade > 34 { 0.85 } else { 1.0 };

    (base * perf_modifier * age_modifier)
        .min(calculate_salary_ceiling(team))
        .max(5_000.0)
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;
    use crate::market::visibility::derive_market_visibility_profile;
    use crate::models::team::{placeholder_team_from_db, Team};

    #[test]
    fn test_renew_good_performer() {
        let driver = sample_driver(29, None);
        let contract = sample_contract(TeamRole::Numero1, 80_000.0);
        let team = sample_healthy_team();
        let mut rng = StdRng::seed_from_u64(1);

        let decision = should_renew_contract(&driver, 82.0, &contract, &team, &mut rng);

        assert!(decision.should_renew);
        assert!(decision.new_salary.is_some());
    }

    #[test]
    fn test_no_renew_bad_performer() {
        let driver = sample_driver(28, None);
        let contract = sample_contract(TeamRole::Numero1, 80_000.0);
        let team = sample_healthy_team();
        let mut rng = StdRng::seed_from_u64(2);

        let decision = should_renew_contract(&driver, 30.0, &contract, &team, &mut rng);

        assert!(!decision.should_renew);
    }

    #[test]
    fn test_no_renew_old_driver_low_performance() {
        let driver = sample_driver(38, None);
        let contract = sample_contract(TeamRole::Numero1, 80_000.0);
        let team = sample_healthy_team();
        let mut rng = StdRng::seed_from_u64(3);

        let decision = should_renew_contract(&driver, 58.0, &contract, &team, &mut rng);

        assert!(!decision.should_renew);
    }

    #[test]
    fn test_loyal_driver_easier_renewal() {
        let driver = sample_driver(31, Some(PrimaryPersonality::Leal));
        let contract = sample_contract(TeamRole::Numero1, 50_000.0);
        let team = sample_healthy_team();
        let mut rng = StdRng::seed_from_u64(4);

        let decision = should_renew_contract(&driver, 45.0, &contract, &team, &mut rng);

        assert!(decision.should_renew);
        assert!(decision.reason.contains("Leal"));
    }

    #[test]
    fn test_mercenary_wants_more_salary() {
        let driver = sample_driver(30, Some(PrimaryPersonality::Mercenario));
        let contract = sample_contract(TeamRole::Numero1, 100_000.0);
        let team = sample_healthy_team();
        let mut rng = StdRng::seed_from_u64(5);

        let decision = should_renew_contract(&driver, 75.0, &contract, &team, &mut rng);

        assert!(decision.should_renew);
        assert!(decision.new_salary.expect("salary") > 100_000.0);
    }

    #[test]
    fn test_consistent_n2_has_better_renewal_odds() {
        let mut reliable_n2 = sample_driver(29, None);
        reliable_n2.atributos.consistencia = 78.0;

        let mut unreliable_n2 = sample_driver(29, None);
        unreliable_n2.atributos.consistencia = 55.0;

        let contract = sample_contract(TeamRole::Numero2, 55_000.0);
        let reliable_renewals = (1..=64)
            .filter(|seed| {
                let mut rng = StdRng::seed_from_u64(*seed);
                let team = sample_healthy_team();
                should_renew_contract(&reliable_n2, 60.0, &contract, &team, &mut rng).should_renew
            })
            .count();
        let unreliable_renewals = (1..=64)
            .filter(|seed| {
                let mut rng = StdRng::seed_from_u64(*seed);
                let team = sample_healthy_team();
                should_renew_contract(&unreliable_n2, 60.0, &contract, &team, &mut rng).should_renew
            })
            .count();

        assert!(
            reliable_renewals > unreliable_renewals,
            "N2 consistente deve ter protecao perceptivel na renovacao"
        );
    }

    #[test]
    fn renewal_uses_real_money_instead_of_legacy_budget() {
        let driver = sample_driver(29, None);
        let contract = sample_contract(TeamRole::Numero1, 160_000.0);
        let mut rich = sample_team("gt4", 6_000_000.0, 0.0, "healthy");
        rich.budget = 1.0;
        let mut poor = sample_team("gt4", 100_000.0, 2_500_000.0, "crisis");
        poor.budget = 99.0;
        let mut rng_rich = StdRng::seed_from_u64(21);
        let mut rng_poor = StdRng::seed_from_u64(21);

        let rich_decision = should_renew_contract(&driver, 68.0, &contract, &rich, &mut rng_rich);
        let poor_decision = should_renew_contract(&driver, 68.0, &contract, &poor, &mut rng_poor);

        assert!(rich_decision.should_renew);
        assert!(!poor_decision.should_renew);
        assert!(poor_decision.reason.contains("desproporcional"));
    }

    fn sample_driver(age: u32, personality: Option<PrimaryPersonality>) -> Driver {
        let mut driver = Driver::new(
            "P001".to_string(),
            "Piloto".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            age,
            2020,
        );
        driver.personalidade_primaria = personality;
        driver
    }

    fn sample_driver_with_media(
        age: u32,
        personality: Option<PrimaryPersonality>,
        midia: f64,
    ) -> Driver {
        let mut driver = sample_driver(age, personality);
        driver.atributos.midia = midia;
        driver
    }

    fn sample_team(category: &str, cash: f64, debt: f64, state: &str) -> Team {
        let mut team = placeholder_team_from_db(
            "T001".to_string(),
            "Equipe".to_string(),
            category.to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = cash;
        team.debt_balance = debt;
        team.financial_state = state.to_string();
        team.reputacao = 60.0;
        team
    }

    fn sample_healthy_team() -> Team {
        sample_team("gt4", 6_000_000.0, 0.0, "healthy")
    }

    // ── Testes de market_visibility_renewal_resistance ────────────────────────

    #[test]
    fn test_visibility_resistance_zero_for_forte() {
        for media in [0.0_f64, 30.0, 60.0, 90.0] {
            let profile = derive_market_visibility_profile(media);
            let r = market_visibility_renewal_resistance(&profile, RenewalContinuityContext::Forte);
            assert!(
                (r - 0.0).abs() < 1e-9,
                "Forte deve ser 0.0 para midia={media}"
            );
        }
    }

    #[test]
    fn test_visibility_resistance_zero_for_neutro() {
        for media in [0.0_f64, 30.0, 60.0, 90.0] {
            let profile = derive_market_visibility_profile(media);
            let r =
                market_visibility_renewal_resistance(&profile, RenewalContinuityContext::Neutro);
            assert!(
                (r - 0.0).abs() < 1e-9,
                "Neutro deve ser 0.0 para midia={media}"
            );
        }
    }

    #[test]
    fn test_visibility_resistance_positive_for_fraco() {
        let elite = derive_market_visibility_profile(90.0);
        let alta = derive_market_visibility_profile(70.0);
        let rel = derive_market_visibility_profile(40.0);
        let baixa = derive_market_visibility_profile(10.0);
        let r_elite = market_visibility_renewal_resistance(&elite, RenewalContinuityContext::Fraco);
        let r_alta = market_visibility_renewal_resistance(&alta, RenewalContinuityContext::Fraco);
        let r_rel = market_visibility_renewal_resistance(&rel, RenewalContinuityContext::Fraco);
        let r_baixa = market_visibility_renewal_resistance(&baixa, RenewalContinuityContext::Fraco);
        assert!((r_elite - 0.08).abs() < 1e-9);
        assert!((r_alta - 0.05).abs() < 1e-9);
        assert!((r_rel - 0.02).abs() < 1e-9);
        assert!((r_baixa - 0.0).abs() < 1e-9);
        assert!(r_elite > r_alta && r_alta > r_rel && r_rel > r_baixa);
    }

    #[test]
    fn test_classify_renewal_continuity_cases() {
        // Forte: performance >= 70 AND N1
        assert_eq!(
            classify_renewal_continuity(75.0, &TeamRole::Numero1),
            RenewalContinuityContext::Forte
        );
        // Fraco: performance < 50
        assert_eq!(
            classify_renewal_continuity(45.0, &TeamRole::Numero1),
            RenewalContinuityContext::Fraco
        );
        // Fraco: N2 com performance < 65
        assert_eq!(
            classify_renewal_continuity(60.0, &TeamRole::Numero2),
            RenewalContinuityContext::Fraco
        );
        // Neutro: N1 com performance 55
        assert_eq!(
            classify_renewal_continuity(55.0, &TeamRole::Numero1),
            RenewalContinuityContext::Neutro
        );
        // Neutro: N2 com performance >= 65
        assert_eq!(
            classify_renewal_continuity(68.0, &TeamRole::Numero2),
            RenewalContinuityContext::Neutro
        );
    }

    #[test]
    fn test_visibility_renewal_secondary_to_dominant_factor() {
        // Sanity: soft gate máximo (8%) << hard gate performance < 50 (60%)
        let elite = derive_market_visibility_profile(100.0);
        let max_resistance =
            market_visibility_renewal_resistance(&elite, RenewalContinuityContext::Fraco);
        let existing_hard_gate_prob = 0.60;
        assert!(max_resistance < existing_hard_gate_prob);
    }

    #[test]
    fn test_visibility_renewal_no_resistance_in_forte_context() {
        // Comportamental: contexto Forte → resistance = 0.0 → gate não dispara
        // Elite e Baixa produzem mesma decisão com mesmo seed
        let contract = sample_contract(TeamRole::Numero1, 90_000.0);
        let driver_elite = sample_driver_with_media(28, None, 90.0);
        let driver_baixa = sample_driver_with_media(28, None, 10.0);

        // performance=82, N1 → Forte → resistance=0.0 → sem gate extra
        let mut rng_e = StdRng::seed_from_u64(42);
        let mut rng_b = StdRng::seed_from_u64(42);
        let team = sample_healthy_team();
        let dec_elite = should_renew_contract(&driver_elite, 82.0, &contract, &team, &mut rng_e);
        let dec_baixa = should_renew_contract(&driver_baixa, 82.0, &contract, &team, &mut rng_b);

        assert_eq!(dec_elite.should_renew, dec_baixa.should_renew);
    }

    fn sample_contract(role: TeamRole, salary: f64) -> Contract {
        Contract::new(
            "C001".to_string(),
            "P001".to_string(),
            "Piloto".to_string(),
            "T001".to_string(),
            "Equipe".to_string(),
            1,
            1,
            salary,
            role,
            "gt4".to_string(),
        )
    }
}
