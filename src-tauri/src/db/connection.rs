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
    pub fn backup(&self, dest: &Path) -> Result<(), DbError> {
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
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
