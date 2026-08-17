//! Gravador de corrida — SEMPRE LIGADO.
//!
//! Despeja num arquivo JSONL a telemetria CRUA do iRacing + o YAML da sessão + um resumo do
//! histórico acumulado no fim. É a fonte-verdade pra rebalancear o app com dados REAIS de
//! pista (desgaste/estilo/quebra/clima/posições) sem inventar número, e é o que sai junto
//! quando o jogador manda o log pelo botão de diagnóstico.
//!
//! **Por que sempre ligado**: medido em release, gravar custa 23,7 µs por frame — 0,14% de um
//! núcleo a 60 Hz. Uma captura que só liga quando alguém lembra de ligar nunca pega o bug que
//! aconteceu ontem no PC do jogador.
//!
//! **Tamanho, medido no acervo real**: ~1,1 MB por minuto comprimido — 19 a 20 MB para uma
//! corrida de 17 min com 40 carros, e ~45 MB para uma de 40 min. Com [`MAX_CAPTURAS`], a pasta
//! de depuração chega a meio giga. O número de ~0,8 MB por corrida que este comentário trazia
//! era de quando [`CARS_HZ`] valia 4 e não sobreviveu à subida para 20; ficou aqui por meses
//! subestimando o custo em 20×.
//!
//! Guardamos as [`MAX_CAPTURAS`] mais recentes; a partir daí a mais antiga é apagada a cada
//! gravação nova, então o teto de disco é fixo.
//!
//! O amostrador do monitor (~60 Hz) chama `record_session`/`record_frame` a cada tick
//! conectado, e é ele quem abre e fecha o arquivo pelas bordas de conexão do sim. O do JOGADOR
//! vai à TAXA CHEIA (~60 Hz) — os eventos de <100 ms do estilo de pilotagem (toque no
//! limitador, RPM da troca, pico de frenagem) importam pra calibração, e o app já processa
//! a 60 Hz. Só pulamos frames de tempo CONGELADO (pausa/menu/replay parado) pra não duplicar.
//!
//! O arquivo é gravado **comprimido** (`.jsonl.gz`), e o volume é cortado em três frentes,
//! nenhuma delas perdendo sinal útil:
//!   1. **gzip** — o JSONL cru passava de 200 MB em 15 min. Reduz ~11× sem perda alguma.
//!   2. **`cars[]` a [`CARS_HZ`]** — os campos que importam nos outros carros (posição, volta
//!      completa, melhor volta, gap ao líder, box) só mudam na passagem pela linha; o único
//!      que anda a 60 Hz é o `lap_dist_pct`. Era 74% dos bytes. **Os escalares do jogador
//!      seguem a 60 Hz; só o `cars[]` é afinado** — quem ler a captura precisa saber disso,
//!      porque medir duração de estado de OUTRO carro tem resolução de 50 ms, não de 16 ms.
//!   3. **arredondamento** — `roll_rate: 1.1829563391074771e-6` são 22 bytes pra um número
//!      cuja quarta casa já é ruído de sensor. Ver [`CASAS_PADRAO`].
//!
//! Formato: uma linha JSON por registro, com `kind`:
//!   • `header`  — versão + timestamp de início + os parâmetros de subamostragem, pra quem lê
//!                 saber a que taxa os `cars[]` foram gravados.
//!   • `vars`    — uma vez: o inventário do que o SDK publica NESTA build (nome, tipo,
//!                 quantidade, unidade, descrição). Ver [`record_vars`].
//!   • `session` — o `session_yaml` inteiro, REGRAVADO a cada mudança, com `n` crescente.
//!                 O último traz os resultados. Ver [`record_session`].
//!   • `frame`   — `{ kind, tele }` com a `IracingTelemetry`. **O campo `cars` só aparece a
//!                 [`CARS_HZ`]**: quem lê deve carregar o último `cars` visto adiante.
//!   • `history` — no fim, o `RaceHistory` (voltas, paradas, posições, clima, incidentes).

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::write::GzEncoder;
use flate2::Compression;

use super::IracingTelemetry;

