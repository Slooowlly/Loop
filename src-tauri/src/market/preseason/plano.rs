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

fn preseason_plan_staged_path(save_path: &Path) -> std::path::PathBuf {
    save_path.join("preseason_plan.json.novo")
}

/// Plano da pré-temporada escrito em staging, à espera do commit do banco.
///
/// A virada de temporada roda dentro de UMA transação, e o plano é um arquivo em
/// disco — que não participa dela. Gravado direto no `preseason_plan.json`, ele
/// sobrevivia ao rollback de qualquer passo posterior e passava a descrever um
/// mercado que o banco não tem. O `remove_file` pendurado só no erro de `commit`
/// cobria uma saída de erro entre várias.
///
/// Aqui o arquivo nasce ao lado do definitivo e só entra no lugar por
/// [`publicar`](Self::publicar), chamado depois do commit. Enquanto o guard vive,
/// QUALQUER outra saída (erro, `?`, panic) cai no `Drop` e descarta o staging. O
/// arquivo anterior fica intocado até a publicação, e a troca em si passa pelo
/// mesmo mecanismo com rollback usado pelo backup.
pub struct PreSeasonPlanStaging {
    staged: std::path::PathBuf,
    destino: std::path::PathBuf,
}

impl PreSeasonPlanStaging {
    /// Serializa o plano no arquivo de staging. Nada é publicado aqui.
    pub fn preparar(save_path: &Path, plan: &PreSeasonPlan) -> Result<Self, String> {
        std::fs::create_dir_all(save_path)
            .map_err(|e| format!("Falha ao criar diretorio da pre-temporada: {e}"))?;

        let staged = preseason_plan_staged_path(save_path);
        if staged.exists() {
            // Sobra de uma virada que morreu antes do `Drop` rodar (crash, queda de luz).
            std::fs::remove_file(&staged).map_err(|e| {
                format!(
                    "Falha ao limpar plano da pre-temporada em staging '{}': {e}",
                    staged.display()
                )
            })?;
        }

        let json = serde_json::to_string_pretty(plan)
            .map_err(|e| format!("Falha ao serializar plano da pre-temporada: {e}"))?;
        std::fs::write(&staged, json)
            .map_err(|e| format!("Falha ao gravar plano da pre-temporada em staging: {e}"))?;

        Ok(Self {
            staged,
            destino: preseason_plan_path(save_path),
        })
    }

    /// Coloca o staging no lugar do arquivo definitivo. Só pode ser chamado com o
    /// banco já comitado.
    pub fn publicar(self) -> Result<(), String> {
        crate::commands::save::substituir_preservando_anterior(
            &self.staged,
            &self.destino,
            "plano da pre-temporada",
        )
    }
}

impl Drop for PreSeasonPlanStaging {
    fn drop(&mut self) {
        // Depois de uma publicação bem-sucedida o staged já saiu por rename e isto é
        // no-op. Em qualquer outra saída é o que impede o plano novo de sobreviver ao
        // rollback do banco.
        let _ = std::fs::remove_file(&self.staged);
    }
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
