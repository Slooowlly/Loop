//! Historico agregado da carreira: presenca, primeiros marcos, auge, mobilidade e lesoes, lidos do arquivo de temporadas e do corrida-a-corrida.

use super::*;

pub(super) fn build_career_history_block(
    conn: &Connection,
    driver_id: &str,
) -> Result<DriverCareerHistoryBlock, String> {
    let seasons = load_career_season_archive_rows(conn, driver_id)?;
    let races = load_career_race_history_rows(conn, driver_id)?;

    let active_seasons: Vec<&CareerSeasonArchiveRow> = seasons
        .iter()
        .filter(|season| season.corridas > 0)
        .collect();
    let mut categories = HashSet::new();
    for season in &active_seasons {
        if !season.categoria.trim().is_empty() {
            categories.insert(season.categoria.clone());
        }
    }

    let presenca = DriverCareerPresenceBlock {
        tempo_carreira: career_duration_from_archive(&seasons),
        temporadas_disputadas: active_seasons.len() as i32,
        anos_desempregado: seasons
            .iter()
            .filter(|season| season.corridas == 0 && season.categoria.trim().is_empty())
            .count() as i32,
        periodos_desempregado: unemployment_periods(&seasons),
        corridas: active_seasons
            .iter()
            .map(|season| season.corridas)
            .sum::<i32>(),
        categorias_disputadas: categories.len() as i32,
    };

    let primeiros_marcos = DriverCareerFirstMarksBlock {
        primeiro_podio_corrida: races
            .iter()
            .find(|race| !race.is_dnf && race.position <= 3)
            .map(|race| race.race_index),
        primeira_vitoria_corrida: races
            .iter()
            .find(|race| !race.is_dnf && race.position == 1)
            .map(|race| race.race_index),
        primeiro_dnf_corrida: races
            .iter()
            .find(|race| race.is_dnf)
            .map(|race| race.race_index),
        // O arquivo ja vem ordenado por temporada, entao o primeiro titulo e a
        // primeira linha que a regra do ranking mundial aceita como campeonato.
        primeiro_titulo: seasons
            .iter()
            .find(|season| archived_season_is_title(season))
            .map(season_block),
    };

    // Os anos da sequencia saem do numero da temporada, e nao do arquivo do
    // piloto: uma sequencia pode terminar na temporada EM CURSO, que ainda nao
    // foi arquivada — e era justamente essa que ficaria sem ano.
    let season_years = load_season_years(conn)?;
    let ano_da = |numero: Option<i32>| numero.and_then(|n| season_years.get(&n).copied());
    let (streak, streak_first, streak_last) = longest_win_streak_span(&races);
    let (podium_streak, podium_streak_first, podium_streak_last) =
        longest_podium_streak_span(&races);
    let auge = DriverCareerPeakBlock {
        melhor_temporada: best_career_season(&active_seasons),
        maior_sequencia_vitorias: streak,
        sequencia_ano_inicio: ano_da(streak_first),
        sequencia_ano_fim: ano_da(streak_last),
        maior_sequencia_podios: podium_streak,
        sequencia_podios_ano_inicio: ano_da(podium_streak_first),
        sequencia_podios_ano_fim: ano_da(podium_streak_last),
        temporadas_no_top3: active_seasons
            .iter()
            .filter(|season| matches!(season.posicao_campeonato, Some(posicao) if posicao <= 3))
            .count() as i32,
    };

    let (drought, drought_first, drought_last) = longest_winless_streak_span(&races);
    let (podium_drought, podium_first, podium_last) = longest_podiumless_streak_span(&races);
    let queda = DriverCareerDroughtBlock {
        maior_seca_vitorias: drought,
        seca_ano_inicio: ano_da(drought_first),
        seca_ano_fim: ano_da(drought_last),
        maior_seca_podios: podium_drought,
        seca_podios_ano_inicio: ano_da(podium_first),
        seca_podios_ano_fim: ano_da(podium_last),
        pior_temporada: worst_career_season(&active_seasons),
        temporadas_sem_podio: active_seasons
            .iter()
            .filter(|season| season.podios == 0)
            .count() as i32,
    };

    // O denominador e o corrida-a-corrida, e nao `presenca.corridas` (que soma o
    // arquivo): a taxa precisa contar abandonos e largadas na MESMA fonte, senao
    // a temporada em curso entra no numerador e fica de fora do denominador.
    let abandonos = races.iter().filter(|race| race.is_dnf).count() as i32;
    let corridas = races.len() as i32;
    let confiabilidade = DriverCareerReliabilityBlock {
        abandonos,
        corridas,
        taxa_abandono: (corridas > 0).then(|| {
            let raw = abandonos as f64 * 100.0 / corridas as f64;
            (raw * 10.0).round() / 10.0
        }),
        maior_sequencia_chegadas: longest_finish_streak(&races),
    };

    let sabado = build_qualifying_block(&races);
    let duelos = build_teammate_block(conn, driver_id)?;
    let referencias = build_benchmark_block(conn)?;
    // Os especiais vem ANTES do detalhe porque trazem as linhas do proprio hover
    // junto com o bloco — sao a mesma leitura de contratos e campanhas.
    let (eventos_especiais, detalhes_especiais) = build_special_events_block(conn, driver_id)?;
    let mut detalhes = build_career_details(
        conn,
        driver_id,
        &seasons,
        &races,
        &active_seasons,
        &auge,
        &queda,
        &confiabilidade,
        &duelos,
    )?;
    detalhes.extend(detalhes_especiais);
    // Os recordes saem VAZIOS daqui de proposito. Eles custam uma varredura do
    // mundo inteiro (ver `build_dossier_ranks`) e alimentam um toggle desligado
    // por padrao; a ficha os busca em `get_driver_dossier_ranks` quando — e se —
    // o jogador ligar. O campo continua no payload porque o front usa o que
    // estiver aqui como ponto de partida.
    let recordes = HashMap::new();

    let mobility_counts = count_category_mobility(&active_seasons);
    let team_summary = summarize_team_mobility(&races);
    let mobilidade = DriverCareerMobilityBlock {
        promocoes: mobility_counts.0,
        rebaixamentos: mobility_counts.1,
        equipes_defendidas: team_summary.0,
        tempo_medio_por_equipe: team_summary.1,
    };
    let injury_counts = injury_queries::count_injuries_by_severity_for_pilot(conn, driver_id)
        .map_err(|e| format!("Falha ao contar lesoes historicas do piloto: {e}"))?;
    let lesoes = DriverCareerInjuryBlock {
        leves: injury_counts.leves,
        moderadas: injury_counts.moderadas,
        graves: injury_counts.graves,
    };

    Ok(DriverCareerHistoryBlock {
        presenca,
        primeiros_marcos,
        auge,
        queda,
        confiabilidade,
        sabado,
        duelos,
        referencias,
        detalhes,
        recordes,
        mobilidade,
        lesoes,
        eventos_especiais,
    })
}

