//! Cliente do servidor de boletins (proxy do Gemini).
//!
//! O app NUNCA fala com o Gemini direto — só com o NOSSO servidor (Cloud Run),
//! que guarda a chave. O segredo abaixo é embutido por design (client-side): ele
//! vai no binário e serve só como "porta de entrada" do servidor; a defesa real
//! contra abuso é o cooldown por install_id + o teto global de gasto.

use serde::Deserialize;
use std::time::Duration;

const SERVER_URL: &str = "https://iracer-news-124606451488.southamerica-east1.run.app/race-story";
/// Endpoint da prévia pré-corrida (narrativa + voz da equipe, curtas).
const PRE_RACE_URL: &str = "https://iracer-news-124606451488.southamerica-east1.run.app/pre-race";
/// Endpoint do debrief pós-corrida (voz única do engenheiro → piloto, com calor).
const POST_RACE_URL: &str = "https://iracer-news-124606451488.southamerica-east1.run.app/post-race";
/// Endpoint do rodapé "Do mundo do Grid" (reescrita jornalística das notinhas).
const WORLD_NOTES_URL: &str =
    "https://iracer-news-124606451488.southamerica-east1.run.app/world-notes";
/// Endpoint da matéria "O Que Esperar" (prévia de temporada). Ver
/// `docs/season-preview-design.md` e `docs/season-preview-endpoint.md`.
const SEASON_PREVIEW_URL: &str =
    "https://iracer-news-124606451488.southamerica-east1.run.app/season-preview";
/// Comprimento-alvo da prévia, em palavras. Vai no payload (`target_words`) porque a
/// matéria é a peça principal da aba e o bundle hoje carrega uns dez dossiês — cobrir
/// todos com 400 palavras dá uma frase por piloto, que é o que fazia a matéria soar
/// rasa. O servidor manda no texto final; isto é o pedido do cliente.
const SEASON_PREVIEW_TARGET_WORDS: (u32, u32) = (700, 900);
/// `pub(crate)` porque a telemetria de produto (`crate::telemetry`) entra pela
/// MESMA porta do servidor — um segredo só, um lugar só pra trocar.
pub(crate) const APP_SECRET: &str =
    "827119cc235cdea25c04545cd283749e673917d2d424340fb1059925738efcef";
