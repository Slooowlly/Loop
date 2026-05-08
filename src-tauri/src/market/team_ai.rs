use rand::Rng;

use crate::constants::categories::get_category_config;
use crate::finance::salary::calculate_offer_salary_from_money;
use crate::market::proposals::{
    is_real_career_debut_category, is_rookie_market_candidate, MarketProposal, ProposalStatus,
    Vacancy,
};
use crate::market::visibility::{
    derive_market_visibility_profile, MarketVisibilityProfile, MarketVisibilityTier,
};
use crate::models::driver::Driver;
use crate::models::team::placeholder_team_from_db;

#[derive(Debug, Clone)]
pub struct AvailableDriver {
    pub driver: Driver,
    pub visibility: f64,
    pub posicao_campeonato: i32,
    pub categoria_atual: String,
    pub category_tier: u8,
    /// Nível máximo de licença que o piloto possui. None = sem nenhuma licença.
    pub max_license_level: Option<u8>,
}

pub fn generate_team_proposals(
    vacancy: &Vacancy,
    available_drivers: &[AvailableDriver],
    current_season: i32,
    rng: &mut impl Rng,
) -> Vec<MarketProposal> {
    let required_license =
        get_category_config(&vacancy.categoria).and_then(|config| config.licenca_necessaria);

    let mut candidates: Vec<&AvailableDriver> = available_drivers
        .iter()
        .filter(|available| {
            let license_ok = match required_license {
                None => true,
                Some(required) => available
                    .max_license_level
                    .map_or(false, |level| level >= required),
            };
            available.visibility >= 4.0
                && available.driver.status.as_str() != "Aposentado"
                && !available.driver.is_jogador
                && available.category_tier.abs_diff(vacancy.category_tier) <= 1
                && license_ok
                && category_experience_ok(vacancy, available)
        })
        .collect();

    candidates.sort_by(|a, b| {
        // 1º: mérito esportivo principal + bônus público marginal
        // 2º: prioridade coarse por tier de visibilidade pública
        // 3º: refinamento fino dentro do mesmo tier (marketability_bias contínuo)
        let pa = derive_market_visibility_profile(a.driver.atributos.midia);
        let pb = derive_market_visibility_profile(b.driver.atributos.midia);
        candidate_score(b)
            .total_cmp(&candidate_score(a))
            .then_with(|| shortlist_public_priority(&pb).total_cmp(&shortlist_public_priority(&pa)))
            .then_with(|| proposal_attention_weight(&pb).total_cmp(&proposal_attention_weight(&pa)))
    });

    candidates
        .into_iter()
        .take(3)
        .map(|candidate| MarketProposal {
            id: format!(
                "TMP-{}-{}-{}",
                vacancy.team_id, candidate.driver.id, current_season
            ),
            equipe_id: vacancy.team_id.clone(),
            equipe_nome: vacancy.team_name.clone(),
            piloto_id: candidate.driver.id.clone(),
            piloto_nome: candidate.driver.nome.clone(),
            categoria: vacancy.categoria.clone(),
            papel: vacancy.papel_necessario.clone(),
            salario_oferecido: calculate_offer_salary(vacancy, &candidate.driver, rng),
            duracao_anos: match vacancy.category_tier {
                0..=1 => rng.gen_range(1..=2),
                2..=3 => rng.gen_range(1..=3),
                _ => rng.gen_range(2..=3),
            },
            status: ProposalStatus::Pendente,
            motivo_recusa: None,
        })
        .collect()
}

/// Bônus de apelo público de mercado para score de candidatos.
///
/// Política operacional local de seleção — traduz MarketVisibilityTier em
/// um diferencial de score controlado e auditável. Máximo: 5.0 (vs. componente
/// de skill que pode chegar a 40.0). É relevante para desempate e perfil
/// comercial, mas não substitui mérito esportivo claro.
///
/// Nesta v1, o bônus depende apenas do tier (não da posição fina dentro da
/// faixa), o que o torna previsível e semanticamente controlado.
fn market_public_visibility_bonus(profile: &MarketVisibilityProfile) -> f64 {
    match profile.tier {
        MarketVisibilityTier::Baixa => 0.0,
        MarketVisibilityTier::Relevante => 1.5,
        MarketVisibilityTier::Alta => 3.0,
        MarketVisibilityTier::Elite => 5.0,
    }
}

