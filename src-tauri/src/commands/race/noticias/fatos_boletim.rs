//! Curadoria dos FATOS que alimentam o boletim de IA: lesões, pano de fundo dos
//! pilotos em destaque, histórico de pista, recordes, campeonato, rivalidades,
//! desempenho e as quebras de peça. Devolve `(fatos de lesão, fatos de contexto,
//! beats de carreira)` — os beats já vêm pesados e disputam a hierarquia do boletim
//! junto com os da corrida; os "fatos de contexto" continuam sendo cor sem peso.

use super::super::*;

pub(super) fn montar_fatos_do_boletim(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    active_season: &Season,
    round: i32,
    category_id: &str,
    flat_incidents: &[IncidentResult],
    new_injuries: &[Injury],
    extra_context_facts: &[String],
) -> (Vec<String>, Vec<String>, Vec<crate::narrative::Beat>) {
    use crate::db::queries::drivers as driver_queries;

    // Lesões ocorridas nesta corrida → viram fatos do boletim (nome resolvido).
    let injury_facts: Vec<String> = new_injuries
        .iter()
        .map(|inj| {
            let name = driver_queries::get_driver(conn, &inj.pilot_id)
                .map(|d| d.nome)
                .unwrap_or_else(|_| inj.pilot_id.clone());
            rust_i18n::t!("briefing.ctx.injury", name = name.as_str()).to_string()
        })
        .collect();

    // Contexto de carreira (pano de fundo) dos pilotos em DESTAQUE: vencedor,
    // pódio (2º/3º), maior recuperação e o nosso piloto. Atributos do piloto +
    // histórico de pista — sem dependência de ordem de inserção.
    let mut context_facts: Vec<String> = Vec::new();
    let mut featured: Vec<String> = vec![race_result.winner_id.clone()];
    for d in &race_result.race_results {
        if !d.is_dnf && (d.finish_position == 2 || d.finish_position == 3) {
            featured.push(d.pilot_id.clone());
        }
    }
    if let Some(id) = &race_result.most_positions_gained_id {
        featured.push(id.clone());
    }
    if let Some(p) = race_result.race_results.iter().find(|d| d.is_jogador) {
        featured.push(p.pilot_id.clone());
    }
    featured.sort();
    featured.dedup();

    for pilot_id in &featured {
        let driver = match driver_queries::get_driver(conn, pilot_id) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let is_winner = *pilot_id == race_result.winner_id;

        // Rookie em destaque → valoriza a estreia. Veterano → só o vencedor (evita poluir).
        if driver.temporadas_na_categoria == 0 {
            context_facts.push(
                rust_i18n::t!("briefing.ctx.rookie_debut", name = driver.nome.as_str()).to_string(),
            );
        } else if is_winner && driver.temporadas_na_categoria >= 5 {
            context_facts.push(
                rust_i18n::t!(
                    "briefing.ctx.veteran",
                    name = driver.nome.as_str(),
                    season = driver.temporadas_na_categoria + 1
                )
                .to_string(),
            );
        }

        // Histórico de pista: já abandonou aqui antes? (gosto de superação — só
        // para quem TERMINOU hoje, senão seria o abandono desta própria corrida).
        let dnfd_this_race = race_result
            .race_results
            .iter()
            .any(|d| d.pilot_id == *pilot_id && d.is_dnf);
        if !dnfd_this_race {
            if let Ok(Some(_)) = crate::db::queries::track_history::get_pilot_dnf_at_track(
                conn,
                pilot_id,
                &race_result.track_name,
            ) {
                context_facts.push(
                    rust_i18n::t!(
                        "briefing.ctx.overcame_dnf_here",
                        name = driver.nome.as_str()
                    )
                    .to_string(),
                );
            }
        }
    }

    // Grava os DNFs desta corrida no histórico por pista — SÓ AGORA (depois de
    // ler os abandonos ANTERIORES acima), senão o abandono de hoje contaria como
    // "visita anterior" e a narrativa de superação dispararia errado. Camada
    // narrativa, não factual: erro (ex.: reprocessar a mesma etapa) é silencioso.
    let _ = crate::db::queries::track_history::record_race_dnfs(
        conn,
        &race_result.race_results,
        &race_result.track_name,
        active_season.numero,
        round,
    );

    // Recordes e marcos da categoria (todas as temporadas) — peso histórico.
    super::recordes::empurrar_fatos_de_recordes(
        conn,
        race_result,
        active_season,
        round,
        category_id,
        &featured,
        &mut context_facts,
    );

    // Duelo interno: quem levou a melhor sobre o companheiro de equipe.
    super::campeonato::empurrar_duelo_interno(race_result, &mut context_facts);

    // Quadro do campeonato: o que o resultado significa para a temporada.
    super::campeonato::empurrar_quadro_do_campeonato(
        conn,
        race_result,
        active_season,
        round,
        category_id,
        &featured,
        &mut context_facts,
    );

    // --- Arco de rivalidade (a "novela"): registra o capítulo de hoje no log de
    // episódios e recapitula o arco para os destaques que se cruzaram na pista. ---
    record_rivalry_episodes(
        conn,
        race_result,
        flat_incidents,
        category_id,
        round,
        active_season.numero,
        active_season.ano,
    );
    let career_beats = rivalry_arc_beats(conn, race_result, &featured, active_season.numero, round);

    // Desempenho e forma: esperado×real, forma recente, histórico de pista e
    // confronto entre companheiros (pano de fundo dos destaques).
    for fact in performance_context_facts(
        conn,
        race_result,
        &featured,
        active_season,
        round,
        category_id,
    ) {
        context_facts.push(fact);
    }

    // Telemetria REAL do SDK (só corrida importada do iRacing): ritmo, duelo,
    // erro mais caro, melhor momento — cor sobre a corrida do próprio jogador.
    for fact in extra_context_facts {
        context_facts.push(fact.clone());
    }

    // Peça 3 · notícia: PENALIDADES de quebra (não-DNF) — "perdeu tempo arrumando a peça X,
    // problema leve/grave". Os DNFs de quebra já entram pelo beat Abandono (Camada B); aqui
    // entram as paradas `!black`. Vazio no sim offline (só corrida ao vivo dispara quebra).
    let race_id_for_breakdowns =
        crate::db::queries::calendar::get_calendar(conn, &active_season.id, category_id)
            .ok()
            .and_then(|entries| {
                entries
                    .into_iter()
                    .find(|e| e.rodada == round)
                    .map(|e| e.id)
            });
    if let Some(rid) = &race_id_for_breakdowns {
        if let Ok(bds) = crate::db::queries::race_breakdowns::get_breakdowns_for_race(conn, rid) {
            let mut count = 0;
            for b in bds.iter().filter(|b| b.severity != "dnf") {
                if count >= 6 {
                    break;
                }
                let Some(dr) = race_result
                    .race_results
                    .iter()
                    .find(|d| d.pilot_id == b.driver_id)
                else {
                    continue;
                };
                let part_name = crate::car::PartType::from_str(&b.part)
                    .map(|pt| pt.display_name(category_id).to_string())
                    .unwrap_or_else(|| b.part.clone());
                let grav = if b.severity == "heavy" {
                    rust_i18n::t!("briefing.ctx.severity_heavy")
                } else {
                    rust_i18n::t!("briefing.ctx.severity_light")
                };
                context_facts.push(
                    rust_i18n::t!(
                        "briefing.ctx.breakdown_pit",
                        name = dr.pilot_name.as_str(),
                        team = dr.team_name.as_str(),
                        secs = b.penalty_secs.unwrap_or(0),
                        part = part_name.as_str(),
                        label = b.label.as_str(),
                        severity = grav
                    )
                    .to_string(),
                );
                count += 1;
            }
        }
    }

    (injury_facts, context_facts, career_beats)
}
