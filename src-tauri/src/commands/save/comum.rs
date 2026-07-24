//! Utilitarios compartilhados do save: checkpoint do WAL, atualizacao dos carimbos
//! de tempo em meta.json e parsing do identificador da carreira.

use super::*;

pub(super) fn checkpoint_wal(db: &Database) -> Result<(), String> {
    db.conn
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|e| format!("Falha no WAL checkpoint: {e}"))
}

pub(super) fn update_meta_timestamps<F>(meta_path: &Path, mutate: F) -> Result<(), String>
where
    F: FnOnce(&mut SaveMeta),
{
    let content =
        std::fs::read_to_string(meta_path).map_err(|e| format!("Falha ao ler meta.json: {e}"))?;
    let mut meta: SaveMeta =
        serde_json::from_str(&content).map_err(|e| format!("Falha ao parsear meta.json: {e}"))?;
    mutate(&mut meta);
    let updated = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Falha ao serializar meta.json: {e}"))?;
    std::fs::write(meta_path, updated).map_err(|e| format!("Falha ao gravar meta.json: {e}"))
}

pub(crate) fn parse_career_number(career_id: &str) -> Result<u32, String> {
    let s = career_id.trim_start_matches("career_");
    s.parse::<u32>()
        .map_err(|_| format!("career_id invalido: '{career_id}'"))
}
