#![allow(dead_code)]

use rusqlite::{Connection, OptionalExtension};

use crate::db::connection::DbError;

/// Entrada de standings para uma categoria/temporada.
#[derive(Debug, Clone)]
pub struct StandingEntry {
    pub pilot_id: String,
    pub pilot_name: String,
    pub points: f64,
    pub position: i32,
}

/// Retorna a última vitória na carreira do piloto (qualquer categoria/temporada).
/// Retorna (season_num, round) ou None se nunca venceu.
pub fn get_last_career_win(
    conn: &Connection,
    pilot_id: &str,
) -> Result<Option<(i32, i32)>, DbError> {
    let result: Result<(i32, i32), _> = conn.query_row(
        "SELECT s.numero, c.rodada
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN seasons s ON c.temporada_id = s.id
         WHERE r.piloto_id = ?1 AND r.posicao_final = 1
         ORDER BY s.numero DESC, c.rodada DESC
         LIMIT 1",
        rusqlite::params![pilot_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );

    match result {
        Ok(pair) => Ok(Some(pair)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DbError::Sqlite(e)),
    }
}

/// Retorna o número de vitórias do piloto com uma equipe específica (histórico completo).
pub fn get_wins_with_team(
    conn: &Connection,
    pilot_id: &str,
    team_id: &str,
) -> Result<i32, DbError> {
    let count: i32 = conn.query_row(
        "SELECT COUNT(*)
         FROM race_results
         WHERE piloto_id = ?1 AND equipe_id = ?2 AND posicao_final = 1",
        rusqlite::params![pilot_id, team_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

/// Retorna os standings completos de uma categoria em uma temporada.
/// Ordenado por pontos (desc), posição calculada sequencialmente.
pub fn get_category_standings(
    conn: &Connection,
    temporada_id: &str,
    categoria: &str,
) -> Result<Vec<StandingEntry>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT
            r.piloto_id,
            d.nome,
            SUM(r.pontos) as total_points,
            SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) as total_wins
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN drivers d ON r.piloto_id = d.id
         WHERE c.temporada_id = ?1 AND c.categoria = ?2
         GROUP BY r.piloto_id
         ORDER BY total_points DESC, total_wins DESC, d.nome ASC",
    )?;

    let mut standings = Vec::new();
    let mut rows = stmt.query(rusqlite::params![temporada_id, categoria])?;
    let mut position = 1;

    while let Some(row) = rows.next()? {
        standings.push(StandingEntry {
            pilot_id: row.get(0)?,
            pilot_name: row.get(1)?,
            points: row.get(2)?,
            position,
        });
        position += 1;
    }

    Ok(standings)
}

/// Entrada de standings de EQUIPES para uma categoria/temporada.
#[derive(Debug, Clone)]
pub struct TeamStandingEntry {
    pub team_id: String,
    pub team_name: String,
    pub points: f64,
    pub position: i32,
}

/// Retorna os standings de equipes (soma dos pontos de todos os pilotos da equipe)
/// de uma categoria na temporada. Ordenado por pontos (desc); posição sequencial.
/// Linhas sem `equipe_id` correspondente em `teams` são ignoradas pelo JOIN.
pub fn get_team_standings(
    conn: &Connection,
    temporada_id: &str,
    categoria: &str,
) -> Result<Vec<TeamStandingEntry>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT
            r.equipe_id,
            t.nome,
            SUM(r.pontos) as total_points
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN teams t ON r.equipe_id = t.id
         WHERE c.temporada_id = ?1 AND c.categoria = ?2
         GROUP BY r.equipe_id
         ORDER BY total_points DESC, t.nome ASC",
    )?;

    let mut standings = Vec::new();
    let mut rows = stmt.query(rusqlite::params![temporada_id, categoria])?;
    let mut position = 1;

    while let Some(row) = rows.next()? {
        standings.push(TeamStandingEntry {
            team_id: row.get(0)?,
            team_name: row.get(1)?,
            points: row.get(2)?,
            position,
        });
        position += 1;
    }

    Ok(standings)
}

/// Estatística de carreira de um piloto NA CATEGORIA (todas as temporadas no DB).
#[derive(Debug, Clone)]
pub struct DriverCategoryCareer {
    pub wins: i32,
    pub podiums: i32,
    pub starts: i32,
}

/// Vitórias, pódios e largadas de um piloto na categoria, somando todas as temporadas.
/// Inclui a corrida atual se ela já estiver persistida em `race_results`.
pub fn get_driver_category_career(
    conn: &Connection,
    pilot_id: &str,
    categoria: &str,
) -> Result<DriverCategoryCareer, DbError> {
    let row = conn.query_row(
        "SELECT
            COALESCE(SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN r.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END), 0),
            COUNT(*)
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE r.piloto_id = ?1 AND c.categoria = ?2",
        rusqlite::params![pilot_id, categoria],
        |r| {
            Ok(DriverCategoryCareer {
                wins: r.get(0)?,
                podiums: r.get(1)?,
                starts: r.get(2)?,
            })
        },
    )?;
    Ok(row)
}

