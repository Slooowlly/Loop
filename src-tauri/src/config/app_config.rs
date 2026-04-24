use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::commands::career_types::SaveLifecycleStatus;

// ── SaveMeta — espelha career_NNN/meta.json ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveMeta {
    pub career_number: u32,
    pub player_name: String,
    pub current_season: u32,
    pub current_year: u32,
    pub created_at: String,
    pub last_played: String,
    /// Última consolidação explícita do save (flush_save).
    #[serde(default)]
    pub last_saved: Option<String>,
    /// Último backup criado (create_season_backup).
    #[serde(default)]
    pub last_backup: Option<String>,
    #[serde(default)]
    pub team_name: Option<String>,
    pub category: String,
    pub difficulty: String,
    #[serde(default)]
    pub total_races: i32,
    #[serde(default)]
    pub lifecycle_status: SaveLifecycleStatus,
    #[serde(default)]
    pub history_start_year: Option<u32>,
    #[serde(default)]
    pub history_end_year: Option<u32>,
    #[serde(default)]
    pub playable_start_year: Option<u32>,
    #[serde(default)]
    pub draft_progress_year: Option<u32>,
    #[serde(default)]
    pub draft_error: Option<String>,
    #[serde(default)]
    pub pending_player_nationality: Option<String>,
    #[serde(default)]
    pub pending_player_age: Option<i32>,
}

// ── AppConfig — espelha config.json ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub version: String,
    pub last_career: Option<u32>,
    pub language: String,
    pub autosave_enabled: bool,

    // Window state
    pub window_width: u32,
    pub window_height: u32,
    pub window_maximized: bool,

    // iRacing Paths
    pub airosters_path: Option<PathBuf>,
    pub aiseasons_path: Option<PathBuf>,

    /// Diretório base do app (AppData/Local/Loop).
    /// Não persiste no JSON — preenchido em tempo de execução.
    #[serde(skip)]
    pub base_dir: PathBuf,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            version: "1.0.0".to_string(),
            last_career: None,
            language: "pt-BR".to_string(),
            autosave_enabled: true,
            window_width: 1280,
            window_height: 720,
            window_maximized: false,
            airosters_path: None,
            aiseasons_path: None,
            base_dir: PathBuf::new(),
        }
    }
}

impl AppConfig {
    // ── Carregar ou criar padrão ──────────────────────────────────────────────

    pub fn load_or_default(base_dir: &Path) -> Self {
        let path = base_dir.join("config.json");
        if let Ok(content) = std::fs::read_to_string(&path) {
            match serde_json::from_str::<AppConfig>(&content) {
                Ok(mut cfg) => {
                    cfg.base_dir = base_dir.to_path_buf();
                    return cfg;
                }
                Err(e) => {
                    eprintln!("[config] config.json corrompido: {e}. Fazendo backup e usando configuração padrão.");
                    let backup = path.with_extension("json.bak");
                    let _ = std::fs::copy(&path, &backup);
                }
            }
        }
        let mut cfg = AppConfig::default();
        cfg.base_dir = base_dir.to_path_buf();
        cfg
    }

