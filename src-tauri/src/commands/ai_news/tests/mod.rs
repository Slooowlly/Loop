use super::*;
use crate::race_eval::Assessment;
use serde_json::json;
use serial_test::serial;

/// Os fatos saem no locale ativo; estes testes conferem a prosa PT, então fixam o
/// idioma antes de rodar. `#[serial]` porque o locale é estado global do processo.
fn pt() {
    rust_i18n::set_locale("pt-BR");
}

fn sig() -> PostRaceSignals {
    // Base neutra: terminou dentro do esperado, sem drama.
    PostRaceSignals {
        is_dnf: false,
        dnf_mechanical: false,
        grid: 6,
        finish: 6,
        positions_gained: 0,
        has_fastest_lap: false,
        assessment: Some(Assessment::Dentro),
        target_low: 5,
        target_high: 7,
        duel: None,
        track_name: "Interlagos".to_string(),
    }
}

fn thesis_of(s: &PostRaceSignals) -> String {
    select_post_race_thesis(s).0
}

#[test]
#[serial]
fn dnf_mecanico_vence_tudo_e_isenta_o_piloto() {
    pt();
    let mut s = sig();
    s.is_dnf = true;
    s.dnf_mechanical = true;
    s.assessment = Some(Assessment::MuitoAbaixo);
    let (stmt, support) = select_post_race_thesis(&s);
    assert!(stmt.contains("DRAMA MECÂNICO"));
    assert!(stmt.contains("não foi erro"));
    assert!(support.contains(&"breakdowns"));
}

#[test]
#[serial]
fn dnf_por_incidente_e_fim_precoce() {
    pt();
    let mut s = sig();
    s.is_dnf = true;
    s.dnf_mechanical = false;
    assert!(thesis_of(&s).contains("FIM PRECOCE"));
}

#[test]
#[serial]
fn vitoria_e_a_manchete() {
    pt();
    let mut s = sig();
    s.finish = 1;
    s.positions_gained = 5;
    s.has_fastest_lap = true;
    let stmt = thesis_of(&s);
    assert!(stmt.contains("VITÓRIA"));
    assert!(stmt.contains("volta mais rápida"));
}

#[test]
#[serial]
fn remontada_quando_ganha_muitas_posicoes() {
    pt();
    let mut s = sig();
    s.grid = 12;
    s.finish = 4;
    s.positions_gained = 8;
    s.assessment = Some(Assessment::Acima);
    assert!(thesis_of(&s).contains("RECUPERAÇÃO"));
}

/// A borda exata do limiar unificado. O debrief exigia 5 posições antes de passar a
/// ler `race_signals::remontada` (que é 4) — e nenhum teste cobria a faixa que mudou,
/// porque todos usavam ganhos folgados. Uma remontada de 4 é a tese do dia; uma de 3
/// é dia de somar.
#[test]
#[serial]
fn remontada_dispara_com_4_posicoes_e_nao_com_3() {
    pt();
    let mut s = sig();
    s.grid = 10;
    s.finish = 6;
    s.positions_gained = 4;
    assert!(thesis_of(&s).contains("RECUPERAÇÃO"));

    s.grid = 9;
    s.positions_gained = 3;
    assert!(!thesis_of(&s).contains("RECUPERAÇÃO"));
}

#[test]
#[serial]
fn colapso_quando_perde_muitas_posicoes() {
    pt();
    let mut s = sig();
    s.grid = 3;
    s.finish = 11;
    s.positions_gained = -8;
    s.assessment = Some(Assessment::Abaixo);
    assert!(thesis_of(&s).contains("ESCAPOU"));
}

#[test]
#[serial]
fn acima_e_abaixo_do_esperado_sem_drama() {
    pt();
    let mut over = sig();
    over.finish = 3;
    over.assessment = Some(Assessment::Acima);
    assert!(thesis_of(&over).contains("ACIMA DO ESPERADO"));

    let mut under = sig();
    under.finish = 9;
    under.assessment = Some(Assessment::Abaixo);
    assert!(thesis_of(&under).contains("AQUÉM"));
}

#[test]
#[serial]
fn duelo_decide_um_dia_morno() {
    pt();
    let mut s = sig(); // assessment Dentro, nada extremo
    s.duel = Some(PostRaceDuel {
        name: "K. Novak".to_string(),
        player_won: true,
        is_nemesis: true,
        h2h: Some((3, 2)),
    });
    let stmt = thesis_of(&s);
    assert!(stmt.contains("O DUELO"));
    assert!(stmt.contains("K. Novak"));
    assert!(stmt.contains("nemesis"));
    assert!(stmt.contains("3-2"));
}

#[test]
#[serial]
fn dia_de_somar_quando_nada_se_destaca() {
    pt();
    assert!(thesis_of(&sig()).contains("DIA DE SOMAR"));
}

