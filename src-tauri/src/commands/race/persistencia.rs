//! Escrituracao do resultado no banco: transacao unica do fim de semana, baixa de assento por lesao grave, atualizacao de estatisticas e historico.

use super::*;

/// Persiste um `RaceResult` na carreira dentro de UMA transação: standings/pontos,
/// recuperação+geração de lesões, resumo da corrida, avanço de rodada, hierarquia
/// e rivalidades. É o "rabo" compartilhado entre a simulação OFFLINE e o IMPORT do
/// iRacing — ambos produzem um `RaceResult` e caem aqui, então a carreira reage
/// igual nos dois caminhos. Retorna as lesões novas (pode incluir pilotos de IA).
#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_race_result_tx(
    db: &mut Database,
    race_entry: &CalendarEntry,
    result: &RaceResult,
    teams: &[Team],
    active_season: &Season,
    category: &CategoryConfig,
    next_round: Option<i32>,
    persistence_mode: RacePersistenceMode,
    // Estilo de pilotagem do JOGADOR (só quando ele correu no iRacing); `None` na sim offline.
    // Modula o desgaste SÓ do carro dele.
    player_style: Option<crate::car::driving_style::StyleFactors>,
    // Nº de paradas REAIS do jogador (do SDK) — alívio de gasto de peça do enduro (só no carro
    // dele; 10%/parada, teto 30%). 0 na sim offline (a IA modela as paradas pela duração).
    player_pits: u32,
    // Feedback físico da quebra (§4.6): peças que largaram nesta corrida, por time. Vazio na sim
    // offline (sem quebra) e nas corridas fora da categoria do jogador.
    team_breakdowns: &std::collections::HashMap<
        String,
        Vec<(crate::car::PartType, crate::car::breakdown::Severity)>,
    >,
    rng: &mut impl rand::Rng,
) -> Result<Vec<Injury>, String> {
    let mut new_injuries_out: Vec<Injury> = Vec::new();
    db.transaction(|tx| {
        // 0. Guarda de idempotência: o status foi checado fora da transação,
        // então uma invocação concorrente do mesmo comando pode ter concluído
        // esta corrida nesse meio-tempo. Re-checa sob o lock de escrita para
        // nunca persistir o mesmo resultado duas vezes.
        let current_status = calendar_queries::get_calendar_entry_by_id(tx, &race_entry.id)?
            .map(|entry| entry.status)
            .ok_or_else(|| {
                crate::db::connection::DbError::NotFound(format!(
                    "corrida '{}' nao encontrada ao persistir resultado",
                    race_entry.id
                ))
            })?;
        if current_status.as_str() != "Pendente" {
            return Err(crate::db::connection::DbError::InvalidData(format!(
                "corrida '{}' ja foi concluida por outra simulacao concorrente",
                race_entry.id
            )));
        }

        // 1. Processo de recuperação das lesões já ativas
        crate::evolution::injury::process_injury_recovery(tx, &race_entry.categoria)?;

        // 2. Aplica pontuações normais
        let economic_health = global_economic_health_for_season(active_season.numero as i32);
        // Próximas pistas da categoria (após esta rodada) — o cérebro do carro corta pelo
        // horizonte de cada time. Computado aqui e passado adiante.
        let upcoming_track_ids: Vec<u32> =
            calendar_queries::get_calendar(tx, &race_entry.season_id, &race_entry.categoria)?
                .into_iter()
                .filter(|entry| entry.rodada > race_entry.rodada)
                .map(|entry| entry.track_id)
                .collect();
        // Condições reais da corrida → o desgaste da grade toda responde à pista + clima
        // desta rodada (o cérebro de manutenção reage a corridas brutais). Todos os 4 canais
        // vêm dos campos AUTORITATIVOS persistidos da MESMA história que o iRacing roda: clima
        // e temperatura no `race_entry`; umidade e vento nas colunas da etapa (via A, gravadas
        // por `resolve_and_persist_race_weather`). Saves antigos → default neutro (45/25).
        let (humidity, wind_kmh) = tx
            .query_row(
                "SELECT umidade, vento FROM calendar WHERE id = ?1",
                [race_entry.id.as_str()],
                |row| Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?)),
            )
            .unwrap_or((
                crate::car::breakdown::Weather::NEUTRAL.humidity,
                crate::car::breakdown::Weather::NEUTRAL.wind_kmh,
            ));
        let weather = crate::car::breakdown::Weather {
            wetness: race_entry.clima.wetness(),
            temperature: race_entry.temperatura,
            humidity,
            wind_kmh,
        };
        // Duração da corrida (min) da categoria — acima do gate de enduro, o desgaste de peça
        // sobe pra grade toda. Fonte de verdade = config da categoria (mesma que a sim usa em
        // context.rs); categoria não resolvida → 30 (sprint/neutro).
        let duracao_min = crate::constants::categories::get_category_config(&race_entry.categoria)
            .map(|c| c.duracao_corrida_min)
            .unwrap_or(30);
        let wear_conditions = crate::market::car_maintenance::WearConditions::from_race(
            race_entry.track_id,
            weather,
            duracao_min,
        );
        // Time do JOGADOR (só se há estilo capturado) — o estilo modula o desgaste só do carro
        // dele. Piloto → contrato ativo → equipe.
        let player_team_id: Option<String> = if player_style.is_some() {
            crate::db::queries::drivers::get_player_driver(tx)
                .ok()
                .and_then(|p| {
                    crate::db::queries::contracts::get_active_contract_for_pilot(tx, &p.id)
                        .ok()
                        .flatten()
                })
                .map(|c| c.equipe_id)
        } else {
            None
        };
        // Peça 3 · desconfiança mecânica → menos desgaste: times com DNF MECÂNICO nas rodadas
        // recentes POUPAM as peças (o loop emergente da quebra: quebrou → desconfia → poupa →
        // quebra menos). Janela = as 3 rodadas ANTERIORES (a atual ainda não foi persistida).
        let team_cautions: std::collections::HashMap<
            String,
            crate::car::driving_style::StyleFactors,
        > = {
            let r = race_entry.rodada;
            crate::db::queries::race_history::mechanical_dnf_counts_by_team(
                tx,
                &active_season.id,
                &race_entry.categoria,
                (r - 3).max(1),
                r - 1,
            )
            .unwrap_or_default()
            .into_iter()
            .map(|(team, count)| {
                (
                    team,
                    crate::car::driving_style::StyleFactors::uniform(mechanical_caution_factor(
                        count,
                    )),
                )
            })
            .collect()
        };
        apply_race_result_to_database(
            tx,
            result,
            teams,
            economic_health,
            &race_entry.categoria,
            persistence_mode,
            race_entry.track_id,
            active_season.numero as i32,
            race_entry.rodada,
            &upcoming_track_ids,
            wear_conditions,
            venue_prestige_score(race_entry),
            player_team_id.as_deref(),
            player_style,
            player_pits,
            &team_cautions,
            team_breakdowns,
        )?;

        // 3. Verifica os incidentes recém-gerados e processa possíveis lesões
        let flat_incidents: Vec<_> = result
            .race_results
            .iter()
            .flat_map(|r| r.incidents.clone())
            .collect();
        let new_injuries = crate::evolution::injury::process_new_injuries(
            tx,
            active_season.numero as i32,
            &race_entry.id,
            &flat_incidents,
            &mut *rng,
        )?;
        new_injuries_out = new_injuries;

        // 3b. Lesão GRAVE pode (raramente, só IA) encerrar a carreira no meio da temporada.
        // Quem pendura o capacete fica congelado na classificação e a vaga é preenchida por
        // um substituto que entra como NOME NOVO (pilot_id próprio → começa do zero).
        process_severe_injury_retirements(tx, &new_injuries_out, active_season, &mut *rng)?;

        // 4. Salva o resumo da corrida e avança
        crate::db::queries::races::insert_race_results_batch(
            tx,
            &race_entry.id,
            &result.race_results,
        )?;
        calendar_queries::mark_race_completed(tx, &race_entry.id)?;
        if let Some(round) = next_round {
            season_queries::update_season_rodada(tx, &active_season.id, round)?;
        }
        season_queries::move_to_encerramento_if_completed(tx, active_season)?;

        // 5. Processa hierarquia interna das equipes da categoria
        if !runs_in_special_phase(&race_entry.categoria) {
            crate::hierarchy::orders::process_hierarchy_for_category(
                tx,
                &result.race_results,
                &race_entry.categoria,
                race_entry.rodada,
                category.corridas_por_temporada as i32,
                active_season.numero,
            )?;
        }

        // 6. Processa rivalidades por disputa de campeonato (últimas rodadas)
        crate::rivalry::process_championship_rivalry(
            tx,
            &race_entry.categoria,
            race_entry.rodada,
            category.corridas_por_temporada as i32,
            active_season.numero,
        )?;

        // 7. Processa rivalidades geradas por colisões bilaterais (fatos da corrida)
        crate::rivalry::process_collisions_rivalry(
            tx,
            &flat_incidents,
            &race_entry.categoria,
            race_entry.rodada,
            active_season.numero,
        )?;

        // 8. Rivalidade entre EQUIPES (Fontes 3+4 + moral de derby): reusa os mesmos fatos
        // da corrida. Mapa driver→time e melhor chegada por time, do próprio resultado.
        let team_by_driver: std::collections::HashMap<String, String> = result
            .race_results
            .iter()
            .map(|r| (r.pilot_id.clone(), r.team_id.clone()))
            .collect();
        // Fonte 3 — guerra na pista: agrega as colisões por par de times diferentes.
        crate::rivalry::team::process_team_collisions_rivalry(
            tx,
            &flat_incidents,
            &team_by_driver,
            &race_entry.categoria,
            race_entry.rodada,
            active_season.numero,
        )?;
        // Fonte 4 — transbordamento: rivalidades intensas de piloto cross-time pingam nos times.
        crate::rivalry::team::process_driver_rivalry_bleed(
            tx,
            &team_by_driver,
            &race_entry.categoria,
            race_entry.rodada,
            active_season.numero,
        )?;
        // Tier 2 — pulso de moral de derby: bater o rival empurra a moral (sutil, simétrico).
        let mut team_best_finish: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        for r in &result.race_results {
            team_best_finish
                .entry(r.team_id.clone())
                .and_modify(|p| {
                    if r.finish_position < *p {
                        *p = r.finish_position;
                    }
                })
                .or_insert(r.finish_position);
        }
        crate::rivalry::team::apply_derby_morale(tx, &team_best_finish)?;

        Ok(())
    })
    .map_err(|e| format!("Falha ao persistir resultado da corrida: {e}"))?;

    Ok(new_injuries_out)
}

