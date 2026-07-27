//! Instalação/uso de uma macro de chat do iRacing para acionar bandeira amarela
//! (`!y$`) em corridas offline contra IA.
//!
//! O SDK só dispara MACROS de chat já configuradas — não digita texto livre.
//! Então, com consentimento do usuário, o app:
//! 1. localiza o `app.ini` do iRacing;
//! 2. faz backup (uma vez);
//! 3. acha o slot `AutoChatStr` cujo texto contém "welcome" (robusto a idioma/
//!    versão; fallback para o slot 7);
//! 4. salva o valor original e troca por `!y$`;
//! 5. guarda o slot, para disparar depois pelo SDK ([`super::send_chat_macro`]).
//!
//! Reversível **à mão**: o valor original fica salvo no estado (e exposto em
//! [`status`]), e há um backup completo do `app.ini` ao lado dele. Não expomos um
//! botão de restaurar — quem quiser desfazer troca o slot de volta ou recupera o
//! backup; o iRacing também reescreve o `app.ini` ao fechar.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Texto da macro de bandeira (o `$` faz o iRacing "enviar" o chat).
const YELLOW_MACRO_TEXT: &str = "!y$";
/// O número de macro do SDK é 0-based a partir do `AutoChatStr1`, então para
/// disparar `AutoChatStrN` envia-se `N - 1` (confirmado empiricamente).
const SDK_MACRO_OFFSET: i32 = -1;
/// Slot padrão da macro "You're welcome" quando a busca por texto falha.
const FALLBACK_SLOT: i32 = 7;
/// Arquivo de estado (e o nome antigo, para migração transparente).
const STATE_FILE: &str = "loop_yellow_macro.json";
const OLD_STATE_FILE: &str = "iracerapp_yellow_macro.json";

#[derive(Serialize, Deserialize, Clone)]
struct MacroState {
    slot: i32,
    original: String,
    macro_text: String,
}

/// Estado exposto à UI.
#[derive(Serialize, Clone)]
pub struct YellowMacroStatus {
    pub app_ini_found: bool,
    pub app_ini_path: Option<String>,
    pub installed: bool,
    pub slot: Option<i32>,
    pub original: Option<String>,
    pub current_value: Option<String>,
    pub macro_text: String,
}

/// Caminhos candidatos do `app.ini`. Primeiro o caminho RESOLVIDO pela Known
/// Folder API (robusto a OneDrive/idioma/relocação); depois heurísticas por env
/// var como rede de segurança.
fn candidate_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(iracing) = super::paths::iracing_dir() {
        out.push(iracing.join("app.ini"));
    }
    if let Ok(up) = std::env::var("USERPROFILE") {
        let base = PathBuf::from(&up);
        for docs in [
            "Documents",
            "OneDrive/Documents",
            "OneDrive/Documentos",
            "Documentos",
        ] {
            out.push(base.join(docs).join("iRacing").join("app.ini"));
        }
    }
    out
}

fn find_app_ini() -> Option<PathBuf> {
    candidate_paths().into_iter().find(|p| p.is_file())
}

fn file_name_str(app_ini: &Path) -> &str {
    app_ini
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("app.ini")
}

fn backup_path(app_ini: &Path) -> PathBuf {
    app_ini.with_file_name(format!("{}.loop.bak", file_name_str(app_ini)))
}

fn old_backup_path(app_ini: &Path) -> PathBuf {
    app_ini.with_file_name(format!("{}.iracerapp.bak", file_name_str(app_ini)))
}

fn state_path(app_ini: &Path) -> PathBuf {
    app_ini.with_file_name(STATE_FILE)
}

fn old_state_path(app_ini: &Path) -> PathBuf {
    app_ini.with_file_name(OLD_STATE_FILE)
}

/// Lê o estado, aceitando o nome antigo (migração transparente).
fn read_state(app_ini: &Path) -> Option<MacroState> {
    for p in [state_path(app_ini), old_state_path(app_ini)] {
        if let Ok(s) = fs::read_to_string(&p) {
            if let Ok(st) = serde_json::from_str::<MacroState>(&s) {
                return Some(st);
            }
        }
    }
    None
}

/// A linha é `AutoChatStr{slot}=` (tolera espaços antes do `=`).
fn is_slot_line(line: &str, slot: i32) -> bool {
    line.trim_start()
        .strip_prefix(&format!("AutoChatStr{slot}"))
        .map(|rest| rest.trim_start().starts_with('='))
        .unwrap_or(false)
}

fn slot_value(content: &str, slot: i32) -> Option<String> {
    content
        .lines()
        .find(|l| is_slot_line(l, slot))
        .and_then(|l| l.split_once('=').map(|(_, v)| v.trim().to_string()))
}

/// Acha qualquer `AutoChatStrN` cujo texto contenha "welcome".
fn find_welcome_slot(content: &str) -> Option<(i32, String)> {
    for line in content.lines() {
        let trimmed = line.trim_start();
        let rest = match trimmed.strip_prefix("AutoChatStr") {
            Some(r) => r,
            None => continue,
        };
        if let Some((num, val)) = rest.split_once('=') {
            if let Ok(n) = num.trim().parse::<i32>() {
                if val.to_lowercase().contains("welcome") {
                    return Some((n, val.trim().to_string()));
                }
            }
        }
    }
    None
}

