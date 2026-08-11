//! Testes dos dois módulos de query que ainda estavam sem nenhum.
//!
//! `rivalry_episodes` e `special_team_entries` fechavam a lista dos sete módulos do
//! diretório com zero `#[cfg(test)]`. Os dois guardam estado que só aparece MUITO depois
//! de gravado — o arco de uma rivalidade ao longo de temporadas, a vaga garantida do ano
//! que vem — e é exatamente esse tipo de dado que ninguém percebe estar errado até a
//! temporada seguinte.
//!
//! Tudo roda contra o schema real (`migrations::run_all`), não contra tabela de bancada.

use rusqlite::Connection;

use crate::db::migrations::run_all;
use crate::db::queries::rivalry_episodes::RivalryEpisode;
use crate::db::queries::special_team_entries::NewSpecialTeamEntry;
use crate::db::queries::{rivalry_episodes, special_team_entries};

fn banco() -> Connection {
    let conn = Connection::open_in_memory().expect("banco em memória");
    run_all(&conn).expect("migrações");
    conn
}

fn episodio(p1: &str, p2: &str, temporada: i32, rodada: i32) -> RivalryEpisode {
    RivalryEpisode {
        piloto1_id: p1.to_string(),
        piloto2_id: p2.to_string(),
        temporada,
        rodada,
        ano: 2025 + temporada,
        categoria: "gt3".to_string(),
        track_name: "Interlagos".to_string(),
        interaction: "colisao".to_string(),
        winner_id: Some(p1.to_string()),
        summary: "Bateram na entrada do S".to_string(),
        perceived: 0.7,
    }
}

// ── rivalry_episodes: a memória da novela ────────────────────────────────────

/// O par é normalizado na gravação, então a ordem em que o caller passa os dois pilotos
/// não pode criar duas rivalidades diferentes entre as mesmas duas pessoas.
#[test]
fn o_par_e_normalizado_e_a_ordem_dos_argumentos_nao_cria_duas_rivalidades() {
    let conn = banco();
    rivalry_episodes::insert_episode(&conn, &episodio("D009", "D002", 1, 3)).expect("gravar");

    let pela_ordem_dada = rivalry_episodes::get_episodes_for_pair(&conn, "D009", "D002")
        .expect("leitura na ordem dada");
    let pela_ordem_inversa = rivalry_episodes::get_episodes_for_pair(&conn, "D002", "D009")
        .expect("leitura na ordem inversa");

    assert_eq!(pela_ordem_dada.len(), 1);
    assert_eq!(pela_ordem_inversa.len(), 1);
    assert_eq!(
        (
            pela_ordem_dada[0].piloto1_id.as_str(),
            pela_ordem_dada[0].piloto2_id.as_str()
        ),
        ("D002", "D009"),
        "o menor id fica sempre em piloto1_id"
    );
}

/// Duas fontes gravam o capítulo da mesma rodada (a percepção do import e o boletim).
/// Quem chegar primeiro vence, e o arco não ganha um capítulo fantasma.
#[test]
fn o_capitulo_da_rodada_nao_duplica_quando_duas_fontes_gravam() {
    let conn = banco();
    rivalry_episodes::insert_episode(&conn, &episodio("D001", "D002", 1, 3)).expect("import");

    let mut pelo_boletim = episodio("D001", "D002", 1, 3);
    pelo_boletim.summary = "Outra redação do mesmo capítulo".to_string();
    rivalry_episodes::insert_episode(&conn, &pelo_boletim).expect("boletim");

    let episodios =
        rivalry_episodes::get_episodes_for_pair(&conn, "D001", "D002").expect("leitura");
    assert_eq!(episodios.len(), 1, "a mesma rodada é um capítulo só");
    assert_eq!(
        episodios[0].summary, "Bateram na entrada do S",
        "quem gravou primeiro é quem fica"
    );
}