/// Esvazia o assento do piloto no time (libera a vaga para reposição), mantendo o
/// companheiro no lugar.
pub(super) fn vacate_team_seat(
    tx: &rusqlite::Transaction,
    team_id: &str,
    driver_id: &str,
) -> Result<(), DbError> {
    if let Some(team) = team_queries::get_team_by_id(tx, team_id)? {
        let p1 = team.piloto_1_id.filter(|id| id.as_str() != driver_id);
        let p2 = team.piloto_2_id.filter(|id| id.as_str() != driver_id);
        team_queries::update_team_pilots(tx, team_id, p1.as_deref(), p2.as_deref())?;
    }
    Ok(())
}

/// Aposentadoria por lesão grave no meio da temporada (só IA). Para cada lesão GRAVE
/// recém-gerada, rola a chance ponderada por idade; se aposentar: registra no hall dos
/// aposentados, desativa a lesão (para a recuperação por corrida não "ressuscitar" o
/// piloto para Ativo), marca Aposentado MANTENDO a categoria (fica congelado na
/// classificação da temporada), rescinde o contrato, esvazia o assento e contrata um
/// substituto que entra como nome novo (pilot_id próprio → não herda resultados).
pub(super) fn process_severe_injury_retirements(
    tx: &rusqlite::Transaction,
    new_injuries: &[Injury],
    active_season: &Season,
    rng: &mut impl rand::Rng,
) -> Result<(), DbError> {
    use crate::models::enums::InjuryType;
    for injury in new_injuries {
        if injury.injury_type != InjuryType::Grave {
            continue;
        }
        let mut driver = driver_queries::get_driver(tx, &injury.pilot_id)?;
        // O piloto do jogador NUNCA é aposentado à força (decisão de design).
        if driver.is_jogador {
            continue;
        }
        let chance =
            crate::evolution::retirement::severe_injury_retirement_chance(driver.idade);
        if !rng.gen_bool(chance.clamp(0.0, 1.0)) {
            continue;
        }

        let final_category = driver
            .categoria_atual
            .clone()
            .unwrap_or_else(|| "SemCategoria".to_string());
        let reason = rust_i18n::t!("race.retirement.injury", age = driver.idade).to_string();

        // 1. Hall dos aposentados (snapshot de carreira).
        crate::evolution::pipeline::persist_retired_driver(
            tx,
            &driver,
            active_season,
            &final_category,
            &reason,
        )
        .map_err(DbError::InvalidData)?;

        // 2. Desativa a lesão: senão a recuperação por corrida a zeraria e devolveria o
        // piloto aposentado ao status Ativo.
        crate::db::queries::injuries::update_injury_status(tx, &injury.id, 0, false)?;

        // 3. Marca Aposentado, MANTENDO categoria_atual (fica congelado na classificação;
        // a grade filtra Aposentado e a virada de temporada pula quem não é Ativo).
        crate::evolution::retirement::process_retirement(&mut driver);
        driver_queries::update_driver(tx, &driver).map_err(|e| {
            DbError::InvalidData(format!("Falha ao aposentar piloto lesionado: {e}"))
        })?;

        // 4. Libera a vaga e contrata um substituto (agente livre licenciado ou novato).
        if let Some(contract) =
            contract_queries::get_active_regular_contract_for_pilot(tx, &driver.id)?
        {
            let team_id = contract.equipe_id.clone();
            contract_queries::update_contract_status(
                tx,
                &contract.id,
                &crate::models::enums::ContractStatus::Rescindido,
            )?;
            vacate_team_seat(tx, &team_id, &driver.id)?;
            crate::commands::career::backfill_team_vacancy(
                tx,
                &team_id,
                active_season.numero,
                active_season.ano,
            )
            .map_err(DbError::InvalidData)?;
        }
    }
    Ok(())
}