static ENABLED: AtomicBool = AtomicBool::new(false);
static FRAMES: AtomicU64 = AtomicU64::new(0);

/// Versão do formato. 1 = JSONL cru com `cars` em todo frame. 2 = gzip + `cars` subamostrado
/// + floats arredondados. 3 = inventário de variáveis do SDK, `session` repetido a cada
/// mudança do YAML, contexto de obstáculo por carro (material da superfície, RPM, bandeiras,
/// formação) e de sessão (tick, `PaceMode`, boxes, chuva, câmera/replay).
const FORMAT_VERSION: u32 = 3;

/// A cada quantos frames dar flush no disco (segurança contra crash/fechar o sim sem parar).
/// A ~60 Hz, 60 = ~1 flush/s.
const FLUSH_EVERY: u64 = 60;

/// Taxa de gravação do array `cars[]` (os OUTROS carros). O frame do jogador continua cheio.
///
/// Era 4 Hz, medido como suficiente para saber SE um carro está parado (o erro da
/// velocidade derivada contra a `Speed` real ficou em 2,0 km/h). Subiu para 20 quando a
/// pergunta mudou de "está parado?" para "QUANDO parou, e por quanto tempo ficou fora":
/// a 4 Hz toda fronteira de estado tem ±250 ms de incerteza, e é justamente uma duração
/// de fronteira que precisa ser calibrada. A 20 Hz a incerteza cai para ±50 ms, e o
/// arquivo custa o que está medido em [`MAX_CAPTURAS`].
const CARS_HZ: f64 = 20.0;

/// Intervalo mínimo de tempo de SESSÃO entre dois `cars[]` gravados.
const CARS_INTERVALO_S: f64 = 1.0 / CARS_HZ;

/// Casas decimais dos floats. 4 casas cobrem tudo com folga: tempo de sessão em 0,1 ms,
/// tempos de volta em 0,1 ms, entradas do piloto em 0,01%.
const CASAS_PADRAO: i32 = 4;

/// `lap_dist_pct` é a POSIÇÃO na pista e ganha mais casas: num circuito de 3 km, 6 casas
/// valem 3 mm e 4 casas já valeriam 30 cm — perto do que o carro anda entre dois frames.
const CASAS_POSICAO: i32 = 6;

/// Quantas capturas ficam no disco. A partir da 11ª, a mais antiga é apagada.
///
/// MEDIDO em Lime Rock com 40 carros de IA (`race_1785885657.jsonl.gz`, ver
/// `docs/spotter-obstaculo.md`): **19 MB para 17,1 min ≈ 1,1 MB/min** já comprimido. Uma
/// corrida de 40 min dá ~45 MB, então este teto é da ordem de MEIO GIGA, e não dos 10 MB
/// que o número velho de 0,8 MB por corrida prometia — ele é anterior ao `cars[]` a 20 Hz.
/// O tamanho é o preço de gravar o campo inteiro; se a pasta incomodar, o botão é este
/// teto, e não a taxa.
pub const MAX_CAPTURAS: usize = 10;

/// Pasta das capturas, resolvida uma vez no boot. `None` até o [`init`] — o amostrador roda
/// numa thread sem `AppHandle` e só enxerga o estático daqui (mesmo padrão do `diagnostico`).
static PASTA: OnceLock<PathBuf> = OnceLock::new();

struct Capture {
    writer: GzEncoder<BufWriter<File>>,
    path: PathBuf,
    last_t: f64,
    /// Tempo de sessão do último frame em que `cars[]` foi gravado (-1 = nenhum ainda).
    last_cars_t: f64,
    /// Impressão digital do último YAML gravado, e quantos já saíram.
    yaml_hash: u64,
    yaml_n: u32,
    /// O inventário de variáveis do SDK já foi gravado.
    vars_written: bool,
}

