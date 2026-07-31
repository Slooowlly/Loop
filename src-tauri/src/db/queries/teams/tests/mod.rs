use rand::{rngs::StdRng, SeedableRng};
use rusqlite::Connection;

use super::*;
use crate::constants::teams::get_team_templates;
use crate::db::connection::DbError;
use crate::finance::cashflow::{RoundCashflowSummary, TeamRoundFinanceContext};
use crate::models::team::Team;

#[test]
fn test_insert_and_get_team() {
    let conn = setup_test_db().expect("test db");
    let team = sample_team("gt3", "T001");

    insert_team(&conn, &team).expect("insert team");
    let loaded = get_team_by_id(&conn, "T001")
        .expect("get team")
        .expect("team should exist");

    assert_eq!(loaded.id, "T001");
    assert_eq!(loaded.nome, team.nome);
    assert_eq!(loaded.categoria, "gt3");
    assert_eq!(loaded.stats_vitorias, 0);
}

fn create_finance_history_table(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE team_finance_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                team_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                round INTEGER NOT NULL,
                category TEXT NOT NULL DEFAULT '',
                sponsorship_income REAL NOT NULL DEFAULT 0.0,
                gate_income REAL NOT NULL DEFAULT 0.0,
                result_bonus REAL NOT NULL DEFAULT 0.0,
                partial_prize_income REAL NOT NULL DEFAULT 0.0,
                aid_income REAL NOT NULL DEFAULT 0.0,
                salary_expense REAL NOT NULL DEFAULT 0.0,
                event_operations_cost REAL NOT NULL DEFAULT 0.0,
                structural_maintenance_cost REAL NOT NULL DEFAULT 0.0,
                technical_investment_cost REAL NOT NULL DEFAULT 0.0,
                debt_service_cost REAL NOT NULL DEFAULT 0.0,
                income_total REAL NOT NULL DEFAULT 0.0,
                expenses_total REAL NOT NULL DEFAULT 0.0,
                net REAL NOT NULL DEFAULT 0.0,
                cash_balance REAL NOT NULL DEFAULT 0.0,
                debt_balance REAL NOT NULL DEFAULT 0.0,
                constructor_prize_income REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(team_id, season_number, round)
            );",
    )
    .expect("create team_finance_history table");
}

fn sample_finance_context() -> TeamRoundFinanceContext {
    TeamRoundFinanceContext {
        sponsorship_income: 100_000.0,
        gate_income: 15_000.0,
        result_bonus: 20_000.0,
        partial_prize_income: 5_000.0,
        aid_income: 0.0,
        salary_expense: 60_000.0,
        event_operations_cost: 25_000.0,
        structural_maintenance_cost: 12_000.0,
        technical_investment_cost: 8_000.0,
        debt_service_cost: 3_000.0,
    }
}

#[test]
fn test_insert_and_read_team_finance_history_roundtrip() {
    let conn = setup_test_db().expect("test db");
    create_finance_history_table(&conn);
    let mut team = sample_team("gt3", "T001");
    insert_team(&conn, &team).expect("insert team");

    let context = sample_finance_context();
    let summary_r3 = RoundCashflowSummary {
        income: 125_000.0,
        expenses: 108_000.0,
        net: 17_000.0,
    };
    team.cash_balance = 517_000.0;
    team.debt_balance = 0.0;
    insert_team_finance_history(&conn, &team, &context, &summary_r3, 1, 3)
        .expect("insert history r3");

    let summary_r4 = RoundCashflowSummary {
        income: 130_000.0,
        expenses: 110_000.0,
        net: 20_000.0,
    };
    team.cash_balance = 537_000.0;
    insert_team_finance_history(&conn, &team, &context, &summary_r4, 1, 4)
        .expect("insert history r4");

    let entries = get_team_finance_history_recent(&conn, "T001", 10).expect("read history");
    assert_eq!(entries.len(), 2);
    // Ordem cronológica ASC (season, round).
    assert_eq!(entries[0].round, 3);
    assert_eq!(entries[1].round, 4);
    assert_eq!(entries[0].sponsorship_income, 100_000.0);
    assert_eq!(entries[0].income_total, 125_000.0);
    assert_eq!(entries[0].net, 17_000.0);
    assert_eq!(entries[0].cash_balance, 517_000.0);
    assert_eq!(entries[1].cash_balance, 537_000.0);
}

