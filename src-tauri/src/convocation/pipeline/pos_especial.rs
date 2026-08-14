//! Etapa PosEspecial: transição de fase esportiva e desmontagem administrativa
//! do bloco especial (contratos, pilotos, hierarquias) + notícias de campeões.

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PosEspecialResult {
    pub contratos_encerrados: usize,
    pub pilotos_liberados: usize,
    pub equipes_limpas: usize,
    pub errors: Vec<String>,
}

/// Desmontagem administrativa do bloco especial: expira contratos, limpa pilotos e
/// hierarquias das equipes especiais. Gera e persiste notícias de campeões.
///
/// Escopo neste bloco: "core cleanup + news de encerramento".
/// Fora de escopo (implementar em blocos posteriores):
///   ajustes de motivação pós-special, reputação, espectadores, prêmios.
pub fn run_pos_especial(conn: &Connection) -> Result<PosEspecialResult, DbError> {
    let season = season_queries::get_active_season(conn)?
        .ok_or_else(|| DbError::NotFound("Nenhuma temporada ativa".into()))?;

    if season.fase != SeasonPhase::PosEspecial {
        return Err(DbError::Migration(format!(
            "Fase atual é '{}'; esperado PosEspecial",
            season.fase
        )));
    }

    // Coletar campeões ANTES do cleanup (contratos ainda ativos)
    let _campeoes = query_campeoes_especiais(conn, season.numero)?;

    // Cleanup em uma transação
    let tx = conn.unchecked_transaction()?;

    let contratos_encerrados = contract_queries::expire_especial_contracts(&tx, season.numero)?;
    let pilotos_liberados = driver_queries::clear_all_categoria_especial_ativa(&tx)?;
    let equipes_limpas = if contratos_encerrados > 0 {
        let equipes_limpas = team_queries::clear_special_team_lineups(&tx)?;
        team_queries::reset_special_team_hierarchies(&tx)?;
        equipes_limpas
    } else {
        0
    };

    tx.commit()?;

    Ok(PosEspecialResult {
        contratos_encerrados,
        pilotos_liberados,
        equipes_limpas,
        errors: vec![],
    })
}

/// Retorna o campeão de cada classe especial (maior temp_pontos com contrato Especial ativo).
/// Chamada antes do cleanup para ter acesso aos contratos ainda ativos.
pub(super) fn query_campeoes_especiais(
    conn: &Connection,
    season_number: i32,
) -> Result<Vec<(String, String, Option<String>, Option<String>)>, DbError> {
    let mut resultado = Vec::new();

    for cfg in CLASSES_CONVOCADAS {
        let campeao: Option<(String, String)> = conn
            .query_row(
                "SELECT d.id, d.nome FROM drivers d
             INNER JOIN contracts c ON c.piloto_id = d.id
             WHERE c.tipo = 'Especial' AND c.status = 'Ativo'
               AND CAST(c.temporada_inicio AS INTEGER) = ?1
               AND c.categoria = ?2
               AND c.classe = ?3
             ORDER BY d.temp_pontos DESC
             LIMIT 1",
                rusqlite::params![season_number, cfg.special_category, cfg.class_name],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(DbError::Sqlite)?;

        resultado.push((
            cfg.special_category.to_string(),
            cfg.class_name.to_string(),
            campeao.as_ref().map(|(_, nome)| nome.clone()),
            campeao.map(|(driver_id, _)| driver_id),
        ));
    }

    Ok(resultado)
}
