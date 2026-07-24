//! Rótulos e listas de categorias: agregado do piloto, categoria regular e escada inferida.

use super::*;

/// Agregado de carreira do piloto. NÃO é histórico confiável por si só: soma o bloco
/// carimbado no nascimento (`seed_initial_career_history`) ao que ele correu. Serve
/// de rótulo de categoria e de último recurso em save sem resultado gravado.
/// Carimba o rótulo de categoria num agregado que não tem um.
pub(super) fn labelled_stats(stats: CategoryStats, category: Option<&str>) -> CategoryStats {
    let (category, class_name) = category_stats_parts(category.unwrap_or("unknown"));
    CategoryStats {
        category,
        class_name,
        ..stats
    }
}

pub(super) fn stats_from_driver(driver: &Driver, category: Option<&str>) -> CategoryStats {
    let (category, class_name) = category_stats_parts(category.unwrap_or("unknown"));
    CategoryStats {
        category,
        class_name,
        points: driver.stats_carreira.pontos_total,
        wins: driver.stats_carreira.vitorias as i32,
        podiums: driver.stats_carreira.podios as i32,
        poles: driver.stats_carreira.poles as i32,
        races: driver.stats_carreira.corridas as i32,
        titles: driver.stats_carreira.titulos as i32,
        title_years: Vec::new(),
        dnfs: driver.stats_carreira.dnfs as i32,
    }
}

pub(super) fn regular_category(category: Option<&str>, class_name: Option<&str>) -> Option<String> {
    let category = category?.trim();
    if category.is_empty() {
        return None;
    }
    if let Some((base_category, key_class_name)) = category.split_once(':') {
        if is_valid_competitive_division(base_category, Some(key_class_name)) {
            return Some(competitive_division_key(
                base_category,
                Some(key_class_name),
            ));
        }
        return None;
    }
    if is_valid_competitive_division(category, class_name) {
        Some(competitive_division_key(category, class_name))
    } else {
        None
    }
}

pub(super) fn category_stats_parts(category: &str) -> (String, Option<String>) {
    let category = category.trim();
    if let Some((base_category, class_name)) = category.split_once(':') {
        let class_name = class_name.trim();
        if is_valid_competitive_division(base_category, Some(class_name)) {
            return (
                base_category.trim().to_string(),
                Some(class_name.to_string()),
            );
        }
    }
    (category.to_string(), None)
}

pub(super) fn load_contract_categories(conn: &Connection, driver_id: &str) -> Result<Vec<String>, String> {
    let contracts = contract_queries::get_contracts_for_pilot(conn, driver_id)
        .map_err(|e| format!("Falha ao carregar historico de contratos do piloto: {e}"))?;
    let mut categories = Vec::new();
    for contract in contracts {
        if let Some(category) =
            regular_category(Some(&contract.categoria), contract.classe.as_deref())
        {
            push_category(&mut categories, &category);
        }
    }
    Ok(categories)
}

pub(super) fn historical_categories(
    stats: &[CategoryStats],
    fallback_category: Option<&str>,
    extra_categories: &[String],
) -> Vec<String> {
    let mut categories = Vec::new();
    for category in stats
        .iter()
        .map(|entry| entry.category.as_str())
        .chain(fallback_category.into_iter())
        .chain(extra_categories.iter().map(String::as_str))
    {
        push_category(&mut categories, category);
    }
    categories
}

pub(super) fn push_category(categories: &mut Vec<String>, category: &str) {
    let category = category.trim();
    if category.is_empty() || category == "unknown" {
        return;
    }
    if !categories.iter().any(|value| value == category) {
        categories.push(category.to_string());
    }
}

pub(super) fn inferred_foundation_categories(
    driver: &Driver,
    current_category: Option<&str>,
    stats: &[CategoryStats],
) -> Vec<String> {
    let Some(current_category) = current_category else {
        return Vec::new();
    };
    if current_category == "mazda_rookie" || current_category == "toyota_rookie" {
        return Vec::new();
    }
    if stats.iter().any(|entry| {
        let category = entry.category.as_str();
        category != current_category && category != "unknown"
    }) {
        return Vec::new();
    }
    if driver.stats_carreira.corridas == 0 && driver.stats_carreira.temporadas == 0 {
        return Vec::new();
    }

    inferred_ladder_for_category(&driver.id, current_category)
}

pub(super) fn inferred_ladder_for_category(driver_id: &str, category: &str) -> Vec<String> {
    match category {
        "mazda_amador" => vec!["mazda_rookie".to_string()],
        "toyota_amador" => vec!["toyota_rookie".to_string()],
        "bmw_m2" => branded_foundation(driver_id),
        "production_challenger" => match stable_bucket(driver_id, 3) {
            0 => vec!["mazda_rookie".to_string(), "mazda_amador".to_string()],
            1 => vec!["toyota_rookie".to_string(), "toyota_amador".to_string()],
            _ => {
                let mut values = branded_foundation(driver_id);
                values.push("bmw_m2".to_string());
                values
            }
        },
        "gt4" => inferred_gt4_foundation(driver_id),
        "gt3" => {
            let mut ladder = inferred_gt4_foundation(driver_id);
            ladder.push("gt4".to_string());
            ladder
        }
        "lmp2" => {
            let mut ladder = inferred_gt4_foundation(driver_id);
            ladder.push("gt4".to_string());
            ladder.push("gt3".to_string());
            ladder
        }
        "endurance" => {
            let mut ladder = inferred_ladder_for_category(driver_id, "lmp2");
            ladder.push("lmp2".to_string());
            ladder
        }
        other => get_feeder_categories(other)
            .into_iter()
            .map(str::to_string)
            .collect(),
    }
}

pub(super) fn inferred_gt4_foundation(driver_id: &str) -> Vec<String> {
    match stable_bucket(driver_id, 4) {
        0 => vec!["mazda_rookie".to_string(), "mazda_amador".to_string()],
        1 => vec!["toyota_rookie".to_string(), "toyota_amador".to_string()],
        2 => {
            let mut values = branded_foundation(driver_id);
            values.push("bmw_m2".to_string());
            values
        }
        _ => {
            let mut values = inferred_ladder_for_category(driver_id, "production_challenger");
            values.push("production_challenger".to_string());
            values
        }
    }
}

pub(super) fn branded_foundation(driver_id: &str) -> Vec<String> {
    if stable_bucket(driver_id, 2) == 0 {
        vec!["mazda_rookie".to_string(), "mazda_amador".to_string()]
    } else {
        vec!["toyota_rookie".to_string(), "toyota_amador".to_string()]
    }
}

pub(super) fn stable_bucket(value: &str, buckets: usize) -> usize {
    if buckets == 0 {
        return 0;
    }
    value.bytes().fold(0_usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    }) % buckets
}