#[test]
fn test_season_close_row_records_constructor_prize_after_races() {
    let conn = setup_test_db().expect("test db");
    create_finance_history_table(&conn);
    let mut team = sample_team("gt3", "T001");
    insert_team(&conn, &team).expect("insert team");

    // Uma rodada de corrida normal (sem prêmio).
    let context = sample_finance_context();
    let summary = RoundCashflowSummary {
        income: 125_000.0,
        expenses: 108_000.0,
        net: 17_000.0,
    };
    team.cash_balance = 517_000.0;
    insert_team_finance_history(&conn, &team, &context, &summary, 1, 14)
        .expect("insert last race round");

    // Encerramento: prêmio de construtores creditado como linha de receita real.
    team.cash_balance = 517_000.0 + 5_200_000.0;
    insert_team_finance_season_close(&conn, &team, 1, 5_200_000.0).expect("insert prize row");

    let entries = get_team_finance_history_recent(&conn, "T001", 10).expect("read history");
    assert_eq!(entries.len(), 2);
    // A rodada de corrida não tem prêmio.
    assert_eq!(entries[0].round, 14);
    assert_eq!(entries[0].constructor_prize_income, 0.0);
    // A linha de encerramento ordena DEPOIS da última corrida e carrega o prêmio.
    assert_eq!(entries[1].round, SEASON_CLOSE_ROUND);
    assert_eq!(entries[1].constructor_prize_income, 5_200_000.0);
    assert_eq!(entries[1].income_total, 5_200_000.0);
    assert_eq!(entries[1].net, 5_200_000.0);
    assert_eq!(entries[1].expenses_total, 0.0);
    assert_eq!(entries[1].cash_balance, 5_717_000.0);
}

#[test]
fn test_team_finance_history_reround_is_idempotent() {
    let conn = setup_test_db().expect("test db");
    create_finance_history_table(&conn);
    let mut team = sample_team("gt3", "T001");
    insert_team(&conn, &team).expect("insert team");
    let context = sample_finance_context();

    let first = RoundCashflowSummary {
        income: 125_000.0,
        expenses: 108_000.0,
        net: 17_000.0,
    };
    team.cash_balance = 517_000.0;
    insert_team_finance_history(&conn, &team, &context, &first, 1, 4).expect("insert r4");

    // Re-simular a MESMA (season, round) substitui a linha, não duplica.
    let second = RoundCashflowSummary {
        income: 200_000.0,
        expenses: 100_000.0,
        net: 100_000.0,
    };
    team.cash_balance = 999_000.0;
    insert_team_finance_history(&conn, &team, &context, &second, 1, 4).expect("re-insert r4");

    let entries = get_team_finance_history_recent(&conn, "T001", 10).expect("read history");
    assert_eq!(
        entries.len(),
        1,
        "re-gravar a mesma rodada não deve duplicar"
    );
    assert_eq!(
        entries[0].cash_balance, 999_000.0,
        "deve refletir o novo valor"
    );
    assert_eq!(entries[0].income_total, 200_000.0);
}

