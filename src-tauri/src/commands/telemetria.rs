//! Ponte do front para a telemetria de produto.
//!
//! O que o backend não consegue observar sozinho: quanto tempo o jogador passou nas
//! telas que custam geração de IA. Corrida, largada, PTT e abertura do app têm bordas
//! no Rust; permanência em tela, não.
//!
//! Só o TEMPO atravessa esta ponte. Nada do conteúdo lido, nada de qual matéria, nada
//! de qual notícia. E só as três telas que custam servidor — ver
//! [`crate::telemetry::uso_tela`], que ignora em silêncio qualquer outro nome, para
//! uma tela nova não virar métrica sem passar por uma decisão.

/// Soma tempo de permanência numa das três telas pagas.
///
/// `tela` é `noticias` | `briefing` | `debriefing`. Chamada pelo front ao sair da tela
/// e também de tempos em tempos enquanto ela está aberta, para uma leitura longa não
/// se perder se o app for fechado no meio.
///
/// Nunca falha: telemetria que devolve erro vira erro na tela do jogador, e nenhuma
/// decisão de produto vale isso.
#[tauri::command]
pub fn telemetria_tela(tela: String, segundos: i64) {
    crate::telemetry::uso_tela(&tela, segundos);
}