// 45s e não 20s: o servidor (Cloud Run) faz scale-to-zero quando ocioso, e a 1ª
// chamada após um período parado paga um cold start (subir o container + init do
// Firestore) ANTES de gerar. Quente, gera em ~3s; frio pode passar de 20s. 45s dá
// folga pra um cold start caber sem o cliente desistir e cair no template.
const TIMEOUT_SECS: u64 = 45;
/// Raiz do serviço — usada só pelo aquecimento (`spawn_warmup`), que não pede geração.
const BASE_URL: &str = "https://iracer-news-124606451488.southamerica-east1.run.app/";
/// Intervalo mínimo entre aquecimentos. O gatilho vive no poll da torre (2 Hz), então
/// sem freio seriam centenas de requisições por corrida; o Cloud Run segura o container
/// vivo por alguns minutos depois da última chamada, e cinco minutos cobrem essa janela.
const WARMUP_MIN_INTERVAL_SECS: i64 = 5 * 60;
static ULTIMO_WARMUP: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// Tira o servidor do zero SEM pedir geração nenhuma.
///
/// O Cloud Run faz scale-to-zero: a 1ª chamada depois de um tempo parado paga o cold
/// start (subir o container + init do Firestore) ANTES de escrever a primeira linha —
/// é de onde vêm os 20-40s de espera que o `TIMEOUT_SECS` precisa cobrir. Um GET na raiz
/// obriga o container a subir; o status da resposta é irrelevante (404 serve). Não gasta
/// cota do modelo e não mexe no cooldown, porque não passa por nenhum endpoint de geração.
///
/// Fire-and-forget, com guarda de intervalo própria: pode ser chamado de um laço de poll.
pub fn spawn_warmup() {
    use std::sync::atomic::Ordering;

    let agora = chrono::Local::now().timestamp();
    let anterior = ULTIMO_WARMUP.load(Ordering::Relaxed);
    if agora - anterior < WARMUP_MIN_INTERVAL_SECS {
        return;
    }
    // CAS: entre duas batidas do poll só uma thread ganha o direito de aquecer.
    if ULTIMO_WARMUP
        .compare_exchange(anterior, agora, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    std::thread::spawn(move || {
        let Ok(client) = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .build()
        else {
            return;
        };
        let _ = client.get(BASE_URL).send();
    });
}

#[derive(Debug)]
pub enum StoryError {
    /// 429 — cooldown de 10 min ou teto diário. Cai no template, silencioso.
    RateLimited,
    /// 401 — segredo inválido (não deveria acontecer em produção).
    Unauthorized,
    /// Erro do servidor / Gemini (5xx) ou resposta inesperada.
    Server(String),
    /// Falha de rede / sem internet.
    Network(String),
    /// Resposta vazia.
    Empty,
}

#[derive(Deserialize)]
struct StoryResponse {
    story: String,
}

/// Fecho de frase de verdade — o que uma matéria pode terminar sem parecer cortada.
const FECHOS_DE_FRASE: [char; 6] = ['.', '!', '?', '…', '"', '»'];

/// O servidor gera com teto de tokens: quando a geração estoura, o texto chega
/// cortado no meio da frase (fica um "O resultado em Lédenon" pendurado no fim).
/// Apara a cauda até o último fim de frase real — o texto perde a última ideia, mas
/// FECHA. NUNCA descarta: texto curto, cortado cedo ou sem nenhum fim de frase volta
/// como veio. O que a IA escreveu sempre vale mais que o template determinístico; a
/// solução do corte é o teto de geração no servidor, não jogar a matéria fora aqui.
pub(crate) fn aparar_frase_incompleta(texto: &str) -> String {
    let t = texto.trim_end();
    if t.chars()
        .last()
        .is_some_and(|c| FECHOS_DE_FRASE.contains(&c))
    {
        return t.to_string();
    }
    match t
        .char_indices()
        .rev()
        .find(|(_, c)| FECHOS_DE_FRASE.contains(c))
    {
        Some((idx, ch)) => t[..idx + ch.len_utf8()].trim_end().to_string(),
        None => t.to_string(),
    }
}

/// Envia os fatos curados ao servidor e devolve o boletim redigido no idioma
/// pedido. Em QUALQUER erro, o chamador deve cair no template (notícia genérica)
/// — o boletim de IA nunca pode quebrar a tela.
pub fn fetch_story(
    facts: &str,
    lang: &str,
    install_id: &str,
    reading_seconds: Option<f64>,
) -> Result<String, StoryError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
        .map_err(|e| StoryError::Network(e.to_string()))?;

    let body = serde_json::json!({
        "facts": facts,
        "lang": lang,
        "install_id": install_id,
        "reading_seconds": reading_seconds,
    });

    let resp = client
        .post(SERVER_URL)
        .header("x-app-secret", APP_SECRET)
        .json(&body)
        .send()
        .map_err(|e| StoryError::Network(e.to_string()))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => StoryError::Unauthorized,
            429 => StoryError::RateLimited,
            other => StoryError::Server(format!("HTTP {other}")),
        });
    }

    let parsed: StoryResponse = resp.json().map_err(|e| StoryError::Server(e.to_string()))?;

    let story = aparar_frase_incompleta(&parsed.story);
    if story.is_empty() {
        return Err(StoryError::Empty);
    }
    Ok(story)
}

#[derive(Deserialize)]
struct PreRaceResponse {
    headline: String,
    body: String,
    team_voice: String,
}

/// Prévia pré-corrida: manchete + corpo (cinematográfico) + voz da equipe. Mesmo
/// contrato do boletim (segredo no header, cooldown próprio no servidor). Em QUALQUER
/// erro o chamador cai no template atual da Sala de Estratégia.
pub struct PreRaceBriefing {
    pub headline: String,
    pub body: String,
    pub team_voice: String,
}

pub fn fetch_pre_race_briefing(
    facts: &str,
    lang: &str,
    install_id: &str,
    force: bool,
) -> Result<PreRaceBriefing, StoryError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
        .map_err(|e| StoryError::Network(e.to_string()))?;

    let body = serde_json::json!({
        "facts": facts,
        "lang": lang,
        "install_id": install_id,
        // Reroll de debug: pula o cooldown no servidor (sem regravá-lo).
        "force": force,
    });

    let resp = client
        .post(PRE_RACE_URL)
        .header("x-app-secret", APP_SECRET)
        .json(&body)
        .send()
        .map_err(|e| StoryError::Network(e.to_string()))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => StoryError::Unauthorized,
            429 => StoryError::RateLimited,
            other => StoryError::Server(format!("HTTP {other}")),
        });
    }

    let parsed: PreRaceResponse = resp.json().map_err(|e| StoryError::Server(e.to_string()))?;

    let headline = parsed.headline.trim().to_string();
    let body = aparar_frase_incompleta(&parsed.body);
    let team_voice = parsed.team_voice.trim().to_string();
    if headline.is_empty() || body.is_empty() || team_voice.is_empty() {
        return Err(StoryError::Empty);
    }
    Ok(PreRaceBriefing {
        headline,
        body,
        team_voice,
    })
}

#[derive(Deserialize)]
struct PostRaceResponse {
    headline: String,
    body: String,
}

/// Debrief pós-corrida do engenheiro: manchete + parágrafo (2ª pessoa, com calor).
/// Voz ÚNICA (sem imprensa). Mesmo contrato dos demais (segredo no header, cooldown
/// próprio no servidor). Em QUALQUER erro o chamador cai no texto determinístico.
pub struct PostRaceDebrief {
    pub headline: String,
    pub body: String,
}

