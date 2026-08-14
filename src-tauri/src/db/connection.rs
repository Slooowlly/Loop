use rusqlite::Connection;
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::db::migrations;

// ── Tipo de erro do banco ─────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Migration error: {0}")]
    Migration(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("WAL checkpoint não concluído: {0}")]
    WalBusy(String),
}

/// DbError precisa ser Serialize para ser retornado em comandos Tauri.
impl Serialize for DbError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Struct principal ──────────────────────────────────────────────────────────

pub struct Database {
    pub conn: Connection,
    pub path: PathBuf,
}

impl Database {
    // ── Construtores ──────────────────────────────────────────────────────────

    /// Cria um banco novo no caminho especificado e aplica todas as migrações.
    pub fn create_new(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        apply_pragmas(&conn)?;
        migrations::run_all(&conn)?;
        Ok(Database {
            conn,
            path: path.to_path_buf(),
        })
    }

    /// Abre um banco existente e aplica migrações pendentes.
    pub fn open_existing(path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open(path)?;
        apply_pragmas(&conn)?;
        migrations::run_pending(&conn)?;
        Ok(Database {
            conn,
            path: path.to_path_buf(),
        })
    }

    // ── Backup ────────────────────────────────────────────────────────────────

    /// Copia o arquivo .db para o destino indicado.
    /// Faz checkpoint do WAL antes para garantir consistência.
    ///
    /// A cópia leva só o arquivo principal, sem o `-wal`. Se o checkpoint não drenar
    /// o WAL, o backup nasce sem os últimos commits, então um checkpoint ocupado
    /// aborta o backup em vez de produzir um arquivo incompleto.
    pub fn backup(&self, dest: &Path) -> Result<(), DbError> {
        checkpoint_wal_truncate(&self.conn)?;
        std::fs::copy(&self.path, dest)?;
        Ok(())
    }

    // ── Transação ─────────────────────────────────────────────────────────────

    /// Executa uma função dentro de uma transação SQLite.
    /// Faz commit ao sucesso e rollback automático em caso de erro.
    ///
    /// Usa BEGIN IMMEDIATE: o lock de escrita é adquirido já no início,
    /// serializando escritores concorrentes via busy_timeout. Com BEGIN
    /// DEFERRED (padrão), uma transação que lê antes de escrever falharia
    /// com SQLITE_BUSY_SNAPSHOT ("database is locked") imediatamente — sem
    /// retry — se outra conexão comitasse entre a leitura e a escrita.
    pub fn transaction<T, F>(&mut self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&rusqlite::Transaction) -> Result<T, DbError>,
    {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }
}

// ── Checkpoint do WAL ─────────────────────────────────────────────────────────

/// Tentativas do checkpoint TRUNCATE antes de declarar o WAL ocupado.
const WAL_CHECKPOINT_TENTATIVAS: u32 = 3;
/// Espera entre as tentativas, em milissegundos.
const WAL_CHECKPOINT_ESPERA_MS: u64 = 50;

/// Drena o WAL para dentro do .db e trunca o arquivo de log.
///
/// `PRAGMA wal_checkpoint(TRUNCATE)` devolve a linha `(busy, log, checkpointed)`.
/// `busy = 1` significa que o checkpoint não conseguiu o lock e o WAL continua com
/// páginas que não entraram no .db. Quem copiar o .db depois disso leva um arquivo
/// sem os últimos commits. `execute_batch` engolia essa linha, então o caminho de
/// backup tratava um checkpoint ocupado como flush concluído.
///
/// A conexão já tem `busy_timeout=15000`, que cobre a espera curta por um leitor.
/// O retry aqui é o complemento explícito e limitado para o caso de o timeout
/// estourar com o leitor prestes a sair.
pub fn checkpoint_wal_truncate(conn: &Connection) -> Result<(), DbError> {
    let mut ultimo_busy = 0i64;

    for tentativa in 0..WAL_CHECKPOINT_TENTATIVAS {
        let busy: i64 = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))?;
        if busy == 0 {
            return Ok(());
        }

        ultimo_busy = busy;
        if tentativa + 1 < WAL_CHECKPOINT_TENTATIVAS {
            std::thread::sleep(std::time::Duration::from_millis(WAL_CHECKPOINT_ESPERA_MS));
        }
    }

    Err(DbError::WalBusy(format!(
        "o WAL continuou ocupado após {WAL_CHECKPOINT_TENTATIVAS} tentativas (busy={ultimo_busy}); \
         o banco não foi drenado e a cópia ficaria sem os últimos commits"
    )))
}

