use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use crate::commands::career_types::{
    GlobalDriverRankingLeaders, GlobalDriverRankingPayload, GlobalDriverRankingRow,
    GlobalDriverTitleCategorySummary, GlobalDriverTitleYearTeam,
};
use crate::config::app_config::AppConfig;
use crate::constants::categories::{
    competitive_division_key, get_feeder_categories, is_especial, is_valid_competitive_division,
};
use crate::db::connection::Database;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::injuries as injury_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::models::driver::Driver;
use crate::models::enums::DriverStatus;

#[derive(Debug, Clone, Default)]
struct CategoryStats {
    category: String,
    class_name: Option<String>,
    points: f64,
    wins: i32,
    podiums: i32,
    poles: i32,
    races: i32,
    titles: i32,
    title_years: Vec<TitleYear>,
    dnfs: i32,
}

/// Um ano de título e a equipe (por `team_id`) com a qual foi conquistado.
#[derive(Debug, Clone)]
struct TitleYear {
    year: i32,
    team_id: Option<String>,
}

#[derive(Debug, Clone)]
struct RetiredDriverSnapshot {
    id: String,
    name: String,
    retirement_season: String,
    category: String,
    stats: CategoryStats,
    title_categories: Vec<GlobalDriverTitleCategorySummary>,
    career_start_year: Option<i32>,
    career_years: Option<i32>,
}

#[derive(Debug, Clone)]
struct RankingEntry {
    row: GlobalDriverRankingRow,
    stats_by_category: Vec<CategoryStats>,
}

#[derive(Debug, Clone, Default)]
struct RaceContribution {
    category: String,
    points: f64,
    wins: i32,
    podiums: i32,
    poles: i32,
    races: i32,
    dnfs: i32,
}

type TitleEventKey = (i32, String, Option<String>);
type TeamTitleStatsByDriver = HashMap<String, Vec<(TitleEventKey, CategoryStats)>>;
/// `team_id` -> (nome atual da equipe, cor primária), usado para resolver a logo
/// da equipe campeã por ano de título.
type TeamLookup = HashMap<String, (String, String)>;

#[derive(Debug, Clone)]
struct TeamTitleDriverScore {
    driver_id: String,
    points: f64,
    wins: i32,
    podiums: i32,
    best_finish: i32,
    races: i32,
}

#[derive(Debug, Clone)]
struct SpecialTeamTitleCandidate {
    event_key: TitleEventKey,
    season_number: i32,
    year: i32,
    category: String,
    class_name: Option<String>,
    team_id: String,
    points: f64,
    wins: i32,
    podiums: i32,
    poles: i32,
    races: i32,
}

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

fn build_global_driver_rankings(
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

/// O que cada piloto REALMENTE fez na pista, direto de `race_results` — a única
/// fonte que não carrega passado carimbado. `stats_carreira` não serve como
/// histórico: todo piloto gerado fora das categorias rookie nasce com um bloco de
/// corridas/temporadas inventado por `seed_initial_career_history` (sem uma única
/// corrida por trás), e o acumulado ainda arrasta a contagem dobrada de saves
/// antigos. Usado como lastro quando o piloto não tem archive de temporada.
///
/// `titles` fica 0 de propósito: título é evento de campeonato, vem do archive.
/// `driver_id = None` agrega a tabela inteira; `Some(id)` agrega só aquele piloto.
fn real_career_by_driver_filtered(
    conn: &Connection,
    driver_id: Option<&str>,
) -> Result<HashMap<String, CategoryStats>, String> {
    if !table_exists(conn, "race_results")? {
        return Ok(HashMap::new());
    }
    let mut stmt = conn
        .prepare(
            "SELECT piloto_id,
                COUNT(*) AS corridas,
                COALESCE(SUM(pontos), 0) AS pontos,
                COALESCE(SUM(CASE WHEN posicao_final = 1 AND dnf = 0 THEN 1 ELSE 0 END), 0) AS vitorias,
                COALESCE(SUM(CASE WHEN posicao_final BETWEEN 1 AND 3 AND dnf = 0 THEN 1 ELSE 0 END), 0) AS podios,
                COALESCE(SUM(CASE WHEN posicao_largada = 1 THEN 1 ELSE 0 END), 0) AS poles,
                COALESCE(SUM(CASE WHEN dnf = 1 THEN 1 ELSE 0 END), 0) AS dnfs
             FROM race_results
             WHERE ?1 IS NULL OR piloto_id = ?1
             GROUP BY piloto_id",
        )
        .map_err(|e| format!("Falha ao preparar carreira real do piloto: {e}"))?;
    let rows = stmt
        .query_map(params![driver_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                CategoryStats {
                    category: String::new(),
                    class_name: None,
                    races: row.get::<_, i32>(1)?,
                    points: row.get::<_, f64>(2)?,
                    wins: row.get::<_, i32>(3)?,
                    podiums: row.get::<_, i32>(4)?,
                    poles: row.get::<_, i32>(5)?,
                    dnfs: row.get::<_, i32>(6)?,
                    titles: 0,
                    title_years: Vec::new(),
                },
            ))
        })
        .map_err(|e| format!("Falha ao consultar carreira real do piloto: {e}"))?;
    let mut by_driver = HashMap::new();
    for row in rows {
        let (id, stats) = row.map_err(|e| format!("Falha ao ler carreira real do piloto: {e}"))?;
        by_driver.insert(id, stats);
    }
    Ok(by_driver)
}

/// Índice do que cada piloto realmente correu. Quando o save tem resultado gravado,
/// ele é a única fonte de histórico aceita no lugar do archive de temporada. Num save
/// sem nenhum resultado (fixture de teste, base recém-criada) não há verdade de campo
/// pra consultar, e o agregado de carreira volta a ser o último recurso.
struct RealCareerIndex {
    by_driver: HashMap<String, CategoryStats>,
    has_results: bool,
}

impl RealCareerIndex {
    fn load(conn: &Connection) -> Result<Self, String> {
        let by_driver = real_career_by_driver_filtered(conn, None)?;
        Ok(Self {
            has_results: !by_driver.is_empty(),
            by_driver,
        })
    }

    fn for_driver(conn: &Connection, driver_id: &str) -> Result<Self, String> {
        let by_driver = real_career_by_driver_filtered(conn, Some(driver_id))?;
        Ok(Self {
            has_results: !by_driver.is_empty(),
            by_driver,
        })
    }

    /// Histórico de lastro do piloto, herdando o rótulo de categoria de `from_career`.
    /// Com resultado gravado no save, quem nunca largou fica zerado — é assim que o
    /// bloco carimbado deixa de contar como carreira.
    fn history_for(&self, driver_id: &str, from_career: CategoryStats) -> CategoryStats {
        if !self.has_results {
            return from_career;
        }
        let mut stats = self.by_driver.get(driver_id).cloned().unwrap_or_default();
        stats.category = from_career.category;
        stats.class_name = from_career.class_name;
        stats
    }
}

/// Conta, por piloto e pela carreira inteira, quantas vezes terminou em 2º e em 3º
/// — o detalhe que quebra "pódios que não foram vitória". Lê os resultados reais
/// (`race_results`, nunca podados entre temporadas), então cobre tudo o que foi
/// corrido no jogo. Pilotos históricos pré-gerados não têm linhas aqui e simplesmente
/// não aparecem no mapa (o chamador os deixa em 0). Uma varredura indexada, barata.
fn career_podium_splits(conn: &Connection) -> Result<HashMap<String, (i32, i32)>, String> {
    if !table_exists(conn, "race_results")? {
        return Ok(HashMap::new());
    }
    let mut stmt = conn
        .prepare(
            "SELECT piloto_id,
                COALESCE(SUM(CASE WHEN posicao_final = 2 THEN 1 ELSE 0 END), 0) AS segundos,
                COALESCE(SUM(CASE WHEN posicao_final = 3 THEN 1 ELSE 0 END), 0) AS terceiros
             FROM race_results
             GROUP BY piloto_id",
        )
        .map_err(|e| format!("Falha ao preparar split de podios: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                (row.get::<_, i32>(1)?, row.get::<_, i32>(2)?),
            ))
        })
        .map_err(|e| format!("Falha ao consultar split de podios: {e}"))?;
    let mut splits = HashMap::new();
    for row in rows {
        let (id, split) = row.map_err(|e| format!("Falha ao ler split de podios: {e}"))?;
        splits.insert(id, split);
    }
    Ok(splits)
}

