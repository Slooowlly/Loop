use super::*;
use rusqlite::Connection;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "
        CREATE TABLE seasons (id TEXT PRIMARY KEY, numero INTEGER NOT NULL);
        CREATE TABLE calendar (
            id TEXT PRIMARY KEY,
            temporada_id TEXT NOT NULL,
            rodada INTEGER NOT NULL,
            categoria TEXT NOT NULL,
            track_name TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE drivers (id TEXT PRIMARY KEY, nome TEXT NOT NULL);
        CREATE TABLE race_results (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            race_id TEXT NOT NULL,
            piloto_id TEXT NOT NULL,
            equipe_id TEXT NOT NULL,
            posicao_largada INTEGER NOT NULL DEFAULT 0,
            posicao_final INTEGER NOT NULL,
            dnf INTEGER NOT NULL DEFAULT 0,
            pontos REAL NOT NULL
        );
        CREATE TABLE standings (
            temporada_id TEXT NOT NULL,
            piloto_id TEXT NOT NULL,
            equipe_id TEXT NOT NULL DEFAULT '',
            categoria TEXT NOT NULL,
            classe TEXT,
            posicao INTEGER NOT NULL DEFAULT 0,
            pontos REAL NOT NULL DEFAULT 0.0,
            vitorias INTEGER NOT NULL DEFAULT 0,
            podios INTEGER NOT NULL DEFAULT 0,
            poles INTEGER NOT NULL DEFAULT 0,
            corridas INTEGER NOT NULL DEFAULT 0
        );
        INSERT INTO seasons (id, numero) VALUES ('S1', 1), ('S2', 2);
        INSERT INTO drivers (id, nome) VALUES ('P001', 'Piloto Um'), ('P002', 'Piloto Dois');
        INSERT INTO calendar (id, temporada_id, rodada, categoria, track_name) VALUES
            ('R1', 'S1', 1, 'gt4', 'Interlagos'),
            ('R2', 'S1', 2, 'gt4', 'Spa'),
            ('R3', 'S2', 1, 'gt4', 'Monza');
        ",
    )
    .unwrap();
    conn
}

#[test]
fn test_get_last_career_win() {
    let conn = setup_db();
    conn.execute(
        "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos)
         VALUES ('R2', 'P001', 'T001', 1, 25.0)",
        [],
    )
    .unwrap();

    let result = get_last_career_win(&conn, "P001").unwrap();
    assert_eq!(result, Some((1, 2)));
}

#[test]
fn test_get_last_career_win_none() {
    let conn = setup_db();
    let result = get_last_career_win(&conn, "P001").unwrap();
    assert_eq!(result, None);
}

#[test]
fn mechanical_dnf_counts_conta_so_mecanicos_na_janela() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE calendar (id TEXT PRIMARY KEY, temporada_id TEXT, rodada INTEGER, categoria TEXT);
         CREATE TABLE incident_catalog (id TEXT PRIMARY KEY, incident_source TEXT);
         CREATE TABLE race_results (id INTEGER PRIMARY KEY AUTOINCREMENT, race_id TEXT, equipe_id TEXT, dnf INTEGER, dnf_catalog_id TEXT);
         INSERT INTO calendar VALUES ('R1','S1',1,'gt4'),('R2','S1',2,'gt4'),('R3','S1',3,'gt4');
         INSERT INTO incident_catalog VALUES ('MEC','Mechanical'),('OPS','Operational'),('DRV','DriverError');
         INSERT INTO race_results (race_id, equipe_id, dnf, dnf_catalog_id) VALUES
            ('R1','TeamA',1,'MEC'),
            ('R1','TeamB',1,'DRV'),
            ('R2','TeamA',1,'OPS'),
            ('R3','TeamA',1,'MEC');",
    )
    .unwrap();
    // Janela = rodadas 1..2 (antes da 3). TeamA: MEC(r1)+OPS(r2)=2; erro do TeamB não conta;
    // o MEC da r3 (fora da janela) não entra.
    let counts = mechanical_dnf_counts_by_team(&conn, "S1", "gt4", 1, 2).unwrap();
    assert_eq!(counts.get("TeamA"), Some(&2));
    assert_eq!(counts.get("TeamB"), None);
    // Janela inválida (min > max) → vazio.
    assert!(mechanical_dnf_counts_by_team(&conn, "S1", "gt4", 2, 1)
        .unwrap()
        .is_empty());
}

