//! Notas de RECORDE: os recém-quebrados (com data) e os que estão a caminho.

use super::*;

/// Notas de RECORDE recém-quebrado (com data), a partir dos marcos persistidos
/// (`milestones`). Só marcos RECENTES — desta temporada ou da anterior — para não
/// ressuscitar recorde velho. Uma nota por piloto.
pub(super) fn record_broken_notes(
    conn: &rusqlite::Connection,
    categoria: &str,
    current_season: i32,
    used_drivers: &mut HashSet<String>,
    budget: usize,
) -> Vec<WorldNote> {
    use crate::db::queries::milestones;

    let mut out = Vec::new();
    if budget == 0 {
        return out;
    }
    let Ok(recent) = milestones::get_recent_milestones(conn, categoria, 5) else {
        return out;
    };
    for m in recent {
        if out.len() >= budget {
            break;
        }
        // Recente = quebrado nesta temporada ou na imediatamente anterior.
        if current_season > 0 && current_season - m.season_number > 1 {
            continue;
        }
        if !used_drivers.insert(m.pilot_id.clone()) {
            continue;
        }
        let name = m.pilot_name.as_str();
        let ctx = m.context.as_str();
        let text = match m.metric.as_str() {
            "lap_record" => rust_i18n::t!(
                "world_footer.record_broken.lap_record",
                name = name,
                context = ctx,
                lap = format_lap_ms(m.value),
                prev = m.previous_value.map(format_lap_ms).unwrap_or_default()
            )
            .to_string(),
            "comeback" => rust_i18n::t!(
                "world_footer.record_broken.comeback",
                name = name,
                value = m.value
            )
            .to_string(),
            "season_wins" => match m.previous_value {
                Some(prev) => rust_i18n::t!(
                    "world_footer.record_broken.season_wins_prev",
                    name = name,
                    value = m.value,
                    prev = prev
                )
                .to_string(),
                None => rust_i18n::t!(
                    "world_footer.record_broken.season_wins",
                    name = name,
                    value = m.value
                )
                .to_string(),
            },
            "track_wins" => rust_i18n::t!(
                "world_footer.record_broken.track_wins",
                name = name,
                context = ctx,
                value = m.value
            )
            .to_string(),
            "win_streak" => rust_i18n::t!(
                "world_footer.record_broken.win_streak",
                name = name,
                value = m.value
            )
            .to_string(),
            "constructor_titles" => rust_i18n::t!(
                "world_footer.record_broken.constructor_titles",
                name = name,
                ord = ord_label(m.value, false)
            )
            .to_string(),
            "team_wins" => rust_i18n::t!(
                "world_footer.record_broken.team_wins",
                name = name,
                value = m.value
            )
            .to_string(),
            "one_two" => rust_i18n::t!(
                "world_footer.record_broken.one_two",
                name = name,
                ord = ord_label(m.value, true)
            )
            .to_string(),
            "youngest_winner" => rust_i18n::t!(
                "world_footer.record_broken.youngest_winner",
                name = name,
                value = m.value
            )
            .to_string(),
            "oldest_winner" => rust_i18n::t!(
                "world_footer.record_broken.oldest_winner",
                name = name,
                value = m.value
            )
            .to_string(),
            "youngest_champion" => rust_i18n::t!(
                "world_footer.record_broken.youngest_champion",
                name = name,
                value = m.value
            )
            .to_string(),
            "most_chaotic_race" => rust_i18n::t!(
                "world_footer.record_broken.most_chaotic_race",
                name = name,
                value = m.value
            )
            .to_string(),
            "drought_broken" => rust_i18n::t!(
                "world_footer.record_broken.drought_broken",
                name = name,
                value = m.value
            )
            .to_string(),
            "closest_championship" => {
                if m.value == 0 {
                    rust_i18n::t!(
                        "world_footer.record_broken.closest_championship_tie",
                        name = name
                    )
                    .to_string()
                } else {
                    rust_i18n::t!(
                        "world_footer.record_broken.closest_championship",
                        name = name,
                        value = m.value
                    )
                    .to_string()
                }
            }
            "biggest_blowout" => rust_i18n::t!(
                "world_footer.record_broken.biggest_blowout",
                name = name,
                value = m.value
            )
            .to_string(),
            "longest_pairing" => rust_i18n::t!(
                "world_footer.record_broken.longest_pairing",
                name = name,
                value = m.value
            )
            .to_string(),
            "most_starts_no_win" => rust_i18n::t!(
                "world_footer.record_broken.most_starts_no_win",
                name = name,
                context = ctx,
                value = m.value
            )
            .to_string(),
            "most_career_dnfs" => rust_i18n::t!(
                "world_footer.record_broken.most_career_dnfs",
                name = name,
                context = ctx,
                value = m.value
            )
            .to_string(),
            "most_poles_no_title" => rust_i18n::t!(
                "world_footer.record_broken.most_poles_no_title",
                name = name,
                context = ctx,
                value = m.value
            )
            .to_string(),
            "most_career_points" => rust_i18n::t!(
                "world_footer.record_broken.most_career_points",
                name = name,
                context = ctx
            )
            .to_string(),
            _ => {
                // O substantivo concorda com o VALOR do recorde citado no texto
                // (`%{value}`), não com um plural fixo: recorde de 1 vira "1 vitória".
                let noun = metric_noun(metric_noun_id(&m.metric), m.value);
                match m.previous_value {
                    Some(prev) => rust_i18n::t!(
                        "world_footer.record_broken.generic_prev",
                        name = name,
                        noun = noun,
                        value = m.value,
                        prev = prev
                    )
                    .to_string(),
                    None => rust_i18n::t!(
                        "world_footer.record_broken.generic",
                        name = name,
                        noun = noun,
                        value = m.value
                    )
                    .to_string(),
                }
            }
        };
        out.push(WorldNote {
            id: format!("broken:{}:{}:{}", categoria, m.metric, m.value),
            tag: tag_label("record"),
            subject: m.pilot_name,
            kind: "recorde_quebrado".to_string(),
            tone: "recorde".to_string(),
            text,
        });
    }
    out
}

