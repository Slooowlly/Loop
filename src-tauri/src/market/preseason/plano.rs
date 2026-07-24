//! Persistência do plano de pré-temporada em disco e as datas de exibição.

use super::*;

pub fn refresh_preseason_state_display_date(
    conn: &Connection,
    season_id: &str,
    state: &mut PreSeasonState,
) -> Result<(), String> {
    state.current_display_date =
        compute_preseason_display_date(conn, season_id, state.current_week, state.total_weeks)?;
    Ok(())
}

pub fn save_preseason_plan(save_path: &Path, plan: &PreSeasonPlan) -> Result<(), String> {
    std::fs::create_dir_all(save_path)
        .map_err(|e| format!("Falha ao criar diretorio da pre-temporada: {e}"))?;
    let json = serde_json::to_string_pretty(plan)
        .map_err(|e| format!("Falha ao serializar plano da pre-temporada: {e}"))?;
    std::fs::write(preseason_plan_path(save_path), json)
        .map_err(|e| format!("Falha ao salvar plano da pre-temporada: {e}"))
}

pub fn load_preseason_plan(save_path: &Path) -> Result<Option<PreSeasonPlan>, String> {
    let path = preseason_plan_path(save_path);
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Falha ao ler plano da pre-temporada: {e}"))?;
    let plan = serde_json::from_str(&content)
        .map_err(|e| format!("Falha ao parsear plano da pre-temporada: {e}"))?;
    Ok(Some(plan))
}

pub fn delete_preseason_plan(save_path: &Path) -> Result<(), String> {
    let path = preseason_plan_path(save_path);
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_file(path).map_err(|e| format!("Falha ao apagar plano da pre-temporada: {e}"))
}

pub(super) fn preseason_plan_path(save_path: &Path) -> std::path::PathBuf {
    save_path.join("preseason_plan.json")
}

pub(super) fn compute_preseason_display_date(
    conn: &Connection,
    season_id: &str,
    current_week: i32,
    _total_weeks: i32,
) -> Result<Option<String>, String> {
    let season = season_queries::get_season_by_id(conn, season_id)
        .map_err(|e| format!("Falha ao carregar temporada da pre-temporada: {e}"))?
        .ok_or_else(|| format!("Temporada {season_id} nao encontrada"))?;
    let season_week = current_week.clamp(1, i32::from(MARKET_DURATION_WEEKS)) as u8;
    display_date_for_season_week(season_week, season.ano, Weekday::Sat).map(Some)
}

pub(super) fn get_season_id_by_number(
    conn: &Connection,
    season_number: i32,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT id FROM seasons WHERE numero = ?1 LIMIT 1",
        rusqlite::params![season_number],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| format!("Falha ao buscar temporada {season_number}: {e}"))
}
