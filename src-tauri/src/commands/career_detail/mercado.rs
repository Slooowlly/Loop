//! Valor de mercado do piloto: salario estimado, valor e chance de transferencia.

use super::*;

pub(super) fn build_driver_market_block(
    driver: &Driver,
    contract: Option<&Contract>,
    team: Option<&Team>,
    current_season: i32,
) -> DriverMarketBlock {
    let category_id = resolve_driver_category(driver, contract, team);
    let base_salary = salary_baseline_for_category(category_id.as_deref());
    let skill_factor = 0.72 + driver.atributos.skill.clamp(0.0, 100.0) / 68.0;
    let career_factor = 1.0
        + driver.stats_carreira.titulos as f64 * 0.16
        + driver.stats_carreira.vitorias as f64 * 0.018
        + driver.stats_carreira.podios as f64 * 0.008;
    let media_factor = 0.9 + driver.atributos.midia.clamp(0.0, 100.0) / 500.0;
    let salario_estimado = contract
        .map(|value| value.salario_anual)
        .unwrap_or(base_salary * skill_factor * career_factor)
        .max(5_000.0)
        .round();
    let value_multiplier = 2.2 + driver.atributos.desenvolvimento.clamp(0.0, 100.0) / 70.0;
    let valor_mercado = (salario_estimado * value_multiplier * media_factor).round();
    let chance_transferencia = transfer_chance_for_driver(driver, contract, current_season);

    DriverMarketBlock {
        valor_mercado: Some(valor_mercado),
        salario_estimado: Some(salario_estimado),
        chance_transferencia: Some(chance_transferencia),
    }
}

pub(super) fn salary_baseline_for_category(category_id: Option<&str>) -> f64 {
    match category_id
        .and_then(categories::get_category_config)
        .map(|config| config.tier)
    {
        Some(0) => 10_000.0,
        Some(1) => 27_500.0,
        Some(2) => 55_000.0,
        Some(3) => 105_000.0,
        Some(4) => 200_000.0,
        Some(5) => 165_000.0,
        _ => 20_000.0,
    }
}

pub(super) fn transfer_chance_for_driver(
    driver: &Driver,
    contract: Option<&Contract>,
    current_season: i32,
) -> u8 {
    let Some(contract) = contract else {
        return 100;
    };

    let remaining = contract.anos_restantes(current_season);
    let contract_pressure = if remaining <= 0 {
        54.0
    } else if remaining == 1 {
        34.0
    } else {
        14.0
    };
    let motivation_pressure = (70.0 - driver.motivacao).max(0.0) * 0.45;
    let market_pull = (driver.atributos.skill - 60.0).max(0.0) * 0.28;

    (contract_pressure + motivation_pressure + market_pull)
        .round()
        .clamp(5.0, 95.0) as u8
}
