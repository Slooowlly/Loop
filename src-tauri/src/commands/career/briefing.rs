//! Montagem do briefing da proxima corrida: historico na pista, rival principal e as historias do fim de semana.

use super::*;

pub(crate) fn empty_track_history_summary() -> TrackHistorySummary {
    TrackHistorySummary {
        has_data: false,
        starts: 0,
        best_finish: None,
        last_finish: None,
        dnfs: 0,
        last_visit_season: None,
        last_visit_round: None,
    }
}

pub(crate) fn empty_next_race_briefing_summary() -> NextRaceBriefingSummary {
    NextRaceBriefingSummary {
        track_history: Some(empty_track_history_summary()),
        primary_rival: None,
        weekend_stories: Vec::new(),
        contract_warning: None,
    }
}

pub(crate) fn build_next_race_briefing_summary(
    conn: &rusqlite::Connection,
    player_id: &str,
    season_number: i32,
    race: &CalendarEntry,
) -> Result<NextRaceBriefingSummary, String> {
    let contract_warning = contract_queries::get_active_regular_contract_for_pilot(conn, player_id)
        .map_err(|e| format!("Falha ao buscar contrato regular do jogador: {e}"))?
        .and_then(|c| {
            if c.is_ultimo_ano(season_number) {
                Some(ContractWarningInfo {
                    temporada_fim: c.temporada_fim,
                    equipe_nome: c.equipe_nome,
                })
            } else {
                None
            }
        });

    Ok(NextRaceBriefingSummary {
        track_history: Some(build_track_history_summary(
            conn,
            player_id,
            &race.track_name,
        )?),
        primary_rival: build_primary_rival_summary(conn, player_id, &race.categoria)?,
        weekend_stories: build_weekend_story_summaries(
            conn,
            season_number,
            &race.categoria,
            race.rodada,
        )?,
        contract_warning,
    })
}

pub(crate) fn build_track_history_summary(
    conn: &rusqlite::Connection,
    player_id: &str,
    track_name: &str,
) -> Result<TrackHistorySummary, String> {
    let mut stmt = conn
        .prepare(
            "SELECT s.numero, c.rodada, r.posicao_final, r.dnf
             FROM race_results r
             JOIN calendar c ON r.race_id = c.id
             JOIN seasons s ON COALESCE(c.season_id, c.temporada_id) = s.id
             WHERE r.piloto_id = ?1
               AND c.track_name = ?2
             ORDER BY s.numero DESC, c.rodada DESC",
        )
        .map_err(|e| format!("Falha ao preparar historico de pista: {e}"))?;

    let rows = stmt
        .query_map(rusqlite::params![player_id, track_name], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)? != 0,
            ))
        })
        .map_err(|e| format!("Falha ao buscar historico de pista: {e}"))?;

    let mut visits = Vec::new();
    for row in rows {
        visits.push(row.map_err(|e| format!("Falha ao ler historico de pista: {e}"))?);
    }

    if visits.is_empty() {
        return Ok(empty_track_history_summary());
    }

    let last_visit = visits[0];
    let best_finish = visits
        .iter()
        .filter(|(_, _, position, is_dnf)| !*is_dnf && *position > 0)
        .map(|(_, _, position, _)| *position)
        .min();
    let dnfs = visits.iter().filter(|(_, _, _, is_dnf)| *is_dnf).count() as i32;

    Ok(TrackHistorySummary {
        has_data: true,
        starts: visits.len() as i32,
        best_finish,
        last_finish: Some(last_visit.2),
        dnfs,
        last_visit_season: Some(last_visit.0),
        last_visit_round: Some(last_visit.1),
    })
}

pub(crate) fn build_primary_rival_summary(
    conn: &rusqlite::Connection,
    player_id: &str,
    categoria: &str,
) -> Result<Option<PrimaryRivalSummary>, String> {
    let mut drivers = driver_queries::get_drivers_by_category(conn, categoria)
        .map_err(|e| format!("Falha ao buscar pilotos da categoria para rival principal: {e}"))?;

    drivers.sort_by(|a, b| {
        b.stats_temporada
            .pontos
            .partial_cmp(&a.stats_temporada.pontos)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.stats_temporada.vitorias.cmp(&a.stats_temporada.vitorias))
            .then_with(|| b.stats_temporada.podios.cmp(&a.stats_temporada.podios))
            .then_with(|| a.nome.cmp(&b.nome))
    });

    let Some(player_index) = drivers.iter().position(|driver| driver.id == player_id) else {
        return Ok(None);
    };

    let player = &drivers[player_index];
    let rival_index = if player_index == 0 {
        if drivers.len() > 1 {
            1
        } else {
            return Ok(None);
        }
    } else {
        player_index - 1
    };
    let rival = &drivers[rival_index];
    // `is_ahead` fala do RIVAL: true = o rival está à frente do jogador na tabela.
    let is_ahead = rival_index < player_index;
    let gap_points = if is_ahead {
        (rival.stats_temporada.pontos - player.stats_temporada.pontos)
            .max(0.0)
            .round() as i32
    } else {
        (player.stats_temporada.pontos - rival.stats_temporada.pontos)
            .max(0.0)
            .round() as i32
    };

    Ok(Some(PrimaryRivalSummary {
        driver_id: rival.id.clone(),
        driver_name: rival.nome.clone(),
        championship_position: rival_index as i32 + 1,
        gap_points,
        is_ahead,
        rivalry_label: None,
    }))
}

pub(crate) fn build_weekend_story_summaries(
    conn: &rusqlite::Connection,
    season_number: i32,
    categoria: &str,
    round_number: i32,
) -> Result<Vec<BriefingStorySummary>, String> {
    let mut stories = news_queries::get_news_by_season(conn, season_number, 200)
        .map_err(|e| format!("Falha ao buscar noticias da temporada para a previa: {e}"))?
        .into_iter()
        .filter(|item| {
            item.categoria_id.as_deref() == Some(categoria) && item.rodada == Some(round_number)
        })
        .collect::<Vec<_>>();

    stories.sort_by(|left, right| {
        briefing_importance_rank(&right.importancia)
            .cmp(&briefing_importance_rank(&left.importancia))
            .then_with(|| briefing_type_rank(&right.tipo).cmp(&briefing_type_rank(&left.tipo)))
            .then_with(|| right.timestamp.cmp(&left.timestamp))
    });

    Ok(stories
        .into_iter()
        .take(3)
        .map(|item| BriefingStorySummary {
            id: item.id,
            icon: item.icone,
            title: item.titulo,
            summary: build_briefing_story_summary_text(&item.texto),
            importance: item.importancia.as_str().to_string(),
        })
        .collect())
}

pub(crate) fn briefing_importance_rank(value: &NewsImportance) -> i32 {
    match value {
        NewsImportance::Destaque => 4,
        NewsImportance::Alta => 3,
        NewsImportance::Media => 2,
        NewsImportance::Baixa => 1,
    }
}

pub(crate) fn briefing_type_rank(value: &NewsType) -> i32 {
    match value {
        NewsType::Rivalidade => 5,
        NewsType::Hierarquia => 4,
        NewsType::Corrida => 3,
        NewsType::Incidente => 2,
        NewsType::FramingSazonal => 1,
        _ => 0,
    }
}

pub(crate) fn build_briefing_story_summary_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "O paddock segue produzindo contexto para a proxima largada.".to_string();
    }

    if let Some((first_sentence, _)) = trimmed.split_once('.') {
        let sentence = first_sentence.trim();
        if !sentence.is_empty() {
            return format!("{sentence}.");
        }
    }

    trimmed.chars().take(140).collect()
}
