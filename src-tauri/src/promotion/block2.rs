#![allow(dead_code)]

use std::collections::HashSet;

use rand::Rng;
use rusqlite::Connection;

use crate::promotion::TeamMovement;

pub fn execute_block2(conn: &Connection, rng: &mut impl Rng) -> Result<Vec<TeamMovement>, String> {
    execute_block2_with_exclusions(conn, &HashSet::new(), rng)
}

pub(crate) fn execute_block2_with_exclusions(
    _conn: &Connection,
    _excluded_team_ids: &HashSet<String>,
    _rng: &mut impl Rng,
) -> Result<Vec<TeamMovement>, String> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rand::{rngs::StdRng, SeedableRng};
    use rusqlite::Connection;

    use super::*;
    use crate::db::migrations;
    use crate::db::queries::teams as team_queries;
    use crate::models::team::Team;

    #[test]
    fn test_block2_does_not_move_teams_into_or_out_of_special_category() {
        let conn = setup_block2_db();
        let mut rng = StdRng::seed_from_u64(20);
        let excluded = HashSet::from(["MA10".to_string()]);

        let movements =
            execute_block2_with_exclusions(&conn, &excluded, &mut rng).expect("block2 should run");

        assert!(
            movements.is_empty(),
            "Production Challenger entries are now earned through special_team_entries, not category moves"
        );
    }

    fn setup_block2_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        insert_ranked_teams(&conn, "mazda_amador", "MA", 10, None);
        insert_ranked_teams(&conn, "toyota_amador", "TA", 10, None);
        insert_ranked_teams(&conn, "bmw_m2", "BM", 10, None);
        insert_ranked_teams(&conn, "production_challenger", "PM", 5, Some("mazda"));
        insert_ranked_teams(&conn, "production_challenger", "PT", 5, Some("toyota"));
        insert_ranked_teams(&conn, "production_challenger", "PB", 5, Some("bmw"));

        conn
    }

    fn insert_ranked_teams(
        conn: &Connection,
        category: &str,
        prefix: &str,
        count: usize,
        class: Option<&str>,
    ) {
        for index in 0..count {
            let rank = index + 1;
            let mut team = sample_team(
                category,
                &format!("{prefix}{rank}"),
                &format!("{prefix} Team {rank}"),
                class,
            );
            team.stats_pontos = ((count - index) * 10) as i32;
            team.stats_vitorias = (count - index) as i32;
            team.stats_melhor_resultado = rank as i32;
            team_queries::insert_team(conn, &team).expect("insert ranked team");
        }
    }

    fn sample_team(category: &str, id: &str, name: &str, class: Option<&str>) -> Team {
        let template = crate::constants::teams::get_reference_team_template(category, class)
            .expect("team template");
        let mut rng = StdRng::seed_from_u64(id.bytes().map(u64::from).sum());
        let mut team =
            Team::from_template_with_rng(template, category, id.to_string(), 2025, &mut rng);
        team.nome = name.to_string();
        team.nome_curto = name.to_string();
        team.classe = class.map(str::to_string);
        team
    }
}
