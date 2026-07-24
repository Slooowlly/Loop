//! Geracao das noticias do pos-corrida: importancia da pauta, boletim da corrida do jogador, pre-aquecimento do texto por IA e as notas das demais categorias.

use super::*;

pub(super) fn race_news_importance(
    bias: i32,
    tier: &InterestTier,
    finish_position: i32,
) -> crate::news::NewsImportance {
    use crate::event_interest::InterestTier;
    use crate::news::NewsImportance;
    let tier_score = match tier {
        InterestTier::Baixo => 0,
        InterestTier::Moderado => 1,
        InterestTier::Alto => 2,
        InterestTier::MuitoAlto => 3,
        InterestTier::EventoPrincipal => 4,
    };
    let position_bonus = if finish_position == 1 {
        2
    } else if finish_position <= 3 {
        1
    } else {
        0
    };
    let total = bias + tier_score + position_bonus;
    let importance = if total >= 5 {
        NewsImportance::Destaque
    } else if total >= 3 {
        NewsImportance::Alta
    } else if total >= 1 {
        NewsImportance::Media
    } else {
        NewsImportance::Baixa
    };
    // Vitória sempre dispara pelo menos Alta para que detect_race_trigger acione LeaderWon/ShockWin/etc.
    if finish_position == 1 && matches!(importance, NewsImportance::Baixa | NewsImportance::Media) {
        NewsImportance::Alta
    } else {
        importance
    }
}

