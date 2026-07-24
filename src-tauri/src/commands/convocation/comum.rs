//! Resolucao do caminho do banco da carreira usado por toda a casca de convocacao.

use super::*;

pub(super) fn career_db_path(base_dir: &Path, career_id: &str) -> PathBuf {
    let config = AppConfig::load_or_default(base_dir);
    config.saves_dir().join(career_id).join("career.db")
}