#[test]
fn test_insert_and_get_team_persists_extended_fields() {
    let conn = setup_test_db().expect("test db");
    let mut team = sample_team("gt3", "T010");
    team.piloto_1_id = Some("P001".to_string());
    team.piloto_2_id = Some("P002".to_string());
    team.cash_balance = 2_450_000.0;
    team.debt_balance = 325_000.0;
    team.financial_state = "healthy".to_string();
    team.season_strategy = "balanced".to_string();
    team.last_round_income = 180_000.0;
    team.last_round_expenses = 152_500.0;
    team.last_round_net = 27_500.0;
    team.parachute_payment_remaining = 500_000.0;
    team.hierarquia_n1_id = Some("P001".to_string());
    team.hierarquia_n2_id = Some("P002".to_string());
    team.hierarquia_tensao = 33.0;
    team.stats_podios = 4;
    team.stats_poles = 2;
    team.stats_pontos = 87;
    team.stats_melhor_resultado = 1;
    team.historico_podios = 12;
    team.historico_poles = 5;
    team.historico_pontos = 230;
    team.historico_titulos_pilotos = 1;

    insert_team(&conn, &team).expect("insert team");
    update_team_pilots(&conn, "T010", Some("P001"), Some("P002")).expect("update pilots");
    update_team_hierarchy(
        &conn,
        "T010",
        Some("P001"),
        Some("P002"),
        "competitivo",
        33.0,
    )
    .expect("update hierarchy");
    update_team_season_stats(&conn, "T010", 3, 4, 2, 87, 1).expect("update season stats");

    let loaded = get_team_by_id(&conn, "T010")
        .expect("get team")
        .expect("team should exist");

    assert_eq!(loaded.nome_curto, team.nome_curto);
    assert_eq!(loaded.cor_primaria, team.cor_primaria);
    assert_eq!(loaded.cor_secundaria, team.cor_secundaria);
    assert_eq!(loaded.pais_sede, team.pais_sede);
    assert_eq!(loaded.piloto_1_id.as_deref(), Some("P001"));
    assert_eq!(loaded.piloto_2_id.as_deref(), Some("P002"));
    assert_eq!(loaded.pit_strategy_risk, team.pit_strategy_risk);
    assert_eq!(loaded.pit_crew_quality, team.pit_crew_quality);
    assert_eq!(loaded.cash_balance, team.cash_balance);
    assert_eq!(loaded.debt_balance, team.debt_balance);
    assert_eq!(loaded.financial_state, team.financial_state);
    assert_eq!(loaded.season_strategy, team.season_strategy);
    assert_eq!(loaded.last_round_income, team.last_round_income);
    assert_eq!(loaded.last_round_expenses, team.last_round_expenses);
    assert_eq!(loaded.last_round_net, team.last_round_net);
    assert_eq!(
        loaded.parachute_payment_remaining,
        team.parachute_payment_remaining
    );
    assert_eq!(loaded.hierarquia_n1_id.as_deref(), Some("P001"));
    assert_eq!(loaded.hierarquia_n2_id.as_deref(), Some("P002"));
    assert_eq!(loaded.hierarquia_status, "competitivo");
    assert_eq!(loaded.hierarquia_tensao, 33.0);
    assert_eq!(loaded.stats_podios, 4);
    assert_eq!(loaded.stats_poles, 2);
    assert_eq!(loaded.stats_pontos, 87);
    assert_eq!(loaded.stats_melhor_resultado, 1);
}

#[test]
fn test_insert_team_syncs_legacy_budget_from_money() {
    let conn = setup_test_db().expect("test db");
    let mut team = sample_team("gt4", "T020");
    team.cash_balance = 6_000_000.0;
    team.debt_balance = 0.0;
    team.financial_state = "healthy".to_string();
    team.budget = 1.0;

    insert_team(&conn, &team).expect("insert team");
    let loaded = get_team_by_id(&conn, "T020")
        .expect("get team")
        .expect("team should exist");

    let expected_budget = crate::finance::planning::derive_budget_index_from_money(&loaded);
    assert!((loaded.budget - expected_budget).abs() < 0.0001);
    assert!(loaded.budget > 1.0);
}

#[test]
fn test_insert_and_get_team_uses_current_team_schema_without_legacy_columns() {
    let conn = setup_test_db().expect("test db");
    assert!(test_column_exists(&conn, "teams", "confiabilidade"));
    assert!(test_column_exists(&conn, "teams", "reputacao"));
    assert!(!test_column_exists(&conn, "teams", "reliability"));
    assert!(!test_column_exists(&conn, "teams", "prestige"));
    assert!(!test_column_exists(&conn, "teams", "temp_pontos"));
    assert!(!test_column_exists(&conn, "teams", "temp_vitorias"));
    assert!(!test_column_exists(&conn, "teams", "carreira_vitorias"));

    let mut team = sample_team("gt4", "T_SCHEMA");
    team.confiabilidade = 71.0;
    team.reputacao = 63.0;
    team.stats_vitorias = 4;
    team.stats_pontos = 98;
    team.historico_vitorias = 12;

    insert_team(&conn, &team).expect("insert team with current schema");
    let loaded = get_team_by_id(&conn, "T_SCHEMA")
        .expect("get team")
        .expect("team should exist");

    assert_eq!(loaded.confiabilidade, 71.0);
    assert_eq!(loaded.reputacao, 63.0);
    assert_eq!(loaded.stats_vitorias, 4);
    assert_eq!(loaded.stats_pontos, 98);
    assert_eq!(loaded.historico_vitorias, 12);
}

