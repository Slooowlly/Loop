//! PosEspecial: o que a virada do bloco especial desfaz — contratos que expiram,
//! marcas de convocação que somem e as escalações que voltam ao normal.

use super::super::*;
use super::fases_e_grid::setup_world_db;
use crate::db::queries::{drivers as dq, seasons as sq};
use crate::models::enums::SeasonPhase;
use rusqlite::Connection;
// ── Testes PosEspecial ────────────────────────────────────────────────────

/// Helper: avança até BlocoEspecial com convocação completa.
fn setup_bloco_especial(conn: &Connection) {
    advance_to_convocation_window(conn).expect("advance to janela");
    run_convocation_window(conn).expect("run convocação");
    iniciar_bloco_especial(conn).expect("iniciar bloco especial");
}

#[test]
fn test_encerrar_bloco_especial_transitions_phase() {
    let (conn, season_id) = setup_world_db();
    setup_bloco_especial(&conn);

    encerrar_bloco_especial(&conn).expect("encerrar bloco especial");
    let s = sq::get_season_by_id(&conn, &season_id).unwrap().unwrap();
    assert_eq!(s.fase, SeasonPhase::PosEspecial);
}

#[test]
fn test_encerrar_bloco_especial_rejects_wrong_phase() {
    let (conn, _) = setup_world_db();
    // Estamos em BlocoRegular, não BlocoEspecial
    let result = encerrar_bloco_especial(&conn);
    assert!(result.is_err(), "deveria rejeitar fora de BlocoEspecial");
}

#[test]
fn test_run_pos_especial_rejects_wrong_phase() {
    let (conn, _) = setup_world_db();
    // Estamos em BlocoRegular, não PosEspecial
    let result = run_pos_especial(&conn);
    assert!(result.is_err(), "deveria rejeitar fora de PosEspecial");
}

#[test]
fn test_run_pos_especial_expires_especial_contracts() {
    let (conn, _) = setup_world_db();
    setup_bloco_especial(&conn);
    encerrar_bloco_especial(&conn).expect("encerrar");

    run_pos_especial(&conn).expect("run pos especial");

    let ativos: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM contracts WHERE tipo='Especial' AND status='Ativo'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    assert_eq!(
        ativos, 0,
        "contratos Especial ainda ativos após PosEspecial: {}",
        ativos
    );
}

#[test]
fn test_run_pos_especial_clears_categoria_especial_ativa() {
    let (conn, _) = setup_world_db();
    setup_bloco_especial(&conn);
    encerrar_bloco_especial(&conn).expect("encerrar");

    run_pos_especial(&conn).expect("run pos especial");

    let com_especial: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM drivers WHERE categoria_especial_ativa IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    assert_eq!(
        com_especial, 0,
        "pilotos com categoria_especial_ativa após PosEspecial: {}",
        com_especial
    );
}

#[test]
fn test_run_pos_especial_clears_team_lineups() {
    let (conn, _) = setup_world_db();
    setup_bloco_especial(&conn);
    encerrar_bloco_especial(&conn).expect("encerrar");

    run_pos_especial(&conn).expect("run pos especial");

    let com_pilotos: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM teams WHERE categoria IN ('production_challenger','endurance') AND piloto_1_id IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    assert_eq!(
        com_pilotos, 36,
        "lineups reais de Production/Endurance devem permanecer apos PosEspecial"
    );
}

#[test]
fn test_run_pos_especial_resets_hierarchy() {
    let (conn, _) = setup_world_db();
    setup_bloco_especial(&conn);
    encerrar_bloco_especial(&conn).expect("encerrar");

    run_pos_especial(&conn).expect("run pos especial");

    let com_hierarquia: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM teams WHERE categoria IN ('production_challenger','endurance') AND hierarquia_n1_id IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    assert_eq!(
        com_hierarquia, 36,
        "hierarquias reais de Production/Endurance devem permanecer apos PosEspecial"
    );
}

