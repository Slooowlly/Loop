use rand::Rng;
use rusqlite::Connection;

use crate::promotion::TeamMovement;

pub fn execute_block3(
    _conn: &Connection,
    _rng: &mut impl Rng,
) -> Result<Vec<TeamMovement>, String> {
    Ok(Vec::new())
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
    fn test_block3_does_not_move_teams_into_or_out_of_endurance() {
        let conn = setup_block3_db();
        let mut rng = StdRng::seed_from_u64(30);

        let movements = execute_block3(&conn, &mut rng).expect("block3 should run");

        assert!(
            movements.is_empty(),
            "Endurance entries are now earned through special_team_entries, not category moves"
        );
    }

    fn setup_block3_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        insert_ranked_teams(&conn, "gt4", "GT4", 10, None);
        insert_ranked_teams(&conn, "gt3", "GT3", 14, None);
        insert_ranked_teams(&conn, "endurance", "EG4", 6, Some("gt4"));
        insert_ranked_teams(&conn, "endurance", "EG3", 6, Some("gt3"));
        insert_ranked_teams(&conn, "endurance", "LMP", 5, Some("lmp2"));

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
