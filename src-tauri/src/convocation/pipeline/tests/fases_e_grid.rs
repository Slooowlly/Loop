//! Convocação e bloco especial: as fases, o grid especial e o encerramento.
//!
//! Guarda o mundo de teste (`setup_world_db`) compartilhado com [`super::pos_especial`].

use super::super::*;
use rand::{rngs::StdRng, SeedableRng};
use rusqlite::Connection;

use super::*;
use crate::calendar::CalendarEntry;
use crate::db::migrations;
use crate::db::queries::{calendar as calq, contracts as cq, drivers as dq, seasons as sq};
use crate::generators::world::generate_world_with_rng;
use crate::models::enums::{RaceStatus, SeasonPhase, ThematicSlot, WeatherCondition};

pub(super) fn setup_world_db() -> (Connection, String) {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("migrations");

    let mut rng = StdRng::seed_from_u64(99);
    let world = generate_world_with_rng(
        "Test Player",
        "🇧🇷 Brasileiro",
        20,
        "mazda_rookie",
        0,
        "medio",
        &mut rng,
    )
    .expect("world generation");

    let season_id = "S001".to_string();
    let season = crate::models::season::Season::new(season_id.clone(), 1, 2024);
    sq::insert_season(&conn, &season).expect("insert season");
    for driver in &world.drivers {
        dq::insert_driver(&conn, driver).expect("insert driver");
    }
    crate::db::queries::teams::insert_teams(&conn, &world.teams).expect("insert teams");
    cq::insert_contracts(&conn, &world.contracts).expect("insert contracts");

    // Sincronizar o contador de IDs com a quantidade de contratos inseridos
    let next_contract = world.contracts.len() + 1;
    conn.execute(
        "UPDATE meta SET value = ?1 WHERE key = 'next_contract_id'",
        rusqlite::params![next_contract.to_string()],
    )
    .expect("update meta contract counter");

    (conn, season_id)
}

fn make_player_eligible_for_specials(conn: &Connection, category: &str) -> String {
    let mut player = dq::get_player_driver(conn).expect("player");
    player.categoria_atual = Some(category.to_string());
    player.atributos.skill = 98.0;
    player.melhor_resultado_temp = Some(1);
    player.stats_temporada.vitorias = 4;
    dq::update_driver(conn, &player).expect("update player");
    player.id
}

fn insert_pending_regular_race(conn: &Connection, season_id: &str, category: &str) {
    calq::insert_calendar_entry(
        conn,
        &CalendarEntry {
            id: "R-PENDING-REGULAR".to_string(),
            season_id: season_id.to_string(),
            categoria: category.to_string(),
            rodada: 1,
            nome: "Corrida regular pendente".to_string(),
            track_id: 1,
            track_name: "Interlagos".to_string(),
            track_config: "GP".to_string(),
            clima: WeatherCondition::Dry,
            temperatura: 24.0,
            voltas: 20,
            duracao_corrida_min: 30,
            duracao_classificacao_min: 10,
            status: RaceStatus::Pendente,
            horario: "14:00".to_string(),
            week_of_year: 30,
            season_phase: SeasonPhase::BlocoRegular,
            display_date: "2024-09-15".to_string(),
            thematic_slot: ThematicSlot::RodadaRegular,
            season_week: None,
        },
    )
    .expect("insert pending regular race");
}

#[test]
fn test_season_phase_transitions() {
    let (conn, season_id) = setup_world_db();

    // Começa em BlocoRegular
    let s = sq::get_season_by_id(&conn, &season_id).unwrap().unwrap();
    assert_eq!(s.fase, SeasonPhase::BlocoRegular);

    // advance → JanelaConvocacao
    advance_to_convocation_window(&conn).expect("advance");
    let s = sq::get_season_by_id(&conn, &season_id).unwrap().unwrap();
    assert_eq!(s.fase, SeasonPhase::JanelaConvocacao);

    // iniciar_bloco_especial → BlocoEspecial
    iniciar_bloco_especial(&conn).expect("iniciar");
    let s = sq::get_season_by_id(&conn, &season_id).unwrap().unwrap();
    assert_eq!(s.fase, SeasonPhase::BlocoEspecial);
}

