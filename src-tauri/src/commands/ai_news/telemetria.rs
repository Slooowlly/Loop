//! Leitura do race trace: ritmo, degradação, ultrapassagens e fatos de telemetria.

/// Posição do jogador no PRIMEIRO ponto captado do race trace (≈ largada).
fn player_first_position(tel: &serde_json::Value) -> Option<i64> {
    tel.get("charts")?
        .get("cars")?
        .as_array()?
        .iter()
        .find(|c| c.get("is_player").and_then(|x| x.as_bool()) == Some(true))?
        .get("points")?
        .as_array()?
        .first()?
        .get("position")?
        .as_i64()
}

/// Inclinação dos tempos de volta do jogador em ms/volta (mínimos quadrados).
/// >0 = ritmo caindo (degradação); <0 = melhorando. `None` se poucas voltas.
fn tire_deg_ms_per_lap(tel: &serde_json::Value) -> Option<f64> {
    let laps = tel.get("charts")?.get("lap_times")?.as_array()?;
    let pts: Vec<(f64, f64)> = laps
        .iter()
        .filter_map(|p| {
            Some((
                p.get("lap")?.as_f64()?,
                p.get("time_s")?.as_f64()? * 1000.0,
            ))
        })
        .collect();
    if pts.len() < 4 {
        return None;
    }
    let n = pts.len() as f64;
    let sx: f64 = pts.iter().map(|(x, _)| x).sum();
    let sy: f64 = pts.iter().map(|(_, y)| y).sum();
    let sxx: f64 = pts.iter().map(|(x, _)| x * x).sum();
    let sxy: f64 = pts.iter().map(|(x, y)| x * y).sum();
    let denom = n * sxx - sx * sx;
    if denom.abs() < 1e-9 {
        return None;
    }
    Some((n * sxy - sx * sy) / denom)
}

/// Uma troca de posição do jogador no race trace.
struct Overtake {
    lap: String,
    rival: String,
    pos: i64,
    /// `true` = o jogador ganhou a posição; `false` = perdeu.
    gained: bool,
}

impl Overtake {
    fn frase(&self) -> String {
        let key = if self.gained {
            "ai_news.overtake.passed"
        } else {
            "ai_news.overtake.lost"
        };
        rust_i18n::t!(
            key,
            lap = self.lap.as_str(),
            rival = self.rival.as_str(),
            pos = self.pos.to_string()
        )
        .to_string()
    }
}

/// Posição do último ponto captado de cada carro (nome → posição final no trace).
/// É a régua do desfecho: quem passou quem no meio da corrida não decide nada.
fn final_position_by_name(tel: &serde_json::Value) -> std::collections::HashMap<String, i64> {
    let mut out = std::collections::HashMap::new();
    let Some(cars) = tel.get("charts").and_then(|c| c.get("cars")).and_then(|c| c.as_array())
    else {
        return out;
    };
    for car in cars {
        let (Some(name), Some(pos)) = (
            car.get("name").and_then(|x| x.as_str()),
            car.get("points")
                .and_then(|p| p.as_array())
                .and_then(|p| p.last())
                .and_then(|p| p.get("position"))
                .and_then(|x| x.as_i64()),
        ) else {
            continue;
        };
        out.insert(name.to_string(), pos);
    }
    out
}

/// Posição do jogador no ÚLTIMO ponto captado do race trace (≈ bandeirada).
fn player_final_position(tel: &serde_json::Value) -> Option<i64> {
    tel.get("charts")?
        .get("cars")?
        .as_array()?
        .iter()
        .find(|c| c.get("is_player").and_then(|x| x.as_bool()) == Some(true))?
        .get("points")?
        .as_array()?
        .last()?
        .get("position")?
        .as_i64()
}

