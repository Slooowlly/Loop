//! AVISO PESSOAL (peça do jogador na zona de risco) + banner de chat bloqueado.

use super::demo::demo_player_warnings;
use super::tipos::BreakdownMessage;
use crate::iracing_sdk::race_monitor;

// ───────────────────── AVISO PESSOAL (peça do jogador na zona de risco) ─────────────────────

/// Peça com preposição na 2ª pessoa ("no seu motor", "na sua suspensão") — pro engenheiro.
fn part_com_seu(part_key: &str) -> &'static str {
    match part_key {
        "engine" => "no seu motor",
        "gearbox" => "no seu câmbio",
        "brakes" => "nos seus freios",
        "suspension" => "na sua suspensão",
        "cooling" => "no seu arrefecimento",
        "front_wing" => "na sua asa dianteira",
        "rear_wing" => "na sua asa traseira",
        "sidepods" => "nas suas laterais",
        "underbody" => "no seu assoalho",
        "chassis" => "no seu chassi",
        "electronics" => "na sua parte elétrica",
        _ => "no seu carro",
    }
}

/// Monta o AVISO pessoal na VOZ DO ENGENHEIRO (rádio, 1ª pessoa dele → você). Sem número:
/// ele "ouve algo estranho" e alerta que a peça pode dar problema a qualquer momento. 3
/// variações; severidade "warn" → card distinto no front.
pub(crate) fn player_warning_msg(part_key: &str, id: usize) -> BreakdownMessage {
    let onde = part_com_seu(part_key);
    // (abertura, alerta) — a peça (`onde`) é encaixada na abertura.
    let variants: [(&str, &str); 3] = [
        (
            "Estou ouvindo algo estranho",
            "pode dar problema a qualquer momento",
        ),
        (
            "Não gostei de um barulho",
            "fica de olho — pode falhar a qualquer hora",
        ),
        (
            "Tem algo esquisito acontecendo",
            "risco de pane a qualquer momento",
        ),
    ];
    let (open, detail) = variants[id % variants.len()];
    BreakdownMessage {
        id,
        severity: "warn".to_string(),
        text: format!("{open} {onde}"),
        detail: detail.to_string(),
    }
}

/// AVISOS pessoais do jogador (peça DELE entrou na zona de risco) — o overlay mostra num card
/// DISTINTO (2ª pessoa). Lê o log vivo do monitor. No demo, cicla o tour pelo tempo.
#[tauri::command]
pub fn get_player_warnings() -> Result<Vec<BreakdownMessage>, String> {
    if crate::commands::overlay_window::demo_enabled() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let tour = demo_player_warnings();
        if tour.is_empty() {
            return Ok(Vec::new());
        }
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let step = (secs / 6) as usize; // troca a cada 6 s
        let mut m = tour[step % tour.len()].clone();
        m.id = step;
        return Ok(vec![m]);
    }
    let warns = race_monitor::peek_player_warnings();
    Ok(warns
        .iter()
        .enumerate()
        .map(|(i, w)| player_warning_msg(w.part, i))
        .collect())
}

/// `true` se algum comando de quebra (`!black`/`!dq`) falhou em chegar ao iRacing nesta
/// corrida (janela não encontrada / foreground recusado). É estado LATCH por corrida — não
/// um stream de eventos —, então o overlay o consome como banner booleano persistente
/// (canal separado dos avisos de peça, que são stream por id) para não mascará-los.
#[tauri::command]
pub fn iracing_chat_blocked() -> bool {
    race_monitor::chat_send_blocked()
}
