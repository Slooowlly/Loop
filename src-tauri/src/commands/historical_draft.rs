use std::path::Path;

use chrono::Local;

use crate::calendar::{generate_all_calendars_with_year, CalendarEntry};
use crate::commands::career_types::{
    CareerDraftState, CreateCareerResult, CreateHistoricalDraftInput, FinalizeHistoricalDraftInput,
    SaveLifecycleStatus,
};
use crate::config::app_config::AppConfig;
use crate::db::connection::{Database, DbError};
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::meta as meta_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::evolution::pipeline::run_end_of_season;
use crate::generators::ids::{next_id, IdType};
use crate::generators::nationality::format_nationality;
use crate::generators::world::generate_historical_world;
use crate::market::pipeline::fill_all_remaining_vacancies;
use crate::models::contract::generate_initial_contract;
use crate::models::driver::Driver;
use crate::models::enums::{ContractStatus, TeamRole};
use crate::models::license::grant_driver_license_for_category_if_needed;
use crate::models::season::Season;

const HISTORY_START_YEAR: i32 = 2000;
const HISTORY_END_YEAR: i32 = 2024;
const PLAYABLE_START_YEAR: i32 = 2025;

pub(crate) fn create_historical_career_draft_in_base_dir(
    base_dir: &Path,
    input: CreateHistoricalDraftInput,
) -> Result<CareerDraftState, String> {
    let state = create_historical_career_draft_base(base_dir, input)?;
    let career_id = state
        .career_id
        .clone()
        .ok_or_else(|| "Draft sem career_id".to_string())?;
    let config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(&career_id);
    let db_path = career_dir.join("career.db");
    let mut db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco do draft: {e}"))?;

    simulate_historical_range(
        &mut db,
        &career_dir,
        HISTORY_START_YEAR,
        HISTORY_END_YEAR,
        PLAYABLE_START_YEAR,
    )?;

    Ok(CareerDraftState {
        progress_year: Some(PLAYABLE_START_YEAR as u32),
        ..state
    })
}

pub(crate) fn get_career_draft_in_base_dir(_base_dir: &Path) -> Result<CareerDraftState, String> {
    Ok(CareerDraftState {
        exists: false,
        career_id: None,
        lifecycle_status: SaveLifecycleStatus::Active,
        progress_year: None,
        error: None,
        categories: Vec::new(),
        teams: Vec::new(),
    })
}

pub(crate) fn discard_career_draft_in_base_dir(_base_dir: &Path) -> Result<(), String> {
    Err("Descarte de draft historico ainda nao implementado.".to_string())
}

pub(crate) fn finalize_career_draft_in_base_dir(
    base_dir: &Path,
    input: FinalizeHistoricalDraftInput,
) -> Result<CreateCareerResult, String> {
    finalize_career_draft(base_dir, input)
}

#[cfg(test)]
pub(crate) fn create_historical_career_draft_base_for_test(
    base_dir: &Path,
    input: CreateHistoricalDraftInput,
) -> Result<CareerDraftState, String> {
    create_historical_career_draft_base(base_dir, input)
}