pub(super) fn persist_race_news(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    active_season: &Season,
    round: i32,
    category_id: &str,
    news_importance_bias: i32,
    _thematic_slot: crate::models::enums::ThematicSlot,
    interest_tier: &InterestTier,
    flat_incidents: &[IncidentResult],
    new_injuries: &[Injury],
    // Fatos extras de pano de fundo (ex.: telemetria REAL do SDK numa corrida
    // importada do iRacing). Vazio no fluxo simulado. Entram na seção "Contexto".
    extra_context_facts: &[String],
) -> Result<Option<String>, String> {
    use crate::db::queries::news as news_queries;
    use crate::generators::ids::{next_id, IdType};
    use crate::news::{NewsImportance, NewsItem, NewsType};

    use crate::db::queries::drivers as driver_queries;

    let now = chrono::Local::now().timestamp();
    let mut items: Vec<NewsItem> = Vec::new();
    // Id da notícia de Corrida do jogador — usado para atrelar os fatos do boletim de IA.
    let mut corrida_news_id: Option<String> = None;

    // 1. Corrida — notícia sobre o VENCEDOR da corrida (não o jogador)
    // O sistema editorial foi projetado para compor histórias sobre quem ganhou.
    // A importância Alta garante que detect_race_trigger gera algo além do FallbackRaceResult.
    {
        let winner_id = &race_result.winner_id;
        let winner_name = driver_queries::get_driver(conn, winner_id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| winner_id.clone());
        let importance = race_news_importance(news_importance_bias, interest_tier, 1);

        let total_rodadas = crate::constants::categories::get_category_config(category_id)
            .map(|c| c.corridas_por_temporada as i32)
            .unwrap_or(round);
        let fallback_races = total_rodadas - round;

        let track = race_result.track_name.as_str();
        let (titulo, texto) = if fallback_races == 0 {
            (
                rust_i18n::t!("race.news.win_final_title", name = winner_name, track = track).to_string(),
                rust_i18n::t!("race.news.win_final_text", name = winner_name, season = active_season.numero).to_string(),
            )
        } else if fallback_races <= 2 {
            (
                rust_i18n::t!("race.news.win_crucial_title", name = winner_name, track = track).to_string(),
                rust_i18n::t!("race.news.win_crucial_text", name = winner_name, round = round).to_string(),
            )
        } else {
            (
                rust_i18n::t!("race.news.win_title", name = winner_name, track = track).to_string(),
                rust_i18n::t!(
                    "race.news.win_text",
                    name = winner_name,
                    round = round,
                    season = active_season.numero
                )
                .to_string(),
            )
        };

        let winner_team = race_result
            .race_results
            .iter()
            .find(|r| &r.pilot_id == winner_id)
            .map(|r| r.team_id.clone());
        let id = next_id(conn, IdType::News).map_err(|e| format!("next_id news: {e:?}"))?;
        corrida_news_id = Some(id.clone());
        items.push(NewsItem {
            id,
            tipo: NewsType::Corrida,
            icone: NewsType::Corrida.icone().to_string(),
            titulo,
            texto,
            rodada: Some(round),
            semana_pretemporada: None,
            temporada: active_season.numero,
            categoria_id: Some(category_id.to_string()),
            categoria_nome: None,
            importancia: importance,
            timestamp: now,
            driver_id: Some(winner_id.clone()),
            driver_id_secondary: None,
            team_id: winner_team.map(Some).unwrap_or(None),
        });

        if fallback_races == 0 {
            if let Ok(standings) = crate::db::queries::race_history::get_category_standings(
                conn,
                &active_season.id,
                category_id,
            ) {
                if let Some(champion) = standings.into_iter().next() {
                    let champ_id =
                        next_id(conn, IdType::News).unwrap_or_else(|_| "news_champ".to_string());
                    items.push(NewsItem {
                        id: champ_id,
                        tipo: NewsType::FramingSazonal,
                        icone: NewsType::FramingSazonal.icone().to_string(),
                        titulo: rust_i18n::t!("race.news.champion_title", name = champion.pilot_name.as_str(), season = active_season.numero).to_string(),
                        texto: rust_i18n::t!("race.news.champion_text", rounds = total_rodadas, name = champion.pilot_name.as_str()).to_string(),
                        rodada: Some(round),
                        semana_pretemporada: None,
                        temporada: active_season.numero,
                        categoria_id: Some(category_id.to_string()),
                        categoria_nome: None,
                        importancia: NewsImportance::Destaque,
                        timestamp: now,
                        driver_id: Some(champion.pilot_id),
                        driver_id_secondary: None,
                        team_id: None,
                    });
                }
            }
        }
    }

    // 2. Incidentes — um item por DNF + incidentes de hint >= 2 não-DNF
    // Evita duplicatas: se um piloto já tem DNF, não gera segundo item por hint >= 2 dele.
    let mut seen_incident_pilots: HashSet<String> = HashSet::new();
    let mut noticiable: Vec<&IncidentResult> = flat_incidents
        .iter()
        .filter(|i| i.is_dnf || i.narrative_importance_hint >= 2)
        .collect();
    // DNFs primeiro, depois por hint decrescente
    noticiable.sort_by_key(|i| {
        (
            std::cmp::Reverse(i.is_dnf as u8),
            std::cmp::Reverse(i.narrative_importance_hint),
        )
    });

    for inc in noticiable {
        if !seen_incident_pilots.insert(inc.pilot_id.clone()) {
            continue; // piloto já tem notícia nesta rodada
        }
        let driver_name = driver_queries::get_driver(conn, &inc.pilot_id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| inc.pilot_id.clone());
        let id = next_id(conn, IdType::News).map_err(|e| format!("next_id incident: {e:?}"))?;
        let titulo = if inc.is_dnf {
            format!("{} abandona a corrida após incidente", driver_name)
        } else {
            format!("{} envolvido em incidente durante a prova", driver_name)
        };
        let texto = inc.description.clone();
        let inc_importance = if inc.narrative_importance_hint >= 3 {
            NewsImportance::Destaque
        } else {
            NewsImportance::Alta
        };
        items.push(NewsItem {
            id,
            tipo: NewsType::Incidente,
            icone: NewsType::Incidente.icone().to_string(),
            titulo,
            texto,
            rodada: Some(round),
            semana_pretemporada: None,
            temporada: active_season.numero,
            categoria_id: Some(category_id.to_string()),
            categoria_nome: None,
            importancia: inc_importance,
            timestamp: now,
            driver_id: Some(inc.pilot_id.clone()),
            driver_id_secondary: inc.linked_pilot_id.clone(),
            team_id: None,
        });
    }

    // 3. Lesão — uma notícia por piloto lesionado
    for injury in new_injuries {
        let driver_name = driver_queries::get_driver(conn, &injury.pilot_id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| injury.pilot_id.clone());
        let id = next_id(conn, IdType::News).map_err(|e| format!("next_id injury: {e:?}"))?;
        let titulo = "desfalque confirmado".to_string();
        let texto = format!(
            "{} está fora da próxima etapa após lesão confirmada. Situação será reavaliada nos próximos dias.",
            driver_name
        );
        items.push(NewsItem {
            id,
            tipo: NewsType::Lesao,
            icone: NewsType::Lesao.icone().to_string(),
            titulo,
            texto,
            rodada: Some(round),
            semana_pretemporada: None,
            temporada: active_season.numero,
            categoria_id: Some(category_id.to_string()),
            categoria_nome: None,
            importancia: NewsImportance::Alta,
            timestamp: now,
            driver_id: Some(injury.pilot_id.clone()),
            driver_id_secondary: None,
            team_id: None,
        });
    }

    if !items.is_empty() {
        news_queries::insert_news_batch(conn, &items)
            .map_err(|e| format!("insert_news_batch: {e:?}"))?;
    }

    // Boletim de IA (teste via simulação): monta os fatos curados da corrida do
    // jogador e os guarda atrelados à notícia de Corrida, para o comando lazy
    // enviá-los ao servidor quando o jogador abrir a notícia. A fonte trocará
    // para os dados reais do SDK quando a integração corrida-real→carreira existir.
    let returned_news_id = corrida_news_id.clone();
    if let Some(news_id) = corrida_news_id {
        let category_name: &str = match crate::constants::categories::get_category_config(category_id)
        {
            Some(c) => c.nome,
            None => category_id,
        };
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
                    rust_i18n::t!("briefing.ctx.rookie_debut", name = driver.nome.as_str())
                        .to_string(),
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

        // --- Recordes e marcos da categoria (todas as temporadas) — peso histórico. ---
        // Os agregados já incluem a corrida atual (persistida antes daqui).
        {
            let winner_id = &race_result.winner_id;
            let winner_name = driver_queries::get_driver(conn, winner_id)
                .map(|d| d.nome)
                .unwrap_or_else(|_| winner_id.clone());
            let records = crate::db::queries::race_history::get_category_records(conn, category_id)
                .ok();

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
            let winner_career = crate::db::queries::race_history::get_driver_category_career(
                conn,
                winner_id,
                category_id,
            )
            .ok();

            // Caça a um rival que AINDA está no grid: vencedor a poucas vitórias de
            // igualar alguém logo acima dele no total histórico da categoria.
            if let Some(wc) = &winner_career {
                if let Ok(actives) =
                    crate::db::queries::race_history::get_active_category_win_counts(
                        conn,
                        category_id,
                    )
                {
                    let target = actives
                        .iter()
                        .filter(|a| {
                            a.pilot_id != *winner_id
                                && a.value > wc.wins
                                && a.value - wc.wins <= 3
                        })
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
                if is_new_record
                    && wc.wins >= 3
                    && recs.second_most_wins == Some(wc.wins - 1)
                {
                    if let Ok(Some(year)) =
                        crate::db::queries::race_history::first_year_reaching_wins(
                            conn,
                            category_id,
                            wc.wins - 1,
                        )
                    {
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
                if let Ok(Some(top)) =
                    crate::db::queries::race_history::get_team_top_winner_in_category(
                        conn,
                        &cur.team_id,
                        category_id,
                    )
                {
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

            // Memória temporal dos recordes (vitórias e pódios): registra QUANDO o
            // recorde all-time da categoria é batido, para notícias de "recorde
            // quebrado" com data e o rodapé do mundo. Condição: o recordista PONTUOU
            // hoje na métrica (então o recorde avançou nesta corrida) e é dono ISOLADO
            // do topo (2º colocado == valor-1). `previous = valor-1` (cada corrida soma
            // no máximo 1). Pisos evitam marcos triviais. Idempotente por valor.
            if let Some(recs) = records.as_ref() {
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
                    if r.pilot_id == *winner_id
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
                                pilot_id: winner_id.clone(),
                                pilot_name: winner_name.clone(),
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
                    .find(|d| d.pilot_id == *winner_id)
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
                    scalar("youngest_winner", winner_id, &winner_name, winner_age, "", false);
                    scalar("oldest_winner", winner_id, &winner_name, winner_age, "", true);
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
                        scalar("drought_broken", winner_id, &winner_name, drought, "", true);
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

            // Por destaque: o RECORDISTA aparece sempre que está em evidência (descreve
            // quem ele é, independe do resultado de hoje). Marcos de número redondo só
            // para quem REALMENTE fez aquilo hoje (venceu / subiu ao pódio / largou).
            for pilot_id in &featured {
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
                let holds = |rec: fn(&crate::db::queries::race_history::CategoryRecords) -> &Option<crate::db::queries::race_history::CategoryRecord>| {
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
                        d.pilot_id == *pilot_id
                            && !d.is_dnf
                            && (1..=3).contains(&d.finish_position)
                    });
                    if is_winner && !is_wins_record && [5, 10, 25, 50, 75, 100].contains(&career.wins)
                    {
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.nth_win",
                                n = career.wins,
                                name = name.as_str()
                            )
                            .to_string(),
                        );
                    }
                    if podium_today
                        && !is_podiums_record
                        && [25, 50, 100, 150].contains(&career.podiums)
                    {
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

        // --- Duelo interno: quem levou a melhor sobre o companheiro de equipe. ---
        // Só para o vencedor e o jogador (foco), lendo o próprio resultado — o "carro
        // irmão" é a referência mais justa da corrida do piloto. Par deduplicado.
        {
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

        // --- Quadro do campeonato: o que o resultado significa para a temporada. ---
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

                for pilot_id in &featured {
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
        for fact in rivalry_arc_facts(conn, race_result, &featured, active_season.numero, round) {
            context_facts.push(fact);
        }

        // Desempenho e forma: esperado×real, forma recente, histórico de pista e
        // confronto entre companheiros (pano de fundo dos destaques).
        for fact in
            performance_context_facts(conn, race_result, &featured, active_season, round, category_id)
        {
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
        let race_id_for_breakdowns = crate::db::queries::calendar::get_calendar(
            conn,
            &active_season.id,
            category_id,
        )
        .ok()
        .and_then(|entries| entries.into_iter().find(|e| e.rodada == round).map(|e| e.id));
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

        let ctx = crate::narrative::build_race_context(
            race_result,
            &crate::narrative::RaceContextInput {
                category_name,
                year: active_season.ano,
                round,
                injuries: &injury_facts,
                incidents: flat_incidents,
                context_facts: &context_facts,
            },
        );
        // Mapa nome da equipe → cor primária das equipes desta corrida. O front usa
        // para colorir os nomes das equipes citadas no boletim. Dedup por team_id.
        let mut team_colors = serde_json::Map::new();
        let mut seen_teams: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for d in &race_result.race_results {
            if d.team_name.is_empty() || !seen_teams.insert(d.team_id.as_str()) {
                continue;
            }
            let color: Option<String> = conn
                .query_row(
                    "SELECT cor_primaria FROM teams WHERE id = ?1",
                    rusqlite::params![d.team_id],
                    |r| r.get::<_, String>(0),
                )
                .ok();
            if let Some(c) = color {
                if !c.trim().is_empty() {
                    team_colors.insert(d.team_name.clone(), serde_json::Value::String(c));
                }
            }
        }
        let teams_json = serde_json::Value::Object(team_colors).to_string();

        if let Err(e) =
            crate::db::queries::ai_story::store_race_facts(conn, &news_id, &ctx.facts, &teams_json)
        {
            eprintln!("[narrative] Falha ao guardar fatos do boletim de IA: {e:?}");
        }
    }

    Ok(returned_news_id)
}

/// Pré-gera o boletim de IA em BACKGROUND logo após a corrida, para que ele já
/// esteja em cache quando o jogador abrir a aba de Notícias (sem sentir a latência
/// do servidor). Roda numa thread própria com conexão própria ao banco. Silencioso:
/// se falhar (rede/cooldown), o caminho lazy de abrir a notícia tenta de novo.
pub(super) fn spawn_prewarm_boletim(
    db_path: std::path::PathBuf,
    news_id: String,
    lang: String,
    install_id: String,
) {
    std::thread::spawn(move || {
        let db = match Database::open_existing(&db_path) {
            Ok(d) => d,
            Err(_) => return,
        };
        let row = match crate::db::queries::ai_story::get_story(&db.conn, &news_id) {
            Ok(Some(r)) => r,
            _ => return,
        };
        if row.story.is_some() {
            return; // já gerado em algum momento — nada a fazer
        }
        // reading_seconds = None → tamanho padrão do servidor (a adaptação por
        // engajamento continua valendo no caminho lazy, se ainda não houver cache).
        if let Ok(story) =
            crate::narrative::client::fetch_story(&row.facts, &lang, &install_id, None)
        {
            let _ = crate::db::queries::ai_story::set_story(&db.conn, &news_id, &story);
        }
    });
}

pub(super) fn persist_other_category_news(
    conn: &rusqlite::Connection,
    highlights: &[SimHighlight],
    season_number: i32,
) -> Result<(), String> {
    use crate::db::queries::news as news_queries;
    use crate::generators::ids::{next_ids, IdType};
    use crate::news::{NewsImportance, NewsItem, NewsType};

    if highlights.is_empty() {
        return Ok(());
    }

    let ids = next_ids(conn, IdType::News, highlights.len() as u32)
        .map_err(|e| format!("next_ids news: {e:?}"))?;
    let now = chrono::Local::now().timestamp();
    let items = highlights
        .iter()
        .zip(ids)
        .map(|(highlight, id)| NewsItem {
            id,
            tipo: NewsType::Corrida,
            icone: NewsType::Corrida.icone().to_string(),
            titulo: highlight.headline.clone(),
            texto: rust_i18n::t!(
                "race.news.other_categories_summary",
                headline = highlight.headline.as_str()
            )
            .to_string(),
            rodada: None,
            semana_pretemporada: None,
            temporada: season_number,
            categoria_id: Some(highlight.category.clone()),
            categoria_nome: get_category_config(&highlight.category)
                .map(|category| category.nome.to_string()),
            importancia: NewsImportance::Media,
            timestamp: now,
            driver_id: None,
            driver_id_secondary: None,
            team_id: None,
        })
        .collect::<Vec<_>>();

    news_queries::insert_news_batch(conn, &items)
        .map_err(|e| format!("insert_news_batch outras categorias: {e:?}"))?;
    Ok(())
}