#[test]
fn test_get_teams_by_category() {
    let conn = setup_test_db().expect("test db");
    insert_team(&conn, &sample_team("gt3", "T001")).expect("insert team 1");
    insert_team(&conn, &sample_team("gt3", "T002")).expect("insert team 2");
    insert_team(&conn, &sample_team("gt4", "T003")).expect("insert team 3");

    let gt3_teams = get_teams_by_category(&conn, "gt3").expect("query teams");

    assert_eq!(gt3_teams.len(), 2);
    assert!(gt3_teams.iter().all(|team| team.categoria == "gt3"));
}

#[test]
fn test_update_team_pilots() {
    let conn = setup_test_db().expect("test db");
    insert_team(&conn, &sample_team("gt3", "T001")).expect("insert team");

    update_team_pilots(&conn, "T001", Some("P001"), Some("P002")).expect("update pilots");
    let loaded = get_team_by_id(&conn, "T001")
        .expect("get team")
        .expect("team should exist");

    assert_eq!(loaded.piloto_1_id.as_deref(), Some("P001"));
    assert_eq!(loaded.piloto_2_id.as_deref(), Some("P002"));
}

#[test]
fn test_count_teams_by_category() {
    let conn = setup_test_db().expect("test db");
    insert_team(&conn, &sample_team("gt3", "T001")).expect("insert team 1");
    insert_team(&conn, &sample_team("gt3", "T002")).expect("insert team 2");
    insert_team(&conn, &sample_team("gt4", "T003")).expect("insert team 3");

    let count = count_teams_by_category(&conn, "gt3").expect("count teams");

    assert_eq!(count, 2);
}

#[test]
fn test_remove_pilot_from_team_clears_matching_slot() {
    let conn = setup_test_db().expect("test db");
    let mut team = sample_team("gt3", "T001");
    team.piloto_1_id = Some("P001".to_string());
    team.piloto_2_id = Some("P002".to_string());
    insert_team(&conn, &team).expect("insert team");

    remove_pilot_from_team(&conn, "P002", "T001").expect("remove pilot");

    let refreshed = get_team_by_id(&conn, "T001")
        .expect("team query")
        .expect("team");
    assert_eq!(refreshed.piloto_1_id.as_deref(), Some("P001"));
    assert!(refreshed.piloto_2_id.is_none());
}

#[test]
fn test_blob_in_required_text_field_returns_error() {
    let conn = setup_test_db().expect("test db");
    conn.execute(
        "INSERT INTO teams (id, nome, nome_curto, cor_primaria, categoria, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            "T_BLOB_TEXT",
            "Blob Team",
            "Blob",
            rusqlite::types::Value::Blob(vec![0xDE, 0xAD, 0xBE, 0xEF]),
            "gt3",
            "2026-01-01",
            "2026-01-01",
        ],
    )
    .expect("insert blob team");

    let result = get_team_by_id(&conn, "T_BLOB_TEXT");
    assert!(
        result.is_err(),
        "BLOB em campo obrigatorio TEXT deve retornar erro"
    );
}

#[test]
fn test_blob_in_required_real_field_returns_error() {
    let conn = setup_test_db().expect("test db");
    conn.execute(
        "INSERT INTO teams (
                id, nome, nome_curto, categoria, hierarquia_tensao, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            "T_BLOB_REAL",
            "Blob Team",
            "Blob",
            "gt3",
            rusqlite::types::Value::Blob(vec![0xBA, 0xAD, 0xF0, 0x0D]),
            "2026-01-01",
            "2026-01-01",
        ],
    )
    .expect("insert blob hierarchy");

    let result = get_team_by_id(&conn, "T_BLOB_REAL");
    assert!(
        result.is_err(),
        "BLOB em campo obrigatorio REAL deve retornar erro"
    );
}

