#![allow(dead_code)]

use std::collections::HashMap;

use rusqlite::Connection;

use crate::db::queries::teams as team_queries;

#[derive(Debug, Clone)]
pub struct ConstructorStanding {
    pub team_id: String,
    pub team_name: String,
    pub categoria: String,
    pub classe: Option<String>,
    pub pontos: i32,
    pub vitorias: i32,
    pub melhor_resultado: i32,
    pub posicao: i32,
}

pub fn calculate_constructor_standings(
    conn: &Connection,
    categoria: &str,
) -> Result<Vec<ConstructorStanding>, String> {
    let teams = team_queries::get_teams_by_category(conn, categoria)
        .map_err(|e| format!("Falha ao buscar equipes de '{categoria}': {e}"))?;
    Ok(build_standings(teams, categoria, None))
}

pub fn calculate_constructor_standings_by_class(
    conn: &Connection,
    categoria: &str,
    classe: &str,
) -> Result<Vec<ConstructorStanding>, String> {
    let teams = team_queries::get_teams_by_category(conn, categoria)
        .map_err(|e| format!("Falha ao buscar equipes de '{categoria}': {e}"))?;
    Ok(build_standings(teams, categoria, Some(classe)))
}

fn build_standings(
    teams: Vec<crate::models::team::Team>,
    categoria: &str,
    class_filter: Option<&str>,
) -> Vec<ConstructorStanding> {
    let mut standings: Vec<ConstructorStanding> = teams
        .into_iter()
        .filter(|team| {
            class_filter.is_none_or(|class_name| team.classe.as_deref() == Some(class_name))
        })
        .map(|team| ConstructorStanding {
            team_id: team.id,
            team_name: team.nome,
            categoria: categoria.to_string(),
            classe: team.classe,
            pontos: team.stats_pontos,
            vitorias: team.stats_vitorias,
            melhor_resultado: team.stats_melhor_resultado,
            posicao: 0,
        })
        .collect();

    standings.sort_by(|a, b| {
        b.pontos
            .cmp(&a.pontos)
            .then_with(|| b.vitorias.cmp(&a.vitorias))
            .then_with(|| a.melhor_resultado.cmp(&b.melhor_resultado))
            .then_with(|| a.team_name.cmp(&b.team_name))
    });

    for (index, standing) in standings.iter_mut().enumerate() {
        standing.posicao = index as i32 + 1;
    }

    standings
}

// ── Standings por classe (Production/Endurance) ──────────────────────────────
//
// O campeonato das categorias especiais é decidido por classe (mazda/toyota/bmw
// e gt4/gt3/lmp2). Os pontos vêm dos resultados oficiais das corridas da
// categoria — não de teams.stats_pontos nem de drivers.stats_temporada, que em
// lmp2 misturam o campeonato regular com a Endurance. O campeão de cada classe
// é o primeiro do Vec; o rebaixado, o último.

#[derive(Debug, Clone)]
pub struct ClassTeamStanding {
    pub team_id: String,
    pub team_name: String,
    pub categoria: String,
    pub classe: String,
    pub pontos: f64,
    pub vitorias: i32,
    pub melhor_resultado: i32,
    pub posicao: i32,
}

#[derive(Debug, Clone)]
pub struct ClassDriverStanding {
    pub driver_id: String,
    pub driver_name: String,
    pub team_id: String,
    pub categoria: String,
    pub classe: String,
    pub pontos: f64,
    pub vitorias: i32,
    pub posicao: i32,
}

