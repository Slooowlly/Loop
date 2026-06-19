//! Comandos Tauri do teste do SDK do iRacing (ver [`crate::iracing_sdk`]).

use crate::iracing_sdk::race_control::{self, YellowMacroStatus};
use crate::iracing_sdk::race_monitor::{self, RaceHistory, RaceStatus};
use crate::iracing_sdk::{self, IracingSession, IracingTelemetry};

/// Lê a info de sessão do iRacing em execução e devolve para a UI.
///
/// Retorna `Err(mensagem)` legível quando o sim não está rodando ou a sessão
/// ainda não está pronta — útil para um botão de "testar conexão".
#[tauri::command]
pub fn iracing_read_session() -> Result<IracingSession, String> {
    iracing_sdk::read_session().map_err(|error| error.to_string())
}

/// Lê um snapshot de telemetria ao vivo. Pensado para polling pela UI.
#[tauri::command]
pub fn iracing_read_telemetry() -> Result<IracingTelemetry, String> {
    iracing_sdk::read_telemetry().map_err(|error| error.to_string())
}

/// `custid` (id iRacing) do jogador. Usa o valor capturado automaticamente pelo
/// sampler (persistido); se ainda não houver, tenta ler a sessão atual agora.
#[tauri::command]
pub fn iracing_player_custid() -> Result<i64, String> {
    if let Some(id) = iracing_sdk::cached_custid() {
        return Ok(id);
    }
    let session = iracing_sdk::read_session().map_err(|e| e.to_string())?;
    iracing_sdk::note_session_custid(&session.session_yaml);
    iracing_sdk::cached_custid().ok_or_else(|| {
        "Ainda não capturei seu custid — entre numa sessão/pista do iRacing.".to_string()
    })
}

/// Lê o snapshot do Monitor de Corrida unificado (tentativas + batidas + DNF).
/// O monitor é alimentado por um sampler de fundo a ~60 Hz; este comando só lê.
#[tauri::command]
pub fn iracing_poll_race() -> RaceStatus {
    race_monitor::poll()
}

/// Zera o Monitor de Corrida para começar um novo teste.
#[tauri::command]
pub fn iracing_reset_race() {
    race_monitor::reset();
}

/// Se o iRacing está conectado agora (para a UI ativar sozinha o que precisar).
/// Liga o sampler de fundo se ainda não estiver ligado.
#[tauri::command]
pub fn iracing_connected() -> bool {
    race_monitor::is_connected()
}

/// Lê o histórico volta a volta (race trace + gap ao líder + ritmo do jogador)
/// para o painel pós-corrida.
#[tauri::command]
pub fn iracing_get_race_history() -> RaceHistory {
    race_monitor::get_history()
}

// ─── Salvar / carregar corridas ──────────────────────────────────────────────

/// Resumo de uma corrida salva (para a lista no painel).
#[derive(serde::Serialize)]
pub struct SavedRaceInfo {
    pub name: String,
    pub laps: usize,
    pub track_points: usize,
    pub outcome: String,
    pub size_kb: u64,
    pub modified: String,
}

/// Garante o diretório `app_data_dir/iracing_races` e o devolve.
fn races_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?
        .join("iracing_races");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    Ok(dir)
}

/// Salva o histórico atual da corrida num arquivo JSON e devolve o nome.
#[tauri::command]
pub fn iracing_save_race_history(app: tauri::AppHandle) -> Result<String, String> {
    let history = race_monitor::get_history();
    if history.laps.is_empty() && history.player_laps.is_empty() && history.player_track.is_empty() {
        return Err("Nada para salvar ainda — a corrida não gerou dados.".to_string());
    }
    let dir = races_dir(&app)?;
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let name = format!("corrida_t{}_{}.json", history.attempt_number, stamp);
    let json =
        serde_json::to_string(&history).map_err(|e| format!("Falha ao serializar: {e}"))?;
    std::fs::write(dir.join(&name), json).map_err(|e| format!("Falha ao gravar: {e}"))?;
    Ok(name)
}

