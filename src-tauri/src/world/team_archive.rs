use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::models::season::Season;

#[derive(Debug, Clone)]
struct TeamSeasonSnapshot {
    team_id: String,
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
                COALESCE(NULLIF(TRIM(t.classe), ''), e.class_name) AS classe,
                t.piloto_1_id,
                t.piloto_2_id,
                COALESCE(SUM(rr.pontos), 0.0) AS pontos,
                COALESCE(SUM(CASE WHEN rr.posicao_final = 1 THEN 1 ELSE 0 END), 0) AS vitorias,
                COALESCE(SUM(CASE WHEN rr.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END), 0) AS podios,
                COALESCE(SUM(CASE WHEN rr.posicao_largada = 1 THEN 1 ELSE 0 END), 0) AS poles,
                COUNT(DISTINCT rr.race_id) AS corridas
             FROM race_results rr
             JOIN calendar c ON c.id = rr.race_id
             JOIN teams t ON t.id = rr.equipe_id
             LEFT JOIN special_team_entries e
                ON e.team_id = rr.equipe_id
               AND e.season_id = COALESCE(c.season_id, c.temporada_id)
               AND e.special_category = c.categoria
             WHERE COALESCE(c.season_id, c.temporada_id) = ?1
             GROUP BY rr.equipe_id, c.categoria, COALESCE(NULLIF(TRIM(t.classe), ''), e.class_name)",
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
        ) = row.map_err(|e| format!("Falha ao mapear arquivo de equipes: {e}"))?;

        snapshots.push(TeamSeasonSnapshot {
            team_id,
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
                .then_with(|| snapshots[*left].team_id.cmp(&snapshots[*right].team_id))
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
