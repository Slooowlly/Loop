//! Salvar / carregar corridas do monitor e a percepção de rivalidades sobre elas.

use super::*;

// ─── Salvar / carregar corridas ──────────────────────────────────────────────

/// Resumo de uma corrida salva (para a lista no painel).
#[derive(serde::Serialize)]
pub struct SavedRaceInfo {
    pub name: String,
    pub laps: usize,
    pub track_points: usize,
    pub outcome: String,
    pub size_kb: u64,
    pub modified: String,
}

/// Garante o diretório `app_data_dir/iracing_races` e o devolve.
fn races_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?
        .join("iracing_races");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    Ok(dir)
}

/// Salva o histórico atual da corrida num arquivo JSON e devolve o nome.
#[tauri::command]
pub fn iracing_save_race_history(app: tauri::AppHandle) -> Result<String, String> {
    let history = race_monitor::get_history();
    if history.laps.is_empty() && history.player_laps.is_empty() && history.player_track.is_empty()
    {
        return Err("Nada para salvar ainda — a corrida não gerou dados.".to_string());
    }
    let dir = races_dir(&app)?;
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let name = format!("corrida_t{}_{}.json", history.attempt_number, stamp);
    let json = serde_json::to_string(&history).map_err(|e| format!("Falha ao serializar: {e}"))?;
    std::fs::write(dir.join(&name), json).map_err(|e| format!("Falha ao gravar: {e}"))?;
    Ok(name)
}

/// Lista as corridas salvas (mais recentes primeiro).
#[tauri::command]
pub fn iracing_list_saved_races(app: tauri::AppHandle) -> Result<Vec<SavedRaceInfo>, String> {
    let dir = races_dir(&app)?;
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| format!("Falha ao ler pasta: {e}"))? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let meta = entry.metadata().ok();
        let size_kb = meta.as_ref().map(|m| m.len() / 1024).unwrap_or(0);
        let modified = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let dt: chrono::DateTime<chrono::Local> = t.into();
                dt.format("%d/%m %H:%M").to_string()
            })
            .unwrap_or_default();
        let (laps, track_points, outcome) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<RaceHistory>(&s).ok())
            .map(|h| (h.laps.len(), h.player_track.len(), h.outcome))
            .unwrap_or((0, 0, String::new()));
        out.push(SavedRaceInfo {
            name,
            laps,
            track_points,
            outcome,
            size_kb,
            modified,
        });
    }
    out.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(out)
}

/// Carrega uma corrida salva pelo nome do arquivo.
#[tauri::command]
pub fn iracing_load_saved_race(app: tauri::AppHandle, name: String) -> Result<RaceHistory, String> {
    let dir = races_dir(&app)?;
    // Evita path traversal: usa só o nome do arquivo.
    let file = std::path::Path::new(&name)
        .file_name()
        .ok_or("Nome de arquivo inválido")?;
    let path = dir.join(file);
    let s = std::fs::read_to_string(&path).map_err(|e| format!("Falha ao ler: {e}"))?;
    serde_json::from_str(&s).map_err(|e| format!("Falha ao interpretar: {e}"))
}

/// PERCEPÇÃO de rivalidade (calibração/debug): roda o motor de percepção sobre uma
/// corrida — ao vivo (`saved_name = None`) ou salva — para um CARRO-SONDA
/// (`probe_car_idx`; default = o jogador). Devolve o livro-razão por oponente SEM
/// aplicar nada no motor de rivalidade. Contato atribuído só é resolvido no caso
/// ao-vivo + probe-jogador (para uma IA-sonda vem vazio, como projetado).
///
/// Ver `docs/superpowers/specs/2026-07-18-track-rivalry-perception-design.md`.
#[tauri::command]
pub fn iracing_perceive_rivalries(
    app: tauri::AppHandle,
    saved_name: Option<String>,
    probe_car_idx: Option<i32>,
) -> Result<crate::iracing_sdk::rivalry_perception::RivalryPerception, String> {
    use crate::iracing_sdk::rivalry_perception::{
        perceive_rivalries, ContactSeed, ContactTier, PerceptionParams,
    };

    let history = match &saved_name {
        Some(name) => iracing_load_saved_race(app, name.clone())?,
        None => race_monitor::get_history(),
    };
    if history.laps.is_empty() {
        return Err("Sem trace de campo — a corrida não gerou voltas.".to_string());
    }

    let probe = probe_car_idx.unwrap_or(history.player_car_idx);

    // Contato só quando analisamos a corrida AO VIVO e o probe é o jogador.
    let contact: Option<ContactSeed> = if saved_name.is_none() && probe == history.player_car_idx {
        let status = race_monitor::poll();
        let attempt = status
            .attempts
            .iter()
            .find(|a| a.number == status.attempt_number)
            .or_else(|| status.attempts.last());
        attempt.and_then(|a| {
            a.collided_with_car_number.and_then(|num| {
                history
                    .cars_meta
                    .iter()
                    .find(|m| m.car_number == num)
                    .map(|m| {
                        // Tier fino (crítico/leve) fica pra fase de calibração; por ora
                        // DNF vs. contato grave é o suficiente.
                        let tier = if a.evidence.raced && !a.evidence.reached_checkered {
                            ContactTier::Dnf
                        } else {
                            ContactTier::Major
                        };
                        ContactSeed {
                            opponent_car_idx: m.idx,
                            tier,
                        }
                    })
            })
        })
    } else {
        None
    };

    Ok(perceive_rivalries(
        &history,
        probe,
        contact,
        &PerceptionParams::default(),
    ))
}
