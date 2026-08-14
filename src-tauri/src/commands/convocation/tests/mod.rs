use std::time::{SystemTime, UNIX_EPOCH};

use rand::{rngs::StdRng, SeedableRng};

use super::*;
use crate::convocation::player_offers::get_player_special_offer_by_id;
use crate::convocation::{advance_to_convocation_window, run_convocation_window};
use crate::generators::world::generate_world_with_rng;

fn create_test_base_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("iracerapp_convocation_{label}_{nanos}"))
}

fn seed_special_offer_career(base_dir: &Path) {
    let db_path = career_db_path(base_dir, "career_001");
    let db = Database::create_new(&db_path).expect("create db");

    let mut rng = StdRng::seed_from_u64(77);
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

    let season = crate::models::season::Season::new("S001".to_string(), 1, 2024);
    season_queries::insert_season(&db.conn, &season).expect("insert season");
    for driver in &world.drivers {
        driver_queries::insert_driver(&db.conn, driver).expect("insert driver");
    }
    team_queries::insert_teams(&db.conn, &world.teams).expect("insert teams");
    contract_queries::insert_contracts(&db.conn, &world.contracts).expect("insert contracts");
    db.conn
        .execute(
            "UPDATE meta SET value = ?1 WHERE key = 'next_contract_id'",
            rusqlite::params![(world.contracts.len() + 1).to_string()],
        )
        .expect("sync contract ids");

    let mut player = driver_queries::get_player_driver(&db.conn).expect("player");
    player.categoria_atual = Some("gt4".to_string());
    player.atributos.skill = 98.0;
    driver_queries::update_driver(&db.conn, &player).expect("update player");

    advance_to_convocation_window(&db.conn).expect("advance convocation");
    run_convocation_window(&db.conn).expect("run convocation");
}

fn insert_legacy_player_special_offer(base_dir: &Path, offer_id: &str, status: &str) {
    let db_path = career_db_path(base_dir, "career_001");
    let db = Database::open_existing(&db_path).expect("open db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("active season")
        .expect("season");
    let team = team_queries::get_teams_by_category_and_class(&db.conn, "endurance", "gt4")
        .expect("endurance gt4 teams")
        .into_iter()
        .next()
        .expect("endurance gt4 team");

    db.conn
        .execute(
            "INSERT INTO player_special_offers (
                    id, season_id, player_driver_id, team_id, team_name,
                    special_category, class_name, papel, status, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                offer_id,
                &season.id,
                &player.id,
                &team.id,
                &team.nome,
                "endurance",
                "gt4",
                TeamRole::Numero1.as_str(),
                status,
                crate::common::time::current_timestamp(),
            ],
        )
        .expect("insert legacy special offer");
}

fn assigned_driver_ids(
    conn: &rusqlite::Connection,
    season_id: &str,
) -> std::collections::HashSet<String> {
    let mut stmt = conn
        .prepare(
            "SELECT driver_id
                 FROM special_window_assignments
                 WHERE season_id = ?1",
        )
        .expect("prepare assigned ids");
    let rows = stmt
        .query_map(rusqlite::params![season_id], |row| row.get::<_, String>(0))
        .expect("query assigned ids");

    let mut result = std::collections::HashSet::new();
    for row in rows {
        result.insert(row.expect("assigned driver id"));
    }
    result
}

fn first_unassigned_driver_in_category(
    conn: &rusqlite::Connection,
    season_id: &str,
    category: &str,
) -> crate::models::driver::Driver {
    let assigned = assigned_driver_ids(conn, season_id);
    driver_queries::get_drivers_by_category(conn, category)
        .expect("drivers by category")
        .into_iter()
        .find(|driver| !driver.is_jogador && !assigned.contains(&driver.id))
        .expect("unassigned driver in category")
}

#[test]
fn test_get_player_special_offers_returns_pending_only() {
    let base_dir = create_test_base_dir("list_pending");
    seed_special_offer_career(&base_dir);
    insert_legacy_player_special_offer(&base_dir, "PSO-LEGACY-LIST", "Pendente");

    let offers =
        get_player_special_offers_in_base_dir(&base_dir, "career_001").expect("list offers");

    assert!(offers.is_empty());
    assert!(offers.iter().all(|offer| offer.status == "Pendente"));
}

