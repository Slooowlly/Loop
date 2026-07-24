//! Telemetria de produto — ANÔNIMA, agregada, opt-out.
//!
//! Responde perguntas de produto que o save local não responde: quantas corridas
//! estão rolando agora, em que temporada/categoria cada instalação está, onde o
//! jogador para de jogar. NUNCA carrega dado pessoal: a chave é o `install_id`
//! (UUID gerado na máquina, sem vínculo com e-mail/conta), e nada de nome de
//! piloto, nome de equipe ou conteúdo de save sai daqui.
//!
//! ## Regras de ouro
//!
//! 1. **Nunca bloqueia.** Todo envio sai numa thread própria, com timeout curto,
//!    e QUALQUER erro é engolido. Já levamos esse tiro uma vez: um `fetch`
//!    síncrono de 45s travou a thread principal do Tauri. Aqui o jogo não pode
//!    nem piscar se o servidor estiver fora do ar.
//! 2. **Nunca fala se o jogador não deixou.** `ENABLED` é lido antes de tudo.
//! 3. **Nunca inventa evento.** Início e fim vêm das bordas que o
//!    `race_monitor` JÁ detecta pra outros fins.
//!
//! ## Ciclo de vida de uma corrida
//!
//! - `race_start` — borda de largada verde (`prev_session_state < RACING` →
//!   `>= RACING`). É um UPSERT por `subsession_id` no servidor: se o jogador dá
//!   restart, a mesma corrida é reaberta em vez de virar duas.
//! - `race_ping` — a cada 30 min de corrida aberta. Só existe pra impedir
//!   corrida-fantasma (PC desligado no meio, `race_end` nunca chega). A corrida
//!   mais curta do jogo é de 15 min, então o caso comum NUNCA pinga: começa e
//!   acaba antes do primeiro. Enduro de 2h pinga 3 vezes.
//! - `race_end` — `finalize_attempt()` (bandeirada/DNF) ou queda da conexão.
//!
//! O servidor conta "rolando agora" como `start` sem `end` e visto há < 35 min.

use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::narrative::client::APP_SECRET;

const ENDPOINT: &str = "https://iracer-news-124606451488.southamerica-east1.run.app/telemetry";

/// Ping de vida da corrida aberta. Ver o cabeçalho: 2× a corrida mais curta
/// (15 min), então uma corrida normal termina antes de pingar uma única vez.
const PING_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// Curto de propósito. Telemetria que demora é telemetria que atrapalha; se o
/// servidor está frio (Cloud Run faz scale-to-zero), o evento se perde e tudo
/// bem — nenhuma decisão de produto depende de um evento isolado.
const TIMEOUT_SECS: u64 = 5;

static INSTALL_ID: OnceLock<String> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(false);
/// Atalho pra `maybe_ping()` não pagar nem um `try_lock` no caso comum (99,99%
/// dos ticks do sampler não têm nada a fazer). O sampler roda a 60 Hz.
static HAS_ACTIVE: AtomicBool = AtomicBool::new(false);
static ACTIVE: Mutex<Option<ActiveRace>> = Mutex::new(None);
/// Contexto da carreira carregada (temporada/categoria). Vive fora do
/// `ActiveRace` porque quem sabe disso é a camada de comandos, não o sampler.
static CAREER: Mutex<Option<CareerContext>> = Mutex::new(None);

struct ActiveRace {
    subsession_id: i64,
    track_id: i64,
    started: Instant,
    last_ping: Instant,
}

#[derive(Clone)]
struct CareerContext {
    ano: i32,
    categoria: String,
    dificuldade: String,
    temporadas_completas: i32,
    corridas_totais: i32,
}

