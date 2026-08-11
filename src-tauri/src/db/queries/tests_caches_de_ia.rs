//! Testes dos módulos de query que nasceram fora do padrão do diretório.
//!
//! Sete módulos (os quatro caches de IA, o nêmesis, os episódios de rivalidade e as
//! entradas do bloco especial) não tinham bloco de teste algum, e eram justamente os que
//! criavam as próprias tabelas por `ensure_table`. A migração v62 trouxe o schema deles
//! para dentro das migrações; estes testes trazem o COMPORTAMENTO para dentro da suíte.
//!
//! O que cada um deles é, no fundo, é um cache com regra de sobrescrita. É essa regra que
//! erra na prática: reabrir uma notícia não pode regenerar (custa servidor e cooldown), e
//! guardar o texto gerado não pode apagar os fatos que o produziram.

use rusqlite::Connection;

use crate::db::migrations::run_all;
use crate::db::queries::{ai_post_race, ai_pre_race, ai_story, ai_world_notes, player_nemesis};

fn banco() -> Connection {
    let conn = Connection::open_in_memory().expect("banco em memória");
    run_all(&conn).expect("migrações");
    conn
}

// ── ai_story: fatos curados + boletim redigido ───────────────────────────────

#[test]
fn o_boletim_gravado_nao_apaga_os_fatos_que_o_produziram() {
    let conn = banco();
    ai_story::store_race_facts(
        &conn,
        "N001",
        "EIXO: venceu de ponta a ponta",
        "{\"Alfa\":\"#f00\"}",
    )
    .expect("fatos");

    ai_story::set_story(&conn, "N001", "O boletim redigido.").expect("boletim");

    let row = ai_story::get_story(&conn, "N001")
        .expect("leitura")
        .expect("linha");
    assert_eq!(row.story.as_deref(), Some("O boletim redigido."));
    assert_eq!(row.facts, "EIXO: venceu de ponta a ponta");
    assert_eq!(row.teams_json.as_deref(), Some("{\"Alfa\":\"#f00\"}"));
}

#[test]
fn regravar_os_fatos_zera_o_boletim_anterior() {
    let conn = banco();
    ai_story::store_race_facts(&conn, "N001", "fatos v1", "{}").expect("fatos");
    ai_story::set_story(&conn, "N001", "boletim v1").expect("boletim");

    // A corrida foi refeita (reimportação): os fatos mudam, e o texto velho descreve
    // uma corrida que não aconteceu mais.
    ai_story::store_race_facts(&conn, "N001", "fatos v2", "{}").expect("fatos de novo");

    let row = ai_story::get_story(&conn, "N001")
        .expect("leitura")
        .expect("linha");
    assert_eq!(row.facts, "fatos v2");
    assert_eq!(
        row.story, None,
        "o boletim velho descreve a corrida antiga e não pode sobreviver aos fatos novos"
    );
}

#[test]
fn noticia_sem_fatos_devolve_none_em_vez_de_erro() {
    let conn = banco();
    assert!(ai_story::get_story(&conn, "N_INEXISTENTE")
        .expect("leitura")
        .is_none());
}

#[test]
fn set_story_em_noticia_inexistente_e_no_op() {
    let conn = banco();
    // Sem linha para atualizar, o UPDATE não afeta nada — e não pode virar erro na tela.
    ai_story::set_story(&conn, "N_FANTASMA", "texto").expect("no-op");
    assert!(ai_story::get_story(&conn, "N_FANTASMA")
        .expect("leitura")
        .is_none());
}

#[test]
fn o_carimbo_de_tempo_acompanha_a_gravacao_dos_fatos() {
    let conn = banco();
    ai_story::store_race_facts(&conn, "N001", "fatos", "{}").expect("fatos");

    let row = ai_story::get_story(&conn, "N001")
        .expect("leitura")
        .expect("linha");
    // É o carimbo que sustenta o backoff de quem caiu no template e não quer bater no
    // servidor a cada reabertura. Vazio quebraria esse cálculo em silêncio.
    assert!(
        !row.created_at.is_empty(),
        "sem carimbo não há backoff possível"
    );
    assert!(
        row.created_at.parse::<i64>().is_ok(),
        "o carimbo precisa ser numérico"
    );
}

// ── ai_pre_race / ai_post_race: prévia e debrief por etapa ───────────────────

#[test]
fn a_previa_da_etapa_sobrevive_a_ida_e_volta() {
    let conn = banco();
    ai_pre_race::set_pre_race(
        &conn,
        "C001",
        "Manchete",
        "Narrativa longa.",
        "Voz da equipe.",
    )
    .expect("gravar");

    let row = ai_pre_race::get_pre_race(&conn, "C001")
        .expect("leitura")
        .expect("linha");
    assert_eq!(row.headline, "Manchete");
    assert_eq!(row.narrative, "Narrativa longa.");
    assert_eq!(row.team_voice, "Voz da equipe.");
}