/// Lista as corridas salvas (mais recentes primeiro).
#[tauri::command]
pub fn iracing_list_saved_races(app: tauri::AppHandle) -> Result<Vec<SavedRaceInfo>, String> {
    let dir = races_dir(&app)?;
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| format!("Falha ao ler pasta: {e}"))? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let meta = entry.metadata().ok();
        let size_kb = meta.as_ref().map(|m| m.len() / 1024).unwrap_or(0);
        let modified = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let dt: chrono::DateTime<chrono::Local> = t.into();
                dt.format("%d/%m %H:%M").to_string()
            })
            .unwrap_or_default();
        let (laps, track_points, outcome) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<RaceHistory>(&s).ok())
            .map(|h| (h.laps.len(), h.player_track.len(), h.outcome))
            .unwrap_or((0, 0, String::new()));
        out.push(SavedRaceInfo {
            name,
            laps,
            track_points,
            outcome,
            size_kb,
            modified,
        });
    }
    out.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(out)
}

/// Carrega uma corrida salva pelo nome do arquivo.
#[tauri::command]
pub fn iracing_load_saved_race(app: tauri::AppHandle, name: String) -> Result<RaceHistory, String> {
    let dir = races_dir(&app)?;
    // Evita path traversal: usa só o nome do arquivo.
    let file = std::path::Path::new(&name)
        .file_name()
        .ok_or("Nome de arquivo inválido")?;
    let path = dir.join(file);
    let s = std::fs::read_to_string(&path).map_err(|e| format!("Falha ao ler: {e}"))?;
    serde_json::from_str(&s).map_err(|e| format!("Falha ao interpretar: {e}"))
}

// ─── Geração de AI roster (carreira → iRacing) ───────────────────────────────

/// Resultado da geração de roster (para a UI).
#[derive(serde::Serialize)]
pub struct RosterGenResult {
    pub path: String,
    pub drivers: usize,
}

/// Caminho do mapa de números fixos de uma carreira.
fn numbers_path(base_dir: &std::path::Path, career_id: &str) -> std::path::PathBuf {
    base_dir
        .join("iracing_numbers")
        .join(format!("{career_id}.json"))
}

/// Garante um número FIXO por piloto na temporada: carrega o mapa salvo, atribui
/// o menor número livre (1..) aos pilotos novos e persiste. Números vinculados ao
/// piloto não mudam entre as rodadas.
fn ensure_driver_numbers(
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
        let json =
            serde_json::to_string_pretty(&map).map_err(|e| format!("Falha ao serializar números: {e}"))?;
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
) -> Result<RosterGenResult, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::{contracts as cq, drivers as dq, teams as tq};
    use crate::iracing_sdk::{paths, roster_gen};
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
        .and_then(|p| cq::get_active_contract_for_pilot(&db.conn, &p.id).ok().flatten())
        .map(|c| c.equipe_id);

    // Grid da categoria (exclui o jogador — ele dirige, não é IA).
    let drivers = dq::get_drivers_by_category(&db.conn, &categoria)
        .map_err(|e| format!("Falha ao ler pilotos: {e}"))?;
    let mut entries = Vec::new();
    for driver in drivers {
        if driver.is_jogador {
            continue;
        }
        let team_info = cq::get_active_contract_for_pilot(&db.conn, &driver.id)
            .ok()
            .flatten()
            .and_then(|contract| {
                tq::get_team_by_id(&db.conn, &contract.equipe_id)
                    .ok()
                    .flatten()
            })
            .map(|team| roster_gen::TeamInfo {
                is_player_team: player_team_id.as_deref() == Some(team.id.as_str()),
                team_id: team.id,
                color: team.cor_primaria,
                color2: team.cor_secundaria,
                pit_crew: team.pit_crew_quality,
                strategy: team.pit_strategy_risk,
            });
        entries.push((driver, team_info));
    }
    if entries.is_empty() {
        return Err(format!("Nenhum piloto de IA na categoria '{categoria}'."));
    }

    // Números FIXOS por piloto na temporada: carrega o mapa salvo, atribui os
    // que faltam (menor número livre) e persiste.
    let driver_ids: Vec<String> = entries.iter().map(|(d, _)| d.id.clone()).collect();
    let numbers = ensure_driver_numbers(&base_dir, &career_id, &driver_ids)?;

    let roster =
        roster_gen::build_roster(&entries, &car, &numbers, || uuid::Uuid::new_v4().to_string());

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

    Ok(RosterGenResult {
        path: path.display().to_string(),
        drivers: roster.drivers.len(),
    })
}

