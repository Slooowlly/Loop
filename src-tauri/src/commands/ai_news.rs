//! Comando lazy do boletim de IA.
//!
//! Quando o jogador ABRE uma notícia de corrida, o front chama este comando. Ele
//! lê os fatos curados guardados no fim da corrida, manda ao servidor (que chama
//! o Gemini) e devolve o boletim — cacheando o resultado. Em qualquer falha,
//! devolve `story: None` e o front cai no texto-template padrão (nunca quebra).

use rusqlite::OptionalExtension;
use serde::Serialize;
use tauri::Manager;

use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::db::queries::{ai_post_race, ai_pre_race, ai_story, meta};
use crate::narrative::client::{self, StoryError};

/// Chave no `meta` (career.db) com a sequência de prévias pré-corrida que o jogador
/// NÃO leu seguidas. Usada para só gastar IA com quem lê.
const PRE_RACE_STREAK_KEY: &str = "pre_race_unread_streak";

/// Decide se a prévia da próxima corrida deve usar IA, a partir da sequência de
/// "não-leu". 0 = vinha lendo → IA; 1 = alterna p/ template; 2 = mais uma chance de
/// IA; ≥3 = ignorou 3 seguidas → só template. Qualquer leitura zera a sequência.
fn pre_race_use_ai(unread_streak: i64) -> bool {
    unread_streak == 0 || unread_streak == 2
}

#[derive(Serialize)]
pub struct AiNewsResult {
    /// O boletim redigido, se disponível. `None` → front usa o texto padrão.
    pub story: Option<String>,
    /// ok | cached | unavailable | rate_limited | error
    pub status: String,
    /// Mapa nome da equipe → cor primária das equipes da corrida (p/ colorir os
    /// nomes no boletim). `None` se a notícia não tem fatos de IA.
    pub teams: Option<serde_json::Value>,
}

#[tauri::command]
pub fn enrich_race_news_ai(
    app: tauri::AppHandle,
    career_id: String,
    news_id: String,
    reading_seconds: Option<f64>,
) -> Result<AiNewsResult, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    // Idioma + install_id vêm do config do app (get_or_create persiste o id).
    let mut config = AppConfig::load_or_default(&base_dir);
    let install_id = config.get_or_create_install_id();
    let lang = config.language.clone();

    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db =
        Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let row = ai_story::get_story(&db.conn, &news_id)
        .map_err(|e| format!("Falha ao ler cache do boletim: {e:?}"))?;

    // Sem fatos guardados → não há boletim de IA para esta notícia (ex.: corrida
    // antiga, ou notícia que não é a do jogador). Front usa o template.
    let Some(row) = row else {
        return Ok(AiNewsResult {
            story: None,
            status: "unavailable".to_string(),
            teams: None,
        });
    };

    // Cores das equipes da corrida (mapa nome→cor). Acompanha story em todo retorno.
    let teams = row
        .teams_json
        .as_deref()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());

    // Já gerado antes → devolve do cache (instantâneo, sem tocar no servidor).
    if let Some(story) = row.story {
        return Ok(AiNewsResult {
            story: Some(story),
            status: "cached".to_string(),
            teams,
        });
    }

    // 1ª vez: gera no servidor e cacheia.
    match client::fetch_story(&row.facts, &lang, &install_id, reading_seconds) {
        Ok(story) => {
            if let Err(e) = ai_story::set_story(&db.conn, &news_id, &story) {
                eprintln!("[narrative] Falha ao cachear boletim: {e:?}");
            }
            Ok(AiNewsResult {
                story: Some(story),
                status: "ok".to_string(),
                teams,
            })
        }
        Err(StoryError::RateLimited) => Ok(AiNewsResult {
            story: None,
            status: "rate_limited".to_string(),
            teams,
        }),
        Err(_) => Ok(AiNewsResult {
            story: None,
            status: "error".to_string(),
            teams,
        }),
    }
}