pub(super) fn unemployment_periods(seasons: &[CareerSeasonArchiveRow]) -> Vec<String> {
    let mut periods = Vec::new();
    let mut current_start: Option<i32> = None;
    let mut current_end: Option<i32> = None;

    for season in seasons {
        let unemployed = season.corridas == 0 && season.categoria.trim().is_empty();
        if unemployed {
            match current_end {
                Some(end) if season.ano == end + 1 => current_end = Some(season.ano),
                Some(end) => {
                    periods.push(format_year_period(current_start.unwrap_or(end), end));
                    current_start = Some(season.ano);
                    current_end = Some(season.ano);
                }
                None => {
                    current_start = Some(season.ano);
                    current_end = Some(season.ano);
                }
            }
        } else if let Some(end) = current_end {
            periods.push(format_year_period(current_start.unwrap_or(end), end));
            current_start = None;
            current_end = None;
        }
    }

    if let Some(end) = current_end {
        periods.push(format_year_period(current_start.unwrap_or(end), end));
    }

    periods
}

pub(super) fn career_duration_from_archive(seasons: &[CareerSeasonArchiveRow]) -> i32 {
    let Some(first_year) = seasons.iter().map(|season| season.ano).min() else {
        return 0;
    };
    let Some(last_year) = seasons.iter().map(|season| season.ano).max() else {
        return 0;
    };

    (last_year - first_year + 1).max(0)
}

