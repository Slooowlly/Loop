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

/// Mundo mínimo da garantia de porta: uma temporada, um time vazio da categoria e o
/// jogador agente livre. Devolve a conexão e o id do time.
///
/// `financas` fixa a saúde da equipe em MESES de operação (B22), como `(caixa, dívida)`
/// — o estado sai daí pela fonte canônica (`refresh_team_financial_state`), em vez de uma
/// string escrita à mão que poderia não bater com o caixa. `None` deixa o template.
fn fixture_garantia_de_porta(
    categoria: &str,
    team_id: &str,
    skill: f64,
    midia: f64,
    licenca: Option<u8>,
    financas: Option<(f64, f64)>,
) -> (Connection, String) {
    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("schema");
    conn.execute(
        "UPDATE meta SET value = '10' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");
    season_queries::insert_season(&conn, &Season::new("S002".to_string(), 2, 2025))
        .expect("season");

    let mut rng = StdRng::seed_from_u64(4242);
    let mut team = sample_team(categoria, team_id, &mut rng);
    if let Some((caixa, divida)) = financas {
        let mensal = crate::finance::state::custo_operacional_mensal(
            &team.categoria,
            team.classe.as_deref(),
        );
        team.cash_balance = caixa * mensal;
        team.debt_balance = divida * mensal;
        crate::finance::state::refresh_team_financial_state(&mut team);
    }
    team_queries::insert_team(&conn, &team).expect("team");

    let mut player = sample_driver(
        "P001",
        "Jogador",
        Some(categoria),
        skill,
        DriverStatus::Ativo,
    );
    player.is_jogador = true;
    player.atributos.midia = midia;
    driver_queries::insert_driver(&conn, &player).expect("player");
    if let Some(nivel) = licenca {
        conn.execute(
            "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
             VALUES ('P001', ?1, ?2, '2024-12-31T00:00:00', 2)",
            params![nivel.to_string(), categoria],
        )
        .expect("license");
    }
    (conn, team.id)
}

/// O que a Janela MOSTRA para o assento em que o jogador acabou sentando, e o que ficou
/// PERSISTIDO no contrato. Os dois têm que ser o mesmo número.
fn mostrado_e_persistido(conn: &Connection) -> (f64, f64, bool) {
    let ofertas = player_market_offers(conn, 2).expect("ofertas");
    ensure_player_seated(conn, 2).expect("garantia de porta");
    let contrato = contract_queries::get_active_regular_contract_for_pilot(conn, "P001")
        .expect("contrato")
        .expect("a garantia de porta tem que sentar o jogador");
    let assento = format!("{}#{}", contrato.equipe_id, contrato.papel.as_str());
    let oferta = ofertas
        .iter()
        .find(|oferta| oferta.seat_id == assento)
        .unwrap_or_else(|| panic!("assento {assento} tinha que estar ofertado: {ofertas:?}"));
    (
        oferta.salary,
        contrato.salario_anual,
        matches!(contrato.papel, TeamRole::Numero1),
    )
}

#[test]
fn garantia_de_porta_paga_o_mesmo_que_a_oferta_no_rookie() {
    // A3: a garantia de porta usava fórmula própria (`12k + skill*1.8k`), que ignora a
    // categoria — no rookie ela prometia ~111k onde a faixa paga ~5k–21k. Agora o valor
    // sai da MESMA fonte da oferta mostrada na Janela.
    let (conn, team_id) = fixture_garantia_de_porta("mazda_rookie", "TROOK", 55.0, 0.0, None, None);
    let (mostrado, persistido, is_n1) = mostrado_e_persistido(&conn);

    assert!(
        (mostrado - persistido).abs() < 1e-6,
        "oferta mostrada ({mostrado}) e contrato ({persistido}) têm que coincidir"
    );
    assert!(
        (persistido - player_offer_salary(0, is_n1, 55.0, &team_id)).abs() < 1e-6,
        "o valor tem que sair da fonte de oferta do tier 0, sem prêmio (fama 0)"
    );
    let formula_antiga = 12_000.0 + 55.0 * 1_800.0;
    assert!(
        persistido < formula_antiga * 0.5,
        "o rookie não pode assinar no valor da fórmula antiga ({formula_antiga}): saiu {persistido}"
    );
    assert!(persistido >= 5_000.0, "piso salarial preservado");
}