#[test]
fn test_get_wins_with_team() {
    let conn = setup_db();
    conn.execute_batch(
        "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
         ('R1', 'P001', 'T001', 1, 25.0),
         ('R2', 'P001', 'T001', 1, 25.0),
         ('R3', 'P001', 'T002', 1, 25.0);",
    )
    .unwrap();

    assert_eq!(get_wins_with_team(&conn, "P001", "T001").unwrap(), 2);
    assert_eq!(get_wins_with_team(&conn, "P001", "T002").unwrap(), 1);
    assert_eq!(get_wins_with_team(&conn, "P001", "T003").unwrap(), 0);
}

#[test]
fn test_get_category_standings() {
    let conn = setup_db();
    conn.execute_batch(
        "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
         ('R1', 'P001', 'T001', 1, 25.0),
         ('R1', 'P002', 'T002', 2, 18.0),
         ('R2', 'P001', 'T001', 2, 18.0),
         ('R2', 'P002', 'T002', 1, 25.0);",
    )
    .unwrap();

    let standings = get_category_standings(&conn, "S1", "gt4").unwrap();
    assert_eq!(standings.len(), 2);
    assert_eq!(standings[0].points, 43.0);
    assert_eq!(standings[0].position, 1);
    assert_eq!(standings[1].position, 2);
}

#[test]
fn test_get_category_wins_this_season() {
    let conn = setup_db();
    conn.execute_batch(
        "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
         ('R1', 'P001', 'T001', 1, 25.0),
         ('R2', 'P001', 'T001', 2, 18.0);",
    )
    .unwrap();

    assert_eq!(
        get_category_wins_this_season(&conn, "P001", "S1", "gt4").unwrap(),
        1
    );
    assert_eq!(
        get_category_wins_this_season(&conn, "P001", "S2", "gt4").unwrap(),
        0
    );
}

#[test]
fn test_get_category_records_tracks_second_most_podiums() {
    let conn = setup_db();
    conn.execute_batch(
        "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
         ('R1', 'P001', 'T1', 1, 25.0),
         ('R2', 'P001', 'T1', 3, 15.0),
         ('R3', 'P001', 'T1', 2, 18.0),
         ('R1', 'P002', 'T2', 2, 18.0),
         ('R2', 'P002', 'T2', 3, 15.0),
         ('R3', 'P002', 'T2', 10, 1.0);",
    )
    .unwrap();

    let recs = get_category_records(&conn, "gt4").unwrap();
    // P001 subiu ao pódio 3x (P1/P3/P2); P002, 2x (P2/P3).
    let leader = recs.most_podiums.as_ref().unwrap();
    assert_eq!(leader.pilot_id, "P001");
    assert_eq!(leader.value, 3);
    assert_eq!(recs.second_most_podiums, Some(2));
}

#[test]
fn test_get_category_records_counts_poles_from_grid() {
    let conn = setup_db();
    conn.execute_batch(
        "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_largada, posicao_final, dnf, pontos) VALUES
         ('R1', 'P001', 'T1', 1, 1, 0, 25.0),
         ('R2', 'P001', 'T1', 1, 3, 0, 15.0),
         ('R3', 'P001', 'T1', 5, 2, 0, 18.0),
         ('R1', 'P002', 'T2', 2, 2, 0, 18.0),
         ('R2', 'P002', 'T2', 1, 1, 0, 25.0);",
    )
    .unwrap();

    let recs = get_category_records(&conn, "gt4").unwrap();
    // P001 largou em 1º em R1 e R2 (2 poles); P002, só em R2 (1 pole).
    let poles = recs.most_poles.as_ref().unwrap();
    assert_eq!(poles.pilot_id, "P001");
    assert_eq!(poles.value, 2);
    assert_eq!(recs.second_most_poles, Some(1));
}

#[test]
fn test_azarao_and_drought_queries() {
    let conn = setup_db();
    // P001 nunca venceu (3 largadas, 1 DNF). P002 venceu R2 e abandonou R3.
    conn.execute_batch(
        "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_largada, posicao_final, dnf, pontos) VALUES
         ('R1', 'P001', 'T1', 5, 4, 0, 12.0),
         ('R2', 'P001', 'T1', 3, 2, 0, 18.0),
         ('R3', 'P001', 'T1', 8, 20, 1, 0.0),
         ('R1', 'P002', 'T2', 1, 3, 0, 15.0),
         ('R2', 'P002', 'T2', 2, 1, 0, 25.0),
         ('R3', 'P002', 'T2', 1, 20, 1, 0.0);",
    )
    .unwrap();

    // Azarão = P001 (3 largadas, nunca venceu).
    let azarao = get_category_most_starts_no_win(&conn, "gt4")
        .unwrap()
        .unwrap();
    assert_eq!(azarao.pilot_id, "P001");
    assert_eq!(azarao.value, 3);

    // Maior pontuador = P001 (12+18+0=30) vs P002 (15+25+0=40) → P002.
    let pts = get_category_most_career_points(&conn, "gt4")
        .unwrap()
        .unwrap();
    assert_eq!(pts.pilot_id, "P002");

    // Última vitória de P002 antes de S2/r1 = S1 (R2). Jejum a partir daí.
    let prev = get_pilot_previous_win_season(&conn, "P002", "gt4", 2, 1).unwrap();
    assert_eq!(prev, Some(1));
}