/// Resultado da prévia pré-corrida por IA (Sala de Estratégia). `None` nos textos →
/// o front cai no template atual (narrativa + voz da equipe geradas localmente).
#[derive(Serialize)]
pub struct PreRaceAiResult {
    /// Manchete cinematográfica (negrito no card). `None` → front usa o template.
    pub headline: Option<String>,
    /// Corpo da prévia (1-2 parágrafos).
    pub narrative: Option<String>,
    pub team_voice: Option<String>,
    /// ok | cached | rate_limited | unavailable | error
    pub status: String,
}

/// Prévia pré-corrida (narrativa + voz da equipe, CURTAS) para a Sala de Estratégia.
/// O front monta os `facts` do briefing e chama isto ao abrir a tela. Cacheia por
/// `race_id` (reentrar não regenera). Em cooldown/rede/cota → textos `None` e o front
/// usa o template. O cooldown de 10 min entre prévias é imposto pelo servidor
/// (escopo "pre-race", separado do boletim pós-corrida).
#[tauri::command]
pub fn pre_race_briefing_ai(
    app: tauri::AppHandle,
    career_id: String,
    race_id: String,
    facts: String,
    force: Option<bool>,
) -> Result<PreRaceAiResult, String> {
    let force = force.unwrap_or(false);
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    let mut config = AppConfig::load_or_default(&base_dir);
    let install_id = config.get_or_create_install_id();
    let lang = config.language.clone();

    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db =
        Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    // Cache por etapa → reentrar na tela não regenera (sem custo, sem cooldown).
    // No reroll de debug (force) ignoramos o cache e regeramos pelo servidor.
    if !force {
        if let Ok(Some(row)) = ai_pre_race::get_pre_race(&db.conn, &race_id) {
            return Ok(PreRaceAiResult {
                headline: Some(row.headline),
                narrative: Some(row.narrative),
                team_voice: Some(row.team_voice),
                status: "cached".to_string(),
            });
        }

        // Gate de engajamento: se o jogador não vem lendo a prévia, segura/alterna no
        // template para não gastar IA (sem tocar no servidor). O reroll (force) pula.
        let streak = meta::get_meta_value(&db.conn, PRE_RACE_STREAK_KEY)
            .ok()
            .flatten()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        if !pre_race_use_ai(streak) {
            return Ok(PreRaceAiResult {
                headline: None,
                narrative: None,
                team_voice: None,
                status: "engagement_template".to_string(),
            });
        }
    }

    if facts.trim().is_empty() {
        return Ok(PreRaceAiResult {
            headline: None,
            narrative: None,
            team_voice: None,
            status: "unavailable".to_string(),
        });
    }

    match client::fetch_pre_race_briefing(&facts, &lang, &install_id, force) {
        Ok(b) => {
            // O corpo cinematográfico é guardado na coluna `narrative` do cache.
            if let Err(e) =
                ai_pre_race::set_pre_race(&db.conn, &race_id, &b.headline, &b.body, &b.team_voice)
            {
                eprintln!("[narrative] Falha ao cachear prévia pré-corrida: {e:?}");
            }
            Ok(PreRaceAiResult {
                headline: Some(b.headline),
                narrative: Some(b.body),
                team_voice: Some(b.team_voice),
                status: "ok".to_string(),
            })
        }
        Err(StoryError::RateLimited) => Ok(PreRaceAiResult {
            headline: None,
            narrative: None,
            team_voice: None,
            status: "rate_limited".to_string(),
        }),
        Err(_) => Ok(PreRaceAiResult {
            headline: None,
            narrative: None,
            team_voice: None,
            status: "error".to_string(),
        }),
    }
}

/// Reporta se o jogador LEU a prévia pré-corrida (ficou tempo suficiente na Sala de
/// Estratégia) e atualiza a sequência de "não-leu" no `meta`. Leu → zera; não leu →
/// +1 (limitado). O front chama isto ao simular/sair da tela. Devolve a sequência
/// nova (útil só para debug). Falha de IO vira erro string, mas o front ignora.
#[tauri::command]
pub fn report_pre_race_engagement(
    app: tauri::AppHandle,
    career_id: String,
    read: bool,
) -> Result<i64, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db =
        Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let streak = meta::get_meta_value(&db.conn, PRE_RACE_STREAK_KEY)
        .ok()
        .flatten()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let next = if read { 0 } else { (streak + 1).min(10) };
    meta::put_meta_value(&db.conn, PRE_RACE_STREAK_KEY, &next.to_string())
        .map_err(|e| format!("Falha ao gravar engajamento: {e:?}"))?;
    Ok(next)
}

