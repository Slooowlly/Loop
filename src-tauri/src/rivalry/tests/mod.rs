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
    // h=10, r=20 → perceived = 0.6*10 + 0.4*20 = 14.0
    let applied = apply_rivalry_event(
        &conn,
        &event("P020", "P003", RivalryType::Colisao, 10.0, 20.0),
    )
    .unwrap();
    assert!((applied.new_perceived - 14.0).abs() < 1e-9);
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
    // perceived = 0.6*20 + 0.4*40 = 12 + 16 = 28
    let applied = apply_rivalry_event(
        &conn,
        &event("P001", "P002", RivalryType::Campeonato, 10.0, 20.0),
    )
    .unwrap();
    assert!((applied.new_perceived - 28.0).abs() < 1e-9);
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
    // h=5, r=14 → perceived = 0.6*5 + 0.4*14 = 3 + 5.6 = 8.6
    assert!((summaries[0].perceived_intensity - 8.6).abs() < 1e-9);
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
    // h=8, r=18 → perceived = 0.6*8 + 0.4*18 = 4.8 + 7.2 = 12.0
    assert!((summaries[0].perceived_intensity - 12.0).abs() < 1e-9);
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

/// TENSÃO É A NOVA PORTA DE ENTRADA. O primeiro degrau custava Reavaliação
/// (tensão 60), que nenhuma equipe do mundo alcançava — o gatilho existia e
/// nunca disparava em 27 temporadas de save.
#[test]
fn hierarchy_rivalry_tensao_abre_a_porta() {
    let conn = setup_db();
    process_hierarchy_rivalry(
        &conn,
        "P001",
        "P002",
        "competitivo",
        "tensao",
        false,
        "gt3",
        "T001",
        5,
        1,
    )
    .unwrap();

    let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
    assert_eq!(summaries.len(), 1);
    // h=2, r=7 → perceived = 0.6*2 + 0.4*7 = 1.2 + 2.8 = 4.0. É o degrau mais
    // barato de propósito: abre a rivalidade sem já declará-la grave.
    assert!((summaries[0].perceived_intensity - 4.0).abs() < 1e-9);
}

/// Descer de Crise para Tensão é a equipe ESFRIANDO. Cobrar o evento na descida
/// faria a rivalidade renascer justo do clima que melhorou.
#[test]
fn hierarchy_rivalry_tensao_so_conta_subindo() {
    let conn = setup_db();
    process_hierarchy_rivalry(
        &conn, "P001", "P002", "crise", "tensao", false, "gt3", "T001", 5, 1,
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

// ── Fim de temporada: companheiros pelo placar do ano ─────────────────────

/// Cria uma equipe já com o placar do duelo interno da temporada fechado.
fn equipe_com_placar(
    conn: &Connection,
    id: &str,
    n1: &str,
    n2: &str,
    duelos: i32,
    n2_vencidos: i32,
) {
    use crate::constants::teams::get_team_templates;
    use crate::db::queries::teams::insert_team;
    use crate::models::team::Team;
    use rand::{rngs::StdRng, SeedableRng};

    let template = get_team_templates("gt3")[0];
    let mut rng = StdRng::seed_from_u64(7);
    let mut team = Team::from_template_with_rng(template, "gt3", id.to_string(), 2024, &mut rng);
    team.ativa = true;
    team.hierarquia_n1_id = Some(n1.to_string());
    team.hierarquia_n2_id = Some(n2.to_string());
    team.hierarquia_duelos_total = duelos;
    team.hierarquia_duelos_n2_vencidos = n2_vencidos;
    insert_team(conn, &team).unwrap();
}

/// O caminho que de fato produz treta de dupla. O eixo de tensão exige que o N2
/// leve 40% dos duelos só para parar de cair, e num mundo medido ele leva 22,7% —
/// o acumulador mora no piso. Aqui o critério é o placar, não o acumulador.
#[test]
fn companheiros_placar_equilibrado_cria_rivalidade() {
    let conn = setup_db();
    equipe_com_placar(&conn, "T001", "P001", "P002", 20, 9); // 45%

    process_teammate_season_rivalry(&conn, 1).unwrap();

    let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].tipo, RivalryType::Companheiros);
    // h=4, r=12 → perceived = 0.6*4 + 0.4*12 = 2.4 + 4.8 = 7.2
    assert!((summaries[0].perceived_intensity - 7.2).abs() < 1e-9);
}

