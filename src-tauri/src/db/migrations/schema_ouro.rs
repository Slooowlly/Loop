//! Captura do "schema-ouro": a descrição normalizada do schema que as migrações
//! produzem num banco novo. Serve de trava para o colapso das migrações numa
//! baseline única — a baseline só está correta se gerar exatamente este schema.
//!
//! A comparação é SEMÂNTICA, nunca textual: uma tabela criada com `CREATE` +
//! vários `ALTER TABLE ADD COLUMN` guarda em `sqlite_master.sql` um texto
//! diferente de um `CREATE` com as colunas inline, mesmo sendo o mesmo schema.
//! Por isso lemos `PRAGMA table_info` / `index_list` / `index_info` e serializamos
//! tudo ordenado.

use rusqlite::Connection;

/// Caminho do fixture versionado, relativo ao crate.
const FIXTURE_REL: &str = "src/db/migrations/schema_ouro.txt";

/// Serializa o schema de forma determinística: uma linha por item, tudo ordenado.
pub fn descrever_schema(conn: &Connection) -> String {
    let mut linhas = Vec::new();

    for tabela in nomes_por_tipo(conn, "table") {
        if tabela.starts_with("sqlite_") {
            continue;
        }
        linhas.push(format!("tabela {tabela}"));

        // Colunas: ignoramos o `cid`, que depende da ordem de criação.
        let mut colunas: Vec<String> = {
            let mut stmt = conn
                .prepare(&format!("PRAGMA table_info(\"{tabela}\")"))
                .expect("table_info");
            let linhas_col = stmt
                .query_map([], |row| {
                    let nome: String = row.get(1)?;
                    let tipo: String = row.get(2)?;
                    let notnull: i64 = row.get(3)?;
                    let default: Option<String> = row.get(4)?;
                    let pk: i64 = row.get(5)?;
                    Ok((nome, tipo, notnull, default, pk))
                })
                .expect("query table_info")
                .collect::<Result<Vec<_>, _>>()
                .expect("linhas table_info");
            linhas_col
                .into_iter()
                .map(|(nome, tipo, notnull, default, pk)| {
                    let default = default.unwrap_or_else(|| "-".to_string());
                    format!(
                        "  coluna {nome} | tipo={tipo} | notnull={notnull} | default={default} | pk={pk}"
                    )
                })
                .collect()
        };
        colunas.sort();
        linhas.extend(colunas);

        // Índices: os autoíndices do SQLite são consequência de PK/UNIQUE e não
        // fazem parte da declaração, então ficam de fora.
        let mut indices: Vec<(String, i64, String, i64)> = {
            let mut stmt = conn
                .prepare(&format!("PRAGMA index_list(\"{tabela}\")"))
                .expect("index_list");
            stmt.query_map([], |row| {
                let nome: String = row.get(1)?;
                let unico: i64 = row.get(2)?;
                let origem: String = row.get(3)?;
                let parcial: i64 = row.get(4)?;
                Ok((nome, unico, origem, parcial))
            })
            .expect("query index_list")
            .collect::<Result<Vec<_>, _>>()
            .expect("linhas index_list")
        };
        indices.sort();

        for (nome, unico, origem, parcial) in indices {
            if nome.starts_with("sqlite_autoindex_") {
                continue;
            }
            let mut stmt = conn
                .prepare(&format!("PRAGMA index_info(\"{nome}\")"))
                .expect("index_info");
            let mut cols: Vec<(i64, String)> = stmt
                .query_map([], |row| {
                    let seqno: i64 = row.get(0)?;
                    let nome_col: Option<String> = row.get(2)?;
                    Ok((seqno, nome_col.unwrap_or_else(|| "<expr>".to_string())))
                })
                .expect("query index_info")
                .collect::<Result<Vec<_>, _>>()
                .expect("linhas index_info");
            cols.sort();
            let cols = cols
                .into_iter()
                .map(|(_, c)| c)
                .collect::<Vec<_>>()
                .join(",");
            linhas.push(format!(
                "  indice {nome} | unique={unico} | origem={origem} | parcial={parcial} | colunas={cols}"
            ));
        }
    }

    for view in nomes_por_tipo(conn, "view") {
        linhas.push(format!("view {view}"));
    }
    for trigger in nomes_por_tipo(conn, "trigger") {
        linhas.push(format!("trigger {trigger}"));
    }

    let mut texto = linhas.join("\n");
    texto.push('\n');
    texto
}

fn nomes_por_tipo(conn: &Connection, tipo: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = ?1 AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .expect("sqlite_master");
    stmt.query_map([tipo], |row| row.get::<_, String>(0))
        .expect("query sqlite_master")
        .collect::<Result<Vec<_>, _>>()
        .expect("nomes sqlite_master")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::run_all;
    use std::path::PathBuf;

    fn caminho_fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_REL)
    }

    /// Trava do schema-ouro: roda as migrações num banco novo e compara a
    /// descrição normalizada com o fixture versionado. Se der diff, o errado é o
    /// código de migração — o fixture é a verdade.
    #[test]
    fn schema_das_migracoes_bate_com_o_fixture() {
        let conn = Connection::open_in_memory().expect("banco em memória");
        run_all(&conn).expect("migrações");

        let atual = descrever_schema(&conn);
        let caminho = caminho_fixture();

        if !caminho.exists() {
            std::fs::write(&caminho, &atual).expect("gravar fixture inicial");
            panic!(
                "fixture do schema-ouro não existia e foi gerado em {}. Confira e commite.",
                caminho.display()
            );
        }

        let esperado = std::fs::read_to_string(&caminho).expect("ler fixture");
        let esperado = esperado.replace("\r\n", "\n");

        if esperado != atual {
            let esperadas: Vec<&str> = esperado.lines().collect();
            let atuais: Vec<&str> = atual.lines().collect();
            let so_no_fixture: Vec<&&str> =
                esperadas.iter().filter(|l| !atuais.contains(l)).collect();
            let so_no_atual: Vec<&&str> =
                atuais.iter().filter(|l| !esperadas.contains(l)).collect();
            panic!(
                "schema divergiu do fixture.\n\nfaltando (só no fixture):\n{}\n\nsobrando (só no schema atual):\n{}",
                so_no_fixture
                    .iter()
                    .map(|l| l.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
                so_no_atual
                    .iter()
                    .map(|l| l.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
    }
}
