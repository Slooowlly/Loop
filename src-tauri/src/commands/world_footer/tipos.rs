//! DTOs do rodapé: a notinha em si e os resultados dos dois comandos.

use super::*;

/// Uma notinha do rodapé. `tone` guia o acento visual; `tag` é o rótulo temático.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldNote {
    /// Chave estável (key de lista / dedup no front).
    pub id: String,
    /// Rótulo temático da revista: MERCADO | FINANÇAS | BASTIDORES | RECORDE.
    pub tag: String,
    /// Nome da equipe ou piloto de quem a nota fala.
    pub subject: String,
    /// Categoria de estado (máquina).
    pub kind: String,
    /// "crise" | "alerta" | "reforma" | "recorde" | "neutro" — acento visual.
    pub tone: String,
    /// Texto PT jornalístico (fallback determinístico, sem 2ª pessoa).
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct WorldFooterResult {
    pub notes: Vec<WorldNote>,
    /// "template" (determinístico) | "ai" (reescrito pelo servidor, futuro).
    pub source: String,
    /// Fatos crus (uma linha por nota) — reservados para a reescrita por IA.
    pub facts: String,
}

/// Resultado da reescrita por IA do rodapé. `notes` só vem preenchido quando a IA
/// respondeu e casou 1-para-1 com o template; caso contrário o front mantém o texto
/// determinístico que já recebeu de `get_world_footer`.
#[derive(Debug, Serialize)]
pub struct WorldFooterAiResult {
    pub notes: Option<Vec<WorldNote>>,
    /// "ai" | "template".
    pub source: String,
    /// ok | cached | unavailable | rate_limited | error | mismatch
    pub status: String,
}