#[test]
fn garantia_de_porta_paga_o_mesmo_que_a_oferta_na_categoria_alta() {
    // Mesma fonte no topo da escada, e com o prêmio de interesse ativo aplicado: um
    // jogador de fama alta é cobiçado pelo time da categoria, então a oferta mostrada já
    // vem com o prêmio — o contrato tem que trazer o mesmo número, não a base.
    // Caixa de sobra (24 meses de operação): o teto financeiro do B22 fica acima do valor
    // de mercado, então este caso mede a fórmula e o prêmio, sem o teto no meio.
    let (conn, team_id) =
        fixture_garantia_de_porta("gt3", "TGT3", 88.0, 95.0, Some(3), Some((24.0, 0.0)));
    let interessados = player_active_interest_teams(
        &conn,
        &driver_queries::get_player_driver(&conn).expect("player"),
    )
    .expect("interesse");
    assert!(
        interessados.iter().any(|(id, _, _)| id == &team_id),
        "fama 95 tem que gerar interesse ativo do time gt3"
    );

    let (mostrado, persistido, is_n1) = mostrado_e_persistido(&conn);
    assert!(
        (mostrado - persistido).abs() < 1e-6,
        "oferta mostrada ({mostrado}) e contrato ({persistido}) têm que coincidir"
    );
    let base_sem_premio = player_offer_salary(4, is_n1, 88.0, &team_id);
    assert!(
        (persistido - base_sem_premio * crate::fame::ACTIVE_INTEREST_SALARY_PREMIUM).abs() < 1e-6,
        "o prêmio de interesse ativo tem que estar no contrato: base={base_sem_premio}, contrato={persistido}"
    );
}

// ── B22: o teto financeiro das ofertas ao jogador ────────────────────────────
//
// A oferta passiva e a assinatura na tela usavam faixa por tier/skill/equipe sem olhar o
// caixa, enquanto a proposta formal da MESMA equipe passava pelo teto financeiro
// (`calculate_offer_salary_from_money` fecha em `calculate_salary_ceiling`). Aqui se mede
// o teto entrando no caminho do jogador, e o da IA seguindo intacto.
//
// Onde o teto MORDE, medido no gt3 com skill 88 (valor de mercado ~378k, ~491k com o
// prêmio de interesse): saudável ~815k, pressionada ~540k, crise ~495k, colapso ~293k.
// Isto é, sem prêmio o corte só aparece no colapso; com prêmio ele já aparece na crise.
// O teto é o LIMITE de sustentação, não o preço — a IA continua pagando menos que ele.

/// Um caso financeiro do B22, medido no MESMO assento que a Janela ofertou.
struct CasoB22 {
    estado: String,
    /// Valor de mercado do jogador: fórmula-base × prêmio de interesse, sem teto.
    mercado: f64,
    teto: f64,
    interesse_ativo: bool,
    mostrado: f64,
    assinado: f64,
}

/// Roda o caminho inteiro do jogador numa equipe gt3 com `(caixa, dívida)` em MESES de
/// operação: lê a oferta passiva, assina por `sign_player_to_vacancy` e devolve os
/// números do assento. Mercado e teto saem da vaga ANTES da assinatura — depois dela o
/// assento não é mais vaga.
fn caso_b22(caixa: f64, divida: f64, midia: f64) -> CasoB22 {
    const SKILL: f64 = 88.0;
    let (conn, _) =
        fixture_garantia_de_porta("gt3", "TGT3", SKILL, midia, Some(3), Some((caixa, divida)));
    let oferta = player_market_offers(&conn, 2)
        .expect("ofertas")
        .first()
        .cloned()
        .expect("o mundo mínimo tem uma oferta");
    let vaga = find_vacancies(&conn)
        .expect("vagas")
        .into_iter()
        .find(|v| format!("{}#{}", v.team_id, v.papel_necessario.as_str()) == oferta.seat_id)
        .expect("a vaga ofertada tem que existir");

    let time = crate::market::team_ai::vacancy_as_finance_team(&vaga);
    let is_n1 = matches!(vaga.papel_necessario, TeamRole::Numero1);
    let base = player_offer_salary(vaga.category_tier, is_n1, SKILL, &vaga.team_id);
    let interesse_ativo = player_active_interest_teams(
        &conn,
        &driver_queries::get_player_driver(&conn).expect("player"),
    )
    .expect("interesse")
    .iter()
    .any(|(id, _, _)| id == &vaga.team_id);
    let mercado = if interesse_ativo {
        base * crate::fame::ACTIVE_INTEREST_SALARY_PREMIUM
    } else {
        base
    };

    sign_player_to_vacancy(&conn, 2, &oferta.seat_id).expect("assinatura");
    let contrato = contract_queries::get_active_regular_contract_for_pilot(&conn, "P001")
        .expect("contrato")
        .expect("o jogador tem que ter assinado");

    CasoB22 {
        estado: time.financial_state.clone(),
        mercado,
        teto: crate::finance::salary::calculate_salary_ceiling(&time),
        interesse_ativo,
        mostrado: oferta.salary,
        assinado: contrato.salario_anual,
    }
}