// ── PRAGMAs obrigatórios ──────────────────────────────────────────────────────

fn apply_pragmas(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA foreign_keys=ON;
         PRAGMA busy_timeout=15000;",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_temporario(rotulo: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("relógio")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("iracer_wal_{rotulo}_{nanos}"));
        std::fs::create_dir_all(&dir).expect("dir temporário");
        dir
    }

    fn abre_wal(path: &Path, busy_timeout_ms: u32) -> Connection {
        let conn = Connection::open(path).expect("abrir banco");
        conn.execute_batch(&format!(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA busy_timeout={busy_timeout_ms};"
        ))
        .expect("pragmas");
        conn
    }

    #[test]
    fn checkpoint_wal_truncate_drena_o_wal_quando_ninguem_segura_o_lock() {
        let dir = dir_temporario("livre");
        let db_path = dir.join("teste.db");
        let conn = abre_wal(&db_path, 15_000);
        conn.execute_batch("CREATE TABLE t (id INTEGER); INSERT INTO t VALUES (1), (2);")
            .expect("semear dados");

        checkpoint_wal_truncate(&conn).expect("checkpoint deve concluir");

        let wal = dir.join("teste.db-wal");
        let tamanho_wal = std::fs::metadata(&wal).map(|m| m.len()).unwrap_or(0);
        assert_eq!(tamanho_wal, 0, "TRUNCATE deve zerar o -wal");

        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// O ponto do teste: um leitor com transação aberta bloqueia o TRUNCATE, o PRAGMA
    /// devolve `busy = 1` e isso tem que virar erro. Antes o retorno era descartado e o
    /// backup seguia copiando um .db sem as páginas que ainda estavam no WAL.
    #[test]
    fn checkpoint_wal_truncate_falha_quando_o_wal_esta_ocupado() {
        let dir = dir_temporario("ocupado");
        let db_path = dir.join("teste.db");

        // busy_timeout zerado: sem espera, o leitor concorrente basta para dar busy.
        let escritor = abre_wal(&db_path, 0);
        escritor
            .execute_batch("CREATE TABLE t (id INTEGER); INSERT INTO t VALUES (1), (2);")
            .expect("semear dados");

        let leitor = abre_wal(&db_path, 0);
        leitor
            .execute_batch("BEGIN; SELECT COUNT(*) FROM t;")
            .expect("abrir transação de leitura");

        let erro = checkpoint_wal_truncate(&escritor)
            .expect_err("checkpoint com leitor aberto deve ser reportado como ocupado");
        assert!(
            matches!(erro, DbError::WalBusy(_)),
            "esperava WalBusy, veio {erro:?}"
        );

        leitor.execute_batch("ROLLBACK;").expect("fechar leitura");
        drop(leitor);
        drop(escritor);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Com o WAL ocupado, `backup` não pode gerar arquivo nenhum: um .db copiado sem as
    /// páginas do WAL é um backup silenciosamente incompleto.
    #[test]
    fn backup_aborta_quando_o_checkpoint_fica_ocupado() {
        let dir = dir_temporario("backup_ocupado");
        let db_path = dir.join("carreira.db");
        let destino = dir.join("copia.db");

        let db = Database::create_new(&db_path).expect("criar banco");
        db.conn
            .execute_batch("PRAGMA busy_timeout=0;")
            .expect("zerar busy_timeout");

        let leitor = abre_wal(&db_path, 0);
        leitor
            .execute_batch("BEGIN; SELECT COUNT(*) FROM sqlite_master;")
            .expect("abrir transação de leitura");

        let erro = db
            .backup(&destino)
            .expect_err("backup com WAL ocupado deve falhar");
        assert!(matches!(erro, DbError::WalBusy(_)));
        assert!(
            !destino.exists(),
            "backup abortado não pode deixar cópia parcial"
        );

        leitor.execute_batch("ROLLBACK;").expect("fechar leitura");
        drop(leitor);
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
