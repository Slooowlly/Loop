use std::collections::HashMap;

use rand::Rng;
use rusqlite::Connection;

use crate::constants::historical_timeline::{class_start_year, is_category_active_in_year};
use crate::promotion::standings::{
    calculate_constructor_standings, calculate_constructor_standings_by_class,
    calculate_special_team_standings_by_class, ClassTeamStanding,
};
use crate::promotion::{MovementType, TeamMovement};

/// Pares estruturais GT ↔ Endurance: cada classe GT da Endurance troca uma
/// equipe por temporada com a própria categoria de origem. LMP2 é fixa: não
/// sobe, não desce e não participa do block3.
const ENDURANCE_PAIRS: [(&str, &str); 2] = [("gt4", "gt4"), ("gt3", "gt3")];

#[cfg(test)]
pub fn execute_block3(
    conn: &Connection,
    season_number: i32,
    rng: &mut impl Rng,
) -> Result<Vec<TeamMovement>, String> {
    execute_block3_for_year(conn, season_number, i32::MAX, rng)
}

pub(crate) fn execute_block3_for_year(
    conn: &Connection,
    season_number: i32,
    year: i32,
    _rng: &mut impl Rng,
) -> Result<Vec<TeamMovement>, String> {
    let mut movements = Vec::new();
    if !is_category_active_in_year("endurance", year) {
        return Ok(movements);
    }

    // Standings oficiais da Endurance vêm dos resultados de corrida da
    // temporada (pontuação por classe da Fase 3B). teams.stats_pontos não é
    // alimentado para categorias especiais.
    let endurance_by_class = load_endurance_standings_by_class(conn, season_number)?;

    for (feeder_category, class_name) in ENDURANCE_PAIRS {
        // As classes GT estrearam depois da categoria (gt3 em 2005, gt4 em 2007).
        if year < class_start_year("endurance", Some(class_name))
            || !is_category_active_in_year(feeder_category, year)
        {
            continue;
        }
        append_pair_movements(
            &mut movements,
            conn,
            feeder_category,
            class_name,
            &endurance_by_class,
        )?;
    }

    Ok(movements)
}

fn load_endurance_standings_by_class(
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

    calculate_special_team_standings_by_class(conn, &season_id, "endurance")
}

fn append_pair_movements(
    movements: &mut Vec<TeamMovement>,
    conn: &Connection,
    feeder_category: &str,
    class_name: &str,
    endurance_by_class: &HashMap<String, Vec<ClassTeamStanding>>,
) -> Result<(), String> {
    let feeder_standings = calculate_constructor_standings(conn, feeder_category)?;
    let promoted = feeder_standings.first();
    let relegated = find_endurance_last_place(conn, class_name, endurance_by_class)?;

    // O par só se move completo: exatamente uma equipe sobe e uma desce,
    // preservando os tamanhos das categorias GT e da Endurance.
    let (Some(promoted), Some(relegated)) = (promoted, relegated) else {
        return Ok(());
    };

    movements.push(TeamMovement {
        team_id: promoted.team_id.clone(),
        team_name: promoted.team_name.clone(),
        from_category: feeder_category.to_string(),
        to_category: "endurance".to_string(),
        movement_type: MovementType::Promocao,
        reason: format!("Campea de construtores do {feeder_category}"),
    });
    movements.push(TeamMovement {
        team_id: relegated.0,
        team_name: relegated.1,
        from_category: "endurance".to_string(),
        to_category: feeder_category.to_string(),
        movement_type: MovementType::Rebaixamento,
        reason: format!("Ultima colocada da classe {class_name} na Endurance"),
    });

    Ok(())
}