#[test]
fn oferta_passiva_ao_jogador_respeita_o_teto_financeiro_da_equipe() {
    // Os quatro degraus de saúde financeira, do caixa cheio ao buraco. Em todos, o que a
    // Janela mostra é o que o contrato grava, e nenhum contrato passa do teto da equipe.
    let mut assinados: Vec<f64> = Vec::new();
    for (caixa, divida, estado) in [
        (18.0, 0.0, "healthy"),
        (4.0, 0.0, "pressured"),
        (2.0, 1.5, "crisis"),
        (0.0, 24.0, "collapse"),
    ] {
        let caso = caso_b22(caixa, divida, 0.0);
        let (mercado, teto, mostrado, assinado) =
            (caso.mercado, caso.teto, caso.mostrado, caso.assinado);
        assert_eq!(
            caso.estado, estado,
            "caixa={caixa} dívida={divida} tinha que cair no estado {estado}"
        );
        assert!(
            (mostrado - assinado).abs() < 1e-6,
            "{estado}: mostrado ({mostrado}) e assinado ({assinado}) têm que coincidir"
        );
        assert!(
            (assinado - mercado.clamp(5_000.0, teto)).abs() < 1e-6,
            "{estado}: o salário tem que ser o valor de mercado ({mercado}) limitado pelo teto ({teto}), e saiu {assinado}"
        );
        assert!(
            assinado <= teto + 1e-6,
            "{estado}: nenhuma equipe assina acima do próprio teto ({teto}): saiu {assinado}"
        );
        assert!(assinado >= 5_000.0, "{estado}: piso salarial preservado");
        assinados.push(assinado);
    }

    let (saudavel, pressionada, crise, colapso) =
        (assinados[0], assinados[1], assinados[2], assinados[3]);
    assert!(
        saudavel >= pressionada && pressionada >= crise && crise >= colapso,
        "menos caixa não pode pagar mais: saudável={saudavel}, pressionada={pressionada}, crise={crise}, colapso={colapso}"
    );
    assert!(
        colapso < saudavel,
        "a equipe sem caixa tem que assinar por MENOS que a saudável: colapso={colapso}, saudável={saudavel}"
    );
}

#[test]
fn equipe_saudavel_continua_pagando_o_valor_de_mercado_ao_jogador() {
    // O teto não pode virar corte geral: com caixa, a fórmula-base chega inteira.
    let caso = caso_b22(18.0, 0.0, 0.0);
    let (mercado, teto, assinado) = (caso.mercado, caso.teto, caso.assinado);

    assert_eq!(caso.estado, "healthy");
    assert!(
        teto > mercado,
        "equipe com 18 meses de caixa tem que ter teto ({teto}) acima do valor de mercado ({mercado})"
    );
    assert!(
        (assinado - mercado).abs() < 1e-6,
        "sem aperto financeiro o valor de mercado ({mercado}) tem que chegar inteiro ao contrato: saiu {assinado}"
    );
}

