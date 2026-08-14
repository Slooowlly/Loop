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
    archive_row_na_temporada(conn, team_id, categoria, 1, pos, pontos);
}

fn archive_row_na_temporada(
    conn: &Connection,
    team_id: &str,
    categoria: &str,
    temporada: i32,
    pos: i32,
    pontos: f64,
) {
    conn.execute(
        "INSERT INTO team_season_archive
             (team_id, season_number, ano, categoria, posicao_campeonato, pontos)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![team_id, temporada, 2025 + temporada, categoria, pos, pontos],
    )
    .unwrap();
}

/// O pedaço da virada de temporada que este arquivo guarda, na ordem em que
/// `evolution::pipeline::orquestracao` o roda: DECAIMENTO primeiro, veredito do
/// campeonato depois.
fn fecha_temporada(conn: &Connection, temporada: i32) {
    apply_season_end_team_rivalry_decay(conn, temporada).unwrap();
    process_constructor_battle_rivalry(conn, temporada).unwrap();
}

/// Pódio de três em `gt3` na temporada dada: T001 (1º) e T002 (2º) decidem o título;
/// T003 (3º) entra por top-3 e forma com T002 um par NÃO decisor.
fn arquiva_podio(conn: &Connection, temporada: i32) {
    archive_row_na_temporada(conn, "T001", "gt3", temporada, 1, 100.0);
    archive_row_na_temporada(conn, "T002", "gt3", temporada, 2, 95.0);
    archive_row_na_temporada(conn, "T003", "gt3", temporada, 3, 90.0);
}

fn par(conn: &Connection, a: &str, b: &str) -> crate::rivalry::team::TeamRivalrySummary {
    get_team_rivalries(conn, a)
        .unwrap()
        .into_iter()
        .find(|r| r.rival_id == b)
        .unwrap_or_else(|| panic!("rivalidade {a} x {b} não existe"))
}

/// **O par não decisor tem de sobreviver ao próprio nascimento e acumular histórico.**
///
/// Ele nasce em (4, 10). Com o veredito de campeonato rodando ANTES do decaimento anual,
/// o decaimento da mesma virada o levava a (4, 5) — percebida 4,4, abaixo do limiar de
/// extinção — e a linha era APAGADA na mesma transação em que foi criada. Só o par que
/// decidiu o título, que nasce em (6, 15), sobrevivia ao próprio tique. Na prática a
/// espinha dorsal do sistema entregava um clássico por categoria e mais nada.
#[test]
fn par_nao_decisor_sobrevive_a_virada_e_acumula_historico() {
    let conn = setup_db();
    for id in ["T001", "T002", "T003"] {
        insert_test_team(&conn, id, "gt3");
    }

    arquiva_podio(&conn, 1);
    fecha_temporada(&conn, 1);

    // Temporada 1: nasce em (4, 10) e continua de pé no fim da virada.
    let t2_t3 = par(&conn, "T002", "T003");
    assert!((t2_t3.historical_intensity - 4.0).abs() < 1e-9);
    assert!((t2_t3.recent_activity - 10.0).abs() < 1e-9);

    // Temporada 2, mesma briga: o decaimento esfria o recente pela metade (5) e o
    // veredito soma (4, 10) por cima → (8, 15). O histórico ACUMULA.
    arquiva_podio(&conn, 2);
    fecha_temporada(&conn, 2);

    let t2_t3 = par(&conn, "T002", "T003");
    assert!(
        (t2_t3.historical_intensity - 8.0).abs() < 1e-9,
        "histórico devia acumular para 8, foi {}",
        t2_t3.historical_intensity
    );
    assert!(
        (t2_t3.recent_activity - 15.0).abs() < 1e-9,
        "recente devia ser 10*0.5 + 10 = 15, foi {}",
        t2_t3.recent_activity
    );
}

/// O par que decidiu o título nasce em (6, 15) e sempre sobreviveu — o que este teste
/// guarda é que a inversão da ordem não mexeu nele.
#[test]
fn par_decisor_do_titulo_continua_funcionando() {
    let conn = setup_db();
    for id in ["T001", "T002", "T003"] {
        insert_test_team(&conn, id, "gt3");
    }

    arquiva_podio(&conn, 1);
    fecha_temporada(&conn, 1);

    let decisor = par(&conn, "T001", "T002");
    assert_eq!(decisor.tipo, TeamRivalryType::Campeonato);
    assert!((decisor.historical_intensity - 6.0).abs() < 1e-9);
    assert!((decisor.recent_activity - 15.0).abs() < 1e-9);

    arquiva_podio(&conn, 2);
    fecha_temporada(&conn, 2);

    // (6, 15) → decaimento ativo (6, 7.5) → veredito (12, 22.5).
    let decisor = par(&conn, "T001", "T002");
    assert!((decisor.historical_intensity - 12.0).abs() < 1e-9);
    assert!(
        (decisor.recent_activity - 22.5).abs() < 1e-9,
        "foi {}",
        decisor.recent_activity
    );
}

