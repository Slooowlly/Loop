use super::*;
use crate::simulation::incidents::{IncidentResult, IncidentSeverity, IncidentType};

/// Fatos + tese resolvem nos dois locales, com interpolação (sem `%{...}` cru).
/// `#[serial]` porque troca o locale global. Parity garante as chaves; isto garante
/// que os NOMES dos placeholders casam.
#[test]
#[serial_test::serial]
fn fatos_e_tese_resolvem_nos_dois_locales() {
    rust_i18n::set_locale("pt-BR");
    let pt = rust_i18n::t!("narrative.beat.winner", name = "Ana", team = "Alfa", grid = 4, extra = "").to_string();
    assert!(pt.contains("Ana") && pt.contains("Vencedor") && !pt.contains("%{"), "{pt}");
    let pt_t = rust_i18n::t!("narrative.thesis.improbable_win", name = "Ana", team = "Alfa", grid = 8).to_string();
    assert!(pt_t.contains("Ana") && pt_t.contains("P8") && !pt_t.contains("%{"), "{pt_t}");

    rust_i18n::set_locale("en-US");
    let en = rust_i18n::t!("narrative.beat.winner", name = "Ana", team = "Alfa", grid = 4, extra = "").to_string();
    assert!(en.contains("Ana") && en.contains("Winner") && !en.contains("%{"), "{en}");
    assert_ne!(pt, en);

    rust_i18n::set_locale("pt-BR");
}

fn inc(
    pilot: &str,
    typ: IncidentType,
    sev: IncidentSeverity,
    is_dnf: bool,
    positions_lost: i32,
) -> IncidentResult {
    crate::simulation::incidents::make_incident(
        pilot.to_string(),
        typ,
        sev,
        "Mid",
        positions_lost,
        is_dnf,
        "toque na entrada da curva".to_string(),
        None,
        false,
        None,
        None,
    )
}

/// O CORAÇÃO do pedido: a batida do jogador é sempre citada, mas o peso — e com
/// ele o tom — acompanha o tamanho do impacto.
#[test]
fn batida_do_jogador_escala_com_a_gravidade() {
    let leve = inc("p", IncidentType::Collision, IncidentSeverity::Minor, false, 0);
    let forte = inc("p", IncidentType::Collision, IncidentSeverity::Major, false, 2);
    let grave = inc("p", IncidentType::Collision, IncidentSeverity::Critical, false, 5);

    let (wl, wf, wg) = (
        incident_weight(&leve, true),
        incident_weight(&forte, true),
        incident_weight(&grave, true),
    );
    assert!(wl < wf && wf < wg, "gradação: {wl} < {wf} < {wg}");
    // Mesmo o toque leve passa do limiar — ele TEM que ser mencionado.
    assert!(wl >= THRESHOLD, "toque leve do jogador entra: {wl}");
}

/// A IA não pode inundar o boletim: susto leve fica fora, batida de verdade entra.
#[test]
fn batida_de_ia_so_entra_quando_e_de_verdade() {
    let leve = inc("ai", IncidentType::Collision, IncidentSeverity::Minor, false, 0);
    let forte = inc("ai", IncidentType::Collision, IncidentSeverity::Major, false, 3);
    assert!(incident_weight(&leve, false) < THRESHOLD, "susto leve de IA fica fora");
    assert!(incident_weight(&forte, false) >= THRESHOLD, "batida de IA entra");
}

/// O mesmo incidente pesa mais no piloto do leitor do que num rival — é a revista
/// do jogador, o acidente dele importa mais.
#[test]
fn jogador_pesa_mais_que_ia_no_mesmo_incidente() {
    let i = inc("x", IncidentType::Collision, IncidentSeverity::Major, false, 2);
    assert!(incident_weight(&i, true) > incident_weight(&i, false));
}

/// "Batida" exclui pane mecânica: o carro quebrar não é alguém bater. A regra
/// mora em `race_signals` — é a mesma que o debrief do jogador usa.
#[test]
fn pane_mecanica_nao_conta_como_batida() {
    use crate::race_signals::dnf_kind;
    let mecanico = inc("a", IncidentType::Mechanical, IncidentSeverity::Critical, true, 0);
    let colisao = inc("b", IncidentType::Collision, IncidentSeverity::Minor, true, 0);
    let erro = inc("c", IncidentType::DriverError, IncidentSeverity::Major, true, 0);
    assert!(!dnf_kind(Some(&mecanico), false, None).is_crash());
    assert!(dnf_kind(Some(&colisao), false, None).is_crash());
    assert!(dnf_kind(Some(&erro), false, None).is_crash());
}

