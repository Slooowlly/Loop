use rusqlite::{params, Connection};

use super::*;
use crate::db::connection::DbError;
use crate::models::contract::Contract;
use crate::models::enums::{ContractStatus, ContractType, TeamRole};

#[test]
fn test_insert_and_get_contract() {
    let conn = setup_test_db().expect("test db");
    let contract = sample_contract("C001", "P001", "T001", ContractStatus::Ativo);

    insert_contract(&conn, &contract).expect("insert contract");
    let loaded = get_contract_by_id(&conn, "C001")
        .expect("query contract")
        .expect("contract should exist");

    assert_eq!(loaded.id, "C001");
    assert_eq!(loaded.piloto_nome, contract.piloto_nome);
    assert_eq!(loaded.equipe_nome, contract.equipe_nome);
    assert_eq!(loaded.papel, TeamRole::Numero1);
}

#[test]
fn test_get_active_contract_for_pilot() {
    let conn = setup_test_db().expect("test db");
    let expired = sample_contract("C001", "P001", "T001", ContractStatus::Expirado);
    let active = sample_contract("C002", "P001", "T002", ContractStatus::Ativo);
    insert_contracts(&conn, &[expired, active]).expect("insert contracts");

    let loaded = get_active_contract_for_pilot(&conn, "P001")
        .expect("query active contract")
        .expect("active contract should exist");

    assert_eq!(loaded.id, "C002");
    assert_eq!(loaded.equipe_id, "T002");
}

#[test]
fn test_get_active_contracts_for_team() {
    let conn = setup_test_db().expect("test db");
    insert_contract(
        &conn,
        &sample_contract("C001", "P001", "T001", ContractStatus::Ativo),
    )
    .expect("insert 1");
    insert_contract(
        &conn,
        &sample_contract("C002", "P002", "T001", ContractStatus::Ativo),
    )
    .expect("insert 2");
    insert_contract(
        &conn,
        &sample_contract("C003", "P003", "T001", ContractStatus::Expirado),
    )
    .expect("insert 3");

    let contracts = get_active_contracts_for_team(&conn, "T001").expect("query team contracts");

    assert_eq!(contracts.len(), 2);
    assert!(contracts
        .iter()
        .all(|contract| contract.status == ContractStatus::Ativo));
}

#[test]
fn test_get_all_active_regular_contracts_filters_special() {
    let conn = setup_test_db().expect("test db");
    let regular = sample_contract("C001", "P001", "T001", ContractStatus::Ativo);
    let mut special = sample_contract("C002", "P002", "T002", ContractStatus::Ativo);
    special.tipo = ContractType::Especial;
    insert_contracts(&conn, &[regular.clone(), special]).expect("insert contracts");

    let contracts = get_all_active_regular_contracts(&conn).expect("query active regular");

    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0].id, regular.id);
    assert_eq!(contracts[0].tipo, ContractType::Regular);
}

#[test]
fn test_expire_ending_contracts() {
    let conn = setup_test_db().expect("test db");
    let mut contract = sample_contract("C001", "P001", "T001", ContractStatus::Ativo);
    contract.temporada_fim = 3;
    insert_contract(&conn, &contract).expect("insert contract");

    let updated = expire_ending_contracts(&conn, 3).expect("expire contracts");
    assert_eq!(updated, 1);

    let loaded = get_contract_by_id(&conn, "C001")
        .expect("query contract")
        .expect("contract should exist");
    assert_eq!(loaded.status, ContractStatus::Expirado);
}

#[test]
fn test_get_expiring_contracts() {
    let conn = setup_test_db().expect("test db");
    let mut expiring = sample_contract("C001", "P001", "T001", ContractStatus::Ativo);
    expiring.temporada_fim = 4;
    let mut long = sample_contract("C002", "P002", "T002", ContractStatus::Ativo);
    long.temporada_fim = 5;
    insert_contracts(&conn, &[expiring, long]).expect("insert contracts");

    let expiring_contracts = get_expiring_contracts(&conn, 4).expect("query expiring contracts");

    assert_eq!(expiring_contracts.len(), 1);
    assert_eq!(expiring_contracts[0].id, "C001");
}