/// Último colocado da classe na Endurance. Preferência pelos standings de
/// corrida da temporada; sem resultados (fixtures/mundos recém-criados), cai
/// para o ranking por stats da classe.
fn find_endurance_last_place(
    conn: &Connection,
    class_name: &str,
    endurance_by_class: &HashMap<String, Vec<ClassTeamStanding>>,
) -> Result<Option<(String, String)>, String> {
    if let Some(entries) = endurance_by_class.get(class_name) {
        if let Some(last) = entries.last() {
            return Ok(Some((last.team_id.clone(), last.team_name.clone())));
        }
    }

    let fallback = calculate_constructor_standings_by_class(conn, "endurance", class_name)?;
    Ok(fallback
        .last()
        .map(|standing| (standing.team_id.clone(), standing.team_name.clone())))
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
    fn test_block3_swaps_gt_champion_with_endurance_last_per_class() {
        let conn = setup_block3_db();
        let mut rng = StdRng::seed_from_u64(30);

        let movements = execute_block3(&conn, 1, &mut rng).expect("block3 should run");

        assert_eq!(movements.len(), 4);
        for (feeder, prefix_up, prefix_down) in [("gt4", "GT41", "EG46"), ("gt3", "GT31", "EG36")] {
            assert!(movements.iter().any(|movement| {
                movement.team_id == prefix_up
                    && movement.from_category == feeder
                    && movement.to_category == "endurance"
                    && movement.movement_type == MovementType::Promocao
            }));
            assert!(movements.iter().any(|movement| {
                movement.team_id == prefix_down
                    && movement.from_category == "endurance"
                    && movement.to_category == feeder
                    && movement.movement_type == MovementType::Rebaixamento
            }));
        }
    }

    #[test]
    fn test_block3_prefers_race_results_standings_for_endurance_last() {
        let conn = setup_block3_db();
        seed_endurance_race_results(&conn);
        let mut rng = StdRng::seed_from_u64(31);

        let movements = execute_block3(&conn, 1, &mut rng).expect("block3 should run");

        // Nos resultados de corrida, EG41 (melhor por stats) terminou em último
        // na classe gt4 — é ela que desce, não a última por stats.
        assert!(movements.iter().any(|movement| {
            movement.team_id == "EG41"
                && movement.to_category == "gt4"
                && movement.movement_type == MovementType::Rebaixamento
        }));
        assert!(!movements.iter().any(|movement| movement.team_id == "EG46"
            && movement.movement_type == MovementType::Rebaixamento));
    }

    #[test]
    fn test_block3_never_moves_lmp2() {
        let conn = setup_block3_db();
        let mut rng = StdRng::seed_from_u64(32);

        let movements = execute_block3(&conn, 1, &mut rng).expect("block3 should run");

        assert!(movements
            .iter()
            .all(|movement| movement.from_category != "lmp2"
                && movement.to_category != "lmp2"
                && !movement.team_id.starts_with("LMP")));
        let lmp2_count = team_queries::count_teams_by_category(&conn, "lmp2").expect("lmp2 count");
        let endurance_lmp2_count =
            team_queries::count_teams_by_category_and_class(&conn, "endurance", "lmp2")
                .expect("endurance lmp2 count");
        assert_eq!(lmp2_count, 0);
        assert_eq!(endurance_lmp2_count, 6);
    }

    #[test]
    fn test_block3_respects_class_inauguration_years() {
        let conn = setup_block3_db();
        let mut rng = StdRng::seed_from_u64(33);

        // gt4 entra na Endurance só em 2007; gt3 em 2005.
        let movements_2006 =
            execute_block3_for_year(&conn, 1, 2006, &mut rng).expect("block3 timeline should run");
        assert!(movements_2006
            .iter()
            .all(|movement| movement.from_category != "gt4" && movement.to_category != "gt4"));
        assert!(movements_2006
            .iter()
            .any(|movement| movement.from_category == "gt3"));

        let movements_2004 =
            execute_block3_for_year(&conn, 1, 2004, &mut rng).expect("block3 timeline should run");
        assert!(movements_2004.is_empty());
    }

    fn setup_block3_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");

        insert_ranked_teams(&conn, "gt4", "GT4", 10, None);
        insert_ranked_teams(&conn, "gt3", "GT3", 14, None);
        insert_ranked_teams(&conn, "endurance", "EG4", 6, Some("gt4"));
        insert_ranked_teams(&conn, "endurance", "EG3", 6, Some("gt3"));
        insert_ranked_teams(&conn, "endurance", "LMP", 6, Some("lmp2"));

        conn
    }

    fn seed_endurance_race_results(conn: &Connection) {
        conn.execute_batch(
            "
            INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
            VALUES ('S001', 1, 2025, 'Finalizada', 1, 'BlocoEspecial', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

            INSERT INTO drivers (id, nome, idade, nacionalidade, genero, categoria_atual, status, ano_inicio_carreira)
            VALUES
                ('D_EG41', 'Piloto EG41', 25, 'Brasil', 'M', 'endurance', 'Ativo', 2020),
                ('D_EG46', 'Piloto EG46', 25, 'Brasil', 'M', 'endurance', 'Ativo', 2020);

            INSERT INTO calendar (
                id, temporada_id, season_id, rodada, pista, categoria, status, nome,
                track_name, track_config
            ) VALUES ('R_END', 'S001', 'S001', 1, 'Le Mans', 'endurance',
                'Concluida', 'End R1', 'Le Mans', 'default');

            INSERT INTO race_results (
                race_id, piloto_id, equipe_id, posicao_largada, posicao_final, voltas_completadas, pontos
            ) VALUES
                ('R_END', 'D_EG46', 'EG46', 1, 1, 10, 35.0),
                ('R_END', 'D_EG41', 'EG41', 2, 2, 10, 28.0);
            ",
        )
        .expect("seed endurance race results");
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
