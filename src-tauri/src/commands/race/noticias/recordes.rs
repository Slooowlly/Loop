//! Recordes e marcas históricas da categoria virando FATO NARRATIVO do boletim:
//! sequência de vitórias, caça a rival, recorde superado, maior vencedor da
//! equipe e os números redondos dos pilotos em destaque. A gravação dos marcos
//! fica em [`super::marcos`].

use super::super::*;

pub(super) fn empurrar_fatos_de_recordes(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    active_season: &Season,
    round: i32,
    category_id: &str,
    featured: &[String],
    context_facts: &mut Vec<String>,
) {
    use crate::db::queries::drivers as driver_queries;

    let winner_id = &race_result.winner_id;
    let winner_name = driver_queries::get_driver(conn, winner_id)
        .map(|d| d.nome)
        .unwrap_or_else(|_| winner_id.clone());
    let records = crate::db::queries::race_history::get_category_records(conn, category_id).ok();

    // Sequência de vitórias do vencedor (feito em destaque).
    if let Ok(streak) = crate::db::queries::race_history::get_win_streak(
        conn,
        winner_id,
        &active_season.id,
        category_id,
    ) {
        if streak >= 3 {
            context_facts.push(
                rust_i18n::t!(
                    "briefing.ctx.win_streak",
                    name = winner_name.as_str(),
                    n = streak
                )
                .to_string(),
            );
        }
    }

    // Carreira do vencedor na categoria (para caça a rival e recorde batido).
    let winner_career =
        crate::db::queries::race_history::get_driver_category_career(conn, winner_id, category_id)
            .ok();

    // Caça a um rival que AINDA está no grid: vencedor a poucas vitórias de
    // igualar alguém logo acima dele no total histórico da categoria.
    if let Some(wc) = &winner_career {
        if let Ok(actives) =
            crate::db::queries::race_history::get_active_category_win_counts(conn, category_id)
        {
            let target = actives
                .iter()
                .filter(|a| a.pilot_id != *winner_id && a.value > wc.wins && a.value - wc.wins <= 3)
                .min_by_key(|a| a.value - wc.wins);
            if let Some(t) = target {
                let diff = t.value - wc.wins;
                let plural = if diff == 1 {
                    rust_i18n::t!("briefing.ctx.win_singular")
                } else {
                    rust_i18n::t!("briefing.ctx.win_plural")
                };
                context_facts.push(
                    rust_i18n::t!(
                        "briefing.ctx.chasing_rival",
                        wins = wc.wins,
                        name = winner_name.as_str(),
                        diff = diff,
                        word = plural,
                        target = t.pilot_name.as_str(),
                        value = t.value
                    )
                    .to_string(),
                );
            }
        }
    }

    // Recorde de vitórias da categoria SUPERADO hoje: o vencedor passou a marca
    // anterior (era o 2º+1). Diz há quanto tempo a marca resistia, sem nomear o
    // dono anterior. Só vale se a marca era antiga (>= 2 anos) e não-trivial.
    if let (Some(recs), Some(wc)) = (records.as_ref(), winner_career.as_ref()) {
        let is_new_record = recs
            .most_wins
            .as_ref()
            .map_or(false, |m| m.pilot_id == *winner_id && m.value == wc.wins);
        if is_new_record && wc.wins >= 3 && recs.second_most_wins == Some(wc.wins - 1) {
            if let Ok(Some(year)) = crate::db::queries::race_history::first_year_reaching_wins(
                conn,
                category_id,
                wc.wins - 1,
            ) {
                let dur = active_season.ano - year;
                if dur >= 2 {
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.new_win_record",
                            name = winner_name.as_str(),
                            years = dur
                        )
                        .to_string(),
                    );
                }
            }
        }
    }

    // Vencedor é quem mais venceu pela própria equipe na categoria.
    if let Some(cur) = race_result
        .race_results
        .iter()
        .find(|d| d.pilot_id == *winner_id)
    {
        if let Ok(Some(top)) = crate::db::queries::race_history::get_team_top_winner_in_category(
            conn,
            &cur.team_id,
            category_id,
        ) {
            if top.pilot_id == *winner_id && top.value >= 2 {
                context_facts.push(
                    rust_i18n::t!(
                        "briefing.ctx.team_top_winner",
                        name = winner_name.as_str(),
                        team = cur.team_name.as_str(),
                        wins = top.value
                    )
                    .to_string(),
                );
            }
        }
    }

    super::marcos::registrar_marcos(
        conn,
        race_result,
        active_season,
        round,
        category_id,
        records.as_ref(),
        winner_id,
        &winner_name,
    );

    // Por destaque: o RECORDISTA aparece sempre que está em evidência (descreve
    // quem ele é, independe do resultado de hoje). Marcos de número redondo só
    // para quem REALMENTE fez aquilo hoje (venceu / subiu ao pódio / largou).
    for pilot_id in featured {
        let is_winner = pilot_id == winner_id;
        let is_player = race_result
            .race_results
            .iter()
            .any(|d| d.pilot_id == *pilot_id && d.is_jogador);
        let Ok(career) = crate::db::queries::race_history::get_driver_category_career(
            conn,
            pilot_id,
            category_id,
        ) else {
            continue;
        };
        let name = driver_queries::get_driver(conn, pilot_id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| pilot_id.clone());
        let recs = records.as_ref();
        let holds = |rec: fn(
            &crate::db::queries::race_history::CategoryRecords,
        )
            -> &Option<crate::db::queries::race_history::CategoryRecord>| {
            recs.and_then(|r| rec(r).as_ref())
                .map_or(false, |m| m.pilot_id == *pilot_id)
        };
        let is_wins_record = holds(|r| &r.most_wins);
        let is_podiums_record = holds(|r| &r.most_podiums);
        let is_starts_record = holds(|r| &r.most_starts);

        // Recordes históricos da categoria (estado — vale sempre que aparece).
        if is_wins_record {
            context_facts.push(
                rust_i18n::t!(
                    "briefing.ctx.record_wins",
                    name = name.as_str(),
                    wins = career.wins
                )
                .to_string(),
            );
        }
        if is_podiums_record {
            context_facts.push(
                rust_i18n::t!(
                    "briefing.ctx.record_podiums",
                    name = name.as_str(),
                    podiums = career.podiums
                )
                .to_string(),
            );
        }
        if is_starts_record {
            context_facts.push(
                rust_i18n::t!(
                    "briefing.ctx.record_starts",
                    name = name.as_str(),
                    starts = career.starts
                )
                .to_string(),
            );
        }

        // Marcos de número redondo — só vencedor e jogador, e só se fez hoje.
        if is_winner || is_player {
            let podium_today = race_result.race_results.iter().any(|d| {
                d.pilot_id == *pilot_id && !d.is_dnf && (1..=3).contains(&d.finish_position)
            });
            if is_winner && !is_wins_record && [5, 10, 25, 50, 75, 100].contains(&career.wins) {
                context_facts.push(
                    rust_i18n::t!(
                        "briefing.ctx.nth_win",
                        n = career.wins,
                        name = name.as_str()
                    )
                    .to_string(),
                );
            }
            if podium_today && !is_podiums_record && [25, 50, 100, 150].contains(&career.podiums) {
                context_facts.push(
                    rust_i18n::t!(
                        "briefing.ctx.nth_podium",
                        name = name.as_str(),
                        n = career.podiums
                    )
                    .to_string(),
                );
            }
            if !is_starts_record && [50, 100, 150, 200, 250].contains(&career.starts) {
                context_facts.push(
                    rust_i18n::t!(
                        "briefing.ctx.nth_start",
                        n = career.starts,
                        name = name.as_str()
                    )
                    .to_string(),
                );
            }
        }
    }
}
