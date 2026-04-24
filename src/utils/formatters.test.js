import { describe, expect, it } from "vitest";

import { extractFlag, extractNationalityCode, formatNextRaceCountdown } from "./formatters";

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
});
