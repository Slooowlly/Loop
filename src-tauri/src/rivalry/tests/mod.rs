//! Suíte de testes de rivalidade (extraída de `rivalry/mod.rs`).
//!
//! Continua sendo o mesmo módulo `tests` de antes: `use super::*` enxerga o
//! módulo `rivalry` inteiro, incluindo os itens privados.

use rusqlite::Connection;

use super::*;
use crate::db::migrations;
use crate::db::queries::drivers::insert_driver;
use crate::db::queries::news::get_news_by_type;
use crate::db::queries::seasons::insert_season;
use crate::models::driver::Driver;
use crate::models::season::Season;
use crate::news::NewsType;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    migrations::run_all(&conn).unwrap();
    insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).unwrap();
    for (id, nome) in [
        ("P001", "Piloto1"),
        ("P002", "Piloto2"),
        ("P003", "Piloto3"),
        ("P020", "Piloto20"),
    ] {
        let mut d = Driver::create_player(id.to_string(), nome.to_string(), "BR".to_string(), 25);
        d.is_jogador = false;
        insert_driver(&conn, &d).unwrap();
    }
    conn
}

fn event(a: &str, b: &str, tipo: RivalryType, h: f64, r: f64) -> RivalryEvent {
    RivalryEvent {
        piloto_a: a.to_string(),
        piloto_b: b.to_string(),
        tipo,
        historical_delta: h,
        recent_delta: r,
        temporada: 1,
    }
}

// ── Passos 1-5 (regressão) ────────────────────────────────────────────────

#[test]
fn cria_rivalidade_nova() {
    let conn = setup_db();
    // h=10, r=20 → perceived = 0.4*10 + 0.6*20 = 16.0
    let applied = apply_rivalry_event(
        &conn,
        &event("P020", "P003", RivalryType::Colisao, 10.0, 20.0),
    )
    .unwrap();
    assert!((applied.new_perceived - 16.0).abs() < 1e-9);
    assert!(applied.old_perceived.abs() < 1e-9);

    let summaries = get_pilot_rivalries(&conn, "P003").unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].rival_id, "P020");
}

#[test]
fn reforco_acumula_nos_dois_eixos() {
    let conn = setup_db();
    // 1ª aplicação: h=10, r=20
    apply_rivalry_event(
        &conn,
        &event("P001", "P002", RivalryType::Campeonato, 10.0, 20.0),
    )
    .unwrap();
    // 2ª aplicação: h=10, r=20 → acumulado h=20, r=40
    // perceived = 0.4*20 + 0.6*40 = 8 + 24 = 32
    let applied = apply_rivalry_event(
        &conn,
        &event("P001", "P002", RivalryType::Campeonato, 10.0, 20.0),
    )
    .unwrap();
    assert!((applied.new_perceived - 32.0).abs() < 1e-9);
}

#[test]
fn clamp_nao_passa_de_100() {
    let conn = setup_db();
    apply_rivalry_event(
        &conn,
        &event("P001", "P002", RivalryType::Pista, 70.0, 70.0),
    )
    .unwrap();
    // h=70, r=70 → perceived=70; depois h=100(clamped), r=100 → perceived=100
    let applied = apply_rivalry_event(
        &conn,
        &event("P001", "P002", RivalryType::Pista, 70.0, 70.0),
    )
    .unwrap();
    assert!((applied.new_perceived - 100.0).abs() < 1e-9);
}

#[test]
fn tipo_original_preservado_no_reforco() {
    let conn = setup_db();
    apply_rivalry_event(
        &conn,
        &event("P001", "P002", RivalryType::Campeonato, 10.0, 10.0),
    )
    .unwrap();
    apply_rivalry_event(
        &conn,
        &event("P001", "P002", RivalryType::Colisao, 10.0, 10.0),
    )
    .unwrap();

    let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
    assert_eq!(summaries[0].tipo, RivalryType::Campeonato);
}

