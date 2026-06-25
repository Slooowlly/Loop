// Funções de simulação/mercado carregam muitos parâmetros de domínio e alguns
// retornos de query agregam tuplas amplas; preferimos manter as assinaturas
// explícitas a fragmentá-las só para satisfazer o lint de estilo.
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
// O `json!` da AI season monta um objeto grande (~45 chaves) — eleva o limite.
#![recursion_limit = "256"]

use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tauri::Manager;

const RESIZE_DEBOUNCE_MS: u64 = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
struct WindowStateSnapshot {
    width: Option<u32>,
    height: Option<u32>,
    maximized: bool,
}

impl WindowStateSnapshot {
    fn resized(width: f64, height: f64) -> Self {
        Self {
            width: Some(normalize_dimension(width)),
            height: Some(normalize_dimension(height)),
            maximized: false,
        }
    }

    fn maximized() -> Self {
        Self {
            width: None,
            height: None,
            maximized: true,
        }
    }
}

#[derive(Debug, Default)]
struct ResizeDebounceInner {
    generation: AtomicU64,
    latest: Mutex<Option<WindowStateSnapshot>>,
}

#[derive(Debug, Clone, Default)]
struct ResizeDebounceState {
    inner: Arc<ResizeDebounceInner>,
}

impl ResizeDebounceState {
    fn schedule(&self, snapshot: WindowStateSnapshot) -> u64 {
        let generation = self.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;

        match self.inner.latest.lock() {
            Ok(mut latest) => {
                *latest = Some(snapshot);
            }
            Err(error) => {
                eprintln!("[window] Falha ao registrar resize pendente: {error}");
            }
        }

        generation
    }

    fn latest_if_current(&self, generation: u64) -> Option<WindowStateSnapshot> {
        if self.inner.generation.load(Ordering::SeqCst) != generation {
            return None;
        }

        match self.inner.latest.lock() {
            Ok(latest) => latest.clone(),
            Err(error) => {
                eprintln!("[window] Falha ao ler resize pendente: {error}");
                None
            }
        }
    }
}

fn normalize_dimension(value: f64) -> u32 {
    value.round().max(1.0) as u32
}

fn persist_window_snapshot(base_dir: &Path, snapshot: &WindowStateSnapshot) -> Result<(), String> {
    let mut config = config::app_config::AppConfig::load_or_default(base_dir);
    config.window_maximized = snapshot.maximized;

    if let Some(width) = snapshot.width {
        config.window_width = width;
    }

    if let Some(height) = snapshot.height {
        config.window_height = height;
    }

    config.save()
}

fn schedule_resize_persist(
    base_dir: PathBuf,
    debounce: ResizeDebounceState,
    snapshot: WindowStateSnapshot,
) {
    let generation = debounce.schedule(snapshot);

    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(Duration::from_millis(RESIZE_DEBOUNCE_MS));

        if let Some(snapshot) = debounce.latest_if_current(generation) {
            if let Err(error) = persist_window_snapshot(&base_dir, &snapshot) {
                eprintln!("[window] Falha ao salvar resize: {error}");
            }
        }
    });
}

fn snapshot_from_resize_event<R: tauri::Runtime>(
    window: &tauri::Window<R>,
    size: tauri::PhysicalSize<u32>,
) -> Result<WindowStateSnapshot, String> {
    let is_maximized = window
        .is_maximized()
        .map_err(|error| format!("Falha ao ler estado maximizado: {error}"))?;

    if is_maximized {
        Ok(WindowStateSnapshot::maximized())
    } else {
        let scale_factor = window
            .scale_factor()
            .map_err(|error| format!("Falha ao ler escala da janela: {error}"))?;
        let logical_size = size.to_logical::<f64>(scale_factor);
        Ok(WindowStateSnapshot::resized(
            logical_size.width,
            logical_size.height,
        ))
    }
}

fn snapshot_from_window<R: tauri::Runtime>(
    window: &tauri::Window<R>,
) -> Result<WindowStateSnapshot, String> {
    let size = window
        .inner_size()
        .map_err(|error| format!("Falha ao ler tamanho atual da janela: {error}"))?;
    snapshot_from_resize_event(window, size)
}

// Modulos do sistema
mod calendar;
mod commands;
mod common;
mod config;
mod constants;
mod convocation;
mod db;
mod event_interest;
mod evolution;
mod finance;
mod generators;
mod hierarchy;
mod iracing_sdk;
mod market;
mod models;
mod narrative;
mod news;
mod promotion;
mod public_presence;
mod race_eval;
mod rivalry;
mod simulation;
mod world;

