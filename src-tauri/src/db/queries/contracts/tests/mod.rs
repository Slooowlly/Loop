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

/// `archive_driver_season` grava uma linha por piloto por temporada — inclusive
/// para quem ficou sem vaga, e nesse caso com `categoria` vazia e
/// `posicao_campeonato` nula. Se a conta de temporadas paradas tomar a linha como
/// prova de que o piloto correu, todo mundo fica com `seasons_idle = 0` e o
/// marcador de inatividade da vitrine de agentes livres nunca aparece.
#[test]
fn test_seasons_idle_ignores_archive_rows_of_seasons_without_a_grid() {
    let conn = setup_test_db().expect("test db");
    let arquivar = |piloto: &str, temporada: i32, categoria: &str, posicao: Option<i32>| {
        conn.execute(
            "INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria,
                posicao_campeonato, pontos, snapshot_json
             ) VALUES (?1, ?2, ?3, 'Piloto', ?4, ?5, 0.0, ?6)",
            params![
                piloto,
                temporada,
                2020 + temporada,
                categoria,
                posicao,
                r#"{"total_pilotos":12}"#
            ],
        )
        .expect("insert archive");
    };

    // P001 correu a temporada 3 e ficou fora da 4 — uma temporada parado.
    arquivar("P001", 3, "mazda_amador", Some(5));
    arquivar("P001", 4, "", None);
    // P002 correu a última temporada arquivada — agente fresco.
    arquivar("P002", 4, "mazda_amador", Some(2));
    // P003 nunca competiu: só linhas de temporada sem grid.
    arquivar("P003", 3, "", None);
    arquivar("P003", 4, "", None);

    let agentes = get_free_agents_for_preseason(&conn).expect("free agents query");
    let idle = |id: &str| {
        agentes
            .iter()
            .find(|agent| agent.driver_id == id)
            .expect("piloto deveria estar entre os agentes livres")
            .seasons_idle
    };

    assert_eq!(idle("P001"), Some(1));
    assert_eq!(idle("P002"), Some(0));
    assert_eq!(idle("P003"), None);
}

/// Rescindir encurta a vigencia, senao o contrato segue "valendo" no banco por
/// temporadas que o piloto nunca cumpriu por aquela equipe — e quem le contrato
/// por vigencia (curva de mercado, categoria do agente livre) desenha um vinculo
/// que ja nao existe.
#[test]
fn test_rescinding_a_contract_trims_it_to_the_current_season() {
    let conn = setup_test_db().expect("test db");
    conn.execute_batch(
        "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO meta (key, value) VALUES ('current_season', '3');",
    )
    .expect("meta com temporada corrente");
    let mut contrato = sample_contract("C001", "P001", "T001", ContractStatus::Ativo);
    contrato.temporada_inicio = 2;
    contrato.temporada_fim = 5;
    insert_contract(&conn, &contrato).expect("insert contract");

    update_contract_status(&conn, "C001", &ContractStatus::Rescindido).expect("rescinde");

    let loaded = get_contract_by_id(&conn, "C001")
        .expect("query")
        .expect("contrato");
    assert_eq!(loaded.status, ContractStatus::Rescindido);
    assert_eq!(
        loaded.temporada_fim, 3,
        "a vigencia para na temporada em que o vinculo acabou",
    );
    assert_eq!(loaded.temporada_inicio, 2, "o inicio e historico, nao muda");
}

/// Contrato ainda nao comecado fica intacto: cortar `temporada_fim` para antes do
/// inicio inverteria a janela, e uma vigencia de tras para frente e pior que uma
/// vigencia longa demais.
#[test]
fn test_rescinding_a_future_contract_leaves_the_window_alone() {
    let conn = setup_test_db().expect("test db");
    conn.execute_batch(
        "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO meta (key, value) VALUES ('current_season', '3');",
    )
    .expect("meta com temporada corrente");
    let mut contrato = sample_contract("C001", "P001", "T001", ContractStatus::Ativo);
    contrato.temporada_inicio = 4;
    contrato.temporada_fim = 6;
    insert_contract(&conn, &contrato).expect("insert contract");

    update_contract_status(&conn, "C001", &ContractStatus::Rescindido).expect("rescinde");

    let loaded = get_contract_by_id(&conn, "C001")
        .expect("query")
        .expect("contrato");
    assert_eq!(loaded.temporada_inicio, 4);
    assert_eq!(loaded.temporada_fim, 6);
}

