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
/// Monta o bloco "MEMÓRIA RECENTE" que dá continuidade entre etapas: uma LISTA CURTA
/// das últimas até 3 corridas do jogador na categoria (chegada + manchete do debrief
/// de cada uma). É de propósito compacto — apenas o fio da meada para o servidor
/// retomar a voz, NÃO o debrief inteiro.
///
/// Histórico: antes este bloco despejava o corpo COMPLETO do debrief anterior. Com a
/// prévia reescrita em torno de uma tese dominante (o front já marca o eixo, ex.:
/// "reação a um DNF"), esse despejo passava por cima do eixo e fazia o texto colapsar
/// no último tombo. Agora a memória reforça o eixo em vez de competir com ele.
///
/// Só depende do banco (`conn` + `race_id`), como `build_post_race_facts`. Sem
/// histórico (1ª corrida da carreira / 1ª na categoria) devolve string vazia e o
/// briefing fica idêntico ao de hoje. Etapa antiga sem debrief de IA (offline / gate
/// de engajamento) ainda entra pela chegada — a fala aparece só quando existe.
fn build_recent_arc_facts(conn: &rusqlite::Connection, race_id: &str) -> String {
    use std::fmt::Write;

    let Some(entry) = crate::db::queries::calendar::get_calendar_entry_by_id(conn, race_id)
        .ok()
        .flatten()
    else {
        return String::new();
    };
    let Ok(player) = crate::db::queries::drivers::get_player_driver(conn) else {
        return String::new();
    };
    let season_num = crate::db::queries::seasons::get_active_season(conn)
        .ok()
        .flatten()
        .map(|s| s.numero)
        .unwrap_or(0);

    let recent = match crate::db::queries::race_history::get_recent_races_before(
        conn,
        &player.id,
        &entry.categoria,
        season_num,
        entry.rodada,
        3,
    ) {
        Ok(r) if !r.is_empty() => r,
        _ => return String::new(),
    };

    let mut f = String::new();
    let _ = writeln!(f, "{}", rust_i18n::t!("ai_news.arc.header"));
    for r in &recent {
        let resultado = if r.is_dnf {
            rust_i18n::t!("ai_news.arc.dnf").to_string()
        } else {
            format!("P{}", r.finish)
        };
        let manchete = crate::db::queries::ai_post_race::get_post_race(conn, &r.race_id)
            .ok()
            .flatten()
            .map(|d| d.headline)
            .filter(|h| !h.is_empty());
        let round = r.round.to_string();
        match manchete {
            Some(h) => {
                let _ = writeln!(
                    f,
                    "{}",
                    rust_i18n::t!(
                        "ai_news.arc.line_headline",
                        round = round.as_str(),
                        track = r.track_name.as_str(),
                        result = resultado.as_str(),
                        headline = h.as_str()
                    )
                );
            }
            None => {
                let _ = writeln!(
                    f,
                    "{}",
                    rust_i18n::t!(
                        "ai_news.arc.line",
                        round = round.as_str(),
                        track = r.track_name.as_str(),
                        result = resultado.as_str()
                    )
                );
            }
        }
    }

    f
}

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

    // Arco narrativo entre etapas: anexa a memória das últimas corridas (chegada +
    // manchete do debrief, e o corpo do debrief anterior) para o engenheiro retomar de
    // onde parou. Vazio na 1ª corrida da carreira/categoria → briefing inalterado.
    let facts = {
        let arc = build_recent_arc_facts(&db.conn, &race_id);
        if arc.trim().is_empty() {
            facts
        } else {
            format!("{facts}\n{arc}")
        }
    };

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

