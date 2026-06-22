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
    if history.laps.is_empty() && history.player_laps.is_empty() && history.player_track.is_empty()
    {
        return Err("Nada para salvar ainda — a corrida não gerou dados.".to_string());
    }
    let dir = races_dir(&app)?;
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let name = format!("corrida_t{}_{}.json", history.attempt_number, stamp);
    let json = serde_json::to_string(&history).map_err(|e| format!("Falha ao serializar: {e}"))?;
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

/// Perfil ADAPTATIVO de dificuldade do JOGADOR (não do save): nível geral + por
/// pista, vs a baseline universal (os offsets por pista). Guardado por `custid`
/// (conta do iRacing) em `app_data/iracing_adaptive/<custid>.json`, então uma
/// carreira NOVA já nasce calibrada ao jogador — ele não recalibra do zero.
#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct AdaptiveProfile {
    /// Delta global: nível geral do jogador vs a baseline.
    pub global: i64,
    /// Delta por pista (`track_id` como string → delta): aptidão por circuito.
    pub tracks: std::collections::HashMap<String, i64>,
}

impl AdaptiveProfile {
    /// Delta acumulado do jogador para uma pista (0 se nunca correu nela).
    pub fn track_delta(&self, track_id: i64) -> i64 {
        self.tracks.get(&track_id.to_string()).copied().unwrap_or(0)
    }
}

fn adaptive_profile_path(base_dir: &std::path::Path, custid: i64) -> std::path::PathBuf {
    base_dir
        .join("iracing_adaptive")
        .join(format!("{custid}.json"))
}

/// Carrega o perfil adaptativo do jogador (vazio se ainda não existe).
fn load_adaptive_profile(base_dir: &std::path::Path, custid: i64) -> AdaptiveProfile {
    std::fs::read_to_string(adaptive_profile_path(base_dir, custid))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Persiste o perfil adaptativo do jogador.
fn save_adaptive_profile(
    base_dir: &std::path::Path,
    custid: i64,
    profile: &AdaptiveProfile,
) -> Result<(), String> {
    let path = adaptive_profile_path(base_dir, custid);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    }
    let json =
        serde_json::to_string_pretty(profile).map_err(|e| format!("Falha ao serializar: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Falha ao gravar: {e}"))
}

/// Resultado do processamento adaptativo pós-corrida (para a UI).
#[derive(serde::Serialize)]
pub struct AdaptiveResult {
    /// Se a corrida foi válida para adaptar (false = DNF/dados insuficientes).
    pub applied: bool,
    /// Explicação legível ("Dominou → sobe", "Trânsito → mantém", etc.).
    pub verdict: String,
    pub d_global: i64,
    pub d_track: i64,
    /// Deltas resultantes do jogador (já com piso/teto).
    pub global: i64,
    pub track: i64,
    pub track_id: i64,
    pub track_name: Option<String>,
}

/// Processa o resultado da ÚLTIMA corrida e atualiza o perfil adaptativo do
/// jogador (por `custid`). Chamado pelo frontend quando detecta a corrida
/// encerrada (opção a — automático). Só aplica em corrida limpa do jogador.
#[tauri::command]
pub fn iracing_process_race_result(app: tauri::AppHandle) -> Result<AdaptiveResult, String> {
    use crate::constants::tracks::get_track;
    use crate::iracing_sdk::{adaptive, race_monitor};
    use tauri::Manager;

    let history = race_monitor::get_history();
    if !history.finished {
        return Err("A corrida ainda não encerrou.".to_string());
    }
    let track_id = history.track_id;
    if track_id <= 0 {
        return Err("Pista da corrida não identificada (sem TrackID na sessão).".to_string());
    }
    let custid = iracing_sdk::cached_custid().unwrap_or(0);
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let mut profile = load_adaptive_profile(&base_dir, custid);

    let result = race_monitor::build_adaptive_result(&history, track_id);
    let current = adaptive::Deltas {
        global: profile.global,
        track: profile.track_delta(track_id),
    };
    let update = adaptive::compute_adaptive_update(&result, &current);
    if update.applied {
        profile.global = update.new.global;
        profile
            .tracks
            .insert(track_id.to_string(), update.new.track);
        save_adaptive_profile(&base_dir, custid, &profile)?;
    }
    Ok(AdaptiveResult {
        applied: update.applied,
        verdict: update.verdict,
        d_global: update.d_global,
        d_track: update.d_track,
        global: profile.global,
        track: profile.track_delta(track_id),
        track_id,
        track_name: get_track(track_id as u32).map(|t| t.nome.to_string()),
    })
}

