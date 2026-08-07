//! Tela "Campeão da Temporada": monta o payload do pop-up de fim de campeonato a
//! partir do que REALMENTE aconteceu na pista.
//!
//! A única fonte é `race_results` cruzado com o `calendar` da categoria — nada vem
//! de stats agregadas do piloto, para que os totais do gráfico, do pódio e dos
//! recordes fechem entre si. Um recorde ou prêmio sem base nos resultados é
//! OMITIDO em vez de preenchido com placeholder.
//!
//! Nada de prosa aqui: cada linha viaja como `id` + valores, e o rótulo/frase mora
//! no i18n do frontend (`seasonChampion.*`).

use std::collections::BTreeMap;

use super::*;

use crate::commands::career_types::{
    SeasonChampionAward, SeasonChampionConstructor, SeasonChampionConstructorDriver,
    SeasonChampionDriver, SeasonChampionPayload, SeasonChampionRecord, SeasonChampionStanding,
};

/// Quantos pilotos entram no gráfico e na legenda do pop-up.
const CHART_DRIVERS: usize = 4;

/// Duelos precisam de repetição para virar história — abaixo disto é coincidência.
const MIN_DUEL_ENCOUNTERS: i32 = 3;

/// De onde uma vitória deixa de ser esperada e vira zebra. Ganhar da primeira fila
/// é o script; da terceira para trás é a corrida que se conta depois.
const MIN_UPSET_GRID: i32 = 4;

/// Uma etapa já disputada da categoria, na ordem do calendário.
struct FinishedRound {
    id: String,
    track: String,
    is_wet: bool,
}

/// O que um piloto fez em UMA etapa.
struct DriverRound {
    round_index: usize,
    grid: i32,
    finish: i32,
    dnf: bool,
    points: f64,
    fastest_lap: bool,
    /// A camisa daquele dia — guardada por etapa, e não só por piloto, para que o
    /// campeonato de construtores não credite o ano inteiro a quem trocou de time no
    /// meio dele.
    equipe: Option<String>,
    equipe_cor: Option<String>,
}

/// A temporada inteira de um piloto na categoria, montada a partir das etapas.
#[derive(Default)]
struct DriverSeason {
    id: String,
    nome: String,
    nacionalidade: String,
    is_player: bool,
    is_rookie: bool,
    equipe: Option<String>,
    equipe_cor: Option<String>,
    rounds: Vec<DriverRound>,
}

impl DriverSeason {
    fn pontos(&self) -> f64 {
        self.rounds.iter().map(|round| round.points).sum()
    }

    fn vitorias(&self) -> i32 {
        self.rounds
            .iter()
            .filter(|round| !round.dnf && round.finish == 1)
            .count() as i32
    }

    fn podios(&self) -> i32 {
        self.rounds
            .iter()
            .filter(|round| !round.dnf && round.finish >= 1 && round.finish <= 3)
            .count() as i32
    }

    fn poles(&self) -> i32 {
        self.rounds.iter().filter(|round| round.grid == 1).count() as i32
    }

    fn voltas_rapidas(&self) -> i32 {
        self.rounds.iter().filter(|round| round.fastest_lap).count() as i32
    }

    fn abandonos(&self) -> i32 {
        self.rounds.iter().filter(|round| round.dnf).count() as i32
    }

    /// Etapas levadas até a bandeirada — o outro lado do recorde de abandonos.
    fn chegadas(&self) -> i32 {
        self.rounds
            .iter()
            .filter(|round| !round.dnf && round.finish > 0)
            .count() as i32
    }

    /// Vitórias saindo da pole: converteu o sábado no domingo.
    fn vitorias_da_pole(&self) -> i32 {
        self.rounds
            .iter()
            .filter(|round| !round.dnf && round.grid == 1 && round.finish == 1)
            .count() as i32
    }

    /// Maior sequência de vitórias em etapas CONSECUTIVAS do calendário — vencer a 1ª
    /// e a 3ª não é sequência, e quem faltou à 2ª também quebra a corrente.
    fn maior_sequencia_vitorias(&self) -> i32 {
        let mut best = 0;
        let mut run = 0;
        let mut next_expected: Option<usize> = None;
        for round in &self.rounds {
            if !round.dnf && round.finish == 1 {
                run = if next_expected == Some(round.round_index) {
                    run + 1
                } else {
                    1
                };
                best = best.max(run);
                next_expected = Some(round.round_index + 1);
            } else {
                run = 0;
                next_expected = None;
            }
        }
        best
    }

    fn posicoes_ganhas(&self) -> i32 {
        self.rounds
            .iter()
            .filter(|round| !round.dnf && round.grid > 0 && round.finish > 0)
            .map(|round| (round.grid - round.finish).max(0))
            .sum()
    }

    fn posicoes_perdidas(&self) -> i32 {
        self.rounds
            .iter()
            .filter(|round| !round.dnf && round.grid > 0 && round.finish > 0)
            .map(|round| (round.finish - round.grid).max(0))
            .sum()
    }

    /// Melhor chegada da temporada, ignorando abandono. Sem chegada válida devolve
    /// o pior valor possível — assim quem nunca terminou cai para o fim do empate.
    fn melhor_chegada(&self) -> i32 {
        self.rounds
            .iter()
            .filter(|round| !round.dnf && round.finish > 0)
            .map(|round| round.finish)
            .min()
            .unwrap_or(i32::MAX)
    }

    /// Média de chegada e QUANTAS chegadas entraram nela. A contagem é parte do
    /// resultado porque o corte de amostra tem de ser sobre etapas terminadas: quem
    /// abandonou dez vezes e venceu duas fecharia uma "média 1.0" imbatível.
    fn media_chegada(&self) -> Option<(f64, usize)> {
        let finishes: Vec<i32> = self
            .rounds
            .iter()
            .filter(|round| !round.dnf && round.finish > 0)
            .map(|round| round.finish)
            .collect();
        if finishes.is_empty() {
            return None;
        }
        Some((
            finishes.iter().sum::<i32>() as f64 / finishes.len() as f64,
            finishes.len(),
        ))
    }

    fn media_grid(&self) -> Option<(f64, usize)> {
        let grids: Vec<i32> = self
            .rounds
            .iter()
            .filter(|round| round.grid > 0)
            .map(|round| round.grid)
            .collect();
        if grids.is_empty() {
            return None;
        }
        Some((
            grids.iter().sum::<i32>() as f64 / grids.len() as f64,
            grids.len(),
        ))
    }

    /// Pontos acumulados etapa a etapa. A curva não pode ter buraco: quem faltou a
    /// uma rodada repete o acumulado anterior, senão a linha do gráfico despenca.
    fn cumulative(&self, total_rounds: usize) -> Vec<f64> {
        let mut by_round = vec![0.0; total_rounds];
        for round in &self.rounds {
            if let Some(slot) = by_round.get_mut(round.round_index) {
                *slot += round.points;
            }
        }
        let mut running = 0.0;
        by_round
            .into_iter()
            .map(|points| {
                running += points;
                running
            })
            .collect()
    }
}