#[test]
fn test_update_team_pilots_returns_not_found_for_missing_team() {
    let conn = setup_test_db().expect("test db");

    let error = update_team_pilots(&conn, "T404", Some("P001"), Some("P002"))
        .expect_err("missing team should fail");

    assert!(matches!(error, DbError::NotFound(_)));
}

#[test]
fn test_remove_pilot_from_team_resets_hierarchy_when_removed_pilot_was_ranked() {
    let conn = setup_test_db().expect("test db");
    let mut team = sample_team("gt3", "T777");
    team.piloto_1_id = Some("P001".to_string());
    team.piloto_2_id = Some("P002".to_string());
    team.hierarquia_n1_id = Some("P001".to_string());
    team.hierarquia_n2_id = Some("P002".to_string());
    team.hierarquia_status = "competitivo".to_string();
    team.hierarquia_tensao = 55.0;
    team.hierarquia_duelos_total = 4;
    team.hierarquia_duelos_n2_vencidos = 2;
    team.hierarquia_sequencia_n2 = 1;
    team.hierarquia_sequencia_n1 = 2;
    team.hierarquia_inversoes_temporada = 1;
    insert_team(&conn, &team).expect("insert team");

    remove_pilot_from_team(&conn, "P001", "T777").expect("remove pilot");

    let refreshed = get_team_by_id(&conn, "T777")
        .expect("team query")
        .expect("team exists");
    assert!(refreshed.piloto_1_id.is_none());
    assert_eq!(refreshed.piloto_2_id.as_deref(), Some("P002"));
    assert!(refreshed.hierarquia_n1_id.is_none());
    assert!(refreshed.hierarquia_n2_id.is_none());
    assert_eq!(refreshed.hierarquia_status, "estavel");
    assert_eq!(refreshed.hierarquia_tensao, 0.0);
    assert_eq!(refreshed.hierarquia_duelos_total, 0);
    assert_eq!(refreshed.hierarquia_duelos_n2_vencidos, 0);
    assert_eq!(refreshed.hierarquia_sequencia_n2, 0);
    assert_eq!(refreshed.hierarquia_sequencia_n1, 0);
    assert_eq!(refreshed.hierarquia_inversoes_temporada, 0);
}

#[test]
fn test_invalid_hierarchy_status_from_db_returns_error() {
    let conn = setup_test_db().expect("test db");
    conn.execute(
        "INSERT INTO teams (id, nome, nome_curto, categoria, hierarquia_status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            "T_BAD_HIER",
            "Bad Team",
            "BAD",
            "gt3",
            "alienigena",
            "2026-01-01",
            "2026-01-01",
        ],
    )
    .expect("insert invalid hierarchy team");

    let result = get_team_by_id(&conn, "T_BAD_HIER");
    assert!(
        result.is_err(),
        "hierarquia_status invalido deve retornar erro, nao cair em estavel"
    );
}

#[test]
fn test_invalid_meta_posicao_from_db_returns_error() {
    let conn = setup_test_db().expect("test db");
    conn.execute(
        "INSERT INTO teams (id, nome, nome_curto, categoria, meta_posicao, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            "T_BAD_META",
            "Bad Meta Team",
            "BMT",
            "gt3",
            "abc",
            "2026-01-01",
            "2026-01-01",
        ],
    )
    .expect("insert invalid meta_posicao team");

    let result = get_team_by_id(&conn, "T_BAD_META");
    assert!(
        result.is_err(),
        "meta_posicao invalida deve retornar erro, nao cair em default silencioso"
    );
}

#[test]
fn test_raw_legacy_team_row_without_current_schema_returns_error() {
    let conn = Connection::open_in_memory().expect("legacy db");
    conn.execute_batch(
        "CREATE TABLE teams (
                id TEXT PRIMARY KEY,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT ''
            );
            INSERT INTO teams (id, nome, categoria, created_at)
            VALUES ('T_OLD', 'Equipe Legada', 'gt3', '2026-01-01');",
    )
    .expect("legacy schema");

    let result = get_team_by_id(&conn, "T_OLD");
    assert!(
        result.is_err(),
        "raw legacy teams schema must be migrated before query mapping"
    );
}

