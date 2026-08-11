//! A REGRA da escada de licenças: qual nível cada divisão exige e qual ela concede.
//!
//! O SQL que este arquivo continha mudou-se para [`crate::db::queries::licenses`] — model
//! declara, `db/queries` grava, que é o padrão do resto do projeto. O que sobrou aqui são
//! as decisões (`endurance/lmp2` exige 5) e as funções de fachada que combinam regra e
//! persistência para os call sites do mercado.

use rusqlite::Connection;

use crate::constants::categories::{
    competitive_division_key, competitive_division_label, get_category_config,
    is_valid_competitive_division,
};
use crate::db::queries::licenses;

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

/// Verdadeiro se o piloto pode competir na divisão composta informada.
///
/// Divisão inválida no fluxo novo retorna `false`. Divisão válida sem exigência
/// de licença (Rookie) retorna `true`.
pub fn driver_has_required_license_for_division(
    conn: &Connection,
    driver_id: &str,
    category_id: &str,
    class_id: Option<&str>,
) -> Result<bool, String> {
    let class = class_id.map(str::trim).filter(|value| !value.is_empty());
    if !is_valid_competitive_division(category_id, class) {
        return Ok(false);
    }
    let Some(required_level) = required_license_for_division(category_id, class) else {
        return Ok(true);
    };
    driver_has_required_license_level(conn, driver_id, required_level)
}

/// Garante que o piloto possa assinar na divisão composta; erro descritivo caso
/// a divisão seja inválida ou a licença insuficiente.
pub fn ensure_driver_can_join_division(
    conn: &Connection,
    driver_id: &str,
    driver_name: &str,
    category_id: &str,
    class_id: Option<&str>,
) -> Result<(), String> {
    let class = class_id.map(str::trim).filter(|value| !value.is_empty());
    if !is_valid_competitive_division(category_id, class) {
        return Err(format!(
            "Divisao competitiva invalida '{}' para o piloto '{driver_name}'",
            competitive_division_key(category_id, class)
        ));
    }
    let Some(required_level) = required_license_for_division(category_id, class) else {
        return Ok(());
    };
    if driver_has_required_license_level(conn, driver_id, required_level)? {
        return Ok(());
    }

    let division_label = competitive_division_label(category_id, class);
    Err(format!(
        "Piloto '{driver_name}' nao possui a licenca {required_level} necessaria para {division_label}"
    ))
}

/// Concede a licença mínima da divisão composta ao piloto, se ainda não a tiver.
pub fn grant_driver_license_for_division_if_needed(
    conn: &Connection,
    driver_id: &str,
    category_id: &str,
    class_id: Option<&str>,
) -> Result<(), String> {
    let class = class_id.map(str::trim).filter(|value| !value.is_empty());
    let Some(required_level) = required_license_for_division(category_id, class) else {
        return Ok(());
    };
    if driver_has_required_license_level(conn, driver_id, required_level)? {
        return Ok(());
    }

    licenses::concede_nivel_se_faltar(
        conn,
        driver_id,
        required_level,
        &competitive_division_key(category_id, class),
    )
    .map_err(|e| format!("Falha ao conceder licenca emergencial para '{driver_id}': {e}"))?;
    Ok(())
}

/// Licença mínima exigida por categoria (sem classe). Mantida como wrapper de
/// compatibilidade para fluxos legados (convocação, auditoria histórica) que
/// ainda raciocinam por categoria. O fluxo novo usa
/// [`required_license_for_division`].
pub fn required_license_for_category(category_id: &str) -> Option<u8> {
    get_category_config(category_id).and_then(|config| config.licenca_necessaria)
}

pub fn driver_has_required_license_level(
    conn: &Connection,
    driver_id: &str,
    required_level: u8,
) -> Result<bool, String> {
    licenses::tem_nivel_minimo(conn, driver_id, required_level)
        .map_err(|e| format!("Falha ao verificar licenca do piloto '{driver_id}': {e}"))
}

pub fn driver_has_required_license_for_category(
    conn: &Connection,
    driver_id: &str,
    category_id: &str,
) -> Result<bool, String> {
    let Some(required_level) = required_license_for_category(category_id) else {
        return Ok(true);
    };
    driver_has_required_license_level(conn, driver_id, required_level)
}

/// Wrapper legado mantido por completude de API. O fluxo novo usa
/// [`ensure_driver_can_join_division`].
#[allow(dead_code)]
pub fn ensure_driver_can_join_category(
    conn: &Connection,
    driver_id: &str,
    driver_name: &str,
    category_id: &str,
) -> Result<(), String> {
    let Some(required_level) = required_license_for_category(category_id) else {
        return Ok(());
    };
    if driver_has_required_license_level(conn, driver_id, required_level)? {
        return Ok(());
    }

    let category_label = get_category_config(category_id)
        .map(|config| config.nome_curto)
        .unwrap_or(category_id);
    Err(format!(
        "Piloto '{driver_name}' nao possui a licenca {required_level} necessaria para {category_label}"
    ))
}

