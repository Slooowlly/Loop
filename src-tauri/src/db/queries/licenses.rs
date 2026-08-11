//! Persistência de licenças de piloto.
//!
//! O SQL de licença vivia dentro de `models/license.rs` — três consultas e dois `INSERT`
//! num arquivo cuja função é declarar a REGRA da escada (qual nível cada divisão exige,
//! qual ela concede). O padrão do projeto é o oposto: model declara, `db/queries` grava.
//! A exceção convidava imitação, e o reparo de licenças legadas era o caso mais gritante:
//! um laço que lia pilotos e escrevia licenças, inteiro, de dentro da camada de modelo.
//!
//! A divisão que ficou: a REGRA continua em `models::license` (é dela a decisão de que
//! `endurance/lmp2` exige nível 5); a ESCRITA e a LEITURA moram aqui. O reparo mora aqui
//! porque é, do começo ao fim, uma varredura de banco — só consulta a regra para saber
//! qual nível conceder.

use rusqlite::Connection;

use crate::common::time::current_timestamp;
use crate::db::connection::DbError;

/// O piloto já tem licença de nível maior ou igual ao exigido?
///
/// `nivel` é TEXT na tabela por herança de save antigo, daí o `CAST` — comparar como
/// texto faria "10" < "2".
pub fn tem_nivel_minimo(
    conn: &Connection,
    piloto_id: &str,
    nivel_minimo: u8,
) -> Result<bool, DbError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM licenses WHERE piloto_id = ?1 AND CAST(nivel AS INTEGER) >= ?2",
        rusqlite::params![piloto_id, nivel_minimo as i64],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Concede o nível ao piloto se ele ainda não tiver nível igual ou maior.
///
/// A condição vai DENTRO do `INSERT ... WHERE NOT EXISTS` de propósito: checar antes e
/// inserir depois abriria janela para dois caminhos concederem a mesma licença duas vezes
/// na mesma transição de temporada. Devolve quantas linhas nasceram (0 ou 1).
pub fn concede_nivel_se_faltar(
    conn: &Connection,
    piloto_id: &str,
    nivel: u8,
    categoria_origem: &str,
) -> Result<usize, DbError> {
    let inseridas = conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         SELECT ?1, ?2, ?3, ?4, 0
         WHERE NOT EXISTS (
             SELECT 1 FROM licenses WHERE piloto_id = ?1 AND CAST(nivel AS INTEGER) >= ?5
         )",
        rusqlite::params![
            piloto_id,
            nivel.to_string(),
            categoria_origem,
            current_timestamp(),
            nivel as i64,
        ],
    )?;
    Ok(inseridas)
}

/// Pares `(piloto_id, categoria_atual)` de quem está ativo e alocado numa categoria.
pub fn pilotos_ativos_com_categoria(conn: &Connection) -> Result<Vec<(String, String)>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, categoria_atual
         FROM drivers
         WHERE status = 'Ativo' AND categoria_atual IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut pilotos = Vec::new();
    for row in rows {
        pilotos.push(row?);
    }
    Ok(pilotos)
}