/// `season_number` = `None` significa a temporada ativa. Passar um número abre o
/// campeonato daquele ano — o calendário e os `race_results` de temporadas passadas
/// continuam no banco, nada os apaga na virada.
pub(crate) fn get_season_champion_payload_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    category: Option<&str>,
    season_number: Option<i32>,
) -> Result<Option<SeasonChampionPayload>, String> {
    let (db, _, _) = open_career_resources_read_only(base_dir, career_id)?;
    let season = match season_number {
        Some(numero) => {
            let seasons = season_queries::get_all_seasons(&db.conn)
                .map_err(|e| format!("Falha ao listar temporadas: {e}"))?;
            match seasons.into_iter().find(|s| s.numero == numero) {
                Some(found) => found,
                None => return Ok(None),
            }
        }
        None => season_queries::get_active_season(&db.conn)
            .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
            .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?,
    };
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;

    let category = match category
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        // A categoria do JOGADOR NAQUELE ano — ele pode ter subido de escada desde
        // então, e abrir a temporada passada na categoria de hoje mostraria o
        // campeonato errado.
        .or_else(|| player_category_in_season(&db.conn, &player.id, &season.id))
        .or_else(|| player.categoria_atual.clone())
    {
        Some(value) => value,
        // Jogador sem categoria (agente livre que nunca correu): não há campeonato
        // "dele" para celebrar.
        None => return Ok(None),
    };

    let rounds = load_finished_rounds(&db.conn, &season.id, &category)?;
    if rounds.is_empty() {
        return Ok(None);
    }

    let mut drivers = load_driver_seasons(&db.conn, &rounds, &player.id)?;
    if drivers.is_empty() {
        return Ok(None);
    }

    // Mesma régua da classificação da Home: pontos, vitórias, pódios, melhor
    // chegada na pista e, por último, o nome.
    drivers.sort_by(|a, b| {
        b.pontos()
            .partial_cmp(&a.pontos())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.vitorias().cmp(&a.vitorias()))
            .then_with(|| b.podios().cmp(&a.podios()))
            .then_with(|| a.melhor_chegada().cmp(&b.melhor_chegada()))
            .then_with(|| a.nome.cmp(&b.nome))
    });

    let total_rounds = rounds.len();
    let champion_id = drivers[0].id.clone();
    let margin = if drivers.len() > 1 {
        (drivers[0].pontos() - drivers[1].pontos()).max(0.0)
    } else {
        drivers[0].pontos()
    };

    let chart_drivers: Vec<SeasonChampionDriver> = drivers
        .iter()
        .take(CHART_DRIVERS)
        .enumerate()
        .map(|(index, driver)| SeasonChampionDriver {
            id: driver.id.clone(),
            nome: driver.nome.clone(),
            equipe: driver.equipe.clone(),
            equipe_cor: driver.equipe_cor.clone(),
            nacionalidade: driver.nacionalidade.clone(),
            posicao: index as i32 + 1,
            pontos: driver.pontos(),
            vitorias: driver.vitorias(),
            podios: driver.podios(),
            poles: driver.poles(),
            voltas_rapidas: driver.voltas_rapidas(),
            cumulative: driver.cumulative(total_rounds),
            is_champion: driver.id == champion_id,
            is_player: driver.is_player,
        })
        .collect();

    let standings: Vec<SeasonChampionStanding> = drivers
        .iter()
        .enumerate()
        .map(|(index, driver)| SeasonChampionStanding {
            id: driver.id.clone(),
            nome: driver.nome.clone(),
            equipe: driver.equipe.clone(),
            equipe_cor: driver.equipe_cor.clone(),
            posicao: index as i32 + 1,
            pontos: driver.pontos(),
            is_player: driver.is_player,
        })
        .collect();

    let constructors = build_constructors(&drivers, total_rounds);

    Ok(Some(SeasonChampionPayload {
        year: season.ano,
        season_number: season.numero,
        category_id: category,
        rounds: total_rounds as i32,
        player_is_champion: champion_id == player.id,
        margin,
        awards: build_awards(&drivers, &rounds, &champion_id, &constructors),
        records: build_records(&drivers, &rounds),
        drivers: chart_drivers,
        standings,
        constructors,
    }))
}

/// Em que categoria o jogador correu numa dada temporada, pelos resultados que ele
/// deixou no calendário dela. `None` quando ele não correu nenhuma etapa daquele ano.
fn player_category_in_season(
    conn: &rusqlite::Connection,
    player_id: &str,
    season_id: &str,
) -> Option<String> {
    conn.query_row(
        "SELECT c.categoria
         FROM race_results rr
         JOIN calendar c ON c.id = rr.race_id
         WHERE rr.piloto_id = ?1
           AND COALESCE(c.season_id, c.temporada_id) = ?2
         GROUP BY c.categoria
         ORDER BY COUNT(*) DESC
         LIMIT 1",
        rusqlite::params![player_id, season_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
    .filter(|value| !value.trim().is_empty())
}

/// Etapas da categoria já disputadas, na ordem em que rodaram.
fn load_finished_rounds(
    conn: &rusqlite::Connection,
    season_id: &str,
    category: &str,
) -> Result<Vec<FinishedRound>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, track_name, pista, clima
             FROM calendar
             WHERE COALESCE(season_id, temporada_id) = ?1
               AND categoria = ?2
               AND status <> 'Pendente'
             ORDER BY COALESCE(season_week, week_of_year + 4) ASC, rodada ASC",
        )
        .map_err(|e| format!("Falha ao preparar etapas da categoria: {e}"))?;

    let mapped = stmt
        .query_map(rusqlite::params![season_id, category], |row| {
            let track_name: String = row.get(1)?;
            let pista: String = row.get(2)?;
            let clima: String = row.get(3)?;
            Ok(FinishedRound {
                id: row.get(0)?,
                track: if track_name.trim().is_empty() {
                    pista
                } else {
                    track_name
                },
                is_wet: matches!(clima.as_str(), "Wet" | "HeavyRain"),
            })
        })
        .map_err(|e| format!("Falha ao ler etapas da categoria: {e}"))?;

    mapped
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Falha ao montar etapas da categoria: {e}"))
}