fn create_historical_career_draft_base(
    base_dir: &Path,
    input: CreateHistoricalDraftInput,
) -> Result<CareerDraftState, String> {
    let normalized_name = input.player_name.trim().to_string();
    if normalized_name.is_empty() {
        return Err("Informe um nome para o piloto.".to_string());
    }

    let normalized_nationality = input.player_nationality.trim().to_lowercase();
    let normalized_difficulty = input.difficulty.trim().to_lowercase();
    let normalized_age = input.player_age.unwrap_or(20).clamp(16, 60);

    let config = AppConfig::load_or_default(base_dir);
    let saves_dir = config.saves_dir();
    let career_id = next_draft_career_id(&saves_dir);
    let career_number = career_number_from_id(&career_id)
        .ok_or_else(|| format!("Falha ao interpretar career_id '{career_id}'"))?;
    let career_dir = saves_dir.join(&career_id);
    let db_path = career_dir.join("career.db");
    let meta_path = career_dir.join("meta.json");

    std::fs::create_dir_all(&career_dir)
        .map_err(|e| format!("Falha ao criar diretorio do draft: {e}"))?;

    let creation_result = (|| -> Result<CareerDraftState, String> {
        let mut db = Database::create_new(&db_path)
            .map_err(|e| format!("Falha ao criar banco do draft: {e}"))?;
        let world = generate_historical_world(&normalized_difficulty, HISTORY_START_YEAR)?;

        let season_id = "S001".to_string();
        let season = Season::new(season_id.clone(), 1, HISTORY_START_YEAR);
        let calendars =
            generate_all_calendars_with_year(&season_id, season.ano, &mut rand::thread_rng())?;
        let all_calendar_entries: Vec<CalendarEntry> = calendars
            .values()
            .flat_map(|entries| entries.iter().cloned())
            .collect();
        let total_races = all_calendar_entries.len();

        db.transaction(|tx| {
            for driver in &world.drivers {
                driver_queries::insert_driver(tx, driver)?;
            }
            team_queries::insert_teams(tx, &world.teams)?;
            contract_queries::insert_contracts(tx, &world.contracts)?;
            for contract in &world.contracts {
                grant_driver_license_for_category_if_needed(
                    tx,
                    &contract.piloto_id,
                    &contract.categoria,
                )
                .map_err(DbError::Migration)?;
            }
            season_queries::insert_season(tx, &season)?;
            calendar_queries::insert_calendar_entries(tx, &all_calendar_entries)?;
            sync_draft_meta_counters(
                tx,
                world.drivers.len(),
                world.teams.len(),
                world.contracts.len(),
                1,
                total_races,
                HISTORY_START_YEAR,
            )?;
            Ok(())
        })
        .map_err(|e| format!("Falha ao persistir dados do draft: {e}"))?;

        write_draft_meta(
            &meta_path,
            career_number,
            &normalized_name,
            &normalized_nationality,
            normalized_age,
            &normalized_difficulty,
            total_races as i32,
        )?;

        Ok(CareerDraftState {
            exists: true,
            career_id: Some(career_id),
            lifecycle_status: SaveLifecycleStatus::Draft,
            progress_year: Some(HISTORY_START_YEAR as u32),
            error: None,
            categories: Vec::new(),
            teams: Vec::new(),
        })
    })();

    if creation_result.is_err() && career_dir.exists() {
        let _ = std::fs::remove_dir_all(&career_dir);
    }

    creation_result
}

fn sync_draft_meta_counters(
    conn: &rusqlite::Connection,
    total_drivers: usize,
    total_teams: usize,
    total_contracts: usize,
    total_seasons: usize,
    total_races: usize,
    current_year: i32,
) -> Result<(), DbError> {
    meta_queries::set_meta_value(
        conn,
        "next_driver_id",
        &(total_drivers as u32 + 1).to_string(),
    )?;
    meta_queries::set_meta_value(conn, "next_team_id", &(total_teams as u32 + 1).to_string())?;
    meta_queries::set_meta_value(
        conn,
        "next_contract_id",
        &(total_contracts as u32 + 1).to_string(),
    )?;
    meta_queries::set_meta_value(
        conn,
        "next_season_id",
        &(total_seasons as u32 + 1).to_string(),
    )?;
    meta_queries::set_meta_value(conn, "next_race_id", &(total_races as u32 + 1).to_string())?;
    meta_queries::set_meta_value(conn, "current_season", &total_seasons.to_string())?;
    meta_queries::set_meta_value(conn, "current_year", &current_year.to_string())?;
    Ok(())
}

fn write_draft_meta(
    meta_path: &Path,
    career_number: u32,
    player_name: &str,
    player_nationality: &str,
    player_age: i32,
    difficulty: &str,
    total_races: i32,
) -> Result<(), String> {
    let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let meta = serde_json::json!({
        "version": 1,
        "career_number": career_number,
        "player_name": player_name,
        "current_season": 1,
        "current_year": HISTORY_START_YEAR,
        "created_at": now,
        "last_played": now,
        "team_name": null,
        "category": "",
        "difficulty": difficulty,
        "total_races": total_races,
        "lifecycle_status": "draft",
        "history_start_year": HISTORY_START_YEAR,
        "history_end_year": HISTORY_END_YEAR,
        "playable_start_year": PLAYABLE_START_YEAR,
        "draft_progress_year": HISTORY_START_YEAR,
        "draft_error": null,
        "pending_player_nationality": player_nationality,
        "pending_player_age": player_age,
    });
    let payload = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Falha ao serializar meta do draft: {e}"))?;
    std::fs::write(meta_path, payload).map_err(|e| format!("Falha ao gravar meta do draft: {e}"))
}

fn next_draft_career_id(saves_dir: &Path) -> String {
    if !saves_dir.exists() {
        return "career_001".to_string();
    }

    let next_number = std::fs::read_dir(saves_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.strip_prefix("career_")?.parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0)
        + 1;

    format!("career_{next_number:03}")
}