/// Save sem `meta` (ou meio migrado) nao pode derrubar a rescisao: o corte e um
/// extra, e o status e que e a operacao.
#[test]
fn test_rescinding_without_meta_table_still_updates_the_status() {
    let conn = setup_test_db().expect("test db");
    let contrato = sample_contract("C001", "P001", "T001", ContractStatus::Ativo);
    insert_contract(&conn, &contrato).expect("insert contract");

    update_contract_status(&conn, "C001", &ContractStatus::Rescindido).expect("rescinde");

    let loaded = get_contract_by_id(&conn, "C001")
        .expect("query")
        .expect("contrato");
    assert_eq!(loaded.status, ContractStatus::Rescindido);
    assert_eq!(loaded.temporada_fim, 2, "sem temporada corrente, sem corte");
}

fn sample_contract(id: &str, piloto_id: &str, equipe_id: &str, status: ContractStatus) -> Contract {
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

        -- `temporada_inicio` e `temporada_fim` sao TEXT aqui porque sao TEXT na
        -- tabela real (db::migrations::baseline). Enquanto o fixture as declarava
        -- INTEGER, a suite nao conseguia ver a comparacao lexicografica: as
        -- temporadas 9, 10, 12 e 26 ordenavam certo por acidente do tipo.
        CREATE TABLE contracts (
            id TEXT PRIMARY KEY NOT NULL,
            piloto_id TEXT NOT NULL,
            piloto_nome TEXT NOT NULL,
            equipe_id TEXT NOT NULL,
            equipe_nome TEXT NOT NULL,
            temporada_inicio TEXT NOT NULL,
            duracao_anos INTEGER NOT NULL,
            temporada_fim TEXT NOT NULL,
            salario REAL NOT NULL DEFAULT 0.0,
            salario_anual REAL NOT NULL DEFAULT 0.0,
            papel TEXT NOT NULL DEFAULT 'Numero2',
            status TEXT NOT NULL DEFAULT 'Ativo',
            tipo TEXT NOT NULL DEFAULT 'Regular',
            categoria TEXT NOT NULL,
            classe TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        -- Um unico contrato ativo por (piloto, tipo), como no schema real. Sem ele
        -- o fixture aceitaria dois Regulares ativos no mesmo piloto e um teste
        -- poderia se apoiar num estado que o banco de verdade recusa.
        CREATE UNIQUE INDEX idx_contracts_active_pilot_tipo
            ON contracts(piloto_id, tipo)
            WHERE status = 'Ativo';",
    )?;
    Ok(conn)
}