/// Recordista (líder histórico) de uma métrica na categoria.
#[derive(Debug, Clone)]
pub struct CategoryRecord {
    pub pilot_id: String,
    pub pilot_name: String,
    pub value: i32,
}

/// Líderes históricos da categoria em vitórias, pódios e largadas (todas as temporadas).
#[derive(Debug, Clone, Default)]
pub struct CategoryRecords {
    pub most_wins: Option<CategoryRecord>,
    pub most_podiums: Option<CategoryRecord>,
    pub most_starts: Option<CategoryRecord>,
    /// 2º maior número de vitórias (valor). Usado para detectar quando o recorde de
    /// vitórias acabou de ser SUPERADO (líder == 2º+1).
    pub second_most_wins: Option<i32>,
}

/// Calcula, numa passada, quem lidera a categoria em vitórias, pódios e largadas.
pub fn get_category_records(
    conn: &Connection,
    categoria: &str,
) -> Result<CategoryRecords, DbError> {
    let mut stmt = conn.prepare(
        "SELECT r.piloto_id, d.nome,
            SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS wins,
            SUM(CASE WHEN r.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END) AS podiums,
            COUNT(*) AS starts
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN drivers d ON r.piloto_id = d.id
         WHERE c.categoria = ?1
         GROUP BY r.piloto_id",
    )?;

    let mut records = CategoryRecords::default();
    // Top-2 de vitórias (valores) para detectar recorde recém-superado.
    let mut top1_wins = i32::MIN;
    let mut top2_wins = i32::MIN;
    let mut rows = stmt.query(rusqlite::params![categoria])?;
    while let Some(row) = rows.next()? {
        let pilot_id: String = row.get(0)?;
        let pilot_name: String = row.get(1)?;
        let wins: i32 = row.get(2)?;
        let podiums: i32 = row.get(3)?;
        let starts: i32 = row.get(4)?;
        // Atualiza o recordista de uma métrica se este piloto a supera (>0).
        let beats = |cur: &Option<CategoryRecord>, val: i32| {
            val > 0 && cur.as_ref().map_or(true, |c| val > c.value)
        };
        if beats(&records.most_wins, wins) {
            records.most_wins = Some(CategoryRecord {
                pilot_id: pilot_id.clone(),
                pilot_name: pilot_name.clone(),
                value: wins,
            });
        }
        if beats(&records.most_podiums, podiums) {
            records.most_podiums = Some(CategoryRecord {
                pilot_id: pilot_id.clone(),
                pilot_name: pilot_name.clone(),
                value: podiums,
            });
        }
        if beats(&records.most_starts, starts) {
            records.most_starts = Some(CategoryRecord {
                pilot_id,
                pilot_name,
                value: starts,
            });
        }
        // Top-2 de vitórias (mantém duplicatas: empate no topo → top2 == top1).
        if wins > top1_wins {
            top2_wins = top1_wins;
            top1_wins = wins;
        } else if wins > top2_wins {
            top2_wins = wins;
        }
    }
    if top2_wins > i32::MIN {
        records.second_most_wins = Some(top2_wins);
    }
    Ok(records)
}

