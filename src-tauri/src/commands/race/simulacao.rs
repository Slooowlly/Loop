//! Execucao da corrida em si: sorteio previo de quebras, simulacao de uma categoria (carreira ou historica) e varredura das demais categorias da mesma semana.

use super::*;

pub(crate) fn simulate_category_race(
    db: &mut Database,
    race_entry: &CalendarEntry,
    advance_player_round: bool,
) -> Result<(RaceResult, Vec<Injury>), String> {
    simulate_category_race_with_mode(
        db,
        race_entry,
        advance_player_round,
        RacePersistenceMode::Playable,
    )
}

pub(crate) fn simulate_historical_category_race(
    db: &mut Database,
    race_entry: &CalendarEntry,
) -> Result<(RaceResult, Vec<Injury>), String> {
    simulate_category_race_with_mode(db, race_entry, false, RacePersistenceMode::HistoricalDraft)
}

/// `career_id` da carreira aberta, deduzido do caminho do banco (`<saves>/<career_id>/career.db`).
///
/// A quebra da corrida simulada precisa dele pra semear exatamente como o caminho AO VIVO
/// (`event_seed`): o id da etapa é SEQUENCIAL por save ("R001"), então sem a carreira na mistura
/// dois saves diferentes rolariam a MESMA sorte na mesma rodada. Vazio se o caminho não tiver a
/// forma esperada — aí a semente perde só o tempero entre saves, não o determinismo dentro de um.
pub(super) fn career_id_from_db(db: &Database) -> String {
    db.path
        .parent()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Uma quebra pré-rolada de corrida simulada, com as três caras que ela precisa ter: o que a
/// SIMULAÇÃO cobra (tempo/abandono), o que o SAVE registra (peça, volta, severidade) e o que a
/// ECONOMIA recebe de volta (peça danificada do time). Juntas aqui pra o caller não remontar nada.
pub(super) struct PrerolledBreakdown {
    outcome: crate::simulation::race::MechanicalOutcome,
    row: crate::db::queries::race_breakdowns::RaceBreakdownRow,
    team_id: String,
    part: crate::car::PartType,
    severity: crate::car::breakdown::Severity,
}

/// Pré-rola a QUEBRA DE PEÇA do grid inteiro de uma corrida SIMULADA — a Fase 7 do Sistema de
/// Quebra, o que faltava pra corrida não-dirigida ter culpado.
///
/// Usa o MESMO cérebro e os MESMOS inputs do disparo ao vivo: o desgaste REAL do `team_car` de
/// cada equipe, o pit crew dela, a demanda P/H/A da pista e o clima determinístico da etapa. É
/// isso que faz a quebra ser consequência da economia e não um dado jogado por cima — o time
/// pobre, que estica peça por falta de caixa, é o que larga na pista.
///
/// Piloto cujo time não tem carro no banco (save antigo, grid sintético) simplesmente não rola:
/// a corrida segue sem quebra pra ele, em vez de inventar um carro médio que não existe.
///
/// Devolve `None` quando NENHUM carro do grid pôde ser lido — aí o sistema não tem como rodar e
/// quem responde pela falha mecânica continua sendo a pane do catálogo de incidentes. `Some`
/// (mesmo vazio) significa "a quebra rodou": o catálogo sai de cena e esta é a fonte única.
pub(super) fn preroll_simulated_breakdowns(
    conn: &rusqlite::Connection,
    career_id: &str,
    race_entry: &CalendarEntry,
    category: &CategoryConfig,
    total_laps: i32,
    sim_drivers: &[SimDriver],
    teams: &[Team],
) -> Option<Vec<PrerolledBreakdown>> {
    use crate::car::breakdown::{is_enduro_duration, roll_race_breakdowns_cfg};
    use crate::db::queries::race_breakdowns::RaceBreakdownRow;
    use crate::db::queries::team_car as tcq;
    use crate::market::car_maintenance::maintenance_demand;
    use crate::simulation::race::MechanicalOutcome;

    let ev_seed = crate::commands::iracing::event_seed(career_id, &race_entry.id);
    let weather = crate::commands::iracing::race_breakdown_weather(
        race_entry.track_id,
        race_entry.week_of_year,
        ev_seed,
        false,
    );
    let track_pha = maintenance_demand(&[race_entry.track_id]);
    // Enduro: severidade abrandada (o grid não esvazia) + rampa de desgaste no fim.
    let is_enduro = is_enduro_duration(category.duracao_corrida_min);
    // Tenda de durabilidade por nível só em categoria GERIDA (teto ≥ 3); spec fica de fora.
    let apply_tent = crate::car::cost::category_ceiling(&race_entry.categoria) > 2;
    let laps = total_laps.max(1) as u32;

    let pit_by_team: HashMap<&str, f64> = teams
        .iter()
        .map(|t| (t.id.as_str(), t.pit_crew_quality))
        .collect();
    // O carro é da EQUIPE (os dois pilotos partilham) — lido uma vez e reusado.
    let mut car_cache: HashMap<String, Option<crate::car::Car>> = HashMap::new();

    let mut out = Vec::new();
    let mut algum_carro_lido = false;
    for driver in sim_drivers {
        let car = car_cache
            .entry(driver.team_id.clone())
            .or_insert_with(|| tcq::get_team_car(conn, &driver.team_id).ok().flatten());
        let Some(car) = car.as_ref() else {
            continue;
        };
        algum_carro_lido = true;
        // Semente por PILOTO sobre a da etapa — o mesmo `seed_for` do disparo ao vivo. Os dois
        // pilotos de um time rolam sortes independentes a partir do MESMO desgaste do carro.
        let mut seed = ev_seed;
        for b in driver.id.bytes() {
            seed = seed.wrapping_mul(0x0000_0100_0000_01B3).wrapping_add(b as u64);
        }
        let pit = pit_by_team
            .get(driver.team_id.as_str())
            .copied()
            .unwrap_or(50.0);
        let events = roll_race_breakdowns_cfg(
            car, laps, seed, pit, track_pha, weather, &[], is_enduro, apply_tent,
        );
        for ev in events {
            let label = ev.problem_label().to_string();
            out.push(PrerolledBreakdown {
                outcome: MechanicalOutcome {
                    pilot_id: driver.id.clone(),
                    lap: ev.lap,
                    is_dnf: ev.is_dnf(),
                    penalty_secs: ev.penalty_secs.unwrap_or(0),
                    label: label.clone(),
                },
                row: RaceBreakdownRow {
                    driver_id: driver.id.clone(),
                    part: ev.part.as_str().to_string(),
                    problem: ev.problem,
                    lap: ev.lap,
                    severity: ev.severity.key().to_string(),
                    penalty_secs: ev.penalty_secs,
                    forced: ev.forced,
                    label,
                },
                team_id: driver.team_id.clone(),
                part: ev.part,
                severity: ev.severity,
            });
        }
    }
    algum_carro_lido.then_some(out)
}

pub(super) fn simulate_category_race_with_mode(
    db: &mut Database,
    race_entry: &CalendarEntry,
    advance_player_round: bool,
    persistence_mode: RacePersistenceMode,
) -> Result<(RaceResult, Vec<Injury>), String> {
    let category = get_category_config(&race_entry.categoria)
        .ok_or_else(|| "Categoria da corrida nao encontrada.".to_string())?;
    let active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let uses_regular_special_grid = uses_regular_special_event_grid(&race_entry.categoria);
    let mut teams = if uses_regular_special_grid {
        get_regular_special_event_teams(&db.conn, &race_entry.categoria)
    } else if runs_in_special_phase(&race_entry.categoria) {
        special_entry_queries::get_entry_teams_for_category(
            &db.conn,
            &active_season.id,
            &race_entry.categoria,
        )
    } else {
        team_queries::get_teams_by_category(&db.conn, &race_entry.categoria)
    }
    .map_err(|e| format!("Falha ao buscar equipes da categoria: {e}"))?;
    if persistence_mode == RacePersistenceMode::HistoricalDraft {
        teams.retain(|team| is_team_active_in_year(team, active_season.ano));
    }
    let regular_special_contracts = if uses_regular_special_grid {
        let contracts =
            get_regular_special_event_contracts(&db.conn, &race_entry.categoria, &teams)?;
        if contracts.is_empty() {
            None
        } else {
            Some(contracts)
        }
    } else {
        None
    };
    let drivers = if let Some(contracts) = regular_special_contracts.as_ref() {
        get_drivers_for_contracts(&db.conn, contracts)?
    } else if uses_regular_special_grid {
        get_drivers_for_team_lineups(&db.conn, &teams)?
    } else {
        driver_queries::get_drivers_by_active_category(&db.conn, &race_entry.categoria)
            .map_err(|e| format!("Falha ao buscar pilotos da categoria: {e}"))?
    };

    let team_by_driver = if let Some(contracts) = regular_special_contracts.as_ref() {
        build_regular_contract_team_lookup(contracts, &teams)
    } else if runs_in_special_phase(&race_entry.categoria) {
        build_special_team_lookup(&db.conn, &teams, &race_entry.categoria)?
    } else {
        build_team_lookup(&teams)
    };
    // Lesionados CONTINUAM correndo (com penalidade de ritmo decrescente aplicada abaixo);
    // ninguém é mais removido da grade por lesão. Só ficam de fora os aposentados — inclusive
    // quem pendurou o capacete no meio da temporada por lesão grave, que segue congelado na
    // classificação mas não volta à pista.
    let driver_pool: Vec<_> = drivers
        .iter()
        .filter(|driver| driver.status != crate::models::enums::DriverStatus::Aposentado)
        .collect();
    // Lesões ativas da categoria, por piloto: alimentam a penalidade de ritmo do machucado.
    let injuries_by_driver: HashMap<String, Injury> =
        crate::db::queries::injuries::get_active_injuries_for_category(
            &db.conn,
            &race_entry.categoria,
        )
        .map_err(|e| format!("Falha ao buscar lesoes ativas da categoria: {e}"))?
        .into_iter()
        .map(|injury| (injury.pilot_id.clone(), injury))
        .collect();
    // Pressão de campeonato: standings da categoria (pontos de todos) + corridas
    // restantes + pontos do vencedor (P1 + volta rápida). Usado por piloto abaixo.
    let title_points: Vec<f64> = drivers.iter().map(|d| d.stats_temporada.pontos).collect();
    let races_left = (category.corridas_por_temporada as i32 - race_entry.rodada + 1).max(1) as u32;
    let max_race_points =
        (get_points_for_position(1, category.id == "endurance") + BONUS_FASTEST_LAP) as f64;

    // Casa cheia: interesse "de local" do evento (categoria + fase + rodada + papel
    // narrativo), SEM protagonismo do jogador nem drama de título — esses já entram
    // pela pressão de campeonato acima. Vale igual pra todos os pilotos do grid; a
    // variação por piloto vem só da forma recente ("algo a provar"). Calculado uma vez.
    let venue_ctx = EventInterestContext {
        categoria: race_entry.categoria.clone(),
        season_phase: race_entry.season_phase,
        rodada: race_entry.rodada,
        total_rodadas: category.corridas_por_temporada as i32,
        week_of_year: race_entry.week_of_year,
        track_id: race_entry.track_id as i32,
        track_name: race_entry.track_name.clone(),
        is_player_event: false,
        player_championship_position: None,
        player_media: None,
        championship_gap_to_leader: None,
        is_title_decider_candidate: false,
        thematic_slot: race_entry.thematic_slot,
    };
    let event_stakes = crate::simulation::pressure::event_stakes_from_score(
        calculate_expected_event_interest(&venue_ctx).score as f64,
    );

    // Pressão de Duelo (Nemesis): a rivalidade pessoal do jogador é a 3ª fonte de pressão
    // (clutch/choke via pressure.rs), SIMÉTRICA entre jogador e rival e só acesa quando os
    // dois vão brigar de verdade (gate de pace). Substitui o antigo bônus fixo de skill
    // (+2/+1), que não passava pela máquina de resiliência. Só vale com o jogador nesta
    // corrida. Mapa driver_id -> (percebida do par com o jogador, pace do oponente p/ o gate).
    let duel_by_driver: std::collections::HashMap<String, (f64, f64)> = {
        let mut m = std::collections::HashMap::new();
        if let Some(player) = driver_pool.iter().find(|d| d.is_jogador) {
            let player_id = player.id.clone();
            let player_skill = player.atributos.skill;
            let base_skill: std::collections::HashMap<String, f64> = driver_pool
                .iter()
                .map(|d| (d.id.clone(), d.atributos.skill))
                .collect();
            let current =
                crate::db::queries::player_nemesis::get_current_nemesis(&db.conn).unwrap_or(None);
            let interests =
                crate::commands::career::select_player_interests(&db.conn, current.as_deref());
            // Lado do jogador: sente o duelo contra o rival presente mais quente (o Nemesis
            // se estiver na corrida). Guardamos a percebida e o pace desse rival.
            let mut player_strongest: Option<(f64, f64)> = None;
            for r in interests.nemesis.into_iter().chain(interests.rivais) {
                // Só rivais realmente presentes nesta corrida.
                let Some(&rival_skill) = base_skill.get(&r.driver_id) else {
                    continue;
                };
                // Lado do rival: sente o duelo contra o jogador.
                m.insert(r.driver_id.clone(), (r.perceived, player_skill));
                if player_strongest.map_or(true, |(p, _)| r.perceived > p) {
                    player_strongest = Some((r.perceived, rival_skill));
                }
            }
            if let Some(info) = player_strongest {
                m.insert(player_id, info);
            }
        }
        m
    };

    let mut orphaned_drivers = Vec::new();
    let sim_drivers: Vec<SimDriver> = driver_pool
        .into_iter()
        .filter_map(|driver| match team_by_driver.get(&driver.id) {
            Some(team) => {
                let mut sd =
                    SimDriver::from_driver_team_and_track(driver, team, race_entry.track_id);
                // Conhecimento de pista: pista nova/pouco conhecida = mais lento.
                // Desconta do skill E do ritmo de classificação (corrida + quali).
                let pen =
                    track_penalty_for(driver, race_entry.track_id, active_season.numero as i32);
                sd.skill = (sd.skill as f64 - pen).max(5.0).round() as u8;
                sd.ritmo_classificacao =
                    (sd.ritmo_classificacao as f64 - pen).max(5.0).round() as u8;
                // Adaptação à categoria: quem acabou de subir corre abaixo do próprio
                // número nas primeiras etapas e vai se ajustando. Freia o campeão da
                // categoria de baixo que chegava atropelando a de cima já na estreia.
                let adapt_pen = crate::simulation::category_adaptation::category_adaptation_penalty(
                    driver.corridas_na_categoria,
                    driver.atributos.adaptabilidade,
                    driver.atributos.experiencia,
                );
                sd.skill = (sd.skill as f64 - adapt_pen).max(5.0).round() as u8;
                sd.ritmo_classificacao = (sd.ritmo_classificacao as f64 - adapt_pen)
                    .max(5.0)
                    .round() as u8;
                // Lesão: o machucado CORRE, mas com o ritmo reduzido. A penalidade é cheia
                // logo após a batida e DIMINUI a cada etapa até sarar (volta enferrujado e
                // vai melhorando). Fração do skill × (corridas restantes / total). Liga os
                // campos skill_penalty da lesão, antes só gravados e nunca lidos.
                if let Some(injury) = injuries_by_driver.get(&driver.id) {
                    let recovery = (injury.races_remaining as f64
                        / injury.races_total.max(1) as f64)
                        .clamp(0.0, 1.0);
                    let inj_pen = sd.skill as f64 * injury.skill_penalty * recovery;
                    sd.skill = (sd.skill as f64 - inj_pen).max(5.0).round() as u8;
                    sd.ritmo_classificacao =
                        (sd.ritmo_classificacao as f64 - inj_pen).max(5.0).round() as u8;
                }
                // (Chuva NÃO entra aqui: a sim já aplica rain_skill_penalty no score
                // em simulation/race.rs + qualifying.rs — aplicar de novo dobraria.)
                // Motivação: um piloto desmotivado corre ABAIXO do próprio skill (efeito
                // modesto e calibrável). Aplicado ao skill efetivo antes da pressão, para
                // o headroom já enxergar o rendimento real do piloto.
                let mot = crate::simulation::pressure::motivation_pace_delta(sd.motivacao);
                sd.skill = (sd.skill as f64 + mot).clamp(5.0, 100.0).round() as u8;
                sd.ritmo_classificacao =
                    (sd.ritmo_classificacao as f64 + mot).clamp(5.0, 100.0).round() as u8;
                // Pressão de campeonato (clutch/choke): ajusta ritmo + taxa de erro.
                let pctx = crate::simulation::pressure::title_context(
                    driver.stats_temporada.pontos,
                    &title_points,
                    races_left,
                    max_race_points,
                );
                let title_eff = crate::simulation::pressure::pressure_for(
                    &pctx,
                    races_left,
                    driver.atributos.mentalidade,
                    driver.atributos.experiencia,
                );
                // Pressão de casa cheia (universal): pesa mais em quem vem em má fase.
                let event_eff = crate::simulation::pressure::event_pressure_for(
                    event_stakes,
                    recent_avg_finish(&driver.ultimos_resultados),
                    driver.atributos.mentalidade,
                    driver.atributos.experiencia,
                );
                // Pressão de Duelo (Nemesis): 3ª fonte, acesa só quando jogador e rival
                // brigam de verdade (gate de pace). Herda mentalidade/experiência — não é
                // mais um bônus fixo. Percebida + pace do oponente vêm de `duel_by_driver`.
                let duel_eff = match duel_by_driver.get(&driver.id) {
                    Some(&(perceived, opponent_skill)) => {
                        crate::simulation::pressure::rivalry_pressure_for(
                            perceived,
                            sd.skill as f64,
                            opponent_skill,
                            driver.atributos.mentalidade,
                            driver.atributos.experiencia,
                            1.0,
                        )
                    }
                    None => crate::simulation::pressure::PressureEffect::NONE,
                };
                let peff = crate::simulation::pressure::combine(
                    crate::simulation::pressure::combine(title_eff, event_eff),
                    duel_eff,
                );
                // Headroom: converte o Δpace da pressão em pontos de skill conforme onde
                // o piloto está na curva (subir tem teto, cair tem chão). Usa o skill
                // efetivo do fim de semana (já com a penalidade de conhecimento de pista).
                let hr = crate::simulation::pressure::headroom_pace_mult(
                    sd.skill as f64,
                    peff.pace_delta >= 0.0,
                );
                let pace_delta = peff.pace_delta * hr;
                sd.skill = (sd.skill as f64 + pace_delta).clamp(5.0, 100.0).round() as u8;
                sd.ritmo_classificacao = (sd.ritmo_classificacao as f64 + pace_delta)
                    .clamp(5.0, 100.0)
                    .round() as u8;
                sd.pressure_error_mult = peff.error_mult;
                Some(sd)
            }
            None if persistence_mode == RacePersistenceMode::HistoricalDraft => None,
            None => {
                orphaned_drivers.push(format!("{} ({})", driver.nome, driver.id));
                None
            }
        })
        .collect();

    if !orphaned_drivers.is_empty() {
        return Err(format!(
            "Pilotos ativos sem equipe na categoria '{}': {}",
            race_entry.categoria,
            orphaned_drivers.join(", ")
        ));
    }

    if sim_drivers.is_empty() {
        return Err(format!(
            "Nenhum piloto ativo encontrado para a categoria '{}' na rodada {}. \
             A corrida nao foi disputada.",
            race_entry.categoria, race_entry.rodada
        ));
    }

    let ctx = SimulationContext::from_calendar_entry(
        race_entry,
        category.tier,
        race_entry.rodada >= category.corridas_por_temporada as i32,
    );
    let mut rng = rand::thread_rng();
    let catalog = IncidentCatalog::load(&db.conn).unwrap_or_else(|_| IncidentCatalog::empty());

    // QUEBRA DE PEÇA (Fase 7): pré-rola o grid inteiro sobre o desgaste REAL dos carros e deixa
    // a simulação cobrar o preço. Só na carreira jogável — o rascunho histórico reconstrói
    // temporadas antigas com grid sintético, onde não existe `team_car` pra ler. Quando roda,
    // ela vira a FONTE ÚNICA de pane: a mecânica genérica do catálogo é desligada na simulação.
    let prerolled = if persistence_mode == RacePersistenceMode::Playable {
        preroll_simulated_breakdowns(
            &db.conn,
            &career_id_from_db(db),
            race_entry,
            category,
            ctx.total_laps,
            &sim_drivers,
            &teams,
        )
    } else {
        None
    };
    let mechanicals: Option<Vec<crate::simulation::race::MechanicalOutcome>> = prerolled
        .as_ref()
        .map(|list| list.iter().map(|p| p.outcome.clone()).collect());

    let mut result = run_full_race_with_breakdowns(
        &sim_drivers,
        &ctx,
        category.id == "endurance",
        &catalog,
        mechanicals.as_deref(),
        &mut rng,
    );
    if is_multiclass_category(&race_entry.categoria) {
        apply_special_class_scoring(&mut result, &teams, category.id == "endurance");
    }
    let next_round = if advance_player_round {
        Some((active_season.rodada_atual + 1).min(category.corridas_por_temporada as i32))
    } else {
        None
    };

    // Só o que a corrida de fato COBROU: quem já tinha abandonado por batida antes da volta da
    // quebra não conta (ver `RaceResult::applied_mechanicals`). Daqui saem as três consequências.
    let applied: Vec<&PrerolledBreakdown> = match prerolled.as_ref() {
        Some(list) => result
            .applied_mechanicals
            .iter()
            .filter_map(|&i| list.get(i))
            .collect(),
        None => Vec::new(),
    };

    // Abandono POR QUEBRA precisa apontar pro catálogo `Mechanical`: é por esse id que o
    // histórico de DNF e o beat de Abandono da notícia reconhecem a pane. O `dnf_reason` a
    // simulação já gravou com a frase da peça. MESMO tratamento do import do iRacing.
    if applied.iter().any(|p| p.severity == crate::car::breakdown::Severity::Dnf) {
        let mech_id: Option<String> = db
            .conn
            .query_row(
                "SELECT id FROM incident_catalog WHERE incident_source = 'Mechanical' LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok();
        if let Some(id) = mech_id {
            for p in applied
                .iter()
                .filter(|p| p.severity == crate::car::breakdown::Severity::Dnf)
            {
                if let Some(dr) = result
                    .race_results
                    .iter_mut()
                    .find(|d| d.pilot_id == p.row.driver_id && d.is_dnf)
                {
                    dr.dnf_catalog_id = Some(id.clone());
                }
            }
        }
    }

    // Feedback físico da quebra (§4.6), por TIME: Leve segue, Grave vira fim de vida, DNF
    // destrói a peça (troca forçada a débito no fechamento da fatura). É o que fecha o laço
    // quebra → economia, igual à corrida ao vivo.
    let team_breakdowns: HashMap<
        String,
        Vec<(crate::car::PartType, crate::car::breakdown::Severity)>,
    > = {
        let mut map: HashMap<String, Vec<_>> = HashMap::new();
        for p in &applied {
            map.entry(p.team_id.clone()).or_default().push((p.part, p.severity));
        }
        map
    };

    let new_injuries_out = persist_race_result_tx(
        db,
        race_entry,
        &result,
        &teams,
        &active_season,
        category,
        next_round,
        persistence_mode,
        None, // sim offline: sem inputs do jogador → sem estilo
        0,    // sim offline: sem paradas reais (a IA modela pela duração)
        &team_breakdowns,
        &mut rng,
    )?;

    // Registra os desfechos pra tela pós-corrida e pra notícia. Best-effort: uma falha aqui não
    // desfaz o resultado já persistido — a corrida aconteceu, só o detalhe da peça se perde.
    if !applied.is_empty() {
        let rows: Vec<_> = applied.iter().map(|p| p.row.clone()).collect();
        warn_if_side_effect_fails(
            crate::db::queries::race_breakdowns::insert_breakdowns_batch(
                &db.conn,
                &race_entry.id,
                &rows,
            )
            .map_err(|e| e.to_string()),
            "Falha ao gravar race_breakdowns da corrida simulada",
        );
    }

    Ok((result, new_injuries_out))
}

pub(super) fn simulate_other_categories(
    db: &mut Database,
    career_dir: &Path,
    player_category: &str,
    target_week: i32,
    target_display_date: &str,
    season_id: &str,
    season_number: i32,
) -> Result<SimultaneousResults, String> {
    let mut categories_simulated = Vec::new();
    let mut total_races_simulated = 0;
    let mut highlights = Vec::new();
    let is_last_player_race = calendar_queries::get_next_race(&db.conn, season_id, player_category)
        .map_err(|e| {
            format!(
                "Falha ao verificar proximas corridas restantes de {}: {e}",
                player_category
            )
        })?
        .is_none();

    for category in get_all_categories() {
        if category.id == player_category {
            continue;
        }

        // Busca corridas pendentes com season_week <= target_week, em ordem cronológica.
        // Categorias especiais são excluídas naturalmente durante o BlocoRegular
        // até a abertura da janela especial de setembro, e incluídas no BlocoEspecial.
        let races_to_simulate = if is_last_player_race {
            calendar_queries::get_pending_races_up_to_week(
                &db.conn,
                season_id,
                category.id,
                i32::MAX,
            )
            .map_err(|e| {
                format!(
                    "Falha ao buscar todas as corridas pendentes de {}: {e}",
                    category.id
                )
            })?
        } else {
            calendar_queries::get_pending_races_up_to_week(
                &db.conn,
                season_id,
                category.id,
                target_week,
            )
            .map_err(|e| format!("Falha ao buscar corridas pendentes de {}: {e}", category.id))?
        };

        if races_to_simulate.is_empty() {
            continue;
        }

        let mut summaries = Vec::new();
        for entry in races_to_simulate {
            let (result, ai_injuries) = simulate_category_race(db, &entry, false)?;
            // Fama do MUNDO: astros da IA nascem nas outras categorias também. Usa o
            // interesse "de local" (sem jogador). Best-effort — não quebra o sim.
            let _ = apply_post_race_fame(&db.conn, &entry, &result, &ai_injuries);
            warn_if_side_effect_fails(
                append_race_result(
                    career_dir,
                    &entry.categoria,
                    entry.rodada,
                    &result.race_results,
                ),
                "Falha ao gravar race_results.json de outra categoria",
            );
            warn_if_side_effect_fails(
                track_history_queries::record_race_dnfs(
                    &db.conn,
                    &result.race_results,
                    &entry.track_name,
                    season_number,
                    entry.rodada,
                )
                .map_err(|e| {
                    format!("Falha ao registrar historico de DNF de outra categoria: {e}")
                }),
                "Falha ao registrar historico de DNF de outra categoria",
            );

            let is_visible_to_player = if is_last_player_race {
                !target_display_date.trim().is_empty() && entry.display_date == target_display_date
            } else {
                true
            };
            if !is_visible_to_player {
                continue;
            }

            let winner = result
                .race_results
                .iter()
                .find(|driver| driver.finish_position == 1);
            summaries.push(BriefRaceResult {
                race_id: entry.id.clone(),
                track_name: entry.track_name.clone(),
                winner_name: winner
                    .map(|driver| driver.pilot_name.clone())
                    .unwrap_or_default(),
                winner_team: winner
                    .map(|driver| driver.team_name.clone())
                    .unwrap_or_default(),
            });
            total_races_simulated += 1;
        }

        if summaries.is_empty() {
            continue;
        }

        if let Some(last) = summaries.last() {
            highlights.push(SimHighlight {
                headline: format!(
                    "{} vence em {} ({})",
                    last.winner_name, last.track_name, category.nome_curto
                ),
                category: category.id.to_string(),
            });
        }

        let races_simulated = summaries.len() as i32;
        categories_simulated.push(CategorySimResult {
            category_id: category.id.to_string(),
            category_name: category.nome.to_string(),
            races_simulated,
            results: summaries,
        });
    }

    warn_if_side_effect_fails(
        persist_other_category_news(&db.conn, &highlights, season_number),
        "Falha ao persistir noticias de outras categorias",
    );

    Ok(SimultaneousResults {
        categories_simulated,
        total_races_simulated,
        highlights,
    })
}

/// Penalidade de skill (pontos) por não conhecer a pista, a partir do histórico do
/// piloto. Mesma lógica usada no export pro iRacing (fonte única).
pub(super) fn track_penalty_for(driver: &Driver, track_id: u32, season: i32) -> f64 {
    use crate::simulation::track_knowledge;
    let knowledge = track_knowledge::from_history(&driver.historico_circuitos, track_id as i64);
    let length = crate::constants::tracks::get_track(track_id)
        .map(|t| t.comprimento_km)
        .unwrap_or(3.0);
    track_knowledge::track_knowledge_penalty(
        &knowledge,
        length,
        driver.atributos.adaptabilidade,
        season,
    )
}

#[allow(clippy::too_many_arguments)]
/// Fator de cautela mecânica por nº de DNFs mecânicos recentes do time (< 1.0 = poupa mais).
/// 1 DNF → 0.90 · 2 → 0.83 · 3+ → 0.78. Assimétrico com a economia do jogador (piso 0.75).
pub(super) fn mechanical_caution_factor(mechanical_dnfs: u32) -> f64 {
    match mechanical_dnfs {
        0 => 1.0,
        1 => 0.90,
        2 => 0.83,
        _ => 0.78,
    }
}