/// Reescreve o valor de um slot preservando recuo e quebra de linha (CRLF).
fn set_slot(content: &str, slot: i32, new_value: &str) -> Option<String> {
    let mut out = String::with_capacity(content.len() + 8);
    let mut replaced = false;
    for seg in content.split_inclusive('\n') {
        if !replaced && is_slot_line(seg, slot) {
            let lead_len = seg.len() - seg.trim_start().len();
            let ending = if seg.ends_with("\r\n") {
                "\r\n"
            } else if seg.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            out.push_str(&seg[..lead_len]);
            out.push_str(&format!("AutoChatStr{slot}={new_value}"));
            out.push_str(ending);
            replaced = true;
        } else {
            out.push_str(seg);
        }
    }
    replaced.then_some(out)
}

/// Migra os arquivos com nome antigo (`iracerapp_*`) para o novo (`loop_*`),
/// transparentemente, sem perder o estado/backup já criados.
fn migrate_names(app_ini: &Path) {
    let (new_s, old_s) = (state_path(app_ini), old_state_path(app_ini));
    if old_s.exists() && !new_s.exists() {
        let _ = fs::rename(&old_s, &new_s);
    }
    let (new_b, old_b) = (backup_path(app_ini), old_backup_path(app_ini));
    if old_b.exists() && !new_b.exists() {
        let _ = fs::rename(&old_b, &new_b);
    }
}

/// Estado atual da instalação.
pub fn status() -> YellowMacroStatus {
    let app_ini = find_app_ini();
    if let Some(path) = &app_ini {
        migrate_names(path);
    }
    let mut st = YellowMacroStatus {
        app_ini_found: app_ini.is_some(),
        app_ini_path: app_ini.as_ref().map(|p| p.display().to_string()),
        installed: false,
        slot: None,
        original: None,
        current_value: None,
        macro_text: YELLOW_MACRO_TEXT.to_string(),
    };
    if let Some(path) = &app_ini {
        if let Some(state) = read_state(path) {
            st.slot = Some(state.slot);
            st.original = Some(state.original.clone());
            if let Ok(content) = fs::read_to_string(path) {
                let cur = slot_value(&content, state.slot);
                st.installed = cur.as_deref() == Some(state.macro_text.as_str());
                st.current_value = cur;
            }
        }
    }
    st
}

/// Instala a macro de bandeira (com backup). Idempotente.
pub fn install() -> Result<YellowMacroStatus, String> {
    let app_ini =
        find_app_ini().ok_or("app.ini do iRacing não encontrado (Documents/iRacing/app.ini).")?;
    let content = fs::read_to_string(&app_ini).map_err(|e| format!("Falha ao ler app.ini: {e}"))?;

    // Backup completo, preservando o original verdadeiro. Se já houver backup
    // com o nome antigo, migra (renomeia) em vez de gravar o arquivo já alterado.
    let bak = backup_path(&app_ini);
    if !bak.exists() {
        let old_bak = old_backup_path(&app_ini);
        if old_bak.exists() {
            let _ = fs::rename(&old_bak, &bak);
        } else {
            fs::write(&bak, &content).map_err(|e| format!("Falha ao criar backup: {e}"))?;
        }
    }

    // Se já instalado, mantém o original salvo; senão acha o slot/valor.
    let (slot, original) = if let Some(st) = read_state(&app_ini) {
        (st.slot, st.original)
    } else if let Some((n, val)) = find_welcome_slot(&content) {
        (n, val)
    } else {
        let val = slot_value(&content, FALLBACK_SLOT).ok_or(
            "Não achei macro com 'welcome' nem o slot 7. Abra as macros de chat no iRacing uma vez.",
        )?;
        (FALLBACK_SLOT, val)
    };

    let new_content = set_slot(&content, slot, YELLOW_MACRO_TEXT)
        .ok_or(format!("Slot AutoChatStr{slot} não encontrado no app.ini."))?;
    fs::write(&app_ini, &new_content).map_err(|e| format!("Falha ao escrever app.ini: {e}"))?;

    let state = MacroState {
        slot,
        original,
        macro_text: YELLOW_MACRO_TEXT.to_string(),
    };
    fs::write(
        state_path(&app_ini),
        serde_json::to_string_pretty(&state).unwrap_or_default(),
    )
    .map_err(|e| format!("Falha ao salvar estado: {e}"))?;
    // Migração: remove o arquivo de estado com o nome antigo.
    let _ = fs::remove_file(old_state_path(&app_ini));

    Ok(status())
}

/// Dispara a macro instalada (aciona a bandeira amarela no iRacing).
pub fn throw_yellow() -> Result<(), String> {
    let app_ini = find_app_ini().ok_or("app.ini não encontrado.")?;
    let state = read_state(&app_ini).ok_or("Macro de bandeira não instalada.")?;
    super::send_chat_macro(state.slot + SDK_MACRO_OFFSET).map_err(|e| e.to_string())
}
