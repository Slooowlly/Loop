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

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::narrative::client::APP_SECRET;

const ENDPOINT: &str = "https://iracer-news-124606451488.southamerica-east1.run.app/telemetry";

/// Ping de vida da corrida aberta. Ver o cabeçalho: 2× a corrida mais curta
/// (15 min), então uma corrida normal termina antes de pingar uma única vez.
const PING_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// O mesmo teto do cliente de notícias, e pelo mesmo motivo: o Cloud Run dorme, e
/// a primeira chamada depois de um tempo parado paga o cold start (subir o
/// container + init do Firestore) antes de responder qualquer coisa. Isto roda numa
/// thread de fundo que não segura nada, então esperar aqui não custa ao jogo.
const TIMEOUT_SECS: u64 = 20;

/// Espera entre as tentativas de um MESMO evento, em segundos. Três tentativas no
/// total. O cold start acontece uma vez: quando a primeira paga a subida do
/// container, a segunda costuma achá-lo de pé.
const ESPERAS_SECS: [u64; 2] = [3, 12];

/// Teto da fila em disco. Um evento tem ~400 bytes, então 200 é da ordem de 80 KB
/// no pior caso. Quem estoura isso está offline há semanas, e aí o que interessa é
/// o começo da fila, não o fim.
const FILA_MAX_EVENTOS: usize = 200;

/// Idade máxima na fila. Passou disso, o evento não descreve mais nada de útil e
/// só atrapalharia a leitura do servidor.
const FILA_MAX_IDADE_SECS: i64 = 7 * 24 * 60 * 60;

/// Fracassos seguidos que abortam a drenagem. Servidor fora do ar é servidor fora
/// do ar: insistir nos 200 seguintes só queima tempo e o resto volta pra fila.
const FILA_FRACASSOS_ATE_DESISTIR: u32 = 3;

/// Abaixo disso o `atraso_s` não vai no payload. O evento leva alguns segundos até
/// ser aceito no caso normal, e carimbar isso em todo mundo só polui.
const ATRASO_MINIMO_SECS: i64 = 30;

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
/// Arquivo da fila de reenvio. `None` antes do `init()` do boot — sem ele a
/// telemetria continua funcionando, só sem rede-de-segurança.
static FILA: OnceLock<PathBuf> = OnceLock::new();
/// Serializa leitura e escrita da fila: cada envio tem sua própria thread, e a
/// drenagem do boot é mais uma.
static FILA_TRAVA: Mutex<()> = Mutex::new(());

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
    let _ = FILA.set(base_dir.join("telemetria-fila.jsonl"));
    let _ = USO.set(base_dir.join("telemetria-uso.json"));
    let _ = INSTALL_ID.set(install_id);
    let _ = SESSION_ID.set(uuid::Uuid::new_v4().to_string());
    ENABLED.store(enabled, Ordering::Relaxed);
    if !enabled {
        // Quem abriu com a telemetria desligada não tem fila nem acumulado para
        // guardar. Inclui quem recusou DEPOIS de já ter enfileirado algo.
        apagar_fila();
        apagar_uso();
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
        apagar_fila();
        apagar_uso();
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
        if !entregar(&body, criado_em, &ESPERAS_SECS) {
            enfileirar(body, criado_em);
        }
    });
}

/// Desfecho de um POST, na granularidade que decide o que fazer em seguida.
enum Resultado {
    Entregue(u16),
    /// O servidor entendeu e recusou (corpo malformado, segredo errado). Repetir
    /// não conserta, e enfileirar só encheria o disco com um evento natimorto.
    Recusado(u16),
    /// Rede, timeout ou 5xx. Vale tentar de novo.
    Falhou(String),
}