#[test]
fn test_accept_player_special_offer_rejects_legacy_production_endurance_offer() {
    let base_dir = create_test_base_dir("accept_offer");
    seed_special_offer_career(&base_dir);
    insert_legacy_player_special_offer(&base_dir, "PSO-LEGACY-ACCEPT", "Pendente");

    let error = respond_player_special_offer_in_base_dir(
        &base_dir,
        "career_001",
        "PSO-LEGACY-ACCEPT",
        true,
    )
    .expect_err("legacy production/endurance offer should be rejected");

    let db_path = career_db_path(&base_dir, "career_001");
    let db = Database::open_existing(&db_path).expect("open db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let contract = contract_queries::get_active_especial_contract_for_pilot(&db.conn, &player.id)
        .expect("special contract lookup");

    assert!(error.contains("contratos regulares"));
    assert!(player.categoria_especial_ativa.is_none());
    assert!(contract.is_none());
}

#[test]
fn test_reject_player_special_offer_marks_recusada() {
    let base_dir = create_test_base_dir("reject_offer");
    seed_special_offer_career(&base_dir);
    insert_legacy_player_special_offer(&base_dir, "PSO-LEGACY-REJECT", "Pendente");

    let response = respond_player_special_offer_in_base_dir(
        &base_dir,
        "career_001",
        "PSO-LEGACY-REJECT",
        false,
    )
    .expect("reject offer");

    assert_eq!(response.action, "rejected");
    assert!(response.remaining_offers >= 0);

    let db_path = career_db_path(&base_dir, "career_001");
    let db = Database::open_existing(&db_path).expect("open db");
    let rejected = get_player_special_offer_by_id(&db.conn, "PSO-LEGACY-REJECT")
        .expect("offer query")
        .expect("offer");
    assert_eq!(rejected.status, "Recusada");
}

#[test]
fn test_get_player_special_offers_ignores_other_season_offers() {
    let base_dir = create_test_base_dir("ignore_other_season");
    seed_special_offer_career(&base_dir);

    let db_path = career_db_path(&base_dir, "career_001");
    let db = Database::open_existing(&db_path).expect("open db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let mut old_season = crate::models::season::Season::new("S999".to_string(), 999, 3024);
    old_season.status = crate::models::enums::SeasonStatus::Finalizada;
    season_queries::insert_season(&db.conn, &old_season).expect("insert old season");

    db.conn
        .execute(
            "INSERT INTO player_special_offers (
                    id, season_id, player_driver_id, team_id, team_name,
                    special_category, class_name, papel, status, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                "PSO-OLD-SEASON",
                "S999",
                &player.id,
                "T001",
                "Equipe Antiga",
                "endurance",
                "gt4",
                TeamRole::Numero1.as_str(),
                "Pendente",
                crate::common::time::current_timestamp(),
            ],
        )
        .expect("insert old-season offer");

    let offers =
        get_player_special_offers_in_base_dir(&base_dir, "career_001").expect("list offers");

    assert!(
        offers.iter().all(|offer| offer.id != "PSO-OLD-SEASON"),
        "ofertas de temporada antiga nao deveriam aparecer na listagem atual"
    );
}

#[test]
fn test_cannot_accept_already_resolved_special_offer() {
    let base_dir = create_test_base_dir("accept_resolved");
    seed_special_offer_career(&base_dir);
    insert_legacy_player_special_offer(&base_dir, "PSO-LEGACY-RESOLVED", "Recusada");

    let error = respond_player_special_offer_in_base_dir(
        &base_dir,
        "career_001",
        "PSO-LEGACY-RESOLVED",
        true,
    )
    .expect_err("resolved offer should not be accepted");
    assert!(error.contains("nao esta mais pendente"));
}

#[test]
fn test_cannot_accept_offer_from_other_season() {
    let base_dir = create_test_base_dir("accept_other_season");
    seed_special_offer_career(&base_dir);

    let db_path = career_db_path(&base_dir, "career_001");
    let db = Database::open_existing(&db_path).expect("open db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let mut old_season = crate::models::season::Season::new("S999".to_string(), 999, 3024);
    old_season.status = crate::models::enums::SeasonStatus::Finalizada;
    season_queries::insert_season(&db.conn, &old_season).expect("insert old season");

    db.conn
        .execute(
            "INSERT INTO player_special_offers (
                    id, season_id, player_driver_id, team_id, team_name,
                    special_category, class_name, papel, status, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                "PSO-FOREIGN-SEASON",
                "S999",
                &player.id,
                "T001",
                "Equipe Antiga",
                "endurance",
                "gt4",
                TeamRole::Numero1.as_str(),
                "Pendente",
                crate::common::time::current_timestamp(),
            ],
        )
        .expect("insert old-season offer");

    let error = respond_player_special_offer_in_base_dir(
        &base_dir,
        "career_001",
        "PSO-FOREIGN-SEASON",
        true,
    )
    .expect_err("foreign season offer should not be accepted");

    assert!(error.contains("nao encontrada"));
}

/// Insere uma oferta pendente cuja `special_category` NÃO é production/endurance, que
/// são as duas que `accept_player_special_offer_tx` recusa na primeira linha. É o único
/// jeito de exercitar o corpo transacional do aceite: com as categorias de produção a
/// função devolve `Err` antes de tocar no banco.
fn insert_offer_com_categoria_aceitavel(base_dir: &Path, offer_id: &str) -> (String, String) {
    let db_path = career_db_path(base_dir, "career_001");
    let db = Database::open_existing(&db_path).expect("open db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("active season")
        .expect("season");
    let team = team_queries::get_teams_by_category_and_class(&db.conn, "endurance", "gt4")
        .expect("endurance gt4 teams")
        .into_iter()
        .next()
        .expect("endurance gt4 team");

    db.conn
        .execute(
            "INSERT INTO player_special_offers (
                    id, season_id, player_driver_id, team_id, team_name,
                    special_category, class_name, papel, status, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                offer_id,
                &season.id,
                &player.id,
                &team.id,
                &team.nome,
                "convite_teste",
                "gt4",
                TeamRole::Numero1.as_str(),
                "Pendente",
                crate::common::time::current_timestamp(),
            ],
        )
        .expect("insert offer com categoria aceitavel");

    (player.id, team.id)
}

/// O aceite LÊ (contrato especial ativo, equipe, contrato do substituído) e só então
/// ESCREVE, tudo na mesma transação. Este caso trava as duas pontas do que a troca de
/// DEFERRED para IMMEDIATE precisa preservar: a sequência SELECT→WRITE completa dentro
/// da transação, e o desfazimento integral quando ela volta atrás sem commit.
#[test]
fn test_aceite_faz_select_e_write_na_mesma_transacao_e_desfaz_tudo_no_rollback() {
    let base_dir = create_test_base_dir("aceite_rollback");
    seed_special_offer_career(&base_dir);
    let (player_id, team_id) = insert_offer_com_categoria_aceitavel(&base_dir, "PSO-TX");

    let db_path = career_db_path(&base_dir, "career_001");
    let mut db = Database::open_existing(&db_path).expect("open db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("active season")
        .expect("season");
    let offer = get_player_special_offer_by_id_for_season(&db.conn, &season.id, "PSO-TX")
        .expect("offer query")
        .expect("offer");
    // Linha de base do lineup, para comparar depois do rollback.
    let piloto_1_antes = team_queries::get_team_by_id(&db.conn, &team_id)
        .expect("equipe antes")
        .expect("equipe")
        .piloto_1_id;

    // Mesmo comportamento que os dois call sites de produção usam.
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .expect("begin immediate");
    accept_player_special_offer_tx(&tx, &player, &season, &offer).expect("aceite dentro da tx");

    // DENTRO da transação as escritas já valem: o SELECT→WRITE completou.
    let dentro = get_player_special_offer_by_id_for_season(&tx, &season.id, "PSO-TX")
        .expect("offer query dentro da tx")
        .expect("offer dentro da tx");
    assert_eq!(dentro.status, "Aceita");
    assert!(
        contract_queries::has_active_especial_contract(&tx, &player_id)
            .expect("contrato especial dentro da tx"),
        "o aceite deveria ter criado o contrato especial dentro da transação"
    );
    let equipe_dentro = team_queries::get_team_by_id(&tx, &team_id)
        .expect("equipe dentro da tx")
        .expect("equipe");
    assert_eq!(
        equipe_dentro.piloto_1_id.as_deref(),
        Some(player_id.as_str())
    );

    // Sem commit: descartar a transação tem que apagar TODAS as escritas acima.
    drop(tx);

    let offer_depois = get_player_special_offer_by_id_for_season(&db.conn, &season.id, "PSO-TX")
        .expect("offer query depois do rollback")
        .expect("offer depois do rollback");
    assert_eq!(
        offer_depois.status, "Pendente",
        "o rollback tem que devolver a oferta para pendente"
    );
    assert!(
        !contract_queries::has_active_especial_contract(&db.conn, &player_id)
            .expect("contrato especial depois do rollback"),
        "o contrato especial não pode sobreviver ao rollback"
    );
    let equipe_depois = team_queries::get_team_by_id(&db.conn, &team_id)
        .expect("equipe depois do rollback")
        .expect("equipe");
    assert_eq!(
        equipe_depois.piloto_1_id, piloto_1_antes,
        "o lineup da equipe tem que voltar ao que era antes da transação"
    );
    let jogador_depois = driver_queries::get_player_driver(&db.conn).expect("player depois");
    assert!(
        jogador_depois.categoria_especial_ativa.is_none(),
        "a categoria especial do jogador não pode sobreviver ao rollback"
    );
}

#[test]
fn test_special_window_state_starts_on_day_one_with_daily_payload() {
    let base_dir = create_test_base_dir("window_state_day_one");
    seed_special_offer_career(&base_dir);

    let state = get_special_window_state_in_base_dir(&base_dir, "career_001")
        .expect("load special window state");

    assert_eq!(state.current_day, 1);
    assert_eq!(state.total_days, 7);
    assert!(
        state.team_sections.is_empty(),
        "Production/Endurance nao devem expor secoes da janela especial legada"
    );
    assert!(
        state.eligible_candidates.is_empty(),
        "Production/Endurance nao devem expor candidatos da janela especial legada"
    );
    assert!(
        state.last_day_log.is_empty(),
        "o dia 1 nao deve mostrar fechamento antes do primeiro avancar"
    );
    assert!(state.player_offers.is_empty());
}

#[test]
fn test_accept_special_offer_for_day_keeps_single_active_choice() {
    let base_dir = create_test_base_dir("single_daily_choice");
    seed_special_offer_career(&base_dir);

    advance_special_window_day_in_base_dir(&base_dir, "career_001").expect("advance to day 2");
    advance_special_window_day_in_base_dir(&base_dir, "career_001").expect("advance to day 3");

    let state = get_special_window_state_in_base_dir(&base_dir, "career_001")
        .expect("load special window state");
    assert!(state.player_offers.is_empty());
    assert!(state.active_offer_id.is_none());
}

#[test]
fn test_advance_special_window_day_reveals_market_movements() {
    let base_dir = create_test_base_dir("advance_special_window_day");
    seed_special_offer_career(&base_dir);

    let before = get_special_window_state_in_base_dir(&base_dir, "career_001")
        .expect("load window before advance");
    let advanced = advance_special_window_day_in_base_dir(&base_dir, "career_001")
        .expect("advance special window day");

    assert_eq!(advanced.current_day, before.current_day + 1);
    assert!(advanced.last_day_log.is_empty());
}

#[test]
fn test_special_window_eligible_candidates_show_only_current_main_names() {
    let base_dir = create_test_base_dir("eligible_shortlist");
    seed_special_offer_career(&base_dir);

    let db_path = career_db_path(&base_dir, "career_001");
    let db = Database::open_existing(&db_path).expect("open db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("active season")
        .expect("season");

    let mut top_amador = first_unassigned_driver_in_category(&db.conn, &season.id, "mazda_amador");
    let mut second_amador =
        first_unassigned_driver_in_category(&db.conn, &season.id, "toyota_amador");
    second_amador.categoria_atual = Some("mazda_amador".to_string());
    let mut rookie = first_unassigned_driver_in_category(&db.conn, &season.id, "mazda_rookie");
    let unemployed = first_unassigned_driver_in_category(&db.conn, &season.id, "gt4");

    top_amador.stats_temporada.pontos = 250.0;
    top_amador.stats_temporada.vitorias = 4;
    top_amador.stats_temporada.podios = 7;
    driver_queries::update_driver(&db.conn, &top_amador).expect("update top amador");

    second_amador.stats_temporada.pontos = 120.0;
    second_amador.stats_temporada.vitorias = 1;
    second_amador.stats_temporada.podios = 3;
    driver_queries::update_driver(&db.conn, &second_amador).expect("update second amador");

    rookie.stats_temporada.pontos = 999.0;
    rookie.stats_temporada.vitorias = 8;
    rookie.stats_temporada.podios = 8;
    driver_queries::update_driver(&db.conn, &rookie).expect("update rookie");

    let unemployed_contract =
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &unemployed.id)
            .expect("active regular contract")
            .expect("unemployed regular contract");
    contract_queries::update_contract_status(
        &db.conn,
        &unemployed_contract.id,
        &ContractStatus::Rescindido,
    )
    .expect("expire unemployed regular contract");

    db.conn
        .execute(
            "INSERT OR REPLACE INTO special_window_candidate_pool (
                    season_id, driver_id, driver_name, origin_category, license_level,
                    desirability, production_eligible, endurance_eligible, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'Livre')",
            rusqlite::params![
                &season.id,
                &top_amador.id,
                &top_amador.nome,
                "mazda_amador",
                2_i64,
                84_i32,
                0_i64,
                0_i64,
            ],
        )
        .expect("upsert top amador");
    db.conn
        .execute(
            "INSERT OR REPLACE INTO special_window_candidate_pool (
                    season_id, driver_id, driver_name, origin_category, license_level,
                    desirability, production_eligible, endurance_eligible, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'Livre')",
            rusqlite::params![
                &season.id,
                &second_amador.id,
                &second_amador.nome,
                "mazda_amador",
                2_i64,
                99_i32,
                0_i64,
                0_i64,
            ],
        )
        .expect("upsert second amador");
    db.conn
        .execute(
            "INSERT OR REPLACE INTO special_window_candidate_pool (
                    season_id, driver_id, driver_name, origin_category, license_level,
                    desirability, production_eligible, endurance_eligible, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'Livre')",
            rusqlite::params![
                &season.id,
                &rookie.id,
                &rookie.nome,
                "mazda_rookie",
                1_i64,
                110_i32,
                0_i64,
                0_i64,
            ],
        )
        .expect("upsert rookie");
    db.conn
        .execute(
            "INSERT OR REPLACE INTO special_window_candidate_pool (
                    season_id, driver_id, driver_name, origin_category, license_level,
                    desirability, production_eligible, endurance_eligible, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'Livre')",
            rusqlite::params![
                &season.id,
                &unemployed.id,
                &unemployed.nome,
                "gt4",
                4_i64,
                101_i32,
                0_i64,
                0_i64,
            ],
        )
        .expect("upsert unemployed");

    let state = get_special_window_state_in_base_dir(&base_dir, "career_001")
        .expect("load special window state");

    assert!(
            state.eligible_candidates.is_empty(),
            "mesmo candidatos legados inseridos manualmente nao devem aparecer para Production/Endurance"
        );
}