#[test]
fn equipe_em_crise_apara_ate_o_premio_de_interesse_do_jogador() {
    // O prêmio de interesse ativo é parte do valor de mercado, e o teto vem DEPOIS dele:
    // quem cobiça o nome mas não tem caixa não passa a assinar acima do que sustenta.
    let caso = caso_b22(2.0, 1.5, 95.0);
    let (mercado, teto, mostrado, assinado) =
        (caso.mercado, caso.teto, caso.mostrado, caso.assinado);

    assert_eq!(caso.estado, "crisis");
    assert!(
        caso.interesse_ativo,
        "fama 95 tem que gerar interesse ativo do time gt3"
    );
    assert!(
        (mostrado - assinado).abs() < 1e-6,
        "mostrado ({mostrado}) e assinado ({assinado}) têm que coincidir também com prêmio"
    );
    assert!(
        mercado > teto,
        "o caso só mede o corte se o valor com prêmio ({mercado}) passar do teto ({teto})"
    );
    assert!(
        (assinado - teto).abs() < 1e-6,
        "a equipe em crise tem que parar no próprio teto ({teto}), e saiu {assinado}"
    );
}

#[test]
fn ia_contra_ia_nao_sente_o_teto_do_jogador() {
    // Controle do B22: a proposta de uma equipe a um piloto da IA continua saindo só da
    // fonte de sempre (`calculate_offer_salary_from_money` ± a variação de 15%), com
    // caixa ou sem caixa. O teto do jogador não entrou nesse caminho.
    for (caixa, divida) in [(18.0, 0.0), (0.0, 24.0)] {
        let (conn, _) =
            fixture_garantia_de_porta("gt3", "TGT3", 88.0, 0.0, Some(3), Some((caixa, divida)));
        let vaga = find_vacancies(&conn)
            .expect("vagas")
            .into_iter()
            .next()
            .expect("vaga");
        let piloto = sample_driver("IA01", "Piloto IA", Some("gt3"), 76.0, DriverStatus::Ativo);
        let esperado = crate::finance::salary::calculate_offer_salary_from_money(
            &crate::market::team_ai::vacancy_as_finance_team(&vaga),
            piloto.atributos.skill,
        );
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..25 {
            let oferta = calculate_offer_salary(&vaga, &piloto, &mut rng);
            assert!(
                oferta >= (esperado * 0.85).max(5_000.0) - 1e-6 && oferta <= esperado * 1.15 + 1e-6,
                "caixa={caixa} dívida={divida}: a oferta da IA ({oferta}) saiu da banda da fonte de dinheiro ({esperado})"
            );
        }
    }
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

    let held = generate_player_window_proposals(&conn, 2, 5, &mut rng).expect("gen");
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
    let held2 = generate_player_window_proposals(&conn, 2, 5, &mut rng).expect("gen2");
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
    let held3 = generate_player_window_proposals(&conn, 2, 5, &mut rng).expect("gen3");
    assert!(
        held3.is_empty(),
        "jogador com contrato ativo nao recebe proposta formal na Fase A"
    );
}

