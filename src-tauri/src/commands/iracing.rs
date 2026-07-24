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

/// DUMP do YAML de sessão do iRacing num arquivo (para inspecionar a estrutura
/// real do `ResultsPositions`/quali antes de escrever o parser). Grava em
/// `%TEMP%/loop_session_dump.yaml` e devolve o caminho. Use com o iRacing aberto
/// numa sessão que já rodou (corrida/quali concluída ou em andamento).
#[tauri::command]
pub fn iracing_dump_session_yaml() -> Result<String, String> {
    let session = iracing_sdk::read_session().map_err(|e| e.to_string())?;
    let path = std::env::temp_dir().join("loop_session_dump.yaml");
    std::fs::write(&path, &session.session_yaml)
        .map_err(|e| format!("Falha ao gravar o dump: {e}"))?;
    Ok(path.to_string_lossy().to_string())
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

/// Versão enxuta do histórico (sem os arrays grandes) para o overlay ao vivo
/// "iRacing Conectado" — pensada para polling rápido (1Hz) sem peso.
#[tauri::command]
pub fn iracing_get_race_feedback() -> race_monitor::RaceFeedback {
    race_monitor::get_feedback()
}

/// Traz a janela do iRacing (simulador ou launcher) para frente, para o jogador
/// "cair" no iRacing logo após exportar os dados. `Ok(false)` = iRacing fechado.
#[tauri::command]
pub fn iracing_focus_window() -> Result<bool, String> {
    iracing_sdk::focus_iracing_window().map_err(|e| e.to_string())
}

/// Inicia o iRacingUI (launcher/menu) quando ele não está aberto. Best-effort:
/// procura o `iRacingUI.exe` nos caminhos de instalação comuns. `Ok(false)` = não
/// encontrei o executável (aí o usuário abre manualmente).
#[tauri::command]
pub fn iracing_launch_ui() -> Result<bool, String> {
    #[cfg(windows)]
    {
        use std::path::PathBuf;
        let mut candidates: Vec<PathBuf> = Vec::new();
        for key in ["ProgramFiles(x86)", "ProgramW6432", "ProgramFiles"] {
            if let Ok(base) = std::env::var(key) {
                let base = PathBuf::from(base);
                candidates.push(base.join("iRacing").join("ui").join("iRacingUI.exe"));
                candidates.push(base.join("iRacing").join("iRacingUI.exe"));
            }
        }
        for path in candidates {
            if path.is_file() {
                // stdio nulo: senão o iRacingUI herda nossos pipes e despeja os
                // logs dele no nosso console.
                return std::process::Command::new(&path)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .map(|_| true)
                    .map_err(|e| format!("Falha ao iniciar o iRacingUI: {e}"));
            }
        }
        Ok(false)
    }
    #[cfg(not(windows))]
    {
        Ok(false)
    }
}

/// Cores dos carros/times por NOME do piloto (para colorir as linhas dos gráficos
/// com a cor do time). `player_color` = cor do time do jogador (o nome dele na
/// pista é o username do iRacing, não casa com o banco — por isso vai à parte).
#[derive(serde::Serialize)]
pub struct CarColors {
    pub by_name: std::collections::HashMap<String, String>,
    pub player_color: Option<String>,
}

#[tauri::command]
pub fn iracing_car_colors(app: tauri::AppHandle, career_id: String) -> Result<CarColors, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::{contracts as cq, drivers as dq, teams as tq};
    use std::collections::HashMap;
    use tauri::Manager;

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let team_color = |pilot_id: &str| -> Option<String> {
        cq::get_active_contract_for_pilot(&db.conn, pilot_id)
            .ok()
            .flatten()
            .and_then(|c| tq::get_team_by_id(&db.conn, &c.equipe_id).ok().flatten())
            .map(|t| t.cor_primaria)
            .filter(|c| !c.is_empty())
    };

    let player = dq::get_player_driver(&db.conn).ok();
    let player_color = player.as_ref().and_then(|p| team_color(&p.id));
    // Categoria do jogador = a do time dele (via contrato); cai pra categoria_atual.
    let categoria = player.as_ref().and_then(|p| {
        cq::get_active_contract_for_pilot(&db.conn, &p.id)
            .ok()
            .flatten()
            .and_then(|c| tq::get_team_by_id(&db.conn, &c.equipe_id).ok().flatten())
            .map(|t| t.categoria)
            .or_else(|| p.categoria_atual.clone())
    });

    let mut by_name: HashMap<String, String> = HashMap::new();
    if let Some(cat) = categoria {
        if let Ok(drivers) = dq::get_drivers_by_category(&db.conn, &cat) {
            for d in drivers {
                if let Some(color) = team_color(&d.id) {
                    by_name.insert(d.nome, color);
                }
            }
        }
    }

    Ok(CarColors {
        by_name,
        player_color,
    })
}

