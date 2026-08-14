//! PROVA DE CONCEITO — latência do TTS do Google em streaming.
//!
//! Não é o engenheiro de pista. É só o cano mais curto possível entre "texto pronto"
//! e "primeiro byte de áudio na mão do front", para medir se a arquitetura de fala
//! DINÂMICA se sustenta. Nada de telemetria, STT, intenção, cache ou filtro de rádio.
//!
//! Modelo: `gemini-3.1-flash-tts-preview` — hoje é o ÚNICO TTS do Gemini que aceita
//! streaming (`stream: true`); os 2.5 (flash/pro) só devolvem o áudio inteiro no fim,
//! o que por definição reprova no critério "tempo até o primeiro som".
//!
//! Endpoint: `POST https://generativelanguage.googleapis.com/v1beta/interactions`
//! (Interactions API), autenticado por header `x-goog-api-key`. A resposta é SSE:
//! linhas `data: {json}`, e o áudio vem em base64 dentro de um delta de tipo `audio`.
//! Formato: PCM linear 16 bits, 24 kHz, mono (little-endian).
//!
//! Este módulo NÃO decodifica o base64: repassa a string crua pro front por evento
//! Tauri. O front faz `atob` e alimenta o Web Audio direto — um passo a menos entre
//! o byte chegar e o alto-falante, que é justamente o que estamos cronometrando.
//!
//! Eventos emitidos (payload em `tipos` abaixo):
//! - `tts-poc-chunk` — um bloco de áudio base64
//! - `tts-poc-done`  — fim do stream (com os tempos medidos do lado do Rust)
//! - `tts-poc-error` — falha (rede, HTTP, chave, parse)

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

const ENDPOINT: &str = "https://generativelanguage.googleapis.com/v1beta/interactions";
const MODELO_PADRAO: &str = "gemini-3.1-flash-tts-preview";
const VOZ_PADRAO: &str = "Charon";
/// Teto do fim de vida da requisição. Generoso de propósito: a POC quer MEDIR o pior
/// caso, não escondê-lo atrás de um timeout curto. O corte por SLA é do front.
const TIMEOUT_TOTAL: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------------------
// Tipos que cruzam a ponte
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct TtsPocPedido {
    /// Correlaciona os eventos com a medição aberta no front.
    pub request_id: String,
    pub texto: String,
    /// Prefixo de direção de atuação (o Gemini TTS é dirigido por prompt, não por
    /// parâmetro). Vazio = fala neutra.
    #[serde(default)]
    pub instrucao: Option<String>,
    #[serde(default)]
    pub voz: Option<String>,
    #[serde(default)]
    pub modelo: Option<String>,
    /// `false` força o modo não-streaming — serve pra comparar contra o streaming e
    /// provar quanto o streaming realmente adianta o primeiro som.
    #[serde(default = "streaming_padrao")]
    pub streaming: bool,
}