/// Arredonda todo float da árvore, por campo. Inteiros passam intactos (o `fract() != 0`
/// os pula), assim `idx`, `lap` e as flags continuam exatos.
fn arredonda(v: &mut serde_json::Value, casas: i32) {
    match v {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if f.fract() != 0.0 {
                    let escala = 10f64.powi(casas);
                    if let Some(novo) = serde_json::Number::from_f64((f * escala).round() / escala)
                    {
                        *n = novo;
                    }
                }
            }
        }
        serde_json::Value::Array(itens) => {
            for item in itens {
                arredonda(item, casas);
            }
        }
        serde_json::Value::Object(campos) => {
            for (chave, item) in campos.iter_mut() {
                let casas = if chave == "lap_dist_pct" {
                    CASAS_POSICAO
                } else {
                    casas
                };
                arredonda(item, casas);
            }
        }
        _ => {}
    }
}

fn state() -> &'static Mutex<Option<Capture>> {
    static S: OnceLock<Mutex<Option<Capture>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// Fixa a pasta das capturas. Chamado UMA vez no `setup()` do boot, antes do sampler.
pub fn init(base_dir: &std::path::Path) {
    let _ = PASTA.set(base_dir.join("debug").join("race_captures"));
}

/// A pasta das capturas (`None` antes do [`init`]).
pub fn pasta() -> Option<&'static PathBuf> {
    PASTA.get()
}

/// Lista as capturas do disco, da mais NOVA pra mais antiga. O nome carrega o timestamp
/// (`race_<epoch>.jsonl.gz`), então ordenar por nome ao contrário já ordena por data — sem
/// depender do mtime, que backup e sincronizador mexem.
pub fn listar() -> Vec<PathBuf> {
    PASTA.get().map(|d| listar_em(d)).unwrap_or_default()
}

/// Como [`listar`], mas numa pasta explícita — a rotação precisa agir na pasta do arquivo
/// que está sendo aberto, que nem sempre é a do [`init`] (o comando de debug passa a sua).
fn listar_em(dir: &std::path::Path) -> Vec<PathBuf> {
    let Ok(entradas) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut arquivos: Vec<PathBuf> = entradas
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("race_") && n.ends_with(".jsonl.gz"))
        })
        .collect();
    arquivos.sort();
    arquivos.reverse();
    arquivos
}

/// Apaga as capturas que passam de [`MAX_CAPTURAS`], da mais antiga pra frente. Best-effort:
/// um arquivo travado por outro processo é pulado e some na próxima rodada.
fn rotacionar(dir: &std::path::Path) {
    for antiga in listar_em(dir)
        .into_iter()
        .skip(MAX_CAPTURAS.saturating_sub(1))
    {
        let _ = fs::remove_file(&antiga);
    }
}

/// Abre uma captura nova na pasta do [`init`], se ainda não há uma gravando. É o que o
/// amostrador chama quando o sim conecta. No-op silencioso antes do `init`.
pub fn ensure_started() {
    if ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let Some(dir) = PASTA.get() else {
        return;
    };
    match start(dir.clone()) {
        Ok(path) => crate::diagnostico::linha(
            "captura",
            &format!(
                "gravação iniciada: {}",
                crate::diagnostico::redigir(&path.display().to_string())
            ),
        ),
        Err(e) => crate::diagnostico::linha("captura", &format!("falha ao iniciar: {e}")),
    }
}

/// Começa a gravar num arquivo novo dentro de `dir` (criada se não existir). Devolve o caminho.
/// Antes de abrir, aplica a rotação: deixa espaço pra esta ser a [`MAX_CAPTURAS`]-ésima.
pub fn start(dir: PathBuf) -> Result<PathBuf, String> {
    fs::create_dir_all(&dir).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    rotacionar(&dir);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // O carimbo tem resolução de SEGUNDO: duas capturas abertas no mesmo segundo (o sim
    // caindo e voltando) colidiriam, e `File::create` TRUNCA — a primeira sumiria calada.
    let mut path = dir.join(format!("race_{stamp}.jsonl.gz"));
    let mut sufixo = 1;
    while path.exists() {
        path = dir.join(format!("race_{stamp}_{sufixo}.jsonl.gz"));
        sufixo += 1;
    }
    let file = File::create(&path).map_err(|e| format!("Falha ao criar arquivo: {e}"))?;
    let mut cap = Capture {
        writer: GzEncoder::new(BufWriter::new(file), Compression::default()),
        path: path.clone(),
        last_t: -1.0,
        last_cars_t: -1.0,
        yaml_hash: 0,
        yaml_n: 0,
        vars_written: false,
    };
    let _ = writeln!(
        cap.writer,
        "{}",
        serde_json::json!({
            "kind": "header",
            "version": FORMAT_VERSION,
            "stamp": stamp,
            "cars_hz": CARS_HZ,
            "casas": CASAS_PADRAO,
        })
    );
    let _ = cap.writer.flush();
    *state().lock().map_err(|e| e.to_string())? = Some(cap);
    FRAMES.store(0, Ordering::Relaxed);
    ENABLED.store(true, Ordering::Relaxed);
    Ok(path)
}