/// Todos os pilotos que pontuaram/correram nas etapas dadas, já com equipe e time.
fn load_driver_seasons(
    conn: &rusqlite::Connection,
    rounds: &[FinishedRound],
    player_id: &str,
) -> Result<Vec<DriverSeason>, String> {
    let index_by_race: HashMap<&str, usize> = rounds
        .iter()
        .enumerate()
        .map(|(index, round)| (round.id.as_str(), index))
        .collect();

    let placeholders = vec!["?"; rounds.len()].join(",");
    let sql = format!(
        "SELECT rr.race_id, rr.piloto_id, rr.equipe_id, rr.posicao_largada, rr.posicao_final,
                rr.dnf, rr.pontos, rr.fastest_lap,
                d.nome, d.nacionalidade, d.is_jogador, d.temporadas_na_categoria,
                t.nome, t.cor_primaria
         FROM race_results rr
         JOIN drivers d ON d.id = rr.piloto_id
         LEFT JOIN teams t ON t.id = rr.equipe_id
         WHERE rr.race_id IN ({placeholders})"
    );

    let params: Vec<&dyn rusqlite::ToSql> = rounds
        .iter()
        .map(|round| &round.id as &dyn rusqlite::ToSql)
        .collect();

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Falha ao preparar resultados da temporada: {e}"))?;

    let mut by_driver: HashMap<String, DriverSeason> = HashMap::new();
    let mut rows = stmt
        .query(params.as_slice())
        .map_err(|e| format!("Falha ao ler resultados da temporada: {e}"))?;

    while let Some(row) = rows
        .next()
        .map_err(|e| format!("Falha ao percorrer resultados da temporada: {e}"))?
    {
        let read = |row: &rusqlite::Row<'_>| -> rusqlite::Result<_> {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(3)?,
                row.get::<_, i32>(4)?,
                row.get::<_, i32>(5)? != 0,
                row.get::<_, f64>(6)?,
                row.get::<_, i32>(7)? != 0,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, i32>(10)? != 0,
                row.get::<_, i32>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
            ))
        };
        let (
            race_id,
            piloto_id,
            grid,
            finish,
            dnf,
            points,
            fastest_lap,
            nome,
            nacionalidade,
            is_jogador,
            temporadas_na_categoria,
            equipe,
            equipe_cor,
        ) = read(row).map_err(|e| format!("Falha ao mapear resultado da temporada: {e}"))?;

        let Some(&round_index) = index_by_race.get(race_id.as_str()) else {
            continue;
        };

        let entry = by_driver
            .entry(piloto_id.clone())
            .or_insert_with(|| DriverSeason {
                id: piloto_id.clone(),
                nome,
                nacionalidade,
                is_player: is_jogador || piloto_id == player_id,
                is_rookie: temporadas_na_categoria == 0,
                ..Default::default()
            });
        // A equipe exibida é a da etapa mais recente — quem trocou de time no meio
        // do ano aparece com a camisa em que terminou a temporada. Compara com o
        // MAIOR índice já visto, não com a última linha lida: o SQL devolve as
        // etapas em ordem arbitrária.
        let round_equipe = equipe.clone();
        let round_equipe_cor = equipe_cor.clone();
        if entry
            .rounds
            .iter()
            .map(|seen| seen.round_index)
            .max()
            .map(|latest| round_index >= latest)
            .unwrap_or(true)
        {
            entry.equipe = equipe;
            entry.equipe_cor = equipe_cor;
        }
        entry.rounds.push(DriverRound {
            round_index,
            grid,
            finish,
            dnf,
            points,
            fastest_lap,
            equipe: round_equipe,
            equipe_cor: round_equipe_cor,
        });
    }

    let mut drivers: Vec<DriverSeason> = by_driver.into_values().collect();
    for driver in &mut drivers {
        driver.rounds.sort_by_key(|round| round.round_index);
    }
    Ok(drivers)
}

/// Recordes da temporada. Cada linha só entra se alguém de fato pontuou nela.
fn build_records(drivers: &[DriverSeason], rounds: &[FinishedRound]) -> Vec<SeasonChampionRecord> {
    let mut records = Vec::new();
    let total_rounds = rounds.len();
    let leading = rounds_leading_by_driver(drivers, total_rounds);

    let mut push_max = |id: &str, value: i32, driver: Option<&DriverSeason>| {
        if let Some(driver) = driver {
            if value > 0 {
                records.push(SeasonChampionRecord {
                    id: id.to_string(),
                    who: driver.nome.clone(),
                    is_player: driver.is_player,
                    valor: value.to_string(),
                    sufixo: None,
                });
            }
        }
    };

    let leader = |metric: &dyn Fn(&DriverSeason) -> i32| -> (Option<&DriverSeason>, i32) {
        let best = drivers
            .iter()
            .max_by(|a, b| metric(a).cmp(&metric(b)).then_with(|| b.nome.cmp(&a.nome)));
        let value = best.map(|driver| metric(driver)).unwrap_or(0);
        (best, value)
    };

    let (driver, value) = leader(&|d: &DriverSeason| d.vitorias());
    push_max("vitorias", value, driver);
    let (driver, value) = leader(&|d: &DriverSeason| d.poles());
    push_max("poles", value, driver);
    let (driver, value) = leader(&|d: &DriverSeason| d.podios());
    push_max("podios", value, driver);
    let (driver, value) = leader(&|d: &DriverSeason| d.voltas_rapidas());
    push_max("voltas_rapidas", value, driver);
    let (driver, value) = leader(&|d: &DriverSeason| d.posicoes_ganhas());
    push_max("posicoes_ganhas", value, driver);
    let (driver, value) = leader(&|d: &DriverSeason| d.posicoes_perdidas());
    push_max("posicoes_perdidas", value, driver);
    let (driver, value) = leader(&|d: &DriverSeason| rain_wins(d, rounds));
    push_max("rei_da_chuva", value, driver);
    let (driver, value) = leader(&|d: &DriverSeason| d.abandonos());
    push_max("abandonos", value, driver);
    let (driver, value) = leader(&|d: &DriverSeason| d.chegadas());
    push_max("mais_chegadas", value, driver);
    let (driver, value) = leader(&|d: &DriverSeason| d.vitorias_da_pole());
    push_max("vitorias_da_pole", value, driver);
    let (driver, value) = leader(&|d: &DriverSeason| d.maior_sequencia_vitorias());
    push_max("sequencia_vitorias", value, driver);
    let (driver, value) =
        leader(&|d: &DriverSeason| leading.get(d.id.as_str()).copied().unwrap_or_default());
    push_max("etapas_na_lideranca", value, driver);

    // Maior recuperação numa única etapa — o número vem com a pista onde aconteceu.
    if let Some((driver, gained, round_index)) = drivers
        .iter()
        .filter_map(|driver| {
            driver
                .rounds
                .iter()
                .filter(|round| !round.dnf && round.grid > 0 && round.finish > 0)
                .map(|round| (driver, round.grid - round.finish, round.round_index))
                .max_by_key(|(_, gained, _)| *gained)
        })
        .max_by_key(|(_, gained, _)| *gained)
    {
        if gained > 0 {
            records.push(SeasonChampionRecord {
                id: "maior_recuperacao".to_string(),
                who: driver.nome.clone(),
                is_player: driver.is_player,
                valor: format!("+{gained}"),
                sufixo: rounds.get(round_index).map(|round| round.track.clone()),
            });
        }
    }

    // Maior colheita numa etapa só.
    if let Some((driver, points, round_index)) = drivers
        .iter()
        .filter_map(|driver| {
            driver
                .rounds
                .iter()
                .map(|round| (driver, round.points, round.round_index))
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    {
        if points > 0.0 {
            records.push(SeasonChampionRecord {
                id: "melhor_etapa".to_string(),
                who: driver.nome.clone(),
                is_player: driver.is_player,
                valor: format_points(points),
                sufixo: rounds.get(round_index).map(|round| round.track.clone()),
            });
        }
    }

    // Médias só fazem sentido com meia temporada de amostra — abaixo disso uma
    // corrida sortuda vira "recorde".
    let min_samples = rounds.len().div_ceil(2).max(1);
    if let Some((driver, media)) = best_average(drivers, min_samples, &|d| d.media_chegada()) {
        records.push(SeasonChampionRecord {
            id: "melhor_media_chegada".to_string(),
            who: driver.nome.clone(),
            is_player: driver.is_player,
            valor: format!("{media:.1}"),
            sufixo: None,
        });
    }
    if let Some((driver, media)) = best_average(drivers, min_samples, &|d| d.media_grid()) {
        records.push(SeasonChampionRecord {
            id: "melhor_media_grid".to_string(),
            who: driver.nome.clone(),
            is_player: driver.is_player,
            valor: format!("{media:.1}"),
            sufixo: None,
        });
    }

    records
}

/// Em quantas etapas cada piloto TERMINOU na liderança do campeonato. Empate no
/// acumulado conta para os dois — é raro e mentir seria pior que dividir.
fn rounds_leading_by_driver<'a>(
    drivers: &'a [DriverSeason],
    total_rounds: usize,
) -> HashMap<&'a str, i32> {
    let curves: Vec<(&str, Vec<f64>)> = drivers
        .iter()
        .map(|driver| (driver.id.as_str(), driver.cumulative(total_rounds)))
        .collect();

    let mut counts: HashMap<&str, i32> = HashMap::new();
    for round in 0..total_rounds {
        let best = curves
            .iter()
            .map(|(_, curve)| curve[round])
            .fold(f64::MIN, f64::max);
        // Antes de alguém pontuar não existe liderança para contar.
        if best <= 0.0 {
            continue;
        }
        for (id, curve) in &curves {
            if (curve[round] - best).abs() < f64::EPSILON {
                *counts.entry(id).or_insert(0) += 1;
            }
        }
    }
    counts
}