fn build_current_driver_entry(
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
fn active_driver_career_years(
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
fn career_starts(driver: &Driver, total: &CategoryStats) -> i32 {
    total
        .races
        .max(total.dnfs)
        .max(driver.stats_temporada.corridas as i32)
        .max(driver.stats_temporada.dnfs as i32)
}

fn build_retired_driver_entry(
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

fn build_retired_driver_entry_from_driver(
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

fn driver_status_label(driver: &Driver, has_active_contract: bool) -> (String, String) {
    if driver.status == DriverStatus::Aposentado {
        return ("Aposentado".to_string(), "retired".to_string());
    }
    if has_active_contract || driver.categoria_especial_ativa.is_some() {
        return ("Ativo".to_string(), "active".to_string());
    }
    ("Livre".to_string(), "dimmed".to_string())
}

/// Lê o histórico por categoria do archive (uma `CategoryStats` por temporada-
/// categoria) e os eventos de título já contados. Núcleo compartilhado entre o
/// caminho de ativo (`load_driver_category_stats`) e o de aposentado por id
/// (`load_archive_category_stats`).
fn read_archive_category_stats(
    conn: &Connection,
    driver_id: &str,
) -> Result<(Vec<CategoryStats>, HashSet<TitleEventKey>), String> {
    let mut stmt = conn
        .prepare(
            "SELECT categoria, pontos, snapshot_json, posicao_campeonato, season_number, ano
             FROM driver_season_archive
             WHERE piloto_id = ?1",
        )
        .map_err(|e| format!("Falha ao preparar historico global do piloto: {e}"))?;
    let rows = stmt
        .query_map(params![driver_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                row.get::<_, String>(2)?,
                row.get::<_, Option<i32>>(3)?,
                row.get::<_, i32>(4)?,
                row.get::<_, i32>(5)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar historico global do piloto: {e}"))?;

    let mut stats = Vec::new();
    let mut counted_title_events = HashSet::<TitleEventKey>::new();
    for row in rows {
        let (category, points, snapshot_json, championship_position, season_number, year) =
            row.map_err(|e| format!("Falha ao ler historico global do piloto: {e}"))?;
        let snapshot: Value = serde_json::from_str(&snapshot_json).unwrap_or_default();
        let category = normalized_archive_category(&snapshot, category);
        let class_name =
            archived_title_class(conn, driver_id, &category, season_number, &snapshot)?;
        let points = json_f64(&snapshot, "pontos").unwrap_or(points);
        let wins = json_i32(&snapshot, "vitorias");
        let podiums = json_i32(&snapshot, "podios");
        let poles = json_i32(&snapshot, "poles");
        let races = json_i32(&snapshot, "corridas");
        let titles = valid_archived_title_count(
            json_i32_option(&snapshot, "titulos"),
            championship_position,
            points,
            wins,
            podiums,
            poles,
            races,
        );
        if titles > 0 {
            counted_title_events.insert(title_event_key(
                season_number,
                &category,
                class_name.as_deref(),
            ));
        }
        let title_team_id =
            json_string(&snapshot, "team_id").filter(|value| !value.trim().is_empty());
        stats.push(CategoryStats {
            category,
            class_name,
            points,
            wins,
            podiums,
            poles,
            races,
            titles,
            title_years: title_years_for_event(titles, year, title_team_id),
            dnfs: json_i32(&snapshot, "dnfs"),
        });
    }

    Ok((stats, counted_title_events))
}

/// Fama (`atributos.midia`) registrada no snapshot MAIS RECENTE do archive de
/// temporadas — a base pra medir "quanto a fama subiu" nesta temporada. `None`
/// quando não há archive/tabela/snapshot com o campo (ex.: 1ª temporada).
fn latest_archived_media(conn: &Connection, driver_id: &str) -> Result<Option<f64>, String> {
    if !table_exists(conn, "driver_season_archive")? {
        return Ok(None);
    }
    let snapshot_json: Option<String> = conn
        .query_row(
            "SELECT snapshot_json FROM driver_season_archive
             WHERE piloto_id = ?1
             ORDER BY season_number DESC, ano DESC
             LIMIT 1",
            params![driver_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| format!("Falha ao carregar fama arquivada do piloto: {e}"))?;
    let Some(snapshot_json) = snapshot_json else {
        return Ok(None);
    };
    let snapshot: Value = serde_json::from_str(&snapshot_json).unwrap_or_default();
    Ok(snapshot
        .get("atributos")
        .and_then(|atributos| atributos.get("midia"))
        .and_then(Value::as_f64))
}

/// Histórico por categoria de um piloto (por id), incluindo títulos como campeão
/// de equipe. Vazio se não houver archive — o chamador decide o fallback.
fn load_archive_category_stats(
    conn: &Connection,
    driver_id: &str,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
) -> Result<Vec<CategoryStats>, String> {
    if !table_exists(conn, "driver_season_archive")? {
        return Ok(Vec::new());
    }
    let (mut stats, counted_title_events) = read_archive_category_stats(conn, driver_id)?;
    let team_title_stats = team_champion_title_stats_for_driver(
        driver_id,
        &counted_title_events,
        team_title_stats_by_driver,
    );
    stats.extend(team_title_stats);
    Ok(stats)
}

fn load_driver_category_stats(
    conn: &Connection,
    driver: &Driver,
    fallback_category: Option<&str>,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
    real_career: &RealCareerIndex,
) -> Result<Vec<CategoryStats>, String> {
    let fallback =
        || real_career.history_for(&driver.id, stats_from_driver(driver, fallback_category));
    if !table_exists(conn, "driver_season_archive")? {
        return Ok(vec![fallback()]);
    }

    let (mut stats, counted_title_events) = read_archive_category_stats(conn, &driver.id)?;
    let team_title_stats = team_champion_title_stats_for_driver(
        &driver.id,
        &counted_title_events,
        team_title_stats_by_driver,
    );
    if stats.is_empty() {
        stats.push(fallback());
    }
    stats.extend(team_title_stats);

    Ok(stats)
}

/// Agregado de carreira do piloto. NÃO é histórico confiável por si só: soma o bloco
/// carimbado no nascimento (`seed_initial_career_history`) ao que ele correu. Serve
/// de rótulo de categoria e de último recurso em save sem resultado gravado.
/// Carimba o rótulo de categoria num agregado que não tem um.
fn labelled_stats(stats: CategoryStats, category: Option<&str>) -> CategoryStats {
    let (category, class_name) = category_stats_parts(category.unwrap_or("unknown"));
    CategoryStats {
        category,
        class_name,
        ..stats
    }
}

fn stats_from_driver(driver: &Driver, category: Option<&str>) -> CategoryStats {
    let (category, class_name) = category_stats_parts(category.unwrap_or("unknown"));
    CategoryStats {
        category,
        class_name,
        points: driver.stats_carreira.pontos_total,
        wins: driver.stats_carreira.vitorias as i32,
        podiums: driver.stats_carreira.podios as i32,
        poles: driver.stats_carreira.poles as i32,
        races: driver.stats_carreira.corridas as i32,
        titles: driver.stats_carreira.titulos as i32,
        title_years: Vec::new(),
        dnfs: driver.stats_carreira.dnfs as i32,
    }
}

fn regular_category(category: Option<&str>, class_name: Option<&str>) -> Option<String> {
    let category = category?.trim();
    if category.is_empty() {
        return None;
    }
    if let Some((base_category, key_class_name)) = category.split_once(':') {
        if is_valid_competitive_division(base_category, Some(key_class_name)) {
            return Some(competitive_division_key(
                base_category,
                Some(key_class_name),
            ));
        }
        return None;
    }
    if is_valid_competitive_division(category, class_name) {
        Some(competitive_division_key(category, class_name))
    } else {
        None
    }
}

fn category_stats_parts(category: &str) -> (String, Option<String>) {
    let category = category.trim();
    if let Some((base_category, class_name)) = category.split_once(':') {
        let class_name = class_name.trim();
        if is_valid_competitive_division(base_category, Some(class_name)) {
            return (
                base_category.trim().to_string(),
                Some(class_name.to_string()),
            );
        }
    }
    (category.to_string(), None)
}

fn active_driver_debut_year(
    conn: &Connection,
    driver: &Driver,
    current_year: i32,
) -> Result<i32, String> {
    let fallback_year = driver.ano_inicio_carreira as i32;
    if !table_exists(conn, "driver_season_archive")? {
        return Ok(inferred_active_driver_debut_year(
            driver,
            current_year,
            fallback_year,
        ));
    }

    let mut stmt = conn
        .prepare(
            "SELECT ano, categoria, snapshot_json
             FROM driver_season_archive
             WHERE piloto_id = ?1",
        )
        .map_err(|e| format!("Falha ao preparar estreia historica do piloto: {e}"))?;
    let rows = stmt
        .query_map(params![driver.id], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar estreia historica do piloto: {e}"))?;

    let mut archive_year: Option<i32> = None;
    for row in rows {
        let (year, category, snapshot_json) =
            row.map_err(|e| format!("Falha ao ler estreia historica do piloto: {e}"))?;
        let snapshot: Value = serde_json::from_str(&snapshot_json).unwrap_or_default();
        let category = normalized_archive_category(&snapshot, category);
        if category == "unknown"
            || is_especial(&category)
            || !has_competitive_archive_participation(&snapshot)
        {
            continue;
        }
        archive_year = Some(archive_year.map_or(year, |current| current.min(year)));
    }

    Ok(match archive_year {
        Some(year) => year,
        None => inferred_active_driver_debut_year(driver, current_year, fallback_year),
    })
}

fn inferred_active_driver_debut_year(
    driver: &Driver,
    current_year: i32,
    fallback_year: i32,
) -> i32 {
    if current_year > 0 {
        let career_seasons = driver.stats_carreira.temporadas as i32;
        if career_seasons > 0 {
            return (current_year - career_seasons + 1).max(1);
        }
        // Sem temporada fechada, toda largada que ele tem é da temporada em
        // curso: a estreia é este ano. `ano_inicio_carreira` NÃO serve aqui —
        // nasce como pano de fundo (o ano em que o piloto pegou num kart, aos
        // 16), não como estreia na carreira, e fazia o piloto do jogador saltar
        // de 0 pra 5 anos assim que largava pela primeira vez.
        return current_year;
    }

    fallback_year.max(0)
}

fn has_competitive_archive_participation(snapshot: &Value) -> bool {
    json_i32(snapshot, "corridas") > 0
        || json_f64(snapshot, "pontos").unwrap_or(0.0) > 0.0
        || json_i32(snapshot, "vitorias") > 0
        || json_i32(snapshot, "podios") > 0
        || json_i32(snapshot, "poles") > 0
        || json_i32(snapshot, "titulos") > 0
}

fn load_contract_categories(conn: &Connection, driver_id: &str) -> Result<Vec<String>, String> {
    let contracts = contract_queries::get_contracts_for_pilot(conn, driver_id)
        .map_err(|e| format!("Falha ao carregar historico de contratos do piloto: {e}"))?;
    let mut categories = Vec::new();
    for contract in contracts {
        if let Some(category) =
            regular_category(Some(&contract.categoria), contract.classe.as_deref())
        {
            push_category(&mut categories, &category);
        }
    }
    Ok(categories)
}

fn load_retired_snapshots(
    conn: &Connection,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
    team_lookup: &TeamLookup,
) -> Result<Vec<RetiredDriverSnapshot>, String> {
    if !table_exists(conn, "retired")? {
        return Ok(Vec::new());
    }

    let mut stmt = conn
        .prepare("SELECT piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas FROM retired")
        .map_err(|e| format!("Falha ao preparar aposentados globais: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar aposentados globais: {e}"))?;

    let mut retired = Vec::new();
    for row in rows {
        let (id, name, retirement_season, category, stats_json) =
            row.map_err(|e| format!("Falha ao ler aposentado global: {e}"))?;
        let snapshot: Value = serde_json::from_str(&stats_json).unwrap_or_default();
        let retirement_season = normalize_retirement_season(conn, &retirement_season)?;
        let archived_titles =
            valid_archived_title_count_for_pilot(conn, &id, team_title_stats_by_driver)?;
        let title_categories = valid_archived_title_categories_for_pilot(
            conn,
            &id,
            team_title_stats_by_driver,
            team_lookup,
        )?
        .unwrap_or_else(|| {
            title_categories(
                &[CategoryStats {
                    category: category.clone(),
                    class_name: None,
                    points: json_f64(&snapshot, "pontos")
                        .or_else(|| json_f64(&snapshot, "pontos_total"))
                        .or_else(|| json_f64(&snapshot, "carreira_pontos_total"))
                        .unwrap_or(0.0),
                    wins: json_i32(&snapshot, "vitorias"),
                    podiums: json_i32(&snapshot, "podios"),
                    poles: json_i32(&snapshot, "poles"),
                    races: json_i32(&snapshot, "corridas"),
                    titles: json_i32(&snapshot, "titulos"),
                    title_years: Vec::new(),
                    dnfs: json_i32(&snapshot, "dnfs"),
                }],
                team_lookup,
            )
        });
        let snapshot_titles = json_i32(&snapshot, "titulos");
        retired.push(RetiredDriverSnapshot {
            id,
            name,
            retirement_season,
            category: category.clone(),
            career_start_year: json_i32_option(&snapshot, "ano_inicio_carreira"),
            career_years: json_i32_option(&snapshot, "anos_carreira")
                .or_else(|| json_i32_option(&snapshot, "temporadas")),
            stats: CategoryStats {
                category,
                class_name: None,
                points: json_f64(&snapshot, "pontos")
                    .or_else(|| json_f64(&snapshot, "pontos_total"))
                    .or_else(|| json_f64(&snapshot, "carreira_pontos_total"))
                    .unwrap_or(0.0),
                wins: json_i32(&snapshot, "vitorias"),
                podiums: json_i32(&snapshot, "podios"),
                poles: json_i32(&snapshot, "poles"),
                races: json_i32(&snapshot, "corridas"),
                titles: archived_titles.unwrap_or(snapshot_titles),
                title_years: Vec::new(),
                dnfs: json_i32(&snapshot, "dnfs"),
            },
            title_categories,
        });
    }
    Ok(retired)
}

fn normalize_retirement_season(conn: &Connection, value: &str) -> Result<String, String> {
    let Some(parsed) = parse_positive_i32(value) else {
        return Ok(value.to_string());
    };
    if parsed >= 1900 {
        return Ok(value.to_string());
    }

    conn.query_row(
        "SELECT ano FROM seasons WHERE numero = ?1 LIMIT 1",
        params![parsed],
        |row| row.get::<_, i32>(0),
    )
    .optional()
    .map(|year| {
        year.map(|value| value.to_string())
            .unwrap_or_else(|| value.to_string())
    })
    .map_err(|e| format!("Falha ao normalizar temporada de aposentadoria '{value}': {e}"))
}

fn total_stats(stats: &[CategoryStats]) -> CategoryStats {
    stats
        .iter()
        .fold(CategoryStats::default(), |mut total, entry| {
            total.points += entry.points;
            total.wins += entry.wins;
            total.podiums += entry.podiums;
            total.poles += entry.poles;
            total.races += entry.races;
            total.titles += entry.titles;
            total.dnfs += entry.dnfs;
            total
        })
}

fn title_categories(
    stats: &[CategoryStats],
    team_lookup: &TeamLookup,
) -> Vec<GlobalDriverTitleCategorySummary> {
    let mut totals = HashMap::<(String, Option<String>), (i32, Vec<TitleYear>)>::new();
    for entry in stats {
        if entry.titles <= 0 {
            continue;
        }
        let total = totals
            .entry((entry.category.clone(), entry.class_name.clone()))
            .or_default();
        total.0 += entry.titles;
        total.1.extend(entry.title_years.iter().cloned());
    }
    let mut summaries = totals
        .into_iter()
        .map(|((categoria, classe), (titulos, years))| {
            let (anos, anos_equipes) = build_title_year_teams(&years, team_lookup);
            GlobalDriverTitleCategorySummary {
                categoria,
                classe,
                titulos,
                anos,
                anos_equipes,
            }
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        right
            .titulos
            .cmp(&left.titulos)
            .then_with(|| left.categoria.cmp(&right.categoria))
            .then_with(|| left.classe.cmp(&right.classe))
    });
    summaries
}

fn load_team_lookup(conn: &Connection) -> Result<TeamLookup, String> {
    let teams = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao carregar equipes para titulos: {e}"))?;
    Ok(teams
        .into_iter()
        .map(|team| (team.id, (team.nome, team.cor_primaria)))
        .collect())
}

/// Agrega os anos de título (com a equipe de cada ano) em (`anos` ordenados desc,
/// `anos_equipes` na mesma ordem, com o nome/cor da equipe resolvidos pelo `team_id`).
fn build_title_year_teams(
    years: &[TitleYear],
    team_lookup: &TeamLookup,
) -> (Vec<i32>, Vec<GlobalDriverTitleYearTeam>) {
    let mut ordered: Vec<i32> = Vec::new();
    let mut team_by_year: HashMap<i32, Option<String>> = HashMap::new();
    for entry in years {
        if entry.year <= 0 {
            continue;
        }
        match team_by_year.get_mut(&entry.year) {
            Some(existing) => {
                if existing.is_none() && entry.team_id.is_some() {
                    *existing = entry.team_id.clone();
                }
            }
            None => {
                team_by_year.insert(entry.year, entry.team_id.clone());
                ordered.push(entry.year);
            }
        }
    }
    ordered.sort_unstable_by(|left, right| right.cmp(left));
    let anos_equipes = ordered
        .iter()
        .map(|&ano| {
            let (equipe, equipe_cor) = team_by_year
                .get(&ano)
                .and_then(|team_id| team_id.as_deref())
                .and_then(|team_id| team_lookup.get(team_id))
                .map(|(nome, cor)| (Some(nome.clone()), Some(cor.clone())))
                .unwrap_or((None, None));
            GlobalDriverTitleYearTeam {
                ano,
                equipe,
                equipe_cor,
            }
        })
        .collect();
    (ordered, anos_equipes)
}

fn historical_categories(
    stats: &[CategoryStats],
    fallback_category: Option<&str>,
    extra_categories: &[String],
) -> Vec<String> {
    let mut categories = Vec::new();
    for category in stats
        .iter()
        .map(|entry| entry.category.as_str())
        .chain(fallback_category.into_iter())
        .chain(extra_categories.iter().map(String::as_str))
    {
        push_category(&mut categories, category);
    }
    categories
}

fn push_category(categories: &mut Vec<String>, category: &str) {
    let category = category.trim();
    if category.is_empty() || category == "unknown" {
        return;
    }
    if !categories.iter().any(|value| value == category) {
        categories.push(category.to_string());
    }
}

fn inferred_foundation_categories(
    driver: &Driver,
    current_category: Option<&str>,
    stats: &[CategoryStats],
) -> Vec<String> {
    let Some(current_category) = current_category else {
        return Vec::new();
    };
    if current_category == "mazda_rookie" || current_category == "toyota_rookie" {
        return Vec::new();
    }
    if stats.iter().any(|entry| {
        let category = entry.category.as_str();
        category != current_category && category != "unknown"
    }) {
        return Vec::new();
    }
    if driver.stats_carreira.corridas == 0 && driver.stats_carreira.temporadas == 0 {
        return Vec::new();
    }

    inferred_ladder_for_category(&driver.id, current_category)
}

fn inferred_ladder_for_category(driver_id: &str, category: &str) -> Vec<String> {
    match category {
        "mazda_amador" => vec!["mazda_rookie".to_string()],
        "toyota_amador" => vec!["toyota_rookie".to_string()],
        "bmw_m2" => branded_foundation(driver_id),
        "production_challenger" => match stable_bucket(driver_id, 3) {
            0 => vec!["mazda_rookie".to_string(), "mazda_amador".to_string()],
            1 => vec!["toyota_rookie".to_string(), "toyota_amador".to_string()],
            _ => {
                let mut values = branded_foundation(driver_id);
                values.push("bmw_m2".to_string());
                values
            }
        },
        "gt4" => inferred_gt4_foundation(driver_id),
        "gt3" => {
            let mut ladder = inferred_gt4_foundation(driver_id);
            ladder.push("gt4".to_string());
            ladder
        }
        "lmp2" => {
            let mut ladder = inferred_gt4_foundation(driver_id);
            ladder.push("gt4".to_string());
            ladder.push("gt3".to_string());
            ladder
        }
        "endurance" => {
            let mut ladder = inferred_ladder_for_category(driver_id, "lmp2");
            ladder.push("lmp2".to_string());
            ladder
        }
        other => get_feeder_categories(other)
            .into_iter()
            .map(str::to_string)
            .collect(),
    }
}

fn inferred_gt4_foundation(driver_id: &str) -> Vec<String> {
    match stable_bucket(driver_id, 4) {
        0 => vec!["mazda_rookie".to_string(), "mazda_amador".to_string()],
        1 => vec!["toyota_rookie".to_string(), "toyota_amador".to_string()],
        2 => {
            let mut values = branded_foundation(driver_id);
            values.push("bmw_m2".to_string());
            values
        }
        _ => {
            let mut values = inferred_ladder_for_category(driver_id, "production_challenger");
            values.push("production_challenger".to_string());
            values
        }
    }
}

fn branded_foundation(driver_id: &str) -> Vec<String> {
    if stable_bucket(driver_id, 2) == 0 {
        vec!["mazda_rookie".to_string(), "mazda_amador".to_string()]
    } else {
        vec!["toyota_rookie".to_string(), "toyota_amador".to_string()]
    }
}

fn stable_bucket(value: &str, buckets: usize) -> usize {
    if buckets == 0 {
        return 0;
    }
    value.bytes().fold(0_usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    }) % buckets
}

fn score_category_stats(stats: &CategoryStats) -> f64 {
    balanced_score(
        &stats.category,
        stats.titles,
        stats.wins,
        stats.podiums,
        stats.poles,
        stats.points,
        stats.races,
        stats.dnfs,
    )
}

fn category_multiplier(category: &str) -> f64 {
    match category {
        "mazda_rookie" | "toyota_rookie" => 0.75,
        "mazda_amador" | "toyota_amador" => 0.85,
        "bmw_m2" => 0.95,
        "gt4" => 1.08,
        "production_challenger" => 1.12,
        "gt3" => 1.22,
        "lmp2" => 1.28,
        "endurance" => 1.30,
        _ => 1.0,
    }
}

fn balanced_score(
    category: &str,
    titles: i32,
    wins: i32,
    podiums: i32,
    poles: i32,
    points: f64,
    races: i32,
    dnfs: i32,
) -> f64 {
    let normalized_points = points.max(0.0).sqrt() * 0.4;
    let race_bonus = (races.max(0) as f64).sqrt() * 0.5;
    // Título pesa mais que volume de vitória: o que conta é CONVERTER vitórias em
    // campeonato, não só vencer muito (título 650 vs vitória 40 ≈ 16 vitórias/título).
    let base = titles as f64 * 650.0
        + wins as f64 * 40.0
        + podiums as f64 * 4.0
        + poles as f64 * 7.0
        + normalized_points
        + race_bonus
        - dnfs.max(0) as f64 * 1.5;
    round_one(base.max(0.0) * category_multiplier(category))
}

/// Categorias-base (sem classe) em que o piloto foi CAMPEÃO ao menos uma vez.
fn distinct_title_categories(stats: &[CategoryStats]) -> HashSet<String> {
    stats
        .iter()
        .filter(|entry| entry.titles > 0)
        .map(|entry| {
            entry
                .category
                .split(':')
                .next()
                .unwrap_or(&entry.category)
                .to_string()
        })
        .collect()
}

/// Bônus de amplitude: cada categoria DISTINTA conquistada além da primeira soma
/// 8% ao índice. Premia diversidade de títulos (subir a escada vencendo) em vez de
/// farmar a mesma categoria.
fn diversity_multiplier(stats: &[CategoryStats]) -> f64 {
    let distinct = distinct_title_categories(stats).len();
    1.0 + 0.08 * distinct.saturating_sub(1) as f64
}

/// Se o piloto foi campeão em alguma CLASSE específica (ex.: "lmp2"), em qualquer
/// categoria. LMP2 não é categoria autônoma — só existe como classe da Endurance,
/// então "ganhou lmp2" = tem título com `class_name == "lmp2"`.
fn won_class_title(stats: &[CategoryStats], class: &str) -> bool {
    stats
        .iter()
        .any(|entry| entry.titles > 0 && entry.class_name.as_deref() == Some(class))
}

/// Nº de CLASSES distintas em que o piloto foi campeão numa categoria-base
/// (usado nas especiais multiclasse: Production = mazda/toyota/bmw, Endurance =
/// gt4/gt3/lmp2). Título sem classe conta como 1.
fn titled_class_count(stats: &[CategoryStats], base_category: &str) -> usize {
    stats
        .iter()
        .filter(|entry| {
            entry.titles > 0
                && entry.category.split(':').next().unwrap_or(&entry.category) == base_category
        })
        .map(|entry| entry.class_name.clone().unwrap_or_default())
        .collect::<HashSet<String>>()
        .len()
}

/// Coroas de prestígio (bônus fixos, acumulam). Hierarquia definida pelo user:
///   prod(1) < Cup Slam < GT Slam < Production Slam < GT Super Slam < Endurance Slam.
/// - Cup Slam   = campeão mazda_amador + toyota_amador + bmw_m2.
/// - Production = especial multiclasse (escala por classe; 3 classes = Production Slam).
/// - GT Slam    = gt4 + gt3; vira GT SUPER SLAM ao somar LMP2 (super substitui o slam).
/// - Endurance  = especial multiclasse (escala; 3 classes = Endurance Slam = ouro).
fn crown_bonus(stats: &[CategoryStats]) -> f64 {
    let cats = distinct_title_categories(stats);
    let mut bonus = 0.0;

    // Cup Slam (entrada).
    if ["mazda_amador", "toyota_amador", "bmw_m2"]
        .iter()
        .all(|cat| cats.contains(*cat))
    {
        bonus += 1500.0;
    }

    // Production (especial), escalando até o Production Slam (3 classes).
    bonus += match titled_class_count(stats, "production_challenger") {
        0 => 0.0,
        1 => 800.0,
        2 => 1800.0,
        _ => 3500.0, // Production Slam (mazda + toyota + bmw)
    };

    // GT Slam (gt4 + gt3) → GT Super Slam ao somar LMP2. O super SUBSTITUI o slam.
    // LMP2 só existe como classe da Endurance → detecta pela classe, não pela base.
    if cats.contains("gt4") && cats.contains("gt3") {
        bonus += if won_class_title(stats, "lmp2") {
            5000.0
        } else {
            2500.0
        };
    }

    // Endurance (especial), escalando até o Endurance Slam (vale ouro).
    bonus += match titled_class_count(stats, "endurance") {
        0 => 0.0,
        1 => 2000.0,
        2 => 4500.0,
        _ => 8000.0, // Endurance Slam (gt4 + gt3 + lmp2)
    };

    bonus
}

/// Multiplicador de EFICIÊNCIA: premia quem converteu em título rápido (títulos por
/// temporada) e venceu muito por corrida. Só vale com volume mínimo (guarda contra
/// carreira-relâmpago) e tem teto. Sempre ≥ 1.0 — é upside, não pune carreira longa.
fn efficiency_multiplier(stats: &[CategoryStats]) -> f64 {
    let total_wins: i32 = stats.iter().map(|entry| entry.wins).sum();
    let total_races: i32 = stats.iter().map(|entry| entry.races).sum();
    let total_titles: i32 = stats.iter().map(|entry| entry.titles).sum();
    let seasons = stats.iter().filter(|entry| entry.races > 0).count() as i32;
    if total_races < 30 || seasons < 3 {
        return 1.0;
    }
    let win_rate = total_wins as f64 / total_races.max(1) as f64;
    let title_rate = total_titles as f64 / seasons.max(1) as f64;
    (1.0 + 0.7 * win_rate + 1.4 * title_rate).clamp(1.0, 2.6)
}

/// Índice histórico unificado — ativos e aposentados pontuam pela MESMA régua
/// (soma por categoria × diversidade × eficiência + bônus de coroa).
fn compute_historical_index(stats: &[CategoryStats]) -> f64 {
    let base: f64 = stats.iter().map(score_category_stats).sum();
    let value =
        base * diversity_multiplier(stats) * efficiency_multiplier(stats) + crown_bonus(stats);
    round_one(value.max(0.0))
}

/// Índice histórico (pedigree de CARREIRA) de UM piloto, para uso fora da tela de ranking
/// (ex: valor de mercado nas propostas formais). Mesma régua de `compute_historical_index`,
/// mas ignora títulos de construtores (aproximação barata: não constrói o mapa global de
/// títulos de equipe). Devolve 0.0 se não há arquivo de temporadas.
pub(crate) fn historical_index_for_driver(
    conn: &Connection,
    driver: &Driver,
) -> Result<f64, String> {
    let category = regular_category(driver.categoria_atual.as_deref(), None);
    // Um piloto só: filtra o agregado de `race_results` em vez de varrer a tabela.
    let real_career = RealCareerIndex::for_driver(conn, &driver.id)?;
    let stats = load_driver_category_stats(
        conn,
        driver,
        category.as_deref(),
        &TeamTitleStatsByDriver::new(),
        &real_career,
    )?;
    Ok(compute_historical_index(&stats))
}

fn assign_ranks(rows: &mut [GlobalDriverRankingRow]) {
    rows.sort_by(compare_historical_rows);
    for (index, row) in rows.iter_mut().enumerate() {
        row.historical_rank = index as i32 + 1;
    }
    assign_metric_rank(rows, |row| row.vitorias, |row, rank| row.wins_rank = rank);
    assign_metric_rank(rows, |row| row.titulos, |row, rank| row.titles_rank = rank);
    assign_metric_rank(rows, |row| row.podios, |row, rank| row.podiums_rank = rank);
    assign_metric_rank(rows, |row| row.lesoes, |row, rank| row.injuries_rank = rank);
}

fn assign_rank_deltas(
    conn: &Connection,
    rows: &mut [GlobalDriverRankingRow],
    stats_by_driver: &HashMap<String, Vec<CategoryStats>>,
) -> Result<(), String> {
    let contributions = load_latest_race_contributions(conn)?;
    if contributions.is_empty() {
        return Ok(());
    }

    let previous_ranks = previous_historical_ranks(rows, stats_by_driver, &contributions);
    for row in rows {
        if let Some(previous_rank) = previous_ranks.get(&row.id) {
            let delta = previous_rank - row.historical_rank;
            if delta != 0 {
                row.historical_rank_delta = Some(delta);
            }
        }
    }

    Ok(())
}

fn previous_historical_ranks(
    rows: &[GlobalDriverRankingRow],
    stats_by_driver: &HashMap<String, Vec<CategoryStats>>,
    contributions: &HashMap<String, Vec<RaceContribution>>,
) -> HashMap<String, i32> {
    let mut previous_rows = rows
        .iter()
        .map(|row| {
            let previous_index = stats_by_driver
                .get(&row.id)
                .map(|stats| previous_historical_index(stats, contributions.get(&row.id)))
                .unwrap_or(row.historical_index);
            let driver_contributions = contributions.get(&row.id).cloned().unwrap_or_default();
            let wins_delta = driver_contributions
                .iter()
                .map(|entry| entry.wins)
                .sum::<i32>();
            let podiums_delta = driver_contributions
                .iter()
                .map(|entry| entry.podiums)
                .sum::<i32>();

            (
                row.id.clone(),
                row.nome.clone(),
                previous_index,
                row.titulos,
                (row.vitorias - wins_delta).max(0),
                (row.podios - podiums_delta).max(0),
            )
        })
        .collect::<Vec<_>>();

    previous_rows.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.3.cmp(&a.3))
            .then_with(|| b.4.cmp(&a.4))
            .then_with(|| b.5.cmp(&a.5))
            .then_with(|| a.1.cmp(&b.1))
    });

    previous_rows
        .into_iter()
        .enumerate()
        .map(|(index, (id, _, _, _, _, _))| (id, index as i32 + 1))
        .collect()
}

fn previous_historical_index(
    stats: &[CategoryStats],
    contributions: Option<&Vec<RaceContribution>>,
) -> f64 {
    let Some(contributions) = contributions else {
        return compute_historical_index(stats);
    };
    let mut previous_stats = stats.to_vec();

    for contribution in contributions {
        if let Some(entry) = previous_stats
            .iter_mut()
            .find(|entry| entry.category == contribution.category)
        {
            entry.points = (entry.points - contribution.points).max(0.0);
            entry.wins = (entry.wins - contribution.wins).max(0);
            entry.podiums = (entry.podiums - contribution.podiums).max(0);
            entry.poles = (entry.poles - contribution.poles).max(0);
            entry.races = (entry.races - contribution.races).max(0);
            entry.dnfs = (entry.dnfs - contribution.dnfs).max(0);
        }
    }

    compute_historical_index(&previous_stats)
}

fn load_latest_race_contributions(
    conn: &Connection,
) -> Result<HashMap<String, Vec<RaceContribution>>, String> {
    if !table_exists(conn, "calendar")? || !table_exists(conn, "race_results")? {
        return Ok(HashMap::new());
    }

    let latest_race = conn
        .query_row(
            "SELECT c.id, c.categoria
             FROM calendar c
             JOIN seasons s ON c.temporada_id = s.id
             WHERE EXISTS (
                SELECT 1 FROM race_results r WHERE r.race_id = c.id
             )
             ORDER BY s.numero DESC, c.rodada DESC, c.id DESC
             LIMIT 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|e| format!("Falha ao carregar ultima corrida do ranking global: {e}"))?;

    let Some((race_id, category)) = latest_race else {
        return Ok(HashMap::new());
    };

    let mut stmt = conn
        .prepare(
            "SELECT piloto_id, pontos, posicao_largada, posicao_final, dnf
             FROM race_results
             WHERE race_id = ?1",
        )
        .map_err(|e| format!("Falha ao preparar resultados da ultima corrida global: {e}"))?;
    let rows = stmt
        .query_map(params![race_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
                row.get::<_, i32>(4)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar resultados da ultima corrida global: {e}"))?;

    let mut contributions: HashMap<String, Vec<RaceContribution>> = HashMap::new();
    for row in rows {
        let (driver_id, points, grid_position, finish_position, dnf) =
            row.map_err(|e| format!("Falha ao ler resultado da ultima corrida global: {e}"))?;
        contributions
            .entry(driver_id)
            .or_default()
            .push(RaceContribution {
                category: category.clone(),
                points,
                wins: if finish_position == 1 { 1 } else { 0 },
                podiums: if (1..=3).contains(&finish_position) {
                    1
                } else {
                    0
                },
                poles: if grid_position == 1 { 1 } else { 0 },
                races: 1,
                dnfs: if dnf != 0 { 1 } else { 0 },
            });
    }

    Ok(contributions)
}

fn compare_historical_rows(
    a: &GlobalDriverRankingRow,
    b: &GlobalDriverRankingRow,
) -> std::cmp::Ordering {
    b.historical_index
        .partial_cmp(&a.historical_index)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| b.titulos.cmp(&a.titulos))
        .then_with(|| b.vitorias.cmp(&a.vitorias))
        .then_with(|| b.podios.cmp(&a.podios))
        .then_with(|| a.nome.cmp(&b.nome))
}

fn assign_metric_rank(
    rows: &mut [GlobalDriverRankingRow],
    metric: fn(&GlobalDriverRankingRow) -> i32,
    assign: fn(&mut GlobalDriverRankingRow, i32),
) {
    let mut ordered = rows
        .iter()
        .enumerate()
        .map(|(index, row)| (index, metric(row), row.nome.clone()))
        .collect::<Vec<_>>();
    ordered.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.2.cmp(&b.2)));
    for (rank_index, (row_index, _, _)) in ordered.into_iter().enumerate() {
        assign(&mut rows[row_index], rank_index as i32 + 1);
    }
}

fn build_leaders(rows: &[GlobalDriverRankingRow]) -> GlobalDriverRankingLeaders {
    GlobalDriverRankingLeaders {
        historical_index_driver_id: rows.first().map(|row| row.id.clone()),
        wins_driver_id: leader_by(rows, |row| row.vitorias),
        titles_driver_id: leader_by(rows, |row| row.titulos),
        injuries_driver_id: leader_by(rows, |row| row.lesoes),
    }
}

fn has_competitive_history(row: &GlobalDriverRankingRow) -> bool {
    row.historical_index > 0.0
        || row.corridas > 0
        || row.pontos > 0
        || row.titulos > 0
        || row.vitorias > 0
        || row.podios > 0
        || row.poles > 0
        || row.dnfs > 0
}

fn has_ranking_visibility(row: &GlobalDriverRankingRow) -> bool {
    has_competitive_history(row) || is_current_regular_grid_driver(row)
}

fn is_current_regular_grid_driver(row: &GlobalDriverRankingRow) -> bool {
    row.status == "Ativo" && row.categoria_atual.is_some() && row.equipe_nome.is_some()
}

fn leader_by(
    rows: &[GlobalDriverRankingRow],
    metric: fn(&GlobalDriverRankingRow) -> i32,
) -> Option<String> {
    rows.iter()
        .max_by(|a, b| metric(a).cmp(&metric(b)).then_with(|| b.nome.cmp(&a.nome)))
        .map(|row| row.id.clone())
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
        params![table],
        |_| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(|e| format!("Falha ao verificar tabela {table}: {e}"))
}

fn json_i32(value: &Value, key: &str) -> i32 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0) as i32
}

fn json_i32_option(value: &Value, key: &str) -> Option<i32> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .map(|value| value as i32)
}