/// Prioridade pública secundária para ordenação da shortlist.
///
/// Critério de desempate na sort da shortlist — não é score principal.
/// Só tem efeito quando `candidate_score()` empata. Escala deliberadamente
/// menor que `market_public_visibility_bonus()` (máx 0.06 vs. máx 5.0)
/// para evitar dupla amplificação de mídia.
///
/// Semântica: entre candidatos esportivamente equivalentes, o de maior
/// apelo público sobe levemente na shortlist. Não desbloqueio — apenas ordenação.
fn shortlist_public_priority(profile: &MarketVisibilityProfile) -> f64 {
    match profile.tier {
        MarketVisibilityTier::Baixa => 0.0,
        MarketVisibilityTier::Relevante => 0.01,
        MarketVisibilityTier::Alta => 0.03,
        MarketVisibilityTier::Elite => 0.06,
    }
}

/// Peso de atenção pública para priorização final de proposta.
///
/// Critério terciário de ordenação — só atua quando `candidate_score()` e
/// `shortlist_public_priority()` já empataram (mesmo tier de visibilidade).
/// Usa `marketability_bias` (contínuo 0.0–1.0) para discriminação fina
/// dentro do mesmo tier.
///
/// Dois pilotos Elite com midia=85 e midia=99 são indistinguíveis pelo
/// tier-coarse de `shortlist_public_priority`. Este critério diferencia:
/// o de maior apelo público bruto é levemente preferido na proposta.
///
/// Distinto das camadas anteriores: não é um bônus de score nem uma
/// prioridade tier-coarse — é refinamento contínuo dentro do tier.
fn proposal_attention_weight(profile: &MarketVisibilityProfile) -> f64 {
    profile.marketability_bias
}

fn category_experience_ok(vacancy: &Vacancy, available: &AvailableDriver) -> bool {
    if is_real_career_debut_category(&vacancy.categoria) {
        let candidate_category = if available.categoria_atual.trim().is_empty() {
            available
                .driver
                .categoria_atual
                .as_deref()
                .unwrap_or_default()
        } else {
            available.categoria_atual.as_str()
        };
        return is_rookie_market_candidate(
            &vacancy.categoria,
            candidate_category,
            available.driver.stats_carreira.corridas,
            available.driver.stats_carreira.temporadas,
        );
    }

    available.driver.stats_carreira.corridas > 0
}

fn candidate_score(available: &AvailableDriver) -> f64 {
    let age_bonus = age_market_bonus(available.driver.idade);

    // Apelo público de mercado — bônus controlado, mas perceptível.
    // `calculate_visibility()` mede visibilidade esportiva (posição/vitórias/títulos).
    // Este bônus mede apelo público persistente (midia). São complementares, não equivalentes.
    let media_profile = derive_market_visibility_profile(available.driver.atributos.midia);
    let media_bonus = market_public_visibility_bonus(&media_profile);

    available.driver.atributos.skill * 0.4
        + available.driver.atributos.consistencia * 0.2
        + (available.visibility * 10.0) * 0.2
        + age_bonus
        + media_bonus
}

fn age_market_bonus(age: u32) -> f64 {
    (20.0 - f64::from(age.saturating_sub(27)) * 2.0).max(5.0)
}

fn calculate_offer_salary(vacancy: &Vacancy, driver: &Driver, rng: &mut impl Rng) -> f64 {
    let variance = rng.gen_range(0.85..=1.15);
    let team = vacancy_as_finance_team(vacancy);

    (calculate_offer_salary_from_money(&team, driver.atributos.skill) * variance).max(5_000.0)
}

