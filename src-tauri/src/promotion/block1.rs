use rand::Rng;
use rusqlite::Connection;

use crate::constants::categories::get_category_config;
use crate::constants::historical_timeline::is_category_active_in_year;
use crate::promotion::standings::calculate_constructor_standings;
use crate::promotion::{MovementType, TeamMovement};

pub fn execute_block1(conn: &Connection, _rng: &mut impl Rng) -> Result<Vec<TeamMovement>, String> {
    execute_block1_for_year(conn, i32::MAX, _rng)
}

pub(crate) fn execute_block1_for_year(
    conn: &Connection,
    year: i32,
    _rng: &mut impl Rng,
) -> Result<Vec<TeamMovement>, String> {
    let mut movements = Vec::new();
    append_pair_movements(
        &mut movements,
        conn,
        year,
        "mazda_rookie",
        "mazda_amador",
        "Campea de construtores do Rookie",
    )?;
    append_pair_movements(
        &mut movements,
        conn,
        year,
        "toyota_rookie",
        "toyota_amador",
        "Campea de construtores do Rookie",
    )?;
    Ok(movements)
}

fn append_pair_movements(
    movements: &mut Vec<TeamMovement>,
    conn: &Connection,
    year: i32,
    rookie_category: &str,
    amateur_category: &str,
    promotion_reason: &str,
) -> Result<(), String> {
    if !is_category_active_in_year(rookie_category, year)
        || !is_category_active_in_year(amateur_category, year)
    {
        return Ok(());
    }

    let rookie_standings = calculate_constructor_standings(conn, rookie_category)?;
    let amateur_standings = calculate_constructor_standings(conn, amateur_category)?;
    let promoted = rookie_standings
        .first()
        .ok_or_else(|| format!("Sem equipes em '{rookie_category}' para promover"))?;
    let amateur_capacity = get_category_config(amateur_category)
        .map(|config| config.num_equipes as usize)
        .unwrap_or(amateur_standings.len());

    movements.push(TeamMovement {
        team_id: promoted.team_id.clone(),
        team_name: promoted.team_name.clone(),
        from_category: rookie_category.to_string(),
        to_category: amateur_category.to_string(),
        movement_type: MovementType::Promocao,
        reason: promotion_reason.to_string(),
    });
    if amateur_standings.len() >= amateur_capacity {
        let relegated = amateur_standings
            .last()
            .ok_or_else(|| format!("Sem equipes em '{amateur_category}' para rebaixar"))?;
        movements.push(TeamMovement {
            team_id: relegated.team_id.clone(),
            team_name: relegated.team_name.clone(),
            from_category: amateur_category.to_string(),
            to_category: rookie_category.to_string(),
            movement_type: MovementType::Rebaixamento,
            reason: "Ultima colocada no campeonato de construtores".to_string(),
        });
    }

    Ok(())
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
    fn test_block1_champion_promoted() {
        let conn = setup_block1_db();
        let mut rng = StdRng::seed_from_u64(10);

        let movements = execute_block1(&conn, &mut rng).expect("block1 should run");

        assert!(movements.iter().any(|movement| {
            movement.team_id == "MR1"
                && movement.from_category == "mazda_rookie"
                && movement.to_category == "mazda_amador"
                && movement.movement_type == MovementType::Promocao
        }));
        assert!(movements.iter().any(|movement| {
            movement.team_id == "TR1"
                && movement.from_category == "toyota_rookie"
                && movement.to_category == "toyota_amador"
                && movement.movement_type == MovementType::Promocao
        }));
    }

    #[test]
    fn test_block1_last_relegated() {
        let conn = setup_block1_db();
        let mut rng = StdRng::seed_from_u64(11);

        let movements = execute_block1(&conn, &mut rng).expect("block1 should run");

        assert!(movements.iter().any(|movement| {
            movement.team_id == "MA10"
                && movement.from_category == "mazda_amador"
                && movement.to_category == "mazda_rookie"
                && movement.movement_type == MovementType::Rebaixamento
        }));
        assert!(movements.iter().any(|movement| {
            movement.team_id == "TA10"
                && movement.from_category == "toyota_amador"
                && movement.to_category == "toyota_rookie"
                && movement.movement_type == MovementType::Rebaixamento
        }));
    }

    #[test]
    fn test_block1_sizes_preserved() {
        let conn = setup_block1_db();
        let mut rng = StdRng::seed_from_u64(12);

        let movements = execute_block1(&conn, &mut rng).expect("block1 should run");

        let mazda_rookie_out = movements
            .iter()
            .filter(|movement| movement.from_category == "mazda_rookie")
            .count();
        let mazda_rookie_in = movements
            .iter()
            .filter(|movement| movement.to_category == "mazda_rookie")
            .count();
        let toyota_rookie_out = movements
            .iter()
            .filter(|movement| movement.from_category == "toyota_rookie")
            .count();
        let toyota_rookie_in = movements
            .iter()
            .filter(|movement| movement.to_category == "toyota_rookie")
            .count();

        assert_eq!(mazda_rookie_out, 1);
        assert_eq!(mazda_rookie_in, 1);
        assert_eq!(toyota_rookie_out, 1);
        assert_eq!(toyota_rookie_in, 1);
    }

    #[test]
    fn test_block1_expands_amateur_grid_before_relegating() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        insert_ranked_teams(&conn, "mazda_rookie", "MR", 10, None);
        insert_ranked_teams(&conn, "mazda_amador", "MA", 6, None);
        insert_ranked_teams(&conn, "toyota_rookie", "TR", 10, None);
        insert_ranked_teams(&conn, "toyota_amador", "TA", 6, None);
        let mut rng = StdRng::seed_from_u64(1212);

        let movements =
            execute_block1_for_year(&conn, 2016, &mut rng).expect("block1 timeline should run");

        assert!(movements.iter().any(|movement| {
            movement.team_id == "MR1"
                && movement.from_category == "mazda_rookie"
                && movement.to_category == "mazda_amador"
                && movement.movement_type == MovementType::Promocao
        }));
        assert!(movements
            .iter()
            .all(|movement| movement.from_category != "mazda_amador"));
    }

    #[test]
    fn test_block1_does_not_relegate_into_rookie_before_rookie_inauguration() {
        let conn = setup_block1_db();
        let mut rng = StdRng::seed_from_u64(13);

        let movements =
            execute_block1_for_year(&conn, 2007, &mut rng).expect("block1 timeline should run");

        assert!(movements
            .iter()
            .all(|movement| movement.to_category != "mazda_rookie"));
        assert!(movements
            .iter()
            .all(|movement| movement.to_category != "toyota_rookie"));
    }

    fn setup_block1_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        insert_ranked_teams(&conn, "mazda_rookie", "MR", 6, None);
        insert_ranked_teams(&conn, "toyota_rookie", "TR", 6, None);
        insert_ranked_teams(&conn, "mazda_amador", "MA", 10, None);
        insert_ranked_teams(&conn, "toyota_amador", "TA", 10, None);

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
