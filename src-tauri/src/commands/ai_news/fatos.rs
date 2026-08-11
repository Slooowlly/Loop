//! Montagem do "fact bundle" pós-corrida enviado ao servidor.

use super::*;

fn weather_label(w: &str) -> String {
    let key = match w {
        "HeavyRain" => "ai_news.weather.heavy_rain",
        "Wet" => "ai_news.weather.wet",
        "Damp" => "ai_news.weather.damp",
        _ => "ai_news.weather.dry",
    };
    rust_i18n::t!(key).to_string()
}

/// Escolhe a frase de fechamento do fim de semana: o que a equipe ANUNCIOU no sábado
/// contra o que o piloto ENTREGOU no domingo.
///
/// `soma` é a favorabilidade geral anunciada (as três faixas somadas, em [-6, 6]);
/// `assessment` é o veredito do cérebro pós-corrida.
///
/// **O eixo do tom é um só: quem leva a conta.** Quando anunciado e entregue CONCORDAM, o
/// fim de semana explica o resultado; quando DIVERGEM, quem explica é o piloto. Os dois
/// casos de divergência são os informativos — previsão que falha ensina que a leitura é
/// probabilística e não promessa; previsão que acerta apenas confirma.
///
/// | anunciado | entregue | quem leva a conta | tom |
/// |---|---|---|---|
/// | a favor | acima | condições | rodapé, confirma e sai |
/// | a favor | abaixo | **piloto** | o mais duro — as condições não explicam |
/// | contra | abaixo | condições | contexto, sem absolver |
/// | contra | acima | **piloto** | crédito — tirou o que não estava lá |
///
/// O caso "a favor × abaixo" é o que impede a leitura de virar álibi automático: sem ele,
/// anunciar o fim de semana só serviria para desculpar resultado ruim.
///
/// `None` quando não há o que fechar — anúncio neutro (`soma == 0`) ou resultado dentro do
/// esperado. Nem toda corrida rende uma frase, e forçar uma seria ruído; é o mesmo
/// princípio da regra do vazio.
// `pub(crate)` e não `pub(super)`: `ai_news.rs` reexporta este módulo com
// `pub(crate) use fatos::*`, e um glob não reexporta item MENOS visível que ele — o
// `pub(super)` sumia em silêncio antes de chegar ao módulo de testes.
pub(crate) fn caso_do_anuncio(
    soma: i32,
    assessment: crate::race_eval::Assessment,
) -> Option<&'static str> {
    use crate::race_eval::Assessment;
    let acima = matches!(assessment, Assessment::MuitoAcima | Assessment::Acima);
    let abaixo = matches!(assessment, Assessment::Abaixo | Assessment::MuitoAbaixo);

    match (soma.signum(), acima, abaixo) {
        (1, true, _) => Some("ai_news.facts.forecast_good_delivered"),
        (1, _, true) => Some("ai_news.facts.forecast_good_missed"),
        (-1, _, true) => Some("ai_news.facts.forecast_bad_confirmed"),
        (-1, true, _) => Some("ai_news.facts.forecast_bad_beaten"),
        _ => None,
    }
}

