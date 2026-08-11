//! Consultas de leitura de contratos (por piloto, equipe, categoria e temporada).

use rusqlite::{params, Connection, OptionalExtension};

use super::mapeamento::{collect_contracts, colunas_select_contract, contract_from_row};
use crate::db::connection::DbError;
use crate::models::contract::Contract;

pub fn get_contract_by_id(conn: &Connection, id: &str) -> Result<Option<Contract>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM contracts WHERE id = ?1",
        colunas_select_contract()
    ))?;
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
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM contracts
         WHERE piloto_id = ?1 AND status = 'Ativo'
         ORDER BY temporada_inicio DESC, created_at DESC
         LIMIT 1",
        colunas_select_contract()
    ))?;
    let contract = stmt
        .query_row(params![piloto_id], contract_from_row)
        .optional()?;
    Ok(contract)
}

pub fn get_contracts_for_pilot(
    conn: &Connection,
    piloto_id: &str,
) -> Result<Vec<Contract>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM contracts
         WHERE piloto_id = ?1
         ORDER BY temporada_inicio DESC, created_at DESC",
        colunas_select_contract()
    ))?;
    let mapped = stmt.query_map(params![piloto_id], contract_from_row)?;
    collect_contracts(mapped)
}

/// Ex-companheiros de equipe do piloto: outros pilotos que dividiram a MESMA equipe
/// em temporadas SOBREPOSTAS (dupla real de alguma temporada). Devolve pares
/// `(piloto_id, piloto_nome)` distintos, excluindo o próprio piloto. Base para o
/// rodapé de notícias do mundo (laço "já correu com você"). A fonte é a tabela
/// `contracts` — o histórico de duplas está nela.
pub fn get_former_teammates(
    conn: &Connection,
    piloto_id: &str,
) -> Result<Vec<(String, String)>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT c2.piloto_id, c2.piloto_nome
         FROM contracts c1
         JOIN contracts c2
           ON c1.equipe_id = c2.equipe_id
          AND c2.piloto_id <> c1.piloto_id
          AND c2.temporada_inicio <= c1.temporada_fim
          AND c2.temporada_fim >= c1.temporada_inicio
         WHERE c1.piloto_id = ?1",
    )?;
    let rows = stmt
        .query_map(params![piloto_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_active_contracts_for_team(
    conn: &Connection,
    equipe_id: &str,
) -> Result<Vec<Contract>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM contracts
         WHERE equipe_id = ?1 AND status = 'Ativo'
         ORDER BY papel ASC, piloto_nome ASC",
        colunas_select_contract()
    ))?;
    let mapped = stmt.query_map(params![equipe_id], contract_from_row)?;
    collect_contracts(mapped)
}

pub fn get_all_active_contracts(conn: &Connection) -> Result<Vec<Contract>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM contracts
         WHERE status = 'Ativo'
         ORDER BY categoria, equipe_nome, piloto_nome",
        colunas_select_contract()
    ))?;
    let mapped = stmt.query_map([], contract_from_row)?;
    collect_contracts(mapped)
}

pub fn get_all_active_regular_contracts(conn: &Connection) -> Result<Vec<Contract>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM contracts
         WHERE status = 'Ativo' AND tipo = 'Regular'
         ORDER BY categoria, equipe_nome, piloto_nome",
        colunas_select_contract()
    ))?;
    let mapped = stmt.query_map([], contract_from_row)?;
    collect_contracts(mapped)
}

pub fn get_active_regular_contracts_by_team(
    conn: &Connection,
    team_id: &str,
) -> Result<Vec<Contract>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM contracts
         WHERE status = 'Ativo' AND tipo = 'Regular' AND equipe_id = ?1
         ORDER BY piloto_nome",
        colunas_select_contract()
    ))?;
    let mapped = stmt.query_map(params![team_id], contract_from_row)?;
    collect_contracts(mapped)
}

pub fn get_expiring_contracts(conn: &Connection, temporada: i32) -> Result<Vec<Contract>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM contracts
         WHERE status = 'Ativo' AND CAST(temporada_fim AS INTEGER) = ?1
         ORDER BY categoria, equipe_nome, piloto_nome",
        colunas_select_contract()
    ))?;
    let mapped = stmt.query_map(params![temporada], contract_from_row)?;
    collect_contracts(mapped)
}

pub fn get_contracts_by_category(
    conn: &Connection,
    categoria: &str,
) -> Result<Vec<Contract>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM contracts
         WHERE categoria = ?1
         ORDER BY equipe_nome, piloto_nome",
        colunas_select_contract()
    ))?;
    let mapped = stmt.query_map(params![categoria], contract_from_row)?;
    collect_contracts(mapped)
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
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM contracts
         WHERE piloto_id = ?1 AND status = 'Ativo' AND tipo = 'Regular'
         ORDER BY temporada_inicio DESC, created_at DESC
         LIMIT 1",
        colunas_select_contract()
    ))?;
    let contract = stmt
        .query_row(params![piloto_id], contract_from_row)
        .optional()?;
    Ok(contract)
}
