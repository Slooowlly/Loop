use super::*;

/// Save mínimo com DOIS pilotos na categoria, cada um com UMA vitória: o recorde de
/// vitórias da categoria vale 1 e o segundo piloto o iguala (gap 0). É o cenário que
/// revela de uma vez os dois defeitos da nota "recorde a caminho" — o plural cravado
/// ("1 vitórias") e o `subject` que carregava a frase inteira em vez do nome.
fn seed_recorde_de_uma_vitoria() -> Database {
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("unix epoch")
        .as_nanos();
    let db_path = std::env::temp_dir()
        .join(format!("iracerapp_world_footer_recorde_{nanos}"))
        .join("career.db");
    let db = Database::create_new(&db_path).expect("create db");

    // `foreign_keys=ON` no save real: temporada, equipes e pilotos precisam existir
    // antes do calendário e dos resultados.
    let season = crate::models::season::Season::new("S1".to_string(), 1, 2024);
    crate::db::queries::seasons::insert_season(&db.conn, &season).expect("insert season");

    for (id, nome) in [("T001", "Equipe Um"), ("T002", "Equipe Dois")] {
        let team = crate::models::team::placeholder_team_from_db(
            id.to_string(),
            nome.to_string(),
            "gt3".to_string(),
            crate::common::time::current_timestamp(),
        );
        crate::db::queries::teams::insert_team(&db.conn, &team).expect("insert team");
    }

    for (id, nome) in [("P001", "Ana Recordista"), ("P002", "Bruno Perseguidor")] {
        let mut driver = crate::models::driver::Driver::new(
            id.to_string(),
            nome.to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            25,
            2024,
        );
        driver.categoria_atual = Some("gt3".to_string());
        driver.status = DriverStatus::Ativo;
        crate::db::queries::drivers::insert_driver(&db.conn, &driver).expect("insert driver");
    }

    db.conn
        .execute_batch(
            "INSERT INTO calendar (id, temporada_id, rodada, pista, categoria, track_name)
                VALUES ('R1', 'S1', 1, 'Interlagos', 'gt3', 'Interlagos'),
                       ('R2', 'S1', 2, 'Spa', 'gt3', 'Spa');
             INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos)
                VALUES ('R1', 'P001', 'T001', 1, 25.0),
                       ('R2', 'P002', 'T002', 1, 25.0);",
        )
        .expect("seed calendario e resultados");

    db
}

/// A nota de "recorde a caminho" com recorde de valor 1. Trava as duas correções no
/// PONTO DE USO: `subject` é o nome de quem a nota fala (e não uma segunda cópia do
/// texto, que era o que o front recebia como rótulo curto), e a concordância sai do
/// valor real — "de vitória: 1", não "de vitórias: 1".
#[test]
#[serial_test::serial]
fn nota_de_recorde_a_caminho_traz_o_nome_no_subject_e_singular_no_valor_um() {
    rust_i18n::set_locale("pt-BR");
    let db = seed_recorde_de_uma_vitoria();

    let mut used = HashSet::new();
    let notas = record_watch_notes(&db.conn, "gt3", &mut used, 5);

    let nota = notas
        .iter()
        .find(|n| n.kind == "recorde_a_caminho")
        .expect("deveria haver uma nota de recorde a caminho");

    assert_eq!(
        nota.subject, "Bruno Perseguidor",
        "subject tem que ser o NOME do piloto, não o texto da nota: {:?}",
        nota.subject
    );
    assert_ne!(
        nota.subject, nota.text,
        "subject e text não podem ser a mesma string"
    );
    assert!(
        nota.text.contains("de vitória da categoria: 1"),
        "recorde de 1 pede o singular: {}",
        nota.text
    );
    assert!(!nota.text.contains("vitórias"), "{}", nota.text);
}

fn note(text: &str) -> WorldNote {
    WorldNote {
        id: "x".into(),
        tag: "RECORDE".into(),
        subject: "Fulano".into(),
        kind: "recorde_quebrado".into(),
        tone: "recorde".into(),
        text: text.into(),
    }
}

#[test]
fn substitui_quando_conta_bate() {
    let notes = vec![note("template 1"), note("template 2")];
    let ai = vec!["reescrita 1".to_string(), "reescrita 2".to_string()];
    let out = apply_ai_texts(notes, &ai).expect("deveria casar");
    assert_eq!(out[0].text, "reescrita 1");
    assert_eq!(out[1].text, "reescrita 2");
    // Preserva os metadados (só o texto muda).
    assert_eq!(out[0].kind, "recorde_quebrado");
}

