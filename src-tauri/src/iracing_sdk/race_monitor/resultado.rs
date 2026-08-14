//! Desfecho e agregação do resultado: a ponte para a camada adaptativa e os
//! helpers que traduzem severidade/status em prosa de DNF.

use super::*;

/// Converte o histórico capturado no [`RaceResult`](crate::iracing_sdk::adaptive::RaceResult)
/// que a camada adaptativa consome (a ponte Fase A → Fase B). `track_id` vem de
/// quem chama (o export sabe a pista). As voltas de TODOS os carros já estão em
/// `car_laps`; aqui só agrupamos por carro e marcamos jogador/DNF.
pub fn build_adaptive_result(
    history: &RaceHistory,
    track_id: i64,
) -> crate::iracing_sdk::adaptive::RaceResult {
    use crate::iracing_sdk::adaptive::{DriverData, Lap, RaceResult};
    let player_idx = history.player_car_idx;
    let player_dnf = history.outcome.to_lowercase().contains("dnf");
    // Monta os pilotos a partir de um conjunto de voltas (corrida OU quali),
    // reusando o resumo por carro (classe/IA/posição). dnf só vale na corrida.
    let build = |laps_src: &[CarLap], dnf_applies: bool| -> Vec<DriverData> {
        history
            .cars_meta
            .iter()
            .filter(|m| !m.is_pace)
            .map(|m| {
                let laps: Vec<Lap> = laps_src
                    .iter()
                    .filter(|l| l.car_idx == m.idx)
                    .map(|l| Lap {
                        lap: l.lap,
                        time: l.time,
                    })
                    .collect();
                let is_player = m.idx == player_idx;
                DriverData {
                    car_idx: m.idx,
                    is_player,
                    is_ai: m.is_ai,
                    car_class_id: m.class_id,
                    finish_pos_in_class: m.class_position,
                    dnf: is_player && dnf_applies && player_dnf,
                    laps,
                }
            })
            .collect()
    };
    let race = build(&history.car_laps, true);
    let qualy = if history.qualy_laps.is_empty() {
        None
    } else {
        Some(build(&history.qualy_laps, false))
    };
    RaceResult {
        track_id,
        yellow_laps: history.yellow_laps.clone(),
        race,
        qualy,
    }
}

// ─── Helpers de desfecho ─────────────────────────────────────────────────────
/// Posto da severidade a partir da CHAVE em texto (0 = "nenhum", 5 = "catastrófico").
///
/// Dentro do monitor ninguém precisa disto: [`Severidade`] é `Ord` e compara sozinha.
/// A função sobrevive para quem recebe a severidade já serializada como texto e não
/// tem como voltar ao enum — hoje o conserto do carro no import
/// (`commands::race::importacao`). Chave desconhecida vale 0, como antes.
pub fn severity_rank(severity: &str) -> usize {
    Severidade::from_key(severity)
        .map(Severidade::rank)
        .unwrap_or(0)
}

/// Pior batida BRUTA de uma tentativa (sem rebaixamento): a maior entre o pico ao vivo e as
/// batidas já fechadas. Só entra impacto confirmado; perda de controle não é dano.
///
/// Os dois caminhos existem porque nenhum sozinho serve. O PICO pega a batida que nunca
/// "fecha" (o jogador bate e segue), mas é sempre um piso: a velocidade PERDIDA na pancada,
/// que é o componente que separa o encostão da destruição e vale até 160 pontos, só é
/// calculada quando a batida fecha. Ler só o pico dizia "leve" para um carro que virou
/// sucata no muro. A batida FECHADA tem a conta inteira; o `max` fica com quem viu mais.
pub(crate) fn worst_raw_severity(attempt: &Attempt) -> Severidade {
    let pico = severity_label(attempt.peak_crash_score);
    let fechada = attempt
        .crashes
        .iter()
        .filter(|c| c.had_impact)
        .map(|c| c.impact_severity)
        .max()
        .unwrap_or(Severidade::Nenhum);
    pico.max(fechada)
}

