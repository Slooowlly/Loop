use std::collections::HashMap;

use rusqlite::Connection;

use super::*;
use crate::db::migrations;
use crate::models::driver::Driver;
use crate::models::team_rivalry::TeamRivalryType;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    // Estes testes de unidade usam ids sintéticos de time (T001…) sem popular a
    // tabela `teams`; a FK `team_rivalries → teams(id)` é irrelevante aqui (a produção
    // sempre semeia com ids reais). Desliga a checagem para o motor puro.
    conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
    migrations::run_all(&conn).unwrap();
    conn
}

fn event(a: &str, b: &str, tipo: TeamRivalryType, h: f64, r: f64) -> TeamRivalryEvent {
    TeamRivalryEvent {
        team_a: a.to_string(),
        team_b: b.to_string(),
        tipo,
        historical_delta: h,
        recent_delta: r,
        temporada: 1,
    }
}

#[test]
fn cria_rivalidade_nova_normalizando_o_par() {
    let conn = setup_db();
    // Passa fora de ordem de propósito — deve normalizar (T003 < T020).
    let applied = apply_team_rivalry_event(
        &conn,
        &event("T020", "T003", TeamRivalryType::Campeonato, 10.0, 20.0),
    )
    .unwrap();
    // 0.6*10 + 0.4*20 = 14.0
    assert!((applied.new_perceived - 14.0).abs() < 1e-9);
    assert!(applied.old_perceived.abs() < 1e-9);

    let summ = get_team_rivalries(&conn, "T003").unwrap();
    assert_eq!(summ.len(), 1);
    assert_eq!(summ[0].rival_id, "T020");
    assert_eq!(summ[0].tipo, TeamRivalryType::Campeonato);
}

#[test]
fn reforco_acumula_nos_dois_eixos_e_preserva_tipo() {
    let conn = setup_db();
    apply_team_rivalry_event(
        &conn,
        &event("T001", "T002", TeamRivalryType::Mercado, 10.0, 20.0),
    )
    .unwrap();
    // Reforço com outro tipo — o tipo ORIGINAL (Mercado) deve permanecer.
    let applied = apply_team_rivalry_event(
        &conn,
        &event("T001", "T002", TeamRivalryType::Pista, 10.0, 20.0),
    )
    .unwrap();
    // acumulado h=20, r=40 → perceived = 0.6*20 + 0.4*40 = 28
    assert!((applied.new_perceived - 28.0).abs() < 1e-9);
    assert!((applied.old_perceived - 14.0).abs() < 1e-9);

    let summ = get_team_rivalries(&conn, "T001").unwrap();
    assert_eq!(summ[0].tipo, TeamRivalryType::Mercado);
}

#[test]
fn clamp_nao_passa_de_100() {
    let conn = setup_db();
    apply_team_rivalry_event(
        &conn,
        &event("T001", "T002", TeamRivalryType::Pista, 70.0, 70.0),
    )
    .unwrap();
    let applied = apply_team_rivalry_event(
        &conn,
        &event("T001", "T002", TeamRivalryType::Pista, 70.0, 70.0),
    )
    .unwrap();
    assert!((applied.new_perceived - 100.0).abs() < 1e-9);
}

#[test]
fn mesmo_time_ignorado() {
    let conn = setup_db();
    let applied = apply_team_rivalry_event(
        &conn,
        &event("T001", "T001", TeamRivalryType::Campeonato, 50.0, 50.0),
    )
    .unwrap();
    assert!(applied.rivalry_id.is_empty());
    assert!(get_team_rivalries(&conn, "T001").unwrap().is_empty());
}

#[test]
fn decay_ativa_esfria_recente_mantem_historico() {
    let conn = setup_db();
    apply_team_rivalry_event(
        &conn,
        &event("T001", "T002", TeamRivalryType::Campeonato, 20.0, 40.0),
    )
    .unwrap();
    apply_season_end_team_rivalry_decay(&conn, 1).unwrap();

    let summ = get_team_rivalries(&conn, "T001").unwrap();
    assert_eq!(summ.len(), 1);
    // h intacto (20), r = 40*0.5 = 20
    assert!((summ[0].historical_intensity - 20.0).abs() < 1e-9);
    assert!((summ[0].recent_activity - 20.0).abs() < 1e-9);
}