fn streaming_padrao() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
pub struct TtsPocInicio {
    pub request_id: String,
    pub modelo: String,
    pub voz: String,
    pub streaming: bool,
    /// De onde saiu a chave: `"env"` ou `"arquivo"`. Só pra POC saber o que testou.
    pub origem_chave: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TtsPocChunk {
    pub request_id: String,
    pub seq: u32,
    /// Base64 do PCM cru (s16le, 24 kHz, mono). Pode vir com cabeçalho RIFF no 1º.
    pub b64: String,
    pub bytes_b64: usize,
    /// Milissegundos desde o disparo da requisição HTTP, medidos no Rust.
    pub ms: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TtsPocDone {
    pub request_id: String,
    pub blocos: u32,
    pub bytes_b64: usize,
    /// Tempo até o cabeçalho da resposta HTTP (handshake + fila do modelo).
    pub ms_ate_resposta: f64,
    /// Tempo até o PRIMEIRO bloco de áudio sair do Rust.
    pub ms_ate_primeiro_bloco: Option<f64>,
    pub ms_total: f64,
    pub cancelado: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TtsPocErro {
    pub request_id: String,
    pub mensagem: String,
    pub http_status: Option<u16>,
    pub ms: f64,
}

// ---------------------------------------------------------------------------
// Cancelamento (Etapa 11: descartar a comemoração quando a vitória evapora)
// ---------------------------------------------------------------------------

fn cancelados() -> &'static Mutex<HashSet<String>> {
    static CANCELADOS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    CANCELADOS.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Requisições EM VOO. Existe só pra decidir se um cancelamento é válido: sem isso,
/// um `tts_poc_cancelar` com id errado (ou de uma execução já terminada) gravaria uma
/// marca que ninguém viria consumir, e o `HashSet` de cancelados cresceria pra sempre.
fn ativos() -> &'static Mutex<HashSet<String>> {
    static ATIVOS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    ATIVOS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn foi_cancelado(request_id: &str) -> bool {
    cancelados()
        .lock()
        .map(|c| c.contains(request_id))
        .unwrap_or(false)
}

/// Devolve os dois conjuntos ao estado limpo quando a execução sai de cena — por
/// QUALQUER porta: fim normal, `return erro(...)` no meio ou pânico da thread. Antes
/// isso era uma limpeza solta no fim de `executar`, que todos os caminhos de erro
/// pulavam e deixavam o id preso em `cancelados` até o app fechar.
struct BaixaDaRequisicao(String);

impl Drop for BaixaDaRequisicao {
    fn drop(&mut self) {
        if let Ok(mut c) = cancelados().lock() {
            c.remove(&self.0);
        }
        if let Ok(mut a) = ativos().lock() {
            a.remove(&self.0);
        }
    }
}

/// Marca uma requisição EM VOO pra cancelamento. Id desconhecido = cancelamento
/// inválido (digitação errada, ou a execução já acabou): vira no-op em vez de sujar
/// o conjunto com uma marca eterna.
#[tauri::command]
pub fn tts_poc_cancelar(request_id: String) {
    let em_voo = ativos()
        .lock()
        .map(|a| a.contains(&request_id))
        .unwrap_or(false);
    if !em_voo {
        return;
    }
    if let Ok(mut c) = cancelados().lock() {
        c.insert(request_id);
    }
}

// ---------------------------------------------------------------------------
// Chave de API
// ---------------------------------------------------------------------------

/// Ordem: variável de ambiente (o caminho de quem roda `npm run tauri dev`) e, se
/// faltar, um arquivo solto no app_data. A chave NUNCA vai pro front — nem pra tela,
/// nem por evento; o front só recebe a ORIGEM.
fn resolver_chave(app: &AppHandle) -> Result<(String, String), String> {
    for var in ["GEMINI_API_KEY", "GOOGLE_API_KEY"] {
        if let Ok(v) = std::env::var(var) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Ok((v, format!("env:{var}")));
            }
        }
    }

    let caminho = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?
        .join("gemini_tts_key.txt");

    if let Ok(conteudo) = std::fs::read_to_string(&caminho) {
        let v = conteudo.trim().to_string();
        if !v.is_empty() {
            return Ok((v, "arquivo".to_string()));
        }
    }

    Err(format!(
        "Chave do Gemini ausente. Defina GEMINI_API_KEY no ambiente antes de abrir o app, \
         ou grave a chave em {}.",
        caminho.display()
    ))
}

/// Onde a POC grava o log de execuções. O front pergunta pra mostrar na tela.
fn caminho_log(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?
        .join("tts-poc");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Falha ao criar {}: {e}", dir.display()))?;
    Ok(dir.join("resultados.jsonl"))
}

#[tauri::command]
pub fn tts_poc_log_caminho(app: AppHandle) -> Result<String, String> {
    Ok(caminho_log(&app)?.display().to_string())
}

/// Achata um registro em UMA linha física. JSONL é "um objeto por linha": se o front
/// mandar o JSON indentado (ou qualquer coisa com `\n` no meio), o `writeln!` cru
/// partiria o registro em várias linhas e o leitor devolveria cada pedaço como se
/// fosse uma execução — todas inválidas. Emendar com ESPAÇO é seguro: entre tokens
/// JSON o espaço em branco é irrelevante, e quebra de linha dentro de uma string JSON
/// já seria JSON ilegal na origem.
fn achatar_em_uma_linha(linha: &str) -> String {
    linha
        .lines()
        .map(str::trim)
        .filter(|parte| !parte.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Acrescenta UMA linha JSON ao log. Append puro: cada execução é um registro
/// independente, então uma corrida interrompida não perde o que já mediu.
#[tauri::command]
pub fn tts_poc_log_registrar(app: AppHandle, linha: String) -> Result<(), String> {
    use std::io::Write;
    let registro = achatar_em_uma_linha(&linha);
    if registro.is_empty() {
        return Ok(()); // nada a registrar — não suja o arquivo com linha vazia.
    }
    let caminho = caminho_log(&app)?;
    let mut arquivo = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&caminho)
        .map_err(|e| format!("Falha ao abrir {}: {e}", caminho.display()))?;
    writeln!(arquivo, "{registro}").map_err(|e| format!("Falha ao escrever no log: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn tts_poc_log_ler(app: AppHandle) -> Result<Vec<String>, String> {
    let caminho = caminho_log(&app)?;
    match std::fs::read_to_string(&caminho) {
        Ok(conteudo) => Ok(conteudo
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect()),
        Err(_) => Ok(Vec::new()),
    }
}

// ---------------------------------------------------------------------------
// Extração do áudio de dentro do JSON do evento
// ---------------------------------------------------------------------------

/// Chaves que a API usa (ou já usou) pra carregar o áudio em base64. A lista é ampla
/// de propósito: a Interactions API é preview e o nome do campo já mudou entre
/// revisões da doc. Varrer a árvore custa microssegundos e nos deixa imunes a isso.
const CHAVES_AUDIO: &[&str] = &[
    "data",
    "audio",
    "audioData",
    "audio_data",
    "inlineData",
    "inline_data",
    "b64",
];

/// Uma string só é aceita como áudio se for longa e do alfabeto base64. Sem isso, um
/// `"data": "ok"` qualquer viraria um bloco de áudio corrompido.
fn parece_base64(s: &str) -> bool {
    s.len() >= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
}

/// Percorre o JSON em profundidade juntando, em ordem, todo base64 de áudio. Objetos
/// marcados como `"type": "text"` são pulados — lá o campo `data` é a transcrição, não
/// o som.
fn coletar_audio(v: &Value, saida: &mut Vec<String>) {
    match v {
        Value::Object(mapa) => {
            let eh_texto = matches!(mapa.get("type").and_then(Value::as_str), Some("text"));
            for (chave, valor) in mapa {
                if !eh_texto {
                    if let Value::String(s) = valor {
                        if CHAVES_AUDIO.contains(&chave.as_str()) && parece_base64(s) {
                            saida.push(s.clone());
                            continue;
                        }
                    }
                }
                coletar_audio(valor, saida);
            }
        }
        Value::Array(itens) => {
            for item in itens {
                coletar_audio(item, &mut *saida);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Comando principal
// ---------------------------------------------------------------------------

/// Dispara a geração e volta IMEDIATAMENTE. O áudio chega por evento; o cronômetro
/// de verdade é o do front, que é quem sabe quando o som saiu na caixa.
///
/// A thread é `std::thread` (não `spawn_blocking`): o stream fica preso num `read`
/// bloqueante por segundos e não deve ocupar uma vaga do pool do Tauri.
#[tauri::command]
pub fn tts_poc_falar(app: AppHandle, pedido: TtsPocPedido) -> Result<TtsPocInicio, String> {
    let (chave, origem_chave) = resolver_chave(&app)?;

    let modelo = pedido
        .modelo
        .clone()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| MODELO_PADRAO.to_string());
    let voz = pedido
        .voz
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| VOZ_PADRAO.to_string());

    let inicio = TtsPocInicio {
        request_id: pedido.request_id.clone(),
        modelo: modelo.clone(),
        voz: voz.clone(),
        streaming: pedido.streaming,
        origem_chave,
    };

    // Limpa um cancelamento antigo com o mesmo id e abre o voo ANTES de soltar a thread:
    // o front pode chamar `tts_poc_cancelar` no instante seguinte ao retorno daqui, e um
    // id ainda não registrado como ativo faria o cancelamento ser descartado.
    if let Ok(mut c) = cancelados().lock() {
        c.remove(&pedido.request_id);
    }
    if let Ok(mut a) = ativos().lock() {
        a.insert(pedido.request_id.clone());
    }

    let app_thread = app.clone();
    std::thread::spawn(move || {
        executar(app_thread, pedido, chave, modelo, voz);
    });

    Ok(inicio)
}

fn executar(app: AppHandle, pedido: TtsPocPedido, chave: String, modelo: String, voz: String) {
    let t0 = Instant::now();
    let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;
    let request_id = pedido.request_id.clone();
    // Dá baixa em `ativos`/`cancelados` na saída, seja ela qual for.
    let _baixa = BaixaDaRequisicao(request_id.clone());

    let erro = |mensagem: String, status: Option<u16>| {
        let _ = app.emit(
            "tts-poc-error",
            TtsPocErro {
                request_id: request_id.clone(),
                mensagem,
                http_status: status,
                ms: ms(t0),
            },
        );
    };

    let entrada = match pedido.instrucao.as_deref().map(str::trim) {
        Some(i) if !i.is_empty() => format!("{i}\n\n{}", pedido.texto),
        _ => pedido.texto.clone(),
    };

    let corpo = serde_json::json!({
        "model": modelo,
        "input": entrada,
        "response_format": { "type": "audio" },
        "generation_config": {
            "speech_config": [ { "voice": voz } ]
        },
        "stream": pedido.streaming,
    });

    let cliente = match reqwest::blocking::Client::builder()
        .timeout(TIMEOUT_TOTAL)
        // Sem isso o reqwest pode segurar bytes esperando encher um buffer, o que
        // falsificaria justamente a métrica que a POC existe pra medir.
        .tcp_nodelay(true)
        .build()
    {
        Ok(c) => c,
        Err(e) => return erro(format!("Falha ao criar cliente HTTP: {e}"), None),
    };

    let resposta = cliente
        .post(ENDPOINT)
        .header("x-goog-api-key", &chave)
        .header("content-type", "application/json")
        .header(
            "accept",
            if pedido.streaming {
                "text/event-stream"
            } else {
                "application/json"
            },
        )
        .json(&corpo)
        .send();

    let ms_ate_resposta = ms(t0);

    let resposta = match resposta {
        Ok(r) => r,
        Err(e) => return erro(format!("Falha de rede: {e}"), None),
    };

    let status = resposta.status();
    if !status.is_success() {
        let detalhe = resposta.text().unwrap_or_default();
        let detalhe: String = detalhe.chars().take(600).collect();
        return erro(
            format!("HTTP {}: {}", status.as_u16(), detalhe),
            Some(status.as_u16()),
        );
    }

    let mut seq: u32 = 0;
    let mut bytes_b64: usize = 0;
    let mut ms_primeiro: Option<f64> = None;
    let mut cancelado = false;

    let mut emitir = |b64: String| {
        let agora = ms(t0);
        if ms_primeiro.is_none() {
            ms_primeiro = Some(agora);
        }
        bytes_b64 += b64.len();
        let _ = app.emit(
            "tts-poc-chunk",
            TtsPocChunk {
                request_id: request_id.clone(),
                seq,
                bytes_b64: b64.len(),
                b64,
                ms: agora,
            },
        );
        seq += 1;
    };

    if pedido.streaming {
        let mut leitor = BufReader::new(resposta);
        let mut linha = String::new();
        loop {
            if foi_cancelado(&request_id) {
                cancelado = true;
                break;
            }
            linha.clear();
            match leitor.read_line(&mut linha) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) => {
                    return erro(format!("Falha ao ler o stream: {e}"), Some(status.as_u16()))
                }
            }

            let bruto = linha.trim();
            if bruto.is_empty() {
                continue;
            }
            // SSE: só a linha `data:` carrega payload. `event:`/`id:`/comentários (`:`)
            // são ruído de protocolo.
            let Some(json_bruto) = bruto.strip_prefix("data:") else {
                continue;
            };
            let json_bruto = json_bruto.trim();
            if json_bruto.is_empty() || json_bruto == "[DONE]" {
                continue;
            }
            let Ok(valor) = serde_json::from_str::<Value>(json_bruto) else {
                continue;
            };

            let mut blocos = Vec::new();
            coletar_audio(&valor, &mut blocos);
            for b64 in blocos {
                emitir(b64);
            }
        }
    } else {
        // Modo controle: um único JSON no fim. Serve pra provar, com número, o quanto o
        // streaming adianta o primeiro som em relação a esperar o arquivo inteiro.
        let texto = match resposta.text() {
            Ok(t) => t,
            Err(e) => return erro(format!("Falha ao ler a resposta: {e}"), None),
        };
        let Ok(valor) = serde_json::from_str::<Value>(&texto) else {
            return erro("Resposta não é JSON válido.".to_string(), None);
        };
        let mut blocos = Vec::new();
        coletar_audio(&valor, &mut blocos);
        if blocos.is_empty() {
            let amostra: String = texto.chars().take(400).collect();
            return erro(format!("Nenhum áudio na resposta: {amostra}"), None);
        }
        for b64 in blocos {
            emitir(b64);
        }
    }

    if seq == 0 && !cancelado {
        return erro(
            "Stream terminou sem nenhum bloco de áudio.".to_string(),
            Some(status.as_u16()),
        );
    }

    let _ = app.emit(
        "tts-poc-done",
        TtsPocDone {
            request_id: request_id.clone(),
            blocos: seq,
            bytes_b64,
            ms_ate_resposta,
            ms_ate_primeiro_bloco: ms_primeiro,
            ms_total: ms(t0),
            cancelado,
        },
    );
    // A baixa em `ativos`/`cancelados` é do `_baixa` (Drop), não daqui: os `return
    // erro(...)` acima nunca chegariam a esta linha.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64_falso(tamanho: usize) -> String {
        "QUJDRA".chars().cycle().take(tamanho).collect()
    }

    #[test]
    fn coleta_audio_do_formato_documentado() {
        let audio = b64_falso(128);
        let evento = serde_json::json!({
            "event_type": "step.delta",
            "delta": { "type": "audio", "data": audio }
        });
        let mut saida = Vec::new();
        coletar_audio(&evento, &mut saida);
        assert_eq!(saida, vec![audio]);
    }

    #[test]
    fn coleta_audio_em_formato_alternativo_aninhado() {
        // Se a preview voltar ao formato `inline_data` do generateContent, a POC não
        // pode parar de achar o áudio.
        let audio = b64_falso(200);
        let evento = serde_json::json!({
            "candidates": [
                { "content": { "parts": [
                    { "inline_data": { "mime_type": "audio/L16;rate=24000", "data": audio } }
                ] } }
            ]
        });
        let mut saida = Vec::new();
        coletar_audio(&evento, &mut saida);
        assert_eq!(saida, vec![audio]);
    }

    #[test]
    fn ignora_transcricao_de_texto() {
        let evento = serde_json::json!({
            "delta": { "type": "text", "data": b64_falso(300) }
        });
        let mut saida = Vec::new();
        coletar_audio(&evento, &mut saida);
        assert!(saida.is_empty(), "transcrição não é áudio");
    }

    #[test]
    fn ignora_string_curta_ou_fora_do_alfabeto() {
        let evento = serde_json::json!({
            "delta": { "type": "audio", "data": "ok" },
            "meta": { "audio": "isto não é base64, tem espaço e acento" }
        });
        let mut saida = Vec::new();
        coletar_audio(&evento, &mut saida);
        assert!(saida.is_empty());
    }

    #[test]
    fn registro_com_newline_interno_vira_uma_linha_so() {
        // JSON indentado é o caso real: o `writeln!` cru partiria isto em 4 linhas e o
        // leitor devolveria 4 "execuções", todas JSON inválido.
        let indentado = "{\n  \"request_id\": \"abc\",\n  \"ms\": 12\n}";
        let achatado = achatar_em_uma_linha(indentado);

        assert!(!achatado.contains('\n'), "{achatado}");
        assert_eq!(achatado, "{ \"request_id\": \"abc\", \"ms\": 12 }");
        // E continua sendo JSON legível — emendar com espaço não quebra os tokens.
        let valor: Value = serde_json::from_str(&achatado).expect("segue JSON válido");
        assert_eq!(valor["request_id"], "abc");
    }

    #[test]
    fn registro_vazio_ou_so_de_quebras_nao_vira_linha() {
        assert_eq!(achatar_em_uma_linha(""), "");
        assert_eq!(achatar_em_uma_linha("\n\n   \r\n"), "");
    }

    #[test]
    fn registro_de_uma_linha_atravessa_intacto() {
        let linha = r#"{"request_id":"abc","ms":12}"#;
        assert_eq!(achatar_em_uma_linha(&format!("  {linha}  ")), linha);
    }

    /// Cancelar um id que não está em voo é no-op. Antes, qualquer id (errado, ou de
    /// uma execução já encerrada) entrava no `HashSet` e ficava lá até o app fechar.
    #[test]
    #[serial_test::serial]
    fn cancelamento_de_id_desconhecido_nao_suja_o_conjunto() {
        cancelados().lock().unwrap().clear();
        ativos().lock().unwrap().clear();

        tts_poc_cancelar("id-que-nunca-existiu".to_string());

        assert!(
            cancelados().lock().unwrap().is_empty(),
            "cancelamento inválido não pode deixar marca"
        );
        assert!(!foi_cancelado("id-que-nunca-existiu"));
    }

    /// A baixa (Drop) limpa os dois conjuntos — é ela que cobre os `return erro(...)`
    /// que saem de `executar` no meio e nunca chegariam a uma limpeza no fim do corpo.
    #[test]
    #[serial_test::serial]
    fn baixa_limpa_ativos_e_cancelados_em_qualquer_saida() {
        cancelados().lock().unwrap().clear();
        ativos().lock().unwrap().clear();

        ativos().lock().unwrap().insert("req-1".to_string());
        tts_poc_cancelar("req-1".to_string());
        assert!(foi_cancelado("req-1"), "id em voo aceita cancelamento");

        {
            let _baixa = BaixaDaRequisicao("req-1".to_string());
        } // saída da execução, por qualquer porta

        assert!(!foi_cancelado("req-1"));
        assert!(cancelados().lock().unwrap().is_empty());
        assert!(ativos().lock().unwrap().is_empty());
    }

    #[test]
    fn preserva_a_ordem_dos_blocos_do_array() {
        let a = b64_falso(64);
        let b = b64_falso(96);
        let evento = serde_json::json!({
            "deltas": [
                { "type": "audio", "data": a.clone() },
                { "type": "audio", "data": b.clone() }
            ]
        });
        let mut saida = Vec::new();
        coletar_audio(&evento, &mut saida);
        assert_eq!(saida, vec![a, b]);
    }
}