#[test]
fn test_run_pos_especial_does_not_touch_production_endurance_legacy_marks_or_lineups() {
    let (conn, season_id) = setup_world_db();
    let season = sq::get_season_by_id(&conn, &season_id)
        .expect("season query")
        .expect("season");
    sq::update_season_fase(&conn, &season_id, &SeasonPhase::PosEspecial)
        .expect("force pos especial");

    let production_team_id: String = conn
        .query_row(
            "SELECT id FROM teams
             WHERE categoria = 'production_challenger'
               AND piloto_1_id IS NOT NULL
               AND piloto_2_id IS NOT NULL
             ORDER BY id
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("production team with lineup");
    let production_driver_id: String = conn
        .query_row(
            "SELECT piloto_1_id FROM teams WHERE id = ?1",
            rusqlite::params![production_team_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .expect("production pilot")
        .expect("pilot id");
    let production_driver_name: String = conn
        .query_row(
            "SELECT nome FROM drivers WHERE id = ?1",
            rusqlite::params![production_driver_id],
            |row| row.get(0),
        )
        .expect("production pilot name");
    let production_team_name: String = conn
        .query_row(
            "SELECT nome FROM teams WHERE id = ?1",
            rusqlite::params![production_team_id],
            |row| row.get(0),
        )
        .expect("production team name");

    dq::update_driver_especial_category(
        &conn,
        &production_driver_id,
        Some("production_challenger"),
    )
    .expect("seed legacy special mark");

    conn.execute(
        "INSERT INTO contracts (
            id, piloto_id, piloto_nome, equipe_id, equipe_nome,
            temporada_inicio, duracao_anos, temporada_fim,
            salario, salario_anual, papel, status, tipo, categoria, classe, created_at
        ) VALUES (
            'C-LEGACY-PROD-SPECIAL', ?1, ?2, ?3, ?4,
            ?5, 1, ?5,
            0, 0, 'Numero1', 'Ativo', 'Especial', 'production_challenger', 'mazda',
            '2024-01-01T00:00:00Z'
        )",
        rusqlite::params![
            production_driver_id,
            production_driver_name,
            production_team_id,
            production_team_name,
            season.numero
        ],
    )
    .expect("insert legacy production special contract");

    let result = run_pos_especial(&conn).expect("run pos especial");

    let refreshed_driver = dq::get_driver(&conn, &production_driver_id).expect("driver");
    assert_eq!(
        refreshed_driver.categoria_especial_ativa.as_deref(),
        Some("production_challenger"),
        "PosEspecial nao deve limpar categoria_especial_ativa legada de Production/Endurance"
    );

    let lineups_reais: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM teams
             WHERE categoria IN ('production_challenger','endurance')
               AND piloto_1_id IS NOT NULL
               AND piloto_2_id IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .expect("lineup count");
    assert_eq!(
        lineups_reais, 36,
        "PosEspecial nao deve limpar lineups reais de Production/Endurance"
    );
    assert_eq!(
        result.contratos_encerrados, 0,
        "contratos Especial legados de Production/Endurance nao devem acionar cleanup legado"
    );
}

// --------------------------------------------------------------------------
// Temporadas de dois dígitos: `contracts.temporada_inicio` é coluna TEXT. A
// igualdade contra parâmetro inteiro só acerta enquanto os dois lados
// escreverem o número igual, e é por isso que a consulta de campeões usa
// `CAST(... AS INTEGER)`.
// --------------------------------------------------------------------------

fn conn_com_schema_para_campeoes() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    crate::db::migrations::run_all(&conn).expect("schema");
    conn
}

/// Piloto com pontuação, e o contrato Especial ativo dele gravado com a
/// temporada **como texto cru**, para o teste escrever `'09'` e `'026'`.
fn campeao_stub(
    conn: &Connection,
    piloto_id: &str,
    nome: &str,
    pontos: f64,
    inicio_texto: &str,
    classe: &str,
) {
    conn.execute(
        "INSERT INTO drivers (id, nome, idade, nacionalidade, temp_pontos)
         VALUES (?1, ?2, 28, 'BR', ?3)",
        rusqlite::params![piloto_id, nome, pontos],
    )
    .expect("insert piloto");
    // A chave estrangeira de `contracts` vale nesta conexão: a equipe precisa existir.
    conn.execute(
        "INSERT OR IGNORE INTO teams (id, nome, categoria)
         VALUES ('T001', 'Equipe', 'production_challenger')",
        [],
    )
    .expect("insert equipe");
    conn.execute(
        "INSERT INTO contracts (
            id, piloto_id, piloto_nome, equipe_id, equipe_nome, categoria, classe,
            tipo, status, papel, salario, salario_anual, duracao_anos,
            temporada_inicio, temporada_fim, created_at
        ) VALUES (
            ?1, ?2, ?3, 'T001', 'Equipe', 'production_challenger', ?4,
            'Especial', 'Ativo', 'Numero1', 0.0, 0.0, 1,
            ?5, ?5, '2026-01-01T00:00:00Z'
        )",
        rusqlite::params![
            format!("CE_{piloto_id}"),
            piloto_id,
            nome,
            classe,
            inicio_texto
        ],
    )
    .expect("insert contrato especial");
}

fn campeao_da_classe(
    campeoes: &[(String, String, Option<String>, Option<String>)],
    classe: &str,
) -> Option<String> {
    campeoes
        .iter()
        .find(|(_, class_name, _, _)| class_name == classe)
        .and_then(|(_, _, nome, _)| nome.clone())
}

/// Temporada 9 gravada como `'09'` continua sendo a temporada 9. Em comparação
/// de texto o campeão sumia e a classe voltava vazia.
#[test]
fn campeoes_especiais_encontram_a_temporada_9_gravada_com_zero_a_esquerda() {
    let conn = conn_com_schema_para_campeoes();
    campeao_stub(&conn, "P09", "Piloto Nove", 120.0, "09", "mazda");
    campeao_stub(&conn, "P26", "Piloto Vinte e Seis", 300.0, "26", "toyota");

    let campeoes = query_campeoes_especiais(&conn, 9).expect("campeões da temporada 9");

    assert_eq!(
        campeao_da_classe(&campeoes, "mazda").as_deref(),
        Some("Piloto Nove"),
        "'09' é a temporada 9",
    );
    assert_eq!(
        campeao_da_classe(&campeoes, "toyota"),
        None,
        "a temporada 26 não entra na apuração da 9",
    );
}

/// E o inverso: `'026'` é a temporada 26, e pedi-la não pode trazer a 9, que é a
/// maior das quatro em ordem lexicográfica.
#[test]
fn campeoes_especiais_encontram_a_temporada_26_e_nao_arrastam_a_9() {
    let conn = conn_com_schema_para_campeoes();
    campeao_stub(&conn, "P09", "Piloto Nove", 400.0, "9", "mazda");
    campeao_stub(&conn, "P10", "Piloto Dez", 200.0, "10", "toyota");
    campeao_stub(&conn, "P12", "Piloto Doze", 250.0, "12", "mazda");
    campeao_stub(&conn, "P26", "Piloto Vinte e Seis", 100.0, "026", "toyota");

    let campeoes = query_campeoes_especiais(&conn, 26).expect("campeões da temporada 26");

    assert_eq!(
        campeao_da_classe(&campeoes, "toyota").as_deref(),
        Some("Piloto Vinte e Seis"),
        "'026' é a temporada 26",
    );
    assert_eq!(
        campeao_da_classe(&campeoes, "mazda"),
        None,
        "nenhum contrato mazda é da temporada 26",
    );
}
