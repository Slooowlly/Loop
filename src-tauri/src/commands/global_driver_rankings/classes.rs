//! Resolução da classe de um título arquivado: snapshot, equipe, inscrição especial e contrato.

use super::*;

pub(super) fn normalized_archive_category(snapshot: &Value, fallback: String) -> String {
    let category = json_string(snapshot, "categoria")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback);
    if category.trim().is_empty() {
        "unknown".to_string()
    } else {
        category
    }
}

pub(super) fn clean_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn archived_title_class(
    conn: &Connection,
    driver_id: &str,
    category: &str,
    season_number: i32,
    snapshot: &Value,
) -> Result<Option<String>, String> {
    if let Some(class_name) = snapshot_class(snapshot) {
        return Ok(Some(class_name));
    }

    if let Some(team_id) = json_string(snapshot, "team_id").filter(|value| !value.trim().is_empty())
    {
        if let Some(class_name) = archived_team_class(conn, &team_id, category, season_number)? {
            return Ok(Some(class_name));
        }
        if let Some(class_name) =
            archived_special_entry_class(conn, &team_id, category, season_number)?
        {
            return Ok(Some(class_name));
        }
    }

    archived_contract_class(conn, driver_id, category, season_number)
}

pub(super) fn snapshot_class(snapshot: &Value) -> Option<String> {
    ["classe", "class_name", "special_class"]
        .iter()
        .find_map(|key| json_string(snapshot, key))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn archived_team_class(
    conn: &Connection,
    team_id: &str,
    category: &str,
    season_number: i32,
) -> Result<Option<String>, String> {
    if !table_exists(conn, "team_season_archive")? {
        return Ok(None);
    }

    conn.query_row(
        "SELECT classe
         FROM team_season_archive
         WHERE team_id = ?1
           AND season_number = ?2
           AND categoria = ?3
           AND classe IS NOT NULL
           AND TRIM(classe) <> ''
         LIMIT 1",
        params![team_id, season_number, category],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| format!("Falha ao buscar classe historica da equipe: {e}"))
}

pub(super) fn archived_special_entry_class(
    conn: &Connection,
    team_id: &str,
    category: &str,
    season_number: i32,
) -> Result<Option<String>, String> {
    if !table_exists(conn, "special_team_entries")? || !table_exists(conn, "seasons")? {
        return Ok(None);
    }

    conn.query_row(
        "SELECT e.class_name
         FROM special_team_entries e
         INNER JOIN seasons s ON s.id = e.season_id
         WHERE e.team_id = ?1
           AND e.special_category = ?2
           AND s.numero = ?3
           AND e.class_name IS NOT NULL
           AND TRIM(e.class_name) <> ''
         ORDER BY e.class_name
         LIMIT 1",
        params![team_id, category, season_number],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| format!("Falha ao buscar classe historica da inscricao especial: {e}"))
}

pub(super) fn archived_contract_class(
    conn: &Connection,
    driver_id: &str,
    category: &str,
    season_number: i32,
) -> Result<Option<String>, String> {
    if !table_exists(conn, "contracts")? {
        return Ok(None);
    }

    conn.query_row(
        "SELECT classe
         FROM contracts
         WHERE piloto_id = ?1
           AND categoria = ?2
           AND classe IS NOT NULL
           AND TRIM(classe) <> ''
           AND CAST(temporada_inicio AS INTEGER) <= ?3
           AND CAST(temporada_fim AS INTEGER) >= ?3
         ORDER BY tipo DESC, created_at DESC
         LIMIT 1",
        params![driver_id, category, season_number],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| format!("Falha ao buscar classe historica do contrato: {e}"))
}
