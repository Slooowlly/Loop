//! A janela pelo lado do JOGADOR: propostas por mérito, assentos reservados, prazo de
//! validade, a porta de saída da equipe vendida e a ambição de slam.

use super::super::*;
use super::*;
#[test]
fn test_run_market_does_not_auto_sign_player() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");

    let previous = Season::new("S001".to_string(), 1, 2024);
    let next = Season::new("S002".to_string(), 2, 2025);
    season_queries::insert_season(&conn, &previous).expect("previous season");
    season_queries::finalize_season(&conn, &previous.id).expect("finalize previous");
    season_queries::insert_season(&conn, &next).expect("next season");

    let mut team_rng = StdRng::seed_from_u64(404);
    let current_team = sample_team("mazda_rookie", "T001", &mut team_rng);
    let vacancy_team = sample_team("mazda_rookie", "T002", &mut team_rng);
    team_queries::insert_team(&conn, &current_team).expect("current team");
    team_queries::insert_team(&conn, &vacancy_team).expect("vacancy team");

    let mut player = sample_driver(
        "P001",
        "Jogador",
        Some("mazda_rookie"),
        80.0,
        DriverStatus::Ativo,
    );
    player.is_jogador = true;
    let retired = sample_driver(
        "P002",
        "Veterano",
        Some("mazda_rookie"),
        55.0,
        DriverStatus::Aposentado,
    );
    driver_queries::insert_driver(&conn, &player).expect("insert player");
    driver_queries::insert_driver(&conn, &retired).expect("insert retired");

    let player_contract = Contract::new(
        "C001".to_string(),
        player.id.clone(),
        player.nome.clone(),
        current_team.id.clone(),
        current_team.nome.clone(),
        1,
        1,
        45_000.0,
        TeamRole::Numero1,
        "mazda_rookie".to_string(),
    );
    let retired_contract = Contract::new(
        "C002".to_string(),
        retired.id.clone(),
        retired.nome.clone(),
        vacancy_team.id.clone(),
        vacancy_team.nome.clone(),
        1,
        1,
        20_000.0,
        TeamRole::Numero1,
        "mazda_rookie".to_string(),
    );
    contract_queries::insert_contract(&conn, &player_contract).expect("insert player contract");
    contract_queries::insert_contract(&conn, &retired_contract).expect("insert retired contract");

    team_queries::update_team_pilots(&conn, &current_team.id, Some(&player.id), None)
        .expect("current team lineup");
    team_queries::update_team_pilots(&conn, &vacancy_team.id, Some(&retired.id), None)
        .expect("vacancy team lineup");

    insert_standing(
        &conn,
        &previous.id,
        &player.id,
        &current_team.id,
        "mazda_rookie",
        2,
        90.0,
        1,
        1,
    );

    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         VALUES ('P001', '1', 'mazda_rookie', '2024-12-31T00:00:00', 1)",
        [],
    )
    .expect("insert player license");
    conn.execute(
        "UPDATE meta SET value = '3' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");
    conn.execute(
        "UPDATE meta SET value = '3' WHERE key = 'next_driver_id'",
        [],
    )
    .expect("driver counter");

    let mut rng = StdRng::seed_from_u64(405);
    let report = run_market(&conn, 2, &mut rng).expect("market should run");
    let active_contracts = contract_queries::get_contracts_for_pilot(&conn, &player.id)
        .expect("player contracts")
        .into_iter()
        .filter(|contract| contract.status == ContractStatus::Ativo)
        .collect::<Vec<_>>();

    assert!(
        report
            .new_signings
            .iter()
            .all(|signing| signing.driver_id != player.id),
        "o mercado não deve auto-assinar o jogador"
    );
    assert!(
        active_contracts.is_empty(),
        "o jogador não deveria ganhar contrato automático; contratos ativos: {:?}",
        active_contracts
            .iter()
            .map(|contract| (&contract.id, &contract.equipe_id, &contract.categoria))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_player_reserved_seats_holds_multiple_for_free_agent() {
    // Design do usuário: quando o jogador está sem vaga, o mercado SEGURA alguns
    // assentos (2-3) que a carteira dele alcança — só vagas vazias, nunca dispensando
    // ninguém. Com contrato ativo, não segura nada.
    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("schema");
    conn.execute(
        "UPDATE meta SET value = '10' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");

    // Dois times gt3 vazios → 4 vagas regulares abertas.
    let mut rng = StdRng::seed_from_u64(880);
    let t1 = sample_team("gt3", "T001", &mut rng);
    let t2 = sample_team("gt3", "T002", &mut rng);
    team_queries::insert_team(&conn, &t1).expect("t1");
    team_queries::insert_team(&conn, &t2).expect("t2");

    // Jogador livre, licenciado pra gt3, com último contrato em gt3 (define o tier).
    let mut player = sample_driver("P001", "Jogador", None, 80.0, DriverStatus::Ativo);
    player.is_jogador = true;
    driver_queries::insert_driver(&conn, &player).expect("player");
    let old = Contract::new(
        "C001".to_string(),
        player.id.clone(),
        player.nome.clone(),
        t1.id.clone(),
        t1.nome.clone(),
        1,
        1,
        90_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &old).expect("old contract");
    contract_queries::update_contract_status(&conn, &old.id, &ContractStatus::Rescindido)
        .expect("rescind");
    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         VALUES ('P001', '3', 'gt3', '2024-12-31T00:00:00', 2)",
        [],
    )
    .expect("license");

    let seats = player_reserved_seats(&conn, 2).expect("seats");
    assert_eq!(
        seats.len(),
        MAX_PLAYER_RESERVED_SEATS,
        "deve segurar {MAX_PLAYER_RESERVED_SEATS} assentos pro agente livre licenciado, teve {}: {seats:?}",
        seats.len()
    );
    let uniq: std::collections::HashSet<_> = seats.iter().collect();
    assert_eq!(
        uniq.len(),
        seats.len(),
        "assentos reservados nao devem repetir"
    );

    // Com contrato ATIVO → não reserva nada (o mercado roda normal).
    let active = Contract::new(
        "C002".to_string(),
        player.id.clone(),
        player.nome.clone(),
        t1.id.clone(),
        t1.nome.clone(),
        2,
        3,
        90_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &active).expect("active contract");
    team_queries::update_team_pilots(&conn, &t1.id, Some(&player.id), None).expect("seat");
    assert!(
        player_reserved_seats(&conn, 2).expect("seats2").is_empty(),
        "jogador com contrato ativo nao deve ter assentos reservados"
    );
}

