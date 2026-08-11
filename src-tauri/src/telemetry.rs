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
//! 1. **Nunca bloqueia.** Todo envio sai numa thread própria, e QUALQUER erro
//!    morre lá dentro. Já levamos esse tiro uma vez: um `fetch` síncrono de 45s
//!    travou a thread principal do Tauri. Aqui o jogo não pode nem piscar se o
//!    servidor estiver fora do ar.
//! 2. **Nunca fala se o jogador não deixou.** `ENABLED` é lido antes de tudo.
//! 3. **Nunca inventa evento.** Início e fim vêm das bordas que o
//!    `race_monitor` JÁ detecta pra outros fins.
//! 4. **Nunca perde em silêncio.** Todo desfecho de envio vira uma linha no
//!    `diagnostico`, e o que não entrou fica numa fila em disco para a próxima
//!    abertura. Ver "Entrega" no fim do arquivo.
//!
//! ## Por que a entrega precisa de rede-de-segurança
//!
//! Medido em 08/08/2026, numa instalação de teste: cinco corridas dirigidas no
//! iRacing, com consentimento ligado, produziram **um** `race_start` e nenhum
//! `race_end` no servidor. O timeout era de 5s contra um Cloud Run com
//! scale-to-zero, cujo cold start passa de 20s (é por isso que o cliente de
//! notícias já usava 45s). E como o resultado do POST era descartado sem log,
//! não havia como saber se o evento saiu e falhou ou se nunca foi disparado.
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
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[path = "telemetry/entrega.rs"]
mod entrega;
#[path = "telemetry/fila.rs"]
pub(crate) mod fila;
#[path = "telemetry/uso.rs"]
mod uso;

pub use fila::drenar_fila;
pub use uso::{uso_enviar_pendente, uso_ptt_aperto, uso_ptt_pergunta, uso_tela, uso_virar_rodada};

/// Teto da fila em disco. Um evento tem ~400 bytes, então 200 é da ordem de 80 KB
/// no pior caso. Quem estoura isso está offline há semanas, e aí o que interessa é
/// o começo da fila, não o fim.
pub(crate) const FILA_MAX_EVENTOS: usize = 200;

/// Idade máxima na fila. Passou disso, o evento não descreve mais nada de útil e
/// só atrapalharia a leitura do servidor.
pub(crate) const FILA_MAX_IDADE_SECS: i64 = 7 * 24 * 60 * 60;

/// Fracassos seguidos que abortam a drenagem. Servidor fora do ar é servidor fora
/// do ar: insistir nos 200 seguintes só queima tempo e o resto volta pra fila.
pub(crate) const FILA_FRACASSOS_ATE_DESISTIR: u32 = 3;

/// Rota da telemetria de produto, no MESMO host do resto (boletins, PTT, log). O host
/// vem de `narrative::client`, que já guarda o segredo desta porta.
pub(crate) const ENDPOINT: &str =
    concat!(crate::narrative::client::host_do_servidor!(), "/telemetry");

/// Ping de vida da corrida aberta. Ver o cabeçalho: 2× a corrida mais curta
/// (15 min), então uma corrida normal termina antes de pingar uma única vez.
const PING_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// O mesmo teto do cliente de notícias, e pelo mesmo motivo: o Cloud Run dorme, e
/// a primeira chamada depois de um tempo parado paga o cold start (subir o
/// container + init do Firestore) antes de responder qualquer coisa. Isto roda numa
/// thread de fundo que não segura nada, então esperar aqui não custa ao jogo.
pub(crate) const TIMEOUT_SECS: u64 = 20;

/// Espera entre as tentativas de um MESMO evento, em segundos. Três tentativas no
/// total. O cold start acontece uma vez: quando a primeira paga a subida do
/// container, a segunda costuma achá-lo de pé.
pub(crate) const ESPERAS_SECS: [u64; 2] = [3, 12];

/// Abaixo disso o `atraso_s` não vai no payload. O evento leva alguns segundos até
/// ser aceito no caso normal, e carimbar isso em todo mundo só polui.
pub(crate) const ATRASO_MINIMO_SECS: i64 = 30;

static INSTALL_ID: OnceLock<String> = OnceLock::new();
/// UUID de UMA abertura do app. Viaja em todo evento e é o que permite ao servidor
/// perguntar "quantas corridas por sessão" e "quantas sessões por dia" sem inventar
/// heurística de janela de tempo: dois eventos com o mesmo `session_id` são, por
/// definição, a mesma vez que a pessoa abriu o Loop.
static SESSION_ID: OnceLock<String> = OnceLock::new();
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
    /// Ano DA CARREIRA (1, 2, 3…), o número da temporada ativa. Nunca o ano do
    /// calendário: 2026 não diz nada sobre onde a pessoa está na progressão, e duas
    /// carreiras começadas em anos diferentes ficariam incomparáveis por nada.
    ano_carreira: i32,
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
    /// Reinícios da sessão de CORRIDA. Contados por sessão no `race_monitor`, e não
    /// pelo número da tentativa: a tentativa também nasce a cada troca de sessão do fim
    /// de semana, e por isso um fim de semana limpo reportava dois reinícios.
    pub restarts: i32,
    /// Reinícios da CLASSIFICAÇÃO. Separado porque responde outra pergunta: refazer a
    /// quali é o jogador caçando uma volta boa, refazer a corrida é ele fugindo de um
    /// resultado. Somados num campo só, os dois viram um número que não decide nada.
    pub restarts_quali: i32,
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
pub fn init(base_dir: &Path, install_id: String, enabled: bool) {
    fila::definir_arquivo(base_dir.join("telemetria-fila.jsonl"));
    uso::definir_arquivo(base_dir.join("telemetria-uso.json"));
    let _ = INSTALL_ID.set(install_id);
    let _ = SESSION_ID.set(uuid::Uuid::new_v4().to_string());
    ENABLED.store(enabled, Ordering::Relaxed);
    if !enabled {
        // Quem abriu com a telemetria desligada não tem fila nem acumulado para
        // guardar. Inclui quem recusou DEPOIS de já ter enfileirado algo.
        fila::apagar();
        uso::apagar();
    }
}

