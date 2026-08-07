// Etapa 7 — estatística da bateria, e Etapa 8 — o veredito.
//
// A média sozinha mente aqui: uma chamada de 4 segundos no meio de vinte de 700 ms
// some na média e destrói a experiência de quem jogou. Por isso o corpo do relatório
// é mediana + P90 + P95 + as faixas de "quantas falas começaram abaixo de X".

/** Percentil por posto mais próximo (nearest-rank). `p` em 0..100. */
export function percentil(valores, p) {
  const ordenado = [...valores].filter((v) => Number.isFinite(v)).sort((a, b) => a - b);
  if (ordenado.length === 0) return null;
  const posto = Math.ceil((p / 100) * ordenado.length);
  return ordenado[Math.min(ordenado.length - 1, Math.max(0, posto - 1))];
}

export function mediana(valores) {
  return percentil(valores, 50);
}

export function media(valores) {
  const v = valores.filter((x) => Number.isFinite(x));
  if (v.length === 0) return null;
  return v.reduce((a, b) => a + b, 0) / v.length;
}

/** Fração (0..1) dos valores abaixo do limite. */
export function fracaoAbaixo(valores, limiteMs) {
  const v = valores.filter((x) => Number.isFinite(x));
  if (v.length === 0) return null;
  return v.filter((x) => x < limiteMs).length / v.length;
}

export function fracaoAcima(valores, limiteMs) {
  const v = valores.filter((x) => Number.isFinite(x));
  if (v.length === 0) return null;
  return v.filter((x) => x > limiteMs).length / v.length;
}

/**
 * Resume um conjunto de registros. `chave` é a métrica em foco — por padrão o tempo
 * até o primeiro som, que é o único número que o jogador sente.
 */
export function resumir(registros, chave = "msPrimeiroSom") {
  const total = registros.length;
  const ok = registros.filter((r) => r.sucesso);
  const valores = ok.map((r) => r[chave]).filter((v) => Number.isFinite(v));

  return {
    total,
    sucessos: ok.length,
    falhas: total - ok.length,
    percentualFalhas: total === 0 ? null : (total - ok.length) / total,
    melhor: valores.length ? Math.min(...valores) : null,
    pior: valores.length ? Math.max(...valores) : null,
    media: media(valores),
    mediana: mediana(valores),
    p90: percentil(valores, 90),
    p95: percentil(valores, 95),
    abaixo1000: fracaoAbaixo(valores, 1000),
    abaixo1500: fracaoAbaixo(valores, 1500),
    abaixo2000: fracaoAbaixo(valores, 2000),
    acima3000: fracaoAcima(valores, 3000),
    interrupcoes: ok.reduce((soma, r) => soma + (r.interrupcoes || 0), 0),
  };
}

export const VEREDITOS = {
  excelente: {
    id: "excelente",
    rotulo: "Excelente — resposta dinâmica em corrida",
    detalhe: "Mediana < 800 ms, P95 < 1,5 s e praticamente sem falhas.",
  },
  viavel: {
    id: "viavel",
    rotulo: "Muito viável — resposta dinâmica",
    detalhe: "Mediana < 1,3 s, P95 < 2 s, reprodução contínua.",
  },
  antecipada: {
    id: "antecipada",
    rotulo: "Viável apenas para falas antecipadas",
    detalhe: "Mediana entre 1,3 s e 2 s, P95 entre 2 s e 3 s.",
  },
  inadequado: {
    id: "inadequado",
    rotulo: "Inadequado para geração durante a corrida",
    detalhe: "Mediana acima de 2,5 s, P95 inconsistente ou muitas falhas.",
  },
  indefinido: {
    id: "indefinido",
    rotulo: "Amostra insuficiente",
    detalhe: "Rode a bateria completa antes de concluir qualquer coisa.",
  },
};

/**
 * Etapa 8 — aplica os cortes combinados. Deliberadamente severo: qualquer critério
 * violado rebaixa a faixa, porque a experiência é ditada pelo pior caso frequente e
 * não pelo melhor caso ocasional.
 */
export function classificar(resumo, { minimoAmostras = 10 } = {}) {
  if (!resumo || resumo.sucessos < minimoAmostras) return VEREDITOS.indefinido;

  const { mediana: med, p95, percentualFalhas, interrupcoes } = resumo;
  if (med == null || p95 == null) return VEREDITOS.indefinido;

  const muitasFalhas = (percentualFalhas ?? 0) > 0.05;
  const reproducaoInstavel = interrupcoes > resumo.sucessos * 0.2;

  if (muitasFalhas || med > 2500 || p95 > 4000) return VEREDITOS.inadequado;
  if (med < 800 && p95 < 1500 && (percentualFalhas ?? 0) <= 0.01 && !reproducaoInstavel) {
    return VEREDITOS.excelente;
  }
  if (med < 1300 && p95 < 2000 && !reproducaoInstavel) return VEREDITOS.viavel;
  if (med <= 2000 && p95 <= 3000) return VEREDITOS.antecipada;
  return VEREDITOS.inadequado;
}

/**
 * Classifica a chamada em fria/morna/quente pelo tempo desde a anterior. A primeira
 * chamada de um processo costuma pagar handshake TLS, DNS e alocação do modelo — sem
 * separar isso, a mediana da bateria fica contaminada.
 */
export function faseDaChamada({ primeiraDoProcesso, msDesdeUltima }) {
  if (primeiraDoProcesso) return "primeira";
  if (msDesdeUltima == null || msDesdeUltima >= 120000) return "fria";
  if (msDesdeUltima >= 20000) return "morna";
  return "sequencia";
}

export function formatarMs(v) {
  if (v == null || !Number.isFinite(v)) return "—";
  return `${Math.round(v)} ms`;
}

export function formatarPercentual(v) {
  if (v == null || !Number.isFinite(v)) return "—";
  return `${(v * 100).toFixed(0)}%`;
}
