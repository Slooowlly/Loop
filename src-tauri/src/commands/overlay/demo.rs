//! Modo DEMO do rádio: tours de exemplo montados a partir da fonte única de frases.

use std::collections::HashMap;

use super::avisos::player_warning_msg;
use super::radio::{breakdown_frases, dnf_frase, part_com_artigo};
use super::tipos::BreakdownMessage;
use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::db::queries::drivers as dq;
use crate::iracing_sdk::race_monitor;

/// Causa de exemplo por peça (só pro DEMO — na corrida real vem do `problem_label`).
fn demo_causa(part_key: &str) -> &'static str {
    match part_key {
        "engine" => "superaquecimento",
        "gearbox" => "engrenagem gasta",
        "brakes" => "freios superaquecidos",
        "suspension" => "braço trincado",
        "cooling" => "vazamento de água",
        "front_wing" => "toque na dianteira",
        "rear_wing" => "flap solto",
        "sidepods" => "batida na lateral",
        "underbody" => "assoalho danificado",
        "chassis" => "dano estrutural",
        "electronics" => "falha no chicote",
        _ => "",
    }
}

/// Nomes REAIS do grid da sessão atual (roster do monitor → número → nosso elenco), pra
/// o demo usar pilotos de verdade. `None`/vazio quando não há sessão/save → o tour cai
/// nos nomes fictícios.
pub(crate) fn session_driver_names(app: &tauri::AppHandle, career_id: &str) -> Vec<String> {
    use tauri::Manager;
    let Ok(base_dir) = app.path().app_data_dir() else {
        return Vec::new();
    };
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let Ok(db) = Database::open_existing(&db_path) else {
        return Vec::new();
    };
    let numbers: HashMap<String, i64> =
        std::fs::read_to_string(crate::commands::iracing::numbers_path(&base_dir, career_id))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
    let by_number: HashMap<i64, String> = numbers.into_iter().map(|(id, n)| (n, id)).collect();

    let feedback = race_monitor::get_feedback();
    let mut names = Vec::new();
    for meta in &feedback.cars_yaml_meta {
        if meta.is_pace {
            continue;
        }
        if let Some(id) = by_number.get(&(meta.car_number as i64)) {
            if let Ok(d) = dq::get_driver(&db.conn, id) {
                names.push(d.nome);
            }
        }
    }
    names
}

/// Monta o TOUR de exemplos do demo a partir da fonte única (`breakdown_frases`): toda
/// peça × leve/grave × 3 redações + alguns abandonos. Usa `real_names` (pilotos do grid)
/// se houver; senão, nomes fictícios. É o que o overlay cicla no modo demo.
fn demo_tour(real_names: &[String]) -> Vec<BreakdownMessage> {
    const FALLBACK: [&str; 6] = [
        "Ryan Jones",
        "Hunter Jackson",
        "Logan Garcia",
        "Adrian Alvarez",
        "Nathan Brown",
        "Camila Monteiro",
    ];
    const PARTS: [&str; 11] = [
        "engine",
        "gearbox",
        "brakes",
        "suspension",
        "cooling",
        "front_wing",
        "rear_wing",
        "sidepods",
        "underbody",
        "chassis",
        "electronics",
    ];
    let name_at = |i: usize| -> String {
        if real_names.is_empty() {
            FALLBACK[i % FALLBACK.len()].to_string()
        } else {
            real_names[i % real_names.len()].clone()
        }
    };
    let mut out = Vec::new();
    let mut ni = 0usize;
    for part in PARTS {
        for sev in ["light", "heavy"] {
            for frase in breakdown_frases(part, sev) {
                let name = name_at(ni);
                ni += 1;
                out.push(BreakdownMessage {
                    id: out.len(),
                    severity: sev.to_string(),
                    text: format!("{name} {frase}"),
                    detail: demo_causa(part).to_string(),
                    // O tour do demo é para POSICIONAR o overlay, não para ouvi-lo: as frases
                    // ciclam a cada segundo e o áudio viraria uma sobreposição contínua.
                    pecas: Vec::new(),
                });
            }
        }
    }
    for v in 0..3 {
        let name = name_at(ni);
        ni += 1;
        out.push(BreakdownMessage {
            id: out.len(),
            severity: "dnf".to_string(),
            text: dnf_frase(&name, part_com_artigo("gearbox"), v),
            detail: String::new(),
            pecas: Vec::new(),
        });
    }
    out
}

/// Lista completa de exemplos do demo (a janela do rádio busca uma vez e cicla localmente).
/// Usa os pilotos REAIS do grid da carreira quando há sessão; senão, nomes fictícios.
#[tauri::command]
pub fn overlay_demo_messages(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<Vec<BreakdownMessage>, String> {
    Ok(demo_tour(&session_driver_names(&app, &career_id)))
}

/// Modo DEMO do rádio (VR / fallback): cicla o tour pelo tempo. Troca a cada ~5 s; o `id`
/// cresce, então o front reconhece como novo. Ligado pelo botão das Configurações ou env.
pub(crate) fn demo_breakdown_feed(real_names: &[String]) -> Vec<BreakdownMessage> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let tour = demo_tour(real_names);
    if tour.is_empty() {
        return Vec::new();
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let step = (secs / 5) as usize; // avança um exemplo a cada 5 s
    let mut m = tour[step % tour.len()].clone();
    m.id = step; // id crescente → o front troca o card
    vec![m]
}

/// Tour de avisos do demo: um por peça (desgaste 95–100%). Fonte única do card de aviso.
pub(crate) fn demo_player_warnings() -> Vec<BreakdownMessage> {
    const PARTS: [&str; 11] = [
        "engine",
        "gearbox",
        "brakes",
        "suspension",
        "cooling",
        "front_wing",
        "rear_wing",
        "sidepods",
        "underbody",
        "chassis",
        "electronics",
    ];
    PARTS
        .iter()
        .enumerate()
        .map(|(i, p)| player_warning_msg(p, i))
        .collect()
}

/// Lista completa dos avisos de exemplo (a janela do rádio busca uma vez e cicla localmente).
#[tauri::command]
pub fn overlay_demo_warnings() -> Vec<BreakdownMessage> {
    demo_player_warnings()
}