pub(super) fn format_year_period(start: i32, end: i32) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}->{end}")
    }
}

/// Uma linha do arquivo de temporada a partir da coluna `base`, na ordem
/// `season_number, ano, categoria, posicao_campeonato, pontos, snapshot_json`.
///
/// Existe porque a ficha do piloto e o ranking do mundo leem o MESMO arquivo:
/// quase tudo (corridas, vitorias, podios, classe, equipe) mora dentro do
/// `snapshot_json`, e duas leituras diferentes dele divergiriam em silencio.
pub(super) fn season_archive_row_from(
    row: &rusqlite::Row,
    base: usize,
) -> rusqlite::Result<CareerSeasonArchiveRow> {
    let snapshot_json: String = row.get(base + 5)?;
    let snapshot: serde_json::Value = serde_json::from_str(&snapshot_json).unwrap_or_default();
    let categoria: String = row.get(base + 2)?;
    Ok(CareerSeasonArchiveRow {
        season_number: row.get(base)?,
        ano: row.get(base + 1)?,
        categoria: snapshot
            .get("categoria")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(categoria.as_str())
            .to_string(),
        classe: snapshot
            .get("classe")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        posicao_campeonato: row.get(base + 3)?,
        pontos: snapshot
            .get("pontos")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(row.get(base + 4)?),
        corridas: snapshot
            .get("corridas")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0) as i32,
        vitorias: snapshot
            .get("vitorias")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0) as i32,
        podios: snapshot
            .get("podios")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0) as i32,
        poles: snapshot
            .get("poles")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0) as i32,
        titulos: snapshot
            .get("titulos")
            .and_then(serde_json::Value::as_i64)
            .map(|value| value as i32),
        equipe_id: snapshot
            .get("team_id")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    })
}

pub(super) fn load_career_season_archive_rows(
    conn: &Connection,
    driver_id: &str,
) -> Result<Vec<CareerSeasonArchiveRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT season_number, ano, categoria, posicao_campeonato, pontos, snapshot_json
             FROM driver_season_archive
             WHERE piloto_id = ?1
             ORDER BY season_number ASC",
        )
        .map_err(|e| format!("Falha ao preparar historico de temporadas do piloto: {e}"))?;
    let mapped = stmt
        .query_map(rusqlite::params![driver_id], |row| {
            season_archive_row_from(row, 0)
        })
        .map_err(|e| format!("Falha ao consultar historico de temporadas do piloto: {e}"))?;

    let mut rows = Vec::new();
    for row in mapped {
        rows.push(row.map_err(|e| format!("Falha ao ler historico de temporada: {e}"))?);
    }
    Ok(rows)
}

