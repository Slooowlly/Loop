use std::collections::{HashMap, HashSet};

use rand::Rng;
use rusqlite::Connection;

use crate::constants::historical_timeline::is_category_active_in_year;
use crate::promotion::standings::{
    calculate_constructor_standings, calculate_constructor_standings_by_class,
    calculate_special_team_standings_by_class, ClassTeamStanding,
};
use crate::promotion::{MovementType, TeamMovement};

/// Pares estruturais Amador ↔ Production: cada classe da Production troca uma
/// equipe por temporada com a própria categoria de origem.
const PRODUCTION_PAIRS: [(&str, &str); 3] = [
    ("mazda_amador", "mazda"),
    ("toyota_amador", "toyota"),
    ("bmw_m2", "bmw"),
];

#[cfg(test)]
pub fn execute_block2(
    conn: &Connection,
    season_number: i32,
    rng: &mut impl Rng,
) -> Result<Vec<TeamMovement>, String> {
    execute_block2_with_exclusions(conn, season_number, i32::MAX, &HashSet::new(), rng)
}

pub(crate) fn execute_block2_with_exclusions(
    conn: &Connection,
    season_number: i32,
    year: i32,
    excluded_team_ids: &HashSet<String>,
    _rng: &mut impl Rng,
) -> Result<Vec<TeamMovement>, String> {
    let mut movements = Vec::new();
    if !is_category_active_in_year("production_challenger", year) {
        return Ok(movements);
    }

    // Standings oficiais da Production vêm dos resultados de corrida da
    // temporada (pontuação por classe da Fase 3B). teams.stats_pontos não é
    // alimentado para categorias especiais.
    let production_by_class = load_production_standings_by_class(conn, season_number)?;

    for (amateur_category, class_name) in PRODUCTION_PAIRS {
        if !is_category_active_in_year(amateur_category, year) {
            continue;
        }
        append_pair_movements(
            &mut movements,
            conn,
            amateur_category,
            class_name,
            &production_by_class,
            excluded_team_ids,
        )?;
    }

    Ok(movements)
}

fn load_production_standings_by_class(
    conn: &Connection,
    season_number: i32,
) -> Result<HashMap<String, Vec<ClassTeamStanding>>, String> {
    let season_id: Option<String> = conn
        .query_row(
            "SELECT id FROM seasons WHERE numero = ?1",
            rusqlite::params![season_number],
            |row| row.get(0),
        )
        .map(Some)
        .or_else(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(format!(
                "Falha ao buscar temporada {season_number}: {other}"
            )),
        })?;

    let Some(season_id) = season_id else {
        return Ok(HashMap::new());
    };

    calculate_special_team_standings_by_class(conn, &season_id, "production_challenger")
}

fn append_pair_movements(
    movements: &mut Vec<TeamMovement>,
    conn: &Connection,
    amateur_category: &str,
    class_name: &str,
    production_by_class: &HashMap<String, Vec<ClassTeamStanding>>,
    excluded_team_ids: &HashSet<String>,
) -> Result<(), String> {
    let amateur_standings = calculate_constructor_standings(conn, amateur_category)?;
    let promoted = amateur_standings
        .iter()
        .find(|standing| !excluded_team_ids.contains(&standing.team_id));
    let relegated = find_production_last_place(conn, class_name, production_by_class)?;

    // O par só se move completo: exatamente uma equipe sobe e uma desce,
    // preservando os tamanhos de Amador e Production.
    let (Some(promoted), Some(relegated)) = (promoted, relegated) else {
        return Ok(());
    };

    movements.push(TeamMovement {
        team_id: promoted.team_id.clone(),
        team_name: promoted.team_name.clone(),
        from_category: amateur_category.to_string(),
        to_category: "production_challenger".to_string(),
        movement_type: MovementType::Promocao,
        reason: format!("Campea de construtores do {amateur_category}"),
    });
    movements.push(TeamMovement {
        team_id: relegated.0,
        team_name: relegated.1,
        from_category: "production_challenger".to_string(),
        to_category: amateur_category.to_string(),
        movement_type: MovementType::Rebaixamento,
        reason: format!("Ultima colocada da classe {class_name} na Production"),
    });

    Ok(())
}

