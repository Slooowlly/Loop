import { describe, expect, it } from "vitest";

import {
  FAVORITE_EXPECTATION_POOLS,
  buildFavoriteExpectation,
  buildFavoriteExpectationSelection,
} from "./nextRaceBriefing";

function makeDriver(overrides = {}) {
  return {
    id: "driver-1",
    nome: "R. Silva",
    posicao_campeonato: 1,
    rating: 90,
    results: [
      { position: 1, is_dnf: false },
      { position: 2, is_dnf: false },
      { position: 1, is_dnf: false },
    ],
    ...overrides,
  };
}

describe("nextRaceBriefing", () => {
  it("exposes ten expectation templates for each grid position bucket", () => {
    expect(Object.values(FAVORITE_EXPECTATION_POOLS)).toHaveLength(5);

    Object.values(FAVORITE_EXPECTATION_POOLS).forEach((pool) => {
      expect(pool).toHaveLength(10);
      expect(new Set(pool.map((entry) => entry.id)).size).toBe(10);
    });
  });

  it("uses different expectation pools for each position in the top five", () => {
    expect(buildFavoriteExpectation(makeDriver(), 0)).toMatch(/referencia|frente|ritmo|parametro|controle/i);
    expect(buildFavoriteExpectation(makeDriver({ posicao_campeonato: 2, rating: 85 }), 1)).toMatch(
      /primeira fila|ataque|pressionar|frente/i,
    );
    expect(buildFavoriteExpectation(makeDriver({ posicao_campeonato: 4, rating: 80 }), 2)).toMatch(
      /p[oó]dio|bloco|frente/i,
    );
    expect(buildFavoriteExpectation(makeDriver({ posicao_campeonato: 5, rating: 76 }), 3)).toMatch(
      /top 5|erro|ameaca|oportunidade|p[oó]dio|pontos/i,
    );
    expect(buildFavoriteExpectation(makeDriver({ posicao_campeonato: 7, rating: 72 }), 4)).toMatch(
      /corre por fora|radar|oportunidade|surpresa|outsider|top 5/i,
    );
  });

  it("reacts to form context instead of returning the same line for every similar driver", () => {
    const controlled = buildFavoriteExpectation(
      makeDriver({
        posicao_campeonato: 1,
        rating: 92,
        results: [
          { position: 1, is_dnf: false },
          { position: 1, is_dnf: false },
          { position: 2, is_dnf: false },
        ],
      }),
      0,
    );
    const unstable = buildFavoriteExpectation(
      makeDriver({
        posicao_campeonato: 1,
        rating: 88,
        results: [
          { position: 7, is_dnf: false },
          { position: 2, is_dnf: false },
          { position: 9, is_dnf: false },
        ],
      }),
      0,
    );
    const dnfRisk = buildFavoriteExpectation(
      makeDriver({
        posicao_campeonato: 1,
        rating: 87,
        results: [
          { position: 2, is_dnf: false },
          { position: null, is_dnf: true },
          { position: 3, is_dnf: false },
        ],
      }),
      0,
    );

    expect(controlled).not.toBe(unstable);
    expect(dnfRisk).not.toBe(controlled);
    expect(dnfRisk).toMatch(/limpa|erro|conversao|volatil/i);
  });

  it("avoids repeating the last five phrases for the same driver and bucket when alternatives exist", () => {
    const recentEntries = FAVORITE_EXPECTATION_POOLS.p1.slice(0, 5).map((phrase) => ({
      season_number: 1,
      round_number: 10,
      driver_id: "driver-1",
      bucket_key: "p1",
      phrase_id: phrase.id,
    }));

    const selection = buildFavoriteExpectationSelection(makeDriver(), 0, {
      seasonNumber: 1,
      roundNumber: 11,
      historyEntries: recentEntries,
    });

    expect(recentEntries.map((entry) => entry.phrase_id)).not.toContain(selection.phraseId);
  });

  it("reuses the persisted phrase for the same round when the briefing is reopened", () => {
    const pinned = FAVORITE_EXPECTATION_POOLS.p2[4];

    const selection = buildFavoriteExpectationSelection(
      makeDriver({ id: "driver-2", posicao_campeonato: 2, rating: 84 }),
      1,
      {
        seasonNumber: 1,
        roundNumber: 8,
        historyEntries: [
          {
            season_number: 1,
            round_number: 8,
            driver_id: "driver-2",
            bucket_key: "p2",
            phrase_id: pinned.id,
          },
        ],
      },
    );

    expect(selection.phraseId).toBe(pinned.id);
    expect(selection.text).toBe(pinned.text);
  });
});
