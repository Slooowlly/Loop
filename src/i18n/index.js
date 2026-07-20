// ── i18n (frontend) — Fase 0 da tradução ─────────────────────────────────────
//
// Fonte única de verdade do idioma = `config.language` (config.json, lido no boot
// via get_config). O store (useCareerStore) empurra o idioma pra cá em dois pontos:
// loadLanguage() (boot/loadCareer) e setLanguage() (troca no Settings).
//
// PT é o locale-base: enquanto a extração da Fase 1 não roda, o pt-BR carrega o
// texto atual e nada muda na tela. Adicionar um idioma novo (es/de/fr/it) = só
// somar a pasta de locale — nenhuma lógica é hardcoded em "pt".
//
// Plural e gênero NÃO têm helper próprio de propósito: o i18next resolve os dois
// de forma genérica por locale (Intl.PluralRules pro plural; `context` pro gênero),
// então idiomas novos entram sem tocar em código.

import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import ptBRCommon from "./locales/pt-BR/common.json";
import enUSCommon from "./locales/en-US/common.json";

export const SUPPORTED_LANGUAGES = ["pt-BR", "en-US"];
export const DEFAULT_LANGUAGE = "pt-BR";

// Um namespace por área de UI (common, settings, nav, race, market, ...). Por ora
// só `common` existe; a Fase 1 vai adicionando os demais arquivo a arquivo.
export const resources = {
  "pt-BR": { common: ptBRCommon },
  "en-US": { common: enUSCommon },
};

i18n.use(initReactI18next).init({
  resources,
  lng: DEFAULT_LANGUAGE,
  fallbackLng: DEFAULT_LANGUAGE,
  supportedLngs: SUPPORTED_LANGUAGES,
  ns: ["common"],
  defaultNS: "common",
  interpolation: {
    escapeValue: false, // React já escapa; e vários textos têm <b>/<p> internos.
  },
  returnNull: false,
  returnEmptyString: false,
});

// Troca o idioma ativo, validando contra os suportados (default se desconhecido).
export function applyLanguage(language) {
  const lang = SUPPORTED_LANGUAGES.includes(language) ? language : DEFAULT_LANGUAGE;
  if (i18n.language !== lang) {
    void i18n.changeLanguage(lang);
  }
  return lang;
}

export default i18n;
