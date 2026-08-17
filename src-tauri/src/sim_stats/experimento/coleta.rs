//! O laço Monte Carlo: cria as carreiras, avança as temporadas e alimenta os totais.

use crate::sim_stats::*;

pub(super) fn coletar(runs: usize, seasons: usize, start: Instant, t: &mut Totals) {
    for run in 0..runs {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        // O pid entra no nome porque o relógio do Windows anda em passos de ~15 ms:
        // dois harnesses abertos no mesmo instante pegavam o MESMO diretório e um
        // apagava o save do outro no meio da run (medido em 17/08/2026, quatro
        // processos paralelos, panic no preseason_plan.json que o irmão removeu).
        let pid = std::process::id();
        let base_dir = std::env::temp_dir().join(format!("iracer_mc_{pid}_{run}_{nanos}"));
        std::fs::create_dir_all(&base_dir).expect("base dir");

        let input = CreateCareerInput {
            player_name: "MC Bot".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(22),
            category: "mazda_rookie".to_string(),
            team_index: 0,
            difficulty: "medio".to_string(),
        };
        create_career_in_base_dir(&base_dir, input).expect("criar carreira");
        let db_path = career_db_path(&base_dir);

        // Trajetória de carreira: primeiro/último overall por piloto nesta run
        let mut traj: HashMap<String, Trajectory> = HashMap::new();
        let mut last_after: HashMap<String, DriverSnap> = HashMap::new();
        // Trajetória financeira por equipe nesta run (para medir recuperação)
        let mut team_states: HashMap<String, TeamStateTrack> = HashMap::new();
        // Trajetória de tier (carreira) por piloto nesta run
        let mut careers: HashMap<String, CareerTrack> = HashMap::new();
        // Textura de nomes do Rookie: ids já vistos nesta run e o grid rookie anterior.
        let mut seen_ever: HashSet<String> = HashSet::new();
        let mut prev_rookie: HashSet<String> = HashSet::new();
        // DIAGNÓSTICO SNOWBALL: promoções (campeonatos) por equipe nesta run, como
        // (temporada, tier_de_origem). Uma equipe que sobe a escada ganhando ano
        // após ano vira uma cadeia de (s, t), (s+1, t+1), (s+2, t+2)...
        let mut promo_by_team: HashMap<String, Vec<(usize, u8)>> = HashMap::new();
        let mut team_name_of: HashMap<String, String> = HashMap::new();
        // Ideia 1: eventos de promoção/rebaixamento por equipe nesta run, para medir
        // bounce-down (promovida cai logo) e ricochete (rebaixada volta logo).
        let mut promoted_at: Vec<(String, usize)> = Vec::new();
        let mut relegated_seasons: HashMap<String, Vec<usize>> = HashMap::new();
        // Há quantas temporadas consecutivas cada piloto está sem assento.
        let mut livre_ha: HashMap<String, u32> = HashMap::new();

        for season in 0..seasons {
            // Snapshot ANTES da temporada
            let before = snapshot_drivers(&db_path);

            // ── Textura de nomes do Rookie: estreia nova vs. retido vs. conhecido
            //    retornando. Medido no início da temporada (o grid já foi montado pela
            //    pré-temporada anterior). A 1ª temporada (mundo novo) é ignorada. ──
            let cats_now = snapshot_driver_categories(&db_path);
            let mut rookie_now: HashSet<String> = HashSet::new();
            for (id, cat) in &cats_now {
                if tier_of(cat) != 0 {
                    continue;
                }
                rookie_now.insert(id.clone());
                if season > 0 {
                    t.rookie_obs += 1;
                    if !seen_ever.contains(id) {
                        t.rookie_fresh += 1;
                    } else if prev_rookie.contains(id) {
                        t.rookie_retained += 1;
                    } else {
                        t.rookie_returning += 1;
                    }
                    if let Some(s) = before.get(id) {
                        t.rookie_age_sum += s.age.max(0) as u64;
                        t.rookie_age_n += 1;
                    }
                }
            }
            if season > 0 {
                t.rookie_season_count += 1;
            }
            for id in cats_now.keys() {
                seen_ever.insert(id.clone());
            }
            prev_rookie = rookie_now;

            // Alimenta a trajetória com o estado de início de temporada
            for (id, snap) in &before {
                traj.entry(id.clone())
                    .and_modify(|tr| {
                        tr.last_overall = snap.overall;
                        tr.seasons_seen += 1;
                    })
                    .or_insert(Trajectory {
                        first_overall: snap.overall,
                        last_overall: snap.overall,
                        first_age: snap.age,
                        seasons_seen: 1,
                    });
            }
            // ── Agentes livres: Ativo e sem contrato no INÍCIO da temporada. Perder o
            //    assento não aposenta ninguém — o piloto fica livre e disputa a janela.
            //    A regra do órfão ocioso só age depois de uma temporada inteira sem correr,
            //    e existe para o mundo não acumular agente livre eterno. Estas contagens
            //    dizem se esse acúmulo é real hoje.
            let com_assento = snapshot_seats(&db_path);
            for (id, snap) in &before {
                if com_assento.contains_key(id) {
                    livre_ha.remove(id);
                    continue;
                }
                let streak = livre_ha.entry(id.clone()).or_insert(0);
                *streak += 1;
                let e = t.free_agents_by_season.entry(season).or_insert([0.0; 3]);
                e[0] += 1.0;
                e[1] += snap.age.max(0) as f64;
                e[2] += snap.overall;
                *t.free_streak_hist.entry((*streak).min(6)).or_insert(0) += 1;
            }

            let inj_before: HashSet<String> = snapshot_injuries(&db_path).into_keys().collect();
            let active_at_start = before.len() as u64;
            t.driver_seasons += active_at_start;

            // Salários ativos no início da temporada (por tier)
            for (cat, sal) in snapshot_salaries(&db_path) {
                let tier = tier_of(&cat);
                let e = t
                    .salary_by_tier
                    .entry(tier)
                    .or_insert([0.0, 0.0, f64::INFINITY, 0.0]);
                e[0] += sal;
                e[1] += 1.0;
                e[2] = e[2].min(sal);
                e[3] = e[3].max(sal);
            }

            // Roda todas as corridas (gera resultados, lesões, finanças)
            skip_all_pending_races_in_base_dir(&base_dir, "career_001").expect("skip all pending");

            // ── Desempenho da temporada (ler ANTES do advance, que arquiva/zera temp_*) ──
            for p in snapshot_season_perf(&db_path) {
                t.total_starts += p.corridas;
                t.total_dnfs += p.dnfs;
                if p.corridas > 0 {
                    t.drivers_raced += 1;
                    match p.vitorias {
                        0 => t.win_0 += 1,
                        1..=2 => t.win_1_2 += 1,
                        3..=5 => t.win_3_5 += 1,
                        _ => t.win_6p += 1,
                    }
                    if p.podios > 0 {
                        t.with_podium += 1;
                    }
                }
                t.motiv_sum += p.motivacao;
                t.motiv_n += 1;
                if p.motivacao < 20.0 {
                    t.motiv_lt20 += 1;
                }
                // Mesma leitura, mas guardada POR TEMPORADA — ver `motiv_by_season`.
                let e = t.motiv_by_season.entry(season).or_insert([0.0; 4]);
                e[0] += p.motivacao;
                e[1] += 1.0;
                if p.motivacao >= 99.5 {
                    e[2] += 1.0;
                }
                if p.motivacao < 20.0 {
                    e[3] += 1.0;
                }
            }

            // Assentos ANTES da virada, com a motivação que cada piloto levou para
            // a decisão (a evolução só mexe nela dentro do advance, logo abaixo).
            // O par depois/antes cobre a janela do OFFSEASON — mercado de
            // pré-temporada e promoção/rebaixamento. Trocas feitas pela janela
            // semanal no meio da temporada ficam fora desta medida.
            let seats_before = snapshot_seats(&db_path);

            // Fama e público da ÚLTIMA temporada, lidos ANTES do advance. A virada
            // chama `reset_team_season_stats`, que zera `stats_pontos` — e é dos pontos
            // que sai a classificação viva de construtores. Medir depois do advance
            // daria um grid inteiro sem campeonato e mataria justamente o termo de
            // competitividade que se quer medir.
            if season + 1 == seasons {
                super::fama::coletar_fama(&db_path, t);
                super::fama::coletar_atracao(&db_path, t);
            }

            // Fecha a temporada (aplica growth/decline/lesão/aposentadoria/promoção)
            let result =
                advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");

            // ── O ânimo mexeu a agulha do mercado? ──
            let seats_after = snapshot_seats(&db_path);
            for (id, antes) in &seats_before {
                let Some(depois) = seats_after.get(id) else {
                    continue;
                };
                let e = t
                    .mercado_by_motiv
                    .entry(motiv_band(antes.motivacao))
                    .or_insert([0.0; 4]);
                e[0] += 1.0;
                if depois.team_id != antes.team_id {
                    e[1] += 1.0;
                }
                // Tier 99 é categoria desconhecida — comparar degraus com ele
                // inventaria quedas que não existiram.
                if antes.tier != 99 && depois.tier != 99 && depois.tier < antes.tier {
                    e[2] += 1.0;
                }
                if antes.salary > 0.0 {
                    e[3] += (depois.salary - antes.salary) / antes.salary * 100.0;
                }
            }

            // ── KPI anti-deflação: correlação carro↔skill por tier ──
            for (car, skill, categoria) in snapshot_grid_pairs(&db_path) {
                let m = t
                    .grid_corr_by_tier
                    .entry(tier_of(&categoria))
                    .or_insert([0.0; 6]);
                m[0] += 1.0;
                m[1] += car;
                m[2] += skill;
                m[3] += car * car;
                m[4] += skill * skill;
                m[5] += car * skill;
            }

            // ── Equipes: snapshot pós-temporada ──
            let team_snaps = snapshot_teams(&db_path);
            // Mapa id → (categoria, classe, car) para medir onde o promovido aterrissou.
            let car_by_id: HashMap<String, (String, String, f64)> = team_snaps
                .iter()
                .map(|tm| {
                    (
                        tm.id.clone(),
                        (tm.categoria.clone(), tm.classe.clone(), tm.car_performance),
                    )
                })
                .collect();
            for tm in &team_snaps {
                t.team_seasons += 1;
                *t.fin_state.entry(tm.financial_state.clone()).or_insert(0) += 1;
                *t.focus_dist.entry(tm.foco.clone()).or_insert(0) += 1;
                if tm.cash_balance < 0.0 || tm.debt_balance > 0.0 {
                    t.team_insolvent += 1;
                }
                t.cash_sum += tm.cash_balance;
                t.debt_sum += tm.debt_balance;
                let tier = tier_of(&tm.categoria);
                let e = t.car_perf_by_tier.entry(tier).or_insert([0.0, 0.0]);
                e[0] += tm.car_performance;
                e[1] += 1.0;
                // Nível do Carro (1–10) por categoria.
                let cl = t
                    .car_level_by_category
                    .entry(tm.categoria.clone())
                    .or_insert([0.0, 0.0, f64::MAX, f64::MIN]);
                cl[0] += tm.car_level as f64;
                cl[1] += 1.0;
                cl[2] = cl[2].min(tm.car_level as f64);
                cl[3] = cl[3].max(tm.car_level as f64);
                // Distribuição por peça + foco do carro (só categorias não-spec: teto > 1).
                if let Some(car) = &tm.car {
                    if crate::car::cost::category_ceiling(&tm.categoria) > 1 {
                        let focus = classify_shape(car);
                        for part in &car.parts {
                            let pe = t
                                .part_level_by_type
                                .entry(part.part_type.as_str().to_string())
                                .or_insert([0.0, 0.0]);
                            pe[0] += part.level as f64;
                            pe[1] += 1.0;
                            let fe = t
                                .part_level_by_focus
                                .entry(format!("{focus}|{}", part.part_type.as_str()))
                                .or_insert([0.0, 0.0]);
                            fe[0] += part.level as f64;
                            fe[1] += 1.0;
                        }
                        *t.shape_focus.entry(focus.to_string()).or_insert(0) += 1;
                    }
                }
                let r = t
                    .rep_by_tier
                    .entry(tier)
                    .or_insert([0.0, 0.0, f64::MAX, f64::MIN, 0.0]);
                r[0] += tm.reputacao;
                r[1] += tm.reputacao * tm.reputacao;
                r[2] = r[2].min(tm.reputacao);
                r[3] = r[3].max(tm.reputacao);
                r[4] += 1.0;
                t.team_attr_sum[0] += tm.facilities;
                t.team_attr_sum[1] += tm.engineering;
                t.team_attr_sum[2] += tm.reputacao;
                t.team_attr_sum[3] += tm.morale;
                t.team_attr_sum[4] += tm.confiabilidade;
                let md = &mut t.morale_dist;
                if md[4] == 0.0 {
                    md[2] = tm.morale;
                    md[3] = tm.morale;
                }
                md[0] += tm.morale;
                md[1] += tm.morale * tm.morale;
                md[2] = md[2].min(tm.morale);
                md[3] = md[3].max(tm.morale);
                md[4] += 1.0;

                // Trajetória financeira: detecta colapso e recuperação posterior
                let rank = state_rank(&tm.financial_state);
                let track = team_states.entry(tm.id.clone()).or_default();
                track.final_state_rank = rank;
                if tm.financial_state == "collapse" {
                    track.seasons_in_collapse += 1;
                    if !track.ever_collapse {
                        track.ever_collapse = true;
                        track.first_collapse_season = Some(season);
                    }
                } else if track.ever_collapse {
                    // Já colapsou antes e agora está fora do colapso
                    track.escaped = true;
                    if rank >= state_rank("stable") && !track.recovered {
                        track.recovered = true;
                        track.recover_season = Some(season);
                    }
                }
            }
            // Movimentos de EQUIPE (promoção/rebaixamento de times)
            for m in &result.promotion_result.movements {
                match m.movement_type {
                    MovementType::Promocao => {
                        t.team_promoted += 1;
                        promo_by_team
                            .entry(m.team_id.clone())
                            .or_default()
                            .push((season, tier_of(&m.from_category)));
                        team_name_of.insert(m.team_id.clone(), m.team_name.clone());
                        if tier_of(&m.from_category) == 0 {
                            *t.rookie_champ_names.entry(m.team_name.clone()).or_insert(0) += 1;
                        }
                        promoted_at.push((m.team_id.clone(), season));
                        // Onde o carro do promovido aterrissou no campo de destino
                        // (mesma categoria+classe, excluindo ele próprio; a rebaixada
                        // já saiu na troca). Mede se entra isolado em último (gap<0)
                        // ou logo acima do lanterna (Ideia 1: gap≈margem).
                        if let Some((cat, cls, car)) = car_by_id.get(&m.team_id) {
                            let others: Vec<f64> = car_by_id
                                .iter()
                                .filter(|(id, (c, cl, _))| {
                                    id.as_str() != m.team_id && c == cat && cl == cls
                                })
                                .map(|(_, (_, _, cp))| *cp)
                                .collect();
                            if !others.is_empty() {
                                let worst = others.iter().copied().fold(f64::INFINITY, f64::min);
                                t.promo_landing_gap_sum += car - worst;
                                t.promo_landing_n += 1;
                                let rank_from_bottom = others.iter().filter(|&&c| c < *car).count();
                                match rank_from_bottom {
                                    0 => t.promo_landing_rank_worst += 1,
                                    1..=2 => t.promo_landing_rank_near += 1,
                                    _ => t.promo_landing_rank_mid += 1,
                                }
                            }
                        }
                    }
                    MovementType::Rebaixamento => {
                        t.team_relegated += 1;
                        relegated_seasons
                            .entry(m.team_id.clone())
                            .or_default()
                            .push(season);
                    }
                }
            }

            // Snapshot DEPOIS
            let after = snapshot_drivers(&db_path);
            last_after = after.clone();
            let inj_after = snapshot_injuries(&db_path);

            // ── Funil de carreira: tier de cada piloto pós-promoção ──
            for (id, categoria) in snapshot_driver_categories(&db_path) {
                let tier = tier_of(&categoria);
                if tier > 6 {
                    continue; // categoria desconhecida
                }
                let overall = after.get(&id).map(|s| s.overall).unwrap_or(0.0);
                let track = careers.entry(id).or_insert_with(|| CareerTrack {
                    first_season: season,
                    started_rookie: tier == 0,
                    peak_tier: tier,
                    peak_skill: overall,
                    reached_at: [None; 7],
                });
                track.peak_tier = track.peak_tier.max(tier);
                track.peak_skill = track.peak_skill.max(overall);
                if track.reached_at[tier as usize].is_none() {
                    track.reached_at[tier as usize] = Some(season);
                }
            }

            // ── Lesões geradas nesta temporada ──
            let mut injured_pilots: HashSet<String> = HashSet::new();
            let mut leve = 0u64;
            let mut moderada = 0u64;
            let mut grave = 0u64;
            let mut critica = 0u64;
            for (id, (pilot, tipo)) in &inj_after {
                if !inj_before.contains(id) {
                    injured_pilots.insert(pilot.clone());
                    match tipo.as_str() {
                        "Leve" => leve += 1,
                        "Moderada" => moderada += 1,
                        "Grave" => grave += 1,
                        "Critica" => critica += 1,
                        _ => {}
                    }
                }
            }
            let injured_this_season = injured_pilots.len() as u64;
            t.injured_drivers += injured_this_season;
            t.inj_leve += leve;
            t.inj_moderada += moderada;
            t.inj_grave += grave;
            t.inj_critica += critica;
            t.inj_rate_samples
                .push(pct(injured_this_season, active_at_start));

            // ── Evolução: sobe / desce / estagna (sobreviventes) ──
            let mut s_sobe = 0u64;
            let mut s_desce = 0u64;
            let mut s_estagna = 0u64;
            for (id, snap_before) in &before {
                if let Some(snap_after) = after.get(id) {
                    let delta = snap_after.overall - snap_before.overall;
                    let bucket = age_bucket(snap_before.age);
                    let entry = t.by_age.entry(bucket).or_insert([0.0; 5]);
                    entry[3] += delta;
                    entry[4] += 1.0;
                    t.survivors += 1;
                    if delta > STAGNATION_THRESHOLD {
                        s_sobe += 1;
                        entry[0] += 1.0;
                    } else if delta < -STAGNATION_THRESHOLD {
                        s_desce += 1;
                        entry[1] += 1.0;
                    } else {
                        s_estagna += 1;
                        entry[2] += 1.0;
                    }
                }
            }
            t.sobe += s_sobe;
            t.desce += s_desce;
            t.estagna += s_estagna;
            let survivors_season = s_sobe + s_desce + s_estagna;
            t.sobe_rate_samples.push(pct(s_sobe, survivors_season));
            t.desce_rate_samples.push(pct(s_desce, survivors_season));
            t.estagna_rate_samples
                .push(pct(s_estagna, survivors_season));

            // ── Aposentadorias ──
            let retired_this_season = result.retirements.len() as u64;
            t.retirements += retired_this_season;
            for r in &result.retirements {
                t.retire_age_sum += r.age.max(0) as u64;
                *t.retire_reasons.entry(r.reason.clone()).or_insert(0) += 1;
                let rt = r.categoria.as_deref().map(tier_of).unwrap_or(99);
                if rt <= 6 {
                    t.retire_by_tier[rt as usize] += 1;
                }
                // Duração de carreira observada nesta simulação (temporadas vistas)
                if let Some(tr) = traj.get(&r.driver_id) {
                    t.retire_career_len_sum += tr.seasons_seen as u64;
                    t.retire_career_len_n += 1;
                    if r.reason.contains("falta de motivacao") {
                        t.motiv_retire_n += 1;
                        t.motiv_retire_overall_sum += tr.last_overall;
                        if tr.last_overall >= 60.0 {
                            t.motiv_retire_good += 1;
                        }
                    }
                }
                // Idade de quem larga POR DESMOTIVAÇÃO. O ramo de desistência em
                // `check_retirement` compra paciência só com skill — não há termo
                // de idade nenhum —, então um piloto de 22 anos desiste no mesmo
                // prazo de um de 38. Esta faixa mede o tamanho real do problema.
                if r.reason.contains("falta de motivacao") {
                    *t.motiv_retire_by_age.entry(age_bucket(r.age)).or_insert(0) += 1;
                }
            }
            t.retire_rate_samples
                .push(pct(retired_this_season, active_at_start));

            // ── A MESMA temporada, indexada pelo lugar dela na run ──
            //
            // Tudo acima foi para um saco sem índice. Aqui a mesma medida entra com o número da
            // temporada, que é o que separa "o mundo tem 2% de lesão" de "o mundo COMEÇA em 1,2%
            // e TERMINA em 2,8%". Ver `TaxasDaTemporada`.
            let faixa = t.taxas_por_temporada.entry(season).or_default();
            faixa.lesao.push(pct(injured_this_season, active_at_start));
            faixa
                .aposentadoria
                .push(pct(retired_this_season, active_at_start));
            faixa.sobe.push(pct(s_sobe, survivors_season));
            faixa.desce.push(pct(s_desce, survivors_season));
            faixa.estagna.push(pct(s_estagna, survivors_season));
            faixa.ativos.push(active_at_start as f64);

            // ── Promoções / Rebaixamentos de pilotos ──
            let mut team_dir: HashMap<&str, &MovementType> = HashMap::new();
            for m in &result.promotion_result.movements {
                team_dir.insert(m.team_id.as_str(), &m.movement_type);
            }
            for e in &result.promotion_result.pilot_effects {
                match e.effect {
                    PilotEffectType::MovesWithTeam => match team_dir.get(e.team_id.as_str()) {
                        Some(MovementType::Promocao) => t.promoted += 1,
                        Some(MovementType::Rebaixamento) => t.relegated += 1,
                        None => {}
                    },
                    PilotEffectType::FreedNoLicense => t.freed_no_license += 1,
                    PilotEffectType::FreedPlayerStays => {}
                }
            }

            // Avança para a próxima Temporada (exceto na última iteração)
            if season + 1 < seasons {
                run_preseason_to_temporada(&base_dir);
            }
        }

        // Atualiza o último overall com o estado final pós-última temporada
        for (id, snap) in &last_after {
            if let Some(tr) = traj.get_mut(id) {
                tr.last_overall = snap.overall;
            }
        }

        // Consolida trajetórias de carreira (pilotos vistos em >= 2 temporadas)
        for tr in traj.values() {
            if tr.seasons_seen < 2 {
                continue;
            }
            let delta = tr.last_overall - tr.first_overall;
            t.traj_count += 1;
            t.traj_delta_sum += delta;
            let bucket = age_bucket(tr.first_age);
            let e = t.traj_by_age.entry(bucket).or_insert([0.0; 5]);
            e[3] += delta;
            e[4] += 1.0;
            if delta > CAREER_THRESHOLD {
                t.traj_sobe += 1;
                e[0] += 1.0;
            } else if delta < -CAREER_THRESHOLD {
                t.traj_desce += 1;
                e[1] += 1.0;
            } else {
                t.traj_estagna += 1;
                e[2] += 1.0;
            }
        }

        // Consolida trajetórias financeiras de equipe desta run
        for track in team_states.values() {
            if !track.ever_collapse {
                continue;
            }
            t.teams_ever_collapse += 1;
            t.collapse_seasons_sum += track.seasons_in_collapse as u64;
            if track.escaped {
                t.teams_escaped += 1;
            }
            if track.recovered {
                t.teams_recovered += 1;
                if let (Some(start), Some(end)) =
                    (track.first_collapse_season, track.recover_season)
                {
                    t.recover_time_sum += end.saturating_sub(start) as u64;
                    t.recover_time_n += 1;
                }
            }
            // "Preso": colapsou e terminou a simulação ainda em colapso
            if track.final_state_rank == state_rank("collapse") {
                t.teams_stuck += 1;
            }
        }

        // DIAGNÓSTICO SNOWBALL: maior cadeia de promoções consecutivas (temporada+1,
        // tier+1) por equipe = "sobe a escada ganhando todo ano". Cadeia de comprimento
        // 1 = campeã uma vez. 3+ = exatamente o bug relatado (rookie→cup→production...).
        for (team_id, promos) in &promo_by_team {
            let mut seq = promos.clone();
            seq.sort_by_key(|&(s, tier)| (s, tier));
            let mut best = 1usize;
            let mut cur = 1usize;
            for w in seq.windows(2) {
                let (s0, t0) = w[0];
                let (s1, t1) = w[1];
                if s1 == s0 + 1 && t1 == t0 + 1 {
                    cur += 1;
                    best = best.max(cur);
                } else {
                    cur = 1;
                }
            }
            *t.ladder_chain_hist.entry(best).or_insert(0) += 1;
            t.max_ladder_chain = t.max_ladder_chain.max(best);
            // Identidade dos "climbers" (cadeia >= 2): mostra se é sempre a mesma equipe.
            if best >= 2 {
                if let Some(name) = team_name_of.get(team_id) {
                    *t.climber_names.entry(name.clone()).or_insert(0) += 1;
                }
            }
        }

        // Ideia 1: bounce-down do promovido — promovida em S rebaixada em S+1 (ou S+2).
        // Só conta promoções cuja janela é observável dentro do horizonte da run.
        for (team_id, s) in &promoted_at {
            let rels = relegated_seasons.get(team_id);
            if s + 1 < seasons {
                t.promo_events_obs1 += 1;
                if rels.is_some_and(|v| v.contains(&(s + 1))) {
                    t.promo_bounce_1 += 1;
                }
            }
            if s + 2 < seasons {
                t.promo_events_obs2 += 1;
                if rels.is_some_and(|v| v.iter().any(|&r| r == s + 1 || r == s + 2)) {
                    t.promo_bounce_2 += 1;
                }
            }
        }
        // Ricochete do rebaixado — rebaixada em S volta a ser promovida em ≤2 temporadas.
        for (team_id, seasons_down) in &relegated_seasons {
            let ups: Vec<usize> = promoted_at
                .iter()
                .filter(|(id, _)| id == team_id)
                .map(|(_, s)| *s)
                .collect();
            for &s in seasons_down {
                if s + 2 < seasons {
                    t.releg_events_obs2 += 1;
                    if ups.iter().any(|&u| u == s + 1 || u == s + 2) {
                        t.releg_bounce_back_2 += 1;
                    }
                }
            }
        }

        // Consolida o funil de carreira do cohort que começou no rookie.
        for track in careers.values() {
            if !track.started_rookie {
                continue;
            }
            t.rookie_cohort += 1;
            let band = skill_band_of(track.peak_skill);
            let entry = t.skill_band.entry(band).or_insert([0; 3]);
            entry[0] += 1;
            entry[1] += track.peak_tier as u64;
            if track.peak_tier >= 5 {
                entry[2] += 1;
            }
            for tier in 1..=6usize {
                if track.peak_tier as usize >= tier {
                    t.reached_tier[tier] += 1;
                }
                if let Some(reached) = track.reached_at[tier] {
                    t.time_to_tier_sum[tier] += reached.saturating_sub(track.first_season) as u64;
                    t.time_to_tier_n[tier] += 1;
                }
            }
        }

        // Desfecho dos episódios de colapso (contadores de produção)
        t.episodes_self_rescued += rescue_counter(&db_path, "self_rescued") as u64;
        t.episodes_sold += rescue_counter(&db_path, "sold") as u64;
        {
            let db = Database::open_existing(&db_path).expect("db");
            let n: i64 = db
                .conn
                // Só as VENDAS: o contador existe para conferir com `episodes_sold`.
                // A tabela também guarda o alerta de insolvência do 1º ano em colapso
                // (`collapse_warning`), que não é venda e inflaria a verificação.
                .query_row(
                    "SELECT COUNT(*) FROM team_ownership_events WHERE event_type = 'sale'",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            t.ownership_events_recorded += n as u64;
        }

        // Concentração de títulos de construtores acumulados ao fim da run
        {
            let titles = constructor_titles_by_team(&db_path);
            let total_titles: i64 = titles.iter().sum();
            if total_titles > 0 {
                let top: i64 = titles.iter().copied().max().unwrap_or(0);
                let with_any = titles.iter().filter(|&&n| n > 0).count();
                t.title_top_share_sum += top as f64 / total_titles as f64;
                t.title_teams_with_any_sum += with_any as f64;
                t.title_runs += 1;
            }
            // Dinastia por classe premium (granularidade correta do Pilar D).
            for class_titles in premium_class_title_dist(&db_path) {
                let total: i64 = class_titles.iter().sum();
                if total > 0 {
                    let top = class_titles.iter().copied().max().unwrap_or(0);
                    t.premium_top_share_sum += top as f64 / total as f64;
                    t.premium_unique_sum += class_titles.len() as f64;
                    t.premium_class_count += 1;
                }
            }
        }

        // Tenure de vínculo ao fim da run (ideia 4): duplas de era vs mercado congelado.
        {
            let (sum, count, max_t, ge3, ge4) = bond_tenure_snapshot(&db_path);
            if count > 0 {
                t.bond_tenure_sum += sum;
                t.bond_pairs += count as u64;
                t.bond_max_tenure = t.bond_max_tenure.max(max_t);
                t.bond_ge3 += ge3 as u64;
                t.bond_ge4 += ge4 as u64;
            }
        }

        // Rivalidade entre EQUIPES ao fim da run (Fase 2): magnitude e fontes.
        {
            let (count, sum, max, by_source) = team_rivalry_snapshot(&db_path);
            t.tr_runs += 1;
            t.tr_count_sum += count;
            t.tr_perceived_sum += sum;
            t.tr_perceived_max = t.tr_perceived_max.max(max);
            for (src, n) in by_source {
                *t.tr_by_source.entry(src).or_insert(0) += n;
            }
        }

        let _ = std::fs::remove_dir_all(&base_dir);
        println!(
            "  run {}/{} concluída ({:.1}s acumulado)",
            run + 1,
            runs,
            start.elapsed().as_secs_f64()
        );
    }
}
