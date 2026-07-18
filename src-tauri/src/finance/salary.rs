use crate::constants::categories::get_category_config;
use crate::finance::planning::{calculate_financial_plan, category_finance_scale};
use crate::models::team::Team;

pub fn calculate_salary_ceiling(team: &Team) -> f64 {
    let base = category_salary_base(&team.categoria);
    let plan = calculate_financial_plan(team);
    let scale = category_finance_scale(&team.categoria);
    let spending_ratio = (plan.spending_power / scale.operating_cost_midpoint()).clamp(-0.5, 2.5);
    let reputation_factor = 0.85 + team.reputacao.clamp(0.0, 100.0) / 260.0;
    let money_factor = (0.75 + spending_ratio * 0.45).clamp(0.35, 1.85);

    (base * 2.2 * reputation_factor * money_factor).max(5_000.0)
}

pub fn calculate_offer_salary_from_money(team: &Team, driver_skill: f64) -> f64 {
    let base = category_salary_base(&team.categoria);
    let skill_modifier = (driver_skill / 70.0).clamp(0.70, 1.75);
    let ceiling = calculate_salary_ceiling(team);
    let affordability_modifier = (ceiling / (base * 2.2).max(1.0)).clamp(0.55, 1.65);
    let target = base * skill_modifier * affordability_modifier;

    target.clamp(5_000.0, ceiling).round()
}

pub fn calculate_renewal_pressure_from_money(team: &Team, current_salary: f64) -> f64 {
    let ceiling = calculate_salary_ceiling(team).max(5_000.0);
    (current_salary.max(0.0) / ceiling).max(0.0)
}

fn category_salary_base(category: &str) -> f64 {
    let tier = get_category_config(category)
        .map(|config| config.tier)
        .unwrap_or(0);
    category_salary_base_for_tier(tier)
}

/// Fração do custo operacional que a FOLHA de uma equipe (2 pilotos) deve representar.
/// Peso CONSTANTE em toda a escada (decisão travada com o user): o salário pesa o mesmo
/// no rookie e no endurance, em vez de comprimir de ~15% pra ~5% subindo. 0.15 ≈ o nível
/// que o rookie/amador já tinham, então o piso não muda e o topo sobe pra alcançá-lo.
/// TUNÁVEL: é o único botão da magnitude salarial da grade — calibrado por Monte Carlo.
/// Pode ser sobrescrito por `IRACER_SALARY_SHARE` (lido uma vez) para varreduras de MC.
const DEFAULT_TEAM_SALARY_SHARE_OF_OPERATING: f64 = 0.15;

pub(crate) fn team_salary_share_of_operating() -> f64 {
    static SHARE: std::sync::LazyLock<f64> = std::sync::LazyLock::new(|| {
        std::env::var("IRACER_SALARY_SHARE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| *v > 0.0 && *v < 1.0)
            .unwrap_or(DEFAULT_TEAM_SALARY_SHARE_OF_OPERATING)
    });
    *SHARE
}

/// Soma dos multiplicadores de papel de uma dupla (N1 ~1.30 + N2 ~1.06), usada em
/// `models::contract` na geração inicial. Converte "folha da equipe" em "base por piloto":
/// `base × ROLE_SUM ≈ folha`, de modo que `folha ≈ SHARE × custo operacional`.
const PAYROLL_ROLE_SUM: f64 = 2.36;

/// Base salarial de mercado por tier (ponto médio, por piloto). DERIVADA do custo
/// operacional do tier (`planning::operating_cost_midpoint_for_tier`) — não é mais uma
/// tabela escrita à mão em paralelo à de custos, então as duas escadas não podem
/// divergir. FONTE ÚNICA da escala por tier: a faixa de geração de contratos
/// (`contract::salary_range_for_tier`) deve sempre abraçar estes valores (garantido
/// pelo teste de acoplamento em `contract`).
pub(crate) fn category_salary_base_for_tier(tier: u8) -> f64 {
    let op_mid = crate::finance::planning::operating_cost_midpoint_for_tier(tier);
    op_mid * team_salary_share_of_operating() / PAYROLL_ROLE_SUM
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::team::{placeholder_team_from_db, Team};

    fn sample_team(category: &str, cash: f64, debt: f64, state: &str) -> Team {
        let mut team = placeholder_team_from_db(
            "TSAL".to_string(),
            "Equipe Salario".to_string(),
            category.to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = cash;
        team.debt_balance = debt;
        team.financial_state = state.to_string();
        team.reputacao = 55.0;
        team
    }

    #[test]
    fn salary_ceiling_is_higher_for_rich_team_than_indebted_team() {
        let rich = sample_team("gt3", 15_000_000.0, 0.0, "healthy");
        let poor = sample_team("gt3", 500_000.0, 8_000_000.0, "crisis");

        assert!(calculate_salary_ceiling(&rich) > calculate_salary_ceiling(&poor));
    }

    #[test]
    fn salary_offer_ignores_legacy_budget_field() {
        let mut team = sample_team("gt4", 6_000_000.0, 0.0, "healthy");
        team.budget = 1.0;

        let offer = calculate_offer_salary_from_money(&team, 80.0);

        assert!(offer > 50_000.0);
    }

    #[test]
    fn salary_offer_has_safe_floor() {
        let team = sample_team("mazda_rookie", -100_000.0, 900_000.0, "collapse");

        assert!(calculate_offer_salary_from_money(&team, 5.0) >= 5_000.0);
    }

    #[test]
    fn higher_category_still_has_higher_salary_ceiling() {
        let rookie = sample_team("mazda_rookie", 600_000.0, 0.0, "healthy");
        let gt3 = sample_team("gt3", 15_000_000.0, 0.0, "healthy");

        assert!(calculate_salary_ceiling(&gt3) > calculate_salary_ceiling(&rookie));
    }
}
