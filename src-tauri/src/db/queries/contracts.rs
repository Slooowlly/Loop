#![allow(dead_code)]

use rusqlite::{params, types::ValueRef, Connection, OptionalExtension};

use crate::constants::categories::get_category_config;
use crate::db::connection::DbError;
use crate::models::contract::Contract;
use crate::models::enums::{ContractStatus, ContractType, TeamRole};

pub fn insert_contract(conn: &Connection, contract: &Contract) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO contracts (
            id, piloto_id, piloto_nome, equipe_id, equipe_nome,
            temporada_inicio, duracao_anos, temporada_fim,
            salario, salario_anual, papel, status, tipo, categoria, classe, created_at
        ) VALUES (
            :id, :piloto_id, :piloto_nome, :equipe_id, :equipe_nome,
            :temporada_inicio, :duracao_anos, :temporada_fim,
            :salario, :salario_anual, :papel, :status, :tipo, :categoria, :classe, :created_at
        )",
        rusqlite::named_params! {
            ":id": &contract.id,
            ":piloto_id": &contract.piloto_id,
            ":piloto_nome": &contract.piloto_nome,
            ":equipe_id": &contract.equipe_id,
            ":equipe_nome": &contract.equipe_nome,
            ":temporada_inicio": contract.temporada_inicio,
            ":duracao_anos": contract.duracao_anos,
            ":temporada_fim": contract.temporada_fim,
            ":salario": contract.salario_anual,
            ":salario_anual": contract.salario_anual,
            ":papel": contract.papel.as_str(),
            ":status": contract.status.as_str(),
            ":tipo": contract.tipo.as_str(),
            ":categoria": &contract.categoria,
            ":classe": &contract.classe,
            ":created_at": &contract.created_at,
        },
    )?;
    Ok(())
}

pub fn insert_contracts(conn: &Connection, contracts: &[Contract]) -> Result<(), DbError> {
    for contract in contracts {
        insert_contract(conn, contract)?;
    }
    Ok(())
}

pub fn get_contract_by_id(conn: &Connection, id: &str) -> Result<Option<Contract>, DbError> {
    let mut stmt = conn.prepare("SELECT * FROM contracts WHERE id = ?1")?;
    let contract = stmt.query_row(params![id], contract_from_row).optional()?;
    Ok(contract)
}

/// Retorna o contrato ativo mais recente para o piloto (qualquer tipo).
/// ATENÇÃO: com dual contrato (Regular + Especial), esta função pode retornar
/// qualquer um dos dois. Para semântica precisa, use
/// `get_active_regular_contract_for_pilot` ou `get_active_especial_contract_for_pilot`.
pub fn get_active_contract_for_pilot(
    conn: &Connection,
    piloto_id: &str,
) -> Result<Option<Contract>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM contracts
         WHERE piloto_id = ?1 AND status = 'Ativo'
         ORDER BY temporada_inicio DESC, created_at DESC
         LIMIT 1",
    )?;
    let contract = stmt
        .query_row(params![piloto_id], contract_from_row)
        .optional()?;
    Ok(contract)
}

pub fn get_contracts_for_pilot(
    conn: &Connection,
    piloto_id: &str,
) -> Result<Vec<Contract>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM contracts
         WHERE piloto_id = ?1
         ORDER BY temporada_inicio DESC, created_at DESC",
    )?;
    let mapped = stmt.query_map(params![piloto_id], contract_from_row)?;
    collect_contracts(mapped)
}

pub fn get_active_contracts_for_team(
    conn: &Connection,
    equipe_id: &str,
) -> Result<Vec<Contract>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM contracts
         WHERE equipe_id = ?1 AND status = 'Ativo'
         ORDER BY papel ASC, piloto_nome ASC",
    )?;
    let mapped = stmt.query_map(params![equipe_id], contract_from_row)?;
    collect_contracts(mapped)
}

pub fn get_all_active_contracts(conn: &Connection) -> Result<Vec<Contract>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM contracts
         WHERE status = 'Ativo'
         ORDER BY categoria, equipe_nome, piloto_nome",
    )?;
    let mapped = stmt.query_map([], contract_from_row)?;
    collect_contracts(mapped)
}

pub fn get_all_active_regular_contracts(conn: &Connection) -> Result<Vec<Contract>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM contracts
         WHERE status = 'Ativo' AND tipo = 'Regular'
         ORDER BY categoria, equipe_nome, piloto_nome",
    )?;
    let mapped = stmt.query_map([], contract_from_row)?;
    collect_contracts(mapped)
}

pub fn get_active_especial_contracts_by_category(
    conn: &Connection,
    categoria: &str,
) -> Result<Vec<Contract>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM contracts
         WHERE status = 'Ativo' AND tipo = 'Especial' AND categoria = ?1
         ORDER BY equipe_nome, papel ASC, piloto_nome",
    )?;
    let mapped = stmt.query_map(params![categoria], contract_from_row)?;
    collect_contracts(mapped)
}

