use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::models::season::Season;

#[derive(Debug, Clone)]
struct TeamSeasonSnapshot {
    team_id: String,
    team_name: String,
    season_number: i32,
    year: i32,
    category: String,
    class_name: Option<String>,
    championship_position: Option<i32>,
    points: f64,
    wins: i32,
    podiums: i32,
    poles: i32,
    races: i32,
    /// Melhor resultado do ano (menor posição de chegada), DNFs incluídos —
    /// calculado igual a `teams.stats_melhor_resultado` (ver race.rs). Usado só
    /// como desempate na classificação de construtores, para que o arquivo/Atlas
    /// concorde com a decisão de rebaixamento (promotion::standings).
    best_result: i32,
    constructor_titles: i32,
    driver_one_id: Option<String>,
    driver_two_id: Option<String>,
    snapshot_json: String,
}

pub(crate) fn archive_team_season(conn: &Connection, season: &Season) -> Result<(), String> {
    let mut snapshots = load_team_snapshots(conn, season)?;
    assign_constructor_positions(&mut snapshots);

    for snapshot in snapshots {
        conn.execute(
            "INSERT INTO team_season_archive (
                team_id, season_number, ano, categoria, classe, posicao_campeonato,
                pontos, vitorias, podios, poles, corridas, titulos_construtores,
                piloto_1_id, piloto_2_id, snapshot_json
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
             )
             ON CONFLICT(team_id, season_number, categoria) DO UPDATE SET
                ano = excluded.ano,
                classe = excluded.classe,
                posicao_campeonato = excluded.posicao_campeonato,
                pontos = excluded.pontos,
                vitorias = excluded.vitorias,
                podios = excluded.podios,
                poles = excluded.poles,
                corridas = excluded.corridas,
                titulos_construtores = excluded.titulos_construtores,
                piloto_1_id = excluded.piloto_1_id,
                piloto_2_id = excluded.piloto_2_id,
                snapshot_json = excluded.snapshot_json,
                archived_at = datetime('now')",
            params![
                snapshot.team_id,
                snapshot.season_number,
                snapshot.year,
                snapshot.category,
                snapshot.class_name,
                snapshot.championship_position,
                snapshot.points,
                snapshot.wins,
                snapshot.podiums,
                snapshot.poles,
                snapshot.races,
                snapshot.constructor_titles,
                snapshot.driver_one_id,
                snapshot.driver_two_id,
                snapshot.snapshot_json,
            ],
        )
        .map_err(|e| format!("Falha ao arquivar equipe na temporada: {e}"))?;

        // Marco de TÍTULOS DE CONSTRUTORES: com o arquivo desta temporada já gravado, a
        // campeã pode ter virado a dona ISOLADA do recorde da categoria. A partir de 3,
        // para não marcar a bicampeã. Camada narrativa — erro é silencioso. O marco
        // guarda a EQUIPE nos campos `pilot_id`/`pilot_name` (o rodapé trata por métrica).
        if snapshot.championship_position == Some(1) && !snapshot.category.is_empty() {
            let titles = crate::db::queries::teams::get_team_category_constructor_titles(
                conn,
                &snapshot.team_id,
                &snapshot.category,
            )
            .unwrap_or(0);
            let others = crate::db::queries::teams::get_category_constructor_titles_leader_excluding(
                conn,
                &snapshot.category,
                &snapshot.team_id,
            )
            .unwrap_or(0);
            if titles >= 3 && titles > others {
                let team_name = crate::db::queries::teams::get_team_by_id(conn, &snapshot.team_id)
                    .ok()
                    .flatten()
                    .map(|t| t.nome)
                    .unwrap_or_else(|| snapshot.team_id.clone());
                let _ = crate::db::queries::milestones::insert_milestone(
                    conn,
                    &snapshot.category,
                    &crate::db::queries::milestones::RecordMilestone {
                        metric: "constructor_titles".to_string(),
                        pilot_id: snapshot.team_id.clone(),
                        pilot_name: team_name,
                        value: titles,
                        previous_value: Some(titles - 1),
                        context: String::new(),
                        season_number: snapshot.season_number,
                        ano: snapshot.year,
                        round: 0,
                    },
                );
            }
        }
    }

    Ok(())
}