/// Entrega um evento, com até `esperas.len() + 1` tentativas. Devolve `true`
/// quando o assunto está resolvido — entregue OU recusado em definitivo.
///
/// `criado_em` é o carimbo de quando o evento NASCEU. Ele não vai no payload como
/// data (quem carimba a hora é o servidor, porque o relógio da máquina do jogador
/// não é confiável); vira o campo `atraso_s`, que é a informação de que o servidor
/// precisa para não tratar um evento drenado de ontem como se fosse de agora.
fn entregar(corpo: &serde_json::Value, criado_em: i64, esperas: &[u64]) -> bool {
    let evento = corpo
        .get("event")
        .and_then(|e| e.as_str())
        .unwrap_or("?")
        .to_string();

    let Ok(cliente) = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
    else {
        crate::diagnostico::linha("telemetria", &format!("{evento}: sem cliente HTTP"));
        return false;
    };

    let mut corpo = corpo.clone();
    for tentativa in 0..=esperas.len() {
        // Recalculado a cada tentativa: entre a primeira e a última passam ~35s, e
        // na drenagem podem ter passado dias.
        let atraso = (agora() - criado_em).max(0);
        if let Some(map) = corpo.as_object_mut() {
            if atraso >= ATRASO_MINIMO_SECS {
                map.insert("atraso_s".into(), json!(atraso));
            }
        }

        match postar(&cliente, &corpo) {
            Resultado::Entregue(status) => {
                let sufixo = if tentativa > 0 {
                    format!(" na tentativa {}", tentativa + 1)
                } else {
                    String::new()
                };
                crate::diagnostico::linha(
                    "telemetria",
                    &format!("{evento}: entregue ({status}){sufixo}, atraso {atraso}s"),
                );
                return true;
            }
            Resultado::Recusado(status) => {
                crate::diagnostico::linha(
                    "telemetria",
                    &format!("{evento}: recusado pelo servidor ({status}), descartado"),
                );
                return true;
            }
            Resultado::Falhou(motivo) => {
                crate::diagnostico::linha(
                    "telemetria",
                    &format!("{evento}: falhou ({motivo}) na tentativa {}", tentativa + 1),
                );
                if let Some(espera) = esperas.get(tentativa) {
                    std::thread::sleep(Duration::from_secs(*espera));
                }
            }
        }
    }
    false
}

fn postar(cliente: &reqwest::blocking::Client, corpo: &serde_json::Value) -> Resultado {
    match cliente
        .post(ENDPOINT)
        .header("x-app-secret", APP_SECRET)
        .json(corpo)
        .send()
    {
        Ok(resposta) => {
            let status = resposta.status();
            let codigo = status.as_u16();
            if status.is_success() {
                // 202 com `dropped: daily_cap` cai aqui de propósito: o servidor
                // recebeu e decidiu não guardar. Reenviar não muda a decisão dele.
                Resultado::Entregue(codigo)
            } else if status.is_client_error() && codigo != 408 && codigo != 429 {
                Resultado::Recusado(codigo)
            } else {
                Resultado::Falhou(format!("HTTP {codigo}"))
            }
        }
        Err(e) if e.is_timeout() => Resultado::Falhou("timeout".to_string()),
        Err(e) if e.is_connect() => Resultado::Falhou("sem conexão".to_string()),
        Err(e) => Resultado::Falhou(e.to_string()),
    }
}

// ── Uso do app por fim de semana ────────────────────────────────────────────

/// Quanto tempo o jogador passou nas telas que CUSTAM geração de IA, mais o uso do
/// push-to-talk, acumulado por rodada.
///
/// Por que só três telas: notícias, briefing e debriefing são as que consomem o
/// servidor de IA. Calendário, tabela e ficha de piloto não custam nada, e medir a
/// permanência nelas seria vigiar por vigiar.
///
/// Por que por RODADA e não por sessão: o número sozinho ("12 minutos de notícias")
/// não decide nada. Cruzado com a corrida, ele responde se a matéria que foi gerada
/// para aquela etapa chegou a ser lida — que é a pergunta que paga a conta.
///
/// O bloco fecha quando a rodada VIRA. O que se mede é a janela entre uma corrida e a
/// seguinte: o debriefing da rodada N, as notícias da semana e o briefing da N+1 caem
/// todos no bloco rotulado N. Fechar no `race_end` perderia justamente o debriefing,
/// que só é lido depois.
#[derive(Serialize, Deserialize, Clone, Default)]
struct UsoRodada {
    /// `None` até a primeira corrida da instalação. O tempo lido antes disso não
    /// pertence a rodada nenhuma.
    temporada: Option<i32>,
    rodada: Option<i32>,
    categoria: Option<String>,
    #[serde(default)]
    segundos_noticias: i64,
    #[serde(default)]
    segundos_briefing: i64,
    #[serde(default)]
    segundos_debriefing: i64,
    /// Apertos que abriram o microfone.
    #[serde(default)]
    ptt_apertos: i64,
    /// Apertos que viraram pergunta de verdade. A diferença para os apertos é o que
    /// foi descartado como toque acidental ou cortado pelo freio do rádio, e é
    /// exatamente o aperto que NÃO custou transcrição nem geração.
    #[serde(default)]
    ptt_perguntas: i64,
}

