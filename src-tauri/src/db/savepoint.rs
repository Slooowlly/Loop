//! Savepoint aninhado: roda um bloco dentro de um `SAVEPOINT` e desfaz tudo o que ele
//! escreveu quando o bloco falha.
//!
//! Existe como utilitário único porque o mercado e a escada rodam na MESMA virada de
//! temporada, cada um abrindo o seu savepoint. Enquanto cada módulo tinha a sua cópia,
//! um `ROLLBACK` divergente entre eles não seria diferença de estilo: seria metade da
//! virada aplicada e metade desfeita, com o banco salvo no meio.

use rusqlite::Connection;

/// Executa `action` dentro de um `SAVEPOINT` de nome `name`: confirma (`RELEASE`) quando
/// devolve `Ok`, desfaz (`ROLLBACK TO` + `RELEASE`) quando devolve `Err`.
///
/// `name` vai cru para o SQL — use sempre um literal do código, nunca dado do jogador.
///
/// Se o próprio rollback falhar, o erro original é preservado na mensagem: perder a causa
/// raiz num relato de bug é pior do que a mensagem ficar longa.
pub fn with_savepoint<T, F>(conn: &Connection, name: &str, action: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    conn.execute_batch(&format!("SAVEPOINT {name}"))
        .map_err(|e| format!("Falha ao abrir savepoint '{name}': {e}"))?;

    match action() {
        Ok(value) => {
            conn.execute_batch(&format!("RELEASE SAVEPOINT {name}"))
                .map_err(|e| format!("Falha ao confirmar savepoint '{name}': {e}"))?;
            Ok(value)
        }
        Err(err) => {
            conn.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {name}; RELEASE SAVEPOINT {name};"
            ))
            .map_err(|rollback_err| {
                format!("{err}; alem disso falhou o rollback do savepoint '{name}': {rollback_err}")
            })?;
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn_com_tabela() -> Connection {
        let conn = Connection::open_in_memory().expect("banco em memoria");
        conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY)")
            .expect("tabela");
        conn
    }

    fn total(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))
            .expect("contagem")
    }

    #[test]
    fn confirma_a_escrita_quando_o_bloco_da_certo() {
        let conn = conn_com_tabela();
        let valor = with_savepoint(&conn, "teste_ok", || {
            conn.execute("INSERT INTO t (id) VALUES (1)", [])
                .map_err(|e| e.to_string())?;
            Ok(42)
        })
        .expect("savepoint confirma");
        assert_eq!(valor, 42);
        assert_eq!(total(&conn), 1);
    }

    #[test]
    fn desfaz_a_escrita_quando_o_bloco_falha() {
        let conn = conn_com_tabela();
        let erro = with_savepoint(&conn, "teste_erro", || {
            conn.execute("INSERT INTO t (id) VALUES (1)", [])
                .map_err(|e| e.to_string())?;
            Err::<(), String>("estourou no meio".to_string())
        })
        .expect_err("savepoint propaga o erro");
        assert_eq!(erro, "estourou no meio");
        assert_eq!(total(&conn), 0, "a linha escrita antes do erro deve sumir");
        // E o savepoint tem de ter sido liberado: outra transacao abre normalmente.
        with_savepoint(&conn, "teste_erro", || Ok(())).expect("savepoint reabre");
    }

    #[test]
    fn savepoints_aninhados_desfazem_so_o_de_dentro() {
        let conn = conn_com_tabela();
        with_savepoint(&conn, "externo", || {
            conn.execute("INSERT INTO t (id) VALUES (1)", [])
                .map_err(|e| e.to_string())?;
            let _ = with_savepoint(&conn, "interno", || {
                conn.execute("INSERT INTO t (id) VALUES (2)", [])
                    .map_err(|e| e.to_string())?;
                Err::<(), String>("falha interna".to_string())
            });
            Ok(())
        })
        .expect("savepoint externo confirma");
        assert_eq!(total(&conn), 1, "so a escrita do savepoint interno some");
    }
}
