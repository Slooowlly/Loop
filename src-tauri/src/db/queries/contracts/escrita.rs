//! Escrita de contratos: inserção, atualização, expiração e remoção.

use rusqlite::{params, Connection};

use crate::db::connection::DbError;
use crate::models::contract::Contract;
use crate::models::enums::{ContractStatus, TeamRole};

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
    if matches!(status, ContractStatus::Rescindido) {
        encurtar_vigencia_ao_rescindir(conn, id)?;
    }
    Ok(())
}

/// Rescindir é ENCURTAR a vigência, não só carimbar o status.
///
/// A rescisão trocava o status e deixava `temporada_fim` no ano que fora
/// assinado, então o contrato continuava valendo no banco por temporadas que o
/// piloto nunca cumpriu por aquela equipe. Quem lê contrato por vigência —
/// a curva de mercado da ficha, a categoria do agente livre, o mapa de
/// piloto→categoria do mercado — via um vínculo que já não existia, e no gráfico
/// isso virava salário e logo de uma equipe que ele tinha deixado.
///
/// O corte é na temporada corrente e nunca antes do início: um contrato ainda
/// não começado (assinado para o ano que vem e desfeito antes) fica intacto, e
/// quem lê por vigência já o ignora por outros caminhos.
///
/// Fica aqui dentro, e não nos ~20 pontos que rescindem, porque rescisão é uma
/// operação só: espalhada, a próxima chamada nova nasceria sem o encurtamento.
///
/// Precisão de um ano: o mercado da pré-temporada roda DEPOIS de a meta virar
/// para a temporada seguinte, então uma dispensa ali corta em N+1 e não em N. A
/// curva não depende disso — ela corta pelo que o arquivo diz que foi cumprido
/// ([`crate::commands::career_detail`]) — mas quem só olha a vigência vê um ano
/// a mais nesse caso.
fn encurtar_vigencia_ao_rescindir(conn: &Connection, id: &str) -> Result<(), DbError> {
    // Sem `meta` legivel nao ha onde cortar, e o corte e um extra: a rescisao em
    // si ja aconteceu. Erro na leitura vira "nao sei a temporada" em vez de
    // derrubar a operacao — vale para o save meio migrado e para os fixtures de
    // teste que montam meia dúzia de tabelas.
    let Some(corrente) = crate::db::queries::meta::get_meta_value(conn, "current_season")
        .unwrap_or(None)
        .and_then(|valor| valor.trim().parse::<i32>().ok())
    else {
        return Ok(());
    };

    conn.execute(
        "UPDATE contracts
            SET temporada_fim = ?1
          WHERE id = ?2
            AND CAST(temporada_inicio AS INTEGER) <= ?1
            AND CAST(temporada_fim AS INTEGER) > ?1",
        params![corrente, id],
    )?;
    Ok(())
}

/// Reajusta o salário anual de um contrato. Usado pela **retenção** do poaching
/// (Fase 2b.2): segurar um piloto assediado custa aumento, e o aumento fica no
/// contrato — pesando no orçamento do time nas temporadas seguintes.
pub fn update_contract_salary(
    conn: &Connection,
    id: &str,
    salario_anual: f64,
) -> Result<(), DbError> {
    let affected = conn.execute(
        "UPDATE contracts SET salario_anual = ?1 WHERE id = ?2",
        params![salario_anual, id],
    )?;
    if affected == 0 {
        return Err(DbError::NotFound(format!(
            "Contrato '{id}' nao encontrado para reajustar salario"
        )));
    }
    Ok(())
}

/// Troca o papel (N1/N2) de um contrato.
///
/// Usado pela INVERSÃO de hierarquia ([`crate::hierarchy::orders::apply_inversao`]):
/// quem vence a política interna da garagem passa a ser N1 **no contrato**, que é a
/// fonte que o mercado lê ([`crate::market::renewal::should_renew_contract`] decide por
/// `contract.papel`, não por `team.hierarquia_n1_id`). Sem esta sincronia a inversão
/// era cosmética — o piloto promovido continuava caindo nos gates de N2 na renovação.
pub fn update_contract_role(conn: &Connection, id: &str, papel: &TeamRole) -> Result<(), DbError> {
    let affected = conn.execute(
        "UPDATE contracts SET papel = ?1 WHERE id = ?2",
        params![papel.as_str(), id],
    )?;
    if affected == 0 {
        return Err(DbError::NotFound(format!(
            "Contrato '{id}' nao encontrado para atualizar papel"
        )));
    }
    Ok(())
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

pub fn delete_contract(conn: &Connection, id: &str) -> Result<(), DbError> {
    conn.execute("DELETE FROM contracts WHERE id = ?1", params![id])?;
    Ok(())
}