pub fn fetch_post_race_debrief(
    facts: &str,
    lang: &str,
    install_id: &str,
    force: bool,
) -> Result<PostRaceDebrief, StoryError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
        .map_err(|e| StoryError::Network(e.to_string()))?;

    let body = serde_json::json!({
        "facts": facts,
        "lang": lang,
        "install_id": install_id,
        "force": force,
    });

    let resp = client
        .post(POST_RACE_URL)
        .header("x-app-secret", APP_SECRET)
        .json(&body)
        .send()
        .map_err(|e| StoryError::Network(e.to_string()))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => StoryError::Unauthorized,
            429 => StoryError::RateLimited,
            other => StoryError::Server(format!("HTTP {other}")),
        });
    }

    let parsed: PostRaceResponse = resp.json().map_err(|e| StoryError::Server(e.to_string()))?;

    let headline = parsed.headline.trim().to_string();
    let body = aparar_frase_incompleta(&parsed.body);
    if headline.is_empty() || body.is_empty() {
        return Err(StoryError::Empty);
    }
    Ok(PostRaceDebrief { headline, body })
}

/// Matéria "O Que Esperar": manchete + linha-fina + corpo (3ª pessoa, voz de revista).
/// O corpo vem em parágrafos separados por linha em branco. Em QUALQUER erro — inclusive
/// o endpoint `/season-preview` ainda não existir no servidor — o chamador cai no
/// montador determinístico (`deterministic_article`), nunca numa tela vazia.
pub struct SeasonPreview {
    pub headline: String,
    pub standfirst: String,
    pub body: String,
}

#[derive(Deserialize)]
struct SeasonPreviewResponse {
    headline: String,
    standfirst: String,
    body: String,
}

pub fn fetch_season_preview(
    facts: &str,
    lang: &str,
    install_id: &str,
) -> Result<SeasonPreview, StoryError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
        .map_err(|e| StoryError::Network(e.to_string()))?;

    let body = serde_json::json!({
        "facts": facts,
        "lang": lang,
        "install_id": install_id,
        "target_words": { "min": SEASON_PREVIEW_TARGET_WORDS.0, "max": SEASON_PREVIEW_TARGET_WORDS.1 },
    });

    let resp = client
        .post(SEASON_PREVIEW_URL)
        .header("x-app-secret", APP_SECRET)
        .json(&body)
        .send()
        .map_err(|e| StoryError::Network(e.to_string()))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => StoryError::Unauthorized,
            429 => StoryError::RateLimited,
            other => StoryError::Server(format!("HTTP {other}")),
        });
    }

    let parsed: SeasonPreviewResponse =
        resp.json().map_err(|e| StoryError::Server(e.to_string()))?;

    let headline = parsed.headline.trim().to_string();
    let standfirst = parsed.standfirst.trim().to_string();
    // O corpo é o que não pode faltar; manchete/linha-fina vazias o front tolera.
    let body = aparar_frase_incompleta(&parsed.body);
    if body.is_empty() {
        return Err(StoryError::Empty);
    }
    Ok(SeasonPreview {
        headline,
        standfirst,
        body,
    })
}

#[derive(Deserialize)]
struct WorldNotesResponse {
    notes: Vec<String>,
}

/// Reescreve as notinhas do rodapé "Do mundo do Grid" em voz de revista (3ª pessoa,
/// jornalística). Recebe os fatos crus (`[kind] assunto — texto`, um por linha) e
/// devolve UMA string reescrita por fato, na MESMA ordem. Mesmo contrato dos demais
/// (segredo no header, cooldown/teto no servidor). Em QUALQUER erro — inclusive o
/// endpoint ainda não existir no servidor — o chamador mantém o texto determinístico.
pub fn fetch_world_notes(
    facts: &str,
    lang: &str,
    install_id: &str,
) -> Result<Vec<String>, StoryError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
        .map_err(|e| StoryError::Network(e.to_string()))?;

    let body = serde_json::json!({
        "facts": facts,
        "lang": lang,
        "install_id": install_id,
    });

    let resp = client
        .post(WORLD_NOTES_URL)
        .header("x-app-secret", APP_SECRET)
        .json(&body)
        .send()
        .map_err(|e| StoryError::Network(e.to_string()))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => StoryError::Unauthorized,
            429 => StoryError::RateLimited,
            other => StoryError::Server(format!("HTTP {other}")),
        });
    }

    let parsed: WorldNotesResponse = resp.json().map_err(|e| StoryError::Server(e.to_string()))?;

    let notes: Vec<String> = parsed
        .notes
        .into_iter()
        .map(|s| s.trim().to_string())
        .collect();
    if notes.is_empty() || notes.iter().any(|s| s.is_empty()) {
        return Err(StoryError::Empty);
    }
    Ok(notes)
}