/// Motivo do DNF: cita a PIOR batida (se houve) + como encerrou.
pub(crate) fn build_dnf_reason(
    attempt: &Attempt,
    ev: &AttemptEvidence,
    ended_by: FimDaTentativa,
) -> String {
    let how = rust_i18n::t!(match ended_by {
        FimDaTentativa::Restart => "race_monitor.ended.restart",
        FimDaTentativa::SimClosed => "race_monitor.ended.sim_closed",
        _ => "race_monitor.ended.other",
    });
    let worst = attempt.crashes.iter().max_by_key(|c| c.severity);
    if let Some(crash) = worst {
        let severidade = severidade_visivel(crash.severity);
        let lap = crash.lap.to_string();
        // Os fatores já vêm traduzidos do scorer (`race_monitor.factor.*`). Frota sem
        // fator registrado existe — batida detectada só pela perda de velocidade —, e aí
        // a frase não pode ficar com um parêntese vazio.
        let detail = crash.factors.join(", ");
        if detail.is_empty() {
            rust_i18n::t!(
                "race_monitor.dnf.after_crash_no_detail",
                severity = severidade,
                lap = lap,
                how = how
            )
            .to_string()
        } else {
            rust_i18n::t!(
                "race_monitor.dnf.after_crash",
                severity = severidade,
                lap = lap,
                detail = detail,
                how = how
            )
            .to_string()
        }
    } else {
        // Sem batida: descreve pela evidência.
        //
        // Nenhuma destas frases pode carregar palavra de `race_signals::PALAVRAS_BATIDA`.
        // A versão anterior fechava com "(sem batida registrada)", e `dnf_kind` lê o motivo
        // por palavra quando o incidente cru não sobreviveu ao save: o abandono SEM contato
        // era classificado como batida justamente pela frase que dizia não ter havido uma.
        let mut parts: Vec<String> = Vec::new();
        if ev.disqualified {
            parts.push(rust_i18n::t!("race_monitor.dnf.evidence_disqualified").to_string());
        }
        if ev.garage {
            parts.push(rust_i18n::t!("race_monitor.dnf.evidence_garage").to_string());
        }
        if ev.off_track || ev.not_in_world {
            parts.push(rust_i18n::t!("race_monitor.dnf.evidence_off_track").to_string());
        }
        if parts.is_empty() {
            rust_i18n::t!("race_monitor.dnf.plain", how = how).to_string()
        } else {
            rust_i18n::t!(
                "race_monitor.dnf.with_evidence",
                how = how,
                parts = parts.join(", ")
            )
            .to_string()
        }
    }
}