fn load_team_snapshots(
    conn: &Connection,
    season: &Season,
) -> Result<Vec<TeamSeasonSnapshot>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                rr.equipe_id,
                c.categoria,
                NULLIF(TRIM(t.classe), '') AS classe,
                t.piloto_1_id,
                t.piloto_2_id,
                COALESCE(SUM(rr.pontos), 0.0) AS pontos,
                COALESCE(SUM(CASE WHEN rr.posicao_final = 1 THEN 1 ELSE 0 END), 0) AS vitorias,
                COALESCE(SUM(CASE WHEN rr.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END), 0) AS podios,
                COALESCE(SUM(CASE WHEN rr.posicao_largada = 1 THEN 1 ELSE 0 END), 0) AS poles,
                COUNT(DISTINCT rr.race_id) AS corridas,
                COALESCE(t.nome, rr.equipe_id) AS nome,
                COALESCE(MIN(rr.posicao_final), 99) AS melhor_resultado
             FROM race_results rr
             JOIN calendar c ON c.id = rr.race_id
             JOIN teams t ON t.id = rr.equipe_id
             WHERE COALESCE(c.season_id, c.temporada_id) = ?1
             GROUP BY rr.equipe_id, c.categoria, NULLIF(TRIM(t.classe), '')",
        )
        .map_err(|e| format!("Falha ao preparar arquivo de equipes: {e}"))?;
    let rows = stmt
        .query_map(params![&season.id], |row| {
            let team_id: String = row.get(0)?;
            let category: String = row.get(1)?;
            let class_name: Option<String> = row.get(2)?;
            let driver_one_id: Option<String> = row.get(3)?;
            let driver_two_id: Option<String> = row.get(4)?;
            let points: f64 = row.get(5)?;
            let wins: i32 = row.get(6)?;
            let podiums: i32 = row.get(7)?;
            let poles: i32 = row.get(8)?;
            let races: i32 = row.get(9)?;
            let team_name: String = row.get(10)?;
            let best_result: i32 = row.get(11)?;
            Ok((
                team_id,
                category,
                class_name,
                driver_one_id,
                driver_two_id,
                points,
                wins,
                podiums,
                poles,
                races,
                team_name,
                best_result,
            ))
        })
        .map_err(|e| format!("Falha ao consultar arquivo de equipes: {e}"))?;

    let mut snapshots = Vec::new();
    for row in rows {
        let (
            team_id,
            category,
            class_name,
            driver_one_id,
            driver_two_id,
            points,
            wins,
            podiums,
            poles,
            races,
            team_name,
            best_result,
        ) = row.map_err(|e| format!("Falha ao mapear arquivo de equipes: {e}"))?;

        snapshots.push(TeamSeasonSnapshot {
            team_id,
            team_name,
            season_number: season.numero,
            year: season.ano,
            category,
            class_name,
            championship_position: None,
            points,
            wins,
            podiums,
            poles,
            races,
            best_result,
            constructor_titles: 0,
            driver_one_id,
            driver_two_id,
            snapshot_json: String::new(),
        });
    }

    Ok(snapshots)
}

fn assign_constructor_positions(snapshots: &mut [TeamSeasonSnapshot]) {
    let mut by_category: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, snapshot) in snapshots.iter().enumerate() {
        by_category
            .entry(constructor_ranking_group_key(snapshot))
            .or_default()
            .push(index);
    }

    for indices in by_category.values_mut() {
        indices.sort_by(|left, right| {
            snapshots[*right]
                .points
                .partial_cmp(&snapshots[*left].points)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| snapshots[*right].wins.cmp(&snapshots[*left].wins))
                // Desempate IGUAL ao do campeonato/rebaixamento (promotion::standings):
                // melhor resultado do ano (menor = melhor), depois nome. Antes o
                // desempate era por team_id (alfabético, sem sentido esportivo), o que
                // fazia o arquivo/Atlas ordenar o empate ao contrário do rebaixamento —
                // aparecia "9º rebaixado, 10º ficou" num empate de 0 pontos/0 vitórias.
                .then_with(|| snapshots[*left].best_result.cmp(&snapshots[*right].best_result))
                .then_with(|| snapshots[*left].team_name.cmp(&snapshots[*right].team_name))
        });

        for (position, index) in indices.iter().enumerate() {
            let snapshot = &mut snapshots[*index];
            let position = position as i32 + 1;
            snapshot.championship_position = Some(position);
            snapshot.constructor_titles = i32::from(position == 1);
            snapshot.snapshot_json = serde_json::json!({
                "team_id": snapshot.team_id,
                "season_number": snapshot.season_number,
                "ano": snapshot.year,
                "categoria": snapshot.category,
                "classe": snapshot.class_name,
                "posicao_campeonato": position,
                "pontos": snapshot.points,
                "vitorias": snapshot.wins,
                "podios": snapshot.podiums,
                "poles": snapshot.poles,
                "corridas": snapshot.races,
                "titulos_construtores": snapshot.constructor_titles,
                "piloto_1_id": snapshot.driver_one_id,
                "piloto_2_id": snapshot.driver_two_id,
            })
            .to_string();
        }
    }
}