fn career_number_from_id(career_id: &str) -> Option<u32> {
    career_id.strip_prefix("career_")?.parse::<u32>().ok()
}

fn finalize_career_draft(
    base_dir: &Path,
    input: FinalizeHistoricalDraftInput,
) -> Result<CreateCareerResult, String> {
    let config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(&input.career_id);
    let db_path = career_dir.join("career.db");
    let meta_path = career_dir.join("meta.json");
    if !career_dir.exists() {
        return Err("Draft nao encontrado.".to_string());
    }

    let meta_content = std::fs::read_to_string(&meta_path)
        .map_err(|e| format!("Falha ao ler meta do draft: {e}"))?;
    let mut meta: crate::config::app_config::SaveMeta = serde_json::from_str(&meta_content)
        .map_err(|e| format!("Falha ao parsear meta do draft: {e}"))?;
    if meta.lifecycle_status != SaveLifecycleStatus::Draft {
        return Err("Somente drafts podem ser finalizados.".to_string());
    }

    let mut db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco do draft: {e}"))?;
    let active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa do draft: {e}"))?
        .ok_or_else(|| "Temporada ativa do draft nao encontrada.".to_string())?;
    let mut selected_team = team_queries::get_team_by_id(&db.conn, &input.team_id)
        .map_err(|e| format!("Falha ao buscar equipe selecionada: {e}"))?
        .ok_or_else(|| "Equipe selecionada nao encontrada.".to_string())?;
    if selected_team.categoria != input.category {
        return Err("Equipe selecionada nao pertence a categoria escolhida.".to_string());
    }
    let displaced_n2 = selected_team
        .piloto_2_id
        .clone()
        .ok_or_else(|| "Equipe selecionada nao possui N2 para substituir.".to_string())?;

    let pending_nationality = meta
        .pending_player_nationality
        .clone()
        .unwrap_or_else(|| "br".to_string());
    let player_age = meta.pending_player_age.unwrap_or(20).clamp(16, 60);
    let player_nationality = format_nationality(&pending_nationality, "M", "pt-BR");
    let player_name = meta.player_name.clone();

    let (player_id, player_team_id, player_team_name, total_drivers, total_teams, total_races) = db
        .transaction(|tx| {
            let player_id = next_id(tx, IdType::Driver)?;
            let contract_id = next_id(tx, IdType::Contract)?;
            let mut player = Driver::new_player(
                player_id.clone(),
                player_name.clone(),
                player_nationality,
                player_age as u32,
                active_season.ano.max(0) as u32,
            );
            player.categoria_atual = Some(input.category.clone());
            driver_queries::insert_driver(tx, &player)?;
            grant_driver_license_for_category_if_needed(tx, &player.id, &input.category)
                .map_err(DbError::Migration)?;

            if let Some(displaced_contract) =
                contract_queries::get_active_regular_contract_for_pilot(tx, &displaced_n2)?
            {
                contract_queries::update_contract_status(
                    tx,
                    &displaced_contract.id,
                    &ContractStatus::Rescindido,
                )?;
            }

            selected_team.piloto_2_id = Some(player.id.clone());
            selected_team.hierarquia_n2_id = Some(player.id.clone());
            selected_team.is_player_team = true;
            team_queries::update_team(tx, &selected_team)?;

            let player_contract = generate_initial_contract(
                contract_id,
                &player.id,
                &player.nome,
                &selected_team.id,
                &selected_team.nome,
                TeamRole::Numero2,
                &input.category,
                active_season.numero,
            );
            contract_queries::insert_contract(tx, &player_contract)?;

            let total_drivers = driver_queries::count_drivers(tx)? as usize;
            let total_teams = count_rows(tx, "teams")?;
            let total_races = count_rows(tx, "calendar")?;

            Ok((
                player.id.clone(),
                selected_team.id.clone(),
                selected_team.nome.clone(),
                total_drivers,
                total_teams,
                total_races,
            ))
        })
        .map_err(|e| format!("Falha ao finalizar draft: {e}"))?;

    meta.lifecycle_status = SaveLifecycleStatus::Active;
    meta.current_season = active_season.numero.max(1) as u32;
    meta.current_year = active_season.ano.max(0) as u32;
    meta.team_name = Some(player_team_name.clone());
    meta.category = input.category;
    meta.total_races = total_races as i32;
    meta.draft_progress_year = None;
    meta.draft_error = None;
    let payload = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Falha ao serializar meta finalizado: {e}"))?;
    std::fs::write(&meta_path, payload)
        .map_err(|e| format!("Falha ao gravar meta finalizado: {e}"))?;

    Ok(CreateCareerResult {
        success: true,
        career_id: input.career_id,
        save_path: career_dir.to_string_lossy().to_string(),
        player_id,
        player_team_id,
        player_team_name,
        season_id: active_season.id,
        total_drivers,
        total_teams,
        total_races,
        message: "Carreira historica criada com sucesso".to_string(),
    })
}