#[test]
fn test_pedigree_boost_is_bounded_and_monotonic() {
    // Rookie (índice 0) não ganha nada; boost cresce com o índice e satura no teto.
    assert_eq!(pedigree_boost_from_index(0.0), 0.0);
    assert_eq!(
        pedigree_boost_from_index(-50.0),
        0.0,
        "índice negativo tratado como 0"
    );
    let mid = pedigree_boost_from_index(PEDIGREE_BOOST_SCALE); // índice = escala → metade do teto
    assert!((mid - PEDIGREE_BOOST_MAX / 2.0).abs() < 1e-9);
    let strong = pedigree_boost_from_index(4.0 * PEDIGREE_BOOST_SCALE);
    assert!(strong > mid, "mais pedigree → mais boost");
    assert!(
        pedigree_boost_from_index(1_000_000.0) < PEDIGREE_BOOST_MAX,
        "nunca ultrapassa o teto"
    );
}

#[test]
fn test_generate_player_window_proposals_courts_free_agent_by_merit() {
    // Fase A: agente livre FORTE (bate o pool) recebe proposta formal ("Proposta
    // recebida"), o assento é segurado, é idempotente (não duplica), e jogador COM
    // contrato ativo não recebe nada.
    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("schema");
    conn.execute(
        "UPDATE meta SET value = '20' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");

    let s1 = Season::new("S001".to_string(), 1, 2024);
    let s2 = Season::new("S002".to_string(), 2, 2025);
    season_queries::insert_season(&conn, &s1).expect("s1");
    season_queries::finalize_season(&conn, &s1.id).expect("finalize s1");
    season_queries::insert_season(&conn, &s2).expect("s2");

    let mut rng = StdRng::seed_from_u64(915);
    let team = sample_team("gt3", "T001", &mut rng); // vazio → vagas gt3 abertas
    team_queries::insert_team(&conn, &team).expect("team");

    // Jogador livre, forte, licenciado gt3, com standing dominante na s1 (visibilidade alta).
    let mut player = sample_driver("P001", "Jogador", Some("gt3"), 95.0, DriverStatus::Ativo);
    player.is_jogador = true;
    driver_queries::insert_driver(&conn, &player).expect("player");
    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         VALUES ('P001', '3', 'gt3', '2024-12-31T00:00:00', 3)",
        [],
    )
    .expect("license");
    insert_standing(&conn, &s1.id, &player.id, &team.id, "gt3", 1, 400.0, 10, 8);

    let held = generate_player_window_proposals(&conn, 2, 1, &mut rng).expect("gen");
    assert!(
        !held.is_empty(),
        "jogador forte livre deve receber ao menos uma proposta formal: held={held:?}"
    );
    let pending = crate::db::queries::market_proposals::get_pending_player_proposals(
        &conn, &s2.id, &player.id,
    )
    .expect("pending");
    assert!(
        !pending.is_empty(),
        "deve persistir proposta pendente pro jogador"
    );

    // Idempotência: rodar de novo não duplica (mesmo ID) e segue segurando o assento.
    let held2 = generate_player_window_proposals(&conn, 2, 1, &mut rng).expect("gen2");
    let pending2 = crate::db::queries::market_proposals::get_pending_player_proposals(
        &conn, &s2.id, &player.id,
    )
    .expect("pending2");
    assert_eq!(
        pending.len(),
        pending2.len(),
        "nao deve duplicar propostas ao rodar de novo"
    );
    assert!(
        !held2.is_empty(),
        "deve continuar segurando o assento da proposta pendente"
    );

    // Jogador COM contrato ativo → nenhuma proposta formal na Fase A.
    let contract = Contract::new(
        "C100".to_string(),
        player.id.clone(),
        player.nome.clone(),
        team.id.clone(),
        team.nome.clone(),
        2,
        3,
        100_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &contract).expect("contract");
    team_queries::update_team_pilots(&conn, &team.id, Some(&player.id), None).expect("seat");
    let held3 = generate_player_window_proposals(&conn, 2, 1, &mut rng).expect("gen3");
    assert!(
        held3.is_empty(),
        "jogador com contrato ativo nao recebe proposta formal na Fase A"
    );
}