#[test]
fn decay_inativa_decai_nos_dois_eixos() {
    let conn = setup_db();
    apply_team_rivalry_event(
        &conn,
        &event("T001", "T002", TeamRivalryType::Campeonato, 20.0, 40.0),
    )
    .unwrap();
    // Decaimento de uma temporada POSTERIOR à do reforço → inativa.
    apply_season_end_team_rivalry_decay(&conn, 2).unwrap();

    let summ = get_team_rivalries(&conn, "T001").unwrap();
    // h = 20*0.85 = 17, r = 40*0.2 = 8
    assert!((summ[0].historical_intensity - 17.0).abs() < 1e-9);
    assert!((summ[0].recent_activity - 8.0).abs() < 1e-9);
}

#[test]
fn decay_extinta_e_removida() {
    let conn = setup_db();
    // Rivalidade fraca; após decaimento inativo cai abaixo do limiar de extinção.
    apply_team_rivalry_event(
        &conn,
        &event("T001", "T002", TeamRivalryType::Pista, 3.0, 5.0),
    )
    .unwrap();
    apply_season_end_team_rivalry_decay(&conn, 5).unwrap();
    assert!(get_team_rivalries(&conn, "T001").unwrap().is_empty());
}

#[test]
fn ids_sao_sequenciais_com_prefixo_trv() {
    let conn = setup_db();
    apply_team_rivalry_event(
        &conn,
        &event("T001", "T002", TeamRivalryType::Campeonato, 5.0, 12.0),
    )
    .unwrap();
    apply_team_rivalry_event(
        &conn,
        &event("T003", "T004", TeamRivalryType::Campeonato, 5.0, 12.0),
    )
    .unwrap();
    let first = get_team_rivalries(&conn, "T001").unwrap()[0]
        .rivalry_id
        .clone();
    let second = get_team_rivalries(&conn, "T003").unwrap()[0]
        .rivalry_id
        .clone();
    assert!(first.starts_with("TRV"), "id foi {first}");
    assert!(second.starts_with("TRV"), "id foi {second}");
    assert_ne!(first, second);
}

// ── Fontes/consequências (Fase 2+) ────────────────────────────────────────

fn insert_test_team(conn: &Connection, id: &str, categoria: &str) -> crate::models::team::Team {
    use crate::constants::teams::get_team_templates;
    use crate::db::queries::teams::insert_team;
    use rand::{rngs::StdRng, SeedableRng};
    let template = get_team_templates(categoria)[0];
    let mut rng = StdRng::seed_from_u64(9);
    let mut team = crate::models::team::Team::from_template_with_rng(
        template,
        categoria,
        id.to_string(),
        2026,
        &mut rng,
    );
    team.morale = 1.0;
    team.hierarquia_n1_id = None;
    insert_team(conn, &team).unwrap();
    team
}

fn insert_test_driver(conn: &Connection, id: &str, skill: f64, midia: f64) -> Driver {
    use crate::db::queries::drivers::insert_driver;
    let mut d = Driver::create_player(id.to_string(), format!("Piloto {id}"), "BR".to_string(), 25);
    d.is_jogador = false;
    d.atributos.skill = skill;
    d.atributos.midia = midia;
    insert_driver(conn, &d).unwrap();
    d
}

fn archive_row(conn: &Connection, team_id: &str, categoria: &str, pos: i32, pontos: f64) {
    conn.execute(
        "INSERT INTO team_season_archive
             (team_id, season_number, ano, categoria, posicao_campeonato, pontos)
         VALUES (?1, 1, 2026, ?2, ?3, ?4)",
        rusqlite::params![team_id, categoria, pos, pontos],
    )
    .unwrap();
}

#[test]
fn seed_transfer_astro_assediado_gera_rancor_maximo() {
    let conn = setup_db();
    insert_test_team(&conn, "T001", "gt3");
    insert_test_team(&conn, "T002", "gt3");
    let star = insert_test_driver(&conn, "P100", 85.0, 40.0);

    seed_team_rivalry_from_transfer(&conn, "T001", "T002", &star, true, 1).unwrap();

    let summ = get_team_rivalries(&conn, "T001").unwrap();
    assert_eq!(summ.len(), 1);
    assert_eq!(summ[0].tipo, TeamRivalryType::Mercado);
    // Astro + poaching: h=8, r=16 → perceived = 0.6*8 + 0.4*16 = 11.2.
    assert!((summ[0].perceived_intensity - 11.2).abs() < 1e-9);
}

