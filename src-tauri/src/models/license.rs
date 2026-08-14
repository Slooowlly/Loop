//! A REGRA da escada de licenças, e só ela: qual nível cada divisão exige e qual ela
//! concede.
//!
//! Este arquivo é PURO — nada de `rusqlite` aqui. O SQL mudou-se para
//! [`crate::db::queries::licenses`] e as fachadas que combinavam regra com persistência
//! (`driver_has_required_license_*`, `ensure_driver_can_join_*`, `grant_*_if_needed`,
//! `repair_missing_licenses_*`) mudaram-se para [`crate::licensing`]. Model declara,
//! `db/queries` grava, `licensing` decide. O que sobrou aqui são as decisões de domínio
//! (`endurance/lmp2` exige 5), exercitáveis sem abrir banco.

use crate::constants::categories::{get_category_config, is_valid_competitive_division};

/// Licença mínima exigida pela divisão competitiva composta `categoria + classe`.
///
/// Fonte de verdade da escada da Fase 9C. Retorna:
/// - `Some(nivel)` quando a divisão é válida e exige licença;
/// - `None` quando a divisão é válida mas não exige licença (Rookie) **ou**
///   quando a divisão é inválida no fluxo novo (ex.: `lmp2` standalone,
///   `endurance`/`production_challenger` sem classe). Para distinguir os dois
///   casos use [`is_valid_competitive_division`].
pub fn required_license_for_division(category_id: &str, class_id: Option<&str>) -> Option<u8> {
    let class = class_id.map(str::trim).filter(|value| !value.is_empty());
    if !is_valid_competitive_division(category_id, class) {
        return None;
    }
    match (category_id, class) {
        // Endurance é meta-categoria: cada classe é uma divisão própria.
        ("endurance", Some("gt4")) => Some(3),
        ("endurance", Some("gt3")) => Some(4),
        ("endurance", Some("lmp2")) => Some(5),
        // Demais divisões (regulares e Production) seguem a licença da categoria.
        _ => get_category_config(category_id).and_then(|config| config.licenca_necessaria),
    }
}

/// Licença concedida pela divisão composta `categoria + classe` à metade
/// superior da classificação no fim da temporada.
pub fn granted_license_for_division(category_id: &str, class_id: Option<&str>) -> Option<u8> {
    let class = class_id.map(str::trim).filter(|value| !value.is_empty());
    if !is_valid_competitive_division(category_id, class) {
        return None;
    }
    match (category_id, class) {
        ("endurance", Some("gt4")) => Some(4),
        ("endurance", Some("gt3")) => Some(5),
        ("endurance", Some("lmp2")) => Some(6),
        _ => get_category_config(category_id).map(|config| config.tier),
    }
}

/// Licença mínima exigida por categoria (sem classe). Mantida como wrapper de
/// compatibilidade para fluxos legados (convocação, auditoria histórica) que
/// ainda raciocinam por categoria. O fluxo novo usa
/// [`required_license_for_division`].
pub fn required_license_for_category(category_id: &str) -> Option<u8> {
    get_category_config(category_id).and_then(|config| config.licenca_necessaria)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── required_license_for_division ────────────────────────────────────────

    #[test]
    fn test_required_license_rookie_is_none() {
        assert_eq!(required_license_for_division("mazda_rookie", None), None);
        assert_eq!(required_license_for_division("toyota_rookie", None), None);
    }

    #[test]
    fn test_required_license_amador_is_zero() {
        assert_eq!(required_license_for_division("mazda_amador", None), Some(0));
    }

    #[test]
    fn test_required_license_bmw_m2_is_one() {
        assert_eq!(required_license_for_division("bmw_m2", None), Some(1));
    }

    #[test]
    fn test_required_license_production_class_is_one() {
        assert_eq!(
            required_license_for_division("production_challenger", Some("mazda")),
            Some(1)
        );
        assert_eq!(
            required_license_for_division("production_challenger", Some("toyota")),
            Some(1)
        );
        assert_eq!(
            required_license_for_division("production_challenger", Some("bmw")),
            Some(1)
        );
    }

    #[test]
    fn test_required_license_gt4_is_two() {
        assert_eq!(required_license_for_division("gt4", None), Some(2));
    }

    #[test]
    fn test_required_license_gt3_is_three() {
        assert_eq!(required_license_for_division("gt3", None), Some(3));
    }

    #[test]
    fn test_required_license_endurance_gt4_is_three() {
        assert_eq!(
            required_license_for_division("endurance", Some("gt4")),
            Some(3)
        );
    }

    #[test]
    fn test_required_license_endurance_gt3_is_four() {
        assert_eq!(
            required_license_for_division("endurance", Some("gt3")),
            Some(4)
        );
    }

    #[test]
    fn test_required_license_endurance_lmp2_is_five() {
        assert_eq!(
            required_license_for_division("endurance", Some("lmp2")),
            Some(5)
        );
    }

    #[test]
    fn test_lmp2_standalone_is_invalid_in_new_flow() {
        // Divisão inválida no fluxo novo: sem exigência consultável e sem validade.
        assert_eq!(required_license_for_division("lmp2", None), None);
        assert!(!is_valid_competitive_division("lmp2", None));
        // Meta-categorias sem classe também são inválidas.
        assert_eq!(required_license_for_division("endurance", None), None);
        assert_eq!(
            required_license_for_division("production_challenger", None),
            None
        );
    }

    // ── granted_license_for_division ─────────────────────────────────────────

    #[test]
    fn test_granted_license_for_divisions() {
        assert_eq!(
            granted_license_for_division("endurance", Some("lmp2")),
            Some(6)
        );
        assert_eq!(
            granted_license_for_division("endurance", Some("gt3")),
            Some(5)
        );
        assert_eq!(
            granted_license_for_division("endurance", Some("gt4")),
            Some(4)
        );
        assert_eq!(granted_license_for_division("gt3", None), Some(4));
        assert_eq!(granted_license_for_division("gt4", None), Some(3));
        assert_eq!(
            granted_license_for_division("production_challenger", Some("mazda")),
            Some(2)
        );
        assert_eq!(granted_license_for_division("mazda_rookie", None), Some(0));
        assert_eq!(granted_license_for_division("lmp2", None), None);
    }
}
