//! Resolucao da divisao competitiva do piloto (categoria + classe) e o rotulo dela.

use super::*;

pub(super) fn resolve_driver_category(
    driver: &Driver,
    contract: Option<&Contract>,
    team: Option<&Team>,
) -> Option<String> {
    contract
        .and_then(|value| regular_division_key(&value.categoria, value.classe.as_deref()))
        .or_else(|| {
            team.and_then(|value| regular_division_key(&value.categoria, value.classe.as_deref()))
        })
        .or_else(|| regular_category(driver.categoria_atual.as_deref()))
}

pub(super) fn regular_category(category: Option<&str>) -> Option<String> {
    let category = category?.trim();
    if category.is_empty() {
        return None;
    }
    if let Some((base_category, class_name)) = category.split_once(':') {
        return regular_division_key(base_category, Some(class_name));
    }
    if categories::is_especial(category) {
        None
    } else {
        Some(category.to_string())
    }
}

pub(super) fn regular_division_key(category: &str, class_name: Option<&str>) -> Option<String> {
    categories::is_valid_competitive_division(category, class_name)
        .then(|| categories::competitive_division_key(category, class_name))
}

pub(super) fn competitive_division_label_from_key(key: &str) -> Option<String> {
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    if let Some((category, class_name)) = key.split_once(':') {
        return categories::is_valid_competitive_division(category, Some(class_name))
            .then(|| categories::competitive_division_label(category, Some(class_name)));
    }
    categories::get_category_config(key).map(|category| category.nome_curto.to_string())
}