/// Um piloto que se meteu em três confusões rende UMA linha — a pior delas.
#[test]
fn cada_piloto_rende_so_o_pior_incidente() {
    let incidents = vec![
        inc("a", IncidentType::Collision, IncidentSeverity::Minor, false, 0),
        inc("a", IncidentType::Collision, IncidentSeverity::Critical, false, 4),
        inc("a", IncidentType::Collision, IncidentSeverity::Major, false, 1),
        inc("b", IncidentType::Collision, IncidentSeverity::Minor, false, 0),
        // DNF não entra aqui — quem abandonou já tem o beat de Abandono.
        inc("c", IncidentType::Collision, IncidentSeverity::Critical, true, 9),
    ];
    let worst = worst_non_dnf_incident_per_pilot(&incidents);
    assert_eq!(worst.len(), 2, "um por piloto, sem os DNFs");
    let a = worst.iter().find(|i| i.pilot_id == "a").unwrap();
    assert_eq!(a.severity, IncidentSeverity::Critical);
    assert!(!worst.iter().any(|i| i.pilot_id == "c"), "DNF fica de fora");
}

/// A amarela é consequência de batida que para carro na pista — não de pane, nem
/// de susto leve. Incidentes no mesmo segmento são a MESMA bandeira.
#[test]
fn amarela_so_nasce_de_batida_que_para_carro() {
    use crate::simulation::race::derive_caution_segments;
    use IncidentSeverity::{Critical, Major, Minor};
    use IncidentType::{Collision, Mechanical};

    let caso = |i: IncidentResult| derive_caution_segments(&vec![i]).len();

    // Pane mecânica grave: o carro recolhe pro box, não neutraliza.
    assert_eq!(caso(inc("a", Mechanical, Critical, true, 0)), 0);
    // Susto leve: não neutraliza.
    assert_eq!(caso(inc("a", Collision, Minor, false, 0)), 0);
    // Batida forte: neutraliza mesmo sem abandono.
    assert_eq!(caso(inc("a", Collision, Critical, false, 3)), 1);
    // Batida média só neutraliza se tirou o carro da corrida.
    assert_eq!(caso(inc("a", Collision, Major, false, 2)), 0);
    assert_eq!(caso(inc("a", Collision, Major, true, 0)), 1);
}

/// Duas batidas no mesmo segmento = uma bandeira só (seria a mesma amarela).
#[test]
fn incidentes_no_mesmo_segmento_sao_uma_amarela_so() {
    use crate::simulation::race::derive_caution_segments;

    let incidents = vec![
        inc("a", IncidentType::Collision, IncidentSeverity::Critical, true, 0),
        inc("b", IncidentType::Collision, IncidentSeverity::Critical, true, 0),
    ];
    assert_eq!(derive_caution_segments(&incidents).len(), 1);
}

/// O beat de batida usa o limiar padrão (não o do jogador): quem separa jogador de
/// IA é o PESO, não o limiar.
#[test]
fn beat_de_acidente_usa_o_limiar_padrao() {
    assert!(beat(BeatKind::Acidente, 32.0).passes());
    assert!(!beat(BeatKind::Acidente, 16.0).passes());
}

fn beat(kind: BeatKind, weight: f64) -> Beat {
    Beat {
        kind,
        weight,
        text: "x".to_string(),
        driver_id: None,
        team_name: None,
    }
}

#[test]
fn limiar_corta_beats_fracos_mas_preserva_o_nosso_piloto() {
    let beats = vec![
        beat(BeatKind::Vitoria, 70.0),
        beat(BeatKind::VoltaRapida, 15.0), // abaixo de 30 → fora
        beat(BeatKind::NossoPiloto, 27.0), // abaixo de 30, mas acima do limiar do jogador (25) → entra
    ];
    let sel = select(beats);
    assert!(sel.iter().any(|b| b.kind == BeatKind::Vitoria));
    assert!(!sel.iter().any(|b| b.kind == BeatKind::VoltaRapida));
    assert!(sel.iter().any(|b| b.kind == BeatKind::NossoPiloto));
}

/// O arco de rivalidade chega de fora (o callsite tem o banco) e passa pelo MESMO
/// limiar dos beats da corrida — não é passe livre por ser de carreira.
#[test]
fn arco_de_rivalidade_obedece_o_limiar_padrao() {
    assert!(beat(BeatKind::RivalidadeArco, 48.0).passes());
    assert!(!beat(BeatKind::RivalidadeArco, 22.0).passes());
}