/// Outra rodada é outro capítulo — a trava é por (par, temporada, rodada), não por par.
#[test]
fn rodadas_diferentes_viram_capitulos_diferentes_em_ordem_cronologica() {
    let conn = banco();
    // Gravados fora de ordem de propósito: a ordem é da leitura, não da escrita.
    rivalry_episodes::insert_episode(&conn, &episodio("D001", "D002", 2, 1)).expect("t2 r1");
    rivalry_episodes::insert_episode(&conn, &episodio("D001", "D002", 1, 7)).expect("t1 r7");
    rivalry_episodes::insert_episode(&conn, &episodio("D001", "D002", 1, 3)).expect("t1 r3");

    let episodios =
        rivalry_episodes::get_episodes_for_pair(&conn, "D001", "D002").expect("leitura");
    let arco: Vec<(i32, i32)> = episodios
        .iter()
        .map(|ep| (ep.temporada, ep.rodada))
        .collect();
    assert_eq!(
        arco,
        vec![(1, 3), (1, 7), (2, 1)],
        "o arco é lido do começo, e o primeiro capítulo é o que dá nome à rivalidade"
    );
}

/// Par sem histórico devolve lista vazia — é o caso comum, e não pode ser erro.
#[test]
fn par_sem_historico_devolve_lista_vazia() {
    let conn = banco();
    assert!(
        rivalry_episodes::get_episodes_for_pair(&conn, "D001", "D002")
            .expect("leitura")
            .is_empty()
    );
}

/// O nome da rivalidade sai do PRIMEIRO capítulo e carrega a pista da origem.
///
/// Não asseveramos a prosa (o locale é global do processo e mudaria com o idioma); o que
/// vale aqui é que a pista interpolada aparece e que interações diferentes não colapsam
/// no mesmo rótulo.
#[test]
fn o_nome_da_rivalidade_carrega_a_pista_da_origem() {
    let colisao = episodio("D001", "D002", 1, 3);
    let rotulo_colisao = rivalry_episodes::rivalry_label(&colisao);
    assert!(
        rotulo_colisao.contains("Interlagos"),
        "a origem tem pista e ela precisa aparecer no rótulo, e veio: {rotulo_colisao}"
    );

    let mut sem_pista = episodio("D001", "D002", 1, 3);
    sem_pista.track_name = String::new();
    let rotulo_sem_pista = rivalry_episodes::rivalry_label(&sem_pista);
    assert!(!rotulo_sem_pista.is_empty());
    assert!(
        !rotulo_sem_pista.contains("Interlagos"),
        "sem pista na origem, o rótulo não pode inventar uma"
    );

    let mut campeonato = episodio("D001", "D002", 1, 3);
    campeonato.interaction = "campeonato".to_string();
    assert_ne!(
        rivalry_episodes::rivalry_label(&campeonato),
        rotulo_colisao,
        "briga de título e revanche de batida não são a mesma novela"
    );
}

// ── special_team_entries: o grid do bloco especial ───────────────────────────

fn entrada(team_id: &str, via: &str, garantida: bool) -> NewSpecialTeamEntry {
    NewSpecialTeamEntry {
        team_id: team_id.to_string(),
        source_category: "gt3".to_string(),
        qualified_via: via.to_string(),
        guaranteed_next_year: garantida,
    }
}

fn cria_equipe(conn: &Connection, id: &str, nome: &str) {
    conn.execute(
        "INSERT INTO teams (id, nome, categoria) VALUES (?1, ?2, 'endurance')",
        rusqlite::params![id, nome],
    )
    .expect("equipe");
}

/// O mínimo de mundo que `special_team_entries` exige para existir.
///
/// A tabela tem chave estrangeira para `seasons` e para `teams`, e as migrações deixam
/// `PRAGMA foreign_keys` LIGADO — uma entrada apontando para equipe que não existe é
/// rejeitada pelo banco, não aceita em silêncio. Cada teste daqui para baixo precisa do
/// mundo antes do grid.
fn mundo_com_temporadas_e_equipes(conn: &Connection) {
    conn.execute_batch(
        "INSERT INTO seasons (id, numero, ano) VALUES ('S1', 1, 2026), ('S2', 2, 2027);",
    )
    .expect("temporadas");
    cria_equipe(conn, "T001", "Alfa");
    cria_equipe(conn, "T002", "Beta");
    cria_equipe(conn, "T003", "Gama");
}

fn cria_piloto(conn: &Connection, id: &str) {
    let piloto = crate::models::driver::Driver::new(
        id.to_string(),
        format!("Piloto {id}"),
        "br".to_string(),
        "M".to_string(),
        24,
        2020,
    );
    crate::db::queries::drivers::insert_driver(conn, &piloto).expect("piloto");
}

