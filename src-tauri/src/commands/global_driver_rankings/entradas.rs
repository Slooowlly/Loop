//! Montagem da linha do ranking (DTO) para pilotos ativos e aposentados.

use super::*;

pub(super) fn build_current_driver_entry(
    conn: &Connection,
    driver: &Driver,
    current_year: i32,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
    team_lookup: &TeamLookup,
    real_career: &RealCareerIndex,
) -> Result<RankingEntry, String> {
    let contract = contract_queries::get_active_regular_contract_for_pilot(conn, &driver.id)
        .map_err(|e| format!("Falha ao buscar contrato regular ativo do piloto: {e}"))?;
    let team = contract.as_ref().and_then(|value| {
        team_queries::get_team_by_id(conn, &value.equipe_id)
            .ok()
            .flatten()
    });
    let category = contract
        .as_ref()
        .and_then(|value| regular_category(Some(&value.categoria), value.classe.as_deref()))
        .or_else(|| regular_category(driver.categoria_atual.as_deref(), None));
    let stats_by_category = load_driver_category_stats(
        conn,
        driver,
        category.as_deref(),
        team_title_stats_by_driver,
        real_career,
    )?;
    let historical_index = compute_historical_index(&stats_by_category);
    let injuries = injury_queries::count_injuries_by_severity_for_pilot(conn, &driver.id)
        .map_err(|e| format!("Falha ao contar lesoes do piloto: {e}"))?;
    let active_injury_type = injury_queries::get_active_injury_for_pilot(conn, &driver.id)
        .map_err(|e| format!("Falha ao buscar lesao ativa do piloto: {e}"))?
        .map(|injury| injury.injury_type.as_str().to_string());
    let total = total_stats(&stats_by_category);
    let (status, status_tone) = driver_status_label(driver, contract.is_some());
    let mut extra_historical_categories = load_contract_categories(conn, &driver.id)?;
    extra_historical_categories.extend(inferred_foundation_categories(
        driver,
        category.as_deref(),
        &stats_by_category,
    ));
    let historical_categories = historical_categories(
        &stats_by_category,
        category.as_deref(),
        &extra_historical_categories,
    );
    let debut_year = active_driver_debut_year(conn, driver, current_year)?;
    let career_years = active_driver_career_years(driver, &total, debut_year, current_year);

    let fama = driver.atributos.midia.clamp(0.0, 100.0).round() as i32;
    let carisma = driver.atributos.carisma.clamp(0.0, 100.0).round() as i32;
    let fama_delta = latest_archived_media(conn, &driver.id)?
        .map(|previous| fama - previous.clamp(0.0, 100.0).round() as i32)
        .filter(|delta| *delta != 0);

    let row = GlobalDriverRankingRow {
        id: driver.id.clone(),
        nome: driver.nome.clone(),
        nacionalidade: driver.nacionalidade.clone(),
        idade: driver.idade as i32,
        status,
        status_tone,
        is_jogador: driver.is_jogador,
        is_favorito: false, // carimbado adiante via get_favorite_ids
        is_lesionado: active_injury_type.is_some(),
        lesao_ativa_tipo: active_injury_type,
        equipe_nome: contract.as_ref().map(|value| value.equipe_nome.clone()),
        equipe_cor_primaria: team.map(|value| value.cor_primaria),
        categoria_atual: category,
        categorias_historicas: historical_categories,
        salario_anual: contract.as_ref().map(|value| value.salario_anual),
        ano_inicio_carreira: Some(debut_year),
        anos_carreira: career_years,
        temporada_aposentadoria: None,
        anos_aposentado: None,
        historical_index: round_one(historical_index),
        historical_rank: 0,
        historical_rank_delta: None,
        fama,
        carisma,
        fama_delta,
        wins_rank: 0,
        titles_rank: 0,
        podiums_rank: 0,
        injuries_rank: 0,
        corridas: total.races,
        pontos: total.points.round() as i32,
        vitorias: total.wins,
        podios: total.podiums,
        segundos: 0,  // preenchido adiante pela agregação de race_results
        terceiros: 0, // preenchido adiante pela agregação de race_results
        poles: total.poles,
        titulos: total.titles,
        titulos_por_categoria: title_categories(&stats_by_category, team_lookup),
        dnfs: total.dnfs,
        lesoes: injuries.leves + injuries.moderadas + injuries.graves,
        lesoes_leves: injuries.leves,
        lesoes_moderadas: injuries.moderadas,
        lesoes_graves: injuries.graves,
    };

    Ok(RankingEntry {
        row,
        stats_by_category,
    })
}

/// Anos de carreira de um piloto ativo. Sem NENHUMA largada não existe carreira:
/// ele ainda é um novato, por mais temporadas que tenha atravessado. O órfão sem
/// assento ganha `temporadas` de graça todo fim de ano (o acumulador não olha se
/// ele correu), e era isso que enchia o ranking de "7 anos de carreira" ao lado
/// de uma linha inteira zerada.
pub(super) fn active_driver_career_years(
    driver: &Driver,
    total: &CategoryStats,
    debut_year: i32,
    current_year: i32,
) -> Option<i32> {
    if career_starts(driver, total) <= 0 {
        return Some(0);
    }

    years_since(debut_year, current_year)
}

/// Largadas em toda a trajetória: o histórico exibido (archive ou o que ele correu
/// de verdade) mais a temporada em curso, que só entra no archive quando o ano
/// fecha. Um DNF conta como largada: ele alinhou no grid.
///
/// `stats_carreira` fica FORA de propósito — é ele que carrega o bloco carimbado no
/// nascimento do piloto, e bastava ele pra um piloto que nunca largou ganhar anos
/// de carreira. `stats_temporada` é limpo: zera todo ano e só cresce por corrida.
pub(super) fn career_starts(driver: &Driver, total: &CategoryStats) -> i32 {
    total
        .races
        .max(total.dnfs)
        .max(driver.stats_temporada.corridas as i32)
        .max(driver.stats_temporada.dnfs as i32)
}

