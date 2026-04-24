#![allow(dead_code)]

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

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        conn
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
