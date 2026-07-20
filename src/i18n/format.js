// ── Formatação sensível a locale ─────────────────────────────────────────────
//
// Genérico por locale: recebe (valor, lang) e delega ao Intl. Nada hardcoded em
// "pt". A Fase 5 troca as chamadas espalhadas (toLocaleDateString("pt-BR"),
// `${n}º`, etc.) por estes helpers.
//
// Moeda NÃO entra aqui: já é USD/en-US em todo o app por decisão (fonte única em
// utils/formatters.js). Não mexer.

import i18n, { DEFAULT_LANGUAGE } from "./index.js";

// Idioma ativo (do i18next), com fallback seguro.
export function currentLang() {
  return i18n.language || DEFAULT_LANGUAGE;
}

// Ordinal por locale: 12º (pt) / 12th (en). Português usa o indicador "º"
// (posições em corrida são masculinas: "12º colocado"). Inglês usa sufixo por
// regra do Intl.PluralRules ordinal.
const EN_ORDINAL_SUFFIX = { one: "st", two: "nd", few: "rd", other: "th" };

export function ordinal(n, lang = currentLang()) {
  if (n == null || Number.isNaN(Number(n))) return String(n ?? "");
  const num = Number(n);
  if (lang.startsWith("pt")) return `${num}º`;
  if (lang.startsWith("en")) {
    const rule = new Intl.PluralRules("en", { type: "ordinal" }).select(num);
    return `${num}${EN_ORDINAL_SUFFIX[rule] ?? "th"}`;
  }
  // Fallback neutro pra idiomas latinos futuros (es/it/fr usam "º"/"e"/etc.);
  // por ora, número puro até definirmos a regra de cada um.
  return `${num}º`;
}

// Data por locale. Aceita Date, epoch ms, ou string ISO.
export function formatDate(value, options = {}, lang = currentLang()) {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return new Intl.DateTimeFormat(lang, options).format(date);
}

// Data compacta numérica (substitui o DD/MM/YYYY hardcoded). O Intl escolhe a
// ordem certa por locale (30/12/2025 em pt-BR, 12/30/2025 em en-US).
export function formatCompactDate(value, lang = currentLang()) {
  return formatDate(value, { day: "2-digit", month: "2-digit", year: "numeric" }, lang);
}

// Número com agrupamento por locale (substitui toLocaleString("pt-BR")).
export function formatNumber(value, options = {}, lang = currentLang()) {
  if (value == null || Number.isNaN(Number(value))) return String(value ?? "");
  return new Intl.NumberFormat(lang, options).format(Number(value));
}

// Comparador de ordenação por locale (substitui localeCompare(..., "pt-BR")).
export function localeCompare(a, b, lang = currentLang()) {
  return String(a).localeCompare(String(b), lang);
}

// Nomes de mês por extenso, capitalizados (Intl pt devolve minúsculo). Índice 0=Jan.
export function monthLongLabels(lang = currentLang()) {
  const fmt = new Intl.DateTimeFormat(lang, { month: "long" });
  return Array.from({ length: 12 }, (_, m) => {
    const s = fmt.format(new Date(2000, m, 1));
    return s.charAt(0).toUpperCase() + s.slice(1);
  });
}

// Iniciais dos dias da semana começando no DOMINGO (2023-01-01 foi um domingo).
export function weekdayNarrowLabels(lang = currentLang()) {
  const fmt = new Intl.DateTimeFormat(lang, { weekday: "narrow" });
  return Array.from({ length: 7 }, (_, d) => fmt.format(new Date(2023, 0, 1 + d)));
}
