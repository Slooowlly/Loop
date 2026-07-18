use rand::Rng;

use crate::finance::focus::TeamFocus;
use crate::finance::salary::{
    calculate_offer_salary_from_money, calculate_renewal_pressure_from_money,
    calculate_salary_ceiling,
};
use crate::market::bond::bond_level;
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

/// Aplica **Vínculo + Foco** à decisão-base de renovação (ideia 4 "Foco + Vínculo",
/// Fase 1). Layer NÃO-invasiva: roda DEPOIS de [`should_renew_contract`] sem alterar
/// os gates estruturais dele — só adiciona a lealdade da relação/fase por cima.
///
/// - **Buffer de confiança:** um par com história (Vínculo nível ≥ 4, "Pilar do time")
///   banca uma temporada mediana — é o "segurar o piloto pra fazer história". Não vale
///   em Sobrevivência (fase mercenária) nem com desempenho muito fraco.
/// - **Contrato de projeto:** foco de longo prazo (Celeiro/Dinastia) + vínculo → renovação
///   plurianual.
/// - **Mercenário:** em Sobrevivência, nada de longo prazo (1 ano).
pub fn apply_bond_and_focus_to_renewal(
    mut decision: RenewalDecision,
    driver: &Driver,
    performance_score: f64,
    contract: &Contract,
    team: &Team,
    vinculo: f64,
    foco: TeamFocus,
) -> RenewalDecision {
    let level = bond_level(vinculo);
    let mercenary_focus = matches!(foco, TeamFocus::Sobrevivencia);
    let long_term_focus = matches!(foco, TeamFocus::Dinastia | TeamFocus::Celeiro);

    if !decision.should_renew && level >= 4 && !mercenary_focus && performance_score > 40.0 {
        let salary = (calculate_renewal_salary(contract, performance_score, driver, team) * 0.95)
            .max(5_000.0)
            .round();
        decision.should_renew = true;
        decision.reason = "Vínculo forte — a equipe banca mais um ano".into();
        decision.new_salary = Some(salary);
        decision.new_duration = Some(1);
        decision.new_role = Some(contract.papel.clone());
    }

    if decision.should_renew {
        if mercenary_focus {
            decision.new_duration = Some(1);
        } else if long_term_focus && level >= 3 {
            let want = if level >= 5 { 3 } else { 2 };
            let current = decision.new_duration.unwrap_or(1);
            decision.new_duration = Some(current.max(want));
        }
    }

    decision
}