#[test]
fn a_proposta_so_nasce_quando_o_jogador_e_a_primeira_escolha_da_vaga() {
    // A regra da escassez: com um agente livre MELHOR disputando o mesmo assento, a
    // equipe prefere o outro e o jogador não recebe proposta — ele é interesse, não
    // escolha. Quando o rival sai do mercado, o jogador vira a primeira escolha ainda
    // livre e a proposta nasce. É assim que o momento da proposta passa a vir do mundo.
    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("schema");
    conn.execute(
        "UPDATE meta SET value = '40' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");

    let s1 = Season::new("S001".to_string(), 1, 2024);
    let s2 = Season::new("S002".to_string(), 2, 2025);
    season_queries::insert_season(&conn, &s1).expect("s1");
    season_queries::finalize_season(&conn, &s1.id).expect("finalize s1");
    season_queries::insert_season(&conn, &s2).expect("s2");

    let mut rng = StdRng::seed_from_u64(4411);
    let team = sample_team("gt3", "T001", &mut rng);
    team_queries::insert_team(&conn, &team).expect("team");

    // Titular ocupa a N1 → sobra UMA vaga, para a disputa ser de um assento só.
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

    // Jogador competente (80) e um agente livre claramente melhor (98), ambos livres,
    // licenciados e com currículo na gt3.
    let mut player = sample_driver("P001", "Jogador", Some("gt3"), 80.0, DriverStatus::Ativo);
    player.is_jogador = true;
    driver_queries::insert_driver(&conn, &player).expect("player");
    let rival = sample_driver("P003", "Rival", Some("gt3"), 98.0, DriverStatus::Ativo);
    driver_queries::insert_driver(&conn, &rival).expect("rival");
    for id in ["P001", "P003"] {
        conn.execute(
            "INSERT INTO licenses (piloto_id, nivel, categoria_origem, data_obtencao, temporadas_na_categoria)
             VALUES (?1, '3', 'gt3', '2024-12-31T00:00:00', 3)",
            params![id],
        )
        .expect("license");
    }
    insert_standing(&conn, &s1.id, &rival.id, &team.id, "gt3", 1, 400.0, 10, 8);
    insert_standing(&conn, &s1.id, &player.id, &team.id, "gt3", 2, 300.0, 3, 2);

    let com_rival = generate_player_window_proposals(&conn, 2, 5, &mut rng).expect("com rival");
    let pendentes = crate::db::queries::market_proposals::get_pending_player_proposals(
        &conn, &s2.id, &player.id,
    )
    .expect("pendentes");
    assert!(
        pendentes.is_empty() && com_rival.is_empty(),
        "com um agente livre melhor na praça a equipe não propõe ao jogador: held={com_rival:?}"
    );

    // O rival assina em outro lugar (sai do pool de livres) → a vaga desce até o jogador.
    conn.execute(
        "UPDATE drivers SET status = ?1 WHERE id = 'P003'",
        params![DriverStatus::Aposentado.as_str()],
    )
    .expect("rival sai do mercado");

    let sem_rival = generate_player_window_proposals(&conn, 2, 6, &mut rng).expect("sem rival");
    let pendentes = crate::db::queries::market_proposals::get_pending_player_proposals(
        &conn, &s2.id, &player.id,
    )
    .expect("pendentes 2");
    assert_eq!(
        pendentes.len(),
        1,
        "sem ninguém melhor livre, a proposta nasce: held={sem_rival:?}"
    );
    assert!(!sem_rival.is_empty(), "e o assento passa a ser segurado");
}

#[test]
fn a_onda_segura_a_proposta_da_equipe_do_fundo_ate_a_liberacao() {
    // MERCADO EM ONDAS pelo lado do jogador: a única equipe da categoria é fundo por
    // definição (corte = len/2 = 0), então a liberação dela é a semana 5 (gt3, tier
    // alto). Mesmo com o jogador sendo a PRIMEIRA escolha da vaga, a proposta não
    // nasce na semana 4 — nasce na 5, quando a equipe entra no mercado. É o mecanismo
    // que faz a proposta do fundo chegar tarde.
    if std::env::var("IRACER_MERCADO_EM_ONDAS").is_ok() {
        return; // o harness pode estar rodando o braço "antes"
    }
    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("schema");
    conn.execute(
        "UPDATE meta SET value = '70' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");

    let s1 = Season::new("S001".to_string(), 1, 2024);
    let s2 = Season::new("S002".to_string(), 2, 2025);
    season_queries::insert_season(&conn, &s1).expect("s1");
    season_queries::finalize_season(&conn, &s1.id).expect("finalize s1");
    season_queries::insert_season(&conn, &s2).expect("s2");

    let mut rng = StdRng::seed_from_u64(8181);
    let team = sample_team("gt3", "T001", &mut rng);
    team_queries::insert_team(&conn, &team).expect("team");

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

    let semana_4 = generate_player_window_proposals(&conn, 2, 4, &mut rng).expect("semana 4");
    let pendentes = crate::db::queries::market_proposals::get_pending_player_proposals(
        &conn, &s2.id, &player.id,
    )
    .expect("pendentes");
    assert!(
        pendentes.is_empty() && semana_4.is_empty(),
        "antes da liberação a equipe do fundo não propõe, mesmo ao primeiro da lista"
    );

    let semana_5 = generate_player_window_proposals(&conn, 2, 5, &mut rng).expect("semana 5");
    let pendentes = crate::db::queries::market_proposals::get_pending_player_proposals(
        &conn, &s2.id, &player.id,
    )
    .expect("pendentes 2");
    assert!(
        !pendentes.is_empty() && !semana_5.is_empty(),
        "na semana de liberação a proposta nasce"
    );
}

