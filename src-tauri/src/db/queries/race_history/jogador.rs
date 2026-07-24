//! Amostras e telemetria das corridas do JOGADOR — base do dossiê de habilidade.

use rusqlite::Connection;

use crate::db::connection::DbError;

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