/// Grava o YAML da sessão SEMPRE QUE ELE MUDA: pista, elenco, carro, redline, composto,
/// e — a razão de não bastar gravar uma vez — o `ResultsPositions` com o `ReasonOutStr`
/// de cada carro.
///
/// A string de sessão não é estática. Ela é reescrita quando a sessão avança, quando um
/// piloto entra ou sai, e sobretudo quando os resultados são publicados. Guardar só a
/// primeira versão significa guardar a corrida sem o desfecho: quem abandonou, quem foi
/// desclassificado, quem simplesmente parou. Como a comparação é por hash e o YAML só é
/// consultado a cada poucos segundos, o custo de acompanhar as mudanças é nada.
pub fn record_session(yaml: &str) {
    if !ENABLED.load(Ordering::Relaxed) || yaml.is_empty() {
        return;
    }
    let hash = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        yaml.hash(&mut h);
        h.finish()
    };
    if let Ok(mut g) = state().lock() {
        if let Some(cap) = g.as_mut() {
            if cap.yaml_hash != hash {
                cap.yaml_hash = hash;
                cap.yaml_n += 1;
                let _ = writeln!(
                    cap.writer,
                    "{}",
                    serde_json::json!({ "kind": "session", "n": cap.yaml_n, "yaml": yaml })
                );
            }
        }
    }
}

/// Grava, uma vez por captura, TUDO que o SDK publica nesta build — nome, tipo,
/// quantidade, unidade e descrição de cada variável.
///
/// É o que separa "o canal não existe" de "o canal existe e vem zerado". A leitura casa
/// nomes num `match`; um nome errado ou um canal que a build não publica caem no mesmo
/// silêncio, e olhando só a telemetria os dois são idênticos. Com o inventário, dá para
/// saber o que se poderia ter lido sem sequer abrir o sim de novo.
pub fn record_vars(vars: &[crate::iracing_sdk::VarDoSdk]) {
    if !ENABLED.load(Ordering::Relaxed) || vars.is_empty() {
        return;
    }
    if let Ok(mut g) = state().lock() {
        if let Some(cap) = g.as_mut() {
            if !cap.vars_written {
                cap.vars_written = true;
                let _ = writeln!(
                    cap.writer,
                    "{}",
                    serde_json::json!({ "kind": "vars", "vars": vars })
                );
            }
        }
    }
}