/// A etapa mais movimentada do ano: soma das posições trocadas entre grid e chegada
/// por todos que terminaram. Devolve (índice da etapa, total de posições).
fn wildest_round(drivers: &[DriverSeason], total_rounds: usize) -> Option<(usize, i32)> {
    let mut changes = vec![0i32; total_rounds];
    for driver in drivers {
        for round in &driver.rounds {
            if round.dnf || round.grid <= 0 || round.finish <= 0 {
                continue;
            }
            if let Some(slot) = changes.get_mut(round.round_index) {
                *slot += (round.grid - round.finish).abs();
            }
        }
    }
    changes
        .into_iter()
        .enumerate()
        .max_by_key(|(_, total)| *total)
        .filter(|(_, total)| *total > 0)
}

fn rain_wins(driver: &DriverSeason, rounds: &[FinishedRound]) -> i32 {
    driver
        .rounds
        .iter()
        .filter(|round| {
            !round.dnf
                && round.finish == 1
                && rounds
                    .get(round.round_index)
                    .map(|entry| entry.is_wet)
                    .unwrap_or(false)
        })
        .count() as i32
}

/// Melhor (menor) média entre quem tem amostra suficiente. `min_samples` é medido
/// sobre as etapas que ENTRARAM na média, não sobre as disputadas.
fn best_average<'a>(
    drivers: &'a [DriverSeason],
    min_samples: usize,
    metric: &dyn Fn(&DriverSeason) -> Option<(f64, usize)>,
) -> Option<(&'a DriverSeason, f64)> {
    drivers
        .iter()
        .filter_map(|driver| metric(driver).map(|value| (driver, value)))
        .filter(|(_, (_, samples))| *samples >= min_samples)
        .map(|(driver, (media, _))| (driver, media))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
}

