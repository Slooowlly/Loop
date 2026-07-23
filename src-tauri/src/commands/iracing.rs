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
        let weather = if let Some(track) = get_track(race.track_id) {
            let mut story = weather::generate_weather(
                month_from_week(race.week_of_year),
                track_hemisphere(track.pais),
                climate_tendency(track.rain_group),
                ev_seed,
                false,
            );
            if force_wet.unwrap_or(false) {
                story.is_wet_race = true;
                story.race_intensity = weather::RainIntensity::Heavy;
                story.scenario = weather::WeatherScenario::SteadyRain;
            }
            crate::car::breakdown::Weather {
                wetness: story_to_weather_condition(&story).wetness(),
                temperature: weather::story_temperature(&story, ev_seed) as f64,
                humidity: weather::story_to_profile(&story, 60).humidity as f64,
                wind_kmh: weather::generate_wind(&story, ev_seed).speed_kmh as f64,
            }
        } else {
            crate::car::breakdown::Weather::NEUTRAL
        };
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
/// Degrau FIXO abaixo do sweet spot no rookie (tier 0), seja qual for a pista. Rookie é a
/// categoria de iniciante: rebaixamos SEMPRE este tanto de pontos, independente do offset da
/// pista (Rudskogen 72→64, Oulton 78→70, VIR 105→97). Só o rookie recebe; as outras divisões
/// ficam iguais. Aplicado sobre o sweet spot final — pode descer abaixo do baseline do tier.
const ROOKIE_DIFFICULTY_DISCOUNT: i64 = 8;

fn tier_difficulty_base(tier: u8) -> i64 {
    // ESCADA ACHATADA (frente efetiva). A diferença entre divisões é pequena de propósito:
    // no iRacing o CARRO já faz a divisão de cima ser mais rápida; o driverSkill% só precisa
    // subir um pouco. Isso deixa MARGEM (até 125) pro offset de pista + adaptativo + futuro
    // efeito de carro, e a dificuldade real mora no adaptativo (que se estica pra quem é bom).
    match tier {
        0 => 72, // Rookie   (base baixa = "rodinhas"; o adaptativo sobe se dominar)
        1 => 79, // Amador
        2 => 81, // Pro / Production / BMW
        3 => 82, // GT4
        4 => 83, // GT3
        5 => 84, // LMP2
        _ => 84, // Endurance / Elite
    }
}

/// Amortecimento do delta GLOBAL por tier. O `global` é por-jogador e compartilhado entre
/// divisões/carreiras; sem isto, um alien acumularia +40 na elite e o rookie da carreira
/// nova viraria absurdo. Aqui o boost pesa menos nas divisões baixas: o alien sente a elite
/// cheia, mas o rookie continua sendo rookie.
fn tier_difficulty_damp(tier: u8) -> f64 {
    match tier {
        0 => 0.50, // Rookie
        1 => 0.70, // Amador
        2 => 0.85, // Pro
        3 => 0.90, // GT4
        4 => 0.95, // GT3
        _ => 1.00, // LMP2 / Endurance
    }
}

/// Sweet spot da ponta da IA (efetivo do melhor) ANTES da penalidade de chuva: base do
/// tier + offset da pista + perfil adaptativo do jogador (global amortecido por tier). É a
/// ÂNCORA da curva de skill — a season (banda) e o roster (skill por piloto) chamam o
/// MESMO valor pra a forma e o cap da cauda baterem dos dois lados.
fn ai_sweet_spot(tier: u8, track_id: Option<i64>, base_dir: &std::path::Path, custid: i64) -> i64 {
    let track_offset = track_id.map(track_skill_offset).unwrap_or(0);
    let profile = load_adaptive_profile(base_dir, custid);
    let adapt_track = track_id.map(|id| profile.track_delta(id)).unwrap_or(0);
    // Boost global amortecido por tier (não infla as divisões baixas — ver tier_difficulty_damp).
    let global_eff = (profile.global as f64 * tier_difficulty_damp(tier)).round() as i64;
    // Rookie (tier 0) rebaixa o sweet spot um degrau FIXO, seja qual for a pista.
    let rookie_discount = if tier == 0 { ROOKIE_DIFFICULTY_DISCOUNT } else { 0 };
    (tier_difficulty_base(tier) + track_offset + global_eff + adapt_track - rookie_discount).clamp(0, 125)
}