/// Duração (anos) da oferta ao JOGADOR conforme Foco do time + Vínculo com ele
/// (ideia 4). É o "segurar-vs-vender" do lado do jogador: um time-casa (vínculo alto)
/// ou de foco de longo prazo oferece um **contrato de projeto plurianual**; um time
/// em Sobrevivência (mercenário) só um ano. O jogador SEMPRE decide — isto só define
/// o que ELE vê na oferta e assina se aceitar.
pub fn player_offer_duration(foco: TeamFocus, vinculo: f64) -> i32 {
    let level = bond_level(vinculo);
    if matches!(foco, TeamFocus::Sobrevivencia) {
        1
    } else if level >= 5 {
        3
    } else if level >= 3 || matches!(foco, TeamFocus::Dinastia | TeamFocus::Celeiro) {
        2
    } else {
        1
    }
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

/// Prêmio salarial pela FAMA pública (midia) na renovação: apelo de mercado vira
/// poder de barganha do piloto. SEMPRE ≥ 1.0 — é um prêmio POR CIMA do valor
/// esportivo, nunca o substitui (o mérito vem de `perf_modifier`). Decisão travada:
/// o jogador famoso ganha salário melhor sem depender só de resultado.
fn fame_salary_premium(media: f64) -> f64 {
    match derive_market_visibility_profile(media).tier {
        MarketVisibilityTier::Baixa => 1.0,
        MarketVisibilityTier::Relevante => 1.03,
        MarketVisibilityTier::Alta => 1.08,
        MarketVisibilityTier::Elite => 1.15,
    }
}

/// Velocidade com que o salário persegue o valor de mercado a cada renovação.
/// ASSIMÉTRICO de propósito: sobe rápido, desce devagar — o contrato protege o
/// piloto no curto prazo, mas o veterano em declínio acaba cedendo, em vez de
/// congelar num "salário zumbi" que o time nunca consegue reajustar.
const RENEWAL_CATCHUP_UP: f64 = 0.60;
const RENEWAL_CATCHUP_DOWN: f64 = 0.25;

/// Salário-ALVO da renovação: o que este piloto valeria indo ao mercado hoje,
/// nesta equipe e nesta categoria.
///
/// Ancorar no MERCADO (e não no salário anterior) é o que faz o salário
/// acompanhar a escada. A versão antiga fazia `base = contract.salario_anual`,
/// e daí dois defeitos: (a) nunca se re-ancorava na categoria, então quem subia
/// de divisão carregava o salário velho pra sempre; (b) o modificador de
/// performance ≤ 60 — o caso da MAIORIA, já que performance é relativa —
/// multiplicava por 0.90 a cada renovação, uma catraca composta que só descia.
/// Medido numa carreira real: o salário médio do GT3 caiu de 216k (temp. 1) para
/// 101k (temp. 42) enquanto o caixa das equipes subia.
///
/// Performance, idade e fama agora modulam o ALVO, não o histórico — um ano ruim
/// custa um desconto sobre o valor de mercado, não um corte permanente que se
/// acumula sobre todos os anos ruins anteriores.
fn renewal_target_salary(driver: &Driver, team: &Team, performance: f64) -> f64 {
    let market_value = calculate_offer_salary_from_money(team, driver.atributos.skill);
    let perf_modifier = if performance > 80.0 {
        1.20
    } else if performance > 60.0 {
        1.05
    } else {
        0.90
    };
    let age_modifier = if driver.idade > 34 { 0.85 } else { 1.0 };
    let fame_modifier = fame_salary_premium(driver.atributos.midia);

    market_value * perf_modifier * age_modifier * fame_modifier
}

fn calculate_renewal_salary(
    contract: &Contract,
    performance: f64,
    driver: &Driver,
    team: &Team,
) -> f64 {
    let current = contract.salario_anual;
    let gap = renewal_target_salary(driver, team, performance) - current;
    let catchup = if gap >= 0.0 {
        RENEWAL_CATCHUP_UP
    } else {
        RENEWAL_CATCHUP_DOWN
    };

    (current + gap * catchup)
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
    fn fama_da_premio_de_salario_por_cima_do_merito() {
        // Prêmio SEMPRE ≥ 1.0 (nunca reduz) e monotônico: Elite > Baixa.
        assert!((fame_salary_premium(10.0) - 1.0).abs() < 1e-9);
        assert!(fame_salary_premium(90.0) > fame_salary_premium(10.0));
        for m in [0.0, 30.0, 60.0, 90.0, 100.0] {
            assert!(fame_salary_premium(m) >= 1.0, "prêmio nunca reduz: {m}");
        }
        // Na renovação: mesmo piloto/contrato, mais fama = salário maior.
        let contract = sample_contract(TeamRole::Numero1, 80_000.0);
        let team = sample_healthy_team();
        let mut apagado = sample_driver(29, None);
        apagado.atributos.midia = 10.0;
        let mut estrela = sample_driver(29, None);
        estrela.atributos.midia = 95.0;
        let s_apagado = calculate_renewal_salary(&contract, 82.0, &apagado, &team);
        let s_estrela = calculate_renewal_salary(&contract, 82.0, &estrela, &team);
        assert!(s_estrela > s_apagado, "estrela: {s_estrela} vs apagado: {s_apagado}");
    }

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
        let mut rich = sample_team("gt4", 6_000_000.0, 0.0, "healthy");
        rich.budget = 1.0;
        let mut poor = sample_team("gt4", 100_000.0, 2_500_000.0, "crisis");
        poor.budget = 99.0;
        // Salário desproporcional PARA A EQUIPE POBRE, ancorado no teto dela (não num
        // número fixo) — resiste a recalibrações do peso salarial. Acima do teto do
        // pobre mas confortável pro teto muito maior do rico.
        let salario = calculate_salary_ceiling(&poor) * 1.20;
        let contract = sample_contract(TeamRole::Numero1, salario);
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

    // ── Layer de Vínculo + Foco (ideia 4) ─────────────────────────────────────

    fn renew_decision(duration: i32) -> RenewalDecision {
        RenewalDecision {
            should_renew: true,
            reason: "base".into(),
            new_salary: Some(80_000.0),
            new_duration: Some(duration),
            new_role: Some(TeamRole::Numero1),
        }
    }

    #[test]
    fn strong_bond_buffers_a_mediocre_season() {
        let driver = sample_driver(29, None);
        let contract = sample_contract(TeamRole::Numero1, 80_000.0);
        let team = sample_healthy_team();
        // Vínculo nível 4 (Pilar do time) + Celeiro + desempenho mediano (45) → banca.
        let out = apply_bond_and_focus_to_renewal(
            no_renewal("Desempenho abaixo da média"),
            &driver,
            45.0,
            &contract,
            &team,
            60.0,
            TeamFocus::Celeiro,
        );
        assert!(out.should_renew);
        assert!(out.reason.contains("Vínculo forte"));
        assert!(out.new_salary.is_some());
    }

    #[test]
    fn survival_focus_does_not_buffer_and_stays_mercenary() {
        let driver = sample_driver(29, None);
        let contract = sample_contract(TeamRole::Numero1, 80_000.0);
        let team = sample_healthy_team();
        let out = apply_bond_and_focus_to_renewal(
            no_renewal("Desempenho abaixo da média"),
            &driver,
            45.0,
            &contract,
            &team,
            90.0, // vínculo altíssimo não salva na fase mercenária
            TeamFocus::Sobrevivencia,
        );
        assert!(!out.should_renew);
    }

    #[test]
    fn weak_bond_does_not_buffer() {
        let driver = sample_driver(29, None);
        let contract = sample_contract(TeamRole::Numero1, 80_000.0);
        let team = sample_healthy_team();
        let out = apply_bond_and_focus_to_renewal(
            no_renewal("Desempenho abaixo da média"),
            &driver,
            45.0,
            &contract,
            &team,
            20.0, // nível 2 — sem história ainda
            TeamFocus::Celeiro,
        );
        assert!(!out.should_renew);
    }

    #[test]
    fn long_term_focus_offers_multi_year_project_contract() {
        let driver = sample_driver(29, None);
        let contract = sample_contract(TeamRole::Numero1, 80_000.0);
        let team = sample_healthy_team();
        // Dinastia + vínculo nível 5 (Símbolo) → contrato de 3 anos.
        let out = apply_bond_and_focus_to_renewal(
            renew_decision(1),
            &driver,
            75.0,
            &contract,
            &team,
            80.0,
            TeamFocus::Dinastia,
        );
        assert_eq!(out.new_duration, Some(3));
    }

    #[test]
    fn survival_focus_caps_duration_to_one_year() {
        let driver = sample_driver(29, None);
        let contract = sample_contract(TeamRole::Numero1, 80_000.0);
        let team = sample_healthy_team();
        let out = apply_bond_and_focus_to_renewal(
            renew_decision(3),
            &driver,
            75.0,
            &contract,
            &team,
            90.0,
            TeamFocus::Sobrevivencia,
        );
        assert_eq!(out.new_duration, Some(1));
    }

    #[test]
    fn player_offer_duration_reflects_focus_and_bond() {
        // Sobrevivência (mercenário) = sempre 1 ano, mesmo com vínculo altíssimo.
        assert_eq!(player_offer_duration(TeamFocus::Sobrevivencia, 95.0), 1);
        // Vínculo nível 5+ (Símbolo/Casa) = projeto de 3 anos.
        assert_eq!(player_offer_duration(TeamFocus::MeioDeGrid, 80.0), 3);
        // Vínculo nível 3 (Confiança) = 2 anos.
        assert_eq!(player_offer_duration(TeamFocus::MeioDeGrid, 40.0), 2);
        // Time novo (sem história) mas de foco de longo prazo = 2 anos (projeto).
        assert_eq!(player_offer_duration(TeamFocus::Dinastia, 0.0), 2);
        // Time comum sem história = 1 ano.
        assert_eq!(player_offer_duration(TeamFocus::MeioDeGrid, 0.0), 1);
    }

    /// REGRESSÃO — a CATRACA. A versão antiga ancorava no próprio salário anterior
    /// (`base = contract.salario_anual`) e multiplicava por 0.90 sempre que a
    /// performance ficasse ≤ 60 — o caso da MAIORIA, porque performance é relativa.
    /// Resultado: decaimento composto que só descia, e um piloto subvalorizado
    /// jamais alcançava a categoria. Aqui o salário PERSEGUE o valor de mercado.
    #[test]
    fn renewal_lifts_an_underpaid_driver_instead_of_ratcheting_him_down() {
        let team = sample_healthy_team();
        let mut driver = sample_driver(26, None);
        driver.atributos.skill = 78.0;

        // Bem abaixo do que ele vale — o caso do piloto que subiu de categoria
        // carregando o salário velho.
        let contract = sample_contract(TeamRole::Numero1, 20_000.0);
        // Performance medíocre: exatamente o caso que a catraca antiga cortava 10%.
        let renovado = calculate_renewal_salary(&contract, 55.0, &driver, &team);

        assert!(
            renovado > 20_000.0,
            "piloto subvalorizado tem de SUBIR rumo ao mercado, não cair: {renovado:.0}"
        );

        // E converge: renovações sucessivas aproximam do alvo, sem ultrapassá-lo.
        let alvo = renewal_target_salary(&driver, &team, 55.0);
        let mut salario = 20_000.0;
        for _ in 0..6 {
            let c = sample_contract(TeamRole::Numero1, salario);
            salario = calculate_renewal_salary(&c, 55.0, &driver, &team);
            assert!(
                salario <= alvo + 1.0,
                "a convergência nunca deve ultrapassar o alvo: {salario:.0} > {alvo:.0}"
            );
        }
        assert!(
            (salario - alvo).abs() < alvo * 0.05,
            "após 6 renovações o salário deve estar colado no mercado: {salario:.0} vs alvo {alvo:.0}"
        );
    }

    /// A queda é LENTA de propósito (decisão travada): o contrato protege o piloto
    /// no curto prazo, mas o declínio acaba chegando — sem virar salário zumbi.
    #[test]
    fn renewal_cuts_an_overpaid_driver_slowly_but_really_cuts() {
        let team = sample_healthy_team();
        let mut driver = sample_driver(35, None);
        driver.atributos.skill = 40.0;

        let caro = 200_000.0;
        let contract = sample_contract(TeamRole::Numero1, caro);
        let alvo = renewal_target_salary(&driver, &team, 55.0);
        let renovado = calculate_renewal_salary(&contract, 55.0, &driver, &team);

        assert!(alvo < caro, "cenário inválido: o alvo precisa estar abaixo do salário atual");
        assert!(renovado < caro, "piloto acima do mercado precisa ceder: {renovado:.0}");
        assert!(
            renovado > alvo,
            "mas a queda é gradual — não desaba pro alvo de uma vez: {renovado:.0} vs alvo {alvo:.0}"
        );
        // Sobe mais rápido do que desce.
        assert!(
            RENEWAL_CATCHUP_DOWN < RENEWAL_CATCHUP_UP,
            "a assimetria é o coração da regra"
        );
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
