//! O que a corrida significou para a temporada: o duelo com o companheiro de
//! equipe e o quadro do campeonato (reta final, brigas de pilotos e equipes,
//! e o arrependimento de quem trocou de equipe).

use super::super::*;

/// Duelo interno: quem levou a melhor sobre o companheiro de equipe.
pub(super) fn empurrar_duelo_interno(race_result: &RaceResult, context_facts: &mut Vec<String>) {
    // Só para o vencedor e o jogador (foco), lendo o próprio resultado — o "carro
    // irmão" é a referência mais justa da corrida do piloto. Par deduplicado.
    let mut focus: Vec<&str> = vec![race_result.winner_id.as_str()];
    if let Some(p) = race_result.race_results.iter().find(|d| d.is_jogador) {
        focus.push(p.pilot_id.as_str());
    }
    let mut seen_pairs: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    for pid in focus {
        let Some(me) = race_result.race_results.iter().find(|d| d.pilot_id == pid) else {
            continue;
        };
        if me.team_id.is_empty() {
            continue;
        }
        let Some(mate) = race_result
            .race_results
            .iter()
            .find(|d| d.team_id == me.team_id && d.pilot_id != me.pilot_id)
        else {
            continue;
        };
        let key = if me.pilot_id <= mate.pilot_id {
            (me.pilot_id.clone(), mate.pilot_id.clone())
        } else {
            (mate.pilot_id.clone(), me.pilot_id.clone())
        };
        if !seen_pairs.insert(key) {
            continue;
        }
        // Quem terminou à frente: melhor posição, ou o único a completar.
        let me_ahead = match (me.is_dnf, mate.is_dnf) {
            (false, true) => true,
            (true, false) => false,
            (false, false) => me.finish_position < mate.finish_position,
            (true, true) => continue, // ambos fora → sem duelo interno
        };
        let (ahead, team, behind) = if me_ahead {
            (&me.pilot_name, &me.team_name, &mate.pilot_name)
        } else {
            (&mate.pilot_name, &mate.team_name, &me.pilot_name)
        };
        context_facts.push(
            rust_i18n::t!(
                "briefing.ctx.internal_duel",
                team = team.as_str(),
                ahead = ahead.as_str(),
                behind = behind.as_str()
            )
            .to_string(),
        );
    }
}