fn count_rows(conn: &rusqlite::Connection, table: &str) -> Result<usize, DbError> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    let count: i64 = conn.query_row(&sql, [], |row| row.get(0))?;
    Ok(count as usize)
}

#[cfg(test)]
pub(crate) fn create_historical_career_draft_for_range_for_test(
    base_dir: &Path,
    input: CreateHistoricalDraftInput,
    start_year: i32,
    end_year: i32,
    playable_year: i32,
) -> Result<CareerDraftState, String> {
    let state = create_historical_career_draft_base(base_dir, input)?;
    let career_id = state
        .career_id
        .clone()
        .ok_or_else(|| "Draft sem career_id".to_string())?;
    let config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(&career_id);
    let db_path = career_dir.join("career.db");
    let mut db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco do draft: {e}"))?;

    simulate_historical_range(&mut db, &career_dir, start_year, end_year, playable_year)?;

    Ok(CareerDraftState {
        progress_year: Some(playable_year as u32),
        ..state
    })
}

fn simulate_historical_range(
    db: &mut Database,
    career_dir: &Path,
    start_year: i32,
    end_year: i32,
    playable_year: i32,
) -> Result<(), String> {
    for _year in start_year..=end_year {
        simulate_current_historical_season(db)?;
        let season = season_queries::get_active_season(&db.conn)
            .map_err(|e| format!("Falha ao buscar temporada historica ativa: {e}"))?
            .ok_or_else(|| "Temporada historica ativa nao encontrada.".to_string())?;
        run_end_of_season(&mut db.conn, &season, career_dir)?;
        let next_season = season_queries::get_active_season(&db.conn)
            .map_err(|e| format!("Falha ao buscar proxima temporada historica: {e}"))?
            .ok_or_else(|| "Proxima temporada historica nao encontrada.".to_string())?;
        fill_all_remaining_vacancies(&db.conn, next_season.numero, &mut rand::thread_rng())?;
        clear_historical_news(&db.conn)?;
        clear_historical_preseason_plan(career_dir)?;
        update_draft_progress(career_dir, (season.ano + 1) as u32)?;
    }

    let active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada jogavel ativa: {e}"))?
        .ok_or_else(|| "Temporada jogavel ativa nao encontrada.".to_string())?;
    if active_season.ano != playable_year {
        return Err(format!(
            "Ano jogavel esperado {playable_year}, encontrado {}.",
            active_season.ano
        ));
    }
    Ok(())
}

fn update_draft_progress(career_dir: &Path, progress_year: u32) -> Result<(), String> {
    let meta_path = career_dir.join("meta.json");
    let content = std::fs::read_to_string(&meta_path)
        .map_err(|e| format!("Falha ao ler meta do draft: {e}"))?;
    let mut meta: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Falha ao parsear meta do draft: {e}"))?;
    meta["draft_progress_year"] = serde_json::json!(progress_year);
    meta["current_year"] = serde_json::json!(progress_year);
    let payload = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Falha ao serializar progresso do draft: {e}"))?;
    std::fs::write(&meta_path, payload)
        .map_err(|e| format!("Falha ao gravar progresso do draft: {e}"))
}

fn clear_historical_news(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.execute("DELETE FROM news", [])
        .map_err(|e| format!("Falha ao limpar noticias historicas: {e}"))?;
    Ok(())
}

fn clear_historical_preseason_plan(career_dir: &Path) -> Result<(), String> {
    let path = career_dir.join("preseason_plan.json");
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| format!("Falha ao limpar plano de pre-temporada historico: {e}"))?;
    }
    Ok(())
}

