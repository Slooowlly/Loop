import { describe, expect, it } from "vitest";

import {
  extractFlag,
  extractNationalityCode,
  formatLapSeconds,
  formatLapTime,
  formatNextRaceCountdown,
} from "./formatters";

// Os dois formatadores de volta compartilham o miolo "m:ss.mmm" e diferem só na
// unidade de entrada e na sentinela de "sem volta" — ver o comentário em formatters.js.
describe("formatação de tempo de volta", () => {
  it("formata milissegundos como m:ss.mmm", () => {
    expect(formatLapTime(83456)).toBe("1:23.456");
    expect(formatLapTime(59999)).toBe("0:59.999");
    expect(formatLapTime(600000)).toBe("10:00.000");
  });

  it("formata segundos como m:ss.mmm", () => {
    expect(formatLapSeconds(83.456)).toBe("1:23.456");
    expect(formatLapSeconds(59.999)).toBe("0:59.999");
    expect(formatLapSeconds(600)).toBe("10:00.000");
  });

  it("devolve a sentinela de cada um quando não há volta válida", () => {
    for (const invalido of [null, undefined, 0, -1, NaN, Infinity, -Infinity]) {
      expect(formatLapTime(invalido)).toBe("-");
      expect(formatLapSeconds(invalido)).toBe("--");
    }
  });
});

describe("formatNextRaceCountdown", () => {
  it("formats the countdown across months weeks and days", () => {
    expect(formatNextRaceCountdown(null)).toBe("Sem corrida pendente");
    expect(formatNextRaceCountdown(0)).toBe("Próxima corrida hoje");
    expect(formatNextRaceCountdown(1)).toBe("Próxima corrida amanhã");
    expect(formatNextRaceCountdown(6)).toBe("Próxima corrida em 6 dias");
    expect(formatNextRaceCountdown(14)).toBe("Próxima corrida em 2 semanas");
    expect(formatNextRaceCountdown(28)).toBe("Próxima corrida em 1 mês");
    expect(formatNextRaceCountdown(56)).toBe("Próxima corrida em 2 meses");
  });
});

describe("nationality flag formatting", () => {
  it("recognizes plain Portuguese country labels", () => {
    expect(extractNationalityCode("Brasil")).toBe("br");
    expect(extractFlag("Brasil")).toBe("\u{1F1E7}\u{1F1F7}");
    expect(extractNationalityCode("Portugal")).toBe("pt");
  });

  it("recognizes stored Argentine country-code labels", () => {
    expect(extractNationalityCode("AR Argentino")).toBe("ar");
    expect(extractFlag("AR Argentino")).toBe("\u{1F1E6}\u{1F1F7}");
  });
});