fn valid_archived_title_count(
    snapshot_titles: Option<i32>,
    championship_position: Option<i32>,
    points: f64,
    wins: i32,
    podiums: i32,
    poles: i32,
    races: i32,
) -> i32 {
    let title_count = snapshot_titles.unwrap_or_else(|| {
        if championship_position == Some(1) {
            1
        } else {
            0
        }
    });
    if title_count <= 0 || !has_title_worthy_participation(points, wins, podiums, poles, races) {
        0
    } else {
        title_count
    }
}

fn has_title_worthy_participation(
    points: f64,
    wins: i32,
    podiums: i32,
    poles: i32,
    races: i32,
) -> bool {
    races > 0 && (points > 0.0 || wins > 0 || podiums > 0 || poles > 0)
}

fn valid_archived_title_count_for_pilot(
    conn: &Connection,
    driver_id: &str,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
) -> Result<Option<i32>, String> {
    let mut total = 0;
    let mut saw_archive = false;
    let mut counted_title_events = HashSet::<TitleEventKey>::new();

    if table_exists(conn, "driver_season_archive")? {
        let mut stmt = conn
            .prepare(
                "SELECT categoria, pontos, snapshot_json, posicao_campeonato, season_number
                 FROM driver_season_archive
                 WHERE piloto_id = ?1",
            )
            .map_err(|e| format!("Falha ao preparar titulos historicos do piloto: {e}"))?;
        let rows = stmt
            .query_map(params![driver_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i32>>(3)?,
                    row.get::<_, i32>(4)?,
                ))
            })
            .map_err(|e| format!("Falha ao consultar titulos historicos do piloto: {e}"))?;

        for row in rows {
            let (category, points, snapshot_json, championship_position, season_number) =
                row.map_err(|e| format!("Falha ao ler titulo historico do piloto: {e}"))?;
            saw_archive = true;
            let snapshot: Value = serde_json::from_str(&snapshot_json).unwrap_or_default();
            let category = normalized_archive_category(&snapshot, category);
            let class_name =
                archived_title_class(conn, driver_id, &category, season_number, &snapshot)?;
            let points = json_f64(&snapshot, "pontos").unwrap_or(points);
            let titles = valid_archived_title_count(
                json_i32_option(&snapshot, "titulos"),
                championship_position,
                points,
                json_i32(&snapshot, "vitorias"),
                json_i32(&snapshot, "podios"),
                json_i32(&snapshot, "poles"),
                json_i32(&snapshot, "corridas"),
            );
            if titles > 0 {
                counted_title_events.insert(title_event_key(
                    season_number,
                    &category,
                    class_name.as_deref(),
                ));
                total += titles;
            }
        }
    }

    let team_title_stats = team_champion_title_stats_for_driver(
        driver_id,
        &counted_title_events,
        team_title_stats_by_driver,
    );
    saw_archive = saw_archive || !team_title_stats.is_empty();
    total += team_title_stats
        .iter()
        .map(|stats| stats.titles)
        .sum::<i32>();

    Ok(saw_archive.then_some(total))
}