pub fn get_expiring_contracts(conn: &Connection, temporada: i32) -> Result<Vec<Contract>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM contracts
         WHERE status = 'Ativo' AND CAST(temporada_fim AS INTEGER) = ?1
         ORDER BY categoria, equipe_nome, piloto_nome",
    )?;
    let mapped = stmt.query_map(params![temporada], contract_from_row)?;
    collect_contracts(mapped)
}

pub fn get_contracts_by_category(
    conn: &Connection,
    categoria: &str,
) -> Result<Vec<Contract>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM contracts
         WHERE categoria = ?1
         ORDER BY equipe_nome, piloto_nome",
    )?;
    let mapped = stmt.query_map(params![categoria], contract_from_row)?;
    collect_contracts(mapped)
}

pub fn update_contract_status(
    conn: &Connection,
    id: &str,
    status: &ContractStatus,
) -> Result<(), DbError> {
    let affected = conn.execute(
        "UPDATE contracts SET status = ?1 WHERE id = ?2",
        params![status.as_str(), id],
    )?;
    if affected == 0 {
        return Err(DbError::NotFound(format!(
            "Contrato '{id}' nao encontrado para atualizar status"
        )));
    }
    Ok(())
}

/// Expira todos os contratos Especial ativos da temporada indicada.
/// Chamado durante PosEspecial — nenhum contrato Especial deve sobreviver ao bloco.
///
/// Filtra por `temporada_inicio = season_number` para precisão semântica e proteção
/// contra bugs futuros. No modelo atual só existe um ciclo especial ativo por vez,
/// portanto o resultado seria idêntico sem o filtro.
pub fn expire_especial_contracts(conn: &Connection, season_number: i32) -> Result<usize, DbError> {
    let n = conn.execute(
        "UPDATE contracts SET status = 'Expirado'
         WHERE tipo = 'Especial' AND status = 'Ativo' AND temporada_inicio = ?1",
        params![season_number],
    )?;
    Ok(n)
}

pub fn expire_ending_contracts(conn: &Connection, temporada_atual: i32) -> Result<i32, DbError> {
    let updated = conn.execute(
        "UPDATE contracts
         SET status = 'Expirado'
         WHERE status = 'Ativo' AND CAST(temporada_fim AS INTEGER) <= ?1",
        params![temporada_atual],
    )?;
    Ok(updated as i32)
}

pub fn count_active_contracts_for_team(conn: &Connection, equipe_id: &str) -> Result<i32, DbError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM contracts WHERE equipe_id = ?1 AND status = 'Ativo'",
        params![equipe_id],
        |row| row.get(0),
    )?;
    Ok(count as i32)
}

pub fn get_free_pilots(conn: &Connection) -> Result<Vec<String>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT d.id
         FROM drivers d
         LEFT JOIN contracts c
           ON c.piloto_id = d.id AND c.status = 'Ativo'
         WHERE c.id IS NULL
         ORDER BY d.nome",
    )?;

    let mapped = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut pilots = Vec::new();
    for row in mapped {
        pilots.push(row?);
    }
    Ok(pilots)
}

/// Retorna true se o piloto já possui um contrato Especial ativo.
pub fn has_active_especial_contract(conn: &Connection, piloto_id: &str) -> Result<bool, DbError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM contracts
         WHERE piloto_id = ?1 AND status = 'Ativo' AND tipo = 'Especial'",
        params![piloto_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Retorna true se o piloto já possui um contrato Regular ativo.
pub fn has_active_regular_contract(conn: &Connection, piloto_id: &str) -> Result<bool, DbError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM contracts
         WHERE piloto_id = ?1 AND status = 'Ativo' AND tipo = 'Regular'",
        params![piloto_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Retorna o contrato Regular ativo do piloto, se houver.
pub fn get_active_regular_contract_for_pilot(
    conn: &Connection,
    piloto_id: &str,
) -> Result<Option<Contract>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM contracts
         WHERE piloto_id = ?1 AND status = 'Ativo' AND tipo = 'Regular'
         ORDER BY temporada_inicio DESC, created_at DESC
         LIMIT 1",
    )?;
    let contract = stmt
        .query_row(params![piloto_id], contract_from_row)
        .optional()?;
    Ok(contract)
}

/// Retorna o contrato Especial ativo do piloto, se houver.
pub fn get_active_especial_contract_for_pilot(
    conn: &Connection,
    piloto_id: &str,
) -> Result<Option<Contract>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM contracts
         WHERE piloto_id = ?1 AND status = 'Ativo' AND tipo = 'Especial'
         ORDER BY temporada_inicio DESC, created_at DESC
         LIMIT 1",
    )?;
    let contract = stmt
        .query_row(params![piloto_id], contract_from_row)
        .optional()?;
    Ok(contract)
}