#[test]
fn seed_transfer_entre_divisoes_pesa_metade() {
    let conn = setup_db();
    insert_test_team(&conn, "T001", "gt3");
    insert_test_team(&conn, "T002", "gt4");
    let star = insert_test_driver(&conn, "P100", 85.0, 40.0);

    seed_team_rivalry_from_transfer(&conn, "T001", "T002", &star, true, 1).unwrap();

    let summ = get_team_rivalries(&conn, "T001").unwrap();
    // Metade de (8,16) = (4,8) → perceived = 0.6*4 + 0.4*8 = 5.6.
    assert!((summ[0].perceived_intensity - 5.6).abs() < 1e-9);
}

#[test]
fn seed_transfer_reserva_pesa_pouco() {
    let conn = setup_db();
    insert_test_team(&conn, "T001", "gt3");
    insert_test_team(&conn, "T002", "gt3");
    let reserva = insert_test_driver(&conn, "P100", 40.0, 20.0);

    seed_team_rivalry_from_transfer(&conn, "T001", "T002", &reserva, false, 1).unwrap();

    let summ = get_team_rivalries(&conn, "T001").unwrap();
    // Reserva: h=1, r=4 → perceived = 0.6*1 + 0.4*4 = 2.2.
    assert!((summ[0].perceived_intensity - 2.2).abs() < 1e-9);
}

#[test]
fn constructor_battle_top3_e_gap_apertado() {
    let conn = setup_db();
    for id in ["T001", "T002", "T003", "T004"] {
        insert_test_team(&conn, id, "gt3");
    }
    archive_row(&conn, "T001", "gt3", 1, 100.0);
    archive_row(&conn, "T002", "gt3", 2, 95.0);
    archive_row(&conn, "T003", "gt3", 3, 50.0);
    archive_row(&conn, "T004", "gt3", 4, 10.0);

    process_constructor_battle_rivalry(&conn, 1).unwrap();

    // Top-3 entre si (T001,T002,T003) formam pares; T004 (4º, gap grande) fica de fora.
    assert_eq!(get_team_rivalries(&conn, "T001").unwrap().len(), 2);
    assert!(get_team_rivalries(&conn, "T004").unwrap().is_empty());
    // Par 1º×2º decidiu o título → delta reforçado (6,15) → perceived = 9.6.
    let leader = get_team_rivalries(&conn, "T001").unwrap();
    let vs_t2 = leader.iter().find(|r| r.rival_id == "T002").unwrap();
    assert!(
        (vs_t2.perceived_intensity - 9.6).abs() < 1e-9,
        "foi {}",
        vs_t2.perceived_intensity
    );
}

#[test]
fn collisions_agrega_por_par_de_times_ignora_companheiro() {
    use crate::simulation::incidents::{IncidentResult, IncidentSeverity, IncidentType};
    let conn = setup_db();
    let mut team_by_driver = HashMap::new();
    team_by_driver.insert("P1".to_string(), "T001".to_string());
    team_by_driver.insert("P2".to_string(), "T002".to_string());
    team_by_driver.insert("P3".to_string(), "T001".to_string());

    let mk = |pilot: &str, linked: &str| IncidentResult {
        pilot_id: pilot.to_string(),
        incident_type: IncidentType::Collision,
        severity: IncidentSeverity::Major,
        segment: String::new(),
        positions_lost: 2,
        is_dnf: false,
        description: String::new(),
        linked_pilot_id: Some(linked.to_string()),
        is_two_car_incident: true,
        injury_risk_multiplier: 1.0,
        narrative_importance_hint: 0,
        catalog_id: None,
        damage_origin_segment: None,
    };
    // T001 x T002 (conta) + T001 interno P1×P3 (ignora — mesmo time).
    let incidents = vec![mk("P1", "P2"), mk("P1", "P3")];
    process_team_collisions_rivalry(&conn, &incidents, &team_by_driver, "gt3", 5, 1).unwrap();

    let summ = get_team_rivalries(&conn, "T001").unwrap();
    assert_eq!(summ.len(), 1);
    assert_eq!(summ[0].rival_id, "T002");
    assert_eq!(summ[0].tipo, TeamRivalryType::Pista);
    // Base (2,6) → perceived = 0.6*2 + 0.4*6 = 3.6.
    assert!((summ[0].perceived_intensity - 3.6).abs() < 1e-9);
}