/// Regravar a classe SUBSTITUI o grid inteiro dela. Se acumulasse, o grid do bloco
/// especial dobraria a cada regeração.
#[test]
fn regravar_a_classe_substitui_o_grid_e_nao_acumula() {
    let conn = banco();
    mundo_com_temporadas_e_equipes(&conn);

    special_team_entries::replace_entries_for_class(
        &conn,
        "S1",
        "endurance",
        "LMP2",
        &[
            entrada("T001", "ranking", false),
            entrada("T002", "ranking", false),
        ],
    )
    .expect("primeira montagem");

    special_team_entries::replace_entries_for_class(
        &conn,
        "S1",
        "endurance",
        "LMP2",
        &[entrada("T003", "ranking", false)],
    )
    .expect("remontagem");

    let entradas = special_team_entries::get_entries_for_class(&conn, "S1", "endurance", "LMP2")
        .expect("leitura");
    assert_eq!(entradas.len(), 1);
    assert_eq!(entradas[0].team_id, "T003");
}

/// A substituição é cirúrgica: mexe só na classe pedida, não no resto do grid.
#[test]
fn regravar_uma_classe_nao_toca_nas_outras() {
    let conn = banco();
    mundo_com_temporadas_e_equipes(&conn);
    special_team_entries::replace_entries_for_class(
        &conn,
        "S1",
        "endurance",
        "LMP2",
        &[entrada("T001", "ranking", false)],
    )
    .expect("LMP2");
    special_team_entries::replace_entries_for_class(
        &conn,
        "S1",
        "endurance",
        "GTE",
        &[entrada("T002", "ranking", false)],
    )
    .expect("GTE");

    special_team_entries::replace_entries_for_class(
        &conn,
        "S1",
        "endurance",
        "LMP2",
        &[entrada("T003", "ranking", false)],
    )
    .expect("remontar só LMP2");

    let gte = special_team_entries::get_entries_for_class(&conn, "S1", "endurance", "GTE")
        .expect("leitura GTE");
    assert_eq!(gte.len(), 1);
    assert_eq!(gte[0].team_id, "T002", "a outra classe ficou intacta");
}

/// A leitura põe as vagas GARANTIDAS na frente — é a ordem em que o grid é montado.
#[test]
fn as_vagas_garantidas_vem_primeiro_na_leitura() {
    let conn = banco();
    mundo_com_temporadas_e_equipes(&conn);
    special_team_entries::replace_entries_for_class(
        &conn,
        "S1",
        "endurance",
        "LMP2",
        &[
            entrada("T001", "ranking", false),
            entrada("T002", "convite", true),
        ],
    )
    .expect("montagem");

    let entradas = special_team_entries::get_entries_for_class(&conn, "S1", "endurance", "LMP2")
        .expect("leitura");
    assert_eq!(entradas[0].team_id, "T002");
    assert!(entradas[0].guaranteed_next_year);
    assert!(!entradas[1].guaranteed_next_year);
}

/// A garantia é uma promessa feita ao ano SEGUINTE: quem a leu na temporada N + 1 tem de
/// enxergar o que ficou marcado na temporada N, e nada do resto.
#[test]
fn a_garantia_do_ano_passado_e_lida_pela_temporada_seguinte() {
    let conn = banco();
    mundo_com_temporadas_e_equipes(&conn);

    special_team_entries::replace_entries_for_class(
        &conn,
        "S1",
        "endurance",
        "LMP2",
        &[
            entrada("T001", "ranking", true),
            entrada("T002", "ranking", false),
        ],
    )
    .expect("grid do ano passado");

    let garantidas =
        special_team_entries::get_previous_guaranteed_team_ids(&conn, 2, "endurance", "LMP2")
            .expect("garantidas do ano passado");
    assert_eq!(garantidas, vec!["T001".to_string()]);

    // Quem olha da própria temporada (numero 1) enxerga a temporada 0, que não existe.
    let sem_historico =
        special_team_entries::get_previous_guaranteed_team_ids(&conn, 1, "endurance", "LMP2")
            .expect("primeira temporada");
    assert!(
        sem_historico.is_empty(),
        "a primeira temporada do bloco não tem ano anterior de onde herdar vaga"
    );
}

