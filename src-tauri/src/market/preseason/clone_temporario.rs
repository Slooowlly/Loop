//! Clone temporário do banco usado pelos testes da pré-temporada (dry-run sobre
//! uma cópia descartável, sem tocar no save).

use super::*;

#[cfg(test)]
pub(super) static PRESEASON_CLONE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
pub(super) struct TempPreseasonClone {
    path: std::path::PathBuf,
    conn: Option<Connection>,
}

#[cfg(test)]
impl TempPreseasonClone {
    pub(super) fn new(source: &Connection) -> Result<Self, String> {
        let path = clone_connection_to_temp(source)?;
        let conn = Connection::open(&path)
            .map_err(|e| format!("Falha ao abrir clone temporario do banco: {e}"))?;
        Ok(Self {
            path,
            conn: Some(conn),
        })
    }

    pub(super) fn connection(&self) -> &Connection {
        self.conn
            .as_ref()
            .expect("clone temporario da preseason ja foi liberado")
    }

    #[cfg(test)]
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
impl Drop for TempPreseasonClone {
    fn drop(&mut self) {
        let _ = self.conn.take();
        if let Err(err) = cleanup_temp_db(&self.path) {
            eprintln!("Falha ao limpar clone temporario da preseason: {err}");
        }
    }
}

#[cfg(test)]
pub(super) fn clone_connection_to_temp(conn: &Connection) -> Result<std::path::PathBuf, String> {
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|e| format!("Falha ao checkpointar banco antes do clone: {e}"))?;
    let temp_path = next_preseason_clone_path()?;
    let escaped = temp_path
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{escaped}';"))
        .map_err(|e| format!("Falha ao clonar banco para planejamento da pre-temporada: {e}"))?;
    Ok(temp_path)
}

#[cfg(test)]
pub(super) fn next_preseason_clone_path() -> Result<std::path::PathBuf, String> {
    let pid = std::process::id();
    for _ in 0..64 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("Falha ao gerar timestamp do clone: {e}"))?
            .as_nanos();
        let counter = PRESEASON_CLONE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = std::env::temp_dir().join(format!(
            "iracerapp_preseason_clone_{pid}_{nanos}_{counter}.db"
        ));

        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err("Falha ao reservar caminho unico para clone temporario da pre-temporada".to_string())
}

#[cfg(test)]
pub(super) fn cleanup_temp_db(path: &Path) -> Result<(), String> {
    fn remove_if_exists(path: &Path) -> Result<(), String> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(format!(
                "Falha ao remover arquivo temporario '{}': {err}",
                path.display()
            )),
        }
    }

    remove_if_exists(path)?;
    let wal = std::path::PathBuf::from(format!("{}-wal", path.to_string_lossy()));
    let shm = std::path::PathBuf::from(format!("{}-shm", path.to_string_lossy()));
    remove_if_exists(&wal)?;
    remove_if_exists(&shm)?;
    Ok(())
}