#[test]
fn test_player_window_proposals_expire_after_ttl() {
    // Fase B: proposta criada na semana 1 (prazo = 1 + PROPOSAL_TTL_WEEKS) expira na
    // semana do prazo e não é reoferecida (o assento deixa de ser segurado).
    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("schema");
    conn.execute(
        "UPDATE meta SET value = '30' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");

    let s1 = Season::new("S001".to_string(), 1, 2024);
    let s2 = Season::new("S002".to_string(), 2, 2025);
    season_queries::insert_season(&conn, &s1).expect("s1");
    season_queries::finalize_season(&conn, &s1.id).expect("finalize s1");
    season_queries::insert_season(&conn, &s2).expect("s2");

    let mut rng = StdRng::seed_from_u64(732);
    let team = sample_team("gt3", "T001", &mut rng);
    team_queries::insert_team(&conn, &team).expect("team");

    // Titular ocupa a N1 → sobra só UMA vaga (N2), pra o teste ser determinístico.
    let teammate = sample_driver("P002", "Titular", Some("gt3"), 70.0, DriverStatus::Ativo);
    driver_queries::insert_driver(&conn, &teammate).expect("teammate");
    team_queries::update_team_pilots(&conn, &team.id, Some(&teammate.id), None).expect("lineup");
    let tc = Contract::new(
        "C001".to_string(),
        teammate.id.clone(),
        teammate.nome.clone(),
        team.id.clone(),
        team.nome.clone(),
        1,
        3,
        90_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &tc).expect("teammate contract");

    let mut player = sample_driver("P001", "Jogador", Some("gt3"), 95.0, DriverStatus::Ativo);
    player.is_jogador = true;
    driver_queries::insert_driver(&conn, &player).expect("player");
    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         VALUES ('P001', '3', 'gt3', '2024-12-31T00:00:00', 3)",
        [],
    )
    .expect("license");
    insert_standing(&conn, &s1.id, &player.id, &team.id, "gt3", 1, 400.0, 10, 8);

    let held_w1 = generate_player_window_proposals(&conn, 2, 1, &mut rng).expect("w1");
    let pending_w1 = crate::db::queries::market_proposals::get_pending_player_proposals(
        &conn, &s2.id, &player.id,
    )
    .expect("p1");
    assert_eq!(pending_w1.len(), 1, "semana 1 cria a proposta");
    assert!(!held_w1.is_empty(), "semana 1 segura o assento da proposta");

    // Semana 4 = criada(1) + TTL(3): expira e não reoferece.
    let held_w4 = generate_player_window_proposals(&conn, 2, 4, &mut rng).expect("w4");
    let pending_w4 = crate::db::queries::market_proposals::get_pending_player_proposals(
        &conn, &s2.id, &player.id,
    )
    .expect("p4");
    assert!(
        pending_w4.is_empty(),
        "proposta expira na semana do prazo e não é reoferecida"
    );
    assert!(held_w4.is_empty(), "assento expirado não é mais segurado");
}