fn valid_archived_title_categories_for_pilot(
    conn: &Connection,
    driver_id: &str,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
    team_lookup: &TeamLookup,
) -> Result<Option<Vec<GlobalDriverTitleCategorySummary>>, String> {
    let mut saw_archive = false;
    let mut totals = HashMap::<(String, Option<String>), (i32, Vec<TitleYear>)>::new();
    let mut counted_title_events = HashSet::<TitleEventKey>::new();

    if table_exists(conn, "driver_season_archive")? {
        let mut stmt = conn
            .prepare(
                "SELECT categoria, pontos, snapshot_json, posicao_campeonato, season_number, ano
                 FROM driver_season_archive
                 WHERE piloto_id = ?1",
            )
            .map_err(|e| format!("Falha ao preparar categorias campeas do piloto: {e}"))?;
        let rows = stmt
            .query_map(params![driver_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i32>>(3)?,
                    row.get::<_, i32>(4)?,
                    row.get::<_, i32>(5)?,
                ))
            })
            .map_err(|e| format!("Falha ao consultar categorias campeas do piloto: {e}"))?;

        for row in rows {
            let (category, points, snapshot_json, championship_position, season_number, year) =
                row.map_err(|e| format!("Falha ao ler categoria campea do piloto: {e}"))?;
            saw_archive = true;
            let snapshot: Value = serde_json::from_str(&snapshot_json).unwrap_or_default();
            let category = normalized_archive_category(&snapshot, category);
            let class_name =
                archived_title_class(conn, driver_id, &category, season_number, &snapshot)?;
            let points = json_f64(&snapshot, "pontos").unwrap_or(points);
            let titles = valid_archived_title_count(
                json_i32_option(&snapshot, "titulos"),
                championship_position,
                points,
                json_i32(&snapshot, "vitorias"),
                json_i32(&snapshot, "podios"),
                json_i32(&snapshot, "poles"),
                json_i32(&snapshot, "corridas"),
            );
            if titles > 0 {
                counted_title_events.insert(title_event_key(
                    season_number,
                    &category,
                    class_name.as_deref(),
                ));
                let title_team_id =
                    json_string(&snapshot, "team_id").filter(|value| !value.trim().is_empty());
                let total = totals.entry((category, class_name)).or_default();
                total.0 += titles;
                total
                    .1
                    .extend(title_years_for_event(titles, year, title_team_id));
            }
        }
    }

    let team_title_stats = team_champion_title_stats_for_driver(
        driver_id,
        &counted_title_events,
        team_title_stats_by_driver,
    );
    saw_archive = saw_archive || !team_title_stats.is_empty();
    for stats in team_title_stats {
        if stats.titles > 0 {
            let total = totals
                .entry((stats.category, stats.class_name))
                .or_default();
            total.0 += stats.titles;
            total.1.extend(stats.title_years);
        }
    }

    if !saw_archive {
        return Ok(None);
    }

    let mut summaries = totals
        .into_iter()
        .map(|((categoria, classe), (titulos, years))| {
            let (anos, anos_equipes) = build_title_year_teams(&years, team_lookup);
            GlobalDriverTitleCategorySummary {
                categoria,
                classe,
                titulos,
                anos,
                anos_equipes,
            }
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        right
            .titulos
            .cmp(&left.titulos)
            .then_with(|| left.categoria.cmp(&right.categoria))
            .then_with(|| left.classe.cmp(&right.classe))
    });
    Ok(Some(summaries))
}