/// Vantagens de carro (car-perf) do CAMPO e do JOGADOR na pista alvo, para a inversão
/// carro→dificuldade (Sistema de Nível do Carro). Mapeia cada piloto de IA → time → carro
/// (`team_car`); carro ausente ou rookie spec → vantagem 0. Devolve
/// `(vantagem_do_jogador, vantagens_da_ia, mapa piloto→vantagem)`. Cache por time (os
/// companheiros dividem o mesmo carro). Usado pela season (banda) e pelo roster (banda +
/// spread) com a MESMA fonte, pra os dois lados baterem sob o esticão do iRacing.
fn field_car_advantages(
    conn: &rusqlite::Connection,
    categoria: &str,
    player_team_id: Option<&str>,
    track_id: i64,
) -> (f64, Vec<f64>, std::collections::HashMap<String, f64>) {
    use crate::car::sim_bridge::car_advantage;
    use crate::db::queries::{contracts as cq, drivers as dq, team_car as tcq};
    use crate::simulation::track_profile::get_track_simulation_data;
    use std::collections::HashMap;

    let tsd = get_track_simulation_data(track_id as u32);
    let track = (
        tsd.acceleration_weight,
        tsd.power_weight,
        tsd.handling_weight,
    );

    let load = |team_id: &str, cache: &mut HashMap<String, f64>| -> f64 {
        if let Some(v) = cache.get(team_id) {
            return *v;
        }
        let v = tcq::get_team_car(conn, team_id)
            .ok()
            .flatten()
            .map(|car| car_advantage(&car, track))
            .unwrap_or(0.0);
        cache.insert(team_id.to_string(), v);
        v
    };

    let mut cache: HashMap<String, f64> = HashMap::new();
    let player_adv = player_team_id.map(|t| load(t, &mut cache)).unwrap_or(0.0);

    let mut ai_advs = Vec::new();
    let mut per_ai = HashMap::new();
    for d in dq::get_drivers_by_category(conn, categoria).unwrap_or_default() {
        if d.is_jogador {
            continue;
        }
        let team = cq::get_active_contract_for_pilot(conn, &d.id)
            .ok()
            .flatten()
            .map(|c| c.equipe_id);
        let adv = team.as_deref().map(|t| load(t, &mut cache)).unwrap_or(0.0);
        ai_advs.push(adv);
        per_ai.insert(d.id, adv);
    }
    (player_adv, ai_advs, per_ai)
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
        // mesmo valor nos dois layouts livres. (o Short duplicado 542 foi removido do catálogo.)
        166 | 167 => 7,
        // Oran Park Raceway (202 GP 2,6 km / 208 South 2,0 km): sweet spot 74 — quase
        // baseline, IA já competitiva com pouco skill (igual Rudskogen). Mesmo valor nos 2.
        202 | 208 => 1,
        // Oulton Park - International (180, 4.4 km) + variações da Intl: 183 w/out Hislop,
        // 184 w/out Brittens, 185 w/no Chicanes. Sweet spot 79 nos 4 layouts livres da
        // família Intl. (a Intl duplicada 342 foi removida; Fosters/Island não são variação da Intl.)
        180 | 183 | 184 | 185 => 6,
        // Oulton Park - Fosters (181), Island (182): layouts não-Intl do mesmo venue.
        // User mandou herdar o valor do Oulton (sweet spot 79) sem teste à parte.
        181 | 182 => 6,
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
    // TESTE: força a PRÓXIMA corrida pendente como molhada (chuva forte).
    force_wet: Option<bool>,
) -> Result<SeasonGenResult, String> {
    use crate::config::app_config::AppConfig;
    use crate::constants::categories::get_category_config;
    use crate::constants::tracks::{free_or_substitute, get_track};
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
    // Mapa evento→corrida da carreira, NA MESMA ORDEM dos eventos escritos no
    // aiseason. É o "post-it" que o import usa para achar o resultado certo:
    // events[i] no JSON ↔ event_race_map[i] = (race_id, track_id).
    let mut event_race_map: Vec<(String, i64)> = Vec::new();
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
    // Modo TESTE (zerado): força o roteiro da 1ª corrida na etapa 1 (mesmo que a carreira
    // já tenha avançado) e omite resultados. Corridas 2+ ficam com o clima variado normal.
    let test_blank = test_blank.unwrap_or(false);
    let first_race_id = entries
        .iter()
        .min_by_key(|e| e.rodada)
        .map(|e| e.id.clone());
    // TESTE: força a PRÓXIMA corrida pendente (não concluída) como molhada.
    let force_wet = force_wet.unwrap_or(false);
    let next_pending_id = entries
        .iter()
        .filter(|e| !matches!(e.status, crate::models::enums::RaceStatus::Concluida))
        .min_by_key(|e| e.rodada)
        .map(|e| e.id.clone());

    // Clima + horário gerados por pista+estação (determinístico por etapa). Guarda a
    // história de cada pista para a penalidade da chuva na banda.
    let mut stories: std::collections::HashMap<i64, crate::iracing_sdk::weather::WeatherStory> =
        std::collections::HashMap::new();
    let mut substituted = 0;
    for entry in &entries {
        // Fallback de TESTE: se a pista do calendário é conteúdo PAGO (que o jogador
        // pode não possuir), roda numa pista GRÁTIS no lugar — o iRacing só carrega
        // pistas que o jogador tem. Pista já grátis passa intacta. A banda de skill /
        // sweet spot continua ancorada no `entry.track_id` ORIGINAL (alinhada com o
        // roster); só o que o iRacing carrega (EventInput/import) vira a free.
        let Some(track) = free_or_substitute(entry.track_id) else {
            continue;
        };
        {
            if track.track_id != entry.track_id {
                substituted += 1;
            }
                let is_first = (career_first_race && entry.week_of_year == first_week)
                    || (test_blank && first_race_id.as_deref() == Some(entry.id.as_str()));
                let wet_here = force_wet && next_pending_id.as_deref() == Some(entry.id.as_str());
                // Etapa noturna designada pelo calendário (≥1 corrida de noite/temporada).
                let night_here = crate::calendar::is_night_horario(&entry.horario);
                let seed = event_seed(&career_id, &entry.id);
                let (ew, story) = build_event_weather(
                    track,
                    entry.week_of_year,
                    season.ano,
                    cat.tier,
                    custid,
                    seed,
                    is_first,
                    race_end,
                    wet_here,
                    night_here,
                );
                // FONTE ÚNICA: persiste clima E temperatura desta MESMA história, pra a
                // UI e a simulação offline baterem com o que o iRacing vai rodar (e a
                // temp nunca destoar da chuva real).
                let wc = story_to_weather_condition(&story);
                let _ = db.conn.execute(
                    "UPDATE calendar SET clima = ?1, temperatura = ?2 WHERE id = ?3",
                    rusqlite::params![wc.as_str(), ew.temp_c as f64, entry.id],
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
                    // Pista EFETIVA que o iRacing carrega (a free substituta, quando a
                    // original é paga). Nenhuma pista free é oval de verdade — Roval
                    // (Charlotte) é ROAD no iRacing (paceCar road, sem largada lançada).
                    track_id: track.track_id as i64,
                    is_oval: false,
                    event_id: uuid::Uuid::new_v4().to_string(),
                    weather: ew,
                    results,
                });
                // Guarda a pista EFETIVA no post-it: o import compara o resultado do
                // iRacing contra o que foi de fato exportado (não contra a original paga).
                event_race_map.push((entry.id.clone(), track.track_id as i64));
        }
    }
    let _ = substituted; // (contagem de substituições — reservado p/ UI/log futuro)
    if events.is_empty() {
        return Err(format!(
            "Calendário da categoria '{categoria}' está vazio — nada para exportar."
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
    // Sweet spot do tier na pista alvo (âncora da curva; MESMO valor que o roster usa).
    // Perfil adaptativo por custid entra aqui dentro.
    let base_sweet = ai_sweet_spot(cat.tier, resolved_track_id, &base_dir, custid);
    // Sistema de Nível do Carro → dificuldade: rebaixa/eleva a BANDA inteira pela vantagem do
    // SEU carro vs a média do campo na pista alvo (o spread por-IA vai no roster). MESMO
    // cálculo que o roster usa, pra os dois baterem sob o esticão do iRacing.
    let player_team_id = dq::get_player_driver(&db.conn)
        .ok()
        .and_then(|p| {
            crate::db::queries::contracts::get_active_contract_for_pilot(&db.conn, &p.id)
                .ok()
                .flatten()
        })
        .map(|c| c.equipe_id);
    let car_band = resolved_track_id
        .map(|tid| {
            let (player_adv, ai_advs, _) =
                field_car_advantages(&db.conn, &categoria, player_team_id.as_deref(), tid);
            crate::iracing_sdk::car_difficulty::band_skill_delta(player_adv, &ai_advs)
        })
        .unwrap_or(0.0);
    let max_skill = ((base_sweet as f64 + car_band).round() as i64).clamp(0, 125);
    // Piso da banda pela CURVA DE 2 TRECHOS (ver roster_gen::skill_curve): o melhor da IA
    // vira max_skill (frente fiel/competitiva); o PIOR aterrissa onde a cauda o joga — mas
    // NUNCA abaixo do skill real dele (cap da cauda). No rookie (grid apertado) o fundo
    // afunda de propósito; no GT3 (grid largo) o cap segura o pior no próprio skill real.
    // O roster escreve a MESMA forma por piloto; a banda re-ancora no sweet spot.
    let min_skill = if skills.is_empty() {
        (max_skill - 25).max(0)
    } else {
        let curve = roster_gen::skill_curve_from(&skills, max_skill as f64);
        (roster_gen::skill_curve(curve.lo, &curve).round() as i64).clamp(0, max_skill)
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
        wind_kmh: 10,
        wind_dir_deg: 0,
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
        start_time: format!("{}-06-01T16:00:00", sim_safe_year(season.ano)),
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

    // POST-IT do import: qual arquivo de aiseason e qual evento corresponde a cada
    // corrida da carreira. Sobrescrito a cada export → sempre aponta para o
    // campeonato atual. (Opção "Guardar" — exata, sem varrer/adivinhar.)
    let pointer = serde_json::json!({
        "aiseason_file": path.to_string_lossy(),
        "events": event_race_map
            .iter()
            .map(|(rid, tid)| serde_json::json!({ "race_id": rid, "track_id": tid }))
            .collect::<Vec<_>>(),
    });
    if let Some(ppath) = season_pointer_path(&base_dir, &career_id) {
        if let Some(parent) = ppath.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&ppath, pointer.to_string());
    }

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

/// Uma linha da tabela de referência do teste de chuva (skill por cenário).
#[derive(serde::Serialize)]
pub struct RainTestRow {
    pub name: String,
    pub base_skill: i64,
    pub fator_chuva: i64,
    pub skill_seco: i64,
    pub skill_molhado: i64,
    pub skill_tempestade: i64,
}

#[derive(serde::Serialize)]
pub struct RainTestResult {
    /// Nomes das 3 seasons criadas (carregar no iRacing).
    pub seasons: Vec<String>,
    /// 16 pilotos com o skill efetivo em cada cenário (referência do esperado).
    pub rows: Vec<RainTestRow>,
}

/// TESTE DE CHUVA: gera 3 seasons (Seco / Molhado / Tempestade), cada uma com seu
/// próprio roster de 16 pilotos controlados (4 skills × 4 fatores de chuva), onde o
/// `driver_skill` já entra RE-RANKEADO pela penalidade de chuva POR PILOTO
/// (`rain_skill_penalty`). Valida que bons-de-chuva sobem no molhado/tempestade.
/// Como a IA do iRacing não tem atributo de chuva e o roster é fixo por season, cada
/// clima precisa do seu export — daí as 3 seasons.
#[tauri::command]
pub fn iracing_export_rain_test() -> Result<RainTestResult, String> {
    use crate::iracing_sdk::weather::{RainIntensity, Season, WeatherScenario, WeatherStory};
    use crate::iracing_sdk::{paths, roster_gen, season_gen, weather};

    let car = roster_gen::car_spec("mx5").ok_or("carro mx5 não encontrado")?;
    // Matriz: 4 níveis de skill × 4 níveis de fator_chuva. Cor do carro por skill (pra
    // achar na pista); o resultado vem pelo NOME ("Bom-Mestre" etc.).
    let skills = [
        ("Ruim", 45.0, "888888"),
        ("Mediano", 62.0, "2255FF"),
        ("Bom", 80.0, "22CC44"),
        ("Alien", 94.0, "FF2222"),
    ];
    let rains = [
        ("Pessimo", 10.0),
        ("Mediano", 50.0),
        ("Bom", 78.0),
        ("Mestre", 100.0),
    ];
    // (rótulo, cenário de clima, intensidade da corrida, molhada?)
    let scenarios = [
        (
            "Seco",
            WeatherScenario::FirstRaceScript,
            RainIntensity::None,
            false,
        ),
        (
            "Molhado",
            WeatherScenario::SteadyRain,
            RainIntensity::Decent,
            true,
        ),
        (
            "Tempestade",
            WeatherScenario::SteadyRain,
            RainIntensity::VeryHeavy,
            true,
        ),
    ];

    let track_id = 516; // Navarra (road, conteúdo grátis)
    let start_time = "2031-06-15T16:00:00".to_string();
    let race_end: i64 = 15;

    let airosters = paths::airosters_dir()
        .ok_or("Não foi possível localizar a pasta airosters do iRacing.")?;
    let aiseasons = paths::aiseasons_dir()
        .ok_or("Não foi possível localizar a pasta aiseasons do iRacing.")?;

    let mut seasons = Vec::new();
    for (sc_label, scenario, intensity, is_wet) in scenarios {
        // Roster: driver_skill = base − penalidade(fator, intensidade) deste cenário.
        let mut drivers = Vec::new();
        let mut row = 0i64;
        for (_sk_label, base, color) in skills {
            for (rn_label, fator) in rains {
                let _ = rn_label;
                let pen = weather::rain_skill_penalty(fator, intensity);
                let skill = (base - pen as f64).clamp(1.0, 100.0).round() as i64;
                let name = format!("{}-{}", skills_label(base), rains_label(fator));
                let design = format!("0,{color},111111,FFFFFF");
                drivers.push(roster_gen::RosterDriver {
                    driver_name: name,
                    car_number: (row + 1).to_string(),
                    car_design: design.clone(),
                    suit_design: design.clone(),
                    helmet_design: design,
                    car_path: car.car_path.to_string(),
                    car_id: car.car_id,
                    car_class_id: car.car_class_id,
                    sponsor1: car.sponsors[0],
                    sponsor2: car.sponsors.get(1).copied().unwrap_or(car.sponsors[0]),
                    number_design: "0,0,FFFFFF,777777,000000".to_string(),
                    driver_skill: skill,
                    driver_aggression: 50,
                    driver_optimism: 50,
                    driver_smoothness: 50,
                    pit_crew_skill: 50,
                    strategy_riskiness: 50,
                    driver_age: 28,
                    id: uuid::Uuid::new_v4().to_string(),
                    row_index: row,
                });
                row += 1;
            }
        }
        let roster = roster_gen::RosterFile { drivers };
        let roster_name = format!("Teste Chuva - {sc_label}");
        let safe: String = roster_name
            .chars()
            .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
            .collect();

        let rdir = airosters.join(&safe);
        std::fs::create_dir_all(&rdir).map_err(|e| format!("Falha ao criar pasta roster: {e}"))?;
        let rjson = serde_json::to_string_pretty(&roster)
            .map_err(|e| format!("Falha ao serializar roster: {e}"))?;
        std::fs::write(rdir.join("roster.json"), rjson)
            .map_err(|e| format!("Falha ao gravar roster: {e}"))?;

        // Clima do cenário via o modelo calibrado.
        let story = WeatherStory {
            scenario,
            is_wet_race: is_wet,
            race_intensity: intensity,
            qualy_intensity: RainIntensity::None,
            season: Season::Summer,
            tendency: 0.0,
        };
        let profile = weather::story_to_profile(&story, race_end);
        let wind = weather::generate_wind(&story, 0x5EED ^ story.race_intensity as u64);
        let ew = season_gen::EventWeather {
            skies: profile.skies,
            humidity: profile.humidity,
            temp_c: 18,
            track_water: profile.track_water,
            wind_kmh: wind.speed_kmh,
            wind_dir_deg: wind.dir_deg,
            keyframes: profile
                .keyframes
                .into_iter()
                .map(|(event_type, time_offset)| season_gen::WeatherKeyframe {
                    event_type,
                    time_offset,
                })
                .collect(),
            weather_id: format!("0_{}", uuid::Uuid::new_v4()),
            start_time: start_time.clone(),
        };
        let global = season_gen::EventWeather {
            skies: 1,
            humidity: 45,
            temp_c: 18,
            track_water: 0,
            wind_kmh: 10,
            wind_dir_deg: 0,
            keyframes: vec![season_gen::WeatherKeyframe {
                event_type: 1,
                time_offset: 0,
            }],
            weather_id: format!("0_{}", uuid::Uuid::new_v4()),
            start_time: start_time.clone(),
        };
        let params = season_gen::SeasonParams {
            roster_name: roster_name.clone(),
            name: roster_name.clone(),
            car_id: car.car_id,
            car_class_id: car.car_class_id,
            race_length_min: race_end,
            max_drivers: 16,
            min_skill: 25,
            max_skill: 95,
            year: 2031,
            global_weather: global,
            events: vec![season_gen::EventInput {
                track_id,
                is_oval: false,
                event_id: uuid::Uuid::new_v4().to_string(),
                weather: ew,
                results: None,
            }],
        };
        let season_json = season_gen::build_season(&params);
        std::fs::create_dir_all(&aiseasons)
            .map_err(|e| format!("Falha ao criar pasta aiseasons: {e}"))?;
        let sjson = serde_json::to_string_pretty(&season_json)
            .map_err(|e| format!("Falha ao serializar season: {e}"))?;
        std::fs::write(aiseasons.join(format!("{safe}.json")), sjson)
            .map_err(|e| format!("Falha ao gravar season: {e}"))?;
        seasons.push(roster_name);
    }

    // Tabela de referência (skill efetivo de cada piloto por cenário).
    let mut rows = Vec::new();
    for (_sk, base, _color) in skills {
        for (_rn, fator) in rains {
            rows.push(RainTestRow {
                name: format!("{}-{}", skills_label(base), rains_label(fator)),
                base_skill: base as i64,
                fator_chuva: fator as i64,
                skill_seco: (base - weather::rain_skill_penalty(fator, RainIntensity::None) as f64)
                    .clamp(1.0, 100.0)
                    .round() as i64,
                skill_molhado: (base
                    - weather::rain_skill_penalty(fator, RainIntensity::Decent) as f64)
                    .clamp(1.0, 100.0)
                    .round() as i64,
                skill_tempestade: (base
                    - weather::rain_skill_penalty(fator, RainIntensity::VeryHeavy) as f64)
                    .clamp(1.0, 100.0)
                    .round() as i64,
            });
        }
    }
    Ok(RainTestResult { seasons, rows })
}

fn skills_label(base: f64) -> &'static str {
    match base as i64 {
        45 => "Ruim",
        62 => "Mediano",
        80 => "Bom",
        _ => "Alien",
    }
}
fn rains_label(fator: f64) -> &'static str {
    match fator as i64 {
        10 => "Pessimo",
        50 => "Mediano",
        78 => "Bom",
        _ => "Mestre",
    }
}

/// Risco de UMA peça na previsão pré-corrida (probabilidade + nível pra UI).
#[derive(serde::Serialize)]
pub struct ForecastPartView {
    pub part: String,
    pub part_name: String,
    pub any_prob: f64,
    pub dnf_prob: f64,
    /// "baixo" | "médio" | "alto".
    pub level: String,
    /// CONSEQUÊNCIA pro jogador (o que a UI mostra em palavra + cor), derivada do que a peça
    /// custa e não da probabilidade crua: "confiavel" | "custa_tempo" | "pode_abandonar".
    pub consequencia: String,
}

/// Previsão de risco de quebra do carro do jogador pra próxima corrida (aviso pré-corrida).
#[derive(serde::Serialize)]
pub struct BreakdownForecastView {
    /// `false` se não deu pra prever (sem time/corrida/carro) — a UI esconde o card.
    pub available: bool,
    /// Risco geral de ABANDONO por quebra nesta corrida.
    pub dnf_prob: f64,
    pub overall_level: String,
    /// Peças em risco, a mais arriscada primeiro (só as relevantes, no máx. 5).
    pub parts: Vec<ForecastPartView>,
}

/// Contexto compartilhado da previsão de quebra da PRÓXIMA corrida do jogador — categoria,
/// clima, pista, seed determinística e enduro. Base tanto do card do jogador
/// ([`get_breakdown_forecast`]) quanto do aviso na tabela do campeonato
/// ([`get_grid_breakdown_risk`]). `None` quando não dá pra prever (sem time/corrida).
struct RaceBreakdownCtx {
    player_team_id: String,
    categoria: String,
    weather: crate::car::breakdown::Weather,
    track_pha: (f64, f64, f64),
    ev_seed: u64,
    is_enduro: bool,
}

fn resolve_race_breakdown_ctx(
    db: &crate::db::connection::Database,
    career_id: &str,
) -> Option<RaceBreakdownCtx> {
    use crate::constants::tracks::get_track;
    use crate::db::queries::{
        calendar as calq, contracts as cq, drivers as dq, seasons as sq, teams as tq,
    };
    use crate::iracing_sdk::weather;
    use crate::market::car_maintenance::maintenance_demand;

    // Time + categoria do jogador (a tabela do campeonato mostrada é a da categoria dele).
    let team_id = dq::get_player_driver(&db.conn)
        .ok()
        .and_then(|p| cq::get_active_contract_for_pilot(&db.conn, &p.id).ok().flatten())
        .map(|c| c.equipe_id)?;
    let team = tq::get_team_by_id(&db.conn, &team_id).ok().flatten()?;
    let categoria = team.categoria.clone();

    // Próxima corrida pendente da categoria.
    let season = sq::get_active_season(&db.conn).ok().flatten()?;
    let race = calq::get_next_race(&db.conn, &season.id, &categoria).ok().flatten()?;

    // Clima da etapa — MESMA história determinística do export/disparo vivo.
    let ev_seed = event_seed(career_id, &race.id);
    let weather = if let Some(track) = get_track(race.track_id) {
        let story = weather::generate_weather(
            month_from_week(race.week_of_year),
            track_hemisphere(track.pais),
            climate_tendency(track.rain_group),
            ev_seed,
            false,
        );
        crate::car::breakdown::Weather {
            wetness: story_to_weather_condition(&story).wetness(),
            temperature: weather::story_temperature(&story, ev_seed) as f64,
            humidity: weather::story_to_profile(&story, 60).humidity as f64,
            wind_kmh: weather::generate_wind(&story, ev_seed).speed_kmh as f64,
        }
    } else {
        crate::car::breakdown::Weather::NEUTRAL
    };
    let track_pha = maintenance_demand(&[race.track_id]);

    // Enduro (corrida longa) → o forecast reflete o DNF raro (severidade abrandada).
    let is_enduro = crate::constants::categories::get_category_config(&categoria)
        .map(|c| crate::car::breakdown::is_enduro_duration(c.duracao_corrida_min))
        .unwrap_or(false);

    Some(RaceBreakdownCtx {
        player_team_id: team_id,
        categoria,
        weather,
        track_pha,
        ev_seed,
        is_enduro,
    })
}

/// AVISO PRÉ-CORRIDA: prevê o risco de quebra do carro do JOGADOR na PRÓXIMA corrida via Monte
/// Carlo sobre o desgaste REAL do `team_car` + a pista + o clima da etapa — os MESMOS inputs do
/// disparo ao vivo. É RISCO (probabilidade), não o desfecho: não revela qual peça/volta vai
/// quebrar. Alimenta o card da Sala de Estratégia e um fato do briefing do engenheiro.
#[tauri::command]
pub fn get_breakdown_forecast(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<BreakdownForecastView, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::{team_car as tcq, teams as tq};
    use tauri::Manager;

    let none = BreakdownForecastView {
        available: false,
        dnf_prob: 0.0,
        overall_level: "baixo".to_string(),
        parts: Vec::new(),
    };

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Ok(none);
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let Some(ctx) = resolve_race_breakdown_ctx(&db, &career_id) else {
        return Ok(none);
    };
    let categoria = ctx.categoria.clone();
    let Some(team) = tq::get_team_by_id(&db.conn, &ctx.player_team_id).ok().flatten() else {
        return Ok(none);
    };
    let Some(car) = tcq::get_team_car(&db.conn, &ctx.player_team_id).ok().flatten() else {
        return Ok(none);
    };

    // 18 voltas = referência de sprint (a escala calibrada). 400 amostras dão um % estável.
    let f = crate::car::breakdown::forecast_breakdown_risk(
        &car,
        18,
        ctx.ev_seed,
        team.pit_crew_quality,
        ctx.track_pha,
        ctx.weather,
        &[],
        400,
        ctx.is_enduro,
        crate::car::cost::category_ceiling(&categoria) > 2,
    );

    let part_level = |p: f64| {
        if p < 0.08 {
            "baixo"
        } else if p < 0.20 {
            "médio"
        } else {
            "alto"
        }
    };
    // CONSEQUÊNCIA (o que a UI mostra). Limiares de calibração — ainda por afinar na pista:
    //  · "pode_abandonar" (vermelho): há risco REAL de a peça encerrar a corrida.
    //  · "custa_tempo" (laranja): penalidade pesada provável, OU tantas idas ao box que doem.
    //  · "confiavel" (verde): no máximo desgaste trivial.
    const DNF_VERMELHO: f64 = 0.03;
    const CUSTO_LARANJA: f64 = 0.08;
    const IDAS_LARANJA: f64 = 0.50;
    let consequencia = |r: &crate::car::breakdown::PartRisk| {
        if r.dnf_prob >= DNF_VERMELHO {
            "pode_abandonar"
        } else if r.costly_prob >= CUSTO_LARANJA || r.any_prob >= IDAS_LARANJA {
            "custa_tempo"
        } else {
            "confiavel"
        }
    };
    let overall_level = if f.dnf_prob < 0.05 {
        "baixo"
    } else if f.dnf_prob < 0.12 {
        "médio"
    } else {
        "alto"
    };
    // A mais perigosa primeiro (DNF > custo > idas): o topo vira o "ponto fraco" na UI.
    let mut ranked: Vec<&crate::car::breakdown::PartRisk> =
        f.parts.iter().filter(|r| r.any_prob >= 0.03).collect();
    ranked.sort_by(|a, b| {
        (b.dnf_prob, b.costly_prob, b.any_prob)
            .partial_cmp(&(a.dnf_prob, a.costly_prob, a.any_prob))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let parts = ranked
        .into_iter()
        .take(5)
        .map(|r| ForecastPartView {
            part: r.part.as_str().to_string(),
            part_name: r.part.display_name(&categoria).to_string(),
            any_prob: r.any_prob,
            dnf_prob: r.dnf_prob,
            level: part_level(r.any_prob).to_string(),
            consequencia: consequencia(r).to_string(),
        })
        .collect();

    Ok(BreakdownForecastView {
        available: true,
        dnf_prob: f.dnf_prob,
        overall_level: overall_level.to_string(),
        parts,
    })
}

/// AVISO NA TABELA DO CAMPEONATO: devolve os IDs das EQUIPES cujo carro tem risco REAL de
/// quebra na próxima corrida (penalidade pesada ou DNF — o desgaste trivial NÃO conta, senão
/// quase toda equipe acenderia). A UI marca com 🔧 os pilotos dessas equipes (ambos partilham o
/// carro). Mesmos inputs deterministas do card do jogador; menos amostras (é só sim/não).
#[tauri::command]
pub fn get_grid_breakdown_risk(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<Vec<String>, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::{team_car as tcq, teams as tq};
    use tauri::Manager;

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Ok(Vec::new());
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let Some(ctx) = resolve_race_breakdown_ctx(&db, &career_id) else {
        return Ok(Vec::new());
    };
    let teams = tq::get_teams_by_category(&db.conn, &ctx.categoria).unwrap_or_default();

    let mut risky: Vec<String> = Vec::new();
    for team in teams {
        let Some(car) = tcq::get_team_car(&db.conn, &team.id).ok().flatten() else {
            continue;
        };
        // Semente decorrelacionada por equipe (FNV-1a do id) pra os times não partilharem o
        // mesmo padrão de sorteio — a probabilidade em si já é estável com 150 amostras.
        let team_hash = team
            .id
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
                (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3)
            });
        let f = crate::car::breakdown::forecast_breakdown_risk(
            &car,
            18,
            ctx.ev_seed ^ team_hash,
            team.pit_crew_quality,
            ctx.track_pha,
            ctx.weather,
            &[],
            150,
            ctx.is_enduro,
            crate::car::cost::category_ceiling(&ctx.categoria) > 2,
        );
        // "Risco real" = mesma régua do card (peça que custa tempo de verdade ou pode abandonar);
        // o desgaste trivial (any_prob) fica de fora pra o marcador não virar ruído.
        let notable = f.dnf_prob >= 0.05
            || f
                .parts
                .iter()
                .any(|p| p.dnf_prob >= 0.03 || p.costly_prob >= 0.08);
        if notable {
            risky.push(team.id);
        }
    }

    Ok(risky)
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

/// Converte a história do clima (`generate_weather`) → `WeatherCondition` do
/// calendário/sim. FONTE ÚNICA: o mesmo gerador do export passa a alimentar o
/// `entry.clima` (UI + simulação offline batem com o iRacing). Mapa: seco→Dry,
/// Light→Damp, Decent→Wet, Heavy/VeryHeavy→HeavyRain.
fn story_to_weather_condition(
    story: &crate::iracing_sdk::weather::WeatherStory,
) -> crate::models::enums::WeatherCondition {
    use crate::iracing_sdk::weather::RainIntensity;
    use crate::models::enums::WeatherCondition as W;
    if !story.is_wet_race {
        return W::Dry;
    }
    match story.race_intensity {
        RainIntensity::Heavy | RainIntensity::VeryHeavy => W::HeavyRain,
        RainIntensity::Decent => W::Wet,
        _ => W::Damp, // Light (None não ocorre em corrida molhada)
    }
}

/// Resolve o clima de uma etapa pela FONTE ÚNICA (`generate_weather`) e PERSISTE em
/// `calendar.clima`, pra UI + sim offline baterem com o export. Devolve a condição.
pub(crate) fn resolve_and_persist_race_weather(
    conn: &rusqlite::Connection,
    career_id: &str,
    track: &crate::constants::tracks::TrackInfo,
    week_of_year: i32,
    race_id: &str,
    is_first_race: bool,
) -> crate::models::enums::WeatherCondition {
    let seed = event_seed(career_id, race_id);
    let story = crate::iracing_sdk::weather::generate_weather(
        month_from_week(week_of_year),
        track_hemisphere(track.pais),
        climate_tendency(track.rain_group),
        seed,
        is_first_race,
    );
    let wc = story_to_weather_condition(&story);
    // Temperatura alinhada à MESMA história (mesma fonte do export) → UI e sim batem.
    let temp_c = crate::iracing_sdk::weather::story_temperature(&story, seed) as f64;
    // Umidade e vento da MESMA história (o Sistema de Quebra usa: umidade amplifica o calor
    // no motor; vento estressa suspensão + asas). A umidade é constante por cenário no perfil.
    let humidity = crate::iracing_sdk::weather::story_to_profile(&story, 60).humidity as f64;
    let wind_kmh = crate::iracing_sdk::weather::generate_wind(&story, seed).speed_kmh as f64;
    let _ = conn.execute(
        "UPDATE calendar SET clima = ?1, temperatura = ?2, umidade = ?3, vento = ?4 WHERE id = ?5",
        rusqlite::params![wc.as_str(), temp_c, humidity, wind_kmh, race_id],
    );
    wc
}

/// Monta o `EventWeather` de uma etapa via o gerador de clima + horário (golden
/// hour por estação). Devolve também a `WeatherStory` (p/ a penalidade da chuva).
/// Ano SEGURO para o `simulated_start_time` do clima. O iRacing calcula sol/estação a
/// partir dessa data e ENGASGA com anos muito no futuro (a carreira pode estar em 2042+):
/// cada bloco de clima fica lento o bastante para, somado em muitas etapas, estourar o
/// watchdog de load do sim ("Simulator appeared to be unresponsive for more than 25
/// seconds"). Mapeia o ano da carreira para a janela recente [2024, 2027] preservando
/// mês/dia/hora (o que importa para estação e golden hour) e a fase de ano bissexto. Só o
/// iRacing vê o ano trocado — a carreira segue no ano real. Ver [[project_aiseason_weather_hang]].
fn sim_safe_year(year: i32) -> i32 {
    2024 + (year - 2024).rem_euclid(4)
}

fn build_event_weather(
    track: &crate::constants::tracks::TrackInfo,
    week_of_year: i32,
    year: i32,
    tier: u8,
    custid: i64,
    seed: u64,
    is_first_race: bool,
    race_end: i64,
    force_wet: bool,
    force_night: bool,
) -> (
    crate::iracing_sdk::season_gen::EventWeather,
    crate::iracing_sdk::weather::WeatherStory,
) {
    use crate::iracing_sdk::{season_gen, weather};
    let month = month_from_week(week_of_year);
    let mut story = weather::generate_weather(
        month,
        track_hemisphere(track.pais),
        climate_tendency(track.rain_group),
        seed,
        is_first_race,
    );
    // TESTE: força chuva forte nesta etapa (corrida molhada o tempo todo).
    if force_wet {
        story.is_wet_race = true;
        story.race_intensity = weather::RainIntensity::Heavy;
        story.scenario = weather::WeatherScenario::SteadyRain;
    }
    let is_lit = track.track_id == 556; // Charlotte Roval — única com iluminação
    // Etapa designada como noturna pelo calendário força a hora no escuro (sobrepõe
    // o sorteio por-pista, mas nunca em rookie — o calendário nunca designa tier 0).
    let hour = if force_night {
        weather::night_start_hour(story.season, seed ^ 0x55)
    } else {
        weather::generate_race_start_hour(story.season, tier, is_lit, seed ^ 0x55)
    };
    let profile = weather::story_to_profile(&story, race_end);
    // Temperatura ALINHADA à história de chuva (mesma fonte determinística) — nunca
    // uma temp "de chuva" numa corrida que roda seca. Presa em [18, 32] pelo gerador.
    let temp_c = weather::story_temperature(&story, seed);
    // Vento VARIÁVEL por corrida (2–48 km/h + direção).
    let wind = weather::generate_wind(&story, seed);
    // Umidade com pequeno jitter determinístico (varia por corrida, ±8), clamp [0,100].
    let hum_jitter = ((seed >> 17) % 17) as i64 - 8;
    let humidity = (profile.humidity + hum_jitter).clamp(0, 100);
    let hh = (hour.floor() as i64).clamp(0, 23);
    let mm = (((hour - hour.floor()) * 60.0).round() as i64).clamp(0, 59);
    let start_time = format!(
        "{}-{month:02}-15T{hh:02}:{mm:02}:00",
        sim_safe_year(year)
    );
    let ew = season_gen::EventWeather {
        skies: profile.skies,
        humidity,
        temp_c,
        track_water: profile.track_water,
        wind_kmh: wind.speed_kmh,
        wind_dir_deg: wind.dir_deg,
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

/// Rótulo PT do cenário de clima (para a tela do timeline).
fn scenario_label_pt(s: crate::iracing_sdk::weather::WeatherScenario) -> String {
    use crate::iracing_sdk::weather::WeatherScenario::*;
    match s {
        ClearDry => "Seco e limpo",
        Scare => "Céu fecha (sem chuva)",
        LastDrops => "Pingos no fim",
        PassingDrizzle => "Garoa passageira",
        ClearingUp => "Abrindo o tempo",
        WetQualyDryRace => "Secou para a corrida",
        SteadyRain => "Chuva constante",
        Improving => "Chuva afrouxando",
        StormArrives => "Tempestade chegando",
        PulsingStorm => "Tempestade pulsante",
        LightQualyWorseRace => "Piora na corrida",
        FirstRaceScript => "Nublado, pingos no fim",
    }
    .to_string()
}

/// Rótulo PT da intensidade da chuva.
fn intensity_label_pt(i: crate::iracing_sdk::weather::RainIntensity) -> String {
    use crate::iracing_sdk::weather::RainIntensity::*;
    match i {
        None => "Seco",
        Light => "Garoa",
        Decent => "Chuva",
        Heavy => "Chuva forte",
        VeryHeavy => "Temporal",
    }
    .to_string()
}

/// Timeline do clima de uma corrida (frações 0..1) — para a tela de clima (previsão
/// na sala de estratégia + revisão na pós-corrida). Reconstrói o MESMO clima
/// determinístico do export (pista + estação + seed), idêntico ao que a prova seguiu.
#[derive(serde::Serialize)]
pub struct RaceWeatherTimeline {
    pub scenario: String,
    pub is_wet_race: bool,
    pub intensity: String,
    pub points: Vec<crate::iracing_sdk::weather::WeatherTimelinePoint>,
}

#[tauri::command]
pub fn get_race_weather_timeline(
    app: tauri::AppHandle,
    career_id: String,
    race_id: String,
) -> Result<RaceWeatherTimeline, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use tauri::Manager;

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    build_race_weather_timeline(&db.conn, &career_id, &race_id)
}

/// Núcleo de [`get_race_weather_timeline`] sem depender do `AppHandle` — recebe a conexão
/// direto, para ser reusado pelo overlay ao vivo (torre) além da tela de clima. Reconstrói o
/// MESMO clima determinístico (pista + estação + seed) que a prova seguiu.
pub(crate) fn build_race_weather_timeline(
    conn: &rusqlite::Connection,
    career_id: &str,
    race_id: &str,
) -> Result<RaceWeatherTimeline, String> {
    let entry = crate::db::queries::calendar::get_calendar_entry_by_id(conn, race_id)
        .map_err(|e| format!("Falha ao buscar corrida: {e}"))?
        .ok_or_else(|| "Corrida não encontrada".to_string())?;
    let track = crate::constants::tracks::get_track(entry.track_id)
        .ok_or_else(|| "Pista não encontrada".to_string())?;

    // É a corrida de ESTREIA do save? (única que usa o roteiro fixo do 1º clima.)
    let first_id: Option<String> = conn
        .query_row(
            "SELECT c.id FROM calendar c JOIN seasons s ON c.season_id = s.id \
             ORDER BY s.numero ASC, c.week_of_year ASC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok();
    let is_first = first_id.as_deref() == Some(race_id);

    let story = crate::iracing_sdk::weather::generate_weather(
        month_from_week(entry.week_of_year),
        track_hemisphere(track.pais),
        climate_tendency(track.rain_group),
        event_seed(career_id, race_id),
        is_first,
    );
    Ok(RaceWeatherTimeline {
        scenario: scenario_label_pt(story.scenario),
        is_wet_race: story.is_wet_race,
        intensity: intensity_label_pt(story.race_intensity),
        points: crate::iracing_sdk::weather::story_to_timeline(&story),
    })
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

/// Chave da tabela `meta` (career.db) que guarda o custid do iRacing do jogador
/// VINCULADO a este save. Capturado uma vez (popup da 1ª corrida) e reutilizado
/// para repintar o carro automaticamente a cada troca de equipe no mercado.
const PLAYER_CUSTID_META_KEY: &str = "player_iracing_custid";

/// Mapeia a categoria da carreira no carro do iRacing (mesma regra do export).
fn car_key_for_category(categoria: &str) -> &'static str {
    let c = categoria.to_lowercase();
    if c.contains("gr86") || c.contains("toyota") {
        "gr86"
    } else if c.contains("bmw") || c.contains("m2") {
        "bmwm2"
    } else {
        "mx5" // mazda mx-5 e padrão
    }
}

/// Escreve `car_<custid>.tga` (cor sólida `hex`) na pasta de pintura do carro.
/// Núcleo compartilhado pela pintura da 1ª vez e pela repintura no mercado.
/// Recebe o `hex` já normalizado pelo chamador.
fn write_player_car_tga(car_key: &str, hex: &str, custid: i64) -> Result<(String, String), String> {
    use crate::iracing_sdk::{paint_gen, paths, roster_gen};
    let car =
        roster_gen::car_spec(car_key).ok_or_else(|| format!("Carro desconhecido: {car_key}"))?;
    let hex = roster_gen::normalize_hex(hex);
    let dir = paths::car_paint_dir(car.car_path)
        .ok_or("Não foi possível localizar a pasta de pintura do iRacing.")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    let path = dir.join(format!("car_{custid}.tga"));
    paint_gen::write_solid_tga(&path, &hex).map_err(|e| format!("Falha ao gravar pintura: {e}"))?;
    Ok((path.display().to_string(), format!("#{hex}")))
}

/// Lê o custid já capturado (sampler) ou tenta ler a sessão atual agora.
fn capture_player_custid() -> Option<i64> {
    if let Some(id) = iracing_sdk::cached_custid() {
        return Some(id);
    }
    if let Ok(session) = iracing_sdk::read_session() {
        iracing_sdk::note_session_custid(&session.session_yaml);
    }
    iracing_sdk::cached_custid()
}

/// `true` se já capturamos o custid do jogador — ou seja, ele já conectou ao iRacing
/// (SDK) ao menos uma vez. O front usa para mostrar a opção "pegar a cor do carro"
/// na Sala de Estratégia só quando faz sentido: já temos o ID para poder pintar.
#[tauri::command]
pub fn iracing_has_player_id() -> bool {
    iracing_sdk::cached_custid().is_some()
}

/// Custid do iRacing VINCULADO a este save (`None` se ainda não vinculado). O front
/// usa isto para decidir se mostra o popup de "pegar a cor do carro" na 1ª corrida.
#[tauri::command]
pub fn iracing_linked_custid(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<Option<i64>, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
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
    let val = crate::db::queries::meta::get_meta_value(&db.conn, PLAYER_CUSTID_META_KEY)
        .map_err(|e| format!("Falha ao ler meta: {e}"))?;
    Ok(val.and_then(|s| s.trim().parse::<i64>().ok()))
}

/// 1ª vez (popup da Sala de Estratégia): captura o custid do iRacing, VINCULA ao
/// save (tabela meta) e pinta o carro na cor do time atual do jogador. Erro claro
/// se o custid ainda não foi capturado (precisa abrir o iRacing uma vez).
#[tauri::command]
pub fn iracing_link_player_paint(
    app: tauri::AppHandle,
    career_id: String,
    car_key: String,
) -> Result<ApplyPaintResult, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::{contracts as cq, drivers as dq, teams as tq};
    use crate::iracing_sdk::roster_gen;
    use tauri::Manager;

    let custid = capture_player_custid().ok_or(
        "Ainda não capturei seu ID do iRacing — abra o iRacing e entre numa sessão ou pista uma vez para vincularmos.",
    )?;

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

    let (path, color) =
        write_player_car_tga(&car_key, &roster_gen::normalize_hex(&team.cor_primaria), custid)?;

    crate::db::queries::meta::put_meta_value(&db.conn, PLAYER_CUSTID_META_KEY, &custid.to_string())
        .map_err(|e| format!("Falha ao vincular o ID ao save: {e}"))?;

    Ok(ApplyPaintResult { path, custid, color })
}

/// Mercado: repinta o carro do jogador na cor da NOVA equipe ao aceitar um contrato.
/// Usa o custid vinculado ao save (ou o capturado na sessão, persistindo-o). Devolve
/// `None` silenciosamente se ainda não há custid (jamais abriu o iRacing) — o front
/// simplesmente não mostra o toast nesse caso.
#[tauri::command]
pub fn iracing_apply_market_paint(
    app: tauri::AppHandle,
    career_id: String,
    team_color: String,
    category: String,
) -> Result<Option<ApplyPaintResult>, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
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

    let linked = crate::db::queries::meta::get_meta_value(&db.conn, PLAYER_CUSTID_META_KEY)
        .map_err(|e| format!("Falha ao ler meta: {e}"))?
        .and_then(|s| s.trim().parse::<i64>().ok());
    let custid = match linked.or_else(capture_player_custid) {
        Some(id) => id,
        None => return Ok(None), // sem ID ainda → nada a fazer (silencioso)
    };
    if linked.is_none() {
        let _ = crate::db::queries::meta::put_meta_value(
            &db.conn,
            PLAYER_CUSTID_META_KEY,
            &custid.to_string(),
        );
    }

    let car_key = car_key_for_category(&category);
    let (path, color) = write_player_car_tga(car_key, &team_color, custid)?;
    Ok(Some(ApplyPaintResult { path, custid, color }))
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

/// Envia um comando de chat de TEXTO LIVRE ao iRacing (ex.: `!black #1 20`).
/// Teste do caminho parametrizado (foca a janela → abre o chat → digita + Enter),
/// sem depender de macro no `app.ini`.
#[tauri::command]
pub fn iracing_send_chat_text(text: String) -> Result<(), String> {
    iracing_sdk::send_chat_text(&text).map_err(|e| e.to_string())
}

/// DEBUG: arma uma quebra GARANTIDA no carro do jogador pra próxima volta cruzada (motor na
/// parede). Testa o disparo ao vivo ponta a ponta: ao cruzar a linha, o monitor manda o
/// `!black`/`!dq` sozinho. Requer estar numa sessão do iRacing (número do carro conhecido).
#[tauri::command]
pub fn iracing_arm_test_breakdown() -> Result<bool, String> {
    Ok(crate::iracing_sdk::race_monitor::arm_test_breakdown())
}

/// DEBUG: arma a GRADE TODA com uma peça perto de quebrar por carro. Ao longo das próximas
/// voltas, os carros vão largando peças (`!black`/`!dq`), estrangulado pra não spammar o chat.
#[tauri::command]
pub fn iracing_arm_test_breakdown_grid() -> Result<(), String> {
    crate::iracing_sdk::race_monitor::arm_test_breakdown_grid();
    Ok(())
}
