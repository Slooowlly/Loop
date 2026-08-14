// Regra de fallback das telas de narrativa por IA (briefing, debrief, boletim, rodapé).
//
// Quando a IA NÃO produziu texto (cooldown, offline, servidor fora, endpoint ausente):
//   - Idioma PORTUGUÊS  → mostra o texto DETERMINÍSTICO em PT (o fallback "bonitinho").
//   - Outro idioma      → mostra "erro na geração de texto" NO idioma escolhido, em vez
//                         de despejar português para quem não joga em português.
//
// Assim não precisamos traduzir centenas de strings de template: só uma frase de erro
// por idioma. Quando o texto de IA existe, ele já vem no idioma certo (o `lang` é
// enviado ao servidor), então esta regra só vale para o caminho de fallback.

// i18n-ignore-file: ESTA TABELA É a tradução. Passar as frases daqui por `t()` seria pedir a
// tradução de dentro do caminho que existe justamente para quando a tradução não veio.
const AI_ERROR_TEXT = {
  "pt-BR": "Erro na geração de texto.",
  "pt-PT": "Erro na geração de texto.",
  "en-US": "Text generation failed.",
};

/// Português (qualquer variante) mostra o fallback determinístico; o resto, o erro.
export function isPortuguese(language) {
  return String(language ?? "pt-BR")
    .toLowerCase()
    .startsWith("pt");
}

/// Frase de erro localizada para quando não há texto de IA e o idioma não é português.
export function localizedAiError(language) {
  return AI_ERROR_TEXT[language] ?? AI_ERROR_TEXT["en-US"];
}

/// Conveniência: dado (temTextoIA, idioma), decide o que a tela deve fazer no caminho
/// SEM IA. `"template"` = renderizar o fallback determinístico (PT); `"error"` =
/// mostrar a frase de erro localizada.
export function fallbackMode(hasAiText, language) {
  if (hasAiText) return "ai";
  return isPortuguese(language) ? "template" : "error";
}