fn weather_label(w: &str) -> String {
    let key = match w {
        "HeavyRain" => "ai_news.weather.heavy_rain",
        "Wet" => "ai_news.weather.wet",
        "Damp" => "ai_news.weather.damp",
        _ => "ai_news.weather.dry",
    };
    rust_i18n::t!(key).to_string()
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
        let lap = format!("{lap1:.1}");
        let pos = pos1.to_string();
        if pos1 < pos0 {
            out.push(
                rust_i18n::t!(
                    "ai_news.overtake.passed",
                    lap = lap.as_str(),
                    rival = rival.as_str(),
                    pos = pos.as_str()
                )
                .to_string(),
            );
        } else {
            out.push(
                rust_i18n::t!(
                    "ai_news.overtake.lost",
                    lap = lap.as_str(),
                    rival = rival.as_str(),
                    pos = pos.as_str()
                )
                .to_string(),
            );
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
        for p in passes.iter().take(8) {
            let _ = writeln!(f, "  · {p}");
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

/// Duelo pessoal DECIDIDO na corrida (nemesis ou rival de pista) — usado quando o
/// resultado em si foi morno mas o confronto direto é a história do dia.
struct PostRaceDuel {
    name: String,
    player_won: bool,
    is_nemesis: bool,
    h2h: Option<(i32, i32)>,
}

/// Sinais destilados da corrida que alimentam a tese do debrief.
struct PostRaceSignals {
    is_dnf: bool,
    dnf_mechanical: bool,
    grid: i32,
    finish: i32,
    positions_gained: i32,
    has_fastest_lap: bool,
    assessment: Option<crate::race_eval::Assessment>,
    target_low: i32,
    target_high: i32,
    duel: Option<PostRaceDuel>,
    track_name: String,
}

/// A TESE DOMINANTE do debrief pós-corrida. Mesmo princípio da prévia (nextRaceThesis.js):
/// em vez de despejar todos os blocos achatados e deixar o servidor adivinhar QUAL foi a
/// história da corrida, elegemos UM eixo e organizamos o resto em APOIO/PANO DE FUNDO.
/// Semeado pelo cérebro `race_eval` (assessment/nota/meta) + o evento de destaque (DNF
/// mecânico vs erro, remontada, colapso, vitória, over/under, ou um duelo decidido).
/// Devolve (statement do eixo, ids de bloco promovidos ao APOIO). `resultado` e `pre_race`
/// são sempre promovidos pelo chamador (resultado é o núcleo; pre_race fecha o loop).
fn select_post_race_thesis(s: &PostRaceSignals) -> (String, Vec<&'static str>) {
    use crate::race_eval::Assessment::{Abaixo, Acima, MuitoAbaixo, MuitoAcima};
    let track = &s.track_name;
    let overperf = matches!(s.assessment, Some(Acima) | Some(MuitoAcima));
    let underperf = matches!(s.assessment, Some(Abaixo) | Some(MuitoAbaixo));

    // 1) DNF mecânico — o carro falhou, não foi erro seu.
    if s.is_dnf && s.dnf_mechanical {
        return (
            rust_i18n::t!("ai_news.thesis.mechanical_dnf", track = track.as_str()).to_string(),
            vec!["breakdowns", "maintenance"],
        );
    }
    // 2) DNF por incidente/contato — fim precoce na pista.
    if s.is_dnf {
        return (
            rust_i18n::t!("ai_news.thesis.incident_dnf", track = track.as_str()).to_string(),
            vec![],
        );
    }
    // 3) Vitória — a manchete é a própria vitória.
    if s.finish == 1 {
        let fl = if s.has_fastest_lap {
            rust_i18n::t!("ai_news.thesis.win_fastest").to_string()
        } else {
            String::new()
        };
        return (
            rust_i18n::t!("ai_news.thesis.win", track = track.as_str(), fl = fl).to_string(),
            vec!["telemetry", "lived_rivalry", "champ_rival"],
        );
    }
    // 4) Remontada — ganhou muitas posições e não ficou abaixo da meta.
    if s.positions_gained >= 5 && !underperf {
        return (
            rust_i18n::t!(
                "ai_news.thesis.comeback",
                grid = s.grid,
                finish = s.finish,
                gained = s.positions_gained
            )
            .to_string(),
            vec!["telemetry", "eval", "lived_rivalry"],
        );
    }
    // 5) Colapso — perdeu muitas posições, ou ficou abaixo da meta largando bem.
    if s.positions_gained <= -4 || (underperf && s.grid <= s.target_low) {
        return (
            rust_i18n::t!("ai_news.thesis.collapse", grid = s.grid, finish = s.finish).to_string(),
            vec!["eval", "telemetry"],
        );
    }
    // 6) Acima do esperado (entrega além do conjunto).
    if overperf {
        return (
            rust_i18n::t!(
                "ai_news.thesis.overperform",
                finish = s.finish,
                low = s.target_low,
                high = s.target_high
            )
            .to_string(),
            vec!["eval", "telemetry"],
        );
    }
    // 7) Aquém do esperado (sem drama de abandono).
    if underperf {
        return (
            rust_i18n::t!(
                "ai_news.thesis.underperform",
                finish = s.finish,
                low = s.target_low,
                high = s.target_high
            )
            .to_string(),
            vec!["eval", "telemetry"],
        );
    }
    // 8) Resultado morno, mas um DUELO pessoal foi decidido → ele é a história.
    if let Some(d) = &s.duel {
        let verbo = if d.player_won {
            rust_i18n::t!("ai_news.thesis.duel_won")
        } else {
            rust_i18n::t!("ai_news.thesis.duel_lost")
        };
        let quem = if d.is_nemesis {
            rust_i18n::t!("ai_news.thesis.duel_nemesis")
        } else {
            rust_i18n::t!("ai_news.thesis.duel_rival")
        };
        let h2h = d
            .h2h
            .map(|(p, r)| rust_i18n::t!("ai_news.thesis.duel_h2h", p = p, r = r).to_string())
            .unwrap_or_default();
        return (
            rust_i18n::t!(
                "ai_news.thesis.duel",
                verb = verbo,
                who = quem,
                name = d.name.as_str(),
                h2h = h2h
            )
            .to_string(),
            vec!["lived_rivalry", "champ_rival"],
        );
    }
    // 9) Dia de somar — dentro do esperado, sem grande drama.
    (
        rust_i18n::t!("ai_news.thesis.points_day", finish = s.finish).to_string(),
        vec!["eval", "champ_rival"],
    )
}

/// Monta o "fact bundle" do pós-corrida a partir da tela salva (resultado +
/// manutenção + avaliação) cruzada com o banco (companheiro, rival de campeonato,
/// últimas 3 corridas, contexto da pré-corrida). Organizado em torno de uma TESE
/// dominante (EIXO → APOIO → PANO DE FUNDO); o tom/voz fica no prompt do servidor.
/// String vazia → sem fatos suficientes (front cai no template).
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
            rust_i18n::t!("ai_news.facts.scenario_category", category = categoria.as_str())
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
    let _ = writeln!(res_b, "{}", rust_i18n::t!("ai_news.facts.balance", txt = saldo_txt));
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
            let standing = if rival.is_ahead {
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
        let current =
            crate::db::queries::player_nemesis::get_current_nemesis(conn).unwrap_or(None);
        let interests =
            crate::commands::career::select_player_interests(conn, current.as_deref());
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
                if let Some(rr) = result.race_results.iter().find(|d| d.pilot_id == ri.driver_id) {
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
            let today = match result.race_results.iter().find(|d| d.pilot_id == ri.driver_id) {
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
                rust_i18n::t!("ai_news.facts.fame", level = level, value = midia.round() as i64)
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
        let all_points: Vec<f64> = cat_drivers.iter().map(|d| d.stats_temporada.pontos).collect();
        let max_race_points = (crate::constants::scoring::get_points_for_position(
            1,
            categoria == "endurance",
        ) + crate::constants::scoring::BONUS_FASTEST_LAP) as f64;
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

    // ---- Bloco: pré-corrida (FECHA o loop do que foi prometido) ----
    let mut pre_b = String::new();
    if let Ok(Some(pre)) = crate::db::queries::ai_pre_race::get_pre_race(conn, race_id) {
        let _ = writeln!(pre_b, "{}", rust_i18n::t!("ai_news.facts.pre_head"));
        if !pre.headline.is_empty() {
            let _ = writeln!(
                pre_b,
                "{}",
                rust_i18n::t!("ai_news.facts.pre_headline", headline = pre.headline.as_str())
            );
        }
        if !pre.narrative.is_empty() {
            let _ = write!(
                pre_b,
                "{}",
                rust_i18n::t!("ai_news.facts.pre_briefing", narrative = pre.narrative.as_str())
            );
        }
    }

    // DNF mecânico = peça grave no carro do jogador OU motivo textual mecânico (vs
    // batida/incidente). Separa "o carro te traiu" de "você/alguém rodou".
    let dnf_mechanical = player.is_dnf
        && (player_mech_break
            || player
                .dnf_reason
                .as_deref()
                .map(|r| {
                    let r = r.to_lowercase();
                    [
                        // PT
                        "motor", "câmbio", "cambio", "mecân", "mecan", "suspens", "freio",
                        "transmiss", "embreagem", "turbo", "óleo", "oleo", "superaquec", "pane",
                        "elétric", "eletric", "diferencial",
                        // EN (saves feitos no locale inglês guardam o motivo em inglês)
                        "engine", "gearbox", "mechanic", "suspension", "brake", "clutch", "oil",
                        "overheat", "electric", "differential", "failure",
                    ]
                    .iter()
                    .any(|k| r.contains(k))
                })
                .unwrap_or(false));

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
    for id in ["resultado", "pre_race", "injury", "pressure"] {
        if !support.contains(&id) {
            support.push(id);
        }
    }

    // ---- Montagem em camadas (EIXO → APOIO → PANO DE FUNDO) ----
    let block_for = |id: &str| -> &str {
        match id {
            "eval" => eval_b.as_str(),
            "resultado" => res_b.as_str(),
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
    use crate::race_eval::Assessment;
    use serde_json::json;
    use serial_test::serial;

    /// Os fatos saem no locale ativo; estes testes conferem a prosa PT, então fixam o
    /// idioma antes de rodar. `#[serial]` porque o locale é estado global do processo.
    fn pt() {
        rust_i18n::set_locale("pt-BR");
    }

    fn sig() -> PostRaceSignals {
        // Base neutra: terminou dentro do esperado, sem drama.
        PostRaceSignals {
            is_dnf: false,
            dnf_mechanical: false,
            grid: 6,
            finish: 6,
            positions_gained: 0,
            has_fastest_lap: false,
            assessment: Some(Assessment::Dentro),
            target_low: 5,
            target_high: 7,
            duel: None,
            track_name: "Interlagos".to_string(),
        }
    }

    fn thesis_of(s: &PostRaceSignals) -> String {
        select_post_race_thesis(s).0
    }

    #[test]
    #[serial]
    fn dnf_mecanico_vence_tudo_e_isenta_o_piloto() {
        pt();
        let mut s = sig();
        s.is_dnf = true;
        s.dnf_mechanical = true;
        s.assessment = Some(Assessment::MuitoAbaixo);
        let (stmt, support) = select_post_race_thesis(&s);
        assert!(stmt.contains("DRAMA MECÂNICO"));
        assert!(stmt.contains("não foi erro"));
        assert!(support.contains(&"breakdowns"));
    }

    #[test]
    #[serial]
    fn dnf_por_incidente_e_fim_precoce() {
        pt();
        let mut s = sig();
        s.is_dnf = true;
        s.dnf_mechanical = false;
        assert!(thesis_of(&s).contains("FIM PRECOCE"));
    }

    #[test]
    #[serial]
    fn vitoria_e_a_manchete() {
        pt();
        let mut s = sig();
        s.finish = 1;
        s.positions_gained = 5;
        s.has_fastest_lap = true;
        let stmt = thesis_of(&s);
        assert!(stmt.contains("VITÓRIA"));
        assert!(stmt.contains("volta mais rápida"));
    }

    #[test]
    #[serial]
    fn remontada_quando_ganha_muitas_posicoes() {
        pt();
        let mut s = sig();
        s.grid = 12;
        s.finish = 4;
        s.positions_gained = 8;
        s.assessment = Some(Assessment::Acima);
        assert!(thesis_of(&s).contains("RECUPERAÇÃO"));
    }

    #[test]
    #[serial]
    fn colapso_quando_perde_muitas_posicoes() {
        pt();
        let mut s = sig();
        s.grid = 3;
        s.finish = 11;
        s.positions_gained = -8;
        s.assessment = Some(Assessment::Abaixo);
        assert!(thesis_of(&s).contains("ESCAPOU"));
    }

    #[test]
    #[serial]
    fn acima_e_abaixo_do_esperado_sem_drama() {
        pt();
        let mut over = sig();
        over.finish = 3;
        over.assessment = Some(Assessment::Acima);
        assert!(thesis_of(&over).contains("ACIMA DO ESPERADO"));

        let mut under = sig();
        under.finish = 9;
        under.assessment = Some(Assessment::Abaixo);
        assert!(thesis_of(&under).contains("AQUÉM"));
    }

    #[test]
    #[serial]
    fn duelo_decide_um_dia_morno() {
        pt();
        let mut s = sig(); // assessment Dentro, nada extremo
        s.duel = Some(PostRaceDuel {
            name: "K. Novak".to_string(),
            player_won: true,
            is_nemesis: true,
            h2h: Some((3, 2)),
        });
        let stmt = thesis_of(&s);
        assert!(stmt.contains("O DUELO"));
        assert!(stmt.contains("K. Novak"));
        assert!(stmt.contains("nemesis"));
        assert!(stmt.contains("3-2"));
    }

    #[test]
    #[serial]
    fn dia_de_somar_quando_nada_se_destaca() {
        pt();
        assert!(thesis_of(&sig()).contains("DIA DE SOMAR"));
    }

    #[test]
    #[serial]
    fn telemetry_facts_resume_ritmo_ultrapassagens_e_erro() {
        pt();
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