/// Menções especiais. Cada prêmio é omitido quando a temporada não produziu o fato.
fn build_awards(
    drivers: &[DriverSeason],
    rounds: &[FinishedRound],
    champion_id: &str,
    constructors: &[SeasonChampionConstructor],
) -> Vec<SeasonChampionAward> {
    let mut awards = Vec::new();

    // Grand Chelem: pole, vitória e volta mais rápida na MESMA etapa. Havendo mais
    // de um, o do campeão tem preferência — é o fato que fecha a história do ano.
    let chelems: Vec<(&DriverSeason, usize)> = drivers
        .iter()
        .flat_map(|driver| {
            driver
                .rounds
                .iter()
                .filter(|round| {
                    !round.dnf && round.grid == 1 && round.finish == 1 && round.fastest_lap
                })
                .map(move |round| (driver, round.round_index))
        })
        .collect();
    if let Some((driver, round_index)) = chelems
        .iter()
        .find(|(driver, _)| driver.id == champion_id)
        .or_else(|| chelems.first())
    {
        let mut args = BTreeMap::new();
        args.insert(
            "track".to_string(),
            rounds
                .get(*round_index)
                .map(|round| round.track.clone())
                .unwrap_or_default(),
        );
        awards.push(SeasonChampionAward {
            id: "grand_chelem".to_string(),
            who: driver.nome.clone(),
            who_id: Some(driver.id.clone()),
            is_player: driver.is_player,
            args,
        });
    }

    // Zebra do ano: a vitória que veio de mais longe no grid. Diferente da "maior
    // recuperação" (que premia posições ganhas em qualquer chegada), aqui só conta
    // quem terminou em primeiro.
    if let Some((driver, grid, round_index)) = drivers
        .iter()
        .filter_map(|driver| {
            driver
                .rounds
                .iter()
                .filter(|round| !round.dnf && round.finish == 1 && round.grid >= MIN_UPSET_GRID)
                .map(|round| (driver, round.grid, round.round_index))
                .max_by_key(|(_, grid, _)| *grid)
        })
        .max_by_key(|(_, grid, _)| *grid)
    {
        let mut args = BTreeMap::new();
        args.insert("grid".to_string(), grid.to_string());
        args.insert(
            "track".to_string(),
            rounds
                .get(round_index)
                .map(|round| round.track.clone())
                .unwrap_or_default(),
        );
        awards.push(SeasonChampionAward {
            id: "zebra".to_string(),
            who: driver.nome.clone(),
            who_id: Some(driver.id.clone()),
            is_player: driver.is_player,
            args,
        });
    }

    // Duelo da temporada: a dupla que mais vezes cruzou a linha em posições vizinhas.
    if let Some((a, b, count)) = closest_duel(drivers) {
        let mut args = BTreeMap::new();
        args.insert("count".to_string(), count.to_string());
        awards.push(SeasonChampionAward {
            id: "duelo".to_string(),
            who: format!("{} × {}", a.nome, b.nome),
            who_id: None,
            is_player: a.is_player || b.is_player,
            args,
        });
    }

    // Revelação: o estreante na categoria melhor colocado (drivers já vem ordenado).
    if let Some((index, driver)) = drivers
        .iter()
        .enumerate()
        .find(|(_, driver)| driver.is_rookie)
    {
        let mut args = BTreeMap::new();
        args.insert("position".to_string(), (index + 1).to_string());
        // `count` (e não `podiums`) porque é dele que o i18next tira o plural da frase.
        args.insert("count".to_string(), driver.podios().to_string());
        awards.push(SeasonChampionAward {
            id: "revelacao".to_string(),
            who: driver.nome.clone(),
            who_id: Some(driver.id.clone()),
            is_player: driver.is_player,
            args,
        });
    }

    // Regularidade: quem entregou a MESMA corrida o ano inteiro — a menor distância
    // entre a melhor e a pior chegada. Vale mais aqui que a média: um vice-campeão
    // que oscila entre o pódio e o meio do grid perde para quem nunca saiu do 6º.
    if let Some((driver, best, worst, finishes)) =
        most_consistent(drivers, min_finishes_for_consistency(rounds.len()))
    {
        let mut args = BTreeMap::new();
        args.insert("best".to_string(), best.to_string());
        args.insert("worst".to_string(), worst.to_string());
        // `count` porque é dele que o i18next tira o plural da frase.
        args.insert("count".to_string(), finishes.to_string());
        awards.push(SeasonChampionAward {
            id: "regularidade".to_string(),
            who: driver.nome.clone(),
            who_id: Some(driver.id.clone()),
            is_player: driver.is_player,
            args,
        });
    }

    // Virada da temporada: a etapa em que o campeão assumiu a ponta para não mais
    // largá-la. Se ele liderou desde a primeira, não houve virada para contar.
    if let Some((driver, round_index)) = turning_point(drivers, champion_id, rounds.len()) {
        let mut args = BTreeMap::new();
        args.insert("round".to_string(), (round_index + 1).to_string());
        args.insert(
            "track".to_string(),
            rounds
                .get(round_index)
                .map(|round| round.track.clone())
                .unwrap_or_default(),
        );
        awards.push(SeasonChampionAward {
            id: "virada".to_string(),
            who: driver.nome.clone(),
            who_id: Some(driver.id.clone()),
            is_player: driver.is_player,
            args,
        });
    }

    // Etapa do ano: a corrida em que o grid mais se embaralhou entre a largada e a
    // bandeirada. Aqui o "quem" é uma PISTA, não um piloto — é o prêmio da temporada
    // que não fala de gente.
    if let Some((round_index, changes)) = wildest_round(drivers, rounds.len()) {
        let mut args = BTreeMap::new();
        args.insert("count".to_string(), changes.to_string());
        args.insert("round".to_string(), (round_index + 1).to_string());
        awards.push(SeasonChampionAward {
            id: "etapa_do_ano".to_string(),
            who: rounds
                .get(round_index)
                .map(|round| round.track.clone())
                .unwrap_or_default(),
            who_id: None,
            is_player: false,
            args,
        });
    }

    // Equipe do ano: a ponta do campeonato de construtores, que a tela mostra logo
    // abaixo. O prêmio e o quadro saem da MESMA conta — não há como discordarem.
    if let Some(campea) = constructors.iter().find(|team| team.pontos > 0.0) {
        let mut args = BTreeMap::new();
        args.insert("count".to_string(), format_points(campea.pontos));
        awards.push(SeasonChampionAward {
            id: "equipe_do_ano".to_string(),
            who: campea.nome.clone(),
            who_id: None,
            is_player: campea.is_player_team,
            args,
        });
    }

    awards
}

/// Chegadas mínimas para alguém disputar a Regularidade: metade da temporada, nunca
/// menos de três. Amplitude zero em duas corridas é acaso, não constância.
fn min_finishes_for_consistency(total_rounds: usize) -> usize {
    total_rounds.div_ceil(2).max(3)
}

/// O piloto com a menor distância entre a melhor e a pior chegada. Devolve
/// (piloto, melhor, pior, chegadas). Empate na amplitude vai para quem chegou mais
/// vezes, depois para quem chegou mais na frente.
fn most_consistent(
    drivers: &[DriverSeason],
    min_finishes: usize,
) -> Option<(&DriverSeason, i32, i32, usize)> {
    drivers
        .iter()
        .filter_map(|driver| {
            let finishes: Vec<i32> = driver
                .rounds
                .iter()
                .filter(|round| !round.dnf && round.finish > 0)
                .map(|round| round.finish)
                .collect();
            if finishes.len() < min_finishes {
                return None;
            }
            let best = *finishes.iter().min()?;
            let worst = *finishes.iter().max()?;
            Some((driver, best, worst, finishes.len()))
        })
        .min_by(|a, b| {
            (a.2 - a.1)
                .cmp(&(b.2 - b.1))
                .then_with(|| b.3.cmp(&a.3))
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.0.nome.cmp(&b.0.nome))
        })
}

/// O campeonato de construtores da categoria, do mesmo caldo do de pilotos.
///
/// A soma é POR ETAPA, com a camisa daquele dia: quem se transferiu no meio do ano
/// deixa os pontos onde os fez. E vem daqui, e não de `get_teams_standings_in_base_dir`,
/// por dois motivos: aquela lê `teams.stats_pontos`, que é da temporada ATIVA e não
/// sobrevive à virada do ano (o pop-up abre campeonatos passados), e porque somando
/// os mesmos `race_results` do resto da tela os dois quadros fecham entre si.
fn build_constructors(
    drivers: &[DriverSeason],
    total_rounds: usize,
) -> Vec<SeasonChampionConstructor> {
    // BTreeMap para a ordem de desempate ser estável entre leituras.
    let mut by_team: BTreeMap<&str, TeamTally> = BTreeMap::new();
    for driver in drivers {
        for round in &driver.rounds {
            let Some(equipe) = round
                .equipe
                .as_deref()
                .map(str::trim)
                .filter(|nome| !nome.is_empty())
            else {
                continue;
            };
            let entry = by_team.entry(equipe).or_default();
            let venceu = !round.dnf && round.finish == 1;
            entry.pontos += round.points;
            entry.vitorias += i32::from(venceu);
            entry.podios += i32::from(!round.dnf && round.finish >= 1 && round.finish <= 3);
            entry.poles += i32::from(round.grid == 1);
            entry.voltas_rapidas += i32::from(round.fastest_lap);
            if entry.cor.is_none() {
                entry.cor = round.equipe_cor.clone();
            }
            entry.is_player_team |= driver.is_player;
            if entry.por_etapa.len() < total_rounds {
                entry.por_etapa.resize(total_rounds, 0.0);
            }
            if let Some(slot) = entry.por_etapa.get_mut(round.round_index) {
                *slot += round.points;
            }
            let piloto = entry.pilotos.entry(driver.id.as_str()).or_insert_with(|| {
                SeasonChampionConstructorDriver {
                    id: driver.id.clone(),
                    nome: driver.nome.clone(),
                    pontos: 0.0,
                    vitorias: 0,
                    is_player: driver.is_player,
                }
            });
            piloto.pontos += round.points;
            piloto.vitorias += i32::from(venceu);
        }
    }

    let mut standings: Vec<SeasonChampionConstructor> = by_team
        .into_iter()
        .map(|(nome, tally)| SeasonChampionConstructor {
            nome: nome.to_string(),
            cor: tally.cor,
            posicao: 0,
            pontos: tally.pontos,
            vitorias: tally.vitorias,
            podios: tally.podios,
            poles: tally.poles,
            voltas_rapidas: tally.voltas_rapidas,
            cumulative: acumular(&tally.por_etapa, total_rounds),
            pilotos: ordenar_pilotos(tally.pilotos),
            is_player_team: tally.is_player_team,
        })
        .collect();
    standings.sort_by(|a, b| {
        b.pontos
            .partial_cmp(&a.pontos)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.vitorias.cmp(&a.vitorias))
            .then_with(|| a.nome.cmp(&b.nome))
    });
    for (index, team) in standings.iter_mut().enumerate() {
        team.posicao = index as i32 + 1;
    }
    standings
}