#[test]
fn mantem_template_quando_conta_diverge() {
    let notes = vec![note("a"), note("b")];
    let ai = vec!["só uma".to_string()];
    assert!(apply_ai_texts(notes, &ai).is_none());
}

#[test]
fn mantem_template_quando_ha_reescrita_vazia() {
    let notes = vec![note("a"), note("b")];
    let ai = vec!["ok".to_string(), "   ".to_string()];
    assert!(apply_ai_texts(notes, &ai).is_none());
}

#[test]
fn vazio_nao_casa() {
    assert!(apply_ai_texts(vec![], &[]).is_none());
}

/// Guarda a i18n do rodapé nos DOIS locales: tags, nouns singular/plural, ordinais
/// gendered (PT) vs sufixo (EN) e interpolação (sem `%{...}` cru). `#[serial]` porque
/// troca o locale global (não corre junto do i18n_smoke).
#[test]
#[serial_test::serial]
fn i18n_do_rodape_resolve_nos_dois_locales() {
    rust_i18n::set_locale("pt-BR");
    assert_eq!(tag_label("record"), "RECORDE");
    assert_eq!(metric_noun("wins", 1), "vitória");
    assert_eq!(metric_noun("wins", 3), "vitórias");
    assert_eq!(ord_label(2, false), "2º");
    assert_eq!(ord_label(2, true), "2ª");
    let pt = rust_i18n::t!(
        "world_footer.record_broken.season_wins_prev",
        name = "Fulano",
        value = 9,
        prev = 7
    )
    .to_string();
    assert!(
        pt.contains('9') && pt.contains('7') && !pt.contains("%{"),
        "{pt}"
    );

    rust_i18n::set_locale("en-US");
    assert_eq!(tag_label("record"), "RECORD");
    assert_eq!(metric_noun("wins", 1), "win");
    assert_eq!(metric_noun("wins", 3), "wins");
    assert_eq!(ord_label(2, false), "2nd");
    assert_eq!(ord_label(3, true), "3rd");
    assert_eq!(ord_label(11, false), "11th"); // regra do 11–13
    let en = rust_i18n::t!(
        "world_footer.record_watch.approaching",
        name = "X",
        gap = 2,
        noun = metric_noun("wins", 2),
        value = 50,
        holder = "Y"
    )
    .to_string();
    assert!(en.contains("2 wins") && !en.contains("%{"), "{en}");

    rust_i18n::set_locale("pt-BR"); // restaura o default pros demais testes.
}

/// Recorde de UM. É o caso que o plural fixo escondia: `record_watch.tied` e o texto
/// genérico de `record_broken` pediam o substantivo com contagem 2 cravada, então um
/// recorde de 1 saía "o recorde histórico de vitórias: 1". A concordância agora vem do
/// mesmo número que o texto imprime, e este caso é quem cobra isso.
#[test]
#[serial_test::serial]
fn recorde_de_valor_um_sai_no_singular() {
    rust_i18n::set_locale("pt-BR");

    let igualou = rust_i18n::t!(
        "world_footer.record_watch.tied",
        name = "Fulano",
        noun = metric_noun("wins", 1),
        value = 1,
        holder = "Beltrano"
    )
    .to_string();
    assert!(igualou.contains("de vitória da categoria: 1"), "{igualou}");
    assert!(!igualou.contains("vitórias"), "{igualou}");

    let quebrou = rust_i18n::t!(
        "world_footer.record_broken.generic",
        name = "Fulano",
        noun = metric_noun("titles", 1),
        value = 1
    )
    .to_string();
    assert!(quebrou.contains("de título"), "{quebrou}");
    assert!(
        !quebrou.contains("títulos") && !quebrou.contains("%{"),
        "{quebrou}"
    );

    // E o plural segue intacto onde o valor realmente é maior que 1.
    let plural = rust_i18n::t!(
        "world_footer.record_watch.tied",
        name = "Fulano",
        noun = metric_noun("wins", 12),
        value = 12,
        holder = "Beltrano"
    )
    .to_string();
    assert!(plural.contains("de vitórias da categoria: 12"), "{plural}");

    rust_i18n::set_locale("pt-BR");
}
