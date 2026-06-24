//! Cliente do servidor de boletins (proxy do Gemini).
//!
//! O app NUNCA fala com o Gemini direto — só com o NOSSO servidor (Cloud Run),
//! que guarda a chave. O segredo abaixo é embutido por design (client-side): ele
//! vai no binário e serve só como "porta de entrada" do servidor; a defesa real
//! contra abuso é o cooldown por install_id + o teto global de gasto.

use serde::Deserialize;
use std::time::Duration;

const SERVER_URL: &str =
    "https://iracer-news-124606451488.southamerica-east1.run.app/race-story";
/// Endpoint da prévia pré-corrida (narrativa + voz da equipe, curtas).
const PRE_RACE_URL: &str =
    "https://iracer-news-124606451488.southamerica-east1.run.app/pre-race";
const APP_SECRET: &str = "827119cc235cdea25c04545cd283749e673917d2d424340fb1059925738efcef";
// 45s e não 20s: o servidor (Cloud Run) faz scale-to-zero quando ocioso, e a 1ª
// chamada após um período parado paga um cold start (subir o container + init do
// Firestore) ANTES de gerar. Quente, gera em ~3s; frio pode passar de 20s. 45s dá
// folga pra um cold start caber sem o cliente desistir e cair no template.
const TIMEOUT_SECS: u64 = 45;

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

    let parsed: StoryResponse = resp
        .json()
        .map_err(|e| StoryError::Server(e.to_string()))?;

    let story = parsed.story.trim().to_string();
    if story.is_empty() {
        return Err(StoryError::Empty);
    }
    Ok(story)
}

#[derive(Deserialize)]
struct PreRaceResponse {
    narrative: String,
    team_voice: String,
}

/// Prévia pré-corrida: narrativa + voz da equipe, ambas CURTAS. Mesmo contrato do
/// boletim (segredo no header, cooldown próprio no servidor). Em QUALQUER erro o
/// chamador cai no template atual da Sala de Estratégia.
pub struct PreRaceBriefing {
    pub narrative: String,
    pub team_voice: String,
}

pub fn fetch_pre_race_briefing(
    facts: &str,
    lang: &str,
    install_id: &str,
) -> Result<PreRaceBriefing, StoryError> {
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

    let parsed: PreRaceResponse = resp
        .json()
        .map_err(|e| StoryError::Server(e.to_string()))?;

    let narrative = parsed.narrative.trim().to_string();
    let team_voice = parsed.team_voice.trim().to_string();
    if narrative.is_empty() || team_voice.is_empty() {
        return Err(StoryError::Empty);
    }
    Ok(PreRaceBriefing {
        narrative,
        team_voice,
    })
}