#[test]
#[serial]
fn telemetry_facts_resume_ritmo_ultrapassagens_e_erro() {
    pt();
    let tel = json!({
        "has_telemetry": true,
        "pace": { "vs_grid_ms": -506.0, "vs_grid_reliable": true, "good_laps": 8 },
        "position_flow": { "gained_on_track": 4, "lost_on_track": 1 },
        "best_moment": { "lap": 8, "positions_gained": 3 },
        "mistake": { "lap": 9, "positions_lost": 1, "time_lost_ms": 600.0 },
        "charts": {
            "rival_name": "Massimo Caruso",
            "rival_gap": [ { "lap": 13.0, "gap_s": 0.8 } ],
            "lap_times": [
                { "lap": 6.0, "time_s": 71.0 },
                { "lap": 7.0, "time_s": 71.3 },
                { "lap": 8.0, "time_s": 71.6 },
                { "lap": 9.0, "time_s": 71.9 }
            ],
            "cars": [
                { "idx": 0, "is_player": true, "name": "Você", "points": [
                    { "lap": 6.0, "position": 7 },
                    { "lap": 7.4, "position": 4 },
                    { "lap": 9.0, "position": 6 }
                ] },
                { "idx": 1, "is_player": false, "name": "Bruno Perez", "points": [
                    { "lap": 6.0, "position": 4 },
                    { "lap": 7.4, "position": 7 },
                    { "lap": 9.0, "position": 4 }
                ] }
            ]
        }
    });
    let out = telemetry_facts(Some(&tel), 8);
    assert!(out.contains("MAIS RÁPIDO"), "ritmo vs grid: {out}");
    assert!(out.contains("Degradação"), "degradação: {out}");
    assert!(
        out.contains("volta 7.4: passou Bruno Perez"),
        "feed de ultrapassagem: {out}"
    );
    assert!(
        out.contains("Bruno Perez: você terminou ATRÁS dele (P6 contra P4)"),
        "desfecho do duelo (passou e foi repassado): {out}"
    );
    assert!(out.contains("Largada: P8 → P7"), "largada: {out}");
    assert!(out.contains("Erro mais caro: volta 9"), "erro: {out}");
    assert!(
        out.contains("Massimo Caruso terminou à sua frente"),
        "duelo direto: {out}"
    );
}

#[test]
fn telemetry_facts_vazio_sem_telemetria() {
    assert!(telemetry_facts(None, 5).is_empty());
    assert!(telemetry_facts(Some(&json!({ "has_telemetry": false })), 5).is_empty());
}

/// A MATRIZ DE TOM do fechamento do fim de semana. O eixo é "quem leva a conta": anunciado
/// e entregue concordando → as condições explicam; divergindo → o piloto explica.
///
/// Este teste existe porque a matriz é a decisão de design, não um detalhe: trocar duas
/// frases de lugar inverteria a atribuição sem quebrar mais nada.
#[test]
fn matriz_de_tom_do_anuncio_atribui_a_conta_a_quem_deve() {
    // Anunciado A FAVOR.
    assert_eq!(
        caso_do_anuncio(3, Assessment::MuitoAcima),
        Some("ai_news.facts.forecast_good_delivered"),
        "a favor × acima: confirmação, a conta é das condições"
    );
    // O caso que impede a leitura de virar álibi: as condições estavam do lado dele.
    assert_eq!(
        caso_do_anuncio(2, Assessment::MuitoAbaixo),
        Some("ai_news.facts.forecast_good_missed"),
        "a favor × abaixo: a conta é do PILOTO"
    );

    // Anunciado CONTRA.
    assert_eq!(
        caso_do_anuncio(-2, Assessment::Abaixo),
        Some("ai_news.facts.forecast_bad_confirmed"),
        "contra × abaixo: contexto, a conta é das condições"
    );
    assert_eq!(
        caso_do_anuncio(-4, Assessment::Acima),
        Some("ai_news.facts.forecast_bad_beaten"),
        "contra × acima: o crédito é do PILOTO"
    );

    // Os dois casos de DIVERGÊNCIA (os informativos) nunca podem colidir com os de
    // confirmação — é a inversão que este teste existe para pegar.
    assert_ne!(
        caso_do_anuncio(2, Assessment::MuitoAbaixo),
        caso_do_anuncio(-2, Assessment::Abaixo)
    );
    assert_ne!(
        caso_do_anuncio(-4, Assessment::Acima),
        caso_do_anuncio(3, Assessment::MuitoAcima)
    );
}

/// Nem toda corrida rende uma frase. Anúncio neutro não tem previsão a cobrar, e resultado
/// "dentro do esperado" não tem desvio a explicar — calar é a resposta certa, mesmo
/// princípio da regra do vazio.
#[test]
fn anuncio_neutro_ou_resultado_esperado_nao_rende_frase() {
    assert_eq!(caso_do_anuncio(0, Assessment::MuitoAcima), None);
    assert_eq!(caso_do_anuncio(0, Assessment::MuitoAbaixo), None);
    assert_eq!(caso_do_anuncio(5, Assessment::Dentro), None);
    assert_eq!(caso_do_anuncio(-5, Assessment::Dentro), None);
}

/// A frase depende do SINAL do anúncio, não da magnitude: +1 e +6 contam a mesma história
/// ("estava a favor"). A magnitude é assunto da tela, que mostra faixa por camada.
#[test]
fn so_o_sinal_do_anuncio_importa_para_a_frase() {
    for soma in 1..=6 {
        assert_eq!(
            caso_do_anuncio(soma, Assessment::Abaixo),
            Some("ai_news.facts.forecast_good_missed")
        );
    }
    for soma in -6..=-1 {
        assert_eq!(
            caso_do_anuncio(soma, Assessment::Abaixo),
            Some("ai_news.facts.forecast_bad_confirmed")
        );
    }
}