// ─── Debrief pós-corrida do engenheiro (voz única, com calor) ────────────────────

#[derive(Serialize)]
pub struct PostRaceAiResult {
    /// Manchete do debrief. `None` → front usa o texto determinístico (cérebro).
    pub headline: Option<String>,
    /// Parágrafo do engenheiro (2ª pessoa). `None` → front usa o determinístico.
    pub body: Option<String>,
    /// ok | cached | unavailable | rate_limited | error
    pub status: String,
}

fn weather_pt(w: &str) -> &'static str {
    match w {
        "HeavyRain" => "chuva forte",
        "Wet" => "chuva",
        "Damp" => "úmido",
        _ => "seco",
    }
}

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

/// Reconstrói as ultrapassagens do jogador a partir do race trace: cada vez que a
/// posição dele muda entre dois pontos, o rival envolvido é o carro que assumiu a
/// posição ANTIGA do jogador naquele mesmo instante. Devolve frases prontas.
fn overtake_feed(tel: &serde_json::Value) -> Vec<String> {
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
        let name = car
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("Carro")
            .to_string();
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
            .unwrap_or_else(|| "rival".to_string());
        if pos1 < pos0 {
            out.push(format!("volta {lap1:.1}: passou {rival} (subiu para P{pos1})"));
        } else {
            out.push(format!("volta {lap1:.1}: perdido para {rival} (caiu para P{pos1})"));
        }
    }
    out
}

/// Transforma o bloco `telemetry` do race_screens em fatos pro debrief. PURO
/// (testável): recebe o Value da telemetria e a posição de largada. Vazio se não
/// houver telemetria de verdade.
fn telemetry_facts(tel: Option<&serde_json::Value>, grid_position: i32) -> String {
    use std::fmt::Write;
    let Some(tel) = tel else {
        return String::new();
    };
    if tel.get("has_telemetry").and_then(|x| x.as_bool()) != Some(true) {
        return String::new();
    }

    let mut f = String::new();
    let _ = writeln!(f, "\nTELEMETRIA (dados reais da pista):");

    if let Some(pace) = tel.get("pace") {
        let vs_grid = pace.get("vs_grid_ms").and_then(|x| x.as_f64());
        let reliable = pace
            .get("vs_grid_reliable")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        if let Some(v) = vs_grid {
            if reliable && v.abs() >= 30.0 {
                let s = v.abs() / 1000.0;
                let dir = if v < 0.0 { "MAIS RÁPIDO" } else { "mais lento" };
                let _ = writeln!(f, "- Ritmo: {s:.2}s/volta {dir} que a média do grid");
            }
        }
        let good = pace.get("good_laps").and_then(|x| x.as_i64()).unwrap_or(0);
        if good > 0 {
            let _ = writeln!(f, "- Voltas limpas (base de ritmo): {good}");
        }
    }

    if let Some(deg) = tire_deg_ms_per_lap(tel) {
        if deg >= 40.0 {
            let _ = writeln!(
                f,
                "- Degradação: ritmo caiu ~{:.2}s/volta ao longo do stint",
                deg / 1000.0
            );
        } else if deg <= -40.0 {
            let _ = writeln!(
                f,
                "- Ritmo MELHOROU ~{:.2}s/volta ao longo do stint (pista/pneu esquentando)",
                deg.abs() / 1000.0
            );
        }
    }

    if let Some(pf) = tel.get("position_flow") {
        let gained = pf.get("gained_on_track").and_then(|x| x.as_i64()).unwrap_or(0);
        let lost = pf.get("lost_on_track").and_then(|x| x.as_i64()).unwrap_or(0);
        if gained > 0 || lost > 0 {
            let _ = writeln!(f, "- Na pista: ganhou {gained}, perdeu {lost} (posições)");
        }
    }

    if let Some(fuel) = tel.get("fuel") {
        let per_lap = fuel.get("used_per_lap_l").and_then(|x| x.as_f64());
        let laps_left = fuel.get("laps_left").and_then(|x| x.as_f64());
        if let (Some(pl), Some(ll)) = (per_lap, laps_left) {
            if pl > 0.0 {
                let _ = writeln!(
                    f,
                    "- Combustível: {pl:.2} L/volta, terminou com ~{ll:.1} voltas de autonomia"
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
                    "- Setores (seu melhor): S1 {s1:.1}s · S2 {s2:.1}s · S3 {s3:.1}s"
                );
            }
        }
        let weak = sec.get("weakest_sector").and_then(|x| x.as_i64()).unwrap_or(0);
        let loss = sec.get("weakest_loss_ms").and_then(|x| x.as_f64()).unwrap_or(0.0) / 1000.0;
        if weak >= 1 && loss >= 0.1 {
            let _ = writeln!(
                f,
                "- Ponto fraco: setor {weak} (perde ~{loss:.2}s vs seu melhor por volta)"
            );
        }
    }

    let passes = overtake_feed(tel);
    if !passes.is_empty() {
        let _ = writeln!(f, "- Ultrapassagens ({} no total):", passes.len());
        for p in passes.iter().take(8) {
            let _ = writeln!(f, "  · {p}");
        }
    }

    if let Some(first) = player_first_position(tel) {
        if grid_position > 0 && first as i32 != grid_position {
            let d = grid_position - first as i32;
            let verb = if d > 0 {
                format!("ganhou {d}")
            } else {
                format!("perdeu {}", d.abs())
            };
            let _ = writeln!(
                f,
                "- Largada: P{grid_position} → P{first} ({verb} na 1ª volta captada)"
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
            format!(" (+{g} posições)")
        } else {
            String::new()
        };
        let _ = writeln!(f, "- Melhor momento: volta {lap}{extra}");
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
            extra.push(format!("~{t:.1}s perdidos"));
        }
        if l > 0 {
            extra.push(format!("{l} posição(ões)"));
        }
        let tail = if extra.is_empty() {
            String::new()
        } else {
            format!(" — {}", extra.join(", "))
        };
        let _ = writeln!(f, "- Erro mais caro: volta {lap}{tail}");
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
                let who = if gap > 0.0 { "à sua frente" } else { "atrás de você" };
                let _ = writeln!(
                    f,
                    "- Duelo direto: {rn} terminou {who} por {:.1}s",
                    gap.abs()
                );
            }
        }
    }

    f
}

