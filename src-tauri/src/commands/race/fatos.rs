//! Fatos que alimentam a narrativa do pos-corrida: episodios de rivalidade, arco das rivalidades, contexto de desempenho e leitura da telemetria.

use super::*;

/// Registra um CAPÍTULO de rivalidade por corrida em que dois rivais interagiram
/// (colisão, duelo de posições coladas, ou briga na ponta). Constrói a memória que o
/// boletim recapitula depois. As intensidades já foram atualizadas na transação da
/// corrida, então aqui só lemos o estado e gravamos o episódio.
pub(super) fn record_rivalry_episodes(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    flat_incidents: &[IncidentResult],
    categoria: &str,
    rodada: i32,
    temporada: i32,
    ano: i32,
) {
    use crate::db::queries::drivers as driver_queries;
    use crate::models::rivalry::normalize_pair;
    use crate::simulation::incidents::IncidentType;
    use std::collections::{HashMap, HashSet};

    // pilot -> (posição final, dnf)
    let mut info: HashMap<&str, (i32, bool)> = HashMap::new();
    for d in &race_result.race_results {
        info.insert(d.pilot_id.as_str(), (d.finish_position, d.is_dnf));
    }

    // pares que colidiram nesta corrida (normalizados)
    let mut collided: HashSet<(String, String)> = HashSet::new();
    for inc in flat_incidents {
        if inc.incident_type == IncidentType::Collision {
            if let Some(other) = &inc.linked_pilot_id {
                if let Some(pair) = normalize_pair(&inc.pilot_id, other) {
                    collided.insert((pair.piloto1_id, pair.piloto2_id));
                }
            }
        }
    }

    let rivalries = match crate::db::queries::rivalries::get_all_rivalries(conn) {
        Ok(r) => r,
        Err(_) => return,
    };

    for riv in rivalries {
        let Some((pos_a, dnf_a)) = info.get(riv.piloto1_id.as_str()).copied() else {
            continue;
        };
        let Some((pos_b, dnf_b)) = info.get(riv.piloto2_id.as_str()).copied() else {
            continue;
        };

        let perceived = riv.perceived_intensity();
        let pair_key = (riv.piloto1_id.clone(), riv.piloto2_id.clone());
        let did_collide = collided.contains(&pair_key);

        // Colisão sempre vira capítulo (é a origem); duelo/ponta só se já é notável.
        if !did_collide && perceived < 30.0 {
            continue;
        }

        let both_finished = !dnf_a && !dnf_b;
        let close = both_finished && (pos_a - pos_b).abs() <= 3;
        let top_front = both_finished && pos_a <= 5 && pos_b <= 5;

        let interaction = if did_collide {
            "colisao"
        } else if close {
            "duelo"
        } else if top_front {
            "campeonato"
        } else {
            continue; // sem interação de verdade hoje
        };

        // Quem levou a melhor: melhor posição, ou o único a completar.
        let winner_id = if both_finished {
            match pos_a.cmp(&pos_b) {
                std::cmp::Ordering::Less => Some(riv.piloto1_id.clone()),
                std::cmp::Ordering::Greater => Some(riv.piloto2_id.clone()),
                std::cmp::Ordering::Equal => None,
            }
        } else if !dnf_a {
            Some(riv.piloto1_id.clone())
        } else if !dnf_b {
            Some(riv.piloto2_id.clone())
        } else {
            None
        };

        let name = |id: &str| {
            driver_queries::get_driver(conn, id)
                .map(|d| d.nome)
                .unwrap_or_else(|_| id.to_string())
        };
        let na = name(&riv.piloto1_id);
        let nb = name(&riv.piloto2_id);
        let summary = match interaction {
            "colisao" => format!("contato entre {na} e {nb} em {}", race_result.track_name),
            "duelo" => match &winner_id {
                Some(w) => {
                    let (wn, wp, lp) = if *w == riv.piloto1_id {
                        (&na, pos_a, pos_b)
                    } else {
                        (&nb, pos_b, pos_a)
                    };
                    format!("{wn} levou a melhor no duelo direto, {wp}º contra {lp}º")
                }
                None => format!("duelo parelho entre {na} e {nb}"),
            },
            _ => format!("{na} e {nb} brigaram por posições de ponta ({pos_a}º e {pos_b}º)"),
        };

        let ep = crate::db::queries::rivalry_episodes::RivalryEpisode {
            piloto1_id: riv.piloto1_id.clone(),
            piloto2_id: riv.piloto2_id.clone(),
            temporada,
            rodada,
            ano,
            categoria: categoria.to_string(),
            track_name: race_result.track_name.clone(),
            interaction: interaction.to_string(),
            winner_id,
            summary,
            perceived,
        };
        let _ = crate::db::queries::rivalry_episodes::insert_episode(conn, &ep);
    }
}

