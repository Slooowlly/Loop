//! Helpers de ciclo de temporada (não-assertivos): caminhos do save e o avanço
//! pré-temporada → temporada usado para encadear muitas temporadas seguidas.

use super::*;

// ── Helpers de ciclo de temporada (não-assertivos) ────────────────────────────

pub(super) fn career_db_path(base_dir: &Path) -> PathBuf {
    let config = AppConfig::load_or_default(base_dir);
    config.saves_dir().join("career_001").join("career.db")
}

pub(super) fn career_dir(base_dir: &Path) -> PathBuf {
    let config = AppConfig::load_or_default(base_dir);
    config.saves_dir().join("career_001")
}

/// Pré-temporada → Temporada, sem asserts (para rodar muitas temporadas seguidas).
pub(super) fn run_preseason_to_temporada(base_dir: &Path) {
    let db_path = career_db_path(base_dir);
    let career_dir = career_dir(base_dir);

    let _ = advance_market_week_in_base_dir(base_dir, "career_001", None);

    // Limpa propostas pendentes
    {
        let db = Database::open_existing(&db_path).expect("db");
        let _ = db.conn.execute(
            "UPDATE market_proposals SET status = 'Rejeitada' WHERE status = 'Pendente'",
            [],
        );
    }

    // Força conclusão do plano de pré-temporada
    if let Ok(Some(mut plan)) = crate::market::preseason::load_preseason_plan(&career_dir) {
        plan.state.is_complete = true;
        plan.state.current_week = plan.state.total_weeks + 1;
        plan.state.phase = PreSeasonPhase::Complete;
        plan.state.player_has_pending_proposals = false;
        crate::market::preseason::save_preseason_plan(&career_dir, &plan).expect("salvar plano");
    }

    finalize_preseason_in_base_dir(base_dir, "career_001").expect("finalizar pré-temporada");
}