#[test]
fn test_free_player_without_categoria_atual_still_gets_offers_at_last_level() {
    // Regressão do bug relatado: jogador que fica um tempo sem correr tem o
    // `categoria_atual` limpo pelo sync do mercado (sync.rs zera quem não tem
    // contrato regular ativo). Antes, o tier dele caía a 0 (rookie) e o feed só
    // mostrava vagas de estreia — ocupadas por rookies reais → ZERO propostas para
    // sempre, mesmo com a escada segurando um assento invisível. Agora o tier vem
    // do ÚLTIMO contrato (gt3) e ele volta a receber oferta no nível dele.
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");
    conn.execute(
        "UPDATE meta SET value = '10' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");

    // Time gt3 (tier 4) com UMA vaga aberta (só o titular ocupa um assento).
    // Nenhum time rookie no mundo: sem a correção, o jogador não veria vaga alguma.
    let mut team_rng = StdRng::seed_from_u64(770);
    let gt3_team = sample_team("gt3", "T001", &mut team_rng);
    team_queries::insert_team(&conn, &gt3_team).expect("insert gt3 team");

    // Jogador livre e SEM categoria_atual (limpo pelo sync após ficar sem correr).
    let mut player = sample_driver("P001", "Jogador", None, 80.0, DriverStatus::Ativo);
    player.is_jogador = true;
    let teammate = sample_driver(
        "P002",
        "Titular GT3",
        Some("gt3"),
        78.0,
        DriverStatus::Ativo,
    );
    driver_queries::insert_driver(&conn, &player).expect("insert player");
    driver_queries::insert_driver(&conn, &teammate).expect("insert teammate");
    team_queries::update_team_pilots(&conn, &gt3_team.id, Some(&teammate.id), None)
        .expect("gt3 lineup deixa a vaga do jogador aberta");

    // Contrato ATIVO do titular (ancora o time) + contrato PASSADO (rescindido) do
    // jogador em gt3 — a única pista da categoria dele, já que categoria_atual é NULL.
    let teammate_contract = Contract::new(
        "C001".to_string(),
        teammate.id.clone(),
        teammate.nome.clone(),
        gt3_team.id.clone(),
        gt3_team.nome.clone(),
        1,
        3,
        120_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &teammate_contract).expect("teammate contract");
    let player_old_contract = Contract::new(
        "C002".to_string(),
        player.id.clone(),
        player.nome.clone(),
        gt3_team.id.clone(),
        gt3_team.nome.clone(),
        1,
        1,
        90_000.0,
        TeamRole::Numero2,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &player_old_contract).expect("player old contract");
    contract_queries::update_contract_status(
        &conn,
        &player_old_contract.id,
        &ContractStatus::Rescindido,
    )
    .expect("rescind player contract");

    // Licença do jogador cobre gt3 (nível 3).
    conn.execute(
        "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
         VALUES ('P001', '3', 'gt3', '2024-12-31T00:00:00', 2)",
        [],
    )
    .expect("insert player license");

    // Sanidade: jogador está mesmo livre.
    assert!(
        contract_queries::get_active_regular_contract_for_pilot(&conn, &player.id)
            .expect("check contract")
            .is_none(),
        "jogador deve estar sem contrato ativo"
    );

    let offers = player_market_offers(&conn, 2).expect("offers");
    assert!(
        offers.iter().any(|o| o.category == "gt3"),
        "jogador livre sem categoria_atual deve receber oferta de gt3 (pelo último \
         contrato), não zero propostas: {offers:?}"
    );

    // E deve conseguir ASSINAR a vaga ofertada (sign consistente com a oferta).
    let seat = offers[0].seat_id.clone();
    sign_player_to_vacancy(&conn, 2, &seat)
        .expect("jogador deve conseguir assinar a vaga que lhe foi ofertada");
}