#[test]
fn bleed_so_transborda_rivalidade_intensa_cross_time() {
    use crate::models::rivalry::RivalryType;
    use crate::rivalry::{apply_rivalry_event, RivalryEvent};
    let conn = setup_db();
    // Rivalidade de piloto INTENSA (percebida 60) entre P1(T001) e P2(T002).
    apply_rivalry_event(
        &conn,
        &RivalryEvent {
            piloto_a: "P1".to_string(),
            piloto_b: "P2".to_string(),
            tipo: RivalryType::Colisao,
            historical_delta: 60.0,
            recent_delta: 60.0,
            temporada: 1,
        },
    )
    .unwrap();
    // Rivalidade FRACA (percebida 20) entre P3(T001) e P4(T002) — não transborda.
    apply_rivalry_event(
        &conn,
        &RivalryEvent {
            piloto_a: "P3".to_string(),
            piloto_b: "P4".to_string(),
            tipo: RivalryType::Colisao,
            historical_delta: 20.0,
            recent_delta: 20.0,
            temporada: 1,
        },
    )
    .unwrap();

    let mut team_by_driver = HashMap::new();
    for (p, t) in [
        ("P1", "T001"),
        ("P2", "T002"),
        ("P3", "T001"),
        ("P4", "T002"),
    ] {
        team_by_driver.insert(p.to_string(), t.to_string());
    }
    process_driver_rivalry_bleed(&conn, &team_by_driver, "gt3", 5, 1).unwrap();

    let summ = get_team_rivalries(&conn, "T001").unwrap();
    assert_eq!(summ.len(), 1, "só a rivalidade intensa deve transbordar");
    assert_eq!(summ[0].tipo, TeamRivalryType::Herdada);
    // Trickle (1,3) → perceived = 0.6*1 + 0.4*3 = 1.8.
    assert!((summ[0].perceived_intensity - 1.8).abs() < 1e-9);
}

#[test]
fn derby_morale_vencer_o_rival_empurra_moral() {
    let conn = setup_db();
    let t1 = insert_test_team(&conn, "T001", "gt3");
    let t2 = insert_test_team(&conn, "T002", "gt3");
    assert!((t1.morale - 1.0).abs() < 1e-9 && (t2.morale - 1.0).abs() < 1e-9);
    // Rivalidade viva e quente (percebida 40).
    apply_team_rivalry_event(
        &conn,
        &event("T001", "T002", TeamRivalryType::Campeonato, 40.0, 40.0),
    )
    .unwrap();

    let mut best = HashMap::new();
    best.insert("T001".to_string(), 1); // T001 venceu o derby
    best.insert("T002".to_string(), 5);
    apply_derby_morale(&conn, &best).unwrap();

    let m1 = crate::db::queries::teams::get_team_by_id(&conn, "T001")
        .unwrap()
        .unwrap()
        .morale;
    let m2 = crate::db::queries::teams::get_team_by_id(&conn, "T002")
        .unwrap()
        .unwrap()
        .morale;
    assert!(m1 > 1.0, "vencedor sobe a moral, foi {m1}");
    assert!(m2 < 1.0, "perdedor desce a moral, foi {m2}");
    // Pulso sutil: delta = 0.015 * (0.5 + 0.4) = 0.0135.
    assert!((m1 - 1.0135).abs() < 1e-6, "foi {m1}");
}

#[test]
fn derby_morale_ignora_par_ausente_ou_fraco() {
    let conn = setup_db();
    insert_test_team(&conn, "T001", "gt3");
    insert_test_team(&conn, "T002", "gt3");
    // Rivalidade fraca (percebida abaixo do mínimo de derby).
    apply_team_rivalry_event(
        &conn,
        &event("T001", "T002", TeamRivalryType::Campeonato, 2.0, 2.0),
    )
    .unwrap();
    let mut best = HashMap::new();
    best.insert("T001".to_string(), 1);
    best.insert("T002".to_string(), 5);
    apply_derby_morale(&conn, &best).unwrap();

    let m1 = crate::db::queries::teams::get_team_by_id(&conn, "T001")
        .unwrap()
        .unwrap()
        .morale;
    assert!(
        (m1 - 1.0).abs() < 1e-9,
        "rivalidade fraca não move a moral, foi {m1}"
    );
}