/// Quadro do campeonato: o que o resultado significa para a temporada.
pub(super) fn empurrar_quadro_do_campeonato(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    active_season: &Season,
    round: i32,
    category_id: &str,
    featured: &[String],
    context_facts: &mut Vec<String>,
) {
    // Os resultados desta corrida já estão em `race_results` (persistidos em
    // simulate_category_race, antes daqui), então os standings já a incluem.
    let total_rodadas = crate::constants::categories::get_category_config(category_id)
        .map(|c| c.corridas_por_temporada as i32)
        .unwrap_or(round);
    let races_left = (total_rodadas - round).max(0);

    // "Valor de uma vitória" nesta categoria = pontos do vencedor desta corrida.
    // Vira o limiar de "briga apertada" sem depender da escala de pontos: um gap
    // menor que isso é recuperável numa única corrida → a disputa segue viva.
    let win_value = race_result
        .race_results
        .iter()
        .find(|d| d.pilot_id == race_result.winner_id)
        .map(|d| d.points_earned)
        .unwrap_or(0)
        .max(1) as f64;

    // Reta final / próxima é a decisiva.
    match races_left {
        0 => context_facts.push(rust_i18n::t!("briefing.ctx.season_last").to_string()),
        1 => context_facts.push(rust_i18n::t!("briefing.ctx.season_one_left").to_string()),
        2 => context_facts.push(rust_i18n::t!("briefing.ctx.season_two_left").to_string()),
        // "Reta final" só faz sentido quando a temporada de fato já passou da
        // metade. Numa temporada curta (ex.: 5 etapas), 4 restantes significa
        // que só a 1ª rodada foi disputada — não é reta final.
        n if n <= 4 && round * 2 > total_rodadas => context_facts
            .push(rust_i18n::t!("briefing.ctx.season_final_stretch", n = n).to_string()),
        _ => {}
    }

    // Brigas no campeonato só fazem sentido depois de algumas corridas (gap real).
    if round >= 2 {
        // Pilotos: título em aberto (P1×P2) OU, com o líder encaminhado, o vice (P2×P3).
        if let Ok(st) = crate::db::queries::race_history::get_category_standings(
            conn,
            &active_season.id,
            category_id,
        ) {
            if st.len() >= 2 {
                let gap12 = (st[0].points - st[1].points).round();
                if gap12 <= win_value {
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.title_fight",
                            leader = st[0].pilot_name.as_str(),
                            gap = gap12 as i32,
                            second = st[1].pilot_name.as_str()
                        )
                        .to_string(),
                    );
                } else if st.len() >= 3 {
                    let gap23 = (st[1].points - st[2].points).round();
                    if gap23 <= win_value {
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.vice_fight",
                                leader = st[0].pilot_name.as_str(),
                                second = st[1].pilot_name.as_str(),
                                third = st[2].pilot_name.as_str(),
                                gap = gap23 as i32
                            )
                            .to_string(),
                        );
                    }
                }
            }
        }

        // Equipes: mesma lógica (ponta OU vice).
        if let Ok(ts) = crate::db::queries::race_history::get_team_standings(
            conn,
            &active_season.id,
            category_id,
        ) {
            if ts.len() >= 2 {
                let gap12 = (ts[0].points - ts[1].points).round();
                if gap12 <= win_value {
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.teams_top_fight",
                            a = ts[0].team_name.as_str(),
                            b = ts[1].team_name.as_str(),
                            gap = gap12 as i32
                        )
                        .to_string(),
                    );
                } else if ts.len() >= 3 {
                    let gap23 = (ts[1].points - ts[2].points).round();
                    if gap23 <= win_value {
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.teams_vice_fight",
                                a = ts[1].team_name.as_str(),
                                b = ts[2].team_name.as_str(),
                                gap = gap23 as i32
                            )
                            .to_string(),
                        );
                    }
                }
            }
        }
    }

    // Final da temporada: piloto em destaque que trocou de equipe e foi parar
    // num time que terminou ATRÁS do que ele deixou — "a aposta saiu cara".
    // Só na última etapa, e só na mesma categoria (troca lateral, não promoção).
    if races_left == 0 {
        if let Ok(ts) = crate::db::queries::race_history::get_team_standings(
            conn,
            &active_season.id,
            category_id,
        ) {
            let pos_of = |team_id: &str| {
                ts.iter().find(|t| t.team_id == team_id).map(|t| t.position)
            };
            let prev_season = active_season.numero - 1;

            // Candidatos (jogador tem prioridade; senão, a virada mais dramática).
            struct SwitchRegret {
                pilot_name: String,
                old_team: String,
                new_team: String,
                old_pos: i32,
                new_pos: i32,
                is_player: bool,
            }
            let mut candidates: Vec<SwitchRegret> = Vec::new();

            for pilot_id in featured {
                let Some(cur) = race_result
                    .race_results
                    .iter()
                    .find(|d| d.pilot_id == *pilot_id)
                else {
                    continue;
                };
                let contracts = match crate::db::queries::contracts::get_contracts_for_pilot(
                    conn, pilot_id,
                ) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                // Equipe na temporada passada, mesma categoria, time diferente do atual.
                let Some(prev) = contracts.iter().find(|c| {
                    c.categoria.as_str() == category_id
                        && c.temporada_inicio <= prev_season
                        && c.temporada_fim >= prev_season
                        && c.equipe_id != cur.team_id
                }) else {
                    continue;
                };
                if let (Some(new_pos), Some(old_pos)) =
                    (pos_of(&cur.team_id), pos_of(&prev.equipe_id))
                {
                    // O time que ele DEIXOU terminou À FRENTE do que ele escolheu.
                    if old_pos < new_pos {
                        candidates.push(SwitchRegret {
                            pilot_name: cur.pilot_name.clone(),
                            old_team: prev.equipe_nome.clone(),
                            new_team: cur.team_name.clone(),
                            old_pos,
                            new_pos,
                            is_player: cur.is_jogador,
                        });
                    }
                }
            }

            // Emite no máximo um (evita poluir): jogador primeiro, senão maior virada.
            if let Some(r) = candidates
                .iter()
                .find(|c| c.is_player)
                .or_else(|| candidates.iter().max_by_key(|c| c.new_pos - c.old_pos))
            {
                context_facts.push(
                    rust_i18n::t!(
                        "briefing.ctx.switch_regret",
                        pilot = r.pilot_name.as_str(),
                        old_team = r.old_team.as_str(),
                        new_team = r.new_team.as_str(),
                        old_pos = r.old_pos,
                        new_pos = r.new_pos
                    )
                    .to_string(),
                );
            }
        }
    }
}