/// Piloto que mais venceu por uma equipe na categoria (todas as temporadas). `None`
/// se a equipe nunca venceu na categoria.
pub fn get_team_top_winner_in_category(
    conn: &Connection,
    team_id: &str,
    categoria: &str,
) -> Result<Option<CategoryRecord>, DbError> {
    let res = conn
        .query_row(
            "SELECT r.piloto_id, d.nome,
                SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS wins
             FROM race_results r
             JOIN calendar c ON r.race_id = c.id
             JOIN drivers d ON r.piloto_id = d.id
             WHERE r.equipe_id = ?1 AND c.categoria = ?2
             GROUP BY r.piloto_id
             HAVING wins > 0
             ORDER BY wins DESC, d.nome ASC
             LIMIT 1",
            rusqlite::params![team_id, categoria],
            |r| {
                Ok(CategoryRecord {
                    pilot_id: r.get(0)?,
                    pilot_name: r.get(1)?,
                    value: r.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(res)
}

/// Vitórias na categoria de cada piloto que AINDA corre nela (ativo + categoria_atual).
/// Serve para comparar o vencedor com rivais que ainda estão no grid. Só `wins > 0`.
pub fn get_active_category_win_counts(
    conn: &Connection,
    categoria: &str,
) -> Result<Vec<CategoryRecord>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT r.piloto_id, d.nome,
            SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS wins
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN drivers d ON r.piloto_id = d.id
         WHERE c.categoria = ?1
           AND d.status != 'Aposentado'
           AND d.categoria_atual = ?1
         GROUP BY r.piloto_id
         HAVING wins > 0",
    )?;
    let mut out = Vec::new();
    let mut rows = stmt.query(rusqlite::params![categoria])?;
    while let Some(row) = rows.next()? {
        out.push(CategoryRecord {
            pilot_id: row.get(0)?,
            pilot_name: row.get(1)?,
            value: row.get(2)?,
        });
    }
    Ok(out)
}

/// Ano (calendário) em que a marca de `target_wins` vitórias acumuladas foi atingida
/// pela PRIMEIRA vez na categoria, por qualquer piloto. Usado para dizer há quanto
/// tempo um recorde resistia. `None` se ninguém jamais chegou a essa marca.
pub fn first_year_reaching_wins(
    conn: &Connection,
    categoria: &str,
    target_wins: i32,
) -> Result<Option<i32>, DbError> {
    if target_wins <= 0 {
        return Ok(None);
    }
    // MIN(ano) sempre retorna uma linha (NULL se ninguém atingiu a marca).
    let year: Option<i32> = conn.query_row(
        "WITH wins AS (
            SELECT s.ano AS ano,
                   ROW_NUMBER() OVER (
                       PARTITION BY r.piloto_id
                       ORDER BY s.numero ASC, c.rodada ASC
                   ) AS rn
            FROM race_results r
            JOIN calendar c ON r.race_id = c.id
            JOIN seasons s ON c.temporada_id = s.id
            WHERE c.categoria = ?1 AND r.posicao_final = 1
         )
         SELECT MIN(ano) FROM wins WHERE rn = ?2",
        rusqlite::params![categoria, target_wins],
        |row| row.get::<_, Option<i32>>(0),
    )?;
    Ok(year)
}

/// Retorna o número de vitórias do piloto na categoria esta temporada.
pub fn get_category_wins_this_season(
    conn: &Connection,
    pilot_id: &str,
    temporada_id: &str,
    categoria: &str,
) -> Result<i32, DbError> {
    let count: i32 = conn.query_row(
        "SELECT COUNT(*)
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE r.piloto_id = ?1
           AND c.temporada_id = ?2
           AND c.categoria = ?3
           AND r.posicao_final = 1",
        rusqlite::params![pilot_id, temporada_id, categoria],
        |row| row.get(0),
    )?;
    Ok(count)
}

/// Retorna a sequência atual de vitórias do piloto na categoria/temporada indicadas.
/// Conta rodadas consecutivas com posicao_final = 1 a partir da mais recente para trás.
/// Retorna 0 se o piloto nunca venceu ou não disputou corridas nessa categoria/temporada.
pub fn get_win_streak(
    conn: &Connection,
    pilot_id: &str,
    temporada_id: &str,
    categoria: &str,
) -> Result<u32, DbError> {
    let mut stmt = conn.prepare(
        "SELECT r.posicao_final
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE r.piloto_id = ?1
           AND c.temporada_id = ?2
           AND c.categoria = ?3
         ORDER BY c.rodada DESC",
    )?;
    let mut positions: Vec<i32> = Vec::new();
    let mut rows = stmt.query(rusqlite::params![pilot_id, temporada_id, categoria])?;
    while let Some(row) = rows.next()? {
        positions.push(row.get::<_, i32>(0)?);
    }
    let streak = positions.iter().take_while(|&&pos| pos == 1).count() as u32;
    Ok(streak)
}

/// Retorna o ID do piloto que liderava a categoria na temporada com base nos resultados
/// anteriores à rodada indicada (exclusive). Retorna None se não há rodadas anteriores.
/// Usado para detectar mudança de liderança: se o líder atual for diferente deste valor,
/// houve troca de liderança na rodada mais recente.
pub fn get_category_leader_before_round(
    conn: &Connection,
    temporada_id: &str,
    categoria: &str,
    before_round: i32,
) -> Result<Option<String>, DbError> {
    let result = conn.query_row(
        "SELECT r.piloto_id
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE c.temporada_id = ?1 AND c.categoria = ?2 AND c.rodada < ?3
         GROUP BY r.piloto_id
         ORDER BY
            SUM(r.pontos) DESC,
            SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) DESC,
            r.piloto_id ASC
         LIMIT 1",
        rusqlite::params![temporada_id, categoria, before_round],
        |row| row.get::<_, String>(0),
    );
    match result {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DbError::Sqlite(e)),
    }
}

/// Retorna os resultados de todos os pilotos numa rodada específica de uma categoria.
/// Retorna Vec<(driver_id, posicao_largada, posicao_final, is_dnf)>.
pub fn get_results_for_round(
    conn: &Connection,
    temporada_id: &str,
    categoria: &str,
    round: i32,
) -> Result<Vec<(String, i32, i32, bool)>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT r.piloto_id, r.posicao_largada, r.posicao_final, r.dnf
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE c.temporada_id = ?1 AND c.categoria = ?2 AND c.rodada = ?3
         ORDER BY r.posicao_final ASC",
    )?;
    let mut results = Vec::new();
    let mut rows = stmt.query(rusqlite::params![temporada_id, categoria, round])?;
    while let Some(row) = rows.next()? {
        results.push((
            row.get::<_, String>(0)?,
            row.get::<_, i32>(1)?,
            row.get::<_, i32>(2)?,
            row.get::<_, i32>(3)? != 0,
        ));
    }
    Ok(results)
}

