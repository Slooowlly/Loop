//! A tese dominante do debrief pós-corrida (eixo + blocos promovidos ao apoio).

/// Duelo pessoal DECIDIDO na corrida (nemesis ou rival de pista) — usado quando o
/// resultado em si foi morno mas o confronto direto é a história do dia.
pub(crate) struct PostRaceDuel {
    pub(crate) name: String,
    pub(crate) player_won: bool,
    pub(crate) is_nemesis: bool,
    pub(crate) h2h: Option<(i32, i32)>,
}

/// Sinais destilados da corrida que alimentam a tese do debrief.
pub(crate) struct PostRaceSignals {
    pub(crate) is_dnf: bool,
    pub(crate) dnf_mechanical: bool,
    pub(crate) grid: i32,
    pub(crate) finish: i32,
    pub(crate) positions_gained: i32,
    pub(crate) has_fastest_lap: bool,
    pub(crate) assessment: Option<crate::race_eval::Assessment>,
    pub(crate) target_low: i32,
    pub(crate) target_high: i32,
    pub(crate) duel: Option<PostRaceDuel>,
    pub(crate) track_name: String,
}

/// A TESE DOMINANTE do debrief pós-corrida. Mesmo princípio da prévia (nextRaceThesis.js):
/// em vez de despejar todos os blocos achatados e deixar o servidor adivinhar QUAL foi a
/// história da corrida, elegemos UM eixo e organizamos o resto em APOIO/PANO DE FUNDO.
/// Semeado pelo cérebro `race_eval` (assessment/nota/meta) + o evento de destaque (DNF
/// mecânico vs erro, remontada, colapso, vitória, over/under, ou um duelo decidido).
/// Devolve (statement do eixo, ids de bloco promovidos ao APOIO). `resultado` e `pre_race`
/// são sempre promovidos pelo chamador (resultado é o núcleo; pre_race fecha o loop).
pub(crate) fn select_post_race_thesis(s: &PostRaceSignals) -> (String, Vec<&'static str>) {
    use crate::race_signals as sinais;
    let track = &s.track_name;
    let overperf = sinais::overperf(s.assessment);
    let underperf = sinais::underperf(s.assessment);

    // 1) DNF mecânico — o carro falhou, não foi erro seu.
    if s.is_dnf && s.dnf_mechanical {
        return (
            rust_i18n::t!("ai_news.thesis.mechanical_dnf", track = track.as_str()).to_string(),
            vec!["breakdowns", "maintenance"],
        );
    }
    // 2) DNF por incidente/contato — fim precoce na pista.
    if s.is_dnf {
        return (
            rust_i18n::t!("ai_news.thesis.incident_dnf", track = track.as_str()).to_string(),
            vec![],
        );
    }
    // 3) Vitória — a manchete é a própria vitória.
    if s.finish == 1 {
        let fl = if s.has_fastest_lap {
            rust_i18n::t!("ai_news.thesis.win_fastest").to_string()
        } else {
            String::new()
        };
        return (
            rust_i18n::t!("ai_news.thesis.win", track = track.as_str(), fl = fl).to_string(),
            vec!["telemetry", "lived_rivalry", "champ_rival"],
        );
    }
    // 4) Remontada — ganhou muitas posições e não ficou abaixo da meta.
    if sinais::remontada(s.positions_gained) && !underperf {
        return (
            rust_i18n::t!(
                "ai_news.thesis.comeback",
                grid = s.grid,
                finish = s.finish,
                gained = s.positions_gained
            )
            .to_string(),
            vec!["telemetry", "eval", "lived_rivalry"],
        );
    }
    // 5) Colapso — perdeu muitas posições, ou ficou abaixo da meta largando bem.
    if sinais::colapso(s.positions_gained) || (underperf && s.grid <= s.target_low) {
        return (
            rust_i18n::t!("ai_news.thesis.collapse", grid = s.grid, finish = s.finish).to_string(),
            vec!["eval", "telemetry"],
        );
    }
    // 6) Acima do esperado (entrega além do conjunto).
    if overperf {
        return (
            rust_i18n::t!(
                "ai_news.thesis.overperform",
                finish = s.finish,
                low = s.target_low,
                high = s.target_high
            )
            .to_string(),
            vec!["eval", "telemetry"],
        );
    }
    // 7) Aquém do esperado (sem drama de abandono).
    if underperf {
        return (
            rust_i18n::t!(
                "ai_news.thesis.underperform",
                finish = s.finish,
                low = s.target_low,
                high = s.target_high
            )
            .to_string(),
            vec!["eval", "telemetry"],
        );
    }
    // 8) Resultado morno, mas um DUELO pessoal foi decidido → ele é a história.
    if let Some(d) = &s.duel {
        let verbo = if d.player_won {
            rust_i18n::t!("ai_news.thesis.duel_won")
        } else {
            rust_i18n::t!("ai_news.thesis.duel_lost")
        };
        let quem = if d.is_nemesis {
            rust_i18n::t!("ai_news.thesis.duel_nemesis")
        } else {
            rust_i18n::t!("ai_news.thesis.duel_rival")
        };
        let h2h = d
            .h2h
            .map(|(p, r)| rust_i18n::t!("ai_news.thesis.duel_h2h", p = p, r = r).to_string())
            .unwrap_or_default();
        return (
            rust_i18n::t!(
                "ai_news.thesis.duel",
                verb = verbo,
                who = quem,
                name = d.name.as_str(),
                h2h = h2h
            )
            .to_string(),
            vec!["lived_rivalry", "champ_rival"],
        );
    }
    // 9) Dia de somar — dentro do esperado, sem grande drama.
    (
        rust_i18n::t!("ai_news.thesis.points_day", finish = s.finish).to_string(),
        vec!["eval", "champ_rival"],
    )
}
