//! DTO do rádio da equipe (mensagens do engenheiro).

use serde::Serialize;

/// Uma mensagem do overlay do engenheiro sobre uma quebra na grade.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BreakdownMessage {
    /// Índice no log da corrida — cursor crescente pro front saber o que é NOVO.
    pub(crate) id: usize,
    /// "light" | "heavy" | "dnf" — tinge o acento do card.
    pub(crate) severity: String,
    /// Frase principal (voz do engenheiro, 3ª pessoa sobre o piloto).
    pub(crate) text: String,
    /// Detalhe concreto do problema (o `problem_label`), como subtítulo.
    pub(crate) detail: String,
}
