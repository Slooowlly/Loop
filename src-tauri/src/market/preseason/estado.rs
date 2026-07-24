//! Estado do mercado no banco: reset, atualização, rótulo da fase e carimbo de
//! data.

use super::*;

pub(super) fn reset_market_state(
    conn: &Connection,
    season_id: &str,
    phase: &PreSeasonPhase,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM market_proposals WHERE temporada_id = ?1",
        rusqlite::params![season_id],
    )
    .map_err(|e| format!("Falha ao limpar propostas antigas da pre-temporada: {e}"))?;
    conn.execute(
        "DELETE FROM market WHERE temporada_id = ?1",
        rusqlite::params![season_id],
    )
    .map_err(|e| format!("Falha ao limpar estado antigo do mercado: {e}"))?;
    conn.execute(
        "INSERT INTO market (temporada_id, status, fase, inicio, fim)
         VALUES (?1, 'Aberto', ?2, ?3, '')",
        rusqlite::params![season_id, phase_label(phase), timestamp_now()],
    )
    .map_err(|e| format!("Falha ao inicializar estado do mercado: {e}"))?;
    Ok(())
}

pub(super) fn update_market_state(
    conn: &Connection,
    season_id: &str,
    status: &str,
    phase: &PreSeasonPhase,
    completed: bool,
) -> Result<(), String> {
    let end_value = if completed {
        timestamp_now()
    } else {
        String::new()
    };
    conn.execute(
        "UPDATE market
         SET status = ?1, fase = ?2, fim = CASE WHEN ?3 = '' THEN fim ELSE ?3 END
         WHERE temporada_id = ?4",
        rusqlite::params![status, phase_label(phase), end_value, season_id],
    )
    .map_err(|e| format!("Falha ao atualizar estado do mercado: {e}"))?;
    Ok(())
}

pub(super) fn phase_label(phase: &PreSeasonPhase) -> &'static str {
    match phase {
        PreSeasonPhase::ContractExpiry => "ContractExpiry",
        PreSeasonPhase::Transfers => "Transfers",
        PreSeasonPhase::PlayerProposals => "PlayerProposals",
        PreSeasonPhase::RookiePlacement => "RookiePlacement",
        PreSeasonPhase::Finalization => "Finalization",
        PreSeasonPhase::Complete => "Complete",
    }
}

pub(super) fn timestamp_now() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}