#[test]
fn test_run_convocation_skips_real_regular_special_categories() {
    let (conn, season_id) = setup_world_db();

    advance_to_convocation_window(&conn).expect("advance convocation");
    let result = run_convocation_window(&conn).expect("run convocation");

    let special_contracts: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM contracts
             WHERE tipo = 'Especial'
               AND categoria IN ('production_challenger', 'endurance')",
            [],
            |row| row.get(0),
        )
        .expect("count special contracts");
    let special_entries: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM special_team_entries
             WHERE season_id = ?1
               AND special_category IN ('production_challenger', 'endurance')",
            rusqlite::params![season_id],
            |row| row.get(0),
        )
        .expect("count special entries");
    let active_special_drivers: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM drivers
             WHERE categoria_especial_ativa IN ('production_challenger', 'endurance')",
            [],
            |row| row.get(0),
        )
        .expect("count active special drivers");

    assert_eq!(result.total_contratos, 0);
    assert!(result.grids.is_empty());
    assert_eq!(special_contracts, 0);
    assert_eq!(special_entries, 0);
    assert_eq!(active_special_drivers, 0);
}

#[test]
fn test_advance_requires_bloco_regular() {
    let (conn, _) = setup_world_db();
    // Avançar duas vezes deve falhar na segunda
    advance_to_convocation_window(&conn).expect("primeira avançada");
    let result = advance_to_convocation_window(&conn);
    assert!(
        result.is_err(),
        "deveria falhar se não estiver em BlocoRegular"
    );
}

#[test]
fn test_advance_to_convocation_rejects_pending_regular_races() {
    let (conn, season_id) = setup_world_db();
    insert_pending_regular_race(&conn, &season_id, "gt3");

    let result = advance_to_convocation_window(&conn);

    assert!(
        result.is_err(),
        "nao deveria abrir convocacao antes do fim real do bloco regular"
    );
}

#[test]
fn test_run_convocation_requires_janela() {
    let (conn, _) = setup_world_db();
    // Tentar convocação em BlocoRegular deve falhar
    let result = run_convocation_window(&conn);
    assert!(result.is_err(), "deveria falhar fora de JanelaConvocacao");
}

#[test]
fn test_iniciar_bloco_especial_rolls_back_phase_when_calendar_generation_fails() {
    let (conn, season_id) = setup_world_db();
    advance_to_convocation_window(&conn).expect("advance");

    conn.execute(
        "CREATE TRIGGER fail_special_calendar_insert
         BEFORE INSERT ON calendar
         BEGIN
             SELECT RAISE(ABORT, 'special calendar blocked');
         END;",
        [],
    )
    .expect("create trigger");

    let result = iniciar_bloco_especial(&conn);
    assert!(result.is_err(), "inicio do bloco especial deveria falhar");

    let season = sq::get_season_by_id(&conn, &season_id)
        .expect("season query")
        .expect("season");
    assert_eq!(
        season.fase,
        SeasonPhase::JanelaConvocacao,
        "a fase nao deve avancar se a geracao do calendario especial falhar"
    );
}

#[test]
fn test_run_convocation_rolls_back_when_player_offer_persistence_fails() {
    let (conn, _) = setup_world_db();
    make_player_eligible_for_specials(&conn, "gt4");
    advance_to_convocation_window(&conn).expect("advance");

    conn.execute(
        "CREATE TRIGGER fail_player_special_offer_insert
         BEFORE INSERT ON player_special_offers
         BEGIN
             SELECT RAISE(ABORT, 'player special offer blocked');
         END;",
        [],
    )
    .expect("create trigger");

    let result = run_convocation_window(&conn).expect("convocation without legacy classes");
    assert_eq!(result.total_contratos, 0);

    let especial_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM contracts WHERE tipo = 'Especial'",
            [],
            |row| row.get(0),
        )
        .expect("special contracts count");
    assert_eq!(
        especial_count, 0,
        "a convocacao precisa ser atomica e nao deixar contratos especiais apos falha nas ofertas"
    );

    let drivers_in_special: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM drivers WHERE categoria_especial_ativa IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .expect("drivers in special count");
    assert_eq!(
        drivers_in_special, 0,
        "a convocacao nao deve marcar pilotos no especial apos rollback"
    );
}