/// Grava um frame de telemetria à TAXA CHEIA do sampler (~60 Hz). Pula só quando o tempo de
/// sessão NÃO avançou (pausa/menu/replay parado) pra não duplicar frames idênticos.
///
/// O array `cars[]` sai a [`CARS_HZ`]: nos demais frames o campo é OMITIDO e quem lê carrega
/// o último valor visto adiante.
///
/// ## O relógio que volta para trás
///
/// `session_time` reinicia quando o jogador REINICIA a sessão, e aí ele não é maior que o
/// último visto — é muito menor. A guarda de tempo congelado, sozinha, lia isso como "o
/// tempo não avançou" e devolvia sem gravar. Como `last_t` continuava no máximo antigo, ela
/// devolvia em TODO quadro seguinte, até o relógio novo ultrapassar o velho.
///
/// Medido em 17/08/2026 numa corrida em Oschersleben: **594 segundos de sim sem um único
/// quadro**, com o `session_tick` pulando de 29067 para 64713 enquanto o `session_time`
/// gravado andava 16 ms. O arquivo resultante é o pior tipo de arquivo defeituoso, porque o
/// filtro só admite valor crescente e o que sobra parece perfeitamente contínuo. O mesmo
/// buraco estava em outras duas capturas do acervo, de 161 s e 614 s.
///
/// A borda já era conhecida logo abaixo, na janela dos `cars` ("volta atrás no tempo (troca
/// de sessão)"), e essa linha nunca era alcançada porque a guarda de cima retornava antes.
///
/// O critério é o mesmo do resto do módulo: recuo maior que
/// [`super::spotter_base::SALTO_MAX_S`] é mundo novo, e mundo novo zera o relógio em vez de
/// ser descartado.
/// O que fazer com um quadro, dado o relógio do último que foi gravado.
#[derive(Debug, PartialEq, Eq)]
enum Passo {
    /// O tempo avançou: quadro normal.
    Gravar,
    /// O tempo não avançou (pausa, menu, replay parado): quadro duplicado.
    Pular,
    /// O tempo voltou muito: sessão nova. Zera o relógio e grava.
    RelogioNovo,
}

/// A decisão, separada do estado do arquivo para poder ser provada sem tocar em disco.
///
/// `last_t` negativo é "ainda não gravei nada".
fn decidir(last_t: f64, agora_s: f64) -> Passo {
    if last_t < 0.0 {
        return Passo::Gravar;
    }
    if agora_s < last_t - super::spotter_base::SALTO_MAX_S {
        return Passo::RelogioNovo;
    }
    if agora_s <= last_t {
        return Passo::Pular;
    }
    Passo::Gravar
}

pub fn record_frame(t: &IracingTelemetry) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    if let Ok(mut g) = state().lock() {
        if let Some(cap) = g.as_mut() {
            match decidir(cap.last_t, t.session_time) {
                Passo::Pular => return,
                Passo::RelogioNovo => {
                    cap.last_t = -1.0;
                    cap.last_cars_t = -1.0;
                }
                Passo::Gravar => {}
            }
            cap.last_t = t.session_time;

            // Volta atrás no tempo (troca de sessão) também reabre a janela dos `cars`.
            let com_cars = cap.last_cars_t < 0.0
                || t.session_time < cap.last_cars_t
                || t.session_time - cap.last_cars_t >= CARS_INTERVALO_S;
            if com_cars {
                cap.last_cars_t = t.session_time;
            }

            let mut v = serde_json::json!({ "kind": "frame", "tele": t });
            if let Some(tele) = v.get_mut("tele").and_then(serde_json::Value::as_object_mut) {
                if !com_cars {
                    tele.remove("cars");
                }
            }
            arredonda(&mut v, CASAS_PADRAO);

            if let Ok(line) = serde_json::to_string(&v) {
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
            let mut v = serde_json::json!({ "kind": kind, "data": value });
            arredonda(&mut v, CASAS_PADRAO);
            let _ = writeln!(cap.writer, "{v}");
        }
    }
}

/// Para a gravação, fecha o arquivo e devolve o caminho (None se não havia gravação).
///
/// O `finish()` é o que escreve o TRAILER do gzip. Sem ele (app fechado sem passar por aqui)
/// o arquivo ainda abre, porque o flush periódico já despejou os blocos, mas o descompressor
/// reclama de fluxo truncado no fim.
pub fn stop() -> Option<PathBuf> {
    ENABLED.store(false, Ordering::Relaxed);
    let mut g = state().lock().ok()?;
    let cap = g.take()?;
    if let Ok(mut inner) = cap.writer.finish() {
        let _ = inner.flush();
    }
    Some(cap.path)
}

/// Quantas capturas entram no envio do log. Menos que as [`MAX_CAPTURAS`] guardadas em disco:
/// o disco é nosso e é barato, mas o corpo do POST é do jogador e do servidor.
pub const CAPTURAS_NO_LOG: usize = 3;