pub(super) fn load_career_race_history_rows(
    conn: &Connection,
    driver_id: &str,
) -> Result<Vec<CareerRaceHistoryRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                COALESCE(s.numero, 0) AS season_number,
                COALESCE(NULLIF(r.equipe_id, ''), '-') AS equipe_id,
                r.posicao_final,
                r.dnf,
                COALESCE(r.posicao_largada, 0),
                COALESCE(r.fastest_lap, 0),
                r.race_id,
                COALESCE(s.ano, 0),
                COALESCE(c.rodada, 0),
                COALESCE(NULLIF(c.track_name, ''), NULLIF(c.pista, '')),
                NULLIF(c.categoria, ''),
                NULLIF(c.data, '')
             FROM race_results r
             INNER JOIN calendar c ON c.id = r.race_id
             LEFT JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
             WHERE r.piloto_id = ?1
             ORDER BY COALESCE(s.numero, 0) ASC, c.rodada ASC, r.id ASC",
        )
        .map_err(|e| format!("Falha ao preparar historico corrida-a-corrida: {e}"))?;
    let mapped = stmt
        .query_map(rusqlite::params![driver_id], |row| {
            Ok(CareerRaceHistoryRow {
                // O indice real e carimbado depois, na ordem da consulta.
                race_index: 0,
                season_number: row.get(0)?,
                team_id: row.get(1)?,
                position: row.get(2)?,
                is_dnf: row.get::<_, i32>(3)? != 0,
                grid_position: row.get(4)?,
                has_fastest_lap: row.get::<_, i32>(5)? != 0,
                race_id: row.get(6)?,
                ano: row.get(7)?,
                rodada: row.get(8)?,
                pista: row.get(9)?,
                categoria: row.get(10)?,
                data: row.get(11)?,
            })
        })
        .map_err(|e| format!("Falha ao consultar historico corrida-a-corrida: {e}"))?;

    let mut rows = Vec::new();
    for (index, row) in mapped.enumerate() {
        let mut row = row.map_err(|e| format!("Falha ao ler historico corrida-a-corrida: {e}"))?;
        row.race_index = index as i32 + 1;
        rows.push(row);
    }
    Ok(rows)
}

/// O sabado da carreira, lido do corrida-a-corrida.
///
/// Grid `0` e corrida sem largada registrada: fica de fora da media e nao conta
/// pole. Inventar uma pole a partir de dado ausente seria pior que nao ter o
/// card.
pub(super) fn build_qualifying_block(
    races: &[CareerRaceHistoryRow],
) -> DriverCareerQualifyingBlock {
    let com_grid: Vec<&CareerRaceHistoryRow> =
        races.iter().filter(|race| race.grid_position > 0).collect();
    let poles = com_grid
        .iter()
        .filter(|race| race.grid_position == 1)
        .count() as i32;

    DriverCareerQualifyingBlock {
        poles,
        poles_convertidas: com_grid
            .iter()
            .filter(|race| race.grid_position == 1 && !race.is_dnf && race.position == 1)
            .count() as i32,
        grid_medio: (!com_grid.is_empty()).then(|| {
            let soma: i32 = com_grid.iter().map(|race| race.grid_position).sum();
            let media = soma as f64 / com_grid.len() as f64;
            (media * 10.0).round() / 10.0
        }),
        voltas_rapidas: races.iter().filter(|race| race.has_fastest_lap).count() as i32,
    }
}

