//! Marcos (milestones) batidos na corrida: recordes de vitória, pódio, pole,
//! volta mais rápida, recuperação, escalares (idade/jejum/caos) e as coroas
//! cumulativas. Só grava — os fatos narrativos ficam em [`super::recordes`].

use super::super::*;

pub(super) fn registrar_marcos(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    active_season: &Season,
    round: i32,
    category_id: &str,
    records: Option<&crate::db::queries::race_history::CategoryRecords>,
    winner_id: &str,
    winner_name: &str,
) {
    use crate::db::queries::drivers as driver_queries;

    // Memória temporal dos recordes (vitórias e pódios): registra QUANDO o
    // recorde all-time da categoria é batido, para notícias de "recorde
    // quebrado" com data e o rodapé do mundo. Condição: o recordista PONTUOU
    // hoje na métrica (então o recorde avançou nesta corrida) e é dono ISOLADO
    // do topo (2º colocado == valor-1). `previous = valor-1` (cada corrida soma
    // no máximo 1). Pisos evitam marcos triviais. Idempotente por valor.
    if let Some(recs) = records {
        let record_milestone = |metric: &str, r: &crate::db::queries::race_history::CategoryRecord| {
            let _ = crate::db::queries::milestones::insert_milestone(
                conn,
                category_id,
                &crate::db::queries::milestones::RecordMilestone {
                    metric: metric.to_string(),
                    pilot_id: r.pilot_id.clone(),
                    pilot_name: r.pilot_name.clone(),
                    value: r.value,
                    previous_value: Some(r.value - 1),
                    context: String::new(),
                    season_number: active_season.numero,
                    ano: active_season.ano,
                    round,
                },
            );
        };
        // Vitórias: o recordista é o vencedor de hoje e abriu a marca.
        if let Some(r) = recs.most_wins.as_ref() {
            if r.pilot_id == winner_id
                && r.value >= 5
                && recs.second_most_wins == Some(r.value - 1)
            {
                record_milestone("wins", r);
            }
        }
        // Pódios: o recordista subiu ao pódio hoje e abriu a marca.
        if let Some(r) = recs.most_podiums.as_ref() {
            let podium_today = race_result.race_results.iter().any(|d| {
                d.pilot_id == r.pilot_id && !d.is_dnf && (1..=3).contains(&d.finish_position)
            });
            if podium_today
                && r.value >= 10
                && recs.second_most_podiums == Some(r.value - 1)
            {
                record_milestone("podiums", r);
            }
        }
        // Poles: o recordista fez a pole de hoje e abriu a marca.
        if let Some(r) = recs.most_poles.as_ref() {
            if r.pilot_id == race_result.pole_sitter_id
                && r.value >= 5
                && recs.second_most_poles == Some(r.value - 1)
            {
                record_milestone("poles", r);
            }
        }
    }

    // Recordes de VITÓRIA do vencedor de hoje: mais vitórias numa temporada,
    // dono da pista (mais vitórias no circuito) e maior sequência de vitórias.
    {
        use crate::db::queries::{milestones, race_history};
        let win_milestone =
            |metric: &str, value: i32, previous: Option<i32>, context: String| {
                let _ = milestones::insert_milestone(
                    conn,
                    category_id,
                    &milestones::RecordMilestone {
                        metric: metric.to_string(),
                        pilot_id: winner_id.to_string(),
                        pilot_name: winner_name.to_string(),
                        value,
                        previous_value: previous,
                        context,
                        season_number: active_season.numero,
                        ano: active_season.ano,
                        round,
                    },
                );
            };

        // (a) Mais vitórias numa única temporada (recorde de `standings`, que só
        // tem temporadas encerradas → a atual não entra e não conta duplicado).
        if let Ok(season_wins) = race_history::get_category_wins_this_season(
            conn,
            winner_id,
            &active_season.id,
            category_id,
        ) {
            let prev = race_history::get_category_single_season_win_record(conn, category_id)
                .ok()
                .flatten()
                .map(|r| r.value);
            if season_wins >= 5 && prev.map_or(true, |p| season_wins > p) {
                win_milestone("season_wins", season_wins, prev, String::new());
            }
        }

        // (b) Dono da pista: vencedor virou o maior vencedor isolado do circuito.
        if let Ok(track_wins) = race_history::get_pilot_track_wins(
            conn,
            winner_id,
            category_id,
            &race_result.track_name,
        ) {
            let others = race_history::get_track_win_leader_excluding(
                conn,
                category_id,
                &race_result.track_name,
                winner_id,
            )
            .unwrap_or(0);
            if track_wins >= 3 && track_wins > others {
                win_milestone(
                    "track_wins",
                    track_wins,
                    Some(track_wins - 1),
                    race_result.track_name.clone(),
                );
            }
        }

        // (c) Maior sequência de vitórias (na temporada). O "recorde atual" vive
        // nos próprios marcos → só anuncia quando supera o maior já registrado.
        if let Ok(streak) =
            race_history::get_win_streak(conn, winner_id, &active_season.id, category_id)
        {
            let streak = streak as i32;
            let prev = milestones::get_max_milestone_value(conn, category_id, "win_streak")
                .ok()
                .flatten();
            if streak >= 4 && prev.map_or(true, |p| streak > p) {
                win_milestone("win_streak", streak, prev, String::new());
            }
        }

        // (d) Maior VENCEDORA da história — a EQUIPE do vencedor de hoje virou a
        // dona isolada do recorde de vitórias da categoria. Guarda a equipe nos
        // campos pilot_* (o rodapé trata a métrica `team_wins` como time).
        if let Some(w) = race_result
            .race_results
            .iter()
            .find(|d| d.pilot_id == winner_id)
        {
            let team_wins =
                crate::db::queries::teams::get_team_category_wins(conn, &w.team_id, category_id)
                    .unwrap_or(0);
            let others = crate::db::queries::teams::get_category_team_win_leader_excluding(
                conn,
                category_id,
                &w.team_id,
            )
            .unwrap_or(0);
            if team_wins >= 5 && team_wins > others {
                let _ = milestones::insert_milestone(
                    conn,
                    category_id,
                    &milestones::RecordMilestone {
                        metric: "team_wins".to_string(),
                        pilot_id: w.team_id.clone(),
                        pilot_name: w.team_name.clone(),
                        value: team_wins,
                        previous_value: Some(team_wins - 1),
                        context: String::new(),
                        season_number: active_season.numero,
                        ano: active_season.ano,
                        round,
                    },
                );
            }

            // (e) Recorde de DOBRADINHAS (1-2): só conta quando HOJE foi uma
            // dobradinha da equipe do vencedor (ele em 1º + outro carro em 2º).
            let one_two_today = race_result.race_results.iter().any(|d| {
                d.team_id == w.team_id && !d.is_dnf && d.finish_position == 2
            });
            if one_two_today {
                let count = crate::db::queries::teams::get_team_category_one_two(
                    conn,
                    &w.team_id,
                    category_id,
                )
                .unwrap_or(0);
                let others = crate::db::queries::teams::get_category_one_two_leader_excluding(
                    conn,
                    category_id,
                    &w.team_id,
                )
                .unwrap_or(0);
                if count >= 3 && count > others {
                    let _ = milestones::insert_milestone(
                        conn,
                        category_id,
                        &milestones::RecordMilestone {
                            metric: "one_two".to_string(),
                            pilot_id: w.team_id.clone(),
                            pilot_name: w.team_name.clone(),
                            value: count,
                            previous_value: Some(count - 1),
                            context: String::new(),
                            season_number: active_season.numero,
                            ano: active_season.ano,
                            round,
                        },
                    );
                }
            }
        }
    }

    // Recorde de VOLTA da pista: o tempo de volta não é histórico, então o
    // recorde vive em `track_lap_records` (atualizado a cada corrida). Compara a
    // volta mais rápida de HOJE (em memória) com o recorde guardado; se for mais
    // rápida (ou não houver recorde), atualiza. Só emite MARCO quando SUPERA um
    // recorde existente — o inaugural fica guardado em silêncio.
    if !race_result.fastest_lap_id.is_empty() {
        if let Some(fl) = race_result
            .race_results
            .iter()
            .find(|d| d.pilot_id == race_result.fastest_lap_id && d.best_lap_time_ms > 0.0)
        {
            let lap_ms = fl.best_lap_time_ms.round() as i32;
            let prev = crate::db::queries::milestones::get_track_lap_record(
                conn,
                category_id,
                &race_result.track_name,
            )
            .ok()
            .flatten();
            let is_record = prev.as_ref().map_or(true, |(_, _, pms)| lap_ms < *pms);
            if is_record {
                let _ = crate::db::queries::milestones::upsert_track_lap_record(
                    conn,
                    category_id,
                    &race_result.track_name,
                    &fl.pilot_id,
                    &fl.pilot_name,
                    lap_ms,
                    active_season.numero,
                    round,
                );
                if let Some((_, _, pms)) = prev {
                    let _ = crate::db::queries::milestones::insert_milestone(
                        conn,
                        category_id,
                        &crate::db::queries::milestones::RecordMilestone {
                            metric: "lap_record".to_string(),
                            pilot_id: fl.pilot_id.clone(),
                            pilot_name: fl.pilot_name.clone(),
                            value: lap_ms,
                            previous_value: Some(pms),
                            context: race_result.track_name.clone(),
                            season_number: active_season.numero,
                            ano: active_season.ano,
                            round,
                        },
                    );
                }
            }
        }
    }

    // Maior RECUPERAÇÃO da categoria numa corrida (o sim já aponta quem mais
    // ganhou posições hoje). Compara com o recorde histórico ANTES de hoje; só
    // anuncia quando SUPERA um recorde existente (inaugural fica só no histórico).
    if let Some(gain_id) = race_result.most_positions_gained_id.as_ref() {
        if let Some(cur) = race_result
            .race_results
            .iter()
            .find(|d| d.pilot_id == *gain_id && !d.is_dnf && d.grid_position > 0)
        {
            let gained = cur.grid_position - cur.finish_position;
            if gained >= 6 {
                let prev = crate::db::queries::race_history::get_category_comeback_record(
                    conn,
                    category_id,
                    &active_season.id,
                    round,
                )
                .ok()
                .flatten();
                if let Some(p) = prev {
                    if gained > p.value {
                        let _ = crate::db::queries::milestones::insert_milestone(
                            conn,
                            category_id,
                            &crate::db::queries::milestones::RecordMilestone {
                                metric: "comeback".to_string(),
                                pilot_id: cur.pilot_id.clone(),
                                pilot_name: cur.pilot_name.clone(),
                                value: gained,
                                previous_value: Some(p.value),
                                context: String::new(),
                                season_number: active_season.numero,
                                ano: active_season.ano,
                                round,
                            },
                        );
                    }
                }
            }
        }
    }

    // Recordes escalares (idade/jejum/caóticos) e "de azar" (coroas cumulativas).
    {
        use crate::db::queries::{milestones, race_history};

        // Emite um marco escalar quando o candidato supera o recorde existente.
        let scalar = |kind: &str,
                      subj_id: &str,
                      subj_name: &str,
                      value: i32,
                      context: &str,
                      higher: bool| {
            if let Ok(Some(prev)) = milestones::update_scalar_and_check(
                conn,
                category_id,
                kind,
                subj_id,
                subj_name,
                value,
                context,
                active_season.numero,
                round,
                higher,
            ) {
                let _ = milestones::insert_milestone(
                    conn,
                    category_id,
                    &milestones::RecordMilestone {
                        metric: kind.to_string(),
                        pilot_id: subj_id.to_string(),
                        pilot_name: subj_name.to_string(),
                        value,
                        previous_value: Some(prev),
                        context: context.to_string(),
                        season_number: active_season.numero,
                        ano: active_season.ano,
                        round,
                    },
                );
            }
        };

        // Piloto mais jovem / mais velho a vencer (idade do vencedor hoje).
        let winner_age = driver_queries::get_driver(conn, winner_id)
            .map(|d| d.idade as i32)
            .unwrap_or(0);
        if winner_age > 0 {
            scalar("youngest_winner", winner_id, winner_name, winner_age, "", false);
            scalar("oldest_winner", winner_id, winner_name, winner_age, "", true);
        }

        // Corrida mais caótica da história (mais abandonos numa etapa).
        if race_result.total_dnfs > 0 {
            scalar(
                "most_chaotic_race",
                &race_result.track_name,
                &race_result.track_name,
                race_result.total_dnfs,
                &race_result.track_name,
                true,
            );
        }

        // Maior jejum quebrado: o vencedor voltou a vencer após anos sem ganhar.
        if let Ok(Some(prev_win)) = race_history::get_pilot_previous_win_season(
            conn,
            winner_id,
            category_id,
            active_season.numero,
            round,
        ) {
            let drought = active_season.numero - prev_win;
            if drought >= 3 {
                scalar("drought_broken", winner_id, winner_name, drought, "", true);
            }
        }

        // "De azar" (coroas que trocam de dono): azarão, batedor, poleiro sem
        // título, maior pontuador. Só anuncia quando o dono muda e passa do piso.
        let crown = |kind: &str, leader: Option<race_history::CategoryRecord>, floor: i32| {
            if let Some(l) = leader {
                if let Ok(Some((prev_name, prev_val))) =
                    milestones::update_leader_and_check_crown(
                        conn,
                        category_id,
                        kind,
                        &l.pilot_id,
                        &l.pilot_name,
                        l.value,
                        floor,
                        active_season.numero,
                        round,
                    )
                {
                    let _ = milestones::insert_milestone(
                        conn,
                        category_id,
                        &milestones::RecordMilestone {
                            metric: kind.to_string(),
                            pilot_id: l.pilot_id,
                            pilot_name: l.pilot_name,
                            value: l.value,
                            previous_value: Some(prev_val),
                            context: prev_name,
                            season_number: active_season.numero,
                            ano: active_season.ano,
                            round,
                        },
                    );
                }
            }
        };
        crown(
            "most_starts_no_win",
            race_history::get_category_most_starts_no_win(conn, category_id).ok().flatten(),
            30,
        );
        crown(
            "most_career_dnfs",
            race_history::get_category_most_career_dnfs(conn, category_id).ok().flatten(),
            20,
        );
        crown(
            "most_poles_no_title",
            race_history::get_category_most_poles_no_title(conn, category_id).ok().flatten(),
            5,
        );
        crown(
            "most_career_points",
            race_history::get_category_most_career_points(conn, category_id).ok().flatten(),
            300,
        );
    }
}