/// GATILHO INVERSO: se o iRacing acabou de fechar (o SDK parou de responder),
/// traz NOSSA janela para frente. Feito para ser chamado no mesmo poller que já
/// importa o resultado. Devolve `true` quando focou (havia acabado de fechar).
#[tauri::command]
pub fn iracing_focus_self_if_closed(app: tauri::AppHandle) -> bool {
    use tauri::Manager;
    // Dentro da janela pós-fechamento? `initial` = primeiro poll (bring-up).
    let (in_window, initial) = race_monitor::poll_focus_self();
    if !in_window {
        return false;
    }
    let Some(win) = app.get_webview_window("main") else {
        return false;
    };

    // Foca no bring-up inicial OU se o iRacing roubou o foco de volta (sua UI/menu
    // se traz para frente alguns segundos após o sim fechar). Se o usuário abriu
    // outra coisa, não incomodamos.
    #[cfg(windows)]
    {
        if let Ok(hwnd) = win.hwnd() {
            if initial || iracing_sdk::foreground_is_iracing() {
                let _ = win.unminimize();
                let _ = win.show();
                iracing_sdk::force_foreground_window(hwnd.0 as isize);
            }
        }
    }
    #[cfg(not(windows))]
    {
        if initial {
            let _ = win.unminimize();
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
    true
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

/// PERCEPÇÃO de rivalidade (calibração/debug): roda o motor de percepção sobre uma
/// corrida — ao vivo (`saved_name = None`) ou salva — para um CARRO-SONDA
/// (`probe_car_idx`; default = o jogador). Devolve o livro-razão por oponente SEM
/// aplicar nada no motor de rivalidade. Contato atribuído só é resolvido no caso
/// ao-vivo + probe-jogador (para uma IA-sonda vem vazio, como projetado).
///
/// Ver `docs/superpowers/specs/2026-07-18-track-rivalry-perception-design.md`.
#[tauri::command]
pub fn iracing_perceive_rivalries(
    app: tauri::AppHandle,
    saved_name: Option<String>,
    probe_car_idx: Option<i32>,
) -> Result<crate::iracing_sdk::rivalry_perception::RivalryPerception, String> {
    use crate::iracing_sdk::rivalry_perception::{
        perceive_rivalries, ContactSeed, ContactTier, PerceptionParams,
    };

    let history = match &saved_name {
        Some(name) => iracing_load_saved_race(app, name.clone())?,
        None => race_monitor::get_history(),
    };
    if history.laps.is_empty() {
        return Err("Sem trace de campo — a corrida não gerou voltas.".to_string());
    }

    let probe = probe_car_idx.unwrap_or(history.player_car_idx);

    // Contato só quando analisamos a corrida AO VIVO e o probe é o jogador.
    let contact: Option<ContactSeed> = if saved_name.is_none() && probe == history.player_car_idx {
        let status = race_monitor::poll();
        let attempt = status
            .attempts
            .iter()
            .find(|a| a.number == status.attempt_number)
            .or_else(|| status.attempts.last());
        attempt.and_then(|a| {
            a.collided_with_car_number.and_then(|num| {
                history
                    .cars_meta
                    .iter()
                    .find(|m| m.car_number == num)
                    .map(|m| {
                        // Tier fino (crítico/leve) fica pra fase de calibração; por ora
                        // DNF vs. contato grave é o suficiente.
                        let tier = if a.evidence.raced && !a.evidence.reached_checkered {
                            ContactTier::Dnf
                        } else {
                            ContactTier::Major
                        };
                        ContactSeed {
                            opponent_car_idx: m.idx,
                            tier,
                        }
                    })
            })
        })
    } else {
        None
    };

    Ok(perceive_rivalries(
        &history,
        probe,
        contact,
        &PerceptionParams::default(),
    ))
}

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
fn season_pointer_path(base_dir: &std::path::Path, career_id: &str) -> Option<std::path::PathBuf> {
    Some(
        base_dir
            .join("iracing_pointers")
            .join(format!("{career_id}.json")),
    )
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

/// Contexto de carro do ÚLTIMO export (por custid), pro mecanismo 2 (adaptativo cego ao
/// carro). O export sabe os carros e os NÚMEROS; o pós-corrida casa a frente
/// (`car_idx`→número via `cars_meta`) e desconta do ritmo o que o carro explica. Persistido
/// junto do perfil adaptativo. Ver [`crate::iracing_sdk::car_difficulty`].
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct CarDifficultyContext {
    /// Pista alvo do export (o pós-corrida só usa se casar com a pista corrida).
    track_id: i64,
    /// Vantagem de carro do jogador (car-perf) na pista.
    player_advantage: f64,
    /// número do carro (string) → vantagem de carro (car-perf) na pista.
    by_number: std::collections::HashMap<String, f64>,
}

fn car_difficulty_context_path(base_dir: &std::path::Path, custid: i64) -> std::path::PathBuf {
    base_dir
        .join("iracing_adaptive")
        .join(format!("{custid}_car.json"))
}

/// Persiste o contexto de carro do export (best-effort; erro só é logado pelo chamador).
fn save_car_difficulty_context(
    base_dir: &std::path::Path,
    custid: i64,
    ctx: &CarDifficultyContext,
) -> Result<(), String> {
    let path = car_difficulty_context_path(base_dir, custid);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    }
    let json =
        serde_json::to_string_pretty(ctx).map_err(|e| format!("Falha ao serializar: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Falha ao gravar: {e}"))
}

/// Lê o contexto de carro do último export (None se não existe).
fn load_car_difficulty_context(
    base_dir: &std::path::Path,
    custid: i64,
) -> Option<CarDifficultyContext> {
    std::fs::read_to_string(car_difficulty_context_path(base_dir, custid))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
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

    let current = adaptive::Deltas {
        global: profile.global,
        track: profile.track_delta(track_id),
    };
    // Regime RÁPIDO (posição+gravidade+RITMO LIMPO) em TODOS os tiers. Reusa
    // build_adaptive_result (voltas por carro) → fast_result_from descarta voltas de erro,
    // então uma rodada não baixa a dificuldade. O regime por ritmo completo
    // (compute_adaptive_update) segue dormente — mantido no código pra revisitar.
    let race = race_monitor::build_adaptive_result(&history, track_id);
    // Mecanismo 2 (cego ao carro): carrega o contexto de carro do último export e casa a
    // frente (car_idx→número via cars_meta → vantagem). Só usa se a pista bater. Sem contexto
    // ou pista diferente → None → comportamento antigo (adaptativo puro por ritmo).
    let car_ctx = load_car_difficulty_context(&base_dir, custid)
        .filter(|c| c.track_id == track_id)
        .map(|c| {
            let by_idx = history
                .cars_meta
                .iter()
                .filter_map(|m| {
                    c.by_number
                        .get(&m.car_number.to_string())
                        .map(|adv| (m.idx, *adv))
                })
                .collect();
            adaptive::CarContext {
                player_advantage: c.player_advantage,
                by_idx,
            }
        });
    let summary = adaptive::fast_result_from(&race, car_ctx.as_ref());
    let update = adaptive::compute_fast_update(&summary, &current);
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

/// Setup compartilhado pelo preview e pelo import. Lê o RESULTADO OFICIAL do
/// iRacing (JSON do aiseason, persistido) — não reconstrói ao vivo. Acha o arquivo
/// e o evento pelo "post-it" gravado no export + a próxima corrida pendente da
/// carreira. A batida do jogador (custo de conserto) ainda vem do monitor ao vivo.
/// Devolve `(banco, career_dir, track_id, pior severidade do jogador, resultado)`.
fn build_session_race_result(
    app: &tauri::AppHandle,
    career_id: &str,
) -> Result<
    (
        crate::db::connection::Database,
        std::path::PathBuf,
        i64,
        String, // pior severidade de batida do jogador (base do conserto)
        crate::simulation::race::RaceResult,
        crate::iracing_sdk::telemetry_analysis::TelemetryAnalysis,
        crate::iracing_sdk::race_monitor::RaceHistory,
        // Mapa número do carro → driver_id (para resolver a identidade dos rivais
        // percebidos do SDK na ponte de rivalidade).
        std::collections::HashMap<i64, String>,
        // Direção do impacto no pico do jogador (front/rear/side/vertical; vazia se sem batida).
        String,
        // Estilo de pilotagem do jogador (fatores de desgaste por peça; neutro se sem estilo).
        crate::car::driving_style::StyleFactors,
    ),
    String,
> {
    use crate::config::app_config::AppConfig;
    use crate::constants::tracks::get_track;
    use crate::db::connection::Database;
    use crate::db::queries::drivers as dq;
    use crate::db::queries::seasons as sq;
    use crate::iracing_sdk::{aiseason_results, race_monitor, result_bridge, telemetry_analysis};
    use tauri::Manager;

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    // Mapa de números: driver_id → número → reverte para número → driver_id.
    let numbers: std::collections::HashMap<String, i64> =
        std::fs::read_to_string(numbers_path(&base_dir, career_id))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
    let by_number: std::collections::HashMap<i64, String> =
        numbers.into_iter().map(|(id, n)| (n, id)).collect();

    let config = AppConfig::load_or_default(&base_dir);
    let career_dir = config.saves_dir().join(career_id);
    let db_path = career_dir.join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;
    let player_driver = dq::get_player_driver(&db.conn).ok();

    // ── Post-it: arquivo de aiseason + mapa evento→corrida ──────────────────
    let pointer: serde_json::Value = season_pointer_path(&base_dir, career_id)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .ok_or_else(|| {
            "Não achei o registro do aiseason exportado. Exporte os dados (gerar AI Season) antes de importar.".to_string()
        })?;
    let aiseason_file = pointer
        .get("aiseason_file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Registro do aiseason inválido.".to_string())?;

    // ── Próxima corrida pendente da carreira → índice do evento ─────────────
    let active_season = sq::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao ler temporada: {e}"))?
        .ok_or("Nenhuma temporada ativa.")?;
    let next_race = crate::commands::race::get_next_player_race(&db.conn, &active_season)?
        .ok_or("O jogador não possui corrida pendente.")?;
    let events = pointer.get("events").and_then(|v| v.as_array());
    let event_index = events
        .and_then(|evs| {
            evs.iter()
                .position(|e| e.get("race_id").and_then(|v| v.as_str()) == Some(next_race.id.as_str()))
        })
        .ok_or_else(|| {
            format!(
                "A próxima corrida ({}) não está no aiseason exportado. Exporte novamente.",
                next_race.track_name
            )
        })?;

    // ── Lê o resultado oficial do JSON ──────────────────────────────────────
    let season_json: serde_json::Value = std::fs::read_to_string(aiseason_file)
        .map_err(|e| format!("Falha ao ler o aiseason: {e}"))
        .and_then(|s| serde_json::from_str(&s).map_err(|e| format!("aiseason inválido: {e}")))?;
    let event = aiseason_results::parse_event_result(&season_json, event_index)
        .ok_or("Evento sem resultado no aiseason.")?;
    if !event.is_final() {
        return Err(
            "O iRacing ainda não gravou o resultado dessa corrida. Termine/saia da corrida no iRacing (ele simula o resto e salva) e tente de novo."
                .to_string(),
        );
    }
    // Pista de fato EXPORTADA para este evento — pode ser uma FREE substituta quando a
    // pista real do calendário é conteúdo pago (ver `free_or_substitute` no export).
    // Comparamos o resultado do iRacing contra o que foi exportado, não contra a pista
    // original da carreira (senão a substituição de teste tropeçaria aqui).
    let exported_track_id = events
        .and_then(|evs| evs.get(event_index))
        .and_then(|e| e.get("track_id"))
        .and_then(|v| v.as_i64())
        .unwrap_or(next_race.track_id as i64);
    if event.track_id != exported_track_id {
        return Err(format!(
            "A pista do resultado (id {}) não bate com a corrida exportada ({}, id {}).",
            event.track_id, next_race.track_name, exported_track_id
        ));
    }

    let track_name = get_track(next_race.track_id)
        .map(|t| t.nome.to_string())
        .unwrap_or_else(|| next_race.track_name.clone());
    let player_custid = crate::iracing_sdk::cached_custid().unwrap_or(0);

    // ── Sinais do monitor AO VIVO (o JSON do iRacing não tem) ───────────────
    // O iRacing AI-completa a corrida e marca todo mundo "Running"; sobrepomos a
    // batida do jogador (conserto + DNF) e os abandonos que o monitor flagrou.
    let history = race_monitor::get_history();
    let status = race_monitor::poll();
    let player_crash = result_bridge::player_worst_severity(&status, status.attempt_number);
    let player_attempt = status
        .attempts
        .iter()
        .find(|a| a.number == status.attempt_number)
        .or_else(|| status.attempts.last());
    // Jogador correu mas não cruzou a bandeira (saiu/bateu) → DNF. Inclui o caso
    // de ficar parado na pista (raced fica true assim que ele entra na corrida).
    let player_dnf = player_attempt
        .map(|a| a.evidence.raced && !a.evidence.reached_checkered)
        .unwrap_or(false);
    // "Quem bateu em mim": o monitor guardou o carro mais próximo no contato.
    let player_collided_with_id: Option<String> = player_attempt
        .and_then(|a| a.collided_with_car_number)
        .and_then(|num| by_number.get(&(num as i64)).cloned());
    // Direção do impacto no pico (front/rear/side/vertical) — base do dano por peça no import.
    let player_impact_dir: String = player_attempt
        .and_then(|a| a.peak_impact_dir.clone())
        .unwrap_or_default();
    // Estilo de pilotagem do jogador (fatores de desgaste por peça) — do acumulador ao vivo.
    let player_style: crate::car::driving_style::StyleFactors = player_attempt
        .map(|a| a.style.factors())
        .unwrap_or_default();
    // Carros que o monitor confirmou ter abandonado → número do carro.
    let num_by_idx: std::collections::HashMap<i32, i32> = history
        .cars_meta
        .iter()
        .map(|m| (m.idx, m.car_number))
        .collect();
    let extra_dnf_numbers: std::collections::HashSet<i32> = status
        .events
        .iter()
        .filter(|e| e.kind == "dnf_confirmed")
        .filter_map(|e| e.car_idx)
        .filter_map(|idx| num_by_idx.get(&idx).copied())
        .filter(|n| *n > 0)
        .collect();

    let result = result_bridge::build_race_result_from_aiseason(
        &event,
        &db.conn,
        &by_number,
        player_custid,
        player_driver.as_ref(),
        player_dnf,
        player_collided_with_id.as_deref(),
        &extra_dnf_numbers,
        "", // clima resolvido na persistência
        &track_name,
        &race_monitor::get_player_incidents(),
        &player_crash,
        &player_impact_dir,
    );

    // TELEMETRIA (Fase 2): ritmo/consistência/rival do histórico ao vivo. Só vem
    // completa se o jogador correu; resolve car_idx→nome pelo cars_meta + roster.
    let name_by_idx: std::collections::HashMap<i32, String> = history
        .cars_meta
        .iter()
        .filter_map(|m| {
            let is_player = m.idx == history.player_car_idx;
            let name = if is_player {
                player_driver.as_ref().map(|d| d.nome.clone())
            } else {
                by_number
                    .get(&(m.car_number as i64))
                    .and_then(|id| dq::get_driver(&db.conn, id).ok())
                    .map(|d| d.nome)
            };
            name.map(|n| (m.idx, n))
        })
        .collect();
    // Equipe por car_idx (mesma fonte do resultado oficial: contrato regular ativo →
    // equipe). Resolve o driver_id por número e busca o time do contrato vigente.
    let team_by_idx: std::collections::HashMap<i32, String> = history
        .cars_meta
        .iter()
        .filter_map(|m| {
            let driver_id = if m.idx == history.player_car_idx {
                player_driver.as_ref().map(|d| d.id.clone())
            } else {
                by_number.get(&(m.car_number as i64)).cloned()
            }?;
            let team = crate::db::queries::contracts::get_active_regular_contract_for_pilot(
                &db.conn, &driver_id,
            )
            .ok()
            .flatten()
            .map(|c| {
                crate::db::queries::teams::get_team_by_id(&db.conn, &c.equipe_id)
                    .ok()
                    .flatten()
                    .map(|t| t.nome)
                    .unwrap_or(c.equipe_id)
            })?;
            Some((m.idx, team))
        })
        .collect();
    // Sinais de batida/DNF do jogador (não estão no RaceHistory) para o "erro
    // mais caro": voltas com contato + se abandonou + a volta em que parou.
    let crash_laps: Vec<i32> = player_attempt
        .map(|a| a.crashes.iter().map(|c| c.lap).filter(|l| *l > 0).collect())
        .unwrap_or_default();
    let dnf_lap = if player_dnf {
        history
            .player_laps
            .iter()
            .map(|l| l.lap)
            .max()
            .or_else(|| crash_laps.iter().max().copied())
    } else {
        None
    };
    let player_incidents = telemetry_analysis::PlayerIncidents {
        crash_laps,
        is_dnf: player_dnf,
        dnf_lap,
    };
    let telemetry =
        telemetry_analysis::analyze(&history, &name_by_idx, &team_by_idx, &player_incidents);

    Ok((
        db,
        career_dir,
        next_race.track_id as i64,
        player_crash,
        result,
        telemetry,
        history,
        by_number,
        player_impact_dir,
        player_style,
    ))
}

/// Peça 3: converte o log de quebras drenado do monitor em linhas prontas pra persistir,
/// resolvendo car_number → driver_id. O número do JOGADOR não está no `by_number` (só IAs) →
/// resolve pelo idx dele (`num_by_idx` do histórico) + o driver do save. Chamado SÓ no import
/// (one-shot); o `drain` esvazia o log, então nunca no preview (que repete).
fn resolve_breakdown_rows(
    conn: &rusqlite::Connection,
    history: &crate::iracing_sdk::race_monitor::RaceHistory,
    by_number: &std::collections::HashMap<i64, String>,
) -> Vec<crate::db::queries::race_breakdowns::RaceBreakdownRow> {
    use crate::db::queries::drivers as dq;
    let player_id: Option<String> = dq::get_player_driver(conn).ok().map(|p| p.id);
    let num_by_idx: std::collections::HashMap<i32, i32> =
        history.cars_meta.iter().map(|m| (m.idx, m.car_number)).collect();
    let player_number: Option<i32> = num_by_idx
        .get(&history.player_car_idx)
        .copied()
        .filter(|n| *n > 0);
    crate::iracing_sdk::race_monitor::drain_breakdown_log()
        .into_iter()
        .filter_map(|o| {
            let driver_id = match (player_number, player_id.as_ref()) {
                (Some(pnum), Some(pid)) if o.car_number as i32 == pnum => Some(pid.clone()),
                _ => by_number.get(&(o.car_number as i64)).cloned(),
            }?;
            Some(crate::db::queries::race_breakdowns::RaceBreakdownRow {
                driver_id,
                part: o.part,
                problem: o.problem,
                lap: o.lap,
                severity: o.severity,
                penalty_secs: o.penalty_secs,
                forced: o.forced,
                label: o.label,
            })
        })
        .collect()
}

/// PREVIEW (read-only) da ponte sessão→`RaceResult`: reconstrói o resultado da
/// última corrida disputada no iRacing como o `RaceResult` que a carreira
/// consome — SEM gravar nada no banco. Serve para validar o mapeamento (posições,
/// grid, volta rápida, DNFs) contra a tela do iRacing antes de importar.
#[tauri::command]
pub fn iracing_preview_race_result(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<crate::simulation::race::RaceResult, String> {
    let (_db, _dir, _track_id, _sev, result, _tel, _hist, _by_number, _impact_dir, _style) =
        build_session_race_result(&app, &career_id)?;
    Ok(result)
}

/// Ponte de rivalidade de PISTA: aplica no motor de rivalidade as rivalidades que a
/// percepção do SDK detectou na corrida importada, resolvendo a identidade dos
/// oponentes (car_number → driver_id via `by_number`). Só o jogador é o probe.
///
/// Best-effort: qualquer falha é engolida (o import já foi persistido). Atualiza os
/// eixos (histórico/recente) E grava um CAPÍTULO do arco por rival (a "novela" que a
/// IA recapitula), com a interação REAL da percepção. Idempotente por rodada (o
/// `insert_episode` deduplica com o `record_rivalry_episodes` do boletim).
/// Ver `docs/superpowers/specs/2026-07-18-track-rivalry-perception-design.md` §10.
fn apply_track_rivalries(
    conn: &rusqlite::Connection,
    history: &crate::iracing_sdk::race_monitor::RaceHistory,
    by_number: &std::collections::HashMap<i64, String>,
    race_result: &crate::simulation::race::RaceResult,
    rodada: i32,
    categoria: &str,
) {
    use crate::iracing_sdk::rivalry_perception::{
        perceive_rivalries, ContactSeed, ContactTier, PerceptionParams,
    };
    use crate::models::rivalry::RivalryType;
    use crate::rivalry::{apply_rivalry_event, RivalryEvent};

    // driver_id do jogador (o probe).
    let Some(player_id) = race_result
        .race_results
        .iter()
        .find(|r| r.is_jogador)
        .map(|r| r.pilot_id.clone())
    else {
        return;
    };

    // car_idx → driver_id, via car_number (cars_meta) + by_number. Só carros mapeados.
    let driver_by_idx: std::collections::HashMap<i32, String> = history
        .cars_meta
        .iter()
        .filter_map(|m| {
            by_number
                .get(&(m.car_number as i64))
                .map(|id| (m.idx, id.clone()))
        })
        .collect();

    // Contato atribuído ("quem bateu em mim"), do monitor ao vivo → semente da percepção.
    let contact: Option<ContactSeed> = {
        let status = race_monitor::poll();
        let attempt = status
            .attempts
            .iter()
            .find(|a| a.number == status.attempt_number)
            .or_else(|| status.attempts.last());
        attempt.and_then(|a| {
            a.collided_with_car_number.and_then(|num| {
                history
                    .cars_meta
                    .iter()
                    .find(|m| m.car_number == num)
                    .map(|m| {
                        let tier = if a.evidence.raced && !a.evidence.reached_checkered {
                            ContactTier::Dnf
                        } else {
                            ContactTier::Major
                        };
                        ContactSeed {
                            opponent_car_idx: m.idx,
                            tier,
                        }
                    })
            })
        })
    };

    let season = crate::db::queries::seasons::get_active_season(conn)
        .ok()
        .flatten();
    let temporada = season.as_ref().map(|s| s.numero).unwrap_or(0);
    let ano = season.as_ref().map(|s| s.ano).unwrap_or(0);

    // Posição final do jogador (para decidir quem levou a melhor em cada capítulo).
    let player_res = race_result
        .race_results
        .iter()
        .find(|d| d.pilot_id == player_id);

    let perception =
        perceive_rivalries(history, history.player_car_idx, contact, &PerceptionParams::default());

    for opp in &perception.opponents {
        let Some(opp_id) = driver_by_idx.get(&opp.car_idx) else {
            continue;
        };
        if *opp_id == player_id {
            continue;
        }
        let is_contact = opp.hits.iter().any(|h| h.kind == "contato");
        let tipo = if is_contact {
            RivalryType::Colisao
        } else {
            RivalryType::Pista
        };
        let applied = match apply_rivalry_event(
            conn,
            &RivalryEvent {
                piloto_a: player_id.clone(),
                piloto_b: opp_id.clone(),
                tipo,
                historical_delta: opp.historical_delta,
                recent_delta: opp.recent_delta,
                temporada,
            },
        ) {
            Ok(a) => a,
            Err(_) => continue,
        };

        // Capítulo do arco: interação real + quem levou a melhor hoje.
        let opp_res = race_result
            .race_results
            .iter()
            .find(|d| &d.pilot_id == opp_id);
        let winner_id = match (player_res, opp_res) {
            (Some(p), Some(o)) => {
                let (p_ok, o_ok) = (!p.is_dnf, !o.is_dnf);
                if p_ok && o_ok {
                    match p.finish_position.cmp(&o.finish_position) {
                        std::cmp::Ordering::Less => Some(player_id.clone()),
                        std::cmp::Ordering::Greater => Some(opp_id.clone()),
                        std::cmp::Ordering::Equal => None,
                    }
                } else if p_ok {
                    Some(player_id.clone())
                } else if o_ok {
                    Some(opp_id.clone())
                } else {
                    None
                }
            }
            _ => None,
        };
        let opp_name = opp_res
            .map(|o| o.pilot_name.clone())
            .unwrap_or_else(|| opp_id.clone());
        let interaction = if is_contact { "colisao" } else { "duelo" };
        let summary = if is_contact {
            format!("contato com {opp_name} em {}", race_result.track_name)
        } else {
            format!(
                "duelo de {} voltas com {opp_name} em {}",
                opp.duel_laps, race_result.track_name
            )
        };
        let _ = crate::db::queries::rivalry_episodes::insert_episode(
            conn,
            &crate::db::queries::rivalry_episodes::RivalryEpisode {
                piloto1_id: player_id.clone(),
                piloto2_id: opp_id.clone(),
                temporada,
                rodada,
                ano,
                categoria: categoria.to_string(),
                track_name: race_result.track_name.clone(),
                interaction: interaction.to_string(),
                winner_id,
                summary,
                perceived: applied.new_perceived,
            },
        );
    }

    // Rivalidade IA-vs-IA: o motor de percepção aceita QUALQUER carro-sonda, não só o jogador.
    // Alimenta o ledger de rivalidade também entre as IAs, pra a "novela" emergir do grid
    // inteiro (piloto larga → vira rival → dá título → ex-time em crise), e não só ao redor do
    // jogador. Sem contato atribuído (só o jogador tem a semente do monitor) e SEM episódio (o
    // arco recapitulado é player-facing). Dedupe por par normalizado — o ledger é simétrico
    // (`normalize_pair`), então cada par de IA é aplicado UMA vez (o outro lado repetiria e
    // dobraria os deltas). Pares que envolvem o jogador já vieram do probe-jogador acima. O
    // custo é O(n²) sobre os snapshots, mas roda uma única vez no import.
    let mut seen_ai_pairs: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    let ai_idxs: Vec<i32> = driver_by_idx
        .keys()
        .copied()
        .filter(|&idx| idx != history.player_car_idx)
        .collect();
    for probe_idx in ai_idxs {
        let Some(probe_id) = driver_by_idx.get(&probe_idx) else {
            continue;
        };
        if *probe_id == player_id {
            continue;
        }
        let probe_perception =
            perceive_rivalries(history, probe_idx, None, &PerceptionParams::default());
        for opp in &probe_perception.opponents {
            let Some(opp_id) = driver_by_idx.get(&opp.car_idx) else {
                continue;
            };
            if opp_id == probe_id || *opp_id == player_id {
                continue; // par com o jogador já tratado pelo probe-jogador
            }
            // Chave canônica (ordem estável) → aplica o par só uma vez.
            let key = if probe_id <= opp_id {
                (probe_id.clone(), opp_id.clone())
            } else {
                (opp_id.clone(), probe_id.clone())
            };
            if !seen_ai_pairs.insert(key) {
                continue;
            }
            let is_contact = opp.hits.iter().any(|h| h.kind == "contato");
            let tipo = if is_contact {
                RivalryType::Colisao
            } else {
                RivalryType::Pista
            };
            let _ = apply_rivalry_event(
                conn,
                &RivalryEvent {
                    piloto_a: probe_id.clone(),
                    piloto_b: opp_id.clone(),
                    tipo,
                    historical_delta: opp.historical_delta,
                    recent_delta: opp.recent_delta,
                    temporada,
                },
            );
        }
    }
}

/// Resultado de um import automático: o `RaceResult` (para a TELA de resultado) +
/// o resumo (para o pop-up de conserto).
#[derive(serde::Serialize)]
pub struct AutoImportResult {
    pub race_result: crate::simulation::race::RaceResult,
    pub summary: crate::commands::race::ImportedRaceSummary,
    /// Avaliação de carreira (expectativa vs resultado, nota, frases). `None` se
    /// não der para avaliar — a tela trata e nunca quebra.
    pub evaluation: Option<crate::race_eval::RaceEvaluation>,
    /// Análise de telemetria (ritmo, consistência, rival). Vazia se não houve
    /// telemetria (jogador saiu cedo / não monitorado).
    pub telemetry: crate::iracing_sdk::telemetry_analysis::TelemetryAnalysis,
}

/// GATILHO AUTOMÁTICO: chamado em loop pelo front. Se o iRacing já gravou o
/// resultado da próxima corrida pendente (jogador terminou/saiu da corrida),
/// IMPORTA para a carreira e devolve o resultado + resumo para a tela abrir
/// sozinha. Se ainda não há resultado pronto (ou nada a importar), devolve `None`
/// — SEM erro, para o poller não fazer barulho. Idempotente: após importar, a
/// corrida vira Concluída e a próxima pendente ainda não terá resultado.
#[tauri::command]
pub fn iracing_auto_import_if_ready(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<Option<AutoImportResult>, String> {
    // "Não está pronto / nada a importar" não é erro: o resultado só existe depois
    // que o jogador termina/sai da corrida no iRacing. Qualquer falha de "ainda
    // não" vira None silencioso; o poller tenta de novo no próximo tick.
    let (
        mut db,
        career_dir,
        track_id,
        player_crash,
        result,
        telemetry,
        history,
        by_number,
        player_impact_dir,
        player_style,
    ) = match build_session_race_result(&app, &career_id) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    // Peça 3: drena os desfechos de quebra (one-shot, só aqui) e resolve para driver_id.
    let breakdowns = resolve_breakdown_rows(&db.conn, &history, &by_number);
    let (summary, race_result) = crate::commands::race::import_iracing_race_result(
        &mut db,
        &career_dir,
        track_id,
        &player_crash,
        &player_impact_dir,
        result,
        &telemetry,
        &history,
        // Estilo neutro (sem sinal capturado) → None, pra não pagar a query de time à toa.
        (!player_style.is_neutral()).then_some(player_style),
        breakdowns,
    )?;

    // Ponte de rivalidade de pista: aplica no motor as rivalidades percebidas do SDK
    // nesta corrida (só o jogador). Atrás da flag IRACER_TRACK_RIVALRY e best-effort —
    // nunca desfaz o import. Idempotente por construção: só roda após um import
    // bem-sucedido (a corrida deixa de ser a pendente e não é reimportada).
    if std::env::var("IRACER_TRACK_RIVALRY").is_ok() {
        if let Ok(Some(entry)) =
            crate::db::queries::calendar::get_calendar_entry_by_id(&db.conn, &summary.race_id)
        {
            apply_track_rivalries(
                &db.conn,
                &history,
                &by_number,
                &race_result,
                entry.rodada,
                &entry.categoria,
            );
        }
    }

    // Clima da corrida importada: resolve+persiste pela fonte única (mesmo do export).
    if let Some(track) = crate::constants::tracks::get_track(track_id as u32) {
        if let Ok(Some(entry)) =
            crate::db::queries::calendar::get_calendar_entry_by_id(&db.conn, &summary.race_id)
        {
            let _ = resolve_and_persist_race_weather(
                &db.conn,
                &career_id,
                track,
                entry.week_of_year,
                &summary.race_id,
                false,
            );
        }
    }
    let evaluation = crate::commands::race::compute_race_evaluation(&db.conn, &race_result);

    // Persiste a tela completa (resultado + avaliação + telemetria/gráficos) para
    // o jogador reabrir a classificação final depois pela Home.
    crate::commands::race::save_race_screen(
        &career_dir,
        &summary.race_id,
        &serde_json::json!({
            "race_result": &race_result,
            "evaluation": &evaluation,
            "telemetry": &telemetry,
            "maintenance": &summary.maintenance,
        }),
    );

    Ok(Some(AutoImportResult {
        race_result,
        summary,
        evaluation,
        telemetry,
    }))
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
    if let Some((_season, race)) = next.as_ref() {
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

        race_monitor::install_breakdown_director(dir, player_live, weather);
    }

    Ok(RosterGenResult {
        path: path.display().to_string(),
        drivers: roster.drivers.len(),
    })
}







#[path = "iracing/controle_corrida.rs"]
mod controle_corrida;
pub use controle_corrida::*;

#[path = "iracing/pintura.rs"]
mod pintura;
pub use pintura::*;

#[path = "iracing/clima.rs"]
mod clima;
pub use clima::*;

#[path = "iracing/previsao_quebras.rs"]
mod previsao_quebras;
pub use previsao_quebras::*;

#[path = "iracing/teste_chuva.rs"]
mod teste_chuva;
pub use teste_chuva::*;

#[path = "iracing/temporada.rs"]
mod temporada;
pub use temporada::*;
