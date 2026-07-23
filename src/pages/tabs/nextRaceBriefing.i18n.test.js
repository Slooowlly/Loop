import { describe, it, expect, afterEach } from "vitest";
import i18n, { DEFAULT_LANGUAGE } from "../../i18n/index.js";
import { buildFavoriteExpectation, FAVORITE_EXPECTATION_POOLS } from "./nextRaceBriefing.js";

afterEach(() => i18n.changeLanguage(DEFAULT_LANGUAGE));

function makeDriver(overrides = {}) {
  return { id: "d1", rating: 90, posicao_campeonato: 1, results: [{ position: 1, is_dnf: false }], ...overrides };
}

describe("nextRaceBriefing i18n (Fase 2 — banco de expectativa)", () => {
  it("mesmo id, texto por locale (getter não congela)", () => {
    const [phrase] = FAVORITE_EXPECTATION_POOLS.p1;
    i18n.changeLanguage("pt-BR");
    const pt = phrase.text;
    i18n.changeLanguage("en-US");
    const en = phrase.text;
    expect(pt).not.toBe(en);
    expect(pt.length).toBeGreaterThan(0);
    expect(en.length).toBeGreaterThan(0);
  });

  it("buildFavoriteExpectation devolve EN quando o locale é en-US", () => {
    i18n.changeLanguage("en-US");
    const text = buildFavoriteExpectation(makeDriver(), 0);
    // Uma frase de p1 em inglês contém pelo menos uma destas âncoras.
    expect(text).toMatch(/reference|front|pace|benchmark|control|round/i);
  });

  it("estrutura intacta: 5 pools de 10 frases com ids únicos", () => {
    expect(Object.values(FAVORITE_EXPECTATION_POOLS)).toHaveLength(5);
    Object.values(FAVORITE_EXPECTATION_POOLS).forEach((pool) => {
      expect(pool).toHaveLength(10);
      expect(new Set(pool.map((p) => p.id)).size).toBe(10);
    });
  });
});
