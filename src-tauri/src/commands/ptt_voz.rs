//! As duas idas ao servidor do push-to-talk.
//!
//! **Nenhuma chave passa por aqui.** O Scribe, o Gemini e a Cloud TTS ficam do lado de lá
//! (projeto `iracer-news-server`), atrás do mesmo segredo de porta que os boletins já
//! usam. O app manda áudio e fatos; recebe texto e áudio.
//!
//! São DUAS viagens, e não uma, por escolha de projeto — a mesma que fez o acervo gravado
//! existir. A primeira volta com a transcrição, e é olhando para ela que o app decide se
//! precisa da segunda: quando a pergunta cabe no acervo (a maioria das perguntas de
//! corrida cabe), a resposta sai em ~0,7 s, sem modelo, sem síntese e sem custo. Fundir as
//! duas viagens numa só economizaria um RTT nas perguntas raras e o cobraria de TODAS as
//! outras.
//!
//! O áudio atravessa este arquivo em base64 e **não é decodificado em lugar nenhum do
//! Rust**: sobe como string no JSON e desce como string no JSON. O front codifica e
//! decodifica, o servidor idem. Guardar os bytes aqui no meio só renderia uma conversão a
//! mais e uma dependência nova para nada — este módulo é um relé.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Manager};

use crate::config::app_config::AppConfig;
use crate::narrative::client::APP_SECRET;

const RAIZ: &str = "https://iracer-news-124606451488.southamerica-east1.run.app";

/// Teto da transcrição. Bem mais curto que os 45 s dos boletins, e de propósito: aqui
/// existe alguém segurando o volante à espera de uma resposta. Passou disto, a resposta
/// perdeu a validade — o engenheiro diz que não sabe e o piloto pergunta de novo, que é
/// melhor que um silêncio de meio minuto.
const TIMEOUT_TRANSCRICAO_S: u64 = 15;
/// Teto da redação + síntese. Maior porque são duas etapas do outro lado (o modelo
/// escreve, a Cloud TTS fala) e porque a frase de espera já está tocando aqui.
const TIMEOUT_RESPOSTA_S: u64 = 25;

/// Teto da síntese de uma peça própria. Curto: é uma palavra, e ninguém está esperando por
/// ela — quem chama é o boot do engenheiro, e uma falha aqui custa o vocativo até a próxima
/// vez que o app abrir.
const TIMEOUT_PECA_S: u64 = 20;

fn cliente(timeout_s: u64) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_s))
        .build()
        .map_err(|e| format!("Falha ao montar o cliente HTTP: {e}"))
}

/// Identificador da instalação — é por ele que o servidor aplica cooldown e teto de
/// gasto. Mesma fonte que os boletins usam, para o app inteiro contar como um só.
fn identidade(app: &AppHandle) -> (String, String) {
    let Ok(base) = app.path().app_data_dir() else {
        return (String::new(), "pt-BR".into());
    };
    let mut config = AppConfig::load_or_default(&base);
    let id = config.get_or_create_install_id();
    let lang = config.language.clone();
    (id, lang)
}

/// Traduz o status HTTP no que o jogador (e o log) precisam saber. O texto vira,
/// no fim da linha, a decisão de tocar "não sei" — então importa que ele diga a
/// diferença entre "sem internet" e "o servidor recusou".
fn erro_de_status(status: u16) -> String {
    match status {
        401 => "O servidor recusou a credencial do app.".into(),
        429 => "Muitas perguntas em pouco tempo — o servidor pediu para esperar.".into(),
        503 => "O servidor de voz está indisponível.".into(),
        outro => format!("O servidor respondeu HTTP {outro}."),
    }
}

#[derive(Deserialize)]
struct RespostaTranscricao {
    texto: String,
}