/// O recálculo das garantias ZERA todas e regrava só o topo — senão a garantia de um ano
/// vira vitalícia sem ninguém decidir isso.
#[test]
fn o_recalculo_das_garantias_zera_as_antigas_antes_de_premiar_o_topo() {
    let conn = banco();
    mundo_com_temporadas_e_equipes(&conn);
    cria_piloto(&conn, "D1");
    cria_piloto(&conn, "D2");
    conn.execute_batch(
        "INSERT INTO calendar (id, temporada_id, season_id, categoria, rodada, pista, clima)
             VALUES ('C1', 'S1', 'S1', 'endurance', 1, 'Le Mans', 'Seco');
         INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, dnf, pontos)
             VALUES ('C1', 'D1', 'T002', 1, 0, 25.0),
                    ('C1', 'D2', 'T001', 2, 0, 18.0);",
    )
    .expect("resultado da etapa");

    // T001 entrou com a garantia do ano passado; T002 é quem pontuou mais nesta temporada.
    special_team_entries::replace_entries_for_class(
        &conn,
        "S1",
        "endurance",
        "LMP2",
        &[
            entrada("T001", "garantida", true),
            entrada("T002", "ranking", false),
        ],
    )
    .expect("grid");

    special_team_entries::update_guarantees_for_class(&conn, "S1", "endurance", "LMP2", 1)
        .expect("recalcular");

    let entradas = special_team_entries::get_entries_for_class(&conn, "S1", "endurance", "LMP2")
        .expect("leitura");
    let garantida: Vec<&str> = entradas
        .iter()
        .filter(|e| e.guaranteed_next_year)
        .map(|e| e.team_id.as_str())
        .collect();
    assert_eq!(
        garantida,
        vec!["T002"],
        "a vaga do ano que vem é de quem pontuou agora, não de quem tinha a vaga antes"
    );
}

/// A hidratação em `Team` ignora entrada cuja equipe sumiu do mundo, em vez de estourar.
///
/// A chave estrangeira impede criar essa entrada pelo caminho normal — e é justamente por
/// isso que o teste desliga o `PRAGMA`: o estado que se quer provar é o de um save onde a
/// equipe SUMIU depois (encerramento, save antigo, reparo mal feito), e a hidratação
/// precisa atravessar isso sem derrubar a tela do bloco especial.
#[test]
fn entrada_de_equipe_inexistente_e_ignorada_na_hidratacao() {
    let conn = banco();
    mundo_com_temporadas_e_equipes(&conn);
    conn.execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("simular save com equipe sumida");

    special_team_entries::replace_entries_for_class(
        &conn,
        "S1",
        "endurance",
        "LMP2",
        &[
            entrada("T001", "ranking", false),
            entrada("T_FANTASMA", "ranking", false),
        ],
    )
    .expect("grid");

    let equipes = special_team_entries::get_entry_teams_for_class(&conn, "S1", "endurance", "LMP2")
        .expect("hidratação");
    assert_eq!(equipes.len(), 1);
    assert_eq!(equipes[0].id, "T001");
    assert_eq!(
        equipes[0].classe.as_deref(),
        Some("LMP2"),
        "a classe da entrada é anexada à equipe hidratada"
    );
}

/// A leitura por CATEGORIA junta as classes numa lista só.
#[test]
fn a_leitura_por_categoria_traz_as_equipes_de_todas_as_classes() {
    let conn = banco();
    mundo_com_temporadas_e_equipes(&conn);
    special_team_entries::replace_entries_for_class(
        &conn,
        "S1",
        "endurance",
        "LMP2",
        &[entrada("T001", "ranking", false)],
    )
    .expect("LMP2");
    special_team_entries::replace_entries_for_class(
        &conn,
        "S1",
        "endurance",
        "GTE",
        &[entrada("T002", "ranking", false)],
    )
    .expect("GTE");

    let equipes = special_team_entries::get_entry_teams_for_category(&conn, "S1", "endurance")
        .expect("leitura por categoria");
    let mut ids: Vec<&str> = equipes.iter().map(|t| t.id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["T001", "T002"]);
}