#[test]
fn o_criterio_da_proposta_e_a_primeira_escolha_e_nao_o_top_3() {
    // O gate puro, sem banco: com a flag no padrão, só a posição 0 vira proposta. As
    // posições 1 e 2 continuam existindo — elas são o INTERESSE que a tela mostra antes
    // de o mercado contratar, e é isso que separa "tem equipe de olho" de "te querem".
    assert!(jogador_e_a_escolha_da_vaga(0));
    if std::env::var("IRACER_PROPOSTA_PRIMEIRA_ESCOLHA").is_err() {
        assert!(!jogador_e_a_escolha_da_vaga(1));
        assert!(!jogador_e_a_escolha_da_vaga(2));
    }
}

#[test]
fn o_lesionado_sem_contrato_ainda_ganha_assento_na_virada() {
    // A garantia de porta saía cedo em `status != Ativo`, e o jogador LESIONADO e sem
    // contrato atravessava a virada sem equipe — 5% dos fechos de janela, medido.
    // Lesão é temporária: o contrato da temporada seguinte não pode depender dela.
    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("schema");
    conn.execute(
        "UPDATE meta SET value = '50' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");

    let s2 = Season::new("S002".to_string(), 2, 2025);
    season_queries::insert_season(&conn, &s2).expect("s2");

    let mut rng = StdRng::seed_from_u64(5151);
    let team = sample_team("mazda_rookie", "T001", &mut rng); // estreia: sempre acessível
    team_queries::insert_team(&conn, &team).expect("team");

    let mut player = sample_driver(
        "P001",
        "Jogador",
        Some("mazda_rookie"),
        70.0,
        DriverStatus::Lesionado,
    );
    player.is_jogador = true;
    driver_queries::insert_driver(&conn, &player).expect("player");

    ensure_player_seated(&conn, 2).expect("garantia de porta");

    let contrato = contract_queries::get_active_regular_contract_for_pilot(&conn, &player.id)
        .expect("consulta")
        .expect("o lesionado livre tem que sair da virada com equipe");
    assert_eq!(contrato.equipe_id, team.id);

    // O aposentado continua fora: nenhuma porta para quem encerrou a carreira.
    let mut retired = sample_driver(
        "P002",
        "Aposentado",
        Some("mazda_rookie"),
        70.0,
        DriverStatus::Aposentado,
    );
    retired.is_jogador = true;
    conn.execute("UPDATE drivers SET is_jogador = 0 WHERE id = 'P001'", [])
        .expect("troca o jogador");
    driver_queries::insert_driver(&conn, &retired).expect("retired");
    ensure_player_seated(&conn, 2).expect("garantia de porta 2");
    assert!(
        contract_queries::get_active_regular_contract_for_pilot(&conn, &retired.id)
            .expect("consulta 2")
            .is_none(),
        "aposentado não ganha assento"
    );
}

#[test]
fn o_passe_zero_da_porta_prefere_a_categoria_de_origem_do_agente_livre() {
    // `sync.rs` zera o categoria_atual de quem está sem contrato, então o passe 0
    // ("própria categoria") comparava com string vazia e nunca casava — 0 de 492
    // fechos medidos; a vaga vinha sempre do passe 1, que escolhe o pior carro do
    // tier. Com o resgate pela categoria do último contrato, o jogador volta para a
    // categoria de ORIGEM mesmo quando o outro lado do tier tem carro pior.
    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("schema");
    conn.execute(
        "UPDATE meta SET value = '60' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");

    let s2 = Season::new("S002".to_string(), 2, 2025);
    season_queries::insert_season(&conn, &s2).expect("s2");

    let mut rng = StdRng::seed_from_u64(6161);
    // Origem (toyota_rookie) com carro FORTE; o outro lado do tier 0 com carro fraco.
    // Sem o passe 0, o pior carro do tier venceria e o jogador cairia na mazda_rookie.
    let mut origem = sample_team("toyota_rookie", "T001", &mut rng);
    origem.car = None;
    origem.car_performance = 12.0;
    let mut outra = sample_team("mazda_rookie", "T002", &mut rng);
    outra.car = None;
    outra.car_performance = -3.0;
    team_queries::insert_team(&conn, &origem).expect("origem");
    team_queries::insert_team(&conn, &outra).expect("outra");

    // Agente livre com categoria_atual já limpa e o último contrato na origem.
    let mut player = sample_driver("P001", "Jogador", None, 70.0, DriverStatus::Ativo);
    player.is_jogador = true;
    driver_queries::insert_driver(&conn, &player).expect("player");
    let antigo = Contract::new(
        "C001".to_string(),
        player.id.clone(),
        player.nome.clone(),
        origem.id.clone(),
        origem.nome.clone(),
        1,
        1,
        20_000.0,
        TeamRole::Numero1,
        "toyota_rookie".to_string(),
    );
    contract_queries::insert_contract(&conn, &antigo).expect("contrato antigo");
    conn.execute(
        "UPDATE contracts SET status = 'Encerrado' WHERE id = 'C001'",
        [],
    )
    .expect("encerra o antigo");

    ensure_player_seated(&conn, 2).expect("garantia de porta");

    let contrato = contract_queries::get_active_regular_contract_for_pilot(&conn, &player.id)
        .expect("consulta")
        .expect("o agente livre tem que sair com equipe");
    assert_eq!(
        contrato.categoria, "toyota_rookie",
        "o passe 0 tem que devolver o jogador à categoria de origem, não ao pior carro do tier"
    );
}