/// Último colocado da classe na Production. Preferência pelos standings de
/// corrida da temporada; sem resultados (fixtures/mundos recém-criados), cai
/// para o ranking por stats da classe.
fn find_production_last_place(
    conn: &Connection,
    class_name: &str,
    production_by_class: &HashMap<String, Vec<ClassTeamStanding>>,
) -> Result<Option<(String, String)>, String> {
    if let Some(entries) = production_by_class.get(class_name) {
        if let Some(last) = entries.last() {
            return Ok(Some((last.team_id.clone(), last.team_name.clone())));
        }
    }

    let fallback =
        calculate_constructor_standings_by_class(conn, "production_challenger", class_name)?;
    Ok(fallback
        .last()
        .map(|standing| (standing.team_id.clone(), standing.team_name.clone())))
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
    fn test_block2_swaps_amateur_champion_with_production_last_per_class() {
        let conn = setup_block2_db();
        let mut rng = StdRng::seed_from_u64(20);

        let movements = execute_block2(&conn, 1, &mut rng).expect("block2 should run");

        assert_eq!(movements.len(), 6);
        for (amateur, prefix_up, prefix_down) in [
            ("mazda_amador", "MA1", "PM6"),
            ("toyota_amador", "TA1", "PT6"),
            ("bmw_m2", "BM1", "PB6"),
        ] {
            assert!(movements.iter().any(|movement| {
                movement.team_id == prefix_up
                    && movement.from_category == amateur
                    && movement.to_category == "production_challenger"
                    && movement.movement_type == MovementType::Promocao
            }));
            assert!(movements.iter().any(|movement| {
                movement.team_id == prefix_down
                    && movement.from_category == "production_challenger"
                    && movement.to_category == amateur
                    && movement.movement_type == MovementType::Rebaixamento
            }));
        }
    }

    #[test]
    fn test_block2_prefers_race_results_standings_for_production_last() {
        let conn = setup_block2_db();
        seed_production_race_results(&conn);
        let mut rng = StdRng::seed_from_u64(21);

        let movements = execute_block2(&conn, 1, &mut rng).expect("block2 should run");

        // Nos resultados de corrida, PM1 (melhor por stats) terminou em último
        // na classe mazda — é ela que desce, não a última por stats.
        assert!(movements.iter().any(|movement| {
            movement.team_id == "PM1"
                && movement.to_category == "mazda_amador"
                && movement.movement_type == MovementType::Rebaixamento
        }));
        assert!(!movements.iter().any(|movement| movement.team_id == "PM6"
            && movement.movement_type == MovementType::Rebaixamento));
    }

    #[test]
    fn test_block2_skips_excluded_amateur_champion() {
        let conn = setup_block2_db();
        let mut rng = StdRng::seed_from_u64(22);
        let excluded = HashSet::from(["MA1".to_string()]);

        let movements = execute_block2_with_exclusions(&conn, 1, i32::MAX, &excluded, &mut rng)
            .expect("block2 should run");

        assert!(movements.iter().any(|movement| {
            movement.team_id == "MA2" && movement.movement_type == MovementType::Promocao
        }));
        assert!(!movements.iter().any(|movement| movement.team_id == "MA1"));
    }

    #[test]
    fn test_block2_inactive_before_production_inauguration() {
        let conn = setup_block2_db();
        let mut rng = StdRng::seed_from_u64(23);

        let movements = execute_block2_with_exclusions(&conn, 1, 2017, &HashSet::new(), &mut rng)
            .expect("block2 timeline should run");

        assert!(movements.is_empty());
    }

    #[test]
    fn test_block2_does_not_touch_endurance_or_lmp2() {
        let conn = setup_block2_db();
        insert_ranked_teams(&conn, "endurance", "EG4", 6, Some("gt4"));
        insert_ranked_teams(&conn, "endurance", "EG3", 6, Some("gt3"));
        insert_ranked_teams(&conn, "endurance", "LMP", 6, Some("lmp2"));
        let mut rng = StdRng::seed_from_u64(24);

        let movements = execute_block2(&conn, 1, &mut rng).expect("block2 should run");

        assert!(movements
            .iter()
            .all(|movement| movement.from_category != "endurance"
                && movement.to_category != "endurance"
                && movement.from_category != "lmp2"
                && movement.to_category != "lmp2"));
    }

    fn setup_block2_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        insert_ranked_teams(&conn, "mazda_amador", "MA", 10, None);
        insert_ranked_teams(&conn, "toyota_amador", "TA", 10, None);
        insert_ranked_teams(&conn, "bmw_m2", "BM", 10, None);
        insert_ranked_teams(&conn, "production_challenger", "PM", 6, Some("mazda"));
        insert_ranked_teams(&conn, "production_challenger", "PT", 6, Some("toyota"));
        insert_ranked_teams(&conn, "production_challenger", "PB", 6, Some("bmw"));

        conn
    }

    fn seed_production_race_results(conn: &Connection) {
        conn.execute_batch(
            "
            INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
            VALUES ('S001', 1, 2025, 'Finalizada', 1, 'BlocoEspecial', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

            INSERT INTO drivers (id, nome, idade, nacionalidade, genero, categoria_atual, status, ano_inicio_carreira)
            VALUES
                ('D_PM1', 'Piloto PM1', 25, 'Brasil', 'M', 'production_challenger', 'Ativo', 2020),
                ('D_PM6', 'Piloto PM6', 25, 'Brasil', 'M', 'production_challenger', 'Ativo', 2020);

            INSERT INTO calendar (
                id, temporada_id, season_id, rodada, pista, categoria, status, nome,
                track_name, track_config
            ) VALUES ('R_PROD', 'S001', 'S001', 1, 'Interlagos', 'production_challenger',
                'Concluida', 'Prod R1', 'Interlagos', 'default');

            INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final, voltas_completadas, pontos
            ) VALUES
                ('R_PROD', 'D_PM6', 'PM6', 1, 1, 10, 35.0),
                ('R_PROD', 'D_PM1', 'PM1', 2, 2, 10, 28.0);
            ",
        )
        .expect("seed production race results");
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
