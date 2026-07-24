//! Gate de engajamento da prévia pré-corrida: só gasta IA com quem lê.

/// Chave no `meta` (career.db) com a sequência de prévias pré-corrida que o jogador
/// NÃO leu seguidas. Usada para só gastar IA com quem lê.
pub(crate) const PRE_RACE_STREAK_KEY: &str = "pre_race_unread_streak";

/// Decide se a prévia da próxima corrida deve usar IA, a partir da sequência de
/// "não-leu". 0 = vinha lendo → IA; 1 = alterna p/ template; 2 = mais uma chance de
/// IA; ≥3 = ignorou 3 seguidas → só template. Qualquer leitura zera a sequência.
pub(crate) fn pre_race_use_ai(unread_streak: i64) -> bool {
    unread_streak == 0 || unread_streak == 2
}