/// Reconstrói as ultrapassagens do jogador a partir do race trace: cada vez que a
/// posição dele muda entre dois pontos, o rival envolvido é o carro que assumiu a
/// posição ANTIGA do jogador naquele mesmo instante.
fn overtake_feed(tel: &serde_json::Value) -> Vec<Overtake> {
    let mut out = Vec::new();
    let Some(cars) = tel.get("charts").and_then(|c| c.get("cars")).and_then(|c| c.as_array())
    else {
        return out;
    };
    // Nome por idx + mapa (lap arredondado → posição → nome) de todos os carros.
    let mut name_by_idx: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    let mut pos_at_lap: std::collections::HashMap<String, std::collections::HashMap<i64, String>> =
        std::collections::HashMap::new();
    for car in cars {
        let idx = car.get("idx").and_then(|x| x.as_i64()).unwrap_or(-1);
        let default_name = rust_i18n::t!("ai_news.overtake.default_name").to_string();
        let name = car
            .get("name")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .unwrap_or(default_name);
        name_by_idx.insert(idx, name.clone());
        if let Some(points) = car.get("points").and_then(|p| p.as_array()) {
            for p in points {
                let (Some(lap), Some(pos)) = (
                    p.get("lap").and_then(|x| x.as_f64()),
                    p.get("position").and_then(|x| x.as_i64()),
                ) else {
                    continue;
                };
                pos_at_lap
                    .entry(format!("{lap:.4}"))
                    .or_default()
                    .insert(pos, name.clone());
            }
        }
    }
    // Percorre os pontos do jogador procurando trocas de posição.
    let Some(player) = cars
        .iter()
        .find(|c| c.get("is_player").and_then(|x| x.as_bool()) == Some(true))
    else {
        return out;
    };
    let Some(points) = player.get("points").and_then(|p| p.as_array()) else {
        return out;
    };
    for w in points.windows(2) {
        let (Some(lap0), Some(pos0)) = (
            w[0].get("lap").and_then(|x| x.as_f64()),
            w[0].get("position").and_then(|x| x.as_i64()),
        ) else {
            continue;
        };
        let (Some(lap1), Some(pos1)) = (
            w[1].get("lap").and_then(|x| x.as_f64()),
            w[1].get("position").and_then(|x| x.as_i64()),
        ) else {
            continue;
        };
        if pos0 < 1 || pos1 < 1 || pos0 == pos1 {
            continue;
        }
        let _ = lap0;
        // Quem assumiu a posição antiga do jogador no instante da troca = o rival.
        let rival = pos_at_lap
            .get(&format!("{lap1:.4}"))
            .and_then(|m| m.get(&pos0))
            .cloned()
            .unwrap_or_else(|| rust_i18n::t!("ai_news.overtake.default_rival").to_string());
        out.push(Overtake {
            lap: format!("{lap1:.1}"),
            rival,
            pos: pos1,
            gained: pos1 < pos0,
        });
    }
    out
}