/// Pilotos com contrato Regular ativo e sem contrato Especial ativo.
/// Representa elegibilidade mínima para convocação especial.
/// A seleção final (score, classe, wildcards) é responsabilidade dos Passos 6+.
pub fn get_pilots_available_for_especial(conn: &Connection) -> Result<Vec<String>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT d.id
         FROM drivers d
         INNER JOIN contracts c_reg
           ON c_reg.piloto_id = d.id AND c_reg.status = 'Ativo' AND c_reg.tipo = 'Regular'
         LEFT JOIN contracts c_esp
           ON c_esp.piloto_id = d.id AND c_esp.status = 'Ativo' AND c_esp.tipo = 'Especial'
         WHERE c_esp.id IS NULL
         ORDER BY d.nome",
    )?;
    let mapped = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut pilots = Vec::new();
    for row in mapped {
        pilots.push(row?);
    }
    Ok(pilots)
}

/// Retorna IDs de pilotos que já tiveram contrato Especial numa categoria+classe específica.
/// Usado para montar a Fonte B (ContinuidadeHistorica) da convocação especial.
pub fn get_pilots_with_especial_history(
    conn: &Connection,
    special_category: &str,
    class_name: &str,
) -> Result<Vec<String>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT piloto_id FROM contracts
         WHERE tipo = 'Especial' AND categoria = ?1 AND classe = ?2
         ORDER BY piloto_id",
    )?;
    let mapped = stmt.query_map(params![special_category, class_name], |row| {
        row.get::<_, String>(0)
    })?;
    let mut pilots = Vec::new();
    for row in mapped {
        pilots.push(row?);
    }
    Ok(pilots)
}

/// Contagem de contratos especiais anteriores de um piloto em categoria+classe.
/// Usado no cálculo do score da Fonte B.
pub fn get_especial_contract_count(
    conn: &Connection,
    piloto_id: &str,
    special_category: &str,
    class_name: &str,
) -> Result<u32, DbError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM contracts
         WHERE piloto_id = ?1 AND tipo = 'Especial' AND categoria = ?2 AND classe = ?3",
        params![piloto_id, special_category, class_name],
        |row| row.get(0),
    )?;
    Ok(count as u32)
}

/// Gera um contrato especial sazonal.
/// tipo = Especial, duracao_anos = 1 (placeholder: válido até fim do BlocoEspecial).
/// Salário ≈ 50% do range regular do tier correspondente.
/// O pipeline de encerramento do bloco especial expirará esses contratos explicitamente.
pub fn generate_especial_contract(
    id: String,
    piloto_id: &str,
    piloto_nome: &str,
    equipe_id: &str,
    equipe_nome: &str,
    papel: TeamRole,
    categoria: &str,
    classe: &str,
    temporada: i32,
) -> Contract {
    let tier = get_category_config(categoria).map(|c| c.tier).unwrap_or(2);
    let salario_anual = salary_midpoint_for_tier(tier) * 0.5;
    let mut contract = Contract::new(
        id,
        piloto_id.to_string(),
        piloto_nome.to_string(),
        equipe_id.to_string(),
        equipe_nome.to_string(),
        temporada,
        1,
        salario_anual,
        papel,
        categoria.to_string(),
    );
    contract.tipo = ContractType::Especial;
    contract.classe = Some(classe.to_string());
    contract
}

fn salary_midpoint_for_tier(tier: u8) -> f64 {
    match tier {
        0 => 10_000.0,
        1 => 27_500.0,
        2 => 55_000.0,
        3 => 105_000.0,
        4 => 200_000.0,
        5 => 165_000.0,
        _ => 10_000.0,
    }
}

pub fn delete_contract(conn: &Connection, id: &str) -> Result<(), DbError> {
    conn.execute("DELETE FROM contracts WHERE id = ?1", params![id])?;
    Ok(())
}

pub struct FreeAgentRaw {
    pub driver_id: String,
    pub driver_name: String,
    pub categoria: String,
    pub is_rookie: bool,
    pub previous_team_name: Option<String>,
    pub previous_team_color: Option<String>,
    pub seasons_at_last_team: i32,
    pub total_career_seasons: i32,
    pub max_license_level: Option<u8>,
    pub last_championship_position: Option<i32>,
    pub last_championship_total_drivers: Option<i32>,
}