/// O caldo de uma equipe enquanto a temporada é varrida etapa a etapa.
#[derive(Default)]
struct TeamTally<'a> {
    pontos: f64,
    vitorias: i32,
    podios: i32,
    poles: i32,
    voltas_rapidas: i32,
    cor: Option<String>,
    is_player_team: bool,
    /// Pontos por rodada, na ordem do calendário — vira a curva acumulada.
    por_etapa: Vec<f64>,
    /// Por ID, e não por nome: dois homônimos no grid não podem virar uma linha só.
    pilotos: BTreeMap<&'a str, SeasonChampionConstructorDriver>,
}

fn acumular(por_etapa: &[f64], total_rounds: usize) -> Vec<f64> {
    let mut running = 0.0;
    (0..total_rounds)
        .map(|index| {
            running += por_etapa.get(index).copied().unwrap_or(0.0);
            running
        })
        .collect()
}

fn ordenar_pilotos(
    pilotos: BTreeMap<&str, SeasonChampionConstructorDriver>,
) -> Vec<SeasonChampionConstructorDriver> {
    let mut lista: Vec<SeasonChampionConstructorDriver> = pilotos.into_values().collect();
    lista.sort_by(|a, b| {
        b.pontos
            .partial_cmp(&a.pontos)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.vitorias.cmp(&a.vitorias))
            .then_with(|| a.nome.cmp(&b.nome))
    });
    lista
}

/// A dupla que mais terminou colada (posições vizinhas na chegada).
fn closest_duel(drivers: &[DriverSeason]) -> Option<(&DriverSeason, &DriverSeason, i32)> {
    let mut best: Option<(&DriverSeason, &DriverSeason, i32)> = None;
    for (i, a) in drivers.iter().enumerate() {
        for b in drivers.iter().skip(i + 1) {
            let mut count = 0;
            for round_a in a
                .rounds
                .iter()
                .filter(|round| !round.dnf && round.finish > 0)
            {
                let neighbour = b.rounds.iter().any(|round_b| {
                    !round_b.dnf
                        && round_b.finish > 0
                        && round_b.round_index == round_a.round_index
                        && (round_b.finish - round_a.finish).abs() == 1
                });
                if neighbour {
                    count += 1;
                }
            }
            if count >= MIN_DUEL_ENCOUNTERS && best.map(|(_, _, best)| count > best).unwrap_or(true)
            {
                best = Some((a, b, count));
            }
        }
    }
    best
}

/// Etapa a partir da qual o campeão nunca mais perdeu a liderança do campeonato.
/// Devolve `None` quando ele liderou desde a primeira — aí não houve virada.
fn turning_point<'a>(
    drivers: &'a [DriverSeason],
    champion_id: &str,
    total_rounds: usize,
) -> Option<(&'a DriverSeason, usize)> {
    let champion = drivers.iter().find(|driver| driver.id == champion_id)?;
    let champion_curve = champion.cumulative(total_rounds);
    let rivals: Vec<Vec<f64>> = drivers
        .iter()
        .filter(|driver| driver.id != champion_id)
        .map(|driver| driver.cumulative(total_rounds))
        .collect();

    let leading_at = |round: usize| -> bool {
        rivals
            .iter()
            .all(|curve| champion_curve[round] >= curve[round])
    };

    // Anda de trás para frente: a virada é a primeira etapa da última sequência
    // ininterrupta de liderança.
    let mut first_of_streak = None;
    for round in (0..total_rounds).rev() {
        if leading_at(round) {
            first_of_streak = Some(round);
        } else {
            break;
        }
    }

    match first_of_streak {
        Some(0) | None => None,
        Some(round) => Some((champion, round)),
    }
}