/// Mundo mínimo para a fuga: o jogador SOB CONTRATO numa gt3 que acabou de ser
/// vendida por colapso, mais uma segunda equipe da mesma categoria com vaga.
/// Devolve a conexão, a temporada NOVA e o id da equipe quebrada.
fn fixture_fuga_da_falencia(vendida: bool) -> (Connection, Season, String) {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");

    // Só a temporada NOVA entra no banco: `seasons.status` é único por ativa, e a
    // anterior só existe aqui como número (é dela que a venda é datada).
    let temporada_anterior = Season::new("S001".to_string(), 4, 2027);
    let nova = Season::new("S002".to_string(), 5, 2028);
    season_queries::insert_season(&conn, &nova).expect("season nova");

    let mut rng = StdRng::seed_from_u64(4242);
    let quebrada = sample_team("gt3", "TQUEBRA", &mut rng);
    let vizinha = sample_team("gt3", "TVIZINHA", &mut rng);
    team_queries::insert_team(&conn, &quebrada).expect("equipe quebrada");
    team_queries::insert_team(&conn, &vizinha).expect("equipe vizinha");

    let mut player = sample_driver("PLAYER", "Jogador", Some("gt3"), 72.0, DriverStatus::Ativo);
    player.is_jogador = true;
    driver_queries::insert_driver(&conn, &player).expect("jogador");

    let contrato = Contract::new(
        "CPLAYER".to_string(),
        player.id.clone(),
        player.nome.clone(),
        quebrada.id.clone(),
        quebrada.nome.clone(),
        temporada_anterior.numero,
        3, // contrato longo: sem a falência ele não estaria no mercado
        400_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &contrato).expect("contrato do jogador");

    if vendida {
        team_queries::insert_team_ownership_event(
            &conn,
            &quebrada.id,
            temporada_anterior.numero,
            temporada_anterior.ano,
            "sale",
            24_600_000.0,
            2_800_000.0,
            "venda por colapso",
        )
        .expect("evento de venda");
    }

    (conn, nova, quebrada.id)
}

#[test]
fn jogador_de_equipe_vendida_recebe_porta_de_saida() {
    // A falência não pode ser só manchete: o jogador preso a um projeto que ruiu
    // precisa de uma saída. Ele está SOB CONTRATO e não expirando — sem a
    // falência, `generate_player_proposals` devolveria vazio.
    let (conn, nova, quebrada_id) = fixture_fuga_da_falencia(true);
    let mut rng = StdRng::seed_from_u64(7);

    let propostas = generate_player_proposals(
        &conn,
        &nova.id,
        nova.numero,
        &find_vacancies(&conn).expect("vagas"),
        false, // não estava expirando
        &HashMap::new(),
        &mut rng,
    )
    .expect("propostas");

    assert!(
        !propostas.is_empty(),
        "quem perdeu a equipe para a falência tem que ver ao menos uma porta"
    );
    assert!(
        propostas.iter().all(|p| p.equipe_id != quebrada_id),
        "a equipe que quebrou não oferta a fuga dela mesma"
    );
    // E a proposta fica pendente no banco — é assim que a tela do jogador a lê.
    let pendentes: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM market_proposals WHERE piloto_id = 'PLAYER' AND status = 'Pendente'",
            [],
            |r| r.get(0),
        )
        .expect("contagem");
    assert!(pendentes > 0);
}