/// Resultado da geração de AI season.
#[derive(serde::Serialize)]
pub struct SeasonGenResult {
    pub path: String,
    pub name: String,
    pub events: usize,
}

/// Gera a **AI season** (calendário) da categoria, espelhando o exemplo do
/// usuário: lê o calendário da carreira (track_ids já são do iRacing), filtra
/// pistas grátis, usa a duração da categoria e o clima do calendário. Aponta para
/// o roster `roster_name`. Sai em `aiseasons/<série> - <ano>.json`.
#[tauri::command]
pub fn iracing_generate_season(
    app: tauri::AppHandle,
    career_id: String,
    categoria: String,
    roster_name: String,
    car_key: String,
) -> Result<SeasonGenResult, String> {
    use crate::config::app_config::AppConfig;
    use crate::constants::categories::get_category_config;
    use crate::constants::tracks::get_track;
    use crate::db::connection::Database;
    use crate::db::queries::{drivers as dq, seasons as sq};
    use crate::db::queries::calendar as calq;
    use crate::iracing_sdk::{paths, roster_gen, season_gen};
    use crate::models::enums::WeatherCondition;
    use tauri::Manager;

    let car = roster_gen::car_spec(&car_key)
        .ok_or_else(|| format!("Carro desconhecido: {car_key}"))?;
    let cat = get_category_config(&categoria)
        .ok_or_else(|| format!("Categoria desconhecida: {categoria}"))?;

    // Abre o banco e pega a temporada ativa.
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
    let season = sq::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao ler temporada: {e}"))?
        .ok_or("Nenhuma temporada ativa nesta carreira.")?;

    // Calendário da categoria → eventos (filtra pistas pagas; track_id já é do iRacing).
    let mut entries = calq::get_calendar(&db.conn, &season.id, &categoria)
        .map_err(|e| format!("Falha ao ler calendário: {e}"))?;
    entries.sort_by_key(|e| e.rodada);

    let mut events = Vec::new();
    // Clima do calendário → bloco weather DINÂMICO (timeline) de cada etapa.
    // Escala real do iRacing:
    //   skies:       0 Limpo · 1 Parcialmente · 2 Predominantemente · 3 Encoberto
    //   track_water: 0 Nenhum … 5 Muito intenso
    //   keyframes event_type: 0 Limpo … 7 Chuva · 8 Chuva intensa
    // Por ora a timeline só "segura" a condição da etapa (a carreira ainda não
    // modela evolução); a ESTRUTURA dinâmica fica provada e pronta para evoluir.
    let custid = iracing_sdk::cached_custid().unwrap_or(0);
    let race_end = cat.duracao_corrida_min as i64;
    let clima_to_weather = |clima: WeatherCondition, temp: f64| {
        let (skies, humidity, track_water, kf): (_, _, _, Vec<(i64, i64)>) = match clima {
            WeatherCondition::Dry => (1, 45, 0, vec![(0, -120), (1, 0), (1, race_end)]),
            WeatherCondition::Damp => (2, 70, 2, vec![(1, -120), (2, 0), (2, race_end)]),
            WeatherCondition::Wet => (3, 88, 4, vec![(3, -120), (7, 0), (7, race_end)]),
            WeatherCondition::HeavyRain => (3, 97, 5, vec![(3, -120), (8, 0), (8, race_end)]),
        };
        season_gen::EventWeather {
            skies,
            humidity,
            temp_c: temp.round() as i64,
            track_water,
            keyframes: kf
                .into_iter()
                .map(|(event_type, time_offset)| season_gen::WeatherKeyframe {
                    event_type,
                    time_offset,
                })
                .collect(),
            weather_id: format!("{custid}_{}", uuid::Uuid::new_v4()),
        }
    };

    let mut skipped_paid = 0;
    for entry in &entries {
        match get_track(entry.track_id) {
            Some(track) if track.gratuita => {
                events.push(season_gen::EventInput {
                    track_id: entry.track_id as i64,
                    // Nenhuma pista free é oval de verdade — Roval (Charlotte) é
                    // ROAD no iRacing (paceCar road, sem largada lançada).
                    is_oval: false,
                    event_id: uuid::Uuid::new_v4().to_string(),
                    weather: clima_to_weather(entry.clima, entry.temperatura),
                });
            }
            _ => skipped_paid += 1, // paga ou desconhecida → fora (ex.: Laguna)
        }
    }
    if events.is_empty() {
        return Err(format!(
            "Nenhuma pista grátis no calendário da categoria '{categoria}' ({skipped_paid} pagas/ignoradas)."
        ));
    }

    // Faixa de skill (básico): min/max dos pilotos de IA da categoria.
    let ai: Vec<f64> = dq::get_drivers_by_category(&db.conn, &categoria)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| !d.is_jogador)
        .map(|d| d.atributos.skill)
        .collect();
    let min_skill = ai.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_skill = ai.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let (min_skill, max_skill) = if ai.is_empty() {
        (25, 50)
    } else {
        (min_skill.round() as i64, max_skill.round() as i64)
    };
    let max_drivers = (ai.len() as i64 + 1).max(2);

    // Clima global (fallback) = o da 1ª etapa do calendário.
    let global_weather = entries
        .first()
        .map(|e| clima_to_weather(e.clima, e.temperatura))
        .unwrap_or_else(|| clima_to_weather(WeatherCondition::Dry, 26.0));

    let name = format!("{} - {}", cat.nome_curto, season.ano);
    let params = season_gen::SeasonParams {
        roster_name: roster_name.trim().to_string(),
        name: name.clone(),
        car_id: car.car_id,
        car_class_id: car.car_class_id,
        race_length_min: cat.duracao_corrida_min as i64,
        max_drivers,
        min_skill,
        max_skill,
        year: season.ano,
        global_weather,
        events,
    };
    let season_json = season_gen::build_season(&params);

    // Grava em aiseasons/<nome>.json.
    let safe_name: String = name
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    let dir = paths::aiseasons_dir()
        .ok_or("Não foi possível localizar a pasta aiseasons do iRacing.")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    let path = dir.join(format!("{}.json", safe_name.trim()));
    let json = serde_json::to_string_pretty(&season_json)
        .map_err(|e| format!("Falha ao serializar: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Falha ao gravar: {e}"))?;

    Ok(SeasonGenResult {
        path: path.display().to_string(),
        name,
        events: params_events_len(&season_json),
    })
}