pub(super) fn apply_race_result_to_database(
    tx: &rusqlite::Transaction<'_>,
    result: &RaceResult,
    teams: &[Team],
    economic_health: GlobalEconomicHealth,
    race_category: &str,
    persistence_mode: RacePersistenceMode,
    track_id: u32,
    season_number: i32,
    round: i32,
    upcoming_track_ids: &[u32],
    wear_conditions: crate::market::car_maintenance::WearConditions,
    // Bilheteria (Fase 3 do Estrelato): prestígio "de local" do evento (pré-vendido),
    // usado pra dimensionar o bolo de público da rodada.
    event_prestige_score: f64,
    // Estilo de pilotagem do JOGADOR (só quando ele correu no iRacing) + o time dele — o
    // estilo modula o desgaste SÓ do carro do jogador. `None` = corrida sem estilo capturado.
    player_team_id: Option<&str>,
    player_style: Option<crate::car::driving_style::StyleFactors>,
    // Nº de paradas REAIS do jogador (SDK) — alívio de gasto de peça do enduro só no carro dele.
    player_pits: u32,
    // Desconfiança mecânica por time (DNF mecânico recente) → poupa as peças. Vazio = ninguém.
    team_cautions: &std::collections::HashMap<String, crate::car::driving_style::StyleFactors>,
    // Feedback físico da quebra (§4.6): peças que largaram nesta corrida, por time. Leve → segue;
    // Grave → fim de vida; DNF → destruída (troca forçada a débito). Vazio na sim offline.
    team_breakdowns: &std::collections::HashMap<
        String,
        Vec<(crate::car::PartType, crate::car::breakdown::Severity)>,
    >,
) -> Result<(), DbError> {
    for race_driver in &result.race_results {
        let mut driver = driver_queries::get_driver(tx, &race_driver.pilot_id)?;
        let mut season_stats = driver.stats_temporada.clone();
        let mut career_stats = driver.stats_carreira.clone();

        let previous_races = season_stats.corridas;
        season_stats.pontos += race_driver.points_earned as f64;
        season_stats.vitorias += u32::from(!race_driver.is_dnf && race_driver.finish_position == 1);
        season_stats.podios += u32::from(!race_driver.is_dnf && race_driver.finish_position <= 3);
        season_stats.poles += u32::from(race_driver.pilot_id == result.pole_sitter_id);
        season_stats.corridas += 1;
        season_stats.dnfs += u32::from(race_driver.is_dnf);
        season_stats.posicao_media = recalculate_average_position(
            season_stats.posicao_media,
            previous_races,
            race_driver.finish_position,
        );

        career_stats.pontos_total += race_driver.points_earned as f64;
        career_stats.vitorias += u32::from(!race_driver.is_dnf && race_driver.finish_position == 1);
        career_stats.podios += u32::from(!race_driver.is_dnf && race_driver.finish_position <= 3);
        career_stats.poles += u32::from(race_driver.pilot_id == result.pole_sitter_id);
        career_stats.corridas += 1;
        career_stats.dnfs += u32::from(race_driver.is_dnf);

        let better_result = driver
            .melhor_resultado_temp
            .map(|current| current.min(race_driver.finish_position as u32))
            .or(Some(race_driver.finish_position as u32));

        driver.stats_temporada = season_stats;
        driver.stats_carreira = career_stats;
        driver.melhor_resultado_temp = better_result;
        driver.corridas_na_categoria += 1;
        // Conhecimento de pista: registra a corrida no cache (largadas, melhor
        // resultado, temporada) — alimenta a penalidade de pista nova (sim + export).
        let track_finish = if race_driver.is_dnf {
            None
        } else {
            Some(race_driver.finish_position as u32)
        };
        crate::simulation::track_knowledge::record_race(
            &mut driver.historico_circuitos,
            track_id as i64,
            track_finish,
            season_number,
        );
        driver.ultimos_resultados = append_recent_result(
            &driver.ultimos_resultados,
            race_driver.finish_position,
            race_driver.is_dnf,
        );

        driver_queries::update_driver(tx, &driver)?;
    }

    if runs_in_special_phase(race_category) {
        return Ok(());
    }

    let active_contracts = if persistence_mode == RacePersistenceMode::Playable {
        Some(contract_queries::get_all_active_regular_contracts(tx)?)
    } else {
        None
    };
    let race_results_by_team = group_results_by_team(result);
    let category_id = teams
        .first()
        .map(|team| team.categoria.as_str())
        .unwrap_or("");
    let rounds_in_season = get_category_config(category_id)
        .map(|config| f64::from(config.corridas_por_temporada.max(1)))
        .unwrap_or(1.0);
    // Bilheteria (Fase 3 do Estrelato): presença pública (fama do lineup) de cada time do
    // grid, pré-computada 1× por rodada. A soma é o denominador da cota competitiva de
    // bilheteria; o loop abaixo reusa a presença por time (evita 2ª consulta ao lineup).
    let team_presences: std::collections::HashMap<String, f64> = teams
        .iter()
        .map(|team| {
            let medias = team_queries::get_team_lineup_medias(tx, &team.id).unwrap_or_default();
            let presence = crate::public_presence::team::derive_team_public_presence(&medias);
            (team.id.clone(), presence)
        })
        .collect();
    let grid_total_presence: f64 = team_presences.values().sum();
    let grid_team_count = teams.len().max(1) as f64;
    for team in teams {
        let Some(team_results) = race_results_by_team.get(&team.id) else {
            continue;
        };

        let added_points: i32 = team_results.iter().map(|entry| entry.points_earned).sum();
        let added_victories: i32 = team_results
            .iter()
            .filter(|entry| entry.finish_position == 1)
            .count() as i32;
        let added_podiums: i32 = team_results
            .iter()
            .filter(|entry| entry.finish_position <= 3)
            .count() as i32;
        let added_poles: i32 = i32::from(
            team_results
                .iter()
                .any(|entry| entry.pilot_id == result.pole_sitter_id),
        );
        let best_result = team_results
            .iter()
            .map(|entry| entry.finish_position)
            .min()
            .unwrap_or(99);
        let current_best = if team.stats_melhor_resultado <= 0 {
            99
        } else {
            team.stats_melhor_resultado
        };

        team_queries::update_team_season_stats(
            tx,
            &team.id,
            team.stats_vitorias + added_victories,
            team.stats_podios + added_podiums,
            team.stats_poles + added_poles,
            team.stats_pontos + added_points,
            current_best.min(best_result),
        )?;

        let Some(active_contracts) = active_contracts.as_ref() else {
            continue;
        };

        let team_salary_total: f64 = active_contracts
            .iter()
            .filter(|contract| contract.equipe_id == team.id)
            .map(|contract| contract.salario_anual)
            .sum();
        let salary_expense = team_salary_total / rounds_in_season;
        // Fama de equipe → patrocínio + bilheteria: presença pública do lineup (fama dos
        // pilotos). É o motor da "2ª moeda" — um rosto famoso capta patrocínio pra construir
        // o carro E puxa público pro portão. Reusa o pré-cômputo do grid.
        let lineup_public_presence = team_presences.get(&team.id).copied().unwrap_or(0.0);
        // Sistema de Nível do Carro: o cérebro decide a manutenção, aplica o desgaste e
        // persiste o carro; o custo decidido vira depreciação REAL na fatura abaixo.
        // O estilo de pilotagem só incide no carro do JOGADOR (o time dele nesta corrida). Os
        // demais times podem POUPAR as peças por desconfiança mecânica (DNF mecânico recente).
        let is_player_car = player_team_id == Some(team.id.as_str());
        let team_style = if is_player_car {
            player_style
        } else {
            team_cautions.get(&team.id).copied()
        };
        // Carro do JOGADOR usa o pit REAL do SDK pro alívio de enduro; a IA modela pela duração.
        // Feedback físico da quebra (§4.6): peças DESTE time que largaram nesta corrida.
        let this_team_breakdowns: &[(crate::car::PartType, crate::car::breakdown::Severity)] =
            team_breakdowns.get(team.id.as_str()).map(|v| v.as_slice()).unwrap_or(&[]);
        let car_maintenance_cost = crate::market::car_maintenance::maintain_team_car_pits(
            tx,
            team,
            category_id,
            season_number,
            upcoming_track_ids,
            wear_conditions,
            team_style,
            is_player_car,
            player_pits,
            this_team_breakdowns,
        )?;

        let finance_context = calculate_team_round_finance_context(
            team,
            lineup_public_presence,
            added_points,
            added_victories,
            added_podiums,
            best_result,
            salary_expense,
            rounds_in_season,
            economic_health,
            car_maintenance_cost,
            round_operation_context(result, &team.id, track_id),
            event_prestige_score,
            grid_total_presence,
            grid_team_count,
        );

        let mut updated_team = team.clone();
        let cashflow_summary = apply_round_cashflow(&mut updated_team, finance_context);
        apply_crisis_event_if_needed(&mut updated_team);
        refresh_team_financial_state(&mut updated_team);
        team_queries::update_team_finance_snapshot(tx, &updated_team)?;
        // Grava a divisão REAL da rodada (as 9 linhas) no histórico — fonte única do
        // dossiê financeiro da aba My Team, no lugar dos números fabricados no front.
        team_queries::insert_team_finance_history(
            tx,
            &updated_team,
            &finance_context,
            &cashflow_summary,
            season_number,
            round,
        )?;
    }

    Ok(())
}

pub(super) fn append_recent_result(
    existing: &serde_json::Value,
    finish_position: i32,
    is_dnf: bool,
) -> serde_json::Value {
    let mut results: Vec<serde_json::Value> = existing
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.as_object().cloned())
        .map(serde_json::Value::Object)
        .collect();

    results.push(serde_json::json!({
        "position": finish_position,
        "is_dnf": is_dnf,
        "has_fastest_lap": false,
        "grid_position": 0,
        "positions_gained": 0
    }));

    if results.len() > 5 {
        let keep_from = results.len() - 5;
        results.drain(0..keep_from);
    }

    serde_json::Value::Array(results)
}

pub(super) fn recalculate_average_position(
    current_average: f64,
    previous_races: u32,
    finish_position: i32,
) -> f64 {
    let total = current_average * previous_races as f64 + finish_position as f64;
    total / (previous_races as f64 + 1.0)
}