impl UsoRodada {
    /// Só o rótulo da rodada, sem um segundo lido nem um aperto. É o estado de quem
    /// abriu o app e fechou sem ler nada.
    fn vazio(&self) -> bool {
        self.segundos_noticias == 0
            && self.segundos_briefing == 0
            && self.segundos_debriefing == 0
            && self.ptt_apertos == 0
    }
}

/// Arquivo do acumulado corrente. Em disco, e não só em memória, porque o bloco
/// atravessa o fechamento do app: quem lê o debriefing e fecha o Loop não pode levar
/// esse tempo embora.
static USO: OnceLock<PathBuf> = OnceLock::new();
static USO_TRAVA: Mutex<()> = Mutex::new(());

/// Somou tempo numa das três telas. `tela` é `noticias` | `briefing` | `debriefing`;
/// qualquer outra coisa é ignorada em silêncio, para o front não conseguir inventar
/// uma métrica nova sem passar por aqui.
pub fn uso_tela(tela: &str, segundos: i64) {
    if !is_enabled() || segundos <= 0 {
        return;
    }
    com_uso(|uso| match tela {
        "noticias" => uso.segundos_noticias += segundos,
        "briefing" => uso.segundos_briefing += segundos,
        "debriefing" => uso.segundos_debriefing += segundos,
        _ => {}
    });
}

/// O jogador apertou o push-to-talk. Contado na borda física do botão, então inclui
/// o toque acidental e o aperto que o freio do rádio vai cortar.
pub fn uso_ptt_aperto() {
    if !is_enabled() {
        return;
    }
    com_uso(|uso| uso.ptt_apertos += 1);
}

/// O aperto virou pergunta de verdade e foi ao servidor. É este que custa dinheiro:
/// transcrição mais geração da resposta.
pub fn uso_ptt_pergunta() {
    if !is_enabled() {
        return;
    }
    com_uso(|uso| uso.ptt_perguntas += 1);
}

/// Uma corrida terminou (dirigida ou simulada). Fecha o bloco anterior, manda, e abre
/// um novo rotulado com esta rodada.
///
/// Chamada DEPOIS do `race_end`/`race_sim` de propósito: o evento de uso é sobre a
/// leitura em volta da corrida, e não sobre a corrida.
pub fn uso_virar_rodada(temporada: i32, rodada: i32, categoria: &str) {
    if !is_enabled() {
        return;
    }
    let anterior = {
        let Ok(_trava) = USO_TRAVA.lock() else {
            return;
        };
        let atual = ler_uso();
        // Mesma rodada de novo (reimportação, corrida refeita): não fecha bloco, senão
        // a mesma janela de leitura viraria dois eventos com metade do tempo cada.
        if atual.temporada == Some(temporada) && atual.rodada == Some(rodada) {
            return;
        }
        gravar_uso(&UsoRodada {
            temporada: Some(temporada),
            rodada: Some(rodada),
            categoria: Some(categoria.to_string()),
            ..Default::default()
        });
        atual
    };
    enviar_uso(anterior);
}

/// Manda o que sobrou da sessão anterior. Chamada no boot, junto da drenagem da fila.
///
/// É isto que fecha o buraco do fechamento do app: o bloco fica no disco e vira evento
/// na próxima abertura, com o `atraso_s` dizendo quanto tempo ele esperou.
pub fn uso_enviar_pendente() {
    if !is_enabled() {
        return;
    }
    let pendente = {
        let Ok(_trava) = USO_TRAVA.lock() else {
            return;
        };
        let atual = ler_uso();
        // Preserva o rótulo da rodada: o jogador reabre o app na mesma etapa em que
        // parou, e o tempo que ele ainda vai gastar lendo pertence a ela.
        gravar_uso(&UsoRodada {
            temporada: atual.temporada,
            rodada: atual.rodada,
            categoria: atual.categoria.clone(),
            ..Default::default()
        });
        atual
    };
    enviar_uso(pendente);
}