fn team_champion_title_stats_for_driver(
    driver_id: &str,
    counted_title_events: &HashSet<TitleEventKey>,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
) -> Vec<CategoryStats> {
    let mut seen_team_events = HashSet::<TitleEventKey>::new();
    let mut stats = Vec::new();

    if let Some(driver_stats) = team_title_stats_by_driver.get(driver_id) {
        for (event_key, title_stats) in driver_stats {
            if counted_title_events.contains(event_key)
                || !seen_team_events.insert(event_key.clone())
            {
                continue;
            }
            stats.push(title_stats.clone());
        }
    }

    stats
}

fn push_team_title_stat(
    stats_by_driver: &mut TeamTitleStatsByDriver,
    driver_id: String,
    event_key: TitleEventKey,
    title_stats: CategoryStats,
) {
    stats_by_driver
        .entry(driver_id)
        .or_default()
        .push((event_key, title_stats));
}

fn load_all_team_champion_title_stats(conn: &Connection) -> Result<TeamTitleStatsByDriver, String> {
    let mut stats_by_driver = load_all_special_class_champion_title_stats(conn)?;

    if !table_exists(conn, "team_season_archive")? {
        return Ok(stats_by_driver);
    }

    let mut stmt = conn
        .prepare(
            "SELECT season_number, ano, categoria, classe, pontos, vitorias, podios, poles,
                    corridas, team_id, piloto_1_id, piloto_2_id
             FROM team_season_archive
             WHERE posicao_campeonato = 1",
        )
        .map_err(|e| format!("Falha ao preparar titulos de equipe do piloto: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
                row.get::<_, Option<i32>>(5)?.unwrap_or(0),
                row.get::<_, Option<i32>>(6)?.unwrap_or(0),
                row.get::<_, Option<i32>>(7)?.unwrap_or(0),
                row.get::<_, Option<i32>>(8)?.unwrap_or(0),
                row.get::<_, String>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar titulos de equipe do piloto: {e}"))?;

    for row in rows {
        let (
            season_number,
            year,
            category,
            class_name,
            points,
            wins,
            podiums,
            poles,
            races,
            team_id,
            driver_one_id,
            driver_two_id,
        ) = row.map_err(|e| format!("Falha ao ler titulo de equipe do piloto: {e}"))?;
        let category = if category.trim().is_empty() {
            "unknown".to_string()
        } else {
            category
        };
        if !uses_team_archive_title_fallback(&category) {
            continue;
        }
        let class_name = if let Some(class_name) = clean_optional_string(class_name) {
            Some(class_name)
        } else if let Some(class_name) =
            archived_special_entry_class(conn, &team_id, &category, season_number)?
        {
            Some(class_name)
        } else {
            let class_from_first = match driver_one_id.as_deref() {
                Some(driver_id) => {
                    archived_contract_class(conn, driver_id, &category, season_number)?
                }
                None => None,
            };
            if class_from_first.is_some() {
                class_from_first
            } else {
                match driver_two_id.as_deref() {
                    Some(driver_id) => {
                        archived_contract_class(conn, driver_id, &category, season_number)?
                    }
                    None => None,
                }
            }
        };
        let event_key = title_event_key(season_number, &category, class_name.as_deref());
        if !has_title_worthy_participation(points, wins, podiums, poles, races) {
            continue;
        }
        let Some(title_driver_id) = best_team_title_driver_id(
            conn,
            season_number,
            &category,
            Some(&team_id),
            driver_one_id,
            driver_two_id,
        )?
        else {
            continue;
        };
        push_team_title_stat(
            &mut stats_by_driver,
            title_driver_id,
            event_key,
            CategoryStats {
                category,
                class_name,
                points: 0.0,
                wins: 0,
                podiums: 0,
                poles: 0,
                races: 0,
                titles: 1,
                title_years: vec![TitleYear {
                    year,
                    team_id: Some(team_id.clone()),
                }],
                dnfs: 0,
            },
        );
    }

    Ok(stats_by_driver)
}

fn load_all_special_class_champion_title_stats(
    conn: &Connection,
) -> Result<TeamTitleStatsByDriver, String> {
    if !table_exists(conn, "special_team_entries")?
        || !table_exists(conn, "race_results")?
        || !table_exists(conn, "calendar")?
        || !table_exists(conn, "seasons")?
    {
        return Ok(HashMap::new());
    }

    let mut stmt = conn
        .prepare(
            "SELECT
                s.numero,
                s.ano,
                e.special_category,
                e.class_name,
                e.team_id,
                COALESCE(SUM(rr.pontos), 0.0) AS pontos,
                COALESCE(SUM(CASE WHEN rr.posicao_final = 1 THEN 1 ELSE 0 END), 0) AS vitorias,
                COALESCE(SUM(CASE WHEN rr.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END), 0) AS podios,
                COALESCE(SUM(CASE WHEN rr.posicao_largada = 1 THEN 1 ELSE 0 END), 0) AS poles,
                COUNT(DISTINCT rr.race_id) AS corridas
             FROM special_team_entries e
             INNER JOIN seasons s ON s.id = e.season_id
             INNER JOIN calendar c
                ON COALESCE(c.season_id, c.temporada_id) = e.season_id
               AND c.categoria = e.special_category
             INNER JOIN race_results rr
                ON rr.race_id = c.id
               AND rr.equipe_id = e.team_id
             WHERE e.special_category IN ('production_challenger', 'endurance')
             GROUP BY s.numero, s.ano, e.special_category, e.class_name, e.team_id
             HAVING COUNT(DISTINCT rr.race_id) > 0",
        )
        .map_err(|e| format!("Falha ao preparar campeoes especiais por classe: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            let season_number = row.get::<_, i32>(0)?;
            let year = row.get::<_, i32>(1)?;
            let category = row.get::<_, String>(2)?;
            let class_name = row.get::<_, String>(3)?;
            let team_id = row.get::<_, String>(4)?;
            let points = row.get::<_, f64>(5)?;
            let wins = row.get::<_, i32>(6)?;
            let podiums = row.get::<_, i32>(7)?;
            let poles = row.get::<_, i32>(8)?;
            let races = row.get::<_, i32>(9)?;
            let class_name = clean_optional_string(Some(class_name));
            let event_key = title_event_key(season_number, &category, class_name.as_deref());
            Ok(SpecialTeamTitleCandidate {
                event_key,
                season_number,
                year,
                category,
                class_name,
                team_id,
                points,
                wins,
                podiums,
                poles,
                races,
            })
        })
        .map_err(|e| format!("Falha ao consultar campeoes especiais por classe: {e}"))?;

    let mut candidates_by_event = HashMap::<TitleEventKey, Vec<SpecialTeamTitleCandidate>>::new();
    for row in rows {
        let candidate =
            row.map_err(|e| format!("Falha ao ler campeao especial por classe: {e}"))?;
        candidates_by_event
            .entry(candidate.event_key.clone())
            .or_default()
            .push(candidate);
    }

    let mut stats_by_driver = HashMap::new();
    for (event_key, mut candidates) in candidates_by_event {
        candidates.sort_by(compare_special_team_title_candidates);
        let Some(champion) = candidates.into_iter().next() else {
            continue;
        };
        let Some(title_driver_id) = best_team_title_driver_id(
            conn,
            champion.season_number,
            &champion.category,
            Some(&champion.team_id),
            None,
            None,
        )?
        else {
            continue;
        };
        push_team_title_stat(
            &mut stats_by_driver,
            title_driver_id,
            event_key,
            CategoryStats {
                category: champion.category,
                class_name: champion.class_name,
                points: 0.0,
                wins: 0,
                podiums: 0,
                poles: 0,
                races: 0,
                titles: 1,
                title_years: vec![TitleYear {
                    year: champion.year,
                    team_id: Some(champion.team_id),
                }],
                dnfs: 0,
            },
        );
    }

    Ok(stats_by_driver)
}

fn compare_special_team_title_candidates(
    left: &SpecialTeamTitleCandidate,
    right: &SpecialTeamTitleCandidate,
) -> std::cmp::Ordering {
    right
        .points
        .total_cmp(&left.points)
        .then_with(|| right.wins.cmp(&left.wins))
        .then_with(|| right.podiums.cmp(&left.podiums))
        .then_with(|| right.poles.cmp(&left.poles))
        .then_with(|| right.races.cmp(&left.races))
        .then_with(|| left.team_id.cmp(&right.team_id))
}

fn title_event_key(season_number: i32, category: &str, class_name: Option<&str>) -> TitleEventKey {
    (
        season_number,
        category.trim().to_string(),
        class_name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    )
}

fn uses_team_archive_title_fallback(category: &str) -> bool {
    matches!(category, "production_challenger" | "endurance")
}

fn best_team_title_driver_id(
    conn: &Connection,
    season_number: i32,
    category: &str,
    team_id: Option<&str>,
    driver_one_id: Option<String>,
    driver_two_id: Option<String>,
) -> Result<Option<String>, String> {
    let candidates = team_title_driver_candidates(
        conn,
        season_number,
        category,
        team_id,
        driver_one_id,
        driver_two_id,
    )?;
    if candidates.len() == 1 {
        return Ok(candidates.into_iter().next());
    }

    let mut scores = Vec::new();
    for driver_id in candidates {
        if let Some(score) =
            team_title_driver_score(conn, &driver_id, season_number, category, team_id)?
        {
            scores.push(score);
        }
    }
    scores.sort_by(compare_team_title_driver_scores);
    Ok(scores.into_iter().next().map(|score| score.driver_id))
}

fn team_title_driver_candidates(
    conn: &Connection,
    season_number: i32,
    category: &str,
    team_id: Option<&str>,
    driver_one_id: Option<String>,
    driver_two_id: Option<String>,
) -> Result<Vec<String>, String> {
    let mut candidates = [driver_one_id, driver_two_id]
        .into_iter()
        .flatten()
        .map(|driver_id| driver_id.trim().to_string())
        .filter(|driver_id| !driver_id.is_empty())
        .collect::<Vec<_>>();

    if let Some(team_id) = team_id.filter(|value| !value.trim().is_empty()) {
        if table_exists(conn, "race_results")?
            && table_exists(conn, "calendar")?
            && table_exists(conn, "seasons")?
        {
            let mut stmt = conn
                .prepare(
                    "SELECT DISTINCT rr.piloto_id
                     FROM race_results rr
                     INNER JOIN calendar c ON c.id = rr.race_id
                     INNER JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
                     WHERE rr.equipe_id = ?1
                       AND s.numero = ?2
                       AND c.categoria = ?3
                       AND rr.piloto_id IS NOT NULL
                       AND TRIM(rr.piloto_id) <> ''",
                )
                .map_err(|e| format!("Falha ao preparar pilotos da equipe campea: {e}"))?;
            let rows = stmt
                .query_map(params![team_id, season_number, category], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(|e| format!("Falha ao consultar pilotos da equipe campea: {e}"))?;
            for row in rows {
                let driver_id =
                    row.map_err(|e| format!("Falha ao ler piloto da equipe campea: {e}"))?;
                let driver_id = driver_id.trim().to_string();
                if !driver_id.is_empty() {
                    candidates.push(driver_id);
                }
            }
        }
    }

    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}

fn team_title_driver_score(
    conn: &Connection,
    driver_id: &str,
    season_number: i32,
    category: &str,
    team_id: Option<&str>,
) -> Result<Option<TeamTitleDriverScore>, String> {
    if !table_exists(conn, "race_results")?
        || !table_exists(conn, "calendar")?
        || !table_exists(conn, "seasons")?
    {
        return Ok(None);
    }

    let (points, wins, podiums, best_finish, races) = conn
        .query_row(
            "SELECT
                COALESCE(SUM(rr.pontos), 0.0),
                COALESCE(SUM(CASE WHEN rr.posicao_final = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN rr.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END), 0),
                COALESCE(MIN(NULLIF(rr.posicao_final, 0)), 9999),
                COUNT(*)
             FROM race_results rr
             INNER JOIN calendar c ON c.id = rr.race_id
             INNER JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
             WHERE rr.piloto_id = ?1
               AND s.numero = ?2
               AND c.categoria = ?3
               AND (?4 IS NULL OR rr.equipe_id = ?4)",
            params![driver_id, season_number, category, team_id],
            |row| {
                Ok((
                    row.get::<_, f64>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, i32>(2)?,
                    row.get::<_, i32>(3)?,
                    row.get::<_, i32>(4)?,
                ))
            },
        )
        .map_err(|e| format!("Falha ao pontuar piloto em titulo de equipe: {e}"))?;

    if races <= 0 {
        return Ok(None);
    }

    Ok(Some(TeamTitleDriverScore {
        driver_id: driver_id.to_string(),
        points,
        wins,
        podiums,
        best_finish,
        races,
    }))
}

fn compare_team_title_driver_scores(
    left: &TeamTitleDriverScore,
    right: &TeamTitleDriverScore,
) -> std::cmp::Ordering {
    right
        .points
        .total_cmp(&left.points)
        .then_with(|| right.wins.cmp(&left.wins))
        .then_with(|| right.podiums.cmp(&left.podiums))
        .then_with(|| left.best_finish.cmp(&right.best_finish))
        .then_with(|| right.races.cmp(&left.races))
        .then_with(|| left.driver_id.cmp(&right.driver_id))
}

fn title_years_for_event(titles: i32, year: i32, team_id: Option<String>) -> Vec<TitleYear> {
    if titles > 0 && year > 0 {
        vec![TitleYear { year, team_id }]
    } else {
        Vec::new()
    }
}

fn normalized_archive_category(snapshot: &Value, fallback: String) -> String {
    let category = json_string(snapshot, "categoria")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback);
    if category.trim().is_empty() {
        "unknown".to_string()
    } else {
        category
    }
}

fn clean_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn archived_title_class(
    conn: &Connection,
    driver_id: &str,
    category: &str,
    season_number: i32,
    snapshot: &Value,
) -> Result<Option<String>, String> {
    if let Some(class_name) = snapshot_class(snapshot) {
        return Ok(Some(class_name));
    }

    if let Some(team_id) = json_string(snapshot, "team_id").filter(|value| !value.trim().is_empty())
    {
        if let Some(class_name) = archived_team_class(conn, &team_id, category, season_number)? {
            return Ok(Some(class_name));
        }
        if let Some(class_name) =
            archived_special_entry_class(conn, &team_id, category, season_number)?
        {
            return Ok(Some(class_name));
        }
    }

    archived_contract_class(conn, driver_id, category, season_number)
}

fn snapshot_class(snapshot: &Value) -> Option<String> {
    ["classe", "class_name", "special_class"]
        .iter()
        .find_map(|key| json_string(snapshot, key))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn archived_team_class(
    conn: &Connection,
    team_id: &str,
    category: &str,
    season_number: i32,
) -> Result<Option<String>, String> {
    if !table_exists(conn, "team_season_archive")? {
        return Ok(None);
    }

    conn.query_row(
        "SELECT classe
         FROM team_season_archive
         WHERE team_id = ?1
           AND season_number = ?2
           AND categoria = ?3
           AND classe IS NOT NULL
           AND TRIM(classe) <> ''
         LIMIT 1",
        params![team_id, season_number, category],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| format!("Falha ao buscar classe historica da equipe: {e}"))
}

fn archived_special_entry_class(
    conn: &Connection,
    team_id: &str,
    category: &str,
    season_number: i32,
) -> Result<Option<String>, String> {
    if !table_exists(conn, "special_team_entries")? || !table_exists(conn, "seasons")? {
        return Ok(None);
    }

    conn.query_row(
        "SELECT e.class_name
         FROM special_team_entries e
         INNER JOIN seasons s ON s.id = e.season_id
         WHERE e.team_id = ?1
           AND e.special_category = ?2
           AND s.numero = ?3
           AND e.class_name IS NOT NULL
           AND TRIM(e.class_name) <> ''
         ORDER BY e.class_name
         LIMIT 1",
        params![team_id, category, season_number],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| format!("Falha ao buscar classe historica da inscricao especial: {e}"))
}

fn archived_contract_class(
    conn: &Connection,
    driver_id: &str,
    category: &str,
    season_number: i32,
) -> Result<Option<String>, String> {
    if !table_exists(conn, "contracts")? {
        return Ok(None);
    }

    conn.query_row(
        "SELECT classe
         FROM contracts
         WHERE piloto_id = ?1
           AND categoria = ?2
           AND classe IS NOT NULL
           AND TRIM(classe) <> ''
           AND CAST(temporada_inicio AS INTEGER) <= ?3
           AND CAST(temporada_fim AS INTEGER) >= ?3
         ORDER BY tipo DESC, created_at DESC
         LIMIT 1",
        params![driver_id, category, season_number],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| format!("Falha ao buscar classe historica do contrato: {e}"))
}

fn json_f64(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn json_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn round_one(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn years_since(start_year: i32, current_year: i32) -> Option<i32> {
    if start_year <= 0 || current_year <= 0 || current_year < start_year {
        return None;
    }
    Some(current_year - start_year + 1)
}

fn parse_year(value: &str) -> Option<i32> {
    parse_positive_i32(value).filter(|year| *year >= 1900)
}

fn parse_positive_i32(value: &str) -> Option<i32> {
    value.trim().parse::<i32>().ok().filter(|year| *year > 0)
}

#[cfg(test)]
#[path = "global_driver_rankings/tests/mod.rs"]
mod tests;