/// Monta o "fact bundle" do pós-corrida a partir da tela salva (resultado +
/// manutenção + avaliação) cruzada com o banco (companheiro, rival de campeonato,
/// últimas 3 corridas, contexto da pré-corrida). Tudo FATO — o tom/voz fica no
/// prompt do servidor. String vazia → sem fatos suficientes (front cai no template).
fn build_post_race_facts(
    conn: &rusqlite::Connection,
    career_dir: &std::path::Path,
    race_id: &str,
) -> String {
    use crate::commands::race::MaintenanceBreakdown;
    use crate::race_eval::RaceEvaluation;
    use crate::simulation::race::RaceResult;
    use std::fmt::Write;

    let path = career_dir.join("race_screens").join(format!("{race_id}.json"));
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

    let mut f = String::new();
    let _ = writeln!(f, "PISTA: {}", result.track_name);
    let _ = writeln!(f, "CLIMA: {}", weather_pt(&result.weather));
    if !categoria.is_empty() {
        let _ = writeln!(f, "CATEGORIA: {categoria}");
    }
    let calendar_entry = crate::db::queries::calendar::get_calendar_entry_by_id(conn, race_id)
        .ok()
        .flatten();
    let season_num = crate::db::queries::seasons::get_active_season(conn)
        .ok()
        .flatten()
        .map(|s| s.numero)
        .unwrap_or(0);
    if let Some(entry) = &calendar_entry {
        let _ = writeln!(f, "TEMPORADA {season_num} · RODADA {}", entry.rodada);
    }
    let _ = writeln!(f, "VOLTAS: {}", result.total_laps);

    if let Some(ev) = &evaluation {
        let _ = writeln!(
            f,
            "\nMETA DA CORRIDA: P{}–P{} (faixa esperada do conjunto)",
            ev.target_low, ev.target_high
        );
        let _ = writeln!(f, "NOTA: {:.1}/10 ({})", ev.grade, ev.assessment.label());
    }

    let _ = writeln!(f, "\nSEU RESULTADO:");
    let _ = writeln!(f, "- Largou em P{}", player.grid_position);
    if player.is_dnf {
        match player.dnf_reason.as_deref().filter(|s| !s.is_empty()) {
            Some(m) => {
                let _ = writeln!(f, "- ABANDONOU (DNF): {m}");
            }
            None => {
                let _ = writeln!(f, "- ABANDONOU (DNF)");
            }
        }
    } else {
        let _ = writeln!(f, "- Chegou em P{}", player.finish_position);
    }
    let saldo = player.positions_gained;
    let saldo_txt = if saldo > 0 {
        format!("ganhou {saldo} posições")
    } else if saldo < 0 {
        format!("perdeu {} posições", saldo.abs())
    } else {
        "manteve a posição de largada".to_string()
    };
    let _ = writeln!(f, "- Saldo: {saldo_txt}");
    if !player.is_dnf && player.finish_position > 1 {
        let gap = player.gap_to_winner_ms / 1000.0;
        if gap > 0.0 {
            let _ = writeln!(f, "- Gap pro vencedor: +{gap:.3}s");
        }
    }
    if player.best_lap_time_ms > 0.0 {
        let s = player.best_lap_time_ms / 1000.0;
        let m = (s / 60.0).floor();
        let rest = s - m * 60.0;
        let fastest = if player.has_fastest_lap {
            " (VOLTA MAIS RÁPIDA DA CORRIDA)"
        } else {
            ""
        };
        let _ = writeln!(f, "- Melhor volta: {}:{:06.3}{fastest}", m as i64, rest);
    }
    let _ = writeln!(f, "- Pontos: {}", player.points_earned);
    let _ = writeln!(f, "- Incidentes: {}", player.incidents_count);

    // Companheiro de equipe (o duelo que mais importa).
    if let Some(mate) = result
        .race_results
        .iter()
        .find(|r| r.team_id == player.team_id && !r.is_jogador)
    {
        let mate_pos = if mate.is_dnf {
            "DNF".to_string()
        } else {
            format!("P{}", mate.finish_position)
        };
        let cmp = if player.is_dnf || mate.is_dnf {
            ""
        } else if player.finish_position < mate.finish_position {
            " — você ficou na frente dele"
        } else {
            " — ele ficou na sua frente"
        };
        let _ = writeln!(
            f,
            "\nCOMPANHEIRO DE EQUIPE: {} terminou em {mate_pos}{cmp}",
            mate.pilot_name
        );
    }

    // Rival de campeonato (o MESMO que a pré-corrida marcou).
    if !categoria.is_empty() {
        if let Ok(Some(rival)) =
            crate::commands::career::build_primary_rival_summary(conn, &player.pilot_id, &categoria)
        {
            let standing = if rival.is_ahead {
                format!("à sua frente no campeonato por {}pts", rival.gap_points)
            } else {
                format!("atrás de você no campeonato por {}pts", rival.gap_points)
            };
            match result
                .race_results
                .iter()
                .find(|r| r.pilot_id == rival.driver_id)
            {
                Some(rr) => {
                    let rpos = if rr.is_dnf {
                        "DNF".to_string()
                    } else {
                        format!("P{}", rr.finish_position)
                    };
                    let cmp = if !player.is_dnf && !rr.is_dnf {
                        if player.finish_position < rr.finish_position {
                            " (você chegou na frente dele hoje)"
                        } else {
                            " (ele chegou na sua frente hoje)"
                        }
                    } else {
                        ""
                    };
                    let _ = writeln!(
                        f,
                        "RIVAL DE CAMPEONATO: {} ({standing}) terminou em {rpos}{cmp}",
                        rival.driver_name
                    );
                }
                None => {
                    let _ = writeln!(
                        f,
                        "RIVAL DE CAMPEONATO: {} ({standing}) — não correu esta etapa",
                        rival.driver_name
                    );
                }
            }
        }
    }

    // Telemetria real da pista (ritmo, ultrapassagens, degradação, erro/melhor
    // momento, duelo direto) — o bloco `telemetry` da própria tela salva.
    if !player.is_dnf {
        let _ = write!(f, "{}", telemetry_facts(v.get("telemetry"), player.grid_position));
    }

    // Últimas 3 corridas (memória curta pro engenheiro).
    if !categoria.is_empty() {
        if let Some(entry) = &calendar_entry {
            if let Ok(recent) = crate::db::queries::race_history::get_recent_finishes_before(
                conn,
                &player.pilot_id,
                &categoria,
                season_num,
                entry.rodada,
                3,
            ) {
                if !recent.is_empty() {
                    let items: Vec<String> = recent
                        .iter()
                        .map(|r| {
                            if r.is_dnf {
                                format!("R{}: DNF", r.round)
                            } else {
                                format!("R{}: P{}", r.round, r.finish)
                            }
                        })
                        .collect();
                    let _ = writeln!(
                        f,
                        "\nSUAS ÚLTIMAS CORRIDAS (mais recente primeiro): {}",
                        items.join(" · ")
                    );
                }
            }
        }
    }

    // Manutenção / batida.
    if maintenance.total > 0.0 {
        let _ = writeln!(
            f,
            "\nMANUTENÇÃO DO CARRO: $ {} no total",
            maintenance.total.round() as i64
        );
        let danos: Vec<String> = maintenance
            .items
            .iter()
            .filter(|i| !matches!(i.key.as_str(), "gasolina" | "pneus"))
            .map(|i| format!("{} $ {}", i.label, i.cost.round() as i64))
            .collect();
        if !danos.is_empty() {
            let _ = writeln!(f, "- CONSERTO DA BATIDA: {}", danos.join(", "));
        }
    }

    // Contexto da pré-corrida (pra FECHAR o loop do que foi prometido).
    if let Ok(Some(pre)) = crate::db::queries::ai_pre_race::get_pre_race(conn, race_id) {
        let _ = writeln!(f, "\nO QUE A EQUIPE TE DISSE ANTES DA LARGADA:");
        if !pre.headline.is_empty() {
            let _ = writeln!(f, "Manchete: {}", pre.headline);
        }
        if !pre.narrative.is_empty() {
            let _ = writeln!(f, "Briefing: {}", pre.narrative);
        }
    }

    f
}