/// Teto do resumo inteiro no corpo do envio. O log em si já leva 60 KB; dobrar o payload por
/// causa do anexo seria trocar a chance de o envio COMPLETAR pela riqueza do anexo.
const RESUMO_MAX_BYTES: usize = 48 * 1024;

/// Resumo das [`CAPTURAS_NO_LOG`] corridas mais recentes, pra viajar junto do log.
///
/// **Não** manda os frames crus: um arquivo tem ~0,8 MB comprimido e o endpoint de log é
/// dimensionado em dezenas de KB. Manda o bloco `history` (que já é o dado DERIVADO: voltas,
/// setores, posições, clima, paradas) sem as duas séries longas — `player_track` e o trace
/// por volta — mais a identificação da pista e do carro. Se o resumo apontar pra algo que só
/// os frames explicam, o caminho do arquivo está no log e dá pra pedir a captura.
pub fn resumo_para_log() -> serde_json::Value {
    let mut resumos = Vec::new();
    let mut total = 0usize;
    for path in listar().into_iter().take(CAPTURAS_NO_LOG) {
        let Some(r) = resumo_de(&path) else { continue };
        let tamanho = r.to_string().len();
        if total + tamanho > RESUMO_MAX_BYTES {
            break;
        }
        total += tamanho;
        resumos.push(r);
    }
    serde_json::Value::Array(resumos)
}

/// Lê UMA captura e devolve o resumo. `None` se o arquivo não abre ou não tem `history`
/// (corrida ainda gravando, ou app fechado no meio).
fn resumo_de(path: &std::path::Path) -> Option<serde_json::Value> {
    use std::io::BufRead;

    let arquivo = File::open(path).ok()?;
    let leitor = std::io::BufReader::new(flate2::read::GzDecoder::new(arquivo));

    let mut header = serde_json::Value::Null;
    let mut pista = serde_json::Value::Null;
    let mut history = serde_json::Value::Null;
    let mut frames = 0u64;

    // Uma passada só. Os frames são 99,99% das linhas e nem são desserializados — só contados.
    for linha in leitor.lines().map_while(Result::ok) {
        if linha.starts_with("{\"kind\":\"frame\"") {
            frames += 1;
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&linha) else {
            continue;
        };
        match v.get("kind").and_then(|k| k.as_str()) {
            Some("header") => header = v,
            Some("session") => {
                pista = v
                    .get("yaml")
                    .and_then(|y| y.as_str())
                    .map(campos_do_weekend)
                    .unwrap_or(serde_json::Value::Null);
            }
            Some("history") => history = v.get("data").cloned().unwrap_or(serde_json::Value::Null),
            _ => {}
        }
    }

    // As duas séries longas ficam de fora: juntas são ~90% do `history` e nenhuma delas
    // responde uma pergunta de suporte que as outras não respondam.
    if let Some(obj) = history.as_object_mut() {
        let track = obj
            .remove("player_track")
            .map(|v| v.as_array().map_or(0, Vec::len));
        let laps = obj.remove("laps").map(|v| v.as_array().map_or(0, Vec::len));
        obj.insert("player_track_len".into(), track.unwrap_or(0).into());
        obj.insert("laps_len".into(), laps.unwrap_or(0).into());
    }

    Some(serde_json::json!({
        "arquivo": path.file_name().and_then(|n| n.to_str()).unwrap_or_default(),
        "bytes": fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        "frames": frames,
        "header": header,
        "pista": pista,
        "history": history,
    }))
}

/// Puxa do YAML da sessão os poucos campos do `WeekendInfo` que identificam a corrida.
/// Busca por prefixo em vez de parsear o YAML inteiro: são 1.300 linhas pra pegar cinco
/// valores, e um YAML malformado não pode derrubar o envio do log.
fn campos_do_weekend(yaml: &str) -> serde_json::Value {
    const CAMPOS: [&str; 6] = [
        "TrackDisplayName",
        "TrackID",
        "TrackLength",
        "EventType",
        "TrackSkies",
        "TrackPrecipitation",
    ];
    let mut saida = serde_json::Map::new();
    for linha in yaml.lines() {
        let corte = linha.trim();
        for campo in CAMPOS {
            if let Some(valor) = corte.strip_prefix(campo).and_then(|r| r.strip_prefix(':')) {
                saida
                    .entry(campo)
                    .or_insert_with(|| valor.trim().trim_matches('"').into());
            }
        }
    }
    serde_json::Value::Object(saida)
}

