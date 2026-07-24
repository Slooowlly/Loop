//! DTOs de retorno dos comandos de IA — o que cruza a ponte para o front.

use super::*;

#[derive(Serialize)]
pub struct AiNewsResult {
    /// O boletim redigido, se disponível. `None` → front usa o texto padrão.
    pub story: Option<String>,
    /// ok | cached | unavailable | rate_limited | error
    pub status: String,
    /// Mapa nome da equipe → cor primária das equipes da corrida (p/ colorir os
    /// nomes no boletim). `None` se a notícia não tem fatos de IA.
    pub teams: Option<serde_json::Value>,
}

/// Resultado da prévia pré-corrida por IA (Sala de Estratégia). `None` nos textos →
/// o front cai no template atual (narrativa + voz da equipe geradas localmente).
#[derive(Serialize)]
pub struct PreRaceAiResult {
    /// Manchete cinematográfica (negrito no card). `None` → front usa o template.
    pub headline: Option<String>,
    /// Corpo da prévia (1-2 parágrafos).
    pub narrative: Option<String>,
    pub team_voice: Option<String>,
    /// ok | cached | rate_limited | unavailable | error
    pub status: String,
}

// ─── Debrief pós-corrida do engenheiro (voz única, com calor) ────────────────────

#[derive(Serialize)]
pub struct PostRaceAiResult {
    /// Manchete do debrief. `None` → front usa o texto determinístico (cérebro).
    pub headline: Option<String>,
    /// Parágrafo do engenheiro (2ª pessoa). `None` → front usa o determinístico.
    pub body: Option<String>,
    /// ok | cached | unavailable | rate_limited | error
    pub status: String,
}