/// Retorna pilotos ativos sem contrato Regular ativo, com dados do último time e contagem de temporadas.
/// `is_rookie = true` para pilotos que nunca tiveram contrato algum.
/// A categoria exibida vem do campo `categoria` do último contrato expirado/rescindido,
/// pois `drivers.categoria_atual` costuma estar NULL para pilotos IA.
pub fn get_free_agents_for_preseason(conn: &Connection) -> Result<Vec<FreeAgentRaw>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT
             d.id   AS driver_id,
             d.nome AS driver_name,
             COALESCE(
                 (SELECT c.categoria
                  FROM contracts c
                  WHERE c.piloto_id = d.id
                    AND c.tipo = 'Regular'
                    AND c.status IN ('Expirado', 'Rescindido')
                  ORDER BY CAST(c.temporada_fim AS INTEGER) DESC, c.created_at DESC
                  LIMIT 1),
                 NULLIF(d.categoria_atual, '')
             ) AS categoria,
             CASE
                 WHEN EXISTS (
                     SELECT 1 FROM contracts
                     WHERE piloto_id = d.id AND tipo = 'Regular'
                 ) THEN 0
                 ELSE 1
             END
                 AS is_rookie,
             (SELECT c.equipe_nome
              FROM contracts c
              WHERE c.piloto_id = d.id
                AND c.tipo = 'Regular'
                AND c.status IN ('Expirado', 'Rescindido')
              ORDER BY CAST(c.temporada_fim AS INTEGER) DESC, c.created_at DESC
              LIMIT 1) AS prev_team_name,
             (SELECT e.cor_primaria
              FROM contracts c
              JOIN teams e ON e.id = c.equipe_id
              WHERE c.piloto_id = d.id
                AND c.tipo = 'Regular'
                AND c.status IN ('Expirado', 'Rescindido')
              ORDER BY CAST(c.temporada_fim AS INTEGER) DESC, c.created_at DESC
              LIMIT 1) AS prev_team_color,
             (SELECT COALESCE(SUM(c2.duracao_anos), 0)
              FROM contracts c2
              WHERE c2.piloto_id = d.id
                AND c2.tipo = 'Regular'
                AND c2.status IN ('Expirado', 'Rescindido')
                AND c2.equipe_id = (
                    SELECT c.equipe_id
                    FROM contracts c
                    WHERE c.piloto_id = d.id
                      AND c.tipo = 'Regular'
                      AND c.status IN ('Expirado', 'Rescindido')
                    ORDER BY CAST(c.temporada_fim AS INTEGER) DESC, c.created_at DESC
                    LIMIT 1
                )
             ) AS seasons_at_team,
             (SELECT COALESCE(SUM(duracao_anos), 0)
              FROM contracts
              WHERE piloto_id = d.id
                AND tipo = 'Regular') AS career_seasons,
             (SELECT MAX(CAST(nivel AS INTEGER))
              FROM licenses
              WHERE piloto_id = d.id) AS max_license
         FROM drivers d
         WHERE NOT EXISTS (
             SELECT 1 FROM contracts c
             WHERE c.piloto_id = d.id
               AND c.status = 'Ativo'
               AND c.tipo = 'Regular'
         )
           AND d.status = 'Ativo'
           AND d.is_jogador = 0
         ORDER BY categoria, is_rookie ASC, d.nome",
    )?;

    let mut result = Vec::new();
    let mapped = stmt.query_map([], |row| {
        let is_rookie_int: i32 = row.get("is_rookie")?;
        let max_license_raw: Option<i64> = row.get("max_license")?;
        Ok(FreeAgentRaw {
            driver_id: row.get("driver_id")?,
            driver_name: row.get("driver_name")?,
            categoria: row
                .get::<_, Option<String>>("categoria")?
                .unwrap_or_default(),
            is_rookie: is_rookie_int != 0,
            previous_team_name: row.get("prev_team_name")?,
            previous_team_color: row.get("prev_team_color")?,
            seasons_at_last_team: row.get("seasons_at_team")?,
            total_career_seasons: row.get("career_seasons")?,
            max_license_level: max_license_raw.map(|v| v as u8),
            last_championship_position: None,
            last_championship_total_drivers: None,
        })
    })?;
    for row in mapped {
        result.push(row?);
    }
    for agent in &mut result {
        if agent.categoria.is_empty() {
            continue;
        }
        if let Some((position, total_drivers)) =
            get_latest_driver_archive_summary(conn, &agent.driver_id, &agent.categoria)?
        {
            agent.last_championship_position = Some(position);
            agent.last_championship_total_drivers = Some(total_drivers);
        }
    }
    Ok(result)
}

fn get_latest_driver_archive_summary(
    conn: &Connection,
    pilot_id: &str,
    categoria: &str,
) -> Result<Option<(i32, i32)>, DbError> {
    let result: Result<(Option<i32>, String), _> = conn.query_row(
        "SELECT posicao_campeonato, snapshot_json
         FROM driver_season_archive
         WHERE piloto_id = ?1
           AND categoria = ?2
         ORDER BY season_number DESC
         LIMIT 1",
        params![pilot_id, categoria],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );

    match result {
        Ok((Some(position), snapshot_json)) => {
            let total_drivers = serde_json::from_str::<serde_json::Value>(&snapshot_json)
                .ok()
                .and_then(|json| json.get("total_pilotos").and_then(|value| value.as_i64()))
                .map(|value| value as i32);
            Ok(total_drivers.map(|total| (position, total)))
        }
        Ok((None, _)) => Ok(None),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DbError::Sqlite(e)),
    }
}

fn collect_contracts(
    mapped: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<Contract>>,
) -> Result<Vec<Contract>, DbError> {
    let mut result = Vec::new();
    for row in mapped {
        result.push(row?);
    }
    Ok(result)
}

