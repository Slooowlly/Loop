//! Posicionamento do piloto entre pares: campeonato da categoria, companheiro de equipe, rankings de carreira e rivais.

use super::*;

pub(super) fn build_career_rank_block(
    conn: &Connection,
    driver: &Driver,
) -> Result<DriverCareerRankBlock, String> {
    let drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos para rankings de carreira: {e}"))?;

    Ok(DriverCareerRankBlock {
        corridas: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.corridas),
        vitorias: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.vitorias),
        podios: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.podios),
        titulos: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.titulos),
    })
}

pub(super) fn rank_driver_by<F>(drivers: &[Driver], driver_id: &str, metric: F) -> Option<i32>
where
    F: Fn(&Driver) -> u32,
{
    let mut ranked: Vec<(&str, u32)> = drivers
        .iter()
        .map(|driver| (driver.id.as_str(), metric(driver)))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    ranked
        .iter()
        .position(|(id, _)| *id == driver_id)
        .map(|index| index as i32 + 1)
}

pub(super) fn build_driver_rivals_block(
    conn: &Connection,
    driver: &Driver,
) -> Result<DriverRivalsBlock, String> {
    let rivalries = crate::rivalry::get_pilot_rivalries(conn, &driver.id)
        .map_err(|e| format!("Falha ao carregar rivalidades do piloto: {e}"))?;
    let mut itens = Vec::new();

    for rivalry in rivalries.into_iter().take(4) {
        let rival_name = driver_queries::get_driver(conn, &rivalry.rival_id)
            .map(|rival| rival.nome)
            .unwrap_or_else(|_| rivalry.rival_id.clone());
        itens.push(DriverRivalInfo {
            driver_id: rivalry.rival_id,
            nome: rival_name,
            tipo: rivalry.tipo.as_str().to_string(),
            intensidade: rivalry.perceived_intensity.round().clamp(0.0, 100.0) as u8,
            intensidade_historica: rivalry.historical_intensity.round().clamp(0.0, 100.0) as u8,
            atividade_recente: rivalry.recent_activity.round().clamp(0.0, 100.0) as u8,
        });
    }

    Ok(DriverRivalsBlock { itens })
}

pub(super) fn find_championship_position(
    conn: &Connection,
    category: &str,
    driver_id: &str,
) -> Result<Option<i32>, String> {
    let mut drivers = driver_queries::get_drivers_by_category(conn, category)
        .map_err(|e| format!("Falha ao carregar classificacao da categoria: {e}"))?;
    drivers.sort_by(|a, b| {
        b.stats_temporada
            .pontos
            .partial_cmp(&a.stats_temporada.pontos)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.stats_temporada.vitorias.cmp(&a.stats_temporada.vitorias))
            .then_with(|| b.stats_temporada.podios.cmp(&a.stats_temporada.podios))
            .then_with(|| a.nome.cmp(&b.nome))
    });

    Ok(drivers
        .iter()
        .position(|driver| driver.id == driver_id)
        .map(|index| index as i32 + 1))
}

pub(super) fn find_teammate(
    conn: &Connection,
    driver: &Driver,
    team: Option<&Team>,
) -> Result<Option<Driver>, String> {
    let Some(team) = team else {
        return Ok(None);
    };
    let teammate_id = [team.piloto_1_id.as_ref(), team.piloto_2_id.as_ref()]
        .into_iter()
        .flatten()
        .find(|id| id.as_str() != driver.id);
    let Some(teammate_id) = teammate_id else {
        return Ok(None);
    };

    driver_queries::get_driver(conn, teammate_id)
        .map(Some)
        .map_err(|e| format!("Falha ao carregar companheiro de equipe: {e}"))
}

