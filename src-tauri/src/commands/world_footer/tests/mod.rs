use super::*;

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
