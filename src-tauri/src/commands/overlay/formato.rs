//! Diagnóstico, formatação e chaves de ordenação da torre de tempos.

use std::collections::HashMap;

use crate::iracing_sdk::{race_monitor, tire_strategy, CarSnapshot};

/// Diagnóstico do overlay: grava o ESTADO atual em `%TEMP%\iracer_overlay_data.log`,
/// só quando o estado MUDA (o front chama ~1×/s; sem flood). Diz exatamente em qual
/// porta o `get_overlay_data` está saindo (telemetria/cars/cruzamento/sucesso).
pub(crate) fn dbg_state(state: &str) {
    use std::io::Write;
    use std::sync::Mutex;
    static LAST: Mutex<Option<String>> = Mutex::new(None);
    {
        let mut last = match LAST.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if last.as_deref() == Some(state) {
            return;
        }
        *last = Some(state.to_string());
    }
    let mut path = std::env::temp_dir();
    path.push("iracer_overlay_data.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(f, "[{secs}] {state}");
    }
}

pub(crate) fn fmt_lap(secs: f64) -> String {
    if secs <= 0.0 {
        return String::new();
    }
    let total_ms = (secs * 1000.0).round() as i64;
    let m = total_ms / 60_000;
    let s = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{:02}:{:02}.{:03}", m, s, ms)
}

pub(crate) fn wetness_to_condition(w: i32) -> &'static str {
    match w {
        n if n >= 4 => "rain",  // LightlyWet+
        1 => "clear",           // Dry
        _ => "clouds",          // MostlyDry / desconhecido
    }
}

pub(crate) fn compound_str(c: tire_strategy::Compound) -> &'static str {
    match c {
        tire_strategy::Compound::Wet => "wet",
        _ => "dry",
    }
}

/// Mapa `SessionNum -> SessionType` (ex.: "Race", "Open Qualify", "Practice") do
/// YAML. Em cada bloco de sessão o `SessionNum` vem antes do `SessionType`.
pub(crate) fn parse_session_types(yaml: &str) -> HashMap<i32, String> {
    let mut map = HashMap::new();
    let mut cur: Option<i32> = None;
    for line in yaml.lines() {
        let l = line.trim();
        if let Some(v) = l.strip_prefix("SessionNum:") {
            cur = v.trim().parse::<i32>().ok();
        } else if let Some(v) = l.strip_prefix("SessionType:") {
            if let Some(n) = cur {
                map.insert(n, v.trim().to_string());
            }
        }
    }
    map
}

/// "Race"/"Qualify"/"Practice" -> "R"/"Q"/"P".
pub(crate) fn session_kind(session_types: &HashMap<i32, String>, session_num: i32) -> &'static str {
    match session_types.get(&session_num).map(|s| s.as_str()) {
        Some(s) if s.contains("Qualify") => "Q",
        Some(s) if s.contains("Practice") || s.contains("Warmup") => "P",
        _ => "R",
    }
}

pub(crate) fn tower_order_key(pos: i32, unclassified: &(i64, i64)) -> (bool, i64, i64) {
    if pos > 0 {
        (false, i64::from(pos), 0)
    } else {
        (true, unclassified.0, unclassified.1)
    }
}

pub(crate) fn best_positive_lap(live: Option<f64>, recorded: Option<f64>) -> f64 {
    match (
        live.filter(|secs| *secs > 0.0),
        recorded.filter(|secs| *secs > 0.0),
    ) {
        (Some(live), Some(recorded)) => live.min(recorded),
        (Some(live), None) => live,
        (None, Some(recorded)) => recorded,
        (None, None) => 0.0,
    }
}

/// O histórico capturado é da MESMA sessão que está rolando agora?
///
/// Igualdade simples — cobre os dois mundos:
///   • ONLINE/hosted: `WeekendInfo:SubSessionID` > 0, único por evento.
///   • OFFLINE (aiseason de IA — o caso do jogo): o iRacing manda SubSessionID = **0**.
///
/// Antes exigíamos `> 0`, então offline caía em `0 == 0` → **false**, e o overlay descartava
/// TODO o histórico ao vivo: sem `grid_by_idx` (delta sempre "— 0") e sem `tire_by_idx`
/// (a coluna de pneu nunca acumulava as paradas). O histórico já é resetado por
/// tentativa/`session_num` (`record_history`), então confiar no id igual — inclusive 0 —
/// é seguro contra dados de uma corrida anterior.
pub(crate) fn history_matches_subsession(history_id: i64, current_id: i64) -> bool {
    history_id == current_id
}

/// Cor usada quando o time do piloto não foi resolvido (sem contrato / sem carreira).
pub(crate) const NEUTRAL_TEAM_COLOR: &str = "#7d8590";

/// Chave de comparação de nome para o join de RESERVA entre o `UserName` que o SDK
/// devolve e o nosso elenco. Como somos NÓS que exportamos o roster da IA, o nome do
/// piloto no iRacing é literalmente o `nome` do nosso piloto — só normalizamos pontas
/// e caixa para o casamento não depender de formatação.
pub(crate) fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
}

pub(crate) fn roster_with_telemetry<'a>(
    roster: &'a [race_monitor::YamlCarMeta],
    telemetry: &'a [CarSnapshot],
) -> Vec<(&'a race_monitor::YamlCarMeta, Option<&'a CarSnapshot>)> {
    let telemetry_by_idx: HashMap<i32, &CarSnapshot> =
        telemetry.iter().map(|car| (car.idx, car)).collect();

    roster
        .iter()
        .filter(|meta| !meta.is_pace)
        .map(|meta| (meta, telemetry_by_idx.get(&meta.idx).copied()))
        .collect()
}
