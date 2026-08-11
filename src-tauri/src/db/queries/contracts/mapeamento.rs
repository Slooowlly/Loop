//! Mapeamento de linhas do SQLite para `Contract` e parsers auxiliares.

use std::sync::OnceLock;

use rusqlite::types::ValueRef;

use crate::db::connection::DbError;
use crate::models::contract::Contract;
use crate::models::enums::{ContractStatus, ContractType, TeamRole};

/// As colunas que o `contract_from_row` lê — a projeção que substitui o `SELECT *`.
///
/// `salario` entra junto de `salario_anual` porque o mapeador ainda cai nela como
/// fallback de save antigo. `clausulas` fica de fora: existe na tabela e ninguém lê.
pub(super) const COLUNAS_CONTRACT: &[&str] = &[
    "id",
    "piloto_id",
    "piloto_nome",
    "equipe_id",
    "equipe_nome",
    "temporada_inicio",
    "duracao_anos",
    "temporada_fim",
    "salario",
    "salario_anual",
    "papel",
    "status",
    "tipo",
    "categoria",
    "classe",
    "created_at",
];

pub(super) fn colunas_select_contract() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| COLUNAS_CONTRACT.join(", "))
}

pub(super) fn collect_contracts(
    mapped: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<Contract>>,
) -> Result<Vec<Contract>, DbError> {
    let mut result = Vec::new();
    for row in mapped {
        result.push(row?);
    }
    Ok(result)
}

pub(super) fn contract_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Contract> {
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