/// O app abriu. Um evento por abertura, e é dele que sai a contagem de sessões:
/// quantas vezes a pessoa abriu o Loop, em que dias, e (cruzando pelo `session_id`)
/// quantas corridas ela fez em cada uma.
///
/// Sai depois do `init()` e antes de qualquer borda de corrida poder disparar.
pub fn app_start() {
    if !is_enabled() {
        return;
    }
    send("app_start", json!({}));
}

/// Consentimento mudou nas Configurações. Desligar fecha a corrida aberta em
/// silêncio (sem mandar o `race_end`): o jogador pediu pra parar de falar, e
/// "parar de falar" inclui não mandar o evento de despedida. O servidor expira
/// a corrida órfã sozinho pela janela de 35 min.
///
/// Desligar também **apaga a fila**. Um evento pendente é uma fala já engatilhada;
/// guardá-lo para mandar depois seria continuar falando pelas costas de quem
/// acabou de pedir silêncio.
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
    if !enabled {
        HAS_ACTIVE.store(false, Ordering::Relaxed);
        if let Ok(mut active) = ACTIVE.lock() {
            *active = None;
        }
        fila::apagar();
        uso::apagar();
    }
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Identificador da instalação, para o envio de log casar o relato com o resto
/// dos eventos daquela máquina. `None` antes do `init()` do boot.
pub fn install_id() -> Option<String> {
    INSTALL_ID.get().cloned()
}

/// Contexto da carreira aberta, anexado a todo evento. Chamado quando uma
/// carreira é carregada e quando a temporada vira.
pub fn set_career_context(
    ano_carreira: i32,
    categoria: String,
    dificuldade: String,
    temporadas_completas: i32,
    corridas_totais: i32,
) {
    if let Ok(mut career) = CAREER.lock() {
        *career = Some(CareerContext {
            ano_carreira,
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
    // Registrada ANTES do gate de consentimento, de propósito. Sem esta linha não
    // se distingue "a borda de largada nunca fechou" de "fechou e o envio se
    // perdeu", que foi a dúvida que sobrou do teste de 08/08/2026. É uma linha por
    // largada, longe de qualquer caminho quente.
    crate::diagnostico::linha(
        "telemetria",
        &format!(
            "borda de largada (subsessao {subsession_id}), consentimento {}",
            if is_enabled() { "ligado" } else { "desligado" }
        ),
    );
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

/// Fim de semana simulado DENTRO do app, sem passar pelo iRacing.
///
/// Existe para responder uma pergunta que nenhum outro evento responde: **quanta
/// gente simula em vez de correr**. Até aqui a telemetria só enxergava quem abre o
/// sim, e uma temporada inteira jogada dentro do app não produzia um único evento.
///
/// Manda o MÍNIMO de propósito: o fato de ter simulado, mais o contexto de carreira
/// que já viaja em todo evento. Nada da corrida em si — nem pista, nem posição, nem
/// resultado. Quem quiser saber como foi a corrida está olhando a pergunta errada;
/// aqui a unidade de medida é a pessoa, não a prova.
pub fn race_simulated() {
    if !is_enabled() {
        return;
    }
    send("race_sim", json!({}));
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
        // Só interessa quando havia corrida aberta: `sim_closed` chega em toda queda
        // de conexão, inclusive nas mil vezes em que ninguém estava correndo.
        if HAS_ACTIVE.load(Ordering::Relaxed) {
            crate::diagnostico::linha(
                "telemetria",
                &format!("fim de corrida ({status}) com o consentimento desligado"),
            );
        }
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
        put_i("restarts_quali", o.restarts_quali);
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

/// Dispara e esquece. Uma thread por evento (são ~3 por corrida). NADA aqui pode
/// propagar pro chamador: as bordas que chamam isso rodam com o lock do
/// `race_monitor` na mão.
///
/// O corpo é montado AQUI, na thread do chamador, e não lá dentro: o contexto de
/// carreira precisa ser o do instante do evento, e a fila precisa poder guardar o
/// corpo pronto.
fn send(event: &str, payload: serde_json::Value) {
    let Some(install_id) = INSTALL_ID.get().cloned() else {
        return;
    };
    let career = CAREER.lock().ok().and_then(|c| c.clone());

    let mut body = json!({
        "event": event,
        "install_id": install_id,
        "session_id": SESSION_ID.get().cloned().unwrap_or_default(),
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
            map.insert("ano_carreira".into(), json!(c.ano_carreira));
            map.insert("categoria".into(), json!(c.categoria));
            map.insert("dificuldade".into(), json!(c.dificuldade));
            map.insert("temporadas_completas".into(), json!(c.temporadas_completas));
            map.insert("corridas_totais".into(), json!(c.corridas_totais));
        }
    }

    let criado_em = agora();
    std::thread::spawn(move || {
        if !entrega::entregar(&body, criado_em, &ESPERAS_SECS) {
            fila::enfileirar(body, criado_em);
        }
    });
}

/// Segundos desde a época. Só serve para medir DURAÇÃO (o `atraso_s` e a poda da
/// fila), nunca para datar o evento no servidor.
pub(crate) fn agora() -> i64 {
    chrono::Utc::now().timestamp()
}