/// Contrato com vigencia explicita, para os casos de temporada de dois digitos.
fn contrato_em(
    id: &str,
    piloto_id: &str,
    equipe_id: &str,
    status: ContractStatus,
    inicio: i32,
    fim: i32,
) -> Contract {
    let mut contract = sample_contract(id, piloto_id, equipe_id, status);
    contract.temporada_inicio = inicio;
    contract.temporada_fim = fim;
    contract.duracao_anos = fim - inicio + 1;
    contract
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
    // Status 'Expirado' em todos: ex-companheiro é vínculo encerrado, e o fixture
    // agora carrega o índice único de um contrato ativo por (piloto, tipo) — P002
    // tem dois contratos aqui e os dois não podem estar ativos.
    let ins = |id: &str, pid: &str, nome: &str, eq: &str, ini: i32, fim: i32| {
        conn.execute(
            "INSERT INTO contracts
                (id, piloto_id, piloto_nome, equipe_id, equipe_nome,
                 temporada_inicio, duracao_anos, temporada_fim, categoria, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'gt3', 'Expirado')",
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
    assert!(
        ids.contains(&"P002"),
        "P002 dividiu a Team A na temporada 3"
    );
    assert!(!ids.contains(&"P003"), "P003 não sobrepôs com o jogador");
    assert!(!ids.contains(&"P001"), "não inclui o próprio piloto");
    // Distinto: P002 aparece uma única vez apesar de dois contratos.
    assert_eq!(ids.iter().filter(|i| **i == "P002").count(), 1);
}

// --------------------------------------------------------------------------
// Temporadas de dois dígitos: `temporada_inicio` e `temporada_fim` são colunas
// TEXT, então comparação e ordenação sem `CAST(... AS INTEGER)` são
// lexicográficas — e aí '10' < '9' e '26' < '9'. Os casos abaixo usam 9, 10, 12
// e 26 justamente porque é o menor conjunto em que a ordem lexicográfica e a
// numérica discordam nas duas direções.
// --------------------------------------------------------------------------

/// O histórico do piloto sai em ordem cronológica de verdade. Em TEXT puro a
/// temporada 9 encabeçava a lista e a 26 caía para o meio.
#[test]
fn historico_do_piloto_ordena_temporadas_de_dois_digitos_numericamente() {
    let conn = setup_test_db().expect("test db");
    insert_contracts(
        &conn,
        &[
            contrato_em("C09", "P001", "T009", ContractStatus::Expirado, 9, 9),
            contrato_em("C26", "P001", "T026", ContractStatus::Ativo, 26, 27),
            contrato_em("C10", "P001", "T010", ContractStatus::Expirado, 10, 11),
            contrato_em("C12", "P001", "T012", ContractStatus::Expirado, 12, 25),
        ],
    )
    .expect("insert contracts");

    let ids: Vec<String> = get_contracts_for_pilot(&conn, "P001")
        .expect("historico do piloto")
        .into_iter()
        .map(|contrato| contrato.id)
        .collect();

    assert_eq!(
        ids,
        vec!["C26", "C12", "C10", "C09"],
        "a ordem é da temporada mais recente para a mais antiga, e 26 > 12 > 10 > 9",
    );
}

/// Com contrato duplo (Regular + Especial), o contrato ativo "mais recente" é o
/// de temporada maior. Em TEXT o Especial da temporada 9 ganhava do Regular da 26.
#[test]
fn contrato_ativo_do_piloto_escolhe_a_temporada_maior_e_nao_a_string_maior() {
    let conn = setup_test_db().expect("test db");
    let regular = contrato_em("C_REG", "P001", "T026", ContractStatus::Ativo, 26, 27);
    let mut especial = contrato_em("C_ESP", "P001", "T009", ContractStatus::Ativo, 9, 9);
    especial.tipo = ContractType::Especial;
    especial.classe = Some("mazda".to_string());
    insert_contracts(&conn, &[regular, especial]).expect("insert contracts");

    let ativo = get_active_contract_for_pilot(&conn, "P001")
        .expect("contrato ativo")
        .expect("piloto tem contrato ativo");
    assert_eq!(
        ativo.id, "C_REG",
        "a temporada 26 é mais recente que a 9, mesmo com '26' < '9' em texto",
    );

    let regular_ativo = get_active_regular_contract_for_pilot(&conn, "P001")
        .expect("regular ativo")
        .expect("piloto tem regular ativo");
    assert_eq!(regular_ativo.id, "C_REG");
    assert_eq!(regular_ativo.temporada_inicio, 26);

    let especial_ativo = get_active_especial_contract_for_pilot(&conn, "P001")
        .expect("especial ativo")
        .expect("piloto tem especial ativo");
    assert_eq!(especial_ativo.id, "C_ESP");
    assert_eq!(especial_ativo.temporada_inicio, 9);
}

/// Sobreposição verdadeira que a comparação em TEXT apagava: 9–26 contra 10–12.
/// O teste de fim (`'12' >= '9'`) dava falso e o companheiro real sumia da lista.
#[test]
fn ex_companheiros_mantem_sobreposicao_real_entre_as_temporadas_9_e_26() {
    let conn = setup_test_db().expect("test db");
    insert_contracts(
        &conn,
        &[
            contrato_em("C1", "P001", "TA", ContractStatus::Expirado, 9, 26),
            contrato_em("C2", "P002", "TA", ContractStatus::Expirado, 10, 12),
            // Mesmas temporadas, equipe diferente: nunca dividiu garagem.
            contrato_em("C3", "P003", "TB", ContractStatus::Expirado, 10, 12),
        ],
    )
    .expect("insert contracts");

    let ids: Vec<String> = get_former_teammates(&conn, "P001")
        .expect("ex-companheiros")
        .into_iter()
        .map(|(id, _)| id)
        .collect();

    assert!(
        ids.contains(&"P002".to_string()),
        "P002 correu pela TA nas temporadas 10 a 12, dentro da janela 9 a 26 do jogador",
    );
    assert!(!ids.contains(&"P003".to_string()), "equipe diferente");
}

/// Sobreposição falsa que a comparação em TEXT inventava: 1–9 contra 12–26.
/// `'12' <= '9'` dava verdadeiro e um piloto que chegou depois virava ex-companheiro.
#[test]
fn ex_companheiros_nao_inventa_sobreposicao_entre_as_temporadas_9_e_12() {
    let conn = setup_test_db().expect("test db");
    insert_contracts(
        &conn,
        &[
            contrato_em("C1", "P001", "TA", ContractStatus::Expirado, 1, 9),
            contrato_em("C2", "P002", "TA", ContractStatus::Expirado, 12, 26),
            // Encostou na janela pela borda: a temporada 9 é a última do jogador.
            contrato_em("C3", "P003", "TA", ContractStatus::Expirado, 9, 10),
        ],
    )
    .expect("insert contracts");

    let ids: Vec<String> = get_former_teammates(&conn, "P001")
        .expect("ex-companheiros")
        .into_iter()
        .map(|(id, _)| id)
        .collect();

    assert!(
        !ids.contains(&"P002".to_string()),
        "P002 só chegou na temporada 12, três anos depois de o jogador sair",
    );
    assert!(
        ids.contains(&"P003".to_string()),
        "a temporada 9 é comum aos dois, a sobreposição de um ano vale",
    );
}

/// A ex-equipe e a ex-categoria do agente livre vêm do contrato de vigência mais
/// recente. Em TEXT o vínculo da temporada 9 se passava pelo mais recente e a
/// vitrine mostrava a equipe errada.
#[test]
fn ex_equipe_e_categoria_do_agente_livre_vem_da_temporada_mais_recente() {
    let conn = setup_test_db().expect("test db");
    insert_team_stub(&conn, "T009", "#090909");
    insert_team_stub(&conn, "T026", "#262626");

    let mut antigo = contrato_em("C09", "P003", "T009", ContractStatus::Expirado, 9, 9);
    antigo.equipe_nome = "Equipe da Nona".to_string();
    antigo.categoria = "mazda_amador".to_string();
    let mut recente = contrato_em("C26", "P003", "T026", ContractStatus::Expirado, 10, 26);
    recente.equipe_nome = "Equipe da Vigesima Sexta".to_string();
    recente.categoria = "bmw_m2".to_string();
    insert_contracts(&conn, &[antigo, recente]).expect("insert contracts");

    let agente = get_free_agents_for_preseason(&conn)
        .expect("agentes livres")
        .into_iter()
        .find(|agente| agente.driver_id == "P003")
        .expect("P003 está sem contrato regular ativo");

    assert_eq!(agente.categoria, "bmw_m2");
    assert_eq!(
        agente.previous_team_name.as_deref(),
        Some("Equipe da Vigesima Sexta"),
    );
    assert_eq!(agente.previous_team_color.as_deref(), Some("#262626"));
    assert_eq!(
        agente.seasons_at_last_team, 17,
        "as temporadas 10 a 26 na mesma equipe",
    );
}

/// A expiração do bloco especial filtra por temporada. Pelo caminho normal de
/// escrita o `= ?1` sem CAST acerta por acaso: a coluna é TEXT, o parâmetro
/// inteiro herda essa afinidade e '26' casa com '26'. O acerto depende de os dois
/// lados escreverem o número igual, e a coluna TEXT não garante isso — o terceiro
/// contrato abaixo grava a mesma temporada 26 como '026' e só o CAST o encontra.
#[test]
fn expiracao_do_bloco_especial_casa_a_temporada_numericamente() {
    let conn = setup_test_db().expect("test db");
    let mut da_temporada_26 = contrato_em("C26", "P001", "T026", ContractStatus::Ativo, 26, 26);
    da_temporada_26.tipo = ContractType::Especial;
    let mut da_temporada_9 = contrato_em("C09", "P002", "T009", ContractStatus::Ativo, 9, 9);
    da_temporada_9.tipo = ContractType::Especial;
    let mut com_zero_a_esquerda =
        contrato_em("C26Z", "P003", "T026", ContractStatus::Ativo, 26, 26);
    com_zero_a_esquerda.tipo = ContractType::Especial;
    insert_contracts(
        &conn,
        &[da_temporada_26, da_temporada_9, com_zero_a_esquerda],
    )
    .expect("insert contracts");
    conn.execute(
        "UPDATE contracts SET temporada_inicio = '026' WHERE id = 'C26Z'",
        [],
    )
    .expect("grava a temporada com zero a esquerda");

    let expirados = expire_especial_contracts(&conn, 26).expect("expira bloco especial");

    assert_eq!(expirados, 2, "as duas grafias da temporada 26 fecham");
    assert_eq!(
        get_contract_by_id(&conn, "C26Z")
            .expect("query")
            .expect("contrato")
            .status,
        ContractStatus::Expirado,
        "'026' é a temporada 26 escrita de outro jeito",
    );
    assert_eq!(
        get_contract_by_id(&conn, "C26")
            .expect("query")
            .expect("contrato")
            .status,
        ContractStatus::Expirado,
    );
    assert_eq!(
        get_contract_by_id(&conn, "C09")
            .expect("query")
            .expect("contrato")
            .status,
        ContractStatus::Ativo,
        "o bloco de outra temporada continua de pé",
    );
}

/// Toda coluna da projeção de contrato existe na tabela real.
#[test]
fn a_projecao_de_contrato_existe_no_schema_real() {
    crate::db::queries::tests_projecoes::a_projecao_existe_no_schema_real(
        "contracts",
        "COLUNAS_CONTRACT",
        super::mapeamento::COLUNAS_CONTRACT,
    );
}