/// Recapitula o ARCO de rivalidade para os destaques que se cruzaram HOJE: origem,
/// número de capítulos, retrospecto direto (h2h), o capítulo de hoje e revanche.
/// Só para rivalidades já claras (percebida ≥ 40) com capítulo registrado nesta corrida.
///
/// Devolve BEATS (com peso), não frases soltas: este é o único material do boletim com
/// MEMÓRIA entre corridas, e ele merece disputar a manchete com a vitória do dia em vez
/// de cair numa lista plana no rodapé. A curadoria final (limiar, teto, hierarquia)
/// continua sendo do `narrative` — aqui só damos o peso, que depende do banco.
pub(super) fn rivalry_arc_beats(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    featured: &[String],
    temporada: i32,
    rodada: i32,
) -> Vec<crate::narrative::Beat> {
    use crate::db::queries::drivers as driver_queries;
    use crate::models::rivalry::RivalryType;
    use std::collections::HashSet;

    let mut out: Vec<crate::narrative::Beat> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let name = |id: &str| {
        driver_queries::get_driver(conn, id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| id.to_string())
    };

    for pilot_id in featured {
        let rivs = match crate::rivalry::get_pilot_rivalries(conn, pilot_id) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for r in rivs {
            if r.perceived_intensity < 40.0 {
                continue;
            }
            // O rival precisa ter corrido hoje.
            if !race_result
                .race_results
                .iter()
                .any(|d| d.pilot_id == r.rival_id)
            {
                continue;
            }
            // Par normalizado para deduplicar (o par pode vir por ambos os lados).
            let (a, b) = if *pilot_id <= r.rival_id {
                (pilot_id.clone(), r.rival_id.clone())
            } else {
                (r.rival_id.clone(), pilot_id.clone())
            };
            if !seen.insert((a.clone(), b.clone())) {
                continue;
            }

            let eps = match crate::db::queries::rivalry_episodes::get_episodes_for_pair(conn, &a, &b)
            {
                Ok(e) => e,
                Err(_) => continue,
            };
            // Só recapitula se houve capítulo HOJE (a rivalidade se manifestou na corrida).
            let Some(today) = eps
                .last()
                .filter(|e| e.temporada == temporada && e.rodada == rodada)
            else {
                continue;
            };

            let na = name(&a);
            let nb = name(&b);
            let nivel = crate::rivalry::intensity_level(r.perceived_intensity).label();
            let chapters = eps.len();
            let ano_origem = eps.first().map(|e| e.ano).unwrap_or(0);

            // Retrospecto direto (h2h).
            let mut wins_a = 0;
            let mut wins_b = 0;
            for e in &eps {
                match e.winner_id.as_deref() {
                    Some(w) if w == a => wins_a += 1,
                    Some(w) if w == b => wins_b += 1,
                    _ => {}
                }
            }

            // Revanche: hoje X venceu e no capítulo ANTERIOR quem venceu foi o outro.
            let revenge = eps.len() >= 2
                && today.winner_id.is_some()
                && eps
                    .get(eps.len() - 2)
                    .and_then(|p| p.winner_id.as_deref())
                    .zip(today.winner_id.as_deref())
                    .map_or(false, |(prev_w, today_w)| prev_w != today_w);

            let origem = match r.tipo {
                RivalryType::Colisao => rust_i18n::t!("briefing.rivalry.origin_collision"),
                RivalryType::Companheiros => rust_i18n::t!("briefing.rivalry.origin_teammates"),
                RivalryType::Campeonato => rust_i18n::t!("briefing.rivalry.origin_championship"),
                RivalryType::Pista => rust_i18n::t!("briefing.rivalry.origin_track"),
            };

            let mut s = rust_i18n::t!(
                "briefing.rivalry.opener",
                a = na.as_str(),
                b = nb.as_str(),
                level = nivel,
                origin = origem
            )
            .to_string();
            if ano_origem > 0 && chapters > 1 {
                s.push_str(&rust_i18n::t!(
                    "briefing.rivalry.chapters",
                    chapters = chapters,
                    year = ano_origem
                ));
            }
            s.push_str(&rust_i18n::t!(
                "briefing.rivalry.today",
                summary = today.summary.as_str()
            ));
            if wins_a > 0 || wins_b > 0 {
                if wins_a == wins_b {
                    s.push_str(&rust_i18n::t!(
                        "briefing.rivalry.h2h_tied",
                        a = wins_a,
                        b = wins_b
                    ));
                } else {
                    let (leader, hi, lo) = if wins_a > wins_b {
                        (&na, wins_a, wins_b)
                    } else {
                        (&nb, wins_b, wins_a)
                    };
                    s.push_str(&rust_i18n::t!(
                        "briefing.rivalry.h2h_leader",
                        leader = leader.as_str(),
                        hi = hi,
                        lo = lo
                    ));
                }
            }
            if revenge {
                if let Some(tw) = today.winner_id.as_deref() {
                    let twn = if tw == a { &na } else { &nb };
                    s.push_str(&rust_i18n::t!("briefing.rivalry.revenge", name = twn.as_str()));
                }
            }

            // Peso na escala do `narrative` (limiar 30; vitória 70). A base 40 já passa
            // folgado — o gate de entrada é a rivalidade EXISTIR e ter tido capítulo hoje,
            // o que é raro. O que o peso decide de verdade é se ela vira manchete (≥ 60).
            let mut weight = 40.0 + (r.perceived_intensity / 5.0).min(20.0);
            if revenge {
                weight += 5.0; // reviravolta: a novela virou hoje
            }
            if chapters >= 5 {
                weight += 5.0; // história longa pesa mais que atrito recente
            }
            let both_up_front = [a.as_str(), b.as_str()].iter().all(|id| {
                race_result
                    .race_results
                    .iter()
                    .any(|d| d.pilot_id == *id && !d.is_dnf && d.finish_position <= 5)
            });
            if both_up_front {
                weight += 5.0; // brigaram onde a corrida é decidida
            }

            out.push(crate::narrative::Beat {
                kind: crate::narrative::BeatKind::RivalidadeArco,
                weight,
                text: s,
                driver_id: Some(a.clone()),
                team_name: None,
            });
        }
    }
    out
}

