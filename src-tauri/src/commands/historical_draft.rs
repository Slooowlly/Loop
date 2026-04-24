use std::path::Path;

use crate::commands::career_types::{
    CareerDraftState, CreateCareerResult, CreateHistoricalDraftInput, FinalizeHistoricalDraftInput,
    SaveLifecycleStatus,
};

pub(crate) fn create_historical_career_draft_in_base_dir(
    _base_dir: &Path,
    _input: CreateHistoricalDraftInput,
) -> Result<CareerDraftState, String> {
    Err("Geracao historica de draft ainda nao implementada.".to_string())
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
    _base_dir: &Path,
    _input: FinalizeHistoricalDraftInput,
) -> Result<CreateCareerResult, String> {
    Err("Finalizacao de draft historico ainda nao implementada.".to_string())
}
