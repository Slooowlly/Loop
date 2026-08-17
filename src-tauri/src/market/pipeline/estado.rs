//! Estado do mercado na temporada e o contexto de campeonato de cada piloto.
//!
//! É a primeira etapa da entressafra: zera o estado da temporada nova, carrega o
//! desempenho da temporada anterior (posição, vitórias, pólos, títulos) que alimenta
//! avaliação/propostas, e no fim persiste o mercado como resolvido.

use super::*;

#[derive(Debug, Clone)]
pub(super) struct DriverMarketContext {
    pub(super) posicao_campeonato: i32,
    pub(super) total_pilotos: i32,
    pub(super) categoria: String,
    pub(super) category_tier: u8,
    pub(super) vitorias: i32,
    pub(super) poles: i32,
    pub(super) titulos: i32,
    pub(super) papel: TeamRole,
}

pub(super) fn get_season_by_number(
    conn: &Connection,
    season_number: i32,
) -> Result<Option<crate::models::season::Season>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, numero, ano, status, rodada_atual, created_at, updated_at
             FROM seasons
             WHERE numero = ?1
             LIMIT 1",
        )
        .map_err(|e| format!("Falha ao preparar busca de temporada: {e}"))?;
    stmt.query_row(params![season_number], |row| {
        Ok(crate::models::season::Season {
            id: row.get(0)?,
            numero: row.get(1)?,
            ano: row.get(2)?,
            status: crate::models::enums::SeasonStatus::from_str_strict(&row.get::<_, String>(3)?)
                .map_err(rusqlite::Error::InvalidParameterName)?,
            rodada_atual: row.get(4)?,
            fase: crate::models::enums::SeasonPhase::BlocoRegular,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })
    .optional()
    .map_err(|e| format!("Falha ao buscar temporada {season_number}: {e}"))
}

pub(super) fn reset_market_state(conn: &Connection, season_id: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM market_proposals WHERE temporada_id = ?1",
        params![season_id],
    )
    .map_err(|e| format!("Falha ao limpar propostas de mercado: {e}"))?;
    conn.execute(
        "DELETE FROM market WHERE temporada_id = ?1",
        params![season_id],
    )
    .map_err(|e| format!("Falha ao limpar estado do mercado: {e}"))?;
    Ok(())
}

pub(super) fn persist_market_state(conn: &Connection, season_id: &str) -> Result<(), String> {
    let now = timestamp_now();
    conn.execute(
        "INSERT INTO market (temporada_id, status, fase, inicio, fim)
         VALUES (?1, 'Fechado', 'PreTemporada', ?2, ?3)",
        params![season_id, now, now],
    )
    .map_err(|e| format!("Falha ao persistir estado do mercado: {e}"))?;
    Ok(())
}

pub(super) fn load_market_contexts(
    conn: &Connection,
    previous_season_id: Option<&str>,
    drivers_by_id: &HashMap<String, Driver>,
    expiring_by_driver: &HashMap<String, Contract>,
) -> Result<HashMap<String, DriverMarketContext>, String> {
    // Papel do último contrato Regular, para quem não está em `expiring_by_driver`.
    // Os caminhos de shortlist (scan do jogador e cascata) passavam um mapa VAZIO,
    // então TODO candidato caía no default `Numero2` e levava -2.0 de visibilidade —
    // medido: era o corte dominante do gate de 4.0 nas faixas baixas (77% a 91% das
    // avaliações de vaga). O papel de verdade existe no banco; a conta usa ele.
    let last_papel_by_driver = load_last_regular_papeis(conn)?;
    let mut contexts = HashMap::new();
    if let Some(season_id) = previous_season_id {
        let mut stmt = conn
            .prepare(
                "SELECT piloto_id, categoria, posicao, vitorias, poles
                 FROM standings
                 WHERE temporada_id = ?1",
            )
            .map_err(|e| format!("Falha ao preparar standings do mercado: {e}"))?;
        let mut rows = stmt
            .query(params![season_id])
            .map_err(|e| format!("Falha ao ler standings do mercado: {e}"))?;
        let mut totals_by_category: HashMap<String, i32> = HashMap::new();
        let mut raw_rows = Vec::new();

        while let Some(row) = rows
            .next()
            .map_err(|e| format!("Falha ao iterar standings do mercado: {e}"))?
        {
            let piloto_id: String = row
                .get("piloto_id")
                .map_err(|e| format!("Falha ao ler piloto_id do standings: {e}"))?;
            let categoria: String = row.get("categoria").map_err(|e| {
                format!(
                    "Falha ao ler categoria do standings para piloto '{}': {e}",
                    piloto_id
                )
            })?;
            let posicao: i32 = row
                .get("posicao")
                .map_err(|e| format!("Falha ao ler posicao do standings: {e}"))?;
            let vitorias: i32 = row
                .get("vitorias")
                .map_err(|e| format!("Falha ao ler vitorias do standings: {e}"))?;
            let poles: i32 = row
                .get("poles")
                .map_err(|e| format!("Falha ao ler poles do standings: {e}"))?;
            *totals_by_category.entry(categoria.clone()).or_insert(0) += 1;
            raw_rows.push((piloto_id, categoria, posicao, vitorias, poles));
        }

        for (piloto_id, categoria, posicao, vitorias, poles) in raw_rows {
            let driver = drivers_by_id.get(&piloto_id);
            contexts.insert(
                piloto_id.clone(),
                DriverMarketContext {
                    posicao_campeonato: posicao,
                    total_pilotos: totals_by_category.get(&categoria).copied().unwrap_or(1),
                    category_tier: get_category_config(&categoria)
                        .map(|config| config.tier)
                        .unwrap_or(0),
                    categoria: categoria.clone(),
                    vitorias,
                    poles,
                    titulos: driver.map(|d| d.stats_carreira.titulos as i32).unwrap_or(0),
                    papel: expiring_by_driver
                        .get(&piloto_id)
                        .map(|contract| contract.papel.clone())
                        .or_else(|| last_papel_by_driver.get(&piloto_id).cloned())
                        .unwrap_or(TeamRole::Numero2),
                },
            );
        }
    }

    for driver in drivers_by_id.values() {
        contexts.entry(driver.id.clone()).or_insert_with(|| {
            let mut context = default_market_context(driver);
            if let Some(papel) = last_papel_by_driver.get(&driver.id) {
                context.papel = papel.clone();
            }
            context
        });
    }
    Ok(contexts)
}

