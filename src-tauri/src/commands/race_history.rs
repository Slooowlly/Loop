use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::simulation::incidents::{IncidentSeverity, IncidentType};
use crate::simulation::race::RaceDriverResult;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoundResult {
    pub position: i32,
    pub is_dnf: bool,
    #[serde(default)]
    pub has_fastest_lap: bool,
    #[serde(default)]
    pub grid_position: i32,
    #[serde(default)]
    pub positions_gained: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriverRaceHistory {
    pub driver_id: String,
    pub results: Vec<Option<RoundResult>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrophyInfo {
    pub tipo: String,
    pub temporada: i32,
    pub is_defending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConstructorChampion {
    pub team_id: String,
    pub titles: i32,
    pub is_defending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreviousChampions {
    pub driver_champion_id: Option<String>,
    pub constructor_champions: Vec<ConstructorChampion>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RaceHistoryStore {
    #[serde(default)]
    categories: HashMap<String, CategoryRaceHistory>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CategoryRaceHistory {
    #[serde(default)]
    rounds: HashMap<i32, Vec<StoredRoundResult>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredIncident {
    pub incident_type: IncidentType,
    pub severity: IncidentSeverity,
    pub segment: String,
    pub positions_lost: i32,
    pub is_dnf: bool,
    pub description: String,
    #[serde(default)]
    pub catalog_id: Option<String>,
    #[serde(default)]
    pub damage_origin_segment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredRoundResult {
    driver_id: String,
    #[serde(default)]
    position: i32,
    #[serde(default)]
    is_dnf: bool,
    #[serde(default)]
    has_fastest_lap: bool,
    #[serde(default)]
    grid_position: i32,
    #[serde(default)]
    positions_gained: i32,
    #[serde(default)]
    pub dnf_reason: Option<String>,
    #[serde(default)]
    pub dnf_segment: Option<String>,
    #[serde(default)]
    pub incidents_count: i32,
    #[serde(default)]
    pub incidents: Vec<StoredIncident>,
    #[serde(default)]
    pub dnf_catalog_id: Option<String>,
}

pub fn append_race_result(
    career_dir: &Path,
    category: &str,
    round: i32,
    race_results: &[RaceDriverResult],
) -> Result<(), String> {
    let mut store = read_race_history(career_dir)?;
    let category_history = store.categories.entry(category.to_string()).or_default();

    category_history.rounds.insert(
        round,
        race_results
            .iter()
            .map(|entry| StoredRoundResult {
                driver_id: entry.pilot_id.clone(),
                position: entry.finish_position,
                is_dnf: entry.is_dnf,
                has_fastest_lap: entry.has_fastest_lap,
                grid_position: entry.grid_position,
                positions_gained: entry.positions_gained,
                dnf_reason: entry.dnf_reason.clone(),
                dnf_segment: entry.dnf_segment.clone(),
                incidents_count: entry.incidents_count,
                incidents: entry
                    .incidents
                    .iter()
                    .map(|inc| StoredIncident {
                        incident_type: inc.incident_type,
                        severity: inc.severity,
                        segment: inc.segment.clone(),
                        positions_lost: inc.positions_lost,
                        is_dnf: inc.is_dnf,
                        description: inc.description.clone(),
                        catalog_id: inc.catalog_id.clone(),
                        damage_origin_segment: inc.damage_origin_segment.clone(),
                    })
                    .collect(),
                dnf_catalog_id: entry.dnf_catalog_id.clone(),
            })
            .collect(),
    );

    write_race_history(career_dir, &store)
}

pub fn build_driver_histories(
    career_dir: &Path,
    category: &str,
    total_rounds: usize,
    driver_ids: &[String],
) -> Result<Vec<DriverRaceHistory>, String> {
    let store = read_race_history(career_dir)?;
    let category_history = store.categories.get(category);

    Ok(driver_ids
        .iter()
        .map(|driver_id| {
            let mut results = Vec::with_capacity(total_rounds);
            for round in 1..=total_rounds {
                let round_result = category_history
                    .and_then(|history| history.rounds.get(&(round as i32)))
                    .and_then(|entries| entries.iter().find(|entry| entry.driver_id == *driver_id))
                    .map(|entry| RoundResult {
                        position: entry.position,
                        is_dnf: entry.is_dnf,
                        has_fastest_lap: entry.has_fastest_lap,
                        grid_position: entry.grid_position,
                        positions_gained: entry.positions_gained,
                    });
                results.push(round_result);
            }

            DriverRaceHistory {
                driver_id: driver_id.clone(),
                results,
            }
        })
        .collect())
}

pub fn empty_previous_champions() -> PreviousChampions {
    PreviousChampions {
        driver_champion_id: None,
        constructor_champions: Vec::new(),
    }
}

fn read_race_history(career_dir: &Path) -> Result<RaceHistoryStore, String> {
    let path = race_history_path(career_dir);
    if !path.exists() {
        return Ok(RaceHistoryStore::default());
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Falha ao ler race_results.json: {e}"))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Falha ao interpretar race_results.json: {e}"))
}

fn write_race_history(career_dir: &Path, store: &RaceHistoryStore) -> Result<(), String> {
    let path = race_history_path(career_dir);
    let temp_path = race_history_temp_path(career_dir);
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Falha ao serializar race_results.json: {e}"))?;
    std::fs::write(&temp_path, json)
        .map_err(|e| format!("Falha ao gravar race_results.json: {e}"))?;

    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| format!("Falha ao substituir race_results.json: {e}"))?;
    }

    std::fs::rename(&temp_path, &path)
        .map_err(|e| format!("Falha ao finalizar race_results.json: {e}"))
}

fn race_history_path(career_dir: &Path) -> PathBuf {
    career_dir.join("race_results.json")
}

fn race_history_temp_path(career_dir: &Path) -> PathBuf {
    career_dir.join("race_results.json.tmp")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_career_dir(test_name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "iracer_race_history_{}_{}_{}",
            test_name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn write_race_history_preserves_existing_file_when_staged_write_fails() {
        let career_dir = temp_career_dir("preserve_existing");
        let history_path = race_history_path(&career_dir);
        let original =
            r#"{"categories":{"gt4":{"rounds":{"1":[{"driver_id":"P001","position":1}]}}}}"#;
        std::fs::write(&history_path, original).unwrap();

        let temp_path = career_dir.join("race_results.json.tmp");
        std::fs::create_dir_all(&temp_path).unwrap();

        let store = RaceHistoryStore::default();
        let err = write_race_history(&career_dir, &store).expect_err("staged write should fail");
        assert!(err.contains("Falha ao gravar race_results.json"));

        let preserved = std::fs::read_to_string(&history_path).unwrap();
        assert_eq!(preserved, original);

        std::fs::remove_dir_all(&career_dir).unwrap();
    }
}