/// Uma entrada do resultado de corrida já mapeado para a carreira (Fase 3).
#[derive(serde::Serialize)]
pub struct CareerRaceEntry {
    /// Nosso `driver_id` (None se o número não casou — ex.: carro do jogador).
    pub driver_id: Option<String>,
    /// Nome do nosso piloto (do banco).
    pub name: Option<String>,
    pub car_number: i32,
    pub class_position: i32,
    pub is_player: bool,
}

/// Resultado da última corrida, mapeado de volta para os nossos pilotos.
#[derive(serde::Serialize)]
pub struct CareerRaceResult {
    pub track_id: i64,
    pub track_name: Option<String>,
    pub finished: bool,
    pub entries: Vec<CareerRaceEntry>,
}

/// Casa o resultado da última corrida (números de carro da IA) de volta com os
/// nossos pilotos (Fase 3). Como NÓS geramos o roster, o número do carro → nosso
/// `driver_id` via o mapa de números fixos salvo na geração do roster.
#[tauri::command]
pub fn iracing_career_race_result(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<CareerRaceResult, String> {
    use crate::config::app_config::AppConfig;
    use crate::constants::tracks::get_track;
    use crate::db::connection::Database;
    use crate::db::queries::drivers as dq;
    use crate::iracing_sdk::race_monitor;
    use tauri::Manager;

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    // Mapa de números: driver_id → número (salvo na geração do roster) → reverte.
    let numbers: std::collections::HashMap<String, i64> =
        std::fs::read_to_string(numbers_path(&base_dir, &career_id))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
    let by_number: std::collections::HashMap<i64, String> =
        numbers.into_iter().map(|(id, n)| (n, id)).collect();

    // Banco da carreira (para os nomes).
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let history = race_monitor::get_history();
    let player_idx = history.player_car_idx;

    // O jogador é excluído do roster da IA, então o número dele não está no mapa —
    // resolvemos pelo piloto-jogador da carreira (is_jogador).
    let player_driver = dq::get_player_driver(&db.conn).ok();

    let mut entries: Vec<CareerRaceEntry> = history
        .cars_meta
        .iter()
        .filter(|m| !m.is_pace)
        .map(|m| {
            let is_player = m.idx == player_idx;
            let (driver_id, name) = if is_player {
                match &player_driver {
                    Some(d) => (Some(d.id.clone()), Some(d.nome.clone())),
                    None => (None, None),
                }
            } else {
                let did = by_number.get(&(m.car_number as i64)).cloned();
                let nm = did
                    .as_deref()
                    .and_then(|id| dq::get_driver(&db.conn, id).ok())
                    .map(|d| d.nome);
                (did, nm)
            };
            CareerRaceEntry {
                driver_id,
                name,
                car_number: m.car_number,
                class_position: m.class_position,
                is_player,
            }
        })
        .collect();
    // Ordena pela posição na classe (não classificados ao fim).
    entries.sort_by_key(|e| {
        if e.class_position >= 1 {
            e.class_position
        } else {
            i32::MAX
        }
    });

    Ok(CareerRaceResult {
        track_id: history.track_id,
        track_name: get_track(history.track_id as u32).map(|t| t.nome.to_string()),
        finished: history.finished,
        entries,
    })
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
                honeymoon: contract
                    .as_ref()
                    .map(|c| c.temporada_inicio == season_num)
                    .unwrap_or(false),
                category_move,
                team_morale: team.as_ref().map(|t| t.morale).unwrap_or(1.0),
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
            if inj.active || inj.season != season.numero as i32 {
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
        // Vingança / azar acumulado: DNFs por fonte nas últimas 3 rodadas.
        let mut last_crashout: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut bad_luck: HashMap<String, u32> = HashMap::new();
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
                    // azar = DNF que não foi erro do piloto.
                    if source.as_deref() != Some("DriverError") {
                        *bad_luck.entry(pid.clone()).or_default() += 1;
                    }
                    // vingança = tirado de corrida (PostCollision) na ÚLTIMA corrida.
                    if back == 1 && source.as_deref() == Some("PostCollision") {
                        last_crashout.insert(pid);
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
            ctx.track_crash = track_crash_set.contains(id);
        }
    }

    let behavior_ctx = next.as_ref().and_then(|(season, race)| {
        let track = get_track(race.track_id)?;
        // Clima da próxima corrida (mesma geração determinística da season).
        let story = weather::generate_weather(
            month_from_week(race.week_of_year),
            track_hemisphere(track.pais),
            climate_tendency(track.rain_group),
            event_seed(&career_id, &race.id),
            false,
        );
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
        Some(roster_gen::BehaviorContext {
            current_season: season.numero as i32,
            track_id: race.track_id,
            track_length_km: track.comprimento_km,
            track_flag: track.pais.to_string(),
            title_points: title_points.clone(),
            races_left: (total - race.rodada + 1).max(1) as u32,
            season_length: total.max(1) as u32,
            max_points: (get_points_for_position(1, categoria == "endurance") + BONUS_FASTEST_LAP)
                as f64,
            field_size: title_points.len().max(1) as u32,
            grid_skills: grid_skills.clone(),
            is_wet: story.is_wet_race,
            rain_intensity,
            temp_c: race.temperatura,
            seed_base: event_seed(&career_id, &race.id),
            recent_positions,
            global_percentile,
            driver_ctx,
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
    /// Pista mirada para a margem de skill (nome). Auto = próxima corrida do
    /// calendário; ou a do override manual.
    pub targeted_track: Option<String>,
    /// Skill efetivo da ponta da IA (maxSkill aplicado) para a pista mirada.
    pub ai_skill: i64,
    /// true se a pista veio do calendário (próxima corrida); false se foi override.
    pub auto_targeted: bool,
}

/// Ponto de dificuldade (SWEET SPOT) base por tier — NÃO é um teto rígido. É o nível
/// efetivo que a ponta da IA deve correr pra dar a dificuldade IDEAL daquele tier, na
/// pista de referência. A ponta fica competitiva mas BATÍVEL, deixando margem pro
/// jogador (decisão do user). Sweet spot final da pista = este base + offset da pista.
/// Acima de rookie é progressivamente mais difícil (sweet spot maior).
fn tier_difficulty_base(tier: u8) -> i64 {
    match tier {
        0 => 73, // Rookie (validado: Rudskogen 73; Lédenon vira 82 com offset +9)
        1 => 80, // Amador      (provisório — calibrar)
        2 => 86, // Pro/Especial (provisório)
        3 => 91, // Super Pro/GT4 (provisório)
        4 => 95, // GT3          (provisório)
        _ => 98, // Elite/LMP2   (provisório)
    }
}

/// Offset de skill por PISTA (a "margem por pista"). A IA rende diferente em cada
/// circuito para o mesmo skill% (no Rudskogen efetivo 73 → 1:36.15; no Lédenon o
/// mesmo 73 → 1:36.95, ~0,8s mais lenta). Então cada pista soma/subtrai do sweet spot
/// base do tier pra acertar a dificuldade ideal. Calibrado em corrida real; default 0.
/// No fluxo de reexportar antes de cada corrida, recebe a pista daquela corrida.
/// VALORES (rookie sweet spot = 73 + offset) — preencher conforme o user for testando:
fn track_skill_offset(track_id: i64) -> i64 {
    match track_id {
        // Lédenon: no sweet spot 83 a ponta (Alvarez 1:35.946) EMPATOU com o jogador
        // 1500iR (1:35.941). Pra rookie, recuamos 1 ponto → sweet spot 82 (offset +9),
        // deixando uma pitada de margem pro jogador.
        489 => 9,
        // Navarra: sweet spot 81. Calibrado no 515 (Speed Circuit 3,9 km) por ritmo de
        // corrida — a pista "engole" a IA (teto de ~1:58.8 em ar limpo, best-lap engana
        // por tráfego). 516 (Medium 3,4 km) usa o mesmo valor: traçado quase idêntico,
        // não vale testar à parte (decisão do user).
        515 | 516 => 8,
        // Lime Rock Park - Grand Prix (353, ~1:00 a volta, pista curta): sweet spot 81.
        353 => 8,
        // Lime Rock Park - Classic (352) + Chicanes (354): mesmo venue. User mandou herdar
        // o valor do Lime Rock (sweet spot 81) sem teste à parte.
        352 | 354 => 8,
        // Motorsport Arena Oschersleben (449 GP / 454 Alt / 455 B Course): sweet spot 82.
        // Aplicado nos 3 layouts do venue. Obs.: o B Course é mais curto na vida real —
        // se sentir diferente, a gente separa o 455 depois.
        449 | 454 | 455 => 9,
        // Okayama (166 full 3,7 km / 167 Short 2,4 km): sweet spot 80. User mandou o
        // mesmo valor nos dois layouts livres. (542 Short 1,7 km é pago, fora.)
        166 | 167 => 7,
        // Oran Park Raceway (202 GP 2,6 km / 208 South 2,0 km): sweet spot 74 — quase
        // baseline, IA já competitiva com pouco skill (igual Rudskogen). Mesmo valor nos 2.
        202 | 208 => 1,
        // Oulton Park - International (180, 4.4 km) + variações da Intl: 183 w/out Hislop,
        // 184 w/out Brittens, 185 w/no Chicanes. Sweet spot 79 nos 4 layouts livres da
        // família Intl. (342 é a Intl paga, fora; Fosters/Island não são variação da Intl.)
        180 | 183 | 184 | 185 => 6,
        // Oulton Park - Fosters (181), Island (182), Fosters w/Hislop (186): layouts não-Intl
        // do mesmo venue. User mandou herdar o valor do Oulton (sweet spot 79) sem teste à parte.
        181 | 182 | 186 => 6,
        // Snetterton Circuit - 300 (297, 4.8 km) + 200 (298, 3.2 km): sweet spot 82.
        // User mandou o mesmo valor nos dois layouts livres.
        297 | 298 => 9,
        // Summit Point - Summit Point Raceway (9, 3.2 km): sweet spot 97 (offset +24).
        // Pista com "macetes" que humanos usam e a IA não pega — mesmo em 95% a IA fica
        // fora do ritmo esperado. Folga OK: o teto real do iRacing é 125%, não 100%.
        // (Jefferson 8 é layout diferente/curto, fora dos pools — não coberto aqui.)
        9 => 24,
        // Tsukuba Circuit - 2000 Full (324, 2.0 km): sweet spot 82. Único layout livre.
        324 => 9,
        // Winton Motor Raceway - National (439, 3.0 km) + Club (440, 2.0 km): sweet spot 80.
        // Mesmo valor nos dois layouts livres do venue.
        439 | 440 => 7,
        // Charlotte Motor Speedway - Roval (554, 3.7 km, versão 2025): sweet spot 86.
        554 => 13,
        // Virginia Int'l Raceway - Full Course (465, 5.3 km) + Grand Course (466, 6.8 km):
        // sweet spot 106 (offset +33). Passa de 100 — só funciona com o clamp(0,125).
        465 | 466 => 33,
        // VIR - North Course (467, 3.6 km): sweet spot 100 (offset +27). (Patriot 259 = pago.)
        467 => 27,
        // Rudskogen Motorsenter (451): pista BASELINE — rookie sweet spot 73 (offset 0),
        // validado em corrida real. Explícito só pra documentar (mesmo valor do default).
        451 => 0,
        _ => 0, // demais: baseline default até calibrar a pista
    }
}

/// Gera a **AI season** (calendário) da categoria, espelhando o exemplo do
/// usuário: lê o calendário da carreira (track_ids já são do iRacing), filtra
/// pistas grátis, usa a duração da categoria e o clima do calendário. Aponta para
/// o roster `roster_name`. Sai em `aiseasons/<série> - <ano>.json`.
/// `target_track_id` (opcional) = a pista da corrida que vai ser disputada: aplica
/// a margem por pista no teto de skill. None → só o teto do tier (sem offset).
#[tauri::command]
pub fn iracing_generate_season(
    app: tauri::AppHandle,
    career_id: String,
    categoria: String,
    roster_name: String,
    car_key: String,
    target_track_id: Option<i64>,
    // Modo TESTE: aiseason "zerado" (sem resultados) com a corrida 1 usando o clima
    // roteirizado da 1ª corrida — pra visualizar o roteiro no menu do iRacing.
    test_blank: Option<bool>,
) -> Result<SeasonGenResult, String> {
    use crate::config::app_config::AppConfig;
    use crate::constants::categories::get_category_config;
    use crate::constants::tracks::get_track;
    use crate::db::connection::Database;
    use crate::db::queries::calendar as calq;
    use crate::db::queries::{drivers as dq, race_history as rhq, seasons as sq};
    use crate::iracing_sdk::{paths, results_gen, roster_gen, season_gen};
    use tauri::Manager;

    let car =
        roster_gen::car_spec(&car_key).ok_or_else(|| format!("Carro desconhecido: {car_key}"))?;
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

    // Números fixos por piloto IA (mesmos do roster) — usados nos resultados das etapas.
    let ai_driver_ids: Vec<String> = dq::get_drivers_by_category(&db.conn, &categoria)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| !d.is_jogador)
        .map(|d| d.id)
        .collect();
    let numbers = ensure_driver_numbers(&base_dir, &career_id, &ai_driver_ids).unwrap_or_default();

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
    // 1ª corrida do save = nenhuma etapa concluída ainda (roteiro especial do clima).
    let career_first_race = entries
        .iter()
        .all(|e| !matches!(e.status, crate::models::enums::RaceStatus::Concluida));
    let first_week = entries
        .iter()
        .map(|e| e.week_of_year)
        .min()
        .unwrap_or(i32::MAX);
    // Modo TESTE (zerado): força o roteiro da 1ª corrida na etapa 1 e omite resultados.
    let test_blank = test_blank.unwrap_or(false);
    let first_race_id = entries
        .iter()
        .min_by_key(|e| e.rodada)
        .map(|e| e.id.clone());

    // Clima + horário gerados por pista+estação (determinístico por etapa). Guarda a
    // história de cada pista para a penalidade da chuva na banda.
    let mut stories: std::collections::HashMap<i64, crate::iracing_sdk::weather::WeatherStory> =
        std::collections::HashMap::new();
    let mut skipped_paid = 0;
    for entry in &entries {
        match get_track(entry.track_id) {
            Some(track) if track.gratuita => {
                let is_first = (career_first_race && entry.week_of_year == first_week)
                    || (test_blank && first_race_id.as_deref() == Some(entry.id.as_str()));
                let seed = event_seed(&career_id, &entry.id);
                let (ew, story) = build_event_weather(
                    track,
                    entry.week_of_year,
                    entry.temperatura,
                    season.ano,
                    cat.tier,
                    custid,
                    seed,
                    is_first,
                    race_end,
                );
                stories.insert(entry.track_id as i64, story);
                // Etapa já disputada no app → escreve os resultados (iRacing "pula").
                // No modo teste (zerado) nunca escreve resultados.
                let results = if !test_blank
                    && matches!(entry.status, crate::models::enums::RaceStatus::Concluida)
                {
                    rhq::get_event_results(&db.conn, &entry.id)
                        .ok()
                        .filter(|r| !r.is_empty())
                        .map(|rows| {
                            let drivers: Vec<results_gen::ResultDriver> = rows
                                .into_iter()
                                .map(|r| {
                                    let num = numbers.get(&r.piloto_id).copied().unwrap_or(0);
                                    results_gen::ResultDriver {
                                        finish: r.finish,
                                        start: r.start,
                                        laps: r.laps,
                                        total_ms: r.total_ms,
                                        gap_ms: r.gap_ms,
                                        incidents: r.incidents,
                                        dnf: r.dnf,
                                        dnf_reason: r.dnf_reason,
                                        has_fastest: r.has_fastest,
                                        car_number: if r.is_jogador {
                                            "0".to_string()
                                        } else {
                                            num.to_string()
                                        },
                                        cust_id: if r.is_jogador { custid } else { 990_000 + num },
                                        name: r.nome,
                                        car_id: car.car_id,
                                        car_class_id: car.car_class_id,
                                    }
                                })
                                .collect();
                            results_gen::build_results(&drivers)
                        })
                } else {
                    None
                };
                events.push(season_gen::EventInput {
                    track_id: entry.track_id as i64,
                    // Nenhuma pista free é oval de verdade — Roval (Charlotte) é
                    // ROAD no iRacing (paceCar road, sem largada lançada).
                    is_oval: false,
                    event_id: uuid::Uuid::new_v4().to_string(),
                    weather: ew,
                    results,
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

    // Faixa de skill — RÉGUA ASSIMÉTRICA por tier. O iRacing ESTICA a ordem do grid
    // para preencher [minSkill, maxSkill]:
    //   ajustada = minSkill + (skill - menor_do_grid)/(maior - menor) * (maxSkill - minSkill)
    // O melhor do grid sempre vira maxSkill; o pior, minSkill. Usamos isso a favor:
    //   - maxSkill = teto do TIER → o melhor piloto corre nesse nível efetivo. Validado
    //     em pista (corrida real 1500iR em Rudskogen): skill ~73 ≈ pace 1500iR
    //     (competitivo médio). Tier 0 (rookie) = 73 → a FRENTE já é disputada, mesmo
    //     sendo rookie ("vieram do kart"). Tiers acima sobem progressivamente.
    //   - minSkill = o pior piloto REAL do grid → o lanterna continua genuinamente ruim.
    // Resultado: frente puxada pro competitivo, fundo ancorado no ruim de verdade.
    let skills: Vec<f64> = dq::get_drivers_by_category(&db.conn, &categoria)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| !d.is_jogador)
        .map(|d| d.atributos.skill)
        .collect();
    // Pista alvo da margem por pista: override manual (target_track_id, p/ testes) OU,
    // se ausente, a PRÓXIMA corrida pendente do calendário (AUTO-TARGETING). Assim,
    // reexportando antes de cada corrida, a banda sempre reflete a pista que vem.
    let auto_track = if target_track_id.is_none() {
        calq::get_next_race(&db.conn, &season.id, &categoria)
            .ok()
            .flatten()
            .map(|r| r.track_id as i64)
    } else {
        None
    };
    let auto_targeted = auto_track.is_some();
    let resolved_track_id = target_track_id.or(auto_track);
    let targeted_track = resolved_track_id
        .and_then(|id| get_track(id as u32))
        .map(|t| t.nome.to_string());
    // Sweet spot de dificuldade = base do tier + offset da pista. Nível efetivo da
    // ponta da IA (não um teto rígido). Teto 125 = limite real do iRacing (não 100):
    // pistas com "macetes" que a IA não pega (ex.: Summit Point) precisam de offsets
    // altos que passam de 100 nos tiers acima do rookie.
    let track_offset = resolved_track_id.map(track_skill_offset).unwrap_or(0);
    // Perfil ADAPTATIVO do jogador (por custid): nível geral + aptidão na pista alvo.
    // Carreira nova herda o perfil; começa em 0 se for um jogador sem histórico.
    let profile = load_adaptive_profile(&base_dir, custid);
    let adapt_track = resolved_track_id
        .map(|id| profile.track_delta(id))
        .unwrap_or(0);
    let max_skill = (tier_difficulty_base(cat.tier) + track_offset + profile.global + adapt_track)
        .clamp(0, 125);
    let min_skill = if skills.is_empty() {
        (max_skill - 25).max(0)
    } else {
        let lo = skills.iter().cloned().fold(f64::INFINITY, f64::min);
        (lo.round() as i64).clamp(0, max_skill)
    };
    // Chuva: se a corrida ALVO é molhada, baixa a banda (pelotão mais lento — chuva
    // no iRacing é punitiva; subir a IA faria o humano forçar e rodar). v1: rebaixa o
    // campo todo pela penalidade num fator_chuva médio (~50). Re-rank por piloto depois.
    let rain_pen = resolved_track_id
        .and_then(|id| stories.get(&id))
        .filter(|s| s.is_wet_race)
        .map(|s| crate::iracing_sdk::weather::rain_skill_penalty(50.0, s.race_intensity))
        .unwrap_or(0);
    let max_skill = (max_skill - rain_pen).clamp(0, 125);
    let min_skill = (min_skill - rain_pen).clamp(0, max_skill);
    let max_drivers = (skills.len() as i64 + 1).max(2);

    // Clima global (fallback) = o da 1ª etapa do calendário.
    // Clima global (fallback p/ eventos sem weather própria) = seco/claro.
    let global_weather = season_gen::EventWeather {
        skies: 1,
        humidity: 45,
        temp_c: 26,
        track_water: 0,
        keyframes: vec![
            season_gen::WeatherKeyframe {
                event_type: 1,
                time_offset: -90,
            },
            season_gen::WeatherKeyframe {
                event_type: 0,
                time_offset: 0,
            },
            season_gen::WeatherKeyframe {
                event_type: 1,
                time_offset: race_end,
            },
        ],
        weather_id: format!("{custid}_global"),
        start_time: format!("{}-06-01T16:00:00", season.ano),
    };

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
    let dir =
        paths::aiseasons_dir().ok_or("Não foi possível localizar a pasta aiseasons do iRacing.")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    let path = dir.join(format!("{}.json", safe_name.trim()));
    let json = serde_json::to_string_pretty(&season_json)
        .map_err(|e| format!("Falha ao serializar: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Falha ao gravar: {e}"))?;

    Ok(SeasonGenResult {
        path: path.display().to_string(),
        name,
        events: params_events_len(&season_json),
        targeted_track,
        ai_skill: max_skill,
        auto_targeted,
    })
}

/// Conta os eventos no JSON gerado (para a UI).
fn params_events_len(v: &serde_json::Value) -> usize {
    v["events"].as_array().map(|a| a.len()).unwrap_or(0)
}

/// Hemisfério da pista pelo país (sul = Austrália, Argentina, Brasil, etc.).
fn track_hemisphere(pais: &str) -> crate::iracing_sdk::weather::Hemisphere {
    use crate::iracing_sdk::weather::Hemisphere;
    const SOUTH: [&str; 9] = [
        "🇦🇺",
        "🇦🇷",
        "🇧🇷",
        "🇿🇦",
        "🇳🇿",
        "🇨🇱",
        "🇺🇾",
        "Austrália",
        "Australia",
    ];
    if SOUTH.iter().any(|s| pais.contains(s)) {
        Hemisphere::South
    } else {
        Hemisphere::North
    }
}

/// `rain_group` da pista → tendência de clima do gerador.
fn climate_tendency(
    g: crate::models::enums::RainGroup,
) -> crate::iracing_sdk::weather::ClimateTendency {
    use crate::iracing_sdk::weather::ClimateTendency;
    use crate::models::enums::RainGroup;
    match g {
        RainGroup::Dry => ClimateTendency::Dry,
        RainGroup::Rainy => ClimateTendency::Rainy,
        _ => ClimateTendency::Normal,
    }
}

/// Mês (1–12) a partir da semana do ano (1–52).
fn month_from_week(week: i32) -> u32 {
    (((week.max(1) - 1) * 12 / 52) + 1).clamp(1, 12) as u32
}

/// Semente estável por etapa (carreira + id da etapa) → clima/horário fixos
/// (não re-sorteia a cada export).
fn event_seed(career_id: &str, event_id: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    career_id.hash(&mut h);
    event_id.hash(&mut h);
    h.finish()
}

/// Monta o `EventWeather` de uma etapa via o gerador de clima + horário (golden
/// hour por estação). Devolve também a `WeatherStory` (p/ a penalidade da chuva).
fn build_event_weather(
    track: &crate::constants::tracks::TrackInfo,
    week_of_year: i32,
    temp_c: f64,
    year: i32,
    tier: u8,
    custid: i64,
    seed: u64,
    is_first_race: bool,
    race_end: i64,
) -> (
    crate::iracing_sdk::season_gen::EventWeather,
    crate::iracing_sdk::weather::WeatherStory,
) {
    use crate::iracing_sdk::{season_gen, weather};
    let month = month_from_week(week_of_year);
    let story = weather::generate_weather(
        month,
        track_hemisphere(track.pais),
        climate_tendency(track.rain_group),
        seed,
        is_first_race,
    );
    let is_lit = track.track_id == 556; // Charlotte Roval — única com iluminação
    let hour = weather::generate_race_start_hour(story.season, tier, is_lit, seed ^ 0x55);
    let profile = weather::story_to_profile(&story, race_end);
    let hh = (hour.floor() as i64).clamp(0, 23);
    let mm = (((hour - hour.floor()) * 60.0).round() as i64).clamp(0, 59);
    let start_time = format!("{year}-{month:02}-15T{hh:02}:{mm:02}:00");
    let ew = season_gen::EventWeather {
        skies: profile.skies,
        humidity: profile.humidity,
        temp_c: temp_c.round() as i64,
        track_water: profile.track_water,
        keyframes: profile
            .keyframes
            .into_iter()
            .map(|(event_type, time_offset)| season_gen::WeatherKeyframe {
                event_type,
                time_offset,
            })
            .collect(),
        weather_id: format!("{custid}_{}", uuid::Uuid::new_v4()),
        start_time,
    };
    (ew, story)
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

    let player =
        dq::get_player_driver(&db.conn).map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let team = cq::get_active_contract_for_pilot(&db.conn, &player.id)
        .ok()
        .flatten()
        .and_then(|contract| {
            tq::get_team_by_id(&db.conn, &contract.equipe_id)
                .ok()
                .flatten()
        })
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

    let custid = iracing_sdk::cached_custid()
        .ok_or("Ainda não capturei seu custid — abra o iRacing e entre numa sessão uma vez.")?;
    let car =
        roster_gen::car_spec(&car_key).ok_or_else(|| format!("Carro desconhecido: {car_key}"))?;

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