#[test]
fn test_count_active_contracts_for_team() {
    let conn = setup_test_db().expect("test db");
    insert_contract(
        &conn,
        &sample_contract("C001", "P001", "T001", ContractStatus::Ativo),
    )
    .expect("insert 1");
    insert_contract(
        &conn,
        &sample_contract("C002", "P002", "T001", ContractStatus::Ativo),
    )
    .expect("insert 2");
    insert_contract(
        &conn,
        &sample_contract("C003", "P003", "T001", ContractStatus::Rescindido),
    )
    .expect("insert 3");

    let count = count_active_contracts_for_team(&conn, "T001").expect("count active");
    assert_eq!(count, 2);
}

#[test]
fn test_get_free_agents_for_preseason_ignores_special_contract_history() {
    let conn = setup_test_db().expect("test db");
    insert_team_stub(&conn, "T001", "#112233");
    insert_team_stub(&conn, "SP001", "#aa5500");
    insert_license_stub(&conn, "P003", 2);

    let mut regular = sample_contract("C100", "P003", "T001", ContractStatus::Expirado);
    regular.equipe_nome = "Equipe Regular".to_string();
    regular.categoria = "mazda_amador".to_string();
    regular.temporada_inicio = 2;
    regular.duracao_anos = 3;
    regular.temporada_fim = 4;
    regular.created_at = "2026-01-01T08:00:00".to_string();

    let mut special = sample_contract("C101", "P003", "SP001", ContractStatus::Expirado);
    special.equipe_nome = "Equipe Especial".to_string();
    special.tipo = ContractType::Especial;
    special.categoria = "production_challenger".to_string();
    special.classe = Some("mazda".to_string());
    special.temporada_inicio = 4;
    special.duracao_anos = 1;
    special.temporada_fim = 4;
    special.created_at = "2026-06-01T08:00:00".to_string();

    insert_contracts(&conn, &[regular, special]).expect("insert contracts");
    conn.execute(
        "INSERT INTO driver_season_archive (
            piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            "P003",
            4,
            2026,
            "Piloto 3",
            "mazda_amador",
            12,
            95.0,
            r#"{"total_pilotos":20}"#
        ],
    )
    .expect("insert archive");

    let free_agents = get_free_agents_for_preseason(&conn).expect("free agents query");
    let driver = free_agents
        .into_iter()
        .find(|agent| agent.driver_id == "P003")
        .expect("driver should be free agent");

    assert_eq!(driver.categoria, "mazda_amador");
    assert_eq!(driver.previous_team_name.as_deref(), Some("Equipe Regular"));
    assert_eq!(driver.previous_team_color.as_deref(), Some("#112233"));
    assert_eq!(driver.seasons_at_last_team, 3);
    assert_eq!(driver.last_championship_position, Some(12));
    assert_eq!(driver.last_championship_total_drivers, Some(20));
}

fn sample_contract(
    id: &str,
    piloto_id: &str,
    equipe_id: &str,
    status: ContractStatus,
) -> Contract {
    Contract {
        id: id.to_string(),
        piloto_id: piloto_id.to_string(),
        piloto_nome: format!("Piloto {}", &piloto_id[1..]),
        equipe_id: equipe_id.to_string(),
        equipe_nome: format!("Equipe {}", &equipe_id[1..]),
        temporada_inicio: 1,
        duracao_anos: 2,
        temporada_fim: 2,
        salario_anual: 100_000.0,
        papel: TeamRole::Numero1,
        status,
        tipo: ContractType::Regular,
        categoria: "gt3".to_string(),
        classe: None,
        created_at: "2026-01-01T12:00:00".to_string(),
    }
}

#[test]
fn test_unknown_contract_status_returns_error() {
    let conn = setup_test_db().expect("test db");
    conn.execute(
        "INSERT INTO contracts (
            id, piloto_id, piloto_nome, equipe_id, equipe_nome,
            temporada_inicio, duracao_anos, temporada_fim,
            salario, salario_anual, papel, status, tipo, categoria, created_at
         ) VALUES ('C_BAD', 'P001', 'Piloto 1', 'T001', 'Equipe', 1, 1, 2,
                   100000, 100000, 'Numero1', 'Suspenso', 'Regular', 'gt3', '2026-01-01')",
        [],
    )
    .expect("insert contract with unknown status");

    let result = get_contract_by_id(&conn, "C_BAD");
    assert!(
        result.is_err(),
        "status desconhecido deve retornar erro, nao default silencioso"
    );
}