fn contract_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Contract> {
    let salario_anual = optional_f64(row, "salario_anual")?
        .or_else(|| optional_f64(row, "salario").ok().flatten())
        .unwrap_or(0.0);

    // status, papel e tipo são campos obrigatórios com semântica crítica.
    // Erros de leitura (NULL, coluna ausente, valor desconhecido) devem ser
    // propagados, não silenciados em defaults que distorcem o estado do mundo.
    let status_str: String = row.get("status")?;
    let papel_str: String = row.get("papel")?;
    let tipo_str: String = row.get("tipo")?;

    Ok(Contract {
        id: row.get("id")?,
        piloto_id: row.get("piloto_id")?,
        piloto_nome: optional_string(row, "piloto_nome")?.unwrap_or_default(),
        equipe_id: row.get("equipe_id")?,
        equipe_nome: optional_string(row, "equipe_nome")?.unwrap_or_default(),
        temporada_inicio: required_i32_column(row, "temporada_inicio")?,
        duracao_anos: required_i32_column(row, "duracao_anos")?,
        temporada_fim: required_i32_column(row, "temporada_fim")?,
        salario_anual,
        papel: parse_contract_role(&papel_str)?,
        status: parse_contract_status(&status_str)?,
        tipo: parse_contract_tipo(&tipo_str)?,
        categoria: optional_string(row, "categoria")?.unwrap_or_default(),
        classe: optional_string(row, "classe")?,
        created_at: optional_string(row, "created_at")?.unwrap_or_default(),
    })
}

fn invalid_text_conversion_error(context: &str, value: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{context}: '{value}'"),
        )),
    )
}

fn invalid_numeric_conversion_error(
    column_name: &str,
    sqlite_type: rusqlite::types::Type,
    detail: impl Into<String>,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        sqlite_type,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("coluna '{column_name}' invalida: {}", detail.into()),
        )),
    )
}

fn parse_contract_status(s: &str) -> rusqlite::Result<ContractStatus> {
    match s {
        "Ativo" => Ok(ContractStatus::Ativo),
        "Expirado" => Ok(ContractStatus::Expirado),
        "Rescindido" => Ok(ContractStatus::Rescindido),
        "Pendente" => Ok(ContractStatus::Pendente),
        other => Err(invalid_text_conversion_error(
            "status de contrato desconhecido",
            other,
        )),
    }
}

fn parse_contract_tipo(s: &str) -> rusqlite::Result<ContractType> {
    ContractType::from_str_strict(s)
        .map_err(|error| invalid_text_conversion_error("tipo de contrato desconhecido", &error))
}

fn parse_contract_role(s: &str) -> rusqlite::Result<TeamRole> {
    match s {
        "Numero1" | "N1" | "Titular" => Ok(TeamRole::Numero1),
        "Numero2" | "N2" | "Reserva" | "Junior" => Ok(TeamRole::Numero2),
        other => Err(invalid_text_conversion_error(
            "papel de contrato desconhecido",
            other,
        )),
    }
}