#[test]
fn test_get_category_single_season_win_record() {
    let conn = setup_db();
    conn.execute_batch(
        "INSERT INTO standings (temporada_id, piloto_id, categoria, posicao, vitorias) VALUES
         ('S1', 'P001', 'gt4', 1, 7),
         ('S1', 'P002', 'gt4', 2, 4),
         ('S2', 'P002', 'gt4', 1, 9);",
    )
    .unwrap();
    let rec = get_category_single_season_win_record(&conn, "gt4")
        .unwrap()
        .unwrap();
    // Maior temporada = P002 com 9 (S2).
    assert_eq!(rec.pilot_id, "P002");
    assert_eq!(rec.value, 9);
}

#[test]
fn test_track_win_queries() {
    let conn = setup_db();
    // Calendário do setup: R1=Interlagos, R2=Spa, R3=Monza. P001 vence R1 e R2.
    conn.execute_batch(
        "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_largada, posicao_final, dnf, pontos) VALUES
         ('R1', 'P001', 'T1', 2, 1, 0, 25.0),
         ('R2', 'P001', 'T1', 1, 1, 0, 25.0),
         ('R1', 'P002', 'T2', 1, 2, 0, 18.0);",
    )
    .unwrap();

    assert_eq!(
        get_pilot_track_wins(&conn, "P001", "gt4", "Interlagos").unwrap(),
        1
    );
    assert_eq!(
        get_pilot_track_wins(&conn, "P001", "gt4", "Spa").unwrap(),
        1
    );
    assert_eq!(
        get_pilot_track_wins(&conn, "P001", "gt4", "Monza").unwrap(),
        0
    );
    // Ninguém além de P001 venceu em Interlagos.
    assert_eq!(
        get_track_win_leader_excluding(&conn, "gt4", "Interlagos", "P001").unwrap(),
        0
    );
}

#[test]
fn test_get_category_comeback_record_excludes_current_race() {
    let conn = setup_db();
    // R1 (S1/r1): P001 largou 10º, chegou 2º → ganhou 8. R2 (S1/r2): P002 5º→1º → 4.
    conn.execute_batch(
        "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_largada, posicao_final, dnf, pontos) VALUES
         ('R1', 'P001', 'T1', 10, 2, 0, 18.0),
         ('R2', 'P002', 'T2', 5, 1, 0, 25.0);",
    )
    .unwrap();

    // Excluindo R1 (S1, rodada 1) → sobra só R2, ganho 4 (P002).
    let rec = get_category_comeback_record(&conn, "gt4", "S1", 1)
        .unwrap()
        .unwrap();
    assert_eq!(rec.pilot_id, "P002");
    assert_eq!(rec.value, 4);

    // Excluindo uma corrida inexistente → o maior histórico é P001 (8).
    let rec2 = get_category_comeback_record(&conn, "gt4", "S2", 9)
        .unwrap()
        .unwrap();
    assert_eq!(rec2.pilot_id, "P001");
    assert_eq!(rec2.value, 8);
}

#[test]
fn test_get_category_standings_breaks_ties_by_name_when_points_and_wins_tie() {
    let conn = setup_db();
    conn.execute_batch(
        "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
         ('R3', 'P001', 'T001', 2, 18.0),
         ('R3', 'P002', 'T002', 2, 18.0);",
    )
    .unwrap();

    let standings = get_category_standings(&conn, "S2", "gt4").unwrap();
    assert_eq!(standings.len(), 2);
    assert_eq!(standings[0].pilot_id, "P002");
    assert_eq!(standings[1].pilot_id, "P001");
}

#[test]
fn test_get_category_leader_before_round_uses_deterministic_tiebreak() {
    let conn = setup_db();
    conn.execute_batch(
        "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos) VALUES
         ('R1', 'P001', 'T001', 1, 25.0),
         ('R1', 'P002', 'T002', 2, 18.0),
         ('R2', 'P001', 'T001', 2, 18.0),
         ('R2', 'P002', 'T002', 1, 25.0);",
    )
    .unwrap();

    let leader = get_category_leader_before_round(&conn, "S1", "gt4", 3).unwrap();
    assert_eq!(leader.as_deref(), Some("P001"));
}

