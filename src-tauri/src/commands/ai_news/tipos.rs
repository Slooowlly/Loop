//! DTOs de retorno dos comandos de IA — o que cruza a ponte para o front.

use super::*;

/// Como terminou a tentativa de gerar texto por IA.
///
/// Era `String` livre nos três DTOs, com o vocabulário escrito só num comentário: um typo
/// de um lado ou um caso novo esquecido do outro degradava para o template em silêncio,
/// porque "status desconhecido" e "não deu" têm o mesmo efeito na tela.
///
/// O JSON que cruza a ponte é EXATAMENTE o mesmo de antes (`snake_case`), então o front
/// não muda; o que muda é o compilador passar a recusar um valor fora da lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiStatus {
    /// Gerado agora pelo servidor.
    Ok,
    /// Veio do cache do save, sem tocar no servidor.
    Cached,
    /// Não há o que gerar (sem fatos guardados para esta etapa/notícia).
    Unavailable,
    /// O servidor recusou por cooldown/cota.
    RateLimited,
    /// Falhou (rede, timeout, resposta inválida).
    Error,
    /// O gate de engajamento decidiu não pagar a geração: o template basta aqui.
    EngagementTemplate,
}

#[derive(Serialize)]
pub struct AiNewsResult {
    /// O boletim redigido, se disponível. `None` → front usa o texto padrão.
    pub story: Option<String>,
    pub status: AiStatus,
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
    pub status: AiStatus,
}

// ─── Debrief pós-corrida do engenheiro (voz única, com calor) ────────────────────

#[derive(Serialize)]
pub struct PostRaceAiResult {
    /// Manchete do debrief. `None` → front usa o texto determinístico (cérebro).
    pub headline: Option<String>,
    /// Parágrafo do engenheiro (2ª pessoa). `None` → front usa o determinístico.
    pub body: Option<String>,
    pub status: AiStatus,
}