pub fn calculate_special_team_standings_by_class(
    conn: &Connection,
    season_id: &str,
    categoria: &str,
) -> Result<HashMap<String, Vec<ClassTeamStanding>>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                rr.equipe_id,
                t.nome,
                NULLIF(TRIM(t.classe), '') AS classe,
                COALESCE(SUM(rr.pontos), 0.0) AS pontos,
                SUM(CASE WHEN rr.dnf = 0 AND rr.posicao_final = 1 THEN 1 ELSE 0 END) AS vitorias,
                MIN(CASE WHEN rr.dnf = 0 THEN rr.posicao_final END) AS melhor_resultado
             FROM race_results rr
             JOIN calendar c ON c.id = rr.race_id
             JOIN teams t ON t.id = rr.equipe_id
             WHERE COALESCE(c.season_id, c.temporada_id) = ?1
               AND c.categoria = ?2
               AND NULLIF(TRIM(t.classe), '') IS NOT NULL
             GROUP BY rr.equipe_id, NULLIF(TRIM(t.classe), '')",
        )
        .map_err(|e| format!("Falha ao preparar standings de equipes por classe: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![season_id, categoria], |row| {
            Ok(ClassTeamStanding {
                team_id: row.get(0)?,
                team_name: row.get(1)?,
                categoria: categoria.to_string(),
                classe: row.get(2)?,
                pontos: row.get(3)?,
                vitorias: row.get(4)?,
                melhor_resultado: row.get::<_, Option<i32>>(5)?.unwrap_or(99),
                posicao: 0,
            })
        })
        .map_err(|e| format!("Falha ao consultar standings de equipes por classe: {e}"))?;

    let mut by_class: HashMap<String, Vec<ClassTeamStanding>> = HashMap::new();
    for row in rows {
        let row = row.map_err(|e| format!("Falha ao mapear standings de equipes: {e}"))?;
        by_class.entry(row.classe.clone()).or_default().push(row);
    }

    for entries in by_class.values_mut() {
        entries.sort_by(|a, b| {
            b.pontos
                .total_cmp(&a.pontos)
                .then_with(|| b.vitorias.cmp(&a.vitorias))
                .then_with(|| a.melhor_resultado.cmp(&b.melhor_resultado))
                .then_with(|| a.team_name.cmp(&b.team_name))
        });
        for (index, entry) in entries.iter_mut().enumerate() {
            entry.posicao = index as i32 + 1;
        }
    }

    Ok(by_class)
}