/// As medias do mundo para os numeros que sozinhos nao dizem nada.
///
/// Sao agregados de UMA consulta cada, sobre a tabela inteira de resultados —
/// nao uma media das medias por piloto. A diferenca entre as duas e pequena e o
/// custo nao e: a ficha abre a cada clique numa linha do ranking.
pub(super) fn build_benchmark_block(
    conn: &Connection,
) -> Result<DriverCareerBenchmarkBlock, String> {
    let (largadas, abandonos, grid_soma, grid_contagem) = conn
        .query_row(
            "SELECT
                COUNT(*),
                COALESCE(SUM(CASE WHEN dnf <> 0 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN posicao_largada > 0 THEN posicao_largada ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN posicao_largada > 0 THEN 1 ELSE 0 END), 0)
             FROM race_results",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .map_err(|e| format!("Falha ao consultar referencias do mundo: {e}"))?;

    let arredonda = |valor: f64| (valor * 10.0).round() / 10.0;
    Ok(DriverCareerBenchmarkBlock {
        taxa_abandono: (largadas > 0)
            .then(|| arredonda(abandonos as f64 * 100.0 / largadas as f64)),
        grid_medio: (grid_contagem > 0).then(|| arredonda(grid_soma as f64 / grid_contagem as f64)),
    })
}

/// Uma temporada arquivada como o bloco que a ficha desenha. Melhor temporada,
/// pior temporada e primeiro titulo sao a MESMA coisa vista de angulos
/// diferentes, entao saem todas por aqui.
pub(super) fn season_block(season: &CareerSeasonArchiveRow) -> DriverBestSeasonBlock {
    DriverBestSeasonBlock {
        ano: season.ano,
        categoria: season.categoria.clone(),
        posicao_campeonato: season.posicao_campeonato,
        pontos: season.pontos.round() as i32,
        vitorias: season.vitorias,
        podios: season.podios,
    }
}

pub(super) fn best_career_season(
    seasons: &[&CareerSeasonArchiveRow],
) -> Option<DriverBestSeasonBlock> {
    seasons
        .iter()
        .copied()
        .max_by(|a, b| {
            best_season_score(a)
                .cmp(&best_season_score(b))
                .then_with(|| a.pontos.total_cmp(&b.pontos))
                .then_with(|| a.vitorias.cmp(&b.vitorias))
                .then_with(|| a.podios.cmp(&b.podios))
        })
        .map(season_block)
}

/// A pior temporada da carreira, pela MESMA regra que elege a melhor — so que
/// pegando o minimo.
///
/// Com UMA temporada so devolve `None` de proposito: a melhor e a pior seriam a
/// mesma linha, e o card diria duas vezes a mesma coisa com sinais opostos.
/// Chamar a temporada de estreia de um novato de "a pior da carreira" e ruido,
/// nao informacao.
pub(super) fn worst_career_season(
    seasons: &[&CareerSeasonArchiveRow],
) -> Option<DriverBestSeasonBlock> {
    if seasons.len() < 2 {
        return None;
    }
    seasons
        .iter()
        .copied()
        .min_by(|a, b| {
            best_season_score(a)
                .cmp(&best_season_score(b))
                .then_with(|| a.pontos.total_cmp(&b.pontos))
                .then_with(|| a.vitorias.cmp(&b.vitorias))
                .then_with(|| a.podios.cmp(&b.podios))
        })
        .map(season_block)
}

pub(super) fn best_season_score(season: &CareerSeasonArchiveRow) -> i32 {
    let position_score = season
        .posicao_campeonato
        .map(|position| (50 - position).max(0) * 100)
        .unwrap_or(0);
    position_score + season.vitorias * 15 + season.podios * 5 + season.pontos.round() as i32
}

/// Maior sequencia de vitorias e as temporadas em que ela aconteceu.
///
/// A sequencia e contada em CORRIDAS consecutivas, nao dentro de uma temporada:
/// vencer as duas ultimas do ano e as duas primeiras do seguinte e uma sequencia
/// de quatro, e por isso o intervalo devolve inicio e fim (que podem ser a mesma
/// temporada). Em empate fica a PRIMEIRA — a marca e a mesma, e a primeira vez
/// e a que conta como o momento em que ele fez aquilo.
pub(super) fn longest_win_streak_span(
    races: &[CareerRaceHistoryRow],
) -> (i32, Option<i32>, Option<i32>) {
    longest_streak_span(races, |race| !race.is_dnf && race.position == 1)
}

/// Maior JEJUM de vitorias, em corridas consecutivas sem vencer.
///
/// E o espelho exato do auge — mesma varredura, condicao invertida — e tem que
/// ser, senao as duas marcas nao sao comparaveis. Um DNF conta como corrida sem
/// vitoria: ele largou e nao venceu, e a seca e sobre isso.
pub(super) fn longest_winless_streak_span(
    races: &[CareerRaceHistoryRow],
) -> (i32, Option<i32>, Option<i32>) {
    longest_streak_span(races, |race| race.is_dnf || race.position != 1)
}

/// Maior sequencia de PODIOS consecutivos. O espelho do jejum de podios: as
/// duas marcas so significam alguma coisa lado a lado.
pub(super) fn longest_podium_streak_span(
    races: &[CareerRaceHistoryRow],
) -> (i32, Option<i32>, Option<i32>) {
    longest_streak_span(races, |race| !race.is_dnf && race.position <= 3)
}

/// Maior jejum de PODIOS, em corridas consecutivas sem subir ao podio.
///
/// Mede uma queda mais funda que a de vitorias, e e a marca que serve para quem
/// nao e vencedor: um piloto de meio de grid passa a carreira inteira sem vencer,
/// e dizer que ele esta em jejum de vitorias ha 130 corridas nao informa nada.
pub(super) fn longest_podiumless_streak_span(
    races: &[CareerRaceHistoryRow],
) -> (i32, Option<i32>, Option<i32>) {
    longest_streak_span(races, |race| race.is_dnf || race.position > 3)
}

/// Maior sequencia de corridas COMPLETADAS, sem abandono no meio.
pub(super) fn longest_finish_streak(races: &[CareerRaceHistoryRow]) -> i32 {
    longest_streak_span(races, |race| !race.is_dnf).0
}

/// A maior sequencia de corridas consecutivas que satisfazem `matches`, e as
/// temporadas em que ela comecou e terminou.
///
/// A contagem e em CORRIDAS, nao dentro da temporada: uma sequencia atravessa a
/// virada do ano, e por isso o retorno traz o par de temporadas (que podem ser a
/// mesma). Em empate fica a PRIMEIRA — a marca e identica, e a primeira vez e a
/// que conta como o momento em que ele fez aquilo.
fn longest_streak_span<F>(
    races: &[CareerRaceHistoryRow],
    matches: F,
) -> (i32, Option<i32>, Option<i32>)
where
    F: Fn(&CareerRaceHistoryRow) -> bool,
{
    let mut current = 0;
    let mut current_start: Option<i32> = None;
    let mut best = 0;
    let mut best_start: Option<i32> = None;
    let mut best_end: Option<i32> = None;

    for race in races {
        if matches(race) {
            current += 1;
            if current == 1 {
                current_start = Some(race.season_number);
            }
            if current > best {
                best = current;
                best_start = current_start;
                best_end = Some(race.season_number);
            }
        } else {
            current = 0;
            current_start = None;
        }
    }

    (best, best_start, best_end)
}

pub(super) fn longest_win_streak(races: &[CareerRaceHistoryRow]) -> i32 {
    longest_win_streak_span(races).0
}

/// Numero da temporada -> ano. Cobre tambem a temporada em curso, que ainda nao
/// tem linha no arquivo do piloto mas ja tem corridas em `race_results`.
pub(super) fn load_season_years(conn: &Connection) -> Result<HashMap<i32, i32>, String> {
    let mut stmt = conn
        .prepare("SELECT numero, ano FROM seasons")
        .map_err(|e| format!("Falha ao preparar anos das temporadas: {e}"))?;
    let mapped = stmt
        .query_map([], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?)))
        .map_err(|e| format!("Falha ao consultar anos das temporadas: {e}"))?;

    let mut years = HashMap::new();
    for row in mapped {
        let (numero, ano) = row.map_err(|e| format!("Falha ao ler ano de temporada: {e}"))?;
        years.insert(numero, ano);
    }
    Ok(years)
}

pub(super) fn count_category_mobility(seasons: &[&CareerSeasonArchiveRow]) -> (i32, i32) {
    let mut promocoes = 0;
    let mut rebaixamentos = 0;
    let mut previous_tier = None;
    for season in seasons {
        let Some(tier) =
            categories::get_category_config(&season.categoria).map(|config| config.tier)
        else {
            continue;
        };
        if let Some(previous) = previous_tier {
            if tier > previous {
                promocoes += 1;
            } else if tier < previous {
                rebaixamentos += 1;
            }
        }
        previous_tier = Some(tier);
    }
    (promocoes, rebaixamentos)
}

pub(super) fn summarize_team_mobility(races: &[CareerRaceHistoryRow]) -> (i32, Option<f64>) {
    let mut teams = HashSet::new();
    let mut team_seasons = HashSet::new();
    for race in races {
        if race.team_id == "-" {
            continue;
        }
        teams.insert(race.team_id.clone());
        team_seasons.insert((race.season_number, race.team_id.clone()));
    }
    let team_count = teams.len() as i32;
    let average = if team_count > 0 {
        let raw = team_seasons.len() as f64 / team_count as f64;
        Some((raw * 10.0).round() / 10.0)
    } else {
        None
    };
    (team_count, average)
}