/// Consolida o histórico de CARREIRA de cada equipe a partir dos arquivos de
/// temporada (ideia 2 do "sistema de equipes vivo"). Até aqui os campos
/// `historico_*` do registro do time nunca eram gravados — ficavam em 0 — e por
/// isso o termo `titulos_construtores*2` do `elite_score` das dinastias
/// ([`crate::finance::strategy`]) era código morto: a elite fictícia (Production)
/// era ordenada só por reputação. Com o roll-up, campeões passados acumulam
/// títulos e ficam elite por LEGADO, não só por forma.
///
/// **Recompute-from-archive** (não incremento): recalcula o total como SOMA sobre
/// `team_season_archive` a cada offseason. É idempotente (rodar 2× dá o mesmo
/// resultado, sem dupla contagem) e faz backfill de saves antigos automaticamente.
/// Deve rodar DEPOIS de `archive_team_season` (a linha da temporada atual já
/// existe). Os títulos de PILOTO da equipe vêm do `driver_season_archive` (onde o
/// `team_id` da temporada mora no snapshot JSON).
pub(crate) fn roll_up_team_career_history(conn: &Connection) -> Result<(), String> {
    // Agregados dos construtores: uma varredura do arquivo de equipes, agrupada
    // por time (soma vitórias/pódios/poles/pontos/títulos de construtor).
    let mut stmt = conn
        .prepare(
            "SELECT team_id,
                    COALESCE(SUM(vitorias), 0),
                    COALESCE(SUM(podios), 0),
                    COALESCE(SUM(poles), 0),
                    CAST(COALESCE(SUM(pontos), 0) AS INTEGER),
                    COALESCE(SUM(titulos_construtores), 0)
             FROM team_season_archive
             GROUP BY team_id",
        )
        .map_err(|e| format!("Falha ao preparar roll-up de histórico: {e}"))?;
    // team_id -> [vitorias, podios, poles, pontos, titulos_construtores, titulos_pilotos]
    let mut totals: HashMap<String, [i64; 6]> = HashMap::new();
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                [
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    0,
                ],
            ))
        })
        .map_err(|e| format!("Falha ao consultar roll-up de construtores: {e}"))?;
    for row in rows {
        let (team_id, agg) = row.map_err(|e| format!("Falha ao mapear roll-up: {e}"))?;
        totals.insert(team_id, agg);
    }
    drop(stmt);

    // Títulos de PILOTO por equipe: uma varredura do arquivo de pilotos, filtrando
    // os campeões (titulos=1) e agrupando pelo team_id da temporada (no JSON).
    let mut stmt = conn
        .prepare(
            "SELECT json_extract(snapshot_json, '$.team_id') AS tid, COUNT(*)
             FROM driver_season_archive
             WHERE json_extract(snapshot_json, '$.titulos') = 1
               AND tid IS NOT NULL
             GROUP BY tid",
        )
        .map_err(|e| format!("Falha ao preparar roll-up de títulos de piloto: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| format!("Falha ao consultar títulos de piloto: {e}"))?;
    for row in rows {
        let (team_id, driver_titles) =
            row.map_err(|e| format!("Falha ao mapear títulos de piloto: {e}"))?;
        totals.entry(team_id).or_insert([0; 6])[5] = driver_titles;
    }
    drop(stmt);

    // Grava os totais recomputados no registro de cada equipe (só as colunas de
    // histórico). Equipes sem arquivo mantêm 0.
    for (team_id, t) in &totals {
        conn.execute(
            "UPDATE teams SET
                historico_vitorias = ?1,
                historico_podios = ?2,
                historico_poles = ?3,
                historico_pontos = ?4,
                carreira_titulos = ?5,
                historico_titulos_pilotos = ?6,
                updated_at = CURRENT_TIMESTAMP
             WHERE id = ?7",
            rusqlite::params![t[0], t[1], t[2], t[3], t[4], t[5], team_id],
        )
        .map_err(|e| format!("Falha ao gravar histórico da equipe {team_id}: {e}"))?;
    }

    Ok(())
}

fn constructor_ranking_group_key(snapshot: &TeamSeasonSnapshot) -> String {
    if matches!(
        snapshot.category.as_str(),
        "production_challenger" | "endurance"
    ) {
        return format!(
            "{}::{}",
            snapshot.category,
            snapshot.class_name.as_deref().unwrap_or("")
        );
    }
    snapshot.category.clone()
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::db::migrations;
    use crate::models::season::Season;

    #[test]
    fn archive_team_season_persists_points_wins_podiums_and_title() {
        let conn = setup_team_archive_conn();
        let season = seed_completed_team_archive_world(&conn);

        archive_team_season(&conn, &season).expect("archive");

        let (points, wins, podiums, titles): (f64, i32, i32, i32) = conn
            .query_row(
                "SELECT pontos, vitorias, podios, titulos_construtores
                 FROM team_season_archive
                 WHERE team_id = 'T001'
                   AND season_number = ?1
                   AND categoria = 'mazda_rookie'",
                params![season.numero],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("archive row");

        assert_eq!(points, 100.0);
        assert_eq!(wins, 1);
        assert_eq!(podiums, 2);
        assert_eq!(titles, 1);
    }

    #[test]
    fn archive_team_season_breaks_point_ties_by_best_result_like_relegation() {
        // Regressão: dois times empatados em 0 pontos / 0 vitórias. O desempate do
        // arquivo tem que ser o MESMO do rebaixamento (melhor resultado do ano), não
        // team_id alfabético — senão a Atlas mostra o penúltimo acima do último e fica
        // "9º rebaixado, 10º ficou". T_A (id menor) foi o PIOR (melhor result 12) e tem
        // que ficar em ÚLTIMO; T_B (melhor result 11) fica na frente.
        let conn = setup_team_archive_conn();
        conn.execute_batch(
            "
            INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
            VALUES ('S_TIE', 5, 2025, 'Finalizada', 1, 'BlocoRegular', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

            INSERT INTO drivers (id, nome, idade, nacionalidade, genero, categoria_atual, status, ano_inicio_carreira)
            VALUES
                ('P_A', 'Piloto A', 25, 'Brasil', 'M', 'mazda_amador', 'Ativo', 2020),
                ('P_B', 'Piloto B', 24, 'Brasil', 'M', 'mazda_amador', 'Ativo', 2021);

            INSERT INTO teams (id, nome, nome_curto, categoria, ativa, piloto_1_id, created_at, updated_at)
            VALUES
                ('T_A', 'Team A', 'TA', 'mazda_amador', 1, 'P_A', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
                ('T_B', 'Team B', 'TB', 'mazda_amador', 1, 'P_B', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

            INSERT INTO calendar (id, temporada_id, season_id, rodada, pista, categoria, status, nome, track_name, track_config)
            VALUES ('R_TIE', 'S_TIE', 'S_TIE', 1, 'Interlagos', 'mazda_amador', 'Concluida', 'Tie', 'Interlagos', 'default');

            -- Ambos 0 pontos; T_A chegou em 12º (pior), T_B em 11º (melhor).
            INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_largada, posicao_final, voltas_completadas, pontos)
            VALUES
                ('R_TIE', 'P_A', 'T_A', 12, 12, 10, 0.0),
                ('R_TIE', 'P_B', 'T_B', 11, 11, 10, 0.0);
            ",
        )
        .expect("seed tie world");
        let season = Season::new("S_TIE".to_string(), 5, 2025);

        archive_team_season(&conn, &season).expect("archive");

        let pos_a = read_team_archive_position(&conn, "T_A", season.numero, "mazda_amador");
        let pos_b = read_team_archive_position(&conn, "T_B", season.numero, "mazda_amador");
        assert_eq!(pos_b, 1, "T_B (melhor resultado 11) fica na frente");
        assert_eq!(pos_a, 2, "T_A (melhor resultado 12) fica em último");
    }

    #[test]
    fn archive_team_season_is_idempotent_for_same_team_season_category() {
        let conn = setup_team_archive_conn();
        let season = seed_completed_team_archive_world(&conn);

        archive_team_season(&conn, &season).expect("first archive");
        archive_team_season(&conn, &season).expect("second archive");

        assert_eq!(count_team_archive_rows(&conn), 1);
        assert_eq!(
            read_team_archive_points(&conn, "T001", season.numero, "mazda_rookie"),
            100.0
        );
    }

    #[test]
    fn archive_team_season_ranks_special_categories_by_class() {
        let conn = setup_team_archive_conn();
        conn.execute_batch(
            "
            INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
            VALUES ('S_SPECIAL', 2, 2025, 'Finalizada', 1, 'BlocoRegular', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

            INSERT INTO drivers (id, nome, idade, nacionalidade, genero, categoria_atual, status, ano_inicio_carreira)
            VALUES
                ('P_BMW', 'Piloto BMW', 25, 'Brasil', 'M', 'production_challenger', 'Ativo', 2020),
                ('P_MAZDA', 'Piloto Mazda', 24, 'Brasil', 'M', 'production_challenger', 'Ativo', 2021);

            INSERT INTO teams (
                id, nome, nome_curto, categoria, classe, ativa, piloto_1_id,
                created_at, updated_at
            ) VALUES
                ('T_BMW', 'Equipe BMW', 'BMW', 'production_challenger', 'bmw', 1, 'P_BMW', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
                ('T_MAZDA', 'Equipe Mazda', 'MZD', 'production_challenger', 'mazda', 1, 'P_MAZDA', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

            INSERT INTO calendar (
                id, temporada_id, season_id, rodada, pista, categoria, status, nome,
                track_name, track_config
            ) VALUES
                ('R_BMW', 'S_SPECIAL', 'S_SPECIAL', 1, 'Interlagos', 'production_challenger', 'Concluida', 'BMW', 'Interlagos', 'default'),
                ('R_MAZDA', 'S_SPECIAL', 'S_SPECIAL', 1, 'Interlagos', 'production_challenger', 'Concluida', 'Mazda', 'Interlagos', 'default');

            INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final, voltas_completadas, pontos
            ) VALUES
                ('R_BMW', 'P_BMW', 'T_BMW', 1, 1, 10, 60.0),
                ('R_MAZDA', 'P_MAZDA', 'T_MAZDA', 1, 1, 10, 30.0);
            ",
        )
        .expect("seed special world");
        let season = Season::new("S_SPECIAL".to_string(), 2, 2025);

        archive_team_season(&conn, &season).expect("archive");

        let bmw_position =
            read_team_archive_position(&conn, "T_BMW", season.numero, "production_challenger");
        let mazda_position =
            read_team_archive_position(&conn, "T_MAZDA", season.numero, "production_challenger");

        assert_eq!(bmw_position, 1);
        assert_eq!(mazda_position, 1);
    }

    #[test]
    fn archive_team_season_uses_lmp2_endurance_class() {
        let conn = setup_team_archive_conn();
        conn.execute_batch(
            "
            INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
            VALUES ('S_END', 3, 2026, 'Finalizada', 1, 'BlocoRegular', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

            INSERT INTO drivers (id, nome, idade, nacionalidade, genero, categoria_atual, status, ano_inicio_carreira)
            VALUES
                ('P_GT3', 'Piloto GT3', 25, 'Brasil', 'M', 'endurance', 'Ativo', 2020),
                ('P_LMP', 'Piloto LMP2', 24, 'Brasil', 'M', 'endurance', 'Ativo', 2021);

            INSERT INTO teams (
                id, nome, nome_curto, categoria, classe, ativa, piloto_1_id,
                created_at, updated_at
            ) VALUES
                ('T_GT3', 'Equipe GT3', 'GT3', 'endurance', 'gt3', 1, 'P_GT3', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
                ('T_LMP', 'Equipe LMP2', 'LMP', 'endurance', 'lmp2', 1, 'P_LMP', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

            INSERT INTO calendar (
                id, temporada_id, season_id, rodada, pista, categoria, status, nome,
                track_name, track_config
            ) VALUES
                ('R_END', 'S_END', 'S_END', 1, 'Le Mans', 'endurance', 'Concluida', 'Endurance', 'Le Mans', 'default');

            INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final, voltas_completadas, pontos
            ) VALUES
                ('R_END', 'P_GT3', 'T_GT3', 1, 1, 10, 35.0),
                ('R_END', 'P_LMP', 'T_LMP', 2, 1, 10, 35.0);
            ",
        )
        .expect("seed endurance world");
        let season = Season::new("S_END".to_string(), 3, 2026);

        archive_team_season(&conn, &season).expect("archive");

        let lmp_classe: Option<String> = conn
            .query_row(
                "SELECT classe FROM team_season_archive
                 WHERE team_id = 'T_LMP' AND season_number = 3 AND categoria = 'endurance'",
                [],
                |row| row.get(0),
            )
            .expect("lmp2 archive row");
        assert_eq!(lmp_classe.as_deref(), Some("lmp2"));

        // Cada classe tem o proprio campeao de construtores.
        let lmp_position = read_team_archive_position(&conn, "T_LMP", season.numero, "endurance");
        let gt3_position = read_team_archive_position(&conn, "T_GT3", season.numero, "endurance");
        assert_eq!(lmp_position, 1);
        assert_eq!(gt3_position, 1);
    }

    #[test]
    fn archive_team_season_does_not_require_special_team_entries_for_real_special_categories() {
        let conn = setup_team_archive_conn();
        conn.execute("DROP TABLE special_team_entries", [])
            .expect("drop legacy special entries table");
        conn.execute_batch(
            "
            INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
            VALUES ('S_PROD', 4, 2026, 'Finalizada', 1, 'BlocoRegular', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

            INSERT INTO drivers (id, nome, idade, nacionalidade, genero, categoria_atual, status, ano_inicio_carreira)
            VALUES ('P_PROD', 'Piloto Production', 25, 'Brasil', 'M', 'production_challenger', 'Ativo', 2020);

            INSERT INTO teams (
                id, nome, nome_curto, categoria, classe, ativa, piloto_1_id,
                created_at, updated_at
            ) VALUES (
                'T_PROD', 'Equipe Production', 'PRD', 'production_challenger', 'mazda',
                1, 'P_PROD', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
            );

            INSERT INTO calendar (
                id, temporada_id, season_id, rodada, pista, categoria, status, nome,
                track_name, track_config
            ) VALUES (
                'R_PROD', 'S_PROD', 'S_PROD', 1, 'Interlagos', 'production_challenger',
                'Concluida', 'Production', 'Interlagos', 'default'
            );

            INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final, voltas_completadas, pontos
            ) VALUES ('R_PROD', 'P_PROD', 'T_PROD', 1, 1, 10, 35.0);
            ",
        )
        .expect("seed production archive world");
        let season = Season::new("S_PROD".to_string(), 4, 2026);

        archive_team_season(&conn, &season).expect("archive without special_team_entries");

        let class_name: Option<String> = conn
            .query_row(
                "SELECT classe FROM team_season_archive
                 WHERE team_id = 'T_PROD'
                   AND season_number = 4
                   AND categoria = 'production_challenger'",
                [],
                |row| row.get(0),
            )
            .expect("production archive row");
        assert_eq!(class_name.as_deref(), Some("mazda"));
    }

    #[test]
    fn roll_up_consolidates_constructor_and_driver_history_into_team_record() {
        let conn = setup_team_archive_conn();
        let season = seed_completed_team_archive_world(&conn);
        archive_team_season(&conn, &season).expect("archive");

        // Título de PILOTO: P001 campeão da rookie pela T001 (team_id no snapshot).
        conn.execute(
            "INSERT INTO driver_season_archive
                (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
             VALUES ('P001', ?1, 2024, 'Piloto Um', 'mazda_rookie', 1, 100,
                     json_object('team_id', 'T001', 'titulos', 1))",
            params![season.numero],
        )
        .expect("driver archive row");

        roll_up_team_career_history(&conn).expect("roll-up");

        let hist = read_team_history(&conn, "T001");
        // vitorias, podios, poles, pontos, titulos_construtores, titulos_pilotos
        assert_eq!(hist, [1, 2, 1, 100, 1, 1], "roll-up consolidou o histórico");
    }

    #[test]
    fn roll_up_is_idempotent_and_does_not_double_count() {
        let conn = setup_team_archive_conn();
        let season = seed_completed_team_archive_world(&conn);
        archive_team_season(&conn, &season).expect("archive");

        roll_up_team_career_history(&conn).expect("first roll-up");
        roll_up_team_career_history(&conn).expect("second roll-up");

        let hist = read_team_history(&conn, "T001");
        // Recompute-from-archive: rodar 2× NÃO dobra os totais.
        assert_eq!(hist, [1, 2, 1, 100, 1, 0], "sem dupla contagem");
    }

    fn read_team_history(conn: &Connection, team_id: &str) -> [i64; 6] {
        conn.query_row(
            "SELECT historico_vitorias, historico_podios, historico_poles,
                    historico_pontos, carreira_titulos, historico_titulos_pilotos
             FROM teams WHERE id = ?1",
            params![team_id],
            |row| {
                Ok([
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ])
            },
        )
        .expect("team history row")
    }

    fn setup_team_archive_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        conn
    }

    fn seed_completed_team_archive_world(conn: &Connection) -> Season {
        conn.execute_batch(
            "
            INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
            VALUES ('S001', 1, 2024, 'Finalizada', 1, 'BlocoRegular', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

            INSERT INTO drivers (id, nome, idade, nacionalidade, genero, categoria_atual, status, ano_inicio_carreira)
            VALUES
                ('P001', 'Piloto Um', 25, 'Brasil', 'M', 'mazda_rookie', 'Ativo', 2020),
                ('P002', 'Piloto Dois', 24, 'Brasil', 'M', 'mazda_rookie', 'Ativo', 2021);

            INSERT INTO teams (
                id, nome, nome_curto, categoria, ativa, piloto_1_id, piloto_2_id,
                created_at, updated_at
            ) VALUES (
                'T001', 'Equipe Um', 'E1', 'mazda_rookie', 1, 'P001', 'P002',
                CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
            );

            INSERT INTO calendar (
                id, temporada_id, season_id, rodada, pista, categoria, status, nome,
                track_name, track_config
            ) VALUES (
                'R001', 'S001', 'S001', 1, 'Laguna Seca', 'mazda_rookie', 'Concluida',
                'R1', 'Laguna Seca', 'default'
            );

            INSERT INTO races (id, temporada_id, calendar_id, rodada, pista, data, clima, status)
            VALUES ('R001', 'S001', 'R001', 1, 'Laguna Seca', '2024-01-01', 'Seco', 'Concluida');

            INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final, voltas_completadas, pontos
            ) VALUES
                ('R001', 'P001', 'T001', 1, 1, 10, 60.0),
                ('R001', 'P002', 'T001', 2, 2, 10, 40.0);
            ",
        )
        .expect("seed world");

        Season::new("S001".to_string(), 1, 2024)
    }

    fn count_team_archive_rows(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM team_season_archive", [], |row| {
            row.get(0)
        })
        .expect("archive count")
    }

    fn read_team_archive_points(
        conn: &Connection,
        team_id: &str,
        season_number: i32,
        category: &str,
    ) -> f64 {
        conn.query_row(
            "SELECT pontos
             FROM team_season_archive
             WHERE team_id = ?1 AND season_number = ?2 AND categoria = ?3",
            params![team_id, season_number, category],
            |row| row.get(0),
        )
        .expect("archive points")
    }

    fn read_team_archive_position(
        conn: &Connection,
        team_id: &str,
        season_number: i32,
        category: &str,
    ) -> i32 {
        conn.query_row(
            "SELECT posicao_campeonato
             FROM team_season_archive
             WHERE team_id = ?1 AND season_number = ?2 AND categoria = ?3",
            params![team_id, season_number, category],
            |row| row.get(0),
        )
        .expect("archive position")
    }
}
