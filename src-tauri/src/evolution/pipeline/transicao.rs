//! Criação da próxima temporada e abertura da pré-temporada.

use super::*;

pub(super) fn create_next_season_phase(
    conn: &Connection,
    season: &Season,
    rng: &mut impl Rng,
    mode: EndOfSeasonMode,
) -> Result<Season, String> {
    let fase_inicial = match mode {
        EndOfSeasonMode::Playable => SeasonPhase::PreTemporada,
        EndOfSeasonMode::HistoricalDraft => SeasonPhase::Temporada,
    };
    let seed: u64 = rng.gen();
    let new_season = create_next_season_9d(conn, season, fase_inicial, seed)?;
    reset_driver_season_stats(conn)?;
    reset_team_season_stats(conn, new_season.numero)?;
    update_meta_for_new_season(conn, new_season.numero, new_season.ano)?;
    Ok(new_season)
}

pub(super) fn initialize_preseason_phase(
    conn: &Connection,
    new_season: &Season,
    save_path: &Path,
    rng: &mut impl Rng,
    mode: EndOfSeasonMode,
) -> Result<(bool, i32), String> {
    let mut preseason_plan = initialize_preseason(conn, new_season.numero, rng)
        .map_err(|e| format!("Erro ao inicializar pre-temporada: {e}"))?;
    if mode == EndOfSeasonMode::Playable {
        save_preseason_plan(save_path, &preseason_plan)
            .map_err(|e| format!("Erro ao salvar plano da pre-temporada: {e}"))?;
    } else {
        // Não-interativo: a IA resolve a janela sozinha (jogador sempre espera →
        // garantia de porta no fecho). Backstop contra loop infinito (a janela fecha
        // por hard_week_cap, mas protegemos mesmo assim).
        let mut guard = 0;
        while !preseason_plan.state.is_complete {
            advance_week(conn, &mut preseason_plan, None)
                .map_err(|e| format!("Erro ao executar pre-temporada historica: {e}"))?;
            guard += 1;
            if guard > 50 {
                return Err(
                    "Pre-temporada historica nao convergiu (a janela nao fechou).".to_string(),
                );
            }
        }
    }
    Ok((true, preseason_plan.state.total_weeks))
}