/// **Primeira ida:** o áudio do piloto sobe, a transcrição desce.
///
/// `audio_b64` é o WebM/Opus que o `MediaRecorder` produziu, em base64. `mime` viaja
/// junto porque o Scribe escolhe o decodificador por ele, e o WebView2 pode entregar
/// `audio/webm;codecs=opus` ou só `audio/webm` conforme a versão.
#[tauri::command]
pub async fn ptt_transcrever(
    app: AppHandle,
    audio_b64: String,
    mime: String,
) -> Result<String, String> {
    // Telemetria de produto: é AQUI que o aperto vira dinheiro. O áudio subindo é o
    // primeiro custo (transcrição), e o `ptt_responder` que vem depois é o segundo.
    // Contar na borda do botão contaria também o toque acidental, que não custa nada.
    crate::telemetry::uso_ptt_pergunta();

    tauri::async_runtime::spawn_blocking(move || {
        let (install_id, lang) = identidade(&app);
        let corpo = serde_json::json!({
            "audio_b64": audio_b64,
            "mime": mime,
            "lang": lang,
            "install_id": install_id,
        });
        let resp = cliente(TIMEOUT_TRANSCRICAO_S)?
            .post(format!("{RAIZ}/ptt-transcrever"))
            .header("x-app-secret", APP_SECRET)
            .json(&corpo)
            .send()
            .map_err(|e| format!("Falha de rede ao transcrever: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(erro_de_status(status.as_u16()));
        }
        let parsed: RespostaTranscricao = resp
            .json()
            .map_err(|e| format!("Resposta de transcrição inesperada: {e}"))?;
        Ok(parsed.texto.trim().to_string())
    })
    .await
    .map_err(|e| format!("A transcrição não chegou ao fim: {e}"))?
}

/// O que a segunda ida devolve: a frase e o áudio dela, já sintetizado.
#[derive(Serialize, Deserialize)]
pub struct FalaSintetizada {
    /// O que o modelo escreveu. Não é para tocar — é para registrar e depurar. Quem
    /// fala é o áudio.
    pub texto: String,
    /// O áudio cru da Cloud TTS, em base64. **Sem a cadeia de rádio**: ela é aplicada
    /// no front, em tempo real, pela mesma receita que ficou gravada nas peças do acervo.
    /// Filtrar do lado de lá obrigaria o servidor a conhecer a nossa cor de som e a
    /// refazê-la a cada ajuste.
    pub audio_b64: String,
    pub mime: String,
}

/// **Segunda ida:** os fatos sobem, a frase falada desce.
///
/// As `linhas` já vêm calculadas por [`crate::engenheiro::responder::dossie`] — o modelo redige em
/// cima delas e não faz conta nenhuma. Foi assim que os erros de leitura de número
/// sumiram na medição: não há número para ele converter, só número para ele copiar.
#[tauri::command]
pub async fn ptt_responder(
    app: AppHandle,
    transcricao: String,
    intencao: String,
    linhas: Vec<String>,
    // `ocasiao`: quando ninguém perguntou nada — `antes_da_largada` ou
    // `depois_da_bandeirada`. Vazia no caminho normal, e é ela que faz o servidor trocar de
    // prompt: responder a uma pergunta e abrir o rádio por conta própria são duas falas
    // diferentes, e a segunda dita com a forma da primeira soa como resposta a algo que o
    // piloto não disse.
    ocasiao: Option<String>,
    // `tratamento`: como ele deve te chamar NESTA fala — "novato" no começo da carreira, o
    // primeiro nome depois. Vazio na maioria das respostas, e o vazio é instrução: sem
    // vocativo nenhum. Quem decide de quantas em quantas vezes ele chama é o app
    // (`engenheiro::tratamento`), não o modelo — deixar a escolha para quem redige daria
    // vocativo em toda frase, que é como uma proximidade vira cacoete.
    tratamento: Option<String>,
) -> Result<FalaSintetizada, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (install_id, lang) = identidade(&app);
        let corpo = serde_json::json!({
            "transcricao": transcricao,
            "intencao": intencao,
            "linhas": linhas,
            "lang": lang,
            "install_id": install_id,
            "ocasiao": ocasiao.unwrap_or_default(),
            "tratamento": tratamento.unwrap_or_default(),
        });
        let resp = cliente(TIMEOUT_RESPOSTA_S)?
            .post(format!("{RAIZ}/ptt-responder"))
            .header("x-app-secret", APP_SECRET)
            .json(&corpo)
            .send()
            .map_err(|e| format!("Falha de rede ao pedir a resposta: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(erro_de_status(status.as_u16()));
        }
        let parsed: FalaSintetizada = resp
            .json()
            .map_err(|e| format!("Resposta falada inesperada: {e}"))?;
        if parsed.audio_b64.is_empty() {
            return Err("O servidor respondeu sem áudio.".into());
        }
        Ok(parsed)
    })
    .await
    .map_err(|e| format!("A resposta não chegou ao fim: {e}"))?
}

/// **A síntese de uma peça própria**: o sobrenome do jogador, uma vez na vida da carreira.
///
/// Bloqueante e sem `#[tauri::command]` — quem a chama é
/// [`super::engenheiro::voz_propria`], que já está dentro de um `spawn_blocking` porque
/// também escreve no disco. Duas camadas de `spawn_blocking` para a mesma operação seriam
/// duas threads para um trabalho sequencial.
///
/// A VÍRGULA não é mandada daqui: quem a põe é o servidor. A prosódia de vocativo é
/// propriedade da peça, e deixá-la na mão de quem chama seria o começo de duas peças com a
/// mesma chave e entoações diferentes.
pub(crate) fn sintetizar_peca(app: &AppHandle, nome: &str) -> Result<(String, String), String> {
    let (install_id, lang) = identidade(app);
    let corpo = serde_json::json!({
        "nome": nome,
        "lang": lang,
        "install_id": install_id,
    });
    let resp = cliente(TIMEOUT_PECA_S)?
        .post(format!("{RAIZ}/peca-voz"))
        .header("x-app-secret", APP_SECRET)
        .json(&corpo)
        .send()
        .map_err(|e| format!("Falha de rede ao pedir a peça de voz: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(erro_de_status(status.as_u16()));
    }
    let parsed: FalaSintetizada = resp
        .json()
        .map_err(|e| format!("Peça de voz inesperada: {e}"))?;
    if parsed.audio_b64.is_empty() {
        return Err("O servidor respondeu sem áudio.".into());
    }
    Ok((parsed.audio_b64, parsed.mime))
}

/// Tira o servidor do zero. Chamado quando a sessão de corrida abre, e não nas voltas
/// finais como o aquecimento do debrief: a primeira pergunta ao engenheiro pode vir na
/// volta 1, e ela é justamente a que pagaria o cold start inteiro do Cloud Run.
#[tauri::command]
pub fn ptt_aquecer() {
    crate::narrative::client::spawn_warmup();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_status_vira_frase_util() {
        // O que distingue "espere" de "quebrou" é o que decide se vale tentar de novo.
        assert!(erro_de_status(429).contains("esperar"));
        assert!(erro_de_status(401).contains("credencial"));
        assert!(erro_de_status(500).contains("500"));
    }
}