/// Reparo de saves legados: dá a cada piloto ativo a licença mínima da categoria em que
/// ele já está correndo, quando ela falta.
///
/// Existe porque a escada de licenças chegou depois de saves já povoados, e um piloto sem
/// a licença da própria categoria trava renovação e transferência no mercado — sem
/// nenhuma mensagem que explique por quê. Devolve quantos pilotos foram reparados.
///
/// A regra de QUAL nível a categoria exige continua sendo de `models::license`; aqui só a
/// consultamos.
pub fn repara_licencas_das_categorias_atuais(conn: &Connection) -> Result<usize, DbError> {
    let mut reparados = 0;
    for (piloto_id, categoria) in pilotos_ativos_com_categoria(conn)? {
        let Some(nivel) = crate::models::license::required_license_for_category(&categoria) else {
            continue;
        };
        if tem_nivel_minimo(conn, &piloto_id, nivel)? {
            continue;
        }
        if concede_nivel_se_faltar(conn, &piloto_id, nivel, &categoria)? > 0 {
            reparados += 1;
        }
    }
    Ok(reparados)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::run_all;

    fn banco() -> Connection {
        let conn = Connection::open_in_memory().expect("banco em memória");
        run_all(&conn).expect("migrações");
        conn
    }

    fn cria_piloto(conn: &Connection, id: &str, categoria: Option<&str>, status: &str) {
        let mut piloto = crate::models::driver::Driver::new(
            id.to_string(),
            format!("Piloto {id}"),
            "br".to_string(),
            "M".to_string(),
            24,
            2020,
        );
        piloto.categoria_atual = categoria.map(str::to_string);
        piloto.status =
            crate::models::enums::DriverStatus::from_str_strict(status).expect("status");
        crate::db::queries::drivers::insert_driver(conn, &piloto).expect("piloto");
    }

    #[test]
    fn conceder_duas_vezes_o_mesmo_nivel_nao_duplica_a_licenca() {
        let conn = banco();
        cria_piloto(&conn, "P001", Some("gt3"), "Ativo");

        assert_eq!(
            concede_nivel_se_faltar(&conn, "P001", 3, "gt3").expect("primeira"),
            1
        );
        assert_eq!(
            concede_nivel_se_faltar(&conn, "P001", 3, "gt3").expect("segunda"),
            0,
            "quem já tem o nível não ganha uma segunda linha da mesma licença"
        );
    }

    #[test]
    fn licenca_mais_alta_cobre_a_exigencia_mais_baixa() {
        let conn = banco();
        cria_piloto(&conn, "P001", Some("gt3"), "Ativo");
        concede_nivel_se_faltar(&conn, "P001", 5, "endurance").expect("nível 5");

        assert!(tem_nivel_minimo(&conn, "P001", 3).expect("consulta"));
        assert!(tem_nivel_minimo(&conn, "P001", 5).expect("consulta"));
        assert!(!tem_nivel_minimo(&conn, "P001", 6).expect("consulta"));
        assert_eq!(
            concede_nivel_se_faltar(&conn, "P001", 3, "gt3").expect("nível menor"),
            0,
            "ter 5 já satisfaz 3 — conceder de novo seria inflar o histórico"
        );
    }

    /// O `CAST` do nível existe por isto: a coluna é TEXT e a comparação textual põe
    /// "10" antes de "2".
    #[test]
    fn o_nivel_e_comparado_como_numero_e_nao_como_texto() {
        let conn = banco();
        cria_piloto(&conn, "P001", Some("gt3"), "Ativo");
        conn.execute(
            "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
             VALUES ('P001', '10', 'teste', '2026-01-01', 1)",
            [],
        )
        .expect("licença de nível 10");

        assert!(
            tem_nivel_minimo(&conn, "P001", 2).expect("consulta"),
            "nível 10 satisfaz uma exigência de 2"
        );
    }

    #[test]
    fn o_reparo_da_a_licenca_da_categoria_a_quem_esta_sem_ela() {
        let conn = banco();
        cria_piloto(&conn, "P_GT3", Some("gt3"), "Ativo");

        let reparados = repara_licencas_das_categorias_atuais(&conn).expect("reparo");

        assert_eq!(reparados, 1);
        assert!(
            tem_nivel_minimo(&conn, "P_GT3", 3).expect("consulta"),
            "gt3 exige nível 3 e o piloto já corria lá sem ele"
        );
    }

    /// Quem o reparo NÃO pode tocar: aposentado, sem categoria e quem já tem a licença.
    #[test]
    fn o_reparo_ignora_quem_nao_precisa_e_e_idempotente() {
        let conn = banco();
        cria_piloto(&conn, "P_GT3", Some("gt3"), "Ativo");
        cria_piloto(&conn, "P_LIVRE", None, "Ativo");
        cria_piloto(&conn, "P_APOSENTADO", Some("gt3"), "Aposentado");
        // Categoria sem exigência de licença: o rookie não gera linha nenhuma.
        cria_piloto(&conn, "P_ROOKIE", Some("mazda_rookie"), "Ativo");

        assert_eq!(
            repara_licencas_das_categorias_atuais(&conn).expect("primeiro reparo"),
            1
        );
        assert_eq!(
            repara_licencas_das_categorias_atuais(&conn).expect("segundo reparo"),
            0,
            "rodado de novo, o reparo não tem o que reparar"
        );

        for sem_licenca in ["P_LIVRE", "P_APOSENTADO", "P_ROOKIE"] {
            assert!(
                !tem_nivel_minimo(&conn, sem_licenca, 1).expect("consulta"),
                "'{sem_licenca}' não deveria ter ganhado licença no reparo"
            );
        }
    }
}
