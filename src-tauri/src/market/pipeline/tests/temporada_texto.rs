//! Temporadas de dois dígitos nas consultas de "último contrato do jogador".
//!
//! `temporada_inicio` e `temporada_fim` são colunas **TEXT** no schema real
//! ([`crate::db::migrations::baseline`]). Sem `CAST(... AS INTEGER)` o `ORDER BY`
//! é lexicográfico, e aí `'26' < '9'`: o "mais recente" vira a temporada 9.
//!
//! Os casos usam 9, 10, 12 e 26 porque é o menor conjunto em que a ordem
//! lexicográfica e a numérica discordam nas duas direções.

use rand::{rngs::StdRng, SeedableRng};
use rusqlite::Connection;

use super::*;
use crate::db::migrations;

/// Banco só com o schema real, sem semear mundo: aqui o que importa é o tipo das
/// colunas de temporada, e ele vem da migração.
fn conn_com_schema() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");
    conn
}

/// Piloto e equipe mínimos. As chaves estrangeiras de `contracts` são cobradas de
/// verdade nesta conexão, então o contrato precisa dos dois lados de pé.
fn piloto_e_equipe(conn: &Connection, piloto_id: &str, equipe_id: &str, categoria: &str) {
    conn.execute(
        "INSERT OR IGNORE INTO drivers (id, nome, idade, nacionalidade)
         VALUES (?1, 'Jogador', 28, 'BR')",
        rusqlite::params![piloto_id],
    )
    .expect("insert piloto");
    conn.execute(
        "INSERT OR IGNORE INTO teams (id, nome, categoria) VALUES (?1, 'Equipe', ?2)",
        rusqlite::params![equipe_id, categoria],
    )
    .expect("insert equipe");
}

/// Insere um contrato encerrado com vigência explícita. Todos ficam `Expirado`
/// para não esbarrar no índice único de um ativo por (piloto, tipo).
fn contrato_encerrado(
    conn: &Connection,
    id: &str,
    piloto_id: &str,
    equipe_id: &str,
    categoria: &str,
    inicio: i32,
    fim: i32,
) {
    piloto_e_equipe(conn, piloto_id, equipe_id, categoria);
    conn.execute(
        "INSERT INTO contracts (
            id, piloto_id, piloto_nome, equipe_id, equipe_nome, categoria,
            tipo, status, papel, salario, salario_anual, duracao_anos,
            temporada_inicio, temporada_fim, created_at
        ) VALUES (
            ?1, ?2, 'Jogador', ?3, 'Equipe', ?4,
            'Regular', 'Expirado', 'Numero1', 0.0, 0.0, ?5,
            ?6, ?7, '2026-01-01T00:00:00Z'
        )",
        rusqlite::params![
            id,
            piloto_id,
            equipe_id,
            categoria,
            fim - inicio + 1,
            inicio,
            fim
        ],
    )
    .expect("insert contrato");
}

/// As quatro temporadas na ordem em que a string atrapalha: a 9 é a maior em
/// texto e a menor em número.
fn semear_historico_de_dois_digitos(conn: &Connection) {
    contrato_encerrado(conn, "C09", "P_JOG", "T009", "mazda_rookie", 9, 9);
    contrato_encerrado(conn, "C26", "P_JOG", "T026", "gt3", 26, 27);
    contrato_encerrado(conn, "C10", "P_JOG", "T010", "toyota_amador", 10, 11);
    contrato_encerrado(conn, "C12", "P_JOG", "T012", "gt4", 12, 25);
}

#[test]
fn ultima_categoria_do_jogador_vem_da_temporada_maior_e_nao_da_string_maior() {
    let conn = conn_com_schema();
    semear_historico_de_dois_digitos(&conn);

    let categoria = find_last_player_category(&conn, "P_JOG").expect("última categoria");

    assert_eq!(
        categoria, "gt3",
        "a temporada 26 é a mais recente; em TEXT puro a 9 encabeçava a ordenação",
    );
}

#[test]
fn equipe_anterior_do_jogador_vem_da_temporada_maior_e_nao_da_string_maior() {
    let conn = conn_com_schema();
    semear_historico_de_dois_digitos(&conn);

    let mut rng = StdRng::seed_from_u64(2609);
    let equipe_09 = sample_team("mazda_rookie", "T009", &mut rng);
    let equipe_10 = sample_team("toyota_amador", "T010", &mut rng);
    let equipe_12 = sample_team("gt4", "T012", &mut rng);
    let equipe_26 = sample_team("gt3", "T026", &mut rng);
    let equipes = [&equipe_09, &equipe_10, &equipe_12, &equipe_26];

    let anterior = find_previous_team_for_player(&conn, "P_JOG", &equipes)
        .expect("equipe anterior")
        .expect("o jogador tem histórico");

    assert_eq!(
        anterior.id, "T026",
        "a equipe anterior é a da temporada 26, mesmo com '26' < '9' em texto",
    );
}

/// A ordem completa importa, não só o topo: com a 26 fora, quem assume é a 12, e
/// depois a 10. Em texto a sequência seria 9, 26, 12, 10.
#[test]
fn a_ordem_das_temporadas_de_dois_digitos_e_numerica_em_toda_a_lista() {
    let conn = conn_com_schema();
    semear_historico_de_dois_digitos(&conn);

    let esperado = [
        ("C26", "gt3"),
        ("C12", "gt4"),
        ("C10", "toyota_amador"),
        ("C09", "mazda_rookie"),
    ];

    for (id_removido, categoria_esperada) in esperado {
        let categoria = find_last_player_category(&conn, "P_JOG").expect("última categoria");
        assert_eq!(
            categoria, categoria_esperada,
            "com {id_removido} ainda no histórico, a categoria mais recente é {categoria_esperada}",
        );
        conn.execute("DELETE FROM contracts WHERE id = ?1", [id_removido])
            .expect("remover contrato do topo");
    }
}
