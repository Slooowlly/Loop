//! Elegibilidade de licença: onde a REGRA encontra a PERSISTÊNCIA.
//!
//! A escada de licenças mora em três camadas, e a razão da separação é que a do meio
//! estava faltando. A REGRA (qual nível cada divisão exige, qual ela concede) é decisão de
//! domínio e vive em [`crate::models::license`]. A ESCRITA e a LEITURA vivem em
//! [`crate::db::queries::licenses`]. O que combina as duas — "este piloto pode assinar
//! nesta divisão?", "conceda o que falta" — não é nem uma nem outra, e estava dentro de
//! `models/license.rs`, obrigando a camada de modelo a receber uma `Connection`.
//!
//! Model declara, `db/queries` grava, e este módulo decide. Com isso `models::license`
//! volta a ser puro: não importa `rusqlite` e dá para exercitar a escada inteira sem
//! banco.
//!
//! Toda função daqui devolve `Result<_, String>` com prosa: os chamadores são o mercado, a
//! promoção e a integridade do mundo, que registram o motivo em log ou no relatório de
//! auditoria.

use rusqlite::Connection;

use crate::constants::categories::{
    competitive_division_key, competitive_division_label, get_category_config,
    is_valid_competitive_division,
};
use crate::db::queries::licenses;
use crate::models::license::{required_license_for_category, required_license_for_division};

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

/// Reparo de licenças de saves legados. Fachada de uma linha sobre
/// [`licenses::repara_licencas_das_categorias_atuais`], que é a varredura inteira: os
/// chamadores em `market/` só querem a contagem e a mensagem de erro em prosa.
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
