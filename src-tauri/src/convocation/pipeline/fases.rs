//! Transições de fase da temporada em volta do bloco especial:
//! BlocoRegular → JanelaConvocacao → BlocoEspecial → PosEspecial.

use super::*;

/// BlocoRegular → JanelaConvocacao.
/// Requer que a temporada ativa esteja em BlocoRegular.
pub fn advance_to_convocation_window(conn: &Connection) -> Result<(), DbError> {
    let season = season_queries::get_active_season(conn)?
        .ok_or_else(|| DbError::NotFound("Nenhuma temporada ativa".into()))?;

    if season.fase != SeasonPhase::BlocoRegular {
        return Err(DbError::Migration(format!(
            "Fase atual é '{}'; esperado BlocoRegular",
            season.fase
        )));
    }

    let pending_regular = calendar_queries::count_pending_races_in_phase(
        conn,
        &season.id,
        &SeasonPhase::BlocoRegular,
    )?;
    if pending_regular > 0 {
        return Err(DbError::Migration(format!(
            "A janela de convocacao so pode abrir depois do fim do bloco regular. Ainda existem {pending_regular} corridas regulares pendentes."
        )));
    }

    season_queries::update_season_fase(conn, &season.id, &SeasonPhase::JanelaConvocacao)?;
    Ok(())
}

/// JanelaConvocacao → BlocoEspecial.
/// Deve ser chamada APÓS run_convocation_window.
/// Gera o calendário das categorias especiais na janela setembro–dezembro.
pub fn iniciar_bloco_especial(conn: &Connection) -> Result<(), DbError> {
    let season = season_queries::get_active_season(conn)?
        .ok_or_else(|| DbError::NotFound("Nenhuma temporada ativa".into()))?;

    if season.fase != SeasonPhase::JanelaConvocacao {
        return Err(DbError::Migration(format!(
            "Fase atual é '{}'; esperado JanelaConvocacao",
            season.fase
        )));
    }

    // Gerar calendário das categorias especiais (production_challenger e endurance)
    let tx = conn.unchecked_transaction()?;
    season_queries::update_season_fase(&tx, &season.id, &SeasonPhase::BlocoEspecial)?;

    let mut rng = rand::thread_rng();
    generate_and_insert_special_calendars(&tx, &season.id, season.ano, &mut rng)
        .map_err(|e| DbError::Migration(format!("Falha ao gerar calendário especial: {e}")))?;

    tx.commit()?;
    Ok(())
}

/// BlocoEspecial → PosEspecial (transição esportiva: as corridas especiais terminaram).
/// Deve ser chamada antes de run_pos_especial.
pub fn encerrar_bloco_especial(conn: &Connection) -> Result<(), DbError> {
    let season = season_queries::get_active_season(conn)?
        .ok_or_else(|| DbError::NotFound("Nenhuma temporada ativa".into()))?;

    if season.fase != SeasonPhase::BlocoEspecial {
        return Err(DbError::Migration(format!(
            "Fase atual é '{}'; esperado BlocoEspecial",
            season.fase
        )));
    }

    season_queries::update_season_fase(conn, &season.id, &SeasonPhase::PosEspecial)?;
    Ok(())
}