#[test]
fn o_papel_do_ultimo_contrato_vale_no_contexto_de_mercado() {
    // Os caminhos de shortlist passavam um mapa vazio de contratos a vencer e TODO
    // candidato caía no default Numero2 (-2.0 de visibilidade) — medido como o corte
    // dominante do gate de 4.0. O papel de verdade está no banco; a conta usa ele.
    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("schema");

    let s1 = Season::new("S001".to_string(), 1, 2024);
    season_queries::insert_season(&conn, &s1).expect("s1");
    season_queries::finalize_season(&conn, &s1.id).expect("finalize");

    let mut rng = StdRng::seed_from_u64(7171);
    let team = sample_team("gt3", "T001", &mut rng);
    team_queries::insert_team(&conn, &team).expect("team");

    let n1 = sample_driver("P001", "Titular", Some("gt3"), 80.0, DriverStatus::Ativo);
    let sem_historia = sample_driver("P002", "Novato", Some("gt3"), 60.0, DriverStatus::Ativo);
    driver_queries::insert_driver(&conn, &n1).expect("n1");
    driver_queries::insert_driver(&conn, &sem_historia).expect("novato");
    insert_standing(&conn, &s1.id, &n1.id, &team.id, "gt3", 1, 400.0, 10, 8);
    insert_standing(&conn, &s1.id, &sem_historia.id, &team.id, "gt3", 8, 100.0, 0, 0);

    let encerrado = Contract::new(
        "C001".to_string(),
        n1.id.clone(),
        n1.nome.clone(),
        team.id.clone(),
        team.nome.clone(),
        1,
        1,
        90_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );
    contract_queries::insert_contract(&conn, &encerrado).expect("contrato");
    conn.execute(
        "UPDATE contracts SET status = 'Encerrado' WHERE id = 'C001'",
        [],
    )
    .expect("encerra");

    let drivers_by_id: HashMap<String, Driver> = [(n1.id.clone(), n1.clone()), (
        sem_historia.id.clone(),
        sem_historia.clone(),
    )]
    .into_iter()
    .collect();
    let contexts = load_market_contexts(&conn, Some("S001"), &drivers_by_id, &HashMap::new())
        .expect("contextos");

    assert_eq!(
        contexts.get("P001").map(|c| c.papel.clone()),
        Some(TeamRole::Numero1),
        "quem foi N1 no último contrato não pode ser avaliado como N2"
    );
    assert_eq!(
        contexts.get("P002").map(|c| c.papel.clone()),
        Some(TeamRole::Numero2),
        "quem nunca teve contrato continua no default"
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

    let held_w1 = generate_player_window_proposals(&conn, 2, 5, &mut rng).expect("w1");
    let pending_w1 = crate::db::queries::market_proposals::get_pending_player_proposals(
        &conn, &s2.id, &player.id,
    )
    .expect("p1");
    assert_eq!(pending_w1.len(), 1, "semana 5 cria a proposta");
    assert!(!held_w1.is_empty(), "semana 5 segura o assento da proposta");

    // Semana 8 = criada(5) + TTL(3): expira e não reoferece.
    let held_w4 = generate_player_window_proposals(&conn, 2, 8, &mut rng).expect("w4");
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