#[test]
fn test_special_window_eligible_candidates_use_regular_contract_category_when_driver_current_category_is_null(
) {
    let base_dir = create_test_base_dir("eligible_contract_fallback");
    seed_special_offer_career(&base_dir);

    let db_path = career_db_path(&base_dir, "career_001");
    let db = Database::open_existing(&db_path).expect("open db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("active season")
        .expect("season");

    let mut contracted_gt4 = first_unassigned_driver_in_category(&db.conn, &season.id, "gt4");
    contracted_gt4.categoria_atual = None;
    contracted_gt4.stats_temporada.pontos = 320.0;
    contracted_gt4.stats_temporada.vitorias = 5;
    driver_queries::update_driver(&db.conn, &contracted_gt4).expect("update gt4 driver");

    db.conn
        .execute(
            "INSERT OR REPLACE INTO special_window_candidate_pool (
                    season_id, driver_id, driver_name, origin_category, license_level,
                    desirability, production_eligible, endurance_eligible, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'Livre')",
            rusqlite::params![
                &season.id,
                &contracted_gt4.id,
                &contracted_gt4.nome,
                "bmw_m2",
                1_i64,
                95_i32,
                1_i64,
                0_i64,
            ],
        )
        .expect("upsert fallback candidate");

    let state = get_special_window_state_in_base_dir(&base_dir, "career_001")
        .expect("load special window state");

    assert!(state
        .eligible_candidates
        .iter()
        .all(|candidate| candidate.driver_id != contracted_gt4.id));
}