fn sample_team(category_id: &str, team_id: &str) -> Team {
    let template = get_team_templates(category_id)[0];
    let mut rng = StdRng::seed_from_u64(55);
    Team::from_template_with_rng(template, category_id, team_id.to_string(), 2026, &mut rng)
}

fn setup_test_db() -> Result<Connection, DbError> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(
        "CREATE TABLE teams (
                id TEXT PRIMARY KEY,
                nome TEXT NOT NULL,
                nome_curto TEXT NOT NULL,
                cor_primaria TEXT NOT NULL DEFAULT '#FFFFFF',
                cor_secundaria TEXT NOT NULL DEFAULT '#000000',
                pais_sede TEXT NOT NULL DEFAULT 'Unknown',
                ano_fundacao INTEGER NOT NULL DEFAULT 2024,
                categoria TEXT NOT NULL,
                ativa INTEGER NOT NULL DEFAULT 1,
                marca TEXT,
                classe TEXT,
                piloto_1_id TEXT,
                piloto_2_id TEXT,
                is_player_team INTEGER NOT NULL DEFAULT 0,
                car_performance REAL NOT NULL DEFAULT 0.0,
                car_build_profile TEXT NOT NULL DEFAULT 'balanced',
                confiabilidade REAL NOT NULL DEFAULT 60.0,
                pit_strategy_risk REAL NOT NULL DEFAULT 50.0,
                pit_crew_quality REAL NOT NULL DEFAULT 50.0,
                budget REAL NOT NULL DEFAULT 50.0,
                cash_balance REAL NOT NULL DEFAULT 0.0,
                debt_balance REAL NOT NULL DEFAULT 0.0,
                financial_state TEXT NOT NULL DEFAULT 'stable',
                season_strategy TEXT NOT NULL DEFAULT 'balanced',
                last_round_income REAL NOT NULL DEFAULT 0.0,
                last_round_expenses REAL NOT NULL DEFAULT 0.0,
                last_round_net REAL NOT NULL DEFAULT 0.0,
                parachute_payment_remaining REAL NOT NULL DEFAULT 0.0,
                facilities REAL NOT NULL DEFAULT 50.0,
                engineering REAL NOT NULL DEFAULT 50.0,
                reputacao REAL NOT NULL DEFAULT 50.0,
                morale REAL NOT NULL DEFAULT 1.0,
                aerodinamica REAL NOT NULL DEFAULT 50.0,
                motor REAL NOT NULL DEFAULT 50.0,
                chassi REAL NOT NULL DEFAULT 50.0,
                hierarquia_n1_id TEXT,
                hierarquia_n2_id TEXT,
                hierarquia_status TEXT NOT NULL DEFAULT 'estavel',
                hierarquia_tensao REAL NOT NULL DEFAULT 0.0,
                hierarquia_duelos_total INTEGER NOT NULL DEFAULT 0,
                hierarquia_duelos_n2_vencidos INTEGER NOT NULL DEFAULT 0,
                hierarquia_sequencia_n2 INTEGER NOT NULL DEFAULT 0,
                hierarquia_sequencia_n1 INTEGER NOT NULL DEFAULT 0,
                hierarquia_inversoes_temporada INTEGER NOT NULL DEFAULT 0,
                parent_team_id TEXT,
                aceita_rookies INTEGER NOT NULL DEFAULT 1,
                meta_posicao INTEGER NOT NULL DEFAULT 10,
                stats_vitorias INTEGER NOT NULL DEFAULT 0,
                stats_podios INTEGER NOT NULL DEFAULT 0,
                stats_poles INTEGER NOT NULL DEFAULT 0,
                stats_pontos INTEGER NOT NULL DEFAULT 0,
                stats_melhor_resultado INTEGER NOT NULL DEFAULT 99,
                temp_posicao INTEGER NOT NULL DEFAULT 0,
                historico_vitorias INTEGER NOT NULL DEFAULT 0,
                historico_podios INTEGER NOT NULL DEFAULT 0,
                historico_poles INTEGER NOT NULL DEFAULT 0,
                historico_pontos INTEGER NOT NULL DEFAULT 0,
                historico_titulos_pilotos INTEGER NOT NULL DEFAULT 0,
                carreira_titulos INTEGER NOT NULL DEFAULT 0,
                temporada_atual INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL DEFAULT '',
                categoria_anterior TEXT
            );",
    )?;
    Ok(conn)
}

