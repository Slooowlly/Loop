//! Geração do AI roster (grade da carreira → iRacing): números fixos, ponteiro da temporada e o roster.json.

use super::*;

// ─── Geração de AI roster (carreira → iRacing) ───────────────────────────────

/// Resultado da geração de roster (para a UI).
#[derive(serde::Serialize)]
pub struct RosterGenResult {
    pub path: String,
    pub drivers: usize,
}

/// Caminho do mapa de números fixos de uma carreira.
pub(crate) fn numbers_path(base_dir: &std::path::Path, career_id: &str) -> std::path::PathBuf {
    base_dir
        .join("iracing_numbers")
        .join(format!("{career_id}.json"))
}

/// "Post-it" do import: aponta para o arquivo de aiseason exportado e o mapa
/// evento→corrida da carreira. Gravado no export, lido no import. Por carreira.
pub(crate) fn season_pointer_path(base_dir: &std::path::Path, career_id: &str) -> Option<std::path::PathBuf> {
    Some(
        base_dir
            .join("iracing_pointers")
            .join(format!("{career_id}.json")),
    )
}

/// Garante um número FIXO por piloto na temporada: carrega o mapa salvo, atribui
/// o menor número livre (1..) aos pilotos novos e persiste. Números vinculados ao
/// piloto não mudam entre as rodadas.
pub(crate) fn ensure_driver_numbers(
    base_dir: &std::path::Path,
    career_id: &str,
    driver_ids: &[String],
) -> Result<std::collections::HashMap<String, i64>, String> {
    use std::collections::{HashMap, HashSet};

    let path = numbers_path(base_dir, career_id);
    let mut map: HashMap<String, i64> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let mut used: HashSet<i64> = map.values().copied().collect();
    // Ordem estável (por id) para a atribuição ser determinística.
    let mut ids: Vec<&String> = driver_ids.iter().collect();
    ids.sort();
    let mut changed = false;
    for id in ids {
        if map.contains_key(id) {
            continue;
        }
        let mut n = 1;
        while used.contains(&n) {
            n += 1;
        }
        map.insert(id.clone(), n);
        used.insert(n);
        changed = true;
    }

    if changed {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
        }
        let json = serde_json::to_string_pretty(&map)
            .map_err(|e| format!("Falha ao serializar números: {e}"))?;
        std::fs::write(&path, json).map_err(|e| format!("Falha ao gravar números: {e}"))?;
    }
    Ok(map)
}