pub(super) fn build_retired_driver_entry(
    retired: RetiredDriverSnapshot,
    current_year: i32,
    team_lookup: &TeamLookup,
    archive_stats: Vec<CategoryStats>,
    real_career: &RealCareerIndex,
) -> RankingEntry {
    let retirement_year = parse_year(&retired.retirement_season);
    let archived_races = archive_stats.iter().map(|entry| entry.races).sum::<i32>();
    let archived_starts = archive_stats
        .iter()
        .map(|entry| entry.races.max(entry.dnfs))
        .sum::<i32>();
    // Aposentado que nunca largou não teve carreira — pendurou o capacete ainda
    // novato. O snapshot carrega `temporadas` acumuladas mesmo nos anos em que
    // ficou sem assento, então sem largada esse número não vira "anos de
    // carreira".
    // O que ele correu de verdade — lastro para quem não tem archive de temporada.
    let real_stats = real_career.history_for(
        &retired.id,
        labelled_stats(retired.stats.clone(), Some(&retired.category)),
    );
    let career_years = if archived_starts <= 0 && real_stats.races.max(real_stats.dnfs) <= 0 {
        Some(0)
    } else {
        retired.career_years.or_else(|| {
            retired
                .career_start_year
                .and_then(|start| retirement_year.and_then(|end| years_since(start, end)))
        })
    };
    let years_retired = retirement_year.map(|year| (current_year - year).max(0));
    // ÍNDICE e DISPLAY pela MESMA régua dos ativos (por categoria) quando o archive
    // tem participação real (corridas > 0); senão, o que sobrou na pista. O snapshot
    // de carreira do aposentado NÃO serve de histórico: ele soma o bloco carimbado
    // no nascimento do piloto (corridas sem nenhuma corrida por trás) ao acumulado,
    // que ainda vem dobrado em saves antigos — era ele que punha "167 corridas / 0
    // pontos" no ranking para quem só largou 28 vezes.
    let archive_has_participation = archived_races > 0;
    let stats_by_category = if archive_has_participation {
        archive_stats
    } else {
        vec![real_stats]
    };
    let total = total_stats(&stats_by_category);
    let score = compute_historical_index(&stats_by_category);
    let title_categories = if retired.title_categories.is_empty() {
        title_categories(&stats_by_category, team_lookup)
    } else {
        retired.title_categories.clone()
    };
    let row = GlobalDriverRankingRow {
        id: retired.id,
        nome: retired.name,
        nacionalidade: "".to_string(),
        idade: 0,
        status: "Aposentado".to_string(),
        status_tone: "retired".to_string(),
        is_jogador: false,
        is_favorito: false, // carimbado adiante via get_favorite_ids
        is_lesionado: false,
        lesao_ativa_tipo: None,
        equipe_nome: None,
        equipe_cor_primaria: None,
        categoria_atual: Some(retired.category.clone()),
        categorias_historicas: historical_categories(
            std::slice::from_ref(&retired.stats),
            Some(&retired.category),
            &[],
        ),
        salario_anual: None,
        ano_inicio_carreira: retired.career_start_year,
        anos_carreira: career_years,
        temporada_aposentadoria: Some(retired.retirement_season),
        anos_aposentado: years_retired,
        historical_index: score,
        historical_rank: 0,
        historical_rank_delta: None,
        fama: 0,
        carisma: 0,
        fama_delta: None,
        wins_rank: 0,
        titles_rank: 0,
        podiums_rank: 0,
        injuries_rank: 0,
        corridas: total.races,
        pontos: total.points.round() as i32,
        vitorias: total.wins,
        podios: total.podiums,
        segundos: 0,  // preenchido adiante pela agregação de race_results
        terceiros: 0, // preenchido adiante pela agregação de race_results
        poles: total.poles,
        titulos: total.titles,
        titulos_por_categoria: title_categories,
        dnfs: total.dnfs,
        lesoes: 0,
        lesoes_leves: 0,
        lesoes_moderadas: 0,
        lesoes_graves: 0,
    };

    RankingEntry {
        row,
        stats_by_category,
    }
}

pub(super) fn build_retired_driver_entry_from_driver(
    retired: RetiredDriverSnapshot,
    driver: &Driver,
    current_year: i32,
    team_lookup: &TeamLookup,
    archive_stats: Vec<CategoryStats>,
    real_career: &RealCareerIndex,
) -> RankingEntry {
    let mut entry = build_retired_driver_entry(
        retired,
        current_year,
        team_lookup,
        archive_stats,
        real_career,
    );
    entry.row.nacionalidade = driver.nacionalidade.clone();
    entry.row.idade = driver.idade as i32;
    entry.row.is_jogador = driver.is_jogador;
    entry.row.fama = driver.atributos.midia.clamp(0.0, 100.0).round() as i32;
    entry.row.carisma = driver.atributos.carisma.clamp(0.0, 100.0).round() as i32;
    entry.row.ano_inicio_carreira = entry
        .row
        .ano_inicio_carreira
        .or(Some(driver.ano_inicio_carreira as i32));
    entry
}

pub(super) fn driver_status_label(driver: &Driver, has_active_contract: bool) -> (String, String) {
    if driver.status == DriverStatus::Aposentado {
        return ("Aposentado".to_string(), "retired".to_string());
    }
    if has_active_contract || driver.categoria_especial_ativa.is_some() {
        return ("Ativo".to_string(), "active".to_string());
    }
    ("Livre".to_string(), "dimmed".to_string())
}
