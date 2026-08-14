//! Ofertas de convocação dirigidas ao JOGADOR: quem recebe, por que recebe e o que
//! elas NÃO fazem antes do aceite.

use super::super::*;
use super::*;

#[test]
fn test_run_convocation_generates_player_special_offers_for_eligible_player() {
    let (conn, _) = setup_world_db();
    let player_id = make_player_eligible_for_specials(&conn, "gt4");
    advance_to_convocation_window(&conn).expect("advance");

    run_convocation_window(&conn).expect("convocação");

    let mut stmt = conn
        .prepare(
            "SELECT team_id, special_category, class_name, papel, status
             FROM player_special_offers
             WHERE player_driver_id = ?1
             ORDER BY team_id",
        )
        .expect("prepare player special offers");
    let offers: Vec<(String, String, String, String, String)> = stmt
        .query_map(rusqlite::params![player_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .expect("query offers")
        .filter_map(|row| row.ok())
        .collect();

    assert!(
        offers.is_empty(),
        "Production/Endurance nao devem gerar ofertas de convocacao especial"
    );
}

#[test]
fn test_run_convocation_keeps_player_special_offers_separate_from_market_proposals() {
    let (conn, _) = setup_world_db();
    let player_id = make_player_eligible_for_specials(&conn, "gt4");
    advance_to_convocation_window(&conn).expect("advance");

    run_convocation_window(&conn).expect("convocação");

    let special_offer_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM player_special_offers WHERE player_driver_id = ?1",
            rusqlite::params![&player_id],
            |row| row.get(0),
        )
        .expect("count special offers");
    let market_proposal_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM market_proposals WHERE piloto_id = ?1",
            rusqlite::params![&player_id],
            |row| row.get(0),
        )
        .expect("count market proposals");

    assert_eq!(special_offer_count, 0);
    assert_eq!(
        market_proposal_count, 0,
        "convocação especial não deve reaproveitar market_proposals"
    );
}

#[test]
fn test_run_convocation_does_not_activate_player_special_contract_before_acceptance() {
    let (conn, _) = setup_world_db();
    let player_id = make_player_eligible_for_specials(&conn, "gt4");
    advance_to_convocation_window(&conn).expect("advance");

    run_convocation_window(&conn).expect("convocação");

    let player =
        crate::db::queries::drivers::get_driver(&conn, &player_id).expect("player refreshed");
    let especial =
        crate::db::queries::contracts::get_active_especial_contract_for_pilot(&conn, &player_id)
            .expect("special contract lookup");

    assert!(
        player.categoria_especial_ativa.is_none(),
        "jogador não deveria entrar automaticamente no especial antes de aceitar"
    );
    assert!(
        especial.is_none(),
        "jogador não deveria ganhar contrato especial antes de aceitar"
    );
}
use crate::db::queries::{contracts as contract_queries, drivers as driver_queries};

fn insert_historical_contract_for_offer_tests(
    conn: &rusqlite::Connection,
    player_id: &str,
    player_name: &str,
    team_id: &str,
    team_name: &str,
    category: &str,
    class_name: Option<&str>,
) {
    let mut contract = crate::models::contract::Contract::new(
        format!("HC-{player_id}-{team_id}-{category}"),
        player_id.to_string(),
        player_name.to_string(),
        team_id.to_string(),
        team_name.to_string(),
        crate::db::queries::seasons::get_active_season(conn)
            .expect("active season query")
            .expect("active season")
            .numero
            .saturating_sub(1),
        1,
        50_000.0,
        TeamRole::Numero1,
        category.to_string(),
    );
    contract.status = crate::models::enums::ContractStatus::Expirado;
    if let Some(class_name) = class_name {
        contract.tipo = crate::models::enums::ContractType::Especial;
        contract.classe = Some(class_name.to_string());
    }
    contract_queries::insert_contract(conn, &contract).expect("insert historical contract");
}

#[test]
fn test_player_special_offers_prioritize_current_car_over_old_other_car_history() {
    let (conn, season_id) = setup_world_db();
    let player_id = make_player_eligible_for_specials(&conn, "bmw_m2");
    let player = driver_queries::get_driver(&conn, &player_id).expect("player");
    let toyota_team = get_special_class_entry_teams(
        &conn,
        &season_id,
        CLASSES_CONVOCADAS
            .iter()
            .find(|cfg| {
                cfg.special_category == "production_challenger" && cfg.class_name == "toyota"
            })
            .expect("toyota config"),
    )
    .expect("toyota teams")
    .into_iter()
    .next()
    .expect("toyota team");

    insert_historical_contract_for_offer_tests(
        &conn,
        &player_id,
        &player.nome,
        &toyota_team.id,
        &toyota_team.nome,
        "production_challenger",
        Some("toyota"),
    );

    let offers =
        build_player_special_offers(&conn, &season_id, &player).expect("build player offers");

    assert!(offers.is_empty());
}

#[test]
fn test_unemployed_player_with_same_car_history_still_receives_matching_offers() {
    let (conn, season_id) = setup_world_db();
    let player_id = make_player_eligible_for_specials(&conn, "gt4");
    let mut player = driver_queries::get_driver(&conn, &player_id).expect("player");
    let gt4_team = get_special_class_entry_teams(
        &conn,
        &season_id,
        CLASSES_CONVOCADAS
            .iter()
            .find(|cfg| cfg.special_category == "endurance" && cfg.class_name == "gt4")
            .expect("gt4 config"),
    )
    .expect("gt4 teams")
    .into_iter()
    .next()
    .expect("gt4 team");

    for contract in
        contract_queries::get_contracts_for_pilot(&conn, &player_id).expect("player contracts")
    {
        if contract.status == crate::models::enums::ContractStatus::Ativo {
            contract_queries::update_contract_status(
                &conn,
                &contract.id,
                &crate::models::enums::ContractStatus::Expirado,
            )
            .expect("expire player contract");
        }
    }

    player.categoria_atual = None;
    driver_queries::update_driver(&conn, &player).expect("update unemployed player");

    insert_historical_contract_for_offer_tests(
        &conn,
        &player_id,
        &player.nome,
        &gt4_team.id,
        &gt4_team.nome,
        "endurance",
        Some("gt4"),
    );

    let refreshed = driver_queries::get_driver(&conn, &player_id).expect("refreshed player");
    let offers = build_player_special_offers(&conn, &season_id, &refreshed).expect("build offers");

    assert!(offers.is_empty());
}

#[test]
fn test_team_history_can_unlock_offer_outside_current_car_lane() {
    let (conn, season_id) = setup_world_db();
    let player_id = make_player_eligible_for_specials(&conn, "gt3");
    let mut player = driver_queries::get_driver(&conn, &player_id).expect("player");
    player.categoria_atual = None;
    driver_queries::update_driver(&conn, &player).expect("clear player category");
    let toyota_team = get_special_class_entry_teams(
        &conn,
        &season_id,
        CLASSES_CONVOCADAS
            .iter()
            .find(|cfg| {
                cfg.special_category == "production_challenger" && cfg.class_name == "toyota"
            })
            .expect("toyota config"),
    )
    .expect("toyota teams")
    .into_iter()
    .next()
    .expect("toyota team");

    insert_historical_contract_for_offer_tests(
        &conn,
        &player_id,
        &player.nome,
        &toyota_team.id,
        &toyota_team.nome,
        "production_challenger",
        Some("toyota"),
    );

    let offers =
        build_player_special_offers(&conn, &season_id, &player).expect("build player offers");

    assert!(offers.is_empty());
}