fn enviar_uso(uso: UsoRodada) {
    // Bloco sem nenhum segundo e sem nenhum aperto não descreve nada. Acontece toda
    // vez que alguém abre o app e fecha sem ler, e mandar isso seria ruído puro.
    if uso.vazio() {
        return;
    }
    let mut payload = json!({
        "segundos_noticias": uso.segundos_noticias,
        "segundos_briefing": uso.segundos_briefing,
        "segundos_debriefing": uso.segundos_debriefing,
        "ptt_apertos": uso.ptt_apertos,
        "ptt_perguntas": uso.ptt_perguntas,
    });
    if let Some(map) = payload.as_object_mut() {
        if let Some(t) = uso.temporada {
            map.insert("temporada".into(), json!(t));
        }
        if let Some(r) = uso.rodada {
            map.insert("rodada".into(), json!(r));
        }
        if let Some(c) = uso.categoria {
            map.insert("categoria_rodada".into(), json!(c));
        }
    }
    send("weekend_usage", payload);
}

/// Lê, deixa o chamador mexer, grava. Toda escrita do acumulado passa por aqui.
fn com_uso(f: impl FnOnce(&mut UsoRodada)) {
    let Ok(_trava) = USO_TRAVA.lock() else {
        return;
    };
    let mut uso = ler_uso();
    f(&mut uso);
    gravar_uso(&uso);
}

fn ler_uso() -> UsoRodada {
    USO.get()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn gravar_uso(uso: &UsoRodada) {
    let Some(path) = USO.get() else {
        return;
    };
    if let (Some(dir), Ok(texto)) = (path.parent(), serde_json::to_string(uso)) {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(path, texto);
    }
}

fn apagar_uso() {
    let Some(path) = USO.get() else {
        return;
    };
    let Ok(_trava) = USO_TRAVA.lock() else {
        return;
    };
    let _ = std::fs::remove_file(path);
}

// ── Fila de reenvio ─────────────────────────────────────────────────────────

/// Um evento que não entrou, guardado inteiro. O corpo já vem montado com o
/// contexto de carreira do instante em que nasceu — remontá-lo na drenagem
/// descreveria a carreira de hoje, e não a daquela corrida.
#[derive(Serialize, Deserialize, Clone)]
struct Pendente {
    criado_em: i64,
    corpo: serde_json::Value,
}

/// Segundos desde a época. Só serve para medir DURAÇÃO (o `atraso_s` e a poda da
/// fila), nunca para datar o evento no servidor.
fn agora() -> i64 {
    chrono::Utc::now().timestamp()
}

fn enfileirar(corpo: serde_json::Value, criado_em: i64) {
    let Some(path) = FILA.get() else {
        return;
    };
    let Ok(_trava) = FILA_TRAVA.lock() else {
        return;
    };
    let mut pendentes = ler_fila(path);
    pendentes.push(Pendente { criado_em, corpo });
    let descartados = podar(&mut pendentes, agora());
    let total = pendentes.len();
    gravar_fila(path, &pendentes);
    let sobre_descarte = if descartados > 0 {
        format!(", {descartados} descartado(s) por idade ou teto")
    } else {
        String::new()
    };
    crate::diagnostico::linha(
        "telemetria",
        &format!("enfileirado para reenvio ({total} na fila{sobre_descarte})"),
    );
}

/// Manda o que ficou para trás. Chamada UMA vez no boot, depois do `init()`.
///
/// Roda em thread própria porque a primeira tentativa pode pagar o cold start
/// inteiro, e o boot não espera por telemetria.
pub fn drenar_fila() {
    if !is_enabled() {
        return;
    }
    let Some(path) = FILA.get() else {
        return;
    };
    std::thread::spawn(move || {
        // Tira tudo do arquivo de uma vez: o que não for entregue volta no fim. Se
        // o app morrer no meio, perde-se a fila, e isso é melhor que reenviar em
        // duplicata a cada boot.
        let pendentes = {
            let Ok(_trava) = FILA_TRAVA.lock() else {
                return;
            };
            let lidos = ler_fila(path);
            let _ = std::fs::remove_file(path);
            lidos
        };
        if pendentes.is_empty() {
            return;
        }
        crate::diagnostico::linha(
            "telemetria",
            &format!("drenando a fila ({} pendente(s))", pendentes.len()),
        );

        let (mut entregues, mut fracassos_seguidos) = (0usize, 0u32);
        let mut sobraram: Vec<Pendente> = Vec::new();
        for (i, p) in pendentes.iter().enumerate() {
            // Uma tentativa por evento aqui. Quem insiste é o boot seguinte: com
            // 200 na fila, as esperas do envio normal virariam mais de uma hora de
            // thread para nada.
            if fracassos_seguidos >= FILA_FRACASSOS_ATE_DESISTIR || !is_enabled() {
                sobraram.extend_from_slice(&pendentes[i..]);
                break;
            }
            if entregar(&p.corpo, p.criado_em, &[]) {
                entregues += 1;
                fracassos_seguidos = 0;
            } else {
                fracassos_seguidos += 1;
                sobraram.push(p.clone());
            }
        }

        crate::diagnostico::linha(
            "telemetria",
            &format!(
                "fila drenada: {entregues} entregue(s), {} de volta na fila",
                sobraram.len()
            ),
        );
        if sobraram.is_empty() || !is_enabled() {
            return;
        }
        let Ok(_trava) = FILA_TRAVA.lock() else {
            return;
        };
        // Junta com o que chegou enquanto a drenagem rodava, em vez de sobrescrever.
        // Os que sobraram vêm primeiro: são os mais antigos.
        let mut atual = sobraram;
        atual.extend(ler_fila(path));
        podar(&mut atual, agora());
        gravar_fila(path, &atual);
    });
}

fn apagar_fila() {
    let Some(path) = FILA.get() else {
        return;
    };
    let Ok(_trava) = FILA_TRAVA.lock() else {
        return;
    };
    let _ = std::fs::remove_file(path);
}

/// JSON Lines, e não um array: uma linha corrompida (queda de energia no meio da
/// escrita) custa aquele evento, em vez do arquivo inteiro.
fn ler_fila(path: &Path) -> Vec<Pendente> {
    let Ok(texto) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    texto
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Pendente>(l).ok())
        .collect()
}