/// Monta o "fact bundle" do pós-corrida a partir da tela salva (resultado +
/// manutenção + avaliação) cruzada com o banco (companheiro, rival de campeonato,
/// últimas 3 corridas, contexto da pré-corrida). Organizado em torno de uma TESE
/// dominante (EIXO → APOIO → PANO DE FUNDO); o tom/voz fica no prompt do servidor.
/// String vazia → sem fatos suficientes (front cai no template).
pub(crate) fn build_post_race_facts(
    conn: &rusqlite::Connection,
    career_dir: &std::path::Path,
    race_id: &str,
) -> String {
    use crate::commands::race::MaintenanceBreakdown;
    use crate::race_eval::RaceEvaluation;
    use crate::simulation::race::RaceResult;
    use std::fmt::Write;

    let path = career_dir
        .join("race_screens")
        .join(format!("{race_id}.json"));
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return String::new();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return String::new();
    };
    let Some(result) = v
        .get("race_result")
        .and_then(|x| serde_json::from_value::<RaceResult>(x.clone()).ok())
    else {
        return String::new();
    };
    let maintenance = v
        .get("maintenance")
        .and_then(|x| serde_json::from_value::<MaintenanceBreakdown>(x.clone()).ok())
        .unwrap_or_default();
    let evaluation = v
        .get("evaluation")
        .and_then(|x| serde_json::from_value::<RaceEvaluation>(x.clone()).ok());

    let Some(player) = result.race_results.iter().find(|r| r.is_jogador) else {
        return String::new();
    };

    let categoria = crate::db::queries::teams::get_team_by_id(conn, &player.team_id)
        .ok()
        .flatten()
        .map(|t| t.categoria)
        .unwrap_or_default();

    // ---- CENÁRIO (cabeçalho) ----
    let calendar_entry = crate::db::queries::calendar::get_calendar_entry_by_id(conn, race_id)
        .ok()
        .flatten();
    let season_num = crate::db::queries::seasons::get_active_season(conn)
        .ok()
        .flatten()
        .map(|s| s.numero)
        .unwrap_or(0);
    let mut cenario = String::new();
    let _ = write!(
        cenario,
        "{}",
        rust_i18n::t!(
            "ai_news.facts.scenario_head",
            track = result.track_name.as_str(),
            weather = weather_label(&result.weather).as_str()
        )
    );
    if !categoria.is_empty() {
        let _ = write!(
            cenario,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.scenario_category",
                category = categoria.as_str()
            )
        );
    }
    if let Some(entry) = &calendar_entry {
        let _ = write!(
            cenario,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.scenario_round",
                season = season_num,
                round = entry.rodada
            )
        );
    }
    let _ = write!(
        cenario,
        "{}",
        rust_i18n::t!("ai_news.facts.scenario_laps", laps = result.total_laps)
    );

    // ---- Bloco: META + NOTA (cérebro race_eval) ----
    let mut eval_b = String::new();
    if let Some(ev) = &evaluation {
        let _ = writeln!(
            eval_b,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.target",
                low = ev.target_low,
                high = ev.target_high
            )
        );
        let _ = write!(
            eval_b,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.grade",
                grade = format!("{:.1}", ev.grade),
                label = ev.assessment.label()
            )
        );
    }

    // ---- Bloco: SEU RESULTADO ----
    let mut res_b = String::new();
    let _ = writeln!(res_b, "{}", rust_i18n::t!("ai_news.facts.result_head"));
    let _ = writeln!(
        res_b,
        "{}",
        rust_i18n::t!("ai_news.facts.started", grid = player.grid_position)
    );
    if player.is_dnf {
        match player.dnf_reason.as_deref().filter(|s| !s.is_empty()) {
            Some(m) => {
                let _ = writeln!(
                    res_b,
                    "{}",
                    rust_i18n::t!("ai_news.facts.dnf_reason", reason = m)
                );
            }
            None => {
                let _ = writeln!(res_b, "{}", rust_i18n::t!("ai_news.facts.dnf"));
            }
        }
    } else {
        let _ = writeln!(
            res_b,
            "{}",
            rust_i18n::t!("ai_news.facts.finished", pos = player.finish_position)
        );
    }
    let saldo = player.positions_gained;
    let saldo_txt = if saldo > 0 {
        rust_i18n::t!("ai_news.facts.gained", n = saldo).to_string()
    } else if saldo < 0 {
        rust_i18n::t!("ai_news.facts.lost", n = saldo.abs()).to_string()
    } else {
        rust_i18n::t!("ai_news.facts.held").to_string()
    };
    let _ = writeln!(
        res_b,
        "{}",
        rust_i18n::t!("ai_news.facts.balance", txt = saldo_txt)
    );
    if !player.is_dnf && player.finish_position > 1 {
        let gap = player.gap_to_winner_ms / 1000.0;
        if gap > 0.0 {
            let _ = writeln!(
                res_b,
                "{}",
                rust_i18n::t!("ai_news.facts.gap_to_winner", secs = format!("{gap:.3}"))
            );
        }
    }
    if player.best_lap_time_ms > 0.0 {
        let s = player.best_lap_time_ms / 1000.0;
        let m = (s / 60.0).floor();
        let rest = s - m * 60.0;
        let fastest = if player.has_fastest_lap {
            rust_i18n::t!("ai_news.facts.fastest_flag").to_string()
        } else {
            String::new()
        };
        let _ = writeln!(
            res_b,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.best_lap",
                time = format!("{}:{:06.3}", m as i64, rest),
                fastest = fastest
            )
        );
    }
    let _ = writeln!(
        res_b,
        "{}",
        rust_i18n::t!("ai_news.facts.points", n = player.points_earned)
    );
    let _ = write!(
        res_b,
        "{}",
        rust_i18n::t!("ai_news.facts.incidents", n = player.incidents_count)
    );

    // ---- Bloco: o CURSO da corrida (trânsito, box, safety car) ----
    //
    // O motor calcula isto desde a reforma da simulação e nada disso chegava ao texto: o
    // debrief falava de P8 sem nunca falar do que produziu o P8. É a diferença entre
    // "você foi mal" e "você ficou 3 trechos atrás do Braun, tentou duas vezes e não
    // passou" — a segunda é a mesma corrida, contada de um jeito que não parece dado.
    //
    // Só entra quando há de fato o que contar. Corrida gravada antes da v55, e import do
    // iRacing (que não tem trecho nenhum), deixam o bloco vazio e ele nem aparece.
    let mut curso_b = String::new();
    {
        let tentou = player.tentativas_ultrapassagem;
        let passou = player.ultrapassagens_concluidas;
        let preso = player.maior_sequencia_preso;
        let ar_sujo = player.segmentos_em_ar_sujo;
        let tem_transito = tentou > 0 || preso > 0 || ar_sujo > 0;
        let tem_parada = !player.volta_da_parada.is_empty();
        let tem_sc = !result.safety_cars.is_empty();

        if tem_transito || tem_parada || tem_sc {
            let _ = writeln!(curso_b, "{}", rust_i18n::t!("ai_news.facts.course_head"));
        }

        if tem_transito {
            // Tentativa que não vira ultrapassagem é o mecanismo mais novo do motor e o
            // mais invisível: antes da reforma a taxa de conversão era 100% implícita.
            let _ = writeln!(
                curso_b,
                "{}",
                rust_i18n::t!(
                    "ai_news.facts.course_traffic",
                    tried = tentou,
                    done = passou,
                    suffered = player.tentativas_sofridas
                )
            );
            if preso > 0 {
                let _ = writeln!(
                    curso_b,
                    "{}",
                    rust_i18n::t!("ai_news.facts.course_stuck", n = preso, dirty = ar_sujo)
                );
            }
        }

        // Custo do box: "largou P4, parou na volta 12, voltou P9, terminou P6" é uma
        // história completa em quatro números — e é o que faz o jogador perceber que
        // estratégia existe. Os três vetores da parada são paralelos por contrato.
        for (idx, volta) in player.volta_da_parada.iter().enumerate() {
            let antes = player.posicao_antes_da_parada.get(idx).copied();
            let depois = player.posicao_depois.get(idx).copied();
            match (antes, depois) {
                (Some(a), Some(d)) => {
                    let _ = writeln!(
                        curso_b,
                        "{}",
                        rust_i18n::t!(
                            "ai_news.facts.course_pit",
                            lap = volta,
                            before = a,
                            after = d,
                            delta = d - a
                        )
                    );
                }
                // Parada gravada sem o par de posições ainda é fato ("parou na volta 12").
                _ => {
                    let _ = writeln!(
                        curso_b,
                        "{}",
                        rust_i18n::t!("ai_news.facts.course_pit_bare", lap = volta)
                    );
                }
            }
        }
        if !player.estrategia_id.is_empty() {
            let _ = writeln!(
                curso_b,
                "{}",
                rust_i18n::t!(
                    "ai_news.facts.course_strategy",
                    strategy = player.estrategia_id.as_str()
                )
            );
        }

        // Safety car: o do jogo troca o vencedor com frequência e o jogador hoje não tem
        // como saber que foi ele. A posição do piloto ANTES da amarela é o número que
        // transforma "perdi posições" em "a amarela me custou posições".
        for (idx, volta) in result.safety_cars.iter().enumerate() {
            let antes = result
                .ordem_pre_safety_car
                .get(idx)
                .and_then(|ordem| ordem.iter().position(|id| id == &player.pilot_id))
                .map(|i| i as i32 + 1);
            match antes {
                Some(pos) if !player.is_dnf => {
                    let _ = writeln!(
                        curso_b,
                        "{}",
                        rust_i18n::t!(
                            "ai_news.facts.course_safety_car",
                            lap = volta,
                            before = pos,
                            after = player.finish_position
                        )
                    );
                }
                _ => {
                    let _ = writeln!(
                        curso_b,
                        "{}",
                        rust_i18n::t!("ai_news.facts.course_safety_car_bare", lap = volta)
                    );
                }
            }
        }
    }

    // ---- Bloco: companheiro de equipe ----
    let mut mate_b = String::new();
    if let Some(mate) = result
        .race_results
        .iter()
        .find(|r| r.team_id == player.team_id && !r.is_jogador)
    {
        let mate_pos = if mate.is_dnf {
            rust_i18n::t!("ai_news.facts.dnf_short").to_string()
        } else {
            format!("P{}", mate.finish_position)
        };
        let cmp = if player.is_dnf || mate.is_dnf {
            String::new()
        } else if player.finish_position < mate.finish_position {
            rust_i18n::t!("ai_news.facts.teammate_ahead").to_string()
        } else {
            rust_i18n::t!("ai_news.facts.teammate_behind").to_string()
        };
        let _ = write!(
            mate_b,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.teammate",
                name = mate.pilot_name.as_str(),
                pos = mate_pos,
                cmp = cmp
            )
        );
    }

    // ---- Bloco: rival de campeonato (o MESMO que a pré-corrida marcou) ----
    let mut champ_b = String::new();
    if !categoria.is_empty() {
        if let Ok(Some(rival)) =
            crate::commands::career::build_primary_rival_summary(conn, &player.pilot_id, &categoria)
        {
            // Empate em pontos vira frase própria: "atrás de você por 0pts" mandaria a
            // IA cravar uma direção que a tabela não sustenta.
            let standing = if rival.gap_points == 0 {
                rust_i18n::t!("ai_news.facts.champ_tied").to_string()
            } else if rival.is_ahead {
                rust_i18n::t!("ai_news.facts.champ_ahead", pts = rival.gap_points).to_string()
            } else {
                rust_i18n::t!("ai_news.facts.champ_behind", pts = rival.gap_points).to_string()
            };
            match result
                .race_results
                .iter()
                .find(|r| r.pilot_id == rival.driver_id)
            {
                Some(rr) => {
                    let rpos = if rr.is_dnf {
                        rust_i18n::t!("ai_news.facts.dnf_short").to_string()
                    } else {
                        format!("P{}", rr.finish_position)
                    };
                    let cmp = if !player.is_dnf && !rr.is_dnf {
                        if player.finish_position < rr.finish_position {
                            rust_i18n::t!("ai_news.facts.champ_you_ahead").to_string()
                        } else {
                            rust_i18n::t!("ai_news.facts.champ_you_behind").to_string()
                        }
                    } else {
                        String::new()
                    };
                    let _ = write!(
                        champ_b,
                        "{}",
                        rust_i18n::t!(
                            "ai_news.facts.champ_rival",
                            name = rival.driver_name.as_str(),
                            standing = standing,
                            pos = rpos,
                            cmp = cmp
                        )
                    );
                }
                None => {
                    let _ = write!(
                        champ_b,
                        "{}",
                        rust_i18n::t!(
                            "ai_news.facts.champ_rival_absent",
                            name = rival.driver_name.as_str(),
                            standing = standing
                        )
                    );
                }
            }
        }
    }

    // ---- Bloco: rivalidade VIVIDA (nemesis + rivais) + captura do DUELO decidido ----
    let mut lived_b = String::new();
    let mut duel: Option<PostRaceDuel> = None;
    {
        use std::cmp::Ordering;
        let current = crate::db::queries::player_nemesis::get_current_nemesis(conn).unwrap_or(None);
        let interests = crate::commands::career::select_player_interests(conn, current.as_deref());
        let mut rows: Vec<(&str, crate::commands::career::RivalInterest)> = Vec::new();
        if let Some(n) = interests.nemesis {
            rows.push(("NEMESIS", n));
        }
        for r in interests.rivais {
            rows.push(("RIVAL", r));
        }
        for (role, ri) in rows {
            // O 1º duelo DECIDIDO (nemesis vem primeiro, então tem prioridade) vira sinal
            // para a tese `DuelDecided` quando o resultado em si foi morno.
            if duel.is_none() && !player.is_dnf {
                if let Some(rr) = result
                    .race_results
                    .iter()
                    .find(|d| d.pilot_id == ri.driver_id)
                {
                    if !rr.is_dnf && rr.finish_position != player.finish_position {
                        duel = Some(PostRaceDuel {
                            name: ri.driver_name.clone(),
                            player_won: player.finish_position < rr.finish_position,
                            is_nemesis: role == "NEMESIS",
                            h2h: if ri.chapters > 0 {
                                Some((ri.h2h_player_wins, ri.h2h_rival_wins))
                            } else {
                                None
                            },
                        });
                    }
                }
            }
            let today = match result
                .race_results
                .iter()
                .find(|d| d.pilot_id == ri.driver_id)
            {
                Some(rr) => {
                    let pos = if rr.is_dnf {
                        rust_i18n::t!("ai_news.facts.dnf_short").to_string()
                    } else {
                        format!("P{}", rr.finish_position)
                    };
                    let cmp = if !player.is_dnf && !rr.is_dnf {
                        match player.finish_position.cmp(&rr.finish_position) {
                            Ordering::Less => {
                                rust_i18n::t!("ai_news.facts.lived_you_ahead").to_string()
                            }
                            Ordering::Greater => {
                                rust_i18n::t!("ai_news.facts.lived_you_behind").to_string()
                            }
                            Ordering::Equal => String::new(),
                        }
                    } else {
                        String::new()
                    };
                    rust_i18n::t!("ai_news.facts.lived_finished", pos = pos, cmp = cmp).to_string()
                }
                None => rust_i18n::t!("ai_news.facts.lived_absent").to_string(),
            };
            // `role` é chave de LÓGICA (comparada acima); o rótulo exibido é resolvido à parte.
            let role_label = if role == "NEMESIS" {
                rust_i18n::t!("ai_news.facts.role_nemesis")
            } else {
                rust_i18n::t!("ai_news.facts.role_rival")
            };
            let label = ri.label.map(|l| format!(" \"{l}\"")).unwrap_or_default();
            let h2h = if ri.chapters > 0 {
                rust_i18n::t!(
                    "ai_news.facts.lived_h2h",
                    p = ri.h2h_player_wins,
                    r = ri.h2h_rival_wins
                )
                .to_string()
            } else {
                String::new()
            };
            let _ = writeln!(
                lived_b,
                "{}",
                rust_i18n::t!(
                    "ai_news.facts.lived_line",
                    role = role_label,
                    label = label,
                    name = ri.driver_name.as_str(),
                    today = today,
                    h2h = h2h
                )
            );
        }
    }

    // Registro do piloto (fama + atributos de pressão): uma leitura só, reusada abaixo.
    let player_driver = crate::db::queries::drivers::get_driver(conn, &player.pilot_id).ok();

    // ---- Bloco: LESÃO (sofrida nesta corrida ou carregada) ----
    // Fecha o loop físico: se o jogador se machucou HOJE (a lesão ativa aponta para esta
    // corrida em `race_occurred`) o debrief referencia isso; senão, nota que já corria
    // machucado. A geração da lesão é de outro sistema — aqui só LEMOS o que existe.
    let mut inj_b = String::new();
    if let Ok(Some(inj)) =
        crate::db::queries::injuries::get_active_injury_for_pilot(conn, &player.pilot_id)
    {
        use crate::models::enums::InjuryType;
        let severity = match inj.injury_type {
            InjuryType::Grave | InjuryType::Critica => rust_i18n::t!("ai_news.facts.injury_severe"),
            InjuryType::Moderada => rust_i18n::t!("ai_news.facts.injury_moderate"),
            InjuryType::Leve => rust_i18n::t!("ai_news.facts.injury_light"),
        };
        let name = if inj.injury_name.trim().is_empty() {
            inj.injury_type.as_str().to_string()
        } else {
            inj.injury_name.clone()
        };
        let _ = if inj.race_occurred == race_id {
            write!(
                inj_b,
                "{}",
                rust_i18n::t!(
                    "ai_news.facts.injury_new",
                    name = name,
                    severity = severity,
                    races = inj.races_remaining
                )
            )
        } else {
            write!(
                inj_b,
                "{}",
                rust_i18n::t!(
                    "ai_news.facts.injury_ongoing",
                    name = name,
                    severity = severity,
                    races = inj.races_remaining
                )
            )
        };
    }

    // ---- Bloco: ESTRELATO (fama) — só quando é estrela de verdade (>70) ----
    let mut fame_b = String::new();
    if let Some(pd) = &player_driver {
        let midia = pd.atributos.midia;
        if midia > 70.0 {
            let level = if midia > 87.0 {
                rust_i18n::t!("ai_news.facts.fame_idol")
            } else {
                rust_i18n::t!("ai_news.facts.fame_star")
            };
            let _ = write!(
                fame_b,
                "{}",
                rust_i18n::t!(
                    "ai_news.facts.fame",
                    level = level,
                    value = midia.round() as i64
                )
            );
        }
    }

    // ---- Bloco: PRESSÃO DE TÍTULO (clutch/choke) — espelha `pressure.rs` ----
    // O sim não persiste o efeito de pressão; aqui recomputamos o MESMO estado com as
    // MESMAS funções (título + intensidade + resiliência) que a corrida aplicou, e o
    // viramos fato narrativo (segurou/afundou sob pressão). Só existe sob pressão real
    // de campeonato (intensidade > 0) — fora disso não vira ruído.
    let mut prs_b = String::new();
    if let (Some(pd), Some(cat), Some(entry)) = (
        player_driver.as_ref(),
        crate::constants::categories::get_category(&categoria),
        calendar_entry.as_ref(),
    ) {
        let races_left = (cat.corridas_por_temporada as i32 - entry.rodada + 1).max(1) as u32;
        let cat_drivers =
            crate::db::queries::drivers::get_drivers_by_active_category(conn, &categoria)
                .unwrap_or_default();
        let all_points: Vec<f64> = cat_drivers
            .iter()
            .map(|d| d.stats_temporada.pontos)
            .collect();
        let max_race_points =
            (crate::constants::scoring::get_points_for_position(1, categoria == "endurance")
                + crate::constants::scoring::BONUS_FASTEST_LAP) as f64;
        let ctx = crate::simulation::pressure::title_context(
            pd.stats_temporada.pontos,
            &all_points,
            races_left,
            max_race_points,
        );
        let intensity = crate::simulation::pressure::pressure_intensity(&ctx, races_left);
        // Precisa de uma tabela REAL (≥2 pilotos) — categorias especiais/vazias não têm
        // matemática de título e não devem disparar pressão fantasma.
        if all_points.len() >= 2 && intensity > 0.0 {
            let is_chaser = ctx.in_contention && !ctx.is_leader;
            let resilience = crate::simulation::pressure::pressure_resilience(
                pd.atributos.mentalidade,
                pd.atributos.experiencia,
            );
            let eff =
                crate::simulation::pressure::pressure_effect(intensity, resilience, is_chaser);
            let band = if races_left <= 1 {
                rust_i18n::t!("ai_news.facts.pressure_max")
            } else if intensity >= 2.0 {
                rust_i18n::t!("ai_news.facts.pressure_high")
            } else {
                rust_i18n::t!("ai_news.facts.pressure_mid")
            };
            // Direção pelo SINAL do pace_delta (deadzone pequena p/ o caso ~neutro).
            let dir = if eff.pace_delta > 0.4 {
                rust_i18n::t!("ai_news.facts.pressure_clutch")
            } else if eff.pace_delta < -0.4 {
                rust_i18n::t!("ai_news.facts.pressure_choke")
            } else {
                rust_i18n::t!("ai_news.facts.pressure_neutral")
            };
            let _ = write!(
                prs_b,
                "{}",
                rust_i18n::t!("ai_news.facts.pressure_line", band = band, dir = dir)
            );
        }
    }

    // ---- Bloco: telemetria real (só se terminou) ----
    let tel_b = if !player.is_dnf {
        telemetry_facts(v.get("telemetry"), player.grid_position)
    } else {
        String::new()
    };

    // ---- Bloco: memória entre etapas (arco) — sempre PANO DE FUNDO ----
    let arc_b = build_recent_arc_facts(conn, race_id);

    // ---- Bloco: manutenção / batida ----
    let mut mnt_b = String::new();
    if maintenance.total > 0.0 {
        let _ = writeln!(
            mnt_b,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.maintenance_total",
                total = maintenance.total.round() as i64
            )
        );
        let danos: Vec<String> = maintenance
            .items
            .iter()
            .filter(|i| !matches!(i.key.as_str(), "gasolina" | "pneus"))
            .map(|i| format!("{} $ {}", i.label, i.cost.round() as i64))
            .collect();
        if !danos.is_empty() {
            let _ = write!(
                mnt_b,
                "{}",
                rust_i18n::t!("ai_news.facts.maintenance_crash", items = danos.join(", "))
            );
        }
    }

    // ---- Bloco: quebras de peça + captura do DNF MECÂNICO ----
    let breakdowns = crate::db::queries::race_breakdowns::get_breakdowns_for_race(conn, race_id)
        .unwrap_or_default();
    let mut brk_b = String::new();
    let mut player_mech_break = false;
    if !breakdowns.is_empty() {
        let mine: Vec<_> = breakdowns
            .iter()
            .filter(|b| b.driver_id == player.pilot_id)
            .collect();
        if !mine.is_empty() {
            player_mech_break = mine
                .iter()
                .any(|b| matches!(b.severity.as_str(), "dnf" | "heavy"));
            let _ = writeln!(brk_b, "{}", rust_i18n::t!("ai_news.facts.parts_head"));
            for b in &mine {
                let desfecho = match b.penalty_secs {
                    Some(s) => rust_i18n::t!("ai_news.facts.part_pit", secs = s).to_string(),
                    None => rust_i18n::t!("ai_news.facts.part_dnf").to_string(),
                };
                let grav = match b.severity.as_str() {
                    "dnf" | "heavy" => rust_i18n::t!("ai_news.facts.part_severe"),
                    _ => rust_i18n::t!("ai_news.facts.part_light"),
                };
                let _ = writeln!(
                    brk_b,
                    "{}",
                    rust_i18n::t!(
                        "ai_news.facts.part_line",
                        lap = b.lap,
                        label = b.label.as_str(),
                        outcome = desfecho,
                        severity = grav
                    )
                );
            }
        }
        let grid_dnf = breakdowns
            .iter()
            .filter(|b| b.severity == "dnf" && b.driver_id != player.pilot_id)
            .count();
        let grid_pen = breakdowns
            .iter()
            .filter(|b| b.severity != "dnf" && b.driver_id != player.pilot_id)
            .count();
        if grid_dnf + grid_pen > 0 {
            let _ = write!(
                brk_b,
                "{}",
                rust_i18n::t!("ai_news.facts.grid_breaks", dnf = grid_dnf, pen = grid_pen)
            );
        }
    }

    // ---- Bloco: o ANÚNCIO do fim de semana × o que foi entregue ----
    //
    // O fechamento do loop da fase 3. A regra é que o pós REFERENCIE a previsão, nunca
    // repita o estado: "o acerto estava ruim" lido depois é desculpa; "a equipe te avisou
    // no sábado, e não veio" é mecanismo se cumprindo. O que licencia o pós é o pré.
    //
    // O eixo do tom é UM só: **quem leva a conta — as condições ou o piloto.** Quando o
    // anunciado e o entregue CONCORDAM, o fim de semana explica o resultado; quando
    // DIVERGEM, quem explica é o piloto. Os dois casos de divergência são os informativos,
    // e é por isso que eles ganham o tom forte: previsão que falha ensina que a leitura é
    // probabilística, previsão que acerta só confirma.
    //
    // O caso 2 (anunciado a favor × entregue abaixo) é o que impede a leitura de virar
    // álibi automático — sem ele, anunciar o fim de semana só serviria para desculpar.
    //
    // Lê a faixa ANUNCIADA do banco (v56), nunca recomputa: recomputar faria uma
    // recalibração do σ mudar retroativamente o que foi dito no sábado.
    let mut anuncio_b = String::new();
    if let Some(ev) = evaluation.as_ref() {
        // Da tabela própria da v57 — a MESMA linha que a Sala de Estratégia leu no sábado,
        // não uma recomputação. É isso que garante que o pós não contradiga o pré.
        let anunciado: Option<i32> =
            crate::db::queries::races::get_race_weekend_reading(conn, race_id, &player.pilot_id)
                .ok()
                .flatten()
                .and_then(|json| {
                    serde_json::from_str::<crate::commands::career_types::WeekendReading>(&json)
                        .ok()
                })
                .filter(|r| r.available)
                // A soma das três camadas é a favorabilidade GERAL do fim de semana — é disso
                // que a frase de fechamento fala. A decomposição em três continua sendo da
                // tela; aqui a pergunta é "o fim de semana estava a favor ou contra?".
                .map(|r| (r.track_affinity.band + r.driver_form.band + r.car_setup.band) as i32);

        if let Some(soma) = anunciado {
            if let Some(chave) = caso_do_anuncio(soma, ev.assessment) {
                let _ = writeln!(
                    anuncio_b,
                    "{}",
                    rust_i18n::t!("ai_news.facts.forecast_head")
                );
                let _ = write!(anuncio_b, "{}", rust_i18n::t!(chave));
            }
        }
    }

    // ---- Bloco: pré-corrida (FECHA o loop do que foi prometido) ----
    let mut pre_b = String::new();
    if let Ok(Some(pre)) = crate::db::queries::ai_pre_race::get_pre_race(conn, race_id) {
        let _ = writeln!(pre_b, "{}", rust_i18n::t!("ai_news.facts.pre_head"));
        if !pre.headline.is_empty() {
            let _ = writeln!(
                pre_b,
                "{}",
                rust_i18n::t!(
                    "ai_news.facts.pre_headline",
                    headline = pre.headline.as_str()
                )
            );
        }
        if !pre.narrative.is_empty() {
            let _ = write!(
                pre_b,
                "{}",
                rust_i18n::t!(
                    "ai_news.facts.pre_briefing",
                    narrative = pre.narrative.as_str()
                )
            );
        }
    }

    // "O carro te traiu" vs "você/alguém rodou". A classificação é a MESMA que o
    // boletim usa (`race_signals::dnf_kind`); aqui os insumos disponíveis são a peça
    // grave registrada no banco e o motivo textual — o incidente cru não sobrevive ao
    // save.
    let dnf_mechanical = player.is_dnf
        && crate::race_signals::dnf_kind(None, player_mech_break, player.dnf_reason.as_deref())
            .is_mecanico();

    // ---- TESE DOMINANTE ----
    let signals = PostRaceSignals {
        is_dnf: player.is_dnf,
        dnf_mechanical,
        grid: player.grid_position,
        finish: player.finish_position,
        positions_gained: player.positions_gained,
        has_fastest_lap: player.has_fastest_lap,
        assessment: evaluation.as_ref().map(|e| e.assessment),
        target_low: evaluation.as_ref().map(|e| e.target_low).unwrap_or(0),
        target_high: evaluation.as_ref().map(|e| e.target_high).unwrap_or(0),
        duel,
        track_name: result.track_name.clone(),
    };
    let (statement, mut support) = select_post_race_thesis(&signals);
    // Núcleo sempre promovido: o resultado (o que aconteceu) e o pré-corrida (fecha o loop).
    // Lesão e pressão de título entram no APOIO quando existem — são beats raros e de peso
    // (físico e mental) que a narrativa não pode tratar como rodapé.
    // "curso" entra no núcleo promovido junto com o resultado: sem ele o APOIO descreve o
    // que aconteceu e cala sobre o porquê, que é exatamente o buraco que este pacote fecha.
    for id in [
        "resultado",
        "curso",
        "anuncio",
        "pre_race",
        "injury",
        "pressure",
    ] {
        if !support.contains(&id) {
            support.push(id);
        }
    }

    // ---- Montagem em camadas (EIXO → APOIO → PANO DE FUNDO) ----
    let block_for = |id: &str| -> &str {
        match id {
            "eval" => eval_b.as_str(),
            "resultado" => res_b.as_str(),
            "curso" => curso_b.as_str(),
            "anuncio" => anuncio_b.as_str(),
            "injury" => inj_b.as_str(),
            "pressure" => prs_b.as_str(),
            "telemetry" => tel_b.as_str(),
            "teammate" => mate_b.as_str(),
            "champ_rival" => champ_b.as_str(),
            "lived_rivalry" => lived_b.as_str(),
            "breakdowns" => brk_b.as_str(),
            "maintenance" => mnt_b.as_str(),
            "fame" => fame_b.as_str(),
            "pre_race" => pre_b.as_str(),
            "arc" => arc_b.as_str(),
            _ => "",
        }
    };
    let order = [
        "eval",
        "resultado",
        // Vem logo depois do resultado porque é a CAUSA dele: o texto precisa ler "P6" e
        // já ter na mão o box e o trânsito que produziram o P6.
        "curso",
        // Logo depois do curso: o curso diz o que aconteceu na corrida, o anúncio diz se
        // o fim de semana tinha avisado. Juntos formam a atribuição completa.
        "anuncio",
        "injury",
        "pressure",
        "telemetry",
        "teammate",
        "champ_rival",
        "lived_rivalry",
        "breakdowns",
        "maintenance",
        "fame",
        "pre_race",
        "arc",
    ];
    let mut apoio = String::new();
    let mut fundo = String::new();
    for id in order {
        let text = block_for(id).trim();
        if text.is_empty() {
            continue;
        }
        let target = if support.contains(&id) {
            &mut apoio
        } else {
            &mut fundo
        };
        let _ = writeln!(target, "\n{text}");
    }

    let mut f = String::new();
    let _ = writeln!(
        f,
        "{}",
        rust_i18n::t!("ai_news.facts.scenario_line", scenario = cenario.trim())
    );
    let _ = writeln!(f, "{}", rust_i18n::t!("ai_news.facts.axis_head"));
    let _ = writeln!(f, "{statement}");
    if !apoio.trim().is_empty() {
        let _ = writeln!(f, "{}", rust_i18n::t!("ai_news.facts.support_head"));
        let _ = write!(f, "{}", apoio.trim_start_matches('\n'));
    }
    if !fundo.trim().is_empty() {
        let _ = writeln!(f, "{}", rust_i18n::t!("ai_news.facts.background_head"));
        let _ = write!(f, "{}", fundo.trim_start_matches('\n'));
    }
    f
}