/// Conta os eventos no JSON gerado (para a UI).
fn params_events_len(v: &serde_json::Value) -> usize {
    v["events"].as_array().map(|a| a.len()).unwrap_or(0)
}

/// Pintura que o JOGADOR deve aplicar na garagem do iRacing para ficar na cor do
/// time (igual à IA). A pintura embutida do jogador é account-side, então só dá
/// para MOSTRAR o esquema certo — o usuário aplica uma vez.
#[derive(serde::Serialize)]
pub struct PlayerPaint {
    pub team_name: String,
    pub pattern: String,
    pub color1: String,
    pub color2: String,
    pub color3: String,
    pub spec: String,
}

/// Lê o time do jogador na carreira e devolve o esquema de pintura a aplicar.
#[tauri::command]
pub fn iracing_player_paint(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<PlayerPaint, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::{contracts as cq, drivers as dq, teams as tq};
    use crate::iracing_sdk::roster_gen;
    use tauri::Manager;

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

    let player = dq::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let team = cq::get_active_contract_for_pilot(&db.conn, &player.id)
        .ok()
        .flatten()
        .and_then(|contract| tq::get_team_by_id(&db.conn, &contract.equipe_id).ok().flatten())
        .ok_or("Você não tem contrato/time ativo nesta carreira.")?;

    let hex = roster_gen::normalize_hex(&team.cor_primaria);
    Ok(PlayerPaint {
        team_name: team.nome,
        pattern: roster_gen::DESIGN_PATTERN.to_string(),
        color1: format!("#{hex}"),
        color2: format!("#{}", roster_gen::DESIGN_COLOR2),
        color3: format!("#{}", roster_gen::DESIGN_COLOR3),
        spec: format!(
            "{},{hex},{},{}",
            roster_gen::DESIGN_PATTERN,
            roster_gen::DESIGN_COLOR2,
            roster_gen::DESIGN_COLOR3
        ),
    })
}