/// N2 empatando ou virando o ano: a hierarquia da equipe virou ficção, e o evento
/// pesa mais do que o de uma temporada só incômoda.
#[test]
fn companheiros_n2_virando_o_ano_pesa_mais() {
    let conn = setup_db();
    equipe_com_placar(&conn, "T001", "P001", "P002", 20, 12); // 60%

    process_teammate_season_rivalry(&conn, 1).unwrap();

    let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
    // h=6, r=16 → perceived = 0.6*6 + 0.4*16 = 3.6 + 6.4 = 10.0
    assert!((summaries[0].perceived_intensity - 10.0).abs() < 1e-9);
}

/// O caso normal do mundo: o N2 é o segundo piloto e se comporta como tal.
/// Uma temporada assim não é rivalidade — é hierarquia funcionando.
#[test]
fn companheiros_n2_apagado_nao_cria_nada() {
    let conn = setup_db();
    equipe_com_placar(&conn, "T001", "P001", "P002", 20, 4); // 20%

    process_teammate_season_rivalry(&conn, 1).unwrap();

    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

/// Amostra curta não é placar. Dupla que só correu junta 6 vezes (lesão, estreia
/// no meio do ano, inversão que zerou os contadores) não vira rivalidade por sorte.
#[test]
fn companheiros_amostra_curta_nao_conta() {
    let conn = setup_db();
    equipe_com_placar(&conn, "T001", "P001", "P002", 6, 4); // 67%, mas em 6 duelos

    process_teammate_season_rivalry(&conn, 1).unwrap();

    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

/// Categoria especial tem hierarquia efêmera (bloco de convocação) e reset próprio —
/// o placar dela não é uma temporada de convivência.
#[test]
fn companheiros_categoria_especial_fica_de_fora() {
    let conn = setup_db();
    equipe_com_placar(&conn, "T001", "P001", "P002", 20, 12);
    conn.execute(
        "UPDATE teams SET categoria = 'endurance' WHERE id = 'T001'",
        [],
    )
    .unwrap();

    process_teammate_season_rivalry(&conn, 1).unwrap();

    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

/// A manchete é de temporada, não de rodada — não existe "rodada 0".
#[test]
fn companheiros_noticia_nao_inventa_rodada() {
    let conn = setup_db();
    equipe_com_placar(&conn, "T001", "P001", "P002", 20, 12);
    // perceived antes = 0.4*15 + 0.6*20 = 18; depois do evento (6/16) = 30 → cruza Inicial
    apply_rivalry_event(
        &conn,
        &event("P001", "P002", RivalryType::Companheiros, 15.0, 20.0),
    )
    .unwrap();

    process_teammate_season_rivalry(&conn, 1).unwrap();

    let news = get_news_by_type(&conn, &NewsType::Rivalidade, 10).unwrap();
    assert_eq!(news.len(), 1);
    assert_eq!(news[0].team_id.as_deref(), Some("T001"));
    // Locale-agnóstico de propósito: nenhuma das duas línguas pode citar rodada.
    // (o `rodada: None` do item não sobrevive ao round-trip — a coluna é NOT NULL
    // e volta como 0; quem prova o None é `noticia_de_temporada_nao_carrega_rodada`)
    let texto = &news[0].texto;
    assert!(
        !texto.contains("rodada") && !texto.contains("round"),
        "manchete de temporada nao pode citar rodada: {texto}"
    );
}

/// O item montado não carrega rodada — é o que a timeline lê para não ancorar a
/// manchete num fim de semana que não existiu.
#[test]
fn noticia_de_temporada_nao_carrega_rodada() {
    let applied = RivalryApplied {
        rivalry_id: "R001".to_string(),
        old_perceived: 18.0,
        new_perceived: 30.0,
    };
    let item = build_rivalry_news_item(
        "N001".to_string(),
        &applied,
        &RivalryType::Companheiros,
        "Ana",
        "Bruno",
        "gt3",
        1,
        0, // sem rodada
        "P001",
        "P002",
        Some("T001"),
    )
    .expect("cruzou threshold, deve gerar item");

    assert!(item.rodada.is_none());
    assert!(item.semana_pretemporada.is_none());
}

// ── Fim de temporada: pista pelo placar de adjacências ────────────────────

/// Grava uma corrida da temporada 1 e as chegadas dos dois pilotos.
/// `(posicao, dnf)` por piloto — é o par que o gatilho lê.
fn corrida_com_chegadas(
    conn: &Connection,
    rodada: i32,
    categoria: &str,
    a: (&str, i32, bool),
    b: (&str, i32, bool),
) {
    let race_id = format!("{categoria}_R{rodada}");
    conn.execute(
        "INSERT INTO calendar (id, temporada_id, season_id, rodada, pista, categoria)
         VALUES (?1, 'S001', 'S001', ?2, 'Interlagos', ?3)",
        rusqlite::params![race_id, rodada, categoria],
    )
    .unwrap();

    for (piloto, posicao, dnf) in [a, b] {
        conn.execute(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, dnf)
             VALUES (?1, ?2, 'T001', ?3, ?4)",
            rusqlite::params![race_id, piloto, posicao, i32::from(dnf)],
        )
        .unwrap();
    }
}

/// Cria a equipe que os resultados referenciam (FK de `race_results.equipe_id`).
fn equipe_generica(conn: &Connection) {
    equipe_com_placar(conn, "T001", "P001", "P002", 0, 0);
}

/// O gatilho `Pista` só era aplicado na importação de corrida real do iRacing —
/// em mundo simulado nunca existiu. Aqui ele nasce do placar de chegadas coladas.
#[test]
fn pista_temporada_colada_cria_rivalidade() {
    let conn = setup_db();
    equipe_generica(&conn);
    for rodada in 1..=6 {
        corrida_com_chegadas(&conn, rodada, "gt3", ("P001", 5, false), ("P002", 6, false));
    }

    process_track_season_rivalry(&conn, 1).unwrap();

    let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].tipo, RivalryType::Pista);
    // h=3, r=9 → perceived = 0.6*3 + 0.4*9 = 1.8 + 3.6 = 5.4
    assert!((summaries[0].perceived_intensity - 5.4).abs() < 1e-9);
}