/// Notas de RECORDE a caminho: pilotos ATIVOS da categoria a até `RECORD_GAP_MAX` de
/// igualar um recorde histórico (vitórias, pódios ou largadas). Mais próximos primeiro,
/// uma nota por piloto. É o fallback quando não há assunto de mercado/bastidores.
pub(super) fn record_watch_notes(
    conn: &rusqlite::Connection,
    categoria: &str,
    used_drivers: &mut HashSet<String>,
    budget: usize,
) -> Vec<WorldNote> {
    use crate::db::queries::{drivers, race_history};

    let mut out = Vec::new();
    if budget == 0 {
        return out;
    }
    let Ok(records) = race_history::get_category_records(conn, categoria) else {
        return out;
    };
    let Ok(field) = drivers::get_drivers_by_category(conn, categoria) else {
        return out;
    };

    // (gap, driver_id, nome, texto) — coleta e ordena por proximidade.
    let mut cands: Vec<(i32, String, String, String)> = Vec::new();
    for d in &field {
        if d.status != DriverStatus::Ativo {
            continue;
        }
        let Ok(career) = race_history::get_driver_category_career(conn, &d.id, categoria) else {
            continue;
        };
        // (recorde, valor do piloto, substantivo)
        let metrics: [(&Option<race_history::CategoryRecord>, i32, &str); 3] = [
            (&records.most_wins, career.wins, "wins"),
            (&records.most_podiums, career.podiums, "podiums"),
            (&records.most_starts, career.starts, "starts"),
        ];
        // Uma métrica por piloto: a mais próxima do recorde.
        let mut best: Option<(i32, String)> = None;
        for (rec, val, noun_id) in metrics {
            let Some(r) = rec else { continue };
            if r.pilot_id == d.id {
                continue; // já é o recordista.
            }
            let gap = r.value - val;
            if !(0..=RECORD_GAP_MAX).contains(&gap) {
                continue;
            }
            let text = if gap == 0 {
                rust_i18n::t!(
                    "world_footer.record_watch.tied",
                    // Concorda com `%{value}` (o recorde igualado), não com um plural
                    // fixo — recorde de 1 lê "o recorde histórico de vitória: 1".
                    name = d.nome.as_str(),
                    noun = metric_noun(noun_id, r.value),
                    value = r.value,
                    holder = r.pilot_name.as_str()
                )
                .to_string()
            } else {
                rust_i18n::t!(
                    "world_footer.record_watch.approaching",
                    name = d.nome.as_str(),
                    gap = gap,
                    noun = metric_noun(noun_id, gap),
                    value = r.value,
                    holder = r.pilot_name.as_str()
                )
                .to_string()
            };
            if best.as_ref().map_or(true, |(bg, _)| gap < *bg) {
                best = Some((gap, text));
            }
        }
        if let Some((gap, text)) = best {
            cands.push((gap, d.id.clone(), d.nome.clone(), text));
        }
    }

    cands.sort_by_key(|(gap, _, _, _)| *gap);
    for (_, driver_id, nome, text) in cands {
        if out.len() >= budget {
            break;
        }
        if !used_drivers.insert(driver_id.clone()) {
            continue;
        }
        out.push(WorldNote {
            id: format!("record:{driver_id}"),
            tag: tag_label("record"),
            // `subject` é o NOME de quem a nota fala (igual às demais notas), não uma
            // segunda cópia do texto: o front usa este campo como rótulo curto.
            subject: nome,
            kind: "recorde_a_caminho".to_string(),
            tone: "recorde".to_string(),
            text,
        });
    }
    out
}