/// **Uma virada, um decaimento.** A rivalidade que já existia é esfriada uma única vez,
/// e o reforço da temporada entra por cima do valor decaído — nunca é decaído junto.
#[test]
fn rivalidade_preexistente_decai_uma_vez_por_virada() {
    let conn = setup_db();
    for id in ["T001", "T002", "T003", "T005", "T006"] {
        insert_test_team(&conn, id, "gt3");
    }
    // Preexistente que VAI brigar de novo (T002 x T003) e preexistente que NÃO briga.
    apply_team_rivalry_event(
        &conn,
        &event("T002", "T003", TeamRivalryType::Campeonato, 20.0, 40.0),
    )
    .unwrap();
    apply_team_rivalry_event(
        &conn,
        &event("T005", "T006", TeamRivalryType::Pista, 20.0, 40.0),
    )
    .unwrap();

    arquiva_podio(&conn, 2);
    fecha_temporada(&conn, 2);

    // Briga de novo: decai UMA vez pelo ramo ativo (20, 20) e recebe (4, 10) por cima.
    // Dois decaimentos deixariam o recente em 15 (ou o reforço abaixo de 10).
    let brigou = par(&conn, "T002", "T003");
    assert!(
        (brigou.historical_intensity - 24.0).abs() < 1e-9,
        "foi {}",
        brigou.historical_intensity
    );
    assert!(
        (brigou.recent_activity - 30.0).abs() < 1e-9,
        "recente devia ser 40*0.5 + 10 = 30, foi {}",
        brigou.recent_activity
    );

    // Não brigou: ramo inativo, uma vez só → (17, 8).
    let parada = par(&conn, "T005", "T006");
    assert!((parada.historical_intensity - 17.0).abs() < 1e-9);
    assert!((parada.recent_activity - 8.0).abs() < 1e-9);
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

/// `team1_id` gravado na linha, para conferir a ORDENAÇÃO do par e não só a
/// existência dele.
fn team1_gravado(conn: &Connection) -> String {
    conn.query_row("SELECT team1_id FROM team_rivalries", [], |row| row.get(0))
        .unwrap()
}

/// **O par de EQUIPES entra pelo normalizador de equipe.** As duas fontes que
/// pegam carona no fato de piloto — Pista e Herdada — montavam a chave com
/// `normalize_pair`, o normalizador de PILOTO, e transportavam ids de time em
/// campos chamados `piloto1_id`/`piloto2_id`. Funcionava por coincidência de
/// formato e a dependência vivia só num comentário.
///
/// O que estes dois testes travam é o efeito observável: menor id primeiro, e a
/// ordem em que os times chegam não abre uma segunda linha para a mesma briga.
#[test]
fn pista_normaliza_o_par_de_times_em_qualquer_ordem() {
    use crate::simulation::incidents::{IncidentResult, IncidentSeverity, IncidentType};
    let conn = setup_db();
    let mut team_by_driver = HashMap::new();
    team_by_driver.insert("P1".to_string(), "T020".to_string());
    team_by_driver.insert("P2".to_string(), "T003".to_string());

    let colisao = |pilot: &str, linked: &str| IncidentResult {
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

    // Chega com o maior primeiro (T020 antes de T003).
    process_team_collisions_rivalry(&conn, &[colisao("P1", "P2")], &team_by_driver, "gt3", 5, 1)
        .unwrap();
    assert_eq!(team1_gravado(&conn), "T003", "o menor id fica em team1_id");
    let primeira = par(&conn, "T003", "T020").perceived_intensity;

    // Mesma briga, ordem invertida: reforça a linha que já existe.
    process_team_collisions_rivalry(&conn, &[colisao("P2", "P1")], &team_by_driver, "gt3", 6, 1)
        .unwrap();
    assert_eq!(
        get_team_rivalries(&conn, "T003").unwrap().len(),
        1,
        "a ordem invertida não pode abrir uma segunda linha para o mesmo par"
    );
    assert!(
        par(&conn, "T003", "T020").perceived_intensity > primeira,
        "a segunda passagem tem que reforçar a rivalidade, não duplicá-la"
    );
}

#[test]
fn herdada_normaliza_o_par_de_times_em_qualquer_ordem() {
    use crate::models::rivalry::RivalryType;
    use crate::rivalry::{apply_rivalry_event, RivalryEvent};
    let conn = setup_db();
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

    let mapa = |a: &str, b: &str| {
        let mut m = HashMap::new();
        m.insert("P1".to_string(), a.to_string());
        m.insert("P2".to_string(), b.to_string());
        m
    };

    process_driver_rivalry_bleed(&conn, &mapa("T020", "T003"), "gt3", 5, 1).unwrap();
    assert_eq!(team1_gravado(&conn), "T003", "o menor id fica em team1_id");
    let primeira = par(&conn, "T003", "T020").perceived_intensity;

    // Os mesmos dois times, trocados de lado no mapa: mesma linha.
    process_driver_rivalry_bleed(&conn, &mapa("T003", "T020"), "gt3", 6, 1).unwrap();
    assert_eq!(
        get_team_rivalries(&conn, "T003").unwrap().len(),
        1,
        "a ordem invertida não pode abrir uma segunda linha para o mesmo par"
    );
    assert!(par(&conn, "T003", "T020").perceived_intensity > primeira);
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
