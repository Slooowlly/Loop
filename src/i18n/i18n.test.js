import { describe, it, expect, beforeEach } from "vitest";
import i18n, { applyLanguage, SUPPORTED_LANGUAGES, DEFAULT_LANGUAGE } from "./index.js";
import { ordinal, formatCompactDate, formatNumber } from "./format.js";

describe("i18n (Fase 0)", () => {
  beforeEach(() => {
    i18n.changeLanguage(DEFAULT_LANGUAGE);
  });

  it("troca a UI estática entre PT e EN via applyLanguage", () => {
    applyLanguage("pt-BR");
    expect(i18n.t("settings.language.label")).toBe("Idioma");

    applyLanguage("en-US");
    expect(i18n.t("settings.language.label")).toBe("Language");
    expect(i18n.t("settings.general")).toBe("General");
  });

  it("cai no default quando o idioma é desconhecido", () => {
    const applied = applyLanguage("xx-YY");
    expect(applied).toBe(DEFAULT_LANGUAGE);
    expect(SUPPORTED_LANGUAGES).toContain(DEFAULT_LANGUAGE);
  });

  it("ordinal é genérico por locale (pt: º, en: sufixo)", () => {
    expect(ordinal(1, "pt-BR")).toBe("1º");
    expect(ordinal(12, "pt-BR")).toBe("12º");
    expect(ordinal(1, "en-US")).toBe("1st");
    expect(ordinal(2, "en-US")).toBe("2nd");
    expect(ordinal(3, "en-US")).toBe("3rd");
    expect(ordinal(11, "en-US")).toBe("11th");
    expect(ordinal(23, "en-US")).toBe("23rd");
  });

  it("data e número seguem o locale", () => {
    const d = new Date(2025, 11, 30); // 30 dez 2025
    expect(formatCompactDate(d, "pt-BR")).toBe("30/12/2025");
    expect(formatCompactDate(d, "en-US")).toBe("12/30/2025");
    expect(formatNumber(1234567, {}, "pt-BR")).toBe("1.234.567");
    expect(formatNumber(1234567, {}, "en-US")).toBe("1,234,567");
  });
});