/// Desfecho de uma corrida, anexado ao `race_end`. Tudo já é calculado pelo
/// `race_monitor` para o painel pós-corrida — aqui é só cópia.
///
/// Campos numéricos usam 0 (ou 0.0) para "desconhecido" e são OMITIDOS do payload:
/// mandar zero faria o servidor achar que o jogador largou da posição zero e fez uma
/// volta de zero segundo. Melhor um campo ausente que um número mentiroso.
///
/// Por que três campos de posição em vez de um: quinto entre 8 não é quinto entre 24,
/// e quinto partindo de décimo segundo não é quinto partindo da pole. Sozinha, a
/// posição final produz uma média que mistura tudo.
#[derive(Default)]
pub struct RaceOutcome {
    pub posicao_final: i32,
    pub posicao_grid: i32,
    pub carros_na_classe: i32,
    /// Melhor volta do jogador, em segundos.
    pub melhor_volta_s: f64,
    /// Melhor volta da CLASSE do jogador. A razão entre as duas é o déficit de ritmo —
    /// o sinal de dificuldade imune a tamanho de grid, incidente e abandono.
    pub melhor_volta_classe_s: f64,
    pub voltas: i32,
    pub incidentes: i32,
    pub restarts: i32,
    pub off_track: bool,
    pub towed: bool,
    pub garage: bool,
    pub black_flag: bool,
    pub disqualified: bool,
    pub pior_batida: Option<String>,
    pub carro: Option<String>,
}

// ── Ligar / desligar ────────────────────────────────────────────────────────