fn vacancy_as_finance_team(vacancy: &Vacancy) -> crate::models::team::Team {
    let mut team = placeholder_team_from_db(
        vacancy.team_id.clone(),
        vacancy.team_name.clone(),
        vacancy.categoria.clone(),
        "2026-01-01".to_string(),
    );
    team.car_performance = vacancy.car_performance;
    team.budget = vacancy.budget;
    team.cash_balance = vacancy.cash_balance;
    team.debt_balance = vacancy.debt_balance;
    team.financial_state = vacancy.financial_state.clone();
    team.reputacao = vacancy.reputacao;
    team
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;
    use crate::finance::planning::category_finance_scale;
    use crate::models::driver::Driver;
    use crate::models::enums::TeamRole;

    #[test]
    fn test_generate_proposals_for_vacancy() {
        let vacancy = sample_vacancy(3);
        let available = vec![
            sample_available_driver("P001", "gt4", 3, 6.5, 72.0),
            sample_available_driver("P002", "bmw_m2", 2, 5.5, 68.0),
            sample_available_driver("P003", "gt3", 4, 7.0, 74.0),
        ];
        let mut rng = StdRng::seed_from_u64(1);

        let proposals = generate_team_proposals(&vacancy, &available, 2, &mut rng);

        assert!(!proposals.is_empty());
        assert!(proposals.len() <= 3);
    }

    #[test]
    fn test_proposals_do_not_offer_non_rookie_seat_to_career_debutant() {
        let vacancy = sample_vacancy(4);
        let mut debutant = sample_available_driver("P001", "gt3", 4, 7.5, 82.0);
        debutant.driver.stats_carreira.corridas = 0;
        debutant.max_license_level = Some(10);
        let mut rng = StdRng::seed_from_u64(12);

        let proposals = generate_team_proposals(&vacancy, &[debutant], 2, &mut rng);

        assert!(
            proposals.is_empty(),
            "piloto com zero corridas nao deve receber proposta direta para categoria acima de rookie"
        );
    }

    #[test]
    fn test_rookie_vacancy_prefers_entry_level_candidates_over_amateur_veterans() {
        let vacancy = sample_vacancy(0);
        let mut debutant = sample_available_driver("P001", "", 0, 6.0, 56.0);
        debutant.driver.stats_carreira.corridas = 0;
        debutant.driver.stats_carreira.temporadas = 0;
        debutant.categoria_atual = String::new();

        let mut retrying_rookie = sample_available_driver("P002", "mazda_rookie", 0, 5.0, 52.0);
        retrying_rookie.driver.stats_carreira.corridas = 8;
        retrying_rookie.driver.stats_carreira.temporadas = 1;

        let mut amateur_veteran = sample_available_driver("P003", "mazda_amador", 1, 8.0, 90.0);
        amateur_veteran.driver.stats_carreira.corridas = 30;
        amateur_veteran.driver.stats_carreira.temporadas = 3;

        let mut rng = StdRng::seed_from_u64(13);
        let proposals = generate_team_proposals(
            &vacancy,
            &[amateur_veteran, retrying_rookie, debutant],
            2,
            &mut rng,
        );

        let proposed_ids = proposals
            .iter()
            .map(|proposal| proposal.piloto_id.as_str())
            .collect::<Vec<_>>();
        assert!(proposed_ids.contains(&"P001"));
        assert!(proposed_ids.contains(&"P002"));
        assert!(
            !proposed_ids.contains(&"P003"),
            "vaga rookie nao deve chamar veterano vindo da categoria amadora: {proposed_ids:?}"
        );
    }

    #[test]
    fn test_proposals_respect_tier_limit() {
        let vacancy = sample_vacancy(2);
        let available = vec![
            sample_available_driver("P001", "mazda_amador", 1, 6.0, 65.0),
            sample_available_driver("P002", "endurance", 5, 8.0, 82.0),
        ];
        let mut rng = StdRng::seed_from_u64(2);

        let proposals = generate_team_proposals(&vacancy, &available, 2, &mut rng);

        assert!(proposals
            .iter()
            .all(|proposal| proposal.piloto_id != "P002"));
    }

    #[test]
    fn test_proposals_salary_scales_with_tier() {
        let low = sample_vacancy(2);
        let high = sample_vacancy(3);
        let available = vec![sample_available_driver("P001", "gt4", 3, 7.0, 72.0)];
        let mut rng_low = StdRng::seed_from_u64(3);
        let mut rng_high = StdRng::seed_from_u64(3);

        let low_offer = generate_team_proposals(&low, &available, 2, &mut rng_low);
        let high_offer = generate_team_proposals(&high, &available, 2, &mut rng_high);

        assert!(high_offer[0].salario_oferecido > low_offer[0].salario_oferecido);
    }

    #[test]
    fn test_proposals_salary_scales_to_top_tier() {
        let gt3 = sample_vacancy(4);
        let endurance = sample_vacancy(5);
        let available = vec![sample_available_driver("P001", "gt3", 4, 7.0, 72.0)];
        let mut rng_gt3 = StdRng::seed_from_u64(33);
        let mut rng_endurance = StdRng::seed_from_u64(33);

        let gt3_offer = generate_team_proposals(&gt3, &available, 2, &mut rng_gt3);
        let endurance_offer =
            generate_team_proposals(&endurance, &available, 2, &mut rng_endurance);

        assert!(
            endurance_offer[0].salario_oferecido > gt3_offer[0].salario_oferecido,
            "o tier maximo nao deve oferecer menos que o tier imediatamente anterior"
        );
    }

    #[test]
    fn test_proposals_salary_uses_real_money_instead_of_legacy_budget() {
        let mut rich = sample_vacancy(3);
        rich.budget = 1.0;
        rich.cash_balance = 6_000_000.0;
        rich.debt_balance = 0.0;
        rich.financial_state = "healthy".to_string();

        let mut poor = sample_vacancy(3);
        poor.budget = 99.0;
        poor.cash_balance = 100_000.0;
        poor.debt_balance = 2_500_000.0;
        poor.financial_state = "crisis".to_string();

        let available = vec![sample_available_driver("P001", "gt4", 3, 7.0, 80.0)];
        let mut rng_rich = StdRng::seed_from_u64(77);
        let mut rng_poor = StdRng::seed_from_u64(77);

        let rich_offer = generate_team_proposals(&rich, &available, 2, &mut rng_rich);
        let poor_offer = generate_team_proposals(&poor, &available, 2, &mut rng_poor);

        assert!(rich_offer[0].salario_oferecido > poor_offer[0].salario_oferecido);
    }

    fn sample_vacancy(tier: u8) -> Vacancy {
        let categoria = match tier {
            0 => "mazda_rookie",
            1 => "mazda_amador",
            2 => "bmw_m2",
            3 => "gt4",
            4 => "gt3",
            5 => "lmp2",
            _ => "endurance",
        };
        let finance_scale = category_finance_scale(categoria);

        Vacancy {
            team_id: "T001".to_string(),
            team_name: "Equipe".to_string(),
            categoria: categoria.to_string(),
            category_tier: tier,
            car_performance: 8.0,
            budget: 75.0,
            cash_balance: finance_scale.expected_cash_midpoint(),
            debt_balance: 0.0,
            financial_state: "healthy".to_string(),
            reputacao: 70.0,
            papel_necessario: TeamRole::Numero1,
            piloto_existente_id: None,
        }
    }

    fn sample_available_driver(
        id: &str,
        category: &str,
        tier: u8,
        visibility: f64,
        skill: f64,
    ) -> AvailableDriver {
        let mut driver = Driver::new(
            id.to_string(),
            format!("Piloto {id}"),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2020,
        );
        driver.atributos.skill = skill;
        driver.atributos.consistencia = 65.0;
        driver.stats_carreira.corridas = 24;
        driver.stats_carreira.temporadas = 3;
        AvailableDriver {
            driver,
            visibility,
            posicao_campeonato: 3,
            categoria_atual: category.to_string(),
            category_tier: tier,
            // nível alto para não bloquear testes que não testam licença
            max_license_level: Some(10),
        }
    }

    // ── Testes de shortlist_public_priority ──────────────────────────────────

    #[test]
    fn test_shortlist_priority_breaks_tie_by_media() {
        // Mesmo score esportivo, mídia diferente → mais midiático vem primeiro
        let vacancy = sample_vacancy(3);
        let mut low_media = sample_available_driver("P001", "gt4", 3, 6.5, 70.0);
        low_media.driver.atributos.midia = 10.0; // Baixa

        let mut high_media = sample_available_driver("P002", "gt4", 3, 6.5, 70.0);
        high_media.driver.atributos.midia = 90.0; // Elite

        let available = vec![low_media, high_media];
        let mut rng = StdRng::seed_from_u64(42);
        let proposals = generate_team_proposals(&vacancy, &available, 2, &mut rng);

        assert_eq!(proposals[0].piloto_id, "P002");
    }

    #[test]
    fn test_shortlist_priority_does_not_override_sporting_merit() {
        // Candidato A: esportivamente superior, mídia mínima
        // Candidato B: esportivamente inferior, mídia máxima
        // A deve vir primeiro independente da ordem de entrada
        let vacancy = sample_vacancy(3);
        let mut sporting_better = sample_available_driver("P001", "gt4", 3, 6.5, 80.0);
        sporting_better.driver.atributos.midia = 5.0; // Baixa

        let mut media_better = sample_available_driver("P002", "gt4", 3, 6.5, 65.0);
        media_better.driver.atributos.midia = 100.0; // Elite

        // ordem invertida na entrada para não depender de ordenação inicial
        let available = vec![media_better, sporting_better];
        let mut rng = StdRng::seed_from_u64(42);
        let proposals = generate_team_proposals(&vacancy, &available, 2, &mut rng);

        assert_eq!(proposals[0].piloto_id, "P001");
    }

    #[test]
    fn test_ineligible_driver_stays_out_despite_elite_media() {
        // Piloto com visibility < 4.0 não deve entrar, mesmo com mídia Elite
        let vacancy = sample_vacancy(3);
        let mut ineligible = sample_available_driver("P001", "gt4", 3, 3.5, 70.0);
        ineligible.driver.atributos.midia = 100.0; // Elite — não deve desbloquear

        let available = vec![ineligible];
        let mut rng = StdRng::seed_from_u64(42);
        let proposals = generate_team_proposals(&vacancy, &available, 2, &mut rng);

        assert!(proposals.is_empty());
    }

    #[test]
    fn test_shortlist_priority_monotonic() {
        let profile_baixa = derive_market_visibility_profile(10.0);
        let profile_relevante = derive_market_visibility_profile(40.0);
        let profile_alta = derive_market_visibility_profile(70.0);
        let profile_elite = derive_market_visibility_profile(90.0);

        assert!(
            shortlist_public_priority(&profile_baixa)
                < shortlist_public_priority(&profile_relevante)
        );
        assert!(
            shortlist_public_priority(&profile_relevante)
                < shortlist_public_priority(&profile_alta)
        );
        assert!(
            shortlist_public_priority(&profile_alta) < shortlist_public_priority(&profile_elite)
        );
    }

    // ── Testes de proposal_attention_weight ──────────────────────────────────

    #[test]
    fn test_proposal_attention_breaks_same_tier_tie() {
        // Dois Elite, mesmo candidate_score esportivo, midia raw diferente dentro do tier.
        // shortlist_public_priority empata (ambos Elite → 0.06).
        // proposal_attention_weight decide: bias=0.99 > bias=0.85.
        let vacancy = sample_vacancy(3);
        let mut low_raw = sample_available_driver("P001", "gt4", 3, 6.5, 70.0);
        low_raw.driver.atributos.midia = 85.0; // Elite, bias=0.85

        let mut high_raw = sample_available_driver("P002", "gt4", 3, 6.5, 70.0);
        high_raw.driver.atributos.midia = 99.0; // Elite, bias=0.99

        let available = vec![low_raw, high_raw];
        let mut rng = StdRng::seed_from_u64(42);
        let proposals = generate_team_proposals(&vacancy, &available, 2, &mut rng);

        assert_eq!(proposals[0].piloto_id, "P002");
    }

    #[test]
    fn test_proposal_attention_does_not_override_sporting_merit() {
        // Candidato A: esportivamente superior, midia mínima.
        // Candidato B: esportivamente inferior, midia máxima.
        // candidate_score decide no 1º critério — mérito esportivo domina.
        let vacancy = sample_vacancy(3);
        let mut sporting_better = sample_available_driver("P001", "gt4", 3, 6.5, 80.0);
        sporting_better.driver.atributos.midia = 5.0; // Baixa

        let mut media_better = sample_available_driver("P002", "gt4", 3, 6.5, 65.0);
        media_better.driver.atributos.midia = 100.0; // Elite máximo

        let available = vec![media_better, sporting_better];
        let mut rng = StdRng::seed_from_u64(42);
        let proposals = generate_team_proposals(&vacancy, &available, 2, &mut rng);

        assert_eq!(proposals[0].piloto_id, "P001");
    }

    #[test]
    fn test_proposal_attention_does_not_override_tier_priority() {
        // Candidato A: tier Alta (midia=80, shortlist_priority=0.03).
        // Candidato B: tier Relevante (midia=58, shortlist_priority=0.01, bias=0.58).
        // Mesmo candidate_score → 2º critério (tier) decide.
        // bias alto de B (0.58) não supera tier superior de A — 3º critério não chega a ser chamado.
        let vacancy = sample_vacancy(3);
        let mut tier_alta = sample_available_driver("P001", "gt4", 3, 6.5, 70.0);
        tier_alta.driver.atributos.midia = 80.0; // Alta

        let mut tier_relevante = sample_available_driver("P002", "gt4", 3, 6.5, 70.0);
        tier_relevante.driver.atributos.midia = 58.0; // Relevante

        let available = vec![tier_relevante, tier_alta]; // ordem invertida
        let mut rng = StdRng::seed_from_u64(42);
        let proposals = generate_team_proposals(&vacancy, &available, 2, &mut rng);

        assert_eq!(proposals[0].piloto_id, "P001");
    }

    #[test]
    fn test_proposal_attention_weight_monotonic() {
        let p0 = derive_market_visibility_profile(0.0);
        let p50 = derive_market_visibility_profile(50.0);
        let p85 = derive_market_visibility_profile(85.0);
        let p99 = derive_market_visibility_profile(99.0);

        assert!(proposal_attention_weight(&p0) < proposal_attention_weight(&p50));
        assert!(proposal_attention_weight(&p50) < proposal_attention_weight(&p85));
        assert!(proposal_attention_weight(&p85) < proposal_attention_weight(&p99));
    }

    // ── Testes semânticos de media_bonus em candidate_score ───────────────────

    #[test]
    fn test_age_bonus_declines_gradually_after_30() {
        let mut age_30 = sample_available_driver("P001", "gt4", 3, 6.5, 70.0);
        age_30.driver.atributos.midia = 10.0;
        age_30.driver.idade = 30;

        let mut age_31 = sample_available_driver("P002", "gt4", 3, 6.5, 70.0);
        age_31.driver.atributos.midia = 10.0;
        age_31.driver.idade = 31;

        let diff = candidate_score(&age_30) - candidate_score(&age_31);
        assert!(
            (1.5..=2.5).contains(&diff),
            "a queda entre 30 e 31 anos deve ser gradual, nao um penhasco: diff={diff}"
        );
    }

    #[test]
    fn test_media_bonus_breaks_tie_between_similar_candidates() {
        // Dois candidatos esportivamente idênticos, midia diferente →
        // o mais midiático deve ter score maior.
        let mut low_media = sample_available_driver("P001", "gt4", 3, 6.5, 65.0);
        low_media.driver.atributos.midia = 10.0; // Baixa → bônus 0.0

        let mut high_media = sample_available_driver("P002", "gt4", 3, 6.5, 65.0);
        high_media.driver.atributos.midia = 90.0; // Elite → bônus 5.0

        assert!(candidate_score(&high_media) > candidate_score(&low_media));
        let diff = candidate_score(&high_media) - candidate_score(&low_media);
        assert!(
            diff >= 4.5,
            "midia precisa ter impacto perceptivel no score: diff={diff}"
        );
    }

    #[test]
    fn test_media_bonus_same_tier_same_bonus() {
        // Nesta v1, bônus depende apenas do tier, não da posição fina dentro da faixa.
        // Dois pilotos no mesmo tier Elite com midia diferente → bônus idêntico.
        let mut a = sample_available_driver("P001", "gt4", 3, 6.5, 65.0);
        a.driver.atributos.midia = 85.0; // Elite (limite inferior)

        let mut b = sample_available_driver("P002", "gt4", 3, 6.5, 65.0);
        b.driver.atributos.midia = 99.0; // Elite (quase máximo)

        let diff = (candidate_score(&a) - candidate_score(&b)).abs();
        assert!(
            diff < 1e-9,
            "pilotos no mesmo tier devem ter bônus idêntico: diff={diff}"
        );
    }

    #[test]
    fn test_media_bonus_does_not_override_sporting_merit() {
        // Candidato A: skill moderadamente superior, midia mínima.
        let mut sporting_better = sample_available_driver("P001", "gt4", 3, 6.5, 80.0);
        sporting_better.driver.atributos.midia = 5.0; // Baixa → 0.0

        // Candidato B: skill inferior, midia máxima.
        let mut media_better = sample_available_driver("P002", "gt4", 3, 6.5, 64.0);
        media_better.driver.atributos.midia = 100.0; // Elite → 5.0

        // Diferença de skill (80-64)*0.4 = 6.4 > bônus máximo de mídia (5.0).
        // Diferença moderada mas clara deve superar o bônus máximo de mídia.
        assert!(
            candidate_score(&sporting_better) > candidate_score(&media_better),
            "diferença esportiva moderada deve superar bônus máximo de mídia"
        );
    }
}