#[test]
fn mesmo_piloto_ignorado() {
    let conn = setup_db();
    apply_rivalry_event(
        &conn,
        &RivalryEvent {
            piloto_a: "P001".to_string(),
            piloto_b: "P001".to_string(),
            tipo: RivalryType::Pista,
            historical_delta: 50.0,
            recent_delta: 50.0,
            temporada: 1,
        },
    )
    .unwrap();
    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

// ── Passo 9: Thresholds ───────────────────────────────────────────────────

#[test]
fn intensity_level_faixas_corretas() {
    assert_eq!(intensity_level(0.0), RivalryIntensityLevel::AtritoLeve);
    assert_eq!(intensity_level(19.9), RivalryIntensityLevel::AtritoLeve);
    assert_eq!(intensity_level(20.0), RivalryIntensityLevel::Inicial);
    assert_eq!(intensity_level(39.9), RivalryIntensityLevel::Inicial);
    assert_eq!(intensity_level(40.0), RivalryIntensityLevel::Clara);
    assert_eq!(intensity_level(60.0), RivalryIntensityLevel::Forte);
    assert_eq!(intensity_level(80.0), RivalryIntensityLevel::Intensa);
    assert_eq!(intensity_level(100.0), RivalryIntensityLevel::Intensa);
}

#[test]
fn crossed_threshold_detecta_threshold_correto() {
    assert_eq!(
        crossed_threshold(15.0, 25.0),
        Some(RivalryIntensityLevel::Inicial)
    );
    assert_eq!(
        crossed_threshold(35.0, 45.0),
        Some(RivalryIntensityLevel::Clara)
    );
    // Salta dois thresholds — retorna o mais alto
    assert_eq!(
        crossed_threshold(15.0, 65.0),
        Some(RivalryIntensityLevel::Forte)
    );
    // Sem cruzamento (já na faixa)
    assert_eq!(crossed_threshold(25.0, 35.0), None);
    // Decaimento: sem cruzamento
    assert_eq!(crossed_threshold(50.0, 30.0), None);
}

// ── Passo 6: Hierarquia ───────────────────────────────────────────────────

#[test]
fn hierarchy_rivalry_crise_cria_evento() {
    let conn = setup_db();
    process_hierarchy_rivalry(
        &conn, "P001", "P002", "tensao", "crise", false, "gt3", "T001", 5, 1,
    )
    .unwrap();

    let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
    assert_eq!(summaries.len(), 1);
    // h=5, r=14 → perceived = 0.4*5 + 0.6*14 = 2 + 8.4 = 10.4
    assert!((summaries[0].perceived_intensity - 10.4).abs() < 1e-9);
}

#[test]
fn hierarchy_rivalry_inversao_maior_delta() {
    let conn = setup_db();
    process_hierarchy_rivalry(
        &conn,
        "P001",
        "P002",
        "crise",
        "reavaliacao",
        true,
        "gt3",
        "T001",
        5,
        1,
    )
    .unwrap();

    let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
    // h=8, r=18 → perceived = 0.4*8 + 0.6*18 = 3.2 + 10.8 = 14.0
    assert!((summaries[0].perceived_intensity - 14.0).abs() < 1e-9);
}

#[test]
fn hierarchy_rivalry_estado_estavel_nao_gera_evento() {
    let conn = setup_db();
    process_hierarchy_rivalry(
        &conn,
        "P001",
        "P002",
        "estavel",
        "competitivo",
        false,
        "gt3",
        "T001",
        5,
        1,
    )
    .unwrap();
    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

#[test]
fn hierarchy_rivalry_crise_persistente_nao_spam() {
    let conn = setup_db();
    process_hierarchy_rivalry(
        &conn, "P001", "P002", "crise", "crise", false, "gt3", "T001", 5, 1,
    )
    .unwrap();
    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

// ── Passo 7: Campeonato ───────────────────────────────────────────────────

#[test]
fn championship_rivalry_ultimas_rodadas_gap_pequeno() {
    let conn = setup_db();
    conn.execute(
        "UPDATE drivers SET categoria_atual = 'gt3', temp_pontos = 50.0 WHERE id = 'P001'",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE drivers SET categoria_atual = 'gt3', temp_pontos = 45.0 WHERE id = 'P002'",
        [],
    )
    .unwrap();

    process_championship_rivalry(&conn, "gt3", 8, 10, 1).unwrap();

    let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
    assert_eq!(summaries.len(), 1);
    // h=4, r=10 → perceived = 0.4*4 + 0.6*10 = 1.6 + 6.0 = 7.6
    assert!((summaries[0].perceived_intensity - 7.6).abs() < 1e-9);
}

#[test]
fn championship_rivalry_muito_cedo_nao_gera() {
    let conn = setup_db();
    conn.execute(
        "UPDATE drivers SET temp_pontos = 50.0 WHERE id = 'P001'",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE drivers SET temp_pontos = 45.0 WHERE id = 'P002'",
        [],
    )
    .unwrap();

    process_championship_rivalry(&conn, "gt3", 3, 10, 1).unwrap();
    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

#[test]
fn championship_rivalry_gap_grande_nao_gera() {
    let conn = setup_db();
    conn.execute(
        "UPDATE drivers SET categoria_atual = 'gt3', temp_pontos = 100.0 WHERE id = 'P001'",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE drivers SET categoria_atual = 'gt3', temp_pontos = 20.0  WHERE id = 'P002'",
        [],
    )
    .unwrap();

    process_championship_rivalry(&conn, "gt3", 9, 10, 1).unwrap();
    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

#[test]
fn championship_rivalry_limita_ao_top3() {
    let conn = setup_db();
    for (id, pontos) in [
        ("P001", 60.0),
        ("P002", 55.0),
        ("P003", 50.0),
        ("P020", 49.0),
    ] {
        conn.execute(
            "UPDATE drivers SET categoria_atual = 'gt3', temp_pontos = ?2 WHERE id = ?1",
            rusqlite::params![id, pontos],
        )
        .unwrap();
    }

    process_championship_rivalry(&conn, "gt3", 9, 10, 1).unwrap();

    assert!(
        get_pilot_rivalries(&conn, "P020").unwrap().is_empty(),
        "o 4o colocado nao deve entrar na regra de rivalidade de campeonato"
    );
}

// ── Passo 14: Decaimento ──────────────────────────────────────────────────

#[test]
fn decay_rivalidade_ativa_esfria_recente() {
    let conn = setup_db();
    // Criar rivalidade na temporada 1
    apply_rivalry_event(
        &conn,
        &event("P001", "P002", RivalryType::Campeonato, 20.0, 40.0),
    )
    .unwrap();

    // Decaimento de fim da temporada 1 (rivalidade foi ativa nesta temporada)
    apply_season_end_rivalry_decay(&conn, 1).unwrap();

    let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
    assert_eq!(summaries.len(), 1);
    // h permanece 20, r = 40 * 0.5 = 20
    // perceived = 0.4*20 + 0.6*20 = 8 + 12 = 20.0
    assert!((summaries[0].historical_intensity - 20.0).abs() < 1e-9);
    assert!((summaries[0].recent_activity - 20.0).abs() < 1e-9);
}

#[test]
fn decay_rivalidade_inativa_decai_nos_dois_eixos() {
    let conn = setup_db();
    // Criar rivalidade na temporada 1
    apply_rivalry_event(
        &conn,
        &event("P001", "P002", RivalryType::Campeonato, 20.0, 40.0),
    )
    .unwrap();

    // Decaimento de fim da temporada 2 (rivalidade foi criada em t1, agora é t2)
    apply_season_end_rivalry_decay(&conn, 2).unwrap();

    let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
    assert_eq!(summaries.len(), 1);
    // h = 20 * 0.85 = 17.0, r = 40 * 0.2 = 8.0
    assert!((summaries[0].historical_intensity - 17.0).abs() < 1e-9);
    assert!((summaries[0].recent_activity - 8.0).abs() < 1e-9);
}

#[test]
fn decay_rivalidade_extinta_e_removida() {
    let conn = setup_db();
    // Criar rivalidade fraca (h=3, r=5) e simular que está inativa há tempos
    apply_rivalry_event(&conn, &event("P001", "P002", RivalryType::Pista, 3.0, 5.0)).unwrap();

    // Após decaimento inativo: h = 3*0.85 = 2.55, r = 5*0.2 = 1.0
    // lifecycle: perceived = 0.4*2.55 + 0.6*1.0 = 1.02 + 0.6 = 1.62 < 5; h=2.55 < 10 → Extinta
    apply_season_end_rivalry_decay(&conn, 5).unwrap();

    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

#[test]
fn hierarchy_rivalry_crossing_threshold_persists_news() {
    let conn = setup_db();
    apply_rivalry_event(
        &conn,
        &event("P001", "P002", RivalryType::Companheiros, 15.0, 20.0),
    )
    .unwrap();

    process_hierarchy_rivalry(
        &conn, "P001", "P002", "tensao", "crise", false, "gt3", "T001", 5, 1,
    )
    .unwrap();

    let news = get_news_by_type(&conn, &NewsType::Rivalidade, 10).unwrap();
    assert_eq!(news.len(), 1);
    assert_eq!(news[0].driver_id.as_deref(), Some("P001"));
    assert_eq!(news[0].driver_id_secondary.as_deref(), Some("P002"));
    assert_eq!(news[0].team_id.as_deref(), Some("T001"));
}

#[test]
fn rivalries_table_rejects_duplicate_pair() {
    let conn = setup_db();
    conn.execute(
        "INSERT INTO rivalries
             (id, piloto1_id, piloto2_id, intensidade, historical_intensity,
              recent_activity, tipo, criado_em, ultima_atualizacao, temporada_update)
         VALUES ('R001', 'P001', 'P002', 10.0, 10.0, 10.0, 'Campeonato', '1', '1', 1)",
        [],
    )
    .unwrap();

    let duplicate = conn.execute(
        "INSERT INTO rivalries
             (id, piloto1_id, piloto2_id, intensidade, historical_intensity,
              recent_activity, tipo, criado_em, ultima_atualizacao, temporada_update)
         VALUES ('R002', 'P001', 'P002', 20.0, 20.0, 20.0, 'Colisao', '2', '2', 1)",
        [],
    );

    assert!(duplicate.is_err(), "par duplicado nao deve ser permitido");
}