fn format_points(points: f64) -> String {
    if (points - points.round()).abs() < f64::EPSILON {
        format!("{}", points.round() as i64)
    } else {
        format!("{points:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(index: usize, grid: i32, finish: i32, points: f64) -> DriverRound {
        DriverRound {
            round_index: index,
            grid,
            finish,
            dnf: false,
            points,
            fastest_lap: false,
            equipe: None,
            equipe_cor: None,
        }
    }

    fn round_por(index: usize, finish: i32, points: f64, equipe: &str) -> DriverRound {
        DriverRound {
            equipe: Some(equipe.to_string()),
            equipe_cor: Some("#F4C752".to_string()),
            ..round(index, finish, finish, points)
        }
    }

    fn driver(nome: &str, rounds: Vec<DriverRound>) -> DriverSeason {
        DriverSeason {
            id: nome.to_string(),
            nome: nome.to_string(),
            rounds,
            ..Default::default()
        }
    }

    fn track(index: usize, wet: bool) -> FinishedRound {
        FinishedRound {
            id: format!("R{index}"),
            track: format!("Pista {index}"),
            is_wet: wet,
        }
    }

    #[test]
    fn cumulative_repete_o_acumulado_na_etapa_que_o_piloto_faltou() {
        let piloto = driver("A", vec![round(0, 1, 1, 25.0), round(2, 1, 2, 18.0)]);

        // A etapa 2 (índice 1) não tem resultado: a curva fica plana em vez de cair.
        assert_eq!(piloto.cumulative(3), vec![25.0, 25.0, 43.0]);
    }

    #[test]
    fn turning_point_ignora_quem_liderou_desde_a_primeira_etapa() {
        let campeao = driver("A", vec![round(0, 1, 1, 25.0), round(1, 1, 1, 25.0)]);
        let vice = driver("B", vec![round(0, 2, 2, 18.0), round(1, 2, 2, 18.0)]);

        assert!(turning_point(&[campeao, vice], "A", 2).is_none());
    }

    #[test]
    fn turning_point_aponta_a_etapa_da_ultrapassagem_definitiva() {
        // B lidera a primeira etapa; A assume na segunda e não larga mais.
        let campeao = driver(
            "A",
            vec![
                round(0, 2, 2, 18.0),
                round(1, 1, 1, 25.0),
                round(2, 1, 1, 25.0),
            ],
        );
        let vice = driver(
            "B",
            vec![
                round(0, 1, 1, 25.0),
                round(1, 2, 2, 18.0),
                round(2, 3, 3, 15.0),
            ],
        );

        let grid = [campeao, vice];
        let (driver, round_index) = turning_point(&grid, "A", 3).expect("houve virada");
        assert_eq!(driver.nome, "A");
        assert_eq!(round_index, 1);
    }

    #[test]
    fn closest_duel_exige_repeticao_para_virar_historia() {
        let a = driver("A", vec![round(0, 1, 1, 25.0), round(1, 1, 1, 25.0)]);
        let b = driver("B", vec![round(0, 2, 2, 18.0), round(1, 2, 2, 18.0)]);

        // Duas chegadas coladas não bastam (MIN_DUEL_ENCOUNTERS = 3).
        assert!(closest_duel(&[a, b]).is_none());

        let a = driver(
            "A",
            vec![
                round(0, 1, 1, 25.0),
                round(1, 1, 1, 25.0),
                round(2, 1, 1, 25.0),
            ],
        );
        let b = driver(
            "B",
            vec![
                round(0, 2, 2, 18.0),
                round(1, 2, 2, 18.0),
                round(2, 2, 2, 18.0),
            ],
        );
        let grid = [a, b];
        let (first, second, count) = closest_duel(&grid).expect("duelo da temporada");
        assert_eq!((first.nome.as_str(), second.nome.as_str()), ("A", "B"));
        assert_eq!(count, 3);
    }

    #[test]
    fn build_records_omite_recorde_que_ninguem_pontuou() {
        let rounds = vec![track(0, false), track(1, false)];
        // Ninguém venceu na chuva (nenhuma etapa molhada) nem abandonou.
        let drivers = vec![
            driver("A", vec![round(0, 1, 1, 25.0), round(1, 1, 1, 25.0)]),
            driver("B", vec![round(0, 2, 2, 18.0), round(1, 2, 2, 18.0)]),
        ];

        let records = build_records(&drivers, &rounds);
        let ids: Vec<&str> = records.iter().map(|record| record.id.as_str()).collect();

        assert!(ids.contains(&"vitorias"));
        assert!(!ids.contains(&"rei_da_chuva"));
        assert!(!ids.contains(&"abandonos"));
        // Ninguém ganhou posição: largaram e chegaram na mesma ordem.
        assert!(!ids.contains(&"posicoes_ganhas"));
    }

    #[test]
    fn sequencia_de_vitorias_exige_etapas_consecutivas() {
        // Venceu a 1ª, a 3ª e a 4ª: a maior corrente é 2, não 3.
        let piloto = driver(
            "A",
            vec![
                round(0, 1, 1, 25.0),
                round(2, 1, 1, 25.0),
                round(3, 1, 1, 25.0),
            ],
        );

        assert_eq!(piloto.maior_sequencia_vitorias(), 2);
    }

    #[test]
    fn etapas_na_lideranca_conta_a_ponta_do_acumulado() {
        // B abre na frente; A vira na 2ª e nunca mais perde. 1 etapa para B, 2 para A.
        let a = driver(
            "A",
            vec![
                round(0, 2, 2, 18.0),
                round(1, 1, 1, 25.0),
                round(2, 1, 1, 25.0),
            ],
        );
        let b = driver(
            "B",
            vec![
                round(0, 1, 1, 25.0),
                round(1, 2, 2, 15.0),
                round(2, 3, 3, 12.0),
            ],
        );

        // Acumulado A: 18 / 43 / 68 · B: 25 / 40 / 52 — sem empate para dividir a ponta.
        let grid = [a, b];
        let leading = rounds_leading_by_driver(&grid, 3);
        assert_eq!(leading.get("A").copied(), Some(2));
        assert_eq!(leading.get("B").copied(), Some(1));
    }

    #[test]
    fn etapa_do_ano_aponta_a_corrida_mais_embaralhada() {
        // Etapa 0 troca 2 posições no total; etapa 1 troca 12.
        let a = driver("A", vec![round(0, 1, 2, 18.0), round(1, 10, 1, 25.0)]);
        let b = driver("B", vec![round(0, 2, 1, 25.0), round(1, 1, 3, 15.0)]);

        assert_eq!(wildest_round(&[a, b], 2), Some((1, 11)));
    }

    #[test]
    fn melhor_media_ignora_quem_abandonou_metade_da_temporada() {
        let rounds = vec![
            track(0, false),
            track(1, false),
            track(2, false),
            track(3, false),
        ];
        let mut abandonador = driver("Sortudo", vec![round(0, 1, 1, 25.0)]);
        // Venceu a única que terminou e abandonou as outras três: média 1.0 sem lastro.
        for index in 1..4 {
            abandonador.rounds.push(DriverRound {
                round_index: index,
                grid: 1,
                finish: 20,
                dnf: true,
                points: 0.0,
                fastest_lap: false,
                equipe: None,
                equipe_cor: None,
            });
        }
        let regular = driver(
            "Regular",
            vec![
                round(0, 2, 2, 18.0),
                round(1, 2, 2, 18.0),
                round(2, 3, 3, 15.0),
                round(3, 2, 2, 18.0),
            ],
        );

        let media = build_records(&[abandonador, regular], &rounds)
            .into_iter()
            .find(|record| record.id == "melhor_media_chegada")
            .expect("melhor media de chegada");

        assert_eq!(media.who, "Regular");
    }

    #[test]
    fn zebra_do_ano_ignora_a_vitoria_que_saiu_da_frente() {
        let rounds = vec![track(0, false), track(1, false)];
        // Vitória da 2ª posição (dentro do limiar) e vitória do 9º: só a segunda conta.
        let drivers = vec![
            driver("A", vec![round(0, 2, 1, 25.0), round(1, 9, 1, 25.0)]),
            driver("B", vec![round(0, 1, 2, 18.0), round(1, 1, 2, 18.0)]),
        ];

        let zebra = build_awards(
            &drivers,
            &rounds,
            "A",
            &build_constructors(&drivers, rounds.len()),
        )
        .into_iter()
        .find(|award| award.id == "zebra")
        .expect("zebra do ano");
        assert_eq!(zebra.who, "A");
        assert_eq!(zebra.args.get("grid").map(String::as_str), Some("9"));
        assert_eq!(zebra.args.get("track").map(String::as_str), Some("Pista 1"));

        // Um ano em que ninguém venceu de trás simplesmente não tem zebra.
        let previsivel = vec![
            driver("A", vec![round(0, 1, 1, 25.0), round(1, 2, 1, 25.0)]),
            driver("B", vec![round(0, 2, 2, 18.0), round(1, 1, 2, 18.0)]),
        ];
        assert!(build_awards(
            &previsivel,
            &rounds,
            "A",
            &build_constructors(&previsivel, rounds.len())
        )
        .iter()
        .all(|award| award.id != "zebra"));
    }

    #[test]
    fn regularidade_prefere_a_amplitude_menor_e_nao_a_media_melhor() {
        // O vice oscila do pódio ao meio do grid; o regular nunca sai do 5º/6º.
        let oscilante = driver(
            "Oscilante",
            vec![
                round(0, 1, 1, 25.0),
                round(1, 1, 12, 0.0),
                round(2, 1, 2, 18.0),
                round(3, 1, 11, 0.0),
            ],
        );
        let constante = driver(
            "Constante",
            vec![
                round(0, 5, 5, 10.0),
                round(1, 6, 6, 8.0),
                round(2, 5, 5, 10.0),
                round(3, 6, 6, 8.0),
            ],
        );

        let grid = [oscilante, constante];
        let (driver, best, worst, finishes) = most_consistent(&grid, 3).expect("regularidade");
        assert_eq!(driver.nome, "Constante");
        assert_eq!((best, worst, finishes), (5, 6, 4));

        // Uma temporada curta demais não elege ninguém: amplitude zero em duas
        // chegadas é acaso, e o prêmio some em vez de premiar o acaso.
        assert_eq!(min_finishes_for_consistency(2), 3);
        assert!(most_consistent(&grid, min_finishes_for_consistency(9)).is_none());
    }

    #[test]
    fn construtores_classificam_por_pontos_com_vitorias_no_desempate() {
        let a = driver(
            "A",
            vec![
                round_por(0, 1, 25.0, "Alfa"),
                round_por(1, 4, 12.0, "Alfa"),
                round_por(2, 1, 25.0, "Alfa"),
            ],
        );
        let b = driver(
            "B",
            vec![
                round_por(0, 2, 18.0, "Beta"),
                round_por(1, 1, 25.0, "Beta"),
                round_por(2, 3, 19.0, "Beta"),
            ],
        );

        let tabela = build_constructors(&[a, b], 3);
        // Empate em 62 pontos: as duas vitórias da Alfa passam à frente da única da Beta.
        assert_eq!(tabela[0].pontos, tabela[1].pontos);
        assert_eq!(
            (
                tabela[0].nome.as_str(),
                tabela[0].posicao,
                tabela[0].vitorias
            ),
            ("Alfa", 1, 2)
        );
        assert_eq!(
            (
                tabela[1].nome.as_str(),
                tabela[1].posicao,
                tabela[1].vitorias
            ),
            ("Beta", 2, 1)
        );
    }

    #[test]
    fn equipe_do_ano_deixa_os_pontos_na_camisa_de_cada_etapa() {
        // O piloto A trocou de equipe no meio do ano: os 25 da 1ª etapa ficam com a
        // Alfa, e não migram para a Beta junto com ele.
        let mut trocou = driver(
            "A",
            vec![
                round_por(0, 1, 25.0, "Alfa"),
                round_por(1, 1, 25.0, "Beta"),
                round_por(2, 1, 25.0, "Beta"),
            ],
        );
        trocou.equipe = Some("Beta".to_string());
        let alfa = driver(
            "B",
            vec![
                round_por(0, 2, 18.0, "Alfa"),
                round_por(1, 2, 18.0, "Alfa"),
                round_por(2, 2, 18.0, "Alfa"),
            ],
        );

        let tabela = build_constructors(&[trocou, alfa], 3);
        // Alfa: 25 + 18*3 = 79. Beta: 25 + 25 = 50.
        assert_eq!((tabela[0].nome.as_str(), tabela[0].pontos), ("Alfa", 79.0));
        assert_eq!((tabela[1].nome.as_str(), tabela[1].pontos), ("Beta", 50.0));
        assert!(!tabela[0].is_player_team);
    }

    #[test]
    fn construtores_abrem_a_curva_e_a_contribuicao_de_cada_piloto() {
        // A trocou de equipe depois da 1ª etapa; B ficou na Alfa o ano todo.
        let trocou = driver(
            "A",
            vec![
                round_por(0, 1, 25.0, "Alfa"),
                round_por(1, 1, 25.0, "Beta"),
                round_por(2, 1, 25.0, "Beta"),
            ],
        );
        let alfa = driver(
            "B",
            vec![
                round_por(0, 2, 18.0, "Alfa"),
                round_por(1, 2, 18.0, "Alfa"),
                round_por(2, 2, 18.0, "Alfa"),
            ],
        );

        let tabela = build_constructors(&[trocou, alfa], 3);
        let beta = tabela.iter().find(|t| t.nome == "Beta").expect("Beta");
        // A curva acumula só o que foi feito COM a camisa: a Beta parte do zero na
        // etapa em que ainda não tinha o piloto.
        assert_eq!(beta.cumulative, vec![0.0, 25.0, 50.0]);
        assert_eq!(beta.pilotos.len(), 1);
        assert_eq!(
            (beta.pilotos[0].nome.as_str(), beta.pilotos[0].pontos),
            ("A", 50.0)
        );

        let alfa_row = tabela.iter().find(|t| t.nome == "Alfa").expect("Alfa");
        assert_eq!(alfa_row.cumulative, vec![43.0, 61.0, 79.0]);
        // Ordenado por pontos: B (54) na frente do que A deixou lá (25).
        let contribuicao: Vec<(&str, f64)> = alfa_row
            .pilotos
            .iter()
            .map(|p| (p.nome.as_str(), p.pontos))
            .collect();
        assert_eq!(contribuicao, vec![("B", 54.0), ("A", 25.0)]);
        // Pódios e vitórias também somam por camisa: a Alfa tem 4 etapas somadas
        // (a 1ª do A mais as 3 do B), todas dentro do pódio, e só uma vitória.
        assert_eq!((alfa_row.vitorias, alfa_row.podios), (1, 4));
    }

    #[test]
    fn build_records_leva_a_pista_no_sufixo_da_recuperacao() {
        let rounds = vec![track(0, false), track(1, false)];
        let drivers = vec![
            driver("A", vec![round(0, 8, 2, 18.0), round(1, 1, 1, 25.0)]),
            driver("B", vec![round(0, 1, 1, 25.0), round(1, 2, 2, 18.0)]),
        ];

        let recuperacao = build_records(&drivers, &rounds)
            .into_iter()
            .find(|record| record.id == "maior_recuperacao")
            .expect("maior recuperacao");

        assert_eq!(recuperacao.who, "A");
        assert_eq!(recuperacao.valor, "+6");
        assert_eq!(recuperacao.sufixo.as_deref(), Some("Pista 0"));
    }
}
