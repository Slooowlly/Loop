//! Gravador de corrida (DEBUG — só pra calibração local, NÃO é fluxo comercial).
//!
//! Despeja num arquivo JSONL a telemetria CRUA do iRacing (subamostrada) + o YAML da sessão
//! + um resumo do histórico acumulado no fim. É a fonte-verdade pra rebalancear o app com
//! dados REAIS de pista (desgaste/estilo/quebra/clima/posições) sem inventar número.
//!
//! Ligado sob demanda por um botão de debug nas Configurações. O amostrador do monitor
//! (~60 Hz) chama `record_session`/`record_frame` a cada tick conectado. Gravamos a TAXA
//! CHEIA (~60 Hz) — os eventos de <100 ms do estilo de pilotagem (toque no limitador, RPM
//! da troca, pico de frenagem) importam pra calibração, e o app já processa a 60 Hz. Só
//! pulamos frames de tempo CONGELADO (pausa/menu/replay parado) pra não duplicar.
//!
//! Formato: uma linha JSON por registro, com `kind`:
//!   • `header`  — versão + timestamp de início.
//!   • `session` — o `session_yaml` inteiro (uma vez): pista, elenco, carro, redline, composto…
//!   • `frame`   — `{ kind, tele }` com a `IracingTelemetry` inteira (todos os `CarIdx*`, inputs,
//!                 clima, flags, estado). É o que dá pra recomputar QUALQUER coisa offline.
//!   • `history` — no fim, o `RaceHistory` (voltas, paradas, posições, clima, incidentes).

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use super::IracingTelemetry;

static ENABLED: AtomicBool = AtomicBool::new(false);
static FRAMES: AtomicU64 = AtomicU64::new(0);

/// A cada quantos frames dar flush no disco (segurança contra crash/fechar o sim sem parar).
/// A ~60 Hz, 60 = ~1 flush/s.
const FLUSH_EVERY: u64 = 60;

struct Capture {
    writer: BufWriter<File>,
    path: PathBuf,
    last_t: f64,
    yaml_written: bool,
}

fn state() -> &'static Mutex<Option<Capture>> {
    static S: OnceLock<Mutex<Option<Capture>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// Começa a gravar num arquivo novo dentro de `dir` (criada se não existir). Devolve o caminho.
pub fn start(dir: PathBuf) -> Result<PathBuf, String> {
    fs::create_dir_all(&dir).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("race_{stamp}.jsonl"));
    let file = File::create(&path).map_err(|e| format!("Falha ao criar arquivo: {e}"))?;
    let mut cap = Capture {
        writer: BufWriter::new(file),
        path: path.clone(),
        last_t: -1.0,
        yaml_written: false,
    };
    let _ = writeln!(
        cap.writer,
        "{}",
        serde_json::json!({ "kind": "header", "version": 1, "stamp": stamp })
    );
    let _ = cap.writer.flush();
    *state().lock().map_err(|e| e.to_string())? = Some(cap);
    FRAMES.store(0, Ordering::Relaxed);
    ENABLED.store(true, Ordering::Relaxed);
    Ok(path)
}

/// Grava o YAML da sessão (uma vez): pista, elenco, carro, redline, composto etc.
pub fn record_session(yaml: &str) {
    if !ENABLED.load(Ordering::Relaxed) || yaml.is_empty() {
        return;
    }
    if let Ok(mut g) = state().lock() {
        if let Some(cap) = g.as_mut() {
            if !cap.yaml_written {
                let _ = writeln!(cap.writer, "{}", serde_json::json!({ "kind": "session", "yaml": yaml }));
                cap.yaml_written = true;
            }
        }
    }
}

/// Grava um frame de telemetria à TAXA CHEIA do sampler (~60 Hz). Pula só quando o tempo de
/// sessão NÃO avançou (pausa/menu/replay parado) pra não duplicar frames idênticos.
pub fn record_frame(t: &IracingTelemetry) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    if let Ok(mut g) = state().lock() {
        if let Some(cap) = g.as_mut() {
            if cap.last_t >= 0.0 && t.session_time <= cap.last_t {
                return;
            }
            cap.last_t = t.session_time;
            if let Ok(line) = serde_json::to_string(&serde_json::json!({ "kind": "frame", "tele": t })) {
                let _ = writeln!(cap.writer, "{line}");
                let n = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
                if n % FLUSH_EVERY == 0 {
                    let _ = cap.writer.flush();
                }
            }
        }
    }
}

/// Anexa um bloco livre (ex.: o resumo do histórico no fim). Só grava se um arquivo está aberto.
pub fn record_block(kind: &str, value: serde_json::Value) {
    if let Ok(mut g) = state().lock() {
        if let Some(cap) = g.as_mut() {
            let _ = writeln!(cap.writer, "{}", serde_json::json!({ "kind": kind, "data": value }));
        }
    }
}

/// Para a gravação, fecha o arquivo e devolve o caminho (None se não havia gravação).
pub fn stop() -> Option<PathBuf> {
    ENABLED.store(false, Ordering::Relaxed);
    let mut g = state().lock().ok()?;
    let mut cap = g.take()?;
    let _ = cap.writer.flush();
    Some(cap.path)
}

/// Está gravando agora?
pub fn is_active() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Quantos frames já foram gravados nesta sessão de captura.
pub fn frame_count() -> u64 {
    FRAMES.load(Ordering::Relaxed)
}