fn test_column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({})", table))
        .expect("pragma table_info");
    let mut rows = stmt.query([]).expect("query pragma");

    while let Some(row) = rows.next().expect("next row") {
        let name: String = row.get("name").expect("column name");
        if name == column {
            return true;
        }
    }

    false
}

#[test]
fn test_constructor_title_queries() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE team_season_archive (
                team_id TEXT, season_number INTEGER, categoria TEXT, titulos_construtores INTEGER
             );
             INSERT INTO team_season_archive (team_id, season_number, categoria, titulos_construtores) VALUES
             ('TA', 1, 'gt3', 1), ('TA', 2, 'gt3', 1), ('TA', 3, 'gt3', 1),
             ('TB', 1, 'gt3', 1), ('TB', 4, 'gt3', 0);",
    )
    .unwrap();
    assert_eq!(
        get_team_category_constructor_titles(&conn, "TA", "gt3").unwrap(),
        3
    );
    assert_eq!(
        get_team_category_constructor_titles(&conn, "TB", "gt3").unwrap(),
        1
    );
    // Maior entre as outras exceto TA → TB, com 1.
    assert_eq!(
        get_category_constructor_titles_leader_excluding(&conn, "gt3", "TA").unwrap(),
        1
    );
}

#[test]
fn test_team_win_queries() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE calendar (id TEXT PRIMARY KEY, categoria TEXT);
             CREATE TABLE race_results (race_id TEXT, equipe_id TEXT, posicao_final INTEGER);
             INSERT INTO calendar (id, categoria) VALUES ('R1', 'gt3'), ('R2', 'gt3'), ('R3', 'gt3');
             INSERT INTO race_results (race_id, equipe_id, posicao_final) VALUES
             ('R1', 'TA', 1), ('R2', 'TA', 1), ('R3', 'TB', 1), ('R1', 'TB', 2);",
    )
    .unwrap();
    assert_eq!(get_team_category_wins(&conn, "TA", "gt3").unwrap(), 2);
    assert_eq!(get_team_category_wins(&conn, "TB", "gt3").unwrap(), 1);
    // Maior entre as outras exceto TA → TB, com 1.
    assert_eq!(
        get_category_team_win_leader_excluding(&conn, "gt3", "TA").unwrap(),
        1
    );
}

#[test]
fn test_one_two_queries() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE calendar (id TEXT PRIMARY KEY, categoria TEXT);
             CREATE TABLE race_results (race_id TEXT, equipe_id TEXT, posicao_final INTEGER);
             INSERT INTO calendar (id, categoria) VALUES ('R1', 'gt3'), ('R2', 'gt3'), ('R3', 'gt3');
             -- R1: TA faz dobradinha (1 e 2). R2: TA vence mas 2º é da TB (não é dobradinha).
             -- R3: TA dobradinha de novo (1 e 2).
             INSERT INTO race_results (race_id, equipe_id, posicao_final) VALUES
             ('R1', 'TA', 1), ('R1', 'TA', 2),
             ('R2', 'TA', 1), ('R2', 'TB', 2),
             ('R3', 'TA', 1), ('R3', 'TA', 2);",
    )
    .unwrap();
    // TA tem 2 dobradinhas (R1 e R3); a R2 não conta (2º foi da TB).
    assert_eq!(get_team_category_one_two(&conn, "TA", "gt3").unwrap(), 2);
    assert_eq!(get_team_category_one_two(&conn, "TB", "gt3").unwrap(), 0);
    // Ninguém além de TA fez dobradinha.
    assert_eq!(
        get_category_one_two_leader_excluding(&conn, "gt3", "TA").unwrap(),
        0
    );
}
