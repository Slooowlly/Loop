//! Contratos do tipo Especial: consultas, expiração do bloco e fábrica.

use rusqlite::{params, Connection, OptionalExtension};

use super::mapeamento::{collect_contracts, contract_from_row};
use crate::constants::categories::get_category_config;
use crate::db::connection::DbError;
use crate::models::contract::Contract;
use crate::models::enums::{ContractType, TeamRole};

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

/// Expira todos os contratos Especial ativos da temporada indicada.
/// Chamado durante PosEspecial — nenhum contrato Especial deve sobreviver ao bloco.
///
/// Filtra por `temporada_inicio = season_number` para precisão semântica e proteção
/// contra bugs futuros. No modelo atual só existe um ciclo especial ativo por vez,
/// portanto o resultado seria idêntico sem o filtro.
pub fn expire_especial_contracts(conn: &Connection, season_number: i32) -> Result<usize, DbError> {
    // Legacy-only cleanup. Production/Endurance now use persistent Regular contracts
    // and are deliberately excluded from this Especial path.
    let n = conn.execute(
        "UPDATE contracts SET status = 'Expirado'
         WHERE tipo = 'Especial'
           AND status = 'Ativo'
           AND temporada_inicio = ?1
           AND categoria NOT IN ('production_challenger', 'endurance')",
        params![season_number],
    )?;
    Ok(n)
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
    // Legacy/future special-event contract factory. Real Production/Endurance
    // teams should be populated through regular market/lineup flows instead.
    let tier = get_category_config(categoria).map(|c| c.tier).unwrap_or(2);
    // Salário do contrato especial = ~50% do ponto médio da faixa regular do tier.
    // Derivado da FONTE ÚNICA (`salary_range_for_tier`) em vez de uma tabela própria:
    // a antiga `salary_midpoint_for_tier` era uma cópia desatualizada desses pontos
    // médios e havia derivado nos tiers 5 (165k vs 300k) e 6 (caía no default de 10k,
    // fazendo o endurance especial pagar 5k). Uma tabela só, sem como derivar de novo.
    let (range_min, range_max) = crate::models::contract::salary_range_for_tier(tier);
    let salario_anual = (range_min + range_max) / 2.0 * 0.5;
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
