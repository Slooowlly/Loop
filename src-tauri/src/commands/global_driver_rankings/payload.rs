//! Entrada do ranking global: abre o banco da carreira e monta o payload completo.

use super::*;

pub(crate) fn get_global_driver_rankings_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    selected_driver_id: Option<&str>,
) -> Result<GlobalDriverRankingPayload, String> {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    if !db_path.exists() {
        return Err("Banco da carreira nao encontrado.".to_string());
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    build_global_driver_rankings(&db.conn, selected_driver_id)
}

pub(super) fn build_global_driver_rankings(
    conn: &Connection,
    selected_driver_id: Option<&str>,
) -> Result<GlobalDriverRankingPayload, String> {
    let current_year = season_queries::get_active_season(conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa do ranking global: {e}"))?
        .map(|season| season.ano)
        .unwrap_or(2024);
    let drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos globais: {e}"))?;
    let team_title_stats_by_driver = load_all_team_champion_title_stats(conn)?;
    let team_lookup = load_team_lookup(conn)?;
    let real_career = RealCareerIndex::load(conn)?;
    let mut entries = Vec::new();
    let mut seen_driver_ids = HashSet::new();
    let mut retired_by_id: HashMap<String, RetiredDriverSnapshot> =
        load_retired_snapshots(conn, &team_title_stats_by_driver, &team_lookup)?
            .into_iter()
            .map(|retired| (retired.id.clone(), retired))
            .collect();

    for driver in drivers {
        seen_driver_ids.insert(driver.id.clone());
        if driver.status == DriverStatus::Aposentado {
            if let Some(retired) = retired_by_id.remove(&driver.id) {
                // Pontuação consistente com ativos: histórico POR CATEGORIA do archive
                // (em vez do agregado de carreira × multiplicador da categoria final, que
                // inflava a carreira toda no peso da endurance). Vazio sem archive → o
                // construtor cai no que ele correu de verdade.
                let archive_stats =
                    load_archive_category_stats(conn, &driver.id, &team_title_stats_by_driver)?;
                entries.push(build_retired_driver_entry_from_driver(
                    retired,
                    &driver,
                    current_year,
                    &team_lookup,
                    archive_stats,
                    &real_career,
                ));
                continue;
            }
        }
        entries.push(build_current_driver_entry(
            conn,
            &driver,
            current_year,
            &team_title_stats_by_driver,
            &team_lookup,
            &real_career,
        )?);
    }

    for retired in retired_by_id.into_values() {
        if seen_driver_ids.contains(&retired.id) {
            continue;
        }
        // Aposentado sem registro na tabela `drivers` (purgado): histórico por
        // categoria do archive; se não houver, cai no que ele correu de verdade.
        let archive_stats =
            load_archive_category_stats(conn, &retired.id, &team_title_stats_by_driver)?;
        entries.push(build_retired_driver_entry(
            retired,
            current_year,
            &team_lookup,
            archive_stats,
            &real_career,
        ));
    }

    let unranked_player_driver = entries
        .iter()
        .find(|entry| entry.row.is_jogador)
        .map(|entry| entry.row.clone());
    entries.retain(|entry| has_ranking_visibility(&entry.row));
    let stats_by_driver = entries
        .iter()
        .map(|entry| (entry.row.id.clone(), entry.stats_by_category.clone()))
        .collect::<HashMap<_, _>>();
    let mut rows = entries
        .into_iter()
        .map(|entry| entry.row)
        .collect::<Vec<_>>();
    rows.retain(has_ranking_visibility);
    // Marca os favoritados (watchlist) — alimenta a estrela inline + o filtro "Favoritos".
    let favorites = crate::db::queries::favorites::get_favorite_ids(conn).unwrap_or_default();
    // Split dos pódios por posição (2º/3º) direto dos resultados reais — alimenta o
    // tooltip "quantos pódios não foram vitória". Pilotos sem `race_results` ficam 0.
    let podium_splits = career_podium_splits(conn)?;
    for row in &mut rows {
        row.is_favorito = favorites.contains(&row.id);
        if let Some(&(segundos, terceiros)) = podium_splits.get(&row.id) {
            row.segundos = segundos;
            row.terceiros = terceiros;
        }
    }
    assign_ranks(&mut rows);
    assign_rank_deltas(conn, &mut rows, &stats_by_driver)?;
    let leaders = build_leaders(&rows);
    let player_driver = rows
        .iter()
        .find(|row| row.is_jogador)
        .cloned()
        .or(unranked_player_driver);

    Ok(GlobalDriverRankingPayload {
        selected_driver_id: selected_driver_id.map(str::to_string),
        player_driver,
        rows,
        leaders,
    })
}