/// Gera o `roster.json` da IA a partir do grid de uma categoria da carreira e o
/// grava em `Documentos/iRacing/airosters/<roster_name>/roster.json`.
#[tauri::command]
pub fn iracing_generate_roster(
    app: tauri::AppHandle,
    career_id: String,
    categoria: String,
    roster_name: String,
    car_key: String,
    // TESTE: força a PRÓXIMA corrida como molhada (chuva forte), pra ver o re-rank de
    // chuva por piloto refletido nos atributos da IA. None/false = clima normal.
    force_wet: Option<bool>,
) -> Result<RosterGenResult, String> {
    use crate::config::app_config::AppConfig;
    use crate::constants::categories::get_category_config;
    use crate::constants::scoring::{get_points_for_position, BONUS_FASTEST_LAP};
    use crate::constants::tracks::get_track;
    use crate::db::connection::Database;
    use crate::db::queries::{
        calendar as calq, contracts as cq, drivers as dq, injuries as injq, race_history as rhq,
        seasons as sq, teams as tq,
    };
    use crate::iracing_sdk::{paths, roster_gen, weather};
    use std::collections::HashMap;
    use tauri::Manager;

    // Exportar é o passo pré-corrida: já liga o monitoramento (custid, etc.).
    race_monitor::start_watching();

    let car = roster_gen::car_spec(&car_key)
        .ok_or_else(|| format!("Carro desconhecido: {car_key} (use mx5, gr86 ou bmwm2)"))?;

    // Abre o banco da carreira.
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Err(format!("Save não encontrado: {career_id}"));
    }
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    // Time do jogador (para o padrão simples nos carros do time dele).
    let player_team_id = dq::get_player_driver(&db.conn)
        .ok()
        .and_then(|p| {
            cq::get_active_contract_for_pilot(&db.conn, &p.id)
                .ok()
                .flatten()
        })
        .map(|c| c.equipe_id);

    // Grid da categoria (exclui o jogador — ele dirige, não é IA).
    let drivers = dq::get_drivers_by_category(&db.conn, &categoria)
        .map_err(|e| format!("Falha ao ler pilotos: {e}"))?;
    // Standings + skills da categoria (inclui o jogador) p/ a camada de comportamento.
    let title_points: Vec<f64> = drivers.iter().map(|d| d.stats_temporada.pontos).collect();
    let grid_skills: Vec<f64> = drivers.iter().map(|d| d.atributos.skill).collect();
    // Temporada atual + tier da categoria (contrato no último ano, promo/rebaixa).
    let season_num: i32 = sq::get_active_season(&db.conn)
        .ok()
        .flatten()
        .map(|s| s.numero as i32)
        .unwrap_or(0);
    let current_tier = get_category_config(&categoria)
        .map(|c| c.tier as i32)
        .unwrap_or(0);
    // Campeão reinante: quem venceu a categoria na temporada PASSADA (numero-1) →
    // defende o título nesta. Vazio na 1ª temporada (sem passado).
    let prev_champion_id: Option<String> = sq::get_all_seasons(&db.conn)
        .ok()
        .and_then(|all| all.into_iter().find(|s| s.numero as i32 == season_num - 1))
        .and_then(|prev| {
            rhq::get_category_champion_for_season(&db.conn, &prev.id, &categoria)
                .ok()
                .flatten()
        });

    let mut entries = Vec::new();
    // Pontos por time (inclui o jogador) p/ o duelo interno; ctx por piloto (Tier 2B).
    let mut team_members: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    let mut driver_ctx: HashMap<String, roster_gen::DriverCtx> = HashMap::new();
    for driver in &drivers {
        let contract = cq::get_active_contract_for_pilot(&db.conn, &driver.id)
            .ok()
            .flatten();
        let team = contract
            .as_ref()
            .and_then(|c| tq::get_team_by_id(&db.conn, &c.equipe_id).ok().flatten());
        if let Some(c) = &contract {
            team_members
                .entry(c.equipe_id.clone())
                .or_default()
                .push((driver.id.clone(), driver.stats_temporada.pontos));
        }
        if driver.is_jogador {
            continue;
        }
        let category_move = team
            .as_ref()
            .and_then(|t| t.categoria_anterior.as_deref())
            .and_then(get_category_config)
            .map(|prev| (current_tier - prev.tier as i32).signum())
            .unwrap_or(0);
        // Contra a ex-equipe: chegou ao time atual NESTA temporada E já teve OUTRO time
        // antes (não é rookie no 1º time nem re-assinatura). Rivalidade com o passado.
        let switched_teams = contract
            .as_ref()
            .map(|cur| {
                cur.temporada_inicio == season_num
                    && cq::get_contracts_for_pilot(&db.conn, &driver.id)
                        .unwrap_or_default()
                        .iter()
                        .any(|p| {
                            p.temporada_inicio < cur.temporada_inicio && p.equipe_id != cur.equipe_id
                        })
            })
            .unwrap_or(false);
        driver_ctx.insert(
            driver.id.clone(),
            roster_gen::DriverCtx {
                contract_last_year: contract
                    .as_ref()
                    .map(|c| c.temporada_fim <= season_num)
                    .unwrap_or(false),
                teammate_points: None, // preenchido após o loop
                injury_return: false,  // preenchidos após resolver a próxima corrida
                crashed_out_last_race: false,
                not_at_fault_dnfs: 0,
                track_crash: false,
                nemesis: false,        // preenchido após resolver a próxima corrida
                mechanical_dnfs: 0,    // idem
                switched_teams,
                reigning_champion: prev_champion_id.as_deref() == Some(driver.id.as_str()),
                career_debut: driver.stats_carreira.corridas == 0,
                honeymoon: contract
                    .as_ref()
                    .map(|c| c.temporada_inicio == season_num)
                    .unwrap_or(false),
                category_move,
                team_morale: team.as_ref().map(|t| t.morale).unwrap_or(1.0),
                // Vínculo com a equipe atual (selo de 6 níveis). Sem contrato → recém-chegado (1).
                bond_level: contract
                    .as_ref()
                    .map(|c| {
                        crate::market::bond::bond_level(
                            crate::market::bond::get_bond(&db.conn, &driver.id, &c.equipe_id)
                                .unwrap_or(0.0),
                        )
                    })
                    .unwrap_or(1),
                injury_active_penalty: 0.0, // preenchido após resolver a próxima corrida
            },
        );
        let team_info = team.map(|team| roster_gen::TeamInfo {
            is_player_team: player_team_id.as_deref() == Some(team.id.as_str()),
            team_id: team.id,
            color: team.cor_primaria,
            color2: team.cor_secundaria,
            pit_crew: team.pit_crew_quality,
            strategy: team.pit_strategy_risk,
        });
        entries.push((driver.clone(), team_info));
    }
    // Duelo interno: melhor pontuação de OUTRO membro do mesmo time.
    for members in team_members.values() {
        for (id, _) in members {
            if let Some(ctx) = driver_ctx.get_mut(id) {
                let best_other = members
                    .iter()
                    .filter(|(oid, _)| oid != id)
                    .map(|(_, p)| *p)
                    .fold(f64::MIN, f64::max);
                if best_other > f64::MIN {
                    ctx.teammate_points = Some(best_other);
                }
            }
        }
    }
    if entries.is_empty() {
        return Err(format!("Nenhum piloto de IA na categoria '{categoria}'."));
    }

    // Números FIXOS por piloto na temporada: carrega o mapa salvo, atribui os
    // que faltam (menor número livre) e persiste.
    let driver_ids: Vec<String> = entries.iter().map(|(d, _)| d.id.clone()).collect();
    let numbers = ensure_driver_numbers(&base_dir, &career_id, &driver_ids)?;

    // Próxima corrida pendente do calendário → pista alvo (conhecimento de pista) +
    // pressão de campeonato. Ambos a MESMA lógica da simulação offline.
    let next = sq::get_active_season(&db.conn)
        .ok()
        .flatten()
        .and_then(|season| {
            calq::get_next_race(&db.conn, &season.id, &categoria)
                .ok()
                .flatten()
                .map(|race| (season, race))
        });
    // Percentil no ranking MUNDIAL por piloto (uma vez; falha → vazio = neutro 0.5).
    let global_percentile: HashMap<String, f64> =
        match crate::commands::global_driver_rankings::get_global_driver_rankings_in_base_dir(
            &base_dir, &career_id, None,
        ) {
            Ok(payload) => {
                let total = payload.rows.len();
                payload
                    .rows
                    .iter()
                    .map(|r| {
                        let pct = if total <= 1 {
                            0.5
                        } else {
                            1.0 - (r.historical_rank as f64 - 1.0) / (total as f64 - 1.0)
                        };
                        (r.id.clone(), pct)
                    })
                    .collect()
            }
            Err(_) => HashMap::new(),
        };

    // Retorno de lesão: lesão já sarada NESTA temporada que terminou nas últimas
    // corridas (return_round = rodada do acidente + duração) → cautela.
    if let Some((season, race)) = next.as_ref() {
        let ids: Vec<String> = driver_ctx.keys().cloned().collect();
        for pilot_id in ids {
            let Ok(Some(inj)) = injq::get_last_injury_for_pilot(&db.conn, &pilot_id) else {
                continue;
            };
            // Lesão ATIVA (ainda em recuperação): o piloto CORRE, mas com o pace reduzido pela
            // MESMA rampa da sim (skill × penalidade × corridas_restantes/total, que decai a cada
            // etapa). Antes só o bool `injury_return` (já sarado) cruzava — a penalidade em si
            // não tinha equivalente no roster. Exporta a fração perdida (0–1).
            if inj.active {
                let recovery =
                    (inj.races_remaining as f64 / inj.races_total.max(1) as f64).clamp(0.0, 1.0);
                let frac = (inj.skill_penalty * recovery).clamp(0.0, 1.0);
                if let Some(ctx) = driver_ctx.get_mut(&pilot_id) {
                    ctx.injury_active_penalty = frac;
                }
                continue;
            }
            if inj.season != season.numero as i32 {
                continue;
            }
            if let Ok(Some(entry)) = calq::get_calendar_entry_by_id(&db.conn, &inj.race_occurred) {
                let since = race.rodada - (entry.rodada + inj.races_total);
                if (0..=2).contains(&since) {
                    if let Some(ctx) = driver_ctx.get_mut(&pilot_id) {
                        ctx.injury_return = true;
                    }
                }
            }
        }
        // Vingança / azar / desconfiança mecânica: DNFs por FONTE nas últimas 3 rodadas.
        // Fontes disjuntas: DriverError = culpa própria (ignora); Mechanical/Operational =
        // carro quebrou (desconfiança, poupa); resto (PostCollision etc.) = tirado/azar
        // (frustração). Nêmesis = cruzou a linha lado a lado com o mesmo rival ≥2 vezes.
        let mut last_crashout: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut bad_luck: HashMap<String, u32> = HashMap::new();
        let mut mechanical: HashMap<String, u32> = HashMap::new();
        let mut adjacency: HashMap<String, HashMap<String, u32>> = HashMap::new();
        for back in 1..=3 {
            let round = race.rodada - back;
            if round < 1 {
                break;
            }
            if let Ok(facts) =
                rhq::get_dnf_incident_facts_for_round(&db.conn, &season.id, &categoria, round)
            {
                for (pid, source, dnf, _seg) in facts {
                    if !dnf {
                        continue;
                    }
                    match source.as_deref() {
                        Some("DriverError") => {} // culpa própria: nem azar nem desconfiança
                        Some("Mechanical") | Some("Operational") => {
                            *mechanical.entry(pid).or_default() += 1;
                        }
                        src => {
                            // tirado de corrida (PostCollision) na última = vingança.
                            if back == 1 && src == Some("PostCollision") {
                                last_crashout.insert(pid.clone());
                            }
                            *bad_luck.entry(pid).or_default() += 1;
                        }
                    }
                }
            }
            // Nêmesis: rivais que terminaram em posições vizinhas (±1 no grid de chegada).
            if let Ok(rows) = rhq::get_results_for_round(&db.conn, &season.id, &categoria, round) {
                let mut finishers: Vec<(String, i32)> = rows
                    .into_iter()
                    .filter(|(_, _, fin, dnf)| !dnf && *fin > 0)
                    .map(|(id, _, fin, _)| (id, fin))
                    .collect();
                finishers.sort_by_key(|(_, fin)| *fin);
                for i in 0..finishers.len() {
                    for j in [i.wrapping_sub(1), i + 1] {
                        if j < finishers.len() && j != i {
                            *adjacency
                                .entry(finishers[i].0.clone())
                                .or_default()
                                .entry(finishers[j].0.clone())
                                .or_default() += 1;
                        }
                    }
                }
            }
        }
        // Trauma de pista: já bateu (DriverError/PostCollision) na pista alvo.
        let track_crash_set =
            rhq::get_track_crash_pilots(&db.conn, race.track_id).unwrap_or_default();
        for (id, ctx) in driver_ctx.iter_mut() {
            ctx.crashed_out_last_race = last_crashout.contains(id);
            ctx.not_at_fault_dnfs = bad_luck.get(id).copied().unwrap_or(0);
            ctx.mechanical_dnfs = mechanical.get(id).copied().unwrap_or(0);
            ctx.track_crash = track_crash_set.contains(id);
            ctx.nemesis = adjacency
                .get(id)
                .map(|rivals| rivals.values().any(|&c| c >= 2))
                .unwrap_or(false);
        }
    }

    let behavior_ctx = next.as_ref().and_then(|(season, race)| {
        let track = get_track(race.track_id)?;
        // Clima da próxima corrida (mesma geração determinística da season).
        let mut story = weather::generate_weather(
            month_from_week(race.week_of_year),
            track_hemisphere(track.pais),
            climate_tendency(track.rain_group),
            event_seed(&career_id, &race.id),
            false,
        );
        // TESTE: força chuva forte na próxima corrida.
        if force_wet.unwrap_or(false) {
            story.is_wet_race = true;
            story.race_intensity = weather::RainIntensity::Heavy;
            story.scenario = weather::WeatherScenario::SteadyRain;
        }
        let rain_intensity = match story.race_intensity {
            weather::RainIntensity::None => 0.0,
            weather::RainIntensity::Light => 0.35,
            weather::RainIntensity::Decent => 0.55,
            weather::RainIntensity::Heavy => 0.8,
            weather::RainIntensity::VeryHeavy => 1.0,
        };
        // Forma: posições finais das até 3 rodadas concluídas anteriores.
        let mut recent_positions: HashMap<String, Vec<u32>> = HashMap::new();
        for back in 1..=3 {
            let round = race.rodada - back;
            if round < 1 {
                break;
            }
            if let Ok(rows) = rhq::get_results_for_round(&db.conn, &season.id, &categoria, round) {
                for (did, _larg, fin, _dnf) in rows {
                    recent_positions
                        .entry(did)
                        .or_default()
                        .push(fin.max(1) as u32);
                }
            }
        }
        let total = get_category_config(&categoria)
            .map(|c| c.corridas_por_temporada as i32)
            .unwrap_or(race.rodada);
        // Casa cheia: interesse "de local" do evento (sem protagonismo do jogador nem
        // drama de título — esses entram pela pressão de campeonato). MESMA fonte da sim.
        let venue_ctx = crate::event_interest::EventInterestContext {
            categoria: categoria.clone(),
            season_phase: race.season_phase,
            rodada: race.rodada,
            total_rodadas: total,
            week_of_year: race.week_of_year,
            track_id: race.track_id as i32,
            track_name: race.track_name.clone(),
            is_player_event: false,
            player_championship_position: None,
            player_media: None,
            championship_gap_to_leader: None,
            is_title_decider_candidate: false,
            thematic_slot: race.thematic_slot,
        };
        let event_stakes = crate::simulation::pressure::event_stakes_from_score(
            crate::event_interest::calculate_expected_event_interest(&venue_ctx).score as f64,
        );
        // Sweet spot do tier na pista alvo — MESMA âncora da curva de skill usada na
        // season. Garante que o cap da cauda (pior piloto ≥ skill real) bata dos 2 lados.
        let custid = crate::iracing_sdk::cached_custid().unwrap_or(0);
        let base_sweet = ai_sweet_spot(
            current_tier as u8,
            Some(race.track_id as i64),
            &base_dir,
            custid,
        ) as f64;
        // Sistema de Nível do Carro → dificuldade da IA (inversão: carro spec no iRacing, então
        // carro melhor só ENFRAQUECE a IA). BANDA (você vs a média do campo) rebaixa/eleva o
        // sweet inteiro; SPREAD por-IA (zero-mean) cavalga o roster. Ver `car_difficulty`.
        let (player_adv, ai_advs, per_ai_adv) = field_car_advantages(
            &db.conn,
            &categoria,
            player_team_id.as_deref(),
            race.track_id as i64,
        );
        let ai_sweet = (base_sweet
            + crate::iracing_sdk::car_difficulty::band_skill_delta(player_adv, &ai_advs))
        .clamp(0.0, 125.0);
        let field_mean = crate::iracing_sdk::car_difficulty::field_mean(&ai_advs);
        let car_spread_nudge: std::collections::HashMap<String, f64> = per_ai_adv
            .iter()
            .map(|(id, adv)| {
                (
                    id.clone(),
                    crate::iracing_sdk::car_difficulty::ai_spread_nudge(*adv, field_mean),
                )
            })
            .collect();
        // Persiste o contexto de carro (número do carro → vantagem) + a vantagem do jogador,
        // pro pós-corrida descontar a FRENTE (mecanismo 2, cego ao carro). Best-effort.
        {
            let by_number: std::collections::HashMap<String, f64> = per_ai_adv
                .iter()
                .filter_map(|(id, adv)| numbers.get(id).map(|n| (n.to_string(), *adv)))
                .collect();
            let _ = save_car_difficulty_context(
                &base_dir,
                custid,
                &CarDifficultyContext {
                    track_id: race.track_id as i64,
                    player_advantage: player_adv,
                    by_number,
                },
            );
        }
        // Bônus de rivalidade (Pressão de Duelo, export): Nemesis +2 / Rivais +1 no
        // AI rival do jogador — corre mais forte contra ele na pista.
        let rival_skill_bonus: std::collections::HashMap<String, f64> = {
            let current =
                crate::db::queries::player_nemesis::get_current_nemesis(&db.conn).unwrap_or(None);
            let interests =
                crate::commands::career::select_player_interests(&db.conn, current.as_deref());
            let mut m = std::collections::HashMap::new();
            if let Some(n) = interests.nemesis {
                m.insert(n.driver_id, 2.0);
            }
            for r in interests.rivais {
                m.insert(r.driver_id, 1.0);
            }
            m
        };
        Some(roster_gen::BehaviorContext {
            current_season: season.numero as i32,
            track_id: race.track_id,
            track_length_km: track.comprimento_km,
            track_flag: crate::constants::country_label(track.pais),
            title_points: title_points.clone(),
            races_left: (total - race.rodada + 1).max(1) as u32,
            event_stakes,
            season_length: total.max(1) as u32,
            max_points: (get_points_for_position(1, categoria == "endurance") + BONUS_FASTEST_LAP)
                as f64,
            field_size: title_points.len().max(1) as u32,
            grid_skills: grid_skills.clone(),
            is_wet: story.is_wet_race,
            rain_intensity,
            rain_level: story.race_intensity,
            // Temp alinhada à MESMA história de chuva (não o placeholder do calendário).
            temp_c: weather::story_temperature(&story, event_seed(&career_id, &race.id)) as f64,
            seed_base: event_seed(&career_id, &race.id),
            recent_positions,
            global_percentile,
            driver_ctx,
            ai_sweet_spot: ai_sweet,
            car_spread_nudge,
            rival_skill_bonus,
        })
    });

    let roster = roster_gen::build_roster(&entries, &car, &numbers, behavior_ctx.as_ref(), || {
        uuid::Uuid::new_v4().to_string()
    });

    // Grava em airosters/<roster_name>/roster.json.
    let safe_name: String = roster_name
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    let safe_name = safe_name.trim();
    if safe_name.is_empty() {
        return Err("Nome do roster inválido.".to_string());
    }
    let dir = paths::airosters_dir()
        .ok_or("Não foi possível localizar a pasta airosters do iRacing.")?
        .join(safe_name);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    let path = dir.join("roster.json");
    let json =
        serde_json::to_string_pretty(&roster).map_err(|e| format!("Falha ao serializar: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Falha ao gravar: {e}"))?;

    // ── Sistema de Quebra: monta o diretor de disparo AO VIVO com o DESGASTE REAL de cada time
    // e o instala no monitor (auto no export). Durante a corrida ele dispara `!black`/`!dq`
    // conforme as peças largam. O número do JOGADOR só é conhecido ao vivo → guardamos o estado
    // dele e o monitor o vincula no verde. Best-effort: falha aqui não bloqueia o export.
    if let Some((season, race)) = next.as_ref() {
        use crate::car::breakdown::{BreakdownDirector, LiveBreakdown};
        use crate::db::queries::team_car as tcq;
        use crate::market::car_maintenance::maintenance_demand;

        let ev_seed = event_seed(&career_id, &race.id);
        // Clima da corrida — MESMA história determinística do resto do export (o "cache" do clima).
        let weather =
            race_breakdown_weather(race.track_id, race.week_of_year, ev_seed, force_wet.unwrap_or(false));
        let track_pha = maintenance_demand(&[race.track_id]);

        // Semente por carro: mistura o piloto na semente do evento → o aviso pré-corrida (pré-roll)
        // e o disparo ao vivo rolam a MESMA sorte.
        let seed_for = |driver_id: &str| -> u64 {
            let mut s = ev_seed;
            for b in driver_id.bytes() {
                s = s.wrapping_mul(0x0000_0100_0000_01B3).wrapping_add(b as u64);
            }
            s
        };

        // Enduro (corrida longa): o disparo ao vivo abranda o DNF (grid não esvazia) e agrava o
        // desgaste da metade pro fim da corrida. Gate único por duração da categoria.
        let is_enduro = get_category_config(&categoria)
            .map(|c| crate::car::breakdown::is_enduro_duration(c.duracao_corrida_min))
            .unwrap_or(false);
        // Tenda de durabilidade por nível (§4.8) só em categoria GERIDA (teto ≥ 3); spec fica de fora.
        let apply_tent = crate::car::cost::category_ceiling(&categoria) > 2;

        let mut dir = BreakdownDirector::new();
        for (driver, team_info) in &entries {
            let Some(ti) = team_info else { continue };
            let Some(num) = numbers.get(&driver.id).copied() else { continue };
            if num <= 0 {
                continue;
            }
            let Ok(Some(car)) = tcq::get_team_car(&db.conn, &ti.team_id) else {
                continue;
            };
            let live = LiveBreakdown::new(&car, seed_for(&driver.id), ti.pit_crew, track_pha)
                .with_enduro(is_enduro)
                .with_tent(apply_tent);
            dir.add_car(num as u32, live, Vec::new());
        }

        // Jogador no disparo: estado montado do carro do time dele (desgaste já ajustado pelo
        // estilo na manutenção). Vinculado ao número ao vivo no verde.
        let player_live = player_team_id.as_ref().and_then(|tid| {
            let player = dq::get_player_driver(&db.conn).ok()?;
            let car = tcq::get_team_car(&db.conn, tid).ok().flatten()?;
            let pit = tq::get_team_by_id(&db.conn, tid)
                .ok()
                .flatten()
                .map(|t| t.pit_crew_quality)
                .unwrap_or(50.0);
            Some(
                LiveBreakdown::new(&car, seed_for(&player.id), pit, track_pha)
                    .with_enduro(is_enduro)
                    .with_tent(apply_tent),
            )
        });

        // Vitrine: só na PRIMEIRA corrida de um save novo (temporada 1, rodada 1). O monitor
        // garante que o penúltimo carro (nunca o jogador) pare pra arrumar uma peça, mostrando o
        // sistema de quebra logo de cara.
        let is_first_race = season.numero == 1 && race.rodada == 1;
        race_monitor::install_breakdown_director(dir, player_live, weather, is_first_race);
    }

    Ok(RosterGenResult {
        path: path.display().to_string(),
        drivers: roster.drivers.len(),
    })
}