/// Fatos de DESEMPENHO e FORMA para os destaques, como pano de fundo do boletim:
/// 1) esperado×real (reaproveita o cérebro `race_eval`: largada + mérito do conjunto);
/// 2) forma recente (últimas 5 corridas na categoria);
/// 3) histórico no circuito (já venceu aqui);
/// 4) confronto entre companheiros de equipe.
/// Tudo gated com folga para não inundar o contexto. Lógica de DB aqui; o módulo
/// `narrative` permanece puro.
pub(super) fn performance_context_facts(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    featured: &[String],
    active_season: &Season,
    round: i32,
    category_id: &str,
) -> Vec<String> {
    use crate::db::queries::drivers as driver_queries;
    use crate::db::queries::race_history as rh;
    use crate::race_eval::{evaluate, Assessment, RaceEvalInput};
    use std::collections::HashMap;

    let mut out: Vec<String> = Vec::new();
    let rows = &race_result.race_results;
    let field_size = rows.len().max(1) as i32;
    if rows.is_empty() {
        return out;
    }

    // Carrega cada participante uma vez, para resolver nome (o mérito vem pronto de
    // `build_merit_field`).
    let mut drivers: HashMap<String, Driver> = HashMap::new();
    for d in rows {
        if let std::collections::hash_map::Entry::Vacant(e) = drivers.entry(d.pilot_id.clone()) {
            if let Ok(drv) = driver_queries::get_driver(conn, &d.pilot_id) {
                e.insert(drv);
            }
        }
    }
    let name_of = |id: &str| -> String {
        drivers
            .get(id)
            .map(|d| d.nome.clone())
            .unwrap_or_else(|| id.to_string())
    };

    // Campo de mérito — a MESMA construção que alimenta o debrief do jogador. Ler o
    // carro daqui por conta própria já custou uma incoerência: a coluna crua
    // `car_performance` vive em −5..16 e `compute_merit` espera 0–100, então o carro
    // sumia do mérito e o boletim ranqueava o grid quase só por skill.
    let field = build_merit_field(conn, race_result);

    // ── 1) Esperado×real: só sinais FORTES (muito acima / muito abaixo). ──────────
    // O pole que floppou já é coberto pelo beat de Decepção; o DNF, pelo de Abandono.
    struct ExpCand {
        text: String,
        is_player: bool,
    }
    let mut exp: Vec<ExpCand> = Vec::new();
    for pilot_id in featured {
        let Some(d) = rows.iter().find(|x| &x.pilot_id == pilot_id) else {
            continue;
        };
        if d.is_dnf {
            continue;
        }
        let ev = evaluate(&RaceEvalInput {
            player_id: pilot_id.clone(),
            grid_position: d.grid_position,
            finish_position: d.finish_position,
            is_dnf: false,
            incidents: d.incidents_count,
            field: field.clone(),
        });
        let is_pole = *pilot_id == race_result.pole_sitter_id;
        let name = name_of(pilot_id);
        let text = match ev.assessment {
            Assessment::MuitoAcima => rust_i18n::t!(
                "briefing.perf.much_above",
                name = name.as_str(),
                grid = d.grid_position,
                finish = d.finish_position
            )
            .to_string(),
            Assessment::MuitoAbaixo if !is_pole => rust_i18n::t!(
                "briefing.perf.much_below",
                name = name.as_str(),
                grid = d.grid_position,
                finish = d.finish_position
            )
            .to_string(),
            _ => continue,
        };
        exp.push(ExpCand { text, is_player: d.is_jogador });
    }
    // No máximo 2, com o jogador tendo prioridade.
    exp.sort_by_key(|c| std::cmp::Reverse(c.is_player));
    for c in exp.into_iter().take(2) {
        out.push(c.text);
    }

    // ── 2) Forma recente (últimas 5 na categoria, antes de hoje). ────────────────
    // prioridade: fim de jejum (3) > sequência de pódios (2) > reação (1).
    let mut form: Vec<(i32, String)> = Vec::new();
    for pilot_id in featured {
        let Some(d) = rows.iter().find(|x| &x.pilot_id == pilot_id) else {
            continue;
        };
        if d.is_dnf {
            continue;
        }
        let recent = rh::get_recent_finishes_before(
            conn,
            pilot_id,
            category_id,
            active_season.numero,
            round,
            5,
        )
        .unwrap_or_default();
        if recent.len() < 3 {
            continue; // pouca história → sem leitura de forma confiável
        }
        let name = name_of(pilot_id);
        let recent_wins = recent.iter().filter(|r| r.finish == 1).count();
        let last_two_podiums = recent
            .iter()
            .take(2)
            .filter(|r| !r.is_dnf && r.finish <= 3)
            .count();

        if d.finish_position == 1 && recent_wins == 0 && recent.len() >= 5 {
            form.push((
                3,
                rust_i18n::t!("briefing.perf.end_drought", name = name.as_str()).to_string(),
            ));
        } else if d.finish_position <= 3 && last_two_podiums == 2 {
            form.push((
                2,
                rust_i18n::t!("briefing.perf.podium_streak", name = name.as_str()).to_string(),
            ));
        } else if d.finish_position <= 5 {
            let valid: Vec<i32> = recent.iter().filter(|r| !r.is_dnf).map(|r| r.finish).collect();
            if valid.len() >= 3 {
                let avg = valid.iter().sum::<i32>() as f64 / valid.len() as f64;
                if avg >= field_size as f64 * 0.5 {
                    form.push((
                        1,
                        rust_i18n::t!("briefing.perf.reaction", name = name.as_str()).to_string(),
                    ));
                }
            }
        }
    }
    form.sort_by_key(|(p, _)| std::cmp::Reverse(*p));
    for (_, t) in form.into_iter().take(2) {
        out.push(t);
    }

    // ── 3) Histórico no circuito: destaque que já venceu aqui antes. ─────────────
    if let Ok(Some(track_id)) =
        rh::get_round_track_id(conn, &active_season.id, category_id, round)
    {
        let mut track_facts: Vec<(i32, String)> = Vec::new();
        for pilot_id in featured {
            let Some(d) = rows.iter().find(|x| &x.pilot_id == pilot_id) else {
                continue;
            };
            if d.is_dnf {
                continue;
            }
            let th = rh::get_pilot_track_history(conn, pilot_id, track_id, &active_season.id, round)
                .unwrap_or_default();
            if th.wins < 1 {
                continue;
            }
            let name = name_of(pilot_id);
            let vez = if th.wins == 1 {
                rust_i18n::t!("briefing.perf.time_singular")
            } else {
                rust_i18n::t!("briefing.perf.time_plural")
            };
            let text = if d.finish_position == 1 {
                rust_i18n::t!(
                    "briefing.perf.track_specialist",
                    name = name.as_str(),
                    wins = th.wins,
                    times = vez
                )
                .to_string()
            } else if d.finish_position <= 3 {
                rust_i18n::t!(
                    "briefing.perf.track_good_history",
                    name = name.as_str(),
                    wins = th.wins,
                    times = vez
                )
                .to_string()
            } else {
                continue;
            };
            track_facts.push((th.wins, text));
        }
        track_facts.sort_by_key(|(w, _)| std::cmp::Reverse(*w));
        for (_, t) in track_facts.into_iter().take(2) {
            out.push(t);
        }
    }

    // ── 4) Confronto entre companheiros: par de destaques na MESMA equipe. ───────
    // Emite no máximo 1 (jogador tem prioridade). Exige ambos classificados hoje.
    let mut h2h: Option<(bool, String)> = None;
    'pairs: for i in 0..featured.len() {
        for j in (i + 1)..featured.len() {
            let (Some(a), Some(b)) = (
                rows.iter().find(|x| x.pilot_id == featured[i]),
                rows.iter().find(|x| x.pilot_id == featured[j]),
            ) else {
                continue;
            };
            if a.team_id != b.team_id || a.is_dnf || b.is_dnf {
                continue;
            }
            // Placar do confronto interno na temporada (rodadas em que ambos completaram).
            let ra = rh::get_pilot_season_results(conn, &a.pilot_id, &active_season.id, category_id)
                .unwrap_or_default();
            let rb = rh::get_pilot_season_results(conn, &b.pilot_id, &active_season.id, category_id)
                .unwrap_or_default();
            let rb_map: HashMap<i32, (i32, bool)> =
                rb.iter().map(|(r, f, dnf)| (*r, (*f, *dnf))).collect();
            let (mut wa, mut wb) = (0, 0);
            for (rnd, fa, da) in &ra {
                if let Some((fb, db)) = rb_map.get(rnd) {
                    if *da || *db {
                        continue;
                    }
                    if fa < fb {
                        wa += 1;
                    } else if fb < fa {
                        wb += 1;
                    }
                }
            }
            let (ahead, behind) = if a.finish_position < b.finish_position {
                (a, b)
            } else {
                (b, a)
            };
            let (an, bn) = (name_of(&ahead.pilot_id), name_of(&behind.pilot_id));
            let mut s = rust_i18n::t!(
                "briefing.perf.teammate_h2h",
                ahead = an.as_str(),
                behind = bn.as_str(),
                ap = ahead.finish_position,
                bp = behind.finish_position
            )
            .to_string();
            if wa + wb >= 2 {
                if wa == wb {
                    s.push_str(&rust_i18n::t!("briefing.perf.teammate_tied", a = wa, b = wb));
                } else {
                    let (ln, hi, lo) = if wa > wb {
                        (name_of(&a.pilot_id), wa, wb)
                    } else {
                        (name_of(&b.pilot_id), wb, wa)
                    };
                    s.push_str(&rust_i18n::t!(
                        "briefing.perf.teammate_leader",
                        leader = ln.as_str(),
                        hi = hi,
                        lo = lo
                    ));
                }
            } else {
                s.push('.');
            }
            let involves_player = ahead.is_jogador || behind.is_jogador;
            if h2h.as_ref().map_or(true, |(p, _)| involves_player && !p) {
                h2h = Some((involves_player, s));
                if involves_player {
                    break 'pairs;
                }
            }
        }
    }
    if let Some((_, s)) = h2h {
        out.push(s);
    }

    out
}