/// Retorna fatos de DNF catalogado por piloto numa rodada de uma categoria.
/// Vec<(driver_id, incident_source, is_dnf, dnf_segment)>
/// incident_source vem de incident_catalog.incident_source (Mechanical/DriverError/PostCollision/Operational)
/// ou None se o piloto não tiver dnf_catalog_id.
pub fn get_dnf_incident_facts_for_round(
    conn: &Connection,
    temporada_id: &str,
    categoria: &str,
    round: i32,
) -> Result<Vec<(String, Option<String>, bool, Option<String>)>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT r.piloto_id, ic.incident_source, r.dnf, r.dnf_segment
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         LEFT JOIN incident_catalog ic ON r.dnf_catalog_id = ic.id
         WHERE c.temporada_id = ?1 AND c.categoria = ?2 AND c.rodada = ?3",
    )?;
    let mut results = Vec::new();
    let mut rows = stmt.query(rusqlite::params![temporada_id, categoria, round])?;
    while let Some(row) = rows.next()? {
        results.push((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, i32>(2)? != 0,
            row.get::<_, Option<String>>(3)?,
        ));
    }
    Ok(results)
}

/// Pilotos que já BATERAM (DNF por DriverError/PostCollision) NESTA pista — trauma.
pub fn get_track_crash_pilots(
    conn: &Connection,
    track_id: u32,
) -> Result<std::collections::HashSet<String>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT r.piloto_id
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN incident_catalog ic ON r.dnf_catalog_id = ic.id
         WHERE c.track_id = ?1 AND r.dnf = 1
           AND ic.incident_source IN ('DriverError', 'PostCollision')",
    )?;
    let mut set = std::collections::HashSet::new();
    let mut rows = stmt.query(rusqlite::params![track_id])?;
    while let Some(row) = rows.next()? {
        set.insert(row.get::<_, String>(0)?);
    }
    Ok(set)
}