/// Comando lazy do debrief pós-corrida: o front chama quando o jogador abre a aba
/// Debrief. Monta os fatos no Rust, manda ao servidor (voz única do engenheiro) e
/// cacheia por `race_id` — reabrir não regenera. Sem gate de engajamento (o jogador
/// sempre olha o resultado). Em qualquer falha devolve `None` e o front usa o
/// texto determinístico do cérebro (nunca quebra).
#[tauri::command]
pub fn post_race_debrief_ai(
    app: tauri::AppHandle,
    career_id: String,
    race_id: String,
    force: Option<bool>,
) -> Result<PostRaceAiResult, String> {
    let force = force.unwrap_or(false);
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    let mut config = AppConfig::load_or_default(&base_dir);
    let install_id = config.get_or_create_install_id();
    let lang = config.language.clone();

    let career_dir = config.saves_dir().join(&career_id);
    let db_path = career_dir.join("career.db");
    let db =
        Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    if !force {
        if let Ok(Some(row)) = ai_post_race::get_post_race(&db.conn, &race_id) {
            return Ok(PostRaceAiResult {
                headline: Some(row.headline),
                body: Some(row.body),
                status: "cached".to_string(),
            });
        }
    }

    let facts = build_post_race_facts(&db.conn, &career_dir, &race_id);
    if facts.trim().is_empty() {
        return Ok(PostRaceAiResult {
            headline: None,
            body: None,
            status: "unavailable".to_string(),
        });
    }

    match client::fetch_post_race_debrief(&facts, &lang, &install_id, force) {
        Ok(d) => {
            if let Err(e) = ai_post_race::set_post_race(&db.conn, &race_id, &d.headline, &d.body) {
                eprintln!("[narrative] Falha ao cachear debrief pós-corrida: {e:?}");
            }
            Ok(PostRaceAiResult {
                headline: Some(d.headline),
                body: Some(d.body),
                status: "ok".to_string(),
            })
        }
        Err(StoryError::RateLimited) => Ok(PostRaceAiResult {
            headline: None,
            body: None,
            status: "rate_limited".to_string(),
        }),
        Err(_) => Ok(PostRaceAiResult {
            headline: None,
            body: None,
            status: "error".to_string(),
        }),
    }
}