/// Chamado UMA vez no boot (`setup()`), antes do `start_watching()`. O sampler
/// roda numa thread de fundo sem `AppHandle`, então o `install_id` precisa
/// estar num estático antes de qualquer borda poder disparar.
pub fn init(install_id: String, enabled: bool) {
    let _ = INSTALL_ID.set(install_id);
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// Consentimento mudou nas Configurações. Desligar fecha a corrida aberta em
/// silêncio (sem mandar o `race_end`): o jogador pediu pra parar de falar, e
/// "parar de falar" inclui não mandar o evento de despedida. O servidor expira
/// a corrida órfã sozinho pela janela de 35 min.
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
    if !enabled {
        HAS_ACTIVE.store(false, Ordering::Relaxed);
        if let Ok(mut active) = ACTIVE.lock() {
            *active = None;
        }
    }
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Contexto da carreira aberta, anexado a todo evento. Chamado quando uma
/// carreira é carregada e quando a temporada vira.
pub fn set_career_context(
    ano: i32,
    categoria: String,
    dificuldade: String,
    temporadas_completas: i32,
    corridas_totais: i32,
) {
    if let Ok(mut career) = CAREER.lock() {
        *career = Some(CareerContext {
            ano,
            categoria,
            dificuldade,
            temporadas_completas,
            corridas_totais,
        });
    }
}

// ── Eventos ─────────────────────────────────────────────────────────────────

/// Largada verde. UPSERT por `subsession_id`: se já havia uma corrida aberta
/// com o MESMO id (restart), só renova o relógio em vez de abrir outra.
pub fn race_start(subsession_id: i64, track_id: i64) {
    if !is_enabled() {
        return;
    }
    let now = Instant::now();
    let Ok(mut active) = ACTIVE.lock() else {
        return;
    };

    // Corrida diferente abriu sem a anterior ter fechado (sim trocou de sessão
    // sem passar por bandeirada nem desconexão): fecha a antiga por segurança.
    if let Some(prev) = active.as_ref() {
        if prev.subsession_id != subsession_id {
            send(
                "race_end",
                json!({
                    "subsession_id": prev.subsession_id,
                    "duracao_s": prev.started.elapsed().as_secs(),
                    "status": "superseded",
                }),
            );
        }
    }

    *active = Some(ActiveRace {
        subsession_id,
        track_id,
        started: now,
        last_ping: now,
    });
    HAS_ACTIVE.store(true, Ordering::Relaxed);

    send(
        "race_start",
        json!({ "subsession_id": subsession_id, "track_id": track_id }),
    );
}

/// Chamado a CADA tick do sampler (60 Hz conectado). Precisa ser praticamente
/// grátis: dois loads atômicos no caso comum, e um `try_lock` (nunca `lock`,
/// pra não segurar a amostragem) quando há corrida aberta.
pub fn maybe_ping() {
    if !HAS_ACTIVE.load(Ordering::Relaxed) || !is_enabled() {
        return;
    }
    let Ok(mut guard) = ACTIVE.try_lock() else {
        return;
    };
    let Some(race) = guard.as_mut() else {
        return;
    };
    if race.last_ping.elapsed() < PING_INTERVAL {
        return;
    }
    race.last_ping = Instant::now();
    send(
        "race_ping",
        json!({
            "subsession_id": race.subsession_id,
            "elapsed_s": race.started.elapsed().as_secs(),
        }),
    );
}

/// Fim de corrida. `status` vem pronto do `finalize_attempt()`
/// (`finished` | `dnf` | `not_started`) ou é `sim_closed` na queda da conexão.
/// No-op se não houver corrida aberta — a queda de conexão chama isso sempre,
/// mesmo quando não havia tentativa ativa.
pub fn race_end(status: &str, outcome: Option<RaceOutcome>) {
    if !is_enabled() {
        return;
    }
    let Ok(mut guard) = ACTIVE.lock() else {
        return;
    };
    let Some(race) = guard.take() else {
        return;
    };
    HAS_ACTIVE.store(false, Ordering::Relaxed);

    let mut payload = json!({
        "subsession_id": race.subsession_id,
        "track_id": race.track_id,
        "duracao_s": race.started.elapsed().as_secs(),
        "status": status,
    });

    if let (Some(o), Some(map)) = (outcome, payload.as_object_mut()) {
        // Só o que tem valor real entra (ver o doc da struct).
        let mut put_i = |k: &str, v: i32| {
            if v > 0 {
                map.insert(k.into(), json!(v));
            }
        };
        put_i("posicao_final", o.posicao_final);
        put_i("posicao_grid", o.posicao_grid);
        put_i("carros_na_classe", o.carros_na_classe);
        put_i("voltas", o.voltas);
        put_i("restarts", o.restarts);
        // Incidentes: 0 é um valor LEGÍTIMO (corrida limpa) e o mais interessante
        // deles, então este não passa pelo filtro de "> 0".
        map.insert("incidentes".into(), json!(o.incidentes.max(0)));
        for (k, v) in [
            ("melhor_volta_s", o.melhor_volta_s),
            ("melhor_volta_classe_s", o.melhor_volta_classe_s),
        ] {
            if v > 0.0 && v.is_finite() {
                map.insert(k.into(), json!((v * 1000.0).round() / 1000.0));
            }
        }
        for (k, v) in [
            ("off_track", o.off_track),
            ("towed", o.towed),
            ("garage", o.garage),
            ("black_flag", o.black_flag),
            ("disqualified", o.disqualified),
        ] {
            if v {
                map.insert(k.into(), json!(true));
            }
        }
        if let Some(c) = o.pior_batida {
            map.insert("pior_batida".into(), json!(c));
        }
        if let Some(c) = o.carro {
            map.insert("carro".into(), json!(c));
        }
    }

    send("race_end", payload);
}

// ── Envio ───────────────────────────────────────────────────────────────────

/// Dispara e esquece. Uma thread por evento (são ~3 por corrida), timeout
/// curto, todo erro engolido. NADA aqui pode propagar pro chamador: as bordas
/// que chamam isso rodam com o lock do `race_monitor` na mão.
fn send(event: &str, payload: serde_json::Value) {
    let Some(install_id) = INSTALL_ID.get().cloned() else {
        return;
    };
    let event = event.to_string();
    let career = CAREER.lock().ok().and_then(|c| c.clone());

    std::thread::spawn(move || {
        let mut body = json!({
            "event": event,
            "install_id": install_id,
            "app_version": env!("CARGO_PKG_VERSION"),
            "os": std::env::consts::OS,
        });
        if let Some(map) = body.as_object_mut() {
            if let Some(fields) = payload.as_object() {
                for (k, v) in fields {
                    map.insert(k.clone(), v.clone());
                }
            }
            if let Some(c) = career {
                map.insert("ano".into(), json!(c.ano));
                map.insert("categoria".into(), json!(c.categoria));
                map.insert("dificuldade".into(), json!(c.dificuldade));
                map.insert("temporadas_completas".into(), json!(c.temporadas_completas));
                map.insert("corridas_totais".into(), json!(c.corridas_totais));
            }
        }

        let Ok(client) = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .build()
        else {
            return;
        };
        let _ = client
            .post(ENDPOINT)
            .header("x-app-secret", APP_SECRET)
            .json(&body)
            .send();
    });
}