/// Reconstrói as amostras de corrida do JOGADOR para o dossiê de habilidade: para
/// cada corrida que ele disputou, monta o grid de IAs com o atributo ATUAL de cada
/// uma (proxy do valor à época — deriva lenta, aceitável pra estimativa visual).
///
/// `race_results.race_id` referencia `calendar.id` (mesma convenção das outras
/// queries deste módulo). Clima diferente de seco = pista molhada.
pub fn get_player_race_samples(
    conn: &Connection,
    player_id: &str,
) -> Result<Vec<crate::player_skill::RaceSample>, DbError> {
    use crate::player_skill::{GridDriver, RaceSample, RaceTelemetry};

    // Telemetria por corrida (Fase 2) — só existe pras corridas dirigidas no iRacing.
    let mut tel_stmt = conn.prepare(
        "SELECT race_id, consistency, battle_fraction, on_track_gained, on_track_lost,
                start_delta, start_valid
         FROM player_race_telemetry
         WHERE race_id IN (SELECT race_id FROM race_results WHERE piloto_id = ?1)",
    )?;
    let tel_map: std::collections::HashMap<String, RaceTelemetry> = tel_stmt
        .query_map(rusqlite::params![player_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                RaceTelemetry {
                    consistency: row.get(1)?,
                    battle_fraction: row.get(2)?,
                    on_track_gained: row.get(3)?,
                    on_track_lost: row.get(4)?,
                    start_delta: row.get(5)?,
                    start_valid: row.get::<_, i32>(6)? != 0,
                },
            ))
        })?
        .collect::<Result<_, _>>()?;

    let mut stmt = conn.prepare(
        "SELECT c.id AS race_id, s.numero AS season, c.rodada AS round, c.clima AS clima,
                d.is_jogador, rr.posicao_final, rr.posicao_largada, rr.dnf, rr.incidents_count,
                d.skill, d.ritmo_classificacao, d.fator_chuva
         FROM race_results rr
         JOIN calendar c ON rr.race_id = c.id
         JOIN seasons s ON c.temporada_id = s.id
         JOIN drivers d ON rr.piloto_id = d.id
         WHERE rr.race_id IN (SELECT race_id FROM race_results WHERE piloto_id = ?1)
         ORDER BY s.numero, c.rodada, rr.race_id",
    )?;

    struct Row {
        race_id: String,
        season: i32,
        round: i32,
        clima: String,
        is_player: bool,
        finish: i32,
        start: i32,
        dnf: bool,
        incidents: i32,
        skill: f64,
        quali_skill: f64,
        rain_skill: f64,
    }

    let rows = stmt
        .query_map(rusqlite::params![player_id], |row| {
            Ok(Row {
                race_id: row.get(0)?,
                season: row.get(1)?,
                round: row.get(2)?,
                clima: row.get(3)?,
                is_player: row.get::<_, i32>(4)? != 0,
                finish: row.get(5)?,
                start: row.get(6)?,
                dnf: row.get::<_, i32>(7)? != 0,
                incidents: row.get(8)?,
                skill: row.get(9)?,
                quali_skill: row.get(10)?,
                rain_skill: row.get(11)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Agrupa por corrida preservando a ordem (a query já vem ordenada).
    let mut samples: Vec<RaceSample> = Vec::new();
    let mut current_id: Option<String> = None;
    for r in rows {
        let is_wet = !matches!(r.clima.as_str(), "Dry" | "Seco" | "");
        if current_id.as_deref() != Some(r.race_id.as_str()) {
            current_id = Some(r.race_id.clone());
            samples.push(RaceSample {
                season: r.season,
                round: r.round,
                is_wet,
                player_finish: 0,
                player_start: 0,
                player_dnf: false,
                player_incidents: 0,
                grid: Vec::new(),
                telemetry: tel_map.get(&r.race_id).cloned(),
            });
        }
        let sample = samples.last_mut().expect("acabou de empurrar");
        if r.is_player {
            sample.player_finish = r.finish;
            sample.player_start = r.start;
            sample.player_dnf = r.dnf;
            sample.player_incidents = r.incidents;
        } else {
            sample.grid.push(GridDriver {
                skill: r.skill,
                quali_skill: r.quali_skill,
                rain_skill: r.rain_skill,
                finish: r.finish,
                start: r.start,
                dnf: r.dnf,
            });
        }
    }

    // Descarta corridas em que o jogador não tem linha (defensivo) ou sem grid.
    samples.retain(|s| s.player_finish > 0 && !s.grid.is_empty());
    Ok(samples)
}

/// Grava (ou substitui) a telemetria compacta de uma corrida do JOGADOR — Fase 2
/// do dossiê de habilidade. `race_id` = id da entrada do calendário (mesma
/// convenção de `race_results`, para a query do dossiê juntar as duas).
pub fn upsert_player_race_telemetry(
    conn: &Connection,
    race_id: &str,
    row: &crate::iracing_sdk::telemetry_analysis::PlayerRaceTelemetry,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO player_race_telemetry
            (race_id, laps_seen, race_laps, consistency, battle_fraction,
             on_track_gained, on_track_lost, start_delta, start_valid)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(race_id) DO UPDATE SET
            laps_seen = excluded.laps_seen,
            race_laps = excluded.race_laps,
            consistency = excluded.consistency,
            battle_fraction = excluded.battle_fraction,
            on_track_gained = excluded.on_track_gained,
            on_track_lost = excluded.on_track_lost,
            start_delta = excluded.start_delta,
            start_valid = excluded.start_valid",
        rusqlite::params![
            race_id,
            row.laps_seen,
            row.race_laps,
            row.consistency,
            row.battle_fraction,
            row.on_track_gained,
            row.on_track_lost,
            row.start_delta,
            row.start_valid as i32,
        ],
    )?;
    Ok(())
}

/// Uma linha de resultado de etapa (com dados do piloto) — p/ exportar ao iRacing.
pub struct EventResultRow {
    pub piloto_id: String,
    pub nome: String,
    pub is_jogador: bool,
    pub finish: i64,
    pub start: i64,
    pub laps: i64,
    pub total_ms: f64,
    pub gap_ms: f64,
    pub incidents: i64,
    pub dnf: bool,
    pub has_fastest: bool,
    pub dnf_reason: Option<String>,
}

/// Resultados de UMA corrida (todos os pilotos + dados pro bloco do aiseason).
pub fn get_event_results(conn: &Connection, race_id: &str) -> Result<Vec<EventResultRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT rr.piloto_id, d.nome, d.is_jogador, rr.posicao_final, rr.posicao_largada,
                rr.voltas_completadas, rr.tempo_total, rr.gap_to_winner_ms, rr.incidents_count,
                rr.dnf, rr.fastest_lap, rr.dnf_reason
         FROM race_results rr
         JOIN drivers d ON rr.piloto_id = d.id
         WHERE rr.race_id = ?1
         ORDER BY rr.posicao_final",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![race_id], |row| {
            Ok(EventResultRow {
                piloto_id: row.get(0)?,
                nome: row.get(1)?,
                is_jogador: row.get::<_, i32>(2)? != 0,
                finish: row.get(3)?,
                start: row.get(4)?,
                laps: row.get(5)?,
                total_ms: row.get::<_, f64>(6)?,
                gap_ms: row.get::<_, f64>(7)?,
                incidents: row.get(8)?,
                dnf: row.get::<_, i32>(9)? != 0,
                has_fastest: row.get::<_, f64>(10)? != 0.0,
                dnf_reason: row.get(11)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Uma chegada recente do piloto (mais recente primeiro) — base para leitura de
/// forma e para a média que alimenta o mérito da posição esperada.
#[derive(Debug, Clone)]
pub struct RecentFinish {
    pub season_num: i32,
    pub round: i32,
    pub finish: i32,
    pub is_dnf: bool,
}

/// Últimas `limit` chegadas do piloto na categoria ANTERIORES à (temporada, rodada)
/// dada — EXCLUI a corrida atual (que já está persistida). Mais recente primeiro.
pub fn get_recent_finishes_before(
    conn: &Connection,
    pilot_id: &str,
    categoria: &str,
    before_season_num: i32,
    before_round: i32,
    limit: i32,
) -> Result<Vec<RecentFinish>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT s.numero, c.rodada, r.posicao_final, r.dnf
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN seasons s ON c.temporada_id = s.id
         WHERE r.piloto_id = ?1 AND c.categoria = ?2
           AND (s.numero < ?3 OR (s.numero = ?3 AND c.rodada < ?4))
         ORDER BY s.numero DESC, c.rodada DESC
         LIMIT ?5",
    )?;
    let rows = stmt
        .query_map(
            rusqlite::params![pilot_id, categoria, before_season_num, before_round, limit],
            |row| {
                Ok(RecentFinish {
                    season_num: row.get(0)?,
                    round: row.get(1)?,
                    finish: row.get(2)?,
                    is_dnf: row.get::<_, i32>(3)? != 0,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Resultado do piloto por rodada na temporada/categoria — (rodada, chegada, dnf).
/// Usado para o confronto interno entre companheiros de equipe ao longo do ano.
pub fn get_pilot_season_results(
    conn: &Connection,
    pilot_id: &str,
    temporada_id: &str,
    categoria: &str,
) -> Result<Vec<(i32, i32, bool)>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT c.rodada, r.posicao_final, r.dnf
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE r.piloto_id = ?1 AND c.temporada_id = ?2 AND c.categoria = ?3
         ORDER BY c.rodada ASC",
    )?;
    let rows = stmt
        .query_map(
            rusqlite::params![pilot_id, temporada_id, categoria],
            |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i32>(2)? != 0)),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// `track_id` da corrida (temporada/categoria/rodada). `None` se não encontrar.
pub fn get_round_track_id(
    conn: &Connection,
    temporada_id: &str,
    categoria: &str,
    round: i32,
) -> Result<Option<i64>, DbError> {
    let res: Option<Option<i64>> = conn
        .query_row(
            "SELECT track_id FROM calendar
             WHERE temporada_id = ?1 AND categoria = ?2 AND rodada = ?3",
            rusqlite::params![temporada_id, categoria, round],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?;
    Ok(res.flatten())
}

/// Histórico do piloto NESTE circuito (`track_id`), em todas as temporadas, EXCLUINDO
/// a rodada atual. Serve para a narrativa de "tem boa história nesta pista".
#[derive(Debug, Clone, Default)]
pub struct PilotTrackHistory {
    pub starts: i32,
    pub wins: i32,
    pub podiums: i32,
    /// Melhor chegada (sem contar abandonos). `None` se nunca completou aqui.
    pub best_finish: Option<i32>,
}

pub fn get_pilot_track_history(
    conn: &Connection,
    pilot_id: &str,
    track_id: i64,
    exclude_temporada_id: &str,
    exclude_round: i32,
) -> Result<PilotTrackHistory, DbError> {
    let row = conn.query_row(
        "SELECT
            COUNT(*),
            COALESCE(SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN r.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END), 0),
            MIN(CASE WHEN r.dnf = 0 THEN r.posicao_final END)
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         WHERE r.piloto_id = ?1 AND c.track_id = ?2
           AND NOT (c.temporada_id = ?3 AND c.rodada = ?4)",
        rusqlite::params![pilot_id, track_id, exclude_temporada_id, exclude_round],
        |r| {
            Ok(PilotTrackHistory {
                starts: r.get(0)?,
                wins: r.get(1)?,
                podiums: r.get(2)?,
                best_finish: r.get::<_, Option<i32>>(3)?,
            })
        },
    )?;
    Ok(row)
}

/// Confronto direto entre o jogador e um rival na categoria (todas as temporadas
/// persistidas). `races_together` = corridas em que ambos aparecem no resultado;
/// `player_ahead` = quantas o jogador terminou à frente. `best_*` = a corrida em
/// que o jogador teve sua MELHOR posição entre as que bateu o rival (para citar).
#[derive(Debug, Clone, Default)]
pub struct HeadToHead {
    pub races_together: i32,
    pub player_ahead: i32,
    pub best_finish: Option<i32>,
    pub best_track: Option<String>,
}

pub fn get_head_to_head(
    conn: &Connection,
    player_id: &str,
    rival_id: &str,
    categoria: &str,
) -> Result<HeadToHead, DbError> {
    let mut stmt = conn.prepare(
        "SELECT pr.posicao_final, rr.posicao_final,
                COALESCE(NULLIF(c.track_name, ''), c.pista)
         FROM race_results pr
         JOIN race_results rr ON rr.race_id = pr.race_id AND rr.piloto_id = ?2
         JOIN calendar c ON c.id = pr.race_id
         WHERE pr.piloto_id = ?1 AND c.categoria = ?3",
    )?;
    let mut rows = stmt.query(rusqlite::params![player_id, rival_id, categoria])?;
    let mut h = HeadToHead::default();
    while let Some(row) = rows.next()? {
        let player_pos: i32 = row.get(0)?;
        let rival_pos: i32 = row.get(1)?;
        let track: String = row.get(2)?;
        h.races_together += 1;
        if player_pos < rival_pos {
            h.player_ahead += 1;
            if h.best_finish.map_or(true, |b| player_pos < b) {
                h.best_finish = Some(player_pos);
                h.best_track = Some(track);
            }
        }
    }
    Ok(h)
}

/// Piloto ATIVO na categoria (não-jogador) com quem o jogador mais correu — o rival
/// de história mais longa no grid atual. `None` se o jogador nunca correu ali.
pub fn most_faced_rival(
    conn: &Connection,
    player_id: &str,
    categoria: &str,
) -> Result<Option<String>, DbError> {
    let id = conn
        .query_row(
            "SELECT rr.piloto_id
             FROM race_results pr
             JOIN race_results rr ON rr.race_id = pr.race_id AND rr.piloto_id != pr.piloto_id
             JOIN calendar c ON c.id = pr.race_id
             JOIN drivers d ON d.id = rr.piloto_id
             WHERE pr.piloto_id = ?1 AND c.categoria = ?2
               AND d.categoria_atual = ?2 AND d.is_jogador = 0 AND d.status != 'Aposentado'
             GROUP BY rr.piloto_id
             ORDER BY COUNT(*) DESC, rr.piloto_id ASC
             LIMIT 1",
            rusqlite::params![player_id, categoria],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE seasons (id TEXT PRIMARY KEY, numero INTEGER NOT NULL);
            CREATE TABLE calendar (
                id TEXT PRIMARY KEY,
                temporada_id TEXT NOT NULL,
                rodada INTEGER NOT NULL,
                categoria TEXT NOT NULL
            );
            CREATE TABLE drivers (id TEXT PRIMARY KEY, nome TEXT NOT NULL);
            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                posicao_largada INTEGER NOT NULL DEFAULT 0,
                posicao_final INTEGER NOT NULL,
                dnf INTEGER NOT NULL DEFAULT 0,
                pontos REAL NOT NULL
            );
            INSERT INTO seasons (id, numero) VALUES ('S1', 1), ('S2', 2);
            INSERT INTO drivers (id, nome) VALUES ('P001', 'Piloto Um'), ('P002', 'Piloto Dois');
            INSERT INTO calendar (id, temporada_id, rodada, categoria) VALUES
                ('R1', 'S1', 1, 'gt4'),
                ('R2', 'S1', 2, 'gt4'),
                ('R3', 'S2', 1, 'gt4');
            ",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_get_last_career_win() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos)
             VALUES ('R2', 'P001', 'T001', 1, 25.0)",
            [],
        )
        .unwrap();

        let result = get_last_career_win(&conn, "P001").unwrap();
        assert_eq!(result, Some((1, 2)));
    }

    #[test]
    fn test_get_last_career_win_none() {
        let conn = setup_db();
        let result = get_last_career_win(&conn, "P001").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_wins_with_team() {
        let conn = setup_db();
        conn.execute_batch(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
             ('R1', 'P001', 'T001', 1, 25.0),
             ('R2', 'P001', 'T001', 1, 25.0),
             ('R3', 'P001', 'T002', 1, 25.0);",
        )
        .unwrap();

        assert_eq!(get_wins_with_team(&conn, "P001", "T001").unwrap(), 2);
        assert_eq!(get_wins_with_team(&conn, "P001", "T002").unwrap(), 1);
        assert_eq!(get_wins_with_team(&conn, "P001", "T003").unwrap(), 0);
    }

    #[test]
    fn test_get_category_standings() {
        let conn = setup_db();
        conn.execute_batch(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
             ('R1', 'P001', 'T001', 1, 25.0),
             ('R1', 'P002', 'T002', 2, 18.0),
             ('R2', 'P001', 'T001', 2, 18.0),
             ('R2', 'P002', 'T002', 1, 25.0);",
        )
        .unwrap();

        let standings = get_category_standings(&conn, "S1", "gt4").unwrap();
        assert_eq!(standings.len(), 2);
        assert_eq!(standings[0].points, 43.0);
        assert_eq!(standings[0].position, 1);
        assert_eq!(standings[1].position, 2);
    }

    #[test]
    fn test_get_category_wins_this_season() {
        let conn = setup_db();
        conn.execute_batch(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
             ('R1', 'P001', 'T001', 1, 25.0),
             ('R2', 'P001', 'T001', 2, 18.0);",
        )
        .unwrap();

        assert_eq!(
            get_category_wins_this_season(&conn, "P001", "S1", "gt4").unwrap(),
            1
        );
        assert_eq!(
            get_category_wins_this_season(&conn, "P001", "S2", "gt4").unwrap(),
            0
        );
    }

    #[test]
    fn test_get_category_standings_breaks_ties_by_name_when_points_and_wins_tie() {
        let conn = setup_db();
        conn.execute_batch(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
             ('R3', 'P001', 'T001', 2, 18.0),
             ('R3', 'P002', 'T002', 2, 18.0);",
        )
        .unwrap();

        let standings = get_category_standings(&conn, "S2", "gt4").unwrap();
        assert_eq!(standings.len(), 2);
        assert_eq!(standings[0].pilot_id, "P002");
        assert_eq!(standings[1].pilot_id, "P001");
    }

    #[test]
    fn test_get_category_leader_before_round_uses_deterministic_tiebreak() {
        let conn = setup_db();
        conn.execute_batch(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
             ('R1', 'P001', 'T001', 1, 25.0),
             ('R1', 'P002', 'T002', 2, 18.0),
             ('R2', 'P001', 'T001', 2, 18.0),
             ('R2', 'P002', 'T002', 1, 25.0);",
        )
        .unwrap();

        let leader = get_category_leader_before_round(&conn, "S1", "gt4", 3).unwrap();
        assert_eq!(leader.as_deref(), Some("P001"));
    }

    #[test]
    fn test_get_recent_finishes_before_excludes_current_and_orders_desc() {
        let conn = setup_db();
        conn.execute_batch(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, dnf, pontos) VALUES
             ('R1', 'P001', 'T001', 5, 0, 10.0),
             ('R2', 'P001', 'T001', 1, 0, 25.0),
             ('R3', 'P001', 'T001', 3, 0, 15.0);",
        )
        .unwrap();

        // Antes de S2/r1 (numero=2, rodada=1) entram só R1 e R2 (ambas em S1).
        let recent =
            get_recent_finishes_before(&conn, "P001", "gt4", 2, 1, 5).unwrap();
        assert_eq!(recent.len(), 2);
        // Mais recente primeiro: R2 (rodada 2) antes de R1 (rodada 1).
        assert_eq!(recent[0].finish, 1);
        assert_eq!(recent[1].finish, 5);
    }

    #[test]
    fn test_get_pilot_season_results_ordered_by_round() {
        let conn = setup_db();
        conn.execute_batch(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, dnf, pontos) VALUES
             ('R2', 'P001', 'T001', 4, 0, 12.0),
             ('R1', 'P001', 'T001', 2, 1, 0.0);",
        )
        .unwrap();

        let res = get_pilot_season_results(&conn, "P001", "S1", "gt4").unwrap();
        assert_eq!(res, vec![(1, 2, true), (2, 4, false)]);
    }

    #[test]
    fn test_get_pilot_track_history_counts_wins_excluding_current() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE calendar (
                id TEXT PRIMARY KEY,
                temporada_id TEXT NOT NULL,
                rodada INTEGER NOT NULL,
                categoria TEXT NOT NULL,
                track_id INTEGER
            );
            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                posicao_final INTEGER NOT NULL,
                dnf INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO calendar (id, temporada_id, rodada, categoria, track_id) VALUES
                ('A', 'S1', 1, 'gt4', 42),
                ('B', 'S2', 1, 'gt4', 42),
                ('C', 'S3', 1, 'gt4', 42),
                ('D', 'S1', 2, 'gt4', 99);
            INSERT INTO race_results (race_id, piloto_id, posicao_final, dnf) VALUES
                ('A', 'P001', 1, 0),
                ('B', 'P001', 3, 0),
                ('C', 'P001', 1, 0),
                ('D', 'P001', 1, 0);",
        )
        .unwrap();

        assert_eq!(get_round_track_id(&conn, "S3", "gt4", 1).unwrap(), Some(42));

        // Excluindo a corrida atual (S3/r1): sobram A (1º) e B (3º) na pista 42.
        let th = get_pilot_track_history(&conn, "P001", 42, "S3", 1).unwrap();
        assert_eq!(th.starts, 2);
        assert_eq!(th.wins, 1); // só A; a vitória atual (C) foi excluída
        assert_eq!(th.podiums, 2);
        assert_eq!(th.best_finish, Some(1));
    }

    #[test]
    fn test_get_results_for_round_propagates_invalid_row_error() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO race_results
                (race_id, piloto_id, equipe_id, posicao_largada, posicao_final, dnf, pontos)
             VALUES ('R1', 'P001', 'T001', 1, 'quebrado', 0, 25.0)",
            [],
        )
        .unwrap();

        let err = get_results_for_round(&conn, "S1", "gt4", 1)
            .expect_err("invalid row should fail instead of being ignored");
        assert!(err.to_string().contains("SQLite error"));
    }
}