pub fn calculate_special_driver_standings_by_class(
    conn: &Connection,
    season_id: &str,
    categoria: &str,
) -> Result<HashMap<String, Vec<ClassDriverStanding>>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                rr.piloto_id,
                d.nome,
                MAX(rr.equipe_id) AS equipe_id,
                NULLIF(TRIM(t.classe), '') AS classe,
                COALESCE(SUM(rr.pontos), 0.0) AS pontos,
                SUM(CASE WHEN rr.dnf = 0 AND rr.posicao_final = 1 THEN 1 ELSE 0 END) AS vitorias
             FROM race_results rr
             JOIN calendar c ON c.id = rr.race_id
             JOIN drivers d ON d.id = rr.piloto_id
             JOIN teams t ON t.id = rr.equipe_id
             WHERE COALESCE(c.season_id, c.temporada_id) = ?1
               AND c.categoria = ?2
               AND NULLIF(TRIM(t.classe), '') IS NOT NULL
             GROUP BY rr.piloto_id, NULLIF(TRIM(t.classe), '')",
        )
        .map_err(|e| format!("Falha ao preparar standings de pilotos por classe: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![season_id, categoria], |row| {
            Ok(ClassDriverStanding {
                driver_id: row.get(0)?,
                driver_name: row.get(1)?,
                team_id: row.get(2)?,
                categoria: categoria.to_string(),
                classe: row.get(3)?,
                pontos: row.get(4)?,
                vitorias: row.get(5)?,
                posicao: 0,
            })
        })
        .map_err(|e| format!("Falha ao consultar standings de pilotos por classe: {e}"))?;

    let mut by_class: HashMap<String, Vec<ClassDriverStanding>> = HashMap::new();
    for row in rows {
        let row = row.map_err(|e| format!("Falha ao mapear standings de pilotos: {e}"))?;
        by_class.entry(row.classe.clone()).or_default().push(row);
    }

    for entries in by_class.values_mut() {
        entries.sort_by(|a, b| {
            b.pontos
                .total_cmp(&a.pontos)
                .then_with(|| b.vitorias.cmp(&a.vitorias))
                .then_with(|| a.driver_name.cmp(&b.driver_name))
        });
        for (index, entry) in entries.iter_mut().enumerate() {
            entry.posicao = index as i32 + 1;
        }
    }

    Ok(by_class)
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};
    use rusqlite::Connection;

    use super::*;
    use crate::db::migrations;
    use crate::db::queries::teams as team_queries;
    use crate::models::team::Team;

    #[test]
    fn test_constructor_standings_ordered_by_points() {
        let conn = setup_db();
        insert_team_with_stats(
            &conn,
            sample_team("gt4", "T001", "Equipe A", None, 120, 3, 1),
        );
        insert_team_with_stats(
            &conn,
            sample_team("gt4", "T002", "Equipe B", None, 90, 5, 1),
        );

        let standings =
            calculate_constructor_standings(&conn, "gt4").expect("standings should load");

        assert_eq!(standings.len(), 2);
        assert_eq!(standings[0].team_id, "T001");
        assert_eq!(standings[0].posicao, 1);
        assert_eq!(standings[1].team_id, "T002");
        assert_eq!(standings[1].posicao, 2);
    }

    #[test]
    fn test_constructor_standings_tiebreak_by_wins() {
        let conn = setup_db();
        insert_team_with_stats(
            &conn,
            sample_team("gt4", "T001", "Equipe A", None, 100, 2, 2),
        );
        insert_team_with_stats(
            &conn,
            sample_team("gt4", "T002", "Equipe B", None, 100, 4, 3),
        );

        let standings =
            calculate_constructor_standings(&conn, "gt4").expect("standings should load");

        assert_eq!(standings[0].team_id, "T002");
        assert_eq!(standings[1].team_id, "T001");
    }

    #[test]
    fn test_constructor_standings_by_class_filters_multi_class() {
        let conn = setup_db();
        insert_team_with_stats(
            &conn,
            sample_team(
                "production_challenger",
                "T001",
                "Mazda Works",
                Some("mazda"),
                110,
                3,
                1,
            ),
        );
        insert_team_with_stats(
            &conn,
            sample_team(
                "production_challenger",
                "T002",
                "Toyota Works",
                Some("toyota"),
                180,
                4,
                1,
            ),
        );
        insert_team_with_stats(
            &conn,
            sample_team(
                "production_challenger",
                "T003",
                "Mazda Junior",
                Some("mazda"),
                95,
                2,
                2,
            ),
        );

        let standings =
            calculate_constructor_standings_by_class(&conn, "production_challenger", "mazda")
                .expect("class standings should load");

        assert_eq!(standings.len(), 2);
        assert!(standings
            .iter()
            .all(|entry| entry.classe.as_deref() == Some("mazda")));
        assert_eq!(standings[0].team_id, "T001");
        assert_eq!(standings[1].team_id, "T003");
    }

    #[test]
    fn test_special_class_standings_split_production_by_class() {
        let conn = setup_db();
        seed_special_race_world(
            &conn,
            "production_challenger",
            &[
                // (race, piloto, equipe, classe da equipe, categoria da equipe, pos, pontos)
                (
                    "R1",
                    "P_MZ1",
                    "T_MZ1",
                    Some("mazda"),
                    "production_challenger",
                    1,
                    35.0,
                ),
                (
                    "R1",
                    "P_MZ2",
                    "T_MZ2",
                    Some("mazda"),
                    "production_challenger",
                    2,
                    28.0,
                ),
                (
                    "R1",
                    "P_TY1",
                    "T_TY1",
                    Some("toyota"),
                    "production_challenger",
                    1,
                    35.0,
                ),
                (
                    "R1",
                    "P_TY2",
                    "T_TY2",
                    Some("toyota"),
                    "production_challenger",
                    2,
                    28.0,
                ),
            ],
        );

        let teams =
            calculate_special_team_standings_by_class(&conn, "S001", "production_challenger")
                .expect("team standings");
        let drivers =
            calculate_special_driver_standings_by_class(&conn, "S001", "production_challenger")
                .expect("driver standings");

        assert_eq!(teams.len(), 2);
        let mazda = &teams["mazda"];
        assert_eq!(mazda.len(), 2);
        assert_eq!(mazda.first().expect("campeao mazda").team_id, "T_MZ1");
        assert_eq!(mazda.last().expect("ultimo mazda").team_id, "T_MZ2");
        assert!(mazda.iter().all(|entry| entry.classe == "mazda"));

        let toyota = &teams["toyota"];
        assert_eq!(toyota.first().expect("campeao toyota").team_id, "T_TY1");
        assert_eq!(toyota.first().expect("campeao toyota").posicao, 1);
        // Equipe mazda nao pontua na classe toyota.
        assert!(toyota
            .iter()
            .all(|entry| !entry.team_id.starts_with("T_MZ")));

        let mazda_drivers = &drivers["mazda"];
        assert_eq!(mazda_drivers[0].driver_id, "P_MZ1");
        assert_eq!(mazda_drivers[0].posicao, 1);
        assert_eq!(mazda_drivers[1].driver_id, "P_MZ2");
        assert_eq!(mazda_drivers[1].posicao, 2);
        assert!(drivers["toyota"]
            .iter()
            .all(|entry| !entry.driver_id.starts_with("P_MZ")));
    }

    #[test]
    fn test_special_class_standings_endurance_includes_lmp2_as_class() {
        let conn = setup_db();
        seed_special_race_world(
            &conn,
            "endurance",
            &[
                ("R1", "P_GT4", "T_GT4", Some("gt4"), "endurance", 1, 35.0),
                ("R1", "P_GT3", "T_GT3", Some("gt3"), "endurance", 1, 35.0),
                ("R1", "P_LMP", "T_LMP", Some("lmp2"), "endurance", 1, 35.0),
            ],
        );

        let teams = calculate_special_team_standings_by_class(&conn, "S001", "endurance")
            .expect("team standings");

        assert_eq!(teams.len(), 3);
        assert!(teams.contains_key("gt4"));
        assert!(teams.contains_key("gt3"));
        let lmp2 = teams.get("lmp2").expect("classe lmp2 presente");
        assert_eq!(lmp2[0].team_id, "T_LMP");
        assert_eq!(lmp2[0].posicao, 1);
        // GT4 nao pontua na classe GT3.
        assert!(teams["gt3"].iter().all(|entry| entry.team_id != "T_GT4"));
    }

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        conn
    }

    fn seed_special_race_world(
        conn: &Connection,
        categoria: &str,
        results: &[(&str, &str, &str, Option<&str>, &str, i32, f64)],
    ) {
        conn.execute(
            "INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
             VALUES ('S001', 1, 2025, 'EmAndamento', 1, 'BlocoEspecial', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [],
        )
        .expect("seed season");

        let mut seeded_races = std::collections::HashSet::new();
        let mut seeded_drivers = std::collections::HashSet::new();
        let mut seeded_teams = std::collections::HashSet::new();
        for (race_id, driver_id, team_id, classe, team_categoria, posicao, pontos) in results {
            if seeded_races.insert(*race_id) {
                conn.execute(
                    "INSERT INTO calendar (
                        id, temporada_id, season_id, rodada, pista, categoria, status, nome,
                        track_name, track_config
                     ) VALUES (?1, 'S001', 'S001', 1, 'Interlagos', ?2, 'Concluida', ?1,
                        'Interlagos', 'default')",
                    rusqlite::params![race_id, categoria],
                )
                .expect("seed calendar");
            }
            if seeded_drivers.insert(*driver_id) {
                conn.execute(
                    "INSERT INTO drivers (
                        id, nome, idade, nacionalidade, genero, categoria_atual, status,
                        ano_inicio_carreira
                     ) VALUES (?1, ?1, 25, 'Brasil', 'M', ?2, 'Ativo', 2020)",
                    rusqlite::params![driver_id, team_categoria],
                )
                .expect("seed driver");
            }
            if seeded_teams.insert(*team_id) {
                conn.execute(
                    "INSERT INTO teams (
                        id, nome, nome_curto, categoria, classe, ativa, created_at, updated_at
                     ) VALUES (?1, ?1, ?1, ?2, ?3, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
                    rusqlite::params![team_id, team_categoria, classe],
                )
                .expect("seed team");
            }
            conn.execute(
                "INSERT INTO race_results (
                    race_id, piloto_id, equipe_id, posicao_largada, posicao_final,
                    voltas_completadas, pontos
                 ) VALUES (?1, ?2, ?3, ?4, ?4, 10, ?5)",
                rusqlite::params![race_id, driver_id, team_id, posicao, pontos],
            )
            .expect("seed race result");
        }
    }

    fn insert_team_with_stats(conn: &Connection, team: Team) {
        team_queries::insert_team(conn, &team).expect("insert team");
    }

    fn sample_team(
        category: &str,
        id: &str,
        name: &str,
        class: Option<&str>,
        points: i32,
        wins: i32,
        best_result: i32,
    ) -> Team {
        let template = crate::constants::teams::get_reference_team_template(category, class)
            .expect("team template");
        let mut rng = StdRng::seed_from_u64(id.bytes().map(u64::from).sum());
        let mut team =
            Team::from_template_with_rng(template, category, id.to_string(), 2025, &mut rng);
        team.nome = name.to_string();
        team.nome_curto = name.to_string();
        team.classe = class.map(str::to_string);
        team.stats_pontos = points;
        team.stats_vitorias = wins;
        team.stats_melhor_resultado = best_result;
        team
    }
}