/// Mapa piloto → papel do contrato Regular mais recente (por `temporada_fim`). O par de
/// [`load_last_regular_categories`], pelo mesmo motivo: agente livre não tem contrato
/// ativo, mas o papel que ele exercia é fato do banco, não um default. Quem nunca teve
/// contrato (rookie gerado) fica fora do mapa e cai no `Numero2` do chamador.
pub(super) fn load_last_regular_papeis(
    conn: &Connection,
) -> Result<HashMap<String, TeamRole>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT c1.piloto_id, c1.papel
             FROM contracts c1
             WHERE c1.tipo = 'Regular'
               AND CAST(c1.temporada_fim AS INTEGER) = (
                   SELECT MAX(CAST(c2.temporada_fim AS INTEGER))
                   FROM contracts c2
                   WHERE c2.piloto_id = c1.piloto_id AND c2.tipo = 'Regular'
               )",
        )
        .map_err(|e| format!("Falha ao preparar últimos papéis: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("Falha ao consultar últimos papéis: {e}"))?;
    let mut map = HashMap::new();
    for row in rows {
        let (piloto_id, papel) = row.map_err(|e| format!("Falha ao ler último papel: {e}"))?;
        map.insert(piloto_id, TeamRole::from_str_strict(&papel)?);
    }
    Ok(map)
}

pub(super) fn default_market_context(driver: &Driver) -> DriverMarketContext {
    let categoria = driver.categoria_atual.clone().unwrap_or_default();
    DriverMarketContext {
        posicao_campeonato: 99,
        total_pilotos: 99,
        category_tier: get_category_config(&categoria)
            .map(|config| config.tier)
            .unwrap_or(0),
        categoria,
        vitorias: driver.stats_temporada.vitorias as i32,
        poles: driver.stats_temporada.poles as i32,
        titulos: driver.stats_carreira.titulos as i32,
        papel: TeamRole::Numero2,
    }
}

/// Mapa piloto → categoria do contrato Regular mais recente (por `temporada_fim`).
/// Serve para resgatar o nível de veteranos parados no leilão (ver `find_available_drivers`).
pub(super) fn load_last_regular_categories(
    conn: &Connection,
) -> Result<HashMap<String, String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT c1.piloto_id, c1.categoria
             FROM contracts c1
             WHERE c1.tipo = 'Regular'
               AND CAST(c1.temporada_fim AS INTEGER) = (
                   SELECT MAX(CAST(c2.temporada_fim AS INTEGER))
                   FROM contracts c2
                   WHERE c2.piloto_id = c1.piloto_id AND c2.tipo = 'Regular'
               )",
        )
        .map_err(|e| format!("Falha ao preparar últimas categorias: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("Falha ao consultar últimas categorias: {e}"))?;
    let mut map = HashMap::new();
    for row in rows {
        let (piloto_id, categoria) =
            row.map_err(|e| format!("Falha ao ler última categoria: {e}"))?;
        map.insert(piloto_id, categoria);
    }
    Ok(map)
}

pub(super) fn sync_team_slots(
    conn: &Connection,
    teams: &[crate::models::team::Team],
    drivers_by_id: &HashMap<String, Driver>,
) -> Result<(), String> {
    sync_team_slots_from_active_regular_contracts(conn, teams, drivers_by_id)
}