pub fn grant_driver_license_for_category_if_needed(
    conn: &Connection,
    driver_id: &str,
    category_id: &str,
) -> Result<(), String> {
    let Some(required_level) = required_license_for_category(category_id) else {
        return Ok(());
    };
    if driver_has_required_license_level(conn, driver_id, required_level)? {
        return Ok(());
    }

    licenses::concede_nivel_se_faltar(conn, driver_id, required_level, category_id)
        .map_err(|e| format!("Falha ao conceder licenca emergencial para '{driver_id}': {e}"))?;
    Ok(())
}

/// Reparo de licenças de saves legados.
///
/// A implementação mudou-se inteira para [`crate::db::queries::licenses`]: era um laço que
/// lia pilotos e escrevia licenças, do começo ao fim, dentro da camada de modelo. Esta
/// função continua existindo como fachada porque os quatro call sites vivem em `market/`,
/// que não é escopo desta frente — na integração eles apontam direto para
/// `db::queries::licenses::repara_licencas_das_categorias_atuais` e este wrapper sai.
pub fn repair_missing_licenses_for_current_categories(conn: &Connection) -> Result<usize, String> {
    licenses::repara_licencas_das_categorias_atuais(conn)
        .map_err(|e| format!("Falha ao reparar licencas legadas: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        conn
    }

    fn insert_driver_fixture(conn: &Connection, driver_id: &str) {
        let driver = crate::models::driver::Driver::new(
            driver_id.to_string(),
            format!("Piloto {driver_id}"),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2020,
        );
        crate::db::queries::drivers::insert_driver(conn, &driver).expect("driver fixture");
    }

    fn grant_level(conn: &Connection, driver_id: &str, level: u8) {
        insert_driver_fixture(conn, driver_id);
        conn.execute(
            "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
             VALUES (?1, ?2, 'teste', '2026-01-01T00:00:00', 1)",
            rusqlite::params![driver_id, level.to_string()],
        )
        .expect("insert license");
    }

    // ── required_license_for_division (Testes 1-10) ──────────────────────────

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

    // ── granted_license_for_division (Teste 11) ──────────────────────────────

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

    // ── driver_has_required_license_for_division (Testes 12-16) ──────────────

    #[test]
    fn test_driver_without_level5_cannot_sign_endurance_lmp2() {
        let conn = memory_db();
        grant_level(&conn, "P001", 4);
        assert!(!driver_has_required_license_for_division(
            &conn,
            "P001",
            "endurance",
            Some("lmp2")
        )
        .expect("query"));
        assert!(ensure_driver_can_join_division(
            &conn,
            "P001",
            "Piloto",
            "endurance",
            Some("lmp2")
        )
        .is_err());
    }

    #[test]
    fn test_driver_with_level5_can_sign_endurance_lmp2() {
        let conn = memory_db();
        grant_level(&conn, "P001", 5);
        assert!(
            driver_has_required_license_for_division(&conn, "P001", "endurance", Some("lmp2"))
                .expect("query")
        );
        assert!(ensure_driver_can_join_division(
            &conn,
            "P001",
            "Piloto",
            "endurance",
            Some("lmp2")
        )
        .is_ok());
    }

    #[test]
    fn test_driver_with_level4_can_sign_endurance_gt3() {
        let conn = memory_db();
        grant_level(&conn, "P001", 4);
        assert!(
            driver_has_required_license_for_division(&conn, "P001", "endurance", Some("gt3"))
                .expect("query")
        );
    }

    #[test]
    fn test_driver_with_level3_cannot_sign_endurance_gt3() {
        let conn = memory_db();
        grant_level(&conn, "P001", 3);
        assert!(
            !driver_has_required_license_for_division(&conn, "P001", "endurance", Some("gt3"))
                .expect("query")
        );
    }

    #[test]
    fn test_driver_with_level3_can_sign_endurance_gt4() {
        let conn = memory_db();
        grant_level(&conn, "P001", 3);
        assert!(
            driver_has_required_license_for_division(&conn, "P001", "endurance", Some("gt4"))
                .expect("query")
        );
    }

    #[test]
    fn test_grant_division_license_grants_required_level() {
        let conn = memory_db();
        insert_driver_fixture(&conn, "P001");
        grant_driver_license_for_division_if_needed(&conn, "P001", "endurance", Some("lmp2"))
            .expect("grant");
        assert!(
            driver_has_required_license_for_division(&conn, "P001", "endurance", Some("lmp2"))
                .expect("query")
        );
    }
}
