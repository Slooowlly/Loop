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

#[path = "iracing/roster.rs"]
mod roster;
pub use roster::*;

#[path = "iracing/importacao.rs"]
mod importacao;
pub use importacao::*;

#[path = "iracing/resultado.rs"]
mod resultado;
pub use resultado::*;

#[path = "iracing/adaptativo.rs"]
mod adaptativo;
pub use adaptativo::*;