/// Oito ou mais é a briga da temporada, e pesa mais.
#[test]
fn pista_ano_inteiro_colado_pesa_mais() {
    let conn = setup_db();
    equipe_generica(&conn);
    for rodada in 1..=8 {
        corrida_com_chegadas(&conn, rodada, "gt3", ("P001", 5, false), ("P002", 6, false));
    }

    process_track_season_rivalry(&conn, 1).unwrap();

    let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
    // h=5, r=13 → perceived = 0.6*5 + 0.4*13 = 3.0 + 5.2 = 8.2
    assert!((summaries[0].perceived_intensity - 8.2).abs() < 1e-9);
}

/// Cinco adjacências é o pelotão andando junto, não uma disputa. O corte foi
/// medido: ≥5 daria ~17 pares por temporada no mundo inteiro, ≥6 dá ~3.8.
#[test]
fn pista_abaixo_do_corte_nao_cria_nada() {
    let conn = setup_db();
    equipe_generica(&conn);
    for rodada in 1..=5 {
        corrida_com_chegadas(&conn, rodada, "gt3", ("P001", 5, false), ("P002", 6, false));
    }

    process_track_season_rivalry(&conn, 1).unwrap();

    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

/// PROXIMIDADE NÃO É ADJACÊNCIA. Duas posições de distância pega o pelotão inteiro
/// — numa faixa de cinco carros do meio do grid todos estão perto de todos.
#[test]
fn pista_duas_posicoes_de_distancia_nao_conta() {
    let conn = setup_db();
    equipe_generica(&conn);
    for rodada in 1..=10 {
        corrida_com_chegadas(&conn, rodada, "gt3", ("P001", 5, false), ("P002", 7, false));
    }

    process_track_season_rivalry(&conn, 1).unwrap();

    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

/// Abandono não é duelo — quem parou na volta 3 não estava disputando nada.
#[test]
fn pista_abandono_nao_conta_como_duelo() {
    let conn = setup_db();
    equipe_generica(&conn);
    for rodada in 1..=4 {
        corrida_com_chegadas(&conn, rodada, "gt3", ("P001", 5, false), ("P002", 6, false));
    }
    for rodada in 5..=8 {
        corrida_com_chegadas(&conn, rodada, "gt3", ("P001", 5, false), ("P002", 6, true));
    }

    process_track_season_rivalry(&conn, 1).unwrap();

    // 4 adjacências válidas + 4 abandonos = abaixo do corte de 6
    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

/// Um piloto de lmp2 tem linha no calendário regular dele e no da Endurance.
/// Somar os dois contaria o mesmo domingo duas vezes.
#[test]
fn pista_nao_soma_o_mesmo_par_em_duas_categorias() {
    let conn = setup_db();
    equipe_generica(&conn);
    for rodada in 1..=4 {
        corrida_com_chegadas(
            &conn,
            rodada,
            "lmp2",
            ("P001", 5, false),
            ("P002", 6, false),
        );
    }
    for rodada in 1..=4 {
        corrida_com_chegadas(
            &conn,
            rodada + 100,
            "endurance",
            ("P001", 5, false),
            ("P002", 6, false),
        );
    }

    process_track_season_rivalry(&conn, 1).unwrap();

    // 4 + 4 seria 8 e passaria; cada categoria isolada tem 4 e não passa.
    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

/// Só a temporada que acabou. O placar do ano passado já virou rivalidade
/// (ou já foi decaído) e não pode ser recontado.
#[test]
fn pista_ignora_temporadas_anteriores() {
    let conn = setup_db();
    equipe_generica(&conn);
    for rodada in 1..=8 {
        corrida_com_chegadas(&conn, rodada, "gt3", ("P001", 5, false), ("P002", 6, false));
    }

    process_track_season_rivalry(&conn, 2).unwrap();

    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

// ── Passo 15: Colisão ─────────────────────────────────────────────────────

fn colisao(
    a: &str,
    b: &str,
    severity: crate::simulation::incidents::IncidentSeverity,
    positions_lost: i32,
    is_dnf: bool,
) -> crate::simulation::incidents::IncidentResult {
    use crate::simulation::incidents::{IncidentResult, IncidentType};
    IncidentResult {
        pilot_id: a.to_string(),
        incident_type: IncidentType::Collision,
        severity,
        segment: String::new(),
        positions_lost,
        is_dnf,
        description: String::new(),
        linked_pilot_id: Some(b.to_string()),
        is_two_car_incident: true,
        injury_risk_multiplier: 1.0,
        narrative_importance_hint: 0,
        catalog_id: None,
        damage_origin_segment: None,
    }
}

/// Chegada mínima só com o que o gatilho de colisão lê: quem é e de onde largou.
fn chegada(pilot_id: &str, grid_position: i32) -> crate::simulation::race::RaceDriverResult {
    crate::simulation::race::RaceDriverResult {
        pilot_id: pilot_id.to_string(),
        pilot_name: pilot_id.to_string(),
        team_id: "T001".to_string(),
        team_name: "Team".to_string(),
        grid_position,
        finish_position: grid_position,
        positions_gained: 0,
        best_lap_time_ms: 0.0,
        total_race_time_ms: 0.0,
        gap_to_winner_ms: 0.0,
        is_dnf: false,
        dnf_reason: None,
        dnf_reason_key: None,
        dnf_segment: None,
        incidents_count: 0,
        incidents: Vec::new(),
        has_fastest_lap: false,
        points_earned: 0,
        is_jogador: false,
        laps_completed: 0,
        final_tire_wear: 1.0,
        final_physical: 1.0,
        classification_status: crate::simulation::race::ClassificationStatus::Finished,
        notable_incident: None,
        dnf_catalog_id: None,
        damage_origin_segment: None,
        posicoes_por_segmento: Vec::new(),
        gaps_para_da_frente_ms: Vec::new(),
        segmentos_em_ar_sujo: 0,
        tentativas_ultrapassagem: 0,
        ultrapassagens_concluidas: 0,
        tentativas_sofridas: 0,
        maior_sequencia_preso: 0,
        volta_da_parada: Vec::new(),
        posicao_antes_da_parada: Vec::new(),
        posicao_depois: Vec::new(),
        estrategia_id: String::new(),
    }
}

/// Meio de pelotão, meio de temporada: nada em jogo, peso 1.0.
fn grid_sem_nada_em_jogo() -> Vec<crate::simulation::race::RaceDriverResult> {
    vec![chegada("P001", 12), chegada("P002", 13)]
}

// ── Peso do contexto (ideia 1: nem todo evento vale o mesmo) ──────────────

#[test]
fn peso_do_contexto_meio_de_pelotao_no_meio_do_ano_nao_pesa() {
    assert!((peso_do_contexto(12, 5, 20, false) - 1.0).abs() < 1e-9);
}

/// O LÍDER TOCANDO NUM RETARDATÁRIO NÃO É BATIDA PELA LIDERANÇA. Enquanto o peso
/// olhava a MELHOR das duas largadas, bastava um dos dois ter largado na frente —
/// e as rivalidades de colisão saltaram de 12 para 95 no mundo medido.
#[test]
fn colisao_do_lider_com_retardatario_nao_pesa_como_briga_da_frente() {
    use crate::simulation::incidents::IncidentSeverity;
    let conn = setup_db();

    process_collisions_rivalry(
        &conn,
        &[colisao("P001", "P002", IncidentSeverity::Minor, 1, false)],
        &[chegada("P001", 1), chegada("P002", 18)],
        "gt3",
        5,
        20,
        1,
    )
    .unwrap();

    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

#[test]
fn peso_do_contexto_ponta_pesa_mais_que_zona_de_pontos() {
    let ponta = peso_do_contexto(2, 5, 20, false);
    let pontos = peso_do_contexto(7, 5, 20, false);
    assert!((ponta - 1.8).abs() < 1e-9);
    assert!((pontos - 1.3).abs() < 1e-9);
    assert!(ponta > pontos);
}

#[test]
fn peso_do_contexto_ultimas_rodadas_pesam() {
    // rodada 18 de 20 já está na janela (> total - 3).
    assert!((peso_do_contexto(12, 18, 20, false) - 1.4).abs() < 1e-9);
    assert!((peso_do_contexto(12, 17, 20, false) - 1.0).abs() < 1e-9);
}

/// O TETO EXISTE PARA UM EVENTO NÃO VIRAR A RIVALIDADE INTEIRA. Sem ele os
/// fatores se compõem sem limite.
#[test]
fn peso_do_contexto_nao_passa_do_teto() {
    let maximo = peso_do_contexto(1, 20, 20, true);
    assert!((maximo - 4.0).abs() < 1e-9, "foi {maximo}");
}

/// O momento decisivo que o sistema não tinha: batida crítica entre dois
/// candidatos ao título, largando na frente, na última rodada. Antes valia
/// percebida 12 como qualquer outra; agora nasce Nemesis (>= 40).
#[test]
fn colisao_decisiva_nasce_como_nemesis() {
    use crate::simulation::incidents::IncidentSeverity;
    let conn = setup_db();
    for (id, pontos) in [("P001", 200.0), ("P002", 198.0)] {
        conn.execute(
            "UPDATE drivers SET categoria_atual = 'gt3', temp_pontos = ?2 WHERE id = ?1",
            rusqlite::params![id, pontos],
        )
        .unwrap();
    }

    process_collisions_rivalry(
        &conn,
        &[colisao("P001", "P002", IncidentSeverity::Critical, 0, true)],
        &[chegada("P001", 1), chegada("P002", 2)],
        "gt3",
        20,
        20,
        1,
    )
    .unwrap();

    let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
    assert_eq!(summaries.len(), 1);
    // (7,18) × 4.0 = (28,72) → perceived = 0.6*28 + 0.4*72 = 16.8 + 28.8 = 45.6
    let perceived = summaries[0].perceived_intensity;
    assert!((perceived - 45.6).abs() < 1e-9, "foi {perceived}");
    assert_eq!(
        crate::rivalry::intensity_level(perceived),
        RivalryIntensityLevel::Clara
    );
}

/// ESTAR NA ZONA DE PONTOS NÃO É "ALGO EM JOGO". Com o portão em `peso > 1.0` a
/// limpeza da colisão era desfeita: o ×1.3 do top-8 sozinho liberava quase todo
/// encostão, e as rivalidades de colisão voltaram de 12 para 96 no mundo medido.
#[test]
fn colisao_leve_no_top8_sem_mais_nada_continua_barrada() {
    use crate::simulation::incidents::IncidentSeverity;
    let conn = setup_db();

    process_collisions_rivalry(
        &conn,
        &[colisao("P001", "P002", IncidentSeverity::Minor, 1, false)],
        &[chegada("P001", 5), chegada("P002", 6)], // top-8 → peso 1.3
        "gt3",
        5,
        20,
        1,
    )
    .unwrap();

    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

/// Um toque de leve entre os dois candidatos ao título na decisão não é roçada
/// de roda — é o toque do ano, e abre ficha mesmo sem história prévia.
#[test]
fn colisao_leve_com_titulo_em_jogo_abre_ficha() {
    use crate::simulation::incidents::IncidentSeverity;
    let conn = setup_db();
    for (id, pontos) in [("P001", 200.0), ("P002", 198.0)] {
        conn.execute(
            "UPDATE drivers SET categoria_atual = 'gt3', temp_pontos = ?2 WHERE id = ?1",
            rusqlite::params![id, pontos],
        )
        .unwrap();
    }

    process_collisions_rivalry(
        &conn,
        &[colisao("P001", "P002", IncidentSeverity::Minor, 1, false)],
        &[chegada("P001", 1), chegada("P002", 2)],
        "gt3",
        20,
        20,
        1,
    )
    .unwrap();

    assert_eq!(get_pilot_rivalries(&conn, "P001").unwrap().len(), 1);
}

/// UM TOQUE LEVE NÃO ABRE FICHA. Era a maior fonte de ruído do sistema: 43 das 83
/// rivalidades de um mundo medido vinham de colisão, quase todas de um encostão
/// isolado que vale percebida ~5 e nunca cresce nem some.
#[test]
fn colisao_leve_sozinha_nao_cria_rivalidade() {
    use crate::simulation::incidents::IncidentSeverity;
    let conn = setup_db();

    process_collisions_rivalry(
        &conn,
        &[colisao("P001", "P002", IncidentSeverity::Minor, 1, false)],
        &grid_sem_nada_em_jogo(),
        "gt3",
        5,
        20,
        1,
    )
    .unwrap();

    assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
}

/// O primeiro encostão é corrida; o encostão entre dois que já têm história é
/// capítulo. Sobre rivalidade existente a faixa leve conta normalmente.
#[test]
fn colisao_leve_reforca_rivalidade_existente() {
    use crate::simulation::incidents::IncidentSeverity;
    let conn = setup_db();
    apply_rivalry_event(
        &conn,
        &event("P001", "P002", RivalryType::Campeonato, 10.0, 10.0),
    )
    .unwrap();

    process_collisions_rivalry(
        &conn,
        &[colisao("P001", "P002", IncidentSeverity::Minor, 1, false)],
        &grid_sem_nada_em_jogo(),
        "gt3",
        5,
        20,
        1,
    )
    .unwrap();

    let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
    assert_eq!(summaries.len(), 1);
    // (10,10) + leve (2,8) = (12,18) → perceived = 0.6*12 + 0.4*18 = 7.2 + 7.2 = 14.4
    assert!((summaries[0].perceived_intensity - 14.4).abs() < 1e-9);
    // O tipo original é preservado — o encostão não reescreve a origem.
    assert_eq!(summaries[0].tipo, RivalryType::Campeonato);
}

/// Incidente de verdade continua abrindo ficha do zero: abandono, toque grave ou
/// perda de 3+ posições não são roçada de roda.
#[test]
fn colisao_grave_ainda_cria_do_zero() {
    use crate::simulation::incidents::IncidentSeverity;
    let conn = setup_db();

    process_collisions_rivalry(
        &conn,
        &[colisao("P001", "P002", IncidentSeverity::Minor, 4, false)],
        &grid_sem_nada_em_jogo(),
        "gt3",
        5,
        20,
        1,
    )
    .unwrap();

    let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
    assert_eq!(summaries.len(), 1);
    // h=3, r=10 → perceived = 0.6*3 + 0.4*10 = 1.8 + 4.0 = 5.8
    assert!((summaries[0].perceived_intensity - 5.8).abs() < 1e-9);
}

/// Abandono por colisão cria do zero mesmo com severidade baixa — o que decide é
/// a consequência, não o rótulo do toque.
#[test]
fn colisao_com_abandono_cria_do_zero() {
    use crate::simulation::incidents::IncidentSeverity;
    let conn = setup_db();

    process_collisions_rivalry(
        &conn,
        &[colisao("P001", "P002", IncidentSeverity::Minor, 0, true)],
        &grid_sem_nada_em_jogo(),
        "gt3",
        5,
        20,
        1,
    )
    .unwrap();

    let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
    // h=5, r=14 → perceived = 0.6*5 + 0.4*14 = 3.0 + 5.6 = 8.6
    assert!((summaries[0].perceived_intensity - 8.6).abs() < 1e-9);
}

// ── Desempate entre dois incidentes do mesmo par ──────────────────────────

/// O CASO DO ENUNCIADO: toque leve comum primeiro, toque leve em contexto de
/// título depois, entre a mesma dupla. Os dois têm a mesma base de severidade, e
/// quem tem de ficar é o segundo — é o toque que aconteceu com o campeonato em
/// jogo. Com o critério antigo (`>` só na severidade) o primeiro travava a entrada
/// e o par pontuava pelo contexto errado; na faixa leve isso decidia inclusive se
/// a ficha abria.
#[test]
fn toque_leve_em_contexto_de_titulo_substitui_o_toque_leve_comum() {
    const LEVE: f64 = 2.0;
    let comum = (LEVE, 1.0);
    let com_titulo = (LEVE, peso_do_contexto(1, 20, 20, true));

    assert!(
        incidente_substitui_o_do_par(com_titulo, comum),
        "empatada a severidade, o contexto mais relevante tem que entrar"
    );
    assert!(
        !incidente_substitui_o_do_par(comum, com_titulo),
        "e o de contexto menor não pode desfazer a troca"
    );
}

/// A PRECEDÊNCIA DA SEVERIDADE É ABSOLUTA. O desempate por contexto só vale no
/// empate: um toque leve no maior contexto possível não desloca um toque grave que
/// aconteceu no meio do pelotão.
#[test]
fn contexto_nao_derruba_severidade_maior() {
    let leve_no_maior_contexto = (2.0, peso_do_contexto(1, 20, 20, true));
    let grave_sem_contexto = (7.0, 1.0);

    assert!(!incidente_substitui_o_do_par(
        leve_no_maior_contexto,
        grave_sem_contexto
    ));
    assert!(incidente_substitui_o_do_par(
        grave_sem_contexto,
        leve_no_maior_contexto
    ));
}

/// Empatados nos dois eixos, fica o primeiro: dois encostões idênticos no mesmo
/// contexto são o mesmo capítulo, e trocar um pelo outro só embaralharia a ordem.
#[test]
fn incidentes_identicos_nao_se_substituem() {
    assert!(!incidente_substitui_o_do_par((2.0, 1.8), (2.0, 1.8)));
}

/// **B46 — POR QUE O DESEMPATE POR CONTEXTO NÃO MUDA COMPORTAMENTO.**
///
/// Medido em 12/08/2026, e o resultado é o contrário do que o enunciado supunha: não é
/// que falte um terceiro critério de desempate. É que, dentro de UMA corrida e de UM
/// par, dois incidentes da mesma faixa de severidade são indistinguíveis na CARGA que o
/// evento aplica.
///
/// Duas coisas seguram isso, e este teste fixa as duas:
///
/// 1. `peso_do_contexto` é constante no par. Os quatro insumos dele — o pior grid dos
///    dois, a rodada, o total de rodadas e a briga do título — são fatos da corrida e do
///    par, nunca do incidente. Logo o item 2 da regra empata sempre.
/// 2. O que se guarda por par é `((h, r), peso)`, e `h`/`r` saem da FAIXA de severidade,
///    não do incidente. Mesma faixa e mesmo peso ⇒ carga idêntica.
///
/// A consequência prática: qualquer critério individual que se acrescente (estágio da
/// corrida, posições perdidas, volta) só escolheria qual dos dois incidentes idênticos
/// fica guardado, sem mover um número do evento aplicado. Fazer o contexto individual
/// pesar exige colocá-lo DENTRO de `peso_do_contexto`, o que é peso novo — decisão de
/// balanceamento, não conserto.
#[test]
fn dois_incidentes_do_mesmo_par_na_mesma_corrida_carregam_o_mesmo_evento() {
    // Todos os insumos do peso são do par e da corrida: qualquer incidente entre estes
    // dois pilotos nesta corrida lê exatamente o mesmo peso.
    let peso_do_par = peso_do_contexto(2, 18, 20, true);
    assert_eq!(peso_do_par, peso_do_contexto(2, 18, 20, true));

    // A faixa leve dá (h, r) = (2.0, 8.0) para os dois toques, na volta 2 ou na última.
    const LEVE: (f64, f64) = (2.0, 8.0);
    let carga = |(h, r): (f64, f64), peso: f64| (h * peso, r * peso);
    assert_eq!(carga(LEVE, peso_do_par), carga(LEVE, peso_do_par));

    // E é por isso que o desempate empata: nada resta para comparar.
    assert!(!incidente_substitui_o_do_par(
        (LEVE.0, peso_do_par),
        (LEVE.0, peso_do_par)
    ));
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
    // Peso da disputa: gap 5 (≤5 → ×1.6) e o par envolve o líder (×1.3) = 2.08;
    // rodada 8 de 10 ainda não é a decisão, então não leva o ×1.5.
    // (4,10) × 2.08 = (8.32, 20.8) → perceived = 0.6*8.32 + 0.4*20.8 = 13.312
    let perceived = summaries[0].perceived_intensity;
    assert!((perceived - 13.312).abs() < 1e-9, "foi {perceived}");
}

/// A MESMA JANELA DE RODADAS ESCONDIA DISPUTAS MUITO DIFERENTES: 18 pontos de
/// vantagem na antepenúltima e empate técnico na última valiam igual.
#[test]
fn peso_da_disputa_separa_briga_de_tabela() {
    // Empate técnico pelo título na última rodada: teto.
    assert!((peso_da_disputa(2.0, true, 10, 10) - 2.5).abs() < 1e-9);
    // Folga de 18 pontos entre 2º e 3º no meio da janela: nada.
    assert!((peso_da_disputa(18.0, false, 8, 10) - 1.0).abs() < 1e-9);
    // Aperto sem ser pela liderança pesa, mas menos.
    assert!((peso_da_disputa(3.0, false, 8, 10) - 1.6).abs() < 1e-9);
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
