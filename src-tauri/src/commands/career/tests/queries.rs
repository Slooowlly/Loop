//! Testes de `career::queries`: leituras de piloto, calendario, resultados e noticias.
//!
//! Fatiado de `tests/mod.rs`, que juntava as dez areas num arquivo so. Os helpers
//! e os `use` continuam no `mod.rs` e chegam aqui pelo glob.

use super::*;

#[test]
fn test_get_drivers_by_category_returns_ordered_standings() {
    let base_dir = create_test_career_dir("drivers_by_category");
    let standings = get_drivers_by_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("driver standings");

    assert_eq!(standings.len(), 12);
    assert_eq!(standings[0].posicao_campeonato, 1);
    assert!(standings
        .windows(2)
        .all(|window| window[0].pontos >= window[1].pontos));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_drivers_by_category_marks_rookies() {
    let base_dir = create_test_career_dir("drivers_rookie_marker");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let mut rookie = driver_queries::get_player_driver(&db.conn).expect("player");
    rookie.stats_carreira.corridas = 0;
    rookie.stats_carreira.temporadas = 0;
    rookie.temporadas_na_categoria = 0;
    driver_queries::update_driver(&db.conn, &rookie).expect("update rookie");

    let mut veteran = driver_queries::get_drivers_by_category(&db.conn, "mazda_rookie")
        .expect("drivers")
        .into_iter()
        .find(|driver| !driver.is_jogador)
        .expect("non-player driver");
    veteran.stats_carreira.corridas = 12;
    veteran.stats_carreira.temporadas = 1;
    veteran.temporadas_na_categoria = 0;
    driver_queries::update_driver(&db.conn, &veteran).expect("update veteran");

    let tx = db.conn.unchecked_transaction().expect("injury tx");
    crate::db::queries::injuries::insert_injury(
        &tx,
        &crate::models::injury::Injury {
            id: "I_TEST_LIGHT".to_string(),
            pilot_id: rookie.id.clone(),
            injury_type: crate::models::enums::InjuryType::Moderada,
            injury_name: "Braço machucado".to_string(),
            modifier: 0.85,
            races_total: 2,
            races_remaining: 2,
            skill_penalty: 0.1,
            season: 1,
            race_occurred: "R001".to_string(),
            active: true,
        },
    )
    .expect("insert rookie injury");
    crate::db::queries::injuries::insert_injury(
        &tx,
        &crate::models::injury::Injury {
            id: "I_TEST_GRAVE".to_string(),
            pilot_id: veteran.id.clone(),
            injury_type: crate::models::enums::InjuryType::Grave,
            injury_name: "Braço fraturado".to_string(),
            modifier: 0.65,
            races_total: 4,
            races_remaining: 4,
            skill_penalty: 0.25,
            season: 1,
            race_occurred: "R002".to_string(),
            active: true,
        },
    )
    .expect("insert veteran injury");
    tx.commit().expect("commit injuries");
    drop(db);

    let standings = get_drivers_by_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("driver standings");
    let rookie_entry = standings
        .iter()
        .find(|entry| entry.id == rookie.id)
        .expect("rookie entry");
    let veteran_entry = standings
        .iter()
        .find(|entry| entry.id == veteran.id)
        .expect("veteran entry");

    assert!(rookie_entry.is_estreante);
    assert!(rookie_entry.is_estreante_da_vida);
    assert!(veteran_entry.is_estreante);
    assert!(!veteran_entry.is_estreante_da_vida);
    assert_eq!(
        rookie_entry.lesao_ativa_tipo,
        Some(crate::models::enums::InjuryType::Moderada)
    );
    assert_eq!(
        veteran_entry.lesao_ativa_tipo,
        Some(crate::models::enums::InjuryType::Grave)
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_drivers_by_category_excludes_non_participants_once_category_has_results() {
    let base_dir = create_test_career_dir("drivers_exclude_non_participants");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let mut outsider = crate::models::driver::Driver::new(
        "P_OUTSIDER".to_string(),
        "Miguel Garcia".to_string(),
        "br".to_string(),
        "M".to_string(),
        19,
        2025,
    );
    outsider.categoria_atual = Some("mazda_rookie".to_string());
    driver_queries::insert_driver(&db.conn, &outsider).expect("insert outsider");

    let participant = driver_queries::get_player_driver(&db.conn).expect("player");
    let participant_team = find_player_team(
        &db.conn,
        &participant.id,
        crate::models::enums::SeasonPhase::BlocoRegular,
    )
    .expect("player team")
    .expect("active player team");
    let race_id: String = db
        .conn
        .query_row(
            "SELECT id FROM calendar WHERE categoria = 'mazda_rookie' ORDER BY rodada ASC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("mazda rookie calendar race");
    db.conn
        .execute(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos)
             VALUES (?1, ?2, ?3, 1, 25.0)",
            rusqlite::params![race_id, participant.id, participant_team.id],
        )
        .expect("seed participant race result");
    drop(db);

    let standings = get_drivers_by_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("driver standings");

    assert!(
        standings.iter().all(|entry| entry.id != "P_OUTSIDER"),
        "driver without season participation should not appear once the category already has race results"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_drivers_by_category_uses_recent_results_fallback_from_driver_record() {
    let base_dir = create_test_career_dir("drivers_recent_fallback");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let mut driver = driver_queries::get_player_driver(&db.conn).expect("player");
    driver.stats_temporada.corridas = 3;
    driver.ultimos_resultados = serde_json::json!([
        { "position": 8, "is_dnf": false },
        { "position": 6, "is_dnf": false },
        { "position": 4, "is_dnf": false }
    ]);
    driver_queries::update_driver(&db.conn, &driver).expect("update driver");

    let results_path = config
        .saves_dir()
        .join("career_001")
        .join("race_results.json");
    if results_path.exists() {
        fs::remove_file(&results_path).expect("remove history file");
    }

    let standings = get_drivers_by_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("driver standings");
    let player = standings
        .into_iter()
        .find(|entry| entry.is_jogador)
        .expect("player standing");

    let fallback_tail: Vec<i32> = player
        .results
        .iter()
        .flatten()
        .map(|result| result.position)
        .collect();

    assert_eq!(fallback_tail, vec![8, 6, 4]);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_drivers_by_category_keeps_special_standings_after_skip_cleanup() {
    let base_dir = create_test_career_dir("special_standings_after_skip");

    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    force_legacy_blocoregular_state(&db);
    db.conn
        .execute(
            "UPDATE calendar SET status = 'Concluida' WHERE season_phase = 'BlocoRegular'",
            [],
        )
        .expect("complete regular block");
    crate::convocation::advance_to_convocation_window(&db.conn).expect("advance convocation");
    crate::convocation::run_convocation_window(&db.conn).expect("run convocation");
    crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");
    drop(db);

    crate::commands::race::simulate_special_block_in_base_dir(&base_dir, "career_001")
        .expect("simulate special block");
    let db = Database::open_existing(&db_path).expect("db after special sim");
    crate::convocation::encerrar_bloco_especial(&db.conn).expect("end special block");
    crate::convocation::run_pos_especial(&db.conn).expect("run pos especial");
    drop(db);

    let standings =
        get_drivers_by_category_in_base_dir(&base_dir, "career_001", "production_challenger")
            .expect("production special standings");

    assert!(
        !standings.is_empty(),
        "standings especiais devem continuar visiveis apos o cleanup"
    );
    assert!(
        standings.iter().any(|driver| driver.pontos > 0),
        "standings especiais devem refletir pontos simulados"
    );
    assert!(
        standings
            .iter()
            .any(|driver| driver.results.iter().any(Option::is_some)),
        "standings especiais devem manter resultados por rodada"
    );
    assert!(
        standings
            .iter()
            .any(|driver| driver.classe.as_deref() == Some("bmw")),
        "standings especiais devem carregar a classe/carro do piloto"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_consecutive_team_seasons_up_to_counts_only_current_streak() {
    let mut season_one = crate::models::contract::Contract::new(
        "C001".to_string(),
        "P001".to_string(),
        "Piloto".to_string(),
        "T001".to_string(),
        "Equipe 1".to_string(),
        1,
        1,
        100_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );
    season_one.status = ContractStatus::Expirado;
    let mut season_two = crate::models::contract::Contract::new(
        "C002".to_string(),
        "P001".to_string(),
        "Piloto".to_string(),
        "T001".to_string(),
        "Equipe 1".to_string(),
        2,
        1,
        110_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );
    season_two.status = ContractStatus::Expirado;
    let season_three = crate::models::contract::Contract::new(
        "C003".to_string(),
        "P001".to_string(),
        "Piloto".to_string(),
        "T001".to_string(),
        "Equipe 1".to_string(),
        3,
        2,
        120_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );
    let mut different_team = crate::models::contract::Contract::new(
        "C004".to_string(),
        "P002".to_string(),
        "Piloto 2".to_string(),
        "T001".to_string(),
        "Equipe 1".to_string(),
        1,
        1,
        95_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );
    different_team.status = ContractStatus::Expirado;
    let current_other_team = crate::models::contract::Contract::new(
        "C005".to_string(),
        "P002".to_string(),
        "Piloto 2".to_string(),
        "T002".to_string(),
        "Equipe 2".to_string(),
        3,
        1,
        105_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );

    let veteran_streak =
        consecutive_team_seasons_up_to(&[season_one, season_two, season_three], "T001", 3);
    let newcomer_streak =
        consecutive_team_seasons_up_to(&[different_team, current_other_team], "T002", 3);

    assert_eq!(veteran_streak, Some(3));
    assert_eq!(newcomer_streak, Some(1));
}

#[test]
fn test_get_calendar_for_category_returns_races() {
    let base_dir = create_test_career_dir("calendar_category");
    let races = get_calendar_for_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("calendar");

    assert_eq!(races.len(), 5);
    assert_eq!(races[0].rodada, 1);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_race_results_by_category_returns_round_history_after_simulation() {
    let base_dir = create_test_career_dir("race_history");
    let career_id = "career_001";
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season")
        .expect("active season");
    let next_race = calendar_queries::get_next_race(&db.conn, &season.id, "mazda_rookie")
        .expect("next race")
        .expect("pending race");

    crate::commands::race::simulate_race_weekend_in_base_dir(&base_dir, career_id, &next_race.id)
        .expect("simulate race");

    let histories = get_race_results_by_category_in_base_dir(&base_dir, career_id, "mazda_rookie")
        .expect("race history");

    assert_eq!(histories.len(), 12);
    assert!(histories.iter().all(|history| history.results.len() == 5));
    assert!(histories.iter().any(|history| history.results[0].is_some()));
    assert!(
        histories.iter().any(|history| {
            history
                .results
                .iter()
                .flatten()
                .any(|result| result.has_fastest_lap)
        }),
        "expected persisted race history to retain the fastest-lap marker",
    );
    assert!(
        histories
            .iter()
            .flat_map(|history| history.results.iter().flatten())
            .all(|result| result.grid_position > 0),
        "expected persisted race history to retain grid positions",
    );
    assert!(
        histories
            .iter()
            .flat_map(|history| history.results.iter().flatten())
            .all(|result| result.positions_gained == result.grid_position - result.position),
        "expected persisted race history to retain positions gained",
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_driver_detail_counts_fastest_laps_from_persisted_history() {
    let base_dir = create_test_career_dir("driver_detail_fastest_lap");
    let career_id = "career_001";
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season")
        .expect("active season");
    let next_race = calendar_queries::get_next_race(&db.conn, &season.id, "mazda_rookie")
        .expect("next race")
        .expect("pending race");

    let race_result = crate::commands::race::simulate_race_weekend_in_base_dir(
        &base_dir,
        career_id,
        &next_race.id,
    )
    .expect("simulate race");
    let fastest_lap_driver_id = race_result
        .player_race
        .race_results
        .iter()
        .find(|entry| entry.has_fastest_lap)
        .map(|entry| entry.pilot_id.clone())
        .expect("fastest lap driver");

    let detail = get_driver_detail_in_base_dir(&base_dir, career_id, &fastest_lap_driver_id)
        .expect("driver detail");

    assert_eq!(detail.performance.temporada.voltas_rapidas, Some(1));
    assert_eq!(detail.performance.carreira.voltas_rapidas, Some(1));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_previous_champions_returns_empty_for_first_season() {
    let base_dir = create_test_career_dir("previous_champions");
    let champions = get_previous_champions_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("previous champions");

    assert!(champions.driver_champion_id.is_none());
    assert!(champions.constructor_champions.is_empty());

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_previous_champions_returns_last_season_category_champion() {
    let base_dir = create_test_career_dir("previous_champions_second_season");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let season = season_queries::get_active_season(&db.conn)
        .expect("temporada ativa")
        .expect("temporada ativa existente");

    // Duas corridas da categoria na temporada 1, com um piloto somando mais.
    let races: Vec<String> = db
        .conn
        .prepare(
            "SELECT id FROM calendar WHERE categoria = 'mazda_rookie' ORDER BY rodada ASC LIMIT 2",
        )
        .expect("prepare calendario")
        .query_map([], |row| row.get(0))
        .expect("query calendario")
        .map(|row| row.expect("linha do calendario"))
        .collect();
    assert_eq!(
        races.len(),
        2,
        "categoria deveria ter ao menos duas rodadas"
    );

    let drivers = get_drivers_by_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("grid da categoria");
    let champion_id = drivers[0].id.clone();
    let runner_up_id = drivers[1].id.clone();

    // race_results tem FK para teams, entao o resultado precisa de equipe real.
    // Qual delas nao importa: o campeao sai por soma de pontos por piloto.
    let team_id: String = db
        .conn
        .query_row(
            "SELECT id FROM teams WHERE categoria = 'mazda_rookie' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("equipe da categoria");

    for race_id in &races {
        db.conn
            .execute(
                "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos)
                 VALUES (?1, ?2, ?3, 1, 25.0)",
                rusqlite::params![race_id, champion_id, team_id],
            )
            .expect("resultado do campeao");
        db.conn
            .execute(
                "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos)
                 VALUES (?1, ?2, ?3, 2, 18.0)",
                rusqlite::params![race_id, runner_up_id, team_id],
            )
            .expect("resultado do vice");
    }

    // Encerra a temporada 1 e abre a 2 — o campeão passa a ser "reinante".
    // Nessa ordem: há índice único garantindo uma só temporada em andamento.
    db.conn
        .execute(
            "UPDATE seasons SET status = 'Finalizada' WHERE id = ?1",
            rusqlite::params![season.id],
        )
        .expect("encerra temporada anterior");
    db.conn
        .execute(
            "INSERT INTO seasons (id, numero, ano, status)
             SELECT 'S_TEST_NEXT', numero + 1, ano + 1, 'EmAndamento' FROM seasons WHERE id = ?1",
            rusqlite::params![season.id],
        )
        .expect("cria temporada seguinte");
    drop(db);

    let champions = get_previous_champions_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("previous champions");

    assert_eq!(
        champions.driver_champion_id.as_deref(),
        Some(champion_id.as_str()),
        "campeao reinante deveria ser quem somou mais pontos na temporada anterior"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_driver_detail_returns_contracted_ai_payload() {
    let base_dir = create_test_career_dir("driver_detail_contracted");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season")
        .expect("active season");
    let mut driver = driver_queries::get_drivers_by_category(&db.conn, "mazda_rookie")
        .expect("drivers")
        .into_iter()
        .find(|candidate| !candidate.is_jogador)
        .expect("ai driver");

    driver.atributos.skill = 97.0;
    driver.atributos.gestao_pneus = 20.0;
    driver.motivacao = 82.0;
    driver.melhor_resultado_temp = Some(2);
    driver.stats_temporada.corridas = 3;
    driver.stats_temporada.pontos = 28.0;
    driver.stats_temporada.vitorias = 1;
    driver.stats_temporada.podios = 2;
    driver.stats_temporada.poles = 0;
    driver.stats_temporada.dnfs = 0;
    driver.stats_carreira.corridas = 9;
    driver.stats_carreira.pontos_total = 84.0;
    driver.stats_carreira.vitorias = 2;
    driver.stats_carreira.podios = 4;
    driver.stats_carreira.poles = 1;
    driver.stats_carreira.dnfs = 1;
    driver.stats_carreira.titulos = 2;
    driver_queries::update_driver(&db.conn, &driver).expect("update driver");

    let contract = contract_queries::get_active_contract_for_pilot(&db.conn, &driver.id)
        .expect("active contract")
        .expect("contract");
    let team = team_queries::get_team_by_id(&db.conn, &contract.equipe_id)
        .expect("team query")
        .expect("team");

    let detail =
        get_driver_detail_in_base_dir(&base_dir, "career_001", &driver.id).expect("driver detail");
    let detail_json = serde_json::to_value(&detail).expect("serialize driver detail");

    assert_eq!(detail.id, driver.id);
    assert_eq!(detail.nome, driver.nome);
    assert_eq!(detail.status, "ativo");
    assert_eq!(
        detail.equipe_id.as_deref(),
        Some(contract.equipe_id.as_str())
    );
    assert_eq!(detail.equipe_nome.as_deref(), Some(team.nome.as_str()));
    assert_eq!(
        detail.equipe_cor_primaria.as_deref(),
        Some(team.cor_primaria.as_str())
    );
    assert_eq!(
        detail.equipe_cor_secundaria.as_deref(),
        Some(team.cor_secundaria.as_str())
    );
    assert_eq!(detail.papel.as_deref(), Some(contract.papel.as_str()));
    assert!(detail.personalidade_primaria.is_some());
    assert!(detail.personalidade_secundaria.is_some());
    assert_eq!(detail.motivacao, 82);
    assert_eq!(detail.stats_temporada.corridas, 3);
    assert_eq!(detail.stats_temporada.pontos, 28);
    assert_eq!(detail.stats_temporada.melhor_resultado, 2);
    assert_eq!(detail.stats_carreira.corridas, 9);
    assert_eq!(detail.stats_carreira.pontos, 84);
    assert_eq!(
        detail.contrato.as_ref().map(|value| value.anos_restantes),
        Some(contract.anos_restantes(season.numero))
    );
    assert!(detail.tags.iter().any(|tag| {
        tag.attribute_name == "skill"
            && tag.tag_text == "Alien"
            && tag.level == "elite"
            && tag.color == "#bc8cff"
    }));
    assert!(detail.tags.iter().any(|tag| {
        tag.attribute_name == "gestao_pneus" && tag.level == "defeito" && tag.color == "#db6d28"
    }));
    assert!(
        detail_json.get("perfil").is_some(),
        "expected modular profile block"
    );
    assert!(
        detail_json.get("competitivo").is_some(),
        "expected modular competitive block",
    );
    assert!(
        detail_json.get("performance").is_some(),
        "expected modular performance block",
    );
    assert!(
        detail_json.get("leitura_tecnica").is_some(),
        "expected backend technical-reading block",
    );
    assert_eq!(detail.leitura_tecnica.itens.len(), 14);
    assert!(detail
        .leitura_tecnica
        .itens
        .iter()
        .any(|item| item.chave == "ritmo" && item.nivel == "Elite"));
    assert!(
        detail_json.get("forma").is_some(),
        "expected current-form block"
    );
    assert!(
        detail_json.get("trajetoria").is_some(),
        "expected basic career-path block",
    );
    assert_eq!(detail.trajetoria.titulos, 2);
    assert!(detail.trajetoria.foi_campeao);
    assert!(
        detail_json.get("contrato_mercado").is_some(),
        "expected contract-and-market block",
    );
    assert!(
        detail.contrato_mercado.mercado.is_some(),
        "expected market block to be connected for active drivers",
    );
    assert_eq!(
        detail_json.pointer("/performance/temporada/pontos"),
        None,
        "expected points to stop being a primary dossier metric",
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_driver_detail_marks_active_driver_without_contract_as_livre() {
    let base_dir = create_test_career_dir("driver_detail_free");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let free_driver = Driver::new(
        "P-LIVRE-001".to_string(),
        "Piloto Livre".to_string(),
        "🇧🇷 Brasileiro".to_string(),
        "M".to_string(),
        27,
        2020,
    );
    driver_queries::insert_driver(&db.conn, &free_driver).expect("insert free driver");

    let detail = get_driver_detail_in_base_dir(&base_dir, "career_001", &free_driver.id)
        .expect("driver detail");
    let detail_json = serde_json::to_value(&detail).expect("serialize driver detail");

    assert_eq!(detail.id, free_driver.id);
    assert_eq!(detail.status, "livre");
    assert!(detail.equipe_id.is_none());
    assert!(detail.equipe_nome.is_none());
    assert!(detail.papel.is_none());
    assert!(detail.contrato.is_none());
    assert_eq!(detail.stats_temporada.melhor_resultado, 0);
    assert_eq!(detail.stats_carreira.melhor_resultado, 0);
    assert_eq!(detail.resumo_atual.veredito, "Estreante");
    assert_eq!(detail.resumo_atual.tom, "info");
    assert!(
        detail_json.get("contrato_mercado").is_some(),
        "expected contract/market block to exist structurally",
    );
    assert!(
        detail_json.pointer("/contrato_mercado/mercado").is_some(),
        "expected market data to be connected even for free active drivers",
    );
    assert!(
        detail_json.get("relacionamentos").is_none()
            || detail_json
                .get("relacionamentos")
                .is_some_and(|value| value.is_null()),
        "expected relationships block to stay empty when there is no real data",
    );
    assert!(
        detail_json.get("reputacao").is_none()
            || detail_json
                .get("reputacao")
                .is_some_and(|value| value.is_null()),
        "expected reputation block to stay empty when there is no real data",
    );
    assert!(
        detail_json.get("saude").is_none()
            || detail_json
                .get("saude")
                .is_some_and(|value| value.is_null()),
        "expected health block to stay empty when there is no real data",
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_driver_detail_includes_active_injury_context() {
    let base_dir = create_test_career_dir("driver_detail_active_injury");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season")
        .expect("active season");
    let mut driver = driver_queries::get_drivers_by_category(&db.conn, "mazda_rookie")
        .expect("drivers")
        .into_iter()
        .find(|candidate| !candidate.is_jogador)
        .expect("ai driver");
    driver.status = crate::models::enums::DriverStatus::Lesionado;
    driver_queries::update_driver(&db.conn, &driver).expect("update injured driver");
    let race = calendar_queries::get_calendar(&db.conn, &season.id, "mazda_rookie")
        .expect("calendar")
        .into_iter()
        .next()
        .expect("race");

    let tx = db.conn.unchecked_transaction().expect("injury tx");
    crate::db::queries::injuries::insert_injury(
        &tx,
        &crate::models::injury::Injury {
            id: "I-DETAIL-001".to_string(),
            pilot_id: driver.id.clone(),
            injury_type: crate::models::enums::InjuryType::Moderada,
            injury_name: "".to_string(),
            modifier: 0.88,
            races_total: 4,
            races_remaining: 3,
            skill_penalty: 0.10,
            season: season.numero,
            race_occurred: race.id.clone(),
            active: true,
        },
    )
    .expect("insert active injury");
    crate::db::queries::injuries::insert_injury(
        &tx,
        &crate::models::injury::Injury {
            id: "I-DETAIL-002".to_string(),
            pilot_id: driver.id.clone(),
            injury_type: crate::models::enums::InjuryType::Leve,
            injury_name: "Dor no braço".to_string(),
            modifier: 0.95,
            races_total: 2,
            races_remaining: 0,
            skill_penalty: 0.05,
            season: season.numero,
            race_occurred: race.id.clone(),
            active: false,
        },
    )
    .expect("insert light injury history");
    crate::db::queries::injuries::insert_injury(
        &tx,
        &crate::models::injury::Injury {
            id: "I-DETAIL-003".to_string(),
            pilot_id: driver.id.clone(),
            injury_type: crate::models::enums::InjuryType::Grave,
            injury_name: "Braço fraturado".to_string(),
            modifier: 0.75,
            races_total: 8,
            races_remaining: 0,
            skill_penalty: 0.15,
            season: season.numero,
            race_occurred: race.id.clone(),
            active: false,
        },
    )
    .expect("insert grave injury history");
    tx.commit().expect("commit injury");

    let detail =
        get_driver_detail_in_base_dir(&base_dir, "career_001", &driver.id).expect("driver detail");
    let active_injury = detail
        .saude
        .as_ref()
        .and_then(|health| health.lesao_ativa.as_ref())
        .expect("active injury context");

    assert_eq!(active_injury.tipo, "Moderada");
    assert_eq!(active_injury.nome.as_deref(), Some("Dor forte nas costas"));
    assert_eq!(active_injury.corridas_total, 4);
    assert_eq!(active_injury.corridas_restantes, 3);
    assert_eq!(active_injury.corrida_ocorrida_id, race.id);
    assert_eq!(active_injury.corrida_ocorrida_rodada, Some(race.rodada));
    assert_eq!(
        active_injury.corrida_ocorrida_pista.as_deref(),
        Some(race.track_name.as_str())
    );
    assert_eq!(detail.trajetoria.historico.lesoes.leves, 1);
    assert_eq!(detail.trajetoria.historico.lesoes.moderadas, 1);
    assert_eq!(detail.trajetoria.historico.lesoes.graves, 1);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_news_filters_by_season_and_type() {
    let base_dir = create_test_career_dir("get_news_filters");
    mark_all_races_completed(&base_dir, "career_001");

    advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    advance_market_week_in_base_dir(&base_dir, "career_001", None).expect("advance market week");

    // news generation is now stubbed; just check the query runs without error
    let _ = get_news_in_base_dir(&base_dir, "career_001", Some(1), None, Some(50)).expect("news");
    let _ = get_news_in_base_dir(&base_dir, "career_001", Some(2), Some("Mercado"), Some(50))
        .expect("market news");

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_news_rejects_invalid_type_filter() {
    let base_dir = create_test_career_dir("get_news_invalid_type");
    let error = get_news_in_base_dir(
        &base_dir,
        "career_001",
        Some(1),
        Some("TipoInvalido"),
        Some(50),
    )
    .expect_err("invalid news type should fail");

    assert!(error.contains("NewsType"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_displaced_driver_context_counts_head_to_head_without_dnf() {
    let base_dir = create_test_career_dir("displaced_context_head_to_head");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let player_team = find_player_team(
        &db.conn,
        &player.id,
        crate::models::enums::SeasonPhase::BlocoRegular,
    )
    .expect("player team")
    .expect("active player team");

    let mut rival = crate::models::driver::Driver::new(
        "P_RIVAL_CTX".to_string(),
        "Marcos Mendes".to_string(),
        "br".to_string(),
        "M".to_string(),
        24,
        2025,
    );
    rival.categoria_atual = Some("mazda_rookie".to_string());
    driver_queries::insert_driver(&db.conn, &rival).expect("insert rival");

    let mut stranger = crate::models::driver::Driver::new(
        "P_STRANGER_CTX".to_string(),
        "Niels Kramer".to_string(),
        "dk".to_string(),
        "M".to_string(),
        22,
        2025,
    );
    stranger.categoria_atual = Some("mazda_rookie".to_string());
    driver_queries::insert_driver(&db.conn, &stranger).expect("insert stranger");

    let race_ids: Vec<String> = db
        .conn
        .prepare(
            "SELECT id FROM calendar WHERE categoria = 'mazda_rookie' ORDER BY rodada ASC LIMIT 3",
        )
        .expect("prepare races")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query races")
        .collect::<Result<Vec<_>, _>>()
        .expect("race ids");
    assert!(race_ids.len() >= 3, "esperava três corridas de mazda_rookie");

    // (corrida, piloto, posição, dnf)
    for (race_id, driver_id, finish, dnf) in [
        (&race_ids[0], &player.id, 2, 0),
        (&race_ids[0], &rival.id, 5, 0),
        (&race_ids[1], &player.id, 7, 0),
        (&race_ids[1], &rival.id, 3, 0),
        // Motor quebrado não é duelo perdido: conta como encontro, não como derrota.
        (&race_ids[2], &player.id, 12, 1),
        (&race_ids[2], &rival.id, 4, 0),
    ] {
        db.conn
            .execute(
                "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos, dnf)
                 VALUES (?1, ?2, ?3, ?4, 0.0, ?5)",
                rusqlite::params![race_id, driver_id, player_team.id, finish, dnf],
            )
            .expect("seed race result");
    }
    drop(db);

    let contexts = get_displaced_driver_context_in_base_dir(
        &base_dir,
        "career_001",
        &[rival.id.clone(), stranger.id.clone()],
    )
    .expect("displaced context");

    assert_eq!(contexts.len(), 2, "devolve na ordem em que a UI pediu");
    assert_eq!(contexts[0].driver_id, rival.id);
    assert_eq!(contexts[0].shared_races, 3);
    assert_eq!(contexts[0].player_ahead, 1);
    assert_eq!(contexts[0].driver_ahead, 1);
    assert!(contexts[0].rival_role.is_none());

    assert_eq!(contexts[1].driver_id, stranger.id);
    assert_eq!(contexts[1].shared_races, 0);
    assert_eq!(contexts[1].player_ahead, 0);
    assert_eq!(contexts[1].driver_ahead, 0);
    assert!(contexts[1].rival_role.is_none());

    let _ = fs::remove_dir_all(base_dir);
}