#[test]
fn test_unknown_contract_role_returns_error() {
    let conn = setup_test_db().expect("test db");
    conn.execute(
        "INSERT INTO contracts (
            id, piloto_id, piloto_nome, equipe_id, equipe_nome,
            temporada_inicio, duracao_anos, temporada_fim,
            salario, salario_anual, papel, status, tipo, categoria, created_at
         ) VALUES ('C_BAD2', 'P001', 'Piloto 1', 'T001', 'Equipe', 1, 1, 2,
                   100000, 100000, 'Wildcard', 'Ativo', 'Regular', 'gt3', '2026-01-01')",
        [],
    )
    .expect("insert contract with unknown role");

    let result = get_contract_by_id(&conn, "C_BAD2");
    assert!(
        result.is_err(),
        "papel desconhecido deve retornar erro, nao default silencioso"
    );
}

#[test]
fn test_blob_in_piloto_nome_returns_error() {
    let conn = setup_test_db().expect("test db");
    conn.execute(
        "INSERT INTO contracts (
            id, piloto_id, piloto_nome, equipe_id, equipe_nome,
            temporada_inicio, duracao_anos, temporada_fim,
            salario, salario_anual, papel, status, tipo, categoria, created_at
         ) VALUES ('C_BLOB_NAME', 'P001', X'DEADBEEF', 'T001', 'Equipe', 1, 1, 2,
                   100000, 100000, 'Numero1', 'Ativo', 'Regular', 'gt3', '2026-01-01')",
        [],
    )
    .expect("insert contract with blob piloto_nome");

    let result = get_contract_by_id(&conn, "C_BLOB_NAME");
    assert!(
        result.is_err(),
        "BLOB em piloto_nome deve retornar erro, nao virar string vazia"
    );
}

#[test]
fn test_blob_in_salario_anual_returns_error() {
    let conn = setup_test_db().expect("test db");
    conn.execute(
        "INSERT INTO contracts (
            id, piloto_id, piloto_nome, equipe_id, equipe_nome,
            temporada_inicio, duracao_anos, temporada_fim,
            salario, salario_anual, papel, status, tipo, categoria, created_at
         ) VALUES ('C_BLOB_SAL', 'P001', 'Piloto 1', 'T001', 'Equipe', 1, 1, 2,
                   100000, X'DEADBEEF', 'Numero1', 'Ativo', 'Regular', 'gt3', '2026-01-01')",
        [],
    )
    .expect("insert contract with blob salario_anual");

    let result = get_contract_by_id(&conn, "C_BLOB_SAL");
    assert!(
        result.is_err(),
        "BLOB em salario_anual deve retornar erro, nao cair em fallback silencioso"
    );
}

#[test]
fn test_invalid_temporada_inicio_returns_error_instead_of_fallback() {
    let conn = setup_test_db().expect("test db");
    conn.execute(
        "INSERT INTO contracts (
            id, piloto_id, piloto_nome, equipe_id, equipe_nome,
            temporada_inicio, duracao_anos, temporada_fim,
            salario, salario_anual, papel, status, tipo, categoria, created_at
         ) VALUES ('C_BAD_SEASON', 'P001', 'Piloto 1', 'T001', 'Equipe', 'abc', 1, 2,
                   100000, 100000, 'Numero1', 'Ativo', 'Regular', 'gt3', '2026-01-01')",
        [],
    )
    .expect("insert contract with invalid temporada_inicio");

    let result = get_contract_by_id(&conn, "C_BAD_SEASON");
    assert!(
        result.is_err(),
        "temporada_inicio invalida deve retornar erro, nao cair em fallback silencioso"
    );
}

#[test]
fn test_update_contract_status_returns_not_found_for_missing_contract() {
    let conn = setup_test_db().expect("test db");

    let err = update_contract_status(&conn, "C404", &ContractStatus::Expirado)
        .expect_err("missing contract should fail");

    assert!(
        matches!(err, DbError::NotFound(_)),
        "expected not-found error, got {err:?}"
    );
}