/// Está gravando agora?
pub fn is_active() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Caminho da captura em curso, se houver.
pub fn caminho_atual() -> Option<PathBuf> {
    state().lock().ok()?.as_ref().map(|c| c.path.clone())
}

/// Quantos frames já foram gravados nesta sessão de captura.
pub fn frame_count() -> u64 {
    FRAMES.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_primeiro_quadro_sempre_grava() {
        assert_eq!(decidir(-1.0, 0.0), Passo::Gravar);
        assert_eq!(decidir(-1.0, 481.45), Passo::Gravar);
    }

    #[test]
    fn tempo_congelado_e_quadro_duplicado() {
        // Pausa, menu, replay parado: o `session_time` não anda e o quadro repetiria o
        // anterior. É o caso que a guarda sempre existiu para pegar.
        assert_eq!(decidir(100.0, 100.0), Passo::Pular);
        assert_eq!(decidir(100.0, 99.5), Passo::Pular);
    }

    #[test]
    fn o_tempo_andando_grava() {
        assert_eq!(decidir(100.0, 100.0167), Passo::Gravar);
        assert_eq!(decidir(100.0, 160.0), Passo::Gravar);
    }

    /// A REGRESSÃO, com o número que ela custou.
    ///
    /// Em 17/08/2026 uma corrida em Oschersleben perdeu **594 segundos de sim sem um único
    /// quadro**: o jogador reiniciou a sessão, o `session_time` voltou de 481 para perto de
    /// zero, e a guarda de tempo congelado leu isso como quadro duplicado. Como `last_t`
    /// continuava em 481, ela recusou TODO quadro seguinte até o relógio novo ultrapassar o
    /// velho. O arquivo resultante parece contínuo, porque o filtro só admite valor
    /// crescente, e o buraco só aparece cruzando com o `session_tick`.
    ///
    /// O mesmo estava em outras duas capturas do acervo, de 161 s e 614 s.
    #[test]
    fn a_sessao_reiniciada_zera_o_relogio_em_vez_de_engolir_a_corrida() {
        assert_eq!(
            decidir(481.45, 0.05),
            Passo::RelogioNovo,
            "o relógio voltou ao começo: é sessão nova, e não quadro repetido"
        );
        assert_eq!(decidir(481.45, 2.5), Passo::RelogioNovo);
        // E o quadro seguinte do mundo novo grava normalmente, sem esperar passar os 481.
        assert_eq!(decidir(-1.0, 0.067), Passo::Gravar);
    }

    /// A fronteira entre "voltou um pouco" e "é outro mundo" é a mesma do resto do módulo.
    #[test]
    fn o_recuo_pequeno_continua_sendo_quadro_repetido() {
        let s = super::super::spotter_base::SALTO_MAX_S;
        assert_eq!(decidir(100.0, 100.0 - s), Passo::Pular, "no limite ainda é congelamento");
        assert_eq!(decidir(100.0, 100.0 - s - 0.1), Passo::RelogioNovo, "além dele, mundo novo");
    }

    /// Cria um arquivo vazio com nome de captura, pra exercitar listagem e rotação sem
    /// depender do gravador (que carimba a hora e só tem resolução de segundo).
    fn toca(dir: &std::path::Path, nome: &str) {
        fs::create_dir_all(dir).unwrap();
        File::create(dir.join(nome)).unwrap();
    }

    fn pasta_limpa(nome: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(nome);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn listar_ordena_da_mais_nova_pra_mais_antiga_e_ignora_intrusos() {
        let dir = pasta_limpa("loop_cap_listar");
        toca(&dir, "race_1000.jsonl.gz");
        toca(&dir, "race_3000.jsonl.gz");
        toca(&dir, "race_2000.jsonl.gz");
        // Nada disso é captura e nada disso pode entrar na conta da rotação.
        toca(&dir, "race_1500.jsonl"); // formato v1, sem gzip
        toca(&dir, "outro.jsonl.gz");
        toca(&dir, "notas.txt");

        let nomes: Vec<String> = listar_em(&dir)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(
            nomes,
            [
                "race_3000.jsonl.gz",
                "race_2000.jsonl.gz",
                "race_1000.jsonl.gz"
            ]
        );
    }

    #[test]
    fn rotacao_deixa_dez_contando_a_que_vai_abrir() {
        let dir = pasta_limpa("loop_cap_rotacao");
        // 12 antigas: a rotação precisa deixar 9, pra nova virar a décima.
        for i in 1..=12 {
            toca(&dir, &format!("race_{:04}.jsonl.gz", i * 100));
        }
        rotacionar(&dir);

        let restantes = listar_em(&dir);
        assert_eq!(restantes.len(), MAX_CAPTURAS - 1);
        // As que sobraram são as MAIS NOVAS: 1200 fica, 300 e as anteriores somem.
        let nomes: Vec<String> = restantes
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(nomes.first().unwrap(), "race_1200.jsonl.gz");
        assert_eq!(nomes.last().unwrap(), "race_0400.jsonl.gz");
    }

    #[test]
    #[serial_test::serial]
    fn start_no_mesmo_segundo_nao_sobrescreve_a_captura_anterior() {
        let dir = pasta_limpa("loop_cap_colisao");
        let a = start(dir.clone()).unwrap();
        stop().unwrap();
        let b = start(dir.clone()).unwrap();
        stop().unwrap();
        assert_ne!(
            a, b,
            "duas capturas seguidas não podem cair no mesmo arquivo"
        );
        assert!(a.exists() && b.exists());
    }

    #[test]
    #[serial_test::serial]
    fn resumo_traz_o_history_sem_as_series_longas() {
        let dir = pasta_limpa("loop_cap_resumo");
        let path = start(dir.clone()).unwrap();
        record_session("---\nWeekendInfo:\n TrackDisplayName: Circuit de Ledenon\n TrackID: 489\n EventType: Race\n");
        record_frame(&IracingTelemetry {
            session_time: 1.0,
            ..Default::default()
        });
        record_frame(&IracingTelemetry {
            session_time: 2.0,
            ..Default::default()
        });
        record_block(
            "history",
            serde_json::json!({
                "player_laps": [{ "lap": 4, "time": 96.888_900_756_835_94 }],
                "player_track": [{ "lap": 1 }, { "lap": 2 }, { "lap": 3 }],
                "laps": [{ "cars": [] }],
                "outcome": "Finalizada",
            }),
        );
        stop().unwrap();

        let r = resumo_de(&path).expect("a captura recém-fechada tem que resumir");
        assert_eq!(r["frames"], 2);
        assert_eq!(r["pista"]["TrackDisplayName"], "Circuit de Ledenon");
        assert_eq!(r["pista"]["TrackID"], "489");
        assert_eq!(r["history"]["outcome"], "Finalizada");
        // As séries longas viram contagem — é o que cabe num envio de log.
        assert!(r["history"]["player_track"].is_null());
        assert!(r["history"]["laps"].is_null());
        assert_eq!(r["history"]["player_track_len"], 3);
        assert_eq!(r["history"]["laps_len"], 1);
        // E o arredondamento chegou até o bloco anexado.
        assert_eq!(r["history"]["player_laps"][0]["time"], 96.8889);
    }

    #[test]
    #[serial_test::serial]
    fn resumo_ignora_captura_sem_history() {
        let dir = pasta_limpa("loop_cap_sem_hist");
        let path = start(dir.clone()).unwrap();
        record_frame(&IracingTelemetry {
            session_time: 1.0,
            ..Default::default()
        });
        stop().unwrap();
        // App fechado no meio da corrida: sem `history`, mas o arquivo abre e conta frames.
        let r = resumo_de(&path).expect("arquivo sem history ainda é legível");
        assert!(r["history"].is_null());
        assert_eq!(r["frames"], 1);
    }
}
