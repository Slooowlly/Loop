use rusqlite::Connection;

use crate::common::time::current_timestamp;
use crate::constants::categories::get_category_config;

pub fn required_license_for_category(category_id: &str) -> Option<u8> {
    get_category_config(category_id).and_then(|config| config.licenca_necessaria)
}

pub fn driver_has_required_license_level(
    conn: &Connection,
    driver_id: &str,
    required_level: u8,
) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM licenses WHERE piloto_id = ?1 AND CAST(nivel AS INTEGER) >= ?2",
            rusqlite::params![driver_id, required_level as i64],
            |row| row.get(0),
        )
        .map_err(|e| format!("Falha ao verificar licenca do piloto '{driver_id}': {e}"))?;
    Ok(count > 0)
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

    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         SELECT ?1, ?2, ?3, ?4, 0
         WHERE NOT EXISTS (
             SELECT 1 FROM licenses WHERE piloto_id = ?1 AND CAST(nivel AS INTEGER) >= ?5
         )",
        rusqlite::params![
            driver_id,
            required_level.to_string(),
            category_id,
            current_timestamp(),
            required_level as i64,
        ],
    )
    .map_err(|e| format!("Falha ao conceder licenca emergencial para '{driver_id}': {e}"))?;
    Ok(())
}

pub fn repair_missing_licenses_for_current_categories(conn: &Connection) -> Result<usize, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, categoria_atual
             FROM drivers
             WHERE status = 'Ativo' AND categoria_atual IS NOT NULL",
        )
        .map_err(|e| format!("Falha ao preparar reparo de licencas legadas: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("Falha ao ler pilotos para reparo de licencas: {e}"))?;

    let mut categorized_drivers = Vec::new();
    for row in rows {
        categorized_drivers
            .push(row.map_err(|e| format!("Falha ao mapear piloto para reparo de licencas: {e}"))?);
    }

    let mut repaired = 0;
    for (driver_id, category_id) in categorized_drivers {
        let Some(required_level) = required_license_for_category(&category_id) else {
            continue;
        };
        if driver_has_required_license_level(conn, &driver_id, required_level)? {
            continue;
        }
        grant_driver_license_for_category_if_needed(conn, &driver_id, &category_id)?;
        repaired += 1;
    }

    Ok(repaired)
}