fn gravar_fila(path: &Path, pendentes: &[Pendente]) {
    if pendentes.is_empty() {
        let _ = std::fs::remove_file(path);
        return;
    }
    let mut texto = String::new();
    for p in pendentes {
        if let Ok(linha) = serde_json::to_string(p) {
            texto.push_str(&linha);
            texto.push('\n');
        }
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, texto);
}

/// Tira o que envelheceu e corta pelo teto, mantendo os MAIS ANTIGOS: o começo da
/// corrida explica o fim, e a ordem do reenvio importa para o upsert do servidor.
/// Devolve quantos saíram.
fn podar(pendentes: &mut Vec<Pendente>, agora: i64) -> usize {
    let antes = pendentes.len();
    pendentes.retain(|p| agora - p.criado_em <= FILA_MAX_IDADE_SECS);
    pendentes.truncate(FILA_MAX_EVENTOS);
    antes - pendentes.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Só a parte de disco e de poda tem teste. O envio em si depende do servidor,
    /// e o que se pode afirmar sem rede é que o evento que não entrou sobrevive ao
    /// fechamento do app — que é exatamente o buraco que a fila veio tapar.
    fn dir_temp(rotulo: &str) -> PathBuf {
        let unico = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("relógio antes da época")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("loop_telemetria_{rotulo}_{unico}"));
        std::fs::create_dir_all(&dir).expect("criar dir temporário");
        dir
    }

    fn pendente(evento: &str, criado_em: i64) -> Pendente {
        Pendente {
            criado_em,
            corpo: json!({ "event": evento, "install_id": "abc" }),
        }
    }

    #[test]
    fn fila_sobrevive_a_ida_e_volta_do_disco() {
        let path = dir_temp("ida_volta").join("fila.jsonl");
        let pendentes = vec![pendente("race_start", 100), pendente("race_end", 200)];
        gravar_fila(&path, &pendentes);

        let lidos = ler_fila(&path);
        assert_eq!(lidos.len(), 2);
        assert_eq!(lidos[0].criado_em, 100);
        assert_eq!(lidos[1].corpo["event"], "race_end");
    }

    #[test]
    fn linha_corrompida_custa_so_aquele_evento() {
        let path = dir_temp("corrompida").join("fila.jsonl");
        let bom = serde_json::to_string(&pendente("race_start", 100)).unwrap();
        let outro = serde_json::to_string(&pendente("race_end", 200)).unwrap();
        std::fs::write(&path, format!("{bom}\n{{isto não é json\n{outro}\n")).unwrap();

        let lidos = ler_fila(&path);
        assert_eq!(lidos.len(), 2, "a linha quebrada levou as vizinhas junto");
    }

    #[test]
    fn fila_vazia_apaga_o_arquivo() {
        let path = dir_temp("vazia").join("fila.jsonl");
        gravar_fila(&path, &[pendente("race_start", 100)]);
        assert!(path.exists());

        gravar_fila(&path, &[]);
        assert!(!path.exists(), "arquivo vazio ficou para trás");
        assert!(ler_fila(&path).is_empty());
    }

    /// O acumulado de uso vive num estático de processo (`USO`), então os testes que o
    /// tocariam de verdade seriam serializados entre si e frágeis. O que dá para testar
    /// sem isso é a regra que decide o que vira evento, e é ela que erra na prática:
    /// bloco vazio nunca deve virar `weekend_usage`.
    #[test]
    fn bloco_sem_leitura_nao_vira_evento() {
        let so_rotulo = UsoRodada {
            temporada: Some(1),
            rodada: Some(3),
            categoria: Some("mazda_rookie".into()),
            ..Default::default()
        };
        assert!(so_rotulo.vazio(), "abriu o app e fechou sem ler: nada a dizer");

        let leu = UsoRodada {
            segundos_debriefing: 4,
            ..so_rotulo.clone()
        };
        assert!(!leu.vazio());

        let so_apertou = UsoRodada {
            ptt_apertos: 1,
            ..so_rotulo
        };
        assert!(
            !so_apertou.vazio(),
            "aperto de PTT sem leitura nenhuma ainda é uso"
        );
    }

    /// Campo novo no meio da vida do arquivo não pode zerar o que já estava lá: o
    /// acumulado sobrevive a um update do app com o jogador no meio de uma rodada.
    #[test]
    fn acumulado_antigo_sobrevive_a_campo_novo() {
        let antigo = r#"{"temporada":2,"rodada":5,"segundos_noticias":90}"#;
        let uso: UsoRodada = serde_json::from_str(antigo).expect("ler acumulado antigo");

        assert_eq!(uso.segundos_noticias, 90);
        assert_eq!(uso.rodada, Some(5));
        assert_eq!(uso.ptt_perguntas, 0);
        assert_eq!(uso.categoria, None);
    }

    #[test]
    fn poda_tira_o_que_envelheceu() {
        let agora = 1_000_000;
        let mut pendentes = vec![
            pendente("velho", agora - FILA_MAX_IDADE_SECS - 1),
            pendente("no_limite", agora - FILA_MAX_IDADE_SECS),
            pendente("novo", agora - 60),
        ];

        assert_eq!(podar(&mut pendentes, agora), 1);
        assert_eq!(pendentes.len(), 2);
        assert_eq!(pendentes[0].corpo["event"], "no_limite");
    }

    #[test]
    fn poda_corta_pelo_teto_mantendo_os_mais_antigos() {
        let agora = 1_000_000;
        let mut pendentes: Vec<Pendente> = (0..FILA_MAX_EVENTOS + 10)
            .map(|i| pendente(&format!("e{i}"), agora - 60))
            .collect();

        assert_eq!(podar(&mut pendentes, agora), 10);
        assert_eq!(pendentes.len(), FILA_MAX_EVENTOS);
        assert_eq!(
            pendentes[0].corpo["event"], "e0",
            "o corte comeu pelo começo da fila"
        );
    }
}