/// Posição do ÚLTIMO ponto captado (≈ bandeirada) do jogador e de um rival, lida do
/// mesmo race trace que gerou os fatos de duelo. É o que impede a narrativa de tratar
/// uma ultrapassagem no meio da corrida como se fosse o desfecho.
fn desfecho_no_trace(
    charts: &crate::iracing_sdk::telemetry_analysis::RaceCharts,
    rival_name: &str,
) -> Option<(i32, i32)> {
    let ultima = |c: &crate::iracing_sdk::telemetry_analysis::ChartCar| {
        c.points.last().map(|p| p.position).filter(|p| *p > 0)
    };
    let jogador = charts.cars.iter().find(|c| c.is_player).and_then(ultima)?;
    let rival = charts
        .cars
        .iter()
        .find(|c| !c.is_player && c.name == rival_name)
        .and_then(ultima)?;
    if jogador == rival {
        return None;
    }
    Some((jogador, rival))
}

/// Converte a TELEMETRIA REAL do SDK (ritmo, duelo, erro mais caro, melhor momento)
/// em fatos de pano de fundo sobre a corrida do JOGADOR — a cor que só existe quando
/// ele correu de verdade no iRacing. O jogador é CITADO (subtrama), nunca protagonista;
/// estes fatos entram na seção "Contexto" e a IA tece quando fizer sentido. Tolerante:
/// cada item só sai se o sinal for confiável (o motor de telemetria já gateia isso).
pub(super) fn telemetry_context_facts(
    telemetry: &crate::iracing_sdk::telemetry_analysis::TelemetryAnalysis,
    player_name: &str,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if !telemetry.has_telemetry || player_name.trim().is_empty() {
        return out;
    }
    let who = player_name;

    if let Some(p) = &telemetry.pace {
        // Ritmo vs campo (só se a amostra do grid for confiável).
        if p.vs_grid_reliable {
            let delta_s = (p.vs_grid_ms.abs() / 1000.0 * 10.0).round() / 10.0;
            if p.vs_grid_ms <= -200.0 {
                out.push(
                    rust_i18n::t!(
                        "briefing.tel.faster_than_grid",
                        who = who,
                        delta = format!("{delta_s:.1}")
                    )
                    .to_string(),
                );
            } else if p.vs_grid_ms >= 200.0 {
                out.push(
                    rust_i18n::t!(
                        "briefing.tel.slower_than_grid",
                        who = who,
                        delta = format!("{delta_s:.1}")
                    )
                    .to_string(),
                );
            }
        }
        // Consistência (só com voltas suficientes).
        if p.consistency_reliable && p.total_laps >= 4 {
            let ratio = p.good_laps as f64 / p.total_laps as f64;
            if ratio >= 0.85 {
                out.push(
                    rust_i18n::t!(
                        "briefing.tel.consistent",
                        who = who,
                        good = p.good_laps,
                        total = p.total_laps
                    )
                    .to_string(),
                );
            } else if ratio <= 0.5 {
                out.push(
                    rust_i18n::t!(
                        "briefing.tel.inconsistent",
                        who = who,
                        good = p.good_laps,
                        total = p.total_laps
                    )
                    .to_string(),
                );
            }
        }
    }

    // Duelo com o rival mais constante.
    if let Some(r) = &telemetry.rival {
        if !r.pilot_name.trim().is_empty() {
            let gap = (r.avg_gap_s * 10.0).round() / 10.0;
            out.push(
                rust_i18n::t!(
                    "briefing.tel.duel",
                    who = who,
                    laps = r.laps_battled,
                    rival = r.pilot_name.as_str(),
                    gap = format!("{gap:.1}")
                )
                .to_string(),
            );
            // O duelo só significa alguma coisa com o desfecho junto: sem isto a IA
            // lê "duelo + ultrapassagem" e escreve que o jogador deixou o rival para
            // trás, mesmo quando o rival cruzou a linha na frente.
            if let Some((eu, dele)) =
                telemetry.charts.as_ref().and_then(|c| desfecho_no_trace(c, &r.pilot_name))
            {
                let key = if eu < dele {
                    "briefing.tel.duel_outcome_ahead"
                } else {
                    "briefing.tel.duel_outcome_behind"
                };
                out.push(
                    rust_i18n::t!(
                        key,
                        who = who,
                        rival = r.pilot_name.as_str(),
                        you = eu,
                        rival_pos = dele
                    )
                    .to_string(),
                );
            }
        }
    }

    // Melhor momento da corrida do jogador.
    if let Some(b) = &telemetry.best_moment {
        let phrase = match b.kind.as_str() {
            // "Levou a melhor" só vale se a passagem tiver sobrevivido até a bandeirada;
            // se o rival terminou na frente, o fato vira a passagem NÃO sustentada.
            "rival_beaten" if !b.rival_name.trim().is_empty() => {
                let perdeu_depois = telemetry
                    .charts
                    .as_ref()
                    .and_then(|c| desfecho_no_trace(c, &b.rival_name))
                    .is_some_and(|(eu, dele)| eu > dele);
                let key = if perdeu_depois {
                    "briefing.tel.best_rival_passed_not_held"
                } else {
                    "briefing.tel.best_rival_beaten"
                };
                Some(rust_i18n::t!(key, who = who, rival = b.rival_name.as_str()).to_string())
            }
            "position_gain" if b.positions_gained >= 1 => Some(
                rust_i18n::t!(
                    "briefing.tel.best_position_gain",
                    who = who,
                    n = b.positions_gained
                )
                .to_string(),
            ),
            "recovery" => {
                Some(rust_i18n::t!("briefing.tel.best_recovery", who = who).to_string())
            }
            "clean_streak" if b.streak >= 3 => Some(
                rust_i18n::t!("briefing.tel.best_clean_streak", who = who, n = b.streak)
                    .to_string(),
            ),
            _ => None,
        };
        if let Some(phrase) = phrase {
            out.push(phrase);
        }
    }

    // Erro mais caro (DNF não entra: o beat de Abandono já cobre).
    if let Some(m) = &telemetry.mistake {
        let phrase = match m.kind.as_str() {
            "incident" => Some(
                rust_i18n::t!(
                    "briefing.tel.mistake_incident",
                    who = who,
                    lap = m.lap,
                    n = m.positions_lost.max(0)
                )
                .to_string(),
            ),
            "position_loss" if m.positions_lost >= 1 => Some(
                rust_i18n::t!(
                    "briefing.tel.mistake_position_loss",
                    who = who,
                    n = m.positions_lost,
                    lap = m.lap
                )
                .to_string(),
            ),
            "pace_drop" if m.time_lost_ms >= 1500.0 => Some(
                rust_i18n::t!(
                    "briefing.tel.mistake_pace_drop",
                    who = who,
                    lap = m.lap,
                    secs = format!("{:.0}", m.time_lost_ms / 1000.0)
                )
                .to_string(),
            ),
            _ => None,
        };
        if let Some(phrase) = phrase {
            out.push(phrase);
        }
    }

    // ── Bandeira amarela REAL (SessionFlags do SDK) ──────────────────────────
    // `yellow_laps` são as voltas do LÍDER em que a corrida esteve sob amarela.
    // Voltas consecutivas viram UM acionamento: é assim que a corrida é vivida e
    // narrada, não como uma lista solta de voltas. Só corrida importada tem isto —
    // no sim offline a amarela não é modelada, então este bloco fica vazio.
    if let Some(charts) = &telemetry.charts {
        let mut yellow = charts.yellow_laps.clone();
        yellow.retain(|l| *l > 0);
        yellow.sort_unstable();
        yellow.dedup();
        if let Some(&first) = yellow.first() {
            let periods = 1 + yellow.windows(2).filter(|w| w[1] - w[0] > 1).count();
            let key = if periods > 1 {
                "briefing.tel.yellow_multi"
            } else {
                "briefing.tel.yellow_single"
            };
            out.push(
                rust_i18n::t!(key, periods = periods, laps = yellow.len(), first = first)
                    .to_string(),
            );
        }
    }

    out
}