#[test]
fn jogador_sob_contrato_sem_falencia_nao_recebe_proposta() {
    // O contrapositivo: sem a venda, um jogador com contrato de 3 anos continua
    // fora do mercado. A porta é da falência, não um portão sempre aberto.
    let (conn, nova, _) = fixture_fuga_da_falencia(false);
    let mut rng = StdRng::seed_from_u64(7);

    let propostas = generate_player_proposals(
        &conn,
        &nova.id,
        nova.numero,
        &find_vacancies(&conn).expect("vagas"),
        false,
        &HashMap::new(),
        &mut rng,
    )
    .expect("propostas");

    assert!(
        propostas.is_empty(),
        "sem falência o jogador sob contrato não é abordado: {propostas:?}"
    );
}

#[test]
fn venda_de_temporada_antiga_nao_reabre_a_porta() {
    // A porta é do offseason DESTA virada. Uma venda de anos atrás não pode
    // deixar o jogador permanentemente no mercado.
    let (conn, nova, _) = fixture_fuga_da_falencia(true);
    let mut rng = StdRng::seed_from_u64(7);

    let propostas = generate_player_proposals(
        &conn,
        &nova.id,
        nova.numero + 3, // três temporadas depois da venda
        &find_vacancies(&conn).expect("vagas"),
        false,
        &HashMap::new(),
        &mut rng,
    )
    .expect("propostas");

    assert!(propostas.is_empty(), "venda velha não reabre o mercado");
}

#[test]
fn slam_history_feeds_brain_to_chase_next_base() {
    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("schema");
    let mut driver = sample_driver(
        "P100",
        "Chaser",
        Some("mazda_amador"),
        67.0,
        DriverStatus::Ativo,
    );
    driver.personalidade_primaria = Some(PrimaryPersonality::Ambicioso);
    driver_queries::insert_driver(&conn, &driver).expect("driver");

    let insert = |season: i32, categoria: &str, pos: i32, snap: &str| {
        conn.execute(
            "INSERT INTO driver_season_archive
             (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params!["P100", season, 2020 + season, "Chaser", categoria, pos, 120.0, snap],
        )
        .expect("archive row");
    };
    insert(
        1,
        "mazda_amador",
        1,
        r#"{"categoria":"mazda_amador","posicao_campeonato":1,"titulos":1,"corridas":8,"vitorias":4}"#,
    );
    insert(
        2,
        "toyota_amador",
        1,
        r#"{"categoria":"toyota_amador","posicao_campeonato":1,"titulos":1,"corridas":8,"vitorias":5}"#,
    );

    let (history, current_results) = read_slam_history(&conn, &driver).expect("history");
    // 2 títulos no histórico; current_results só conta a categoria atual (mazda_amador).
    assert_eq!(history.len(), 2);
    assert_eq!(current_results, vec![true]);

    // Cup precisa de mazda + toyota + bmw → o cérebro manda caçar bmw_m2.
    let decision = slam_ambition::decide(
        &history,
        "mazda_amador",
        driver.atributos.skill,
        true,
        &current_results,
    );
    match decision {
        Some(SlamDecision::Chase { category, .. }) => assert_eq!(category, "bmw_m2"),
        other => panic!("esperava Chase bmw_m2, veio {other:?}"),
    }
}

#[test]
fn interactive_window_persists_and_closes() {
    let conn = setup_market_fixture();
    let mut rng = StdRng::seed_from_u64(2);
    let season = 2;
    // Inicia + persiste a janela.
    let state = window_get_or_init(&conn, season, &mut rng).expect("init");
    let _ = state.week();
    assert!(
        load_window(&conn, season).expect("load").is_some(),
        "janela deve estar persistida após init"
    );
    // Avança até fechar (jogador sempre espera).
    let mut guard = 0;
    loop {
        let s = window_advance(&conn, season, None, &mut rng).expect("advance");
        if s.is_closed() {
            break;
        }
        guard += 1;
        assert!(guard < 30, "janela não fechou em tempo razoável");
    }
    // Persistida como fechada.
    let loaded = load_window(&conn, season)
        .expect("load")
        .expect("janela existe");
    assert!(loaded.is_closed(), "janela deve estar fechada após o ciclo");
}