/// Arco forte vira DESTAQUE mesmo quando a tese não o pediu (a tese só lê o
/// `RaceResult`, então é cega para a novela). Arco morno fica no pano de fundo.
#[test]
fn arco_forte_sobe_para_destaque_sozinho() {
    assert!(beat(BeatKind::RivalidadeArco, ARC_HIGHLIGHT_WEIGHT).forces_highlight());
    assert!(!beat(BeatKind::RivalidadeArco, ARC_HIGHLIGHT_WEIGHT - 1.0).forces_highlight());
    // O privilégio é só do arco: peso alto de outro tipo não fura a tese.
    assert!(!beat(BeatKind::Podio, 90.0).forces_highlight());
}

#[test]
fn selecao_ordena_por_peso_decrescente() {
    let beats = vec![
        beat(BeatKind::Podio, 30.0),
        beat(BeatKind::Vitoria, 70.0),
        beat(BeatKind::Recuperacao, 45.0),
    ];
    let sel = select(beats);
    assert_eq!(sel.first().map(|b| b.kind.clone()), Some(BeatKind::Vitoria));
    assert_eq!(sel.last().map(|b| b.kind.clone()), Some(BeatKind::Podio));
}

fn sig() -> RaceThesisSignals {
    // Base: vencedor de P3, sem caos, sem pole frustrada, sem remontada.
    RaceThesisSignals {
        total_dnfs: 1,
        field_size: 20,
        winner_name: "A. Vega".to_string(),
        winner_team: "Aurora".to_string(),
        winner_grid: 3,
        pole_flopped: None,
        biggest_recovery: None,
    }
}

fn thesis_of(s: &RaceThesisSignals) -> RaceThesis {
    select_race_thesis(s).0
}

#[test]
fn caos_quando_muitos_abandonos() {
    let mut s = sig();
    s.total_dnfs = 6; // >= max(4, 20/4=5)
    assert_eq!(thesis_of(&s), RaceThesis::Caos);
}

#[test]
fn caos_vence_ate_a_vitoria_improvavel() {
    let mut s = sig();
    s.total_dnfs = 7;
    s.winner_grid = 9; // seria VitoriaImprovavel, mas o caos é o ângulo
    assert_eq!(thesis_of(&s), RaceThesis::Caos);
}

#[test]
fn vitoria_improvavel_quando_vencedor_veio_de_tras() {
    let mut s = sig();
    s.winner_grid = 8;
    let (t, stmt, _) = select_race_thesis(&s);
    assert_eq!(t, RaceThesis::VitoriaImprovavel);
    assert!(stmt.contains("P8"));
}

#[test]
fn pole_frustrada_quando_pole_afunda() {
    let mut s = sig();
    s.pole_flopped = Some(("R. Silva".to_string(), 9));
    let (t, stmt, _) = select_race_thesis(&s);
    assert_eq!(t, RaceThesis::PoleFrustrada);
    assert!(stmt.contains("R. Silva"));
}

#[test]
fn remontada_quando_recuperacao_epica_de_nao_vencedor() {
    let mut s = sig();
    s.biggest_recovery = Some(("K. Novak".to_string(), 18, 4, 14));
    assert_eq!(thesis_of(&s), RaceThesis::Remontada);
}

#[test]
fn recuperacao_pequena_nao_vira_remontada() {
    let mut s = sig();
    s.biggest_recovery = Some(("K. Novak".to_string(), 8, 5, 3)); // gained 3 < 8
    assert_eq!(thesis_of(&s), RaceThesis::CorridaLimpa);
}

#[test]
fn dominio_quando_vencedor_larga_na_frente() {
    let mut s = sig();
    s.winner_grid = 1;
    assert_eq!(thesis_of(&s), RaceThesis::Dominio);
}

#[test]
fn corrida_limpa_como_piso() {
    assert_eq!(thesis_of(&sig()), RaceThesis::CorridaLimpa);
}

/// Resposta cortada pelo teto de tokens do servidor: a última frase pendurada é
/// aparada, mas o texto NUNCA é descartado — curto ou cortado cedo, vale o que veio.
#[test]
fn boletim_cortado_no_meio_da_frase_e_aparado() {
    use crate::narrative::client::aparar_frase_incompleta;

    let inteiro = "Foi um domingo de decisões. Ana venceu em Lédenon.";
    assert_eq!(aparar_frase_incompleta(inteiro), inteiro);

    let cortado = "Foi um domingo de decisões. Ana venceu em Lédenon. O resultado em Lédenon";
    assert_eq!(
        aparar_frase_incompleta(cortado),
        "Foi um domingo de decisões. Ana venceu em Lédenon."
    );

    // Cortado antes da primeira frase fechar: não há o que aparar sem perder tudo,
    // então vale o pedaço como veio.
    assert_eq!(
        aparar_frase_incompleta("O resultado em Lédenon"),
        "O resultado em Lédenon"
    );
}