/// Resultado da pintura automática do carro do jogador.
#[derive(serde::Serialize)]
pub struct ApplyPaintResult {
    pub path: String,
    pub custid: i64,
    pub color: String,
}

/// Escreve a pintura (cor sólida do time) do carro do jogador como custom paint
/// do iRacing: `paint/<carro>/car_<custid>.tga`. Usa o custid já capturado.
#[tauri::command]
pub fn iracing_apply_player_paint(
    app: tauri::AppHandle,
    career_id: String,
    car_key: String,
) -> Result<ApplyPaintResult, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::{contracts as cq, drivers as dq, teams as tq};
    use crate::iracing_sdk::{paint_gen, paths, roster_gen};
    use tauri::Manager;

    let custid = iracing_sdk::cached_custid().ok_or(
        "Ainda não capturei seu custid — abra o iRacing e entre numa sessão uma vez.",
    )?;
    let car = roster_gen::car_spec(&car_key)
        .ok_or_else(|| format!("Carro desconhecido: {car_key}"))?;

    // Cor do time do jogador.
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
    let player =
        dq::get_player_driver(&db.conn).map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let team = cq::get_active_contract_for_pilot(&db.conn, &player.id)
        .ok()
        .flatten()
        .and_then(|c| tq::get_team_by_id(&db.conn, &c.equipe_id).ok().flatten())
        .ok_or("Você não tem contrato/time ativo nesta carreira.")?;
    let hex = roster_gen::normalize_hex(&team.cor_primaria);

    // Escreve car_<custid>.tga na pasta de pintura do carro.
    let dir = paths::car_paint_dir(car.car_path)
        .ok_or("Não foi possível localizar a pasta de pintura do iRacing.")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    let path = dir.join(format!("car_{custid}.tga"));
    paint_gen::write_solid_tga(&path, &hex).map_err(|e| format!("Falha ao gravar pintura: {e}"))?;

    Ok(ApplyPaintResult {
        path: path.display().to_string(),
        custid,
        color: format!("#{hex}"),
    })
}

// ─── Race Control: macro de bandeira amarela ─────────────────────────────────

/// Estado da macro de bandeira (app.ini achado, instalada, slot, original).
#[tauri::command]
pub fn iracing_yellow_macro_status() -> YellowMacroStatus {
    race_control::status()
}

/// Instala a macro `!y$` no slot "You're welcome" (com backup).
#[tauri::command]
pub fn iracing_install_yellow_macro() -> Result<YellowMacroStatus, String> {
    race_control::install()
}

/// Restaura o valor original da macro.
#[tauri::command]
pub fn iracing_restore_yellow_macro() -> Result<YellowMacroStatus, String> {
    race_control::restore()
}

/// Dispara a macro instalada (aciona a bandeira no iRacing).
#[tauri::command]
pub fn iracing_throw_yellow() -> Result<(), String> {
    race_control::throw_yellow()
}

/// Liga/desliga o envio AUTOMÁTICO de bandeira pelo RaceControl.
#[tauri::command]
pub fn iracing_set_auto_yellow(enabled: bool) {
    race_monitor::set_auto_yellow(enabled);
}

/// Estado do envio automático de bandeira.
#[tauri::command]
pub fn iracing_auto_yellow_enabled() -> bool {
    race_monitor::auto_yellow_enabled()
}

/// Dispara um macro de chat por número (teste cru — descobrir o slot certo).
#[tauri::command]
pub fn iracing_send_chat_macro(macro_num: i32) -> Result<(), String> {
    iracing_sdk::send_chat_macro(macro_num).map_err(|e| e.to_string())
}
