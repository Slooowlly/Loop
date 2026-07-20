import { describe, it, expect, afterAll } from "vitest";
import i18n, { DEFAULT_LANGUAGE } from "../i18n/index.js";
import {
  difficultyLabel,
  formatLicenseLevel,
  formatSeasonPhase,
  formatPreseasonPhase,
  formatAttributeName,
  formatNextRaceCountdown,
  formatSurfaceSeasonLabel,
  formatSalaryMonthly,
} from "./formatters.js";

function withLang(lang, fn) {
  i18n.changeLanguage(lang);
  try {
    return fn();
  } finally {
    i18n.changeLanguage(DEFAULT_LANGUAGE);
  }
}

afterAll(() => i18n.changeLanguage(DEFAULT_LANGUAGE));

describe("formatters i18n (Fase 1 — mapas de label)", () => {
  it("PT é o baseline (default pt-BR)", () => {
    expect(difficultyLabel("lendario")).toBe("Lendário");
    expect(formatLicenseLevel(0)).toBe("Amadora");
    expect(formatLicenseLevel(-1)).toBe("Sem licença");
    expect(formatSeasonPhase("PreTemporada")).toBe("Pré-Temporada");
    expect(formatPreseasonPhase("Transfers")).toBe("Transferências");
    expect(formatAttributeName("gestao_pneus")).toBe("Pneus");
    expect(formatSurfaceSeasonLabel({ ano: 2025 })).toBe("Ano 2025");
  });

  it("troca pra EN", () => {
    withLang("en-US", () => {
      expect(difficultyLabel("lendario")).toBe("Legendary");
      expect(formatLicenseLevel(0)).toBe("Amateur");
      expect(formatLicenseLevel(9)).toBe("No license");
      expect(formatSeasonPhase("PreTemporada")).toBe("Preseason");
      expect(formatPreseasonPhase("Transfers")).toBe("Transfers");
      expect(formatAttributeName("gestao_pneus")).toBe("Tires");
      expect(formatSurfaceSeasonLabel({ year: 2025 })).toBe("Year 2025");
    });
  });

  it("countdown com plural por locale", () => {
    expect(formatNextRaceCountdown(null)).toBe("Sem corrida pendente");
    expect(formatNextRaceCountdown(0)).toBe("Próxima corrida hoje");
    expect(formatNextRaceCountdown(1)).toBe("Próxima corrida amanhã");
    expect(formatNextRaceCountdown(3)).toBe("Próxima corrida em 3 dias");
    expect(formatNextRaceCountdown(14)).toBe("Próxima corrida em 2 semanas");
    expect(formatNextRaceCountdown(90)).toBe("Próxima corrida em 3 meses");
    withLang("en-US", () => {
      expect(formatNextRaceCountdown(3)).toBe("Next race in 3 days");
      expect(formatNextRaceCountdown(8)).toBe("Next race in 1 week");
      expect(formatNextRaceCountdown(90)).toBe("Next race in 3 months");
    });
  });

  it("sufixo de salário mensal por locale", () => {
    expect(formatSalaryMonthly(120000)).toContain("/mês");
    withLang("en-US", () => {
      expect(formatSalaryMonthly(120000)).toContain("/mo");
    });
  });

  it("fallback: chave desconhecida devolve o valor cru", () => {
    expect(difficultyLabel("xyz")).toBe("xyz");
    expect(formatSeasonPhase("Desconhecida")).toBe("Desconhecida");
  });
});