/// Transforma o bloco `telemetry` do race_screens em fatos pro debrief. PURO
/// (testável): recebe o Value da telemetria e a posição de largada. Vazio se não
/// houver telemetria de verdade.
pub(crate) fn telemetry_facts(tel: Option<&serde_json::Value>, grid_position: i32) -> String {
    use std::fmt::Write;
    let Some(tel) = tel else {
        return String::new();
    };
    if tel.get("has_telemetry").and_then(|x| x.as_bool()) != Some(true) {
        return String::new();
    }

    let mut f = String::new();
    let _ = writeln!(f, "{}", rust_i18n::t!("ai_news.telemetry.header"));

    if let Some(pace) = tel.get("pace") {
        let vs_grid = pace.get("vs_grid_ms").and_then(|x| x.as_f64());
        let reliable = pace
            .get("vs_grid_reliable")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        if let Some(v) = vs_grid {
            if reliable && v.abs() >= 30.0 {
                let secs = format!("{:.2}", v.abs() / 1000.0);
                let dir = if v < 0.0 {
                    rust_i18n::t!("ai_news.telemetry.faster")
                } else {
                    rust_i18n::t!("ai_news.telemetry.slower")
                };
                let _ = writeln!(
                    f,
                    "{}",
                    rust_i18n::t!("ai_news.telemetry.pace", secs = secs, dir = dir)
                );
            }
        }
        let good = pace.get("good_laps").and_then(|x| x.as_i64()).unwrap_or(0);
        if good > 0 {
            let _ = writeln!(f, "{}", rust_i18n::t!("ai_news.telemetry.good_laps", n = good));
        }
    }

    if let Some(deg) = tire_deg_ms_per_lap(tel) {
        if deg >= 40.0 {
            let secs = format!("{:.2}", deg / 1000.0);
            let _ = writeln!(f, "{}", rust_i18n::t!("ai_news.telemetry.deg_up", secs = secs));
        } else if deg <= -40.0 {
            let secs = format!("{:.2}", deg.abs() / 1000.0);
            let _ = writeln!(f, "{}", rust_i18n::t!("ai_news.telemetry.deg_down", secs = secs));
        }
    }

    if let Some(pf) = tel.get("position_flow") {
        let gained = pf.get("gained_on_track").and_then(|x| x.as_i64()).unwrap_or(0);
        let lost = pf.get("lost_on_track").and_then(|x| x.as_i64()).unwrap_or(0);
        if gained > 0 || lost > 0 {
            let _ = writeln!(
                f,
                "{}",
                rust_i18n::t!("ai_news.telemetry.on_track", gained = gained, lost = lost)
            );
        }
    }

    if let Some(fuel) = tel.get("fuel") {
        let per_lap = fuel.get("used_per_lap_l").and_then(|x| x.as_f64());
        let laps_left = fuel.get("laps_left").and_then(|x| x.as_f64());
        if let (Some(pl), Some(ll)) = (per_lap, laps_left) {
            if pl > 0.0 {
                let per_lap = format!("{pl:.2}");
                let laps_left = format!("{ll:.1}");
                let _ = writeln!(
                    f,
                    "{}",
                    rust_i18n::t!(
                        "ai_news.telemetry.fuel",
                        per_lap = per_lap,
                        laps_left = laps_left
                    )
                );
            }
        }
    }

    if let Some(sec) = tel.get("sectors") {
        if let Some(best) = sec.get("best_ms").and_then(|x| x.as_array()) {
            if best.len() == 3 {
                let s1 = best[0].as_f64().unwrap_or(0.0) / 1000.0;
                let s2 = best[1].as_f64().unwrap_or(0.0) / 1000.0;
                let s3 = best[2].as_f64().unwrap_or(0.0) / 1000.0;
                let _ = writeln!(
                    f,
                    "{}",
                    rust_i18n::t!(
                        "ai_news.telemetry.sectors",
                        s1 = format!("{s1:.1}"),
                        s2 = format!("{s2:.1}"),
                        s3 = format!("{s3:.1}")
                    )
                );
            }
        }
        let weak = sec.get("weakest_sector").and_then(|x| x.as_i64()).unwrap_or(0);
        let loss = sec.get("weakest_loss_ms").and_then(|x| x.as_f64()).unwrap_or(0.0) / 1000.0;
        if weak >= 1 && loss >= 0.1 {
            let _ = writeln!(
                f,
                "{}",
                rust_i18n::t!(
                    "ai_news.telemetry.weak_sector",
                    sector = weak,
                    loss = format!("{loss:.2}")
                )
            );
        }
    }

    let passes = overtake_feed(tel);
    if !passes.is_empty() {
        let _ = writeln!(
            f,
            "{}",
            rust_i18n::t!("ai_news.telemetry.overtakes", n = passes.len())
        );
        // Corta o MEIO, nunca o fim: o último evento é o que define quem cruzou a
        // linha na frente, e é justamente ele que a narrativa não pode ignorar.
        if passes.len() <= 8 {
            for p in &passes {
                let _ = writeln!(f, "  · {}", p.frase());
            }
        } else {
            for p in passes.iter().take(4) {
                let _ = writeln!(f, "  · {}", p.frase());
            }
            let _ = writeln!(
                f,
                "{}",
                rust_i18n::t!("ai_news.telemetry.overtakes_gap", n = passes.len() - 8)
            );
            for p in passes.iter().skip(passes.len() - 4) {
                let _ = writeln!(f, "  · {}", p.frase());
            }
        }

        // Desfecho de cada duelo: a passagem no meio da corrida não decide nada, e
        // sem esta régua a narrativa transforma uma ultrapassagem desfeita depois
        // em "deixou o rival para trás".
        if let Some(you) = player_final_position(tel) {
            let finais = final_position_by_name(tel);
            let mut vistos: Vec<&str> = Vec::new();
            for p in &passes {
                if !vistos.contains(&p.rival.as_str()) {
                    vistos.push(p.rival.as_str());
                }
            }
            let mut linhas = String::new();
            for nome in vistos {
                let Some(&deles) = finais.get(nome) else {
                    continue;
                };
                if deles == you {
                    continue;
                }
                let key = if you < deles {
                    "ai_news.telemetry.h2h_ahead"
                } else {
                    "ai_news.telemetry.h2h_behind"
                };
                let _ = writeln!(
                    linhas,
                    "{}",
                    rust_i18n::t!(key, name = nome, you = you, rival = deles)
                );
            }
            if !linhas.is_empty() {
                let _ = writeln!(f, "{}", rust_i18n::t!("ai_news.telemetry.h2h_head"));
                let _ = write!(f, "{linhas}");
            }
        }
    }

    if let Some(first) = player_first_position(tel) {
        if grid_position > 0 && first as i32 != grid_position {
            let d = grid_position - first as i32;
            let verb = if d > 0 {
                rust_i18n::t!("ai_news.telemetry.start_gained", n = d).to_string()
            } else {
                rust_i18n::t!("ai_news.telemetry.start_lost", n = d.abs()).to_string()
            };
            let _ = writeln!(
                f,
                "{}",
                rust_i18n::t!(
                    "ai_news.telemetry.start",
                    grid = grid_position,
                    first = first,
                    verb = verb
                )
            );
        }
    }

    if let Some(lap) = tel
        .get("best_moment")
        .and_then(|m| m.get("lap"))
        .and_then(|x| x.as_i64())
        .filter(|l| *l > 0)
    {
        let g = tel
            .get("best_moment")
            .and_then(|m| m.get("positions_gained"))
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        let extra = if g > 0 {
            rust_i18n::t!("ai_news.telemetry.best_moment_extra", n = g).to_string()
        } else {
            String::new()
        };
        let _ = writeln!(
            f,
            "{}",
            rust_i18n::t!("ai_news.telemetry.best_moment", lap = lap, extra = extra)
        );
    }

    if let Some(lap) = tel
        .get("mistake")
        .and_then(|m| m.get("lap"))
        .and_then(|x| x.as_i64())
        .filter(|l| *l > 0)
    {
        let l = tel
            .get("mistake")
            .and_then(|m| m.get("positions_lost"))
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        let t = tel
            .get("mistake")
            .and_then(|m| m.get("time_lost_ms"))
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0)
            / 1000.0;
        let mut extra = Vec::new();
        if t >= 0.3 {
            extra.push(
                rust_i18n::t!("ai_news.telemetry.mistake_time", secs = format!("{t:.1}"))
                    .to_string(),
            );
        }
        if l > 0 {
            extra.push(rust_i18n::t!("ai_news.telemetry.mistake_pos", n = l).to_string());
        }
        let tail = if extra.is_empty() {
            String::new()
        } else {
            format!(" — {}", extra.join(", "))
        };
        let _ = writeln!(
            f,
            "{}",
            rust_i18n::t!("ai_news.telemetry.mistake", lap = lap, tail = tail)
        );
    }

    if let Some(charts) = tel.get("charts") {
        if let Some(rn) = charts.get("rival_name").and_then(|x| x.as_str()) {
            if let Some(gap) = charts
                .get("rival_gap")
                .and_then(|x| x.as_array())
                .and_then(|a| a.last())
                .and_then(|last| last.get("gap_s"))
                .and_then(|x| x.as_f64())
            {
                let who = if gap > 0.0 {
                    rust_i18n::t!("ai_news.telemetry.duel_ahead")
                } else {
                    rust_i18n::t!("ai_news.telemetry.duel_behind")
                };
                let _ = writeln!(
                    f,
                    "{}",
                    rust_i18n::t!(
                        "ai_news.telemetry.duel",
                        name = rn,
                        who = who,
                        secs = format!("{:.1}", gap.abs())
                    )
                );
            }
        }
    }

    f
}