fn optional_string(row: &rusqlite::Row<'_>, column_name: &str) -> rusqlite::Result<Option<String>> {
    match row.get::<_, Option<String>>(column_name) {
        Ok(value) => Ok(value),
        Err(rusqlite::Error::InvalidColumnName(_)) => Ok(None),
        Err(rusqlite::Error::InvalidColumnIndex(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

fn optional_f64(row: &rusqlite::Row<'_>, column_name: &str) -> rusqlite::Result<Option<f64>> {
    match row.get::<_, Option<f64>>(column_name) {
        Ok(value) => Ok(value),
        Err(rusqlite::Error::InvalidColumnName(_)) => Ok(None),
        Err(rusqlite::Error::InvalidColumnIndex(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

fn parse_i32_column(row: &rusqlite::Row<'_>, column_name: &str) -> rusqlite::Result<Option<i32>> {
    match row.get_ref(column_name) {
        Ok(ValueRef::Null) => Ok(None),
        Ok(ValueRef::Integer(value)) => i32::try_from(value).map(Some).map_err(|_| {
            invalid_numeric_conversion_error(
                column_name,
                rusqlite::types::Type::Integer,
                format!("valor fora do range i32: {value}"),
            )
        }),
        Ok(ValueRef::Real(value)) => {
            let rounded = value.round();
            if !rounded.is_finite() || rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
                return Err(invalid_numeric_conversion_error(
                    column_name,
                    rusqlite::types::Type::Real,
                    format!("valor fora do range i32: {value}"),
                ));
            }
            Ok(Some(rounded as i32))
        }
        Ok(ValueRef::Text(bytes)) => {
            let text = std::str::from_utf8(bytes).map_err(|_| {
                invalid_numeric_conversion_error(
                    column_name,
                    rusqlite::types::Type::Text,
                    "texto UTF-8 invalido",
                )
            })?;
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            let parsed = trimmed.parse::<i32>().map_err(|_| {
                invalid_numeric_conversion_error(
                    column_name,
                    rusqlite::types::Type::Text,
                    format!("texto nao numerico: '{trimmed}'"),
                )
            })?;
            Ok(Some(parsed))
        }
        Ok(ValueRef::Blob(_)) => Err(invalid_numeric_conversion_error(
            column_name,
            rusqlite::types::Type::Blob,
            "blob nao pode ser convertido para i32",
        )),
        Err(rusqlite::Error::InvalidColumnName(_)) => Ok(None),
        Err(rusqlite::Error::InvalidColumnIndex(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

fn required_i32_column(row: &rusqlite::Row<'_>, column_name: &str) -> rusqlite::Result<i32> {
    parse_i32_column(row, column_name)?.ok_or_else(|| {
        invalid_numeric_conversion_error(
            column_name,
            rusqlite::types::Type::Null,
            "campo obrigatorio ausente ou nulo",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::contract::Contract;

    #[test]
    fn test_insert_and_get_contract() {
        let conn = setup_test_db().expect("test db");
        let contract = sample_contract("C001", "P001", "T001", ContractStatus::Ativo);

        insert_contract(&conn, &contract).expect("insert contract");
        let loaded = get_contract_by_id(&conn, "C001")
            .expect("query contract")
            .expect("contract should exist");

        assert_eq!(loaded.id, "C001");
        assert_eq!(loaded.piloto_nome, contract.piloto_nome);
        assert_eq!(loaded.equipe_nome, contract.equipe_nome);
        assert_eq!(loaded.papel, TeamRole::Numero1);
    }

    #[test]
    fn test_get_active_contract_for_pilot() {
        let conn = setup_test_db().expect("test db");
        let expired = sample_contract("C001", "P001", "T001", ContractStatus::Expirado);
        let active = sample_contract("C002", "P001", "T002", ContractStatus::Ativo);
        insert_contracts(&conn, &[expired, active]).expect("insert contracts");

        let loaded = get_active_contract_for_pilot(&conn, "P001")
            .expect("query active contract")
            .expect("active contract should exist");

        assert_eq!(loaded.id, "C002");
        assert_eq!(loaded.equipe_id, "T002");
    }

    #[test]
    fn test_get_active_contracts_for_team() {
        let conn = setup_test_db().expect("test db");
        insert_contract(
            &conn,
            &sample_contract("C001", "P001", "T001", ContractStatus::Ativo),
        )
        .expect("insert 1");
        insert_contract(
            &conn,
            &sample_contract("C002", "P002", "T001", ContractStatus::Ativo),
        )
        .expect("insert 2");
        insert_contract(
            &conn,
            &sample_contract("C003", "P003", "T001", ContractStatus::Expirado),
        )
        .expect("insert 3");

        let contracts = get_active_contracts_for_team(&conn, "T001").expect("query team contracts");

        assert_eq!(contracts.len(), 2);
        assert!(contracts
            .iter()
            .all(|contract| contract.status == ContractStatus::Ativo));
    }

    #[test]
    fn test_get_all_active_regular_contracts_filters_special() {
        let conn = setup_test_db().expect("test db");
        let regular = sample_contract("C001", "P001", "T001", ContractStatus::Ativo);
        let mut special = sample_contract("C002", "P002", "T002", ContractStatus::Ativo);
        special.tipo = ContractType::Especial;
        insert_contracts(&conn, &[regular.clone(), special]).expect("insert contracts");

        let contracts = get_all_active_regular_contracts(&conn).expect("query active regular");

        assert_eq!(contracts.len(), 1);
        assert_eq!(contracts[0].id, regular.id);
        assert_eq!(contracts[0].tipo, ContractType::Regular);
    }

    #[test]
    fn test_expire_ending_contracts() {
        let conn = setup_test_db().expect("test db");
        let mut contract = sample_contract("C001", "P001", "T001", ContractStatus::Ativo);
        contract.temporada_fim = 3;
        insert_contract(&conn, &contract).expect("insert contract");

        let updated = expire_ending_contracts(&conn, 3).expect("expire contracts");
        assert_eq!(updated, 1);

        let loaded = get_contract_by_id(&conn, "C001")
            .expect("query contract")
            .expect("contract should exist");
        assert_eq!(loaded.status, ContractStatus::Expirado);
    }

    #[test]
    fn test_get_expiring_contracts() {
        let conn = setup_test_db().expect("test db");
        let mut expiring = sample_contract("C001", "P001", "T001", ContractStatus::Ativo);
        expiring.temporada_fim = 4;
        let mut long = sample_contract("C002", "P002", "T002", ContractStatus::Ativo);
        long.temporada_fim = 5;
        insert_contracts(&conn, &[expiring, long]).expect("insert contracts");

        let expiring_contracts =
            get_expiring_contracts(&conn, 4).expect("query expiring contracts");

        assert_eq!(expiring_contracts.len(), 1);
        assert_eq!(expiring_contracts[0].id, "C001");
    }

    #[test]
    fn test_count_active_contracts_for_team() {
        let conn = setup_test_db().expect("test db");
        insert_contract(
            &conn,
            &sample_contract("C001", "P001", "T001", ContractStatus::Ativo),
        )
        .expect("insert 1");
        insert_contract(
            &conn,
            &sample_contract("C002", "P002", "T001", ContractStatus::Ativo),
        )
        .expect("insert 2");
        insert_contract(
            &conn,
            &sample_contract("C003", "P003", "T001", ContractStatus::Rescindido),
        )
        .expect("insert 3");

        let count = count_active_contracts_for_team(&conn, "T001").expect("count active");
        assert_eq!(count, 2);
    }

    #[test]
    fn test_get_free_agents_for_preseason_ignores_special_contract_history() {
        let conn = setup_test_db().expect("test db");
        insert_team_stub(&conn, "T001", "#112233");
        insert_team_stub(&conn, "SP001", "#aa5500");
        insert_license_stub(&conn, "P003", 2);

        let mut regular = sample_contract("C100", "P003", "T001", ContractStatus::Expirado);
        regular.equipe_nome = "Equipe Regular".to_string();
        regular.categoria = "mazda_amador".to_string();
        regular.temporada_inicio = 2;
        regular.duracao_anos = 3;
        regular.temporada_fim = 4;
        regular.created_at = "2026-01-01T08:00:00".to_string();

        let mut special = sample_contract("C101", "P003", "SP001", ContractStatus::Expirado);
        special.equipe_nome = "Equipe Especial".to_string();
        special.tipo = ContractType::Especial;
        special.categoria = "production_challenger".to_string();
        special.classe = Some("mazda".to_string());
        special.temporada_inicio = 4;
        special.duracao_anos = 1;
        special.temporada_fim = 4;
        special.created_at = "2026-06-01T08:00:00".to_string();

        insert_contracts(&conn, &[regular, special]).expect("insert contracts");
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                "P003",
                4,
                2026,
                "Piloto 3",
                "mazda_amador",
                12,
                95.0,
                r#"{"total_pilotos":20}"#
            ],
        )
        .expect("insert archive");

        let free_agents = get_free_agents_for_preseason(&conn).expect("free agents query");
        let driver = free_agents
            .into_iter()
            .find(|agent| agent.driver_id == "P003")
            .expect("driver should be free agent");

        assert_eq!(driver.categoria, "mazda_amador");
        assert_eq!(driver.previous_team_name.as_deref(), Some("Equipe Regular"));
        assert_eq!(driver.previous_team_color.as_deref(), Some("#112233"));
        assert_eq!(driver.seasons_at_last_team, 3);
        assert_eq!(driver.last_championship_position, Some(12));
        assert_eq!(driver.last_championship_total_drivers, Some(20));
    }

    fn sample_contract(
        id: &str,
        piloto_id: &str,
        equipe_id: &str,
        status: ContractStatus,
    ) -> Contract {
        Contract {
            id: id.to_string(),
            piloto_id: piloto_id.to_string(),
            piloto_nome: format!("Piloto {}", &piloto_id[1..]),
            equipe_id: equipe_id.to_string(),
            equipe_nome: format!("Equipe {}", &equipe_id[1..]),
            temporada_inicio: 1,
            duracao_anos: 2,
            temporada_fim: 2,
            salario_anual: 100_000.0,
            papel: TeamRole::Numero1,
            status,
            tipo: ContractType::Regular,
            categoria: "gt3".to_string(),
            classe: None,
            created_at: "2026-01-01T12:00:00".to_string(),
        }
    }

    #[test]
    fn test_unknown_contract_status_returns_error() {
        let conn = setup_test_db().expect("test db");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome,
                temporada_inicio, duracao_anos, temporada_fim,
                salario, salario_anual, papel, status, tipo, categoria, created_at
             ) VALUES ('C_BAD', 'P001', 'Piloto 1', 'T001', 'Equipe', 1, 1, 2,
                       100000, 100000, 'Numero1', 'Suspenso', 'Regular', 'gt3', '2026-01-01')",
            [],
        )
        .expect("insert contract with unknown status");

        let result = get_contract_by_id(&conn, "C_BAD");
        assert!(
            result.is_err(),
            "status desconhecido deve retornar erro, nao default silencioso"
        );
    }

    #[test]
    fn test_unknown_contract_role_returns_error() {
        let conn = setup_test_db().expect("test db");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome,
                temporada_inicio, duracao_anos, temporada_fim,
                salario, salario_anual, papel, status, tipo, categoria, created_at
             ) VALUES ('C_BAD2', 'P001', 'Piloto 1', 'T001', 'Equipe', 1, 1, 2,
                       100000, 100000, 'Wildcard', 'Ativo', 'Regular', 'gt3', '2026-01-01')",
            [],
        )
        .expect("insert contract with unknown role");

        let result = get_contract_by_id(&conn, "C_BAD2");
        assert!(
            result.is_err(),
            "papel desconhecido deve retornar erro, nao default silencioso"
        );
    }

    #[test]
    fn test_blob_in_piloto_nome_returns_error() {
        let conn = setup_test_db().expect("test db");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome,
                temporada_inicio, duracao_anos, temporada_fim,
                salario, salario_anual, papel, status, tipo, categoria, created_at
             ) VALUES ('C_BLOB_NAME', 'P001', X'DEADBEEF', 'T001', 'Equipe', 1, 1, 2,
                       100000, 100000, 'Numero1', 'Ativo', 'Regular', 'gt3', '2026-01-01')",
            [],
        )
        .expect("insert contract with blob piloto_nome");

        let result = get_contract_by_id(&conn, "C_BLOB_NAME");
        assert!(
            result.is_err(),
            "BLOB em piloto_nome deve retornar erro, nao virar string vazia"
        );
    }

    #[test]
    fn test_blob_in_salario_anual_returns_error() {
        let conn = setup_test_db().expect("test db");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome,
                temporada_inicio, duracao_anos, temporada_fim,
                salario, salario_anual, papel, status, tipo, categoria, created_at
             ) VALUES ('C_BLOB_SAL', 'P001', 'Piloto 1', 'T001', 'Equipe', 1, 1, 2,
                       100000, X'DEADBEEF', 'Numero1', 'Ativo', 'Regular', 'gt3', '2026-01-01')",
            [],
        )
        .expect("insert contract with blob salario_anual");

        let result = get_contract_by_id(&conn, "C_BLOB_SAL");
        assert!(
            result.is_err(),
            "BLOB em salario_anual deve retornar erro, nao cair em fallback silencioso"
        );
    }

    #[test]
    fn test_invalid_temporada_inicio_returns_error_instead_of_fallback() {
        let conn = setup_test_db().expect("test db");
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome,
                temporada_inicio, duracao_anos, temporada_fim,
                salario, salario_anual, papel, status, tipo, categoria, created_at
             ) VALUES ('C_BAD_SEASON', 'P001', 'Piloto 1', 'T001', 'Equipe', 'abc', 1, 2,
                       100000, 100000, 'Numero1', 'Ativo', 'Regular', 'gt3', '2026-01-01')",
            [],
        )
        .expect("insert contract with invalid temporada_inicio");

        let result = get_contract_by_id(&conn, "C_BAD_SEASON");
        assert!(
            result.is_err(),
            "temporada_inicio invalida deve retornar erro, nao cair em fallback silencioso"
        );
    }

    #[test]
    fn test_update_contract_status_returns_not_found_for_missing_contract() {
        let conn = setup_test_db().expect("test db");

        let err = update_contract_status(&conn, "C404", &ContractStatus::Expirado)
            .expect_err("missing contract should fail");

        assert!(
            matches!(err, DbError::NotFound(_)),
            "expected not-found error, got {err:?}"
        );
    }

    fn setup_test_db() -> Result<Connection, DbError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE drivers (
                id TEXT PRIMARY KEY,
                nome TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'Ativo',
                is_jogador INTEGER NOT NULL DEFAULT 0,
                categoria_atual TEXT
            );
            CREATE TABLE teams (
                id TEXT PRIMARY KEY,
                cor_primaria TEXT
            );
            CREATE TABLE licenses (
                id TEXT PRIMARY KEY,
                piloto_id TEXT NOT NULL,
                nivel INTEGER NOT NULL
            );
            CREATE TABLE driver_season_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL,
                posicao_campeonato INTEGER,
                pontos REAL NOT NULL DEFAULT 0.0,
                snapshot_json TEXT NOT NULL,
                archived_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(piloto_id, season_number)
            );
            INSERT INTO drivers (id, nome) VALUES
                ('P001', 'Piloto 1'),
                ('P002', 'Piloto 2'),
                ('P003', 'Piloto 3');

            CREATE TABLE contracts (
                id TEXT PRIMARY KEY NOT NULL,
                piloto_id TEXT NOT NULL,
                piloto_nome TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                equipe_nome TEXT NOT NULL,
                temporada_inicio INTEGER NOT NULL,
                duracao_anos INTEGER NOT NULL,
                temporada_fim INTEGER NOT NULL,
                salario REAL NOT NULL DEFAULT 0.0,
                salario_anual REAL NOT NULL DEFAULT 0.0,
                papel TEXT NOT NULL DEFAULT 'Numero2',
                status TEXT NOT NULL DEFAULT 'Ativo',
                tipo TEXT NOT NULL DEFAULT 'Regular',
                categoria TEXT NOT NULL,
                classe TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );",
        )?;
        Ok(conn)
    }

    fn insert_team_stub(conn: &Connection, id: &str, cor_primaria: &str) {
        conn.execute(
            "INSERT INTO teams (id, cor_primaria) VALUES (?1, ?2)",
            params![id, cor_primaria],
        )
        .expect("insert team stub");
    }

    fn insert_license_stub(conn: &Connection, piloto_id: &str, nivel: i32) {
        conn.execute(
            "INSERT INTO licenses (id, piloto_id, nivel) VALUES (?1, ?2, ?3)",
            params![format!("L_{piloto_id}_{nivel}"), piloto_id, nivel],
        )
        .expect("insert license stub");
    }
}