fn simulate_current_historical_season(db: &mut Database) -> Result<(), String> {
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada historica ativa: {e}"))?
        .ok_or_else(|| "Temporada historica ativa nao encontrada.".to_string())?;
    let pending_races = calendar_queries::get_pending_races(&db.conn, &season.id)
        .map_err(|e| format!("Falha ao buscar corridas historicas pendentes: {e}"))?;

    for race in &pending_races {
        crate::commands::race::simulate_category_race(db, race, false)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        create_historical_career_draft_base_for_test,
        create_historical_career_draft_for_range_for_test,
    };
    use crate::commands::career_types::{
        CreateHistoricalDraftInput, FinalizeHistoricalDraftInput, SaveLifecycleStatus,
    };
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::drivers as driver_queries;
    use crate::db::queries::seasons as season_queries;
    use crate::db::queries::{contracts as contract_queries, teams as team_queries};

    #[test]
    fn create_draft_base_world_has_no_player_and_starts_in_2000() {
        let base_dir = unique_test_dir("draft_base_world");
        let input = sample_draft_input();

        let state = create_historical_career_draft_base_for_test(&base_dir, input)
            .expect("draft base should be created");

        assert_eq!(state.lifecycle_status, SaveLifecycleStatus::Draft);
        let career_id = state.career_id.as_deref().expect("draft career id");
        let db = open_draft_db(&base_dir, career_id);
        assert!(driver_queries::get_player_driver(&db.conn).is_err());
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        assert_eq!(season.ano, 2000);

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[test]
    fn historical_simulation_reaches_playable_year_with_results_and_no_news() {
        let base_dir = unique_test_dir("historical_short");
        let input = sample_draft_input();

        let state =
            create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2001, 2002)
                .expect("historical generation should finish");

        assert_eq!(state.lifecycle_status, SaveLifecycleStatus::Draft);
        let career_id = state.career_id.as_deref().expect("draft career id");
        let db = open_draft_db(&base_dir, career_id);
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        assert_eq!(season.ano, 2002);

        let result_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM race_results", [], |row| row.get(0))
            .expect("race result count");
        assert!(result_count > 0);

        let news_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM news", [], |row| row.get(0))
            .expect("news count");
        assert_eq!(news_count, 0);

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[test]
    fn finalize_draft_inserts_player_as_n2_and_displaces_existing_n2() {
        let base_dir = unique_test_dir("finalize_draft");
        let state = create_historical_career_draft_for_range_for_test(
            &base_dir,
            sample_draft_input(),
            2000,
            2000,
            2001,
        )
        .expect("draft should be created");
        let career_id = state.career_id.clone().expect("draft career id");
        let db = open_draft_db(&base_dir, &career_id);
        let selected_team = team_queries::get_teams_by_category(&db.conn, "mazda_rookie")
            .expect("teams by category")
            .into_iter()
            .next()
            .expect("at least one rookie team");
        let displaced_n2 = selected_team
            .piloto_2_id
            .clone()
            .expect("team should have N2 before finalization");
        drop(db);

        let result = super::finalize_career_draft_in_base_dir(
            &base_dir,
            FinalizeHistoricalDraftInput {
                career_id: career_id.clone(),
                category: selected_team.categoria.clone(),
                team_id: selected_team.id.clone(),
            },
        )
        .expect("finalize should succeed");

        assert!(result.success);
        let db = open_draft_db(&base_dir, &career_id);
        let player = driver_queries::get_player_driver(&db.conn).expect("player should exist");
        assert_eq!(player.stats_temporada.corridas, 0);
        assert_eq!(player.stats_carreira.corridas, 0);
        let refreshed_team = team_queries::get_team_by_id(&db.conn, &selected_team.id)
            .expect("team query")
            .expect("selected team");
        assert_eq!(
            refreshed_team.piloto_2_id.as_deref(),
            Some(player.id.as_str())
        );
        assert_eq!(
            refreshed_team.hierarquia_n2_id.as_deref(),
            Some(player.id.as_str())
        );
        assert!(refreshed_team.is_player_team);
        assert!(
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &displaced_n2)
                .expect("displaced contract query")
                .is_none()
        );

        let _ = std::fs::remove_dir_all(base_dir);
    }

    fn sample_draft_input() -> CreateHistoricalDraftInput {
        CreateHistoricalDraftInput {
            player_name: "Joao Silva".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(22),
            difficulty: "medio".to_string(),
        }
    }

    fn open_draft_db(base_dir: &Path, career_id: &str) -> Database {
        let config = AppConfig::load_or_default(base_dir);
        let db_path = config.saves_dir().join(career_id).join("career.db");
        Database::open_existing(&db_path).expect("draft db should open")
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("iracer_historical_draft_{label}_{nanos}"))
    }
}