/// A severidade da batida como o jogador a lê, no locale ATIVO.
///
/// [`Severidade::as_str`] devolve a CHAVE de domínio (`"grave"`, `"destruído"`) — ela cruza a
/// ponte, alimenta o conserto do carro e a percepção de rivalidade, e não é texto de tela.
/// Chamar `.to_uppercase()` nela era o que punha "DESTRUÍDO" dentro de uma frase em inglês.
pub(crate) fn severidade_visivel(sev: Severidade) -> String {
    let chave = match sev {
        Severidade::Nenhum => "race_monitor.severity.nenhum",
        Severidade::Leve => "race_monitor.severity.leve",
        Severidade::Moderado => "race_monitor.severity.moderado",
        Severidade::Grave => "race_monitor.severity.grave",
        Severidade::Destruido => "race_monitor.severity.destruido",
        Severidade::Catastrofico => "race_monitor.severity.catastrofico",
    };
    rust_i18n::t!(chave).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iracing_sdk::race_monitor::{Attempt, AttemptEvidence, CrashEvent, StatusTentativa};

    fn tentativa(crashes: Vec<CrashEvent>, evidence: AttemptEvidence) -> Attempt {
        Attempt {
            number: 1,
            status: StatusTentativa::Active,
            started_at_session_time: 0.0,
            laps_completed: 7,
            ended_by: None,
            reason: None,
            worst_crash: None,
            evidence,
            crashes,
            peak_crash_score: 0.0,
            collided_with_car_number: None,
            peak_impact_dir: None,
            sim_repair_needed_s: 0.0,
            sim_repair_required_s: 0.0,
            style: crate::car::driving_style::StyleAccumulator::new(),
        }
    }

    fn batida(severity: Severidade, factors: Vec<String>) -> CrashEvent {
        CrashEvent {
            session_time: 0.0,
            lap: 7,
            score: 9.0,
            severity,
            impact_severity: severity,
            had_impact: true,
            factors,
        }
    }

    /// O motivo do abandono não pode sair em português cru quando o jogo está em inglês.
    /// Serial porque `rust_i18n` guarda o locale no PROCESSO — sem isso este caso contamina
    /// todo teste que asserta prosa em PT.
    #[test]
    #[serial_test::serial]
    fn o_motivo_do_abandono_sai_no_locale_ativo() {
        let com_batida = tentativa(
            vec![batida(Severidade::Grave, vec!["impacto 4.2g".to_string()])],
            AttemptEvidence::default(),
        );
        let mut ev_saida = AttemptEvidence::default();
        ev_saida.off_track = true;
        let sem_batida = tentativa(Vec::new(), ev_saida);
        let seco = tentativa(Vec::new(), AttemptEvidence::default());

        rust_i18n::set_locale("pt-BR");
        let pt_batida =
            build_dnf_reason(&com_batida, &com_batida.evidence, FimDaTentativa::Restart);
        let pt_saida =
            build_dnf_reason(&sem_batida, &sem_batida.evidence, FimDaTentativa::SimClosed);
        let pt_seco = build_dnf_reason(&seco, &seco.evidence, FimDaTentativa::SimClosed);

        rust_i18n::set_locale("en-US");
        let en_batida =
            build_dnf_reason(&com_batida, &com_batida.evidence, FimDaTentativa::Restart);
        let en_saida =
            build_dnf_reason(&sem_batida, &sem_batida.evidence, FimDaTentativa::SimClosed);
        let en_seco = build_dnf_reason(&seco, &seco.evidence, FimDaTentativa::SimClosed);
        rust_i18n::set_locale("pt-BR");

        // Nenhuma frase pode sair como a própria chave (linha faltando no locale).
        for frase in [
            &pt_batida, &pt_saida, &pt_seco, &en_batida, &en_saida, &en_seco,
        ] {
            assert!(
                !frase.contains("race_monitor."),
                "chave sem tradução vazou para o jogador: {frase}"
            );
        }

        // E os dois idiomas produzem textos DIFERENTES — se saíssem iguais, o `t!` não estaria
        // resolvendo e o inglês continuaria lendo português.
        assert_ne!(pt_batida, en_batida);
        assert_ne!(pt_saida, en_saida);
        assert_ne!(pt_seco, en_seco);

        // A severidade é traduzida, e não a chave de domínio crua.
        assert!(pt_batida.contains("GRAVE"), "{pt_batida}");
        assert!(en_batida.contains("HEAVY"), "{en_batida}");
        // O detalhe da batida e a volta continuam na frase.
        assert!(
            pt_batida.contains("4.2g") && pt_batida.contains('7'),
            "{pt_batida}"
        );
    }

    /// `race_signals::dnf_kind` classifica o abandono por PALAVRA quando o incidente cru não
    /// sobreviveu ao save. A frase de batida precisa carregar uma palavra de batida nos dois
    /// idiomas, e as frases SEM batida não podem carregar nenhuma — era esse o defeito de
    /// "(sem batida registrada)", que fazia o abandono sem contato ser lido como batida.
    #[test]
    #[serial_test::serial]
    fn a_classificacao_do_abandono_le_a_frase_certa() {
        use crate::race_signals::{dnf_kind, DnfKind};

        let com_batida = tentativa(
            vec![batida(Severidade::Grave, vec!["impacto 4.2g".to_string()])],
            AttemptEvidence::default(),
        );
        let mut ev_saida = AttemptEvidence::default();
        ev_saida.disqualified = true;
        ev_saida.garage = true;
        let sem_batida = tentativa(Vec::new(), ev_saida);
        let seco = tentativa(Vec::new(), AttemptEvidence::default());

        for locale in ["pt-BR", "en-US"] {
            rust_i18n::set_locale(locale);
            let batida_txt =
                build_dnf_reason(&com_batida, &com_batida.evidence, FimDaTentativa::Restart);
            assert_eq!(
                dnf_kind(None, false, Some(&batida_txt)),
                DnfKind::Contato,
                "{locale}: {batida_txt}"
            );
            for sem in [
                build_dnf_reason(&sem_batida, &sem_batida.evidence, FimDaTentativa::SimClosed),
                build_dnf_reason(&seco, &seco.evidence, FimDaTentativa::SimClosed),
            ] {
                assert_ne!(
                    dnf_kind(None, false, Some(&sem)),
                    DnfKind::Contato,
                    "{locale}: abandono sem contato classificado como batida: {sem}"
                );
            }
        }
        rust_i18n::set_locale("pt-BR");
    }
}