fn setup_test_db() -> Result<Connection, DbError> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(
        "CREATE TABLE drivers (
            id TEXT PRIMARY KEY,
            nome TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'Ativo',
            is_jogador INTEGER NOT NULL DEFAULT 0,
            categoria_atual TEXT
        );
        CREATE TABLE teams (
            id TEXT PRIMARY KEY,
            cor_primaria TEXT
        );
        CREATE TABLE licenses (
            id TEXT PRIMARY KEY,
            piloto_id TEXT NOT NULL,
            nivel INTEGER NOT NULL
        );
        CREATE TABLE driver_season_archive (
            piloto_id TEXT NOT NULL,
            season_number INTEGER NOT NULL,
            ano INTEGER NOT NULL,
            nome TEXT NOT NULL,
            categoria TEXT NOT NULL,
            posicao_campeonato INTEGER,
            pontos REAL NOT NULL DEFAULT 0.0,
            snapshot_json TEXT NOT NULL,
            archived_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(piloto_id, season_number)
        );
        INSERT INTO drivers (id, nome) VALUES
            ('P001', 'Piloto 1'),
            ('P002', 'Piloto 2'),
            ('P003', 'Piloto 3');

        CREATE TABLE contracts (
            id TEXT PRIMARY KEY NOT NULL,
            piloto_id TEXT NOT NULL,
            piloto_nome TEXT NOT NULL,
            equipe_id TEXT NOT NULL,
            equipe_nome TEXT NOT NULL,
            temporada_inicio INTEGER NOT NULL,
            duracao_anos INTEGER NOT NULL,
            temporada_fim INTEGER NOT NULL,
            salario REAL NOT NULL DEFAULT 0.0,
            salario_anual REAL NOT NULL DEFAULT 0.0,
            papel TEXT NOT NULL DEFAULT 'Numero2',
            status TEXT NOT NULL DEFAULT 'Ativo',
            tipo TEXT NOT NULL DEFAULT 'Regular',
            categoria TEXT NOT NULL,
            classe TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )?;
    Ok(conn)
}

fn insert_team_stub(conn: &Connection, id: &str, cor_primaria: &str) {
    conn.execute(
        "INSERT INTO teams (id, cor_primaria) VALUES (?1, ?2)",
        params![id, cor_primaria],
    )
    .expect("insert team stub");
}

fn insert_license_stub(conn: &Connection, piloto_id: &str, nivel: i32) {
    conn.execute(
        "INSERT INTO licenses (id, piloto_id, nivel) VALUES (?1, ?2, ?3)",
        params![format!("L_{piloto_id}_{nivel}"), piloto_id, nivel],
    )
    .expect("insert license stub");
}

#[test]
fn test_get_former_teammates_requires_same_team_and_season_overlap() {
    let conn = setup_test_db().unwrap();
    let ins = |id: &str, pid: &str, nome: &str, eq: &str, ini: i32, fim: i32| {
        conn.execute(
            "INSERT INTO contracts
                (id, piloto_id, piloto_nome, equipe_id, equipe_nome,
                 temporada_inicio, duracao_anos, temporada_fim, categoria)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'gt3')",
            params![id, pid, nome, eq, "Team", ini, fim - ini + 1, fim],
        )
        .unwrap();
    };
    // Jogador P001 correu na Team A nas temporadas 1–3.
    ins("c1", "P001", "Piloto 1", "TA", 1, 3);
    // P002 na Team A nas temporadas 3–5 → sobrepõe em 3 → é ex-companheiro.
    ins("c2", "P002", "Piloto 2", "TA", 3, 5);
    // P003 na Team A nas temporadas 5–6 → NÃO sobrepõe com 1–3.
    ins("c3", "P003", "Piloto 3", "TA", 5, 6);
    // P002 também passou pela Team B, onde o jogador nunca esteve → não conta.
    ins("c4", "P002", "Piloto 2", "TB", 1, 2);

    let mates = get_former_teammates(&conn, "P001").unwrap();
    let ids: Vec<&str> = mates.iter().map(|(id, _)| id.as_str()).collect();
    assert!(ids.contains(&"P002"), "P002 dividiu a Team A na temporada 3");
    assert!(!ids.contains(&"P003"), "P003 não sobrepôs com o jogador");
    assert!(!ids.contains(&"P001"), "não inclui o próprio piloto");
    // Distinto: P002 aparece uma única vez apesar de dois contratos.
    assert_eq!(ids.iter().filter(|i| **i == "P002").count(), 1);
}