#[test]
fn regerar_a_previa_sobrescreve_a_anterior_da_mesma_etapa() {
    let conn = banco();
    ai_pre_race::set_pre_race(&conn, "C001", "H1", "N1", "V1").expect("primeira");
    ai_pre_race::set_pre_race(&conn, "C001", "H2", "N2", "V2").expect("reroll");

    let row = ai_pre_race::get_pre_race(&conn, "C001")
        .expect("leitura")
        .expect("linha");
    assert_eq!(
        (row.headline.as_str(), row.narrative.as_str()),
        ("H2", "N2")
    );
}

#[test]
fn etapa_sem_previa_devolve_none() {
    let conn = banco();
    assert!(ai_pre_race::get_pre_race(&conn, "C_SEM")
        .expect("leitura")
        .is_none());
}

#[test]
fn o_debrief_da_etapa_sobrevive_a_ida_e_volta_e_sobrescreve() {
    let conn = banco();
    ai_post_race::set_post_race(&conn, "C001", "Manchete", "Corpo do engenheiro.").expect("gravar");
    let row = ai_post_race::get_post_race(&conn, "C001")
        .expect("leitura")
        .expect("linha");
    assert_eq!(row.headline, "Manchete");
    assert_eq!(row.body, "Corpo do engenheiro.");

    ai_post_race::set_post_race(&conn, "C001", "Outra", "Outro corpo.").expect("regravar");
    let row = ai_post_race::get_post_race(&conn, "C001")
        .expect("leitura")
        .expect("linha");
    assert_eq!(row.headline, "Outra");
}

#[test]
fn etapa_sem_debrief_devolve_none() {
    let conn = banco();
    assert!(ai_post_race::get_post_race(&conn, "C_SEM")
        .expect("leitura")
        .is_none());
}

// ── ai_world_notes: notinhas do rodapé, chaveadas por temporada:rodada ───────

#[test]
fn as_notas_do_mundo_sao_cacheadas_por_chave_e_nao_vazam_entre_rodadas() {
    let conn = banco();
    ai_world_notes::set_cached(&conn, "2:5", "[\"nota da rodada 5\"]").expect("gravar");

    assert_eq!(
        ai_world_notes::get_cached(&conn, "2:5")
            .expect("leitura")
            .as_deref(),
        Some("[\"nota da rodada 5\"]")
    );
    assert!(
        ai_world_notes::get_cached(&conn, "2:6")
            .expect("leitura")
            .is_none(),
        "o estado do mundo muda a cada rodada; uma chave não pode servir a outra"
    );
}

#[test]
fn regerar_as_notas_da_mesma_chave_sobrescreve() {
    let conn = banco();
    ai_world_notes::set_cached(&conn, "2:5", "[\"v1\"]").expect("gravar");
    ai_world_notes::set_cached(&conn, "2:5", "[\"v2\"]").expect("regravar");

    assert_eq!(
        ai_world_notes::get_cached(&conn, "2:5")
            .expect("leitura")
            .as_deref(),
        Some("[\"v2\"]")
    );
}

// ── player_nemesis: o singleton que dá histerese à seleção ───────────────────

#[test]
fn o_nemesis_e_um_so_e_o_ultimo_a_entrar_reina() {
    let conn = banco();
    assert!(player_nemesis::get_current_nemesis(&conn)
        .expect("leitura")
        .is_none());

    player_nemesis::set_current_nemesis(&conn, Some("D001")).expect("primeiro");
    assert_eq!(
        player_nemesis::get_current_nemesis(&conn)
            .expect("leitura")
            .as_deref(),
        Some("D001")
    );

    player_nemesis::set_current_nemesis(&conn, Some("D002")).expect("destituição");
    assert_eq!(
        player_nemesis::get_current_nemesis(&conn)
            .expect("leitura")
            .as_deref(),
        Some("D002")
    );

    let linhas: i64 = conn
        .query_row("SELECT COUNT(*) FROM player_nemesis", [], |r| r.get(0))
        .expect("contagem");
    assert_eq!(
        linhas, 1,
        "a tabela é um singleton por construção (CHECK id = 1)"
    );
}

#[test]
fn limpar_o_nemesis_deixa_o_trono_vago_sem_apagar_a_linha() {
    let conn = banco();
    player_nemesis::set_current_nemesis(&conn, Some("D001")).expect("reinante");

    player_nemesis::set_current_nemesis(&conn, None).expect("limpar");

    assert!(player_nemesis::get_current_nemesis(&conn)
        .expect("leitura")
        .is_none());
}