/// Resolve o `news_id` da notícia de Corrida do JOGADOR para uma (temporada, rodada).
///
/// A revista (NewsMagazineTab) monta as edições a partir do calendário; para puxar o
/// boletim de IA de cada etapa ela precisa do `news_id` correspondente. Como os fatos
/// de IA só são guardados para a corrida do jogador (uma por rodada), basta cruzar
/// `ai_race_story` com `news` pela rodada da temporada. Devolve `None` quando não há
/// boletim para a etapa (ex.: corrida simulada antes do recurso existir) → o front
/// usa o texto-placeholder.
#[tauri::command]
pub fn player_race_news_id(
    app: tauri::AppHandle,
    career_id: String,
    season_id: String,
    rodada: i32,
) -> Result<Option<String>, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db =
        Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    // Se a tabela ai_race_story ainda não existe (save sem nenhum boletim), a query
    // falha — tratamos como "sem boletim" em vez de erro.
    let id = db
        .conn
        .query_row(
            "SELECT n.id
               FROM news n
               JOIN ai_race_story a ON a.news_id = n.id
              WHERE n.temporada_id = ?1 AND n.rodada = ?2
              LIMIT 1",
            rusqlite::params![season_id, rodada],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .unwrap_or(None);

    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn telemetry_facts_resume_ritmo_ultrapassagens_e_erro() {
        let tel = json!({
            "has_telemetry": true,
            "pace": { "vs_grid_ms": -506.0, "vs_grid_reliable": true, "good_laps": 8 },
            "position_flow": { "gained_on_track": 4, "lost_on_track": 1 },
            "best_moment": { "lap": 8, "positions_gained": 3 },
            "mistake": { "lap": 9, "positions_lost": 1, "time_lost_ms": 600.0 },
            "charts": {
                "rival_name": "Massimo Caruso",
                "rival_gap": [ { "lap": 13.0, "gap_s": 0.8 } ],
                "lap_times": [
                    { "lap": 6.0, "time_s": 71.0 },
                    { "lap": 7.0, "time_s": 71.3 },
                    { "lap": 8.0, "time_s": 71.6 },
                    { "lap": 9.0, "time_s": 71.9 }
                ],
                "cars": [
                    { "idx": 0, "is_player": true, "name": "Você", "points": [
                        { "lap": 6.0, "position": 7 },
                        { "lap": 7.4, "position": 4 },
                        { "lap": 9.0, "position": 6 }
                    ] },
                    { "idx": 1, "is_player": false, "name": "Bruno Perez", "points": [
                        { "lap": 6.0, "position": 4 },
                        { "lap": 7.4, "position": 7 },
                        { "lap": 9.0, "position": 4 }
                    ] }
                ]
            }
        });
        let out = telemetry_facts(Some(&tel), 8);
        assert!(out.contains("MAIS RÁPIDO"), "ritmo vs grid: {out}");
        assert!(out.contains("Degradação"), "degradação: {out}");
        assert!(
            out.contains("volta 7.4: passou Bruno Perez"),
            "feed de ultrapassagem: {out}"
        );
        assert!(out.contains("Largada: P8 → P7"), "largada: {out}");
        assert!(out.contains("Erro mais caro: volta 9"), "erro: {out}");
        assert!(
            out.contains("Massimo Caruso terminou à sua frente"),
            "duelo direto: {out}"
        );
    }

    #[test]
    fn telemetry_facts_vazio_sem_telemetria() {
        assert!(telemetry_facts(None, 5).is_empty());
        assert!(telemetry_facts(Some(&json!({ "has_telemetry": false })), 5).is_empty());
    }
}