#[cfg(test)]
mod tests_9d;

#[cfg(test)]
mod sim_stats;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let base_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
            let config = config::app_config::AppConfig::load_or_default(&base_dir);

            if let Some(window) = app.get_webview_window("main") {
                if let Err(error) = window.set_size(tauri::LogicalSize::new(
                    config.window_width,
                    config.window_height,
                )) {
                    eprintln!("[window] Falha ao restaurar tamanho: {error}");
                }

                if config.window_maximized {
                    if let Err(error) = window.maximize() {
                        eprintln!("[window] Falha ao restaurar maximizacao: {error}");
                    }
                }
            }

            app.manage(ResizeDebounceState::default());

            // Liga o "vigia" do iRacing já no boot: ocioso (1 Hz) enquanto o sim
            // está fechado, 60 Hz quando conecta. Assim telemetria/monitor/custid
            // ativam sozinhos, sem toggle.
            iracing_sdk::race_monitor::start_watching();

            Ok(())
        })
        .on_window_event(|window, event| {
            let app = window.app_handle();
            let base_dir = match app.path().app_data_dir() {
                Ok(path) => path,
                Err(error) => {
                    eprintln!("[window] Falha ao obter app_data_dir: {error}");
                    return;
                }
            };

            match event {
                tauri::WindowEvent::Resized(size) => {
                    let debounce = app.state::<ResizeDebounceState>().inner().clone();

                    match snapshot_from_resize_event(window, *size) {
                        Ok(snapshot) => schedule_resize_persist(base_dir, debounce, snapshot),
                        Err(error) => eprintln!("[window] Falha ao capturar resize: {error}"),
                    }
                }
                tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed => {
                    match snapshot_from_window(window) {
                        Ok(snapshot) => {
                            if let Err(error) = persist_window_snapshot(&base_dir, &snapshot) {
                                eprintln!("[window] Falha ao persistir estado final: {error}");
                            }
                        }
                        Err(error) => {
                            eprintln!("[window] Falha ao capturar estado final: {error}");
                        }
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::config::get_config,
            commands::config::update_config,
            commands::career_commands::create_career,
            commands::career_commands::create_historical_career_draft,
            commands::career_commands::get_career_draft,
            commands::career_commands::discard_career_draft,
            commands::career_commands::finalize_career_draft,
            commands::career_commands::load_career,
            commands::career_commands::advance_season,
            commands::career_commands::skip_all_pending_races,
            commands::career_commands::advance_market_week,
            commands::career_commands::get_preseason_state,
            commands::career_commands::finalize_preseason,
            commands::career_commands::set_career_resume_context,
            commands::career_commands::get_player_proposals,
            commands::career_commands::respond_to_proposal,
            commands::career_commands::get_news,
            commands::career_commands::delete_career,
            commands::career_commands::list_saves,
            commands::career_commands::get_drivers_by_category,
            commands::career_commands::get_teams_standings,
            commands::career_commands::get_team_history_dossier,
            commands::career_commands::get_race_results_by_category,
            commands::career_commands::get_previous_champions,
            commands::career_commands::get_calendar_for_category,
            commands::career_commands::get_driver,
            commands::career_commands::get_driver_detail,
            commands::career_commands::get_global_driver_rankings,
            commands::career_commands::toggle_driver_favorite,
            commands::career_commands::get_transfer_window_state,
            commands::career_commands::advance_transfer_window,
            commands::career_commands::get_global_team_history,
            commands::career_commands::get_briefing_phrase_history,
            commands::career_commands::save_briefing_phrase_history,
            commands::career_commands::get_preseason_free_agents,
            commands::news_tab::get_news_tab_bootstrap,
            commands::news_tab::get_news_tab_snapshot,
            commands::ai_news::enrich_race_news_ai,
            commands::ai_news::pre_race_briefing_ai,
            commands::ai_news::report_pre_race_engagement,
            commands::ai_news::post_race_debrief_ai,
            commands::ai_news::player_race_news_id,
            commands::race::simulate_race_weekend,
            commands::race::get_saved_race_screen,
            commands::race::simulate_special_block,
            commands::iracing::iracing_read_session,
            commands::iracing::iracing_read_telemetry,
            commands::iracing::iracing_player_custid,
            commands::iracing::iracing_poll_race,
            commands::iracing::iracing_reset_race,
            commands::iracing::iracing_connected,
            commands::iracing::iracing_get_race_history,
            commands::iracing::iracing_get_race_feedback,
            commands::iracing::iracing_car_colors,
            commands::iracing::get_race_weather_timeline,
            commands::inbox::get_inbox_messages,
            commands::iracing::iracing_focus_window,
            commands::iracing::iracing_focus_self_if_closed,
            commands::iracing::iracing_launch_ui,
            commands::iracing::iracing_generate_roster,
            commands::iracing::iracing_generate_season,
            commands::iracing::iracing_export_rain_test,
            commands::iracing::iracing_process_race_result,
            commands::iracing::iracing_career_race_result,
            commands::iracing::iracing_preview_race_result,
            commands::iracing::iracing_auto_import_if_ready,
            commands::iracing::iracing_dump_session_yaml,
            commands::iracing::iracing_player_paint,
            commands::iracing::iracing_apply_player_paint,
            commands::iracing::iracing_has_player_id,
            commands::iracing::iracing_linked_custid,
            commands::iracing::iracing_link_player_paint,
            commands::iracing::iracing_apply_market_paint,
            commands::iracing::iracing_save_race_history,
            commands::iracing::iracing_list_saved_races,
            commands::iracing::iracing_load_saved_race,
            commands::iracing::iracing_yellow_macro_status,
            commands::iracing::iracing_install_yellow_macro,
            commands::iracing::iracing_restore_yellow_macro,
            commands::iracing::iracing_throw_yellow,
            commands::iracing::iracing_send_chat_macro,
            commands::iracing::iracing_set_auto_yellow,
            commands::iracing::iracing_auto_yellow_enabled,
            commands::window::minimize_window,
            commands::window::start_window_drag,
            commands::window::toggle_maximize_window,
            commands::window::close_window,
            commands::window::get_window_maximized,
            commands::window::toggle_fullscreen_window,
            commands::window::get_window_fullscreen,
            commands::save::flush_save,
            commands::save::create_season_backup,
            commands::save::list_backups,
            commands::save::restore_backup,
            commands::convocation::advance_to_convocation_window,
            commands::convocation::run_convocation_window,
            commands::convocation::get_player_special_offers,
            commands::convocation::get_special_window_state,
            commands::convocation::accept_special_offer_for_day,
            commands::convocation::advance_special_window_day,
            commands::convocation::respond_player_special_offer,
            commands::convocation::iniciar_bloco_especial,
            commands::convocation::encerrar_bloco_especial,
            commands::convocation::run_pos_especial,
            commands::calendar::get_temporal_summary,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        persist_window_snapshot, ResizeDebounceState, WindowStateSnapshot, RESIZE_DEBOUNCE_MS,
    };
    use crate::config::app_config::AppConfig;
    use uuid::Uuid;

    fn temp_base_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("iracer-lib-tests-{}", Uuid::new_v4()))
    }

    #[test]
    fn debounce_only_keeps_latest_resize_generation() {
        let state = ResizeDebounceState::default();
        let first = WindowStateSnapshot::resized(1280.0, 720.0);
        let second = WindowStateSnapshot::resized(1600.0, 900.0);

        let first_generation = state.schedule(first);
        let second_generation = state.schedule(second.clone());

        assert_eq!(state.latest_if_current(first_generation), None);
        assert_eq!(state.latest_if_current(second_generation), Some(second));
        assert_eq!(RESIZE_DEBOUNCE_MS, 500);
    }

    #[test]
    fn persist_window_snapshot_updates_size_when_not_maximized() {
        let base_dir = temp_base_dir();
        let snapshot = WindowStateSnapshot::resized(1440.0, 810.0);

        persist_window_snapshot(&base_dir, &snapshot).expect("snapshot should persist");

        let config = AppConfig::load_or_default(&base_dir);
        assert_eq!(config.window_width, 1440);
        assert_eq!(config.window_height, 810);
        assert!(!config.window_maximized);

        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn persist_window_snapshot_preserves_last_windowed_size_when_maximized() {
        let base_dir = temp_base_dir();
        let mut config = AppConfig::load_or_default(&base_dir);
        config.window_width = 1280;
        config.window_height = 720;
        config
            .save()
            .expect("seed windowed dimensions before maximize");

        let snapshot = WindowStateSnapshot::maximized();
        persist_window_snapshot(&base_dir, &snapshot).expect("maximized state should persist");

        let reloaded = AppConfig::load_or_default(&base_dir);
        assert_eq!(reloaded.window_width, 1280);
        assert_eq!(reloaded.window_height, 720);
        assert!(reloaded.window_maximized);

        let _ = std::fs::remove_dir_all(&base_dir);
    }
}