#[test]
fn test_run_convocation_propagates_player_lookup_errors() {
    let (conn, _) = setup_world_db();
    advance_to_convocation_window(&conn).expect("advance");
    let player_id = dq::get_player_driver(&conn).expect("player").id;

    conn.execute(
        "UPDATE drivers SET personalidade_primaria = 'perfil_quebrado' WHERE id = ?1",
        rusqlite::params![player_id],
    )
    .expect("corrupt player personality");

    let result = run_convocation_window(&conn);
    assert!(
        result.is_err(),
        "erro estrutural na leitura do jogador nao deveria ser tratado como ausencia de jogador"
    );
}

#[test]
fn test_run_convocation_no_duplicate_drivers() {
    let (conn, _) = setup_world_db();
    advance_to_convocation_window(&conn).expect("advance");

    let result = run_convocation_window(&conn).expect("convocação");

    // Nenhum driver_id duplicado em todos os grids
    let mut all_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for grid in &result.grids {
        for a in &grid.assignments {
            assert!(
                all_ids.insert(a.driver_id.clone()),
                "driver {} duplicado entre grids",
                a.driver_id
            );
        }
    }
}

#[test]
fn test_run_convocation_contracts_are_especial() {
    let (conn, _) = setup_world_db();
    advance_to_convocation_window(&conn).expect("advance");
    let result = run_convocation_window(&conn).expect("convocação");
    assert!(
        result.errors.is_empty(),
        "erros na convocação: {:?}",
        result.errors
    );

    // Todos os contratos especiais gerados devem ter tipo=Especial
    let especiais: Vec<_> = conn
        .prepare("SELECT tipo FROM contracts WHERE tipo = 'Especial'")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert!(
        especiais.is_empty(),
        "Production/Endurance nao geram contrato Especial"
    );
}

#[test]
fn test_run_convocation_contracts_have_classe() {
    let (conn, _) = setup_world_db();
    advance_to_convocation_window(&conn).expect("advance");
    run_convocation_window(&conn).expect("convocação");

    // Contratos especiais devem ter classe não nula
    let null_classe: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM contracts WHERE tipo='Especial' AND classe IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    assert_eq!(
        null_classe, 0,
        "contratos especiais com classe=NULL: {}",
        null_classe
    );
}

#[test]
fn test_run_convocation_drivers_keep_regular_category() {
    let (conn, _) = setup_world_db();
    advance_to_convocation_window(&conn).expect("advance");
    let result = run_convocation_window(&conn).expect("convocação");

    // categoria_atual dos pilotos convocados deve estar intacta
    for grid in &result.grids {
        for a in &grid.assignments {
            let driver = dq::get_driver(&conn, &a.driver_id).expect("get driver");
            // categoria_especial_ativa deve estar preenchida
            assert!(
                driver.categoria_especial_ativa.is_some(),
                "piloto {} não tem categoria_especial_ativa após convocação",
                driver.nome
            );
        }
    }
}

#[test]
fn test_lmp2_class_gets_special_grid() {
    let (conn, _) = setup_world_db();
    advance_to_convocation_window(&conn).expect("advance");
    run_convocation_window(&conn).expect("convocação");

    // LMP2 agora e uma classe da Endurance no bloco especial.
    let lmp2_with_two_pilots: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM teams
             WHERE categoria='endurance'
               AND classe='lmp2'
               AND piloto_1_id IS NOT NULL
               AND piloto_2_id IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let lmp2_contracts: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM contracts
             WHERE tipo='Especial'
               AND categoria='endurance'
               AND classe='lmp2'
               AND status='Ativo'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let lmp2_entries: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM special_team_entries
             WHERE special_category='endurance'
               AND class_name='lmp2'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    assert!(
        lmp2_with_two_pilots > 0,
        "LMP2 deveria montar grid proprio como classe Endurance"
    );
    assert_eq!(
        lmp2_contracts, 0,
        "LMP2 nao gera contratos especiais nesta fase"
    );
    assert_eq!(
        lmp2_entries, 0,
        "LMP2 nao usa special_team_entries nesta fase"
    );
}

