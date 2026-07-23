import { describe, it, expect, afterEach } from "vitest";
import i18n, { DEFAULT_LANGUAGE } from "../../i18n/index.js";
import { selectThesis } from "./nextRaceThesis.js";

function withLang(lang, fn) {
  i18n.changeLanguage(lang);
  try {
    return fn();
  } finally {
    i18n.changeLanguage(DEFAULT_LANGUAGE);
  }
}

afterEach(() => i18n.changeLanguage(DEFAULT_LANGUAGE));

describe("nextRaceThesis i18n (Fase 2 — eixo/tese)", () => {
  it("baseline troca PT↔EN", () => {
    // championshipUnderway:true evita o eixo "debut" (estreia), caindo no baseline.
    const base = { trackName: "Monza", championshipUnderway: true };
    expect(selectThesis(base).statement).toMatch(/SOMAR E CRESCER/);
    withLang("en-US", () => {
      const t = selectThesis(base);
      expect(t.statement).toMatch(/BANK AND BUILD/);
      expect(t.statement).toContain("Monza");
      expect(t.title).toBe("Bank and build");
    });
  });

  it("redemption: EN mantém DNF + pista interpolada", () => {
    const signals = { lastResult: { is_dnf: true, trackName: "Spa" }, playerIsLeader: false };
    withLang("en-US", () => {
      const t = selectThesis(signals);
      expect(t.key).toBe("redemption");
      expect(t.statement).toMatch(/REACTION/);
      expect(t.statement).toContain("DNF");
      expect(t.statement).toContain("Spa");
    });
  });

  it("title_chase: atalho de plural (s) preservado em EN", () => {
    const signals = { championshipState: "chase", gapToLeader: 8, remainingRounds: 4, leaderName: "Novak" };
    withLang("en-US", () => {
      const t = selectThesis(signals);
      expect(t.statement).toContain("8 point(s)");
      expect(t.statement).toContain("(Novak)");
      expect(t.statement).toContain("4 round(s)");
    });
  });
});
