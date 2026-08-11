//! Guard das projeções nomeadas: toda leitura de domínio prepara contra o schema REAL.
//!
//! As consultas de piloto, equipe, contrato, temporada e calendário usavam `SELECT *` e
//! liam a linha por NOME de coluna. Nessa combinação, renomear ou remover uma coluna não
//! quebra compilação, não quebra a trava do schema-ouro e não quebra nenhum teste que
//! monte a tabela à mão: quebra em runtime, dentro do `row.get`, no save de quem estiver
//! jogando. Agora as colunas são nomeadas na consulta, e é este arquivo que transforma a
//! divergência em falha de teste — o `prepare` de uma projeção com coluna inexistente é
//! erro imediato do SQLite.
//!
//! Os bancos aqui estão VAZIOS de propósito. O que se prova é o contrato SQL da consulta
//! (colunas existem, junções resolvem), não o conteúdo — comportamento é assunto dos
//! testes de cada módulo.

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::db::migrations::run_all;
use crate::db::queries::{calendar, contracts, drivers, seasons, teams};
use crate::models::enums::{RaceStatus, SeasonPhase};

fn banco() -> Connection {
    let conn = Connection::open_in_memory().expect("banco em memória");
    run_all(&conn).expect("migrações");
    conn
}

/// Busca de linha única em tabela vazia deve dar `NotFound`, nunca erro de SQL.
///
/// É essa distinção que interessa: `NotFound` é a resposta correta para "não existe";
/// `DbError::Sqlite` seria a projeção quebrada se passando por ausência de dado.
fn e_ausencia_e_nao_erro_de_sql<T>(resultado: Result<T, DbError>, consulta: &str) {
    match resultado {
        Err(DbError::NotFound(_)) => {}
        Err(erro) => panic!("'{consulta}' falhou por erro de SQL, não por ausência: {erro}"),
        Ok(_) => panic!("'{consulta}' devolveu linha num banco vazio"),
    }
}

#[test]
fn as_leituras_de_piloto_preparam_contra_o_schema_real() {
    let conn = banco();

    assert!(drivers::get_all_drivers(&conn).expect("todos").is_empty());
    assert!(drivers::get_drivers_by_category(&conn, "gt3")
        .expect("por categoria")
        .is_empty());
    assert!(drivers::get_drivers_by_active_category(&conn, "gt3")
        .expect("por categoria ativa")
        .is_empty());
    // O ramo especial da mesma função monta outro filtro — precisa preparar também.
    assert!(drivers::get_drivers_by_active_category(&conn, "endurance")
        .expect("por categoria especial")
        .is_empty());
    assert!(drivers::get_drivers_by_status(&conn, "Ativo")
        .expect("por status")
        .is_empty());
    assert!(drivers::get_free_drivers(&conn)
        .expect("sem categoria")
        .is_empty());
    assert!(drivers::get_drivers_without_active_contract(&conn)
        .expect("sem contrato")
        .is_empty());

    e_ausencia_e_nao_erro_de_sql(drivers::get_driver(&conn, "D_INEXISTENTE"), "get_driver");
    e_ausencia_e_nao_erro_de_sql(
        drivers::get_driver_by_name(&conn, "Ninguém"),
        "get_driver_by_name",
    );
    e_ausencia_e_nao_erro_de_sql(drivers::get_player_driver(&conn), "get_player_driver");
}

#[test]
fn as_leituras_de_equipe_preparam_contra_o_schema_real() {
    let conn = banco();

    assert!(teams::get_all_teams(&conn).expect("todas").is_empty());
    assert!(teams::get_teams_by_category(&conn, "gt3")
        .expect("por categoria")
        .is_empty());
    assert!(
        teams::get_teams_by_category_and_class(&conn, "endurance", "LMP2")
            .expect("por categoria e classe")
            .is_empty()
    );
    assert!(teams::get_team_by_id(&conn, "T_INEXISTENTE")
        .expect("por id")
        .is_none());
}

#[test]
fn as_leituras_de_contrato_preparam_contra_o_schema_real() {
    let conn = banco();

    assert!(contracts::get_contract_by_id(&conn, "C_INEXISTENTE")
        .expect("por id")
        .is_none());
    assert!(contracts::get_active_contract_for_pilot(&conn, "D001")
        .expect("ativo do piloto")
        .is_none());
    assert!(
        contracts::get_active_regular_contract_for_pilot(&conn, "D001")
            .expect("regular ativo")
            .is_none()
    );
    assert!(
        contracts::get_active_especial_contract_for_pilot(&conn, "D001")
            .expect("especial ativo")
            .is_none()
    );
    assert!(contracts::get_contracts_for_pilot(&conn, "D001")
        .expect("do piloto")
        .is_empty());
    assert!(contracts::get_active_contracts_for_team(&conn, "T001")
        .expect("ativos da equipe")
        .is_empty());
    assert!(contracts::get_all_active_contracts(&conn)
        .expect("todos ativos")
        .is_empty());
    assert!(contracts::get_all_active_regular_contracts(&conn)
        .expect("regulares ativos")
        .is_empty());
    assert!(
        contracts::get_active_regular_contracts_by_team(&conn, "T001")
            .expect("regulares da equipe")
            .is_empty()
    );
    assert!(contracts::get_expiring_contracts(&conn, 2)
        .expect("expirando")
        .is_empty());
    assert!(contracts::get_contracts_by_category(&conn, "gt3")
        .expect("por categoria")
        .is_empty());
    assert!(
        contracts::get_active_especial_contracts_by_category(&conn, "endurance")
            .expect("especiais da categoria")
            .is_empty()
    );
}

#[test]
fn as_leituras_de_temporada_preparam_contra_o_schema_real() {
    let conn = banco();

    assert!(seasons::get_all_seasons(&conn).expect("todas").is_empty());
    assert!(seasons::get_active_season(&conn).expect("ativa").is_none());
    assert!(seasons::get_season_by_id(&conn, "S_INEXISTENTE")
        .expect("por id")
        .is_none());
}

#[test]
fn as_leituras_de_calendario_preparam_contra_o_schema_real() {
    let conn = banco();

    assert!(calendar::get_calendar(&conn, "S1", "gt3")
        .expect("etapas da categoria")
        .is_empty());
    assert!(calendar::get_next_race(&conn, "S1", "gt3")
        .expect("próxima etapa")
        .is_none());
    assert!(calendar::get_calendar_entry_by_id(&conn, "C_INEXISTENTE")
        .expect("por id")
        .is_none());
    assert!(calendar::get_pending_races(&conn, "S1")
        .expect("pendentes")
        .is_empty());
    assert!(calendar::get_pending_races_for_category(&conn, "S1", "gt3")
        .expect("pendentes da categoria")
        .is_empty());
    assert!(
        calendar::get_pending_races_up_to_week(&conn, "S1", "gt3", 20)
            .expect("pendentes até a semana")
            .is_empty()
    );
    assert!(
        calendar::get_next_any_race_in_phase(&conn, "S1", &SeasonPhase::BlocoRegular)
            .expect("próxima etapa da fase")
            .is_none()
    );
    assert_eq!(
        calendar::count_races_by_status(&conn, "S1", "gt3", &RaceStatus::Pendente)
            .expect("contagem por status"),
        0
    );
}