#[test]
fn test_special_entries_ignore_previous_special_guarantees() {
    let (conn, season_id) = setup_world_db();
    let mut previous_season = crate::models::season::Season::new("S000".to_string(), 0, 2023);
    previous_season.finalizar();
    sq::insert_season(&conn, &previous_season).expect("insert previous season");

    let feeder_category = "mazda_amador";
    let mut team_ids: Vec<String> = conn
        .prepare("SELECT id FROM teams WHERE categoria = ?1 ORDER BY nome ASC")
        .expect("prepare team query")
        .query_map(rusqlite::params![feeder_category], |row| {
            row.get::<_, String>(0)
        })
        .expect("query teams")
        .map(|row| row.expect("team id"))
        .collect();
    assert!(
        team_ids.len() > 5,
        "teste precisa de mais equipes regulares que vagas especiais"
    );

    let previously_guaranteed_team_id = team_ids.pop().expect("guaranteed team");
    special_entry_queries::replace_entries_for_class(
        &conn,
        "S000",
        "production_challenger",
        "mazda",
        &[special_entry_queries::NewSpecialTeamEntry {
            team_id: previously_guaranteed_team_id.clone(),
            source_category: feeder_category.to_string(),
            qualified_via: "GarantiaEspecial".to_string(),
            guaranteed_next_year: true,
        }],
    )
    .expect("previous guarantee");

    conn.execute(
        "UPDATE teams SET stats_pontos = 0, stats_vitorias = 0, stats_melhor_resultado = 99
         WHERE categoria = ?1",
        rusqlite::params![feeder_category],
    )
    .expect("reset standings");
    for (index, team_id) in team_ids.iter().take(5).enumerate() {
        conn.execute(
            "UPDATE teams
             SET stats_pontos = ?2, stats_vitorias = ?3, stats_melhor_resultado = 1
             WHERE id = ?1",
            rusqlite::params![team_id, 100 - index as i32, 5 - index as i32],
        )
        .expect("seed regular contender");
    }

    ensure_special_team_entries(&conn, &season_id, 1).expect("ensure entries");

    let entries = special_entry_queries::get_entries_for_class(
        &conn,
        &season_id,
        "production_challenger",
        "mazda",
    )
    .expect("entries");
    assert!(
        entries
            .iter()
            .all(|entry| entry.qualified_via.starts_with("RegularP")),
        "todas as vagas especiais devem vir da temporada regular: {:?}",
        entries
            .iter()
            .map(|entry| entry.qualified_via.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        entries
            .iter()
            .all(|entry| entry.team_id != previously_guaranteed_team_id),
        "equipe com garantia antiga nao pode furar a fila regular"
    );
}

#[test]
fn test_special_window_team_sections_expose_class_for_each_car() {
    let (conn, season_id) = setup_world_db();
    let player = dq::get_player_driver(&conn).expect("player");
    advance_to_convocation_window(&conn).expect("advance");
    run_convocation_window(&conn).expect("convocação");

    let payload = special_window::load_special_window_payload(&conn, &season_id, &player.id)
        .expect("payload");
    let mut classes_by_category: std::collections::HashMap<
        String,
        std::collections::HashSet<String>,
    > = std::collections::HashMap::new();
    let mut missing_class_teams = Vec::new();

    for section in payload.team_sections {
        for team in section.teams {
            match team.classe {
                Some(class_name) => {
                    classes_by_category
                        .entry(section.category.clone())
                        .or_default()
                        .insert(class_name);
                }
                None => missing_class_teams.push(team.nome),
            }
        }
    }

    assert!(missing_class_teams.is_empty());
    assert!(classes_by_category.get("production_challenger").is_none());
    assert!(classes_by_category.get("endurance").is_none());
}

#[test]
fn test_persistir_grids_rolls_back_all_changes_on_error() {
    let (conn, _) = setup_world_db();
    let season_number = 1;

    let next_contract: i64 = conn
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'next_contract_id'",
            [],
            |row| row.get(0),
        )
        .expect("read next contract id");
    let second_contract_id = format!("C{:03}", next_contract + 1);
    conn.execute_batch(&format!(
        "
        CREATE TRIGGER fail_second_special_contract_insert
        BEFORE INSERT ON contracts
        WHEN NEW.id = '{second_contract_id}'
        BEGIN
            SELECT RAISE(ABORT, 'forced special contract failure');
        END;
        "
    ))
    .expect("create failing trigger");

    let team = team_queries::get_teams_by_category(&conn, "gt4")
        .expect("gt4 teams")
        .into_iter()
        .next()
        .expect("at least one gt4 team");
    let drivers = dq::get_drivers_by_category(&conn, "gt4").expect("gt4 drivers");
    let assignments = vec![
        DriverAssignment {
            driver_id: drivers[0].id.clone(),
            team_id: team.id.clone(),
            papel: TeamRole::Numero1,
            fonte: "MeritoRegular".to_string(),
            score: 99.0,
        },
        DriverAssignment {
            driver_id: drivers[1].id.clone(),
            team_id: team.id.clone(),
            papel: TeamRole::Numero2,
            fonte: "MeritoRegular".to_string(),
            score: 98.0,
        },
    ];

    // Chama o caminho de PRODUÇÃO. Este teste já rodou contra uma cópia
    // `#[cfg(test)] persistir_grids` que repetia a montagem dos contratos e a ordem das
    // escritas — duas regras de domínio para o mesmo fato, e a de teste podia divergir
    // sem ninguém ver. Sem ofertas de jogador o `season_id` não é lido.
    let result = persistir_grids_e_ofertas(
        &conn,
        "",
        &[GridClasse {
            class_name: "gt4".to_string(),
            assignments,
        }],
        season_number,
        None,
    );
    assert!(result.is_err(), "persistência deveria falhar com trigger");

    let especiais: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM contracts WHERE tipo = 'Especial'",
            [],
            |row| row.get(0),
        )
        .expect("count special contracts");
    assert_eq!(
        especiais, 0,
        "nenhum contrato especial deveria sobreviver após rollback"
    );

    for driver in drivers.iter().take(2) {
        let refreshed = dq::get_driver(&conn, &driver.id).expect("refresh driver");
        assert!(
            refreshed.categoria_especial_ativa.is_none(),
            "piloto {} não deveria ficar marcado no especial após rollback",
            refreshed.nome
        );
    }
}

/// A convocação inteira só é alcançável a partir de `BlocoRegular` (ver o portão em
/// `advance_to_convocation_window`), e nenhuma carreira nova entra nessa fase — o módulo
/// existe hoje só para saves pré-v33. Este teste fixa a FRONTEIRA entre os dois modelos:
/// se uma fase do modelo novo passar a contar como legada (ou o contrário), a condição
/// para aposentar este diretório mudou e alguém precisa saber.
#[test]
fn fases_legadas_sao_exatamente_as_da_convocacao() {
    use crate::models::enums::SeasonPhase;

    for fase in [
        SeasonPhase::BlocoRegular,
        SeasonPhase::JanelaConvocacao,
        SeasonPhase::BlocoEspecial,
        SeasonPhase::PosEspecial,
    ] {
        assert!(fase.is_legacy(), "{fase} deveria ser fase legada");
    }
    for fase in [
        SeasonPhase::PreTemporada,
        SeasonPhase::Temporada,
        SeasonPhase::Encerramento,
    ] {
        assert!(
            !fase.is_legacy(),
            "{fase} é do modelo novo e não pode contar como legada"
        );
    }
}