#[test]
fn test_get_recent_finishes_before_excludes_current_and_orders_desc() {
    let conn = setup_db();
    conn.execute_batch(
        "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, dnf, pontos) VALUES
         ('R1', 'P001', 'T001', 5, 0, 10.0),
         ('R2', 'P001', 'T001', 1, 0, 25.0),
         ('R3', 'P001', 'T001', 3, 0, 15.0);",
    )
    .unwrap();

    // Antes de S2/r1 (numero=2, rodada=1) entram só R1 e R2 (ambas em S1).
    let recent = get_recent_finishes_before(&conn, "P001", "gt4", 2, 1, 5).unwrap();
    assert_eq!(recent.len(), 2);
    // Mais recente primeiro: R2 (rodada 2) antes de R1 (rodada 1).
    assert_eq!(recent[0].finish, 1);
    assert_eq!(recent[1].finish, 5);
}

#[test]
fn test_get_recent_races_before_carries_race_id_and_track() {
    let conn = setup_db();
    conn.execute_batch(
        "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, dnf, pontos) VALUES
         ('R1', 'P001', 'T001', 5, 0, 10.0),
         ('R2', 'P001', 'T001', 1, 0, 25.0),
         ('R3', 'P001', 'T001', 3, 0, 15.0);",
    )
    .unwrap();

    // Antes de S2/r1 entram R2 e R1 (mais recente primeiro), com chave e pista.
    let recent = get_recent_races_before(&conn, "P001", "gt4", 2, 1, 3).unwrap();
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].race_id, "R2");
    assert_eq!(recent[0].finish, 1);
    assert_eq!(recent[0].track_name, "Spa");
    assert_eq!(recent[1].race_id, "R1");
    assert_eq!(recent[1].track_name, "Interlagos");

    // Sem histórico anterior à 1ª etapa → vazio (briefing fica inalterado).
    let none = get_recent_races_before(&conn, "P001", "gt4", 1, 1, 3).unwrap();
    assert!(none.is_empty());
}

#[test]
fn test_get_pilot_season_results_ordered_by_round() {
    let conn = setup_db();
    conn.execute_batch(
        "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, dnf, pontos) VALUES
         ('R2', 'P001', 'T001', 4, 0, 12.0),
         ('R1', 'P001', 'T001', 2, 1, 0.0);",
    )
    .unwrap();

    let res = get_pilot_season_results(&conn, "P001", "S1", "gt4").unwrap();
    assert_eq!(res, vec![(1, 2, true), (2, 4, false)]);
}

#[test]
fn test_get_pilot_track_history_counts_wins_excluding_current() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE calendar (
            id TEXT PRIMARY KEY,
            temporada_id TEXT NOT NULL,
            rodada INTEGER NOT NULL,
            categoria TEXT NOT NULL,
            track_id INTEGER
        );
        CREATE TABLE race_results (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            race_id TEXT NOT NULL,
            piloto_id TEXT NOT NULL,
            posicao_final INTEGER NOT NULL,
            dnf INTEGER NOT NULL DEFAULT 0
        );
        INSERT INTO calendar (id, temporada_id, rodada, categoria, track_id) VALUES
            ('A', 'S1', 1, 'gt4', 42),
            ('B', 'S2', 1, 'gt4', 42),
            ('C', 'S3', 1, 'gt4', 42),
            ('D', 'S1', 2, 'gt4', 99);
        INSERT INTO race_results (race_id, piloto_id, posicao_final, dnf) VALUES
            ('A', 'P001', 1, 0),
            ('B', 'P001', 3, 0),
            ('C', 'P001', 1, 0),
            ('D', 'P001', 1, 0);",
    )
    .unwrap();

    assert_eq!(get_round_track_id(&conn, "S3", "gt4", 1).unwrap(), Some(42));

    // Excluindo a corrida atual (S3/r1): sobram A (1º) e B (3º) na pista 42.
    let th = get_pilot_track_history(&conn, "P001", 42, "S3", 1).unwrap();
    assert_eq!(th.starts, 2);
    assert_eq!(th.wins, 1); // só A; a vitória atual (C) foi excluída
    assert_eq!(th.podiums, 2);
    assert_eq!(th.best_finish, Some(1));
}

#[test]
fn test_get_results_for_round_propagates_invalid_row_error() {
    let conn = setup_db();
    conn.execute(
        "INSERT INTO race_results
            (race_id, piloto_id, equipe_id, posicao_largada, posicao_final, dnf, pontos)
         VALUES ('R1', 'P001', 'T001', 1, 'quebrado', 0, 25.0)",
        [],
    )
    .unwrap();

    let err = get_results_for_round(&conn, "S1", "gt4", 1)
        .expect_err("invalid row should fail instead of being ignored");
    assert!(err.to_string().contains("SQLite error"));
}