    /// Persistir config.json no disco.
    pub fn save(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.base_dir)
            .map_err(|e| format!("Falha ao criar diretório base: {e}"))?;
        let path = self.base_dir.join("config.json");
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Falha ao serializar config: {e}"))?;
        std::fs::write(&path, json).map_err(|e| format!("Falha ao gravar config.json: {e}"))
    }

    // ── Helpers de caminho ────────────────────────────────────────────────────

    pub fn saves_dir(&self) -> PathBuf {
        self.base_dir.join("saves")
    }

    pub fn career_dir(&self, career_number: u32) -> PathBuf {
        self.saves_dir()
            .join(format!("career_{:03}", career_number))
    }

    pub fn career_db_path(&self, career_number: u32) -> PathBuf {
        self.career_dir(career_number).join("career.db")
    }

    pub fn career_meta_path(&self, career_number: u32) -> PathBuf {
        self.career_dir(career_number).join("meta.json")
    }

    /// Retorna o próximo número de carreira disponível (max existente + 1).
    #[allow(dead_code)]
    pub fn next_career_number(&self) -> u32 {
        let saves = self.saves_dir();
        if !saves.exists() {
            return 1;
        }
        let max = std::fs::read_dir(&saves)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let name = e.file_name();
                        let s = name.to_string_lossy();
                        if s.starts_with("career_") {
                            s[7..].parse::<u32>().ok()
                        } else {
                            None
                        }
                    })
                    .max()
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        max + 1
    }

    /// Lista todos os saves existentes lendo cada meta.json.
    pub fn list_saves(&self) -> Vec<SaveMeta> {
        let saves = self.saves_dir();
        if !saves.exists() {
            return Vec::new();
        }
        let mut result = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&saves) {
            for entry in entries.filter_map(|e| e.ok()) {
                let meta_path = entry.path().join("meta.json");
                if let Ok(content) = std::fs::read_to_string(&meta_path) {
                    if let Ok(meta) = serde_json::from_str::<SaveMeta>(&content) {
                        if meta.lifecycle_status == SaveLifecycleStatus::Active {
                            result.push(meta);
                        }
                    }
                }
            }
        }
        result.sort_by(|a, b| b.last_played.cmp(&a.last_played));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_temp_base_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("iracer_app_config_{label}_{unique}"));
        fs::create_dir_all(&dir).expect("create temp config dir");
        dir
    }

    fn write_save_meta(
        base_dir: &PathBuf,
        career_id: &str,
        player_name: &str,
        lifecycle: Option<&str>,
    ) {
        let career_dir = base_dir.join("saves").join(career_id);
        fs::create_dir_all(&career_dir).expect("create career dir");

        let lifecycle_field = lifecycle
            .map(|status| format!(r#","lifecycle_status":"{status}""#))
            .unwrap_or_default();
        let meta = format!(
            r#"{{
                "career_number": 1,
                "player_name": "{player_name}",
                "current_season": 1,
                "current_year": 2025,
                "created_at": "2026-04-24T10:00:00",
                "last_played": "2026-04-24T10:00:00",
                "team_name": "Equipe Teste",
                "category": "mazda_rookie",
                "difficulty": "medio",
                "total_races": 58
                {lifecycle_field}
            }}"#
        );
        fs::write(career_dir.join("meta.json"), meta).expect("write save meta");
    }

    #[test]
    fn list_saves_treats_missing_lifecycle_as_active() {
        let base_dir = create_temp_base_dir("legacy_save_lifecycle");
        write_save_meta(&base_dir, "career_001", "Piloto Legado", None);

        let config = AppConfig::load_or_default(&base_dir);
        let saves = config.list_saves();

        assert_eq!(saves.len(), 1);
        assert_eq!(saves[0].player_name, "Piloto Legado");

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn list_saves_excludes_draft_and_failed_saves() {
        let base_dir = create_temp_base_dir("draft_save_filter");
        write_save_meta(&base_dir, "career_001", "Piloto Ativo", Some("active"));
        write_save_meta(&base_dir, "career_002", "Piloto Draft", Some("draft"));
        write_save_meta(&base_dir, "career_003", "Piloto Falho", Some("failed"));

        let config = AppConfig::load_or_default(&base_dir);
        let saves = config.list_saves();

        assert_eq!(saves.len(), 1);
        assert_eq!(saves[0].player_name, "Piloto Ativo");

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn load_or_default_accepts_legacy_partial_config() {
        let base_dir = create_temp_base_dir("legacy_partial");
        let config_path = base_dir.join("config.json");
        fs::write(
            &config_path,
            r#"{
                "version": "0.9.0",
                "last_career": 7,
                "language": "en-US",
                "autosave_enabled": false
            }"#,
        )
        .expect("write legacy config");

        let loaded = AppConfig::load_or_default(&base_dir);

        assert_eq!(loaded.version, "0.9.0");
        assert_eq!(loaded.last_career, Some(7));
        assert_eq!(loaded.language, "en-US");
        assert!(!loaded.autosave_enabled);
        assert_eq!(loaded.window_width, 1280);
        assert_eq!(loaded.window_height, 720);
        assert!(!loaded.window_maximized);
        assert_eq!(loaded.base_dir, base_dir);
        assert!(
            !base_dir.join("config.json.bak").exists(),
            "config parcial compatível não deve ser tratado como corrompido"
        );
    }

    #[test]
    fn load_or_default_accepts_partial_window_state_config() {
        let base_dir = create_temp_base_dir("partial_window");
        let config_path = base_dir.join("config.json");
        fs::write(
            &config_path,
            r#"{
                "version": "1.0.0",
                "last_career": 12,
                "language": "pt-BR",
                "autosave_enabled": true,
                "window_width": 1600
            }"#,
        )
        .expect("write partial window config");

        let loaded = AppConfig::load_or_default(&base_dir);

        assert_eq!(loaded.last_career, Some(12));
        assert_eq!(loaded.window_width, 1600);
        assert_eq!(loaded.window_height, 720);
        assert!(!loaded.window_maximized);
    }
}
